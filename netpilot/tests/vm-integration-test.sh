#!/bin/bash
# Integration test for NetPilot multi-user persistence against a real
# Postgres + Redis. Exercises auth, RBAC, lab sharing, firmware metadata,
# and audit logging end to end. Build the server first, then:
#
#   NETPILOT_BIN=./target/release/netpilot ./tests/vm-integration-test.sh
#
# Requires postgres reachable as netpilot:netpilot@localhost/netpilot with
# createdb rights, and redis on 127.0.0.1. See README "Multi-user mode".
set -u
BIN="${NETPILOT_BIN:-./target/release/netpilot}"
export NETPILOT_DB_URL="postgres://netpilot:netpilot@localhost/netpilot"
export NETPILOT_REDIS_URL="redis://127.0.0.1/"
DATA=/tmp/netpilot-vm-data
rm -rf "$DATA"; mkdir -p "$DATA"

# Fresh schema each run.
sudo -u postgres psql -qc "DROP DATABASE IF EXISTS netpilot" >/dev/null 2>&1
sudo -u postgres psql -qc "CREATE DATABASE netpilot OWNER netpilot" >/dev/null 2>&1

"$BIN" --data "$DATA" --listen 127.0.0.1:8095 >/tmp/netpilot-vm.log 2>&1 &
SRV=$!
trap "kill $SRV 2>/dev/null" EXIT
B=http://127.0.0.1:8095
for i in $(seq 1 30); do curl -sf $B/api/system >/dev/null 2>&1 && break; sleep 1; done

PASS=0; FAIL=0
ok()   { echo "  ✓ $1"; PASS=$((PASS+1)); }
bad()  { echo "  ✗ $1"; FAIL=$((FAIL+1)); }
# check that $1 (http code or json) matches expectation
jq_get() { python3 -c "import json,sys; d=json.load(sys.stdin); print(d$1)" 2>/dev/null; }

echo "== 1. auth is enabled and login works =="
AUTH=$(curl -s $B/api/system | jq_get "['auth_enabled']")
[ "$AUTH" = "True" ] && ok "server reports auth_enabled" || bad "auth_enabled=$AUTH"

# default admin/admin seeded
ADMIN_TOKEN=$(curl -s -X POST $B/api/auth/login -H 'content-type: application/json' -d '{"username":"admin","password":"admin"}' | jq_get "['token']")
[ -n "$ADMIN_TOKEN" ] && ok "admin logged in (seeded admin/admin)" || bad "admin login failed"

# wrong password rejected
CODE=$(curl -s -o /dev/null -w "%{http_code}" -X POST $B/api/auth/login -H 'content-type: application/json' -d '{"username":"admin","password":"wrong"}')
[ "$CODE" = "401" ] && ok "wrong password → 401" || bad "wrong password → $CODE"

echo "== 2. unauthenticated requests are rejected =="
CODE=$(curl -s -o /dev/null -w "%{http_code}" $B/api/labs)
[ "$CODE" = "401" ] && ok "GET /api/labs without token → 401" || bad "no-token labs → $CODE"

echo "== 3. RBAC: admin creates operator + viewer =="
AH="Authorization: Bearer $ADMIN_TOKEN"
curl -s -X POST $B/api/users -H "$AH" -H 'content-type: application/json' -d '{"username":"alice","password":"alicepw","role":"operator"}' >/dev/null
curl -s -X POST $B/api/users -H "$AH" -H 'content-type: application/json' -d '{"username":"bob","password":"bobpw","role":"viewer"}' >/dev/null
NUSERS=$(curl -s $B/api/users -H "$AH" | python3 -c "import json,sys; print(len(json.load(sys.stdin)))")
[ "$NUSERS" = "3" ] && ok "3 users exist (admin/alice/bob)" || bad "user count=$NUSERS"

ALICE=$(curl -s -X POST $B/api/auth/login -H 'content-type: application/json' -d '{"username":"alice","password":"alicepw"}' | jq_get "['token']")
BOB=$(curl -s -X POST $B/api/auth/login -H 'content-type: application/json' -d '{"username":"bob","password":"bobpw"}' | jq_get "['token']")
[ -n "$ALICE" ] && [ -n "$BOB" ] && ok "operator + viewer logged in" || bad "alice/bob login"

echo "== 4. viewer cannot create labs (RBAC write denial) =="
CODE=$(curl -s -o /dev/null -w "%{http_code}" -X POST $B/api/labs -H "Authorization: Bearer $BOB" -H 'content-type: application/json' -d '{"name":"nope"}')
[ "$CODE" = "403" ] && ok "viewer POST /api/labs → 403" || bad "viewer create → $CODE"

echo "== 5. operator creates a lab, owns it =="
LAB=$(curl -s -X POST $B/api/labs -H "Authorization: Bearer $ALICE" -H 'content-type: application/json' -d '{"name":"alice-lab","description":"owned by alice"}' | jq_get "['id']")
[ -n "$LAB" ] && ok "alice created lab $LAB" || bad "alice create failed"

echo "== 6. isolation: bob cannot see or read alice's private lab =="
BOB_LABS=$(curl -s $B/api/labs -H "Authorization: Bearer $BOB" | python3 -c "import json,sys; print(len(json.load(sys.stdin)))")
[ "$BOB_LABS" = "0" ] && ok "bob's lab list is empty" || bad "bob sees $BOB_LABS labs"
CODE=$(curl -s -o /dev/null -w "%{http_code}" $B/api/labs/$LAB -H "Authorization: Bearer $BOB")
[ "$CODE" = "404" ] && ok "bob GET alice's lab → 404 (hidden)" || bad "bob read → $CODE"

echo "== 7. sharing: alice shares view with bob =="
curl -s -X PUT $B/api/labs/$LAB/shares -H "Authorization: Bearer $ALICE" -H 'content-type: application/json' -d '{"username":"bob","access":"view"}' >/dev/null
CODE=$(curl -s -o /dev/null -w "%{http_code}" $B/api/labs/$LAB -H "Authorization: Bearer $BOB")
[ "$CODE" = "200" ] && ok "after share, bob GET lab → 200" || bad "bob read after share → $CODE"
BOB_LABS=$(curl -s $B/api/labs -H "Authorization: Bearer $BOB" | python3 -c "import json,sys; print(len(json.load(sys.stdin)))")
[ "$BOB_LABS" = "1" ] && ok "shared lab appears in bob's list" || bad "bob sees $BOB_LABS labs"

echo "== 8. view-only share cannot edit =="
CODE=$(curl -s -o /dev/null -w "%{http_code}" -X PUT $B/api/labs/$LAB -H "Authorization: Bearer $BOB" -H 'content-type: application/json' -d '{"description":"hacked"}')
[ "$CODE" = "403" ] && ok "view-shared bob PUT lab → 403" || bad "bob edit → $CODE"

echo "== 9. upgrade to edit share, bob can now edit =="
curl -s -X PUT $B/api/labs/$LAB/shares -H "Authorization: Bearer $ALICE" -H 'content-type: application/json' -d '{"username":"bob","access":"edit"}' >/dev/null
CODE=$(curl -s -o /dev/null -w "%{http_code}" -X PUT $B/api/labs/$LAB -H "Authorization: Bearer $BOB" -H 'content-type: application/json' -d '{"description":"edited by bob"}')
[ "$CODE" = "200" ] && ok "edit-shared bob PUT lab → 200" || bad "bob edit after upgrade → $CODE"

echo "== 10. only owner can delete (bob with edit cannot) =="
CODE=$(curl -s -o /dev/null -w "%{http_code}" -X DELETE $B/api/labs/$LAB -H "Authorization: Bearer $BOB")
[ "$CODE" = "403" ] && ok "edit-shared bob DELETE → 403 (owner only)" || bad "bob delete → $CODE"

echo "== 11. firmware upload records metadata + sha256 =="
head -c 4096 /dev/urandom > /tmp/fw.qcow2
curl -s -X PUT "$B/api/images/linux/testfw/disk.qcow2" -H "Authorization: Bearer $ALICE" --data-binary @/tmp/fw.qcow2 >/tmp/fw-resp.json
SHA=$(jq_get "['sha256']" </tmp/fw-resp.json)
EXPECT=$(sha256sum /tmp/fw.qcow2 | cut -d' ' -f1)
[ "$SHA" = "$EXPECT" ] && ok "firmware sha256 matches ($SHA)" || bad "sha256 $SHA != $EXPECT"
# viewer cannot upload
CODE=$(curl -s -o /dev/null -w "%{http_code}" -X PUT "$B/api/images/linux/nope/x.qcow2" -H "Authorization: Bearer $BOB" --data-binary @/tmp/fw.qcow2)
[ "$CODE" = "403" ] && ok "viewer firmware upload → 403" || bad "viewer upload → $CODE"

echo "== 12. audit log recorded actions =="
NAUDIT=$(sudo -u postgres psql -tA netpilot -c "SELECT count(*) FROM audit_log" 2>/dev/null)
[ "$NAUDIT" -ge 5 ] 2>/dev/null && ok "audit_log has $NAUDIT entries" || bad "audit entries=$NAUDIT"

echo "== 13. redis holds the live sessions =="
NKEYS=$(redis-cli --scan --pattern 'netpilot:session:*' 2>/dev/null | wc -l | tr -d ' ')
[ "$NKEYS" -ge 3 ] 2>/dev/null && ok "redis has $NKEYS session tokens" || bad "redis keys=$NKEYS"

echo "== 14. logout revokes the token =="
curl -s -X POST $B/api/auth/logout -H "Authorization: Bearer $BOB" >/dev/null
CODE=$(curl -s -o /dev/null -w "%{http_code}" $B/api/labs -H "Authorization: Bearer $BOB")
[ "$CODE" = "401" ] && ok "after logout, bob's token → 401" || bad "revoked token → $CODE"

echo
echo "RESULT: $PASS passed, $FAIL failed"
[ "$FAIL" = "0" ] && echo "ALL-GREEN" || echo "SOME-FAILED"
