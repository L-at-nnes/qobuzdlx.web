use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};

use super::reporter::ItemReporter;
use super::{Inner, locked};
use crate::download::item::{ItemStatus, QueueItem};
use crate::download::job::{Job, JobError, JobSetup};
use crate::download::log::LogEvent;
use crate::download::progress::{Halt, Progress};
use crate::download::report::{Outcome, Reporter};
use crate::extensions::{self, ItemFinished};
use crate::settings::LogDetail;

/// Goes through the queue, one item at a time, whenever there is something to do and a session to
/// do it with.
pub(super) async fn run(inner: Arc<Inner>) {
    loop {
        tokio::select! {
            () = inner.shutdown.cancelled() => return,
            // The permit is kept if nobody waits: a wake-up is never lost
            () = inner.wake.notified() => {}
        }
        // Whoever woke the worker wants it to try again, with room made or not. A user's pause is
        // lifted by the user alone
        inner.halted.store(false, Ordering::SeqCst);
        inner.clear_halt(Halt::SignedOut);

        // Drained until nothing is left: a wake-up says there is work, not how much
        let mut processed = false;
        while !inner.shutdown.is_cancelled()
            && inner.client.is_signed_in()
            && !inner.paused.load(Ordering::SeqCst)
        {
            let Some(item) = inner.take_next() else {
                break;
            };
            processed = true;
            inner.process(item).await;
            // Nothing more can be written: every item that follows would fail the same way
            if inner.halted.load(Ordering::SeqCst) {
                break;
            }
        }

        if processed && !inner.shutdown.is_cancelled() {
            match inner.stop_reason() {
                Some(halt) => inner.announce_halt(halt),
                None => inner.go_idle(),
            }
        }
    }
}

impl Inner {
    /// Takes the first waiting item and marks it as running.
    fn take_next(&self) -> Option<QueueItem> {
        let item = locked(&self.state).queue.start_next()?;
        self.queue_changed();
        Some(item)
    }

    async fn process(self: &Arc<Self>, item: QueueItem) {
        let cancel = self.shutdown.child_token();
        *locked(&self.current) = Some(cancel.clone());
        self.skip.store(false, Ordering::SeqCst);
        self.interrupt.store(false, Ordering::SeqCst);

        // The settings are those of the moment the item starts: changing them halfway through would
        // give one folder two naming schemes
        let settings = self.settings.get();
        let detail = settings.advanced.log_detail;
        let user_id = self
            .client
            .session()
            .map_or(0, |session| session.account.id);

        {
            let mut state = locked(&self.state);
            state.progress = Progress {
                running: true,
                item_id: Some(item.id.clone()),
                ..Progress::default()
            };
            self.progress_changed(&mut state, true);
        }

        let reporter = Arc::new(ItemReporter {
            inner: Arc::clone(self),
            item_id: item.id.clone(),
            detail,
            folders: Mutex::default(),
        });
        let mut job = Job::new(
            JobSetup {
                env: self.env.clone(),
                settings,
                reporter: Arc::clone(&reporter) as Arc<dyn Reporter>,
                cancel,
                skip: Arc::clone(&self.skip),
                covers: self.covers.clone(),
                user_id,
            },
            &item,
        );
        let result = job.run(&item).await;
        let outcome = job.into_outcome();

        *locked(&self.current) = None;
        let folders = std::mem::take(&mut *locked(&reporter.folders));
        self.finish(&item.id, &outcome, result, detail, &folders);
    }

    /// Records how an item ended.
    fn finish(
        &self,
        id: &str,
        outcome: &Outcome,
        result: Result<(), JobError>,
        detail: LogDetail,
        folders: &BTreeSet<PathBuf>,
    ) {
        if let Some(item) = locked(&self.state).queue.get_mut(id) {
            outcome.write_to(item);
        }

        let interrupted = self.interrupt.swap(false, Ordering::SeqCst);
        // `None`: the item did not end, it waits again
        let ended = match result {
            Ok(()) => Some((ItemStatus::Completed, None)),
            // The application is stopping: the item stays as it is, and waits again at the next start
            Err(JobError::Cancelled) if self.shutdown.is_cancelled() => {
                self.queue_changed();
                return;
            }
            // Paused, or signed out: the item is not given up, nothing about it failed
            Err(JobError::Cancelled) if interrupted || !self.client.is_signed_in() => None,
            Err(JobError::Cancelled) => {
                self.log_at(LogEvent::Aborted, detail);
                Some((ItemStatus::Cancelled, None))
            }
            Err(JobError::SessionExpired) => {
                // Before the next item is picked: it would meet the same refusal. When the user
                // signed out there is nothing to announce
                if self.client.is_signed_in() {
                    self.session_expired();
                }
                None
            }
            Err(error) => {
                if matches!(error, JobError::StorageFull(_)) {
                    self.halted.store(true, Ordering::SeqCst);
                }
                let message = error.to_string();
                self.log_at(
                    LogEvent::Error {
                        message: message.clone(),
                    },
                    detail,
                );
                Some((ItemStatus::Failed, Some(message)))
            }
        };

        let finished = {
            let mut state = locked(&self.state);
            let finished = if let Some((status, error)) = ended {
                state.queue.finish(id, status, error);
                state.queue.get(id).cloned()
            } else {
                state.queue.suspend(id);
                None
            };
            state.progress.item_id = None;
            state.progress.album = None;
            state.progress.tracks.clear();
            state.progress.percent = 0;
            self.progress_changed(&mut state, true);
            finished
        };
        self.queue_changed();
        if let Some(item) = finished {
            self.extensions
                .emit(extensions::Event::ItemFinished(ItemFinished {
                    item_id: item.id,
                    url: item.url,
                    kind: item.kind,
                    label: item.label,
                    status: item.status,
                    error: item.error,
                    downloaded: item.downloaded,
                    skipped: item.skipped,
                    failed: u32::try_from(item.failures.len()).unwrap_or(u32::MAX),
                    folders: folders
                        .iter()
                        .map(|folder| folder.to_string_lossy().into_owned())
                        .collect(),
                }));
        }
    }

    /// Why the worker has to stop going through the queue, if it has to.
    pub(super) fn stop_reason(&self) -> Option<Halt> {
        if self.halted.load(Ordering::SeqCst) {
            Some(Halt::StorageFull)
        } else if self.paused.load(Ordering::SeqCst) {
            Some(Halt::Paused)
        } else if !self.client.is_signed_in() && locked(&self.state).queue.has_pending() {
            Some(Halt::SignedOut)
        } else {
            None
        }
    }

    /// The queue stops: what is waiting stays so, and the interface says why.
    pub(super) fn announce_halt(&self, halt: Halt) {
        {
            let mut state = locked(&self.state);
            state.progress = Progress {
                halt: Some(halt),
                ..Progress::default()
            };
            self.progress_changed(&mut state, true);
        }
        self.queue_changed();
    }

    /// Forgets that the queue stopped for this reason.
    fn clear_halt(&self, halt: Halt) {
        let mut state = locked(&self.state);
        if state.progress.halt == Some(halt) {
            state.progress.halt = None;
            self.progress_changed(&mut state, true);
        }
    }

    /// Nothing is running any more: the card of the last download should not stay on screen.
    fn go_idle(&self) {
        let drained = !locked(&self.state).queue.has_pending();
        {
            let mut state = locked(&self.state);
            state.progress = Progress::default();
            self.progress_changed(&mut state, true);
        }
        if drained {
            self.log(LogEvent::QueueDrained);
            self.extensions.emit(extensions::Event::QueueDrained);
        }
        self.queue_changed();
    }
}
