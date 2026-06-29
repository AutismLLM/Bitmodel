# BitModel - Mini Wiki

BitModel is **BitTorrent for AI model weights**: a content-agnostic mirror network that moves
large model files from seeders to downloaders - verified, resumable, across home NATs - with
**federated truth and open capacity**.

> A small **trusted group** mirrors, signs, and validates the truth.
> **Anyone** can join the network and seed.

## The picture

```
        ORIGIN (e.g. a public model host)
           │  validators fetch + hash + sign
           ▼
   ┌─────────────────────┐        Layer 1: TRUTH  (small, trusted, signed)
   │  VALIDATORS (keyring)│        record accepted iff ≥ quorum signatures
   └─────────┬───────────┘
             │ publishes signed record
             ▼
        REGISTRY (VPS)  ──────────►  CLIENT (downloader)
        model → live seeders            │  verifies quorum sigs + BLAKE3 bytes
             ▲                          │  then becomes a seeder
   announce  │                          ▼
   ┌─────────┴──────────────────────────────────┐  Layer 2: CAPACITY (huge, open, untrusted)
   │  SEEDERS - anyone, serving verified bytes   │  bytes flow peer-to-peer (iroh hole-punch)
   └─────────────────────────────────────────────┘  RELAY (VPS) only when hole-punch fails
```

## Pages

- **[Glossary](glossary.md)** - every term in one place.
- **[Trust model](trust-model.md)** - federated truth + open capacity, the quorum rule.
- **Roles** (one binary, many modes):
  - [Validator](roles/validator.md) - signs truth
  - [Seeder](roles/seeder.md) - serves bytes (open to anyone)
  - [Client](roles/client.md) - downloads + verifies
  - [Relay](roles/relay.md) - NAT hole-punch fallback
  - [Registry](roles/registry.md) - finds live seeders

## Two facts that make the whole thing safe

1. **Integrity is independent of who serves.** Every byte is checked against a BLAKE3 root, so
   an untrusted seeder *cannot* feed corrupt or fake weights - bad bytes are rejected.
2. **Authenticity is independent of who seeds.** Only validators' keys can sign a record, so
   nobody can publish a fake model under the network's name - even while seeding is wide open.

See the top-level [`README`](../README.md) for build and usage.
