# NetPilot Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│  React UI (Vite, TypeScript, dark-first)                        │
│  React Flow canvas · xterm.js consoles · AI chat · dashboards   │
└───────────────┬─────────────────────────────────────────────────┘
                │ REST (JSON) + WebSockets (events, consoles, agent)
┌───────────────┴─────────────────────────────────────────────────┐
│  netpilot-server (axum)                                         │
│  routes · console proxy · event fan-out · import/export         │
├──────────────┬───────────────┬──────────────┬───────────────────┤
│ netpilot-core│ netpilot-qemu │ netpilot-net │ netpilot-ai       │
│ lab model    │ qemu cmdline  │ UDP switch   │ Claude client     │
│ templates    │ overlays      │ tap/bridge   │ agent loop        │
│ store        │ QMP, console  │ NAT, netem   │ lab tools         │
│ events       │ config media  │ capture      │ console driver    │
└──────────────┴───────┬───────┴──────┬───────┴───────────────────┘
                       │              │
                 qemu-system-*   UDP sockets / taps+bridges
```

## Crates

### netpilot-core
Pure domain model, no I/O side effects beyond the lab store.
- `Lab`/`Node`/`Link`/`Network`/`Annotation` — the topology document.
  Links connect `Endpoint::Node{node,iface}` to nodes or `Endpoint::Network`.
  Multipoint segments (`bridge`/`nat`/`management`/`cloud`) are `Network`s.
- `NodeTemplate` + `QemuSpec` — per-family boot recipe (NIC model, disk bus,
  machine, cpu flags, config delivery mechanism, mgmt NIC convention).
  Built-in catalog covers the major vendors; user YAML files override.
- `ImageLibrary` — `images/<template>/<version>/*.qcow2` convention
  (EVE-NG-compatible layout idea, no registry required).
- `LabStore` — YAML document per lab under `labs/<uuid>/lab.yaml`,
  atomic write-then-rename. Node runtime artifacts live next to it.
- `EventBus` — tokio broadcast of `Event` (node state, log lines) consumed
  by the events WebSocket and the AI agent.

### netpilot-net — datapath
Two datapaths, chosen per deployment:

**UDP switch (default, rootless).** Every QEMU NIC is a
`-netdev socket,udp=127.0.0.1:<switch-port>,localaddr=127.0.0.1:<nic-port>`.
A per-lab userspace switch (tokio) owns one UDP socket per attached NIC and
forwards frames according to the link table:
- point-to-point link → forward to the peer port
- multipoint network → flood to all other member ports
- per-link `Impairment` (delay/jitter/loss/rate) applied in-switch, live
- capture taps write standard pcap files per port
- rewiring is a table update → **hot connections on running nodes**

This is the GNS3 UDP-tunnel model with ubridge folded into the daemon,
in-process and rootless. Tradeoff: userspace copies (fine for control-plane
labs), silent drop under extreme burst.

**Linux plumbing (privileged mode).** For cloud/NAT/management networks and
wire-speed paths: tap devices enslaved to per-network Linux bridges, host
bridge for `cloud`, `nft masquerade` for NAT/management. Names are
15-char-safe FNV hashes (`npb-`/`npn-`/`npt-`). All syscalls go through a
`Runner` trait (exec `ip`/`tc`/`nft`) so logic is testable without root.

### netpilot-qemu — node orchestration
- `QemuCommand` builder: template `QemuSpec` + node overrides → full argv.
  Deterministic MAC (`52:54:00:…` from lab/node/iface hash) and `-uuid`;
  NICs emitted in strict index order; PCI bridges appended for >26 NICs.
- Disk: `qemu-img create -f qcow2 -F qcow2 -b <base> overlay.qcow2` on first
  start; wipe deletes the overlay; commit folds it into a new base image.
- Console: serial chardev on a unix/TCP socket per node (no telnet framing —
  the server bridges it straight to WebSockets); `-vnc` for GUI nodes.
- QMP unix socket per node: `system_powerdown` for graceful stop (SIGTERM/
  SIGKILL escalation), `set_link` for carrier state.
- Config media builder: cloud-init `cidata` ISO, Cisco CVAC ISO
  (`iosxe_config.txt`), Juniper `juniper.conf` ISO, FAT config disk
  (`ios_config.txt`), assembled per template's `ConfigDelivery`.
- `NodeRuntime` state machine with event publication.

### netpilot-ai — agent mode
- Minimal Claude Messages API client (streaming, tool use) over reqwest.
- `AgentSession` loop: user prompt → model → tool calls → results → model…
  Every tool call and result is emitted to the UI (auditable transcript).
- Tools operate on a `LabToolbox` trait implemented by the server: read lab,
  create/modify/delete nodes/links/networks, set startup configs,
  start/stop nodes, run a command on a node console and return output,
  list templates/images. The agent never touches disk or QEMU directly.
- System prompt encodes topology conventions (mgmt NIC, addressing hygiene)
  and vendor CLI knowledge lives in the model.

### netpilot-db — accounts, RBAC, sharing (optional)
Activated only when `NETPILOT_DB_URL` is set; otherwise the server is
single-user with the file store and a synthetic local admin.
- **Postgres** (sqlx): users (argon2 password hashes), roles
  (`admin`/`operator`/`viewer`), lab ownership + `private`/`public`
  visibility, per-user `view`/`edit` shares, firmware metadata (size, sha256),
  persisted agent sessions, and an append-only `audit_log`.
- **Redis** (optional): session-token store so bearer tokens are shared across
  server instances / survive restarts; falls back to an in-process store.
- **Effective access** — `lab_access(principal, lab)` folds owner + visibility
  + shares + role into `View`/`Edit`/`Own`; `require_view`/`require_edit` are
  the guards every lab-scoped handler calls. There is no global auth
  middleware, so each route self-guards (tested by `tests/vm-rbac-nodes-test.sh`).

### netpilot-server — API
axum, API-first (every UI action is a public endpoint):
```
GET    /api/system                     status (kvm, counts, versions)
GET    /api/templates                  template catalog
GET    /api/images                     image library
GET/POST /api/labs                     list/create
GET/PUT/DELETE /api/labs/:id           lab document CRUD
POST   /api/labs/:id/clone
GET/POST /api/labs/:id/nodes           + PUT/DELETE /nodes/:nid
POST   /api/labs/:id/nodes/:nid/start|stop|wipe
POST   /api/labs/:id/start|stop        bulk with boot delays
GET/PUT /api/labs/:id/nodes/:nid/config     startup config
GET/POST /api/labs/:id/networks        + PUT/DELETE
GET/POST /api/labs/:id/links           + PUT/DELETE (impairment live-applies)
GET/POST /api/labs/:id/annotations     + PUT/DELETE
POST   /api/labs/:id/nodes/:nid/interfaces/:iface/capture/start|stop
GET    /api/labs/:id/nodes/:nid/interfaces/:iface/capture.pcap
POST   /api/labs/:id/nodes/:nid/exec        run a command on the node console
GET    /api/labs/:id/stats                  per-node cpu/rss (from /proc)
GET    /api/labs/:id/export            zip download
POST   /api/import                     zip/.unl/.clab.yml upload
POST   /api/auth/login|logout · GET /api/auth/me      bearer-token auth
GET/POST /api/users                    account management (admin)         ┐ only when
PUT    /api/labs/:id/shares            grant/revoke per-user view|edit    │ NETPILOT_DB_URL
GET    /api/labs/:id/sessions          persisted agent transcripts        ┘ is set
WS     /api/ws/events                  event stream            (token via ?token=)
WS     /api/ws/console/:lab/:node      xterm.js console bridge (token via ?token=)
WS     /api/ws/vnc/:lab/:node          noVNC framebuffer bridge
WS     /api/ws/agent/:lab              AI chat (streaming + tool transcript)
```
Static files: serves `ui/dist` so the whole product is one binary + assets.

**Node launchers.** `netpilot-qemu` runs VM nodes; a native launcher
(`native.rs`) runs the imageless kinds directly on Linux — **netns** nodes
(FRR-in-namespace with per-node socket pathspaces; Linux hosts) and
**container** nodes (docker, veth-wired into the container netns). These need
`--datapath bridge` (CAP_NET_ADMIN); the rootless UDP switch is QEMU-only.

## Data directory
```
<data>/labs/<uuid>/lab.yaml
<data>/labs/<uuid>/nodes/<uuid>/{disk.qcow2, config.iso, console.sock, qmp.sock}
<data>/images/<template>/<version>/*.qcow2
<data>/templates/*.yaml
<data>/captures/<lab>/<node>-<iface>.pcap
```

## Key decisions vs EVE-NG
| | EVE-NG | NetPilot |
|---|---|---|
| Lab format | XML .unl | YAML, declarative, diff-friendly |
| API | PHP, GET-mutations, poll | axum REST + WS events |
| Consoles | Guacamole stack | direct socket↔WebSocket, xterm.js |
| Links | Linux bridges only | userspace UDP switch (hot rewiring, rootless) + bridges |
| Link quality | tc on taps (Pro) | in-switch, works rootless, live |
| Capture | ssh+tcpdump+local Wireshark | in-switch pcap + download |
| AI | none | first-class agent with auditable tools |
| UI | jQuery/jsPlumb | React Flow, dark-first |
