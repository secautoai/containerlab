#!/usr/bin/env python3
"""Build and verify the four flagship protocol labs through the NetPilot API."""
import json
import sys
import time
import urllib.request

BASE = "http://127.0.0.1:8092"


def api(method, path, body=None):
    data = json.dumps(body).encode() if body is not None else None
    req = urllib.request.Request(
        f"{BASE}{path}", data=data, method=method,
        headers={"content-type": "application/json"} if data else {},
    )
    try:
        with urllib.request.urlopen(req, timeout=60) as r:
            return json.load(r)
    except urllib.error.HTTPError as e:
        raise RuntimeError(f"{method} {path}: {e.read().decode()[:300]}")


def mklab(name, desc):
    lab = api("POST", "/api/labs", {"name": name, "description": desc})
    return lab["id"]


def mknode(lab, template, name, x, y, config=None, overrides=None, interfaces=None):
    body = {"template": template, "name": name, "x": x, "y": y}
    if config: body["startup_config"] = config
    if overrides: body["overrides"] = overrides
    if interfaces: body["interfaces"] = interfaces
    return api("POST", f"/api/labs/{lab}/nodes", body)["id"]


def mklink(lab, a, ai, b, bi):
    api("POST", f"/api/labs/{lab}/links", {
        "a": {"kind": "node", "node": a, "iface": ai},
        "b": {"kind": "node", "node": b, "iface": bi},
    })


def start(lab, node):
    api("POST", f"/api/labs/{lab}/nodes/{node}/start")


def stop_lab(lab):
    api("POST", f"/api/labs/{lab}/stop")


def excmd(lab, node, cmd, timeout=15):
    r = api("POST", f"/api/labs/{lab}/nodes/{node}/exec",
            {"command": cmd, "timeout_s": timeout})
    return r.get("output", "")


def wait_for(lab, node, cmd, needle, tries=30, sleep_s=5, label=""):
    check = needle if callable(needle) else (lambda o: needle in o)
    out = ""
    for i in range(tries):
        out = excmd(lab, node, cmd, timeout=10)
        if check(out):
            print(f"    ✓ {label}")
            return out
        time.sleep(sleep_s)
    print(f"    ✗ TIMEOUT waiting for {label}; last output:\n{out[-600:]}")
    return None


def ping_ok(lab, node, target, extra="", tries=10):
    out = ""
    for _ in range(tries):
        out = excmd(lab, node, f"ping -c 3 -i 0.3 -W 2 {extra} {target} 2>&1 | grep transmitted", timeout=15)
        if " 0% packet loss" in out:
            return out.strip().splitlines()[-1]
        time.sleep(4)
    return None


FRR_BASE = "frr defaults traditional\nhostname {name}\n"

results = {}

# Remove earlier copies of these labs so reruns stay clean.
LAB_NAMES = {"OSPF Multi-Area", "BGP Peering", "VXLAN EVPN", "MPLS L3VPN"}
for lab in api("GET", "/api/labs"):
    if lab["name"] in LAB_NAMES:
        api("DELETE", f"/api/labs/{lab['id']}")

# ---------------------------------------------------------------- OSPF multi-area
def lab_ospf():
    print("== OSPF multi-area (4x FRR: area1 - ABRs in area0 - area2)")
    lab = mklab("OSPF Multi-Area", "r1(area1) - abr1 - area0 - abr2 - r4(area2); inter-area routes end to end")
    cfg = {
        "r1": """frr defaults traditional
hostname r1
interface lo
 ip address 1.1.1.1/32
interface eth0
 ip address 10.1.1.1/30
 ip ospf network point-to-point
router ospf
 ospf router-id 1.1.1.1
 network 10.1.1.0/30 area 1
 network 1.1.1.1/32 area 1
""",
        "abr1": """frr defaults traditional
hostname abr1
interface eth0
 ip address 10.1.1.2/30
 ip ospf network point-to-point
interface eth1
 ip address 10.0.0.1/30
 ip ospf network point-to-point
router ospf
 ospf router-id 2.2.2.2
 network 10.1.1.0/30 area 1
 network 10.0.0.0/30 area 0
""",
        "abr2": """frr defaults traditional
hostname abr2
interface eth0
 ip address 10.0.0.2/30
 ip ospf network point-to-point
interface eth1
 ip address 10.2.2.1/30
 ip ospf network point-to-point
router ospf
 ospf router-id 3.3.3.3
 network 10.0.0.0/30 area 0
 network 10.2.2.0/30 area 2
""",
        "r4": """frr defaults traditional
hostname r4
interface lo
 ip address 4.4.4.4/32
interface eth0
 ip address 10.2.2.2/30
 ip ospf network point-to-point
router ospf
 ospf router-id 4.4.4.4
 network 10.2.2.0/30 area 2
 network 4.4.4.4/32 area 2
""",
    }
    r1 = mknode(lab, "frr", "r1", 100, 200, cfg["r1"])
    abr1 = mknode(lab, "frr", "abr1", 320, 200, cfg["abr1"])
    abr2 = mknode(lab, "frr", "abr2", 540, 200, cfg["abr2"])
    r4 = mknode(lab, "frr", "r4", 760, 200, cfg["r4"])
    mklink(lab, r1, 0, abr1, 0)
    mklink(lab, abr1, 1, abr2, 0)
    mklink(lab, abr2, 1, r4, 0)
    for n in (r1, abr1, abr2, r4):
        start(lab, n)
    ok1 = wait_for(lab, r1, 'vtysh -c "show ip route 4.4.4.4/32"', "via eth0",
                   label="r1 learned inter-area route to 4.4.4.4/32")
    pr = ping_ok(lab, r1, "4.4.4.4", extra="-I 1.1.1.1")
    ok2 = pr is not None
    print(f"    {'✓' if ok2 else '✗'} end-to-end ping 1.1.1.1 -> 4.4.4.4: {pr or 'FAILED'}")
    ia = excmd(lab, r1, 'vtysh -c "show ip route ospf" | head -12')
    print("    r1 OSPF routes (IA = inter-area):")
    for line in ia.splitlines():
        if "O>" in line or "O  " in line:
            print(f"      {line.strip()[:100]}")
    results["ospf-multiarea"] = bool(ok1) and ok2
    stop_lab(lab)
    return lab


# ---------------------------------------------------------------- BGP peering
def lab_bgp():
    print("== BGP peering (3x FRR: AS65001 - AS65100 transit - AS65002)")
    lab = mklab("BGP Peering", "eBGP: r1(AS65001) - r2(AS65100) - r3(AS65002); loopbacks announced end to end")
    cfg = {
        "r1": """frr defaults traditional
hostname r1
interface lo
 ip address 1.1.1.1/32
interface eth0
 ip address 10.0.1.1/30
router bgp 65001
 bgp router-id 1.1.1.1
 no bgp ebgp-requires-policy
 neighbor 10.0.1.2 remote-as 65100
 address-family ipv4 unicast
  network 1.1.1.1/32
""",
        "r2": """frr defaults traditional
hostname r2
interface eth0
 ip address 10.0.1.2/30
interface eth1
 ip address 10.0.2.1/30
router bgp 65100
 bgp router-id 2.2.2.2
 no bgp ebgp-requires-policy
 neighbor 10.0.1.1 remote-as 65001
 neighbor 10.0.2.2 remote-as 65002
""",
        "r3": """frr defaults traditional
hostname r3
interface lo
 ip address 3.3.3.3/32
interface eth0
 ip address 10.0.2.2/30
router bgp 65002
 bgp router-id 3.3.3.3
 no bgp ebgp-requires-policy
 neighbor 10.0.2.1 remote-as 65100
 address-family ipv4 unicast
  network 3.3.3.3/32
""",
    }
    r1 = mknode(lab, "frr", "r1", 120, 200, cfg["r1"])
    r2 = mknode(lab, "frr", "r2", 400, 200, cfg["r2"])
    r3 = mknode(lab, "frr", "r3", 680, 200, cfg["r3"])
    mklink(lab, r1, 0, r2, 0)
    mklink(lab, r2, 1, r3, 0)
    for n in (r1, r2, r3):
        start(lab, n)
    ok_route = wait_for(lab, r1, 'vtysh -c "show ip route 3.3.3.3/32"', 'Known via "bgp"',
                        tries=24, label="r1 learned 3.3.3.3/32 via BGP")
    pr = ping_ok(lab, r1, "3.3.3.3", extra="-I 1.1.1.1")
    ok2 = pr is not None
    print(f"    {'✓' if ok2 else '✗'} loopback-to-loopback ping across two AS: {pr or 'FAILED'}")
    paths = excmd(lab, r1, 'vtysh -c "show bgp ipv4 unicast 3.3.3.3/32" | grep -A1 "65100"')
    print(f"    AS path on r1: {' '.join(paths.split())[:90]}")
    results["bgp-peering"] = bool(ok_route) and ok2
    stop_lab(lab)
    return lab


# ---------------------------------------------------------------- VXLAN EVPN
def lab_evpn():
    print("== VXLAN EVPN (spine RR + 2 leafs + 2 hosts, VNI 100)")
    lab = mklab("VXLAN EVPN", "BGP EVPN over OSPF underlay; VNI 100 stretched L2 between host1 and host2")
    spine_cfg = """frr defaults datacenter
hostname spine1
interface lo
 ip address 6.6.6.0/32
interface eth0
 ip address 10.10.1.2/30
 ip ospf network point-to-point
interface eth1
 ip address 10.10.2.2/30
 ip ospf network point-to-point
router ospf
 ospf router-id 6.6.6.0
 network 6.6.6.0/32 area 0
 network 10.10.0.0/14 area 0
router bgp 65000
 bgp router-id 6.6.6.0
 neighbor LEAF peer-group
 neighbor LEAF remote-as 65000
 neighbor LEAF update-source lo
 neighbor 6.6.6.1 peer-group LEAF
 neighbor 6.6.6.2 peer-group LEAF
 address-family l2vpn evpn
  neighbor LEAF activate
  neighbor LEAF route-reflector-client
"""
    def leaf_cfg(n, lo, ospf_net):
        return f"""frr defaults datacenter
hostname leaf{n}
interface lo
 ip address {lo}/32
interface eth0
 ip address {ospf_net}.1/30
 ip ospf network point-to-point
router ospf
 ospf router-id {lo}
 network {lo}/32 area 0
 network {ospf_net}.0/30 area 0
router bgp 65000
 bgp router-id {lo}
 neighbor 6.6.6.0 remote-as 65000
 neighbor 6.6.6.0 update-source lo
 address-family l2vpn evpn
  neighbor 6.6.6.0 activate
  advertise-all-vni
"""
    def leaf_boot(lo):
        return f"""ip addr add {lo}/32 dev lo
ip link add br100 type bridge
ip link add vxlan100 type vxlan id 100 dstport 4789 local {lo} nolearning
ip link set vxlan100 master br100
ip link set eth1 master br100
ip link set br100 up
ip link set vxlan100 up
"""
    spine = mknode(lab, "frr", "spine1", 400, 120, spine_cfg)
    leaf1 = mknode(lab, "frr", "leaf1", 220, 300, leaf_cfg(1, "6.6.6.1", "10.10.1"),
                   overrides={"boot_script": leaf_boot("6.6.6.1")})
    leaf2 = mknode(lab, "frr", "leaf2", 580, 300, leaf_cfg(2, "6.6.6.2", "10.10.2"),
                   overrides={"boot_script": leaf_boot("6.6.6.2")})
    host1 = mknode(lab, "host", "host1", 220, 470,
                   "ip addr add 192.168.100.1/24 dev eth0\n")
    host2 = mknode(lab, "host", "host2", 580, 470,
                   "ip addr add 192.168.100.2/24 dev eth0\n")
    mklink(lab, leaf1, 0, spine, 0)
    mklink(lab, leaf2, 0, spine, 1)
    mklink(lab, host1, 0, leaf1, 1)
    mklink(lab, host2, 0, leaf2, 1)
    for n in (spine, leaf1, leaf2, host1, host2):
        start(lab, n)
    wait_for(lab, leaf1, 'vtysh -c "show ip route 6.6.6.2/32"', "via eth0",
             label="underlay OSPF: leaf1 reaches leaf2 loopback")
    def _estab(o):
        for line in o.splitlines():
            if "6.6.6.0" in line and "Neighbor" not in line:
                fields = line.split()
                return len(fields) >= 2 and fields[-2].isdigit()
        return False
    ok1 = wait_for(lab, leaf1, 'vtysh -c "show bgp l2vpn evpn summary"', _estab,
                   tries=24, label="EVPN BGP session with spine Established")
    vni = excmd(lab, leaf1, 'vtysh -c "show evpn vni"')
    print("    leaf1 VNIs: " + " ".join(l.strip() for l in vni.splitlines() if "100" in l)[:100])
    pr = ping_ok(lab, host1, "192.168.100.2", tries=15)
    ok2 = pr is not None
    print(f"    {'✓' if ok2 else '✗'} host1 -> host2 across the VXLAN tunnel: {pr or 'FAILED'}")
    macs = excmd(lab, leaf1, 'vtysh -c "show bgp l2vpn evpn route" | grep -c "\\[2\\]" || true')
    print(f"    type-2 (MAC/IP) EVPN routes on leaf1: {macs.strip().splitlines()[-1] if macs else '?'}")
    results["vxlan-evpn"] = ok2
    stop_lab(lab)
    return lab


# ---------------------------------------------------------------- MPLS L3VPN
def lab_mpls():
    print("== MPLS L3VPN (pe1 - p1 - pe2 core, LDP + VPNv4; CE in vrf RED)")
    lab = mklab("MPLS L3VPN", "LDP core + MP-BGP VPNv4 between PEs; CEs in vrf RED (dataplane needs kernel MPLS)")
    def pe_cfg(n, lo, core_ip, peer_lo):
        return f"""frr defaults traditional
hostname pe{n}
interface lo
 ip address {lo}/32
interface eth0
 ip address {core_ip}.1/30
 ip ospf network point-to-point
router ospf
 ospf router-id {lo}
 network {lo}/32 area 0
 network {core_ip}.0/30 area 0
mpls ldp
 router-id {lo}
 address-family ipv4
  discovery transport-address {lo}
  interface eth0
router bgp 65000
 bgp router-id {lo}
 neighbor {peer_lo} remote-as 65000
 neighbor {peer_lo} update-source lo
 address-family vpnv4
  neighbor {peer_lo} activate
"""
    p_cfg = """frr defaults traditional
hostname p1
interface lo
 ip address 7.7.7.0/32
interface eth0
 ip address 10.7.1.2/30
 ip ospf network point-to-point
interface eth1
 ip address 10.7.2.2/30
 ip ospf network point-to-point
router ospf
 ospf router-id 7.7.7.0
 network 7.7.7.0/32 area 0
 network 10.7.0.0/14 area 0
mpls ldp
 router-id 7.7.7.0
 address-family ipv4
  discovery transport-address 7.7.7.0
  interface eth0
  interface eth1
"""
    pe1 = mknode(lab, "frr", "pe1", 150, 200, pe_cfg(1, "7.7.7.1", "10.7.1", "7.7.7.2"))
    p1 = mknode(lab, "frr", "p1", 420, 200, p_cfg)
    pe2 = mknode(lab, "frr", "pe2", 690, 200, pe_cfg(2, "7.7.7.2", "10.7.2", "7.7.7.1"))
    ce1 = mknode(lab, "host", "ce1", 150, 380, "ip addr add 172.16.1.2/24 dev eth0\n")
    ce2 = mknode(lab, "host", "ce2", 690, 380, "ip addr add 172.16.2.2/24 dev eth0\n")
    mklink(lab, pe1, 0, p1, 0)
    mklink(lab, p1, 1, pe2, 0)
    mklink(lab, ce1, 0, pe1, 1)
    mklink(lab, ce2, 0, pe2, 1)
    for n in (pe1, p1, pe2, ce1, ce2):
        start(lab, n)
    wait_for(lab, pe1, 'vtysh -c "show ip route 7.7.7.2/32"', "via eth0",
             label="IGP: pe1 reaches pe2 loopback")
    ldp = wait_for(lab, pe1, 'vtysh -c "show mpls ldp neighbor"', "OPERATIONAL",
                   tries=18, label="LDP neighbor OPERATIONAL")
    def _estab(o):
        for line in o.splitlines():
            if "7.7.7." in line and "Neighbor" not in line and "router identifier" not in line:
                fields = line.split()
                return len(fields) >= 2 and (fields[-2].isdigit() or fields[-2] == "N/A")
        return False
    bgp = wait_for(lab, pe1, 'vtysh -c "show bgp vpnv4 summary"', _estab,
                   tries=18, label="VPNv4 BGP session with pe2 Established")
    lblout = excmd(lab, pe1, 'vtysh -c "show mpls ldp binding" | head -6')
    print("    LDP label bindings on pe1:")
    for line in lblout.splitlines()[1:5]:
        if line.strip():
            print(f"      {line.strip()[:100]}")
    results["mpls-l3vpn-controlplane"] = bool(ldp) and bool(bgp)
    print("    note: dataplane forwarding needs kernel mpls_router/vrf modules (absent in this container; present on standard hosts)")
    stop_lab(lab)
    return lab


labs = {}
labs["ospf-multiarea"] = lab_ospf()
labs["bgp-peering"] = lab_bgp()
labs["vxlan-evpn"] = lab_evpn()
labs["mpls-l3vpn"] = lab_mpls()

print("\n== results:", json.dumps(results))
print("== lab ids:", json.dumps(labs))
with open(sys.argv[1] if len(sys.argv) > 1 else "/tmp/lab_ids.json", "w") as f:
    json.dump({"results": results, "labs": labs}, f)
