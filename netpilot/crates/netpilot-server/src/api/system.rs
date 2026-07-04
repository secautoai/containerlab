//! System status, template catalog, image library.

use axum::extract::State;
use axum::Json;
use netpilot_core::{DiskImage, NodeTemplate};
use serde::Serialize;

use crate::error::ApiResult;
use crate::state::AppState;

#[derive(Serialize)]
pub struct AiStatus {
    pub available: bool,
    pub provider: String,
    pub model: String,
}

#[derive(Serialize)]
pub struct SystemStatus {
    pub version: &'static str,
    pub kvm: bool,
    pub qemu_available: bool,
    pub docker_available: bool,
    pub frr_available: bool,
    pub datapath: String,
    pub running_nodes: usize,
    pub labs: usize,
    pub images: usize,
    pub ai: AiStatus,
    /// True when multi-user persistence is on and a login is required.
    pub auth_enabled: bool,
}

pub async fn status(State(state): State<AppState>) -> ApiResult<Json<SystemStatus>> {
    let qemu_available = which("qemu-system-x86_64") || which("qemu-img");
    let ai = match netpilot_ai::LlmClient::from_env() {
        Ok(c) => AiStatus {
            available: true,
            provider: match c.provider {
                netpilot_ai::Provider::Anthropic => "anthropic".into(),
                netpilot_ai::Provider::OpenAiCompatible => "openai-compatible".into(),
            },
            model: c.model,
        },
        Err(_) => AiStatus {
            available: false,
            provider: String::new(),
            model: String::new(),
        },
    };
    Ok(Json(SystemStatus {
        version: env!("CARGO_PKG_VERSION"),
        kvm: state.kvm(),
        qemu_available,
        docker_available: which("docker"),
        frr_available: std::path::Path::new("/usr/lib/frr/zebra").exists(),
        datapath: format!("{:?}", state.datapath).to_lowercase(),
        running_nodes: state.supervisor.running_count().await,
        labs: state.store.list().map(|l| l.len()).unwrap_or(0),
        images: state.images.scan().map(|i| i.len()).unwrap_or(0),
        ai,
        auth_enabled: state.auth_enabled(),
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
    // Hot-reload user templates so dropped-in YAML files appear without a
    // server restart.
    if let Ok(fresh) = netpilot_core::TemplateCatalog::load(Some(&state.store.templates_dir())) {
        *state.templates.write().await = fresh;
    }
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
/// Requires write access; records firmware metadata (size + sha256) when
/// persistence is on.
pub async fn upload_image(
    State(state): State<AppState>,
    crate::api::auth::Writer(principal): crate::api::auth::Writer,
    axum::extract::Path((template, version, filename)): axum::extract::Path<(
        String,
        String,
        String,
    )>,
    body: axum::body::Body,
) -> ApiResult<Json<serde_json::Value>> {
    use futures::TryStreamExt;
    use sha2::{Digest, Sha256};
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
    let mut hasher = Sha256::new();
    while let Some(chunk) = stream
        .try_next()
        .await
        .map_err(|e| crate::error::ApiError::bad_request(format!("upload aborted: {e}")))?
    {
        written += chunk.len() as u64;
        hasher.update(&chunk);
        file.write_all(&chunk).await?;
    }
    file.flush().await?;
    drop(file);
    tokio::fs::rename(&tmp_path, &final_path).await?;
    let sha = format!("{:x}", hasher.finalize());

    if let Some(db) = state.db.as_ref() {
        let uploader = (principal.user_id != uuid::Uuid::nil()).then_some(principal.user_id);
        let _ = db
            .record_firmware(&template, &version, &filename, written as i64, Some(&sha), uploader)
            .await;
        db.audit(uploader, "firmware.upload", Some(&format!("{template}/{version}")), Some(&sha[..16])).await;
    }

    Ok(Json(serde_json::json!({
        "template": template,
        "version": version,
        "filename": filename,
        "size_bytes": written,
        "sha256": sha,
    })))
}

/// Delete a firmware image (and its metadata). Requires write access.
pub async fn delete_image(
    State(state): State<AppState>,
    crate::api::auth::Writer(principal): crate::api::auth::Writer,
    axum::extract::Path((template, version)): axum::extract::Path<(String, String)>,
) -> ApiResult<Json<serde_json::Value>> {
    let ok_name = |s: &str| {
        !s.is_empty()
            && s.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_'))
            && !s.starts_with('.')
    };
    if !ok_name(&template) || !ok_name(&version) {
        return Err(crate::error::ApiError::bad_request("invalid template/version"));
    }
    let dir = state.images.dir_for(&template, &version);
    if dir.exists() {
        tokio::fs::remove_dir_all(&dir).await?;
    }
    if let Some(db) = state.db.as_ref() {
        let _ = db.delete_firmware(&template, &version).await;
        let uploader = (principal.user_id != uuid::Uuid::nil()).then_some(principal.user_id);
        db.audit(uploader, "firmware.delete", Some(&format!("{template}/{version}")), None).await;
    }
    Ok(Json(serde_json::json!({ "deleted": format!("{template}/{version}") })))
}

/// BYOI: upload a docker image tarball for a container template.
/// `PUT /api/images/docker/{template}` with the raw tar body. Accepts both
/// `docker save` archives (docker load) and filesystem tarballs like
/// Arista cEOS (docker import). The image is tagged as the template's
/// configured reference (e.g. `ceos:byoi`).
pub async fn upload_docker_image(
    State(state): State<AppState>,
    axum::extract::Path(template_id): axum::extract::Path<String>,
    body: axum::body::Body,
) -> ApiResult<Json<serde_json::Value>> {
    use futures::TryStreamExt;
    use tokio::io::AsyncWriteExt;

    let catalog = state.templates.read().await;
    let template = catalog.get(&template_id)?.clone();
    drop(catalog);
    let target = template
        .container
        .as_ref()
        .map(|c| c.image.clone())
        .ok_or_else(|| {
            crate::error::ApiError::bad_request(format!(
                "template '{template_id}' is not a container template"
            ))
        })?;

    // Stream to a temp file (image tarballs are GB-scale).
    let dir = state.images.root().join("docker");
    tokio::fs::create_dir_all(&dir).await?;
    let tmp = dir.join(format!(".{template_id}.upload.tar"));
    let mut file = tokio::fs::File::create(&tmp).await?;
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

    let run = |args: Vec<String>| async move {
        tokio::process::Command::new("docker")
            .args(&args)
            .output()
            .await
    };

    // Try `docker load` (docker-save archives), fall back to `docker import`.
    let load = run(vec!["load".into(), "-i".into(), tmp.display().to_string()]).await?;
    let mut method = "load";
    if load.status.success() {
        // Re-tag whatever was loaded to the template's reference if needed.
        let loaded = String::from_utf8_lossy(&load.stdout);
        if let Some(name) = loaded.lines().find_map(|l| {
            l.strip_prefix("Loaded image: ")
                .map(|s| s.trim().to_string())
        }) {
            if name != target {
                let _ = run(vec!["tag".into(), name, target.clone()]).await;
            }
        }
    } else {
        method = "import";
        let import = run(vec![
            "import".into(),
            tmp.display().to_string(),
            target.clone(),
        ])
        .await?;
        if !import.status.success() {
            let _ = tokio::fs::remove_file(&tmp).await;
            return Err(crate::error::ApiError::bad_request(format!(
                "docker load failed ({}) and docker import failed ({})",
                String::from_utf8_lossy(&load.stderr).trim(),
                String::from_utf8_lossy(&import.stderr).trim()
            )));
        }
    }
    let _ = tokio::fs::remove_file(&tmp).await;

    Ok(Json(serde_json::json!({
        "template": template_id,
        "image": target,
        "method": method,
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
