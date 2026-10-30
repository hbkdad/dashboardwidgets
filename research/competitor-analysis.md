# Competitor Analysis (accessed 2026-10-30)

Source IDs (S#) refer to `sources.md`. **FACT** = sourced. **INFERENCE** = my judgment. **UNVERIFIED** = not confirmed.

## 00. Gap-closing update (2026-10-30) — material changes to earlier sections
1. **Rainmeter 5.0 shipped Oct 12, 2026** [S28] (earlier text said "upcoming"). FACT. Adds: high-DPI, per-skin zoom 10–500%, dark-mode dialogs, multi-skin select/move, debug dialog with display/network tabs, **SVG and TextEdit meters**, JSON in WebParser, Windows media-session NowPlaying, LuaJIT, **hidden skins no longer drawn**, "lower CPU/GPU/memory depending on skin," opt-in `SyntaxVersion=2`, auto-detect of Skins folder changes. Requires 64-bit Win10 1903+/Win11. The changelog has **no security section** and no store, sandbox or permission model. So Rainmeter closes part of the DPI/perf/authoring gap but not safety, store, or onboarding (INFERENCE from absence in the changelog).
2. **New direct competitor: Seelen UI** [S29, S30] — FACT: free, AGPL-3.0, ~18.3k stars, **Rust + Tauri + WebView2**, widget SDK in Svelte/TypeScript, CSS/JSON theming with wallpaper-derived accent, desktop widgets, dock/toolbar, tiling WM, multi-monitor, 70+ languages, distributed via Microsoft Store, winget, GitHub (signed via SignPath). It is a **shell replacement**, i.e. exactly the approach my design rejects for update-resilience (INFERENCE: it will carry Windows-update risk). Reviews report ~78–110 MB RAM (W) after an early RAM-growth bug fixed in 2.4.7 (S). Dedicated store/marketplace: not found (UNVERIFIED). Sandbox/permissions for community widgets: not documented in what I read (UNVERIFIED). It reports pausing on fullscreen/game (W). **Implication: my stack is not novel; differentiation must come from safety, store/creator payouts, onboarding and update-resilience, not from "Rust+Tauri".** It also gives a free data point that WebView2-based desktop UI can fit ~100 MB (W, not my own measurement).
3. **Others seen (thin):** uWidgets (free, open-source macOS-style widgets; license/status UNVERIFIED), WinBar, Windhawk and YASB (named in 2026 guides, W [S36]). Winstep Nexus is actively maintained: v26.6, June 26, 2026 [S31] — so "dock tools declined" is not universal.
4. **Stardock price detail:** only dated figures: Object Desktop Business $79.99/yr (undated), consumer $49.99 first year/$29.99 renewal in an undated community post, Fences from $9.99 (Nov 2023) [S37]. Current consumer prices remain UNVERIFIED; Fences page confirms 1-year-subscription default [S27].
5. **Wallpaper Engine scale:** Workshop ≈2.87M entries (undated), owners est. 20–50M (SteamSpy range, estimate) [S33]. Treat as order-of-magnitude only.
6. **Why gadget-style tools faded — sourced precedent:** Microsoft disabled Windows Sidebar/Gadgets in 2012 after a security advisory (researchers planned a Black Hat talk on gadget vulnerabilities) and removed them in Windows 8 [S34]. FACT. This is the best sourced "why" in the category: an unsandboxed, run-arbitrary-code widget model got killed by the platform owner for security. It strengthens the sandbox requirement. The reasons for Samurize's and ObjectDock-2.x's decline remain unsourced.

## 0. Honest limits of this research
I could read Rainmeter's docs, the Wallpaper Engine Steam page and Lively's GitHub directly. Stardock's own site was unreachable (redirect to a dead archive), several forums returned 403, and I did not obtain verbatim Reddit/Steam review text. So the "praised / complained" lists below are built from forum threads and news snippets, and each item is tagged. Treat the complaint lists as **hypotheses to validate** with the interviews recommended in `market-and-gaps.md`.

---

## 1. Rainmeter
- **What it is:** Free, open-source skin engine; widgets ("skins") are text `.ini` files made of measures (data) and meters (visuals), with optional Lua and native plugins. [S1, S2]
- **Maintenance / cadence (FACT):** Releases are bursty: 4.5.16 Sep 2022 → 4.5.18 Jul 2023 → 4.5.19/.20 Aug 2024 → 4.5.21 Feb 2025 → 4.5.22/.23 Apr 2025 → 4.5.24–26 May 2026 → 4.5.27 Oct 6 2026. 4.5.27 announces **Rainmeter 5.0 will require Win10 1903+/Win11 and 64-bit only**; 4.5.27 is the last 32-bit/old-Windows build. [S1, S5] 5.0 feature list: UNVERIFIED.
- **Windows 11 (FACT):** 4.5.22 fixed skins disappearing on Win+D under Win11 24H2+. [S1] I.e. OS updates do break it, then get patched slowly.
- **Install friction (INFERENCE from S3):** Rainmeter ships only starter skins; the real content lives off-platform. Time to first good-looking desktop = find a skin pack on DeviantArt/Reddit, install .rmskin, often install fonts, edit variables in .ini, restart skins. No in-app catalog.
- **Discovery/updates (FACT):** Discovery is "community hubs": DeviantArt, r/Rainmeter, forum, Discord, Flickr. Rainmeter itself warns to be careful downloading anywhere but rainmeter.net. [S3] Skins have no standard update channel (INFERENCE; packager supports versions and "variables files" to preserve settings across upgrades [S2]).
- **Creators/monetization:** No built-in store (FACT: none documented in S2/S3). Creators publish free on DeviantArt/forum; monetization is external (donations/Patreon) — UNVERIFIED specifics.
- **Performance (FACT/INFERENCE):** Forum guidance is consistently "raise Update interval, avoid visualizers, isolate skins in Safe Start, toggle hardware acceleration." [S4] Rainmeter 4.5 added a game mode (German press, S snippet) — details UNVERIFIED. The engine is light; poorly-written skins are the cost center (INFERENCE).
- **Customization ceiling:** Non-coder can install and tweak colors/fonts via variables; making new skins requires learning the .ini syntax, measures, bangs, and often Lua (INFERENCE). Ceiling for coders is very high (Lua, plugins, WebParser).
- **Security model (FACT):** Packages must come from Skin Packager and are integrity-checked, but this proves *unmodified*, not *safe*. Packages can ship custom native plugin DLLs, and skins can use plugins like RunCommand to launch programs. [S2, S7] AV false positives on installer/.rmskin are common; 4.5.23 and 4.5.25 changed installer tooling and added signatures to reduce them. [S1, S8] 4.5.27 fixed Lua memory-corruption crashes. [S1] **No permission prompts, no sandbox** (INFERENCE from S2: nothing documented).
- **Community (W):** r/Rainmeter ≈322k members (undated). [S6]
- **Praised (INFERENCE, to validate):** free; huge creative ceiling; light on resources when well-built; long-lived community.
- **Complaints (FACT where cited):** (1) heavy skins/visualizers burn CPU [S4]; (2) AV false positives [S1, S8]; (3) Win11 updates break behavior [S1]; (4) no catalog/updater [S3]; (5) .ini learning curve (INFERENCE; no direct source).

## 2. Stardock suite
Products: Fences, Start11, WindowBlinds, DeskScapes, Groupy, CursorFX, Multiplicity, etc., sold singly or via Object Desktop. [S18, S19]
- **Pricing (mixed, dated — UNVERIFIED as current):** Object Desktop was advertised at $39.99 in a 2022 announcement [S19]; Fences 6 launch $9.99 (single PC) / $19.99 (multi-PC) [S14]; a Neowin store listing showed Fences $9.99, Start11 $5.99, DeskScapes 11 $3.99 [S20]. Stardock FAQ: "Lifetime/Permanent" is a one-time purchase for that version; subscriptions auto-renew; with the newer Object Desktop subscription, **software stops functioning if the subscription lapses**, whereas the older "Classic" keeps working at the lapsed version. [S15]
- **Windows 11 (FACT):** Fences 3 unsupported on Win11; Fences 4.x failures reported; Win11 24H2 broke Start11/Fences for some users (explorer restart loops, blank menu), and Stardock shipped compatibility updates after "months of testing." [S16, S17]
- **Install/discovery:** Single "Object Desktop Manager" installs the suite (S snippet from WindowBlinds page). Themes distributed through WinCustomize (INFERENCE from S19; marketplace health UNVERIFIED).
- **Customization ceiling:** WindowBlinds/DeskScapes are consumer-friendly; deep theming ceiling is lower than Rainmeter (INFERENCE).
- **Security:** Closed-source vendor-signed binaries; third-party skins for WindowBlinds etc. are asset packs (INFERENCE; not verified).
- **Complaints (FACT/INFERENCE):** (1) breakage after Windows feature updates [S16, S17]; (2) per-app licensing/versions/upgrade fees confusing [S14, S15]; (3) suite = many separate apps (INFERENCE from product list); (4) subscription-lapse-disables-software [S15]. Praise (INFERENCE): polished, one-click utilities (Fences/Start11) that solve a concrete need.
- **Pattern (INFERENCE):** Stardock hooks into undocumented Explorer/shell internals (Start menu, desktop icons), which is exactly why Windows updates break them.

**Update 2026-10-30 (from stardock.com, fetched directly [S27]):** Homepage lists Object Desktop, Start11, Fences, Groupy, DeskScapes, ObjectDock (still sold), WindowBlinds, Multiplicity, AltTabby, Connection Explorer, Snipboard (beta), Clairvoyance (AI). The Fences page shows three tiers (Single, Multi-Device, Object Desktop), **all labelled "1 Year Subscription"**, with a separate "Need a permanent License?" link; no dollar prices are shown on the page, and Fences states Windows 10/11/ARM. So the subscription-first default is **confirmed**; current dollar prices remain UNVERIFIED. Correction: ObjectDock is an active Stardock product, so "declined" status in §7 applies to its old 2.x era only.

## 3. Wallpaper Engine
- **Facts:** Steam, CDN$6.49 on the page I loaded; 97% positive of 234,819 English reviews (all-time), 96% of 7,412 last 30 days at fetch time; Win10/11; DirectX 11. [S9] Official claim: auto-pauses during games/maximized apps; Performance tab. [S11]
- **Trust incident (W, single outlet):** Workshop "Applications" section reportedly being closed after hackers used the Workshop to spread malware; recent reviews reportedly dipped (a teaser cites 68%, not confirmed). [S10] Another review noted the Workshop accepted `.exe` uploads (W). **Lesson (INFERENCE): an open "run code" marketplace without sandboxing eventually becomes a malware channel and forces a painful policy change.**
- **Creators:** Steam Workshop (free items, no formal sales channel — UNVERIFIED whether creator payouts exist). Strongest distribution engine in the category (INFERENCE from review count).
- **Ceiling:** Great for animated wallpapers; weak as a widget/layout system.

## 4. Lively Wallpaper
- **Facts:** Free, GPL-3.0, WinUI 3; ~20.0k GitHub stars, 1.4k forks, 401 open issues. Supports video/GIF, web (Chromium), Unity/Godot, Shadertoy, screensaver, multi-monitor, pause on fullscreen/battery, CLI and developer API. Store build lacks Unity/Godot/app wallpapers. [S12, S13]
- **Weakness (INFERENCE):** No creator payouts; single-maintainer dependency risk; community library is curated by GitHub/Reddit rather than a store.

## 5. PowerToys and native Windows widgets
- **PowerToys (FACT):** FancyZones, Workspaces, Command Palette (with a dock in 0.98), CursorWrap for multi-monitor, Light Switch, etc. [S25] It's utility-first; no visual desktop theming. It sets the bar for "official, free, safe," but is not competitive on aesthetics (INFERENCE).
- **Windows 11 Widgets board (FACT/UNVERIFIED):** Third-party news *feeds* in the EU via DMA [S24]; a 2026 report of widgets coming to the lock screen in the EU (single-outlet, UNVERIFIED). Widgets live in a flyout, not on the desktop (INFERENCE from product behavior). Microsoft's direction is a risk and a platform tailwind: a sanctioned widget API could commoditize basic widgets.

## 6. Others (thin evidence, UNVERIFIED status)
- **Winstep Nexus, XWidget, Cairo Shell, Conky, KDE Plasma:** I found no reliable current-status sources. Cairo-Dock (Linux) lets dock applets detach as desktop widgets; KDE Store still receives Plasma 6 widgets in 2025 [S26]. KDE's built-in widget store + one-click install is the **UX reference** worth copying (INFERENCE).

## 7. Declined tools
- **Samurize:** ended; project closure announced 2015 per Wikipedia-cited post. [S21] **ObjectDock:** old 2.x line, then a 3.0 listed March 2024. [S22] **No source gives reasons.** My inference of the traps: (a) single-author/config-file engines depend on the author staying active; (b) tools tied to OS shell internals die when the OS changes (Win8/10 shifted shell paradigms); (c) a paid-dock niche collapsed when Windows added its own taskbar/Start features; (d) no marketplace flywheel. Label: **INFERENCE**.

## 8. Cross-product comparison

| Dimension | Rainmeter | Stardock | Wallpaper Engine | Lively |
|---|---|---|---|---|
| Price | Free | Paid, mixed licensing | ~$ (CDN$6.49) | Free |
| In-app catalog | No | Partial | Yes (Steam Workshop) | Partial (library) |
| Non-coder authoring | Poor | n/a (apps) | Editor for wallpapers | Limited |
| Sandbox for third-party content | None documented | Closed vendor | Weak (incident) | Unknown |
| Windows-update breakage | Yes, patched later | Yes, notable | Low reports | Unknown |
| Creator revenue path | None built-in | Unknown | Unverified | None |
