//! The HTTP side: the API the interface talks to, and the interface itself.

mod cookie;
mod error;
mod guard;
mod state;

pub use error::{ApiError, ApiResult};
pub use state::{AppState, Shared};
