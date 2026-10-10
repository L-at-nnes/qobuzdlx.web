use std::path::{Path, PathBuf};

use super::{Job, JobError};
use crate::download::item::PlaylistEntry;
use crate::download::log::LogEvent;
use crate::download::m3u;
use crate::download::report::StepTrack;
use crate::download::track::TrackRequest;
use crate::naming::{DownloadKind, make_safe_filename, track_title, truncate_to_bytes};
use crate::qobuz::{Album, Playlist, Track};

/// Room for the `.m3u8` extension below the 255 bytes a name may take.
const MAX_PLAYLIST_NAME_BYTES: usize = 200;

impl Job {
    /// Downloads a playlist: its tracks, from whichever albums they come from, then a `.m3u8` file.
    pub(super) async fn playlist(&mut self, playlist_id: u64) -> Result<(), JobError> {
        let playlist = self.env.client.playlist(playlist_id).await?;
        self.reporter.named(
            &playlist.name,
            playlist
                .owner
                .as_ref()
                .and_then(|owner| owner.name.as_deref()),
            playlist
                .images150
                .iter()
                .flatten()
                .next()
                .map(String::as_str),
        );
        let tracks = playlist
            .tracks
            .as_ref()
            .map_or(&[][..], |page| page.items.as_slice());
        let total = tracks.len();
        let mut playlist_folder: Option<PathBuf> = None;

        for (index, listed) in tracks.iter().enumerate() {
            self.check_cancelled()?;
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
                .playlist_track(&playlist, index, listed, &mut playlist_folder)
                .await
            {
                Ok(()) => {}
                Err(JobError::Track(error)) => {
                    self.track_failed(listed.id, &title, &error.to_string());
                }
                Err(other) => return Err(other),
            }
        }

        if self.settings.advanced.playlist_file
            && let Some(folder) = playlist_folder
        {
            self.write_playlist_file(&playlist, &folder).await;
        }
        Ok(())
    }

    async fn playlist_track(
        &mut self,
        playlist: &Playlist,
        index: usize,
        listed: &Track,
        playlist_folder: &mut Option<PathBuf>,
    ) -> Result<(), JobError> {
        let mut item = listed.clone();
        if self.settings.advanced.playlist_positions {
            item.track_number = item.position;
        }

        // The listing of a playlist does not say enough about the album of a track
        let album_id = match self.track_info(item.id).await? {
            Some(info) => info.album.map(|album| album.id),
            None => None,
        };
        let Some(album_id) = album_id else {
            self.outcome.left_alone();
            self.outcome_changed();
            return Ok(());
        };
        let album = self.cached_album(&album_id).await?;
        self.reporter.album(Some(&album));

        let folder = self
            .folder(Some(&album), Some(&item), Some(playlist))
            .await?;
        playlist_folder.get_or_insert_with(|| folder.clone());
        let cover = self.cover(&folder, &album).await;

        let suffix = self.collision_suffix(index, &item, &album, &folder, playlist);
        let result = self
            .context()
            .download_track(&TrackRequest {
                kind: DownloadKind::Playlist,
                album: &album,
                track: &item,
                folder: &folder,
                playlist: Some(playlist),
                suffix: &suffix,
                cover: cover.as_deref().map(Vec::as_slice),
            })
            .await?;

        self.record(result.already_existed);
        self.outcome.playlist_entry(PlaylistEntry {
            index,
            file: result.path,
            duration_seconds: item.duration,
            title: entry_title(&item),
        });
        self.outcome_changed();
        Ok(())
    }

    /// The tracks of a playlist come from different albums but end up in one folder: the `1 - Intro`
    /// of two albums would be the same name, the second one dropped as "already there" and the entry
    /// of the playlist pointing at the wrong song. A track that finds its name taken by another entry
    /// of the playlist adds its id.
    fn collision_suffix(
        &self,
        index: usize,
        track: &Track,
        album: &Album,
        folder: &Path,
        playlist: &Playlist,
    ) -> String {
        let plain = self.namer().track_path(
            folder,
            DownloadKind::Playlist,
            album,
            track,
            Some(playlist),
            "",
        );
        let taken = self.outcome.playlist_entries.iter().any(|entry| {
            entry.index != index
                && entry
                    .file
                    .to_string_lossy()
                    .eq_ignore_ascii_case(&plain.to_string_lossy())
        });
        if taken {
            format!(" ({})", track.id)
        } else {
            String::new()
        }
    }

    /// Writes `Playlist name.m3u8` in the folder of the playlist, listing every file known for the
    /// item: those of this pass and those of an earlier one. A track that failed is not in it until a
    /// "retry the failures" gets it.
    async fn write_playlist_file(&self, playlist: &Playlist, folder: &Path) {
        if self.outcome.playlist_entries.is_empty() {
            return;
        }

        let mut name =
            truncate_to_bytes(&make_safe_filename(&playlist.name), MAX_PLAYLIST_NAME_BYTES);
        if name.is_empty() {
            name = playlist.id.to_string();
        }
        let file_name = format!("{name}.m3u8");
        let content = m3u::build(folder, &self.outcome.playlist_entries);

        match tokio::fs::write(folder.join(&file_name), content).await {
            Ok(()) => self.log(LogEvent::PlaylistFileWritten { name: file_name }),
            Err(error) => self.log(LogEvent::PlaylistFileFailed {
                error: error.to_string(),
            }),
        }
    }
}

/// What players show for an entry: `Artist - Title`.
fn entry_title(track: &Track) -> String {
    let title = track_title(track);
    match track
        .performer
        .as_ref()
        .and_then(|performer| performer.name.as_deref())
        .filter(|name| !name.is_empty())
    {
        Some(name) => format!("{name} - {title}"),
        None => title,
    }
}
