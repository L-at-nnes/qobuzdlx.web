//! A scriptable HTTP server standing in for Qobuz and its CDN.

#![allow(dead_code)] // each test binary uses a different part of this module

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use axum::Router;
use axum::body::Body;
use axum::extract::{Request, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use qobuzdlx::qobuz::{Client, ClientConfig, RetryPolicy};
use std::time::Duration;
use tokio::net::TcpListener;

/// One request the server received.
#[derive(Debug, Clone)]
pub struct Call {
    pub path: String,
    pub query: HashMap<String, String>,
    pub headers: HeaderMap,
}

impl Call {
    pub fn param(&self, name: &str) -> &str {
        self.query.get(name).map_or("", String::as_str)
    }

    pub fn offset(&self) -> usize {
        self.param("offset").parse().unwrap_or(0)
    }
}

/// What the server answers.
pub struct Reply {
    status: StatusCode,
    headers: Vec<(&'static str, String)>,
    body: Vec<u8>,
}

impl Reply {
    pub fn json(value: serde_json::Value) -> Self {
        Self {
            status: StatusCode::OK,
            headers: vec![("content-type", "application/json".into())],
            body: value.to_string().into_bytes(),
        }
    }

    pub fn text(body: impl Into<String>) -> Self {
        Self {
            status: StatusCode::OK,
            headers: Vec::new(),
            body: body.into().into_bytes(),
        }
    }

    pub fn bytes(body: Vec<u8>) -> Self {
        Self {
            status: StatusCode::OK,
            headers: Vec::new(),
            body,
        }
    }

    pub fn status(code: u16) -> Self {
        Self {
            status: StatusCode::from_u16(code).unwrap(),
            headers: Vec::new(),
            body: Vec::new(),
        }
    }

    pub fn error(code: u16, message: &str) -> Self {
        Self {
            status: StatusCode::from_u16(code).unwrap(),
            ..Self::json(serde_json::json!({ "status": "error", "message": message }))
        }
    }

    pub fn with_header(mut self, name: &'static str, value: impl Into<String>) -> Self {
        self.headers.push((name, value.into()));
        self
    }
}

impl IntoResponse for Reply {
    fn into_response(self) -> Response {
        let mut response = Response::new(Body::from(self.body));
        *response.status_mut() = self.status;
        for (name, value) in self.headers {
            response.headers_mut().insert(name, value.parse().unwrap());
        }
        response
    }
}

type Handler = Box<dyn Fn(&Call) -> Reply + Send + Sync>;

#[derive(Default)]
struct Shared {
    handlers: Mutex<HashMap<String, Handler>>,
    calls: Mutex<Vec<Call>>,
}

/// A server listening on a free local port, answering what the test scripted.
pub struct FakeServer {
    url: String,
    shared: Arc<Shared>,
}

impl FakeServer {
    pub async fn start() -> Self {
        let shared = Arc::new(Shared::default());
        let app = Router::new().fallback(respond).with_state(shared.clone());
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });

        Self { url, shared }
    }

    /// The address of the server, without a trailing slash.
    pub fn url(&self) -> &str {
        &self.url
    }

    /// Answers `path` with what `handler` returns for each call.
    pub fn on(
        &self,
        path: &str,
        handler: impl Fn(&Call) -> Reply + Send + Sync + 'static,
    ) -> &Self {
        self.shared
            .handlers
            .lock()
            .unwrap()
            .insert(path.to_owned(), Box::new(handler));
        self
    }

    /// Answers `path` with the same JSON every time.
    pub fn on_json(&self, path: &str, value: serde_json::Value) -> &Self {
        self.on(path, move |_| Reply::json(value.clone()))
    }

    /// Every call received for `path`, in order.
    pub fn calls_to(&self, path: &str) -> Vec<Call> {
        self.shared
            .calls
            .lock()
            .unwrap()
            .iter()
            .filter(|call| call.path == path)
            .cloned()
            .collect()
    }

    pub fn count(&self, path: &str) -> usize {
        self.calls_to(path).len()
    }

    /// A client of this server, with retries that do not wait.
    pub fn client(&self) -> Client {
        Client::new(ClientConfig {
            api_base: format!("{}/api/", self.url),
            web_base: self.url.clone(),
            retry: RetryPolicy {
                max_attempts: 3,
                base_delay: Duration::ZERO,
                max_delay: Duration::ZERO,
            },
        })
    }
}

async fn respond(State(shared): State<Arc<Shared>>, request: Request) -> Response {
    let path = request.uri().path().to_owned();
    let query = request
        .uri()
        .query()
        .map(|query| {
            url::form_urlencoded::parse(query.as_bytes())
                .into_owned()
                .collect()
        })
        .unwrap_or_default();
    let call = Call {
        path: path.clone(),
        query,
        headers: request.headers().clone(),
    };

    shared.calls.lock().unwrap().push(call.clone());

    let handlers = shared.handlers.lock().unwrap();
    match handlers.get(&path) {
        Some(handler) => handler(&call).into_response(),
        None => Reply::error(404, "No result matching given argument").into_response(),
    }
}
