//! Minimal Claude Messages API client (non-streaming; the agent loop emits
//! its own progress events, which is what the UI streams).

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{AiError, Result};

pub const DEFAULT_MODEL: &str = "claude-sonnet-5";
const API_VERSION: &str = "2023-06-01";

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

pub struct Claude {
    http: reqwest::Client,
    api_key: String,
    base_url: String,
    pub model: String,
}

impl Claude {
    /// Build from the environment (`ANTHROPIC_API_KEY`, optional
    /// `NETPILOT_AI_MODEL` and `ANTHROPIC_BASE_URL`).
    pub fn from_env() -> Result<Self> {
        let api_key = std::env::var("ANTHROPIC_API_KEY").map_err(|_| AiError::NoApiKey)?;
        let model =
            std::env::var("NETPILOT_AI_MODEL").unwrap_or_else(|_| DEFAULT_MODEL.to_string());
        let base_url = std::env::var("ANTHROPIC_BASE_URL")
            .unwrap_or_else(|_| "https://api.anthropic.com".to_string());
        Ok(Self {
            http: reqwest::Client::new(),
            api_key,
            base_url,
            model,
        })
    }

    pub async fn complete(
        &self,
        system: &str,
        messages: &[ChatMessage],
        tools: &[ToolDefinition],
    ) -> Result<ApiResponse> {
        let body = serde_json::json!({
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
            .header("anthropic-version", API_VERSION)
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
}
