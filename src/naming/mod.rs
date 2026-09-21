//! Folder and file names built from templates.

mod credits;
mod namer;
mod quality;
mod sanitize;

pub use credits::{
    album_title, format_title, is_various_artists, joined_main_artists, parental_warning,
    track_title,
};
pub use namer::{DownloadKind, Namer};
pub use quality::{
    AudioQuality, effective as effective_quality, format_name, format_with_hi_res_quality,
    format_with_quality,
};
pub use sanitize::{
    MAX_COMPONENT_BYTES, is_inside, make_safe_filename, sanitize_path_segment, truncate_to_bytes,
};
