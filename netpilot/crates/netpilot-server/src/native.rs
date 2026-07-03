//! Native node runtimes: network-namespace nodes (FRR from the host
//! package, bare Linux endpoints) and docker-container nodes (SR Linux,
//! BYOI cEOS/cRPD).
//!
//! Both share the datapath and console contracts of QEMU nodes:
//!
//! * every interface's host-side device is named `node_tap(lab,node,i)`,
//!   so the bridge datapath wires links identically for all node kinds
//!   (these kinds require `--datapath bridge`);
//! * a unix listener at `<socket_dir>/console.sock` provides the console —
//!   each connection gets a fresh shell/CLI inside the node, so the
//!   existing WebSocket bridge and `run_console_command` work unchanged.

use std::collections::HashMap;
use std::process::Stdio;
use std::sync::Arc;

use netpilot_core::{ContainerSpec, Event, EventBus, NetnsSpec, NodeState};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixListener;
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::error::{ApiError, ApiResult};

/// Short stable name for namespaces/containers.
pub fn short_id(node: Uuid) -> String {
    node.simple().to_string()[..12].to_string()
}

async fn run(cmd: &str, args: &[&str]) -> ApiResult<String> {
    let out = tokio::process::Command::new(cmd)
        .args(args)
        .output()
        .await
        .map_err(|e| ApiError::internal(format!("{cmd}: {e}")))?;
    if !out.status.success() {
        return Err(ApiError::internal(format!(
            "{cmd} {}: {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr)
        )));
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

async fn run_ok(cmd: &str, args: &[&str]) -> bool {
    tokio::process::Command::new(cmd)
        .args(args)
        .output()
        .await
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// What the console listener should exec per connection.
#[derive(Clone)]
enum ConsoleTarget {
    /// `ip netns exec <ns> bash [--rcfile <rc>]`
    Netns {
        ns: String,
        rcfile: Option<std::path::PathBuf>,
    },
    /// `docker exec -i <name> <cmd>`
    Docker { name: String, cmd: String },
}

struct NativeNode {
    kind: ConsoleTarget,
    /// veth host-side names to delete on stop.
    host_ifaces: Vec<String>,
    /// Console listener task.
    listener: tokio::task::JoinHandle<()>,
    /// Daemon pids (netns services) to kill on stop.
    pidfiles: Vec<std::path::PathBuf>,
    console_socket: std::path::PathBuf,
}

impl Drop for NativeNode {
    fn drop(&mut self) {
        self.listener.abort();
    }
}

/// Supervisor for netns/container nodes, mirroring the QEMU supervisor's
/// surface (start/stop/console_socket/is_running).
#[derive(Clone)]
pub struct NativeSupervisor {
    nodes: Arc<Mutex<HashMap<(Uuid, Uuid), NativeNode>>>,
    events: EventBus,
}

pub struct NativeBootSpec {
    pub lab_id: Uuid,
    pub node_id: Uuid,
    pub name: String,
    pub interfaces: u32,
    /// Inner interface names, index order (from the template pattern).
    pub iface_names: Vec<String>,
    pub macs: Vec<String>,
    pub startup_config: Option<String>,
    /// Optional shell script run inside the namespace before the service
    /// starts (extra devices: vxlan, bridges, vrf...). From
    /// `node.overrides["boot_script"]`.
    pub boot_script: Option<String>,
    pub socket_dir: std::path::PathBuf,
    pub run_dir: std::path::PathBuf,
}

impl NativeSupervisor {
    pub fn new(events: EventBus) -> Self {
        Self {
            nodes: Arc::new(Mutex::new(HashMap::new())),
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
        self.nodes.lock().await.contains_key(&(lab, node))
    }

    pub async fn running_nodes(&self, lab: Uuid) -> Vec<Uuid> {
        self.nodes
            .lock()
            .await
            .keys()
            .filter(|(l, _)| *l == lab)
            .map(|(_, n)| *n)
            .collect()
    }

    pub async fn console_socket(&self, lab: Uuid, node: Uuid) -> Option<std::path::PathBuf> {
        self.nodes
            .lock()
            .await
            .get(&(lab, node))
            .map(|n| n.console_socket.clone())
    }

    /// Start a netns-kind node.
    pub async fn start_netns(&self, spec: NativeBootSpec, service: &NetnsSpec) -> ApiResult<()> {
        let key = (spec.lab_id, spec.node_id);
        if self.nodes.lock().await.contains_key(&key) {
            return Ok(());
        }
        self.publish(spec.lab_id, spec.node_id, NodeState::Starting, None);

        let result = self.start_netns_inner(&spec, service).await;
        match result {
            Ok(node) => {
                self.nodes.lock().await.insert(key, node);
                self.publish(spec.lab_id, spec.node_id, NodeState::Running, None);
                Ok(())
            }
            Err(e) => {
                // Best-effort cleanup of partial state.
                let ns = format!("np-{}", short_id(spec.node_id));
                let _ = run("ip", &["netns", "del", &ns]).await;
                self.publish(
                    spec.lab_id,
                    spec.node_id,
                    NodeState::Error,
                    Some(e.message.clone()),
                );
                Err(e)
            }
        }
    }

    async fn start_netns_inner(
        &self,
        spec: &NativeBootSpec,
        service: &NetnsSpec,
    ) -> ApiResult<NativeNode> {
        let ns = format!("np-{}", short_id(spec.node_id));
        std::fs::create_dir_all(&spec.run_dir)?;
        std::fs::create_dir_all(&spec.socket_dir)?;

        // Stale state from a previous (possibly crashed) server: daemons
        // survive `ip netns del`, so kill anything still in the namespace
        // and anything holding our pidfiles before recreating.
        if let Ok(pids) = run("ip", &["netns", "pids", &ns]).await {
            for pid in pids.split_whitespace() {
                let _ = run("kill", &["-9", pid]).await;
            }
        }
        if let Ok(rd) = std::fs::read_dir(spec.socket_dir.join("frr")) {
            for entry in rd.flatten() {
                if entry.path().extension().is_some_and(|e| e == "pid") {
                    if let Ok(pid) = std::fs::read_to_string(entry.path()) {
                        let _ = run("kill", &["-9", pid.trim()]).await;
                    }
                    let _ = std::fs::remove_file(entry.path());
                }
            }
        }
        let _ = run("ip", &["netns", "del", &ns]).await;
        run("ip", &["netns", "add", &ns]).await?;
        run(
            "ip",
            &["netns", "exec", &ns, "ip", "link", "set", "lo", "up"],
        )
        .await?;
        // Allow forwarding + MPLS/VRF-ish sysctls where available.
        let _ = run(
            "ip",
            &[
                "netns",
                "exec",
                &ns,
                "sysctl",
                "-qw",
                "net.ipv4.ip_forward=1",
            ],
        )
        .await;
        let _ = run(
            "ip",
            &[
                "netns",
                "exec",
                &ns,
                "sysctl",
                "-qw",
                "net.ipv6.conf.all.forwarding=1",
            ],
        )
        .await;

        // veth pairs: host side named like a tap so bridge wiring matches.
        let mut host_ifaces = Vec::new();
        for i in 0..spec.interfaces {
            let host = netpilot_net::node_tap(spec.lab_id, spec.node_id, i);
            let peer = format!("npv-{}", &host[4..]); // temp peer name
            let _ = run("ip", &["link", "del", &host]).await;
            run(
                "ip",
                &["link", "add", &host, "type", "veth", "peer", "name", &peer],
            )
            .await?;
            run("ip", &["link", "set", &host, "up"]).await?;
            run("ip", &["link", "set", &peer, "netns", &ns]).await?;
            let inner = &spec.iface_names[i as usize];
            run(
                "ip",
                &[
                    "netns", "exec", &ns, "ip", "link", "set", &peer, "name", inner,
                ],
            )
            .await?;
            if let Some(mac) = spec.macs.get(i as usize) {
                let _ = run(
                    "ip",
                    &[
                        "netns", "exec", &ns, "ip", "link", "set", inner, "address", mac,
                    ],
                )
                .await;
            }
            run(
                "ip",
                &["netns", "exec", &ns, "ip", "link", "set", inner, "up"],
            )
            .await?;
            host_ifaces.push(host);
        }

        // Pre-service boot script (extra interfaces: vxlan, bridges, vrf…).
        if let Some(script) = &spec.boot_script {
            let path = spec.socket_dir.join("pre-boot.sh");
            std::fs::write(&path, script)?;
            let out = tokio::process::Command::new("ip")
                .args(["netns", "exec", &ns, "sh", path.to_str().unwrap()])
                .output()
                .await?;
            if !out.status.success() {
                return Err(ApiError::bad_request(format!(
                    "boot_script failed: {}",
                    String::from_utf8_lossy(&out.stderr)
                )));
            }
        }

        // Service startup.
        let mut pidfiles = Vec::new();
        let mut rcfile = None;
        match service.service.as_str() {
            "frr" => {
                // FRR runtime state lives under the (world-traversable)
                // socket dir — the packaged frr user must reach it.
                let cfg_dir = spec.socket_dir.join("frr");
                pidfiles = start_frr(&ns, spec, &cfg_dir).await?;
                // Console convenience: plain `vtysh` finds this node's
                // vty sockets via an alias in the shell rcfile.
                let rc = spec.socket_dir.join("bashrc");
                std::fs::write(
                    &rc,
                    format!(
                        "alias vtysh='vtysh --vty_socket {}'\nPS1='{}:\\w# '\n\
                         echo 'NetPilot FRR node {} — type vtysh for the router CLI'\n",
                        cfg_dir.display(),
                        spec.name,
                        spec.name
                    ),
                )?;
                rcfile = Some(rc);
            }
            "" => {
                // Bare endpoint: run the startup config as a boot script.
                if let Some(cfg) = &spec.startup_config {
                    let script = spec.run_dir.join("boot.sh");
                    std::fs::write(&script, cfg)?;
                    let out = tokio::process::Command::new("ip")
                        .args([
                            "netns",
                            "exec",
                            &ns,
                            "sh",
                            script.to_str().unwrap_or("boot.sh"),
                        ])
                        .output()
                        .await?;
                    if !out.status.success() {
                        self.events.log(
                            Some(spec.lab_id),
                            "warn",
                            format!(
                                "{}: boot script: {}",
                                spec.name,
                                String::from_utf8_lossy(&out.stderr)
                            ),
                        );
                    }
                }
            }
            other => {
                return Err(ApiError::bad_request(format!(
                    "netns service '{other}' is not supported (yet)"
                )));
            }
        }

        // Console listener.
        let console_socket = spec.socket_dir.join("console.sock");
        let target = ConsoleTarget::Netns {
            ns: ns.clone(),
            rcfile,
        };
        let listener = spawn_console_listener(&console_socket, target.clone())?;

        Ok(NativeNode {
            kind: target,
            host_ifaces,
            listener,
            pidfiles,
            console_socket,
        })
    }

    /// Start a container-kind node via docker.
    pub async fn start_container(
        &self,
        spec: NativeBootSpec,
        container: &ContainerSpec,
    ) -> ApiResult<()> {
        let key = (spec.lab_id, spec.node_id);
        if self.nodes.lock().await.contains_key(&key) {
            return Ok(());
        }
        self.publish(spec.lab_id, spec.node_id, NodeState::Starting, None);
        match self.start_container_inner(&spec, container).await {
            Ok(node) => {
                self.nodes.lock().await.insert(key, node);
                self.publish(spec.lab_id, spec.node_id, NodeState::Running, None);
                Ok(())
            }
            Err(e) => {
                let cname = format!("np-{}", short_id(spec.node_id));
                let _ = run("docker", &["rm", "-f", &cname]).await;
                self.publish(
                    spec.lab_id,
                    spec.node_id,
                    NodeState::Error,
                    Some(e.message.clone()),
                );
                Err(e)
            }
        }
    }

    async fn start_container_inner(
        &self,
        spec: &NativeBootSpec,
        container: &ContainerSpec,
    ) -> ApiResult<NativeNode> {
        std::fs::create_dir_all(&spec.run_dir)?;
        std::fs::create_dir_all(&spec.socket_dir)?;

        if !run_ok("docker", &["info"]).await {
            return Err(ApiError::bad_request(
                "docker daemon is not available — container nodes need docker on the lab host",
            ));
        }

        // Image present? Try pulling if not (works for public registries).
        let have = run("docker", &["images", "-q", &container.image])
            .await
            .unwrap_or_default();
        if have.trim().is_empty() {
            self.events.log(
                Some(spec.lab_id),
                "info",
                format!("pulling {} (first start)…", container.image),
            );
            if !run_ok("docker", &["pull", &container.image]).await {
                return Err(ApiError::bad_request(format!(
                    "image '{}' is not available locally and could not be pulled — \
                     upload it via the Images page (docker tarball)",
                    container.image
                )));
            }
        }

        let cname = format!("np-{}", short_id(spec.node_id));
        let _ = run("docker", &["rm", "-f", &cname]).await;

        let mut args: Vec<String> = vec![
            "create".into(),
            "--name".into(),
            cname.clone(),
            "--network".into(),
            "none".into(),
            "--hostname".into(),
            spec.name.clone(),
        ];
        if container.privileged {
            args.push("--privileged".into());
        }
        for e in &container.env {
            args.push("-e".into());
            args.push(e.clone());
        }
        args.push(container.image.clone());
        args.extend(container.cmd.iter().cloned());
        let argrefs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
        run("docker", &argrefs).await?;
        run("docker", &["start", &cname]).await?;

        // Wire veths into the container's netns (by pid).
        let pid = run("docker", &["inspect", "-f", "{{.State.Pid}}", &cname])
            .await?
            .trim()
            .to_string();
        let mut host_ifaces = Vec::new();
        for i in 0..spec.interfaces {
            let host = netpilot_net::node_tap(spec.lab_id, spec.node_id, i);
            let peer = format!("npv-{}", &host[4..]);
            let _ = run("ip", &["link", "del", &host]).await;
            run(
                "ip",
                &["link", "add", &host, "type", "veth", "peer", "name", &peer],
            )
            .await?;
            run("ip", &["link", "set", &host, "up"]).await?;
            run("ip", &["link", "set", &peer, "netns", &pid]).await?;
            let inner = &spec.iface_names[i as usize];
            run(
                "nsenter",
                &["-t", &pid, "-n", "ip", "link", "set", &peer, "name", inner],
            )
            .await?;
            if let Some(mac) = spec.macs.get(i as usize) {
                let _ = run(
                    "nsenter",
                    &["-t", &pid, "-n", "ip", "link", "set", inner, "address", mac],
                )
                .await;
            }
            run(
                "nsenter",
                &["-t", &pid, "-n", "ip", "link", "set", inner, "up"],
            )
            .await?;
            host_ifaces.push(host);
        }

        let console_socket = spec.socket_dir.join("console.sock");
        let target = ConsoleTarget::Docker {
            name: cname,
            cmd: container.console_cmd.clone().unwrap_or_else(|| "sh".into()),
        };
        let listener = spawn_console_listener(&console_socket, target.clone())?;

        Ok(NativeNode {
            kind: target,
            host_ifaces,
            listener,
            pidfiles: Vec::new(),
            console_socket,
        })
    }

    pub async fn stop(&self, lab: Uuid, node: Uuid) -> ApiResult<()> {
        let Some(entry) = self.nodes.lock().await.remove(&(lab, node)) else {
            return Ok(());
        };
        self.publish(lab, node, NodeState::Stopping, None);
        entry.listener.abort();

        for pidfile in &entry.pidfiles {
            if let Ok(pid) = std::fs::read_to_string(pidfile) {
                let _ = run("kill", &[pid.trim()]).await;
            }
        }
        match &entry.kind {
            ConsoleTarget::Netns { ns, .. } => {
                // Daemons survive netns deletion — kill them explicitly.
                if let Ok(pids) = run("ip", &["netns", "pids", ns]).await {
                    for pid in pids.split_whitespace() {
                        let _ = run("kill", &["-9", pid]).await;
                    }
                }
                let _ = run("ip", &["netns", "del", ns]).await;
            }
            ConsoleTarget::Docker { name, .. } => {
                let _ = run("docker", &["rm", "-f", name]).await;
            }
        }
        for iface in &entry.host_ifaces {
            let _ = run("ip", &["link", "del", iface]).await;
        }
        let _ = std::fs::remove_file(&entry.console_socket);
        self.publish(lab, node, NodeState::Stopped, None);
        Ok(())
    }
}

/// Write the FRR config and start the daemon suite inside the namespace.
/// Daemons are chosen from the config content; zebra + staticd always run.
async fn start_frr(
    ns: &str,
    spec: &NativeBootSpec,
    cfg_dir: &std::path::Path,
) -> ApiResult<Vec<std::path::PathBuf>> {
    if !std::path::Path::new("/usr/lib/frr/zebra").exists() {
        return Err(ApiError::bad_request(
            "FRR is not installed on the lab host (apt install frr)",
        ));
    }
    std::fs::create_dir_all(cfg_dir)?;
    let config = spec
        .startup_config
        .clone()
        .unwrap_or_else(|| format!("hostname {}\n", spec.name));
    let conf_path = cfg_dir.join("frr.conf");
    std::fs::write(&conf_path, &config)?;
    // vtysh needs vtysh.conf & to find sockets in our runtime dir.
    std::fs::write(
        cfg_dir.join("vtysh.conf"),
        "service integrated-vtysh-config\n",
    )?;
    // Daemons drop privileges to the packaged frr user — it must own the
    // runtime dir (pidfiles, vty sockets).
    let _ = tokio::process::Command::new("chown")
        .args(["-R", "frr:frr", cfg_dir.to_str().unwrap()])
        .output()
        .await;

    let lc = config.to_lowercase();
    let mut daemons = vec!["zebra", "staticd", "mgmtd"];
    for (needle, daemon) in [
        ("router ospf", "ospfd"),
        ("router bgp", "bgpd"),
        ("mpls ldp", "ldpd"),
        ("router isis", "isisd"),
        ("router rip", "ripd"),
        ("router pim", "pimd"),
    ] {
        if lc.contains(needle) {
            daemons.push(daemon);
        }
    }
    // EVPN lives in bgpd; "address-family l2vpn evpn" already matches bgp.

    let mut pidfiles = Vec::new();
    for daemon in daemons {
        let bin = format!("/usr/lib/frr/{daemon}");
        if !std::path::Path::new(&bin).exists() {
            continue; // mgmtd doesn't exist on FRR 8.x — skip quietly
        }
        let pidfile = cfg_dir.join(format!("{daemon}.pid"));
        let out = tokio::process::Command::new("ip")
            .args([
                "netns",
                "exec",
                ns,
                &bin,
                "-d",
                "-f",
                conf_path.to_str().unwrap(),
                "--pid_file",
                pidfile.to_str().unwrap(),
                "--vty_socket",
                cfg_dir.to_str().unwrap(),
                "-i",
                pidfile.to_str().unwrap(),
            ])
            .output()
            .await?;
        if !out.status.success() {
            return Err(ApiError::internal(format!(
                "{daemon}: {}",
                String::from_utf8_lossy(&out.stderr)
            )));
        }
        pidfiles.push(pidfile);
    }
    Ok(pidfiles)
}

/// Accept console connections and pipe each to a fresh shell/CLI process.
fn spawn_console_listener(
    socket: &std::path::Path,
    target: ConsoleTarget,
) -> ApiResult<tokio::task::JoinHandle<()>> {
    let _ = std::fs::remove_file(socket);
    let listener = UnixListener::bind(socket)
        .map_err(|e| ApiError::internal(format!("console listener: {e}")))?;

    Ok(tokio::spawn(async move {
        loop {
            let Ok((stream, _)) = listener.accept().await else {
                return;
            };
            let target = target.clone();
            tokio::spawn(async move {
                let mut cmd = match &target {
                    ConsoleTarget::Netns { ns, rcfile } => {
                        let mut c = tokio::process::Command::new("ip");
                        c.args(["netns", "exec", ns, "bash", "--noprofile"]);
                        match rcfile {
                            Some(rc) => {
                                c.arg("--rcfile");
                                c.arg(rc);
                            }
                            None => {
                                c.arg("--norc");
                            }
                        }
                        c.arg("-i");
                        c
                    }
                    ConsoleTarget::Docker { name, cmd } => {
                        let mut c = tokio::process::Command::new("docker");
                        c.args(["exec", "-i", name, cmd]);
                        c
                    }
                };
                let Ok(mut child) = cmd
                    .stdin(Stdio::piped())
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped())
                    .kill_on_drop(true)
                    .spawn()
                else {
                    return;
                };

                let mut stdin = child.stdin.take().unwrap();
                let mut stdout = child.stdout.take().unwrap();
                let mut stderr = child.stderr.take().unwrap();
                let (mut sock_r, sock_w) = stream.into_split();
                let sock_w = Arc::new(Mutex::new(sock_w));

                let w1 = sock_w.clone();
                let t_out = tokio::spawn(async move {
                    let mut buf = [0u8; 4096];
                    loop {
                        match stdout.read(&mut buf).await {
                            Ok(0) | Err(_) => break,
                            Ok(n) => {
                                if w1.lock().await.write_all(&buf[..n]).await.is_err() {
                                    break;
                                }
                            }
                        }
                    }
                });
                let w2 = sock_w.clone();
                let t_err = tokio::spawn(async move {
                    let mut buf = [0u8; 4096];
                    loop {
                        match stderr.read(&mut buf).await {
                            Ok(0) | Err(_) => break,
                            Ok(n) => {
                                if w2.lock().await.write_all(&buf[..n]).await.is_err() {
                                    break;
                                }
                            }
                        }
                    }
                });

                let mut buf = [0u8; 4096];
                loop {
                    match sock_r.read(&mut buf).await {
                        Ok(0) | Err(_) => break,
                        Ok(n) => {
                            // Normalize CR (xterm sends \r) to newline.
                            let data: Vec<u8> = buf[..n]
                                .iter()
                                .map(|&b| if b == b'\r' { b'\n' } else { b })
                                .collect();
                            if stdin.write_all(&data).await.is_err() {
                                break;
                            }
                        }
                    }
                }
                let _ = child.kill().await;
                t_out.abort();
                t_err.abort();
            });
        }
    }))
}
