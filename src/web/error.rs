use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::Serialize;

use crate::qobuz;

/// What a request that went wrong answers: a status, a code the interface can translate and a
/// message in English for whoever reads the network tab.
///
/// The message never carries what Qobuz said verbatim beyond its own short explanation, and never an
/// address or a token: see [`qobuz::Error`].
#[derive(Debug)]
pub struct ApiError {
    status: StatusCode,
    code: &'static str,
    message: String,
    retry_after: Option<u64>,
}

#[derive(Serialize)]
struct Body<'a> {
    error: &'a str,
    message: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    retry_after: Option<u64>,
}

impl ApiError {
    fn new(status: StatusCode, code: &'static str, message: impl Into<String>) -> Self {
        Self {
            status,
            code,
            message: message.into(),
            retry_after: None,
        }
    }

    pub fn bad_request(code: &'static str, message: impl Into<String>) -> Self {
        Self::new(StatusCode::BAD_REQUEST, code, message)
    }

    pub fn unauthorized(code: &'static str, message: impl Into<String>) -> Self {
        Self::new(StatusCode::UNAUTHORIZED, code, message)
    }

    pub fn forbidden(code: &'static str, message: impl Into<String>) -> Self {
        Self::new(StatusCode::FORBIDDEN, code, message)
    }

    pub fn not_found(code: &'static str, message: impl Into<String>) -> Self {
        Self::new(StatusCode::NOT_FOUND, code, message)
    }

    pub fn conflict(code: &'static str, message: impl Into<String>) -> Self {
        Self::new(StatusCode::CONFLICT, code, message)
    }

    pub fn unavailable(code: &'static str, message: impl Into<String>) -> Self {
        Self::new(StatusCode::SERVICE_UNAVAILABLE, code, message)
    }

    pub fn internal(message: impl Into<String>) -> Self {
        Self::new(StatusCode::INTERNAL_SERVER_ERROR, "internal", message)
    }

    /// The gate is closed: try again in `seconds`.
    #[must_use]
    pub fn throttled(seconds: u64) -> Self {
        Self {
            retry_after: Some(seconds),
            ..Self::new(
                StatusCode::TOO_MANY_REQUESTS,
                "throttled",
                "too many wrong passwords, wait before trying again",
            )
        }
    }
}

impl From<qobuz::Error> for ApiError {
    /// A refused token is the user's to fix (sign in again); the rest is Qobuz's or the network's.
    fn from(error: qobuz::Error) -> Self {
        match &error {
            qobuz::Error::NotSignedIn => {
                Self::unauthorized("qobuz_signed_out", "not signed in to Qobuz")
            }
            qobuz::Error::Api { .. } if error.is_unauthorized() => {
                Self::unauthorized("qobuz_session_expired", error.to_string())
            }
            qobuz::Error::Api { status, .. } if status.as_u16() == 404 => {
                Self::not_found("qobuz_not_found", error.to_string())
            }
            _ => Self::new(
                StatusCode::BAD_GATEWAY,
                "qobuz_unavailable",
                error.to_string(),
            ),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let body = Body {
            error: self.code,
            message: &self.message,
            retry_after: self.retry_after,
        };
        let mut response = (self.status, Json(body)).into_response();
        if let Some(seconds) = self.retry_after
            && let Ok(value) = seconds.to_string().parse()
        {
            response.headers_mut().insert("retry-after", value);
        }
        response
    }
}

pub type ApiResult<T> = Result<T, ApiError>;

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::to_bytes;

    async fn body_of(error: ApiError) -> (StatusCode, serde_json::Value, Response) {
        let response = error.into_response();
        let status = response.status();
        let (parts, body) = response.into_parts();
        let bytes = to_bytes(body, 10_000).await.unwrap();
        let json = serde_json::from_slice(&bytes).unwrap();
        (
            status,
            json,
            Response::from_parts(parts, axum::body::Body::empty()),
        )
    }

    #[tokio::test]
    async fn an_error_answers_a_code_and_a_message() {
        let (status, json, _) = body_of(ApiError::bad_request("bad_thing", "it was bad")).await;

        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(json["error"], "bad_thing");
        assert_eq!(json["message"], "it was bad");
        assert!(json.get("retry_after").is_none());
    }

    #[tokio::test]
    async fn a_throttled_request_says_when_to_come_back_in_the_body_and_the_header() {
        let (status, json, response) = body_of(ApiError::throttled(31)).await;

        assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(json["retry_after"], 31);
        assert_eq!(response.headers()["retry-after"], "31");
    }

    #[tokio::test]
    async fn a_refused_qobuz_token_asks_for_a_new_sign_in() {
        let refused = qobuz::Error::Api {
            status: reqwest::StatusCode::UNAUTHORIZED,
            message: "Qobuz API error 401: expired".into(),
        };

        let (status, json, _) = body_of(refused.into()).await;

        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(json["error"], "qobuz_session_expired");
    }

    #[tokio::test]
    async fn a_qobuz_outage_is_a_bad_gateway_not_a_server_fault() {
        let down = qobuz::Error::Api {
            status: reqwest::StatusCode::SERVICE_UNAVAILABLE,
            message: "Qobuz API error 503".into(),
        };

        let (status, json, _) = body_of(down.into()).await;

        assert_eq!(status, StatusCode::BAD_GATEWAY);
        assert_eq!(json["error"], "qobuz_unavailable");
    }

    #[tokio::test]
    async fn a_missing_thing_at_qobuz_is_not_found() {
        let missing = qobuz::Error::Api {
            status: reqwest::StatusCode::NOT_FOUND,
            message: "Qobuz API error 404: No result".into(),
        };

        let (status, _, _) = body_of(missing.into()).await;

        assert_eq!(status, StatusCode::NOT_FOUND);
    }
}
