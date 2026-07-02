//! Privileged-mode Linux plumbing: tap + bridge datapath, NAT/management
//! networks, and cloud bridging to host interfaces.
//!
//! Used when netpilot runs with CAP_NET_ADMIN. The rootless default is the
//! [`crate::switch::UdpSwitch`]; this module exists for wire-speed labs and
//! for networks that must reach the host/outside world.

use std::sync::Arc;

use crate::runner::{Result, Runner};

pub struct Plumbing {
    runner: Arc<dyn Runner>,
}

impl Plumbing {
    pub fn new(runner: Arc<dyn Runner>) -> Self {
        Self { runner }
    }

    /// Create a bridge (idempotent) and bring it up.
    pub async fn ensure_bridge(&self, name: &str) -> Result<()> {
        // `ip link add` fails if it exists; tolerate that and just set up.
        let _ = self
            .runner
            .run("ip", &["link", "add", "name", name, "type", "bridge"])
            .await?;
        self.runner
            .run_ok("ip", &["link", "set", name, "up"])
            .await?;
        // Make the bridge forgiving for routing-protocol labs.
        let _ = self
            .runner
            .run(
                "sh",
                &[
                    "-c",
                    &format!(
                        "echo 0 > /sys/class/net/{name}/bridge/multicast_snooping; \
                         echo 0x4000 > /sys/class/net/{name}/bridge/group_fwd_mask"
                    ),
                ],
            )
            .await;
        Ok(())
    }

    pub async fn delete_bridge(&self, name: &str) -> Result<()> {
        self.runner.run_ok("ip", &["link", "del", name]).await
    }

    /// Create a tap device owned by `user` (if given), disable offloads,
    /// and enslave it to a bridge.
    pub async fn create_tap(&self, tap: &str, bridge: &str, user: Option<&str>) -> Result<()> {
        let mut args = vec!["tuntap", "add", "dev", tap, "mode", "tap"];
        if let Some(u) = user {
            args.extend_from_slice(&["user", u]);
        }
        self.runner.run_ok("ip", &args).await?;
        self.runner
            .run_ok("ip", &["link", "set", tap, "up", "mtu", "9500"])
            .await?;
        // Guests must not see partial checksums.
        let _ = self
            .runner
            .run(
                "ethtool",
                &[
                    "-K", tap, "tx", "off", "rx", "off", "tso", "off", "gso", "off", "gro", "off",
                ],
            )
            .await;
        self.runner
            .run_ok("ip", &["link", "set", tap, "master", bridge])
            .await
    }

    pub async fn delete_tap(&self, tap: &str) -> Result<()> {
        self.runner.run_ok("ip", &["link", "del", tap]).await
    }

    /// Attach a host physical interface to a bridge (cloud network).
    pub async fn attach_host_iface(&self, iface: &str, bridge: &str) -> Result<()> {
        self.runner
            .run_ok("ip", &["link", "set", iface, "master", bridge])
            .await
    }

    /// Give the bridge a gateway address and masquerade its subnet
    /// (NAT / management networks).
    pub async fn enable_nat(&self, bridge: &str, gateway_cidr: &str, subnet: &str) -> Result<()> {
        self.runner
            .run_ok("ip", &["addr", "replace", gateway_cidr, "dev", bridge])
            .await?;
        self.runner
            .run_ok("sysctl", &["-w", "net.ipv4.ip_forward=1"])
            .await?;
        // nftables: one table for netpilot, idempotent recreate of the rule.
        let _ = self
            .runner
            .run("nft", &["add", "table", "ip", "netpilot"])
            .await;
        let _ = self
            .runner
            .run(
                "nft",
                &[
                    "add",
                    "chain",
                    "ip",
                    "netpilot",
                    "postrouting",
                    "{",
                    "type",
                    "nat",
                    "hook",
                    "postrouting",
                    "priority",
                    "srcnat",
                    ";",
                    "}",
                ],
            )
            .await;
        self.runner
            .run_ok(
                "nft",
                &[
                    "add",
                    "rule",
                    "ip",
                    "netpilot",
                    "postrouting",
                    "ip",
                    "saddr",
                    subnet,
                    "masquerade",
                ],
            )
            .await
    }

    /// Apply tc netem impairment on a tap (bridge-mode link quality).
    pub async fn set_netem(
        &self,
        tap: &str,
        delay_ms: u32,
        jitter_ms: u32,
        loss_pct: f32,
        rate_kbit: u32,
    ) -> Result<()> {
        let _ = self
            .runner
            .run("tc", &["qdisc", "del", "dev", tap, "root"])
            .await;
        if delay_ms == 0 && jitter_ms == 0 && loss_pct == 0.0 && rate_kbit == 0 {
            return Ok(());
        }
        let delay = format!("{delay_ms}ms");
        let jitter = format!("{jitter_ms}ms");
        let loss = format!("{loss_pct}%");
        let rate = format!("{rate_kbit}kbit");
        let mut args: Vec<&str> = vec!["qdisc", "add", "dev", tap, "root", "netem"];
        if delay_ms > 0 || jitter_ms > 0 {
            args.extend_from_slice(&["delay", &delay]);
            if jitter_ms > 0 {
                args.push(&jitter);
            }
        }
        if loss_pct > 0.0 {
            args.extend_from_slice(&["loss", &loss]);
        }
        if rate_kbit > 0 {
            args.extend_from_slice(&["rate", &rate]);
        }
        self.runner.run_ok("tc", &args).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runner::RecordingRunner;

    #[tokio::test]
    async fn commands_are_well_formed() {
        let rec = Arc::new(RecordingRunner::default());
        let p = Plumbing::new(rec.clone());

        p.ensure_bridge("npb-abc").await.unwrap();
        p.create_tap("npt-x1", "npb-abc", Some("labd"))
            .await
            .unwrap();
        p.enable_nat("npn-mgmt", "10.99.0.1/24", "10.99.0.0/24")
            .await
            .unwrap();
        p.set_netem("npt-x1", 50, 5, 1.0, 10000).await.unwrap();

        let calls = rec.calls.lock().unwrap().join("\n");
        assert!(calls.contains("ip link add name npb-abc type bridge"));
        assert!(calls.contains("ip tuntap add dev npt-x1 mode tap user labd"));
        assert!(calls.contains("ip link set npt-x1 master npb-abc"));
        assert!(calls.contains("masquerade"));
        assert!(calls.contains("netem delay 50ms 5ms loss 1% rate 10000kbit"));
    }
}
