# Spike A results: desktop-layer probe

Date: 2026-10-30. Machine: Windows 11 Home, build 10.0.26300, single 1920x1080 monitor, 100% scale assumed (primary reported 1920x1080 after `SetProcessDPIAware`). Tool: `cargo run -p layer-probe -- <strategy>`; raw Win32 FFI, no dependencies. Each run draws a 480x320 magenta window, attaches it, sends Win+D, samples 9 real screen pixels (what the user sees), sends Win+D again.

## Method validity
- Calibration: strategy `topmost` (plain always-on-top window, no Win+D) read **9/9 magenta pixels**, so the pixel sampler and painting work. OBSERVED.
- Win+D itself worked: the center pixel changed from the covering app's color (0x191919) to the wallpaper color (0x3c2228). OBSERVED.

## Results (one run each)
| Strategy | Attach result | Magenta visible after Win+D |
|---|---|---|
| `workerw` (child of the WorkerW that is itself a child of Progman) | attached (parent class WorkerW) | **FAIL 0/9** |
| `progman` (child of Progman, z-order behind icons) | attached | **FAIL 0/9** |
| `progman-top` (child of Progman, above icons) | attached | **FAIL 0/9** |
| `progman-layered` (as `progman` + WS_EX_LAYERED) | attached | **FAIL 0/9** |
| `progman-direct` (created as child of Progman, no popup-to-child conversion) | attached | **FAIL 0/9** |
| `bottom` (HWND_BOTTOM tool window) | n/a | **FAIL 0/9** (window flag says visible, but pixels show wallpaper) |

## Layout observed on this build (OBSERVED)
- `SHELLDLL_DefView` is a **child of Progman** (class Progman), not of a WorkerW.
- After message 0x052C, a `WorkerW` exists as a **child of Progman**, z-order directly below DefView, both rect (0,0,1920,1080).
- So the classic "find the WorkerW sibling" method does not apply here; the 24H2-style layout does.

## What this means
1. **No strategy I tried makes a GDI-painted window visible on the desktop on this build.** Every attach call succeeded and flags said visible, but pixels never showed the window. Visible flags are therefore not a reliable health signal; pixel/screenshot checks are needed in CI. This also reinforces the design's reason for a test gate on Windows updates.
2. I did NOT establish why. Hypotheses (all UNVERIFIED): (a) on this build the desktop children are composed through a surface that ignores plain GDI children, and apps such as Lively/Wallpaper Engine succeed by rendering via a compositor-backed surface (e.g. a WebView2/DirectComposition window) rather than GDI; (b) build 26300 may be a preview build with different behavior from released 24H2/25H2; (c) a flaw in my probe I could not find.
3. `bottom` failing is plausible by design: Show Desktop hides or minimizes ordinary top-level windows. That only means the fallback is useless *for the Win+D case*, not unusable overall.

## Not tested (marked NOT TESTED)
Lock/unlock, sleep/resume, display hot-plug, DPI change, fast user switching, multiple monitors, other Windows builds, a compositor-backed (WebView2) child window, explorer.exe restart.

## Next experiments (smallest first)
1. Re-run with the real renderer: create the host as a **WebView2** (or DirectComposition) child of Progman and repeat the pixel check. If it passes, the WebView2 host is the right layer and GDI is the wrong probe. This is Spike B's first step, so merge A and B.
2. Run the same probe on a stable (non-Insider) Windows 11 24H2/25H2 machine or VM to separate "this build" from "all builds".
3. Check what Lively's open-source code does for 24H2 (read only; it is GPL-3.0, so no code is copied).
4. If nothing works reliably: fall back to a top-level borderless window pinned to the bottom with Show-Desktop-survival via `WS_EX_TOOLWINDOW` plus periodic z-order repair, and accept a Win+D limitation, or require "widgets above windows" mode.

## Verdict (GDI-only stage; superseded by "Revised verdict" at the bottom)
Spike A **did not pass and is not a clean fail either**: the matrix is partial and the root cause is unknown. Risk "desktop layer breaks on current Windows" is **confirmed as real on this machine**; the mitigation path (compositor-backed host) is untested.

---

# Update: WebView2 host probe (crate `spikes/webview-probe`, wry 0.57.1)

Same harness, but the host window (filled BLUE with GDI so it can't be mistaken) contains a **WebView2 child showing a solid magenta page**. Only the WebView content can read as magenta.

| Strategy | Magenta visible after Win+D |
|---|---|
| `topmost` (calibration, no Win+D) | 9/9 (WebView renders and sampler works) |
| `workerw` | **PASS 9/9** |
| `progman` (behind icons per z-order insert) | **PASS 9/9** |
| `progman-top` | **PASS 9/9** |
| `progman-direct` | **PASS 9/9** |
| `bottom` (HWND_BOTTOM top-level) | FAIL 0/9 (Win+D hides it, as expected) |

**Finding (OBSERVED, one machine, one run each):** on build 26300 a WebView2-backed child of Progman/WorkerW stays visible through Win+D, while the same host with GDI content does not. This supports hypothesis (a): the desktop layer composes compositor-backed surfaces. The design's WebView2 host choice is therefore consistent with what works here. Caveats:
- The blue GDI host itself was not visible in earlier GDI tests; here only the WebView is. Not tested whether the WebView stays correct after lock, sleep, display change, DPI change, explorer restart, or on other builds (all NOT TESTED).
- Pixel sampling cannot distinguish "behind icons" from "above icons" (`progman` vs `progman-top` both read 9/9). Icon z-order is NOT VERIFIED; the z-order list shows DefView above WorkerW and the probe inserted after DefView, which is the intended position.
- Click-through/input behavior untested.

## RAM for one trivial WebView2 page (OBSERVED, debug build, ~15 s after start)
Process tree under the probe (descendants only): 1 host exe + 6 `msedgewebview2.exe`.
- Sum of working sets: **314.7 MB** (double-counts shared pages).
- Sum of private bytes: **135.5 MB**.
- Cumulative CPU about 1.0 s during startup (this is not an idle CPU figure; no idle measurement taken).
- Single page, single monitor, no widgets. **The design target "<150 MB per monitor with 5 widgets" is NOT validated: one blank page already uses ~135 MB private.** Treat the budget as at risk; options are fewer processes (shared environment is already used here), declarative widgets rendered without a web runtime, or relaxing the target to something measured honestly (for example "<250 MB private with 5 widgets") after Spike B measures five real widgets, and a release build.

## Revised verdict
Spike A: **partial pass.** The WebView2 route survives Win+D on this build; the GDI route does not. Remaining matrix (lock, sleep, display/DPI, explorer restart, multi-monitor, other builds, icon z-order, input) is still open.
