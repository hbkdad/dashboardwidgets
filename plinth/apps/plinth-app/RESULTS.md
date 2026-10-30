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

## Hub UI (OBSERVED, stubbed)
Loaded `hub.html` in a browser with a stubbed `window.ipc` and a sample state: 3 scenes, 3 widgets, first-run sheet, cost chip, log and proxy rows rendered; clicking a permission switch, Safe mode, Apply and a scene card sent the expected JSON commands; no console errors. A person (or UI automation) has NOT clicked the real hub window.

## Not tested
Multi-monitor, DPI changes, lock/sleep/resume, explorer restart, other Windows builds, low-end hardware, memory over time, the HTTPS allow path, per-widget network attribution, icon z-order (layer sits behind the icons view by construction; not visually verified), real screen-reader use, installer/update.

## Honest caveats
The autotest takes over the screen for about 30 s and sends Win+D. The first-run state in the test is isolated via `PLINTH_STATE`; a normal first run also grants first-party widgets their declared permissions (a policy choice: built-in widgets are trusted by default).
