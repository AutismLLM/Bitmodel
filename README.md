<div align="center">

# BitModel

Peer-to-peer mirroring for open-source model weights.
Verified, resumable transfers between seeders and downloaders, across home networks.

</div>

## What this is

BitModel is an open-source research project by [Modelmirror.org](https://modelmirror.org).
We planned it for weeks before writing code. The design, the trust model, and the
research behind it are human work, not generated. The build itself is early and
partly vibe-coded, but the decisions are ours, and we are still trying out
different concepts, different models, and ideas for privacy by design.

The codebase is kept small on purpose, so that a single person, or a small team,
can still read it end to end and understand how it works.

We think the torrent model is the better way to do mainstream peer-to-peer
distribution. But the word "torrent" carries an aftertaste. A lot of people
associate it with illegal movie downloads, and some ISPs actively throttle
torrent traffic. So BitModel is its own protocol with a narrow purpose: by design
it only accepts model weights and a few specific file types. That file-type check
is already implemented, not a promise.

The goal is a small, lightweight, easy-to-use system that is only for open-source
models, lowers the barrier to mirror and share them, and stays focused on that one
job. We are honest about the risk: a peer-to-peer network is nothing without
people on it, and without adoption there is no network effect. We do not know yet
if this works. We keep building.

## How it stays safe

Two ideas, kept separate on purpose:

- **A small trusted group signs the truth.** Validators each fetch a model, hash
  it, and sign a small record of it (name, file list, sizes, hashes). A record
  counts as real only if it carries enough valid signatures from a pinned set of
  keys (a quorum).
- **Anyone can supply capacity.** Seeding is open to everyone and needs no trust,
  because every byte you receive is checked against the signed hash. A bad or
  malicious seeder is rejected, not believed.

So both guarantees hold no matter who you download from: integrity ("these are the
right bytes") and authenticity ("a trusted party vouched for them"). Trust is just
a list of keys plus a threshold. Start at one key (you), grow to a federation
later, no code change.

```
        ORIGIN (a public model host)
           |  validators fetch + hash + sign
           v
   [ VALIDATORS (keyring) ]      Layer 1: TRUTH (small, trusted, signed)
           |                     a record is accepted only with >= quorum signatures
           |  publishes signed record
           v
        REGISTRY  ----------------->  CLIENT (downloader)
        model -> live seeders             |  checks signatures + every byte
           ^                              |  then becomes a seeder
   announce|                              v
   [ SEEDERS: anyone, serving verified bytes ]   Layer 2: CAPACITY (open)
        bytes flow peer-to-peer; a RELAY helps only when a direct link fails
```

## Roadmap

Rough and honest, will change.

- **Now (POC / MVP):** one client binary; sign and verify with a quorum;
  peer-to-peer transfer with resume and auto-seed; self-hosted relay, registry,
  and an always-on anchor seeder; file-type allowlist. Validated end-to-end,
  including a real cross-network download of a model.
- **0.1 beta (with ~10 to 20 testers):** privacy options for seeders (a relay-only
  mode so peers see a key and not your address; the option to run a seeder behind
  a VPN); simpler one-command onboarding; basic health and metrics.
- **Later:** multi-validator federation (quorum greater than one); more models and
  an always-available seed set; stronger seeder privacy via multi-hop relaying;
  rough edges sanded down.

## Repo layout

```
app/          Rust workspace (the whole system)
  crates/
    core/       record types, BLAKE3 hashing, Ed25519 multi-sig + keyring/quorum, allowlist
    node/       iroh endpoint: seed + get (BLAKE3-verified, resumable) + auto-seed
    validator/  auto-script: fetch origin, hash, sign, publish
    cli/        bitmodel seed | get | validate | peers | keygen
    registry/   axum + SQLite: serves records, takes seeder announces
  deploy/     systemd units, env template, one-command deploy.sh
docs/         the mini-wiki (trust model, roles, glossary)
landingpage/  (placeholder) site, built later
```

## Quickstart (a local swarm)

```bash
cd app
cargo build --release
B=target/release

# 1) registry
$B/bitmodel-registry --bind 127.0.0.1:8090 &

# 2) a validator key + trust config (public keys only)
$B/bitmodel keygen --id mirror-a --out mirror-a.key
printf 'quorum = 1\n' > trust.toml
$B/bitmodel keygen --id mirror-a 2>/dev/null | sed -n '/\[\[validator\]\]/,$p' >> trust.toml

# 3) validate + seed a folder of model files (.safetensors / .gguf / ...)
BITMODEL_REGISTRY=http://127.0.0.1:8090 \
  $B/bitmodel --no-relay validate --model demo --src ./my-model \
  --key mirror-a.key --id mirror-a --keep-seeding &

# 4) download it by name (checks the quorum and every byte, then auto-seeds)
BITMODEL_REGISTRY=http://127.0.0.1:8090 \
  $B/bitmodel --no-relay get --model demo --out ./dl --trust trust.toml
```

## Deploy (one server)

One box runs three small always-on services: a relay (helps when a direct link
fails), the registry, and an anchor seeder. They install as systemd units. See
[`app/deploy/README.md`](app/deploy/README.md).

```bash
SSH_HOST=my-vps PUBLIC_IP=YOUR_VPS_IP ./app/deploy/deploy.sh
```

Binaries are built inside a `rust:1.96-bullseye` container so their glibc is old
enough to run on a typical Ubuntu LTS server (no musl needed).

## Status

MVP built and validated: quorum sign and verify, LAN transfer, kill-and-resume,
multi-seeder failover, and a real cross-network download (home to server) of a
~638 MB model. The bytes were BLAKE3-checked and byte-identical, and records with
too few signatures or the wrong key were correctly rejected.

Stack: Rust 1.96, iroh + iroh-blobs 0.35, BLAKE3, Ed25519 (`ed25519-dalek`),
axum + SQLite.

## License

Dual-licensed under MIT or Apache-2.0.
