use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;
use tokio::sync::mpsc::{self, error::TrySendError};

use super::loader::Problem;
use super::{Event, Extension, Info};

/// How many events may wait for one extension. Past that the newest are dropped and said so: a
/// stuck extension must not make the application hold events without limit.
const BACKLOG: usize = 256;

/// Where an extension comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Source {
    /// Compiled into the application.
    Builtin,
    /// A program run for each event.
    Command,
    /// An HTTP address told about each event.
    Webhook,
}

struct Slot {
    info: Info,
    events: mpsc::Sender<Arc<Event>>,
}

#[derive(Default)]
struct Inner {
    slots: Vec<Slot>,
    problems: Vec<Problem>,
}

/// The loaded extensions. Cloning gives another handle on the same ones; the default has none and
/// costs nothing.
#[derive(Clone, Default)]
pub struct Extensions {
    inner: Arc<Inner>,
}

impl std::fmt::Debug for Extensions {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Extensions")
            .field("loaded", &self.inner.slots.len())
            .finish_non_exhaustive()
    }
}

impl Extensions {
    /// Starts the extensions: each gets its own task and its own queue of events. Must be called
    /// within a Tokio runtime. An extension whose name is already taken is left out and reported
    /// with the other `problems`.
    #[must_use]
    pub fn start(extensions: Vec<Arc<dyn Extension>>, mut problems: Vec<Problem>) -> Self {
        let mut slots: Vec<Slot> = Vec::new();
        for extension in extensions {
            let info = extension.info();
            if slots.iter().any(|slot| slot.info.name == info.name) {
                problems.push(Problem {
                    source: info.name,
                    message: "another extension already has this name".to_owned(),
                });
                continue;
            }
            tracing::info!(extension = %info.name, source = ?info.source, "extension loaded");
            let (events, receiver) = mpsc::channel(BACKLOG);
            tokio::spawn(run(extension, info.name.clone(), receiver));
            slots.push(Slot { info, events });
        }
        for problem in &problems {
            tracing::warn!(extension = %problem.source, "{}", problem.message);
        }
        Self {
            inner: Arc::new(Inner { slots, problems }),
        }
    }

    /// Tells the extensions that want it. Returns at once, whatever they do with it.
    pub fn emit(&self, event: Event) {
        let kind = event.kind();
        let mut wanting = self
            .inner
            .slots
            .iter()
            .filter(|slot| slot.info.wants(kind))
            .peekable();
        if wanting.peek().is_none() {
            return;
        }

        let event = Arc::new(event);
        for slot in wanting {
            // A closed queue means its task is gone: nothing more to do for it
            if let Err(TrySendError::Full(_)) = slot.events.try_send(Arc::clone(&event)) {
                tracing::warn!(
                    extension = %slot.info.name,
                    event = kind.name(),
                    "extension is too far behind: event dropped"
                );
            }
        }
    }

    /// What the loaded extensions say about themselves.
    #[must_use]
    pub fn list(&self) -> Vec<Info> {
        self.inner
            .slots
            .iter()
            .map(|slot| slot.info.clone())
            .collect()
    }

    /// The extensions that were found but could not be loaded, and why.
    #[must_use]
    pub fn problems(&self) -> &[Problem] {
        &self.inner.problems
    }
}

/// Hands the events of one extension to it, one at a time and in order.
async fn run(extension: Arc<dyn Extension>, name: String, mut events: mpsc::Receiver<Arc<Event>>) {
    while let Some(event) = events.recv().await {
        let kind = event.kind();
        // Its own task, so that a panic is a failure of this event and not the end of the extension
        let handler = Arc::clone(&extension);
        let outcome = tokio::spawn(async move { handler.handle(&event).await }).await;
        match outcome {
            Ok(Ok(())) => {}
            Ok(Err(failure)) => {
                tracing::warn!(extension = %name, event = kind.name(), "{failure}");
            }
            Err(_) => tracing::error!(extension = %name, event = kind.name(), "extension panicked"),
        }
    }
}

/// Seconds since the epoch, for the `at` field of an event.
pub(super) fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::extensions::{EventKind, Failure};
    use futures_util::future::BoxFuture;
    use std::sync::Mutex;
    use std::time::Duration;

    struct Recorder {
        name: &'static str,
        events: Vec<EventKind>,
        seen: Arc<Mutex<Vec<EventKind>>>,
        fail_on_drained: bool,
    }

    impl Extension for Recorder {
        fn info(&self) -> Info {
            Info {
                name: self.name.to_owned(),
                version: None,
                description: None,
                source: Source::Builtin,
                events: self.events.clone(),
            }
        }

        fn handle<'a>(&'a self, event: &'a Event) -> BoxFuture<'a, Result<(), Failure>> {
            Box::pin(async move {
                assert!(!(self.fail_on_drained && event.kind() == EventKind::QueueDrained));
                self.seen.lock().unwrap().push(event.kind());
                Ok(())
            })
        }
    }

    fn recorder(
        name: &'static str,
        events: Vec<EventKind>,
    ) -> (Arc<dyn Extension>, Arc<Mutex<Vec<EventKind>>>) {
        let seen = Arc::new(Mutex::new(Vec::new()));
        let extension = Recorder {
            name,
            events,
            seen: Arc::clone(&seen),
            fail_on_drained: name == "panics",
        };
        (Arc::new(extension), seen)
    }

    async fn settle() {
        tokio::time::sleep(Duration::from_millis(50)).await;
    }

    #[tokio::test]
    async fn emitting_with_no_extension_does_nothing() {
        Extensions::default().emit(Event::QueueDrained);
    }

    #[tokio::test]
    async fn an_extension_only_gets_the_events_it_subscribed_to() {
        let (only_items, items) = recorder("items", vec![EventKind::ItemFinished]);
        let (everything, all) = recorder("all", vec![]);
        let extensions = Extensions::start(vec![only_items, everything], vec![]);

        extensions.emit(Event::QueueDrained);
        settle().await;

        assert!(items.lock().unwrap().is_empty());
        assert_eq!(*all.lock().unwrap(), [EventKind::QueueDrained]);
    }

    #[tokio::test]
    async fn a_second_extension_with_the_same_name_is_left_out_and_reported() {
        let (first, _) = recorder("same", vec![]);
        let (second, _) = recorder("same", vec![]);

        let extensions = Extensions::start(vec![first, second], vec![]);

        assert_eq!(extensions.list().len(), 1);
        assert_eq!(extensions.problems().len(), 1);
        assert_eq!(extensions.problems()[0].source, "same");
    }

    #[tokio::test]
    async fn a_panic_costs_one_event_not_the_extension() {
        let (extension, seen) = recorder("panics", vec![]);
        let extensions = Extensions::start(vec![extension], vec![]);

        extensions.emit(Event::QueueDrained);
        extensions.emit(Event::ItemFinished(crate::extensions::ItemFinished {
            item_id: String::new(),
            url: String::new(),
            kind: crate::download::ItemKind::Album,
            label: String::new(),
            status: crate::download::ItemStatus::Completed,
            error: None,
            downloaded: 0,
            skipped: 0,
            failed: 0,
            folders: vec![],
        }));
        settle().await;

        assert_eq!(*seen.lock().unwrap(), [EventKind::ItemFinished]);
    }
}
