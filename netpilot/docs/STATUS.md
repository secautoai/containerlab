# Status Log

Living record of work items. Updated as tasks complete.

## 2026-07-02

| # | Task | Status |
|---|------|--------|
| 1 | Research EVE-NG / netpilot.io / QEMU techniques | ✅ done (docs/research/) |
| 2 | Roadmap + architecture docs | ✅ done |
| 3 | Workspace + UI scaffold | ✅ done |
| 4 | Core domain model + persistence (netpilot-core) | ✅ done — 6 tests |
| 5 | QEMU orchestrator (netpilot-qemu) | ✅ done — 14 tests (cmdline, ISO9660/FAT media, QMP, supervisor) |
| 6 | Network datapath (netpilot-net) | ✅ done — 9 tests (UDP switch fwd/flood/loss/carrier/pcap/hot-rewire) |
| 7 | REST + WS API server | ✅ done — smoke-tested via curl |
| 8 | Topology canvas UI | ✅ done — verified in Chromium (drag-drop, cabling, panels) |
| 9 | Console + monitoring UI | ✅ done — live xterm against real QEMU guest |
| 10 | AI agent mode | ✅ done — full turn verified against scripted mock model; real API needs ANTHROPIC_API_KEY |
| 11 | Import/export interop | ✅ done — zip, EVE-NG .unl, containerlab YAML (tests) |
| 12 | Tests/docs/polish | ✅ done — README, workspace tests green |

## End-to-end verification (this environment, QEMU 8.2 TCG)

1. Built a busybox guest (Ubuntu kernel + initramfs) and a `tiny` user template.
2. Via API: created lab, 2 nodes, 1 link; started both → `running_nodes: 2`.
3. Console (unix socket): configured 10.0.0.1/2 on eth0 both sides.
4. `ping -c 3 10.0.0.2` → **3/3 received, 0% loss, ~2.6 ms** through the userspace UDP switch.
5. Applied 100 ms/dir link delay via `PUT /links/:id` → measured **~205 ms RTT**, cleared it live.
6. Started pcap capture, pinged, stopped, downloaded — valid libpcap header + frames.
7. Browser: dashboard → lab → double-click node → typed in xterm console → real guest output.
8. Agent WS (mock model): get_lab → create_node ×2 → create_link → start → real VM booted.

## Known gaps (tracked in ROADMAP)

- NAT/management/cloud networks reach the host only in privileged mode (plumbing implemented, not wired into the orchestrator path).
- Dual-VM platforms (vMX, vQFX) and boot-state caching not implemented.
- Multi-user auth/RBAC, lab locking not implemented.
- VNC console embed (noVNC) not implemented (VNC nodes get a display, no web viewer yet).
- Agent uses non-streaming completions (events stream per block, not per token).
