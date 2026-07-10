# Lab 10 — Automation & programmability: NETCONF/YANG, RESTCONF, JSON, EEM

**Goal:** stop typing and start programming the network. Enable IOS-XE's model-driven APIs,
then drive both routers from Python: read state over **NETCONF** (ncclient + ietf-interfaces
YANG), write config over NETCONF and **RESTCONF** (requests + YANG-modeled JSON), and finish
with an **EEM** applet that fixes a failure by itself. This is ENCOR domain 6 made concrete.

| | |
| --- | --- |
| Blueprint mapping | **ENCOR 6.x** (Python components/interpretation, JSON, YANG, NETCONF/RESTCONF APIs, EEM), **4.x** (NETCONF/RESTCONF for assurance) — Catalyst Center/SD-WAN APIs and AI/ML items remain reading topics |
| Nodes / RAM | 2× IOL / ~1.5 GB |
| Estimated time | 2–3 h |

## Topology & setup

```
   r1 ---------------- r2        10.10.12.0/30, OSPF pre-provisioned
      e0/1        e0/1           Lo0 10.255.255.1 / .2
```

The Python side runs on the **containerlab host** (node names resolve via `/etc/hosts`):

```bash
./deploy.sh deploy lab10
cd lab10-automation/scripts
python3 -m venv .venv && source .venv/bin/activate
pip install -r requirements.txt
```

## Task 1 — enable NETCONF and look at the machinery

On **both** routers:

```
netconf-yang
```

That's it — but it starts a whole subsystem (give it ~1–2 min on first enable). Explore:

```
r1# show netconf-yang status
netconf-yang: enabled
netconf-yang ssh port: 830
...
r1# show netconf-yang datastores
r1# show netconf-yang sessions
```

Anatomy for the exam: NETCONF = **SSH subsystem on TCP/830**, XML-encoded RPCs
(`<get>`, `<get-config>`, `<edit-config>`, `<lock>`), **datastores** (running, candidate,
startup), data modeled in **YANG**. See the raw handshake once in your life:

```bash
ssh -p 830 admin@clab-ccnp-lab10-r1 -s netconf
# a wall of XML: the <hello> with hundreds of <capability> URIs - each one a YANG model.
# Type ]]>]]> ... actually just Ctrl+C out; ncclient will speak the protocol for us.
```

## Task 2 — read the network with Python + NETCONF

Read [`scripts/netconf_get_interfaces.py`](scripts/netconf_get_interfaces.py) *before* running
it — the exam shows you scripts like this and asks what they do. Then:

```bash
python3 netconf_get_interfaces.py
=== clab-ccnp-lab10-r1 (NETCONF :830) ===
advertised ietf-interfaces capability: 1 match(es)
  Ethernet0/0      enabled=true  ipv4=172.20.20.x
  Ethernet0/1      enabled=true  ipv4=10.10.12.1
  Loopback0        enabled=true  ipv4=10.255.255.1
=== clab-ccnp-lab10-r2 (NETCONF :830) ===
  ...
```

Things to be able to explain: the **subtree filter** (why the reply isn't megabytes of
everything), the **namespace** `urn:ietf:params:xml:ns:yang:ietf-interfaces` (the YANG model's
identity), `hostkey_verify=False` (lab shortcut — name the production fix), and that one script
just inventoried N devices identically — the entire point of model-driven management.

Now write config the same way:

```bash
python3 netconf_set_description.py --interface Ethernet0/1 --description "CONFIGURED-BY-NETCONF"
```

Verify in the CLI (`show run interface e0/1`) — and notice the script *read its change back*
via `<get-config>`: closed-loop automation in 40 lines.

## Task 3 — enable RESTCONF

On **both** routers:

```
restconf
ip http secure-server
ip http authentication local
```

RESTCONF = same YANG data, but **HTTPS + REST verbs + JSON** (or XML). URL grammar you must be
able to parse cold: `https://<host>/restconf/data/<yang-module>:<container>/<list>=<key>`.
First contact with plain curl:

```bash
curl -sk -u admin:admin \
  -H "Accept: application/yang-data+json" \
  https://clab-ccnp-lab10-r1/restconf/data/ietf-interfaces:interfaces/interface=Loopback0 | python3 -m json.tool
```

Read the JSON that comes back and map every key to what you saw in XML in task 2 — same model,
different encoding. That mapping (YANG ↔ XML ↔ JSON) is exactly what ENCOR 6.x tests.

## Task 4 — write config with RESTCONF

```bash
python3 restconf_create_loopback.py
PUT https://clab-ccnp-lab10-r1/restconf/data/ietf-interfaces:interfaces/interface=Loopback100
 -> HTTP 201
{ ... "name": "Loopback100", "description": "CONFIGURED-BY-RESTCONF", ... }
```

Verify: `show ip interface brief | include Loopback100` → `192.0.2.100`. Know your verbs +
status codes: `GET` read / `PUT` replace (201 created, 204 changed) / `PATCH` merge / `DELETE`
remove; 401 = bad auth, 404 = wrong path/model, 409 = conflict. Try the idempotence experiment:
run the script twice — second run returns 204, config unchanged. That's declarative intent vs
a CLI script that would happily paste duplicate lines.

## Task 5 — EEM: the router automates itself

Embedded Event Manager reacts to events *on-box*. Auto-heal the loopback you just created — on
r1:

```
event manager applet AUTO-RECOVER-LO100
 event syslog pattern "%LINK-5-CHANGED: Interface Loopback100.*administratively down"
 action 10 cli command "enable"
 action 20 cli command "configure terminal"
 action 30 cli command "interface Loopback100"
 action 40 cli command "no shutdown"
 action 50 syslog msg "EEM: Loopback100 re-enabled automatically"
```

Now try to break it:

```
r1(config)# interface Loopback100
r1(config-if)# shutdown
r1(config-if)#
%LINK-5-CHANGED: Interface Loopback100, changed state to administratively down
%HA_EM-6-LOG: AUTO-RECOVER-LO100: EEM: Loopback100 re-enabled automatically
%LINK-3-UPDOWN: Interface Loopback100, changed state to up
```

The router overruled you. Inspect with `show event manager policy registered` and
`debug event manager action cli`. EEM event sources worth memorizing: syslog patterns, `event
cli` (command triggers), timers/cron, SNMP OID thresholds, interface counters, object tracking.

## Task 6 — read JSON/YANG like the exam does

No CLI — three quick reps with the artifacts you just produced:

1. In the RESTCONF JSON from task 4, identify: the module (`ietf-interfaces`), the list key
   (`name`), a leaf of type boolean (`enabled`), and a nested container from an *augmenting*
   model (`ietf-ip:ipv4`). Augmentation = one YANG model extending another.
2. `python3 -c "import json;print(json.load(open('/dev/stdin')))"` — pipe the curl from task 3
   into it and note that JSON objects/arrays map to Python dicts/lists — ENCOR asks this
   directly ("what Python type is X?").
3. Skim the model tree: `show netconf-yang datastores` then
   `curl -sk -u admin:admin https://clab-ccnp-lab10-r1/restconf/data/netconf-state/capabilities ...`
   — every capability URI = one more model you could automate against.

## Challenges

1. Extend `netconf_get_interfaces.py` to also print interface **counters** (they live in the
   same model's `statistics` container) and flag any interface with input errors > 0.
2. Write `restconf_get_ospf_neighbors.py` against the operational model
   `Cisco-IOSXE-ospf-oper` (discover the exact path via the capabilities list) and make it
   exit non-zero when r1 has no FULL neighbor — congratulations, you've written a monitoring
   probe.
3. Make the NETCONF description script **transactional**: `--dry-run` flag that validates but
   doesn't apply (hint: candidate datastore isn't enabled here — what *can* you do instead?
   read-modify-verify with rollback via a saved snapshot).
4. Add an EEM applet that runs `write memory` every time anyone leaves config mode
   (`event syslog pattern "%SYS-5-CONFIG_I"`) — then argue whether that's a good idea in
   production (hint: rollback story).
5. Point [Ansible or Nornir] at both routers using the same credentials and re-implement task
   2's inventory — compare the effort against raw ncclient and articulate when you'd choose an
   orchestration layer (ENCOR 6.x "orchestration tools" item).

<details><summary>Solution reference</summary>

Final configs in [`solutions/`](solutions/) (APIs on, Lo100 present, EEM applet);
`./deploy.sh deploy 10 --solved` boots the end state. The scripts in `scripts/` are already
the reference implementation.
</details>

**You made it.** That's the full curriculum — back to the [study plan](../README.md#2-the-curriculum)
for the next pass, or start mixing topologies into your own scenarios.
