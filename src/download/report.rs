use super::item::{FailedTrack, PlaylistEntry, QueueItem};
use super::log::LogEvent;
use super::transfer::Update;
use crate::extensions::TrackSaved;
use crate::qobuz::Album;

/// The track being processed, for the progress shown.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StepTrack {
    pub title: String,
    /// Position among the tracks to process (from 1), and how many there are.
    pub index: usize,
    pub total: usize,
}

/// Where a running download tells what it does. The manager implements it for the interface;
/// tests implement it to see what happened.
pub trait Reporter: Send + Sync {
    fn log(&self, event: LogEvent);

    /// The album being processed, or `None` between albums.
    fn album(&self, album: Option<&Album>);

    /// How far the whole item has got (already fitted to its share of the bar), and the track in
    /// progress: the only one, which replaces any other.
    fn step(&self, percent: u8, track: Option<StepTrack>);

    /// How far the whole item has got, the tracks in progress staying as they are.
    fn overall(&self, _percent: u8) {}

    /// A track starts while others may be in progress: an album downloads several at once.
    fn track_started(&self, _track: StepTrack) {}

    /// The track at this position (from 1) is no longer in progress.
    fn track_ended(&self, _index: usize) {}

    /// How far the file of the track at this position has got.
    fn track_transfer(&self, _index: usize, update: Update) {
        self.transfer(update);
    }

    /// What the thing being downloaded is called, for an item that was queued with nothing but its
    /// address (an imported list). Playlists, artists, labels and single tracks say it; an album
    /// says it through `album`.
    fn named(&self, _name: &str, _detail: Option<&str>, _thumbnail: Option<&str>) {}

    /// How far the file being received has got.
    fn transfer(&self, update: Update);

    /// A track was written to its final place.
    fn track_saved(&self, _saved: TrackSaved) {}

    /// How long to rest now, if a long break is due: between two albums of the same item.
    fn break_due(&self) -> Option<std::time::Duration> {
        None
    }

    /// The item rests for this long (a long break), or no longer does.
    fn resting(&self, _break_for: Option<std::time::Duration>) {}

    /// What has been achieved so far.
    fn outcome(&self, outcome: &Outcome);
}

/// What an item has achieved: files written, files left alone, tracks that failed.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Outcome {
    pub downloaded: u32,
    pub skipped: u32,
    pub failures: Vec<FailedTrack>,
    /// The files of a playlist in place, by position.
    pub playlist_entries: Vec<PlaylistEntry>,
}

impl Outcome {
    /// Where an item stands when it starts. A "retry the failures" pass builds on what the item
    /// already holds; a whole item starts from nothing because its outcome was reset.
    #[must_use]
    pub fn of(item: &QueueItem) -> Self {
        Self {
            downloaded: item.downloaded,
            skipped: item.skipped,
            failures: item.failures.clone(),
            playlist_entries: item.playlist_entries.clone(),
        }
    }

    pub fn write_to(&self, item: &mut QueueItem) {
        item.downloaded = self.downloaded;
        item.skipped = self.skipped;
        item.failures.clone_from(&self.failures);
        item.playlist_entries.clone_from(&self.playlist_entries);
    }

    pub fn written(&mut self) {
        self.downloaded += 1;
    }

    pub fn left_alone(&mut self) {
        self.skipped += 1;
    }

    pub fn failed(&mut self, track_id: u64, title: impl Into<String>, error: impl Into<String>) {
        self.failures.push(FailedTrack {
            track_id,
            title: title.into(),
            error: error.into(),
        });
    }

    /// Records the file of a playlist position, replacing what an earlier pass put there.
    pub fn playlist_entry(&mut self, entry: PlaylistEntry) {
        self.playlist_entries
            .retain(|existing| existing.index != entry.index);
        self.playlist_entries.push(entry);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::download::item::{Display, ItemKind};
    use std::path::PathBuf;

    fn entry(index: usize, file: &str) -> PlaylistEntry {
        PlaylistEntry {
            index,
            file: PathBuf::from(file),
            duration_seconds: 10,
            title: file.to_owned(),
        }
    }

    fn item() -> QueueItem {
        QueueItem::new(
            "https://play.qobuz.com/playlist/1".into(),
            ItemKind::Playlist,
            "1".into(),
            Display::default(),
        )
    }

    #[test]
    fn counters_add_up() {
        let mut outcome = Outcome::default();

        outcome.written();
        outcome.written();
        outcome.left_alone();
        outcome.failed(5, "Song", "restricted");

        assert_eq!((outcome.downloaded, outcome.skipped), (2, 1));
        assert_eq!(outcome.failures.len(), 1);
        assert_eq!(outcome.failures[0].track_id, 5);
    }

    #[test]
    fn a_playlist_position_holds_one_file_the_latest() {
        let mut outcome = Outcome::default();

        outcome.playlist_entry(entry(0, "first.flac"));
        outcome.playlist_entry(entry(1, "second.flac"));
        outcome.playlist_entry(entry(0, "retried.flac"));

        let files: Vec<_> = outcome
            .playlist_entries
            .iter()
            .map(|entry| (entry.index, entry.file.to_str().unwrap()))
            .collect();
        assert_eq!(files, [(1, "second.flac"), (0, "retried.flac")]);
    }

    #[test]
    fn an_outcome_goes_back_to_the_item_it_came_from() {
        let mut queued = item();
        queued.downloaded = 3;
        queued.playlist_entries = vec![entry(2, "kept.flac")];
        let mut outcome = Outcome::of(&queued);

        outcome.written();
        outcome.failed(9, "Song", "network");
        outcome.write_to(&mut queued);

        assert_eq!(queued.downloaded, 4);
        assert_eq!(queued.failures.len(), 1);
        assert_eq!(queued.playlist_entries.len(), 1);
    }
}
