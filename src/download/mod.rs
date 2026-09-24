//! Downloading: the queue, the pipeline of a track and the worker that runs them.

mod chain;
mod item;
mod log;
mod m3u;
mod progress;
mod queue;
mod store;
mod transfer;
mod url;
mod verify;

pub use chain::{describe_stream, fallback_chain, quality_label};
pub use item::{Display, FailedTrack, ItemKind, ItemStatus, PlaylistEntry, QueueItem};
pub use log::{Log, LogEvent, LogLine};
pub use progress::{AlbumCard, Progress, ProgressScope, TrackProgress};
pub use queue::{Enqueued, Queue};
pub use store::QueueStore;
pub use transfer::{Error as TransferError, Fetcher, Transfer, Update as TransferUpdate};
pub use url::{QobuzUrl, parse as parse_url};
pub use verify::{Error as VerifyError, fix_md5, verify_flac};
