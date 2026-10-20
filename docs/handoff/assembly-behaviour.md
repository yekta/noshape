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
  - **State at the lead's PAUSE (20 Oct, after the OOM reboot):** the GO code is in dc0da0a. Uncommitted
    since then in assembly_panels.rs (mine, compile-checked before the reboot):
    `step_edit_activate(k, cx)` replaces the `set_active` closure in `render_step_edit` (fixes E0500), the doc
    comment moved back onto `render_step_edit`, and line 2871's `red.into()` → `red` (clippy
    `useless_conversion`). The other hunks in that file (`insert_search_box` taking `&mut App`,
    `crate::ui::input_with_placeholder` in the search / URL boxes) are another agent's placeholder sweep, not mine.
    Not done yet: (1) clippy `-p noshape -D warnings` after the 2871 fix: stopped by the reboot / PAUSE, so the
    app crate is unconfirmed warning-free (clippy on the model crate stops first on someone else's uncommitted
    `display.rs:50` `chunks_exact_to_as_chunks`; I ran with that lint allowed to reach the app); (2) build
    (bld.sh) and copy to /tmp/skb/noshape (gone with /tmp); (3) GUI checks on :103 of the three GO items:
    mate alignment default (bottom → top stacks upright, rim → rim coincide), the Explode step dialog against
    live 805 (drag opens it, Instances / Specify / Distance / Explode lines / line refs, live preview, ✕ removes a
    new step, menus, rename, double click), the Replace picker (Insert panel in picking mode, the pick fills the
    field, ✓ replaces). Xvfb :103 was restarted after the reboot (no app on it). Still open afterwards: the
    replacement's in-place orange preview and red mates, the picker's four filters, name keeping on replace,
    the Face tab and reference connector in the Mass dialog.

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
