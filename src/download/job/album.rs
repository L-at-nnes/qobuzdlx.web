use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

use futures_util::StreamExt;
use futures_util::stream::FuturesUnordered;
use tokio_util::sync::CancellationToken;

use super::{Job, JobError, TrackReporter};
use crate::download::goody::goody_file_name;
use crate::download::log::LogEvent;
use crate::download::report::StepTrack;
use crate::download::track::{Context, TrackRequest};
use crate::naming::{DownloadKind, track_title};
use crate::qobuz::{Album, Goody, Track};

/// What a track downloading beside others knows of the album's run.
struct TrackSlot<'a> {
    /// Position among the tracks of the album, from 0, and how many there are.
    index: usize,
    total: usize,
    /// Set by "skip": the tracks that have not started are dropped.
    skipped: &'a AtomicBool,
    /// Stops the tracks of the album: the item ended, or was cancelled.
    stop: &'a CancellationToken,
}

/// How a track of an album ended, when it did not fail.
enum TrackEnd {
    Written,
    /// The file was in place already.
    AlreadyThere,
    /// Qobuz does not offer it: counted as left alone.
    LeftAlone,
}

/// How far an album has got, in percent, when `finished` of its `total` tracks are in place.
fn share(finished: usize, total: usize) -> u8 {
    let percent = (finished * 100).checked_div(total).unwrap_or(0);
    u8::try_from(percent.min(100)).unwrap_or(100)
}

impl Job {
    /// Downloads an album: its cover, its tracks, and its bonus files.
    pub(super) async fn album(&mut self, album_id: &str) -> Result<(), JobError> {
        let album = self.cached_album(album_id).await?;
        self.reporter.album(Some(&album));

        if !album.streamable && self.settings.advanced.streamable_check {
            self.log(LogEvent::AlbumNotStreamable);
            return Ok(());
        }

        let folder = self.folder(Some(&album), None, None).await?;
        let cover = self.cover(&folder, &album).await;

        let tracks = album
            .tracks
            .as_ref()
            .map_or(&[][..], |page| page.items.as_slice());
        let skipped = self
            .download_tracks(&album, tracks, &folder, cover.as_deref())
            .await?;

        // An album the user chose to skip is skipped entirely, and a retry of failed tracks has no
        // business with the booklets
        if !skipped && self.only.is_none() && self.settings.advanced.download_goodies {
            for goody in album.goodies.iter().flatten() {
                self.check_cancelled()?;
                self.goody(&folder, &album, goody).await;
            }
        }
        Ok(())
    }

    /// The tracks of an album, several at a time as the settings say. Returns whether the album was
    /// skipped on request.
    ///
    /// A track that fails costs only itself. Anything that ends the item (a cancel, a refused
    /// session, a full disk) stops the tracks still downloading, which are then waited for so that
    /// none leaves a partial file behind.
    async fn download_tracks(
        &mut self,
        album: &Album,
        tracks: &[Track],
        folder: &Path,
        cover: Option<&Vec<u8>>,
    ) -> Result<bool, JobError> {
        let total = tracks.len();
        let skipped = AtomicBool::new(false);
        let stop = self.cancel.child_token();
        // The counts are held apart from the job while the tracks run: they only read the job
        let mut outcome = std::mem::take(&mut self.outcome);
        let mut ended = None;
        let mut finished = 0;

        {
            let job = &*self;
            let parallel = job.settings.advanced.parallel();
            let mut waiting = tracks.iter().enumerate();
            let mut running = FuturesUnordered::new();

            loop {
                // Kept full as long as there are tracks to start and nothing has stopped the item
                while running.len() < parallel && !stop.is_cancelled() {
                    let Some((index, listed)) = waiting.next() else {
                        break;
                    };
                    running.push(job.one_track(
                        TrackSlot {
                            index,
                            total,
                            skipped: &skipped,
                            stop: &stop,
                        },
                        album,
                        listed,
                        folder,
                        cover,
                    ));
                }
                let Some(done) = running.next().await else {
                    break;
                };
                let Some((track_id, title, result)) = done else {
                    continue;
                };
                match result {
                    Ok(TrackEnd::Written) => outcome.written(),
                    Ok(TrackEnd::AlreadyThere | TrackEnd::LeftAlone) => outcome.left_alone(),
                    Err(JobError::Track(error)) => {
                        let error = error.to_string();
                        job.log(LogEvent::TrackFailed {
                            track_id,
                            error: error.clone(),
                        });
                        outcome.failed(track_id, title, error);
                    }
                    Err(other) => {
                        // What follows is the others stopping, not another cause
                        ended.get_or_insert(other);
                        stop.cancel();
                        continue;
                    }
                }
                finished += 1;
                job.reporter.overall(job.scope.map(share(finished, total)));
                job.reporter.outcome(&outcome);
            }
        }

        self.outcome = outcome;
        if let Some(error) = ended {
            return Err(error);
        }
        self.check_cancelled()?;
        Ok(skipped.load(Ordering::SeqCst))
    }

    /// A track of an album that downloads beside others: `None` when it was left alone (skipped,
    /// not wanted, or the item has stopped), else its id, its title and how it went.
    async fn one_track(
        &self,
        slot: TrackSlot<'_>,
        album: &Album,
        listed: &Track,
        folder: &Path,
        cover: Option<&Vec<u8>>,
    ) -> Option<(u64, String, Result<TrackEnd, JobError>)> {
        if slot.stop.is_cancelled() || slot.skipped.load(Ordering::SeqCst) {
            return None;
        }
        if self.take_skip() {
            slot.skipped.store(true, Ordering::SeqCst);
            self.log(LogEvent::AlbumSkipped);
            return None;
        }
        if self.excluded(listed.id) {
            return None;
        }

        let title = track_title(listed);
        let position = slot.index + 1;
        self.reporter.track_started(StepTrack {
            title: title.clone(),
            index: position,
            total: slot.total,
        });
        let reporter = TrackReporter {
            base: self.reporter.as_ref(),
            index: position,
        };
        let context = self.context_for(&reporter, slot.stop);
        let result = self
            .album_track(&context, album, listed, folder, cover)
            .await;
        self.reporter.track_ended(position);
        Some((listed.id, title, result))
    }

    /// One track of an album, from the page of the album.
    async fn album_track(
        &self,
        context: &Context<'_>,
        album: &Album,
        listed: &Track,
        folder: &Path,
        cover: Option<&Vec<u8>>,
    ) -> Result<TrackEnd, JobError> {
        // The listing of an album holds less than the track itself
        let Some(track) = self.track_info(listed.id).await? else {
            self.log(LogEvent::TrackUnavailable {
                number: listed.track_number,
            });
            return Ok(TrackEnd::LeftAlone);
        };
        if !track.streamable && self.settings.advanced.streamable_check {
            self.log(LogEvent::TrackNotStreamable {
                number: track.track_number,
            });
            return Ok(TrackEnd::LeftAlone);
        }

        let result = context
            .download_track(&TrackRequest {
                kind: DownloadKind::Album,
                album,
                track: &track,
                folder,
                playlist: None,
                suffix: "",
                cover: cover.map(Vec::as_slice),
            })
            .await?;
        Ok(if result.already_existed {
            TrackEnd::AlreadyThere
        } else {
            TrackEnd::Written
        })
    }

    /// Downloads one track on its own, into the folder of its album.
    pub(super) async fn single_track(&mut self, track_id: u64) -> Result<(), JobError> {
        let Some(track) = self.track_info(track_id).await? else {
            self.log(LogEvent::TrackNotFound);
            self.outcome
                .failed(track_id, track_id.to_string(), "Track not found");
            self.outcome_changed();
            return Ok(());
        };
        let title = track_title(&track);
        self.step(
            0,
            1,
            Some(StepTrack {
                title: title.clone(),
                index: 1,
                total: 1,
            }),
        );

        let Some(listed) = track.album.as_ref() else {
            self.album_unavailable(&track, &title);
            return Ok(());
        };
        let album = self.cached_album(&listed.id).await?;
        self.reporter.album(Some(&album));
        self.reporter.named(
            &title,
            track
                .performer
                .as_ref()
                .and_then(|performer| performer.name.as_deref()),
            Some(album.image.thumbnail.as_str()),
        );

        let folder = self.folder(Some(&album), Some(&track), None).await?;
        let cover = self.cover(&folder, &album).await;
        let result = self
            .context()
            .download_track(&TrackRequest {
                kind: DownloadKind::Track,
                album: &album,
                track: &track,
                folder: &folder,
                playlist: None,
                suffix: "",
                cover: cover.as_deref().map(Vec::as_slice),
            })
            .await?;
        self.record(result.already_existed);
        Ok(())
    }

    fn album_unavailable(&mut self, track: &Track, title: &str) {
        self.log(LogEvent::AlbumInfoUnavailable);
        self.outcome.failed(
            track.id,
            title,
            "The album of this track could not be found",
        );
        self.outcome_changed();
    }

    /// Counts a track that is in place: written now, or already there.
    pub(super) fn record(&mut self, already_existed: bool) {
        if already_existed {
            self.outcome.left_alone();
        } else {
            self.outcome.written();
        }
        self.outcome_changed();
    }

    /// Notes a track that could not be downloaded, and goes on with the next one.
    pub(super) fn track_failed(&mut self, track_id: u64, title: &str, error: &str) {
        self.log(LogEvent::TrackFailed {
            track_id,
            error: error.to_owned(),
        });
        self.outcome.failed(track_id, title, error);
        self.outcome_changed();
    }

    /// Downloads a booklet or other bonus file next to the tracks. Never fails the album.
    async fn goody(&self, folder: &Path, album: &Album, goody: &Goody) {
        let Some(name) = goody_file_name(album, goody) else {
            return;
        };
        let target = folder.join(name);
        if target.exists() {
            self.log(LogEvent::GoodyExists);
            return;
        }
        let Some(url) = goody.url.as_deref() else {
            return;
        };

        self.log(LogEvent::GoodyFound);
        // A partial file never takes the name of the real one: it would be skipped next time
        let partial = target.with_extension("part");
        let result = self
            .env
            .fetcher
            .to_file(url, &partial, &self.cancel, |_| {})
            .await;
        let result = match result {
            Ok(_) => tokio::fs::rename(&partial, &target)
                .await
                .map_err(|error| error.to_string()),
            Err(error) => Err(error.to_string()),
        };
        match result {
            Ok(()) => self.log(LogEvent::Done),
            Err(error) => {
                let _ = tokio::fs::remove_file(&partial).await;
                self.log(LogEvent::GoodyFailed { error });
            }
        }
    }
}
