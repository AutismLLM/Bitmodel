# Trust Model - Federated Truth, Open Capacity

The core design idea. Two layers that are **deliberately different sets of people**.

## Layer 1 - Truth (small, trusted, automatable)

A set of **validators**, each with an Ed25519 key in the **keyring**. For a given model, each
validator independently:

1. **Fetches** the model from origin.
2. Computes the **BLAKE3 root** over the files.
3. **Signs** the record with its own key.

Because multiple independent parties sign the *same* root hash, no single one can forge or
corrupt truth - they cross-confirm each other. A validator can be a person or just an
auto-script: `fetch → hash → sign → publish`.

## Layer 2 - Capacity (huge, open, untrusted)

**Anyone** runs the binary and seeds. They contribute bandwidth and availability and need
**zero trust** - every byte they serve is checked against the signed BLAKE3 root, so a malicious
or buggy seeder is rejected and dropped. Open swarm, no gatekeeping.

## The one rule

> A record is accepted as **truth** only if it carries **≥ N valid signatures from the trusted
> keyring** (the quorum). **Seeding requires no signature at all.**

```
trust.toml
  quorum = 2
  validator "mirror-a"  key ed25519:....
  validator "mirror-b"  key ed25519:....
  validator "mirror-c"  key ed25519:....

client verifies a record:
  valid_sigs = count signatures that
      (a) come from a keyring member AND
      (b) verify against the record bytes
  accept  ⇔  valid_sigs ≥ quorum
```

## Why this works

| Property | Because |
|---|---|
| **Forge-proof** | Faking a model means stealing **N** validator keys, not just running a seeder. |
| **Tamper-proof** | BLAKE3 root verifies every byte; a bad seeder can't sneak in altered weights. |
| **Scalable** | Truth is a few-KB signed record (cheap to replicate). Capacity is unlimited and open. |
| **Censorship-resistant** | No single seeder or server is load-bearing; the swarm routes around losses. |
| **Evolvable** | Trust is just *a list of keys + a threshold*. Start N=1 (you alone), grow to a federation, move to threshold/multisig - no code change. |

## Two independent guarantees (don't conflate them)

- **Integrity** = "these are the right bytes." Provided by the **BLAKE3 root**. Holds no matter
  who seeds.
- **Authenticity** = "a trusted party vouched for these bytes." Provided by **≥ quorum
  signatures**. Holds no matter who seeds.

Open seeding touches neither guarantee - that's *why* capacity can be fully open.

## Precedent

This is the **Linux-distro model** for model weights: a signed keyring (apt/dnf repo keys)
mirrored by hundreds of untrusted volunteer mirrors. Anyone mirrors; only the keyring signs.

The original project spec explores the full version in `07-mirror-trust.md` and
`12-federation.md`; this is the MVP-sized cut.
