# Changelog

All notable changes to this project are recorded here. Format: [Keep a Changelog](https://keepachangelog.com/en/1.1.0/). Versioning: [Semantic Versioning](https://semver.org/), see `CONTRIBUTING.md`. Pre-1.0 (`0.y.z`): anything may change; the minor number moves with each completed milestone.

## [Unreleased]

## [0.13.1] - 2026-10-30
### Fixed
- **A black console window opened whenever the app was started from Explorer or at sign-in.** The release build is now a Windows (GUI) program; `scripts/logtest.ps1` checks that no console host is started.
### Added
- A rotating log file (`%LOCALAPPDATA%\Plinth\plinth.log`, 1 MB, then `.old`) records what the app does when nothing is attached to its output, including panics.
- Settings, "Troubleshooting": a diagnostics report to read and copy. It lists counts and sizes only (version, Windows build, monitor sizes and scale, widget and picture counts, the last 60 log lines); the user name is replaced everywhere and no monitor names, ids or file names are included. Unit tests and a click-test check cover it.
### Changed
- `verify.ps1` starts the app and waits for it (PowerShell does not wait for a GUI program with `&`); every stage now prints its output and uses the real exit code. Before this fix the stages would have reported success without running the test.

## [0.13.0] - 2026-10-30
### Added
- Live Graphs widget (`com.plinth.graphs`): CPU, GPU, memory and network history over about a minute and a half, four small charts that suit a case screen. Verified on the real 1280x400 case screen (the layout also keeps clear of its taskbar). New first-party widgets added by an update are granted the permissions they declare, like at first run; third-party widgets never are.

## [0.12.1] - 2026-10-30
### Measured and improved
- Hub memory with a 10,014-picture library (release build, this PC, no accessibility client attached, single runs): hub web view 229 MB with the hub open on another page, 275 MB with the Wallpapers view open (+46 MB, inside the 80 MB target); while scrolling the whole grid again and again it peaks around 540 MB (almost all in the browser's GPU process, which gives memory back: 426 MB then 356 MB) and does not grow without bound over five passes. The hub's web view is released when the hub is closed. Using UI Automation to scroll inflated the numbers to 533 MB, so tests that need honest memory must not attach one (`PLINTH_HUBSCROLL`).
- The grid reuses cards that stay in view and releases the picture of any card that scrolls away; thumbnails are served `no-store`.

## [0.12.0] - 2026-10-30
Ready to hand to someone else.
### Added
- `scripts/package.ps1`: builds the release and packs a self-contained folder and zip (3.1 MB: exe, widgets, wallpapers, the 12 bundled pictures, licence, readme). Verified by unzipping elsewhere and running the autotest from the copy with no development paths (20 of 20). The zip is not code-signed and is not published anywhere.
- Settings page: "Start with Windows" (a per-user startup entry that launches `plinth-app.exe --background`: tray only, no window), and an About box with the version. A second launch opens the hub.
- `scripts/backgroundtest.ps1` (3 checks) and Settings checks in the click test (54 of 54); the click test uses a test-only startup entry and leaves nothing behind.

## [0.11.0] - 2026-10-30
Moods and setup cards.
### Added
- Moods: rules that switch your saved profiles by the program you are using, a fullscreen app or game, battery, or time of day (`plinth-core::context` rule parsing with strict validation; the engine never fights a choice you make by hand, waits two minutes between switches, and the learner only suggests a rule after the same manual choice three times). The context (foreground program, fullscreen, battery, time) is sampled every 5 s. Off until you turn it on.
- Setup cards: export your scene and widget places as a `.plinthcard` file (fractions of each monitor, widget ids and versions; no code, no monitor ids, no personal data) and import someone else's by dropping the file or choosing it; a review sheet shows missing widgets and the permissions the card's widgets could ask for, nothing is granted automatically, and Undo restores your previous places (`SetupCard::export`, strict parser round trip).
- Hub "Moods" page; autotests: Moods and cards 11 checks; click test 52 checks; `verify.ps1 -Moods`.
### Fixed
- Tests could be blocked by a stuck web view profile left by an earlier run: the hub and layer web view folders can now be redirected with `PLINTH_WEBVIEW`, and the test scripts use their own.
### Not built
- Builds showcase (a gallery of PC builds); sharing cards through a store; applying a card's wallpapers (cards hold scene and widget places only).

## [0.10.2] - 2026-10-30
Reliability: the intermittent click-test failures are explained and fixed.
### Fixed
- A save could be lost: Plinth wrote the state file through a temp file and renamed it, and Windows refuses to rename over a file another program has open (a backup tool, an antivirus scan, a script); the failure was ignored. Saves now retry and fall back to a direct write (`state::write_atomic`, same for the library index; test included). Explains the occasional "choosing Dusk persisted" failure.
- A second launch sometimes did not bring the hidden hub back: the second process showed the first process's window from outside, which raced with the app's own window bookkeeping (the log proved the window was found and shown, yet stayed hidden). The second launch now sets a named event and the running app shows its own window.
- Click test: leftover app processes from a previous run could hold the single-instance lock and make the next run exit at once; the test now clears them first. Result: 20 click-test runs in a row passed 47 of 47 (before: about 1 failure in 6).

## [0.10.1] - 2026-10-30
### Added
- Several copies of the same widget, each on any monitor with its own place: "Add a copy" / "Remove copy" in the Monitors view. Copies share the widget's permissions and grants (revoking once stops every copy) and each has its own storage (`plinth-host::Host::bridge_instance`, instance names `id#2` to `id#99`, validated). The fake-monitor lifecycle test now covers a copy (15 of 15 checks); host tests 25.
### Known
- One standard-autotest run showed the desktop-visibility check failing once; 3 reruns passed 20 of 20. The click test still occasionally fails on timing (the Dusk selection or the second-launch step).

## [0.10.0] - 2026-10-30
Monitors and the picture wallpaper library (owner addendum; decisions D24-D29, D31-D33).
### Added
- `plinth-core::monitors`: monitor model, widget placement (home monitor, anchor, offsets and size in device-independent units), the pure `reconcile` function (unplug parks widgets on a fallback, replug restores exactly, resolution and DPI changes keep widgets on screen, oversize widgets shrink then clip), `bring_all_to`. `plinth-core::fit`: Fill, Fit, Stretch, Tile, Span, Center geometry. `plinth-core::library`: scan diffing with rename detection, search, favorites and recent filters, slideshow order (13+ tests each).
- `mon.rs`: real monitor enumeration (stable device path, EDID fingerprint, friendly name, DPI, refresh rate, work area) and a fake-monitor mode for tests. `os_wall.rs`: the OS wallpaper per monitor through `IDesktopWallpaper`, snapshot and exact Revert, native folder and file pickers.
- One desktop layer window per monitor that has widgets or a motion wallpaper; a monitor with nothing to draw has no layer.
- Hub "Monitors" view: layout diagram matching the real arrangement, Identify monitors, widget-to-monitor assignment by dropdown, by dragging, or by click then click, parked-widget notice, fallback monitor, Bring all widgets to the primary monitor, Reset positions with Undo, per-monitor wallpaper.
- Picture library: bundled starter set (12 original generated wallpapers, `scripts/make-bundled-wallpapers.py`), your own folders (recursive, skip subfolders, watched every 5 s while the hub is open), index with duplicate detection by content, rename and move tracking, thumbnails (320 px, cached), virtualized grid with search, Favorites, Recent, Problems, detail panel with fit mode, bars color, monitor choice and slideshow, missing/corrupt/offline states with Locate and Remove. Pictures are composed per monitor at its exact resolution and set as the OS wallpaper; Revert restores the previous wallpaper exactly.
- Legibility shading behind widgets (None, Dim, Soften and dim) and named Profiles that save wallpapers, widgets and monitor assignments together.
- Autotests: fake-monitor lifecycle (13 checks), library end to end on the real monitors (10 checks), click test now 50 checks.
### Fixed
- The desktop layer was inset by about 8 px on every side (client 1904x1071 on a 1920x1080 screen): tao reserved an invisible shadow border and the old frame style bits stayed set. Found by reading the real window rectangles with two monitors; fixed (client now exactly the monitor size).
- A pictures with the wrong extension (a JPEG saved as .png) was reported unreadable; the format is now guessed from the content.
### Measured (this PC, release build, single runs; see `impact.rs`)
- Second layer window on the real 1280x400 case screen: +41 MB, +1 process, idle CPU unchanged (264.7 to 305.7 MB).
- 10,003-picture library: first scan 30.7 s (about 326 files/s, runs in the background), unchanged rescan 18 ms, index save 22 ms (3.2 MB), load 28 ms, one grid page 3.4 ms, search 2.3 ms. Thumbnails 26.5 ms each from 2560x1440 JPEGs (one thread); composing a 1080p wallpaper 381 ms.
### Known limits
- One instance per widget: the same widget type on several monitors with different settings is NOT built yet.
- Monitor hot-plug is detected by polling twice a second with a 2 s settle time, not by display events; identity stability across docks, port swaps and two identical monitor models is NOT TESTED.
- Per-monitor wallpaper while several virtual desktops exist is NOT TESTED. The intermittent click-test failure (second launch not bringing the hub back) is still unexplained.

## [0.9.0] - 2026-10-30
Security Center (decision D30) plus the wallpaper/monitor addendum design (D24-D29, `research/monitors-and-wallpapers.md`).
### Added
- `plinth-core::security`: address classification (this PC, local network, internet), listener exposure wording, startup command-path extraction and location labels, Security Center product-state decode (5 tests).
- `plinth-app/src/sec.rs`: read-only collectors: firewall profiles, antivirus and Defender status (one fixed PowerShell command), TCP connections and listeners per program (IP Helper API), Run-key and Startup-folder items.
- Hub "Security" view: Scan now, Windows protection, Plinth's own activity, internet connections by program, listening ports and who can reach them, startup items. States plainly that it is visibility, not an antivirus.
- Autotest check for the collectors (20 of 20); click test 37 of 37; XSS regression check now covers the Wallpapers and Security views.
- Design addendum: wallpaper library and monitor selector (PRD 5.4, architecture 11-13, market gap rows 13-14, sources S42-S54).

## [0.8.0] - 2026-10-30
Wallpaper pillar (decision D23).
### Added
- `plinth-core::wallpaper`: wallpaper manifest parsing with a closed id/entry/file-type vocabulary, HTTP range parsing, safe slugs (11 tests).
- `plinth-host::WallpaperStore`: scans bundled and user wallpaper folders, serves them under the `pwv` origin with Range support, per-file type allow-list, symlink-safe path resolution, a CSP that forbids connections for web wallpapers; imports and removes the user's own video copies (8 tests).
- Three bundled animated web wallpapers (Aurora, Starfield, Embers) and video wallpapers (`.mp4`, `.webm`, `.m4v`) added by dropping a file on the hub.
- Hub "Wallpapers" view: pick, add by drag and drop, remove, open folder, rescan. Wallpapers pause with the governor (video paused, web page unloaded).
- Measured wallpaper cost in the Impact Meter (1080p, one machine, single 20 s runs; baseline 301.7 MB / 0.70% of one core visible): Aurora +40 MB / +2.6%, Starfield +50 MB / +6.8%, Embers +145 MB / +7.2%, 1080p30 H.264 video +90 MB / +2.6%; covered by other windows about 0% CPU. GPU load not measured.
- `scripts/measure-wallpapers.ps1`, `scripts/make-test-video.py`; app autotest checks for wallpaper serving and video playback.

## [0.7.0] - 2026-10-30
Monitoring pillar and the all-in-one direction (decisions D16-D22).
### Added
- `plinth-app/src/hw.rs`: one-thread user-mode sampler (CPU via GetSystemTimes, RAM, NVIDIA GPU via NVML loaded only from System32, network/disk/CPU clock via PDH, parts list from the registry); no kernel driver (D17).
- Widgets PC Vitals and Build Card; `system.info` permission; first-party widgets share one trusted origin (`pwt.localhost`), third-party widgets keep isolated origins (D21). `Host::install_trusted`, `bridge_widget`.
- Pause governor wired to the OS (fullscreen app, battery, remote session) with an end-to-end test; live motion wallpaper is opt-in; the hub's web view is released while hidden; hub revival watcher; state loader tolerates a UTF-8 BOM.
- Impact Meter now uses measured numbers (`scripts/measure-app.ps1`): about 270-300 MB with the hub closed, +200 MB while it is open; about 2% of one core while the desktop is visible (about 9% with live motion), near 0 when covered.
- `research/pc-builders.md`, `scripts/{measure-app,relaunch-test}.ps1`, `scripts/hub-xss-check.js`; 53 unit tests, app autotest 15 checks, click test 28 checks.
### Security
- Security review found stored XSS in the hub: widget-controlled text (diag.log notes, manifest fields, proxy targets) was rendered with `innerHTML`, so a hostile widget could run script in the trusted hub (which owns the grant/safe-mode IPC). Fixed with output escaping plus a hub CSP; verified before (script ran) and after (0 injected elements). Only first-party widgets ship today, so this was latent, not exploited.
### Known limits
- Intermittent (about 3 in roughly 30 runs) click-test failure on re-showing the hub after a second launch; not root-caused, diagnostics added.
- CPU temperature and fan speed are not readable in user mode on the tested PC; trusted status is currently decided by widget id prefix `com.plinth.` and the bundled widgets folder, which must become signature- or location-based before downloaded packages exist.

## [0.6.0] - 2026-10-30
Real-window click testing, background mode and single instance.
### Added
- `scripts/uia.ps1`, `scripts/clicktest.ps1`: click-through of the real hub window via Windows UI Automation (28 checks, 4 of 4 final runs); `verify.ps1 -Click`.
- Tray icon (Open, Toggle Safe mode, Quit), background mode (closing the hub hides it, layer keeps running), single instance (second launch restores the hub).
### Fixed
- "Applied" chip was visible before any scene was applied (CSS `display` overrode `hidden`).
- Hub rebuilt its widget/scene lists every 4 s, swallowing clicks and dropping keyboard focus; regions now re-render only when their data changes.
### Not tested
- Tray menu items, human use, screen reader, high-contrast mode, autostart (not built).

## [0.5.0] - 2026-10-30
First runnable app.
### Added
- `apps/plinth-app`: hub window (Scenes, Safety, Network), desktop layer behind the icons, 3 first-party widgets (`plinth/widgets/clock|sysmon|notes`) with live CPU/RAM, permission broker, forced deny-by-default proxy, persistent state, restore-on-start. Unattended autotest: 10 of 10 checks in 3 of 3 runs.
- `Host::describe()`, `Host::storage_snapshot()/restore_storage()`, `Broker::is_granted()`.
- Working method files: `CLAUDE.md`, `feature_list.json`, `claude-progress.md`, `scripts/verify.ps1` (decision D15).
### Decisions recorded
- D13 wry+tao shell, plain HTML/JS hub; D14 built-ins granted at first run; D15 harness-style durable state.
### Known limits
- Primary monitor only, no tray/autostart, Safe mode pauses first-party too, hub window not click-tested by a human or automation, no installer. See `plinth/apps/plinth-app/README.md`.

## [0.4.0] - 2026-10-30
Spike D: deny-by-default network proxy closes the bare-connection gap in end-to-end tests.
### Added
- `plinth-proxy` crate (std only): deny-by-default HTTPS CONNECT proxy with allow-list policy, private-address guard, request limits and a log; 8 tests.
- `Host::allowed_destinations()` (union of granted net.fetch hosts) with a test; host-demo `PLINTH_PROXY=1` runs against the real proxy (3 extra checks).
### Security
- Spike D: with WebView2 forced through `plinth-proxy`, 0 direct connections and 0 allowed tunnels in 4 of 4 end-to-end runs; closes the bare-connection gap found in v0.3.0 for the tested attack. Allow path over real HTTPS and per-widget attribution remain untested.

## [0.3.0] - 2026-10-30
End-to-end host demo, decisions log, license, and a security correction with a validated mitigation.
### Added
- `apps/host-demo`: plinth-host in WebView2 with real fixture widgets (`fixtures/widgets/clock-cpu`, `hostile`), Permission Center log, 11 automated checks (11 of 11 pass), forced-proxy experiment (`PLINTH_PROXY=1`).
- `design/decisions.md` (D1-D12), Apache-2.0 `LICENSE`.
### Fixed
- host-demo: wry reports custom-protocol requests as `<scheme>://localhost`, so slot identity now comes from the protocol name.
### Security
- Correction: self-navigation opens a bare TCP connection under CSP (destination-address leak); earlier "0 leaked requests" meant 0 HTTP requests. Mitigation validated in 4 of 4 runs: force all WebView2 traffic through a local Plinth proxy (decision D8).
### Decisions recorded
- License Apache-2.0 for engine crates; payments via merchant of record; "Plinth" is a codename with naming collisions (public name pending clearance). See `design/decisions.md`.

## [0.2.0] - 2026-10-30
Consumer-research revision: widget host library, Setup Cards, Moods, Impact Meter, hub prototype, project skills.
### Added
- `plinth-host` crate: widget packages, 8 isolated origin slots, per-widget CSP generated from manifest and grants, permission-checked bridge with storage quotas (11 tests).
- `plinth-core::setup` (Setup Cards: validation, permission summary, monitor fitting), `::context` (Moods engine and suggestion learner), `::impact` (Impact Meter), broker helpers `granted_fetch_hosts`, `note`, `is_registered` (27 core tests).
- Hub prototype `design/prototype/index.html` and `design/ux-flows.md`: scenes with live parallax stage, Safety/Permission Center, Library with permission-diff updates, Remix, Settings, first-run, plus Moods, Share setup, Edit layout, Impact chips.
- `research/consumer-demand.md`; `design/tooling-plan.md`; project skills `.claude/skills/plinth-{research,spike,widget-security}`.
### Changed
- PRD revised with consumer-driven features M11-M14 and a new pitch (design/product-requirements.md §1.0).
### Decisions recorded
- Pitch broadened from "safe widgets" to "make it yours, share it in one click, keep it cheap on your PC, and know it is safe".
- Plugins: A11y Enforcer enabled by the user; Rust CLI Skills offered but not confirmed enabled.

## [0.1.0] - 2026-10-30
Phase 0 spikes A, B and C run. Milestone: the sandbox and window-layer approach is validated on one machine; budgets revised.
### Added
- Spike C: `plinth/spikes/sandbox-probe` hostile-widget suite with a local test server and `RESULTS.md`.
- Repo hygiene: `.gitignore`, `.gitattributes`, `CONTRIBUTING.md` (versioning, branches, commits, release steps), CI workflow (Windows: `cargo test -p plinth-core`, `cargo build --workspace`).
### Changed
- `design/architecture.md` security model updated with Spike C findings (sandbox alone insufficient; CSP plus host `frame-src`; process isolation or watchdog required).
### Findings
- Sandboxed iframe alone leaks network and remote code; sandbox + generated CSP + host `frame-src about:` blocked every tested channel.
- A busy-loop widget starves the host and other widgets (7 of 8 runs); no CPU watchdog exists.
- Per-widget origin delivery (custom protocol, header CSP, host `frame-src`): 0 leaked requests, hog contained 5/5, about +29 MB and +1 process per widget.
### Decisions recorded
- Hybrid widget delivery: first-party shared origin, third-party one origin each (replaces srcdoc for third-party).
### Known open items
- Memory-bomb and GPU abuse tests, high widget counts, and wiring the permission broker into the real host are not done.

## [0.0.1] - 2026-10-30
Baseline: research, design, and first engineering spikes.
### Added
- `research/`: competitor analysis, market and gap analysis, Seelen UI audit (desk research), Seelen UI source-reading test, source list.
- `design/`: product requirements, architecture, business and roadmap (positioning: provable widget safety, no-code builder, creator payouts, no shell replacement).
- `plinth/crates/plinth-core`: manifest validation, permission broker with activity log, pause governor (12 unit tests).
- `plinth/spikes/layer-probe` (Spike A): GDI host fails Win+D on Windows 11 build 26300.
- `plinth/spikes/webview-probe` (Spike A+B): WebView2 host passes Win+D; 5 sandboxed iframe widgets measured at 158.7 MB private memory, 8 processes, 0.21% of one core idle.
### Decisions recorded
- Compete on the narrowed wedge (not partner/contribute).
- Seelen UI hands-on test skipped; its runtime claims stay NOT TESTED.
- Original <150 MB per-monitor RAM target dropped; revised to about 200 MB with 5 light widgets (to re-measure).
### Known open items
- Lock/sleep/display/DPI/multi-monitor/other-build matrix NOT TESTED; icon z-order and input unverified.
- License undecided (`UNLICENSED`); name unchecked for trademarks; payments/store backend undecided.
