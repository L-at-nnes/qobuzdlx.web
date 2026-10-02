mod common;

use std::time::Duration;

use common::{FakeServer, Reply};
use qobuzdlx::download::{Fetcher, TransferError};
use tempfile::TempDir;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpListener;
use tokio_util::sync::CancellationToken;

fn fetcher() -> Fetcher {
    Fetcher::new(reqwest::Client::new(), Duration::from_secs(5))
}

#[tokio::test]
async fn a_file_is_written_as_received() {
    let server = FakeServer::start().await;
    let content: Vec<u8> = (0..=255u8).cycle().take(300_000).collect();
    let body = content.clone();
    server.on("/file", move |_| Reply::bytes(body.clone()));
    let directory = TempDir::new().unwrap();
    let path = directory.path().join("track.flac");

    let transfer = fetcher()
        .to_file(
            &format!("{}/file", server.url()),
            &path,
            &CancellationToken::new(),
            |_| {},
        )
        .await
        .unwrap();

    assert_eq!(std::fs::read(&path).unwrap(), content);
    assert_eq!(transfer.bytes, 300_000);
}

#[tokio::test]
async fn an_error_status_gives_an_error_and_no_file() {
    let server = FakeServer::start().await;
    server.on("/file", |_| Reply::status(403));
    let directory = TempDir::new().unwrap();
    let path = directory.path().join("track.flac");

    let error = fetcher()
        .to_file(
            &format!("{}/file", server.url()),
            &path,
            &CancellationToken::new(),
            |_| {},
        )
        .await
        .unwrap_err();

    assert!(matches!(error, TransferError::Status(status) if status.as_u16() == 403));
    assert!(!path.exists());
}

#[tokio::test]
async fn a_cancelled_transfer_leaves_nothing_behind() {
    let server = FakeServer::start().await;
    server.on("/file", |_| Reply::bytes(vec![0; 1000]));
    let directory = TempDir::new().unwrap();
    let path = directory.path().join("track.flac");
    let cancel = CancellationToken::new();
    cancel.cancel();

    let error = fetcher()
        .to_file(&format!("{}/file", server.url()), &path, &cancel, |_| {})
        .await
        .unwrap_err();

    assert!(matches!(error, TransferError::Cancelled));
    assert!(!path.exists());
}

#[tokio::test]
async fn a_cut_connection_is_an_error_and_the_partial_file_is_removed() {
    // The CDN cuts a file it has not served yet: the body stops before its announced length
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut request = [0u8; 1024];
        let _ = tokio::io::AsyncReadExt::read(&mut socket, &mut request).await;
        socket
            .write_all(b"HTTP/1.1 200 OK\r\ncontent-length: 5000\r\n\r\nonly a little")
            .await
            .unwrap();
        // The socket is dropped here
    });
    let directory = TempDir::new().unwrap();
    let path = directory.path().join("track.flac");

    let error = fetcher()
        .to_file(
            &format!("http://{address}/file"),
            &path,
            &CancellationToken::new(),
            |_| {},
        )
        .await
        .unwrap_err();

    assert!(
        matches!(
            error,
            TransferError::Http(_) | TransferError::Incomplete { .. }
        ),
        "{error}"
    );
    assert!(!path.exists());
}

#[tokio::test]
async fn a_server_that_stays_silent_is_given_up_on() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    tokio::spawn(async move {
        // Every connection gets the start of an answer and then nothing
        loop {
            let Ok((mut socket, _)) = listener.accept().await else {
                return;
            };
            tokio::spawn(async move {
                let mut request = [0u8; 1024];
                let _ = tokio::io::AsyncReadExt::read(&mut socket, &mut request).await;
                let _ = socket
                    .write_all(b"HTTP/1.1 200 OK\r\ncontent-length: 5000\r\n\r\nstart")
                    .await;
                tokio::time::sleep(Duration::from_secs(30)).await;
            });
        }
    });
    let directory = TempDir::new().unwrap();
    let path = directory.path().join("track.flac");

    let error = Fetcher::new(reqwest::Client::new(), Duration::from_millis(300))
        .to_file(
            &format!("http://{address}/file"),
            &path,
            &CancellationToken::new(),
            |_| {},
        )
        .await
        .unwrap_err();

    assert!(matches!(error, TransferError::Stalled(_)), "{error}");
    assert!(!path.exists());
}

#[tokio::test]
async fn the_address_never_appears_in_a_network_error() {
    // Nothing listens on this port: the connection is refused
    let directory = TempDir::new().unwrap();

    let error = fetcher()
        .to_file(
            "http://127.0.0.1:1/file?request_sig=SECRET",
            &directory.path().join("track.flac"),
            &CancellationToken::new(),
            |_| {},
        )
        .await
        .unwrap_err();

    assert!(!format!("{error:?}").contains("SECRET"));
    assert!(!error.to_string().contains("SECRET"));
}
