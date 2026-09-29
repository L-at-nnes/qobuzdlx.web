mod common;

use std::sync::Arc;
use std::time::Duration;

use common::{FakeServer, Reply};
use qobuzdlx::qobuz::{AppCredentials, Restored, Sessions, SignInError};
use serde_json::json;
use tempfile::TempDir;

fn app() -> AppCredentials {
    AppCredentials {
        app_id: "1234".into(),
        app_secret: "the-secret".into(),
    }
}

fn sessions(server: &FakeServer, directory: &TempDir) -> Sessions {
    Sessions::new(server.client(), directory.path()).with_credentials(app())
}

fn login_works(server: &FakeServer) {
    server.on_json(
        "/api/user/login",
        json!({
            "user_auth_token": "the-token",
            "user": { "id": 7, "display_name": "Ada", "credential": { "parameters": { "short_label": "Studio" } } },
        }),
    );
}

fn saved(directory: &TempDir) -> std::path::PathBuf {
    directory.path().join("session.json")
}

#[tokio::test]
async fn signing_in_opens_the_session_and_keeps_it() {
    let server = FakeServer::start().await;
    login_works(&server);
    let directory = TempDir::new().unwrap();
    let sessions = sessions(&server, &directory);

    let account = sessions.sign_in("ada@example.com", "p@ss").await.unwrap();

    assert_eq!(account.display_name.as_deref(), Some("Ada"));
    assert_eq!(account.plan.as_deref(), Some("Studio"));
    assert_eq!(sessions.account(), Some(account));
    assert!(saved(&directory).exists());
}

#[tokio::test]
async fn wrong_credentials_open_nothing_and_keep_nothing() {
    let server = FakeServer::start().await;
    server.on("/api/user/login", |_| {
        Reply::error(401, "Invalid username/email and password combination")
    });
    let directory = TempDir::new().unwrap();
    let sessions = sessions(&server, &directory);

    let result = sessions.sign_in("ada@example.com", "wrong").await;

    assert!(matches!(result, Err(SignInError::Qobuz(error)) if error.is_unauthorized()));
    assert_eq!(sessions.account(), None);
    assert!(!saved(&directory).exists());
}

#[tokio::test]
async fn a_token_from_the_web_player_signs_in_after_being_checked() {
    let server = FakeServer::start().await;
    server.on_json("/api/user/get", json!({ "id": 7, "display_name": "Ada" }));
    let directory = TempDir::new().unwrap();
    let sessions = sessions(&server, &directory);

    let account = sessions.sign_in_with_token("  the-token \n").await.unwrap();

    assert_eq!(account.id, 7);
    assert_eq!(
        server.calls_to("/api/user/get")[0].headers["x-user-auth-token"],
        "the-token",
        "the token is trimmed"
    );
}

#[tokio::test]
async fn a_stale_token_is_refused_before_anything_is_kept() {
    let server = FakeServer::start().await;
    server.on("/api/user/get", |_| Reply::error(401, "expired"));
    let directory = TempDir::new().unwrap();
    let sessions = sessions(&server, &directory);

    assert!(sessions.sign_in_with_token("stale").await.is_err());
    assert!(!saved(&directory).exists());
}

#[tokio::test]
async fn signing_out_forgets_the_session() {
    let server = FakeServer::start().await;
    login_works(&server);
    let directory = TempDir::new().unwrap();
    let sessions = sessions(&server, &directory);
    sessions.sign_in("ada@example.com", "p@ss").await.unwrap();

    sessions.sign_out();
    sessions.sign_out(); // nothing left to forget is not a problem

    assert_eq!(sessions.account(), None);
    assert!(!saved(&directory).exists());
}

// ─── Restoring ───

async fn with_a_saved_session() -> (FakeServer, TempDir) {
    let server = FakeServer::start().await;
    login_works(&server);
    let directory = TempDir::new().unwrap();
    sessions(&server, &directory)
        .sign_in("ada@example.com", "p@ss")
        .await
        .unwrap();
    (server, directory)
}

#[tokio::test]
async fn a_saved_session_is_opened_again_if_qobuz_accepts_it() {
    let (server, directory) = with_a_saved_session().await;
    server.on_json(
        "/api/user/get",
        json!({ "id": 7, "display_name": "Ada Lovelace" }),
    );
    let next_run = sessions(&server, &directory);

    let restored = next_run.restore(3, Duration::ZERO).await;

    assert_eq!(restored, Restored::Resumed);
    assert_eq!(
        next_run.account().unwrap().display_name.as_deref(),
        Some("Ada Lovelace"),
        "the account is the one Qobuz gives now"
    );
}

#[tokio::test]
async fn a_refused_session_is_forgotten() {
    let (server, directory) = with_a_saved_session().await;
    server.on("/api/user/get", |_| Reply::error(401, "expired"));
    let next_run = sessions(&server, &directory);

    let restored = next_run.restore(3, Duration::ZERO).await;

    assert_eq!(restored, Restored::Refused);
    assert_eq!(next_run.account(), None);
    assert!(!saved(&directory).exists());
    assert_eq!(server.count("/api/user/get"), 1, "no point asking again");
}

#[tokio::test]
async fn an_unreachable_qobuz_does_not_sign_anyone_out() {
    let (server, directory) = with_a_saved_session().await;
    server.on("/api/user/get", |_| Reply::status(500));
    let next_run = sessions(&server, &directory);

    let restored = next_run.restore(3, Duration::ZERO).await;

    assert_eq!(restored, Restored::Unreachable);
    assert_eq!(server.count("/api/user/get"), 3);
    assert!(saved(&directory).exists(), "kept for later");
}

#[tokio::test]
async fn an_outage_that_ends_lets_the_restoration_succeed() {
    let (server, directory) = with_a_saved_session().await;
    let calls = std::sync::atomic::AtomicUsize::new(0);
    server.on("/api/user/get", move |_| {
        if calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst) < 2 {
            Reply::status(500)
        } else {
            Reply::json(json!({ "id": 7 }))
        }
    });

    let restored = sessions(&server, &directory)
        .restore(5, Duration::ZERO)
        .await;

    assert_eq!(restored, Restored::Resumed);
}

#[tokio::test]
async fn nothing_saved_means_nothing_to_restore() {
    let server = FakeServer::start().await;
    let directory = TempDir::new().unwrap();

    assert_eq!(
        sessions(&server, &directory)
            .restore(3, Duration::ZERO)
            .await,
        Restored::Nothing
    );
}

#[tokio::test]
async fn a_session_refused_while_in_use_is_forgotten_too() {
    let (server, directory) = with_a_saved_session().await;
    server.on_json("/api/user/get", json!({ "id": 7 }));
    let client = server.client();
    let sessions =
        Arc::new(Sessions::new(client.clone(), directory.path()).with_credentials(app()));
    sessions.restore(1, Duration::ZERO).await;
    let watcher = {
        let sessions = Arc::clone(&sessions);
        tokio::spawn(async move { sessions.forget_when_refused().await })
    };
    tokio::task::yield_now().await;
    server.on("/api/track/get", |_| Reply::error(401, "expired"));

    // Any signed-in call that meets the refusal announces it
    assert!(client.track(7).await.is_err());

    for _ in 0..100 {
        if !saved(&directory).exists() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    assert!(!saved(&directory).exists());
    watcher.abort();
}
