//! A small Qobuz: albums, tracks, playlists and the rest, answered from tables the test fills.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use qobuzdlx::qobuz::Client;
use serde_json::{Value, json};

use super::{FakeServer, Reply, TINY_JPEG, flac_bytes, signed_in};

pub type Table<K> = Arc<Mutex<HashMap<K, Value>>>;

pub struct Catalogue {
    pub server: FakeServer,
    /// A client of the server that is signed in.
    pub client: Client,
    pub albums: Table<String>,
    pub tracks: Table<u64>,
    pub playlists: Table<u64>,
    pub artists: Table<u64>,
    pub labels: Table<u64>,
    pub favorites: Table<String>,
}

fn serve<K: std::hash::Hash + Eq + std::str::FromStr + Send + 'static>(
    server: &FakeServer,
    path: &str,
    param: &'static str,
    table: &Table<K>,
) where
    K::Err: std::fmt::Debug,
{
    let table = Arc::clone(table);
    server.on(path, move |call| {
        let key: K = call.param(param).parse().unwrap();
        match table.lock().unwrap().get(&key) {
            Some(value) => Reply::json(value.clone()),
            None => Reply::error(404, "No result matching given argument"),
        }
    });
}

/// A page of Qobuz's, complete.
pub fn page(items: impl AsRef<[Value]>) -> Value {
    let items = items.as_ref();
    json!({ "offset": 0, "limit": 500, "total": items.len(), "items": items })
}

impl Catalogue {
    /// The server answers for every kind of thing, and serves the same short FLAC for every track
    /// and the same tiny picture for every cover.
    pub async fn new() -> Self {
        let (server, client) = signed_in().await;
        let catalogue = Self {
            albums: Table::default(),
            tracks: Table::default(),
            playlists: Table::default(),
            artists: Table::default(),
            labels: Table::default(),
            favorites: Table::default(),
            server,
            client,
        };
        let server = &catalogue.server;
        serve(server, "/api/album/get", "album_id", &catalogue.albums);
        serve(server, "/api/track/get", "track_id", &catalogue.tracks);
        serve(
            server,
            "/api/playlist/get",
            "playlist_id",
            &catalogue.playlists,
        );
        serve(server, "/api/artist/get", "artist_id", &catalogue.artists);
        serve(server, "/api/label/get", "label_id", &catalogue.labels);
        serve(
            server,
            "/api/favorite/getUserFavorites",
            "type",
            &catalogue.favorites,
        );

        let audio = flac_bytes();
        server.on("/cdn/audio", move |_| Reply::bytes(audio.clone()));
        server.on("/cdn/cover", |_| Reply::bytes(TINY_JPEG.to_vec()));
        let url = format!("{}/cdn/audio", server.url());
        server.on("/api/track/getFileUrl", move |call| {
            Reply::json(json!({
                "url": url,
                "format_id": call.param("format_id").parse::<u32>().unwrap(),
                "bit_depth": 16,
                "sampling_rate": 44.1,
            }))
        });
        catalogue
    }

    fn cover_url(&self, album_id: &str) -> String {
        format!("{}/cdn/cover?{album_id}_600.jpg", self.server.url())
    }

    /// An album of the given tracks (id, number, title), by an artist.
    pub fn add_album(&self, id: &str, title: &str, artist: &str, tracks: &[(u64, u32, &str)]) {
        let items: Vec<Value> = tracks
            .iter()
            .map(|(track_id, number, name)| {
                json!({
                    "id": track_id, "title": name, "track_number": number, "media_number": 1,
                    "duration": 100, "performer": { "id": 1, "name": artist },
                })
            })
            .collect();
        self.albums.lock().unwrap().insert(
            id.to_owned(),
            json!({
                "id": id, "title": title, "artist": { "id": 1, "name": artist },
                "tracks_count": tracks.len(), "media_count": 1, "streamable": true,
                "image": { "large": self.cover_url(id) },
                "tracks": page(&items),
            }),
        );
        for (track_id, number, name) in tracks {
            self.tracks.lock().unwrap().insert(
                *track_id,
                json!({
                    "id": track_id, "title": name, "track_number": number, "media_number": 1,
                    "duration": 100, "performer": { "id": 1, "name": artist },
                    "album": { "id": id, "title": title, "artist": { "id": 1, "name": artist } },
                }),
            );
        }
    }

    pub fn change_album(&self, id: &str, change: impl FnOnce(&mut Value)) {
        change(self.albums.lock().unwrap().get_mut(id).unwrap());
    }

    /// A playlist of tracks already added to albums, as (track id, position).
    pub fn add_playlist(&self, id: u64, name: &str, entries: &[(u64, u32)]) {
        let items: Vec<Value> = entries
            .iter()
            .map(|(track_id, position)| {
                let mut track = self.tracks.lock().unwrap().get(track_id).cloned().unwrap();
                track["position"] = json!(position);
                track["playlist_track_id"] = json!(5_000_000_000u64 + track_id);
                track
            })
            .collect();
        self.playlists.lock().unwrap().insert(
            id,
            json!({ "id": id, "name": name, "tracks_count": entries.len(), "tracks": page(&items) }),
        );
    }
}
