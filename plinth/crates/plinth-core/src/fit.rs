//! Wallpaper fit geometry (PRD W1): how a source image is painted onto one monitor's canvas for Fill, Fit, Stretch, Tile,
//! Span and Center (the same set the OS offers, `DESKTOP_WALLPAPER_POSITION` [S44]). Plinth composes the final bitmap per
//! monitor itself so each monitor can have its own mode and letterbox color; the OS then only has to show a monitor-sized
//! picture. Pure geometry here, pixels in the app.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FitMode {
    Fill,
    Fit,
    Stretch,
    Tile,
    Span,
    Center,
}

impl FitMode {
    pub fn parse(s: &str) -> Option<FitMode> {
        Some(match s {
            "fill" => FitMode::Fill,
            "fit" => FitMode::Fit,
            "stretch" => FitMode::Stretch,
            "tile" => FitMode::Tile,
            "span" => FitMode::Span,
            "center" => FitMode::Center,
            _ => return None,
        })
    }

    pub fn as_str(self) -> &'static str {
        match self {
            FitMode::Fill => "fill",
            FitMode::Fit => "fit",
            FitMode::Stretch => "stretch",
            FitMode::Tile => "tile",
            FitMode::Span => "span",
            FitMode::Center => "center",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RectF {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

/// Paint `src` (a rectangle of the source image, in source pixels) into `dst` (a rectangle of the canvas, in canvas pixels).
/// Everything outside `dst` is the letterbox color. When `tile` is true the whole source repeats from the canvas origin at
/// its natural size and the other fields are the full source and full canvas.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Plan {
    pub src: RectF,
    pub dst: RectF,
    pub tile: bool,
}

/// `canvas` is the monitor size in pixels. For [`FitMode::Span`], `span_offset` is the monitor's top-left inside the bounding
/// box of all monitors and `span_total` is that bounding box's size, both in pixels; other modes ignore them.
pub fn plan(mode: FitMode, src: (u32, u32), canvas: (u32, u32), span_offset: (i32, i32), span_total: (u32, u32)) -> Plan {
    let (sw, sh) = (src.0.max(1) as f64, src.1.max(1) as f64);
    let (cw, ch) = (canvas.0.max(1) as f64, canvas.1.max(1) as f64);
    let full_src = RectF { x: 0.0, y: 0.0, w: sw, h: sh };
    let full_dst = RectF { x: 0.0, y: 0.0, w: cw, h: ch };
    match mode {
        FitMode::Stretch => Plan { src: full_src, dst: full_dst, tile: false },
        FitMode::Tile => Plan { src: full_src, dst: full_dst, tile: true },
        FitMode::Fill => {
            let s = (cw / sw).max(ch / sh);
            let (vw, vh) = (cw / s, ch / s);
            Plan { src: RectF { x: (sw - vw) / 2.0, y: (sh - vh) / 2.0, w: vw, h: vh }, dst: full_dst, tile: false }
        }
        FitMode::Fit => {
            let s = (cw / sw).min(ch / sh);
            let (dw, dh) = (sw * s, sh * s);
            Plan { src: full_src, dst: RectF { x: (cw - dw) / 2.0, y: (ch - dh) / 2.0, w: dw, h: dh }, tile: false }
        }
        FitMode::Center => {
            // Natural size, centered; the part that does not fit is cropped, the rest is letterbox.
            let (vw, vh) = (sw.min(cw), sh.min(ch));
            Plan { src: RectF { x: (sw - vw) / 2.0, y: (sh - vh) / 2.0, w: vw, h: vh }, dst: RectF { x: (cw - vw) / 2.0, y: (ch - vh) / 2.0, w: vw, h: vh }, tile: false }
        }
        FitMode::Span => {
            let (tw, th) = (span_total.0.max(1) as f64, span_total.1.max(1) as f64);
            let s = (tw / sw).max(th / sh); // Fill the whole bounding box...
            let (ox, oy) = ((sw * s - tw) / 2.0, (sh * s - th) / 2.0);
            // ...then this monitor shows its own window of the scaled image.
            let (wx, wy) = (span_offset.0 as f64 + ox, span_offset.1 as f64 + oy);
            Plan { src: RectF { x: wx / s, y: wy / s, w: cw / s, h: ch / s }, dst: full_dst, tile: false }
        }
    }
}

/// Parse `#rrggbb` into (r, g, b). Anything else is rejected.
pub fn parse_color(s: &str) -> Option<(u8, u8, u8)> {
    let h = s.strip_prefix('#')?;
    if h.len() != 6 || !h.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let v = u32::from_str_radix(h, 16).ok()?;
    Some(((v >> 16) as u8, (v >> 8) as u8, v as u8))
}

#[cfg(test)]
mod tests {
    use super::*;

    const Z: (i32, i32) = (0, 0);
    const T: (u32, u32) = (1, 1);

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-6
    }

    #[test]
    fn fill_same_aspect_is_the_whole_image() {
        let p = plan(FitMode::Fill, (3840, 2160), (1920, 1080), Z, T);
        assert!(close(p.src.w, 3840.0) && close(p.src.h, 2160.0) && close(p.src.x, 0.0));
        assert_eq!((p.dst.w, p.dst.h), (1920.0, 1080.0));
    }

    #[test]
    fn fill_crops_the_longer_side_and_centers() {
        // 4:3 photo on a 16:9 monitor: full width, cropped height
        let p = plan(FitMode::Fill, (1600, 1200), (1920, 1080), Z, T);
        assert!(close(p.src.w, 1600.0));
        assert!(close(p.src.h, 1600.0 * 1080.0 / 1920.0));
        assert!(close(p.src.y, (1200.0 - p.src.h) / 2.0));
        // portrait photo on landscape monitor
        let q = plan(FitMode::Fill, (1000, 2000), (1920, 1080), Z, T);
        assert!(close(q.src.w, 1000.0) && q.src.h < 2000.0);
    }

    #[test]
    fn fit_letterboxes_and_never_crops() {
        let p = plan(FitMode::Fit, (1600, 1200), (1920, 1080), Z, T);
        assert_eq!((p.src.w, p.src.h), (1600.0, 1200.0));
        assert!(close(p.dst.h, 1080.0));
        assert!(close(p.dst.w, 1440.0));
        assert!(close(p.dst.x, 240.0) && close(p.dst.y, 0.0));
        let wide = plan(FitMode::Fit, (4000, 1000), (1920, 1080), Z, T);
        assert!(close(wide.dst.w, 1920.0) && close(wide.dst.h, 480.0) && close(wide.dst.y, 300.0));
    }

    #[test]
    fn stretch_and_tile_use_everything() {
        let s = plan(FitMode::Stretch, (100, 50), (1920, 1080), Z, T);
        assert_eq!((s.dst.w, s.dst.h, s.tile), (1920.0, 1080.0, false));
        let t = plan(FitMode::Tile, (100, 50), (1920, 1080), Z, T);
        assert!(t.tile);
        assert_eq!((t.src.w, t.src.h), (100.0, 50.0));
    }

    #[test]
    fn center_small_image_is_boxed_and_large_is_cropped() {
        let small = plan(FitMode::Center, (800, 600), (1920, 1080), Z, T);
        assert_eq!((small.dst.x, small.dst.y, small.dst.w, small.dst.h), (560.0, 240.0, 800.0, 600.0));
        let big = plan(FitMode::Center, (4000, 3000), (1920, 1080), Z, T);
        assert_eq!((big.dst.w, big.dst.h), (1920.0, 1080.0));
        assert!(close(big.src.x, 1040.0) && close(big.src.y, 960.0));
    }

    #[test]
    fn span_pieces_are_adjacent_and_cover_the_image() {
        // two 1920x1080 monitors side by side: bounding box 3840x1080, a 4:1 panorama
        let src = (8000, 2000);
        let left = plan(FitMode::Span, src, (1920, 1080), (0, 0), (3840, 1080));
        let right = plan(FitMode::Span, src, (1920, 1080), (1920, 0), (3840, 1080));
        assert!(close(left.src.x + left.src.w, right.src.x), "no gap or overlap between the monitors");
        assert!(close(left.src.h, right.src.h) && close(left.src.y, right.src.y));
        assert!(left.src.x >= -1e-6 && right.src.x + right.src.w <= 8000.0 + 1e-6);
    }

    #[test]
    fn span_works_with_monitors_at_different_heights() {
        // 1920x1080 at (0,0) and a 1080x1920 portrait at (1920,-420): bounding box 3000 x 1920 starting at y=-420
        let total = (3000, 1920);
        let a = plan(FitMode::Span, (6000, 3840), (1920, 1080), (0, 420), total);
        let b = plan(FitMode::Span, (6000, 3840), (1080, 1920), (1920, 0), total);
        assert!(a.src.w > 0.0 && b.src.w > 0.0);
        assert!(close(a.src.x + a.src.w, b.src.x));
    }

    #[test]
    fn degenerate_sizes_do_not_panic_or_divide_by_zero() {
        for mode in [FitMode::Fill, FitMode::Fit, FitMode::Stretch, FitMode::Tile, FitMode::Span, FitMode::Center] {
            let p = plan(mode, (0, 0), (0, 0), Z, (0, 0));
            assert!(p.src.w.is_finite() && p.dst.w.is_finite());
        }
    }

    #[test]
    fn colors_and_names() {
        assert_eq!(parse_color("#102030"), Some((16, 32, 48)));
        for bad in ["102030", "#12345", "#12345g", "#1234567", "", "red"] {
            assert_eq!(parse_color(bad), None, "{bad}");
        }
        for m in ["fill", "fit", "stretch", "tile", "span", "center"] {
            assert_eq!(FitMode::parse(m).unwrap().as_str(), m);
        }
        assert_eq!(FitMode::parse("zoom"), None);
    }
}
