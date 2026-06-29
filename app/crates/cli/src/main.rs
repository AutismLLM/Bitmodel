//! `bitmodel` — one binary, many modes: `keygen | validate | seed | get | peers`.

use std::path::PathBuf;
use std::time::Duration;

use anyhow::{Context, Result};
use bitmodel_core::{Keyring, ValidatorKey};
use bitmodel_node::{
    pipeline::{self, ValidateParams},
    registry::RegistryClient,
    Node, NodeConfig,
};
use clap::{Parser, Subcommand};
use indicatif::{ProgressBar, ProgressStyle};

/// Re-announce to the registry this often (heartbeat).
const HEARTBEAT: Duration = Duration::from_secs(120);

#[derive(Parser)]
#[command(name = "bitmodel", version, about = "BitTorrent for model weights")]
struct Cli {
    /// Node data directory (blob store + node key).
    #[arg(long, global = true, env = "BITMODEL_DATA", default_value = "~/.bitmodel")]
    data_dir: String,
    /// Registry base URL.
    #[arg(long, global = true, env = "BITMODEL_REGISTRY", default_value = "http://127.0.0.1:8090")]
    registry: String,
    /// Custom relay URL (the VPS relay). Empty → n0 default relays.
    #[arg(long, global = true, env = "BITMODEL_RELAY", default_value = "")]
    relay: String,
    /// Disable relays entirely (pure direct / LAN testing).
    #[arg(long, global = true, action = clap::ArgAction::SetTrue)]
    no_relay: bool,
    /// Fixed UDP port for the iroh endpoint (open this in the firewall on a
    /// public seeder). Default: ephemeral.
    #[arg(long, global = true, env = "BITMODEL_PORT")]
    port: Option<u16>,

    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Generate a validator Ed25519 keypair; print the keyring entry.
    Keygen {
        /// Validator key-id (e.g. "mirror-a").
        #[arg(long)]
        id: String,
        /// Write the secret key to this file (default: print to stdout).
        #[arg(long)]
        out: Option<PathBuf>,
    },
    /// Validate a model: import → sign manifest → publish to the registry.
    Validate {
        #[arg(long)]
        model: String,
        #[arg(long, default_value = "1")]
        version: String,
        #[arg(long, default_value = "")]
        origin: String,
        /// Directory of model files to validate + seed.
        #[arg(long)]
        src: PathBuf,
        /// Validator secret key file (from `keygen --out`).
        #[arg(long)]
        key: PathBuf,
        /// Validator key-id (must match the keyring).
        #[arg(long)]
        id: String,
        /// Publish token (Bearer) for the registry.
        #[arg(long, env = "BITMODEL_TOKEN", default_value = "")]
        token: String,
        /// Keep seeding (announce + serve) after publishing.
        #[arg(long)]
        keep_seeding: bool,
    },
    /// Seed a model from a local copy: import → announce → serve forever.
    Seed {
        #[arg(long)]
        model: String,
        #[arg(long)]
        src: PathBuf,
    },
    /// Download a model by name: verify quorum → fetch → verify → auto-seed.
    Get {
        #[arg(long)]
        model: String,
        /// Output directory for the model files.
        #[arg(long)]
        out: PathBuf,
        /// Pinned trust config (keyring + quorum).
        #[arg(long)]
        trust: PathBuf,
        /// Don't keep seeding after the download completes.
        #[arg(long)]
        no_seed: bool,
    },
    /// List live seeders for a model.
    Peers {
        #[arg(long)]
        model: String,
    },
}

fn expand_path(s: &str) -> PathBuf {
    if let Some(rest) = s.strip_prefix("~/") {
        if let Some(home) = std::env::var_os("HOME") {
            return PathBuf::from(home).join(rest);
        }
    }
    PathBuf::from(s)
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "warn".into()),
        )
        .with_writer(std::io::stderr)
        .init();

    let cli = Cli::parse();
    let relay = if cli.relay.is_empty() { None } else { Some(cli.relay.clone()) };

    match &cli.cmd {
        Cmd::Keygen { id, out } => keygen(id, out.as_ref()),
        Cmd::Validate { .. } | Cmd::Seed { .. } | Cmd::Get { .. } | Cmd::Peers { .. } => {
            run_networked(&cli, relay).await
        }
    }
}

fn keygen(id: &str, out: Option<&PathBuf>) -> Result<()> {
    let key = ValidatorKey::generate(id);
    let secret = key.secret_hex();
    match out {
        Some(path) => {
            std::fs::write(path, &secret).with_context(|| format!("write {}", path.display()))?;
            // tighten perms (best-effort, unix)
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600));
            }
            println!("Secret key written to {} (keep it safe).", path.display());
        }
        None => {
            println!("# secret key (KEEP SAFE — never put on a seeder):");
            println!("{secret}");
        }
    }
    println!();
    println!("# add this to your trust.toml keyring:");
    println!("[[validator]]");
    println!("id  = \"{id}\"");
    println!("key = \"{}\"", key.public_hex());
    Ok(())
}

async fn run_networked(cli: &Cli, relay: Option<String>) -> Result<()> {
    let data_dir = expand_path(&cli.data_dir);
    let node = Node::spawn(NodeConfig {
        data_dir,
        relay_url: relay.clone(),
        no_relay: cli.no_relay,
        bind_port: cli.port,
    })
    .await
    .context("spawn node")?;

    let registry = RegistryClient::new(&cli.registry);

    match &cli.cmd {
        Cmd::Validate {
            model,
            version,
            origin,
            src,
            key,
            id,
            token,
            keep_seeding,
        } => {
            let secret = std::fs::read_to_string(key)
                .with_context(|| format!("read key file {}", key.display()))?;
            let vkey = ValidatorKey::from_secret_hex(id, secret.trim())
                .context("load validator key")?;

            println!("Importing + hashing {} …", src.display());
            let manifest = pipeline::validate_and_publish(
                &node,
                &vkey,
                src,
                ValidateParams {
                    model: model.clone(),
                    version: version.clone(),
                    origin: origin.clone(),
                    registry: cli.registry.clone(),
                    token: opt(token),
                },
            )
            .await?;

            println!("Published manifest for '{model}':");
            println!("  files:      {}", manifest.files.len());
            println!("  total size: {} bytes", manifest.total_size());
            println!("  collection: {}", manifest.collection);
            println!("  signed by:  {}", vkey.id);

            if *keep_seeding {
                println!("Now seeding (node {}). Ctrl-C to stop.", node.node_id());
                announce_and_serve(node, &registry, model).await?;
            }
        }

        Cmd::Seed { model, src } => {
            println!("Importing {} …", src.display());
            let seeded = node.seed_dir(src).await.context("seed")?;
            println!(
                "Seeding '{model}' ({} files, collection {}).",
                seeded.files.len(),
                bitmodel_node::format_collection_id(&seeded.collection_hash)
            );
            println!("Node {}. Ctrl-C to stop.", node.node_id());
            announce_and_serve(node, &registry, model).await?;
        }

        Cmd::Get {
            model,
            out,
            trust,
            no_seed,
        } => {
            let keyring = Keyring::load(trust)
                .with_context(|| format!("load trust config {}", trust.display()))?;

            let spinner = ProgressBar::new_spinner();
            spinner.set_style(
                ProgressStyle::with_template("{spinner} {msg}").unwrap(),
            );
            spinner.enable_steady_tick(Duration::from_millis(120));
            spinner.set_message(format!("Resolving + downloading '{model}' …"));

            let result = pipeline::download_verified(
                &node,
                &keyring,
                &cli.registry,
                model,
                out,
                relay.as_deref(),
            )
            .await;
            spinner.finish_and_clear();
            let result = result?;

            println!("✓ Verified manifest for '{model}':");
            println!(
                "  quorum: {}/{} valid signature(s) from {:?}",
                result.report.valid_sigs, result.report.quorum, result.report.signers
            );
            println!("  files:  {} ({} bytes)", result.manifest.files.len(), result.manifest.total_size());
            println!("  saved to: {}", out.display());

            if *no_seed {
                node.shutdown().await?;
            } else {
                println!("Auto-seeding (node {}). Ctrl-C to stop.", node.node_id());
                announce_and_serve(node, &registry, model).await?;
            }
        }

        Cmd::Peers { model } => {
            let seeders = registry.seeders(model).await.context("query seeders")?;
            if seeders.is_empty() {
                println!("No live seeders for '{model}'.");
            } else {
                println!("Live seeders for '{model}':");
                for s in seeders {
                    println!("  {s}");
                }
            }
            node.shutdown().await?;
        }

        Cmd::Keygen { .. } => unreachable!(),
    }

    Ok(())
}

fn opt(s: &str) -> Option<String> {
    if s.is_empty() {
        None
    } else {
        Some(s.to_string())
    }
}

/// Announce to the registry immediately, then heartbeat on an interval while
/// serving blobs, until Ctrl-C.
async fn announce_and_serve(node: Node, registry: &RegistryClient, model: &str) -> Result<()> {
    let node_id = node.node_id();
    if let Err(e) = registry.announce(model, &node_id).await {
        eprintln!("warning: initial announce failed: {e}");
    }

    let reg = registry.clone();
    let model_owned = model.to_string();
    let nid = node_id.clone();
    let hb = tokio::spawn(async move {
        loop {
            tokio::time::sleep(HEARTBEAT).await;
            if let Err(e) = reg.announce(&model_owned, &nid).await {
                eprintln!("warning: heartbeat announce failed: {e}");
            }
        }
    });

    let res = node.serve_forever().await;
    hb.abort();
    res
}
