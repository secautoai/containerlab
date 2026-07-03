//! Deterministic, collision-resistant interface names.
//!
//! Linux interface names are limited to 15 characters, so raw UUIDs don't
//! fit. We derive short stable hashes from (lab, entity, ...) tuples.

use uuid::Uuid;

fn short_hash(parts: &[&str]) -> String {
    // FNV-1a: stable across runs (unlike DefaultHasher, which is randomized
    // per-process for HashMap DoS resistance — names must survive restarts).
    let mut h: u64 = 0xcbf29ce484222325;
    for p in parts {
        for b in p.as_bytes() {
            h ^= *b as u64;
            h = h.wrapping_mul(0x100000001b3);
        }
        h ^= 0xff;
        h = h.wrapping_mul(0x100000001b3);
    }
    // 40 bits -> exactly 10 hex chars, keeping names within IFNAMSIZ.
    format!("{:010x}", h & 0xff_ffff_ffff)
}

/// Bridge for a point-to-point link: `npb-<hash>` (14 chars).
pub fn link_bridge(lab: Uuid, link: Uuid) -> String {
    format!("npb-{}", short_hash(&[&lab.to_string(), &link.to_string()]))
}

/// Bridge for a multipoint network: `npn-<hash>`.
pub fn network_bridge(lab: Uuid, network: Uuid) -> String {
    format!(
        "npn-{}",
        short_hash(&[&lab.to_string(), &network.to_string()])
    )
}

/// Tap for a node interface: `npt-<hash>`.
pub fn node_tap(lab: Uuid, node: Uuid, iface: u32) -> String {
    format!(
        "npt-{}",
        short_hash(&[&lab.to_string(), &node.to_string(), &iface.to_string()])
    )
}

/// Deterministic locally-administered MAC for a node interface.
/// Prefix 52:54 is the conventional QEMU OUI space.
pub fn node_mac(lab: Uuid, node: Uuid, iface: u32) -> String {
    let hex = short_hash(&[
        "mac",
        &lab.to_string(),
        &node.to_string(),
        &iface.to_string(),
    ]);
    let b = &hex.as_bytes()[..8];
    format!(
        "52:54:00:{}{}:{}{}:{}{}",
        b[0] as char, b[1] as char, b[2] as char, b[3] as char, b[4] as char, b[5] as char
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_fit_ifnamsiz() {
        let lab = Uuid::new_v4();
        let node = Uuid::new_v4();
        assert!(link_bridge(lab, node).len() <= 15);
        assert!(network_bridge(lab, node).len() <= 15);
        assert!(node_tap(lab, node, 3).len() <= 15);
    }

    #[test]
    fn names_deterministic_and_distinct() {
        let lab = Uuid::parse_str("6b0efc61-1111-2222-3333-444455556666").unwrap();
        let node = Uuid::parse_str("aaaaaaaa-bbbb-cccc-dddd-eeeeffff0000").unwrap();
        assert_eq!(node_tap(lab, node, 0), node_tap(lab, node, 0));
        assert_ne!(node_tap(lab, node, 0), node_tap(lab, node, 1));
        assert_ne!(link_bridge(lab, node), network_bridge(lab, node));
    }

    #[test]
    fn mac_format() {
        let lab = Uuid::new_v4();
        let node = Uuid::new_v4();
        let mac = node_mac(lab, node, 0);
        assert_eq!(mac.len(), 17);
        assert!(mac.starts_with("52:54:00:"));
        assert_eq!(mac, node_mac(lab, node, 0));
    }
}
