//! Plinth core: the pure-logic parts of the design in `../../../design/architecture.md`.
//! No Windows or UI code here, so it is testable on any machine.

pub mod broker;
pub mod context;
pub mod governor;
pub mod impact;
pub mod fit;
pub mod library;
pub mod manifest;
pub mod monitors;
pub mod motion;
pub mod placement;
pub mod security;
pub mod setup;
pub mod wallpaper;

pub use broker::{ActivityEntry, Broker, Decision, DenyReason};
pub use context::{Condition, Context, Engine, Learner, Rule, Suggestion};
pub use governor::{should_pause, Policy, SystemState};
pub use impact::{rate, scene_cost_with_motion, Impact, Rating};
pub use manifest::{DeclaredPermission, Kind, Manifest, ManifestError, PermissionId};
pub use setup::{PixelRect, Placement, SetupCard, SetupError};
pub use wallpaper::{Fit, Wallpaper, WallpaperError, WallpaperKind};
