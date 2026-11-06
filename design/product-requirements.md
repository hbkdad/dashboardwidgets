# Product Requirements: Plinth (working name)

Gap numbers (G1–G10) refer to the ranked list in `research/market-and-gaps.md`.

## 1. Vision
Plinth is the desktop-widget platform you can trust: every widget declares what it can touch, you can see and revoke it, and nothing replaces your shell. Anyone can make or remix a widget without code, and creators get paid through a curated store.

### 1.0 Revision after consumer research (2026-10-30, `research/consumer-demand.md`)
The first positioning was defensive (safety, builder, payouts). Consumer evidence says people also want: widgets **on the desktop**, a **useful** dashboard without a feed, to **copy other people's setups**, **low cost** on their PC (battery, games), and automation that **doesn't fight them**. New pitch: **"Make it yours, share it in one click, keep it cheap on your PC, and know it is safe."** Four additions, each with code or a prototype: **Setup Cards** (shareable whole-desktop files, no code, remix credit), **Impact Meter** (honest RAM/CPU/"pauses in games" on every item), **Moods** (context rules that suggest, never force), **Edit layout** on the live desktop (drag, snap, keyboard). Safety remains the trust base. See `plinth-core::{setup,context,impact}`.

New MVP rows: **M11 Setup Cards** (create, validate, permission summary, 60-second try-out with undo, remix attribution; Gap: copy culture), **M12 Impact Meter** (estimates now, live counters after host telemetry), **M13 Moods** (rules engine + suggestions; app/fullscreen/battery/time conditions), **M14 Edit layout** (grid snap, resize later). v1.0 adds a store with a "Cards" tab (browse, try, remix), creator credit chains, and measured Impact badges as a listing requirement.

### 1.1 Positioning (revised after the Seelen UI audit, see `research/seelen-ui-audit.md`)
Compete on four things only: (1) **provable widget safety**, (2) **no-code builder and a great first 10 minutes**, (3) **creator payouts with curation**, (4) **no shell replacement**. We do *not* claim: unified hub, performance governor, modern look, or Rust/Tauri as differentiators; those are table stakes (Seelen UI, Rainmeter 5.0, Lively).
- vs Seelen UI (free, AGPL, shell-adjacent): as of v2.8.10 (Oct 2026) it has a coarse per-widget prompt model (5 command permissions, per-widget persisted decisions) but, from source reading, no documented review/signing of community resources, no network-host or file scoping evidence, and no activity log. We differentiate on **finer scoping + visible activity log + reviewed and signed publishing**, plus builder-first and no shell replacement. Hands-on test (2026-11-06, v2.8.10, `research/seelen-ui-handson-test.md`, OBSERVED): its run/open-file prompts appear and persist; file I/O outside a widget's own folder was blocked; own-folder I/O and localhost HTTP needed no prompt; the prompt does not name the program. Idle RAM was about 1.3 GB working set on two monitors (not the 78 to 110 MB quoted by reviewers).
- vs Rainmeter 5.0 (free, powerful, text-config, no sandbox/store): we are visual and safe.
- vs Stardock/Wallpaper Engine: we are widget-first with creator payouts.
- Licensing note: Seelen UI is AGPL-3.0, so **no code is copied** from it into Plinth; Plinth is clean-room.
- These claims about Seelen UI are from desk research and a competitor's comparison; the 2026-11-06 hands-on test confirmed the permission claims above but did not test lock/sleep, the Win key after wake, a real game, or desktop widgets under Win+D (NOT TESTED / INCONCLUSIVE); keep launch messaging inside what is OBSERVED.

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
| V5 | **Animated wallpaper support (video, web)** | **Moved to MVP and shipped in v0.8.0 (D29).** Reuses the layer; shares the governor; measured cost in the Impact Meter | G4 |
| V6 | **HDR / mixed-DPI polish** | HDR tone-mapping for video; extended test matrix. (The monitor selector itself is MVP, see 5.4.) | G8 |
| V7 | **Auto-updates** for app + installed content | Delta updates, one-click rollback | G3, G4 |

### 5.3 Later / rejected
- **AI-assisted layout generation** (later): attractive but not tied to a top-3 gap; revisit after builder ships. Requires paid inference or a local model: flag as cost risk.
- **Cloud sync** (later): needs accounts and backend; file-based export covers G10 for now.
- **macOS/Linux** (Phase 2 consideration): different compositors and APIs; defer.
- **Replacing Start menu/taskbar/Explorer** (rejected): shell injection is exactly what breaks on Windows updates (G3).
- **Lua/native plugin execution** (rejected): breaks the security model. Power users use JS widgets with permissions.
- **Rainmeter skin import** (investigate later): could ease migration but .ini+plugins semantics are large; UNVERIFIED feasibility.
- **Subscription that disables the app on lapse** (rejected): known user pain [Stardock FAQ].

### 5.4 Wallpapers and monitors (addendum 2026-10-30; research in `research/monitors-and-wallpapers.md`)
Priority and phase, with the reason. Decisions D24 to D29.
| ID | Feature | Phase | Why |
|---|---|---|---|
| W1 | Wallpaper picker: grid, large preview, search, favorites, recent, bundled starter library, one-click apply with preview, per-wallpaper fit (Fill, Fit, Stretch, Tile, Span, Center), letterbox color, slideshow with interval and random order | **MVP** | First-run impression and the first thing a PC builder does; native fit modes exist [S44] |
| W2 | Folder scanning: add several folders, recursive, exclude subfolders, watch for changes, thumbnail cache, metadata, dedupe, graceful missing/corrupt/offline handling, scanned images are first-class library items | **MVP** | Users already own collections; without it the library is empty |
| W3 | Static image formats: JPG, PNG, WebP, BMP | **MVP** | Requested; WebP may need a converted cache copy (NOT TESTED) |
| W4 | Animated wallpapers (video, web) | **MVP (shipped v0.8.0)** | See D29; cost stated honestly in the Impact Meter. GIF is not supported yet (convert to video) |
| W5 | Profiles save wallpaper(s) and widget layout together | **MVP** | Scenes already bundle both; extended to per-monitor entries |
| W6 | Legibility assist: none / dim / blur / auto behind widgets | **MVP (dim and blur), v1.0 (auto from measured luminance)** | A busy wallpaper must not make widgets unreadable |
| M11 | Monitor detection, layout diagram matching the physical arrangement, per-monitor facts (name, resolution, scale, orientation, refresh rate, primary), Identify monitors | **MVP** | Prerequisite for everything below |
| M12 | Widget-to-monitor assignment: settings dropdown, drag in the diagram, click widget then monitor, right-click Move to monitor, keep size/settings/relative position | **MVP** | Mandatory per owner; competitors fail at persistence |
| M13 | Per-monitor layouts: different widgets per monitor, same widget type on several monitors with different settings; per-monitor wallpapers | **MVP** | Mandatory per owner |
| M14 | Persistence and edge cases (table below), Reset widget positions, Bring all widgets to primary monitor | **MVP** | This is where the documented Rainmeter failure lives [S52] |
| M15 | A widget that spans monitors | **Later** | Mixed DPI makes one window unreliable; rarely needed; the Span wallpaper covers the common case |

**User stories and acceptance criteria**
- W1: As a new user I pick a wallpaper from the bundled library in under 30 seconds. Accept: first run shows at least 8 bundled wallpapers; preview before commit; Apply puts it on the chosen monitor; Revert restores the previous wallpaper, fit mode and color per monitor.
- W2: As a user with a wallpaper folder I point Plinth at it and see the images. Accept: a 1,000-image folder is browsable while still scanning; files added, renamed, moved or deleted appear within 5 seconds for local folders and within 5 minutes for network/USB folders; a missing file shows a "file missing" card with Locate and Remove; an unplugged drive marks its items offline instead of deleting them; a corrupt or oversized file is skipped with a visible reason; identical files appear once.
- M11/M12: As a user with three monitors I put the clock on the left screen and the vitals on the middle one. Accept: the diagram matches Windows' arrangement (checked against Display settings); Identify flashes a number on each screen; assignment is stored per widget.
- M14: As a laptop user I undock and redock without losing my layout. Accept: every row of the edge-case table below is a test.

**Edge-case behavior (the contract)**
| Situation | Behavior |
|---|---|
| Assigned monitor is unplugged or off | Each widget keeps its `home` monitor. It is shown on the fallback monitor (user choice, default primary), clamped fully on screen, with a small "parked" badge in the hub. It returns to `home` automatically 2 seconds after the monitor's identity reappears. Parked placement is never saved as the home placement |
| Monitor appears late after wake or dock (the Rainmeter failure [S52]) | Display events are debounced (1.5 s); persistence only ever writes `home` data; a late-appearing monitor therefore just causes the widgets to move back |
| Resolution, scaling or orientation changes | Positions are anchor plus offsets in device-independent units; the layout is recomputed from the saved data; widgets stay inside the work area; a portrait/landscape change keeps relative positions |
| Widget larger than the monitor | Scaled down to fit (not below 50%); if it still does not fit, it is shown clipped with a notice and a "shrink to fit" action |
| Primary monitor changes | Assignments follow monitor identity, not "primary". Widgets assigned by role (imported setups) follow the new primary |
| Two identical monitor models | Told apart by device path and connector instance; if the user swaps ports they swap (documented limitation, NOT TESTED); Identify helps recover |
| Mirrored (duplicate) displays | Treated as one monitor |
| Remote desktop session | Different monitor set: widgets park and return on local reconnect |
| Virtual desktops | The layer sits in the desktop window, so widgets are **global across virtual desktops** (INFERENCE, NOT TESTED). Per-virtual-desktop layouts are Later. OS per-monitor wallpaper with several virtual desktops: NOT TESTED |
| Layout is messed up | "Bring all widgets to primary monitor" parks everything without changing homes; "Reset widget positions" restores the Scene's default layout (with undo) |

**Build status at v0.10.0 (what exists and what was verified):**
- Built and verified on this PC: monitor detection and diagram (real two-monitor layout), Identify, assignment by dropdown, by drag and by click-then-click, per-monitor layer windows, parked widgets and exact return (fake-monitor lifecycle test, 13 of 13), Bring all to primary, Reset with Undo, per-monitor motion wallpapers, picture library with folders, search, favorites, recent, problems, thumbnails, duplicate hiding, slideshow, per-monitor picture with fit mode and bars color, Revert (library end-to-end test, 10 of 10), shading behind widgets, profiles.
- Changed from the plan: new pictures and renames appear within about 5 s while the hub is open (60 s otherwise), by polling, not notifications; right-click "Move to monitor" on the widget itself is NOT built (the Monitors view and the widget list do it); the same widget type on several monitors works since v0.10.1 (copies share permissions, own storage and place); GIF and WebP-through-the-OS are not used (pictures are decoded by Plinth and composed, so WebP works).
- NOT TESTED: unplug during a real dock sequence, two identical monitor models, mixed DPI, several virtual desktops.

**Non-goals (this addendum):** spanning widgets, per-virtual-desktop layouts, GIF files, HDR wallpapers, wallpaper marketplace/download in MVP, cloud sync of the library, AI upscaling or generation.

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
