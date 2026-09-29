//! Signing in and out, and remembering the session across restarts.

use std::path::Path;
use std::sync::{Mutex, PoisonError};
use std::time::Duration;

use super::{Account, AppCredentials, Client, Error, Session, SessionCheck, SessionEvent};
use crate::storage::{Access, JsonFile};

const FILE_NAME: &str = "session.json";

#[derive(Debug, thiserror::Error)]
pub enum SignInError {
    /// No app id and secret that Qobuz accepts could be found, so nobody can sign in.
    #[error("could not find the identifiers the Qobuz web player uses; try again later")]
    NoCredentials,
    #[error("{0}")]
    Qobuz(#[from] Error),
}

/// What became of a saved session at start-up.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Restored {
    /// There was none.
    Nothing,
    /// Qobuz accepted it: the session is open.
    Resumed,
    /// Qobuz refused it: it is forgotten, and a sign-in is needed.
    Refused,
    /// Qobuz could not be reached, however many times it was tried: the session is kept for later.
    Unreachable,
}

/// Who is signed in to Qobuz, and the session kept on disk so that a restart does not sign anyone out.
///
/// The session holds the token of the account: it is kept in a file only its owner can read.
#[derive(Debug)]
pub struct Sessions {
    client: Client,
    file: JsonFile,
    credentials: Mutex<Option<AppCredentials>>,
}

impl Sessions {
    #[must_use]
    pub fn new(client: Client, config_dir: &Path) -> Self {
        Self {
            client,
            file: JsonFile::new(config_dir.join(FILE_NAME), Access::OwnerOnly),
            credentials: Mutex::new(None),
        }
    }

    /// Uses these app identifiers instead of looking for them on the Qobuz web player.
    #[must_use]
    pub fn with_credentials(self, credentials: AppCredentials) -> Self {
        *self.credentials() = Some(credentials);
        self
    }

    fn credentials(&self) -> std::sync::MutexGuard<'_, Option<AppCredentials>> {
        self.credentials
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    }

    /// The account that is signed in.
    #[must_use]
    pub fn account(&self) -> Option<Account> {
        self.client.session().map(|session| session.account)
    }

    /// The identifiers of the web player, found once and kept.
    async fn app_credentials(&self) -> Result<AppCredentials, SignInError> {
        if let Some(known) = self.credentials().clone() {
            return Ok(known);
        }
        let found = self
            .client
            .resolve_app_credentials()
            .await
            .ok_or(SignInError::NoCredentials)?;
        *self.credentials() = Some(found.clone());
        Ok(found)
    }

    /// Signs in with an e-mail address and a password.
    pub async fn sign_in(&self, email: &str, password: &str) -> Result<Account, SignInError> {
        let app = self.app_credentials().await?;
        let session = self.client.sign_in(&app, email, password).await?;
        Ok(self.open(session))
    }

    /// Signs in with a token taken from the web player.
    pub async fn sign_in_with_token(&self, token: &str) -> Result<Account, SignInError> {
        let app = self.app_credentials().await?;
        let session = self.client.sign_in_with_token(&app, token.trim()).await?;
        Ok(self.open(session))
    }

    /// Opens the session and keeps it.
    fn open(&self, session: Session) -> Account {
        if let Err(error) = self.file.save(&session) {
            tracing::warn!(%error, "the session could not be saved: it ends with this run");
        }
        let account = session.account.clone();
        self.client.start_session(session);
        account
    }

    /// Ends the session and forgets it.
    pub fn sign_out(&self) {
        self.forget();
        self.client.end_session();
    }

    /// Removes the saved session.
    fn forget(&self) {
        match std::fs::remove_file(self.file.path()) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => tracing::warn!(%error, "the saved session could not be removed"),
        }
    }

    /// Opens the saved session again, if Qobuz still accepts it.
    ///
    /// A refusal means the token is stale. Anything else (Qobuz unreachable, a timeout) says nothing
    /// about it, and nobody should be signed out at every network hiccup: it is tried again, `attempts`
    /// times, `delay` apart.
    pub async fn restore(&self, attempts: u32, delay: Duration) -> Restored {
        let Some(session) = self.file.load::<Session>() else {
            return Restored::Nothing;
        };

        for attempt in 1..=attempts {
            if self.client.is_signed_in() {
                // Someone signed in from the interface in the meantime
                return Restored::Resumed;
            }
            match self.client.verify_session(&session).await {
                SessionCheck::Valid(account) => {
                    self.client.start_session(Session { account, ..session });
                    tracing::info!("the saved Qobuz session is open again");
                    return Restored::Resumed;
                }
                SessionCheck::Rejected => {
                    self.forget();
                    tracing::warn!("the saved Qobuz session was refused: a sign-in is needed");
                    return Restored::Refused;
                }
                SessionCheck::Unreachable => {
                    tracing::warn!(
                        attempt,
                        attempts,
                        "Qobuz is unreachable, trying again shortly"
                    );
                    if attempt < attempts {
                        tokio::time::sleep(delay).await;
                    }
                }
            }
        }
        Restored::Unreachable
    }

    /// Forgets the saved session when Qobuz refuses it while it is in use. Runs until the process
    /// ends.
    pub async fn forget_when_refused(&self) {
        let mut events = self.client.subscribe();
        loop {
            match events.recv().await {
                Ok(SessionEvent::Rejected) => self.forget(),
                Ok(_) | Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {}
                Err(tokio::sync::broadcast::error::RecvError::Closed) => return,
            }
        }
    }
}
