# Lab 02 — Multi-area OSPF

**Goal:** configure and dissect a 3-area OSPF domain: adjacencies and DR/BDR behavior, network
types, cost engineering, inter-area routing, summarization, externals, stub/totally-stubby/NSSA
areas, default-route origination and cryptographic authentication.

| | |
| --- | --- |
| Blueprint mapping | **ENCOR 3.2.b** (OSPFv2: neighbor adjacencies, point-to-point vs broadcast, passive interfaces, areas), **ENARSI 1.10** (network types, path preference, operations, areas/LSAs, summarization, filtering, stub/NSSA, authentication) |
| Nodes / RAM | 4× IOL / ~3 GB |
| Estimated time | 3–4 h |

## Topology

```
   area 1 (-> totally stubby)   area 0 (backbone)      area 2 (-> NSSA)
  r1 ----------------------- r2 ------------------ r3 ----------------------- r4
     e0/1   10.0.12.0/24 e0/1  e0/2 10.0.23.0/24 e0/1 e0/2  10.0.34.0/24  e0/1
  Lo1-4: 172.16.0-3.0/24 (ABR) Lo100: 203.0.113.0/24 (ABR)  Lo1: 192.168.44.0/24
  "branch LANs"                "external" (ASBR)            "legacy" (NSSA ASBR)
                                                            Lo2: 172.31.4.0/24
```

Router IDs: `10.255.255.N` (Loopback0). All interface IPs are pre-configured — verify with
`show ip interface brief` — but **no OSPF is running yet**.

```bash
./deploy.sh deploy lab02 && ./deploy.sh ssh 2 r1    # admin/admin
```

## Task 1 — bring up the backbone and area 1

Two configuration styles exist; learn both. On **r1** and **r2**, use classic `network`
statements:

```
! r1
router ospf 1
 router-id 10.255.255.1
 network 10.0.12.0 0.0.0.255 area 1
 network 10.255.255.1 0.0.0.0 area 1
 network 172.16.0.0 0.0.3.255 area 1

! r2
router ospf 1
 router-id 10.255.255.2
 network 10.0.12.0 0.0.0.255 area 1
 network 10.0.23.0 0.0.0.255 area 0
 network 10.255.255.2 0.0.0.0 area 0
```

On **r3**, network statements again (e0/1 → area 0, e0/2 → area 2, Lo0 → area 0; **exclude
Lo100** — it becomes an external later). On **r4**, use the interface style:

```
router ospf 1
 router-id 10.255.255.4
interface Ethernet0/1
 ip ospf 1 area 2
interface Loopback0
 ip ospf 1 area 2
interface Loopback2
 ip ospf 1 area 2
```

Verify adjacencies and routes:

```
r2# show ip ospf neighbor

Neighbor ID     Pri   State           Dead Time   Address         Interface
10.255.255.1      1   FULL/DR         00:00:36    10.0.12.1       Ethernet0/1
10.255.255.3      1   FULL/DR         00:00:33    10.0.23.3       Ethernet0/2

r1# show ip route ospf
...
O IA  10.0.23.0/24 [110/20] via 10.0.12.2, ...
O IA  10.0.34.0/24 [110/30] via 10.0.12.2, ...
O IA  172.31.4.1/32 [110/31] via 10.0.12.2, ...
```

Notes: `FULL/DR` means the *neighbor* is the DR for that segment — and since DR elections are
**non-preemptive**, your DR/BDR columns depend on the order/timing in which you enabled OSPF
(configure r2 within r1's 40 s wait timer and the higher RID wins instead). `O IA` = inter-area (Type-3
LSA from an ABR). If a neighbor hangs in `EXSTART/EXCHANGE`, think MTU mismatch; stuck `INIT` =
one-way hellos; no neighbor at all = area/subnet/timer/authentication mismatch — the four
classics ENARSI loves.

## Task 2 — two gotchas: loopback /32s and passive interfaces

Look at r2's routing table: r1's branch LANs show up as **/32s** (`172.16.0.1/32` …) even though
they are /24s. Loopbacks default to OSPF network type `LOOPBACK`, always advertised as host
routes. Fix on r1:

```
interface range Loopback1 - 4
 ip ospf network point-to-point
```

Re-check r2: now `O 172.16.0.0/24 ...` etc. Second gotcha: r1 is happily sending hellos out
every OSPF-enabled interface. Suppress hellos where no neighbor can exist — on **all four**
routers:

```
router ospf 1
 passive-interface default
 no passive-interface Ethernet0/1
```

(r2/r3 also need `no passive-interface Ethernet0/2`.) Watch the adjacencies drop and return as
you type — and remember: a **passive interface is still advertised**, it just sends no hellos.

## Task 3 — DR/BDR and network types

On the r1–r2 Ethernet segment (type BROADCAST) a DR/BDR pair exists:

```
r1# show ip ospf interface Ethernet0/1
  ...  Network Type BROADCAST, Cost: 10
  ...  Designated Router (ID) 10.255.255.2, Interface address 10.0.12.2
  ...  Backup Designated router (ID) 10.255.255.1, ...
  Timer intervals configured, Hello 10, Dead 40, Wait 40, Retransmit 5
```

1. Force r1 to win the next election: `ip ospf priority 100` on r1 e0/1, then
   `clear ip ospf process` on both (election is **non-preemptive** — priority alone doesn't
   steal DR-ship; the exam tests exactly this).
2. A DR is pointless on a link with two routers. Convert both ends to point-to-point:

    ```
    interface Ethernet0/1
     ip ospf network point-to-point
    ```

    `show ip ospf interface e0/1` — no DR/BDR line, state `P2P`, and the Type-2 (network) LSA
    for that segment disappears from `show ip ospf database`.

## Task 4 — cost engineering

Default reference bandwidth is 100 Mb/s, so a 10 Mb IOL Ethernet costs 10 and anything ≥100 Mb
costs 1 — useless in modern networks. Set a 10 Gb/s reference on **all four** routers
(consistency matters — mismatched references corrupt path selection):

```
router ospf 1
 auto-cost reference-bandwidth 10000
```

Interfaces now cost 1000 (`10000 Mb / 10 Mb`). Confirm end-to-end metrics:
r1 → `172.31.4.1/32` should be `[110/3001]` (three 1000-cost hops + loopback 1). Then override a
single link: `ip ospf cost 500` on r1 e0/1 and watch the routes re-price. Priority of methods:
interface `ip ospf cost` beats bandwidth-derived cost. Remove the override afterwards
(`no ip ospf cost`) so the metrics in the remaining tasks match.

## Task 5 — inter-area summarization (ABR)

r1's four branch LANs (172.16.0.0/24 – 172.16.3.0/24) are contiguous. Summarize them at the
**area border**, on r2:

```
router ospf 1
 area 1 range 172.16.0.0 255.255.252.0
```

Verify on r3: the four Type-3 LSAs collapse into one `O IA 172.16.0.0/22`. On r2 notice the
automatic **discard route** `O 172.16.0.0/22 ... Null0` (loop prevention). Key exam facts:
`area range` summarizes *into* other areas at an ABR and takes the area the routes are *from*;
`summary-address` (task 6) is for ASBRs.

## Task 6 — externals: redistribution at an ASBR

Make r3 an ASBR by injecting its "external" 203.0.113.0/24:

```
route-map RM-EXTERNAL permit 10
 match interface Loopback100
router ospf 1
 redistribute connected subnets route-map RM-EXTERNAL
```

On r1:

```
r1# show ip route 203.0.113.0
Routing entry for 203.0.113.0/24
  Known via "ospf 1", distance 110, metric 20, type extern 2, forward metric 2000
```

**E2** (default): metric fixed at 20 domain-wide, internal cost only breaks ties ("forward
metric"). Change it to **E1** (`redistribute ... metric-type 1`) and compare — now the metric
grows per hop (20 + path cost). E1 vs E2 selection is a guaranteed exam topic.

## Task 7 — stub, totally stubby, NSSA

Area 1 routers don't need externals (they'd exit via r2 anyway). Filter LSA types at the border:

```
! r1 AND r2 (stub flag must match or adjacency drops!)
router ospf 1
 area 1 stub

! r2 only - upgrade to TOTALLY stubby (no-summary is ABR-only)
router ospf 1
 area 1 stub no-summary
```

r1's table collapses to intra-area routes + one default:

```
r1# show ip route ospf
O*IA  0.0.0.0/0 [110/1001] via 10.0.12.2
```

Area 2 has a twist: r4 must redistribute its legacy 192.168.44.0/24 — an ASBR **cannot live in a
stub area**. That's what NSSA is for:

```
! r3
router ospf 1
 area 2 nssa default-information-originate

! r4
router ospf 1
 area 2 nssa
route-map RM-LEGACY permit 10
 match interface Loopback1
router ospf 1
 redistribute connected subnets route-map RM-LEGACY
```

Verify the Type-7 → Type-5 translation:

```
r4# show ip ospf database nssa-external     ! Type 7, originated by r4
r2# show ip route 192.168.44.0              ! arrives as O E2 - translated by r3
r4# show ip route ospf | include 0.0.0.0    ! O*N2 default from r3
```

Why `default-information-originate` on r3's NSSA statement? Careful — **not** for
203.0.113.0/24: r3 is simultaneously the NSSA ABR *and* an ASBR, and by default it injects its
own redistributed prefixes into area 2 as Type-7 too (verify: `show ip route 203.0.113.0` on r4
shows `O N2` even without the default; the `area 2 nssa no-redistribution` keyword would
suppress that copy — a favorite exam knob). The NSSA default exists for **Type-5-only**
destinations that the NSSA blocks — like the domain default you'll originate in task 8. An NSSA
does not auto-generate that default unless told to (totally-NSSA `no-summary` would).

## Task 8 — domain default route

Give the whole domain a way out via r3 (imagine Lo100 is the internet uplink):

```
! r3
ip route 0.0.0.0 0.0.0.0 Null0 250          ! stand-in for a real upstream next hop
router ospf 1
 default-information originate
```

r2 now shows `O*E2 0.0.0.0/0`. Without the `always` keyword the default is originated **only
while r3 itself has a default** — pull the static and watch it vanish.

## Task 9 — authentication

Protect the backbone link r2↔r3. Modern IOS does SHA via key chains (MD5 is legacy but still
examable):

```
! r2 and r3
key chain OSPF-KEYS
 key 1
  key-string CCNPospf
  cryptographic-algorithm hmac-sha-256
!
interface Ethernet0/2      ! e0/1 on r3
 ip ospf authentication key-chain OSPF-KEYS
```

Configure one side first and watch the adjacency die (`debug ip ospf adj` shows the mismatch),
then the other side — it returns to FULL. Verify: `show ip ospf interface e0/2 | include Crypto`.

## Task 10 — read the LSDB like the exam does

```
r2# show ip ospf database
```

Map every section to its LSA type and answer:

| LSA | Name | Who originates | Where seen in this lab |
| --- | --- | --- | --- |
| 1 | Router | every router, per area | everywhere |
| 2 | Network | DR of a broadcast segment | only segments still BROADCAST |
| 3 | Summary | ABRs (r2, r3) | inter-area routes, the /22 summary |
| 4 | ASBR-summary | ABRs | how r1 (pre-stub) finds r3-the-ASBR |
| 5 | AS-external | ASBR (r3) | 203.0.113.0/24, the default |
| 7 | NSSA-external | NSSA ASBR (r4) | 192.168.44.0/24 inside area 2 |

## Challenges

1. First revert area 1 to a normal area (`no area 1 stub` on r1 **and** r2 — totally stubby
   already filters every Type-3, leaving nothing to demonstrate). Then: filter the
   172.31.4.1/32 route from entering area 1 using `area 1 range ... not-advertise` — wrong
   tool? Prove it (hint: `area X range` matches prefixes *from* area X), then do it correctly
   with a prefix-list + `area filter-list`.
2. Convert the r2↔r3 link to hello/dead 1/4 (fast convergence, pre-BFD style) without dropping
   the adjacency for more than a few seconds.
3. Predict, then verify: what happens to r4's default route if you change area 2 to
   `nssa no-summary` on r3 (totally NSSA)? What LSA delivers it now?
4. Break FULL on purpose three ways (MTU 1400 one side; mismatched hello; wrong area) and match
   each to its symptom in `debug ip ospf adj` / `show ip ospf neighbor`.
5. Add a second link between r2 and r3 (edit the topology!) and load-share, then force all
   traffic onto one link with cost while keeping the other as pure backup.

<details><summary>Solution reference</summary>

Final configs in [`solutions/`](solutions/); `./deploy.sh reset 2 && ./deploy.sh deploy 2 --solved`
boots the end state (tasks 1–9).
</details>

**Next:** [Lab 03 — EIGRP named mode](../lab03-eigrp/README.md)
