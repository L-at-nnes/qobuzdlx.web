use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use super::{Event, Inner, locked};
use crate::download::item::ItemKind;
use crate::download::log::LogEvent;
use crate::download::progress::{AlbumCard, TrackProgress};
use crate::download::report::{Outcome, Reporter, StepTrack};
use crate::download::transfer::Update;
use crate::extensions::{self, TrackSaved};
use crate::qobuz::Album;
use crate::settings::LogDetail;

/// What the job of one item says, put where the interface reads it.
pub(super) struct ItemReporter {
    pub(super) inner: Arc<Inner>,
    pub(super) item_id: String,
    /// The level of detail when the item started: the log does not change halfway through.
    pub(super) detail: LogDetail,
    /// The folders that received a file, for the extensions told when the item ends.
    pub(super) folders: Mutex<BTreeSet<PathBuf>>,
}

/// A track that has just started: nothing received yet.
fn started(track: StepTrack) -> TrackProgress {
    TrackProgress {
        title: track.title,
        index: track.index,
        total: track.total,
        percent: 0,
        bytes_per_second: 0,
    }
}

impl ItemReporter {
    /// Gives the item the name of what it turned out to be, when it only had its address to show
    /// and is of a kind that this thing names. A name the interface gave is never replaced.
    fn name_item(
        &self,
        kinds: &[ItemKind],
        name: &str,
        detail: Option<&str>,
        thumbnail: Option<&str>,
    ) {
        let named = {
            let mut state = locked(&self.inner.state);
            let Some(item) = state.queue.get_mut(&self.item_id) else {
                return;
            };
            if !kinds.contains(&item.kind) || item.label != item.url || name.trim().is_empty() {
                return;
            }
            name.clone_into(&mut item.label);
            if item.sub_label.is_none() {
                item.sub_label = detail.map(str::to_owned);
            }
            if item.thumbnail.is_none() {
                item.thumbnail = thumbnail.map(str::to_owned);
            }
            true
        };
        if named {
            self.inner.queue_changed();
        }
    }
}

impl Reporter for ItemReporter {
    fn log(&self, event: LogEvent) {
        self.inner.log_at(event, self.detail);
    }

    fn album(&self, album: Option<&Album>) {
        {
            let mut state = locked(&self.inner.state);
            state.progress.album = album.map(AlbumCard::from);
            self.inner.progress_changed(&mut state, true);
        }
        if let Some(card) = album.map(AlbumCard::from) {
            self.name_item(
                &[ItemKind::Album],
                &card.title,
                Some(&card.artist),
                album.map(|album| album.image.thumbnail.as_str()),
            );
        }
    }

    fn named(&self, name: &str, detail: Option<&str>, thumbnail: Option<&str>) {
        self.name_item(
            &[
                ItemKind::Playlist,
                ItemKind::Artist,
                ItemKind::Label,
                ItemKind::Track,
            ],
            name,
            detail,
            thumbnail,
        );
    }

    fn step(&self, percent: u8, track: Option<StepTrack>) {
        let mut state = locked(&self.inner.state);
        state.progress.percent = percent;
        state.progress.tracks = track.map(started).into_iter().collect();
        self.inner.progress_changed(&mut state, true);
    }

    fn overall(&self, percent: u8) {
        let mut state = locked(&self.inner.state);
        state.progress.percent = percent;
        self.inner.progress_changed(&mut state, true);
    }

    fn track_started(&self, track: StepTrack) {
        let mut state = locked(&self.inner.state);
        let tracks = &mut state.progress.tracks;
        tracks.retain(|known| known.index != track.index);
        tracks.push(started(track));
        tracks.sort_by_key(|known| known.index);
        self.inner.progress_changed(&mut state, true);
    }

    fn track_ended(&self, index: usize) {
        let mut state = locked(&self.inner.state);
        state.progress.tracks.retain(|known| known.index != index);
        self.inner.progress_changed(&mut state, true);
    }

    fn transfer(&self, update: Update) {
        // One track at a time: the one in progress
        let mut state = locked(&self.inner.state);
        if let Some(track) = state.progress.tracks.first_mut() {
            track.percent = update.percent;
            track.bytes_per_second = update.bytes_per_second;
        }
        self.inner.progress_changed(&mut state, false);
    }

    fn track_transfer(&self, index: usize, update: Update) {
        let mut state = locked(&self.inner.state);
        if let Some(track) = state
            .progress
            .tracks
            .iter_mut()
            .find(|known| known.index == index)
        {
            track.percent = update.percent;
            track.bytes_per_second = update.bytes_per_second;
        }
        self.inner.progress_changed(&mut state, false);
    }

    fn track_saved(&self, saved: TrackSaved) {
        let bytes = std::fs::metadata(&saved.path).map_or(0, |file| file.len());
        self.inner.count_saved(&saved.album_id, bytes);
        self.folders
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .insert(PathBuf::from(&saved.folder));
        self.inner.extensions.emit(extensions::Event::TrackSaved {
            item_id: self.item_id.clone(),
            track: saved,
        });
    }

    fn break_due(&self) -> Option<Duration> {
        self.inner.break_due()
    }

    fn resting(&self, break_for: Option<Duration>) {
        self.inner.show_break(break_for);
    }

    fn outcome(&self, outcome: &Outcome) {
        if let Some(item) = locked(&self.inner.state).queue.get_mut(&self.item_id) {
            outcome.write_to(item);
        }
        self.inner.emit(Event::Item(self.item_id.clone()));
    }
}
