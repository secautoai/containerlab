//! Event bus: state changes broadcast to WebSocket subscribers and the AI agent.

use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;
use uuid::Uuid;

use crate::lab::NodeState;

/// Events pushed to UI clients over the events WebSocket.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Event {
    LabCreated { lab: Uuid },
    LabUpdated { lab: Uuid },
    LabDeleted { lab: Uuid },
    NodeState { lab: Uuid, node: Uuid, state: NodeState, detail: Option<String> },
    LinkUpdated { lab: Uuid, link: Uuid },
    /// Free-form log line (orchestrator, agent...) for the UI activity feed.
    Log { lab: Option<Uuid>, level: String, message: String },
}

/// Cheap clonable broadcast bus.
#[derive(Debug, Clone)]
pub struct EventBus {
    tx: broadcast::Sender<Event>,
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new()
    }
}

impl EventBus {
    pub fn new() -> Self {
        let (tx, _) = broadcast::channel(1024);
        Self { tx }
    }

    pub fn publish(&self, event: Event) {
        // Ignore "no receivers" errors: publishing must never fail callers.
        let _ = self.tx.send(event);
    }

    pub fn subscribe(&self) -> broadcast::Receiver<Event> {
        self.tx.subscribe()
    }

    pub fn log(&self, lab: Option<Uuid>, level: &str, message: impl Into<String>) {
        self.publish(Event::Log {
            lab,
            level: level.into(),
            message: message.into(),
        });
    }
}
