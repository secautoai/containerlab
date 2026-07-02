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
    /// QEMU boot recipe.
    #[serde(default)]
    pub qemu: QemuSpec,
    /// Free-form notes shown in the UI (image requirements, credentials...).
    #[serde(default)]
    pub notes: String,
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
        qemu,
        notes: notes.into(),
        export_command: export_command_for(id),
    };

    vec![
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
            "Any cloud image (Ubuntu/Debian/Alpine qcow2). Uses cloud-init for user/config.",
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
            "frr",
            "FRRouting (Linux)",
            "Open Source",
            "router",
            1,
            768,
            8,
            "eth{i}",
            QemuSpec {
                config_delivery: ConfigDelivery::CloudInit,
                ..Default::default()
            },
            "Linux cloud image with FRR installed via cloud-init.",
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
