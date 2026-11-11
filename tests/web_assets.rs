mod common;

use common::app::TestApp;

#[tokio::test]
async fn an_unknown_api_route_is_a_json_not_found_never_a_page() {
    let app = TestApp::start().await;

    let answer = app.browser().get("/api/nothing-here").await;

    assert_eq!(answer.status, 404);
    assert_eq!(answer.json()["error"], "not_found");
}

#[tokio::test]
async fn a_missing_script_fails_instead_of_coming_back_as_html() {
    let app = TestApp::start().await;

    let answer = app.browser().get("/_app/immutable/missing.js").await;

    assert_eq!(answer.status, 404);
}

#[tokio::test]
async fn the_interface_files_carry_the_security_headers_too() {
    let app = TestApp::start().await;

    let answer = app.browser().get("/").await;

    // The interface may or may not be built when this runs; the headers are there either way
    assert!(matches!(answer.status, 200 | 503), "{}", answer.status);
    assert_eq!(answer.headers["x-frame-options"], "DENY");
    assert_eq!(answer.headers["x-content-type-options"], "nosniff");
}

/// A client that takes answers as they come, without unpacking them.
fn plain_client() -> reqwest::Client {
    reqwest::Client::builder().no_gzip().build().unwrap()
}

#[tokio::test]
async fn the_entry_page_is_not_sent_again_to_a_browser_that_holds_it() {
    let app = TestApp::start().await;
    let first = app.browser().get("/").await;
    if first.status == 503 {
        // The interface is not built where this runs: nothing to hold
        return;
    }
    let etag = first.headers["etag"].to_str().unwrap().to_owned();

    let again = app
        .browser()
        .send(
            reqwest::Method::GET,
            "/",
            None,
            &[("if-none-match", etag.as_str())],
        )
        .await;

    assert_eq!(again.status, 304);
    assert_eq!(again.text, "");
    assert_eq!(again.headers["x-frame-options"], "DENY");
}

#[tokio::test]
async fn what_the_api_answers_is_compressed_for_a_browser_that_takes_it() {
    let app = TestApp::start().await;
    let url = format!("{}/api/settings", app.base);

    let packed = plain_client()
        .get(&url)
        .header("accept-encoding", "gzip")
        .send()
        .await
        .unwrap();
    let plain = plain_client().get(&url).send().await.unwrap();

    assert_eq!(packed.headers()["content-encoding"], "gzip");
    assert!(!plain.headers().contains_key("content-encoding"));
    let (packed, plain) = (packed.bytes().await.unwrap(), plain.bytes().await.unwrap());
    assert!(
        packed.len() < plain.len(),
        "{} < {}",
        packed.len(),
        plain.len()
    );
    serde_json::from_slice::<serde_json::Value>(&plain).unwrap();
}

#[tokio::test]
async fn the_live_events_are_not_held_back_by_the_compression() {
    let app = TestApp::start().await;

    let mut events = plain_client()
        .get(format!("{}/api/downloads/events", app.base))
        .header("accept-encoding", "gzip")
        .send()
        .await
        .unwrap();

    assert!(!events.headers().contains_key("content-encoding"));
    // The state as it is comes at once, not when a buffer fills up
    let first = tokio::time::timeout(std::time::Duration::from_secs(5), events.chunk())
        .await
        .expect("the first event comes at once")
        .unwrap()
        .unwrap();
    assert!(String::from_utf8_lossy(&first).contains("event: queue"));
}

#[tokio::test]
async fn the_interface_needs_no_password_to_be_loaded() {
    let app = TestApp::start_with(Some("correct horse battery")).await;

    let answer = app.browser().get("/").await;

    // It holds no data of the account: it asks for the password itself
    assert!(matches!(answer.status, 200 | 503), "{}", answer.status);
}
