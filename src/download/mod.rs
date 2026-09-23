//! Downloading: the queue, the pipeline of a track and the worker that runs them.

mod item;
mod queue;
mod url;

pub use item::{Display, FailedTrack, ItemKind, ItemStatus, PlaylistEntry, QueueItem};
pub use queue::{Enqueued, Queue};
pub use url::{QobuzUrl, parse as parse_url};
