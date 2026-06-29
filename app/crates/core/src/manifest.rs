//! The signed manifest: the unit of *truth* in BitModel.
//!
//! A manifest names a model, lists its files (path + size + BLAKE3), records the
//! iroh collection root, and carries one or more validator signatures. The bytes
//! that get signed are the manifest **minus** its `signatures` field, encoded
//! canonically so every validator and client agrees byte-for-byte.

use serde::{Deserialize, Serialize};

use crate::Error;

/// One file in the model.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileEntry {
    pub path: String,
    pub size: u64,
    /// `"blake3:<hex>"`
    pub blake3: String,
}

/// A validator's signature over the canonical manifest bytes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Sig {
    /// Key-id of the validator (must match a keyring entry).
    pub validator: String,
    /// Ed25519 signature, hex-encoded.
    pub sig: String,
}

/// A signed manifest. `signatures` is excluded from the signed bytes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Manifest {
    pub name: String,
    pub version: String,
    /// Where validators fetched it (provenance, informational).
    pub origin: String,
    pub files: Vec<FileEntry>,
    /// `"blake3:<hex>"` — root over the whole set (the iroh collection hash).
    pub collection: String,
    #[serde(default)]
    pub signatures: Vec<Sig>,
}

impl Manifest {
    /// Total bytes across all files.
    pub fn total_size(&self) -> u64 {
        self.files.iter().map(|f| f.size).sum()
    }

    /// The canonical, deterministic byte encoding that validators sign and
    /// clients verify — the manifest with `signatures` cleared. Field order is
    /// fixed by the struct definition, so this is stable across machines.
    pub fn signing_bytes(&self) -> Result<Vec<u8>, Error> {
        let unsigned = Manifest {
            signatures: Vec::new(),
            ..self.clone()
        };
        Ok(serde_json::to_vec(&unsigned)?)
    }

    /// Pretty JSON for publishing (`manifest.json`).
    pub fn to_json(&self) -> Result<String, Error> {
        Ok(serde_json::to_string_pretty(self)?)
    }

    /// Parse from JSON.
    pub fn from_json(s: &str) -> Result<Self, Error> {
        Ok(serde_json::from_str(s)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Manifest {
        Manifest {
            name: "qwen2.5-1.5b".into(),
            version: "1".into(),
            origin: "https://example.invalid/qwen".into(),
            files: vec![FileEntry {
                path: "model.safetensors".into(),
                size: 3_100_000_000,
                blake3: "blake3:aa".into(),
            }],
            collection: "blake3:bb".into(),
            signatures: vec![Sig {
                validator: "mirror-a".into(),
                sig: "deadbeef".into(),
            }],
        }
    }

    #[test]
    fn signing_bytes_ignore_signatures() {
        let mut a = sample();
        let bytes_a = a.signing_bytes().unwrap();
        a.signatures.clear();
        let bytes_b = a.signing_bytes().unwrap();
        assert_eq!(bytes_a, bytes_b, "signatures must not affect signed bytes");
    }

    #[test]
    fn signing_bytes_are_deterministic() {
        assert_eq!(
            sample().signing_bytes().unwrap(),
            sample().signing_bytes().unwrap()
        );
    }

    #[test]
    fn json_round_trips() {
        let m = sample();
        let j = m.to_json().unwrap();
        assert_eq!(Manifest::from_json(&j).unwrap(), m);
    }
}
