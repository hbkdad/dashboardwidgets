# Click-through test of the REAL Plinth hub window via Windows UI Automation (no screenshots, no mouse takeover).
# Starts the app with an isolated state file, clicks through the main flows, verifies the effects in state.json and the
# app's console log, then stops the app. Usage: powershell -File scripts/clicktest.ps1
$ErrorActionPreference = "Stop"
. "$PSScriptRoot\uia.ps1"
$root = Resolve-Path "$PSScriptRoot\.."
$exe = "$root\plinth\target\release\plinth-app.exe"
$state = Join-Path $env:TEMP "plinth-click-state.json"; $log = Join-Path $env:TEMP "plinth-click-app.txt"
Remove-Item $state, $log -ErrorAction SilentlyContinue
$env:PLINTH_STATE = $state
$env:PLINTH_USER_WALLPAPERS = Join-Path $env:TEMP "plinth-click-walls"; Remove-Item $env:PLINTH_USER_WALLPAPERS -Recurse -Force -ErrorAction SilentlyContinue
$p = Start-Process -FilePath $exe -RedirectStandardOutput $log -PassThru -WindowStyle Hidden
$results = @()
function Check($name, [bool]$ok) { $script:results += [pscustomobject]@{ Name = $name; Ok = $ok }; "{0} {1}" -f $(if ($ok) { "[PASS]" } else { "[FAIL]" }), $name }
function St { if (Test-Path $state) { Get-Content $state -Raw | ConvertFrom-Json } else { $null } }
function Wait-Until([scriptblock]$cond, [int]$secs = 8) { $end = (Get-Date).AddSeconds($secs); while ((Get-Date) -lt $end) { if (& $cond) { return $true }; Start-Sleep -Milliseconds 300 }; return $false }
try {
  $w = $null; Wait-Until { $script:w = Get-PlinthWindow; [bool]$script:w } 15 | Out-Null
  Check "hub window found" ([bool]$w)
  Start-Sleep -Seconds 2
  Check "first-run sheet is showing" (Wait-Until { [bool](Find-ByName $w "Got it") } 8)
  Invoke-ByName $w "Got it" | Out-Null
  Check "first-run dismissed (Got it gone, app acknowledged)" (Wait-Until { -not (Find-ByName $w "Got it") -and (Select-String -Path $log -Pattern "first run acknowledged" -Quiet) })
  Check "'Applied' chip hidden before applying" (-not (Find-ByName $w "Applied"))
  Check "Revert disabled before applying" (-not (Find-ByName $w "Revert").Current.IsEnabled)
  Invoke-ByName $w "Dusk Warm, calm, low glare" | Out-Null
  Check "choosing Dusk persisted scene=dusk" (Wait-Until { (St).scene -eq "dusk" })
  Invoke-ByName $w "Apply to my desktop" | Out-Null
  Check "Apply persisted applied=true" (Wait-Until { (St).applied -eq $true })
  Check "app created the desktop layer" (Wait-Until { Select-String -Path $log -Pattern "layer present: true" -Quiet })
  Check "'Applied' chip visible after applying" (Wait-Until { [bool](Find-ByName $w "Applied") })
  Invoke-ByName $w "Safety" | Out-Null
  Start-Sleep -Milliseconds 800
  $btnCond = New-Object System.Windows.Automation.PropertyCondition([System.Windows.Automation.AutomationElement]::ControlTypeProperty, [System.Windows.Automation.ControlType]::Button)
  Wait-Until { $script:sys = $w.FindAll([System.Windows.Automation.TreeScope]::Descendants, $btnCond) | Where-Object { $_.Current.Name -like "*PC Vitals*" } | Select-Object -First 1; [bool]$script:sys } 6 | Out-Null
  Check "Safety lists the PC Vitals widget" ([bool]$sys)
  Check "text uses correct Unicode (the middle dot in the widget row renders as U+00B7, no mojibake)" ($sys.Current.Name.Contains([string][char]0x00B7))
  if ($sys) { try { $sys.GetCurrentPattern([System.Windows.Automation.ExpandCollapsePattern]::Pattern).Expand() } catch { try { $sys.GetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern).Invoke() } catch {} }; Start-Sleep -Milliseconds 800 }
  $before = @((St).grants.'com.plinth.pcvitals').Count
  Check "PC Vitals starts with its five readings granted" ($before -eq 5)
  Check "the CPU permission switch is reachable after expanding" (Wait-Until { [bool](Find-ByName $w "CPU load for PC Vitals") } 6)
  Invoke-ByName $w "CPU load for PC Vitals" | Out-Null
  Check "revoking CPU in the UI persisted (PC Vitals keeps its other four readings)" (Wait-Until { @((St).grants.'com.plinth.pcvitals').Count -eq 4 -and (St).grants.'com.plinth.pcvitals' -contains "system.ram" })
  Invoke-ByName $w "Safe mode Off" | Out-Null
  Check "Safe mode on persisted" (Wait-Until { (St).safe -eq $true })
  Check "safe mode button now reads On" (Wait-Until { [bool](Find-ByName $w "Safe mode On") } 6)
  Invoke-ByName $w "Safe mode On" | Out-Null
  Check "Safe mode off persisted" (Wait-Until { (St).safe -eq $false })
  Invoke-ByName $w "Network" | Out-Null
  Start-Sleep -Milliseconds 600
  Check "Network view shows the proxy explanation" ([bool](Find-ByName $w "Widgets never talk to the internet directly."))
  # --- security center ---
  Invoke-ByName $w "Security" | Out-Null
  Start-Sleep -Milliseconds 600
  Check "Security Center says it is visibility, not an antivirus" (Wait-Until { $e = $w.FindAll([System.Windows.Automation.TreeScope]::Descendants, [System.Windows.Automation.Condition]::TrueCondition) | Where-Object { $_.Current.Name -like "*visibility, not an antivirus*" } | Select-Object -First 1; [bool]$e } 6)
  Invoke-ByName $w "Scan now" | Out-Null
  Check "a scan completes and shows the Windows protection card" (Wait-Until { [bool](Find-ByName $w "Windows protection") } 25)
  Check "the scan reports its duration" (Wait-Until { $e = $w.FindAll([System.Windows.Automation.TreeScope]::Descendants, [System.Windows.Automation.Condition]::TrueCondition) | Where-Object { $_.Current.Name -like "Scanned in *" } | Select-Object -First 1; [bool]$e } 8)
  Check "the connections table lists programs" (Wait-Until { [bool](Find-ByName $w "Connected to the internet right now") } 6)
  # --- wallpapers ---
  Invoke-ByName $w "Wallpapers" | Out-Null
  Start-Sleep -Milliseconds 600
  Check "Wallpapers view shows its heading" (Wait-Until { [bool](Find-ByName $w "Motion that respects your PC.") } 6)
  $radioCond = New-Object System.Windows.Automation.PropertyCondition([System.Windows.Automation.AutomationElement]::ControlTypeProperty, [System.Windows.Automation.ControlType]::RadioButton)
  $card = $null; Wait-Until { $script:card = $w.FindAll([System.Windows.Automation.TreeScope]::Descendants, $radioCond) | Where-Object { $_.Current.Name -like "Aurora*" } | Select-Object -First 1; [bool]$script:card } 6 | Out-Null
  Check "the bundled Aurora wallpaper is listed" ([bool]$card)
  if ($card) { try { $card.GetCurrentPattern([System.Windows.Automation.SelectionItemPattern]::Pattern).Select() } catch { try { $card.GetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern).Invoke() } catch {} } }
  Check "choosing Aurora persisted wallpaper=com.plinth.aurora" (Wait-Until { (St).wallpaper -eq "com.plinth.aurora" })
  Check "wallpaper change was handled by the app" (Wait-Until { Select-String -Path $log -Pattern "hub: wallpaper handled" -Quiet })
  Check "the cost chip names the wallpaper's measured cost" (Wait-Until { $e = $w.FindAll([System.Windows.Automation.TreeScope]::Descendants, [System.Windows.Automation.Condition]::TrueCondition) | Where-Object { $_.Current.Name -like "adds *MB*" } | Select-Object -First 1; [bool]$e } 6)
  # --- background mode and single instance ---
  Check "tray icon was created" (Select-String -Path $log -Pattern "tray icon: created" -Quiet)
  Invoke-ByName $w "Close" | Out-Null
  Check "closing the hub hides it and the app keeps running in the background" (Wait-Until { (Select-String -Path $log -Pattern "hub hidden" -Quiet) -and -not $p.HasExited })
  Check "hub window is no longer on screen after close" (Wait-Until { $h = Get-PlinthWindow; (-not $h) -or $h.Current.IsOffscreen } 6)
  Check "applied scene stays applied while the hub is hidden" ((St).applied -eq $true)
  $log2 = Join-Path $env:TEMP "plinth-click-app2.txt"; Remove-Item $log2 -ErrorAction SilentlyContinue
  $p2 = Start-Process -FilePath $exe -RedirectStandardOutput $log2 -PassThru -WindowStyle Hidden
  Wait-Until { $p2.HasExited } 10 | Out-Null
  Check "a second launch does not start a second app (exits at once)" ($p2.HasExited)
  Check "second launch said Plinth is already running" (Select-String -Path $log2 -Pattern "already running" -Quiet)
  Check "second launch brought the hub back" (Wait-Until { $h = Get-PlinthWindow; $h -and -not $h.Current.IsOffscreen } 8)
  $w = Get-PlinthWindow
  Invoke-ByName $w "Scenes" | Out-Null
  Check "Revert is reachable and enabled after applying" (Wait-Until { $r = Find-ByName $w "Revert"; $r -and $r.Current.IsEnabled } 8)
  Invoke-ByName $w "Revert" | Out-Null
  Check "Revert persisted applied=false" (Wait-Until { (St).applied -eq $false })
  Check "app reported revert" (Wait-Until { Select-String -Path $log -Pattern "hub: revert" -Quiet })
} catch {
  "SCRIPT ERROR: $($_.Exception.Message)"
  $results += [pscustomobject]@{ Name = "script aborted: $($_.Exception.Message)"; Ok = $false }
} finally {
  if (@($results | Where-Object { -not $_.Ok }).Count -gt 0) {
    "--- diagnostics ---"
    $h = Get-PlinthWindow; "hub window now: exists=$([bool]$h) offscreen=$(if ($h) { $h.Current.IsOffscreen } else { '-' })"
    "running plinth-app processes: $(@(Get-Process plinth-app -ErrorAction SilentlyContinue).Count)"
    "--- app log tail ---"; Get-Content $log -Tail 14
    if (Test-Path $log2) { "--- second launch log ---"; Get-Content $log2 -Tail 5 }
  }
  if ($p -and -not $p.HasExited) { Stop-Process -Id $p.Id -Force }
}
$bad = @($results | Where-Object { -not $_.Ok }).Count
"{0} of {1} click checks passed" -f ($results.Count - $bad), $results.Count
if ($bad -gt 0) { exit 1 }
