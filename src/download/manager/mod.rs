//! The download queue as the rest of the application sees it: what can be asked, what is shown, and
//! the worker that goes through the queue one item at a time.

mod quota;
mod reporter;
mod worker;

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

use serde::Serialize;
use tokio::sync::{Notify, broadcast};
use tokio_util::sync::CancellationToken;

use super::artwork::Covers;
use super::item::{Display, QueueItem};
use super::log::{Log, LogEvent, LogLine};
use super::progress::{Halt, Progress};
use super::queue::{Enqueued, EnqueuedCounts, Queue};
use super::store::QueueStore;
use super::track::{Environment, TEMP_PREFIX, Tuning};
use super::transfer::Fetcher;
use super::url::{QobuzUrl, parse};
use crate::extensions::Extensions;
use crate::qobuz::{Client, SessionEvent};
use crate::settings::SettingsStore;

/// How long a change waits for others before the queue is written to disk: a burst of changes (a
/// long list added, many short items in a row) is one write, not one each.
const SAVE_AFTER: Duration = Duration::from_millis(500);

/// How often the progress of a file in transfer is announced at most.
const PROGRESS_EVERY: Duration = Duration::from_millis(200);

/// What the interface is told as the download goes.
#[derive(Debug, Clone)]
pub enum Event {
    /// The queue changed: an item was added, moved, started, named or finished.
    Queue,
    /// The counts of one item changed (a track was written, skipped or failed): the item is named
    /// by its id. Much lighter to tell than the whole queue, and there is one per track.
    Item(String),
    /// The progress changed.
    Progress,
    /// A line was added to the log.
    Log(LogLine),
    /// Qobuz refused the session: whoever kept it has to forget it.
    SessionExpired,
}

/// The queue and what is going on, at one moment.
#[derive(Debug, Clone, Serialize)]
pub struct Snapshot {
    pub queue: Vec<QueueItem>,
    pub progress: Progress,
    /// Items waiting or running: what the badge of the menu counts.
    pub remaining: usize,
}

/// The address is not one that can be downloaded.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("not a Qobuz address that can be downloaded")]
pub struct InvalidAddress;

/// What the manager is built from.
#[derive(Debug, Clone)]
pub struct ManagerConfig {
    pub client: Client,
    pub settings: SettingsStore,
    /// Where `queue.json` is kept, and the partial files.
    pub config_dir: PathBuf,
    /// Used for the CDN, the covers and the bonus files. It should not have an overall timeout: a
    /// 24-bit album takes as long as it takes. A silent connection is handled by `stall_timeout`.
    pub http: reqwest::Client,
    pub stall_timeout: Duration,
    pub tuning: Tuning,
    /// Told about what the downloads do. The default has none.
    pub extensions: Extensions,
}

pub(super) struct State {
    pub(super) queue: Queue,
    pub(super) log: Log,
    pub(super) progress: Progress,
    last_progress: Instant,
}

pub(super) struct Inner {
    pub(super) client: Client,
    pub(super) settings: SettingsStore,
    pub(super) env: Environment,
    pub(super) covers: Covers,
    pub(super) extensions: Extensions,
    store: QueueStore,
    pub(super) state: Mutex<State>,
    /// Serialises the writes of `queue.json`: a slow writer must not replace a recent version with an
    /// older one.
    persist: Mutex<()>,
    /// The queue changed since it was written.
    unsaved: Notify,
    events: broadcast::Sender<Event>,
    pub(super) wake: Notify,
    pub(super) shutdown: CancellationToken,
    /// The item in progress, which "cancel" stops.
    pub(super) current: Mutex<Option<CancellationToken>>,
    /// "Skip": the album in progress is dropped.
    pub(super) skip: Arc<AtomicBool>,
    /// The queue stopped by itself (the disk is full) and waits to be told to go on.
    pub(super) halted: AtomicBool,
    /// The user paused the queue: no item starts until it goes on.
    pub(super) paused: AtomicBool,
    /// The item in progress is being stopped to wait again (paused, signed out) rather than to end.
    pub(super) interrupt: AtomicBool,
    /// Wakes the worker out of a wait it was set to take (a pause, a sign-out).
    pub(super) stir: Notify,
    /// How many times in a row Qobuz asked to slow down, with no item going well in between.
    pub(super) strikes: AtomicU32,
    /// What was fetched since the last long break.
    quota: Mutex<quota::Quota>,
    /// The interface was told the session expired, and has not been given a new one since.
    expiry_announced: AtomicBool,
}

/// The download queue. Cloning gives another handle on the same queue.
#[derive(Clone)]
pub struct Manager {
    inner: Arc<Inner>,
}

impl std::fmt::Debug for Manager {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_struct("Manager").finish_non_exhaustive()
    }
}

pub(super) fn locked<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

impl Manager {
    /// Restores the queue, cleans up after a killed process and starts the worker. Must be called
    /// within a Tokio runtime.
    #[must_use]
    pub fn start(config: ManagerConfig) -> Self {
        let temp_dir = config.config_dir.join("qbdlx-temp");
        purge_partial_files(&temp_dir);

        let store = QueueStore::new(&config.config_dir);
        let queue = store.load();
        let paused = store.load_paused();
        let env = Environment {
            client: config.client.clone(),
            fetcher: Fetcher::new(config.http.clone(), config.stall_timeout),
            temp_dir,
            tuning: config.tuning,
        };
        let (events, _) = broadcast::channel(256);
        let manager = Self {
            inner: Arc::new(Inner {
                client: config.client,
                settings: config.settings,
                env,
                covers: Covers::new(config.http),
                extensions: config.extensions,
                store,
                state: Mutex::new(State {
                    queue,
                    log: Log::default(),
                    progress: Progress::default(),
                    last_progress: Instant::now(),
                }),
                persist: Mutex::new(()),
                unsaved: Notify::new(),
                events,
                wake: Notify::new(),
                shutdown: CancellationToken::new(),
                current: Mutex::new(None),
                skip: Arc::new(AtomicBool::new(false)),
                halted: AtomicBool::new(false),
                paused: AtomicBool::new(paused),
                interrupt: AtomicBool::new(false),
                stir: Notify::new(),
                strikes: AtomicU32::new(0),
                quota: Mutex::default(),
                expiry_announced: AtomicBool::new(false),
            }),
        };

        let waiting = locked(&manager.inner.state)
            .queue
            .items()
            .iter()
            .filter(|item| !item.is_finished())
            .count();
        if waiting > 0 {
            manager.inner.log(LogEvent::QueueRestored { waiting });
            manager.inner.wake.notify_one();
        }

        if paused {
            manager.inner.log(LogEvent::QueuePaused);
            manager.inner.announce_halt(Halt::Paused);
        }

        tokio::spawn(worker::run(Arc::clone(&manager.inner)));
        tokio::spawn(keep_saved(Arc::clone(&manager.inner)));
        tokio::spawn(watch_session(Arc::clone(&manager.inner)));
        manager
    }

    /// Stops the worker. What was running is put back to waiting, for the next start.
    pub fn shutdown(&self) {
        self.inner.shutdown.cancel();
    }

    #[must_use]
    pub fn subscribe(&self) -> broadcast::Receiver<Event> {
        self.inner.events.subscribe()
    }

    // ─── Reading ───

    #[must_use]
    pub fn snapshot(&self) -> Snapshot {
        let state = locked(&self.inner.state);
        Snapshot {
            queue: state.queue.items().to_vec(),
            progress: state.progress.clone(),
            remaining: state.queue.remaining(),
        }
    }

    /// An item of the queue, as it is now.
    #[must_use]
    pub fn item(&self, id: &str) -> Option<QueueItem> {
        locked(&self.inner.state).queue.get(id).cloned()
    }

    #[must_use]
    pub fn progress(&self) -> Progress {
        locked(&self.inner.state).progress.clone()
    }

    #[must_use]
    pub fn log_lines(&self) -> Vec<LogLine> {
        locked(&self.inner.state).log.lines()
    }

    /// The log as a text file, with a header telling the version, the date and the level.
    #[must_use]
    pub fn export_log(&self, version: &str) -> String {
        let detail = self.inner.settings.get().advanced.log_detail;
        locked(&self.inner.state).log.export(version, detail)
    }

    pub fn clear_log(&self) {
        locked(&self.inner.state).log.clear();
        self.inner.emit(Event::Progress);
    }

    /// Whether the address is waiting or running: its button is greyed out.
    #[must_use]
    pub fn is_queued(&self, address: &str) -> bool {
        parse(address)
            .is_some_and(|parsed| locked(&self.inner.state).queue.is_active(&parsed.canonical))
    }

    /// Whether the address was downloaded successfully: its button keeps a mark.
    #[must_use]
    pub fn is_done(&self, address: &str) -> bool {
        parse(address)
            .is_some_and(|parsed| locked(&self.inner.state).queue.is_done(&parsed.canonical))
    }

    // ─── Asking ───

    /// Adds an address to the queue.
    pub fn enqueue(&self, address: &str, display: Display) -> Result<Enqueued, InvalidAddress> {
        let Some(parsed) = parse(address) else {
            self.inner.log(LogEvent::InvalidUrl {
                url: address.chars().take(200).collect(),
            });
            return Err(InvalidAddress);
        };

        let result = locked(&self.inner.state).queue.enqueue(
            &parsed.canonical,
            parsed.kind,
            &parsed.ref_id,
            display,
        );
        if result != Enqueued::AlreadyQueued {
            self.inner.wake.notify_one();
            self.inner.queue_changed();
        }
        Ok(result)
    }

    /// Adds several addresses with a single save of the queue and a single announcement, however
    /// many there are. Returns what became of them, and how many were not addresses that can be
    /// downloaded.
    #[must_use]
    pub fn enqueue_all(
        &self,
        wanted: impl IntoIterator<Item = (String, Display)>,
    ) -> (EnqueuedCounts, usize) {
        let mut invalid = 0;
        let mut parsed = Vec::new();
        for (address, display) in wanted {
            if let Some(address) = parse(&address) {
                parsed.push((address, display));
            } else {
                invalid += 1;
                self.inner.log(LogEvent::InvalidUrl {
                    url: address.chars().take(200).collect(),
                });
            }
        }
        (self.enqueue_parsed(parsed), invalid)
    }

    /// Adds addresses that were already read, with a single save and a single announcement.
    #[must_use]
    pub fn enqueue_parsed(&self, wanted: Vec<(QobuzUrl, Display)>) -> EnqueuedCounts {
        let counts = locked(&self.inner.state).queue.enqueue_many(wanted);
        if counts.added + counts.requeued > 0 {
            self.inner.wake.notify_one();
            self.inner.queue_changed();
        }
        counts
    }

    /// Goes on after the queue stopped by itself, once the cause has been dealt with, or after the
    /// user paused it.
    pub fn resume(&self) {
        let paused = self.inner.paused.swap(false, Ordering::SeqCst);
        let halted = self.inner.halted.swap(false, Ordering::SeqCst);
        if !(paused || halted) {
            return;
        }
        if paused {
            self.inner.remember_pause(false);
            self.inner.log(LogEvent::QueueResumed);
        }
        {
            let mut state = locked(&self.inner.state);
            state.progress.halt = None;
            self.inner.progress_changed(&mut state, true);
        }
        self.inner.wake.notify_one();
    }

    /// Stops the queue until `resume`. The item in progress is given back to the queue, not failed: it
    /// is the first to start again, and the files it had written are recognised and left alone.
    pub fn pause(&self) {
        if self.inner.paused.swap(true, Ordering::SeqCst) {
            return;
        }
        self.inner.remember_pause(true);
        self.inner.log(LogEvent::QueuePaused);
        self.inner.stir.notify_waiters();
        if locked(&self.inner.current).is_some() {
            self.inner.interrupt_current();
        } else {
            self.inner.announce_halt(Halt::Paused);
        }
    }

    /// "Skip": drops the album in progress, an artist has dozens. The item goes on.
    pub fn skip_current(&self) {
        self.inner.skip.store(true, Ordering::SeqCst);
    }

    /// "Cancel": gives up the item in progress, and the worker goes on to the next one.
    pub fn cancel_current(&self) {
        if let Some(token) = locked(&self.inner.current).as_ref() {
            token.cancel();
        }
    }

    /// Removes what is waiting. The item in progress goes to the end, unlike with a cancel.
    pub fn clear_pending(&self) {
        let changed = locked(&self.inner.state).queue.clear_pending();
        self.inner.changed_if(changed);
    }

    pub fn clear_finished(&self) {
        let changed = locked(&self.inner.state).queue.clear_finished();
        self.inner.changed_if(changed);
    }

    /// Removes an item. The one in progress has to be cancelled first.
    pub fn remove(&self, id: &str) {
        let changed = locked(&self.inner.state).queue.remove(id);
        self.inner.changed_if(changed);
    }

    /// Moves a waiting item up (`-1`) or down (`1`) among the waiting ones.
    pub fn move_item(&self, id: &str, direction: isize) {
        let changed = locked(&self.inner.state).queue.move_item(id, direction);
        self.inner.changed_if(changed);
    }

    /// Processes a finished item again, from scratch.
    pub fn retry(&self, id: &str) {
        let changed = locked(&self.inner.state).queue.retry(id);
        if changed {
            self.inner.wake.notify_one();
        }
        self.inner.changed_if(changed);
    }

    /// Processes again only the tracks of a finished item that failed.
    pub fn retry_failures(&self, id: &str) {
        let changed = locked(&self.inner.state).queue.retry_failures(id);
        if changed {
            self.inner.wake.notify_one();
        }
        self.inner.changed_if(changed);
    }
}

impl Inner {
    pub(super) fn emit(&self, event: Event) {
        // Nobody listening is not a problem
        let _ = self.events.send(event);
    }

    /// Qobuz refused the session: forget it so that nothing presents the token again, and tell the
    /// interface once. Called by whoever notices first, the job or the listener of the session.
    pub(super) fn session_expired(&self) {
        // Before the session ends: whoever hears of the end must find it was an expiry
        let first = !self.expiry_announced.swap(true, Ordering::SeqCst);
        self.client.end_session();
        if first {
            self.log(LogEvent::SessionExpired);
            self.emit(Event::SessionExpired);
        }
    }

    /// Stops the item in progress so that it waits again instead of ending.
    pub(super) fn interrupt_current(&self) {
        self.interrupt.store(true, Ordering::SeqCst);
        self.stir.notify_waiters();
        if let Some(token) = locked(&self.current).as_ref() {
            token.cancel();
        }
    }

    /// Has the queue written soon, and tells the interface.
    pub(super) fn queue_changed(&self) {
        self.unsaved.notify_one();
        self.emit(Event::Queue);
    }

    /// Writes the queue to disk now.
    fn save_queue(&self) {
        let _writing = locked(&self.persist);
        let snapshot = locked(&self.state).queue.clone();
        if let Err(error) = self.store.save(&snapshot) {
            tracing::warn!(%error, "the download queue could not be saved");
        }
    }

    fn changed_if(&self, changed: bool) {
        if changed {
            self.queue_changed();
        }
    }

    /// Tells the interface the progress changed. A file in transfer reports many times a second:
    /// those are announced a few times a second.
    pub(super) fn progress_changed(&self, state: &mut State, force: bool) {
        if !force && state.last_progress.elapsed() < PROGRESS_EVERY {
            return;
        }
        state.last_progress = Instant::now();
        self.emit(Event::Progress);
    }

    /// Adds a line to the log, if the level asks for it.
    /// Writes down that the queue is paused, or no longer, so that a restart keeps it so.
    fn remember_pause(&self, paused: bool) {
        if let Err(error) = self.store.save_paused(paused) {
            tracing::warn!(%error, "the pause of the queue could not be saved");
        }
    }

    /// A file was written: it counts toward the next long break.
    pub(super) fn count_saved(&self, album_id: &str, bytes: u64) {
        locked(&self.quota).saved(album_id, bytes);
    }

    /// How long to rest if a condition of the long break is reached, which starts the count again.
    pub(super) fn break_due(&self) -> Option<Duration> {
        let limits = self.settings.get().rate_limit;
        let mut quota = locked(&self.quota);
        if !quota.reached(&limits) {
            return None;
        }
        quota.reset();
        Some(limits.break_length.pick())
    }

    pub(super) fn log(&self, event: LogEvent) {
        let detail = self.settings.get().advanced.log_detail;
        self.log_at(event, detail);
    }

    pub(super) fn log_at(&self, event: LogEvent, detail: crate::settings::LogDetail) {
        let line = locked(&self.state).log.push(event, detail).cloned();
        if let Some(line) = line {
            self.emit(Event::Log(line));
        }
    }
}

/// Partial files left by a process that was killed mid-download. Nothing runs yet at start, so
/// everything with our prefix is stale.
fn purge_partial_files(directory: &std::path::Path) {
    let Ok(entries) = std::fs::read_dir(directory) else {
        return;
    };
    for entry in entries.flatten() {
        if entry.file_name().to_string_lossy().starts_with(TEMP_PREFIX) {
            let _ = std::fs::remove_file(entry.path());
        }
    }
}

/// Writes the queue to disk after it changed, at most twice a second, and a last time when the
/// application stops.
async fn keep_saved(inner: Arc<Inner>) {
    loop {
        tokio::select! {
            () = inner.shutdown.cancelled() => break,
            () = inner.unsaved.notified() => {}
        }
        tokio::select! {
            () = inner.shutdown.cancelled() => break,
            () = tokio::time::sleep(SAVE_AFTER) => {}
        }
        inner.save_queue();
    }
    inner.save_queue();
}

/// Follows the session: a new one wakes the worker (what was restored waited for it), a refused one
/// is forgotten so that nothing keeps presenting the token.
async fn watch_session(inner: Arc<Inner>) {
    let mut events = inner.client.subscribe();
    loop {
        let event = tokio::select! {
            () = inner.shutdown.cancelled() => return,
            event = events.recv() => event,
        };
        match event {
            Ok(SessionEvent::Started) => {
                inner.expiry_announced.store(false, Ordering::SeqCst);
                inner.wake.notify_one();
            }
            Ok(SessionEvent::Rejected) => inner.session_expired(),
            // What runs waits for the next sign-in instead of failing on its next request
            Ok(SessionEvent::Ended) => {
                if !inner.expiry_announced.load(Ordering::SeqCst) {
                    inner.log(LogEvent::SignedOut);
                }
                inner.interrupt_current();
            }
            Err(broadcast::error::RecvError::Lagged(_)) => {}
            Err(broadcast::error::RecvError::Closed) => return,
        }
    }
}
