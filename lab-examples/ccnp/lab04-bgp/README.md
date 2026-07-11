# Lab 04 — BGP peering & policy

**Goal:** dual-home an enterprise AS to two ISPs: bring up eBGP and loopback-based iBGP over an
OSPF underlay, watch iBGP split-horizon break things and fix it with a route reflector, then
steer traffic with the path attributes the exams obsess over — weight, local preference, AS-path
prepending, MED — and advertise a clean aggregate to the world.

| | |
| --- | --- |
| Blueprint mapping | **ENCOR 3.2.c** (eBGP: peering, path selection), **ENARSI 1.11** (iBGP/eBGP neighbors, path selection & attributes, route reflectors, policies) |
| Nodes / RAM | 5× IOL / ~3.8 GB |
| Estimated time | 3–4 h |

## Topology

```
                     AS 65001 - your job
                            r1  (10.255.255.1, Lo1 10.1.1.0/24)
                     e0/1 /    \ e0/2
                         /      \
      (10.255.255.2) r2 -------- r3 (10.255.255.3)
      Lo1 10.1.2.0/24 |   e0/2   | Lo1 10.1.3.0/24
                 e0/3 |          | e0/3
   ~~~~~~~~~~~~~~~~~~ | ~~~~~~~~ | ~~~~~~~~~~~~~~~~~~~~
      100.64.24.0/30  |          |  100.64.35.0/30
                      r4 ------- r5
        AS 65100 (ISP-A)          AS 65200 (ISP-B)
        198.51.100.0/24           203.0.113.0/24
              both advertise 100.100.100.0/24 ("the internet")
```

Pre-provisioned: all addressing, the OSPF underlay inside AS 65001 (links + Lo0s only — **not**
the 10.1.x.0/24 Lo1 prefixes), and both ISP routers (fully configured, hands off — their configs
are in `configs/r4|r5.partial.cfg` if you're curious what the "provider side" looks like).

```bash
./deploy.sh deploy lab04 && ./deploy.sh ssh 4 r2
```

Confirm the underlay first: `show ip route ospf` on r1 must show all three Lo0s.

## Task 1 — eBGP to the ISPs

On the edges:

```
! r2
router bgp 65001
 bgp router-id 10.255.255.2
 bgp log-neighbor-changes
 neighbor 100.64.24.2 remote-as 65100
 network 10.1.2.0 mask 255.255.255.0

! r3
router bgp 65001
 bgp router-id 10.255.255.3
 neighbor 100.64.35.2 remote-as 65200
 network 10.1.3.0 mask 255.255.255.0
```

```
r2# show ip bgp summary
Neighbor        V    AS MsgRcvd MsgSent   TblVer  InQ OutQ Up/Down  State/PfxRcd
100.64.24.2     4 65100      ...                            00:01:12        3
```

`State/PfxRcd` showing a *number* = session Established. (Idle/Active/OpenSent states and their
causes — wrong IP, TTL, no route — are classic troubleshooting questions.) `show ip bgp` on r2:
`198.51.100.0/24`, `203.0.113.0/24` (path `65100 65200`), `100.100.100.0/24` — note `network`
requires an **exact** RIB match, which is why `10.1.2.0 mask 255.255.255.0` matters.

## Task 2 — iBGP full mesh on loopbacks

iBGP sessions ride the OSPF underlay between Lo0s so they survive any single link failure. On
**r1, r2, r3**, peer with the other two, e.g. r1:

```
router bgp 65001
 bgp router-id 10.255.255.1
 neighbor 10.255.255.2 remote-as 65001
 neighbor 10.255.255.2 update-source Loopback0
 neighbor 10.255.255.3 remote-as 65001
 neighbor 10.255.255.3 update-source Loopback0
 network 10.1.1.0 mask 255.255.255.0
```

`update-source Loopback0` is mandatory both ways: iBGP TCP sessions must source/target the
loopbacks or the peer rejects the SYN. Why doesn't eBGP need this? (Directly connected /30.)

Now the classic failure — on r1:

```
r1# show ip bgp 100.100.100.0
  ...
  65100
    100.64.24.2 (inaccessible) from 10.255.255.2 ...
```

**Two problems, one symptom** (`show ip bgp` shows the route but RIB doesn't install it):
the next hop is the ISP's address, which is not in the IGP. Fix at the edges:

```
! r2 (and r3, towards their iBGP neighbors)
router bgp 65001
 neighbor 10.255.255.1 next-hop-self
 neighbor 10.255.255.3 next-hop-self
```

Re-check r1: next hop is now 10.255.255.2 — resolvable, and the route installs in the RIB
(`show ip route 100.100.100.0`). Don't expect `ping 100.100.100.1` to get replies — the ISPs
null-route that test prefix, so it answers with timeouts/unreachables by design; ping
198.51.100.1 and 203.0.113.1 for real end-to-end replies.

## Task 3 — break the mesh, fix with a route reflector

Remove the r2↔r3 session on both:

```
r2(config-router)# no neighbor 10.255.255.3
r3(config-router)# no neighbor 10.255.255.2
```

Check r2: ISP-B's `203.0.113.0/24` via r3 is **gone** (only the long path via ISP-A remains).
r3 told r1, but r1 refused to pass it on — **iBGP split horizon**: routes learned from an iBGP
peer are never re-advertised to another iBGP peer. Full mesh or reflectors, pick one. Make r1 a
route reflector:

```
! r1
router bgp 65001
 neighbor 10.255.255.2 route-reflector-client
 neighbor 10.255.255.3 route-reflector-client
```

r2 re-learns `203.0.113.0/24` with next hop 10.255.255.3 (RRs don't touch next hop), plus two
new attributes in `show ip bgp 203.0.113.0`: **Originator ID** (r3's RID) and **Cluster list**
(r1's) — the RR loop-prevention mechanisms ENARSI asks about.

## Task 4 — path selection: watch the algorithm work

Both ISPs advertise `100.100.100.0/24`, so AS 65001 has a real choice. On r1:

```
r1# show ip bgp 100.100.100.0
Paths: (2 available, best #2, table default)
  65200
    10.255.255.3 ... from 10.255.255.3 (10.255.255.3)
  65100
    10.255.255.2 ... from 10.255.255.2 (10.255.255.2)
      ... best
```

(`from A (B)`: A = the neighbor the update came from, B = that neighbor's router-ID — on a
reflected route B becomes the **Originator ID** instead; you'll see that on r2/r3.) With every
attribute tied (weight 0, LP 100, AS-path length 1, origin IGP, no MED comparison — different
neighbor AS, both paths internal, equal IGP metric to both next hops), selection falls all the
way through to the **lowest router-ID**. Memorize the full order — including the two steps
people forget:

> Weight → Local pref → Locally originated → AS-path → Origin → MED → eBGP over iBGP →
> **lowest IGP metric to the next hop** → oldest (eBGP only) → **lowest RID** → shortest
> cluster list → lowest neighbor address.

Now steer it deliberately, three ways:

1. **Weight** (Cisco-local, this router only) — on r2:
   `neighbor 100.64.24.2 weight 40000` + `clear ip bgp * soft in`. r2's best flips to ISP-A
   regardless of anything else. Remove it (`no neighbor ... weight`) — weight doesn't propagate,
   which is exactly why it's rarely the right tool.
2. **Local preference** (AS-wide, outbound traffic) — on r2:

    ```
    route-map RM-FROM-ISPA permit 10
     set local-preference 200
    router bgp 65001
     neighbor 100.64.24.2 route-map RM-FROM-ISPA in
    ```

    `clear ip bgp 100.64.24.2 soft in`. Every router in AS 65001 now exits via r2/ISP-A for all
    internet prefixes (LP 200 > 100, checked before AS-path). Verify on r3: its best path to
    100.100.100.0/24 goes *through the RR to r2*.

3. **AS-path prepend** (influences *inbound* traffic) — task 6.

## Task 5 — aggregate your address space

The internet shouldn't see three /24s. On **r2 and r3**:

```
router bgp 65001
 aggregate-address 10.1.0.0 255.255.0.0
!
ip prefix-list PL-AGG-ONLY seq 5 permit 10.1.0.0/16
router bgp 65001
 neighbor 100.64.24.2 prefix-list PL-AGG-ONLY out    ! r3: neighbor 100.64.35.2
```

`clear ip bgp * soft out`. Verify on r4: `show ip bgp neighbors 100.64.24.1 routes` — exactly
one prefix, `10.1.0.0/16`, flagged with `atomic-aggregate` and `aggregator` 65001. Notes:

- The aggregate needs ≥1 component *in the BGP table* (that's why the Lo1 `network` statements
  matter) and installs a `Null0` discard route locally.
- We filtered with an outbound prefix-list instead of `summary-only` — the components stay
  visible **inside** the AS (suppressing them here would blackhole edge-to-edge traffic via the
  aggregate's Null0). Bonus: the prefix-list also stops AS 65001 from ever announcing ISP-A's
  routes to ISP-B — you just prevented becoming accidental transit, a real-world (and exam)
  must.

## Task 6 — influence inbound: AS-path prepend

Inbound traffic is the *remote* AS's outbound decision — you can only make one path look worse.
Make the world prefer ISP-A by prepending towards ISP-B, on r3:

```
route-map RM-TO-ISPB permit 10
 set as-path prepend 65001 65001 65001
router bgp 65001
 neighbor 100.64.35.2 route-map RM-TO-ISPB out
```

`clear ip bgp 100.64.35.2 soft out`, then check **r5** (ISP-B): for `10.1.0.0/16` it now has
`65001 65001 65001 65001` (direct) vs `65100 65001` (via ISP-A) — and picks the ISP-A path.
Traceroute from r5 to 10.1.2.1 to see it land on r2. Also understand why **MED** couldn't do
this here: MED is only compared between routes from the *same* neighboring AS (we touch both
ISPs via different ASes). `bgp always-compare-med` exists but requires the remote side's
cooperation — see challenges.

## Task 7 — verification battery

```
show ip bgp summary                       ! sessions + prefixes received
show ip bgp                               ! table: >=best, i=internal, next hops
show ip bgp 100.100.100.0                 ! full attribute dump for one prefix
show ip bgp neighbors 10.255.255.2 advertised-routes
show ip bgp neighbors 10.255.255.2 routes
show ip bgp regexp _65200$                ! originated in AS 65200
clear ip bgp * soft                       ! policy refresh WITHOUT killing sessions
```

Explain (to your rubber duck) why `clear ip bgp *` hard reset is an outage and soft/route-refresh
isn't — a standard exam distinction.

## Challenges

1. r1 sets `bgp default local-preference 50` — predict the effect on exit selection before
   applying, verify, revert.
2. Write a route-map on r2 that sets LP 200 **only** for `203.0.113.0/24` (prefix-list match)
   and LP 100 for the rest — deliberate per-prefix traffic engineering.
3. Kill the r2–r4 link (`shutdown`) and measure the failover to ISP-B — it's near-instant.
   Why? (`bgp fast-external-fallover`, on by default for directly connected eBGP, tears the
   session on link-down.) Now disable it (`no bgp fast-external-fallover` on r2), repeat, and
   watch the hold timer (180 s) govern instead; only then do `neighbor ... timers 10 30` — and
   BFD (ENARSI 1.8) — have something to improve. Which real-world failures does
   fast-external-fallover *not* catch? (Anything that keeps the interface up: a switch in the
   path, unidirectional loss.)
4. Configure `maximum-prefix 10 warning-only` from ISP-A on r2, then think through what happens
   at 10 prefixes *without* warning-only — why do providers make you ask before raising it?
5. Add a static default on r2/r3 towards the ISPs plus `default-information originate` in OSPF —
   now non-BGP r1 traffic can reach the internet even before BGP; discuss when enterprises run
   default-only vs full-table edges.
6. (Topology edit) Add a second r3↔r4 link and use **MED** properly: two links to the *same*
   ISP, cold-potato vs hot-potato.

<details><summary>Solution reference</summary>

Final configs in [`solutions/`](solutions/) (RR topology, LP 200 on r2, prepend on r3,
aggregate + outbound filter on both edges); deploy with `./deploy.sh reset 4 && ./deploy.sh deploy 4 --solved`.
</details>

**Next:** [Lab 05 — Redistribution & path control](../lab05-redistribution/README.md)
