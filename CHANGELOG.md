# Changelog

All notable changes to this project are recorded here. Format: [Keep a Changelog](https://keepachangelog.com/en/1.1.0/). Versioning: [Semantic Versioning](https://semver.org/), see `CONTRIBUTING.md`. Pre-1.0 (`0.y.z`): anything may change; the minor number moves with each completed milestone.

## [Unreleased]
### Added
- Spike C scaffold: `plinth/spikes/sandbox-probe` (hostile-widget attacks against sandboxed iframes). Not yet run.

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
