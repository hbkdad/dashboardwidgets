# Real-mouse test of drag-and-drop widget placement (D35). Starts the app with an isolated state file, opens the edit-layout overlays,
# then uses the REAL mouse (SetCursorPos + mouse_event) to drag a widget on your desktop, onto the other monitor, and checks:
# click-through outside widgets, the saved placement in state.json, and that Done removes the overlay. It moves your mouse for about 20 s.
# Usage: powershell -File scripts/dragtest.ps1
$ErrorActionPreference = "Stop"
Add-Type -TypeDefinition @"
using System; using System.Runtime.InteropServices; using System.Text;
public static class Mouse {
  [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
  [DllImport("user32.dll")] public static extern void mouse_event(uint f, uint dx, uint dy, uint d, UIntPtr e);
  [DllImport("user32.dll")] public static extern IntPtr WindowFromPoint(long pt);
  [DllImport("user32.dll")] public static extern IntPtr GetAncestor(IntPtr h, uint f);
  [DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern int GetWindowText(IntPtr h, StringBuilder s, int n);
  public static string TitleAt(int x, int y) { long pt = ((long)y << 32) | (uint)x; IntPtr h = WindowFromPoint(pt); if (h == IntPtr.Zero) return "(none)"; IntPtr r = GetAncestor(h, 2); var sb = new StringBuilder(256); GetWindowText(r, sb, 256); return sb.ToString(); }
  public static void Down() { mouse_event(2, 0, 0, 0, UIntPtr.Zero); }
  public static void Up() { mouse_event(4, 0, 0, 0, UIntPtr.Zero); }
}
"@
$root = Resolve-Path "$PSScriptRoot\.."
$exe = "$root\plinth\target\release\plinth-app.exe"
$d = Join-Path $env:TEMP "plinth-drag-test"; Remove-Item $d -Recurse -Force -ErrorAction SilentlyContinue; New-Item -ItemType Directory $d | Out-Null
$state = "$d\state.json"; $log = "$d\app.txt"
$env:PLINTH_STATE = $state; $env:PLINTH_WEBVIEW = "$d\wv"; $env:PLINTH_LIBRARY = "$d\lib\library.json"; $env:PLINTH_USER_WALLPAPERS = "$d\walls"; $env:PLINTH_AUTOEDIT = "1"
Remove-Item Env:PLINTH_AUTOTEST, Env:PLINTH_FAKE_MONITORS -ErrorAction SilentlyContinue
Get-Process plinth-app -ErrorAction SilentlyContinue | Stop-Process -Force; $end = (Get-Date).AddSeconds(8); while ((Get-Process plinth-app -ErrorAction SilentlyContinue) -and (Get-Date) -lt $end) { Start-Sleep -Milliseconds 200 }
$results = @(); function Check($n, [bool]$ok) { $script:results += $ok; "{0} {1}" -f $(if ($ok) { "[PASS]" } else { "[FAIL]" }), $n }
function St { if (Test-Path $state) { try { Get-Content $state -Raw | ConvertFrom-Json } catch { $null } } }
function Wait-Until([scriptblock]$c, [int]$s = 10) { $e = (Get-Date).AddSeconds($s); while ((Get-Date) -lt $e) { if (& $c) { return $true }; Start-Sleep -Milliseconds 250 }; $false }
function Drag($x0, $y0, $x1, $y1) {
  # like a person: move onto the widget, pause (the overlay widens its window while the pointer is on a widget), then press
  [Mouse]::SetCursorPos($x0 - 6, $y0 - 6) | Out-Null; Start-Sleep -Milliseconds 200; [Mouse]::SetCursorPos($x0, $y0) | Out-Null; Start-Sleep -Milliseconds 700; [Mouse]::Down(); Start-Sleep -Milliseconds 200
  $n = 25; for ($i = 1; $i -le $n; $i++) { [Mouse]::SetCursorPos([int]($x0 + ($x1 - $x0) * $i / $n), [int]($y0 + ($y1 - $y0) * $i / $n)) | Out-Null; Start-Sleep -Milliseconds 30 }
  Start-Sleep -Milliseconds 400; [Mouse]::Up(); Start-Sleep -Milliseconds 600
}
function Placement($id) { (St).placements | Where-Object { $_.widget -eq $id } | Select-Object -First 1 }
# --background keeps the hub window closed, so nothing but the layer and the overlays is on the desktop
$p = Start-Process -FilePath $exe -ArgumentList "--background" -RedirectStandardOutput $log -PassThru -WindowStyle Hidden
try {
  Check "edit overlays opened" (Wait-Until { (Test-Path $log) -and (Select-String -Path $log -Pattern "autoedit: overlays [1-9]" -Quiet) } 25)
  Start-Sleep -Seconds 2
  $lines = Get-Content $log
  $wid = @{}; foreach ($l in $lines) { if ($l -match "^autoedit: widget (\S+) monitor (.+) screen (-?\d+) (-?\d+) (\d+) (\d+)$") { $wid[$Matches[1]] = @{ x = [int]$Matches[3]; y = [int]$Matches[4]; w = [int]$Matches[5]; h = [int]$Matches[6] } } }
  $mons = @(); foreach ($l in $lines) { if ($l -match "^autoedit: monitor (.+) id (.+) screen (-?\d+) (-?\d+) (\d+) (\d+) primary (\w+)$") { $mons += @{ name = $Matches[1]; id = $Matches[2]; x = [int]$Matches[3]; y = [int]$Matches[4]; w = [int]$Matches[5]; h = [int]$Matches[6]; primary = ($Matches[7] -eq "true") } } }
  $done = $null; foreach ($l in $lines) { if ($l -match "^autoedit: done button screen (-?\d+) (-?\d+)$") { $done = @{ x = [int]$Matches[1]; y = [int]$Matches[2] } } }
  Check "the app reported its widgets and monitors" ($wid.ContainsKey("com.plinth.clock") -and $mons.Count -ge 1 -and $done)
  $clock = $wid["com.plinth.clock"]; $cx = $clock.x + 40; $cy = $clock.y + 40
  Check "a real pointer over a widget lands on the edit overlay" ((Wait-Until { [Mouse]::TitleAt($cx, $cy) -eq "Plinth edit layout" } 8))
  $pm = $mons | Where-Object { $_.primary } | Select-Object -First 1
  # a spot on the primary monitor that is outside every widget rectangle (and outside the toolbar)
  $free = $null
  foreach ($try in @(@(0.5, 0.5), @(0.5, 0.3), @(0.4, 0.6), @(0.55, 0.45))) {
    $fx = [int]($pm.x + $pm.w * $try[0]); $fy = [int]($pm.y + $pm.h * $try[1]); $hit = $false
    foreach ($w in $wid.Values) { if ($fx -ge $w.x - 20 -and $fx -le $w.x + $w.w + 20 -and $fy -ge $w.y - 20 -and $fy -le $w.y + $w.h + 20) { $hit = $true } }
    if (-not $hit -and [Math]::Abs($fy - $done.y) -gt 80) { $free = @{ x = $fx; y = $fy }; break }
  }
  Check "found a desktop spot outside all widgets" ([bool]$free)
  $t = [Mouse]::TitleAt($free.x, $free.y)
  Check "outside the widgets the pointer reaches the desktop (icons stay clickable), not the overlay ('$t')" ($t -ne "Plinth edit layout" -and $t -ne "Plinth")
  $before = Placement "com.plinth.clock"
  Drag $cx $cy ($cx + 300) $cy
  Check "dropping saved a new place for the clock" (Wait-Until { $a = Placement "com.plinth.clock"; $a -and ($a.dx -ne $before.dx -or $a.dy -ne $before.dy -or $a.anchor -ne $before.anchor) } 8)
  $a = Placement "com.plinth.clock"
  Check "the new place is about 300 to the right (snapping moves it at most about 24) [dx=$($a.dx) dy=$($a.dy) was $($before.dx),$($before.dy)]" ($a -and [Math]::Abs($a.dx - ($before.dx + 300)) -le 30 -and [Math]::Abs($a.dy - $before.dy) -le 30)
  Check "the app logged the drop" (Select-String -Path $log -Pattern "edit: drop com.plinth.clock" -Quiet)
  # the clock's new position on screen, then drag it onto the other monitor
  $other = $mons | Where-Object { -not $_.primary } | Select-Object -First 1
  if ($other) {
    Start-Sleep -Milliseconds 800
    $pl = $null; foreach ($l in (Get-Content $log)) { if ($l -match "^edit: placed com.plinth.clock at screen (-?\d+) (-?\d+) on") { $pl = @{ x = [int]$Matches[1]; y = [int]$Matches[2] } } }
    $nx = $pl.x + 40; $ny = $pl.y + 40
    Check "the overlay followed the widget to its new place" ((Wait-Until { [Mouse]::TitleAt($nx, $ny) -eq "Plinth edit layout" } 6))
    Drag $nx $ny ($other.x + [int]($other.w / 2)) ($other.y + [int]($other.h / 2))
    Check "dragging onto the other monitor moved the widget's home there" (Wait-Until { $b = Placement "com.plinth.clock"; $b -and $b.home -eq $other.id } 8)
    $b = Placement "com.plinth.clock"
    Check "the home is the other monitor's stable id and fingerprint, saved with a corner anchor" ($b -and $b.home_fingerprint -and $b.anchor)
  } else { Check "second monitor present (NOT TESTED: only one monitor)" $true }
  Check "no widget was written back with a shrunken size (the clock keeps its 360 DIP width)" ((Placement "com.plinth.clock").w -eq 360)
  [Mouse]::SetCursorPos($done.x, $done.y) | Out-Null; Start-Sleep -Milliseconds 300; [Mouse]::Down(); Start-Sleep -Milliseconds 80; [Mouse]::Up()
  Check "Done finished editing" (Wait-Until { Select-String -Path $log -Pattern "layout editing finished" -Quiet } 6)
  Check "after Done the overlay is gone from the screen" ((Wait-Until { [Mouse]::TitleAt($cx, $cy) -ne "Plinth edit layout" } 6))
} finally { Get-Process plinth-app -ErrorAction SilentlyContinue | Stop-Process -Force; Remove-Item Env:PLINTH_AUTOEDIT -ErrorAction SilentlyContinue }
$bad = @($results | Where-Object { -not $_ }).Count
"{0} of {1} drag checks passed" -f ($results.Count - $bad), $results.Count
if ($bad -gt 0) { "app log tail:"; Get-Content $log -Tail 25 | Where-Object { $_ -notmatch "ERROR:ui" }; exit 1 }
