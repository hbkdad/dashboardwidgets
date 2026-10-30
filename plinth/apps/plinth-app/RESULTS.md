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
