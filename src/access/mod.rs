//! Keeping the interface to those who know the password.

mod gate;
mod password;
mod token;

pub use gate::Gate;
pub use password::{hash as hash_password, needs_rehash, verify as verify_password};
pub use token::{LIFETIME_SECONDS, Tokens, Validity, stamp_for};
