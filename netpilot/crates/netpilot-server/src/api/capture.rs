//! Packet capture endpoints: per-interface pcap on the UDP switch.

use axum::extract::{Path, State};
use axum::http::header;
use axum::response::IntoResponse;
use axum::Json;
use netpilot_net::PortId;
use uuid::Uuid;

use crate::error::{ApiError, ApiResult};
use crate::state::AppState;

fn capture_path(state: &AppState, lab: Uuid, node: Uuid, iface: u32) -> std::path::PathBuf {
    let dir = state
        .store
        .data_dir()
        .join("captures")
        .join(lab.to_string());
    let _ = std::fs::create_dir_all(&dir);
    dir.join(format!("{node}-{iface}.pcap"))
}

pub async fn start(
    State(state): State<AppState>,
    Path((lab_id, node_id, iface)): Path<(Uuid, Uuid, u32)>,
) -> ApiResult<Json<serde_json::Value>> {
    let switch = state.switch_for(lab_id).await;
    let path = capture_path(&state, lab_id, node_id, iface);
    switch
        .start_capture(
            PortId {
                node: node_id,
                iface,
            },
            &path,
        )
        .map_err(|e| ApiError::conflict(format!("capture: {e} (node must be running)")))?;
    state.events.log(
        Some(lab_id),
        "info",
        format!("capture started on {node_id}/{iface}"),
    );
    Ok(Json(serde_json::json!({ "capturing": true })))
}

pub async fn stop(
    State(state): State<AppState>,
    Path((lab_id, node_id, iface)): Path<(Uuid, Uuid, u32)>,
) -> ApiResult<Json<serde_json::Value>> {
    let switch = state.switch_for(lab_id).await;
    switch
        .stop_capture(PortId {
            node: node_id,
            iface,
        })
        .map_err(|e| ApiError::conflict(e.to_string()))?;
    Ok(Json(serde_json::json!({ "capturing": false })))
}

pub async fn download(
    State(state): State<AppState>,
    Path((lab_id, node_id, iface)): Path<(Uuid, Uuid, u32)>,
) -> ApiResult<impl IntoResponse> {
    let path = capture_path(&state, lab_id, node_id, iface);
    let data = std::fs::read(&path)
        .map_err(|_| ApiError::not_found("no capture file — start a capture first"))?;
    Ok((
        [
            (header::CONTENT_TYPE, "application/vnd.tcpdump.pcap"),
            (
                header::CONTENT_DISPOSITION,
                "attachment; filename=\"capture.pcap\"",
            ),
        ],
        data,
    ))
}
