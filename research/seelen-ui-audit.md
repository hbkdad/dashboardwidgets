# Seelen UI Audit (accessed 2026-10-30)

Tags: **FACT** (read directly), **CLAIM** (stated by a party with an interest), **INFERENCE**, **UNVERIFIED**. I did not install or run Seelen UI, read its source, or measure anything myself. The DeepWiki security page returned HTTP 429, so source-level permission details are unread.

## 1. What it is
Free (AGPL-3.0), ~18.3k stars, Rust + Tauri + WebView2, Svelte/TS widget SDK; toolbar/dock, launcher, tiling WM, desktop widgets, CSS/JSON themes, multi-monitor. Ships via Microsoft Store, winget, GitHub (signed through SignPath). [S29] FACT. Its FAQ says it "adds a customization layer on top" of Windows rather than fully taking over the desktop [S38] CLAIM, but the repo describes a taskbar/Start-menu replacement [S29] FACT, so it hooks shell behavior in practice (INFERENCE).

## 2. Store / resources
- Community catalog on seelen.io with six resource types (themes, widgets, wallpapers, icon packs, plugins, sound packs), installed from the website; authors bundle a YAML via CLI and upload through the site. [S39] CLAIM (competitor's description).
- **Some items are subscriber-only**, and **Seelen Sky** (monthly/annual) sells cloud save/sync, exclusive widgets/themes and priority support. [S39] CLAIM; confirmed by FAQ mentioning optional paid plans with "exclusive resources." [S38] FACT. **Correction to my earlier note: Seelen already has a paid tier and a gated-resource model.** Whether creators get a revenue share: UNVERIFIED (nothing found).
- Review/vetting of community resources: the FAQ says nothing; it says resources are safe "if downloaded from official Seelen sources." [S38] FACT. No documented review process found.

## 3. Permissions and sandbox
- Widgets are web apps calling app commands via a typed `invoke` wrapper; the competitor's comparison says docs describe **no permission model, grant step, or restrictions on files/network/processes**, and the Store build declares **runFullTrust**. [S39] CLAIM (unverified by me; the competitor has an incentive).
- Only toolbar plugins are described as sandboxed (three whitelisted operations). [S39] CLAIM.
- DeepWiki summary: Tauri capability-based permissions for UI components, WebView integrity checks, MSIX sandbox detection. [S40] S/W (AI-generated wiki, not official).
- An old release note lists a fix where third-party widgets could change settings or control windows without asking. [S40] S; age ~2 years, current state unknown.
- **Net (INFERENCE):** there is no evidence of a per-widget, user-visible permission model for community widgets. This is the thing my design offers. Confirming requires reading the repo's capability files and a hands-on test with a hostile test widget.

### 3.1 CORRECTION from source reading (see `seelen-ui-handson-test.md`, 2026-10-30)
Reading v2.8.10 (released 2026-10-22) source, not running it: Seelen UI **does** have a per-widget permission layer, added about 3–4 weeks earlier (commits dated 2026-10-05 and 2026-10-08). Five permissions (`Run`, `OpenFile`, `ModifyConfiguration`, `KillProcesses`, `ManageAppWindows`), a blocking Yes/No prompt for third-party widgets, decisions persisted per widget, and a settings "security tab." The competitor's "no permission model" claim was probably true earlier and is now outdated. Still true: Store build declares runFullTrust (host app, normal for MSIX); no review/signing of community resources documented; permissions undocumented on the widget docs pages read; network/file access from web content untested (INCONCLUSIVE); tauri.conf.json shows no `csp` key. **G2 is therefore partly closed, not open.** Remaining wedge: finer scoping (network hosts, file access), a visible Permission Center with activity log, and reviewed/signed publishing.

## 4. Stability and Windows-update resilience
Open-issue sample (first page of 47; partial) [S41] FACT:
- #1674 Win+D hides Dock and Desktop Widgets on Win11 24H2 (open since 1 Jul 2026 per competitor [S39]; compare Rainmeter's fix of the same symptom in 4.5.22 [S1]).
- #1734 Windows key stops opening Start after lock/restart/display wake.
- #1874 panic on every sleep; #1914 startup exit on slow cold start (hard-coded 3 s WebView check); #1798 UI hang; #1894 white screen in a game launch; taskbar icon/dock bugs (#1787, #1864, #1886); Wi-Fi module bugs (#1915, #1821).
- Competitor-cited: launch failure if Store and .exe builds both installed; conflicts with Start11/StartAllBack. [S39] CLAIM.
- Related platform risk (not Seelen-specific): Microsoft in 2023 told users to uninstall third-party UI customization apps (ExplorerPatcher, StartAllBack named) before an update that caused explorer crashes. [search results; Seelen not named] FACT for the advisory, INFERENCE for relevance.
- **Net:** shell-adjacent behavior (Win key, Win+D, taskbar) is where its open issues cluster, which supports my update-resilience argument (INFERENCE). It also shows the "Win+D" class of bug hits the desktop-layer technique that **my design also relies on** (WorkerW/desktop layer). My design isolates that to widgets only; it does not fix Win+D, so a prototype test is mandatory.

## 5. Memory/performance
- Issue #1081: RAM growth over time on v2.3.12 (opened 16 Jul 2025); maintainer first said it "scales to Tauri and WebView2," later (23 Nov 2025) said fixed in 2.4.7 and closed. No numbers in the report. [S30] FACT.
- Reviewers: ~78–110 MB (W); XDA says "a bit of a toll on performance" (CLAIM via S39). **My WebView2 budget (<150 MB/monitor) looks plausible but is still unmeasured.**
- Game/fullscreen pause: reported by a review (W), "Suspend widgets on Game Mode" UNVERIFIED.

## 6. UX and positioning weaknesses (CLAIM via S39; consistent with my gap list)
Features "haphazardly thrown in" (XDA), info requires Discord, docs aimed at authors not users, authoring needs a built web project (no no-code path). Supports Gaps 1 (onboarding) and 5 (no-code builder) as still open. INFERENCE: these are likely real but should be tested by installing it.

## 7. Verdict on my thesis
| Gap | Seelen UI status | Effect on my plan |
|---|---|---|
| G1 first-10-minute onboarding | Weak per reviews (CLAIM) | **Still open** |
| G2 safe third-party widgets | No documented permission model; runFullTrust (CLAIM) | **Open, strongest wedge** pending hands-on confirmation |
| G3 update resilience | Shell-adjacent; open 24H2 bug | **Open**, but I share Win+D risk |
| G4 hub | Largely closed (all-in-one) | **Closed** — drop as a differentiator |
| G5 no-code builder | None found | Open |
| G6 perf governor | Partial (fullscreen pause reported) | Mostly closed — table stakes |
| G7 creator revenue/store | Store + paid tier exist; creator payout UNVERIFIED | **Partly closed**; only the payout/quality-curation angle remains |
| G8 Win11 look | Strong theming | Closed |
| G9 accessibility | Unknown | Open, low-evidence |

**Revised wedge:** (1) provable safety (permissions + signing + review), (2) a no-code builder and first-10-minutes experience, (3) creator payouts with curation, (4) no shell replacement. Dropped as differentiators: "unified hub," "perf governor," "modern look," "Rust/Tauri."

## 8. Options (decision for you)
1. **Compete** with the narrowed wedge. Risk: Seelen can add permissions and a builder faster than a solo dev ships a store; it is AGPL, so I cannot borrow code into a closed product.
2. **Contribute/partner**: build the permission model, sandbox and review tooling inside Seelen UI (AGPL) or as a store layer. Lower upside, higher odds. Check the maintainer's openness first (UNVERIFIED).
3. **Pivot the wedge** to the no-code builder + creator tools as a standalone "widget studio" that exports to Seelen/Rainmeter-like targets (INFERENCE; untested demand).

## 9. Remaining checks before deciding (≈1 day)
Install Seelen UI on a VM; load a hostile test widget (file read, network, process spawn); measure RAM/CPU idle and in-game; read `capabilities` files in the repo; ask the maintainer about permissions/review roadmap; survey Seelen Discord/Reddit sentiment (read rules first).
