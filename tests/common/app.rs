//! The whole application on a local port, and a browser to talk to it.

use std::sync::Mutex;
use std::time::Duration;

use qobuzdlx::access::{AccessStore, Gate, Tokens};
use qobuzdlx::download::{Manager, ManagerConfig, Tuning};
use qobuzdlx::qobuz::Sessions;
use qobuzdlx::settings::{Quality, SettingsStore};
use qobuzdlx::web::{self, AppState, Shared};
use reqwest::header::HeaderMap;
use serde_json::Value;
use tempfile::TempDir;
use tokio::net::TcpListener;

use super::catalogue::Catalogue;
use super::session;

pub struct TestApp {
    pub base: String,
    pub catalogue: Catalogue,
    pub state: AppState,
    pub music: TempDir,
    pub config: TempDir,
}

impl TestApp {
    /// An application with no password, signed in to a fake Qobuz.
    pub async fn start() -> Self {
        Self::start_with(None).await
    }

    /// An application protected by a password, as `ACCESS_PASSWORD` would impose it.
    pub async fn start_with(access_password: Option<&str>) -> Self {
        let catalogue = Catalogue::new().await;
        let music = TempDir::new().unwrap();
        let config = TempDir::new().unwrap();

        let settings = SettingsStore::open(config.path(), music.path());
        settings
            .update(|settings| {
                settings.download.quality = Quality::Flac16;
                settings.download.directory = music.path().to_path_buf();
            })
            .unwrap();
        let client = catalogue.client.clone();
        client.start_session(session());
        let downloads = Manager::start(ManagerConfig {
            client: client.clone(),
            settings: settings.clone(),
            config_dir: config.path().to_path_buf(),
            http: reqwest::Client::new(),
            stall_timeout: Duration::from_secs(10),
            tuning: Tuning {
                attempts_per_quality: 2,
                backoff: Duration::ZERO,
            },
        });
        let state = AppState::new(Shared {
            version: "test",
            config_dir: config.path().to_path_buf(),
            cookie_secure: false,
            access: AccessStore::open(config.path(), access_password).unwrap(),
            gate: Gate::default(),
            tokens: Tokens::open(config.path()),
            sessions: Sessions::new(client.clone(), config.path()).with_credentials(session().app),
            client,
            settings,
            downloads,
        });

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let router = web::router(state.clone());
        tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });

        Self {
            base,
            catalogue,
            state,
            music,
            config,
        }
    }

    pub fn browser(&self) -> Browser {
        Browser {
            base: self.base.clone(),
            http: reqwest::Client::builder()
                .redirect(reqwest::redirect::Policy::none())
                .build()
                .unwrap(),
            token: Mutex::new(None),
        }
    }
}

/// What a request got back.
#[derive(Debug)]
pub struct Answer {
    pub status: u16,
    pub headers: HeaderMap,
    pub text: String,
}

impl Answer {
    pub fn json(&self) -> Value {
        serde_json::from_str(&self.text)
            .unwrap_or_else(|error| panic!("not JSON ({error}): {}", self.text))
    }
}

/// A browser: it keeps the unlock cookie it is given, and sends it back.
pub struct Browser {
    base: String,
    http: reqwest::Client,
    token: Mutex<Option<String>>,
}

impl Browser {
    /// The cookie this browser holds, if any.
    pub fn token(&self) -> Option<String> {
        self.token.lock().unwrap().clone()
    }

    pub fn hold(&self, token: Option<String>) {
        *self.token.lock().unwrap() = token;
    }

    pub async fn send(
        &self,
        method: reqwest::Method,
        path: &str,
        body: Option<Value>,
        headers: &[(&str, &str)],
    ) -> Answer {
        let mut request = self.http.request(method, format!("{}{path}", self.base));
        if let Some(token) = self.token() {
            request = request.header("cookie", format!("qbdlx_access={token}"));
        }
        for (name, value) in headers {
            request = request.header(*name, *value);
        }
        if let Some(body) = body {
            request = request.json(&body);
        }

        let response = request.send().await.unwrap();
        let status = response.status().as_u16();
        let headers = response.headers().clone();
        for set in headers.get_all("set-cookie") {
            let set = set.to_str().unwrap();
            if let Some(value) = set.strip_prefix("qbdlx_access=") {
                let value = value.split(';').next().unwrap_or_default();
                self.hold((!value.is_empty()).then(|| value.to_owned()));
            }
        }
        let text = response.text().await.unwrap();
        Answer {
            status,
            headers,
            text,
        }
    }

    pub async fn get(&self, path: &str) -> Answer {
        self.send(reqwest::Method::GET, path, None, &[]).await
    }

    pub async fn post(&self, path: &str, body: Value) -> Answer {
        self.send(reqwest::Method::POST, path, Some(body), &[])
            .await
    }

    pub async fn put(&self, path: &str, body: Value) -> Answer {
        self.send(reqwest::Method::PUT, path, Some(body), &[]).await
    }

    pub async fn delete(&self, path: &str, body: Value) -> Answer {
        self.send(reqwest::Method::DELETE, path, Some(body), &[])
            .await
    }
}
