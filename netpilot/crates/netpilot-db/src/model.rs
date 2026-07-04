//! Domain types stored in Postgres, plus the RBAC role/access enums.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A user's global capability tier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    /// Full control: user management, any lab, firmware, settings.
    Admin,
    /// Build/run/share labs they own or are granted edit on.
    Operator,
    /// Read-only across everything they can see.
    Viewer,
}

impl Role {
    pub fn as_str(self) -> &'static str {
        match self {
            Role::Admin => "admin",
            Role::Operator => "operator",
            Role::Viewer => "viewer",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "admin" => Some(Role::Admin),
            "operator" => Some(Role::Operator),
            "viewer" => Some(Role::Viewer),
            _ => None,
        }
    }

    /// Can this role mutate anything at all (create labs, edit, upload)?
    pub fn can_write(self) -> bool {
        matches!(self, Role::Admin | Role::Operator)
    }

    pub fn is_admin(self) -> bool {
        matches!(self, Role::Admin)
    }
}

/// Access level a specific user has on a specific lab.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Access {
    /// No access.
    None,
    /// Read-only.
    View,
    /// Mutate + start/stop.
    Edit,
    /// Owner: everything including delete + re-share.
    Own,
}

impl Access {
    pub fn as_str(self) -> &'static str {
        match self {
            Access::None => "none",
            Access::View => "view",
            Access::Edit => "edit",
            Access::Own => "own",
        }
    }

    pub fn parse(s: &str) -> Self {
        match s {
            "own" => Access::Own,
            "edit" => Access::Edit,
            "view" => Access::View,
            _ => Access::None,
        }
    }

    pub fn can_view(self) -> bool {
        self >= Access::View
    }

    pub fn can_edit(self) -> bool {
        self >= Access::Edit
    }
}

/// A user record (never serialized with the password hash).
#[derive(Debug, Clone, Serialize)]
pub struct User {
    pub id: Uuid,
    pub username: String,
    pub role: Role,
    pub disabled: bool,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

/// The authenticated principal attached to a request.
#[derive(Debug, Clone)]
pub struct Principal {
    pub user_id: Uuid,
    pub username: String,
    pub role: Role,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Visibility {
    Private,
    Public,
}

impl Visibility {
    pub fn as_str(self) -> &'static str {
        match self {
            Visibility::Private => "private",
            Visibility::Public => "public",
        }
    }
    pub fn parse(s: &str) -> Self {
        match s {
            "public" => Visibility::Public,
            _ => Visibility::Private,
        }
    }
}

/// Lab ownership + visibility metadata (the document itself is in the file store).
#[derive(Debug, Clone, Serialize)]
pub struct LabMeta {
    pub id: Uuid,
    pub owner_id: Uuid,
    pub owner_name: String,
    pub name: String,
    pub visibility: Visibility,
}

/// One share grant.
#[derive(Debug, Clone, Serialize)]
pub struct ShareGrant {
    pub user_id: Uuid,
    pub username: String,
    pub access: Access,
}

/// Firmware image metadata.
#[derive(Debug, Clone, Serialize)]
pub struct FirmwareImage {
    pub id: Uuid,
    pub template: String,
    pub version: String,
    pub filename: String,
    pub size_bytes: i64,
    pub sha256: Option<String>,
    pub uploaded_by: Option<Uuid>,
    pub uploaded_at: chrono::DateTime<chrono::Utc>,
}

/// A resumable agent session header.
#[derive(Debug, Clone, Serialize)]
pub struct AgentSessionMeta {
    pub id: Uuid,
    pub lab_id: Uuid,
    pub user_id: Uuid,
    pub title: String,
    pub updated_at: chrono::DateTime<chrono::Utc>,
    pub event_count: i64,
}
