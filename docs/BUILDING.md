# Building noshape

## OpenCascade (OCCT) and the geometry kernel

The `noshape-kernel` crate links a **static, headless OCCT 8** (no OpenGL,
FreeType, Tcl/Tk or TBB). Build it once and point `OCCT_ROOT` at the install
prefix:

```sh
# Linux / macOS (needs cmake, a C++17 compiler; ninja optional)
scripts/build-occt.sh ~/.cache/noshape/occt      # ~15-30 min
export OCCT_ROOT=~/.cache/noshape/occt
```

```powershell
# Windows (VS 2022 developer PowerShell: cmake + MSVC)
scripts\build-occt.ps1 $env:USERPROFILE\.cache\noshape\occt
$env:OCCT_ROOT = "$env:USERPROFILE\.cache\noshape\occt"
```

The install contains `include/opencascade/*.hxx` and `lib/libTK*.a`
(`lib/TK*.lib` on Windows). CI runs the script for each OS (cache the prefix
keyed on the OCCT tag) and sets `OCCT_ROOT`.

Then build / test the kernel:

```sh
OCCT_ROOT=/path/to/occt cargo test -p noshape-kernel
# ignored performance test (plate with 400 filleted holes), prints timings:
OCCT_ROOT=/path/to/occt cargo test -p noshape-kernel --release --test perf -- --ignored --nocapture
```

What `crates/kernel/build.rs` does:

* fails with a clear message if `OCCT_ROOT` is unset;
* compiles the C++ bridge (`crates/kernel/cpp/*.cpp`, cxx bridge in
  `src/ffi.rs`) as C++17 against `$OCCT_ROOT/include/opencascade`, always with
  `-O2` (the bridge is hot: tessellation, history);
* links the OCCT toolkits statically (`static:-bundle`, so the large archives
  are not copied into the rlib) in dependency order (dependents first), plus
  system libraries: `pthread dl m rt` on Linux; `objc` and the
  Foundation/CoreFoundation/AppKit/IOKit frameworks on macOS (referenced by
  TKService, pulled in through XCAF); `advapi32 gdi32 user32 shell32 wsock32
  ws2_32 psapi winmm ole32` on Windows. X11 is not needed on Linux (the
  X11-using objects of TKService are never referenced);
* re-runs only when the bridge sources, `build.rs` or `OCCT_ROOT` change, so
  Rust-only edits do not recompile the C++.

## openNURBS (Rhino .3dm export)

Rhino export uses openNURBS (McNeel's MIT-style licensed toolkit), built once
like OCCT and found through `OPENNURBS_ROOT`:

```sh
scripts/build-opennurbs.sh ~/.cache/noshape/opennurbs   # cmake, C++17; ~5 min
export OPENNURBS_ROOT=~/.cache/noshape/opennurbs
```

CI and release builds always include it (`.github/actions/setup-opennurbs`
caches the build and sets `NOSHAPE_REQUIRE_OPENNURBS=1`, which makes the kernel
build fail without it). Local builds may leave it out: Rhino export then reports
that it needs openNURBS. The script builds with all cores (`JOBS=2` limits it).

