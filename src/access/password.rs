use argon2::Argon2;
use argon2::password_hash::{PasswordHasher, PasswordVerifier, phc::PasswordHash};

/// The start of every hash made with the current parameters: variant, version, memory (KiB), time
/// and parallelism. A hash that does not start with it was made with other costs.
const CURRENT_PREFIX: &str = "$argon2id$v=19$m=19456,t=2,p=1$";

/// Hashes a password with Argon2id, a random salt and the recommended costs. The result is a PHC
/// string that carries everything needed to check the password later.
pub fn hash(password: &str) -> Result<String, argon2::password_hash::Error> {
    Ok(Argon2::default()
        .hash_password(password.as_bytes())?
        .to_string())
}

/// Whether the password is the one the hash was made from. A hash that cannot be read never matches.
#[must_use]
pub fn verify(password: &str, stored: &str) -> bool {
    PasswordHash::new(stored).is_ok_and(|parsed| {
        Argon2::default()
            .verify_password(password.as_bytes(), &parsed)
            .is_ok()
    })
}

/// Whether the hash was made with other costs than today's. Only worth asking after a successful
/// [`verify`], the one moment the password is known and can be hashed again.
#[must_use]
pub fn needs_rehash(stored: &str) -> bool {
    !stored.starts_with(CURRENT_PREFIX)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_password_checks_against_its_own_hash_only() {
        let stored = hash("correct horse").unwrap();

        assert!(verify("correct horse", &stored));
        assert!(!verify("correct horse ", &stored));
        assert!(!verify("", &stored));
    }

    #[test]
    fn two_hashes_of_one_password_differ_by_their_salt() {
        assert_ne!(hash("same").unwrap(), hash("same").unwrap());
    }

    #[test]
    fn the_hash_is_a_phc_string_with_the_current_costs() {
        let stored = hash("password!").unwrap();

        assert!(stored.starts_with(CURRENT_PREFIX), "{stored}");
        assert!(!needs_rehash(&stored));
    }

    #[test]
    fn something_that_is_not_a_hash_never_matches() {
        for stored in [
            "",
            "plain",
            "$argon2id$",
            "$argon2id$v=19$m=1$$",
            "e3b0c44298fc1c14",
        ] {
            assert!(!verify("", stored), "{stored:?}");
            assert!(!verify("plain", stored), "{stored:?}");
        }
    }

    #[test]
    fn a_hash_made_with_other_costs_is_to_be_made_again() {
        assert!(needs_rehash(
            "$argon2id$v=19$m=4096,t=1,p=1$c29tZXNhbHQ$aGFzaA"
        ));
        assert!(needs_rehash(
            "$argon2i$v=19$m=19456,t=2,p=1$c29tZXNhbHQ$aGFzaA"
        ));
        assert!(needs_rehash("not a hash"));
    }

    #[test]
    fn a_hash_with_other_costs_still_verifies() {
        // 4 MiB, one pass: what a lighter configuration would have stored
        let params = argon2::Params::new(4096, 1, 1, None).unwrap();
        let lighter = Argon2::new(argon2::Algorithm::Argon2id, argon2::Version::V0x13, params);
        let stored = lighter.hash_password(b"old password").unwrap().to_string();

        assert!(verify("old password", &stored));
        assert!(needs_rehash(&stored));
    }
}
