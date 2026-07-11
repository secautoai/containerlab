# Lab 08 — MPLS L3VPN

**Goal:** be the service provider for once. Build a tiny MPLS core (OSPF + LDP), isolate a
customer in a VRF, carry their routes between PEs with MP-BGP VPNv4, connect the sites with
PE-CE eBGP, and read label stacks off the wire. After this lab, ENARSI's "describe MPLS
operations" items are things you've *watched happen*.

| | |
| --- | --- |
| Blueprint mapping | **ENARSI 2.1** (MPLS operations: LSR, LDP, label switching, LSPs), **2.2** (MPLS L3VPN), **1.7 VRF-Lite** (contrast), **ENCOR 2.2.a** (VRF) |
| Nodes / RAM | 5× IOL / ~3.8 GB |
| Estimated time | 3–4 h |

## Topology

```
  ce1 --------- pe1 ========= p1 ========= pe2 --------- ce2
  AS 65010      |    10.0.0.0/30  10.0.0.4/30  |         AS 65020
  Lo1:          |     <-- AS 65000 core -->    |         Lo1:
  172.16.10.0/24|    Lo0 .1     Lo0 .2   Lo0 .3|         172.16.20.0/24
     192.168.10.0/30 (VRF CUST-A)   192.168.20.0/30 (VRF CUST-A)
```

Roles: **CE** = customer edge (plain IP, no MPLS awareness), **PE** = provider edge (VRFs, MP-BGP,
label imposition), **P** = provider core (label switching only — it will never know a single
customer route; proving that is half this lab).

Baseline: addressing only. The PE→CE interfaces are deliberately **unaddressed** — they get
their IP *after* you place them into the VRF (task 3).

```bash
./deploy.sh deploy lab08 && ./deploy.sh ssh 8 pe1
```

## Task 1 — provider IGP

The core needs to reach the PE loopbacks (BGP next hops *and* LDP router-IDs). OSPF area 0 on
**pe1, p1, pe2** — core links + Loopback0 only, e.g. pe1:

```
router ospf 1
 router-id 10.255.255.1
 passive-interface default
 no passive-interface Ethernet0/2
 network 10.0.0.0 0.0.0.3 area 0
 network 10.255.255.1 0.0.0.0 area 0
```

Verify pe1 ↔ pe2 loopback reachability (`ping 10.255.255.3 source Loopback0`). Note the
loopbacks are /32 — with OSPF that also guarantees exact-match FECs for LDP (a subtle classic:
a /24-advertised loopback breaks the LSP because the label FEC won't match the /32 binding).

## Task 2 — turn on label switching (LDP)

On every **core** interface (pe1 e0/2, p1 e0/1+e0/2, pe2 e0/1), plus a pinned router-id:

```
mpls ldp router-id Loopback0 force
interface Ethernet0/2
 mpls ip
```

Now inspect the machinery — these four commands *are* ENARSI 2.1:

```
pe1# show mpls interfaces               ! where LDP runs
pe1# show mpls ldp neighbor             ! TCP/646 session to 10.255.255.2
pe1# show mpls ldp bindings 10.255.255.3 32     ! LIB: all advertised labels
pe1# show mpls forwarding-table         ! LFIB: what forwarding actually uses
```

Read `show mpls forwarding-table` on **p1** for 10.255.255.3/32: local label (say 17) →
outgoing label `Pop Label`. That's **PHP** (penultimate-hop popping): pe2 advertised
`imp-null (3)` for its own loopback, so p1 removes the transport label before delivery. Follow
one LSP end-to-end (pe1 → p1 → pe2) writing down each hop's in/out label — that's an LSP, and
LDP built it from the OSPF topology automatically.

## Task 3 — the customer VRF

On **pe1** (mirror on pe2 with 192.168.20.1):

```
vrf definition CUST-A
 rd 65000:10
 address-family ipv4
  route-target export 65000:10
  route-target import 65000:10
!
interface Ethernet0/1
 vrf forwarding CUST-A
 ip address 192.168.10.1 255.255.255.252
```

Facts to internalize:

- `vrf forwarding` **erases** any IP on the interface (that's why the baseline left it blank).
- **RD** (route distinguisher) makes overlapping customer prefixes globally unique
  (65000:10:172.16.10.0/24) — it *distinguishes*, nothing more.
- **RT** (route target, an extended community) controls which VRFs *import/export* which routes
  — RTs, not RDs, define the VPN topology (any-to-any here; hub-spoke designs use split RTs).

Verify separation immediately: `ping vrf CUST-A 192.168.10.2` works,
plain `ping 192.168.10.2` fails — different routing table (`show ip route vrf CUST-A`).

## Task 4 — MP-BGP VPNv4 between the PEs

```
! pe1 (mirror on pe2 towards 10.255.255.1)
router bgp 65000
 bgp router-id 10.255.255.1
 neighbor 10.255.255.3 remote-as 65000
 neighbor 10.255.255.3 update-source Loopback0
 address-family vpnv4
  neighbor 10.255.255.3 activate
  neighbor 10.255.255.3 send-community extended
```

`send-community extended` is what carries the RTs — forget it and routes silently fail to
import (a beautiful troubleshooting exercise; IOS adds it automatically under vpnv4, verify
with `show run | section bgp`). Check: `show bgp vpnv4 unicast all summary` — session up,
0 prefixes (nothing feeds it yet).

## Task 5 — PE-CE eBGP

Customer side (ce1, AS 65010; mirror ce2 with AS 65020):

```
router bgp 65010
 bgp router-id 172.16.10.1
 network 172.16.10.0 mask 255.255.255.0
 neighbor 192.168.10.1 remote-as 65000
```

Provider side — inside the VRF address family (pe1; mirror pe2):

```
router bgp 65000
 address-family ipv4 vrf CUST-A
  neighbor 192.168.10.2 remote-as 65010
  neighbor 192.168.10.2 activate
```

Watch the route travel: ce1 `network` → pe1 VRF table → **exported** to VPNv4 (RD prepended,
RT attached, **VPN label allocated**) → iBGP to pe2 → **imported** (RT match) → advertised to
ce2. Verify each stage:

```
pe1# show bgp vpnv4 unicast all              ! both 172.16.x.0/24 with RD 65000:10
pe1# show bgp vpnv4 unicast all labels       ! the VPN label per prefix
pe2# show ip route vrf CUST-A 172.16.10.0    ! via 10.255.255.1, label stack
ce2# show ip route                            ! B 172.16.10.0/24 via 192.168.20.1
```

## Task 6 — end-to-end + label stack forensics

```
ce1# ping 172.16.20.1 source Loopback1
ce1# traceroute 172.16.20.1 source Loopback1
  1 192.168.10.1 ...
  2 10.0.0.2 [MPLS: Labels 17/22 Exp 0] ...     <- TWO labels!
  3 192.168.20.1 [MPLS: Label 22 Exp 0] ...
  4 192.168.20.2 ...
```

Decode hop 2: outer **17** = transport label (LDP, reach pe2's loopback), inner **22** = VPN
label (BGP, identifies VRF CUST-A on pe2). Hop 3 shows only the VPN label — PHP stripped the
transport label at p1. Cross-check the numbers yourself against
`show mpls forwarding-table` (p1) and `show bgp vpnv4 unicast all labels` (pe2) — they'll match
your traceroute exactly.

Now the punchline — on **p1**:

```
p1# show ip route 172.16.20.0
% Network not in table
```

The core forwarded customer traffic it has **no route for** — labels only. That's why MPLS VPN
scales: customer state lives only at the edges. Also confirm on the wire:

```bash
sudo ip netns exec clab-ccnp-lab08-p1 tcpdump -nni eth1 mpls
```

## Task 7 — verification battery

```
show mpls ldp discovery                     ! hellos per interface
show mpls ldp neighbor detail               ! session, address list
show mpls forwarding-table detail           ! LFIB incl. label stacks
show bgp vpnv4 unicast vrf CUST-A summary   ! PE-CE sessions
show ip cef vrf CUST-A 172.16.20.0 detail   ! imposition: full stack resolution
ping vrf CUST-A 192.168.10.2                ! PE-side VRF-aware tools (local CE)
```

Why ping the *local* CE and not the far PE-CE /30? Because the /30s were never **exported**
into VPNv4 — only what BGP carries crosses the core, and the CEs advertise just their LANs.
Try `ping vrf CUST-A 192.168.20.2` and watch it fail for exactly that reason (no route in
pe1's VRF table, and ce2 would lack a return route anyway) — then see challenge 4.

## Challenges

1. Onboard a **second customer** `CUST-B` on the same PEs (rd 65000:20, rt 65000:20, new
   loopbacks with the *same* 172.16.x.0/24 prefixes as CUST-A). Prove overlapping address
   spaces coexist — then explain exactly how the RD makes it possible.
2. Break it three ways and diagnose each from symptoms only: (a) `no mpls ip` on p1's e0/2 —
   why do VPN pings die while PE loopback pings survive… or do they? (b) remove
   `send-community extended`; (c) change pe2's import RT to 65000:99.
3. Convert the PE-CE protocol on site 2 to **OSPF** (`router ospf 10 vrf CUST-A` +
   `redistribute bgp`/`redistribute ospf` at pe2). What's a sham-link and when would you need
   one?
4. Export the PE-CE /30s by adding `redistribute connected` under `address-family ipv4 vrf
   CUST-A` on **pe2** — verify ce1 now learns 192.168.20.0/30 (and task 7's failing ping now
   works). Then filter within the VPN: allow only 172.16.20.0/24 (not the /30) towards ce1
   using a prefix-list on pe1's VRF address family.
5. Compare with **VRF-Lite** (ENARSI 1.7): explain what breaks if you delete LDP but keep the
   VRFs, and how VRF-Lite would have to carry CUST-A between pe1 and pe2 instead (per-VRF
   subinterfaces end-to-end) — why doesn't that scale?

<details><summary>Solution reference</summary>

Final configs in [`solutions/`](solutions/); deploy with `./deploy.sh reset 8 && ./deploy.sh deploy 8 --solved`.
</details>

**Next:** [Lab 09 — Infrastructure security](../lab09-security/README.md)
