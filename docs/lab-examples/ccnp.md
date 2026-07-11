|                               |                                                                                                    |
| ----------------------------- | -------------------------------------------------------------------------------------------------- |
| **Description**               | An 11-lab CCNP Enterprise study curriculum (switching, OSPF, EIGRP, BGP, redistribution, IP services, DMVPN, MPLS L3VPN, security, automation) |
| **Components**                | Cisco IOL (via [vrnetlab](../manual/vrnetlab.md)), [FRR](https://docs.frrouting.org/), Linux hosts |
| **Resource requirements**[^1] | :fontawesome-solid-microchip: 2+ <br/>:fontawesome-solid-memory: 4 GB (per lab; largest labs ~4 GB) |
| **Labs folder**               | [ccnp][labsfolder]                                                                                 |
| **Lab names**                 | `ccnp-lab00` … `ccnp-lab10`                                                                        |
| **Version information**[^2]   | `containerlab:0.70+`, `vrnetlab/cisco_iol:17.12.01`, `frr:10.5.1`, `docker-ce:27+`                 |

## Description

A complete, self-paced lab curriculum for the **Cisco CCNP Enterprise** certification
(ENCOR 350-401 v1.2 + ENARSI 300-410), designed for students who want to understand enterprise
and telecom networking hands-on rather than from books alone.

The curriculum ships eleven labs, each with a deployable topology, baseline startup
configurations (IP plumbing pre-done, protocols left as exercises), reference solutions
(deployable via a `--solved` switch), and a step-by-step tutorial README with verification
outputs and challenge tasks:

| Lab | Focus | Blueprint |
| --- | --- | --- |
| lab00 | containerlab foundation (free images: FRR + Linux) | tooling |
| lab01 | VLANs, 802.1Q, LACP EtherChannel, Rapid-PVST+ | ENCOR 3.1 |
| lab02 | multi-area OSPF, stub/NSSA, summarization, auth | ENCOR 3.2 / ENARSI 1.10 |
| lab03 | EIGRP named mode, DUAL, variance, stub | ENARSI 1.9 |
| lab04 | eBGP/iBGP, route reflectors, path attributes | ENCOR 3.2 / ENARSI 1.11 |
| lab05 | two-point redistribution, tags, PBR, IP SLA | ENARSI 1.1–1.4 |
| lab06 | HSRP, NAT/PAT, DHCP, NTP, syslog, SNMP | ENCOR 3.4 / ENARSI 4.x |
| lab07 | GRE, DMVPN phase 1→3, IPsec | ENCOR 2.2 / ENARSI 2.3 |
| lab08 | MPLS L3VPN: LDP, VRF, MP-BGP VPNv4 | ENARSI 2.1/2.2 |
| lab09 | hardening, AAA, ACLs, uRPF, CoPP | ENCOR 5.x / ENARSI 3.x |
| lab10 | NETCONF/YANG, RESTCONF, Python, EEM | ENCOR 6.x |

Start with the [curriculum README][readme] and the [getting-started tutorial][tutorial].
A `deploy.sh` helper wraps deploy/destroy/reset/save/ssh for all labs:

```bash
cd lab-examples/ccnp
./deploy.sh check           # verify docker/containerlab/images
./deploy.sh deploy lab00    # free foundation lab - no Cisco image needed
./deploy.sh deploy 2        # lab02 (multi-area OSPF)
./deploy.sh deploy 2 --solved   # boot lab02 with reference solutions
```

/// note
Labs 01–10 use Cisco IOL images which are **not** distributed with containerlab; you must build
them with [vrnetlab](../manual/vrnetlab.md) from software you are licensed to use (Cisco CML
refplat). Lab 00 runs entirely on public images. See the [Cisco IOL kind docs](../manual/kinds/cisco_iol.md).
///

[labsfolder]: https://github.com/srl-labs/containerlab/tree/main/lab-examples/ccnp
[readme]: https://github.com/srl-labs/containerlab/tree/main/lab-examples/ccnp/README.md
[tutorial]: https://github.com/srl-labs/containerlab/tree/main/lab-examples/ccnp/TUTORIAL.md

[^1]: Resource requirements are provisional. Consult with the installation guides for additional information.
[^2]: The lab has been validated using these versions of the required tools/components. Using versions other than stated might lead to a non-operational setup process.
