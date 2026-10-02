use std::io::{self, SeekFrom};
use std::path::Path;
use std::time::{Duration, Instant};

use futures_util::StreamExt;
use reqwest::StatusCode;
use reqwest::header;
use tokio::io::{AsyncSeekExt, AsyncWriteExt};
use tokio_util::sync::CancellationToken;

/// How often the progress is reported while a file comes in.
const REPORT_EVERY: Duration = Duration::from_millis(250);

/// What a finished transfer received.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Transfer {
    pub bytes: u64,
    pub elapsed: Duration,
}

impl Transfer {
    #[must_use]
    #[allow(clippy::cast_precision_loss)] // A file is far below 2^52 bytes
    pub fn megabytes(self) -> f64 {
        self.bytes as f64 / 1_048_576.0
    }

    #[must_use]
    pub fn megabytes_per_second(self) -> f64 {
        self.megabytes() / self.elapsed.as_secs_f64().max(0.001)
    }
}

/// How far a transfer has got.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Update {
    pub percent: u8,
    /// Average since the start of the file.
    pub bytes_per_second: u64,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("download failed: {0}")]
    Http(String),
    #[error("the server answered {0}")]
    Status(reqwest::StatusCode),
    #[error("no data received for {} seconds", .0.as_secs())]
    Stalled(Duration),
    #[error("incomplete download: {received} of {expected} bytes")]
    Incomplete { received: u64, expected: u64 },
    #[error("could not write the file: {0}")]
    Io(#[from] io::Error),
    /// The answer to a request for the rest of the file starts elsewhere than asked.
    #[error("the server sent another part of the file than the one asked for")]
    UnexpectedRange,
    #[error("cancelled")]
    Cancelled,
}

impl From<reqwest::Error> for Error {
    /// The address is removed: a signed stream link is not for the log.
    fn from(error: reqwest::Error) -> Self {
        Self::Http(crate::cause::describe(&error.without_url()))
    }
}

/// Connections in a row that bring nothing new before the transfer is given up.
const MAX_BARREN_CONNECTIONS: u32 = 3;

/// How a connection ended.
enum Ended {
    /// Everything has been received.
    Complete,
    /// The connection stopped before the end. What was received is kept; the rest can be asked for.
    Cut(Error),
}

/// The file being received, across the connections it takes.
struct Receiving {
    file: tokio::fs::File,
    received: u64,
    total: Option<u64>,
    started: Instant,
    last_report: Instant,
}

/// Downloads files from the CDN.
///
/// The CDN cuts a connection after a few seconds and a few megabytes, whatever the client: a
/// 24-bit album track takes a dozen connections. A cut is therefore not a failure. The transfer
/// goes on from where it stopped (`Range`), with the same address, and only gives up when
/// connections stop bringing anything.
#[derive(Debug, Clone)]
pub struct Fetcher {
    http: reqwest::Client,
    /// How long the connection may stay silent before it is given up.
    stall_timeout: Duration,
}

impl Fetcher {
    #[must_use]
    pub fn new(http: reqwest::Client, stall_timeout: Duration) -> Self {
        Self {
            http,
            stall_timeout,
        }
    }

    /// Writes the content of `url` to `path`, starting from scratch. The partial file is removed
    /// when the transfer fails.
    pub async fn to_file(
        &self,
        url: &str,
        path: &Path,
        cancel: &CancellationToken,
        mut report: impl FnMut(Update) + Send,
    ) -> Result<Transfer, Error> {
        let result = self.receive(url, path, cancel, &mut report).await;
        if result.is_err() {
            let _ = tokio::fs::remove_file(path).await;
        }
        result
    }

    async fn receive(
        &self,
        url: &str,
        path: &Path,
        cancel: &CancellationToken,
        report: &mut (impl FnMut(Update) + Send),
    ) -> Result<Transfer, Error> {
        let now = Instant::now();
        let mut receiving = Receiving {
            file: tokio::fs::File::create(path).await?,
            received: 0,
            total: None,
            started: now,
            last_report: now,
        };

        let mut barren = 0;
        loop {
            let before = receiving.received;
            match self.connection(url, &mut receiving, cancel, report).await? {
                Ended::Complete => break,
                Ended::Cut(error) => {
                    barren = if receiving.received > before {
                        0
                    } else {
                        barren + 1
                    };
                    // Without a length there is no telling where to go on from, or whether it is over
                    if barren >= MAX_BARREN_CONNECTIONS || receiving.total.is_none() {
                        return Err(error);
                    }
                    tracing::debug!(
                        received = receiving.received,
                        total = ?receiving.total,
                        %error,
                        "connection cut, going on from where it stopped"
                    );
                }
            }
        }

        receiving.file.flush().await?;
        Ok(Transfer {
            bytes: receiving.received,
            elapsed: receiving.started.elapsed(),
        })
    }

    /// One connection: asks for the file from the first byte missing, and keeps what comes.
    async fn connection(
        &self,
        url: &str,
        receiving: &mut Receiving,
        cancel: &CancellationToken,
        report: &mut (impl FnMut(Update) + Send),
    ) -> Result<Ended, Error> {
        let resuming = receiving.received > 0;
        let mut request = self.http.get(url);
        if resuming {
            request = request.header(header::RANGE, format!("bytes={}-", receiving.received));
        }
        let sent = tokio::select! {
            () = cancel.cancelled() => return Err(Error::Cancelled),
            sent = request.send() => sent,
        };
        let response = match sent {
            Ok(response) => response,
            Err(error) if resuming => return Ok(Ended::Cut(error.into())),
            Err(error) => return Err(error.into()),
        };

        let status = response.status();
        if !status.is_success() {
            // A first request refused is the end of it; a later one is one more connection lost
            return if resuming {
                Ok(Ended::Cut(Error::Status(status)))
            } else {
                Err(Error::Status(status))
            };
        }
        if status == StatusCode::PARTIAL_CONTENT {
            let range = content_range(&response);
            if range.map(|range| range.start) != Some(receiving.received) {
                return Err(Error::UnexpectedRange);
            }
            receiving.total = range.and_then(|range| range.total);
        } else {
            // The whole file, from the start: either the first request, or a server that does not
            // do ranges and starts over
            if resuming {
                receiving.file.set_len(0).await?;
                receiving.file.seek(SeekFrom::Start(0)).await?;
                receiving.received = 0;
            }
            receiving.total = response.content_length().filter(|length| *length > 0);
        }
        tracing::debug!(
            status = status.as_u16(),
            from = receiving.received,
            total = ?receiving.total,
            "receiving a file"
        );

        let mut stream = response.bytes_stream();
        loop {
            let chunk = tokio::select! {
                () = cancel.cancelled() => return Err(Error::Cancelled),
                chunk = tokio::time::timeout(self.stall_timeout, stream.next()) => chunk,
            };
            match chunk {
                Err(_) => return Ok(Ended::Cut(Error::Stalled(self.stall_timeout))),
                Ok(Some(Err(error))) => return Ok(Ended::Cut(error.into())),
                Ok(Some(Ok(bytes))) => {
                    receiving.file.write_all(&bytes).await?;
                    receiving.received += bytes.len() as u64;
                    receiving.report(report);
                }
                Ok(None) => break,
            }
        }

        match receiving.total {
            Some(total) if receiving.received < total => Ok(Ended::Cut(Error::Incomplete {
                received: receiving.received,
                expected: total,
            })),
            _ => Ok(Ended::Complete),
        }
    }
}

impl Receiving {
    /// Tells how far it has got, a few times a second.
    fn report(&mut self, report: &mut impl FnMut(Update)) {
        if let Some(total) = self.total
            && self.last_report.elapsed() >= REPORT_EVERY
        {
            self.last_report = Instant::now();
            report(update(self.received, total, self.started.elapsed()));
        }
    }
}

/// What a `Content-Range: bytes 100-199/1000` header says.
#[derive(Debug, Clone, Copy)]
struct ContentRange {
    start: u64,
    total: Option<u64>,
}

fn content_range(response: &reqwest::Response) -> Option<ContentRange> {
    let value = response
        .headers()
        .get(header::CONTENT_RANGE)?
        .to_str()
        .ok()?
        .strip_prefix("bytes ")?;
    let (span, total) = value.split_once('/')?;
    let (start, _end) = span.split_once('-')?;
    Some(ContentRange {
        start: start.trim().parse().ok()?,
        total: total.trim().parse().ok(),
    })
}

#[allow(clippy::cast_precision_loss, clippy::cast_possible_truncation)]
#[allow(clippy::cast_sign_loss)] // Sizes and durations are positive and far below 2^52
fn update(received: u64, expected: u64, elapsed: Duration) -> Update {
    let percent = (received.saturating_mul(100) / expected).min(100) as u8;
    let bytes_per_second = (received as f64 / elapsed.as_secs_f64().max(0.001)) as u64;
    Update {
        percent,
        bytes_per_second,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_percentage_follows_what_is_received() {
        assert_eq!(update(0, 200, Duration::from_secs(1)).percent, 0);
        assert_eq!(update(50, 200, Duration::from_secs(1)).percent, 25);
        assert_eq!(update(200, 200, Duration::from_secs(1)).percent, 100);
    }

    #[test]
    fn more_than_announced_does_not_exceed_a_hundred() {
        assert_eq!(update(300, 200, Duration::from_secs(1)).percent, 100);
    }

    #[test]
    fn the_speed_is_the_average_since_the_start() {
        assert_eq!(
            update(4_000_000, 8_000_000, Duration::from_secs(2)).bytes_per_second,
            2_000_000
        );
    }

    #[test]
    fn a_transfer_reports_its_size_and_speed_in_megabytes() {
        let transfer = Transfer {
            bytes: 10 * 1_048_576,
            elapsed: Duration::from_secs(4),
        };

        assert!((transfer.megabytes() - 10.0).abs() < 1e-9);
        assert!((transfer.megabytes_per_second() - 2.5).abs() < 1e-9);
    }

    #[test]
    fn an_instant_transfer_does_not_divide_by_zero() {
        let transfer = Transfer {
            bytes: 1_048_576,
            elapsed: Duration::ZERO,
        };

        assert!(transfer.megabytes_per_second().is_finite());
    }
}
