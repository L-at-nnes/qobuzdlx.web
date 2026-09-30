mod common;

use common::Reply;
use common::app::TestApp;
use serde_json::json;

fn saved(app: &TestApp) -> std::path::PathBuf {
    app.config.path().join("session.json")
}

#[tokio::test]
async fn the_session_tells_who_is_signed_in() {
    let app = TestApp::start().await;

    let answer = app.browser().get("/api/session").await;

    assert_eq!(answer.status, 200);
    assert_eq!(answer.json()["signed_in"], true);
}

#[tokio::test]
async fn signing_out_forgets_the_session() {
    let app = TestApp::start().await;
    let browser = app.browser();

    let answer = browser.post("/api/session/logout", json!({})).await;

    assert_eq!(answer.status, 204);
    let current = browser.get("/api/session").await.json();
    assert_eq!(current["signed_in"], false);
    assert_eq!(current["account"], json!(null));
}

#[tokio::test]
async fn a_password_sign_in_opens_the_session_and_keeps_it() {
    let app = TestApp::start().await;
    let browser = app.browser();
    browser.post("/api/session/logout", json!({})).await;
    app.catalogue.server.on_json(
        "/api/user/login",
        json!({
            "user_auth_token": "the-token",
            "user": { "id": 7, "display_name": "Ada", "credential": { "parameters": { "short_label": "Studio" } } },
        }),
    );

    let answer = browser
        .post(
            "/api/session/login",
            json!({ "email": " ada@example.com ", "password": "p@ss" }),
        )
        .await;

    assert_eq!(answer.status, 200);
    assert_eq!(answer.json()["display_name"], "Ada");
    assert_eq!(answer.json()["plan"], "Studio");
    assert_eq!(browser.get("/api/session").await.json()["account"]["id"], 7);
    assert!(saved(&app).exists());
    let call = &app.catalogue.server.calls_to("/api/user/login")[0];
    assert_eq!(
        call.param("email"),
        "ada@example.com",
        "the e-mail is trimmed"
    );
}

#[tokio::test]
async fn wrong_credentials_are_the_users_to_fix() {
    let app = TestApp::start().await;
    let browser = app.browser();
    browser.post("/api/session/logout", json!({})).await;
    app.catalogue.server.on("/api/user/login", |_| {
        Reply::error(401, "Invalid username/email and password combination")
    });

    let answer = browser
        .post(
            "/api/session/login",
            json!({ "email": "ada@example.com", "password": "wrong" }),
        )
        .await;

    assert_eq!(answer.status, 401);
    assert_eq!(answer.json()["error"], "wrong_credentials");
    assert_eq!(browser.get("/api/session").await.json()["signed_in"], false);
}

#[tokio::test]
async fn the_password_never_comes_back_in_an_error() {
    let app = TestApp::start().await;
    let browser = app.browser();
    app.catalogue
        .server
        .on("/api/user/login", |_| Reply::error(503, "busy"));

    let answer = browser
        .post(
            "/api/session/login",
            json!({ "email": "ada@example.com", "password": "my-secret-phrase" }),
        )
        .await;

    assert_eq!(answer.status, 502);
    assert!(!answer.text.contains("my-secret-phrase"));
}

#[tokio::test]
async fn a_token_from_the_web_player_is_checked_before_it_is_kept() {
    let app = TestApp::start().await;
    let browser = app.browser();
    app.catalogue
        .server
        .on("/api/user/get", |_| Reply::error(401, "expired"));

    let answer = browser
        .post("/api/session/token", json!({ "token": "stale" }))
        .await;

    assert_eq!(answer.status, 401);
    assert_eq!(answer.json()["error"], "invalid_token");
}

#[tokio::test]
async fn a_good_token_signs_in() {
    let app = TestApp::start().await;
    let browser = app.browser();
    browser.post("/api/session/logout", json!({})).await;
    app.catalogue
        .server
        .on_json("/api/user/get", json!({ "id": 9, "display_name": "Grace" }));

    let answer = browser
        .post("/api/session/token", json!({ "token": "good" }))
        .await;

    assert_eq!(answer.status, 200);
    assert_eq!(answer.json()["display_name"], "Grace");
}

#[tokio::test]
async fn nothing_is_asked_of_qobuz_for_an_empty_form() {
    let app = TestApp::start().await;
    let browser = app.browser();

    let no_password = browser
        .post(
            "/api/session/login",
            json!({ "email": "ada@example.com", "password": "" }),
        )
        .await;
    let no_token = browser
        .post("/api/session/token", json!({ "token": "   " }))
        .await;

    assert_eq!(no_password.status, 400);
    assert_eq!(no_password.json()["error"], "missing_credentials");
    assert_eq!(no_token.status, 400);
    assert_eq!(app.catalogue.server.count("/api/user/login"), 0);
    assert_eq!(app.catalogue.server.count("/api/user/get"), 0);
}

#[tokio::test]
async fn a_locked_application_gives_no_access_to_the_qobuz_account() {
    let app = TestApp::start_with(Some("correct horse battery")).await;
    let browser = app.browser();

    let current = browser.get("/api/session").await;
    let logout = browser.post("/api/session/logout", json!({})).await;

    assert_eq!(current.status, 401);
    assert_eq!(logout.status, 401);
    assert!(app.state.client.is_signed_in(), "nothing was signed out");
}
