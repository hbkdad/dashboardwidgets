# plinth-app results (2026-10-30)

Machine: Windows 11 Home build 26300, 28 logical cores, 1920x1080 single monitor, WebView2 156.0.4314.50, release build.

## Unattended end-to-end autotest: 10 of 10 checks, 3 of 3 runs (exit code 0) (OBSERVED)
`PLINTH_AUTOTEST=28`: the test drives the same hub command handler the UI uses (`apply`, `grant`, `safe`), not a shortcut.
1. System monitor's CPU and memory reads were allowed and logged (live Windows counters, not mocks).
2. Notes' storage read allowed; Clock made no requests.
3. After the user revoked CPU, the sysmon CPU request was BLOCKED immediately (every poll after the revoke), memory stayed allowed; nothing else was blocked.
4. Safe mode: 0 widget events while on (3 of 3 runs).
5. Apply created the layer and persisted state (`applied: true`; the revoked grant persisted: sysmon keeps only `system.ram`).
6. Proxy: 0 tunnels allowed. Its log held only browser service traffic (`config.edge.skype.com`, `nav-edge.smartscreen.microsoft.com`) and empty speculative connections, all denied.
7. Layer visible after Show Desktop: a beacon pixel at the screen center read magenta (pixel check on the composed screen).
Also OBSERVED: restart restores an applied scene (`restored applied scene graphite (layer created: true)`).

## Real hub window click-through (OBSERVED): 28 of 28 checks, 4 of 4 final runs
`scripts/clicktest.ps1` launches the app with an isolated state file and drives the real hub window through Windows UI Automation (WebView2 exposes its accessibility tree), then verifies effects in `state.json` and the app log: first-run sheet and dismissal, choosing a scene, Apply (state persisted, layer created, "Applied" chip and Revert enabled), Safety list, expanding a widget, revoking CPU (persisted), Safe mode on and off (persisted, button label changes), Network view, Close hides the hub while the app and layer keep running, a second launch exits at once and brings the hub back, Revert (persisted).
Bugs this found and fixed: (1) the "Applied" chip showed before anything was applied (CSS `display` overrode the `hidden` attribute); (2) the hub rebuilt its whole widget and scene lists every 4 s, which could swallow a click and drop keyboard focus (an accessibility defect): regions now re-render only when their data changes. Earlier runs before fix 2 failed 1 of 5 times on the revoke step; after it, 10 of 10 runs passed. Console `?` characters in PowerShell output were a console-encoding artifact, confirmed by checking the U+00B7 code point in the element name.
NOT tested here: the tray menu items themselves (only that the icon is created), human use, a screen reader, high-contrast mode.

## Hub UI with stubbed state (OBSERVED)
Loaded `hub.html` in a browser with a stubbed `window.ipc` and a sample state: 3 scenes, 3 widgets, first-run sheet, cost chip, log and proxy rows rendered; clicking a permission switch, Safe mode, Apply and a scene card sent the expected JSON commands; no console errors. A person (or UI automation) has NOT clicked the real hub window.

## Not tested
Multi-monitor, DPI changes, lock/sleep/resume, explorer restart, other Windows builds, low-end hardware, memory over time, the HTTPS allow path, per-widget network attribution, icon z-order (layer sits behind the icons view by construction; not visually verified), real screen-reader use, installer/update.

## Honest caveats
The autotest takes over the screen for about 30 s and sends Win+D. The first-run state in the test is isolated via `PLINTH_STATE`; a normal first run also grants first-party widgets their declared permissions (a policy choice: built-in widgets are trusted by default).

## Motion overlays: cost on this PC (2026-11-06, branch feat/motion-overlays, release build, 20 s per run, one run per row)
Method: `scripts/measure-wallpapers.ps1 -Overlays` (starts the app with an isolated state, Scene background, overlay on at medium amount, hub hidden; "visible" = Win+D pressed so the layer is on screen, "covered" = normal windows on top; whole process tree, private bytes and CPU over 20 s). Two real 1920x1080 monitors; the overlay is drawn on the primary only.

| Overlay | RAM visible | CPU visible | RAM covered | CPU covered |
|---|---|---|---|---|
| none (baseline) | 320 MB | 3.48% | 326 MB | 3.33% |
| Particles | 391 MB (+71) | 8.25% (+4.8) | 382 MB | 6.56% |
| Soft light | 364 MB (+44) | 3.63% (+0.2) | 360 MB | 19.50% |
| Rain | 367 MB (+47) | 9.12% (+5.6) | 372 MB | 11.18% |
| Gradient drift | 354 MB (+34) | 11.75% (+8.3) | 355 MB | 2.86% |

FACT: each overlay adds one WebView2 process and 34 to 71 MB. The CPU numbers are NOT reliable: the baseline itself was 3.4% (earlier runs of the same build on this PC measured 0.3 to 2%, so something else on the PC was active), the "covered" column is sometimes higher than "visible" (Soft light 19.5%) and single 20 s runs were used. INFERENCE: the real cost is a few percent of one core while visible. NOT MEASURED: GPU load, repeat runs on a quiet PC, why a covered layer still costs CPU for some overlays (the earlier web-wallpaper finding was about 0 when covered). The Impact Meter uses 50 MB and 5% (medium); the factors 0.7 and 1.4 for subtle and busy are UNVERIFIED.
