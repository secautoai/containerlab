//! Node process lifecycle.
//!
//! Owns the QEMU child processes and drives the state machine
//! stopped → starting → running → stopping → stopped (or error), publishing
//! transitions on the core event bus.

use std::collections::HashMap;
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;

use netpilot_core::{Event, EventBus, NodeState};
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::cmdline::NodeBootSpec;
use crate::qmp::QmpClient;
use crate::{QemuError, Result};

struct RunningNode {
    child: tokio::process::Child,
    spec: NodeBootSpec,
}

/// Manages all running QEMU processes across labs.
#[derive(Clone)]
pub struct NodeSupervisor {
    inner: Arc<Mutex<HashMap<(Uuid, Uuid), RunningNode>>>,
    events: EventBus,
}

/// How long to wait after ACPI powerdown before escalating to SIGKILL.
const GRACEFUL_STOP: Duration = Duration::from_secs(20);

impl NodeSupervisor {
    pub fn new(events: EventBus) -> Self {
        Self {
            inner: Arc::new(Mutex::new(HashMap::new())),
            events,
        }
    }

    fn publish(&self, lab: Uuid, node: Uuid, state: NodeState, detail: Option<String>) {
        self.events.publish(Event::NodeState {
            lab,
            node,
            state,
            detail,
        });
    }

    pub async fn is_running(&self, lab: Uuid, node: Uuid) -> bool {
        self.inner.lock().await.contains_key(&(lab, node))
    }

    pub async fn running_nodes(&self, lab: Uuid) -> Vec<Uuid> {
        self.inner
            .lock()
            .await
            .keys()
            .filter(|(l, _)| *l == lab)
            .map(|(_, n)| *n)
            .collect()
    }

    pub async fn running_count(&self) -> usize {
        self.inner.lock().await.len()
    }

    /// Spawn QEMU for a prepared boot spec. The caller must have created
    /// the overlay and config media already.
    pub async fn start(&self, spec: NodeBootSpec) -> Result<()> {
        let key = (spec.lab_id, spec.node_id);
        {
            let running = self.inner.lock().await;
            if running.contains_key(&key) {
                return Ok(()); // already running — idempotent
            }
        }
        self.publish(spec.lab_id, spec.node_id, NodeState::Starting, None);

        std::fs::create_dir_all(&spec.run_dir)?;
        std::fs::create_dir_all(&spec.socket_dir)?;
        // Stale sockets from a previous run would make QEMU fail to bind.
        let _ = std::fs::remove_file(spec.qmp_socket());
        let _ = std::fs::remove_file(spec.console_socket());

        let binary = spec.qemu_binary();
        let args = spec.build_args();
        tracing::info!(node = %spec.name, "starting: {}", crate::render_cmdline(&binary, &args));

        let log = std::fs::File::create(spec.run_dir.join("qemu.log"))?;
        let child = tokio::process::Command::new(&binary)
            .args(&args)
            .stdin(Stdio::null())
            .stdout(Stdio::from(log.try_clone()?))
            .stderr(Stdio::from(log))
            .kill_on_drop(true)
            .spawn()
            .map_err(|e| {
                if e.kind() == std::io::ErrorKind::NotFound {
                    QemuError::QemuMissing(binary.clone())
                } else {
                    QemuError::Io(e)
                }
            });

        let mut child = match child {
            Ok(c) => c,
            Err(e) => {
                self.publish(
                    spec.lab_id,
                    spec.node_id,
                    NodeState::Error,
                    Some(e.to_string()),
                );
                return Err(e);
            }
        };

        // Give QEMU a moment to fail fast on bad arguments/images.
        tokio::time::sleep(Duration::from_millis(400)).await;
        if let Ok(Some(status)) = child.try_wait() {
            let log_tail: String = std::fs::read_to_string(spec.run_dir.join("qemu.log"))
                .map(|s| {
                    let tail: String = s.chars().rev().take(500).collect();
                    tail.chars().rev().collect()
                })
                .unwrap_or_default();
            let detail = format!("qemu exited immediately ({status}): {log_tail}");
            self.publish(
                spec.lab_id,
                spec.node_id,
                NodeState::Error,
                Some(detail.clone()),
            );
            return Err(QemuError::Other(detail));
        }

        self.inner
            .lock()
            .await
            .insert(key, RunningNode { child, spec: spec.clone() });
        self.publish(spec.lab_id, spec.node_id, NodeState::Running, None);

        // Reaper: notice unexpected exits and clean up state.
        let supervisor = self.clone();
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_secs(2)).await;
                let mut running = supervisor.inner.lock().await;
                let Some(entry) = running.get_mut(&key) else { return };
                match entry.child.try_wait() {
                    Ok(Some(status)) => {
                        running.remove(&key);
                        drop(running);
                        tracing::info!(?key, "qemu exited: {status}");
                        supervisor.publish(key.0, key.1, NodeState::Stopped, Some(format!("exited: {status}")));
                        return;
                    }
                    Ok(None) => {}
                    Err(_) => return,
                }
            }
        });

        Ok(())
    }

    /// Stop a node: try ACPI powerdown over QMP, then escalate to kill.
    pub async fn stop(&self, lab: Uuid, node: Uuid) -> Result<()> {
        let key = (lab, node);
        let entry = {
            let mut running = self.inner.lock().await;
            running.remove(&key)
        };
        let Some(mut entry) = entry else {
            return Ok(()); // already stopped — idempotent
        };
        self.publish(lab, node, NodeState::Stopping, None);

        let graceful = async {
            let mut qmp = QmpClient::connect(&entry.spec.qmp_socket()).await.ok()?;
            qmp.system_powerdown().await.ok()?;
            Some(())
        };
        if graceful.await.is_some() {
            match tokio::time::timeout(GRACEFUL_STOP, entry.child.wait()).await {
                Ok(_) => {
                    self.publish(lab, node, NodeState::Stopped, None);
                    return Ok(());
                }
                Err(_) => tracing::warn!(?key, "graceful stop timed out; killing"),
            }
        }
        let _ = entry.child.kill().await;
        let _ = entry.child.wait().await;
        self.publish(lab, node, NodeState::Stopped, None);
        Ok(())
    }

    /// Immediately kill every node of a lab (lab stop / delete).
    pub async fn stop_lab(&self, lab: Uuid) -> Result<()> {
        for node in self.running_nodes(lab).await {
            self.stop(lab, node).await?;
        }
        Ok(())
    }

    /// Path to a running node's serial console socket.
    pub async fn console_socket(&self, lab: Uuid, node: Uuid) -> Result<std::path::PathBuf> {
        let running = self.inner.lock().await;
        running
            .get(&(lab, node))
            .map(|e| e.spec.console_socket())
            .ok_or(QemuError::NotRunning)
    }

    /// Set guest-visible carrier state for an interface of a running node.
    pub async fn set_link(&self, lab: Uuid, node: Uuid, iface: u32, up: bool) -> Result<()> {
        let sock = {
            let running = self.inner.lock().await;
            running
                .get(&(lab, node))
                .map(|e| e.spec.qmp_socket())
                .ok_or(QemuError::NotRunning)?
        };
        let mut qmp = QmpClient::connect(&sock).await?;
        qmp.set_link(&format!("np{iface}"), up).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use netpilot_core::{ConsoleKind, QemuSpec};

    fn boot_spec(dir: &std::path::Path, binary_arch: &str) -> NodeBootSpec {
        NodeBootSpec {
            lab_id: Uuid::new_v4(),
            node_id: Uuid::new_v4(),
            name: "t1".into(),
            qemu: QemuSpec {
                arch: binary_arch.into(),
                ..Default::default()
            },
            cpus: 1,
            ram_mb: 128,
            interfaces: 0,
            console: ConsoleKind::Serial,
            overlay_disk: dir.join("disk.qcow2"),
            extra_disks: vec![],
            config_media: None,
            nics: vec![],
            run_dir: dir.to_path_buf(),
            socket_dir: dir.to_path_buf(),
            kvm: false,
            vnc_display: None,
        }
    }

    #[tokio::test]
    async fn missing_binary_reports_and_sets_error_state() {
        let dir = tempfile::tempdir().unwrap();
        let events = EventBus::new();
        let mut rx = events.subscribe();
        let sup = NodeSupervisor::new(events.clone());
        let spec = boot_spec(dir.path(), "definitely-not-an-arch");

        let err = sup.start(spec.clone()).await.unwrap_err();
        assert!(matches!(err, QemuError::QemuMissing(_)));

        // starting then error events
        let e1 = rx.recv().await.unwrap();
        assert!(matches!(e1, Event::NodeState { state: NodeState::Starting, .. }));
        let e2 = rx.recv().await.unwrap();
        assert!(matches!(e2, Event::NodeState { state: NodeState::Error, .. }));
        assert!(!sup.is_running(spec.lab_id, spec.node_id).await);
    }

    #[tokio::test]
    async fn stop_of_stopped_node_is_idempotent() {
        let sup = NodeSupervisor::new(EventBus::new());
        sup.stop(Uuid::new_v4(), Uuid::new_v4()).await.unwrap();
    }
}
