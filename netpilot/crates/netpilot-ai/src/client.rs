//! LLM client with two wire protocols behind one interface:
//!
//! * **Anthropic Messages API** (Claude)
//! * **OpenAI-compatible chat completions** — OpenRouter, DeepSeek, Kimi,
//!   OpenAI, local vLLM/ollama gateways…
//!
//! Provider selection (first match wins):
//! 1. `NETPILOT_AI_PROVIDER` = `anthropic` | `openrouter` | `openai-compatible`
//! 2. `OPENROUTER_API_KEY` set → OpenRouter
//! 3. `ANTHROPIC_API_KEY` set → Anthropic
//!
//! Model via `NETPILOT_AI_MODEL` (defaults: Claude for Anthropic,
//! `deepseek/deepseek-chat` for OpenRouter). Base URL overrides:
//! `ANTHROPIC_BASE_URL` / `OPENAI_BASE_URL`.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::{AiError, Result};

pub const DEFAULT_ANTHROPIC_MODEL: &str = "claude-sonnet-5";
pub const DEFAULT_OPENROUTER_MODEL: &str = "deepseek/deepseek-chat";
const ANTHROPIC_VERSION: &str = "2023-06-01";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Provider {
    Anthropic,
    OpenAiCompatible,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentBlock {
    Text {
        text: String,
    },
    ToolUse {
        id: String,
        name: String,
        input: Value,
    },
    ToolResult {
        tool_use_id: String,
        content: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        is_error: Option<bool>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String, // "user" | "assistant"
    pub content: Vec<ContentBlock>,
}

impl ChatMessage {
    pub fn user_text(text: impl Into<String>) -> Self {
        Self {
            role: "user".into(),
            content: vec![ContentBlock::Text { text: text.into() }],
        }
    }
}

#[derive(Debug, Serialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub input_schema: Value,
}

#[derive(Debug, Deserialize)]
pub struct ApiResponse {
    pub content: Vec<ContentBlock>,
    pub stop_reason: Option<String>,
}

pub struct LlmClient {
    http: reqwest::Client,
    api_key: String,
    base_url: String,
    pub provider: Provider,
    pub model: String,
}

/// Backwards-compatible alias (the original client was Anthropic-only).
pub type Claude = LlmClient;

impl LlmClient {
    /// Build from the environment; see module docs for the variables.
    pub fn from_env() -> Result<Self> {
        let explicit = std::env::var("NETPILOT_AI_PROVIDER").unwrap_or_default();
        let openrouter_key = std::env::var("OPENROUTER_API_KEY").ok();
        let anthropic_key = std::env::var("ANTHROPIC_API_KEY").ok();

        let (provider, api_key, base_url, default_model) = match explicit.as_str() {
            "anthropic" => (
                Provider::Anthropic,
                anthropic_key.ok_or(AiError::NoApiKey)?,
                std::env::var("ANTHROPIC_BASE_URL")
                    .unwrap_or_else(|_| "https://api.anthropic.com".into()),
                DEFAULT_ANTHROPIC_MODEL,
            ),
            "openrouter" => (
                Provider::OpenAiCompatible,
                openrouter_key.ok_or(AiError::NoApiKey)?,
                std::env::var("OPENAI_BASE_URL")
                    .unwrap_or_else(|_| "https://openrouter.ai/api".into()),
                DEFAULT_OPENROUTER_MODEL,
            ),
            "openai-compatible" | "openai" => (
                Provider::OpenAiCompatible,
                std::env::var("OPENAI_API_KEY")
                    .ok()
                    .or(openrouter_key)
                    .ok_or(AiError::NoApiKey)?,
                std::env::var("OPENAI_BASE_URL")
                    .unwrap_or_else(|_| "https://api.openai.com".into()),
                "gpt-4o-mini",
            ),
            _ => {
                if let Some(key) = openrouter_key {
                    (
                        Provider::OpenAiCompatible,
                        key,
                        std::env::var("OPENAI_BASE_URL")
                            .unwrap_or_else(|_| "https://openrouter.ai/api".into()),
                        DEFAULT_OPENROUTER_MODEL,
                    )
                } else if let Some(key) = anthropic_key {
                    (
                        Provider::Anthropic,
                        key,
                        std::env::var("ANTHROPIC_BASE_URL")
                            .unwrap_or_else(|_| "https://api.anthropic.com".into()),
                        DEFAULT_ANTHROPIC_MODEL,
                    )
                } else {
                    return Err(AiError::NoApiKey);
                }
            }
        };

        let model =
            std::env::var("NETPILOT_AI_MODEL").unwrap_or_else(|_| default_model.to_string());
        Ok(Self {
            http: reqwest::Client::new(),
            api_key,
            base_url,
            provider,
            model,
        })
    }

    pub async fn complete(
        &self,
        system: &str,
        messages: &[ChatMessage],
        tools: &[ToolDefinition],
    ) -> Result<ApiResponse> {
        match self.provider {
            Provider::Anthropic => self.complete_anthropic(system, messages, tools).await,
            Provider::OpenAiCompatible => self.complete_openai(system, messages, tools).await,
        }
    }

    async fn complete_anthropic(
        &self,
        system: &str,
        messages: &[ChatMessage],
        tools: &[ToolDefinition],
    ) -> Result<ApiResponse> {
        let body = json!({
            "model": self.model,
            "max_tokens": 4096,
            "system": system,
            "messages": messages,
            "tools": tools,
        });
        let resp = self
            .http
            .post(format!("{}/v1/messages", self.base_url))
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", ANTHROPIC_VERSION)
            .json(&body)
            .send()
            .await?;

        let status = resp.status();
        let text = resp.text().await?;
        if !status.is_success() {
            return Err(AiError::Api(format!("{status}: {text}")));
        }
        serde_json::from_str(&text).map_err(|e| AiError::Api(format!("bad response: {e}")))
    }

    async fn complete_openai(
        &self,
        system: &str,
        messages: &[ChatMessage],
        tools: &[ToolDefinition],
    ) -> Result<ApiResponse> {
        // ---- translate our neutral message model to OpenAI chat format ----
        let mut oai_messages = vec![json!({"role": "system", "content": system})];
        for m in messages {
            match m.role.as_str() {
                "assistant" => {
                    let mut text = String::new();
                    let mut tool_calls = Vec::new();
                    for block in &m.content {
                        match block {
                            ContentBlock::Text { text: t } => {
                                if !text.is_empty() {
                                    text.push('\n');
                                }
                                text.push_str(t);
                            }
                            ContentBlock::ToolUse { id, name, input } => {
                                tool_calls.push(json!({
                                    "id": id,
                                    "type": "function",
                                    "function": {
                                        "name": name,
                                        "arguments": input.to_string(),
                                    }
                                }));
                            }
                            ContentBlock::ToolResult { .. } => {}
                        }
                    }
                    let mut msg = json!({"role": "assistant"});
                    msg["content"] = if text.is_empty() {
                        Value::Null
                    } else {
                        Value::String(text)
                    };
                    if !tool_calls.is_empty() {
                        msg["tool_calls"] = Value::Array(tool_calls);
                    }
                    oai_messages.push(msg);
                }
                _ => {
                    // user turn: text blocks become a user message; tool
                    // results become role:"tool" messages.
                    let mut text = String::new();
                    for block in &m.content {
                        match block {
                            ContentBlock::Text { text: t } => {
                                if !text.is_empty() {
                                    text.push('\n');
                                }
                                text.push_str(t);
                            }
                            ContentBlock::ToolResult {
                                tool_use_id,
                                content,
                                is_error,
                            } => {
                                let body = if is_error.unwrap_or(false) {
                                    format!("ERROR: {content}")
                                } else {
                                    content.clone()
                                };
                                oai_messages.push(json!({
                                    "role": "tool",
                                    "tool_call_id": tool_use_id,
                                    "content": body,
                                }));
                            }
                            ContentBlock::ToolUse { .. } => {}
                        }
                    }
                    if !text.is_empty() {
                        oai_messages.push(json!({"role": "user", "content": text}));
                    }
                }
            }
        }

        let oai_tools: Vec<Value> = tools
            .iter()
            .map(|t| {
                json!({
                    "type": "function",
                    "function": {
                        "name": t.name,
                        "description": t.description,
                        "parameters": t.input_schema,
                    }
                })
            })
            .collect();

        let body = json!({
            "model": self.model,
            "messages": oai_messages,
            "tools": oai_tools,
        });
        let resp = self
            .http
            .post(format!("{}/v1/chat/completions", self.base_url))
            .header("authorization", format!("Bearer {}", self.api_key))
            .header("x-title", "NetPilot")
            .json(&body)
            .send()
            .await?;

        let status = resp.status();
        let text = resp.text().await?;
        if !status.is_success() {
            return Err(AiError::Api(format!("{status}: {text}")));
        }
        let v: Value =
            serde_json::from_str(&text).map_err(|e| AiError::Api(format!("bad response: {e}")))?;

        // ---- translate the response back to our neutral model ----
        let choice = v
            .pointer("/choices/0")
            .ok_or_else(|| AiError::Api(format!("no choices in response: {text}")))?;
        let msg = &choice["message"];
        let mut content = Vec::new();
        if let Some(t) = msg["content"].as_str() {
            if !t.trim().is_empty() {
                content.push(ContentBlock::Text {
                    text: t.to_string(),
                });
            }
        }
        let mut has_tools = false;
        if let Some(calls) = msg["tool_calls"].as_array() {
            for (i, call) in calls.iter().enumerate() {
                has_tools = true;
                let id = call["id"]
                    .as_str()
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| format!("call_{i}"));
                let name = call
                    .pointer("/function/name")
                    .and_then(|n| n.as_str())
                    .unwrap_or("")
                    .to_string();
                let args_raw = call
                    .pointer("/function/arguments")
                    .and_then(|a| a.as_str())
                    .unwrap_or("{}");
                let input: Value =
                    serde_json::from_str(args_raw).unwrap_or_else(|_| json!({ "raw": args_raw }));
                content.push(ContentBlock::ToolUse { id, name, input });
            }
        }
        let finish = choice["finish_reason"].as_str().unwrap_or("stop");
        let stop_reason = if has_tools || finish == "tool_calls" {
            "tool_use"
        } else {
            "end_turn"
        };
        Ok(ApiResponse {
            content,
            stop_reason: Some(stop_reason.to_string()),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    /// One-shot HTTP server: capture the request body, reply with `reply`.
    async fn one_shot_http(reply: String) -> (String, tokio::sync::oneshot::Receiver<Value>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let (tx, rx) = tokio::sync::oneshot::channel();
        tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut buf = Vec::new();
            let mut tmp = [0u8; 4096];
            let body_start;
            let content_len;
            loop {
                let n = stream.read(&mut tmp).await.unwrap();
                buf.extend_from_slice(&tmp[..n]);
                if let Some(pos) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
                    let headers = String::from_utf8_lossy(&buf[..pos]).to_lowercase();
                    content_len = headers
                        .lines()
                        .find_map(|l| l.strip_prefix("content-length:"))
                        .and_then(|v| v.trim().parse::<usize>().ok())
                        .unwrap_or(0);
                    body_start = pos + 4;
                    break;
                }
            }
            while buf.len() < body_start + content_len {
                let n = stream.read(&mut tmp).await.unwrap();
                buf.extend_from_slice(&tmp[..n]);
            }
            let body: Value =
                serde_json::from_slice(&buf[body_start..body_start + content_len]).unwrap();
            let _ = tx.send(body);
            let resp = format!(
                "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
                reply.len(),
                reply
            );
            stream.write_all(resp.as_bytes()).await.unwrap();
        });
        (format!("http://{addr}"), rx)
    }

    fn client(base: String) -> LlmClient {
        LlmClient {
            http: reqwest::Client::new(),
            api_key: "test".into(),
            base_url: base,
            provider: Provider::OpenAiCompatible,
            model: "deepseek/deepseek-chat".into(),
        }
    }

    #[tokio::test]
    async fn openai_protocol_roundtrip() {
        // Model replies with a tool call.
        let reply = serde_json::json!({
            "choices": [{
                "finish_reason": "tool_calls",
                "message": {
                    "content": "Let me check the lab.",
                    "tool_calls": [{
                        "id": "call_abc",
                        "type": "function",
                        "function": {"name": "get_lab", "arguments": "{}"}
                    }]
                }
            }]
        });
        let (base, rx) = one_shot_http(reply.to_string()).await;

        // History includes a prior assistant tool call + its result, so the
        // request translation of every block kind is exercised.
        let history = vec![
            ChatMessage::user_text("build a lab"),
            ChatMessage {
                role: "assistant".into(),
                content: vec![
                    ContentBlock::Text { text: "ok".into() },
                    ContentBlock::ToolUse {
                        id: "call_1".into(),
                        name: "create_node".into(),
                        input: serde_json::json!({"template": "frr", "name": "r1"}),
                    },
                ],
            },
            ChatMessage {
                role: "user".into(),
                content: vec![ContentBlock::ToolResult {
                    tool_use_id: "call_1".into(),
                    content: "{\"created\":\"r1\"}".into(),
                    is_error: None,
                }],
            },
        ];
        let tools = vec![ToolDefinition {
            name: "get_lab".into(),
            description: "read the lab".into(),
            input_schema: serde_json::json!({"type": "object", "properties": {}}),
        }];

        let resp = client(base)
            .complete("you are a lab agent", &history, &tools)
            .await
            .unwrap();

        // ---- request translation ----
        let req = rx.await.unwrap();
        assert_eq!(req["model"], "deepseek/deepseek-chat");
        let msgs = req["messages"].as_array().unwrap();
        assert_eq!(msgs[0]["role"], "system");
        assert_eq!(msgs[1]["role"], "user");
        assert_eq!(msgs[2]["role"], "assistant");
        assert_eq!(msgs[2]["tool_calls"][0]["function"]["name"], "create_node");
        assert_eq!(msgs[3]["role"], "tool");
        assert_eq!(msgs[3]["tool_call_id"], "call_1");
        assert_eq!(req["tools"][0]["function"]["name"], "get_lab");

        // ---- response translation ----
        assert_eq!(resp.stop_reason.as_deref(), Some("tool_use"));
        assert!(matches!(&resp.content[0], ContentBlock::Text { text } if text.contains("check")));
        match &resp.content[1] {
            ContentBlock::ToolUse { id, name, .. } => {
                assert_eq!(id, "call_abc");
                assert_eq!(name, "get_lab");
            }
            other => panic!("expected tool use, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn openai_plain_text_reply() {
        let reply = serde_json::json!({
            "choices": [{
                "finish_reason": "stop",
                "message": {"content": "All adjacencies are up."}
            }]
        });
        let (base, _rx) = one_shot_http(reply.to_string()).await;
        let resp = client(base)
            .complete("sys", &[ChatMessage::user_text("status?")], &[])
            .await
            .unwrap();
        assert_eq!(resp.stop_reason.as_deref(), Some("end_turn"));
        assert!(
            matches!(&resp.content[0], ContentBlock::Text { text } if text.contains("adjacencies"))
        );
    }
}
