# Handoff: Assembly behaviour parity (agent "asb")

Checklist (kept current): `docs/parity/assembly.md` — one line per Onshape behaviour with status.

## State right now
- The shared tree **compiles** (last `cargo build -p noshape` with `CARGO_TARGET_DIR=/root/target-asb`
  finished cleanly after the Insert-dialog configuration change). No half-finished edits.
- `cargo test -p noshape-model` passes (all assembly tests incl. the new ones).
- `cargo clippy -p noshape-model -p noshape --all-targets -D warnings`: no findings in my files
  (assembly_*.rs, model/assembly/*, actions.rs, workspace.rs); other agents' files currently have
  clippy errors (assets.rs, part_studio/*, scene_builder.rs, sketch_mode.rs, render tests).

## Done (files)
- `crates/model/src/assembly/ops.rs` (new): list filter + colon commands, instance clipboard
  copy/paste (mates/MCs among copied instances), rename (instances, features, folders), hide
  others / all, Unfix all, mate visibility, Move to new subassembly / Dissolve subassembly,
  folders (add/unpack/delete/rename, drag reorder, drop into folder), display states
  (save/apply with Onshape defaults for new items), `next_assembly_tab_name`.
- `crates/model/src/assembly/mod.rs`: `MateConnectorRef.align: Option<Box<Reference>>`
  (Z axis from a face normal / edge tangent), `Mate.rotation_axis`, `Mate.hidden`,
  `Assembly.folders`, `Assembly.display_states`, `ListFolder`, `DisplayState`; groups now
  survive until their last member is deleted (Onshape); folder ids in `next_id`.
- `crates/model/src/assembly/eval.rs`: `resolve_connector_aligned` (align, cone virtual sharp,
  sphere center, inner-loop centroid), new `inference_candidates` returning `InferencePoint`
  with Onshape's per-entity point sets (planar: centroid + vertices + edge midpoints + circle
  centers + loop centers, normal Z; cylinder 3 axis points; cone + apex; circle edge center
  only; line mid + ends; no seam vertices), `nearest_inference`, `mate_dofs`,
  `SolveOptions.hold_values` (hold one DOF; animate / apply limit), mate rotation offset about X/Y.
- `crates/model/src/assembly/solver.rs`: `SolverMate.hold` residuals.
- `crates/model/src/demo.rs`: uses `Mate::new` / `MateConnectorRef::on` (struct grew).
- `crates/app/src/assembly_behaviors.rs` (new child module of assembly_view): context menus in
  Onshape order (instance / subassembly / top assembly / mate / MC / folder / graphics empty
  space / mate indicator) via `MenuEntry` + `fill_menu`; y / Shift+Y / Shift+I / Shift+T on
  hovered-or-selected; Exit isolate; Unfix all; zoom to selection; copy/paste (system clipboard
  JSON `noshape_instances`, paste follows cursor); inline rename (Shift+N, menus; focus deferred
  40 ms because closing a menu restores focus); list filter box; S shortcut toolbar; Shift+/
  list; a/q (snap drag, dialogs); Reset / Apply limit position; folders UI; subassembly ops;
  display states dialog; Show mates mode toggle; Replace tool; Animate dialog (DOF selector,
  start/end/steps, Single/Reciprocate/Loop, reverse, current value) + mate-dialog preview;
  Group dialog; Select other (`); Insert dialog configuration inputs; relation mate picking.
- `assembly_view.rs`: Shift-lock inference (from PointerMove modifiers), list (top assembly row,
  filter, hover highlight, eye on hover, scroll to selection, mate rows with tooltip, limit icon,
  expandable implicit MCs, folders, drag & drop), Insert dialog tabs/search/Esc, HUD checkmark,
  Solve, Shift+Enter, box select, fixed-drag message, mate indicator picking, viewport click
  blurs inputs (`focus_from_viewport`), `AssemblyViewEvent::ActivateTab` (Switch to Part Studio).
- `assembly_mate_dialog.rs`: rotation axis X/Y/Z, Reorder items, Solve, Animate, AcceptAndNew.
- `assembly_panels.rs`: triad context menu per handle (Move to origin / Align, Anti-align with Z /
  Rotate 90, 180 degrees); `refresh_snap_preview`, `ray_triangle` made `pub(super)`.
- `assembly_scene.rs`: pickable mate indicators (`OverlayPick::Mate`, ids from `1<<20`),
  `MateGlyphState` colors.
- `actions.rs`: `ReorientSecondaryAxis`, `a`/`q` bound in `Assembly && !Input`.
- `workspace.rs`: subscribes to `AssemblyViewEvent::ActivateTab` (5 lines).
- New icon `crates/app/assets/icons/mate-limits.svg`.

## Remaining (priority order, with notes)
1. Flexible subassemblies (Onshape default) + Lock/follow position: eval currently solves a
   subassembly internally and makes it rigid in the parent (`prepare_inner`, InstanceSource::Assembly).
   Needs flattening sub bodies/mates into the parent problem; then `Instance.locked` restores rigid.
2. Live mate values while dragging / selected mate value graphics with double-click edit (R11).
3. Ball mate cone-angle limit (D13): solver inequality on angle between Z axes.
4. Edit an implicit/assembly MC from the mate dialog (D17): open the MC dialog on the field's MC icon.
5. Drop instances into a subassembly (L20 rest): like `move_to_new_subassembly` but into an existing
   tab with id remapping and placement relative to the sub instance.
6. Snap-insert with explicit MCs from the Insert dialog (I14); Insert filters (I12); Generate for configs.
7. Measure / section view in assemblies (K26), transparency slider + expand by distance/connectivity (G16).
8. Vertex inference Z normal to a face (N9); MC Between entities / Realign (X4).
9. Rigid-assembly icon and warnings beyond "More than one part is fixed" (L24/L25, R13).

## How to test
- Model: `OCCT_ROOT=/root/occt CARGO_TARGET_DIR=/root/target-asb cargo test -p noshape-model --lib assembly::`
  (new tests: list_filter_colon_commands, copy_paste_..., rename_hide_others_and_unfix_all,
  inference_points_match_onshape, mate_reset_and_apply_limit_position, mate_rotation_offset_about_any_axis,
  move_to_new_subassembly_and_dissolve_keep_positions, hold_one_degree_of_freedom, folders_and_display_states).
- App: scripts in `/tmp/asb/bin/` (`build.sh`, `start.sh` opens Assembly 1 on `DISPLAY=:103`,
  `shot.sh name`). Window is not maximized: list rows at x≈190, Part 1 <2> y≈281, Pin y≈307,
  Assembly 1 tab at (338,933). Demo: Part 1 <1> fixed, Part 1 <2> fastened on it, Pin revolute.

## Gotchas
- Never run `pkill -f "...target-asb/debug/noshape"` inside a command whose own text contains that
  string (heredocs!) — it kills your shell. Use the scripts.
- Other agents break the build for minutes at a time; check the full error list, not only yours.
- xdotool: a mouse-up right after a key press without motion during a snap drag is sometimes lost;
  move the mouse 1-2 px before releasing.
- Modifier-change events are unreliable under xdotool; Shift lock reads PointerMove modifiers.
- The theme agent restyles render code in the same files (toolbar via `crate::ui`, list header
  "Mates"); re-read before editing and keep edits surgical.
- `MateConnectorRef.align` is boxed to keep `PatternKind` under clippy's large-variant limit.
