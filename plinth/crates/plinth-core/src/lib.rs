//! Plinth core: the pure-logic parts of the design in `../../../design/architecture.md`.
//! No Windows or UI code here, so it is testable on any machine.

pub mod broker;
pub mod governor;
pub mod manifest;

pub use broker::{ActivityEntry, Broker, Decision, DenyReason};
pub use governor::{should_pause, Policy, SystemState};
pub use manifest::{DeclaredPermission, Kind, Manifest, ManifestError, PermissionId};
