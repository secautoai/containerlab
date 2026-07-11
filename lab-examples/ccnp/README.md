# CCNP Enterprise study labs for containerlab

A complete, self-paced lab curriculum for the **Cisco CCNP Enterprise** certification, built on
[containerlab](https://containerlab.dev). Eleven hands-on labs take you from "what is containerlab"
to MPLS L3VPNs, DMVPN and model-driven automation — the topics students consistently rank as the
hardest parts of the exams, and of real telecom/enterprise networks.

Every lab ships with:

| Artifact | Purpose |
| --- | --- |
| `*.clab.yml` | The containerlab topology — deployable in one command |
| `configs/` | Baseline startup configuration (IP plumbing done, protocols left for you) |
| `solutions/` | Complete reference configuration (deployable with `--solved`) |
| `README.md` | A step-by-step tutorial: objectives, tasks, verification output, challenges |

A single [`deploy.sh`](deploy.sh) script deploys, destroys, resets and inspects every lab, and
[TUTORIAL.md](TUTORIAL.md) is the end-to-end getting-started guide (install → images → first lab →
study workflow). Start there if this is your first time with containerlab.

> **Trademark / image note:** Cisco, CCNP, IOS and CML are trademarks of Cisco Systems. This
> curriculum is an independent community study aid, not affiliated with or endorsed by Cisco.
> **No Cisco software is included in this repository** — you must build the Cisco IOL container
> image yourself from software you are licensed to use (see [Images](#images)). Lab 00 uses only
> free, publicly pullable images.

---

## 1. The CCNP Enterprise certification — what is actually required

Researched against the official Cisco exam pages (Cisco Learning Network), current as of
**July 2026**.

To earn CCNP Enterprise you must pass **two exams**:

1. **The core exam — [350-401 ENCOR](https://learningnetwork.cisco.com/s/encor-exam-topics)
   "Implementing Cisco Enterprise Network Core Technologies"**.
   120 minutes, ~$400 USD. Also serves as the qualifying exam for CCIE Enterprise
   Infrastructure/Wireless. **Version 1.2 is the live version since 19 March 2026** — wireless
   design/deployment topics were removed, Zero Trust / SASE / MACsec were added, and the automation
   domain was renamed "Automation and Artificial Intelligence".

2. **One concentration exam** — most candidates (and this curriculum) choose
   **[300-410 ENARSI](https://learningnetwork.cisco.com/s/enarsi-exam-topics) "Implementing Cisco
   Enterprise Advanced Routing and Services"**, 90 minutes. Other options exist (SD-WAN, wireless,
   automation concentrations), but ENARSI is the classic routing-and-services deep dive that pairs
   best with hands-on labbing.

There are no formal prerequisites; Cisco recommends 3–5 years of enterprise networking experience.
CCNA-level knowledge is assumed throughout these labs.

### ENCOR 350-401 v1.2 exam domains

| # | Domain | Weight | What it covers (lab-relevant highlights) |
| --- | --- | --- | --- |
| 1.0 | Architecture | 15% | Enterprise design (2/3-tier, fabric), high availability (FHRP), Catalyst SD-WAN, SD-Access, QoS concepts, switching mechanisms (CEF, FIB/RIB) |
| 2.0 | Virtualization | 10% | Device virtualization (hypervisors, VMs, virtual switching), data-path virtualization: **VRF, GRE, IPsec**, LISP/VXLAN concepts |
| 3.0 | Infrastructure | 30% | **Layer 2:** 802.1Q trunking, static/LACP EtherChannel, RSTP/MST + guards. **Layer 3:** OSPFv2/v3 (areas, network types, summarization, filtering), eBGP (peering, path selection). **IP services:** NTP, NAT/PAT, HSRP/VRRP, multicast concepts |
| 4.0 | Network Assurance | 10% | debug/conditional debug, ping/traceroute, **NetFlow/Flexible NetFlow**, SPAN/ERSPAN, **IP SLA**, Catalyst Center workflows, **NETCONF/RESTCONF** |
| 5.0 | Security | 20% | Device access control (lines, **AAA**), **ACLs, CoPP**, REST API security, **Zero Trust Architecture, SASE, MACsec/TrustSec** *(new in v1.2)*, 802.1X/MAB/WebAuth concepts |
| 6.0 | Automation and Artificial Intelligence | 15% | Python fundamentals, **JSON**, **YANG models**, **NETCONF/RESTCONF APIs**, Catalyst Center & SD-WAN Manager APIs, **EEM**, orchestration tools, AI/ML for network operations *(new in v1.2)* |

### ENARSI 300-410 exam domains

| # | Domain | Weight | What it covers (lab-relevant highlights) |
| --- | --- | --- | --- |
| 1.0 | Layer 3 Technologies | 35% | Administrative distance, route maps, loop prevention, **bidirectional redistribution with filtering/tagging**, summarization, **policy-based routing**, VRF-Lite, BFD, **EIGRP (classic & named mode, stubs, unequal-cost load balancing)**, **OSPF (network types, path preference, areas/LSAs)**, **BGP (iBGP/eBGP, path selection, attributes, route reflectors, policies)** |
| 2.0 | VPN Technologies | 20% | **MPLS operations (LSR, LDP, labels, LSPs), MPLS L3VPN**, **DMVPN (mGRE, NHRP, IPsec, spoke-to-spoke)** |
| 3.0 | Infrastructure Security | 20% | **IOS AAA** (local, TACACS+/RADIUS concepts), **IPv4 ACLs / IPv6 traffic filters, uRPF, CoPP**, IPv6 first-hop security concepts |
| 4.0 | Infrastructure Services | 25% | Device management (SSH/SCP, console/VTY), **SNMP, syslog, debugs**, **DHCP (server/relay/options)**, **NetFlow v5/v9/Flexible**, **IP SLA + object tracking** |

### What a virtual lab can and cannot cover

Roughly 75–80% of the combined blueprints is CLI-configurable technology that these labs cover
hands-on. The remainder is concept/architecture material you should study from books/videos:
SD-Access & SD-WAN controller workflows (need Catalyst Center / SD-WAN Manager appliances),
wireless (removed from ENCOR v1.2 anyway), QoS hardware behavior, Zero Trust/SASE architecture,
and AI/ML concepts. Each lab README lists exactly which blueprint items it maps to.

---

## 2. The curriculum

Recommended order below. Labs are independent (any lab can be deployed alone), but concepts build
on earlier labs. "RAM" is the approximate memory used by the routers (768 MB per IOL node).

| Lab | Title | Key technologies | Blueprint mapping | Nodes | RAM |
| --- | --- | --- | --- | --- | --- |
| [00](lab00-foundation/README.md) | Containerlab foundation *(free images)* | containerlab workflow, IP addressing, static routing, FRR | tooling prerequisite | 2 FRR + 2 hosts | <0.5 GB |
| [01](lab01-switching/README.md) | Enterprise switching | VLANs, 802.1Q, LACP EtherChannel, Rapid-PVST+, root manipulation, PortFast/BPDU Guard, inter-VLAN SVIs | ENCOR 3.1.a/b/c, 1.0 design | 3 IOL-L2 + 2 hosts | ~2.3 GB |
| [02](lab02-ospf/README.md) | Multi-area OSPF | Areas, DR/BDR, network types, cost, passive-interface, totally stubby, NSSA, summarization, default origination, authentication | ENCOR 3.2.b · ENARSI 1.10 | 4 IOL | ~3 GB |
| [03](lab03-eigrp/README.md) | EIGRP named mode | DUAL, successors/FS, wide metrics, variance (unequal-cost LB), summarization, stub, SHA authentication | ENARSI 1.9 · ENCOR 3.2.a | 4 IOL | ~3 GB |
| [04](lab04-bgp/README.md) | BGP peering & policy | eBGP/iBGP, OSPF underlay, next-hop-self, route reflectors, weight/local-pref/AS-path/MED, aggregation | ENCOR 3.2.c · ENARSI 1.11 | 5 IOL | ~3.8 GB |
| [05](lab05-redistribution/README.md) | Redistribution & path control | Two-point mutual redistribution, tags & loop prevention, distribute/prefix lists, PBR, IP SLA + tracking, floating statics | ENARSI 1.1–1.6, 4.5 | 4 IOL | ~3 GB |
| [06](lab06-services/README.md) | IP services (FHRP, NAT, DHCP) | HSRPv2 + tracking, VRRP, PAT/static NAT, DHCP server/relay, NTP, syslog, SNMP | ENCOR 3.4.a–c · ENARSI 4.2–4.4 | 3 IOL + 1 IOL-L2 + 2 hosts | ~3 GB |
| [07](lab07-vpn/README.md) | GRE, DMVPN & IPsec | p2p GRE, mGRE+NHRP (DMVPN phase 1→3), EIGRP overlay, spoke-to-spoke tunnels, IPsec profiles | ENCOR 2.2.b · ENARSI 2.3 | 4 IOL | ~3 GB |
| [08](lab08-mpls/README.md) | MPLS L3VPN | LDP, label switching, VRFs, RD/RT, MP-BGP VPNv4, PE-CE eBGP | ENARSI 2.1/2.2 · ENCOR 2.2.a | 5 IOL | ~3.8 GB |
| [09](lab09-security/README.md) | Infrastructure security | Device hardening, local AAA, VTY ACLs, extended & object-group ACLs, uRPF, CoPP | ENCOR 5.1/5.2 · ENARSI 3.1–3.3 | 3 IOL + 1 host | ~2.3 GB |
| [10](lab10-automation/README.md) | Automation & programmability | NETCONF + YANG, RESTCONF, JSON, Python (ncclient/requests), EEM applets | ENCOR 6.x, 4.x · ENARSI 4.x | 2 IOL | ~1.5 GB |

**Suggested pacing** for a cohort or self-study: one lab per week, 2–4 hours each including the
challenge tasks — an 11-week program that touches every configurable blueprint area.

---

## 3. Requirements

### Host

- Linux host (bare metal, VM, or WSL2) with **x86_64/amd64 CPU** — Cisco IOL is an x86 binary and
  does not run on ARM hosts (Apple Silicon users: use a x86 cloud VM or devcontainer).
- Docker and containerlab installed (see [TUTORIAL.md](TUTORIAL.md)).
- **4 GB free RAM** minimum (8 GB recommended) — enough for any single lab here.
- ~2 GB disk for images.

### Images

| Image | Used by | How to get it |
| --- | --- | --- |
| `quay.io/frrouting/frr:10.5.1` | lab00 | public — pulled automatically |
| `wbitt/network-multitool:3.22.2` | host/PC nodes | public — pulled automatically |
| `vrnetlab/cisco_iol:17.12.01` | labs 02–10 (routers) | **you build it**: obtain the IOL binary (`x86_64_crb_linux-adventerprisek9-ms`) from the Cisco CML refplat ISO (requires a CML license / CCO entitlement), then package it with [vrnetlab](https://containerlab.dev/manual/vrnetlab/) |
| `vrnetlab/cisco_iol:L2-17.12.01` | labs 01, 06 (switches) | same — from the `ioll2` binary in the CML refplat ISO |

Step-by-step image build instructions are in [TUTORIAL.md § 3](TUTORIAL.md#3-build-the-cisco-iol-images).
If your image tags differ, export `CCNP_IOL_IMAGE` / `CCNP_IOL_L2_IMAGE` — every topology reads them:

```bash
export CCNP_IOL_IMAGE=vrnetlab/cisco_iol:17.15.01
export CCNP_IOL_L2_IMAGE=vrnetlab/cisco_iol:L2-17.15.01
```

---

## 4. Quick start

```bash
# from this directory
./deploy.sh check              # verify docker/containerlab/images
./deploy.sh list               # list all labs and their state
./deploy.sh deploy lab00       # deploy the free foundation lab
./deploy.sh deploy 2           # deploy lab02 (multi-area OSPF)
./deploy.sh ssh 2 r1           # SSH to node r1 of lab02 (admin/admin)
./deploy.sh save 2             # write memory on all lab02 nodes
./deploy.sh destroy 2          # stop lab02, KEEP saved configs (NVRAM)
./deploy.sh reset 2            # stop lab02 and wipe it back to baseline
./deploy.sh deploy 2 --solved  # deploy lab02 with the solution configs
```

Then open the lab's `README.md` and work through the tasks. Full workflow explained in
[TUTORIAL.md](TUTORIAL.md).

### Conventions used in every lab

- **Credentials:** `admin` / `admin` (privilege 15), SSH enabled out of the box.
- **Management:** `Ethernet0/0` sits in the `clab-mgmt` VRF with a DHCP address from the
  containerlab management network — never touch it, and it never participates in the lab routing.
- **Addressing:** point-to-point links use `10.0.XY.0/24` (X, Y = router numbers, router N is
  host `.N`); router IDs are `10.255.255.N/32` loopbacks; internal LANs use `172.16.0.0/16`;
  "internet"/external prefixes use documentation ranges (`198.51.100.0/24`, `203.0.113.0/24`) and
  CGN space (`100.64.0.0/10`) for WAN underlays.
- **Persistence:** `write memory` saves to NVRAM, which survives `destroy`/`deploy` cycles.
  Baseline `configs/` are applied only on the **first** boot; use `./deploy.sh reset <lab>` to wipe
  NVRAM and return to the baseline.

---

## 5. Sources

- [Cisco: 350-401 ENCOR exam topics](https://learningnetwork.cisco.com/s/encor-exam-topics) — v1.2 blueprint (live 19 Mar 2026)
- [Cisco: 300-410 ENARSI exam topics](https://learningnetwork.cisco.com/s/enarsi-exam-topics)
- [Cisco: ENARSI exam page](https://www.cisco.com/site/us/en/learn/training-certifications/exams/enarsi.html)
- [CCNP ENCOR v1.2 change summaries](https://www.nwkings.com/ccnp-encor-v1-2-updates) ([also](https://certland.net/blog/cisco-ccnp-encor-350-401-study-guide-2026/))
- [containerlab: Cisco IOL kind documentation](https://containerlab.dev/manual/kinds/cisco_iol/)
- [containerlab: vrnetlab integration](https://containerlab.dev/manual/vrnetlab/)
