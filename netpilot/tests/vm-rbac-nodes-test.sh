#!/bin/bash
# Focused RBAC test for the lab-scoped handlers that previously had NO auth
# guard: nodes, topology (networks/links/annotations), capture, interop
# (export/import), system (lab_stats), and the console/vnc/events WebSocket
# bridges. Proves a view-only collaborator can READ but not MUTATE, that a
# viewer-role user can't import, and that the WS upgrades reject a missing
# token. Companion to vm-integration-test.sh (which covers labs.rs).
#
#   NETPILOT_BIN=./target/release/netpilot ./tests/vm-rbac-nodes-test.sh
#
# Requires postgres netpilot:netpilot@localhost/netpilot + redis on 127.0.0.1.
set -u
BIN="${NETPILOT_BIN:-./target/release/netpilot}"
export NETPILOT_DB_URL="postgres://netpilot:netpilot@localhost/netpilot"
export NETPILOT_REDIS_URL="redis://127.0.0.1/"
DATA=/tmp/netpilot-rbac-data
rm -rf "$DATA"; mkdir -p "$DATA"

sudo -u postgres psql -qc "DROP DATABASE IF EXISTS netpilot" >/dev/null 2>&1
sudo -u postgres psql -qc "CREATE DATABASE netpilot OWNER netpilot" >/dev/null 2>&1

"$BIN" --data "$DATA" --listen 127.0.0.1:8096 >/tmp/netpilot-rbac.log 2>&1 &
SRV=$!
trap "kill $SRV 2>/dev/null" EXIT
B=http://127.0.0.1:8096
for i in $(seq 1 30); do curl -sf $B/api/system >/dev/null 2>&1 && break; sleep 1; done

PASS=0; FAIL=0
ok()  { echo "  ✓ $1"; PASS=$((PASS+1)); }
bad() { echo "  ✗ $1"; FAIL=$((FAIL+1)); }
jq_get() { python3 -c "import json,sys; d=json.load(sys.stdin); print(d$1)" 2>/dev/null; }
code() { curl -s -o /dev/null -w "%{http_code}" "$@"; }
# HTTP code for a WS upgrade attempt (auth runs before the handshake).
wscode() {
  curl -s -o /dev/null -w "%{http_code}" \
    -H "Connection: Upgrade" -H "Upgrade: websocket" \
    -H "Sec-WebSocket-Version: 13" -H "Sec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==" "$@"
}

# --- set up: admin, operator alice (owner), viewer bob (view-share) ---
ADMIN=$(curl -s -X POST $B/api/auth/login -H 'content-type: application/json' -d '{"username":"admin","password":"admin"}' | jq_get "['token']")
AH="Authorization: Bearer $ADMIN"
curl -s -X POST $B/api/users -H "$AH" -H 'content-type: application/json' -d '{"username":"alice","password":"alicepw","role":"operator"}' >/dev/null
curl -s -X POST $B/api/users -H "$AH" -H 'content-type: application/json' -d '{"username":"bob","password":"bobpw","role":"viewer"}' >/dev/null
ALICE=$(curl -s -X POST $B/api/auth/login -H 'content-type: application/json' -d '{"username":"alice","password":"alicepw"}' | jq_get "['token']")
BOB=$(curl -s -X POST $B/api/auth/login -H 'content-type: application/json' -d '{"username":"bob","password":"bobpw"}' | jq_get "['token']")
LAB=$(curl -s -X POST $B/api/labs -H "Authorization: Bearer $ALICE" -H 'content-type: application/json' -d '{"name":"rbac-lab"}' | jq_get "['id']")
# alice adds a node (frr = netns, no image needed) so bob's node access can be tested
NODE=$(curl -s -X POST $B/api/labs/$LAB/nodes -H "Authorization: Bearer $ALICE" -H 'content-type: application/json' -d '{"template":"frr","name":"r1","x":100,"y":100}' | jq_get "['id']")
curl -s -X PUT $B/api/labs/$LAB/shares -H "Authorization: Bearer $ALICE" -H 'content-type: application/json' -d '{"username":"bob","access":"view"}' >/dev/null
[ -n "$LAB" ] && [ -n "$NODE" ] && ok "setup: alice owns lab $LAB with node $NODE, shared view→bob" || bad "setup failed (lab=$LAB node=$NODE)"

echo "== nodes.rs: view-share reads OK, mutations denied =="
[ "$(code $B/api/labs/$LAB/nodes -H "Authorization: Bearer $BOB")" = "200" ] && ok "bob GET nodes → 200" || bad "bob list nodes"
[ "$(code $B/api/labs/$LAB/nodes/$NODE -H "Authorization: Bearer $BOB")" = "200" ] && ok "bob GET node → 200" || bad "bob get node"
[ "$(code $B/api/labs/$LAB/nodes/$NODE/interfaces -H "Authorization: Bearer $BOB")" = "200" ] && ok "bob GET interfaces → 200" || bad "bob interfaces"
[ "$(code -X POST $B/api/labs/$LAB/nodes -H "Authorization: Bearer $BOB" -H 'content-type: application/json' -d '{"template":"frr","name":"evil"}')" = "403" ] && ok "bob POST node → 403" || bad "bob create node NOT blocked"
[ "$(code -X POST $B/api/labs/$LAB/nodes/$NODE/start -H "Authorization: Bearer $BOB")" = "403" ] && ok "bob start node → 403" || bad "bob start node NOT blocked"
[ "$(code -X POST $B/api/labs/$LAB/nodes/$NODE/exec -H "Authorization: Bearer $BOB" -H 'content-type: application/json' -d '{"command":"id"}')" = "403" ] && ok "bob exec (root console) → 403" || bad "bob exec NOT blocked"
[ "$(code -X DELETE $B/api/labs/$LAB/nodes/$NODE -H "Authorization: Bearer $BOB")" = "403" ] && ok "bob DELETE node → 403" || bad "bob delete node NOT blocked"

echo "== topology.rs: view reads OK, mutations denied =="
[ "$(code $B/api/labs/$LAB/networks -H "Authorization: Bearer $BOB")" = "200" ] && ok "bob GET networks → 200" || bad "bob list networks"
NETC=$(code -X POST $B/api/labs/$LAB/networks -H "Authorization: Bearer $BOB" -H 'content-type: application/json' -d '{"kind":"bridge"}')
[ "$NETC" = "403" ] && ok "bob POST network → 403" || bad "bob create network → $NETC (expected 403)"
[ "$(code -X POST $B/api/labs/$LAB/annotations -H "Authorization: Bearer $BOB" -H 'content-type: application/json' -d '{"kind":"text","x":0,"y":0}')" = "403" ] && ok "bob POST annotation → 403" || bad "bob create annotation NOT blocked"

echo "== capture.rs: start denied for viewer, summary is view-level =="
[ "$(code -X POST $B/api/labs/$LAB/nodes/$NODE/interfaces/0/capture/start -H "Authorization: Bearer $BOB")" = "403" ] && ok "bob capture start → 403" || bad "bob capture start NOT blocked"
SUM=$(code $B/api/labs/$LAB/nodes/$NODE/interfaces/0/capture/summary -H "Authorization: Bearer $BOB")
[ "$SUM" != "403" ] && ok "bob capture summary → $SUM (view allowed, not 403)" || bad "bob capture summary wrongly 403"

echo "== interop.rs: export is view, import needs writer =="
[ "$(code $B/api/labs/$LAB/export -H "Authorization: Bearer $BOB")" = "200" ] && ok "bob GET export → 200 (view)" || bad "bob export"
[ "$(code -X POST $B/api/import -H "Authorization: Bearer $BOB" --data-binary 'name: x')" = "403" ] && ok "viewer POST import → 403" || bad "viewer import NOT blocked"

echo "== system.rs: lab_stats is view-scoped =="
[ "$(code $B/api/labs/$LAB/stats -H "Authorization: Bearer $BOB")" = "200" ] && ok "bob GET lab stats → 200 (view)" || bad "bob lab stats"

echo "== unauthenticated access is rejected =="
[ "$(code $B/api/labs/$LAB/nodes)" = "401" ] && ok "no-token GET nodes → 401" || bad "no-token nodes NOT 401"
[ "$(code $B/api/labs/$LAB/networks)" = "401" ] && ok "no-token GET networks → 401" || bad "no-token networks NOT 401"

echo "== ws.rs: console/vnc/events upgrades require a token =="
[ "$(wscode $B/api/ws/console/$LAB/$NODE)" = "401" ] && ok "no-token console WS → 401" || bad "no-token console WS NOT 401"
[ "$(wscode $B/api/ws/vnc/$LAB/$NODE)" = "401" ] && ok "no-token vnc WS → 401" || bad "no-token vnc WS NOT 401"
[ "$(wscode $B/api/ws/events)" = "401" ] && ok "no-token events WS → 401" || bad "no-token events WS NOT 401"
# viewer bob cannot open the console (require_edit) even with a valid token
[ "$(wscode "$B/api/ws/console/$LAB/$NODE?token=$BOB")" = "403" ] && ok "view-share bob console WS → 403" || bad "bob console WS NOT 403"

echo
echo "RESULT: $PASS passed, $FAIL failed"
[ "$FAIL" = "0" ] && echo "ALL-GREEN" || echo "SOME-FAILED"
