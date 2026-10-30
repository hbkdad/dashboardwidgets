# One-command deterministic verification. Usage: powershell -File scripts/verify.ps1 [-App]
# -Click also click-tests the real hub window through UI Automation (about 40 s, shows the desktop layer).
# Runs unit/socket tests and builds the whole workspace; with -App also runs the app's end-to-end autotest
# (applies a full-screen desktop layer for ~30 s and sends Win+D; do not run while presenting).
param([switch]$App, [switch]$Click)
$ErrorActionPreference = "Stop"
Push-Location "$PSScriptRoot\..\plinth"
try {
  cargo test --workspace
  if ($LASTEXITCODE -ne 0) { throw "tests failed" }
  cargo build --release --workspace
  if ($LASTEXITCODE -ne 0) { throw "build failed" }
  if ($App) {
    $state = Join-Path $env:TEMP "plinth-verify-state.json"
    Remove-Item $state -ErrorAction SilentlyContinue
    $env:PLINTH_STATE = $state; $env:PLINTH_AUTOTEST = "28"
    # Wallpaper checks use a throwaway user library. Set PLINTH_TEST_VIDEO to an .mp4 (make one with scripts/make-test-video.py)
    # to also test video import, ranged streaming and resume after a pause; without it those two checks pass trivially.
    $env:PLINTH_USER_WALLPAPERS = Join-Path $env:TEMP "plinth-verify-walls"
    Remove-Item $env:PLINTH_USER_WALLPAPERS -Recurse -Force -ErrorAction SilentlyContinue
    if (-not $env:PLINTH_TEST_VIDEO) { Write-Host "note: PLINTH_TEST_VIDEO not set; video checks are skipped" }
    & .\target\release\plinth-app.exe
    if ($LASTEXITCODE -ne 0) { throw "app autotest failed" }
  }
  if ($Click) {
    & "$PSScriptRoot\clicktest.ps1"
    if ($LASTEXITCODE -ne 0) { throw "click test failed" }
  }
  Write-Host "VERIFY OK"
} finally { Pop-Location }
