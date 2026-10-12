# QobuzDLX

A self-hosted web application that searches Qobuz and downloads what your subscription gives you into a tidy, tagged library. One small Rust executable with its interface inside, made to run in a container on a NAS or a home server.

[![License](https://img.shields.io/badge/License-MIT-green)](LICENSE)

<p align="center">
  <img src="docs/screenshots/search-light.png" alt="Searching Beethoven symphonies, results as rows with their quality" width="880">
</p>

> Qobuz is a trademark of its owner. This project is not affiliated with Qobuz. It downloads what an active subscription is entitled to: use it for your own library and respect the terms of the service.

## Where it comes from

QobuzDLX started as a fork of [thimble9057's `qobuzdlx.web`](https://gitlab.com/thimble9057/qobuzdlx.web), a Blazor Server application, and this is a rewrite of it in Rust with a new interface. The behaviour, the naming variables and many of the lessons learned (how the Qobuz API really answers, what the CDN does to a download) come from that project and from the desktop program it was itself inspired by, and the rewrite follows them closely. It is an adaptation, not an independent invention: thimble9057 is credited in the [license](LICENSE) next to my own name, and the .NET version stays available at the `legacy-dotnet` tag.

I rewrote it because I was not happy with the interface or with the language it was written in for a tool like this one, and I wanted something lighter, more powerful and nicer to use.

## Features

- **Search** albums, tracks, artists and playlists; sort by title, artist, date, quality, track count, label or duration; keep only Hi-Res; open an album or a playlist to read its tracks (discs, durations, artists), tick only the ones you want and download just those.
- **Select like in a file explorer**: shift-click takes every row from the last click to this one, ctrl-click adds or removes a row, in the results as well as in the tracks of an album or a playlist.
- **Download** albums, single tracks, playlists, whole discographies, labels and the favourites of the account, in MP3 320, FLAC 16-bit, FLAC up to 24-bit 96 kHz or FLAC up to 24-bit 192 kHz. FLAC never falls back to MP3, and the log tells the quality that was really served.
- **A queue that runs unattended.** It survives a restart: what was waiting resumes, files already on disk are recognised, and the saved Qobuz session is opened again. Reorder, skip an album, cancel an item, retry only the tracks that failed.
- **Made for large libraries.** Several tracks of an album download at once (1 to 6, 3 by default): Qobuz serves each file at a modest speed, so this is what turns weeks into days. A queue of thousands of albums stays light in the interface, an album already on disk is gone through with one request, and the queue stops by itself, with a notice, when the disk is full instead of failing every item.
- **Import a list.** A text file with one Qobuz link per line; blank lines, lines starting with `#` and links to other sites are skipped, and a link may be written without `https://`.
- **Resilient transfers.** The Qobuz CDN cuts connections after a few megabytes; a download goes on from where it stopped. Every FLAC file is decoded after the transfer to be sure it is whole, and a damaged file is fetched again.
- **Tags and covers.** Vorbis comments and ID3v2.4 with every tag switchable, embedded and saved covers at a size you choose, parental advisory markers, optional repair of the missing FLAC checksum.
- **Naming you control.** Templates with more than thirty variables for artist, album, track, compilation, playlist and disc folders, tried live on an invented release, with the problems of each template pointed out. A template can never leave the download folder.
- **Playlists** keep their order: tracks numbered by position if you wish, one `.m3u8` per playlist with relative paths, and tracks of the same name from different albums never overwrite each other.
- **Bonus files** (booklets) are downloaded next to the music.
- **A live view** of the album in progress, the speed, and a log you can filter, export and clear.
- **French and English**, two themes (dark and light, following the system or chosen by hand), usable on a phone and installable from the browser.
- **Protected access.** An optional password (Argon2id, with a progressive lock after wrong attempts and cookies signed by a key kept on disk) keeps the interface, and the Qobuz account behind it, to those who know it.

<p align="center">
  <img src="docs/screenshots/downloads-dark.png" alt="The downloads page with an album in progress" width="880">
</p>

<p align="center">
  <img src="docs/screenshots/album-tracks-dark.png" alt="An album opened onto its movements" width="430">
  <img src="docs/screenshots/settings-light.png" alt="Naming templates with a live preview" width="430">
</p>
<p align="center">
  <img src="docs/screenshots/search-phone.png" alt="The search on a phone" width="240">
</p>

## Running it

### With Docker Compose

No image is published: Compose builds it from this repository, interface and server included.

```bash
git clone https://gitlab.com/L-at-nnes/qobuzdlx.web.git
cd qobuzdlx.web
sudo docker compose up -d --build
```

Then open `http://<your server>:8080` and sign in to Qobuz. The settings, the session and the queue live in a Docker volume and survive a rebuild; to update, `git pull` and run the same `docker compose up -d --build`.

To change anything, copy `.env.example` to `.env` and edit it (all of it is optional):

| Setting | Default | Meaning |
|---|---|---|
| `PORT` | `8080` | Port of the interface on the host |
| `ACCESS_PASSWORD` | none | Password of the interface, at least 8 characters |
| `DOWNLOADS_DIR` | `downloads` | A name keeps the music in a Docker volume; a path (`/srv/music`) puts it in that folder of the host |
| `COOKIE_SECURE` | `false` | `true` behind a reverse proxy that serves the interface over HTTPS |

The application runs as an unprivileged user (uid 65532): a folder of the host given as `DOWNLOADS_DIR` has to belong to it (`sudo chown -R 65532 /srv/music`).

### With plain Docker

```bash
docker build -t qobuzdlx .
docker run -d --name qobuzdlx \
  -p 8080:8080 \
  -v qobuzdlx-config:/config \
  -v /srv/music:/downloads \
  -e ACCESS_PASSWORD='a long password' \
  qobuzdlx
```

The variables of the application, which Compose fills in for you:

| Variable | Default | Meaning |
|---|---|---|
| `BIND` | `127.0.0.1:8080` (`0.0.0.0:8080` in the image) | Address the server listens on |
| `CONFIG_PATH` | `/config` | Where settings, session, queue and keys are kept |
| `DOWNLOADS_PATH` | `/downloads` | Download folder used until you choose another in the settings |
| `ACCESS_PASSWORD` | none | Password of the interface, at least 8 characters. When set, it cannot be changed from the interface |
| `COOKIE_SECURE` | `false` | Set to `true` when the interface is served over HTTPS, to mark the cookie `Secure` |

Without a password, anyone who can reach the server can use the Qobuz account linked to it. Set one if the server is reachable from anywhere but your own machine. Behind a reverse proxy, forward the `Host` header (or `X-Forwarded-Host`) so that the check of cross-site requests recognises your address.

### From source

You need Rust (1.85 or newer) and Node.js (22 or newer).

```bash
cd web && npm ci && npm run build && cd ..
cargo run --release
```

The interface is built into `web/build` and carried inside the executable; build it before `cargo build --release`. For work on the interface, `cd web && npm run dev` serves it with hot reload and proxies the API to a server on `127.0.0.1:8080`.

Checks:

```bash
cargo fmt --check && cargo clippy --all-targets && cargo test
cd web && npm run check && npm test
```

## How it is organised

| Path | What it holds |
|---|---|
| `src/qobuz/` | The Qobuz client: sign-in, app identifiers found in the web player, signed requests, retries, pagination |
| `src/download/` | The queue, the track pipeline (quality fallback, resuming, integrity check), jobs per kind of item, the manager |
| `src/naming/`, `src/tagging/` | Folder and file names from templates; tags and covers written with `lofty` |
| `src/access/` | The password, the lock after wrong attempts, the signed cookie |
| `src/web/` | The HTTP API, the live events, and the interface served from the executable |
| `web/` | The interface (SvelteKit, built to static files) |
| `tests/` | Integration tests against a scripted fake of Qobuz and its CDN: no network, no account |

## Credits and license

Released under the [MIT license](LICENSE). Original project: [thimble9057/qobuzdlx.web](https://gitlab.com/thimble9057/qobuzdlx.web). The covers shown in the screenshots belong to their labels.
