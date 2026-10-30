# Progress log (update at the end of every work session)

## State as of 2026-10-30 (v0.6.0)
- Released: v0.1.0 (spikes A-C), v0.2.0 (host lib, setup cards, moods, impact, prototype), v0.3.0 (host-demo, decisions, security correction), v0.4.0 (plinth-proxy). v0.5.0 first runnable app; v0.6.0 real-window click test (28 checks), tray + background mode, single instance.
- App works end to end: hub window, desktop layer behind icons, 3 first-party widgets with live CPU/RAM, permission broker, forced proxy, persistence. App autotest: 10 of 10 checks, 3 of 3 runs.
- Tests: 27 core, 13 host, 8 proxy.

## Next (in order)
1. Done in v0.6.0: hub click test via UI Automation; tray/background mode. Remaining: tray menu click test, autostart option.
2. Close known gaps: HTTPS allow path test (local TLS server), per-widget proxy attribution, wss, lock/sleep/display/DPI, multi-monitor.
3. Moods in the app (foreground app, fullscreen, battery probes), Setup Card import/export and 60-second try-out.
4. Name clearance and public brand; signed installer; store design with merchant of record.

## Known issues / failures (do not hide)
- Naming: "Plinth" collides with existing products and a trademark filing.
- Spike C early claims were corrected (bare TCP connection under CSP); see spikes/sandbox-probe/RESULTS.md.
- Safe mode currently pauses ALL widgets (first-party too); a first-party exemption is not implemented.
- First-party widgets use isolated origins like third-party ones (hybrid shared-origin delivery not implemented yet): about 29 MB each.
