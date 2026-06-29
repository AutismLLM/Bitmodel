# Role: Seeder

**Untrusted and open - anyone can be one.** Provides the **capacity**.

## What it does

- Holds a model's files in its local iroh-blobs store.
- Serves verified byte ranges to any client that dials its **NodeId**.
- **Announces** to the registry: "I have model X, NodeId Y, I'm alive" - repeated as a heartbeat
  every few minutes.

## Why it needs no trust

Every byte a seeder serves is checked by the client against the **BLAKE3 root** in the signed
record. A malicious or buggy seeder physically cannot deliver corrupt or fake weights - the
bad bytes fail verification and that peer is dropped. So opening seeding to the public is safe.

## How the swarm grows: auto-seed-on-finish

When a [client](client.md) finishes a download, it **keeps serving by default** → every
downloader becomes a seeder. This is the flywheel: more downloads → more seeders → faster
downloads. (Same as a BitTorrent leecher becoming a seeder.)

## Becoming a seeder

1. Run the binary with the model files (received from a download, or handed over directly).
2. It starts an iroh endpoint (stable NodeId) and announces to the registry.
3. Done - no account, no login, no port-forwarding, no signature.

## The sponsor case

Your first seeder is a "sponsor": you hand them the binary + the model, they run one command.
Nothing special about them technically - they're just the first member of an open swarm.

## Not to be confused with

- **Validator** - signs truth (trusted). A seeder serves bytes (untrusted).

See: [Trust model](../trust-model.md) · [Relay](relay.md) (how a home seeder stays reachable).
