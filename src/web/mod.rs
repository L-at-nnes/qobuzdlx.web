//! The HTTP side: the API the interface talks to, and the interface itself.

mod access;
mod catalogue;
mod cookie;
mod downloads;
mod dto;
mod error;
mod guard;
mod session;
mod state;

use axum::extract::State;
use axum::middleware::{from_fn, from_fn_with_state};
use axum::routing::get;
use axum::{Json, Router};
use serde::Serialize;

pub use error::{ApiError, ApiResult};
pub use state::{AppState, Shared};

/// The application: every route, with the guards in front of them.
pub fn router(state: AppState) -> Router {
    let guarded = Router::new()
        .merge(access::guarded())
        .merge(session::routes())
        .merge(catalogue::routes())
        .merge(downloads::routes())
        .layer(from_fn_with_state(state.clone(), guard::require_unlock));
    let api = Router::new().merge(access::open()).merge(guarded);

    Router::new()
        .route("/healthz", get(health))
        .nest("/api", api)
        .layer(from_fn(guard::same_origin))
        .layer(from_fn(guard::security_headers))
        .with_state(state)
}

#[derive(Serialize)]
struct Health {
    app: &'static str,
    version: &'static str,
}

/// Answers without being asked for a password and without any data of the account: enough for a
/// client to check that an address is a `QobuzDLX`.
async fn health(State(state): State<AppState>) -> Json<Health> {
    Json(Health {
        app: "QobuzDLX",
        version: state.version,
    })
}
