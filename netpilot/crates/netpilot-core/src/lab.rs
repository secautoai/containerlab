//! Lab topology model: labs, nodes, links, networks, annotations.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::{CoreError, Result};

/// A lab is a self-contained topology: nodes, networks connecting them,
/// and visual annotations. It is the unit of persistence and sharing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Lab {
    pub id: Uuid,
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub author: String,
    #[serde(default)]
    pub folder: String,
    #[serde(default = "default_version")]
    pub version: u32,
    pub created_at: DateTime<Utc>,
    pub modified_at: DateTime<Utc>,
    /// Nodes keyed by id. BTreeMap keeps serialization deterministic.
    #[serde(default)]
    pub nodes: BTreeMap<Uuid, Node>,
    /// Multipoint networks (bridges/clouds/NAT segments).
    #[serde(default)]
    pub networks: BTreeMap<Uuid, Network>,
    /// Point-to-point and node-to-network links.
    #[serde(default)]
    pub links: BTreeMap<Uuid, Link>,
    /// Free-form canvas annotations (text, shapes, images).
    #[serde(default)]
    pub annotations: BTreeMap<Uuid, Annotation>,
}

fn default_version() -> u32 {
    1
}

impl Lab {
    pub fn new(name: impl Into<String>) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            name: name.into(),
            description: String::new(),
            author: String::new(),
            folder: "/".into(),
            version: 1,
            created_at: now,
            modified_at: now,
            nodes: BTreeMap::new(),
            networks: BTreeMap::new(),
            links: BTreeMap::new(),
            annotations: BTreeMap::new(),
        }
    }

    pub fn touch(&mut self) {
        self.modified_at = Utc::now();
    }

    pub fn node(&self, id: Uuid) -> Result<&Node> {
        self.nodes
            .get(&id)
            .ok_or_else(|| CoreError::NodeNotFound(id.to_string()))
    }

    pub fn node_mut(&mut self, id: Uuid) -> Result<&mut Node> {
        self.nodes
            .get_mut(&id)
            .ok_or_else(|| CoreError::NodeNotFound(id.to_string()))
    }

    pub fn link(&self, id: Uuid) -> Result<&Link> {
        self.links
            .get(&id)
            .ok_or_else(|| CoreError::LinkNotFound(id.to_string()))
    }

    pub fn network(&self, id: Uuid) -> Result<&Network> {
        self.networks
            .get(&id)
            .ok_or_else(|| CoreError::NetworkNotFound(id.to_string()))
    }

    /// A unique node name for duplicates: R1 -> R2 -> R3 ...
    pub fn next_node_name(&self, prefix: &str) -> String {
        let mut n = 1;
        loop {
            let candidate = format!("{prefix}{n}");
            if !self.nodes.values().any(|node| node.name == candidate) {
                return candidate;
            }
            n += 1;
        }
    }

    /// Returns the link (if any) using the given node interface.
    pub fn link_on_interface(&self, node: Uuid, iface: u32) -> Option<&Link> {
        self.links.values().find(|l| l.uses_interface(node, iface))
    }

    /// First free interface index on a node, honoring its interface count.
    pub fn free_interface(&self, node_id: Uuid) -> Result<u32> {
        let node = self.node(node_id)?;
        for i in 0..node.interfaces {
            if self.link_on_interface(node_id, i).is_none() {
                return Ok(i);
            }
        }
        Err(CoreError::Validation(format!(
            "node {} has no free interfaces",
            node.name
        )))
    }

    /// Validate and insert a link.
    pub fn add_link(&mut self, link: Link) -> Result<Uuid> {
        for ep in link.endpoints() {
            match ep {
                Endpoint::Node { node, iface } => {
                    let n = self.node(*node)?;
                    if *iface >= n.interfaces {
                        return Err(CoreError::InvalidInterface {
                            node: n.name.clone(),
                            iface: *iface,
                            max: n.interfaces,
                        });
                    }
                    if self.link_on_interface(*node, *iface).is_some() {
                        return Err(CoreError::InterfaceBusy {
                            node: n.name.clone(),
                            iface: *iface,
                        });
                    }
                }
                Endpoint::Network { network } => {
                    self.network(*network)?;
                }
            }
        }
        if let (Endpoint::Network { .. }, Endpoint::Network { .. }) = (&link.a, &link.b) {
            return Err(CoreError::Validation(
                "cannot link a network directly to a network".into(),
            ));
        }
        let id = link.id;
        self.links.insert(id, link);
        self.touch();
        Ok(id)
    }

    /// Remove a node together with any links touching it.
    pub fn remove_node(&mut self, id: Uuid) -> Result<Node> {
        let node = self
            .nodes
            .remove(&id)
            .ok_or_else(|| CoreError::NodeNotFound(id.to_string()))?;
        self.links.retain(|_, l| !l.touches_node(id));
        self.touch();
        Ok(node)
    }

    /// Remove a network together with any links touching it.
    pub fn remove_network(&mut self, id: Uuid) -> Result<Network> {
        let net = self
            .networks
            .remove(&id)
            .ok_or_else(|| CoreError::NetworkNotFound(id.to_string()))?;
        self.links.retain(|_, l| !l.touches_network(id));
        self.touch();
        Ok(net)
    }
}

/// Runtime state of a node process. Not persisted with the lab; the
/// orchestrator owns it, but the type lives here so API/core share it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum NodeState {
    #[default]
    Stopped,
    Starting,
    Running,
    Stopping,
    Error,
}

/// How a user reaches the node's console.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ConsoleKind {
    /// Serial console exposed via telnet (proxied to the UI over WebSocket).
    #[default]
    Serial,
    /// Graphical console over VNC.
    Vnc,
}

/// A device in the topology.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Node {
    pub id: Uuid,
    pub name: String,
    /// Template this node was created from (e.g. "vyos", "ceos", "linux").
    pub template: String,
    /// Image name/version within the template's image family.
    #[serde(default)]
    pub image: String,
    /// vCPU count.
    pub cpus: u32,
    /// RAM in MiB.
    pub ram_mb: u32,
    /// Number of network interfaces.
    pub interfaces: u32,
    #[serde(default)]
    pub console: ConsoleKind,
    /// Icon key for the UI (router, switch, firewall, server, cloud...).
    #[serde(default)]
    pub icon: String,
    /// Canvas position.
    pub x: f64,
    pub y: f64,
    /// Optional startup configuration pushed on first boot.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub startup_config: Option<String>,
    /// Optional delay (seconds) before boot when starting the whole lab.
    #[serde(default)]
    pub boot_delay_s: u32,
    /// Free-form key/value overrides for the QEMU template (advanced users).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub overrides: BTreeMap<String, String>,
}

impl Node {
    /// Conventional interface name shown in the UI, derived from the
    /// template naming scheme (e.g. Gi0/0, eth0, ge-0/0/0).
    pub fn iface_name(&self, pattern: &str, index: u32) -> String {
        iface_name_from_pattern(pattern, index)
    }
}

/// Render an interface name from a template pattern.
/// Pattern syntax: `{i}` is replaced with the index (plus optional offset,
/// `{i+1}` style). Examples: "eth{i}", "Gi0/{i}", "ge-0/0/{i}".
pub fn iface_name_from_pattern(pattern: &str, index: u32) -> String {
    if let Some(start) = pattern.find("{i") {
        if let Some(end_rel) = pattern[start..].find('}') {
            let end = start + end_rel;
            let inner = &pattern[start + 2..end]; // "" or "+N"
            let offset: u32 = inner
                .strip_prefix('+')
                .and_then(|n| n.parse().ok())
                .unwrap_or(0);
            return format!(
                "{}{}{}",
                &pattern[..start],
                index + offset,
                &pattern[end + 1..]
            );
        }
    }
    format!("{pattern}{index}")
}

/// The kind of a multipoint network segment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum NetworkKind {
    /// Plain L2 bridge internal to the lab.
    #[default]
    Bridge,
    /// Bridge with NAT to the host's default route (internet access).
    Nat,
    /// Management network: NAT plus DHCP for node management interfaces.
    Management,
    /// Bridged to a physical/host interface (cloud).
    Cloud,
}

/// A multipoint network segment (rendered as a small node on the canvas).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Network {
    pub id: Uuid,
    pub name: String,
    pub kind: NetworkKind,
    /// Host interface name for `Cloud` kind.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub host_interface: Option<String>,
    /// CIDR subnet for NAT/Management kinds (e.g. "10.99.0.0/24").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subnet: Option<String>,
    pub x: f64,
    pub y: f64,
}

/// One end of a link.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Endpoint {
    Node { node: Uuid, iface: u32 },
    Network { network: Uuid },
}

/// Traffic impairment applied to a link (both directions).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Default)]
pub struct Impairment {
    /// One-way delay in milliseconds.
    #[serde(default)]
    pub delay_ms: u32,
    /// Jitter in milliseconds.
    #[serde(default)]
    pub jitter_ms: u32,
    /// Packet loss percentage (0-100).
    #[serde(default)]
    pub loss_pct: f32,
    /// Bandwidth cap in kbit/s (0 = unlimited).
    #[serde(default)]
    pub rate_kbit: u32,
}

impl Impairment {
    pub fn is_noop(&self) -> bool {
        self.delay_ms == 0 && self.jitter_ms == 0 && self.loss_pct == 0.0 && self.rate_kbit == 0
    }
}

/// A link between two endpoints (node-node or node-network).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Link {
    pub id: Uuid,
    pub a: Endpoint,
    pub b: Endpoint,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub impairment: Option<Impairment>,
}

impl Link {
    pub fn between(a: Endpoint, b: Endpoint) -> Self {
        Self {
            id: Uuid::new_v4(),
            a,
            b,
            label: None,
            impairment: None,
        }
    }

    pub fn endpoints(&self) -> [&Endpoint; 2] {
        [&self.a, &self.b]
    }

    pub fn touches_node(&self, id: Uuid) -> bool {
        self.endpoints()
            .iter()
            .any(|e| matches!(e, Endpoint::Node { node, .. } if *node == id))
    }

    pub fn touches_network(&self, id: Uuid) -> bool {
        self.endpoints()
            .iter()
            .any(|e| matches!(e, Endpoint::Network { network } if *network == id))
    }

    pub fn uses_interface(&self, node_id: Uuid, iface_idx: u32) -> bool {
        self.endpoints().iter().any(
            |e| matches!(e, Endpoint::Node { node, iface } if *node == node_id && *iface == iface_idx),
        )
    }
}

/// Visual annotation kinds on the canvas.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AnnotationKind {
    Text,
    Rect,
    Ellipse,
}

/// A canvas annotation (text label, colored region...).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Annotation {
    pub id: Uuid,
    pub kind: AnnotationKind,
    pub x: f64,
    pub y: f64,
    #[serde(default)]
    pub width: f64,
    #[serde(default)]
    pub height: f64,
    #[serde(default)]
    pub text: String,
    /// CSS color for stroke/text.
    #[serde(default)]
    pub color: String,
    /// CSS color for fill (shapes).
    #[serde(default)]
    pub fill: String,
    #[serde(default)]
    pub font_size: u32,
    /// Z-order: annotations render below nodes when negative.
    #[serde(default)]
    pub z: i32,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_node(name: &str) -> Node {
        Node {
            id: Uuid::new_v4(),
            name: name.into(),
            template: "linux".into(),
            image: "default".into(),
            cpus: 1,
            ram_mb: 512,
            interfaces: 4,
            console: ConsoleKind::Serial,
            icon: "server".into(),
            x: 0.0,
            y: 0.0,
            startup_config: None,
            boot_delay_s: 0,
            overrides: BTreeMap::new(),
        }
    }

    #[test]
    fn link_validation() {
        let mut lab = Lab::new("t");
        let n1 = test_node("a");
        let n2 = test_node("b");
        let (id1, id2) = (n1.id, n2.id);
        lab.nodes.insert(id1, n1);
        lab.nodes.insert(id2, n2);

        lab.add_link(Link::between(
            Endpoint::Node {
                node: id1,
                iface: 0,
            },
            Endpoint::Node {
                node: id2,
                iface: 0,
            },
        ))
        .unwrap();

        // same interface again -> busy
        let err = lab
            .add_link(Link::between(
                Endpoint::Node {
                    node: id1,
                    iface: 0,
                },
                Endpoint::Node {
                    node: id2,
                    iface: 1,
                },
            ))
            .unwrap_err();
        assert!(matches!(err, CoreError::InterfaceBusy { .. }));

        // out-of-range interface
        let err = lab
            .add_link(Link::between(
                Endpoint::Node {
                    node: id1,
                    iface: 9,
                },
                Endpoint::Node {
                    node: id2,
                    iface: 1,
                },
            ))
            .unwrap_err();
        assert!(matches!(err, CoreError::InvalidInterface { .. }));

        assert_eq!(lab.free_interface(id1).unwrap(), 1);

        // removing node removes its links
        lab.remove_node(id1).unwrap();
        assert!(lab.links.is_empty());
    }

    #[test]
    fn iface_patterns() {
        assert_eq!(iface_name_from_pattern("eth{i}", 0), "eth0");
        assert_eq!(iface_name_from_pattern("Gi0/{i}", 2), "Gi0/2");
        assert_eq!(iface_name_from_pattern("ge-0/0/{i}", 3), "ge-0/0/3");
        assert_eq!(iface_name_from_pattern("Ethernet{i+1}", 0), "Ethernet1");
    }

    #[test]
    fn unique_names() {
        let mut lab = Lab::new("t");
        let mut n = test_node("R1");
        n.name = "R1".into();
        lab.nodes.insert(n.id, n);
        assert_eq!(lab.next_node_name("R"), "R2");
    }
}
