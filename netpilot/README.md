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

Requirements on the lab host: `qemu-system-x86_64` + `qemu-img`
(`apt install qemu-system-x86 qemu-utils`), `/dev/kvm` for hardware
acceleration (TCG fallback works but is slow for big NOS images).

Enable the AI agent:

```bash
export ANTHROPIC_API_KEY=sk-ant-…
# optional: NETPILOT_AI_MODEL=claude-sonnet-5, ANTHROPIC_BASE_URL=…
```

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
Palo Alto, MikroTik CHR, VyOS, FRR, generic Linux with cloud-init). Add your
own as YAML files under `<data>/templates/` — same schema, overrides built-ins
by id, including raw `extra_args` for exotic platforms.

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

Privileged extras (tap/bridge datapath, NAT/management/cloud networks via
`ip`/`nft`) live in `netpilot-net::plumbing` for hosts where NetPilot runs
with CAP_NET_ADMIN.

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
overlays, config media, QMP, supervisor) · `netpilot-ai` (Claude client,
agent loop, tools) · `netpilot-server` (axum API + UI hosting).

Docs: [ROADMAP](docs/ROADMAP.md) · [ARCHITECTURE](docs/ARCHITECTURE.md) ·
[STATUS](docs/STATUS.md) · [research notes](docs/research/).

## Verified

Tested end-to-end in CI-like conditions (QEMU 8.2, TCG, busybox guests):
two VMs booted through the API, consoles bridged to browser xterm.js, ping
across the UDP switch (0% loss), live 100 ms impairment measured at ~205 ms
RTT, pcap capture downloaded, and a full agent turn (mock model) that built
and started a topology.
