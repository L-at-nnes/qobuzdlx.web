mod common;

use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use common::{FakeServer, Recorder, Reply, flac_bytes, signed_in};
use lofty::file::TaggedFileExt;
use lofty::prelude::*;
use lofty::probe::Probe;
use qobuzdlx::download::{
    Context, Environment, Fetcher, LogEvent, TrackError, TrackRequest, TrackResult, Tuning,
};
use qobuzdlx::naming::DownloadKind;
use qobuzdlx::qobuz::{Album, Client, Track};
use qobuzdlx::settings::{DuplicateMode, Quality, Settings};
use serde_json::json;
use tempfile::TempDir;
use tokio_util::sync::CancellationToken;

struct Rig {
    server: FakeServer,
    client: Client,
    music: TempDir,
    partial: TempDir,
    recorder: Recorder,
    cancel: CancellationToken,
    settings: Settings,
    album: Album,
    track: Track,
}

impl Rig {
    async fn new() -> Self {
        let (server, client) = signed_in().await;
        let album = serde_json::from_value(json!({
            "id": "alb1",
            "title": "Kind of Blue",
            "artist": { "id": 1, "name": "Miles Davis" },
            "tracks_count": 12,
            "media_count": 1,
        }))
        .unwrap();
        let track = serde_json::from_value(json!({
            "id": 42,
            "title": "So What",
            "track_number": 1,
            "media_number": 1,
            "duration": 60,
            "performer": { "id": 1, "name": "Miles Davis" },
        }))
        .unwrap();
        let mut settings = Settings::default();
        settings.download.quality = Quality::Flac16;

        Self {
            server,
            client,
            music: TempDir::new().unwrap(),
            partial: TempDir::new().unwrap(),
            recorder: Recorder::default(),
            cancel: CancellationToken::new(),
            settings,
            album,
            track,
        }
    }

    /// The CDN serves `body` at `/cdn/file`, and Qobuz points every format at it.
    fn serve(&self, body: Vec<u8>) {
        self.server
            .on("/cdn/file", move |_| Reply::bytes(body.clone()));
        let url = format!("{}/cdn/file", self.server.url());
        self.server.on("/api/track/getFileUrl", move |call| {
            let format: u32 = call.param("format_id").parse().unwrap();
            Reply::json(json!({
                "url": url,
                "format_id": format,
                "bit_depth": 16,
                "sampling_rate": 44.1,
            }))
        });
    }

    async fn download(&self) -> Result<TrackResult, TrackError> {
        let env = Environment {
            client: self.client.clone(),
            fetcher: Fetcher::new(reqwest::Client::new(), Duration::from_secs(5)),
            temp_dir: self.partial.path().to_path_buf(),
            tuning: Tuning {
                attempts_per_quality: 5,
                backoff: Duration::ZERO,
            },
        };
        let context = Context {
            env: &env,
            settings: &self.settings,
            reporter: &self.recorder,
            cancel: &self.cancel,
        };
        context
            .download_track(&TrackRequest {
                kind: DownloadKind::Album,
                album: &self.album,
                track: &self.track,
                folder: self.music.path(),
                playlist: None,
                suffix: "",
                cover: None,
            })
            .await
    }

    fn expected_file(&self) -> PathBuf {
        self.music.path().join("01 - So What.flac")
    }

    fn partial_files(&self) -> usize {
        std::fs::read_dir(self.partial.path()).unwrap().count()
    }
}

#[tokio::test]
async fn a_track_is_downloaded_tagged_and_put_in_place() {
    let rig = Rig::new().await;
    rig.serve(flac_bytes());

    let result = rig.download().await.unwrap();

    assert_eq!(result.path, rig.expected_file());
    assert!(!result.already_existed);
    let tagged = Probe::open(&result.path).unwrap().read().unwrap();
    let tag = tagged.primary_tag().expect("the file is tagged");
    assert_eq!(tag.title().as_deref(), Some("So What"));
    assert_eq!(tag.album().as_deref(), Some("Kind of Blue"));
    assert_eq!(rig.partial_files(), 0, "no partial file is left");
    assert!(rig.recorder.has(|event| matches!(event, LogEvent::Done)));
}

#[tokio::test]
async fn the_quality_obtained_is_logged_from_what_qobuz_served() {
    let rig = Rig::new().await;
    rig.serve(flac_bytes());

    rig.download().await.unwrap();

    assert!(rig.recorder.has(|event| matches!(
        event,
        LogEvent::TrackTransfer { quality, .. } if quality == "FLAC 16bit/44.1kHz"
    )));
}

#[tokio::test]
async fn an_existing_file_is_left_alone_without_asking_qobuz() {
    let rig = Rig::new().await;
    std::fs::write(rig.expected_file(), b"already here").unwrap();

    let result = rig.download().await.unwrap();

    assert!(result.already_existed);
    assert_eq!(std::fs::read(rig.expected_file()).unwrap(), b"already here");
    assert_eq!(rig.server.count("/api/track/getFileUrl"), 0);
    assert!(
        rig.recorder
            .has(|event| matches!(event, LogEvent::FileExists { number: 1 }))
    );
}

#[tokio::test]
async fn rename_mode_keeps_both_files() {
    let mut rig = Rig::new().await;
    rig.settings.advanced.duplicates = DuplicateMode::Rename;
    rig.serve(flac_bytes());
    std::fs::write(rig.expected_file(), b"already here").unwrap();

    let first = rig.download().await.unwrap();
    let second = rig.download().await.unwrap();

    assert_eq!(first.path, rig.music.path().join("01 - So What (1).flac"));
    assert_eq!(second.path, rig.music.path().join("01 - So What (2).flac"));
    assert_eq!(std::fs::read(rig.expected_file()).unwrap(), b"already here");
}

#[tokio::test]
async fn overwrite_mode_replaces_the_file() {
    let mut rig = Rig::new().await;
    rig.settings.advanced.duplicates = DuplicateMode::Overwrite;
    rig.serve(flac_bytes());
    std::fs::write(rig.expected_file(), b"old").unwrap();

    rig.download().await.unwrap();

    assert!(std::fs::metadata(rig.expected_file()).unwrap().len() > 1000);
}

#[tokio::test]
async fn a_quality_that_keeps_failing_gives_way_to_the_next_one() {
    let mut rig = Rig::new().await;
    rig.settings.download.quality = Quality::Flac24Max;
    let good = flac_bytes();
    rig.server
        .on("/cdn/good", move |_| Reply::bytes(good.clone()));
    let base = rig.server.url().to_owned();
    rig.server.on("/api/track/getFileUrl", move |call| {
        // The 24 bit / 192 kHz file is cut by the CDN every time
        let path = if call.param("format_id") == "27" { "broken" } else { "good" };
        Reply::json(json!({ "url": format!("{base}/cdn/{path}"), "format_id": 7, "bit_depth": 24, "sampling_rate": 96.0 }))
    });
    rig.server.on("/cdn/broken", |_| Reply::status(500));

    let result = rig.download().await.unwrap();

    assert!(result.path.exists());
    assert_eq!(
        rig.server.count("/cdn/broken"),
        5,
        "five attempts per quality"
    );
    assert!(
        rig.recorder
            .has(|event| matches!(event, LogEvent::QualityFailed { attempts: 5, .. }))
    );
    assert!(rig.recorder.has(|event| matches!(
        event,
        LogEvent::QualityUnavailable { quality } if quality == "FLAC 24bit/96kHz"
    )));
}

#[tokio::test]
async fn a_cold_cdn_file_is_served_on_a_later_attempt_in_the_same_quality() {
    let rig = Rig::new().await;
    let good = flac_bytes();
    let attempts = AtomicUsize::new(0);
    rig.server.on("/cdn/file", move |_| {
        // Cut three times, then served, like a file the CDN has not fetched yet
        if attempts.fetch_add(1, Ordering::SeqCst) < 3 {
            Reply::status(503)
        } else {
            Reply::bytes(good.clone())
        }
    });
    let url = format!("{}/cdn/file", rig.server.url());
    rig.server.on("/api/track/getFileUrl", move |_| {
        Reply::json(json!({ "url": url, "format_id": 6, "bit_depth": 16, "sampling_rate": 44.1 }))
    });

    rig.download().await.unwrap();

    assert_eq!(rig.server.count("/cdn/file"), 4);
    assert!(
        !rig.recorder
            .has(|event| matches!(event, LogEvent::QualityUnavailable { .. }))
    );
    assert_eq!(
        rig.recorder
            .count(|event| matches!(event, LogEvent::AttemptFailed { .. })),
        3
    );
}

#[tokio::test]
async fn flac_never_falls_back_to_mp3() {
    let rig = Rig::new().await;
    rig.server
        .on("/api/track/getFileUrl", |_| Reply::error(400, "restricted"));

    let error = rig.download().await.unwrap_err();

    assert!(matches!(error, TrackError::Api(_)), "{error}");
    let formats: Vec<String> = rig
        .server
        .calls_to("/api/track/getFileUrl")
        .iter()
        .map(|call| call.param("format_id").to_owned())
        .collect();
    assert_eq!(
        formats,
        ["6"],
        "only the quality asked for, and not repeated"
    );
}

#[tokio::test]
async fn a_preview_is_refused_and_nothing_is_written() {
    let rig = Rig::new().await;
    let url = format!("{}/cdn/file", rig.server.url());
    rig.server.on("/api/track/getFileUrl", move |_| {
        Reply::json(json!({ "url": url, "format_id": 5, "sample": true }))
    });

    let error = rig.download().await.unwrap_err();

    assert!(matches!(error, TrackError::PreviewOnly));
    assert_eq!(rig.server.count("/cdn/file"), 0);
    assert!(!rig.expected_file().exists());
    assert!(
        rig.recorder
            .has(|event| matches!(event, LogEvent::PreviewOnly))
    );
}

#[tokio::test]
async fn a_rejected_session_stops_at_once() {
    let rig = Rig::new().await;
    rig.server.on("/api/track/getFileUrl", |_| {
        Reply::error(401, "Invalid or expired user_auth_token")
    });

    let error = rig.download().await.unwrap_err();

    assert!(matches!(&error, TrackError::Api(api) if api.is_unauthorized()));
    assert_eq!(rig.server.count("/api/track/getFileUrl"), 1);
}

#[tokio::test]
async fn a_damaged_file_is_not_accepted() {
    let rig = Rig::new().await;
    let mut damaged = flac_bytes();
    let middle = damaged.len() / 2;
    damaged[middle..middle + 32].fill(0x5A);
    rig.serve(damaged);

    let error = rig.download().await.unwrap_err();

    assert!(matches!(error, TrackError::Damaged(_)), "{error}");
    assert_eq!(rig.server.count("/cdn/file"), 5);
    assert!(!rig.expected_file().exists());
    assert_eq!(rig.partial_files(), 0);
}

#[tokio::test]
async fn a_damaged_file_goes_through_when_the_check_is_off() {
    let mut rig = Rig::new().await;
    rig.settings.advanced.verify_integrity = false;
    let mut damaged = flac_bytes();
    let middle = damaged.len() / 2;
    damaged[middle..middle + 32].fill(0x5A);
    rig.serve(damaged);

    let result = rig.download().await;

    assert!(result.is_ok(), "{:?}", result.err());
}

#[tokio::test]
async fn a_cancelled_download_leaves_no_partial_file() {
    let rig = Rig::new().await;
    rig.serve(flac_bytes());
    rig.cancel.cancel();

    let error = rig.download().await.unwrap_err();

    assert!(matches!(error, TrackError::Cancelled), "{error}");
    assert_eq!(rig.partial_files(), 0);
    assert!(!rig.expected_file().exists());
}

#[tokio::test]
async fn a_failure_after_the_download_still_removes_the_partial_file() {
    let mut rig = Rig::new().await;
    // Not audio at all: the check is off, so only the tagging notices
    rig.settings.advanced.verify_integrity = false;
    rig.serve(b"definitely not a flac file".to_vec());

    let error = rig.download().await.unwrap_err();

    assert!(matches!(error, TrackError::Tag(_)), "{error}");
    assert_eq!(rig.partial_files(), 0);
    assert!(!rig.expected_file().exists());
}

#[tokio::test]
async fn a_missing_md5_is_written_when_asked() {
    let mut rig = Rig::new().await;
    rig.settings.advanced.fix_md5 = true;
    let mut bytes = flac_bytes();
    bytes[26..42].fill(0);
    rig.serve(bytes);

    let result = rig.download().await.unwrap();

    let written = std::fs::read(&result.path).unwrap();
    assert_ne!(
        &written[26..42],
        &[0u8; 16],
        "the checksum is in the header"
    );
}

#[tokio::test]
async fn an_mp3_download_is_not_checked_as_flac() {
    let mut rig = Rig::new().await;
    rig.settings.download.quality = Quality::Mp3;
    // Not a decodable file, but tagging an MP3 header-less file is allowed; the point is that the
    // FLAC check and the FLAC fallback chain do not apply
    rig.serve(vec![0u8; 4096]);

    let _ = rig.download().await;

    let formats: Vec<String> = rig
        .server
        .calls_to("/api/track/getFileUrl")
        .iter()
        .map(|call| call.param("format_id").to_owned())
        .collect();
    assert!(formats.iter().all(|format| format == "5"), "{formats:?}");
}
