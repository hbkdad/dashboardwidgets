# Measures the cost of each wallpaper kind against a no-wallpaper baseline, desktop visible and covered.
# Usage: powershell -File scripts/measure-wallpapers.ps1 -Video <path to an .mp4> [-Seconds 20]
# Uses a private copy of the exe so a rebuild does not collide, and a throwaway user-wallpaper folder.
param([Parameter(Mandatory)][string]$Video, [int]$Seconds = 20)
$root = Resolve-Path "$PSScriptRoot\.."
$measure = "$root\plinth\target\measure"; New-Item -ItemType Directory -Force $measure | Out-Null
Copy-Item "$root\plinth\target\release\plinth-app.exe" "$measure\plinth-app.exe" -Force
$env:PLINTH_EXE = "$measure\plinth-app.exe"
$env:PLINTH_WIDGETS = "$root\plinth\widgets"
$env:PLINTH_WALLPAPERS = "$root\plinth\wallpapers"
$user = Join-Path $env:TEMP "plinth-user-walls"; $d = Join-Path $user "testloop"
New-Item -ItemType Directory -Force $d | Out-Null
Copy-Item $Video (Join-Path $d "clip.mp4") -Force
[System.IO.File]::WriteAllText((Join-Path $d "wallpaper.json"), '{"id":"user.testloop","name":"Test loop","kind":"video","entry":"clip.mp4"}', (New-Object System.Text.UTF8Encoding($false)))
$env:PLINTH_USER_WALLPAPERS = $user
$base = Join-Path $env:TEMP "plinth-wall-base.json"
if (-not (Test-Path $base)) { throw "need a base state file at $base (a state.json with the first-party grants)" }
foreach ($w in @("", "com.plinth.aurora", "com.plinth.starfield", "com.plinth.embers", "user.testloop")) {
  foreach ($mode in @("visible", "covered")) {
    "--- wallpaper=[$w] $mode"
    if ($mode -eq "visible") { & "$PSScriptRoot\measure-app.ps1" -State $base -HideHub -ShowDesktop -Seconds $Seconds -Wallpaper $w }
    else { & "$PSScriptRoot\measure-app.ps1" -State $base -HideHub -Seconds $Seconds -Wallpaper $w }
  }
}
