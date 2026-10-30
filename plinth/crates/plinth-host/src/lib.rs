//! Plinth widget host logic (no GUI). The GUI shell wires two things to it:
//! (1) a custom-protocol handler per origin slot -> [`Host::serve`], and
//! (2) the trusted host page's IPC -> [`Host::bridge`].
//! Design: `design/architecture.md` (hybrid delivery, Spike C results).

mod bridge;
mod host;
mod package;
mod wallpapers;

pub use host::{valid_copy, DataProvider, Host, HostError, MockProvider, PermissionInfo, Response, WidgetInfo, SDK_PATH, THIRD_PARTY_SLOTS, TRUSTED_SCHEME};
pub use wallpapers::{Entry as WallpaperEntry, WallpaperStore, WALLPAPER_SCHEME, WEB_CSP};
pub use package::{normalize_path, Package, PackageError, ENTRY, MAX_FILES, MAX_TOTAL_BYTES};
