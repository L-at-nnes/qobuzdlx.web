# Changelog

All notable changes to QobuzDLX are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [2.0.0] - Unreleased

QobuzDLX is rewritten in Rust, with a new interface. It does what version 1 did and fixes the problems
that version 1 had accumulated. Settings, session and queue are stored in new files (see below): version 1
files are not read, so settings have to be entered again and the Qobuz account signed in again. The last .NET version is the
`legacy-dotnet` tag.

### Added
- **A new interface.** SvelteKit, built to static files and carried inside the executable. Two themes
  (a warm dark one and a warm light one) that follow the system or are chosen by hand, a text serif for
  the titles of music so that long classical titles stay readable, and layouts for a phone (bottom bar,
  safe areas, touch targets).
- **Search by albums, tracks, artists and playlists**, with sorting (title, artist, date, quality, track
  count, label, duration), a Hi-Res filter, "Show more", selection of several releases, and rows that
  open onto their tracks (discs, durations, artists and albums of playlist tracks). The search is kept
  per browser tab, not shared between devices.
- **A live view of the queue over server-sent events**: the album in progress with its cover, genre,
  label and quality, the overall progress, the track in progress and its speed, and a log that can be
  filtered by level, exported and cleared.
- **Resuming interrupted downloads.** The Qobuz CDN cuts a connection after a few seconds and a few
  megabytes; a transfer now goes on from the byte where it stopped (`Range`) with the same address and
  only gives up when connections stop bringing anything.
- **Integrity check in pure Rust.** Every FLAC file is decoded after the download (frame checksums, the
  checksum of the audio when there is one, the number of samples). A damaged file is fetched again, then
  tried in a lower quality. No `flac` program is needed any more.
- **Missing FLAC checksum written without re-encoding.** The MD5 of the audio is computed and the 16
  bytes of the header are rewritten.
- **A login screen for the access password** with a progressive lock (5 wrong passwords, then 30 s,
  1 min, 5 min, 15 min), Argon2id hashes, cookies signed with a key kept on disk (30 days, sliding),
  rejection of cross-site changes (`Origin`, `Sec-Fetch-Site`) and security headers on every answer.
- **Settings page** with the naming templates tried live on an invented release, a list of variables to
  insert, the problems of each template pointed out, and a configuration check.
- **`--healthcheck`** for the container, and a `Dockerfile` that builds the interface and the server.
- Quality chips, "downloaded" marks and greyed buttons for what is already in the queue.

### Changed
- **The server is a single small executable** (tokio, axum, reqwest with rustls). Tags are written with
  `lofty`; nothing else needs to be installed.
- **Files kept apart**: `settings.json`, `session.json`, `access.json`, `queue.json` and `cookie-key.json`,
  each written atomically with a `.bak` and set aside as `.corrupt-<time>` if it cannot be read. Secrets are
  in files only their owner can read and never appear in settings, logs or error messages.
- **The log is a list of events**, worded by the interface in its language; the exported file is in
  English.
- **An item's settings are those of the moment it starts**, so a folder never gets two naming schemes.
- **The server listens on `127.0.0.1` unless `BIND` says otherwise**; the image sets `0.0.0.0:8080`.
- **A session refused by Qobuz ends the item at once**, forgets the saved session and leaves what is
  waiting in the queue for the next sign-in, instead of failing every track.

### Fixed (carried over from version 1 and covered by tests)
- A cover that is not a picture (an error page served as one) is refused, and a cover that cannot be
  embedded no longer costs the track.
- Track numbers are padded to the size of the album, or of the playlist when playlist positions are used.
- Two tracks of a playlist with the same name no longer overwrite each other; each playlist file lists
  every file, including those recovered by a retry.
- Templates cannot leave the download folder (`..`, absolute paths), a `/` in data never makes a
  sub-folder, names are cut to the 255 bytes a filesystem accepts, and a variable that does not exist is
  left as written.
- A track of 30 seconds (Qobuz's preview) is never filed as a whole track.
- Qobuz's limit of 500 results per call is handled for albums, playlists, artists, labels and favourites.
- The quality named in the log is the one Qobuz really served, not the one asked for. FLAC never falls
  back to MP3.
- Partial files live in a folder of their own and are removed after a failure, a cancel and at start-up.

### Removed
- The Android application, `docker-compose.yml` (the image is built by hand for now), the dependency on
  the `flac` program, and the per-device search state kept on the server.
