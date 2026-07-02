//! System status, template catalog, image library.

use axum::extract::State;
use axum::Json;
use netpilot_core::{DiskImage, NodeTemplate};
use serde::Serialize;

use crate::error::ApiResult;
use crate::state::AppState;

#[derive(Serialize)]
pub struct SystemStatus {
    pub version: &'static str,
    pub kvm: bool,
    pub qemu_available: bool,
    pub running_nodes: usize,
    pub labs: usize,
    pub images: usize,
}

pub async fn status(State(state): State<AppState>) -> ApiResult<Json<SystemStatus>> {
    let qemu_available = which("qemu-system-x86_64") || which("qemu-img");
    Ok(Json(SystemStatus {
        version: env!("CARGO_PKG_VERSION"),
        kvm: state.kvm(),
        qemu_available,
        running_nodes: state.supervisor.running_count().await,
        labs: state.store.list().map(|l| l.len()).unwrap_or(0),
        images: state.images.scan().map(|i| i.len()).unwrap_or(0),
    }))
}

fn which(binary: &str) -> bool {
    std::env::var_os("PATH")
        .map(|paths| std::env::split_paths(&paths).any(|dir| dir.join(binary).is_file()))
        .unwrap_or(false)
}

#[derive(Serialize)]
pub struct TemplateView {
    #[serde(flatten)]
    pub template: NodeTemplate,
    /// Image versions available in the library for this template.
    pub available_images: Vec<String>,
}

pub async fn templates(State(state): State<AppState>) -> ApiResult<Json<Vec<TemplateView>>> {
    let images = state.images.scan().unwrap_or_default();
    let catalog = state.templates.read().await;
    let views = catalog
        .all()
        .map(|t| TemplateView {
            template: t.clone(),
            available_images: images
                .iter()
                .filter(|i| i.template == t.id)
                .map(|i| i.version.clone())
                .collect(),
        })
        .collect();
    Ok(Json(views))
}

pub async fn images(State(state): State<AppState>) -> ApiResult<Json<Vec<DiskImage>>> {
    Ok(Json(
        state.images.scan().map_err(crate::error::ApiError::from)?,
    ))
}
