#!/usr/bin/env bash
#
# BitModel one-command VPS bring-up (M5).
#
#   ./deploy/deploy.sh
#
# Builds VPS-compatible binaries (in a Debian-bullseye container so the glibc is
# old enough for the target), generates a validator key if missing, ships
# everything to the VPS, opens the firewall, and starts the relay + registry +
# anchor-seeder systemd units. Re-runnable / idempotent.
set -euo pipefail

# --- config (override via env) ---------------------------------------------
# Set these for your own box, e.g. SSH_HOST=my-vps PUBLIC_IP=YOUR_VPS_IP ./deploy.sh
SSH_HOST=${SSH_HOST:-your-vps-ssh-alias}
PUBLIC_IP=${PUBLIC_IP:-YOUR_VPS_IP}
REGISTRY_PORT=${REGISTRY_PORT:-8090}
RELAY_HTTP_PORT=${RELAY_HTTP_PORT:-3340}
RELAY_STUN_PORT=${RELAY_STUN_PORT:-3478}
SEEDER_UDP_PORT=${SEEDER_UDP_PORT:-11204}
TOKEN=${TOKEN:-$(head -c16 /dev/urandom | xxd -p)}
REMOTE=/opt/bitmodel

REPO="$(cd "$(dirname "$0")/.." && pwd)"
BUILD_OUT=${BUILD_OUT:-$REPO/target/vps}

echo "==> 1/6 build VPS binaries (bullseye container)"
mkdir -p "$BUILD_OUT"
docker run --rm \
  -v "$REPO":/work -w /work \
  -v "$BUILD_OUT":/target \
  -v "$HOME/.cargo/registry":/usr/local/cargo/registry \
  -e CARGO_TARGET_DIR=/target \
  rust:1.96-bullseye \
  bash -c 'set -e
    cargo build --release -p bitmodel-cli -p bitmodel-registry -p bitmodel-validator
    cargo install iroh-relay@0.35.0 --features server --root /target/relay'

echo "==> 2/6 validator key"
KEYDIR="$REPO/deploy/keys"; mkdir -p "$KEYDIR"
if [ ! -f "$KEYDIR/validator.key" ]; then
  "$BUILD_OUT/release/bitmodel" keygen --id mirror-a --out "$KEYDIR/validator.key" \
    | tee "$KEYDIR/keygen.txt"
  { echo "quorum = 1"; sed -n '/\[\[validator\]\]/,$p' "$KEYDIR/keygen.txt"; } > "$REPO/deploy/trust.toml"
fi
echo "client trust config: $REPO/deploy/trust.toml"

echo "==> 3/6 render env file"
ENVFILE="$BUILD_OUT/bitmodel.env"
sed -e "s|change-me-please|$TOKEN|g" \
    -e "s|YOUR_VPS_IP|$PUBLIC_IP|g" \
    "$REPO/deploy/bitmodel.env.example" > "$ENVFILE"

echo "==> 4/6 ship files"
ssh "$SSH_HOST" "mkdir -p $REMOTE/bin $REMOTE/data/model $REMOTE/keys"
rsync -avz "$BUILD_OUT/release/bitmodel" "$BUILD_OUT/release/bitmodel-registry" \
           "$BUILD_OUT/release/bitmodel-validator" "$BUILD_OUT/relay/bin/iroh-relay" \
           "$SSH_HOST:$REMOTE/bin/"
rsync -avz "$KEYDIR/validator.key" "$SSH_HOST:$REMOTE/keys/"
rsync -avz "$ENVFILE" "$SSH_HOST:/tmp/bitmodel.env"
rsync -avz "$REPO/deploy/"*.service "$SSH_HOST:/tmp/"

echo "==> 5/6 firewall"
ssh "$SSH_HOST" "ufw allow $REGISTRY_PORT/tcp; ufw allow $RELAY_HTTP_PORT/tcp; \
  ufw allow $RELAY_STUN_PORT/udp; ufw allow $SEEDER_UDP_PORT/udp; ufw status | tail -8"

echo "==> 6/6 install + start units"
ssh "$SSH_HOST" "set -e
  install -m600 /tmp/bitmodel.env /etc/bitmodel.env
  install -m644 /tmp/bitmodel-*.service /etc/systemd/system/
  chmod 600 $REMOTE/keys/validator.key
  systemctl daemon-reload
  systemctl enable --now bitmodel-relay bitmodel-registry
  sleep 2
  systemctl enable --now bitmodel-anchor
  sleep 2
  systemctl --no-pager --lines=0 status bitmodel-relay bitmodel-registry bitmodel-anchor | grep -E 'Active|●' || true"

echo
echo "Deployed. Clients use:"
echo "  BITMODEL_REGISTRY=http://$PUBLIC_IP:$REGISTRY_PORT"
echo "  BITMODEL_RELAY=http://$PUBLIC_IP:$RELAY_HTTP_PORT"
echo "  trust: deploy/trust.toml"
