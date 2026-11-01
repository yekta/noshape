# The installed `noshape` typed in a terminal (release.yml verify, Windows). cmd and PowerShell resolve it to
# noshape.com, the console launcher, which waits for noshape.exe (a GUI program they wouldn't wait for): a command's
# output comes before the next prompt and its exit code comes through; opening the app doesn't keep the terminal.
#   check_terminal.ps1 <install dir>
param([Parameter(Mandatory)][string]$Dir)
function Fail([string]$m) { Write-Host "FAIL: $m"; exit 1 }

# Which line of `$out` (text) matches first: the command's output must come before the marker printed next.
function Assert-Before([string]$what, [string]$out, [string]$first, [string]$then) {
  $lines = $out -split "`r?`n"
  $a = [array]::FindIndex($lines, [Predicate[string]]{ param($l) $l -match $first })
  $b = [array]::FindIndex($lines, [Predicate[string]]{ param($l) $l -match $then })
  Write-Host "--- $what`n$out"
  if ($a -lt 0 -or $b -lt 0 -or $a -gt $b) { Fail "$what`: '$first' (line $a) must come before '$then' (line $b)" }
}

$found = (Get-Command noshape).Source
if ($found -ne "$Dir\noshape.com") { Fail "noshape resolves to $found, not the console launcher" }

# cmd reading its commands from a pipe runs as at its prompt: prompt, command, output, next prompt.
$cmd = "noshape --version`r`necho NEXT-PROMPT`r`nexit`r`n" | cmd.exe 2>&1 | Out-String
Assert-Before 'cmd' $cmd '^noshape \d' '>echo NEXT-PROMPT'
# For comparison (not checked): the GUI exe itself, which cmd doesn't wait for.
$gui = "noshape.exe --version`r`necho NEXT-PROMPT`r`nexit`r`n" | cmd.exe 2>&1 | Out-String
Write-Host "--- cmd with noshape.exe (for comparison)`n$gui"

foreach ($shell in 'pwsh', 'powershell') {
  $out = & $shell -NoProfile -Command "noshape --version; 'NEXT-PROMPT'" 2>&1 | Out-String
  Assert-Before $shell $out '^noshape \d' '^NEXT-PROMPT'
}

# Exit codes come through the launcher (2: a failed command).
cmd /c "noshape call no_such_tool {} > NUL"
if ($LASTEXITCODE -ne 2) { Fail "cmd: exit code $LASTEXITCODE, expected 2" }
& pwsh -NoProfile -Command "noshape call no_such_tool '{}' | Out-Null; exit `$LASTEXITCODE"
if ($LASTEXITCODE -ne 2) { Fail "pwsh: exit code $LASTEXITCODE, expected 2" }

# Opening a document returns at once, also when its output is read through a pipe (a background job's): the window runs
# on its own and holds none of the launcher's handles.
$doc = Join-Path $env:RUNNER_TEMP 'launcher-check.noshape'
$t = [Diagnostics.Stopwatch]::StartNew()
$job = Start-Job { cmd /c "noshape `"$using:doc`"" }
$done = Wait-Job $job -Timeout 60
Get-Process noshape -ErrorAction SilentlyContinue | Stop-Process -Force
if (-not $done) { Stop-Job $job; Fail 'opening a document kept the terminal waiting (60 s)' }
Write-Host "opening a document returned after $([math]::Round($t.Elapsed.TotalSeconds, 2)) s"
Write-Host 'OK'
exit 0  # not the exit code of the last check above (2 on purpose)
