//! Impact Meter: an honest, visible cost for every widget and Scene. Wallpaper Engine users report battery drain and
//! frame drops with no way to see which item causes it (research/consumer-demand.md); Plinth shows the cost up front.
//! Numbers come from our own spikes (plinth/spikes/*/RESULTS.md) and are estimates until live counters replace them.

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Impact {
    pub ram_mb: f32,
    pub cpu_pct: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rating {
    Light,
    Moderate,
    Heavy,
}

/// Fixed cost of the full-screen desktop layer plus the app process, measured with plinth-app v0.7 (release, one machine,
/// 1080p): layer WebView2 tree 235-276 MB private + app 31 MB, about 2.1% of one core while the desktop is visible and
/// about 0.3% while other windows cover it. `scripts/measure-app.ps1` reproduces the numbers.
pub const HOST_FIXED: Impact = Impact { ram_mb: 270.0, cpu_pct: 2.0 };
/// Extra CPU of live motion (animated wallpaper) while the desktop is visible: 9.2% vs 2.1% of one core in the same run.
pub const MOTION_EXTRA_CPU_PCT: f32 = 7.0;
/// Extra memory while the hub window is open (its own WebView2 process tree, about 200 MB); released when the hub is hidden.
pub const HUB_OPEN_RAM_MB: f32 = 200.0;
/// Extra cost of one isolated third-party widget (Spike C: about +29 MB, +1 process).
pub const ISOLATED_WIDGET: Impact = Impact { ram_mb: 29.0, cpu_pct: 0.2 };
/// Extra cost of a bundled first-party widget sharing the trusted origin (Spike B: about 2.5 MB each).
pub const SHARED_WIDGET: Impact = Impact { ram_mb: 5.0, cpu_pct: 0.1 };

/// Extra cost of a live wallpaper while the desktop is visible, measured with `scripts/measure-wallpapers.ps1` (release build,
/// 1080p, one machine, 20 s per run, CPU of the app's process tree only, GPU load not measured). Against a static baseline of
/// 301.7 MB / 0.70%: Aurora +40 MB / +2.6%, Starfield +50 MB / +6.8%, Embers +145 MB / +7.2% (web wallpapers; a typical value is
/// used), a 1080p30 H.264 video +90 MB / +2.6%. When other windows cover the desktop the CPU cost is about 0 and the memory
/// cost mostly stays (video +30 MB, web about 0 because the page is paused by the browser engine).
pub const WEB_WALLPAPER_EXTRA: Impact = Impact { ram_mb: 100.0, cpu_pct: 5.0 };
pub const VIDEO_WALLPAPER_EXTRA: Impact = Impact { ram_mb: 90.0, cpu_pct: 2.6 };

pub fn wallpaper_extra(kind: crate::wallpaper::WallpaperKind) -> Impact {
    match kind {
        crate::wallpaper::WallpaperKind::Web => WEB_WALLPAPER_EXTRA,
        crate::wallpaper::WallpaperKind::Video => VIDEO_WALLPAPER_EXTRA,
    }
}

/// Extra cost of one more desktop layer window (one more monitor with widgets or a motion wallpaper), sharing the same WebView2
/// environment: measured on this PC with two real monitors (the second a 1280x400 case screen showing two widgets), release build,
/// single 20 s runs: 264.7 MB with one layer, 305.7 MB with two (+41 MB, +1 process), idle CPU 0.31% in both. A monitor with no
/// widgets and only a static picture needs no layer at all (the picture is set through the OS).
pub const EXTRA_LAYER: Impact = Impact { ram_mb: 41.0, cpu_pct: 0.1 };

pub fn total(parts: &[Impact]) -> Impact {
    parts.iter().fold(Impact { ram_mb: 0.0, cpu_pct: 0.0 }, |a, b| Impact { ram_mb: a.ram_mb + b.ram_mb, cpu_pct: a.cpu_pct + b.cpu_pct })
}

/// Rating of the incremental cost of an item (not counting the fixed host).
pub fn rate(i: &Impact) -> Rating {
    if i.ram_mb <= 40.0 && i.cpu_pct <= 0.5 {
        Rating::Light
    } else if i.ram_mb <= 120.0 && i.cpu_pct <= 2.0 {
        Rating::Moderate
    } else {
        Rating::Heavy
    }
}

/// Estimated cost of a Scene given how many widgets are bundled vs isolated, including the fixed host.
pub fn scene_cost(shared: usize, isolated: usize) -> Impact {
    let mut parts = vec![HOST_FIXED];
    parts.extend(std::iter::repeat(SHARED_WIDGET).take(shared));
    parts.extend(std::iter::repeat(ISOLATED_WIDGET).take(isolated));
    total(&parts)
}

/// Cost of a Scene with live motion on or off.
pub fn scene_cost_with_motion(shared: usize, isolated: usize, motion: bool) -> Impact {
    let mut c = scene_cost(shared, isolated);
    if motion {
        c.cpu_pct += MOTION_EXTRA_CPU_PCT;
    }
    c
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_extra_layer_is_cheap_next_to_the_first() {
        assert!(EXTRA_LAYER.ram_mb < HOST_FIXED.ram_mb / 4.0);
        assert_eq!(rate(&EXTRA_LAYER), Rating::Moderate);
    }

    #[test]
    fn wallpaper_costs_are_heavier_than_widgets() {
        use crate::wallpaper::WallpaperKind::*;
        assert_eq!(rate(&wallpaper_extra(Web)), Rating::Heavy);
        assert!(wallpaper_extra(Video).cpu_pct < wallpaper_extra(Web).cpu_pct);
    }

    #[test]
    fn ratings_follow_thresholds() {
        assert_eq!(rate(&SHARED_WIDGET), Rating::Light);
        assert_eq!(rate(&ISOLATED_WIDGET), Rating::Light);
        assert_eq!(rate(&Impact { ram_mb: 90.0, cpu_pct: 1.0 }), Rating::Moderate);
        assert_eq!(rate(&Impact { ram_mb: 30.0, cpu_pct: 3.0 }), Rating::Heavy);
        assert_eq!(rate(&Impact { ram_mb: 500.0, cpu_pct: 0.1 }), Rating::Heavy);
    }

    #[test]
    fn scene_cost_adds_host_and_widgets() {
        let c = scene_cost(3, 2);
        assert!((c.ram_mb - (HOST_FIXED.ram_mb + 3.0 * SHARED_WIDGET.ram_mb + 2.0 * ISOLATED_WIDGET.ram_mb)).abs() < 0.001);
        assert!(scene_cost(0, 0).ram_mb == HOST_FIXED.ram_mb);
        assert!(scene_cost(3, 8).ram_mb > scene_cost(3, 2).ram_mb);
    }

    #[test]
    fn first_party_widgets_in_the_shared_origin_are_cheaper_than_isolated_ones() {
        assert!(scene_cost(4, 0).ram_mb < scene_cost(0, 4).ram_mb, "shared origin must save memory");
        assert!(scene_cost(4, 0).ram_mb - scene_cost(0, 0).ram_mb < 25.0);
    }

    #[test]
    fn live_motion_adds_cpu_only() {
        let (a, b) = (scene_cost_with_motion(4, 0, false), scene_cost_with_motion(4, 0, true));
        assert!((b.cpu_pct - a.cpu_pct - MOTION_EXTRA_CPU_PCT).abs() < 0.001);
        assert_eq!(a.ram_mb, b.ram_mb);
    }
}
