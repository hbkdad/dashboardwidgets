# Builds the release and packs a self-contained folder and zip: dist\Plinth-<version>-win64\ (exe, widgets, wallpapers, picture set, licence, readme)
# and dist\Plinth-<version>-win64.zip. The app finds widgets, wallpapers and pictures beside its exe, so the folder runs from anywhere.
# Usage: powershell -File scripts/package.ps1      (the zip is NOT code-signed: Windows SmartScreen will warn until a certificate is bought)
$ErrorActionPreference = "Stop"
$root = Resolve-Path "$PSScriptRoot\.."
Push-Location "$root\plinth"
try {
  cargo build --release -p plinth-app
  if ($LASTEXITCODE -ne 0) { throw "build failed" }
  $ver = (Select-String -Path "Cargo.toml" -Pattern '^version = "(.+)"').Matches[0].Groups[1].Value
  $name = "Plinth-$ver-win64"
  $dist = "$root\dist"; $out = "$dist\$name"
  Remove-Item $out, "$out.zip" -Recurse -Force -ErrorAction SilentlyContinue
  New-Item -ItemType Directory -Force $out | Out-Null
  Copy-Item "target\release\plinth-app.exe" $out
  Copy-Item "widgets" "$out\widgets" -Recurse
  Copy-Item "wallpapers" "$out\wallpapers" -Recurse
  Copy-Item "wallpaper-images" "$out\wallpaper-images" -Recurse
  Copy-Item "$root\LICENSE" $out
  @"
Plinth $ver (working name) - Windows desktop widgets, wallpapers, monitors and a security overview.
Run plinth-app.exe. Closing the window keeps Plinth running in the tray; Quit is in the tray menu.
Nothing on your desktop changes until you press Apply, and Revert puts your wallpaper back.
This build is not code-signed, so Windows may show a SmartScreen warning the first time.
Needs the Microsoft Edge WebView2 runtime (included with Windows 11 and current Windows 10).
Licence: Apache-2.0 (see LICENSE). Source: https://github.com/hbkdad/dashboardwidgets
"@ | Set-Content "$out\README.txt" -Encoding UTF8
  Compress-Archive -Path $out -DestinationPath "$out.zip" -Force
  $size = [math]::Round((Get-Item "$out.zip").Length / 1MB, 1)
  $hash = (Get-FileHash "$out.zip" -Algorithm SHA256).Hash.ToLower()
  "packed $out.zip ($size MB)`nsha256 $hash"
} finally { Pop-Location }
