#!/usr/bin/env python3
"""Lab 10 / task 2b - NETCONF <edit-config>: set an interface description.

Usage:
    python3 netconf_set_description.py --interface Ethernet0/1 \
        --description "CONFIGURED-BY-NETCONF" [--host clab-ccnp-lab10-r1]

Writes to the *running* datastore through the ietf-interfaces YANG model, then
reads it back so you can see the round trip. Verify on the router afterwards:
    show run interface Ethernet0/1
"""

import argparse
from xml.sax.saxutils import escape

from ncclient import manager

EDIT_TEMPLATE = """
<config>
  <interfaces xmlns="urn:ietf:params:xml:ns:yang:ietf-interfaces">
    <interface>
      <name>{name}</name>
      <description>{description}</description>
    </interface>
  </interfaces>
</config>
"""


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--host", default="clab-ccnp-lab10-r1")
    ap.add_argument("--user", default="admin")
    ap.add_argument("--password", default="admin")
    ap.add_argument("--interface", default="Ethernet0/1")
    ap.add_argument("--description", default="CONFIGURED-BY-NETCONF")
    args = ap.parse_args()

    with manager.connect(
        host=args.host,
        port=830,
        username=args.user,
        password=args.password,
        hostkey_verify=False,
        device_params={"name": "iosxe"},
    ) as m:
        # escape() keeps user input from breaking (or injecting into) the XML
        payload = EDIT_TEMPLATE.format(
            name=escape(args.interface), description=escape(args.description)
        )
        reply = m.edit_config(target="running", config=payload)
        print(f"edit-config rpc-reply ok={reply.ok}")

        # read-back: same model, config datastore
        confirm = m.get_config(
            source="running",
            filter=(
                "subtree",
                '<interfaces xmlns="urn:ietf:params:xml:ns:yang:'
                f"ietf-interfaces\"><interface><name>{escape(args.interface)}"
                "</name></interface></interfaces>",
            ),
        )
        print(confirm.data_xml)


if __name__ == "__main__":
    main()
