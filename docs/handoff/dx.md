# dx handoff: agent API + drawings + data exchange

Area (PLAN.md 2.3): crates/ops, crates/cli, docs/AGENT_API.md, crates/model/src/drawing, crates/app/src/drawing_*.rs,
import/export (kernel and model import/export files), configurations, Variable Studio, Derived, start window and
document visuals. Older notes: agent-api.md (api), ui2.md + theme-studios.md (ui2), drawing-behaviour.md (asb),
drawing-verify.md (drw2), qa5-routing.md (api-owned rows).

Setup: worktree /root/projects/noshape-wt/dx (branch agent/dx), env in /root/dx-scratch/env.sh (CARGO_TARGET_DIR
/root/target-dx, OPENNURBS_ROOT=/root/opennurbs so Rhino export is built), detached jobs with
/root/dx-scratch/bg.sh <log> "<cmd>". Live: `cd /root/live-pw && node dxr.mjs -e "…"` drives dx's own Chrome tab
(target id in /root/dx-scratch/live/.dx-tab).

## Build times (load ~25–30)
- 10-26 after c9540ae6: first build ~25 min; edit ops/src/values.rs → `cargo test -p noshape-ops --test all
  e2e::hole_standard` 74 s.
- 10-27 after the model split: first build after the rebase ~8 min (+ the kernel's C++ once for ccache); edit
  model-core regen + ops session → `cargo test -p noshape-model-core --lib regen::crash_guard::` then `-p noshape-ops
  --lib session::`: ~6 min for both (model-core, ops rebuild under load); ops `e2e::` run 11 s.
- 10-27 03:00–04:10 (load 30–55): ops edit → one e2e test 1–3 min; session.rs edit → `--lib session::` ~1 min;
  first test after a rebase onto app-core ~6 min; clippy -p noshape (app) after an ops edit ~4–7 min; T0 ~2–5 min.
  Every cargo invocation after a kernel or Cargo.lock change re-runs the kernel's build script (C++ from ccache,
  ~1–2 min each time); a full landing check (coverage + e2e + clippy app/ops + T0) is 13–25 min.

## Landed on main (2026-10-26/27)
- aa563bcd ui2's DL12 piece; c63d2bf5 drawings / data exchange (ANSI thread arcs, PDF FlateDecode, material density
  types, silhouettes in drawing_get, rhino bound); 5ef64eb8 + 829a8627 agent API batch 1 (Variable Studios, version
  properties / publish, Hole Standard, sketch_create plane query, variable_set name / new_name, Derived by tab name…);
  f0da1ca3 batch 2 (singular list args, legacy hidden params refused, VS configuration rules as live, one VERSION).
- b29afb6f fixed my own mistake: 36e4da66 had pushed TICKETS.md mid-rebase with conflict markers (qa's tickets restored).
- 87e8275f P0 DOC-POISON: crash markers around each feature evaluation (model-core `regen::crash_guard`, enabled by
  `ops::session::Session::open/create`); Session::open rolls a Part Studio back above a feature whose evaluation
  stopped a process ("Roll back above <name>: it stopped noshape while regenerating"), notes in `Session::recovered`
  and document_open / document_info `recovered`. Checked on qa's cli-fillet-segv.noshape: open 1 aborts in the kernel
  (Shell 4), opens 2 and 3 work. The app needs nothing new (it opens through Session::open); a toast with the note
  would be nicer (app side, after the app split).
- ece4df3e PDF layers (OCGs as live's export) + live's scale list; 9932ff31 auto centermarks on whole circles only (qa
  DRAWING-SCALE closed); our A3 PDF is now 92.8 KB (live 92.8 KB).
- 02f60fee F19 Width mate e2e (3 DOF left, the live-correct count; api's notes guessed 5).
- b0dc0898 Part Studio created in context updates as live (refreshed_in / out_of_date_in; app switch requested).
- 556bdcb5 Hole add: std holeEditLogic two passes not three (~330 → ~270 ms; the rest is FeatureScript speed, core).
- 8bc476b0 sheet metal table through the API (sheetmetal_* ops, ops::sheetmetal); SM-MODIFY-JOINT ticket for core.
- 16cdcf6d one-line open errors; 6a21a5d0 assembly_set_state `state`; D8 and D12 rows verified in workflow-drawing.md.
- 8993ea92 audit of the app's edit history texts, part 1: config_configure unconfigure, mate connector suppression,
  document_update_workspace, variable_list visible_at_end.
- 3dfa5710 audit part 2: assembly_move_instance (the instance list's drops: Reorder instances / Add to folder /
  Move to subassembly), assembly_exploded_view `move` (Reorder : Explode steps), assembly_pattern mirror `strategy`
  (Derived / Transform + Placement per seed; makes the "-Mirrored" Part Studio like the dialog's Update),
  drawing_image (Move to sheet, Image borders); OPEN-ERROR-SILENT part 2 (the start window's open error stays until ×;
  the Open dialog's failures show it); app's image menu and Mirror dialog call the shared ops functions. Removed the
  stale MENUS line sk-dim-tol (skasm's 4c58bbd4 dropped that menu item; coverage:: failed on main).

- fad02aa1 P0 TWO-PROCESS-LOST-EDIT: a Session remembers the microversion its document was read from; when
  another process moved the head, a commit merges this edit's names three-way onto it (tabs feature by feature,
  noshape_model::merge) and reloads, so neither writer loses an edit (live: one workspace, writers append). Test
  session::tests::two_writers_of_one_file_keep_both_edits. Left: (a) two writers changing different fields of the
  *same* feature: merge_part_studio keeps ours for a feature changed on both sides (core's merge.rs could merge the
  feature's params field by field); (b) live shows B's edit in A's open tab within seconds: `Session::sync_external()`
  loads a head another process moved; DocModel::watch_external polls it every 1.5 s (c7928bb4, app-core, cleared by
  the lead; checked in the app: a CLI rename showed in the open window within 4 s).
- Also 10-27: bd14ff0e material_library_update / _export; 5b4b2831 part_annotation move; da75c032 3MF / OBJ welded
  vertices (EXPORT-MESH-SHARED); 71e8357e PDF Text: Selectable embeds the drawing fonts (TrueType subsets, ToUnicode);
  c7928bb4 API-REMOVE-DIR closed per the lead (the dialog's flip, documented); c30baf56 CRASH-MARKER-COPIES (a marker
  applies only to files its dead process had open, `<pid>.docs`).
- bea661f1 Derived carries a source part's explicit appearance (live: red stays red; a default colour is the
  target's next one, as ours already was); query_parts reports each part's `color`; RHINO-GREEN: RHINO is listed (app
  dialog and export_parts) only in builds with openNURBS (kernel::RHINO_EXPORT from the build script's cfg).
- 3b23c8d1 history_list filters (the history panel's search ▾, ops::history_filter shared with the panel); ui_list on
  Variable Studios and collapse_tables. 872c2e2e release_create / release_revision_history: the Release candidate
  dialog's items, checks and commit moved to ops (release.rs, release_settings.rs, release_conditions.rs; the app's
  modules re-export them). Menu coverage: 0 planned items in every area (was 26).
- 771afc0d Extrude add P1 closed with measurements (7–25 ms once std's prewarm has run); I1 narrowed (x_t blend tube,
  spine, edges exact; live's per-face areas requested); flat-pattern line styles need live (requested).
- Agent benchmark 10-27: part → ISO A3 drawing with suggested views → 80 and Ø16 dimensions → note → Selectable PDF in
  1.2 s through the API (/root/dx-scratch/w/py/bench_dwg.py); pdftotext reads every label.

## Paused 10-27 ~09:50 (lead: app split first)
- Landed since the morning handoff: 7b947277 P1 UNDO-NOOP-SHIFT (store: a no-op commit kept the previous undo step);
  b305c5ca API-MATE-RETYPE-NAME; 56c1ee31 Selectable PDF fonts named as live's (AAAAAA+NotoSans); ba3f2c5e
  DWG-DIM-TWO-EDGES (edge-only drawing tools hit-test edges under annotations; API line_to_line fixed, app half to
  re-check on the next green); b8f58d5b the crash-marker test isolated (Session::open_with_markers; main was red).
- Resumed ~11:30 after the app split: CONFIG-CELL-EDIT was landed by psui (e013107b); my WIP dropped. API-ASM-CONFIG-NAMES
  fixed (configuration by input / option names resolved in assembly_insert / set_configuration / replace and drawing
  sources). I1: live's face data says our x_t = live's x_t exactly; only our STEP reader's blends are off (13.1819 vs
  live's STEP 13.2001): next on I1.
- Landing now only through tools/tiers/land.sh (lead 10-27).
- Afternoon 10-27: 0d75aa8e API-ASM-CONFIG-NAMES; a74853db EXPORT-OBJ-MTL (OBJ zip with .mtl, 3MF in metres as live);
  4a905b47 DWG-DIM-TWO-EDGES verified in the green app; 9c187bd1 drawing_annotation_edit orientation / flip_arrows /
  arc_condition and drawing_view_edit cutting_line_angle (shared, the drawing UI calls them); 7bca8284 assembly_snap
  (Snap mode, shared with the assembly view). I1: STEP surface-curve modes ruled out. FLAT-FORM-STYLES filed (needs
  core's flat-pattern data). CONFIG-INPUT-ID waits on live (REQUESTS dx 10-27 P3).
- Evening 10-27: CONFIG-INPUT-ID Checkbox as live's dialog (name required, id from it, default checked) in
  config_add_input (Part Studio and Assembly); the dialogs are psui's (request below). EXPORT-OBJ-ZIP-NAME (the save
  prompt says .zip; zip entries dated). FLAT-FORM-STYLES done: counterbore / countersink heads in the flat (native
  route; a std-built flat, i.e. a std sheet metal feature after the hole, has no heads yet: check next).
- Audit of edit texts (10-27 afternoon): left uncovered on purpose: "Use view geometry" (a step of the drawing sketch's
  constrain tool), "Edit configured dimension" (= config_set_value), "Copy view label" (Alt+drag copy of a label).
- Live answered I1 (behaviours "dx (2026-10-27, P2 I1 …)") and the flat-pattern line styles (REQUESTS dx 10-27 P3): read the latter next.

## Daily walk 10-27 03:40 (green 8165fca6, :131, /root/dx-scratch/w/o*.png)
- No regressions in my area. Open of a truncated file: the error bubble stays (OPEN-ERROR-SILENT verified). Drawing
  from ISO A3 suggested views: first-angle layout, hole centre mark; Export › PDF: 1.5, A3 landscape, 5 layers, Flate,
  92,359 B (live 92,767). Variable Studio: typed "12" → "12 mm". Part › Export… Parasolid 38.1 then + › Import…: the
  dialog offers material density for x_t, Import 1 rebuilds the same part, "CAD Imports" folder. Derived of Part
  Studio 1 into it: dialog, configuration row and the part as DER1/DER10.
- Live answered the derived colour question: the target's next default colour unless the source part's appearance is
  explicit (then carried); ours now does both.

## Next (in order, after landing)
0. Import numbers: the remaining ✗ rows of docs/parity/workflow-import-matrix.md (g50 F8 / F19, McMaster helix) and
   I1 (x_t blend trims). (Configurations C1–C8, Derived DER1–14, Variable Studio VS1–4: all closed in the docs.)
1. Audit done ("Move annotation" = part_annotation move, landed). Covered elsewhere:
   Move configuration input (config_reorder_inputs), Hide all instances (assembly_set_state on every instance), Save
   custom color (any colour is settable), Update material library (material_library_update, this landing).
2. API: benchmarks done for Variable Studio, in-context, sheet metal (/root/dx-scratch/bench/*.py); next: drive a model
   through the MCP server with a real client; render_image of the window camera (image size).
3. Drawings (P2): PDF Selectable text landed 10-27 (embedded TrueType subsets, CID fonts with ToUnicode; same
   placement as the outlines); open: compare its file size / font object layout with a live Selectable export; ui2's
   Tables-end and thread tiles checked vs live 10-27 (match); flat pattern Counterbore / Countersink / Form styles.
4. Re-verify old rows on the green build: workflow-configurations.md C1–C8, workflow-derived.md DER2–DER14 (DER8 works
   through the API; check the app's Insert panel), workflow-drawing.md D14/D16 (D8, D12 verified 10-27).

## Requests to the lead
- I1 (P2) is core's now: the kernel's face integrator under-reports our STEP blend faces (Gauss 13.18188 vs a fine mesh
  13.19411 on the same face); dx ruled out the reader and the geometry (TICKETS I1 has the numbers).
- assembly-ui (after your move settles): assembly_mate_dialog.rs set_type renames a default name inline; it can call
  `noshape_model::default_names::retyped(&self.name, old, new)` (the same rule the API now uses), one copy.
- App (after the app split): crates/app/src/part_studio/sheet_metal_panel.rs has the same logic as the new
  `noshape_ops::sheetmetal` (joint_rows, joint_edit_index, insert_joint_edit, bend_faces, bend_row_order, plain, the
  Radius / K factor edit, Convert to rip, Move up / down); its handlers should call that module (dx can do it once the
  app crates are settled). Until then the two copies must change together.
- App (after the app split): crates/app/src/doc_dialogs.rs outdated_references / apply_update are now
  `noshape_ops::workspace_update::{outdated, apply}`, and config_panel.rs's "Unconfigure parameter" is
  `noshape_ops::config::unconfigure_parameter`: switch the app to them (same logic, one copy).
- App (after the app split; 1 line each): crates/app/src/part_studio/in_context.rs:788 Update context should call
  `noshape_ops::in_context::refreshed_in(&old, &result, asm, None, Some(<this Part Studio's tab>))` (a Part Studio
  created in context then updates like live; today the app keeps refusing it), and the app's out-of-date dot should use
  `out_of_date_in(.., Some(tab))`. The API already does (dx, e2e part_studio_created_in_context_updates).
- psui (part-studio-ui config_panel.rs, assembly-ui assembly_config.rs): live's "Checkbox" button opens a create
  dialog (Name, empty and required; Default, checked) and the id comes from the Name ("My flag" → My_flag). Ours adds
  "Checkbox N", unchecked, at once. The Part Studio's dialog should call `noshape_ops::config::add_checkbox_input(ps,
  name, default)` (the API's config_add_input does, CONFIG-INPUT-ID); the Variable Studio's dialog already matches.
- RHINO-GREEN: please build the watcher / green app with `OPENNURBS_ROOT=/root/opennurbs` (release.yml builds with it,
  dx's builds too). Hiding RHINO in builds without it needs a kernel build.rs cfg, i.e. a C++ rebuild for everyone; not
  worth it unless you want it.
