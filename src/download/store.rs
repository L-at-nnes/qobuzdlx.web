use std::io;
use std::path::Path;

use serde::{Deserialize, Serialize};

use super::item::QueueItem;
use super::queue::Queue;
use crate::storage::{Access, JsonFile};

/// The queue on disk, so that what was waiting survives a restart.
#[derive(Debug, Clone)]
pub struct QueueStore {
    file: JsonFile,
    /// What is kept besides the items: for now, whether the user paused the queue.
    state: JsonFile,
}

/// What the queue remembers besides its items.
#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default)]
struct SavedState {
    paused: bool,
}

impl QueueStore {
    #[must_use]
    pub fn new(config_dir: &Path) -> Self {
        Self {
            file: JsonFile::new(config_dir.join("queue.json"), Access::Normal),
            state: JsonFile::new(config_dir.join("queue-state.json"), Access::Normal),
        }
    }

    /// Whether the user had paused the queue when the process stopped: a pause lasts until the user
    /// lifts it, restarts included.
    #[must_use]
    pub fn load_paused(&self) -> bool {
        self.state.load::<SavedState>().unwrap_or_default().paused
    }

    pub fn save_paused(&self, paused: bool) -> io::Result<()> {
        self.state.save(&SavedState { paused })
    }

    /// The saved queue. An item that was running when the process stopped is waiting again.
    #[must_use]
    pub fn load(&self) -> Queue {
        let mut queue = Queue::new(self.file.load::<Vec<QueueItem>>().unwrap_or_default());
        queue.interrupt_running();
        queue
    }

    pub fn save(&self, queue: &Queue) -> io::Result<()> {
        self.file.save(&queue.items())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::download::item::{Display, ItemKind, ItemStatus};
    use crate::download::queue::MAX_FINISHED;
    use std::fs;
    use tempfile::TempDir;

    fn queue_of(count: usize) -> Queue {
        let mut queue = Queue::default();
        for number in 0..count {
            queue.enqueue(
                &format!("https://play.qobuz.com/album/{number}"),
                ItemKind::Album,
                &number.to_string(),
                Display::default(),
            );
        }
        queue
    }

    fn urls(queue: &Queue) -> Vec<String> {
        queue.items().iter().map(|item| item.url.clone()).collect()
    }

    #[test]
    fn without_a_file_the_queue_is_empty() {
        let directory = TempDir::new().unwrap();

        assert!(QueueStore::new(directory.path()).load().items().is_empty());
    }

    #[test]
    fn a_saved_queue_is_read_back_in_order() {
        let directory = TempDir::new().unwrap();
        let store = QueueStore::new(directory.path());
        let queue = queue_of(3);

        store.save(&queue).unwrap();

        assert_eq!(urls(&store.load()), urls(&queue));
    }

    #[test]
    fn an_item_that_was_running_waits_again_after_a_restart() {
        let directory = TempDir::new().unwrap();
        let store = QueueStore::new(directory.path());
        let mut queue = queue_of(2);
        queue.start_next();
        store.save(&queue).unwrap();

        let restored = store.load();

        assert!(
            restored
                .items()
                .iter()
                .all(|item| item.status == ItemStatus::Pending)
        );
    }

    #[test]
    fn only_the_most_recent_finished_items_are_kept() {
        let directory = TempDir::new().unwrap();
        let store = QueueStore::new(directory.path());
        let mut queue = queue_of(MAX_FINISHED + 5);
        let ids: Vec<String> = queue.items().iter().map(|item| item.id.clone()).collect();
        for id in &ids {
            queue.finish(id, ItemStatus::Completed, None);
        }
        // One more, still waiting, at the very end
        queue.enqueue(
            "https://play.qobuz.com/album/waiting",
            ItemKind::Album,
            "w",
            Display::default(),
        );

        store.save(&queue).unwrap();
        let restored = store.load();

        assert_eq!(restored.items().len(), MAX_FINISHED + 1);
        assert_eq!(
            restored.items()[0].url,
            "https://play.qobuz.com/album/5",
            "the five oldest are gone"
        );
        assert!(restored.has_pending(), "the waiting item is never dropped");
    }

    #[test]
    fn waiting_items_are_kept_whatever_their_number() {
        let directory = TempDir::new().unwrap();
        let store = QueueStore::new(directory.path());

        store.save(&queue_of(MAX_FINISHED + 50)).unwrap();

        assert_eq!(store.load().items().len(), MAX_FINISHED + 50);
    }

    #[test]
    fn a_pause_is_remembered_until_it_is_lifted() {
        let directory = TempDir::new().unwrap();
        let store = QueueStore::new(directory.path());
        assert!(!store.load_paused(), "no file: not paused");

        store.save_paused(true).unwrap();
        assert!(QueueStore::new(directory.path()).load_paused());

        store.save_paused(false).unwrap();
        assert!(!QueueStore::new(directory.path()).load_paused());
    }

    #[test]
    fn a_damaged_state_file_does_not_pause_the_queue() {
        let directory = TempDir::new().unwrap();
        fs::write(directory.path().join("queue-state.json"), "{ broken").unwrap();

        assert!(!QueueStore::new(directory.path()).load_paused());
    }

    #[test]
    fn a_damaged_file_gives_an_empty_queue_instead_of_an_error() {
        let directory = TempDir::new().unwrap();
        fs::write(directory.path().join("queue.json"), "[{ broken").unwrap();

        assert!(QueueStore::new(directory.path()).load().items().is_empty());
    }
}
