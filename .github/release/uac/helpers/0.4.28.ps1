# noshape's update helper (crates/update): once the app has quit, run the new installer silently (the install
# folder, its PATH entry and the agents' connections stay) or swap the portable exe, then start noshape with the
# documents that were open.
param([int]$AppPid, [string]$Mode, [string]$Exe, [string]$Payload, [string]$List)
function Log($m) { if ($env:NOSHAPE_UPDATE_LOG) { Add-Content -Path $env:NOSHAPE_UPDATE_LOG -Value "helper: $m" } }
while (Get-Process -Id $AppPid -ErrorAction SilentlyContinue) { Start-Sleep -Milliseconds 200 }
if ($Mode -eq 'installer') {
  $p = Start-Process -FilePath $Payload -ArgumentList '/VERYSILENT', '/SUPPRESSMSGBOXES', '/NORESTART', '/NOCANCEL' -Verb RunAs -Wait -PassThru
  if ($p.ExitCode -ne 0) { Log "installer exit $($p.ExitCode)"; exit 1 }
} else {
  try {
    $dir = Split-Path $Exe
    Copy-Item (Join-Path $Payload 'noshape.exe') $Exe -Force -ErrorAction Stop
    if (Test-Path (Join-Path $Payload 'noshape.com')) { Copy-Item (Join-Path $Payload 'noshape.com') (Join-Path $dir 'noshape.com') -Force -ErrorAction Stop }
  } catch { Log "copy failed: $_"; exit 1 }
}
Log "swapped $Exe"
Remove-Item Env:NOSHAPE_UPDATE_AUTORESTART -ErrorAction SilentlyContinue
Start-Process -FilePath $Exe -ArgumentList '--reopen', ('"' + $List + '"')
