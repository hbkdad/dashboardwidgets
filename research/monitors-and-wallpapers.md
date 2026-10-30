# Wallpaper libraries and multi-monitor placement: evidence and verified Windows APIs (accessed 2026-10-30)

Question (owner addendum): are wallpaper management and per-monitor widget placement real gaps, and which Windows APIs make them safe to build? Labels: FACT (read in the cited source), CLAIM (interested party or user report), INFERENCE (mine), UNVERIFIED / NOT TESTED.

## 1. Are these gaps? (evidence is thin; read the labels)
| Pain | Evidence | Label |
|---|---|---|
| Rainmeter skins land on the wrong monitor or all pile onto the first one after a resolution change or monitor wake | Forum users report that when the main monitor's resolution changed, Rainmeter did not detect the second monitor in time, drew every skin on the first monitor, and autosave then wrote those bad positions into `rainmeter.ini`, overwriting the good ones; one user restores a backup of the ini [S52] | CLAIM (user report), one thread plus related threads; no official fix found |
| Rainmeter monitor numbering does not match Windows display order; `@1` is not necessarily the primary monitor | Forum thread [S52] | CLAIM |
| Community workaround: pin skins to the "@0 Virtual Monitor", or hand-edit `WindowX`/`WindowY`, or use a third-party layout-switcher skin that exists because Rainmeter has no built-in way to switch layouts on resolution change | Forum threads and a GitHub project [S52] | CLAIM |
| Windows' built-in per-monitor wallpaper control is awkward when several virtual desktops exist (Settings reportedly shows "Set for desktop" instead of "Set for monitor") | Several how-to sites [S53]; mostly low-quality SEO pages | CLAIM, weak sources; NOT TESTED by me |
| Wallpaper tools are separate from widget tools: Wallpaper Engine supports a different wallpaper per screen; DisplayFusion assigns wallpapers per monitor and can span one image; a forum user runs DisplayFusion for taskbars and Wallpaper Engine for backgrounds because they cannot be coordinated | [S53], [S54] | CLAIM; DisplayFusion's free-vs-Pro split for per-monitor wallpaper not established |
| Demand size for "per-monitor widgets" specifically | None found | UNVERIFIED |

INFERENCE: the gap is real but narrow and specific. It is not "nobody has per-monitor wallpapers" (several tools have them). It is (a) **placement that survives display changes** (the documented Rainmeter failure is a persistence bug, not a missing feature) and (b) **one place that handles wallpaper, widgets and monitors together** (today these are 2 to 3 apps). That matches gap row 2 ("one hub") and row 10 ("multi-monitor correctness") in `market-and-gaps.md`, and it is strongest for the PC-builder beachhead (D16) because builders own several monitors.

## 2. Windows APIs (verified against Microsoft Learn on 2026-10-30)
| Need | API | What the page says (FACT) | Notes |
|---|---|---|---|
| Per-monitor wallpaper (set, get) | `IDesktopWallpaper::SetWallpaper(monitorID, path)` [S42, S43] | `monitorID` comes from `GetMonitorDevicePathAt`; NULL sets the image on all monitors. Windows 8+, desktop apps | Also `GetWallpaper`, `GetPosition`, `GetBackgroundColor`, `Enable`, `GetMonitorRECT` [S42]. The wallpaper persists as an OS setting, so it survives Plinth exiting (INFERENCE from it being a shell setting) |
| Fit modes | `SetPosition` with `DESKTOP_WALLPAPER_POSITION` [S44] | CENTER=0, TILE=1, STRETCH=2, FIT=3, FILL=4, SPAN=5. FIT letterboxes using the background color; FILL crops; SPAN stretches one image across all monitors; TILE tiles across all monitors | Maps exactly to the requested Fill / Fit / Stretch / Tile / Span / Center. `SetBackgroundColor` gives the letterbox color. Position and background color look global, not per monitor in the interface (INFERENCE from the signatures: `SetPosition(position)` takes no monitor argument) so per-monitor fit modes need Plinth to compose the image itself |
| Native slideshow | `SetSlideshow(IShellItemArray)`, `SetSlideshowOptions`, `AdvanceSlideshow(monitorID)` [S45] | Items must all be in one container (files in one folder) or be a single folder item; any other mix fails | A multi-folder library cannot use the native slideshow. Plinth must run its own rotation timer and call `SetWallpaper` (INFERENCE) |
| Enumerate monitors, rectangles | `EnumDisplayMonitors` [S46] | With NULL hdc and NULL clip, enumerates all display monitors, callback receives HMONITOR and rectangle | Use `GetMonitorInfo` for work area and primary flag (not fetched; standard API, NOT re-verified this session) |
| Per-monitor DPI | `GetDpiForMonitor` [S47] | Returns different values depending on the caller's DPI awareness; "not DPI aware ... should not be used if the calling thread is per-monitor DPI aware" (use `GetDpiForWindow`); Windows 8.1+, Shcore | The process must be per-monitor aware (v2 on Windows 10 1703+) [S51] |
| DPI awareness mode | Per-Monitor v2 [S51] | v2: windows notified of DPI change, never bitmap-scaled, non-client area scaled; Windows 10 Creators Update (1703) | **Risk (INFERENCE from the table in [S51])**: `SetParent` across processes can "forcibly reset" a process' DPI awareness; Plinth attaches its layer to Progman with `SetParent`, so DPI behavior with mixed-DPI monitors must be tested on real hardware (NOT TESTED; one monitor here) |
| Friendly monitor name, stable path, EDID ids | `DisplayConfigGetDeviceInfo` + `DISPLAYCONFIG_TARGET_DEVICE_NAME` [S48, S49] | Fields: `monitorFriendlyDeviceName[64]`, `monitorDevicePath[128]`, `edidManufactureId`, `edidProductCodeId` (valid only when `edidIdsValid` flag set), `connectorInstance`, `outputTechnology`. Needs a WDDM driver; fails with ERROR_ACCESS_DENIED in a remote session. Windows 7+ for the struct | Friendly name can be empty if it cannot be read. Refresh rate and rotation come from `QueryDisplayConfig` (page not fetched; NOT verified this session) |
| Detect changes | `WM_DISPLAYCHANGE` [S50] | Sent to all top-level windows when the display resolution changes (wParam = bits per pixel, lParam = width/height) | The page says resolution; it does not say it fires on plug/unplug. Plinth should also listen for `WM_DPICHANGED` [S51] and `WM_DEVICECHANGE`/`WM_SETTINGCHANGE` and then re-enumerate (INFERENCE; the exact event set for hot-plug is NOT verified) |
| Identify monitors | Small borderless always-on-top window per monitor rectangle showing a number | Plinth-owned windows, no special API | Standard UI technique |

## 3. Decisions this research drives
D24 to D27 in `design/decisions.md`: static images set through `IDesktopWallpaper` and also drawn in the layer where widgets need glass; monitor identity by device path plus EDID fingerprint; placement stored per monitor in device-independent units with a pure reconcile function; one layer window per monitor that needs one, cost to be measured before promising.

## 4a. Results from the build (2026-10-30, this PC: two monitors, 1920x1080 primary and a 1280x400 case screen at -1280,679)
1. RESOLVED (FACT for this PC): `GetMonitorDevicePathAt` ids are identical to the DisplayConfig `monitorDevicePath` strings for both monitors (unit test).
2. WebP through the OS: not needed; Plinth decodes and composes, so WebP works (unit tests cover JPEG, PNG; WebP decode is enabled in the `image` crate).
6. MEASURED: a second layer window costs +41 MB and one process (264.7 to 305.7 MB), idle CPU unchanged.
Also found: the layer was inset 8 px on every side because of tao's undecorated-shadow border; fixed.

## 4. Still to verify before or during the build (do not skip)
1. That `GetMonitorDevicePathAt` ids equal the `monitorDevicePath` strings from `DisplayConfigGetDeviceInfo` (needed to join the two worlds). NOT TESTED.
2. Whether `SetWallpaper` accepts WebP; if not Plinth writes a cached JPEG/PNG. NOT TESTED.
3. Behavior with several virtual desktops: per-monitor `SetWallpaper`, and whether the Progman-attached layer shows on every virtual desktop. NOT TESTED.
4. Whether a transparent WebView2 layer can show the OS wallpaper beneath it (and whether `backdrop-filter` glass would then see nothing). NOT TESTED; the design avoids depending on it.
5. Monitor identity stability across dock/undock, port swaps and identical monitor models. NOT TESTED (needs 2+ real monitors).
6. Memory and CPU of a second layer window sharing one WebView2 environment. NOT TESTED; to be measured with a second layer window on this single monitor.
