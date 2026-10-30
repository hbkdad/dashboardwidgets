//! Windows-only glue (raw FFI, no extra dependencies): desktop-layer attach and live system counters.
//! Attach technique validated in `spikes/layer-probe` and `spikes/webview-probe`: the host window becomes a child of
//! Progman, ordered directly behind the desktop-icons view (SHELLDLL_DefView). Only WebView2 content stays visible there
//! (GDI-painted content did not on Windows 11 build 26300).

use plinth_core::PermissionId;
use plinth_host::DataProvider;
use serde_json::{json, Value};
use std::ffi::c_void;
use std::ptr::null;
use std::sync::Mutex;

type HWND = isize;

#[link(name = "user32")]
extern "system" {
    fn FindWindowW(class: *const u16, title: *const u16) -> HWND;
    fn FindWindowExW(parent: HWND, after: HWND, class: *const u16, title: *const u16) -> HWND;
    fn SendMessageTimeoutW(h: HWND, msg: u32, w: usize, l: isize, flags: u32, timeout: u32, result: *mut usize) -> isize;
    fn SetParent(child: HWND, parent: HWND) -> HWND;
    fn SetWindowPos(h: HWND, after: HWND, x: i32, y: i32, cx: i32, cy: i32, flags: u32) -> i32;
    fn GetWindowLongPtrW(h: HWND, idx: i32) -> isize;
    fn SetWindowLongPtrW(h: HWND, idx: i32, v: isize) -> isize;
    fn ScreenToClient(h: HWND, p: *mut [i32; 2]) -> i32;
    fn keybd_event(vk: u8, scan: u8, flags: u32, extra: usize);
    fn GetDC(h: HWND) -> isize;
    fn ReleaseDC(h: HWND, dc: isize) -> i32;
    fn GetSystemMetrics(i: i32) -> i32;
    fn ShowWindow(h: HWND, cmd: i32) -> i32;
    fn SetForegroundWindow(h: HWND) -> i32;
}
#[link(name = "gdi32")]
extern "system" {
    fn GetPixel(dc: isize, x: i32, y: i32) -> u32;
}
#[link(name = "kernel32")]
extern "system" {
    fn GetSystemTimes(idle: *mut u64, kernel: *mut u64, user: *mut u64) -> i32;
    fn GlobalMemoryStatusEx(buf: *mut MemoryStatus) -> i32;
    fn CreateMutexW(attrs: *const c_void, initial_owner: i32, name: *const u16) -> isize;
    fn GetLastError() -> u32;
}

#[repr(C)]
struct MemoryStatus {
    length: u32,
    load: u32,
    total_phys: u64,
    avail_phys: u64,
    total_page: u64,
    avail_page: u64,
    total_virt: u64,
    avail_virt: u64,
    avail_ext: u64,
}

fn w(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

const GWL_STYLE: i32 = -16;
const WS_POPUP: isize = 0x8000_0000u32 as isize;
const WS_CHILD: isize = 0x4000_0000;
const WS_VISIBLE: isize = 0x1000_0000;
const SWP_NOACTIVATE: u32 = 0x10;
const SWP_SHOWWINDOW: u32 = 0x40;

/// Re-parents `hwnd` under the desktop (Progman), behind the icons, covering `(x, y, width, height)` in screen pixels.
/// Returns false if the desktop windows could not be found (caller falls back to a normal window).
pub fn attach_to_desktop(hwnd: isize, x: i32, y: i32, width: i32, height: i32) -> bool {
    unsafe {
        let progman = FindWindowW(w("Progman").as_ptr(), null());
        if progman == 0 {
            return false;
        }
        let mut res = 0usize;
        SendMessageTimeoutW(progman, 0x052C, 0xD, 1, 0, 1000, &mut res);
        let defview = FindWindowExW(progman, 0, w("SHELLDLL_DefView").as_ptr(), null());
        let style = GetWindowLongPtrW(hwnd, GWL_STYLE);
        SetWindowLongPtrW(hwnd, GWL_STYLE, (style & !WS_POPUP) | WS_CHILD | WS_VISIBLE);
        SetParent(hwnd, progman);
        let mut p = [x, y];
        ScreenToClient(progman, &mut p);
        SetWindowPos(hwnd, defview, p[0], p[1], width, height, SWP_NOACTIVATE | SWP_SHOWWINDOW) != 0
    }
}

/// Live CPU and memory numbers for the `system.cpu` and `system.ram` permissions.
pub struct SystemProvider {
    prev: Mutex<Option<(u64, u64, u64)>>,
}

impl SystemProvider {
    pub fn new() -> Self {
        SystemProvider { prev: Mutex::new(None) }
    }
}

impl DataProvider for SystemProvider {
    fn read(&self, perm: PermissionId) -> Option<Value> {
        match perm {
            PermissionId::SystemCpu => {
                let (mut idle, mut kernel, mut user) = (0u64, 0u64, 0u64);
                if unsafe { GetSystemTimes(&mut idle, &mut kernel, &mut user) } == 0 {
                    return None;
                }
                let mut prev = self.prev.lock().ok()?;
                let pct = match *prev {
                    Some((pi, pk, pu)) => {
                        let (di, dk, du) = (idle.saturating_sub(pi), kernel.saturating_sub(pk), user.saturating_sub(pu));
                        let total = dk + du; // kernel time includes idle time
                        if total == 0 { 0.0 } else { 100.0 * (total.saturating_sub(di)) as f64 / total as f64 }
                    }
                    None => 0.0,
                };
                *prev = Some((idle, kernel, user));
                Some(json!({ "percent": (pct * 10.0).round() / 10.0 }))
            }
            PermissionId::SystemRam => {
                let mut m = MemoryStatus { length: std::mem::size_of::<MemoryStatus>() as u32, load: 0, total_phys: 0, avail_phys: 0, total_page: 0, avail_page: 0, total_virt: 0, avail_virt: 0, avail_ext: 0 };
                if unsafe { GlobalMemoryStatusEx(&mut m as *mut MemoryStatus) } == 0 {
                    return None;
                }
                Some(json!({ "used_mb": (m.total_phys - m.avail_phys) / (1024 * 1024), "total_mb": m.total_phys / (1024 * 1024) }))
            }
            _ => None,
        }
    }
}

#[allow(dead_code)]
fn _unused(_: *const c_void) {}

/// Autotest helper: sends Win+D (Show Desktop toggle).
pub fn show_desktop() {
    unsafe {
        keybd_event(0x5B, 0, 0, 0);
        keybd_event(0x44, 0, 0, 0);
        keybd_event(0x44, 0, 2, 0);
        keybd_event(0x5B, 0, 2, 0);
    }
}

/// Autotest helper: true if the composed screen pixel at the center of the primary monitor equals `colorref` (0x00BBGGRR).
/// Reads what the user sees, not window flags (flags lied on this Windows build in the spikes).
pub fn probe_center_is(colorref: u32) -> bool {
    unsafe {
        let (w, h) = (GetSystemMetrics(0), GetSystemMetrics(1));
        let dc = GetDC(0);
        let hit = [(0, 0), (6, 0), (-6, 0), (0, 6), (0, -6)].iter().filter(|(dx, dy)| GetPixel(dc, w / 2 + dx, h / 2 + dy) == colorref).count();
        ReleaseDC(0, dc);
        hit >= 3
    }
}

/// Single instance: returns true if this is the first instance. The mutex handle is intentionally leaked for the process lifetime.
pub fn first_instance(name: &str) -> bool {
    unsafe {
        let h = CreateMutexW(null(), 0, w(name).as_ptr());
        h != 0 && GetLastError() != 183 // ERROR_ALREADY_EXISTS
    }
}

/// Brings the already-running instance's hub window to the front (it may be hidden in the tray).
pub fn activate_existing_hub() -> bool {
    unsafe {
        let h = FindWindowW(null(), w("Plinth").as_ptr());
        if h == 0 {
            return false;
        }
        ShowWindow(h, 5); // SW_SHOW
        ShowWindow(h, 9); // SW_RESTORE
        SetForegroundWindow(h);
        true
    }
}

/// 32x32 RGBA tray icon drawn in code (mint-to-blue rounded square with a dark center), no image assets needed.
pub fn tray_icon_rgba() -> Vec<u8> {
    let n = 32usize;
    let mut px = vec![0u8; n * n * 4];
    for y in 0..n {
        for x in 0..n {
            let (fx, fy) = (x as f32 - 15.5, y as f32 - 15.5);
            // rounded square mask
            let (ax, ay) = (fx.abs() - 10.5, fy.abs() - 10.5);
            let d = (ax.max(0.0).powi(2) + ay.max(0.0).powi(2)).sqrt() + ax.max(ay).min(0.0) - 4.0;
            if d > 0.5 {
                continue;
            }
            let t = (x + y) as f32 / (2.0 * n as f32);
            let (r, g, b) = (124.0 * (1.0 - t) + 91.0 * t, 242.0 * (1.0 - t) + 140.0 * t, 200.0 * (1.0 - t) + 255.0 * t);
            let inner = fx.abs() < 5.0 && fy.abs() < 5.0;
            let i = (y * n + x) * 4;
            let (r, g, b) = if inner { (11.0, 12.0, 15.0) } else { (r, g, b) };
            px[i] = r as u8;
            px[i + 1] = g as u8;
            px[i + 2] = b as u8;
            px[i + 3] = 255;
        }
    }
    px
}
