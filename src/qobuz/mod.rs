//! Client of the Qobuz API.

mod error;
mod models;
mod signing;

pub use error::Error;
pub use models::*;
pub use signing::sign_file_url;
