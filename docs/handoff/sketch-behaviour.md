# Handoff: sketch behaviour parity (Onshape sketcher)

Owner area: `crates/app/src/sketch_mode.rs` (editor logic, overlay, headless tests),
`crates/app/src/part_studio/sketch.rs` (GPUI integration: input routing, toolbar, labels,
glyphs, value boxes, panels, context menus), `crates/sketch` (solver, inference, tools).
Also touched (sketch-specific, surgical): `crates/model/src/regen/features/sketch.rs`
(external references, configured dimensions, text expressions, rho units) and
`crates/model/src/sketch_feature.rs` (new `SketchData` fields: `dimension_tolerances`,
`patterns`, `images`; `SketchText.expression`; image helpers; base64).

The checklist with statuses is `docs/parity/sketch.md`. **Keep it current** (it is up to
date as of the current continuation: 210 rows, 191 ✅, 15 ⏳, 4 🔧).

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

* Sketch crate: `CARGO_TARGET_DIR=/root/target-shared cargo test -p noshape-sketch`
  (`crates/sketch/tests/onshape_ux.rs` has the behaviour tests; `inference.rs` and `dxf.rs`
  have unit tests).
* Model: `cargo test -p noshape-model --lib text_expression image_tests`.
* App editor (headless, no GPU): `OCCT_ROOT=/root/occt CARGO_TARGET_DIR=/root/target-shared
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
* (Superseded 2026-10-19) A new sketch now starts with no tool active, per live sketch/00, 01, 44.
* `SketchData` fields are compared in its manual `PartialEq`: add new fields there.
* Ellipse axes are stored as radii but shown/typed as diameters
  (`ConstraintKind::display_scale`); expressions are stored in the shown scale and
  regen divides by the scale.

## skb continuation — 2026-10-18

- Working on DISPLAY=:103, CARGO_TARGET_DIR=/root/target-shared. No commits/pushes.
- Both requested test commands started; cold dependency compilation in progress.
- Added circular-pattern solver links (points, circle radii, ellipse axes/sizes),
  shared draggable construction center, and center recovery when editing a pattern.
  Regression tests cover seed dragging, center dragging, size edits and deletion.
- Fixed Bezier creation accepting more than 16 control points (silently became a
  multi-span B-spline): now shows the exact max-degree message and can still finish.
- Added Extend error and Bezier creation regressions. Verification still pending.
- Updated tools/sketch-ui scripts to respect DISPLAY, default :103 (old :106).
- Requested focused live checks in /root/onshape-research/live/REQUESTS.md.
- Sketch crate tests: **89 passed, 1 ignored**, including circular pattern tests.
- GUI baseline checked conic rho auto-focus/value commit (0.7), polygon side
  entry after locking circumference (7 sides/Enter), open spline white handles.
  Evidence: /tmp/skb/2026-10-18/*.png. Baseline is a copy of the integrated
  /root/target-lead/debug/noshape under target-skb/debug/noshape-baseline;
  new code still needs the fresh app build.
- Added sketch shortcut preferences in the sketch context menu; save/default/
  cancel; saved assignments take effect immediately; canonical modifier ordering.
  Compile/GUI verification pending. The initial app test compilation was terminated
  (SIGTERM, exit 143); restarted with -j 2, log app-tests.log in the evidence dir.
- Xvfb has no window manager: explicitly `xdotool windowfocus <window>` before
  keyboard scenarios. Added this to sketch_top.sh. Backtick Select other popup
  verified after setting native focus (row 9.7 updated).
- Foreground tool sessions may receive SIGTERM before long builds/UI sessions
  finish. Restarted the GUI with setsid; PID saved in evidence dir/app.pid.
- Fixed Use hover preview arriving one pointer event late: the GPU Hover result
  now refreshes the projection directly; switching tools resets its preview cache.
- Reused copied read-only dependency cache from target-lead in target-skb to
  avoid duplicating the full cold dependency build on this heavily loaded server.
  Builds still run only in target-skb. Stopped exact own orphan rustc PIDs after
  cancelling the earlier build; never broad-killed processes.
- Further fixes: Shift/Alt modifier changes now refresh inference even without
  pointer movement, and pressing Shift immediately before releasing a dragged point
  suppresses a pending coincident drop. New editor regression.
- In-place dimension input used length units for rho and radius scaling for ellipse
  diameters. Centralized edit text with dim_unit/dim_display_scale; new unchanged-
  acceptance regression for ellipse and rho. These app checks remain pending.
- GUI found a real Insert image/DXF fallback chooser bug: callbacks re-entered
  window_handle.update while that window was already borrowed by the chooser's
  confirm handler, silently dropping the import. Image now updates its view
  directly; DXF defers the window update before regeneration. Must recheck with
  NOSHAPE_NO_NATIVE_DIALOGS=1 in the fresh build; baseline cannot load either.
- Live behaviours.md now has sketch observations. Important correction: **one
  Esc ends a Line chain AND exits Line**, while double-click ends only the chain.
  Corrected cancel logic/test; prior test incorrectly required two Esc presses.
  Requested clarification of other tools' Esc, conic default rho persistence,
  and elliptical-arc click count (capture text differs from help's four clicks).
- Added Confirm <sketch name> to the sketch context menu per live observations.
- Conic now creates its intrinsic default driving Rho dimension immediately;
  dismissing its box retains it, typing edits that same dimension, rho >= 1 is
  rejected. Regression added; live sketch/84–86 shows persistent default Rho.
- T:ux pick priority regression added; stale-GPU fallback now also prefers an
  under-constrained point over nearby fully constrained points (not only GPU hover).
- GUI baseline model-face sketch: Use successfully projected the demo plate's
  large hole edge; right-click then offered "Delete sketch entity" (not just
  model actions), and selected the projected circle. Evidence use-circle.png and
  model-edge-context.png. The preview still needs the fresh-build timing recheck.
- Bezier creation finally verified (Degree 4 label in bezier-retry.png). Do not
  count degree editing as GUI-passed yet: the double-click edit attempt did not
  open an input in the baseline, despite T:ux degree-elevation passing.
- Found why baseline popup keyboard scenarios became unreliable: sketch root's
  capture-phase Escape handler did not treat context_menu as transient, so it
  intercepted Escape before the popup could dismiss/restore focus. Added it to
  the transient guard (also fixes Select other popup dismissal). Fresh GUI check
  required; click the canvas to dismiss baseline menus while testing it.
- Verified external constraint glyphs (8.16): Equal between a new circle and
  the projected demo model circle resizes the new one and shows blue glyph
  backgrounds on both entities. Screenshot external-equal.png. Cleared row 8.16.
- Updated row 4.28 to partial: GUI label exists, but baseline double-click input
  failed to open. This is now explicitly tracked instead of claimed complete.
- Verified 12.5: right-click a different, unused model edge in the face sketch
  offers "Use" and "View normal to sketch plane" (model-context-use.png).
- Verified chained slots (4.35) in baseline GUI: draw two connected L-shaped
  lines, preselect both, activate Slot. One continuous slot follows the chain,
  centerlines become construction, and one width input controls the end caps
  (chained-slot.png). Row updated with GUI evidence.
- Heavy-user sweep: trim used a capped 64-sample path, so a single large mouse
  move could skip curves between samples. Replaced it with swept-segment curve
  intersections and added a regression with lines deliberately between samples.
  Still awaiting the app test build.
- Verified Extend's exact error text (11.10) against a circle in the GUI:
  "Unable to extend the selected geometry." (extend-error.png). Row updated.
- Started subsequent baseline GUI runs with LP_NUM_THREADS=2 to reduce llvmpipe
  contention while the independent app dependency build is running.
- Live menu observations: Esc preserves selection; changed editor cancel and
  added a regression. Pending drawing/Mirror/Offset/pattern tools now exit on
  Esc too (polygon Esc must discard its pending preview per live observations).
- Live S toolbar: ten tools (includes Spline), two rows of five, S again moves
  it to the new cursor instead of toggling closed. Corrected sketch-only branch
  in part_studio/behaviors.rs; Part Studio toolbar behavior outside sketches kept.
- Sketch context menu now starts with Confirm and includes Select other,
  Zoom to fit/selection. Remaining generic analysis/Select submenus need integration.
- Surgical cross-file changes: part_studio/view_tools.rs adds a sketch-only
  Customize button to Shift+? shortcut sheet; preferences moved there from the
  sketch context menu. part_studio/behaviors.rs changes only sketch S toolbar.
- Live polygon captures exposed two missing interactions: on-canvas ◀ N ▶
  side-count control, and starting the next polygon with the commit click.
  Added both plus a regression; Enter still commits without starting another,
  Esc discards only the pending preview. Spinner locks the explicitly selected
  count so pointer movement toward its buttons does not alter it.
- Newly captured behaviors reveal broader inherited gaps, now explicitly tracked:
  14.15 Text uses a confirm/cancel dialog (250-character limit), but current code
  creates immediately and uses option rows; 14.16 fillet should stage its preview
  until Enter before opening radius input; 14.17 linear pattern needs a second
  independent direction/count (current linked implementation has one axis).
  These are not claimed complete. Fresh build/tests of current fixes take priority.
- LP_NUM_THREADS=2 and a 30-ms double-click made the baseline degree editor
  reliable: degree 16 shows the exact limit message, then 6 succeeds. Cleared
  4.28 and 13.3 with screenshots bezier-degree-input-lp2.png,
  bezier-max-degree-error.png, bezier-degree-six-verified.png. Earlier failed
  double-click attempts were not a degree-editor logic defect.
- Found stale max-degree message after a successful correction; cleared it on
  successful degree edit. Matched live strings "Deg N" and "Rho 0.5".
- Dimension value-box initialization now uses the document's actual units
  (previous generic creation used default mm even in inch documents), with
  extra entry precision instead of label precision. Inch ellipse regression
  added to the in-place edit test.
- Latest sketch-crate rerun completed: **90 passed, 1 ignored**, including
  circular seed/center/size links and movable-point pick priority. Used private
  nested cache /root/target-skb/sketch-tests while the app build held the main
  target-skb lock. Both stay inside the assigned build directory. Clippy for
  the sketch crate now running there; logs in the evidence directory.
- Sketch crate Clippy passed with -D warnings, all targets, no warnings.
- Refreshed the main target-skb dependency cache from completed target-ui;
  preserve cxxbridge symlinks when copying build/ (dereferencing hits SameFile
  errors against shared source). After a coherent refresh Cargo now compiles
  only the app test binary, rather than rebuilding the dependency graph.
- Fresh shared-root build became available in target-ui (12:26). Copied it to
  target-skb/debug/noshape-current for GUI validation while own app tests compile;
  its binary contains the new shortcut preferences/rho validation strings.
- App test compilation reports two existing unused-code warnings: stale
  MINI_FEATURES in behaviors.rs (removed, as S toolbar now uses toolbar prefs),
  and os_ui::secondary_button (UI agent area; leave for its owner).
- Requested app editor tests now passed: **36 passed, 0 failed** (`sketch_mode`),
  including all new Escape/polygon/fast-trim/rho/dimension/modifier regressions.
  Test build had the two warnings noted above; MINI_FEATURES has since been
  removed. Running the sketch integration parser/toolbar tests next.
- Fresh GUI passed 9.5: a free point placed one pixel from the fixed origin
  with Shift can be dragged from the origin overlap; origin stays fixed. Baseline
  failed this same scenario. Evidence current-point-priority.png. Row cleared.

- Fresh GUI verified import callback fixes: image chooser loads fixture.png and
  placing its box renders the bitmap; DXF chooser inserts both fixture entities
  (line + circle). Evidence current-image-loaded/current-image-inserted and
  current-dxf-inserted.png. 14.8 cleared; DWG reader remains absent (4.38).
- Fresh S toolbar verified ten tools in live order, two rows of five; second S
  at new pointer relocates it. Evidence current-s-toolbar-focused and
  current-s-repositioned.png. 14.12 cleared. Click canvas first after file dialog
  to restore keyboard focus. LP_NUM_THREADS=2 keeps interaction reliable.
- Used shared os_ui::secondary_button for shortcut Cancel, eliminating its
  unused-helper warning through a real UI integration.
- Integration-test rerun was interrupted by another agent's feature_dialog.rs
  select_all API typo; the owner has already corrected it. New editor test run
  now includes staged fillet/chamfer and Text dialog changes below.
- Text second click now stages a box, opening confirm/cancel Text dialog with
  preview, style/flip controls and 250-character validation; no document text
  exists until accept. Existing selected-text edits still use option rows.
- Fillet/chamfer click now stages an isolated SketchData preview; Enter commits
  then opens radius field. Fillet manipulator edits only that preview. These
  additions are compiling/test verification in progress; 14.15/14.16 pending.

## skb (Claude) continuation — 2026-10-18 13:00 UTC onwards

Took over from the Codex skb. Baseline check: the previous agent left one half-applied
edit (fillet "staged preview" assertions had landed in the tangent-arc section of
`remaining_geometry_tools_click_sequences`); moved back into `fillet_two_curves`. App
editor tests: **47 pass** (`cargo test -p noshape --bin noshape sketch`), sketch crate 91 pass.

New since then (all with T:sm/T:ux tests; GUI evidence in /tmp/skb/):
* Enter while any tool shows an idle value box = place it as a driving dimension at the
  current value (`SketchEditor::accept_otf`, Accept capture in sketch.rs); Enter with a
  drawing tool and nothing pending is swallowed (does not accept the sketch). Live 212–213.
* Press-drag-release from a Line chain's end places nothing (no arc, no point); the boxed
  length stays. Live 210–211.
* Spline/Bezier: first Esc commits through the placed points and keeps the tool
  (`escape_commit`, called from the Cancel capture before `sketch_cancel`); second exits.
* The sketch's own plane (or a coincident datum plane / the sketch face) is never
  hover-highlighted in sketch mode (`is_own_sketch_plane`; was tan under the cursor).
* Idle value boxes show bare numbers like dimension labels (`otf_labels`); units only
  once typing starts.
* `SketchEditor::apply_unit_defaults` (called on sketch entry): inch docs use live
  defaults; metric values are guesses until live capture answers (REQUESTS.md).
* Sketch context menu rebuilt in live order with Show all / Copy sketch / Curve-surface
  analysis / Section view / Select ▸ submenu (`noshape_sketch::select` —
  `Sketch::select_related`, `RelatedSelection`). "Construction" item removed (not in Onshape).
* Not my area but noticed: Sketch dialog "Show constraints" defaults off in noshape, live
  00-sketch-dialog-empty has it on (09 shows it off — user toggled?). Not changed.
* Later (after the lead's live-capture note): mm defaults R20/5/5/Ø20/25 mm; Enter never
  accepts the sketch (all unhandled Enter swallowed in the Accept capture); Show
  constraints starts on; Slot is preview-until-next-click (`pending_slot`,
  `commit_slot`, committed by click/Enter/Esc/tool switch — the toolbar switch path uses
  `needs_regen` because `set_tool` has no window), Slot is no longer a `draws()` tool
  (no 3-click slot drawing; live has none), centerlines stay solid (sketch crate
  `slot_around`/`slot_chains`); fillet Enter now syncs the radius value box; options
  panel removed for fillet/polygon/conic/linear pattern; on-canvas `Nx` count labels and
  linear spacing values (`pattern_count_labels`, `pattern_distance_labels`,
  `open_count_edit`); circular default 3.
* Next: circular pattern angle + Transform values on canvas (14.33), model-edge menu items
  (14.34), the remaining ⏳ GUI checks. For ui2: constraint glyphs draw over the value
  box (k3-enter-c.png); colours: fully defined #000000 vs live #101011, region #9aa5af
  (alpha?) vs live #dde2e6.
* Circular pattern / Transform panels removed too: `pattern_value_labels` /
  `set_pattern_value` (linear spacing, open circular angle, transform rotate/scale) and
  `pattern_value_dim` in sketch.rs; the Transform triad follows the selection
  (`follow_transform_selection`, `xf_center_pinned`). Text is the only tool with option
  rows left (OptKind is now Length/Text). App tests: 47 pass.
* Text: no side panel for a selected text any more; double-click a text reopens the Text
  dialog (`begin_text_edit`, `editing_text`, `text_dialog_open`), ✓ applies via
  `accept_text`, ×/Esc keeps the text; Convert to expression lives in the dialog
  (`text_expression`). Style buttons only change the dialog's copy.
* Model-edge right-click in a sketch uses the live sketch/204 layout (14.34).
* Sketch selection readout at the bottom right (`selection_readout`, 14.35).
* target-skb was wiped by someone around 14:30 UTC; full rebuild needed (~30+ min -j3).

## No-deviation pass (TEAM-RULES "No deviations from Onshape")
* Stale notes fixed: 2.2 (Insert group exists), 11.17 (circular instances linked), 11.19.
* 12.9 / 14.24 / 14.34 "Add comment": comments are a cloud-only exclusion per TEAM-RULES.
* In progress: DWG import via `cadkit-dwg` (MIT/Apache; adds a crates.io dependency to
  crates/sketch → Cargo.lock change for the lead), renderer textured image quads
  (`Drawable::Image` in crates/render) so sketch images draw in any orientation,
  Restore toast after ✕ (mod.rs, surgical), Select ▸ submenus (Create selection…,
  empty-space and model-edge variants), ellipse expressions.
* Import: `crates/sketch/src/cad.rs` is the single DXF/DWG pipeline (cadkit-dxf /
  cadkit-dwg → `Document::walk` with blocks expanded → exact conic mapping onto XY,
  dimensions left out). `dxf::parse_dxf` and `dwg::parse_dwg` are thin wrappers;
  the app calls `dwg::parse_cad` (bytes). New deps in crates/sketch: cadkit-core,
  cadkit-dwg, cadkit-dxf (=0.3.3, MIT/Apache) → Cargo.lock changed.
* DWG fixtures in crates/sketch/tests/fixtures/dwg come from LibreDWG test-data (GPLv3
  project, files unlicensed): the lead must decide whether they may ship (see SOURCES).
* Images: `SketchImage.data_base64` is now `Arc<str>` (serde rc); `SketchResult.images`
  → `SketchDisplay.images` (world corners) → scene_builder uploads one texture per image
  data (cache keyed by the Arc pointer) and draws a unit quad placed by the instance
  transform (`Drawable::Image`, renderer textured quads in crates/render). The GPUI
  overlay image path in sketch.rs is gone.
* Verified in GUI :103 (evidence /tmp/skb/m*.png): image in 3D with exact colors after
  the transparent-mesh depth compare became strict `Greater` (crates/render renderer.rs,
  so a coplanar datum plane no longer veils images), iso view, visible after confirming;
  Restore toast reopens the cancelled sketch; DWG insert (7 entities incl. block line);
  selection readout; model-edge menu + its Select ▸.
* Toast in mod.rs: ui2 restyled it with its own ×; my old extra × was removed and the
  new × also clears `canceled_edit`.
* Remaining ⏳: 3.13 (shortcut prefs GUI), 6.14 (model vertex/edge snaps), 8.8 (pierce),
  11.20 (Use hover preview), 14.3, 14.4, 14.10 — all implemented with tests, need GUI.
  Empty-space Select ▸ items in a sketch still await live capture (REQUESTS.md).
* Disk hygiene (TEAM-RULES): the lead's `find …/debug/deps -maxdepth 1 -type f -executable
  -mmin +60 -delete` also deletes proc-macro `*.so` files (they are executable) and breaks
  the target dir ("extern location for bytemuck_derive does not exist"). Use
  `! -name '*.so'`. Repair: remove `.fingerprint/*-<hash>` for every deps/*.d whose
  outputs are gone (done once on 2026-10-18).
* Verified in GUI today as well: 6.14 vertex snap on a face, 11.20 Use hover preview, 8.8
  Pierce, 14.3 Shift, 14.10 fast trim, 14.37 use glyph + unlink. Left: 3.13 (match the
  live "Customize keyboard shortcuts" page once captured), 14.38 (used geometry colour,
  ui2), Select ▸ items with nothing selected (live capture pending).

## skb run 3 (2026-10-19)
* Verified in GUI :103 against the live answers: "Escape line" menu (14.43), Select ▸ with nothing selected =
  Create selection… only (14.44), the two-step Esc with the menu open (14.45), L clears the selection (14.46,
  new unit test `choosing_a_drawing_tool_clears_the_selection`), Customize keyboard shortcuts → Preferences
  window (3.13). Fixed: the separator before Select ▸ in the nothing-selected sketch menu.
* QA3 sketch items (live values, hollow points, dimension text placement, applied-dimension highlight) were
  already done by the previous run (14.39–14.42).
* Builds now go to the shared `/root/target-shared` (TEAM-RULES); copy the binary to /tmp/skb/noshape.
* Lead / ui pixel pass: a new sketch starts with no tool active (part_studio/sketch.rs, `start_sketch`
  path) and the empty sketch frame is 200 x 112.5 mm (`EMPTY_SKETCH_BOX_HALF_HEIGHT`, sketch_mode.rs
  `sketch_box`); live confirmation requested in REQUESTS.md.

## skb run 4 (2026-10-19 afternoon)
* QA3 "dimension value box shows 80 without unit": `dimension_edit_text` now formats plain-number
  expressions with the document unit ("80" → "80 mm"); real expressions ("2 * 15 mm", "#w") keep their text.
  Row 14.47; the extended test `reopening_ellipse_and_rho_dimensions_preserves_the_displayed_value` checks it.
* The dimension box text now uses 6 decimals for lengths and 7 for angles (live sketch/13, 141, 143), so the
  unit fits the 100 px input; Enter on an untouched box closes it without re-applying the rounded value.
  Verified (test + GUI dm3). For ui2: the box's Input renders the text left-aligned and clips the unit's last letter
  ("16.825693 m"); live right-aligns "1.725582 in" inside the same 100 px field.

## skb run 6 (2026-10-19 evening, takeover after the quota stop)
* Sketch frame = Onshape's `SketchPlaneDisplay` (bundle webgl class `oM`, `updateBounds` / `updateText`; constants in
  woolsthorpe: `VIEWPORT_HEIGHT_TO_PLANE_HEIGHT_RATIO` .7, `PLANE_DEFAULT_WIDTH_HEIGHT_RATIO` 1.77777,
  `PLANE_TEXT_HEIGHT_PX` 20, `PLANE_TEXT_MARGIN` 5, `PLANE_TEXT_BACKGROUND_PADDING` 2, colour `--os-color-seaport-500`
  #90cef1): placed ONCE when first drawn, centred on the sketch plane entity's bounds centre (plane origin / face mesh
  bounds / connector; if off screen, the ray through the graphics centre, ≥15° to the plane), 0.7 × graphics height
  tall at the current zoom, then fixed in the plane (no growth with geometry, zoom doesn't change it); a new sketch
  plane re-places it. Code: `SketchEditor::frame_bounds` / `place_frame` / `sketch_box()` (no data arg any more),
  `PartStudioView::place_sketch_frame` (sketch.rs, called from `sync_sketch_editor`). GUI n1: 2.02 × 1.14 of the Top
  plane, same as live 501. Label: 16.5 px ascender in the plane, 7 px inside the top-left corner, shrinks to fit.
* Sketch dialog options are user settings (bundle: `sketchShowConstraints || false`, `sketchShowExpressions || false`,
  `sketchShowErrors !== false`; `commitFeature` saves them when they differ from the opened ones): `SketchOptions`,
  `sketch_options()`, `SketchEditor::save_options_if_changed` (called from `accept_edit` in part_studio/mod.rs, one
  surgical line), file `<config>/sketch-options.json`. So a fresh install starts with Show constraints OFF (live 00);
  live 501 had it on because that account saved it.
* Dimension value box: tolerance button is now Onshape's `hole-tolerance-precision` icon (new asset, copied from
  ui-ref/icons-svg) at opacity 0.5 (was a "±" glyph). Text right-aligned, ends 4 px from the field end (live 3 px).
  For the lead / theme: the focused input's text selection is the global `t.selection` #b2ddf6 (main.rs) with dark
  text; live Chrome shows #3367d1 with white text in the dimension box (live 13, 141).
* No tool active after picking the plane: verified (GUI n1, a click in empty space does nothing).
* Lead question (applied dimension selected?): settled from the bundle. The sketch Dimension tool's commit ends in
  `cleanUpAfterCommit()` → `selectionList.reset()` + preselection reset; the dimensioned entities are only
  highlighted (`highlightDimensionEntities`, cleared with the edit box). So a NEW dimension leaves nothing selected
  (live sketch/15); sketch/148's selected 30° isn't a fresh commit. `DimEdit.created` (true from `open_value_box`,
  false when an existing label is clicked); `commit_dimension` (both the apply and the untouched-Enter paths) and
  `SketchEditor::cancel` clear the selection only for created ones. Test `a_new_dimension_leaves_nothing_selected`;
  GUI dm4. Row 14.42.

## ui2 10-22: took the sketch Text tool look (lead)
- ui2 owns the Text tool dialog's look now (`part_studio/sketch.rs` `render_text_dialog`, surgical):
  standard feature-dialog header (green ✓ / red ✕) in the UI font in the sketch dialog's place, as live
  (dom/dialog-sketch-text, sketch/132).
- Fonts (lead 10-22): `noshape_sketch::text::SKETCH_FONTS` = live's 32 fonts in live's order, default
  `SKETCH_DEFAULT_FONT` = Open Sans (new texts; scene `sketch_mode.rs`), files in `crates/sketch/fonts/sketch/`
  (zstd, 34 MB; README there). B / I buttons only for fonts with the style (`SketchFont.bold/italic`), hidden
  otherwise. `DEFAULT_FONT` (Liberation Sans) stays for the app's own glyphs and older documents. FS host
  `font_of` maps Onshape font file names to these families (`sketch_font_of_file`). Test
  `every_sketch_font_lays_out_in_its_styles`.
