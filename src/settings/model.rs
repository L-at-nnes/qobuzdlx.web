//! The preferences a user can change from the interface.
//!
//! Credentials and the access password are deliberately not part of this file: it can be shared or
//! pasted into a bug report without leaking anything. Every group falls back to its defaults when
//! missing, so a file written by an older version keeps loading.

use std::path::PathBuf;
use std::time::Duration;

use rand::RngExt;

use serde::{Deserialize, Serialize};

/// Languages the interface is translated into.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Language {
    En,
    Fr,
}

/// Audio quality requested from Qobuz. Lower qualities are used when a release has no better.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Quality {
    /// MP3, 320 kbps.
    Mp3,
    /// FLAC, 16 bit / 44.1 kHz.
    Flac16,
    /// FLAC, 24 bit up to 96 kHz.
    Flac24,
    /// FLAC, 24 bit up to 192 kHz.
    Flac24Max,
}

impl Quality {
    /// The `format_id` Qobuz knows this quality by.
    #[must_use]
    pub const fn format_id(self) -> u32 {
        match self {
            Self::Mp3 => 5,
            Self::Flac16 => 6,
            Self::Flac24 => 7,
            Self::Flac24Max => 27,
        }
    }

    #[must_use]
    pub const fn is_flac(self) -> bool {
        !matches!(self, Self::Mp3)
    }

    /// File extension of the audio files, with its leading dot.
    #[must_use]
    pub const fn extension(self) -> &'static str {
        if self.is_flac() { ".flac" } else { ".mp3" }
    }
}

/// What to do when the file a track would be written to already exists.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DuplicateMode {
    /// Leave the existing file alone and count the track as skipped.
    Skip,
    /// Write the new file next to it under a numbered name.
    Rename,
    /// Replace the existing file.
    Overwrite,
}

/// How much the download log keeps. Each level contains the previous one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LogDetail {
    /// Failures and the summary of each item.
    Errors,
    /// What is going on: item, track, quality, skipped files.
    Normal,
    /// Normal, plus the quality, size and speed of every track and every failed attempt.
    Detailed,
}

/// Size of a cover: a width in pixels, or the original file the label supplied.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArtworkSize {
    Original,
    Pixels(u16),
}

impl ArtworkSize {
    /// Sizes Qobuz serves; anything else is refused when settings are updated.
    pub const PIXEL_RANGE: std::ops::RangeInclusive<u16> = 50..=600;

    #[must_use]
    pub fn is_valid(self) -> bool {
        match self {
            Self::Original => true,
            Self::Pixels(width) => Self::PIXEL_RANGE.contains(&width),
        }
    }
}

#[derive(Serialize, Deserialize)]
#[serde(untagged)]
enum ArtworkSizeRepr {
    Pixels(u16),
    Named(String),
}

impl Serialize for ArtworkSize {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::Original => ArtworkSizeRepr::Named("original".to_owned()),
            Self::Pixels(width) => ArtworkSizeRepr::Pixels(*width),
        }
        .serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for ArtworkSize {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        match ArtworkSizeRepr::deserialize(deserializer)? {
            ArtworkSizeRepr::Pixels(width) => Ok(Self::Pixels(width)),
            ArtworkSizeRepr::Named(name) if name.eq_ignore_ascii_case("original") => {
                Ok(Self::Original)
            }
            // Older files stored the width as text
            ArtworkSizeRepr::Named(name) => name
                .parse()
                .map(Self::Pixels)
                .map_err(|_| serde::de::Error::custom(format!("unknown artwork size {name:?}"))),
        }
    }
}

/// Where a marker added to a track title goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MarkerPosition {
    Prefix,
    Suffix,
}

/// A text appended to (or put before) the title of explicit or clean tracks, such as ` (Explicit)`.
/// The spaces are kept exactly as typed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct TitleMarker {
    pub enabled: bool,
    pub text: String,
    pub position: MarkerPosition,
}

impl TitleMarker {
    fn new(text: &str) -> Self {
        Self {
            enabled: false,
            text: text.to_owned(),
            position: MarkerPosition::Suffix,
        }
    }
}

impl Default for TitleMarker {
    fn default() -> Self {
        Self::new("")
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct DownloadSettings {
    /// Where the music goes. Empty until the user picks a folder: the runtime default applies then.
    pub directory: PathBuf,
    pub quality: Quality,
}

impl Default for DownloadSettings {
    fn default() -> Self {
        Self {
            directory: PathBuf::new(),
            quality: Quality::Flac24Max,
        }
    }
}

/// Templates for folder and file names, with `%Variable%` placeholders.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Naming {
    pub artist: String,
    pub album: String,
    pub track: String,
    /// Track file name inside a compilation ("Various Artists"), which needs the track artist.
    pub compilation_track: String,
    pub playlist: String,
    /// Sub-folder of each disc of a multi-disc release. Empty: no sub-folder.
    pub disc: String,
    /// Name the album artist "A, B & C" when a release has several main artists.
    pub merge_artists: bool,
    /// Same, in the artist folder name. Only applies when `merge_artists` is on.
    pub merge_artists_in_folders: bool,
}

impl Default for Naming {
    fn default() -> Self {
        Self {
            artist: "%ArtistName%".to_owned(),
            album: "%AlbumTitle%".to_owned(),
            track: "%TrackNumber% - %TrackTitle%".to_owned(),
            compilation_track: "%TrackNumber% - %TrackArtist% - %TrackTitle%".to_owned(),
            playlist: "%PlaylistTitle%".to_owned(),
            disc: "CD %DiscNumber%".to_owned(),
            merge_artists: false,
            merge_artists_in_folders: false,
        }
    }
}

/// Which tags are written to the audio files.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
#[allow(clippy::struct_excessive_bools)] // one switch per tag is the point of this group
pub struct Tags {
    pub album: bool,
    pub album_artist: bool,
    pub artist: bool,
    pub title: bool,
    pub track_number: bool,
    pub total_tracks: bool,
    pub disc_number: bool,
    pub total_discs: bool,
    pub genre: bool,
    pub year: bool,
    pub release_date: bool,
    pub composer: bool,
    pub copyright: bool,
    pub label: bool,
    pub isrc: bool,
    pub upc: bool,
    pub release_type: bool,
    pub explicit: bool,
    pub cover: bool,
    pub url: bool,
    pub comment: bool,
    /// Text of the comment. `%Description%` stands for the album description.
    pub comment_text: String,
    pub explicit_marker: TitleMarker,
    pub clean_marker: TitleMarker,
}

impl Default for Tags {
    fn default() -> Self {
        Self {
            album: true,
            album_artist: true,
            artist: true,
            title: true,
            track_number: true,
            total_tracks: true,
            disc_number: true,
            total_discs: true,
            genre: true,
            year: true,
            release_date: true,
            composer: true,
            copyright: true,
            label: true,
            isrc: true,
            upc: true,
            release_type: true,
            explicit: true,
            cover: true,
            url: true,
            comment: false,
            comment_text: String::new(),
            explicit_marker: TitleMarker::new(" (Explicit)"),
            clean_marker: TitleMarker::new(" (Clean)"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Artwork {
    /// Cover embedded in each audio file.
    pub embedded_size: ArtworkSize,
    /// Cover saved next to the audio files.
    pub saved_size: ArtworkSize,
    pub save_to_disk: bool,
}

impl Default for Artwork {
    fn default() -> Self {
        Self {
            embedded_size: ArtworkSize::Pixels(600),
            saved_size: ArtworkSize::Pixels(600),
            save_to_disk: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
#[allow(clippy::struct_excessive_bools)] // independent switches
pub struct Advanced {
    /// Skip tracks Qobuz marks as not streamable.
    pub streamable_check: bool,
    /// Decode every downloaded FLAC to check it, and download it again when it is damaged.
    pub verify_integrity: bool,
    /// Write the checksum of FLAC files that carry none.
    pub fix_md5: bool,
    /// Download booklets and other bonus material of an album.
    pub download_goodies: bool,
    /// Number the tracks of a playlist by their position in it instead of in their album.
    pub playlist_positions: bool,
    /// Write an `.m3u8` file next to the tracks of a playlist.
    pub playlist_file: bool,
    /// How many tracks of an album are downloaded at the same time. Qobuz serves a file at a modest
    /// speed per connection: several at once is what makes a large library finish in days, not weeks.
    pub parallel_tracks: u8,
    pub log_detail: LogDetail,
    pub duplicates: DuplicateMode,
}

impl Advanced {
    /// What `parallel_tracks` may be set to.
    pub const PARALLEL_TRACKS: std::ops::RangeInclusive<u8> = 1..=6;

    /// `parallel_tracks` held to its range: a file edited by hand cannot ask for a hundred.
    #[must_use]
    pub fn parallel(&self) -> usize {
        usize::from(
            self.parallel_tracks
                .clamp(*Self::PARALLEL_TRACKS.start(), *Self::PARALLEL_TRACKS.end()),
        )
    }
}

impl Default for Advanced {
    fn default() -> Self {
        Self {
            streamable_check: true,
            verify_integrity: true,
            fix_md5: false,
            download_goodies: true,
            playlist_positions: false,
            playlist_file: true,
            parallel_tracks: 3,
            log_detail: LogDetail::Normal,
            duplicates: DuplicateMode::Skip,
        }
    }
}

/// A wait drawn at random, each time, between two bounds in milliseconds. A fixed pause is a
/// pattern a server can recognise; a drawn one is not.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Delay {
    pub min_ms: u32,
    pub max_ms: u32,
}

impl Delay {
    /// The longest wait that is accepted: one hour.
    pub const LIMIT_MS: u32 = 3_600_000;

    #[must_use]
    pub const fn new(min_ms: u32, max_ms: u32) -> Self {
        Self { min_ms, max_ms }
    }

    /// No wait at all.
    pub const NONE: Self = Self::new(0, 0);

    /// Whether the bounds are in order and not beyond the limit.
    #[must_use]
    pub const fn is_valid(self) -> bool {
        self.min_ms <= self.max_ms && self.max_ms <= Self::LIMIT_MS
    }

    /// The bounds as a file edited by hand may have left them: in order and within the limit.
    fn bounds(self) -> (u32, u32) {
        let max = self.max_ms.min(Self::LIMIT_MS);
        (self.min_ms.min(max), max)
    }

    /// A wait between the bounds, drawn now.
    #[must_use]
    pub fn pick(self) -> Duration {
        let (min, max) = self.bounds();
        Duration::from_millis(u64::from(rand::rng().random_range(min..=max)))
    }

    /// The shortest wait the bounds allow.
    #[must_use]
    pub fn shortest(self) -> Duration {
        Duration::from_millis(u64::from(self.bounds().0))
    }
}

impl Default for Delay {
    fn default() -> Self {
        Self::NONE
    }
}

/// How gently Qobuz is asked for things. Qobuz publishes no limit: these waits keep a large queue
/// from looking like a machine going as fast as it can.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct RateLimit {
    /// Before each track that has to be asked for. A file already in place costs no wait.
    pub between_tracks: Delay,
    /// Between two albums, and between two items of the queue.
    pub between_albums: Delay,
    /// A long break is taken as soon as any of these is reached since the last one. `0`: that
    /// condition is not used.
    pub break_after_tracks: u32,
    pub break_after_albums: u32,
    /// Megabytes written, a megabyte being a million bytes.
    pub break_after_megabytes: u32,
    pub break_length: Delay,
    /// First wait after Qobuz answered "too many requests"; it doubles each time it happens again
    /// without an item going well in between.
    pub cooldown_ms: u32,
}

impl RateLimit {
    /// The most tracks, albums and megabytes that a break condition may be set to.
    pub const BREAK_TRACKS_LIMIT: u32 = 100_000;
    pub const BREAK_ALBUMS_LIMIT: u32 = 10_000;
    pub const BREAK_MEGABYTES_LIMIT: u32 = 10_000_000;
    /// The cooldown can double up to this many times its first value.
    pub const COOLDOWN_DOUBLINGS: u32 = 6;

    /// No wait anywhere but after Qobuz asked for one, for programs that talk to a server of their
    /// own.
    #[must_use]
    pub const fn off() -> Self {
        Self {
            between_tracks: Delay::NONE,
            between_albums: Delay::NONE,
            break_after_tracks: 0,
            break_after_albums: 0,
            break_after_megabytes: 0,
            break_length: Delay::NONE,
            cooldown_ms: 1000,
        }
    }

    #[must_use]
    pub const fn is_valid(&self) -> bool {
        self.between_tracks.is_valid()
            && self.between_albums.is_valid()
            && self.break_length.is_valid()
            && self.break_after_tracks <= Self::BREAK_TRACKS_LIMIT
            && self.break_after_albums <= Self::BREAK_ALBUMS_LIMIT
            && self.break_after_megabytes <= Self::BREAK_MEGABYTES_LIMIT
            && self.cooldown_ms <= Delay::LIMIT_MS
    }

    /// How long to wait after Qobuz said "too many requests" for the `strikes`-th time in a row
    /// (from 1).
    #[must_use]
    pub fn cooldown(&self, strikes: u32) -> Duration {
        let doublings = strikes.saturating_sub(1).min(Self::COOLDOWN_DOUBLINGS);
        let wait = u64::from(self.cooldown_ms) << doublings;
        Duration::from_millis(wait.min(u64::from(Delay::LIMIT_MS)))
    }
}

impl Default for RateLimit {
    fn default() -> Self {
        Self {
            between_tracks: Delay::new(500, 1500),
            between_albums: Delay::new(3000, 8000),
            break_after_tracks: 150,
            break_after_albums: 10,
            break_after_megabytes: 3000,
            break_length: Delay::new(120_000, 300_000),
            cooldown_ms: 60_000,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// `None` until chosen: the interface then follows the browser's language.
    pub language: Option<Language>,
    pub download: DownloadSettings,
    pub naming: Naming,
    pub tags: Tags,
    pub artwork: Artwork,
    pub advanced: Advanced,
    pub rate_limit: RateLimit,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tracks_at_once_are_held_to_their_range() {
        let mut advanced = Advanced::default();
        assert_eq!(advanced.parallel(), 3);

        advanced.parallel_tracks = 0;
        assert_eq!(advanced.parallel(), 1);
        advanced.parallel_tracks = 200;
        assert_eq!(advanced.parallel(), 6);
    }

    #[test]
    fn a_delay_is_drawn_between_its_bounds() {
        let delay = Delay::new(100, 200);

        for _ in 0..200 {
            let wait = delay.pick();
            assert!(wait >= Duration::from_millis(100) && wait <= Duration::from_millis(200));
        }
        assert_eq!(delay.shortest(), Duration::from_millis(100));
        assert_eq!(Delay::NONE.pick(), Duration::ZERO);
    }

    #[test]
    fn a_delay_edited_by_hand_cannot_panic_or_run_for_days() {
        let reversed = Delay::new(500, 100);
        let huge = Delay::new(0, u32::MAX);

        assert!(!reversed.is_valid() && !huge.is_valid());
        assert_eq!(reversed.pick(), Duration::from_millis(100));
        assert!(huge.pick() <= Duration::from_millis(u64::from(Delay::LIMIT_MS)));
    }

    #[test]
    fn the_cooldown_doubles_up_to_a_bound() {
        let limits = RateLimit {
            cooldown_ms: 1000,
            ..RateLimit::off()
        };

        assert_eq!(limits.cooldown(1), Duration::from_secs(1));
        assert_eq!(limits.cooldown(2), Duration::from_secs(2));
        assert_eq!(limits.cooldown(4), Duration::from_secs(8));
        assert_eq!(limits.cooldown(500), Duration::from_secs(64));
        let long = RateLimit {
            cooldown_ms: Delay::LIMIT_MS,
            ..RateLimit::off()
        };
        assert_eq!(
            long.cooldown(7),
            Duration::from_millis(u64::from(Delay::LIMIT_MS))
        );
    }

    #[test]
    fn the_defaults_are_valid_and_a_file_without_the_group_gets_them() {
        assert!(RateLimit::default().is_valid());
        assert!(RateLimit::off().is_valid());
        let settings: Settings = serde_json::from_str(r#"{"advanced":{}}"#).unwrap();
        assert_eq!(settings.rate_limit, RateLimit::default());
    }

    #[test]
    fn a_group_missing_a_field_keeps_the_default_of_that_field() {
        let settings: Settings =
            serde_json::from_str(r#"{"rate_limit":{"break_after_albums":50}}"#).unwrap();

        assert_eq!(settings.rate_limit.break_after_albums, 50);
        assert_eq!(
            settings.rate_limit.between_albums,
            RateLimit::default().between_albums
        );
    }

    #[test]
    fn the_conditions_of_a_break_are_held_to_their_limits() {
        let mut limits = RateLimit::default();
        assert!(limits.break_after_tracks > 0 && limits.break_after_albums > 0);
        assert!(limits.break_after_megabytes > 0);

        limits.break_after_tracks = RateLimit::BREAK_TRACKS_LIMIT + 1;
        assert!(!limits.is_valid());
        limits.break_after_tracks = 0;
        limits.break_after_albums = RateLimit::BREAK_ALBUMS_LIMIT + 1;
        assert!(!limits.is_valid());
        limits.break_after_albums = 0;
        limits.break_after_megabytes = RateLimit::BREAK_MEGABYTES_LIMIT + 1;
        assert!(!limits.is_valid());
        limits.break_after_megabytes = RateLimit::BREAK_MEGABYTES_LIMIT;
        assert!(limits.is_valid());
    }

    #[test]
    fn an_empty_file_gives_the_defaults() {
        let settings: Settings = serde_json::from_str("{}").unwrap();

        assert_eq!(settings, Settings::default());
    }

    #[test]
    fn a_partial_file_keeps_the_defaults_of_what_it_leaves_out() {
        let settings: Settings = serde_json::from_str(
            r#"{ "naming": { "album": "%Year% - %AlbumTitle%" }, "download": { "quality": "flac16" } }"#,
        )
        .unwrap();

        assert_eq!(settings.naming.album, "%Year% - %AlbumTitle%");
        assert_eq!(settings.naming.track, Naming::default().track);
        assert_eq!(settings.download.quality, Quality::Flac16);
        assert_eq!(
            settings.download.directory,
            DownloadSettings::default().directory
        );
    }

    #[test]
    fn unknown_keys_are_ignored() {
        let settings: Settings = serde_json::from_str(
            r#"{ "theme": "dark", "tags": { "retired": true, "genre": false } }"#,
        )
        .unwrap();

        assert!(!settings.tags.genre);
        assert!(settings.tags.album);
    }

    #[test]
    fn settings_survive_a_round_trip() {
        let settings = Settings {
            language: Some(Language::Fr),
            advanced: Advanced {
                duplicates: DuplicateMode::Rename,
                ..Advanced::default()
            },
            artwork: Artwork {
                embedded_size: ArtworkSize::Original,
                ..Artwork::default()
            },
            tags: Tags {
                explicit_marker: TitleMarker {
                    enabled: true,
                    ..TitleMarker::default()
                },
                ..Tags::default()
            },
            ..Settings::default()
        };

        let json = serde_json::to_string(&settings).unwrap();

        assert_eq!(serde_json::from_str::<Settings>(&json).unwrap(), settings);
    }

    #[test]
    fn enums_are_written_as_readable_words() {
        let json = serde_json::to_value(Settings::default()).unwrap();

        assert_eq!(json["download"]["quality"], "flac24_max");
        assert_eq!(json["advanced"]["duplicates"], "skip");
        assert_eq!(json["advanced"]["log_detail"], "normal");
        assert_eq!(json["tags"]["explicit_marker"]["position"], "suffix");
    }

    #[test]
    fn qualities_map_to_the_format_ids_and_extensions_of_qobuz() {
        let expected = [
            (Quality::Mp3, 5, ".mp3"),
            (Quality::Flac16, 6, ".flac"),
            (Quality::Flac24, 7, ".flac"),
            (Quality::Flac24Max, 27, ".flac"),
        ];

        for (quality, format_id, extension) in expected {
            assert_eq!(quality.format_id(), format_id);
            assert_eq!(quality.extension(), extension);
        }
    }

    #[test]
    fn log_levels_are_ordered_by_how_much_they_keep() {
        assert!(LogDetail::Errors < LogDetail::Normal);
        assert!(LogDetail::Normal < LogDetail::Detailed);
    }

    #[test]
    fn an_artwork_size_is_a_number_or_the_word_original() {
        let sizes: Vec<ArtworkSize> =
            serde_json::from_str(r#"[600, "original", "300", "Original"]"#).unwrap();

        assert_eq!(
            sizes,
            [
                ArtworkSize::Pixels(600),
                ArtworkSize::Original,
                ArtworkSize::Pixels(300),
                ArtworkSize::Original
            ]
        );
        assert_eq!(
            serde_json::to_string(&ArtworkSize::Pixels(300)).unwrap(),
            "300"
        );
        assert_eq!(
            serde_json::to_string(&ArtworkSize::Original).unwrap(),
            "\"original\""
        );
    }

    #[test]
    fn an_unknown_artwork_size_is_rejected() {
        assert!(serde_json::from_str::<ArtworkSize>(r#""huge""#).is_err());
    }

    #[test]
    fn only_sizes_qobuz_serves_are_valid() {
        assert!(ArtworkSize::Original.is_valid());
        assert!(ArtworkSize::Pixels(50).is_valid());
        assert!(ArtworkSize::Pixels(600).is_valid());
        assert!(!ArtworkSize::Pixels(49).is_valid());
        assert!(!ArtworkSize::Pixels(601).is_valid());
    }
}
