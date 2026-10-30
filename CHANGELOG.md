# Changelog

All notable changes to this project are recorded here. Format: [Keep a Changelog](https://keepachangelog.com/en/1.1.0/). Versioning: [Semantic Versioning](https://semver.org/), see `CONTRIBUTING.md`. Pre-1.0 (`0.y.z`): anything may change; the minor number moves with each completed milestone.

## [Unreleased]

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
