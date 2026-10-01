//! Reading and changing the settings, trying the templates, and checking the configuration.

use std::path::Path;

use axum::extract::State;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Serialize;

use super::error::{ApiError, ApiResult};
use super::state::AppState;
use crate::naming::{Issue, IssueKind, Preview, preview, validate};
use crate::qobuz::SessionCheck;
use crate::settings::Settings;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/settings", get(read).put(save))
        .route("/settings/preview", post(try_templates))
        .route("/settings/check", get(check))
}

async fn read(State(state): State<AppState>) -> Json<Settings> {
    Json(state.settings.get())
}

#[derive(Serialize)]
struct Tried {
    preview: Preview,
    issues: Vec<Issue>,
}

/// What a draft of the settings would produce, without saving it: the page shows it as it is typed.
async fn try_templates(Json(draft): Json<Settings>) -> Json<Tried> {
    Json(Tried {
        preview: preview(&draft),
        issues: validate(&draft),
    })
}

/// Problems that make a template unusable. The others (characters that will be replaced, a variable
/// that is not known) are shown but let through: the user may mean them.
fn blocks_saving(issue: &Issue) -> bool {
    matches!(
        issue.kind,
        IssueKind::Empty | IssueKind::Rooted | IssueKind::Parent
    )
}

async fn save(
    State(state): State<AppState>,
    Json(settings): Json<Settings>,
) -> ApiResult<Json<Settings>> {
    if !settings.artwork.embedded_size.is_valid() || !settings.artwork.saved_size.is_valid() {
        return Err(ApiError::bad_request(
            "invalid_artwork_size",
            "a cover is 50 to 600 pixels wide, or the original",
        ));
    }
    let directory = &settings.download.directory;
    if directory.as_os_str().is_empty() {
        return Err(ApiError::bad_request(
            "missing_download_directory",
            "no download folder given",
        ));
    }
    if !directory.is_absolute() {
        return Err(ApiError::bad_request(
            "download_directory_not_absolute",
            "the download folder must be an absolute path",
        ));
    }
    if validate(&settings).iter().any(blocks_saving) {
        return Err(ApiError::unprocessable(
            "invalid_templates",
            "a naming template cannot be used",
        ));
    }

    state
        .settings
        .replace(settings.clone())
        .map_err(|error| ApiError::internal(format!("the settings could not be saved: {error}")))?;
    Ok(Json(settings))
}

// ─── Checking the configuration ───

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
enum Status {
    Ok,
    Error,
}

/// The verdict on one point of the configuration.
#[derive(Serialize)]
struct Verdict {
    check: &'static str,
    status: Status,
    /// What the interface words: `writable`, `not_writable`, `valid`, `rejected`...
    code: &'static str,
    detail: Option<String>,
}

impl Verdict {
    fn ok(check: &'static str, code: &'static str, detail: Option<String>) -> Self {
        Self {
            check,
            status: Status::Ok,
            code,
            detail,
        }
    }

    fn error(check: &'static str, code: &'static str, detail: Option<String>) -> Self {
        Self {
            check,
            status: Status::Error,
            code,
            detail,
        }
    }
}

/// What stops a download from succeeding is usually found track by track in the log. Here it is all
/// checked at once and named.
async fn check(State(state): State<AppState>) -> Json<Vec<Verdict>> {
    let download_folder = state.settings.get().download.directory;
    let config_folder = state.config_dir.clone();

    let (downloads, config) = tokio::task::spawn_blocking(move || {
        (
            folder_is_writable("download_folder", &download_folder),
            folder_is_writable("config_folder", &config_folder),
        )
    })
    .await
    .unwrap_or_else(|_| {
        (
            Verdict::error("download_folder", "not_writable", None),
            Verdict::error("config_folder", "not_writable", None),
        )
    });

    let (session, credentials) = session_checks(&state).await;
    Json(vec![downloads, config, session, credentials])
}

/// Creates then removes a test file: only a real write proves the permissions.
fn folder_is_writable(check: &'static str, folder: &Path) -> Verdict {
    let probe = folder.join(format!(".qbdlx-write-test-{}", std::process::id()));
    let result = std::fs::create_dir_all(folder).and_then(|()| std::fs::write(&probe, "ok"));
    let _ = std::fs::remove_file(&probe);
    match result {
        Ok(()) => Verdict::ok(check, "writable", Some(folder.display().to_string())),
        Err(error) => Verdict::error(
            check,
            "not_writable",
            Some(format!("{}: {error}", folder.display())),
        ),
    }
}

async fn session_checks(state: &AppState) -> (Verdict, Verdict) {
    let Some(session) = state.client.session() else {
        return (
            Verdict::error("session", "not_signed_in", None),
            Verdict::error("credentials", "unknown", None),
        );
    };

    let session_verdict = match state.client.verify_session(&session).await {
        SessionCheck::Valid(account) => Verdict::ok("session", "valid", account.display_name),
        SessionCheck::Rejected => Verdict::error("session", "rejected", None),
        SessionCheck::Unreachable => Verdict::error("session", "unreachable", None),
    };
    let credentials_verdict = if state.client.app_credentials_are_valid(&session.app).await {
        Verdict::ok("credentials", "accepted", None)
    } else {
        Verdict::error("credentials", "refused", None)
    };
    (session_verdict, credentials_verdict)
}
