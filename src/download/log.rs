use std::collections::VecDeque;
use std::fmt;

use serde::Serialize;
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;

use crate::settings::LogDetail;

/// The log keeps this many lines; when it is full the oldest ones go, a slice at a time (removing
/// a few hundred lines every few hundred lines costs far less than shifting the whole log for each
/// new one).
const MAX_LINES: usize = 1200;
const TRIM_LINES: usize = 400;

/// Something that happened during a download.
///
/// The log is made of events, not of sentences: the interface writes them in its own language, and
/// the English wording of [`fmt::Display`] serves the exported file and the server logs.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum LogEvent {
    /// The start of an item: the log separates items like this.
    ItemStarted {
        label: String,
    },
    CheckingLink,
    InvalidUrl {
        url: String,
    },
    DownloadingTrack {
        number: u32,
        title: String,
    },
    FileExists {
        number: u32,
    },
    QualityUnavailable {
        quality: String,
    },
    TrackNotInQuality {
        number: u32,
        quality: String,
    },
    AttemptFailed {
        attempt: u32,
        attempts: u32,
        quality: String,
        error: String,
    },
    QualityFailed {
        quality: String,
        attempts: u32,
        error: String,
    },
    TrackTransfer {
        quality: String,
        megabytes: f64,
        megabytes_per_second: f64,
    },
    Done,
    ItemSummary {
        downloaded: u32,
        skipped: u32,
        failed: u32,
    },
    Complete,
    Aborted,
    QueueDrained,
    QueueRestored {
        waiting: usize,
    },
    Error {
        message: String,
    },
    SessionExpired,
    AlbumSkipped,
    AlbumNotStreamable,
    TrackNotStreamable {
        number: u32,
    },
    TrackUnavailable {
        number: u32,
    },
    TrackFailed {
        track_id: u64,
        error: String,
    },
    AlbumFailed {
        album_id: String,
        error: String,
    },
    ArtistFailed {
        artist_id: u64,
        error: String,
    },
    PreviewOnly,
    FavoritesEmpty,
    LabelNotFound,
    TrackNotFound,
    AlbumInfoUnavailable,
    PlaylistFileWritten {
        name: String,
    },
    PlaylistFileFailed {
        error: String,
    },
    GoodyFound,
    GoodyExists,
    GoodyFailed {
        error: String,
    },
    Md5FixFailed {
        error: String,
    },
    VerifyFailed {
        error: String,
    },
}

impl LogEvent {
    /// The least detailed level that shows the event.
    #[must_use]
    pub const fn level(&self) -> LogDetail {
        match self {
            Self::InvalidUrl { .. }
            | Self::QualityFailed { .. }
            | Self::ItemSummary { .. }
            | Self::Error { .. }
            | Self::SessionExpired
            | Self::TrackFailed { .. }
            | Self::AlbumFailed { .. }
            | Self::ArtistFailed { .. }
            | Self::PreviewOnly
            | Self::TrackNotFound
            | Self::AlbumInfoUnavailable
            | Self::PlaylistFileFailed { .. }
            | Self::GoodyFailed { .. }
            | Self::Md5FixFailed { .. }
            | Self::VerifyFailed { .. } => LogDetail::Errors,
            Self::AttemptFailed { .. } | Self::TrackTransfer { .. } => LogDetail::Detailed,
            _ => LogDetail::Normal,
        }
    }
}

impl fmt::Display for LogEvent {
    #[allow(clippy::too_many_lines)] // one arm per event
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ItemStarted { label } => write!(formatter, "=== {label} ==="),
            Self::CheckingLink => write!(formatter, "Checking link..."),
            Self::InvalidUrl { url } => write!(formatter, "Not a Qobuz address: {url}"),
            Self::DownloadingTrack { number, title } => {
                write!(formatter, "Downloading - {number:02} {title}...")
            }
            Self::FileExists { number } => write!(
                formatter,
                "File for track {number} already exists, skipping."
            ),
            Self::QualityUnavailable { quality } => {
                write!(formatter, "Quality not available, trying {quality}")
            }
            Self::TrackNotInQuality { number, quality } => {
                write!(formatter, "Track {number} is not available in {quality}")
            }
            Self::AttemptFailed {
                attempt,
                attempts,
                quality,
                error,
            } => {
                write!(
                    formatter,
                    "Attempt {attempt}/{attempts} in {quality} failed: {error}"
                )
            }
            Self::QualityFailed {
                quality,
                attempts,
                error,
            } => {
                write!(
                    formatter,
                    "Quality {quality} failed after {attempts} attempts: {error}"
                )
            }
            Self::TrackTransfer {
                quality,
                megabytes,
                megabytes_per_second,
            } => {
                write!(
                    formatter,
                    "Quality obtained: {quality} - {megabytes:.1} MB at {megabytes_per_second:.1} MB/s"
                )
            }
            Self::Done => write!(formatter, " DONE"),
            Self::ItemSummary {
                downloaded,
                skipped,
                failed,
            } => {
                write!(
                    formatter,
                    "Summary - downloaded: {downloaded}, skipped: {skipped}, failed: {failed}"
                )
            }
            Self::Complete => write!(formatter, "DOWNLOAD COMPLETE"),
            Self::Aborted => write!(formatter, "Download cancelled."),
            Self::QueueDrained => write!(formatter, "Queue finished."),
            Self::QueueRestored { waiting } => {
                write!(
                    formatter,
                    "Queue restored after a restart: {waiting} item(s) waiting."
                )
            }
            Self::Error { message } => write!(formatter, "ERROR: {message}"),
            Self::SessionExpired => write!(formatter, "The Qobuz session expired: sign in again."),
            Self::AlbumSkipped => write!(formatter, "Album skipped."),
            Self::AlbumNotStreamable => write!(formatter, "This album is not streamable."),
            Self::TrackNotStreamable { number } => {
                write!(formatter, "Track {number} is not streamable.")
            }
            Self::TrackUnavailable { number } => {
                write!(formatter, "Track {number} is unavailable.")
            }
            Self::TrackFailed { track_id, error } => {
                write!(formatter, "Track {track_id} failed: {error}")
            }
            Self::AlbumFailed { album_id, error } => {
                write!(formatter, "Album {album_id} failed: {error}")
            }
            Self::ArtistFailed { artist_id, error } => {
                write!(formatter, "Artist {artist_id} failed: {error}")
            }
            Self::PreviewOnly => write!(
                formatter,
                "Qobuz only gave the 30 second preview of this track: it is skipped (is the subscription active?)."
            ),
            Self::FavoritesEmpty => write!(formatter, "No favorites to download."),
            Self::LabelNotFound => write!(formatter, "Label not found."),
            Self::TrackNotFound => write!(formatter, "Track not found."),
            Self::AlbumInfoUnavailable => {
                write!(formatter, "The album of this track could not be found.")
            }
            Self::PlaylistFileWritten { name } => {
                write!(formatter, "Playlist file written: {name}")
            }
            Self::PlaylistFileFailed { error } => {
                write!(formatter, "Could not write the playlist file: {error}")
            }
            Self::GoodyFound => write!(formatter, "Bonus found, downloading..."),
            Self::GoodyExists => write!(formatter, "Bonus already downloaded."),
            Self::GoodyFailed { error } => write!(formatter, "Bonus failed: {error}"),
            Self::Md5FixFailed { error } => {
                write!(formatter, "Could not fix the MD5 checksum: {error}")
            }
            Self::VerifyFailed { error } => {
                write!(formatter, "The downloaded file is damaged: {error}")
            }
        }
    }
}

/// A line of the log. `seq` only grows, so a reader knows which lines it has already seen.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct LogLine {
    pub seq: u64,
    pub level: LogDetail,
    #[serde(flatten)]
    pub event: LogEvent,
}

/// The download log, bounded in size.
#[derive(Debug, Default)]
pub struct Log {
    lines: VecDeque<LogLine>,
    next_seq: u64,
}

impl Log {
    /// Records the event when `detail` asks for that much. The filter is applied when writing and not
    /// when reading: the log is bounded, and drowning it in detail nobody asked for would push the
    /// useful lines out.
    pub fn push(&mut self, event: LogEvent, detail: LogDetail) -> Option<&LogLine> {
        let level = event.level();
        if level > detail {
            return None;
        }

        self.lines.push_back(LogLine {
            seq: self.next_seq,
            level,
            event,
        });
        self.next_seq += 1;
        if self.lines.len() > MAX_LINES {
            self.lines.drain(..TRIM_LINES);
        }
        self.lines.back()
    }

    #[must_use]
    pub fn lines(&self) -> Vec<LogLine> {
        self.lines.iter().cloned().collect()
    }

    pub fn clear(&mut self) {
        self.lines.clear();
    }

    /// The log as a text file: a header with the version, the date and the level, then every line.
    #[must_use]
    pub fn export(&self, version: &str, detail: LogDetail) -> String {
        let date = OffsetDateTime::now_utc()
            .format(&Rfc3339)
            .unwrap_or_default();
        let mut text = format!("# QobuzDLX {version} - {date} - log detail: {detail:?}\n\n");
        for line in &self.lines {
            text.push_str(&line.event.to_string());
            text.push('\n');
        }
        text
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use LogDetail::{Detailed, Errors, Normal};

    fn failure() -> LogEvent {
        LogEvent::TrackFailed {
            track_id: 7,
            error: "restricted".into(),
        }
    }

    fn narration() -> LogEvent {
        LogEvent::DownloadingTrack {
            number: 1,
            title: "Song".into(),
        }
    }

    fn detail() -> LogEvent {
        LogEvent::TrackTransfer {
            quality: "FLAC 16bit/44.1kHz".into(),
            megabytes: 17.2,
            megabytes_per_second: 3.8,
        }
    }

    fn texts(log: &Log) -> Vec<String> {
        log.lines()
            .iter()
            .map(|line| line.event.to_string())
            .collect()
    }

    #[test]
    fn each_level_keeps_what_the_previous_ones_keep() {
        let mut log = Log::default();
        for level in [LogDetail::Errors, LogDetail::Normal, LogDetail::Detailed] {
            for event in [failure(), narration(), detail()] {
                log.push(event, level);
            }
        }

        let kept = log
            .lines()
            .iter()
            .map(|line| line.level)
            .collect::<Vec<_>>();
        // errors: failure | normal: failure, narration | detailed: all three
        assert_eq!(kept, [Errors, Errors, Normal, Errors, Normal, Detailed]);
    }

    #[test]
    fn the_summary_of_an_item_survives_the_quietest_level() {
        let mut log = Log::default();

        log.push(
            LogEvent::ItemSummary {
                downloaded: 1,
                skipped: 2,
                failed: 3,
            },
            LogDetail::Errors,
        );

        assert_eq!(
            texts(&log),
            ["Summary - downloaded: 1, skipped: 2, failed: 3"]
        );
    }

    #[test]
    fn a_filtered_event_leaves_no_trace_and_no_gap_in_the_numbering() {
        let mut log = Log::default();

        assert!(log.push(narration(), LogDetail::Errors).is_none());
        log.push(failure(), LogDetail::Errors);
        log.push(failure(), LogDetail::Errors);

        let numbers: Vec<_> = log.lines().iter().map(|line| line.seq).collect();
        assert_eq!(numbers, [0, 1]);
    }

    #[test]
    fn a_full_log_drops_its_oldest_lines_a_slice_at_a_time() {
        let mut log = Log::default();

        for number in 0..=MAX_LINES {
            log.push(
                LogEvent::FileExists {
                    number: u32::try_from(number).unwrap(),
                },
                LogDetail::Normal,
            );
        }

        let lines = log.lines();
        assert_eq!(lines.len(), MAX_LINES + 1 - TRIM_LINES);
        assert_eq!(lines[0].seq, TRIM_LINES as u64);
        assert_eq!(lines.last().unwrap().seq, MAX_LINES as u64);
    }

    #[test]
    fn clearing_the_log_does_not_reset_the_numbering() {
        let mut log = Log::default();
        log.push(failure(), LogDetail::Normal);

        log.clear();
        log.push(failure(), LogDetail::Normal);

        assert_eq!(log.lines()[0].seq, 1);
    }

    #[test]
    fn the_export_starts_with_the_version_and_the_level_then_holds_every_line() {
        let mut log = Log::default();
        log.push(LogEvent::CheckingLink, LogDetail::Normal);
        log.push(failure(), LogDetail::Normal);

        let export = log.export("2.0.0", LogDetail::Normal);

        let mut lines = export.lines();
        let header = lines.next().unwrap();
        assert!(header.starts_with("# QobuzDLX 2.0.0 - "), "{header}");
        assert!(header.ends_with("log detail: Normal"), "{header}");
        assert_eq!(
            lines.collect::<Vec<_>>(),
            ["", "Checking link...", "Track 7 failed: restricted"]
        );
        assert!(export.ends_with('\n'));
    }

    #[test]
    fn events_serialise_with_their_name_and_their_fields_for_the_interface() {
        let line = LogLine {
            seq: 4,
            level: LogDetail::Normal,
            event: narration(),
        };

        let json = serde_json::to_value(&line).unwrap();

        assert_eq!(json["seq"], 4);
        assert_eq!(json["level"], "normal");
        assert_eq!(json["event"], "downloading_track");
        assert_eq!(json["number"], 1);
        assert_eq!(json["title"], "Song");
    }

    #[test]
    fn the_english_wording_pads_the_track_number() {
        assert_eq!(narration().to_string(), "Downloading - 01 Song...");
        assert_eq!(
            detail().to_string(),
            "Quality obtained: FLAC 16bit/44.1kHz - 17.2 MB at 3.8 MB/s"
        );
    }
}
