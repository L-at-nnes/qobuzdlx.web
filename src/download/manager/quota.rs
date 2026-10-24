use crate::settings::RateLimit;

/// What has been fetched since the last long break, which is what tells when the next is due.
#[derive(Debug, Default)]
pub(super) struct Quota {
    tracks: u32,
    albums: u32,
    bytes: u64,
    /// The album of the last file written: an album counts once, at its first file.
    album: Option<String>,
}

impl Quota {
    /// A megabyte, as a disk maker counts it.
    const MEGABYTE: u64 = 1_000_000;

    /// A file was written.
    pub(super) fn saved(&mut self, album_id: &str, bytes: u64) {
        self.tracks = self.tracks.saturating_add(1);
        self.bytes = self.bytes.saturating_add(bytes);
        if self.album.as_deref() != Some(album_id) {
            self.albums = self.albums.saturating_add(1);
            self.album = Some(album_id.to_owned());
        }
    }

    /// Whether any of the conditions of the break is reached. A condition set to `0` is not used.
    pub(super) fn reached(&self, limits: &RateLimit) -> bool {
        let over = |count: u64, limit: u32| limit > 0 && count >= u64::from(limit);
        over(u64::from(self.tracks), limits.break_after_tracks)
            || over(u64::from(self.albums), limits.break_after_albums)
            || over(self.bytes / Self::MEGABYTE, limits.break_after_megabytes)
    }

    /// A break is taken: counting starts again.
    pub(super) fn reset(&mut self) {
        *self = Self::default();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn only(tracks: u32, albums: u32, megabytes: u32) -> RateLimit {
        RateLimit {
            break_after_tracks: tracks,
            break_after_albums: albums,
            break_after_megabytes: megabytes,
            ..RateLimit::off()
        }
    }

    #[test]
    fn nothing_is_due_when_no_condition_is_set() {
        let mut quota = Quota::default();
        for track in 0..500 {
            quota.saved(&format!("album{track}"), 500_000_000);
        }

        assert!(!quota.reached(&only(0, 0, 0)));
    }

    #[test]
    fn any_one_condition_is_enough() {
        let mut quota = Quota::default();
        quota.saved("a", 10_000_000);
        quota.saved("a", 10_000_000);
        quota.saved("b", 10_000_000);

        assert!(quota.reached(&only(3, 0, 0)), "three tracks");
        assert!(quota.reached(&only(0, 2, 0)), "two albums");
        assert!(quota.reached(&only(0, 0, 30)), "thirty megabytes");
        assert!(!quota.reached(&only(4, 3, 31)));
        assert!(quota.reached(&only(4, 3, 30)), "the last one decides");
    }

    #[test]
    fn an_album_counts_once_however_many_files_it_gives() {
        let mut quota = Quota::default();
        for _ in 0..12 {
            quota.saved("same", 1);
        }

        assert!(!quota.reached(&only(0, 2, 0)));
        assert!(quota.reached(&only(12, 0, 0)));
    }

    #[test]
    fn a_break_starts_the_count_again() {
        let mut quota = Quota::default();
        quota.saved("a", 5_000_000);
        quota.saved("b", 5_000_000);
        assert!(quota.reached(&only(2, 0, 0)));

        quota.reset();

        assert!(!quota.reached(&only(1, 1, 1)));
    }
}
