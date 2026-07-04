#!/bin/bash
# Deploy NetPilot as a systemd service on a Linux lab host (e.g. the
# `netpilot-dev` omnictl VM), with the bridge datapath so FRR / container
# node kinds boot for real, backed by local Postgres + Redis.
#
# Run this ON the lab host, from a checkout of the repo. It:
#   - builds the release binary + UI (if not already built),
#   - installs a self-contained copy under /opt/netpilot,
#   - configures the lab host for FRR-in-netns (mask the system frr
#     service; put FRR AppArmor profiles in complain mode),
#   - writes /etc/netpilot.env and a systemd unit, and starts it.
#
# Env (override as needed):
#   NETPILOT_PORT       listen port                (default 8899)
#   NETPILOT_DATA       data dir                   (default /var/lib/netpilot)
#   NETPILOT_DB_URL     Postgres URL   (default postgres://netpilot:netpilot@localhost/netpilot)
#   NETPILOT_REDIS_URL  Redis URL                  (default redis://127.0.0.1/)
#   OPENROUTER_API_KEY / ANTHROPIC_API_KEY / NETPILOT_AI_MODEL  (agent, optional)
set -euo pipefail

REPO="$(cd "$(dirname "$0")/../.." && pwd)"          # containerlab/
NP="$REPO/netpilot"
PORT="${NETPILOT_PORT:-8899}"
DATA="${NETPILOT_DATA:-/var/lib/netpilot}"
DB_URL="${NETPILOT_DB_URL:-postgres://netpilot:netpilot@localhost/netpilot}"
REDIS_URL="${NETPILOT_REDIS_URL:-redis://127.0.0.1/}"

echo "== 1. build (if needed) =="
if [ ! -x "$NP/target/release/netpilot" ]; then
  ( cd "$NP" && cargo build --release -p netpilot-server )
fi
if [ ! -f "$NP/ui/dist/index.html" ]; then
  ( cd "$NP/ui" && npm ci && npm run build )
fi

echo "== 2. install to /opt/netpilot =="
sudo mkdir -p /opt/netpilot "$DATA"
sudo cp "$NP/target/release/netpilot" /opt/netpilot/netpilot
sudo rm -rf /opt/netpilot/ui /opt/netpilot/examples
sudo cp -r "$NP/ui/dist" /opt/netpilot/ui
sudo cp -r "$NP/examples" /opt/netpilot/examples

echo "== 3. lab-host config for FRR-in-netns =="
# NetPilot owns FRR (runs it per-node in namespaces); the host's own frr
# service would collide on the shared /run/frr sockets.
sudo systemctl stop frr 2>/dev/null || true
sudo systemctl mask frr 2>/dev/null || true
# FRR protocol daemons ship enforcing AppArmor profiles that forbid the
# custom per-node socket/pid paths NetPilot uses. Complain mode lets them run.
sudo apt-get install -y -qq apparmor-utils >/dev/null 2>&1 || true
for d in mgmtd zebra ospfd ospf6d bgpd staticd ldpd isisd ripd pimd nhrpd pbrd; do
  [ -x "/usr/lib/frr/$d" ] && sudo aa-complain "/usr/lib/frr/$d" >/dev/null 2>&1 || true
done

echo "== 4. database =="
sudo -u postgres psql -tc "SELECT 1 FROM pg_roles WHERE rolname='netpilot'" | grep -q 1 \
  || sudo -u postgres psql -qc "CREATE USER netpilot WITH PASSWORD 'netpilot' CREATEDB"
sudo -u postgres psql -tc "SELECT 1 FROM pg_database WHERE datname='netpilot'" | grep -q 1 \
  || sudo -u postgres createdb -O netpilot netpilot

echo "== 5. env file (root:600) =="
sudo tee /etc/netpilot.env >/dev/null <<ENV
NETPILOT_DB_URL=$DB_URL
NETPILOT_REDIS_URL=$REDIS_URL
${OPENROUTER_API_KEY:+OPENROUTER_API_KEY=$OPENROUTER_API_KEY}
${ANTHROPIC_API_KEY:+ANTHROPIC_API_KEY=$ANTHROPIC_API_KEY}
${NETPILOT_AI_MODEL:+NETPILOT_AI_MODEL=$NETPILOT_AI_MODEL}
RUST_LOG=info,netpilot=debug
ENV
sudo chmod 600 /etc/netpilot.env

echo "== 6. systemd unit =="
sudo tee /etc/systemd/system/netpilot.service >/dev/null <<UNIT
[Unit]
Description=NetPilot network emulation server
After=network-online.target postgresql.service redis-server.service
Wants=network-online.target

[Service]
Type=simple
EnvironmentFile=/etc/netpilot.env
# root: the bridge datapath and FRR-in-netns need CAP_NET_ADMIN.
User=root
ExecStart=/opt/netpilot/netpilot --data $DATA --listen 0.0.0.0:$PORT --ui /opt/netpilot/ui --datapath bridge
Restart=on-failure
RestartSec=3

[Install]
WantedBy=multi-user.target
UNIT

echo "== 7. start =="
sudo systemctl daemon-reload
sudo systemctl enable netpilot.service >/dev/null 2>&1
sudo systemctl restart netpilot.service
sleep 3
sudo systemctl --no-pager --lines=0 status netpilot.service | head -3
echo
curl -s "http://127.0.0.1:$PORT/api/system" | python3 -c "import json,sys; d=json.load(sys.stdin); print(f\"OK — datapath={d['datapath']} frr={d['frr_available']} auth={d['auth_enabled']} ai={d['ai']['available']}\")"
echo "NetPilot listening on 0.0.0.0:$PORT (default login admin/admin — change it)"
