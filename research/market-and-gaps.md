# Market and Gap Analysis (accessed 2026-10-30)

Evidence tags: **FACT** (sourced, see `sources.md`), **INFERENCE** (my judgment), **UNVERIFIED**. Where I lacked evidence I say so; the ranking below is therefore *evidence-weighted judgment*, and the validation plan at the end is what turns it into data.

## 0. Update after gap-closing (2026-10-30) — read this first
New facts that change the analysis (details in `competitor-analysis.md` §00):
- **Rainmeter 5.0 (Oct 12, 2026)** adds high-DPI, per-skin zoom, SVG/TextEdit meters, lower resource use, hidden-skin culling, Safe Start. **Gap impact:** Gap 6 (perf governor) and Gap 8-ish DPI polish are **partly closed** for Rainmeter users; Gaps 1, 2, 4, 5, 7 remain open (no store, sandbox, builder or onboarding in the changelog).
- **Seelen UI** (free, AGPL, Rust+Tauri, ~18.3k stars, Store/winget) is a credible, growing free competitor with widgets, CSS themes and a Tauri/WebView2 stack. **Gap impact:** Gap 4 (hub) and Gap 8 (modern look) are **partly closed** by it; it is shell-replacement-based so Gap 3 (update resilience) is still open for it (INFERENCE), and a dedicated safe store / creator payouts (Gaps 2, 7) are **not evidenced** (UNVERIFIED).
- **Demand sizes (W):** r/unixporn ~641k (+17%/yr), r/Windows11 ~281k, r/Rainmeter ~322k (+0.7%/yr), r/desktops ~141k [S6, S32]; Wallpaper Engine Workshop ~2.87M entries; owners est. 20–50M [S33]. **Interpretation (INFERENCE): the fast-growing interest is "ricing"/setup culture (unixporn-style), Rainmeter is flat, so the target is the new-entrant who wants a rice without config files.**
- **Platform precedent:** Microsoft killed Gadgets in 2012 over security [S34].
- **Still UNVERIFIED:** YouTube/TikTok view data, Google Trends, DeviantArt counts, direct user quotes, XWidget/Cairo Shell/Conky status, what users would pay.

**Revised thesis:** The opportunity is narrower than first written. "Rust/Tauri widget platform" is not an edge (Seelen UI) and "free, polished desktop widgets" is partly served (Rainmeter 5.0). The defensible wedge is **trust + content economy + update-resilience**: a sandboxed, permissioned widget model with a curated store and creator payouts, delivered without replacing the shell, aimed at setup-culture newcomers. If a 2-week survey of Seelen UI shows it already has a vetted store and permissions, this wedge shrinks and the right move may be to contribute to or partner with it rather than compete (decision for the user).

## 1. Segments (all INFERENCE unless noted)

| Segment | Wants | Frustrated by |
|---|---|---|
| Casual aesthetic | A beautiful desktop in minutes | Rainmeter setup, finding skins, breakage |
| Power users | Data widgets, scripting, control | Updates breaking things, closed tools |
| Streamers/creators | A "setup" worth showing; consistency across scenes | Fragmented tools, performance hit on stream PC |
| Gamers | Looks without FPS loss | Fear of overhead; skins not pausing |
| Productivity | Fences-style organization, calendar/tasks widgets | Subscription licensing, Win11 shell breakage |
| Theme creators | Reach, feedback, income | No store, no payout, copy/rip-off, scattered hosting |

## 2. Where demand is visible
- Wallpaper Engine: 234,819 English Steam reviews at 97% positive (all languages ≈ 925k–978k per secondary sources). Strongest demand evidence in category. FACT/W [S9, S10]
- Lively: ~20.0k GitHub stars, 1.4k forks. FACT [S12]
- r/Rainmeter ≈322k members, undated, third-party tracker, ~0.7%/yr growth. W [S6] — **Interpretation (INFERENCE): a large but flat community; interest is stable, not exploding.**
- Rainmeter 5.0 in the pipeline (64-bit only) shows the incumbent engine is still maintained but slow. FACT [S1]
- Not obtained (UNVERIFIED): Steam Workshop subscriber totals, YouTube/TikTok "desktop setup" view counts, Google Trends, DeviantArt counts, r/desktops and r/Windows11 sizes. **Action: collect these before spending money.**

## 3. Pricing and monetization audit
| Model | Example | Evidence |
|---|---|---|
| One-time cheap | Wallpaper Engine (CDN$6.49 on fetch) | FACT [S9] |
| Free + FOSS | Rainmeter, Lively | FACT [S1, S12] |
| Per-app + bundle + subscription | Stardock ($9.99 apps, ~$39.99 suite in 2022 post; subscription lapses disable software) | S, dated, UNVERIFIED current [S14, S15, S19, S20] |
| Free + donations | Rainmeter skin creators | UNVERIFIED specifics |
| Creator store with revenue share | **None found in this category** | INFERENCE (absence of evidence in my searches, not proof) |

Complaint pattern (INFERENCE + S15/S17): users tolerate $5–10 one-time prices; friction comes from license complexity and "software stops working if I stop paying." What users say they'd pay for: not found in sources. UNVERIFIED.

## 4. Gap matrix
✔ = does well, ◐ = partial, ✘ = weak/absent, ? = unverified. "Nobody" = no incumbent does it well.

| # | Need / pain | Rainmeter | Stardock | Wallpaper Engine | Lively | Nobody does well? |
|---|---|---|---|---|---|---|
| 1 | Beautiful desktop in <10 min, no tutorial | ✘ | ◐ | ✔ (wallpapers only) | ◐ | **Yes** for full desktops |
| 2 | One hub: wallpaper + widgets + layout + theme | ✘ | ◐ (bundle of separate apps) | ✘ | ✘ | **Yes** |
| 3 | No-code visual widget builder | ✘ | ✘ | ◐ (wallpaper editor) | ✘ | **Yes** |
| 4 | One-click discover/install/update of themes+widgets | ✘ | ◐ | ✔ (Workshop) | ◐ | Partly; nobody for widgets |
| 5 | Safe third-party content (permissions, sandbox, signing) | ✘ | ✔ (closed) but no ecosystem | ✘ (incident) | ? | **Yes** |
| 6 | Creator revenue share / paid marketplace | ✘ | ? | ? | ✘ | **Yes** |
| 7 | Auto-pause/throttle for games, battery, fullscreen | ◐ (game mode, details ?) | ? | ✔ | ✔ | Widgets specifically: **Yes** |
| 8 | Survives Windows feature updates | ◐ (patched after) | ✘ (24H2 breakage) | ✔ ? | ? | **Yes** (esp. shell-hook tools) |
| 9 | Native Win11 look (Mica, rounded, theme/accent follow) | ✘ | ◐ | n/a | ◐ | Yes |
| 10 | Multi-monitor / HDR / DPI correctness | ◐ | ? | ✔ | ✔ | Partly; HDR UNVERIFIED |
| 11 | Accessibility (scaling, contrast, reduced motion) | ✘ | ? | ✘ | ? | **Yes** |
| 12 | Settings/profile sync and sharing "my setup" | ✘ | ✘ | ◐ | ✘ | **Yes** |
| 13 | Per-monitor widget placement that survives plug/unplug, resolution and DPI changes | ✘ (documented failure: skins pile onto one monitor and autosave overwrites good positions [S52]) | ? | n/a | n/a | **Yes** (evidence is user reports only) |
| 14 | Wallpaper library (folders, favorites, per-monitor, slideshow) in the same place as widgets | ✘ | ◐ (DeskScapes, separate app) | ✔ (wallpapers only, no widgets) | ◐ | Partly: the wallpaper half exists elsewhere; the **combination with widgets and profiles** does not |

## 5. Failure modes to avoid (from Phase 1)
1. Performance bloat from poorly built community content (Rainmeter visualizers). [S4]
2. Breakage when Windows ships feature updates, especially for shell-hooking tools. [S1, S17]
3. Fragmented tools that don't work together (Stardock suite; Rainmeter+WE+Fences stacks). INFERENCE.
4. Steep learning curve / text-config authoring. INFERENCE.
5. Unvetted open marketplace becoming a malware vector. [S2, S10]
6. Licensing that confuses users or disables software on lapse. [S14, S15]
7. Antivirus false positives from unsigned binaries. [S1, S8]
8. Single-maintainer/dying projects (Samurize, ObjectDock history). [S21, S22] INFERENCE on cause.
9. Ugly default experience — Rainmeter ships bare starter skins. [S3]

## 6. Ranked top-10 gaps
Score = breadth across segments × incumbents failing × strength of evidence. Evidence strength: A = direct source, B = reasoned from source, C = my judgment only.

| Rank | Gap | Evidence | Strength |
|---|---|---|---|
| 1 | Time-to-first-great-desktop (curated, one-click full-desktop themes) | Rainmeter has no catalog; content off-platform [S3]; WE's success shows users buy "instantly beautiful" [S9] | B |
| 2 | Safe, verified third-party widgets (permissions + signing) | Rainmeter no sandbox + RunCommand risk [S2, S7]; WE Workshop incident [S10] | A/B |
| 3 | Win-update-resilient architecture (no shell injection) | 24H2 breaks [S1, S16, S17] | A |
| 4 | Unified hub instead of 4–5 apps | Stardock = many apps [S18]; users stack WE+Rainmeter+Fences (INFERENCE) | B/C |
| 5 | No-code visual widget builder | Rainmeter .ini barrier [S2/S3] (INFERENCE on difficulty) | B/C |
| 6 | Widget-aware performance governor (game/battery/fullscreen pause) | CPU complaints [S4]; Lively/WE already pause wallpapers [S11, S12] | B |
| 7 | Creator revenue share + quality marketplace | No store found in category; creators distribute on DeviantArt/forum [S3] | B (absence) |
| 8 | Native Win11 aesthetics + theme/accent following | Rainmeter skins are bespoke bitmaps/text (INFERENCE) | C |
| 9 | Accessibility (scale, contrast, reduced motion) | No evidence of coverage in any competitor | C |
| 10 | Shareable "setup" profiles / sync | No competitor feature found | C |

## 7. Opportunity thesis
**The underserved opportunity is a safe, update-proof, no-code desktop-widget-and-theme platform with a built-in curated store, for casual-aesthetic users, streamers and theme creators, because Rainmeter has the creativity but no onboarding, safety or store; Stardock has polish but fragmented, update-fragile, license-complicated apps; and Wallpaper Engine/Lively prove the demand but stop at wallpapers.** (INFERENCE grounded in the sources above.)

## 8. SWOT of the market (incumbent landscape)
- **Strengths:** Strong, proven demand (WE reviews, Lively stars, Rainmeter community). Cheap price points are accepted.
- **Weaknesses:** Fragmentation, setup friction, no safe store for widgets, update fragility.
- **Opportunities:** Rainmeter 5.0 transition/64-bit cutoff may unsettle users; Microsoft Widgets is shallow; creators lack a payout path.
- **Threats:** Microsoft may add desktop widgets natively; Rainmeter 5.0 may fix onboarding (UNVERIFIED); WE's brand + Steam distribution; a store cold-start problem.

## 9. Validation plan (do before building past MVP)
1. 15 interviews: 5 Rainmeter users, 5 WE/Lively users, 5 theme creators.
2. Collect missing numbers: Workshop subscribers, DeviantArt Rainmeter counts, YouTube "desktop setup" views, Google Trends.
3. Landing page + waitlist (free) to measure sign-up rate from r/Rainmeter, r/desktops, r/Windows11 posts (read subreddit rules first).
4. Pricing test with 3 price points on the waitlist survey.

## 10. Addendum (2026-10-30): wallpaper management and multi-monitor placement
Full evidence and API verification: `research/monitors-and-wallpapers.md`. Short version: these are **real but narrow gaps**. Per-monitor wallpapers already exist in Windows, Wallpaper Engine and DisplayFusion [S53, S54]; what is missing is (a) placement that survives display changes (Rainmeter's documented failure is a persistence bug: transient wrong monitor positions get autosaved [S52]) and (b) one hub that stores wallpapers, widgets and monitor assignments in one profile. Evidence strength: **C** (user reports and weak how-to pages, no demand numbers). They are added as gap rows 13 and 14 and fold into top-10 gaps #4 (unified hub) and the multi-monitor part of the matrix. For the PC-builder beachhead (D16) multi-monitor is common, so the build priority is high even though the evidence is thin; the validation plan (section 9) should ask builders directly.
