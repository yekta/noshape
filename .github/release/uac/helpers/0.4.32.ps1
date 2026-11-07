# noshape's update helper (crates/update): once the app has quit, run the new installer silently (the install
# folder, its PATH entry and the agents' connections stay) or swap the portable exe, then start noshape with the
# documents that were open. If the update can't be installed, the old noshape starts again and says why.
param([int]$AppPid, [string]$Mode, [string]$Exe, [string]$Payload, [string]$List)
$HelperLog = Join-Path (Split-Path $Payload) 'update-helper.log'
function Log($m) {
  Add-Content -Path $HelperLog -Value "$(Get-Date -Format o) helper: $m" -ErrorAction SilentlyContinue
  if ($env:NOSHAPE_UPDATE_LOG) { Add-Content -Path $env:NOSHAPE_UPDATE_LOG -Value "helper: $m" }
}
function Failed($why) {
  Log "failed: $why"
  Remove-Item Env:NOSHAPE_UPDATE_AUTORESTART -ErrorAction SilentlyContinue
  Start-Process -FilePath $Exe -ArgumentList '--update-failed', ('"' + $why + '"'), '--reopen', ('"' + $List + '"')
  exit 1
}
while (Get-Process -Id $AppPid -ErrorAction SilentlyContinue) { Start-Sleep -Milliseconds 200 }
# Other copies from the install folder (a coding agent's `noshape mcp`, a terminal's `noshape`) hold the files Windows
# won't replace; the agents start them again when they need them.
$dir = Split-Path $Exe
Get-Process -ErrorAction SilentlyContinue | Where-Object { $_.Path -and (Split-Path $_.Path) -eq $dir } | ForEach-Object { Log "closing $($_.Path) ($($_.Id))"; Stop-Process -Id $_.Id -Force -ErrorAction SilentlyContinue }
if ($Mode -eq 'installer') {
  try {
    $p = Start-Process -FilePath $Payload -ArgumentList '/VERYSILENT', '/SUPPRESSMSGBOXES', '/NORESTART', '/NOCANCEL', '/FORCECLOSEAPPLICATIONS' -Verb RunAs -Wait -PassThru -ErrorAction Stop
  } catch { Failed "The update was not installed: Windows' permission prompt was declined or failed. Restart from the update card to try again." }
  if ($p.ExitCode -ne 0) { Failed "The update was not installed: its installer stopped (code $($p.ExitCode)). Close noshape everywhere it runs, then restart from the update card. Details: $HelperLog" }
} else {
  try {
    Copy-Item (Join-Path $Payload 'noshape.exe') $Exe -Force -ErrorAction Stop
    if (Test-Path (Join-Path $Payload 'noshape.com')) { Copy-Item (Join-Path $Payload 'noshape.com') (Join-Path $dir 'noshape.com') -Force -ErrorAction Stop }
  } catch { Failed "The update was not installed: $_" }
}
Log "swapped $Exe"
Remove-Item Env:NOSHAPE_UPDATE_AUTORESTART -ErrorAction SilentlyContinue
Start-Process -FilePath $Exe -ArgumentList '--reopen', ('"' + $List + '"')
