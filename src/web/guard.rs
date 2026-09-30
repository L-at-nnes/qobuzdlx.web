//! What every request goes through before reaching a handler: headers that limit what a page can
//! do, the check that a change comes from this site, and the check that the browser has unlocked.

use std::time::{SystemTime, UNIX_EPOCH};

use axum::extract::{Request, State};
use axum::http::{HeaderMap, HeaderValue, Method, header};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};

use super::cookie;
use super::error::ApiError;
use super::state::AppState;
use crate::access::{Validity, stamp_for};

/// The policy of every page. The interface is served by this server and loads nothing else but the
/// covers, which come from Qobuz's image servers. `frame-ancestors` forbids framing the interface in
/// someone else's page to trick a click.
const CONTENT_SECURITY_POLICY: &str = "default-src 'self'; img-src 'self' data: https:; \
    style-src 'self' 'unsafe-inline'; script-src 'self' 'unsafe-inline'; connect-src 'self'; \
    base-uri 'self'; form-action 'self'; frame-ancestors 'none'";

pub fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs())
}

/// Put on every response, the static files included.
pub async fn security_headers(request: Request, next: Next) -> Response {
    let is_api = request.uri().path().starts_with("/api/");
    let mut response = next.run(request).await;

    let headers = response.headers_mut();
    headers.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    headers.insert(header::X_FRAME_OPTIONS, HeaderValue::from_static("DENY"));
    headers.insert(
        header::CONTENT_SECURITY_POLICY,
        HeaderValue::from_static(CONTENT_SECURITY_POLICY),
    );
    headers.insert(
        header::REFERRER_POLICY,
        HeaderValue::from_static("same-origin"),
    );
    if is_api {
        // Answers are about the account of whoever asks: no shared cache keeps them
        headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    }
    response
}

/// Refuses a change that does not come from this site.
///
/// The cookie is `SameSite=Lax`, which already keeps it off cross-site `POST`s; this is the second
/// lock. A browser says where a request comes from (`Origin`, `Sec-Fetch-Site`) and a page of another
/// site cannot forge those headers.
pub async fn same_origin(request: Request, next: Next) -> Response {
    let changes_something = !matches!(
        *request.method(),
        Method::GET | Method::HEAD | Method::OPTIONS
    );
    if changes_something && !comes_from_this_site(request.headers()) {
        return ApiError::forbidden("cross_site", "this request does not come from this site")
            .into_response();
    }
    next.run(request).await
}

fn comes_from_this_site(headers: &HeaderMap) -> bool {
    let text = |name: &str| headers.get(name).and_then(|value| value.to_str().ok());

    if let Some(site) = text("sec-fetch-site")
        && site != "same-origin"
        && site != "none"
    {
        return false;
    }

    let Some(origin) = text("origin") else {
        // Not a browser, or an old one: nothing to compare
        return true;
    };
    // `null` is what privacy-sensitive contexts send
    let Some(origin_host) = origin.split_once("://").map(|(_, host)| host) else {
        return false;
    };

    // Behind a proxy the host the browser asked for may only be in X-Forwarded-Host. A cross-site
    // page cannot set it, so honouring it cannot let one in.
    ["host", "x-forwarded-host"]
        .into_iter()
        .filter_map(text)
        .any(|host| host.eq_ignore_ascii_case(origin_host))
}

/// Whether the browser may use the application.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unlock {
    /// No password is set: everyone may.
    Open,
    /// The browser holds a good token.
    Unlocked,
    /// The browser holds a good token that should be replaced by a newer one.
    Refresh,
    Locked,
}

impl Unlock {
    #[must_use]
    pub fn allows(self) -> bool {
        self != Self::Locked
    }
}

pub fn unlock_of(state: &AppState, headers: &HeaderMap) -> Unlock {
    let hash = state.access.hash();
    if hash.is_none() {
        return Unlock::Open;
    }
    let Some(token) = cookie::read(headers) else {
        return Unlock::Locked;
    };
    match state
        .tokens
        .check(&token, &stamp_for(hash.as_deref()), unix_now())
    {
        Validity::Valid => Unlock::Unlocked,
        Validity::Refresh => Unlock::Refresh,
        Validity::Invalid => Unlock::Locked,
    }
}

/// Lets a request through only if the browser has unlocked the application (or nothing locks it).
pub async fn require_unlock(
    State(state): State<AppState>,
    request: Request,
    next: Next,
) -> Response {
    let unlock = unlock_of(&state, request.headers());
    if !unlock.allows() {
        return ApiError::unauthorized("locked", "the application is locked").into_response();
    }

    let mut response = next.run(request).await;
    if unlock == Unlock::Refresh {
        // A device in use never has to unlock again
        let stamp = stamp_for(state.access.hash().as_deref());
        let token = state.tokens.issue(&stamp, unix_now());
        response
            .headers_mut()
            .append(header::SET_COOKIE, cookie::set(&token, state.cookie_secure));
    }
    response
}

#[cfg(test)]
mod tests {
    use super::*;

    fn headers(pairs: &[(&str, &str)]) -> HeaderMap {
        let mut headers = HeaderMap::new();
        for (name, value) in pairs {
            headers.insert(
                axum::http::HeaderName::from_bytes(name.as_bytes()).unwrap(),
                HeaderValue::from_str(value).unwrap(),
            );
        }
        headers
    }

    #[test]
    fn a_request_without_origin_is_let_through() {
        assert!(comes_from_this_site(&headers(&[("host", "music.example")])));
    }

    #[test]
    fn a_request_from_the_same_site_is_let_through() {
        assert!(comes_from_this_site(&headers(&[
            ("host", "music.example:8080"),
            ("origin", "http://music.example:8080"),
            ("sec-fetch-site", "same-origin"),
        ])));
    }

    #[test]
    fn a_request_from_another_site_is_refused() {
        assert!(!comes_from_this_site(&headers(&[
            ("host", "music.example"),
            ("origin", "https://evil.example"),
        ])));
        assert!(!comes_from_this_site(&headers(&[
            ("host", "music.example"),
            ("sec-fetch-site", "cross-site"),
        ])));
        assert!(!comes_from_this_site(&headers(&[
            ("host", "music.example"),
            ("sec-fetch-site", "same-site"),
        ])));
    }

    #[test]
    fn an_origin_that_is_not_an_address_is_refused() {
        assert!(!comes_from_this_site(&headers(&[
            ("host", "music.example"),
            ("origin", "null"),
        ])));
    }

    #[test]
    fn behind_a_proxy_the_forwarded_host_counts() {
        assert!(comes_from_this_site(&headers(&[
            ("host", "10.0.0.5:8080"),
            ("x-forwarded-host", "music.example"),
            ("origin", "https://music.example"),
        ])));
    }

    #[test]
    fn the_host_is_compared_without_regard_to_case() {
        assert!(comes_from_this_site(&headers(&[
            ("host", "Music.Example"),
            ("origin", "https://music.example"),
        ])));
    }
}
