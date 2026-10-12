mod common;

use std::collections::BTreeSet;
use std::ops::Deref;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use common::catalogue::{Catalogue, page};
use common::{Recorder, Reply, TINY_JPEG, flac_bytes};
use qobuzdlx::download::{
    Covers, Display, Environment, Fetcher, ItemKind, Job, JobError, JobSetup, LogEvent, Outcome,
    QueueItem, Tuning,
};
use qobuzdlx::settings::{Quality, Settings};
use serde_json::json;
use tempfile::TempDir;
use tokio_util::sync::CancellationToken;

/// A small Qobuz and everything a job needs around it.
struct Rig {
    catalogue: Catalogue,
    music: TempDir,
    partial: TempDir,
    recorder: Arc<Recorder>,
    cancel: CancellationToken,
    skip: Arc<AtomicBool>,
    settings: Settings,
}

impl Deref for Rig {
    type Target = Catalogue;

    fn deref(&self) -> &Catalogue {
        &self.catalogue
    }
}

impl Rig {
    async fn new() -> Self {
        Self {
            catalogue: Catalogue::new().await,
            music: TempDir::new().unwrap(),
            partial: TempDir::new().unwrap(),
            recorder: Arc::new(Recorder::default()),
            cancel: CancellationToken::new(),
            skip: Arc::new(AtomicBool::new(false)),
            settings: Settings::default(),
        }
    }

    fn music_path(&self) -> std::path::PathBuf {
        self.music.path().to_path_buf()
    }

    fn item(kind: ItemKind, ref_id: &str) -> QueueItem {
        QueueItem::new(
            format!("https://play.qobuz.com/x/{ref_id}"),
            kind,
            ref_id.to_owned(),
            Display {
                label: Some("An item".into()),
                ..Display::default()
            },
        )
    }

    async fn run(&self, kind: ItemKind, ref_id: &str) -> (Result<(), JobError>, Outcome) {
        self.run_item(&Self::item(kind, ref_id)).await
    }

    async fn run_item(&self, item: &QueueItem) -> (Result<(), JobError>, Outcome) {
        let mut settings = self.settings.clone();
        settings.download.directory = self.music_path();
        settings.download.quality = Quality::Flac16;
        let http = reqwest::Client::new();
        let mut job = Job::new(
            JobSetup {
                env: Environment {
                    client: self.client.clone(),
                    fetcher: Fetcher::new(http.clone(), Duration::from_secs(5)),
                    temp_dir: self.partial.path().to_path_buf(),
                    tuning: Tuning {
                        attempts_per_quality: 2,
                        backoff: Duration::ZERO,
                    },
                },
                settings,
                reporter: self.recorder.clone(),
                cancel: self.cancel.clone(),
                skip: Arc::clone(&self.skip),
                covers: Covers::new(http),
                user_id: 99,
            },
            item,
        );
        let result = job.run(item).await;
        (result, job.into_outcome())
    }

    fn file(&self, relative: &str) -> std::path::PathBuf {
        self.music.path().join(relative)
    }
}

const KIND_OF_BLUE: [(u64, u32, &str); 3] = [
    (11, 1, "So What"),
    (12, 2, "Freddie Freeloader"),
    (13, 3, "Blue in Green"),
];

// ─── Album ───

#[tokio::test]
async fn an_album_gives_its_tracks_and_its_cover_in_the_folder_of_the_artist() {
    let rig = Rig::new().await;
    rig.add_album("alb", "Kind of Blue", "Miles Davis", &KIND_OF_BLUE);

    let (result, outcome) = rig.run(ItemKind::Album, "alb").await;

    result.unwrap();
    assert_eq!(outcome.downloaded, 3);
    for name in ["1 - So What", "2 - Freddie Freeloader", "3 - Blue in Green"] {
        assert!(
            rig.file(&format!("Miles Davis/Kind of Blue/{name}.flac"))
                .exists(),
            "{name}"
        );
    }
    assert_eq!(
        std::fs::read(rig.file("Miles Davis/Kind of Blue/Cover.jpg")).unwrap(),
        TINY_JPEG
    );
    assert!(rig.recorder.has(|event| matches!(
        event,
        LogEvent::ItemSummary {
            downloaded: 3,
            skipped: 0,
            failed: 0
        }
    )));
    assert!(
        rig.recorder
            .has(|event| matches!(event, LogEvent::Complete))
    );
}

#[tokio::test]
async fn running_an_album_again_leaves_every_file_alone() {
    let rig = Rig::new().await;
    rig.add_album("alb", "Kind of Blue", "Miles Davis", &KIND_OF_BLUE);
    rig.run(ItemKind::Album, "alb").await.0.unwrap();

    let (result, outcome) = rig.run(ItemKind::Album, "alb").await;

    result.unwrap();
    assert_eq!((outcome.downloaded, outcome.skipped), (0, 3));
}

#[tokio::test]
async fn an_album_that_is_not_streamable_is_left_out() {
    let rig = Rig::new().await;
    rig.add_album("alb", "Kind of Blue", "Miles Davis", &KIND_OF_BLUE);
    rig.change_album("alb", |album| album["streamable"] = json!(false));

    let (result, outcome) = rig.run(ItemKind::Album, "alb").await;

    result.unwrap();
    assert_eq!(outcome.downloaded, 0);
    assert!(
        rig.recorder
            .has(|event| matches!(event, LogEvent::AlbumNotStreamable))
    );
    assert_eq!(rig.server.count("/api/track/getFileUrl"), 0);
}

const SIX_TRACKS: [(u64, u32, &str); 6] = [
    (11, 1, "One"),
    (12, 2, "Two"),
    (13, 3, "Three"),
    (14, 4, "Four"),
    (15, 5, "Five"),
    (16, 6, "Six"),
];

/// An album whose files take 300 ms each to arrive.
fn slow_album(rig: &Rig) {
    rig.add_album("alb", "Six Pieces", "Anna", &SIX_TRACKS);
    let audio = flac_bytes();
    rig.server.on("/cdn/audio", move |_| {
        Reply::bytes(audio.clone()).after(Duration::from_millis(300))
    });
}

#[tokio::test]
async fn several_tracks_of_an_album_download_at_the_same_time() {
    let mut rig = Rig::new().await;
    rig.settings.advanced.parallel_tracks = 3;
    slow_album(&rig);

    let started = std::time::Instant::now();
    let (result, outcome) = rig.run(ItemKind::Album, "alb").await;
    let took = started.elapsed();

    result.unwrap();
    assert_eq!(outcome.downloaded, 6);
    for name in ["1 - One", "4 - Four", "6 - Six"] {
        assert!(rig.file(&format!("Anna/Six Pieces/{name}.flac")).exists());
    }
    // Two rounds of three files; one after the other it would take at least six
    assert!(took < Duration::from_millis(1500), "{took:?}");
}

#[tokio::test]
async fn tracks_are_started_in_the_order_of_the_album() {
    let mut rig = Rig::new().await;
    rig.settings.advanced.parallel_tracks = 3;
    rig.add_album("alb", "Six Pieces", "Anna", &SIX_TRACKS);

    let (result, _) = rig.run(ItemKind::Album, "alb").await;

    result.unwrap();
    let asked: Vec<String> = rig
        .server
        .calls_to("/api/track/get")
        .iter()
        .map(|call| call.param("track_id").to_owned())
        .collect();
    assert_eq!(asked, ["11", "12", "13", "14", "15", "16"]);
}

#[tokio::test]
async fn one_at_a_time_is_one_at_a_time() {
    let mut rig = Rig::new().await;
    rig.settings.advanced.parallel_tracks = 1;
    slow_album(&rig);

    let started = std::time::Instant::now();
    let (result, outcome) = rig.run(ItemKind::Album, "alb").await;

    result.unwrap();
    assert_eq!(outcome.downloaded, 6);
    assert!(started.elapsed() >= Duration::from_millis(1800));
}

#[tokio::test]
async fn an_album_already_in_place_is_gone_through_without_asking_for_each_track() {
    let rig = Rig::new().await;
    rig.add_album("alb", "Six Pieces", "Anna", &SIX_TRACKS);
    let (first, _) = rig.run(ItemKind::Album, "alb").await;
    first.unwrap();
    assert_eq!(rig.server.count("/api/track/get"), 6);

    let (second, outcome) = rig.run(ItemKind::Album, "alb").await;

    second.unwrap();
    assert_eq!((outcome.downloaded, outcome.skipped), (0, 6));
    assert_eq!(
        rig.server.count("/api/track/get"),
        6,
        "no request for a track that is there"
    );
    assert_eq!(rig.server.count("/api/track/getFileUrl"), 6);
}

#[tokio::test]
async fn the_progress_of_an_album_counts_the_tracks_in_place() {
    let mut rig = Rig::new().await;
    rig.settings.advanced.parallel_tracks = 2;
    rig.add_album("alb", "Six Pieces", "Anna", &SIX_TRACKS);

    let (result, _) = rig.run(ItemKind::Album, "alb").await;

    result.unwrap();
    let steps = rig.recorder.steps();
    assert_eq!(steps, [16, 33, 50, 66, 83, 100]);
}

#[tokio::test]
async fn a_track_that_fails_is_recorded_and_the_others_go_on() {
    let rig = Rig::new().await;
    rig.add_album("alb", "Kind of Blue", "Miles Davis", &KIND_OF_BLUE);
    let url = format!("{}/cdn/audio", rig.server.url());
    rig.server.on("/api/track/getFileUrl", move |call| {
        if call.param("track_id") == "12" {
            Reply::error(403, "Track restricted")
        } else {
            Reply::json(
                json!({ "url": url, "format_id": 6, "bit_depth": 16, "sampling_rate": 44.1 }),
            )
        }
    });

    let (result, outcome) = rig.run(ItemKind::Album, "alb").await;

    result.unwrap();
    assert_eq!(outcome.downloaded, 2);
    assert_eq!(outcome.failures.len(), 1);
    assert_eq!(outcome.failures[0].track_id, 12);
    assert_eq!(outcome.failures[0].title, "Freddie Freeloader");
    assert!(
        outcome.failures[0].error.contains("restricted"),
        "{}",
        outcome.failures[0].error
    );
}

#[tokio::test]
async fn retrying_the_failures_only_does_those_tracks() {
    let rig = Rig::new().await;
    rig.add_album("alb", "Kind of Blue", "Miles Davis", &KIND_OF_BLUE);
    let mut item = Rig::item(ItemKind::Album, "alb");
    item.downloaded = 2;
    item.only_track_ids = Some(BTreeSet::from([12]));

    let (result, outcome) = rig.run_item(&item).await;

    result.unwrap();
    assert_eq!(
        outcome.downloaded, 3,
        "builds on what the item had achieved"
    );
    assert!(
        rig.file("Miles Davis/Kind of Blue/2 - Freddie Freeloader.flac")
            .exists()
    );
    assert!(
        !rig.file("Miles Davis/Kind of Blue/1 - So What.flac")
            .exists()
    );
}

#[tokio::test]
async fn a_skip_drops_the_album_in_progress() {
    let rig = Rig::new().await;
    rig.add_album("alb", "Kind of Blue", "Miles Davis", &KIND_OF_BLUE);
    rig.skip.store(true, Ordering::SeqCst);

    let (result, outcome) = rig.run(ItemKind::Album, "alb").await;

    result.unwrap();
    assert_eq!(outcome.downloaded, 0);
    assert!(
        rig.recorder
            .has(|event| matches!(event, LogEvent::AlbumSkipped))
    );
    assert!(
        !rig.skip.load(Ordering::SeqCst),
        "asking again is needed for the next one"
    );
}

#[tokio::test]
async fn bonus_files_are_downloaded_once_with_the_id_in_their_name() {
    let rig = Rig::new().await;
    rig.add_album("alb", "Kind of Blue", "Miles Davis", &KIND_OF_BLUE[..1]);
    rig.server
        .on("/cdn/booklet.pdf", |_| Reply::bytes(b"%PDF".to_vec()));
    // A signed address: the query must not end up in the name
    let booklet = format!("{}/cdn/booklet.pdf?Expires=1", rig.server.url());
    rig.change_album("alb", |album| {
        album["goodies"] = json!([
            { "id": 5, "file_format_id": 21, "description": "Livret", "url": booklet },
            { "id": 6, "file_format_id": 52, "description": "Link", "url": booklet },
        ]);
    });

    rig.run(ItemKind::Album, "alb").await.0.unwrap();
    rig.run(ItemKind::Album, "alb").await.0.unwrap();

    assert_eq!(
        std::fs::read(rig.file("Miles Davis/Kind of Blue/Livret (5).pdf")).unwrap(),
        b"%PDF"
    );
    assert!(!rig.file("Miles Davis/Kind of Blue/Link (6).pdf").exists());
    assert_eq!(rig.server.count("/cdn/booklet.pdf"), 1);
    assert!(
        rig.recorder
            .has(|event| matches!(event, LogEvent::GoodyExists))
    );
}

#[tokio::test]
async fn a_failed_bonus_file_does_not_fail_the_album() {
    let rig = Rig::new().await;
    rig.add_album("alb", "Kind of Blue", "Miles Davis", &KIND_OF_BLUE[..1]);
    let broken = format!("{}/cdn/missing.pdf", rig.server.url());
    rig.change_album("alb", |album| {
        album["goodies"] =
            json!([{ "id": 5, "file_format_id": 21, "description": "Livret", "url": broken }]);
    });

    let (result, outcome) = rig.run(ItemKind::Album, "alb").await;

    result.unwrap();
    assert_eq!(outcome.downloaded, 1);
    assert!(
        rig.recorder
            .has(|event| matches!(event, LogEvent::GoodyFailed { .. }))
    );
    assert!(!rig.file("Miles Davis/Kind of Blue/Livret (5).pdf").exists());
    assert!(
        !rig.file("Miles Davis/Kind of Blue/Livret (5).part")
            .exists()
    );
}

// ─── Stopping ───

#[tokio::test]
async fn a_rejected_session_ends_the_item_instead_of_failing_every_track() {
    let mut rig = Rig::new().await;
    rig.settings.advanced.parallel_tracks = 1;
    rig.add_album("alb", "Kind of Blue", "Miles Davis", &KIND_OF_BLUE);
    rig.server.on("/api/track/get", |_| {
        Reply::error(401, "Invalid or expired user_auth_token")
    });

    let (result, outcome) = rig.run(ItemKind::Album, "alb").await;

    assert!(
        matches!(result, Err(JobError::SessionExpired)),
        "{result:?}"
    );
    assert!(outcome.failures.is_empty());
    assert_eq!(rig.server.count("/api/track/get"), 1);
}

#[tokio::test]
async fn a_rejected_session_ends_an_album_downloading_several_tracks_at_once() {
    let mut rig = Rig::new().await;
    rig.settings.advanced.parallel_tracks = 3;
    rig.add_album("alb", "Kind of Blue", "Miles Davis", &KIND_OF_BLUE);
    rig.server.on("/api/track/get", |_| {
        Reply::error(401, "Invalid or expired user_auth_token")
    });

    let (result, outcome) = rig.run(ItemKind::Album, "alb").await;

    assert!(
        matches!(result, Err(JobError::SessionExpired)),
        "{result:?}"
    );
    assert!(
        outcome.failures.is_empty(),
        "the others stopping are not failures"
    );
    assert!(rig.server.count("/api/track/get") <= 3);
}

#[tokio::test]
async fn a_cancelled_item_stops_where_it_is() {
    let rig = Rig::new().await;
    rig.add_album("alb", "Kind of Blue", "Miles Davis", &KIND_OF_BLUE);
    rig.cancel.cancel();

    let (result, outcome) = rig.run(ItemKind::Album, "alb").await;

    assert!(matches!(result, Err(JobError::Cancelled)));
    assert_eq!(outcome.downloaded, 0);
}

// ─── Single track ───

#[tokio::test]
async fn a_track_on_its_own_goes_to_the_folder_of_its_album() {
    let rig = Rig::new().await;
    rig.add_album("alb", "Kind of Blue", "Miles Davis", &KIND_OF_BLUE);

    let (result, outcome) = rig.run(ItemKind::Track, "12").await;

    result.unwrap();
    assert_eq!(outcome.downloaded, 1);
    assert!(
        rig.file("Miles Davis/Kind of Blue/2 - Freddie Freeloader.flac")
            .exists()
    );
    assert!(
        !rig.file("Miles Davis/Kind of Blue/1 - So What.flac")
            .exists()
    );
}

#[tokio::test]
async fn a_track_that_does_not_exist_is_a_failure_of_the_item() {
    let rig = Rig::new().await;

    let (result, outcome) = rig.run(ItemKind::Track, "777").await;

    result.unwrap();
    assert_eq!(outcome.failures.len(), 1);
    assert!(
        rig.recorder
            .has(|event| matches!(event, LogEvent::TrackNotFound))
    );
}

#[tokio::test]
async fn a_track_that_cannot_be_downloaded_fails_the_item_with_the_reason() {
    let rig = Rig::new().await;
    rig.add_album("alb", "Kind of Blue", "Miles Davis", &KIND_OF_BLUE);
    rig.server.on("/api/track/getFileUrl", |_| {
        Reply::error(403, "Track restricted")
    });

    let (result, _) = rig.run(ItemKind::Track, "12").await;

    let error = result.unwrap_err();
    assert!(error.to_string().contains("restricted"), "{error}");
}

// ─── Playlist ───

#[tokio::test]
async fn a_playlist_gathers_tracks_of_several_albums_with_an_m3u8_in_order() {
    let rig = Rig::new().await;
    rig.add_album("a1", "Kind of Blue", "Miles Davis", &KIND_OF_BLUE);
    rig.add_album(
        "a2",
        "A Love Supreme",
        "John Coltrane",
        &[(21, 1, "Acknowledgement")],
    );
    rig.add_playlist(7, "Evening", &[(21, 1), (12, 2)]);

    let (result, outcome) = rig.run(ItemKind::Playlist, "7").await;

    result.unwrap();
    assert_eq!(outcome.downloaded, 2);
    assert!(rig.file("Evening/1 - Acknowledgement.flac").exists());
    assert!(rig.file("Evening/2 - Freddie Freeloader.flac").exists());
    let playlist = std::fs::read_to_string(rig.file("Evening/Evening.m3u8")).unwrap();
    assert_eq!(
        playlist,
        "#EXTM3U\n#EXTINF:100,John Coltrane - Acknowledgement\n1 - Acknowledgement.flac\n\
         #EXTINF:100,Miles Davis - Freddie Freeloader\n2 - Freddie Freeloader.flac\n"
    );
    assert_eq!(rig.recorder.albums(), ["A Love Supreme", "Kind of Blue"]);
}

#[tokio::test]
async fn two_tracks_with_the_same_name_in_a_playlist_do_not_overwrite_each_other() {
    let rig = Rig::new().await;
    rig.add_album("a1", "First", "Artist", &[(31, 1, "Intro")]);
    rig.add_album("a2", "Second", "Artist", &[(32, 1, "Intro")]);
    rig.add_playlist(8, "Mix", &[(31, 1), (32, 2)]);
    // Both are numbered by their position in the playlist, so both are called "1 - Intro"
    rig.tracks.lock().unwrap().get_mut(&32).unwrap()["position"] = json!(1);

    let (result, outcome) = rig.run(ItemKind::Playlist, "8").await;

    result.unwrap();
    assert_eq!(outcome.downloaded, 2);
    assert!(rig.file("Mix/1 - Intro.flac").exists());
    assert!(rig.file("Mix/1 - Intro (32).flac").exists());
    let playlist = std::fs::read_to_string(rig.file("Mix/Mix.m3u8")).unwrap();
    assert!(playlist.contains("1 - Intro.flac\n"));
    assert!(playlist.contains("1 - Intro (32).flac\n"));
}

#[tokio::test]
async fn the_playlist_file_can_be_turned_off() {
    let mut rig = Rig::new().await;
    rig.settings.advanced.playlist_file = false;
    rig.add_album("a1", "Kind of Blue", "Miles Davis", &KIND_OF_BLUE);
    rig.add_playlist(7, "Evening", &[(12, 1)]);

    rig.run(ItemKind::Playlist, "7").await.0.unwrap();

    assert!(!rig.file("Evening/Evening.m3u8").exists());
}

#[tokio::test]
async fn a_retry_adds_the_recovered_track_to_the_playlist_file() {
    let rig = Rig::new().await;
    rig.add_album("a1", "Kind of Blue", "Miles Davis", &KIND_OF_BLUE);
    rig.add_playlist(7, "Evening", &[(11, 1), (12, 2)]);
    let mut item = Rig::item(ItemKind::Playlist, "7");
    item.downloaded = 1;
    item.playlist_entries = vec![qobuzdlx::download::PlaylistEntry {
        index: 0,
        file: rig.file("Evening/1 - So What.flac"),
        duration_seconds: 100,
        title: "Miles Davis - So What".into(),
    }];
    item.only_track_ids = Some(BTreeSet::from([12]));

    let (result, outcome) = rig.run_item(&item).await;

    result.unwrap();
    assert_eq!(outcome.playlist_entries.len(), 2);
    let playlist = std::fs::read_to_string(rig.file("Evening/Evening.m3u8")).unwrap();
    assert!(playlist.contains("1 - So What.flac\n"));
    assert!(playlist.contains("2 - Freddie Freeloader.flac\n"));
}

// ─── Artist, label and favorites ───

#[tokio::test]
async fn an_artist_gives_every_album_and_the_progress_covers_the_whole_discography() {
    let rig = Rig::new().await;
    rig.add_album("a1", "Kind of Blue", "Miles Davis", &KIND_OF_BLUE[..2]);
    rig.add_album(
        "a2",
        "Bitches Brew",
        "Miles Davis",
        &[(41, 1, "Pharaoh's Dance")],
    );
    rig.artists.lock().unwrap().insert(
        1,
        json!({ "id": 1, "name": "Miles Davis", "albums": page(vec![json!({"id": "a1", "title": "Kind of Blue", "artist": {"id": 1}}), json!({"id": "a2", "title": "Bitches Brew", "artist": {"id": 1}})]) }),
    );

    let (result, outcome) = rig.run(ItemKind::Artist, "1").await;

    result.unwrap();
    assert_eq!(outcome.downloaded, 3);
    assert!(
        rig.file("Miles Davis/Bitches Brew/1 - Pharaoh's Dance.flac")
            .exists()
    );
    let steps = rig.recorder.steps();
    assert_eq!(steps.first(), Some(&0));
    assert!(
        steps.windows(2).all(|pair| pair[0] <= pair[1]),
        "never goes back: {steps:?}"
    );
    assert!(
        steps.iter().any(|percent| *percent >= 50),
        "the second album starts half way: {steps:?}"
    );
}

#[tokio::test]
async fn an_album_of_an_artist_that_cannot_be_had_is_reported_and_the_next_goes_on() {
    let rig = Rig::new().await;
    rig.add_album(
        "a2",
        "Bitches Brew",
        "Miles Davis",
        &[(41, 1, "Pharaoh's Dance")],
    );
    rig.artists.lock().unwrap().insert(
        1,
        json!({ "id": 1, "name": "Miles Davis", "albums": page(vec![json!({"id": "gone", "title": "Gone", "artist": {"id": 1}}), json!({"id": "a2", "title": "Bitches Brew", "artist": {"id": 1}})]) }),
    );

    let (result, outcome) = rig.run(ItemKind::Artist, "1").await;

    result.unwrap();
    assert_eq!(outcome.downloaded, 1);
    assert!(rig.recorder.has(
        |event| matches!(event, LogEvent::AlbumFailed { album_id, .. } if album_id == "gone")
    ));
}

#[tokio::test]
async fn a_label_gives_its_albums() {
    let rig = Rig::new().await;
    rig.add_album("a1", "Kind of Blue", "Miles Davis", &KIND_OF_BLUE[..1]);
    rig.labels.lock().unwrap().insert(
        3,
        json!({ "id": 3, "name": "Columbia", "albums": page(vec![json!({"id": "a1", "title": "Kind of Blue", "artist": {"id": 1}})]) }),
    );

    let (result, outcome) = rig.run(ItemKind::Label, "3").await;

    result.unwrap();
    assert_eq!(outcome.downloaded, 1);
}

#[tokio::test]
async fn favorites_are_downloaded_by_kind() {
    let rig = Rig::new().await;
    rig.add_album("a1", "Kind of Blue", "Miles Davis", &KIND_OF_BLUE);
    rig.favorites.lock().unwrap().insert(
        "albums".into(),
        json!({ "albums": page(vec![json!({"id": "a1", "title": "Kind of Blue", "artist": {"id": 1}})]) }),
    );
    rig.favorites.lock().unwrap().insert(
        "tracks".into(),
        json!({ "tracks": page(vec![json!({"id": 13, "title": "Blue in Green"})]) }),
    );

    let (albums, outcome) = rig.run(ItemKind::FavoriteAlbums, "").await;
    albums.unwrap();
    assert_eq!(outcome.downloaded, 3);

    let (tracks, outcome) = rig.run(ItemKind::FavoriteTracks, "").await;
    tracks.unwrap();
    assert_eq!(
        (outcome.downloaded, outcome.skipped),
        (0, 1),
        "already there from the album"
    );
    assert_eq!(
        rig.server.calls_to("/api/favorite/getUserFavorites")[0].param("user_id"),
        "99"
    );
}

#[tokio::test]
async fn empty_favorites_say_so() {
    let rig = Rig::new().await;
    rig.favorites
        .lock()
        .unwrap()
        .insert("albums".into(), json!({ "albums": page(vec![]) }));

    let (result, outcome) = rig.run(ItemKind::FavoriteAlbums, "").await;

    result.unwrap();
    assert_eq!(outcome.downloaded, 0);
    assert!(
        rig.recorder
            .has(|event| matches!(event, LogEvent::FavoritesEmpty))
    );
}

#[tokio::test]
async fn an_address_with_a_bad_identifier_is_refused() {
    let rig = Rig::new().await;

    let (result, _) = rig.run(ItemKind::Playlist, "not-a-number").await;

    assert!(matches!(result, Err(JobError::InvalidAddress)));
}
