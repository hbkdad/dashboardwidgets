# Focused repro for "second launch brings the hidden hub back". Usage: powershell -File scripts/relaunch-test.ps1 [-Runs 8]
param([int]$Runs = 8)
. "$PSScriptRoot\uia.ps1"
$root = Resolve-Path "$PSScriptRoot\.."
$exe = "$root\plinth\target\release\plinth-app.exe"
$fails = 0
for ($i = 1; $i -le $Runs; $i++) {
  Get-Process plinth-app -ErrorAction SilentlyContinue | Stop-Process -Force
  $state = Join-Path $env:TEMP "plinth-relaunch-state.json"; $log = Join-Path $env:TEMP "plinth-relaunch-app.txt"
  Remove-Item $state, $log -ErrorAction SilentlyContinue
  $env:PLINTH_STATE = $state
  $p = Start-Process -FilePath $exe -RedirectStandardOutput $log -PassThru -WindowStyle Hidden
  $w = $null; for ($k = 0; $k -lt 40 -and -not $w; $k++) { Start-Sleep -Milliseconds 300; $w = Get-PlinthWindow }
  Start-Sleep -Seconds 2
  Invoke-ByName $w "Got it" | Out-Null; Start-Sleep -Milliseconds 500
  Invoke-ByName $w "Close" | Out-Null
  for ($k = 0; $k -lt 30 -and -not (Select-String -Path $log -Pattern "hub hidden" -Quiet); $k++) { Start-Sleep -Milliseconds 200 }
  Start-Sleep -Milliseconds ([int](Get-Random -Minimum 0 -Maximum 1500))
  $p2 = Start-Process -FilePath $exe -PassThru -WindowStyle Hidden -RedirectStandardOutput (Join-Path $env:TEMP "plinth-relaunch-app2.txt")
  $ok = $false; $trace = @()
  for ($k = 0; $k -lt 40; $k++) {
    Start-Sleep -Milliseconds 250
    $h = Get-PlinthWindow
    $trace += ("{0}ms exists={1} offscreen={2}" -f (($k + 1) * 250), [bool]$h, $(if ($h) { $h.Current.IsOffscreen } else { "-" }))
    if ($h -and -not $h.Current.IsOffscreen) { $ok = $true; break }
  }
  $rebuilt = Select-String -Path $log -Pattern "hub window rebuilt" -Quiet
  "run {0}: {1} (rebuilt log: {2}, after {3})" -f $i, $(if ($ok) { "ok" } else { "FAIL" }), $rebuilt, $trace[-1]
  if (-not $ok) { $fails++; $trace | Select-Object -Last 4 }
  Get-Process plinth-app -ErrorAction SilentlyContinue | Stop-Process -Force
}
"$fails failures of $Runs"
