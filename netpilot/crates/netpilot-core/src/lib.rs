//! netpilot-core — domain model for the NetPilot network emulator.
//!
//! This crate defines the persistent lab model (labs, nodes, links, networks,
//! annotations), device templates, the on-disk lab store, and the event bus
//! used to push state changes to API/WebSocket consumers.

pub mod error;
pub mod event;
pub mod lab;
pub mod store;
pub mod template;

pub use error::{CoreError, Result};
pub use event::{Event, EventBus};
pub use lab::*;
pub use store::LabStore;
pub use template::*;
