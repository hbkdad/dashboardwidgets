//! Monitor enumeration (Windows, raw FFI) and a fake-monitor provider for tests.
//!
//! Real path (verified against Microsoft Learn, research/monitors-and-wallpapers.md): `EnumDisplayMonitors` + `GetMonitorInfoW`
//! for rectangles, work area and primary flag; `GetDpiForMonitor` for scale; `GetDisplayConfigBufferSizes` +
//! `QueryDisplayConfig` + `DisplayConfigGetDeviceInfo` for the stable device path, EDID ids, friendly name and refresh rate.
//! If the display-config calls fail (no WDDM driver, a remote session) the monitor id falls back to the GDI device name.
//!
//! Fake path: `PLINTH_FAKE_MONITORS="1920x1080@0,0*;1280x1024@1920,0"` (the `*` marks the primary) lets the layer lifecycle,
//! unplug/replug and reconcile be exercised on a one-monitor PC. Fake monitors are shown as ordinary windows.

use plinth_core::monitors::MonitorInfo;
use std::collections::HashMap;

#[link(name = "user32")]
extern "system" {
    fn EnumDisplayMonitors(hdc: isize, clip: *const i32, cb: unsafe extern "system" fn(isize, isize, *mut [i32; 4], isize) -> i32, data: isize) -> i32;
    fn GetMonitorInfoW(hmon: isize, info: *mut u8) -> i32;
    fn GetDisplayConfigBufferSizes(flags: u32, paths: *mut u32, modes: *mut u32) -> i32;
    fn QueryDisplayConfig(flags: u32, paths: *mut u32, path_buf: *mut u8, modes: *mut u32, mode_buf: *mut u8, topology: *mut u32) -> i32;
    fn DisplayConfigGetDeviceInfo(packet: *mut u8) -> i32;
}
#[link(name = "shcore")]
extern "system" {
    fn GetDpiForMonitor(hmon: isize, kind: i32, x: *mut u32, y: *mut u32) -> i32;
}

const MONITORINFOF_PRIMARY: u32 = 1;
const QDC_ONLY_ACTIVE_PATHS: u32 = 2;
const PATH_SIZE: usize = 72;
const MODE_SIZE: usize = 64;

unsafe extern "system" fn collect(hmon: isize, _hdc: isize, _rect: *mut [i32; 4], data: isize) -> i32 {
    (*(data as *mut Vec<isize>)).push(hmon);
    1
}

fn u16s_to_string(units: &[u8]) -> String {
    let v: Vec<u16> = units.chunks_exact(2).map(|b| u16::from_le_bytes([b[0], b[1]])).take_while(|&c| c != 0).collect();
    String::from_utf16_lossy(&v)
}

fn rd32(b: &[u8], o: usize) -> u32 {
    u32::from_le_bytes(b[o..o + 4].try_into().unwrap())
}

fn ri32(b: &[u8], o: usize) -> i32 {
    i32::from_le_bytes(b[o..o + 4].try_into().unwrap())
}

#[derive(Default, Clone)]
struct Target {
    path: String,
    friendly: String,
    fingerprint: String,
    refresh: Option<u32>,
}

/// GDI device name (`\\.\DISPLAY1`) -> display-config details, for every active path.
fn display_config() -> HashMap<String, Target> {
    let mut out = HashMap::new();
    unsafe {
        let (mut np, mut nm) = (0u32, 0u32);
        if GetDisplayConfigBufferSizes(QDC_ONLY_ACTIVE_PATHS, &mut np, &mut nm) != 0 || np == 0 {
            return out;
        }
        let mut paths = vec![0u8; np as usize * PATH_SIZE];
        let mut modes = vec![0u8; nm as usize * MODE_SIZE];
        if QueryDisplayConfig(QDC_ONLY_ACTIVE_PATHS, &mut np, paths.as_mut_ptr(), &mut nm, modes.as_mut_ptr(), std::ptr::null_mut()) != 0 {
            return out;
        }
        for i in 0..np as usize {
            let p = &paths[i * PATH_SIZE..(i + 1) * PATH_SIZE];
            // source: adapterId (8) at 0, id at 8; target starts at 20: adapterId at 20, id at 28, refresh rate at 48/52
            let (src_adapter, src_id) = (p[0..8].to_vec(), rd32(p, 8));
            let (tgt_adapter, tgt_id) = (p[20..28].to_vec(), rd32(p, 28));
            let (num, den) = (rd32(p, 48), rd32(p, 52));
            // DISPLAYCONFIG_SOURCE_DEVICE_NAME: header(20) + viewGdiDeviceName[32] ; type 1
            let mut s = vec![0u8; 20 + 64];
            s[0..4].copy_from_slice(&1u32.to_le_bytes());
            let slen = s.len() as u32;
            s[4..8].copy_from_slice(&slen.to_le_bytes());
            s[8..16].copy_from_slice(&src_adapter);
            s[16..20].copy_from_slice(&src_id.to_le_bytes());
            if DisplayConfigGetDeviceInfo(s.as_mut_ptr()) != 0 {
                continue;
            }
            let gdi = u16s_to_string(&s[20..84]);
            // DISPLAYCONFIG_TARGET_DEVICE_NAME: header(20) flags(4) tech(4) manu u16 prod u16 connector u32 friendly[64] path[128]; type 2
            let mut t = vec![0u8; 20 + 4 + 4 + 2 + 2 + 4 + 128 + 256];
            t[0..4].copy_from_slice(&2u32.to_le_bytes());
            let tlen = t.len() as u32;
            t[4..8].copy_from_slice(&tlen.to_le_bytes());
            t[8..16].copy_from_slice(&tgt_adapter);
            t[16..20].copy_from_slice(&tgt_id.to_le_bytes());
            if DisplayConfigGetDeviceInfo(t.as_mut_ptr()) != 0 {
                continue;
            }
            let flags = rd32(&t, 20);
            let (manu, prod) = (u16::from_le_bytes([t[28], t[29]]), u16::from_le_bytes([t[30], t[31]]));
            let connector = rd32(&t, 32);
            let friendly = u16s_to_string(&t[36..164]);
            let path = u16s_to_string(&t[164..420]);
            let fingerprint = if flags & 4 != 0 { format!("{manu:04x}:{prod:04x}:{connector}") } else { String::new() };
            out.insert(gdi, Target { path, friendly, fingerprint, refresh: if den > 0 { Some((num as f64 / den as f64).round() as u32) } else { None } });
        }
    }
    out
}

pub fn enumerate_real() -> Vec<MonitorInfo> {
    let mut handles: Vec<isize> = Vec::new();
    unsafe { EnumDisplayMonitors(0, std::ptr::null(), collect, &mut handles as *mut Vec<isize> as isize) };
    let cfg = display_config();
    let mut out = Vec::new();
    for (n, h) in handles.iter().enumerate() {
        // MONITORINFOEXW: cbSize u32, rcMonitor[4], rcWork[4], dwFlags u32, szDevice[32] wchar = 104 bytes
        let mut info = vec![0u8; 104];
        info[0..4].copy_from_slice(&104u32.to_le_bytes());
        if unsafe { GetMonitorInfoW(*h, info.as_mut_ptr()) } == 0 {
            continue;
        }
        let (mx, my, mr, mb) = (ri32(&info, 4), ri32(&info, 8), ri32(&info, 12), ri32(&info, 16));
        let (wx, wy, wr, wb) = (ri32(&info, 20), ri32(&info, 24), ri32(&info, 28), ri32(&info, 32));
        let primary = rd32(&info, 36) & MONITORINFOF_PRIMARY != 0;
        let gdi = u16s_to_string(&info[40..104]);
        let (mut dx, mut dy) = (96u32, 96u32);
        if unsafe { GetDpiForMonitor(*h, 0, &mut dx, &mut dy) } != 0 {
            dx = 96;
        }
        let t = cfg.get(&gdi).cloned().unwrap_or_default();
        out.push(MonitorInfo {
            id: if t.path.is_empty() { gdi.clone() } else { t.path.clone() },
            fingerprint: if t.fingerprint.is_empty() { gdi.clone() } else { t.fingerprint },
            name: if t.friendly.trim().is_empty() { format!("Display {}", n + 1) } else { t.friendly.trim().to_string() },
            x: mx,
            y: my,
            width_px: (mr - mx).max(1) as u32,
            height_px: (mb - my).max(1) as u32,
            work: (wx, wy, (wr - wx).max(1) as u32, (wb - wy).max(1) as u32),
            dpi: dx.max(48),
            primary,
            refresh_hz: t.refresh,
        });
    }
    // Stable order: primary first, then left to right, top to bottom, so "Display 1/2/3" labels do not shuffle.
    out.sort_by_key(|m| (!m.primary, m.x, m.y));
    out
}

/// Parse `"1920x1080@0,0*;1280x1024@1920,0"`. A `*` suffix marks the primary; without one the first entry is primary.
pub fn parse_fake(spec: &str) -> Vec<MonitorInfo> {
    let mut out: Vec<MonitorInfo> = Vec::new();
    for (i, part) in spec.split(';').map(str::trim).filter(|p| !p.is_empty()).enumerate() {
        let (part, name) = match part.split_once('#') {
            Some((p, n)) => (p, n.trim().to_string()),
            None => (part, (i + 1).to_string()),
        };
        let primary = part.ends_with('*');
        let part = part.trim_end_matches('*');
        let Some((size, pos)) = part.split_once('@') else { continue };
        let Some((w, h)) = size.split_once('x') else { continue };
        let (x, y) = pos.split_once(',').unwrap_or((pos, "0"));
        let (Ok(w), Ok(h), Ok(x), Ok(y)) = (w.parse::<u32>(), h.parse::<u32>(), x.parse::<i32>(), y.parse::<i32>()) else { continue };
        if w == 0 || h == 0 || w > 16384 || h > 16384 {
            continue;
        }
        out.push(MonitorInfo {
            id: format!("FAKE#{name}"),
            fingerprint: format!("fake:{}x{}:{name}", w, h),
            name: format!("Fake display {name}"),
            x,
            y,
            width_px: w,
            height_px: h,
            work: (x, y, w, h.saturating_sub(40).max(1)),
            dpi: 96,
            primary,
            refresh_hz: Some(60),
        });
    }
    if !out.is_empty() && !out.iter().any(|m| m.primary) {
        out[0].primary = true;
    }
    out.sort_by_key(|m| (!m.primary, m.x, m.y));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fake_spec_parsing() {
        let named = parse_fake("1920x1080@0,0*#A;1280x1024@1920,0#B");
        assert_eq!((named[0].id.as_str(), named[1].id.as_str()), ("FAKE#A", "FAKE#B"));
        let m = parse_fake("1920x1080@0,0*;1280x1024@1920,0;bad;0x0@1,1");
        assert_eq!(m.len(), 2);
        assert!(m[0].primary && !m[1].primary);
        assert_eq!((m[1].x, m[1].width_px), (1920, 1280));
        let m = parse_fake("800x600@0,0;800x600@800,0");
        assert!(m[0].primary, "the first entry is primary when none is marked");
        assert_eq!(m[0].id, "FAKE#1");
        assert!(parse_fake("").is_empty());
    }

    #[test]
    fn real_enumeration_returns_a_primary_with_sane_numbers() {
        let m = enumerate_real();
        for x in &m {
            println!("MONITOR {x:?}");
        }
        assert!(!m.is_empty(), "this machine must have a display");
        assert_eq!(m.iter().filter(|x| x.primary).count(), 1);
        assert!(m.iter().all(|x| x.width_px > 100 && x.height_px > 100 && x.dpi >= 96 && !x.id.is_empty() && !x.name.is_empty()));
    }
}
