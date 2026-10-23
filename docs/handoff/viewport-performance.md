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

## CI measurement (the reference from 2026-10-20 on)
`.github/workflows/perf.yml` (branch `perf-ci` until the lead merges it): downloads a release (input `tag`,
default latest; also after every successful Release run), runs `.github/perf/bench.sh` / `bench.ps1` on
macOS 15 + 14 (Apple silicon, paravirtualized Metal), Windows (WARP) and Linux (Xvfb, lavapipe) with the
heavy fixture `tests/perf/nist-mtc-crada-assembly.noshape` (NIST MTC CRADA assembly, public domain,
36 parts, 670k triangles): orbit + zoom, `NOSHAPE_PRESENT=texture` and `=readback`, screenshots, then
`.github/perf/analyze.py` (step summary table, `summary.json`; macOS: zero-copy still must match the
read-back still and not be black; macOS 15: orbit p95 ≤ 16.7 ms gate). Diagnosis: input `variants`, e.g.
`msaa1:NOSHAPE_PERF_MSAA=1 nolines:NOSHAPE_PERF_SKIP=lines`.
Run: `gh workflow run perf.yml --ref perf-ci -f tag=v0.4.2`; results: `gh run download <id>` and
`python3 .github/perf/analyze.py <dir> <label> --stills`. Runner displays are not Retina (scale 1:
macOS 14 1920x1080 → 1194x794 px viewport, macOS 15 1024x768 → 778x575, Windows 1024x768).

## Measuring on a Mac (send the output back)
Open a heavy document, keep the window still, and run (the orbit starts ~10 s after launch, after a zoom to fit):
```
NOSHAPE_PERF=1 NOSHAPE_PERF_ORBIT=600 /Applications/noshape.app/Contents/MacOS/noshape \
  ~/Documents/noshape/<document>.noshape 2>&1 | grep -E "viewport frames|BENCH SUMMARY"
```
Compare with the old path: prefix `NOSHAPE_PRESENT=readback`. Zoom instead of orbit: `NOSHAPE_PERF_BENCH=zoom`.
The `BENCH SUMMARY` line gives the presentation path, device pixels, presented fps, window renders, frame
interval mean/p50/p95/p99/max, frames over 16.7 ms / 8.3 ms, and UI-thread CPU per frame.

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
- **2026-10-20 17:30 (perf) Results (A)+(B), integrated in v0.4.2.** A/B in one binary (`NOSHAPE_PRESENT`),
  Xvfb :113 + llvmpipe, debug build, orbit bench:

  | case | path | UI-thread CPU/frame (mean, p95) | render-thread CPU/frame | CPU copies/frame |
  |---|---|---|---|---|
  | demo part, 1194x794 px (scale 1) | readback | 17.3, 23.2 ms | 1.93 ms | 3 × 3.8 MB |
  | | GPU texture | **13.8, 15.8 ms** | 1.74 ms | 0 |
  | linkrods.step (OCCT sample), 2388x1588 px (scale 2) | readback | 32.6, 40.0 ms | 10.7 ms (p95 23.1) | 3 × 15.2 MB |
  | | GPU texture | **14.8, 17.4 ms** | **2.0 ms** | 0 |

  Zero-copy frames are pixel-identical to readback frames (ImageMagick AE = 0 on full-window screenshots).
  Hover highlight frames go through `request_present` only (`v.present_only_count`), no window re-render.
  llvmpipe fps is lower with a shared device (2.9 → 2.0 fps at scale 2): GPUI's queue present on a CPU
  rasterizer is synchronous, so our submit waits behind it (`r.encode_submit` 0.5 → 150 ms of waiting, CPU
  unchanged). Real GPUs present asynchronously; CPU adapters now keep readback (`gpu.rs spawn_worker`;
  `NOSHAPE_PRESENT=texture` forces it for tests here).
  New test `texture_frames_stay_on_the_gpu_and_textures_are_reused_once_released` (crates/render/tests).
  macOS: `/root/perf-scratch/macchk/run.sh` type-checks + clippies the Metal patch and gpu.rs's `metal`
  module for aarch64-apple-darwin (clean). **Not run on a Mac yet**: the Mac numbers must come from the user
  (command above). If anything looks wrong there, `NOSHAPE_PRESENT=readback` is the escape hatch.
- **(C) remaining per-frame cost while orbiting** (for qa4): with zero-copy, the ~15 ms UI CPU per orbit frame
  (debug build, this box) is the window re-render the camera-following overlay forces (overlay slice +
  PartStudioView + Workspace render bodies, layout, paint). Self-time top items on the UI thread during the
  bench: memmove ~10%, malloc/realloc ~13%, lyon `FillTessellator` 3–5% (view cube, in-plane plane labels:
  re-tessellated every frame), taffy flexbox ~5%, `ParamValue::clone` ~3% (model params cloned during
  render), wgpu descriptor sets ~3.5% (mine, see Next 2).

## Next
1. Mac numbers from the user (command above); if `BENCH SUMMARY` shows "frames read back" on Apple silicon,
   the GPU id check in `compositor_shares_gpu` failed: log `gpui::renderer_gpus()` vs `metal::RENDERER_GPU`.
2. Linux: GPUI re-creates the external texture's bind group whenever the shown texture changes (one cache slot
   per texture id): key the cache by generation in vendor/gpui-pre-wgpu `prepare_texture_bind_groups`.
3. Windows: zero-copy needs a D3D12→D3D11 shared texture (wgpu DX12 `CreateSharedHandle`, GPUI D3D11
   `OpenSharedResource1`) + fence; until then Windows reads back.
4. Camera-following overlays: cache the view cube / plane-label tessellation (lyon) per camera, coordinate
   with qa4's slices; then measure orbit UI CPU again.
5. Intel Macs with two GPUs: GPUI picks the low-power GPU, wgpu the high-performance one → readback there;
   pick the wgpu adapter whose `registryID` matches `gpui::renderer_gpus()`.
6. (lead, 2026-10-20) Preferences › "Environment profile / pixel density": take the exact options from
   live and implement their effect on rendering quality (renderer side).
7. (lead) Compare mode (ui2 builds it): grey compare graphics and a Main↔V3 blend slider in the viewport;
   ui2 asks through the lead for renderer support.
- **2026-10-20 19:00 (perf) CI run 39477961638, v0.4.2:** zero-copy on a real Metal GPU is **correct**
  (macOS 14/15 and Linux: texture still vs read-back still 0.00% pixels differ, not black; screenshots
  in the artifacts). Orbit, heavy fixture:

  | runner | path | fps | p95 interval | >16.7 ms | UI CPU/frame (mean, p95) |
  |---|---|---|---|---|---|
  | macOS 14, 1194x794 | readback | 15.8 | 73.7 ms | 99% | 6.12, 7.35 ms |
  | | texture | **27.2** | **44.0 ms** | 99% | 6.51, 9.04 ms |
  | macOS 15, 778x575 | readback | 27.0 | 44.2 ms | 98% | 6.22, 9.83 ms |
  | | texture | 27.7 | 43.3 ms | 98% | 6.16, 8.34 ms |
  | Linux lavapipe, 1194x794 | readback | 1.7 | 699 ms | 100% | 22.1, 29.9 ms |
  | | texture | 1.8 | 589 ms | 100% | 12.3, 14.0 ms |
  | Windows WARP, 778x614 | readback | 2.7 | 384 ms | 100% | n/a (no thread clock) |

  The p95 gate (16.7 ms) fails: on the runner's paravirtualized GPU frames take ~37 ms although our GPU
  work is ~11 ms (`w.gpu_wait`) and UI CPU ~6.5 ms: GPU-bound (our frame + GPUI's window frame on a slow
  virtual GPU). Diagnosis run with MSAA 1 / no lines / no meshes: 39479898303.

## Item 4 proposal (2026-10-20 19:40, perf → lead → qa4/ui): camera moves without any GPUI layout/paint
Today an orbit frame re-renders, lays out and paints the Viewport and both overlay slices (they observe the
viewport), plus their ancestors: 6.4 ms prepaint + 4.5 ms paint on this box, UI CPU p95 9–12 ms on the macOS
runner, and the runner's frame pacing follows it (37/73 ms = 2/4 refreshes).
Camera-following content today: view cube (faces, frames, corner pieces, face labels as glyph outlines,
triad lines, X/Y/Z text labels; arrows static), in-plane plane names (glyph outlines), and situational ones:
sketch labels, shown dimensions, hole callouts, analysis-tool marks, manipulators/triad.
**Design:**
1. Renderer (mine): an overlay layer in the `Scene`, drawn at the end of the frame's MSAA color pass with
   depth = near (AO skips those pixels, so no halos), never in the pick pass. Primitives:
   - filled triangle meshes in three spaces: *world* (plane names: glyph outlines laid out once in the
     plane), *widget* (view cube: cube-local 3D shapes rotated by the frame's camera rotation and drawn
     orthographically at a fixed screen corner, same math as `cube_shapes`), *screen* (static widget parts);
   - polylines (triad), with per-vertex color/alpha (hidden axis parts at 30%);
   - *billboards*: RGBA images anchored at a world point + pixel offset, pixel-snapped (X/Y/Z, dimension
     and callout labels), rasterized on the UI thread with GPUI's own text system so glyphs are identical.
   The renderer computes per-frame geometry from the frame's camera, so overlays are in sync with the
   3D image (today they can lag a frame behind it).
2. App: the view cube and plane labels become overlay data built when their inputs change (rotation is not
   an input; hover cell, planes, theme are). Hit testing stays as is (`widget_hit` with the current camera).
   The Viewport stops notifying on camera changes when the owner reports no remaining GPUI-drawn
   camera-following overlays (`Viewport::set_camera_overlays(bool)`), so an orbit frame is render +
   `request_present`. Situational overlays keep today's path until moved in later steps (sketch labels,
   shown dims, callouts, manipulators).
3. Look: pixel crops of the cube and plane labels, old (GPUI) vs new (wgpu), at scale 1 and 2, plus live
   Onshape crops; MSAA 4x in both, so edges should match within a few levels.
**Needs from qa4/ui:** OK to move the drawing (not the hit testing or the menu button) of `view_cube.rs`
(`view_cube`, `cube_shapes`, label/axis computation) into overlay builders, and to drop the plane-name
`paint_paths` from `render_viewport_underlay`; their slices then stop observing the viewport for camera
changes. I write the renderer part first (my files) and show crops before touching theirs.
- **2026-10-20 21:40 (perf)** Gates per the lead (a–d) in perf.yml + analyze.py; perf.yml can build any ref
  from source (`-f ref=…`; Windows builds in PowerShell). GPU time per frame from wgpu timestamp queries
  (`r.gpu_ms`, "GPU per frame" in BENCH SUMMARY): works on Vulkan (lavapipe); **the macOS runners' paravirtual
  Metal device has no timestamp queries**, so the GPU gate is skipped there.
  macOS runner, release builds, GPUI phases per orbit frame: request_layout 0.24 ms, **prepaint 5.1 ms**,
  paint 1.1 ms, present 0.3 ms. Runner fps is bimodal (15–16 or 27–28 fps, 2 or 4 refreshes per frame) in
  the same build (v0.4.2 shows both; main built from source and perf-dev both ~16): not a usable gate.
- **Item 4 (approved 20:00):** `crates/render/src/overlay.rs` (view cube geometry moved from view_cube.rs and
  shared with its hit testing; per-frame build from the frame's camera; lyon tessellation like GPUI's
  `PathBuilder`), `overlay_gpu.rs` + `overlay.wgsl` (4x MSAA UNORM layer, premultiplied gamma-space blending,
  composited after AO; X/Y/Z masks from `Window::rasterize_text_line`, a gpui-pre patch mirroring
  `paint_line`/`paint_glyph`, with GPUI's 4 x-sub-pixel variants and Linux text gamma from gpui-pre-wgpu
  `text_shading()`). Behind `NOSHAPE_OVERLAY=wgpu` until the lead approves the crops; then camera moves
  are present-only when the owner reports nothing else following the camera
  (`Viewport::set_ui_follows_camera`; Part Studio: sketch, dialog, shown dims, analysis tool, section).
  First crops (demo, Xvfb): plane names identical (0 px at 1x, 1 px at 2x); cube ≤0.84% of pixels differ
  (sub-pixel widget offset + label placement), fixed by using GPUI's own widget position
  (`CubeHooks::placed`) and GPUI's glyph quantization; re-measuring.
- **2026-10-21 00:40 (perf) Item 4, look and speed (local, Xvfb/lavapipe, debug build):**
  - Pixel match old (GPUI) vs new (wgpu overlay), demo document, default view, whole widget crop:
    cube 1x 233 px differ, **2 by more than 8 levels** (max 16); 2x 529 px, **5** (max 32); the rest are 1-level
    rounding; plane names **0 px** at 1x and 2x. Hover (Top face, up arrow) and click (Front) states ≤2 px.
    What it took (GPUI's exact behaviour, measured): the widget origin from GPUI's layout (`CubeHooks::placed`);
    path pixels clipped to the path's bounds, half-open [min, max) by pixel center (GPUI's wgpu path shader;
    Metal clips geometrically, so off there: `TextShading::clip_path_pixels`); X/Y/Z glyphs placed like
    `taffy.rs` lays out (authored offsets rounded half toward zero to device px, text leaf width rounded up,
    centered, absolute position rounded half toward zero) and `paint_glyph` quantizes (4 x-variants, one mask
    each); Linux text gamma/contrast from GPUI's renderer.
    Crops (not in the repo: they include a live Onshape crop): /root/perf-scratch/crops/cube-compare-sheet.png,
    planes-compare-sheet.png, hover-wgpu-row.png.
  - Orbit bench, demo at scale 2 (2388x1588), texture frames, same binary: GPUI overlays: 61 window renders
    for 60 frames, UI CPU per frame mean 12.20 p95 14.88 ms (prepaint 10.1, paint 8.1 ms); **wgpu overlays:
    2 window renders (start, fit), 60 present-only frames, UI CPU mean 2.43 p95 6.92 ms.** Needed:
    `request_frame`'s "on screen" test no longer goes stale during present-only streaks.
  - Built and measured in a private worktree (/root/perf-scratch/wt/noshape, HEAD + perf's hunks) because the
    shared tree kept failing in other files; every change is also in the shared tree. CI: run 39520247805
    (perf-dev with `NOSHAPE_OVERLAY=wgpu` as a variant).
- **2026-10-21 (perf) Freeze for v0.4.3.** In the tree: wgpu view cube + plane names (default, GPUI
  drawing removed; crops approved); manipulators/triads built per frame on the render thread from the
  frame's camera (`render::camera_geometry`, `ManipGeometry`), so they no longer re-render the window
  while orbiting (pixel crops still to do). Reverted for the freeze: a rewrite of the section caps in
  Onshape's way (float mask of ± part colors with +16 alpha per part, 3×3 edge detect + 5-tap blur,
  plane quad with hash-driven hatch, red overlap; saved at /root/perf-scratch/caps/wip). G40 finding:
  the G40 shots are 1× renders upscaled 2×, so the true hatch period is ~12.9 px at 1×, from Onshape's
  pattern scale 30/(P[5]·viewport height); a true 2× capture and the planes' tangents are requested
  in REQUESTS.md. Next: caps (after GO), BOM-panel view cube placement, class B anchored labels.

## From kern (10-21 06:30): pattern instances and TShape sharing (for instancing)
- Pattern instances that stay parts (Part pattern New; bodies a Feature pattern creates from a New feature) are
  rigidly *moved*, not copied: they share the seed's TShape and differ only in `TopLoc_Location`. Instances that
  go into a boolean are copies (moved ones make the boolean quadratic), but then the result is one merged body.
- Kernel API (uncommitted, `crates/kernel/src/shape.rs`): `Shape::partner_hash()` (TShape identity, equal for
  partners), `Shape::location() -> [f64; 12]` (3x4 row-major rigid matrix), `Shape::unlocated()` (same TShape at
  the identity location). Tessellate `unlocated()` once per `partner_hash`, draw each body with its `location()`.
  BRepMesh stores triangulations on the shared TFaces, so meshing one instance already meshes all of them in OCCT.
- Need anything else from the model/kernel: write it in docs/handoff/kernel-robustness.md ("kern ↔ perf").

- **2026-10-21 07:45 (perf, new account)** Took over mid-edit. State found and finished:
  1. **Release smoke CI** (priority 1, after v0.4.3's `+[NSCursor _moveCursor]` abort on macOS 27):
     `crates/app/src/smoke.rs` (`NOSHAPE_SMOKE=1`, hooked in `Workspace::new`): every tab + a new tab of each kind,
     a sketch with the Dimension tool started, every GPUI `CursorStyle` through the platform
     (`App::set_platform_cursor_style`, gpui-pre patch) and every `AppCursor`, Extrude / Section view / shortcuts /
     tab manager / version graph / tool search opened and closed, right-click menus of the viewport and the
     features list (synthesized events, `os_menu::is_open` logged), an orbit, quit; `SMOKE <step>` lines on
     stderr, 300 s watchdog (exit 3). macOS patch (`set_active_window_cursor_style`): under NOSHAPE_SMOKE any GPUI
     window will do and the *view's* `resetCursorRects` runs at once, so each style's NSCursor is really created
     on a runner with no pointer over the window. perf.yml job `smoke` (every Release and dispatch; input
     `jobs=all|smoke|bench`): macOS 14, 15, 26 (newest image; `macos-27` doesn't exist yet), Windows, Linux;
     `.github/perf/smoke.sh` (kill after 480 s, exit status + "SMOKE done" checked, macOS DiagnosticReports
     copied), Windows LocalDumps; artifacts `smoke-<os>`. New composite `.github/actions/noshape-binary`
     (release asset or source build of `ref`, sets BIN/BUILD), used by `smoke` and `bench`. actionlint clean.
     Releases ≤ v0.4.4 have no smoke hook, so test with `-f ref=…`: perf-ci = v0.4.4 + smoke hunks,
     run 39563625213 (`gh workflow run perf.yml --ref perf-ci -f ref=perf-ci -f jobs=smoke`).
  2. **Hidden edges drawn through thin faces** (user screenshot, ribs on a block): edges were pulled toward the
     camera by `z*1.0004 + 2e-5*w` (perspective: ~0.04% of the eye distance plus a constant in reverse-Z depth,
     i.e. millimetres at normal zoom, more than a 1 mm rib). Now `pull_toward_camera(c, half + 1 px)` in
     shaders.wgsl: the pull is the world size of (half line width + 1) pixels at the edge's depth (points 2 px),
     enough to beat FACE_DEPTH_BIAS on their own faces. Test `edges_behind_faces_stay_hidden_at_close_zoom`
     (offscreen.rs: red probe edges at each rib's hidden far corner, ortho + perspective, 3 zooms, 1x/2x);
     `NOSHAPE_TEST_OUT=<dir>` writes probe images and app-like black-edge images.
  3. **GPU instancing**: instances live in a storage buffer (`array<Inst>`, 96 B each instead of 256 B uniform
     slots) and each pass uploads its draw order (`slots`, skipped when unchanged); consecutive copies sharing a
     buffer are one instanced call (meshes/images: instance index = slot; lines/points: vertex index / 6).
     `r.draw_calls` perf stage. Test `shared_mesh_copies_keep_their_own_picks_and_states`; bench
     `instancing_bench` (ignored). Model side (new): `StudioDisplay::shared_meshes` (`SharedMesh`: unlocated mesh,
     3x4 location, the world mesh it stands for): regen/display.rs meshes each TShape (`Shape::partner_hash`, kern)
     once via `Shape::unlocated()` and derives each copy's world mesh with `MeshBuffers::transformed` (CPU users
     keep world meshes, untouched); scene builder uploads the shared mesh once and places each copy's instances
     by its transform (not when vertex colors / replaced meshes apply). Test crates/model/tests/shared_meshes.rs.
     Needs: GPUs with vertex-stage storage buffers (all of Metal/Vulkan/D3D12; not GL ES).
- **2026-10-21 08:45 (perf) Hidden edges, root cause and Onshape's exact scheme** (lead urgent; user screenshots
  /root/onshape-research/live/user-reports/2026-10-21-fins-*.png: segments/triangles at the fin roots, 1000 fins).
  Root cause in v0.4.4: (1) `vs_line` pulled every edge toward the camera by `z*1.0004 + 2e-5*w`, a pull
  proportional to the *scene's* depth range, not to local geometry: orthographic depth spans ±4·max(scene radius,
  distance), so with 1000 fins (~2 m) the pull is 1–3 mm, more than a 1 mm fin; perspective (near = distance·1e-3)
  2e-5 of depth is ~2% of the eye distance. Edges a fin-thickness behind a face won the depth test. (2) Faces were
  pushed back by 2 px of their own depth slope (FACE_DEPTH_BIAS slope -2), so at grazing fin sides edges behind
  them came through near the silhouettes. Not the cause (checked): depth compare (GreaterEqual, edges after
  opaque faces, depth write on, same MSAA pass), polygon offset sign (reverse-Z negative = away), reversed-Z
  precision (infinite reverse-Z float; ortho range ±4 R gives ~0.5 µm at 2 m), MSAA resolve.
  Onshape (webgl/woolsthorpe bundles): edge quads keep the vertex's exact projected depth (the shader only offsets
  x/y, `computeTangentAndNormalOffsets`); each node type gets `setZOffset(z)` = `polygonOffset(z / 29, z)`,
  MAXIMUM_Z_OFFSET 29, PART_FACES 27, PART_EDGES 15, PART_POINTS 9 (also SURFACE 25, SKETCH_EDGES 12, ...), on a
  DEPTH24_STENCIL8 standard-Z buffer whose near/far `updateNearFar` fits to the scene bounds (near = max(0.01 D,
  nearest corner - 0.1 D), far = max(D, farthest + 0.1 D); ortho eye 0.6 D before the center).
  Ours now: slope part as pipeline depth bias (faces -27/29, edges -15/29, points -9/29: format independent);
  constant part in the shaders (`in_front_of_faces`: edges 12, points 18 Onshape units), converted per frame by
  `onshape_depth_unit` (Globals.depth_unit) from Onshape's near/far to our reverse-Z depth. Faces keep -16 float
  units (~2e-6 relative) so coplanar overlays still win. The earlier "pixel pull" is gone.
  Test: crates/ops/tests/hidden_edges.rs (`edges_behind_fins_stay_hidden`): the user's fins through noshape-ops
  (block 100×2010×40, fin x 10..90, 1 mm, flush with the front, ×1000 at 2 mm), built as a *Part* pattern (1000 fin
  parts, also exercising instancing) because the Feature pattern is red in the tree while kern changes
  pattern.rs (`merged_into_body`; kern's own pattern_fins tests fail the same way). Magenta probes on each fin's
  hidden back root and far end edge must not show; cyan controls on edges nothing hides (top front edge, near-end
  root) must show (a fin's *front* root is hidden by the fin in front of it, so it is no control). Ortho +
  perspective, ±4/12/40 mm, 1x/2x. With v0.4.4's offsets it fails (up to 1747 px, merged-body variant), with the
  fix it passes. `NOSHAPE_TEST_FINS_DOC=<doc>` runs it on another document of the same fins (e.g. the merged comb
  body /root/perf-scratch/hidden/fins1000-comb.noshape). Note: the shaded orange part color matched a "red"
  filter, hence magenta/cyan.
  The "two faces on the front wall" in the split-face shot is kern's face-merge bug.
- **2026-10-21 10:40 (perf) Blank viewport after "GPU wait failed"** (lead HIGH, ui's doc "x": a 900-part studio drew one
  line, then a small Pin studio drew nothing). Not reproducible with the 10:00 tree on an idle-ish llvmpipe
  (/root/perf-scratch/parts/x.noshape, 900-pin Part pattern + Pin tab, both render). Cause in the code: `wait_oldest`
  treated wgpu's `PollError::Timeout` (10 s; the only other error is a wrong submission index, device loss does not
  come through here) as a failed frame: it popped the read, reported "the frame failed", decremented the target's
  in-flight count and dropped the readback buffer while the GPU was still rendering it. On a software rasterizer
  under load (ui's machine: load 30+, 900 parts + AO + pick pass) the next frames queued behind it on the GPU,
  timed out the same way, and no frame was ever delivered: a blank viewport, also for the small studio queued behind.
  Fix: a timeout leaves the read pending (one warning per frame, "GPU frame still running after 10s; waiting"); the
  worker loop / blocking `render` just wait again. `Renderer::set_wait_slice` sets the wait (tests).
  Tests: crates/ops/tests/many_parts.rs (900-pin Part pattern + a one-pin studio through noshape-ops, scene builder,
  ortho/perspective, whole + close, with the default wait and with a 1 µs wait slice = a GPU slower than every wait;
  asserts shaded, edge and picked pixels on every frame). Smoke (CI): `SMOKE many parts` builds a 900-pin studio in
  the app (model API), opens it, then the first other Part Studio; each must present 5 frames (orbiting) in 60 s with
  no failed frame (`Viewport::frame_counts`, `PartStudioView::viewport_entity`).
- **2026-10-21 14:10 (perf) Section caps landed (Onshape's endcap scheme; the shelved freeze rewrite).** Merged onto the
  current renderer (instancing, depth offsets) and checked line by line against the bundle's shaders:
  mask pass (each kept surface adds its sRGB part color + (id mod 31/71/97)·0.001, alpha + partCountOrder 16, front
  +, back −; Onshape offsets by uMeshGroupIdHash + uOccurrenceId, session ids we don't have, so we use the part's pick
  base), 3×3 edge detect + 5-tap blur, a quad per plane: median-of-3 sample, getColorHash / getHatchingSmoothstep /
  dark-part brightening, intersecting red rgba(220,0,0) + 0.1 alpha, 10% selector color (plane 0 black, 1 red, 2
  green), black outline. Pattern scale = SECTION_PLANE_ORTHOGRAPHIC_PATTERN_SCALE 30 / (P[5]·viewport height);
  perspective 10·(distanceToCenter + 0.5 D)/height with distanceToCenter = |dir·(bounds center − eye)| (fixed: was the
  camera distance). `ClipPlane::new` gives Onshape's frame (G44: a cut from Right +20: tangent (0,−1,0); from Top:
  (−1,0,0); v = n × u, as getSectionPlaneFrame's cross(N, −u) with N the kept-side normal = −ours). 32-bit float mask
  (FLOAT32_BLENDABLE requested, also through GPUI's device; 16-bit fallback turns the hatch by the rounding × 1000).
  Call sites: scene builder and assembly_scene.rs use `ClipPlane::new`. Test `section_cap_hatch_follows_onshape`
  (direction, 15·pace px period, 90% color, outline at 1x/2x; a ~49° color so mirroring or a 16-bit mask fails);
  the old section tests pass. Exact pixel parity with the G44 shots isn't possible (session-id offsets, and their
  mask color is not the REST appearance: the predictable shot's cap base is (225,206,185) for a (234,234,234) part).
- **2026-10-21 16:00 (perf) KB4 orbit rate (qa5): no rate deviation; view-cube drag added.** Onshape (webgl `onDrag`):
  angle per px = 2π·ROTATION_CYCLES_PER_SCREEN (3; 2 for a touch pointer) / |canvas size|, deltas and canvas both
  multiplied by the effective device pixel ratio (`onDrag(t, i·dpr, n·dpr)`), so per pointer px it is
  2π·3 / logical canvas diagonal on every platform: what `navigation::rotation_per_px` already does. Live 0.55°/px is
  the 1673×973 canvas of a 1919×1079 window (formula: 0.558°/px); our graphics area has Onshape's size in a window of
  the same size (1440×900: 1194×794 in both → 0.753°/px). qa5's 0.9°/px = a ~1200 px viewport diagonal (a smaller
  window), or the release inertia of a quick scripted drag (both apps: released < 50 ms after the last move, decays
  ×0.77 per 60 Hz frame). Pan (rays onto the view-normal plane through the pivot), wheel zoom (0.5/120 per
  wheelDelta, 2^(−i/6)), drag zoom (device px / DRAG_ZOOM_FACTOR 10) and inertia were already Onshape's.
  Test `navigation::drag_rotate_rate_matches_live` (100 px → 55.80° on 1673×973, 75.32° on 1194×794).
  **View cube:** Onshape doesn't rotate on a cube drag, it moves the cube (`updateDragPosition`, kept as an offset
  from the top-right corner, clamped 15 px left/right/top, 40 px bottom; ≤ 2 px moved = click; actions on click,
  not on press). Implemented in part_studio/view_cube.rs (shared with the Assembly; `view_cube(.., viewport, cx)`).
- **Overlays (item 2), started:** `overlay::Label` (box rects + GPUI text mask at a model point, the corner
  rounded like GPUI's layout from the window origin), `Viewport::set_overlay_labels`, app `overlay_labels.rs`
  (`value_box`, a GPUI value-box div replicated). Moved: section offset box (section.rs `section_value_label`),
  analysis "Min/Max/R" readouts (`analysis_labels`); `follows` no longer includes the section dialog / analysis tool
  (NOSHAPE_GPUI_LABELS=1 draws them the old way, for pixel comparison). TODO: pixel crops vs GPUI (needs an app
  build: the tree didn't compile at 15:00 in assembly files); then sketch dimension labels / constraint glyphs /
  shown dims as labels with viewport hit testing, the sketch name and datums (render-thread), and drop the
  `sketch.rs:509` CameraChanged notify.
- **Compare (item 3), renderer done, app wiring TODO:** `scene::compare` (instances tagged BASE/TARGET,
  `Scene::compare = Some(Compare { blend })`): per-version depth+normal prepass (Rgba32Float, Onshape's vDepth =
  (eye distance − near)/(far − near) with its near/far rule), faces tinted per pixel in `mesh_color`
  (COMPARE_SAME_LIT_TINT white / COMPARE_BASE_TINT (144,207,241) / COMPARE_TARGET_TINT (221,82,40); |Δdepth| < 0.0004,
  normals dot > 0.98), the "under" version alone in the frame (base below 0.5, else target), both versions in an
  over layer mixed by 2f / 2(1−f) (Onshape's underBasePass/underComparePass + compareBlendPass), AO on both layers.
  Test `compare_blend_follows_onshape`. App: the Part Studio must build the target version's scene (regenerate the
  target document's studio) tagged TARGET, its own tagged BASE, and pass `compare_mode::blend()`; edges stay
  normal (Onshape tints only nodes `isTintedByCompare`); planes/origin/sketches hidden; picking: base only.
- **2026-10-21 17:50 (perf, third account; took over at 14:50)** State:
  - Done: clippy fixes for the freeze (compare_graphics `Owner<V>`, needless `&up`, view_cube `+=`); **compare blend in
    gamma space** (Onshape's `fs-compareBlend`: over color with alpha uBlend = f<.5 ? 2f : 2(1−f), blended on a WebGL
    canvas that is not sRGB; ours drew into the sRGB view, i.e. mixed linear values: +7 in blue). The mix pass now draws
    through the frame's unorm view (`resolve_unorm` / present `view`, like the overlay) and the shader writes
    `linear_to_srgb(over)`. `compare_blend_follows_onshape` un-ignored, passes; offscreen 22/22.
  - Item 5 (Windows Cursors): run 39608471542 on perf-ci, 52/52 ok; results in docs/handoff/icons2.md (no channel to
    icons2 from this account), artifact /root/perf-scratch/dl/cursors-39608471542/.
  - KB4: bundle checked again (`onDrag`: 2π·ROTATION_CYCLES_PER_SCREEN 3 / |canvas width,height attrs|; inertia
    ROTATION_INERTIA .77 only if released < 50 ms after the last move AND the interactive frame rate is not below
    `REDUCTION_THRESHOLD_MS`). Ours: same formula; our document window opens at 1440×900 → 1194×794 graphics area →
    75.3°/100 px without inertia. qa5's 85° ≈ that + inertia from a fast xdotool release; live's 55.8° = its 1673×973
    canvas with no inertia: either released ≥ 50 ms after the last move, or below Onshape's 20 fps
    `DETAIL_REDUCTION_THRESHOLD_FPS` (inertia off); unverified until live answers the request. Measuring: viewport
    logs `nav: rotate drag N px, released still|moving|with inertia: D° (°/px); graphics area WxH (diagonal, formula)`
    under NOSHAPE_PERF (viewport.rs `rotate_log`). Live request filed (REQUESTS.md, exact protocol). TODO: Onshape's
    "no inertia below 20 fps interactive" (detailManager `isInteractiveFramerateBelowThreshold`).
  - Item 6: main has instancing + `NOSHAPE_PERF_SKIP=instancing` + perf.yml `fixture` input (no baseline unless default;
    variants on macOS and Linux). Fixture from `many_parts.rs::write_parts_1000_fixture` (ignored,
    NOSHAPE_FIXTURE_OUT) → tests/perf/parts-1000.noshape on a perf branch, then
    `gh workflow run perf.yml --ref <branch> -f ref=<branch> -f jobs=bench -f fixture=tests/perf/parts-1000.noshape
    -f variants='noinst:NOSHAPE_PERF_SKIP=instancing'`.
  - PERF1/PERF2 (lead 17:30, with kern): probe `crates/ops/tests/big_plate_perf.rs` (ignored, --nocapture): qa5's
    400-hole plate through the agent API; prints call times, a from-scratch regen split into features vs display, and
    the plate's tessellation alone. Display tessellation is serial over bodies (display.rs `build`), BRepMesh parallel
    per face only; RegenOptions relative_deflection 8e-4 of the diagonal, angular 0.35 rad (Onshape's tolerances TBD).
  - Next: items 2 (overlays) and 3 (compare app wiring: compare_graphics.rs is wired into part_studio/mod.rs
    `refresh_scene` and `render`; needs an app check in compare mode + crops vs live).
- **2026-10-21 19:40 (perf) Results + item 2 in progress.**
  - **Item 6 measured** (run 39646897129, perf-inst = 5d28447 + tests/perf/parts-1000.noshape; 1000 pins as a Part
    pattern, 184k triangles, 3007 instances): instancing works (`r.draw_calls` 9 vs 2007 with
    `NOSHAPE_PERF_SKIP=instancing`), but orbit fps is the same either way: macOS 14 runner texture orbit 15.5 fps
    (p95 74 ms; noinst 15.7), UI CPU 0.57 ms; lavapipe 2.3 fps, GPU 379 ms/frame (noinst 371). So this studio is GPU
    bound, not draw-call bound, and slower than the 670k-triangle NIST fixture (macOS 27.8 fps, lavapipe GPU 455 ms):
    the per-pixel/per-instance cost (edges and vertex points of 1000 parts, MSAA, AO) dominates. Local diagnosis:
    `many_parts.rs::thousand_pins_frame_time` (ignored; run per variant NOSHAPE_PERF_SKIP=lines|meshes|instancing,
    NOSHAPE_PERF_MSAA=1, NOSHAPE_TEST_AO=0). Fixture is on branch perf-inst only (not in the tree).
  - **PERF1/PERF2**: `big_plate_perf.rs` (in main): the 400-hole Remove's *feature evaluation* is 142.8 s (kern's
    boolean; live 491 ms); the plate's tessellation is 230 ms (59k triangles). 400 single sketch_draw calls took
    50 s (api: per-call re-solve). Display meshing is now **parallel over bodies** (display.rs `mesh_in_parallel`:
    rayon, one job per cache-missing body or pattern-partner group, bodies sharing a face/edge TShape meshed in
    one group because BRepMesh writes triangulations onto them; composites left to the serial loop). Test
    `shared_meshes.rs::parallel_meshing_keeps_every_body_whole` (6 cylinders + a split box whose halves share the
    cut face, 8 rounds). Onshape's display tolerances requested from live (REQUESTS.md).
  - **Rotation inertia off below 20 fps** like Onshape (navigation.rs `FrameTimer`: drawLoopDetails + frameDetails,
    5-sample averages, timing every 10th interactive frame with a forced redraw; interactive = 500 ms after a
    camera change). Test `frame_timer_follows_onshape`.
  - **Item 2 (overlays), first slice in the tree, NOT yet built/verified in the app:** renderer: `overlay::Quad`
    (GPUI `fs_quad` port: rounded, solid border, SDF AA), `overlay::Shadow` (GPUI `fs_shadow`), `Mask::density`
    (icons: GPUI's 2× SVG tile drawn on ceil(size/2) px), `Label::hit` + `overlay::label_at` (hit test with the
    frame camera, same corner rounding as the drawing); test `label_quads_and_shadows_follow_gpui`. App:
    `overlay_labels::row_box` (dimension-label div replica: h 22, px 4, items_center, rounded 4, ƒx/ƒ italic
    marker leaf), viewport `label_at` + `ViewportEvent::LabelHover/LabelDown`, sketch.rs
    `placed_dimension_labels` (shared by GPUI and frame paths) / `sketch_dimension_overlay` / `sketch_name_contours`
    (name as a WorldFill in the sketch plane), `ui.sketch_follows_camera` (only when GPUI still draws something at
    projected points: dimension box, glyphs, counts, inference, on-the-fly boxes); the CameraChanged notify now
    only on zoom (wpp). TODO: build, pixel crops vs `NOSHAPE_GPUI_LABELS=1` at 1x/2x (hover, selected,
    configured, ƒx), then constraint glyphs (icons via resvg 0.46 like GPUI's `render_alpha_mask`, shadow_xs =
    offset (0,1) blur 2 black 5%), pattern counts, datums.
- **2026-10-21 22:30 (perf, after the green-up freeze)**
  - Local llvmpipe, many_parts.rs::thousand_pins_frame_time (1194x794, 1000 pins, 184k tris; machine loaded):
    default 1149 ms; skip meshes 371; MSAA 1: 331; AO off 959; no instancing 1530 (instancing -25% here).
    The "lines" variant printed nothing (rerun). Meshes under 4x MSAA dominate on software rasterizers.
  - Onshape adapts interactive detail (webgl DetailManager): while the INTERACTIVE frame rate is below
    DETAIL_REDUCTION_THRESHOLD_FPS 20 it steps down: (1) edge silhouettes + section plane edges off, (2) SSAO off,
    (3) image-based (backface) silhouettes off, (4) lowLevelBodyProportion += LOW_LEVEL_BODY_PROPORTION_STEP .1 up to
    MAX_LOW_LEVEL_BODY_PROPORTION .9 (that share of bodies at the lowest LOD); above DETAIL_ENHANCEMENT_THRESHOLD_FPS
    30 it steps back up; settings persisted per [viewerUsage, workspace, element, modelUsage]. Bodies under
    DEFAULT_EXTENT_CULLING_THRESHOLD_IN_PIXELS 2 px are culled (occurrence LOD manager; read where it applies).
    Next perf item: implement this on top of navigation::FrameTimer (AO off first; LOD = coarser meshes).
  - Item 2 gate: frame sketch labels only with NOSHAPE_FRAME_SKETCH_LABELS=1 (overlay_labels::frame_sketch_labels)
    until crops vs GPUI are approved; the sketch name is in the frame unconditionally.
- **2026-10-22 02:00 (perf) Silhouettes (Onshape draws them; we drew none) + label crops.**
  - Edge-based silhouettes (Onshape `vs-draw-silhouettes`): scene builder `silhouette_edges` = every tessellation
    edge inside a face shared by two non-coplanar facets, with both facet normals (`SilhouetteSegment`, uploaded
    with the body, so pattern copies share it); renderer `GpuLines::silhouette` → its own list and pipeline
    (`vs_silhouette`: drawn where one facet faces the viewer and the other doesn't, otherwise like an edge via the
    shared `line_vertex`; face hover/selection masked; never in the pick pass). Image-based backface silhouettes
    (`vs-backfacesil-faces` port: back faces pushed out by half the edge width on screen × (1 − |n·v|), edge color;
    `Instance::back_silhouette`, `mesh_backsil`, drawn before the opaque faces) for opaque solids in Shaded with
    edges / Hidden line. Not in plain Shaded; wires/surfaces no backface pass. Test
    crates/scene/tests/silhouettes.rs (cylinder from Front, ortho + perspective: sides much darker than in Shaded,
    the face unchanged). Onshape's exact line width/color for silhouettes requested from live (REQUESTS.md);
    the shader source is at /root/perf-scratch/onshape-silhouette-shaders.txt.
  - Frame sketch labels vs GPUI (crops2/, fixture crops2/labels.noshape via mklabels.py + noshape-cli): 1x rest and
    hover ≤1 level (11 / 19 px); 2x: the value after the ƒx/ƒ marker 1 device px left → GPUI's text leaf width is
    `round(ceil(width_logical) · scale)` (text.rs measure ceils in logical px, taffy rounds to device px), fixed in
    `overlay_labels::leaf_width` (also the section/analysis value boxes). Re-crop pending a build.
- **2026-10-22 05:30 (perf, after the 04:10 freeze)**
  - Local llvmpipe, many_parts.rs::thousand_pins_frame_time (1194x794, 1000 pins, 184k tris; machine loaded):
    default 1149 ms; skip meshes 371; MSAA 1: 331; AO off 959; no instancing 1530 (instancing -25% here).
    The "lines" variant printed nothing (rerun). Meshes under 4x MSAA dominate on software rasterizers.
  - Onshape adapts interactive detail (webgl DetailManager): while the INTERACTIVE frame rate is below
    DETAIL_REDUCTION_THRESHOLD_FPS 20 it steps down: (1) edge silhouettes + section plane edges off, (2) SSAO off,
    (3) image-based (backface) silhouettes off, (4) lowLevelBodyProportion += LOW_LEVEL_BODY_PROPORTION_STEP .1 up to
    MAX_LOW_LEVEL_BODY_PROPORTION .9 (that share of bodies at the lowest LOD); above DETAIL_ENHANCEMENT_THRESHOLD_FPS
    30 it steps back up; settings persisted per [viewerUsage, workspace, element, modelUsage]. Bodies under
    DEFAULT_EXTENT_CULLING_THRESHOLD_IN_PIXELS 2 px are culled (occurrence LOD manager; read where it applies).
    Next perf item: implement this on top of navigation::FrameTimer (AO off first; LOD = coarser meshes).
  - Item 2 gate: frame sketch labels only with NOSHAPE_FRAME_SKETCH_LABELS=1 (overlay_labels::frame_sketch_labels)
    until crops vs GPUI are approved; the sketch name is in the frame unconditionally.
  
  - Live 459 (behaviours.md): Onshape's display mesh is angle based: segments per full circle Coarse 24, Medium 48,
    Fine 144, Very fine 600, Auto = Fine (144) at Ø20/Ø200/Ø2000; REST default 72. Plan: regen/display.rs
    tessellate_body/tessellate_shape → angular deflection 2π/segments (Auto/Fine 0.0436 rad, Medium 0.1309, Coarse
    0.2618, Very fine 0.01047), linear deflection out of the way (large, e.g. the body diagonal) so the angle rules;
    preview meshes: Coarse; partstudio.rs TessQuality: segments() instead of factor(); per-part UI exists
    ("Edit appearance…" Tessellation quality ▾, check it is wired); test: a Ø20 and Ø2000 cylinder both have 144
    segments on their circular edges (count distinct vertices on the top circle); measure big-plate (230 ms before)
    and 1000-pin regen + orbit after.
  - Live silhouettes answer: outlines = edge colour/width (1 px ≈ #343434), none in Shaded, same in Hidden line, front
    part's outline over the rear part, rear outlines hidden: matches ours (edge colour/width, depth tested).
  - Offscreen suite SIGSEGV'd once (02:5x, machine loaded), passed on rerun: watch for flakiness (llvmpipe).

## From ui2 (10-22): Preferences › Environment profile › "Match pixel density on high resolution displays"
- Stored per profile (`ListPrefs.env_profiles[..].density`: Automatic (default) / On / Off, the selected one in
  `env_profile`; `prefs_general::PixelDensity`). Live: picking a profile applies it immediately. Not applied to
  the viewport yet: Off would render at 1 px per logical px on HiDPI (scale 1.0 instead of `window.scale_factor()`
  in viewport.rs, picks mapped accordingly), On/Automatic = device pixels. Yours to wire (a cached atomic, not a
  file read per frame) — tell ui2 and I'll expose a reader.
- **2026-10-22 09:55 (perf): cost of the angle-based display mesh** (Auto = 144 segments per circle, live 459), same loaded
  machine (load 13–18), llvmpipe, old = aa75fdf display.rs (chord 8e-4 × diagonal, 0.35 rad):
  - Big plate (400 holes): 59 212 → 232 012 triangles (×3.9); meshing the part 351 → 533 ms; regen from scratch 3.0 → 3.8 s.
  - 1000 pins (5000 instances): 184 000 → 572 000 triangles (×3.1); frame mean 1612 → 9282 ms, worst 2035 → 16 980 ms
    (software rendering, triangle bound; on a GPU 572k triangles is cheap). Onshape pays the same triangle count and
    relies on interactive detail reduction (lowest-LOD boxes for up to 90 % of bodies, ordered by coarse triangles /
    radius², `performAggressiveCulling`): next item.
  - Fix d8d0955: the chord tolerance scales with quality too (BRepMesh keeps an existing coarser triangulation
    otherwise; "Use best available tessellation" = Very fine, 600 segments).
- **2026-10-22 16:50 (perf): offscreen suite SIGSEGV, cause and fix.** The intermittent crash (02:50, 16:20; ~1 run in 8
  under load) is in the Vulkan loader (`libvulkan.so.1`), in wgpu's `set_object_name` while one test thread builds a
  pipeline and others are in `vkCreateInstance`. Not WGPU_DEBUG (1/8 with `WGPU_DEBUG=0` too); 0/10 with
  `--test-threads=1`. `crates/render/tests/offscreen.rs` now holds a `GPU` mutex per test (one test on the GPU at a
  time, ~20 s instead of ~6 s). Other test binaries that create several renderers on parallel threads can hit the same.
- **Lowest level of detail (a1af5f5):** `renderer.rs` `lod_boxes` = webgl BodyManager `performAggressiveCulling`:
  boxes (`Instance::lod_box`, the body's bounds in its color, `getDisplayIncrement`) for bodies under
  `extent_culling_px` (2 px still, 10 px interactive: DEFAULT/AGGRESSIVE_EXTENT_CULLING_THRESHOLD_IN_PIXELS) and the
  interactive `low_level_body_proportion` share ordered by triangles / r² (`getBenefitToCull`; Onshape uses the
  server's coarse triangle count, we the displayed mesh's), only under 1% of the viewport diagonal
  (MAXIMUM_BODY_CULL_DIAMETER_PROPORTION; 7.5% with the "more aggressive culling" capability, not modelled); bodies
  outside the view count against the share. Pick and depth-sample passes draw full detail. Not done:
  FULL_DETAIL_OCCURRENCE_REDUCTION_THRESHOLD (over 5000 occurrences, full-detail frames drop silhouettes/SSAO and use
  the 10 px threshold). At the 1000-pin fit view each pin is ~50 px (> 1% ≈ 14 px), so Onshape boxes none there either.
- **2026-10-22 18:40 (perf): macOS CI, chord vs angle display mesh** (perf.yml on perf-dev 56a9530, `variants`
  chord:NOSHAPE_DISPLAY_MESH=chord; runs 39824106798 parts-1000, 39824123076 big-plate; Apple Paravirtual Metal).
  Presented fps is capped near 20 by the runner (frame interval ~50 ms while the GPU finishes in ~12 ms), and these
  builds report no GPU timestamps, so the GPU cost is `w.gpu_wait` (orbit, mean / p95):

  | case | chord (old) | angle (Onshape, 144/circle) | mean |
  |---|---|---|---|
  | 1000 pins, macOS 14 | 14.2 / 21.9 ms | 19.8 / 27.2 ms | +39% |
  | 1000 pins, macOS 15 | 12.6 / 18.9 ms | 20.6 / 39.6 ms | +64% |
  | big plate, macOS 14 | 12.1 / 17.5 ms | 11.8 / 25.1 ms | flat, p95 +43% |
  | big plate, macOS 15 | 12.0 / 17.5 ms | 10.3 / 21.7 ms | flat, p95 +24% |

  UI-thread CPU per frame < 1 ms in all. The lowest-LOD boxes don't apply at these views (r.lod_boxes 0: pins ~50 px,
  over 1% of the diagonal). Gating rule applied: `regen::angle_display_mesh()` is off by default (chord mesh),
  `NOSHAPE_DISPLAY_MESH=angle` or `set_angle_display_mesh(true)` turns it on. To lift the gate: cut the per-triangle
  GPU cost (each body's mesh is drawn for color, backface silhouettes and depth samples), then rerun both fixtures with
  `variants` = `angle:NOSHAPE_DISPLAY_MESH=angle`.
- Kernel side (lead, 18:00): kern is writing a fast planar-face triangulator in the kernel's mesh.cpp (the big plate's
  two planar faces: 1.65 s of its 2.7 s display mesh in BRepMesh). Display density, LOD and upload stay perf's.
  Rerun the big-plate fixture on CI when it lands.
- **2026-10-22 21:00 (perf): where angle mode's GPU time goes** (perf.yml run 39844027399, parts-1000, macOS, every
  variant `NOSHAPE_PERF_DETAIL=full`; `w.gpu_wait` mean, macOS 14 / 15): chord 15.8 / 12.0 ms; angle 21.2 / 19.5;
  angle without backface silhouettes 21.6 / 19.2; without edge silhouettes 21.1 / 19.7; without AO 21.5 / 18.8;
  without meshes 20.5 / 18.9; **without lines (edges, silhouettes, backface silhouettes) 12.6 / 9.7**. Redrawing
  the meshes is not the cost: the B-rep edge lines are (each segment a 6-vertex quad; 144-segment circles have ~4x
  the segments of the chord mesh, sub-pixel at the 1000-pin fit view). Fix in progress: edge-line LOD (Douglas–Peucker
  levels relative to the body's size, chosen per body per frame under 0.25 px error; pick pass full).
