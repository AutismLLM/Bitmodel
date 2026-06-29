# Glossary

| Term | Meaning |
|---|---|
| **Peer** | Any machine running the BitModel binary and participating in the network. A peer can be a seeder, a client, or both at once. |
| **Seeder** | A peer that **has** a model and serves its bytes to others. Open to anyone. Untrusted - its bytes are always verified. |
| **Client / Downloader / Leecher** | A peer **fetching** a model. While downloading it can already serve the pieces it has; when done it can flip to a seeder. |
| **Validator** | A trusted party (human or auto-script) that fetches a model from origin, hashes it, and **signs the record**. Membership = being in the keyring. |
| **Origin** | Where validators originally fetch the model from (e.g. a public model host). Recorded in the record as provenance. |
| **Record** | The signed source of truth for a model: its files, sizes, the **BLAKE3 root**, and a list of validator **signatures**. |
| **BLAKE3 root** | A single hash that commits to all the model's bytes via a Merkle tree. Lets a downloader verify every byte as it arrives. Replaces per-chunk SHA-256. |
| **Signature** | An Ed25519 signature by a validator over the record. Proves a trusted party vouches for the BLAKE3 root. |
| **Keyring** | The list of trusted validator public keys the client pins. Trust = this list. |
| **Quorum / Threshold (N)** | The minimum number of valid keyring signatures a record needs to be accepted as truth. Start N=1, grow later. |
| **NodeId** | A peer's address = an Ed25519 public key. Stable across IP changes. (iroh "dial keys, not IPs".) |
| **Ticket** | A pasteable string encoding a NodeId (+ hints) so a client can reach a peer/content directly, no registry needed. iroh's answer to a magnet link. |
| **Swarm** | All peers sharing one model. More seeders = faster, more resilient downloads. |
| **Registry** | The directory mapping `model → live seeder NodeIds`. A tracker. JSON file now; SQLite + announce/heartbeat when the swarm opens. |
| **Announce / Heartbeat** | A seeder telling the registry "I have model X, NodeId Y, I'm alive" - repeated every few minutes so the list stays fresh. |
| **Relay** | A server (on our VPS) that forwards traffic **only when two peers can't hole-punch a direct connection** (~5-10% of cases). |
| **Hole-punching** | The NAT-traversal trick (via iroh) that lets two home machines connect directly without port-forwarding or config. |
| **Allowlist** | The set of permitted file extensions (`.safetensors`, `.gguf`, …). Anti-abuse/format guard, **not** a license check. |

### Two analogies that map cleanly
- **BitTorrent:** record ≈ `.torrent`, registry ≈ tracker, ticket ≈ magnet link, swarm ≈ swarm,
  BLAKE3 root ≈ piece hashes (but stronger).
- **Linux distros:** keyring + quorum ≈ signed apt/dnf repo keys, seeders ≈ untrusted volunteer
  mirrors.
