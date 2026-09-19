use std::sync::{Arc, RwLock};
use std::time::Duration;

use reqwest::Response;
use reqwest::header::RETRY_AFTER;
use serde::de::DeserializeOwned;
use tokio::sync::broadcast;

use super::error::is_transient_status;
use super::{Error, Session, SessionEvent};

/// Result of a call to Qobuz.
pub type Result<T> = std::result::Result<T, Error>;

const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0 Safari/537.36";

/// Catalogue calls answer in well under a second and the largest transfer is a few megabytes of
/// JavaScript: a longer wait only means Qobuz stopped answering.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

/// How transient failures (429 and 5xx from the edge) are retried.
#[derive(Debug, Clone, Copy)]
pub struct RetryPolicy {
    /// Attempts in total, the first one included.
    pub max_attempts: u32,
    /// Wait before the second attempt when Qobuz gives no `Retry-After`; it doubles after that.
    pub base_delay: Duration,
    /// Longest wait honoured, whatever `Retry-After` says.
    pub max_delay: Duration,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            base_delay: Duration::from_secs(1),
            max_delay: Duration::from_secs(30),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ClientConfig {
    /// Base of the JSON API, with a trailing slash.
    pub api_base: String,
    /// Base of the web player, where the app credentials are found.
    pub web_base: String,
    pub retry: RetryPolicy,
}

impl Default for ClientConfig {
    fn default() -> Self {
        Self {
            api_base: "https://www.qobuz.com/api.json/0.2/".to_owned(),
            web_base: "https://play.qobuz.com".to_owned(),
            retry: RetryPolicy::default(),
        }
    }
}

/// How a request identifies itself.
#[derive(Debug, Clone, Copy)]
pub(super) enum Auth<'a> {
    /// The app only, as before signing in.
    App { app_id: &'a str },
    /// The app and the signed-in user.
    User { app_id: &'a str, token: &'a str },
}

/// A client of the Qobuz API. Cloning is cheap: clones share the connection pool and the session.
#[derive(Debug, Clone)]
pub struct Client {
    inner: Arc<Inner>,
}

#[derive(Debug)]
struct Inner {
    http: reqwest::Client,
    config: ClientConfig,
    session: RwLock<Option<Session>>,
    events: broadcast::Sender<SessionEvent>,
}

impl Client {
    /// Builds a client for the given endpoints.
    ///
    /// # Panics
    ///
    /// Panics when the TLS backend cannot be initialised, which cannot be recovered from.
    #[must_use]
    pub fn new(config: ClientConfig) -> Self {
        let http = reqwest::Client::builder()
            .user_agent(USER_AGENT)
            .timeout(REQUEST_TIMEOUT)
            .gzip(true)
            .build()
            .expect("the TLS backend failed to initialise");
        let (events, _) = broadcast::channel(16);

        Self {
            inner: Arc::new(Inner {
                http,
                config,
                session: RwLock::new(None),
                events,
            }),
        }
    }

    /// A client of the real Qobuz.
    #[must_use]
    pub fn production() -> Self {
        Self::new(ClientConfig::default())
    }

    // ─── Session ───

    /// The current session, if someone is signed in.
    #[must_use]
    pub fn session(&self) -> Option<Session> {
        self.inner
            .session
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }

    #[must_use]
    pub fn is_signed_in(&self) -> bool {
        self.session().is_some()
    }

    /// Starts a session. Whoever listens is told, so that what waited for one can resume.
    pub fn start_session(&self, session: Session) {
        *self
            .inner
            .session
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(session);
        self.notify(SessionEvent::Started);
    }

    /// Ends the session, on the user's request or because Qobuz refused it.
    pub fn end_session(&self) {
        let had_session = self
            .inner
            .session
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take()
            .is_some();
        if had_session {
            self.notify(SessionEvent::Ended);
        }
    }

    /// Listens to what happens to the session.
    #[must_use]
    pub fn subscribe(&self) -> broadcast::Receiver<SessionEvent> {
        self.inner.events.subscribe()
    }

    fn notify(&self, event: SessionEvent) {
        // Nobody listening is not a problem
        let _ = self.inner.events.send(event);
    }

    // ─── Requests ───

    /// Calls an endpoint as the signed-in user and reads the JSON answer.
    ///
    /// A 401 means the token is no longer valid: it is announced to the listeners of the session
    /// (see [`Client::subscribe`]) before the error is returned.
    pub(super) async fn signed_in_get<T: DeserializeOwned>(
        &self,
        endpoint: &str,
        params: &[(&str, String)],
    ) -> Result<T> {
        let session = self.session().ok_or(Error::NotSignedIn)?;
        let auth = Auth::User {
            app_id: &session.app.app_id,
            token: &session.user_auth_token,
        };

        match self.get_json(endpoint, params, auth).await {
            Err(error) if error.is_unauthorized() => {
                self.notify(SessionEvent::Rejected);
                Err(error)
            }
            other => other,
        }
    }

    pub(super) async fn get_json<T: DeserializeOwned>(
        &self,
        endpoint: &str,
        params: &[(&str, String)],
        auth: Auth<'_>,
    ) -> Result<T> {
        let body = self.get_text(endpoint, params, auth).await?;
        serde_json::from_str(&body)
            .map_err(|error| Error::Unexpected(format!("{endpoint}: {error}")))
    }

    /// Calls an endpoint and returns the body of a successful answer.
    pub(super) async fn get_text(
        &self,
        endpoint: &str,
        params: &[(&str, String)],
        auth: Auth<'_>,
    ) -> Result<String> {
        self.get_text_with(endpoint, params, auth, self.inner.config.retry)
            .await
    }

    /// Like [`Client::get_text`] but never replays the request: for logins, where a second attempt
    /// would be a second try of the same password.
    pub(super) async fn get_text_once(
        &self,
        endpoint: &str,
        params: &[(&str, String)],
        auth: Auth<'_>,
    ) -> Result<String> {
        let once = RetryPolicy {
            max_attempts: 1,
            ..self.inner.config.retry
        };
        self.get_text_with(endpoint, params, auth, once).await
    }

    async fn get_text_with(
        &self,
        endpoint: &str,
        params: &[(&str, String)],
        auth: Auth<'_>,
        policy: RetryPolicy,
    ) -> Result<String> {
        let url = format!("{}{endpoint}", self.inner.config.api_base);
        let response = self.send_with_retry(&url, params, auth, policy).await?;
        Self::successful_body(endpoint, response).await
    }

    pub(super) async fn successful_body(what: &str, response: Response) -> Result<String> {
        let status = response.status();
        let body = response.text().await?;

        if status.is_success() {
            return Ok(body);
        }

        // The body is kept for the logs only: it is never part of the error shown to the user
        tracing::warn!(call = what, status = status.as_u16(), body = %body, "Qobuz answered with an error");
        Err(Error::from_response(status, &body))
    }

    async fn send_with_retry(
        &self,
        url: &str,
        params: &[(&str, String)],
        auth: Auth<'_>,
        policy: RetryPolicy,
    ) -> Result<Response> {
        let mut attempt = 0;
        loop {
            attempt += 1;
            let response = self.request(url, params, auth).send().await?;
            if attempt >= policy.max_attempts || !is_transient_status(response.status()) {
                return Ok(response);
            }

            let delay = retry_delay(&response, attempt, policy);
            tracing::warn!(
                status = response.status().as_u16(),
                delay_ms = u64::try_from(delay.as_millis()).unwrap_or(u64::MAX),
                attempt,
                max = policy.max_attempts,
                "Qobuz asked to wait, retrying"
            );
            tokio::time::sleep(delay).await;
        }
    }

    fn request(
        &self,
        url: &str,
        params: &[(&str, String)],
        auth: Auth<'_>,
    ) -> reqwest::RequestBuilder {
        let request = self.inner.http.get(url).query(params);

        // Identification goes in the headers and in the query, as the web player does it
        match auth {
            Auth::App { app_id } => request
                .header("X-App-Id", app_id)
                .query(&[("app_id", app_id)]),
            Auth::User { app_id, token } => request
                .header("X-App-Id", app_id)
                .header("X-User-Auth-Token", token)
                .query(&[("app_id", app_id)]),
        }
    }

    /// Fetches a page of the web player: a plain GET with no identification.
    pub(super) async fn get_web_text(&self, path: &str) -> Result<String> {
        let response = self
            .inner
            .http
            .get(format!("{}{path}", self.inner.config.web_base))
            .send()
            .await?;
        Self::successful_body(path, response).await
    }
}

/// `Retry-After` in seconds when Qobuz gives one, otherwise 1 s, 2 s, 4 s... Never more than the
/// policy's maximum.
fn retry_delay(response: &Response, attempt: u32, policy: RetryPolicy) -> Duration {
    let advised = response
        .headers()
        .get(RETRY_AFTER)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.trim().parse::<u64>().ok())
        .map(Duration::from_secs);

    let exponent = attempt.saturating_sub(1).min(16);
    advised
        .unwrap_or_else(|| policy.base_delay.saturating_mul(1 << exponent))
        .min(policy.max_delay)
}
