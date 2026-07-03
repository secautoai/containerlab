#!/usr/bin/env python3
"""NetPilot DNSSEC + anycast DNS lab orchestrator (stdlib only).

Topology (all Alpine QEMU nodes, `linux` template, image version 3.22-dns —
an Alpine 3.22 cloud image with unbound, bind, bind-tools, and frr
preinstalled):

    resolver ── core ── root-a   (anycast 10.53.53.53/32 on lo, OSPF)
                 ├──── root-b    (anycast 10.53.53.53/32 on lo, OSPF, AXFR secondary)
                 ├──── tld       (lab. zone)
                 └──── auth      (example.lab. zone)

Zones: . (root, on root-a/b) -> lab. (tld) -> example.lab. (auth), all
DNSSEC-signed at runtime; resolver runs unbound with the lab root DS as
trust anchor. Anycast preference: core's link to root-b has OSPF cost 50.

Usage against a running netpilot server (default http://127.0.0.1:8090,
override with NETPILOT_BASE):

    dns_lab.py setup      create the lab, boot it, write dns-ids.json
    dns_lab.py poll       wait for nodes to come up
    dns_lab.py reconfig   write reboot-proof /etc/local.d boot scripts
    dns_lab.py sign       DNSSEC-sign the chain, start daemons, trust anchor
    dns_lab.py test       validated lookups (AD flag), chain + NXDOMAIN
    dns_lab.py failover break|restore   anycast failover via OSPF
    dns_lab.py exec <node> <cmd> [timeout]

State (node ids) is kept in dns-ids.json next to this script, or set
NETPILOT_DNS_IDS.
"""
import json
import os
import sys
import time
import urllib.request
import urllib.error

BASE = os.environ.get("NETPILOT_BASE", "http://127.0.0.1:8090")
IDS_FILE = os.environ.get(
    "NETPILOT_DNS_IDS",
    os.path.join(os.path.dirname(os.path.abspath(__file__)), "dns-ids.json"),
)

INITTAB = """sed -i 's|^ttyS0.*|ttyS0::respawn:/bin/sh|' /etc/inittab
grep -q '^ttyS0' /etc/inittab || echo 'ttyS0::respawn:/bin/sh' >> /etc/inittab
kill -HUP 1"""


def api(method, path, body=None, timeout=130):
    data = json.dumps(body).encode() if body is not None else None
    req = urllib.request.Request(BASE + path, data=data,
                                 headers={"Content-Type": "application/json"} if data else {},
                                 method=method)
    try:
        with urllib.request.urlopen(req, timeout=timeout) as r:
            payload = r.read()
            return r.status, json.loads(payload) if payload else None
    except urllib.error.HTTPError as e:
        return e.code, e.read().decode(errors="replace")


def ids():
    with open(IDS_FILE) as f:
        return json.load(f)


def exec_node(lab, node, command, timeout_s=30):
    st, r = api("POST", f"/api/labs/{lab}/nodes/{node}/exec",
                {"command": command, "timeout_s": timeout_s}, timeout=timeout_s + 25)
    out = r.get("output", "") if isinstance(r, dict) else str(r)
    return st, out


def x(name, command, timeout_s=30, show=True):
    i = ids()
    st, out = exec_node(i["lab"], i[name], command, timeout_s)
    if show:
        print(f"── {name} $ {command[:90]}{'…' if len(command) > 90 else ''}  [http {st}]")
        print(out.rstrip()[:3000])
    return out


# ─── cloud-init configs ───

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


def frr_conf(rid, networks, extra=""):
    nets = "\n".join(f" network {n} area 0" for n in networks)
    return f"frr defaults traditional\nhostname {rid}\n{extra}router ospf\n ospf router-id {rid_ip(rid)}\n{nets}\n"


def rid_ip(rid):
    return {"core": "10.255.0.1", "root-a": "10.255.0.11", "root-b": "10.255.0.12"}[rid]


def named_conf(server_id, zones, extra_opts=""):
    return (
        'options {\n  directory "/var/bind";\n  listen-on { any; };\n  listen-on-v6 { none; };\n'
        f'  recursion no;\n  dnssec-validation no;\n  server-id "{server_id}";\n{extra_opts}}};\n'
        + "\n".join(zones) + "\n"
    )


ROOT_ZONE = """$TTL 300
.                    IN SOA a.root-servers.lab. admin.lab. 1 3600 900 604800 300
.                    IN NS  a.root-servers.lab.
a.root-servers.lab.  IN A   10.53.53.53
lab.                 IN NS  ns1.lab.
ns1.lab.             IN A   10.0.4.2
"""

LAB_ZONE = """$TTL 300
lab.                 IN SOA ns1.lab. admin.lab. 1 3600 900 604800 300
lab.                 IN NS  ns1.lab.
ns1.lab.             IN A   10.0.4.2
example.lab.         IN NS  ns1.example.lab.
ns1.example.lab.     IN A   10.0.5.2
"""

EXAMPLE_ZONE = """$TTL 300
example.lab.         IN SOA ns1.example.lab. admin.example.lab. 1 3600 900 604800 300
example.lab.         IN NS  ns1.example.lab.
ns1.example.lab.     IN A   10.0.5.2
www.example.lab.     IN A   10.99.0.80
note.example.lab.    IN TXT "DNSSEC anycast lab"
"""

ROOT_HINTS = """.                   3600000 IN NS a.root-servers.lab.
a.root-servers.lab. 3600000 IN A  10.53.53.53
"""

UNBOUND_CONF = """server:
  interface: 0.0.0.0
  access-control: 0.0.0.0/0 allow
  root-hints: "/etc/unbound/root.hints"
  do-ip6: no
  qname-minimisation: no
"""


def ip_cmds(addr, gw=None, extra=""):
    lines = ["ip addr flush dev eth0 2>/dev/null || true",
             f"ip addr add {addr} dev eth0", "ip link set eth0 up"]
    if gw:
        lines.append(f"ip route add default via {gw}")
    if extra:
        lines.append(extra)
    return "\n".join(lines)


FRR_START = "sed -i 's/^ospfd=no/ospfd=yes/' /etc/frr/daemons\nrc-service frr start"


def node_configs():
    return {
        "core": cc("core", [
            ("/etc/frr/frr.conf", frr_conf("core",
                ["10.0.1.0/30", "10.0.2.0/30", "10.0.3.0/30", "10.0.4.0/30", "10.0.5.0/30"],
                extra="interface eth2\n ip ospf cost 50\n")),
        ], [
            INITTAB,
            "sysctl -w net.ipv4.ip_forward=1\n"
            + "\n".join(
                f"ip addr flush dev eth{i} 2>/dev/null || true\nip addr add 10.0.{i+1}.1/30 dev eth{i}\nip link set eth{i} up"
                for i in range(5))
            + "\n" + FRR_START,
        ]),
        "resolver": cc("resolver", [
            ("/etc/unbound/unbound.conf", UNBOUND_CONF),
            ("/etc/unbound/root.hints", ROOT_HINTS),
        ], [
            INITTAB,
            ip_cmds("10.0.1.2/30", "10.0.1.1"),
        ]),
        "root-a": cc("root-a", [
            ("/etc/frr/frr.conf", frr_conf("root-a", ["10.0.2.0/30", "10.53.53.53/32"])),
            ("/etc/bind/named.conf", named_conf("root-a", [
                'zone "." { type primary; file "/etc/bind/zones/root.zone.signed"; allow-transfer { 10.0.3.2; }; also-notify { 10.0.3.2; }; };',
            ])),
            ("/etc/bind/zones/root.zone", ROOT_ZONE),
        ], [
            INITTAB,
            ip_cmds("10.0.2.2/30", "10.0.2.1"),
            FRR_START,
        ]),
        "root-b": cc("root-b", [
            ("/etc/frr/frr.conf", frr_conf("root-b", ["10.0.3.0/30", "10.53.53.53/32"])),
            ("/etc/bind/named.conf", named_conf("root-b", [
                'zone "." { type secondary; primaries { 10.0.2.2; }; file "/var/bind/root.zone.db"; };',
            ])),
        ], [
            INITTAB,
            ip_cmds("10.0.3.2/30", "10.0.3.1"),
            FRR_START,
        ]),
        "tld": cc("tld", [
            ("/etc/bind/named.conf", named_conf("tld", [
                'zone "lab" { type primary; file "/etc/bind/zones/lab.zone.signed"; };',
            ])),
            ("/etc/bind/zones/lab.zone", LAB_ZONE),
        ], [
            INITTAB,
            ip_cmds("10.0.4.2/30", "10.0.4.1"),
        ]),
        "auth": cc("auth", [
            ("/etc/bind/named.conf", named_conf("auth", [
                'zone "example.lab" { type primary; file "/etc/bind/zones/example.lab.zone.signed"; };',
            ])),
            ("/etc/bind/zones/example.lab.zone", EXAMPLE_ZONE),
        ], [
            INITTAB,
            ip_cmds("10.0.5.2/30", "10.0.5.1"),
        ]),
    }


NODE_POS = {"core": (470, 330), "resolver": (470, 90), "root-a": (180, 250),
            "root-b": (180, 470), "tld": (760, 250), "auth": (760, 470)}


def cmd_setup():
    st, lab = api("POST", "/api/labs", {
        "name": "DNSSEC Anycast DNS",
        "description": "root/TLD/auth chain signed with DNSSEC; anycast root (10.53.53.53) x2 via OSPF"})
    assert st == 200, (st, lab)
    lab_id = lab["id"]
    cfgs = node_configs()
    nodes = {}
    for name, cfg in cfgs.items():
        x_, y_ = NODE_POS[name]
        body = {"template": "linux", "name": name, "image": "3.22-dns",
                "ram_mb": 384, "x": x_, "y": y_, "startup_config": cfg}
        if name == "core":
            body["interfaces"] = 6
        st, n = api("POST", f"/api/labs/{lab_id}/nodes", body)
        assert st == 200, (name, st, n)
        nodes[name] = n["node"]["id"] if "node" in n else n["id"]
    order = ["resolver", "root-a", "root-b", "tld", "auth"]
    for i, leaf in enumerate(order):
        st, l = api("POST", f"/api/labs/{lab_id}/links",
                    {"a": {"kind": "node", "node": nodes["core"], "iface": i},
                     "b": {"kind": "node", "node": nodes[leaf], "iface": 0},
                     "label": f"10.0.{i+1}.0/30"})
        assert st == 200, (leaf, st, l)
    st, res = api("POST", f"/api/labs/{lab_id}/start")
    assert st == 200, (st, res)
    out = {"lab": lab_id, **nodes}
    with open(IDS_FILE, "w") as f:
        json.dump(out, f)
    print(json.dumps(out, indent=1))


def cmd_poll(console_only=False):
    i = ids()
    names = [k for k in i if k != "lab"]
    ready = {n: False for n in names}
    deadline = time.time() + 1200
    while time.time() < deadline:
        for n in names:
            if not ready[n]:
                # console alive AND (unless console_only) IPs configured
                st, out = exec_node(i["lab"], i[n], "echo NP-READY && ip -4 addr show", 12)
                if st == 200 and "NP-READY" in out and (console_only or "10.0." in out):
                    ready[n] = True
                    print(f"[{time.strftime('%H:%M:%S')}] {n} ready", flush=True)
        if all(ready.values()):
            print("ALL-CONSOLES-READY")
            return
        time.sleep(15)
    print(f"TIMEOUT: {ready}")
    sys.exit(1)


SIGN = ("cd /etc/bind/zones && "
        "dnssec-keygen -a ECDSAP256SHA256 -f KSK {zone} >/dev/null 2>&1 && "
        "dnssec-keygen -a ECDSAP256SHA256 {zone} >/dev/null 2>&1 && "
        "dnssec-signzone -S -K . -o {zone} -f {file}.signed {file} && "
        "chown -R named:named /etc/bind/zones")


def ds_from(out, zone):
    for ln in out.splitlines():
        ln = " ".join(ln.split())  # collapse tabs/multi-space — tabs break console echo
        if ln.startswith(zone + " ") and " DS " in ln:
            return ln
    return None


def cmd_sign():
    print("═══ 1. sign example.lab. on auth")
    x("auth", SIGN.format(zone="example.lab", file="example.lab.zone"), 60)
    out = x("auth", "cat /etc/bind/zones/dsset-example.lab.")
    ds_example = ds_from(out, "example.lab.")
    assert ds_example, "no DS for example.lab"
    x("auth", "rc-service named start", 30)

    print("═══ 2. add DS to lab., sign on tld")
    x("tld", f"echo '{ds_example}' >> /etc/bind/zones/lab.zone")
    x("tld", SIGN.format(zone="lab", file="lab.zone"), 60)
    out = x("tld", "cat /etc/bind/zones/dsset-lab.")
    ds_lab = ds_from(out, "lab.")
    assert ds_lab, "no DS for lab"
    x("tld", "rc-service named start", 30)

    print("═══ 3. add DS to root, sign on root-a, go anycast-live")
    x("root-a", f"echo '{ds_lab}' >> /etc/bind/zones/root.zone")
    x("root-a", SIGN.format(zone=".", file="root.zone"), 60)
    out = x("root-a", "cat /etc/bind/zones/dsset-.")
    ds_root = ds_from(out, ".")
    assert ds_root, "no DS for root"
    x("root-a", "rc-service named start && sleep 1 && ip addr add 10.53.53.53/32 dev lo", 30)

    print("═══ 4. root-b: secondary AXFR, then anycast-live")
    x("root-b", "rc-service named start", 30)
    got = False
    for _ in range(8):
        out = x("root-b", "dig @127.0.0.1 . SOA +norec +time=2 +tries=1 | grep -E 'status|SOA'", 20)
        if "NOERROR" in out and "root-servers" in out:
            got = True
            break
        x("root-b", "rc-service named restart", 25, show=False)
        time.sleep(6)
    assert got, "root-b never transferred the root zone"
    x("root-b", "ip addr add 10.53.53.53/32 dev lo")

    print("═══ 5. resolver: install trust anchor, start unbound")
    ta = ds_root.replace(" IN DS ", " DS ")
    x("resolver", f"echo 'trust-anchor: \"{ta}\"' >> /etc/unbound/unbound.conf")
    x("resolver", "rc-service unbound start", 30)
    print("SIGN-DONE — root trust anchor:", ta)


def cmd_test():
    print("═══ core: route to anycast (expect via 10.0.2.2 = root-a)")
    x("core", "ip route show 10.53.53.53")
    print("═══ direct anycast query + NSID")
    x("resolver", "dig +nsid @10.53.53.53 . SOA +norec +time=3 | grep -E 'NSID|status|SOA' ", 25)
    print("═══ validated resolution (expect: NOERROR, flags contain ad, A 10.99.0.80)")
    x("resolver", "dig @127.0.0.1 www.example.lab A +dnssec +time=4 | grep -E 'status|flags|10\\.99\\.0\\.80'", 30)
    print("═══ validated TXT")
    x("resolver", "dig @127.0.0.1 note.example.lab TXT +dnssec +time=4 | grep -E 'status|flags|anycast'", 30)
    print("═══ validated negative answer (NXDOMAIN with ad)")
    x("resolver", "dig @127.0.0.1 doesnotexist.example.lab A +dnssec +time=4 | grep -E 'status|flags'", 30)
    print("═══ chain check: root DNSKEY via anycast, validated")
    x("resolver", "dig @127.0.0.1 . DNSKEY +dnssec +time=4 | grep -E 'status|flags' ", 30)


def cmd_failover(step):
    if step == "break":
        print("═══ withdrawing anycast from root-a (ip addr del on lo)")
        x("root-a", "ip addr del 10.53.53.53/32 dev lo")
        time.sleep(4)
        x("core", "ip route show 10.53.53.53")
        for attempt in range(3):
            out = x("resolver", "dig +nsid @10.53.53.53 . SOA +norec +time=3 +tries=1 | grep -E 'NSID|status'", 25)
            if "root-b" in out:
                print("FAILOVER-OK: anycast now served by root-b")
                return
            time.sleep(4)
        print("FAILOVER-INCONCLUSIVE")
        sys.exit(1)
    else:
        print("═══ restoring anycast on root-a")
        x("root-a", "ip addr add 10.53.53.53/32 dev lo")
        time.sleep(4)
        x("core", "ip route show 10.53.53.53")
        x("resolver", "dig +nsid @10.53.53.53 . SOA +norec +time=3 +tries=1 | grep -E 'NSID|status'", 25)


RECONFIG = {
    "core": ["sysctl -w net.ipv4.ip_forward=1"]
    + [f"ip addr add 10.0.{i+1}.1/30 dev eth{i} 2>/dev/null; ip link set eth{i} up" for i in range(5)]
    + ["rc-service frr start"],
    "resolver": ["ip addr add 10.0.1.2/30 dev eth0 2>/dev/null; ip link set eth0 up",
                 "ip route add default via 10.0.1.1 2>/dev/null",
                 "rc-service unbound start"],
    "root-a": ["ip addr add 10.0.2.2/30 dev eth0 2>/dev/null; ip link set eth0 up",
               "ip route add default via 10.0.2.1 2>/dev/null",
               "rc-service frr start", "rc-service named start",
               "ip addr add 10.53.53.53/32 dev lo 2>/dev/null"],
    "root-b": ["ip addr add 10.0.3.2/30 dev eth0 2>/dev/null; ip link set eth0 up",
               "ip route add default via 10.0.3.1 2>/dev/null",
               "rc-service frr start", "rc-service named start",
               "ip addr add 10.53.53.53/32 dev lo 2>/dev/null"],
    "tld": ["ip addr add 10.0.4.2/30 dev eth0 2>/dev/null; ip link set eth0 up",
            "ip route add default via 10.0.4.1 2>/dev/null", "rc-service named start"],
    "auth": ["ip addr add 10.0.5.2/30 dev eth0 2>/dev/null; ip link set eth0 up",
             "ip route add default via 10.0.5.1 2>/dev/null", "rc-service named start"],
}


def cmd_reconfig():
    """Write per-node /etc/local.d boot script (reboot-proof) and run it now."""
    for name, lines in RECONFIG.items():
        quoted = " ".join(f"'{ln}'" for ln in ["#!/bin/sh"] + lines)
        cmd = (f"mkdir -p /etc/local.d && printf '%s\\n' {quoted} > /etc/local.d/netlab.start"
               " && chmod +x /etc/local.d/netlab.start && rc-update add local default >/dev/null 2>&1;"
               " sh /etc/local.d/netlab.start >/dev/null 2>&1; echo RECONFIG-OK")
        out = x(name, cmd, 45, show=False)
        print(f"{name}: {'RECONFIG-OK' if 'RECONFIG-OK' in out else 'FAILED: ' + out[-200:]}")


if __name__ == "__main__":
    cmd = sys.argv[1]
    if cmd == "setup":
        cmd_setup()
    elif cmd == "poll":
        cmd_poll(console_only=(len(sys.argv) > 2 and sys.argv[2] == "console"))
    elif cmd == "reconfig":
        cmd_reconfig()
    elif cmd == "sign":
        cmd_sign()
    elif cmd == "test":
        cmd_test()
    elif cmd == "failover":
        cmd_failover(sys.argv[2])
    elif cmd == "exec":
        x(sys.argv[2], sys.argv[3], int(sys.argv[4]) if len(sys.argv) > 4 else 30)
    else:
        sys.exit("unknown cmd")
