//! netpilot-qemu — QEMU node orchestration.
//!
//! Builds QEMU command lines from device templates, manages per-node qcow2
//! overlays, generates config-injection media (cloud-init / Cisco CVAC /
//! Juniper config ISO / FAT config disk) in pure Rust, talks QMP for
//! graceful control, and runs the node process lifecycle.

pub mod cmdline;
pub mod disk;
pub mod media;
pub mod node;
pub mod qmp;

pub use cmdline::*;
pub use disk::*;
pub use media::*;
pub use node::*;
pub use qmp::*;

#[derive(Debug, thiserror::Error)]
pub enum QemuError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("qemu-img failed: {0}")]
    QemuImg(String),
    #[error("qemu binary not found: {0} (install qemu-system packages on the lab host)")]
    QemuMissing(String),
    #[error("node is not running")]
    NotRunning,
    #[error("qmp error: {0}")]
    Qmp(String),
    #[error("{0}")]
    Other(String),
}

pub type Result<T> = std::result::Result<T, QemuError>;

/// Detect whether hardware acceleration is available on this host.
pub fn kvm_available() -> bool {
    std::path::Path::new("/dev/kvm").exists()
}
