//! Networks, links and annotations.

use axum::extract::{Path, State};
use axum::Json;
use netpilot_core::{Annotation, AnnotationKind, Endpoint, Impairment, Link, Network, NetworkKind};
use serde::Deserialize;
use uuid::Uuid;

use crate::api::auth::{require_edit, require_view, Auth};
use crate::error::{ApiError, ApiResult};
use crate::state::AppState;

// ---------- networks ----------

pub async fn list_networks(
    State(state): State<AppState>,
    Auth(principal): Auth,
    Path(lab_id): Path<Uuid>,
) -> ApiResult<Json<Vec<Network>>> {
    require_view(&state, &principal, lab_id).await?;
    let lab = state.store.load(lab_id)?;
    Ok(Json(lab.networks.values().cloned().collect()))
}

#[derive(Deserialize)]
pub struct CreateNetwork {
    pub name: Option<String>,
    #[serde(default)]
    pub kind: NetworkKind,
    pub host_interface: Option<String>,
    pub subnet: Option<String>,
    pub x: Option<f64>,
    pub y: Option<f64>,
}

pub async fn create_network(
    State(state): State<AppState>,
    Auth(principal): Auth,
    Path(lab_id): Path<Uuid>,
    Json(req): Json<CreateNetwork>,
) -> ApiResult<Json<Network>> {
    require_edit(&state, &principal, lab_id).await?;
    state
        .mutate_lab(lab_id, |lab| {
            let n = lab.networks.len() + 1;
            let network = Network {
                id: Uuid::new_v4(),
                name: req.name.unwrap_or_else(|| format!("Net{n}")),
                kind: req.kind,
                host_interface: req.host_interface,
                subnet: req.subnet,
                x: req.x.unwrap_or(300.0),
                y: req.y.unwrap_or(300.0),
            };
            lab.networks.insert(network.id, network.clone());
            Ok(network)
        })
        .await
        .map(Json)
}

#[derive(Deserialize)]
pub struct UpdateNetwork {
    pub name: Option<String>,
    pub kind: Option<NetworkKind>,
    pub host_interface: Option<String>,
    pub subnet: Option<String>,
    pub x: Option<f64>,
    pub y: Option<f64>,
}

pub async fn update_network(
    State(state): State<AppState>,
    Auth(principal): Auth,
    Path((lab_id, net_id)): Path<(Uuid, Uuid)>,
    Json(req): Json<UpdateNetwork>,
) -> ApiResult<Json<Network>> {
    require_edit(&state, &principal, lab_id).await?;
    state
        .mutate_lab(lab_id, |lab| {
            let net = lab
                .networks
                .get_mut(&net_id)
                .ok_or_else(|| netpilot_core::CoreError::NetworkNotFound(net_id.to_string()))?;
            if let Some(v) = req.name {
                net.name = v;
            }
            if let Some(v) = req.kind {
                net.kind = v;
            }
            if req.host_interface.is_some() {
                net.host_interface = req.host_interface;
            }
            if req.subnet.is_some() {
                net.subnet = req.subnet;
            }
            if let Some(v) = req.x {
                net.x = v;
            }
            if let Some(v) = req.y {
                net.y = v;
            }
            Ok(net.clone())
        })
        .await
        .map(Json)
}

pub async fn remove_network(
    State(state): State<AppState>,
    Auth(principal): Auth,
    Path((lab_id, net_id)): Path<(Uuid, Uuid)>,
) -> ApiResult<Json<serde_json::Value>> {
    require_edit(&state, &principal, lab_id).await?;
    let removed_links: Vec<Uuid> = state
        .mutate_lab(lab_id, |lab| {
            let links: Vec<Uuid> = lab
                .links
                .values()
                .filter(|l| l.touches_network(net_id))
                .map(|l| l.id)
                .collect();
            lab.remove_network(net_id)?;
            Ok(links)
        })
        .await?;
    for link in removed_links {
        state.unwire_link(lab_id, link).await;
    }
    Ok(Json(serde_json::json!({ "deleted": net_id })))
}

// ---------- links ----------

pub async fn list_links(
    State(state): State<AppState>,
    Auth(principal): Auth,
    Path(lab_id): Path<Uuid>,
) -> ApiResult<Json<Vec<Link>>> {
    require_view(&state, &principal, lab_id).await?;
    let lab = state.store.load(lab_id)?;
    Ok(Json(lab.links.values().cloned().collect()))
}

#[derive(Deserialize)]
pub struct CreateLink {
    pub a: Endpoint,
    pub b: Endpoint,
    pub label: Option<String>,
    pub impairment: Option<Impairment>,
}

pub async fn create_link(
    State(state): State<AppState>,
    Auth(principal): Auth,
    Path(lab_id): Path<Uuid>,
    Json(req): Json<CreateLink>,
) -> ApiResult<Json<Link>> {
    require_edit(&state, &principal, lab_id).await?;
    if req.a == req.b {
        return Err(ApiError::bad_request("cannot link an endpoint to itself"));
    }
    let link = state
        .mutate_lab(lab_id, |lab| {
            let mut link = Link::between(req.a, req.b);
            link.label = req.label;
            link.impairment = req.impairment;
            lab.add_link(link.clone())?;
            Ok(link)
        })
        .await?;
    // Hot-wire if the endpoints are live.
    state.hot_wire_link(lab_id, link.id).await?;
    Ok(Json(link))
}

#[derive(Deserialize)]
pub struct UpdateLink {
    pub label: Option<String>,
    pub impairment: Option<Impairment>,
    pub suspended: Option<bool>,
}

pub async fn update_link(
    State(state): State<AppState>,
    Auth(principal): Auth,
    Path((lab_id, link_id)): Path<(Uuid, Uuid)>,
    Json(req): Json<UpdateLink>,
) -> ApiResult<Json<Link>> {
    require_edit(&state, &principal, lab_id).await?;
    let link = state
        .mutate_lab(lab_id, |lab| {
            let link = lab
                .links
                .get_mut(&link_id)
                .ok_or_else(|| netpilot_core::CoreError::LinkNotFound(link_id.to_string()))?;
            if req.label.is_some() {
                link.label = req.label;
            }
            link.impairment = req.impairment;
            if let Some(s) = req.suspended {
                link.suspended = s;
            }
            Ok(link.clone())
        })
        .await?;
    // Live-apply impairment/suspension in whichever datapath is active.
    state.hot_wire_link(lab_id, link_id).await?;
    if state.datapath == crate::state::DatapathMode::UdpSwitch {
        let switch = state.switch_for(lab_id).await;
        let imp = link
            .impairment
            .map(|i| netpilot_net::WireImpairment {
                delay_ms: i.delay_ms,
                jitter_ms: i.jitter_ms,
                loss_pct: i.loss_pct,
                rate_kbit: i.rate_kbit,
            })
            .unwrap_or_default();
        switch.set_impairment(link_id, imp);
        switch.set_link_suspended(link_id, link.suspended);
    }
    state.events.publish(netpilot_core::Event::LinkUpdated {
        lab: lab_id,
        link: link_id,
    });
    Ok(Json(link))
}

pub async fn remove_link(
    State(state): State<AppState>,
    Auth(principal): Auth,
    Path((lab_id, link_id)): Path<(Uuid, Uuid)>,
) -> ApiResult<Json<serde_json::Value>> {
    require_edit(&state, &principal, lab_id).await?;
    state
        .mutate_lab(lab_id, |lab| {
            lab.links
                .remove(&link_id)
                .ok_or_else(|| netpilot_core::CoreError::LinkNotFound(link_id.to_string()))?;
            Ok(())
        })
        .await?;
    state.unwire_link(lab_id, link_id).await;
    Ok(Json(serde_json::json!({ "deleted": link_id })))
}

// ---------- annotations ----------

pub async fn list_annotations(
    State(state): State<AppState>,
    Auth(principal): Auth,
    Path(lab_id): Path<Uuid>,
) -> ApiResult<Json<Vec<Annotation>>> {
    require_view(&state, &principal, lab_id).await?;
    let lab = state.store.load(lab_id)?;
    Ok(Json(lab.annotations.values().cloned().collect()))
}

#[derive(Deserialize)]
pub struct CreateAnnotation {
    pub kind: AnnotationKind,
    pub x: f64,
    pub y: f64,
    #[serde(default)]
    pub width: f64,
    #[serde(default)]
    pub height: f64,
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub color: String,
    #[serde(default)]
    pub fill: String,
    #[serde(default)]
    pub font_size: u32,
    #[serde(default)]
    pub z: i32,
}

pub async fn create_annotation(
    State(state): State<AppState>,
    Auth(principal): Auth,
    Path(lab_id): Path<Uuid>,
    Json(req): Json<CreateAnnotation>,
) -> ApiResult<Json<Annotation>> {
    require_edit(&state, &principal, lab_id).await?;
    state
        .mutate_lab(lab_id, |lab| {
            let ann = Annotation {
                id: Uuid::new_v4(),
                kind: req.kind,
                x: req.x,
                y: req.y,
                width: req.width,
                height: req.height,
                text: req.text,
                color: req.color,
                fill: req.fill,
                font_size: req.font_size,
                z: req.z,
            };
            lab.annotations.insert(ann.id, ann.clone());
            Ok(ann)
        })
        .await
        .map(Json)
}

#[derive(Deserialize)]
pub struct UpdateAnnotation {
    pub x: Option<f64>,
    pub y: Option<f64>,
    pub width: Option<f64>,
    pub height: Option<f64>,
    pub text: Option<String>,
    pub color: Option<String>,
    pub fill: Option<String>,
    pub font_size: Option<u32>,
    pub z: Option<i32>,
}

pub async fn update_annotation(
    State(state): State<AppState>,
    Auth(principal): Auth,
    Path((lab_id, ann_id)): Path<(Uuid, Uuid)>,
    Json(req): Json<UpdateAnnotation>,
) -> ApiResult<Json<Annotation>> {
    require_edit(&state, &principal, lab_id).await?;
    state
        .mutate_lab(lab_id, |lab| {
            let ann = lab.annotations.get_mut(&ann_id).ok_or_else(|| {
                netpilot_core::CoreError::Validation(format!("annotation not found: {ann_id}"))
            })?;
            if let Some(v) = req.x {
                ann.x = v;
            }
            if let Some(v) = req.y {
                ann.y = v;
            }
            if let Some(v) = req.width {
                ann.width = v;
            }
            if let Some(v) = req.height {
                ann.height = v;
            }
            if let Some(v) = req.text {
                ann.text = v;
            }
            if let Some(v) = req.color {
                ann.color = v;
            }
            if let Some(v) = req.fill {
                ann.fill = v;
            }
            if let Some(v) = req.font_size {
                ann.font_size = v;
            }
            if let Some(v) = req.z {
                ann.z = v;
            }
            Ok(ann.clone())
        })
        .await
        .map(Json)
}

pub async fn remove_annotation(
    State(state): State<AppState>,
    Auth(principal): Auth,
    Path((lab_id, ann_id)): Path<(Uuid, Uuid)>,
) -> ApiResult<Json<serde_json::Value>> {
    require_edit(&state, &principal, lab_id).await?;
    state
        .mutate_lab(lab_id, |lab| {
            lab.annotations.remove(&ann_id).ok_or_else(|| {
                netpilot_core::CoreError::Validation(format!("annotation not found: {ann_id}"))
            })?;
            Ok(())
        })
        .await?;
    Ok(Json(serde_json::json!({ "deleted": ann_id })))
}
