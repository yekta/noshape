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
| `noshape-kernel` | crates/kernel | (C++ OCCT) | Safe Rust wrapper over a `cxx` bridge to OCCT: shapes, topology exploration, primitive/feature operations, booleans, fillets, offsets, sweeps, lofts, tessellation, HLR projection, STEP/IGES/STL/OBJ/glTF I/O, mass properties, measurement. Knows nothing about features. |
| `noshape-model` | crates/model | units, sketch, kernel | The CAD document model: Part Studio feature list & regeneration engine, persistent topological naming, feature definitions (extrude, revolve, ...), variables, configurations, assemblies & mate solver, drawings model. Deterministic, headless, fully testable. |
| `noshape-store` | crates/store | – | Local persistence: single-file document (`.noshape`, SQLite) with content-addressed snapshots, microversion history (undo/redo across sessions), named versions, branches, merges. Generic over opaque blobs. |
| `noshape-render` | crates/render | – | wgpu renderer: GPU meshes for faces/edges/vertices, sketch overlay, grid, triad, picking via an ID buffer, highlight/selection states, section planes, offscreen rendering with BGRA readback for GPUI. Its own scene types; no OCCT. |
| `noshape` | crates/app | all | GPUI application: document browser, tabs, feature list, toolbars, feature dialogs, sketch tools, selection, mouse/keyboard interaction, viewport element. |

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
GPUI does not expose custom GPU passes on all platforms, so `noshape-render`
owns its own wgpu device and renders offscreen into a BGRA8 texture which is
read back and displayed as a GPUI `RenderImage` element (zero-copy surfaces
can be added later on macOS). Rendering happens only when the scene, camera or
hover state changes. Picking uses an R32Uint ID buffer read back at the cursor.

### Units and expressions
Every numeric field accepts an expression (`25 mm`, `#width / 2`,
`2 * #t + 1 in`, `sin(30 deg) * 10`). Values are stored as the expression
string plus its evaluated SI value; the document has default units.

### Performance rules
* Release builds use thin LTO; dev profile optimizes dependencies.
* Tessellation runs in parallel (rayon / OCCT parallel meshing) and is cached
  per shape; meshes are uploaded once and reused.
* No allocation per frame in the render loop; scene diffs only.
* The UI thread never blocks on the kernel.
