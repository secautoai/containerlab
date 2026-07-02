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

/// Upload a base image (streamed to disk):
/// `PUT /api/images/{template}/{version}/{filename}` with the raw bytes.
pub async fn upload_image(
    State(state): State<AppState>,
    axum::extract::Path((template, version, filename)): axum::extract::Path<(
        String,
        String,
        String,
    )>,
    body: axum::body::Body,
) -> ApiResult<Json<serde_json::Value>> {
    use futures::TryStreamExt;
    use tokio::io::AsyncWriteExt;

    let ok_name = |s: &str| {
        !s.is_empty()
            && s.chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_'))
            && !s.starts_with('.')
    };
    if !ok_name(&template) || !ok_name(&version) || !ok_name(&filename) {
        return Err(crate::error::ApiError::bad_request(
            "template/version/filename must be simple names (alphanumeric . - _)",
        ));
    }
    let allowed = ["qcow2", "img", "iso", "vmdk"];
    let ext = std::path::Path::new(&filename)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");
    if !allowed.contains(&ext) {
        return Err(crate::error::ApiError::bad_request(format!(
            "unsupported image extension '{ext}' (want {allowed:?})"
        )));
    }

    let dir = state.images.dir_for(&template, &version);
    tokio::fs::create_dir_all(&dir).await?;
    let final_path = dir.join(&filename);
    let tmp_path = dir.join(format!(".{filename}.upload"));

    let mut file = tokio::fs::File::create(&tmp_path).await?;
    let mut stream = body.into_data_stream();
    let mut written: u64 = 0;
    while let Some(chunk) = stream
        .try_next()
        .await
        .map_err(|e| crate::error::ApiError::bad_request(format!("upload aborted: {e}")))?
    {
        written += chunk.len() as u64;
        file.write_all(&chunk).await?;
    }
    file.flush().await?;
    drop(file);
    tokio::fs::rename(&tmp_path, &final_path).await?;

    Ok(Json(serde_json::json!({
        "template": template,
        "version": version,
        "filename": filename,
        "size_bytes": written,
    })))
}

/// Per-node resource usage sampled from /proc (Linux).
#[derive(Serialize)]
pub struct NodeStats {
    pub node: uuid::Uuid,
    pub rss_mb: u64,
    pub cpu_seconds: u64,
}

pub async fn lab_stats(
    State(state): State<AppState>,
    axum::extract::Path(lab_id): axum::extract::Path<uuid::Uuid>,
) -> ApiResult<Json<Vec<NodeStats>>> {
    let mut out = Vec::new();
    for (node, pid) in state.supervisor.pids(lab_id).await {
        let status = tokio::fs::read_to_string(format!("/proc/{pid}/status"))
            .await
            .unwrap_or_default();
        let rss_kb: u64 = status
            .lines()
            .find(|l| l.starts_with("VmRSS:"))
            .and_then(|l| l.split_whitespace().nth(1))
            .and_then(|v| v.parse().ok())
            .unwrap_or(0);
        let stat = tokio::fs::read_to_string(format!("/proc/{pid}/stat"))
            .await
            .unwrap_or_default();
        // fields 14+15 (utime+stime) in clock ticks; after the comm field.
        let cpu_seconds = stat
            .rsplit(')')
            .next()
            .map(|rest| {
                let f: Vec<&str> = rest.split_whitespace().collect();
                let utime: u64 = f.get(11).and_then(|v| v.parse().ok()).unwrap_or(0);
                let stime: u64 = f.get(12).and_then(|v| v.parse().ok()).unwrap_or(0);
                (utime + stime) / 100 // USER_HZ is 100 on all mainstream kernels
            })
            .unwrap_or(0);
        out.push(NodeStats {
            node,
            rss_mb: rss_kb / 1024,
            cpu_seconds,
        });
    }
    Ok(Json(out))
}
