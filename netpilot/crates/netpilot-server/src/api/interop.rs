//! Import/export: NetPilot zip, EVE-NG .unl XML, containerlab YAML.

use std::collections::{BTreeMap, HashMap};
use std::io::{Cursor, Read, Write};

use axum::body::Bytes;
use axum::extract::{Path, State};
use axum::http::header;
use axum::response::IntoResponse;
use axum::Json;
use netpilot_core::{Endpoint, Event, Lab, Link, Network, NetworkKind, Node};
use uuid::Uuid;

use crate::error::{ApiError, ApiResult};
use crate::state::AppState;

// ---------- export ----------

pub async fn export_lab(
    State(state): State<AppState>,
    Path(lab_id): Path<Uuid>,
) -> ApiResult<impl IntoResponse> {
    let lab = state.store.load(lab_id)?;
    let yaml = serde_yaml::to_string(&lab).map_err(|e| ApiError::internal(e.to_string()))?;

    let mut zip_buf = Cursor::new(Vec::new());
    {
        let mut zip = zip::ZipWriter::new(&mut zip_buf);
        let opts = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);
        zip.start_file("lab.yaml", opts)
            .map_err(|e| ApiError::internal(e.to_string()))?;
        zip.write_all(yaml.as_bytes())?;
        zip.finish().map_err(|e| ApiError::internal(e.to_string()))?;
    }

    let filename = format!(
        "attachment; filename=\"{}.zip\"",
        lab.name.replace(|c: char| !c.is_alphanumeric() && c != '-' && c != '_', "_")
    );
    Ok((
        [
            (header::CONTENT_TYPE, "application/zip".to_string()),
            (header::CONTENT_DISPOSITION, filename),
        ],
        zip_buf.into_inner(),
    ))
}

// ---------- import ----------

/// Accepts a NetPilot export zip, a bare lab.yaml, an EVE-NG .unl XML
/// document, or a containerlab topology YAML. Format is sniffed.
pub async fn import_lab(
    State(state): State<AppState>,
    body: Bytes,
) -> ApiResult<Json<Lab>> {
    if body.is_empty() {
        return Err(ApiError::bad_request("empty import body"));
    }

    let mut lab = if body.starts_with(b"PK") {
        import_zip(&body)?
    } else {
        let text = String::from_utf8_lossy(&body);
        let trimmed = text.trim_start();
        if trimmed.starts_with("<?xml") || trimmed.starts_with("<lab") {
            import_unl(&text)?
        } else if text.contains("topology:") && text.contains("nodes:") && !text.contains("modified_at") {
            import_clab(&text)?
        } else {
            serde_yaml::from_str::<Lab>(&text)
                .map_err(|e| ApiError::bad_request(format!("unrecognized lab format: {e}")))?
        }
    };

    // Always a fresh identity so imports never clobber existing labs.
    lab.id = Uuid::new_v4();
    lab.created_at = chrono::Utc::now();
    lab.touch();
    state.store.save(&lab)?;
    state.events.publish(Event::LabCreated { lab: lab.id });
    Ok(Json(lab))
}

fn import_zip(bytes: &[u8]) -> ApiResult<Lab> {
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes))
        .map_err(|e| ApiError::bad_request(format!("bad zip: {e}")))?;
    for i in 0..archive.len() {
        let mut file = archive
            .by_index(i)
            .map_err(|e| ApiError::bad_request(e.to_string()))?;
        let name = file.name().to_string();
        if name.ends_with("lab.yaml") || name.ends_with(".unl") {
            let mut text = String::new();
            file.read_to_string(&mut text)
                .map_err(|e| ApiError::bad_request(e.to_string()))?;
            return if name.ends_with(".unl") {
                import_unl(&text)
            } else {
                serde_yaml::from_str(&text)
                    .map_err(|e| ApiError::bad_request(format!("bad lab.yaml: {e}")))
            };
        }
    }
    Err(ApiError::bad_request("zip contains no lab.yaml or .unl"))
}

// ---------- EVE-NG .unl ----------

mod unl {
    //! Serde model of the EVE-NG .unl XML schema (subset we import).
    use serde::Deserialize;

    #[derive(Debug, Deserialize)]
    pub struct UnlLab {
        #[serde(rename = "@name", default)]
        pub name: String,
        #[serde(default)]
        pub description: Option<String>,
        #[serde(default)]
        pub topology: Option<Topology>,
        #[serde(default)]
        pub objects: Option<Objects>,
    }

    #[derive(Debug, Default, Deserialize)]
    pub struct Topology {
        #[serde(default)]
        pub nodes: Nodes,
        #[serde(default)]
        pub networks: Networks,
    }

    #[derive(Debug, Default, Deserialize)]
    pub struct Nodes {
        #[serde(rename = "node", default)]
        pub items: Vec<UnlNode>,
    }

    #[derive(Debug, Default, Deserialize)]
    pub struct Networks {
        #[serde(rename = "network", default)]
        pub items: Vec<UnlNetwork>,
    }

    #[derive(Debug, Deserialize)]
    pub struct UnlNode {
        #[serde(rename = "@id")]
        pub id: u32,
        #[serde(rename = "@name", default)]
        pub name: String,
        #[serde(rename = "@template", default)]
        pub template: String,
        #[serde(rename = "@image", default)]
        pub image: String,
        #[serde(rename = "@cpu", default)]
        pub cpu: Option<u32>,
        #[serde(rename = "@ram", default)]
        pub ram: Option<u32>,
        #[serde(rename = "@ethernet", default)]
        pub ethernet: Option<u32>,
        #[serde(rename = "@left", default)]
        pub left: Option<String>,
        #[serde(rename = "@top", default)]
        pub top: Option<String>,
        #[serde(rename = "interface", default)]
        pub interfaces: Vec<UnlInterface>,
    }

    #[derive(Debug, Deserialize)]
    pub struct UnlInterface {
        #[serde(rename = "@id")]
        pub id: u32,
        #[serde(rename = "@network_id", default)]
        pub network_id: Option<u32>,
    }

    #[derive(Debug, Deserialize)]
    pub struct UnlNetwork {
        #[serde(rename = "@id")]
        pub id: u32,
        #[serde(rename = "@type", default)]
        pub kind: String,
        #[serde(rename = "@name", default)]
        pub name: String,
        #[serde(rename = "@left", default)]
        pub left: Option<String>,
        #[serde(rename = "@top", default)]
        pub top: Option<String>,
        #[serde(rename = "@visibility", default)]
        pub visibility: Option<String>,
    }

    #[derive(Debug, Default, Deserialize)]
    pub struct Objects {
        #[serde(default)]
        pub configs: Option<Configs>,
    }

    #[derive(Debug, Default, Deserialize)]
    pub struct Configs {
        #[serde(rename = "config", default)]
        pub items: Vec<UnlConfig>,
    }

    #[derive(Debug, Deserialize)]
    pub struct UnlConfig {
        #[serde(rename = "@id")]
        pub id: u32,
        #[serde(rename = "$text", default)]
        pub data: String,
    }
}

/// EVE template id → NetPilot template id (best effort).
fn map_eve_template(t: &str) -> &'static str {
    match t {
        "vios" | "iosv" => "iosv",
        "viosl2" | "iosvl2" => "iosvl2",
        "csr1000v" | "csr1000vng" => "csr1000v",
        "c8000v" | "cat8000v" => "cat8000v",
        "xrv9k" => "xrv9k",
        "veos" => "veos",
        "vsrx" | "vsrxng" => "vsrx",
        "vjunosswitch" => "vjunos-switch",
        "fortinet" | "fortigate" => "fortigate",
        "paloalto" | "pa" => "panos",
        "mikrotik" | "chr" => "chr",
        "vyos" => "vyos",
        _ => "linux",
    }
}

fn import_unl(xml: &str) -> ApiResult<Lab> {
    let parsed: unl::UnlLab = quick_xml::de::from_str(xml)
        .map_err(|e| ApiError::bad_request(format!("bad .unl XML: {e}")))?;

    let mut lab = Lab::new(if parsed.name.is_empty() {
        "Imported EVE-NG lab".to_string()
    } else {
        parsed.name.clone()
    });
    lab.description = parsed
        .description
        .unwrap_or_else(|| "Imported from EVE-NG .unl".into());

    let topo = parsed.topology.unwrap_or_else(|| unl::Topology {
        nodes: Default::default(),
        networks: Default::default(),
    });

    // Configs by EVE node id (base64 or plain).
    let mut configs: HashMap<u32, String> = HashMap::new();
    if let Some(objects) = parsed.objects {
        if let Some(cfgs) = objects.configs {
            for c in cfgs.items {
                let text = decode_b64(&c.data).unwrap_or(c.data);
                configs.insert(c.id, text);
            }
        }
    }

    // Nodes.
    let mut node_ids: HashMap<u32, Uuid> = HashMap::new();
    for (i, n) in topo.nodes.items.iter().enumerate() {
        let template = map_eve_template(&n.template);
        let id = Uuid::new_v4();
        node_ids.insert(n.id, id);
        lab.nodes.insert(
            id,
            Node {
                id,
                name: if n.name.is_empty() {
                    format!("N{}", n.id)
                } else {
                    n.name.clone()
                },
                template: template.into(),
                image: n.image.clone(),
                cpus: n.cpu.unwrap_or(1).max(1),
                ram_mb: n.ram.unwrap_or(1024).max(128),
                interfaces: n.ethernet.unwrap_or(4).max(1),
                console: Default::default(),
                icon: String::new(),
                x: n.left.as_deref().and_then(parse_px).unwrap_or(100.0 + (i as f64 % 6.0) * 160.0),
                y: n.top.as_deref().and_then(parse_px).unwrap_or(100.0 + (i as f64 / 6.0).floor() * 140.0),
                startup_config: configs.get(&n.id).cloned(),
                boot_delay_s: 0,
                overrides: BTreeMap::new(),
            },
        );
    }

    // Networks: hidden 2-endpoint bridges become p2p links; the rest become
    // visible network objects.
    let mut net_endpoints: HashMap<u32, Vec<(Uuid, u32)>> = HashMap::new();
    for n in &topo.nodes.items {
        let Some(&nid) = node_ids.get(&n.id) else { continue };
        for itf in &n.interfaces {
            if let Some(net) = itf.network_id {
                net_endpoints.entry(net).or_default().push((nid, itf.id));
            }
        }
    }

    for net in &topo.networks.items {
        let endpoints = net_endpoints.remove(&net.id).unwrap_or_default();
        let hidden = net.visibility.as_deref() == Some("0");
        if hidden && endpoints.len() == 2 {
            let link = Link::between(
                Endpoint::Node {
                    node: endpoints[0].0,
                    iface: endpoints[0].1,
                },
                Endpoint::Node {
                    node: endpoints[1].0,
                    iface: endpoints[1].1,
                },
            );
            lab.links.insert(link.id, link);
            continue;
        }
        let kind = if net.kind.starts_with("pnet") {
            NetworkKind::Cloud
        } else if net.kind == "nat0" || net.kind == "nat" {
            NetworkKind::Nat
        } else {
            NetworkKind::Bridge
        };
        let id = Uuid::new_v4();
        lab.networks.insert(
            id,
            Network {
                id,
                name: if net.name.is_empty() {
                    format!("Net{}", net.id)
                } else {
                    net.name.clone()
                },
                kind,
                host_interface: None,
                subnet: None,
                x: net.left.as_deref().and_then(parse_px).unwrap_or(400.0),
                y: net.top.as_deref().and_then(parse_px).unwrap_or(400.0),
            },
        );
        for (node, iface) in endpoints {
            let link = Link::between(
                Endpoint::Node { node, iface },
                Endpoint::Network { network: id },
            );
            lab.links.insert(link.id, link);
        }
    }

    Ok(lab)
}

fn parse_px(s: &str) -> Option<f64> {
    s.trim_end_matches("px").trim_end_matches('%').parse().ok()
}

fn decode_b64(s: &str) -> Option<String> {
    // Tiny base64 decoder (standard alphabet) — avoids a dependency.
    let cleaned: Vec<u8> = s.bytes().filter(|b| !b.is_ascii_whitespace()).collect();
    if cleaned.is_empty() || cleaned.len() % 4 != 0 {
        return None;
    }
    let val = |c: u8| -> Option<u32> {
        match c {
            b'A'..=b'Z' => Some((c - b'A') as u32),
            b'a'..=b'z' => Some((c - b'a') as u32 + 26),
            b'0'..=b'9' => Some((c - b'0') as u32 + 52),
            b'+' => Some(62),
            b'/' => Some(63),
            _ => None,
        }
    };
    let mut out = Vec::new();
    for chunk in cleaned.chunks(4) {
        let pad = chunk.iter().filter(|&&c| c == b'=').count();
        let mut acc: u32 = 0;
        for &c in chunk {
            acc = (acc << 6) | if c == b'=' { 0 } else { val(c)? };
        }
        out.push((acc >> 16) as u8);
        if pad < 2 {
            out.push((acc >> 8) as u8);
        }
        if pad < 1 {
            out.push(acc as u8);
        }
    }
    String::from_utf8(out).ok()
}

// ---------- containerlab ----------

mod clab {
    use serde::Deserialize;
    use std::collections::BTreeMap;

    #[derive(Debug, Deserialize)]
    pub struct ClabTopo {
        #[serde(default)]
        pub name: String,
        pub topology: Topology,
    }

    #[derive(Debug, Deserialize)]
    pub struct Topology {
        #[serde(default)]
        pub nodes: BTreeMap<String, ClabNode>,
        #[serde(default)]
        pub links: Vec<ClabLink>,
    }

    #[derive(Debug, Default, Deserialize)]
    pub struct ClabNode {
        #[serde(default)]
        pub kind: Option<String>,
        #[serde(default)]
        pub image: Option<String>,
    }

    #[derive(Debug, Deserialize)]
    pub struct ClabLink {
        pub endpoints: Vec<String>,
    }
}

/// containerlab kind → NetPilot template (QEMU equivalents where they exist).
fn map_clab_kind(kind: &str) -> &'static str {
    match kind {
        k if k.contains("vr-csr") || k.contains("cisco_csr") => "csr1000v",
        k if k.contains("vr-veos") || k.contains("ceos") || k.contains("arista") => "veos",
        k if k.contains("vr-vsrx") || k.contains("crpd") || k.contains("juniper") => "vsrx",
        k if k.contains("vjunos") => "vjunos-switch",
        k if k.contains("fortinet") => "fortigate",
        k if k.contains("panos") || k.contains("paloalto") => "panos",
        k if k.contains("vyos") => "vyos",
        k if k.contains("vr-xrv") || k.contains("xrd") || k.contains("cisco_xr") => "xrv9k",
        k if k.contains("mikrotik") || k.contains("vr-ros") => "chr",
        k if k.contains("frr") => "frr",
        _ => "linux",
    }
}

fn import_clab(yaml: &str) -> ApiResult<Lab> {
    let parsed: clab::ClabTopo = serde_yaml::from_str(yaml)
        .map_err(|e| ApiError::bad_request(format!("bad containerlab YAML: {e}")))?;

    let mut lab = Lab::new(if parsed.name.is_empty() {
        "Imported containerlab topology".to_string()
    } else {
        parsed.name.clone()
    });
    lab.description = "Imported from containerlab (.clab.yml)".into();

    let mut by_name: HashMap<String, Uuid> = HashMap::new();
    for (i, (name, cn)) in parsed.topology.nodes.iter().enumerate() {
        let template = map_clab_kind(cn.kind.as_deref().unwrap_or(""));
        let id = Uuid::new_v4();
        by_name.insert(name.clone(), id);
        lab.nodes.insert(
            id,
            Node {
                id,
                name: name.clone(),
                template: template.into(),
                image: cn.image.clone().unwrap_or_default(),
                cpus: 1,
                ram_mb: 1024,
                interfaces: 8,
                console: Default::default(),
                icon: String::new(),
                x: 120.0 + (i as f64 % 5.0) * 180.0,
                y: 120.0 + (i as f64 / 5.0).floor() * 150.0,
                startup_config: None,
                boot_delay_s: 0,
                overrides: BTreeMap::new(),
            },
        );
    }

    for l in &parsed.topology.links {
        if l.endpoints.len() != 2 {
            continue;
        }
        let mut eps = Vec::new();
        for ep in &l.endpoints {
            let Some((node_name, iface)) = ep.split_once(':') else { continue };
            let Some(&node) = by_name.get(node_name) else { continue };
            // eth1 / e1-1 / Gi0-0 → take trailing digits as index
            let idx: u32 = iface
                .chars()
                .rev()
                .take_while(|c| c.is_ascii_digit())
                .collect::<String>()
                .chars()
                .rev()
                .collect::<String>()
                .parse()
                .unwrap_or(0);
            eps.push(Endpoint::Node { node, iface: idx });
        }
        if eps.len() == 2 {
            let link = Link::between(eps[0], eps[1]);
            // add_link validates ranges; loosen on conflict by skipping
            let _ = lab.add_link(link);
        }
    }

    Ok(lab)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unl_import_maps_nodes_networks_links() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<lab name="ospf-triangle" version="1" scripttimeout="300" lock="0">
  <topology>
    <nodes>
      <node id="1" name="R1" type="qemu" template="vios" image="vios-15.6" console="telnet" cpu="1" ram="1024" ethernet="4" left="100" top="100">
        <interface id="0" name="Gi0/0" type="ethernet" network_id="1"/>
      </node>
      <node id="2" name="R2" type="qemu" template="vios" image="vios-15.6" console="telnet" ram="1024" ethernet="4" left="300" top="100">
        <interface id="0" name="Gi0/0" type="ethernet" network_id="1"/>
        <interface id="1" name="Gi0/1" type="ethernet" network_id="2"/>
      </node>
      <node id="3" name="SW1" type="qemu" template="viosl2" image="viosl2" ram="1024" ethernet="8" left="500" top="100">
        <interface id="0" name="Gi0/0" type="ethernet" network_id="2"/>
      </node>
    </nodes>
    <networks>
      <network id="1" type="bridge" name="" left="200" top="100" visibility="0"/>
      <network id="2" type="bridge" name="LAN" left="400" top="100" visibility="1"/>
    </networks>
  </topology>
  <objects>
    <configs>
      <config id="1">aG9zdG5hbWUgUjEK</config>
    </configs>
  </objects>
</lab>"#;
        let lab = import_unl(xml).unwrap();
        assert_eq!(lab.name, "ospf-triangle");
        assert_eq!(lab.nodes.len(), 3);
        // hidden 2-endpoint bridge -> p2p link; visible one -> network + 2 links
        assert_eq!(lab.networks.len(), 1);
        assert_eq!(lab.links.len(), 3);
        let r1 = lab.nodes.values().find(|n| n.name == "R1").unwrap();
        assert_eq!(r1.template, "iosv");
        assert_eq!(r1.startup_config.as_deref(), Some("hostname R1\n"));
        let sw = lab.nodes.values().find(|n| n.name == "SW1").unwrap();
        assert_eq!(sw.template, "iosvl2");
    }

    #[test]
    fn clab_import_maps_kinds_and_links() {
        let yaml = r#"
name: srl-ceos
topology:
  nodes:
    srl1:
      kind: nokia_srlinux
      image: ghcr.io/nokia/srlinux
    eos1:
      kind: ceos
      image: ceos:4.32
  links:
    - endpoints: ["srl1:e1-1", "eos1:eth1"]
"#;
        let lab = import_clab(yaml).unwrap();
        assert_eq!(lab.name, "srl-ceos");
        assert_eq!(lab.nodes.len(), 2);
        assert_eq!(lab.links.len(), 1);
        let eos = lab.nodes.values().find(|n| n.name == "eos1").unwrap();
        assert_eq!(eos.template, "veos");
    }

    #[test]
    fn b64_decoder() {
        assert_eq!(decode_b64("aG9zdG5hbWUgUjEK").unwrap(), "hostname R1\n");
        assert_eq!(decode_b64("YQ==").unwrap(), "a");
        assert!(decode_b64("!!!").is_none());
    }
}
