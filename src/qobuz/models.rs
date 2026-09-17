//! The parts of the Qobuz API responses the application uses.
//!
//! Qobuz leaves fields out whenever it feels like it, so almost everything has a default: a
//! response that lacks a field must still be readable.

use serde::{Deserialize, Serialize};

fn yes() -> bool {
    true
}

fn one() -> u32 {
    1
}

fn album_type() -> String {
    "album".to_owned()
}

/// One page of a listing, with the total the whole listing holds.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Page<T> {
    #[serde(default)]
    pub offset: u32,
    #[serde(default)]
    pub limit: u32,
    #[serde(default)]
    pub total: u32,
    #[serde(default = "Vec::new")]
    pub items: Vec<T>,
}

impl<T> Default for Page<T> {
    fn default() -> Self {
        Self {
            offset: 0,
            limit: 0,
            total: 0,
            items: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Image {
    #[serde(default)]
    pub small: String,
    #[serde(default)]
    pub thumbnail: String,
    #[serde(default)]
    pub large: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Genre {
    #[serde(default)]
    pub name: String,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Label {
    #[serde(default)]
    pub id: u64,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub albums_count: u32,
    /// Only present on `label/get` with `extra=albums`.
    #[serde(default)]
    pub albums: Option<Page<Album>>,
}

/// An artist, composer or performer as a track or album credits them.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Person {
    #[serde(default)]
    pub id: u64,
    #[serde(default)]
    pub name: Option<String>,
}

/// An artist of a release with the roles they played on it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtistRole {
    #[serde(default)]
    pub id: u64,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub roles: Vec<String>,
}

impl ArtistRole {
    #[must_use]
    pub fn is_main_artist(&self) -> bool {
        self.roles.iter().any(|role| role == "main-artist")
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Artist {
    #[serde(default)]
    pub id: u64,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub picture: Option<String>,
    #[serde(default)]
    pub albums_count: u32,
    /// Only present on `artist/get` with `extra=albums`.
    #[serde(default)]
    pub albums: Option<Page<Album>>,
}

/// A booklet or other bonus file attached to an album.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Goody {
    #[serde(default)]
    pub id: u64,
    #[serde(default)]
    pub file_format_id: u32,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub original_url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Album {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub artist: Person,
    #[serde(default)]
    pub artists: Vec<ArtistRole>,
    #[serde(default)]
    pub image: Image,
    #[serde(default)]
    pub genre: Genre,
    #[serde(default)]
    pub label: Label,
    #[serde(default)]
    pub composer: Option<Person>,
    #[serde(default)]
    pub release_date_original: Option<String>,
    #[serde(default)]
    pub upc: Option<String>,
    #[serde(default)]
    pub copyright: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default = "album_type")]
    pub product_type: String,
    #[serde(default = "yes")]
    pub streamable: bool,
    #[serde(default)]
    pub parental_warning: bool,
    #[serde(default)]
    pub tracks_count: u32,
    #[serde(default = "one")]
    pub media_count: u32,
    #[serde(default)]
    pub maximum_bit_depth: u32,
    #[serde(default)]
    pub maximum_sampling_rate: f64,
    /// Only present on `album/get`.
    #[serde(default)]
    pub tracks: Option<Page<Track>>,
    #[serde(default)]
    pub goodies: Option<Vec<Goody>>,
}

impl Default for Album {
    fn default() -> Self {
        Self {
            id: String::new(),
            title: String::new(),
            version: None,
            url: None,
            artist: Person::default(),
            artists: Vec::new(),
            image: Image::default(),
            genre: Genre::default(),
            label: Label::default(),
            composer: None,
            release_date_original: None,
            upc: None,
            copyright: None,
            description: None,
            product_type: album_type(),
            streamable: true,
            parental_warning: false,
            tracks_count: 0,
            media_count: 1,
            maximum_bit_depth: 0,
            maximum_sampling_rate: 0.0,
            tracks: None,
            goodies: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Track {
    #[serde(default)]
    pub id: u64,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub track_number: u32,
    #[serde(default = "one")]
    pub media_number: u32,
    /// Rank in a playlist (playlists only).
    #[serde(default)]
    pub position: u32,
    /// Length in seconds.
    #[serde(default)]
    pub duration: u32,
    #[serde(default = "yes")]
    pub streamable: bool,
    #[serde(default)]
    pub parental_warning: bool,
    #[serde(default)]
    pub isrc: Option<String>,
    #[serde(default)]
    pub maximum_bit_depth: u32,
    #[serde(default)]
    pub maximum_sampling_rate: f64,
    #[serde(default)]
    pub performer: Option<Person>,
    #[serde(default)]
    pub composer: Option<Person>,
    #[serde(default)]
    pub album: Option<Album>,
    /// Qobuz numbers the entries of a playlist beyond what 32 bits hold (12 424 235 026 and up).
    #[serde(default)]
    pub playlist_track_id: Option<u64>,
}

impl Default for Track {
    fn default() -> Self {
        Self {
            id: 0,
            title: String::new(),
            version: None,
            url: None,
            track_number: 0,
            media_number: 1,
            position: 0,
            duration: 0,
            streamable: true,
            parental_warning: false,
            isrc: None,
            maximum_bit_depth: 0,
            maximum_sampling_rate: 0.0,
            performer: None,
            composer: None,
            album: None,
            playlist_track_id: None,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Playlist {
    #[serde(default)]
    pub id: u64,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub tracks_count: u32,
    #[serde(default)]
    pub owner: Option<Person>,
    #[serde(default)]
    pub images150: Option<Vec<String>>,
    /// Only present on `playlist/get` with `extra=tracks`.
    #[serde(default)]
    pub tracks: Option<Page<Track>>,
}

/// What `track/getFileUrl` answers: where to fetch the audio, and what it really is.
///
/// The quality here can differ from the one requested: asking for 24 bit / 192 kHz on a CD-quality
/// release is not an error, Qobuz simply serves what exists.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct StreamUrl {
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub format_id: u32,
    #[serde(default)]
    pub mime_type: Option<String>,
    #[serde(default)]
    pub sampling_rate: f64,
    #[serde(default)]
    pub bit_depth: u32,
    /// True when the URL only gives the 30 second preview, not the whole track.
    #[serde(default)]
    pub sample: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct UserInfo {
    #[serde(default)]
    pub id: u64,
    #[serde(default)]
    pub display_name: Option<String>,
    #[serde(default)]
    pub email: Option<String>,
    #[serde(default)]
    pub credential: Option<Credential>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Credential {
    #[serde(default)]
    pub parameters: Option<CredentialParameters>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CredentialParameters {
    #[serde(default)]
    pub short_label: Option<String>,
    #[serde(default)]
    pub lossless_streaming: bool,
    #[serde(default)]
    pub hires_streaming: bool,
}

/// `user/login`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LoginResponse {
    #[serde(default)]
    pub user_auth_token: Option<String>,
    #[serde(default)]
    pub user: Option<UserInfo>,
}

/// `album/search`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AlbumSearch {
    #[serde(default)]
    pub albums: Page<Album>,
}

/// `track/search`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct TrackSearch {
    #[serde(default)]
    pub tracks: Page<Track>,
}

/// `artist/search`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ArtistSearch {
    #[serde(default)]
    pub artists: Page<Artist>,
}

/// `playlist/search`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct PlaylistSearch {
    #[serde(default)]
    pub playlists: Page<Playlist>,
}

/// `favorite/getUserFavorites`: only the list of the requested type is filled in.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Favorites {
    #[serde(default)]
    pub albums: Option<Page<Album>>,
    #[serde(default)]
    pub tracks: Option<Page<Track>>,
    #[serde(default)]
    pub artists: Option<Page<Artist>>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn an_album_lacking_most_fields_still_reads() {
        let album: Album = serde_json::from_value(json!({ "id": "abc", "title": "Blue" })).unwrap();

        assert_eq!(album.id, "abc");
        assert!(album.streamable);
        assert_eq!(album.media_count, 1);
        assert_eq!(album.product_type, "album");
        assert_eq!(
            album,
            Album {
                id: "abc".into(),
                title: "Blue".into(),
                ..Album::default()
            }
        );
    }

    #[test]
    #[allow(clippy::float_cmp)] // the value comes straight from the JSON, no arithmetic
    fn a_full_album_response_reads_its_tracks_and_credits() {
        let album: Album = serde_json::from_value(json!({
            "id": "0886444350747",
            "title": "Kind of Blue",
            "artist": { "id": 1, "name": "Miles Davis" },
            "artists": [{ "id": 1, "name": "Miles Davis", "roles": ["main-artist"] }],
            "image": { "large": "https://img/l.jpg", "thumbnail": "https://img/t.jpg" },
            "maximum_bit_depth": 24,
            "maximum_sampling_rate": 96.0,
            "media_count": 2,
            "tracks": { "total": 1, "items": [{ "id": 20_273_753, "title": "So What", "track_number": 1, "media_number": 2, "duration": 547 }] },
            "goodies": [{ "id": 71160, "file_format_id": 21, "original_url": "https://goodies/booklet.pdf" }]
        }))
        .unwrap();

        assert_eq!(album.artist.name.as_deref(), Some("Miles Davis"));
        assert!(album.artists[0].is_main_artist());
        assert_eq!(album.maximum_sampling_rate, 96.0);
        assert_eq!(album.media_count, 2);
        let track = &album.tracks.unwrap().items[0];
        assert_eq!(
            (track.id, track.media_number, track.duration),
            (20_273_753, 2, 547)
        );
        assert_eq!(album.goodies.unwrap()[0].file_format_id, 21);
    }

    #[test]
    fn a_playlist_track_id_beyond_32_bits_is_read() {
        // Qobuz returned 12 424 235 026 and a 32-bit field broke every playlist download
        let track: Track =
            serde_json::from_value(json!({ "id": 1, "playlist_track_id": 12_424_235_026_u64 }))
                .unwrap();

        assert_eq!(track.playlist_track_id, Some(12_424_235_026));
    }

    #[test]
    fn a_stream_url_tells_a_preview_from_a_whole_track() {
        let preview: StreamUrl = serde_json::from_value(
            json!({ "url": "https://cdn/x", "format_id": 5, "sample": true }),
        )
        .unwrap();
        let full: StreamUrl = serde_json::from_value(json!({ "url": "https://cdn/x", "format_id": 6, "bit_depth": 16, "sampling_rate": 44.1 })).unwrap();

        assert!(preview.sample);
        assert!(!full.sample);
        assert_eq!((full.format_id, full.bit_depth), (6, 16));
    }

    #[test]
    fn a_listing_without_items_is_empty_rather_than_an_error() {
        let page: Page<Track> = serde_json::from_value(json!({ "total": 3 })).unwrap();

        assert_eq!(page.total, 3);
        assert!(page.items.is_empty());
    }

    #[test]
    fn a_login_response_carries_the_token_and_the_subscription() {
        let login: LoginResponse = serde_json::from_value(json!({
            "user_auth_token": "secret-token",
            "user": { "id": 7, "display_name": "me", "credential": { "parameters": { "short_label": "Studio", "hires_streaming": true } } }
        }))
        .unwrap();

        assert_eq!(login.user_auth_token.as_deref(), Some("secret-token"));
        let user = login.user.unwrap();
        assert_eq!(user.display_name.as_deref(), Some("me"));
        assert!(user.credential.unwrap().parameters.unwrap().hires_streaming);
    }

    #[test]
    fn only_the_role_main_artist_counts_as_a_main_artist() {
        let role = |roles: &[&str]| ArtistRole {
            roles: roles.iter().map(ToString::to_string).collect(),
            ..ArtistRole::default()
        };

        assert!(role(&["main-artist", "composer"]).is_main_artist());
        assert!(!role(&["featured-artist"]).is_main_artist());
        assert!(!role(&[]).is_main_artist());
    }
}
