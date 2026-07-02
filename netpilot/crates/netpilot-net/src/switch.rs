//! Userspace UDP frame switch — the default (rootless) datapath.
//!
//! Every QEMU NIC is started with a UDP socket netdev:
//!
//! ```text
//! -netdev socket,id=pN,udp=127.0.0.1:<switch_port>,localaddr=127.0.0.1:<qemu_port>
//! ```
//!
//! QEMU encapsulates each Ethernet frame 1:1 in a UDP datagram. The switch
//! binds `switch_port` for every attached NIC and forwards frames according
//! to the wiring table:
//!
//! * point-to-point link  → forward to the peer's NIC port
//! * multipoint network   → flood to every other member
//!
//! Because wiring is just a table, links can be added/removed while nodes
//! run ("hot connections"), impairment (delay/jitter/loss/rate) is applied
//! per link live, and any port can be tapped into a pcap file.

use std::collections::HashMap;
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU16, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde::Serialize;
use tokio::net::UdpSocket;
use uuid::Uuid;

use crate::runner::{NetError, Result};

const MAX_FRAME: usize = 65536;

/// A NIC attachment point on the switch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub struct PortId {
    pub node: Uuid,
    pub iface: u32,
}

/// Live-tunable impairment for a link.
#[derive(Debug, Clone, Copy, Default, Serialize)]
pub struct WireImpairment {
    pub delay_ms: u32,
    pub jitter_ms: u32,
    pub loss_pct: f32,
    pub rate_kbit: u32,
}

impl WireImpairment {
    pub fn is_noop(&self) -> bool {
        self.delay_ms == 0 && self.jitter_ms == 0 && self.loss_pct == 0.0 && self.rate_kbit == 0
    }
}

/// Token bucket used to police link rate.
#[derive(Debug)]
struct Bucket {
    tokens: f64,
    last: Instant,
}

#[derive(Debug)]
struct LinkState {
    impairment: WireImpairment,
    /// Suspended links drop every frame (admin-down without touching
    /// guest interface state) — EVE-NG Pro "suspend link".
    suspended: AtomicBool,
    bucket: Mutex<Bucket>,
}

impl LinkState {
    fn new(impairment: WireImpairment) -> Self {
        Self {
            impairment,
            suspended: AtomicBool::new(false),
            bucket: Mutex::new(Bucket {
                tokens: 0.0,
                last: Instant::now(),
            }),
        }
    }

    /// Returns false if the frame exceeds the policed rate and must drop.
    fn admit(&self, len: usize) -> bool {
        let rate = self.impairment.rate_kbit;
        if rate == 0 {
            return true;
        }
        let bytes_per_sec = rate as f64 * 1000.0 / 8.0;
        let burst = (bytes_per_sec / 10.0).max(16384.0); // 100ms of burst, min 16 KiB
        let mut b = self.bucket.lock().unwrap();
        let now = Instant::now();
        b.tokens = (b.tokens + now.duration_since(b.last).as_secs_f64() * bytes_per_sec).min(burst);
        b.last = now;
        if b.tokens >= len as f64 {
            b.tokens -= len as f64;
            true
        } else {
            false
        }
    }
}

/// Where frames arriving on a port go.
#[derive(Debug, Clone)]
enum Wiring {
    /// Forward to a single peer port (p2p link).
    Peer { port: PortId, link: Uuid },
    /// Flood to all other members of a segment; optional per-attachment link
    /// id so node→network links can carry impairment too.
    Segment { network: Uuid, link: Option<Uuid> },
}

struct PortState {
    /// Socket the switch bound for this NIC (QEMU sends here; we send from here).
    socket: Arc<UdpSocket>,
    /// Where QEMU listens for frames destined to this NIC.
    qemu_addr: SocketAddr,
    /// What QEMU was told: ports for its netdev args.
    wiring_info: NicWiring,
    /// Carrier state: when false frames are dropped both ways.
    up: Arc<AtomicBool>,
    /// pcap sink, if capturing.
    capture: Arc<Mutex<Option<PcapWriter>>>,
    rx_frames: Arc<AtomicU64>,
    tx_frames: Arc<AtomicU64>,
    rx_task: tokio::task::JoinHandle<()>,
}

impl Drop for PortState {
    fn drop(&mut self) {
        self.rx_task.abort();
    }
}

#[derive(Default)]
struct Tables {
    wiring: HashMap<PortId, Wiring>,
    segments: HashMap<Uuid, Vec<PortId>>,
    links: HashMap<Uuid, Arc<LinkState>>,
}

struct SwitchInner {
    ports: RwLock<HashMap<PortId, Arc<PortState>>>,
    tables: RwLock<Tables>,
    next_port: AtomicU16,
}

/// Address pair for one NIC: what to pass to QEMU.
#[derive(Debug, Clone, Copy, Serialize)]
pub struct NicWiring {
    /// The switch's port for this NIC (QEMU's `udp=` remote).
    pub switch_port: u16,
    /// QEMU's local port (`localaddr=`), where the switch sends frames.
    pub qemu_port: u16,
}

/// Port statistics snapshot for APIs.
#[derive(Debug, Clone, Serialize)]
pub struct PortStats {
    pub node: Uuid,
    pub iface: u32,
    pub up: bool,
    pub rx_frames: u64,
    pub tx_frames: u64,
    pub capturing: bool,
}

/// Per-lab userspace switch.
#[derive(Clone)]
pub struct UdpSwitch {
    inner: Arc<SwitchInner>,
}

impl UdpSwitch {
    /// `port_base` is the start of the UDP port range used for NIC pairs.
    pub fn new(port_base: u16) -> Self {
        Self {
            inner: Arc::new(SwitchInner {
                ports: RwLock::new(HashMap::new()),
                tables: RwLock::new(Tables::default()),
                next_port: AtomicU16::new(port_base),
            }),
        }
    }

    /// Attach a NIC: bind a UDP socket pair and start its receive loop.
    /// Returns the address pair to hand to QEMU. Idempotent per port.
    pub async fn attach(&self, port: PortId) -> Result<NicWiring> {
        if let Some(existing) = self.inner.ports.read().unwrap().get(&port) {
            return Ok(existing.wiring_info);
        }

        // Find a free consecutive pair: switch side (even) and qemu side.
        let (socket, switch_port, qemu_port) = loop {
            let base = self.inner.next_port.fetch_add(2, Ordering::SeqCst);
            if base > u16::MAX - 2 {
                return Err(NetError::Other("switch port range exhausted".into()));
            }
            match UdpSocket::bind(SocketAddrV4::new(Ipv4Addr::LOCALHOST, base)).await {
                Ok(s) => break (s, base, base + 1),
                Err(_) => continue,
            }
        };

        let socket = Arc::new(socket);
        let qemu_addr = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, qemu_port));
        let up = Arc::new(AtomicBool::new(true));
        let capture: Arc<Mutex<Option<PcapWriter>>> = Arc::new(Mutex::new(None));
        let rx_frames = Arc::new(AtomicU64::new(0));
        let tx_frames = Arc::new(AtomicU64::new(0));
        let wiring_info = NicWiring {
            switch_port,
            qemu_port,
        };

        let rx_task = tokio::spawn(rx_loop(
            port,
            socket.clone(),
            Arc::downgrade(&self.inner),
            up.clone(),
            capture.clone(),
            rx_frames.clone(),
        ));

        let state = Arc::new(PortState {
            socket,
            qemu_addr,
            wiring_info,
            up,
            capture,
            rx_frames,
            tx_frames,
            rx_task,
        });
        self.inner.ports.write().unwrap().insert(port, state);
        Ok(wiring_info)
    }

    /// Detach a NIC (e.g. node stopped); its wiring entries are removed.
    pub fn detach(&self, port: PortId) {
        self.inner.ports.write().unwrap().remove(&port);
        let mut t = self.inner.tables.write().unwrap();
        t.wiring.remove(&port);
        for members in t.segments.values_mut() {
            members.retain(|p| *p != port);
        }
    }

    /// Wire a point-to-point link between two attached NICs.
    pub fn connect_p2p(
        &self,
        link: Uuid,
        a: PortId,
        b: PortId,
        impairment: WireImpairment,
    ) -> Result<()> {
        let mut t = self.inner.tables.write().unwrap();
        t.links.insert(link, Arc::new(LinkState::new(impairment)));
        t.wiring.insert(a, Wiring::Peer { port: b, link });
        t.wiring.insert(b, Wiring::Peer { port: a, link });
        Ok(())
    }

    /// Join a NIC to a multipoint segment (bridge/hub network).
    pub fn join_segment(&self, network: Uuid, port: PortId, link: Option<(Uuid, WireImpairment)>) {
        let mut t = self.inner.tables.write().unwrap();
        let link_id = link.map(|(id, imp)| {
            t.links.insert(id, Arc::new(LinkState::new(imp)));
            id
        });
        t.wiring.insert(
            port,
            Wiring::Segment {
                network,
                link: link_id,
            },
        );
        let members = t.segments.entry(network).or_default();
        if !members.contains(&port) {
            members.push(port);
        }
    }

    /// Remove a link (p2p) by id: clears wiring entries that reference it.
    pub fn disconnect_link(&self, link: Uuid) {
        let mut t = self.inner.tables.write().unwrap();
        t.links.remove(&link);
        let affected: Vec<PortId> = t
            .wiring
            .iter()
            .filter_map(|(p, w)| match w {
                Wiring::Peer { link: l, .. } if *l == link => Some(*p),
                Wiring::Segment { link: Some(l), .. } if *l == link => Some(*p),
                _ => None,
            })
            .collect();
        for p in affected {
            let removed = t.wiring.remove(&p);
            if let Some(Wiring::Segment { network, .. }) = removed {
                if let Some(members) = t.segments.get_mut(&network) {
                    members.retain(|m| *m != p);
                }
            }
        }
    }

    /// Update impairment on a live link (preserves suspension state).
    pub fn set_impairment(&self, link: Uuid, impairment: WireImpairment) {
        let mut t = self.inner.tables.write().unwrap();
        let suspended = t
            .links
            .get(&link)
            .map(|l| l.suspended.load(Ordering::Relaxed))
            .unwrap_or(false);
        let state = LinkState::new(impairment);
        state.suspended.store(suspended, Ordering::Relaxed);
        t.links.insert(link, Arc::new(state));
    }

    /// Suspend/resume a link: while suspended every frame is dropped.
    pub fn set_link_suspended(&self, link: Uuid, suspended: bool) {
        let t = self.inner.tables.read().unwrap();
        if let Some(l) = t.links.get(&link) {
            l.suspended.store(suspended, Ordering::Relaxed);
        }
    }

    /// Set carrier state on a port (drops frames both directions when down).
    pub fn set_carrier(&self, port: PortId, up: bool) -> Result<()> {
        let ports = self.inner.ports.read().unwrap();
        let p = ports
            .get(&port)
            .ok_or_else(|| NetError::Other(format!("port not attached: {port:?}")))?;
        p.up.store(up, Ordering::Relaxed);
        Ok(())
    }

    /// Start writing this port's traffic (both directions) to a pcap file.
    pub fn start_capture(&self, port: PortId, path: &Path) -> Result<()> {
        let ports = self.inner.ports.read().unwrap();
        let p = ports
            .get(&port)
            .ok_or_else(|| NetError::Other(format!("port not attached: {port:?}")))?;
        let writer = PcapWriter::create(path)?;
        *p.capture.lock().unwrap() = Some(writer);
        Ok(())
    }

    pub fn stop_capture(&self, port: PortId) -> Result<()> {
        let ports = self.inner.ports.read().unwrap();
        let p = ports
            .get(&port)
            .ok_or_else(|| NetError::Other(format!("port not attached: {port:?}")))?;
        *p.capture.lock().unwrap() = None;
        Ok(())
    }

    pub fn stats(&self) -> Vec<PortStats> {
        self.inner
            .ports
            .read()
            .unwrap()
            .iter()
            .map(|(id, p)| PortStats {
                node: id.node,
                iface: id.iface,
                up: p.up.load(Ordering::Relaxed),
                rx_frames: p.rx_frames.load(Ordering::Relaxed),
                tx_frames: p.tx_frames.load(Ordering::Relaxed),
                capturing: p.capture.lock().unwrap().is_some(),
            })
            .collect()
    }

    pub fn is_attached(&self, port: PortId) -> bool {
        self.inner.ports.read().unwrap().contains_key(&port)
    }
}

/// Receive loop for one port: read frames from QEMU, forward per wiring.
async fn rx_loop(
    port: PortId,
    socket: Arc<UdpSocket>,
    inner: std::sync::Weak<SwitchInner>,
    up: Arc<AtomicBool>,
    capture: Arc<Mutex<Option<PcapWriter>>>,
    rx_frames: Arc<AtomicU64>,
) {
    let mut buf = vec![0u8; MAX_FRAME];
    loop {
        let n = match socket.recv(&mut buf).await {
            Ok(n) => n,
            Err(e) => {
                tracing::debug!(?port, "switch rx socket error: {e}");
                return;
            }
        };
        let Some(inner) = inner.upgrade() else { return };
        if !up.load(Ordering::Relaxed) {
            continue;
        }
        rx_frames.fetch_add(1, Ordering::Relaxed);
        let frame = &buf[..n];
        if let Some(w) = capture.lock().unwrap().as_mut() {
            let _ = w.write_frame(frame);
        }

        // Resolve destinations under the read lock, then deliver outside it.
        let mut deliveries: Vec<(Arc<PortState>, Option<Arc<LinkState>>)> = Vec::new();
        {
            let tables = inner.tables.read().unwrap();
            let ports = inner.ports.read().unwrap();
            match tables.wiring.get(&port) {
                Some(Wiring::Peer { port: peer, link }) => {
                    if let Some(dst) = ports.get(peer) {
                        deliveries.push((dst.clone(), tables.links.get(link).cloned()));
                    }
                }
                Some(Wiring::Segment { network, link }) => {
                    let ls = link.as_ref().and_then(|l| tables.links.get(l).cloned());
                    if let Some(members) = tables.segments.get(network) {
                        for m in members {
                            if *m != port {
                                if let Some(dst) = ports.get(m) {
                                    deliveries.push((dst.clone(), ls.clone()));
                                }
                            }
                        }
                    }
                }
                None => {} // unwired port: drop
            }
        }

        for (dst, link_state) in deliveries {
            deliver(frame, dst, link_state).await;
        }
    }
}

/// Apply impairment and send one frame to a destination port.
async fn deliver(frame: &[u8], dst: Arc<PortState>, link: Option<Arc<LinkState>>) {
    if !dst.up.load(Ordering::Relaxed) {
        return;
    }

    let mut delay = Duration::ZERO;
    if let Some(ls) = &link {
        if ls.suspended.load(Ordering::Relaxed) {
            return;
        }
        let imp = ls.impairment;
        if !imp.is_noop() {
            if imp.loss_pct > 0.0 && rand::random::<f32>() * 100.0 < imp.loss_pct {
                return;
            }
            if !ls.admit(frame.len()) {
                return;
            }
            if imp.delay_ms > 0 || imp.jitter_ms > 0 {
                let jitter = if imp.jitter_ms > 0 {
                    rand::random::<f64>() * imp.jitter_ms as f64
                } else {
                    0.0
                };
                delay =
                    Duration::from_micros((imp.delay_ms as f64 * 1000.0 + jitter * 1000.0) as u64);
            }
        }
    }

    if delay.is_zero() {
        send_frame(&dst, frame).await;
    } else {
        let data = frame.to_vec();
        tokio::spawn(async move {
            tokio::time::sleep(delay).await;
            send_frame(&dst, &data).await;
        });
    }
}

async fn send_frame(dst: &PortState, frame: &[u8]) {
    if dst.socket.send_to(frame, dst.qemu_addr).await.is_ok() {
        dst.tx_frames.fetch_add(1, Ordering::Relaxed);
        if let Some(w) = dst.capture.lock().unwrap().as_mut() {
            let _ = w.write_frame(frame);
        }
    }
}

/// Minimal pcap (libpcap classic format) writer.
pub struct PcapWriter {
    file: std::fs::File,
}

impl PcapWriter {
    pub fn create(path: &Path) -> std::io::Result<Self> {
        use std::io::Write;
        let mut file = std::fs::File::create(path)?;
        // magic(LE), major=2, minor=4, thiszone=0, sigfigs=0, snaplen=65535, linktype=1(EN10MB)
        let mut hdr = Vec::with_capacity(24);
        hdr.extend_from_slice(&0xa1b2c3d4u32.to_le_bytes());
        hdr.extend_from_slice(&2u16.to_le_bytes());
        hdr.extend_from_slice(&4u16.to_le_bytes());
        hdr.extend_from_slice(&0i32.to_le_bytes());
        hdr.extend_from_slice(&0u32.to_le_bytes());
        hdr.extend_from_slice(&65535u32.to_le_bytes());
        hdr.extend_from_slice(&1u32.to_le_bytes());
        file.write_all(&hdr)?;
        Ok(Self { file })
    }

    pub fn write_frame(&mut self, frame: &[u8]) -> std::io::Result<()> {
        use std::io::Write;
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or(Duration::ZERO);
        let mut rec = Vec::with_capacity(16 + frame.len());
        rec.extend_from_slice(&(now.as_secs() as u32).to_le_bytes());
        rec.extend_from_slice(&now.subsec_micros().to_le_bytes());
        rec.extend_from_slice(&(frame.len() as u32).to_le_bytes());
        rec.extend_from_slice(&(frame.len() as u32).to_le_bytes());
        rec.extend_from_slice(frame);
        self.file.write_all(&rec)?;
        self.file.flush()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::net::UdpSocket as TokioUdp;

    /// Simulates a QEMU NIC endpoint: binds the qemu-side port.
    async fn fake_nic(wiring: NicWiring) -> TokioUdp {
        TokioUdp::bind(SocketAddrV4::new(Ipv4Addr::LOCALHOST, wiring.qemu_port))
            .await
            .expect("bind qemu-side port")
    }

    fn pid(iface: u32) -> PortId {
        PortId {
            node: Uuid::new_v4(),
            iface,
        }
    }

    #[tokio::test]
    async fn p2p_forwarding() {
        let sw = UdpSwitch::new(46000);
        let (pa, pb) = (pid(0), pid(0));
        let wa = sw.attach(pa).await.unwrap();
        let wb = sw.attach(pb).await.unwrap();
        let na = fake_nic(wa).await;
        let nb = fake_nic(wb).await;
        sw.connect_p2p(Uuid::new_v4(), pa, pb, WireImpairment::default())
            .unwrap();

        // A -> switch -> B
        na.send_to(b"hello-frame", (Ipv4Addr::LOCALHOST, wa.switch_port))
            .await
            .unwrap();
        let mut buf = [0u8; 64];
        let (n, _) = tokio::time::timeout(Duration::from_secs(2), nb.recv_from(&mut buf))
            .await
            .expect("frame should arrive")
            .unwrap();
        assert_eq!(&buf[..n], b"hello-frame");

        // B -> A
        nb.send_to(b"reply", (Ipv4Addr::LOCALHOST, wb.switch_port))
            .await
            .unwrap();
        let (n, _) = tokio::time::timeout(Duration::from_secs(2), na.recv_from(&mut buf))
            .await
            .expect("reply should arrive")
            .unwrap();
        assert_eq!(&buf[..n], b"reply");
    }

    #[tokio::test]
    async fn segment_flooding() {
        let sw = UdpSwitch::new(46200);
        let net = Uuid::new_v4();
        let ports = [pid(0), pid(0), pid(0)];
        let mut nics = Vec::new();
        for p in ports {
            let w = sw.attach(p).await.unwrap();
            nics.push((p, fake_nic(w).await, w));
            sw.join_segment(net, p, None);
        }

        // frame from nic0 floods to nic1 and nic2
        let (_, ref n0, w0) = nics[0];
        n0.send_to(b"bcast", (Ipv4Addr::LOCALHOST, w0.switch_port))
            .await
            .unwrap();
        let mut buf = [0u8; 64];
        for (_, nic, _) in &nics[1..] {
            let (n, _) = tokio::time::timeout(Duration::from_secs(2), nic.recv_from(&mut buf))
                .await
                .expect("flooded frame should arrive")
                .unwrap();
            assert_eq!(&buf[..n], b"bcast");
        }
    }

    #[tokio::test]
    async fn loss_and_carrier() {
        let sw = UdpSwitch::new(46400);
        let (pa, pb) = (pid(0), pid(0));
        let wa = sw.attach(pa).await.unwrap();
        let wb = sw.attach(pb).await.unwrap();
        let na = fake_nic(wa).await;
        let nb = fake_nic(wb).await;
        let link = Uuid::new_v4();
        sw.connect_p2p(
            link,
            pa,
            pb,
            WireImpairment {
                loss_pct: 100.0,
                ..Default::default()
            },
        )
        .unwrap();

        na.send_to(b"lost", (Ipv4Addr::LOCALHOST, wa.switch_port))
            .await
            .unwrap();
        let mut buf = [0u8; 64];
        assert!(
            tokio::time::timeout(Duration::from_millis(300), nb.recv_from(&mut buf))
                .await
                .is_err(),
            "100% loss must drop the frame"
        );

        // clear impairment, but take carrier down
        sw.set_impairment(link, WireImpairment::default());
        sw.set_carrier(pb, false).unwrap();
        na.send_to(b"down", (Ipv4Addr::LOCALHOST, wa.switch_port))
            .await
            .unwrap();
        assert!(
            tokio::time::timeout(Duration::from_millis(300), nb.recv_from(&mut buf))
                .await
                .is_err(),
            "carrier down must drop the frame"
        );

        sw.set_carrier(pb, true).unwrap();
        na.send_to(b"up-again", (Ipv4Addr::LOCALHOST, wa.switch_port))
            .await
            .unwrap();
        let (n, _) = tokio::time::timeout(Duration::from_secs(2), nb.recv_from(&mut buf))
            .await
            .expect("frame should arrive after carrier up")
            .unwrap();
        assert_eq!(&buf[..n], b"up-again");
    }

    #[tokio::test]
    async fn suspend_and_resume() {
        let sw = UdpSwitch::new(47000);
        let (pa, pb) = (pid(0), pid(0));
        let wa = sw.attach(pa).await.unwrap();
        let wb = sw.attach(pb).await.unwrap();
        let na = fake_nic(wa).await;
        let nb = fake_nic(wb).await;
        let link = Uuid::new_v4();
        sw.connect_p2p(link, pa, pb, WireImpairment::default())
            .unwrap();

        sw.set_link_suspended(link, true);
        na.send_to(b"dropped", (Ipv4Addr::LOCALHOST, wa.switch_port))
            .await
            .unwrap();
        let mut buf = [0u8; 64];
        assert!(
            tokio::time::timeout(Duration::from_millis(300), nb.recv_from(&mut buf))
                .await
                .is_err(),
            "suspended link must drop"
        );

        sw.set_link_suspended(link, false);
        na.send_to(b"resumed", (Ipv4Addr::LOCALHOST, wa.switch_port))
            .await
            .unwrap();
        let (n, _) = tokio::time::timeout(Duration::from_secs(2), nb.recv_from(&mut buf))
            .await
            .expect("resumed link must forward")
            .unwrap();
        assert_eq!(&buf[..n], b"resumed");

        // updating impairment must not clear suspension
        sw.set_link_suspended(link, true);
        sw.set_impairment(
            link,
            WireImpairment {
                delay_ms: 1,
                ..Default::default()
            },
        );
        na.send_to(b"still-down", (Ipv4Addr::LOCALHOST, wa.switch_port))
            .await
            .unwrap();
        assert!(
            tokio::time::timeout(Duration::from_millis(300), nb.recv_from(&mut buf))
                .await
                .is_err(),
            "suspension must survive impairment updates"
        );
    }

    #[tokio::test]
    async fn capture_writes_pcap() {
        let dir = tempfile::tempdir().unwrap();
        let pcap_path = dir.path().join("cap.pcap");
        let sw = UdpSwitch::new(46600);
        let (pa, pb) = (pid(0), pid(0));
        let wa = sw.attach(pa).await.unwrap();
        let wb = sw.attach(pb).await.unwrap();
        let na = fake_nic(wa).await;
        let nb = fake_nic(wb).await;
        sw.connect_p2p(Uuid::new_v4(), pa, pb, WireImpairment::default())
            .unwrap();
        sw.start_capture(pb, &pcap_path).unwrap();

        na.send_to(b"captured-frame", (Ipv4Addr::LOCALHOST, wa.switch_port))
            .await
            .unwrap();
        let mut buf = [0u8; 64];
        tokio::time::timeout(Duration::from_secs(2), nb.recv_from(&mut buf))
            .await
            .expect("frame should arrive")
            .unwrap();
        sw.stop_capture(pb).unwrap();

        let data = std::fs::read(&pcap_path).unwrap();
        assert!(data.len() >= 24 + 16 + 14);
        assert_eq!(&data[..4], &0xa1b2c3d4u32.to_le_bytes());
        // frame bytes present after global+record headers
        assert_eq!(&data[40..40 + 14], b"captured-frame");
    }

    #[tokio::test]
    async fn hot_rewire() {
        let sw = UdpSwitch::new(46800);
        let (pa, pb, pc) = (pid(0), pid(0), pid(0));
        let wa = sw.attach(pa).await.unwrap();
        let wb = sw.attach(pb).await.unwrap();
        let wc = sw.attach(pc).await.unwrap();
        let na = fake_nic(wa).await;
        let _nb = fake_nic(wb).await;
        let nc = fake_nic(wc).await;

        let l1 = Uuid::new_v4();
        sw.connect_p2p(l1, pa, pb, WireImpairment::default())
            .unwrap();
        // rewire A to C while "running"
        sw.disconnect_link(l1);
        sw.connect_p2p(Uuid::new_v4(), pa, pc, WireImpairment::default())
            .unwrap();

        na.send_to(b"to-c", (Ipv4Addr::LOCALHOST, wa.switch_port))
            .await
            .unwrap();
        let mut buf = [0u8; 64];
        let (n, _) = tokio::time::timeout(Duration::from_secs(2), nc.recv_from(&mut buf))
            .await
            .expect("frame should arrive at C")
            .unwrap();
        assert_eq!(&buf[..n], b"to-c");
    }
}
