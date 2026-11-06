# Progress log (update at the end of every work session)

## Feature work 2026-11-06 (branches feat/drag-placement, then feat/motion-overlays, feat/skins; NOT merged, NOT pushed)
- FACT (observed 2026-11-06): this PC's second monitor is now a 1920x1080 (22CV1Q, left of the primary at -1920,0), not the 1280x400 case screen of D33. Tests below ran on that layout; the 1280x400 shrink case is covered by unit tests only.
- Drag-and-drop placement (D35) built: `plinth-core::placement` (18 tests), edit overlay (`edit.html`), hub diagram drag, snapping settings. `scripts/dragtest.ps1` (real mouse) 14/14, three runs in a row; monitors lifecycle 17/17; click test 55/55. NOT TESTED: hub-diagram drag with a real pointer, keyboard nudge, touch, mixed DPI on hardware.
- Motion overlays (D36) built on `feat/motion-overlays`: 4 bundled overlays, hub card, governor pause, Impact Meter. `verify.ps1 -Overlay` 14/14. Cost measured once per overlay (RESULTS.md): +34 to +71 MB; CPU numbers noisy (baseline was 3.4% vs 0.3 to 2% earlier), covered case unexplained; GPU not measured.
- Custom skins (D37) built on `feat/skins`: `plinth-core::skin`, card fields, hub Skins view, SDK message for font/ink/accent. `verify.ps1 -Skins` 11/11 reads the live layer's computed styles. One screenshot OBSERVED fonts applied in widgets (Terminal mono, Paper card serif); the hub editor itself is NOT click-tested.
- Lesson: changing a window's region during a drag cancels the pointer capture (the drop arrived with stale coordinates). The overlay now widens its region while the pointer hovers a widget, never mid-drag.

## PC test run 2026-11-06 (v0.13.1, this PC: Windows 11 build 26300, two monitors, release build)
- PASS: `cargo test --workspace` (83 core, 25 host, 8 proxy, 19 app; 3 app tests ignored); app autotest 21/21; fake-monitor lifecycle 15/15; Moods and cards 11/11; real-hub click test 55/55; background 3/3; log 3/3; wallpaper library end to end 10/10 on the real monitors (Windows' wallpaper registry path unchanged afterwards; Plinth's own check says Revert restored every monitor).
- Lock/unlock: app stayed alive, memory normal, idle CPU 0.008% of the CPU afterwards; log shows widgets paused then resumed. Layer visuals after unlock NOT checked by screenshot (my window enumeration cannot see the layer: it is a child of the desktop window).
- Sleep/resume (Windows log: sleep 12:26:14, resume 12:26:18): app alive, idle CPU 0.017%, and a screenshot after resume shows the clock, vitals, graphs, notes and build widgets on both monitors (OBSERVED). Short sleep (about 4 s), not an overnight one.
- NOT TESTED: video wallpaper checks (needs a test clip; `make-test-video.py` needs a browser to record it), display wake/hot-plug on a real dock, mixed DPI, virtual desktops, Win key after wake (Plinth has no Start menu), a real game, low-end hardware. Script quirk found: my lock/sleep helper closed the hub, so its final Revert click did not run (isolated test state only).
- Compiler warnings present in the app crate (unused assignments, dead code); CI has no clippy step yet.

## State as of 2026-11-06 (v0.13.1)
- v0.13.0: Live Graphs widget; new first-party widgets are auto-granted what they declare. v0.13.1: no console window (GUI subsystem), rotating log, Settings diagnostics report, verify script waits for the app. v0.12.1: hub memory measured at 10k pictures.
- Unit tests (2026-11-06, `cargo test --workspace`): 83 core, 25 host, 8 proxy, 19 app (3 ignored), all pass. App autotests, click test and release build NOT re-run in this audit.
- Seelen UI hands-on test done: `research/seelen-ui-handson-test.md` (permission prompts exist and persist; coarse; idle about 1.3 GB WS on two monitors; Win+D did not hide its dock). PRD 1.1 updated.
- CI now runs core, host and proxy tests (the app tests need a display and are run locally via `scripts/verify.ps1`).
- Not published: zip is unsigned; `dist/` holds only the v0.11.0 build. Next: name clearance, code-signing certificate, clean-machine install test, Builds showcase, store design; multi-monitor docks, mixed DPI, virtual desktops, lock/sleep still NOT TESTED.

## Earlier state (v0.12.0)
- v0.12.0: packaging (`scripts/package.ps1`) and Start with Windows. Not published: the zip is unsigned. Next: name clearance, code-signing certificate, clean-machine install test, Builds showcase, store design.

## Earlier state (v0.11.0)
- v0.11.0: Moods and setup cards (D34). Next: name clearance, signed installer, installer-level testing on a clean machine, Builds showcase, store design; hardware cases still NOT TESTED (docks, mixed DPI, virtual desktops).

## Earlier state (v0.10.x)
- v0.10.0: monitors (per-monitor layers, assignment, reconcile, parking, recovery) and the picture wallpaper library (folders, scan, thumbnails, dedupe, fit, slideshow, OS wallpaper per monitor, Revert, profiles, shading). This PC has TWO monitors (primary 1920x1080 and a 1280x400 case screen); real tests used both.
- Tests: 80 core, 14 app, 24 host, 8 proxy; autotest 20/20; fake-monitor lifecycle 13/13; library end to end 10/10; click test 50 checks. Run everything: `scripts/verify.ps1 -App -Monitors -Library -Click`.
- v0.10.1: copies of a widget (own storage and place, shared permissions).
- v0.10.2: root-caused and fixed the flaky click test (lost saves under a file reader, second-launch race, leftover processes); 20 runs in a row pass.
- Open: right-click Move to monitor, hot-plug on real docks, mixed DPI, virtual desktops, name clearance, installer, store.

## Earlier state (v0.9.0)
- v0.9.0: Security Center (read-only visibility; D30) and the wallpaper/monitor addendum design (D24-D29). Next: v0.10.0 monitors + static wallpaper library (F19, F20).

## Earlier state (v0.8.0)
- v0.8.0: wallpaper pillar. Video (.mp4/.webm/.m4v by drag and drop) and sandboxed web wallpapers (Aurora, Starfield, Embers) in the layer, pause with the governor, hub Wallpapers view, measured cost. Tests: 40 core, 24 host, 8 proxy; autotest 19/19 (with PLINTH_TEST_VIDEO); click test 33/33.
- Not built: per-monitor targeting, wallpaper thumbnails, GPU-load measurement, web-wallpaper import UI.

## Earlier state (v0.7.0)
- Released: v0.1.0 (spikes A-C), v0.2.0 (host lib, setup cards, moods, impact, prototype), v0.3.0 (host-demo, decisions, security correction), v0.4.0 (plinth-proxy). v0.5.0 first runnable app; v0.6.0 real-window click test (28 checks), tray + background mode, single instance.
- App works end to end: hub window, desktop layer behind icons, 3 first-party widgets with live CPU/RAM, permission broker, forced proxy, persistence. App autotest: 10 of 10 checks, 3 of 3 runs.
- Tests: 27 core, 13 host, 8 proxy.

## Next (in order)
0. v0.10.0 shipped monitors and the picture library. Next: Moods + Setup Cards import/export + Builds showcase (v0.11.0), multi-instance widgets, then name clearance, signed installer, store, then Moods/Setup Cards import-export. Hardening before store: signature- or location-based trust instead of the `com.plinth.` id prefix.
1. Done in v0.6.0: hub click test via UI Automation; tray/background mode. Remaining: tray menu click test, autostart option.
2. Close known gaps: HTTPS allow path test (local TLS server), per-widget proxy attribution, wss, lock/sleep/display/DPI, multi-monitor.
3. Moods in the app (foreground app, fullscreen, battery probes), Setup Card import/export and 60-second try-out.
4. Name clearance and public brand; signed installer; store design with merchant of record.

## Known issues / failures (do not hide)
- Naming: "Plinth" collides with existing products and a trademark filing.
- Spike C early claims were corrected (bare TCP connection under CSP); see spikes/sandbox-probe/RESULTS.md.
- Safe mode currently pauses ALL widgets (first-party too); a first-party exemption is not implemented.
- First-party widgets use isolated origins like third-party ones (hybrid shared-origin delivery not implemented yet): about 29 MB each.
- Intermittent click-test failure (not root-caused): "second launch brought the hub back" failed about 3 times in roughly 30 runs of the v0.7.0 build (it often followed an autotest run); a focused repro passed 10 of 10 and 12 further full runs passed. `scripts/clicktest.ps1` now prints diagnostics (hub window state, process count, app logs) on failure; capture one failing run and fix it.
- Trust for first-party widgets is decided by the `com.plinth.` id prefix inside the bundled widgets folder; make it signature- or location-based before downloaded packages exist.
