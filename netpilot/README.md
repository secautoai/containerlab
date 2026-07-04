# NetPilot

A modern, AI-native network emulator — EVE-NG-class QEMU device emulation with
a Rust backend, a React topology UI, and a first-class AI agent that designs,
builds, configures, and troubleshoots labs.

![stack](docs/ARCHITECTURE.md)

## Why

EVE-NG and GNS3 proved the category; their architecture shows its age (PHP,
XML lab files, GET-mutation APIs, jQuery canvases, no events, no AI).
NetPilot is a clean rebuild:

| | EVE-NG | NetPilot |
|---|---|---|
| Backend | PHP + shell wrappers | Rust (axum, tokio) |
| Lab format | XML `.unl` | declarative YAML |
| API | poll-only, GET mutations | REST + WebSocket events, proper verbs |
| Links | Linux bridges (root required) | **rootless userspace UDP switch** — hot rewiring, live impairment, in-switch pcap |
| Consoles | Guacamole stack | direct socket ↔ WebSocket ↔ xterm.js |
| UI | jQuery/jsPlumb | React Flow, dark-first |
| AI | none | agent with auditable tool-calls (topology gen, config gen, console troubleshooting) |

## Quick start

```bash
# build
cd netpilot
cargo build --release
(cd ui && npm install && npm run build)

# run (rootless)
./target/release/netpilot --data ~/netpilot-data --listen 127.0.0.1:8090 --ui ui/dist
# open http://127.0.0.1:8090
```

Requirements on the lab host depend on which node kinds you use:
`qemu-system-x86 qemu-utils` for VM nodes (`/dev/kvm` for speed),
`frr` for the built-in native FRR routers, `docker` for container
nodes (SR Linux, cEOS, cRPD). Native/container nodes need
`--datapath bridge`.

Enable the AI agent (either provider):

```bash
# OpenRouter (DeepSeek/Kimi/…, cheap):
export OPENROUTER_API_KEY=sk-or-…          # default model deepseek/deepseek-chat
# or Anthropic:
export ANTHROPIC_API_KEY=sk-ant-…          # default model claude-sonnet-5
# overrides: NETPILOT_AI_PROVIDER, NETPILOT_AI_MODEL, OPENAI_BASE_URL / ANTHROPIC_BASE_URL
```

The server also reads these from a `.env` file (`KEY=VALUE` lines) found in
its working directory or any ancestor — handy for launch configs; real
environment variables take precedence.

## Multi-user mode (optional)

By default NetPilot is single-user with a file store. Point it at Postgres to
turn on accounts, RBAC, lab sharing, firmware metadata, and persisted agent
sessions:

```bash
export NETPILOT_DB_URL=postgres://netpilot:netpilot@localhost/netpilot
export NETPILOT_REDIS_URL=redis://127.0.0.1/   # optional: shared session tokens
```

On first start the schema is migrated and a default **admin / admin** is
seeded (change it). Then:

- **Accounts & RBAC** — three roles: `admin` (everything, user management),
  `operator` (create/run/share own labs), `viewer` (read-only, can't create
  labs but can be granted edit on a shared lab). `POST /api/auth/login`
  returns a bearer token; `/api/users` manages accounts (admin only).
- **Lab ownership & sharing** — each lab has an owner and `private`/`public`
  visibility; owners grant per-user `view`/`edit` via the Share dialog
  (`/api/labs/{id}/shares`). The lab list and every lab route are filtered by
  effective access. There is no global auth middleware: each lab-scoped
  handler self-guards with `require_view`/`require_edit`, so a read-only
  collaborator can't reach another user's console, VNC, packet capture, or
  config (covered by `tests/vm-rbac-nodes-test.sh`).
- **Firmware library** — uploads record size + sha256 and an audit entry;
  `DELETE /api/images/{template}/{version}` removes an image (write access).
- **Agent sessions** — every agent conversation is saved per (lab, user) and
  resumable; `GET /api/labs/{id}/sessions` lists them, and the chat can
  replay a transcript.

Without `NETPILOT_DB_URL` none of this is active and the server behaves
exactly as the single-user file-store build (`auth_enabled: false`).

## Deploy on a Linux lab host

macOS runs only the rootless QEMU datapath. To boot the FRR and container
node kinds for real you need a Linux host with the **bridge datapath**.
[`deploy/deploy.sh`](deploy/deploy.sh) does the whole install — release build,
systemd service on `:8899`, QEMU + Docker, FRR-in-netns host config (masks the
system `frr`, puts the protocol daemons' AppArmor profiles in complain mode),
and a local Postgres so multi-user mode is on:

```bash
OPENROUTER_API_KEY=sk-or-… NETPILOT_AI_MODEL=deepseek/deepseek-v4-flash \
  netpilot/deploy/deploy.sh                              # → http://<host>:8899 (admin/admin)
DATA=/var/lib/netpilot netpilot/deploy/fetch-images.sh  # free NOS images
```

`fetch-images.sh` pulls the images that are legitimately free and directly
downloadable — Alpine (host), OpenWrt, MikroTik CHR, and the public Nokia SR
Linux container. Proprietary NOS (Cisco/Arista/Juniper/PAN/Forti) stay gated
behind vendor logins; upload those from the Images page. Full notes:
[deploy/README.md](deploy/README.md).

## Node kinds & device support

Three execution backends, mixable in one lab:

| Kind | Templates | Needs |
|---|---|---|
| **native** (netns) | **FRR** (full routing suite: OSPF/BGP/EVPN/LDP), **Linux endpoint** | nothing — no image at all |
| **container** (docker) | **Nokia SR Linux** (auto-pulled from ghcr.io), **Arista cEOS** + **Juniper cRPD** (BYOI tarball upload) | docker |
| **qemu** (VM) | Cisco IOSv/IOSvL2/CSR1000v/Cat8000v/XRv9k, Arista vEOS, Juniper vSRX/vJunos, PAN-OS, FortiGate, MikroTik CHR, VyOS, Linux clouds | qemu + image upload |

BYOI container images: Images page → upload the vendor tarball; it is
docker-loaded (`docker save` archives) or docker-imported (filesystem
tarballs like cEOS-lab) and tagged for the template automatically.

## Example labs (bundled, verified)

`examples/` ships four labs built with the zero-image FRR nodes — import
them from the dashboard:

- **OSPF Multi-Area** — area 1 ⇢ ABRs in area 0 ⇢ area 2; inter-area routes + end-to-end ping ✔ verified
- **BGP Peering** — AS 65001 ↔ transit AS 65100 ↔ AS 65002; loopback-to-loopback across AS path ✔ verified
- **VXLAN EVPN** — spine RR + 2 leafs (BGP EVPN, VNI 100) + 2 hosts; L2 stretch ping through the tunnel ✔ verified
- **MPLS L3VPN** — LDP core + MP-BGP VPNv4 between PEs (control plane ✔; forwarding needs kernel `mpls_router`)

## Images

Drop base images into the data directory (immutable; nodes boot copy-on-write
overlays):

```
<data>/images/<template>/<version>/image.qcow2
# e.g. images/vyos/1.5/vyos-1.5.qcow2
#      images/iosv/15.9/vios-adventerprisek9.qcow2
```

`GET /api/templates` lists the built-in template catalog (Cisco IOSv/IOSvL2/
CSR1000v/Cat8000v/XRv9k, Arista vEOS, Juniper vSRX/vJunos-switch, Fortinet,
Palo Alto, MikroTik CHR, VyOS, OpenWrt, FRR, generic Linux with cloud-init).
Add your own as YAML files under `<data>/templates/` — same schema, overrides
built-ins by id, including raw `extra_args` for exotic platforms.

**Default logins.** `linux` (cloud-init) nodes auto-login root on the serial
console and accept **root / netpilot** over SSH; a startup config that is a
full `#cloud-config` (or `#!` script) replaces those defaults. OpenWrt uses
its stock root-without-password console. NOS images keep their vendor
defaults (VyOS `vyos/vyos`, cEOS `admin`, …).

## Startup configs

Set a startup configuration per node (UI side panel, API, or the agent).
Delivery is per-template, generated in pure Rust (no genisoimage/mtools):

- cloud-init NoCloud seed ISO (Linux, FRR, VyOS)
- Cisco CVAC config ISO (`iosxe_config.txt` — CSR1000v, Cat8000v)
- FAT config disk (`ios_config.txt` — IOSv/IOSvL2)
- Juniper `juniper.conf` ISO

## The datapath

Every QEMU NIC is a UDP socket netdev wired into a per-lab userspace switch.
Point-to-point links forward frames port-to-port; multipoint networks flood.
Because wiring is a table, NetPilot gets for free:

- **hot connections** — cable/uncable running nodes (EVE-NG Pro feature)
- **live link quality** — delay/jitter/loss/rate applied in-switch per link
- **packet capture** — per-interface pcap files, downloadable while running
- **rootless operation** — no bridges, taps, or CAP_NET_ADMIN needed

Run with `--datapath bridge` (needs CAP_NET_ADMIN) to switch to kernel
taps + Linux bridges instead: wire-speed forwarding, NAT/management
networks (nft masquerade + gateway IP), cloud networks bridged to host
NICs, and link quality via `tc netem`. Interfaces are torn down cleanly on
lab stop. The UDP switch remains the default because it needs no
privileges and supports in-switch capture/suspend.

## AI agent

Open a lab → **Agent**. The agent operates through the same API the UI uses,
with 11 tools: read lab, list templates, create/update/delete nodes, links,
networks, set startup configs, start/stop, and `run_command` (expect-style
execution on a node's serial console). Every tool call and result renders in
the chat as an expandable, auditable transcript.

```
“Build a 3-router OSPF triangle with VyOS and verify adjacencies come up.”
```

## MCP server

`netpilot-mcp` exposes lab control to any MCP-capable agent (Claude Code,
Claude Desktop, …) over stdio — list/create labs, add nodes and links,
start/stop, set configs, and run CLI commands on node consoles:

```json
{ "mcpServers": { "netpilot": {
    "command": "netpilot-mcp",
    "env": { "NETPILOT_URL": "http://127.0.0.1:8090" } } } }
```

## Import / export

- Export: lab zip (topology + configs) — `GET /api/labs/:id/export`
- Import (`POST /api/import`, or the dashboard button), format sniffed:
  - NetPilot zip / bare `lab.yaml`
  - **EVE-NG `.unl`** — nodes, networks, links (hidden bridges → p2p),
    base64 startup configs, template mapping
  - **containerlab `.clab.yml`** — kind → template mapping

## Development

```bash
cargo test --workspace        # 45+ unit/integration tests
cd ui && npm run build        # typecheck + bundle
```

Crates: `netpilot-core` (domain model, store, events) ·
`netpilot-net` (UDP switch, Linux plumbing) · `netpilot-qemu` (cmdline,
overlays, config media, QMP, supervisor) · `netpilot-ai` (Claude/OpenRouter
client, agent loop, tools) · `netpilot-db` (Postgres accounts/RBAC/sharing +
Redis session tokens) · `netpilot-server` (axum API, native netns/docker node
launcher, UI hosting).

Multi-user integration tests need a real Postgres + Redis (a Linux host — see
[deploy](deploy/README.md)):

```bash
NETPILOT_BIN=./target/release/netpilot tests/vm-integration-test.sh   # auth · sharing · firmware · audit
NETPILOT_BIN=./target/release/netpilot tests/vm-rbac-nodes-test.sh    # per-route RBAC on nodes/topology/capture/ws
```

Docs: [ROADMAP](docs/ROADMAP.md) · [ARCHITECTURE](docs/ARCHITECTURE.md) ·
[STATUS](docs/STATUS.md) · [research notes](docs/research/) ·
[deploy](deploy/README.md).

## Verified

Tested end-to-end in CI-like conditions (QEMU 8.2, TCG, busybox guests):
two VMs booted through the API, consoles bridged to browser xterm.js, ping
across the UDP switch (0% loss), live 100 ms impairment measured at ~205 ms
RTT, pcap capture downloaded, and a full agent turn (mock model) that built
and started a topology.
