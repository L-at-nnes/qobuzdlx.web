//! Folder and file names built from templates.

mod sanitize;

pub use sanitize::{
    MAX_COMPONENT_BYTES, is_inside, make_safe_filename, sanitize_path_segment, truncate_to_bytes,
};
