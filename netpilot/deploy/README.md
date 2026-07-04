# Deploying NetPilot on a Linux lab host

`deploy.sh` installs NetPilot as a systemd service with the **bridge
datapath**, so the FRR and container node kinds boot for real (unlike
macOS, where only QEMU nodes run). It's what turns a plain Linux VM into a
working multi-vendor lab host.

## Quick start (on the lab host)

```bash
# with the agent enabled:
OPENROUTER_API_KEY=sk-or-… NETPILOT_AI_MODEL=deepseek/deepseek-v4-flash \
  netpilot/deploy/deploy.sh
```

Then browse to `http://<host>:8899` (default login **admin / admin** — change
it). The service is persistent (`systemctl enable`d) and survives reboots.

## What it configures

The lab host runs FRR **inside per-node network namespaces** with custom
socket/pid paths. Two host-level settings make that work, both applied by the
script:

- **Mask the system `frr` service** — otherwise the host's own FRR collides
  with the per-node instances on the shared `/run/frr` sockets.
- **FRR AppArmor profiles → complain mode** — Ubuntu ships *enforcing*
  profiles for the protocol daemons (`ospfd`, `bgpd`, `staticd`, …) that
  forbid the non-standard paths NetPilot uses; complain mode lets them run.
  (This is standard for containerlab-style FRR-in-netns.)

It also masks nothing else and touches no other AppArmor profiles.

## On the omnictl dev VM

The `netpilot-dev` VM (arm64, Postgres + Redis + FRR preinstalled) is the
intended target. The repo is shared at `/mnt/omni/containerlab`, so:

```bash
omnictl run netpilot-dev -- bash /mnt/omni/containerlab/netpilot/deploy/deploy.sh
```

Once it's listening, omnictl's service discovery publishes it as
`p8899.netpilot-dev.omni.local` (reachable via the omni HTTP proxy on
:50924 / :50925 when the omni daemon's proxy is running). A direct
alternative that always works:

```bash
ssh -F ~/.omni/lima/netpilot-dev/ssh.config -N -L 8899:127.0.0.1:8899 lima-netpilot-dev
# → http://localhost:8899
```

## Verified

On `netpilot-dev` (FRR 10.5, bridge datapath) the bundled OSPF multi-area
lab boots all four FRR routers, forms **FULL** adjacencies, propagates
inter-area routes, and passes an end-to-end ping across three areas
(`r1 → 4.4.4.4`, 0% loss).
