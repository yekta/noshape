# The Windows installer's update across UAC, as a user with UAC on gets it (release.yml's verify job, dry runs): an
# older release installed, then its in-app Restart's helper (helpers\<version>.ps1, or today's) run with a Medium
# token by a second admin account (as_user.ps1, a runas-style logon: the runner's own account is the built-in
# administrator, which UAC doesn't split), elevating <setup> through UAC (consent auto-approved on the runner). Then:
# <setup>'s version installed, noshape started again at Medium with the document reopened. Exits 1 otherwise.
#   update_across_uac.ps1 -From <release> -Helper <ps1> -Setup <setup.exe> -Want <version> [-Version <helper's version argument>] [-Mcp] [-ElevatedMcp]
param([string]$From, [string]$Helper, [string]$Setup, [string]$Want, [string]$Version = '', [switch]$Mcp, [switch]$ElevatedMcp)
$ErrorActionPreference = 'Stop'
if (-not ('Integrity' -as [type])) { Add-Type -Path "$PSScriptRoot\Integrity.cs" }
$label = "$From-$((Split-Path $Helper -Leaf) -replace '\.ps1$')$(if ($Mcp) { '-mcp' })$(if ($ElevatedMcp) { '-elevated-mcp' })"
$Out = "C:\Users\Public\uac\$label"
New-Item -ItemType Directory -Force $Out | Out-Null
icacls C:\Users\Public\uac /grant 'Everyone:(OI)(CI)F' | Out-Null
$dir = "$env:ProgramFiles\noshape"
$exe = "$dir\noshape.exe"
function T($m) { $l = "$(Get-Date -Format 'HH:mm:ss.fff') $m"; Write-Host $l; Add-Content "$Out\trace.txt" $l }
function Procs { Get-CimInstance Win32_Process | Where-Object { $_.Name -match '^(noshape|.*setup.*)' -or $_.CommandLine -match 'update-helper' } }
function Installed {
  $k = Get-ItemProperty 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\{6E0B3C55-2D7A-4C4B-9C8E-5F1A4E2B7D10}_is1' -ErrorAction SilentlyContinue
  [pscustomobject]@{ Version = $k.DisplayVersion; Sha = (Get-FileHash $exe).Hash.Substring(0, 12) }
}
function Stop-Noshape { Get-Process noshape -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue; Start-Sleep 1 }

Stop-Noshape
gh release download "v$From" --repo yekta/noshape --pattern "noshape-$From-windows-x86_64-setup.exe" --dir "$env:RUNNER_TEMP\uac-old" --clobber
$p = Start-Process "$env:RUNNER_TEMP\uac-old\noshape-$From-windows-x86_64-setup.exe" -ArgumentList '/VERYSILENT', '/SUPPRESSMSGBOXES', '/NORESTART', "/LOG=$Out\install-old.log" -Wait -PassThru
if ($p.ExitCode -ne 0) { throw "installing v${From}: exit $($p.ExitCode)" }
$before = Installed
T "== ${label}: v$($before.Version) installed; its update to $Want by helper $(Split-Path $Helper -Leaf)$(if ($Mcp) { ", Claude Desktop's noshape mcp running" })$(if ($ElevatedMcp) { ', an elevated noshape mcp running' })"

$doc = "C:\Users\Public\uac\update check.noshape"
if (-not (Test-Path $doc)) {
  $made = python -c "import json, subprocess, sys; r = subprocess.run([sys.argv[1], 'call', 'document_create', json.dumps({'path': sys.argv[2]})], capture_output=True, text=True, timeout=300); print(r.returncode, r.stderr)" $exe $doc 2>&1
  if (-not (Test-Path $doc)) { throw "document_create: $made" }
  icacls $doc /grant 'Everyone:F' | Out-Null
}
$pw = "Ns-$([guid]::NewGuid().ToString('N').Substring(0, 16))!"
$secure = ConvertTo-SecureString $pw -AsPlainText -Force
if (Get-LocalUser nsuser -ErrorAction SilentlyContinue) { Set-LocalUser nsuser -Password $secure } else {
  New-LocalUser -Name nsuser -Password $secure -PasswordNeverExpires | Out-Null
  Add-LocalGroupMember -Group Administrators -Member nsuser
}
Copy-Item $Setup "$Out\$(Split-Path $Setup -Leaf)"
Copy-Item $Helper "$Out\helper.ps1"
Copy-Item "$PSScriptRoot\as_user.ps1" "$Out\as_user.ps1"
if ($ElevatedMcp) {
  # A copy the helper can't end: started elevated (an administrator's terminal or agent).
  Start-Process cmd.exe -ArgumentList '/c', "powershell -NoProfile -Command Start-Sleep 3600 | `"$exe`" mcp" -WindowStyle Hidden
  Start-Sleep 3
  Get-Process noshape | ForEach-Object { T "noshape mcp $($_.Id): $([Integrity]::Of($_.Id))" }
}
$arg = "-NoProfile -ExecutionPolicy Bypass -File `"$Out\as_user.ps1`" -Helper `"$Out\helper.ps1`" -Setup `"$Out\$(Split-Path $Setup -Leaf)`" -Exe `"$exe`" -Doc `"$doc`" -Out `"$Out`""
if ($Version) { $arg += " -Version $Version" }
if ($Mcp) { $arg += ' -Mcp' }
$start = Get-Date
Start-Process powershell.exe -Credential (New-Object System.Management.Automation.PSCredential 'nsuser', $secure) -LoadUserProfile -WorkingDirectory $Out -ArgumentList $arg -WindowStyle Hidden
$seen = ''
while (((Get-Date) - $start).TotalMinutes -lt 20) {
  Start-Sleep -Milliseconds 500
  $now = @(Procs | ForEach-Object { "$($_.Name)#$($_.ProcessId)" } | Sort-Object) -join ' '
  if ($now -ne $seen) {
    T "-- processes ($([int]((Get-Date) - $start).TotalSeconds) s)"
    Procs | ForEach-Object { T ("   {0} {1} ppid {2} s{3} {4}: {5}" -f $_.Name, $_.ProcessId, $_.ParentProcessId, $_.SessionId, [Integrity]::Of([int]$_.ProcessId), $_.CommandLine) }
    $seen = $now
  }
  if ((Test-Path "$Out\as-user.txt") -and (Get-Content "$Out\as-user.txt" -Raw) -match ' done') { break }
}
$after = Installed
foreach ($f in 'as-user.txt', 'update.log', 'update-helper.log') { if (Test-Path "$Out\$f") { T "${f}:`n$((Get-Content "$Out\$f" -Raw).Trim())" } }
$started = @(Get-CimInstance Win32_Process -Filter "Name='noshape.exe'" | Where-Object { $_.CommandLine -match '--reopen' })
foreach ($s in $started) { T "noshape started again: $($s.ProcessId) $([Integrity]::Of([int]$s.ProcessId)): $($s.CommandLine)" }
$log = if (Test-Path "$Out\update.log") { Get-Content "$Out\update.log" -Raw } else { '' }
$problems = @()
if ($after.Version -ne $Want) { $problems += "installed $($after.Version), not $Want" }
if ($after.Sha -eq $before.Sha) { $problems += 'noshape.exe unchanged' }
if (-not ($started | Where-Object { [Integrity]::Of([int]$_.ProcessId) -eq 'Medium' })) { $problems += 'noshape not started again at Medium integrity' }
if ($log -notmatch 'reopened 1 document') { $problems += 'the document not reopened' }
if ($log -match 'update failed') { $problems += 'started with --update-failed' }
Stop-Noshape
if ($problems) { T "NOT UPDATED ($label): $($problems -join '; ')"; exit 1 }
T "UPDATED ($label): v$($before.Version) -> v$($after.Version) across UAC, noshape started again at Medium with its document"
