# Lab 07 — GRE, DMVPN (phase 1 → 3) & IPsec

**Goal:** build overlay VPNs across an "internet" that knows nothing about your LANs: first a
point-to-point GRE tunnel, then a proper DMVPN — hub-and-spoke mGRE + NHRP with EIGRP on top —
evolved from phase 1 to phase 3 (dynamic spoke-to-spoke tunnels), and finally encrypted with
IPsec. You'll capture packets on the "internet" router and watch GRE turn into ESP.

| | |
| --- | --- |
| Blueprint mapping | **ENCOR 2.1.c** (GRE/IPsec data-path virtualization), **ENARSI 2.3** (DMVPN: GRE/mGRE, NHRP, IPsec, dynamic neighbors, spoke-to-spoke), 2.4 (IPsec concepts) |
| Nodes / RAM | 4× IOL / ~3 GB |
| Estimated time | 3–4 h |

## Topology

```
                    HQ r1  Lo1 10.0.1.0/24
                     | e0/1 100.64.1.1/30
                     |
                    r4  = "internet" (knows ONLY the /30s - hands off)
                    / \
   100.64.2.1/30  /   \  100.64.3.1/30
                 /     \
     branch r2         r3 branch
  Lo1 10.0.2.0/24    Lo1 10.0.3.0/24

  Overlay: Tunnel100 = 172.16.100.0/24  (hub .1, r2 .2, r3 .3)
```

Baseline: WAN addressing + default routes to r4. Try `ping 10.0.2.1` from r1 — it fails, and
*that's the product requirement*: r4 (like any ISP) doesn't route your private space.

```bash
./deploy.sh deploy lab07 && ./deploy.sh ssh 7 r1
```

## Part A — point-to-point GRE (the warm-up)

### Task A1 — tunnel r1↔r2

```
! r1                                        ! r2
interface Tunnel12                          interface Tunnel12
 ip address 172.16.12.1 255.255.255.252      ip address 172.16.12.2 255.255.255.252
 tunnel source Ethernet0/1                   tunnel source Ethernet0/1
 tunnel destination 100.64.2.1               tunnel destination 100.64.1.1
```

The tunnel is up when *both* the source interface is up and the destination is routable
(via the default). Verify + route LANs over it:

```
r1# show interface Tunnel12 | include Tunnel protocol
  Tunnel protocol/transport GRE/IP
r1(config)# ip route 10.0.2.0 255.255.255.0 172.16.12.2
r2(config)# ip route 10.0.1.0 255.255.255.0 172.16.12.1
r1# ping 10.0.2.1 source Loopback1      ! !!!!!
```

### Task A2 — see the encapsulation

On the host, capture the r1–r4 wire while pinging:

```bash
sudo ip netns exec clab-ccnp-lab07-r4 tcpdump -nni eth1 proto gre
# 100.64.1.1 > 100.64.2.1: GREv0 ... IP 10.0.1.1 > 10.0.2.1: ICMP echo request
```

Outer header 100.64.x (routable), inner 10.0.x (private) — data-path virtualization in one line
of tcpdump. Know the numbers: GRE = IP protocol **47**, adds **24 bytes** (20 IP + 4 GRE), hence
the classic `ip mtu 1400` + `ip tcp adjust-mss 1360` you'll set on the DMVPN tunnels.

Also know the failure mode: if the tunnel *destination* ever becomes reachable **through the
tunnel itself** (e.g. you advertise 100.64.0.0 into the overlay IGP), IOS logs
`%TUN-5-RECURDOWN` and flaps the tunnel — recursive routing.

### Task A3 — tear it down

p2p GRE needs n·(n-1)/2 tunnels + static config for a full mesh — unmanageable at 50 branches.
Delete before Part B: `no interface Tunnel12` (both) + remove both static routes.

## Part B — DMVPN phase 1 (hub-and-spoke)

One mGRE interface on the hub; spokes register themselves via **NHRP**. Hub (r1):

```
interface Tunnel100
 bandwidth 10000
 ip address 172.16.100.1 255.255.255.0
 no ip redirects
 ip mtu 1400
 ip tcp adjust-mss 1360
 ip nhrp authentication CCNP7
 ip nhrp map multicast dynamic
 ip nhrp network-id 100
 tunnel source Ethernet0/1
 tunnel mode gre multipoint
 tunnel key 100
```

Spokes (r2 shown; r3 = .3): classic **phase 1** = plain p2p GRE towards the hub + NHRP
registration:

```
interface Tunnel100
 bandwidth 10000
 ip address 172.16.100.2 255.255.255.0
 ip mtu 1400
 ip tcp adjust-mss 1360
 ip nhrp authentication CCNP7
 ip nhrp map 172.16.100.1 100.64.1.1
 ip nhrp map multicast 100.64.1.1
 ip nhrp network-id 100
 ip nhrp nhs 172.16.100.1
 tunnel source Ethernet0/1
 tunnel destination 100.64.1.1
 tunnel key 100
```

Decode each piece (exam!): `map` = static NBMA↔overlay entry for the hub; `map multicast` = who
gets routing-protocol multicasts; `nhs` = register with this next-hop server; `network-id` and
`tunnel key` must match domain-wide. Verify on r1:

```
r1# show dmvpn
...
 # Ent  Peer NBMA Addr Peer Tunnel Add State  UpDn Tm Attrb
     1 100.64.2.1          172.16.100.2    UP 00:01:30     D
     1 100.64.3.1          172.16.100.3    UP 00:00:41     D
```

`D` = dynamically learned (the spokes registered). `show ip nhrp` shows the actual mappings.

### Task B2 — EIGRP over the overlay

All three: `router eigrp 100` + `network 10.0.0.0` + `network 172.16.100.0 0.0.0.255` +
`passive-interface default` + `no passive-interface Tunnel100` + `eigrp router-id 10.255.255.N`.

Check r2's table… **r3's LAN is missing.** The hub learned it, but won't re-advertise it out
the same interface it came in — split horizon. Fix on the hub only:

```
! r1
interface Tunnel100
 no ip split-horizon eigrp 100
```

Now r2 sees `D 10.0.3.0/24 via 172.16.100.1` — *via the hub*, because phase-1 spokes have p2p
tunnels: `traceroute 10.0.3.1 source Loopback1` from r2 = two overlay hops (hub, then r3).

## Part C — phase 3 (dynamic spoke-to-spoke)

Spoke↔spoke traffic hairpinning through the hub wastes its bandwidth. Phase 3 lets the hub
*redirect* spokes to each other:

```
! r1 (hub)
interface Tunnel100
 ip nhrp redirect

! r2 and r3 (spokes)
interface Tunnel100
 no tunnel destination         ! p2p -> mGRE
 tunnel mode gre multipoint
 ip nhrp shortcut
```

Trigger and observe the magic:

```
r2# traceroute 10.0.3.1 source Loopback1   ! 1st packets: via 172.16.100.1 (hub)
r2# traceroute 10.0.3.1 source Loopback1   ! moments later: 172.16.100.3 DIRECT
r2# show dmvpn
     1 100.64.1.1          172.16.100.1    UP ... S      <- static NHS
     1 100.64.3.1          172.16.100.3    UP ... DT2    <- dynamic spoke-spoke!
r2# show ip nhrp shortcut
r2# show ip route next-hop-override         ! the NHRP '%' override routes
```

Sequence worth narrating on the exam: first packet → hub forwards *and* sends an NHRP
**redirect** → spokes resolve each other's NBMA addresses → dynamic tunnel (`DT2`) → NHRP
installs next-hop overrides. Phase 2 vs 3 (favorite question): phase 2 needed spokes to keep
each other's *specific* routes with unchanged next hops (no summarization allowed); phase 3
redirects/shortcuts work **even with summaries** — the hub could advertise just 10.0.0.0/8.

## Part D — protect it with IPsec

Everything so far is cleartext GRE on the "internet". Same config on **all three** DMVPN
routers:

```
crypto isakmp policy 10
 encryption aes 256
 hash sha256
 authentication pre-share
 group 14
crypto isakmp key CCNP-DMVPN address 0.0.0.0
!
crypto ipsec transform-set TS-DMVPN esp-aes 256 esp-sha256-hmac
 mode transport
!
crypto ipsec profile PROF-DMVPN
 set transform-set TS-DMVPN
!
interface Tunnel100
 tunnel protection ipsec profile PROF-DMVPN
```

(Wildcard PSK `address 0.0.0.0` fits DMVPN's any-to-any reality; production would use
certificates. `mode transport` saves 20 bytes since GRE already provides the outer IP.)

Verify layer by layer:

```
r1# show crypto isakmp sa                       ! QM_IDLE = IKE phase 1 up
r1# show crypto ipsec sa | include pkts         ! encaps/decaps counting up
r1# show dmvpn detail                            ! per-peer crypto session
```

And re-run the Part A2 capture on r4 while pinging spoke-to-spoke:

```bash
sudo ip netns exec clab-ccnp-lab07-r4 tcpdump -nni eth2 'ip proto 50'
# 100.64.2.1 > 100.64.3.1: ESP(spi=...,seq=...)
```

Protocol 50 (ESP), payload unreadable — mission accomplished.

## Challenges

1. Kill r3's tunnel (`shutdown Tunnel100`), clear NHRP on r2 (`clear ip nhrp`), and time how
   long r2's DT2 entry and override route take to disappear. Which NHRP timers control this
   (`show ip nhrp detail`)?
2. Summarize on the hub: `ip summary-address eigrp 100 10.0.0.0 255.0.0.0` on Tunnel100.
   Verify spokes now hold ONE route yet still build direct spoke-spoke tunnels — the phase-3
   party trick phase 2 couldn't do.
3. Add a second NHS for redundancy: build Tunnel200 with r2 as a second "hub" (backup DMVPN
   cloud) and make r3 prefer Tunnel100 via EIGRP metrics.
4. Replace ISAKMP/IKEv1 with an **IKEv2** profile (keyring, proposal, profile) — the
   modern-config variant ENARSI increasingly references.
5. Explain (then prove with `debug nhrp packet`) why spokes behind **NAT** complicate DMVPN,
   and which NHRP extension (`NAT-T`, claimed NBMA) handles it.

<details><summary>Solution reference</summary>

Final configs (phase 3 + IPsec) in [`solutions/`](solutions/);
`./deploy.sh deploy 7 --solved` boots the end state.
</details>

**Next:** [Lab 08 — MPLS L3VPN](../lab08-mpls/README.md)
