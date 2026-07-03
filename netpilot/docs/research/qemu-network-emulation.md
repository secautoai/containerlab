# QEMU Network OS Emulation — Technical Reference (research)

Sources: hellt/vrnetlab (common/vrnetlab.py + launch.py), GNS3 server qemu_vm.py + ubridge,
GNS3 registry appliances, EVE-NG cookbooks, QEMU docs.

## 1. Invocation pattern (vrnetlab/containerlab style)
qemu-system-x86_64 -enable-kvm(-if /dev/kvm) -display none -machine pc -m 4096 -cpu host
  -smp 2,sockets=1,cores=2,threads=1
  -monitor tcp / -qmp unix:/run/lab/n1.qmp,server,nowait
  -chardev socket,id=serial0,host=::,port=50NN,server=on,wait=off,telnet=on -serial chardev:serial0
  -drive if=ide,file=overlay.qcow2
- Ports: vrnetlab 50xx serial, 40xx monitor. Machine `pc` safe default; q35 only for PCIe images.
- -uuid fixed per node (PAN licensing); -smbios type=1 = config channel for Nokia vSIM / identity for vJunos.
- TCG fallback 10-50x slower; XRv9k/vJunosEvolved unusable without KVM.
- State-resume: -incoming defer + QMP migrate file: for near-instant boot of slow NOSes.
- QMP verbs needed: query-status, system_powerdown, quit, set_link, device_add/del, blockdev-snapshot-sync, object-add (filter-dump).

## 2. NIC wiring
- Models: virtio-net-pci (modern), e1000 (IOSv/vQFX/older vEOS), vmxnet3 (IOS-XE in vrnetlab), virtio speed=10000.
- i440FX ~30 slots; pci-bridge for more (26 NICs per bridge). NIC ordering = interface naming; NIC0 = mgmt
  almost everywhere; XRv9k reserves NIC0-2. Provision full fixed NIC set at boot (no hot-add on NOS);
  model link down via QMP set_link off.
- Backends:
  - tap + linux bridge (EVE-NG): -netdev tap,ifname=X,script=no,downscript=no,vhost=on
  - tap + tc mirred redirect (containerlab): transparent to LACP/STP/LLDP
  - UDP tunnels (GNS3): -netdev socket,id=x,udp=127.0.0.1:B,localaddr=127.0.0.1:A — 1 frame = 1 datagram,
    no root, hot-wirable, cross-host. Modern: -netdev dgram. GNS3 uses ubridge userspace bridge to splice.
  - TCP/stream netdev with reconnect; vhost-user (DPDK, overkill); slirp user mode w/ hostfwd for rootless mgmt.
- MACs: locally administered, derive from (lab,node,iface), STABLE across reboots (licensing).
  vrnetlab 0C:00:xx:xx:xx:II.

## 3. Vendor matrix (RAM MB / vCPU / NIC model / console / bootstrap)
- IOSv: 512/1/e1000(<=16)/serial / config disk hdb (ios_config.txt in FAT img)
- IOSvL2: 1024/1/e1000(16)/serial / same
- CSR1000v: 3-4G/1-2/vmxnet3-or-virtio/serial / CD-ROM ISO iosxe_config.txt (CVAC), wait CVAC-4-CONFIG_DONE
- Cat8000v: 4G+/1-4/vmxnet3/serial / same CVAC ISO
- XRv9k: 16-24G/4/virtio(NIC0-2 reserved mgmt/ctrl/dev)/3 serial ports / cpu +ssse3,+sse4.1,+sse4.2, -machine smm=off
- vEOS: 2-4G/1-2/e1000/serial / Aboot ISO cdrom + zerotouch cancel; Ma1=NIC0
- vSRX3: 4G/2/virtio/serial login: / CD-ROM config.iso juniper.conf; cpu SandyBridge+vmx flags (nested)
- vMX: dual VM (VCP 2G + VFP 4G), internal bridge int_cp
- vJunos-switch: 5G/4/virtio(57)/serial / smbios product=VM-VEX + USB config.img (qemu-xhci+usb-storage)
- vJunos-router: 5G/4/virtio / smbios product=VM-VMX,family=lab
- vJunosEvolved: 8G/4/virtio / SMBIOS trio Bochs + chassis serial string; >=24.2 needs OVMF
- Nokia vSIM: 4G+/2/virtio/serial / ALL config via smbios type=1 TIMOS: line (address, license tftp, primary-config, system-base-mac, slot, chassis, card); license mandatory
- FortiGate: 2G/1/virtio(12)/serial / serial CLI admin/empty; needs empty 30G log disk; MAC-tied eval license
- PA-VM: 6G/2 cpu host,level=9/virtio(25, NIC0 mgmt-only)/serial / very slow boot + autocommit poll; fixed -uuid; bootstrap disk vol 'bootstrap'
- CHR: 256/1/virtio speed=10000(31)/serial / admin/empty, forced pw change on ROS7
- VyOS: 1G/1/virtio(10)/serial / vyos/vyos; cloud-init NoCloud ISO supported
- Linux cloud: 512+/1/virtio/serial console=ttyS0 / cloud-init NoCloud: ISO volid cidata w/ user-data+meta-data
Cross-cutting: genisoimage -volid cidata; expect-over-serial fallback; two-phase install boot for CSR/XRv9k;
nested virt for vSRX.

## 4. Console
- Serial telnet: QEMU speaks IAC. Raw alternative: -serial unix:/path,server,nowait (no telnet framing) — best to bridge to WebSockets ourselves. Handle IAC if telnet: escape 0xFF, answer DO/WILL.
- VNC: -vnc 127.0.0.1:D (5900+D); native websocket: -vnc :0,websocket=5700 (noVNC direct).
- Patterns: EVE=Guacamole; GNS3=xterm.js<->WS<->server-side telnet client. Multiplex N viewers per console.

## 5. Overlays
qemu-img create -f qcow2 -F qcow2 -b base.qcow2 overlay.qcow2 (-F mandatory now).
Wipe=delete overlay. Persist=qemu-img commit / convert. Rebase -b new -F qcow2 (-u metadata-only).
Internal snapshots: qemu-img snapshot -c/-a/-d; live savevm/loadvm (qcow2-only, blocks).
Layout: images/ read-only + labs/<lab>/<node>/disk.qcow2 + config media regenerated per start.

## 6. Linux plumbing
ip tuntap add dev X mode tap; ip link set X up mtu 9500; ip link add BR type bridge; ip link set X master BR
ethtool -K tap tx off rx off tso off gso off gro off (checksum offload)
Bridge pitfalls: 01:80:C2:00:00:0X reserved MACs not forwarded — STP/PAUSE/LACP kernel-restricted even with
group_fwd_mask (LLDP unlockable via 0x4000). Fixes: tc mirred cross-redirect or UDP socket links.
bridge-nf-call-iptables=0; multicast_snooping=0 for OSPF/VRRP.
Privileges: CAP_NET_ADMIN, /dev/net/tun, /dev/kvm. qemu-bridge-helper setuid alternative.
Mgmt NAT: bridge + dnsmasq + nft masquerade + per-node dnat port maps. Rootless alt: slirp hostfwd.
Capture: tcpdump on tap; QEMU-native backend-agnostic: -object filter-dump,id=c0,netdev=p01,file=x.pcap
(live add/remove via QMP object-add/del); tc mirror to capture tap.

## Implementation checklist (Rust orchestrator)
1. Per-node spec: arch/machine/cpu flags/RAM/SMP/ordered disks/ordered NICs(model,MAC,backend)/serial count/smbios/config media/extra args.
2. Deterministic MAC/UUID from (lab,node,index).
3. QMP-first control; console chardev->WebSocket (xterm.js); -vnc websocket for GUI nodes.
4. Links abstraction over 3 datapaths: UDP dgram pair (default, hot-wirable), tap+tc-redirect (fidelity), tap+bridge (multipoint). set_link for carrier; filter-dump for capture.
5. Overlay lifecycle: create-on-start, wipe-on-reset, commit for save-as-image, migrate-to-file boot cache.
6. Bootstrap engine: ISO builder (CVAC/juniper.conf/cloud-init), raw config-disk builder, SMBIOS composer, expect-over-console fallback.
