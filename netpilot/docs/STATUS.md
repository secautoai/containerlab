# Status Log

Living record of work items. Updated as tasks complete.

## 2026-07-02 — final

| # | Task | Status |
|---|------|--------|
| 1 | Research EVE-NG / netpilot.io / QEMU techniques | ✅ done (docs/research/) |
| 2 | Roadmap + architecture docs | ✅ done |
| 3 | Workspace + UI scaffold | ✅ done |
| 4 | Core domain model + persistence (netpilot-core) | ✅ done |
| 5 | QEMU orchestrator (netpilot-qemu) | ✅ done |
| 6 | Network datapath (netpilot-net) | ✅ done |
| 7 | REST + WS API server | ✅ done |
| 8 | Topology canvas UI | ✅ done |
| 9 | Console + monitoring UI | ✅ done |
| 10 | AI agent mode | ✅ done |
| 11 | Import/export interop (zip, .unl, .clab.yml, CML2) | ✅ done |
| 12 | Tests/docs/polish | ✅ done — 49 tests green, clippy/fmt clean |
| 13 | Round 2: link suspend, config export, folders, VNC, image upload, stats | ✅ done |
| 14 | Round 3: MCP server, node exec API, packet decode, lab locking, CML import, agent link-quality tool | ✅ done |

## Feature summary

**Backend (Rust)** — axum REST+WS API; per-lab rootless userspace UDP frame
switch (p2p + flood, live delay/jitter/loss/rate, suspend, pcap, hot rewiring);
QEMU orchestration (qcow2 overlays, deterministic MACs/UUIDs, PCI bridges,
QMP, PDEATHSIG child safety, VNC); pure-Rust config media (cloud-init ISO,
Cisco CVAC ISO, Juniper ISO, FAT config disk); template catalog (13 vendors) +
user YAML templates; image library with streamed uploads; console-driven
config export; per-node /proc stats; lab locking; packet decode
(eth/ARP/IPv4/IPv6/TCP/UDP/ICMP/OSPF); privileged tap/bridge/NAT plumbing.

**UI (React, dark-first)** — dashboard with folders + image manager; React
Flow canvas (drag-drop palette, auto interface allocation, per-end interface
labels, context menus, annotations, minimap); node/link/network panels with
live link-quality sliders and suspend toggle; xterm.js + noVNC consoles;
live packet table; lab workbook; AI chat with auditable tool transcript.

**AI** — agent loop on the Claude API with 12 tools (topology CRUD, configs,
lifecycle, console commands, link quality); `netpilot-mcp` MCP stdio server
exposing 10 tools to external agents.

**Interop** — lab zip export/import, EVE-NG .unl import, containerlab
.clab.yml import, CML2 YAML import.

## End-to-end verification (QEMU 8.2, TCG, busybox guests, this container)

1. Two VMs booted through the API; consoles bridged to browser xterm.js.
2. `ping` across the userspace UDP switch: 3/3, 0% loss, ~2.6 ms.
3. 100 ms/dir link delay applied live → measured ~205 ms RTT; cleared live.
4. Suspend link → frames dropped; resume → forwarding resumes (unit + UI).
5. pcap captured, downloaded, and decoded in-browser (echo request/reply rows).
6. VNC console: RFB 003.008 handshake through the WS bridge to a live QEMU.
7. Agent turn (scripted mock model): get_lab → create ×2 → link → start; the
   started node was a real running VM.
8. MCP stdio session: initialize → tools/list → start VM → run_command on its
   console (`MCP-EXEC-WORKS`) → stop lab.
9. Streamed 1 MiB image upload; path traversal and bad extensions rejected.
10. Lab lock: edits 409 while locked, allowed after unlock (integration test).

11. Bridge datapath (`--datapath bridge`): taps + Linux bridge created by
    the orchestrator, VM↔VM ping over the kernel bridge 3/3 0% loss,
    clean teardown (0 leftover interfaces). netem verified as commands
    (sch_netem module absent in this container's kernel).
12. PDEATHSIG verified: SIGKILL of the server killed all QEMU children.

## Known gaps (tracked in ROADMAP)

- Rootless mode cannot provide NAT/cloud host connectivity (use
  `--datapath bridge` for that).
- Dual-VM platforms (vMX, vQFX), boot-state caching, config sets.
- Multi-user auth/RBAC, countdown timers, clustering.
- Agent responses stream per content block, not per token.
