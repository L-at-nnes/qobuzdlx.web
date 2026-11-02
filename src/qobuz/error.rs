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

    /// What stands in front of the API refused the request before Qobuz saw it: a 403 whose body is
    /// not a Qobuz error. This is what an address Qobuz does not serve (a VPN, a datacenter) gets,
    /// for every call, whatever the account. It says nothing about the session or the credentials.
    #[error(
        "Qobuz refuses the requests of this network (HTTP {}): a VPN or a datacenter address is the usual cause",
        .status.as_u16()
    )]
    Blocked { status: StatusCode },

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
        // The API always answers in JSON, its refusals included: a 403 that is anything else comes
        // from the edge in front of it
        if status == StatusCode::FORBIDDEN && !is_json_object(body) {
            return Self::Blocked { status };
        }
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

    /// Qobuz could not be asked at all: the network is down, the edge refuses this address or
    /// answers with a gateway error. Nothing was decided about what was asked, and asking again
    /// later is the only thing to do.
    #[must_use]
    pub fn is_unreachable(&self) -> bool {
        match self {
            Self::Network(_) | Self::Blocked { .. } => true,
            Self::Api { status, .. } => is_gateway_status(*status),
            _ => false,
        }
    }

    /// The edge in front of Qobuz refuses this network.
    #[must_use]
    pub fn is_blocked(&self) -> bool {
        matches!(self, Self::Blocked { .. })
    }

    /// The HTTP status of an API error. An answer of the edge is not one.
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
            Self::Network(_) | Self::Blocked { .. } => true,
            Self::Api { status, .. } => is_transient_status(*status),
            _ => false,
        }
    }
}

/// Statuses worth retrying: Qobuz answers 429 when asked too much, and its edge briefly answers
/// 502/503/504 now and then. Anything else (401, 404, 500...) will answer the same way again.
pub(super) fn is_transient_status(status: StatusCode) -> bool {
    status == StatusCode::TOO_MANY_REQUESTS || is_gateway_status(status)
}

fn is_gateway_status(status: StatusCode) -> bool {
    matches!(
        status,
        StatusCode::BAD_GATEWAY | StatusCode::SERVICE_UNAVAILABLE | StatusCode::GATEWAY_TIMEOUT
    )
}

fn is_json_object(body: &str) -> bool {
    serde_json::from_str::<serde_json::Value>(body).is_ok_and(|json| json.is_object())
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
    fn a_403_that_is_not_from_the_api_is_the_edge_refusing_the_network() {
        let page = "<HTML><HEAD><TITLE>Access Denied</TITLE></HEAD><BODY>...</BODY></HTML>";
        let blocked = Error::from_response(StatusCode::FORBIDDEN, page);

        assert!(blocked.is_blocked());
        assert!(blocked.is_unreachable());
        assert_eq!(blocked.status(), None, "not an answer of the API");
        assert!(!blocked.is_session_gone());
        assert!(blocked.to_string().contains("403"));
        assert!(!blocked.to_string().contains("Access Denied"));

        assert!(Error::from_response(StatusCode::FORBIDDEN, "").is_blocked());
    }

    #[test]
    fn a_403_of_the_api_is_an_answer_like_any_other() {
        let refused = Error::from_response(
            StatusCode::FORBIDDEN,
            r#"{"status":"error","code":403,"message":"Not allowed"}"#,
        );

        assert!(!refused.is_blocked());
        assert!(!refused.is_unreachable());
        assert_eq!(refused.status(), Some(StatusCode::FORBIDDEN));
    }

    #[test]
    fn a_gateway_error_is_unreachable_and_a_refusal_is_not() {
        for status in [502, 503, 504] {
            let error = Error::from_response(StatusCode::from_u16(status).unwrap(), "");
            assert!(error.is_unreachable(), "{status}");
        }
        for status in [400, 401, 404, 429, 500] {
            let error = Error::from_response(StatusCode::from_u16(status).unwrap(), "");
            assert!(!error.is_unreachable(), "{status}");
        }
    }

    #[test]
    fn only_401_means_the_session_was_refused() {
        assert!(Error::from_response(StatusCode::UNAUTHORIZED, "").is_unauthorized());
        assert!(!Error::from_response(StatusCode::FORBIDDEN, "{}").is_unauthorized());
    }

    #[test]
    fn a_missing_session_is_as_good_as_a_refused_one() {
        assert!(Error::NotSignedIn.is_session_gone());
        assert!(Error::from_response(StatusCode::UNAUTHORIZED, "").is_session_gone());
        assert!(!Error::from_response(StatusCode::FORBIDDEN, "{}").is_session_gone());
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
