# Strato Agent Workspace

Implementation of the **Strato Agent Workspace** design from the Claude Design
project "Network topology lab platform" (`design/Strato Agent Workspace.dc.html`
is the imported source of truth).

Strato is an interactive prototype of an "AI network engineer" workspace:

- **Agent chat** (left) — describe a network in plain language; the agent
  narrates its work as a step timeline (parse → design → configs → deploy →
  validate) and posts clickable validation report cards.
- **Topology canvas** (center) — devices pop in as they deploy, links draw in
  with animated packet dots and subnet labels, OSPF area badges, auto-fit
  viewport, failed links render dashed red with a ⊗ marker.
- **Inspector** (right) — Console (emulated per-vendor SSH CLIs: SR Linux,
  FRR, IOS, cEOS, Alpine, FortiOS — `show version`, `show ip route`,
  `show ip ospf neighbor`, `ping`, history with ↑/↓), Configs (per-device
  files with agent-diff highlighting), Validation (PASS/WARN/FAIL checks),
  Sessions (full audit log: PROMPT/AGENT/STEP/DEPLOY/CHECK/SSH/CLI, exportable
  as `.log`).

This is the design's scripted prototype, faithfully ported: the demo flows are
the suggestion chips (build a multi-area OSPF lab, digital twin from configs,
add an Arista spine, fail/restore a link, re-validate). The console `ping` and
`show ip ospf neighbor` answers are computed from the live topology graph
(link status aware), not canned strings.

## Run

```bash
npm install
npm run dev        # http://localhost:5173
```

URL params (the design's exposed props): `?speed=2` (agent speed 0.5–4),
`?packets=0` (hide packet animation), `?accent=%235a7ff0` (accent color).

## Structure

- `src/App.jsx` — the workspace (1:1 port of the design-container component)
- `src/vendors.jsx` — vendor catalog + device icons
- `src/settings.js` — design props via URL params
- `src/theme.css` — palette variables, keyframes, hover states
- `design/` — original `.dc.html` design source + container runtime, for
  diffing against future design revisions (`/design-sync`)
