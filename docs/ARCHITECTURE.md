# noshape architecture

noshape is a fully local, native, parametric feature-based CAD application that
works like Onshape (documents with Part Studios, Assemblies and Drawings tabs, a
history-based feature list, sketches with a constraint solver, mates, version
graph with branches) but runs entirely on the user's machine. Rust + GPUI (via
GPUI Kit / `gpui-kit`) for UI, wgpu for the 3D viewport, OpenCascade (OCCT 8)
as the B-rep kernel.

## Crates

| crate | path | depends on | responsibility |
|---|---|---|---|
| `noshape-units` | crates/units | – | Lengths/angles/unitless quantities, unit parsing (`mm`, `in`, `deg`...), expression language (`#var * 2 + 3 mm`, functions, conditionals), variable scopes. Pure Rust. |
| `noshape-sketch` | crates/sketch | – | 2D sketch entities, constraints, dimensions and the geometric constraint solver (Newton/LM, DOF analysis, drag solving), region (profile) detection, trim/extend/offset/fillet helpers. Pure Rust. |
| `noshape-kernel` | crates/kernel | (C++ OCCT) | Safe Rust wrapper over a `cxx` bridge to OCCT: shapes, topology exploration, primitive/feature operations, booleans, fillets, offsets, sweeps, lofts, tessellation, HLR projection, STEP/IGES/BREP I/O plus STL/OBJ mesh import and export, mass properties, measurement. Knows nothing about features. |
| `noshape-model` | crates/model | units, sketch, kernel | The CAD document model: Part Studio feature list & regeneration engine, persistent topological naming, feature definitions (extrude, revolve, ...), variables, configurations, assemblies & mate solver, drawings model. Deterministic, headless, fully testable. |
| `noshape-featurescript` | crates/featurescript | – | FeatureScript: lexer, parser, compiler, evaluator, core builtins, and the embedded Onshape FeatureScript Standard Library (MIT). The model crate registers the modeling builtins (`custom/host`). |
| `noshape-store` | crates/store | – | Local persistence: single-file document (`.noshape`, SQLite) with content-addressed snapshots, microversion history (undo/redo across sessions), named versions, branches, merges. Generic over opaque blobs. |
| `noshape-render` | crates/render | – | wgpu renderer: GPU meshes for faces/edges/vertices, sketch overlay, grid, triad, picking via an ID buffer, highlight/selection states, section planes, offscreen rendering with pipelined BGRA readback for GPUI, frame-time instrumentation (`perf`). Its own scene types; no OCCT. |
| `noshape` | crates/app | all | GPUI application: start window (document library), document windows with tabs, feature list, toolbars, feature dialogs, sketch tools, selection, mouse/keyboard interaction, viewport element. |

Dependency direction is strictly downward. `model` never depends on GPUI or wgpu.
`render` never depends on `model`; the app converts model output into render scenes.

## Key design decisions

### Fully local
No server, account or network. A document is one `.noshape` file (SQLite)
that contains every tab, its complete edit history, versions and branches.

### Parametric regeneration
A Part Studio is an ordered list of features. Each feature is pure data
(`FeatureDef`, serde-serializable) evaluated against the context produced by
the previous features. Regeneration:
* is incremental: per-feature output contexts are cached and keyed by a hash of
  (feature definition, upstream hash, configuration values); editing feature *k*
  re-evaluates from *k* onward;
* runs on a background thread; the UI keeps showing the last good result;
* never aborts on a failed feature: the feature is marked failed (red in the
  list, with the error message) and downstream features evaluate against the
  last successful context, exactly like Onshape.
* the rollback bar evaluates only up to the bar's position.

### Persistent topological naming
Features reference geometry (faces/edges/vertices) through *queries* rather
than indices. Every B-rep entity created by a feature receives a stable
`TopoName` derived from what created it, e.g.
`Extrude1.side(Sketch1.line3)`, `Extrude1.cap(end)`, `Fillet1.blend(<edge name>)`.
OCCT's `Generated/Modified/IsDeleted` history (from `BRepBuilderAPI_MakeShape`,
`BRepAlgoAPI_*`, fillet/chamfer/offset builders) is used to carry names through
each operation; split entities get a disambiguating ordinal ordered by a
deterministic geometric key. A query resolves names against the current
topology at regeneration time; unresolved queries produce a feature error
("missing selection"), never a crash.

### Sketch solver
Our own solver in `noshape-sketch` (no external dependency): variables are
point coordinates, radii, etc.; every constraint contributes residual
equations; solved with damped Newton (LM) using a dense or sparse Jacobian.
DOF analysis via rank-revealing QR of the Jacobian gives per-entity
fully/under-constrained state (Onshape: black vs blue) and detects
over-constrained/conflicting constraints (red). Dragging adds weak
"stay near cursor" terms so the rest of the sketch moves minimally.

### Viewport
GPUI does not let applications draw into its own GPU passes, so
`noshape-render` owns a wgpu device and renders offscreen. One render thread
(`render/src/worker.rs`) serves every viewport ("target"):

* **Frames.** The viewport submits a `RenderJob` when its scene, camera or
  size changes (`Viewport::request_frame`: submitted at the end of the current
  update, coalescing several changes, and only while the viewport is on
  screen). Waiting jobs are coalesced per target. Each target owns its render
  textures and up to two readback buffers: on real GPUs two frames may be in
  flight, so frame N+1 renders while frame N is mapped and copied out; on CPU
  rasterizers (llvmpipe, WARP) one, because a second frame only competes with
  the UI thread for the same cores (`NOSHAPE_FRAMES_IN_FLIGHT` overrides).
  The viewport keeps the same number of frames in flight and submits the
  next one when a frame is presented. Camera moves and scene changes do not
  invalidate the GPUI window (nothing visible changes until the frame
  arrives), so during navigation GPUI re-renders the UI once per presented
  frame instead of on every mouse event. `Scene::same_as` drops refreshes that
  would render an identical image (same buffers, instances and states).
* **Presentation.** The resolved `Bgra8UnormSrgb` frame is copied out of the
  mapped buffer once (row padding removed) and wrapped in a GPUI
  `RenderImage` without any channel swizzle (GPUI's atlas is BGRA). GPUI then
  uploads it into its sprite atlas (one more CPU copy and a texture upload
  inside GPUI; the old image is dropped from the atlas when the next arrives).
* **Picking.** The R32Uint pick-id buffer is rendered lazily: a frame only
  marks it stale (geometry, camera or size changed) and the pick pass runs
  when a pick query arrives, for the scene and camera of the newest frame.
  Queries read back small rectangles (`PickRect`): the hover query reads 81x81
  device pixels around the cursor and is cached, so small moves, clicks and
  context menus are answered on the UI thread without a GPU round trip; box
  selection reads the box (crossing) or the whole frame (window). After the
  camera settles (60 ms without changes) the hover is refreshed with one
  query; nothing pick-related is rendered or read back during orbit, pan,
  zoom or camera animations.
* **Culling and batching.** Instances whose bounds are outside the view
  frustum are skipped (conservative clip-space test; margins for wide lines
  and points). Geometry is uploaded once per `Arc<MeshBuffers>` (`GpuCache`)
  and interleaved straight into mapped buffers; all regions of a sketch share
  one pick mesh (one draw call instead of one per region). Draw calls scale
  with bodies (3 per body), not with features or faces.

**Zero-copy presentation (investigated, not implemented).** GPUI (gpui-pre
0.3.9, used by gpui-kit 0.7.3) can show a platform surface only on macOS:
`surface()` / `Window::paint_surface` take a `CVPixelBuffer`. The Metal
renderer (`gpui-pre-apple/src/metal_renderer.rs`, `draw_surfaces`) asserts that
the buffer is `kCVPixelFormatType_420YpCbCr8BiPlanarFullRange`, i.e. video
YCbCr with chroma at half resolution, and converts it to RGB in its shader.
Rendering wgpu frames into an IOSurface-backed CVPixelBuffer is possible
(`wgpu::hal::metal::Device::texture_from_raw` on an `MTLTexture` made with
`newTextureWithDescriptor:iosurface:plane:`, plus a conversion pass writing
the Y and CbCr planes), but 4:2:0 chroma subsampling and the 8-bit YCbCr
round trip visibly smear thin colored edges and highlights, so the image
would not match the readback path. The Linux (wgpu) and Windows (DirectX 11)
GPUI renderers ignore `PaintSurface` entirely, and GPUI does not expose its
wgpu device or atlas textures (on Linux it is the same wgpu 29, so a GPU copy
into the atlas would otherwise be possible). A real zero-copy path needs a GPUI
change: a BGRA branch in the macOS `draw_surfaces` (single plane, `BGRA8Unorm`
texture from the CoreVideo texture cache, pass-through fragment shader), or a
public way to register an external `wgpu::Texture` for an element on
Linux/Windows. Until then the readback path is used on every platform.

### Units and expressions
Every numeric field accepts an expression (`25 mm`, `#width / 2`,
`2 * #t + 1 in`, `sin(30 deg) * 10`). Values are stored as the expression
string plus its evaluated SI value; the document has default units.

### Performance rules
* Release builds use thin LTO; dev profile optimizes dependencies.
* Tessellation uses OCCT's parallel BRepMesh and is cached per shape hash
  across regenerations; meshes are uploaded once and reused. BRepMesh skips
  faces whose stored triangulation is already fine enough (faces an operation
  carried over unchanged from its input).
* No allocation per frame in the render loop (draw lists, instance and state
  uploads reuse buffers and skip unchanged data); scene diffs only.
* The UI thread never blocks on the kernel or the GPU: device creation and
  pipeline compilation run on background threads at startup.

### Measuring (`NOSHAPE_PERF=1`)
`noshape_render::perf` collects stage timings and logs mean/p50/p95/max every
120 samples with log target `perf`:

| stage | meaning |
|---|---|
| `r.prep`, `r.encode_submit` | render thread: uploads, draw lists, encoding |
| `w.gpu_wait` | render thread blocked on the GPU (rendering + transfer) |
| `r.readback_copy`, `r.pick_submit` | copy-out of a frame; lazy pick pass + region copy |
| `g.submit_to_frame` | UI submit until the frame is read back |
| `v.frame_to_present`, `v.present_to_paint` | until the UI thread takes the frame; until GPUI lays it out |
| `v.submit_to_present`, `v.pick_latency` | end-to-end frame and pick-query latency |
| `s.build`, `s.with_highlights`, `ps.apply_display`, `ps.refresh_scene` | scene building on the UI thread |

Regenerations log per-feature times and triangle counts (`regen[...]`), meshing
logs `tessellate ...` per body, and startup milestones are logged ("GPU
initialized", "first viewport frame presented"). `NOSHAPE_PERF_PERIOD=<s>`
flushes all stages periodically. Benchmarks: `NOSHAPE_PERF_ORBIT=<frames>`
(with `NOSHAPE_PERF_ORBIT_DELAY=<s>`, default 10) rotates the camera every 8 ms
like a 125 Hz mouse drag and logs the presented fps; with
`NOSHAPE_PERF_BENCH=zoom` it scroll-zooms instead. GPU cost analysis:
`NOSHAPE_PERF_MSAA=<n>` and `NOSHAPE_PERF_SKIP=lines,meshes`.

## Application structure (crates/app)

* **Windows.** `start_window.rs` is the document library: recent documents,
  folders, thumbnails rendered by `start_thumbnail.rs`. Each open document is
  a `Workspace` window (`workspace.rs`) holding a `DocModel` entity
  (`doc_model.rs`, wrapping the store `Session`) and one view per tab:
  * `part_studio/` for Part Studios;
  * `assembly_view.rs` for Assemblies;
  * `drawing_view.rs` for Drawings;
  * `variable_studio.rs` for Variable Studios;
  * `feature_studio.rs` for Feature Studios.

  Every edit goes through `DocModel::edit_studio`/`edit_tab` and becomes one
  microversion.
* **Feature dialogs.** `feature_dialog.rs` renders any feature from its
  `ParamSpec` schema. Built-in features have static schemas. Custom features
  (`crates/model/src/custom`, FeatureScript in Feature Studios, run by the
  `noshape-featurescript` crate with Onshape's standard library) register schemas
  at runtime under `custom:<tab>:<name>`. Dialog edits regenerate a preview
  rolled to the edited feature (or the finished model with "Final").
* **Part Studio extras.**
  * `part_studio/manipulators.rs`: drag handles that write back to dialog
    fields (extrude depth arrow, revolve angle arc).
  * `feature_ops.rs`: copy/paste of features through the system clipboard as
    `{"noshape_features": [...]}` JSON. Ids are remapped, so paste works across
    documents. Also feature folders.
  * `fab_panel.rs`: the sheet metal flat pattern and bend table, and the frame
    cut list.
  * `config_panel.rs`: variables and configurations.
  * `view_tools.rs`: tool search, display modes, section view, named views.
* **History.** `history_panel.rs` shows the version graph. It supports
  restore, branch, merge, read-only viewing, and "compare with current", backed
  by `noshape_model::compare`.
* **Derived parts.** `derived.rs` provides other Part Studios' regenerated
  contexts to Derived features, with caching and cycle detection.

