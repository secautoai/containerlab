//! Authentication + RBAC.
//!
//! When persistence is enabled (`state.auth_enabled()`), every request must
//! carry a bearer token (`Authorization: Bearer <t>` or `?token=` for
//! WebSockets). Extractors resolve it to a [`Principal`]; guard extractors
//! enforce role/lab-access requirements. With persistence off, a synthetic
//! single-user admin principal is returned so all existing behavior stands.

use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use axum::http::StatusCode;
use axum::Json;
use netpilot_db::{Access, Principal, Role};
use serde::Deserialize;
use uuid::Uuid;

use crate::error::ApiError;
use crate::state::AppState;

/// The synthetic principal used when persistence is disabled: a local admin
/// so single-user, file-only deployments behave exactly as before.
fn local_admin() -> Principal {
    Principal {
        user_id: Uuid::nil(),
        username: "local".into(),
        role: Role::Admin,
    }
}

fn bearer_header(headers: &axum::http::HeaderMap) -> Option<String> {
    headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer "))
        .map(|s| s.trim().to_string())
}

/// Pull `token=<t>` out of a query string (for WebSocket upgrades, which
/// can't carry an Authorization header).
fn query_token(query: Option<&str>) -> Option<String> {
    query?
        .split('&')
        .filter_map(|kv| kv.split_once('='))
        .find(|(k, _)| *k == "token")
        .map(|(_, v)| urldecode(v))
}

fn urldecode(s: &str) -> String {
    // Tokens are hex, so this only needs to handle '%NN' defensively.
    let bytes = s.as_bytes();
    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(b) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                out.push(b as char);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i] as char);
        i += 1;
    }
    out
}

/// Resolve a caller to a principal from a bearer header and/or query string
/// (or the local admin when auth is off).
pub async fn principal_from(
    state: &AppState,
    headers: &axum::http::HeaderMap,
    query: Option<&str>,
) -> Result<Principal, ApiError> {
    let (Some(db), Some(tokens)) = (state.db.as_ref(), state.tokens.as_ref()) else {
        return Ok(local_admin());
    };
    // Header first, then ?token= (WebSocket upgrades can't set headers).
    let token = bearer_header(headers).or_else(|| query_token(query));
    let Some(token) = token else {
        return Err(unauthorized());
    };
    let Some(user_id) = tokens.resolve(&token).await else {
        return Err(unauthorized());
    };
    db.principal(user_id).await.map_err(|_| unauthorized())
}

/// Resolve the caller to a principal from request parts.
pub async fn principal_of(state: &AppState, parts: &Parts) -> Result<Principal, ApiError> {
    principal_from(state, &parts.headers, parts.uri.query()).await
}

fn unauthorized() -> ApiError {
    ApiError {
        status: StatusCode::UNAUTHORIZED,
        message: "authentication required".into(),
    }
}

fn forbidden(msg: &str) -> ApiError {
    ApiError {
        status: StatusCode::FORBIDDEN,
        message: msg.into(),
    }
}

/// Extractor: any authenticated caller.
pub struct Auth(pub Principal);

impl FromRequestParts<AppState> for Auth {
    type Rejection = ApiError;
    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, Self::Rejection> {
        Ok(Auth(principal_of(state, parts).await?))
    }

}

/// Extractor: caller must be able to write (admin or operator).
pub struct Writer(pub Principal);

impl FromRequestParts<AppState> for Writer {
    type Rejection = ApiError;
    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, Self::Rejection> {
        let p = principal_of(state, parts).await?;
        if !p.role.can_write() {
            return Err(forbidden("this action requires operator or admin"));
        }
        Ok(Writer(p))
    }
}

/// Extractor: caller must be an admin.
pub struct Admin(pub Principal);

impl FromRequestParts<AppState> for Admin {
    type Rejection = ApiError;
    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, Self::Rejection> {
        let p = principal_of(state, parts).await?;
        if !p.role.is_admin() {
            return Err(forbidden("admin only"));
        }
        Ok(Admin(p))
    }
}

/// Resolve a principal's access to a lab (Access::Own when auth is off).
pub async fn lab_access(state: &AppState, p: &Principal, lab_id: Uuid) -> Result<Access, ApiError> {
    match state.db.as_ref() {
        Some(db) => db
            .access_for(p, lab_id)
            .await
            .map_err(|e| ApiError::internal(e.to_string())),
        None => Ok(Access::Own),
    }
}

/// Require at least view access to a lab, else 403/404.
pub async fn require_view(state: &AppState, p: &Principal, lab_id: Uuid) -> Result<Access, ApiError> {
    let a = lab_access(state, p, lab_id).await?;
    if a.can_view() {
        Ok(a)
    } else {
        Err(ApiError::not_found("lab not found"))
    }
}

/// Require edit access to a lab, else 403.
pub async fn require_edit(state: &AppState, p: &Principal, lab_id: Uuid) -> Result<Access, ApiError> {
    let a = lab_access(state, p, lab_id).await?;
    if a.can_edit() {
        Ok(a)
    } else if a.can_view() {
        Err(forbidden("you have read-only access to this lab"))
    } else {
        Err(ApiError::not_found("lab not found"))
    }
}

// ---- login / logout / me handlers -----------------------------------------

#[derive(Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

pub async fn login(
    axum::extract::State(state): axum::extract::State<AppState>,
    Json(req): Json<LoginRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let (Some(db), Some(tokens)) = (state.db.as_ref(), state.tokens.as_ref()) else {
        return Err(ApiError::bad_request("authentication is not enabled on this server"));
    };
    let principal = db
        .authenticate(&req.username, &req.password)
        .await
        .map_err(|_| ApiError {
            status: StatusCode::UNAUTHORIZED,
            message: "invalid username or password".into(),
        })?;
    let token = tokens
        .issue(principal.user_id)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    db.audit(Some(principal.user_id), "login", None, None).await;
    Ok(Json(serde_json::json!({
        "token": token,
        "user": { "id": principal.user_id, "username": principal.username, "role": principal.role.as_str() }
    })))
}

pub async fn logout(
    axum::extract::State(state): axum::extract::State<AppState>,
    headers: axum::http::HeaderMap,
) -> Result<Json<serde_json::Value>, ApiError> {
    if let Some(tokens) = state.tokens.as_ref() {
        if let Some(token) = bearer_header(&headers) {
            let _ = tokens.revoke(&token).await;
        }
    }
    Ok(Json(serde_json::json!({ "ok": true })))
}

pub async fn me(
    axum::extract::State(state): axum::extract::State<AppState>,
    Auth(p): Auth,
) -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "authenticated": state.auth_enabled(),
        "user": { "id": p.user_id, "username": p.username, "role": p.role.as_str() }
    }))
}
