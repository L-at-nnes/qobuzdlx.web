use std::io;
use std::path::Path;
use std::time::{Duration, Instant};

use futures_util::StreamExt;
use tokio::io::AsyncWriteExt;
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
    Http(reqwest::Error),
    #[error("the server answered {0}")]
    Status(reqwest::StatusCode),
    #[error("no data received for {} seconds", .0.as_secs())]
    Stalled(Duration),
    #[error("incomplete download: {received} of {expected} bytes")]
    Incomplete { received: u64, expected: u64 },
    #[error("could not write the file: {0}")]
    Io(#[from] io::Error),
    #[error("cancelled")]
    Cancelled,
}

impl From<reqwest::Error> for Error {
    /// The address is removed: a signed stream link is not for the log.
    fn from(error: reqwest::Error) -> Self {
        Self::Http(error.without_url())
    }
}

/// Downloads files from the CDN.
#[derive(Debug, Clone)]
pub struct Fetcher {
    http: reqwest::Client,
    /// How long the connection may stay silent before the transfer is given up.
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

    /// Writes the content of `url` to `path`, starting from scratch: a signed link is not worth
    /// resuming. The partial file is removed when the transfer fails.
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
        let response = tokio::select! {
            () = cancel.cancelled() => return Err(Error::Cancelled),
            response = self.http.get(url).send() => response?,
        };
        if !response.status().is_success() {
            return Err(Error::Status(response.status()));
        }
        let expected = response.content_length().filter(|length| *length > 0);

        let mut file = tokio::fs::File::create(path).await?;
        let mut stream = response.bytes_stream();
        let started = Instant::now();
        let mut last_report = started;
        let mut received: u64 = 0;

        loop {
            let chunk = tokio::select! {
                () = cancel.cancelled() => return Err(Error::Cancelled),
                chunk = tokio::time::timeout(self.stall_timeout, stream.next()) => chunk,
            };
            let Some(chunk) = chunk.map_err(|_| Error::Stalled(self.stall_timeout))? else {
                break;
            };
            let chunk = chunk?;
            file.write_all(&chunk).await?;
            received += chunk.len() as u64;

            if let Some(expected) = expected
                && last_report.elapsed() >= REPORT_EVERY
            {
                last_report = Instant::now();
                report(update(received, expected, started.elapsed()));
            }
        }
        file.flush().await?;

        if let Some(expected) = expected
            && received < expected
        {
            return Err(Error::Incomplete { received, expected });
        }
        Ok(Transfer {
            bytes: received,
            elapsed: started.elapsed(),
        })
    }
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
