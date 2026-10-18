# Handoff: sketch behaviour parity (Onshape sketcher)

Owner area: `crates/app/src/sketch_mode.rs` (editor logic, overlay, headless tests),
`crates/app/src/part_studio/sketch.rs` (GPUI integration: input routing, toolbar, labels,
glyphs, value boxes, panels, context menus), `crates/sketch` (solver, inference, tools).
Also touched (sketch-specific, surgical): `crates/model/src/regen/features/sketch.rs`
(external references, configured dimensions, text expressions, rho units) and
`crates/model/src/sketch_feature.rs` (new `SketchData` fields: `dimension_tolerances`,
`patterns`, `images`; `SketchText.expression`; image helpers; base64).

The checklist with statuses is `docs/parity/sketch.md`. **Keep it current** (it is up to
date as of the latest handoff update: 187 ✅, 9 ⏳, 4 🔧).

## State right now

* The tree compiles (`cargo check -p noshape` clean as of this writing).
* `cargo test -p noshape-sketch` passes; app editor tests
  `cargo test -p noshape --bin noshape sketch_mode` pass (24 tests, more in round 2 not yet
  rerun after the latest edits: rerun them).
* Nothing is half-edited. Latest additions: text "Convert to expression" toggle, linear
  pattern instances linked to the seed (`ConstraintKind::PatternLink` + spacing dimension),
  Esc never cancels the sketch (capture of `Cancel` on the root), the Sketch dialog options
  panel (Show constraints / expressions / errors + diagnostics tools) under the dialog, user
  sketch shortcuts via `cx.intercept_keystrokes` (`sketch-shortcuts.json`).
* Next: compare against the live Onshape capture in `/root/onshape-research/live/`
  (`behaviours.md`, `dom/dropdowns-sketch.json`, `sketch/`), fix differences.

## Done since the checklist was updated (update these rows in docs/parity/sketch.md)

Implemented and compiling, mostly covered by crate tests; GUI not yet re-verified:

* 4.18 Conic rho dimension: `ConstraintKind::Rho` (sets the conic's rho directly; unitless),
  rho box opens right after the third click; smart dimension on a conic gives rho.
* 4.21 Polygon: second click locks the circumference, moving toward/away sets sides
  (15 px per side), digits type it, click or Enter creates (`finish_polygon`).
* 4.25 Spline tangent handles: `Geometry::FitSpline { points, handles }`,
  `Nurbs::interpolate_with_ends`, `add_fit_spline_with_handles` (used by the Spline tool for
  open splines), solver equations use handle direction / handled curve; white handles drawn
  in the overlay; handles can't be deleted.
* 4.28 Bezier degree: `bezier_degree` / `set_bezier_degree` (exact elevation, LSQ
  reduction, max 15); "Degree N" label, double-click to edit.
* 4.33 Text expressions: model side only (`eval_text_expression` in regen, box ratio
  follows evaluated text). TODO: UI toggle in the text edit rows (set
  `SketchText.expression` via `edit_text`).
* 4.35 Chained slots: `Sketch::slot_chains` (offset both sides + end caps), used by the
  Slot tool; one shared width dimension per session.
* 4.38 Insert DXF: `crates/sketch/src/dxf.rs` parser (LINE, CIRCLE, ARC, POINT,
  LWPOLYLINE/POLYLINE with bulges, ELLIPSE, SPLINE, $INSUNITS), `insert_drawing`
  joins ends; toolbar Insert group; units dropdown, "Use file origin position", file picker
  (`files::request_open_path`). DWG: message "save as DXF" (no DWG reader).
* 4.38 Insert image: `SketchData::add_image` (text-box style rectangle with locked aspect,
  horizontal bottom), drawn as a GPUI `img` in the overlay when the box is upright on screen
  (outline only otherwise), drag/select like text, pruned with its box.
  Image crate features jpeg/gif/bmp added in `crates/app/Cargo.toml`.
* 4.17 / 6.16 Ellipse quadrant inference + `ConstraintKind::Quadrant`
  (`EqKind::OnEllipseAxis`), elliptical-arc end snaps to construction-ellipse axis points.
* 6.14 / 7.13 / 8.8 / 8.11 / 11.19 / 11.21 External references: `external_entity()` in
  sketch.rs creates linked external (construction for dims/constraints) entities for model
  vertices, edges (exact or sampled projection), other sketches' entities, plane traces
  (datum/feature planes, planar faces), silhouettes of cylinders/spheres/tori/cones, and
  pierce points. Regen re-projects them (`plane_trace`, `update_trace`, `face_silhouettes`,
  `project_edge_any`, `pierce_point` in model `regen/features/sketch.rs`). Dimension,
  constraint and Mirror tools accept model geometry; Use handles faces (curved: silhouettes,
  planar: edges); edge snaps while drawing link a coincident.
* 7.24 / 12.9 Tolerance + precision panel ("±" next to the value box, also from the
  dimension menu); display texts with tolerance (`dim_value_text_tol`). Configure dimension:
  dimension menu "Configure dimension with <input>" / "Unconfigure dimension"; regen applies
  `sketchDim:<cid>` params (`CONFIGURED_DIMENSION_PREFIX`); editing a configured dimension
  writes the active configuration's value; configured labels have a gold border.
  Add comment: no comment system exists in noshape (document feature, out of scope).
* 7.25 Min / max distance to circles: picking a circle on its rim gives minimum or maximum
  (`circle_mode`), drawn to the rims.
* 8.18 Constraint manager (filters type/mode/status, auto-select, sort, delete, remove,
  clear all, delete all) and Profile inspector (`Sketch::profile_issues`: unclosed loops,
  overlapping lines; prev/next; live) from the toolbar diagnostics dropdown.
* 9.7 Select other (`) in a sketch: popup of entities under the cursor.
* 11.4 Fillet: preview at the hovered corner (red if too large), radius manipulator handle
  on the session's first fillet.
* 11.12 Offset: pick curves -> preview + direction arrow (drag = distance, click = flip) +
  distance box (negative flips), Enter / click in space commits, later clicks repeat.
* 11.15 / 11.17 / 11.18 Patterns and Transform: click in empty space or Enter accepts, Esc
  cancels, no Apply button; patterns recorded in `SketchData.patterns`, pattern glyph
  (hover / Show constraints) with right-click "Edit pattern" (re-creates instances).
  Instances are not constrained to the seed (Onshape links them): open item.

## Remaining items (priority order)

1. Diff against the live Onshape capture (`/root/onshape-research/live/`).
2. The ⏳ rows of the checklist (implemented, GUI not re-verified): 6.14, 8.8, 8.16, 9.5,
   9.7, 11.10, 11.20, 12.5, 13.3.
3. Circular pattern instances are not linked to the seed (needs a rotation pattern
   constraint: nonlinear, center/angle unknowns).
4. 1.9 Restore bubble after cancelling a sketch (feature dialog / mod.rs, other agent).
5. 3.13 a preferences page for sketch shortcuts (file-based works).
6. 4.38 DWG reader (DXF only); images are drawn only when the box is upright on screen.

## How to test

* Sketch crate: `CARGO_TARGET_DIR=/root/target-skb cargo test -p noshape-sketch`
  (`crates/sketch/tests/onshape_ux.rs` has the behaviour tests; `inference.rs` and `dxf.rs`
  have unit tests).
* Model: `cargo test -p noshape-model --lib text_expression image_tests`.
* App editor (headless, no GPU): `OCCT_ROOT=/root/occt CARGO_TARGET_DIR=/root/target-skb
  cargo test -p noshape --bin noshape sketch_mode`. Helpers `click`, `drag`, `editor(tool)`
  drive `SketchEditor::tool_down/tool_drag/tool_up/pointer_move` exactly like the view.
* GUI: `tools/sketch-ui/` (copies of `/root/skb-scripts`). Start your own Xvfb
  (`Xvfb :106 -screen 0 1600x1000x24 &`), then `tools/sketch-ui/fresh.sh` (starts the app on
  an empty doc, maximizes, opens a sketch on Top, presses `n`), then `xdotool` clicks;
  `shot.sh out.png` screenshots. Scripts `t1.sh`..`t9.sh` are the scenario scripts used so
  far (line chains, rectangles with typed sizes, dimensions, context menu, slots, trim drag,
  polygons, copy/paste, glyph menus). Run with `NOSHAPE_NO_NATIVE_DIALOGS=1`.
* Never run `pkill` with a pattern that appears in your own command line: the scripts use
  `pkill -f "[t]arget-skb/debug/noshape"`; calling that from a bash command whose text
  contains the same string kills the shell (exit 144). Run the script file instead.

## Gotchas

* llvmpipe rendering lags: wait 1.5-2 s before screenshots; the GPU hover pick lags behind
  fast xdotool moves (the view now falls back to a geometric pick on click, and ignores a
  stale hover far from the click).
* The viewport sometimes shows a stale frame after an edit until the mouse moves (renderer).
* Other agents' test modules with `use super::*;` plus `use gpui_kit::*;` break compiling the
  app's tests ("recursion limit reached while expanding #[test]"): add
  `use std::prelude::v1::test;` inside the test module (done for behaviors, view_cube,
  view_tools, analysis, appearance).
* `cargo clippy --workspace` may fail in other agents' crates; lint your code with
  `cargo clippy -p noshape-sketch -p noshape --no-deps --all-targets -- -D warnings`.
* A new sketch starts with the Line tool active: pressing `l` then toggles it off.
* `SketchData` fields are compared in its manual `PartialEq`: add new fields there.
* Ellipse axes are stored as radii but shown/typed as diameters (`dim_display_scale`);
  their expressions aren't stored (regen would re-evaluate them as radii).
