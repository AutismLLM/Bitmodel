//! Validator signing keys — the crown jewels. A `ValidatorKey` holds the Ed25519
//! signing key and produces signatures over the canonical manifest bytes.

use ed25519_dalek::{Signer, SigningKey, VerifyingKey};
use rand::rngs::OsRng;

use crate::{
    keyring::ValidatorEntry,
    manifest::{Manifest, Sig},
    Error,
};

/// A validator's key material plus its key-id.
pub struct ValidatorKey {
    pub id: String,
    signing: SigningKey,
}

impl ValidatorKey {
    /// Generate a fresh keypair for `id`.
    pub fn generate(id: impl Into<String>) -> Self {
        let signing = SigningKey::generate(&mut OsRng);
        Self {
            id: id.into(),
            signing,
        }
    }

    /// Load from a hex-encoded 32-byte secret key.
    pub fn from_secret_hex(id: impl Into<String>, hex_str: &str) -> Result<Self, Error> {
        let hex_str = hex_str.strip_prefix("ed25519:").unwrap_or(hex_str);
        let bytes: [u8; 32] = hex::decode(hex_str)
            .map_err(|_| Error::Trust("secret key not hex".into()))?
            .try_into()
            .map_err(|_| Error::Trust("secret key wrong length".into()))?;
        Ok(Self {
            id: id.into(),
            signing: SigningKey::from_bytes(&bytes),
        })
    }

    /// Hex of the 32-byte secret (store this safely, never on seeders).
    pub fn secret_hex(&self) -> String {
        format!("ed25519:{}", hex::encode(self.signing.to_bytes()))
    }

    pub fn verifying_key(&self) -> VerifyingKey {
        self.signing.verifying_key()
    }

    /// `"ed25519:<hex>"` public key.
    pub fn public_hex(&self) -> String {
        format!("ed25519:{}", hex::encode(self.verifying_key().to_bytes()))
    }

    /// The keyring entry that clients would pin for this validator.
    pub fn entry(&self) -> ValidatorEntry {
        ValidatorEntry {
            id: self.id.clone(),
            key: self.public_hex(),
        }
    }

    /// Sign a manifest's canonical bytes, returning a [`Sig`].
    pub fn sign_manifest(&self, manifest: &Manifest) -> Result<Sig, Error> {
        let msg = manifest.signing_bytes()?;
        let signature = self.signing.sign(&msg);
        Ok(Sig {
            validator: self.id.clone(),
            sig: hex::encode(signature.to_bytes()),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn secret_round_trips() {
        let v = ValidatorKey::generate("a");
        let s = v.secret_hex();
        let v2 = ValidatorKey::from_secret_hex("a", &s).unwrap();
        assert_eq!(v.public_hex(), v2.public_hex());
    }
}
