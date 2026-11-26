//! What was downloaded in the last 24 hours, which is what the daily limit is measured against.
//!
//! It is kept on disk: a restart must not give back a day's worth of downloads.

use std::collections::VecDeque;
use std::path::Path;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::storage::{Access, JsonFile};

/// The length of the window the limit is counted over, in seconds.
const WINDOW: u64 = 86_400;

/// Files written within the same minute are one entry: the file stays small however many tracks.
const GRAIN: u64 = 60;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
struct Entry {
    /// When, in Unix seconds.
    at: u64,
    bytes: u64,
}

#[derive(Debug)]
pub(super) struct Budget {
    file: JsonFile,
    entries: VecDeque<Entry>,
}

impl Budget {
    pub(super) fn load(config_dir: &Path) -> Self {
        let file = JsonFile::new(config_dir.join("usage.json"), Access::Normal);
        let entries = file.load::<VecDeque<Entry>>().unwrap_or_default();
        Self { file, entries }
    }

    /// A file of `bytes` was written at `now`.
    pub(super) fn record(&mut self, now: u64, bytes: u64) {
        self.forget_before(now);
        match self.entries.back_mut() {
            Some(last) if last.at / GRAIN == now / GRAIN => {
                last.bytes = last.bytes.saturating_add(bytes);
            }
            _ => self.entries.push_back(Entry { at: now, bytes }),
        }
        if let Err(error) = self.file.save_compact(&self.entries) {
            tracing::warn!(%error, "the daily usage could not be saved");
        }
    }

    /// What was written in the 24 hours before `now`.
    pub(super) fn used(&self, now: u64) -> u64 {
        self.entries
            .iter()
            .filter(|entry| entry.at + WINDOW > now)
            .fold(0, |total, entry| total.saturating_add(entry.bytes))
    }

    /// How long until what was written in the last 24 hours comes under `limit`, `None` when it is
    /// already under it, or when there is no limit (`0`).
    pub(super) fn wait(&self, now: u64, limit: u64) -> Option<Duration> {
        if limit == 0 {
            return None;
        }
        let mut used = self.used(now);
        if used < limit {
            return None;
        }
        // The oldest entries leave the window first: the wait is that of the one whose leaving
        // makes room
        for entry in self.entries.iter().filter(|entry| entry.at + WINDOW > now) {
            used = used.saturating_sub(entry.bytes);
            if used < limit {
                return Some(Duration::from_secs((entry.at + WINDOW - now).max(1)));
            }
        }
        None
    }

    fn forget_before(&mut self, now: u64) {
        while self
            .entries
            .front()
            .is_some_and(|entry| entry.at + WINDOW <= now)
        {
            self.entries.pop_front();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    const GB: u64 = 1_000_000_000;

    #[test]
    fn what_was_written_counts_for_a_day() {
        let directory = TempDir::new().unwrap();
        let mut budget = Budget::load(directory.path());

        budget.record(1_000, 3 * GB);
        budget.record(5_000, 2 * GB);

        assert_eq!(budget.used(6_000), 5 * GB);
        assert_eq!(budget.used(1_000 + WINDOW), 2 * GB, "the first has left");
        assert_eq!(budget.used(5_000 + WINDOW), 0);
    }

    #[test]
    fn under_the_limit_there_is_nothing_to_wait_for() {
        let directory = TempDir::new().unwrap();
        let mut budget = Budget::load(directory.path());
        budget.record(1_000, 49 * GB);

        assert_eq!(budget.wait(2_000, 50 * GB), None);
    }

    #[test]
    fn no_limit_means_no_wait_whatever_was_written() {
        let directory = TempDir::new().unwrap();
        let mut budget = Budget::load(directory.path());
        budget.record(1_000, 500 * GB);

        assert_eq!(budget.wait(2_000, 0), None);
    }

    #[test]
    fn over_the_limit_the_wait_is_until_enough_has_left_the_window() {
        let directory = TempDir::new().unwrap();
        let mut budget = Budget::load(directory.path());
        // 30 GB at t = 1 h, 15 GB at t = 5 h, 10 GB at t = 9 h: 55 GB in all
        budget.record(3_600, 30 * GB);
        budget.record(18_000, 15 * GB);
        budget.record(32_400, 10 * GB);
        let now = 40_000;

        // The first 30 GB leaving brings it to 25 GB: that is enough, and it leaves at 1 h + 24 h
        assert_eq!(
            budget.wait(now, 50 * GB),
            Some(Duration::from_secs(3_600 + WINDOW - now))
        );
        // Under a limit of 20 GB, the second must leave as well
        assert_eq!(
            budget.wait(now, 20 * GB),
            Some(Duration::from_secs(18_000 + WINDOW - now))
        );
    }

    #[test]
    fn a_wait_lasts_a_second_at_least() {
        let directory = TempDir::new().unwrap();
        let mut budget = Budget::load(directory.path());
        budget.record(100, 60 * GB);

        assert_eq!(
            budget.wait(100 + WINDOW - 1, 50 * GB),
            Some(Duration::from_secs(1))
        );
    }

    #[test]
    fn files_of_the_same_minute_make_one_entry() {
        let directory = TempDir::new().unwrap();
        let mut budget = Budget::load(directory.path());

        for second in 0..50 {
            budget.record(6_000 + second, GB);
        }

        assert_eq!(budget.entries.len(), 1);
        assert_eq!(budget.used(6_100), 50 * GB);
    }

    #[test]
    fn what_was_written_survives_a_restart() {
        let directory = TempDir::new().unwrap();
        Budget::load(directory.path()).record(1_000, 7 * GB);

        let restarted = Budget::load(directory.path());

        assert_eq!(restarted.used(2_000), 7 * GB);
    }

    #[test]
    fn what_is_older_than_a_day_is_forgotten_at_the_next_write() {
        let directory = TempDir::new().unwrap();
        let mut budget = Budget::load(directory.path());
        budget.record(1_000, 7 * GB);

        budget.record(1_000 + WINDOW + 10, GB);

        assert_eq!(budget.entries.len(), 1);
    }
}
