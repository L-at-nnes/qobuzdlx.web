//! What the interface is told about albums, tracks, artists and playlists.
//!
//! Only what it shows, sorts or filters by: the models of Qobuz hold far more, and are not part of
//! the contract with the interface.

use serde::Serialize;

use crate::download::{
    FailedTrack, ItemKind, ItemStatus, Progress, QueueItem, Snapshot, canonical_url,
};
use crate::qobuz::{Album, Artist, Playlist, Track};

#[derive(Debug, Serialize)]
pub struct AlbumItem {
    pub id: String,
    /// The address the download queue knows the album by.
    pub url: String,
    pub title: String,
    pub version: Option<String>,
    pub artist: String,
    pub release_date: Option<String>,
    pub tracks_count: u32,
    pub bit_depth: u32,
    pub sampling_rate: f64,
    pub label: String,
    pub thumbnail: String,
    pub parental_warning: bool,
}

impl From<&Album> for AlbumItem {
    fn from(album: &Album) -> Self {
        Self {
            id: album.id.clone(),
            url: canonical_url(ItemKind::Album, &album.id),
            title: album.title.clone(),
            version: album.version.clone(),
            artist: album.artist.name.clone().unwrap_or_default(),
            release_date: album.release_date_original.clone(),
            tracks_count: album.tracks_count,
            bit_depth: album.maximum_bit_depth,
            sampling_rate: album.maximum_sampling_rate,
            label: album.label.name.clone(),
            thumbnail: album.image.thumbnail.clone(),
            parental_warning: album.parental_warning,
        }
    }
}

/// The album a track comes from, as much as a list of tracks shows of it.
#[derive(Debug, Serialize)]
pub struct AlbumRef {
    pub id: String,
    pub title: String,
    pub thumbnail: String,
    pub release_date: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct TrackItem {
    pub id: u64,
    pub url: String,
    pub title: String,
    pub version: Option<String>,
    pub number: u32,
    /// The disc of a multi-disc album, from 1.
    pub disc: u32,
    pub artist: String,
    pub album: Option<AlbumRef>,
    pub duration: u32,
    pub bit_depth: u32,
    pub sampling_rate: f64,
    pub parental_warning: bool,
}

impl From<&Track> for TrackItem {
    fn from(track: &Track) -> Self {
        Self {
            id: track.id,
            url: canonical_url(ItemKind::Track, &track.id.to_string()),
            title: track.title.clone(),
            version: track.version.clone(),
            number: track.track_number,
            disc: track.media_number,
            artist: track
                .performer
                .as_ref()
                .and_then(|performer| performer.name.clone())
                .unwrap_or_default(),
            album: track.album.as_ref().map(|album| AlbumRef {
                id: album.id.clone(),
                title: album.title.clone(),
                thumbnail: album.image.thumbnail.clone(),
                release_date: album.release_date_original.clone(),
            }),
            duration: track.duration,
            bit_depth: track.maximum_bit_depth,
            sampling_rate: track.maximum_sampling_rate,
            parental_warning: track.parental_warning,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct ArtistItem {
    pub id: u64,
    pub url: String,
    pub name: String,
    pub picture: Option<String>,
    pub albums_count: u32,
}

impl From<&Artist> for ArtistItem {
    fn from(artist: &Artist) -> Self {
        Self {
            id: artist.id,
            url: canonical_url(ItemKind::Artist, &artist.id.to_string()),
            name: artist.name.clone(),
            picture: artist.picture.clone().filter(|picture| !picture.is_empty()),
            albums_count: artist.albums_count,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct PlaylistItem {
    pub id: u64,
    pub url: String,
    pub name: String,
    pub owner: Option<String>,
    pub tracks_count: u32,
    pub image: Option<String>,
}

impl From<&Playlist> for PlaylistItem {
    fn from(playlist: &Playlist) -> Self {
        Self {
            id: playlist.id,
            url: canonical_url(ItemKind::Playlist, &playlist.id.to_string()),
            name: playlist.name.clone(),
            owner: playlist.owner.as_ref().and_then(|owner| owner.name.clone()),
            tracks_count: playlist.tracks_count,
            image: playlist
                .images150
                .as_ref()
                .and_then(|images| images.first().cloned())
                .filter(|image| !image.is_empty()),
        }
    }
}

/// An item of the download queue, as the page shows it.
///
/// Without what the server keeps for itself (the files of a playlist already in place, the tracks a
/// retry is limited to): a playlist of a thousand tracks would otherwise weigh as much as the rest of
/// the queue each time the queue is sent.
#[derive(Debug, Serialize)]
pub struct ItemView<'a> {
    pub id: &'a str,
    pub url: &'a str,
    pub kind: ItemKind,
    pub ref_id: &'a str,
    pub label: &'a str,
    pub sub_label: Option<&'a str>,
    pub thumbnail: Option<&'a str>,
    pub status: ItemStatus,
    pub error: Option<&'a str>,
    pub enqueued_at: u64,
    pub started_at: Option<u64>,
    pub finished_at: Option<u64>,
    pub downloaded: u32,
    pub skipped: u32,
    pub failures: &'a [FailedTrack],
}

impl<'a> From<&'a QueueItem> for ItemView<'a> {
    fn from(item: &'a QueueItem) -> Self {
        Self {
            id: &item.id,
            url: &item.url,
            kind: item.kind,
            ref_id: &item.ref_id,
            label: &item.label,
            sub_label: item.sub_label.as_deref(),
            thumbnail: item.thumbnail.as_deref(),
            status: item.status,
            error: item.error.as_deref(),
            enqueued_at: item.enqueued_at,
            started_at: item.started_at,
            finished_at: item.finished_at,
            downloaded: item.downloaded,
            skipped: item.skipped,
            failures: &item.failures,
        }
    }
}

/// The queue and what is going on, as the page reads them.
#[derive(Debug, Serialize)]
pub struct SnapshotView<'a> {
    pub queue: Vec<ItemView<'a>>,
    pub progress: &'a Progress,
    pub remaining: usize,
}

impl<'a> From<&'a Snapshot> for SnapshotView<'a> {
    fn from(snapshot: &'a Snapshot) -> Self {
        Self {
            queue: snapshot.queue.iter().map(ItemView::from).collect(),
            progress: &snapshot.progress,
            remaining: snapshot.remaining,
        }
    }
}

/// A page of results.
#[derive(Debug, Serialize)]
pub struct Results<T> {
    pub total: u32,
    pub offset: u32,
    pub items: Vec<T>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn an_album_carries_what_the_list_shows_and_sorts_by() {
        let album: Album = serde_json::from_value(json!({
            "id": "abc", "title": "Kind of Blue", "version": "Mono",
            "artist": { "id": 1, "name": "Miles Davis" },
            "release_date_original": "1959-08-17", "tracks_count": 5,
            "maximum_bit_depth": 24, "maximum_sampling_rate": 96.0,
            "label": { "id": 2, "name": "Columbia" },
            "image": { "thumbnail": "https://static.example/t.jpg" },
            "parental_warning": true,
        }))
        .unwrap();

        let value = serde_json::to_value(AlbumItem::from(&album)).unwrap();

        assert_eq!(value["url"], "https://play.qobuz.com/album/abc");
        assert_eq!(value["artist"], "Miles Davis");
        assert_eq!(value["bit_depth"], 24);
        assert_eq!(value["sampling_rate"], 96.0);
        assert_eq!(value["label"], "Columbia");
        assert_eq!(value["thumbnail"], "https://static.example/t.jpg");
        assert_eq!(value["version"], "Mono");
    }

    #[test]
    fn a_track_names_its_artist_and_the_album_it_comes_from() {
        let track: Track = serde_json::from_value(json!({
            "id": 42, "title": "So What", "track_number": 1, "media_number": 2, "duration": 562,
            "performer": { "id": 1, "name": "Miles Davis" },
            "album": { "id": "abc", "title": "Kind of Blue", "artist": { "id": 1 },
                       "release_date_original": "1959-08-17" },
        }))
        .unwrap();

        let value = serde_json::to_value(TrackItem::from(&track)).unwrap();

        assert_eq!(value["url"], "https://play.qobuz.com/track/42");
        assert_eq!(value["disc"], 2);
        assert_eq!(value["artist"], "Miles Davis");
        assert_eq!(value["album"]["title"], "Kind of Blue");
        assert_eq!(value["duration"], 562);
    }

    #[test]
    fn missing_names_are_empty_rather_than_absent() {
        let track: Track = serde_json::from_value(json!({ "id": 1, "title": "T" })).unwrap();

        let value = serde_json::to_value(TrackItem::from(&track)).unwrap();

        assert_eq!(value["artist"], "");
        assert_eq!(value["album"], json!(null));
    }

    #[test]
    fn an_artist_without_a_picture_has_none() {
        let artist: Artist = serde_json::from_value(
            json!({ "id": 3, "name": "Nina", "picture": "", "albums_count": 12 }),
        )
        .unwrap();

        let value = serde_json::to_value(ArtistItem::from(&artist)).unwrap();

        assert_eq!(value["picture"], json!(null));
        assert_eq!(value["albums_count"], 12);
        assert_eq!(value["url"], "https://play.qobuz.com/artist/3");
    }

    #[test]
    fn a_playlist_shows_its_first_image_and_its_owner() {
        let playlist: Playlist = serde_json::from_value(json!({
            "id": 7, "name": "Evening", "tracks_count": 20,
            "owner": { "id": 1, "name": "Ada" },
            "images150": ["https://static.example/a.jpg", "https://static.example/b.jpg"],
        }))
        .unwrap();

        let value = serde_json::to_value(PlaylistItem::from(&playlist)).unwrap();

        assert_eq!(value["image"], "https://static.example/a.jpg");
        assert_eq!(value["owner"], "Ada");
        assert_eq!(value["url"], "https://play.qobuz.com/playlist/7");
    }
}
