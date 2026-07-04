# NetPilot

> This repository is a fork of [srl-labs/containerlab](https://github.com/srl-labs/containerlab).
> Everything under [`netpilot/`](netpilot/) is a **new, from-scratch project** —
> NetPilot, an AI-native network emulator — that lives alongside the upstream
> tree. The upstream containerlab code and its README are unchanged; this file
> is the front door for the NetPilot work.

NetPilot is an EVE-NG-class network emulator with a **Rust backend**, a
**React topology UI** ("Strato"), and a **first-class AI agent** that designs,
builds, configures, and troubleshoots labs through the same API the UI uses.

It runs three execution backends in one topology:

| Kind | Devices | Needs |
|---|---|---|
| **native** (Linux netns) | FRR (OSPF/BGP/EVPN/LDP/IS-IS…), Linux hosts | nothing — no image |
| **container** (docker) | Nokia SR Linux (auto-pulled), Arista cEOS, Juniper cRPD | docker |
| **qemu** (VM) | Cisco IOSv/CSR/Cat8000v/XRv9k, Arista vEOS, Juniper vSRX/vJunos, PAN-OS, FortiGate, MikroTik CHR, VyOS, OpenWrt, Linux clouds | qemu + image |

## Where things live

```
netpilot/
├── crates/
│   ├── netpilot-core     domain model (Lab/Node/Link/Network), store, events, templates
│   ├── netpilot-net      per-lab userspace UDP frame switch + Linux tap/bridge/NAT datapath
│   ├── netpilot-qemu     QEMU orchestration (overlays, QMP, console, config media)
│   ├── netpilot-ai       AI agent loop + tools (OpenRouter / Anthropic)
│   ├── netpilot-db       Postgres + Redis: accounts, RBAC, lab sharing, firmware, sessions
│   └── netpilot-server   axum REST + WebSocket API, native (netns/docker) launcher, UI hosting
├── ui/                   React + Vite + React Flow front end (the "Strato" workspace)
├── deploy/               one-shot Linux-host deployment (systemd, bridge datapath, firmware)
├── examples/             bundled, verified labs (OSPF, BGP, VXLAN EVPN, MPLS L3VPN, …)
└── docs/                 ARCHITECTURE · ROADMAP · STATUS · research notes
```

## Quick start (rootless, single-user)

```bash
cd netpilot
cargo build --release
(cd ui && npm install && npm run build)
./target/release/netpilot --data ~/netpilot-data --listen 127.0.0.1:8090 --ui ui/dist
# open http://127.0.0.1:8090
```

On macOS this runs the rootless **UDP-switch** datapath (QEMU nodes only).
For the FRR/container node kinds you need a Linux host with `--datapath bridge`.

Enable the agent with either provider:

```bash
export OPENROUTER_API_KEY=sk-or-…    # cheap: default deepseek/deepseek-chat
export ANTHROPIC_API_KEY=sk-ant-…    # or Anthropic: default claude-sonnet-5
```

## Deploy on a Linux lab host

[`netpilot/deploy/deploy.sh`](netpilot/deploy/deploy.sh) turns a plain Linux VM
into a full multi-vendor lab host — installs the binary as a systemd service
with the bridge datapath, configures FRR-in-netns, and provisions Postgres so
multi-user mode is on:

```bash
OPENROUTER_API_KEY=sk-or-… netpilot/deploy/deploy.sh   # → http://<host>:8899
DATA=/var/lib/netpilot netpilot/deploy/fetch-images.sh # free NOS images (Alpine, OpenWrt, CHR, SR Linux)
```

See [`netpilot/deploy/README.md`](netpilot/deploy/README.md).

## Multi-user & security

Point NetPilot at Postgres (`NETPILOT_DB_URL`) to turn on accounts, RBAC
(`admin`/`operator`/`viewer`), lab ownership + per-user view/edit sharing, a
firmware library, and resumable agent sessions. There is **no global auth
middleware** — every lab-scoped route self-guards with a `require_view` /
`require_edit` check against the caller's effective access on that lab, so a
read-only collaborator cannot reach another user's console, VNC, packet
capture, or config. Enforcement is covered by two integration tests against
real Postgres + Redis:

```bash
NETPILOT_BIN=./target/release/netpilot netpilot/tests/vm-integration-test.sh    # auth/sharing/firmware
NETPILOT_BIN=./target/release/netpilot netpilot/tests/vm-rbac-nodes-test.sh     # node/topology/capture/ws guards
```

Without `NETPILOT_DB_URL` the server is single-user with a file store and
`auth_enabled: false`.

## Docs

- [netpilot/README.md](netpilot/README.md) — full feature reference
- [netpilot/docs/ARCHITECTURE.md](netpilot/docs/ARCHITECTURE.md) — crate-by-crate design
- [netpilot/docs/ROADMAP.md](netpilot/docs/ROADMAP.md) · [netpilot/docs/STATUS.md](netpilot/docs/STATUS.md)
- [netpilot/deploy/README.md](netpilot/deploy/README.md) — Linux-host deployment
