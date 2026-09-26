use super::{Job, JobError};
use crate::download::item::{ItemKind, QueueItem};
use crate::download::log::LogEvent;
use crate::download::report::StepTrack;
use crate::naming::track_title;
use crate::qobuz::FavoriteKind;

impl Job {
    /// Processes the item for its address, and says how it went.
    pub async fn run(&mut self, item: &QueueItem) -> Result<(), JobError> {
        self.log(LogEvent::ItemStarted {
            label: item.label.clone(),
        });
        self.log(LogEvent::CheckingLink);

        match item.kind {
            ItemKind::Album => self.album(&item.ref_id).await?,
            ItemKind::Track => self.single_track(numeric(&item.ref_id)?).await?,
            ItemKind::Playlist => self.playlist(numeric(&item.ref_id)?).await?,
            ItemKind::Artist => self.artist(numeric(&item.ref_id)?).await?,
            ItemKind::Label => self.label(numeric(&item.ref_id)?).await?,
            ItemKind::FavoriteAlbums => self.favorite_albums().await?,
            ItemKind::FavoriteTracks => self.favorite_tracks().await?,
            ItemKind::FavoriteArtists => self.favorite_artists().await?,
        }

        let outcome = &self.outcome;
        if outcome.downloaded + outcome.skipped > 0 || !outcome.failures.is_empty() {
            self.log(LogEvent::ItemSummary {
                downloaded: outcome.downloaded,
                skipped: outcome.skipped,
                failed: u32::try_from(outcome.failures.len()).unwrap_or(u32::MAX),
            });
        }
        self.log(LogEvent::Complete);
        Ok(())
    }

    /// Every album of an artist.
    async fn artist(&mut self, artist_id: u64) -> Result<(), JobError> {
        let artist = self.env.client.artist(artist_id).await?;
        let albums = artist
            .albums
            .map(|page| page.items.into_iter().map(|album| album.id).collect())
            .unwrap_or_default();
        self.each_album(albums).await
    }

    /// Every album of a label.
    async fn label(&mut self, label_id: u64) -> Result<(), JobError> {
        let label = self.env.client.label(label_id).await?;
        let Some(page) = label.albums else {
            self.log(LogEvent::LabelNotFound);
            return Ok(());
        };
        self.each_album(page.items.into_iter().map(|album| album.id).collect())
            .await
    }

    async fn favorite_albums(&mut self) -> Result<(), JobError> {
        let favorites = self
            .env
            .client
            .favorites(self.user_id, FavoriteKind::Albums)
            .await?;
        let albums: Vec<String> = favorites
            .albums
            .map(|page| page.items.into_iter().map(|album| album.id).collect())
            .unwrap_or_default();
        if albums.is_empty() {
            self.log(LogEvent::FavoritesEmpty);
            return Ok(());
        }
        self.each_album(albums).await
    }

    async fn favorite_tracks(&mut self) -> Result<(), JobError> {
        let favorites = self
            .env
            .client
            .favorites(self.user_id, FavoriteKind::Tracks)
            .await?;
        let tracks = favorites.tracks.map(|page| page.items).unwrap_or_default();
        if tracks.is_empty() {
            self.log(LogEvent::FavoritesEmpty);
            return Ok(());
        }

        let total = tracks.len();
        for (index, track) in tracks.iter().enumerate() {
            self.check_cancelled()?;
            if self.excluded(track.id) {
                continue;
            }
            let title = track_title(track);
            self.step(
                index,
                total,
                Some(StepTrack {
                    title: title.clone(),
                    index: index + 1,
                    total,
                }),
            );

            match self.single_track(track.id).await {
                Ok(()) => {}
                Err(error @ (JobError::Track(_) | JobError::Api(_))) => {
                    self.track_failed(track.id, &title, &error.to_string());
                }
                Err(other) => return Err(other),
            }
        }
        Ok(())
    }

    async fn favorite_artists(&mut self) -> Result<(), JobError> {
        let favorites = self
            .env
            .client
            .favorites(self.user_id, FavoriteKind::Artists)
            .await?;
        let artists = favorites.artists.map(|page| page.items).unwrap_or_default();
        if artists.is_empty() {
            self.log(LogEvent::FavoritesEmpty);
            return Ok(());
        }

        let total = artists.len();
        for (index, listed) in artists.iter().enumerate() {
            self.check_cancelled()?;
            let outer = self.scope;
            self.scope = outer.slice(index, total);
            let result = self.artist(listed.id).await;
            self.scope = outer;

            match result {
                Ok(()) => {}
                Err(error @ (JobError::Track(_) | JobError::Api(_))) => {
                    self.log(LogEvent::ArtistFailed {
                        artist_id: listed.id,
                        error: error.to_string(),
                    });
                }
                Err(other) => return Err(other),
            }
        }
        Ok(())
    }

    /// Albums one after the other, each taking its share of the progress bar. An album that cannot be
    /// had is reported and the next one goes on.
    async fn each_album(&mut self, album_ids: Vec<String>) -> Result<(), JobError> {
        let total = album_ids.len();
        for (index, album_id) in album_ids.iter().enumerate() {
            self.check_cancelled()?;
            if self.take_skip() {
                self.log(LogEvent::AlbumSkipped);
                continue;
            }
            self.step(index, total, None);

            let outer = self.scope;
            self.scope = outer.slice(index, total);
            let result = self.album(album_id).await;
            self.scope = outer;

            match result {
                Ok(()) => {}
                Err(error @ (JobError::Track(_) | JobError::Api(_))) => {
                    self.log(LogEvent::AlbumFailed {
                        album_id: album_id.clone(),
                        error: error.to_string(),
                    });
                }
                Err(other) => return Err(other),
            }
        }
        Ok(())
    }
}

/// An identifier that is a number, which every kind but albums is.
fn numeric(ref_id: &str) -> Result<u64, JobError> {
    ref_id.parse().map_err(|_| JobError::InvalidAddress)
}
