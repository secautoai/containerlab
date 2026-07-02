//! WebSocket endpoints: event stream, console bridge, AI agent chat.

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Path, State};
use axum::response::Response;
use futures::{SinkExt, StreamExt};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use uuid::Uuid;

use crate::state::AppState;

/// Live event stream: every core Event as a JSON text message.
pub async fn events(ws: WebSocketUpgrade, State(state): State<AppState>) -> Response {
    ws.on_upgrade(move |socket| async move {
        let (mut tx, mut rx) = socket.split();
        let mut events = state.events.subscribe();
        loop {
            tokio::select! {
                ev = events.recv() => {
                    let Ok(ev) = ev else { break };
                    let Ok(json) = serde_json::to_string(&ev) else { continue };
                    if tx.send(Message::Text(json.into())).await.is_err() {
                        break;
                    }
                }
                msg = rx.next() => {
                    match msg {
                        Some(Ok(Message::Close(_))) | None | Some(Err(_)) => break,
                        _ => {} // ignore pings/client chatter
                    }
                }
            }
        }
    })
}

/// Serial console bridge: WebSocket ⇄ node's unix console socket.
///
/// Binary/text WS frames carry raw bytes both ways — xterm.js attaches
/// directly. Multiple viewers may connect; QEMU's chardev accepts one
/// client at a time, so we hold one upstream connection per WS client
/// (QEMU socket chardev supports reconnect; concurrent viewers each get
/// their own accept when `server=on`).
pub async fn console(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
    Path((lab_id, node_id)): Path<(Uuid, Uuid)>,
) -> Response {
    ws.on_upgrade(move |socket| async move {
        if let Err(e) = bridge_console(socket, state, lab_id, node_id).await {
            tracing::debug!("console bridge closed: {e}");
        }
    })
}

async fn bridge_console(
    socket: WebSocket,
    state: AppState,
    lab_id: Uuid,
    node_id: Uuid,
) -> anyhow::Result<()> {
    let (mut ws_tx, mut ws_rx) = socket.split();

    let sock_path = match state.supervisor.console_socket(lab_id, node_id).await {
        Ok(p) => p,
        Err(_) => {
            let _ = ws_tx
                .send(Message::Text(
                    "\r\n[netpilot] node is not running — start it first\r\n"
                        .to_string()
                        .into(),
                ))
                .await;
            return Ok(());
        }
    };

    let stream = match tokio::net::UnixStream::connect(&sock_path).await {
        Ok(s) => s,
        Err(e) => {
            let _ = ws_tx
                .send(Message::Text(
                    format!("\r\n[netpilot] console unavailable: {e}\r\n").into(),
                ))
                .await;
            return Ok(());
        }
    };
    let (mut sock_rx, mut sock_tx) = stream.into_split();

    // guest -> browser
    let downstream = tokio::spawn(async move {
        let mut buf = [0u8; 4096];
        loop {
            match sock_rx.read(&mut buf).await {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    if ws_tx
                        .send(Message::Binary(buf[..n].to_vec().into()))
                        .await
                        .is_err()
                    {
                        break;
                    }
                }
            }
        }
    });

    // browser -> guest
    while let Some(Ok(msg)) = ws_rx.next().await {
        let bytes: Vec<u8> = match msg {
            Message::Binary(b) => b.to_vec(),
            Message::Text(t) => t.as_bytes().to_vec(),
            Message::Close(_) => break,
            _ => continue,
        };
        if sock_tx.write_all(&bytes).await.is_err() {
            break;
        }
    }

    downstream.abort();
    Ok(())
}

/// VNC bridge: binary WebSocket frames ⇄ the node's local VNC TCP port.
/// noVNC's RFB client connects straight to this endpoint.
pub async fn vnc(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
    Path((lab_id, node_id)): Path<(Uuid, Uuid)>,
) -> Response {
    ws.on_upgrade(move |socket| async move {
        let (mut ws_tx, mut ws_rx) = socket.split();
        let port = match state.supervisor.vnc_port(lab_id, node_id).await {
            Ok(p) => p,
            Err(e) => {
                let _ = ws_tx
                    .send(Message::Close(Some(axum::extract::ws::CloseFrame {
                        code: 1011,
                        reason: format!("vnc unavailable: {e}").into(),
                    })))
                    .await;
                return;
            }
        };
        let stream = match tokio::net::TcpStream::connect(("127.0.0.1", port)).await {
            Ok(s) => s,
            Err(e) => {
                tracing::debug!("vnc connect failed: {e}");
                return;
            }
        };
        let (mut tcp_rx, mut tcp_tx) = stream.into_split();

        let down = tokio::spawn(async move {
            let mut buf = [0u8; 16384];
            loop {
                match tcp_rx.read(&mut buf).await {
                    Ok(0) | Err(_) => break,
                    Ok(n) => {
                        if ws_tx
                            .send(Message::Binary(buf[..n].to_vec().into()))
                            .await
                            .is_err()
                        {
                            break;
                        }
                    }
                }
            }
        });
        while let Some(Ok(msg)) = ws_rx.next().await {
            match msg {
                Message::Binary(b) => {
                    if tcp_tx.write_all(&b).await.is_err() {
                        break;
                    }
                }
                Message::Close(_) => break,
                _ => {}
            }
        }
        down.abort();
    })
}

/// AI agent chat socket. Protocol: client sends user messages as JSON
/// `{"message": "..."}`; server streams agent output events as JSON.
pub async fn agent(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
    Path(lab_id): Path<Uuid>,
) -> Response {
    ws.on_upgrade(move |socket| async move {
        crate::agent::run_agent_socket(socket, state, lab_id).await;
    })
}
