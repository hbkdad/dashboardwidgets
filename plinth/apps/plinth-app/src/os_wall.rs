//! The operating system's own wallpaper, per monitor, through `IDesktopWallpaper` (Windows 8+, desktop apps; decision D24).
//! Plinth composes a monitor-sized picture itself (fit mode and letterbox color included) and the OS only has to show it, so
//! the wallpaper keeps working when Plinth is closed. Monitors are matched to Plinth's list by their rectangle (`GetMonitorRECT`),
//! not by assuming the two device-path strings are equal (that is NOT verified, research/monitors-and-wallpapers.md).
//!
//! Every call runs on its own short-lived thread with its own COM apartment, so the UI thread never waits on the shell.

use windows::core::{w, PCWSTR, PWSTR};
use windows::Win32::Foundation::COLORREF;
use windows::Win32::System::Com::{CoCreateInstance, CoInitializeEx, CoTaskMemFree, CoUninitialize, CLSCTX_ALL, COINIT_APARTMENTTHREADED};
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::Shell::Common::COMDLG_FILTERSPEC;
use windows::Win32::UI::Shell::{DesktopWallpaper, FileOpenDialog, IDesktopWallpaper, IFileOpenDialog, DESKTOP_WALLPAPER_POSITION, DWPOS_FILL, FOS_FORCEFILESYSTEM, FOS_PICKFOLDERS, SIGDN_FILESYSPATH};

/// One monitor as the shell sees it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OsMonitor {
    pub id: String,
    /// left, top, width, height in virtual-screen pixels
    pub rect: (i32, i32, i32, i32),
}

/// What the desktop looked like before Plinth changed it, for Revert.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Snapshot {
    /// (shell monitor id, wallpaper path; empty if none)
    pub wallpapers: Vec<(String, String)>,
    pub position: i32,
    pub color: u32,
}

struct Com;
impl Com {
    fn new() -> Result<Com, String> {
        unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED).ok().map_err(|e| format!("COM init failed: {e}"))? };
        Ok(Com)
    }
}
impl Drop for Com {
    fn drop(&mut self) {
        unsafe { CoUninitialize() };
    }
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

unsafe fn take(p: PWSTR) -> String {
    if p.is_null() {
        return String::new();
    }
    let s = p.to_string().unwrap_or_default();
    CoTaskMemFree(Some(p.0 as *const _));
    s
}

fn desktop() -> Result<IDesktopWallpaper, String> {
    unsafe { CoCreateInstance(&DesktopWallpaper, None, CLSCTX_ALL).map_err(|e| format!("IDesktopWallpaper unavailable: {e}")) }
}

fn list_with(dw: &IDesktopWallpaper) -> Result<Vec<OsMonitor>, String> {
    unsafe {
        let n = dw.GetMonitorDevicePathCount().map_err(|e| e.to_string())?;
        let mut out = Vec::new();
        for i in 0..n {
            let id = take(dw.GetMonitorDevicePathAt(i).map_err(|e| e.to_string())?);
            let w = wide(&id);
            if let Ok(r) = dw.GetMonitorRECT(PCWSTR(w.as_ptr())) {
                out.push(OsMonitor { id, rect: (r.left, r.top, r.right - r.left, r.bottom - r.top) });
            }
        }
        Ok(out)
    }
}

pub fn list() -> Result<Vec<OsMonitor>, String> {
    let _c = Com::new()?;
    list_with(&desktop()?)
}

pub fn snapshot() -> Result<Snapshot, String> {
    let _c = Com::new()?;
    let dw = desktop()?;
    let mons = list_with(&dw)?;
    unsafe {
        let mut wallpapers = Vec::new();
        for m in &mons {
            let w = wide(&m.id);
            let path = dw.GetWallpaper(PCWSTR(w.as_ptr())).map(|p| take(p)).unwrap_or_default();
            wallpapers.push((m.id.clone(), path));
        }
        Ok(Snapshot { wallpapers, position: dw.GetPosition().map(|p| p.0).unwrap_or(DWPOS_FILL.0), color: dw.GetBackgroundColor().map(|c| c.0).unwrap_or(0) })
    }
}

/// Show `path` on the monitor whose rectangle is `rect`. The picture is already monitor-sized, so the OS fit mode is Fill.
pub fn set_for_rect(rect: (i32, i32, i32, i32), path: &str) -> Result<(), String> {
    let _c = Com::new()?;
    let dw = desktop()?;
    let mon = list_with(&dw)?.into_iter().find(|m| m.rect == rect).ok_or_else(|| "The shell does not list that monitor.".to_string())?;
    unsafe {
        dw.SetPosition(DWPOS_FILL).map_err(|e| e.to_string())?;
        let (m, p) = (wide(&mon.id), wide(path));
        dw.SetWallpaper(PCWSTR(m.as_ptr()), PCWSTR(p.as_ptr())).map_err(|e| format!("SetWallpaper failed: {e}"))
    }
}

pub fn restore(snap: &Snapshot) -> Result<(), String> {
    let _c = Com::new()?;
    let dw = desktop()?;
    let now = list_with(&dw)?;
    unsafe {
        let mut first_err = None;
        for (id, path) in &snap.wallpapers {
            if !now.iter().any(|m| &m.id == id) {
                continue; // that monitor is not connected any more
            }
            let (m, p) = (wide(id), wide(path));
            if let Err(e) = dw.SetWallpaper(PCWSTR(m.as_ptr()), PCWSTR(p.as_ptr())) {
                first_err.get_or_insert(e.to_string());
            }
        }
        let _ = dw.SetPosition(DESKTOP_WALLPAPER_POSITION(snap.position));
        let _ = dw.SetBackgroundColor(COLORREF(snap.color));
        first_err.map_or(Ok(()), Err)
    }
}

/// Native folder or image picker. Returns None when the user cancels. Runs on the calling thread (call it from a worker).
pub fn pick(folder: bool, owner: isize) -> Option<String> {
    let _c = Com::new().ok()?;
    unsafe {
        let d: IFileOpenDialog = CoCreateInstance(&FileOpenDialog, None, CLSCTX_ALL).ok()?;
        let mut o = d.GetOptions().ok()? | FOS_FORCEFILESYSTEM;
        if folder {
            o |= FOS_PICKFOLDERS;
        }
        d.SetOptions(o).ok()?;
        if !folder {
            let spec = [COMDLG_FILTERSPEC { pszName: w!("Images"), pszSpec: w!("*.jpg;*.jpeg;*.png;*.webp;*.bmp") }];
            d.SetFileTypes(&spec).ok()?;
        }
        d.Show(Some(HWND(owner as *mut _))).ok()?;
        let item = d.GetResult().ok()?;
        let p = item.GetDisplayName(SIGDN_FILESYSPATH).ok()?;
        Some(take(p))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_shell_lists_the_same_monitors_as_plinth_enumeration() {
        let os = list().expect("IDesktopWallpaper must work on this machine");
        let ours = crate::mon::enumerate_real();
        println!("OS monitors: {os:?}");
        assert_eq!(os.len(), ours.len());
        for m in &ours {
            assert!(os.iter().any(|o| o.rect == (m.x, m.y, m.width_px as i32, m.height_px as i32)), "no shell monitor matches {m:?}");
        }
    }

    /// Emergency tool: PLINTH_RESTORE_WALLPAPER=<path> sets every monitor back to one picture (Fill, black bars).
    #[test]
    #[ignore]
    fn restore_known_original() {
        let Ok(path) = std::env::var("PLINTH_RESTORE_WALLPAPER") else { return };
        let snap = Snapshot { wallpapers: list().unwrap().into_iter().map(|m| (m.id, path.clone())).collect(), position: 4, color: 0 };
        restore(&snap).unwrap();
        println!("RESTORED {:?}", snapshot().unwrap());
    }

    #[test]
    fn snapshot_reads_the_current_wallpapers() {
        let s = snapshot().expect("snapshot");
        println!("SNAPSHOT {s:?}");
        assert_eq!(s.wallpapers.len(), list().unwrap().len());
    }

    #[test]
    fn shell_monitor_ids_equal_display_config_device_paths() {
        // The join D25 would like to rely on: print and compare (a mismatch is reported, rect matching is used regardless).
        let os = list().unwrap();
        let ours = crate::mon::enumerate_real();
        for m in &ours {
            let o = os.iter().find(|o| o.rect == (m.x, m.y, m.width_px as i32, m.height_px as i32)).unwrap();
            println!("ours {:?}\nshell {:?}\nequal: {}", m.id, o.id, m.id.eq_ignore_ascii_case(&o.id));
        }
    }
}
