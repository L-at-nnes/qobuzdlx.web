//! Processing one queue item: what to download for an address, album by album and track by track.

mod album;
mod collections;
mod playlist;

use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use tokio_util::sync::CancellationToken;

use super::artwork::Covers;
use super::item::QueueItem;
use super::log::LogEvent;
use super::progress::ProgressScope;
use super::report::{Outcome, Reporter, StepTrack};
use super::track::{Context, Environment, TrackError};
use crate::extensions::TrackSaved;
use crate::naming::Namer;
use crate::qobuz::{self, Album, Playlist, Track};
use crate::settings::Settings;

/// How many albums are remembered. The point is to catch neighbours (a playlist often draws several
/// tracks from one album), not to remember everything: a full cache is simply emptied.
const ALBUM_CACHE_MAX: usize = 50;

#[derive(Debug, thiserror::Error)]
pub enum JobError {
    #[error("cancelled")]
    Cancelled,
    /// Qobuz refused the session token, or nobody is signed in any more: nothing more can be asked.
    #[error("the Qobuz session expired")]
    SessionExpired,
    #[error("{0}")]
    Api(qobuz::Error),
    /// An address whose identifier is not what its kind needs.
    #[error("not a valid Qobuz address")]
    InvalidAddress,
    /// A track that was asked for on its own could not be downloaded.
    #[error("{0}")]
    Track(TrackError),
    /// The disk is full (or over its quota, or read-only): nothing more can be written.
    #[error("the disk cannot take any more files: {0}")]
    StorageFull(String),
}

impl From<qobuz::Error> for JobError {
    fn from(error: qobuz::Error) -> Self {
        if error.is_session_gone() {
            Self::SessionExpired
        } else {
            Self::Api(error)
        }
    }
}

impl From<TrackError> for JobError {
    fn from(error: TrackError) -> Self {
        match error {
            TrackError::Cancelled => Self::Cancelled,
            TrackError::Api(api) if api.is_session_gone() => Self::SessionExpired,
            full if full.is_storage_full() => Self::StorageFull(full.to_string()),
            other => Self::Track(other),
        }
    }
}

/// The reporter of one track among several downloading at the same time: what it says about its
/// file is told as the file of that position.
struct TrackReporter<'a> {
    base: &'a dyn Reporter,
    index: usize,
}

impl Reporter for TrackReporter<'_> {
    fn log(&self, event: LogEvent) {
        self.base.log(event);
    }

    fn album(&self, album: Option<&Album>) {
        self.base.album(album);
    }

    fn step(&self, percent: u8, track: Option<StepTrack>) {
        self.base.step(percent, track);
    }

    fn transfer(&self, update: super::transfer::Update) {
        self.base.track_transfer(self.index, update);
    }

    fn track_saved(&self, saved: TrackSaved) {
        self.base.track_saved(saved);
    }

    fn outcome(&self, outcome: &Outcome) {
        self.base.outcome(outcome);
    }
}

/// What a job is given to work with.
pub struct JobSetup {
    pub env: Environment,
    /// The settings as they were when the item started: changing them mid-way would give a folder
    /// two naming schemes.
    pub settings: Settings,
    pub reporter: Arc<dyn Reporter>,
    pub cancel: CancellationToken,
    /// Set by "skip": the album in progress is dropped and the item goes on.
    pub skip: Arc<AtomicBool>,
    pub covers: Covers,
    /// The Qobuz id of the account, for favourites.
    pub user_id: u64,
}

impl std::fmt::Debug for JobSetup {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_struct("JobSetup").finish_non_exhaustive()
    }
}

/// The processing of one queue item.
pub struct Job {
    env: Environment,
    settings: Settings,
    reporter: Arc<dyn Reporter>,
    cancel: CancellationToken,
    skip: Arc<AtomicBool>,
    covers: Covers,
    user_id: u64,
    /// Set by "retry the failures": only these tracks are processed.
    only: Option<BTreeSet<u64>>,
    outcome: Outcome,
    scope: ProgressScope,
    albums: HashMap<String, Arc<Album>>,
    pictures: HashMap<String, Option<Arc<Vec<u8>>>>,
}

impl std::fmt::Debug for Job {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_struct("Job").finish_non_exhaustive()
    }
}

impl Job {
    #[must_use]
    pub fn new(setup: JobSetup, item: &QueueItem) -> Self {
        Self {
            env: setup.env,
            settings: setup.settings,
            reporter: setup.reporter,
            cancel: setup.cancel,
            skip: setup.skip,
            covers: setup.covers,
            user_id: setup.user_id,
            only: item.only_track_ids.clone(),
            outcome: Outcome::of(item),
            scope: ProgressScope::FULL,
            albums: HashMap::new(),
            pictures: HashMap::new(),
        }
    }

    /// What has been achieved, whether the job ended well or not.
    #[must_use]
    pub fn into_outcome(self) -> Outcome {
        self.outcome
    }

    fn log(&self, event: LogEvent) {
        self.reporter.log(event);
    }

    fn namer(&self) -> Namer<'_> {
        Namer::new(&self.settings)
    }

    fn context(&self) -> Context<'_> {
        self.context_for(self.reporter.as_ref(), &self.cancel)
    }

    /// The same, reporting to `reporter` and stopped by `cancel`: what a track that downloads beside
    /// others needs.
    fn context_for<'a>(
        &'a self,
        reporter: &'a dyn Reporter,
        cancel: &'a CancellationToken,
    ) -> Context<'a> {
        Context {
            env: &self.env,
            settings: &self.settings,
            reporter,
            cancel,
        }
    }

    fn check_cancelled(&self) -> Result<(), JobError> {
        if self.cancel.is_cancelled() {
            Err(JobError::Cancelled)
        } else {
            Ok(())
        }
    }

    /// Whether "skip" was asked for since the last time, which asks again.
    fn take_skip(&self) -> bool {
        self.skip.swap(false, Ordering::SeqCst)
    }

    /// A "retry the failures" pass leaves every track but the failed ones alone.
    fn excluded(&self, track_id: u64) -> bool {
        self.only
            .as_ref()
            .is_some_and(|only| !only.contains(&track_id))
    }

    /// Moves the progress to the `index`-th of `total` things, and names the track in progress.
    fn step(&self, index: usize, total: usize, track: Option<StepTrack>) {
        let percent = (index * 100).checked_div(total).unwrap_or(0);
        let percent = u8::try_from(percent.min(100)).unwrap_or(100);
        self.reporter.step(self.scope.map(percent), track);
    }

    fn outcome_changed(&self) {
        self.reporter.outcome(&self.outcome);
    }

    /// The folder of an album, a track or a playlist, created.
    async fn folder(
        &self,
        album: Option<&Album>,
        track: Option<&Track>,
        playlist: Option<&Playlist>,
    ) -> Result<PathBuf, JobError> {
        let folder = self
            .namer()
            .folder(&self.settings.download.directory, album, track, playlist);
        tokio::fs::create_dir_all(&folder)
            .await
            .map_err(|error| JobError::from(TrackError::Io(error)))?;
        Ok(folder)
    }

    /// The album with all its tracks, from the cache when it was fetched earlier in this item.
    async fn cached_album(&mut self, album_id: &str) -> Result<Arc<Album>, JobError> {
        if let Some(album) = self.albums.get(album_id) {
            return Ok(Arc::clone(album));
        }

        let album = Arc::new(self.env.client.album(album_id).await?);
        if self.albums.len() >= ALBUM_CACHE_MAX {
            self.albums.clear();
            self.pictures.clear();
        }
        self.albums.insert(album_id.to_owned(), Arc::clone(&album));
        Ok(album)
    }

    /// A track by its id, `None` when Qobuz does not know it.
    async fn track_info(&self, track_id: u64) -> Result<Option<Track>, JobError> {
        match self.env.client.track(track_id).await {
            Ok(track) => Ok(Some(track)),
            Err(error) if error.status().is_some_and(|status| status.as_u16() == 404) => Ok(None),
            Err(error) => Err(error.into()),
        }
    }

    /// Saves `Cover.jpg` in the folder and returns the picture to embed in the tracks.
    ///
    /// A cover that cannot be had is not a reason to refuse the music: it is reported in the log of
    /// the server and the tracks go without.
    async fn cover(&mut self, folder: &Path, album: &Album) -> Option<Arc<Vec<u8>>> {
        let artwork = &self.settings.artwork;

        let mut saved = None;
        let on_disk = folder.join("Cover.jpg");
        if artwork.save_to_disk && !on_disk.exists() {
            match self.covers.fetch(album, artwork.saved_size).await {
                Ok(picture) => {
                    if let Err(error) = tokio::fs::write(&on_disk, &picture).await {
                        tracing::warn!(%error, "cover could not be saved");
                    }
                    saved = Some(picture);
                }
                Err(error) => {
                    tracing::warn!(%error, "cover could not be downloaded");
                }
            }
        }

        if !self.settings.tags.cover {
            return None;
        }
        if let Some(cached) = self.pictures.get(&album.id) {
            return cached.clone();
        }
        let embedded = if artwork.embedded_size == artwork.saved_size && saved.is_some() {
            saved
        } else {
            match self.covers.fetch(album, artwork.embedded_size).await {
                Ok(picture) => Some(picture),
                Err(error) => {
                    tracing::warn!(%error, "cover could not be downloaded");
                    None
                }
            }
        };
        let embedded = embedded.map(Arc::new);
        self.pictures.insert(album.id.clone(), embedded.clone());
        embedded
    }
}

#[cfg(test)]
mod tests {
    use std::io::{Error, ErrorKind};

    use super::*;
    use crate::download::transfer::Error as TransferError;

    #[test]
    fn a_full_disk_stops_the_job_whichever_way_it_was_met() {
        for kind in [
            ErrorKind::StorageFull,
            ErrorKind::QuotaExceeded,
            ErrorKind::ReadOnlyFilesystem,
        ] {
            let writing = TrackError::Io(Error::from(kind));
            let receiving = TrackError::Transfer(TransferError::Io(Error::from(kind)));

            assert!(writing.is_storage_full(), "{kind:?}");
            assert!(receiving.is_storage_full(), "{kind:?}");
            assert!(matches!(JobError::from(writing), JobError::StorageFull(_)));
            assert!(matches!(
                JobError::from(receiving),
                JobError::StorageFull(_)
            ));
        }
    }

    #[test]
    fn other_write_errors_only_cost_their_track() {
        let error = TrackError::Io(Error::from(ErrorKind::PermissionDenied));

        assert!(!error.is_storage_full());
        assert!(matches!(JobError::from(error), JobError::Track(_)));
    }

    #[test]
    fn a_dropped_connection_is_not_a_full_disk() {
        let error = TrackError::Transfer(TransferError::Stalled(std::time::Duration::from_secs(1)));

        assert!(!error.is_storage_full());
    }
}
