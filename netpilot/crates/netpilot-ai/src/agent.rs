//! The agent loop: user prompt → model → tool calls → results → model …
//!
//! Every step is emitted as an [`AgentEvent`] so the UI renders an
//! auditable transcript of what the agent did to the lab.

use serde::Serialize;
use serde_json::Value;
use tokio::sync::mpsc;

use crate::client::{ChatMessage, Claude, ContentBlock};
use crate::tools::{dispatch, tool_definitions, LabToolbox};
use crate::Result;

/// Events streamed to the UI during an agent turn.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AgentEvent {
    /// Assistant prose (a full text block).
    Text { text: String },
    /// The agent is invoking a tool.
    ToolCall { id: String, name: String, input: Value },
    /// A tool finished.
    ToolResult { id: String, output: String, is_error: bool },
    /// Turn finished.
    Done,
    /// Fatal error for this turn.
    Error { message: String },
}

const SYSTEM_PROMPT: &str = r#"You are NetPilot's lab engineer agent: an expert network engineer
operating a QEMU-based network emulation lab on the user's behalf.

You can inspect and edit the topology, write vendor-native startup
configurations, start/stop nodes, and run CLI commands on running nodes'
serial consoles.

Working rules:
- Always call get_lab before modifying anything, and list_templates before
  creating nodes, so names/interfaces/templates are grounded in reality.
- Lay out topologies readably on the canvas (roughly 160-220px spacing,
  cores above access, management to the side).
- When asked to build a topology, create the nodes, cable them, and write
  complete startup configs (hostnames, interface addressing, routing) in
  each platform's native syntax unless the user asks otherwise. Use
  sensible RFC5737/private addressing when the user doesn't specify.
- Interface indexes are 0-based; index 0 is the management interface on
  templates that declare one (check the template's notes).
- Verification: after configuring running nodes, use run_command with show
  commands to confirm (interfaces up, adjacencies, routes, pings).
- Booting NOS images can take minutes. Don't spin waiting on boots — tell
  the user what to expect instead.
- Be concise. Report what you did, what you verified, and anything that
  needs the user's attention. Never invent command output.
"#;

/// Maximum model round-trips per user message (tool loop guard).
const MAX_STEPS: usize = 24;

pub struct AgentSession {
    claude: Claude,
    history: Vec<ChatMessage>,
}

impl AgentSession {
    pub fn new(claude: Claude) -> Self {
        Self {
            claude,
            history: Vec::new(),
        }
    }

    /// Run one user turn. Emits events on `tx` (dropped when the turn
    /// ends, closing the channel); returns when the turn ends.
    pub async fn run_turn(
        &mut self,
        user_message: &str,
        toolbox: &dyn LabToolbox,
        tx: mpsc::Sender<AgentEvent>,
    ) -> Result<()> {
        self.history.push(ChatMessage::user_text(user_message));
        let tools = tool_definitions();

        for _ in 0..MAX_STEPS {
            let response = match self
                .claude
                .complete(SYSTEM_PROMPT, &self.history, &tools)
                .await
            {
                Ok(r) => r,
                Err(e) => {
                    let _ = tx.send(AgentEvent::Error { message: e.to_string() }).await;
                    // Drop the failed exchange so the session stays usable.
                    self.history.pop();
                    return Err(e);
                }
            };

            let mut tool_uses = Vec::new();
            for block in &response.content {
                match block {
                    ContentBlock::Text { text } => {
                        let _ = tx.send(AgentEvent::Text { text: text.clone() }).await;
                    }
                    ContentBlock::ToolUse { id, name, input } => {
                        let _ = tx
                            .send(AgentEvent::ToolCall {
                                id: id.clone(),
                                name: name.clone(),
                                input: input.clone(),
                            })
                            .await;
                        tool_uses.push((id.clone(), name.clone(), input.clone()));
                    }
                    ContentBlock::ToolResult { .. } => {}
                }
            }

            self.history.push(ChatMessage {
                role: "assistant".into(),
                content: response.content.clone(),
            });

            if tool_uses.is_empty() || response.stop_reason.as_deref() != Some("tool_use") {
                let _ = tx.send(AgentEvent::Done).await;
                return Ok(());
            }

            // Execute tools sequentially (topology edits are order-sensitive).
            let mut results = Vec::new();
            for (id, name, input) in tool_uses {
                let outcome = dispatch(toolbox, &name, &input).await;
                let (content, is_error) = match outcome {
                    Ok(v) => (compact(&v), false),
                    Err(e) => (e, true),
                };
                let _ = tx
                    .send(AgentEvent::ToolResult {
                        id: id.clone(),
                        output: truncate(&content, 2000),
                        is_error,
                    })
                    .await;
                results.push(ContentBlock::ToolResult {
                    tool_use_id: id,
                    content: truncate(&content, 16000),
                    is_error: if is_error { Some(true) } else { None },
                });
            }
            self.history.push(ChatMessage {
                role: "user".into(),
                content: results,
            });
        }

        let _ = tx
            .send(AgentEvent::Error {
                message: format!("agent stopped after {MAX_STEPS} steps (loop guard)"),
            })
            .await;
        let _ = tx.send(AgentEvent::Done).await;
        Ok(())
    }
}

fn compact(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        let cut: String = s.chars().take(max).collect();
        format!("{cut}…[truncated {} bytes]", s.len() - cut.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::LabToolbox;
    use async_trait::async_trait;
    use serde_json::json;
    use std::result::Result;

    struct FakeLab;

    #[async_trait]
    impl LabToolbox for FakeLab {
        async fn get_lab(&self) -> Result<Value, String> {
            Ok(json!({"nodes": []}))
        }
        async fn list_templates(&self) -> Result<Value, String> {
            Ok(json!([{"id": "vyos"}]))
        }
        async fn create_node(&self, args: Value) -> Result<Value, String> {
            Ok(args)
        }
        async fn update_node(&self, _: Value) -> Result<Value, String> {
            Ok(json!({}))
        }
        async fn delete_node(&self, _: String) -> Result<Value, String> {
            Ok(json!({}))
        }
        async fn create_link(&self, _: Value) -> Result<Value, String> {
            Err("iface busy".into())
        }
        async fn create_network(&self, _: Value) -> Result<Value, String> {
            Ok(json!({}))
        }
        async fn set_startup_config(&self, _: String, _: String) -> Result<Value, String> {
            Ok(json!({}))
        }
        async fn start(&self, _: Option<String>) -> Result<Value, String> {
            Ok(json!({}))
        }
        async fn stop(&self, _: Option<String>) -> Result<Value, String> {
            Ok(json!({}))
        }
        async fn run_command(&self, node: String, command: String, _: u32) -> Result<Value, String> {
            Ok(json!(format!("{node}# {command}\nok")))
        }
    }

    #[tokio::test]
    async fn dispatch_routes_tools() {
        let lab = FakeLab;
        let out = dispatch(&lab, "get_lab", &json!({})).await.unwrap();
        assert_eq!(out, json!({"nodes": []}));

        let out = dispatch(&lab, "run_command", &json!({"node": "R1", "command": "show ip route"}))
            .await
            .unwrap();
        assert!(out.as_str().unwrap().contains("R1# show ip route"));

        let err = dispatch(&lab, "create_link", &json!({"a_node": "R1", "a_iface": 0}))
            .await
            .unwrap_err();
        assert_eq!(err, "iface busy");

        let err = dispatch(&lab, "nope", &json!({})).await.unwrap_err();
        assert!(err.contains("unknown tool"));
    }

    #[test]
    fn truncation() {
        assert_eq!(truncate("short", 10), "short");
        assert!(truncate(&"x".repeat(100), 10).starts_with("xxxxxxxxxx…"));
    }
}
