//! Which extensions are loaded, and which could not be.

use axum::extract::State;
use axum::routing::get;
use axum::{Json, Router};
use serde::Serialize;

use super::state::AppState;
use crate::extensions::{Info, Problem};

pub fn routes() -> Router<AppState> {
    Router::new().route("/extensions", get(list))
}

#[derive(Serialize)]
struct Listing {
    extensions: Vec<Info>,
    problems: Vec<Problem>,
}

async fn list(State(state): State<AppState>) -> Json<Listing> {
    Json(Listing {
        extensions: state.extensions.list(),
        problems: state.extensions.problems().to_vec(),
    })
}
