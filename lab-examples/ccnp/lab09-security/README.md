# Lab 09 — Infrastructure security: hardening, AAA, ACLs, uRPF, CoPP

**Goal:** secure the box, the management plane, the data plane and the control plane — in that
order: device hardening + VTY protection, local AAA, extended and object-group ACLs at the
untrusted edge, strict uRPF against spoofing (with a real spoofed-packet test), and CoPP to
protect the CPU from a live ping flood.

| | |
| --- | --- |
| Blueprint mapping | **ENCOR 5.1** (device access control: lines, local auth, AAA concepts), **5.2** (ACLs, CoPP), **ENARSI 3.1** (IOS AAA), **3.2** (uRPF, CoPP, IPv4 ACLs), 3.3 (concept items) |
| Nodes / RAM | 3× IOL + 1 host / ~2.3 GB |
| Estimated time | 2.5–3 h |

## Topology

```
 pc1 ---------- r1 ---------- r2 ---------- r3
 10.9.1.100    (edge)        (core)       (services)
 UNTRUSTED   10.9.1.0/24  10.9.12.0/30  10.9.23.0/30
                                          Lo1 10.9.3.1/24 = "server LAN"
                                          (also the trusted mgmt subnet;
                                           runs a small HTTP server)
```

Baseline: addressing + OSPF everywhere (pre-provisioned — today is about security, not routing).
pc1 can ping 10.9.3.1 out of the box; by the end, only exactly what policy allows will work.

```bash
./deploy.sh deploy lab09 && ./deploy.sh ssh 9 r1
```

> **Safety net:** if you ever lock yourself out of SSH, the IOL console is on the container's
> stdio: `docker attach clab-ccnp-lab09-r1` (detach with `Ctrl-P Ctrl-Q` — **not** Ctrl-C,
> which kills the container).

## Task 1 — device hardening & management-plane control

Two stages on r1 — hardening first, then the VTY access filter. Order matters: one of the
tests below only works *before* the filter exists.

**Stage A — hardening + brute-force protection:**

```
enable secret S3cure-Enable!
service password-encryption
banner motd ^
*** AUTHORIZED ACCESS ONLY - ccnp-lab09 r1 ***
^
login block-for 120 attempts 3 within 60
login on-failure log
login on-success log
```

- `enable secret` (hashed; type 9/scrypt on modern IOS) vs `enable password` (reversible) —
  `show run | include enable` and compare.
- `login block-for` = brute-force quiet period; watch it work **now, while pc1 can still open
  connections**: from pc1 `ssh wrong@10.9.1.1` three times with bad passwords →
  `%SEC_LOGIN-2-SYSTEM_MSG` + a 120 s quiet period (`show login`). During the quiet period
  **all** logins are refused — including yours — unless you whitelist trusted sources, so add
  that exception right away (it references the ACL you're about to build):

    ```
    login quiet-mode access-class ACL-MGMT
    ```

**Stage B — restrict who may even open a VTY session:**

```
ip access-list standard ACL-MGMT
 permit 10.9.3.0 0.0.0.255
 permit 172.20.20.0 0.0.0.255
 deny   any log
!
line vty 0 4
 access-class ACL-MGMT in vrf-also
 exec-timeout 10 0
 transport input ssh
```

- Two lab-specific details: `172.20.20.0/24` is the **containerlab management network** (this
  lab's topology pins it to that value; adjust the ACL if you changed it), and `vrf-also` is
  required because that management SSH arrives via the `clab-mgmt` VRF — omit it and you're
  managing r1 by console only (try it, read `show users`, put it back).
- Test the deny: `ssh admin@10.9.1.1` from **pc1** (untrusted) → connection **refused** +
  `%SEC-6-IPACCESSLOGS` log entry. From r3 (`ssh -l admin 10.9.12.1` sourced from Lo1:
  `ip ssh source-interface Loopback1` first) → works.
- Connect the two stages for the exam: with the access-class in place, pc1 can no longer even
  *attempt* passwords (refused at TCP connect — refused connections don't count as login
  failures), so `login block-for` now only guards against brute force from the *permitted*
  management sources. Defense in depth, each layer with a distinct job.

## Task 2 — local AAA

Turn the "new model" on and keep working credentials (open a **second** SSH session before you
commit — rule one of AAA changes):

```
aaa new-model
aaa authentication login default local
aaa authorization exec default local
username netadmin privilege 15 secret NetAdmin123!
```

Verify: new SSH login as `netadmin` from r3 works; `show users` shows both accounts. Know for
the exam how this scales up: method lists (`aaa authentication login MGMT group tacacs+ local`
— TACACS+ first, local as *fallback*), TACACS+ (TCP/49, per-command authorization, full
encryption) vs RADIUS (UDP/1812-1813, only password encrypted, no command authz) — ENARSI 3.1
loves that comparison. There's no TACACS+ server in this lab; the *fallback logic* is the part
you can and did practice.

## Task 3 — extended ACL at the untrusted edge

Policy for the client LAN: web (80/443) and ping **to the server LAN only**, nothing else. On
r1:

```
ip access-list extended ACL-EDGE-IN
 permit icmp 10.9.1.0 0.0.0.255 10.9.3.0 0.0.0.255 echo
 permit tcp 10.9.1.0 0.0.0.255 10.9.3.0 0.0.0.255 eq www
 permit tcp 10.9.1.0 0.0.0.255 10.9.3.0 0.0.0.255 eq 443
 deny   ip any any log
!
interface Ethernet0/1
 ip access-group ACL-EDGE-IN in
```

Test from pc1, predicting each result first:

```bash
ping -c 3 10.9.3.1          # works (icmp echo permitted)
nc -zv 10.9.3.1 80          # works (TCP handshake to the http server)
nc -zv 10.9.3.1 22          # blocked
traceroute -n 10.9.3.1      # blocked (UDP high ports) - and each probe logs
ping -c 3 10.9.1.1          # blocked! echo to the GATEWAY isn't in policy
```

On r1: `show access-lists ACL-EDGE-IN` (hit counters per ACE) and the `%SEC-6-IPACCESSLOGP`
messages. Fundamentals being tested: implicit deny, top-down first-match, `log` keyword cost
(punts to CPU — fine in labs, careful in production), and placement doctrine — **extended ACLs
close to the source**, standard ACLs close to the destination.

## Task 4 — object-group ACLs (how adults write policy)

Same policy, maintainable form — on r1:

```
object-group network OG-CLIENTS
 10.9.1.0 255.255.255.0
object-group network OG-SERVERS
 10.9.3.0 255.255.255.0
object-group service OG-WEB-ICMP
 tcp eq www
 tcp eq 443
 icmp echo
!
no ip access-list extended ACL-EDGE-IN
ip access-list extended ACL-EDGE-IN
 permit object-group OG-WEB-ICMP object-group OG-CLIENTS object-group OG-SERVERS
 deny ip any any log
```

We delete and recreate the whole ACL rather than editing ACEs in place — a common trap: if you
only removed the three permits (`no 10`/`no 20`/`no 30`) and appended the new permit, it would
be added at sequence **50**, *behind* the surviving `deny ip any any log` at 40, and top-down
first-match would blackhole the LAN. (The interface reference survives the delete/recreate; an
ACL that is momentarily empty permits nothing you care about here, but on production gear you'd
build the new ACL under a new name and swap `ip access-group` atomically.) Re-run the pc1
tests — identical behavior to task 3. Now "add a server subnet" = one line in `OG-SERVERS`
instead of ACE surgery. `show object-group` + `show access-lists` to see the expansion.

## Task 5 — uRPF: drop spoofed sources

Strict-mode uRPF on the edge: accept a packet only if the *source* is reachable **back out the
same interface**:

```
! r1
interface Ethernet0/1
 ip verify unicast source reachable-via rx
```

One order-of-operations subtlety first: on IOS/IOS-XE the **input ACL is evaluated before
uRPF** (Cisco's uRPF configuration guide spells the order out). Your task-4 edge ACL would
therefore eat the spoofed packets before uRPF ever sees them — and the uRPF counters we want to
watch would stay at zero. Take the ACL off for this experiment:

```
r1(config)# interface Ethernet0/1
r1(config-if)# no ip access-group ACL-EDGE-IN in
```

Now actually spoof from pc1 (multitool ships the tooling):

```bash
docker exec -it clab-ccnp-lab09-pc1 bash
ip addr add 172.99.99.99/32 dev eth1          # bogus source
ping -c 3 -I 172.99.99.99 10.9.3.1            # spoofed pings - dropped by uRPF
ping -c 3 10.9.3.1                            # control: legit source still passes
```

On r1 the spoofed packets die at the uRPF check (no route back to 172.99.99.99 via e0/1), while
the legitimately-sourced control ping sails through:

```
r1# show ip interface Ethernet0/1 | include verify|suppressed
  IP verify source reachable-via RX
  ... verification drops
r1# show ip traffic | include RPF
         0 format errors, ... 3 unicast RPF, ...
```

Strict (`rx`) vs loose (`any` — route exists via *any* interface, for multihomed/asymmetric
edges) is the ENARSI distinction; strict belongs at single-homed edges exactly like this one.
Clean up: `ip addr del 172.99.99.99/32 dev eth1` on pc1, and **re-apply the edge ACL** on r1
(`ip access-group ACL-EDGE-IN in`) — in production the two coexist fine (ACL first, uRPF
second); we removed it only so the drops would land on the uRPF counters you were studying.

## Task 6 — CoPP: protect the control plane

Transit packets are CEF-switched in hardware/fast path; packets **to** the router (SSH, OSPF,
your pings at its own IPs) hit the CPU. Police the ICMP slice of that on r2:

```
ip access-list extended ACL-COPP-ICMP
 permit icmp any any
!
class-map match-all CM-COPP-ICMP
 match access-group name ACL-COPP-ICMP
!
policy-map PM-COPP
 class CM-COPP-ICMP
  police 32000 conform-action transmit exceed-action drop
!
control-plane
 service-policy input PM-COPP
```

One prerequisite: pc1's flood must first *transit r1*, and your edge ACL only permits traffic
to the server LAN — packets aimed at r2's own address would die at `deny ip any any log`. Open
a surgical hole for the test (and note how the sequence number places it before the deny):

```
r1(config)# ip access-list extended ACL-EDGE-IN
r1(config-ext-nacl)# 15 permit icmp 10.9.1.0 0.0.0.255 host 10.9.12.2 echo
```

Now attack from pc1: `ping -f -s 1200 10.9.12.2` (flood mode). Watch the policer earn its keep:

```
r2# show policy-map control-plane
 Control Plane
  Service-policy input: PM-COPP
    Class-map: CM-COPP-ICMP (match-all)
      ... police: cir 32000 bps
        conformed ... packets; actions: transmit
        exceeded  ... packets; actions: drop        <- climbing during the flood
```

Two must-know facts: CoPP applies **only to punted traffic** (prove it: pc1 → `ping -f
10.9.3.1` *through* r2 is not policed), and a careless CoPP that classifies OSPF/LDP/SSH into a
tight policer is a self-inflicted outage — production policies whitelist routing protocols
first (`match protocol` / dedicated classes), then police the rest.

## Task 7 — the concept corner (no CLI)

Round out the blueprint items this topology can't demonstrate — one paragraph each, out loud:
802.1X vs MAB vs WebAuth (who talks EAP, who needs RADIUS); IPv6 first-hop security (RA Guard,
DHCPv6 Guard, ND inspection — switch features); TrustSec/MACsec (SGTs vs 802.1AE link
encryption); and ENCOR v1.2's **Zero Trust / SASE** additions (identity-based, continuously
verified access vs perimeter trust; SASE = cloud-delivered SD-WAN + security stack).

## Challenges

1. Make the edge ACL **time-based**: web to the servers only 08:00–18:00 weekdays
   (`time-range`, then verify with `clock set` and `show time-range`).
2. Create `helpdesk` (privilege 5) allowed to run `show` commands but not `configure` —
   `privilege exec level 5 show`, `aaa authorization commands` — and prove it.
3. Extend PM-COPP with a class that polices **telnet/SSH from non-mgmt sources** to 8 kbps and
   a default class policing everything unclassified. What must you *never* police to zero here,
   given r2's neighbors?
4. Add uRPF **loose mode** on r2's core links and explain why strict mode there could drop
   legitimate traffic if the lab had asymmetric paths.
5. Replace the shared `admin` account entirely: per-user SSH **public-key** auth
   (`ip ssh pubkey-chain`) for `netadmin`, password login disabled for it.

<details><summary>Solution reference</summary>

Final configs in [`solutions/`](solutions/) (tasks 1–6);
`./deploy.sh reset 9 && ./deploy.sh deploy 9 --solved` boots the hardened end state.
</details>

**Next:** [Lab 10 — Automation & programmability](../lab10-automation/README.md)
