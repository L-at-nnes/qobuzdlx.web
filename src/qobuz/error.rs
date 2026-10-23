use reqwest::StatusCode;

/// Longest part of an error message Qobuz wrote that is shown to the user.
const MAX_DETAIL_LEN: usize = 200;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Qobuz answered with an error status.
    ///
    /// The message is meant for the user: the status and, when the body is a Qobuz JSON error, its
    /// `message`. The raw body is only ever written to the logs, because it can be an HTML page from
    /// a proxy or contain credentials echoed back.
    #[error("{message}")]
    Api { status: StatusCode, message: String },

    /// The request never got an answer: network down, timeout, TLS failure.
    #[error("could not reach Qobuz: {}", crate::cause::describe(.0))]
    Network(reqwest::Error),

    /// A call that needs a session was made without one.
    #[error("not signed in to Qobuz")]
    NotSignedIn,

    /// A call that needs the app secret was made before it was known.
    #[error("the Qobuz app secret is not set")]
    MissingAppSecret,

    /// Qobuz answered 200 with something that is not what was asked for.
    #[error("unexpected answer from Qobuz: {0}")]
    Unexpected(String),
}

impl From<reqwest::Error> for Error {
    /// The URL is removed from the error: login requests carry the password in their query.
    fn from(error: reqwest::Error) -> Self {
        Self::Network(error.without_url())
    }
}

impl Error {
    /// Builds the error for a failed response from its status and raw body.
    pub(super) fn from_response(status: StatusCode, body: &str) -> Self {
        Self::Api {
            status,
            message: describe(status, body),
        }
    }

    /// Qobuz refused the session token: it expired or was revoked.
    #[must_use]
    pub fn is_unauthorized(&self) -> bool {
        matches!(self, Self::Api { status, .. } if *status == StatusCode::UNAUTHORIZED)
    }

    /// Qobuz answered "too many requests", and kept answering it through the retries.
    #[must_use]
    pub fn is_rate_limited(&self) -> bool {
        matches!(self, Self::Api { status, .. } if *status == StatusCode::TOO_MANY_REQUESTS)
    }

    /// There is no session to ask with any more: Qobuz refused the token, or nobody is signed in.
    /// Nothing can be asked until someone signs in, and what was being asked has not failed.
    #[must_use]
    pub fn is_session_gone(&self) -> bool {
        self.is_unauthorized() || matches!(self, Self::NotSignedIn)
    }

    /// The HTTP status of an API error.
    #[must_use]
    pub fn status(&self) -> Option<StatusCode> {
        match self {
            Self::Api { status, .. } => Some(*status),
            Self::Network(error) => error.status(),
            _ => None,
        }
    }

    /// Whether trying again later could work, as opposed to an answer that will not change.
    #[must_use]
    pub fn is_transient(&self) -> bool {
        match self {
            Self::Network(_) => true,
            Self::Api { status, .. } => is_transient_status(*status),
            _ => false,
        }
    }
}

/// Statuses worth retrying: Qobuz answers 429 when asked too much, and its edge briefly answers
/// 502/503/504 now and then. Anything else (401, 404, 500...) will answer the same way again.
pub(super) fn is_transient_status(status: StatusCode) -> bool {
    matches!(
        status,
        StatusCode::TOO_MANY_REQUESTS
            | StatusCode::BAD_GATEWAY
            | StatusCode::SERVICE_UNAVAILABLE
            | StatusCode::GATEWAY_TIMEOUT
    )
}

fn describe(status: StatusCode, body: &str) -> String {
    let mut description = format!("Qobuz API error {}", status.as_u16());

    let detail = serde_json::from_str::<serde_json::Value>(body)
        .ok()
        .and_then(|json| {
            json.get("message")?
                .as_str()
                .map(str::trim)
                .map(str::to_owned)
        })
        .filter(|message| !message.is_empty());

    if let Some(detail) = detail {
        description.push_str(": ");
        match detail.char_indices().nth(MAX_DETAIL_LEN) {
            Some((end, _)) => {
                description.push_str(&detail[..end]);
                description.push('…');
            }
            None => description.push_str(&detail),
        }
    }

    description
}

#[cfg(test)]
mod tests {
    use super::*;

    fn message(status: u16, body: &str) -> String {
        Error::from_response(StatusCode::from_u16(status).unwrap(), body).to_string()
    }

    #[test]
    fn a_qobuz_json_error_keeps_its_message() {
        assert_eq!(
            message(
                404,
                r#"{"status":"error","code":404,"message":"No result matching given argument"}"#
            ),
            "Qobuz API error 404: No result matching given argument"
        );
    }

    #[test]
    fn a_blank_message_is_left_out() {
        assert_eq!(message(400, r#"{"message":"   "}"#), "Qobuz API error 400");
    }

    #[test]
    fn an_html_page_from_a_proxy_is_never_echoed() {
        assert_eq!(
            message(
                502,
                "<html><body>Bad gateway from nginx 1.2.3</body></html>"
            ),
            "Qobuz API error 502"
        );
    }

    #[test]
    fn an_empty_body_gives_the_status_alone() {
        assert_eq!(message(500, ""), "Qobuz API error 500");
    }

    #[test]
    fn a_json_body_without_a_message_gives_the_status_alone() {
        assert_eq!(
            message(403, r#"{"user_auth_token":"secret"}"#),
            "Qobuz API error 403"
        );
    }

    #[test]
    fn a_long_message_is_cut_on_a_character_boundary() {
        let long = "é".repeat(300);

        let text = message(400, &format!(r#"{{"message":"{long}"}}"#));

        assert!(text.ends_with('…'));
        assert_eq!(
            text.chars().count(),
            "Qobuz API error 400: ".chars().count() + MAX_DETAIL_LEN + 1
        );
    }

    #[test]
    fn only_401_means_the_session_was_refused() {
        assert!(Error::from_response(StatusCode::UNAUTHORIZED, "").is_unauthorized());
        assert!(!Error::from_response(StatusCode::FORBIDDEN, "").is_unauthorized());
    }

    #[test]
    fn a_missing_session_is_as_good_as_a_refused_one() {
        assert!(Error::NotSignedIn.is_session_gone());
        assert!(Error::from_response(StatusCode::UNAUTHORIZED, "").is_session_gone());
        assert!(!Error::from_response(StatusCode::FORBIDDEN, "").is_session_gone());
        assert!(!Error::NotSignedIn.is_unauthorized());
    }

    #[test]
    fn only_429_is_a_rate_limit() {
        assert!(Error::from_response(StatusCode::TOO_MANY_REQUESTS, "").is_rate_limited());
        assert!(!Error::from_response(StatusCode::SERVICE_UNAVAILABLE, "").is_rate_limited());
        assert!(!Error::NotSignedIn.is_rate_limited());
    }

    #[test]
    fn rate_limits_and_gateway_errors_are_transient() {
        for status in [429, 502, 503, 504] {
            assert!(
                Error::from_response(StatusCode::from_u16(status).unwrap(), "").is_transient(),
                "{status}"
            );
        }
        for status in [400, 401, 404, 500] {
            assert!(
                !Error::from_response(StatusCode::from_u16(status).unwrap(), "").is_transient(),
                "{status}"
            );
        }
    }
}
