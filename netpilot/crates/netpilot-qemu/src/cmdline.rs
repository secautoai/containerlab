//! QEMU command-line builder.
//!
//! Turns a node + its template's [`QemuSpec`] + datapath wiring into a full
//! argv. Conventions follow vrnetlab/GNS3 practice (see
//! docs/research/qemu-network-emulation.md):
//!
//! * NICs emitted in strict index order — NOS images name interfaces by PCI
//!   enumeration order.
//! * Deterministic MACs and `-uuid` (licensing on several platforms keys
//!   off them).
//! * PCI bridges appended automatically when the NIC count outgrows the
//!   root bus.
//! * Serial console on a unix socket chardev (no telnet framing; the API
//!   server bridges it to WebSockets).

use std::path::{Path, PathBuf};

use netpilot_core::{ConsoleKind, DiskBus, QemuSpec};
use netpilot_net::{node_mac, NicWiring};
use uuid::Uuid;

/// Everything needed to boot one node.
#[derive(Debug, Clone)]
pub struct NodeBootSpec {
    pub lab_id: Uuid,
    pub node_id: Uuid,
    pub name: String,
    pub qemu: QemuSpec,
    pub cpus: u32,
    pub ram_mb: u32,
    pub interfaces: u32,
    pub console: ConsoleKind,
    /// Node's qcow2 overlay (already created).
    pub overlay_disk: PathBuf,
    /// Optional extra disks (e.g. FortiGate log disk), attached after the boot disk.
    pub extra_disks: Vec<PathBuf>,
    /// Config media produced by [`crate::media`]: attached per its kind.
    pub config_media: Option<ConfigMedia>,
    /// Datapath wiring per interface index (from the UDP switch).
    pub nics: Vec<NicWiring>,
    /// Directory for runtime artifacts (qemu.log). May be a deep path.
    pub run_dir: PathBuf,
    /// Directory for qmp/console unix sockets. MUST be short: sun_path is
    /// limited to 107 bytes (see [`socket_dir_for`]).
    pub socket_dir: PathBuf,
    /// Use KVM acceleration.
    pub kvm: bool,
    /// VNC display number when console is VNC (port = 5900 + display).
    pub vnc_display: Option<u16>,
}

/// A config-injection artifact and how to attach it.
#[derive(Debug, Clone)]
pub enum ConfigMedia {
    /// ISO image attached as CD-ROM (cloud-init seed, CVAC, juniper.conf).
    Cdrom(PathBuf),
    /// Raw FAT disk attached as a secondary disk (IOSv config disk).
    Disk(PathBuf),
}

/// Root PCI bus slots we allow for NICs before adding bridges (i440FX).
const ROOT_BUS_NICS: usize = 24;
/// NICs per additional PCI bridge (vrnetlab convention).
const NICS_PER_BRIDGE: usize = 26;

impl NodeBootSpec {
    pub fn qemu_binary(&self) -> String {
        format!("qemu-system-{}", self.qemu.arch)
    }

    pub fn qmp_socket(&self) -> PathBuf {
        self.socket_dir.join("qmp.sock")
    }

    pub fn console_socket(&self) -> PathBuf {
        self.socket_dir.join("console.sock")
    }

    /// Build the full argument list (excluding argv[0]).
    pub fn build_args(&self) -> Vec<String> {
        let mut a: Vec<String> = Vec::new();
        let push = |a: &mut Vec<String>, args: &[&str]| {
            a.extend(args.iter().map(|s| s.to_string()))
        };

        push(&mut a, &["-name", &self.name]);
        push(&mut a, &["-uuid", &self.node_id.to_string()]);
        push(&mut a, &["-display", "none"]);

        // Machine + acceleration.
        let machine = if self.qemu.machine.is_empty() {
            "pc"
        } else {
            &self.qemu.machine
        };
        let accel = if self.kvm { "kvm" } else { "tcg" };
        a.push("-machine".into());
        a.push(format!("{machine},accel={accel}"));

        // CPU model: explicit > host-when-kvm > qemu default.
        if !self.qemu.cpu_model.is_empty() {
            push(&mut a, &["-cpu", &self.qemu.cpu_model]);
        } else if self.kvm {
            push(&mut a, &["-cpu", "host"]);
        }
        a.push("-smp".into());
        a.push(format!(
            "{},sockets=1,cores={},threads=1",
            self.cpus, self.cpus
        ));
        a.push("-m".into());
        a.push(self.ram_mb.to_string());

        // Boot disk.
        let disk_if = match self.qemu.disk_bus {
            DiskBus::Virtio => "virtio",
            DiskBus::Ide | DiskBus::Sata => "ide",
            DiskBus::Scsi => "scsi",
        };
        a.push("-drive".into());
        a.push(format!(
            "if={},format=qcow2,file={}",
            disk_if,
            self.overlay_disk.display()
        ));
        for (i, disk) in self.extra_disks.iter().enumerate() {
            a.push("-drive".into());
            a.push(format!(
                "if={},index={},format=qcow2,file={}",
                disk_if,
                i + 1,
                disk.display()
            ));
        }

        // Config media.
        if let Some(media) = &self.config_media {
            match media {
                ConfigMedia::Cdrom(path) => {
                    a.push("-drive".into());
                    a.push(format!(
                        "if=ide,index=2,media=cdrom,file={}",
                        path.display()
                    ));
                }
                ConfigMedia::Disk(path) => {
                    a.push("-drive".into());
                    a.push(format!(
                        "if={},index=3,format=raw,file={}",
                        disk_if,
                        path.display()
                    ));
                }
            }
        }

        // Extra PCI bridges if the NIC count needs them.
        let nic_count = self.nics.len();
        if nic_count > ROOT_BUS_NICS {
            let bridges = nic_count.saturating_sub(ROOT_BUS_NICS).div_ceil(NICS_PER_BRIDGE);
            for b in 0..bridges {
                a.push("-device".into());
                a.push(format!("pci-bridge,chassis_nr={},id=pci.{}", b + 1, b + 1));
            }
        }

        // NICs in strict index order.
        for (i, wiring) in self.nics.iter().enumerate() {
            let mac = node_mac(self.lab_id, self.node_id, i as u32);
            a.push("-netdev".into());
            a.push(format!(
                "socket,id=np{i},udp=127.0.0.1:{},localaddr=127.0.0.1:{}",
                wiring.switch_port, wiring.qemu_port
            ));
            a.push("-device".into());
            let mut dev = format!(
                "{},netdev=np{i},mac={mac}",
                self.qemu.nic_model.qemu_name()
            );
            if i >= ROOT_BUS_NICS {
                let bridge = 1 + (i - ROOT_BUS_NICS) / NICS_PER_BRIDGE;
                let slot = 1 + (i - ROOT_BUS_NICS) % NICS_PER_BRIDGE;
                dev.push_str(&format!(",bus=pci.{bridge},addr=0x{slot:x}"));
            }
            a.push(dev);
        }

        // Console.
        match self.console {
            ConsoleKind::Serial => {
                a.push("-chardev".into());
                a.push(format!(
                    "socket,id=console0,path={},server=on,wait=off",
                    self.console_socket().display()
                ));
                push(&mut a, &["-serial", "chardev:console0"]);
            }
            ConsoleKind::Vnc => {
                let display = self.vnc_display.unwrap_or(0);
                a.push("-vnc".into());
                a.push(format!("127.0.0.1:{display}"));
            }
        }

        // QMP control socket.
        a.push("-qmp".into());
        a.push(format!(
            "unix:{},server=on,wait=off",
            self.qmp_socket().display()
        ));

        // Template extras (SMBIOS strings, -boot flags, platform quirks).
        a.extend(self.qemu.extra_args.iter().cloned());

        a
    }
}

/// Render args for logging/debugging.
pub fn render_cmdline(binary: &str, args: &[String]) -> String {
    let mut s = String::from(binary);
    for arg in args {
        s.push(' ');
        if arg.contains(' ') || arg.contains(',') && arg.contains('=') && arg.len() > 60 {
            s.push('\'');
            s.push_str(arg);
            s.push('\'');
        } else {
            s.push_str(arg);
        }
    }
    s
}

/// Compute the deterministic overlay path for a node inside its run dir.
pub fn overlay_path(node_dir: &Path) -> PathBuf {
    node_dir.join("disk.qcow2")
}

/// Short, stable per-node socket directory. Unix socket paths are limited
/// to ~107 bytes, so these live under the system temp dir rather than the
/// (potentially deep) data directory.
pub fn socket_dir_for(node_id: Uuid) -> PathBuf {
    let short = &node_id.simple().to_string()[..12];
    std::env::temp_dir().join("netpilot").join(short)
}

#[cfg(test)]
mod tests {
    use super::*;
    use netpilot_core::NicModel;

    fn spec(nics: usize) -> NodeBootSpec {
        NodeBootSpec {
            lab_id: Uuid::parse_str("11111111-2222-3333-4444-555566667777").unwrap(),
            node_id: Uuid::parse_str("aaaaaaaa-bbbb-cccc-dddd-eeeeffff0000").unwrap(),
            name: "R1".into(),
            qemu: QemuSpec {
                nic_model: NicModel::E1000,
                ..Default::default()
            },
            cpus: 2,
            ram_mb: 2048,
            interfaces: nics as u32,
            console: ConsoleKind::Serial,
            overlay_disk: "/data/labs/x/nodes/y/disk.qcow2".into(),
            extra_disks: vec![],
            config_media: None,
            nics: (0..nics)
                .map(|i| NicWiring {
                    switch_port: 46000 + (i as u16) * 2,
                    qemu_port: 46001 + (i as u16) * 2,
                })
                .collect(),
            run_dir: "/data/labs/x/nodes/y".into(),
            socket_dir: "/run/np/y".into(),
            kvm: false,
            vnc_display: None,
        }
    }

    #[test]
    fn basic_cmdline() {
        let s = spec(2);
        let args = s.build_args();
        let joined = args.join(" ");
        assert!(joined.contains("-machine pc,accel=tcg"));
        assert!(joined.contains("-smp 2,sockets=1,cores=2,threads=1"));
        assert!(joined.contains("-m 2048"));
        assert!(joined.contains("if=virtio,format=qcow2,file=/data/labs/x/nodes/y/disk.qcow2"));
        assert!(joined.contains("socket,id=np0,udp=127.0.0.1:46000,localaddr=127.0.0.1:46001"));
        assert!(joined.contains("e1000,netdev=np0,mac=52:54:00:"));
        assert!(joined.contains("-qmp unix:/run/np/y/qmp.sock,server=on,wait=off"));
        assert!(joined.contains("path=/run/np/y/console.sock"));
        // no -cpu without kvm and without explicit model
        assert!(!joined.contains("-cpu"));
    }

    #[test]
    fn macs_are_stable_and_ordered() {
        let s = spec(3);
        let a1 = s.build_args().join(" ");
        let a2 = s.build_args().join(" ");
        assert_eq!(a1, a2);
        let np0 = a1.find("netdev=np0").unwrap();
        let np1 = a1.find("netdev=np1").unwrap();
        let np2 = a1.find("netdev=np2").unwrap();
        assert!(np0 < np1 && np1 < np2);
    }

    #[test]
    fn pci_bridges_for_many_nics() {
        let s = spec(40);
        let joined = s.build_args().join(" ");
        assert!(joined.contains("pci-bridge,chassis_nr=1,id=pci.1"));
        assert!(joined.contains("bus=pci.1,addr=0x1"));
    }

    #[test]
    fn kvm_and_cpu_model() {
        let mut s = spec(1);
        s.kvm = true;
        let joined = s.build_args().join(" ");
        assert!(joined.contains("accel=kvm"));
        assert!(joined.contains("-cpu host"));

        s.qemu.cpu_model = "SandyBridge,vmx=on".into();
        let joined = s.build_args().join(" ");
        assert!(joined.contains("-cpu SandyBridge,vmx=on"));
    }

    #[test]
    fn vnc_console() {
        let mut s = spec(1);
        s.console = ConsoleKind::Vnc;
        s.vnc_display = Some(7);
        let joined = s.build_args().join(" ");
        assert!(joined.contains("-vnc 127.0.0.1:7"));
        assert!(!joined.contains("console.sock"));
    }

    #[test]
    fn config_media_attachment() {
        let mut s = spec(1);
        s.config_media = Some(ConfigMedia::Cdrom("/x/config.iso".into()));
        let joined = s.build_args().join(" ");
        assert!(joined.contains("media=cdrom,file=/x/config.iso"));
    }
}
