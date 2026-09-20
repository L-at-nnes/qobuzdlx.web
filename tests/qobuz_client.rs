mod common;

use std::sync::atomic::{AtomicUsize, Ordering};

use common::{FakeServer, Reply};
use qobuzdlx::qobuz::{
    Account, AppCredentials, Client, Error, FavoriteKind, Session, SessionCheck, SessionEvent,
};
use serde_json::json;

fn session() -> Session {
    Session {
        app: AppCredentials {
            app_id: "1234".into(),
            app_secret: "the-secret".into(),
        },
        user_auth_token: "user-token".into(),
        account: Account::default(),
    }
}

async fn signed_in() -> (FakeServer, Client) {
    let server = FakeServer::start().await;
    let client = server.client();
    client.start_session(session());
    (server, client)
}

fn page(items: &[serde_json::Value], total: usize, offset: usize) -> serde_json::Value {
    json!({ "offset": offset, "limit": 500, "total": total, "items": items })
}

// ─── Identification ───

#[tokio::test]
async fn a_signed_in_call_carries_the_app_and_the_token_in_headers_and_query() {
    let (server, client) = signed_in().await;
    server.on_json("/api/track/get", json!({ "id": 7, "title": "Song" }));

    let track = client.track(7).await.unwrap();

    assert_eq!(track.title, "Song");
    let call = &server.calls_to("/api/track/get")[0];
    assert_eq!(call.param("track_id"), "7");
    assert_eq!(call.param("app_id"), "1234");
    assert_eq!(call.headers["x-app-id"], "1234");
    assert_eq!(call.headers["x-user-auth-token"], "user-token");
}

#[tokio::test]
async fn a_call_without_a_session_fails_before_reaching_qobuz() {
    let server = FakeServer::start().await;

    let result = server.client().track(7).await;

    assert!(matches!(result, Err(Error::NotSignedIn)));
    assert_eq!(server.count("/api/track/get"), 0);
}

#[tokio::test]
async fn a_refused_token_is_announced_to_the_listeners_of_the_session() {
    let (server, client) = signed_in().await;
    server.on("/api/track/get", |_| {
        Reply::error(401, "Invalid or expired user_auth_token")
    });
    let mut events = client.subscribe();

    let error = client.track(7).await.unwrap_err();

    assert!(error.is_unauthorized());
    assert_eq!(events.recv().await.unwrap(), SessionEvent::Rejected);
}

#[tokio::test]
async fn starting_and_ending_a_session_is_announced() {
    let server = FakeServer::start().await;
    let client = server.client();
    let mut events = client.subscribe();

    client.start_session(session());
    client.end_session();
    client.end_session(); // already ended: nothing more to say

    assert_eq!(events.recv().await.unwrap(), SessionEvent::Started);
    assert_eq!(events.recv().await.unwrap(), SessionEvent::Ended);
    assert!(events.try_recv().is_err());
    assert!(!client.is_signed_in());
}

// ─── Retries ───

#[tokio::test]
async fn a_rate_limit_and_a_gateway_error_are_retried_until_it_works() {
    let (server, client) = signed_in().await;
    let calls = AtomicUsize::new(0);
    server.on("/api/track/get", move |_| {
        match calls.fetch_add(1, Ordering::SeqCst) {
            0 => Reply::status(503),
            1 => Reply::status(429).with_header("retry-after", "0"),
            _ => Reply::json(json!({ "id": 7, "title": "Song" })),
        }
    });

    let track = client.track(7).await.unwrap();

    assert_eq!(track.title, "Song");
    assert_eq!(server.count("/api/track/get"), 3);
}

#[tokio::test]
async fn a_persistent_rate_limit_gives_up_after_three_attempts_with_the_429() {
    let (server, client) = signed_in().await;
    server.on("/api/track/get", |_| Reply::status(429));

    let error = client.track(7).await.unwrap_err();

    assert_eq!(error.status().map(|status| status.as_u16()), Some(429));
    assert_eq!(server.count("/api/track/get"), 3);
}

#[tokio::test]
async fn an_answer_that_will_not_change_is_not_retried() {
    let (server, client) = signed_in().await;

    let error = client.track(7).await.unwrap_err(); // the fake answers 404 to anything unscripted

    assert_eq!(
        error.to_string(),
        "Qobuz API error 404: No result matching given argument"
    );
    assert_eq!(server.count("/api/track/get"), 1);
}

#[tokio::test]
async fn the_file_url_call_is_retried_like_the_others() {
    let (server, client) = signed_in().await;
    let calls = AtomicUsize::new(0);
    server.on("/api/track/getFileUrl", move |_| {
        if calls.fetch_add(1, Ordering::SeqCst) == 0 {
            Reply::status(502)
        } else {
            Reply::json(json!({ "url": "https://cdn/x", "format_id": 6 }))
        }
    });

    let stream = client.stream_url(7, 6).await.unwrap();

    assert_eq!(stream.format_id, 6);
}

// ─── Streams ───

#[tokio::test]
async fn a_file_url_request_is_signed_with_the_app_secret() {
    let (server, client) = signed_in().await;
    server.on_json(
        "/api/track/getFileUrl",
        json!({ "url": "https://cdn/x", "format_id": 27, "bit_depth": 24, "sampling_rate": 96.0 }),
    );

    let stream = client.stream_url(197_432_204, 27).await.unwrap();

    assert_eq!((stream.bit_depth, stream.format_id), (24, 27));
    let call = &server.calls_to("/api/track/getFileUrl")[0];
    let timestamp: u64 = call.param("request_ts").parse().unwrap();
    assert_eq!(
        call.param("request_sig"),
        qobuzdlx::qobuz::sign_file_url(197_432_204, 27, timestamp, "the-secret")
    );
    assert_eq!(call.param("intent"), "stream");
    assert_eq!(call.param("user_auth_token"), "user-token");
}

#[tokio::test]
async fn a_preview_is_flagged_as_such() {
    let (server, client) = signed_in().await;
    server.on_json(
        "/api/track/getFileUrl",
        json!({ "url": "https://cdn/x", "format_id": 5, "sample": true }),
    );

    assert!(client.stream_url(7, 6).await.unwrap().sample);
}

#[tokio::test]
async fn a_file_url_cannot_be_asked_without_the_app_secret() {
    let server = FakeServer::start().await;
    let client = server.client();
    client.start_session(Session {
        app: AppCredentials {
            app_id: "1".into(),
            app_secret: String::new(),
        },
        ..session()
    });

    assert!(matches!(
        client.stream_url(7, 6).await,
        Err(Error::MissingAppSecret)
    ));
    assert_eq!(server.count("/api/track/getFileUrl"), 0);
}

// ─── Listings capped at 500 ───

#[tokio::test]
async fn every_page_of_a_large_label_is_fetched() {
    let (server, client) = signed_in().await;
    server.on("/api/label/get", |call| {
        let offset = call.offset();
        let items: Vec<_> = (offset..(offset + 500).min(1994))
            .map(|i| json!({ "id": format!("album{i}") }))
            .collect();
        Reply::json(json!({ "id": 9, "name": "Big Label", "albums": page(&items, 1994, offset) }))
    });

    let label = client.label(9).await.unwrap();

    let albums = label.albums.unwrap().items;
    assert_eq!(albums.len(), 1994);
    assert_eq!(albums[1993].id, "album1993");
    assert_eq!(server.count("/api/label/get"), 4);
}

#[tokio::test]
async fn every_page_of_the_favourites_is_fetched_for_the_requested_kind() {
    let (server, client) = signed_in().await;
    server.on("/api/favorite/getUserFavorites", |call| {
        assert_eq!(call.param("type"), "albums");
        let offset = call.offset();
        let items: Vec<_> = (offset..(offset + 500).min(1200))
            .map(|i| json!({ "id": format!("a{i}") }))
            .collect();
        Reply::json(json!({ "albums": page(&items, 1200, offset) }))
    });

    let favorites = client.favorites(5, FavoriteKind::Albums).await.unwrap();

    assert_eq!(favorites.albums.unwrap().items.len(), 1200);
    assert!(favorites.tracks.is_none());
    assert_eq!(server.count("/api/favorite/getUserFavorites"), 3);
}

#[tokio::test]
async fn the_tracks_of_an_album_beyond_the_first_page_are_fetched() {
    let (server, client) = signed_in().await;
    server.on("/api/album/get", |call| {
        let offset = call.offset();
        let items: Vec<_> = (offset..(offset + 500).min(620))
            .map(|i| json!({ "id": i + 1, "title": format!("T{i}") }))
            .collect();
        Reply::json(json!({ "id": "big", "title": "Big", "tracks": page(&items, 620, offset) }))
    });

    let album = client.album("big").await.unwrap();

    assert_eq!(album.tracks.unwrap().items.len(), 620);
}

#[tokio::test]
async fn a_playlist_is_asked_with_its_tracks() {
    let (server, client) = signed_in().await;
    server.on("/api/playlist/get", |call| {
        assert_eq!(call.param("extra"), "tracks");
        Reply::json(json!({ "id": 71, "name": "Mix", "tracks": page(&[json!({ "id": 1, "position": 1, "playlist_track_id": 12_424_235_026_u64 })], 1, 0) }))
    });

    let playlist = client.playlist(71).await.unwrap();

    assert_eq!(
        playlist.tracks.unwrap().items[0].playlist_track_id,
        Some(12_424_235_026)
    );
}

#[tokio::test]
async fn an_artist_discography_is_read_with_its_albums() {
    let (server, client) = signed_in().await;
    server.on("/api/artist/get", |_| {
        Reply::json(json!({ "id": 3, "name": "Nina", "albums": page(&[json!({ "id": "a1" }), json!({ "id": "a2" })], 2, 0) }))
    });

    let artist = client.artist(3).await.unwrap();

    assert_eq!(artist.albums.unwrap().items.len(), 2);
    assert_eq!(
        server.calls_to("/api/artist/get")[0].param("extra"),
        "albums,albums_with_last_release"
    );
}

// ─── Search ───

#[tokio::test]
async fn a_search_passes_the_query_and_the_paging() {
    let (server, client) = signed_in().await;
    server.on_json(
        "/api/album/search",
        json!({ "albums": page(&[json!({ "id": "x", "title": "Café & Crème" })], 130, 50) }),
    );

    let result = client.search_albums("café & crème", 50, 50).await.unwrap();

    assert_eq!((result.total, result.items.len()), (130, 1));
    let call = &server.calls_to("/api/album/search")[0];
    assert_eq!(call.param("query"), "café & crème");
    assert_eq!((call.param("limit"), call.param("offset")), ("50", "50"));
}

// ─── Signing in ───

#[tokio::test]
async fn a_password_sign_in_returns_the_session_of_the_answer() {
    let server = FakeServer::start().await;
    server.on_json(
        "/api/user/login",
        json!({ "user_auth_token": "fresh", "user": { "id": 9, "display_name": "Ada", "credential": { "parameters": { "short_label": "Studio" } } } }),
    );
    let app = AppCredentials {
        app_id: "1234".into(),
        app_secret: "s".into(),
    };

    let session = server
        .client()
        .sign_in(&app, "ada@example.com", "p@ss w0rd")
        .await
        .unwrap();

    assert_eq!(session.user_auth_token, "fresh");
    assert_eq!(session.account.plan.as_deref(), Some("Studio"));
    let call = &server.calls_to("/api/user/login")[0];
    assert_eq!(
        (call.param("email"), call.param("password")),
        ("ada@example.com", "p@ss w0rd")
    );
}

#[tokio::test]
async fn a_failed_login_is_not_replayed() {
    let server = FakeServer::start().await;
    server.on("/api/user/login", |_| Reply::error(503, "busy"));
    let app = AppCredentials {
        app_id: "1".into(),
        app_secret: "s".into(),
    };

    let result = server.client().sign_in(&app, "a@b.c", "wrong").await;

    assert!(result.is_err());
    assert_eq!(server.count("/api/user/login"), 1);
}

#[tokio::test]
async fn a_login_answer_without_a_token_is_an_error() {
    let server = FakeServer::start().await;
    server.on_json("/api/user/login", json!({ "user": { "id": 1 } }));
    let app = AppCredentials {
        app_id: "1".into(),
        app_secret: "s".into(),
    };

    assert!(matches!(
        server.client().sign_in(&app, "a@b.c", "x").await,
        Err(Error::Unexpected(_))
    ));
}

#[tokio::test]
async fn a_network_error_never_shows_the_password_it_carried() {
    // Nothing listens on this port
    let client = Client::new(qobuzdlx::qobuz::ClientConfig {
        api_base: "http://127.0.0.1:9/api/".into(),
        ..qobuzdlx::qobuz::ClientConfig::default()
    });
    let app = AppCredentials {
        app_id: "1".into(),
        app_secret: "s".into(),
    };

    let error = client
        .sign_in(&app, "a@b.c", "hunter2-password")
        .await
        .unwrap_err();

    assert!(matches!(error, Error::Network(_)));
    assert!(!error.to_string().contains("hunter2"), "{error}");
}

#[tokio::test]
async fn a_token_sign_in_checks_the_token_first() {
    let server = FakeServer::start().await;
    server.on("/api/user/get", |_| Reply::error(401, "bad token"));
    let app = AppCredentials {
        app_id: "1".into(),
        app_secret: "s".into(),
    };

    let result = server.client().sign_in_with_token(&app, "stale").await;

    assert!(result.unwrap_err().is_unauthorized());
}

#[tokio::test]
async fn a_saved_session_is_valid_rejected_or_unreachable() {
    let server = FakeServer::start().await;
    let client = server.client();

    server.on_json("/api/user/get", json!({ "id": 3, "display_name": "Ada" }));
    assert!(
        matches!(client.verify_session(&session()).await, SessionCheck::Valid(account) if account.id == 3)
    );

    server.on("/api/user/get", |_| Reply::error(401, "expired"));
    assert_eq!(
        client.verify_session(&session()).await,
        SessionCheck::Rejected
    );

    server.on("/api/user/get", |_| Reply::error(500, "boom"));
    assert_eq!(
        client.verify_session(&session()).await,
        SessionCheck::Unreachable
    );
}

// ─── App credentials ───

fn bundle_with_secret(secret: &str) -> String {
    use base64::Engine;
    let payload = format!(
        "{}{}",
        base64::engine::general_purpose::STANDARD.encode(secret),
        "T".repeat(44)
    );
    let (seed, rest) = payload.split_at(10);
    let (info, extras) = rest.split_at(rest.len() / 2);
    format!(
        r#"production:{{api:{{appId:"555000111",appSecret:"x"}} a.initialSeed("{seed}",window.utimezone.berlin) name:"Europe/Berlin",info:"{info}",extras:"{extras}""#
    )
}

#[tokio::test]
async fn the_app_pair_is_found_in_the_bundle_and_proven_against_the_api() {
    let server = FakeServer::start().await;
    let secret = "0123456789abcdef0123456789abcdef";
    server.on("/login", |_| {
        Reply::text(r#"<script src="/resources/7.0.0-b001/bundle.js"></script>"#)
    });
    server.on("/resources/7.0.0-b001/bundle.js", move |_| {
        Reply::text(bundle_with_secret(secret))
    });
    server.on("/api/track/getFileUrl", move |call| {
        let timestamp: u64 = call.param("request_ts").parse().unwrap();
        let expected = qobuzdlx::qobuz::sign_file_url(197_432_204, 5, timestamp, secret);
        if call.param("app_id") == "555000111" && call.param("request_sig") == expected {
            Reply::json(json!({ "url": "https://cdn/preview", "sample": true }))
        } else {
            Reply::error(401, "bad signature")
        }
    });

    let credentials = server.client().resolve_app_credentials().await.unwrap();

    assert_eq!(
        credentials,
        AppCredentials {
            app_id: "555000111".into(),
            app_secret: secret.into()
        }
    );
}

#[tokio::test]
async fn the_known_pairs_are_the_last_resort() {
    let server = FakeServer::start().await;
    // No login page, so the bundle cannot be read; the API only accepts the second known pair
    server.on("/api/track/getFileUrl", |call| {
        if call.param("app_id") == "798273057" {
            Reply::json(json!({ "url": "x" }))
        } else {
            Reply::error(401, "no")
        }
    });
    // A signature check would need the real secret: accept by app id alone for this scenario

    let credentials = server.client().resolve_app_credentials().await.unwrap();

    assert_eq!(credentials.app_id, "798273057");
}

#[tokio::test]
async fn no_pair_is_returned_when_none_is_accepted() {
    let server = FakeServer::start().await;
    server.on("/api/track/getFileUrl", |_| Reply::error(401, "no"));

    assert!(server.client().resolve_app_credentials().await.is_none());
}

#[tokio::test]
async fn a_pair_is_valid_only_when_qobuz_answers_200() {
    let server = FakeServer::start().await;
    let client = server.client();
    let pair = AppCredentials {
        app_id: "1".into(),
        app_secret: "s".into(),
    };

    server.on_json("/api/track/getFileUrl", json!({ "url": "x" }));
    assert!(client.app_credentials_are_valid(&pair).await);

    server.on("/api/track/getFileUrl", |_| Reply::error(401, "no"));
    assert!(!client.app_credentials_are_valid(&pair).await);
}
