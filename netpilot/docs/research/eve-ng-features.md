# EVE-NG Feature Inventory (research)

Sources: EVE-NG Community REST API source (api.php), .unl XML schema (eve2cml parser), evengsdk,
official Cookbooks (CE 6.2 / PE 6.4). Versions: Community 6.2.0-4 (free), Professional 6.x, Learning Center.

## 1. Core architecture
- Single Linux host; Apache+PHP web UI + REST API; MySQL for users; labs are files on disk.
- Backends per node `type`: qemu (per-node overlay qcow2 in /opt/unetlab/tmp/<pod>/<lab_uuid>/<node_id>/;
  "wipe" deletes overlay; commit to base with qemu-img commit), iol (IOU binaries + iourc license),
  dynamips (idlepc), docker (Pro), vpcs (Pro).
- Images: filesystem convention /opt/unetlab/addons/qemu/<template>-<version>/hda.qcow2 (disk-name prefix
  selects bus: hda/sda/virtioa). Folder name = template + version; UI enumerates folders.
- Node templates: YAML per device family (RAM, CPU, eth count, console type, icon, qemu options/arch, eth name format).
- Multi-tenancy pods: console TCP ports = 32768 + 128*pod + node_id (community); Pro dynamic ports.
- Tiers: Community (1 open lab, 63 nodes, 2 admins), Pro (1024 nodes, multi-lab, docker, hot links,
  config sets, link quality, clustering via `sat`, RBAC), Learning Center (classes, countdown timers).

## 2. Lab management
- One lab = one .unl XML under /opt/unetlab/labs/; folders = directories; folder CRUD API.
- Metadata: name, author, version, description, body (Markdown), scripttimeout, countdown, lock, grid, linkwidth, uuid.
- Lab locking (Lock/Unlock API); move/clone/close; multiple labs open (Pro).
- Startup configs stored in .unl (<configs><config id=N> base64); node `config` attr = boot mode (0 none, 1 from saved set).
  Export config runs per-template expect scripts. Config sets (Pro): multiple named sets, switch lab between them.
- No first-class VM snapshots (gap). Lab tasks/workbook embedded (<tasks>).

## 3. Node features
- Attrs: id, name, type, template, image, console, cpu, cpulimit, ram, ethernet, nvram, idlepc, uuid, firstmac,
  qemu_options, qemu_version, qemu_arch, delay (staggered boot), sat, icon, config, left/top, slotN modules.
- Console: telnet | vnc | rdp; native clients or HTML5 via Guacamole; HTML5 Desktop (Pro).
- Packet capture: right-click node -> capture interface (sshdump->tcpdump->local Wireshark); Pro in-browser Wireshark + pcap download.
- 100+ templates: Cisco IOL/vIOS/CSR/Cat8k/XRv/NX-OS/ASAv/FTDv, Juniper vMX/vSRX/vQFX, Arista vEOS, Fortinet,
  PA, Check Point, F5, MikroTik, Cumulus, VyOS, pfSense, Windows/Linux. User supplies images.
- Ops: start/stop/wipe (one/selected/all), start with delay, export config, boot from config set.

## 4. Networking
- Everything is a "network" object; p2p link = hidden 2-port bridge. Types: bridge, ovs, pnet0-9 (cloud,
  pnet0 = mgmt), nat0 (Pro, DHCP+NAT), internal/private (Pro).
- Link quality (Pro): bandwidth/delay/jitter/loss + suspend link, live.
- Hot connections (Pro): cable running nodes.
- Interfaces bind by network_id; IOL serial links node-to-node (remote_id/remote_if).

## 5. UI
- HTML5 jQuery/jsPlumb canvas: drag-drop, lasso multi-select, group move, context menus, zoom, snap-to-grid, linkwidth.
- Annotations: text objects (font/size/color/bg) + shapes (rect/ellipse, resize/rotate/z-order) — textobjects API.
- Pictures: upload backgrounds, clickable hotspot maps (picturesmapped) opening consoles.
- Link labels/interface names on link ends; Pro link designer: bezier curviness, midpoint, srcpos/dstpos, labelpos, colors, dash.
- Status icons, interface status view, icon library + custom upload. No dark mode (gap).

## 6. REST API shape (JSend envelope {code,status,message,data}; cookie session)
POST /api/auth/login {username,password,html5}; GET /api/status; GET /api/list/templates/[t];
GET /api/list/networks; GET /api/icons; folders CRUD /api/folders[/path];
POST /api/labs; GET/PUT/DELETE /api/labs/<path>.unl; PUT .../move, .../Lock, .../Unlock;
GET/POST .../nodes; GET/PUT/DELETE .../nodes/<id>; GET .../nodes/<id>/interfaces;
PUT .../nodes/<id>/interfaces {"0":<network_id>}; GET .../nodes/<id>/start|stop|wipe (GET-triggered!);
GET .../nodes/start|stop|wipe (all); PUT .../nodes/<id>/export; networks CRUD .../networks[/<id>];
GET .../links; GET .../topology; GET/PUT .../configs[/<node_id>]; textobjects CRUD; pictures CRUD;
GET .../capture/<node>/<iface>; users CRUD /api/users; POST /api/export (zip), /api/import (multipart).

## 7. Import/export
- .unl XML: <lab name version scripttimeout countdown lock sat id grid linkwidth author>
  <topology><nodes><node ...><interface id name type network_id [curviness...]/>
  <networks><network id type name top left visibility/>
  <objects>: configs (base64), configsets (Pro), textobjects, pictures, tasks; <body> Markdown.
- Export = ZIP of .unl files preserving folder tree; disk state NOT exported.

## 8. Pro-only delta (roadmap targets)
Docker nodes; 1024 nodes; hot links; link quality shaping; link designer; config sets; dynamic console
ports; HTML5 desktop; RBAC/LDAP/MFA; clustering; NAT/internal networks; browser Wireshark; custom
template UI; countdown timers.

## Gaps = differentiation opportunities
No VM snapshots; no declarative/IaC lab format (XML, GET lifecycle); no image distribution; fragile
expect-script config export; no dark mode; no websocket/event API.
