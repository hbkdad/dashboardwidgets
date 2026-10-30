# plinth-app (v0.5)

The first runnable Plinth app. Windows 11 with the WebView2 runtime (ships with Windows 11).

```powershell
cd plinth
cargo run --release -p plinth-app
```
Opens the **hub**. Nothing touches your desktop until you press **Apply to my desktop**; **Revert** removes the layer. Closing the hub **hides it**: the app keeps running in the background (tray icon: Open, Toggle Safe mode, Quit) and the desktop layer stays. Starting Plinth a second time brings the existing hub back instead of starting another copy. If a Scene was applied when you quit, it is restored the next time the app starts.

## What it does
- **Desktop layer**: a window attached behind the desktop icons paints the Scene wallpaper and hosts the widgets (technique from the spikes; survives Win+D on the tested build).
- **Widgets**: `plinth/widgets/{clock,sysmon,notes}`, each in its own isolated origin with a CSP generated from its manifest and your grants. System monitor shows live CPU and memory through the permission broker.
- **Safety**: per-widget permission switches, live activity log (allowed and blocked), Safe mode (pauses all widgets and closes the network).
- **Network**: all widget traffic is forced through a local deny-by-default proxy; the Network tab shows allowed destinations and every attempt.
- **State**: `%APPDATA%\Plinth\state.json` (scene, applied, safe mode, grants, widget storage). WebView2 data in `%LOCALAPPDATA%\Plinth`.

## Dev switches
`PLINTH_NO_ATTACH=1` normal window instead of the desktop layer; `PLINTH_STATE=<file>`; `PLINTH_WIDGETS=<dir>`; `PLINTH_AUTOTEST=<seconds>` runs an unattended end-to-end check (applies a full-screen layer, revokes CPU, toggles Safe mode, sends Win+D, samples a pixel) and exits 0 or 1; `scripts/verify.ps1 -App` wraps it.

## Limits (v0.5)
Primary monitor only; no autostart; the tray menu items were not click-tested (only that the icon is created); first-party widgets are isolated like third-party ones (about 29 MB each); Safe mode pauses first-party widgets too; no installer or signing; the hub window is click-tested through UI Automation (`scripts/clicktest.ps1`, 28 checks), not yet by a human or with screen-reader software; Moods, Setup Card import/export, Remix and the store are not in the app yet (see `feature_list.json`).

## Tests
`scripts/verify.ps1` (unit tests + build), `-App` (unattended end-to-end autotest), `-Click` (click-through of the real hub window via UI Automation).
