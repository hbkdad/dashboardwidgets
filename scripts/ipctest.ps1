# IPC identity test on the REAL app: a third-party widget frame that talks to window.ipc directly (and tries to act as the
# Notes widget) must be refused, while a legitimate first-party widget (PC Vitals) keeps working, and the app's own web views
# must not have been blocked from loading. Isolated state; shows the desktop layer for about 25 s. Usage: powershell -File scripts/ipctest.ps1
$ErrorActionPreference = "Stop"
. "$PSScriptRoot\uia.ps1"
$root = Resolve-Path "$PSScriptRoot\.."
$exe = "$root\plinth\target\release\plinth-app.exe"
$tmp = Join-Path $env:TEMP "plinth-ipctest"; Remove-Item $tmp -Recurse -Force -ErrorAction SilentlyContinue
$widgets = Join-Path $tmp "widgets"; New-Item -ItemType Directory -Force $widgets, "$tmp\lib" | Out-Null
Copy-Item "$root\plinth\fixtures\ipc-probe\evil-impersonator" "$widgets\evil-impersonator" -Recurse
Copy-Item "$root\plinth\widgets\pcvitals" "$widgets\pcvitals" -Recurse
Copy-Item "$root\plinth\widgets\notes" "$widgets\notes" -Recurse
[System.IO.File]::WriteAllText("$tmp\lib\library.json", '{"version":1,"roots":[],"items":[]}', (New-Object System.Text.UTF8Encoding($false)))
$state = "$tmp\state.json"; $log = "$tmp\app.txt"
$env:PLINTH_STATE = $state; $env:PLINTH_WEBVIEW = "$tmp\wv"; $env:PLINTH_WIDGETS = $widgets
$env:PLINTH_USER_WALLPAPERS = "$tmp\walls"; $env:PLINTH_LIBRARY = "$tmp\lib\library.json"
Get-Process plinth-app -ErrorAction SilentlyContinue | Stop-Process -Force; Start-Sleep 2
$p = Start-Process $exe -RedirectStandardOutput $log -PassThru -WindowStyle Hidden
$results = @()
function Check($name, [bool]$ok) { $script:results += [pscustomobject]@{ Name = $name; Ok = $ok }; "{0} {1}" -f $(if ($ok) { "[PASS]" } else { "[FAIL]" }), $name }
function Wait-Until([scriptblock]$cond, [int]$secs = 10) { $end = (Get-Date).AddSeconds($secs); while ((Get-Date) -lt $end) { if (& $cond) { return $true }; Start-Sleep -Milliseconds 300 }; return $false }
try {
  $w = $null; Wait-Until { $script:w = Get-PlinthWindow; [bool]$script:w } 15 | Out-Null
  Check "hub window found (the hub page was allowed to load)" ([bool]$w)
  Start-Sleep 2
  try { Invoke-ByName $w "Got it" | Out-Null } catch {}
  Invoke-ByName $w "Apply to my desktop" | Out-Null
  Check "scene applied" (Wait-Until { (Test-Path $state) -and ((Get-Content $state -Raw | ConvertFrom-Json).applied -eq $true) } 10)
  Check "the evil widget is installed as a third-party widget" (Wait-Until { Select-String -Path $log -Pattern "installed test.evil-impersonator in isolated slot" -Quiet } 5)
  $evilSeen = Wait-Until { @(Select-String -Path $log -Pattern "layer ipc refused").Count -ge 1 } 12
  "info: refused direct IPC messages from widget frames = $(@(Select-String -Path $log -Pattern 'layer ipc refused').Count) (0 means WebView2 never delivered them: sub-frame messages do not reach the handler; the secret is a second layer)"
  Invoke-ByName $w "Close" | Out-Null
  Start-Sleep 3
  # legitimate path: second launch reopens the hub; Safety lists PC Vitals having used a permission through the layer page
  Start-Process $exe -WindowStyle Hidden | Out-Null
  $w2 = $null; Wait-Until { $script:w2 = Get-PlinthWindow; [bool]$script:w2 } 15 | Out-Null
  Start-Sleep 3; try { Invoke-ByName $w2 "Got it" | Out-Null } catch {}; Start-Sleep 1
  Invoke-ByName $w2 "Safety" | Out-Null; Start-Sleep 3
  $txt = @(Show-Elements $w2 @("Text","Button"))
  Check "positive control: PC Vitals used permissions through the layer IPC (activity shows it)" (@($txt | Where-Object { $_ -match "use " }).Count -ge 1)
  Check "the forged storage write did not reach the Notes widget " (-not (Select-String -Path $state -Pattern "PWNED" -Quiet))
  Check "no navigation of the app's own web views was refused during normal use" (-not (Select-String -Path $log -Pattern "navigation refused" -Quiet))
} finally {
  Get-Process plinth-app -ErrorAction SilentlyContinue | Stop-Process -Force
}
$bad = @($results | Where-Object { -not $_.Ok }).Count
"{0} of {1} ipc checks passed" -f ($results.Count - $bad), $results.Count
if ($bad -gt 0) { "--- app log"; Get-Content $log | Select-Object -First 40; exit 1 }
