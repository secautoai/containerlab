# CCNP labs — step-by-step getting started tutorial

This tutorial takes you from a bare Linux machine to working through the CCNP lab curriculum.
Follow it top to bottom once; afterwards the per-lab `README.md` tutorials are all you need.

**Contents**

1. [Prepare the host](#1-prepare-the-host)
2. [Install Docker and containerlab](#2-install-docker-and-containerlab)
3. [Build the Cisco IOL images](#3-build-the-cisco-iol-images)
4. [Deploy your first lab (no Cisco image needed)](#4-deploy-your-first-lab-no-cisco-image-needed)
5. [The study workflow used by every lab](#5-the-study-workflow-used-by-every-lab)
6. [Working with IOL nodes](#6-working-with-iol-nodes)
7. [Troubleshooting](#7-troubleshooting)

---

## 1. Prepare the host

You need:

- A Linux environment with an **x86_64 CPU**: a physical machine, a cloud VM, or Windows + WSL2
  (Ubuntu). macOS on Apple Silicon **cannot** run IOL (ARM); use an x86 VM in the cloud instead.
- 4–8 GB of free RAM and ~10 GB of disk.
- Root/sudo access.

Verify the architecture:

```bash
uname -m        # must print: x86_64
```

## 2. Install Docker and containerlab

```bash
# Docker (official convenience script)
curl -fsSL https://get.docker.com | sudo sh
sudo usermod -aG docker $USER && newgrp docker

# containerlab (installs the `containerlab` and `clab` binaries)
curl -sL https://containerlab.dev/setup | sudo -E bash -s "all"
```

Verify:

```bash
docker version
containerlab version
```

Finally, get the labs. If you installed containerlab from a package, the examples are already at
`/etc/containerlab/lab-examples`; copy them so your changes are safe (or clone the repo):

```bash
mkdir -p ~/ccnp && cp -a /etc/containerlab/lab-examples/ccnp ~/ccnp/labs
# — or —
git clone https://github.com/srl-labs/containerlab && cp -a containerlab/lab-examples/ccnp ~/ccnp/labs

cd ~/ccnp/labs
./deploy.sh check      # tells you exactly what is still missing
```

## 3. Build the Cisco IOL images

> Skip this section for now if you don't yet have Cisco software access — Lab 00 runs on free
> images and teaches the whole workflow. Come back here before Lab 01.

IOL ("IOS On Linux", also known as IOU) is IOS-XE packaged as a native x86 binary — no VM, which is
why a 5-router lab fits in ~4 GB of RAM. Cisco distributes IOL inside the **CML** (Cisco Modeling
Labs) *refplat* ISO; a personal CML license or valid CCO entitlement gives you access. **Do not
download images from random sites** — apart from being illegal, they are a malware vector.

1. Mount the refplat ISO and copy the two binaries (names vary slightly by version):

    ```bash
    sudo mount -o loop refplat-*.iso /mnt
    ls /mnt/virl-base-images/
    # iol-xe-17.12.01/x86_64_crb_linux-adventerprisek9-ms      <- L3 router image
    # ioll2-xe-17.12.01/x86_64_crb_linux_l2-adventerprisek9-ms <- L2 switch image
    ```

2. Clone vrnetlab (the containerlab-tuned fork) and drop the binaries in:

    ```bash
    git clone https://github.com/srl-labs/vrnetlab && cd vrnetlab/cisco/iol
    cp /mnt/virl-base-images/iol-xe-*/x86_64_crb_linux-adventerprisek9-ms   cisco_iol-17.12.01.bin
    cp /mnt/virl-base-images/ioll2-xe-*/x86_64_crb_linux_l2-adventerprisek9-ms cisco_iol-L2-17.12.01.bin
    ```

3. Build both images:

    ```bash
    make docker-image
    docker images | grep cisco_iol
    # vrnetlab/cisco_iol   17.12.01     ...
    # vrnetlab/cisco_iol   L2-17.12.01  ...
    ```

4. Tell the labs about your tags **only if** they differ from the defaults above:

    ```bash
    export CCNP_IOL_IMAGE=vrnetlab/cisco_iol:<your-l3-tag>
    export CCNP_IOL_L2_IMAGE=vrnetlab/cisco_iol:L2-<your-l2-tag>
    ```

5. Re-run `./deploy.sh check` — everything should now be green.

## 4. Deploy your first lab (no Cisco image needed)

```bash
./deploy.sh deploy lab00
```

containerlab pulls the free FRR and multitool images, creates the virtual wiring, and prints a
table with every node's management IP. Inspect the lab at any time:

```bash
./deploy.sh status lab00        # containerlab inspect
sudo containerlab graph -t lab00-foundation/lab00-foundation.clab.yml   # topology web view :50080
```

Now open [lab00-foundation/README.md](lab00-foundation/README.md) and complete the tutorial — it
teaches connecting to nodes, verifying routing, captures with tcpdump, and persisting configs.
When you're done:

```bash
./deploy.sh destroy lab00
```

## 5. The study workflow used by every lab

Each lab follows the same loop:

1. **Deploy the baseline** — `./deploy.sh deploy labNN`. Interfaces and addressing are
   pre-configured (that's CCNA material); the protocol work is left to you.
2. **Open the lab's `README.md`** — read the topology + objectives, then work through the
   numbered tasks. Every task states *what* to achieve, *how* (commands), and *how to verify*
   (with expected output).
3. **Verify like the exam expects** — the tutorials lean on `show`/`debug` commands because
   ENCOR/ENARSI test interpretation of their output heavily.
4. **Save your work** — `./deploy.sh save labNN` (or `write memory` per node). Saved configs live
   in NVRAM inside the lab directory and survive destroy/deploy.
5. **Stuck? Compare with the solution** — every node in labs 01–10 has a reference config in
   `solutions/` (lab00 is baseline-only). Diff yours against it, or redeploy the whole lab
   pre-solved — the `reset` matters: startup configs only apply to a wiped lab:

    ```bash
    ./deploy.sh reset labNN               # wipe back to baseline
    ./deploy.sh deploy labNN --solved     # boots with the final configs
    ```

6. **Do the challenge tasks** — each lab ends with tasks that give you a goal but no commands.
   That's the level the exam (and real operations work) requires.
7. **Reset for a re-run** — `./deploy.sh reset labNN`. Repetition is what makes CLI knowledge
   stick; most students run each lab 2–3 times.

## 6. Working with IOL nodes

- **SSH (preferred):** `ssh admin@clab-ccnp-lab02-r1` — password `admin`. containerlab adds every
  node to `/etc/hosts`, so node names always resolve. `./deploy.sh ssh 2 r1` does the same.
- **Console via docker:** `docker attach clab-ccnp-lab02-r1` gives you the IOL console on the
  container's stdio (useful if you break SSH — e.g. in the security lab). Detach with
  `Ctrl-P Ctrl-Q` — **not** Ctrl-C, which would kill the container.
- **Interface naming:** `Ethernet0/0` is management (in VRF `clab-mgmt` — leave it alone).
  Data ports are `Ethernet0/1`–`Ethernet0/3`, then `Ethernet1/0`–`Ethernet1/3`, etc. The lab
  READMEs and topology files always tell you which port connects where.
- **Linux "PC" nodes:** `docker exec -it clab-ccnp-lab06-pc1 bash` — they run a network multitool
  image with `ping`, `traceroute`, `dig`, `curl`, `tcpdump`, `nc`, `iperf3`.
- **Packet captures** (great for STP/OSPF/NHRP study):

    ```bash
    # capture on r1's Ethernet0/1 (= eth1) from the host:
    sudo ip netns exec clab-ccnp-lab02-r1 tcpdump -nni eth1
    ```

- **First boot vs. later boots:** baseline `configs/` apply only on first boot. After a
  `write memory`, NVRAM wins on the next deploy. `./deploy.sh reset` clears NVRAM.

## 7. Troubleshooting

| Symptom | Fix |
| --- | --- |
| `deploy.sh check` says image missing | Build/tag the IOL images (§3) or export `CCNP_IOL_IMAGE`/`CCNP_IOL_L2_IMAGE` |
| Node stuck in `booting` / restart loop | Almost always RAM exhaustion — free memory or run one lab at a time; check `docker logs clab-<lab>-<node>` |
| `exec format error` in node logs | You are on an ARM host — IOL requires x86_64 |
| SSH refused right after deploy | IOL takes 30–60 s to boot (no docker healthcheck — the container shows plain `Up`); wait a minute and retry |
| Can't reach node names (`clab-...`) | Deploy adds them to `/etc/hosts`; use the mgmt IPs from `./deploy.sh status <lab>` otherwise |
| Lab behaves "already configured" after redeploy | NVRAM persisted your last session — `./deploy.sh reset <lab>` for a clean baseline |
| Need to see boot/console output | `docker attach <container>` (detach with `Ctrl-P Ctrl-Q`) or `docker logs <container>` |
| Weird L2 behavior on IOL-L2 | Check you built the node with the **L2** image (`type: L2` nodes); the L3 image has no `switchport` |

Happy labbing! Continue with [Lab 00](lab00-foundation/README.md), then follow the
[curriculum table](README.md#2-the-curriculum).
