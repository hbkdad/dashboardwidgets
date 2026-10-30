# Product Requirements: Plinth (working name)

Gap numbers (G1–G10) refer to the ranked list in `research/market-and-gaps.md`.

## 1. Vision
Plinth is the desktop-widget platform you can trust: every widget declares what it can touch, you can see and revoke it, and nothing replaces your shell. Anyone can make or remix a widget without code, and creators get paid through a curated store.

### 1.1 Positioning (revised after the Seelen UI audit, see `research/seelen-ui-audit.md`)
Compete on four things only: (1) **provable widget safety**, (2) **no-code builder and a great first 10 minutes**, (3) **creator payouts with curation**, (4) **no shell replacement**. We do *not* claim: unified hub, performance governor, modern look, or Rust/Tauri as differentiators; those are table stakes (Seelen UI, Rainmeter 5.0, Lively).
- vs Seelen UI (free, AGPL, shell-adjacent): as of v2.8.10 (Oct 2026) it has a coarse per-widget prompt model (5 command permissions, per-widget persisted decisions) but, from source reading, no documented review/signing of community resources, no network-host or file scoping evidence, and no activity log. We differentiate on **finer scoping + visible activity log + reviewed and signed publishing**, plus builder-first and no shell replacement. Runtime behavior untested (hands-on test pending).
- vs Rainmeter 5.0 (free, powerful, text-config, no sandbox/store): we are visual and safe.
- vs Stardock/Wallpaper Engine: we are widget-first with creator payouts.
- Licensing note: Seelen UI is AGPL-3.0, so **no code is copied** from it into Plinth; Plinth is clean-room.
- These claims about Seelen UI are from desk research and a competitor's comparison; the Phase 0 hands-on test must confirm them before launch messaging uses them.

## 2. Name options (trademark status: **UNVERIFIED, no search done**; run USPTO/EUIPO/domain checks before committing)
1. **Plinth** — the base a sculpture sits on; implies "a stage for your desktop," short, easy to type.
2. **Tessel** — one tile of a mosaic; implies composable widgets.
3. **Dwell** — where you live; friendly. Likely crowded as a common word.

## 3. Users
- **Primary:** Casual-aesthetic Windows 10/11 users and streamers who want a standout desktop without learning config files (G1, G4).
- **Secondary:** Theme/widget creators who want reach and income (G5, G7); power users who want data widgets with permissions (G2).
- Not targeted in v1: enterprise, macOS/Linux.

## 4. First 10 minutes (no tutorial video)
1. **0:00** Installer (signed, per-user, no admin, <60 MB target). Launches into the hub.
2. **0:45** Welcome screen: "Choose a vibe" — 6 Scene cards with live previews rendered from the user's own wallpaper resolution.
3. **1:30** One click on **Apply**: wallpaper, 3–5 widgets, accent color, dock-less layout appear on all monitors, auto-fitted to resolution and DPI.
4. **2:30** "Make it yours" panel: drag widgets, change a color (live), toggle widget on/off. Undo always visible.
5. **5:00** Optional: connect calendar/weather (OAuth, only for widgets that ask, shown in a permission sheet).
6. **8:00** Prompt: "Pause widgets while gaming? (recommended)" default on. Done; tray icon explains how to restore the old desktop in one click ("Revert to before Plinth").

## 5. Features

### 5.1 MVP (solo-buildable; each ties to a top-3 gap)
| ID | Feature | User story | How it works | Gap |
|---|---|---|---|---|
| M1 | **Scenes** (curated bundles) | As a casual user I pick a vibe and my desktop is done | Scene = signed bundle (wallpaper + widget layout + theme tokens). Apply is atomic and revertible | G1 |
| M2 | **Widget runtime, sandboxed** | I install widgets without fearing malware | Each widget runs in a sandboxed WebView2 frame with a capability-gated API; no filesystem/network unless the manifest declares and user approves | G2 |
| M3 | **Basic pause governor** (table stakes, kept minimal) | My game FPS and laptop battery are unaffected | Rust service pauses widget ticks on fullscreen/game/battery/remote session; no tuning UI in MVP | G6 (parity) |
| M9 | **Permission Center** (headline feature) | I can see what every widget can access and revoke it | Per-widget screen listing declared permissions, last-used time, network hosts contacted (from core proxy log), one-click revoke/uninstall; install-time plain-language sheet; "Safe mode" disables all third-party widgets | G2 |
| M10 | **Remix editor (builder-lite)** | I can change any declarative widget without code | Edit text, fonts, colors, size, data source (dropdown) on declarative widgets; save as my own variant. Full drag-and-drop builder is v1.0 | G5 |
| M4 | **Window-layer integration without shell hooks** | Windows updates don't break it | Widgets are normal top-level windows pinned to desktop layer (WorkerW technique, with fallback to bottom-most window) rather than Explorer injection | G3 |
| M5 | **First-party widget set (10)** | Day-one content is good | Clock, calendar, weather, system monitor, media controls, notes, tasks, quick launcher, battery, world clocks | G1 |
| M6 | **Theme tokens** | Desktop matches Windows | Follow system light/dark and accent; Mica-like translucent cards via DWM backdrop APIs | G8 |
| M7 | **Revert / profiles** | I can undo everything | Snapshot of previous wallpaper/settings; JSON export/import of a profile (file-based, no cloud) | G10 (partial) |
| M8 | **Accessibility baseline** | Large text and reduced motion work | Global scale, high-contrast token set, reduced-motion switch honoring Windows setting | G9 |

MVP explicitly excludes the store and the full drag-and-drop builder (see 5.2; M10 remix editor is the builder-lite) to stay solo-buildable; MVP ships a **local gallery** where Scenes are installed from a file or a signed GitHub-hosted index.

### 5.2 v1.0
| ID | Feature | Notes | Gap |
|---|---|---|---|
| V1 | **Visual widget builder** (no-code) | Drag text/image/chart/gauge elements, bind to data sources (CPU, clock, weather, RSS, JSON URL) via dropdowns; exports a declarative widget | G5 |
| V2 | **Built-in curated marketplace** | In-app browse/search/install/update; review queue before listing | G1, G4, G7 |
| V3 | **Creator revenue share** | Stripe Connect payouts (paid license: external fee, see business doc) | G7 |
| V4 | **Signed packages + malware review** | Developer certs, publisher verification, static scan, permission diff on update | G2 |
| V5 | **Animated wallpaper support (video, web)** | Reuses the runtime; shares governor | G4 |
| V6 | **Multi-monitor/HDR/DPI polish** | Per-monitor Scenes; HDR tone-mapping for video; test matrix | G8 |
| V7 | **Auto-updates** for app + installed content | Delta updates, one-click rollback | G3, G4 |

### 5.3 Later / rejected
- **AI-assisted layout generation** (later): attractive but not tied to a top-3 gap; revisit after builder ships. Requires paid inference or a local model: flag as cost risk.
- **Cloud sync** (later): needs accounts and backend; file-based export covers G10 for now.
- **macOS/Linux** (Phase 2 consideration): different compositors and APIs; defer.
- **Replacing Start menu/taskbar/Explorer** (rejected): shell injection is exactly what breaks on Windows updates (G3).
- **Lua/native plugin execution** (rejected): breaks the security model. Power users use JS widgets with permissions.
- **Rainmeter skin import** (investigate later): could ease migration but .ini+plugins semantics are large; UNVERIFIED feasibility.
- **Subscription that disables the app on lapse** (rejected): known user pain [Stardock FAQ].

## 6. Non-goals for v1
No shell replacement (taskbar, Start menu, Alt+Tab, tiling; this is the deliberate contrast with Seelen UI), no desktop icon management (Fences' territory), no cross-platform, no AI features, no social network, no scripted-widget store listings until the Permission Center and review pipeline are proven.

## 6.1 Known shared risk
The desktop-layer technique can hit the same class of bug Seelen UI reports on Windows 11 24H2 (Win+D hiding desktop widgets, its issue #1674). Plinth does not avoid this by design; it limits blast radius (widgets only, no shell hooks) and must pass a Win+D/Show-Desktop/lock/sleep/display-change test matrix before MVP ships.

## 7. Requirements (non-functional)
- Idle CPU, RAM, startup targets: see `architecture.md` §6.
- Works on Windows 10 22H2 and Windows 11 22H2+ (10 EOL consideration: UNVERIFIED how large the Win10 base remains; MVP supports it only if no extra cost).
- Fully usable offline.
- Opt-in telemetry only; no account required for the free core.

## 8. Success metrics
See `business-and-roadmap.md` §5.

## 9. Solo-build flags
- **Not purely solo/free:** code-signing certificate (paid, annual), store payments (Stripe fees, KYC), moderation workload for marketplace, hosting costs. MVP avoids all except a signing certificate (a SmartScreen-reputation problem even with one).
- Visual builder is the largest single engineering item (est. months; INFERENCE).
