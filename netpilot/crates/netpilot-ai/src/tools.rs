//! Tool surface the agent operates through.
//!
//! The server implements [`LabToolbox`]; the agent loop translates Claude
//! tool_use blocks into these calls. Keeping this a trait means the agent
//! is testable with a fake lab and the server stays the single authority
//! over what the agent may do.

use async_trait::async_trait;
use serde_json::{json, Value};

use crate::client::ToolDefinition;

/// Everything the agent can do to a lab, implemented by the server.
#[async_trait]
pub trait LabToolbox: Send + Sync {
    /// Current lab document + runtime states, JSON-serialized.
    async fn get_lab(&self) -> Result<Value, String>;
    /// Device template catalog with available images.
    async fn list_templates(&self) -> Result<Value, String>;
    /// Create a node; returns the created node JSON.
    async fn create_node(&self, args: Value) -> Result<Value, String>;
    /// Update node properties (position, cpu/ram, name...).
    async fn update_node(&self, args: Value) -> Result<Value, String>;
    /// Delete a node by name.
    async fn delete_node(&self, name: String) -> Result<Value, String>;
    /// Create a link: {a_node, a_iface, b_node, b_iface} or node<->network.
    async fn create_link(&self, args: Value) -> Result<Value, String>;
    /// Create a multipoint network {name, kind}.
    async fn create_network(&self, args: Value) -> Result<Value, String>;
    /// Set a node's startup configuration.
    async fn set_startup_config(&self, node: String, config: String) -> Result<Value, String>;
    /// Start a node (or all when node is None).
    async fn start(&self, node: Option<String>) -> Result<Value, String>;
    /// Stop a node (or all when node is None).
    async fn stop(&self, node: Option<String>) -> Result<Value, String>;
    /// Run a CLI command on a running node's console; returns output.
    async fn run_command(
        &self,
        node: String,
        command: String,
        timeout_s: u32,
    ) -> Result<Value, String>;
    /// Set link quality / suspension on the link between two nodes.
    async fn set_link_quality(&self, args: Value) -> Result<Value, String>;
}

/// Tool definitions advertised to the model.
pub fn tool_definitions() -> Vec<ToolDefinition> {
    let t = |name: &str, description: &str, schema: Value| ToolDefinition {
        name: name.into(),
        description: description.into(),
        input_schema: schema,
    };
    vec![
        t(
            "get_lab",
            "Read the current lab topology: nodes (with names, templates, interfaces, positions, startup configs), networks, links, and runtime state of every node.",
            json!({"type": "object", "properties": {}}),
        ),
        t(
            "list_templates",
            "List available device templates (vendors, default cpu/ram/interfaces, interface naming) and which disk images are available for each.",
            json!({"type": "object", "properties": {}}),
        ),
        t(
            "create_node",
            "Add a device to the lab. Choose a template from list_templates. Position on a 1600x900 canvas.",
            json!({
                "type": "object",
                "properties": {
                    "template": {"type": "string"},
                    "name": {"type": "string"},
                    "x": {"type": "number"},
                    "y": {"type": "number"},
                    "cpus": {"type": "integer"},
                    "ram_mb": {"type": "integer"},
                    "interfaces": {"type": "integer"},
                    "startup_config": {"type": "string", "description": "full startup configuration in the device's native syntax"}
                },
                "required": ["template", "name"]
            }),
        ),
        t(
            "update_node",
            "Update an existing node by name (position, cpus, ram_mb, interfaces).",
            json!({
                "type": "object",
                "properties": {
                    "name": {"type": "string"},
                    "x": {"type": "number"},
                    "y": {"type": "number"},
                    "cpus": {"type": "integer"},
                    "ram_mb": {"type": "integer"},
                    "interfaces": {"type": "integer"}
                },
                "required": ["name"]
            }),
        ),
        t(
            "delete_node",
            "Remove a node (and its links) by name.",
            json!({
                "type": "object",
                "properties": {"name": {"type": "string"}},
                "required": ["name"]
            }),
        ),
        t(
            "create_link",
            "Cable two nodes together (point-to-point), or a node to a named network. Interface indexes are 0-based; check get_lab for free interfaces.",
            json!({
                "type": "object",
                "properties": {
                    "a_node": {"type": "string"},
                    "a_iface": {"type": "integer"},
                    "b_node": {"type": "string", "description": "peer node name (omit if linking to a network)"},
                    "b_iface": {"type": "integer"},
                    "network": {"type": "string", "description": "network name (omit if node-to-node)"}
                },
                "required": ["a_node", "a_iface"]
            }),
        ),
        t(
            "create_network",
            "Create a multipoint network segment (kind: bridge | nat | management | cloud).",
            json!({
                "type": "object",
                "properties": {
                    "name": {"type": "string"},
                    "kind": {"type": "string", "enum": ["bridge", "nat", "management", "cloud"]},
                    "x": {"type": "number"},
                    "y": {"type": "number"}
                },
                "required": ["name"]
            }),
        ),
        t(
            "set_startup_config",
            "Set (replace) a node's startup configuration, applied on next boot from a wiped state.",
            json!({
                "type": "object",
                "properties": {
                    "node": {"type": "string"},
                    "config": {"type": "string"}
                },
                "required": ["node", "config"]
            }),
        ),
        t(
            "start",
            "Start one node by name, or the whole lab when name is omitted. Booting NOS images takes minutes; tell the user.",
            json!({
                "type": "object",
                "properties": {"node": {"type": "string"}}
            }),
        ),
        t(
            "stop",
            "Stop one node by name, or the whole lab when name is omitted.",
            json!({
                "type": "object",
                "properties": {"node": {"type": "string"}}
            }),
        ),
        t(
            "set_link_quality",
            "Impair or suspend the link between two nodes, live (delay/jitter in ms, loss in %, rate in kbit/s; zeros clear). Useful for failure testing.",
            json!({
                "type": "object",
                "properties": {
                    "a_node": {"type": "string"},
                    "b_node": {"type": "string"},
                    "delay_ms": {"type": "integer"},
                    "jitter_ms": {"type": "integer"},
                    "loss_pct": {"type": "number"},
                    "rate_kbit": {"type": "integer"},
                    "suspended": {"type": "boolean"}
                },
                "required": ["a_node", "b_node"]
            }),
        ),
        t(
            "run_command",
            "Run a CLI command on a RUNNING node's serial console and return the output. Use for verification (show commands) and live configuration. One command per call.",
            json!({
                "type": "object",
                "properties": {
                    "node": {"type": "string"},
                    "command": {"type": "string"},
                    "timeout_s": {"type": "integer", "description": "seconds to wait for output, default 10"}
                },
                "required": ["node", "command"]
            }),
        ),
    ]
}

/// Dispatch one tool_use block to the toolbox.
pub async fn dispatch(
    toolbox: &dyn LabToolbox,
    name: &str,
    input: &Value,
) -> Result<Value, String> {
    let s = |v: &Value, k: &str| -> Option<String> {
        v.get(k).and_then(|x| x.as_str()).map(|x| x.to_string())
    };
    match name {
        "get_lab" => toolbox.get_lab().await,
        "list_templates" => toolbox.list_templates().await,
        "create_node" => toolbox.create_node(input.clone()).await,
        "update_node" => toolbox.update_node(input.clone()).await,
        "delete_node" => {
            let name = s(input, "name").ok_or("missing name")?;
            toolbox.delete_node(name).await
        }
        "create_link" => toolbox.create_link(input.clone()).await,
        "create_network" => toolbox.create_network(input.clone()).await,
        "set_startup_config" => {
            let node = s(input, "node").ok_or("missing node")?;
            let config = s(input, "config").ok_or("missing config")?;
            toolbox.set_startup_config(node, config).await
        }
        "start" => toolbox.start(s(input, "node")).await,
        "stop" => toolbox.stop(s(input, "node")).await,
        "run_command" => {
            let node = s(input, "node").ok_or("missing node")?;
            let command = s(input, "command").ok_or("missing command")?;
            let timeout = input
                .get("timeout_s")
                .and_then(|v| v.as_u64())
                .unwrap_or(10) as u32;
            toolbox.run_command(node, command, timeout).await
        }
        "set_link_quality" => toolbox.set_link_quality(input.clone()).await,
        other => Err(format!("unknown tool: {other}")),
    }
}
