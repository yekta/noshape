# Handoff: Assembly behaviour parity (agent "asb")

Checklist (kept current): `docs/parity/assembly.md` — one line per Onshape behaviour with status.

## skb handoff at the halt (2026-10-26, agent "skb")

**On main (all quick-checked):**
- Sketch: solver plateau stop (8ba5475c, conflicting dimension ~10 ms); regen reuses the cached DOF analysis
  (082f47d1); `sketch_get` names the Origin instead of the hidden stand-in (5c94bb51; tests fixed a12a4c86).
- Assembly solver: F9 exact least change (f2cecb07); typed triad values exact + box shows the achieved value
  (0b4a8f58, `solve_placed`); mate values in live's sense + Ball cone limits seated (9d0f292f, oracle S2/B2,
  R2's turn); least change pulls only along free directions (215aa174, oracle M2); main fix 75669a58.
- Assembly perf: connector memo (18cacdd5); drag steps move the built scene (`asm_scene::retransform`,
  app `move_scene`) and Part Studios hashed once per evaluation (2d558082). perf.yml model job: sketch_perf
  (90e6ac14) and assembly_perf steps.
- Standard content: ASME socket button / countersunk cap screws as live (88600cd6).

**On branch `agent/skb` (WIP b826d8f8, unchecked, NOT on main):**
- F19 Width mate: `crates/ops/src/assembly.rs` mate_preview_options holds the first connector instance that isn't
  `a`'s (cause: with both block faces as a/b the preview held the block itself, so the mate read "could not be
  solved"); `solver::pair_width_connectors` groups the four connectors by part (called in eval.rs, both places);
  test `crates/ops/tests/mate_flip.rs` width_mate_solves_in_any_face_order.
- DOF cache: `solver::DOF_CACHE` / `dof_key` (numbering-independent exact key) reuses each group's DOF result.

**Open items, in priority order (exact next steps):**
1. F19: `git cherry-pick b826d8f8` (or rebase agent/skb on main); run `cargo clippy -p noshape-model -p noshape-ops
   --all-targets -- -D warnings`, `cargo test -p noshape-ops --test mate_flip`, `cargo test -p noshape-model --lib
   assembly::solver::tests`; push. If the DOF cache should land separately, split solver.rs (DOF_CACHE/dof_key and
   the cache lookup/insert in solve_inner's DOF loop) from the Width parts.
2. Assembly perf, inserting a pin into 200 pins / 300 mates still ~50 ms: the DOF key contains the global
   `scale` (largest |origin| of all bodies), so a far insert misses every group. Next: compute a per-group
   characteristic length for the DOF analysis (jacobian + key) in `solve_inner`. Then rerun
   `cargo test -p noshape-ops --test assembly_perf -- --ignored --nocapture` (targets: frame < 16.7 ms, edits < 100 ms).
3. oracle_asm R2 waits for fs's R1 connector flip (our turn already matches); CP1 broke with kern 339de774
   (extrude swept from the far end: the pin cylinder's MidAxis connector points the other way, the pattern turns
   the other way) — fs/kern, not in my files.
4. Long tail: ASME socket head shoulder screw capture (geo8.js can't parse its table: write a dedicated capture).

**Known failing tests in my area:** `oracle_asm` (CP1, from kern 339de774). Main was RED at e339bda0 on
kernel_robustness `propagation_off_next_to_corner_fillets` (not mine).

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
- Verified in the app since: mate HUD (drag, type select), menus after the reorder, Export… (STEP),
  limits rows, Insert thumbnails (`assembly_thumbs.rs`), Show center of mass, origin row menu,
  Animate mate dialog. Not verified: View normal to, Find in instance list, Select other… entry.

## Handover to agent "skb" (2026-10-18, asb moves to drawings only)
- No half-done edits in the assembly files. Everything listed above compiles in the shared tree.
- Pass-4 additions not in the list above: `assembly_thumbs.rs` (Insert dialog thumbnails, child module
  of assembly_view, `AsmUi.thumbs`), Show center of mass (`toggle_center_of_mass`,
  `refresh_center_of_mass` from `apply_result`, `AsmUi.com`, icon `com-indicator.svg`), origin row menu
  (`origin_row` context menu, `AsmUi.origin_hidden`), Animate mate dialog rebuilt to the live layout
  (default Reciprocate, 300 steps, "0 deg"), limits rows with `limit-*.svg` icons.
- Open ⏳ rows in docs/parity/assembly.md: K11, I14, L18, L24, C20, N4, N5, N8, N9, R13, S3 (need an app
  check). Lead rule: nothing may be skipped except cloud-only, multi-user or simulation, so every ❌ /
  🔧 row is in scope (see point 6 below).
- Build tip: other agents often break the shared tree for a while; `/tmp/asb/bin/wbuild.sh` syncs it
  into the private worktree `/root/noshape-asb` and builds with `/root/target-asb` (use your own).

## Remaining (priority order, with notes)
1. Finish section 13: Insert thumbnails (render each part offscreen, `gpu.render_thumbnail`),
   Show center of mass, Properties… / Instance properties…, origin row menu (LA16), triad look
   (ui2); "Simulation connection" is n/a (simulation).
2. Standard content: Insert closest / furthest (stacking on washers and nuts), washer under a
   mated bolt head, thread type, part number / description.
3. App checks still to do: Realign fields, rigid icon (needs a subassembly with one fixed part and
   0 DOF), surfaces filter.
4. Insert dialog: thumbnails, inserted count (I16); Generate for configurations (I13).
5. Isolate: Select transparent geometry; sliders instead of −/+ (ui2 owns the visuals).
6. In scope (lead, 2026-10-18: nothing may be skipped except cloud-only, multi-user or simulation):
   in-context modelling (Edit in context, Create Part Studio in context: T9, G13), release / revision
   (Release…, Revision history), Change to version, Dynamic suppression, Use best available
   tessellation, Properties… / Instance properties…, sketch instances (Sketches filter), sliders in the
   Isolate panel. Only comments / tasks to other users and "Simulation connection" are n/a.

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

## Takeover by skb (2026-10-18 evening) — skb now owns assembly behaviour
asb moved to drawings. skb builds in /root/target-shared (since 2026-10-19; copy the binary to /tmp/skb/noshape and run the copy), display :103; app start script
`/root/skb-scripts/astart.sh` (demo document, Assembly 1 tab on :103), screenshots in /tmp/skb/asm/.
Done so far:
* "Use best available tessellation" (instance menus, toggle with ✓): `Instance.best_tessellation`,
  evaluator re-tessellates the instance's parts at `BEST_TESSELLATION` (0.1 × chord tolerance,
  `regen::tessellate_shape`, cached per shape hash in `AssemblyEvaluator.fine_meshes`). Model test
  `best_available_tessellation_refines_only_flagged_instances`. Verified in the app.
* Instance menus re-ordered to the live dumps; "Select instances with same part and same
  configuration" removed (not in Onshape's menu); Show mates in its own group (list only).
* Mate menu: Expand ▸ / Collapse ▸ (selected / all).
* QA3: mate HUD ✓ = "Commit current mate and start a new mate" (live tooltip, help) — kept;
  Insert footer counts the instance following the cursor (live 04: "Inserted: 1"), the
  "Click to place…" toast and banner removed, the following instance is selected (amber) and
  deselected on placing; no row tooltip while a context menu is open (`os_menu::is_open`);
  mate values hidden while Animate is open; Animate playback is time based (60 steps/s).
  Animate dialog already had the live layout (QA ran an older build).
Round 2 (live answers in behaviours.md "Assembly right-click menus"):
* Properties… (instance row: part metadata; top row / subassembly: assembly metadata with
  Subassembly BOM behavior) — shared `crates/app/src/properties_dialog.rs` (800 px Bootstrap
  modal, form undo/redo, Apply/Save/Close); model `PartProperties.meta: ItemMeta`,
  `Assembly.properties: AssemblyProperties`. Instance properties… → `Instance.reference_designator`.
* Show / Hide instance list properties: gray state pill after the names (`ui.list_properties`).
* Dynamic suppression ▸ Add expression… → "Dynamic suppression" dialog; `Instance.unsuppressed_if`,
  applied in `eval::prepare_inner` via `dynamically_suppressed` (model test). Rows of dynamically
  suppressed instances show suppressed (node state).
* Use best available tessellation ↔ "Use automatic tessellation setting" (same slot).
* Edit in context: `AssemblyViewEvent::EditInContext` → workspace activates the Part Studio and
  calls `PartStudioView::set_assembly_context` (`part_studio/in_context.rs`: translucent context
  meshes in the Part Studio's coordinates, banner, Hide/show instances, transparency slider).
  Transient; NOT yet: persistent Assembly contexts, references to context geometry, Update context.
* Create Part Studio in context (toolbar + Origin row): dialog "Origin of new Part Studio"
  (Origin or an assembly mate connector row), new Part Studio tab after the assembly, opened in
  context; banner "Go to assembly ▾" › Insert and go to assembly (`PartStudioEvent::InsertInAssembly`).
* Reference manager (Change to version…, toolbar Update all references): `assembly_props.rs`.
* Tool search Alt+C in assemblies (`assembly_search.rs`); the non-Onshape "Fix / unfix selected"
  toolbar button removed.
* Sketch instances: region fill + gray curves/dots (`eval::sketch_wire_result`, assembly_scene),
  Insert dialog sketch thumbnails (normal view) under the parts.
Open / waiting for live captures: Named positions panel + "Edit named position driver mate…"
(REQUESTS), Dynamic suppression menu once set, Unit of measure list, tool descriptions.
Release… is a paid-plan upsell on Onshape (subscription → excluded).

Requested from live capture (REQUESTS.md "skb (assembly)"): Properties… / Instance properties… /
Show instance list properties, Dynamic suppression ▸, Edit named position driver mate…,
Use best available tessellation look, Change to version…, Release…, Edit in context,
Create Part Studio in context…, Sketches toggle in Insert.

## skb run 3 (2026-10-19, taking over after the quota stop)
State: shared tree compiles in my files (cargo check -p noshape clean except other agents' in-progress files).
* Clippy blockers: `TabContent::Assembly(Box<Assembly>)` (all call sites updated, incl. drawing/demo/tests,
  workspace.rs); eval.rs sketch-instance cache uses type aliases `SketchKey` / `SketchBodyEntry`.
* `assembly::tests::sketch_instances_are_wire_occurrences_with_edges` passes.
* Fixed someone's half-done dropdown migration in assembly files (the `DropdownMenu`/`PopupMenuItem`
  import was removed from assembly_view.rs): assembly_behaviors filter / Animate playback, assembly_standard
  dropdowns and the mate HUD type list now use `os_dropdown_menu` + `OsMenuItem`.
* BOM Table panel (live 181–187): model `crates/model/src/assembly/bom.rs` (`BomSettings` stored in
  `Assembly.bom`: Flattened / Structured, columns (Item frozen) + Add column list, Show excluded/suppressed,
  Show top level assembly row; `lines()` walks the instance tree honouring Subassembly BOM behavior and
  Exclude from all BOMs; `cell` / `editable` / `set_cell` write part / subassembly Properties; `table_text`
  CSV / TSV). App `crates/app/src/assembly_bom.rs` (docked 712 px pane, header row, header and row
  right-click menus, inline cell editing, Export to CSV, Copy table). Model test
  `bom_flattened_structured_exclusion_and_cell_edits`. Old `analysis::bom` still used by drawings.
* Assembly configurations (live 190–200): model `crates/model/src/assembly/config.rs`
  (`Assembly.configuration: AsmConfiguration` = inputs + `AsmConfiguredParam { input, target:
  ConfigTarget::Instance|Mate, param, values }`; params unsuppressed / offset / offsetZ/X/Y / rotation /
  flipPrimary; `configured(values)`, `apply_configuration`, `configure_param`, `store_mate_edit`,
  `set_configured_value`, `add_config_row`, `delete_config_input`, `prune_configured`).
  `InstanceSource::Assembly { assembly, config }` (subassembly instances in a configuration; evaluator
  applies it). App `crates/app/src/assembly_config.rs`: Configuration panel (empty state, tabs, sections,
  two-level header, Name add row, footer split button), Configurations box above the instance list,
  yellow "Select instances or mates to configure" / "Select parameters to configure" modes (toolbar greyed),
  mate dialog `configuring` mode (dashed amber frames, Unsuppressed checkbox, ✕ only,
  `MateDialogEvent::ConfigureParam`), instance menu "Configure suppression", info toasts. The active
  configuration is `AsmUi.config_values` (applied in `effective()`). Model test
  `assembly_configurations_drive_suppression_and_mate_offsets`.
* Right strip now matches live: BOM Table, Configuration panel, Exploded views, Named positions, Variable
  table (Simulation excluded; Interference via top row › Check interference…, Animate via mate row).
* Known failing model tests NOT mine: `standard_content_*` (FeatureScript host changes by agent fs).
* REQUESTS.md (skb): mate values visibility after a drag, composite parts, Customize toolbar, BOM details.
Next: build + verify BOM / configurations in the app, then the remaining 🔧/⏳ rows.

### skb run 3, milestone 2 (live answers for mate values, BOM details, composite, customize toolbar, in-context)
* Build: shared target `/root/target-shared` (TEAM-RULES). My scripts: `/root/skb-scripts/bld.sh` (retrying
  build, kills my app, copies to /tmp/skb/noshape), `astart.sh` (demo doc, Assembly 1 on :103), `s.sh name`
  (screenshot to /tmp/skb/asm/). pkill patterns must be anchored (`^/tmp/skb/noshape`), else they kill the shell.
* Mate values (R11, live 300–318): only the selected mate; blue dashed circle with arrowheads
  (`Highlights.value_arcs`, assembly_scene.rs) + "275.326°" label (render_mate_values); nothing during/after a
  drag (fixes QA3 "drag readout stays"); Esc keeps the selection (the Esc fallback no longer clears it).
* BOM details (live 350–378): excluded rows "--" at the end, suppressed instances gone, top level row at the
  bottom (Item = assembly name, bold, CSV "Assembly1"), Appearance "Default", Material "- None -", Mass "No
  value" on pink with the ƒ header marker, Not revision managed checkbox, Tessellation select; click selects a
  cell (blue frame, header + Item #d7d7d7), double-click edits, Enter commits and moves down, Esc cancels;
  Structured: "›/⌄ 3" chevron in the Item, double-click a row to expand, ⋯ Collapse all / Expand all; Copy table
  toast "Flattened BOM table copied to clipboard."; CSV "BOM of Assembly 1.csv". Verified GUI g2.
* In-context modelling persisted (G13, live 500–525): model `crates/model/src/in_context.rs` +
  `PartStudio.contexts / active_context`, regen injection (regen/mod.rs: `uses_context`, `ContextCache`,
  `StudioDisplay.context_bodies`), Part Studio: `part_studio/in_context.rs` (`display_for_scene` wraps the
  renamed `display_for_scene_inner` in behaviors.rs; "Assembly contexts" section hook in mod.rs; context
  arrows in feature rows; labeler "Face of Context 1"; missing-reference check skips context features).
  Assembly: `assembly_props.rs` (`add_assembly_context`, `update_context`, `set_primary_instance`,
  `instance_contexts`, `context_icons`), menus in assembly_behaviors.rs. Important: Part Studios regenerate
  only on `DocEvent::TabChanged(tab)`, so context edits emit it explicitly. Verified GUI e2–e16.
  Run 4: magenta context references (`context_ref_overlay` in part_studio/in_context.rs, hooked in
  `refresh_scene`; `ui.context_arrow_hover`; model `in_context::context_refs`), greyed section during a
  feature dialog, and stale dots in the Part Studio (`assembly_view::StaleContexts` global, filled by
  `publish_stale_contexts` in `apply_result`, cleared by both Update context paths). Verified GUI c11–c31.
  Note: in a Part Studio, translucent default planes win the pick over context faces behind or in front of
  them (renderer pick pass, not mine); pick ghost faces away from the planes.
  To do: "Insert and go to assembly" part chooser dialog (LA33, waiting on REQUESTS line 201).
* Next: composite parts (I12), Customize toolbar (T12, live captured), remaining 🔧 rows, QA3 Animate check.

### skb run 4 (2026-10-19 afternoon, after the context reset)
* Clippy clean in my files (`cargo clippy -p noshape --tests`: remaining warnings are in material_libraries,
  session, material_panel, material_library_view, feature_studio, drawing_section, drawing_export, none mine).
  `AssemblyViewEvent::EditInContext` now carries `Box<AssemblyContext>` (large_enum_variant); workspace.rs
  derefs it. Model `assembly::` tests 57/57 green.
* In-context: magenta references, greyed section while a feature dialog is open, Part Studio stale dots
  (see milestone 2 above).
* Verified and marked ✅: I12 (Insert filters + composite row, GUI i1–i7), I14 (snap-insert with a Part Studio
  mate connector → Fastened 2, k6–k9), I16, K11/S3 (Ctrl cycles the dragged connector, k1–k3), L6 (`:warnings`
  now uses `instance_warning`, `:folder` lists folders while filtering, w1/w4).
* Fixed: Insert dialog "Create version…" dispatched `actions::CreateVersion`, which nothing handles; it now
  calls `history_panel::open_create_version`. (The Ctrl+Alt+V binding of that action is still unhandled; that's for the lead
  or the workspace owner.) LA34 Reference manager checked against live 111 (v3–v6).
* Graphics instance menu: "Edit in context" (or its ▸ submenus once contexts exist) now sits before "Find in
  instance list", as in the live dump. LA12 row rewritten (it had a status spliced into its title).
* REQUESTS.md: one batched skb line (fixed-instance drag cue, snap drag message, Animate in the mate dialog,
  MC icon in the mate dialog, Solve scope, configured Part Studio Insert/Generate, sketch region inference,
  after Update all). Lines 200–202 (Unfix all / Check interference, Insert and go to assembly dialog, sketch
  start) are still open.
* D18 (help: Mates): while a mate is being defined only that mate is solved (`SolveOptions::only_mate`, set in
  `effective()` until Solve); model test `a_mate_being_defined_is_solved_alone_until_solve`.
* The render shader break mid-run (someone else's) was fixed by its owner; LA12 (g3–g5) and D18 (d1–d2) are now
  verified in the GUI.
* Lead notes: Edit in context selects the part, and the Part Studio then opens its Measure panel; live 502 shows
  none (Part Studio measure code, not mine). `actions::CreateVersion` (Ctrl+Alt+V) has no handler.

### skb run 5 (2026-10-19 afternoon, takeover after the quota stop)
State: all edits below are in the shared tree; `cargo check -p noshape` showed no errors in my files (other agents'
release_dialog.rs / variable_studio.rs / drawing_drawprops.rs / numbering_prefs.rs were mid-edit). Model `assembly::`
tests: 58/58 at the start; new tests listed below. GUI verification of this run still to do (build lock was held
for a long time by others' kernel rebuilds).
* Found half-done (previous run): Insert › Other documents (now rewritten, see below), Reference manager row
  thumbnails + 20 px two-tone update icon, Insert footer `?` (`props::help_dot`), sketch dimension box
  right-aligned 13.33 px (`part_studio/sketch.rs`, `Input…text_align(Right)`), sketch name drawn in the plane.
* LA33 (live 545–551): `part_studio/in_context.rs` banner for a Part Studio created in context = split
  [Insert and go to assembly][▾] (Insert and go to assembly / Go to assembly / Hide/show instances) + ✕;
  "Select items to insert" dialog (`render_insert_go`, `InContextUi.insert_go`; hooks: `pick` / `pick_body` /
  `highlighted_hover` in behaviors.rs, Esc and the overlay in mod.rs). `PartStudioEvent::InsertInAssembly` now has
  `sketches`; `AssemblyView::insert_from_context(studio, parts, sketches, at)`.
* REQUESTS 200 (Unfix all): top row Unfix all right after Show center of mass; folder rows: Dynamic suppression ▸
  (`ListFolder.unsuppressed_if`, eval `dynamically_suppressed`; test `dynamic_suppression_of_a_folder_suppresses_its_instances`),
  Unfix all, Expand ▸ / Collapse ▸; `DynTarget::Folder`.
* REQUESTS 204: (1) fixed drag → `AsmUi.fixed_drag` (no-drop cursor + fix glyph, no message); (2) no snap-drag
  message (Ctrl cycling kept: live had a single connector, so it matches); (3) mate dialog ▶ = 2 full turns once then
  back to the start pose (`toggle_mate_preview`, `finish_mate_preview` via `AsmUi.preview_done` in render), value arc
  shown for the preview mate; (4) implicit connector edit stays implicit (`MateConnectorRef.moved: ConnectorMove`,
  `eval::implicit_move`, `McSession.implicit/to_implicit`, `sync_implicit_mc` called from `schedule_eval`; test
  `implicit_connector_move_offsets_the_mate`); (5) drags enforce every mate even with `only_mate` (eval
  `solve`; extended test `a_mate_being_defined_is_solved_alone_until_solve`); (6) configured insert split button
  (`insert_config_button`, `release_dialog::last_viewed_config` made pub(crate)); (7) sketch regions pickable with
  centroid + corner + mid-edge inference (`sketch_wire_result` face_ranges/face_refs, `regen/implicit.rs`
  SketchRegion branch, `inference_candidates` SketchRegion branch, align SketchRegion in `resolve_connector_aligned`;
  test in `sketch_instances_are_wire_occurrences_with_edges`); (8) Change to version… scoped (`RefManager.scope`,
  `open_reference_manager(scope, cx)`), no message, dialog stays open; toolbar button disabled unless
  `has_newer_references`; row icons `list-versioned.svg` / `inserted-version-update` badge.
* REQUESTS 209 (Other documents, live 610–621): `assembly_panels.rs` `OtherDoc` (versions newest first + Start =
  root microversion, changes since the last version), `render_insert_other` (categories, category view, search with
  blue match, document view with Create version… / Version graph / Part Studios | Assemblies / info box / tree with
  expandable Part Studios → parts via `expand_other_studio`), `insert_external(key, only, name)`. `ext_pick` removed.
  The opened document is remembered across Insert sessions (`AsmUi.insert_other_doc`).
* Next: build, GUI-verify everything above (⏳ rows in docs/parity/assembly.md: T10, I13, D16, D17, R3, N13, LA33,
  LA34, L17, A4, LA39), T10 fixture document, sketch start outline size check, psb pick-order check (renderer.rs),
  live layout of the Mate connector panel (D17 note).

### skb run 6 (2026-10-19 evening, takeover after the quota stop)
State: run 5's edits were complete (no half-done code found); everything below builds in the shared tree. App tests
`cargo test -p noshape --bin noshape -- t10_fixture sketch_mode`: 50 pass.
* Verified in the GUI (:103, /tmp/skb/asm): Insert dialog footer `?` (i1 = live 320), LA33 end to end (g1–g10: Origin
  menu, "Origin of new Part Studio", banner split button, empty "Select items to insert", ✓ → "Part 1 <3>" at the Part
  Studio position, floating icon only), psb's pick order (context faces win over the translucent planes, p5–p9).
* New: hovered context face outlined `#ffc685` (live 503) and selected context face amber-outlined outside dialogs
  (`context_ref_overlay`); the see-through ghost faces showed no hover/selection otherwise. LA40.
* New: a context exists for the user only once a feature references it (LA41): `instance_contexts`, `contexts_in`,
  `publish_stale_contexts` skip unreferenced contexts; `add_assembly_context` drops them before adding a new one;
  the Part Studio's Assembly contexts section no longer appears just because a dialog is open in context
  (`render_contexts_section` `pending` checks `params_use_context`).
* Banner ✕ = 11 px btn-close (`os_ui::GLYPH_CLOSE` at 20 px, opacity 0.5), was 7 px. Dialog ✕ in Select items to
  insert, Dynamic suppression, Reference manager, Named positions and the mate dialog = the shared `dialog-cancel`
  icon (#bd3039, as the feature dialogs), was a faint Lucide X.
* Assembly list filter showed two placeholders on top of each other (input placeholder + italic overlay): the input's
  placeholder removed (assembly_behaviors.rs).
* T10 fixture: test `assembly_view::props::tests::t10_fixture_reports_the_newer_version_of_a_referenced_document`
  (assembly_props.rs). With `NOSHAPE_T10_DIR=<dir>` it writes "T10 source.noshape" (demo, V1, V2) and
  "T10 assembly.noshape" (Assembly 1 with one instance of the source's Part Studio pinned at V1) there; open the
  assembly document to check T10 / LA34 (fixture made in /tmp/skb/t10).
* REQUESTS.md: skb (10-19 in-context look, no dialog open).
* Mate dialog pixel pass (live dom/assembly-revolute-dialog-both.txt): 220 px, 29 px header, 194 px type select
  (14 px, 6 px in), query box rows with Onshape's `mate-connector-qlv-item` (15 px) and a 16.67 px "×", the
  reorder-items icon (op 0.5, "Reorder items"), bottom row icons `flip-direction-opposite`, `realign-mate-dialog`,
  `play-dialog` ("Animate mate degree of freedom"), no gaps between the 27 px checkbox rows.
  Icons copied from /root/onshape-research/ui-ref/icons-svg (Onshape's own): play-dialog, mate-connector-qlv-item,
  reorder-items-button new; flip-direction-opposite and realign-mate-dialog REPLACED (they were hand-drawn; the
  feature dialog uses them too — lead: cross-area asset change).
* Mate connector dialog rebuilt (`render_mc_panel`, live dom/dialog-mate-connector.txt,
  assembly-mate-chip-connector-dialog.txt; std `mateConnector` fields without owner entity): OsSelect Origin type,
  Origin entity query box naming the entity ("Face of Part 1 <1>"; "Origin"), Between entity, ☐ Realign (Primary /
  Secondary axis entity), ☐ Move (X/Y/Z translation, "Rotate about X/Y/Z" OsSelect, Rotation angle), flip /
  reorient icons + `?`; implicit: close button only and the field active; from a mate chip it docks right of the
  mate dialog (1 px gap). Model: `AsmMateConnector.move_off`, `ConnectorMove.off` (Move unchecked keeps the values,
  eval ignores them).
* Mate values as Onshape's angular dimension (live 305z): `value_arc()` (assembly_behaviors.rs) = arc in frame_a's XY
  from +X counter-clockwise through the value, arrowheads at both ends pointing out, text centred on the arc at half
  the sweep, laid in the plane along X (flipped to stay readable), the arc broken around it (`assembly_scene::ValueArc`);
  ▶ preview shows whole degrees. `render_value_text` paints the glyphs and a double-click hit box.
* Lead: the clippy warning in part_studio/in_context.rs (redundant closure, my hover outline) is fixed; the other
  app-crate clippy warnings are in history_panel, numbering_prefs, os_select, session, tab_manager (not mine).
* Double-clicking the in-plane mate value can't be exercised under Xvfb/llvmpipe: gpui's X11 backend stamps clicks
  with `Instant::now()` at processing time (vendor/gpui-pre-linux/src/linux/x11/client.rs:1284, 400 ms window), and
  each click's software-rendered frame delays the second one past it (debug print showed click_count 1 twice; the mate
  row's double-click-to-edit fails the same way). The code path is the old one plus a 40 ms deferred focus. Debug
  prints removed.
* State at the end of run 6: tree compiles with no warnings in my files (`cargo check -p noshape`), model `assembly::`
  60/60, app `sketch_mode` + `t10_fixture` 51/51. Integrated in 2352909 except the last edits (mate dialog spacing,
  MC panel, value arc/label, dimension selection, LA41, Reference manager 0-documents line, R3 hover).
* Next: lengths of mate values as in-plane linear dimensions (only the rotation is done; live capture of a slider
  value would help), the Mate connector dialog's preview slider (standalone dialog only, dom/dialog-mate-connector.txt),
  L17 / N13 / I13 / LA39 GUI checks, live answer for REQUESTS "skb (10-19 in-context look, no dialog open)".

### skb run 7 (2026-10-19 night)
* Icons: the copied Onshape SVGs are gone (icons agent redrew most; guard test `crates/app/tests/no_onshape_assets.rs`
  passes). New icons drawn here (original geometry, 20-unit grid): `remove-circle` (folder Delete ⊗), `cfg-last-viewed`,
  `cfg-reset-default` (Insert configured-row menu), `ui-chevron-left`. Never copy from /root/onshape-research/ui-ref.
* Mate values (R11) as Onshape draws them (bundle `calculateMateValues` / `calculateDimensionDisplayData` /
  `calculateAxisAngleDimensionDisplayData`): first connector measured in the second's frame (`onshape_mate_values`;
  typed values mapped back with `solver_values_for`, test `typed_mate_values_round_trip_through_the_solver_convention`);
  angular radius 80 px fixed on first display (`AsmUi.value_radius`); translations as in-plane linear dimensions
  (`assembly_scene::ValueLine`). `VALUE_ARC_SCALE` removed.
* Mate connector dialog: standalone preview slider (feature-dialog look, rest 0.7, fades the connector glyph:
  `McSession.preview`, `h.connectors` 4th field = opacity).
* Bottom-right tools (lead via ui2), new `assembly_mass.rs`: Show analysis tools = the assembly tool list (Curve/surface
  analysis…, Interference detection…, Zebra stripes…, Reflection analysis…); Interference and Zebra stripes work
  (`AsmSceneOptions.zebra / hide_edges`, needs `rebuild_scene`); Curve/surface and Reflection: see below. Display mass and section properties = 440 px dialog; model
  `analysis::assembly_mass_properties` (test `assembly_mass_properties_combine_parts_where_they_sit`). Note for psb:
  `regen::measure::mass_properties` sums the bodies' inertia about their own centres (no parallel-axis term).
* L17: "Add selection to folder…" opens the live "Folder name" dialog (536), default "New folder" (the
  "(2)" in live notes is the member count the row shows; `Assembly::next_folder_name`); folder menu always offers it; Delete uses ⊗.
* I13: configured row ▾ menu right-aligned under the caret with new icons; List inputs are `OsSelect` (live 595).
* LA39: Owned by me navigates folders (top = documents folder's subfolders + watched outside folders, then that
  level's documents; ‹ goes up, home to the categories); placeholder italic; header glyphs #333.
* Double-click under Xvfb still doesn't register (also with `--delay 80`): gpui stamps clicks at processing time and
  llvmpipe frames are slower than the 400 ms window. Logic covered by the conversion unit test.
* REQUESTS: skb (10-19 assembly analysis + mass).
* Analysis tools now complete in the menu: Reflection analysis (`assembly_reflection.rs`, reuses
  `part_studio::reflection::{EnvImageRef, ReflectionSettings}` and `part_studio::environment_maps` — those two modules
  and `EnvImageRef::{key, decode}` were made pub(crate): lead, cross-area visibility change in psb's files) and
  Curve/surface analysis (`assembly_curvature.rs`). Opening one closes the other view tools.
* Insert › Other documents: Part Studio rows show rendered thumbnails of the other document's version
  (`AssemblyView::request_external_thumbs`, assembly_thumbs.rs; an empty Part Studio keeps the generic icon);
  the search placeholder is an italic overlay (`insert_search_box`), so the field itself has no placeholder.
* Mate dialog preview: the instance list's degrees of freedom are the whole assembly's with the new mate
  (eval `solve`: a DOF-only full solve when `only_mate`); before, the one-mate preview showed e.g. a fastened
  instance as "Has degrees of freedom (6)". Model test `a_mate_being_defined_is_solved_alone_until_solve`.
* GUI-verified this round (:103): LA39 (categories, Owned by me, document view with thumbnails, italic
  placeholder), N13 (Insert › Sketches, hovering a sketch region snaps to its centroid with Z on the sketch normal,
  Fastened onto a hole), L17 (folder menu = live 537), I13 (▾ menu, OsSelect inputs).
* In-context look without a dialog (live answer, assembly/700–710): a selected context face gets a translucent amber
  fill (`CONTEXT_SELECTED_FILL` 0xe9b94d at 80 %, blended in linear light, fitted to 706–707) and the amber outline;
  several can be selected; they are measured. `PartStudioView::measure_context` (in_context.rs) adds the shown
  context's bodies (`AssemblyContext::regen_bodies`) when the selection holds a context entity. Surgical edits in psb's
  files: `selection_readout` and the pinned Measure panel (menus.rs) and `measure_overlay` (behaviors.rs, now takes
  `cx`; call in mod.rs) measure through it; before, `measure_overlay` cached an empty result for two context faces
  and blanked the readout. Model test `in_context::tests::context_body_faces_are_measured_alone_and_in_pairs`.
  Note for psb: two planar faces read "Total area" in our readout; live part-studio/81 reads "Perp planes angle".

## skb run 8 (20 Oct): linked assemblies, R11, session

* LA39 done: Insert › Other documents › Assemblies inserts the other document's Assembly as one linked
  subassembly. No new source variant: `InstanceSource::External { key, body: None }` whose key names an Assembly
  tab, so the Reference manager, Change to version / Update all, BOM and key updates work unchanged.
  Model: `AssemblyEvaluator::external_assembly` (cached per key + configuration) configures it and re-points its
  instances (Part / PartStudio / Assembly → External keys of the same pinned version via the new
  `ExternalStudios::sibling`, default None; app `AppExternal::sibling`); `prepare_inner` evaluates it through the
  shared `prepare_subassembly` (factored out of the Assembly arm; flexible, named positions, mates to its parts);
  `InstanceNode.linked` = (name there, content). BOM: `BomSource::ExternalAssembly` row (read-only properties of
  the linked Assembly) with its parts as children; `BomSource::is_assembly`. Test `assemblies_from_other_documents`.
  App: `insert_external_assembly`, instance menu "Open linked document" (bundle `switchToImageOwner`: replaces
  "Switch to …" for other documents' instances, parts too; opens the version on the referenced tab), subassembly
  entries for linked ones (`AssemblyView::linked_assembly`), Change to version… on this document's subassemblies
  makes them version references. Not yet: mirrored parts and inserted sketches inside a linked assembly (no keyed
  form; they read as missing), thumbnails of other documents' Assemblies rows.
* R11 GUI-checked: under Xvfb a double-click registers with `xdotool click --repeat 2 --delay 1` (both presses in
  one gpui batch); 80 ms splits them.
* Live answer "assembly analysis + mass" (live assembly/730–739): the analysis menu has only Interference detection…,
  Zebra stripes…, Reflection analysis…, so Curve/surface analysis and `assembly_curvature.rs` are removed (with
  `Highlights.analysis_lines`). Interference detection is now the live 220 px dialog (`assembly_interference.rs`,
  `AsmUi.interference`), replacing `SidePanel::Interference` / `run_interference`; model
  `analysis::instance_interferences` (reuses `interference::interferences`: volume, box, mesh). Scene:
  `Highlights.red_meshes` (red, over the see-through parts) and `Highlights.boxes` (hovered row, yellow).
* Mass and section properties per live 737–739 (see LA43). Open: Mass Override behaviour (REQUESTS "skb (10-20
  assembly mass override)"), Face tab values.

* Realistic session (Assembly 2 in the demo: insert ×3, fix, Revolute with offset, Fastened pin with flip, Animate,
  exploded view) found and fixed:
  - mate offset direction: the first connector sits at +offset in the second's frame (live 740–748); solver
    `residuals` now `t = rel.origin + R_rel·offset` (was in the first's frame); test in `two_blocks_fastened_face_to_face`.
    Open: default primary alignment of same-orientation edge connectors (Onshape keeps two top hole rims coincident
    with primaryAxisAlignment false; ours aligns Z opposed, so a Fastened pin rim → bore rim flips). REQUESTS
    "skb (10-20 mate primary-axis alignment)" asks for face/edge cases + REST connector frames before changing it.
  - mate dialog Offset section per dom/dialog-mate-fastened-offset.txt (icon rows, Rotate about X ▾, Rotation angle).
  - Exploded views pane rebuilt per the bundle (`explosionItemComponent`): docked 712 px, tree with steps and the
    rollback bar (`ExplodeRollbackDrag`), info box rule, footer button; Auto / Play / the Assembled↔Exploded slider
    removed (not Onshape); Done leaves explode mode; new icons `explode-step-translation`, `explode-step-rotation`
    (original). Open: REQUESTS "skb (10-20 exploded view steps)" (step names, row menus, rolled-back look).
  - Mass Override (live 720–730), see LA43. Surgical edit in the shared `properties_dialog.rs`: the Mass row also
    shows when `computed_mass` is given without a material (assemblies).
  - New-assembly flow, Insert, Fix, mate menus, Animate dialog, mate-row menu: as live (DOM checked).

* qa4 (docs/handoff/qa-round-4.md):
  - Configurations panel: "Filter inputs" is a real field (name filter), the List / Checkbox / Configuration variable
    icons toggle "Show only … inputs", ⋯ = Copy all inputs (shared `config_visibility::copy_inputs`, green toast
    "Inputs copied.") and Collapse / Expand all inputs (2+ inputs), then Visibility conditions… / Input
    propagation… for all inputs, as the Part Studio panel. The input ⋯ (Copy table, Copy input, Rename,
    Visibility conditions…, Input propagation… | Delete, live 200) opens psb's dialogs for that input
    (`open_asm_visibility`, `open_asm_input_propagation` via psb's `open_propagation_dialog`, surgical edits noted in
    part-studio-behaviour.md). Propagation = instances whose source (Part Studio / subassembly, this or another
    document) has an input with the same id and kind; ✓ adds `param::INPUT_PREFIX + id` columns (values
    `Text(option id)`, applied by `apply_configuration` through `InstanceSource::config_mut`; cells are a dropdown
    of the instance's own options; header = its input name). Model test
    `propagated_inputs_set_the_instances_configuration`.
  - Replace instance: no "Select an instance to replace." notice; the toolbar opens the 220 px "Replace instance"
    ✓ ✕ dialog (live 833–836): Instances to replace (active; clicks in the graphics / list add or remove, its picks
    are the selection), the part-icon "Select replacement component" field (title and field red once an instance is
    picked without a replacement), ☐ Replace all instances, ?; Esc cancels, empty clicks don't. The field opens the
    Insert panel in picking mode beside the dialog (live 840–841, see "After v0.4.1" below).
  - Notices: assembly info notices (`message`, non-error) and the configuration toasts (`show_cfg_toast`) fade
    after 4 s like the Part Studio's info / success bubbles, and Esc dismisses an info notice; errors stay until ×.
    The move triad is hidden while the Replace / Interference dialogs take instance picks.
  - Docked panes as live 181 / part-studio/77: fixed 712 px flush right with the strip at the pane's left edge, and
    the graphics canvas (with its fit and bottom-right tools) ends at the strip (`pane_docked`, `AsmUi.dock_w`).

* After v0.4.1 (GO): live answers 760–780 / 800–826 / 840–845:
  - Mate default alignment: `deliver_connector` sets the flip when both connectors of a new mate are picked
    (`MateDialog::set_flip`, Z axes agree ⇔ `flip_primary`); the flip button shows Onshape's flag (`!flip_primary`).
  - Explode step dialog rebuilt (`StepEdit`, `open_step_edit(k, created)`, `step_from_edit` (also the live preview
    through `explode_view`), `accept_step_edit`, `cancel_step_edit`, `step_edit_pick`, `render_step_edit`); model
    `ExplodeStep::{name, lines, line_refs, display_name}`; tree, menus and clicks per live 805–826 (see G5).
  - Replace picker = the Insert panel in `picking_replacement` mode (see T5).
  - GUI-verified after RESUME (20 Oct): mate alignment default (a part mated top corner → top corner keeps its
    orientation, top faces level; connectors whose Z axes oppose stack as before), the Explode step dialog (drag
    opens it, live preview, ✕ removes a new step, ✓ stores it, menu Edit… / Rename / Delete, rename, double
    click opens it titled with the step's name, a typed Distance is stored), the Replace picker (the pick fills
    the field and closes the picker, ✓ replaces and re-points the mates).
  - Active exploded view row (ui2, live 819): blue medium-weight name and the 5 px bar, no selection fill; a
    grey tree guide line under the chevron beside its steps.
  - Standard content tab rebuilt from live 70 / 900–902 (ui2 18:10), `assembly_standard.rs`:
    - Catalogue (`model/assembly/standard.rs`): live's names ("ANSI inch", "Bolts & screws", "Pins & studs"),
      every picker alphabetical, the default = the first of each (`default_component`: ANSI inch › Bolts &
      screws › Countersunk bolts › Countersunk bolt, #10-24, 0.25). ANSI sizes carry the thread ("1/4-20"),
      ANSI lengths are live's 23 decimals (`INCH_LENGTHS`, bare numbers in inches, `Component::length_m`).
      New ANSI Countersunk bolt (20 UNC/UNF sizes) with the Head slot option (`countersunkSlotted` script
      feature, a slot instead of the socket). Spec fields `head_slot`, `material`, `finish` (`MATERIALS` 37,
      `FINISHES` 7, live's lists); Thread length (2D + 1/4, never longer than the bolt) for components with
      `thread_length`. `description()` = live's "Countersunk Bolt #10-24 x 0.25 Stainless Steel " (also the
      BOM Description column). Old documents load through `SpecFile` (ANSI → ANSI inch, "1/4" → "1/4-20",
      "1 1/4 in" → "1.25").
    - Panel: hierarchy group 105 px and parameter group 114 px, each scrolling with live's thin always-visible
      scrollbar (▲ thumb ▼, `std_scrollbar`); 12 px labels in a 90 px column; 181 / 146 px selects with a
      #d7d7d7 underline (`std_select`); auto-size 26 px icon button (rich tooltip "Auto-size fastener");
      Part number (read-only #eee, pencil 32×24 with live's tooltip; what it opens is asked live) and
      Description; our own line drawing of the component (`paint_fastener`, end + side view, 242×105); the
      141×34 group Insert | Insert closest to selection | Insert furthest from selection with live's rich
      tooltips, the two stack buttons at 40 % until a planar face with holes is picked. No "Inserted: N" on
      this tab (live 70).
    - Picks: while the tab is shown, graphics clicks pre-select faces / edges (`StdPick`, drawn selected);
      Insert fastens one part in each of their holes; closest puts it against the picked face (what was there
      moves onto its top face), furthest at the outer end of the stack (under a bolt head). In the Replace
      picker, Insert picks the component as the replacement.
    - Icons (ours): `auto-size` redrawn as live's cylinder with a dimension, `stack-closest`,
      `stack-furthest`, `edit-pencil`.
    - Asked live (REQUESTS "skb (10-20 standard content…)"): the pencil's dialog, the inserted instance's
      name, closest / furthest with an existing stack, the Edit standard content dialog, Head slot off, the
      rows of nuts.
  - Fixes: the right strip's tooltip stayed at the pointer after the pane docked (the strip now takes a new id
    when it moves, `rail` / `rail-docked`); torus (fillet ring) and revolved faces offered their centroid with
    Z sideways, now the centers of their circular edges round the axis (`inference_candidates`, test
    `torus_face_inferences_lie_on_the_axis`); the view cube and canvas tools were drawn over the dialogs (the
    replacement picker beside Replace instance reached it), now under them.
  - Replace instance (live 840–843): the picked replacement is previewed in place before ✓ (`effective()`
    applies it; `AsmUi::eval_requested` re-evaluates from `render`, which has the Window), mates that no
    longer resolve go red; ✓ matches mate references against `ReplaceSession::before` (the result from
    before the preview). Picker filters: live's Parts | Sketches | Surfaces | Flat patterns (our own
    `flat-pattern` icon). An instance already named after the replacement keeps its name.
  - Draggable panels (ui's request): Replace instance (its picker moves with it) and Interference detection
    take ui's shared panel drag (`ui::panel_title` keys `asm-replace` / `asm-interference`, `nudged_panel`):
    they follow the pointer with the grab cursor. Moved assembly panels reopen where they were left, in later
    sessions too and shared with the Part Studio (ui's `PanelOffsets`, `panel_positions.json`, `ps-` / `asm-`
    prefixes stripped; verified across sessions). Interference detection opens at live's default place (600×80
    rect snapped to the graphics area's right edge, left = W − 600, top = H − 401). Open (ui's shared code,
    spec in theme-layout.md "From skb"): live's 10 px edge / panel snapping and keeping snapped edges on resize.
  - Assembly S shortcut toolbar (ui2's handover): `AsmToolbarPrefs::shortcut_tools` (assembly_toolbar.json,
    defaults `DEFAULT_SHORTCUT_TOOLS` until live answers), `run_toolbar_tool(id)`, `shortcut_catalog()`;
    `render_mini_toolbar` in live's look (rows of five 30 px buttons at cursor +10/−30, #ccc border, 95 %
    white). Preferences › Shortcut toolbars › Assembly edits it (surgical in ui2's preferences_window.rs,
    noted in theme-studios.md). Single-line dialog inputs use `os_ui::input_h` (live 26 px).
  - Preferences › Assembly settings › Display instance list properties now drives the instance list at
    open, and the list menu's Show / Hide instance list properties saves it (`ListPrefs`). Animate mate:
    Current value stays empty until ▶ (live 53; `AnimState::played`). Single-line dialog inputs: 26 px
    via `os_ui::input_h` (live `input.os-param-text` 26 px).
  - Live answers 10-20 applied (GUI-verified): Animate steps/format; S toolbar list/defaults/look; standard
    content naming (`StandardSpec::part_name` = "Countersunk Bolt #10-24 x 0.25", `description` adds the
    material), stacking per live (`std_insert`, `stack_end`, `held`), Part number pencil edit mode (user-wide
    `standard_content_properties.json`, `StdProps`), Edit standard content frame; Mass Face tab
    (`analysis::face_section_properties`, `mass_pick_face`). Replace naming: a part of another name takes its
    name (live 830–841), same name keeps it — as implemented. Asked live 10-21: the standard content option
    lists (Series, other classes, DIN/ISO lists).
  - Live 10-21 option lists (dom/skb-sc-lists.json) applied where our catalogue can build the geometry:
    Series Narrow / Wide (ASME B18.22.1 Wide dims, `ANSI_WASHER_WIDE`, `StandardSpec::series`), Drive type
    (socket heads "Hex"), Bearing face (ANSI nuts Chamfered; ISO Washer face / Chamfered,
    `StandardSpec::bearing_face`), Thread length on bolts and screws (inch 2D + 1/4, metric 2d + 6), the
    first size and its first length as defaults (the ANSI Type A washer 3/8), live's naming (standard code
    dropped, metric pitch "M6x1.00", the Series: "Type A flat washer 3/8 Narrow"). Mass dialog reference
    frame: "Mate connector for reference frame" takes a connector pick and the Part tab's inertia is about
    its origin in its axes (help; `analysis::inertia_about_frame`, test `inertia_moves_to_a_reference_frame`).
  - After GO (v0.4.3): metric tables take live's sizes with published dimensions — ISO 4762 (M1.6…M64,
    21), DIN 912 (19, no M14/M18), ISO 4017/4014 hex (21), ISO 4032 nuts (24), DIN EN 24032 (19), ISO 7089
    washers (19), ISO 8734 / DIN pins (1…20); `LIVE_LENGTHS` holds live's exact Length lists for the 29
    (component, size) pairs captured; metric lengths are bare numbers like live ("2.5", "16"; documents with
    "20 mm" load as "20"). Defaults: the first size and its first length (ISO 4762 → M1.6 x 2.5). The Mass
    reference field's chip reads "Mate connector of <instance>" (like the mate dialog).
  - Still open: live's ANSI sizes beyond ours (washers #0…3, socket head #0-80…3/4-16, hex cap screws to 3-12,
    …) and components / classes we have no dimensions for, per-size Length lists for uncaptured sizes, the
    Face tab with a reference connector (unverified), live's 10 px panel snapping (ui's shared code; spec in theme-layout.md
    "From skb"), the stack buttons' "face selected before opening Insert" (our assembly has no face
    pre-selection outside dialogs), DIN / ISO / NAS / PEM / SAE classes beyond ours (catalogue subset).

### From psb (20 Oct): visibility conditions in assemblies (surgical edits in your files)

* Evaluation: `AsmConfiguration::value_of` → `inputs_only().value_of` → `Configuration::effective`, so
  `apply_configuration` / `configured` already honour the conditions (a hidden input takes its default; a
  hidden chosen option takes the default or the first shown option). Test
  `configuration::visibility_tests::assembly_configuration_respects_visibility`.
* UI: three loops now go through `Configuration::visibility(values)` and skip hidden inputs and options,
  reading the effective values: the left Configurations box (assembly_config.rs, after the header `split`),
  the Insert dialog's configured-row inputs (`render_insert_config`, assembly_behaviors.rs) and the
  instance's Change configuration panel (`render_config_panel`, assembly_panels.rs). The stored
  values are left as chosen (Onshape keeps them; evaluation applies the conditions).
* (psb, lead's measure fixes) `assembly::analysis::measure_entities` now ends in
  `regen::measure::measure_shapes` (shared Onshape readout rules: "Perp planes angle" for two
  perpendicular faces as live 707, "Min dist" instead of "Distance", "Area"/"Length" totals). Your
  `assembly_mass_properties` already had the parallel-axis term; the Part Studio one now does too.
* (psb, user bug) Hover-revealed eyes on instance, folder and mate rows now use `ui::hover_reveal`
  (opacity, not `invisible()`): an invisible element has no mouse listeners in gpui, so quick clicks fell
  through to the row. Please use `hover_reveal` for new hover-only controls.

## From kern (2026-10-20): proposal, a BOM tab ("BOM : <assembly>") — please reply here before I touch the document model
Onshape's native assembly import (live G38/G38f, behaviours.md "G38f") creates a **Bill of materials tab** next to the
Assembly and its "<name> parts" Part Studio: element type BILLOFMATERIALS, named "BOM : <assembly>", visible columns
**Item · Quantity · Part number · Description**, rows from the assembly ("Show Assembly and components"). kern's import
(`model::native_assembly::add_native_assembly`, app `import_dialog.rs`) makes the Assembly and Part Studio today.
Proposal: `TabContent::BillOfMaterials { assembly: TabId }` (serde-compatible addition, no other model change), its
rows from `assembly::bom`, and a tab view in the app (owner: asb's BOM table code). kern would add the variant and the
import's tab only; the view is yours. Live capture of the tab's UI is requested (REQUESTS.md "kern G38g").

## From psb (20 Oct): Part Studio configured properties and the assembly BOM
psb added Configured properties: the model is `noshape_model::configured_properties` and the tables live in
`PartStudio.property_tables`. Live (psb 10-20 q, part-studio/626–627) shows that **editing a property in an assembly
BOM, for an instance whose Part Studio part has a configured-properties table containing that property, sets
it for the instance's configuration and adds that row when it's missing**. The model call for it is
`configured_properties::set_for_configuration(ps, &part, "Part number", &instance_config, value)`. It
returns false when the part has no table or no such column; then the BOM should write the part's own
property as today. For reading, `configured_properties::configured_value(ps, &part, prop, &config)` gives the
configured value (None = use the part's own). Regeneration already renames parts by a configured Name and
colours them by a configured Appearance. psb did not touch your files.

## skb run 9 (21 Oct, takeover after the account move) — READ THIS FIRST

State: the shared tree compiled with my files at 07:45 (other agents' files — drawing_tools.rs, custom/host/query.rs,
a CodeRow file — were mid-edit at times; not mine). Nothing of mine is half-done in the tree; staging copies live in
/root/skb-scratch/next/ (catalogue.rs, mod.rs, fasteners.fs, assembly_standard.rs, data/) and must be kept in sync
with the tree when editing there.

### Standard content catalogue rebuilt as data (task "close the catalogue")
* `crates/model/src/assembly/standard/` = `mod.rs` (spec, part studio, holes), `catalogue.rs` (types, loads
  `data/{ansi-inch,din,iso,nas,pem,sae}.json`), `fasteners.fs` (geometry), `data/*.json` (GENERATED, don't hand-edit).
* Generator: `/root/skb-scratch/gen/build_catalogue.py <live dom> <dims dir> <out> --report [--fallback]` with
  `/root/skb-scratch/gen/mapping.py` (per live component: family, forms, table(s), column → script symbol; standard
  formulas only: thread minor diameter for the chamfered end, a dome's sphere radius from its diameter and height,
  90° cone points). Then copy `next/data/*.json` into the tree. `--fallback` adds components live hasn't crawled yet
  from dom/skb-sc-lists.json, with ONLY the Length lists live captured (other sizes are left out).
* Dimension tables (published values, sources inside each file): /root/skb-scratch/dims/*.json (~200), schema
  dims/README-schema.md. Each component's `source` names its standard, edition and pages.
* Families (`Shape`): Screw (head none/hex/square/round[rf: corner or dome]/countersunk/raised/T/eye/lifting eye/wing;
  drive none/hex socket/slot/cross H/cross Z/hexalobular; neck square/ribbed/fin/shoulder; point chamfer/cone/cup/dog/
  cone tip/round/flat), Nut (hex/square/slotted/castle/cap/insert/weld, flange), Washer (plain/chamfered/split/teeth/
  conical/square), Pin (parallel/dowel/taper/slotted/coiled/cotter/clevis/stud/drive stud/T-head), Key, RetainingRing,
  ORing. Component JSON: `form` (integers), params with `forms` overrides per option (Head slot, Head type),
  per-size `values` (selects that depend on the size), `threads` (live Thread length list per length).
* Live crawl: REQUESTS "skb (10-21 standard content FULL crawl)" + my 07:40 addendum (Thread length for every
  (size, length) — live's lists are NOT a rule; per-size selects; material names). Live is re-crawling into
  dom/skb-sc-full-<Std>.json; rerun the generator as files grow. Geometry request (B-rep of one size per family)
  is open; use it to correct head/drive/point details.
* Known gaps (generator --report): ASME #10 countersunk bolt, DIN 580 M72–M100, ASME button 7/16 & 3/4, SHCS #12 &
  9/16, machine dowel 7/32 (not in B18.8.2: needs live geometry), hex machine screw "Type I" recess dims, DIN 7050/7051
  recess penetration, Hex jam nut #6–#12 (no ASME table), O-ring material names (live). Not yet crawled: ANSI Keys,
  Retaining rings, more Nuts/Washers classes; NAS; PEM®.
* Test `standard_content_parts_regenerate_with_mate_connectors` regenerates every component: run it after each
  data change (it couldn't compile at 07:50 because of another agent's lib-test error).

### Selection model (task 1) — coded by run 8, verify in GUI
Entity selection outside dialogs (`AsmUi.entity_sel`, `select_entity`, `clear_selection`, readout
`selection_readout`), double click clears, window vs crossing box, Insert takes the pre-selection
(`std_take_selection`). Still to check against live: hover = orange boundary edges (no fill), vertex dots, row-click
whole-instance lighter amber.

### Mate flip button (icons2's report)
Live (dom/dialog-mate-fastened-empty/-filled, mate-hud.txt): the flip is never drawn pressed; its icon shows the
flag: `flip-direction` while Onshape's flip is off (our `flip_primary` true = Z axes agree), `flip-direction-opposite`
when on. Dialog and HUD now use `assembly_mate_dialog::flip_icon_name`.

### First insert lands at the origin (lead, from qa4)
Help ("Insert parts and assemblies") + live behaviours "Insert dialog": inserts start with the Part Studio origin on
the assembly origin; the same part again at a slight offset; once the cursor enters the graphics the instance
follows it and lands at the click. Live: the first instance into an empty assembly lands at the origin. Now
`noshape_ops::assembly::inserted_into_empty` decides it, and `start_placing` skips following the cursor for that
insert (test `first_insert_lands_at_the_origin_later_ones_follow_the_cursor` in crates/ops). Asked live (REQUESTS
"skb (10-21 insert placement)") for the exact offset and whether the first instance is selected.

### Lead's integration try 8: the two standard content tests (fixed)
* `concatenate(a, b)` doesn't exist in our FeatureScript std → every shank failed ("Function concatenate with 2
  argument(s) not found"); fasteners.fs uses its own `join`.
* Sizes live hasn't crawled had empty labels: the generator now takes live's description, else the interim
  catalogue's label (run 8, live's naming), else a crawled sibling's label with this size's token (metric pitch with
  two decimals). Series rows (Type A washer) end with their own series.
* ANSI Countersunk bolt #10-24 / #10-32 (live's default component): ASME B18.5 starts at 1/4, so the #10 sizes take the
  82° slotted flat head of ASME B18.6.3 (recorded in the component's `source`). Confirm with the live geometry request.
* To debug catalogue tests quickly, run the lead's already-built test binary (/root/target-lead/debug/deps/noshape_model-*)
  from /root/noshape-lead with the test name as the filter; it prints which component failed and why.
* fasteners.fs gotchas: `box` is a FeatureScript keyword (helper is `cuboid`); optional lengths must use the script's
  `LENGTH_0` bounds (NONNEGATIVE_ZERO_INCLUSIVE_LENGTH_BOUNDS defaults to 25 mm, which put a 25 mm flange/collar/round
  on every part); never reuse a body query after a boolean (`solidsOf` / `uniteAll`); in `screw` work on
  `var def = definition`. The regeneration test lists every failing component with its feature parameters.

### Exploded view / named position banner (icons2 via lead)
The banner toolbar had no width of its own, so with only Undo / Redo in flow it shrank to ~70 px: the banner was
clipped at the far left and covered Undo / Redo. `render_toolbar` now gives that row `w_full()`; the banner centres
in the toolbar row like live 62 (x 868–1090 of 40–1919), Undo / Redo stay at the left. Same fix for named positions.

### Dialog ✓ / ✕ tooltips removed (lead: live has none)
All "Accept (Enter)" / "Cancel (Esc)" / "Close (Esc)" / "Close" / "Accept" tooltips on the assembly dialogs' ✓ / ✕ are
gone (mate, mass, reflection, named positions, properties, panels, standard content edit, group, interference, the
replacement picker). The mate heads-up toolbar keeps its tooltips (live mate-hud.txt: "Commit current mate and start a
new mate", "Cancel").

### State 12:00 (21 Oct)
`cargo test -p noshape-model --lib`: 293 passed, 0 failed (standard content tests green with the new catalogue).
Kernel: surgical mesh-deflection clamp in cpp/common.cpp (noted in kernel-robustness.md).
Clippy (`--locked -p noshape-ops -p noshape --all-targets -D warnings`, ~12:40): nothing in my files; remaining errors are
other agents' (drawing_drawprops.rs, smoke.rs, variable_config.rs; earlier regen/features/pattern.rs).

### GUI check 13:15 (21 Oct) — findings
* Verified: explode banner centred with Undo/Redo kept (fix above); flip button never pressed, icon swaps on click;
  no tooltip on dialog ✓; first insert into an empty assembly stays at the origin after moving into the graphics and
  clicking.
* **Assembly picking is dead** (no hover, clicks pick nothing, right-click on a face shows the empty-space menu) with
  qa4's gpui-pre patch (nested cached views not re-rendered, R7 in qa-round-4.md). With `NOSHAPE_NESTED_VIEW_REFRESH=1`
  picking works again. Reported to the lead (qa4 is paused); not fixed by me (qa4's vendor patch + workspace.rs).
* Fixed (mine): a click on geometry arrives as ToolDown/ToolUp (the left button is captured to drag instances) and
  ToolUp selected the whole instance; both click paths now go through `click_select` (live: face/edge/vertex only,
  additive, double click clears, empty click clears).
* Verified 13:45 (with NOSHAPE_NESTED_VIEW_REFRESH=1): a face click selects only that face (amber, row selected,
  triad at the click, "Area: 4469.713 mm²"); an empty click clears; hovering an edge turns it orange.
  Known gap: a click right after a jump of the pointer uses the previous hover (ToolUp takes `self.hover`).

### qa5 assembly findings (lead, 21 Oct afternoon) — docs/parity/workflow-assembly.md "Findings"
Live reference: behaviours.md "qa5 assembly" (dialogs, placements) and the Replicate / pattern help pages.
* A7 Replicate: the model now has `ReplicateScope` (Match faces in parts / in features / individual faces; edges on
  plane / on face / individual edges) and `scope_refs` ("Parts to find match in"); `analysis::scoped_candidates`.
  Older documents (no scope) keep the signature search, but an edge seed now matches through its hole's face and
  takes the edge at the same end (was: both ends of every hole = duplicates). Dialog: scope select (face or edge
  options by the seed mate's entity), "Parts to find match in" (red / title red while empty; picks parts for
  "faces in parts", else faces / edges; `PatternSession.scope*`, pick slot 2 in Entity mode).
  Tests: `replicate_from_a_hole_edge_matches_holes_once` (legacy + scoped).
* A9: `MateConnectorRef::linear_direction` (Z along a picked edge); the dialog applies it to linear directions.
  Test `linear_pattern_along_an_edge`.
* A4: test `slider_flip_primary_turns_the_part_over` (model); app flip path to verify in the GUI.
* A6: the dialogs were already feature dialogs in the working tree (qa5 had an older build); now live's defaults
  (25 mm × 2, 360 deg × 2, Instances focused) and ☐ Centered (`centered` on Linear / Circular, copies indexed with
  `CENTERED_BACK`; help: N each way). Test `centered_linear_pattern_goes_both_ways`.
* A8: `effective()` applies the open pattern dialog's pattern (preview copies); every dialog change requests an eval.
  Not yet: the pattern's rows in the instance list while the dialog is open (live shows them at once).
* A1: the first insert into an empty assembly is selected. Zoom-to-fit crop: to check in the GUI.
* A2: the instance a mate preview moves is drawn amber (`ops::assembly::mate_preview_moving`, test).
* A3: new mates are hidden (`ops::assembly::commit_mate`, test); tooltip header "<name> (Hidden)" / "has error: …".
* A5: inference look: hovered face outlined (`Highlights.hover_outline`, `AsmPickMap::boundary_ids`), white dots,
  live's khaki notched connector glyph (`push_connector_glyph`); no face fill in inference mode.
* A11: typing into a selected BOM cell starts editing it (keystroke interceptor, `bom_panel::typed_text` + test).
* C7: "Change configuration" dialog in live's layout (300×587, title, doc header, studio row, inputs, Generate, parts).
* The api agent removed crates/ops/src/ops/assembly/ (patterns no longer in the agent API); my edge-direction edit
  there went with it — re-add `.linear_direction()` if patterns come back to the API.
* Queued: ASME B18.6.4 tapping screws (10 components; Head type Slot/Type I/IA/VI, Tip type A…T need per-size option
  values in the catalogue; tables in dims/ASME_B18.6.4_*), askew head bolt (dims/ASME_B18.2.1_askew_head_bolt).

### qa5 pass 3 (after the freeze, 21 Oct evening) — on top of 5d28447
* A12: a pattern's generated mates are listed under its row as a collapsed "Mate features (N)"
  folder (`Assembly::pattern_mates`); the top-level "Mate features (N)" counts
  `Assembly::listed_mate_count` (live qa5/asm-056). The pattern row's instance count is gone (live has
  none). Test: assertions in `replicate_on_matching_holes`.
* A3: hidden mate rows `#999` with icons at 0.5 (`feature_row_color`, test
  `hidden_mate_rows_are_grey` in assembly_view.rs).
* A14: the pattern dialogs' red hint line is removed (an incomplete pattern only greys ✓ / reddens the title).
* UNDO1: `DocModel::edit_tab_deferred` / `flush_pending` (doc_model.rs, `PendingEdit`, test
  `deferred_edits_are_one_undo_step`): an Insert session (inserts, paste, the drop) is one undo
  step, committed when the Insert dialog closes, by the next edit, undo/redo, or on drop of the
  model. `AssemblyView::edit_insert` / `end_insert_step`. Undo button: `DocModel::can_undo/can_redo`.
  The Insert dialog's "undo last insert" removes the instance from the pending step.
* A11: BOM type-to-edit put the caret after the typed key (`set_selected_range`, the input's
  select-on-focus selected the "P" and the next key replaced it); `revert_on_escape(false)` so Esc
  drops the edit; Delete clears the selected cell (`bom_clear_selected`).
* A1: zoom to fit uses `AssemblyResult::bounds()` (the scene's bounds lagged the first insert) and
  `fit_inset_left` 244 px while a left dialog is open (`AsmUi::left_open`, set at render).
* A13: the relation dialog is a feature dialog as live (behaviours.md "Assembly relation dialogs"):
  "<Type> 1" ✓ ✕, type select (Gear, Rack and pinion, Screw, Linear), Mate(s) query with chips
  "⟳ - z - Revolute 1" (`noshape_ops::assembly::relation_mate_label`), "Relation ratio 1" /
  "Distance per revolution 25 mm" (`relation_default_ratio`, `relation_value_label`), ☐ Reverse
  direction, ?; the new row ("Gear 1", bold, selected) is in the list while open. Picks toggle
  (screw: one mate). Test `relation_dialog_matches_live` (ops). Not done: the chips' × (shared
  `os_ui::query_row` has none), the DOF manipulators in the graphics.
* A4: extra model case (top face onto top face, the qa5 setup) in `slider_flip_primary_turns_the_part_over`.
* SK3 (mine: sketch leftovers): a dimension / constraint to model geometry makes a reference-only
  entity (`noshape_sketch::Entity::reference_only`, `Sketch::set_reference_only`,
  set in `noshape_ops::sketch_use::external_entity` when `construction`): solved, not drawn
  (`regen/display.rs`), no use glyph, pruned with its last constraint (`prune_reference_only` in
  `prune_exprs`). Use on such geometry makes it normal used geometry. Test
  `model_edge_references_are_hidden_and_go_with_their_dimension` (scene).
* ✓/✕ tooltips: none left in assembly dialogs (the mate HUD keeps "Commit current mate and start a
  new mate" / "Cancel": live mate-hud.txt).

### qa5 pass 7 (lead, 21 Oct night)
* A4 (root cause found in the GUI): the flip did turn the block over, but the Slider's free Z
  value stayed where the solve from the stored placement left it (the block landed under the
  plate). A new mate's preview now holds every free value at zero
  (`eval::new_mate_holds` → `SolveOptions::hold_values` in `effective()`), so the connectors meet
  as in Onshape. Test: end of `slider_flip_primary_turns_the_part_over`.
* A18: `ops::assembly::insert_part` puts a repeat of the same part at n × bbox/8 (live: 5, 2.5,
  1.25 for a 40×20×10 block), another part at the origin; the instance only follows the cursor
  once the pointer moves over the graphics (`AsmUi::place_moved`), so ✓ right away keeps it there.
* A15 (my part; ui2 does the face/edge colours in assembly_scene): no manipulator triad on a
  just-inserted instance (`AsmUi::inserted_selection`), root row highlighted only after a click
  on it (`AsmUi::root_selected`).
* CA1: configured Part Studio parts use `icons/part-configured.svg` (own drawing: part + blue
  grid; icons2 may redraw it). CA2: "Select instances with same configuration", "… with same part
  and same configuration" (`ops::assembly::same_configuration`, test) after Change
  configuration…; "Release…" after Export… in the list menu (`release_instances`).
* SK1 leftover: regen marks a sketch whose solve isn't `Solved` as Error (regen/mod.rs), so the
  dialog title and the feature row turn red. Test `an_unsolvable_sketch_is_in_error` (features_c).
* A16 waits for live. "Accept (Enter)" tooltips: no such string exists in the tree (grep).
* CA5: the configuration button's ▾ › Reset to default / Set to last viewed also set an open
  Change configuration panel's inputs (`set_config_panel_values`, Generate enabled on a change).
* CA6: BOM part rows key on `bom::canonical_config` (entries equal to the Part Studio's defaults,
  numerically for variable inputs, and excluded inputs dropped), so instances in the same
  configuration share a row (assembly panel and drawing table). Test
  `instances_in_the_same_configuration_share_a_row` (bom.rs). Live confirmation pending.
* CA2: root-row menu Release… after Export… (`release_dialog::tab_items`).
* Live confirmed (qa5 pass 7): CA5 and CA6 as built. A16: the toolbar "solving…" label is removed
  (live never shows one). BOM cell edits are committed not undoable
  (`DocModel::edit_document_not_undoable` in `set_bom_cell`), as live's Ctrl+Z leaves them.
* Fixed the integration blocker assembly_panels.rs E0502 (`ConfigSession::focus` read before
  `insert_config_button`).

### Note from ui (21 Oct ~20:30)
`assembly_view.rs click_select`: an empty click no longer clears the selection while Mass properties is open
(its "Parts to measure" chips are the selection; live: empty clicks keep dialog chips). Also Change configuration
(`assembly_panels.rs`) now commits values with the unit and has feature-dialog keys (qa5 CA4).

## Moved to the shared ops (api, 2026-10-21 21:20; lead's call, no freeze)
`assembly_panels.rs`: the pattern dialog's defaults (`pattern_defaults`: 25 mm / 360 deg / 2), Replicate's default
scope (`replicate_on_edge` -> `default_replicate_scope`), the seed mate (`seed_mate`) and the pattern commit
(`pattern_description` + `commit_pattern`); `assembly_interference.rs`: `interference_rows`. They now call
`noshape_ops::assembly::*`, the same code moved verbatim, so the CLI/MCP patterns, Replicate and interference
behave exactly as the dialogs. Behaviour unchanged. Change these rules in `crates/ops/src/assembly.rs`.
Still to switch (needs a quiet `assembly_view.rs`): Suppress mate (`toggle_mate_suppressed`) and the Group commit
(`commit_group`, `GROUP_TOO_FEW`), script /root/api-scratch/patch_asm_switch.py assembly_view.rs.

### 22 Oct (after the freeze): DER8, MT*, A18/A19, sketch Text, catalogue
* DER8: `AssemblyEvaluator::set_studio_options` (model) + `noshape_ops::derived::assembly_evaluator(cache)` /
  `studio_options`: Part Studios with Derived (importDerived) or custom features regenerate with the document's
  sources (their versions in the cache key). Used by the assembly view, assembly drawings (drawing_view, ops
  drawing), version properties, in-context. Test `derived_parts_are_insertable`. (Build fix: read
  `studio_cache` before `doc` moves into the view.)
* DER11: out-of-date context = one icon `icons/incontext-arrow-out-of-date.svg` (own drawing, measured on live
  519 zoom), one tooltip "Update available for <context>".
* MT9: Replace instance keeps mate references (no nearest-geometry remap; `analysis::remap_replaced` kept but
  unused by the app/agent API) → unrelated parts fail with live's "Mate cannot resolve mate connectors."
* MT6: `Mate::new` Fastened → rotation_axis 0 (Rotate about X). Test.
* MT3/MT4: chip tooltip "<label> (Click to edit)"; the chip the connector panel edits is bold
  (`MateDialog::editing_field`, synced at render); related-highlight rows (#ecf0f4 + #90cef1 line, live 305):
  `related_instances` (selected mate's instances, edited connector's instance) and the Mate features row.
* MT7 (part): a selected mate highlights only its mated entities (amber, `selected_entities`), triad letters
  X/Y/Z (`render_triad_letters`). Open: the dashed circle's radius rule and the 0° look (live request 10-22 00:40).
* MT8: connector Move rows = X↘ Y↗ Z↑ icon rows; preview slider removed (qa5/asm-043), ? on the flip row.
* A18: Insert catalogue bounds are the exact (optimal) boxes. A19: pending red "<Pattern> N" row + badge
  (`pending_pattern_row`), root-row error badge, red "Parts to find match in" border.
* Sketch Text dialog (sketch leftovers): live's feature-dialog layout in the sketch dialog's place
  (dom/dialog-sketch-text); Convert to expression on the edit box's context menu. Open: fonts (live default
  Open Sans; we embed Liberation only) — font list requested from live, shipping fonts needs the lead.
* Catalogue: generator now writes 195 components but the regenerated data FAILED the standard-content tests →
  tree data restored to HEAD; `NOSHAPE_STD_DATA=<dir>` (tests only) runs the tests on a scratch catalogue.
  Conditional dims `"<dim>@<param>=<option>"` (standard/mod.rs). Tapping screws staged in scratch
  (gen/tapping.py, tap/fasteners.fs POINT_BT); blocked on tips C–T, Type III/VI recesses, indent heads (live
  requests filed). Plan: /root/skb-scratch/PLAN-catalogue.md.

### 22 Oct morning: tapping screws, MT7 0°, SK5, CH1 (worktree /root/wt/skb from 07:00)
* Tapping screws: catalogue 197 components; the 2 trim heads (Flat / Oval countersunk trim, Type I/IA only) are
  in with all 11 tips (fasteners.fs POINT_BT 7, POINT_TIP_D/G/F 8–10; BT sphere R = L − (3L² − D²)/4L). Test
  `tapping_screw_tips_regenerate` (#8-#6 trim). The other 8 tapping components wait for Type III/VI recess shapes
  (live request 05:30) and the indent heads; `standard/mod.rs`: `param()` is row-aware, `fix_length()`.
* MT7: the mate angle label trims zeros ("0°", `mate_angle_label`) and sits at the X-axis end at 0°.
* SK5 (sketch solver): `SolveOptions::light` / `still` (first an exact pass with everything else held, then a
  weighted fallback), `Sketch::dimension_edit_options(cid)`; the dimension edit path uses it. Fillet radius
  rule from live (REST, qa5 sketch tools): the arc keeps its CCW end point (that line keeps its length), the
  line at its start moves. Test `fillet_radius_change_moves_like_onshape` (30.089×22.065, R5→R3 = live's
  edges). Open: the default fillet radius (live R20 earlier, R5 now; request 10-22 07:05); live also keeps a
  `visualSharp` point after every fillet (ours only when something refers to the corner).
* CH1: the pending sketch chamfer draws the light-blue chamfer line, both distance dimensions ("5", 50 px
  outside the edges) and the hollow drag arrow from the first chamfer end along its edge (drag sets both
  distances); Enter commits and opens distance 1's box, accepting it opens distance 2's
  (`value_box_next`/`open_next_value_box`). Arrow look is icons2's.
* EXV4: Done in an exploded view keeps its row expanded (`ui.explode_open`) and clears the selection and triad
  (`activate_explode(None)`). EXV5: explode drags round like live (`explode_drag_round`: whole degrees, 0.1 mm /
  0.01 in) and the step dialog writes angles as "55 deg". NP1: no Delete on the mate row menu while editing a
  named position.
* Tapping screws, 10-22 morning (b): catalogue 200 components. **Pan** (head as live's configuration strings:
  Ø = A_max, height = recessed H_max for every head type, R_min edge fillet → `re`, crown sphere 1.65·D → `rf`;
  fasteners.fs `panCrown`), **Fillister** and **Oval countersunk** are complete with **Type III** (square + 140°
  cone, `DRIVE_SQUARE_III`) and **Type VI** (12-point closed fit spline + cone, `DRIVE_TYPE_VI`) from live's
  FSRules columns (dom/skb-sc-recess-III-VI.json). Closed FS fit splines are periodic
  (`Sketch::add_closed_fit_spline`, `Nurbs::periodic_end_derivative`), and a region loop of one closed spline is
  one edge (regen/eval.rs `region_profile`), so the lobe wall is one face as live's. Pan #8 Slot = live's 10 faces;
  III/VI = live minus the 0.002 in mouth fillet, which OCCT fails at 0.0508 mm (`try silent`; ignored test
  `pan_head_tapping_screw_mouth_fillets` = kern follow-up). Still out: Flat countersunk (no FSRules III/VI data
  yet, live request 07:45), Flat undercut, Oval undercut, Hex, Hex washer (Indent / Slot and indent heads).
* Shared target dirs: worktrees in one group overwrite each other's workspace crates (cargo's hash ignores the
  worktree path), so a build can link another worktree's stale crate. Touch your changed sources before a check.

### 22 Oct midday: SK5 in the app, RECT1, TRIM1, SLOT1, POLY1, CH2
* SK5 in the app: the sketch feature's regeneration re-applies dimension expressions; a single changed value
  now solves with `Sketch::dimension_edit_options` (regen/features/sketch.rs), so a typed fillet radius moves only
  the fillet and the far edge. Checked in the app (CLI-made 30.089 × 22.065 rectangle, fillet tool, typed value):
  R3 → 28.089 / 19.065 / 25.089 / 22.065, R8 → 33.089 / 14.065 / 25.089 / 22.065 = live. Tests
  `sketch_fillet_regen.rs`, scene `fillet_radius_edit_in_the_editor_moves_like_onshape`.
* RECT1: `corner_rectangle` = live's Perpendicular (top, left), Parallel ×2, Horizontal (top).
* TRIM1: `Sketch::trim_span`; with Trim the hovered curve isn't highlighted whole, the overlay draws only the piece
  that would go (thick light orange + end dots). Checked in the app.
* SLOT1: the pending slot shows its join points and the Ø width dimension ("Ø20", `slot_dimension`).
* POLY1: the polygon preview's circle is dash-dot; the side spinner sits outside the edge after the clicked vertex
  (◀ n ▶ laid out along the edge; the glyphs stay upright, GPUI can't rotate text).
* CH2: chamfer preview "5" labels checked in the app; Enter on the untouched first box now opens the second.

## From ui2 (10-22): assembly visuals taken from the lead (qa5 workflow 28, pass 17b), surgical edits
- **EXV3** (assembly_scene.rs, assembly_view.rs): `Highlights.explode_triad` drops the plane handles in explode
  mode (live 802); `Highlights.step_arrow` replaces the triad with one hollow arrow along the move while an
  explode step is dragged or its dialog is open (a ring with an arrowhead for rotations; live 805 / 810); the
  step being edited is highlighted in the panel.
- **EXV2 / EXV1 / EXV6** (assembly_panels.rs `render_explode_panel`): tree guide at x 16 with a 9 px tick even
  with no steps; rollback bar 7 px #b8b8b8 from 51 px to −6 px; the how-to box in live's info colours and
  margins (one line); empty state: the button on a 33 px #eaeaea band and a new illustration
  (`icons/exploded-views-empty.svg`, ui2's drawing); dragging the rollback bar shows live's blue "Rollback bar"
  pill and drops on the row boundary nearest the cursor (`AsmUi.explode_rb_drop`, a 2 px blue line + ring).
- **MNU2** (assembly_behaviors.rs mate menu): separators around "Edit named position driver mate…".
- **NP2** (assembly_positions.rs): with drivers, "Name" sits in the lower row of a #eaeaea cell; mate and DOF
  header cells #d7d7d7.
* SLOT2 (live's 14 constraints): new `ConstraintKind::Offset { reference, curve, master_ref, master, side }`
  (equations.rs `offset`: line ∥ master line at the reference's radius on `side`; circle/arc centred on a master
  point with that radius). A line slot is now live's set: caps on their own centre points, 6 Coincident, 4 Tangent,
  3 Offset (left, right, start cap from the end cap), and the tool's Diameter on the end cap only. Glyph "offset"
  at the end cap (`glyph_entities`). Not done: live's yellow highlight of the master on glyph hover (icons2), arc
  slots keep tangents + equal (no live data).

### 22 Oct evening: all 10 ANSI tapping screws (catalogue 205)
* Flat countersunk, Flat undercut, Oval undercut, Hex, Hex washer from live's MAIN tables
  (dom/skb-sc-576-sources.json; generator style "live" in /root/skb-scratch/gen/tapping.py: head, slot, I/IA
  (RLEN/RWID/RDEP), III, VI columns, per-size head types from live's option lists).
* fasteners.fs: `undercut` (82° cone stops at k, flat step to the shank; oval: crown from rim to apex), `indent`
  (hex: Ø0.9 s dish, (k − c)/6 deep, before slot/recess; Type I / VI heads carry it), hex washer = disc (c, dc)
  under the hex, length from the washer bottom.
* Mouth fillet (kern): FS opFillet must use the checked kernel fillet (kern's patch). Our side: Type VI spline
  starts at 0° (on the crown seam); Type III crowns are built seamless (`seamlessCrown`: profile to the rim,
  extended up, ∩ a sphere revolved about X). With kern's patch every #8 count = live (Pan III 19 / VI 15, Oval
  undercut III 18, Hex VI 27, Hex washer VI 29; heights = live's files). Tests accept live's count or the
  unfilleted one until kern lands; `pan_head_tapping_screw_mouth_fillets` is ignored until then.
* Type I (Phillips), live's recipe (dom/skb-sc-recess-I-IA.json) on every ANSI tapping head (`recessTypeI` in
  fasteners.fs): cross L × W + 4 kites (Cd, Fd/2) cut from the floor plane top − (D − T) up through the head; the
  tool's arm ends drafted 26.3° about live's THREE_POINT plane (where the arm-end corners meet the head
  top: the crown's height there, `headTopAt`; the top on flat heads) and its sides / kites by the slot angle about the floor (drafting
  the tool, reversed pull, is robust where drafting the part's faces failed); live's healed delete face is built
  directly (the cone is cut only inside the same drafted outline below the floor); 0.002 in mouth fillet.
  Hex / Hex washer #8 = live's 63 / 65 faces, every face area within 0.5 % (fillets 1.5 %). Values from live's
  MAIN tables for all 10 components (mainTables-5 + 576); Pan's cone uses RTHICK_IA (`rch`) as Onshape's part.
  Crowned heads use the seamless crown for Type I too. Type IA: next (still the generic cross).
* Type IA, live's recipe (`recessTypeIA`): one arm drafted 26.3° about plane 1 and patterned 90° (Fillister: onto
  itself, one arm), core Ø RCOREDIA_IA at the floor tapering by the slot angle with a 28° cone, four 10° wings
  (L·1.25 Pan, ·1.125 Oval, ·1 Flat): V-grooves (live's extrude to plane 1 + healed delete face = a tetrahedron
  per wing, sides cut by rotated boxes) or blind HeadHeight/2 on flat heads; RFILLET_IA on the 8 arm-side/core
  edges. Pan #8 with kern's checked fillet = live's 43 faces, every area to 7 digits; without it 35 (no fillets).

## Assembly sweep (skb 10-23, docs/parity/workflow-assembly-sweep.md rows 14–31)
* Model (`eval.rs`): `RelationStatus.inactive`; relations on inactive/suppressed mates are Inactive ("One or more
  mates is inactive or suppressed"), relations that fight each other show no error (live E8); a mate on a suppressed
  instance carries the reason "One or more instances is suppressed" (tooltip "<name> (Inactive: …)"). New failures:
  "Mate is between fixed instances." (both bodies fixed), limits min > max (E11). A part gone from its Part Studio:
  node "Failed to resolve instance.", mates "Failed to resolve the instance for mate." (Tangent: "Selection is
  invalid."), assembly mate connectors "Failed to resolve mate connector origin." (`Prepared.inactive_instances`).
* App: every assembly feature dialog (mate, relation, pattern / replicate, mate connector, group) has live's red
  title with the tooltip "<name> did not regenerate properly: <text>"; the mate dialog's old yellow alert box is gone
  (E1: a same-instance pick is silently refused). Pattern count errors keep the title black (live). Lost parts:
  red chips, Tangent chips "Missing entity of <instance>" (`MateDialog.lost`). Root badge "Assembly contains
  error", Mate features folder badge "Mate error", ⚠ on the fix icons when 2+ are fixed.
* Animate: no Animate… for Ball; translations 0 → 25 mm; one DOF auto-fills, several open empty and the mate row /
  chip box opens "Select degree of freedom" (rotation first: `anim_dofs`). Chips use ↹ like the relation dialog.
* Group dialog: live's 220 px feature dialog ("Group 1", Instances field, ✓ at 0.2 while empty, "Nothing selected.").
* Open: the named-position DOF columns still use "↔" (no live capture of that glyph).

## Standard content from Onshape's own tables (skb 10-23)
* Live's captures dom/skb-sc-comp/<key>.json (every component's *_MAIN table, all rows) are read by
  `/root/skb-scratch/gen/live_tables.py`; a mapping entry may name live columns with `live(names, **specs)` in
  mapping.py. Live's table is then tried before the published ones (its values win), rows keyed by every
  designation the Size option can take ("1-1/8-7", "M8x1.00", "M8 x 1"). `length_col` gives a one-length component
  (DIN 580) its Length. `published(table, col)` pulls a value live has no column for (B18.3's short-screw socket
  depth T_S under live's NOMINAL_LENGTH).
* Done with live's tables: ASME socket set / cap / button / countersunk / shoulder screws, ISO 4026–4029 + DIN 914,
  ISO 7379, DIN 923, DIN 7991, fine pitch hex bolts / screws / nuts (ISO 8765/8676/8673–8675 + DIN EN), ISO 7413,
  7719, 10513, 12125, 12126, DIN 935-3, DIN 580, DIN 125-1/-2, DIN 127 A/B, ISO 7090/7416/8738, ISO 2338.
  New: ANSI hex / heavy hex flat (+ jam) nuts, hex machine screw nut, square / heavy square / square machine
  screw nut, hex / thick / heavy hex slotted nuts, hex lock nut, nylon insert lock nut, acorn high / low crown
  nuts, hardened structural washer, regular / heavy-duty / extra-duty / high collar spring lock washers.
  Catalogue 205 → 243 components; report 159 → 95 problems.
* Geometry from live's B-rep (dom/skb-sc-geom; `gen/brep.py <pattern>` dumps one): ANSI nuts have the hole at the
  minor diameter (`dh`) with 90° countersinks to the major on both faces; 30° corner chamfers (`ca` 25 on square
  nuts, 0 on the square machine screw nut; flat nuts the top only); slotted nuts' slots end in half cylinders
  (`roundSlot`), the heavy one stands on a 1/64 in washer face; acorn crown = BODY_DIAMETER cylinder, an arc of
  BODY_RADIUS tangent into the NOSE_RADIUS sphere, hole drilled DRILL_DEPTH with a 118° tip (`rn`, `rb`, `dd`).
  Spring lock washers (form 8): faces sloping 2.5°, rounds (MEAN_WIDTH − BEARING_WIDTH)/2 as profile arcs
  (`roundedProfile`: exact tori; opFillet gave B-spline patches), split THICKNESS/2. Test
  `ansi_nuts_and_lock_washers_match_live` pins face types and total areas (live's to 0.1 %).
* Lock nuts (live's real-size captures; their "default" bodies are config placeholders): Hex lock nut = a 30°
  conical top from the major diameter (`dt`), countersunk below only (`cskTop` 0); nylon insert nuts = the hex to
  HEIGHT, a collar as wide as the flats to OVERALL_HEIGHT rounded 0.2 of its height, bored 0.75 s to the hex top
  (thin: 0.7 s, 0.345 of the collar deep; `cb`, `cd`). Both sizes of each pinned in the test.
* Flat countersunk tapping screw (live Type VI file, 14 faces): the cone runs at 41° from the head edge and an arc
  tangent to it and the shank ends at the head height on the shank (`cskFillet`; radius from the tangencies,
  0.0398 in at #8). Its Type VI mouth fillet still fails in OCCT on the Ø0.132 recess (13 faces, test allows it).
* DIN keys (6885-1, 6883, 6886) from live's FSRules tables (dom/skb-sc-fsrules-tables.json, loaded by
  live_tables.py for components skb-sc-comp lacks): the key along Y on the XY plane, Type A round ends, B square,
  AB one round end (−Y, confirmed by live's /features), tapers 1:100 falling towards +Y from h at -Y (`key` forms 0/1/3/5/6).
  Part names swap the chosen option for the default's " A x " token. 6885-1's 440–550 size has no length list in
  live's data (left out until live reports what the Insert dialog shows, REQUESTS "skb (10-23 14:00)"; then
  match it). `din_keys_match_live` pins faces, areas and orientation. B17.1 keys wait for PART 2.
* ANSI T-slot nut (nut form 7, live B-rep 5/16 + 1-1/2): the section extruded along Y (`tl`), tongue root fillets
  `tr` (live's FaceFillet ← CORNER_FILLET), 45° corner chamfers `tc` (CornerFillet ← CORNER_CHAMFER), the hole at
  major − pitch, countersunk both faces.
* AS568 O-rings now read live's table (inch; the old rows were ISO 3601 metric values, wrong) and live's four
  materials (Butyl default; generator `MATERIAL_NAMES`).
* Deviations left: our kernel reports the acorn's spindle torus (negative major radius) as a surface of revolution
  (kern); live's acorn 1/4 hex measures 0.438 where its table says 0.4375 (0.1 % area). ISO 7380's head uses HEAD_TOP_DIA (no rule yet).
* ANSI toothed lock washers (PART 2 raw /features; washer forms 9 / 10 / 11, helper `toothCut`): straight bands on
  radial lines (`teeth`, half angle `ta`, first at `t0`). External (A 12 × 7.5°, B 10 × 9°): root Ø 1.05 (ID+OD)/2,
  the bands are the teeth (all else beyond the root cut away). Internal (A / A heavy 10 × 9°, B / B heavy 9 × 10°):
  the bands as wide as the bore's chord are the slots, out to Ø (ID+OD)/2. Internal/external (form 11): 8 inner
  slots (±11.25°) to Ø ID+(OD−ID)/3, outer teeth (A 16 × 5.625°, B 12 × 7.5°) from Ø ID+2(OD−ID)/3 — live's volume
  fixes which bands are teeth (to 1e-15). Its "Outside diameter" select is per size (FSRules DATA_AI_OUTSIDE_DIAMETER,
  several ODs each with its thickness; generator `size_dims` → "d2@Outside diameter=…", "h@…"); the top
  connector follows the chosen thickness. Unconfirmed: Type B's OD table (its FSRules wasn't captured; same table
  name in its setData) and the list order/default for sizes whose table rows aren't ascending (#6: 1.510, 1.610,
  0.690) — REQUESTS "skb (10-24)". Tests `tooth_lock_washers_match_live`,
  `internal_external_tooth_lock_washers_match_live`.
* Retaining rings (PART 2; feature `retainingRing` rewritten, one sketch extruded symmetric about z = 0, lugs at
  −Y, live's single mid-plane "Near" connector). External (DIN 471, DIN 471 heavy, ANSI): groove circle inside; the
  outer circle as wide as L at +Y and S at −Y (DIN S = 0.3 L; ANSI SMALL_SECTION); lugs out to rg + lug with sides
  tilted 7° (heavy 7.5°), lug fillet 0.8 lug (ANSI 0.5 LugHeight), tip rounds 0.85 mm (heavy 0.8 mm, ANSI
  0.011116 in), gap s/1.95 (heavy s/2); holes on rg + lug/2, s apart edge to edge, the lug side placed 0.4 D from
  the hole: DIN 471 measures it from the lug fillet, heavy and ANSI from the side line (`holeRule`). Internal (DIN
  472, ANSI): the bore circle outside, the inner circle L wide at +Y and S at −Y (DIN S = L/2); lug ends on radial
  planes through the bore's dg/4 chord (±14.48°); each lug end an arc 0.5 (lug − L/2) (ANSI 0.55) tangent to the
  end plane and the lug circle rg − lug, a second equal arc into the inner circle; holes concentric with the end
  arcs. ANSI's select is "Shaft diameter" (rows keyed by SHAFT_/HOUSING_DIAMETER). Live's internal rings have a
  hidden Configuration enum (A/B/C: a 0.5 mm / 0.02 in fillet, or a second hole sketch); every captured size is
  A, which is what we build — the rule is unconfirmed (REQUESTS "skb (10-24)"). Internal heavy DIN 472 is left
  out: its default B-rep has a different lug (lug circle + an r 8.49 arc + 0.25 rounds), only one size captured.
  Test `retaining_rings_match_live` (faces + areas at 11 live sizes).
* Internal rings by live's FSRules configuration (behaviours "skb (10-24) rings / washers"; counts `ringConfig`):
  DIN 472 < 25 A, < 170 B, else C; heavy DIN 472 ≤ 24 A, else B (now in the catalogue); ANSI internal groove >
  1.962 in B, else A. B: the lug's inner edge on the lug circle, a blend arc (through the A-arc's centre, tangent
  in the inner circle; meeting the lug circle square for DIN 472, 0.96 D off the hole for heavy / ANSI: form
  `blendRule`, solved in `ringBlend`) and its 4 corners rounded `rc` (0.5 mm / 0.25 mm / 0.02 in), as profile
  arcs. C: the inner circle concentric at rg − lug, holes on rg − lug/2 1.5 D from the end planes. The test now
  pins 19 live sizes (A 12 faces, B 16, C 8; the earlier #300 / ANSI 10 captures were config A, wrong).
* B17.1 keys (FSRules + raw /features): Type per size by WIDTH (< 1/8 Square only, < 4 Square / Rectangle, else
  Rectangle; HEIGHT_SQR / HEIGHT_REC) as size rows with `when` {Type} (generator `variants`); parallel = the box
  (key form 1), tapered = key form 7 (`flat` = the width for Taper type Plain, 0 for Alternate, 1/8 in per foot
  falling to −Y, extruded symmetric). Live's Length is a free text box (behaviours "skb (10-24) B17.1 length";
  catalogue `lengthEdit`, app `std_length_box`): default "1", the component's unit, blur commits (Enter does
  nothing), a positive number names the part ("Parallel key B17.1 7 x 5 x 2.5"); otherwise live's red toast
  ("…@stringToNumber: String is not a number <text>" / "Not found." for 0 and negatives), the text stays and
  the part keeps the last good length. `fix_length` keeps a typed length. The tapered key's
  sizes from 4-1/2 - 5-1/2 up answer "Not found." on live's insert endpoint (crawl), listed as failing sizes.
  Test `b17_keys_match_live`.
* Askew head bolt (screw head form 10, `askewBolt`): live's order — square head with its bearing face skewed
  9.45° through the origin (HEAD_HEIGHT at +X), 25° corner cone cut from the flats, shank revolved, FILLET_RADIUS
  fillet, end chamfer 0.625·P·√3/2, the shank face split at the Thread length above the end (Thread length per
  length from DATA_AI_ASKEW_HEAD_BOLT_LENGTHS, generator `size_threads`; mod.rs now passes the chosen Thread
  length to the screw script as `lt`). Test `askew_head_bolt_matches_live`.
* NAS clinch nuts (raw /features; nut forms 8–10, `clinchNut` / `selfLockingClinchNut`): NASM 45938/2 = hex (flats
  ±Y) on a pilot hex, top corners 30°, the bore's pilot end coned out to the pilot's flats over 0.83·PILOT_HEIGHT
  (Chamfer 1's two offsets), top chamfer to the major; NASM 3214 = hex (flats ±X) LLen/3 on a CDia shank LLen/3
  then 0.92·CDia LLen/3, both bore ends chamfered to the major, a row per Nut thickness (DATA_CLINCH_FLUSH_NUT_
  THICKNESS; part name "… #6-32 x 0.147"); NASM 45938 (without self-locking threads) = the 12-line profile (collar
  CDia falling 15°, step 0.012 in, shank E_DIA T_LEN with E_DIA/40 rounds as profile arcs, bore opening 25° to the
  top, 1.5·(major − minor) countersink at the bottom) and 24 scallops (radius 0.06·E_DIA, neighbours meeting on
  the step's edge (CDia + EDia)/4). Connectors: catalogue `nearOnly` = live's single "Near" (3214 flipped, 45938
  not flipped, at the origin — which face live puts it on is not in the capture, unconfirmed). Test
  `nas_clinch_nuts_match_live` (faces + areas, both captured sizes).
* Still to do from PART 2: PEM® (44), NAS 45938 with self-locking threads, 45938/4 flush nut, NAS 1129 blind /
  thru standoffs (live's captures lack ALen / Length, so only the recipes check them); DIN 7050/7051/967 recesses, DIN 6908 height, taper and
  slotted spring pins (no live table). The face-type/area diagnostic is /root/skb-scratch/zz_live_face_types.rs
  (paste into assembly/tests.rs to run; reads the live captures directly).

## Mirror strategy (skb 10-23, live "skb (10-23 14:30)")
* `PatternKind::Mirror { derived, placement }` per seed (Derived → `MirroredPart`; `handed` kept for older
  documents = every seed Derived). App: `MirrorRow` per seed in `PatternSession`, `render_mirror_strategy`
  (781 px table over the dialog), `mirror_pending` (button + title red until Update). GUI-checked on :103
  (/root/skb-scratch/m45.png).
* Live's follow-up (behaviours "skb (10-23 15:30)"):
  - Symmetry: `eval::part_symmetry` (part vs its mirror image about each bounding-box mid-plane, common volume to
    1e-6), cached per regeneration; `AssemblyEvaluator::instance_symmetric` for the dialog (no plane needed),
    `Prepared::mirror_symmetric` per seed. Column "Symmetric" / "Non-symmetric" (our own `asymmetric` glyph); a
    newly listed Non-symmetric seed defaults to Derived.
  - Transform placement (eval.rs Mirror follower): the world reflection completed by a part plane — Bounding box
    XY / YZ / ZX through the bounds' centre, Origin XY / YZ / ZX through the part origin (live: an L mirrored in x
    → diag(−1, 1, −1), XY types). Auto position: a symmetry plane of the part (the one across the mirror first),
    else Bounding box XY (UNCONFIRMED beyond live's one asymmetric sample).
  - Derived: Update calls `Document::ensure_mirrored_studio` — "<PS>-Mirrored" with one "Derived mirror" feature
    (regen `importing::derived_mirror`: the source's solid parts mirrored in its YZ plane, "<part>-Mirrored",
    same part ids; the plane is UNCONFIRMED) in the folder "Mirrored parts and assemblies" at the end of the tab
    bar, reused afterwards and left when the Mirror is deleted. `PatternKind::Mirror::mirrored_studios` maps
    source → mirrored studio; Derived copies are those parts (eval completes the reflection with the YZ plane).
    `features::importing::is_derived` (importDerived + derivedMirror) gates the derived provider, cache keys and
    source-change refreshes (the assembly evaluator regenerated "-Mirrored" without it: "Failed to resolve
    instance."). Tests: model `mirror_placement_types_and_symmetry`, `derived_mirror_makes_one_mirrored_studio`;
    ops `tests/derived_mirror.rs` (parts mirrored with the source's ids; the Derived copy's centre of mass is the
    seed's reflected). GUI-checked on :103 (/root/skb-scratch/g3.png Non-symmetric → Derived, g5 Update →
    "Part Studio 1-Mirrored" + folder at the end, g8 its Part Studio, h2 the copy).
* Replicate / pattern fields carrying the error: `os::query_box_error` (pink, red border, grey label) and the
  error as the label tooltip (`os::query_box_tip`).

## Assembly section view manipulator (skb 10-24, lead's priority; live "skb (10-24) assembly section")
* On icons2's shared Section view dialog. While the dialog is open the active plane has live's manipulator
  (assembly_scene.rs `SectionGlyph` / `push_section_glyph`, handles `SectionHandle`, picks `OverlayPick::Section`):
  a base circle at the plane's base (face centroid / connector origin + offset), the shaft and hollow head, and
  two small rings with their axis lines at the tip (ring 0 turns the plane about its in-plane Y, ring 1 about X;
  `AsmSectionPlane::x` keeps the in-plane axes). Idle strokes light grey (230,232,233 on 194,199,203), the
  hovered handle blue (70,123,177), the dragged one 40,97,182; no tooltips.
* Drags (assembly_behaviors.rs `start_section_drag` / `drag_section` / `end_section_drag`): the arrow moves the
  plane, the value being the distance from the drag's start ("⊥" mark at the start with a leader); a ring turns
  the plane about its axis through the base ("⊢" mark at the start direction), snapping to 5° multiples within
  2° (the window is UNCONFIRMED; live left 92.111 / 97.206 free); the base circle slides the manipulator over the
  plane (live also snaps it to edges: not done). The value box (upper right of the tip, 75 × 26): the dragged
  value while dragging (1 decimal mm, up to 3 decimals "deg"), then Onshape's canvas box focused and selected;
  Enter puts the plane at the typed value from the drag's start and keeps the box; Esc closes the box only.
* Chips "Section plane N (Top)" named by the arrow's standard direction (Top / Bottom / Front / Back / Right /
  Left), no suffix once rotated off them (`direction_name`; the dialog drops an empty suffix).
* Right-click empty space with a section on: Show all | Edit section view… (section icon) | Turn off section
  view | — | Create Drawing of … ; Edit reopens over the section (offset / rotation kept) and ✕ there turns the
  section off.
* Not done here (for icons2 / the lead): Flip lives in the "Select default plane" popup (icons2's dialog); the
  red overlap fill in caps where instances overlap (renderer). GUI-checked on :103 (/root/skb-scratch/t1–t9.png).

## P1 mates audit (skb 10-24, PRIORITIES.md item 1)
- **Solver**: `solver::tests::core_mates_leave_their_degrees_of_freedom` checks the six everyday mates against live's DOF
  table (behaviours "skb (10-22 20:30)" › DOF per mate type): Fastened 0, Revolute 1, Slider 1, Cylindrical 2, Planar 3,
  Ball 3. A drag of the moving part's origin follows only the free translations, and a Ball drag turns the part about
  its centre.
- **GUI pass on :103** (/root/skb-scratch/mates: `mk.py` builds a 60×40×5 plate with a Ø10 hole plus a Ø10×30 pin;
  `retype.sh` retypes through Edit… and drags): Revolute from the hole edge to the pin's bottom edge stands the pin on
  the hole (z 5). The drag turns it about Z only. Retyping to Slider snaps the rotation back, and the drag then slides
  along Z only. Cylindrical slides and turns (2 DOF). Planar keeps z 5 and moves in XY plus Z rotation. Ball snaps the
  centre back to the hole and tilts freely. The dialog rows per type match live (Cylindrical and Ball have no Offset).
  Inference on the pin's side face puts the connector mid-axis, with a dot at the end centre (live assembly/31).
- **Fixed: where the mate heads-up toolbar opens.** It used to open +14/+14 px below-right of the second pick, with no
  clamp, so near the right edge its ✓/✕ fell off-screen. Live (assembly/35 + dom/mate-hud.txt: pick ≈ (1419, 362),
  panel [1264, 311, 310×51]) centres the panel on the pick with its bottom edge on it. Assembly/93 shows it pushed in from
  the right edge. Ours now does both (`render_mate_hud`). The note "just below-right" in behaviours.md is not what the
  coordinates show.

## P2 drag/solve speed (skb 10-24, PRIORITIES.md item 2)
- **How to measure:** run the app with `NOSHAPE_PERF=1 NOSHAPE_PERF_PERIOD=3`. Each drag step logs `a.drag_solve` (the
  background solve), `a.drag_apply` (apply + scene rebuild on the UI thread) and `a.drag_step` (pointer target to
  applied). Scripts are in /root/skb-scratch/perf:
  - `mk_chain.py` + `mate_chain.py`: 20 links chained by 19 revolutes.
  - `mk_pins.py`: plate + revolute pin + 18 free pins, 20 parts.
  - `run.sh <doc>`, then `drag.sh X Y DX DY STEPS` (125 Hz pointer moves).
- **Before:** dragging the chain's last link took **≈600 ms per solve** (p50 580–655 ms).
- **After:** **≈39 ms** p50 for the chain, and **0.2 ms** for the mated pin among 20 parts.
- `a.drag_step` latency beyond the solve is the UI thread waiting on Xvfb's software GPU (r.gpu_ms ≈ 300 ms per
  frame on :103), not ours to fix here.
- **Solver changes (`solver.rs`):**
  - **Phase 1 (follow the drag) starts nearly undamped.** The damping λ·(1+diag) used to start at λ = 1e-3. Along the
    mates' free directions the only curvature is the drag term (DRAG_WEIGHT² = 1e-4), so each step followed ≈10 % of
    the drag and needed 14–48 iterations. It now starts at 1e-9 (7–13 iterations, exact tracking).
  - **Bodies are solved in connected groups.** Union of the free bodies each term reads; fixed bodies link nothing.
    The LM and the DOF eigen-analysis run per group, so a drag only solves its own group, and the dense algebra
    scales with the group, not the assembly.
  - **Sparse finite-difference Jacobian.** A body's columns re-evaluate only the terms that read it.
  - **No DOF analysis in phase 1.** It ran twice per drag step before.
- **`SolveOptions::skip_dof`:** the app's drag steps skip the DOF analysis and keep the previous instance-list glyphs.
  The drag's "Move" edit re-evaluates them.
- **Tests:** `chain_drag_follows_in_few_iterations` (exact tracking, ≤ 80 drag-phase iterations over 5 steps; it was
  111) and `drag_moves_only_its_group`.
- **After an edit** (`a.eval` / `a.eval_apply` / `a.eval_catalog`, same NOSHAPE_PERF run): the 20-link chain re-evaluates in
  19–33 ms after a drag's "Move" commit, and applying takes 10–11 ms; the Insert catalog takes < 0.3 ms.
- **Insert:** live doesn't fix any inserted instance (ins-914: all three show the free glyph), and neither do we.
  qa4's open "? both inserted instances show the fixed icon" (core-workflow 6) doesn't reproduce:
  - two free instances fastened together show the free glyph on both (dof 6 each);
  - a part fastened to the fixed one shows no glyph, as live 2282 does for Part 2 <3>.

## Standard content: DIN 985 back in the catalogue (skb 10-25, P3)
- **The problem:** live names DIN 985's sizes with their pitch ("M3x0.5", fine "M8x1", and "M48X3" / "M20x2x2" as typed in
  live's list). The generator matched none of them to the published table's "M3" rows, so the whole component was dropped.
- **The fix:** `build_catalogue.py` `candidates` now falls back to the diameter's row for "M<d>x<P>" names, and the pitch comes
  from the name, case-insensitively. DIN 985 now has 44 sizes, built with the ISO 10511 insert-nut recipe from DIN 985:1987.
- **Still missing:** M2 and M2.5, which the 1987 table lacks.
- **Asked live** for DIN 985's geometry (REQUESTS "skb (10-25)" DIN 985): it would confirm Onshape's collar and give M2 / M2.5.
- **Test:** `din_985_nuts_build_every_live_size` (every size builds; M8 is 13 across the flats and 8 high).
- **Still in the catalogue report:** 52 PEM and 4 NAS sheet-metal fasteners unmapped. Size gaps remain in ISO 7380 (M2, M2.5,
  M18), ISO 10511 (M2, M2.5, M18), ISO 10512 fine pitch, ISO 7379 lengths, DIN 7050 / 7051 / 967 / 923 / 6908, and ASME taper /
  spring pins. Onshape's own tables are captured for all of them (dom/skb-sc-comp), so live() mappings can close them.

## Standard content: hex socket screws against live's B-rep (skb 10-25)
- **The gap:** none of our socket screws had been checked against live. For the 8 captured in dom/skb-sc-geom (ISO 4762, 10642,
  7380, 4026–4029 M6; ANSI 1/4-20 socket head), ours were 2–18 % off in area, with 10–23 extra faces.
- **Socket (all screws):** a hex to the depth t, then a drill point from the flats' circle, ISO 140°, inch 120° (`drill` param,
  set by `mod.rs` from `c.inch`). This leaves six flat ledges in the hex corners, as live does. The old 118° cone from the
  corners left ~18 sliver faces.
- **Point chamfer:** socket screws end at the basic minor diameter d − 1.0825 P (`minor_basic_live` for ISO 7380 and the ANSI
  SOCKET_LIVE family), as every live socket screw does.
- **Under-head fillet:** new `ru` param (ISO 7380 HEAD_FILLET).
- **ISO 7380:** now takes live's own table, which adds M2, M2.5 and M18. Every face but the dome matches live exactly (test
  `iso_7380_button_head_matches_live_but_the_dome`). Live's dome is a sphere through the rim (new `rimDome` flag), but its radius
  doesn't follow from M6's row (it reaches the head height at r 1.30, while HEAD_TOP_DIA is 6.0); asked live (REQUESTS "skb
  (10-25) ISO 7380 dome").
- **Next:**
  - ISO 4762: a HeadFillet on the head top edge and an under-head transition cone (AverageDiameter, TransitionLength), with
    UnderHeadFillet at both ends.
  - ISO 10642.
  - Set screws: live's socket is a 0.7 mm flat-bottomed hex in the one capture, against table depths of 2 / 3.5; asked live
    (REQUESTS "skb (10-25) set screw sockets").
