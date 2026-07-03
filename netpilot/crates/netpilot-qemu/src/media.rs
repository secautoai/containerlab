//! Config-injection media builders — pure Rust, no genisoimage/mtools needed.
//!
//! * [`build_iso`] — minimal ISO9660 (single root directory) with Rock Ridge
//!   NM entries so guests see exact lowercase filenames. Used for cloud-init
//!   NoCloud seeds (`cidata`), Cisco CVAC (`iosxe_config.txt`) and Juniper
//!   (`juniper.conf`) config CD-ROMs.
//! * [`build_fat_disk`] — raw FAT16 disk image (IOSv `ios_config.txt`
//!   config disk and similar).

use std::io::Write;
use std::path::{Path, PathBuf};

use netpilot_core::ConfigDelivery;

use crate::cmdline::ConfigMedia;
use crate::{QemuError, Result};

const SECTOR: usize = 2048;

fn both_u16(v: u16) -> [u8; 4] {
    let le = v.to_le_bytes();
    let be = v.to_be_bytes();
    [le[0], le[1], be[0], be[1]]
}

fn both_u32(v: u32) -> [u8; 8] {
    let mut out = [0u8; 8];
    out[..4].copy_from_slice(&v.to_le_bytes());
    out[4..].copy_from_slice(&v.to_be_bytes());
    out
}

/// Fixed recording timestamp (deterministic output; value is irrelevant
/// to consumers). 2026-01-01 00:00:00 UTC.
const DIR_DATE: [u8; 7] = [126, 1, 1, 0, 0, 0, 0];

/// Translate a filename into ISO9660 d-characters (fallback identity when
/// Rock Ridge is unavailable in the reader).
fn iso_name(name: &str) -> String {
    let mapped: String = name
        .to_ascii_uppercase()
        .chars()
        .map(|c| match c {
            'A'..='Z' | '0'..='9' | '_' | '.' => c,
            _ => '_',
        })
        .collect();
    format!("{};1", mapped)
}

/// One directory record with optional Rock Ridge system-use entries.
fn dir_record(name_bytes: &[u8], extent: u32, size: u32, is_dir: bool, susp: &[u8]) -> Vec<u8> {
    let name_len = name_bytes.len();
    let mut base = 33 + name_len;
    if base % 2 == 1 {
        base += 1; // pad byte so system-use area starts on even offset
    }
    let total = base + susp.len();
    let mut r = vec![0u8; total];
    r[0] = total as u8;
    r[2..10].copy_from_slice(&both_u32(extent));
    r[10..18].copy_from_slice(&both_u32(size));
    r[18..25].copy_from_slice(&DIR_DATE);
    r[25] = if is_dir { 0x02 } else { 0x00 };
    r[28..32].copy_from_slice(&both_u16(1));
    r[32] = name_len as u8;
    r[33..33 + name_len].copy_from_slice(name_bytes);
    r[base..].copy_from_slice(susp);
    r
}

/// Rock Ridge SP entry (SUSP indicator, goes on the root "." record).
fn rr_sp() -> Vec<u8> {
    vec![b'S', b'P', 7, 1, 0xBE, 0xEF, 0]
}

/// Rock Ridge NM entry carrying the exact filename.
fn rr_nm(name: &str) -> Vec<u8> {
    let mut v = vec![b'N', b'M', (5 + name.len()) as u8, 1, 0];
    v.extend_from_slice(name.as_bytes());
    v
}

/// Build a small ISO9660 image with the given files in the root directory.
///
/// Layout: sectors 0-15 zero, 16 PVD, 17 terminator, 18/19 path tables,
/// 20 root directory, 21+ file extents.
pub fn build_iso(volume_id: &str, files: &[(&str, &[u8])]) -> Result<Vec<u8>> {
    const ROOT_SECTOR: u32 = 20;
    let first_file_sector: u32 = ROOT_SECTOR + 1;

    // --- root directory extent ---
    let mut root = Vec::new();
    root.extend(dir_record(
        &[0u8],
        ROOT_SECTOR,
        SECTOR as u32,
        true,
        &rr_sp(),
    )); // "."
    root.extend(dir_record(&[1u8], ROOT_SECTOR, SECTOR as u32, true, &[])); // ".."

    let mut extent = first_file_sector;
    let mut file_extents = Vec::new();
    for (name, data) in files {
        let iso = iso_name(name);
        root.extend(dir_record(
            iso.as_bytes(),
            extent,
            data.len() as u32,
            false,
            &rr_nm(name),
        ));
        file_extents.push((extent, *data));
        extent += data.len().div_ceil(SECTOR).max(1) as u32;
    }
    if root.len() > SECTOR {
        return Err(QemuError::Other(
            "too many files for single-sector ISO root directory".into(),
        ));
    }
    root.resize(SECTOR, 0);

    let total_sectors = extent;

    // --- path tables (just the root) ---
    // L: len_di(1), ext_attr(0), extent(u32), parent(u16), name(0x00), pad
    let mut lpath = vec![1u8, 0];
    lpath.extend_from_slice(&ROOT_SECTOR.to_le_bytes());
    lpath.extend_from_slice(&1u16.to_le_bytes());
    lpath.extend_from_slice(&[0, 0]);
    let mut mpath = vec![1u8, 0];
    mpath.extend_from_slice(&ROOT_SECTOR.to_be_bytes());
    mpath.extend_from_slice(&1u16.to_be_bytes());
    mpath.extend_from_slice(&[0, 0]);
    let path_table_size = lpath.len() as u32;
    lpath.resize(SECTOR, 0);
    mpath.resize(SECTOR, 0);

    // --- primary volume descriptor ---
    let mut pvd = vec![0u8; SECTOR];
    pvd[0] = 1;
    pvd[1..6].copy_from_slice(b"CD001");
    pvd[6] = 1;
    for b in &mut pvd[8..72] {
        *b = b' ';
    }
    let vid = volume_id.as_bytes();
    let n = vid.len().min(32);
    pvd[40..40 + n].copy_from_slice(&vid[..n]);
    pvd[80..88].copy_from_slice(&both_u32(total_sectors));
    pvd[120..124].copy_from_slice(&both_u16(1)); // volume set size
    pvd[124..128].copy_from_slice(&both_u16(1)); // sequence number
    pvd[128..132].copy_from_slice(&both_u16(SECTOR as u16));
    pvd[132..140].copy_from_slice(&both_u32(path_table_size));
    pvd[140..144].copy_from_slice(&18u32.to_le_bytes()); // L path table
    pvd[148..152].copy_from_slice(&19u32.to_be_bytes()); // M path table
    let root_rec = dir_record(&[0u8], ROOT_SECTOR, SECTOR as u32, true, &[]);
    pvd[156..156 + root_rec.len()].copy_from_slice(&root_rec);
    for b in &mut pvd[190..881] {
        *b = b' ';
    }
    pvd[881] = 1; // file structure version

    let mut terminator = vec![0u8; SECTOR];
    terminator[0] = 255;
    terminator[1..6].copy_from_slice(b"CD001");
    terminator[6] = 1;

    // --- assemble ---
    let mut iso = Vec::with_capacity(total_sectors as usize * SECTOR);
    iso.resize(16 * SECTOR, 0);
    iso.extend_from_slice(&pvd);
    iso.extend_from_slice(&terminator);
    iso.extend_from_slice(&lpath);
    iso.extend_from_slice(&mpath);
    iso.extend_from_slice(&root);
    for (sector, data) in file_extents {
        assert_eq!(iso.len(), sector as usize * SECTOR);
        iso.extend_from_slice(data);
        let pad = iso.len().next_multiple_of(SECTOR) - iso.len();
        iso.extend(std::iter::repeat_n(
            0u8,
            pad.max(if data.is_empty() { SECTOR } else { 0 }),
        ));
    }
    Ok(iso)
}

/// Build a raw FAT16 disk image containing the given files in its root.
pub fn build_fat_disk(label: &str, files: &[(&str, &[u8])]) -> Result<Vec<u8>> {
    let payload: usize = files.iter().map(|(_, d)| d.len()).sum();
    let size = (payload + 4 * 1024 * 1024).next_multiple_of(512); // 4 MiB headroom
    let mut buf = vec![0u8; size];
    {
        let cursor = fscommon::BufStream::new(std::io::Cursor::new(&mut buf[..]));
        let mut lbl = [b' '; 11];
        let lb = label.as_bytes();
        lbl[..lb.len().min(11)].copy_from_slice(&lb[..lb.len().min(11)]);
        fatfs::format_volume(
            cursor,
            fatfs::FormatVolumeOptions::new()
                .volume_label(lbl)
                .fat_type(fatfs::FatType::Fat16),
        )
        .map_err(|e| QemuError::Other(format!("FAT format: {e}")))?;
    }
    {
        let cursor = fscommon::BufStream::new(std::io::Cursor::new(&mut buf[..]));
        let fs = fatfs::FileSystem::new(cursor, fatfs::FsOptions::new())
            .map_err(|e| QemuError::Other(format!("FAT open: {e}")))?;
        for (name, data) in files {
            let mut f = fs
                .root_dir()
                .create_file(name)
                .map_err(|e| QemuError::Other(format!("FAT create {name}: {e}")))?;
            f.write_all(data)
                .map_err(|e| QemuError::Other(format!("FAT write {name}: {e}")))?;
        }
    }
    Ok(buf)
}

/// Produce the config media for a node according to its template's delivery
/// mechanism. Returns what to attach, or None when the template takes no
/// automated config (or no config was set).
pub fn build_config_media(
    delivery: &ConfigDelivery,
    node_name: &str,
    startup_config: Option<&str>,
    out_dir: &Path,
) -> Result<Option<ConfigMedia>> {
    let Some(config) = startup_config else {
        return Ok(None);
    };
    std::fs::create_dir_all(out_dir)?;

    match delivery {
        ConfigDelivery::None => Ok(None),
        ConfigDelivery::CdromIso { filename } => {
            let iso = build_iso("CONFIG", &[(filename.as_str(), config.as_bytes())])?;
            let path = out_dir.join("config.iso");
            write_atomic(&path, &iso)?;
            Ok(Some(ConfigMedia::Cdrom(path)))
        }
        ConfigDelivery::FatDisk { filename } => {
            let img = build_fat_disk("CONFIG", &[(filename.as_str(), config.as_bytes())])?;
            let path = out_dir.join("config.img");
            write_atomic(&path, &img)?;
            Ok(Some(ConfigMedia::Disk(path)))
        }
        ConfigDelivery::CloudInit => {
            let user_data = if config.starts_with("#cloud-config") || config.starts_with("#!") {
                config.to_string()
            } else {
                // Bare text: treat as a shell provisioning script.
                format!("#!/bin/sh\n{config}\n")
            };
            let meta_data = format!("instance-id: {node_name}\nlocal-hostname: {node_name}\n");
            let iso = build_iso(
                "cidata",
                &[
                    ("user-data", user_data.as_bytes()),
                    ("meta-data", meta_data.as_bytes()),
                ],
            )?;
            let path = out_dir.join("seed.iso");
            write_atomic(&path, &iso)?;
            Ok(Some(ConfigMedia::Cdrom(path)))
        }
    }
}

fn write_atomic(path: &Path, data: &[u8]) -> Result<()> {
    let tmp: PathBuf = path.with_extension("tmp");
    std::fs::write(&tmp, data)?;
    std::fs::rename(&tmp, path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;

    #[test]
    fn iso_structure_valid() {
        let iso = build_iso(
            "cidata",
            &[("user-data", b"#cloud-config\n"), ("meta-data", b"id: x\n")],
        )
        .unwrap();
        // PVD at sector 16
        assert_eq!(&iso[16 * SECTOR + 1..16 * SECTOR + 6], b"CD001");
        assert_eq!(iso[16 * SECTOR], 1);
        // volume id
        assert_eq!(&iso[16 * SECTOR + 40..16 * SECTOR + 46], b"cidata");
        // terminator at 17
        assert_eq!(iso[17 * SECTOR], 255);
        // root dir "." record at sector 20 marks a directory
        assert_eq!(iso[20 * SECTOR + 25] & 0x02, 0x02);
        // Rock Ridge NM with exact name present in root dir sector
        let root = &iso[20 * SECTOR..21 * SECTOR];
        let has_nm = root
            .windows(14)
            .any(|w| &w[..2] == b"NM" && w.ends_with(b"user-data"));
        assert!(has_nm, "NM entry with exact filename must exist");
        // file contents at sector 21 and 22
        assert_eq!(&iso[21 * SECTOR..21 * SECTOR + 14], b"#cloud-config\n");
        assert_eq!(&iso[22 * SECTOR..22 * SECTOR + 6], b"id: x\n");
        // whole image is sector aligned and matches PVD size
        assert_eq!(iso.len() % SECTOR, 0);
        let total = u32::from_le_bytes(iso[16 * SECTOR + 80..16 * SECTOR + 84].try_into().unwrap());
        assert_eq!(iso.len(), total as usize * SECTOR);
    }

    #[test]
    fn fat_disk_roundtrip() {
        let img = build_fat_disk("CONFIG", &[("ios_config.txt", b"hostname R1\n")]).unwrap();
        let cursor = fscommon::BufStream::new(std::io::Cursor::new(img));
        let fs = fatfs::FileSystem::new(cursor, fatfs::FsOptions::new()).unwrap();
        let mut f = fs.root_dir().open_file("ios_config.txt").unwrap();
        let mut s = String::new();
        f.read_to_string(&mut s).unwrap();
        assert_eq!(s, "hostname R1\n");
    }

    #[test]
    fn media_dispatch() {
        let dir = tempfile::tempdir().unwrap();
        // no config -> no media
        assert!(
            build_config_media(&ConfigDelivery::CloudInit, "h1", None, dir.path())
                .unwrap()
                .is_none()
        );

        let m = build_config_media(
            &ConfigDelivery::CdromIso {
                filename: "iosxe_config.txt".into(),
            },
            "R1",
            Some("hostname R1"),
            dir.path(),
        )
        .unwrap()
        .unwrap();
        matches!(m, ConfigMedia::Cdrom(_));

        let m = build_config_media(
            &ConfigDelivery::FatDisk {
                filename: "ios_config.txt".into(),
            },
            "R1",
            Some("hostname R1"),
            dir.path(),
        )
        .unwrap()
        .unwrap();
        matches!(m, ConfigMedia::Disk(_));
    }
}
