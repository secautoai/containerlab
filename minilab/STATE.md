# minilab — STATE log

**Protocol.** This file is the build journal of the autonomous builder agent
implementing `PRD.md`. After **every milestone** (M1–M4) a dated entry is
appended, newest last, with exactly three subsections:

1. **What passed** — the milestone's exit test, with verbatim (trimmed)
   command output as evidence.
2. **What failed** — honest account of failures and dead ends hit during the
   milestone, and how each was fixed.
3. **Rules worth remembering** — durable lessons: API quirks, race timings,
   environment facts, flag incantations.

Environment baseline (verified 2026-07-12 before M1): root shell; dockerd
29.3.1 running with `--bridge=none --iptables=false`; Go 1.25.0; module cache
holds yaml.v3 v3.0.1, netlink v1.3.1, netns v0.0.5, x/sys v0.41.0 (including
`cache/download` — go.sum can be generated offline); **no** `ip`, `ping`, or
busybox on the host; registry pulls blocked (403). Host netns has only
`eth0` + `lo`. minilab must run as root (CAP_NET_ADMIN).

---

## 2026-07-12 — M1: topology parse + validate

### What passed
`go test ./...` (also `go vet`, `gofmt -l` clean) — all PRD-required cases:

```
--- PASS: TestParseExample (0.00s)
--- PASS: TestValidateErrors (0.00s)   # 16 subtests: unknown_node, dup_endpoint_across_links,
                                       # dup_endpoint_within_link, bare_node, empty_iface,
                                       # extra_colon, one/three_endpoints, ipv4_longer_than_endpoints,
                                       # ipv4_missing_mask, ipv4_is_v6, missing_image, null_node,
                                       # missing_name, no_nodes, dup_node_name
--- PASS: TestIPv4SkipAndShort (0.00s)
ok  	minilab	0.008s
```

### What failed
- `GOPROXY=off go mod tidy` failed: yaml.v3's go.mod declares a *test* dep
  (`gopkg.in/check.v1`) absent from the local cache, and tidy insists on
  resolving it. Fix: run tidy with the default proxy (allowed through
  HTTPS_PROXY) — it downloaded only check.v1's metadata and produced go.sum.
- `go mod tidy` silently **dropped** the pinned netlink/netns/x/sys requires
  because no file imports them yet. Not fought — they get re-added by the M3
  tidy once links.go imports them.

### Rules worth remembering
- yaml.v3 rejects duplicate mapping keys by itself (`mapping key "n1"
  already defined`), so `topology.nodes` uniqueness needs no custom code.
- `netip.ParsePrefix` (unlike `net.ParseCIDR`) keeps host bits — right tool
  for "10.10.0.1/24"; guard v4-ness with `.Addr().Is4()`.
- Endpoint format check is `strings.Cut(s, ":")` + reject empty halves +
  reject a second ":" in the iface half ("n1:e:x").
- Parsed endpoints are cached on the Link (unexported `eps [2]Endpoint`)
  during Validate, so runtime code never re-splits strings.

---

## 2026-07-12 — M2: node lifecycle (docker.go + main.go)

### What passed
Exit test against a links-free `pair` topology (isolates lifecycle from
wiring; image built by hand with the exact commands that become `make image`):

```
$ ./minilab deploy -t .../pair-nolinks.clab.yml
lab "pair" deployed: 2 nodes, 0 links
$ docker ps --filter label=minilab.lab=pair --format '{{.Names}} {{.State}}'
minilab-pair-n2 running
minilab-pair-n1 running
$ ./minilab inspect -t .../pair-nolinks.clab.yml
NODE  CONTAINER        STATE    PID    INTERFACES
n1    minilab-pair-n1  running  14106  -
n2    minilab-pair-n2  running  14167  -
$ ./minilab destroy -t .../pair-nolinks.clab.yml
lab "pair" destroyed: 2 containers removed, 0 leftover veths swept
$ docker ps -a --filter label=minilab.lab --format '{{.Names}}'
(empty)
$ ./minilab destroy -t ...   # idempotent re-destroy
lab "pair" destroyed: 0 containers removed, 0 leftover veths swept  (exit 0)
```
`inspect --json` emits the same rows as JSON. `docker import --change
'ENTRYPOINT ["/nodeagent"]' rootfs.tar minilab/nodeagent:v1` worked as the
PRD promised.

### What failed
- **nodeagent died instantly, exit 2**: "no args -> sleep forever" was coded
  as `select {}`; with a single goroutine Go's runtime deadlock detector
  panics (`fatal error: all goroutines are asleep - deadlock!`), so both
  containers were `exited` and deploy's waitRunning timed out with
  `state=exited pid=0`. Fix: `for { time.Sleep(time.Hour) }` — a pending
  timer makes the sleeping goroutine legal.
- First `go mod tidy` after adding links.go resolved `golang.org/x/sys` to
  netlink's declared minimum v0.10.0 (downloading it needlessly) instead of
  the cached pin. Fix: `go mod edit -require=golang.org/x/sys@v0.41.0` +
  re-tidy; go.mod now pins exactly the PRD's four versions.

### Rules worth remembering
- Never use `select {}` as sleep-forever in a single-goroutine binary; the
  deadlock detector kills it. A `time.Sleep` loop is immune.
- Go exit-status quirk: runtime fatal errors exit 2 — visible as
  `.State.ExitCode=2` in docker inspect; `docker logs` shows the panic.
- `docker import` of a tar containing just one static binary boots fine;
  `--change 'ENTRYPOINT [...]'` is honored on create.
- `docker ps --format '{{.Label "minilab.node"}}'` can read labels directly
  (handy for debugging label-based discovery).
- waitRunning's bounded poll (20 x 100ms on PID>0 + /proc/<pid>/ns/net stat)
  was the thing that surfaced the crash cleanly — keep its "last reason"
  string, it turned a mystery timeout into `state=exited pid=0`.

---

## 2026-07-12 — M3: links (veth wiring via netlink)

### What passed
Exit test on `examples/pair.clab.yml`:

```
$ ./minilab deploy -t examples/pair.clab.yml
lab "pair" deployed: 2 nodes, 1 links
$ ./minilab inspect -t examples/pair.clab.yml
NODE  CONTAINER        STATE    PID    INTERFACES
n1    minilab-pair-n1  running  15730  eth1:10.10.0.1/24
n2    minilab-pair-n2  running  15790  eth1:10.10.0.2/24
$ docker exec minilab-pair-n1 /nodeagent -probe 10.10.0.2:9000
probe ok: 10.10.0.2:9000        (exit 0)
```

Interfaces verified inside both netns without any `ip` binary, from the host:
`/proc/<pid>/net/dev` lists `lo` + `eth1` in both; `/proc/<pid>/net/fib_trie`
shows `10.10.0.1` / `10.10.0.2` as `host LOCAL`; `/proc/<pid>/root/sys/class/
net/eth1/mtu` = 9500 on both ends (Veth LinkAttrs.MTU propagates to the peer).

Crash-recovery paths, both live-tested with a planted stray pair
(`ml-pair-{9a,9b}`, then colliding `ml-pair-{0a,0b}`):
- `destroy` swept root-ns leftovers: "2 containers removed, 1 leftover veths
  swept", `/sys/class/net` back to `eth0`+`lo` only.
- `deploy` over a stale colliding temp name deleted it first, wired fresh,
  probe passed.

### What failed
- **sweepVeths double-delete bug** (caught by review before the live test,
  confirmed by it): with BOTH stale ends in the root ns, `LinkDel(endA)`
  implicitly destroys peer endB, so deleting from the stale `LinkList()`
  snapshot would hit "no such device" on endB and error out the destroy.
  Fix: re-`LinkByName` each candidate and skip if already gone — hence the
  honest "1 leftover veths swept" for a pair (one delete op kills both).
  The wireLink pre-delete never had the bug because it looks up by name.
- Nothing else: the netns-move recipe (LinkSetNsFd by PID handle →
  NewHandleAt → LinkByName → LinkSetName → AddrAdd → LinkSetUp) worked on
  the first live run.

### Rules worth remembering
- veth pairs are one kernel object: deleting either end removes both. Any
  bulk-delete over a stale link list must re-resolve names before LinkDel.
- The PRD recipe holds: rename/addr/up MUST go through the namespaced handle
  (`netlink.NewHandleAt(nsHandle)`); the root-ns package-level funcs no
  longer see the link after `LinkSetNsFd`. Close both NsHandle and Handle.
- Rename keeps the ifindex, so the `*Handle.LinkByName` result stays valid
  for AddrAdd/LinkSetUp after LinkSetName.
- A link moved into a netns arrives DOWN even if it was up — always
  `LinkSetUp` after the move, never before.
- No-tooling verification tricks: `/proc/<pid>/net/dev` (iface list),
  `/proc/<pid>/net/fib_trie` (addresses), `/proc/<pid>/root/sys/class/net/*`
  (any sysfs attr) all read a container's netns from the host.
- Address+up on a veth auto-installs the connected route (10.10.0.0/24 dev
  eth1) — TCP between the two /24 peers needs nothing else.

---

## 2026-07-12 — M4: end-to-end (Makefile + SC1/SC2/SC3)

### What passed
`make e2e` from a fully clean slate (binary, build/, and the docker image all
removed first) exits 0: image build → deploy → inspect → probe → destroy →
`test -z "$(docker ps -aq --filter label=minilab.lab)"` → "e2e: OK".

The PRD's three success criteria, run verbatim:

```
SC1  $ make image && ./minilab deploy -t examples/pair.clab.yml
     ...
     docker import --change 'ENTRYPOINT ["/nodeagent"]' build/rootfs.tar minilab/nodeagent:v1
     lab "pair" deployed: 2 nodes, 1 links          deploy exit: 0
     $ ./minilab inspect -t examples/pair.clab.yml
     NODE  CONTAINER        STATE    PID    INTERFACES
     n1    minilab-pair-n1  running  19815  eth1:10.10.0.1/24
     n2    minilab-pair-n2  running  19876  eth1:10.10.0.2/24
                                                    inspect exit: 0
SC2  $ docker exec minilab-pair-n1 /nodeagent -probe 10.10.0.2:9000
     probe ok: 10.10.0.2:9000                       probe exit: 0
SC3  $ ./minilab destroy -t examples/pair.clab.yml
     lab "pair" destroyed: 2 containers removed, 0 leftover veths swept
                                                    destroy exit: 0
     $ docker ps -a --filter label=minilab.lab      (only the header — empty)
     $ go test ./...
     ok  minilab 0.008s                             go test exit: 0
```

Post-run hygiene: host netns back to `eth0`+`lo`; zero containers of any
kind; `gofmt -l` and `go vet ./...` clean. Go size: 597 lines across the five
files (main 170, topology 132, docker 113, links 117, nodeagent 65) — within
the ~600 budget.

### What failed
- Nothing in M4 itself. One spec-reading decision: SC1 runs `make image &&
  ./minilab deploy ...`, so the `image` target must also produce the CLI —
  `image: build` dependency added (PRD's M4 text mentions only nodeagent for
  `image`, but SC1 is the authority).

### Rules worth remembering
- `go build -o build/nodeagent` does NOT create the output directory —
  `mkdir -p build` first in the Makefile.
- In a Makefile, shell-substitution assertions need `$$`:
  `test -z "$$(docker ps -aq --filter label=minilab.lab)"`.
- nodeagent's `-probe` retries for up to 5s (200ms steps): `docker exec -d`
  returns before the peer's listener is bound, so a raw single dial could
  race a just-deployed lab; the retry keeps `make e2e` deterministic while
  still exiting 0-on-connect / 1-on-failure.
- Full e2e wall time is ~3s; the PID/netns wait loop never needed more than
  the first iteration on this host, but keep it — it is the only guard
  against the documented docker PID-0 race.
