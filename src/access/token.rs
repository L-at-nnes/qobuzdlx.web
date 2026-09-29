use std::path::Path;

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use hmac::{Hmac, KeyInit, Mac};
use rand::RngExt;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::storage::{Access, JsonFile};

type HmacSha256 = Hmac<Sha256>;

const KEY_FILE: &str = "cookie-key.json";
const KEY_LEN: usize = 32;

/// How long an unlocked device stays unlocked without coming back.
pub const LIFETIME_SECONDS: u64 = 30 * 24 * 60 * 60;

/// A token older than this is replaced by a fresh one when it is presented, which makes the
/// lifetime slide: a device in use never has to unlock again.
const REFRESH_AFTER_SECONDS: u64 = 24 * 60 * 60;

/// What a token claims: the password it was made for, and when.
#[derive(Debug, Serialize, Deserialize)]
struct Claims {
    /// See [`stamp_for`].
    stamp: String,
    /// Seconds since the Unix epoch.
    issued: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Validity {
    /// The token is good as it is.
    Valid,
    /// The token is good, and a newer one should replace it.
    Refresh,
    Invalid,
}

#[derive(Serialize, Deserialize)]
struct StoredKey {
    key: String,
}

/// Makes and checks the tokens that tell the server a browser has unlocked the interface.
///
/// A token is signed with a key that only the server holds and is kept across restarts: a key drawn
/// at each start would quietly lock every device out each time the container is recreated.
#[derive(Clone)]
pub struct Tokens {
    key: [u8; KEY_LEN],
}

impl std::fmt::Debug for Tokens {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_struct("Tokens").finish_non_exhaustive()
    }
}

impl Tokens {
    /// Reads the signing key from the configuration folder, drawing and saving one the first time.
    #[must_use]
    pub fn open(config_dir: &Path) -> Self {
        let file = JsonFile::new(config_dir.join(KEY_FILE), Access::OwnerOnly);

        if let Some(stored) = file.load::<StoredKey>()
            && let Ok(bytes) = URL_SAFE_NO_PAD.decode(&stored.key)
            && let Ok(key) = <[u8; KEY_LEN]>::try_from(bytes)
        {
            return Self { key };
        }

        let key: [u8; KEY_LEN] = rand::rng().random();
        let stored = StoredKey {
            key: URL_SAFE_NO_PAD.encode(key),
        };
        if let Err(error) = file.save(&stored) {
            // Tokens still work until the next start, after which devices unlock again
            tracing::warn!(%error, "the cookie key could not be saved");
        }
        Self { key }
    }

    #[cfg(test)]
    fn with_key(key: [u8; KEY_LEN]) -> Self {
        Self { key }
    }

    fn sign(&self, payload: &str) -> HmacSha256 {
        let mut mac = HmacSha256::new_from_slice(&self.key).expect("any key length is accepted");
        mac.update(payload.as_bytes());
        mac
    }

    /// A token for a device that just unlocked, made for the password with this stamp.
    #[must_use]
    pub fn issue(&self, stamp: &str, now: u64) -> String {
        let claims = Claims {
            stamp: stamp.to_owned(),
            issued: now,
        };
        let payload =
            URL_SAFE_NO_PAD.encode(serde_json::to_vec(&claims).expect("claims always serialise"));
        let signature = URL_SAFE_NO_PAD.encode(self.sign(&payload).finalize().into_bytes());
        format!("{payload}.{signature}")
    }

    /// Whether a token was issued by this server, for the password that is set now, and is not too
    /// old.
    #[must_use]
    pub fn check(&self, token: &str, stamp: &str, now: u64) -> Validity {
        let Some((payload, signature)) = token.split_once('.') else {
            return Validity::Invalid;
        };
        let Ok(signature) = URL_SAFE_NO_PAD.decode(signature) else {
            return Validity::Invalid;
        };
        // Compared in constant time by the MAC itself
        if self.sign(payload).verify_slice(&signature).is_err() {
            return Validity::Invalid;
        }
        let Some(claims) = URL_SAFE_NO_PAD
            .decode(payload)
            .ok()
            .and_then(|bytes| serde_json::from_slice::<Claims>(&bytes).ok())
        else {
            return Validity::Invalid;
        };

        if claims.stamp != stamp || claims.issued > now {
            return Validity::Invalid;
        }
        let age = now - claims.issued;
        if age > LIFETIME_SECONDS {
            Validity::Invalid
        } else if age > REFRESH_AFTER_SECONDS {
            Validity::Refresh
        } else {
            Validity::Valid
        }
    }
}

/// The fingerprint of a password hash, put in the tokens made for it.
///
/// Changing or removing the password changes the stamp, which invalidates at once every token made
/// for the old one: without it a device that unlocked would stay unlocked after the password changed.
#[must_use]
pub fn stamp_for(password_hash: Option<&str>) -> String {
    match password_hash {
        None | Some("") => "none".to_owned(),
        Some(hash) => {
            let digest = Sha256::digest(hash.as_bytes());
            crate::hex::encode(&digest[..8])
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    const NOW: u64 = 1_700_000_000;

    fn tokens() -> Tokens {
        Tokens::with_key([7; KEY_LEN])
    }

    #[test]
    fn a_fresh_token_is_valid_for_the_password_it_was_made_for() {
        let token = tokens().issue("abc", NOW);

        assert_eq!(tokens().check(&token, "abc", NOW + 60), Validity::Valid);
    }

    #[test]
    fn another_password_makes_it_invalid() {
        let token = tokens().issue("abc", NOW);

        assert_eq!(tokens().check(&token, "other", NOW), Validity::Invalid);
    }

    #[test]
    fn a_token_signed_with_another_key_is_invalid() {
        let token = Tokens::with_key([8; KEY_LEN]).issue("abc", NOW);

        assert_eq!(tokens().check(&token, "abc", NOW), Validity::Invalid);
    }

    #[test]
    fn a_token_that_was_edited_is_invalid() {
        let token = tokens().issue("abc", NOW);
        let (payload, signature) = token.split_once('.').unwrap();
        let forged_claims = URL_SAFE_NO_PAD.encode(br#"{"stamp":"abc","issued":4000000000}"#);

        assert_eq!(
            tokens().check(&format!("{forged_claims}.{signature}"), "abc", NOW),
            Validity::Invalid
        );
        assert_eq!(
            tokens().check(&format!("{payload}.AAAA"), "abc", NOW),
            Validity::Invalid
        );
    }

    #[test]
    fn rubbish_is_invalid() {
        for token in ["", ".", "a.b", "no dot at all", "....", "é.é"] {
            assert_eq!(
                tokens().check(token, "abc", NOW),
                Validity::Invalid,
                "{token:?}"
            );
        }
    }

    #[test]
    fn a_token_from_the_future_is_invalid() {
        let token = tokens().issue("abc", NOW + 1000);

        assert_eq!(tokens().check(&token, "abc", NOW), Validity::Invalid);
    }

    #[test]
    fn a_token_past_its_lifetime_is_invalid() {
        let token = tokens().issue("abc", NOW);

        assert_eq!(
            tokens().check(&token, "abc", NOW + LIFETIME_SECONDS),
            Validity::Refresh
        );
        assert_eq!(
            tokens().check(&token, "abc", NOW + LIFETIME_SECONDS + 1),
            Validity::Invalid
        );
    }

    #[test]
    fn a_token_used_after_a_day_asks_to_be_replaced() {
        let token = tokens().issue("abc", NOW);

        assert_eq!(
            tokens().check(&token, "abc", NOW + REFRESH_AFTER_SECONDS),
            Validity::Valid
        );
        assert_eq!(
            tokens().check(&token, "abc", NOW + REFRESH_AFTER_SECONDS + 1),
            Validity::Refresh
        );
    }

    #[test]
    fn the_key_is_kept_across_starts() {
        let directory = TempDir::new().unwrap();
        let token = Tokens::open(directory.path()).issue("abc", NOW);

        assert_eq!(
            Tokens::open(directory.path()).check(&token, "abc", NOW),
            Validity::Valid
        );
    }

    #[test]
    fn a_damaged_key_file_is_replaced_instead_of_stopping_the_start() {
        let directory = TempDir::new().unwrap();
        std::fs::write(directory.path().join(KEY_FILE), "{ nonsense").unwrap();

        let tokens = Tokens::open(directory.path());

        assert_eq!(
            tokens.check(&tokens.issue("abc", NOW), "abc", NOW),
            Validity::Valid
        );
    }

    #[test]
    fn the_stamp_changes_with_the_password_hash() {
        assert_eq!(stamp_for(None), "none");
        assert_eq!(stamp_for(Some("")), "none");
        assert_ne!(stamp_for(Some("hash one")), stamp_for(Some("hash two")));
        assert_eq!(stamp_for(Some("hash one")), stamp_for(Some("hash one")));
        assert_eq!(stamp_for(Some("hash one")).len(), 16);
    }
}
