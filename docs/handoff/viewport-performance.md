# Viewport performance (agent: perf)

## Goal
The 3D viewport must hold the display's refresh rate (60 fps, and 120 fps on ProMotion Macs) during orbit,
pan, zoom, hover highlighting and selection, on realistic models, at Retina resolution (scale 2). The user
reported on 2026-10-20 (v0.4.1, Apple Silicon Mac) that the viewport doesn't always hold 60 fps.
Onshape (WebGL in a browser) stays smooth on the same machines, and we must be at least as smooth.

## Known architecture (read docs/ARCHITECTURE.md "Performance rules" and "Measuring" first)
- `crates/render`: an offscreen wgpu renderer. Each frame is rendered offscreen, read back to the CPU as BGRA8,
  and handed to GPUI as a new `RenderImage` (`crates/app/src/gpu.rs`, `viewport.rs`). GPUI then uploads
  it to its own texture atlas.
- At Retina sizes (e.g. a 2400x1500 px viewport is about 14 MB per frame) that GPU→CPU→GPU round trip, plus a new
  `RenderImage` per frame (atlas churn), is the prime suspect. Prove or disprove it with numbers first.
- Instrumentation already exists: `NOSHAPE_PERF=1`, the `NOSHAPE_PERF_ORBIT=<frames>` benchmark,
  `NOSHAPE_PERF_BENCH=zoom`, `NOSHAPE_PERF_MSAA`, `NOSHAPE_PERF_SKIP`.

## Likely direction (decide from measurements)
1. Measure per-stage time and bytes moved per frame at 1x and 2x sizes, and with simple and heavy models
   (e.g. imported STEP assemblies in /root/onshape-research or test fixtures).
2. Zero-copy presentation: render straight into a texture GPUI composites, with no CPU readback.
   - macOS: an IOSurface-backed texture shown through GPUI's surface/CVPixelBuffer path, or a patched
     vendored GPUI that composites an external Metal texture.
   - Linux/Windows: the equivalent in the vendored gpui crates. Patches go in vendor/ with
     `NOSHAPE PATCH` comments.
   - Keep the readback path only for picking, thumbnails and tests.
3. Frame pacing: render on demand, coalesce input to one frame per vsync, never queue frames behind
   each other, and present the latest camera.
4. GPU cost: MSAA level, AO pass cost, edge/line rendering, overdraw, draw-call count, and the per-frame
   upload of instance/state buffers (it should be diffs only).
5. UI thread: nothing heavy per frame in render/prepaint of the Part Studio, Assembly or Drawing views.
   qa4 is working on the keystroke re-render of the Part Studio (cached child views,
   `Viewport::project` bounds). Coordinate with qa4 rather than duplicating that work.

## Verifying
Xvfb uses Mesa llvmpipe (a software renderer), so absolute fps here isn't representative of a Mac.
Report CPU-side numbers (UI-thread ms per frame, render-thread ms, bytes copied per frame) that carry
over to a Mac, and fps under llvmpipe as a relative before/after. Add a `NOSHAPE_PERF` summary the
user can run on their Mac and send back (mean/p95 frame time, presented fps), and write the exact
command in this file.

## Rules
- docs/handoff/TEAM-RULES.md applies (no deviations from Onshape's look and behaviour, build group,
  disk, and C++ must build on GCC 13).
- Build group: Engine, `CARGO_TARGET_DIR=/root/target-engine` (shared with kern and fs).
- Display: Xvfb :113.
- Files: `crates/render`, `crates/app/src/gpu.rs`, `crates/app/src/viewport.rs`, and vendor/gpui-*. Tell
  the lead before you edit Part Studio/Assembly/Drawing view files, since ui, ui2, psb, skb and asb work there.
- Keep everything compiling, clippy-clean (`-D warnings`) and the tests green. The lead integrates the shared
  tree; don't commit.

## Status log
(append dated entries here)
- **2026-10-20 15:05 (perf)** Started. Read brief, TEAM-RULES, ARCHITECTURE (Viewport, Performance rules,
  Measuring) and qa4's R4 notes. Machine load 30–70 (other agents' builds + llvmpipe apps), so wall-clock
  numbers here are noise; the metrics that carry over are **thread CPU time** and **bytes moved per frame**.
- **15:10 Baseline (v0.4.1-era binary, demo doc, 1194x794 px, scale 1, llvmpipe):** orbit bench 2.0 fps;
  `w.gpu_wait` ~260 ms (llvmpipe rasterizing under load), `r.readback_copy` 1.0 ms for 3.8 MB.
- **Bottleneck analysis (code reading, per presented frame of a W×H device-px viewport, 4·W·H bytes = 15 MB
  at 2400x1560):**
  1. render thread: GPU copy texture→buffer, map, CPU copy out (`r.readback_copy`) into a fresh 15 MB Vec
     (page faults on every frame);
  2. UI thread, GPUI atlas (macOS `metal_atlas.rs`, Linux `wgpu_atlas.rs`): the image is larger than the
     1024² atlas pages, so **every frame allocates a new atlas texture** of the frame size and uploads into
     it (macOS `replaceRegion` = CPU copy + swizzle into GPU tiling; Linux `bytes.to_vec()` + `write_texture`
     staging copy = two more copies); the old texture is freed when the next frame arrives;
  3. UI thread: `cx.notify()` on every presented frame → GPUI marks the Viewport **and its ancestors**
     (PartStudioView, Workspace) dirty → the whole window is rebuilt, laid out and painted per frame
     (qa4 measured Window::draw ≈ 17.5 ms UI CPU per frame for the Part Studio, dev build).
  So per frame: 3 (macOS) / 4 (Linux) full-frame CPU copies, one 15 MB GPU texture allocation, and a
  full-window re-render. At 120 Hz (8.3 ms) on Retina this cannot hold; at 60 Hz it is marginal and
  stutters whenever the atlas allocation or a page-fault burst lands.
- **Plan:** (A) zero-copy presentation on every platform GPUI lets us: the renderer leaves the frame in a
  small ring of GPU textures and GPUI samples it directly via a new `Window::paint_external_texture`
  (vendored gpui-pre core patch, registry `set_external_texture` keyed by a stable id, so the newest
  texture is drawn even when a cached paint is replayed). Linux: the renderer runs on GPUI's own wgpu
  device (gpui-pre-wgpu patch `wait_for_shared_context`). macOS: same MTLDevice (Apple silicon has one
  GPU); gpui-pre-apple patch resolves the external `MTLTexture` and reports frame completion so a texture
  is only re-rendered once GPUI no longer samples it. Windows (D3D11 GPUI vs wgpu D3D12): readback for
  now. (B) frames that only change pixels (hover, selection, refinement) don't re-render the window:
  `Window::request_present` re-presents the last scene. (C) camera-following overlays still need a
  window render per orbit frame: needs cached child views in Part Studio/Workspace (qa4's item 3, ui's
  files) → proposal to the lead.
- **2026-10-20 16:15 (perf)** Lead: go ahead with (A)+(B); (C) builds on qa4's `cached_slice.rs`. Fixed the
  clippy `collapsible_if` in `crates/render/src/worker.rs` (render crate clippy `--all-targets -D warnings`
  clean). Shared tree with all patches passes `cargo check -p noshape` (no warnings).
  macOS: objc2's `ProtocolObject<dyn MTLTexture>` is not `Send`/`Sync`, so the core patch has a small
  `gpui::NativeTextureHandle` (retained pointer + release fn) that the app hands to the Metal atlas.
  Cross-check: /root/perf-scratch/macchk/run.sh (scratch crate: vendored gpui-pre + gpui-pre-apple with
  `runtime_shaders`, a copy of gpu.rs's `metal` module) passes `cargo clippy --target
  aarch64-apple-darwin -D warnings` (needs an empty `stitched_shaders.metal` in the build OUT_DIR).
- **2026-10-20 16:55 (perf)** Server OOM + reboot. Scratch moved to /root/perf-scratch (macOS check:
  `run.sh` regenerates the crate from gpu.rs, checks, then deletes its target dir). After the reboot the
  app failed to link (undefined `.llvm.*` hidden symbols = incremental cache truncated by the crash);
  `CARGO_INCREMENTAL=0` rebuild instead of deleting anything in the shared dir. **Lead:** the
  `/root/target-engine/debug/incremental/noshape-*` dirs are probably corrupt; clear them at a quiet moment.
  Clippy of the app is blocked by `crates/model/src/display.rs:50` (`chunks_exact` with a constant size;
  not mine).
