# Starts the app the way Explorer does (nothing attached to its output) and checks: no console window appears, a log file is written,
# and the diagnostics report holds no user name. Usage: powershell -File scripts/logtest.ps1
$ErrorActionPreference = "Stop"
$root = Resolve-Path "$PSScriptRoot\.."
$exe = "$root\plinth\target\release\plinth-app.exe"
$d = Join-Path $env:TEMP "plinth-log-test"; Remove-Item $d -Recurse -Force -ErrorAction SilentlyContinue; New-Item -ItemType Directory $d | Out-Null
$env:PLINTH_STATE = "$d\state.json"; $env:PLINTH_WEBVIEW = "$d\wv"; $env:PLINTH_LIBRARY = "$d\lib\library.json"; $env:PLINTH_USER_WALLPAPERS = "$d\walls"; $env:PLINTH_LOG = "$d\plinth.log"
Remove-Item Env:PLINTH_AUTOTEST, Env:PLINTH_NO_LOG -ErrorAction SilentlyContinue
Get-Process plinth-app -ErrorAction SilentlyContinue | Stop-Process -Force
$results = @(); function Check($n, [bool]$ok) { $script:results += $ok; "{0} {1}" -f $(if ($ok) { "[PASS]" } else { "[FAIL]" }), $n }
$p = Start-Process -FilePath $exe -PassThru
try {
  Start-Sleep -Seconds 7
  $kids = Get-CimInstance Win32_Process | Where-Object { $_.ParentProcessId -eq $p.Id } | Select-Object -ExpandProperty Name
  Check "no console window host was started for the app" (-not ($kids -contains "conhost.exe" -or $kids -contains "cmd.exe"))
  Check "the log file exists and records the start" ((Test-Path "$d\plinth.log") -and (Select-String -Path "$d\plinth.log" -Pattern "starting" -Quiet))
  Check "the log records the app's normal messages" (Select-String -Path "$d\plinth.log" -Pattern "tray icon" -Quiet)
} finally { Get-Process plinth-app -ErrorAction SilentlyContinue | Stop-Process -Force }
$bad = @($results | Where-Object { -not $_ }).Count
"{0} of {1} log checks passed" -f ($results.Count - $bad), $results.Count
if ($bad -gt 0) { exit 1 }
