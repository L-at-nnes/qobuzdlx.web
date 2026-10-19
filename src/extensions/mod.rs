//! Extensions: code that reacts to what the application does (a track saved, an item finished)
//! without being part of it.
//!
//! An extension is anything that implements [`Extension`]. Two kinds can be added without
//! recompiling, by dropping a folder with an `extension.json` into the extensions directory: a
//! program run for each event ([`CommandExtension`]) and an HTTP address told about each event
//! ([`WebhookExtension`]). Built-in extensions are written in Rust and handed to
//! [`Extensions::start`] by `main`.
//!
//! The download code only calls [`Extensions::emit`], which never waits and never fails: a slow,
//! broken or missing extension cannot slow down or stop a download.

mod command;
mod event;
mod loader;
mod manifest;
mod registry;
mod webhook;

pub use command::CommandExtension;
pub use event::{Event, EventKind, ItemFinished, SCHEMA, TrackSaved};
pub use loader::{Loaded, Problem, load_dir};
pub use manifest::{Manifest, ManifestError, Webhook};
pub use registry::{Extensions, Source};
pub use webhook::WebhookExtension;

use futures_util::future::BoxFuture;
use serde::Serialize;

/// Why an extension could not handle an event. Only logged: the application goes on.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{0}")]
pub struct Failure(pub String);

/// What an extension says about itself.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Info {
    /// Unique among the loaded extensions: letters, digits, `-` and `_`.
    pub name: String,
    pub version: Option<String>,
    pub description: Option<String>,
    pub source: Source,
    /// The events it wants. Empty means all of them, including the ones added later.
    pub events: Vec<EventKind>,
}

impl Info {
    #[must_use]
    pub fn wants(&self, kind: EventKind) -> bool {
        self.events.is_empty() || self.events.contains(&kind)
    }
}

/// Code that reacts to events.
///
/// Events reach an extension one at a time, in the order they happened. A handler that takes long
/// delays only the later events of its own extension, and one that panics is reported as failed
/// and does not stop the next.
pub trait Extension: Send + Sync + 'static {
    fn info(&self) -> Info;

    fn handle<'a>(&'a self, event: &'a Event) -> BoxFuture<'a, Result<(), Failure>>;
}
