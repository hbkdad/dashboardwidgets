//! Windows-only glue (raw FFI, no extra dependencies): desktop-layer attach and live system counters.
//! Attach technique validated in `spikes/layer-probe` and `spikes/webview-probe`: the host window becomes a child of
//! Progman, ordered directly behind the desktop-icons view (SHELLDLL_DefView). Only WebView2 content stays visible there
//! (GDI-painted content did not on Windows 11 build 26300).

use std::ffi::c_void;
use std::ptr::null;

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
    fn GetForegroundWindow() -> HWND;
    fn GetWindowRect(h: HWND, r: *mut [i32; 4]) -> i32;
    fn GetClassNameW(h: HWND, buf: *mut u16, max: i32) -> i32;
    fn GetWindowThreadProcessId(h: HWND, pid: *mut u32) -> u32;
    fn IsWindowVisible(h: HWND) -> i32;
    fn ShowWindow(h: HWND, cmd: i32) -> i32;
    fn SetForegroundWindow(h: HWND) -> i32;
}
#[link(name = "gdi32")]
extern "system" {
    fn GetPixel(dc: isize, x: i32, y: i32) -> u32;
}
#[link(name = "kernel32")]
extern "system" {
    fn GetSystemPowerStatus(s: *mut [u8; 12]) -> i32;
    fn CreateMutexW(attrs: *const c_void, initial_owner: i32, name: *const u16) -> isize;
    fn GetLastError() -> u32;
    fn CreateEventW(attrs: *const c_void, manual_reset: i32, initial: i32, name: *const u16) -> isize;
    fn SetEvent(h: isize) -> i32;
    fn WaitForSingleObject(h: isize, ms: u32) -> u32;
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
const SWP_FRAMECHANGED: u32 = 0x20;
const GWL_EXSTYLE: i32 = -20;
const WS_CLIPSIBLINGS: isize = 0x0400_0000;
const WS_CLIPCHILDREN: isize = 0x0200_0000;
/// WS_EX_DLGMODALFRAME | WINDOWEDGE | CLIENTEDGE | STATICEDGE | APPWINDOW
const WS_EX_FRAME_BITS: isize = 0x0000_0001 | 0x0000_0100 | 0x0000_0200 | 0x0002_0000 | 0x0004_0000;

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
        // A plain child window with no frame at all. Keeping the original style bits left a caption/border behind (found on
        // 2026-10-30 by reading the real window rectangles: the client area was 1904x1071 on a 1920x1080 screen), which
        // inset the whole layer by about 8 px.
        let _ = WS_POPUP;
        SetWindowLongPtrW(hwnd, GWL_STYLE, WS_CHILD | WS_VISIBLE | WS_CLIPSIBLINGS | WS_CLIPCHILDREN);
        let ex = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
        SetWindowLongPtrW(hwnd, GWL_EXSTYLE, ex & !WS_EX_FRAME_BITS);
        SetParent(hwnd, progman);
        let mut p = [x, y];
        ScreenToClient(progman, &mut p);
        SetWindowPos(hwnd, defview, p[0], p[1], width, height, SWP_NOACTIVATE | SWP_SHOWWINDOW | SWP_FRAMECHANGED) != 0
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

/// A named event the second launch sets to tell the first instance "show your hub". Asking the running app to show its own window
/// is reliable; showing another process's hidden window from outside raced with the app's own window bookkeeping (seen in about
/// 1 of 10 click-test runs). Returns the event handle (0 if it could not be created).
pub fn show_event(instance: &str) -> isize {
    unsafe { CreateEventW(null(), 0, 0, w(&format!("{instance}-show")).as_ptr()) }
}

pub fn signal_show(instance: &str) -> bool {
    let h = show_event(instance);
    h != 0 && unsafe { SetEvent(h) } != 0
}

/// Waits up to `ms` for the show signal; true if it was set (auto-reset).
pub fn wait_show(h: isize, ms: u32) -> bool {
    h != 0 && unsafe { WaitForSingleObject(h, ms) } == 0
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

/// Reads the situation the pause governor needs (plinth-core `should_pause`): fullscreen app in front, battery, remote session.
/// A window counts as fullscreen when it covers the whole primary screen and is not the desktop or the taskbar.
/// `include_own` lets the autotest use its own window as the "game".
pub fn system_state(include_own: bool) -> plinth_core::SystemState {
    let mut st = plinth_core::SystemState::default();
    unsafe {
        let fg = GetForegroundWindow();
        if fg != 0 && IsWindowVisible(fg) != 0 {
            let mut pid = 0u32;
            GetWindowThreadProcessId(fg, &mut pid);
            let mut cls = [0u16; 64];
            let n = GetClassNameW(fg, cls.as_mut_ptr(), 64).max(0) as usize;
            let class = String::from_utf16_lossy(&cls[..n]);
            let mut r = [0i32; 4];
            GetWindowRect(fg, &mut r);
            let (sw, sh) = (GetSystemMetrics(0), GetSystemMetrics(1));
            let covers = r[0] <= 0 && r[1] <= 0 && r[2] >= sw && r[3] >= sh;
            let shell = matches!(class.as_str(), "Progman" | "WorkerW" | "Shell_TrayWnd" | "Shell_SecondaryTrayWnd");
            st.fullscreen_app = covers && !shell && (include_own || pid != std::process::id());
        }
        let mut p = [0u8; 12];
        if GetSystemPowerStatus(&mut p) != 0 {
            let (ac, pct) = (p[0], p[2]);
            if ac == 0 {
                st.on_battery = true;
            }
            st.battery_percent = if pct <= 100 { pct } else { 100 };
        }
        st.remote_session = GetSystemMetrics(0x1000) != 0;
    }
    st
}

/// True if the hub window (title "Plinth") exists and is visible. Lets the first instance notice that a second launch
/// re-showed its hidden hub, so it can rebuild the hub's web view deterministically.
pub fn hub_visible() -> bool {
    unsafe {
        let h = FindWindowW(null(), w("Plinth").as_ptr());
        h != 0 && IsWindowVisible(h) != 0
    }
}
