use serde::{Deserialize, Serialize};

use super::UserInfo;

/// The pair of identifiers the Qobuz web player uses to identify itself to the API.
///
/// `app_id` accompanies every call; `app_secret` signs the ones that fetch audio.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppCredentials {
    pub app_id: String,
    pub app_secret: String,
}

/// Debug output hides the secret so credentials cannot end up in a log by accident.
impl std::fmt::Debug for AppCredentials {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("AppCredentials")
            .field("app_id", &self.app_id)
            .field("app_secret", &"<hidden>")
            .finish()
    }
}

/// Who is signed in, as the interface shows it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Account {
    pub id: u64,
    pub display_name: Option<String>,
    pub email: Option<String>,
    /// The subscription, such as "Studio" or "Sublime".
    pub plan: Option<String>,
}

impl From<&UserInfo> for Account {
    fn from(user: &UserInfo) -> Self {
        Self {
            id: user.id,
            display_name: user.display_name.clone(),
            email: user.email.clone(),
            plan: user
                .credential
                .as_ref()
                .and_then(|credential| credential.parameters.as_ref())
                .and_then(|parameters| parameters.short_label.clone()),
        }
    }
}

/// A signed-in Qobuz session.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Session {
    pub app: AppCredentials,
    pub user_auth_token: String,
    #[serde(default)]
    pub account: Account,
}

impl std::fmt::Debug for Session {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Session")
            .field("app", &self.app)
            .field("user_auth_token", &"<hidden>")
            .field("account", &self.account)
            .finish()
    }
}

/// What happened to the session, for whoever has to react: the download queue stops when the
/// session is refused and resumes when a new one starts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionEvent {
    /// A session was opened.
    Started,
    /// Qobuz answered 401 to a signed-in call: the token expired or was revoked.
    Rejected,
    /// The session was closed by the user.
    Ended,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::qobuz::{Credential, CredentialParameters};

    fn session() -> Session {
        Session {
            app: AppCredentials {
                app_id: "123".into(),
                app_secret: "very-secret".into(),
            },
            user_auth_token: "private-token".into(),
            account: Account::default(),
        }
    }

    #[test]
    fn debug_output_never_shows_the_secret_or_the_token() {
        let printed = format!("{:?}", session());

        assert!(printed.contains("123"));
        assert!(!printed.contains("very-secret"));
        assert!(!printed.contains("private-token"));
    }

    #[test]
    fn a_session_survives_a_round_trip() {
        let original = session();

        let json = serde_json::to_string(&original).unwrap();

        assert_eq!(serde_json::from_str::<Session>(&json).unwrap(), original);
    }

    #[test]
    fn a_session_file_without_an_account_still_loads() {
        let json = r#"{ "app": { "app_id": "1", "app_secret": "s" }, "user_auth_token": "t" }"#;

        assert_eq!(
            serde_json::from_str::<Session>(json).unwrap().account,
            Account::default()
        );
    }

    #[test]
    fn the_account_takes_its_plan_from_the_credential() {
        let user = UserInfo {
            id: 9,
            display_name: Some("Ada".into()),
            credential: Some(Credential {
                parameters: Some(CredentialParameters {
                    short_label: Some("Studio".into()),
                    ..Default::default()
                }),
            }),
            ..UserInfo::default()
        };

        let account = Account::from(&user);

        assert_eq!(
            (
                account.id,
                account.display_name.as_deref(),
                account.plan.as_deref()
            ),
            (9, Some("Ada"), Some("Studio"))
        );
    }
}
