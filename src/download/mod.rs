//! Downloading: the queue, the pipeline of a track and the worker that runs them.

mod artwork;
mod chain;
mod goody;
mod item;
mod job;
mod list;
mod log;
mod m3u;
mod manager;
mod progress;
mod queue;
mod report;
mod store;
mod track;
mod transfer;
mod url;
mod verify;

pub use artwork::{CoverError, Covers, cover_url};
pub use chain::{describe_stream, fallback_chain, quality_label};
pub use goody::goody_file_name;
pub use item::{Display, FailedTrack, ItemKind, ItemStatus, PlaylistEntry, QueueItem};
pub use job::{Job, JobError, JobSetup};
pub use list::{AddressList, parse_list};
pub use log::{Log, LogEvent, LogLine};
pub use manager::{Event, InvalidAddress, Manager, ManagerConfig, Snapshot};
pub use progress::{AlbumCard, Halt, Progress, ProgressScope, TrackProgress};
pub use queue::{Enqueued, EnqueuedCounts, Queue};
pub use report::{Outcome, Reporter, StepTrack};
pub use store::QueueStore;
pub use track::{Context, Environment, TEMP_PREFIX, TrackError, TrackRequest, TrackResult, Tuning};
pub use transfer::{Error as TransferError, Fetcher, Transfer, Update as TransferUpdate};
pub use url::{QobuzUrl, canonical as canonical_url, parse as parse_url};
pub use verify::{Error as VerifyError, fix_md5, verify_flac};
