use std::sync::Arc;

use super::{Event, Inner, locked};
use crate::download::log::LogEvent;
use crate::download::progress::{AlbumCard, TrackProgress};
use crate::download::report::{Outcome, Reporter, StepTrack};
use crate::download::transfer::Update;
use crate::qobuz::Album;
use crate::settings::LogDetail;

/// What the job of one item says, put where the interface reads it.
pub(super) struct ItemReporter {
    pub(super) inner: Arc<Inner>,
    pub(super) item_id: String,
    /// The level of detail when the item started: the log does not change halfway through.
    pub(super) detail: LogDetail,
}

impl Reporter for ItemReporter {
    fn log(&self, event: LogEvent) {
        self.inner.log_at(event, self.detail);
    }

    fn album(&self, album: Option<&Album>) {
        let mut state = locked(&self.inner.state);
        state.progress.album = album.map(AlbumCard::from);
        self.inner.progress_changed(&mut state, true);
    }

    fn step(&self, percent: u8, track: Option<StepTrack>) {
        let mut state = locked(&self.inner.state);
        state.progress.percent = percent;
        state.progress.track = track.map(|track| TrackProgress {
            title: track.title,
            index: track.index,
            total: track.total,
            percent: 0,
            bytes_per_second: 0,
        });
        self.inner.progress_changed(&mut state, true);
    }

    fn transfer(&self, update: Update) {
        let mut state = locked(&self.inner.state);
        if let Some(track) = state.progress.track.as_mut() {
            track.percent = update.percent;
            track.bytes_per_second = update.bytes_per_second;
        }
        self.inner.progress_changed(&mut state, false);
    }

    fn outcome(&self, outcome: &Outcome) {
        if let Some(item) = locked(&self.inner.state).queue.get_mut(&self.item_id) {
            outcome.write_to(item);
        }
        self.inner.emit(Event::Queue);
    }
}
