mod common;

use std::time::Duration;

use common::catalogue::Catalogue;
use common::{Reply, session};
use qobuzdlx::download::{
    Display, Enqueued, Event, ItemKind, ItemStatus, LogEvent, Manager, ManagerConfig, Queue,
    QueueItem, QueueStore, Tuning,
};
use qobuzdlx::settings::{Quality, SettingsStore};
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
            })
            .unwrap();
        rig
    }

    fn start(&self) -> Manager {
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
        manager.snapshot().progress.track.is_some()
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
        manager.snapshot().progress.track.is_some()
    })
    .await;

    manager.shutdown();
    tokio::time::sleep(Duration::from_millis(300)).await;

    let restored = QueueStore::new(rig.config.path()).load();
    assert_eq!(restored.items().len(), 1);
    assert_eq!(restored.items()[0].status, ItemStatus::Pending);
}

#[tokio::test]
async fn a_refused_session_fails_the_item_and_leaves_the_rest_waiting() {
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

    assert_eq!(
        finished(&manager, &album_url("one")).await,
        ItemStatus::Failed
    );
    let item = manager.snapshot().queue[0].clone();
    assert!(item.error.unwrap().contains("session"));
    eventually("the session to be forgotten", || {
        !rig.catalogue.client.is_signed_in()
    })
    .await;
    assert_eq!(
        status_of(&manager, &album_url("two")),
        Some(ItemStatus::Pending)
    );
    let mut announced = false;
    while let Ok(event) = events.try_recv() {
        announced |= matches!(event, Event::SessionExpired);
    }
    assert!(announced, "the interface is told to ask for a new sign-in");
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
    let temp = rig.config.path().join("qbdlx-temp");
    std::fs::create_dir_all(&temp).unwrap();
    std::fs::write(temp.join("qbdlx_downloading-42.flac"), b"half").unwrap();
    std::fs::write(temp.join("keep-me.txt"), b"not ours").unwrap();

    let _manager = rig.start();

    assert!(!temp.join("qbdlx_downloading-42.flac").exists());
    assert!(temp.join("keep-me.txt").exists());
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
