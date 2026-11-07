# check_restart.py on Windows: the previous release's installer run as a user runs it, then the app started at Medium
# integrity by a second admin account (a runas-style logon, as in uac\update_across_uac.ps1: the runner's own account
# is the built-in administrator, which UAC doesn't split), so its update crosses UAC as a user's does.
#   restart_windows.ps1 -From <previous release> -Want <new release>
param([string]$From, [string]$Want)
$ErrorActionPreference = 'Stop'
$out = 'C:\Users\Public\restart'
New-Item -ItemType Directory -Force $out | Out-Null
icacls $out /grant 'Everyone:(OI)(CI)F' | Out-Null
gh release download "v$From" --repo $env:GITHUB_REPOSITORY --pattern "noshape-$From-windows-x86_64-setup.exe" --dir "$env:RUNNER_TEMP\prev" --clobber
$p = Start-Process "$env:RUNNER_TEMP\prev\noshape-$From-windows-x86_64-setup.exe" -ArgumentList '/VERYSILENT', '/SUPPRESSMSGBOXES', '/NORESTART', "/LOG=$out\install.log" -Wait -PassThru
if ($p.ExitCode -ne 0) { throw "installing v${From}: exit $($p.ExitCode)" }
$exe = "$env:ProgramFiles\noshape\noshape.exe"
$pw = "Ns-$([guid]::NewGuid().ToString('N').Substring(0, 16))!"
$secure = ConvertTo-SecureString $pw -AsPlainText -Force
if (Get-LocalUser nsuser -ErrorAction SilentlyContinue) { Set-LocalUser nsuser -Password $secure } else {
  New-LocalUser -Name nsuser -Password $secure -PasswordNeverExpires | Out-Null
  Add-LocalGroupMember -Group Administrators -Member nsuser
}
$doc = "$out\update check.noshape"
$log = "$out\update.log"
# What the account runs: noshape with the document, the log in its environment (as from a shortcut).
Set-Content "$out\as_user.ps1" @"
`$env:NOSHAPE_UPDATE_LOG = '$log'
`$env:NOSHAPE_NO_SETUP_PROMPT = '1'
Start-Process '$exe' -ArgumentList '"$doc"'
"@
Set-Content "$out\start.ps1" @"
`$c = New-Object System.Management.Automation.PSCredential 'nsuser', (ConvertTo-SecureString '$pw' -AsPlainText -Force)
Start-Process powershell.exe -Credential `$c -LoadUserProfile -WorkingDirectory '$out' -ArgumentList '-NoProfile -ExecutionPolicy Bypass -File "$out\as_user.ps1"' -WindowStyle Hidden
"@
python "$PSScriptRoot\check_restart.py" --want $Want --exe $exe --log $log --doc $doc --shots "$env:RUNNER_TEMP\restart-shots" --label 'Windows installer, across UAC' -- powershell.exe -NoProfile -ExecutionPolicy Bypass -File "$out\start.ps1"
$code = $LASTEXITCODE
Get-Process noshape -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue
if ($code -ne 0) { exit 1 }
