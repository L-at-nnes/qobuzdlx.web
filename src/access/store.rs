use std::path::Path;
use std::sync::{RwLock, RwLockReadGuard, RwLockWriteGuard};

use serde::{Deserialize, Serialize};

use super::password;
use crate::config::MIN_ACCESS_PASSWORD_LEN;
use crate::storage::{Access, JsonFile};

const FILE_NAME: &str = "access.json";

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("the password must be at least {MIN_ACCESS_PASSWORD_LEN} characters long")]
    TooShort,
    /// `ACCESS_PASSWORD` is set: the interface cannot change what the operator imposes.
    #[error("the password is set by the ACCESS_PASSWORD environment variable")]
    Managed,
    #[error("the password could not be hashed")]
    Hashing,
    #[error("the password could not be saved: {0}")]
    Io(#[from] std::io::Error),
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default)]
struct Stored {
    /// An Argon2 PHC string. The password itself is never kept.
    password_hash: Option<String>,
}

/// The password that protects the interface.
///
/// It lives in its own file, readable by its owner only, apart from the settings: those can be
/// shared or pasted in a bug report without leaking anything.
#[derive(Debug)]
pub struct AccessStore {
    file: JsonFile,
    hash: RwLock<Option<String>>,
    managed: bool,
}

impl AccessStore {
    /// Opens the store. When the operator imposes a password (`ACCESS_PASSWORD`) it is applied:
    /// the stored hash is only replaced if it no longer matches, because hashing again at every
    /// start would change the salt and unlock nobody's device twice.
    pub fn open(config_dir: &Path, imposed: Option<&str>) -> Result<Self, Error> {
        let file = JsonFile::new(config_dir.join(FILE_NAME), Access::OwnerOnly);
        let stored: Stored = file.load().unwrap_or_default();
        let store = Self {
            file,
            hash: RwLock::new(stored.password_hash),
            managed: imposed.is_some(),
        };

        if let Some(password) = imposed {
            let matches = store
                .hash()
                .is_some_and(|hash| password::verify(password, &hash));
            if !matches {
                store.save(Some(hash_of(password)?))?;
                tracing::info!("access password set from the ACCESS_PASSWORD environment variable");
            }
        }
        Ok(store)
    }

    fn read(&self) -> RwLockReadGuard<'_, Option<String>> {
        self.hash
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    fn write(&self) -> RwLockWriteGuard<'_, Option<String>> {
        self.hash
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// The hash of the password, if the interface is protected.
    #[must_use]
    pub fn hash(&self) -> Option<String> {
        self.read().clone()
    }

    #[must_use]
    pub fn is_protected(&self) -> bool {
        self.read().is_some()
    }

    /// The password is imposed by the environment and cannot be changed from the interface.
    #[must_use]
    pub fn is_managed(&self) -> bool {
        self.managed
    }

    /// Sets a new password, which unlocks nobody's device but the one that sets it.
    pub fn set_password(&self, password: &str) -> Result<(), Error> {
        if self.managed {
            return Err(Error::Managed);
        }
        if password.chars().count() < MIN_ACCESS_PASSWORD_LEN {
            return Err(Error::TooShort);
        }
        self.save(Some(hash_of(password)?))
    }

    /// Removes the protection.
    pub fn clear(&self) -> Result<(), Error> {
        if self.managed {
            return Err(Error::Managed);
        }
        self.save(None)
    }

    /// Makes the hash again with today's costs. Only called after a successful check, the one moment
    /// the password is known. The devices that unlocked with the old hash have to unlock once more.
    pub fn upgrade(&self, password: &str) {
        let outdated = self
            .hash()
            .is_some_and(|hash| password::needs_rehash(&hash));
        if !outdated {
            return;
        }
        let result = hash_of(password).and_then(|hash| self.save(Some(hash)));
        if let Err(error) = result {
            tracing::warn!(%error, "the access password could not be hashed again");
        }
    }

    fn save(&self, hash: Option<String>) -> Result<(), Error> {
        let mut current = self.write();
        self.file.save(&Stored {
            password_hash: hash.clone(),
        })?;
        *current = hash;
        Ok(())
    }
}

fn hash_of(password: &str) -> Result<String, Error> {
    password::hash(password).map_err(|_| Error::Hashing)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn open(directory: &TempDir, imposed: Option<&str>) -> AccessStore {
        AccessStore::open(directory.path(), imposed).unwrap()
    }

    #[test]
    fn nothing_is_protected_at_first() {
        let directory = TempDir::new().unwrap();

        let store = open(&directory, None);

        assert!(!store.is_protected());
        assert!(!store.is_managed());
        assert_eq!(store.hash(), None);
    }

    #[test]
    fn a_password_protects_and_survives_a_restart() {
        let directory = TempDir::new().unwrap();
        open(&directory, None).set_password("long enough!").unwrap();

        let reopened = open(&directory, None);

        assert!(reopened.is_protected());
        assert!(password::verify("long enough!", &reopened.hash().unwrap()));
    }

    #[test]
    fn a_short_password_is_refused() {
        let directory = TempDir::new().unwrap();
        let store = open(&directory, None);

        assert!(matches!(store.set_password("short"), Err(Error::TooShort)));
        assert!(!store.is_protected());
    }

    #[test]
    fn the_protection_can_be_removed() {
        let directory = TempDir::new().unwrap();
        let store = open(&directory, None);
        store.set_password("long enough!").unwrap();

        store.clear().unwrap();

        assert!(!store.is_protected());
        assert!(!open(&directory, None).is_protected());
    }

    #[test]
    fn the_password_is_never_in_the_file() {
        let directory = TempDir::new().unwrap();
        open(&directory, None)
            .set_password("a secret phrase")
            .unwrap();

        let text = std::fs::read_to_string(directory.path().join(FILE_NAME)).unwrap();

        assert!(!text.contains("a secret phrase"));
        assert!(text.contains("$argon2id$"));
    }

    #[test]
    fn the_environment_imposes_its_password() {
        let directory = TempDir::new().unwrap();

        let store = open(&directory, Some("from the environment"));

        assert!(store.is_managed());
        assert!(password::verify(
            "from the environment",
            &store.hash().unwrap()
        ));
        assert!(matches!(
            store.set_password("another one!"),
            Err(Error::Managed)
        ));
        assert!(matches!(store.clear(), Err(Error::Managed)));
    }

    #[test]
    fn the_environment_replaces_what_was_set_before() {
        let directory = TempDir::new().unwrap();
        open(&directory, None)
            .set_password("the old password")
            .unwrap();

        let store = open(&directory, Some("the new password"));

        assert!(password::verify("the new password", &store.hash().unwrap()));
        assert!(!password::verify(
            "the old password",
            &store.hash().unwrap()
        ));
    }

    #[test]
    fn the_same_environment_password_keeps_the_hash_so_devices_stay_unlocked() {
        let directory = TempDir::new().unwrap();
        let first = open(&directory, Some("from the environment")).hash();

        let second = open(&directory, Some("from the environment")).hash();

        assert_eq!(first, second, "hashing again would change the salt");
    }

    #[test]
    fn an_old_hash_is_upgraded_after_a_good_password() {
        let directory = TempDir::new().unwrap();
        let store = open(&directory, None);
        let params = argon2::Params::new(4096, 1, 1, None).unwrap();
        let light =
            argon2::Argon2::new(argon2::Algorithm::Argon2id, argon2::Version::V0x13, params);
        let old = argon2::password_hash::PasswordHasher::hash_password(&light, b"old but good")
            .unwrap()
            .to_string();
        store.save(Some(old.clone())).unwrap();

        store.upgrade("old but good");

        let upgraded = store.hash().unwrap();
        assert_ne!(upgraded, old);
        assert!(!password::needs_rehash(&upgraded));
        assert!(password::verify("old but good", &upgraded));
    }

    #[test]
    fn a_current_hash_is_left_alone() {
        let directory = TempDir::new().unwrap();
        let store = open(&directory, None);
        store.set_password("long enough!").unwrap();
        let before = store.hash();

        store.upgrade("long enough!");

        assert_eq!(store.hash(), before);
    }

    #[test]
    fn a_damaged_file_leaves_the_interface_open_rather_than_stopping_the_start() {
        let directory = TempDir::new().unwrap();
        std::fs::write(directory.path().join(FILE_NAME), "{ nonsense").unwrap();

        assert!(!open(&directory, None).is_protected());
    }
}
