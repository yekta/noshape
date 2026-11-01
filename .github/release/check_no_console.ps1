# Opening a document creates no console window (release.yml verify, Windows; P0 11-01: every open flashed one, from
# `tasklist`). A first noshape.exe keeps a document open, so the second one, opening another document, checks the
# first's process (what ran tasklist); meanwhile every new visible console window is recorded, polled every few ms.
#   check_no_console.ps1 <noshape.exe> [-Report]     (-Report: print what was seen, don't fail: for an older release)
param([Parameter(Mandatory)][string]$Exe, [switch]$Report)

Add-Type @"
using System; using System.Text; using System.Threading; using System.Collections.Generic; using System.Runtime.InteropServices;
public static class ConsoleWindows {
  delegate bool EnumProc(IntPtr h, IntPtr p);
  [DllImport("user32.dll")] static extern bool EnumWindows(EnumProc f, IntPtr p);
  [DllImport("user32.dll")] static extern bool IsWindowVisible(IntPtr h);
  [DllImport("user32.dll", CharSet = CharSet.Unicode)] static extern int GetClassName(IntPtr h, StringBuilder s, int n);
  [DllImport("user32.dll", CharSet = CharSet.Unicode)] static extern int GetWindowText(IntPtr h, StringBuilder s, int n);
  public static Dictionary<IntPtr, string> Now() {
    var r = new Dictionary<IntPtr, string>();
    EnumWindows((h, p) => {
      if (!IsWindowVisible(h)) return true;
      var c = new StringBuilder(256); GetClassName(h, c, 256);
      if (c.ToString() == "ConsoleWindowClass" || c.ToString() == "CASCADIA_HOSTING_WINDOW_CLASS") {
        var t = new StringBuilder(512); GetWindowText(h, t, 512); r[h] = c + " '" + t + "'";
      }
      return true;
    }, IntPtr.Zero);
    return r;
  }
  // Console windows that appear (even for a moment) within ms milliseconds.
  public static List<string> Watch(int ms) {
    var before = Now(); var seen = new Dictionary<IntPtr, string>();
    var end = DateTime.UtcNow.AddMilliseconds(ms);
    while (DateTime.UtcNow < end) {
      foreach (var w in Now()) if (!before.ContainsKey(w.Key)) seen[w.Key] = w.Value;
      Thread.Sleep(3);
    }
    return new List<string>(seen.Values);
  }
}
"@

# The detector sees a console program's window (else this check would prove nothing here).
Start-Process tasklist -ArgumentList '/FI', 'PID eq 4'
$control = [ConsoleWindows]::Watch(3000)
Write-Host "control (tasklist): $($control.Count) console window(s): $($control -join '; ')"
if ($control.Count -eq 0) { Write-Host 'FAIL: no console window seen for tasklist; the check cannot see them on this runner'; exit 1 }

$dir = Join-Path $env:RUNNER_TEMP ('console-check-' + [guid]::NewGuid().ToString('N').Substring(0, 8))
New-Item -ItemType Directory -Path $dir | Out-Null
$first = Start-Process $Exe -ArgumentList "`"$dir\first.noshape`"" -PassThru
Start-Sleep -Seconds 12
$second = Start-Process $Exe -ArgumentList "`"$dir\second.noshape`"" -PassThru
$seen = [ConsoleWindows]::Watch(20000)
foreach ($p in $first, $second) { if (-not $p.HasExited) { Stop-Process -Id $p.Id -Force } }
Get-Process noshape -ErrorAction SilentlyContinue | Where-Object { $_.Path -eq $Exe } | Stop-Process -Force
Write-Host "opening a document beside another noshape: $($seen.Count) console window(s): $($seen -join '; ')"
if ($Report) { exit 0 }
if ($seen.Count -gt 0) { Write-Host 'FAIL: opening a document showed a console window'; exit 1 }
Write-Host 'OK'
exit 0
