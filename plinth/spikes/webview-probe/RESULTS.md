# Spike B results: WebView2 memory/CPU with sandboxed widgets

Date: 2026-10-30. Machine: Windows 11 Home build 26300, 28 logical cores (a strong machine; low-end hardware NOT measured), 1 monitor 1920x1080. Release build, wry 0.57.1, WebView2 runtime 156.0.4314.50. Method: `measure.ps1` starts the probe attached by the `progman` strategy, waits 20 s, samples the process tree (probe + all descendant `msedgewebview2.exe`), waits 30 s, samples again. Memory = sum of private commit and sum of working sets over the tree; CPU = delta of cumulative CPU time over the 30 s window. **One run per configuration; expect roughly +-10 MB noise.**

| Config | Processes | Private (MB) | Working-set sum (MB) | Idle CPU (% of one core) |
|---|---|---|---|---|
| blank page | 8 | 146.0 | 350.5 | 0.16 |
| 5 widgets, sandboxed iframes (`sandbox="allow-scripts"`, opaque origin) | 8 | 158.7 | 359.2 | 0.21 |
| 5 widgets, inline in one page | 8 | 150.1 | 346.3 | 0.36 |
| 10 widgets, sandboxed iframes | 8 | 166.0 | 359.1 | 0.78 |

Each widget is a tiny clock that updates once per second (light but non-zero activity). Real widgets (charts, images, fonts, network) will cost more.

## Findings
1. **Fixed WebView2 overhead dominates: about 146 MB private / 350 MB working set / 8 processes before any widget.** Each extra tiny iframe widget added only about 2-2.5 MB private (blank to 5 iframes: +12.7 MB; to 10: +20 MB). OBSERVED.
2. **The design target "<150 MB per monitor with 5 widgets" is NOT met** (158.7 MB private with 5 trivial widgets; and a working-set view is 2x higher). A realistic target on this evidence is **about 200 MB private for a monitor hosting 5 light widgets**, to be re-measured with real widgets. Multi-monitor: does one WebView2 per monitor double the fixed cost? NOT TESTED; a single WebView2 spanning the virtual desktop is an alternative to test.
3. **Idle CPU is negligible:** 0.2-0.8% of one core (<0.05% of this CPU). On a low-end 2-4 core laptop the percentage of total would be larger, but still small for these widgets. Low-end hardware NOT TESTED.
4. **Sandboxed iframes do not add processes** (8 processes in every configuration). So widget isolation is **web-level only (opaque origin, no same-origin access), not process-level**: all widgets share one renderer process in this setup. A renderer exploit or a side channel (e.g. Spectre-class) could cross widgets. The architecture's security section should say this plainly; per-widget processes would multiply the ~100+ MB fixed overhead? NOT MEASURED (only the browser-process share is fixed; extra renderers are probably tens of MB each, UNVERIFIED).
5. Inline vs iframe: memory within noise; inline showed higher CPU (0.36 vs 0.21) in this run, which is also within plausible noise. Do not conclude iframes are cheaper or dearer.
6. Show-Desktop (Win+D) test with 5 iframe widgets, release build: **PASS 9/9**. OBSERVED.

## What this means for the design
- Keep WebView2; the cost is mostly fixed, so cost per widget is small. Marketing must not claim "tiny memory footprint": a Rainmeter skin set will likely use far less (not measured here, UNVERIFIED).
- Consider a **lazy WebView**: don't create the host until at least one widget is enabled, and destroy it during game/fullscreen pause (the governor can free ~150 MB, not just stop timers). Untested.
- Consider a **declarative renderer without a web runtime** for first-party widgets (Direct2D/native) only if the fixed overhead proves unacceptable on low-end hardware; this is the existing fallback in `architecture.md`.
- Update `architecture.md` budget: private memory per monitor "about 200 MB with 5 light widgets" (target, to be re-measured), plus a note that isolation is not process-level.

## Not tested
Real widgets, release vs debug beyond this run, idle after hours (leaks; compare Seelen UI's early WebView2 growth issue #1081), low-end hardware, multi-monitor, GPU usage, battery impact, lock/sleep/display/DPI changes, fullscreen-game behavior.
