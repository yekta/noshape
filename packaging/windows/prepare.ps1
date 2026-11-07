# Run by the installer before it copies files (noshape.iss PrepareToInstall). -Count: exit 1 if a noshape runs from
# the install folder. -Close: ends those (by process, never through their console: Restart Manager's way closed a
# whole console, and with it whatever shared it). Then waits until no program holds the files (a scanner reading
# them): exit 0 ready, 2 a copy wouldn't end, 3 a file stayed in use.
param([string]$Dir, [switch]$Count, [switch]$Close, [int]$Seconds = 120)
$mine = { @(Get-Process -Name noshape -ErrorAction SilentlyContinue | Where-Object { $_.Path -and (Split-Path $_.Path) -eq $Dir }) }
if ($Count) { if ((& $mine).Count) { exit 1 } else { exit 0 } }
if ($Close) {
  for ($i = 0; $i -lt 40 -and (& $mine).Count; $i++) {
    & $mine | ForEach-Object { Write-Output "closing $($_.Path) ($($_.Id))"; Stop-Process -Id $_.Id -Force -ErrorAction SilentlyContinue }
    Start-Sleep -Milliseconds 250
  }
  if ((& $mine).Count) { exit 2 }
}
$files = @('noshape.exe', 'noshape.com' | ForEach-Object { Join-Path $Dir $_ } | Where-Object { Test-Path $_ })
$end = (Get-Date).AddSeconds($Seconds)
while ($true) {
  $busy = @($files | Where-Object { try { [IO.File]::Open($_, 'Open', 'ReadWrite', 'None').Close(); $false } catch { $true } })
  if ($busy.Count -eq 0) { exit 0 }
  if ((Get-Date) -gt $end) { Write-Output "still in use: $($busy -join ', ')"; exit 3 }
  Start-Sleep -Milliseconds 500
}
