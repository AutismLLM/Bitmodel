# Role: Client (Downloader)

The peer **fetching** a model. Does the verification that makes everything else trustworthy.

## What it does

```
bitmodel get qwen2.5-1.5b
```

1. **Resolve** - ask the [registry](registry.md) for live seeder NodeIds for the model
   (or take a ticket directly). Fetch the signed record.
2. **Verify truth** - check the record's signatures against the pinned keyring; accept only if
   **≥ quorum** valid signatures. If not, abort - the model is not vouched-for.
3. **Fetch** - dial seeders by NodeId (iroh hole-punches a direct connection; [relay](relay.md)
   fallback if needed) and stream byte ranges, in parallel from multiple seeders.
4. **Verify bytes** - every range is checked against the **BLAKE3 root** as it arrives. Bad
   bytes → reject that range, drop that seeder, fetch elsewhere.
5. **Reassemble** to disk; resume cleanly if interrupted.
6. **Auto-seed** - keep serving by default, becoming a [seeder](seeder.md).

## The two checks the client enforces

| Check | Question | Mechanism |
|---|---|---|
| **Authenticity** | Did trusted parties vouch for this model? | ≥ quorum keyring signatures on the record |
| **Integrity** | Are these the exact right bytes? | BLAKE3 root, verified per byte range |

Both run on the client. This is *why* seeders can be untrusted and the registry can be dumb -
the client never has to trust them.

## Resume & multi-source

Because content is addressed by hash, the client can pull different ranges from different
seeders at once and resume after a kill mid-transfer - no seeder state needed.

See: [Trust model](../trust-model.md).
