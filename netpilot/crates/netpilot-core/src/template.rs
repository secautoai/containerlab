//! Device templates: how to boot a given network OS family under QEMU,
//! and the image library that backs them.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::{CoreError, Result};
use crate::lab::ConsoleKind;

/// NIC model handed to QEMU (`-device <model>`), per vendor requirements.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum NicModel {
    #[default]
    VirtioNetPci,
    E1000,
    E1000e,
    Vmxnet3,
    Rtl8139,
}

impl NicModel {
    pub fn qemu_name(&self) -> &'static str {
        match self {
            NicModel::VirtioNetPci => "virtio-net-pci",
            NicModel::E1000 => "e1000",
            NicModel::E1000e => "e1000e",
            NicModel::Vmxnet3 => "vmxnet3",
            NicModel::Rtl8139 => "rtl8139",
        }
    }
}

/// Disk bus the image is attached to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum DiskBus {
    #[default]
    Virtio,
    Ide,
    Sata,
    Scsi,
}

/// How the startup configuration is delivered to the guest on first boot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum ConfigDelivery {
    /// No automated config injection.
    #[default]
    None,
    /// Attach an ISO9660 CD-ROM containing the config file (named inside).
    CdromIso { filename: String },
    /// Attach a small FAT disk image containing the config file.
    FatDisk { filename: String },
    /// cloud-init style: ISO labeled cidata with user-data/meta-data.
    CloudInit,
}

/// QEMU boot recipe for a device family.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QemuSpec {
    /// qemu system binary architecture, e.g. "x86_64", "aarch64".
    #[serde(default = "default_arch")]
    pub arch: String,
    #[serde(default)]
    pub nic_model: NicModel,
    #[serde(default)]
    pub disk_bus: DiskBus,
    /// Machine type, e.g. "pc", "q35". Empty = QEMU default.
    #[serde(default)]
    pub machine: String,
    /// Extra raw QEMU arguments appended to the command line.
    #[serde(default)]
    pub extra_args: Vec<String>,
    /// Required CPU flags / model, e.g. "host" (KVM) or "qemu64".
    #[serde(default)]
    pub cpu_model: String,
    /// Config injection mechanism.
    #[serde(default)]
    pub config_delivery: ConfigDelivery,
    /// First NIC is reserved for management (dedicated to mgmt network).
    #[serde(default)]
    pub mgmt_nic: bool,
}

fn default_arch() -> String {
    "x86_64".into()
}

impl Default for QemuSpec {
    fn default() -> Self {
        Self {
            arch: default_arch(),
            nic_model: NicModel::default(),
            disk_bus: DiskBus::default(),
            machine: String::new(),
            extra_args: Vec::new(),
            cpu_model: String::new(),
            config_delivery: ConfigDelivery::None,
            mgmt_nic: false,
        }
    }
}

/// How nodes of a template are executed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum NodeKind {
    /// Full VM under QEMU (needs a disk image in the library).
    #[default]
    Qemu,
    /// Docker container wired into the lab via veth pairs.
    Container,
    /// Native process(es) in a Linux network namespace — no image at all
    /// (FRR from the host package, plain Linux shells).
    Netns,
}

/// Recipe for container-kind templates.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ContainerSpec {
    /// Image reference (pulled on first start when absent locally, or
    /// provided via BYOI docker-load upload).
    pub image: String,
    /// Optional command override.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub cmd: Vec<String>,
    /// Environment variables.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub env: Vec<String>,
    /// Run privileged (most NOS containers require it).
    #[serde(default)]
    pub privileged: bool,
    /// Command exec'd for the interactive console (default: sh).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub console_cmd: Option<String>,
}

/// Recipe for netns-kind templates.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NetnsSpec {
    /// Service to launch inside the namespace: "frr" starts the FRR
    /// daemon suite from the host package; empty = bare endpoint.
    #[serde(default)]
    pub service: String,
}

/// A device template: defaults for creating nodes plus the QEMU recipe.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeTemplate {
    /// Stable identifier, e.g. "vyos", "iosv", "ceos".
    pub id: String,
    /// Display name, e.g. "VyOS Router".
    pub name: String,
    /// Vendor label for grouping in the UI palette.
    #[serde(default)]
    pub vendor: String,
    /// UI icon key.
    #[serde(default)]
    pub icon: String,
    /// Default vCPUs.
    pub cpus: u32,
    /// Default RAM MiB.
    pub ram_mb: u32,
    /// Default number of NICs.
    pub interfaces: u32,
    /// Max NICs supported by the platform.
    #[serde(default = "default_max_ifaces")]
    pub max_interfaces: u32,
    /// Interface naming pattern with `{i}` placeholder (e.g. "Gi0/{i}").
    #[serde(default = "default_iface_pattern")]
    pub iface_pattern: String,
    #[serde(default)]
    pub console: ConsoleKind,
    /// Execution backend for this template.
    #[serde(default)]
    pub kind: NodeKind,
    /// QEMU boot recipe.
    #[serde(default)]
    pub qemu: QemuSpec,
    /// Container recipe (kind = container).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub container: Option<ContainerSpec>,
    /// Netns recipe (kind = netns).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub netns: Option<NetnsSpec>,
    /// Free-form notes shown in the UI (image requirements, credentials...).
    #[serde(default)]
    pub notes: String,
    /// Platform configuration primer distilled from the vendor's official
    /// documentation: config format, interface naming, minimal routed +
    /// protocol examples, save/commit semantics. Consumed by the AI agent
    /// (list_templates) so generated startup configs follow vendor syntax.
    #[serde(default)]
    pub config_guide: String,
    /// CLI command that prints the full running configuration on the
    /// serial console (used by "export running config"). None = platform
    /// has no meaningful config dump.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub export_command: Option<String>,
}

fn default_max_ifaces() -> u32 {
    32
}
fn default_iface_pattern() -> String {
    "eth{i}".into()
}

/// Per-platform "print the running config" command for config export.
fn export_command_for(template_id: &str) -> Option<String> {
    let cmd = match template_id {
        "iosv" | "iosvl2" | "csr1000v" | "cat8000v" | "xrv9k" | "veos" => "show running-config",
        "vyos" | "frr" => "show configuration commands",
        "vsrx" | "vjunos-switch" => "show configuration | display set | no-more",
        "fortigate" => "show full-configuration",
        "chr" => "/export",
        _ => return None,
    };
    Some(cmd.into())
}

/// The built-in template catalog. Users can add more via YAML files in the
/// data directory; those override built-ins with the same id.
/// Per-platform configuration primer, distilled from the vendor's official
/// documentation (FRR docs, Nokia SR Linux docs, Cisco IOS/IOS-XE/IOS-XR
/// config guides, Arista EOS manual, Juniper Junos docs, VyOS docs, OpenWrt
/// UCI reference, FortiOS/PAN-OS CLI references, MikroTik RouterOS docs).
/// Keep these short and operational: format, interface naming, a minimal
/// routed example, protocol snippet, and save/commit semantics.
fn config_guide_for(template_id: &str) -> &'static str {
    match template_id {
        "frr" => "\
Startup config = complete /etc/frr/frr.conf (integrated config, docs.frrouting.org).
Interfaces are Linux eth0..N; no 'no shutdown' needed. Live CLI: vtysh.
  frr defaults traditional
  hostname r1
  interface eth0
   ip address 10.0.0.1/30
   ip ospf network point-to-point
  router ospf
   ospf router-id 1.1.1.1
   network 10.0.0.0/30 area 0
BGP: 'router bgp 65001' + 'neighbor X remote-as Y' + address-family blocks.
Persist from live CLI: vtysh -c 'write memory'.",
        "host" => "\
Startup config = shell script run at boot (BusyBox/ash, iproute2).
  ip addr add 10.0.0.10/24 dev eth0
  ip link set eth0 up
  ip route add default via 10.0.0.1
No routing daemons; static routes only.",
        "srlinux" => "\
SR Linux CLI (sr_cli) is transactional (documentation.nokia.com/srlinux).
Enter 'enter candidate', apply flat set commands, then 'commit stay'.
Data ports are ethernet-1/1..N; every L3 config lives on a subinterface
bound into a network-instance:
  set / interface ethernet-1/1 admin-state enable
  set / interface ethernet-1/1 subinterface 0 ipv4 admin-state enable
  set / interface ethernet-1/1 subinterface 0 ipv4 address 10.0.0.1/30
  set / network-instance default type default
  set / network-instance default interface ethernet-1/1.0
  set / network-instance default protocols ospf instance main version ospf-v2
  set / network-instance default protocols ospf instance main router-id 1.1.1.1
  set / network-instance default protocols ospf instance main area 0.0.0.0 interface ethernet-1/1.0 interface-type point-to-point
OSPF refuses to commit without a router-id. Show: 'show interface brief',
'show network-instance default protocols ospf neighbor'.",
        "ceos" | "veos" => "\
Arista EOS CLI = industry-standard config mode (www.arista.com EOS manual).
cEOS ports Ethernet1..N (vEOS: Management1 first). Routed ports need
'no switchport'; nothing is shut by default on cEOS.
  configure
  hostname leaf1
  interface Ethernet1
    no switchport
    ip address 10.0.0.1/30
  router ospf 1
    router-id 1.1.1.1
    network 10.0.0.0/30 area 0
  end
  write memory
BGP/EVPN: 'router bgp 65001', 'address-family evpn', 'vxlan interface Vxlan1'.",
        "crpd" => "\
Juniper cRPD = Junos control plane over Linux interfaces (eth0..N).
CLI: 'cli' -> 'configure' -> set commands -> 'commit' (juniper.net cRPD guide).
  set interfaces eth1 unit 0 family inet address 10.0.0.1/30
  set routing-options router-id 1.1.1.1
  set protocols ospf area 0.0.0.0 interface eth1.0 interface-type p2p
BGP: set protocols bgp group X type external / peer-as / neighbor.
No security zones (routing-only container).",
        "linux" => "\
Startup config via cloud-init. Bare shell commands run at boot alongside
the default access (serial auto-login root; SSH root/netpilot):
  ip addr add 10.0.0.10/24 dev eth1
  ip link set eth1 up
  ip route add default via 10.0.0.1
A full '#cloud-config' document replaces the defaults (set your own users,
write_files, runcmd — cloudinit.readthedocs.io).",
        "openwrt" => "\
OpenWrt uses UCI (openwrt.org/docs/guide-user/base-system/uci).
Config files under /etc/config; eth0 is bridged into br-lan (192.168.1.1,
DHCP server on), eth1 = wan (DHCP client). Serial console root, no password.
  uci set network.lan.ipaddr='10.0.0.1'
  uci commit network && service network restart
Firewall zones: uci show firewall. Packages: opkg update && opkg install X
(needs WAN). LuCI web UI on the LAN address.",
        "vyos" => "\
VyOS set-style CLI (docs.vyos.io). Login vyos/vyos. Interfaces eth0..N.
  configure
  set interfaces ethernet eth0 address 10.0.0.1/30
  set protocols ospf area 0 network 10.0.0.0/30
  set protocols ospf parameters router-id 1.1.1.1
  commit ; save
BGP: set protocols bgp system-as 65001 / neighbor X remote-as Y.
NAT: set nat source rule 10 outbound-interface name eth0 translation address masquerade.",
        "iosv" | "iosvl2" => "\
Classic Cisco IOS 15.x (Cisco IOS configuration guides). Ports Gi0/0..N
(IOSvL2: Gi0/0-3, Gi1/0-3 switchports). Interfaces are shutdown by default.
  configure terminal
  hostname r1
  no ip domain-lookup
  interface GigabitEthernet0/0
   ip address 10.0.0.1 255.255.255.252
   no shutdown
  router ospf 1
   router-id 1.1.1.1
   network 10.0.0.0 0.0.0.3 area 0
  end
  write memory
IOSvL2 switching: 'switchport mode access/trunk', 'vlan 10'. Wildcard masks
(not prefix lengths) in network statements.",
        "csr1000v" | "cat8000v" => "\
Cisco IOS-XE (Cisco IOS-XE configuration guides). Ports GigabitEthernet1..N,
shutdown by default. Same CLI shape as IOS:
  configure terminal
  interface GigabitEthernet1
   ip address 10.0.0.1 255.255.255.252
   no shutdown
  router ospf 1
   network 10.0.0.0 0.0.0.3 area 0
  end
  write memory
First boot takes minutes; config is injected as iosxe_config.txt.",
        "xrv9k" => "\
Cisco IOS XR (XR configuration guides). Ports GigabitEthernet0/0/0/0..N.
Two-stage config: changes apply only on 'commit'.
  configure
  interface GigabitEthernet0/0/0/0
   ipv4 address 10.0.0.1/30
   no shutdown
  router ospf 1
   router-id 1.1.1.1
   area 0 interface GigabitEthernet0/0/0/0
  commit
XR nests protocol interfaces under the process (no network statements).",
        "vsrx" => "\
Juniper vSRX = Junos with a flow-based firewall (Junos security docs).
fxp0 is management; data ports ge-0/0/0..N. Traffic is DROPPED until
interfaces are in security zones with policies:
  set interfaces ge-0/0/0 unit 0 family inet address 10.0.0.1/30
  set security zones security-zone trust interfaces ge-0/0/0.0
  set security zones security-zone trust host-inbound-traffic system-services ping
  set security policies default-policy permit-all
  set protocols ospf area 0 interface ge-0/0/0.0
  commit",
        "vjunos-switch" => "\
Juniper vJunos-switch = EX-style Junos (Junos EX docs). Ports ge-0/0/0..N.
  set interfaces ge-0/0/0 unit 0 family inet address 10.0.0.1/30
  set protocols ospf area 0 interface ge-0/0/0.0
  commit
L2: 'set interfaces ge-0/0/1 unit 0 family ethernet-switching vlan members v10',
'set vlans v10 vlan-id 10', IRB via 'set interfaces irb unit 10 family inet address'.",
        "fortigate" => "\
FortiOS CLI (docs.fortinet.com CLI reference). Ports port1..N. Traffic needs
interfaces up + a firewall policy; login admin / (blank) on first boot.
  config system interface
    edit port1
      set mode static
      set ip 10.0.0.1 255.255.255.252
      set allowaccess ping https ssh
    next
  end
  config router static
    edit 1
      set gateway 10.0.0.2
      set device port1
    next
  end
  config firewall policy
    edit 1
      set srcintf port2  set dstintf port1
      set srcaddr all    set dstaddr all
      set action accept  set schedule always  set service ALL
    next
  end",
        "panos" => "\
PAN-OS CLI (docs.paloaltonetworks.com). Login admin/admin. Ports
ethernet1/1..N; interfaces need a zone + virtual router to pass traffic.
  configure
  set network interface ethernet ethernet1/1 layer3 ip 10.0.0.1/30
  set network virtual-router default interface ethernet1/1
  set zone trust network layer3 ethernet1/1
  set rulebase security rules allow-all from any to any action allow
  commit
Commits take ~1 minute; management plane boots slowly.",
        "chr" => "\
MikroTik RouterOS CHR (help.mikrotik.com). Login admin / (blank).
Menu-path CLI:
  /ip address add address=10.0.0.1/30 interface=ether1
  /routing ospf instance add name=default router-id=1.1.1.1
  /routing ospf area add name=backbone instance=default
  /routing ospf interface-template add interfaces=ether1 area=backbone type=ptp
  /ip route add dst-address=0.0.0.0/0 gateway=10.0.0.2
Config is applied immediately (no commit); '/export' shows it.",
        _ => "",
    }
}

pub fn builtin_templates() -> Vec<NodeTemplate> {
    let t = |id: &str,
             name: &str,
             vendor: &str,
             icon: &str,
             cpus: u32,
             ram: u32,
             ifaces: u32,
             pattern: &str,
             qemu: QemuSpec,
             notes: &str| NodeTemplate {
        id: id.into(),
        name: name.into(),
        vendor: vendor.into(),
        icon: icon.into(),
        cpus,
        ram_mb: ram,
        interfaces: ifaces,
        max_interfaces: 32,
        iface_pattern: pattern.into(),
        console: ConsoleKind::Serial,
        kind: NodeKind::Qemu,
        qemu,
        container: None,
        netns: None,
        notes: notes.into(),
        export_command: export_command_for(id),
        config_guide: config_guide_for(id).into(),
    };

    let netns_t =
        |id: &str, name: &str, vendor: &str, icon: &str, service: &str, notes: &str| NodeTemplate {
            id: id.into(),
            name: name.into(),
            vendor: vendor.into(),
            icon: icon.into(),
            cpus: 1,
            ram_mb: 0,
            interfaces: 8,
            max_interfaces: 32,
            iface_pattern: "eth{i}".into(),
            console: ConsoleKind::Serial,
            kind: NodeKind::Netns,
            qemu: QemuSpec::default(),
            container: None,
            netns: Some(NetnsSpec {
                service: service.into(),
            }),
            notes: notes.into(),
            export_command: export_command_for(id),
            config_guide: config_guide_for(id).into(),
        };

    let container_t = |id: &str,
                       name: &str,
                       vendor: &str,
                       icon: &str,
                       ifaces: u32,
                       pattern: &str,
                       spec: ContainerSpec,
                       notes: &str| NodeTemplate {
        id: id.into(),
        name: name.into(),
        vendor: vendor.into(),
        icon: icon.into(),
        cpus: 2,
        ram_mb: 0,
        interfaces: ifaces,
        max_interfaces: 32,
        iface_pattern: pattern.into(),
        console: ConsoleKind::Serial,
        kind: NodeKind::Container,
        qemu: QemuSpec::default(),
        container: Some(spec),
        netns: None,
        notes: notes.into(),
        export_command: export_command_for(id),
        config_guide: config_guide_for(id).into(),
    };

    vec![
        // ---- built-ins that need no image at all ----
        netns_t(
            "frr",
            "FRRouting",
            "Open Source",
            "router",
            "frr",
            "Built-in: FRR daemons from the host package in an isolated namespace. \
             Startup config is a full frr.conf (OSPF/BGP/EVPN/LDP). No image needed. \
             Console: shell — run vtysh.",
        ),
        netns_t(
            "host",
            "Linux Endpoint",
            "Open Source",
            "server",
            "",
            "Built-in: lightweight Linux endpoint (network namespace + shell). \
             Startup config is a shell script run at boot. No image needed.",
        ),
        container_t(
            "srlinux",
            "Nokia SR Linux",
            "Nokia",
            "switch",
            8,
            "e1-{i+1}",
            ContainerSpec {
                image: "ghcr.io/nokia/srlinux:24.10.1".into(),
                cmd: vec![
                    "/tini".into(),
                    "--".into(),
                    "fixuid".into(),
                    "-q".into(),
                    "/entrypoint.sh".into(),
                    "sudo".into(),
                    "bash".into(),
                    "-c".into(),
                    "touch /.dockerenv && /opt/srlinux/bin/sr_linux".into(),
                ],
                env: vec!["SRLINUX=1".into()],
                privileged: true,
                console_cmd: Some("sr_cli".into()),
            },
            "Built-in: pulled automatically from ghcr.io/nokia/srlinux on first start \
             (public, no login). Console drops into sr_cli. Data ports e1-1..e1-N.",
        ),
        // ---- BYOI containers (upload a docker image tarball) ----
        container_t(
            "ceos",
            "Arista cEOS",
            "Arista",
            "switch",
            8,
            "eth{i}",
            ContainerSpec {
                image: "ceos:byoi".into(),
                cmd: vec![
                    "/sbin/init".into(),
                    "systemd.setenv=INTFTYPE=eth".into(),
                    "systemd.setenv=ETBA=1".into(),
                    "systemd.setenv=SKIP_ZEROTOUCH_BARRIER_IN_SYSDBINIT=1".into(),
                    "systemd.setenv=CEOS=1".into(),
                    "systemd.setenv=EOS_PLATFORM=ceoslab".into(),
                    "systemd.setenv=container=docker".into(),
                ],
                env: vec![
                    "INTFTYPE=eth".into(),
                    "ETBA=1".into(),
                    "SKIP_ZEROTOUCH_BARRIER_IN_SYSDBINIT=1".into(),
                    "CEOS=1".into(),
                    "EOS_PLATFORM=ceoslab".into(),
                    "container=docker".into(),
                ],
                privileged: true,
                console_cmd: Some("Cli".into()),
            },
            "BYOI: upload the cEOS-lab tarball (arista.com) via the Images page — \
             it is docker-loaded as ceos:byoi. Console drops into Cli.",
        ),
        container_t(
            "crpd",
            "Juniper cRPD",
            "Juniper",
            "router",
            8,
            "eth{i}",
            ContainerSpec {
                image: "crpd:byoi".into(),
                cmd: vec![],
                env: vec![],
                privileged: true,
                console_cmd: Some("cli".into()),
            },
            "BYOI: upload the cRPD docker tarball (juniper.net) via the Images page — \
             docker-loaded as crpd:byoi. Console drops into Junos cli.",
        ),
        t(
            "linux",
            "Linux Host",
            "Generic",
            "server",
            1,
            1024,
            4,
            "eth{i}",
            QemuSpec {
                config_delivery: ConfigDelivery::CloudInit,
                ..Default::default()
            },
            "Any cloud image (Ubuntu/Debian/Alpine qcow2), configured via cloud-init. \
             By default the serial console auto-logs-in as root and SSH accepts \
             root / netpilot. Bare shell commands in the startup config run at \
             boot alongside those defaults; a full #cloud-config (or #!script) \
             replaces them entirely.",
        ),
        t(
            "openwrt",
            "OpenWrt",
            "Open Source",
            "router",
            1,
            256,
            4,
            "eth{i}",
            QemuSpec::default(),
            "OpenWrt x86-64 (convert the ext4-combined image to qcow2). Serial \
             console, root with no password. eth0 is the LAN bridge (192.168.1.1, \
             DHCP server on), eth1 is WAN (DHCP client). Configure with uci/LuCI.",
        ),
        t(
            "vyos",
            "VyOS Router",
            "VyOS",
            "router",
            1,
            1024,
            8,
            "eth{i}",
            QemuSpec::default(),
            "VyOS 1.4/1.5 qcow2 images. Default login vyos/vyos.",
        ),
        t(
            "iosv",
            "Cisco IOSv",
            "Cisco",
            "router",
            1,
            1024,
            8,
            "Gi0/{i}",
            QemuSpec {
                nic_model: NicModel::E1000,
                disk_bus: DiskBus::Virtio,
                config_delivery: ConfigDelivery::FatDisk {
                    filename: "ios_config.txt".into(),
                },
                ..Default::default()
            },
            "vios-adventerprisek9 qcow2. Serial console.",
        ),
        t(
            "iosvl2",
            "Cisco IOSvL2",
            "Cisco",
            "switch",
            1,
            1024,
            16,
            "Gi{i/4}/{i%4}",
            QemuSpec {
                nic_model: NicModel::E1000,
                config_delivery: ConfigDelivery::FatDisk {
                    filename: "ios_config.txt".into(),
                },
                ..Default::default()
            },
            "vios_l2 qcow2 switch image.",
        ),
        t(
            "csr1000v",
            "Cisco CSR1000v",
            "Cisco",
            "router",
            1,
            3072,
            8,
            "Gi{i+1}",
            QemuSpec {
                nic_model: NicModel::VirtioNetPci,
                config_delivery: ConfigDelivery::CdromIso {
                    filename: "iosxe_config.txt".into(),
                },
                ..Default::default()
            },
            "csr1000v universalk9 qcow2. Boot takes several minutes.",
        ),
        t(
            "cat8000v",
            "Cisco Catalyst 8000v",
            "Cisco",
            "router",
            1,
            4096,
            8,
            "Gi{i+1}",
            QemuSpec {
                nic_model: NicModel::VirtioNetPci,
                config_delivery: ConfigDelivery::CdromIso {
                    filename: "iosxe_config.txt".into(),
                },
                ..Default::default()
            },
            "c8000v qcow2 (17.x).",
        ),
        t(
            "veos",
            "Arista vEOS",
            "Arista",
            "switch",
            1,
            2048,
            8,
            "Ethernet{i}",
            QemuSpec {
                nic_model: NicModel::E1000,
                disk_bus: DiskBus::Ide,
                mgmt_nic: true,
                ..Default::default()
            },
            "vEOS-lab qcow2 plus Aboot iso attached as CD-ROM. First NIC is Management1.",
        ),
        t(
            "vsrx",
            "Juniper vSRX",
            "Juniper",
            "firewall",
            2,
            4096,
            8,
            "ge-0/0/{i}",
            QemuSpec {
                nic_model: NicModel::VirtioNetPci,
                cpu_model: "host".into(),
                mgmt_nic: true,
                ..Default::default()
            },
            "vSRX 3.0 qcow2. First NIC is fxp0 (management). Needs nested virt for best results.",
        ),
        t(
            "vjunos-switch",
            "Juniper vJunos-switch",
            "Juniper",
            "switch",
            4,
            5120,
            10,
            "ge-0/0/{i}",
            QemuSpec {
                nic_model: NicModel::VirtioNetPci,
                cpu_model: "host".into(),
                mgmt_nic: true,
                ..Default::default()
            },
            "vJunos-switch qcow2. First NIC management.",
        ),
        t(
            "fortigate",
            "Fortinet FortiGate",
            "Fortinet",
            "firewall",
            1,
            2048,
            8,
            "port{i+1}",
            QemuSpec {
                nic_model: NicModel::VirtioNetPci,
                ..Default::default()
            },
            "FGT VM64 KVM qcow2. Default login admin / blank.",
        ),
        t(
            "panos",
            "Palo Alto VM-Series",
            "Palo Alto",
            "firewall",
            2,
            6144,
            8,
            "ethernet1/{i}",
            QemuSpec {
                nic_model: NicModel::VirtioNetPci,
                mgmt_nic: true,
                ..Default::default()
            },
            "PA-VM KVM qcow2. First NIC is management. Slow first boot.",
        ),
        t(
            "chr",
            "MikroTik CHR",
            "MikroTik",
            "router",
            1,
            512,
            8,
            "ether{i+1}",
            QemuSpec {
                disk_bus: DiskBus::Ide,
                ..Default::default()
            },
            "Cloud Hosted Router chr .img/.qcow2. Login admin / blank.",
        ),
        t(
            "xrv9k",
            "Cisco XRv 9000",
            "Cisco",
            "router",
            4,
            16384,
            8,
            "Gi0/0/0/{i}",
            QemuSpec {
                nic_model: NicModel::VirtioNetPci,
                cpu_model: "host".into(),
                mgmt_nic: true,
                ..Default::default()
            },
            "xrv9k-fullk9 qcow2. Very heavy; first NICs are mgmt/ctrl.",
        ),
    ]
}

/// A disk image registered in the library.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiskImage {
    /// Template family this image belongs to (template id).
    pub template: String,
    /// Version/name label, taken from the directory name.
    pub version: String,
    /// Absolute path to the base image file.
    pub path: PathBuf,
    /// File size in bytes.
    pub size_bytes: u64,
}

/// Image library rooted at `<data_dir>/images/<template>/<version>/<file>`.
///
/// Base images are immutable; nodes boot from qcow2 overlays created on top.
#[derive(Debug, Clone)]
pub struct ImageLibrary {
    root: PathBuf,
}

const IMAGE_EXTS: [&str; 4] = ["qcow2", "img", "iso", "vmdk"];

impl ImageLibrary {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn dir_for(&self, template: &str, version: &str) -> PathBuf {
        self.root.join(template).join(version)
    }

    /// Scan the library directory tree for images.
    pub fn scan(&self) -> Result<Vec<DiskImage>> {
        let mut images = Vec::new();
        if !self.root.exists() {
            return Ok(images);
        }
        for tpl_entry in std::fs::read_dir(&self.root)? {
            let tpl_entry = tpl_entry?;
            if !tpl_entry.file_type()?.is_dir() {
                continue;
            }
            let template = tpl_entry.file_name().to_string_lossy().to_string();
            for ver_entry in std::fs::read_dir(tpl_entry.path())? {
                let ver_entry = ver_entry?;
                if !ver_entry.file_type()?.is_dir() {
                    continue;
                }
                let version = ver_entry.file_name().to_string_lossy().to_string();
                for f in std::fs::read_dir(ver_entry.path())? {
                    let f = f?;
                    let path = f.path();
                    let ext = path
                        .extension()
                        .map(|e| e.to_string_lossy().to_lowercase())
                        .unwrap_or_default();
                    if f.file_type()?.is_file() && IMAGE_EXTS.contains(&ext.as_str()) {
                        images.push(DiskImage {
                            template: template.clone(),
                            version: version.clone(),
                            path,
                            size_bytes: f.metadata()?.len(),
                        });
                    }
                }
            }
        }
        images.sort_by(|a, b| (&a.template, &a.version).cmp(&(&b.template, &b.version)));
        Ok(images)
    }

    /// Find the primary boot image for template/version. Prefers qcow2.
    pub fn find(&self, template: &str, version: &str) -> Result<DiskImage> {
        let all = self.scan()?;
        all.into_iter()
            .filter(|i| i.template == template && i.version == version)
            .min_by_key(|i| {
                // qcow2 first, then img, then others
                match i.path.extension().and_then(|e| e.to_str()) {
                    Some("qcow2") => 0,
                    Some("img") => 1,
                    _ => 2,
                }
            })
            .ok_or_else(|| CoreError::ImageNotFound(format!("{template}/{version}")))
    }
}

/// Template catalog: built-ins merged with user-defined YAML templates.
#[derive(Debug, Clone)]
pub struct TemplateCatalog {
    templates: BTreeMap<String, NodeTemplate>,
}

impl TemplateCatalog {
    /// Load built-ins, then merge user templates from `dir` (*.yaml).
    pub fn load(user_dir: Option<&Path>) -> Result<Self> {
        let mut templates: BTreeMap<String, NodeTemplate> = builtin_templates()
            .into_iter()
            .map(|t| (t.id.clone(), t))
            .collect();
        if let Some(dir) = user_dir {
            if dir.exists() {
                for entry in std::fs::read_dir(dir)? {
                    let path = entry?.path();
                    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
                    if ext == "yaml" || ext == "yml" {
                        let text = std::fs::read_to_string(&path)?;
                        let tpl: NodeTemplate = serde_yaml::from_str(&text)?;
                        templates.insert(tpl.id.clone(), tpl);
                    }
                }
            }
        }
        Ok(Self { templates })
    }

    pub fn get(&self, id: &str) -> Result<&NodeTemplate> {
        self.templates
            .get(id)
            .ok_or_else(|| CoreError::TemplateNotFound(id.into()))
    }

    pub fn all(&self) -> impl Iterator<Item = &NodeTemplate> {
        self.templates.values()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_catalog_loads() {
        let cat = TemplateCatalog::load(None).unwrap();
        assert!(cat.get("vyos").is_ok());
        assert!(cat.get("iosv").is_ok());
        assert!(cat.get("nope").is_err());
        assert!(cat.all().count() >= 10);
    }

    #[test]
    fn image_scan() {
        let dir = tempfile::tempdir().unwrap();
        let lib = ImageLibrary::new(dir.path());
        assert!(lib.scan().unwrap().is_empty());

        let d = lib.dir_for("vyos", "1.5");
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(d.join("vyos.qcow2"), b"fake").unwrap();
        std::fs::write(d.join("readme.txt"), b"skip").unwrap();

        let images = lib.scan().unwrap();
        assert_eq!(images.len(), 1);
        assert_eq!(images[0].template, "vyos");
        assert_eq!(images[0].version, "1.5");

        let found = lib.find("vyos", "1.5").unwrap();
        assert!(found.path.ends_with("vyos.qcow2"));
        assert!(lib.find("vyos", "9.9").is_err());
    }
}
