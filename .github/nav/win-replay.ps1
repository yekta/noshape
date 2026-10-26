# Navigation replay on Windows with documented input injection (CI runners;
# .github/workflows/nav.yml): SendInput wheel notches (whole and high-
# resolution WHEEL_DELTA units, with Shift / Ctrl), touch screen gestures
# through InjectTouchInput, and mouse drags. Every posted wheel is logged
# into the app's navigation trace ({"posted": …}) so check.py can recompute
# what Chrome hands Onshape for the same messages.
#
#   win-replay.ps1 -Trace <trace.jsonl>
#
# Precision touchpads cannot be injected (InjectSyntheticPointerInput takes
# PT_TOUCH and PT_PEN only); navigation.rs unit-tests their mapping.
param([Parameter(Mandatory)] [string] $Trace)
$ErrorActionPreference = 'Stop'

Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
public static class Nav {
  [StructLayout(LayoutKind.Sequential)] public struct POINT { public int x, y; }
  [StructLayout(LayoutKind.Sequential)] public struct RECT { public int left, top, right, bottom; }
  [StructLayout(LayoutKind.Sequential)] public struct MOUSEINPUT { public int dx, dy; public int mouseData; public uint dwFlags, time; public IntPtr extra; }
  [StructLayout(LayoutKind.Sequential)] public struct KEYBDINPUT { public ushort vk, scan; public uint flags, time; public IntPtr extra; }
  [StructLayout(LayoutKind.Explicit)] public struct INPUT {
    [FieldOffset(0)] public uint type;
    [FieldOffset(8)] public MOUSEINPUT mi;
    [FieldOffset(8)] public KEYBDINPUT ki;
  }
  [StructLayout(LayoutKind.Sequential)] public struct POINTER_INFO {
    public int pointerType; public uint pointerId, frameId; public int pointerFlags;
    public IntPtr sourceDevice, hwndTarget;
    public POINT ptPixelLocation, ptHimetricLocation, ptPixelLocationRaw, ptHimetricLocationRaw;
    public uint dwTime, historyCount; public int inputData; public uint dwKeyStates;
    public ulong performanceCount; public int buttonChangeType;
  }
  [StructLayout(LayoutKind.Sequential)] public struct POINTER_TOUCH_INFO {
    public POINTER_INFO pointerInfo; public int touchFlags, touchMask;
    public RECT rcContact, rcContactRaw; public uint orientation, pressure;
  }
  [DllImport("user32.dll")] public static extern uint SendInput(uint n, INPUT[] inputs, int size);
  [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
  [DllImport("user32.dll")] public static extern bool ClientToScreen(IntPtr hwnd, ref POINT p);
  [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr hwnd);
  [DllImport("user32.dll")] public static extern bool InitializeTouchInjection(uint maxCount, uint mode);
  [DllImport("user32.dll")] public static extern bool InjectTouchInput(uint count, [In] POINTER_TOUCH_INFO[] contacts);

  static void Send(INPUT i) { SendInput(1, new[] { i }, Marshal.SizeOf(typeof(INPUT))); }
  public static void Mouse(uint flags, int data) {
    var i = new INPUT { type = 0 }; i.mi.dwFlags = flags; i.mi.mouseData = data; Send(i);
  }
  public static void Key(ushort vk, bool down) {
    var i = new INPUT { type = 1 }; i.ki.vk = vk; i.ki.flags = down ? 0u : 2u; Send(i);
  }
  // fingers: x0, y0, x1, y1, … in screen pixels; flags per POINTER_FLAG_*.
  public static bool Touch(int[] xy, int flags) {
    int n = xy.Length / 2;
    var c = new POINTER_TOUCH_INFO[n];
    for (int k = 0; k < n; k++) {
      c[k].pointerInfo.pointerType = 2; // PT_TOUCH
      c[k].pointerInfo.pointerId = (uint)k;
      c[k].pointerInfo.pointerFlags = flags;
      c[k].pointerInfo.ptPixelLocation = new POINT { x = xy[2 * k], y = xy[2 * k + 1] };
      c[k].touchMask = 0;
    }
    return InjectTouchInput((uint)n, c);
  }
}
'@

function Read-Trace { if (Test-Path $Trace) { Get-Content $Trace | Where-Object { $_ } | ForEach-Object { $_ | ConvertFrom-Json } } }
function Log($obj) { Add-Content -Path $Trace -Value ($obj | ConvertTo-Json -Compress -Depth 5) }

$proc = $null
for ($i = 0; $i -lt 120 -and -not $proc; $i++) {
  $proc = Get-Process noshape -ErrorAction SilentlyContinue | Where-Object { $_.MainWindowHandle -ne 0 } | Select-Object -First 1
  Start-Sleep 1
}
if (-not $proc) { throw 'no noshape window' }
$hwnd = $proc.MainWindowHandle
[Nav]::SetForegroundWindow($hwnd) | Out-Null
# Windows turns UI Automation on for the app at some point of the run (run
# 40252517286: during touch-pinch, "Accessibility activated" in the app log);
# the app's one-time accessibility build then holds its UI thread for a few
# hundred ms and the OS coalesces the touch frames sent meanwhile (the pinch's
# 20 frames arrived as 2, so its per-frame steps were lost). Turn it on before
# any step and let it settle.
try {
  Add-Type -AssemblyName UIAutomationClient
  $root = [System.Windows.Automation.AutomationElement]::FromHandle($hwnd)
  $null = $root.FindAll([System.Windows.Automation.TreeScope]::Descendants, [System.Windows.Automation.Condition]::TrueCondition)
} catch { Write-Host "::warning::UI Automation warm-up failed: $_" }
Start-Sleep 2
$o = New-Object Nav+POINT; [Nav]::ClientToScreen($hwnd, [ref]$o) | Out-Null

# The viewport (wait for its first traced camera; nudge it with a wheel).
$cam = $null
for ($i = 0; $i -lt 120 -and -not $cam; $i++) {
  $cam = Read-Trace | Where-Object { $_.camera -and $_.viewport[0] -gt 100 } | Select-Object -Last 1
  if (-not $cam) { [Nav]::SetCursorPos(($o.x + 500), ($o.y + 400)) | Out-Null; [Nav]::Mouse(0x800, 120); Start-Sleep 0.3; [Nav]::Mouse(0x800, -120); Start-Sleep 2 }
}
if (-not $cam) { throw 'noshape never traced a camera' }
Start-Sleep 8
$origin = $cam.origin; $size = $cam.viewport
# Function arguments are typed: PowerShell passes `-200` to an untyped
# parameter as the string "-200", and "-200" * 2 is "-200-200".
# Calibrate logical pixels -> screen pixels with two probes inside the
# viewport (each waits for the mouse move it causes to be traced). Note:
# in PowerShell `@($a + 1, $b)` is `$a + (1, $b)`: parenthesise elements.
function Probe([int] $sx, [int] $sy) {
  $before = @(Read-Trace | Where-Object { $_.input -eq 'mouse_move' }).Count
  [Nav]::SetCursorPos($sx, $sy) | Out-Null
  for ($k = 0; $k -lt 30; $k++) {
    Start-Sleep 0.1
    $moves = @(Read-Trace | Where-Object { $_.input -eq 'mouse_move' })
    if ($moves.Count -gt $before) { return , $moves[-1].data.at }
  }
  throw "no mouse move traced for the probe at ($sx, $sy)"
}
$s0 = @(($o.x + [int]($origin[0] + $size[0] * 0.3)), ($o.y + [int]($origin[1] + $size[1] * 0.3)))
$s1 = @(($o.x + [int]($origin[0] + $size[0] * 0.6)), ($o.y + [int]($origin[1] + $size[1] * 0.6)))
$a0 = Probe $s0[0] $s0[1]; $a1 = Probe $s1[0] $s1[1]
$scale = ($s1[0] - $s0[0]) / ($a1[0] - $a0[0])
function Screen([double] $dx, [double] $dy) {
  $lx = $size[0] / 2 + $dx; $ly = $size[1] / 2 + $dy
  @([int]($s0[0] + ($lx - $a0[0]) * $scale), [int]($s0[1] + ($ly - $a0[1]) * $scale))
}
function Step($name, [scriptblock] $body) {
  Log @{ step = $name; at = 'start' }
  & $body
  Start-Sleep 0.6
  Log @{ step = $name; at = 'end' }
}
function MoveTo($p) { [Nav]::SetCursorPos($p[0], $p[1]) | Out-Null; Start-Sleep 0.15 }
function Wheel([int] $data, [switch] $Horizontal) {
  Log @{ posted = @{ kind = 'scroll'; wheel = $data; horizontal = [bool]$Horizontal } }
  [Nav]::Mouse($(if ($Horizontal) { 0x1000 } else { 0x800 }), $data); Start-Sleep 0.12
}
function Drag([int] $down, [int] $up, [double] $dx, [double] $dy, [double] $pause) {
  $p = Screen 0 0; MoveTo $p
  [Nav]::Mouse($down, 0); Start-Sleep 0.05
  for ($k = 1; $k -le 20; $k++) { [Nav]::SetCursorPos($p[0] + [int]($dx * $scale * $k / 20), $p[1] + [int]($dy * $scale * $k / 20)) | Out-Null; Start-Sleep 0.016 }
  Start-Sleep $pause
  [Nav]::Mouse($up, 0)
  Start-Sleep $(if ($pause -lt 0.05) { 1.2 } else { 0.2 })
}
# Send a touch frame and hold it, repeating it as an update every ~30 ms
# ($hold times), so each position reaches the app as its own event: on the
# runners' software renderer a frame can take ~150 ms, and Windows
# coalesces the touch updates sent meanwhile (run 40268926730: touch-pan's
# finger jumped 60 px in one event, which Onshape's per-event pan rule
# drops, as it would in Chrome). The repeats keep the injected contacts
# alive (run 40287408478: contacts left without updates were cancelled).
# Holds stay short and steps small: two fingers that stay within 10 px for
# 750 ms start a box selection (run 40293268330, 400 ms holds).
function TouchFrame([int[]] $xy, [int] $flags, [int] $hold) {
  [Nav]::Touch($xy, $flags) | Out-Null
  for ($i = 0; $i -lt $hold; $i++) { Start-Sleep -Milliseconds 25; [Nav]::Touch($xy, (0x20000 -bor 0x2 -bor 0x4)) | Out-Null }
}
function TouchGesture([object[]] $from, [object[]] $to, [int] $steps) {
  # $from / $to: arrays of logical (dx, dy) offsets from the viewport centre per finger.
  $pt = { param($f, $t) $q = Screen ($f[0] + ($t[0] - $f[0]) * $args[0]) ($f[1] + ($t[1] - $f[1]) * $args[0]); $q }
  $frame = { param($t) $xy = @(); for ($k = 0; $k -lt $from.Count; $k++) { $xy += (& $pt $from[$k] $to[$k] $t) }; [int[]]$xy }
  TouchFrame (& $frame 0) (0x10000 -bor 0x2 -bor 0x4) 1
  for ($k = 1; $k -le $steps; $k++) { TouchFrame (& $frame ($k / $steps)) (0x20000 -bor 0x2 -bor 0x4) 4 }
  Start-Sleep 0.05
  [Nav]::Touch((& $frame 1), 0x40000) | Out-Null
  Start-Sleep 1.2
}

Step 'wheel' { MoveTo (Screen 150 80); foreach ($d in 120, 120, 120, -120, -120, -120, 30, 30, 30, 30, -30, -30, -30, -30, 7, -7) { Wheel $d }; Wheel 120 -Horizontal }
Step 'shift-wheel' { MoveTo (Screen -120 -60); [Nav]::Key(0x10, $true); Start-Sleep 0.1; Wheel 120; Wheel -120; [Nav]::Key(0x10, $false) }
Step 'ctrl-wheel' { MoveTo (Screen -120 -60); [Nav]::Key(0x11, $true); Start-Sleep 0.1; Wheel 120; Wheel -120; [Nav]::Key(0x11, $false) }
Step 'right-drag' { Drag 0x8 0x10 -200 0 0.1 }
Step 'right-drag-fling' { Drag 0x8 0x10 -200 0 0.0 }
Step 'middle-drag' { Drag 0x20 0x40 200 100 0.1 }
if ([Nav]::InitializeTouchInjection(10, 1)) {
  Step 'touch-rotate' { TouchGesture @(, @(0, 0)) @(, @(-200, 0)) 20 }
  Step 'touch-pan' { TouchGesture @(@(-50, 0), @(50, 0)) @(@(100, 0), @(200, 0)) 30 }
  Step 'touch-pinch' { TouchGesture @(@(-50, 0), @(50, 0)) @(@(-100, 0), @(100, 0)) 15 }
  Step 'touch-roll' { TouchGesture @(@(-80, 0), @(80, 0)) @(@(-69.28, -40), @(69.28, 40)) 10 }
} else { Write-Host '::warning::InitializeTouchInjection failed; touch steps skipped' }
'win replay done'
