//! BitModel `core` — pure logic, no network.
//!
//! Manifest types, BLAKE3 hashing, Ed25519 multi-signature, keyring + quorum
//! verification, and the file-extension allowlist. This crate defines the unit
//! of *truth* (the signed manifest) and the one rule that accepts it.

pub mod allowlist;
pub mod hash;
pub mod keyring;
pub mod manifest;
pub mod sign;

pub use keyring::{Keyring, ValidatorEntry, VerifyReport};
pub use manifest::{FileEntry, Manifest, Sig};
pub use sign::ValidatorKey;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("toml parse error: {0}")]
    TomlDe(#[from] toml::de::Error),
    #[error("toml encode error: {0}")]
    TomlSer(#[from] toml::ser::Error),
    #[error("malformed hash string: {0}")]
    BadHash(String),
    #[error("trust/keyring error: {0}")]
    Trust(String),
    #[error("disallowed file extension: {0}")]
    Disallowed(String),
}
