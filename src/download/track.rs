use std::io;
use std::path::{Path, PathBuf};
use std::time::Duration;

use tokio_util::sync::CancellationToken;

use super::chain::{describe_stream, fallback_chain, quality_label};
use super::lapse::Refusals;
use super::log::LogEvent;
use super::report::Reporter;
use super::transfer::{Error as TransferError, Fetcher, Transfer};
use super::verify::{self, Error as VerifyError};
use crate::extensions::TrackSaved;
use crate::naming::{DownloadKind, Namer, track_title};
use crate::qobuz::{self, Album, Client, Playlist, Track};
use crate::settings::{DuplicateMode, Quality, Settings};
use crate::tagging;

/// How a quality is retried before giving way to the next one.
#[derive(Debug, Clone, Copy)]
pub struct Tuning {
    /// A file the CDN has not served yet is cut after a few seconds ("unexpected EOF") for the first
    /// three or four requests and served after: fewer attempts stop one short of it.
    pub attempts_per_quality: u32,
    /// Pause after the first failed attempt, multiplied by the number of attempts so far.
    pub backoff: Duration,
}

impl Default for Tuning {
    fn default() -> Self {
        Self {
            attempts_per_quality: 5,
            backoff: Duration::from_secs(1),
        }
    }
}

/// What downloading a track needs from the outside.
#[derive(Debug, Clone)]
pub struct Environment {
    pub client: Client,
    pub fetcher: Fetcher,
    pub tuning: Tuning,
    /// The tracks Qobuz refused to give, one after the other: what tells that the account can no
    /// longer download.
    pub refusals: Refusals,
}

/// Prefix of the partial files, which is what lets a start-up clean up after a killed process.
pub const TEMP_PREFIX: &str = "qbdlx_downloading-";

/// The folder of the partial files, inside the download folder. Hidden, so that a library scanner
/// leaves it alone.
pub const PARTIAL_DIR: &str = ".qobuzdlx-partial";

/// Where partial files live until they are complete: next to the music, on the same volume. A
/// finished file then takes its place by being renamed, which either happens or does not: a track
/// is never half there under its own name, and it is written to the disk once, not twice.
#[must_use]
pub fn partial_dir(settings: &Settings) -> PathBuf {
    settings.download.directory.join(PARTIAL_DIR)
}

/// Everything a track download works with.
#[derive(Clone, Copy)]
pub struct Context<'a> {
    pub env: &'a Environment,
    pub settings: &'a Settings,
    pub reporter: &'a dyn Reporter,
    pub cancel: &'a CancellationToken,
}

impl std::fmt::Debug for Context<'_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_struct("Context").finish_non_exhaustive()
    }
}

/// The track to download and where it goes.
#[derive(Debug, Clone, Copy)]
pub struct TrackRequest<'a> {
    pub kind: DownloadKind,
    pub album: &'a Album,
    pub track: &'a Track,
    pub folder: &'a Path,
    pub playlist: Option<&'a Playlist>,
    /// Appended to the name of the file, before its extension.
    pub suffix: &'a str,
    /// The picture to embed, already downloaded.
    pub cover: Option<&'a [u8]>,
}

/// A track that is in place.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrackResult {
    pub path: PathBuf,
    /// The file was there already and was left as it is.
    pub already_existed: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum TrackError {
    #[error("cancelled")]
    Cancelled,
    #[error("Qobuz only gave the 30 second preview of this track")]
    PreviewOnly,
    #[error("{0}")]
    Api(#[from] qobuz::Error),
    #[error("{0}")]
    Transfer(#[from] TransferError),
    #[error("{0}")]
    Damaged(VerifyError),
    #[error("{0}")]
    Tag(#[from] tagging::Error),
    #[error("{0}")]
    Io(#[from] io::Error),
    #[error("the track is not available in any quality")]
    Unavailable,
    /// Several tracks in a row were refused and none written: the account can no longer download.
    #[error("Qobuz keeps refusing the files: the subscription seems to have ended")]
    SubscriptionLapsed,
}

impl TrackError {
    /// Whether trying again, or in a lower quality, is pointless: the whole job has to stop. Asking
    /// again at once is what Qobuz just refused.
    fn is_final(&self) -> bool {
        match self {
            Self::Cancelled | Self::PreviewOnly | Self::SubscriptionLapsed => true,
            Self::Api(error) => {
                error.is_session_gone() || error.is_rate_limited() || error.is_blocked()
            }
            _ => self.is_storage_full(),
        }
    }

    /// Whether Qobuz refused the file itself, as it does for an account that cannot download: a
    /// preview instead of the track, or a refusal of the request that is not about the session, the
    /// rate or a track that does not exist.
    fn is_refusal(&self) -> bool {
        match self {
            Self::PreviewOnly => true,
            Self::Api(error) => error.status().is_some_and(|status| {
                status.is_client_error() && !matches!(status.as_u16(), 401 | 404 | 408 | 429)
            }),
            _ => false,
        }
    }

    /// Whether the error says that this quality is not there to be had, as opposed to one that is
    /// there and could not be fetched this time: Qobuz answered that the request will not work, or
    /// the CDN that it has no such file. Only then is a lower quality worth taking: a file fetched
    /// lower because the network hiccuped would be kept for ever in place of the one asked for.
    fn means_no_such_quality(&self) -> bool {
        match self {
            Self::Api(api) => !api.is_transient(),
            Self::Transfer(TransferError::Status(status)) => {
                status.is_client_error() && !matches!(status.as_u16(), 408 | 429)
            }
            _ => false,
        }
    }

    /// Whether the disk cannot take any more: it is full, over its quota or read-only. Every other
    /// track would meet the same refusal.
    #[must_use]
    pub fn is_storage_full(&self) -> bool {
        let (Self::Io(error) | Self::Transfer(TransferError::Io(error))) = self else {
            return false;
        };
        matches!(
            error.kind(),
            io::ErrorKind::StorageFull
                | io::ErrorKind::QuotaExceeded
                | io::ErrorKind::ReadOnlyFilesystem
        )
    }
}

/// What extensions are told about a track that is in place.
fn saved(request: &TrackRequest<'_>, path: &Path, quality: String) -> TrackSaved {
    let name = |person: &qobuz::Person| person.name.clone();
    TrackSaved {
        path: path.to_string_lossy().into_owned(),
        folder: path
            .parent()
            .map_or_else(String::new, |folder| folder.to_string_lossy().into_owned()),
        track_id: request.track.id,
        title: track_title(request.track),
        track_number: request.track.track_number,
        disc_number: request.track.media_number,
        artist: request.track.performer.as_ref().and_then(name),
        album_id: request.album.id.clone(),
        album_title: request.album.title.clone(),
        album_artist: name(&request.album.artist),
        quality,
    }
}

/// A track received in full.
struct Received {
    quality: String,
    transfer: Transfer,
}

impl Context<'_> {
    /// Downloads a track, tags it and puts it in its place.
    pub async fn download_track(
        &self,
        request: &TrackRequest<'_>,
    ) -> Result<TrackResult, TrackError> {
        let namer = Namer::new(self.settings);
        let wanted = namer.track_path(
            request.folder,
            request.kind,
            request.album,
            request.track,
            request.playlist,
            request.suffix,
        );

        if self.settings.advanced.duplicates == DuplicateMode::Skip && wanted.exists() {
            self.reporter.log(LogEvent::FileExists {
                number: request.track.track_number,
            });
            return Ok(TrackResult {
                path: wanted,
                already_existed: true,
            });
        }

        let partial_dir = partial_dir(self.settings);
        tokio::fs::create_dir_all(&partial_dir).await?;
        let temp = partial_dir.join(format!(
            "{TEMP_PREFIX}{}{}",
            request.track.id,
            self.settings.download.quality.extension()
        ));
        self.reporter.log(LogEvent::DownloadingTrack {
            number: request.track.track_number,
            title: track_title(request.track),
        });

        // Whatever happens, cancellation, a tagging error or a failed move included, the partial
        // file must not stay behind
        let result = self.produce(request, &temp, wanted).await;
        let _ = tokio::fs::remove_file(&temp).await;
        self.watch_account(result)
    }

    /// Keeps count of the tracks Qobuz refused one after the other. A file written says the account
    /// can download; the refusal that makes the series long enough ends the item, whatever the
    /// reason the track itself gives.
    fn watch_account(
        &self,
        result: Result<TrackResult, TrackError>,
    ) -> Result<TrackResult, TrackError> {
        match &result {
            Ok(track) if !track.already_existed => self.env.refusals.served(),
            Err(error) if error.is_refusal() && self.env.refusals.refused() => {
                self.reporter.log(LogEvent::SubscriptionLapsed);
                return Err(TrackError::SubscriptionLapsed);
            }
            _ => {}
        }
        result
    }

    async fn produce(
        &self,
        request: &TrackRequest<'_>,
        temp: &Path,
        wanted: PathBuf,
    ) -> Result<TrackResult, TrackError> {
        let settings = self.settings;
        let received = self.receive_with_fallback(request.track, temp).await?;

        if settings.advanced.fix_md5 && settings.download.quality.is_flac() {
            self.fix_md5(temp).await;
        }

        let values = tagging::build(
            &settings.tags,
            settings.naming.merge_artists,
            request.album,
            request.track,
        );
        let cover = request.cover.filter(|_| settings.tags.cover);
        self.tag(temp, values, cover).await?;

        let target = place(temp, wanted, settings.advanced.duplicates).await?;

        self.reporter.log(LogEvent::TrackTransfer {
            quality: received.quality.clone(),
            megabytes: received.transfer.megabytes(),
            megabytes_per_second: received.transfer.megabytes_per_second(),
        });
        self.reporter.log(LogEvent::Done);
        self.reporter
            .track_saved(saved(request, &target, received.quality));
        Ok(TrackResult {
            path: target,
            already_existed: false,
        })
    }

    /// Downloads the track into `temp`, in the quality asked for, or in the next one down that
    /// exists when that one does not.
    ///
    /// A quality that exists and could not be fetched (the connection kept being cut, the file kept
    /// arriving damaged) is tried again, then fails the track, which can be retried: it is not
    /// replaced by a lower one. The log says which quality was taken when it is not the one asked.
    async fn receive_with_fallback(
        &self,
        track: &Track,
        temp: &Path,
    ) -> Result<Received, TrackError> {
        let requested = self.settings.download.quality;
        let attempts = self.env.tuning.attempts_per_quality;
        let mut last_error = None;

        for quality in fallback_chain(requested) {
            let label = quality_label(quality);
            if quality != requested {
                self.reporter.log(LogEvent::QualityUnavailable {
                    quality: label.to_owned(),
                });
            }

            // `None`: Qobuz has no stream of this quality. An error: it says this one will not come.
            let mut missing = None;
            let mut attempt = 0;
            loop {
                attempt += 1;
                match self.receive(track, quality, temp).await {
                    Ok(Some(received)) => {
                        if quality != requested {
                            self.reporter.log(LogEvent::QualityLowered {
                                number: track.track_number,
                                quality: received.quality.clone(),
                            });
                        }
                        return Ok(received);
                    }
                    Ok(None) => break,
                    Err(error) if error.is_final() => return Err(error),
                    Err(error) => {
                        self.reporter.log(LogEvent::AttemptFailed {
                            attempt,
                            attempts,
                            quality: label.to_owned(),
                            error: error.to_string(),
                        });
                        let _ = tokio::fs::remove_file(temp).await;
                        // Asking again changes nothing: the next quality down may exist
                        if error.means_no_such_quality() {
                            missing = Some(error);
                            break;
                        }
                        if attempt >= attempts {
                            self.reporter.log(LogEvent::QualityFailed {
                                quality: label.to_owned(),
                                attempts,
                                error: error.to_string(),
                            });
                            return Err(error);
                        }
                        self.pause(attempt).await?;
                    }
                }
            }

            self.reporter.log(LogEvent::TrackNotInQuality {
                number: track.track_number,
                quality: label.to_owned(),
            });
            if missing.is_some() {
                last_error = missing;
            }
        }

        Err(last_error.unwrap_or(TrackError::Unavailable))
    }

    /// One attempt: a fresh signed address, the file, and its check. `None` when Qobuz has no
    /// stream of this quality.
    async fn receive(
        &self,
        track: &Track,
        quality: Quality,
        temp: &Path,
    ) -> Result<Option<Received>, TrackError> {
        let stream = self
            .env
            .client
            .stream_url(track.id, quality.format_id())
            .await?;
        let Some(url) = stream.url.as_deref().filter(|url| !url.is_empty()) else {
            return Ok(None);
        };
        // Without this the 30 second extract would be tagged and filed as a whole track
        if stream.sample {
            self.reporter.log(LogEvent::PreviewOnly);
            return Err(TrackError::PreviewOnly);
        }

        let transfer = self
            .env
            .fetcher
            .to_file(url, temp, self.cancel, |update| {
                self.reporter.transfer(update);
            })
            .await
            .map_err(|error| match error {
                TransferError::Cancelled => TrackError::Cancelled,
                other => TrackError::Transfer(other),
            })?;

        if self.settings.advanced.verify_integrity && quality.is_flac() {
            let checked = temp.to_path_buf();
            blocking(move || verify::verify_flac(&checked))
                .await?
                .map_err(TrackError::Damaged)?;
        }

        Ok(Some(Received {
            quality: describe_stream(&stream, quality),
            transfer,
        }))
    }

    /// Writes the tags. A picture that cannot be embedded (an error page served as a cover) is no
    /// reason to lose the track: it goes without.
    async fn tag(
        &self,
        path: &Path,
        values: tagging::TagValues,
        cover: Option<&[u8]>,
    ) -> Result<(), TrackError> {
        let target = path.to_path_buf();
        let picture = cover.map(<[u8]>::to_vec);
        let fallback = values.clone();
        let result = blocking({
            let target = target.clone();
            move || tagging::write(&target, &values, picture.as_deref())
        })
        .await?;

        match result {
            Err(error) if cover.is_some() => {
                tracing::warn!(%error, "cover could not be embedded, tagging without it");
                blocking(move || tagging::write(&target, &fallback, None)).await??;
                Ok(())
            }
            other => Ok(other?),
        }
    }

    /// A failure is not fatal: the track stays playable, so it is reported and the download goes on.
    async fn fix_md5(&self, path: &Path) {
        let target = path.to_path_buf();
        let error = match blocking(move || verify::fix_md5(&target)).await {
            Ok(Ok(())) => return,
            Ok(Err(error)) => error.to_string(),
            Err(error) => error.to_string(),
        };
        self.reporter.log(LogEvent::Md5FixFailed { error });
    }

    async fn pause(&self, attempt: u32) -> Result<(), TrackError> {
        tokio::select! {
            () = self.cancel.cancelled() => Err(TrackError::Cancelled),
            () = tokio::time::sleep(self.env.tuning.backoff * attempt) => Ok(()),
        }
    }
}

/// Runs blocking work (decoding, tagging) off the async threads.
async fn blocking<T: Send + 'static>(work: impl FnOnce() -> T + Send + 'static) -> io::Result<T> {
    tokio::task::spawn_blocking(work)
        .await
        .map_err(io::Error::other)
}

/// Moves the finished file to its place and returns where it ended up.
async fn place(temp: &Path, wanted: PathBuf, duplicates: DuplicateMode) -> io::Result<PathBuf> {
    if let Some(directory) = wanted.parent() {
        tokio::fs::create_dir_all(directory).await?;
    }
    let target = if duplicates == DuplicateMode::Rename {
        free_name(wanted)
    } else {
        wanted
    };
    move_file(temp, &target).await?;
    Ok(target)
}

/// `name.flac`, or `name (1).flac`, `name (2).flac`... the first one that is free.
fn free_name(wanted: PathBuf) -> PathBuf {
    if !wanted.exists() {
        return wanted;
    }
    let stem = wanted
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned();
    let extension = wanted
        .extension()
        .map(|extension| format!(".{}", extension.to_string_lossy()))
        .unwrap_or_default();
    let mut counter = 1;
    loop {
        let candidate = wanted.with_file_name(format!("{stem} ({counter}){extension}"));
        if !candidate.exists() {
            return candidate;
        }
        counter += 1;
    }
}

/// Renames, or copies when the two places are on different volumes (a folder of the library that is
/// mounted from elsewhere).
async fn move_file(from: &Path, to: &Path) -> io::Result<()> {
    if tokio::fs::rename(from, to).await.is_ok() {
        return Ok(());
    }
    copy_into_place(from, to).await?;
    tokio::fs::remove_file(from).await
}

/// Copies `from` to `to` without `to` ever holding a part of it: the copy is made under another name
/// next to it, and takes the name once it is whole. A copy cut short (the disk fills up, the process
/// is stopped) must not be taken for the track the next time, and left as it is for ever.
async fn copy_into_place(from: &Path, to: &Path) -> io::Result<()> {
    let mut name = to.file_name().unwrap_or_default().to_os_string();
    name.push(".part");
    let partial = to.with_file_name(name);

    let copied = async {
        tokio::fs::copy(from, &partial).await?;
        tokio::fs::rename(&partial, to).await
    }
    .await;
    if copied.is_err() {
        let _ = tokio::fs::remove_file(&partial).await;
    }
    copied
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn names_in(folder: &Path) -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(folder)
            .unwrap()
            .flatten()
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    }

    #[tokio::test]
    async fn a_copy_takes_its_name_only_once_it_is_whole() {
        let source = TempDir::new().unwrap();
        let library = TempDir::new().unwrap();
        let from = source.path().join("partial.flac");
        std::fs::write(&from, b"the whole track").unwrap();
        let to = library.path().join("01 - So What.flac");

        copy_into_place(&from, &to).await.unwrap();

        assert_eq!(std::fs::read(&to).unwrap(), b"the whole track");
        assert_eq!(names_in(library.path()), ["01 - So What.flac"]);
    }

    #[tokio::test]
    async fn a_copy_that_fails_leaves_nothing_under_the_name_of_the_track() {
        let source = TempDir::new().unwrap();
        let library = TempDir::new().unwrap();
        let to = library.path().join("01 - So What.flac");

        let result = copy_into_place(&source.path().join("missing.flac"), &to).await;

        assert!(result.is_err());
        assert!(names_in(library.path()).is_empty());
    }

    #[test]
    fn partial_files_live_in_a_hidden_folder_of_the_download_folder() {
        let mut settings = Settings::default();
        settings.download.directory = PathBuf::from("/music");

        assert_eq!(
            partial_dir(&settings),
            Path::new("/music").join(".qobuzdlx-partial")
        );
    }
}
