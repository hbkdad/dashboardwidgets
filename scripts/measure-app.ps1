# Measures the Plinth app's memory (private bytes, per WebView2 profile) and idle CPU after a warm-up.
# Usage: powershell -File scripts/measure-app.ps1 -State <state.json> [-HideHub] [-Motion] [-Wallpaper <id>] [-Seconds 30]
# (set PLINTH_USER_WALLPAPERS to a folder of imported wallpapers to measure a video).
# Starts the release exe with an isolated state file whose "applied" is true (so the desktop layer is up), optionally
# hides the hub through UI Automation (background mode), waits, then samples the whole process tree twice.
param([string]$State, [switch]$HideHub, [switch]$Motion, [switch]$ShowDesktop, [int]$Seconds = 30, [string]$Wallpaper = "", [string]$Overlay = "", [int]$Intensity = 2)
$ErrorActionPreference = "Stop"
. "$PSScriptRoot\uia.ps1"
$root = Resolve-Path "$PSScriptRoot\.."
$exe = if ($env:PLINTH_EXE) { $env:PLINTH_EXE } else { "$root\plinth\target\release\plinth-app.exe" }
$j = Get-Content $State -Raw | ConvertFrom-Json
$j.applied = $true
if ($j.PSObject.Properties.Name -contains "motion") { $j.motion = [bool]$Motion } else { $j | Add-Member motion ([bool]$Motion) }
if ($j.PSObject.Properties.Name -contains "wallpaper") { $j.wallpaper = $Wallpaper } else { $j | Add-Member wallpaper $Wallpaper }
if ($j.PSObject.Properties.Name -contains "overlay") { $j.overlay = $Overlay } else { $j | Add-Member overlay $Overlay }
if ($j.PSObject.Properties.Name -contains "overlay_intensity") { $j.overlay_intensity = $Intensity } else { $j | Add-Member overlay_intensity $Intensity }
$tmp = Join-Path $env:TEMP "plinth-measure-state.json"
[System.IO.File]::WriteAllText($tmp, ($j | ConvertTo-Json -Depth 8), (New-Object System.Text.UTF8Encoding($false)))
$env:PLINTH_STATE = $tmp
$p = Start-Process -FilePath $exe -PassThru -WindowStyle Hidden
try {
  Start-Sleep -Seconds 15
  if ($HideHub) { $w = Get-PlinthWindow; if ($w) { Invoke-ByName $w "Close" | Out-Null } else { "WARNING: hub window not found" } }
  if ($ShowDesktop) { Add-Type -TypeDefinition 'using System; using System.Runtime.InteropServices; public static class K { [DllImport("user32.dll")] public static extern void keybd_event(byte vk, byte scan, uint flags, UIntPtr extra); }'; [K]::keybd_event(0x5B,0,0,[UIntPtr]::Zero); [K]::keybd_event(0x44,0,0,[UIntPtr]::Zero); [K]::keybd_event(0x44,0,2,[UIntPtr]::Zero); [K]::keybd_event(0x5B,0,2,[UIntPtr]::Zero) }
  Start-Sleep -Seconds 12
  function Snap {
    $all = Get-CimInstance Win32_Process
    $set = @($p.Id); $changed = $true
    while ($changed) { $changed = $false; foreach ($q in $all) { if (($set -contains $q.ParentProcessId) -and ($set -notcontains $q.ProcessId)) { $set += $q.ProcessId; $changed = $true } } }
    $rows = $all | Where-Object { $set -contains $_.ProcessId }
    $cpu = (Get-Process -Id $set -ErrorAction SilentlyContinue | Measure-Object CPU -Sum).Sum
    $g = @{ layer = @(); hub = @(); app = @() }
    foreach ($r in $rows) { $k = if ($r.CommandLine -match "wv-layer") { "layer" } elseif ($r.CommandLine -match "wv-hub") { "hub" } else { "app" }; $g[$k] += $r }
    [pscustomobject]@{ G = $g; Cpu = $cpu; Procs = $rows.Count }
  }
  $a = Snap; $t0 = Get-Date; Start-Sleep -Seconds $Seconds; $b = Snap; $secs = ((Get-Date) - $t0).TotalSeconds
  $total = 0
  foreach ($k in "layer", "hub", "app") {
    $rows = $b.G[$k]; $priv = [math]::Round((($rows | Measure-Object PrivatePageCount -Sum).Sum) / 1MB, 1); $ws = [math]::Round((($rows | Measure-Object WorkingSetSize -Sum).Sum) / 1MB, 1)
    $total += $priv
    "{0,-6} procs={1,2} private={2,7} MB  workingset_sum={3,7} MB" -f $k, $rows.Count, $priv, $ws
  }
  "TOTAL  procs={0} private={1:N1} MB  idle cpu over {2:N0}s = {3:N2}% of one core  (motion={4}, hub hidden={5}, desktop shown={6}, wallpaper={7}, overlay={8})" -f $b.Procs, $total, $secs, ((($b.Cpu - $a.Cpu) / $secs) * 100), [bool]$Motion, [bool]$HideHub, [bool]$ShowDesktop, $Wallpaper, $Overlay
} finally { if ($ShowDesktop) { [K]::keybd_event(0x5B,0,0,[UIntPtr]::Zero); [K]::keybd_event(0x44,0,0,[UIntPtr]::Zero); [K]::keybd_event(0x44,0,2,[UIntPtr]::Zero); [K]::keybd_event(0x5B,0,2,[UIntPtr]::Zero) }; if ($p -and -not $p.HasExited) { Stop-Process -Id $p.Id -Force }; Get-Process plinth-app -ErrorAction SilentlyContinue | Stop-Process -Force }
