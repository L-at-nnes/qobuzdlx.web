mod common;

use std::time::Duration;

use common::catalogue::{Catalogue, page};
use common::{Reply, session};
use futures_util::future::BoxFuture;
use qobuzdlx::download::{
    Display, Enqueued, Event, ItemKind, ItemStatus, LogEvent, Manager, ManagerConfig, Queue,
    QueueItem, QueueStore, Tuning,
};
use qobuzdlx::download::{Halt, WaitReason};
use qobuzdlx::extensions::{
    Event as Told, Extension, Extensions, Failure, HaltReason, Info, Source,
};
use qobuzdlx::settings::{Delay, Quality, RateLimit, SettingsStore};
use serde_json::json;
use tempfile::TempDir;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpListener;

const KIND_OF_BLUE: [(u64, u32, &str); 3] = [
    (11, 1, "So What"),
    (12, 2, "Freddie Freeloader"),
    (13, 3, "Blue in Green"),
];

struct Rig {
    catalogue: Catalogue,
    music: TempDir,
    config: TempDir,
    settings: SettingsStore,
}

impl Rig {
    async fn new() -> Self {
        let rig = Self {
            catalogue: Catalogue::new().await,
            music: TempDir::new().unwrap(),
            config: TempDir::new().unwrap(),
            settings: SettingsStore::open(TempDir::new().unwrap().path(), std::path::Path::new("")),
        };
        let music = rig.music.path().to_path_buf();
        rig.settings
            .update(|settings| {
                settings.download.directory = music;
                settings.download.quality = Quality::Flac16;
                settings.rate_limit = RateLimit::off();
            })
            .unwrap();
        rig
    }

    fn start(&self) -> Manager {
        self.start_with(Extensions::default())
    }

    fn start_with(&self, extensions: Extensions) -> Manager {
        Manager::start(ManagerConfig {
            client: self.catalogue.client.clone(),
            settings: self.settings.clone(),
            config_dir: self.config.path().to_path_buf(),
            http: reqwest::Client::new(),
            stall_timeout: Duration::from_secs(10),
            tuning: Tuning {
                attempts_per_quality: 2,
                backoff: Duration::ZERO,
            },
            extensions,
        })
    }

    fn file(&self, relative: &str) -> std::path::PathBuf {
        self.music.path().join(relative)
    }
}

fn album_url(id: &str) -> String {
    format!("https://play.qobuz.com/album/{id}")
}

/// Waits for something that happens in the background.
async fn eventually(what: &str, mut condition: impl FnMut() -> bool) {
    for _ in 0..300 {
        if condition() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    panic!("timed out waiting for: {what}");
}

fn status_of(manager: &Manager, url: &str) -> Option<ItemStatus> {
    manager
        .snapshot()
        .queue
        .iter()
        .find(|item| item.url == url)
        .map(|item| item.status)
}

async fn finished(manager: &Manager, url: &str) -> ItemStatus {
    eventually("the item to finish", || {
        status_of(manager, url).is_some_and(ItemStatus::is_finished)
    })
    .await;
    status_of(manager, url).unwrap()
}

/// A CDN that sends the start of a file and then says nothing more.
async fn stalled_cdn() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    tokio::spawn(async move {
        loop {
            let Ok((mut socket, _)) = listener.accept().await else {
                return;
            };
            tokio::spawn(async move {
                let mut request = [0u8; 2048];
                let _ = tokio::io::AsyncReadExt::read(&mut socket, &mut request).await;
                let _ = socket
                    .write_all(b"HTTP/1.1 200 OK\r\ncontent-length: 900000\r\n\r\nstart")
                    .await;
                tokio::time::sleep(Duration::from_secs(60)).await;
            });
        }
    });
    format!("http://{address}/file")
}

/// An extension that keeps everything it is told.
struct Recorder(std::sync::Arc<std::sync::Mutex<Vec<Told>>>);

impl Extension for Recorder {
    fn info(&self) -> Info {
        Info {
            name: "recorder".into(),
            version: None,
            description: None,
            source: Source::Builtin,
            events: vec![],
        }
    }

    fn handle<'a>(&'a self, event: &'a Told) -> BoxFuture<'a, Result<(), Failure>> {
        Box::pin(async move {
            self.0.lock().unwrap().push(event.clone());
            Ok(())
        })
    }
}

// ─── Extensions ───

#[tokio::test]
async fn extensions_are_told_of_the_tracks_the_item_and_the_end_of_the_queue() {
    let rig = Rig::new().await;
    rig.catalogue
        .add_album("alb", "Kind of Blue", "Miles Davis", &KIND_OF_BLUE);
    let told = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let extensions = Extensions::start(vec![std::sync::Arc::new(Recorder(told.clone()))], vec![]);
    let manager = rig.start_with(extensions);

    manager
        .enqueue(&album_url("alb"), Display::default())
        .unwrap();
    eventually("the queue to be drained", || {
        told.lock()
            .unwrap()
            .iter()
            .any(|event| matches!(event, Told::QueueDrained))
    })
    .await;

    let told = told.lock().unwrap();
    let tracks: Vec<_> = told
        .iter()
        .filter_map(|event| match event {
            Told::TrackSaved { track, .. } => Some(track),
            _ => None,
        })
        .collect();
    assert_eq!(tracks.len(), 3);
    assert!(
        tracks
            .iter()
            .all(|track| std::path::Path::new(&track.path).exists())
    );
    assert!(
        tracks
            .iter()
            .all(|track| track.album_title == "Kind of Blue")
    );
    assert!(tracks.iter().any(|track| track.title == "So What"));

    let finished: Vec<_> = told
        .iter()
        .filter_map(|event| match event {
            Told::ItemFinished(item) => Some(item),
            _ => None,
        })
        .collect();
    assert_eq!(finished.len(), 1);
    assert_eq!(finished[0].status, ItemStatus::Completed);
    assert_eq!(finished[0].downloaded, 3);
    assert_eq!(finished[0].folders.len(), 1);
    // The item is announced after its tracks, and the end of the queue after the item
    let position = |wanted: fn(&Told) -> bool| told.iter().position(wanted).unwrap();
    assert!(
        position(|event| matches!(event, Told::ItemFinished(_)))
            < position(|event| matches!(event, Told::QueueDrained))
    );
}

// ─── Asking ───

#[tokio::test]
async fn something_that_is_not_an_address_is_refused_and_logged() {
    let rig = Rig::new().await;
    let manager = rig.start();

    let result = manager.enqueue("not an address", Display::default());

    assert!(result.is_err());
    assert!(manager.snapshot().queue.is_empty());
    assert!(
        manager
            .log_lines()
            .iter()
            .any(|line| matches!(line.event, LogEvent::InvalidUrl { .. }))
    );
}

#[tokio::test]
async fn an_album_is_downloaded_in_the_background() {
    let rig = Rig::new().await;
    rig.catalogue
        .add_album("alb", "Kind of Blue", "Miles Davis", &KIND_OF_BLUE);
    let manager = rig.start();

    let result = manager.enqueue(&album_url("alb"), Display::default());

    assert_eq!(result, Ok(Enqueued::Added));
    assert_eq!(
        finished(&manager, &album_url("alb")).await,
        ItemStatus::Completed
    );
    for name in ["1 - So What", "2 - Freddie Freeloader", "3 - Blue in Green"] {
        assert!(
            rig.file(&format!("Miles Davis/Kind of Blue/{name}.flac"))
                .exists(),
            "{name}"
        );
    }
    let item = &manager.snapshot().queue[0];
    assert_eq!(item.downloaded, 3);
    assert!(item.finished_at.is_some());
}

#[tokio::test]
async fn the_progress_returns_to_rest_when_the_queue_is_drained() {
    let rig = Rig::new().await;
    rig.catalogue
        .add_album("alb", "Kind of Blue", "Miles Davis", &KIND_OF_BLUE);
    let manager = rig.start();

    manager
        .enqueue(&album_url("alb"), Display::default())
        .unwrap();
    finished(&manager, &album_url("alb")).await;
    eventually("the queue to be drained", || {
        manager
            .log_lines()
            .iter()
            .any(|line| matches!(line.event, LogEvent::QueueDrained))
    })
    .await;

    let snapshot = manager.snapshot();
    assert!(!snapshot.progress.running);
    assert!(snapshot.progress.album.is_none(), "the last card is gone");
    assert_eq!(snapshot.remaining, 0);
}

#[tokio::test]
async fn an_address_already_waiting_is_not_added_again() {
    let rig = Rig::new().await;
    rig.catalogue
        .add_album("alb", "Kind of Blue", "Miles Davis", &KIND_OF_BLUE);
    // Signed out, so that the first one stays waiting
    rig.catalogue.client.end_session();
    let manager = rig.start();

    let first = manager.enqueue(&album_url("alb"), Display::default());
    let again = manager.enqueue(
        "https://www.qobuz.com/fr-fr/album/kind-of-blue/alb?utm=x",
        Display::default(),
    );

    assert_eq!(first, Ok(Enqueued::Added));
    assert_eq!(again, Ok(Enqueued::AlreadyQueued));
    assert_eq!(manager.snapshot().queue.len(), 1);
    assert!(manager.is_queued(&album_url("alb")));
    assert!(!manager.is_done(&album_url("alb")));
}

#[tokio::test]
async fn a_downloaded_address_can_be_queued_again_without_a_duplicate() {
    let rig = Rig::new().await;
    rig.catalogue
        .add_album("alb", "Kind of Blue", "Miles Davis", &KIND_OF_BLUE[..1]);
    let manager = rig.start();
    manager
        .enqueue(&album_url("alb"), Display::default())
        .unwrap();
    finished(&manager, &album_url("alb")).await;
    assert!(manager.is_done(&album_url("alb")));

    let again = manager.enqueue(&album_url("alb"), Display::default());

    assert_eq!(again, Ok(Enqueued::Requeued));
    assert_eq!(manager.snapshot().queue.len(), 1);
}

#[tokio::test]
async fn items_are_processed_in_the_order_they_were_asked() {
    let rig = Rig::new().await;
    rig.catalogue
        .add_album("one", "First", "Artist", &[(21, 1, "A")]);
    rig.catalogue
        .add_album("two", "Second", "Artist", &[(22, 1, "B")]);
    rig.catalogue
        .add_album("three", "Third", "Artist", &[(23, 1, "C")]);
    rig.catalogue.client.end_session();
    let manager = rig.start();
    for id in ["one", "two", "three"] {
        manager.enqueue(&album_url(id), Display::default()).unwrap();
    }
    // The last one goes first
    let third = manager.snapshot().queue[2].id.clone();
    manager.move_item(&third, -1);
    manager.move_item(&third, -1);

    rig.catalogue.client.start_session(session());
    finished(&manager, &album_url("two")).await;
    finished(&manager, &album_url("one")).await;

    let order: Vec<String> = manager
        .log_lines()
        .iter()
        .filter_map(|line| match &line.event {
            LogEvent::DownloadingTrack { title, .. } => Some(title.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(order, ["C", "A", "B"]);
}

// ─── Stopping ───

#[tokio::test]
async fn cancelling_gives_up_the_item_and_the_queue_goes_on() {
    let rig = Rig::new().await;
    rig.catalogue
        .add_album("slow", "Slow", "Artist", &[(31, 1, "Stuck")]);
    rig.catalogue
        .add_album("fast", "Fast", "Artist", &[(32, 1, "Fine")]);
    let stalled = stalled_cdn().await;
    let good = format!("{}/cdn/audio", rig.catalogue.server.url());
    rig.catalogue
        .server
        .on("/api/track/getFileUrl", move |call| {
            let url = if call.param("track_id") == "31" {
                &stalled
            } else {
                &good
            };
            Reply::json(
                json!({ "url": url, "format_id": 6, "bit_depth": 16, "sampling_rate": 44.1 }),
            )
        });
    let manager = rig.start();
    manager
        .enqueue(&album_url("slow"), Display::default())
        .unwrap();
    manager
        .enqueue(&album_url("fast"), Display::default())
        .unwrap();
    eventually("the first track to start", || {
        !manager.snapshot().progress.tracks.is_empty()
    })
    .await;

    manager.cancel_current();

    assert_eq!(
        finished(&manager, &album_url("slow")).await,
        ItemStatus::Cancelled
    );
    assert_eq!(
        finished(&manager, &album_url("fast")).await,
        ItemStatus::Completed
    );
    assert!(rig.file("Artist/Fast/1 - Fine.flac").exists());
    assert!(!rig.file("Artist/Slow/1 - Stuck.flac").exists());
    assert!(
        manager
            .log_lines()
            .iter()
            .any(|line| matches!(line.event, LogEvent::Aborted))
    );
}

#[tokio::test]
async fn an_item_stopped_by_the_shutdown_waits_again_at_the_next_start() {
    let rig = Rig::new().await;
    rig.catalogue
        .add_album("slow", "Slow", "Artist", &[(31, 1, "Stuck")]);
    let stalled = stalled_cdn().await;
    rig.catalogue.server.on("/api/track/getFileUrl", move |_| {
        Reply::json(
            json!({ "url": stalled, "format_id": 6, "bit_depth": 16, "sampling_rate": 44.1 }),
        )
    });
    let manager = rig.start();
    manager
        .enqueue(&album_url("slow"), Display::default())
        .unwrap();
    eventually("the track to start", || {
        !manager.snapshot().progress.tracks.is_empty()
    })
    .await;

    manager.shutdown();
    tokio::time::sleep(Duration::from_millis(300)).await;

    let restored = QueueStore::new(rig.config.path()).load();
    assert_eq!(restored.items().len(), 1);
    assert_eq!(restored.items()[0].status, ItemStatus::Pending);
}

#[tokio::test]
async fn a_refused_session_pauses_the_item_instead_of_failing_it() {
    let rig = Rig::new().await;
    rig.catalogue
        .add_album("one", "First", "Artist", &[(21, 1, "A")]);
    rig.catalogue
        .add_album("two", "Second", "Artist", &[(22, 1, "B")]);
    rig.catalogue.server.on("/api/track/get", |_| {
        Reply::error(401, "Invalid or expired user_auth_token")
    });
    let manager = rig.start();
    let mut events = manager.subscribe();

    manager
        .enqueue(&album_url("one"), Display::default())
        .unwrap();
    manager
        .enqueue(&album_url("two"), Display::default())
        .unwrap();

    eventually("the session to be forgotten", || {
        !rig.catalogue.client.is_signed_in()
    })
    .await;
    eventually("the item to wait again", || {
        status_of(&manager, &album_url("one")) == Some(ItemStatus::Pending)
    })
    .await;
    assert_eq!(
        status_of(&manager, &album_url("two")),
        Some(ItemStatus::Pending)
    );
    assert!(
        manager
            .snapshot()
            .queue
            .iter()
            .all(|item| item.error.is_none())
    );
    let mut announced = false;
    while let Ok(event) = events.try_recv() {
        announced |= matches!(event, Event::SessionExpired);
    }
    assert!(announced, "the interface is told to ask for a new sign-in");
}

fn halts_told(told: &std::sync::Mutex<Vec<Told>>) -> Vec<HaltReason> {
    told.lock()
        .unwrap()
        .iter()
        .filter_map(|event| match event {
            Told::QueueHalted { reason } => Some(*reason),
            _ => None,
        })
        .collect()
}

#[tokio::test]
async fn extensions_are_told_once_when_the_session_expires_with_work_waiting() {
    let rig = Rig::new().await;
    rig.catalogue
        .add_album("one", "First", "Artist", &[(21, 1, "A")]);
    rig.catalogue.server.on("/api/track/get", |_| {
        Reply::error(401, "Invalid or expired user_auth_token")
    });
    let told = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let extensions = Extensions::start(vec![std::sync::Arc::new(Recorder(told.clone()))], vec![]);
    let manager = rig.start_with(extensions);

    manager
        .enqueue(&album_url("one"), Display::default())
        .unwrap();

    eventually("the extensions to be told", || {
        !halts_told(&told).is_empty()
    })
    .await;
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert_eq!(halts_told(&told), [HaltReason::SessionExpired]);
}

#[tokio::test]
async fn a_session_that_expires_with_nothing_to_download_is_not_a_halt() {
    let rig = Rig::new().await;
    rig.catalogue.server.on("/api/track/get", |_| {
        Reply::error(401, "Invalid or expired user_auth_token")
    });
    let told = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let extensions = Extensions::start(vec![std::sync::Arc::new(Recorder(told.clone()))], vec![]);
    let _manager = rig.start_with(extensions);

    // Someone browsing meets the refusal; no item is involved
    assert!(rig.catalogue.client.track(1).await.is_err());

    eventually("the session to be forgotten", || {
        !rig.catalogue.client.is_signed_in()
    })
    .await;
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert!(halts_told(&told).is_empty());
}

// ─── An account that went back to free ───

const TEN_TRACKS: [(u64, u32, &str); 10] = [
    (31, 1, "One"),
    (32, 2, "Two"),
    (33, 3, "Three"),
    (34, 4, "Four"),
    (35, 5, "Five"),
    (36, 6, "Six"),
    (37, 7, "Seven"),
    (38, 8, "Eight"),
    (39, 9, "Nine"),
    (40, 10, "Ten"),
];

fn music_files(rig: &Rig) -> usize {
    walk(rig.music.path())
}

fn walk(folder: &std::path::Path) -> usize {
    std::fs::read_dir(folder).map_or(0, |entries| {
        entries
            .filter_map(Result::ok)
            .map(|entry| {
                let path = entry.path();
                if path.is_dir() {
                    walk(&path)
                } else {
                    usize::from(
                        path.extension()
                            .is_some_and(|extension| extension == "flac"),
                    )
                }
            })
            .sum()
    })
}

/// Two albums of ten tracks, and a Qobuz that refuses every file the way `refusal` says.
async fn refused_everywhere(
    refusal: impl Fn() -> Reply + Send + Sync + 'static,
) -> (Rig, Manager, std::sync::Arc<std::sync::Mutex<Vec<Told>>>) {
    let rig = Rig::new().await;
    rig.catalogue
        .add_album("one", "First", "Artist", &TEN_TRACKS);
    rig.catalogue
        .add_album("two", "Second", "Artist", &[(51, 1, "Other")]);
    rig.catalogue
        .server
        .on("/api/track/getFileUrl", move |_| refusal());
    let told = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let extensions = Extensions::start(vec![std::sync::Arc::new(Recorder(told.clone()))], vec![]);
    let manager = rig.start_with(extensions);
    manager
        .enqueue(&album_url("one"), Display::default())
        .unwrap();
    manager
        .enqueue(&album_url("two"), Display::default())
        .unwrap();
    (rig, manager, told)
}

async fn assert_the_queue_stopped_on_the_account(
    rig: &Rig,
    manager: &Manager,
    told: &std::sync::Mutex<Vec<Told>>,
) {
    eventually("the queue to stop on the subscription", || {
        manager.progress().halt == Some(Halt::SubscriptionLapsed)
    })
    .await;

    // Not the ten tracks, not the next album: a handful of requests, and nothing written
    let asked = rig.catalogue.server.count("/api/track/getFileUrl");
    assert!(
        (3..=5).contains(&asked),
        "{asked} files were asked for in a queue that should have stopped"
    );
    assert_eq!(music_files(rig), 0);
    // Nothing failed: both items wait, the first one is the first to start again
    for url in [album_url("one"), album_url("two")] {
        let item = item_of(manager, &url);
        assert_eq!(item.status, ItemStatus::Pending, "{url}");
        assert_eq!(item.error, None, "{url}");
    }
    assert_eq!(rig.catalogue.server.count("/api/album/get"), 1);
    assert!(
        manager
            .log_lines()
            .iter()
            .any(|line| matches!(line.event, LogEvent::SubscriptionLapsed))
    );
    eventually("the extensions to be told", || !halts_told(told).is_empty()).await;
    assert_eq!(halts_told(told), [HaltReason::SubscriptionLapsed]);
}

#[tokio::test]
async fn an_account_that_only_gets_previews_stops_the_queue_after_a_few_requests() {
    let (rig, manager, told) = refused_everywhere({
        let url = String::new();
        move || {
            Reply::json(
                json!({ "url": format!("{url}/cdn/audio"), "format_id": 6, "sample": true }),
            )
        }
    })
    .await;

    assert_the_queue_stopped_on_the_account(&rig, &manager, &told).await;
}

#[tokio::test]
async fn an_account_whose_files_are_refused_with_an_error_stops_the_queue_too() {
    let (rig, manager, told) =
        refused_everywhere(|| Reply::error(403, "Not allowed for this subscription")).await;

    assert_the_queue_stopped_on_the_account(&rig, &manager, &told).await;
}

#[tokio::test]
async fn once_the_account_can_download_again_going_on_finishes_what_was_waiting() {
    let (rig, manager, _told) =
        refused_everywhere(|| Reply::error(403, "Not allowed for this subscription")).await;
    eventually("the queue to stop", || {
        manager.progress().halt == Some(Halt::SubscriptionLapsed)
    })
    .await;

    // The subscription is renewed: files are given again
    let audio = format!("{}/cdn/audio", rig.catalogue.server.url());
    rig.catalogue
        .server
        .on("/api/track/getFileUrl", move |call| {
            Reply::json(json!({
                "url": audio,
                "format_id": call.param("format_id").parse::<u32>().unwrap(),
                "bit_depth": 16,
                "sampling_rate": 44.1,
            }))
        });
    manager.resume();

    assert_eq!(
        finished(&manager, &album_url("one")).await,
        ItemStatus::Completed
    );
    assert_eq!(
        finished(&manager, &album_url("two")).await,
        ItemStatus::Completed
    );
    assert_eq!(music_files(&rig), 11);
    assert_eq!(manager.progress().halt, None);
}

#[tokio::test]
async fn a_track_that_is_only_ever_a_preview_does_not_stop_the_queue() {
    let rig = Rig::new().await;
    rig.catalogue
        .add_album("alb", "Kind of Blue", "Miles Davis", &KIND_OF_BLUE);
    let audio = format!("{}/cdn/audio", rig.catalogue.server.url());
    rig.catalogue.server.on("/api/track/getFileUrl", move |call| {
        // One track of the three is offered as a preview, whatever the account
        let sample = call.param("track_id") == "12";
        Reply::json(json!({
            "url": audio, "format_id": 6, "bit_depth": 16, "sampling_rate": 44.1, "sample": sample,
        }))
    });
    let manager = rig.start();

    manager
        .enqueue(&album_url("alb"), Display::default())
        .unwrap();

    let status = finished(&manager, &album_url("alb")).await;
    assert_eq!(status, ItemStatus::Completed);
    let item = item_of(&manager, &album_url("alb"));
    assert_eq!((item.downloaded, item.failures.len()), (2, 1));
    assert_eq!(manager.progress().halt, None);
}

#[tokio::test]
async fn tracks_that_do_not_exist_are_not_taken_for_a_lapsed_account() {
    let (rig, manager, _told) = refused_everywhere(|| Reply::error(404, "Not found")).await;

    // Every track of both albums fails by itself, and the queue goes through them all
    assert_eq!(
        finished(&manager, &album_url("two")).await,
        ItemStatus::Completed
    );
    assert_eq!(manager.progress().halt, None);
    assert_eq!(rig.catalogue.server.count("/api/track/getFileUrl"), 11);
}

#[tokio::test]
async fn signing_out_midway_pauses_the_queue_and_a_new_sign_in_goes_on_by_itself() {
    let rig = Rig::new().await;
    rig.catalogue
        .add_album("one", "First", "Artist", &[(21, 1, "A")]);
    rig.catalogue
        .add_album("two", "Second", "Artist", &[(22, 1, "B")]);
    // The file of the first album is slow to arrive: the sign-out comes in the middle of it
    let audio = common::flac_bytes();
    rig.catalogue.server.on("/cdn/audio", move |_| {
        Reply::bytes(audio.clone()).after(Duration::from_millis(500))
    });
    let manager = rig.start();
    manager
        .enqueue(&album_url("one"), Display::default())
        .unwrap();
    manager
        .enqueue(&album_url("two"), Display::default())
        .unwrap();
    eventually("the file to be asked for", || {
        rig.catalogue.server.count("/cdn/audio") > 0
    })
    .await;

    rig.catalogue.client.end_session();

    eventually("the item to wait again", || {
        status_of(&manager, &album_url("one")) == Some(ItemStatus::Pending)
            && !manager.progress().running
    })
    .await;
    for url in [album_url("one"), album_url("two")] {
        assert_eq!(status_of(&manager, &url), Some(ItemStatus::Pending));
    }
    assert_eq!(
        manager.progress().halt,
        Some(qobuzdlx::download::Halt::SignedOut)
    );
    assert!(
        manager
            .snapshot()
            .queue
            .iter()
            .all(|item| item.error.is_none())
    );

    rig.catalogue.client.start_session(session());

    assert_eq!(
        finished(&manager, &album_url("one")).await,
        ItemStatus::Completed
    );
    assert_eq!(
        finished(&manager, &album_url("two")).await,
        ItemStatus::Completed
    );
}

#[tokio::test]
async fn a_paused_queue_keeps_its_item_and_goes_on_when_resumed() {
    let rig = Rig::new().await;
    rig.catalogue
        .add_album("one", "First", "Artist", &[(21, 1, "A")]);
    rig.catalogue
        .add_album("two", "Second", "Artist", &[(22, 1, "B")]);
    let audio = common::flac_bytes();
    rig.catalogue.server.on("/cdn/audio", move |_| {
        Reply::bytes(audio.clone()).after(Duration::from_millis(500))
    });
    let manager = rig.start();
    manager
        .enqueue(&album_url("one"), Display::default())
        .unwrap();
    manager
        .enqueue(&album_url("two"), Display::default())
        .unwrap();
    eventually("the file to be asked for", || {
        rig.catalogue.server.count("/cdn/audio") > 0
    })
    .await;

    manager.pause();

    eventually("the queue to stand still", || {
        !manager.progress().running
            && manager.progress().halt == Some(qobuzdlx::download::Halt::Paused)
    })
    .await;
    tokio::time::sleep(Duration::from_millis(300)).await;
    for url in [album_url("one"), album_url("two")] {
        assert_eq!(status_of(&manager, &url), Some(ItemStatus::Pending));
    }

    manager.resume();

    assert_eq!(
        finished(&manager, &album_url("one")).await,
        ItemStatus::Completed
    );
    assert_eq!(
        finished(&manager, &album_url("two")).await,
        ItemStatus::Completed
    );
    assert_eq!(manager.progress().halt, None);
}

#[tokio::test]
async fn something_queued_while_paused_waits_for_the_resume() {
    let rig = Rig::new().await;
    rig.catalogue
        .add_album("one", "First", "Artist", &[(21, 1, "A")]);
    let manager = rig.start();
    manager.pause();

    manager
        .enqueue(&album_url("one"), Display::default())
        .unwrap();
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert_eq!(
        status_of(&manager, &album_url("one")),
        Some(ItemStatus::Pending)
    );

    manager.resume();

    assert_eq!(
        finished(&manager, &album_url("one")).await,
        ItemStatus::Completed
    );
}

/// Every file with this extension below `folder`, and what they weigh together.
fn weight_of(folder: &std::path::Path, extension: &str) -> u64 {
    let mut total = 0;
    for entry in std::fs::read_dir(folder).unwrap().flatten() {
        let path = entry.path();
        if path.is_dir() {
            total += weight_of(&path, extension);
        } else if path.extension().is_some_and(|found| found == extension) {
            total += entry.metadata().unwrap().len();
        }
    }
    total
}

#[tokio::test]
async fn an_item_keeps_what_the_files_it_wrote_weigh() {
    let rig = Rig::new().await;
    rig.catalogue
        .add_album("one", "First", "Artist", &[(21, 1, "A"), (22, 2, "B")]);
    let manager = rig.start();
    manager
        .enqueue(&album_url("one"), Display::default())
        .unwrap();
    finished(&manager, &album_url("one")).await;

    let item = item_of(&manager, &album_url("one"));

    assert_eq!(item.downloaded, 2);
    assert!(item.bytes > 0);
    assert_eq!(item.bytes, weight_of(rig.music.path(), "flac"));
}

#[tokio::test]
async fn a_pause_survives_a_restart_until_it_is_lifted() {
    let rig = Rig::new().await;
    rig.catalogue
        .add_album("one", "First", "Artist", &[(21, 1, "A")]);
    let manager = rig.start();
    manager.pause();
    manager
        .enqueue(&album_url("one"), Display::default())
        .unwrap();
    manager.shutdown();
    tokio::time::sleep(Duration::from_millis(300)).await;

    // The same folder, a new process: still paused, and it says so
    let restarted = rig.start();
    assert_eq!(restarted.progress().halt, Some(Halt::Paused));
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert_eq!(
        status_of(&restarted, &album_url("one")),
        Some(ItemStatus::Pending),
        "nothing starts while paused"
    );

    restarted.resume();
    assert_eq!(
        finished(&restarted, &album_url("one")).await,
        ItemStatus::Completed
    );
    restarted.shutdown();
    tokio::time::sleep(Duration::from_millis(300)).await;

    // Lifted: the next start is not paused
    let again = rig.start();
    assert_eq!(again.progress().halt, None);
}

// ─── Not going too fast ───

fn pace(rig: &Rig, change: impl FnOnce(&mut RateLimit)) {
    rig.settings
        .update(|settings| change(&mut settings.rate_limit))
        .unwrap();
}

fn waiting_for(manager: &Manager) -> Option<WaitReason> {
    manager.progress().wait.map(|wait| wait.reason)
}

#[tokio::test]
async fn the_queue_waits_between_two_albums_and_says_so() {
    let rig = Rig::new().await;
    rig.catalogue
        .add_album("one", "First", "Artist", &[(21, 1, "A")]);
    rig.catalogue
        .add_album("two", "Second", "Artist", &[(22, 1, "B")]);
    pace(&rig, |limits| limits.between_albums = Delay::new(800, 800));
    let manager = rig.start();
    manager
        .enqueue(&album_url("one"), Display::default())
        .unwrap();
    manager
        .enqueue(&album_url("two"), Display::default())
        .unwrap();

    eventually("the pause between the albums", || {
        waiting_for(&manager) == Some(WaitReason::BetweenAlbums)
    })
    .await;
    assert_eq!(
        status_of(&manager, &album_url("one")),
        Some(ItemStatus::Completed)
    );
    assert_eq!(
        status_of(&manager, &album_url("two")),
        Some(ItemStatus::Pending)
    );
    let started = std::time::Instant::now();

    assert_eq!(
        finished(&manager, &album_url("two")).await,
        ItemStatus::Completed
    );
    assert!(started.elapsed() >= Duration::from_millis(500));
    assert_eq!(waiting_for(&manager), None);
}

#[tokio::test]
async fn pausing_cuts_the_wait_short() {
    let rig = Rig::new().await;
    rig.catalogue
        .add_album("one", "First", "Artist", &[(21, 1, "A")]);
    rig.catalogue
        .add_album("two", "Second", "Artist", &[(22, 1, "B")]);
    pace(&rig, |limits| {
        limits.between_albums = Delay::new(60_000, 60_000);
    });
    let manager = rig.start();
    manager
        .enqueue(&album_url("one"), Display::default())
        .unwrap();
    manager
        .enqueue(&album_url("two"), Display::default())
        .unwrap();
    eventually("the pause between the albums", || {
        waiting_for(&manager).is_some()
    })
    .await;

    manager.pause();

    eventually("the wait to end", || waiting_for(&manager).is_none()).await;
    assert_eq!(manager.progress().halt, Some(Halt::Paused));
    // Going on does not start the wait again: only an item that ends makes one
    manager.resume();
    assert_eq!(
        finished(&manager, &album_url("two")).await,
        ItemStatus::Completed
    );
}

#[tokio::test]
async fn a_long_break_is_taken_once_the_albums_written_reach_the_limit() {
    let rig = Rig::new().await;
    for id in ["one", "two", "three"] {
        rig.catalogue
            .add_album(id, id, "Artist", &[(u64::from(id.as_bytes()[0]), 1, "A")]);
    }
    pace(&rig, |limits| {
        limits.break_after_albums = 2;
        limits.break_length = Delay::new(600, 600);
    });
    let manager = rig.start();
    for id in ["one", "two", "three"] {
        manager.enqueue(&album_url(id), Display::default()).unwrap();
    }

    eventually("the break", || {
        waiting_for(&manager) == Some(WaitReason::Break)
    })
    .await;
    assert_eq!(
        status_of(&manager, &album_url("two")),
        Some(ItemStatus::Completed)
    );
    assert_eq!(
        status_of(&manager, &album_url("three")),
        Some(ItemStatus::Pending)
    );
    assert_eq!(
        finished(&manager, &album_url("three")).await,
        ItemStatus::Completed
    );
}

#[tokio::test]
async fn a_long_break_is_also_due_when_enough_tracks_were_written() {
    let rig = Rig::new().await;
    rig.catalogue.add_album(
        "one",
        "One",
        "Artist",
        &[(31, 1, "A"), (32, 2, "B"), (33, 3, "C")],
    );
    rig.catalogue
        .add_album("two", "Two", "Artist", &[(34, 1, "D")]);
    pace(&rig, |limits| {
        limits.break_after_tracks = 3;
        limits.break_length = Delay::new(600, 600);
    });
    let manager = rig.start();
    manager
        .enqueue(&album_url("one"), Display::default())
        .unwrap();
    manager
        .enqueue(&album_url("two"), Display::default())
        .unwrap();

    // One album only, but three tracks: the break comes before the next album
    eventually("the break", || {
        waiting_for(&manager) == Some(WaitReason::Break)
    })
    .await;
    assert_eq!(
        status_of(&manager, &album_url("two")),
        Some(ItemStatus::Pending)
    );
    assert_eq!(
        finished(&manager, &album_url("two")).await,
        ItemStatus::Completed
    );
}

#[tokio::test]
async fn a_long_break_is_taken_between_two_albums_of_the_same_artist() {
    let rig = Rig::new().await;
    rig.catalogue
        .add_album("a1", "First", "Artist", &[(51, 1, "A")]);
    rig.catalogue
        .add_album("a2", "Second", "Artist", &[(52, 1, "B")]);
    rig.catalogue.artists.lock().unwrap().insert(
        7,
        json!({ "id": 7, "name": "Artist", "albums": page(vec![json!({"id": "a1", "title": "First", "artist": {"id": 7}}), json!({"id": "a2", "title": "Second", "artist": {"id": 7}})]) }),
    );
    pace(&rig, |limits| {
        limits.break_after_albums = 1;
        limits.break_length = Delay::new(600, 600);
    });
    let manager = rig.start();
    let url = "https://play.qobuz.com/artist/7";
    manager.enqueue(url, Display::default()).unwrap();

    // Nothing else is waiting in the queue: the break is the one inside the item
    eventually("the break", || {
        waiting_for(&manager) == Some(WaitReason::Break)
    })
    .await;
    assert_eq!(status_of(&manager, url), Some(ItemStatus::Running));
    assert_eq!(finished(&manager, url).await, ItemStatus::Completed);
}

fn subscription_answer(rig: &Rig, lossless: bool, hires: bool) {
    rig.catalogue.server.on_json(
        "/api/user/get",
        json!({
            "id": 7,
            "credential": { "parameters": { "lossless_streaming": lossless, "hires_streaming": hires } }
        }),
    );
}

#[tokio::test]
async fn the_account_is_checked_at_a_long_break_and_a_lapsed_one_stops_the_queue() {
    let rig = Rig::new().await;
    for id in ["one", "two", "three"] {
        rig.catalogue
            .add_album(id, id, "Artist", &[(u64::from(id.as_bytes()[0]), 1, "A")]);
    }
    subscription_answer(&rig, false, false);
    pace(&rig, |limits| {
        limits.break_after_albums = 1;
        limits.break_length = Delay::new(600, 600);
    });
    let manager = rig.start();
    for id in ["one", "two", "three"] {
        manager.enqueue(&album_url(id), Display::default()).unwrap();
    }

    eventually("the queue to stop", || {
        manager.progress().halt == Some(Halt::SubscriptionLapsed)
    })
    .await;
    // The break was due after the first album: the second never started
    assert_eq!(
        status_of(&manager, &album_url("one")),
        Some(ItemStatus::Completed)
    );
    for id in ["two", "three"] {
        assert_eq!(
            status_of(&manager, &album_url(id)),
            Some(ItemStatus::Pending),
            "{id}"
        );
    }
    assert_eq!(rig.catalogue.server.count("/api/user/get"), 1);
    assert_eq!(rig.catalogue.server.count("/api/album/get"), 1);
}

#[tokio::test]
async fn a_lapsed_account_is_found_in_the_break_inside_an_item_too() {
    let rig = Rig::new().await;
    rig.catalogue
        .add_album("a1", "First", "Artist", &[(51, 1, "A")]);
    rig.catalogue
        .add_album("a2", "Second", "Artist", &[(52, 1, "B")]);
    rig.catalogue.artists.lock().unwrap().insert(
        7,
        json!({ "id": 7, "name": "Artist", "albums": page(vec![json!({"id": "a1", "title": "First", "artist": {"id": 7}}), json!({"id": "a2", "title": "Second", "artist": {"id": 7}})]) }),
    );
    subscription_answer(&rig, false, false);
    pace(&rig, |limits| {
        limits.break_after_albums = 1;
        limits.break_length = Delay::new(600, 600);
    });
    let manager = rig.start();
    let url = "https://play.qobuz.com/artist/7";
    manager.enqueue(url, Display::default()).unwrap();

    eventually("the queue to stop", || {
        manager.progress().halt == Some(Halt::SubscriptionLapsed)
    })
    .await;
    // The item waits again, from the start, and nothing about it failed
    let item = item_of(&manager, url);
    assert_eq!(item.status, ItemStatus::Pending);
    assert_eq!(item.error, None);
}

#[tokio::test]
async fn an_account_that_qobuz_does_not_describe_never_stops_the_queue() {
    let rig = Rig::new().await;
    for id in ["one", "two"] {
        rig.catalogue
            .add_album(id, id, "Artist", &[(u64::from(id.as_bytes()[0]), 1, "A")]);
    }
    // No credential at all, as a test server that knows nothing of plans answers
    rig.catalogue
        .server
        .on_json("/api/user/get", json!({ "id": 7 }));
    pace(&rig, |limits| {
        limits.break_after_albums = 1;
        limits.break_length = Delay::new(20, 20);
    });
    let manager = rig.start();
    for id in ["one", "two"] {
        manager.enqueue(&album_url(id), Display::default()).unwrap();
    }

    assert_eq!(
        finished(&manager, &album_url("two")).await,
        ItemStatus::Completed
    );
    assert_eq!(manager.progress().halt, None);
    assert!(rig.catalogue.server.count("/api/user/get") >= 1);
}

// ─── The daily limit ───

fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

/// What was downloaded earlier today, as the server found it in `usage.json` when it started.
fn written_before(rig: &Rig, entries: &[(u64, u64)]) {
    let entries: Vec<_> = entries
        .iter()
        .map(|(at, bytes)| json!({ "at": at, "bytes": bytes }))
        .collect();
    std::fs::write(
        rig.config.path().join("usage.json"),
        serde_json::to_string(&entries).unwrap(),
    )
    .unwrap();
}

const GIGABYTE: u64 = 1_000_000_000;

#[tokio::test]
async fn nothing_is_started_while_the_daily_limit_is_reached() {
    let rig = Rig::new().await;
    rig.catalogue
        .add_album("one", "First", "Artist", &[(21, 1, "A")]);
    pace(&rig, |limits| limits.daily_limit_gb = 50);
    written_before(&rig, &[(unix_now() - 100, 60 * GIGABYTE)]);
    let manager = rig.start();

    manager
        .enqueue(&album_url("one"), Display::default())
        .unwrap();

    eventually("the wait for the day to go by", || {
        waiting_for(&manager) == Some(WaitReason::DailyLimit)
    })
    .await;
    assert_eq!(
        item_of(&manager, &album_url("one")).status,
        ItemStatus::Pending
    );
    assert_eq!(rig.catalogue.server.count("/api/album/get"), 0);
    let until = manager.progress().wait.unwrap().until;
    assert!(
        until > unix_now() + 80_000,
        "the wait ends when the day is over, not soon"
    );
    assert!(
        manager
            .log_lines()
            .iter()
            .any(|line| matches!(line.event, LogEvent::DailyLimitReached { .. }))
    );
}

#[tokio::test]
async fn the_queue_goes_on_by_itself_when_enough_of_the_day_has_gone_by() {
    let rig = Rig::new().await;
    rig.catalogue
        .add_album("one", "First", "Artist", &[(21, 1, "A")]);
    pace(&rig, |limits| limits.daily_limit_gb = 50);
    // The 60 GB leave the 24 hours window in two seconds
    written_before(&rig, &[(unix_now() - 86_400 + 2, 60 * GIGABYTE)]);
    let manager = rig.start();

    manager
        .enqueue(&album_url("one"), Display::default())
        .unwrap();

    eventually("the wait", || {
        waiting_for(&manager) == Some(WaitReason::DailyLimit)
    })
    .await;
    assert_eq!(
        finished(&manager, &album_url("one")).await,
        ItemStatus::Completed
    );
    assert_eq!(waiting_for(&manager), None);
}

#[tokio::test]
async fn without_a_daily_limit_what_was_written_does_not_matter() {
    let rig = Rig::new().await;
    rig.catalogue
        .add_album("one", "First", "Artist", &[(21, 1, "A")]);
    pace(&rig, |limits| limits.daily_limit_gb = 0);
    written_before(&rig, &[(unix_now() - 100, 900 * GIGABYTE)]);
    let manager = rig.start();

    manager
        .enqueue(&album_url("one"), Display::default())
        .unwrap();

    assert_eq!(
        finished(&manager, &album_url("one")).await,
        ItemStatus::Completed
    );
}

#[tokio::test]
async fn the_limit_is_reached_in_the_middle_of_an_album_and_the_item_waits_there() {
    let rig = Rig::new().await;
    rig.catalogue.add_album(
        "alb",
        "Three",
        "Artist",
        &[(31, 1, "One"), (32, 2, "Two"), (33, 3, "Three")],
    );
    pace(&rig, |limits| limits.daily_limit_gb = 50);
    rig.settings
        .update(|settings| settings.advanced.parallel_tracks = 1)
        .unwrap();
    // One byte under the limit: the first track still starts, and brings it over
    written_before(&rig, &[(unix_now() - 100, 50 * GIGABYTE - 1)]);
    let manager = rig.start();

    manager
        .enqueue(&album_url("alb"), Display::default())
        .unwrap();

    eventually("the wait", || {
        waiting_for(&manager) == Some(WaitReason::DailyLimit)
    })
    .await;
    assert_eq!(music_files(&rig), 1, "one track written, the others wait");
    assert_eq!(
        item_of(&manager, &album_url("alb")).status,
        ItemStatus::Running
    );
}

#[tokio::test]
async fn pausing_ends_the_wait_for_the_day_and_keeps_what_is_waiting() {
    let rig = Rig::new().await;
    rig.catalogue
        .add_album("one", "First", "Artist", &[(21, 1, "A")]);
    pace(&rig, |limits| limits.daily_limit_gb = 50);
    written_before(&rig, &[(unix_now() - 100, 60 * GIGABYTE)]);
    let manager = rig.start();
    manager
        .enqueue(&album_url("one"), Display::default())
        .unwrap();
    eventually("the wait", || {
        waiting_for(&manager) == Some(WaitReason::DailyLimit)
    })
    .await;

    manager.pause();

    eventually("the pause", || {
        manager.progress().halt == Some(Halt::Paused)
    })
    .await;
    assert_eq!(waiting_for(&manager), None);
    assert_eq!(
        item_of(&manager, &album_url("one")).status,
        ItemStatus::Pending
    );
}

#[tokio::test]
async fn an_album_that_found_everything_in_place_only_waits_the_shortest_time() {
    let rig = Rig::new().await;
    rig.catalogue
        .add_album("one", "First", "Artist", &[(21, 1, "A")]);
    rig.catalogue
        .add_album("two", "Second", "Artist", &[(22, 1, "B")]);
    let manager = rig.start();
    manager
        .enqueue(&album_url("one"), Display::default())
        .unwrap();
    assert_eq!(
        finished(&manager, &album_url("one")).await,
        ItemStatus::Completed
    );
    // The same album again finds its file: it costs one request, and the wait that follows is the
    // shortest the bounds allow
    pace(&rig, |limits| {
        limits.between_albums = Delay::new(10, 60_000);
    });
    manager
        .enqueue(&album_url("one"), Display::default())
        .unwrap();
    manager
        .enqueue(&album_url("two"), Display::default())
        .unwrap();

    assert_eq!(
        finished(&manager, &album_url("two")).await,
        ItemStatus::Completed
    );
}

#[tokio::test]
async fn when_qobuz_says_to_slow_down_the_item_is_not_lost() {
    let rig = Rig::new().await;
    rig.catalogue
        .add_album("one", "First", "Artist", &[(21, 1, "A")]);
    pace(&rig, |limits| limits.cooldown_ms = 300);
    // Qobuz refuses the first request of the album, and every retry the client makes of it
    let refused = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let albums = rig.catalogue.albums.clone();
    rig.catalogue.server.on("/api/album/get", {
        let refused = refused.clone();
        move |call| {
            if refused.fetch_add(1, std::sync::atomic::Ordering::SeqCst) < 3 {
                return Reply::error(429, "Too many requests");
            }
            let key = call.param("album_id").to_owned();
            Reply::json(albums.lock().unwrap().get(&key).unwrap().clone())
        }
    });
    let manager = rig.start();

    manager
        .enqueue(&album_url("one"), Display::default())
        .unwrap();

    eventually("the cooldown", || {
        waiting_for(&manager) == Some(WaitReason::RateLimited)
    })
    .await;
    let item = item_of(&manager, &album_url("one"));
    assert_eq!(item.status, ItemStatus::Pending);
    assert_eq!(item.error, None);
    assert_eq!(
        finished(&manager, &album_url("one")).await,
        ItemStatus::Completed
    );
}

#[tokio::test]
async fn when_qobuz_cannot_be_reached_nothing_is_failed_and_the_queue_waits() {
    let rig = Rig::new().await;
    rig.catalogue
        .add_album("one", "First", "Artist", &[(21, 1, "A")]);
    rig.catalogue
        .add_album("two", "Second", "Artist", &[(22, 1, "B")]);
    pace(&rig, |limits| limits.cooldown_ms = 300);
    // The gateway in front of Qobuz fails for the three attempts of the client, then answers
    let refused = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let albums = rig.catalogue.albums.clone();
    rig.catalogue.server.on("/api/album/get", {
        let refused = refused.clone();
        move |call| {
            if refused.fetch_add(1, std::sync::atomic::Ordering::SeqCst) < 3 {
                return Reply::status(502);
            }
            let key = call.param("album_id").to_owned();
            Reply::json(albums.lock().unwrap().get(&key).unwrap().clone())
        }
    });
    let manager = rig.start();

    manager
        .enqueue(&album_url("one"), Display::default())
        .unwrap();
    manager
        .enqueue(&album_url("two"), Display::default())
        .unwrap();

    eventually("the wait for Qobuz", || {
        waiting_for(&manager) == Some(WaitReason::Unreachable)
    })
    .await;
    for url in [album_url("one"), album_url("two")] {
        let item = item_of(&manager, &url);
        assert_eq!(item.status, ItemStatus::Pending, "{url}");
        assert_eq!(item.error, None, "{url}");
    }
    assert!(
        manager
            .log_lines()
            .iter()
            .any(|line| matches!(line.event, LogEvent::QobuzUnreachable { .. }))
    );

    assert_eq!(
        finished(&manager, &album_url("one")).await,
        ItemStatus::Completed
    );
    assert_eq!(
        finished(&manager, &album_url("two")).await,
        ItemStatus::Completed
    );
}

#[tokio::test]
async fn a_network_that_qobuz_refuses_stops_the_queue_instead_of_waiting_for_it() {
    let rig = Rig::new().await;
    rig.catalogue
        .add_album("one", "First", "Artist", &[(21, 1, "A")]);
    rig.catalogue
        .add_album("two", "Second", "Artist", &[(22, 1, "B")]);
    let blocked = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true));
    let albums = rig.catalogue.albums.clone();
    rig.catalogue.server.on("/api/album/get", {
        let blocked = blocked.clone();
        move |call| {
            if blocked.load(std::sync::atomic::Ordering::SeqCst) {
                return Reply::text("<HTML><TITLE>Access Denied</TITLE></HTML>").with_status(403);
            }
            let key = call.param("album_id").to_owned();
            Reply::json(albums.lock().unwrap().get(&key).unwrap().clone())
        }
    });
    let told = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let extensions = Extensions::start(vec![std::sync::Arc::new(Recorder(told.clone()))], vec![]);
    let manager = rig.start_with(extensions);

    manager
        .enqueue(&album_url("one"), Display::default())
        .unwrap();
    manager
        .enqueue(&album_url("two"), Display::default())
        .unwrap();

    eventually("the queue to stop", || {
        manager.progress().halt == Some(Halt::Blocked)
    })
    .await;
    for url in [album_url("one"), album_url("two")] {
        let item = item_of(&manager, &url);
        assert_eq!(item.status, ItemStatus::Pending, "{url}");
        assert_eq!(item.error, None, "{url}");
    }
    assert_eq!(
        waiting_for(&manager),
        None,
        "it is not waiting to try again"
    );
    eventually("the extensions to be told", || {
        !halts_told(&told).is_empty()
    })
    .await;
    assert_eq!(halts_told(&told), [HaltReason::Blocked]);

    // Another network: the queue goes on
    blocked.store(false, std::sync::atomic::Ordering::SeqCst);
    manager.resume();
    assert_eq!(
        finished(&manager, &album_url("one")).await,
        ItemStatus::Completed
    );
    assert_eq!(
        finished(&manager, &album_url("two")).await,
        ItemStatus::Completed
    );
}

#[tokio::test]
async fn a_gateway_error_that_outlasts_the_retries_is_waited_out_too() {
    let rig = Rig::new().await;
    rig.catalogue
        .add_album("one", "First", "Artist", &[(21, 1, "A")]);
    pace(&rig, |limits| limits.cooldown_ms = 300);
    // Three attempts of the client meet a 503, the next request is served
    let refused = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let albums = rig.catalogue.albums.clone();
    rig.catalogue.server.on("/api/album/get", {
        let refused = refused.clone();
        move |call| {
            if refused.fetch_add(1, std::sync::atomic::Ordering::SeqCst) < 3 {
                return Reply::status(503);
            }
            let key = call.param("album_id").to_owned();
            Reply::json(albums.lock().unwrap().get(&key).unwrap().clone())
        }
    });
    let manager = rig.start();

    manager
        .enqueue(&album_url("one"), Display::default())
        .unwrap();

    eventually("the wait for Qobuz", || {
        waiting_for(&manager) == Some(WaitReason::Unreachable)
    })
    .await;
    assert_eq!(
        item_of(&manager, &album_url("one")).status,
        ItemStatus::Pending
    );
    assert_eq!(
        finished(&manager, &album_url("one")).await,
        ItemStatus::Completed
    );
}

// ─── Waiting and resuming ───

#[tokio::test]
async fn what_is_asked_without_a_session_waits_for_one() {
    let rig = Rig::new().await;
    rig.catalogue
        .add_album("alb", "Kind of Blue", "Miles Davis", &KIND_OF_BLUE[..1]);
    rig.catalogue.client.end_session();
    let manager = rig.start();
    manager
        .enqueue(&album_url("alb"), Display::default())
        .unwrap();
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert_eq!(
        status_of(&manager, &album_url("alb")),
        Some(ItemStatus::Pending)
    );

    rig.catalogue.client.start_session(session());

    assert_eq!(
        finished(&manager, &album_url("alb")).await,
        ItemStatus::Completed
    );
}

#[tokio::test]
async fn a_restored_queue_is_resumed_once_signed_in() {
    let rig = Rig::new().await;
    rig.catalogue
        .add_album("alb", "Kind of Blue", "Miles Davis", &KIND_OF_BLUE[..1]);
    // What a previous run left: one item that was running when the process stopped
    let mut left = Queue::default();
    left.enqueue(
        &album_url("alb"),
        ItemKind::Album,
        "alb",
        Display::default(),
    );
    left.start_next();
    QueueStore::new(rig.config.path()).save(&left).unwrap();

    let manager = rig.start();

    assert_eq!(
        finished(&manager, &album_url("alb")).await,
        ItemStatus::Completed
    );
    assert!(
        manager
            .log_lines()
            .iter()
            .any(|line| matches!(line.event, LogEvent::QueueRestored { waiting: 1 }))
    );
}

#[tokio::test]
async fn partial_files_of_a_killed_run_are_removed_at_start() {
    let rig = Rig::new().await;
    // Next to the music, and where earlier versions kept them
    let folders = [
        rig.music.path().join(".qobuzdlx-partial"),
        rig.config.path().join("qbdlx-temp"),
    ];
    for folder in &folders {
        std::fs::create_dir_all(folder).unwrap();
        std::fs::write(folder.join("qbdlx_downloading-42.flac"), b"half").unwrap();
        std::fs::write(folder.join("keep-me.txt"), b"not ours").unwrap();
    }

    let _manager = rig.start();

    for folder in &folders {
        assert!(!folder.join("qbdlx_downloading-42.flac").exists());
        assert!(folder.join("keep-me.txt").exists());
    }
}

#[tokio::test]
async fn a_track_in_progress_is_written_next_to_the_music_and_leaves_nothing_behind() {
    let rig = Rig::new().await;
    rig.catalogue
        .add_album("alb", "Kind of Blue", "Miles Davis", &KIND_OF_BLUE);
    let manager = rig.start();

    manager
        .enqueue(&album_url("alb"), Display::default())
        .unwrap();

    assert_eq!(
        finished(&manager, &album_url("alb")).await,
        ItemStatus::Completed
    );
    let partial = rig.music.path().join(".qobuzdlx-partial");
    assert!(partial.is_dir(), "on the volume of the music");
    assert_eq!(std::fs::read_dir(&partial).unwrap().count(), 0);
    assert!(
        !rig.config.path().join("qbdlx-temp").exists(),
        "nothing goes through the configuration folder any more"
    );
}

// ─── Retrying ───

#[tokio::test]
async fn retrying_the_failures_downloads_only_those_tracks() {
    let rig = Rig::new().await;
    rig.catalogue
        .add_album("alb", "Kind of Blue", "Miles Davis", &KIND_OF_BLUE);
    let good = format!("{}/cdn/audio", rig.catalogue.server.url());
    let broken = good.clone();
    rig.catalogue.server.on("/api/track/getFileUrl", move |call| {
        if call.param("track_id") == "12" {
            Reply::error(403, "Track restricted")
        } else {
            Reply::json(json!({ "url": broken, "format_id": 6, "bit_depth": 16, "sampling_rate": 44.1 }))
        }
    });
    let manager = rig.start();
    manager
        .enqueue(&album_url("alb"), Display::default())
        .unwrap();
    finished(&manager, &album_url("alb")).await;
    let item = manager.snapshot().queue[0].clone();
    assert_eq!((item.downloaded, item.failures.len()), (2, 1));

    // The track is available again
    rig.catalogue.server.on("/api/track/getFileUrl", move |_| {
        Reply::json(json!({ "url": good, "format_id": 6, "bit_depth": 16, "sampling_rate": 44.1 }))
    });
    manager.retry_failures(&item.id);
    eventually("the retry to start", || {
        status_of(&manager, &album_url("alb")) != Some(ItemStatus::Completed)
    })
    .await;
    finished(&manager, &album_url("alb")).await;

    let item = manager.snapshot().queue[0].clone();
    assert_eq!((item.downloaded, item.failures.len()), (3, 0));
    assert!(
        rig.file("Miles Davis/Kind of Blue/2 - Freddie Freeloader.flac")
            .exists()
    );
    assert_eq!(rig.catalogue.server.count("/api/album/get"), 2);
}

// ─── Telling the interface ───

#[tokio::test]
async fn the_interface_is_told_about_the_queue_the_progress_and_the_log() {
    let rig = Rig::new().await;
    rig.catalogue
        .add_album("alb", "Kind of Blue", "Miles Davis", &KIND_OF_BLUE[..1]);
    let manager = rig.start();
    let mut events = manager.subscribe();

    manager
        .enqueue(&album_url("alb"), Display::default())
        .unwrap();
    finished(&manager, &album_url("alb")).await;

    let mut seen = (false, false, false);
    while let Ok(event) = events.try_recv() {
        match event {
            Event::Queue | Event::Item(_) => seen.0 = true,
            Event::Progress => seen.1 = true,
            Event::Log(_) => seen.2 = true,
            Event::SessionExpired => {}
        }
    }
    assert_eq!(seen, (true, true, true));
}

#[tokio::test]
async fn the_queue_survives_on_disk_in_the_order_it_has() {
    let rig = Rig::new().await;
    rig.catalogue.client.end_session();
    let manager = rig.start();
    manager
        .enqueue(&album_url("a"), Display::default())
        .unwrap();
    manager
        .enqueue(&album_url("b"), Display::default())
        .unwrap();

    // The queue is written a moment after it changes, once for a burst of changes
    let store = QueueStore::new(rig.config.path());
    eventually("the queue to be written", || {
        store.load().items().len() == 2
    })
    .await;
    let saved: Vec<QueueItem> = store.load().items().to_vec();

    assert_eq!(
        saved
            .iter()
            .map(|item| item.url.as_str())
            .collect::<Vec<_>>(),
        [album_url("a"), album_url("b")]
    );
}

// ─── Items queued with nothing but their address ───

fn item_of(manager: &Manager, url: &str) -> QueueItem {
    manager
        .snapshot()
        .queue
        .into_iter()
        .find(|item| item.url == url)
        .unwrap()
}

#[tokio::test]
async fn an_album_queued_by_its_address_takes_its_name_when_it_starts() {
    let rig = Rig::new().await;
    rig.catalogue
        .add_album("alb", "Kind of Blue", "Miles Davis", &KIND_OF_BLUE);
    let manager = rig.start();

    manager
        .enqueue(&album_url("alb"), Display::default())
        .unwrap();
    finished(&manager, &album_url("alb")).await;

    let item = item_of(&manager, &album_url("alb"));
    assert_eq!(item.label, "Kind of Blue");
    assert_eq!(item.sub_label.as_deref(), Some("Miles Davis"));
}

#[tokio::test]
async fn a_playlist_and_a_track_queued_by_their_address_take_their_name_too() {
    let rig = Rig::new().await;
    rig.catalogue
        .add_album("alb", "Kind of Blue", "Miles Davis", &KIND_OF_BLUE);
    rig.catalogue
        .add_playlist(77, "Evening Concert", &[(11, 1), (12, 2)]);
    let manager = rig.start();
    let playlist = "https://play.qobuz.com/playlist/77";
    let track = "https://play.qobuz.com/track/13";

    manager.enqueue(playlist, Display::default()).unwrap();
    manager.enqueue(track, Display::default()).unwrap();
    finished(&manager, playlist).await;
    finished(&manager, track).await;

    assert_eq!(item_of(&manager, playlist).label, "Evening Concert");
    assert_eq!(item_of(&manager, track).label, "Blue in Green");
}

#[tokio::test]
async fn a_name_the_interface_gave_is_kept() {
    let rig = Rig::new().await;
    rig.catalogue
        .add_album("alb", "Kind of Blue", "Miles Davis", &KIND_OF_BLUE);
    let manager = rig.start();

    manager
        .enqueue(
            &album_url("alb"),
            Display {
                label: Some("My choice".into()),
                ..Display::default()
            },
        )
        .unwrap();
    finished(&manager, &album_url("alb")).await;

    assert_eq!(item_of(&manager, &album_url("alb")).label, "My choice");
}
