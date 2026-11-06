//! Drag-and-drop placement math (decisions D26, D35): snapping, picking the monitor under a dropped widget, and turning a dropped
//! rectangle back into the stored anchor + offsets. Pure functions with no window code, so every rule is unit-tested.
//!
//! Coordinates are device-independent pixels (DIP, 96 per inch) relative to a monitor's own top-left corner, the same frame
//! [`crate::monitors::reconcile`] returns in [`crate::monitors::Resolved`]. Nothing here reads or writes saved state: the caller saves
//! only the [`Placement`] returned by [`placed`], and only after the user lets go of a widget (never a parked or clamped result).

use crate::monitors::{Anchor, MonitorInfo, Placement, ROLE_PRIMARY};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Rect {
    pub fn new(x: f32, y: f32, w: f32, h: f32) -> Rect {
        Rect { x, y, w, h }
    }
    pub fn right(&self) -> f32 {
        self.x + self.w
    }
    pub fn bottom(&self) -> f32 {
        self.y + self.h
    }
    pub fn center(&self) -> (f32, f32) {
        (self.x + self.w / 2.0, self.y + self.h / 2.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SnapMode {
    Off,
    /// Round the widget's top-left corner to a grid that starts at the work area's corner.
    Grid,
    /// Pull the widget's edges and center to the work area's edges and center and to other widgets' edges.
    Edges,
    /// Edges when one is close, otherwise the grid.
    Both,
}

impl SnapMode {
    pub fn parse(s: &str) -> Option<SnapMode> {
        Some(match s {
            "off" => SnapMode::Off,
            "grid" => SnapMode::Grid,
            "edges" => SnapMode::Edges,
            "both" => SnapMode::Both,
            _ => return None,
        })
    }
    pub fn as_str(self) -> &'static str {
        match self {
            SnapMode::Off => "off",
            SnapMode::Grid => "grid",
            SnapMode::Edges => "edges",
            SnapMode::Both => "both",
        }
    }
}

pub const MIN_GRID: f32 = 4.0;
pub const MAX_GRID: f32 = 200.0;
pub const DEFAULT_GRID: f32 = 24.0;
/// How close (DIP) an edge must be to pull a widget onto it.
pub const SNAP_DISTANCE: f32 = 10.0;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct SnapConfig {
    pub mode: SnapMode,
    pub grid: f32,
}

impl Default for SnapConfig {
    fn default() -> Self {
        SnapConfig { mode: SnapMode::Both, grid: DEFAULT_GRID }
    }
}

impl SnapConfig {
    /// A config from untrusted input (saved state or a message): unknown mode falls back to the default, the grid is clamped.
    pub fn sanitized(mode: &str, grid: f32) -> SnapConfig {
        SnapConfig { mode: SnapMode::parse(mode).unwrap_or(SnapMode::Both), grid: if grid.is_finite() { grid.clamp(MIN_GRID, MAX_GRID).round() } else { DEFAULT_GRID } }
    }
}

/// Result of snapping: the rectangle to use and the guide lines to draw (x of a vertical guide, y of a horizontal one).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Snapped {
    pub rect: Rect,
    pub guide_x: Option<f32>,
    pub guide_y: Option<f32>,
}

/// Keep a rectangle inside `work` (it is left at the work area's corner when it is bigger than the area).
pub fn clamp_into(r: Rect, work: Rect) -> Rect {
    let c = |v: f32, lo: f32, size: f32, span: f32| if size >= span { lo } else { v.clamp(lo, lo + span - size) };
    Rect { x: c(r.x, work.x, r.w, work.w), y: c(r.y, work.y, r.h, work.h), ..r }
}

/// Best snap on one axis: `pos`/`size` is the widget, `targets` are coordinates the widget's start, middle or end may land on.
/// Returns the corrected start coordinate and the guide coordinate, or `None` when nothing is within `SNAP_DISTANCE`.
fn edge_snap(pos: f32, size: f32, targets: &[f32]) -> Option<(f32, f32)> {
    let mut best: Option<(f32, f32, f32)> = None; // (distance, new pos, guide)
    for &t in targets {
        for offset in [0.0, size / 2.0, size] {
            let d = (pos + offset - t).abs();
            if d <= SNAP_DISTANCE && best.map_or(true, |b| d < b.0) {
                best = Some((d, t - offset, t));
            }
        }
    }
    best.map(|(_, p, g)| (p, g))
}

/// Snap a rectangle being dragged. `others` are the other widgets on the same monitor (never the dragged one).
pub fn snap(proposed: Rect, work: Rect, others: &[Rect], cfg: SnapConfig) -> Snapped {
    let mut r = clamp_into(proposed, work);
    let (mut gx, mut gy) = (None, None);
    if matches!(cfg.mode, SnapMode::Edges | SnapMode::Both) {
        let mut xs = vec![work.x, work.right(), work.x + work.w / 2.0];
        let mut ys = vec![work.y, work.bottom(), work.y + work.h / 2.0];
        for o in others {
            xs.extend([o.x, o.right()]);
            ys.extend([o.y, o.bottom()]);
        }
        if let Some((p, g)) = edge_snap(r.x, r.w, &xs) {
            r.x = p;
            gx = Some(g);
        }
        if let Some((p, g)) = edge_snap(r.y, r.h, &ys) {
            r.y = p;
            gy = Some(g);
        }
    }
    if matches!(cfg.mode, SnapMode::Grid | SnapMode::Both) {
        let g = cfg.grid.clamp(MIN_GRID, MAX_GRID);
        if gx.is_none() {
            r.x = work.x + ((r.x - work.x) / g).round() * g;
        }
        if gy.is_none() {
            r.y = work.y + ((r.y - work.y) / g).round() * g;
        }
    }
    Snapped { rect: clamp_into(r, work), guide_x: gx, guide_y: gy }
}

/// Which connected monitor a widget dropped at `rect` (DIP, relative to the origin of monitor `from`) belongs to, and where that
/// rectangle is relative to the chosen monitor's own origin. The monitor holding the rectangle's center wins; a center in the gap
/// between monitors goes to the nearest one. Works across monitors with different scale, because the position is converted through
/// virtual-screen pixels; the size in DIP is unchanged.
pub fn retarget<'a>(monitors: &'a [MonitorInfo], from: &MonitorInfo, rect: Rect) -> Option<(&'a MonitorInfo, Rect)> {
    if monitors.is_empty() {
        return None;
    }
    let s = from.scale();
    let (cx, cy) = rect.center();
    let (px, py) = (from.x as f32 + cx * s, from.y as f32 + cy * s);
    let contains = |m: &MonitorInfo| px >= m.x as f32 && px < (m.x + m.width_px as i32) as f32 && py >= m.y as f32 && py < (m.y + m.height_px as i32) as f32;
    let dist = |m: &MonitorInfo| {
        let dx = (m.x as f32 - px).max(0.0).max(px - (m.x + m.width_px as i32) as f32);
        let dy = (m.y as f32 - py).max(0.0).max(py - (m.y + m.height_px as i32) as f32);
        dx * dx + dy * dy
    };
    let target = monitors.iter().find(|m| contains(m)).or_else(|| monitors.iter().min_by(|a, b| dist(a).partial_cmp(&dist(b)).unwrap()))?;
    let t = target.scale();
    let (vx, vy) = (from.x as f32 + rect.x * s, from.y as f32 + rect.y * s);
    Some((target, Rect { x: (vx - target.x as f32) / t, y: (vy - target.y as f32) / t, w: rect.w, h: rect.h }))
}

/// The factor `reconcile` shrinks `p` by on `target` (1.0 when it fits), so a drag can use the widget's real on-screen size there.
pub fn scale_on(p: &Placement, target: &MonitorInfo) -> f32 {
    let (_, _, ww, wh) = target.work_dip();
    if p.w > ww || p.h > wh {
        (ww / p.w.max(1.0)).min(wh / p.h.max(1.0)).max(crate::monitors::MIN_SCALE)
    } else {
        1.0
    }
}

/// The stored placement for a widget the user dropped at `rect` (relative to `target`'s origin, work area clamp already applied).
///
/// * The nearest corner of the work area becomes the anchor, so the widget keeps its distance from that corner when the monitor
///   changes resolution (D26).
/// * `scale` is the factor `reconcile` shrank the widget by (1.0 normally); offsets are stored unscaled so they reproduce the
///   same place, and the saved size stays the user's size, never the shrunken one.
/// * `home` is the monitor id, except that a widget already homed on the role `"primary"` stays on it when dropped on the primary.
pub fn placed(old: &Placement, target: &MonitorInfo, rect: Rect, scale: f32) -> Placement {
    let (wx, wy, ww, wh) = target.work_dip();
    let scale = if scale.is_finite() && scale > 0.0 { scale } else { 1.0 };
    let (w, h) = (old.w * scale, old.h * scale);
    let (cx, cy) = (rect.x + w / 2.0 - wx, rect.y + h / 2.0 - wy);
    let left = cx < ww / 2.0;
    let top = cy < wh / 2.0;
    let anchor = match (left, top) {
        (true, true) => Anchor::TopLeft,
        (false, true) => Anchor::TopRight,
        (true, false) => Anchor::BottomLeft,
        (false, false) => Anchor::BottomRight,
    };
    let (dx, dy) = match anchor {
        Anchor::TopLeft => (rect.x - wx, rect.y - wy),
        Anchor::TopRight => (wx + ww - w - rect.x, rect.y - wy),
        Anchor::BottomLeft => (rect.x - wx, wy + wh - h - rect.y),
        Anchor::BottomRight => (wx + ww - w - rect.x, wy + wh - h - rect.y),
        Anchor::Center => (0.0, 0.0),
    };
    let keep_primary_role = old.home == ROLE_PRIMARY && target.primary;
    Placement {
        widget: old.widget.clone(),
        home: if keep_primary_role { ROLE_PRIMARY.to_string() } else { target.id.clone() },
        home_fingerprint: target.fingerprint.clone(),
        anchor,
        dx: dx / scale,
        dy: dy / scale,
        w: old.w,
        h: old.h,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::monitors::{default_placement, reconcile};

    fn mon(id: &str, x: i32, y: i32, w: u32, h: u32, dpi: u32, primary: bool) -> MonitorInfo {
        MonitorInfo { id: id.into(), fingerprint: format!("fp-{id}"), name: id.into(), x, y, width_px: w, height_px: h, work: (x, y, w, h - (40.0 * dpi as f32 / 96.0) as u32), dpi, primary, refresh_hz: None }
    }
    /// This PC: 1920x1080 primary and a 1280x400 case screen above-left at negative coordinates.
    fn pc() -> Vec<MonitorInfo> {
        vec![mon("main", 0, 0, 1920, 1080, 96, true), mon("case", -1280, -400, 1280, 400, 96, false)]
    }
    fn work(m: &MonitorInfo) -> Rect {
        let (x, y, w, h) = m.work_dip();
        Rect::new(x, y, w, h)
    }
    const GRID: SnapConfig = SnapConfig { mode: SnapMode::Grid, grid: 24.0 };
    const EDGES: SnapConfig = SnapConfig { mode: SnapMode::Edges, grid: 24.0 };

    #[test]
    fn grid_snap_rounds_to_the_grid_from_the_work_corner() {
        let w = work(&pc()[0]);
        let s = snap(Rect::new(100.0, 61.0, 200.0, 100.0), w, &[], GRID);
        assert_eq!((s.rect.x, s.rect.y), (96.0, 72.0));
        assert_eq!((s.guide_x, s.guide_y), (None, None));
    }

    #[test]
    fn off_changes_nothing_except_keeping_the_widget_on_screen() {
        let w = work(&pc()[0]);
        let off = SnapConfig { mode: SnapMode::Off, grid: 24.0 };
        assert_eq!(snap(Rect::new(101.3, 62.7, 200.0, 100.0), w, &[], off).rect, Rect::new(101.3, 62.7, 200.0, 100.0));
        let s = snap(Rect::new(-50.0, 9000.0, 200.0, 100.0), w, &[], off);
        assert_eq!((s.rect.x, s.rect.y), (0.0, w.bottom() - 100.0));
    }

    #[test]
    fn edges_pull_to_work_area_edges_and_center() {
        let w = work(&pc()[0]);
        let s = snap(Rect::new(6.0, 4.0, 300.0, 100.0), w, &[], EDGES);
        assert_eq!((s.rect.x, s.rect.y, s.guide_x, s.guide_y), (0.0, 0.0, Some(0.0), Some(0.0)));
        let s = snap(Rect::new(w.w - 300.0 - 7.0, 400.0, 300.0, 100.0), w, &[], EDGES);
        assert_eq!(s.rect.right(), w.right());
        let s = snap(Rect::new(w.w / 2.0 - 150.0 + 6.0, 400.0, 300.0, 100.0), w, &[], EDGES);
        assert!((s.rect.center().0 - (w.x + w.w / 2.0)).abs() < 0.01);
    }

    #[test]
    fn edges_pull_to_other_widgets_and_far_edges_do_not() {
        let w = work(&pc()[0]);
        let other = Rect::new(400.0, 200.0, 300.0, 100.0);
        let s = snap(Rect::new(706.0, 195.0, 200.0, 100.0), w, &[other], EDGES); // right of the other, 6 away; top 5 off
        assert_eq!((s.rect.x, s.rect.y), (700.0, 200.0));
        let s = snap(Rect::new(780.0, 270.0, 200.0, 100.0), w, &[other], EDGES); // nothing within the distance
        assert_eq!((s.rect.x, s.rect.y, s.guide_x, s.guide_y), (780.0, 270.0, None, None));
    }

    #[test]
    fn both_prefers_an_edge_and_falls_back_to_the_grid() {
        let w = work(&pc()[0]);
        let both = SnapConfig { mode: SnapMode::Both, grid: 24.0 };
        let s = snap(Rect::new(5.0, 61.0, 200.0, 100.0), w, &[], both);
        assert_eq!((s.rect.x, s.rect.y, s.guide_x, s.guide_y), (0.0, 72.0, Some(0.0), None));
    }

    #[test]
    fn snapping_never_leaves_the_work_area_and_is_idempotent() {
        let w = work(&pc()[0]);
        let others = [Rect::new(300.0, 300.0, 100.0, 100.0)];
        for cfg in [GRID, EDGES, SnapConfig::default()] {
            for (x, y) in [(-500.0, -500.0), (5000.0, 5000.0), (123.4, 456.7), (w.w - 1.0, w.h - 1.0)] {
                let a = snap(Rect::new(x, y, 360.0, 118.0), w, &others, cfg);
                assert!(a.rect.x >= w.x - 0.001 && a.rect.right() <= w.right() + 0.001 && a.rect.y >= w.y - 0.001 && a.rect.bottom() <= w.bottom() + 0.001, "{a:?}");
                let b = snap(a.rect, w, &others, cfg);
                assert_eq!(a.rect, b.rect, "snapping twice must give the same place");
            }
        }
    }

    #[test]
    fn a_widget_bigger_than_the_work_area_stays_at_its_corner() {
        let w = work(&pc()[1]);
        let s = snap(Rect::new(50.0, 50.0, 2000.0, 900.0), w, &[], SnapConfig::default());
        assert_eq!((s.rect.x, s.rect.y), (w.x, w.y));
    }

    #[test]
    fn config_from_untrusted_input_is_sanitized() {
        assert_eq!(SnapConfig::sanitized("nonsense", 24.0).mode, SnapMode::Both);
        assert_eq!(SnapConfig::sanitized("grid", f32::NAN).grid, DEFAULT_GRID);
        assert_eq!(SnapConfig::sanitized("grid", 0.0).grid, MIN_GRID);
        assert_eq!(SnapConfig::sanitized("grid", 1e9).grid, MAX_GRID);
        for m in [SnapMode::Off, SnapMode::Grid, SnapMode::Edges, SnapMode::Both] {
            assert_eq!(SnapMode::parse(m.as_str()), Some(m));
        }
    }

    #[test]
    fn placed_then_reconcile_gives_back_the_dropped_rectangle() {
        let mons = pc();
        for m in &mons {
            let w = work(m);
            for (x, y) in [(10.0, 10.0), (w.w - 400.0 - 30.0, 20.0), (40.0, w.h - 190.0 - 25.0), (w.w - 400.0 - 100.0, w.h - 190.0 - 100.0), (w.w / 2.0 - 200.0, w.h / 2.0 - 95.0)] {
                let old = default_placement("com.plinth.buildcard", 0);
                let off = SnapConfig { mode: SnapMode::Off, grid: 24.0 };
                let r = snap(Rect::new(x, y, 400.0, 190.0), w, &[], off);
                if old.w > w.w || old.h > w.h {
                    continue; // does not fit this (small) monitor: covered by the scaling test
                }
                let p = placed(&old, m, r.rect, 1.0);
                let back = reconcile(&[p], &mons, None);
                assert_eq!(back[0].monitor, m.id);
                assert!((back[0].x - r.rect.x).abs() < 0.01 && (back[0].y - r.rect.y).abs() < 0.01, "{:?} vs {:?}", back[0], r.rect);
            }
        }
    }

    #[test]
    fn the_nearest_corner_becomes_the_anchor() {
        let m = &pc()[0];
        let old = default_placement("com.plinth.clock", 0);
        let w = work(m);
        assert_eq!(placed(&old, m, Rect::new(10.0, 10.0, 360.0, 118.0), 1.0).anchor, Anchor::TopLeft);
        assert_eq!(placed(&old, m, Rect::new(w.w - 370.0, 10.0, 360.0, 118.0), 1.0).anchor, Anchor::TopRight);
        assert_eq!(placed(&old, m, Rect::new(10.0, w.h - 130.0, 360.0, 118.0), 1.0).anchor, Anchor::BottomLeft);
        assert_eq!(placed(&old, m, Rect::new(w.w - 370.0, w.h - 130.0, 360.0, 118.0), 1.0).anchor, Anchor::BottomRight);
    }

    #[test]
    fn a_widget_dropped_at_the_bottom_right_keeps_its_distance_when_the_monitor_changes_size() {
        let a = mon("a", 0, 0, 1920, 1080, 96, true);
        let old = default_placement("com.plinth.buildcard", 0);
        let (wx, wy, ww, wh) = a.work_dip();
        let p = placed(&old, &a, Rect::new(wx + ww - 400.0 - 50.0, wy + wh - 190.0 - 60.0, 400.0, 190.0), 1.0);
        assert_eq!((p.anchor, p.dx, p.dy), (Anchor::BottomRight, 50.0, 60.0));
        let b = mon("a", 0, 0, 2560, 1440, 96, true);
        let r = reconcile(&[p], &[b.clone()], None);
        let (bx, by, bw, bh) = b.work_dip();
        assert!((bx + bw - r[0].w - r[0].x - 50.0).abs() < 0.01 && (by + bh - r[0].h - r[0].y - 60.0).abs() < 0.01);
    }

    #[test]
    fn a_shrunken_widget_saves_its_real_size_and_unscaled_offsets() {
        let case = &pc()[1]; // 1280x400: a 700x500 widget is too tall and is shrunk
        let mut old = default_placement("com.plinth.pcvitals", 0);
        old.w = 700.0;
        old.h = 500.0;
        let resolved = reconcile(&[old.clone()], &[case.clone()], Some("case"));
        let r = &resolved[0];
        assert!(r.scale < 1.0);
        let p = placed(&old, case, Rect::new(r.x, r.y, r.w, r.h), r.scale);
        assert_eq!((p.w, p.h), (old.w, old.h), "the saved size is the user's size, not the shrunken one");
        let again = reconcile(&[p], &[case.clone()], Some("case"));
        assert!((again[0].x - r.x).abs() < 0.01 && (again[0].y - r.y).abs() < 0.01 && (again[0].scale - r.scale).abs() < 0.001);
    }

    #[test]
    fn dropping_on_another_monitor_changes_home_and_converts_through_pixels() {
        let mons = pc();
        let (main, case) = (&mons[0], &mons[1]);
        // a widget dragged from the main monitor's top-left corner to the left: its center lands on the case screen
        let (target, r) = retarget(&mons, main, Rect::new(-700.0, -300.0, 360.0, 118.0)).unwrap();
        assert_eq!(target.id, case.id);
        assert_eq!((r.x, r.y), (-700.0 + 1280.0, -300.0 + 400.0));
        let old = default_placement("com.plinth.clock", 0);
        let p = placed(&old, target, r, 1.0);
        assert_eq!(p.home, "case");
        assert_eq!(p.home_fingerprint, "fp-case");
    }

    #[test]
    fn retarget_between_monitors_with_different_scale_keeps_the_same_physical_spot() {
        let hi = mon("hi", 0, 0, 3840, 2160, 192, true); // 200%
        let lo = mon("lo", 3840, 0, 1920, 1080, 96, false);
        let mons = vec![hi.clone(), lo.clone()];
        // 100 DIP right of hi's right edge = 200 px => 200 px into lo => 200 DIP in lo
        let (t, r) = retarget(&mons, &hi, Rect::new(1920.0 + 100.0, 50.0, 100.0, 100.0)).unwrap();
        assert_eq!(t.id, "lo");
        assert!((r.x - 200.0).abs() < 0.01 && (r.y - 100.0).abs() < 0.01, "{r:?}");
        assert_eq!((r.w, r.h), (100.0, 100.0));
    }

    #[test]
    fn a_center_in_the_gap_goes_to_the_nearest_monitor_and_nothing_means_none() {
        let a = mon("a", 0, 0, 1000, 800, 96, true);
        let b = mon("b", 1200, 0, 1000, 800, 96, false);
        let mons = vec![a.clone(), b.clone()];
        assert_eq!(retarget(&mons, &a, Rect::new(1010.0, 100.0, 100.0, 100.0)).unwrap().0.id, "a");
        assert_eq!(retarget(&mons, &a, Rect::new(1090.0, 100.0, 100.0, 100.0)).unwrap().0.id, "b");
        assert!(retarget(&[], &a, Rect::new(0.0, 0.0, 10.0, 10.0)).is_none());
    }

    #[test]
    fn primary_role_is_kept_on_the_primary_and_dropped_on_another_monitor() {
        let mons = pc();
        let old = default_placement("com.plinth.clock", 0);
        assert_eq!(old.home, ROLE_PRIMARY);
        assert_eq!(placed(&old, &mons[0], Rect::new(50.0, 50.0, 360.0, 118.0), 1.0).home, ROLE_PRIMARY);
        assert_eq!(placed(&old, &mons[1], Rect::new(50.0, 50.0, 360.0, 118.0), 1.0).home, "case");
    }

    #[test]
    fn scale_on_matches_what_reconcile_does() {
        let mons = pc();
        let mut p = default_placement("com.plinth.pcvitals", 0);
        p.w = 700.0;
        p.h = 500.0;
        for m in &mons {
            let r = reconcile(&[p.clone()], &mons, Some(&m.id));
            let mut q = p.clone();
            q.home = m.id.clone();
            let r2 = reconcile(&[q.clone()], &mons, None);
            assert!((scale_on(&p, m) - r2[0].scale).abs() < 1e-6, "{} {:?} {:?}", m.id, r, r2);
        }
    }

    #[test]
    fn placed_is_pure_and_leaves_the_input_untouched() {
        let mons = pc();
        let old = default_placement("com.plinth.clock", 0);
        let copy = old.clone();
        let _ = placed(&old, &mons[1], Rect::new(10.0, 10.0, 360.0, 118.0), 0.8);
        assert_eq!(old, copy);
    }
}
