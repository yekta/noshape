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

## Milestones since the first handoff
- Flexible subassemblies + Lock/follow position (done, tested): `Instance.lock: SubassemblyLock`
  (Flexible default / Current / Named) and `Instance.nested` (children positions stored in the
  parent). eval: `flex_subs`, `Prepared.nested_body_of / nested_bodies / flexible / connector_child`,
  `Owner` enum in connector resolution, sub mates added with synthetic ids (`1<<62`+),
  `Solved.nested`, `SolveOptions.nested_start / nested_drags`, `PartOccurrence.flex`.
  App: `edit()` folds `solved.nested` into `Instance.nested`; free drag of a part of a flexible
  subassembly uses `nested_drags`; menu "Lock/follow position to"; lock icon.

- Live mate values (done): `render_mate_values` / `apply_mate_value` in assembly_behaviors.rs.
- Ball cone limit (done): limit key "cone" = value index 4 in the solver limits (angle of Z axes).

## Pass 3 (2026-10-18, agent "asb", second run)
- Standard content (done, verified in the app): `crates/model/src/assembly/standard.rs`: catalogue
  (ANSI / DIN / ISO bolts, screws, nuts, washers, pins; size tables in mm or inches), `StandardSpec`
  (`InstanceSource::Standard { spec }`), parts generated by a built-in Feature Studio script
  (`SOURCE`, compiled once in `scripts()` under the reserved tab id `STANDARD_TAB`) in a one-feature
  Part Studio with fixed feature ids plus explicit mate connectors (bolts: under the head, +Z out of
  the head; nuts / washers: bottom and top). `auto_size` (bolts round down, nuts up, washers by bolt
  size), `hole_connectors` (circular edge / cylinder: one hole end facing the viewer; planar face:
  all its holes). Evaluator caches parts per spec (`AssemblyEvaluator::standard_part`, error text
  goes to the instance row). App: `crates/app/src/assembly_standard.rs` (tab UI, auto-size pick,
  Insert, `a` flip while placing, Edit standard content instance). Placing standard content snaps
  to mate connectors without snap mode (`update_placing` / `update_insert_snap`).
  Script gotcha: ctx booleans return only *new* bodies; keep using the original handles.
- Isolate / Make transparent › Expand by distance / connectivity (`analysis::isolate_expansion`,
  `set_iso_expand`), verified.
- Assembly mate connector: Origin type Between entities, Realign (primary / secondary axis entity)
  (`AsmMateConnector.between / primary_axis / secondary_axis`, dialog fields, `McSession.slot`).
  Between verified in the app; Realign model-tested.
- Instance warnings (verified) and rigid subassembly icon (`InstanceNode.rigid`, not yet seen in the app).
- Mates list drag to reorder (`Assembly::move_feature_before`, `MateDrag`), verified.
- Insert dialog Parts / Surfaces filter icons (only when the document has surfaces).
- Vertex hover: Z normal to a planar face around it.

## Pass 4 (2026-10-18, agent "asb", third run): live Onshape comparison
Checklist section 13 (LA1–LA21) lists every live-capture difference and its status.
- Insert dialog rebuilt to the live layout (`render_insert_panel`): 300 x 587, ✓ (dim until inserted) / ✕,
  two-line source tabs, document row with Create version / Version graph, Part Studios | Assemblies
  sub-tabs, per-tab search placeholder, filter row (Parts default on, Surfaces, rigid toggle), tree
  rows with chevrons (collapse state `AsmUi.insert_collapsed`) and 70x40 icon cells, footer
  "Inserted: N" (`inserted_count`, `AsmUi.insert_inserted`) + "↶ Undo to remove instances"
  (`undo_last_insert` undoes insert + place).
- Instance list glyphs after the name: `list-fixed`, `list-free` (has DOF), `list-grounded` on the
  top row; no DOF numbers; "Filter by name".
- Triad at the clicked point (`triad_at_click`); triad handles are selected by a click
  (`AsmUi.manip_selected`) and only a selected handle has a context menu.
- Graphics right-click: unselected instance → general menu; selected → instance menu.
- Menus reordered to the live DOM dumps (instance list / graphics / top row / mate / empty space);
  new: Section view…, Find in instance list, Select other…, View normal to (`hovered_face_normal`),
  Export… (`open_asm_export`, `export_occurrences`: STEP / IGES / STL / OBJ / BREP at solved positions),
  Hide other mates / Hide all mates, Animate… label.
- Mate dialog: chips "Mate connector of <instance>" (labeler), × always shown, ⇅ swap in the box
  header (replaces "Reorder items"), ▶ always shown (dim at 0 DOF), type order Slider, Cylindrical,
  Revolute, Pin slot, Planar, Ball, Fastened, Parallel. Floating mate toolbar `render_mate_hud`
  (◻◻◻ drag handle via `HudDrag`, type select, flip, reorient, Solve, ✓ commit-and-new, ✕).
- Not yet verified in the app (build blocked by other agents' files when written): mate HUD, menus
  after the reorder, Export…, View normal to. Verify these first.

## Remaining (priority order, with notes)
1. Finish section 13: Insert thumbnails (render each part offscreen, `gpu.render_thumbnail`),
   Show center of mass, Properties… / Instance properties…, origin row menu (LA16), triad look
   (ui2), "Simulation connection" n/a.
2. Standard content: Insert closest / furthest (stacking on washers and nuts), washer under a
   mated bolt head, thread type, part number / description.
3. App checks still to do: Realign fields, rigid icon (needs a subassembly with one fixed part and
   0 DOF), surfaces filter.
4. Insert dialog: thumbnails, inserted count (I16); Generate for configurations (I13).
5. Isolate: Select transparent geometry; sliders instead of −/+ (ui2 owns the visuals).
6. Not planned: in-context modelling, PDM / release, comments (C25, A6, G13, T9).

## How to test
- Model: `OCCT_ROOT=/root/occt CARGO_TARGET_DIR=/root/target-asb cargo test -p noshape-model --lib assembly::`
  (new tests: list_filter_colon_commands, copy_paste_..., rename_hide_others_and_unfix_all,
  inference_points_match_onshape, mate_reset_and_apply_limit_position, mate_rotation_offset_about_any_axis,
  move_to_new_subassembly_and_dissolve_keep_positions, hold_one_degree_of_freedom, folders_and_display_states).
- App: scripts in `/tmp/asb/bin/` (`build.sh`, `start.sh` opens Assembly 1 on `DISPLAY=:104` (window resized to 1600x1000), `std.sh` opens Insert › Standard content › ISO socket head screws,
  `shot.sh name`). List rows at x≈170, Part 1 <2> y≈231, Pin y≈257, Assembly 1 tab at (256,985). Demo: Part 1 <1> fixed, Part 1 <2> fastened on it, Pin revolute.

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
