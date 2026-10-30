# UI Automation helper for click-testing the real Plinth hub window (WebView2 exposes its accessibility tree).
# Usage: . scripts/uia.ps1 ; $w = Get-PlinthWindow ; Show-Elements $w ; Invoke-ByName $w "Apply to my desktop"
Add-Type -AssemblyName UIAutomationClient, UIAutomationTypes
function Get-PlinthWindow {
  $root = [System.Windows.Automation.AutomationElement]::RootElement
  $cond = New-Object System.Windows.Automation.PropertyCondition([System.Windows.Automation.AutomationElement]::NameProperty, "Plinth")
  $root.FindFirst([System.Windows.Automation.TreeScope]::Children, $cond)
}
function Show-Elements($win, [string[]]$types = @("Button","Text","CheckBox","RadioButton")) {
  $all = $win.FindAll([System.Windows.Automation.TreeScope]::Descendants, [System.Windows.Automation.Condition]::TrueCondition)
  foreach ($e in $all) { $t = $e.Current.ControlType.ProgrammaticName -replace "ControlType\.",""; if ($types -contains $t -and $e.Current.Name) { "{0,-12} {1}" -f $t, $e.Current.Name } }
}
function Find-ByName($win, [string]$name) {
  $cond = New-Object System.Windows.Automation.PropertyCondition([System.Windows.Automation.AutomationElement]::NameProperty, $name)
  $win.FindFirst([System.Windows.Automation.TreeScope]::Descendants, $cond)
}
function Invoke-ByName($win, [string]$name) {
  $e = Find-ByName $win $name
  if (-not $e) { return "NOT FOUND: $name" }
  try { $p = $e.GetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern); $p.Invoke(); return "invoked: $name" }
  catch { try { $p = $e.GetCurrentPattern([System.Windows.Automation.TogglePattern]::Pattern); $p.Toggle(); return "toggled: $name" } catch { try { $e.GetCurrentPattern([System.Windows.Automation.SelectionItemPattern]::Pattern).Select(); return "selected: $name" } catch { return "NO PATTERN: $name" } } }
}
function Find-Like($win, [string]$pattern) {
  $win.FindAll([System.Windows.Automation.TreeScope]::Descendants, [System.Windows.Automation.Condition]::TrueCondition) | Where-Object { $_.Current.Name -like $pattern } | Select-Object -First 1
}
function Invoke-Element($e) {
  if (-not $e) { return "NOT FOUND" }
  try { $e.GetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern).Invoke(); return "invoked" }
  catch { try { $e.GetCurrentPattern([System.Windows.Automation.SelectionItemPattern]::Pattern).Select(); return "selected" } catch { try { $e.GetCurrentPattern([System.Windows.Automation.TogglePattern]::Pattern).Toggle(); return "toggled" } catch { return "NO PATTERN" } } }
}
function Set-Text($win, [string]$name, [string]$text) {
  $e = Find-ByName $win $name
  if (-not $e) { return "NOT FOUND: $name" }
  try { $e.GetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern).SetValue($text); return "set: $name" } catch { return "NO VALUE PATTERN: $name" }
}
