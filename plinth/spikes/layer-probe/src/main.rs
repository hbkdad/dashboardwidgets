//! Spike A: can a Plinth host window live on the desktop layer and survive Show Desktop (Win+D)?
//!
//! Usage: layer-probe <workerw|bottom> [--hold SECONDS] [--no-matrix]
//!
//! It draws a 480x320 magenta window centered on the primary monitor, attaches it by the chosen
//! strategy, then (unless --no-matrix) sends Win+D, samples real screen pixels to see whether the
//! magenta is still visible, and sends Win+D again to restore. Pixel sampling reads the composed
//! screen, so it measures what the user sees, not just window flags.
//! Raw FFI on purpose: no dependency drift. Windows only.

#![allow(non_snake_case, clippy::upper_case_acronyms)]

use std::ffi::c_void;
use std::ptr::null;

type HWND = isize;

#[repr(C)]
#[derive(Default, Clone, Copy)]
struct RECT {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
}
#[repr(C)]
#[derive(Default, Clone, Copy)]
struct POINT {
    x: i32,
    y: i32,
}
#[repr(C)]
struct MSG {
    hwnd: HWND,
    message: u32,
    wParam: usize,
    lParam: isize,
    time: u32,
    pt: POINT,
}
#[repr(C)]
struct PAINTSTRUCT {
    hdc: isize,
    fErase: i32,
    rcPaint: RECT,
    fRestore: i32,
    fIncUpdate: i32,
    rgbReserved: [u8; 32],
}
#[repr(C)]
struct WNDCLASSW {
    style: u32,
    lpfnWndProc: extern "system" fn(HWND, u32, usize, isize) -> isize,
    cbClsExtra: i32,
    cbWndExtra: i32,
    hInstance: isize,
    hIcon: isize,
    hCursor: isize,
    hbrBackground: isize,
    lpszMenuName: *const u16,
    lpszClassName: *const u16,
}

#[link(name = "user32")]
extern "system" {
    fn FindWindowW(class: *const u16, title: *const u16) -> HWND;
    fn FindWindowExW(parent: HWND, after: HWND, class: *const u16, title: *const u16) -> HWND;
    fn SendMessageTimeoutW(h: HWND, msg: u32, w: usize, l: isize, flags: u32, timeout: u32, result: *mut usize) -> isize;
    fn EnumWindows(cb: extern "system" fn(HWND, isize) -> i32, l: isize) -> i32;
    fn GetClassNameW(h: HWND, buf: *mut u16, max: i32) -> i32;
    fn GetParent(h: HWND) -> HWND;
    fn GetWindow(h: HWND, cmd: u32) -> HWND;
    fn SetLayeredWindowAttributes(h: HWND, key: u32, alpha: u8, flags: u32) -> i32;
    fn RegisterClassW(c: *const WNDCLASSW) -> u16;
    fn CreateWindowExW(ex: u32, class: *const u16, title: *const u16, style: u32, x: i32, y: i32, w: i32, h: i32, parent: HWND, menu: isize, inst: isize, param: *const c_void) -> HWND;
    fn DefWindowProcW(h: HWND, m: u32, w: usize, l: isize) -> isize;
    fn ShowWindow(h: HWND, cmd: i32) -> i32;
    fn SetParent(child: HWND, parent: HWND) -> HWND;
    fn SetWindowPos(h: HWND, after: HWND, x: i32, y: i32, cx: i32, cy: i32, flags: u32) -> i32;
    fn GetWindowLongPtrW(h: HWND, idx: i32) -> isize;
    fn SetWindowLongPtrW(h: HWND, idx: i32, v: isize) -> isize;
    fn PeekMessageW(m: *mut MSG, h: HWND, min: u32, max: u32, remove: u32) -> i32;
    fn TranslateMessage(m: *const MSG) -> i32;
    fn DispatchMessageW(m: *const MSG) -> isize;
    fn GetSystemMetrics(i: i32) -> i32;
    fn IsWindowVisible(h: HWND) -> i32;
    fn GetWindowRect(h: HWND, r: *mut RECT) -> i32;
    fn ScreenToClient(h: HWND, p: *mut POINT) -> i32;
    fn SetProcessDPIAware() -> i32;
    fn BeginPaint(h: HWND, ps: *mut PAINTSTRUCT) -> isize;
    fn EndPaint(h: HWND, ps: *const PAINTSTRUCT) -> i32;
    fn FillRect(dc: isize, r: *const RECT, brush: isize) -> i32;
    fn GetClientRect(h: HWND, r: *mut RECT) -> i32;
    fn GetDC(h: HWND) -> isize;
    fn ReleaseDC(h: HWND, dc: isize) -> i32;
    fn DestroyWindow(h: HWND) -> i32;
    fn InvalidateRect(h: HWND, r: *const RECT, erase: i32) -> i32;
    fn UpdateWindow(h: HWND) -> i32;
    fn keybd_event(vk: u8, scan: u8, flags: u32, extra: usize);
}
#[link(name = "gdi32")]
extern "system" {
    fn CreateSolidBrush(color: u32) -> isize;
    fn GetPixel(dc: isize, x: i32, y: i32) -> u32;
}
#[link(name = "kernel32")]
extern "system" {
    fn GetModuleHandleW(name: *const u16) -> isize;
    fn Sleep(ms: u32);
}
#[link(name = "dwmapi")]
extern "system" {
    fn DwmGetWindowAttribute(h: HWND, attr: u32, out: *mut c_void, size: u32) -> i32;
}

const WM_PAINT: u32 = 0x000F;
const WM_ERASEBKGND: u32 = 0x0014;
const WS_POPUP: u32 = 0x8000_0000;
const WS_VISIBLE: u32 = 0x1000_0000;
const WS_CHILD: u32 = 0x4000_0000;
const WS_EX_TOOLWINDOW: u32 = 0x80;
const WS_EX_NOACTIVATE: u32 = 0x0800_0000;
const GWL_STYLE: i32 = -16;
const SWP_NOACTIVATE: u32 = 0x10;
const SWP_SHOWWINDOW: u32 = 0x40;
const HWND_BOTTOM: HWND = 1;
const SW_SHOWNOACTIVATE: i32 = 4;
const PM_REMOVE: u32 = 1;
const SM_CXSCREEN: i32 = 0;
const SM_CYSCREEN: i32 = 1;
const DWMWA_CLOAKED: u32 = 14;
// COLORREF is 0x00BBGGRR: magenta RGB(255,0,255)
const MAGENTA: u32 = 0x00FF_00FF;

fn w(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

fn class_of(h: HWND) -> String {
    if h == 0 {
        return "<none>".into();
    }
    let mut b = [0u16; 128];
    let n = unsafe { GetClassNameW(h, b.as_mut_ptr(), 128) } as usize;
    String::from_utf16_lossy(&b[..n])
}

extern "system" fn wndproc(h: HWND, m: u32, wp: usize, lp: isize) -> isize {
    unsafe {
        match m {
            WM_ERASEBKGND => 1,
            WM_PAINT => {
                let mut ps: PAINTSTRUCT = std::mem::zeroed();
                let dc = BeginPaint(h, &mut ps);
                let mut r = RECT::default();
                GetClientRect(h, &mut r);
                FillRect(dc, &r, CreateSolidBrush(MAGENTA));
                EndPaint(h, &ps);
                0
            }
            _ => DefWindowProcW(h, m, wp, lp),
        }
    }
}

fn pump(ms: u64) {
    let end = std::time::Instant::now() + std::time::Duration::from_millis(ms);
    while std::time::Instant::now() < end {
        unsafe {
            let mut m: MSG = std::mem::zeroed();
            while PeekMessageW(&mut m, 0, 0, 0, PM_REMOVE) != 0 {
                TranslateMessage(&m);
                DispatchMessageW(&m);
            }
            Sleep(10);
        }
    }
}

struct Find {
    defview_host: HWND,
}
extern "system" fn enum_cb(top: HWND, l: isize) -> i32 {
    unsafe {
        let f = &mut *(l as *mut Find);
        let dv = w("SHELLDLL_DefView");
        if FindWindowExW(top, 0, dv.as_ptr(), null()) != 0 {
            f.defview_host = top;
        }
    }
    1
}

/// Returns (target parent window, description). Tries the Win11 24H2 layout first.
fn find_worker(report: &mut Vec<String>) -> Option<HWND> {
    unsafe {
        let progman = FindWindowW(w("Progman").as_ptr(), null());
        report.push(format!("Progman hwnd={progman:#x}"));
        let mut res = 0usize;
        // Ask Progman to spawn the WorkerW behind the icons.
        SendMessageTimeoutW(progman, 0x052C, 0xD, 1, 0, 1000, &mut res);
        let wk = w("WorkerW");
        let child_worker = FindWindowExW(progman, 0, wk.as_ptr(), null());
        report.push(format!("WorkerW child of Progman (24H2-style layout): {child_worker:#x}"));
        let mut f = Find { defview_host: 0 };
        EnumWindows(enum_cb, &mut f as *mut Find as isize);
        report.push(format!(
            "SHELLDLL_DefView host: hwnd={:#x} class={}",
            f.defview_host,
            class_of(f.defview_host)
        ));
        if child_worker != 0 {
            return Some(child_worker);
        }
        // Classic layout: the WorkerW that is the next sibling after the DefView host.
        if f.defview_host != 0 && class_of(f.defview_host) == "WorkerW" {
            let sib = FindWindowExW(0, f.defview_host, wk.as_ptr(), null());
            report.push(format!("classic sibling WorkerW: {sib:#x}"));
            if sib != 0 {
                return Some(sib);
            }
        }
        None
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let strategy = args.get(1).map(String::as_str).unwrap_or("workerw");
    let hold: u64 = args.iter().position(|a| a == "--hold").and_then(|i| args.get(i + 1)).and_then(|s| s.parse().ok()).unwrap_or(0);
    let matrix = !args.iter().any(|a| a == "--no-matrix");

    unsafe {
        SetProcessDPIAware();
        let (sw, sh) = (GetSystemMetrics(SM_CXSCREEN), GetSystemMetrics(SM_CYSCREEN));
        let (ww, wh) = (480, 320);
        let (sx, sy) = ((sw - ww) / 2, (sh - wh) / 2);
        println!("strategy={strategy} primary={sw}x{sh} window@screen=({sx},{sy}) {ww}x{wh}");

        let inst = GetModuleHandleW(null());
        let cls = w("PlinthLayerProbe");
        let wc = WNDCLASSW { style: 0, lpfnWndProc: wndproc, cbClsExtra: 0, cbWndExtra: 0, hInstance: inst, hIcon: 0, hCursor: 0, hbrBackground: 0, lpszMenuName: null(), lpszClassName: cls.as_ptr() };
        RegisterClassW(&wc);
        let direct = strategy.ends_with("-direct");
        let (style0, parent0, x0, y0) = if direct {
            // Create as a real child of Progman from the start (no popup->child conversion).
            let progman = FindWindowW(w("Progman").as_ptr(), null());
            let mut p = POINT { x: sx, y: sy };
            ScreenToClient(progman, &mut p);
            (WS_CHILD | WS_VISIBLE, progman, p.x, p.y)
        } else {
            (WS_POPUP, 0, sx, sy)
        };
        let hwnd = CreateWindowExW(WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE, cls.as_ptr(), w("Plinth layer probe").as_ptr(), style0, x0, y0, ww, wh, parent0, 0, inst, null());
        if hwnd == 0 {
            println!("RESULT create_window=FAILED (not an interactive desktop session?)");
            std::process::exit(2);
        }

        match strategy {
            "workerw" => {
                let mut rep = Vec::new();
                let target = find_worker(&mut rep);
                for l in &rep {
                    println!("  diag: {l}");
                }
                let Some(t) = target else {
                    println!("RESULT attach=FAILED no WorkerW target found");
                    DestroyWindow(hwnd);
                    std::process::exit(3);
                };
                println!("  attach target={t:#x} class={}", class_of(t));
                let style = GetWindowLongPtrW(hwnd, GWL_STYLE);
                SetWindowLongPtrW(hwnd, GWL_STYLE, (style & !(WS_POPUP as isize)) | WS_CHILD as isize | WS_VISIBLE as isize);
                SetParent(hwnd, t);
                let mut p = POINT { x: sx, y: sy };
                ScreenToClient(t, &mut p);
                SetWindowPos(hwnd, 0, p.x, p.y, ww, wh, SWP_NOACTIVATE | SWP_SHOWWINDOW);
            }
            "progman" | "progman-top" | "progman-layered" => {
                // 24H2-style layout: children of Progman, top to bottom. Put us behind icons, above WorkerW.
                let progman = FindWindowW(w("Progman").as_ptr(), null());
                let mut c = GetWindow(progman, 5 /* GW_CHILD = topmost child */);
                let mut i = 0;
                while c != 0 && i < 20 {
                    let mut r = RECT::default();
                    GetWindowRect(c, &mut r);
                    println!("  progman child z#{i}: hwnd={c:#x} class={} visible={} rect=({},{},{},{})", class_of(c), IsWindowVisible(c), r.left, r.top, r.right, r.bottom);
                    c = GetWindow(c, 2 /* GW_HWNDNEXT */);
                    i += 1;
                }
                let defview = FindWindowExW(progman, 0, w("SHELLDLL_DefView").as_ptr(), null());
                println!("  defview child of progman: {defview:#x}");
                let style = GetWindowLongPtrW(hwnd, GWL_STYLE);
                SetWindowLongPtrW(hwnd, GWL_STYLE, (style & !(WS_POPUP as isize)) | WS_CHILD as isize | WS_VISIBLE as isize);
                SetParent(hwnd, progman);
                let mut p = POINT { x: sx, y: sy };
                ScreenToClient(progman, &mut p);
                let after = if strategy != "progman-top" && defview != 0 { defview } else { 0 };
                if strategy == "progman-layered" {
                    let ex = GetWindowLongPtrW(hwnd, -20);
                    SetWindowLongPtrW(hwnd, -20, ex | 0x0008_0000 /* WS_EX_LAYERED */);
                    SetLayeredWindowAttributes(hwnd, 0, 255, 2 /* LWA_ALPHA */);
                }
                SetWindowPos(hwnd, after, p.x, p.y, ww, wh, SWP_NOACTIVATE | SWP_SHOWWINDOW);
            }
            "progman-direct" => {
                let progman = FindWindowW(w("Progman").as_ptr(), null());
                let defview = FindWindowExW(progman, 0, w("SHELLDLL_DefView").as_ptr(), null());
                SetWindowPos(hwnd, defview, x0, y0, ww, wh, SWP_NOACTIVATE | SWP_SHOWWINDOW);
            }
            "topmost" => {
                // Calibration only: proves the pixel sampler can see our window at all.
                SetWindowPos(hwnd, -1, sx, sy, ww, wh, SWP_NOACTIVATE | SWP_SHOWWINDOW);
            }
            "bottom" => {
                SetWindowPos(hwnd, HWND_BOTTOM, sx, sy, ww, wh, SWP_NOACTIVATE | SWP_SHOWWINDOW);
            }
            other => {
                println!("unknown strategy {other}");
                std::process::exit(1);
            }
        }
        ShowWindow(hwnd, SW_SHOWNOACTIVATE);
        InvalidateRect(hwnd, null(), 1);
        UpdateWindow(hwnd);
        pump(800);
        println!("  parent class={} visible_flag={}", class_of(GetParent(hwnd)), IsWindowVisible(hwnd));

        let sample = |label: &str| -> (usize, usize) {
            let dc = GetDC(0);
            let mut hit = 0;
            let mut tot = 0;
            println!("  [{label}] center pixel COLORREF={:#08x} (probe={MAGENTA:#08x})", GetPixel(dc, sx + ww / 2, sy + wh / 2));
            for gx in 1..=3 {
                for gy in 1..=3 {
                    let x = sx + ww * gx / 4;
                    let y = sy + wh * gy / 4;
                    tot += 1;
                    if GetPixel(dc, x, y) == MAGENTA {
                        hit += 1;
                    }
                }
            }
            ReleaseDC(0, dc);
            let mut cloaked: u32 = 0;
            DwmGetWindowAttribute(hwnd, DWMWA_CLOAKED, &mut cloaked as *mut u32 as *mut c_void, 4);
            let mut r = RECT::default();
            GetWindowRect(hwnd, &mut r);
            println!("  [{label}] screen magenta pixels {hit}/{tot}  IsWindowVisible={} cloaked={cloaked} rect=({},{},{},{})", IsWindowVisible(hwnd), r.left, r.top, r.right, r.bottom);
            (hit, tot)
        };

        // Baseline can legitimately read low if the user's apps cover the center: informational only.
        sample("baseline (apps may cover it)");

        if matrix {
            let win_d = || {
                keybd_event(0x5B, 0, 0, 0);
                keybd_event(0x44, 0, 0, 0);
                keybd_event(0x44, 0, 2, 0);
                keybd_event(0x5B, 0, 2, 0);
            };
            println!("  sending Win+D (Show Desktop)...");
            win_d();
            pump(1800);
            let (hit, tot) = sample("after Win+D");
            let ok = hit * 2 >= tot;
            println!("RESULT show_desktop_visible={} ({hit}/{tot} sampled pixels are probe color)", if ok { "PASS" } else { "FAIL" });
            println!("  sending Win+D again (restore windows)...");
            win_d();
            pump(1800);
        }

        if hold > 0 {
            println!("  holding {hold}s for visual check");
            pump(hold * 1000);
        }
        DestroyWindow(hwnd);
        println!("done");
    }
}
