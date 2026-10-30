# Hub UX: art direction, flows, and handoff

Interactive prototype: `design/prototype/index.html` (single file, no dependencies; open it in a browser, add `?skip=1` to bypass first-run). Built with the `newage-webui` rules: tokens first, controlled depth instead of heavy 3D (the WebView2 budget in `architecture.md` is already tight), reduced-motion and keyboard support on every interaction.

## 1. Art direction
- **Feeling:** a calm, high-trust control room for your desktop. Premium and tactile, never neon. The desktop preview is the hero; the chrome recedes.
- **Surfaces:** graphite layers (`--bg` to `--surface-3`) with 1px hairline borders, soft inner highlight, large radii (14-30 px). Glass is used only on widgets in the preview, where blur sells the "this is your wallpaper" illusion.
- **Color:** one live accent that changes with the selected Scene (brand mint `#7cf2c8` by default). Semantic colors are fixed and meaningful everywhere: mint = allowed/safe, amber = asks/paused/needs attention, red = blocked/never, blue = information.
- **Type:** Windows-native stack (Segoe UI Variable) so the hub looks like part of Windows 11; Cascadia Mono for the activity log. Tight display tracking, generous body line height.
- **Depth & motion:** pointer parallax (two wallpaper layers, 8 and 16 px) plus a 3-degree stage tilt; hover lifts on widgets and cards; 600 ms ease-out view entrances. All of it stops under "Reduce motion" or the OS setting. No scroll hijacking, no 3D canvas.
- **Tokens:** defined in `:root` of the prototype (surfaces, text, signal/amber/danger/info, radii, 4-px spacing scale, motion durations, one easing curve). The Svelte build must import the same token names.

## 2. Information architecture
Left rail: **Scenes** (default), **Library**, **Safety**, **Remix**, **Settings**, plus a persistent **Safe mode** control at the bottom so panic is always one click away. The Library tab shows an amber badge when an update wants new access.

## 3. Flows (each is implemented in the prototype)
1. **First run (under 90 s, no tutorial).** Pick a vibe (3 Scene cards) -> see it on a preview fitted to your screen -> a three-line trust summary (Allowed / Asks / Never). Escape or "Skip setup" always works. Nothing touches the system until Apply.
2. **Apply and undo.** One button, short determinate progress, toast "Applied. Your previous desktop is saved" with Undo; "Revert to before Plinth" stays available.
3. **Permission Center.** Per widget: what it can touch, why (publisher's reason in plain language), a switch per permission, and "Built in" vs "Isolated" labels. A live activity log shows allowed and blocked requests; clicking a blocked row explains why in a sentence. A slots meter shows the memory cost of isolation (about 29 MB per third-party widget, cap 8).
4. **Update with permission diff.** An update never inherits new powers. The Library card shows "wants new access" with the exact additions; updating keeps old grants only.
5. **Safe mode.** One switch dims and pauses all third-party widgets, disables their toggles, and says so on the stage and in the log. Also auto-offered after two widget-host crashes in five minutes (see architecture).
6. **Remix.** Change title, size, accent, and data source with live preview; the permission chip updates ("none", "CPU", "Memory") so the user sees the cost of a change before saving. Saved variants can only use what the original could.
7. **Comfort settings.** Reduce motion (defaults to the OS preference), high contrast, interface size 90-130%, pause in games/fullscreen, pause on low battery.

## 4. Microcopy rules
Plain verbs ("Allow", "Revoke", "Undo"), no jargon in the primary layer (no "CSP", "origin", "sandbox"; those live in an Advanced details disclosure). Always say what happens if you decline. Never use fear language; state the guarantee ("No widget can ever request your files").

## 5. Accessibility (built in, to be verified with a screen reader and Accessibility Insights)
- Landmarks, skip link, focus-visible ring, roving focus not needed (all controls are buttons).
- Toggles use `role="switch"` with `aria-checked`; scene choices use `role="radiogroup"/radio`; toasts and the activity log are `aria-live`.
- Contrast: text on surfaces at least 4.5:1 after raising `--text-faint` to `#8a91ab`; high-contrast mode adds stronger borders and brighter secondary text. Not yet measured with a tool: UNVERIFIED.
- Never rely on color alone: every state also has a text label.
- Reduced motion honored from the OS and a manual switch; zoom/scale via the interface-size slider.

## 6. QA done on the prototype (2026-10-30, in the built-in browser)
Scripted pass: first-run steps and close, all five views, scene switching, apply/revert toast, Safe mode on/off, permission switches, activity log growth, update flow wiring; **no console errors**. Not done: real screen-reader test, measured contrast, mobile widths (the hub is a desktop app; narrow layout only degrades gracefully), animation performance profiling, and the WebView2 process cost of `backdrop-filter` (blur can be GPU heavy; test on low-end hardware and provide a "Simplify visuals" fallback if needed).

## 7. Handoff to the Svelte build
Component list: `Rail`, `SceneStage` (layers + widgets), `SceneCard`, `ApplyBar`, `Toast`, `WidgetRow` + `PermissionRow` + `Switch`, `ActivityLog`, `SlotsMeter`, `UpdateDiff`, `RemixPanel`, `FirstRunSheet`, `SettingsRow`. State to wire to `plinth-host`: installed widgets, grants (`Host::grant/revoke`), activity (`Host::activity`), safe mode, slot usage (`Host::installed`). The "blocked" explanations map from broker `DenyReason`.
