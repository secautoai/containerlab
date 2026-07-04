//! Bearer-token session store. Uses Redis when `NETPILOT_REDIS_URL` is set
//! (shared across instances, TTL-managed); otherwise an in-process map with
//! manual expiry (fine for a single server).

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use rand::RngCore;
use redis::AsyncCommands;
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::Result;

const TTL_SECONDS: u64 = 60 * 60 * 12; // 12h sliding window
const PREFIX: &str = "netpilot:session:";

#[derive(Clone)]
enum Backend {
    Redis(redis::aio::ConnectionManager),
    Memory(Arc<Mutex<HashMap<String, (Uuid, u64)>>>),
}

/// Opaque bearer tokens → user id, with a sliding TTL.
#[derive(Clone)]
pub struct TokenStore {
    backend: Backend,
}

impl TokenStore {
    /// Redis-backed if `redis_url` is Some and reachable, else in-memory.
    pub async fn new(redis_url: Option<&str>) -> Self {
        if let Some(url) = redis_url {
            match redis::Client::open(url) {
                Ok(client) => match redis::aio::ConnectionManager::new(client).await {
                    Ok(cm) => {
                        tracing::info!("sessions: redis at {url}");
                        return Self { backend: Backend::Redis(cm) };
                    }
                    Err(e) => tracing::warn!("sessions: redis connect failed ({e}); using memory"),
                },
                Err(e) => tracing::warn!("sessions: redis url invalid ({e}); using memory"),
            }
        }
        Self { backend: Backend::Memory(Arc::new(Mutex::new(HashMap::new()))) }
    }

    /// Mint a new token for a user.
    pub async fn issue(&self, user_id: Uuid) -> Result<String> {
        let token = random_token();
        match &self.backend {
            Backend::Redis(cm) => {
                let mut cm = cm.clone();
                let _: () = cm
                    .set_ex(format!("{PREFIX}{token}"), user_id.to_string(), TTL_SECONDS)
                    .await?;
            }
            Backend::Memory(map) => {
                map.lock().await.insert(token.clone(), (user_id, now() + TTL_SECONDS));
            }
        }
        Ok(token)
    }

    /// Resolve a token to a user id, refreshing its TTL (sliding window).
    pub async fn resolve(&self, token: &str) -> Option<Uuid> {
        match &self.backend {
            Backend::Redis(cm) => {
                let mut cm = cm.clone();
                let key = format!("{PREFIX}{token}");
                let val: Option<String> = cm.get(&key).await.ok().flatten();
                let id = val.and_then(|v| Uuid::parse_str(&v).ok())?;
                let _: std::result::Result<(), _> = cm.expire(&key, TTL_SECONDS as i64).await;
                Some(id)
            }
            Backend::Memory(map) => {
                let mut map = map.lock().await;
                match map.get(token) {
                    Some(&(id, exp)) if exp > now() => {
                        map.insert(token.to_string(), (id, now() + TTL_SECONDS));
                        Some(id)
                    }
                    Some(_) => {
                        map.remove(token);
                        None
                    }
                    None => None,
                }
            }
        }
    }

    pub async fn revoke(&self, token: &str) -> Result<()> {
        match &self.backend {
            Backend::Redis(cm) => {
                let mut cm = cm.clone();
                let _: () = cm.del(format!("{PREFIX}{token}")).await?;
            }
            Backend::Memory(map) => {
                map.lock().await.remove(token);
            }
        }
        Ok(())
    }
}

fn random_token() -> String {
    let mut bytes = [0u8; 32];
    rand::rng().fill_bytes(&mut bytes);
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or(Duration::ZERO).as_secs()
}
