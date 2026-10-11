use std::sync::Arc;
use std::sync::atomic::Ordering;

use super::reporter::ItemReporter;
use super::{Inner, locked};
use crate::download::item::{ItemStatus, QueueItem};
use crate::download::job::{Job, JobError, JobSetup};
use crate::download::log::LogEvent;
use crate::download::progress::{Halt, Progress};
use crate::download::report::Outcome;
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
        // Whoever woke the worker wants it to try again, with room made or not
        inner.halted.store(false, Ordering::SeqCst);

        // Drained until nothing is left: a wake-up says there is work, not how much
        let mut processed = false;
        while !inner.shutdown.is_cancelled() && inner.client.is_signed_in() {
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
            if inner.halted.load(Ordering::SeqCst) {
                inner.announce_halt();
            } else {
                inner.go_idle();
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

        let mut job = Job::new(
            JobSetup {
                env: self.env.clone(),
                settings,
                reporter: Arc::new(ItemReporter {
                    inner: Arc::clone(self),
                    item_id: item.id.clone(),
                    detail,
                }),
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
        self.finish(&item.id, &outcome, result, detail);
    }

    /// Records how an item ended.
    fn finish(&self, id: &str, outcome: &Outcome, result: Result<(), JobError>, detail: LogDetail) {
        if let Some(item) = locked(&self.state).queue.get_mut(id) {
            outcome.write_to(item);
        }

        let (status, error) = match result {
            Ok(()) => (ItemStatus::Completed, None),
            // The application is stopping: the item stays as it is, and waits again at the next start
            Err(JobError::Cancelled) if self.shutdown.is_cancelled() => {
                self.queue_changed();
                return;
            }
            Err(JobError::Cancelled) => {
                self.log_at(LogEvent::Aborted, detail);
                (ItemStatus::Cancelled, None)
            }
            Err(JobError::SessionExpired) => {
                // Before the next item is picked: it would meet the same refusal
                self.session_expired();
                self.log_at(LogEvent::SessionExpired, detail);
                (
                    ItemStatus::Failed,
                    Some(LogEvent::SessionExpired.to_string()),
                )
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
                (ItemStatus::Failed, Some(message))
            }
        };

        {
            let mut state = locked(&self.state);
            state.queue.finish(id, status, error);
            state.progress.item_id = None;
            state.progress.album = None;
            state.progress.tracks.clear();
            state.progress.percent = 0;
            self.progress_changed(&mut state, true);
        }
        self.queue_changed();
    }

    /// The queue stops by itself: what is waiting stays so, and the interface says why.
    fn announce_halt(&self) {
        {
            let mut state = locked(&self.state);
            state.progress = Progress {
                halt: Some(Halt::StorageFull),
                ..Progress::default()
            };
            self.progress_changed(&mut state, true);
        }
        self.queue_changed();
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
        }
        self.queue_changed();
    }
}
