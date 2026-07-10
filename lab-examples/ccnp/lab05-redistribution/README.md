# Lab 05 — Redistribution & path control

**Goal:** the ENARSI centerpiece. Merge an EIGRP domain and an OSPF domain at **two** border
routers, create the classic loop/suboptimal-routing problem on purpose, then fix it like a
professional: route tags, administrative-distance tuning, prefix filtering. Finish with
policy-based routing and an IP SLA-tracked floating default.

| | |
| --- | --- |
| Blueprint mapping | **ENARSI 1.1** (administrative distance), **1.2** (route maps), **1.3** (loop prevention: filtering/tagging), **1.4** (redistribution), **1.5** (summarization), **ENARSI 4.9/4.10 & ENCOR 4.5** (IP SLA + object tracking), PBR |
| Nodes / RAM | 4× IOL / ~3 GB |
| Estimated time | 3–4 h |

## Topology

```
        EIGRP 100                            OSPF 1
                       +------ r2 ------+
                  e0/1 | 10.0.12  10.0.24 | e0/1
        r1 ------------+                  +------------ r4
        |         e0/2 | 10.0.13  10.0.34 | e0/2        |
        |              +------ r3 ------+               |
   Lo1 172.16.1.0/24                          Lo1 192.168.41.0/24
   Lo2 172.16.2.0/24                          Lo2 192.168.42.0/24
                                              Lo100 198.51.100.0/24  <- in NO IGP
                                                    ("internet", task 8)
```

Baseline: both IGPs already run **inside their own domain** (r2/r3 speak EIGRP towards r1 and
OSPF towards r4 — no redistribution). r2/r3 carry static defaults towards r4.

```bash
./deploy.sh deploy lab05 && ./deploy.sh ssh 5 r2
```

## Task 1 — map the starting state

On r2: `show ip route` — EIGRP routes (D) from r1's side, OSPF routes (O) from r4's side,
coexisting because they serve different prefixes. On r1: `show ip route` — no 192.168.x routes.
Ping 192.168.41.1 from r1 — fails. Recite the AD table you'll need today:

| Source | AD |
| --- | --- |
| Connected / Static | 0 / 1 |
| EIGRP internal | **90** |
| OSPF (all types) | **110** |
| EIGRP **external** | **170** |

## Task 2 — mutual redistribution at r2

```
! r2
router ospf 1
 redistribute eigrp 100 subnets
router eigrp 100
 redistribute ospf 1 metric 10000 100 255 1 1500
```

Two things the exam checks every time:

- `subnets` — without it, classful-only networks make it into OSPF (IOS-XE now adds it
  implicitly, but state it).
- EIGRP has **no default seed metric** (∞ = unreachable): `metric <bw> <delay> <rel> <load> <mtu>`
  (or `default-metric`) is mandatory, else nothing is redistributed. OSPF's default seed is 20
  (E2).

Verify:

```
r1# show ip route eigrp | include EX
D EX  192.168.41.0/24 [170/...] via 10.0.12.2 ...
r4# show ip route ospf
O E2  172.16.1.0/24 [110/20] via 10.0.24.2 ...
```

`D EX` (AD 170) and `O E2` (metric 20, non-incrementing) — know both signatures. End-to-end ping
r1 Lo1 → 192.168.41.1 now works, but everything hairpins through r2 — single point of failure.

## Task 3 — second redistribution point = trouble

Repeat the same two `redistribute` commands on **r3**. Now inject an *external* into EIGRP to
expose the trap:

```
! r1
ip route 10.99.99.0 255.255.255.0 Null0
!
ip prefix-list PL-STATIC-EXT seq 5 permit 10.99.99.0/24
route-map RM-STATIC-TO-EIGRP permit 10
 match ip address prefix-list PL-STATIC-EXT
!
router eigrp 100
 redistribute static metric 10000 100 255 1 1500 route-map RM-STATIC-TO-EIGRP
```

(Habit to build now: **never redistribute unscoped**. Task 8 adds more statics to r1 — a naked
`redistribute static` would leak your default routes into EIGRP and bounce default-bound
traffic between the borders and r4.)

Watch r2 (give it ~30 s):

```
r2# show ip route 10.99.99.0
Routing entry for 10.99.99.0/24
  Known via "ospf 1", distance 110, metric 20, type extern 2
  ... via 10.0.24.4
```

Read that carefully: r2 reaches a prefix that lives **one EIGRP hop away** (via r1, AD 170) by
going **through r4 and the OSPF domain** (AD 110 beats 170). The path is r1→r3→OSPF→r4→r2 —
r3 redistributed it into OSPF, and r2 believed OSPF over EIGRP-external. This is the
suboptimal-routing/feedback pattern; with unlucky timing the two borders can re-redistribute
each other's copies into a **routing loop**.

## Task 4 — loop prevention with route tags

Rule: *never re-advertise a route back into the protocol it came from.* Tags make provenance
visible. On **both r2 and r3**:

```
route-map RM-EIGRP-TO-OSPF deny 5
 match tag 110
route-map RM-EIGRP-TO-OSPF permit 10
 set tag 90
!
route-map RM-OSPF-TO-EIGRP deny 5
 match tag 90
route-map RM-OSPF-TO-EIGRP permit 10
 set tag 110
!
router ospf 1
 redistribute eigrp 100 subnets route-map RM-EIGRP-TO-OSPF
router eigrp 100
 redistribute ospf 1 metric 10000 100 255 1 1500 route-map RM-OSPF-TO-EIGRP
```

(Convention: tag = AD of the protocol the route was *born* in.) Verify tags travel:
`show ip route 172.16.1.0` on r4 shows `Route tag 90`; `show ip eigrp topology 192.168.41.0/24`
on r1 shows tag 110. Routes now cross the border exactly once, in each direction.

## Task 5 — fix the suboptimal path with AD tuning

Tags stopped re-injection, but r2 *still* prefers the OSPF copy of 10.99.99.0/24 (110 < 170).
Make **external** OSPF routes less believable than EIGRP externals — on r2 and r3:

```
router ospf 1
 distance ospf external 180
```

```
r2# show ip route 10.99.99.0
  Known via "eigrp 100", distance 170 ... via 10.0.12.1
```

Direct path restored. `distance ospf external` changes AD only for E1/E2 (intra/inter untouched)
— surgical, and the exact knob ENARSI 1.1 is about. Internal routes were never at risk — why?
(EIGRP internal 90 already beats OSPF 110.)

## Task 6 — filter at the border

Policy: site LAN B (`172.16.2.0/24`) must stay private to the EIGRP domain. Extend the existing
route-map on **both borders** with a deny clause *before* the permit:

```
ip prefix-list PL-NO-LAN-B seq 5 permit 172.16.2.0/24
route-map RM-EIGRP-TO-OSPF deny 4
 match ip address prefix-list PL-NO-LAN-B
```

Verify: gone from r4 (`show ip route 172.16.2.0` → not found), still fine on r1/r2/r3.
Alternative tools to know: `distribute-list prefix ... out` under the OSPF process, or an
`area filter-list` at an ABR (inter-area only).

## Task 7 — policy-based routing (PBR)

Requirement: traffic **from LAN A (172.16.1.0/24) to the DC** must use the r3 path (compliance
says so), everything else follows the routing table. On r1:

```
ip access-list extended ACL-LAN-A-TO-DC
 permit ip 172.16.1.0 0.0.0.255 192.168.0.0 0.0.255.255
!
route-map RM-PBR permit 10
 match ip address ACL-LAN-A-TO-DC
 set ip next-hop 10.0.13.3
route-map RM-PBR permit 20
!
ip local policy route-map RM-PBR
```

We use `ip local policy` because the test traffic is *generated by r1 itself* (loopback-sourced
pings). For transit traffic you'd apply `ip policy route-map RM-PBR` on the **ingress
interface** — remember: PBR hooks packets *entering* an interface, before the routing table.
The empty `permit 20` clause lets non-matching traffic route normally instead of being dropped.

Verify (compare sources):

```
r1# traceroute 192.168.41.1 source Loopback1    ! via 10.0.13.3 (policy)
r1# traceroute 192.168.41.1 source Loopback2    ! via routing table
r1# show route-map RM-PBR                        ! match counters increment
```

## Task 8 — IP SLA + tracking + floating static

`198.51.100.1` (r4 Lo100) is in **no IGP** — it simulates the internet, reachable only through
the borders' defaults. Give r1 a primary default via r2 that self-heals to r3:

```
! r1
ip sla 1
 icmp-echo 10.0.24.4 source-interface Ethernet0/1
 frequency 5
ip sla schedule 1 life forever start-time now
!
track 1 ip sla 1 reachability
 delay down 5 up 10
!
ip route 0.0.0.0 0.0.0.0 10.0.12.2 track 1     ! primary, valid only while track 1 is Up
ip route 0.0.0.0 0.0.0.0 10.0.13.3 250          ! floating backup (AD 250)
```

Why probe `10.0.24.4` (r4's address on the r2–r4 link) and not the final destination? Because
the probe must test the **primary path specifically** — if it probed 198.51.100.1, the probe
itself would start succeeding over the backup path after failover and flap the track. (`delay
down 5 up 10` adds dampening.)

Test the failover:

```
r1# ping 198.51.100.1                            ! works via r2
r2(config)# interface Ethernet0/2
r2(config-if)# shutdown                          ! kill the r2-r4 link
r1# show track 1                                 ! Reachability Up -> Down
r1# show ip route 0.0.0.0                        ! now via 10.0.13.3
r1# ping 198.51.100.1                            ! still works - via r3
```

`no shutdown` on r2 and watch it recover. Also inspect `show ip sla statistics 1`.

## Challenges

1. Instead of `distance ospf external 180`, solve task 5's problem with `distance 171 10.0.24.4
   0.0.0.0 <ACL>` — per-source AD manipulation. Which approach scales better and why?
2. Summarize `192.168.41.0/24` + `192.168.42.0/24` into one advertisement towards EIGRP
   (`summary-address` where? — think: which router, which interface, which protocol allows
   summarization at redistribution).
3. Replace the prefix-list filter with a `distribute-list route-map` under EIGRP that filters
   *inbound* on r1 instead — compare the failure domains of border-side vs receiver-side
   filtering.
4. Convert the SLA probe to track a **route** instead (`track 2 ip route 10.0.24.0/24
   reachability`) and discuss when route-tracking beats probe-tracking.
5. Break task 4 on purpose: remove the deny clauses on r3 only, add a second external on r1, and
   try to catch the transient loop with `debug ip routing` — then explain why loops here are
   timing-dependent.

<details><summary>Solution reference</summary>

Final configs in [`solutions/`](solutions/); deploy with `./deploy.sh deploy 5 --solved`.
</details>

**Next:** [Lab 06 — IP services](../lab06-services/README.md)
