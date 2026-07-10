# Lab 01 — Enterprise switching: VLANs, trunking, EtherChannel, Rapid-PVST+

**Goal:** build a small campus L2 domain the way ENCOR expects you to: VLANs and 802.1Q trunks,
an LACP EtherChannel, a deliberately engineered Rapid-PVST+ topology with edge-port protection,
and SVI-based inter-VLAN routing.

| | |
| --- | --- |
| Blueprint mapping | **ENCOR 3.1.a** (802.1Q trunking), **3.1.b** (EtherChannel), **3.1.c** (RSTP + PortFast/BPDU Guard), **1.x** (2-tier design), inter-VLAN routing |
| Nodes / RAM | 3× IOL-**L2**, 2× Linux hosts / ~2.3 GB |
| Estimated time | 2–3 h |

## Topology & plan

```
          Po1 (e0/1+e0/2, LACP)
    sw1 ==================== sw2
     \                        /
  e0/3\                      /e0/3
       \e0/1            e0/2/
        +------ sw3 --------+
              e0/3| |e1/0
               pc1   pc2
            VLAN 10  VLAN 20
```

| VLAN | Name | Subnet | Gateway (SVI on sw1) | STP root |
| --- | --- | --- | --- | --- |
| 10 | USERS | 10.1.10.0/24 | 10.1.10.1 | **sw1** (sw2 backup) |
| 20 | SERVERS | 10.1.20.0/24 | 10.1.20.1 | **sw2** (sw1 backup) |
| 99 | NATIVE | — | — | trunk native VLAN |

Hosts: pc1 = 10.1.10.11 (VLAN 10), pc2 = 10.1.20.12 (VLAN 20), both pre-addressed.

```bash
./deploy.sh deploy lab01     # from the ccnp directory
./deploy.sh ssh 1 sw1        # admin/admin
```

> IOL-L2 boots in ~30–60 s. `Ethernet0/0` is management — never configure it.

## Task 0 — explore the baseline

```
sw1# show vlan brief                 ! only default VLANs exist
sw1# show interfaces status          ! ports up, all access/vlan 1
sw1# show spanning-tree summary      ! note the default STP mode
sw1# show cdp neighbors              ! verify the cabling matches the diagram
```

With everything in VLAN 1 and STP running, the sw1–sw2–sw3 triangle already has one port
**blocking** — find it:

```
sw1# show spanning-tree vlan 1
```

## Task 1 — VLANs

On **all three** switches:

```
configure terminal
vlan 10
 name USERS
vlan 20
 name SERVERS
vlan 99
 name NATIVE
end
```

Verify: `show vlan brief` — 10/20/99 present, no ports assigned yet. The baseline pins
`vtp mode transparent`; check `show vtp status` and be able to say *why* transparent mode is safe
practice (a switch with a higher revision number can't overwrite your VLAN database).

## Task 2 — 802.1Q trunks (sw1↔sw3, sw2↔sw3)

IOL-L2 supports ISL and dot1q, so the encapsulation must be set before trunk mode. On sw1:

```
interface Ethernet0/3
 switchport trunk encapsulation dot1q
 switchport trunk native vlan 99
 switchport trunk allowed vlan 10,20
 switchport mode trunk
 switchport nonegotiate
```

Repeat on sw2 `e0/3`, and on sw3 for **both** `e0/1` and `e0/2`.

Verify on sw3:

```
sw3# show interfaces trunk

Port        Mode             Encapsulation  Status        Native vlan
Et0/1       on               802.1q         trunking      99
Et0/2       on               802.1q         trunking      99

Port        Vlans allowed on trunk
Et0/1       10,20
Et0/2       10,20
```

Know for the exam:

- `switchport mode trunk` + `nonegotiate` = static trunk, no DTP frames. Without `nonegotiate`,
  DTP still *originates* negotiation on a static trunk.
- `dynamic desirable` vs `dynamic auto`: desirable/desirable, desirable/auto and trunk/auto all
  form trunks; **auto + auto never does**.
- Native VLAN must match on both ends (`%CDP-4-NATIVE_VLAN_MISMATCH` warns you when it doesn't).

## Task 3 — LACP EtherChannel (sw1↔sw2)

Bundle both parallel links into Port-channel 1. On **sw1 and sw2**:

```
interface range Ethernet0/1 - 2
 switchport trunk encapsulation dot1q
 switchport trunk native vlan 99
 switchport trunk allowed vlan 10,20
 switchport mode trunk
 switchport nonegotiate
 channel-group 1 mode active
!
interface Port-channel1
 switchport trunk encapsulation dot1q
 switchport trunk native vlan 99
 switchport trunk allowed vlan 10,20
 switchport mode trunk
 switchport nonegotiate
```

Verify:

```
sw1# show etherchannel summary
...
Group  Port-channel  Protocol    Ports
------+-------------+-----------+-----------------------------------------------
1      Po1(SU)         LACP      Et0/1(P)    Et0/2(P)
```

`SU` = Layer2 + in-use; members `(P)` = bundled. Also check `show lacp neighbor` and
`show spanning-tree vlan 10` — STP now sees **one** logical port (Po1), which is the whole point:
both links forward, none blocked.

Exam traps: mode `active/active` or `active/passive` bundles (LACP); `passive/passive` never
bundles; `on` must be `on` both sides (no protocol); member ports must match speed/duplex/VLAN
config or they're suspended (`s`).

## Task 4 — engineer Rapid-PVST+

Set the mode explicitly on **all three** switches, then split the root role per VLAN:

```
! all switches
spanning-tree mode rapid-pvst

! sw1
spanning-tree vlan 10 root primary
spanning-tree vlan 20 root secondary

! sw2
spanning-tree vlan 20 root primary
spanning-tree vlan 10 root secondary
```

> `root primary` is a macro — the config stores `spanning-tree vlan 10 priority 24576`
> (secondary = 28672). Priorities must be multiples of 4096; the VLAN ID is added as the
> extended system ID.

Verify from the access layer:

```
sw3# show spanning-tree vlan 10
VLAN0010
  Root ID    Priority    24586          ! 24576 + VLAN 10
             Address     aabb.cc00.xxxx
  ...
Interface           Role Sts Cost      Prio.Nbr Type
------------------- ---- --- --------- -------- --------------------------------
Et0/1               Root FWD 100       128.2    P2p
Et0/2               Altn BLK 100       128.3    P2p
Et0/3               Desg FWD 100       128.4    P2p
```

For VLAN 20 the roles flip (`Et0/2` root, `Et0/1` blocking) — per-VLAN load sharing of the
uplinks. Questions to answer (all fair game on ENCOR):

1. Why is sw3 never root? (both distribution switches advertise better priority)
2. On sw2, which port is the root port for VLAN 10 and why is it Po1? (lowest path cost to root)
3. What P2p/Edge link types does RSTP use for fast transitions, and what happened to
   listening/learning?

## Task 5 — access ports with edge protection

On sw3:

```
interface Ethernet0/3
 switchport mode access
 switchport access vlan 10
 spanning-tree portfast
 spanning-tree bpduguard enable
!
interface Ethernet1/0
 switchport mode access
 switchport access vlan 20
 spanning-tree portfast
 spanning-tree bpduguard enable
```

Verify: `show spanning-tree interface e0/3 detail` (edge port, bpdu guard), `show vlan brief`
(ports in 10/20). PortFast ports go straight to forwarding; BPDU Guard err-disables the port if a
switch is ever plugged in. Recovery from err-disable: `shutdown` / `no shutdown` (or
`errdisable recovery cause bpduguard`).

## Task 6 — inter-VLAN routing (SVIs)

On **sw1** (our small "core"):

```
ip routing
interface Vlan10
 ip address 10.1.10.1 255.255.255.0
 no shutdown
interface Vlan20
 ip address 10.1.20.1 255.255.255.0
 no shutdown
```

End-to-end test from the host shells:

```bash
docker exec -it clab-ccnp-lab01-pc1 bash
ping -c 3 10.1.10.1      # gateway (SVI, same VLAN)
ping -c 3 10.1.20.12     # pc2 - routed VLAN10 -> VLAN20 on sw1
traceroute -n 10.1.20.12 # one L3 hop: 10.1.10.1
```

An SVI comes up only if the VLAN exists **and** has at least one active port — a favorite exam
question (`show interfaces vlan 10` stuck down/down → VLAN missing or no member port).

## Task 7 — break it and watch RSTP converge

Start a continuous ping pc1 → pc2, then fail the inter-switch bundle:

```
sw1(config)# interface range e0/1 - 2
sw1(config-if-range)# shutdown
```

`show spanning-tree vlan 20` on sw1: its path to the VLAN-20 root (sw2) now goes **through sw3**
— the former Altn port is forwarding. RSTP reconverges in well under a second (compare with
802.1D's 30–50 s story). `no shutdown` to restore, then save everywhere:

```bash
./deploy.sh save 1
```

## Challenges (no command hints — exam level)

1. Prune VLAN 10 from the sw2↔sw3 trunk and predict, *before* checking, what pc1↔pc2 traffic
   does when Po1 is also down. Verify, explain, undo.
2. Convert Po1 to static `on` mode on sw1 only. Observe and explain the result
   (`show etherchannel summary`, err-disable risk), then fix it properly.
3. Replace Rapid-PVST+ with **MST**: one region `CCNP`, instance 1 = VLANs 10+20, sw1 root for
   instance 1. Verify with `show spanning-tree mst configuration` — all three switches must show
   the same config digest.
4. Add root guard on sw1/sw2 ports facing sw3 and explain the failure mode it prevents.
5. On the trunk to sw3, allow only the VLANs actually needed. Why is that + `nonegotiate` + an
   unused native VLAN considered trunk hardening? (VLAN-hopping story.)

<details><summary>Solution reference</summary>

Full reference configs: [`solutions/`](solutions/). Deploy them directly with
`./deploy.sh reset 1 && ./deploy.sh deploy 1 --solved` (solutions cover tasks 1–6).
</details>

**Next:** [Lab 02 — Multi-area OSPF](../lab02-ospf/README.md)
