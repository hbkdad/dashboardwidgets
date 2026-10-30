//! Monitors and widget placement (decisions D25, D26; PRD 5.4, architecture 11).
//!
//! Placement is stored per widget as a *home* monitor plus an anchor, offsets and a size in device-independent pixels
//! (DIP, 96 per inch). [`reconcile`] is a pure function of (saved placements, currently connected monitors): it never
//! mutates what was saved, so a transient state (a monitor that is late after wake, a parked widget, a clamped position) can
//! never be written back over the real layout. That is the failure documented for Rainmeter in research/monitors-and-wallpapers.md.

use serde::{Deserialize, Serialize};

pub const MIN_SCALE: f32 = 0.5;
/// A monitor must be present this long (ms) before parked widgets return to it; the app debounces with this.
pub const RETURN_DELAY_MS: u64 = 2000;
/// Display events are coalesced for this long (ms) before the layout is recomputed.
pub const DEBOUNCE_MS: u64 = 1500;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MonitorInfo {
    /// Stable identity: the monitor's device path (D25). Falls back to the GDI device name when the path is unavailable.
    pub id: String,
    /// EDID-based fingerprint `manufacturer:product:connector`, used when the id changes (new port or driver).
    pub fingerprint: String,
    pub name: String,
    /// Top-left of the monitor in virtual-screen pixels.
    pub x: i32,
    pub y: i32,
    pub width_px: u32,
    pub height_px: u32,
    /// Work area (excluding the taskbar) in virtual-screen pixels: x, y, width, height.
    pub work: (i32, i32, u32, u32),
    pub dpi: u32,
    pub primary: bool,
    pub refresh_hz: Option<u32>,
}

impl MonitorInfo {
    pub fn scale(&self) -> f32 {
        self.dpi.max(48) as f32 / 96.0
    }

    pub fn scale_percent(&self) -> u32 {
        (self.scale() * 100.0).round() as u32
    }

    pub fn portrait(&self) -> bool {
        self.height_px > self.width_px
    }

    /// Monitor size in DIP.
    pub fn size_dip(&self) -> (f32, f32) {
        (self.width_px as f32 / self.scale(), self.height_px as f32 / self.scale())
    }

    /// Work area relative to the monitor's own origin, in DIP: x, y, width, height.
    pub fn work_dip(&self) -> (f32, f32, f32, f32) {
        let s = self.scale();
        ((self.work.0 - self.x) as f32 / s, (self.work.1 - self.y) as f32 / s, self.work.2 as f32 / s, self.work.3 as f32 / s)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Anchor {
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
    Center,
}

impl Anchor {
    pub fn parse(s: &str) -> Option<Anchor> {
        Some(match s {
            "top-left" => Anchor::TopLeft,
            "top-right" => Anchor::TopRight,
            "bottom-left" => Anchor::BottomLeft,
            "bottom-right" => Anchor::BottomRight,
            "center" => Anchor::Center,
            _ => return None,
        })
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Anchor::TopLeft => "top-left",
            Anchor::TopRight => "top-right",
            Anchor::BottomLeft => "bottom-left",
            Anchor::BottomRight => "bottom-right",
            Anchor::Center => "center",
        }
    }
}

/// What the user saved for one widget. `home` is a monitor id, or the role `"primary"`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Placement {
    pub widget: String,
    pub home: String,
    /// Fingerprint of the home monitor when it was assigned (lets a renamed device path still match).
    #[serde(default)]
    pub home_fingerprint: String,
    pub anchor: Anchor,
    pub dx: f32,
    pub dy: f32,
    pub w: f32,
    pub h: f32,
}

pub const ROLE_PRIMARY: &str = "primary";

/// The default place for a widget that has no saved placement (the layout shipped through v0.9).
pub fn default_placement(widget: &str, index: usize) -> Placement {
    let base = widget.split('#').next().unwrap_or(widget);
    let copy = widget.split_once('#').and_then(|(_, n)| n.parse::<usize>().ok()).unwrap_or(1);
    let (anchor, dx, dy, w, h) = match base {
        "com.plinth.clock" => (Anchor::TopLeft, 72.0, 72.0, 360.0, 118.0),
        "com.plinth.pcvitals" => (Anchor::TopRight, 72.0, 72.0, 400.0, 250.0),
        "com.plinth.buildcard" => (Anchor::BottomRight, 72.0, 150.0, 400.0, 190.0),
        "com.plinth.notes" => (Anchor::BottomLeft, 72.0, 150.0, 330.0, 200.0),
        _ => (Anchor::TopLeft, 72.0 + 280.0 * index as f32, 430.0, 300.0, 160.0),
    };
    // A further copy of a widget is offset so it does not sit exactly on top of the first one.
    let shift = 40.0 * (copy.saturating_sub(1)) as f32;
    let (dx, dy) = (dx + shift, dy + shift);
    Placement { widget: widget.to_string(), home: ROLE_PRIMARY.to_string(), home_fingerprint: String::new(), anchor, dx, dy, w, h }
}

/// Where a widget is shown right now. Rectangle in DIP relative to the monitor named `monitor`.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Resolved {
    pub widget: String,
    pub monitor: String,
    /// True when the widget is shown on a monitor other than its home (home is disconnected).
    pub parked: bool,
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    /// 1.0 unless the widget was shrunk to fit a small monitor.
    pub scale: f32,
    /// True when even the minimum scale does not fit and the widget is clipped.
    pub clipped: bool,
}

fn find_home<'a>(p: &Placement, monitors: &'a [MonitorInfo]) -> Option<&'a MonitorInfo> {
    if p.home == ROLE_PRIMARY {
        return monitors.iter().find(|m| m.primary);
    }
    monitors
        .iter()
        .find(|m| m.id == p.home)
        .or_else(|| if p.home_fingerprint.is_empty() { None } else { monitors.iter().find(|m| m.fingerprint == p.home_fingerprint) })
}

/// Compute the on-screen rectangle of every widget. Pure: `placements` is only read.
///
/// * home monitor connected: placed there (matched by id, then fingerprint, or the primary for the role `"primary"`);
/// * home missing: parked on `fallback` (a monitor id) if connected, else the primary, else the first monitor;
/// * too big for the monitor's work area: scaled down (never below [`MIN_SCALE`]), then clipped if still too big;
/// * always clamped so the widget stays inside the work area when it fits.
pub fn reconcile(placements: &[Placement], monitors: &[MonitorInfo], fallback: Option<&str>) -> Vec<Resolved> {
    if monitors.is_empty() {
        return Vec::new();
    }
    let fb = fallback.and_then(|f| monitors.iter().find(|m| m.id == f)).or_else(|| monitors.iter().find(|m| m.primary)).unwrap_or(&monitors[0]);
    placements
        .iter()
        .map(|p| {
            let home = find_home(p, monitors);
            let (mon, parked) = match home {
                Some(m) => (m, false),
                None => (fb, true),
            };
            let (wx, wy, ww, wh) = mon.work_dip();
            let mut scale = 1.0f32;
            if p.w > ww || p.h > wh {
                scale = (ww / p.w.max(1.0)).min(wh / p.h.max(1.0)).max(MIN_SCALE);
            }
            let (w, h) = (p.w * scale, p.h * scale);
            let clipped = w > ww + 0.5 || h > wh + 0.5;
            let (dx, dy) = (p.dx * scale, p.dy * scale);
            let (mut x, mut y) = match p.anchor {
                Anchor::TopLeft => (wx + dx, wy + dy),
                Anchor::TopRight => (wx + ww - w - dx, wy + dy),
                Anchor::BottomLeft => (wx + dx, wy + wh - h - dy),
                Anchor::BottomRight => (wx + ww - w - dx, wy + wh - h - dy),
                Anchor::Center => (wx + (ww - w) / 2.0 + dx, wy + (wh - h) / 2.0 + dy),
            };
            let clamp = |v: f32, lo: f32, size: f32, span: f32| if size >= span { lo } else { v.clamp(lo, lo + span - size) };
            x = clamp(x, wx, w, ww);
            y = clamp(y, wy, h, wh);
            Resolved { widget: p.widget.clone(), monitor: mon.id.clone(), parked, x, y, w, h, scale, clipped }
        })
        .collect()
}

/// "Bring all widgets to the primary monitor": the same computation with every home ignored. Does not change `placements`.
pub fn bring_all_to(placements: &[Placement], monitors: &[MonitorInfo], target: &str) -> Vec<Resolved> {
    let moved: Vec<Placement> = placements.iter().map(|p| Placement { home: "\u{0}nowhere".into(), home_fingerprint: String::new(), ..p.clone() }).collect();
    reconcile(&moved, monitors, Some(target))
}

/// Identify what changed between two monitor lists (for logging and the hub's notices).
pub fn diff_monitors(before: &[MonitorInfo], after: &[MonitorInfo]) -> (Vec<String>, Vec<String>) {
    let gone = before.iter().filter(|b| !after.iter().any(|a| a.id == b.id || a.fingerprint == b.fingerprint)).map(|m| m.name.clone()).collect();
    let came = after.iter().filter(|a| !before.iter().any(|b| b.id == a.id || b.fingerprint == a.fingerprint)).map(|m| m.name.clone()).collect();
    (gone, came)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mon(id: &str, x: i32, w: u32, h: u32, dpi: u32, primary: bool) -> MonitorInfo {
        MonitorInfo { id: id.into(), fingerprint: format!("fp-{id}"), name: format!("Mon {id}"), x, y: 0, width_px: w, height_px: h, work: (x, 0, w, h - (40.0 * dpi as f32 / 96.0) as u32), dpi, primary, refresh_hz: Some(60) }
    }

    fn place(widget: &str, home: &str) -> Placement {
        Placement { widget: widget.into(), home: home.into(), home_fingerprint: format!("fp-{home}"), anchor: Anchor::TopLeft, dx: 50.0, dy: 40.0, w: 300.0, h: 200.0 }
    }

    fn three() -> Vec<MonitorInfo> {
        vec![mon("A", 0, 1920, 1080, 96, true), mon("B", 1920, 2560, 1440, 144, false), mon("C", 4480, 1080, 1920, 96, false)]
    }

    #[test]
    fn widgets_land_on_their_home_monitors() {
        let ps = vec![place("w1", "A"), place("w2", "B"), place("w3", "C")];
        let r = reconcile(&ps, &three(), None);
        assert_eq!(r.iter().map(|x| (x.monitor.as_str(), x.parked)).collect::<Vec<_>>(), vec![("A", false), ("B", false), ("C", false)]);
        assert_eq!((r[0].x, r[0].y), (50.0, 40.0));
    }

    #[test]
    fn unplugging_parks_on_primary_and_replugging_restores_exactly() {
        let ps = vec![place("w1", "A"), place("w2", "B"), place("w3", "C")];
        let before = reconcile(&ps, &three(), None);
        let mut two = three();
        two.remove(1);
        let parked = reconcile(&ps, &two, None);
        assert!(parked[1].parked && parked[1].monitor == "A");
        assert!(!parked[0].parked && !parked[2].parked);
        // the saved placements were never touched, so the original layout comes back identically
        let after = reconcile(&ps, &three(), None);
        assert_eq!(before, after);
    }

    #[test]
    fn user_chosen_fallback_is_used_when_connected() {
        let ps = vec![place("w2", "B")];
        let mut two = three();
        two.remove(1);
        assert_eq!(reconcile(&ps, &two, Some("C"))[0].monitor, "C");
        assert_eq!(reconcile(&ps, &two, Some("gone"))[0].monitor, "A", "an unavailable fallback falls back to the primary");
    }

    #[test]
    fn id_change_is_recovered_by_fingerprint() {
        let ps = vec![place("w2", "B")];
        let mut ms = three();
        ms[1].id = "B-new-port".into(); // same hardware on another port: same fingerprint
        let r = reconcile(&ps, &ms, None);
        assert_eq!((r[0].monitor.as_str(), r[0].parked), ("B-new-port", false));
    }

    #[test]
    fn role_primary_follows_the_primary() {
        let ps = vec![place("w1", ROLE_PRIMARY)];
        let mut ms = three();
        assert_eq!(reconcile(&ps, &ms, None)[0].monitor, "A");
        ms[0].primary = false;
        ms[1].primary = true;
        assert_eq!(reconcile(&ps, &ms, None)[0].monitor, "B");
    }

    #[test]
    fn resolution_and_scale_change_keep_widgets_on_screen() {
        let mut p = place("w1", "A");
        p.anchor = Anchor::BottomRight;
        p.dx = 72.0;
        p.dy = 150.0;
        for (w, h, dpi) in [(1920, 1080, 96), (1280, 720, 96), (3840, 2160, 192), (1366, 768, 120), (800, 600, 96)] {
            let ms = vec![mon("A", 0, w, h, dpi, true)];
            let r = &reconcile(&[p.clone()], &ms, None)[0];
            let (wx, wy, ww, wh) = ms[0].work_dip();
            assert!(r.x >= wx - 0.01 && r.y >= wy - 0.01 && r.x + r.w <= wx + ww + 0.01 && r.y + r.h <= wy + wh + 0.01, "{w}x{h}@{dpi}: {r:?} not inside {:?}", (wx, wy, ww, wh));
        }
    }

    #[test]
    fn oversize_widgets_shrink_then_clip() {
        let mut p = place("big", "A");
        p.w = 1100.0;
        p.h = 700.0;
        let small = vec![mon("A", 0, 1024, 768, 96, true)]; // work area about 1024 x 728 DIP
        let r = &reconcile(&[p.clone()], &small, None)[0];
        assert!(r.scale < 1.0 && r.scale >= MIN_SCALE && !r.clipped);
        p.w = 4000.0;
        let r2 = &reconcile(&[p], &small, None)[0];
        assert_eq!(r2.scale, MIN_SCALE);
        assert!(r2.clipped);
    }

    #[test]
    fn portrait_monitors_work() {
        let ps = vec![place("w3", "C")];
        let r = &reconcile(&ps, &three(), None)[0];
        let ms = three();
        let (_, _, ww, wh) = ms[2].work_dip();
        assert!(ww < wh);
        assert!(r.x + r.w <= ww && r.y + r.h <= wh);
    }

    #[test]
    fn no_monitors_means_nothing_to_place() {
        assert!(reconcile(&[place("w", "A")], &[], None).is_empty());
    }

    #[test]
    fn bring_all_to_primary_does_not_change_saved_data() {
        let ps = vec![place("w1", "A"), place("w2", "B")];
        let snapshot = ps.clone();
        let r = bring_all_to(&ps, &three(), "A");
        assert!(r.iter().all(|x| x.monitor == "A" && x.parked));
        assert_eq!(ps, snapshot);
    }

    #[test]
    fn center_anchor_and_clamping() {
        let mut p = place("w", "A");
        p.anchor = Anchor::Center;
        p.dx = 0.0;
        p.dy = 0.0;
        let ms = vec![mon("A", 0, 1920, 1080, 96, true)];
        let r = &reconcile(&[p.clone()], &ms, None)[0];
        assert!((r.x - (1920.0 - 300.0) / 2.0).abs() < 0.01);
        p.dx = 99999.0; // absurd saved offset must not strand the widget off screen
        let r = &reconcile(&[p], &ms, None)[0];
        assert!(r.x + r.w <= 1920.0 + 0.01);
    }

    #[test]
    fn diff_reports_plugs_and_unplugs() {
        let all = three();
        let two: Vec<_> = all.iter().take(2).cloned().collect();
        let (gone, came) = diff_monitors(&all, &two);
        assert_eq!((gone, came.len()), (vec!["Mon C".to_string()], 0));
        let (gone, came) = diff_monitors(&two, &all);
        assert_eq!((gone.len(), came), (0, vec!["Mon C".to_string()]));
    }

    #[test]
    fn extra_copies_get_their_own_default_place() {
        let a = default_placement("com.plinth.clock", 0);
        let b = default_placement("com.plinth.clock#2", 0);
        assert_eq!(a.anchor, b.anchor);
        assert!(b.dx > a.dx && b.dy > a.dy);
        assert_eq!(b.w, a.w);
    }

    #[test]
    fn defaults_are_distinct_and_known_widgets_have_corners() {
        assert_eq!(default_placement("com.plinth.clock", 0).anchor, Anchor::TopLeft);
        assert_eq!(default_placement("com.plinth.pcvitals", 1).anchor, Anchor::TopRight);
        assert_ne!(default_placement("x.a", 0).dx, default_placement("x.b", 1).dx);
        assert_eq!(Anchor::parse("bottom-left"), Some(Anchor::BottomLeft));
        assert_eq!(Anchor::parse("middle"), None);
    }
}
