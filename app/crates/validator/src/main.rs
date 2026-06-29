//! BitModel validator auto-script: `fetch origin → blake3 → sign → publish`.
//!
//! Fetches each model file from an origin (e.g. a public model host) into a
//! working directory, then imports + signs + publishes the manifest via the
//! shared pipeline. Optionally stays up as an anchor seeder.

use std::path::PathBuf;
use std::time::Duration;

use anyhow::{Context, Result};
use bitmodel_core::ValidatorKey;
use bitmodel_node::{
    pipeline::{self, ValidateParams},
    registry::RegistryClient,
    Node, NodeConfig,
};
use clap::Parser;
use futures_lite::StreamExt;
use indicatif::{ProgressBar, ProgressStyle};
use tokio::io::AsyncWriteExt;

const HEARTBEAT: Duration = Duration::from_secs(120);

#[derive(Parser)]
#[command(name = "bitmodel-validator", about = "fetch origin → blake3 → sign → publish")]
struct Args {
    #[arg(long)]
    model: String,
    #[arg(long, default_value = "1")]
    version: String,
    /// Base URL to fetch files from, e.g. https://host/repo/resolve/main
    #[arg(long)]
    origin_base: String,
    /// Comma-separated file names (relative paths) to fetch from origin_base.
    #[arg(long, value_delimiter = ',')]
    files: Vec<String>,
    /// Working directory to download into (and seed from).
    #[arg(long)]
    work: PathBuf,
    /// Validator secret key file.
    #[arg(long)]
    key: PathBuf,
    /// Validator key-id (must match the keyring).
    #[arg(long)]
    id: String,
    #[arg(long, env = "BITMODEL_REGISTRY", default_value = "http://127.0.0.1:8090")]
    registry: String,
    #[arg(long, env = "BITMODEL_TOKEN", default_value = "")]
    token: String,
    #[arg(long, env = "BITMODEL_DATA", default_value = "./validator-data")]
    data_dir: PathBuf,
    #[arg(long, env = "BITMODEL_RELAY", default_value = "")]
    relay: String,
    #[arg(long, action = clap::ArgAction::SetTrue)]
    no_relay: bool,
    /// Fixed UDP port for the iroh endpoint (open in firewall on a public box).
    #[arg(long, env = "BITMODEL_PORT")]
    port: Option<u16>,
    /// Stay up as an anchor seeder after publishing.
    #[arg(long)]
    keep_seeding: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "warn".into()),
        )
        .with_writer(std::io::stderr)
        .init();

    let args = Args::parse();
    anyhow::ensure!(!args.files.is_empty(), "no --files given");
    std::fs::create_dir_all(&args.work)?;

    // 1. Fetch each file from origin (skip if already complete).
    let http = reqwest::Client::new();
    for file in &args.files {
        fetch_file(&http, &args.origin_base, file, &args.work).await?;
    }

    // 2. Bring up a node and validate + publish.
    let relay = if args.relay.is_empty() { None } else { Some(args.relay.clone()) };
    let node = Node::spawn(NodeConfig {
        data_dir: args.data_dir.clone(),
        relay_url: relay,
        no_relay: args.no_relay,
        bind_port: args.port,
    })
    .await
    .context("spawn node")?;

    let secret = std::fs::read_to_string(&args.key)
        .with_context(|| format!("read key {}", args.key.display()))?;
    let vkey = ValidatorKey::from_secret_hex(&args.id, secret.trim()).context("load key")?;

    println!("Validating + signing '{}' …", args.model);
    let manifest = pipeline::validate_and_publish(
        &node,
        &vkey,
        &args.work,
        ValidateParams {
            model: args.model.clone(),
            version: args.version.clone(),
            origin: args.origin_base.clone(),
            registry: args.registry.clone(),
            token: if args.token.is_empty() { None } else { Some(args.token.clone()) },
        },
    )
    .await?;

    println!("Published '{}': {} files, {} bytes, collection {}", args.model,
        manifest.files.len(), manifest.total_size(), manifest.collection);
    println!("Signed by {} (node {})", vkey.id, node.node_id());

    if args.keep_seeding {
        let registry = RegistryClient::new(&args.registry);
        let node_id = node.node_id();
        let _ = registry.announce(&args.model, &node_id).await;
        let reg = registry.clone();
        let model = args.model.clone();
        let nid = node_id.clone();
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(HEARTBEAT).await;
                let _ = reg.announce(&model, &nid).await;
            }
        });
        println!("Anchor-seeding. Ctrl-C to stop.");
        node.serve_forever().await?;
    } else {
        node.shutdown().await?;
    }
    Ok(())
}

/// Download `base/name` → `work/name`, skipping if a same-size file already
/// exists (cheap resume across re-runs). Shows a byte progress bar.
async fn fetch_file(
    http: &reqwest::Client,
    base: &str,
    name: &str,
    work: &std::path::Path,
) -> Result<()> {
    let url = format!("{}/{}", base.trim_end_matches('/'), name);
    let dest = work.join(name);
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let head = http.head(&url).send().await.ok();
    let remote_len = head
        .as_ref()
        .and_then(|r| r.content_length())
        .filter(|_| head.as_ref().map(|r| r.status().is_success()).unwrap_or(false));

    if let (Ok(meta), Some(len)) = (std::fs::metadata(&dest), remote_len) {
        if meta.len() == len {
            println!("✓ {name} already present ({len} bytes), skipping.");
            return Ok(());
        }
    }

    println!("Fetching {url}");
    let resp = http.get(&url).send().await.context("GET origin file")?;
    anyhow::ensure!(resp.status().is_success(), "origin returned {} for {url}", resp.status());
    let total = resp.content_length().or(remote_len);

    let pb = match total {
        Some(t) => {
            let pb = ProgressBar::new(t);
            pb.set_style(
                ProgressStyle::with_template(
                    "{msg} [{bar:30}] {bytes}/{total_bytes} ({eta})",
                )
                .unwrap()
                .progress_chars("=> "),
            );
            pb
        }
        None => ProgressBar::new_spinner(),
    };
    pb.set_message(name.to_string());

    let mut file = tokio::fs::File::create(&dest)
        .await
        .with_context(|| format!("create {}", dest.display()))?;
    let mut stream = resp.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.context("read chunk")?;
        file.write_all(&chunk).await?;
        pb.inc(chunk.len() as u64);
    }
    file.flush().await?;
    pb.finish_with_message(format!("{name} done"));
    Ok(())
}
