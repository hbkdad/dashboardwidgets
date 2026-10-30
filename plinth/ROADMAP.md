# Engineering Roadmap

Mirrors `../design/business-and-roadmap.md` §5. Targets are planning assumptions, not measurements. Time estimates for a solo developer are my inference.

## Phase 0: Validation spikes (about 2–4 weeks)
- [x] Core logic crate: manifest, broker, governor (done, 12 tests)
- [~] Spike A: GDI host FAILS Win+D on build 26300; **WebView2 host PASSES** (workerw/progman strategies, 9/9 pixels). lock/sleep/display/DPI/icon z-order/input/other builds NOT TESTED. See `spikes/layer-probe/RESULTS.md`.
- [~] Spike B: release build, 5 sandboxed iframe widgets = 158.7 MB private / 359 MB WS sum / 8 processes / 0.21% of one core idle (blank page 146 MB); 10 widgets 166 MB. Fixed WebView2 cost dominates, so <150 MB target NOT met. Iframes add no processes (web-level isolation only). Not tested: real widgets, low-end HW, multi-monitor, hours-long leak check. See `spikes/webview-probe/RESULTS.md`.
- [~] Spike C: hostile suite run (see `spikes/sandbox-probe/RESULTS.md`). Sandbox alone leaks network and remote code; sandbox + generated CSP + host `frame-src about:` blocked all tested network channels; storage/DOM isolation held; **busy-loop widget freezes host and other widgets (7/8 runs)**. Next: per-widget origin/process isolation test and a CPU watchdog; memory-bomb test.
- [ ] Interviews and landing page (non-code, see design docs)
- Skipped by decision: hands-on Seelen UI test.

## Alpha (MVP, about 4–6 months after Phase 0)
Window-layer manager, widget host with permission bridge, Permission Center UI (grants, activity log, revoke, Safe mode), 10 first-party widgets, Scenes (local), remix editor, minimal pause governor, theme tokens, revert/profile export, accessibility baseline, signed installer. Gate: Windows test matrix passes; benchmark suite in CI.

## Beta (v1.0, about 4–6 months after Alpha)
Full visual builder, curated store with review pipeline and signed packages, creator payouts via a merchant-of-record provider, animated wallpapers, auto-update with rollback, multi-monitor/HDR polish. Gate: no malware listing live over 24 h; permission-diff on updates enforced.

## v1.0 release criteria
Zero known sandbox escapes, benchmark targets met on a reference low-end laptop, 30 or more launch-quality listings, documented security model.

## Later
Cloud sync, AI layout helper, Rainmeter import helper (feasibility UNVERIFIED), other OSes.
