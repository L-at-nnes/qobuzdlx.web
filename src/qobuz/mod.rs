//! Client of the Qobuz API.

mod auth;
mod catalogue;
mod client;
mod error;
mod models;
mod pagination;
mod session;
mod signing;

pub use auth::SessionCheck;
pub use catalogue::FavoriteKind;
pub use client::{Client, ClientConfig, Result, RetryPolicy};
pub use error::Error;
pub use models::*;
pub use session::{Account, AppCredentials, Session, SessionEvent};
pub use signing::sign_file_url;
