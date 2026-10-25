# Builds a static, headless OpenCascade (OCCT) for noshape on Windows (MSVC).
# Usage (from a "x64 Native Tools" / VS developer PowerShell):
#   scripts\build-occt.ps1 [install-prefix]
# Result: <prefix>\include\opencascade\*.hxx and <prefix>\lib\TK*.lib
# Then build noshape with $env:OCCT_ROOT = <prefix>.
# Mirrors scripts/build-occt.sh (same CMake options).
param(
  [string]$Prefix = "$env:USERPROFILE\.cache\noshape\occt-V8_0_1"
)
$ErrorActionPreference = "Stop"
$Tag  = if ($env:OCCT_TAG) { $env:OCCT_TAG } else { "V8_0_1" }
$Work = if ($env:OCCT_WORK) { $env:OCCT_WORK } else { "$env:USERPROFILE\.cache\noshape\occt-build-$Tag" }
$Jobs = if ($env:JOBS) { $env:JOBS } else { $env:NUMBER_OF_PROCESSORS }

New-Item -ItemType Directory -Force -Path $Work | Out-Null
Set-Location $Work
$Src = "OCCT-" + $Tag.Substring(1)
# noshape's patches to OCCT (patches\occt\*.patch, applied in name order to a
# fresh source tree; the marker re-extracts it when the patch set changes).
$Patches = Join-Path (Split-Path -Parent $PSScriptRoot) "patches\occt"
$PatchFiles = @(Get-ChildItem -Path $Patches -Filter *.patch -ErrorAction SilentlyContinue | Sort-Object Name)
$PatchSet = ($PatchFiles | ForEach-Object { (Get-FileHash $_.FullName -Algorithm SHA256).Hash }) -join ","
$Marker = Join-Path $Src ".noshape-patches"
if ((Test-Path $Src) -and ((Get-Content $Marker -ErrorAction SilentlyContinue) -ne $PatchSet)) {
  Remove-Item -Recurse -Force $Src, build -ErrorAction SilentlyContinue
}
if (-not (Test-Path $Src)) {
  $url = "https://github.com/Open-Cascade-SAS/OCCT/archive/refs/tags/$Tag.tar.gz"
  Invoke-WebRequest -Uri $url -OutFile occt.tgz
  tar xzf occt.tgz
  Remove-Item occt.tgz
  foreach ($p in $PatchFiles) {
    Write-Host "Applying $($p.Name)"
    git apply --whitespace=nowarn --directory=$Src $p.FullName
    if ($LASTEXITCODE -ne 0) { throw "OCCT patch $($p.Name) failed" }
  }
  Set-Content -Path $Marker -Value $PatchSet
}

$gen = @()
if (Get-Command ninja -ErrorAction SilentlyContinue) { $gen = @("-G", "Ninja") }

cmake -S $Src -B build @gen `
  -DCMAKE_BUILD_TYPE=Release `
  "-DCMAKE_INSTALL_PREFIX=$Prefix" `
  -DCMAKE_MSVC_RUNTIME_LIBRARY=MultiThreadedDLL `
  -DCMAKE_POSITION_INDEPENDENT_CODE=ON `
  -DBUILD_LIBRARY_TYPE=Static `
  -DBUILD_DOC_Overview=OFF `
  -DBUILD_MODULE_Draw=OFF `
  -DBUILD_MODULE_FoundationClasses=ON `
  -DBUILD_MODULE_ModelingData=ON `
  -DBUILD_MODULE_ModelingAlgorithms=ON `
  -DBUILD_MODULE_Visualization=ON `
  -DBUILD_MODULE_ApplicationFramework=ON `
  -DBUILD_MODULE_DataExchange=ON `
  -DBUILD_ADDITIONAL_TOOLKITS="" `
  -DUSE_OPENGL=OFF -DUSE_GLES2=OFF -DUSE_D3D=OFF `
  -DUSE_FREETYPE=OFF -DUSE_FREEIMAGE=OFF -DUSE_OPENVR=OFF `
  -DUSE_TK=OFF -DUSE_TCL=OFF -DUSE_VTK=OFF -DUSE_FFMPEG=OFF `
  -DUSE_RAPIDJSON=OFF -DUSE_DRACO=OFF -DUSE_TBB=OFF `
  -DBUILD_SAMPLES_QT=OFF -DBUILD_Inspector=OFF `
  -DINSTALL_DIR_LAYOUT=Unix
if ($LASTEXITCODE -ne 0) { throw "cmake configure failed" }
cmake --build build --config Release --parallel $Jobs
if ($LASTEXITCODE -ne 0) { throw "cmake build failed" }
cmake --install build --config Release
if ($LASTEXITCODE -ne 0) { throw "cmake install failed" }
Write-Host "OCCT installed to $Prefix"
