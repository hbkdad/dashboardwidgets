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

/// Fixed WebView2 host cost per monitor (Spike B: 146 MB private for a blank page, release build, one machine).
pub const HOST_FIXED: Impact = Impact { ram_mb: 146.0, cpu_pct: 0.15 };
/// Extra cost of one isolated third-party widget (Spike C: about +29 MB, +1 process).
pub const ISOLATED_WIDGET: Impact = Impact { ram_mb: 29.0, cpu_pct: 0.2 };
/// Extra cost of a bundled first-party widget sharing the trusted origin (Spike B: about 2.5 MB each).
pub const SHARED_WIDGET: Impact = Impact { ram_mb: 2.5, cpu_pct: 0.1 };

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

#[cfg(test)]
mod tests {
    use super::*;

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
        assert!((c.ram_mb - (146.0 + 3.0 * 2.5 + 2.0 * 29.0)).abs() < 0.001);
        assert!(scene_cost(0, 0).ram_mb == HOST_FIXED.ram_mb);
        assert!(scene_cost(3, 8).ram_mb > scene_cost(3, 2).ram_mb);
    }
}
