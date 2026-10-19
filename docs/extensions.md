# Extensions

Extensions let you make QobuzDLX do something more when a download happens: move or convert a file, rescan a media server, send a notification, call your own script. They react to **events**; they never sit in the way of a download.

- [How it works](#how-it-works)
- [Quick start](#quick-start)
- [The manifest](#the-manifest-extensionjson)
- [Events](#events)
- [Command extensions](#command-extensions)
- [Webhook extensions](#webhook-extensions)
- [Using extensions with Docker](#using-extensions-with-docker)
- [Built-in extensions in Rust](#built-in-extensions-in-rust)
- [Guarantees and limits](#guarantees-and-limits)
- [Security](#security)
- [Seeing what is loaded](#seeing-what-is-loaded)
- [Troubleshooting](#troubleshooting)
- [Compatibility](#compatibility)

## How it works

Three kinds of extension exist. They all receive the same events.

| Kind | What it is | Added by | Works in the container image |
|---|---|---|---|
| **Command** | A program started for each event; the event is written on its standard input | A folder with an `extension.json` | Only if the program can run there (see [Docker](#using-extensions-with-docker)) |
| **Webhook** | An HTTP address; each event is sent to it as a `POST` | A folder with an `extension.json` | Yes |
| **Built-in** | Rust code compiled into QobuzDLX | A few lines in `src/main.rs` | Yes |

The first two are added **without recompiling**: drop a folder into the extensions directory and restart. The directory is `extensions` inside the config folder (`/config/extensions` in the image), or whatever `EXTENSIONS_PATH` says.

```
/config/extensions/
├── move-to-library/        one folder per extension
│   ├── extension.json      the manifest, mandatory
│   └── run.sh              whatever the extension needs
└── notify/
    └── extension.json
```

Each extension has its own queue of events and its own task. A slow, failing or crashing extension delays or loses only its own events: it can never slow down, fail or stop a download, and it never affects another extension.

## Quick start

1. Create `extensions/hello/extension.json` in the config folder:

   ```json
   {
     "events": ["track_saved"],
     "command": ["./run.sh"]
   }
   ```

2. Next to it, `run.sh` (made executable with `chmod +x run.sh`):

   ```sh
   #!/bin/sh
   cat >> "$QOBUZDLX_EXTENSION_DIR/saved.log"
   ```

3. Restart QobuzDLX. The log says `extension loaded` with the name `hello` (taken from the folder).
4. Download an album. `saved.log` gets one line of JSON per track.

Two ready-made examples are in [`examples/extensions/`](../examples/extensions): `log-saved-tracks` (a command) and `notify-webhook` (a webhook, disabled until you set its address).

## The manifest (`extension.json`)

| Field | Type | Default | Meaning |
|---|---|---|---|
| `name` | text | the folder's name | Unique name, 1 to 40 letters, digits, `-` or `_`. Shown in logs and in the API |
| `version` | text | none | Free text, shown in the API |
| `description` | text | none | Free text, shown in the API |
| `events` | list | all events | The events wanted, by name (see [Events](#events)). Empty or absent means all of them, new ones included |
| `enabled` | true/false | `true` | `false` leaves the extension alone without deleting its folder |
| `timeout_seconds` | 1 to 3600 | `60` | Time given to handle one event: a program is killed after it, a request is given up |
| `command` | list of text | | Program and arguments (command extension) |
| `webhook` | object | | `url` (http or https) and optional `headers` (webhook extension) |

Exactly one of `command` and `webhook` is required. An unknown field, a misspelled event name or an invalid value makes the extension **fail to load**, with the reason in the log and in the API: nothing is silently ignored. Other extensions load regardless.

Folders whose name starts with `.` or `_` are skipped (a quick way to set one aside), as are folders without an `extension.json`.

## Events

Every event is one JSON object. Fields are only ever **added** in later versions, so ignore the ones you do not know. Every event has these:

| Field | Meaning |
|---|---|
| `schema` | Version of the format, currently `1` |
| `at` | Unix time, in seconds, when it was sent |
| `event` | Name of the event |

### `track_saved`

A track was downloaded, verified, tagged and moved to its final place. It is not sent for a file that was already there and left alone.

```json
{
  "schema": 1,
  "at": 1760000000,
  "event": "track_saved",
  "item_id": "3fa91c02",
  "path": "/downloads/Miles Davis/Kind of Blue/1 - So What.flac",
  "folder": "/downloads/Miles Davis/Kind of Blue",
  "track_id": 11,
  "title": "So What",
  "track_number": 1,
  "disc_number": 1,
  "artist": "Miles Davis",
  "album_id": "alb",
  "album_title": "Kind of Blue",
  "album_artist": "Miles Davis",
  "quality": "FLAC 16-bit/44.1 kHz"
}
```

`item_id` is the queue item the track belongs to. `quality` is what was **really** served, which can be lower than what was asked for. `artist` and `album_artist` can be `null`.

### `item_finished`

A queue item ended: an album, a playlist, a whole discography. Sent for completed, failed and cancelled items. Not sent when the application stops while an item runs (the item waits again at the next start).

```json
{
  "schema": 1,
  "at": 1760000090,
  "event": "item_finished",
  "item_id": "3fa91c02",
  "url": "https://play.qobuz.com/album/alb",
  "kind": "album",
  "label": "Kind of Blue",
  "status": "completed",
  "error": null,
  "downloaded": 3,
  "skipped": 0,
  "failed": 0,
  "folders": ["/downloads/Miles Davis/Kind of Blue"]
}
```

- `kind`: `album`, `track`, `playlist`, `artist`, `label`, `favorite_albums`, `favorite_tracks` or `favorite_artists`.
- `status`: `completed`, `failed` or `cancelled`. A completed item can still have `failed` tracks: check `failed`.
- `downloaded`, `skipped`, `failed`: files written, files left alone (already there or unavailable) and tracks that failed.
- `folders`: the folders that received at least one file **during this run**, sorted. This is the natural place to act on a finished album, for instance to rescan it. It is empty when nothing was written.

All the `track_saved` events of an item are sent before its `item_finished`.

### `queue_drained`

The last waiting item was processed and nothing is left to do. Nothing but the common fields:

```json
{ "schema": 1, "at": 1760000100, "event": "queue_drained" }
```

It is the right moment for something expensive that should run once after a batch: a full library scan, a backup, a report.

## Command extensions

```json
{
  "name": "move-to-library",
  "events": ["item_finished"],
  "timeout_seconds": 300,
  "command": ["./run.sh", "--target", "/mnt/music"]
}
```

**What the program receives**

- The event as **one line of JSON** on its standard input, ended by a newline. The program may read it or not.
- It runs **from its own folder**.
- Its environment is **empty** except for `PATH` (and `SYSTEMROOT` on Windows) and:
  - `QOBUZDLX_EVENT`: the name of the event, handy to decide without parsing anything;
  - `QOBUZDLX_EXTENSION_DIR`: the full path of its folder, to keep its own files.
- QobuzDLX's own variables, `ACCESS_PASSWORD` included, are **not** passed on.

**Which program**

The first element of `command` is the program. Written with a `/` or a `\` (`./run.sh`, `bin/tool`) it is looked for **from the extension's folder**; written bare (`python3`, `node`) it is looked for on the `PATH`. A script must therefore be executable and begin with a `#!` line, or be started through its interpreter: `["python3", "run.py"]`.

**Success and failure**

Exit code `0` is success. Anything else, a program that cannot be started, or a program still running after `timeout_seconds` (it is killed) is a failure: a warning in the log with the last 500 characters of what the program wrote on its error output. The event is not retried. Use that error output for your own messages.

Events are handled **one at a time**, in order, so a program is never started twice at once for the same extension. A program that needs a long time should be given a matching timeout.

A minimal Python extension:

```python
#!/usr/bin/env python3
import json, sys

event = json.loads(sys.stdin.readline())
if event["event"] == "item_finished" and event["status"] == "completed":
    for folder in event["folders"]:
        print(f"finished {folder}", file=sys.stderr)  # shows up only if it fails
```

## Webhook extensions

```json
{
  "name": "notify",
  "events": ["item_finished", "queue_drained"],
  "timeout_seconds": 10,
  "webhook": {
    "url": "http://my-service:9000/qobuzdlx",
    "headers": { "Authorization": "Bearer a-secret" }
  }
}
```

Each event is sent as a `POST` to `url`:

- the body is the JSON of the event, `Content-Type: application/json`;
- `X-QobuzDLX-Event` repeats the name of the event, so a receiver can route without parsing;
- your `headers` are added to every request.

Any `2xx` answer is success. Any other status, a connection error or no answer within `timeout_seconds` is a failure, logged as a warning and **not retried**. Answer quickly and do the real work afterwards: the extension's next event waits for the answer to this one.

The receiver can be anything: a script behind a small web server, n8n, Home Assistant, a Node-RED flow, a serverless function, another container of the same Compose file (use its service name in the address, as above). Webhooks are the way to run a heavy or complicated action from the container image.

## Using extensions with Docker

**Adding an extension.** The extensions directory is `/config/extensions`, inside the `config` volume. Either bind-mount a folder of the host on it (add this to the `volumes` of the service in `docker-compose.yml`, then `docker compose up -d`):

```yaml
      - ./extensions:/config/extensions:ro
```

or copy a folder into the volume of a running container:

```bash
docker cp ./my-extension qobuzdlx:/config/extensions/my-extension
docker restart qobuzdlx
```

The application runs as uid 65532; the files only need to be readable by it, and executable for a program.

**The image has no shell and no interpreter.** It is built on a minimal base image (`distroless`) for security and size: there is no `sh`, no Python, no `curl`. So in the image:

- a **webhook** works as it is, and is the simplest choice;
- a **command** works if it is a program that needs nothing from the system but the C library: a Rust or Go binary, for instance. A shell or Python script does not, since its interpreter is missing;
- if you want scripts, build your own image from this one's source and a base that has what you need, or run the extension as a **separate container** and reach it by webhook. The second is the cleaner way: QobuzDLX stays minimal and your extension brings its own tools.

A command extension that acts on files sees the paths **as QobuzDLX sees them** (`/downloads/...`), so it has to run where `/downloads` is the same folder, or translate the prefix.

## Built-in extensions in Rust

When you build QobuzDLX yourself you can add extensions as code. Implement `qobuzdlx::extensions::Extension` and hand it to `Extensions::start` in `src/main.rs`.

```rust
use futures_util::future::BoxFuture;
use qobuzdlx::extensions::{Event, EventKind, Extension, Failure, Info, Source};

struct Counter;

impl Extension for Counter {
    fn info(&self) -> Info {
        Info {
            name: "counter".to_owned(),
            version: Some("1.0.0".to_owned()),
            description: Some("Counts the tracks saved".to_owned()),
            source: Source::Builtin,
            events: vec![EventKind::TrackSaved], // empty: every event
        }
    }

    fn handle<'a>(&'a self, event: &'a Event) -> BoxFuture<'a, Result<(), Failure>> {
        Box::pin(async move {
            if let Event::TrackSaved { track, .. } = event {
                tracing::info!(title = %track.title, "saved");
            }
            Ok(())
        })
    }
}
```

In `src/main.rs`, next to the loading of the folders:

```rust
let mut loaded = extensions::load_dir(&config.extensions_dir);
loaded.extensions.push(std::sync::Arc::new(Counter));
let extensions = Extensions::start(loaded.extensions, loaded.problems);
```

`handle` is `async`, so it may await files, the network or a database. Return `Err(Failure("why".into()))` to report a problem: it is logged and nothing else happens. A panic is caught the same way.

Nothing outside `src/extensions/` needs to change to add an event: add a variant to `Event` and `EventKind` in `src/extensions/event.rs`, then call `emit` where it happens.

## Guarantees and limits

- **Never in the way.** Sending an event takes no time and cannot fail. Extensions run on their own tasks.
- **In order, per extension.** An extension sees events in the order they happened, one at a time. Two different extensions are independent.
- **At most once.** A failed event is not retried. After a restart, events that were still waiting are lost: extensions are told about what *happens*, they do not replay history. If you must not miss anything, make your extension idempotent and reconcile from the files on disk, or from `item_finished`.
- **Bounded backlog.** At most 256 events wait for one extension. If it is that far behind, the newest events are dropped with a warning. `track_saved` is the busiest event (one per track): subscribe only to what you need.
- **Loaded at start.** Folders are read once, when QobuzDLX starts. Adding, changing or removing an extension takes a restart. A webhook or a program may of course change its own behaviour at any time.
- **Names are unique.** If two extensions have the same name, the second is not loaded and the API says so.

## Security

Extensions are **trusted code run by the operator**, with the permissions of the QobuzDLX process: a command extension can do anything that user can do. Only install what you have read, as you would for any program. The extensions directory is not reachable from the web interface; there is no way to install an extension from the browser, on purpose.

QobuzDLX limits what it hands over: a command gets an empty environment (no access password), and events carry no Qobuz credentials, session or token. A webhook's headers are written in plain text in `extension.json`: keep that file readable only by the user running QobuzDLX if it holds a secret.

## Seeing what is loaded

`GET /api/extensions` (behind the access password like the rest of the API) returns:

```json
{
  "extensions": [
    {
      "name": "log-saved-tracks",
      "version": "1.0.0",
      "description": "Appends the path of every saved track to saved.log",
      "source": "command",
      "events": ["track_saved"]
    }
  ],
  "problems": [
    { "source": "bad-folder", "message": "extension.json is not valid: ..." }
  ]
}
```

`source` is `builtin`, `command` or `webhook`; an empty `events` means all events. At start-up the log also gets one `extension loaded` line per extension and one warning per problem.

## Troubleshooting

| Symptom | Likely cause |
|---|---|
| Nothing in the log at start | The folder is not directly under the extensions directory, has no `extension.json`, starts with `.` or `_`, or `"enabled": false` |
| `problems` lists the folder | The manifest is invalid: read the message, it names the field |
| `could not start ./run.sh` | Not executable (`chmod +x`), no `#!` line, or its interpreter is missing (in the image: see [Docker](#using-extensions-with-docker)) |
| `did not finish within N seconds` | Raise `timeout_seconds`, or do the long work in the background and return |
| `answered 404` / `connection refused` | Wrong webhook address, or the service is not reachable from the container |
| `too far behind: event dropped` | The extension is slower than the downloads: subscribe to fewer events or speed it up |
| The extension never reacts | It did not subscribe to that event, or it was not restarted after the change |

Set `RUST_LOG=qobuzdlx=debug` for more detail.

## Compatibility

The event format carries a `schema` number, `1` today. It changes only if a field is **removed or renamed**; new fields and new events keep it. So write extensions that ignore unknown fields and unknown events, and subscribe by name in the manifest. The manifest format follows the same rule: a field is never repurposed.
