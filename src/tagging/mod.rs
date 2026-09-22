//! Writing tags into the audio files.

mod comment;
mod values;
mod writer;

pub use comment::{build_comment, html_to_text};
pub use values::{TagValues, album_artist, build};
pub use writer::{Error, write};
