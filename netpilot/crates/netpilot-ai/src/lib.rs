//! netpilot-ai — the AI agent mode.
//!
//! An agent session drives the Claude Messages API in a tool-use loop.
//! Tools operate on a [`LabToolbox`] implemented by the server (read lab
//! state, edit topology, start/stop nodes, run console commands), so the
//! agent never touches disk or QEMU directly, and every tool call is
//! surfaced to the UI as an auditable transcript event.

pub mod agent;
pub mod client;
pub mod tools;

pub use agent::*;
pub use client::*;
pub use tools::*;

#[derive(Debug, thiserror::Error)]
pub enum AiError {
    #[error("ANTHROPIC_API_KEY is not set — export it to enable the AI agent")]
    NoApiKey,
    #[error("api error: {0}")]
    Api(String),
    #[error("http error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("{0}")]
    Other(String),
}

pub type Result<T> = std::result::Result<T, AiError>;
