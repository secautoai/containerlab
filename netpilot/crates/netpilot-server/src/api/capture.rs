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

/// One decoded packet row for the UI table.
#[derive(serde::Serialize)]
pub struct PacketSummary {
    pub ts: f64,
    pub len: u32,
    pub src: String,
    pub dst: String,
    pub proto: String,
    pub info: String,
}

/// Decode a captured pcap into a summary table (ethernet/ARP/IPv4/IPv6,
/// TCP/UDP/ICMP). Not Wireshark — just enough to see what's on the wire.
pub async fn summary(
    State(state): State<AppState>,
    Path((lab_id, node_id, iface)): Path<(Uuid, Uuid, u32)>,
) -> ApiResult<Json<Vec<PacketSummary>>> {
    let path = capture_path(&state, lab_id, node_id, iface);
    let data = std::fs::read(&path)
        .map_err(|_| ApiError::not_found("no capture file — start a capture first"))?;
    Ok(Json(decode_pcap(&data, 500)))
}

fn mac(b: &[u8]) -> String {
    b.iter()
        .map(|x| format!("{x:02x}"))
        .collect::<Vec<_>>()
        .join(":")
}

fn dns_type_name(t: u16) -> String {
    match t {
        1 => "A".into(),
        2 => "NS".into(),
        5 => "CNAME".into(),
        6 => "SOA".into(),
        12 => "PTR".into(),
        15 => "MX".into(),
        16 => "TXT".into(),
        28 => "AAAA".into(),
        43 => "DS".into(),
        46 => "RRSIG".into(),
        47 => "NSEC".into(),
        48 => "DNSKEY".into(),
        50 => "NSEC3".into(),
        252 => "AXFR".into(),
        255 => "ANY".into(),
        other => format!("type{other}"),
    }
}

/// Decode the header + first question of a DNS message (UDP port 53) into a
/// one-line info: "query www.example.lab. A" / "response … NXDOMAIN (0 ans)".
fn dns_info(msg: &[u8]) -> Option<String> {
    if msg.len() < 12 {
        return None;
    }
    let flags = u16::from_be_bytes([msg[2], msg[3]]);
    let qd = u16::from_be_bytes([msg[4], msg[5]]);
    let an = u16::from_be_bytes([msg[6], msg[7]]);
    let mut name = String::new();
    let mut off = 12usize;
    if qd > 0 {
        loop {
            let len = *msg.get(off)? as usize;
            if len == 0 {
                off += 1;
                break;
            }
            if len & 0xc0 == 0xc0 {
                off += 2;
                break;
            }
            for b in msg.get(off + 1..off + 1 + len)? {
                name.push(if b.is_ascii_graphic() { *b as char } else { '?' });
            }
            name.push('.');
            off += 1 + len;
            if name.len() > 96 {
                name.push('…');
                break;
            }
        }
    }
    if name.is_empty() {
        name = ".".into();
    }
    let qtype = if qd > 0 && off + 2 <= msg.len() {
        u16::from_be_bytes([msg[off], msg[off + 1]])
    } else {
        0
    };
    let qt = dns_type_name(qtype);
    if flags & 0x8000 == 0 {
        Some(format!("query {name} {qt}"))
    } else {
        let rcode = match flags & 0x000f {
            0 => "NOERROR".into(),
            1 => "FORMERR".into(),
            2 => "SERVFAIL".into(),
            3 => "NXDOMAIN".into(),
            5 => "REFUSED".into(),
            r => format!("rcode{r}"),
        };
        Some(format!("response {name} {qt} {rcode} ({an} ans)"))
    }
}

fn decode_pcap(data: &[u8], max: usize) -> Vec<PacketSummary> {
    let mut out = Vec::new();
    if data.len() < 24 || &data[..4] != 0xa1b2c3d4u32.to_le_bytes().as_slice() {
        return out;
    }
    let mut off = 24;
    while off + 16 <= data.len() && out.len() < max {
        let ts_sec = u32::from_le_bytes(data[off..off + 4].try_into().unwrap()) as f64;
        let ts_usec = u32::from_le_bytes(data[off + 4..off + 8].try_into().unwrap()) as f64;
        let incl = u32::from_le_bytes(data[off + 8..off + 12].try_into().unwrap()) as usize;
        let orig = u32::from_le_bytes(data[off + 12..off + 16].try_into().unwrap());
        off += 16;
        if off + incl > data.len() {
            break;
        }
        let frame = &data[off..off + incl];
        off += incl;
        out.push(decode_frame(ts_sec + ts_usec / 1e6, orig, frame));
    }
    out
}

fn decode_frame(ts: f64, len: u32, f: &[u8]) -> PacketSummary {
    let mut p = PacketSummary {
        ts,
        len,
        src: String::new(),
        dst: String::new(),
        proto: "eth".into(),
        info: String::new(),
    };
    if f.len() < 14 {
        p.info = "runt frame".into();
        return p;
    }
    p.dst = mac(&f[0..6]);
    p.src = mac(&f[6..12]);
    let ethertype = u16::from_be_bytes([f[12], f[13]]);
    let payload = &f[14..];
    match ethertype {
        0x0806 => {
            p.proto = "ARP".into();
            if payload.len() >= 28 {
                let op = u16::from_be_bytes([payload[6], payload[7]]);
                let spa = format!(
                    "{}.{}.{}.{}",
                    payload[14], payload[15], payload[16], payload[17]
                );
                let tpa = format!(
                    "{}.{}.{}.{}",
                    payload[24], payload[25], payload[26], payload[27]
                );
                p.info = if op == 1 {
                    format!("who has {tpa}? tell {spa}")
                } else {
                    format!("{spa} is at {}", mac(&payload[8..14]))
                };
            }
        }
        0x0800 if payload.len() >= 20 => {
            let ihl = ((payload[0] & 0x0f) as usize) * 4;
            let proto = payload[9];
            p.src = format!(
                "{}.{}.{}.{}",
                payload[12], payload[13], payload[14], payload[15]
            );
            p.dst = format!(
                "{}.{}.{}.{}",
                payload[16], payload[17], payload[18], payload[19]
            );
            let l4 = &payload[ihl.min(payload.len())..];
            match proto {
                1 => {
                    p.proto = "ICMP".into();
                    p.info = match l4.first() {
                        Some(8) => "echo request".into(),
                        Some(0) => "echo reply".into(),
                        Some(t) => format!("type {t}"),
                        None => String::new(),
                    };
                }
                6 if l4.len() >= 14 => {
                    p.proto = "TCP".into();
                    let sport = u16::from_be_bytes([l4[0], l4[1]]);
                    let dport = u16::from_be_bytes([l4[2], l4[3]]);
                    let flags = l4[13];
                    let mut fs = Vec::new();
                    for (bit, name) in [
                        (0x02, "SYN"),
                        (0x10, "ACK"),
                        (0x01, "FIN"),
                        (0x04, "RST"),
                        (0x08, "PSH"),
                    ] {
                        if flags & bit != 0 {
                            fs.push(name);
                        }
                    }
                    p.info = format!("{sport} → {dport} [{}]", fs.join(","));
                }
                17 if l4.len() >= 8 => {
                    let sport = u16::from_be_bytes([l4[0], l4[1]]);
                    let dport = u16::from_be_bytes([l4[2], l4[3]]);
                    let dns = (sport == 53 || dport == 53)
                        .then(|| dns_info(&l4[8..]))
                        .flatten();
                    match dns {
                        Some(info) => {
                            p.proto = "DNS".into();
                            p.info = info;
                        }
                        None => {
                            p.proto = "UDP".into();
                            p.info = format!("{sport} → {dport}");
                        }
                    }
                }
                89 => p.proto = "OSPF".into(),
                other => p.proto = format!("ip/{other}"),
            }
        }
        0x86dd if payload.len() >= 40 => {
            p.proto = "IPv6".into();
            let seg = |b: &[u8]| -> String {
                (0..8)
                    .map(|i| format!("{:x}", u16::from_be_bytes([b[i * 2], b[i * 2 + 1]])))
                    .collect::<Vec<_>>()
                    .join(":")
            };
            p.src = seg(&payload[8..24]);
            p.dst = seg(&payload[24..40]);
            p.info = format!("next header {}", payload[6]);
        }
        other => {
            p.proto = format!("0x{other:04x}");
        }
    }
    p
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pcap_with(frames: &[&[u8]]) -> Vec<u8> {
        let mut d = Vec::new();
        d.extend_from_slice(&0xa1b2c3d4u32.to_le_bytes());
        d.extend_from_slice(&2u16.to_le_bytes());
        d.extend_from_slice(&4u16.to_le_bytes());
        d.extend_from_slice(&0i32.to_le_bytes());
        d.extend_from_slice(&0u32.to_le_bytes());
        d.extend_from_slice(&65535u32.to_le_bytes());
        d.extend_from_slice(&1u32.to_le_bytes());
        for f in frames {
            d.extend_from_slice(&100u32.to_le_bytes());
            d.extend_from_slice(&0u32.to_le_bytes());
            d.extend_from_slice(&(f.len() as u32).to_le_bytes());
            d.extend_from_slice(&(f.len() as u32).to_le_bytes());
            d.extend_from_slice(f);
        }
        d
    }

    #[test]
    fn decodes_icmp_echo() {
        // eth(dst,src,0x0800) + ipv4(icmp 10.0.0.1->10.0.0.2) + icmp echo req
        let mut f = vec![0u8; 14];
        f[..6].copy_from_slice(&[0x52, 0x54, 0, 0, 0, 2]);
        f[6..12].copy_from_slice(&[0x52, 0x54, 0, 0, 0, 1]);
        f[12..14].copy_from_slice(&0x0800u16.to_be_bytes());
        let mut ip = vec![0u8; 20];
        ip[0] = 0x45;
        ip[9] = 1; // icmp
        ip[12..16].copy_from_slice(&[10, 0, 0, 1]);
        ip[16..20].copy_from_slice(&[10, 0, 0, 2]);
        f.extend_from_slice(&ip);
        f.extend_from_slice(&[8, 0, 0, 0]); // echo request
        let pcap = pcap_with(&[&f]);

        let rows = decode_pcap(&pcap, 10);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].proto, "ICMP");
        assert_eq!(rows[0].src, "10.0.0.1");
        assert_eq!(rows[0].dst, "10.0.0.2");
        assert_eq!(rows[0].info, "echo request");
    }

    #[test]
    fn decodes_arp_and_tcp() {
        // ARP request
        let mut arp = vec![0u8; 14];
        arp[12..14].copy_from_slice(&0x0806u16.to_be_bytes());
        let mut body = vec![0u8; 28];
        body[6..8].copy_from_slice(&1u16.to_be_bytes()); // op request
        body[14..18].copy_from_slice(&[10, 0, 0, 1]);
        body[24..28].copy_from_slice(&[10, 0, 0, 2]);
        arp.extend_from_slice(&body);

        // TCP SYN 10.0.0.1:12345 -> 10.0.0.2:179
        let mut tcp = vec![0u8; 14];
        tcp[12..14].copy_from_slice(&0x0800u16.to_be_bytes());
        let mut ip = vec![0u8; 20];
        ip[0] = 0x45;
        ip[9] = 6;
        ip[12..16].copy_from_slice(&[10, 0, 0, 1]);
        ip[16..20].copy_from_slice(&[10, 0, 0, 2]);
        tcp.extend_from_slice(&ip);
        let mut l4 = vec![0u8; 20];
        l4[0..2].copy_from_slice(&12345u16.to_be_bytes());
        l4[2..4].copy_from_slice(&179u16.to_be_bytes());
        l4[13] = 0x02; // SYN
        tcp.extend_from_slice(&l4);

        let rows = decode_pcap(&pcap_with(&[&arp, &tcp]), 10);
        assert_eq!(rows[0].proto, "ARP");
        assert!(rows[0].info.contains("who has 10.0.0.2"));
        assert_eq!(rows[1].proto, "TCP");
        assert_eq!(rows[1].info, "12345 → 179 [SYN]");
    }

    #[test]
    fn garbage_is_safe() {
        assert!(decode_pcap(b"not a pcap", 10).is_empty());
        assert!(decode_pcap(&[], 10).is_empty());
    }

    fn dns_frame(sport: u16, dport: u16, dns: &[u8]) -> Vec<u8> {
        let mut f = vec![0u8; 14];
        f[12..14].copy_from_slice(&0x0800u16.to_be_bytes());
        let mut ip = vec![0u8; 20];
        ip[0] = 0x45;
        ip[9] = 17; // udp
        ip[12..16].copy_from_slice(&[10, 0, 1, 2]);
        ip[16..20].copy_from_slice(&[10, 53, 53, 53]);
        f.extend_from_slice(&ip);
        let mut udp = vec![0u8; 8];
        udp[0..2].copy_from_slice(&sport.to_be_bytes());
        udp[2..4].copy_from_slice(&dport.to_be_bytes());
        f.extend_from_slice(&udp);
        f.extend_from_slice(dns);
        f
    }

    #[test]
    fn decodes_dns_query_and_response() {
        // query www.example.lab. A (rd), then NXDOMAIN response
        let mut q = vec![0, 1, 0x01, 0x00, 0, 1, 0, 0, 0, 0, 0, 0];
        for label in ["www", "example", "lab"] {
            q.push(label.len() as u8);
            q.extend_from_slice(label.as_bytes());
        }
        q.push(0);
        q.extend_from_slice(&1u16.to_be_bytes()); // qtype A
        q.extend_from_slice(&1u16.to_be_bytes()); // class IN
        let mut r = q.clone();
        r[2] = 0x81; // qr rd
        r[3] = 0x83; // ra + rcode 3

        let rows = decode_pcap(
            &pcap_with(&[&dns_frame(40000, 53, &q), &dns_frame(53, 40000, &r)]),
            10,
        );
        assert_eq!(rows[0].proto, "DNS");
        assert_eq!(rows[0].info, "query www.example.lab. A");
        assert_eq!(rows[1].proto, "DNS");
        assert_eq!(rows[1].info, "response www.example.lab. A NXDOMAIN (0 ans)");
        // non-53 UDP stays generic
        let rows = decode_pcap(&pcap_with(&[&dns_frame(4000, 4001, &q)]), 10);
        assert_eq!(rows[0].proto, "UDP");
        assert_eq!(rows[0].info, "4000 → 4001");
    }
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
