# minilab — PRD

## 1. Summary

**minilab** is a from-scratch, ~600-line distillation of containerlab's true kernel: a
declarative YAML topology becomes Docker containers joined by point-to-point virtual wires,
with a deploy / inspect / destroy lifecycle and container discovery via labels. It mirrors
containerlab's layering — **topology model** (parse/validate `*.clab.yml` into nodes + links)
→ **runtime** (Docker: create/start/rm/ps, containers tagged with a discovery label) →
**nodes** (create+start a labeled container per node; the `linux` kind is the whole surface) →
**links** (a `veth` pair per link, each end moved into a container's netns by PID, renamed to
the declared interface, addressed, brought up). It drops everything containerlab wraps around
that kernel (kinds registry, VM nodes, mgmt network, certs, templating, inventories). Just as
real containerlab does, minilab wires the dataplane itself with `github.com/vishvananda/netlink`
and discovers its containers by a label (containerlab uses `containerlab=<lab>`; minilab uses
`minilab.lab=<lab>`), reading each container's netns from `/proc/<State.Pid>/ns/net`.

## 2. Goals

- **Topology subset, containerlab-compatible YAML:** `name`; `topology.nodes.<n>.{image, exec}`
  (`exec` = list of strings run via `docker exec -d` after wiring); `topology.links[].endpoints`
  as `["<node>:<iface>", "<node>:<iface>"]` (exactly 2, brief form).
- **Per-endpoint IPv4 — chosen mechanism (ONE):** a link-level `ipv4` key holding an ordered
  list of CIDR strings, positionally matched to `endpoints` (index 0 → first endpoint). This is
  *identical to containerlab's brief-format link IP syntax* (`ipv4: ["10.10.0.1/24","10.10.0.2/24"]`),
  so it is chosen over a bespoke `minilab.ipv4` var: zero extension, maximal compat. An empty
  string skips that endpoint; a shorter list addresses only the leading endpoints; longer-than-
  endpoints is a validation error. Each entry must parse as an IPv4 `netip.Prefix`.
- **Commands:** `minilab deploy -t <file>`, `minilab destroy -t <file>`, `minilab inspect -t <file> [--json]`.
- **Naming & discovery:** containers named `minilab-<lab>-<node>`, labeled `minilab.lab=<lab>`
  and `minilab.node=<node>`; all discovery (inspect/destroy) is by the `minilab.lab` label,
  never by parsing container names.
- **inspect** prints per node: container name, state, PID (from `docker inspect`), plus the
  declared interface name(s) and IPv4(s) from the topology model. `--json` emits the same as JSON.
- **Idempotent destroy:** removing an absent/partly-removed lab exits 0; also sweeps leftover
  host-side veths carrying the lab prefix (see Risks).
- **Ordering:** deploy is strictly nodes→links (all containers created+started and running before
  any wire is built).

## 3. Non-goals (one line each; what real containerlab does there)

- **VM kinds (vrnetlab):** clab boots QEMU VMs wrapped in containers; minilab runs containers only.
- **Management network / DNS / `/etc/hosts`:** clab builds a Docker bridge for OOB mgmt + IPAM +
  hosts entries. minilab has **no** mgmt network in v1 — the daemon here runs `--bridge=none`, so
  every container is `--network=none` and the *only* dataplane is the veth wires minilab creates
  (this is also the honest clab-like design).
- **TLS/certs:** clab provisions a per-lab CA + node certs on boot; minilab does none.
- **startup-config templating / magic vars / Go-template topologies:** clab renders configs and
  `__clab*__` vars; minilab parses literal YAML only.
- **graph:** clab emits a topology graph; minilab does not.
- **`exec` *subcommand*:** clab has an ad-hoc `clab exec` against a running lab. minilab supports
  only the per-node `exec:` topology field (needed to launch the TCP server), not the subcommand.
- **Multi-runtime (podman) & IPv6:** clab abstracts runtimes and supports v6; minilab is Docker + IPv4 only.

## 4. Stack (choice → environment fact / WHY)

- **Test images built locally via `docker import`.** The outbound proxy BLOCKS registry pulls
  (docker.io/quay.io/ghcr → 403), so no image can be pulled. Build one static Go binary
  (`CGO_ENABLED=0`), tar it, `docker import --change 'ENTRYPOINT ["/nodeagent"]' rootfs.tar
  minilab/nodeagent:v1` — proven to work on this host.
- **`nodeagent` binary** (the container entrypoint): **no args → sleep forever** (keeps the
  `--network=none` container alive); `-serve <:port>` → TCP listener that accepts + echoes;
  `-probe <ip:port>` → `net.DialTimeout`, exit 0 on connect / 1 on failure. TCP is the only
  connectivity test available: the host has **no `ping`** (no ICMP test) and **no `ip`/iproute2**
  (cannot shell out for link ops or verification).
- **Link plumbing = `github.com/vishvananda/netlink` from Go, never shelling out.** Because there
  is no `ip` binary, veth create, netns move-by-PID, rename, addr, and up must all be netlink
  syscalls. netns handle via `github.com/vishvananda/netns` (`GetFromPid`).
- **Containers run `--network=none`.** The daemon runs `--bridge=none --iptables=false`; there is
  no default docker bridge (`docker network ls` shows only `host` + `none`). `--network=none` is
  mandatory and matches the "only dataplane is our veths" design.
- **Docker control via the `docker` CLI through `os/exec`** (not the SDK) with `--format` for
  parsing — keeps the dependency surface tiny.
- **Dependency surface = exactly:** stdlib + `gopkg.in/yaml.v3` + `github.com/vishvananda/netlink`
  (+ its deps `github.com/vishvananda/netns`, `golang.org/x/sys`). minilab gets **its own
  `go.mod`** (standalone module so the parent build ignores it). Pin the parent repo's versions so
  the local module cache satisfies them offline: **yaml.v3 v3.0.1, netlink v1.3.1, netns v0.0.5,
  x/sys v0.41.0** (all verified present in `/root/go/pkg/mod`). Go **1.25**.
- **Size budget:** ≤ ~600 lines of Go across ≤ 5 files (`main.go`, `topology.go`, `docker.go`,
  `links.go`, `cmd/nodeagent/main.go`) + `examples/pair.clab.yml` + `Makefile` + unit tests.

### Netns-move technique (bake in — the crux)

1. `pid = docker inspect -f '{{.State.Pid}}' <container>`; require `pid > 0` and that
   `/proc/<pid>/ns/net` exists (retry — see Risks).
2. Create the pair in the root ns: `netlink.LinkAdd(&netlink.Veth{LinkAttrs{Name: hostA, MTU},
   PeerName: hostB})` with deterministic host-side temp names carrying the lab prefix.
3. For each end: `nsh, _ := netns.GetFromPid(pid)`; `netlink.LinkSetNsFd(link, int(nsh))` to push
   it into the container netns.
4. Operate *inside* that netns without switching threads via a namespaced handle
   `h, _ := netlink.NewHandleAt(nsh)`: `h.LinkByName(hostX)` → `h.LinkSetName(link, "<iface>")` →
   (if addr) `a,_ := netlink.ParseAddr("10.10.0.1/24"); h.AddrAdd(link, a)` → `h.LinkSetUp(link)`.
   (Fallback if needed: `runtime.LockOSThread` + `netns.Set`.) Close every `NsHandle`/`Handle`.

## 5. Milestones (each with a concrete exit test)

- **M1 — topology parse + validate.** `topology.go`: structs + `Parse(path)` + `Validate()`.
  Rules: node names unique; every link endpoint `"<node>:<iface>"` splits on exactly one `:` into
  a known node + non-empty iface; exactly 2 endpoints/link; no duplicate `<node>:<iface>` across
  all links; `ipv4` length ≤ 2 and each non-empty entry a valid IPv4 CIDR.
  *Exit test — `go test ./...` covers:* (a) happy parse of `examples/pair.clab.yml`; (b) unknown
  node referenced in a link → error; (c) duplicate endpoint → error; (d) bad endpoint format
  (`"n1"`, `"n1:"`, `"n1:e:x"`) → error.
- **M2 — node lifecycle.** `docker.go` + deploy/destroy/inspect wiring. `deploy` creates
  (`docker create --network=none --name minilab-<lab>-<n> --label minilab.lab=<lab> --label
  minilab.node=<n> --hostname <n> <image>`) then `docker start`s each node; entrypoint `/nodeagent`
  (sleeps). `destroy` lists by `--filter label=minilab.lab=<lab>` and `docker rm -f`s them.
  `inspect` lists name/state/pid.
  *Exit test:* `deploy` → `docker ps --filter label=minilab.lab=pair` shows both running;
  `inspect` prints their PIDs; `destroy` → filter shows nothing.
- **M3 — links.** `links.go`: for each link, build the veth, move both ends into the two
  containers' netns by PID, rename to declared ifaces, add IPv4 per §2, set up. Deploy order is
  nodes→links (containers running before wiring).
  *Exit test:* after `deploy`, `docker exec minilab-pair-n1 /nodeagent -probe 10.10.0.2:9000`
  exits 0 (n2 started `-serve :9000` via its `exec` list); interfaces `eth1` exist in both netns
  with the declared IPs.
- **M4 — end-to-end.** `make image` builds `nodeagent` static + `docker import`s
  `minilab/nodeagent:v1`; `make e2e` = deploy `examples/pair.clab.yml` → probe n1→n2 exits 0 →
  destroy → assert no `minilab.lab` containers remain.
  *Exit test:* `make e2e` exits 0. **STATE.md protocol:** after every milestone, append a dated
  entry — what passed, what failed, and rules worth remembering (e.g. netns-handle lifecycle,
  PID-race backoff values, `docker import` change flags).

## 6. Success criteria (verbatim — human-verifiable)

- **SC1:** `cd minilab && make image && ./minilab deploy -t examples/pair.clab.yml` exits 0, and
  `./minilab inspect -t examples/pair.clab.yml` shows both nodes `running` with their wired
  interface names and IPs.
- **SC2:** `docker exec minilab-pair-n1 /nodeagent -probe 10.10.0.2:9000` exits 0 — proving L3
  connectivity over the minilab-created veth wire (no docker network involved; containers are
  `--network=none`).
- **SC3:** `./minilab destroy -t examples/pair.clab.yml` exits 0 and
  `docker ps -a --filter label=minilab.lab` shows nothing; `go test ./...` passes.

## 7. Risks & mitigations

- **netlink needs root.** All veth/netns syscalls require `CAP_NET_ADMIN`. Run minilab as root;
  document it in the README/STATE.md and fail fast with a clear message on `EPERM`.
- **PID-based netns races.** A just-started container may report PID 0 or a not-yet-populated
  `/proc/<pid>/ns/net`. After `docker start`, poll `docker inspect .State.Pid` and the ns path
  with bounded retry/backoff (e.g. 10 tries × 100ms) before wiring.
- **Leftover veths on crash.** If minilab dies mid-wire, one end may sit in the root ns. Use
  deterministic host-side temp names carrying the lab prefix (e.g. `ml-<lab8>-<idx>`, ≤15 chars);
  `destroy` sweeps root-ns links via `netlink.LinkList()` and deletes any name with that prefix,
  so cleanup is complete even after a crash.
- **docker CLI output parsing.** Never scrape human tables. Always use `--format` (Go templates)
  / `--filter`, e.g. `docker inspect -f '{{.State.Pid}} {{.State.Status}}'`, and treat "no such
  container" as success in the idempotent destroy path.
