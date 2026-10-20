# Viewport benchmarks of a noshape build on a Windows runner (see bench.sh).
#   bench.ps1 <noshape.exe> <document> <output dir> <modes...>
param([string]$Bin, [string]$Doc, [string]$Out, [Parameter(ValueFromRemainingArguments)][string[]]$Modes)
$ErrorActionPreference = 'Continue'
$frames = if ($env:FRAMES) { $env:FRAMES } else { '120' }
$delay = if ($env:DELAY) { [int]$env:DELAY } else { 120 }
New-Item -ItemType Directory -Force -Path $Out | Out-Null
Add-Type -AssemblyName System.Windows.Forms, System.Drawing
function Shot($path) {
  $b = [System.Windows.Forms.Screen]::PrimaryScreen.Bounds
  $bmp = New-Object System.Drawing.Bitmap $b.Width, $b.Height
  $g = [System.Drawing.Graphics]::FromImage($bmp)
  $g.CopyFromScreen($b.Location, [System.Drawing.Point]::Empty, $b.Size)
  $bmp.Save($path, [System.Drawing.Imaging.ImageFormat]::Png)
  $g.Dispose(); $bmp.Dispose()
}
function WaitFor($pattern, $log, $seconds) {
  for ($i = 0; $i -lt $seconds; $i++) {
    if ((Test-Path $log) -and (Select-String -Path $log -Pattern $pattern -Quiet)) { return $true }
    Start-Sleep 1
  }
  return $false
}
foreach ($mode in $Modes) {
  foreach ($kind in 'orbit', 'zoom') {
    $log = Join-Path $Out "$mode-$kind.log"
    $work = Join-Path $env:RUNNER_TEMP "bench-$mode-$kind"
    New-Item -ItemType Directory -Force -Path $work | Out-Null
    Copy-Item $Doc (Join-Path $work 'bench.noshape')
    Write-Host "== $mode $kind"
    $env:NOSHAPE_PERF = '1'; $env:NOSHAPE_PERF_ORBIT = $frames; $env:NOSHAPE_PERF_ORBIT_DELAY = "$delay"
    $env:NOSHAPE_PERF_BENCH = $kind; $env:NOSHAPE_PRESENT = $mode; $env:RUST_LOG = 'info'
    $p = Start-Process -FilePath $Bin -ArgumentList "`"$(Join-Path $work 'bench.noshape')`"" -RedirectStandardError $log -RedirectStandardOutput "$log.out" -PassThru
    if ($kind -eq 'orbit') {
      if (-not (WaitFor 'regen\[refine\]' $log ($delay - 15))) { Write-Host 'no refine regen logged' }
      Start-Sleep 8
      Shot (Join-Path $Out "$mode-still.png")
      if (WaitFor 'orbit bench: start' $log ($delay + 60)) {
        Start-Sleep -Milliseconds 1500; Shot (Join-Path $Out "$mode-orbit-1.png")
        Start-Sleep -Milliseconds 1500; Shot (Join-Path $Out "$mode-orbit-2.png")
      }
    }
    if (-not (WaitFor 'BENCH SUMMARY' $log ($delay + 900))) { Write-Host "::warning::no BENCH SUMMARY for $mode $kind" }
    Stop-Process -Id $p.Id -Force -ErrorAction SilentlyContinue
    Start-Sleep 2
    Select-String -Path $log -Pattern 'viewport frames|GPU:|orbit bench|BENCH SUMMARY|regen\[|panicked|ERROR' | ForEach-Object { $_.Line }
  }
}
