//! Telling an account that went back to free from the tracks Qobuz will not give.
//!
//! A free account still signs in, and still lists everything: it is only refused the files, with a
//! 30 second preview or an error. One such track says little (some tracks are only ever offered as a
//! preview), but a series of them with not a single file written in between says the account can no
//! longer download. Going on would ask Qobuz for every track of every album of the queue, for
//! nothing: the queue stops instead, and gives its place back to the item it was on.

use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

/// How many tracks in a row may be refused before the queue stops. It is also the default number of
/// tracks downloaded at once: the first album of a lapsed account stops after one round of requests.
pub const LIMIT: u32 = 3;

/// The tracks refused since the last one that was written, shared by everything that downloads.
#[derive(Debug, Clone, Default)]
pub struct Refusals(Arc<AtomicU32>);

impl Refusals {
    /// A track was refused. Returns whether the series is long enough to stop the queue.
    #[must_use]
    pub fn refused(&self) -> bool {
        self.0.fetch_add(1, Ordering::SeqCst) + 1 >= LIMIT
    }

    /// A file was written: the account can download, the series starts again.
    pub fn served(&self) {
        self.0.store(0, Ordering::SeqCst);
    }

    /// Forgets the series, when the queue is told to go on or the account changes.
    pub fn reset(&self) {
        self.served();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_queue_stops_at_the_third_refusal_in_a_row() {
        let refusals = Refusals::default();

        assert!(!refusals.refused());
        assert!(!refusals.refused());
        assert!(refusals.refused());
        assert!(
            refusals.refused(),
            "and stays stopped while it is not reset"
        );
    }

    #[test]
    fn a_file_written_starts_the_series_again() {
        let refusals = Refusals::default();

        assert!(!refusals.refused());
        assert!(!refusals.refused());
        refusals.served();

        assert!(!refusals.refused());
        assert!(!refusals.refused());
        assert!(refusals.refused());
    }

    #[test]
    fn clones_count_together() {
        let refusals = Refusals::default();
        let other = refusals.clone();

        assert!(!refusals.refused());
        assert!(!other.refused());
        assert!(refusals.refused());
        other.reset();
        assert!(!refusals.refused());
    }
}
