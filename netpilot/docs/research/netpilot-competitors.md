# Research: NetPilot & the Modern Network Lab / Emulation Landscape (2025–2026)

## 1. NetPilot (netpilot.io) — AI-native network emulation SaaS
- Positioning: "AI network engineer" — autonomous agent that designs, builds, validates multi-vendor networks from plain-English prompts. Lab is "the agent's workspace, not the product."
- Built on Containerlab under the hood; each lab isolated; ephemeral by default; browser-based; enterprise on-prem option.
- NOS support: SR Linux, FRR, Linux built-in; Cisco IOL, Juniper cRPD, Arista cEOS, PAN-OS, FortiGate, SONiC via BYOI.
- AI loop: prompt -> topology layout -> per-vendor startup configs -> deploy -> post-deploy validation (routing tables, adjacencies, reachability, VPN paths) -> diagnose + propose fixes for human approval. Multi-turn iteration on running lab; import production configs to build "mirror lab".
- UI: topology canvas, click node -> terminal (SSH), .pkt export for Packet Tracer.
- Pricing: free tier, Plus $8/mo, Pro $20/mo, Enterprise.

## 2. Competitors
- GNS3 3.x: Linux server + new Web-UI, AI assistant + fault injector, web-Wireshark, multi-user, REST API for CI/CD.
- CML 2.x: API-first (every UI action = REST call), YAML topology import/export, HTML5 canvas, dockable Workbench panes/consoles, dark mode, smart annotations (tag-driven auto-grouping), Git repo linking, bundled container images.
- Containerlab: topology-as-code .clab.yml; VS Code extension with TopoViewer (React Flow canvas, drag-drop, auto-layout preset/force/geo, YAML side-by-side, SVG/Draw.io export); Edgeshark one-click Wireshark capture; Clabernetes k8s fan-out.
- netlab: intent-level YAML -> auto IPAM/addressing, OSPF/BGP config generation, Ansible inventory. Lesson: deterministic scaffolding under the LLM.
- EVE-NG 6.x Pro: incumbent, multi-user, aging PHP UI — the market gap.
- PNETLab: iShare2 automated image discovery/install, community Lab Store with tasks/grading, React UI, curved links. Lesson: image acquisition automation + lab store are highly valued.
- AI copilots: Juniper Marvis (agentic detect->correlate->remediate), NetBox Copilot + MCP servers, Forward AI, Arista AVA, Selector AI.

## 3. "Modern UI" checklist (2025–2026)
- Web-first SPA over REST API; React Flow/xyflow canvas (drag-drop, custom nodes/edges, multi-select, auto-layout, curved links, groups, annotations, SVG export)
- Dark mode default; xterm.js + WebSocket terminals (click node -> terminal pane); VNC-in-browser with paste
- In-browser packet capture (click link -> see packets)
- Topology-as-code with two-way YAML<->canvas sync; Git integration
- REST + WebSocket API-first; multi-user/RBAC; real-time co-editing is UNSHIPPED whitespace
- Image management UX near-zero-effort; scale-out story; community lab store; VS Code as second front-end

## 4. AI agent capability catalog
Shipped elsewhere (table stakes): NL->topology gen, per-vendor config gen, post-deploy validation agent, troubleshooting with proposed fixes + human approval, fault injector, NL query over lab state, prod config import -> mirror lab.
Whitespace (differentiators): auditable agent-command transcripts (chat pane whose tool-calls are consoles into nodes), lab explanation/documentation generation, AI-generated+graded scenarios, MCP server for lab platform, CI/CD with AI-authored assertions, pcap-aware troubleshooting.
Design principles: human owns sign-off; agent output reviewable; deterministic scaffolding (IPAM/addressing) under LLM; ephemeral isolated labs.

## 5. Strategy
Converging stack: substrate (containerlab/QEMU) -> REST+WS API -> React Flow + xterm.js + YAML sync -> AI agent layer.
NetPilot flanks: closed source, no realtime collab, no editor story, no MCP story, cloud-first.
Biggest unclaimed: real-time multiplayer, AI teaching mode, auditable agent transcripts, MCP-native control, AI-graded labs.
