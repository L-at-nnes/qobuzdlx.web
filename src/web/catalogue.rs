//! Searching Qobuz, and looking inside albums, playlists and artists.

use axum::extract::{Path, Query, State};
use axum::routing::get;
use axum::{Json, Router};
use serde::Deserialize;

use super::dto::{AlbumItem, ArtistItem, PlaylistItem, Results, TrackItem};
use super::error::{ApiError, ApiResult};
use super::state::AppState;
use crate::qobuz::Page;

/// A page of results is as large as the interface asks, within reason.
const DEFAULT_LIMIT: u32 = 50;
const MAX_LIMIT: u32 = 100;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/search/{kind}", get(search))
        .route("/albums/{id}/tracks", get(album_tracks))
        .route("/playlists/{id}/tracks", get(playlist_tracks))
        .route("/artists/{id}/albums", get(artist_albums))
}

#[derive(Deserialize)]
struct SearchQuery {
    q: String,
    offset: Option<u32>,
    limit: Option<u32>,
}

fn page_of<T, U: serde::Serialize>(page: &Page<T>, convert: impl Fn(&T) -> U) -> serde_json::Value {
    let results = Results {
        total: page.total,
        offset: page.offset,
        items: page.items.iter().map(convert).collect::<Vec<U>>(),
    };
    serde_json::to_value(results).unwrap_or(serde_json::Value::Null)
}

async fn search(
    State(state): State<AppState>,
    Path(kind): Path<String>,
    Query(query): Query<SearchQuery>,
) -> ApiResult<Json<serde_json::Value>> {
    let text = query.q.trim();
    if text.is_empty() {
        return Err(ApiError::bad_request(
            "empty_query",
            "nothing to search for",
        ));
    }
    let limit = query.limit.unwrap_or(DEFAULT_LIMIT).clamp(1, MAX_LIMIT);
    let offset = query.offset.unwrap_or(0);
    let client = &state.client;

    let answer = match kind.as_str() {
        "albums" => page_of(&client.search_albums(text, limit, offset).await?, |album| {
            AlbumItem::from(album)
        }),
        "tracks" => page_of(&client.search_tracks(text, limit, offset).await?, |track| {
            TrackItem::from(track)
        }),
        "artists" => page_of(
            &client.search_artists(text, limit, offset).await?,
            |artist| ArtistItem::from(artist),
        ),
        "playlists" => page_of(
            &client.search_playlists(text, limit, offset).await?,
            |playlist| PlaylistItem::from(playlist),
        ),
        _ => {
            return Err(ApiError::not_found(
                "unknown_kind",
                "search albums, tracks, artists or playlists",
            ));
        }
    };
    Ok(Json(answer))
}

async fn album_tracks(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> ApiResult<Json<Vec<TrackItem>>> {
    let album = state.client.album(&id).await?;
    let tracks = album
        .tracks
        .as_ref()
        .map(|page| page.items.iter().map(TrackItem::from).collect())
        .unwrap_or_default();
    Ok(Json(tracks))
}

async fn playlist_tracks(
    State(state): State<AppState>,
    Path(id): Path<u64>,
) -> ApiResult<Json<Vec<TrackItem>>> {
    let playlist = state.client.playlist(id).await?;
    let tracks = playlist
        .tracks
        .as_ref()
        .map(|page| page.items.iter().map(TrackItem::from).collect())
        .unwrap_or_default();
    Ok(Json(tracks))
}

async fn artist_albums(
    State(state): State<AppState>,
    Path(id): Path<u64>,
) -> ApiResult<Json<Vec<AlbumItem>>> {
    let artist = state.client.artist(id).await?;
    let albums = artist
        .albums
        .as_ref()
        .map(|page| page.items.iter().map(AlbumItem::from).collect())
        .unwrap_or_default();
    Ok(Json(albums))
}
