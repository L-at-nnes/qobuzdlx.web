//! Unlocking the interface, locking it again, and the password itself.

use std::time::Instant;

use axum::extract::State;
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post, put};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};

use super::cookie;
use super::error::{ApiError, ApiResult};
use super::guard::{unix_now, unlock_of};
use super::state::AppState;
use crate::access::{AccessError, local_path, stamp_for, verify_password};

/// What needs no unlocking: finding out whether to unlock, and unlocking.
pub fn open() -> Router<AppState> {
    Router::new()
        .route("/access", get(status))
        .route("/access/unlock", post(unlock))
        .route("/access/lock", post(lock))
}

/// What changes the password: only for someone who is in.
pub fn guarded() -> Router<AppState> {
    Router::new().route(
        "/access/password",
        put(set_password).delete(remove_password),
    )
}

#[derive(Serialize)]
struct Status {
    /// A password is set.
    protected: bool,
    /// This browser may use the application.
    unlocked: bool,
    /// The password comes from `ACCESS_PASSWORD` and cannot be changed here.
    managed: bool,
    /// Seconds before another attempt is accepted; 0 when the gate is open.
    locked_out_seconds: u64,
}

async fn status(State(state): State<AppState>, headers: HeaderMap) -> Json<Status> {
    Json(Status {
        protected: state.access.is_protected(),
        unlocked: unlock_of(&state, &headers).allows(),
        managed: state.access.is_managed(),
        locked_out_seconds: state.gate.remaining_seconds(Instant::now()),
    })
}

#[derive(Deserialize)]
struct UnlockRequest {
    password: String,
    /// Where the interface wants to go next; only a path of this site is honoured.
    return_to: Option<String>,
}

#[derive(Serialize)]
struct Unlocked {
    redirect: String,
}

/// Checks a password without blocking the server: hashing is slow on purpose.
async fn check(password: String, stored: String) -> bool {
    tokio::task::spawn_blocking(move || verify_password(&password, &stored))
        .await
        .unwrap_or(false)
}

/// Counts a wrong password against the gate and says what to answer.
fn refuse(state: &AppState) -> ApiError {
    let now = Instant::now();
    state.gate.register_failure(now);
    if state.gate.is_locked_out(now) {
        ApiError::throttled(state.gate.remaining_seconds(now))
    } else {
        ApiError::unauthorized("wrong_password", "wrong password")
    }
}

async fn unlock(
    State(state): State<AppState>,
    Json(request): Json<UnlockRequest>,
) -> ApiResult<Response> {
    let redirect = local_path(request.return_to.as_deref());

    let Some(stored) = state.access.hash() else {
        // Protection removed in the meantime
        return Ok(Json(Unlocked { redirect }).into_response());
    };
    let now = Instant::now();
    if state.gate.is_locked_out(now) {
        return Err(ApiError::throttled(state.gate.remaining_seconds(now)));
    }
    if !check(request.password.clone(), stored).await {
        return Err(refuse(&state));
    }
    state.gate.register_success();

    // The password is only known here: a hash made with older costs is made again
    let access = state.clone();
    let password = request.password;
    let _ = tokio::task::spawn_blocking(move || access.access.upgrade(&password)).await;

    let stamp = stamp_for(state.access.hash().as_deref());
    let token = state.tokens.issue(&stamp, unix_now());
    let mut response = Json(Unlocked { redirect }).into_response();
    response
        .headers_mut()
        .append(header::SET_COOKIE, cookie::set(&token, state.cookie_secure));
    Ok(response)
}

async fn lock(State(state): State<AppState>) -> Response {
    let mut response = StatusCode::NO_CONTENT.into_response();
    response
        .headers_mut()
        .append(header::SET_COOKIE, cookie::clear(state.cookie_secure));
    response
}

#[derive(Deserialize)]
struct PasswordChange {
    /// The password now in force, asked again before changing it: an unlocked browser left open
    /// is not the owner.
    current: Option<String>,
    /// The new password. Absent to remove the protection.
    new: Option<String>,
}

async fn set_password(
    State(state): State<AppState>,
    Json(change): Json<PasswordChange>,
) -> ApiResult<Response> {
    let new = change
        .new
        .as_deref()
        .ok_or_else(|| ApiError::bad_request("missing_password", "no new password given"))?;
    confirm_current(&state, change.current).await?;

    let access = state.clone();
    let new = new.to_owned();
    tokio::task::spawn_blocking(move || access.access.set_password(&new))
        .await
        .map_err(|_| ApiError::internal("the password could not be set"))?
        .map_err(|error| access_error(&error))?;

    // This browser stays in; the others were unlocked for the old password
    let stamp = stamp_for(state.access.hash().as_deref());
    let token = state.tokens.issue(&stamp, unix_now());
    let mut response = StatusCode::NO_CONTENT.into_response();
    response
        .headers_mut()
        .append(header::SET_COOKIE, cookie::set(&token, state.cookie_secure));
    Ok(response)
}

async fn remove_password(
    State(state): State<AppState>,
    Json(change): Json<PasswordChange>,
) -> ApiResult<Response> {
    confirm_current(&state, change.current).await?;
    state.access.clear().map_err(|error| access_error(&error))?;
    Ok(lock(State(state)).await)
}

/// Asks for the password in force before it is changed or removed. Nothing to ask when none is set.
async fn confirm_current(state: &AppState, current: Option<String>) -> ApiResult<()> {
    if state.access.is_managed() {
        return Err(access_error(&AccessError::Managed));
    }
    let Some(stored) = state.access.hash() else {
        return Ok(());
    };
    let now = Instant::now();
    if state.gate.is_locked_out(now) {
        return Err(ApiError::throttled(state.gate.remaining_seconds(now)));
    }
    if !check(current.unwrap_or_default(), stored).await {
        return Err(refuse(state));
    }
    state.gate.register_success();
    Ok(())
}

fn access_error(error: &AccessError) -> ApiError {
    match error {
        AccessError::TooShort => ApiError::bad_request("password_too_short", error.to_string()),
        AccessError::Managed => ApiError::conflict("password_managed", error.to_string()),
        AccessError::Hashing | AccessError::Io(_) => ApiError::internal(error.to_string()),
    }
}
