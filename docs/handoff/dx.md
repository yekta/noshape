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

- (this commit) P0 TWO-PROCESS-LOST-EDIT: a Session remembers the microversion its document was read from; when
  another process moved the head, a commit merges this edit's names three-way onto it (tabs feature by feature,
  noshape_model::merge) and reloads, so neither writer loses an edit (live: one workspace, writers append). Test
  session::tests::two_writers_of_one_file_keep_both_edits. Left: (a) two writers changing different fields of the
  *same* feature: merge_part_studio keeps ours for a feature changed on both sides (core's merge.rs could merge the
  feature's params field by field); (b) live shows B's edit in A's open tab within seconds; the app only picks it up at
  A's next edit (needs a file watch / head poll in the app).

## Next (in order, after landing)
1. Audit leftovers: Part Studio "Move configuration input" (config input order outside Variable Studios), "Move
   annotation" (Part Studio annotation offset), Hide all instances, Discard unregenerated changes, Save custom color,
   Update material library: check each against the ops, add what's missing.
2. API: benchmarks done for Variable Studio, in-context, sheet metal (/root/dx-scratch/bench/*.py); next: drive a model
   through the MCP server with a real client; render_image of the window camera (image size).
3. Drawings (P2): PDF Selectable text with a subset NotoSans (Normal text is already filled outlines); ui2's
   Tables-end and thread tiles checked vs live 10-27 (match); flat pattern Counterbore / Countersink / Form styles.
4. Re-verify old rows on the green build: workflow-configurations.md C1–C8, workflow-derived.md DER2–DER14 (DER8 works
   through the API; check the app's Insert panel), workflow-drawing.md D14/D16 (D8, D12 verified 10-27).

## Requests to the lead
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
- RHINO-GREEN: please build the watcher / green app with `OPENNURBS_ROOT=/root/opennurbs` (release.yml builds with it,
  dx's builds too). Hiding RHINO in builds without it needs a kernel build.rs cfg, i.e. a C++ rebuild for everyone; not
  worth it unless you want it.
