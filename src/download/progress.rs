use serde::Serialize;

use crate::naming::album_title;
use crate::qobuz::Album;

/// The part of the overall progress bar that what is being processed takes up.
///
/// An artist or a label goes through albums that each report their own progress from 0 to 100 %:
/// without a scope the bar would start again at zero with every album instead of reflecting the whole.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ProgressScope {
    start: f64,
    span: f64,
}

impl ProgressScope {
    /// The whole bar.
    pub const FULL: Self = Self {
        start: 0.0,
        span: 100.0,
    };

    /// The part for the `index`-th of `total` things inside this scope.
    #[must_use]
    #[allow(clippy::cast_precision_loss)] // A count of items is far below 2^52
    pub fn slice(self, index: usize, total: usize) -> Self {
        if total == 0 {
            return self;
        }
        let share = self.span / total as f64;
        Self {
            start: self.start + share * index as f64,
            span: share,
        }
    }

    /// Where a progress of `percent` within this scope falls on the whole bar.
    #[must_use]
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // Clamped to 0..=100
    pub fn map(self, percent: u8) -> u8 {
        let mapped = self.start + self.span * f64::from(percent.min(100)) / 100.0;
        mapped.clamp(0.0, 100.0) as u8
    }
}

/// The album being processed, as the interface shows it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AlbumCard {
    pub title: String,
    pub artist: String,
    pub release_date: Option<String>,
    pub tracks_count: u32,
    pub genre: String,
    pub label: String,
    /// `24bit / 96kHz`
    pub quality: String,
    pub cover: String,
}

impl From<&Album> for AlbumCard {
    fn from(album: &Album) -> Self {
        let quality = if album.maximum_bit_depth > 0 && album.maximum_sampling_rate > 0.0 {
            format!(
                "{}bit / {}kHz",
                album.maximum_bit_depth, album.maximum_sampling_rate
            )
        } else {
            String::new()
        };
        Self {
            title: album_title(album),
            artist: album.artist.name.clone().unwrap_or_default(),
            release_date: album.release_date_original.clone(),
            tracks_count: album.tracks_count,
            genre: album.genre.name.clone(),
            label: album.label.name.clone(),
            quality,
            cover: album.image.large.clone(),
        }
    }
}

/// The track being downloaded.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TrackProgress {
    pub title: String,
    /// Position among the tracks to process (from 1), and how many there are.
    pub index: usize,
    pub total: usize,
    /// How much of the file is received.
    pub percent: u8,
    pub bytes_per_second: u64,
}

/// What the download is doing right now.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Progress {
    /// An item of the queue is being processed.
    pub running: bool,
    pub item_id: Option<String>,
    /// How far the item has got, from 0 to 100.
    pub percent: u8,
    pub album: Option<AlbumCard>,
    /// The tracks being downloaded, by position: an album downloads several at once.
    pub tracks: Vec<TrackProgress>,
    /// Why the queue stopped by itself, when it did: it goes on when asked to.
    pub halt: Option<Halt>,
}

/// Why the queue stopped by itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Halt {
    /// The disk is full, over its quota or read-only: going on would fail every item.
    StorageFull,
    /// Nobody is signed in to Qobuz: the queue goes on by itself once someone is.
    SignedOut,
    /// The user paused the queue.
    Paused,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn without_a_scope_the_percentage_is_unchanged() {
        assert_eq!(ProgressScope::FULL.map(0), 0);
        assert_eq!(ProgressScope::FULL.map(37), 37);
        assert_eq!(ProgressScope::FULL.map(100), 100);
    }

    #[test]
    fn the_third_album_of_four_starts_at_half_way() {
        let third = ProgressScope::FULL.slice(2, 4);

        assert_eq!(third.map(0), 50);
        assert_eq!(third.map(50), 62);
        assert_eq!(third.map(100), 75);
    }

    #[test]
    fn scopes_nest() {
        // The second of two artists, whose first of two albums is being downloaded
        let scope = ProgressScope::FULL.slice(1, 2).slice(0, 2);

        assert_eq!(scope.map(0), 50);
        assert_eq!(scope.map(100), 75);
    }

    #[test]
    fn an_empty_slice_changes_nothing() {
        assert_eq!(ProgressScope::FULL.slice(0, 0), ProgressScope::FULL);
    }

    #[test]
    fn a_percentage_beyond_a_hundred_stays_in_the_bar() {
        assert_eq!(ProgressScope::FULL.map(250), 100);
    }

    #[test]
    fn the_card_of_an_album_says_what_it_is() {
        let album: Album = serde_json::from_str(
            r#"{
                "id": "a", "title": "Kind of Blue", "version": "Mono",
                "artist": { "id": 1, "name": "Miles Davis" },
                "tracks_count": 5, "maximum_bit_depth": 24, "maximum_sampling_rate": 96.0,
                "genre": { "name": "Jazz" }, "label": { "id": 2, "name": "Columbia" },
                "release_date_original": "1959-08-17",
                "image": { "large": "https://static.example/c_600.jpg" }
            }"#,
        )
        .unwrap();

        let card = AlbumCard::from(&album);

        assert_eq!(card.title, "Kind of Blue (Mono)");
        assert_eq!(card.artist, "Miles Davis");
        assert_eq!(card.quality, "24bit / 96kHz");
        assert_eq!(card.release_date.as_deref(), Some("1959-08-17"));
        assert_eq!(
            (card.genre.as_str(), card.label.as_str()),
            ("Jazz", "Columbia")
        );
    }

    #[test]
    fn an_album_without_a_known_quality_has_an_empty_one() {
        let album: Album =
            serde_json::from_str(r#"{"id": "a", "title": "T", "artist": {"id": 1}}"#).unwrap();

        assert_eq!(AlbumCard::from(&album).quality, "");
    }
}
