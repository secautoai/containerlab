//! Server-side AI agent integration.
//!
//! Implements [`LabToolbox`] over [`AppState`] (the same code paths the
//! REST API uses) and runs the agent WebSocket protocol:
//! client sends `{"message": "..."}`, server streams `AgentEvent` JSON.

use std::collections::BTreeMap;
use std::time::Duration;

use async_trait::async_trait;
use axum::extract::ws::{Message, WebSocket};
use futures::{SinkExt, StreamExt};
use netpilot_ai::{AgentEvent, AgentSession, Claude, LabToolbox};
use netpilot_core::{ConsoleKind, Endpoint, Link, Network, NetworkKind, Node};
use serde_json::{json, Value};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::mpsc;
use uuid::Uuid;

use crate::state::AppState;

pub struct StateToolbox {
    state: AppState,
    lab_id: Uuid,
}

impl StateToolbox {
    fn find_node(&self, lab: &netpilot_core::Lab, name: &str) -> Result<Node, String> {
        lab.nodes
            .values()
            .find(|n| n.name.eq_ignore_ascii_case(name))
            .cloned()
            .ok_or_else(|| format!("no node named '{name}'"))
    }
}

#[async_trait]
impl LabToolbox for StateToolbox {
    async fn get_lab(&self) -> Result<Value, String> {
        let lab = self
            .state
            .store
            .load(self.lab_id)
            .map_err(|e| e.to_string())?;
        let states = self.state.lab_states(self.lab_id).await;
        let states_by_name: BTreeMap<String, String> = lab
            .nodes
            .values()
            .map(|n| {
                let s = states
                    .get(&n.id)
                    .map(|s| format!("{s:?}").to_lowercase())
                    .unwrap_or_else(|| "stopped".into());
                (n.name.clone(), s)
            })
            .collect();
        let mut v = serde_json::to_value(&lab).map_err(|e| e.to_string())?;
        v["runtime_states"] = json!(states_by_name);
        Ok(v)
    }

    async fn list_templates(&self) -> Result<Value, String> {
        let images = self.state.images.scan().unwrap_or_default();
        let catalog = self.state.templates.read().await;
        let list: Vec<Value> = catalog
            .all()
            .map(|t| {
                json!({
                    "id": t.id,
                    "name": t.name,
                    "vendor": t.vendor,
                    "cpus": t.cpus,
                    "ram_mb": t.ram_mb,
                    "interfaces": t.interfaces,
                    "iface_pattern": t.iface_pattern,
                    "notes": t.notes,
                    "config_guide": t.config_guide,
                    "available_images": images.iter().filter(|i| i.template == t.id)
                        .map(|i| i.version.clone()).collect::<Vec<_>>(),
                })
            })
            .collect();
        Ok(json!(list))
    }

    async fn create_node(&self, args: Value) -> Result<Value, String> {
        let template_id = args
            .get("template")
            .and_then(|v| v.as_str())
            .ok_or("missing template")?
            .to_string();
        let name = args
            .get("name")
            .and_then(|v| v.as_str())
            .ok_or("missing name")?
            .to_string();
        let catalog = self.state.templates.read().await;
        let template = catalog
            .get(&template_id)
            .map_err(|e| e.to_string())?
            .clone();
        drop(catalog);
        let images = self.state.images.scan().unwrap_or_default();
        let image = images
            .iter()
            .filter(|i| i.template == template.id)
            .map(|i| i.version.clone())
            .next_back()
            .unwrap_or_default();

        let node = self
            .state
            .mutate_lab(self.lab_id, |lab| {
                if lab.nodes.values().any(|n| n.name == name) {
                    return Err(crate::error::ApiError::conflict(format!(
                        "node '{name}' already exists"
                    )));
                }
                let node = Node {
                    id: Uuid::new_v4(),
                    name: name.clone(),
                    template: template.id.clone(),
                    image,
                    cpus: args
                        .get("cpus")
                        .and_then(|v| v.as_u64())
                        .unwrap_or(template.cpus as u64) as u32,
                    ram_mb: args
                        .get("ram_mb")
                        .and_then(|v| v.as_u64())
                        .unwrap_or(template.ram_mb as u64) as u32,
                    interfaces: args
                        .get("interfaces")
                        .and_then(|v| v.as_u64())
                        .unwrap_or(template.interfaces as u64)
                        as u32,
                    console: template.console,
                    icon: template.icon.clone(),
                    x: args.get("x").and_then(|v| v.as_f64()).unwrap_or(200.0),
                    y: args.get("y").and_then(|v| v.as_f64()).unwrap_or(200.0),
                    startup_config: args
                        .get("startup_config")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string()),
                    config_sets: BTreeMap::new(),
                    boot_delay_s: 0,
                    overrides: BTreeMap::new(),
                };
                lab.nodes.insert(node.id, node.clone());
                Ok(node)
            })
            .await
            .map_err(|e| e.message)?;
        Ok(json!({"created": node.name, "id": node.id, "interfaces": node.interfaces}))
    }

    async fn update_node(&self, args: Value) -> Result<Value, String> {
        let name = args
            .get("name")
            .and_then(|v| v.as_str())
            .ok_or("missing name")?
            .to_string();
        let lab = self
            .state
            .store
            .load(self.lab_id)
            .map_err(|e| e.to_string())?;
        let node = self.find_node(&lab, &name)?;
        self.state
            .mutate_lab(self.lab_id, |lab| {
                let n = lab.node_mut(node.id)?;
                if let Some(v) = args.get("x").and_then(|v| v.as_f64()) {
                    n.x = v;
                }
                if let Some(v) = args.get("y").and_then(|v| v.as_f64()) {
                    n.y = v;
                }
                if let Some(v) = args.get("cpus").and_then(|v| v.as_u64()) {
                    n.cpus = v as u32;
                }
                if let Some(v) = args.get("ram_mb").and_then(|v| v.as_u64()) {
                    n.ram_mb = v as u32;
                }
                if let Some(v) = args.get("interfaces").and_then(|v| v.as_u64()) {
                    n.interfaces = v as u32;
                }
                Ok(())
            })
            .await
            .map_err(|e| e.message)?;
        Ok(json!({"updated": name}))
    }

    async fn delete_node(&self, name: String) -> Result<Value, String> {
        let lab = self
            .state
            .store
            .load(self.lab_id)
            .map_err(|e| e.to_string())?;
        let node = self.find_node(&lab, &name)?;
        self.state
            .stop_node(self.lab_id, node.id)
            .await
            .map_err(|e| e.message)?;
        self.state
            .mutate_lab(self.lab_id, |lab| Ok(lab.remove_node(node.id)?))
            .await
            .map_err(|e| e.message)?;
        Ok(json!({"deleted": name}))
    }

    async fn create_link(&self, args: Value) -> Result<Value, String> {
        let a_node = args
            .get("a_node")
            .and_then(|v| v.as_str())
            .ok_or("missing a_node")?;
        let a_iface = args.get("a_iface").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
        let lab = self
            .state
            .store
            .load(self.lab_id)
            .map_err(|e| e.to_string())?;
        let a = self.find_node(&lab, a_node)?;

        let (b, desc): (Endpoint, String) =
            if let Some(net_name) = args.get("network").and_then(|v| v.as_str()) {
                let net = lab
                    .networks
                    .values()
                    .find(|n| n.name.eq_ignore_ascii_case(net_name))
                    .ok_or_else(|| format!("no network named '{net_name}'"))?;
                (
                    Endpoint::Network { network: net.id },
                    format!("{a_node}[{a_iface}] <-> {net_name}"),
                )
            } else {
                let b_node = args
                    .get("b_node")
                    .and_then(|v| v.as_str())
                    .ok_or("missing b_node or network")?;
                let b_iface = args.get("b_iface").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
                let bn = self.find_node(&lab, b_node)?;
                (
                    Endpoint::Node {
                        node: bn.id,
                        iface: b_iface,
                    },
                    format!("{a_node}[{a_iface}] <-> {b_node}[{b_iface}]"),
                )
            };

        let link = self
            .state
            .mutate_lab(self.lab_id, |lab| {
                let link = Link::between(
                    Endpoint::Node {
                        node: a.id,
                        iface: a_iface,
                    },
                    b,
                );
                lab.add_link(link.clone())?;
                Ok(link)
            })
            .await
            .map_err(|e| e.message)?;
        self.state
            .hot_wire_link(self.lab_id, link.id)
            .await
            .map_err(|e| e.message)?;
        Ok(json!({"linked": desc}))
    }

    async fn create_network(&self, args: Value) -> Result<Value, String> {
        let name = args
            .get("name")
            .and_then(|v| v.as_str())
            .ok_or("missing name")?
            .to_string();
        let kind = match args
            .get("kind")
            .and_then(|v| v.as_str())
            .unwrap_or("bridge")
        {
            "nat" => NetworkKind::Nat,
            "management" => NetworkKind::Management,
            "cloud" => NetworkKind::Cloud,
            _ => NetworkKind::Bridge,
        };
        self.state
            .mutate_lab(self.lab_id, |lab| {
                let net = Network {
                    id: Uuid::new_v4(),
                    name: name.clone(),
                    kind,
                    host_interface: None,
                    subnet: None,
                    x: args.get("x").and_then(|v| v.as_f64()).unwrap_or(400.0),
                    y: args.get("y").and_then(|v| v.as_f64()).unwrap_or(300.0),
                };
                lab.networks.insert(net.id, net.clone());
                Ok(net)
            })
            .await
            .map_err(|e| e.message)?;
        Ok(json!({"created_network": name}))
    }

    async fn set_startup_config(&self, node: String, config: String) -> Result<Value, String> {
        let lab = self
            .state
            .store
            .load(self.lab_id)
            .map_err(|e| e.to_string())?;
        let n = self.find_node(&lab, &node)?;
        self.state
            .mutate_lab(self.lab_id, |lab| {
                lab.node_mut(n.id)?.startup_config = Some(config);
                Ok(())
            })
            .await
            .map_err(|e| e.message)?;
        Ok(json!({"config_set": node}))
    }

    async fn start(&self, node: Option<String>) -> Result<Value, String> {
        match node {
            Some(name) => {
                let lab = self
                    .state
                    .store
                    .load(self.lab_id)
                    .map_err(|e| e.to_string())?;
                let n = self.find_node(&lab, &name)?;
                self.state
                    .start_node(self.lab_id, n.id)
                    .await
                    .map_err(|e| e.message)?;
                Ok(json!({"started": name}))
            }
            None => {
                let state = self.state.clone();
                let lab_id = self.lab_id;
                tokio::spawn(async move {
                    let _ = state.start_lab(lab_id).await;
                });
                Ok(json!({"starting": "all nodes (boot takes a while)"}))
            }
        }
    }

    async fn stop(&self, node: Option<String>) -> Result<Value, String> {
        match node {
            Some(name) => {
                let lab = self
                    .state
                    .store
                    .load(self.lab_id)
                    .map_err(|e| e.to_string())?;
                let n = self.find_node(&lab, &name)?;
                self.state
                    .stop_node(self.lab_id, n.id)
                    .await
                    .map_err(|e| e.message)?;
                Ok(json!({"stopped": name}))
            }
            None => {
                self.state
                    .stop_lab(self.lab_id)
                    .await
                    .map_err(|e| e.message)?;
                Ok(json!({"stopped": "all"}))
            }
        }
    }

    async fn set_link_quality(&self, args: Value) -> Result<Value, String> {
        let a_name = args
            .get("a_node")
            .and_then(|v| v.as_str())
            .ok_or("missing a_node")?;
        let b_name = args
            .get("b_node")
            .and_then(|v| v.as_str())
            .ok_or("missing b_node")?;
        let lab = self
            .state
            .store
            .load(self.lab_id)
            .map_err(|e| e.to_string())?;
        let a = self.find_node(&lab, a_name)?;
        let b = self.find_node(&lab, b_name)?;
        let link = lab
            .links
            .values()
            .find(|l| l.touches_node(a.id) && l.touches_node(b.id))
            .cloned()
            .ok_or_else(|| format!("no link between {a_name} and {b_name}"))?;

        let g = |k: &str| args.get(k).and_then(|v| v.as_u64()).unwrap_or(0) as u32;
        let imp = netpilot_core::Impairment {
            delay_ms: g("delay_ms"),
            jitter_ms: g("jitter_ms"),
            loss_pct: args.get("loss_pct").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32,
            rate_kbit: g("rate_kbit"),
        };
        let suspended = args
            .get("suspended")
            .and_then(|v| v.as_bool())
            .unwrap_or(link.suspended);

        self.state
            .mutate_lab(self.lab_id, |lab| {
                let l = lab.links.get_mut(&link.id).ok_or_else(|| {
                    crate::error::ApiError::not_found("link vanished during update")
                })?;
                l.impairment = if imp.is_noop() { None } else { Some(imp) };
                l.suspended = suspended;
                Ok(())
            })
            .await
            .map_err(|e| e.message)?;

        let switch = self.state.switch_for(self.lab_id).await;
        switch.set_impairment(
            link.id,
            netpilot_net::WireImpairment {
                delay_ms: imp.delay_ms,
                jitter_ms: imp.jitter_ms,
                loss_pct: imp.loss_pct,
                rate_kbit: imp.rate_kbit,
            },
        );
        switch.set_link_suspended(link.id, suspended);
        Ok(json!({
            "link": format!("{a_name} <-> {b_name}"),
            "impairment": imp,
            "suspended": suspended
        }))
    }

    async fn run_command(
        &self,
        node: String,
        command: String,
        timeout_s: u32,
    ) -> Result<Value, String> {
        let lab = self
            .state
            .store
            .load(self.lab_id)
            .map_err(|e| e.to_string())?;
        let n = self.find_node(&lab, &node)?;
        if n.console != ConsoleKind::Serial {
            return Err(format!(
                "{node} has a VNC console; run_command needs a serial console"
            ));
        }
        let sock = self
            .state
            .console_socket(self.lab_id, n.id)
            .await
            .map_err(|_| format!("{node} is not running"))?;
        let output = run_console_command(&sock, &command, timeout_s)
            .await
            .map_err(|e| format!("console: {e}"))?;
        Ok(json!(output))
    }
}

/// Expect-style command execution on a serial console socket:
/// wake the line, send the command, then collect output until it goes
/// quiet (500ms) or the timeout elapses.
pub async fn run_console_command(
    socket_path: &std::path::Path,
    command: &str,
    timeout_s: u32,
) -> std::io::Result<String> {
    let mut stream = tokio::net::UnixStream::connect(socket_path).await?;

    // Wake and drain any pending output/banner.
    stream.write_all(b"\r").await?;
    let mut scratch = [0u8; 4096];
    let drain_deadline = tokio::time::Instant::now() + Duration::from_millis(700);
    loop {
        match tokio::time::timeout_at(drain_deadline, stream.read(&mut scratch)).await {
            Ok(Ok(0)) => break,
            Ok(Ok(_)) => continue,
            _ => break,
        }
    }

    stream.write_all(command.as_bytes()).await?;
    stream.write_all(b"\r").await?;

    let mut output = Vec::new();
    let deadline = tokio::time::Instant::now() + Duration::from_secs(timeout_s.max(2) as u64);
    loop {
        // Wide enough to survive gaps between slow output lines (ping
        // replies arrive a second apart).
        let quiet = tokio::time::sleep(Duration::from_millis(1500));
        tokio::select! {
            r = stream.read(&mut scratch) => {
                match r {
                    Ok(0) | Err(_) => break,
                    Ok(n) => {
                        output.extend_from_slice(&scratch[..n]);
                        if output.len() > 256 * 1024 { break; }
                    }
                }
            }
            _ = quiet => {
                if !output.is_empty() { break; }
            }
        }
        if tokio::time::Instant::now() >= deadline {
            break;
        }
    }

    Ok(String::from_utf8_lossy(&output).into_owned())
}

/// WebSocket driver for one agent chat session.
///
/// When persistence is enabled the transcript is saved per (lab, user) in an
/// `agent_session`: the client may send `{"resume": "<session-id>"}` to
/// replay history, and every user message + streamed event is appended so
/// the conversation survives reloads. Without a DB it is ephemeral as before.
pub async fn run_agent_socket(
    socket: WebSocket,
    state: AppState,
    lab_id: Uuid,
    principal: netpilot_db::Principal,
) {
    let (mut ws_tx, mut ws_rx) = socket.split();

    let claude = match Claude::from_env() {
        Ok(c) => c,
        Err(e) => {
            let _ = ws_tx
                .send(Message::Text(
                    serde_json::to_string(&AgentEvent::Error {
                        message: e.to_string(),
                    })
                    .unwrap_or_default()
                    .into(),
                ))
                .await;
            return;
        }
    };
    let mut session = AgentSession::new(claude);
    let toolbox = StateToolbox {
        state: state.clone(),
        lab_id,
    };

    // Lazily-created persistence handle for this conversation.
    let mut session_id: Option<Uuid> = None;
    let persist = state.db.clone();

    while let Some(Ok(msg)) = ws_rx.next().await {
        let text = match msg {
            Message::Text(t) => t.to_string(),
            Message::Close(_) => break,
            _ => continue,
        };
        let Ok(v) = serde_json::from_str::<Value>(&text) else {
            continue;
        };

        // Resume: replay a stored transcript, then continue appending to it.
        if let Some(sid) = v.get("resume").and_then(|s| s.as_str()).and_then(|s| Uuid::parse_str(s).ok()) {
            if let Some(db) = persist.as_ref() {
                if db.session_owner(sid).await.map(|o| o == principal.user_id).unwrap_or(false) {
                    if let Ok(events) = db.agent_events(sid).await {
                        for ev in events {
                            let framed = serde_json::json!({ "type": "history", "item": ev });
                            let _ = ws_tx.send(Message::Text(framed.to_string().into())).await;
                        }
                    }
                    session_id = Some(sid);
                    let _ = ws_tx
                        .send(Message::Text(serde_json::json!({"type": "resumed", "session": sid}).to_string().into()))
                        .await;
                }
            }
            continue;
        }

        let Some(user_message) = v.get("message").and_then(|m| m.as_str()) else {
            continue;
        };
        let user_message = user_message.to_string();

        // Open a persisted session on the first real message.
        if session_id.is_none() {
            if let Some(db) = persist.as_ref() {
                let title: String = user_message.chars().take(60).collect();
                if let Ok(id) = db.create_agent_session(lab_id, principal.user_id, &title).await {
                    session_id = Some(id);
                    let _ = ws_tx
                        .send(Message::Text(serde_json::json!({"type": "session", "session": id}).to_string().into()))
                        .await;
                }
            }
        }
        // Record the user turn.
        if let (Some(db), Some(sid)) = (persist.as_ref(), session_id) {
            let _ = db
                .append_agent_event(sid, &serde_json::json!({"type": "user", "text": user_message}))
                .await;
        }

        let (tx, mut rx) = mpsc::channel::<AgentEvent>(64);

        // Run the turn and pump its events to the websocket concurrently.
        // `tx` moves into the turn and is dropped when it finishes, which
        // closes the channel and ends the pump.
        let turn = session.run_turn(&user_message, &toolbox, tx);
        let persist_ref = persist.clone();
        let pump = async {
            while let Some(ev) = rx.recv().await {
                let json = serde_json::to_string(&ev).unwrap_or_default();
                // Persist the streamed event alongside forwarding it.
                if let (Some(db), Some(sid)) = (persist_ref.as_ref(), session_id) {
                    if let Ok(val) = serde_json::to_value(&ev) {
                        let _ = db.append_agent_event(sid, &val).await;
                    }
                }
                if ws_tx.send(Message::Text(json.into())).await.is_err() {
                    break;
                }
            }
        };
        // Turn errors were already emitted as AgentEvent::Error.
        let (_turn_result, ()) = tokio::join!(turn, pump);
    }
}
