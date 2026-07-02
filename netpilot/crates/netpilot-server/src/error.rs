//! API error type with proper HTTP status mapping.

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use netpilot_core::CoreError;
use netpilot_qemu::QemuError;

pub type ApiResult<T> = Result<T, ApiError>;

#[derive(Debug)]
pub struct ApiError {
    pub status: StatusCode,
    pub message: String,
}

impl ApiError {
    pub fn bad_request(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            message: message.into(),
        }
    }

    pub fn not_found(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::NOT_FOUND,
            message: message.into(),
        }
    }

    pub fn conflict(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::CONFLICT,
            message: message.into(),
        }
    }

    pub fn internal(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: message.into(),
        }
    }
}

impl std::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.status, self.message)
    }
}

impl From<CoreError> for ApiError {
    fn from(e: CoreError) -> Self {
        let status = match &e {
            CoreError::LabNotFound(_)
            | CoreError::NodeNotFound(_)
            | CoreError::LinkNotFound(_)
            | CoreError::NetworkNotFound(_)
            | CoreError::TemplateNotFound(_)
            | CoreError::ImageNotFound(_) => StatusCode::NOT_FOUND,
            CoreError::InterfaceBusy { .. } | CoreError::NodeRunning(_) => StatusCode::CONFLICT,
            CoreError::InvalidInterface { .. } | CoreError::Validation(_) => {
                StatusCode::BAD_REQUEST
            }
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        };
        Self {
            status,
            message: e.to_string(),
        }
    }
}

impl From<QemuError> for ApiError {
    fn from(e: QemuError) -> Self {
        let status = match &e {
            QemuError::NotRunning => StatusCode::CONFLICT,
            QemuError::QemuMissing(_) => StatusCode::FAILED_DEPENDENCY,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        };
        Self {
            status,
            message: e.to_string(),
        }
    }
}

impl From<std::io::Error> for ApiError {
    fn from(e: std::io::Error) -> Self {
        Self::internal(e.to_string())
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (
            self.status,
            Json(serde_json::json!({ "error": self.message })),
        )
            .into_response()
    }
}
