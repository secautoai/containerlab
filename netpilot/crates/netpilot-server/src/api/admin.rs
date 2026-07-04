//! User management, lab sharing, firmware metadata, and agent-session
//! endpoints — all backed by [`netpilot_db`] and gated behind persistence.

use axum::extract::{Path, State};
use axum::Json;
use netpilot_db::{Access, Role, Visibility};
use serde::Deserialize;
use uuid::Uuid;

use crate::api::auth::{require_view, Admin, Auth};
use crate::error::{ApiError, ApiResult};
use crate::state::AppState;

fn db(state: &AppState) -> ApiResult<&netpilot_db::Db> {
    state
        .db
        .as_ref()
        .ok_or_else(|| ApiError::bad_request("user management requires database persistence"))
}

// ---- users (admin only) ----------------------------------------------------

pub async fn list_users(
    State(state): State<AppState>,
    Admin(_): Admin,
) -> ApiResult<Json<serde_json::Value>> {
    let users = db(&state)?
        .list_users()
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(Json(serde_json::json!(users)))
}

#[derive(Deserialize)]
pub struct CreateUser {
    pub username: String,
    pub password: String,
    pub role: String,
}

pub async fn create_user(
    State(state): State<AppState>,
    Admin(admin): Admin,
    Json(req): Json<CreateUser>,
) -> ApiResult<Json<serde_json::Value>> {
    let role = Role::parse(&req.role).ok_or_else(|| ApiError::bad_request("invalid role"))?;
    if req.username.trim().is_empty() || req.password.len() < 4 {
        return Err(ApiError::bad_request("username required, password >= 4 chars"));
    }
    let user = db(&state)?
        .create_user(req.username.trim(), &req.password, role)
        .await
        .map_err(map_db_err)?;
    db(&state)?
        .audit(Some(admin.user_id), "user.create", Some(&user.username), Some(role.as_str()))
        .await;
    Ok(Json(serde_json::json!(user)))
}

#[derive(Deserialize)]
pub struct UpdateUser {
    pub password: Option<String>,
    pub role: Option<String>,
}

pub async fn update_user(
    State(state): State<AppState>,
    Admin(admin): Admin,
    Path(user_id): Path<Uuid>,
    Json(req): Json<UpdateUser>,
) -> ApiResult<Json<serde_json::Value>> {
    let d = db(&state)?;
    if let Some(pw) = req.password.as_deref() {
        if pw.len() < 4 {
            return Err(ApiError::bad_request("password >= 4 chars"));
        }
        d.set_password(user_id, pw).await.map_err(map_db_err)?;
    }
    if let Some(r) = req.role.as_deref() {
        let role = Role::parse(r).ok_or_else(|| ApiError::bad_request("invalid role"))?;
        d.set_role(user_id, role).await.map_err(map_db_err)?;
    }
    d.audit(Some(admin.user_id), "user.update", Some(&user_id.to_string()), None).await;
    Ok(Json(serde_json::json!({ "ok": true })))
}

// ---- lab sharing -----------------------------------------------------------

pub async fn get_shares(
    State(state): State<AppState>,
    Auth(principal): Auth,
    Path(lab_id): Path<Uuid>,
) -> ApiResult<Json<serde_json::Value>> {
    require_view(&state, &principal, lab_id).await?;
    let d = db(&state)?;
    let meta = d
        .lab_meta(lab_id)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    let shares = d
        .shares_for(lab_id)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(Json(serde_json::json!({
        "visibility": meta.as_ref().map(|m| m.visibility.as_str()).unwrap_or("private"),
        "owner": meta.as_ref().map(|m| m.owner_name.clone()),
        "shares": shares,
    })))
}

#[derive(Deserialize)]
pub struct ShareRequest {
    /// Grant to this username (mutually exclusive with `visibility`).
    pub username: Option<String>,
    /// 'view' or 'edit'.
    pub access: Option<String>,
    /// Set lab visibility: 'private' or 'public'.
    pub visibility: Option<String>,
}

/// Only the owner (or admin) may change sharing.
async fn require_owner(state: &AppState, principal: &netpilot_db::Principal, lab_id: Uuid) -> ApiResult<()> {
    let access = crate::api::auth::lab_access(state, principal, lab_id).await?;
    if access == Access::Own {
        Ok(())
    } else {
        Err(ApiError {
            status: axum::http::StatusCode::FORBIDDEN,
            message: "only the lab owner can change sharing".into(),
        })
    }
}

pub async fn update_share(
    State(state): State<AppState>,
    Auth(principal): Auth,
    Path(lab_id): Path<Uuid>,
    Json(req): Json<ShareRequest>,
) -> ApiResult<Json<serde_json::Value>> {
    require_owner(&state, &principal, lab_id).await?;
    let d = db(&state)?;
    if let Some(v) = req.visibility.as_deref() {
        d.set_visibility(lab_id, Visibility::parse(v)).await.map_err(map_db_err)?;
    }
    if let Some(username) = req.username.as_deref() {
        let target = d.find_user_by_name(username).await.map_err(|_| ApiError::not_found("no such user"))?;
        let access = match req.access.as_deref() {
            Some("edit") => Access::Edit,
            _ => Access::View,
        };
        d.share_lab(lab_id, target.id, access).await.map_err(map_db_err)?;
        d.audit(Some(principal.user_id), "lab.share", Some(&lab_id.to_string()), Some(username)).await;
    }
    get_shares(State(state), Auth(principal), Path(lab_id)).await
}

pub async fn revoke_share(
    State(state): State<AppState>,
    Auth(principal): Auth,
    Path((lab_id, username)): Path<(Uuid, String)>,
) -> ApiResult<Json<serde_json::Value>> {
    require_owner(&state, &principal, lab_id).await?;
    let d = db(&state)?;
    let target = d.find_user_by_name(&username).await.map_err(|_| ApiError::not_found("no such user"))?;
    d.unshare_lab(lab_id, target.id).await.map_err(map_db_err)?;
    d.audit(Some(principal.user_id), "lab.unshare", Some(&lab_id.to_string()), Some(&username)).await;
    Ok(Json(serde_json::json!({ "ok": true })))
}

// ---- agent sessions --------------------------------------------------------

pub async fn list_sessions(
    State(state): State<AppState>,
    Auth(principal): Auth,
    Path(lab_id): Path<Uuid>,
) -> ApiResult<Json<serde_json::Value>> {
    require_view(&state, &principal, lab_id).await?;
    let sessions = db(&state)?
        .list_agent_sessions(lab_id, &principal)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(Json(serde_json::json!(sessions)))
}

pub async fn get_session(
    State(state): State<AppState>,
    Auth(principal): Auth,
    Path((lab_id, session_id)): Path<(Uuid, Uuid)>,
) -> ApiResult<Json<serde_json::Value>> {
    require_view(&state, &principal, lab_id).await?;
    let d = db(&state)?;
    // A user may read their own sessions; admins any.
    let owner = d.session_owner(session_id).await.map_err(|_| ApiError::not_found("session not found"))?;
    if owner != principal.user_id && !principal.role.is_admin() {
        return Err(ApiError::not_found("session not found"));
    }
    let events = d
        .agent_events(session_id)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(Json(serde_json::json!({ "id": session_id, "events": events })))
}

fn map_db_err(e: netpilot_db::DbError) -> ApiError {
    match e {
        netpilot_db::DbError::Conflict(m) => ApiError::conflict(m),
        netpilot_db::DbError::NotFound => ApiError::not_found("not found"),
        other => ApiError::internal(other.to_string()),
    }
}
