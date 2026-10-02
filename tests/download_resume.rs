//! The CDN cuts connections after a few megabytes. A transfer goes on from where it stopped.

use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use qobuzdlx::download::{Fetcher, TransferError};
use tempfile::TempDir;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio_util::sync::CancellationToken;

/// What the scripted server does.
#[derive(Clone, Copy)]
struct Behaviour {
    /// Bytes of the body sent per connection before it is cut; `None` to send it all.
    cut_after: Option<usize>,
    /// Whether `Range` is honoured. A server that does not sends the whole file again.
    ranges: bool,
    /// The first connection goes silent after `cut_after` bytes instead of being cut.
    stall_first: bool,
    /// Pretends to start the answer elsewhere than asked.
    wrong_part: bool,
}

impl Behaviour {
    const fn cutting_every(bytes: usize) -> Self {
        Self {
            cut_after: Some(bytes),
            ranges: true,
            stall_first: false,
            wrong_part: false,
        }
    }
}

struct Cdn {
    address: SocketAddr,
    /// The `Range` header of each request, in order.
    requests: Arc<Mutex<Vec<Option<String>>>>,
}

impl Cdn {
    fn url(&self) -> String {
        format!("http://{}/file", self.address)
    }

    fn requests(&self) -> Vec<Option<String>> {
        self.requests.lock().unwrap().clone()
    }
}

async fn serve(content: Vec<u8>, behaviour: Behaviour) -> Cdn {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let requests = Arc::new(Mutex::new(Vec::new()));

    let log = Arc::clone(&requests);
    tokio::spawn(async move {
        let mut connection = 0;
        loop {
            let Ok((socket, _)) = listener.accept().await else {
                return;
            };
            connection += 1;
            let stall = behaviour.stall_first && connection == 1;
            let (content, log) = (content.clone(), Arc::clone(&log));
            tokio::spawn(async move { answer(socket, &content, behaviour, stall, &log).await });
        }
    });
    Cdn { address, requests }
}

async fn answer(
    mut socket: TcpStream,
    content: &[u8],
    behaviour: Behaviour,
    stall: bool,
    log: &Mutex<Vec<Option<String>>>,
) {
    let mut request = vec![0u8; 4096];
    let read = socket.read(&mut request).await.unwrap_or(0);
    let text = String::from_utf8_lossy(&request[..read]).into_owned();
    let range = text.lines().find_map(|line| {
        line.to_ascii_lowercase()
            .strip_prefix("range: bytes=")
            .map(str::to_owned)
    });
    log.lock().unwrap().push(range.clone());

    let start = range
        .as_deref()
        .filter(|_| behaviour.ranges)
        .and_then(|range| range.trim_end_matches('-').parse::<usize>().ok())
        .unwrap_or(0);
    let body = &content[start..];
    let head = if start > 0 || (range.is_some() && behaviour.ranges) {
        let shown_start = if behaviour.wrong_part { 0 } else { start };
        format!(
            "HTTP/1.1 206 Partial Content\r\ncontent-length: {}\r\ncontent-range: bytes {shown_start}-{}/{}\r\n\r\n",
            body.len(),
            content.len() - 1,
            content.len()
        )
    } else {
        format!("HTTP/1.1 200 OK\r\ncontent-length: {}\r\n\r\n", body.len())
    };
    let _ = socket.write_all(head.as_bytes()).await;

    let sent = behaviour
        .cut_after
        .map_or(body.len(), |limit| limit.min(body.len()));
    let _ = socket.write_all(&body[..sent]).await;
    let _ = socket.flush().await;
    if stall {
        tokio::time::sleep(Duration::from_secs(30)).await;
    }
    // Dropping the socket cuts the connection
}

fn content(length: usize) -> Vec<u8> {
    (0..length)
        .map(|index| u8::try_from(index % 251).unwrap())
        .collect()
}

fn fetcher() -> Fetcher {
    Fetcher::new(reqwest::Client::new(), Duration::from_millis(400))
}

async fn fetch(cdn: &Cdn) -> (Result<u64, TransferError>, std::path::PathBuf, TempDir) {
    let directory = TempDir::new().unwrap();
    let path = directory.path().join("track.flac");
    let result = fetcher()
        .to_file(&cdn.url(), &path, &CancellationToken::new(), |_| {})
        .await
        .map(|transfer| transfer.bytes);
    (result, path, directory)
}

#[tokio::test]
async fn a_connection_cut_again_and_again_is_resumed_until_the_file_is_whole() {
    let file = content(5000);
    let cdn = serve(file.clone(), Behaviour::cutting_every(1200)).await;

    let (result, path, _directory) = fetch(&cdn).await;

    assert_eq!(result.unwrap(), 5000);
    assert_eq!(std::fs::read(&path).unwrap(), file);
    let requests = cdn.requests();
    assert_eq!(requests.len(), 5, "{requests:?}");
    assert_eq!(requests[0], None);
    assert_eq!(requests[1].as_deref(), Some("1200-"));
    assert_eq!(requests[4].as_deref(), Some("4800-"));
}

#[tokio::test]
async fn a_file_that_comes_in_one_go_takes_one_request() {
    let file = content(5000);
    let cdn = serve(
        file.clone(),
        Behaviour {
            cut_after: None,
            ..Behaviour::cutting_every(0)
        },
    )
    .await;

    let (result, path, _directory) = fetch(&cdn).await;

    assert_eq!(result.unwrap(), 5000);
    assert_eq!(std::fs::read(path).unwrap(), file);
    assert_eq!(cdn.requests().len(), 1);
}

#[tokio::test]
async fn a_silent_connection_is_replaced_and_the_transfer_goes_on() {
    let file = content(3000);
    let cdn = serve(
        file.clone(),
        Behaviour {
            cut_after: Some(1000),
            stall_first: true,
            ..Behaviour::cutting_every(1000)
        },
    )
    .await;

    let (result, path, _directory) = fetch(&cdn).await;

    // The first connection goes quiet after 1000 bytes, the next ones are cut every 1000
    assert_eq!(result.unwrap(), 3000);
    assert_eq!(std::fs::read(path).unwrap(), file);
}

#[tokio::test]
async fn connections_that_bring_nothing_end_the_transfer() {
    let cdn = serve(content(5000), Behaviour::cutting_every(0)).await;

    let (result, path, _directory) = fetch(&cdn).await;

    assert!(result.is_err());
    assert!(!path.exists(), "no partial file is left");
    assert_eq!(
        cdn.requests().len(),
        3,
        "three barren connections, then it gives up"
    );
}

#[tokio::test]
async fn a_server_that_does_not_do_ranges_starts_over_and_gives_up_if_it_never_gets_further() {
    let cdn = serve(
        content(5000),
        Behaviour {
            ranges: false,
            ..Behaviour::cutting_every(1000)
        },
    )
    .await;

    let (result, path, _directory) = fetch(&cdn).await;

    assert!(matches!(
        result,
        Err(TransferError::Incomplete { .. } | TransferError::Http(_))
    ));
    assert!(!path.exists());
}

#[tokio::test]
async fn a_server_that_does_not_do_ranges_is_fine_if_it_sends_everything() {
    let file = content(5000);
    let cdn = serve(
        file.clone(),
        Behaviour {
            ranges: false,
            cut_after: None,
            ..Behaviour::cutting_every(0)
        },
    )
    .await;

    let (result, path, _directory) = fetch(&cdn).await;

    assert_eq!(result.unwrap(), 5000);
    assert_eq!(std::fs::read(path).unwrap(), file);
}

#[tokio::test]
async fn another_part_of_the_file_than_the_one_asked_for_is_refused() {
    let cdn = serve(
        content(5000),
        Behaviour {
            wrong_part: true,
            ..Behaviour::cutting_every(1000)
        },
    )
    .await;

    let (result, path, _directory) = fetch(&cdn).await;

    assert!(
        matches!(result, Err(TransferError::UnexpectedRange)),
        "{result:?}"
    );
    assert!(!path.exists());
}

#[tokio::test]
async fn cancelling_while_resuming_stops_at_once() {
    let cdn = serve(content(5000), Behaviour::cutting_every(500)).await;
    let cancel = CancellationToken::new();
    cancel.cancel();
    let directory = TempDir::new().unwrap();
    let path = directory.path().join("track.flac");

    let result = fetcher().to_file(&cdn.url(), &path, &cancel, |_| {}).await;

    assert!(matches!(result, Err(TransferError::Cancelled)));
    assert!(!path.exists());
}
