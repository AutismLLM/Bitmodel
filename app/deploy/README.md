# BitModel — VPS deployment

Three always-on services on one box (`YOUR_VPS_IP`), plus a client recipe.

| Service | Unit | Ports | What |
|---|---|---|---|
| **relay** | `bitmodel-relay.service` | `3340/tcp` (HTTP relay), `3478/udp` (STUN) | iroh hole-punch fallback. Runs `iroh-relay --dev` (plain HTTP — no domain/TLS needed for an IP-only box). |
| **registry** | `bitmodel-registry.service` | `8090/tcp` | `model → live seeders` + manifest host, SQLite. |
| **anchor** | `bitmodel-anchor.service` | `11204/udp` (iroh QUIC) | `bitmodel-validator --keep-seeding`: fetches the demo model from origin, signs+publishes the manifest, then seeds forever. Guarantees ≥1 seeder. |

State = one SQLite file (`/opt/bitmodel/data/registry.db`). Binaries in `/opt/bitmodel/bin`, validator key in `/opt/bitmodel/keys`, config in `/etc/bitmodel.env`.

## Deploy

```bash
./deploy/deploy.sh          # build (in a bullseye container) → ship → ufw → systemd
```

Binaries are built inside `rust:1.96-bullseye` so their glibc (2.31) is old enough
to run on the VPS (Ubuntu 22.04, glibc 2.35). No musl needed.

## Client (download a model from anywhere)

```bash
export BITMODEL_REGISTRY=http://YOUR_VPS_IP:8090
export BITMODEL_RELAY=http://YOUR_VPS_IP:3340
bitmodel get --model tinyllama-1.1b --out ./dl --trust deploy/trust.toml
```

`get` verifies the quorum signatures (against `trust.toml`) and BLAKE3-verifies
every byte, then auto-seeds. No tickets, no hardcoded paths.

## Ops

```bash
ssh your-vps 'systemctl status bitmodel-relay bitmodel-registry bitmodel-anchor'
ssh your-vps 'journalctl -u bitmodel-anchor -n 50 --no-pager'
curl http://YOUR_VPS_IP:8090/health
curl http://YOUR_VPS_IP:8090/seeders/tinyllama-1.1b
```
