# One-command deterministic verification. Usage: powershell -File scripts/verify.ps1 [-App]
# -Monitors runs the fake-monitor lifecycle test (3 fake monitors, unplug and replug; opens small test windows, changes nothing on your desktop).
# -Library runs the picture library end to end on your REAL monitors: it sets a test picture as the wallpaper of the primary monitor and then
#   reverts; Windows' own record of your wallpapers is compared before and after (it writes the safety snapshot to the state file first).
# -Click also click-tests the real hub window through UI Automation (about 40 s, shows the desktop layer).
# Runs unit/socket tests and builds the whole workspace; with -App also runs the app's end-to-end autotest
# (applies a full-screen desktop layer for ~30 s and sends Win+D; do not run while presenting).
param([switch]$App, [switch]$Click, [switch]$Monitors, [switch]$Library, [switch]$Moods)
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
    $env:PLINTH_WEBVIEW = Join-Path $env:TEMP "plinth-verify-wv"
    $env:PLINTH_STATE = $state; $env:PLINTH_AUTOTEST = "28"
    # Wallpaper checks use a throwaway user library. Set PLINTH_TEST_VIDEO to an .mp4 (make one with scripts/make-test-video.py)
    # to also test video import, ranged streaming and resume after a pause; without it those two checks pass trivially.
    $env:PLINTH_USER_WALLPAPERS = Join-Path $env:TEMP "plinth-verify-walls"
    $env:PLINTH_LIBRARY = Join-Path $env:TEMP "plinth-verify-app-lib\library.json"
    Remove-Item $env:PLINTH_USER_WALLPAPERS -Recurse -Force -ErrorAction SilentlyContinue
    if (-not $env:PLINTH_TEST_VIDEO) { Write-Host "note: PLINTH_TEST_VIDEO not set; video checks are skipped" }
    & .\target\release\plinth-app.exe
    if ($LASTEXITCODE -ne 0) { throw "app autotest failed" }
  }
  if ($Monitors) {
    $env:PLINTH_STATE = Join-Path $env:TEMP "plinth-verify-mon-state.json"; Remove-Item $env:PLINTH_STATE -ErrorAction SilentlyContinue
    $env:PLINTH_LIBRARY = Join-Path $env:TEMP "plinth-verify-mon-lib\library.json"; $env:PLINTH_USER_WALLPAPERS = Join-Path $env:TEMP "plinth-verify-mon-walls"
    $env:PLINTH_AUTOTEST = "40"; $env:PLINTH_AUTOTEST_MON = "1"; $env:PLINTH_FAKE_MONITORS = "1920x1080@0,0*#A;1280x1024@1920,0#B;1080x1920@3200,0#C"
    & .\target\release\plinth-app.exe
    $code = $LASTEXITCODE
    Remove-Item Env:PLINTH_AUTOTEST_MON, Env:PLINTH_FAKE_MONITORS -ErrorAction SilentlyContinue
    if ($code -ne 0) { throw "monitor lifecycle test failed" }
  }
  if ($Library) {
    $fix = Join-Path $env:TEMP "plinth-verify-libfix"; python "$PSScriptRoot\make-library-fixture.py" $fix 30 | Out-Null
    $env:PLINTH_STATE = Join-Path $env:TEMP "plinth-verify-lib-state.json"; Remove-Item $env:PLINTH_STATE -ErrorAction SilentlyContinue
    $libd = Join-Path $env:TEMP "plinth-verify-libdata"; Remove-Item $libd -Recurse -Force -ErrorAction SilentlyContinue
    $env:PLINTH_LIBRARY = Join-Path $libd "library.json"; $env:PLINTH_USER_WALLPAPERS = Join-Path $env:TEMP "plinth-verify-lib-walls"
    $env:PLINTH_AUTOTEST = "60"; $env:PLINTH_AUTOTEST_LIB = $fix
    & .\target\release\plinth-app.exe
    $code = $LASTEXITCODE
    Remove-Item Env:PLINTH_AUTOTEST_LIB -ErrorAction SilentlyContinue
    if ($code -ne 0) { throw "library end-to-end test failed (if your wallpaper looks wrong, press Revert in Plinth or read os_snapshot in $($env:PLINTH_STATE))" }
  }
  if ($Moods) {
    $env:PLINTH_STATE = Join-Path $env:TEMP "plinth-verify-moods-state.json"; Remove-Item $env:PLINTH_STATE -ErrorAction SilentlyContinue
    $env:PLINTH_CARDS = Join-Path $env:TEMP "plinth-verify-cards"; Remove-Item $env:PLINTH_CARDS -Recurse -Force -ErrorAction SilentlyContinue
    $env:PLINTH_LIBRARY = Join-Path $env:TEMP "plinth-verify-moods-lib\library.json"; $env:PLINTH_USER_WALLPAPERS = Join-Path $env:TEMP "plinth-verify-moods-walls"
    $env:PLINTH_AUTOTEST = "40"; $env:PLINTH_AUTOTEST_MOODS = "1"
    & .\target\release\plinth-app.exe
    $code = $LASTEXITCODE
    Remove-Item Env:PLINTH_AUTOTEST_MOODS, Env:PLINTH_CARDS -ErrorAction SilentlyContinue
    if ($code -ne 0) { throw "moods and cards test failed" }
  }
  if ($Click) {
    # the earlier stages set test switches; the click test must start the app normally
    Remove-Item Env:PLINTH_AUTOTEST, Env:PLINTH_AUTOTEST_MOODS, Env:PLINTH_AUTOTEST_MON, Env:PLINTH_AUTOTEST_LIB, Env:PLINTH_FAKE_MONITORS, Env:PLINTH_TEST_VIDEO -ErrorAction SilentlyContinue
    & "$PSScriptRoot\clicktest.ps1"
    if ($LASTEXITCODE -ne 0) { throw "click test failed" }
    & "$PSScriptRoot\backgroundtest.ps1"
    if ($LASTEXITCODE -ne 0) { throw "background start test failed" }
  }
  Write-Host "VERIFY OK"
} finally { Pop-Location }
