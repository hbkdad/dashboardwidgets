# Engineering Roadmap

Mirrors `../design/business-and-roadmap.md` §5. Targets are planning assumptions, not measurements. Time estimates for a solo developer are my inference.

## Phase 0: Validation spikes (about 2–4 weeks)
- [x] Core logic crate: manifest, broker, governor (done, 12 tests)
- [~] Spike A: GDI host FAILS Win+D on build 26300; **WebView2 host PASSES** (workerw/progman strategies, 9/9 pixels). lock/sleep/display/DPI/icon z-order/input/other builds NOT TESTED. See `spikes/layer-probe/RESULTS.md`.
- [~] Spike B: release build, 5 sandboxed iframe widgets = 158.7 MB private / 359 MB WS sum / 8 processes / 0.21% of one core idle (blank page 146 MB); 10 widgets 166 MB. Fixed WebView2 cost dominates, so <150 MB target NOT met. Iframes add no processes (web-level isolation only). Not tested: real widgets, low-end HW, multi-monitor, hours-long leak check. See `spikes/webview-probe/RESULTS.md`.
- [~] Spike C: srcdoc sandbox alone leaks network and starves the host (7/8 runs). **Per-origin custom-protocol delivery + header CSP + host frame-src: 0 server hits, hog contained (5/5), about +29 MB and +1 process per widget.** Next: hybrid design (first-party shared origin, third-party own origin), memory-bomb test, wire broker to the real host. See `spikes/sandbox-probe/RESULTS.md`.
- [ ] Interviews and landing page (non-code, see design docs); add a "post your setup card" test to measure copy-culture on Windows
- Skipped by decision: hands-on Seelen UI test.

- [x] host-demo: plinth-host + WebView2 end to end, 11/11 checks (see `apps/host-demo/RESULTS.md`)
- [x] Spike D: `plinth-proxy` deny-by-default proxy built; 4/4 end-to-end runs with 0 direct connections (see `apps/host-demo/RESULTS.md`). Still to do: HTTPS allow-path test, per-widget attribution, `wss`, WebView2 service traffic allow-list, production wiring (start proxy with the host, update allow-list on grant change)
- [ ] Name clearance and public brand (decision D2)

- [x] First runnable app (`apps/plinth-app`, v0.5.0): hub, desktop layer, widgets, broker, proxy, persistence; autotest 10/10

## Release plan toward the all-in-one app (decision D22)
- v0.7.0 Monitoring pillar: user-mode hardware sampler (CPU, RAM, GPU via NVML, network, disk), PC Vitals and Build Card widgets, shared first-party origin (cheaper), security-review gate.
- v0.8.0 Wallpaper pillar: live video/web wallpapers in the layer with pause rules, per-monitor targeting, Impact Meter fed by the sampler.
- v0.9.0 Security pillar: Security Center (Windows security status, per-process connections, startup items, blocked-attempt log), proxy gaps closed.
- v0.10.0 Moods + Setup Cards import/export + Builds showcase; then name clearance, signed installer, store.

## Alpha (MVP, about 4–6 months after Phase 0)
Window-layer manager, widget host with permission bridge, Permission Center UI (grants, activity log, revoke, Safe mode), 10 first-party widgets, Scenes (local), remix editor, minimal pause governor, theme tokens, revert/profile export, accessibility baseline, signed installer. Gate: Windows test matrix passes; benchmark suite in CI.

## Beta (v1.0, about 4–6 months after Alpha)
Full visual builder, curated store with review pipeline and signed packages, creator payouts via a merchant-of-record provider, animated wallpapers, auto-update with rollback, multi-monitor/HDR polish. Gate: no malware listing live over 24 h; permission-diff on updates enforced.

## v1.0 release criteria
Zero known sandbox escapes, benchmark targets met on a reference low-end laptop, 30 or more launch-quality listings, documented security model.

## Later
Cloud sync, AI layout helper, Rainmeter import helper (feasibility UNVERIFIED), other OSes.
