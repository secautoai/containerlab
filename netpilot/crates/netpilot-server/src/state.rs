//! Shared application state and lab orchestration.
//!
//! `AppState` ties the crates together: the lab store and template catalog
//! (netpilot-core), the per-lab UDP switches (netpilot-net), and the QEMU
//! node supervisor (netpilot-qemu). All REST/WS handlers operate through it.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use netpilot_core::{
    iface_name_from_pattern, ConsoleKind, CoreError, Endpoint, Event, EventBus, ImageLibrary, Lab,
    LabStore, Link, Node, NodeKind, NodeState, TemplateCatalog,
};
use netpilot_net::{
    link_bridge, network_bridge, node_tap, Plumbing, PortId, SystemRunner, UdpSwitch,
    WireImpairment,
};
use netpilot_qemu::{
    build_config_media, kvm_available, overlay_path, NicBackend, NodeBootSpec, NodeSupervisor,
};
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::error::{ApiError, ApiResult};

/// How lab links are realized on this host.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DatapathMode {
    /// Rootless userspace UDP switch (default).
    UdpSwitch,
    /// Linux taps + bridges; needs CAP_NET_ADMIN. Enables NAT/cloud
    /// networks and kernel-speed forwarding; impairment via tc netem.
    Bridge,
}

#[derive(Clone)]
pub struct AppState {
    pub store: Arc<LabStore>,
    pub images: Arc<ImageLibrary>,
    pub events: EventBus,
    pub supervisor: NodeSupervisor,
    /// Runtime for netns/container node kinds.
    pub native: crate::native::NativeSupervisor,
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
    /// Datapath implementation.
    pub datapath: DatapathMode,
    /// Linux plumbing (bridge mode).
    plumbing: Arc<Plumbing>,
}

impl AppState {
    /// Rootless default constructor (used by integration tests; the binary
    /// goes through [`Self::with_datapath`]).
    #[allow(dead_code)]
    pub fn new(data_dir: PathBuf, port_base: u16) -> anyhow::Result<Self> {
        Self::with_datapath(data_dir, port_base, DatapathMode::UdpSwitch)
    }

    pub fn with_datapath(
        data_dir: PathBuf,
        port_base: u16,
        datapath: DatapathMode,
    ) -> anyhow::Result<Self> {
        let store = LabStore::new(&data_dir)?;
        let images = ImageLibrary::new(store.images_dir());
        let events = EventBus::new();
        let supervisor = NodeSupervisor::new(events.clone());
        let templates = TemplateCatalog::load(Some(&store.templates_dir()))?;

        let state = Self {
            store: Arc::new(store),
            images: Arc::new(images),
            events: events.clone(),
            native: crate::native::NativeSupervisor::new(events.clone()),
            supervisor,
            templates: Arc::new(RwLock::new(templates)),
            switches: Arc::new(RwLock::new(HashMap::new())),
            node_states: Arc::new(RwLock::new(HashMap::new())),
            port_base,
            lab_write_lock: Arc::new(tokio::sync::Mutex::new(())),
            datapath,
            plumbing: Arc::new(Plumbing::new(Arc::new(SystemRunner))),
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
        if lab.locked {
            return Err(ApiError::conflict(
                "lab is locked — unlock it before editing",
            ));
        }
        let out = f(&mut lab)?;
        lab.touch();
        self.store.save(&lab)?;
        self.events.publish(Event::LabUpdated { lab: lab_id });
        Ok(out)
    }

    /// Lock/unlock a lab (bypasses the locked check by design).
    pub async fn set_locked(&self, lab_id: Uuid, locked: bool) -> ApiResult<Lab> {
        let _guard = self.lab_write_lock.lock().await;
        let mut lab = self.store.load(lab_id)?;
        lab.locked = locked;
        lab.touch();
        self.store.save(&lab)?;
        self.events.publish(Event::LabUpdated { lab: lab_id });
        Ok(lab)
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

        // Netns/container nodes take the native path (bridge datapath only).
        if template.kind != NodeKind::Qemu {
            if self.datapath != DatapathMode::Bridge {
                return Err(ApiError::bad_request(format!(
                    "'{}' nodes need the bridge datapath — start the server with --datapath bridge",
                    template.id
                )));
            }
            let spec = crate::native::NativeBootSpec {
                lab_id,
                node_id,
                name: node.name.clone(),
                interfaces: node.interfaces,
                iface_names: (0..node.interfaces)
                    .map(|i| iface_name_from_pattern(&template.iface_pattern, i))
                    .collect(),
                macs: (0..node.interfaces)
                    .map(|i| netpilot_net::node_mac(lab_id, node_id, i))
                    .collect(),
                startup_config: node
                    .effective_config(&lab.active_config_set)
                    .map(|s| s.to_string()),
                boot_script: node.overrides.get("boot_script").cloned(),
                socket_dir: netpilot_qemu::socket_dir_for(node_id),
                run_dir: self.store.node_dir(lab_id, node_id),
            };
            match template.kind {
                NodeKind::Netns => {
                    let service = template.netns.clone().unwrap_or_default();
                    self.native.start_netns(spec, &service).await?;
                }
                NodeKind::Container => {
                    let container = template.container.clone().ok_or_else(|| {
                        ApiError::bad_request("container template missing container spec")
                    })?;
                    self.native.start_container(spec, &container).await?;
                }
                NodeKind::Qemu => unreachable!(),
            }
            self.rewire_lab(&lab).await?;
            return Ok(());
        }

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
            node.effective_config(&lab.active_config_set),
            &node_dir,
        )?;

        // Attach every interface to the datapath.
        let mut nics: Vec<NicBackend> = Vec::with_capacity(node.interfaces as usize);
        match self.datapath {
            DatapathMode::UdpSwitch => {
                let switch = self.switch_for(lab_id).await;
                for i in 0..node.interfaces {
                    let wiring = switch
                        .attach(PortId {
                            node: node_id,
                            iface: i,
                        })
                        .await
                        .map_err(|e| ApiError::internal(e.to_string()))?;
                    nics.push(wiring.into());
                }
            }
            DatapathMode::Bridge => {
                for i in 0..node.interfaces {
                    let tap = node_tap(lab_id, node_id, i);
                    self.plumbing
                        .ensure_tap(&tap, None)
                        .await
                        .map_err(|e| ApiError::internal(format!("tap {tap}: {e}")))?;
                    nics.push(NicBackend::Tap { ifname: tap });
                }
            }
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

    /// Recompute datapath wiring for all links whose endpoints are attached.
    pub async fn rewire_lab(&self, lab: &Lab) -> ApiResult<()> {
        match self.datapath {
            DatapathMode::UdpSwitch => {
                let switch = self.switch_for(lab.id).await;
                for link in lab.links.values() {
                    self.apply_link(&switch, lab, link);
                }
            }
            DatapathMode::Bridge => {
                for link in lab.links.values() {
                    if let Err(e) = self.apply_link_bridge(lab, link).await {
                        self.events
                            .log(Some(lab.id), "error", format!("bridge wiring: {e}"));
                    }
                }
            }
        }
        Ok(())
    }

    /// Bridge-mode wiring: a Linux bridge per link/network, endpoint taps
    /// enslaved to it, netem for impairment on the node-side taps.
    async fn apply_link_bridge(&self, lab: &Lab, link: &Link) -> ApiResult<()> {
        let tap_of = |node: &Uuid, iface: &u32| node_tap(lab.id, *node, *iface);
        let running = |node: &Uuid| {
            let qemu = &self.supervisor;
            let native = &self.native;
            let lab_id = lab.id;
            let node = *node;
            async move { qemu.is_running(lab_id, node).await || native.is_running(lab_id, node).await }
        };

        // Pick the bridge and member taps for this link.
        let (bridge, members): (String, Vec<String>) = match (&link.a, &link.b) {
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
                if !running(na).await || !running(nb).await {
                    return Ok(());
                }
                (
                    link_bridge(lab.id, link.id),
                    vec![tap_of(na, ia), tap_of(nb, ib)],
                )
            }
            (Endpoint::Node { node, iface }, Endpoint::Network { network })
            | (Endpoint::Network { network }, Endpoint::Node { node, iface }) => {
                if !running(node).await || !lab.networks.contains_key(network) {
                    return Ok(());
                }
                (network_bridge(lab.id, *network), vec![tap_of(node, iface)])
            }
            _ => return Ok(()),
        };

        let p = &self.plumbing;
        p.ensure_bridge(&bridge)
            .await
            .map_err(|e| ApiError::internal(e.to_string()))?;
        for tap in &members {
            p.enslave(tap, &bridge)
                .await
                .map_err(|e| ApiError::internal(format!("enslave {tap}: {e}")))?;
            let imp = link.impairment.unwrap_or_default();
            let _ = p
                .set_netem(
                    tap,
                    imp.delay_ms,
                    imp.jitter_ms,
                    imp.loss_pct,
                    imp.rate_kbit,
                )
                .await;
        }

        // NAT/management networks get a gateway + masquerade; cloud
        // networks get bridged to a host interface.
        let net_id = link.endpoints().iter().find_map(|e| match e {
            Endpoint::Network { network } => Some(*network),
            _ => None,
        });
        if let Some(net) = net_id.and_then(|id| lab.networks.get(&id)) {
            match net.kind {
                netpilot_core::NetworkKind::Nat | netpilot_core::NetworkKind::Management => {
                    let subnet = net.subnet.clone().unwrap_or_else(|| "10.99.0.0/24".into());
                    let gw = gateway_of(&subnet);
                    let _ = p.enable_nat(&bridge, &gw, &subnet).await;
                }
                netpilot_core::NetworkKind::Cloud => {
                    if let Some(host_if) = &net.host_interface {
                        let _ = p.attach_host_iface(host_if, &bridge).await;
                    }
                }
                netpilot_core::NetworkKind::Bridge => {}
            }
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
        switch.set_link_suspended(link.id, link.suspended);
    }

    /// Apply a single link live (hot connect) if endpoints are up.
    pub async fn hot_wire_link(&self, lab_id: Uuid, link_id: Uuid) -> ApiResult<()> {
        let lab = self.store.load(lab_id)?;
        let link = lab.link(link_id)?.clone();
        match self.datapath {
            DatapathMode::UdpSwitch => {
                let switch = self.switch_for(lab_id).await;
                self.apply_link(&switch, &lab, &link);
            }
            DatapathMode::Bridge => self.apply_link_bridge(&lab, &link).await?,
        }
        Ok(())
    }

    pub async fn unwire_link(&self, lab_id: Uuid, link_id: Uuid) {
        match self.datapath {
            DatapathMode::UdpSwitch => {
                let switch = self.switch_for(lab_id).await;
                switch.disconnect_link(link_id);
            }
            DatapathMode::Bridge => {
                // Removing the per-link bridge severs both taps.
                let _ = self
                    .plumbing
                    .delete_bridge(&link_bridge(lab_id, link_id))
                    .await;
            }
        }
    }

    /// Console socket for a running node of any kind.
    pub async fn console_socket(&self, lab: Uuid, node: Uuid) -> ApiResult<std::path::PathBuf> {
        if let Ok(p) = self.supervisor.console_socket(lab, node).await {
            return Ok(p);
        }
        self.native
            .console_socket(lab, node)
            .await
            .ok_or_else(|| ApiError::conflict("node is not running"))
    }

    pub async fn stop_node(&self, lab_id: Uuid, node_id: Uuid) -> ApiResult<()> {
        self.supervisor.stop(lab_id, node_id).await?;
        self.native.stop(lab_id, node_id).await?;
        // Detach datapath ports so a later start re-attaches fresh.
        let lab = self.store.load(lab_id)?;
        if let Ok(node) = lab.node(node_id) {
            match self.datapath {
                DatapathMode::UdpSwitch => {
                    let switch = self.switch_for(lab_id).await;
                    for i in 0..node.interfaces {
                        switch.detach(PortId {
                            node: node_id,
                            iface: i,
                        });
                    }
                }
                DatapathMode::Bridge => {
                    for i in 0..node.interfaces {
                        let _ = self
                            .plumbing
                            .delete_tap(&node_tap(lab_id, node_id, i))
                            .await;
                    }
                }
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
        for node in self.native.running_nodes(lab_id).await {
            self.native.stop(lab_id, node).await?;
        }
        self.switches.write().await.remove(&lab_id);
        if self.datapath == DatapathMode::Bridge {
            // Best-effort teardown of this lab's bridges and taps.
            if let Ok(lab) = self.store.load(lab_id) {
                for link in lab.links.keys() {
                    let _ = self
                        .plumbing
                        .delete_bridge(&link_bridge(lab_id, *link))
                        .await;
                }
                for net in lab.networks.keys() {
                    let _ = self
                        .plumbing
                        .delete_bridge(&network_bridge(lab_id, *net))
                        .await;
                }
                for node in lab.nodes.values() {
                    for i in 0..node.interfaces {
                        let _ = self
                            .plumbing
                            .delete_tap(&node_tap(lab_id, node.id, i))
                            .await;
                    }
                }
            }
        }
        Ok(())
    }
}

/// Stable VNC display number derived from the node id (5900+display).
fn vnc_display_for(node: Uuid) -> u16 {
    let b = node.as_bytes();
    100 + ((b[0] as u16) << 8 | b[1] as u16) % 5000
}

/// "10.99.0.0/24" → "10.99.0.1/24" (first host = gateway).
fn gateway_of(subnet: &str) -> String {
    if let Some((net, mask)) = subnet.split_once('/') {
        let mut parts: Vec<u8> = net.split('.').filter_map(|p| p.parse().ok()).collect();
        if parts.len() == 4 {
            parts[3] = parts[3].saturating_add(1);
            return format!("{}.{}.{}.{}/{mask}", parts[0], parts[1], parts[2], parts[3]);
        }
    }
    "10.99.0.1/24".into()
}
