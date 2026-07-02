use thiserror::Error;

pub type Result<T> = std::result::Result<T, CoreError>;

#[derive(Debug, Error)]
pub enum CoreError {
    #[error("lab not found: {0}")]
    LabNotFound(String),

    #[error("node not found: {0}")]
    NodeNotFound(String),

    #[error("link not found: {0}")]
    LinkNotFound(String),

    #[error("network not found: {0}")]
    NetworkNotFound(String),

    #[error("template not found: {0}")]
    TemplateNotFound(String),

    #[error("image not found: {0}")]
    ImageNotFound(String),

    #[error("interface {iface} on node {node} is already connected")]
    InterfaceBusy { node: String, iface: u32 },

    #[error("invalid interface index {iface} for node {node} ({max} interfaces)")]
    InvalidInterface { node: String, iface: u32, max: u32 },

    #[error("operation not allowed while node {0} is running")]
    NodeRunning(String),

    #[error("validation error: {0}")]
    Validation(String),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("serialization error: {0}")]
    Yaml(#[from] serde_yaml::Error),

    #[error("serialization error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("{0}")]
    Other(String),
}
