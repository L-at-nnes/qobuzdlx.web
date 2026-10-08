# syntax=docker/dockerfile:1

# ── The interface ──
FROM node:22-alpine AS web
WORKDIR /web
COPY web/package.json web/package-lock.json ./
RUN npm ci --no-audit --no-fund
COPY web/ ./
RUN npm run build

# ── The server, with the interface inside ──
FROM rust:1-bookworm AS build
RUN apt-get update \
 && apt-get install -y --no-install-recommends cmake \
 && rm -rf /var/lib/apt/lists/*
WORKDIR /src

# Dependencies first, so that a change to the code does not download and compile them again
COPY Cargo.toml Cargo.lock ./
RUN mkdir src && echo 'fn main() {}' > src/main.rs && touch src/lib.rs \
 && cargo build --release \
 && rm -rf src

COPY src ./src
COPY --from=web /web/build ./web/build
RUN touch src/main.rs src/lib.rs && cargo build --release

# The folders the application writes to belong to the user it runs as
RUN mkdir -p /rootfs/config /rootfs/downloads

# ── What runs ──
FROM gcr.io/distroless/cc-debian12:nonroot
ENV BIND=0.0.0.0:8080 \
    CONFIG_PATH=/config \
    DOWNLOADS_PATH=/downloads
COPY --from=build --chown=nonroot:nonroot /rootfs/config /config
COPY --from=build --chown=nonroot:nonroot /rootfs/downloads /downloads
COPY --from=build /src/target/release/qobuzdlx /usr/local/bin/qobuzdlx

VOLUME ["/config", "/downloads"]
EXPOSE 8080
HEALTHCHECK --interval=30s --timeout=5s --start-period=10s \
  CMD ["/usr/local/bin/qobuzdlx", "--healthcheck"]
ENTRYPOINT ["/usr/local/bin/qobuzdlx"]
