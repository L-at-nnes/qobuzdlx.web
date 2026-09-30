//! Who is signed in to Qobuz.

use axum::extract::State;
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};

use super::error::{ApiError, ApiResult};
use super::state::AppState;
use crate::qobuz::{Account, SignInError};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/session", get(current))
        .route("/session/login", post(login))
        .route("/session/token", post(login_with_token))
        .route("/session/logout", post(logout))
}

#[derive(Serialize)]
struct Current {
    signed_in: bool,
    account: Option<Account>,
}

async fn current(State(state): State<AppState>) -> Json<Current> {
    let account = state.sessions.account();
    Json(Current {
        signed_in: account.is_some(),
        account,
    })
}

#[derive(Deserialize)]
struct Credentials {
    email: String,
    password: String,
}

async fn login(
    State(state): State<AppState>,
    Json(credentials): Json<Credentials>,
) -> ApiResult<Json<Account>> {
    let email = credentials.email.trim();
    if email.is_empty() || credentials.password.is_empty() {
        return Err(ApiError::bad_request(
            "missing_credentials",
            "an e-mail address and a password are needed",
        ));
    }
    state
        .sessions
        .sign_in(email, &credentials.password)
        .await
        .map(Json)
        .map_err(|error| sign_in_error(error, "wrong_credentials"))
}

#[derive(Deserialize)]
struct TokenRequest {
    token: String,
}

async fn login_with_token(
    State(state): State<AppState>,
    Json(request): Json<TokenRequest>,
) -> ApiResult<Json<Account>> {
    if request.token.trim().is_empty() {
        return Err(ApiError::bad_request("missing_token", "no token given"));
    }
    state
        .sessions
        .sign_in_with_token(&request.token)
        .await
        .map(Json)
        .map_err(|error| sign_in_error(error, "invalid_token"))
}

async fn logout(State(state): State<AppState>) -> StatusCode {
    state.sessions.sign_out();
    StatusCode::NO_CONTENT
}

/// Qobuz answers 401 to a sign-in when what was given is wrong: that is the user's to fix, not a
/// session that expired.
fn sign_in_error(error: SignInError, wrong: &'static str) -> ApiError {
    match error {
        SignInError::NoCredentials => ApiError::unavailable(
            "qobuz_identifiers_unavailable",
            "could not find the identifiers the Qobuz web player uses; try again later",
        ),
        SignInError::Qobuz(error)
            if error
                .status()
                .is_some_and(|status| matches!(status.as_u16(), 400 | 401 | 403)) =>
        {
            ApiError::unauthorized(wrong, error.to_string())
        }
        SignInError::Qobuz(error) => error.into(),
    }
}
