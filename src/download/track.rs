use std::io;
use std::path::{Path, PathBuf};
use std::time::Duration;

use tokio_util::sync::CancellationToken;

use super::chain::{describe_stream, fallback_chain, quality_label};
use super::log::LogEvent;
use super::report::Reporter;
use super::transfer::{Error as TransferError, Fetcher, Transfer};
use super::verify::{self, Error as VerifyError};
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
    /// Where partial files live until they are complete. Kept apart from the music folder so that
    /// a library scanner never sees a half-written file.
    pub temp_dir: PathBuf,
    pub tuning: Tuning,
}

/// Prefix of the partial files, which is what lets a start-up clean up after a killed process.
pub const TEMP_PREFIX: &str = "qbdlx_downloading-";

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
}

impl TrackError {
    /// Whether trying again, or in a lower quality, is pointless: the whole job has to stop.
    fn is_final(&self) -> bool {
        match self {
            Self::Cancelled | Self::PreviewOnly => true,
            Self::Api(error) => error.is_unauthorized(),
            _ => false,
        }
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

        tokio::fs::create_dir_all(&self.env.temp_dir).await?;
        let temp = self.env.temp_dir.join(format!(
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
        let cover = request
            .cover
            .filter(|_| settings.tags.cover)
            .map(<[u8]>::to_vec);
        let tagged = temp.to_path_buf();
        blocking(move || tagging::write(&tagged, &values, cover.as_deref())).await??;

        let target = place(temp, wanted, settings.advanced.duplicates).await?;

        self.reporter.log(LogEvent::TrackTransfer {
            quality: received.quality,
            megabytes: received.transfer.megabytes(),
            megabytes_per_second: received.transfer.megabytes_per_second(),
        });
        self.reporter.log(LogEvent::Done);
        Ok(TrackResult {
            path: target,
            already_existed: false,
        })
    }

    /// Downloads the track into `temp`, stepping down the qualities when one keeps failing.
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

            let mut failure = None;
            let mut unavailable = false;
            for attempt in 1..=attempts {
                match self.receive(track, quality, temp).await {
                    Ok(Some(received)) => return Ok(received),
                    Ok(None) => {
                        unavailable = true;
                        break;
                    }
                    Err(error) if error.is_final() => return Err(error),
                    Err(error) => {
                        self.reporter.log(LogEvent::AttemptFailed {
                            attempt,
                            attempts,
                            quality: label.to_owned(),
                            error: error.to_string(),
                        });
                        let _ = tokio::fs::remove_file(temp).await;
                        // Qobuz answered that this will not work: asking again changes nothing
                        let hopeless =
                            matches!(&error, TrackError::Api(api) if !api.is_transient());
                        failure = Some(error);
                        if hopeless {
                            break;
                        }
                        if attempt < attempts {
                            self.pause(attempt).await?;
                        }
                    }
                }
            }

            if unavailable {
                self.reporter.log(LogEvent::TrackNotInQuality {
                    number: track.track_number,
                    quality: label.to_owned(),
                });
            } else if let Some(error) = failure {
                self.reporter.log(LogEvent::QualityFailed {
                    quality: label.to_owned(),
                    attempts,
                    error: error.to_string(),
                });
                last_error = Some(error);
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

/// Renames, or copies when the two places are on different volumes (the partial files and the
/// music are in different Docker volumes).
async fn move_file(from: &Path, to: &Path) -> io::Result<()> {
    if tokio::fs::rename(from, to).await.is_ok() {
        return Ok(());
    }
    tokio::fs::copy(from, to).await?;
    tokio::fs::remove_file(from).await
}
