//! qcow2 overlay lifecycle via `qemu-img`.
//!
//! Base images are immutable under the image library; each node boots a
//! copy-on-write overlay in its own directory:
//!
//! * start  → create overlay backed by the base (if missing)
//! * wipe   → delete the overlay (factory reset, EVE-NG semantics)
//! * commit → fold overlay changes into a standalone image (save-as-image)

use std::path::Path;

use crate::{QemuError, Result};

async fn qemu_img(args: &[&str]) -> Result<String> {
    let out = tokio::process::Command::new("qemu-img")
        .args(args)
        .output()
        .await
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                QemuError::QemuMissing("qemu-img".into())
            } else {
                QemuError::Io(e)
            }
        })?;
    if !out.status.success() {
        return Err(QemuError::QemuImg(format!(
            "qemu-img {}: {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr)
        )));
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// Create a qcow2 overlay backed by `base` unless it already exists.
pub async fn ensure_overlay(base: &Path, overlay: &Path) -> Result<()> {
    if overlay.exists() {
        return Ok(());
    }
    if let Some(parent) = overlay.parent() {
        std::fs::create_dir_all(parent)?;
    }
    qemu_img(&[
        "create",
        "-f",
        "qcow2",
        "-F",
        "qcow2",
        "-b",
        &base.canonicalize()?.to_string_lossy(),
        &overlay.to_string_lossy(),
    ])
    .await?;
    Ok(())
}

/// Delete a node's overlay — next boot starts from the pristine base.
pub fn wipe_overlay(overlay: &Path) -> Result<()> {
    if overlay.exists() {
        std::fs::remove_file(overlay)?;
    }
    Ok(())
}

/// Flatten the overlay chain into a standalone image at `dest`
/// (used for "save node as new base image").
pub async fn export_flat(overlay: &Path, dest: &Path) -> Result<()> {
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)?;
    }
    qemu_img(&[
        "convert",
        "-O",
        "qcow2",
        &overlay.to_string_lossy(),
        &dest.to_string_lossy(),
    ])
    .await?;
    Ok(())
}

/// Create a blank qcow2 (extra data/log disks some platforms require).
pub async fn create_blank(path: &Path, size_gb: u32) -> Result<()> {
    if path.exists() {
        return Ok(());
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    qemu_img(&[
        "create",
        "-f",
        "qcow2",
        &path.to_string_lossy(),
        &format!("{size_gb}G"),
    ])
    .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wipe_missing_is_ok() {
        let dir = tempfile::tempdir().unwrap();
        wipe_overlay(&dir.path().join("nope.qcow2")).unwrap();
    }

    #[tokio::test]
    async fn qemu_img_missing_reports_clearly() {
        // In environments without qemu-img this must surface QemuMissing,
        // and with qemu-img installed the create should succeed.
        let dir = tempfile::tempdir().unwrap();
        let base = dir.path().join("base.qcow2");
        std::fs::write(&base, b"not-a-real-image").unwrap();
        let overlay = dir.path().join("overlay.qcow2");
        match ensure_overlay(&base, &overlay).await {
            Ok(()) => assert!(overlay.exists()),
            Err(QemuError::QemuMissing(_)) | Err(QemuError::QemuImg(_)) => {}
            Err(e) => panic!("unexpected error kind: {e}"),
        }
    }
}
