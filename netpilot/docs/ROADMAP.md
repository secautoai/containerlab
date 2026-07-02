# NetPilot Roadmap

A modern, AI-native network emulator: EVE-NG-class QEMU device emulation with a
Rust backend, a React topology UI, and a first-class AI agent that can design,
build, configure, and troubleshoot labs.

Goal: implement the EVE-NG (Community + Pro) feature set and the netpilot.io AI
workflow, with a modern architecture — declarative lab format, API-first
REST+WebSocket backend, dark-mode React Flow canvas, xterm.js consoles, and
auditable agent tool-calls.

Legend: [x] done · [~] partial · [ ] planned

## Phase 0 — Foundation
- [x] Research: EVE-NG feature/API inventory, netpilot.io & competitor landscape, QEMU NOS bootstrapping reference (docs/research/)
- [x] Cargo workspace: netpilot-core / netpilot-qemu / netpilot-net / netpilot-ai / netpilot-server
- [x] React + Vite + TypeScript UI scaffold (xyflow, xterm.js, zustand, tailwind)
- [x] Architecture doc

## Phase 1 — Core domain (EVE-NG lab model, modernized)
- [x] Lab model: nodes, links, multipoint networks, annotations (text/shapes), folders, metadata
- [x] Declarative YAML lab format (vs EVE's XML .unl) with atomic on-disk store
- [x] Device template catalog: Cisco IOSv/IOSvL2/CSR1000v/Cat8000v/XRv9k, Arista vEOS, Juniper vSRX/vJunos-switch, Fortinet, Palo Alto, MikroTik CHR, VyOS, FRR, Linux (cloud-init)
- [x] User-defined templates (YAML drop-in overrides)
- [x] Image library: images/<template>/<version>/ scan, per-template versions
- [x] Interface naming patterns per vendor (Gi0/{i}, ge-0/0/{i}, eth{i}…)
- [x] Startup configs per node; boot-from-config
- [x] Link impairment model (delay/jitter/loss/rate) — EVE-NG Pro "link quality"
- [x] Event bus for live UI updates (vs EVE's poll-only API)

## Phase 2 — QEMU orchestration
- [x] QEMU command builder: machine/cpu/smp/RAM, NIC models (virtio/e1000/vmxnet3…), PCI bridges for high port counts, deterministic MAC/UUID
- [x] qcow2 overlay lifecycle: create-on-start from immutable base, wipe = delete overlay, commit-to-image
- [x] Serial console via chardev socket; VNC for GUI nodes
- [x] QMP control channel: graceful powerdown, link up/down, live capture toggle
- [x] Config injection: cloud-init (cidata ISO), Cisco CVAC ISO, Juniper config ISO, FAT config disk, SMBIOS composer hooks
- [x] KVM detection with TCG fallback
- [x] Node lifecycle state machine (stopped/starting/running/stopping/error) + staggered boot delays
- [ ] Boot-state cache (migrate-to-file) for slow NOSes (XRv9k, PA-VM)
- [ ] Dual-VM platforms (vMX VCP+VFP, vQFX RE+PFE)

## Phase 3 — Datapath
- [x] Userspace UDP switch (Rust): every QEMU NIC = UDP dgram netdev into the switch
  - rootless, hot-wirable links on running nodes (EVE-NG Pro "hot connections")
  - per-link impairment (delay/jitter/loss/rate) applied in-switch, live
  - pcap capture per port/link, downloadable (EVE-NG Pro browser Wireshark)
  - multipoint networks = N-port hub in the same switch
- [x] Privileged mode plumbing: tap + Linux bridge, NAT/management networks (nft masquerade), cloud bridges to host NICs
- [x] `--datapath bridge`: taps/bridges wired into the orchestrator (verified: VM↔VM ping over a kernel bridge), netem impairment, clean teardown
- [x] Deterministic interface naming (15-char safe) and MAC derivation
- [ ] tc mirred cross-connect datapath (LACP/STP transparency)
- [ ] Cross-host links (UDP tunnels between servers — EVE-NG Pro clustering analog)

## Phase 4 — API server (API-first, CML-style)
- [x] REST: labs CRUD, folders, nodes CRUD, links/networks CRUD, node actions (start/stop/wipe, bulk), templates, images, system status
- [x] WebSocket: events stream (node state, logs), console proxy (socket ↔ WS for xterm.js)
- [x] Startup-config get/set API
- [x] Capture start/stop + pcap download
- [x] Static serving of the built UI
- [x] OpenAPI-documented JSON (consistent envelope, proper verbs — no GET-mutations like EVE)
- [ ] Multi-user auth, RBAC, per-user folders (EVE Pro)
- [x] Lab locking (edits rejected while locked) · [ ] countdown timers

## Phase 5 — React UI (modern, dark-first)
- [x] Lab dashboard: lab cards, create/clone/delete, folders
- [x] Topology canvas (React Flow): drag-drop palette by vendor, node icons, curved links with interface labels, multi-select, context menus, minimap, snap grid
- [x] Node/link/network property panels; impairment editor
- [x] Annotations: text + shapes on canvas
- [x] Run controls: start/stop/wipe node & lab, live state colors via events WS
- [x] Console: xterm.js tabs bottom panel, multi-console
- [x] Startup-config editor (per node)
- [x] AI chat panel (streaming, tool-call transcript)
- [x] Image/template manager pages
- [x] VNC viewer embed (noVNC over WS bridge)
- [x] In-browser packet view (live decode summary table) + pcap download for Wireshark
- [x] Lab documentation/workbook panel (Markdown body like EVE)
- [ ] Custom canvas backgrounds/pictures, hotspot maps

## Phase 6 — AI agent mode (netpilot.io-class)
- [x] Agent loop on Claude API with auditable tool-calls surfaced in UI
- [x] Tools: list templates/images, read lab, create nodes/links/networks, set configs, start/stop, run console commands (expect-over-serial), read console output
- [x] NL → topology generation ("build me a 3-router OSPF triangle with a mgmt switch")
- [x] Config generation per vendor syntax
- [x] Troubleshooting: agent reads lab state, runs show commands, proposes fixes
- [x] Streaming responses over WebSocket
- [ ] Post-deploy validation suites (assert adjacencies/reachability)
- [ ] Fault injection scenarios + AI grading (teaching mode)
- [x] MCP server (netpilot-mcp, stdio JSON-RPC) exposing lab control to external agents

## Phase 7 — Interop & import/export
- [x] Lab export/import as zip (topology + configs)
- [x] Containerlab .clab.yml import (nodes/links mapping to templates)
- [x] EVE-NG .unl import (nodes, networks, interfaces, configs, text objects)
- [x] CML2 YAML import (node_definition mapping, interface slots, configs, notes)
- [ ] .pkt export (Packet Tracer)

## Phase 8 — Hardening & scale
- [ ] Multi-server workers (satellite model)
- [ ] Real-time collaborative editing (CRDT)
- [ ] Docker container nodes alongside QEMU (containerlab bridge)
- [ ] VS Code extension speaking the same API
- [x] Image upload UI (streamed, validated) — community index planned
- [ ] Lab store (community topologies with tasks/grading)

## Explicit EVE-NG parity map
| EVE-NG feature | NetPilot status |
|---|---|
| QEMU nodes, per-node overlay, wipe | Phase 2 ✅ |
| IOL / dynamips backends | Out of scope (licensing; QEMU covers modern images) |
| Docker nodes (Pro) | Phase 8 |
| Folders, lab files, lock | Folders ✅, lock ✅ |
| Startup configs + config sets | Configs ✅; sets planned |
| Multi-user pods, RBAC (Pro) | Phase 4 planned |
| Telnet/VNC/HTML5 consoles | Serial→WS ✅, VNC embed planned |
| Cloud/pnet, NAT networks (Pro) | Phase 3 ✅ (privileged mode) |
| Link quality (Pro) | ✅ in-switch, live |
| Hot connections (Pro) | ✅ (UDP switch rewiring) |
| Wireshark capture (Pro) | pcap capture/download ✅; live decode planned |
| Suspend link (Pro) | ✅ in-switch |
| Config export (expect scripts) | ✅ per-template export_command |
| Node resource stats | ✅ /proc-based per node |
| Text objects/shapes/pictures | Text/shapes ✅; pictures planned |
| Link designer (Pro) | Curved links + labels ✅ |
| Import/export zip | ✅ |
| Clustering (Pro) | Phase 8 |
| REST API | ✅ modernized (proper verbs, WS events, OpenAPI) |
| AI agent (nobody has this self-hosted) | ✅ Phase 6 |
