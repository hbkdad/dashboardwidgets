# Architecture: Plinth

Targets and estimates here are **design targets, not measurements**. Every performance number must be validated with a prototype in week 1–2 (see §6).

## 1. Stack decision
**Recommended: Rust core service + Tauri 2 hub UI (Svelte + TypeScript) + WebView2-hosted widgets.**

| Option | Pros | Cons | Verdict |
|---|---|---|---|
| **A. Rust + Tauri 2 + WebView2** | Small binary, memory-safe core, web skills = huge widget-author pool, WebView2 ships with Win11 | WebView2 per-process RAM; Win-specific window-layer code needs `windows` crate | **Chosen** |
| B. Electron | Fastest dev, best docs | Ships its own Chromium (~100 MB+ idle RAM per app, INFERENCE), larger installer, weak story for "light on gaming PCs" | Rejected: performance is a headline claim |
| C. Native C++/WinUI 3 + Direct2D | Lowest footprint, best Win11 integration | Slow for a solo dev; tiny widget-author pool; building a no-code builder in native UI is expensive | Rejected for solo scope; revisit for renderer hot path |
| D. .NET WPF/WinUI + WebView2 | Familiar to Windows devs, easy shell APIs | Needs .NET runtime; GC; less sandbox control than Rust | Viable fallback if Rust learning curve blocks |

Rendering engine: **WebView2 (Chromium/DirectComposition)** for widgets and hub. Declarative widgets (§4) render via a small shared renderer, not a full app each, to keep RAM down.

## 2. System design

```
┌────────────────────── Plinth Hub (Tauri window) ──────────────────────┐
│ Svelte UI: Scenes, Library, Builder, Store, Settings                  │
└───────────────▲───────────────────────────────────────────────────────┘
                │ typed IPC (Tauri commands/events)
┌───────────────┴────────────── Core service (Rust, tray) ──────────────┐
│ Scene manager │ Package manager (verify/install/update) │ Governor   │
│ Window-layer manager │ Permission broker │ Data providers │ Settings │
└───────┬───────────────────────────────┬──────────────────────────────┘
        │ postMessage bridge            │ HTTPS
┌───────▼────────────┐          ┌───────▼─────────┐
│ Widget host(s)     │          │ Store API / CDN │
│ WebView2 per       │          │ (v1.0, static   │
│ monitor; each      │          │ index in MVP)   │
│ widget = sandboxed │          └─────────────────┘
│ iframe             │
└────────────────────┘
```
- **One widget-host layer window per monitor that needs one** (widgets or a motion wallpaper), sharing one WebView2 environment (D27; cost of the 2nd and 3rd layer NOT MEASURED). Monitors with only a static wallpaper need no layer (D24). Widgets are origin-isolated iframes (`plinth-widget://<id>`), each with CSP.
- **Core ↔ widget IPC:** only through the permission broker; widgets never touch Tauri IPC directly.
- **Settings/profile:** JSON files in `%APPDATA%\Plinth\`, export/import as `.plinthprofile` (zip). Cloud sync is post-v1.

## 3. Window-layer integration (G3)
- Primary technique: attach the host window beneath desktop icons using the WorkerW child technique (well-known community method). **Known risk:** its behavior has changed across Windows builds (Rainmeter itself needed a 24H2 fix for Win+D [S1]); keep it behind an abstraction with two fallbacks: (1) bottom-most non-activating tool window, (2) "Always behind" via `HWND_BOTTOM`.
- **Windows Insider CI:** nightly smoke test on Insider/Dev VM images; failing job opens an issue. A kill-switch config (remotely toggled static JSON) can switch the layer strategy without a release.
- Never hook Explorer, Start menu, taskbar or inject DLLs.

### 3.1 Post-audit additions
- **Test matrix gate (blocks MVP):** Win+D / Show Desktop, lock/unlock, sleep/resume, display hot-plug, DPI change, fast user switching, on Win11 23H2, 24H2, 25H2 and the current Insider build. Seelen UI's open issues (#1674 Win+D hides widgets; #1734 Win key after wake; #1874 panic on sleep) are the seed test cases.
- **Startup robustness:** no hard-coded WebView-readiness timeout; retry with backoff (cf. Seelen #1914).
- **One build channel:** single installer lineage (avoid Store+exe dual-install conflicts reported for Seelen UI [S39]).
- **Clean-room rule:** no code from AGPL projects.

### 3.2 Permission Center (core to the wedge)
- Core's permission broker records every grant, every data subscription, and every outbound request (host, bytes, timestamp) in a local ring-buffer log; the hub renders it per widget.
- Install flow: manifest permissions → plain-language sheet → user grants subset; widget must handle denial gracefully (API returns `denied`, never crashes).
- Revocation takes effect immediately (subscriptions cancelled, network proxy blocks).
- **Safe mode:** start with all third-party widgets disabled (also auto-triggered after two widget-host crashes in five minutes; cf. Rainmeter's Safe Start [S28]).
- **Honest limit:** sandbox strength depends on WebView2/Chromium site isolation; a Chromium escape would bypass it. State this in security docs. Verify with a hostile-widget test suite (file read, process spawn, undeclared host, cross-widget storage, CSP bypass) run in CI.

## 4. Theme / widget bundle format (`.plinth`)
Zip with a manifest. Semver `format` field; core supports N and N-1 formats.

```
mything.plinth
├─ manifest.json
├─ widgets/clock/{widget.json, main.html|declarative layout, assets/}
├─ scenes/ocean.scene.json
├─ tokens/dark.tokens.json
├─ preview.png
└─ SIGNATURE (detached, over manifest hash + file hashes)
```
```json
{
  "format": 1,
  "id": "com.example.ocean",
  "name": "Ocean",
  "version": "1.2.0",
  "type": "scene",
  "minPlinth": "1.0.0",
  "publisher": { "id": "pub_123", "name": "Example" },
  "contents": [{ "kind": "widget", "path": "widgets/clock" }],
  "permissions": [
    { "id": "system.cpu", "reason": "Show CPU load" },
    { "id": "net.fetch", "hosts": ["api.weather.example"], "reason": "Weather" }
  ],
  "license": { "type": "free" }
}
```
- **Backwards compatibility:** `format` bumps only for breaking changes; core ships migrators. Widgets declare `apiVersion`; the host exposes the matching shim.
- **Two widget kinds:** (1) *Declarative* (JSON layout + data bindings + small expression language; no code → builder output, cheapest to run, easily audited); (2) *Scripted* (HTML/JS in the sandbox, permissions required).

## 5. Plugin / extension API (JS, sandboxed)
```ts
plinth.data.subscribe('system.cpu', {intervalMs: 1000}, cb) // throttled by governor
plinth.net.fetch(url, opts)        // allowed hosts only
plinth.storage.get/set(key)        // per-widget quota
plinth.theme.tokens                // colors, radius, motion
plinth.lifecycle.on('pause'|'resume', cb)
```
Permissions (user-visible, per widget, revocable): `system.cpu|ram|gpu|disk|network`, `media.control`, `calendar.read` (OAuth via broker), `net.fetch(hosts)`, `storage`, `notifications`. **Not offered:** filesystem access, process launching, arbitrary native code, clipboard read, keystroke capture. A widget can request "open URL/file" only through a user-initiated click mediated by core.

## 6. Performance budget (targets to be validated by prototype)
| Metric | Target | Enforcement |
|---|---|---|
| Core service idle CPU | <0.5% on 4-core laptop with 5 declarative widgets | Governor tick coalescing; no per-widget timers; CI benchmark |
| Core RAM | <60 MB | Rust, no embedded runtime |
| Widget host RAM per monitor | **~200 MB private with 5 light widgets (revised target).** Measured (release, 1 run): blank page 146 MB, 5 sandboxed iframe widgets 158.7 MB private (359 MB working-set sum), 8 processes, 0.21% of one core idle; the original <150 MB target is not achievable with WebView2. Real widgets, low-end HW, multi-monitor not yet measured (`plinth/spikes/webview-probe/RESULTS.md`) | Declarative-first; per-widget memory cap, kill+restart on breach |
| GPU | <1% idle; animations capped at 30 fps default, 60 fps opt-in | `requestAnimationFrame` wrapped by host |
| Startup to visible desktop | <3 s after login (delay-start option) | Lazy widget init |
| In-game | 0% measurable: widgets suspended, windows hidden | Governor (fullscreen, game list, battery, RDP, display-off) |
| Wallpaper, video 1080p30 H.264, one monitor | +90 MB, +2.6% of one core while visible, about 0% covered (measured, v0.8.0, one machine) | Impact Meter; governor pause; GPU load NOT measured |
| Wallpaper, web page | +40 to +145 MB, +2.6 to +7.2% while visible (measured, three bundled pages) | Pages capped at 30 fps; unloaded when paused |
| Static wallpaper via OS | 0 runtime cost when the monitor has no widgets (INFERENCE: the OS draws it) | `IDesktopWallpaper` |
| Library of 10,000 images (targets, NOT validated) | Hub grid first paint under 300 ms from the index; resident hub memory increase under 80 MB (virtualized grid, 200 decoded thumbnails max); thumbnails generated at low priority at 20 per second or better; disk cache capped (default 500 MB, LRU) | Benchmark with a generated 10k-image folder |
| Folder scan | Metadata-only pass of 300 or more files per second on SSD (target); incremental after the first scan | Benchmark |
| Display change handling | Reconcile completes in under 200 ms; no persistence writes during the event | Unit tests on the pure function |
**Quality gate:** a release cannot ship if the benchmark suite regresses >10% on any row. Week 1 prototype must validate WebView2 RAM claim; if >250 MB per monitor, switch declarative widgets to a Rust/Direct2D renderer (option C for the hot path).

## 7. Security model
- Packages **signed** (publisher key + store countersignature in v1.0); MVP verifies Scene bundles against pinned first-party key.
- Permissions shown as a plain-language sheet at install; **permission diff** blocks silent escalation on updates.
- Sandbox: separate origin per widget, no `eval` from remote sources, strict CSP, network only to declared hosts via core proxy (core enforces allowlist, rate-limit).
- What a malicious widget **can** do: burn CPU (killed by governor), show misleading content, exfiltrate data it was granted to its declared hosts. **Cannot:** read files, run programs, access other widgets' storage, reach undeclared hosts, persist outside quota.
- Store (v1.0): automated static scan (obfuscation, remote script loading), human review for first listing and permission increases, takedown + revocation list checked at startup. The Wallpaper Engine "Applications" Workshop incident [research S10] is why executables are a permanent non-feature.
- **Spike C findings (2026-10-30, `plinth/spikes/sandbox-probe/RESULTS.md`):** (1) `sandbox="allow-scripts"` alone allowed fetch with readable responses, XHR, beacon, WebSocket, remote script load/execute. (2) A host-generated strict CSP in the widget document blocked all of those, but navigation exfiltration (`location.href = attacker URL`) still worked; a host-page CSP `frame-src about:` stopped it. Both layers are required. (3) Storage, parent DOM, top navigation and file access were blocked by the sandbox. (4) **A busy-loop widget froze the host page and other widgets for its whole duration in 7 of 8 runs**: the Permission Center and Safe mode must therefore run outside the widget host process, and a watchdog or per-widget process isolation is a requirement (per-origin isolation untested). (5) Treat all IPC as untrusted; the direct `window.ipc` call from an iframe did not reach Rust in these runs but the object is exposed.
- **Spike C follow-up (origin isolation):** serving each third-party widget from its own origin via a host custom protocol, CSP as an HTTP header, and a host `frame-src` allowlist gave 0 leaked requests and contained a CPU hog (5/5 runs), at about +29 MB and +1 process per widget. Design decision: **hybrid delivery**; bundled first-party widgets share one trusted origin, third-party widgets each get their own origin, with a soft cap on concurrent third-party widgets per monitor. Replaces srcdoc delivery for third-party content.
- **Spike C correction and fix (host-demo):** self-navigation by a widget opens a bare TCP connection to an address of its choosing even under header CSP and the host `frame-src` allowlist (4 of 4 runs), a destination-address leak and possible DNS covert channel; earlier "0 leaked requests" meant 0 HTTP requests. Forcing all WebView2 traffic through a Plinth-controlled local proxy (`--proxy-server` + `--proxy-bypass-list=<-loopback>`) gave 0 direct connections (4 of 4 runs). **Decision D8: widgets never reach the network directly; Plinth runs a deny-by-default local proxy that allow-lists declared and granted hosts, logs every request, rate-limits, and feeds the Impact Meter.** Open: per-widget attribution at the proxy, allow-listing WebView2's own SmartScreen traffic, HTTPS/WebSocket handling, behavior when the proxy is down.
- **Spike D result (plinth-proxy):** the deny-by-default CONNECT proxy is implemented and tested (8 tests + end to end). With WebView2 forced through it, the hostile widget produced 0 direct connections and 0 allowed tunnels in 4 of 4 runs, and all 11 widget checks still passed. Not yet proven: the HTTPS allow path from a widget, per-widget attribution (union allow-list plus per-widget CSP), `wss`, and any WebView2 service traffic that needs allow-listing.
- **Measured limit:** in the spike, sandboxed iframes did NOT get separate processes (8 processes with 0, 5 or 10 widgets), so isolation is web-level (opaque origins), not process-level. A renderer exploit or side channel could cross widgets. Per-widget processes were not measured.
- Residual risk: WebView2/Chromium vulnerabilities; mitigated by Evergreen runtime updates. Supply chain: lockfiles, `cargo-deny`/`npm audit` in CI.

## 8. Windows update resilience
1. No shell hooks, no undocumented APIs except the contained layer abstraction (§3).
2. Insider-channel CI smoke tests + public "status" page.
3. Feature-flag/kill-switch for layer strategy via signed remote config.
4. Auto-update with rollback; core service supervised by tray launcher.
5. Compatibility matrix tracked per Windows build in the repo.

## 9. Updater and marketplace client
- App: Tauri updater with signed update manifest (static hosting acceptable; GitHub Releases for MVP).
- Content: package manager compares installed vs. index versions; installs to `%LOCALAPPDATA%\Plinth\content\<id>\<version>` side by side; atomic switch; rollback.
- Store API (v1.0): minimal REST (search, package metadata, download URL, license check, purchase intent); backend choice deferred (see decisions).

## 10. Testing
Unit (Rust), golden-image tests for declarative renderer, multi-monitor/DPI matrix in VMs, performance benchmark suite, fuzz the package parser.

## 11. Monitors and widget placement (addendum; D25, D26, D27)
- **Enumeration:** `EnumDisplayMonitors` for handles and rectangles [S46], `GetMonitorInfo` for work area and primary flag, `GetDpiForMonitor`/`GetDpiForWindow` for scale [S47, S51], `QueryDisplayConfig` + `DisplayConfigGetDeviceInfo` for friendly name, device path, EDID ids, refresh rate and rotation [S48, S49]. The process is Per-Monitor v2 aware [S51].
- **Identity:** `MonitorId = monitorDevicePath`, fingerprint = (edidManufactureId, edidProductCodeId, connectorInstance), last resort = position. Matching order: path, fingerprint, then "unmatched" (parked). Shared setups store roles, not hardware ids.
- **Binding model:** `WidgetPlacement { widget_instance, home: MonitorId or Role, anchor: TopLeft|TopRight|BottomLeft|BottomRight|Center, offset_dip: (x, y), size_dip: (w, h), settings }`. Widget instances (not types) are bound, so the same type can live on several monitors with different settings.
- **Reconcile (pure function, unit-tested):** `reconcile(placements, monitors, fallback) -> Vec<Resolved { placement, monitor, parked: bool, rect_px }>`. It clamps inside the work area, scales oversize widgets (not below 50%), and never mutates `placements`. Persistence writes `placements` only, so parked or clamped results can never overwrite the home data (the Rainmeter failure [S52]).
- **Events:** listen for `WM_DISPLAYCHANGE` [S50], `WM_DPICHANGED` [S51] and device/settings change messages (the exact hot-plug event set must be verified on hardware), debounce 1.5 s, re-enumerate, reconcile, then move or create/destroy layer windows. A monitor must be stable for 2 s before widgets return to it.
- **Layer windows:** one per monitor needing one, attached with the same Progman technique as today; a window is destroyed when its monitor disappears and recreated when it returns. The cross-process `SetParent` DPI-reset risk [S51] must be tested with mixed DPI.
- **Identify:** a borderless topmost window per monitor rectangle showing its number for about 2 seconds.
- **Recovery:** "Bring all to primary" = reconcile with every `home` ignored; "Reset positions" = restore the Scene default layout with an undo snapshot.

## 12. Wallpaper engine and library (addendum; D24, D28, D29)
- **Static images:** per monitor, `IDesktopWallpaper::SetWallpaper(monitorID, path)` [S43] plus `SetPosition` [S44] and `SetBackgroundColor`; the monitorId list comes from `GetMonitorDevicePathAt` and must be joined to the DisplayConfig device paths (NOT TESTED that they are equal). Because position and color look global in the interface, per-monitor fit modes are produced by Plinth composing a cached, pre-fitted image for that monitor when the modes differ.
- **Layer drawing:** monitors with widgets also draw the image in the layer so glass blur works; CSS maps the modes (Fill=cover, Fit=contain on the letterbox color, Stretch=100% 100%, Center, Tile=repeat, Span=one image offset across monitor rectangles).
- **Motion wallpapers:** unchanged from v0.8.0 (video and sandboxed web pages, `pwv` origin, governor pause). One active by default.
- **Slideshow:** Plinth runs the timer (interval, shuffle) because the native slideshow takes only one folder [S45]; rotation pauses with the governor and stops when Plinth is not running (disclosed in the UI).
- **Revert:** before the first change, snapshot per monitor: wallpaper path, position, background color. Revert restores them.
- **Library index:** SQLite (WAL) in `%LOCALAPPDATA%\Plinth\library.db`: path, volume id and file id, size, mtime, content hash, width, height, aspect, type, added, favorite, last used, status (ok, missing, offline, corrupt, too large), average luminance grid. Dedupe: size plus hash of first and last 64 KB, full hash only on collision.
- **Scanning:** recursive with exclude list; first pass reads headers only (dimensions via header parse, refuse more than 16384 px on a side or over 256 MB as "too large" to avoid decompression bombs); later passes compare mtime and file id. Watching: directory change notifications for local folders, polling (on hub open and every 5 minutes) for network and removable folders. Moves and renames are matched by file id, else by content hash, so thumbnails and favorites survive.
- **Thumbnails:** 320 px JPEG in `%LOCALAPPDATA%\Plinth\thumbs\<hash>.jpg`, generated lazily by one low-priority worker when an item first becomes visible; served to the hub by a custom protocol with long cache headers; the hub grid is virtualized (only visible rows plus two are in the DOM); an LRU keeps at most 200 decoded thumbnails; disk cache capped (default 500 MB).
- **Failure handling:** missing file shows a card with Locate (file picker) and Remove; offline root marks items offline (kept, dimmed); unreadable or corrupt files get status corrupt with a reason; nothing blocks the UI thread; every error becomes a visible state, never a blank screen.
- **Legibility assist:** the layer draws a scrim between wallpaper and widgets (none, dim, blur); v1.0 auto mode picks scrim strength from the stored luminance grid so widget text meets a 4.5:1 contrast target (INFERENCE that the grid is accurate enough; to be validated).
- **Profiles:** Scene schema v2 stores per-monitor entries (`wallpaper`, fit, slideshow, widget placements). Applying on a machine with different monitors maps by identity, then role/order, then parks on the primary.

## 13. Impacts of the addendum on earlier decisions
Animated wallpapers move into MVP (D29, already shipped). One layer per monitor replaces the single primary-monitor layer built so far (v0.8.0 only handles the primary monitor). Product name, stack (wry/tao WebView2 shell, Rust crates) and MVP scope otherwise unchanged; the `windows` crate is added for COM (D24).

## 14. As built in v0.10.0 (measured on this PC, release build, single runs)
- Monitors: `mon.rs` enumerates with `EnumDisplayMonitors`/`GetMonitorInfoW`, `GetDpiForMonitor`, `QueryDisplayConfig` and `DisplayConfigGetDeviceInfo`; the shell's `IDesktopWallpaper` monitor ids equal the DisplayConfig device paths here, and Plinth still matches shell monitors by rectangle. A watcher thread polls every 500 ms and applies a change only after it has held still for 2 s.
- Layers: one window per monitor needing one, one shared WebView2 environment. Second layer on the real case screen: +41 MB, +1 process, unchanged idle CPU. The layer attach strips every frame style and tao's invisible shadow border (client rectangle verified equal to the monitor size).
- Library: JSON index (D32). 10,003 pictures: first scan 30.7 s, unchanged rescan 18 ms, save 22 ms, load 28 ms, page 3.4 ms, search 2.3 ms. Thumbnails: 3 workers, 26.5 ms each for 2560x1440 JPEGs, 320 px JPEG, cached by content key. Compose to 1080p: 381 ms. Targets in section 6 for scan speed (300 files/s), page paint and thumbnails are met; hub memory with a 10,000-item library was NOT measured.
- Pictures reach the hub through the `pwth.localhost` protocol (async, worker pool) and the layers through `pwc.localhost`; both only serve validated file names from Plinth's own cache folders. The hub CSP allows `img-src` only from the thumbnail origin.
- Safety: Plinth only reads user pictures; it writes cache copies in its own folder and removes library entries, never user files. Wallpaper sandbox rules (D23) are unchanged.
- Hub memory with a 10,014-picture library (v0.12.1, measured): 275 MB with the Wallpapers view open (+46 MB over the same hub on another page), transient peak about 540 MB while scrolling the whole grid repeatedly (GPU process), no unbounded growth over five passes; freed when the hub closes.
