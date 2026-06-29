# Role: Validator

**Trusted.** Member of the keyring. Produces the **truth**.

## What it does

For a model, independently:

1. **Fetch** the files from origin.
2. **Hash** them → compute the BLAKE3 root (and per-file roots).
3. **Sign** the record with its Ed25519 key.
4. **Publish** the (multi-)signed record to the registry/VPS.

A validator is usually an **auto-script**, not a person at a keyboard:
```
fetch origin → blake3 → sign → publish
```

## Why multiple validators

One signer = one point of forgery. Several independent validators signing the **same** BLAKE3
root means an attacker must compromise **≥ quorum** keys to fake a model. They cross-confirm.

## Trust = keyring membership

A validator matters only because its public key is in the clients' `trust.toml` and the quorum
threshold counts it. Add/remove validators by editing the keyring.

## Not to be confused with

- **Seeder** - serves bytes, untrusted, anyone. A validator *may* also seed, but the roles are
  separate: validating is signing, seeding is serving.

## Key handling

- Validator **signing keys** are the crown jewels - they define truth. Keep them off public
  seeders; ideally on locked-down boxes / HSM later.
- A seeder's NodeId key is unrelated and low-stakes (identity only, verifies nothing about
  content).

See: [Trust model](../trust-model.md).
