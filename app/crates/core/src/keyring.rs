//! Trust = a list of keys + a threshold. The keyring (`trust.toml`) is what a
//! client pins; quorum verification against it is the one rule that makes a
//! manifest "truth".

use std::collections::HashMap;
use std::path::Path;

use ed25519_dalek::{Signature, VerifyingKey};
use serde::{Deserialize, Serialize};

use crate::{manifest::Manifest, Error};

/// One trusted validator: a key-id and its Ed25519 public key.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidatorEntry {
    pub id: String,
    /// `"ed25519:<hex>"`
    pub key: String,
}

/// The pinned trust configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Keyring {
    pub quorum: usize,
    #[serde(default, rename = "validator")]
    pub validators: Vec<ValidatorEntry>,
}

/// Outcome of verifying a manifest against the keyring.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifyReport {
    pub valid_sigs: usize,
    pub quorum: usize,
    pub accepted: bool,
    /// Key-ids whose signatures verified.
    pub signers: Vec<String>,
}

impl Keyring {
    pub fn from_toml(s: &str) -> Result<Self, Error> {
        let k: Keyring = toml::from_str(s)?;
        if k.quorum == 0 {
            return Err(Error::Trust("quorum must be >= 1".into()));
        }
        Ok(k)
    }

    pub fn to_toml(&self) -> Result<String, Error> {
        Ok(toml::to_string_pretty(self)?)
    }

    pub fn load(path: &Path) -> Result<Self, Error> {
        Self::from_toml(&std::fs::read_to_string(path)?)
    }

    /// Decode each validator's verifying key, keyed by id.
    fn decode_keys(&self) -> Result<HashMap<String, VerifyingKey>, Error> {
        let mut out = HashMap::new();
        for v in &self.validators {
            let hex = v
                .key
                .strip_prefix("ed25519:")
                .ok_or_else(|| Error::Trust(format!("validator {} key missing ed25519: prefix", v.id)))?;
            let bytes: [u8; 32] = hex::decode(hex)
                .map_err(|_| Error::Trust(format!("validator {} key not hex", v.id)))?
                .try_into()
                .map_err(|_| Error::Trust(format!("validator {} key wrong length", v.id)))?;
            let vk = VerifyingKey::from_bytes(&bytes)
                .map_err(|_| Error::Trust(format!("validator {} key invalid", v.id)))?;
            out.insert(v.id.clone(), vk);
        }
        Ok(out)
    }

    /// Count valid signatures from keyring members over the canonical manifest
    /// bytes, and accept iff `count >= quorum`. Duplicate signatures from the
    /// same validator are counted once.
    pub fn verify(&self, manifest: &Manifest) -> Result<VerifyReport, Error> {
        let keys = self.decode_keys()?;
        let msg = manifest.signing_bytes()?;

        let mut signers: Vec<String> = Vec::new();
        for s in &manifest.signatures {
            let Some(vk) = keys.get(&s.validator) else {
                continue; // not a keyring member
            };
            if signers.contains(&s.validator) {
                continue; // already counted this validator
            }
            let Ok(sig_bytes) = hex::decode(&s.sig) else {
                continue;
            };
            let Ok(sig_arr) = <[u8; 64]>::try_from(sig_bytes.as_slice()) else {
                continue;
            };
            let signature = Signature::from_bytes(&sig_arr);
            if vk.verify_strict(&msg, &signature).is_ok() {
                signers.push(s.validator.clone());
            }
        }

        let valid_sigs = signers.len();
        Ok(VerifyReport {
            valid_sigs,
            quorum: self.quorum,
            accepted: valid_sigs >= self.quorum,
            signers,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sign::ValidatorKey;

    fn manifest() -> Manifest {
        Manifest {
            name: "m".into(),
            version: "1".into(),
            origin: "o".into(),
            files: vec![],
            collection: "blake3:00".into(),
            signatures: vec![],
        }
    }

    #[test]
    fn quorum_1_accepts_single_valid_sig() {
        let v = ValidatorKey::generate("mirror-a");
        let mut m = manifest();
        m.signatures.push(v.sign_manifest(&m).unwrap());

        let kr = Keyring {
            quorum: 1,
            validators: vec![v.entry()],
        };
        let r = kr.verify(&m).unwrap();
        assert!(r.accepted);
        assert_eq!(r.valid_sigs, 1);
    }

    #[test]
    fn quorum_2_rejects_single_sig() {
        let a = ValidatorKey::generate("mirror-a");
        let b = ValidatorKey::generate("mirror-b");
        let mut m = manifest();
        m.signatures.push(a.sign_manifest(&m).unwrap());

        let kr = Keyring {
            quorum: 2,
            validators: vec![a.entry(), b.entry()],
        };
        let r = kr.verify(&m).unwrap();
        assert!(!r.accepted);
        assert_eq!(r.valid_sigs, 1);
    }

    #[test]
    fn quorum_2_accepts_two_distinct_sigs() {
        let a = ValidatorKey::generate("mirror-a");
        let b = ValidatorKey::generate("mirror-b");
        let mut m = manifest();
        m.signatures.push(a.sign_manifest(&m).unwrap());
        m.signatures.push(b.sign_manifest(&m).unwrap());

        let kr = Keyring {
            quorum: 2,
            validators: vec![a.entry(), b.entry()],
        };
        assert!(kr.verify(&m).unwrap().accepted);
    }

    #[test]
    fn unknown_validator_is_ignored() {
        let a = ValidatorKey::generate("mirror-a");
        let outsider = ValidatorKey::generate("evil");
        let mut m = manifest();
        m.signatures.push(outsider.sign_manifest(&m).unwrap());

        let kr = Keyring {
            quorum: 1,
            validators: vec![a.entry()],
        };
        assert!(!kr.verify(&m).unwrap().accepted);
    }

    #[test]
    fn tampered_manifest_fails() {
        let a = ValidatorKey::generate("mirror-a");
        let mut m = manifest();
        m.signatures.push(a.sign_manifest(&m).unwrap());
        // tamper after signing
        m.name = "evil".into();

        let kr = Keyring {
            quorum: 1,
            validators: vec![a.entry()],
        };
        assert!(!kr.verify(&m).unwrap().accepted);
    }

    #[test]
    fn duplicate_sig_counts_once() {
        let a = ValidatorKey::generate("mirror-a");
        let b = ValidatorKey::generate("mirror-b");
        let mut m = manifest();
        let s = a.sign_manifest(&m).unwrap();
        m.signatures.push(s.clone());
        m.signatures.push(s); // duplicate

        let kr = Keyring {
            quorum: 2,
            validators: vec![a.entry(), b.entry()],
        };
        let r = kr.verify(&m).unwrap();
        assert_eq!(r.valid_sigs, 1, "duplicate must not satisfy quorum");
        assert!(!r.accepted);
    }

    #[test]
    fn toml_round_trips() {
        let a = ValidatorKey::generate("mirror-a");
        let kr = Keyring {
            quorum: 1,
            validators: vec![a.entry()],
        };
        let t = kr.to_toml().unwrap();
        let back = Keyring::from_toml(&t).unwrap();
        assert_eq!(back.quorum, 1);
        assert_eq!(back.validators.len(), 1);
    }
}
