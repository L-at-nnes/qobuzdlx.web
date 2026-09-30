mod common;

use common::app::TestApp;
use serde_json::json;

const PASSWORD: &str = "correct horse battery";

async fn protected() -> TestApp {
    TestApp::start_with(Some(PASSWORD)).await
}

fn host_of(app: &TestApp) -> String {
    app.base.trim_start_matches("http://").to_owned()
}

#[tokio::test]
async fn the_health_check_needs_no_password() {
    let app = protected().await;

    let answer = app.browser().get("/healthz").await;

    assert_eq!(answer.status, 200);
    assert_eq!(
        answer.json(),
        json!({ "app": "QobuzDLX", "version": "test" })
    );
}

#[tokio::test]
async fn without_a_password_everything_is_open() {
    let app = TestApp::start().await;
    let browser = app.browser();

    let status = browser.get("/api/access").await.json();
    let guarded = browser
        .put(
            "/api/access/password",
            json!({ "new": "a brand new password" }),
        )
        .await;

    assert_eq!(status["protected"], false);
    assert_eq!(status["unlocked"], true);
    assert_eq!(guarded.status, 204);
}

#[tokio::test]
async fn a_protected_application_is_locked_until_the_password_is_given() {
    let app = protected().await;
    let browser = app.browser();

    let status = browser.get("/api/access").await.json();
    let refused = browser
        .put("/api/access/password", json!({ "new": "something else!" }))
        .await;

    assert_eq!(status["protected"], true);
    assert_eq!(status["unlocked"], false);
    assert_eq!(status["managed"], true);
    assert_eq!(refused.status, 401);
    assert_eq!(refused.json()["error"], "locked");
}

#[tokio::test]
async fn the_right_password_unlocks_this_browser_with_a_private_cookie() {
    let app = protected().await;
    let browser = app.browser();

    let answer = browser
        .post("/api/access/unlock", json!({ "password": PASSWORD }))
        .await;

    assert_eq!(answer.status, 200);
    assert_eq!(answer.json()["redirect"], "/");
    let cookie = answer.headers["set-cookie"].to_str().unwrap().to_owned();
    assert!(
        cookie.contains("HttpOnly") && cookie.contains("SameSite=Lax"),
        "{cookie}"
    );
    assert_eq!(browser.get("/api/access").await.json()["unlocked"], true);
}

#[tokio::test]
async fn the_page_asked_for_is_remembered_only_if_it_belongs_to_the_site() {
    let app = protected().await;
    let browser = app.browser();

    let local = browser
        .post(
            "/api/access/unlock",
            json!({ "password": PASSWORD, "return_to": "/downloads" }),
        )
        .await;
    let foreign = browser
        .post(
            "/api/access/unlock",
            json!({ "password": PASSWORD, "return_to": "//evil.example" }),
        )
        .await;

    assert_eq!(local.json()["redirect"], "/downloads");
    assert_eq!(foreign.json()["redirect"], "/");
}

#[tokio::test]
async fn a_wrong_password_gives_nothing() {
    let app = protected().await;
    let browser = app.browser();

    let answer = browser
        .post("/api/access/unlock", json!({ "password": "nope" }))
        .await;

    assert_eq!(answer.status, 401);
    assert_eq!(answer.json()["error"], "wrong_password");
    assert!(answer.headers.get("set-cookie").is_none());
    assert_eq!(browser.token(), None);
}

#[tokio::test]
async fn five_wrong_passwords_close_the_gate_even_to_the_right_one() {
    let app = protected().await;
    let browser = app.browser();
    for _ in 0..4 {
        let answer = browser
            .post("/api/access/unlock", json!({ "password": "nope" }))
            .await;
        assert_eq!(answer.status, 401);
    }

    let fifth = browser
        .post("/api/access/unlock", json!({ "password": "nope" }))
        .await;
    let right_but_late = browser
        .post("/api/access/unlock", json!({ "password": PASSWORD }))
        .await;

    assert_eq!(fifth.status, 429);
    assert!(fifth.json()["retry_after"].as_u64().unwrap() > 0);
    assert!(fifth.headers.contains_key("retry-after"));
    assert_eq!(right_but_late.status, 429);
    let waiting = app.browser().get("/api/access").await.json()["locked_out_seconds"]
        .as_u64()
        .unwrap();
    assert!(waiting > 0);
}

#[tokio::test]
async fn the_gate_is_shared_by_every_browser() {
    let app = protected().await;
    let attacker = app.browser();
    for _ in 0..5 {
        attacker
            .post("/api/access/unlock", json!({ "password": "nope" }))
            .await;
    }

    let owner = app
        .browser()
        .post("/api/access/unlock", json!({ "password": PASSWORD }))
        .await;

    // Behind a proxy every address is the same, so there is one gate for the whole application
    assert_eq!(owner.status, 429);
}

#[tokio::test]
async fn locking_forgets_this_browser() {
    let app = protected().await;
    let browser = app.browser();
    browser
        .post("/api/access/unlock", json!({ "password": PASSWORD }))
        .await;

    let locked = browser.post("/api/access/lock", json!({})).await;

    assert_eq!(locked.status, 204);
    assert_eq!(browser.token(), None);
    assert_eq!(browser.get("/api/access").await.json()["unlocked"], false);
}

#[tokio::test]
async fn a_forged_cookie_does_not_unlock() {
    let app = protected().await;
    let browser = app.browser();
    browser.hold(Some("eyJzdGFtcCI6Im5vbmUiLCJpc3N1ZWQiOjF9.AAAA".to_owned()));

    assert_eq!(browser.get("/api/access").await.json()["unlocked"], false);
}

#[tokio::test]
async fn a_cookie_from_another_run_with_another_key_does_not_unlock() {
    let first = protected().await;
    let other = first.browser();
    other
        .post("/api/access/unlock", json!({ "password": PASSWORD }))
        .await;
    let second = protected().await;
    let browser = second.browser();
    browser.hold(other.token());

    assert_eq!(browser.get("/api/access").await.json()["unlocked"], false);
}

// ─── Changing the password ───

#[tokio::test]
async fn a_new_password_locks_the_other_devices_and_not_this_one() {
    let app = TestApp::start().await;
    let first = app.browser();
    first
        .put("/api/access/password", json!({ "new": "first password!" }))
        .await;
    let second = app.browser();
    let unlocked = second
        .post(
            "/api/access/unlock",
            json!({ "password": "first password!" }),
        )
        .await;
    assert_eq!(unlocked.status, 200);

    let changed = second
        .put(
            "/api/access/password",
            json!({ "current": "first password!", "new": "second password!" }),
        )
        .await;

    assert_eq!(changed.status, 204);
    assert_eq!(
        second.get("/api/access").await.json()["unlocked"],
        true,
        "this device stays in"
    );
    let relocked = app
        .browser()
        .post(
            "/api/access/unlock",
            json!({ "password": "second password!" }),
        )
        .await;
    assert_eq!(relocked.status, 200);
    let old = app
        .browser()
        .post(
            "/api/access/unlock",
            json!({ "password": "first password!" }),
        )
        .await;
    assert_eq!(old.status, 401);
}

#[tokio::test]
async fn a_device_unlocked_for_the_old_password_is_out_after_the_change() {
    let app = TestApp::start().await;
    let owner = app.browser();
    owner
        .put("/api/access/password", json!({ "new": "first password!" }))
        .await;
    let phone = app.browser();
    phone
        .post(
            "/api/access/unlock",
            json!({ "password": "first password!" }),
        )
        .await;
    assert_eq!(phone.get("/api/access").await.json()["unlocked"], true);

    owner
        .put(
            "/api/access/password",
            json!({ "current": "first password!", "new": "second password!" }),
        )
        .await;

    assert_eq!(phone.get("/api/access").await.json()["unlocked"], false);
}

#[tokio::test]
async fn changing_the_password_asks_for_the_current_one() {
    let app = TestApp::start().await;
    let browser = app.browser();
    browser
        .put("/api/access/password", json!({ "new": "first password!" }))
        .await;
    browser
        .post(
            "/api/access/unlock",
            json!({ "password": "first password!" }),
        )
        .await;

    let without = browser
        .put("/api/access/password", json!({ "new": "second password!" }))
        .await;
    let wrong = browser
        .put(
            "/api/access/password",
            json!({ "current": "not it", "new": "second password!" }),
        )
        .await;

    assert_eq!(without.status, 401);
    assert_eq!(wrong.status, 401);
    assert_eq!(wrong.json()["error"], "wrong_password");
}

#[tokio::test]
async fn a_password_that_is_too_short_is_refused() {
    let app = TestApp::start().await;

    let answer = app
        .browser()
        .put("/api/access/password", json!({ "new": "short" }))
        .await;

    assert_eq!(answer.status, 400);
    assert_eq!(answer.json()["error"], "password_too_short");
}

#[tokio::test]
async fn the_protection_can_be_removed_with_the_current_password() {
    let app = TestApp::start().await;
    let browser = app.browser();
    browser
        .put("/api/access/password", json!({ "new": "first password!" }))
        .await;
    browser
        .post(
            "/api/access/unlock",
            json!({ "password": "first password!" }),
        )
        .await;

    let removed = browser
        .delete(
            "/api/access/password",
            json!({ "current": "first password!" }),
        )
        .await;

    assert_eq!(removed.status, 204);
    assert_eq!(
        app.browser().get("/api/access").await.json()["protected"],
        false
    );
}

#[tokio::test]
async fn a_password_imposed_by_the_environment_cannot_be_changed_from_the_interface() {
    let app = protected().await;
    let browser = app.browser();
    browser
        .post("/api/access/unlock", json!({ "password": PASSWORD }))
        .await;

    let change = browser
        .put(
            "/api/access/password",
            json!({ "current": PASSWORD, "new": "another password!" }),
        )
        .await;
    let removal = browser
        .delete("/api/access/password", json!({ "current": PASSWORD }))
        .await;

    assert_eq!(change.status, 409);
    assert_eq!(removal.status, 409);
    assert_eq!(change.json()["error"], "password_managed");
}

// ─── Requests from other sites ───

#[tokio::test]
async fn a_change_asked_by_another_site_is_refused_before_anything_is_checked() {
    let app = protected().await;
    let browser = app.browser();
    let host = host_of(&app);

    let answer = browser
        .send(
            reqwest::Method::POST,
            "/api/access/unlock",
            Some(json!({ "password": PASSWORD })),
            &[("origin", "https://evil.example"), ("host", &host)],
        )
        .await;

    assert_eq!(answer.status, 403);
    assert_eq!(answer.json()["error"], "cross_site");
    assert_eq!(browser.token(), None);
}

#[tokio::test]
async fn a_change_from_this_site_goes_through() {
    let app = protected().await;
    let browser = app.browser();
    let origin = format!("http://{}", host_of(&app));

    let answer = browser
        .send(
            reqwest::Method::POST,
            "/api/access/unlock",
            Some(json!({ "password": PASSWORD })),
            &[("origin", &origin), ("sec-fetch-site", "same-origin")],
        )
        .await;

    assert_eq!(answer.status, 200);
}

#[tokio::test]
async fn every_answer_forbids_framing_and_sniffing() {
    let app = protected().await;

    let answer = app.browser().get("/api/access").await;

    assert_eq!(answer.headers["x-content-type-options"], "nosniff");
    assert_eq!(answer.headers["x-frame-options"], "DENY");
    assert!(
        answer.headers["content-security-policy"]
            .to_str()
            .unwrap()
            .contains("frame-ancestors 'none'")
    );
    assert_eq!(answer.headers["referrer-policy"], "same-origin");
    assert_eq!(answer.headers["cache-control"], "no-store");
}
