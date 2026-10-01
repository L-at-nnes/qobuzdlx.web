mod common;

use common::Reply;
use common::app::TestApp;
use common::catalogue::page;
use serde_json::json;

fn album_json(id: &str, title: &str) -> serde_json::Value {
    json!({
        "id": id, "title": title, "artist": { "id": 1, "name": "Miles Davis" },
        "release_date_original": "1959-08-17", "tracks_count": 5,
        "maximum_bit_depth": 24, "maximum_sampling_rate": 96.0,
        "label": { "id": 2, "name": "Columbia" },
        "image": { "thumbnail": "https://static.example/t.jpg" },
    })
}

#[tokio::test]
async fn albums_are_found_by_a_search() {
    let app = TestApp::start().await;
    app.catalogue.server.on_json(
        "/api/album/search",
        json!({ "albums": {
            "offset": 0, "limit": 50, "total": 120,
            "items": [album_json("a1", "Kind of Blue"), album_json("a2", "Sketches of Spain")],
        } }),
    );

    let answer = app
        .browser()
        .get("/api/search/albums?q=miles%20davis")
        .await;

    assert_eq!(answer.status, 200);
    let json = answer.json();
    assert_eq!(json["total"], 120);
    assert_eq!(json["items"][0]["title"], "Kind of Blue");
    assert_eq!(json["items"][0]["url"], "https://play.qobuz.com/album/a1");
    assert_eq!(json["items"][1]["artist"], "Miles Davis");
    let call = &app.catalogue.server.calls_to("/api/album/search")[0];
    assert_eq!(call.param("query"), "miles davis");
    assert_eq!(call.param("limit"), "50");
    assert_eq!(call.param("offset"), "0");
}

#[tokio::test]
async fn the_next_page_is_asked_for_by_its_offset() {
    let app = TestApp::start().await;
    app.catalogue.server.on_json(
        "/api/track/search",
        json!({ "tracks": { "offset": 50, "limit": 50, "total": 60, "items": [] } }),
    );

    app.browser()
        .get("/api/search/tracks?q=blue&offset=50&limit=20")
        .await;

    let call = &app.catalogue.server.calls_to("/api/track/search")[0];
    assert_eq!((call.param("offset"), call.param("limit")), ("50", "20"));
}

#[tokio::test]
async fn a_page_is_never_larger_than_the_interface_may_ask_for() {
    let app = TestApp::start().await;
    app.catalogue.server.on_json(
        "/api/artist/search",
        json!({ "artists": { "offset": 0, "limit": 100, "total": 0, "items": [] } }),
    );

    app.browser()
        .get("/api/search/artists?q=nina&limit=5000")
        .await;
    app.browser()
        .get("/api/search/artists?q=nina&limit=0")
        .await;

    let calls = app.catalogue.server.calls_to("/api/artist/search");
    assert_eq!(calls[0].param("limit"), "100");
    assert_eq!(calls[1].param("limit"), "1");
}

#[tokio::test]
async fn tracks_artists_and_playlists_are_searched_too() {
    let app = TestApp::start().await;
    let server = &app.catalogue.server;
    server.on_json(
        "/api/track/search",
        json!({ "tracks": page(vec![json!({ "id": 5, "title": "So What", "duration": 562,
            "performer": { "id": 1, "name": "Miles Davis" },
            "album": { "id": "a1", "title": "Kind of Blue", "artist": { "id": 1 } } })]) }),
    );
    server.on_json(
        "/api/artist/search",
        json!({ "artists": page(vec![json!({ "id": 9, "name": "Nina Simone", "albums_count": 40 })]) }),
    );
    server.on_json(
        "/api/playlist/search",
        json!({ "playlists": page(vec![json!({ "id": 3, "name": "Late jazz", "tracks_count": 12,
            "owner": { "id": 1, "name": "Ada" } })]) }),
    );
    let browser = app.browser();

    let tracks = browser.get("/api/search/tracks?q=x").await.json();
    let artists = browser.get("/api/search/artists?q=x").await.json();
    let playlists = browser.get("/api/search/playlists?q=x").await.json();

    assert_eq!(tracks["items"][0]["album"]["title"], "Kind of Blue");
    assert_eq!(artists["items"][0]["albums_count"], 40);
    assert_eq!(playlists["items"][0]["owner"], "Ada");
}

#[tokio::test]
async fn an_empty_search_is_refused_without_asking_qobuz() {
    let app = TestApp::start().await;

    let answer = app.browser().get("/api/search/albums?q=%20%20").await;

    assert_eq!(answer.status, 400);
    assert_eq!(answer.json()["error"], "empty_query");
    assert_eq!(app.catalogue.server.count("/api/album/search"), 0);
}

#[tokio::test]
async fn only_four_kinds_can_be_searched() {
    let app = TestApp::start().await;

    let answer = app.browser().get("/api/search/labels?q=x").await;

    assert_eq!(answer.status, 404);
    assert_eq!(answer.json()["error"], "unknown_kind");
}

#[tokio::test]
async fn a_qobuz_outage_is_a_bad_gateway() {
    let app = TestApp::start().await;
    app.catalogue
        .server
        .on("/api/album/search", |_| Reply::error(503, "busy"));

    let answer = app.browser().get("/api/search/albums?q=x").await;

    assert_eq!(answer.status, 502);
    assert_eq!(answer.json()["error"], "qobuz_unavailable");
}

// ─── Inside things ───

#[tokio::test]
async fn the_tracks_of_an_album_are_listed_in_order() {
    let app = TestApp::start().await;
    app.catalogue.add_album(
        "alb",
        "Kind of Blue",
        "Miles Davis",
        &[(11, 1, "So What"), (12, 2, "Freddie Freeloader")],
    );

    let answer = app.browser().get("/api/albums/alb/tracks").await;

    assert_eq!(answer.status, 200);
    let tracks = answer.json();
    assert_eq!(tracks.as_array().unwrap().len(), 2);
    assert_eq!(tracks[0]["title"], "So What");
    assert_eq!(tracks[1]["number"], 2);
    assert_eq!(tracks[1]["duration"], 100);
}

#[tokio::test]
async fn an_album_that_does_not_exist_is_not_found() {
    let app = TestApp::start().await;

    let answer = app.browser().get("/api/albums/nowhere/tracks").await;

    assert_eq!(answer.status, 404);
    assert_eq!(answer.json()["error"], "qobuz_not_found");
}

#[tokio::test]
async fn the_tracks_of_a_playlist_come_from_whichever_albums() {
    let app = TestApp::start().await;
    app.catalogue
        .add_album("a1", "Kind of Blue", "Miles Davis", &[(11, 1, "So What")]);
    app.catalogue.add_album(
        "a2",
        "A Love Supreme",
        "John Coltrane",
        &[(21, 1, "Acknowledgement")],
    );
    app.catalogue
        .add_playlist(7, "Evening", &[(21, 1), (11, 2)]);

    let tracks = app.browser().get("/api/playlists/7/tracks").await.json();

    assert_eq!(tracks[0]["artist"], "John Coltrane");
    assert_eq!(tracks[1]["title"], "So What");
}

#[tokio::test]
async fn the_albums_of_an_artist_are_listed() {
    let app = TestApp::start().await;
    app.catalogue.artists.lock().unwrap().insert(
        1,
        json!({ "id": 1, "name": "Miles Davis", "albums": page(vec![album_json("a1", "Kind of Blue")]) }),
    );

    let albums = app.browser().get("/api/artists/1/albums").await.json();

    assert_eq!(albums[0]["title"], "Kind of Blue");
    assert_eq!(albums[0]["bit_depth"], 24);
}

#[tokio::test]
async fn a_locked_application_searches_nothing() {
    let app = TestApp::start_with(Some("correct horse battery")).await;

    let answer = app.browser().get("/api/search/albums?q=miles").await;

    assert_eq!(answer.status, 401);
    assert_eq!(app.catalogue.server.count("/api/album/search"), 0);
}
