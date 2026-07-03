//! Minimal QMP (QEMU Machine Protocol) client over a unix socket.
//!
//! Protocol: server sends a greeting; client must send
//! `{"execute":"qmp_capabilities"}` before other commands. Every command
//! gets a `{"return": ...}` or `{"error": ...}` response; asynchronous
//! events may be interleaved and are skipped when awaiting a response.

use std::path::Path;
use std::time::Duration;

use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;

use crate::{QemuError, Result};

pub struct QmpClient {
    reader: BufReader<tokio::net::unix::OwnedReadHalf>,
    writer: tokio::net::unix::OwnedWriteHalf,
}

const QMP_TIMEOUT: Duration = Duration::from_secs(5);

impl QmpClient {
    /// Connect and negotiate capabilities.
    pub async fn connect(socket: &Path) -> Result<Self> {
        let stream = tokio::time::timeout(QMP_TIMEOUT, UnixStream::connect(socket))
            .await
            .map_err(|_| QemuError::Qmp("connect timeout".into()))??;
        let (r, w) = stream.into_split();
        let mut client = Self {
            reader: BufReader::new(r),
            writer: w,
        };
        // Greeting.
        let greeting = client.read_message().await?;
        if greeting.get("QMP").is_none() {
            return Err(QemuError::Qmp("missing QMP greeting".into()));
        }
        client.execute("qmp_capabilities", None).await?;
        Ok(client)
    }

    async fn read_message(&mut self) -> Result<Value> {
        let mut line = String::new();
        let n = tokio::time::timeout(QMP_TIMEOUT, self.reader.read_line(&mut line))
            .await
            .map_err(|_| QemuError::Qmp("read timeout".into()))??;
        if n == 0 {
            return Err(QemuError::Qmp("connection closed".into()));
        }
        serde_json::from_str(&line).map_err(|e| QemuError::Qmp(format!("bad json: {e}")))
    }

    /// Execute a command, skipping interleaved events, returning `return`.
    pub async fn execute(&mut self, command: &str, arguments: Option<Value>) -> Result<Value> {
        let mut msg = json!({ "execute": command });
        if let Some(args) = arguments {
            msg["arguments"] = args;
        }
        let text = format!("{msg}\n");
        self.writer.write_all(text.as_bytes()).await?;

        loop {
            let reply = self.read_message().await?;
            if let Some(err) = reply.get("error") {
                return Err(QemuError::Qmp(err.to_string()));
            }
            if let Some(ret) = reply.get("return") {
                return Ok(ret.clone());
            }
            // event — skip
        }
    }

    /// Ask the guest to power down gracefully (ACPI).
    pub async fn system_powerdown(&mut self) -> Result<()> {
        self.execute("system_powerdown", None).await.map(|_| ())
    }

    /// Hard-stop the emulator.
    pub async fn quit(&mut self) -> Result<()> {
        // QEMU may close the socket before replying; both are success.
        match self.execute("quit", None).await {
            Ok(_) => Ok(()),
            Err(QemuError::Qmp(m)) if m.contains("connection closed") => Ok(()),
            Err(e) => Err(e),
        }
    }

    /// Set carrier state of a netdev as seen by the guest.
    pub async fn set_link(&mut self, netdev_id: &str, up: bool) -> Result<()> {
        self.execute("set_link", Some(json!({ "name": netdev_id, "up": up })))
            .await
            .map(|_| ())
    }

    pub async fn query_status(&mut self) -> Result<String> {
        let v = self.execute("query-status", None).await?;
        Ok(v.get("status")
            .and_then(|s| s.as_str())
            .unwrap_or("unknown")
            .to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::AsyncReadExt;
    use tokio::net::UnixListener;

    /// Fake QMP server implementing greeting + capabilities + one command.
    async fn fake_qmp(listener: UnixListener) {
        let (mut stream, _) = listener.accept().await.unwrap();
        stream
            .write_all(b"{\"QMP\": {\"version\": {}, \"capabilities\": []}}\n")
            .await
            .unwrap();
        let mut buf = vec![0u8; 4096];
        loop {
            let n = match stream.read(&mut buf).await {
                Ok(0) | Err(_) => return,
                Ok(n) => n,
            };
            let text = String::from_utf8_lossy(&buf[..n]);
            for line in text.lines() {
                let v: Value = serde_json::from_str(line).unwrap();
                let reply = match v["execute"].as_str() {
                    Some("qmp_capabilities") => json!({"return": {}}),
                    Some("query-status") => {
                        // interleave an event first — client must skip it
                        stream
                            .write_all(b"{\"event\": \"NIC_RX_FILTER_CHANGED\"}\n")
                            .await
                            .unwrap();
                        json!({"return": {"status": "running", "running": true}})
                    }
                    Some("set_link") => json!({"return": {}}),
                    _ => json!({"error": {"class": "CommandNotFound", "desc": "nope"}}),
                };
                stream
                    .write_all(format!("{reply}\n").as_bytes())
                    .await
                    .unwrap();
            }
        }
    }

    #[tokio::test]
    async fn qmp_handshake_and_commands() {
        let dir = tempfile::tempdir().unwrap();
        let sock = dir.path().join("qmp.sock");
        let listener = UnixListener::bind(&sock).unwrap();
        tokio::spawn(fake_qmp(listener));

        let mut client = QmpClient::connect(&sock).await.unwrap();
        assert_eq!(client.query_status().await.unwrap(), "running");
        client.set_link("np0", false).await.unwrap();
        let err = client.execute("bogus", None).await.unwrap_err();
        assert!(matches!(err, QemuError::Qmp(_)));
    }
}
