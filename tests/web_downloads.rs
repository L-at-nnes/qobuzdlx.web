mod common;

use std::time::Duration;

use common::app::{Browser, TestApp};
use serde_json::{Value, json};

const KIND_OF_BLUE: [(u64, u32, &str); 2] = [(11, 1, "So What"), (12, 2, "Freddie Freeloader")];

fn album_url(id: &str) -> String {
    format!("https://play.qobuz.com/album/{id}")
}

fn add_one(app: &TestApp) {
    app.catalogue
        .add_album("alb", "Kind of Blue", "Miles Davis", &KIND_OF_BLUE);
}

async fn eventually(what: &str, mut condition: impl AsyncFnMut() -> bool) {
    for _ in 0..300 {
        if condition().await {
            return;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    panic!("timed out waiting for: {what}");
}

async fn queue_of(browser: &Browser) -> Vec<Value> {
    browser.get("/api/downloads").await.json()["queue"]
        .as_array()
        .unwrap()
        .clone()
}

async fn finished(browser: &Browser) {
    eventually("the queue to finish", async || {
        let queue = queue_of(browser).await;
        !queue.is_empty()
            && queue
                .iter()
                .all(|item| item["status"] != "pending" && item["status"] != "running")
    })
    .await;
}

#[tokio::test]
async fn an_album_asked_for_is_downloaded() {
    let app = TestApp::start().await;
    add_one(&app);
    let browser = app.browser();

    let answer = browser
        .post(
            "/api/downloads",
            json!({ "items": [{
                "url": album_url("alb"), "label": "Kind of Blue", "sub_label": "Miles Davis",
                "thumbnail": "https://static.example/t.jpg",
            }] }),
        )
        .await;

    assert_eq!(answer.status, 200);
    assert_eq!(
        answer.json(),
        json!({ "added": 1, "requeued": 0, "duplicates": 0, "invalid": 0 })
    );
    finished(&browser).await;
    let queue = queue_of(&browser).await;
    assert_eq!(queue[0]["status"], "completed");
    assert_eq!(queue[0]["label"], "Kind of Blue");
    assert_eq!(queue[0]["sub_label"], "Miles Davis");
    assert_eq!(queue[0]["downloaded"], 2);
    assert!(
        app.music
            .path()
            .join("Miles Davis/Kind of Blue/1 - So What.flac")
            .exists()
    );
}

#[tokio::test]
async fn several_addresses_are_counted_by_what_became_of_them() {
    let app = TestApp::start().await;
    add_one(&app);
    app.state.client.end_session();
    let browser = app.browser();

    let answer = browser
        .post(
            "/api/downloads",
            json!({ "items": [
                { "url": album_url("alb") },
                { "url": "https://www.qobuz.com/fr-fr/album/kind-of-blue/alb?utm=x" },
                { "url": "https://example.com/not-qobuz" },
                { "url": album_url("other") },
            ] }),
        )
        .await;

    assert_eq!(
        answer.json(),
        json!({ "added": 2, "requeued": 0, "duplicates": 1, "invalid": 1 })
    );
    assert_eq!(queue_of(&browser).await.len(), 2);
}

#[tokio::test]
async fn an_empty_request_is_refused() {
    let app = TestApp::start().await;

    let answer = app
        .browser()
        .post("/api/downloads", json!({ "items": [] }))
        .await;

    assert_eq!(answer.status, 400);
    assert_eq!(answer.json()["error"], "nothing_to_add");
}

#[tokio::test]
async fn the_queue_tells_how_many_are_left() {
    let app = TestApp::start().await;
    app.state.client.end_session();
    let browser = app.browser();
    browser
        .post(
            "/api/downloads",
            json!({ "items": [{ "url": album_url("a") }, { "url": album_url("b") }] }),
        )
        .await;

    let snapshot = browser.get("/api/downloads").await.json();

    assert_eq!(snapshot["remaining"], 2);
    assert_eq!(snapshot["progress"]["running"], false);
}

#[tokio::test]
async fn waiting_items_can_be_moved_removed_and_cleared() {
    let app = TestApp::start().await;
    app.state.client.end_session();
    let browser = app.browser();
    browser
        .post(
            "/api/downloads",
            json!({ "items": [{ "url": album_url("a") }, { "url": album_url("b") }, { "url": album_url("c") }] }),
        )
        .await;
    let ids: Vec<String> = queue_of(&browser)
        .await
        .iter()
        .map(|item| item["id"].as_str().unwrap().to_owned())
        .collect();

    let moved = browser
        .post(
            &format!("/api/downloads/{}/move", ids[2]),
            json!({ "direction": -1 }),
        )
        .await;
    let removed = browser
        .delete(&format!("/api/downloads/{}", ids[0]), json!({}))
        .await;

    assert_eq!((moved.status, removed.status), (204, 204));
    let order: Vec<String> = queue_of(&browser)
        .await
        .iter()
        .map(|item| item["url"].as_str().unwrap().to_owned())
        .collect();
    assert_eq!(order, [album_url("c"), album_url("b")]);

    assert_eq!(
        browser
            .post("/api/downloads/clear-pending", json!({}))
            .await
            .status,
        204
    );
    assert!(queue_of(&browser).await.is_empty());
}

#[tokio::test]
async fn a_direction_other_than_up_or_down_is_refused() {
    let app = TestApp::start().await;

    let answer = app
        .browser()
        .post("/api/downloads/x/move", json!({ "direction": 5 }))
        .await;

    assert_eq!(answer.status, 400);
    assert_eq!(answer.json()["error"], "bad_direction");
}

#[tokio::test]
async fn finished_items_are_cleared_and_can_be_retried() {
    let app = TestApp::start().await;
    add_one(&app);
    let browser = app.browser();
    browser
        .post(
            "/api/downloads",
            json!({ "items": [{ "url": album_url("alb") }] }),
        )
        .await;
    finished(&browser).await;
    let id = queue_of(&browser).await[0]["id"]
        .as_str()
        .unwrap()
        .to_owned();

    let retried = browser
        .post(&format!("/api/downloads/{id}/retry"), json!({}))
        .await;
    finished(&browser).await;
    let cleared = browser
        .post("/api/downloads/clear-finished", json!({}))
        .await;

    assert_eq!((retried.status, cleared.status), (204, 204));
    assert!(queue_of(&browser).await.is_empty());
    assert_eq!(
        app.catalogue.server.count("/api/album/get"),
        2,
        "it ran twice"
    );
}

#[tokio::test]
async fn cancel_and_skip_are_accepted_when_nothing_runs() {
    let app = TestApp::start().await;
    let browser = app.browser();

    assert_eq!(
        browser
            .post("/api/downloads/cancel", json!({}))
            .await
            .status,
        204
    );
    assert_eq!(
        browser.post("/api/downloads/skip", json!({})).await.status,
        204
    );
}

// ─── The log ───

#[tokio::test]
async fn the_log_tells_what_happened_as_events_the_interface_can_translate() {
    let app = TestApp::start().await;
    add_one(&app);
    let browser = app.browser();
    browser
        .post(
            "/api/downloads",
            json!({ "items": [{ "url": album_url("alb") }] }),
        )
        .await;
    finished(&browser).await;

    let lines = browser.get("/api/log").await.json();

    let events: Vec<&str> = lines
        .as_array()
        .unwrap()
        .iter()
        .map(|line| line["event"].as_str().unwrap())
        .collect();
    assert!(events.contains(&"item_started"), "{events:?}");
    assert!(events.contains(&"downloading_track"), "{events:?}");
    assert!(events.contains(&"complete"), "{events:?}");
    let track = lines
        .as_array()
        .unwrap()
        .iter()
        .find(|line| line["event"] == "downloading_track")
        .unwrap();
    assert_eq!(track["title"], "So What");
    assert_eq!(track["number"], 1);
}

#[tokio::test]
async fn the_log_is_exported_as_an_english_text_file() {
    let app = TestApp::start().await;
    add_one(&app);
    let browser = app.browser();
    browser
        .post(
            "/api/downloads",
            json!({ "items": [{ "url": album_url("alb") }] }),
        )
        .await;
    finished(&browser).await;

    let answer = browser.get("/api/log/export").await;

    assert_eq!(answer.status, 200);
    assert!(
        answer.headers["content-type"]
            .to_str()
            .unwrap()
            .starts_with("text/plain")
    );
    let disposition = answer.headers["content-disposition"]
        .to_str()
        .unwrap()
        .to_owned();
    assert!(
        disposition.starts_with("attachment; filename=\"qobuzdlx-log-"),
        "{disposition}"
    );
    assert!(
        answer.text.starts_with("# QobuzDLX test - "),
        "{}",
        answer.text
    );
    assert!(
        answer.text.contains("Downloading - 01 So What..."),
        "{}",
        answer.text
    );
}

#[tokio::test]
async fn the_log_can_be_emptied() {
    let app = TestApp::start().await;
    add_one(&app);
    let browser = app.browser();
    browser
        .post(
            "/api/downloads",
            json!({ "items": [{ "url": album_url("alb") }] }),
        )
        .await;
    finished(&browser).await;

    let answer = browser.delete("/api/log", json!({})).await;

    assert_eq!(answer.status, 204);
    assert!(
        browser
            .get("/api/log")
            .await
            .json()
            .as_array()
            .unwrap()
            .is_empty()
    );
}

// ─── Live ───

/// The text of a stream of events, read until `wanted` appears in it.
async fn read_until(response: &mut reqwest::Response, seen: &mut String, wanted: &str) {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    while !seen.contains(wanted) {
        let chunk = tokio::time::timeout_at(deadline, response.chunk())
            .await
            .unwrap_or_else(|_| panic!("timed out waiting for {wanted:?} in:\n{seen}"))
            .unwrap()
            .expect("the stream ended");
        seen.push_str(&String::from_utf8_lossy(&chunk));
    }
}

#[tokio::test]
async fn the_interface_follows_the_queue_live() {
    let app = TestApp::start().await;
    add_one(&app);
    let mut stream = reqwest::get(format!("{}/api/downloads/events", app.base))
        .await
        .unwrap();
    assert_eq!(stream.headers()["content-type"], "text/event-stream");
    let mut seen = String::new();

    // The state as it is, first
    read_until(&mut stream, &mut seen, "event: progress").await;
    assert!(seen.contains("event: queue"), "{seen}");

    app.browser()
        .post(
            "/api/downloads",
            json!({ "items": [{ "url": album_url("alb") }] }),
        )
        .await;

    read_until(&mut stream, &mut seen, "\"event\":\"complete\"").await;
    assert!(seen.contains("event: log"), "{seen}");
    read_until(&mut stream, &mut seen, "\"status\":\"completed\"").await;
}

#[tokio::test]
async fn a_locked_application_gives_no_live_view() {
    let app = TestApp::start_with(Some("correct horse battery")).await;

    let response = reqwest::get(format!("{}/api/downloads/events", app.base))
        .await
        .unwrap();

    assert_eq!(response.status(), 401);
}
