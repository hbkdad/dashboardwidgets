# Usage: powershell -File measure.ps1 -Mode iframe -Widgets 5 [-Strategy progman] [-Exe path]
# Starts the probe (no Win+D matrix), waits, samples its process tree twice, reports private/WS memory and idle CPU.
param(
  [string]$Mode = "blank",
  [int]$Widgets = 5,
  [string]$Strategy = "progman",
  [string]$Exe = "$PSScriptRoot\..\..\target\release\webview-probe.exe",
  [int]$Warmup = 20,
  [int]$Window = 30
)
function Tree($rootPid) {
  $all = Get-CimInstance Win32_Process
  $set = @($rootPid); $changed = $true
  while ($changed) { $changed = $false
    foreach ($p in $all) { if (($set -contains $p.ParentProcessId) -and ($set -notcontains $p.ProcessId)) { $set += $p.ProcessId; $changed = $true } } }
  $rows = $all | Where-Object { $set -contains $_.ProcessId }
  $cpu = (Get-Process -Id $set -ErrorAction SilentlyContinue | Measure-Object CPU -Sum).Sum
  [pscustomobject]@{ Count = $rows.Count; WS = ($rows | Measure-Object WorkingSetSize -Sum).Sum / 1MB; Priv = ($rows | Measure-Object PrivatePageCount -Sum).Sum / 1MB; Cpu = $cpu }
}
$proc = Start-Process -FilePath $Exe -ArgumentList @($Strategy, "--no-matrix", "--hold", ($Warmup + $Window + 15), "--mode", $Mode, "--widgets", $Widgets) -PassThru -WindowStyle Hidden
Start-Sleep -Seconds $Warmup
$a = Tree $proc.Id; $t0 = Get-Date
Start-Sleep -Seconds $Window
$b = Tree $proc.Id; $secs = ((Get-Date) - $t0).TotalSeconds
$cores = [Environment]::ProcessorCount
$cpuPctOfOneCore = (($b.Cpu - $a.Cpu) / $secs) * 100
Stop-Process -Id $proc.Id -Force -ErrorAction SilentlyContinue
"mode={0} widgets={1} procs={2} private={3:N1}MB workingset_sum={4:N1}MB idle_cpu={5:N2}% of one core ({6:N2}% of {7} cores) over {8:N0}s" -f $Mode, $Widgets, $b.Count, $b.Priv, $b.WS, $cpuPctOfOneCore, ($cpuPctOfOneCore / $cores), $cores, $secs
