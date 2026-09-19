//! Signing in.

use std::time::Duration;

use reqwest::StatusCode;

use super::client::Auth;
use super::{Account, AppCredentials, Client, Error, LoginResponse, Result, Session, UserInfo};

/// How long a saved session is given to prove it is still valid before it is assumed to be.
const VERIFY_TIMEOUT: Duration = Duration::from_secs(10);

/// The verdict on a saved session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionCheck {
    /// Qobuz accepts the token.
    Valid(Account),
    /// Qobuz refuses the token: it expired or was revoked.
    Rejected,
    /// Qobuz could not be asked (network down, timeout). Says nothing about the token: it should be
    /// kept rather than signing the user out at every hiccup.
    Unreachable,
}

impl Client {
    /// Signs in with an e-mail address and a password.
    ///
    /// The login is attempted once, and the answer is not retried: a failure means the credentials are
    /// wrong, and trying again would only try the same password twice. The returned session is not
    /// started; see [`Client::start_session`].
    pub async fn sign_in(
        &self,
        app: &AppCredentials,
        email: &str,
        password: &str,
    ) -> Result<Session> {
        let params = [
            ("email", email.to_owned()),
            ("password", password.to_owned()),
        ];
        let body = self
            .get_text_once(
                "user/login",
                &params,
                Auth::App {
                    app_id: &app.app_id,
                },
            )
            .await?;

        let login: LoginResponse = serde_json::from_str(&body)
            .map_err(|error| Error::Unexpected(format!("user/login: {error}")))?;
        let user_auth_token = login
            .user_auth_token
            .filter(|token| !token.is_empty())
            .ok_or_else(|| Error::Unexpected("the login answer holds no token".to_owned()))?;

        Ok(Session {
            app: app.clone(),
            user_auth_token,
            account: login.user.as_ref().map(Account::from).unwrap_or_default(),
        })
    }

    /// Signs in with a token taken from the web player, after checking that Qobuz accepts it.
    pub async fn sign_in_with_token(&self, app: &AppCredentials, token: &str) -> Result<Session> {
        let user = self.fetch_user(&app.app_id, token).await?;

        Ok(Session {
            app: app.clone(),
            user_auth_token: token.to_owned(),
            account: Account::from(&user),
        })
    }

    /// Asks Qobuz whether a saved session is still valid.
    pub async fn verify_session(&self, session: &Session) -> SessionCheck {
        let check = self.fetch_user(&session.app.app_id, &session.user_auth_token);

        match tokio::time::timeout(VERIFY_TIMEOUT, check).await {
            Ok(Ok(user)) => SessionCheck::Valid(Account::from(&user)),
            Ok(Err(error))
                if matches!(
                    error.status(),
                    Some(StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN)
                ) =>
            {
                SessionCheck::Rejected
            }
            Ok(Err(_)) | Err(_) => SessionCheck::Unreachable,
        }
    }

    async fn fetch_user(&self, app_id: &str, token: &str) -> Result<UserInfo> {
        self.get_json("user/get", &[], Auth::User { app_id, token })
            .await
    }
}
