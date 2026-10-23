use std::collections::BTreeSet;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use rand::RngExt;
use serde::{Deserialize, Serialize};

/// What a queue item downloads.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ItemKind {
    Album,
    Track,
    Playlist,
    /// The whole discography of an artist.
    Artist,
    /// The whole catalogue of a label.
    Label,
    FavoriteAlbums,
    FavoriteTracks,
    FavoriteArtists,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ItemStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Cancelled,
}

impl ItemStatus {
    #[must_use]
    pub const fn is_finished(self) -> bool {
        matches!(self, Self::Completed | Self::Failed | Self::Cancelled)
    }
}

/// A track that could not be downloaded, with Qobuz's reason.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FailedTrack {
    pub track_id: u64,
    pub title: String,
    pub error: String,
}

/// A file written for a playlist: where it is, how long it plays and what players show for it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlaylistEntry {
    /// Position in the playlist, from 0.
    pub index: usize,
    pub file: PathBuf,
    pub duration_seconds: u32,
    pub title: String,
}

/// What the interface knows about an item before it is queued.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Display {
    pub label: Option<String>,
    pub sub_label: Option<String>,
    pub thumbnail: Option<String>,
}

/// One entry of the download queue.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QueueItem {
    /// Stable identity, used by the interface to point at an item. Uniqueness is decided by `url`.
    pub id: String,
    /// The canonical `play.qobuz.com` address: it decides what counts as the same item.
    pub url: String,
    pub kind: ItemKind,
    /// The identifier Qobuz knows the thing by (an album id is text, the others are numbers).
    pub ref_id: String,
    pub label: String,
    pub sub_label: Option<String>,
    pub thumbnail: Option<String>,
    pub status: ItemStatus,
    pub error: Option<String>,
    /// Unix timestamps, in seconds.
    pub enqueued_at: u64,
    pub started_at: Option<u64>,
    pub finished_at: Option<u64>,
    /// Files written, files skipped (already there, unavailable) and tracks that failed.
    pub downloaded: u32,
    pub skipped: u32,
    pub failures: Vec<FailedTrack>,
    /// The files of a playlist already in place, kept from one "retry the failures" to the next so
    /// that the final `.m3u8` lists them all.
    pub playlist_entries: Vec<PlaylistEntry>,
    /// Set by "retry the failures": only these tracks are processed.
    pub only_track_ids: Option<BTreeSet<u64>>,
}

pub(super) fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs())
}

impl QueueItem {
    #[must_use]
    pub fn new(url: String, kind: ItemKind, ref_id: String, display: Display) -> Self {
        let label = display.label.filter(|label| !label.trim().is_empty());
        Self {
            id: format!("{:08x}", rand::rng().random::<u32>()),
            label: label.unwrap_or_else(|| url.clone()),
            url,
            kind,
            ref_id,
            sub_label: display.sub_label,
            thumbnail: display.thumbnail,
            status: ItemStatus::Pending,
            error: None,
            enqueued_at: now(),
            started_at: None,
            finished_at: None,
            downloaded: 0,
            skipped: 0,
            failures: Vec::new(),
            playlist_entries: Vec::new(),
            only_track_ids: None,
        }
    }

    #[must_use]
    pub const fn is_finished(&self) -> bool {
        self.status.is_finished()
    }

    /// Forgets what was counted, before the whole item is processed again.
    pub fn reset_outcome(&mut self) {
        self.downloaded = 0;
        self.skipped = 0;
        self.failures.clear();
        self.playlist_entries.clear();
        self.only_track_ids = None;
    }

    /// Makes a running item wait again, to be processed from the start.
    pub(super) fn put_back(&mut self) {
        self.status = ItemStatus::Pending;
        self.started_at = None;
        // A whole item restarts from scratch: what it had counted would be counted twice. A
        // "retry the failures" pass keeps its earlier outcome, which its own counters build on.
        if self.only_track_ids.is_none() {
            self.reset_outcome();
        }
    }

    /// Puts the item back at the start of its life, as if it had just been queued.
    pub(super) fn requeue(&mut self) {
        self.status = ItemStatus::Pending;
        self.error = None;
        self.started_at = None;
        self.finished_at = None;
        self.enqueued_at = now();
    }
}
