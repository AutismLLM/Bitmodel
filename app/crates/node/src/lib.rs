//! BitModel `node` — the iroh transport layer.
//!
//! One [`Node`] owns an iroh [`Endpoint`], a persistent iroh-blobs store, and a
//! [`Router`] that serves blobs to anyone (open capacity). It can:
//!
//! - **seed** a directory of model files → import, build an iroh *collection*,
//!   return the collection hash + per-file BLAKE3 hashes + a dialable ticket;
//! - **get** a collection by hash from one or more seeders → BLAKE3-verified
//!   streaming fetch (resumable) → export to disk → keep serving (auto-seed).
//!
//! Integrity is free: iroh-blobs verifies every byte against the BLAKE3 root
//! during streaming, so an untrusted seeder cannot feed corrupt bytes.

use std::path::{Path, PathBuf};
use std::str::FromStr;

use anyhow::{Context, Result};
use iroh::{
    protocol::Router, Endpoint, RelayMap, RelayMode, RelayUrl, SecretKey,
};
use iroh_blobs::{
    net_protocol::Blobs,
    rpc::client::blobs::{Client as BlobsClient, DownloadMode, DownloadOptions},
    store::{fs::Store, ExportFormat, ExportMode},
    ticket::BlobTicket,
    util::SetTagOption,
    BlobFormat, Hash,
};

mod collection;
pub mod pipeline;
pub mod registry;
pub use collection::SeededFile;

// Re-exports so binaries can build NodeAddrs etc. without depending on iroh.
pub use iroh::{NodeAddr, NodeId};

/// Configuration for bringing up a [`Node`].
#[derive(Debug, Clone, Default)]
pub struct NodeConfig {
    /// Directory holding the blob store and the persisted node secret key.
    pub data_dir: PathBuf,
    /// Custom relay URL (the VPS relay). `None` → n0 default relays.
    pub relay_url: Option<String>,
    /// Disable relays entirely (pure direct / LAN). Overrides `relay_url`.
    pub no_relay: bool,
    /// Fixed UDP port to bind the iroh endpoint (IPv4). `None` → ephemeral.
    /// Set this on a public seeder so the port can be opened in the firewall.
    pub bind_port: Option<u16>,
}

/// A running BitModel node: endpoint + blob store + router.
pub struct Node {
    router: Router,
    blobs: Blobs<Store>,
    client: BlobsClient,
}

/// Result of seeding a model directory.
#[derive(Debug, Clone)]
pub struct SeedResult {
    /// The iroh collection root hash (`Manifest.collection`, sans `blake3:`).
    pub collection_hash: Hash,
    /// Per-file path/size/hash, sorted by path.
    pub files: Vec<SeededFile>,
    /// A self-contained ticket (node addr + hash + format) to fetch this model.
    pub ticket: BlobTicket,
}

impl Node {
    /// Bring up the node: load/generate a stable secret key, open the persistent
    /// store, spawn the blobs router.
    pub async fn spawn(config: NodeConfig) -> Result<Self> {
        std::fs::create_dir_all(&config.data_dir)
            .with_context(|| format!("create data dir {}", config.data_dir.display()))?;

        let secret_key = load_or_create_secret(&config.data_dir)?;

        let relay_mode = if config.no_relay {
            RelayMode::Disabled
        } else if let Some(url) = &config.relay_url {
            let url = RelayUrl::from_str(url).context("parse relay url")?;
            RelayMode::Custom(RelayMap::from(url))
        } else {
            RelayMode::Default
        };

        let mut builder = Endpoint::builder()
            .secret_key(secret_key)
            .relay_mode(relay_mode)
            .discovery_n0()
            // Also resolve peers by NodeId on the local network (mDNS), so a LAN
            // get-by-id works without internet discovery.
            .discovery_local_network();
        if let Some(port) = config.bind_port {
            builder = builder.bind_addr_v4(std::net::SocketAddrV4::new(
                std::net::Ipv4Addr::UNSPECIFIED,
                port,
            ));
        }
        let endpoint = builder.bind().await.context("bind iroh endpoint")?;

        let store_path = config.data_dir.join("blobs");
        let blobs = Blobs::persistent(&store_path)
            .await
            .context("open persistent blob store")?
            .build(&endpoint);

        let router = Router::builder(endpoint)
            .accept(iroh_blobs::ALPN, blobs.clone())
            .spawn();

        let client = blobs.client().boxed();

        Ok(Self {
            router,
            blobs,
            client,
        })
    }

    /// This node's dialable address (direct addresses + relay url).
    pub async fn node_addr(&self) -> Result<NodeAddr> {
        Ok(self.router.endpoint().node_addr().await?)
    }

    /// This node's id (string form goes in `peers.json` / the registry).
    pub fn node_id(&self) -> String {
        self.router.endpoint().node_id().to_string()
    }

    /// Direct blobs client (for advanced use / tests).
    pub fn client(&self) -> &BlobsClient {
        &self.client
    }

    /// Import every allowlisted file under `dir`, build a deterministic
    /// collection, and return its hash + per-file hashes + a fetch ticket.
    pub async fn seed_dir(&self, dir: &Path) -> Result<SeedResult> {
        let files = collection::import_dir(&self.client, dir).await?;
        anyhow::ensure!(
            !files.is_empty(),
            "no allowlisted model files under {}",
            dir.display()
        );

        let collection: iroh_blobs::format::collection::Collection =
            files.iter().map(|f| (f.path.clone(), f.hash)).collect();
        let (collection_hash, _tag) = self
            .client
            .create_collection(collection, SetTagOption::Auto, Vec::new())
            .await
            .context("create collection")?;

        let addr = self.node_addr().await?;
        let ticket = BlobTicket::new(addr, collection_hash, BlobFormat::HashSeq)?;

        Ok(SeedResult {
            collection_hash,
            files,
            ticket,
        })
    }

    /// Produce a fetch ticket for a collection hash this node holds. Used by the
    /// anchor seeder after a `get` to re-advertise the same content.
    pub async fn ticket_for(&self, collection_hash: Hash) -> Result<BlobTicket> {
        let addr = self.node_addr().await?;
        Ok(BlobTicket::new(addr, collection_hash, BlobFormat::HashSeq)?)
    }

    /// Whether this node already holds the given blob/collection.
    pub async fn has_collection(&self, collection_hash: Hash) -> Result<bool> {
        Ok(self.client.has(collection_hash).await.unwrap_or(false))
    }

    /// Fetch a collection by hash from one or more seeders (BLAKE3-verified,
    /// resumable), then export all files into `out_dir`. Returns the loaded
    /// collection (name → hash) for cross-checking against the manifest.
    pub async fn get(
        &self,
        collection_hash: Hash,
        seeders: Vec<NodeAddr>,
        out_dir: &Path,
    ) -> Result<iroh_blobs::format::collection::Collection> {
        anyhow::ensure!(!seeders.is_empty(), "no seeders provided for download");
        std::fs::create_dir_all(out_dir)
            .with_context(|| format!("create out dir {}", out_dir.display()))?;

        // Download the whole hash-sequence (collection + children) from the
        // given seeders. iroh-blobs verifies every byte against the BLAKE3 root.
        let opts = DownloadOptions {
            format: BlobFormat::HashSeq,
            nodes: seeders,
            tag: SetTagOption::Auto,
            // Queued routes through the downloader, which can pull from several
            // seeders and fail over if one drops mid-transfer.
            mode: DownloadMode::Queued,
        };
        self.client
            .download_with_opts(collection_hash, opts)
            .await
            .context("start download")?
            .finish()
            .await
            .context("download collection")?;

        // Load the collection so we know the child file names.
        let collection = self
            .client
            .get_collection(collection_hash)
            .await
            .context("load downloaded collection")?;

        // Export is not idempotent (it refuses to overwrite), so clear any
        // existing destination files first — we're about to write verified bytes.
        for (name, _) in collection.iter() {
            let dest = out_dir.join(name);
            if dest.exists() {
                let _ = std::fs::remove_file(&dest);
            }
        }

        // Export all children to disk under their collection names.
        self.client
            .export(
                collection_hash,
                out_dir.to_path_buf(),
                ExportFormat::Collection,
                ExportMode::Copy,
            )
            .await
            .context("start export")?
            .finish()
            .await
            .context("export collection")?;

        Ok(collection)
    }

    /// Block until Ctrl-C, then shut down — used by long-running seeders.
    pub async fn serve_forever(self) -> Result<()> {
        tokio::signal::ctrl_c().await?;
        self.shutdown().await
    }

    pub async fn shutdown(self) -> Result<()> {
        self.router.shutdown().await?;
        drop(self.blobs);
        Ok(())
    }
}

/// Parse a `"blake3:<hex>"` or bare-hex collection id into an iroh [`Hash`].
pub fn parse_collection_id(s: &str) -> Result<Hash> {
    let hex = s.strip_prefix("blake3:").unwrap_or(s);
    let bytes: [u8; 32] = hex::decode(hex)
        .context("collection id not hex")?
        .try_into()
        .map_err(|_| anyhow::anyhow!("collection id wrong length"))?;
    Ok(Hash::from_bytes(bytes))
}

/// Render an iroh [`Hash`] as `"blake3:<hex>"` for manifests.
pub fn format_collection_id(hash: &Hash) -> String {
    format!("blake3:{}", hex::encode(hash.as_bytes()))
}

fn load_or_create_secret(data_dir: &Path) -> Result<SecretKey> {
    let key_path = data_dir.join("node.key");
    if key_path.exists() {
        let hex = std::fs::read_to_string(&key_path)?;
        let bytes: [u8; 32] = hex::decode(hex.trim())
            .context("node key not hex")?
            .try_into()
            .map_err(|_| anyhow::anyhow!("node key wrong length"))?;
        Ok(SecretKey::from_bytes(&bytes))
    } else {
        let secret = SecretKey::generate(rand::rngs::OsRng);
        std::fs::write(&key_path, hex::encode(secret.to_bytes()))?;
        Ok(secret)
    }
}
