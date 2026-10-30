# Seelen UI hands-on test (desk-only; NO install performed)

Date: 2026-10-30. Labels: **OBSERVED** (I read it myself in this session), **NOT TESTED**, **INCONCLUSIVE**. Nothing here is guessed.

## 1. Environment, versions, and what was NOT done
- Host: Windows 11 Home 10.0.26300 (no Windows Sandbox / Hyper-V).
- I asked the user which isolation to use (VM, spare PC, or this PC with a restore point, plus approval of a named download). The answer was "No preference", which is **not an approval**. Per the safety rules I **installed nothing, downloaded no installer, ran no widget, started no test server**. So questions 3, 4, 5, 6 are NOT TESTED and the hostile-widget part of Q1 is NOT TESTED.
- Source read: `eythaann/Seelen-UI` branch `master` (raw files via GitHub, read only, nothing copied; AGPL) and the published docs on seelen.io.
- Latest release seen: **v2.8.10, published 2026-10-22**. Assets: `Seelen.UI_2.8.10_x64-setup.exe` 48,665,816 B (also a "-fixed" x64 exe 257,629,904 B, and `Seelen.UI_2.8.10.0_x64.Msix` 43,800,895 B). Not downloaded.
- To run the hands-on parts later, the user must approve one of: x64 setup.exe 48.7 MB from the GitHub release, or the Store/winget build, plus an isolation choice.

## 2. Results table

| # | Question | Result | Evidence |
|---|---|---|---|
| 1a | Per-widget permission model exists in source? | **OBSERVED: YES, for a defined set of commands, in v2.8.10** | see section 3 |
| 1b | Hostile test widget (file read, spawn calc.exe, localhost fetch) | **NOT TESTED** | no install |
| 1c | Store build declares runFullTrust | **OBSERVED: CONFIRMED** in `src/templates/AppxManifest.xml` (master) | section 3 |
| 2 | Review / signing of community resources | **OBSERVED (docs): no review/signing/moderation is documented**; upload flow described | section 4 |
| 3 | Idle RAM/CPU after 5 min; fullscreen behavior | **NOT TESTED** | no install |
| 4 | Win+D (#1674), lock/unlock, sleep, Win key (#1734) | **NOT TESTED** | no install. Issue status not re-checked in this session either |
| 5 | First 10 minutes | **NOT TESTED** | no install |
| 6 | Uninstall cleanliness | **NOT TESTED** | no install |

## 3. Q1 detail: permissions (source reading)
- `libs/core/src/state/widget/permissions.rs` defines `WidgetPerm`: `Run`, `OpenFile`, `ModifyConfiguration`, `KillProcesses`, `ManageAppWindows`. Doc comment: "Commands that require explicit permission for third-party widgets."
- `src/background/widgets/permissions.rs`: `request()` auto-grants **bundled** widgets; for third-party widgets it checks the stored decision in `permissions.json` (app data dir), else shows a **blocking Yes/No warning dialog** naming the widget and the permission, then persists Allowed/Denied per widget id and permission. Decisions are dropped when the widget is uninstalled, so a reinstall asks again. Dialogs are serialized so spamming a command cannot open many.
- Both files exist at tag **v2.8.10** (checked via the contents API at that ref). Git history: "fix(security): require widget permissions for config and window commands" (commit message dated 2026-10-05) and "feat(settings): security tab" (2026-10-08), later edits 2026-10-16. So the model is **recent (about 3-4 weeks before v2.8.10)**. Earlier versions, including those any reviewer or competitor looked at before October 2026, may genuinely have lacked it.
- `src/capabilities/general.json` and `general_window.json`: apply to windows `"*"` ("general permissions for all widgets"): `core:path:default`, `core:app:default`, `dialog:default`, `core:event`, `core:webview` subset, and a long list of `core:window:*` allows (create denied). No Tauri `fs`, `shell`, or `http` plugin permission appears in these two files. These capabilities are **global, not per-widget**; per-widget gating is the custom `WidgetPerm` layer above.
- What I did NOT verify: which `invoke` commands call `request_widget_permission` for which `WidgetPerm` (I only saw the commit message listing config/process/window commands; the code search API needed auth, so I did not enumerate call sites); whether plain web APIs inside the widget webview (`fetch` to arbitrary hosts, reading user-dropped files) are restricted. `tauri.conf.json` has no `csp` key in its `app.security` block, which suggests no CSP is configured, but runtime behavior is **INCONCLUSIVE** without a test.
- Docs (`/docs/widget-guidelines`, `/docs/widget-js-api`) say widgets run in "an isolated webview" and call backend via typed `invoke`; **they do not document the permission prompts** (OBSERVED absence). So the model is real in code but undocumented on the pages I read.
- runFullTrust: `AppxManifest.xml` lists `rescap:Capability runFullTrust` and `unvirtualizedResources`, plus device capabilities (radios, bluetooth, wiFiControl). That is normal for a desktop app packaged as MSIX, and it describes the host app, not widgets. The competitor's runFullTrust statement is **true as stated**, but it does not by itself mean widgets are unrestricted.

## 4. Q2 detail: review, signing, upload
- `/docs/resource-guidelines` (OBSERVED): publish by `slu resource bundle <kind> <path>` into a single `.yaml`, upload through the website; permanent internal ID assigned; images (portrait 1:1, banner 21:9, screenshots 16:9) must be uploaded and hosted on seelen.io. **No mention of review, verification, signing, moderation or licensing.** Terms of Service contents not read.
- `SECURITY.md`: vulnerability reporting by email/Discord only; says nothing about widget vetting.
- Release signing (SignPath) comes from the earlier audit and was not re-verified here. There is no evidence in what I read of **signing of individual community resources**.
- Whether a human or automated review happens server-side: **INCONCLUSIVE** (not documented; I did not use the upload flow).

## 5. Raw numbers
None. No measurement was taken (Q3 to Q6).

## 6. Verdict
- **"Seelen UI has no documented per-widget permission model": PARTLY REFUTED / needs qualifying.** For current source (v2.8.10, Oct 2026), a per-widget, per-permission, user-prompted model **exists in code** (5 permissions, persisted decisions, a settings "security tab"). It is **not documented on the widget docs pages I read**, and it covers only certain backend commands; network and file reads from the web content itself are untested and **INCONCLUSIVE**. The claim was probably true before October 2026. Treat the competitor's statement as outdated, not as false in intent.
- **"Win+D hides desktop widgets on 24H2": NOT TESTED / INCONCLUSIVE.** Only the issue's existence (#1674) is known from prior desk research.
- Design implication (INFERENCE): "safe third-party widgets" is no longer an uncontested wedge; Seelen has a coarse prompt model. The remaining differentiators are finer scoping (network/file), documented and reviewed publishing, and signing; those need the hostile-widget test to confirm.

## 7. Next step to finish the job
Approve an isolation option and the installer (name/source/size above). Then run: hostile widget (file read in a scratch folder, `calc.exe`, localhost fetch), 5-min idle RAM/CPU (all `Seelen*` plus child `msedgewebview2` processes), Win+D, lock/unlock, sleep, uninstall diff.
