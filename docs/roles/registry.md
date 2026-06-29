# Role: Registry

The **directory** that answers "who has model X right now?" - a tracker. Operator-run (on the
VPS). Holds no model bytes and verifies nothing; it just points clients at live seeders.

## What it does

- Maps **`model → live seeder NodeIds`**.
- Serves the signed **record** (as JSON) for each model.
- (Open-swarm phase) accepts **announce/heartbeat** from seeders and prunes stale ones.

## Why it doesn't need to be trusted

If the registry lies (wrong/dead/malicious NodeIds), the worst case is a failed or slow connect.
It **cannot** cause a bad download, because the [client](client.md) still verifies:
- the record's **≥ quorum signatures** (authenticity), and
- every byte against the **BLAKE3 root** (integrity).

So the registry is a convenience, not a trust anchor.

## Two phases

### Phase 1 - static JSON (MVP / closed list)
```json
// peers.json
{ "qwen2.5-1.5b": ["nodeid:aaa...", "nodeid:bbb..."] }
```
Served as a static file from the VPS (or GitHub raw). No write path, no DB.

### Phase 2 - SQLite + announce (open swarm)
Seeders announce themselves, so the list can't be hand-edited. A tiny endpoint takes
announce/heartbeat; SQLite handles concurrent writes + liveness queries a JSON file can't.

```sql
CREATE TABLE seeders (
  model     TEXT NOT NULL,
  node_id   TEXT NOT NULL,
  last_seen INTEGER NOT NULL,   -- unix secs; prune stale rows
  PRIMARY KEY (model, node_id)
);
```
Client query: live seeders for a model =
`SELECT node_id FROM seeders WHERE model=? AND last_seen > now-300`.

## Alternative: no registry at all

iroh also supports **DNS discovery** and **tickets** (paste a string → reach the content). For a
single model you can demo with a ticket and skip the registry entirely; the registry earns its
place once you have many models and an open, changing seeder set.

See: [Trust model](../trust-model.md) · [Seeder](seeder.md).
