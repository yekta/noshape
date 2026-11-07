# Run with a second admin account's UAC-filtered (Medium) token by update_across_uac.ps1: what the app's Restart does
# on Windows (crates/update `apply`): the update in %APPDATA%\noshape\updates, the documents list, the helper started
# hidden (CREATE_NO_WINDOW), then the app gone. The helper script is given (an old release's or today's).
param([string]$Helper, [string]$Setup, [string]$Exe, [string]$Doc, [string]$Out, [string]$Version = '', [switch]$Mcp)
$ErrorActionPreference = 'Stop'
function T($m) { Add-Content "$Out\as-user.txt" "$(Get-Date -Format 'HH:mm:ss.fff') $m" }
$il = (whoami /groups | Select-String 'Mandatory Label') -replace '\s+', ' '
T "user ${env:USERNAME}: $il"
if ($il -notmatch 'Medium') { T 'not Medium: no UAC boundary to cross'; T 'done'; exit 1 }
$upd = "$env:APPDATA\noshape\updates"
New-Item -ItemType Directory -Force $upd | Out-Null
Remove-Item "$upd\*" -Force -ErrorAction SilentlyContinue
$payload = Join-Path $upd (Split-Path $Setup -Leaf)
Copy-Item $Setup $payload -Force
Copy-Item $Helper "$upd\update-helper.ps1" -Force
$list = "$upd\reopen.json"
ConvertTo-Json @($Doc) | Set-Content -Encoding ascii $list
if ($Mcp) {
  # Claude Desktop connected, its `noshape mcp` running from the install folder (stdin a pipe from a sleeping process).
  New-Item -ItemType Directory -Force "$env:APPDATA\Claude" | Out-Null
  ConvertTo-Json @{ mcpServers = @{ noshape = @{ command = $Exe; args = @('mcp') } } } -Depth 5 | Set-Content -Encoding ascii "$env:APPDATA\Claude\claude_desktop_config.json"
  Start-Process cmd.exe -ArgumentList '/c', "powershell -NoProfile -Command Start-Sleep 3600 | `"$Exe`" mcp" -WindowStyle Hidden
  for ($i = 0; $i -lt 30 -and -not (Get-Process noshape -ErrorAction SilentlyContinue); $i++) { Start-Sleep 1 }
  T "noshape mcp: $((Get-Process noshape -ErrorAction SilentlyContinue).Id)"
}
# The app, which quits right after starting the helper.
$app = Start-Process powershell -ArgumentList '-NoProfile', '-Command', 'Start-Sleep 3' -WindowStyle Hidden -PassThru
$si = New-Object System.Diagnostics.ProcessStartInfo "$env:SystemRoot\System32\WindowsPowerShell\v1.0\powershell.exe"
$si.Arguments = "-NoProfile -NonInteractive -ExecutionPolicy Bypass -WindowStyle Hidden -File `"$upd\update-helper.ps1`" $($app.Id) installer `"$Exe`" `"$payload`" `"$list`" $Version"
$si.UseShellExecute = $false
$si.CreateNoWindow = $true
$si.EnvironmentVariables['NOSHAPE_UPDATE_LOG'] = "$Out\update.log"
$h = [System.Diagnostics.Process]::Start($si)
T "helper $($h.Id): $($si.Arguments)"
if ($h.WaitForExit(15 * 60 * 1000)) { T "helper exit $($h.ExitCode)" } else { T 'helper still running after 15 min' }
Start-Sleep 15
foreach ($f in 'update-helper.log', 'update-install.log') { if (Test-Path "$upd\$f") { Copy-Item "$upd\$f" $Out } }
T 'done'
