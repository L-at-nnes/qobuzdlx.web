use serde::Serialize;

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
    pub track: Option<TrackProgress>,
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
}
