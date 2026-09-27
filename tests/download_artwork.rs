mod common;

use common::{FakeServer, Reply};
use qobuzdlx::download::{CoverError, Covers};
use qobuzdlx::qobuz::Album;
use qobuzdlx::settings::ArtworkSize;

/// The start of a JPEG file, which is all the check looks at.
fn jpeg(tag: &str) -> Vec<u8> {
    let mut bytes = vec![0xFF, 0xD8, 0xFF, 0xE0];
    bytes.extend_from_slice(tag.as_bytes());
    bytes
}

fn album_with_cover(server: &FakeServer) -> Album {
    serde_json::from_value(serde_json::json!({
        "id": "abc",
        "title": "Kind of Blue",
        "artist": { "id": 1, "name": "Miles Davis" },
        "image": { "large": format!("{}/covers/abc_600.jpg", server.url()) },
    }))
    .unwrap()
}

#[tokio::test]
async fn the_requested_size_is_downloaded() {
    let server = FakeServer::start().await;
    server.on("/covers/abc_150.jpg", |_| Reply::bytes(jpeg("small")));
    let covers = Covers::new(reqwest::Client::new());

    let image = covers
        .fetch(&album_with_cover(&server), ArtworkSize::Pixels(150))
        .await
        .unwrap();

    assert_eq!(image, jpeg("small"));
}

#[tokio::test]
async fn a_size_qobuz_refuses_gives_the_default_one() {
    let server = FakeServer::start().await;
    server.on("/covers/abc_600.jpg", |_| Reply::bytes(jpeg("regular")));
    // No original for this release: the server answers 404 for it
    let covers = Covers::new(reqwest::Client::new());

    let image = covers
        .fetch(&album_with_cover(&server), ArtworkSize::Original)
        .await
        .unwrap();

    assert_eq!(image, jpeg("regular"));
    assert_eq!(server.count("/covers/abc_org.jpg"), 1);
}

#[tokio::test]
async fn without_any_cover_it_is_an_error_not_an_empty_picture() {
    let server = FakeServer::start().await;
    let covers = Covers::new(reqwest::Client::new());

    let result = covers
        .fetch(&album_with_cover(&server), ArtworkSize::Pixels(600))
        .await;

    assert!(result.is_err());
}

#[tokio::test]
async fn an_error_page_served_as_a_cover_is_not_taken_for_one() {
    let server = FakeServer::start().await;
    server.on("/covers/abc_600.jpg", |_| {
        Reply::text("<html>Service temporarily unavailable</html>")
    });
    let covers = Covers::new(reqwest::Client::new());

    let result = covers
        .fetch(&album_with_cover(&server), ArtworkSize::Pixels(600))
        .await;

    assert!(matches!(result, Err(CoverError::NotAnImage)));
}

#[tokio::test]
async fn an_original_that_is_not_a_picture_gives_the_default_size() {
    let server = FakeServer::start().await;
    server.on("/covers/abc_org.jpg", |_| Reply::text("not a picture"));
    server.on("/covers/abc_600.jpg", |_| Reply::bytes(jpeg("regular")));
    let covers = Covers::new(reqwest::Client::new());

    let image = covers
        .fetch(&album_with_cover(&server), ArtworkSize::Original)
        .await
        .unwrap();

    assert_eq!(image, jpeg("regular"));
}
