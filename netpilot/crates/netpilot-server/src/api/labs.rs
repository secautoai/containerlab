//! Lab collection endpoints.

use axum::extract::{Path, State};
use axum::Json;
use netpilot_core::{Event, Lab, LabSummary, NodeState};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::api::auth::{Auth, Writer};
use crate::error::{ApiError, ApiResult};
use crate::state::AppState;

#[derive(Deserialize)]
pub struct CreateLab {
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub author: String,
    #[serde(default)]
    pub folder: Option<String>,
}

#[derive(Serialize)]
pub struct LabView {
    #[serde(flatten)]
    pub lab: Lab,
    /// Runtime state per node id.
    pub states: std::collections::HashMap<Uuid, NodeState>,
    pub kvm: bool,
}

pub async fn list(
    State(state): State<AppState>,
    Auth(principal): Auth,
) -> ApiResult<Json<Vec<LabSummary>>> {
    let all = state.store.list()?;
    // With persistence on, filter to labs this principal may see.
    let Some(db) = state.db.as_ref() else {
        return Ok(Json(all));
    };
    let visible: std::collections::HashSet<Uuid> = db
        .visible_lab_ids(&principal)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?
        .into_iter()
        .collect();
    Ok(Json(
        all.into_iter().filter(|l| visible.contains(&l.id)).collect(),
    ))
}

pub async fn create(
    State(state): State<AppState>,
    Writer(principal): Writer,
    Json(req): Json<CreateLab>,
) -> ApiResult<Json<Lab>> {
    if req.name.trim().is_empty() {
        return Err(ApiError::bad_request("lab name must not be empty"));
    }
    let mut lab = Lab::new(req.name.trim());
    lab.description = req.description;
    lab.author = req.author;
    if let Some(folder) = req.folder {
        lab.folder = folder;
    }
    state.store.save(&lab)?;
    if let Some(db) = state.db.as_ref() {
        db.register_lab(lab.id, principal.user_id, &lab.name)
            .await
            .map_err(|e| ApiError::internal(e.to_string()))?;
        db.audit(Some(principal.user_id), "lab.create", Some(&lab.id.to_string()), Some(&lab.name)).await;
    }
    state.events.publish(Event::LabCreated { lab: lab.id });
    Ok(Json(lab))
}

pub async fn get_lab(
    State(state): State<AppState>,
    Auth(principal): Auth,
    Path(lab_id): Path<Uuid>,
) -> ApiResult<Json<LabView>> {
    crate::api::auth::require_view(&state, &principal, lab_id).await?;
    let lab = state.store.load(lab_id)?;
    let states = state.lab_states(lab_id).await;
    Ok(Json(LabView {
        lab,
        states,
        kvm: state.kvm(),
    }))
}

#[derive(Deserialize)]
pub struct UpdateLab {
    pub name: Option<String>,
    pub description: Option<String>,
    pub author: Option<String>,
    pub folder: Option<String>,
    pub body: Option<String>,
}

pub async fn update(
    State(state): State<AppState>,
    Auth(principal): Auth,
    Path(lab_id): Path<Uuid>,
    Json(req): Json<UpdateLab>,
) -> ApiResult<Json<Lab>> {
    crate::api::auth::require_edit(&state, &principal, lab_id).await?;
    state
        .mutate_lab(lab_id, |lab| {
            if let Some(name) = req.name {
                if name.trim().is_empty() {
                    return Err(ApiError::bad_request("lab name must not be empty"));
                }
                lab.name = name;
            }
            if let Some(d) = req.description {
                lab.description = d;
            }
            if let Some(a) = req.author {
                lab.author = a;
            }
            if let Some(f) = req.folder {
                lab.folder = f;
            }
            if let Some(b) = req.body {
                lab.body = b;
            }
            Ok(lab.clone())
        })
        .await
        .map(Json)
}

pub async fn remove(
    State(state): State<AppState>,
    Auth(principal): Auth,
    Path(lab_id): Path<Uuid>,
) -> ApiResult<Json<serde_json::Value>> {
    // Only the owner (Own) or an admin may delete.
    let access = crate::api::auth::lab_access(&state, &principal, lab_id).await?;
    if access != netpilot_db::Access::Own {
        return Err(ApiError {
            status: axum::http::StatusCode::FORBIDDEN,
            message: "only the lab owner can delete it".into(),
        });
    }
    state.stop_lab(lab_id).await?;
    state.store.delete(lab_id)?;
    if let Some(db) = state.db.as_ref() {
        let _ = db.forget_lab(lab_id).await;
        db.audit(Some(principal.user_id), "lab.delete", Some(&lab_id.to_string()), None).await;
    }
    state.events.publish(Event::LabDeleted { lab: lab_id });
    Ok(Json(serde_json::json!({ "deleted": lab_id })))
}

pub async fn clone_lab(
    State(state): State<AppState>,
    Writer(principal): Writer,
    Path(lab_id): Path<Uuid>,
) -> ApiResult<Json<Lab>> {
    crate::api::auth::require_view(&state, &principal, lab_id).await?;
    let source = state.store.load(lab_id)?;
    let mut copy = source.clone();
    copy.id = Uuid::new_v4();
    copy.name = format!("{} (copy)", source.name);
    copy.created_at = chrono::Utc::now();
    copy.touch();
    state.store.save(&copy)?;
    if let Some(db) = state.db.as_ref() {
        // The clone belongs to whoever made it.
        let _ = db.register_lab(copy.id, principal.user_id, &copy.name).await;
    }
    state.events.publish(Event::LabCreated { lab: copy.id });
    Ok(Json(copy))
}

/// Config sets: names in use across the lab plus the active one.
#[derive(Serialize)]
pub struct ConfigSetsView {
    pub active: String,
    pub sets: Vec<String>,
}

pub async fn config_sets(
    State(state): State<AppState>,
    Auth(principal): Auth,
    Path(lab_id): Path<Uuid>,
) -> ApiResult<Json<ConfigSetsView>> {
    crate::api::auth::require_view(&state, &principal, lab_id).await?;
    config_sets_view(&state, lab_id).await
}

async fn config_sets_view(state: &AppState, lab_id: Uuid) -> ApiResult<Json<ConfigSetsView>> {
    let lab = state.store.load(lab_id)?;
    let mut sets: Vec<String> = lab
        .nodes
        .values()
        .flat_map(|n| n.config_sets.keys().cloned())
        .collect();
    sets.sort();
    sets.dedup();
    Ok(Json(ConfigSetsView {
        active: lab.active_config_set.clone(),
        sets,
    }))
}

#[derive(Deserialize)]
pub struct ActivateSet {
    /// Set name; empty string returns to each node's default config.
    pub name: String,
}

pub async fn activate_config_set(
    State(state): State<AppState>,
    Auth(principal): Auth,
    Path(lab_id): Path<Uuid>,
    Json(req): Json<ActivateSet>,
) -> ApiResult<Json<ConfigSetsView>> {
    crate::api::auth::require_edit(&state, &principal, lab_id).await?;
    state
        .mutate_lab(lab_id, |lab| {
            lab.active_config_set = req.name;
            Ok(())
        })
        .await?;
    config_sets_view(&state, lab_id).await
}

/// Snapshot every node's *current default* startup config into a named set.
pub async fn snapshot_config_set(
    State(state): State<AppState>,
    Auth(principal): Auth,
    Path((lab_id, name)): Path<(Uuid, String)>,
) -> ApiResult<Json<ConfigSetsView>> {
    crate::api::auth::require_edit(&state, &principal, lab_id).await?;
    if name.trim().is_empty() {
        return Err(ApiError::bad_request("set name must not be empty"));
    }
    let name = name.trim().to_string();
    state
        .mutate_lab(lab_id, |lab| {
            for node in lab.nodes.values_mut() {
                if let Some(cfg) = node.startup_config.clone() {
                    node.config_sets.insert(name.clone(), cfg);
                }
            }
            Ok(())
        })
        .await?;
    config_sets_view(&state, lab_id).await
}

pub async fn delete_config_set(
    State(state): State<AppState>,
    Auth(principal): Auth,
    Path((lab_id, name)): Path<(Uuid, String)>,
) -> ApiResult<Json<ConfigSetsView>> {
    crate::api::auth::require_edit(&state, &principal, lab_id).await?;
    state
        .mutate_lab(lab_id, |lab| {
            for node in lab.nodes.values_mut() {
                node.config_sets.remove(&name);
            }
            if lab.active_config_set == name {
                lab.active_config_set.clear();
            }
            Ok(())
        })
        .await?;
    config_sets_view(&state, lab_id).await
}

#[derive(Deserialize)]
pub struct SetLock {
    pub locked: bool,
}

pub async fn set_lock(
    State(state): State<AppState>,
    Auth(principal): Auth,
    Path(lab_id): Path<Uuid>,
    Json(req): Json<SetLock>,
) -> ApiResult<Json<Lab>> {
    crate::api::auth::require_edit(&state, &principal, lab_id).await?;
    state.set_locked(lab_id, req.locked).await.map(Json)
}

pub async fn start(
    State(state): State<AppState>,
    Auth(principal): Auth,
    Path(lab_id): Path<Uuid>,
) -> ApiResult<Json<serde_json::Value>> {
    crate::api::auth::require_edit(&state, &principal, lab_id).await?;
    // Fire and continue in background: boot delays can be long.
    let s = state.clone();
    tokio::spawn(async move {
        if let Err(e) = s.start_lab(lab_id).await {
            s.events
                .log(Some(lab_id), "error", format!("lab start: {e}"));
        }
    });
    Ok(Json(serde_json::json!({ "starting": lab_id })))
}

pub async fn stop(
    State(state): State<AppState>,
    Auth(principal): Auth,
    Path(lab_id): Path<Uuid>,
) -> ApiResult<Json<serde_json::Value>> {
    crate::api::auth::require_edit(&state, &principal, lab_id).await?;
    state.stop_lab(lab_id).await?;
    Ok(Json(serde_json::json!({ "stopped": lab_id })))
}
