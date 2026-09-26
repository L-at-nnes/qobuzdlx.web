use std::path::Path;

use super::{Job, JobError};
use crate::download::goody::goody_file_name;
use crate::download::log::LogEvent;
use crate::download::report::StepTrack;
use crate::download::track::{TrackError, TrackRequest};
use crate::naming::{DownloadKind, track_title};
use crate::qobuz::{Album, Goody, Track};

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
        let total = tracks.len();
        let mut skipped = false;

        for (index, listed) in tracks.iter().enumerate() {
            self.check_cancelled()?;
            if self.take_skip() {
                self.log(LogEvent::AlbumSkipped);
                skipped = true;
                break;
            }
            if self.excluded(listed.id) {
                continue;
            }
            let title = track_title(listed);
            self.step(
                index,
                total,
                Some(StepTrack {
                    title: title.clone(),
                    index: index + 1,
                    total,
                }),
            );

            match self
                .album_track(&album, listed, &folder, cover.as_deref())
                .await
            {
                Ok(()) => {}
                Err(JobError::Track(error)) => self.track_failed(listed.id, &title, &error),
                Err(other) => return Err(other),
            }
        }

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

    /// One track of an album, from the page of the album.
    async fn album_track(
        &mut self,
        album: &Album,
        listed: &Track,
        folder: &Path,
        cover: Option<&Vec<u8>>,
    ) -> Result<(), JobError> {
        // The listing of an album holds less than the track itself
        let Some(track) = self.track_info(listed.id).await? else {
            self.log(LogEvent::TrackUnavailable {
                number: listed.track_number,
            });
            self.outcome.left_alone();
            self.outcome_changed();
            return Ok(());
        };
        if !track.streamable && self.settings.advanced.streamable_check {
            self.log(LogEvent::TrackNotStreamable {
                number: track.track_number,
            });
            self.outcome.left_alone();
            self.outcome_changed();
            return Ok(());
        }

        let result = self
            .context()
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
        self.record(result.already_existed);
        Ok(())
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
    pub(super) fn track_failed(&mut self, track_id: u64, title: &str, error: &TrackError) {
        self.log(LogEvent::TrackFailed {
            track_id,
            error: error.to_string(),
        });
        self.outcome.failed(track_id, title, error.to_string());
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
