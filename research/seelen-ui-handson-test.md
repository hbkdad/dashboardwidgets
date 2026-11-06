# Seelen UI hands-on test (v2.8.10, installed, tested, uninstalled)

Date: 2026-11-06. Labels: **OBSERVED** (I saw it happen or read it myself), **NOT TESTED**, **INCONCLUSIVE**. Nothing is guessed. Seelen source was read for observation only (AGPL); no Seelen code was copied. The test widget is my own.

## 1. Environment and versions
- Host: the user's own PC, Windows 11 Home **build 10.0.26300** (newer than 24H2's 26100, so it is not exactly the "24H2" in issue #1674), two 1920x1080 monitors side by side, 28 logical cores. The user made a restore point and approved the download. I could not verify the restore point myself (the query needs admin and returned "Access denied"); I relied on the user's confirmation.
- Seelen UI **v2.8.10** (GitHub release 2026-10-22). Installer: `Seelen.UI_2.8.10_x64-setup.exe`, 48,665,816 bytes (matches the release listing), SHA256 `227F8740C106A27056F06C488EC69816593503629B39051E63227A9B196B0511`, Authenticode **Valid**, signer **SignPath Foundation**. Installed silently with `/S` (so the interactive installer screens were not seen), launched `seelen-ui.exe` by hand.
- Tested widget: my own "Hostile Probe" (3 versions, ids `@plinth-test/hostile-probe`, `-probe2`, `-probe3`), loaded with `slu resource load widget <path>`. No community-store widget was run. Local test server was my own on `127.0.0.1:8765`; no real third-party hosts, no real user data. Canary file lived in a scratch folder.
- Caveat on machine: the PC was in normal use (Battle.net, Claude app, 6 other WebView2 processes before install). Numbers are not from a clean machine.

## 2. Results table

| # | Question | Result |
|---|---|---|
| 1a | Per-widget permission model in source | **OBSERVED: yes** (5 permissions, user prompt, persisted per widget) |
| 1b | Hostile widget: spawn calc.exe | **OBSERVED: a permission dialog appeared** ("...requesting permission to run programs. Do you want to allow this?", Yes/No). After the dialog, `run` was allowed and calc.exe started. Both dialogs (run, open file) ended as "allowed"; I believe the user clicked Yes (I did not click and cannot prove who did). **The Deny path is NOT TESTED.** |
| 1c | Hostile widget: open a file (`open_file` on canary.txt) | **OBSERVED:** also prompted, persisted "allowed", then succeeded (opened the file) |
| 1d | Hostile widget: read a file outside its folder | **OBSERVED: blocked.** `fetch(file:///...)` failed; `read_data_file` with absolute path -> "Absolute paths are not allowed"; `..` traversal -> "Path traversal attempt detected" (one depth) or path-not-found (other depths). `write_data_file` with an absolute path blocked. Own-folder read/write worked with **no prompt** (stored under `%APPDATA%\com.seelen.seelen-ui\data\<widget-id>\`). Canary file was never read; no file written into the scratch folder. |
| 1e | Hostile widget: fetch from local server | **OBSERVED: succeeded, no prompt.** `fetch('http://127.0.0.1:8765/ping')` returned 200; my server logged requests with `Origin: http://tauri.localhost`. No CSP or network restriction was seen. Reaching the internet was not tested (rule: no real third-party hosts), but a localhost fetch working implies HTTP egress is not blocked by a permission. |
| 1f | runFullTrust in Store build | **OBSERVED in source: confirmed** (`AppxManifest.xml` template). The Store build was not installed (I used the .exe build). |
| 2 | Review / signing of community resources | **OBSERVED (docs): none documented**; **INCONCLUSIVE** whether server-side review exists (upload flow not used) |
| 3 | Idle RAM/CPU, fullscreen | **OBSERVED**, numbers in section 5 |
| 4a | Win+D (#1674) | **OBSERVED on build 26300:** see section 6. Dock and toolbar were **not** hidden. Desktop-widget part **INCONCLUSIVE** (my widget never became visible). |
| 4b | Lock/unlock, sleep/resume, Win key after wake (#1734) | **NOT TESTED** (would need your password/session interruption; I did not lock or sleep your PC). Issue #1734 still **open** as of today. |
| 5 | First 10 minutes | **PARTLY OBSERVED**, section 7 |
| 6 | Uninstall cleanliness | **OBSERVED**, section 8 |

## 3. Permissions: source reading (earlier in this session, still valid)
- `WidgetPerm` = `Run`, `OpenFile`, `ModifyConfiguration`, `KillProcesses`, `ManageAppWindows`. Bundled widgets are auto-granted; third-party ones get a blocking Yes/No warning dialog, decision persisted in `%APPDATA%\com.seelen.seelen-ui\permissions.json`. Observed file after my test: `{"@plinth-test/hostile-probe": {"run": "allowed", "open_file": "allowed"}}` (and the same for `-probe2`). Entries are dropped when the widget is uninstalled.
- The model is new: code dated 2026-10-05 to 2026-10-16, i.e. 3 to 4 weeks before the release tested.
- The Tauri capability files (`general.json`, `general_window.json`) apply to all windows (`"*"`), not per widget.
- Observed gap for a product design: the permission prompt is **coarse and command-based** (it asks "run programs", not "run calc.exe"; and the first decision applies to any program). Own-folder file I/O and HTTP to localhost were unprompted. The permission dialog text did not name the program or arguments.

## 4. Review and signing (docs)
`/docs/resource-guidelines`: bundle to a single YAML with `slu resource bundle`, upload via the website, images hosted on seelen.io, permanent ID assigned. Nothing about review, signing or moderation. `SECURITY.md` covers vulnerability reports only. The installer itself is signed (SignPath Foundation), verified above. Signing of individual community resources: not documented.

## 5. Performance (raw numbers)
How measured: PowerShell, every 30 s for 300 s, starting about 20 s after launching `seelen-ui.exe`, default setup, two monitors, **my test widget not loaded**. Processes counted = `seelen-ui.exe` + `slu-service.exe` + every `msedgewebview2.exe` whose command line contains "seelen" and its child processes. Memory = Working Set (WS) summed (this over-counts pages shared between WebView2 processes); Private bytes also summed. CPU = delta of process CPU time over 30 s divided by 28 logical cores (percent of the whole CPU).

| t | procs | WS total | Private | CPU % |
|---|---|---|---|---|
| 0 s | 15 | 1,257 MB | 936 MB | - |
| 60 s | 17 | 1,390 MB | 915 MB | 0.28 |
| 90 s | 18 | 1,544 MB (peak) | 1,009 MB | 0.65 |
| 150 to 270 s | 15 | 1,286 to 1,297 MB | 808 to 819 MB | 0.01 to 0.04 |
| 300 s | 15 | 1,336 MB (Seelen 254 + WebView2 1,082) | 863 MB | 0.31 |

- Idle RAM after 5 minutes: **about 1.3 GB working set, about 0.85 GB private**, 15 processes (2 Seelen + 13 WebView2). This is far above the roughly 78 to 110 MB figure quoted by reviewers in the earlier audit, probably because those were single-monitor, and the number here covers two monitors' toolbar and dock windows plus other bundled widgets. **The reason was not investigated.** Idle CPU: **under 0.1 % of the CPU** after the first 2 minutes.
- Fullscreen: opened a kiosk-mode Edge window (isolated profile, not a game) fullscreen on the right monitor for 30 s: Seelen total WS 1,475 MB, 16 processes, CPU 0.05 %. The screenshot shows the right monitor entirely covered by the page (Seelen's bar and dock not visible there) while the left monitor's bar and dock stayed visible. I did **not** verify whether Seelen hides its windows or the fullscreen window simply covers them, and did **not** observe any widget-suspend behaviour. A real game was not tested: **NOT TESTED**.

## 6. Windows behaviour (Win+D)
Synthetic Win+D (key events from a script) on build 26300 with Seelen's "Fancy Toolbar" and "Dock/Taskbar" windows on both monitors: all app windows were hidden (screenshot), while every Seelen toolbar and dock window stayed `visible=True` before, after the first press and after the second press (which restored the apps). So **issue #1674's "hides Dock" was not reproduced here.** The "Desktop Widgets" half could not be tested: my `preset: Desktop` widget window reported `visible=False` the whole time (it never showed, probably needs enabling in the settings app, which I did not do). Not tested with a real keyboard press, and issue #1674 is still open.

## 7. First 10 minutes (partial)
- Install (silent): 8 s. Launch: toolbar, dock and a new green wallpaper appeared with **zero clicks** within the 20 s I waited. No onboarding wizard was seen because of the silent install and CLI launch, so the real first-run flow, click count and confusing points are **NOT TESTED**. Not covered: the settings app, resource store, or time to a "good-looking" customised desktop.
- Observed oddities: the first launch registered a service task `\Seelen\Seelen UI Service`; the installer did not start the app after a silent install; the default wallpaper replaced yours (it came back on uninstall); the setup `-fixed` exe is 258 MB vs 48.7 MB (not examined).

## 8. Uninstall (`uninstall.exe /S`, 5.3 s)
- Desktop returned to normal: Windows taskbar visible on both monitors, original wallpaper restored, `explorer.exe` running, no Seelen or WebView2-Seelen processes left, calc closed (I closed it).
- Removed: install folder contents, scheduled task, uninstall entry, PATH entry.
- **Left behind:** `C:\Program Files\Seelen` (empty folder), `%LOCALAPPDATA%\com.seelen.seelen-ui` (25.7 MB, logs and data), `%APPDATA%\com.seelen.seelen-ui` (my test widget's data file and `permissions.json` with my test widget's decisions), registry keys `HKCU\Software\Seelen UI` and `HKLM\Software\Seelen`. No startup (Run) entry was added by the app at all. I did **not** delete leftovers (not asked); say the word and I will remove them.

## 9. Verdicts
- **"Seelen UI has no documented per-widget permission model": REFUTED for v2.8.10 as a statement about the product.** A per-widget, prompted, persisted model exists and I watched the prompt for `run` and `open_file` appear. It is **undocumented on the widget pages I read** (so "no documented" is partly right), new (October 2026), coarse (action-level, not program- or host-level), and does not cover localhost/HTTP egress or own-folder file I/O. File sandboxing for data files held against absolute-path and `..` attempts.
- **"Win+D hides desktop widgets on 24H2": NOT CONFIRMED and NOT REFUTED.** On build 26300 + v2.8.10 the dock and toolbar were not hidden by Win+D. Desktop widgets were not visible in my test, so that half is INCONCLUSIVE. The issue remains open.
- **runFullTrust:** confirmed in source; irrelevant to widget isolation by itself.

## 10. Implications for the Plinth design (INFERENCE)
1. "Safe third-party widgets" is no longer unclaimed. Differentiate on finer scoping (per program, per host, per folder), plain-language prompts that name the exact target, no auto-grant for localhost/network, documented permissions, and reviewed/signed publishing.
2. RAM was about 1.3 GB WS (0.85 GB private) on two monitors; a budget under 150 MB per monitor would be a real advantage if it holds in Plinth, but re-measure with the same method.
3. Test Win+D against Plinth's own desktop-layer widgets directly; Seelen's result here does not settle that risk.

## 11. Not done / next
Lock/unlock, sleep/resume, Win key after wake; Deny path of the prompt; real game fullscreen; Microsoft Store build; first-run GUI flow; desktop-widget visibility under Win+D (enable a Desktop widget in settings first). Scratch files (installer, widget, logs, screenshots) are in the session scratchpad.
