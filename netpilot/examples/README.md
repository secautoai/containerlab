# Example labs

Import any of these from the NetPilot dashboard (**Import** → pick the `.zip`).
They use the **built-in FRR node kind**, so they need no device images — just
run the server with `--datapath bridge` on a host with `frr` installed
(`apt install frr`).

Each was built and verified end-to-end through the NetPilot API (FRR 8.4,
network-namespace nodes on the Linux-bridge datapath). Verification drove the
real device consoles via `POST /api/labs/:id/nodes/:node/exec`.

## ospf-multiarea.zip — OSPF multi-area
`r1 (area 1) ─ abr1 ─ area 0 ─ abr2 ─ r4 (area 2)`

Two ABRs join a backbone area 0 to stub areas 1 and 2. **Verified:** r1
installs an inter-area route to `4.4.4.4/32` (r4's loopback) and pings it
end-to-end across three areas.

```
O>* 4.4.4.4/32 [110/30] via 10.1.1.2, eth0     ← inter-area route on r1
1.1.1.1 -> 4.4.4.4: 3 packets transmitted, 3 received, 0% packet loss
```

## bgp-peering.zip — eBGP peering
`r1 (AS 65001) ─ r2 (AS 65100 transit) ─ r3 (AS 65002)`

Each router advertises its loopback; the transit AS re-advertises between the
edges. **Verified:** r1 learns `3.3.3.3/32` via BGP with AS-path `65100 65002`
and pings r3's loopback across two autonomous systems.

## vxlan-evpn.zip — VXLAN EVPN (spine-leaf)
`spine1 (RR) ─ leaf1, leaf2 ─ host1, host2`, VNI 100

OSPF underlay carries loopbacks; BGP EVPN (AS 65000, spine as route reflector)
distributes MAC/IP routes; each leaf runs a VXLAN interface (VNI 100) in a
bridge. **Verified:** the EVPN session reaches Established, VNI 100 shows on
both leafs, and **host1 pings host2 across the VXLAN tunnel** even though they
share an L2 segment stretched over the routed fabric.

```
VNI 100  L2  vxlan100  2 MACs  1 Remote VTEP
host1 -> host2 (192.168.100.2): 3 received, 0% packet loss   ← L2 over VXLAN
```

## mpls-l3vpn.zip — MPLS L3VPN
`pe1 ─ p1 ─ pe2` core (OSPF + LDP), MP-BGP VPNv4, CEs per PE

OSPF IGP + LDP distribute transport labels across the core; PEs peer MP-BGP
VPNv4 to exchange customer routes. **Verified (control plane):** OSPF reaches
pe2's loopback, the **LDP neighbor is OPERATIONAL and labels are distributed**
(local/remote 16/17, imp-null). The VPNv4 overlay address-family and dataplane
forwarding require the kernel `mpls_router` module — FRR logs
`Disabling MPLS support (no kernel support)` when it is absent (as in the
CI container used to build these). On a standard Linux host
(`modprobe mpls_router mpls_iptunnel`) the full L3VPN comes up.

```
show mpls ldp binding:
  ipv4 7.7.7.1/32  7.7.7.0  Local 17  Remote 16  In Use yes   ← LDP labels
```

## dnssec-anycast.zip — DNSSEC + anycast DNS

```
resolver ── core ── root-a   (anycast 10.53.53.53/32 on lo, OSPF)
             ├───── root-b   (anycast 10.53.53.53/32 on lo, OSPF, AXFR secondary)
             ├───── tld      (lab. zone)
             └───── auth     (example.lab. zone)
```

A full signed delegation chain — `.` (served anycast by two roots) →
`lab.` → `example.lab.` — with an unbound validator holding the lab root's
DS as trust anchor. The core router prefers root-a (root-b's link carries
OSPF cost 50), so withdrawing the anycast address from root-a fails the
root service over to root-b without touching the resolver.

**Verified** (2026-07-03, QEMU/TCG on macOS): `dig +dnssec` answers for
`www.example.lab` / TXT / NXDOMAIN all return **status NOERROR/NXDOMAIN with
the `ad` flag**; `dig +nsid @10.53.53.53` shows which root instance served
the query; withdrawing `10.53.53.53/32` from root-a reconverges to
**NSID root-b**, and restoring it moves service back. Watch it live with a
packet capture on core's links — the viewer decodes DNS queries/responses
(`query www.example.lab. A`, `response … NOERROR (2 ans)`).

Unlike the FRR labs above, this one uses QEMU nodes (`linux` template) and
expects a disk image named **`3.22-dns`**: Alpine 3.22 with `unbound`,
`bind`, `bind-tools`, and `frr` preinstalled
(`images/linux/3.22-dns/<name>.qcow2`). The zip imports the topology and
per-node cloud-init configs; `dns_lab.py` is the orchestrator that builds
the lab from scratch and runs the signing/validation/failover phases:

```bash
./dns_lab.py setup && ./dns_lab.py poll     # create + boot
./dns_lab.py sign                           # sign . / lab. / example.lab, start daemons
./dns_lab.py test                           # validated lookups (ad flag)
./dns_lab.py failover break                 # anycast fails over to root-b
```

## sdwan-failover.zip — SD-WAN dual-path failover (+ MPLS/LDP)

```
host-a ── edge-a ══ primary WAN (cost 10, LDP) ══ edge-b ── host-b
              ╚═════ backup WAN (cost 50) ═════╝
```

Two FRR edges with dual WAN paths and SD-WAN-grade OSPF timers
(hello 1s / dead 5s). LDP runs over the primary path with the kernel MPLS
modules loaded, so labels are programmed into the dataplane, not just the
RIB. **Verified** (2026-07-04, QEMU/TCG on macOS): host-a→host-b pings
end-to-end (ttl 62, 0% loss); suspending the primary link via the API
reroutes in ~4 s (route metric 20 → 60 onto the backup), traffic keeps
flowing, and restoring the link moves it back; LDP neighbor OPERATIONAL
with label bindings exchanged and installed:

```
ipv4 10.2.0.0/24   2.2.2.2   local 16 / remote imp-null   in use
$ ip -M route
16 via inet 10.0.12.2 dev eth1 proto ldp        ← kernel LFIB
```

Uses the same `3.22-dns` Alpine+FRR image as the DNS lab. `sdwan_lab.py`
rebuilds it from scratch; try the failover from the UI by clicking the
primary link → Suspend.

## Rebuilding

The FRR labs are generated by `build_labs.py` (the verification harness),
which creates each topology through the API, writes the FRR configs, boots
the nodes, and asserts the protocol state shown above. The DNS lab is
generated and verified by `dns_lab.py`, the SD-WAN/MPLS lab by
`sdwan_lab.py` (server URL via `NETPILOT_BASE`).
