//! BLAKE3 hashing for model files. Uses memory-mapped, multi-threaded hashing
//! (the `mmap` + `rayon` features) so tens of GB hash in seconds.

use std::path::Path;

use crate::Error;

/// A BLAKE3 hash rendered as the manifest's `"blake3:<hex>"` string.
pub fn format_hash(hash: &blake3::Hash) -> String {
    format!("blake3:{}", hash.to_hex())
}

/// Parse a `"blake3:<hex>"` string back into a [`blake3::Hash`].
pub fn parse_hash(s: &str) -> Result<blake3::Hash, Error> {
    let hex = s
        .strip_prefix("blake3:")
        .ok_or_else(|| Error::BadHash(s.to_string()))?;
    let bytes: [u8; 32] = hex::decode(hex)
        .map_err(|_| Error::BadHash(s.to_string()))?
        .try_into()
        .map_err(|_| Error::BadHash(s.to_string()))?;
    Ok(blake3::Hash::from_bytes(bytes))
}

/// Hash a file on disk, returning `(blake3_hash, size_in_bytes)`.
/// Memory-maps and hashes in parallel where the platform allows.
pub fn hash_file(path: &Path) -> Result<(blake3::Hash, u64), Error> {
    let size = std::fs::metadata(path)?.len();
    let mut hasher = blake3::Hasher::new();
    hasher.update_mmap_rayon(path)?;
    Ok((hasher.finalize(), size))
}

/// Hash an in-memory byte slice (used in tests / small inputs).
pub fn hash_bytes(bytes: &[u8]) -> blake3::Hash {
    blake3::hash(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_hash_string() {
        let h = hash_bytes(b"hello bitmodel");
        let s = format_hash(&h);
        assert!(s.starts_with("blake3:"));
        assert_eq!(parse_hash(&s).unwrap(), h);
    }

    #[test]
    fn rejects_malformed_hash() {
        assert!(parse_hash("sha256:abcd").is_err());
        assert!(parse_hash("blake3:zzzz").is_err());
        assert!(parse_hash("blake3:00").is_err());
    }

    #[test]
    fn hashes_a_file() {
        let dir = std::env::temp_dir().join("bitmodel-hash-test");
        std::fs::create_dir_all(&dir).unwrap();
        let f = dir.join("blob.bin");
        std::fs::write(&f, b"the quick brown fox").unwrap();
        let (h, size) = hash_file(&f).unwrap();
        assert_eq!(size, 19);
        assert_eq!(h, hash_bytes(b"the quick brown fox"));
    }
}
