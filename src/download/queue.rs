use std::collections::{HashMap, HashSet};

use serde::Serialize;

use super::item::{Display, ItemKind, ItemStatus, QueueItem, now};
use super::url::QobuzUrl;

/// What queuing an address did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Enqueued {
    /// A new item.
    Added,
    /// The address had been processed already: the item went back to the end of the queue.
    Requeued,
    /// The address is waiting or running: nothing was done.
    AlreadyQueued,
}

/// How many finished items are kept. The oldest go first; waiting and running items never do. Without
/// a bound, a queue that processes thousands of albums would hold, show and save all of them.
pub const MAX_FINISHED: usize = 500;

/// What queuing several addresses at once did, counted by outcome.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct EnqueuedCounts {
    pub added: u32,
    pub requeued: u32,
    pub already_queued: u32,
}

/// How much of the queue a listener asks to be given: the queue can hold many thousands of waiting
/// items, and a page shows a hundred of them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Window {
    /// How many waiting items, from the first one.
    pub pending: usize,
    /// How many finished items, the latest ones.
    pub finished: usize,
}

impl Window {
    /// Everything.
    pub const ALL: Self = Self {
        pending: usize::MAX,
        finished: usize::MAX,
    };
}

impl Default for Window {
    fn default() -> Self {
        Self {
            pending: 100,
            finished: 20,
        }
    }
}

/// How many items the queue holds, by what they are doing.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct Counts {
    pub pending: usize,
    pub running: usize,
    pub finished: usize,
}

/// What a finished album cost, for guessing what the next ones will.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Sample {
    pub downloaded: u32,
    pub skipped: u32,
    /// From its start to its end, when both are known.
    pub seconds: Option<u64>,
    pub bytes: u64,
}

/// What is waiting, as far as its size can be guessed: an album is an album, a discography is not.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct Work {
    pub albums: usize,
    pub tracks: usize,
    /// Items that give no idea of their size (artists, labels, playlists, favourites).
    pub uncounted: usize,
}

/// The finished albums looked at to guess the cost of the next ones.
const HISTORY: usize = 200;

/// A part of the queue, with what the rest of it comes to.
#[derive(Debug, Clone, Default)]
pub struct View {
    /// The waiting items asked for, the one running, and the finished items asked for, in the order
    /// of the queue.
    pub items: Vec<QueueItem>,
    pub hidden_pending: usize,
    pub hidden_finished: usize,
    pub counts: Counts,
    pub history: Vec<Sample>,
    pub work: Work,
}

/// What an address stands for in the queue.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum UrlState {
    Queued,
    Done,
}

/// The download queue and the rules that govern it. Pure data: the worker and the locking live in
/// the manager.
#[derive(Debug, Clone, Default)]
pub struct Queue {
    items: Vec<QueueItem>,
}

impl Queue {
    #[must_use]
    pub fn new(items: Vec<QueueItem>) -> Self {
        let mut queue = Self { items };
        queue.forget_old_finished();
        queue
    }

    /// Drops the oldest finished items beyond `MAX_FINISHED`.
    fn forget_old_finished(&mut self) {
        let mut excess = self
            .items
            .iter()
            .filter(|item| item.is_finished())
            .count()
            .saturating_sub(MAX_FINISHED);
        if excess == 0 {
            return;
        }
        self.items.retain(|item| {
            if excess > 0 && item.is_finished() {
                excess -= 1;
                return false;
            }
            true
        });
    }

    #[must_use]
    pub fn items(&self) -> &[QueueItem] {
        &self.items
    }

    #[must_use]
    pub fn get(&self, id: &str) -> Option<&QueueItem> {
        self.items.iter().find(|item| item.id == id)
    }

    pub fn get_mut(&mut self, id: &str) -> Option<&mut QueueItem> {
        self.items.iter_mut().find(|item| item.id == id)
    }

    fn position_of_url(&self, url: &str) -> Option<usize> {
        self.items
            .iter()
            .position(|item| item.url.eq_ignore_ascii_case(url))
    }

    /// Adds an address to the queue. An address that is already waiting or running is not added
    /// twice; one that was processed is put back at the end rather than duplicated, so that an
    /// address is never in the queue more than once.
    pub fn enqueue(
        &mut self,
        url: &str,
        kind: ItemKind,
        ref_id: &str,
        display: Display,
    ) -> Enqueued {
        let Some(position) = self.position_of_url(url) else {
            self.items.push(QueueItem::new(
                url.to_owned(),
                kind,
                ref_id.to_owned(),
                display,
            ));
            return Enqueued::Added;
        };

        if !self.items[position].is_finished() {
            return Enqueued::AlreadyQueued;
        }

        let mut item = self.items.remove(position);
        item.reset_outcome();
        item.requeue();
        if let Some(label) = display.label.filter(|label| !label.trim().is_empty()) {
            item.label = label;
        }
        if display.sub_label.is_some() {
            item.sub_label = display.sub_label;
        }
        if display.thumbnail.is_some() {
            item.thumbnail = display.thumbnail;
        }
        self.items.push(item);
        Enqueued::Requeued
    }

    /// Adds several addresses, in order. The same as calling `enqueue` for each, without looking
    /// through the whole queue for each: a list of thousands of addresses is added at once.
    pub fn enqueue_many(
        &mut self,
        wanted: impl IntoIterator<Item = (QobuzUrl, Display)>,
    ) -> EnqueuedCounts {
        let mut known: HashSet<String> = self
            .items
            .iter()
            .map(|item| item.url.to_ascii_lowercase())
            .collect();
        let mut counts = EnqueuedCounts::default();
        for (address, display) in wanted {
            // An address that is not in the queue yet is added at the end: no need to look for it
            if known.insert(address.canonical.to_ascii_lowercase()) {
                self.items.push(QueueItem::new(
                    address.canonical,
                    address.kind,
                    address.ref_id,
                    display,
                ));
                counts.added += 1;
                continue;
            }
            match self.enqueue(&address.canonical, address.kind, &address.ref_id, display) {
                Enqueued::Added => counts.added += 1,
                Enqueued::Requeued => counts.requeued += 1,
                Enqueued::AlreadyQueued => counts.already_queued += 1,
            }
        }
        counts
    }

    /// Takes the first waiting item and marks it as running.
    pub fn start_next(&mut self) -> Option<QueueItem> {
        let item = self
            .items
            .iter_mut()
            .find(|item| item.status == ItemStatus::Pending)?;
        item.status = ItemStatus::Running;
        item.started_at = Some(now());
        Some(item.clone())
    }

    /// Whether something is waiting to be processed.
    #[must_use]
    pub fn has_pending(&self) -> bool {
        self.items
            .iter()
            .any(|item| item.status == ItemStatus::Pending)
    }

    /// Ends an item with the given status.
    pub fn finish(&mut self, id: &str, status: ItemStatus, error: Option<String>) {
        if let Some(item) = self.get_mut(id) {
            item.status = status;
            item.error = error;
            if status.is_finished() {
                item.finished_at = Some(now());
            }
        }
        self.forget_old_finished();
    }

    /// Puts back to waiting an item that was running when the process stopped.
    pub fn interrupt_running(&mut self) {
        for item in self
            .items
            .iter_mut()
            .filter(|item| item.status == ItemStatus::Running)
        {
            item.put_back();
        }
    }

    /// Puts back to waiting the running item `id`, which keeps its place: it is the next to start.
    /// For an item that could not go on through no fault of its own (signed out, paused).
    pub fn suspend(&mut self, id: &str) {
        if let Some(item) = self
            .items
            .iter_mut()
            .find(|item| item.id == id && item.status == ItemStatus::Running)
        {
            item.put_back();
        }
    }

    /// Removes an item that is not running: that one has to be cancelled first.
    pub fn remove(&mut self, id: &str) -> bool {
        let before = self.items.len();
        self.items
            .retain(|item| !(item.id == id && item.status != ItemStatus::Running));
        self.items.len() != before
    }

    /// Removes what is waiting. The item that runs goes to the end, unlike with a cancel.
    pub fn clear_pending(&mut self) -> bool {
        let before = self.items.len();
        self.items.retain(|item| item.status != ItemStatus::Pending);
        self.items.len() != before
    }

    pub fn clear_finished(&mut self) -> bool {
        let before = self.items.len();
        self.items.retain(|item| !item.is_finished());
        self.items.len() != before
    }

    /// Moves a waiting item up (`-1`) or down (`+1`), past other waiting items only: finished items
    /// and the one that runs keep their place, otherwise the order shown would jump for no visible
    /// reason.
    pub fn move_item(&mut self, id: &str, direction: isize) -> bool {
        let Some(from) = self.items.iter().position(|item| item.id == id) else {
            return false;
        };
        if self.items[from].status != ItemStatus::Pending {
            return false;
        }

        let mut target = from.checked_add_signed(direction);
        while let Some(index) = target {
            match self.items.get(index) {
                Some(item) if item.status == ItemStatus::Pending => {
                    self.items.swap(from, index);
                    return true;
                }
                Some(_) => target = index.checked_add_signed(direction),
                None => return false,
            }
        }
        false
    }

    /// Processes a finished item again, from scratch.
    pub fn retry(&mut self, id: &str) -> bool {
        let Some(position) = self
            .items
            .iter()
            .position(|item| item.id == id && item.is_finished())
        else {
            return false;
        };
        let mut item = self.items.remove(position);
        item.reset_outcome();
        item.requeue();
        self.items.push(item);
        true
    }

    /// Processes again only the tracks of a finished item that failed. The item keeps what it had
    /// already achieved: its counters and playlist files build on it.
    pub fn retry_failures(&mut self, id: &str) -> bool {
        let Some(position) = self
            .items
            .iter()
            .position(|item| item.id == id && item.is_finished() && !item.failures.is_empty())
        else {
            return false;
        };

        let mut item = self.items.remove(position);
        item.only_track_ids = Some(
            item.failures
                .iter()
                .map(|failure| failure.track_id)
                .collect(),
        );
        item.failures.clear();
        item.requeue();
        self.items.push(item);
        true
    }

    /// A part of the queue and what the whole of it comes to, in one go through it.
    #[must_use]
    pub fn view(&self, window: Window) -> View {
        let mut counts = Counts::default();
        let mut work = Work::default();
        for item in &self.items {
            match item.status {
                ItemStatus::Pending => {
                    counts.pending += 1;
                    match item.kind {
                        ItemKind::Album => work.albums += 1,
                        ItemKind::Track => work.tracks += 1,
                        _ => work.uncounted += 1,
                    }
                }
                ItemStatus::Running => counts.running += 1,
                _ => counts.finished += 1,
            }
        }

        let skipped_finished = counts.finished.saturating_sub(window.finished);
        let (mut pending_seen, mut finished_seen) = (0, 0);
        let items = self
            .items
            .iter()
            .filter(|item| match item.status {
                ItemStatus::Running => true,
                ItemStatus::Pending => {
                    pending_seen += 1;
                    pending_seen <= window.pending
                }
                _ => {
                    finished_seen += 1;
                    finished_seen > skipped_finished
                }
            })
            .cloned()
            .collect();

        let mut history: Vec<Sample> = self
            .items
            .iter()
            .rev()
            .filter(|item| {
                item.kind == ItemKind::Album
                    && item.status == ItemStatus::Completed
                    && item.downloaded + item.skipped > 0
            })
            .take(HISTORY)
            .map(|item| Sample {
                downloaded: item.downloaded,
                skipped: item.skipped,
                seconds: item
                    .started_at
                    .zip(item.finished_at)
                    .map(|(start, end)| end.saturating_sub(start)),
                bytes: item.bytes,
            })
            .collect();
        // Oldest first, as the queue is
        history.reverse();

        View {
            items,
            hidden_pending: counts.pending.saturating_sub(window.pending),
            hidden_finished: skipped_finished,
            counts,
            history,
            work,
        }
    }

    /// What each of these addresses stands for: waiting or running, or downloaded. The others are
    /// not in the answer.
    #[must_use]
    pub fn states(&self, urls: &[String]) -> HashMap<String, UrlState> {
        let wanted: HashSet<String> = urls.iter().map(|url| url.to_ascii_lowercase()).collect();
        let mut states: HashMap<String, UrlState> = HashMap::new();
        for item in &self.items {
            let url = item.url.to_ascii_lowercase();
            if !wanted.contains(&url) {
                continue;
            }
            if !item.is_finished() {
                states.insert(url, UrlState::Queued);
            } else if item.status == ItemStatus::Completed {
                states.entry(url).or_insert(UrlState::Done);
            }
        }
        states
    }

    /// Items that are waiting or running: what the badge of the menu counts.
    #[must_use]
    pub fn remaining(&self) -> usize {
        self.items.iter().filter(|item| !item.is_finished()).count()
    }

    /// Whether the address is waiting or running.
    #[must_use]
    pub fn is_active(&self, url: &str) -> bool {
        self.items
            .iter()
            .any(|item| item.url.eq_ignore_ascii_case(url) && !item.is_finished())
    }

    /// Whether the address was downloaded successfully.
    #[must_use]
    pub fn is_done(&self, url: &str) -> bool {
        self.items
            .iter()
            .any(|item| item.url.eq_ignore_ascii_case(url) && item.status == ItemStatus::Completed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::download::item::FailedTrack;

    fn queue_with(urls: &[&str]) -> Queue {
        let mut queue = Queue::default();
        for url in urls {
            queue.enqueue(url, ItemKind::Album, "x", Display::default());
        }
        queue
    }

    fn id_of(queue: &Queue, url: &str) -> String {
        queue
            .items()
            .iter()
            .find(|item| item.url == url)
            .unwrap()
            .id
            .clone()
    }

    fn order(queue: &Queue) -> Vec<&str> {
        queue.items().iter().map(|item| item.url.as_str()).collect()
    }

    fn finished(queue: &mut Queue, url: &str, status: ItemStatus) -> String {
        let id = id_of(queue, url);
        queue.finish(&id, status, None);
        id
    }

    fn address(url: &str, kind: ItemKind, id: &str) -> QobuzUrl {
        QobuzUrl {
            canonical: url.to_owned(),
            kind,
            ref_id: id.to_owned(),
        }
    }

    #[test]
    fn a_suspended_item_waits_again_in_its_place_with_nothing_counted() {
        let mut queue = queue_with(&[
            "https://play.qobuz.com/album/a",
            "https://play.qobuz.com/album/b",
        ]);
        let running = queue.start_next().unwrap();
        queue.get_mut(&running.id).unwrap().downloaded = 3;

        queue.suspend(&running.id);

        assert_eq!(queue.items()[0].status, ItemStatus::Pending);
        assert_eq!(queue.items()[0].downloaded, 0);
        assert_eq!(queue.start_next().unwrap().id, running.id);
    }

    #[test]
    fn only_a_running_item_can_be_suspended() {
        let mut queue = queue_with(&["https://play.qobuz.com/album/a"]);
        let id = finished(
            &mut queue,
            "https://play.qobuz.com/album/a",
            ItemStatus::Completed,
        );

        queue.suspend(&id);

        assert_eq!(queue.get(&id).unwrap().status, ItemStatus::Completed);
    }

    #[test]
    fn only_the_most_recent_finished_items_stay_in_the_queue() {
        let mut queue = Queue::default();
        for number in 0..MAX_FINISHED + 3 {
            let url = format!("https://play.qobuz.com/album/a{number}");
            queue.enqueue(&url, ItemKind::Album, "x", Display::default());
            let id = id_of(&queue, &url);
            queue.finish(&id, ItemStatus::Completed, None);
        }
        queue.enqueue(
            "https://play.qobuz.com/album/waiting",
            ItemKind::Album,
            "w",
            Display::default(),
        );

        assert_eq!(queue.items().len(), MAX_FINISHED + 1);
        assert_eq!(queue.items()[0].url, "https://play.qobuz.com/album/a3");
        assert!(queue.has_pending());
    }

    #[test]
    fn many_addresses_are_added_in_order_and_counted() {
        let mut queue = queue_with(&["https://play.qobuz.com/album/old"]);
        let counts = queue.enqueue_many([
            (
                address("https://play.qobuz.com/album/a", ItemKind::Album, "a"),
                Display::default(),
            ),
            (
                address("https://play.qobuz.com/album/OLD", ItemKind::Album, "old"),
                Display::default(),
            ),
            (
                address("https://play.qobuz.com/album/b", ItemKind::Album, "b"),
                Display::default(),
            ),
            (
                address("https://play.qobuz.com/album/a", ItemKind::Album, "a"),
                Display::default(),
            ),
        ]);

        assert_eq!(
            counts,
            EnqueuedCounts {
                added: 2,
                requeued: 0,
                already_queued: 2
            }
        );
        let urls: Vec<_> = queue.items().iter().map(|item| item.url.as_str()).collect();
        assert_eq!(
            urls,
            [
                "https://play.qobuz.com/album/old",
                "https://play.qobuz.com/album/a",
                "https://play.qobuz.com/album/b"
            ]
        );
    }

    #[test]
    fn a_finished_address_in_a_batch_goes_back_to_the_end() {
        let mut queue = queue_with(&[
            "https://play.qobuz.com/album/done",
            "https://play.qobuz.com/album/other",
        ]);
        let id = id_of(&queue, "https://play.qobuz.com/album/done");
        queue.finish(&id, ItemStatus::Completed, None);

        let counts = queue.enqueue_many([(
            address("https://play.qobuz.com/album/done", ItemKind::Album, "done"),
            Display::default(),
        )]);

        assert_eq!(counts.requeued, 1);
        assert_eq!(
            queue.items().last().unwrap().url,
            "https://play.qobuz.com/album/done"
        );
        assert_eq!(queue.items().last().unwrap().status, ItemStatus::Pending);
    }

    #[test]
    fn a_new_address_is_added_waiting_with_its_address_as_label() {
        let mut queue = Queue::default();

        let result = queue.enqueue(
            "https://play.qobuz.com/album/a",
            ItemKind::Album,
            "a",
            Display::default(),
        );

        assert_eq!(result, Enqueued::Added);
        let item = &queue.items()[0];
        assert_eq!(
            (item.status, item.label.as_str()),
            (ItemStatus::Pending, "https://play.qobuz.com/album/a")
        );
    }

    #[test]
    fn what_the_interface_knows_about_an_item_is_kept() {
        let mut queue = Queue::default();
        let display = Display {
            label: Some("Kind of Blue".into()),
            sub_label: Some("Miles Davis".into()),
            thumbnail: Some("t.jpg".into()),
        };

        queue.enqueue("u", ItemKind::Album, "a", display);

        let item = &queue.items()[0];
        assert_eq!(
            (
                item.label.as_str(),
                item.sub_label.as_deref(),
                item.thumbnail.as_deref()
            ),
            ("Kind of Blue", Some("Miles Davis"), Some("t.jpg"))
        );
    }

    #[test]
    fn an_address_already_waiting_or_running_is_not_added_twice() {
        let mut queue = queue_with(&["u"]);

        assert_eq!(
            queue.enqueue("U", ItemKind::Album, "x", Display::default()),
            Enqueued::AlreadyQueued
        );
        queue.start_next();
        assert_eq!(
            queue.enqueue("u", ItemKind::Album, "x", Display::default()),
            Enqueued::AlreadyQueued
        );
        assert_eq!(queue.items().len(), 1);
    }

    #[test]
    fn a_processed_address_goes_back_to_the_end_with_a_clean_slate() {
        let mut queue = queue_with(&["a", "b"]);
        let id = finished(&mut queue, "a", ItemStatus::Failed);
        queue.get_mut(&id).unwrap().downloaded = 3;
        queue.get_mut(&id).unwrap().error = Some("boom".into());

        let result = queue.enqueue(
            "a",
            ItemKind::Album,
            "x",
            Display {
                label: Some("New name".into()),
                ..Display::default()
            },
        );

        assert_eq!(result, Enqueued::Requeued);
        assert_eq!(order(&queue), ["b", "a"]);
        let item = queue.get(&id).unwrap();
        assert_eq!(
            (
                item.status,
                item.downloaded,
                item.error.as_deref(),
                item.label.as_str()
            ),
            (ItemStatus::Pending, 0, None, "New name")
        );
    }

    #[test]
    fn items_start_in_the_order_they_were_queued() {
        let mut queue = queue_with(&["a", "b"]);

        assert_eq!(queue.start_next().unwrap().url, "a");
        assert_eq!(queue.start_next().unwrap().url, "b");
        assert!(queue.start_next().is_none());
        assert!(
            queue
                .items()
                .iter()
                .all(|item| item.status == ItemStatus::Running && item.started_at.is_some())
        );
    }

    #[test]
    fn finishing_an_item_records_when_and_why() {
        let mut queue = queue_with(&["a"]);
        let id = id_of(&queue, "a");

        queue.finish(&id, ItemStatus::Failed, Some("restricted".into()));

        let item = queue.get(&id).unwrap();
        assert_eq!(
            (item.status, item.error.as_deref()),
            (ItemStatus::Failed, Some("restricted"))
        );
        assert!(item.finished_at.is_some());
    }

    #[test]
    fn a_waiting_item_can_be_moved_past_other_waiting_items_only() {
        let mut queue = queue_with(&["a", "b", "c", "d"]);
        finished(&mut queue, "b", ItemStatus::Completed); // finished items keep their place

        let c = id_of(&queue, "c");
        assert!(queue.move_item(&c, -1));
        assert_eq!(order(&queue), ["c", "b", "a", "d"]);

        let d = id_of(&queue, "d");
        assert!(!queue.move_item(&d, 1), "nothing below the last one");
        let a = id_of(&queue, "a");
        assert!(queue.move_item(&a, 1));
        assert_eq!(order(&queue), ["c", "b", "d", "a"]);
    }

    #[test]
    fn only_a_waiting_item_can_be_moved() {
        let mut queue = queue_with(&["a", "b"]);
        let a = id_of(&queue, "a");
        queue.start_next();

        assert!(!queue.move_item(&a, 1));
        assert!(!queue.move_item("missing", 1));
    }

    #[test]
    fn the_running_item_cannot_be_removed_but_the_others_can() {
        let mut queue = queue_with(&["a", "b"]);
        let a = id_of(&queue, "a");
        let b = id_of(&queue, "b");
        queue.start_next();

        assert!(!queue.remove(&a));
        assert!(queue.remove(&b));
        assert_eq!(order(&queue), ["a"]);
    }

    #[test]
    fn clearing_removes_what_waits_or_what_finished() {
        let mut queue = queue_with(&["a", "b", "c"]);
        finished(&mut queue, "a", ItemStatus::Completed);
        queue.start_next(); // b runs

        assert!(queue.clear_pending());
        assert_eq!(order(&queue), ["a", "b"]);
        assert!(queue.clear_finished());
        assert_eq!(order(&queue), ["b"]);
        assert!(!queue.clear_finished());
    }

    #[test]
    fn a_retry_processes_a_finished_item_again_from_scratch() {
        let mut queue = queue_with(&["a", "b"]);
        let a = finished(&mut queue, "a", ItemStatus::Completed);
        queue.get_mut(&a).unwrap().downloaded = 5;

        assert!(queue.retry(&a));

        assert_eq!(order(&queue), ["b", "a"]);
        assert_eq!(
            (
                queue.get(&a).unwrap().status,
                queue.get(&a).unwrap().downloaded
            ),
            (ItemStatus::Pending, 0)
        );
        assert!(!queue.retry("missing"));
        assert!(!queue.retry(&a), "a waiting item is not retried");
    }

    #[test]
    fn retrying_the_failures_keeps_the_outcome_and_targets_the_failed_tracks() {
        let mut queue = queue_with(&["a"]);
        let a = finished(&mut queue, "a", ItemStatus::Completed);
        let item = queue.get_mut(&a).unwrap();
        item.downloaded = 4;
        item.skipped = 1;
        item.failures = vec![
            FailedTrack {
                track_id: 7,
                title: "Seven".into(),
                error: "restricted".into(),
            },
            FailedTrack {
                track_id: 9,
                title: "Nine".into(),
                error: "boom".into(),
            },
        ];

        assert!(queue.retry_failures(&a));

        let item = queue.get(&a).unwrap();
        assert_eq!(
            item.only_track_ids
                .as_ref()
                .unwrap()
                .iter()
                .copied()
                .collect::<Vec<_>>(),
            [7, 9]
        );
        assert!(item.failures.is_empty());
        assert_eq!(
            (item.downloaded, item.skipped, item.status),
            (4, 1, ItemStatus::Pending)
        );
    }

    #[test]
    fn there_is_nothing_to_retry_without_failures() {
        let mut queue = queue_with(&["a"]);
        let a = finished(&mut queue, "a", ItemStatus::Completed);

        assert!(!queue.retry_failures(&a));
    }

    #[test]
    fn a_running_item_found_at_start_up_is_waiting_again_and_restarts_from_scratch() {
        let mut queue = queue_with(&["a"]);
        queue.start_next();
        queue.items.iter_mut().for_each(|item| item.downloaded = 2);

        queue.interrupt_running();

        let item = &queue.items()[0];
        assert_eq!(
            (item.status, item.downloaded, item.started_at),
            (ItemStatus::Pending, 0, None)
        );
    }

    #[test]
    fn an_interrupted_retry_of_failures_keeps_its_earlier_outcome() {
        let mut queue = queue_with(&["a"]);
        let a = finished(&mut queue, "a", ItemStatus::Completed);
        let item = queue.get_mut(&a).unwrap();
        item.downloaded = 4;
        item.failures = vec![FailedTrack {
            track_id: 7,
            title: "Seven".into(),
            error: "x".into(),
        }];
        queue.retry_failures(&a);
        queue.start_next();

        queue.interrupt_running();

        let item = queue.get(&a).unwrap();
        assert_eq!((item.status, item.downloaded), (ItemStatus::Pending, 4));
        assert!(item.only_track_ids.is_some());
    }

    #[test]
    fn the_queue_tells_what_is_remaining_active_and_done() {
        let mut queue = queue_with(&["a", "b", "c"]);
        finished(&mut queue, "a", ItemStatus::Completed);
        finished(&mut queue, "b", ItemStatus::Failed);

        assert_eq!(queue.remaining(), 1);
        assert!(queue.is_active("c"));
        assert!(!queue.is_active("a"));
        assert!(queue.is_done("A"));
        assert!(!queue.is_done("b"), "a failure is not a success");
        assert!(queue.has_pending());
    }

    /// Ten finished items, the running one, then `waiting` waiting ones.
    fn long_queue(waiting: usize) -> Queue {
        let mut queue = Queue::default();
        let total = 10 + 1 + waiting;
        for number in 0..total {
            queue.enqueue(
                &format!("https://play.qobuz.com/album/{number}"),
                ItemKind::Album,
                &number.to_string(),
                Display::default(),
            );
        }
        for _ in 0..10 {
            let id = queue.start_next().unwrap().id;
            queue.finish(&id, ItemStatus::Completed, None);
        }
        queue.start_next().unwrap();
        queue
    }

    #[test]
    fn a_window_holds_the_first_waiting_items_the_last_finished_ones_and_the_running_one() {
        let queue = long_queue(50);

        let view = queue.view(Window {
            pending: 5,
            finished: 3,
        });

        let statuses: Vec<ItemStatus> = view.items.iter().map(|item| item.status).collect();
        assert_eq!(statuses.len(), 3 + 1 + 5);
        assert_eq!(
            statuses
                .iter()
                .filter(|s| **s == ItemStatus::Running)
                .count(),
            1
        );
        // The latest three of the ten finished, in order
        assert_eq!(view.items[0].url, "https://play.qobuz.com/album/7");
        assert_eq!(view.items[3].status, ItemStatus::Running);
        assert_eq!(view.items[4].url, "https://play.qobuz.com/album/11");
        assert_eq!((view.hidden_finished, view.hidden_pending), (7, 45));
        assert_eq!(
            view.counts,
            Counts {
                pending: 50,
                running: 1,
                finished: 10
            }
        );
    }

    #[test]
    fn asking_for_everything_gives_the_whole_queue() {
        let queue = long_queue(30);

        let view = queue.view(Window::ALL);

        assert_eq!(view.items.len(), queue.items().len());
        assert_eq!((view.hidden_finished, view.hidden_pending), (0, 0));
    }

    #[test]
    fn a_window_larger_than_the_queue_hides_nothing() {
        let view = long_queue(3).view(Window {
            pending: 100,
            finished: 20,
        });

        assert_eq!(view.items.len(), 14);
        assert_eq!((view.hidden_finished, view.hidden_pending), (0, 0));
    }

    #[test]
    fn what_waits_is_counted_by_how_well_its_size_is_known() {
        let mut queue = Queue::default();
        queue.enqueue(
            "https://play.qobuz.com/album/a",
            ItemKind::Album,
            "a",
            Display::default(),
        );
        queue.enqueue(
            "https://play.qobuz.com/album/b",
            ItemKind::Album,
            "b",
            Display::default(),
        );
        queue.enqueue(
            "https://play.qobuz.com/track/1",
            ItemKind::Track,
            "1",
            Display::default(),
        );
        queue.enqueue(
            "https://play.qobuz.com/artist/2",
            ItemKind::Artist,
            "2",
            Display::default(),
        );
        queue.enqueue(
            "https://play.qobuz.com/playlist/3",
            ItemKind::Playlist,
            "3",
            Display::default(),
        );

        let work = queue.view(Window::default()).work;

        assert_eq!(
            work,
            Work {
                albums: 2,
                tracks: 1,
                uncounted: 2
            }
        );
    }

    #[test]
    fn the_history_is_the_latest_finished_albums_with_what_they_cost() {
        let mut queue = long_queue(2);
        for (number, item) in queue.items.iter_mut().take(10).enumerate() {
            item.downloaded = 10 + u32::try_from(number).unwrap();
            item.started_at = Some(1000);
            item.finished_at = Some(1020);
            item.bytes = 5;
        }
        // One that is not an album, one that wrote nothing and saw nothing
        queue.items[0].kind = ItemKind::Playlist;
        queue.items[1].downloaded = 0;

        let history = queue.view(Window::default()).history;

        assert_eq!(history.len(), 8);
        assert_eq!(
            history[0],
            Sample {
                downloaded: 12,
                skipped: 0,
                seconds: Some(20),
                bytes: 5
            }
        );
        assert_eq!(history.last().unwrap().downloaded, 19, "oldest first");
    }

    #[test]
    fn the_history_is_bounded() {
        let mut queue = long_queue(0);
        queue.items.iter_mut().for_each(|item| item.downloaded = 3);
        for number in 0..(HISTORY + 20) {
            let url = format!("https://play.qobuz.com/album/extra{number}");
            queue.enqueue(&url, ItemKind::Album, "x", Display::default());
            let id = queue
                .items()
                .iter()
                .find(|item| item.url == url)
                .unwrap()
                .id
                .clone();
            queue.get_mut(&id).unwrap().downloaded = 3;
            queue.finish(&id, ItemStatus::Completed, None);
        }

        assert_eq!(queue.view(Window::default()).history.len(), HISTORY);
    }

    #[test]
    fn addresses_are_told_apart_by_what_became_of_them() {
        let mut queue = long_queue(2);
        let urls: Vec<String> = [3, 10, 11, 12, 99]
            .iter()
            .map(|number| format!("https://PLAY.qobuz.com/album/{number}"))
            .collect();

        let states = queue.states(&urls);

        assert_eq!(states.len(), 4);
        assert_eq!(states["https://play.qobuz.com/album/3"], UrlState::Done);
        assert_eq!(states["https://play.qobuz.com/album/10"], UrlState::Queued);
        assert_eq!(states["https://play.qobuz.com/album/11"], UrlState::Queued);
        assert_eq!(states["https://play.qobuz.com/album/12"], UrlState::Queued);
        assert!(!states.contains_key("https://play.qobuz.com/album/99"));

        // Done, then queued again: queued wins
        queue.enqueue(
            "https://play.qobuz.com/album/3",
            ItemKind::Album,
            "3",
            Display::default(),
        );
        assert_eq!(
            queue.states(&urls[..1])["https://play.qobuz.com/album/3"],
            UrlState::Queued
        );
    }
}
