//! Importing a directory of model files into the blob store as a deterministic
//! collection. "Deterministic" = files sorted by their relative path, so any
//! validator/seeder importing the same files produces the same collection hash.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use bitmodel_core::allowlist;
use iroh_blobs::{
    rpc::client::blobs::{Client as BlobsClient, WrapOption},
    util::SetTagOption,
    Hash,
};

/// One imported file: its collection name (relative, `/`-separated), size, and
/// BLAKE3 hash (which is exactly the iroh blob hash).
#[derive(Debug, Clone)]
pub struct SeededFile {
    pub path: String,
    pub size: u64,
    pub hash: Hash,
}

/// Import every allowlisted file under `dir` into the store (linked in place,
/// not copied), sorted by relative path for a stable collection hash.
pub async fn import_dir(client: &BlobsClient, dir: &Path) -> Result<Vec<SeededFile>> {
    let mut found: Vec<(String, PathBuf)> = Vec::new();
    walk(dir, dir, &mut found)?;
    found.sort_by(|a, b| a.0.cmp(&b.0));

    let mut out = Vec::new();
    for (rel, abs) in found {
        if !allowlist::is_allowed(&rel) {
            tracing::debug!("skip non-allowlisted file: {rel}");
            continue;
        }
        let abs = std::path::absolute(&abs)
            .with_context(|| format!("absolutize {}", abs.display()))?;
        let size = std::fs::metadata(&abs)?.len();
        let outcome = client
            .add_from_path(abs.clone(), true, SetTagOption::Auto, WrapOption::NoWrap)
            .await
            .with_context(|| format!("import {}", abs.display()))?
            .finish()
            .await
            .with_context(|| format!("finish import {}", abs.display()))?;
        out.push(SeededFile {
            path: rel,
            size,
            hash: outcome.hash,
        });
    }
    Ok(out)
}

/// Recursively collect `(relative_path, absolute_path)` for every file.
fn walk(root: &Path, dir: &Path, out: &mut Vec<(String, PathBuf)>) -> Result<()> {
    for entry in std::fs::read_dir(dir).with_context(|| format!("read dir {}", dir.display()))? {
        let entry = entry?;
        let path = entry.path();
        let ft = entry.file_type()?;
        if ft.is_dir() {
            walk(root, &path, out)?;
        } else if ft.is_file() {
            let rel = path
                .strip_prefix(root)
                .unwrap_or(&path)
                .to_string_lossy()
                .replace('\\', "/");
            out.push((rel, path));
        }
    }
    Ok(())
}
