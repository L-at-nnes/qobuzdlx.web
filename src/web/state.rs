use std::ops::Deref;
use std::sync::Arc;

use crate::access::{AccessStore, Gate, Tokens};
use crate::download::Manager;
use crate::qobuz::{Client, Sessions};
use crate::settings::SettingsStore;

/// Everything the handlers share.
#[derive(Clone, Debug)]
pub struct AppState(Arc<Shared>);

#[derive(Debug)]
pub struct Shared {
    pub version: &'static str,
    /// Whether the unlock cookie is marked `Secure` (`COOKIE_SECURE`).
    pub cookie_secure: bool,
    pub access: AccessStore,
    pub gate: Gate,
    pub tokens: Tokens,
    pub client: Client,
    pub sessions: Sessions,
    pub settings: SettingsStore,
    pub downloads: Manager,
}

impl AppState {
    #[must_use]
    pub fn new(shared: Shared) -> Self {
        Self(Arc::new(shared))
    }
}

impl Deref for AppState {
    type Target = Shared;

    fn deref(&self) -> &Shared {
        &self.0
    }
}
