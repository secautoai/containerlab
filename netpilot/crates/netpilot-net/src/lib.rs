//! netpilot-net — Linux datapath plumbing for labs.
//!
//! Links between QEMU nodes are wired with tap devices enslaved to
//! per-link Linux bridges:
//!
//! ```text
//!   qemu A ── tap npt-xxxx ──┐
//!                            ├── bridge npb-yyyy
//!   qemu B ── tap npt-zzzz ──┘
//! ```
//!
//! Multipoint networks use the same bridge mechanism with more members.
//! NAT/management networks add a host IP on the bridge plus a MASQUERADE
//! rule. Link impairment applies `tc netem` on member taps; capture runs
//! tcpdump on the bridge.
//!
//! All operations shell out to `ip`/`tc`/`iptables`/`tcpdump` through the
//! [`Runner`] trait so the logic is unit-testable without root.

pub mod names;
pub mod plumbing;
pub mod runner;
pub mod switch;

pub use names::*;
pub use plumbing::*;
pub use runner::*;
pub use switch::*;
