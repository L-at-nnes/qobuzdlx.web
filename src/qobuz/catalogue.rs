//! Reading the catalogue: albums, tracks, playlists, artists, labels, favourites and search.

use super::client::Auth;
use super::pagination::{PAGE_SIZE, complete};
use super::{
    Album, AlbumSearch, Artist, ArtistSearch, Client, Error, Favorites, Label, Page, Playlist,
    PlaylistSearch, Result, StreamUrl, Track, TrackSearch, sign_file_url,
};

/// The kinds of favourites a user can list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FavoriteKind {
    Albums,
    Tracks,
    Artists,
}

impl FavoriteKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::Albums => "albums",
            Self::Tracks => "tracks",
            Self::Artists => "artists",
        }
    }
}

fn paging(offset: u32, limit: u32) -> [(&'static str, String); 2] {
    [("limit", limit.to_string()), ("offset", offset.to_string())]
}

impl Client {
    // ─── Album ───

    /// An album with all its tracks.
    pub async fn album(&self, album_id: &str) -> Result<Album> {
        let mut album = self.album_page(album_id, 0).await?;

        if let Some(tracks) = album.tracks.as_mut() {
            complete(tracks, |offset| async move {
                Ok(self
                    .album_page(album_id, offset)
                    .await?
                    .tracks
                    .unwrap_or_default())
            })
            .await?;
        }
        Ok(album)
    }

    async fn album_page(&self, album_id: &str, offset: u32) -> Result<Album> {
        let [limit, offset] = paging(offset, PAGE_SIZE);
        self.signed_in_get(
            "album/get",
            &[("album_id", album_id.to_owned()), limit, offset],
        )
        .await
    }

    // ─── Track ───

    pub async fn track(&self, track_id: u64) -> Result<Track> {
        self.signed_in_get("track/get", &[("track_id", track_id.to_string())])
            .await
    }

    /// Where to download a track at the given Qobuz `format_id`.
    ///
    /// The request is signed with the app secret. Whatever quality is asked for, Qobuz serves the best
    /// the release has up to it: look at the answer's `format_id`, `bit_depth` and `sampling_rate` for
    /// what the file really is.
    pub async fn stream_url(&self, track_id: u64, format_id: u32) -> Result<StreamUrl> {
        let session = self.session().ok_or(Error::NotSignedIn)?;
        if session.app.app_secret.is_empty() {
            return Err(Error::MissingAppSecret);
        }

        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |elapsed| elapsed.as_secs());
        let params = [
            ("request_ts", timestamp.to_string()),
            (
                "request_sig",
                sign_file_url(track_id, format_id, timestamp, &session.app.app_secret),
            ),
            ("track_id", track_id.to_string()),
            ("format_id", format_id.to_string()),
            ("intent", "stream".to_owned()),
            ("user_auth_token", session.user_auth_token.clone()),
        ];

        let auth = Auth::User {
            app_id: &session.app.app_id,
            token: &session.user_auth_token,
        };
        let result = self.get_json("track/getFileUrl", &params, auth).await;
        self.announce_rejection(result)
    }

    // ─── Playlist ───

    /// A playlist with all its tracks.
    pub async fn playlist(&self, playlist_id: u64) -> Result<Playlist> {
        let mut playlist = self.playlist_page(playlist_id, 0).await?;

        if let Some(tracks) = playlist.tracks.as_mut() {
            complete(tracks, |offset| async move {
                Ok(self
                    .playlist_page(playlist_id, offset)
                    .await?
                    .tracks
                    .unwrap_or_default())
            })
            .await?;
        }
        Ok(playlist)
    }

    async fn playlist_page(&self, playlist_id: u64, offset: u32) -> Result<Playlist> {
        let [limit, offset] = paging(offset, PAGE_SIZE);
        let params = [
            ("playlist_id", playlist_id.to_string()),
            ("extra", "tracks".to_owned()),
            limit,
            offset,
        ];
        self.signed_in_get("playlist/get", &params).await
    }

    // ─── Artist ───

    /// An artist with their whole discography.
    pub async fn artist(&self, artist_id: u64) -> Result<Artist> {
        let mut artist = self.artist_page(artist_id, 0).await?;

        if let Some(albums) = artist.albums.as_mut() {
            complete(albums, |offset| async move {
                Ok(self
                    .artist_page(artist_id, offset)
                    .await?
                    .albums
                    .unwrap_or_default())
            })
            .await?;
        }
        Ok(artist)
    }

    async fn artist_page(&self, artist_id: u64, offset: u32) -> Result<Artist> {
        let [limit, offset] = paging(offset, PAGE_SIZE);
        let params = [
            ("artist_id", artist_id.to_string()),
            ("extra", "albums,albums_with_last_release".to_owned()),
            limit,
            offset,
        ];
        self.signed_in_get("artist/get", &params).await
    }

    // ─── Label ───

    /// A label with its whole catalogue. A large label holds thousands of albums: Qobuz answers 500 at
    /// a time.
    pub async fn label(&self, label_id: u64) -> Result<Label> {
        let mut label = self.label_page(label_id, 0).await?;

        if let Some(albums) = label.albums.as_mut() {
            complete(albums, |offset| async move {
                Ok(self
                    .label_page(label_id, offset)
                    .await?
                    .albums
                    .unwrap_or_default())
            })
            .await?;
        }
        Ok(label)
    }

    async fn label_page(&self, label_id: u64, offset: u32) -> Result<Label> {
        let [limit, offset] = paging(offset, PAGE_SIZE);
        let params = [
            ("label_id", label_id.to_string()),
            ("extra", "albums".to_owned()),
            limit,
            offset,
        ];
        self.signed_in_get("label/get", &params).await
    }

    // ─── Favourites ───

    /// Everything the user has marked as a favourite of the given kind.
    pub async fn favorites(&self, user_id: u64, kind: FavoriteKind) -> Result<Favorites> {
        let mut favorites = self.favorites_page(user_id, kind, 0).await?;

        // Only the list of the requested kind is filled in
        let fetch = |offset| self.favorites_page(user_id, kind, offset);
        if let Some(albums) = favorites.albums.as_mut() {
            complete(albums, |offset| async move {
                Ok(fetch(offset).await?.albums.unwrap_or_default())
            })
            .await?;
        }
        if let Some(tracks) = favorites.tracks.as_mut() {
            complete(tracks, |offset| async move {
                Ok(fetch(offset).await?.tracks.unwrap_or_default())
            })
            .await?;
        }
        if let Some(artists) = favorites.artists.as_mut() {
            complete(artists, |offset| async move {
                Ok(fetch(offset).await?.artists.unwrap_or_default())
            })
            .await?;
        }
        Ok(favorites)
    }

    async fn favorites_page(
        &self,
        user_id: u64,
        kind: FavoriteKind,
        offset: u32,
    ) -> Result<Favorites> {
        let [limit, offset] = paging(offset, PAGE_SIZE);
        let params = [
            ("user_id", user_id.to_string()),
            ("type", kind.as_str().to_owned()),
            limit,
            offset,
        ];
        self.signed_in_get("favorite/getUserFavorites", &params)
            .await
    }

    // ─── Search ───

    pub async fn search_albums(&self, query: &str, limit: u32, offset: u32) -> Result<Page<Album>> {
        let [limit, offset] = paging(offset, limit);
        let result: AlbumSearch = self
            .signed_in_get(
                "album/search",
                &[("query", query.to_owned()), limit, offset],
            )
            .await?;
        Ok(result.albums)
    }

    pub async fn search_tracks(&self, query: &str, limit: u32, offset: u32) -> Result<Page<Track>> {
        let [limit, offset] = paging(offset, limit);
        let result: TrackSearch = self
            .signed_in_get(
                "track/search",
                &[("query", query.to_owned()), limit, offset],
            )
            .await?;
        Ok(result.tracks)
    }

    pub async fn search_artists(
        &self,
        query: &str,
        limit: u32,
        offset: u32,
    ) -> Result<Page<Artist>> {
        let [limit, offset] = paging(offset, limit);
        let result: ArtistSearch = self
            .signed_in_get(
                "artist/search",
                &[("query", query.to_owned()), limit, offset],
            )
            .await?;
        Ok(result.artists)
    }

    pub async fn search_playlists(
        &self,
        query: &str,
        limit: u32,
        offset: u32,
    ) -> Result<Page<Playlist>> {
        let [limit, offset] = paging(offset, limit);
        let result: PlaylistSearch = self
            .signed_in_get(
                "playlist/search",
                &[("query", query.to_owned()), limit, offset],
            )
            .await?;
        Ok(result.playlists)
    }
}
