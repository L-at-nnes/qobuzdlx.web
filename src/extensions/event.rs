//! What extensions are told: the events of the application, as they are sent to them.

use serde::{Deserialize, Serialize};

use crate::download::{ItemKind, ItemStatus};

/// Version of the JSON that extensions receive. It changes only when a field is removed or renamed;
/// adding a field or an event does not change it, so extensions must ignore what they do not know.
pub const SCHEMA: u32 = 1;

/// The kinds of event an extension can subscribe to, by the name it is written with in a manifest.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventKind {
    /// A track has been written to its place.
    TrackSaved,
    /// A queue item has ended, well or not.
    ItemFinished,
    /// The last waiting item has been processed.
    QueueDrained,
}

impl EventKind {
    /// The name used in manifests, in the JSON and in `QOBUZDLX_EVENT`.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::TrackSaved => "track_saved",
            Self::ItemFinished => "item_finished",
            Self::QueueDrained => "queue_drained",
        }
    }
}

/// A track written to disk, tagged and in its final place.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TrackSaved {
    /// Full path of the file.
    pub path: String,
    /// Folder that holds it.
    pub folder: String,
    pub track_id: u64,
    pub title: String,
    pub track_number: u32,
    pub disc_number: u32,
    pub artist: Option<String>,
    pub album_id: String,
    pub album_title: String,
    pub album_artist: Option<String>,
    /// The quality that was really served, such as `FLAC 24-bit/96 kHz`.
    pub quality: String,
}

/// A queue item that has ended.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ItemFinished {
    pub item_id: String,
    pub url: String,
    pub kind: ItemKind,
    pub label: String,
    pub status: ItemStatus,
    pub error: Option<String>,
    /// Files written, files left alone and tracks that failed.
    pub downloaded: u32,
    pub skipped: u32,
    pub failed: u32,
    /// The folders that received at least one file during this run of the item.
    pub folders: Vec<String>,
}

/// Something that happened in the application.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum Event {
    TrackSaved {
        /// The queue item the track was downloaded for.
        item_id: String,
        #[serde(flatten)]
        track: TrackSaved,
    },
    ItemFinished(ItemFinished),
    QueueDrained,
}

impl Event {
    #[must_use]
    pub const fn kind(&self) -> EventKind {
        match self {
            Self::TrackSaved { .. } => EventKind::TrackSaved,
            Self::ItemFinished(_) => EventKind::ItemFinished,
            Self::QueueDrained => EventKind::QueueDrained,
        }
    }

    /// The JSON an extension receives: the event, the schema version and the time it happened.
    #[must_use]
    pub fn to_json(&self, at: u64) -> String {
        #[derive(Serialize)]
        struct Envelope<'a> {
            schema: u32,
            at: u64,
            #[serde(flatten)]
            event: &'a Event,
        }
        serde_json::to_string(&Envelope {
            schema: SCHEMA,
            at,
            event: self,
        })
        .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};

    fn saved() -> TrackSaved {
        TrackSaved {
            path: "/music/A/B/01.flac".into(),
            folder: "/music/A/B".into(),
            track_id: 7,
            title: "Song".into(),
            track_number: 1,
            disc_number: 1,
            artist: Some("A".into()),
            album_id: "abc".into(),
            album_title: "B".into(),
            album_artist: None,
            quality: "FLAC 16-bit/44.1 kHz".into(),
        }
    }

    fn json_of(event: &Event) -> Value {
        serde_json::from_str(&event.to_json(42)).unwrap()
    }

    #[test]
    fn a_track_event_is_flat_with_its_item_and_the_envelope() {
        let value = json_of(&Event::TrackSaved {
            item_id: "i1".into(),
            track: saved(),
        });

        assert_eq!(value["schema"], 1);
        assert_eq!(value["at"], 42);
        assert_eq!(value["event"], "track_saved");
        assert_eq!(value["item_id"], "i1");
        assert_eq!(value["path"], "/music/A/B/01.flac");
        assert_eq!(value["album_artist"], Value::Null);
    }

    #[test]
    fn an_item_event_names_its_status_and_kind() {
        let value = json_of(&Event::ItemFinished(ItemFinished {
            item_id: "i1".into(),
            url: "https://play.qobuz.com/album/abc".into(),
            kind: ItemKind::Album,
            label: "B".into(),
            status: ItemStatus::Completed,
            error: None,
            downloaded: 3,
            skipped: 0,
            failed: 1,
            folders: vec!["/music/A/B".into()],
        }));

        assert_eq!(value["event"], "item_finished");
        assert_eq!(value["status"], "completed");
        assert_eq!(value["kind"], "album");
        assert_eq!(value["folders"], json!(["/music/A/B"]));
    }

    #[test]
    fn the_queue_drained_event_carries_only_the_envelope() {
        assert_eq!(
            json_of(&Event::QueueDrained),
            json!({"schema": 1, "at": 42, "event": "queue_drained"})
        );
    }

    #[test]
    fn the_name_of_a_kind_is_the_one_in_the_json() {
        let event = Event::QueueDrained;
        assert_eq!(json_of(&event)["event"], event.kind().name());
        assert_eq!(EventKind::TrackSaved.name(), "track_saved");
        assert_eq!(EventKind::ItemFinished.name(), "item_finished");
    }
}
