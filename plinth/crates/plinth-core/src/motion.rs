//! Motion overlays for still wallpapers (decision D36): an optional animated page drawn above a still picture or the Scene.
//!
//! An overlay is an ordinary wallpaper package of kind `overlay` (see [`crate::wallpaper`]): a web page in an opaque-origin
//! iframe served from its own origin with `connect-src 'none'`, no bridge and no permissions (D23). The only thing the user can
//! tune is DATA: an intensity and a speed that reach the page as two numbers in its query string. This module validates that
//! data and decides, as a pure function, whether an overlay may be drawn at all (one motion item at a time by default, D27/D29).

use crate::impact::Impact;
use crate::wallpaper::WallpaperKind;

pub const MIN_INTENSITY: u8 = 1;
pub const MAX_INTENSITY: u8 = 3;
pub const MIN_SPEED: u8 = 25;
pub const MAX_SPEED: u8 = 200;

/// Extra cost of one motion overlay of medium intensity while the desktop is visible, from `scripts/measure-wallpapers.ps1 -Overlays`
/// (2026-11-06, this PC, release build, 20 s per run, ONE noisy run each; method and every number in `plinth-app/RESULTS.md`):
/// memory +34 to +71 MB (mean +49) over a 320 MB baseline; CPU +0.2 to +8.3% of one core (median about +5) over a 3.5% baseline.
/// The CPU figure is the median of a noisy range, so treat it as an order of magnitude, not a precise value.
pub const OVERLAY_EXTRA: Impact = Impact { ram_mb: 50.0, cpu_pct: 5.0 };

/// What the user chose: which overlay (empty = none) and how it looks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MotionConfig {
    pub id: String,
    /// 1 (subtle) to 3 (busy): how many particles, drops or lights.
    pub intensity: u8,
    /// Animation speed in percent of normal.
    pub speed: u8,
}

impl Default for MotionConfig {
    fn default() -> Self {
        MotionConfig { id: String::new(), intensity: 2, speed: 100 }
    }
}

impl MotionConfig {
    /// Build from untrusted input (saved state or a hub message). An id that is not a valid wallpaper id becomes "none".
    pub fn sanitized(id: &str, intensity: i64, speed: i64) -> MotionConfig {
        let id = if crate::wallpaper::valid_id(id) { id.to_string() } else { String::new() };
        MotionConfig { id, intensity: intensity.clamp(MIN_INTENSITY as i64, MAX_INTENSITY as i64) as u8, speed: speed.clamp(MIN_SPEED as i64, MAX_SPEED as i64) as u8 }
    }

    pub fn is_on(&self) -> bool {
        !self.id.is_empty()
    }

    /// The query string handed to the overlay page: numbers only, so nothing the user typed can reach the page as text.
    pub fn query(&self) -> String {
        format!("i={}&s={}", self.intensity, self.speed)
    }

    /// Cost of this overlay: intensity scales the CPU share of the measured medium value (1 = 70%, 2 = 100%, 3 = 140%).
    pub fn cost(&self) -> Impact {
        let k = match self.intensity {
            1 => 0.7,
            3 => 1.4,
            _ => 1.0,
        };
        Impact { ram_mb: OVERLAY_EXTRA.ram_mb, cpu_pct: OVERLAY_EXTRA.cpu_pct * k }
    }
}

/// Why an overlay is not drawn although one is chosen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Hidden {
    /// A live (video or web) wallpaper is selected on this monitor; only one motion item runs by default.
    LiveWallpaperActive,
    /// The chosen overlay is missing, or is not an overlay package.
    Unavailable,
    /// Safe mode or an unapplied scene: nothing is drawn.
    NotShown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Plan {
    Draw,
    Skip(Hidden),
}

/// Decide whether the overlay is drawn on a monitor. `wallpaper` is the kind of live wallpaper selected there (None = a still
/// picture or the Scene gradient); `overlay_kind` is the kind of the chosen package if it exists.
pub fn plan(cfg: &MotionConfig, overlay_kind: Option<WallpaperKind>, wallpaper: Option<WallpaperKind>, shown: bool, allow_both: bool) -> Option<Plan> {
    if !cfg.is_on() {
        return None;
    }
    if !shown {
        return Some(Plan::Skip(Hidden::NotShown));
    }
    if overlay_kind != Some(WallpaperKind::Overlay) {
        return Some(Plan::Skip(Hidden::Unavailable));
    }
    if wallpaper.is_some() && !allow_both {
        return Some(Plan::Skip(Hidden::LiveWallpaperActive));
    }
    Some(Plan::Draw)
}

#[cfg(test)]
mod tests {
    use super::*;
    use WallpaperKind::*;

    fn on() -> MotionConfig {
        MotionConfig::sanitized("com.plinth.fx-rain", 2, 100)
    }

    #[test]
    fn untrusted_input_is_clamped_and_bad_ids_mean_none() {
        let c = MotionConfig::sanitized("com.plinth.fx-rain", 99, -5);
        assert_eq!((c.intensity, c.speed), (MAX_INTENSITY, MIN_SPEED));
        assert_eq!(MotionConfig::sanitized("x", 2, 100).id, ""); // too short
        assert_eq!(MotionConfig::sanitized("../evil", 2, 100).id, "");
        assert_eq!(MotionConfig::sanitized("a b c", 2, 100).id, "");
        assert_eq!(MotionConfig::sanitized("UPPER.case", 2, 100).id, "");
        assert!(!MotionConfig::sanitized("", 2, 100).is_on());
        assert_eq!(MotionConfig::sanitized("com.plinth.fx-rain", 0, 1_000_000).intensity, MIN_INTENSITY);
    }

    #[test]
    fn the_query_string_is_numbers_only() {
        for (i, s) in [(1, 25), (2, 100), (3, 200), (-9, 7), (50, 9999)] {
            let q = MotionConfig::sanitized("com.plinth.fx-rain", i, s).query();
            assert!(q.bytes().all(|b| b.is_ascii_digit() || b == b'=' || b == b'&' || b == b'i' || b == b's'), "{q}");
        }
        assert_eq!(on().query(), "i=2&s=100");
    }

    #[test]
    fn nothing_is_planned_when_no_overlay_is_chosen() {
        assert_eq!(plan(&MotionConfig::default(), Some(Overlay), None, true, false), None);
    }

    #[test]
    fn draws_above_a_still_picture_or_the_scene() {
        assert_eq!(plan(&on(), Some(Overlay), None, true, false), Some(Plan::Draw));
    }

    #[test]
    fn only_one_motion_item_by_default() {
        assert_eq!(plan(&on(), Some(Overlay), Some(Video), true, false), Some(Plan::Skip(Hidden::LiveWallpaperActive)));
        assert_eq!(plan(&on(), Some(Overlay), Some(Web), true, false), Some(Plan::Skip(Hidden::LiveWallpaperActive)));
        assert_eq!(plan(&on(), Some(Overlay), Some(Web), true, true), Some(Plan::Draw), "explicitly allowed");
    }

    #[test]
    fn a_missing_package_or_a_wallpaper_kind_is_not_an_overlay() {
        assert_eq!(plan(&on(), None, None, true, false), Some(Plan::Skip(Hidden::Unavailable)));
        assert_eq!(plan(&on(), Some(Web), None, true, false), Some(Plan::Skip(Hidden::Unavailable)));
        assert_eq!(plan(&on(), Some(Video), None, true, false), Some(Plan::Skip(Hidden::Unavailable)));
    }

    #[test]
    fn nothing_is_drawn_when_nothing_is_shown() {
        assert_eq!(plan(&on(), Some(Overlay), None, false, false), Some(Plan::Skip(Hidden::NotShown)));
    }

    #[test]
    fn intensity_scales_cpu_but_not_memory() {
        let (a, b, c) = (MotionConfig::sanitized("com.plinth.fx-rain", 1, 100).cost(), on().cost(), MotionConfig::sanitized("com.plinth.fx-rain", 3, 100).cost());
        assert!(a.cpu_pct < b.cpu_pct && b.cpu_pct < c.cpu_pct);
        assert_eq!((a.ram_mb, c.ram_mb), (b.ram_mb, b.ram_mb));
    }

    #[test]
    fn overlay_kind_costs_about_a_web_wallpaper_or_less() {
        assert!(crate::impact::wallpaper_extra(Overlay).cpu_pct <= crate::impact::wallpaper_extra(Web).cpu_pct);
    }
}
