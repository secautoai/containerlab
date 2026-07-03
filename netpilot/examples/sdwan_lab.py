#!/usr/bin/env python3
"""Build the SD-WAN dual-path failover lab on the verify server (stdlib only).

    host-a ── edge-a ══(primary eth1/eth1, ospf cost 10)══ edge-b ── host-b
                  ╚═══(backup  eth2/eth2, ospf cost 50)═══╝

Edges are Alpine+FRR (image 3.22-dns). Traffic host-a→host-b prefers the
primary path; suspending it fails traffic over to the backup in seconds.
"""
import json
import sys
import urllib.request

import os
BASE = os.environ.get("NETPILOT_BASE", "http://127.0.0.1:8090")


def api(method, path, body=None, timeout=120):
    data = json.dumps(body).encode() if body is not None else None
    req = urllib.request.Request(BASE + path, data=data,
                                 headers={"Content-Type": "application/json"} if data else {},
                                 method=method)
    with urllib.request.urlopen(req, timeout=timeout) as r:
        payload = r.read()
        return json.loads(payload) if payload else None


SERIAL = """sed -i 's|^ttyS0.*|ttyS0::respawn:/bin/login -f root|' /etc/inittab
kill -HUP 1"""

FRR_START = "sed -i 's/^ospfd=no/ospfd=yes/;s/^ldpd=no/ldpd=yes/' /etc/frr/daemons\nrc-service frr start"

MPLS_KERNEL = """modprobe mpls_router; modprobe mpls_iptunnel
sysctl -qw net.mpls.platform_labels=1048575
sysctl -qw net.mpls.conf.eth1.input=1 net.mpls.conf.lo.input=1"""


def cc(hostname, files, cmds):
    out = ["#cloud-config", f"hostname: {hostname}"]
    if files:
        out.append("write_files:")
        for path, content in files:
            out.append(f"  - path: {path}")
            out.append("    content: |")
            for ln in content.splitlines():
                out.append("      " + ln)
    out.append("runcmd:")
    for c in cmds:
        out.append("  - |")
        for ln in c.splitlines():
            out.append("    " + ln)
    return "\n".join(out) + "\n"


def edge_frr(name, rid, lan_net, primary_ip, backup_ip):
    # 1s/5s hellos: SD-WAN-grade failover instead of OSPF's default 40s.
    # LDP runs over the primary link (kernel mpls modules loaded in runcmd).
    return f"""frr defaults traditional
hostname {name}
interface eth1
 ip address {primary_ip}/30
 ip ospf network point-to-point
 ip ospf cost 10
 ip ospf hello-interval 1
 ip ospf dead-interval 5
interface eth2
 ip address {backup_ip}/30
 ip ospf network point-to-point
 ip ospf cost 50
 ip ospf hello-interval 1
 ip ospf dead-interval 5
interface eth3
 ip address {lan_net}.1/24
router ospf
 ospf router-id {rid}
 network 10.0.12.0/30 area 0
 network 10.0.21.0/30 area 0
 network {lan_net}.0/24 area 0
mpls ldp
 router-id {rid}
 address-family ipv4
  discovery transport-address {primary_ip}
  interface eth1
 exit-address-family
"""


CONFIGS = {
    "edge-a": cc("edge-a", [("/etc/frr/frr.conf", edge_frr("edge-a", "1.1.1.1", "10.1.0", "10.0.12.1", "10.0.21.1"))],
                 [SERIAL, MPLS_KERNEL, "sysctl -w net.ipv4.ip_forward=1", "for i in 1 2 3; do ip link set eth$i up; done", FRR_START]),
    "edge-b": cc("edge-b", [("/etc/frr/frr.conf", edge_frr("edge-b", "2.2.2.2", "10.2.0", "10.0.12.2", "10.0.21.2"))],
                 [SERIAL, MPLS_KERNEL, "sysctl -w net.ipv4.ip_forward=1", "for i in 1 2 3; do ip link set eth$i up; done", FRR_START]),
    "host-a": cc("host-a", [], [SERIAL, "ip addr add 10.1.0.10/24 dev eth1\nip link set eth1 up\nip route add default via 10.1.0.1"]),
    "host-b": cc("host-b", [], [SERIAL, "ip addr add 10.2.0.10/24 dev eth1\nip link set eth1 up\nip route add default via 10.2.0.1"]),
}

POS = {"edge-a": (330, 300), "edge-b": (650, 300), "host-a": (150, 300), "host-b": (830, 300)}


def main():
    lab = api("POST", "/api/labs", {
        "name": "sdwan-failover",
        "description": "SD-WAN style dual-path failover: primary/backup WAN between two edges, OSPF steers, suspend primary to fail over"})
    lab_id = lab["id"]
    nodes = {}
    for name, cfg in CONFIGS.items():
        x, y = POS[name]
        n = api("POST", f"/api/labs/{lab_id}/nodes", {
            "template": "linux", "name": name, "image": "3.22-dns",
            "ram_mb": 384, "x": x, "y": y, "startup_config": cfg,
        })
        nodes[name] = n["id"]
    links = [
        ("edge-a", 1, "edge-b", 1, "primary 10.0.12.0/30"),
        ("edge-a", 2, "edge-b", 2, "backup 10.0.21.0/30"),
        ("host-a", 1, "edge-a", 3, "10.1.0.0/24"),
        ("host-b", 1, "edge-b", 3, "10.2.0.0/24"),
    ]
    link_ids = {}
    for a, ai, b, bi, label in links:
        l = api("POST", f"/api/labs/{lab_id}/links", {
            "a": {"kind": "node", "node": nodes[a], "iface": ai},
            "b": {"kind": "node", "node": nodes[b], "iface": bi},
            "label": label,
        })
        link_ids[label] = l["id"]
    api("POST", f"/api/labs/{lab_id}/start")
    print(json.dumps({"lab": lab_id, "nodes": nodes, "links": link_ids}))


if __name__ == "__main__":
    main()
