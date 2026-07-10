# Lab 00 — Containerlab foundation

**Goal:** learn the containerlab study workflow used by every lab in this curriculum — deploy,
connect, verify, capture, persist, destroy — using only **free images** (FRR routers + Linux
hosts). No Cisco software needed yet.

| | |
| --- | --- |
| Blueprint mapping | none (tooling prerequisite) — but the routing here is CCNA-level review |
| Nodes / RAM | 2× FRR, 2× Linux host / < 0.5 GB |
| Estimated time | 45–60 min |

## Topology

```
                 10.0.12.0/30
 pc1 --------- r1 ----------- r2 --------- pc2
     eth1 eth2 |  eth1   eth1 | eth2  eth1
 192.168.1.10  |              |        192.168.2.10
           .1  |              |  .1
        192.168.1.0/24     192.168.2.0/24
```

| Node | Interface | IPv4 | Notes |
| --- | --- | --- | --- |
| r1 | eth1 | 10.0.12.1/30 | to r2 |
| r1 | eth2 | 192.168.1.1/24 | pc1's gateway |
| r2 | eth1 | 10.0.12.2/30 | to r1 |
| r2 | eth2 | 192.168.2.1/24 | pc2's gateway |
| pc1 | eth1 | 192.168.1.10/24 | default via 192.168.1.1 |
| pc2 | eth1 | 192.168.2.10/24 | default via 192.168.2.1 |

Static routes are pre-configured on both routers for each other's LAN.

## Task 1 — deploy and inspect

From the `ccnp` directory:

```bash
./deploy.sh deploy lab00
```

Containerlab prints a node table when done. Things to notice:

- Container names follow `clab-<labname>-<node>`, e.g. `clab-ccnp-lab00-r1`.
- Each node has a **management IP** on the `clab` docker network (172.20.20.0/24 by default).
  That network is for *managing* nodes — lab traffic flows over the point-to-point links defined
  in `lab00-foundation.clab.yml`.
- A `clab-ccnp-lab00/` directory appeared next to the topology file: containerlab keeps runtime
  lab state there.

Inspect the running lab:

```bash
./deploy.sh status lab00
docker ps
```

## Task 2 — connect to nodes

Two ways in:

```bash
# a shell on a Linux "PC"
docker exec -it clab-ccnp-lab00-pc1 bash

# the FRR CLI (vtysh) on a router — an IOS-like experience
docker exec -it clab-ccnp-lab00-r1 vtysh
```

Inside `vtysh` try the classics — FRR deliberately mirrors IOS syntax:

```
r1# show interface brief
r1# show ip route
```

Expected routing table (codes trimmed):

```
S>* 192.168.2.0/24 [1/0] via 10.0.12.2, eth1, weight 1
C>* 10.0.12.0/30 is directly connected, eth1
C>* 192.168.1.0/24 is directly connected, eth2
```

`S` marks the pre-configured static route — with administrative distance 1, exactly like IOS.

## Task 3 — verify end-to-end forwarding

From pc1, trace the full path to pc2:

```bash
docker exec -it clab-ccnp-lab00-pc1 bash
ping -c 3 192.168.2.10
traceroute -n 192.168.2.10
```

Expected:

```
 1  192.168.1.1   ...
 2  10.0.12.2     ...
 3  192.168.2.10  ...
```

You have just verified: host default route → r1 static route → r2 connected LAN. That
three-lookup chain is the mental model for every routing lab that follows.

## Task 4 — watch packets on the wire

Every containerlab node is a network namespace on the host, so you can capture anywhere without
touching the nodes. In one terminal:

```bash
sudo ip netns exec clab-ccnp-lab00-r1 tcpdump -nni eth1 icmp
```

In another, ping from pc1 again and watch echoes cross the r1–r2 link. You'll use this constantly
later (STP BPDUs, OSPF hellos, NHRP, MPLS labels).

## Task 5 — make your own change (static routing)

Add a new prefix and route it — this time *you* do the configuration. Give r2 a loopback:

```bash
docker exec -it clab-ccnp-lab00-r2 vtysh
```

```
r2# configure terminal
r2(config)# interface lo
r2(config-if)# ip address 203.0.113.1/32
r2(config-if)# exit
r2(config)# exit
r2# show ip route connected
```

Now teach r1 how to reach it:

```
r1# configure terminal
r1(config)# ip route 203.0.113.1/32 10.0.12.2
```

Verify from pc1 (its default route already points at r1):

```bash
ping -c 3 203.0.113.1
```

## Task 6 — persist or discard

- FRR keeps running-config in memory. `write memory` in vtysh writes `/etc/frr/frr.conf` — which
  is **bind-mounted from `configs/`**, so saving edits the files in this lab directory. Handy,
  but deliberate: only save if you want to keep your changes in the repo copy.
- Destroy the lab when done:

```bash
./deploy.sh destroy lab00
```

## Challenges

1. Re-deploy and replace both static routes with OSPF: enable `ospfd=yes` in both `daemons`
   files, redeploy, and configure `router ospf` + `network` statements in vtysh so pc1 ↔ pc2
   still ping with **no** static routes. (Peek at `lab-examples/frr01` if stuck.)
2. Add a third router r3 between r2 and pc2 by editing the topology file, and re-plumb the
   addressing so the ping path has four hops.
3. Break it on purpose: shut `eth1` on r2 (`interface eth1` → `shutdown` in vtysh), observe the
   ping fail, and read the routing table to explain *exactly* why.

**Next:** [Lab 01 — Enterprise switching](../lab01-switching/README.md) (requires the IOL-L2
image — see [TUTORIAL.md § 3](../TUTORIAL.md#3-build-the-cisco-iol-images)).
