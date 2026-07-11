# Lab 03 — EIGRP named mode: DUAL, variance, summarization, stub

**Goal:** run EIGRP the modern way (named mode / wide metrics), then use this square topology to
*see* DUAL at work: successors vs feasible successors, unequal-cost load balancing with
`variance`, summarization with its Null0 discard route, why a stub router refuses to be transit,
and SHA authentication.

| | |
| --- | --- |
| Blueprint mapping | **ENARSI 1.9** (EIGRP classic vs named, neighbors, DUAL/stuck-in-active, equal & unequal cost load balancing, metrics, stubs, summarization, authentication), **ENCOR 3.2.a** (EIGRP vs OSPF comparison) |
| Nodes / RAM | 4× IOL / ~3 GB |
| Estimated time | 2.5–3.5 h |

## Topology

```
             10.0.12.0/24        10.0.24.0/24
        +------- r2 ---------+
   e0/1 |                    | e0/1
        r1                   r4 --- Lo1-4: 172.16.4.0/24 .. 172.16.7.0/24
   e0/2 |                    | e0/2         ("DC LANs" -> /22 summary)
        +------- r3 ---------+
          10.0.13.0/24        10.0.34.0/24
          (delay 2000 usec on the r1-r3 link = simulated slow WAN)
```

The r1↔r3 link ships with `delay 200` (= 2000 µs — IOS stores delay in **tens of µs**, an exam
favorite) so the two r1→r4 paths have *different* metrics but still satisfy the feasibility
condition. Router IDs `10.255.255.N`. No EIGRP is configured in the baseline.

```bash
./deploy.sh deploy lab03 && ./deploy.sh ssh 3 r1
```

## Task 1 — named-mode EIGRP on all routers

One process, AS 100, virtual instance name `CCNP`. On **r1** (adapt networks for the others):

```
router eigrp CCNP
 address-family ipv4 unicast autonomous-system 100
  eigrp router-id 10.255.255.1
  network 10.0.12.0 0.0.0.255
  network 10.0.13.0 0.0.0.255
  network 10.255.255.1 0.0.0.0
 exit-address-family
```

- r2: networks `10.0.12.0/24`, `10.0.24.0/24`, Lo0
- r3: networks `10.0.13.0/24`, `10.0.34.0/24`, Lo0
- r4: networks `10.0.24.0/24`, `10.0.34.0/24`, Lo0, `network 172.16.4.0 0.0.3.255`

Then suppress hellos on loopbacks with the named-mode pattern (all routers):

```
router eigrp CCNP
 address-family ipv4 unicast autonomous-system 100
  af-interface default
   passive-interface
  exit-af-interface
  af-interface Ethernet0/1
   no passive-interface
  exit-af-interface
  af-interface Ethernet0/2
   no passive-interface
  exit-af-interface
```

(r2/r3/r4 have two Ethernet af-interfaces too; r1's are e0/1 and e0/2.)

Verify:

```
r1# show ip eigrp neighbors
EIGRP-IPv4 VR(CCNP) Address-Family Neighbors for AS(100)
H   Address                 Interface              Hold Uptime   SRTT   RTO  Q  Seq
1   10.0.13.3               Et0/2                    13 00:02:11   ...
0   10.0.12.2               Et0/1                    11 00:03:04   ...

r1# show ip protocols | section eigrp
  ...
  EIGRP-IPv4 VR(CCNP) Address-Family Protocol for AS(100)
    Metric weight K1=1, K2=0, K3=1, K4=0, K5=0 K6=0
    Metric version 64bit
```

`Metric version 64bit` = **wide metrics** — only named mode uses them. K-values must match or no
adjacency forms (`K-value mismatch` in `%DUAL-5-NBRCHANGE`).

## Task 2 — read DUAL: successor vs feasible successor

r1 has two paths to r4's DC LAN `172.16.4.0/24`:

```
r1# show ip eigrp topology 172.16.4.0/24
EIGRP-IPv4 VR(CCNP) Topology Entry for AS(100)/ID(10.255.255.1) for 172.16.4.0/24
  State is Passive, Query origin flag is 1, 1 Successor(s), FD is 111411200
  ...
        via 10.0.12.2 (111411200/104857600), Ethernet0/1     <- successor
        via 10.0.13.3 (117964800/104857600), Ethernet0/2     <- feasible successor
```

Decode it like the exam demands:

- **FD (feasible distance)** = best total metric = 111411200 via r2.
- Each entry shows `(full metric / reported distance)`. r3's **RD** is 104857600.
- **Feasibility condition:** RD < FD → 104857600 < 111411200 ✓, so the r3 path is a **feasible
  successor** — a pre-approved loop-free backup that DUAL can install *instantly*, no queries.
- `State is Passive` is *good* (route stable). *Active* means DUAL is querying neighbors —
  prolonged Active = **SIA (stuck-in-active)**, examined together with `show ip eigrp topology
  active`.

Only the successor is in the RIB for now:

```
r1# show ip route 172.16.4.0
  ... [90/870400] via 10.0.12.2 ...        ! 111411200 / 128 - RIB scales wide metrics
```

Fail the fast path (`shutdown` r1 e0/1), re-check — the FS takes over in milliseconds, no Active
phase. `no shutdown` before continuing.

## Task 3 — unequal-cost load balancing (variance)

The r3 path costs 117964800 ≈ 1.06 × FD. Any multiplier > 1.06 admits it:

```
r1(config)# router eigrp CCNP
r1(config-router)# address-family ipv4 unicast autonomous-system 100
r1(config-router-af)# topology base
r1(config-router-af-topology)# variance 2
```

```
r1# show ip route 172.16.4.0
Routing entry for 172.16.4.0/24
  Known via "eigrp 100" ... 
  Routing Descriptor Blocks:
  * 10.0.13.3, from 10.0.13.3, 00:00:12 ago, via Ethernet0/2
      Route metric is 117964800, traffic share count is 17
    10.0.12.2, from 10.0.12.2, 00:00:12 ago, via Ethernet0/1
      Route metric is 111411200, traffic share count is 18
```

Traffic is shared **inversely proportionally to metric** — the *better* path carries the
*larger* share (18 on the r2 path vs 17 on the slower r3 path; note 870400×18 = 921600×17), not
50/50. Rules to memorize:
variance only admits paths that (a) meet the feasibility condition and (b) have metric <
variance × FD. It can never use a non-FS path.

## Task 4 — summarization and the discard route

Summarize r4's four DC LANs outbound on **both** of r4's uplinks:

```
! r4
router eigrp CCNP
 address-family ipv4 unicast autonomous-system 100
  af-interface Ethernet0/1
   summary-address 172.16.4.0 255.255.252.0
  exit-af-interface
  af-interface Ethernet0/2
   summary-address 172.16.4.0 255.255.252.0
  exit-af-interface
```

Verify:

- r1/r2/r3 now see a single `D 172.16.4.0/22` (the four /24s are gone).
- r4 installed `D 172.16.4.0/22 ... Null0` (AD 5) — the **discard route** that prevents loops
  for unallocated space inside the summary.
- EIGRP advertises the summary with the **lowest** component metric.

## Task 5 — authentication (named mode = easy SHA)

Secure the r1↔r2 adjacency:

```
! r1 and r2
router eigrp CCNP
 address-family ipv4 unicast autonomous-system 100
  af-interface Ethernet0/1
   authentication mode hmac-sha-256 0 CCNPeigrp
```

Configure r1 first: the adjacency drops (`%DUAL-5-NBRCHANGE ... holding time expired` /
authentication failure in `debug eigrp packets`). After r2 matches, it returns. Classic mode
would have required a key chain + `authentication key-chain`; named mode's inline SHA password
is the modern pattern (key chains still work for rotation).

## Task 6 — stub: why spokes shouldn't be transit

Pretend r3 is a small branch. Make it a stub:

```
! r3
router eigrp CCNP
 address-family ipv4 unicast autonomous-system 100
  eigrp stub connected summary
```

Now check r1:

```
r1# show ip eigrp neighbors detail
...
1   10.0.13.3   Et0/2  ...
   Stub Peer Advertising (CONNECTED SUMMARY) Routes
   Suppressing queries
```

Two consequences to verify:

1. `show ip eigrp topology 172.16.4.0/22` on r1 (the **/22** — task 4's summary suppressed the
   /24s domain-wide, so that's the prefix r1 actually holds) — the **via r3 entry is gone** (a
   stub doesn't advertise routes it *learned*, and r3 only ever learned the /22 from r4), so
   your variance path silently disappeared. Design lesson: stub belongs on true spokes, never
   on transit routers.
2. `Suppressing queries` — r1 won't query r3 during DUAL computation, which is the whole point:
   stubs bound the query domain and prevent SIA in hub-and-spoke networks.

Remove it (`no eigrp stub`) to restore the FS/variance state.

## Task 7 — classic vs named (know both for ENARSI)

Nothing to configure — translate. The equivalent *classic* config for r1 would be:

```
key chain EIGRP-KEYS
 key 1
  key-string CCNPeigrp
router eigrp 100
 eigrp router-id 10.255.255.1
 network 10.0.12.0 0.0.0.255
 network 10.0.13.0 0.0.0.255
 network 10.255.255.1 0.0.0.0
 passive-interface default
 no passive-interface Ethernet0/1
 no passive-interface Ethernet0/2
 variance 2
interface Ethernet0/1
 ip authentication mode eigrp 100 md5
 ip authentication key-chain eigrp 100 EIGRP-KEYS
```

Differences that get tested: named mode = one place for everything (af-interface vs interface
commands), wide/64-bit metrics, SHA support, multiple address families under one instance;
classic = interface-scattered config, 32-bit metrics, MD5 only.

## Challenges

1. Set `metric weights 0 1 0 1 0 0 1` (enable K6? try it) on r1 only and explain what happens
   and why K-values are adjacency-forming parameters.
2. Use an **offset-list** on r2 to make the r3 path win for `172.16.5.0/24` only, without
   touching delay/bandwidth.
3. Add a `leak-map` to r4's summary so `172.16.6.0/24` is advertised *alongside* the /22, and
   explain a real-world reason to do this (optimal exit for one critical subnet).
4. Tune hello/hold to 1/3 s on the r1–r2 link, then compare failover speed against the FS
   failover you saw in task 2. What does this buy you that variance doesn't?
5. Predict before testing: with r3 as stub *and* the r1–r2 link down, can r1 still reach
   172.16.4.0/24? Verify, and explain how `eigrp stub connected summary leak-map` could fix it.

<details><summary>Solution reference</summary>

Final configs in [`solutions/`](solutions/) (stub left off, per task 6's ending state);
`./deploy.sh reset 3 && ./deploy.sh deploy 3 --solved` boots the end state.
</details>

**Next:** [Lab 04 — BGP peering & policy](../lab04-bgp/README.md)
