# Starts Plinth the way Windows does at sign-in (--background) and checks: no window appears, the tray icon exists, and a second launch brings the hub up.
# Usage: powershell -File scripts/backgroundtest.ps1
$ErrorActionPreference = "Stop"
. "$PSScriptRoot\uia.ps1"
$root = Resolve-Path "$PSScriptRoot\.."
$exe = "$root\plinth\target\release\plinth-app.exe"
$state = Join-Path $env:TEMP "plinth-bg-state.json"; $log = Join-Path $env:TEMP "plinth-bg-app.txt"; Remove-Item $state, $log -ErrorAction SilentlyContinue
$env:PLINTH_STATE = $state; $env:PLINTH_WEBVIEW = Join-Path $env:TEMP "plinth-bg-wv"; $env:PLINTH_LIBRARY = Join-Path $env:TEMP "plinth-bg-lib\library.json"; $env:PLINTH_USER_WALLPAPERS = Join-Path $env:TEMP "plinth-bg-walls"
Remove-Item Env:PLINTH_AUTOTEST -ErrorAction SilentlyContinue
Get-Process plinth-app -ErrorAction SilentlyContinue | Stop-Process -Force
$results = @()
function Check($n, [bool]$ok) { $script:results += $ok; "{0} {1}" -f $(if ($ok) { "[PASS]" } else { "[FAIL]" }), $n }
$p = Start-Process -FilePath $exe -ArgumentList "--background" -RedirectStandardOutput $log -PassThru -WindowStyle Hidden
try {
  Start-Sleep -Seconds 6
  Check "started with --background: no hub window is shown" (-not (Get-PlinthWindow))
  Check "it is running with its tray icon" ((-not $p.HasExited) -and (Select-String -Path $log -Pattern "tray icon: created" -Quiet))
  $p2 = Start-Process -FilePath $exe -PassThru -WindowStyle Hidden -RedirectStandardOutput (Join-Path $env:TEMP "plinth-bg-app2.txt")
  $end = (Get-Date).AddSeconds(10); $w = $null; while ((Get-Date) -lt $end -and -not $w) { Start-Sleep -Milliseconds 300; $w = Get-PlinthWindow; if ($w -and $w.Current.IsOffscreen) { $w = $null } }
  Check "a second launch opens the hub" ([bool]$w)
} finally { Get-Process plinth-app -ErrorAction SilentlyContinue | Stop-Process -Force }
$bad = @($results | Where-Object { -not $_ }).Count
"{0} of {1} background checks passed" -f ($results.Count - $bad), $results.Count
if ($bad -gt 0) { exit 1 }
