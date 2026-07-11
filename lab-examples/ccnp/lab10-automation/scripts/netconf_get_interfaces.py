#!/usr/bin/env python3
"""Lab 10 / task 2 - NETCONF <get>: interface state from the ietf-interfaces model.

Usage:
    python3 netconf_get_interfaces.py [--host clab-ccnp-lab10-r1] [--host ...]

Talks NETCONF (SSH port 830) to each router, asks for the operational state of
the standard ietf-interfaces YANG model, and prints name / admin status /
IPv4 address per interface. Read the code top to bottom - every ENCOR 6.x
concept (transport, YANG model, XML filter, XPath-ish subtree) is in here.
"""

import argparse
import sys
import xml.etree.ElementTree as ET

from ncclient import manager

# subtree filter: "only the interfaces container of this YANG model, please"
FILTER = """
<filter>
  <interfaces xmlns="urn:ietf:params:xml:ns:yang:ietf-interfaces"/>
</filter>
"""

NS = {
    "if": "urn:ietf:params:xml:ns:yang:ietf-interfaces",
    "ip": "urn:ietf:params:xml:ns:yang:ietf-ip",
}


def dump_interfaces(host: str, user: str, password: str) -> None:
    print(f"\n=== {host} (NETCONF :830) ===")
    with manager.connect(
        host=host,
        port=830,
        username=user,
        password=password,
        hostkey_verify=False,       # lab only! production pins host keys
        device_params={"name": "iosxe"},
    ) as m:
        # every capability the router advertised in its <hello> is a YANG model:
        models = [c for c in m.server_capabilities if "ietf-interfaces" in c]
        print(f"advertised ietf-interfaces capability: {len(models)} match(es)")

        reply = m.get(FILTER)
        root = ET.fromstring(reply.data_xml)

        for iface in root.iterfind(".//if:interface", NS):
            name = iface.findtext("if:name", default="?", namespaces=NS)
            enabled = iface.findtext("if:enabled", default="?", namespaces=NS)
            # anchor to the ipv4 container: ietf-ip's ipv6 addresses share the
            # same namespace and a bare .//ip:address would match those too
            addr = iface.find("ip:ipv4/ip:address/ip:ip", NS)
            ipv4 = addr.text if addr is not None else "-"
            print(f"  {name:<16} enabled={enabled:<5} ipv4={ipv4}")


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument(
        "--host",
        action="append",
        default=None,
        help="router hostname/IP (repeatable). Default: both lab10 routers",
    )
    ap.add_argument("--user", default="admin")
    ap.add_argument("--password", default="admin")
    args = ap.parse_args()

    hosts = args.host or ["clab-ccnp-lab10-r1", "clab-ccnp-lab10-r2"]
    failed = 0
    for host in hosts:
        try:
            dump_interfaces(host, args.user, args.password)
        except Exception as exc:  # noqa: BLE001 - lab script, show the raw cause
            print(f"!! {host}: {exc}", file=sys.stderr)
            failed += 1  # keep going - the other routers may still answer
    return 1 if failed else 0


if __name__ == "__main__":
    raise SystemExit(main())
