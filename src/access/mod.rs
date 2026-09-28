//! Keeping the interface to those who know the password.

mod gate;
mod password;

pub use gate::Gate;
pub use password::{hash as hash_password, needs_rehash, verify as verify_password};
