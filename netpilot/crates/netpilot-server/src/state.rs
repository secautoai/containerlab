//! Shared application state and lab orchestration.
//!
//! `AppState` ties the crates together: the lab store and template catalog
//! (netpilot-core), the per-lab UDP switches (netpilot-net), and the QEMU
//! node supervisor (netpilot-qemu). All REST/WS handlers operate through it.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use netpilot_core::{
    ConsoleKind, CoreError, Endpoint, Event, EventBus, ImageLibrary, Lab, LabStore, Link, Node,
    NodeState, TemplateCatalog,
};
use netpilot_net::{PortId, UdpSwitch, WireImpairment};
use netpilot_qemu::{
    build_config_media, kvm_available, overlay_path, NodeBootSpec, NodeSupervisor,
};
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::error::{ApiError, ApiResult};

#[derive(Clone)]
pub struct AppState {
    pub store: Arc<LabStore>,
    pub images: Arc<ImageLibrary>,
    pub events: EventBus,
    pub supervisor: NodeSupervisor,
    /// Template catalog reloaded on demand (user templates may change).
    pub templates: Arc<RwLock<TemplateCatalog>>,
    /// One userspace switch per lab, created lazily.
    switches: Arc<RwLock<HashMap<Uuid, UdpSwitch>>>,
    /// Last observed runtime state per node (fed from the event bus).
    node_states: Arc<RwLock<HashMap<(Uuid, Uuid), NodeState>>>,
    /// Base port for UDP switch allocation.
    port_base: u16,
    /// Serialized lab mutations (load-modify-save) to avoid lost updates.
    lab_write_lock: Arc<tokio::sync::Mutex<()>>,
}

impl AppState {
    pub fn new(data_dir: PathBuf, port_base: u16) -> anyhow::Result<Self> {
        let store = LabStore::new(&data_dir)?;
        let images = ImageLibrary::new(store.images_dir());
        let events = EventBus::new();
        let supervisor = NodeSupervisor::new(events.clone());
        let templates = TemplateCatalog::load(Some(&store.templates_dir()))?;

        let state = Self {
            store: Arc::new(store),
            images: Arc::new(images),
            events: events.clone(),
            supervisor,
            templates: Arc::new(RwLock::new(templates)),
            switches: Arc::new(RwLock::new(HashMap::new())),
            node_states: Arc::new(RwLock::new(HashMap::new())),
            port_base,
            lab_write_lock: Arc::new(tokio::sync::Mutex::new(())),
        };

        // Track node states off the event bus.
        let states = state.node_states.clone();
        let mut rx = events.subscribe();
        tokio::spawn(async move {
            while let Ok(ev) = rx.recv().await {
                if let Event::NodeState {
                    lab, node, state, ..
                } = ev
                {
                    states.write().await.insert((lab, node), state);
                }
            }
        });

        Ok(state)
    }

    pub fn kvm(&self) -> bool {
        kvm_available()
    }

    /// Atomically apply a mutation to a lab document and persist it.
    pub async fn mutate_lab<T>(
        &self,
        lab_id: Uuid,
        f: impl FnOnce(&mut Lab) -> ApiResult<T>,
    ) -> ApiResult<T> {
        let _guard = self.lab_write_lock.lock().await;
        let mut lab = self.store.load(lab_id)?;
        let out = f(&mut lab)?;
        lab.touch();
        self.store.save(&lab)?;
        self.events.publish(Event::LabUpdated { lab: lab_id });
        Ok(out)
    }

    pub async fn switch_for(&self, lab: Uuid) -> UdpSwitch {
        let mut switches = self.switches.write().await;
        switches
            .entry(lab)
            .or_insert_with(|| UdpSwitch::new(self.port_base))
            .clone()
    }

    pub async fn node_state(&self, lab: Uuid, node: Uuid) -> NodeState {
        *self
            .node_states
            .read()
            .await
            .get(&(lab, node))
            .unwrap_or(&NodeState::Stopped)
    }

    pub async fn lab_states(&self, lab: Uuid) -> HashMap<Uuid, NodeState> {
        self.node_states
            .read()
            .await
            .iter()
            .filter(|((l, _), _)| *l == lab)
            .map(|((_, n), s)| (*n, *s))
            .collect()
    }

    /// Start a node: resolve template/image, prepare disk + config media,
    /// attach NICs to the lab switch, wire links, and boot.
    pub async fn start_node(&self, lab_id: Uuid, node_id: Uuid) -> ApiResult<()> {
        let lab = self.store.load(lab_id)?;
        let node = lab.node(node_id)?.clone();
        let templates = self.templates.read().await;
        let template = templates.get(&node.template)?.clone();
        drop(templates);

        // Image: node.image names a version under images/<template>/.
        let image = self.images.find(&node.template, &node.image).map_err(|_| {
            ApiError::bad_request(format!(
                "no image for template '{}' version '{}' — upload one under images/{}/{}/",
                node.template, node.image, node.template, node.image
            ))
        })?;

        let node_dir = self.store.node_dir(lab_id, node_id);
        std::fs::create_dir_all(&node_dir).map_err(CoreError::from)?;
        let overlay = overlay_path(&node_dir);
        netpilot_qemu::ensure_overlay(&image.path, &overlay).await?;

        let config_media = build_config_media(
            &template.qemu.config_delivery,
            &node.name,
            node.startup_config.as_deref(),
            &node_dir,
        )?;

        // Attach every interface to the lab switch.
        let switch = self.switch_for(lab_id).await;
        let mut nics = Vec::with_capacity(node.interfaces as usize);
        for i in 0..node.interfaces {
            let wiring = switch
                .attach(PortId {
                    node: node_id,
                    iface: i,
                })
                .await
                .map_err(|e| ApiError::internal(e.to_string()))?;
            nics.push(wiring);
        }

        let spec = NodeBootSpec {
            lab_id,
            node_id,
            name: node.name.clone(),
            qemu: template.qemu.clone(),
            cpus: node.cpus,
            ram_mb: node.ram_mb,
            interfaces: node.interfaces,
            console: node.console,
            overlay_disk: overlay,
            extra_disks: vec![],
            config_media,
            nics,
            run_dir: node_dir,
            socket_dir: netpilot_qemu::socket_dir_for(node_id),
            kvm: self.kvm(),
            vnc_display: match node.console {
                ConsoleKind::Vnc => Some(vnc_display_for(node_id)),
                ConsoleKind::Serial => None,
            },
        };

        self.supervisor.start(spec).await?;
        self.rewire_lab(&lab).await?;
        Ok(())
    }

    /// Recompute switch wiring for all links whose endpoints are attached.
    pub async fn rewire_lab(&self, lab: &Lab) -> ApiResult<()> {
        let switch = self.switch_for(lab.id).await;
        for link in lab.links.values() {
            self.apply_link(&switch, lab, link);
        }
        Ok(())
    }

    fn apply_link(&self, switch: &UdpSwitch, lab: &Lab, link: &Link) {
        let imp = link
            .impairment
            .map(|i| WireImpairment {
                delay_ms: i.delay_ms,
                jitter_ms: i.jitter_ms,
                loss_pct: i.loss_pct,
                rate_kbit: i.rate_kbit,
            })
            .unwrap_or_default();

        match (&link.a, &link.b) {
            (
                Endpoint::Node {
                    node: na,
                    iface: ia,
                },
                Endpoint::Node {
                    node: nb,
                    iface: ib,
                },
            ) => {
                let pa = PortId {
                    node: *na,
                    iface: *ia,
                };
                let pb = PortId {
                    node: *nb,
                    iface: *ib,
                };
                if switch.is_attached(pa) && switch.is_attached(pb) {
                    let _ = switch.connect_p2p(link.id, pa, pb, imp);
                }
            }
            (Endpoint::Node { node, iface }, Endpoint::Network { network })
            | (Endpoint::Network { network }, Endpoint::Node { node, iface }) => {
                let p = PortId {
                    node: *node,
                    iface: *iface,
                };
                if switch.is_attached(p) && lab.networks.contains_key(network) {
                    switch.join_segment(*network, p, Some((link.id, imp)));
                }
            }
            _ => {}
        }
    }

    /// Apply a single link live (hot connect) if endpoints are up.
    pub async fn hot_wire_link(&self, lab_id: Uuid, link_id: Uuid) -> ApiResult<()> {
        let lab = self.store.load(lab_id)?;
        let link = lab.link(link_id)?.clone();
        let switch = self.switch_for(lab_id).await;
        self.apply_link(&switch, &lab, &link);
        Ok(())
    }

    pub async fn unwire_link(&self, lab_id: Uuid, link_id: Uuid) {
        let switch = self.switch_for(lab_id).await;
        switch.disconnect_link(link_id);
    }

    pub async fn stop_node(&self, lab_id: Uuid, node_id: Uuid) -> ApiResult<()> {
        self.supervisor.stop(lab_id, node_id).await?;
        let switch = self.switch_for(lab_id).await;
        // Detach ports so a later start re-attaches fresh.
        let lab = self.store.load(lab_id)?;
        if let Ok(node) = lab.node(node_id) {
            for i in 0..node.interfaces {
                switch.detach(PortId {
                    node: node_id,
                    iface: i,
                });
            }
        }
        Ok(())
    }

    /// Wipe: stop if needed and delete the overlay + config media.
    pub async fn wipe_node(&self, lab_id: Uuid, node_id: Uuid) -> ApiResult<()> {
        self.stop_node(lab_id, node_id).await?;
        let node_dir = self.store.node_dir(lab_id, node_id);
        netpilot_qemu::wipe_overlay(&overlay_path(&node_dir))?;
        for media in ["config.iso", "config.img", "seed.iso"] {
            let _ = std::fs::remove_file(node_dir.join(media));
        }
        self.events.publish(Event::NodeState {
            lab: lab_id,
            node: node_id,
            state: NodeState::Stopped,
            detail: Some("wiped".into()),
        });
        Ok(())
    }

    /// Start all nodes of a lab honoring per-node boot delays.
    pub async fn start_lab(&self, lab_id: Uuid) -> ApiResult<()> {
        let lab = self.store.load(lab_id)?;
        let mut nodes: Vec<&Node> = lab.nodes.values().collect();
        nodes.sort_by_key(|n| n.boot_delay_s);
        for node in nodes {
            if node.boot_delay_s > 0 {
                tokio::time::sleep(std::time::Duration::from_secs(node.boot_delay_s as u64)).await;
            }
            if let Err(e) = self.start_node(lab_id, node.id).await {
                self.events
                    .log(Some(lab_id), "error", format!("start {}: {e}", node.name));
            }
        }
        Ok(())
    }

    pub async fn stop_lab(&self, lab_id: Uuid) -> ApiResult<()> {
        self.supervisor.stop_lab(lab_id).await?;
        self.switches.write().await.remove(&lab_id);
        Ok(())
    }
}

/// Stable VNC display number derived from the node id (5900+display).
fn vnc_display_for(node: Uuid) -> u16 {
    let b = node.as_bytes();
    100 + ((b[0] as u16) << 8 | b[1] as u16) % 5000
}
