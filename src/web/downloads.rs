//! The download queue: adding to it, steering it, and following it live.

use std::convert::Infallible;
use std::time::Duration;

use axum::body::Bytes;
use axum::extract::{DefaultBodyLimit, Path, State};
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::sse::{Event as Sse, KeepAlive};
use axum::response::{IntoResponse, Response};
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use futures_util::Stream;
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use tokio::sync::broadcast::error::RecvError;

use super::dto::{ItemView, SnapshotView};
use super::error::{ApiError, ApiResult};
use super::state::AppState;
use crate::download::{Display, EnqueuedCounts, Event, Snapshot, parse_list};

/// The largest list that is read: some 100 000 lines.
const IMPORT_LIMIT: usize = 16 * 1024 * 1024;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/downloads", get(snapshot).post(enqueue))
        .route(
            "/downloads/import",
            post(import).layer(DefaultBodyLimit::max(IMPORT_LIMIT)),
        )
        .route("/downloads/events", get(events))
        .route("/downloads/cancel", post(cancel))
        .route("/downloads/skip", post(skip))
        .route("/downloads/clear-pending", post(clear_pending))
        .route("/downloads/clear-finished", post(clear_finished))
        .route("/downloads/{id}", delete(remove))
        .route("/downloads/{id}/retry", post(retry))
        .route("/downloads/{id}/retry-failures", post(retry_failures))
        .route("/downloads/{id}/move", post(move_item))
        .route("/log", get(log_lines).delete(clear_log))
        .route("/log/export", get(export_log))
}

async fn snapshot(State(state): State<AppState>) -> Response {
    Json(SnapshotView::from(&state.downloads.snapshot())).into_response()
}

#[derive(Deserialize)]
struct Request {
    items: Vec<Wanted>,
}

/// Something to download, with what the interface already knows of it for the queue to show.
#[derive(Deserialize)]
struct Wanted {
    url: String,
    label: Option<String>,
    sub_label: Option<String>,
    thumbnail: Option<String>,
}

#[derive(Serialize, Default)]
struct Counts {
    added: u32,
    requeued: u32,
    /// Already waiting or running.
    duplicates: u32,
    invalid: u32,
}

impl Counts {
    fn from_queue(counts: EnqueuedCounts, invalid: usize) -> Self {
        Self {
            added: counts.added,
            requeued: counts.requeued,
            duplicates: counts.already_queued,
            invalid: u32::try_from(invalid).unwrap_or(u32::MAX),
        }
    }
}

async fn enqueue(
    State(state): State<AppState>,
    Json(request): Json<Request>,
) -> ApiResult<Json<Counts>> {
    if request.items.is_empty() {
        return Err(ApiError::bad_request("nothing_to_add", "no address given"));
    }

    let (counts, invalid) = state
        .downloads
        .enqueue_all(request.items.into_iter().map(|wanted| {
            let display = Display {
                label: wanted.label,
                sub_label: wanted.sub_label,
                thumbnail: wanted.thumbnail,
            };
            (wanted.url, display)
        }));
    Ok(Json(Counts::from_queue(counts, invalid)))
}

/// A text file with one address per line, queued in the order of the file. What is not a Qobuz
/// address counts as `invalid`; comments and blank lines are not counted.
async fn import(State(state): State<AppState>, body: Bytes) -> ApiResult<Json<Counts>> {
    let list = parse_list(&String::from_utf8_lossy(&body));
    if list.addresses.is_empty() && list.ignored == 0 {
        return Err(ApiError::bad_request(
            "nothing_to_add",
            "the list holds no address",
        ));
    }

    let counts = state.downloads.enqueue_parsed(
        list.addresses
            .into_iter()
            .map(|address| (address, Display::default()))
            .collect(),
    );
    Ok(Json(Counts::from_queue(counts, list.ignored)))
}

async fn cancel(State(state): State<AppState>) -> StatusCode {
    state.downloads.cancel_current();
    StatusCode::NO_CONTENT
}

async fn skip(State(state): State<AppState>) -> StatusCode {
    state.downloads.skip_current();
    StatusCode::NO_CONTENT
}

async fn clear_pending(State(state): State<AppState>) -> StatusCode {
    state.downloads.clear_pending();
    StatusCode::NO_CONTENT
}

async fn clear_finished(State(state): State<AppState>) -> StatusCode {
    state.downloads.clear_finished();
    StatusCode::NO_CONTENT
}

async fn remove(State(state): State<AppState>, Path(id): Path<String>) -> StatusCode {
    state.downloads.remove(&id);
    StatusCode::NO_CONTENT
}

async fn retry(State(state): State<AppState>, Path(id): Path<String>) -> StatusCode {
    state.downloads.retry(&id);
    StatusCode::NO_CONTENT
}

async fn retry_failures(State(state): State<AppState>, Path(id): Path<String>) -> StatusCode {
    state.downloads.retry_failures(&id);
    StatusCode::NO_CONTENT
}

#[derive(Deserialize)]
struct Move {
    /// `-1` to move it up, `1` to move it down.
    direction: i8,
}

async fn move_item(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(request): Json<Move>,
) -> ApiResult<StatusCode> {
    if !matches!(request.direction, -1 | 1) {
        return Err(ApiError::bad_request(
            "bad_direction",
            "the direction is -1 (up) or 1 (down)",
        ));
    }
    state
        .downloads
        .move_item(&id, isize::from(request.direction));
    Ok(StatusCode::NO_CONTENT)
}

// ─── The log ───

async fn log_lines(State(state): State<AppState>) -> Json<Vec<crate::download::LogLine>> {
    Json(state.downloads.log_lines())
}

async fn clear_log(State(state): State<AppState>) -> StatusCode {
    state.downloads.clear_log();
    StatusCode::NO_CONTENT
}

/// The log as a text file, in English: whoever reads it may not read the language of the interface.
async fn export_log(State(state): State<AppState>) -> Response {
    let text = state.downloads.export_log(state.version);
    let stamp = OffsetDateTime::now_utc();
    let name = format!(
        "qobuzdlx-log-{:04}{:02}{:02}-{:02}{:02}{:02}.txt",
        stamp.year(),
        u8::from(stamp.month()),
        stamp.day(),
        stamp.hour(),
        stamp.minute(),
        stamp.second()
    );

    let mut response = text.into_response();
    let headers = response.headers_mut();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("text/plain; charset=utf-8"),
    );
    if let Ok(disposition) = HeaderValue::from_str(&format!("attachment; filename=\"{name}\"")) {
        headers.insert(header::CONTENT_DISPOSITION, disposition);
    }
    response
}

// ─── Following live ───

/// What the interface listens to: the queue, the progress and the log, as they change.
///
/// The first events are the state as it is, so that a page opened in the middle of a download is
/// right at once. A listener that fell behind is given the state again rather than the events it
/// missed.
async fn events(
    State(state): State<AppState>,
) -> axum::response::Sse<impl Stream<Item = Result<Sse, Infallible>>> {
    let manager = state.downloads.clone();
    let mut receiver = manager.subscribe();

    let stream = async_stream::stream! {
        yield Ok(queue_event(&manager.snapshot()));
        yield Ok(progress_event(&manager.snapshot()));

        loop {
            match receiver.recv().await {
                Ok(Event::Queue) => yield Ok(queue_event(&manager.snapshot())),
                Ok(Event::Progress) => yield Ok(progress_event(&manager.snapshot())),
                Ok(Event::Log(line)) => yield Ok(json_event("log", &line)),
                Ok(Event::SessionExpired) => yield Ok(Sse::default().event("session_expired").data("")),
                Err(RecvError::Lagged(_)) => {
                    let snapshot = manager.snapshot();
                    yield Ok(queue_event(&snapshot));
                    yield Ok(progress_event(&snapshot));
                }
                Err(RecvError::Closed) => break,
            }
        }
    };

    axum::response::Sse::new(stream).keep_alive(KeepAlive::new().interval(Duration::from_secs(15)))
}

fn json_event<T: Serialize>(name: &str, value: &T) -> Sse {
    Sse::default()
        .event(name)
        .json_data(value)
        .unwrap_or_else(|_| Sse::default().event(name).data("null"))
}

fn queue_event(snapshot: &Snapshot) -> Sse {
    #[derive(Serialize)]
    struct Queue<'a> {
        queue: Vec<ItemView<'a>>,
        remaining: usize,
    }
    json_event(
        "queue",
        &Queue {
            queue: snapshot.queue.iter().map(ItemView::from).collect(),
            remaining: snapshot.remaining,
        },
    )
}

fn progress_event(snapshot: &Snapshot) -> Sse {
    json_event("progress", &snapshot.progress)
}
