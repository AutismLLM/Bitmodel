//! High-level workflows that stitch `core` + `node` + `registry` together:
//! the validator's `validate → sign → publish` and the client's
//! `resolve → verify quorum → fetch → verify bytes → (auto-seed)`.

use std::collections::HashMap;
use std::path::Path;
use std::str::FromStr;

use anyhow::{Context, Result};
use bitmodel_core::{FileEntry, Keyring, Manifest, ValidatorKey};
use iroh::{NodeAddr, NodeId, RelayUrl};

use crate::{format_collection_id, registry::RegistryClient, Node};

/// Render an iroh blob [`Hash`](iroh_blobs::Hash) as `"blake3:<hex>"`.
fn hash_str(h: &iroh_blobs::Hash) -> String {
    format!("blake3:{}", hex::encode(h.as_bytes()))
}

/// Inputs for validating + publishing a model.
pub struct ValidateParams {
    pub model: String,
    pub version: String,
    pub origin: String,
    pub registry: String,
    pub token: Option<String>,
}

/// Import `src`, build + sign the manifest, and publish it to the registry.
/// Returns the signed manifest. The caller's [`Node`] now holds and seeds the
/// content, so a validator can keep serving after publishing.
pub async fn validate_and_publish(
    node: &Node,
    key: &ValidatorKey,
    src: &Path,
    params: ValidateParams,
) -> Result<Manifest> {
    let seeded = node.seed_dir(src).await.context("import/seed source")?;

    let files: Vec<FileEntry> = seeded
        .files
        .iter()
        .map(|f| FileEntry {
            path: f.path.clone(),
            size: f.size,
            blake3: hash_str(&f.hash),
        })
        .collect();

    let mut manifest = Manifest {
        name: params.model.clone(),
        version: params.version,
        origin: params.origin,
        files,
        collection: format_collection_id(&seeded.collection_hash),
        signatures: Vec::new(),
    };
    let sig = key.sign_manifest(&manifest).context("sign manifest")?;
    manifest.signatures.push(sig);

    let client = RegistryClient::new(&params.registry);
    let client = match &params.token {
        Some(t) => client.with_token(t.clone()),
        None => client,
    };
    client
        .put_manifest(&params.model, &manifest)
        .await
        .context("publish manifest")?;

    Ok(manifest)
}

/// Build dialable [`NodeAddr`]s from registry NodeId strings, attaching the
/// shared relay url (if any) so relay fallback works without discovery.
pub fn node_addrs(node_ids: &[String], relay_url: Option<&str>) -> Result<Vec<NodeAddr>> {
    let relay = match relay_url {
        Some(u) => Some(RelayUrl::from_str(u).context("parse relay url")?),
        None => None,
    };
    let mut out = Vec::new();
    for id in node_ids {
        let nid = NodeId::from_str(id).with_context(|| format!("parse node id {id}"))?;
        let addr = match &relay {
            Some(r) => NodeAddr::new(nid).with_relay_url(r.clone()),
            None => NodeAddr::new(nid),
        };
        out.push(addr);
    }
    Ok(out)
}

/// Outcome of a verified download.
pub struct DownloadResult {
    pub manifest: Manifest,
    pub report: bitmodel_core::VerifyReport,
}

/// Resolve a model from the registry, verify the quorum signatures, fetch the
/// collection from live seeders (BLAKE3-verified by iroh), export it to
/// `out_dir`, and cross-check the downloaded files against the manifest.
pub async fn download_verified(
    node: &Node,
    keyring: &Keyring,
    registry: &str,
    model: &str,
    out_dir: &Path,
    relay_url: Option<&str>,
) -> Result<DownloadResult> {
    let client = RegistryClient::new(registry);

    let manifest = client.get_manifest(model).await.context("resolve manifest")?;

    // 1. Authenticity: quorum of trusted signatures.
    let report = keyring.verify(&manifest).context("verify quorum")?;
    anyhow::ensure!(
        report.accepted,
        "manifest REJECTED: {} valid signature(s), quorum is {}",
        report.valid_sigs,
        report.quorum
    );

    // 2. Find live seeders.
    let seeder_ids = client.seeders(model).await.context("query seeders")?;
    anyhow::ensure!(!seeder_ids.is_empty(), "no live seeders for {model}");
    let addrs = node_addrs(&seeder_ids, relay_url)?;

    // 3. Integrity: fetch the signed collection hash (every byte BLAKE3-checked).
    let collection_hash = crate::parse_collection_id(&manifest.collection)?;
    let collection = node
        .get(collection_hash, addrs, out_dir)
        .await
        .context("download collection")?;

    // 4. Cross-check: every manifest file must be present in the collection with
    //    the signed hash. (Bytes are already verified; this binds the manifest's
    //    file list to what we actually got.)
    let got: HashMap<String, String> = collection
        .iter()
        .map(|(name, h)| (name.clone(), hash_str(h)))
        .collect();
    for f in &manifest.files {
        match got.get(&f.path) {
            Some(h) if *h == f.blake3 => {}
            Some(h) => anyhow::bail!("file {} hash mismatch: manifest {} got {}", f.path, f.blake3, h),
            None => anyhow::bail!("file {} missing from downloaded collection", f.path),
        }
    }

    Ok(DownloadResult { manifest, report })
}
