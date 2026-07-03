//! Node endpoints: CRUD, lifecycle actions, startup config, interfaces.

use std::collections::BTreeMap;

use axum::extract::{Path, State};
use axum::Json;
use netpilot_core::{iface_name_from_pattern, ConsoleKind, Node, NodeState};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::{ApiError, ApiResult};
use crate::state::AppState;

#[derive(Serialize)]
pub struct NodeView {
    #[serde(flatten)]
    pub node: Node,
    pub state: NodeState,
}

pub async fn list(
    State(state): State<AppState>,
    Path(lab_id): Path<Uuid>,
) -> ApiResult<Json<Vec<NodeView>>> {
    let lab = state.store.load(lab_id)?;
    let states = state.lab_states(lab_id).await;
    Ok(Json(
        lab.nodes
            .values()
            .map(|n| NodeView {
                node: n.clone(),
                state: *states.get(&n.id).unwrap_or(&NodeState::Stopped),
            })
            .collect(),
    ))
}

#[derive(Deserialize)]
pub struct CreateNode {
    pub template: String,
    /// Optional explicit name; otherwise derived from the template.
    pub name: Option<String>,
    pub image: Option<String>,
    pub cpus: Option<u32>,
    pub ram_mb: Option<u32>,
    pub interfaces: Option<u32>,
    pub x: Option<f64>,
    pub y: Option<f64>,
    pub startup_config: Option<String>,
    /// Advanced knobs (e.g. "boot_script" for netns nodes).
    pub overrides: Option<BTreeMap<String, String>>,
}

pub async fn create(
    State(state): State<AppState>,
    Path(lab_id): Path<Uuid>,
    Json(req): Json<CreateNode>,
) -> ApiResult<Json<Node>> {
    let templates = state.templates.read().await;
    let template = templates.get(&req.template)?.clone();
    drop(templates);

    // Default image: latest version available for this template.
    let image = match req.image {
        Some(i) => i,
        None => state
            .images
            .scan()
            .map(|imgs| {
                imgs.into_iter()
                    .filter(|i| i.template == template.id)
                    .map(|i| i.version)
                    .next_back()
                    .unwrap_or_default()
            })
            .unwrap_or_default(),
    };

    let interfaces = req
        .interfaces
        .unwrap_or(template.interfaces)
        .min(template.max_interfaces);

    let node = state
        .mutate_lab(lab_id, |lab| {
            let name = match req.name {
                Some(n) if !n.trim().is_empty() => {
                    if lab.nodes.values().any(|x| x.name == n.trim()) {
                        return Err(ApiError::conflict(format!(
                            "node name '{}' in use",
                            n.trim()
                        )));
                    }
                    n.trim().to_string()
                }
                _ => {
                    let prefix: String = template
                        .name
                        .chars()
                        .take_while(|c| c.is_ascii_alphanumeric())
                        .collect();
                    let prefix = if prefix.is_empty() {
                        "N".into()
                    } else {
                        prefix
                    };
                    lab.next_node_name(&prefix)
                }
            };
            let node = Node {
                id: Uuid::new_v4(),
                name,
                template: template.id.clone(),
                image,
                cpus: req.cpus.unwrap_or(template.cpus),
                ram_mb: req.ram_mb.unwrap_or(template.ram_mb),
                interfaces,
                console: template.console,
                icon: template.icon.clone(),
                x: req.x.unwrap_or(200.0),
                y: req.y.unwrap_or(200.0),
                startup_config: req.startup_config,
                config_sets: BTreeMap::new(),
                boot_delay_s: 0,
                overrides: req.overrides.unwrap_or_default(),
            };
            lab.nodes.insert(node.id, node.clone());
            Ok(node)
        })
        .await?;
    Ok(Json(node))
}

pub async fn get_node(
    State(state): State<AppState>,
    Path((lab_id, node_id)): Path<(Uuid, Uuid)>,
) -> ApiResult<Json<NodeView>> {
    let lab = state.store.load(lab_id)?;
    let node = lab.node(node_id)?.clone();
    let node_state = state.node_state(lab_id, node_id).await;
    Ok(Json(NodeView {
        node,
        state: node_state,
    }))
}

#[derive(Deserialize)]
pub struct UpdateNode {
    pub name: Option<String>,
    pub image: Option<String>,
    pub cpus: Option<u32>,
    pub ram_mb: Option<u32>,
    pub interfaces: Option<u32>,
    pub x: Option<f64>,
    pub y: Option<f64>,
    pub icon: Option<String>,
    pub console: Option<ConsoleKind>,
    pub boot_delay_s: Option<u32>,
}

pub async fn update(
    State(state): State<AppState>,
    Path((lab_id, node_id)): Path<(Uuid, Uuid)>,
    Json(req): Json<UpdateNode>,
) -> ApiResult<Json<Node>> {
    let running = state.supervisor.is_running(lab_id, node_id).await;
    // Position/name/icon are safe to change live; hardware is not.
    let hw_change = req.cpus.is_some()
        || req.ram_mb.is_some()
        || req.interfaces.is_some()
        || req.image.is_some();
    if running && hw_change {
        return Err(ApiError::conflict(
            "stop the node before changing cpu/ram/interfaces/image",
        ));
    }
    state
        .mutate_lab(lab_id, |lab| {
            // interface shrink must not orphan links
            if let Some(ifaces) = req.interfaces {
                let in_use = lab
                    .links
                    .values()
                    .flat_map(|l| l.endpoints().into_iter().cloned().collect::<Vec<_>>())
                    .filter_map(|e| match e {
                        netpilot_core::Endpoint::Node { node, iface } if node == node_id => {
                            Some(iface)
                        }
                        _ => None,
                    })
                    .max();
                if let Some(max_used) = in_use {
                    if ifaces <= max_used {
                        return Err(ApiError::conflict(format!(
                            "interface {max_used} is cabled; disconnect before shrinking"
                        )));
                    }
                }
            }
            let node = lab.node_mut(node_id)?;
            if let Some(v) = req.name {
                if !v.trim().is_empty() {
                    node.name = v.trim().into();
                }
            }
            if let Some(v) = req.image {
                node.image = v;
            }
            if let Some(v) = req.cpus {
                node.cpus = v.max(1);
            }
            if let Some(v) = req.ram_mb {
                node.ram_mb = v.max(32);
            }
            if let Some(v) = req.interfaces {
                node.interfaces = v;
            }
            if let Some(v) = req.x {
                node.x = v;
            }
            if let Some(v) = req.y {
                node.y = v;
            }
            if let Some(v) = req.icon {
                node.icon = v;
            }
            if let Some(v) = req.console {
                node.console = v;
            }
            if let Some(v) = req.boot_delay_s {
                node.boot_delay_s = v;
            }
            Ok(node.clone())
        })
        .await
        .map(Json)
}

pub async fn remove(
    State(state): State<AppState>,
    Path((lab_id, node_id)): Path<(Uuid, Uuid)>,
) -> ApiResult<Json<serde_json::Value>> {
    state.stop_node(lab_id, node_id).await?;
    state
        .mutate_lab(lab_id, |lab| Ok(lab.remove_node(node_id)?))
        .await?;
    // Remove runtime artifacts.
    let _ = std::fs::remove_dir_all(state.store.node_dir(lab_id, node_id));
    Ok(Json(serde_json::json!({ "deleted": node_id })))
}

pub async fn start(
    State(state): State<AppState>,
    Path((lab_id, node_id)): Path<(Uuid, Uuid)>,
) -> ApiResult<Json<serde_json::Value>> {
    state.start_node(lab_id, node_id).await?;
    Ok(Json(serde_json::json!({ "started": node_id })))
}

pub async fn stop(
    State(state): State<AppState>,
    Path((lab_id, node_id)): Path<(Uuid, Uuid)>,
) -> ApiResult<Json<serde_json::Value>> {
    state.stop_node(lab_id, node_id).await?;
    Ok(Json(serde_json::json!({ "stopped": node_id })))
}

pub async fn wipe(
    State(state): State<AppState>,
    Path((lab_id, node_id)): Path<(Uuid, Uuid)>,
) -> ApiResult<Json<serde_json::Value>> {
    state.wipe_node(lab_id, node_id).await?;
    Ok(Json(serde_json::json!({ "wiped": node_id })))
}

#[derive(Deserialize, Default)]
pub struct ConfigQuery {
    /// Named config set; omitted = the node's default startup config.
    #[serde(default)]
    pub set: Option<String>,
}

pub async fn get_config(
    State(state): State<AppState>,
    Path((lab_id, node_id)): Path<(Uuid, Uuid)>,
    axum::extract::Query(q): axum::extract::Query<ConfigQuery>,
) -> ApiResult<Json<serde_json::Value>> {
    let lab = state.store.load(lab_id)?;
    let node = lab.node(node_id)?;
    let config = match q.set.as_deref().filter(|s| !s.is_empty()) {
        Some(set) => node.config_sets.get(set).cloned().unwrap_or_default(),
        None => node.startup_config.clone().unwrap_or_default(),
    };
    Ok(Json(serde_json::json!({ "config": config })))
}

#[derive(Deserialize)]
pub struct SetConfig {
    pub config: String,
}

pub async fn set_config(
    State(state): State<AppState>,
    Path((lab_id, node_id)): Path<(Uuid, Uuid)>,
    axum::extract::Query(q): axum::extract::Query<ConfigQuery>,
    Json(req): Json<SetConfig>,
) -> ApiResult<Json<serde_json::Value>> {
    state
        .mutate_lab(lab_id, |lab| {
            let node = lab.node_mut(node_id)?;
            match q.set.as_deref().filter(|s| !s.is_empty()) {
                Some(set) => {
                    if req.config.is_empty() {
                        node.config_sets.remove(set);
                    } else {
                        node.config_sets.insert(set.to_string(), req.config);
                    }
                }
                None => {
                    node.startup_config = if req.config.is_empty() {
                        None
                    } else {
                        Some(req.config)
                    };
                }
            }
            Ok(())
        })
        .await?;
    Ok(Json(serde_json::json!({ "saved": node_id })))
}

#[derive(Deserialize)]
pub struct ExecRequest {
    pub command: String,
    #[serde(default = "default_exec_timeout")]
    pub timeout_s: u32,
}

fn default_exec_timeout() -> u32 {
    15
}

/// Run a CLI command on a running node's serial console and return the
/// captured output (the REST twin of the agent's run_command tool).
pub async fn exec(
    State(state): State<AppState>,
    Path((lab_id, node_id)): Path<(Uuid, Uuid)>,
    Json(req): Json<ExecRequest>,
) -> ApiResult<Json<serde_json::Value>> {
    if req.command.trim().is_empty() {
        return Err(ApiError::bad_request("command must not be empty"));
    }
    let sock = state.console_socket(lab_id, node_id).await?;
    let output =
        crate::agent::run_console_command(&sock, &req.command, req.timeout_s.min(120)).await?;
    Ok(Json(serde_json::json!({ "output": output })))
}

/// Export the running configuration off a live node's serial console into
/// its startup config (EVE-NG "export config"). Uses the template's
/// `export_command`.
pub async fn export_config(
    State(state): State<AppState>,
    Path((lab_id, node_id)): Path<(Uuid, Uuid)>,
) -> ApiResult<Json<serde_json::Value>> {
    let lab = state.store.load(lab_id)?;
    let node = lab.node(node_id)?.clone();
    let templates = state.templates.read().await;
    let command = templates
        .get(&node.template)
        .ok()
        .and_then(|t| t.export_command.clone())
        .ok_or_else(|| {
            ApiError::bad_request(format!(
                "template '{}' has no config export command",
                node.template
            ))
        })?;
    drop(templates);

    let sock = state.console_socket(lab_id, node_id).await?;
    let raw = crate::agent::run_console_command(&sock, &command, 30).await?;

    // Strip the echoed command line and trailing prompt.
    let mut lines: Vec<&str> = raw.lines().collect();
    if lines.first().is_some_and(|l| l.contains(&command)) {
        lines.remove(0);
    }
    while lines
        .last()
        .is_some_and(|l| l.trim().is_empty() || l.trim_start().starts_with(&node.name))
    {
        lines.pop();
    }
    let config = lines.join("\n");
    if config.trim().is_empty() {
        return Err(ApiError::conflict(
            "no output captured — is the console at a CLI prompt?",
        ));
    }

    state
        .mutate_lab(lab_id, |lab| {
            lab.node_mut(node_id)?.startup_config = Some(config.clone());
            Ok(())
        })
        .await?;
    Ok(Json(
        serde_json::json!({ "exported": node.name, "config": config }),
    ))
}

#[derive(Serialize)]
pub struct InterfaceView {
    pub index: u32,
    pub name: String,
    pub connected: bool,
    pub link: Option<Uuid>,
}

pub async fn interfaces(
    State(state): State<AppState>,
    Path((lab_id, node_id)): Path<(Uuid, Uuid)>,
) -> ApiResult<Json<Vec<InterfaceView>>> {
    let lab = state.store.load(lab_id)?;
    let node = lab.node(node_id)?;
    let templates = state.templates.read().await;
    let pattern = templates
        .get(&node.template)
        .map(|t| t.iface_pattern.clone())
        .unwrap_or_else(|_| "eth{i}".into());
    let views = (0..node.interfaces)
        .map(|i| {
            let link = lab.link_on_interface(node_id, i);
            InterfaceView {
                index: i,
                name: iface_name_from_pattern(&pattern, i),
                connected: link.is_some(),
                link: link.map(|l| l.id),
            }
        })
        .collect();
    Ok(Json(views))
}
