---
name: plinth-spike
description: How to run and record an engineering spike or experiment for the Plinth project (window layer, WebView2 memory, sandbox, performance). Use when asked to prototype, measure, benchmark, or test a technical assumption in plinth/spikes, or to write a RESULTS.md.
---

# Plinth spike protocol

A spike answers one question with evidence, on a branch, and leaves a result file. It is not production code.

## Steps
1. **Branch**: `spike/<name>` from `main` (see `CONTRIBUTING.md`).
2. **Hypothesis and exit criterion** in the spike's `RESULTS.md` before running (what number or behavior counts as pass).
3. **Calibrate the instrument**: prove the measurement can see a positive case (for example the pixel sampler read 9/9 on an always-on-top window before it was trusted on the desktop layer). A failing result with an unvalidated instrument is not a result.
4. **Repeat**: at least 3 runs for anything timing- or process-dependent; report counts like "7 of 8", including the outlier. Do not average away a contradicting run.
5. **Safety**: run only your own test widgets against a local server on 127.0.0.1; never contact third parties; avoid prompts that touch the user's camera, clipboard, or downloads; do not lock, sleep, or restart the user's machine without asking.
6. **Record** in `RESULTS.md`: date, machine (OS build, cores), tool versions, exact command, raw numbers, what was NOT tested, and a verdict that separates OBSERVED from INFERENCE.
7. **Fold back**: update `design/architecture.md`, `plinth/ROADMAP.md` checkboxes, and `CHANGELOG.md` `[Unreleased]`. Keep budgets honest: revise targets that the data contradicts.
8. **Commit** small and conventional (`spike: ...`), push the branch, merge to `main` only through the release flow in `CONTRIBUTING.md`.

## Measurement notes learned so far
- WebView2 process tree: sum private bytes and working sets over descendants of the probe; working-set sums double-count shared pages. A PowerShell measurer adds a constant to the tree.
- `GetPixel` on the screen DC reads what the user sees; window flags like IsWindowVisible lied on this Windows build.
- Cumulative CPU delta over 30 s after a 20 s warmup is the idle CPU figure; startup CPU is not idle.
