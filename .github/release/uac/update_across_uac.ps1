# The Windows installer's update across UAC, as a user with UAC on gets it (release.yml's verify job, dry runs): an
# older release installed, then its in-app Restart's helper (helpers\<version>.ps1, or today's) run with a Medium
# token by a second admin account (as_user.ps1, a runas-style logon: the runner's own account is the built-in
# administrator, which UAC doesn't split), elevating <setup> through UAC (consent auto-approved on the runner). Then:
# <setup>'s version installed, noshape started again at Medium with the document reopened. Exits 1 otherwise.
# -Reader <s>: another program reads noshape.exe for that long, in a console it shares with a sentinel (a scanner;
# Restart Manager closed such a console whole, GitHub's runner with it); the sentinel must live and the install wait.
# -ExpectFail: the reader never lets go; the old version must stay and start again with the reason.
#   update_across_uac.ps1 -From <release> -Helper <ps1> -Setup <setup.exe> -Want <version> [-Version <helper's version argument>] [-Mcp] [-ElevatedMcp] [-Reader <s> [-ExpectFail]]
param([string]$From, [string]$Helper, [string]$Setup, [string]$Want, [string]$Version = '', [switch]$Mcp, [switch]$ElevatedMcp, [int]$Reader = 0, [switch]$ExpectFail, [switch]$Fresh)
$ErrorActionPreference = 'Stop'
if (-not ('Integrity' -as [type])) { Add-Type -Path "$PSScriptRoot\Integrity.cs" }
$label = "$From-$((Split-Path $Helper -Leaf) -replace '\.ps1$')$(if ($Mcp) { '-mcp' })$(if ($ElevatedMcp) { '-elevated-mcp' })$(if ($Reader) { "-reader$Reader" })$(if ($Fresh) { '-fresh' })"
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
$sentinel = $null
if ($Reader) {
  $r = "$Out\reader.ps1"
  # Loaded as an image resource, as a program reading an exe's version or icon does (what Restart Manager counts as
  # using it).
  Set-Content $r @"
Add-Type -Name K -Namespace W -MemberDefinition '[DllImport("kernel32.dll", CharSet = CharSet.Unicode)] public static extern IntPtr LoadLibraryExW(string f, IntPtr h, uint flags);'
`$m = [W.K]::LoadLibraryExW('$exe', [IntPtr]::Zero, 0x22)
if (`$m -eq [IntPtr]::Zero) { exit 1 }
Start-Sleep $Reader
"@
  Set-Content "$Out\sentinel.ps1" "while (`$true) { Start-Sleep 1 }"
  Start-Process cmd.exe -ArgumentList '/c', "start /b powershell -NoProfile -File `"$r`" & powershell -NoProfile -File `"$Out\sentinel.ps1`"" -WindowStyle Hidden
  Start-Sleep 3
  $sentinel = Get-CimInstance Win32_Process | Where-Object { $_.CommandLine -match 'sentinel\.ps1' -and $_.Name -eq 'powershell.exe' } | Select-Object -First 1
  T "reading noshape.exe for $Reader s; sentinel $($sentinel.ProcessId) shares its console"
}
if ($Fresh) {
  # noshape just started (as a user's Restart comes right after launch): GitHub's runner scans the image for a while.
  & $exe --version | Out-Null
  Start-Sleep 2
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
if ($sentinel -and -not (Get-Process -Id $sentinel.ProcessId -ErrorAction SilentlyContinue)) { $problems += 'the program sharing a console with the reader was closed' }
if ($ExpectFail) {
  if ($after.Version -ne $before.Version -or $after.Sha -ne $before.Sha) { $problems += "installed $($after.Version), expected $($before.Version) to stay" }
  if ($log -notmatch 'update failed: ') { $problems += 'the old noshape not started again with the reason' }
} else {
  if ($after.Version -ne $Want) { $problems += "installed $($after.Version), not $Want" }
  if ($after.Sha -eq $before.Sha) { $problems += 'noshape.exe unchanged' }
  if (-not ($started | Where-Object { [Integrity]::Of([int]$_.ProcessId) -eq 'Medium' })) { $problems += 'noshape not started again at Medium integrity' }
  if ($log -notmatch 'reopened 1 document') { $problems += 'the document not reopened' }
  if ($log -match 'update failed') { $problems += 'started with --update-failed' }
}
Get-CimInstance Win32_Process | Where-Object { $_.CommandLine -match 'reader\.ps1|sentinel\.ps1' } | ForEach-Object { Stop-Process -Id $_.ProcessId -Force -ErrorAction SilentlyContinue }
Stop-Noshape
if ($problems) { T "NOT UPDATED ($label): $($problems -join '; ')"; exit 1 }
if ($ExpectFail) { T "REFUSED AS EXPECTED ($label): v$($before.Version) stayed, noshape started again with the reason$(if ($sentinel) { '; the reader''s console untouched' })"; exit 0 }
T "UPDATED ($label): v$($before.Version) -> v$($after.Version) across UAC, noshape started again at Medium with its document$(if ($sentinel) { '; the reader''s console untouched' })"
