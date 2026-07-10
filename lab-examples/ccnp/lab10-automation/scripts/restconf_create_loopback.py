#!/usr/bin/env python3
"""Lab 10 / task 4 - RESTCONF PUT: create Loopback100 from JSON.

Usage:
    python3 restconf_create_loopback.py [--host clab-ccnp-lab10-r1] [--delete]

RESTCONF = HTTPS + YANG-modeled JSON. This script PUTs a full interface
resource at:
    /restconf/data/ietf-interfaces:interfaces/interface=Loopback100
then GETs it back. --delete removes it again (HTTP DELETE - same URL).
Verify on the router: show ip interface brief | include Loopback100
"""

import argparse
import json

import requests
import urllib3

urllib3.disable_warnings()  # lab only: self-signed HTTPS certificate

HEADERS = {
    "Accept": "application/yang-data+json",
    "Content-Type": "application/yang-data+json",
}

LOOPBACK = {
    "ietf-interfaces:interface": {
        "name": "Loopback100",
        "description": "CONFIGURED-BY-RESTCONF",
        "type": "iana-if-type:softwareLoopback",
        "enabled": True,
        "ietf-ip:ipv4": {
            "address": [{"ip": "192.0.2.100", "netmask": "255.255.255.255"}]
        },
    }
}


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--host", default="clab-ccnp-lab10-r1")
    ap.add_argument("--user", default="admin")
    ap.add_argument("--password", default="admin")
    ap.add_argument("--delete", action="store_true", help="remove Loopback100")
    args = ap.parse_args()

    url = (
        f"https://{args.host}/restconf/data/"
        "ietf-interfaces:interfaces/interface=Loopback100"
    )
    auth = (args.user, args.password)

    if args.delete:
        resp = requests.delete(url, headers=HEADERS, auth=auth, verify=False)
        print(f"DELETE {url}\n -> HTTP {resp.status_code}")
        return

    resp = requests.put(
        url, headers=HEADERS, auth=auth, verify=False, data=json.dumps(LOOPBACK)
    )
    # 201 = created, 204 = modified existing
    print(f"PUT {url}\n -> HTTP {resp.status_code}")
    resp.raise_for_status()

    read_back = requests.get(url, headers=HEADERS, auth=auth, verify=False)
    print(json.dumps(read_back.json(), indent=2))


if __name__ == "__main__":
    main()
