# Lab 06 — IP services: HSRP, NAT/PAT, DHCP, NTP, syslog, SNMP

**Goal:** run the services every enterprise LAN depends on. Build a first-hop-redundant gateway
pair (HSRPv2 with object tracking), translate the LAN to the "internet" with PAT plus a static
port-forward, serve DHCP with failover-aware defaults, sync clocks with NTP, and stream syslog +
SNMP traps to a monitoring host — then *watch* the packets arrive.

| | |
| --- | --- |
| Blueprint mapping | **ENCOR 3.4.c** (FHRP: HSRP/VRRP), **3.4.b** (NAT/PAT), **3.4.a** (NTP), **4.2** (syslog/SNMP), **ENARSI 4.2** (DHCP), 4.3/4.4 (SNMP/logging), 4.6 (NAT) |
| Nodes / RAM | 3× IOL + 1× IOL-L2 + 2 hosts / ~3 GB |
| Estimated time | 2.5–3.5 h |

## Topology

```
   pc1 (.11 static)   pc2 (DHCP, task 4)
        \             /
         +--- sw1 ---+            LAN 10.1.10.0/24  (VIP .1)
         /           \
   e0/1 / .2          \ .3 e0/1
      r1               r2         <- you configure these two
   e0/2 | .1        .5 | e0/2
203.0.113.0/30   203.0.113.4/30
        \             /
         +--- r3 ----+            r3 = ISP (pre-configured, hands off)
              |                       * NTP master (stratum 3)
        Lo0 198.51.100.53             * "internet host" 198.51.100.53
```

sw1 is a plain VLAN-1 switch (pre-configured). r1/r2 have addresses + default routes only.
pc1 = 10.1.10.11 (static, gateway **10.1.10.1** — an address that doesn't exist yet!).

```bash
./deploy.sh deploy lab06 && ./deploy.sh ssh 6 r1
```

## Task 1 — HSRPv2 with preemption

pc1's gateway 10.1.10.1 must be a **virtual** IP owned by whichever router is alive. On the LAN
interfaces:

```
! r1
interface Ethernet0/1
 standby version 2
 standby 10 ip 10.1.10.1
 standby 10 priority 110
 standby 10 preempt

! r2
interface Ethernet0/1
 standby version 2
 standby 10 ip 10.1.10.1
 standby 10 preempt
```

Verify:

```
r1# show standby brief
                     P indicates configured to preempt.
Interface   Grp  Pri P State   Active          Standby         Virtual IP
Et0/1       10   110 P Active  local           10.1.10.3       10.1.10.1

r1# show standby Ethernet0/1 | include Virtual mac
  Virtual mac address is 0000.0c9f.f00a (v2 default)
```

Exam gold in that MAC: HSRPv2 uses `0000.0C9F.Fxxx` (xxx = group in hex — group 10 = 00A); v1
uses `0000.0C07.ACxx` and groups only 0–255. Preemption is **off by default** — without it r1
would never reclaim Active after recovering. From pc1: `ping 10.1.10.1` and
`arp -n | grep 10.1.10.1` — the VIP resolves to the virtual MAC.

## Task 2 — object tracking

If r1's **WAN** dies, r1 is still the LAN's Active gateway — a blackhole. Track the uplink:

```
! r1
track 1 interface Ethernet0/2 line-protocol
interface Ethernet0/1
 standby 10 track 1 decrement 20
```

Test: `shutdown` r1 e0/2 → priority drops 110→90 (< r2's 100) → r2 preempts to Active
(`show standby brief` on r2; `%HSRP-5-STATECHANGE` in the log). Start a continuous ping from
pc1 to 198.51.100.53 during the failover — after NAT is up (task 3), repeat and count lost
packets. `no shutdown` and watch r1 preempt back.

## Task 3 — NAT/PAT to the internet

The ISP knows nothing about 10.1.10.0/24 (like real life). Overload on the egress interface —
on **both** r1 and r2:

```
ip access-list standard ACL-NAT-INSIDE
 permit 10.1.10.0 0.0.0.255
!
interface Ethernet0/1
 ip nat inside
interface Ethernet0/2
 ip nat outside
!
ip nat inside source list ACL-NAT-INSIDE interface Ethernet0/2 overload
```

From pc1: `ping 198.51.100.53` then `traceroute -n 198.51.100.53`. Inspect on r1:

```
r1# show ip nat translations
Pro Inside global      Inside local       Outside local      Outside global
icmp 203.0.113.1:20    10.1.10.11:20      198.51.100.53:20   198.51.100.53:20
```

Memorize the four-column terminology (inside local/global, outside local/global) — guaranteed
question material. Add a **static PAT** (port-forward): pc1's multitool image runs a web server
on :80; publish it as 203.0.113.1:8080:

```
! r1
ip nat inside source static tcp 10.1.10.11 80 203.0.113.1 8080
```

Verify from the ISP: `r3# telnet 203.0.113.1 8080` → `Trying ... Open` (Ctrl+Shift+6 then x,
`disconnect`). `show ip nat translations` now has a permanent `tcp 203.0.113.1:8080` entry.

## Task 4 — DHCP server

pc2 boots with **no address** on purpose. Serve it from r1:

```
! r1
ip dhcp excluded-address 10.1.10.1 10.1.10.10
ip dhcp excluded-address 10.1.10.11
ip dhcp pool LAN
 network 10.1.10.0 255.255.255.0
 default-router 10.1.10.1
 dns-server 198.51.100.53
 domain-name ccnp.lab
 lease 0 4
```

Note `default-router` hands out the **HSRP VIP** — clients survive gateway failover. Request a
lease from pc2 (host shell):

```bash
docker exec -it clab-ccnp-lab06-pc2 udhcpc -i eth1
docker exec clab-ccnp-lab06-pc2 ip addr show eth1     # 10.1.10.12/24 (first free)
docker exec clab-ccnp-lab06-pc2 ping -c 3 198.51.100.53
```

```
r1# show ip dhcp binding
IP address    Client-ID/Hw address    Lease expiration        Type
10.1.10.12    01aa.c1ab.xxxx.xx       ... (4 hours)           Automatic
```

DORA (Discover-Offer-Request-Ack) is observable: `debug ip dhcp server packet` while pc2 renews
(`udhcpc -i eth1` again). **Relay concept:** here server and client share a segment; when they
don't, the gateway needs `ip helper-address <server>` on the *client-facing* interface —
challenge 2 makes you build it.

## Task 5 — NTP

Logs and certificates are worthless without time. r3 is pre-configured as `ntp master 3`:

```
! r1                         ! r2
ntp server 203.0.113.2       ntp server 203.0.113.6
```

NTP converges slowly — deploy the task, continue the lab, and re-check in ~5 min:

```
r1# show ntp associations
  address         ref clock       st   when   poll reach  delay  offset   disp
*~203.0.113.2     127.127.1.1      3     14     64   377  1.000   0.500  1.1
r1# show ntp status
Clock is synchronized, stratum 4, reference is 203.0.113.2
```

`*` = synced peer; `reach 377` = last 8 polls all answered (octal bitmask — exam detail!);
our stratum = upstream + 1.

## Task 6 — syslog to a collector

pc1 doubles as the monitoring station. On **r1 and r2**:

```
service timestamps log datetime msec
logging host 10.1.10.11
logging trap informational
```

Watch messages *arrive on the wire* — in a host terminal:

```bash
docker exec -it clab-ccnp-lab06-pc1 tcpdump -nni eth1 -A udp port 514
```

Trigger one: `conf t` → `interface Loopback99` → `no interface Loopback99` on r1 →
`%SYS-5-CONFIG_I` / `%LINEPROTO-5-UPDOWN` lines appear in the capture. Know the 0–7 severity
ladder (emergency…debugging) and that `logging trap informational` = send severities 0–6.

## Task 7 — SNMP (read-only + traps)

```
! r1
snmp-server community ccnp-ro RO
snmp-server location ccnp-lab06
snmp-server contact student
snmp-server host 10.1.10.11 version 2c ccnp-ro
snmp-server enable traps snmp linkdown linkup coldstart
```

Capture a trap on pc1 (`tcpdump -nni eth1 udp port 162 -A`), then flap `Loopback99` again or
`shutdown`/`no shutdown` e0/2 briefly — the linkdown/linkup traps land in the capture. Security
talking points for the exam: v2c = community strings in cleartext (pair with ACLs:
`snmp-server community ccnp-ro RO <acl>`); v3 adds auth+priv — see challenge 4.

## Task 8 — failover grand finale

With everything running: continuous ping from pc2 (DHCP client!) to 198.51.100.53, then pull r1's
LAN cable the brutal way — `shutdown` its e0/1. Observe, in order: HSRP failover to r2
(`%HSRP-5-STATECHANGE` via syslog on pc1's capture), pings resuming through r2's PAT (new
translations on r2 — old flows break: NAT state isn't replicated, a real design discussion),
and the linkdown trap. Restore and `./deploy.sh save 6`.

## Challenges

1. Convert group 10 from HSRP to **VRRP** (`vrrp 10 ip 10.1.10.1` etc.). List the protocol
   differences you can *see*: default timers, preemption default, virtual MAC
   (`0000.5e00.01xx`), who owns the VIP if it equals a real interface address.
2. Move the DHCP pool to **r3** and make it work with `ip helper-address` on r1/r2 — you'll
   have to solve the return-path problem the ISP has for 10.1.10.0/24 (that's the point).
3. Balance the load: create a second HSRP group 20 with r2 active and hand half the clients a
   VIP of 10.1.10.254 via a second DHCP pool — poor man's GLBP (and know why GLBP itself did
   this automatically).
4. Configure **SNMPv3**: group with `priv`, user with SHA auth + AES 128, and re-capture — the
   trap payload is now encrypted vs the v2c one you saw.
5. Make NAT survive gateway failover for *new* flows only, and explain precisely why *existing*
   TCP sessions still die (translation table locality) and what real designs do about it
   (stateful NAT redundancy / moving NAT upstream).

<details><summary>Solution reference</summary>

Final configs in [`solutions/`](solutions/); deploy with `./deploy.sh deploy 6 --solved`
(covers tasks 1–7).
</details>

**Next:** [Lab 07 — GRE, DMVPN & IPsec](../lab07-vpn/README.md)
