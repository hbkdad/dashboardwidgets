# Desktop Customization: Research + Product Design

Date: 2026-10-30. Folder map: `research/` (competitors, market and gaps, sources), `design/` (PRD, architecture, business and roadmap).

## Opportunity thesis
The underserved opportunity is a **safe, update-proof, no-code desktop-widget-and-theme platform with a built-in curated store**, for casual-aesthetic users, streamers and theme creators. Rainmeter has creativity but no onboarding, safety or store; Stardock has polish but fragmented, update-fragile, license-confusing apps; Wallpaper Engine and Lively prove demand but stop at wallpapers. (Judgment grounded in sourced facts; see `research/market-and-gaps.md`.)

## The product: Plinth (working name; trademark UNVERIFIED)
**Decision made: compete on the narrowed wedge** (safety, no-code builder, creator payouts, no shell replacement).
- **Permission Center:** every widget declares what it can touch; you see, log and revoke it; Safe mode.
- **Sandboxed widgets:** permission-gated JS/declarative widgets; no native code, no shell hooks.
- **Scenes + remix editor:** one-click curated bundles; edit any declarative widget without code.
- **Update-resilient layer:** abstraction with fallbacks, Insider CI, remote kill-switch, strict Win+D/sleep/DPI test gate.
- **v1.0 adds** the full drag-and-drop builder and a curated store with creator payouts.

## MVP scope
Permission Center, sandboxed widget runtime, Scenes, remix editor (builder-lite), minimal pause governor, window-layer manager, 10 first-party widgets, Win11 theme tokens, revert/profile export, accessibility baseline; local gallery only. Stack: Rust core + Tauri 2 + WebView2 + Svelte. No store, full builder, AI, cloud or other OSes in MVP.

## What the research could and couldn't establish
- Solid: Rainmeter release cadence, 5.0 going 64-bit only, .rmskin and plugin risk, Wallpaper Engine review counts, Lively stats, Stardock 24H2 breakage and license behavior.
- **Gap-closing update (2026-10-30), read `market-and-gaps.md` §0:** Rainmeter 5.0 shipped Oct 12, 2026 (high-DPI, SVG meters, lower resource use, still no store/sandbox). **Seelen UI** (free, AGPL, Rust+Tauri, ~18.3k stars) is a direct competitor on my chosen stack, but it replaces the shell and I found no vetted store or permission model. So the thesis is narrowed to **trust + creator economy + update-resilience**, not "modern widgets in Rust."
- **Seelen UI audit done (`research/seelen-ui-audit.md`):** it already has a catalog, paid tier ("Seelen Sky") and an all-in-one hub, so "hub," "perf governor," "modern look" and "Rust/Tauri" are no longer differentiators. Open wedge: **provable widget safety** (no documented per-widget permission model; Store build is runFullTrust per a competitor's claim), **no-code builder**, **creator payouts**, **no shell replacement**. Its open issues cluster on Win key / Win+D / taskbar behavior. Not yet hands-on tested: a 1-day VM test is the next check.
- **Correction from the source-reading test (`research/seelen-ui-handson-test.md`):** Seelen UI v2.8.10 (Oct 22, 2026) already has per-widget permission prompts (added early Oct 2026). "Safe widgets" is now a **partly contested** wedge; the open part is finer scoping (network/file), an activity log, and reviewed/signed publishing. Nothing was installed or run: the hands-on measurements (RAM, Win+D, hostile widget) are still NOT TESTED and waiting on your approval of an isolation setup and the 48.7 MB installer.
- **Decision (2026-10-30): the hands-on Seelen UI test is skipped.** Seelen runtime claims (RAM, Win+D, hostile widget) stay NOT TESTED; launch messaging must not make claims about Seelen UI's runtime behavior. Own Phase 0 prototype tests (WebView2 RAM, Win+D matrix, hostile-widget suite against **Plinth's own** sandbox) still apply.
- Newly sourced: subreddit sizes (r/unixporn ~641k and growing, r/Rainmeter ~322k and flat), Workshop ~2.87M entries, Nexus Dock still active (June 2026), Microsoft killed Gadgets over security in 2012.
- **Still UNVERIFIED:** current Stardock consumer prices, YouTube/TikTok demand data, XWidget/Cairo Shell/Conky status, direct user quotes, why Samurize/ObjectDock-2.x faded, whether Seelen UI has a store or sandbox.
- The Wallpaper Engine Workshop-closure story rests on one low-quality outlet.

## 3 decisions I need from you before any code
1. **Stack/risk gate:** approve a 2-week Phase 0 first: WebView2 memory, WorkerW/Win+D behavior on your Windows 11 build, a hostile-widget sandbox test, and a hands-on Seelen UI VM test that confirms or refutes the "no per-widget permissions" claim (if the claim fails, the wedge must be revisited).
2. **Business model:** open-source the engine core (GPL/MIT, trust + bus-factor benefit, harder to monetize) vs. closed source with a free tier? And is a payments/store backend (tax, KYC, ~20% cut) in scope for v1.0, or do you want merchant-of-record only?
3. **Scope/name:** is the primary user casual aesthetic users and streamers (this design), or Rainmeter power users (a different, builder-lite product)? And pick a name to run trademark clearance on (Plinth / Tessel / Dwell).

Note on your second prompt: its phase-pause protocol and file names (`01_...md`) were not used; I followed the first prompt's deliverable list. Its extras (SWOT, AI-assisted layouts, node-based scripting) are covered or deliberately deferred (see PRD §5.3).
