//! Lab collection endpoints.

use axum::extract::{Path, State};
use axum::Json;
use netpilot_core::{Event, Lab, LabSummary, NodeState};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

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

pub async fn list(State(state): State<AppState>) -> ApiResult<Json<Vec<LabSummary>>> {
    Ok(Json(state.store.list()?))
}

pub async fn create(
    State(state): State<AppState>,
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
    state.events.publish(Event::LabCreated { lab: lab.id });
    Ok(Json(lab))
}

pub async fn get_lab(
    State(state): State<AppState>,
    Path(lab_id): Path<Uuid>,
) -> ApiResult<Json<LabView>> {
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
    Path(lab_id): Path<Uuid>,
    Json(req): Json<UpdateLab>,
) -> ApiResult<Json<Lab>> {
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
    Path(lab_id): Path<Uuid>,
) -> ApiResult<Json<serde_json::Value>> {
    state.stop_lab(lab_id).await?;
    state.store.delete(lab_id)?;
    state.events.publish(Event::LabDeleted { lab: lab_id });
    Ok(Json(serde_json::json!({ "deleted": lab_id })))
}

pub async fn clone_lab(
    State(state): State<AppState>,
    Path(lab_id): Path<Uuid>,
) -> ApiResult<Json<Lab>> {
    let source = state.store.load(lab_id)?;
    let mut copy = source.clone();
    copy.id = Uuid::new_v4();
    copy.name = format!("{} (copy)", source.name);
    copy.created_at = chrono::Utc::now();
    copy.touch();
    state.store.save(&copy)?;
    state.events.publish(Event::LabCreated { lab: copy.id });
    Ok(Json(copy))
}

#[derive(Deserialize)]
pub struct SetLock {
    pub locked: bool,
}

pub async fn set_lock(
    State(state): State<AppState>,
    Path(lab_id): Path<Uuid>,
    Json(req): Json<SetLock>,
) -> ApiResult<Json<Lab>> {
    state.set_locked(lab_id, req.locked).await.map(Json)
}

pub async fn start(
    State(state): State<AppState>,
    Path(lab_id): Path<Uuid>,
) -> ApiResult<Json<serde_json::Value>> {
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
    Path(lab_id): Path<Uuid>,
) -> ApiResult<Json<serde_json::Value>> {
    state.stop_lab(lab_id).await?;
    Ok(Json(serde_json::json!({ "stopped": lab_id })))
}
