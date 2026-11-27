# psui (Part Studio UI): current state
Updated 2026-11-25 08:3x UTC. History up to then: docs/handoff/archive/psui.md (not read by default). Standing rulings:
docs/DECISIONS.md (read first; it wins over anything here).

## Setup

- Worktree /root/projects/noshape-wt/psui, branch agent/psui. Commit as yekta (AGENTS.md), subject "psui: …".
- Builds, crate tests and scenario runs on vibex7 (lead 11-26 02:0x: vibex8 is bool's; never vibex8): `NOSHAPE_RB_HOST=… tools/tiers/rb.sh bash -c "$(cat script)"`,
  CARGO_INCREMENTAL=0; `git diff --name-only origin/main | xargs touch -c` first. ~/bin/slot
  only for suites, gates, timing pairs (DECISIONS 9e5b481d4). Scripts in /root/psui-scratch/run*.sh. The GPUI dialog matrix is
  `dialogs::the_dialogs_matrix_matches_live` (rows filter: DIALOG_ROW=…).
- Scenario / app runs: vibex6, at most 3 app instances (shared with studios). Heavy runs (scenario shards) through
  `~/bin/slot <cmd>` there (one per host, waits for 10 GB free). The app build there OOMs under load: scripts build
  with `-j 4`.
- Land only with `tools/tiers/land.sh` (`--status` for results). Start an rb.sh run, wait until it builds (its sync
  done), then land.sh: land.sh's rebase during an rb.sh sync breaks that build.
- Live: Chrome on :107, CDP 127.0.0.1:9333, helpers /root/live-pw; captures are asked of live2 in
  onshape-research/live/REQUESTS.md (dated `date -u`). Recorded behaviour: onshape-research/live/behaviours.md.
- Private documents: work on a mode-700 copy in ~/tmp, delete it after.
- GPUI tests: platform keys as "secondary-…"; marks (`ui_script::mark`) only record while a scenario runs, so tests
  find controls by `debug_selector`.
- psui owns docs/parity/interaction-feel.md (the feel matrices; all matched as of 11-15): keep it current.
- A new menu label needs its line in crates/ops/src/coverage.rs (`cargo test -p noshape-ops --test all coverage::`).

## Tools (/root/psui-scratch)

- `uip-some.sh <out> <names…>`: release build on the rb host, then the named ui-parity scenarios on one Xvfb (:392).
  Run: `NOSHAPE_RB_HOST=vibex7 tools/tiers/rb.sh bash -c "$(cat /root/psui-scratch/uip-some.sh)" uip-some <out> <names…>`
  (`~` in remote paths expands locally: pass /home/ngyekta/… paths). Output buffered: the log fills at the end.
- `psui-shards.sh <out> <names file>`: the same in 3 shards, then one `--compare-only` score. The names file must be on
  the host (scp it first). psui-names.txt: the 1021 psui-area scenarios with live takes.
- Compare without rerunning: `python3 tools/ui-parity/run.py --compare-only --out <dir> <names…>` (through rb.sh).
- Runner env: NOSHAPE_UIP_LOGS=1 (app stderr), NOSHAPE_UI_SHOTS=<dir> (a screenshot per step), NOSHAPE_PERF=1,
  NOSHAPE_CACHE_DIR (fresh: cold runs).

## DONE, QUEUED 1f2f4a5d (11-25 21:55, 31 commits, rebased on f25d15ae0; lead told): snaps fix + page runner
- Snaps cause: schedule_regen's burst coalescing (requests < 100 ms apart on the REAL clock wait a 12 ms EXECUTOR
  timer). Unloaded test moves were bursts -> no preview mid-drag; on the loaded queue they weren't -> the 2.8 deg preview
  came, and revolve_arc's axis_of resolved "edge through [20,0,20]" in the turned preview's context (None) -> ring gone,
  drag dead. Fix: axis_of resolves edges in ui.before_context (manipulators.rs); test advances the executor 12 ms per
  move and draws until !ui_busy (each move's preview applied, deterministic).
- Lead item (before combined hover), from qa: page scenarios (documents page) couldn't target a page-opened document's
  graphics (page runner called locate with no views). ui_script.rs: page_locate / doc_workspace / settle_doc, follow()
  sizes+activates a newly opened document window, redraw repaints its chrome+tab. T1 workspace_tests::
  a_page_step_finds_the_graphics_of_a_document_the_page_opened. Tell qa after landing: flow 91 had no perf lines
  because milestones need NOSHAPE_PERF=1; "no assembly frames" maybe the inactive new window (now activated).

## P0 DONE (lead 11-26): failed-blend blame off regeneration
- Landed d027ad72 (72bc8ad3) + step 2 (f558a4ae): regen blames only a lone pick; the dialog asks
  `blame::failing_alone` in the background (≤24 picks, 10 s); ops `feature_get {blame:true}` -> `red_selections`.
- Same-host A/B (vibe2 05:1x, release T2, 3 alternating runs): base 47b49528b median 5.31 s, main 5.03 s (0.95x):
  no residual regression; told lead. bool's 1.1 s was a cli measurement, not the T2's feature time.
- Main RED 05:06 at 72bc8ad36 (not the fix): parity::a_face_two_parts_have_is_the_one_on_lives_part shared the scratch
  file face-on-lives-part.noshape with a_face_live_names_on_its_part_… (parallel document_create race); renamed. Lead
  picks the fix into v0.4.71.
- QUEUED dc6768f9: copy-paste (menu-open hover guard + hover end; T1 in sketch_hover_feel_tests).

## QUEUE (11-25 19:5x, lead + live)
1. LOOK-OUTLINE-WEIGHT CLOSED = live (lead 11-26 01:5x; TICKETS row has the numbers): band = live's
   vs-backfacesil-faces exactly (push 0.5*1.2*lw*(1-|ndv|), black, 4x MSAA); left 0.58 vs live 0.73 px ink, right
   0.61 vs 0.59; 151a1e = fillet tangent edge (live 263035). No more time on it.
2. DONE (fe39cfd77) combined hover P0: 3 tests at the end of hover_matrix_tests.rs (combined_hover_*); fixed: a
   sketch click took the edge under a lit model vertex (sketch.rs lit_vertex_at); harness: Matrix::reset now clears
   SketchEditor::model_selection. interaction-feel.md rows filled; left "? (qa)": Extrude region picked hovered,
   face-selected + sketch line, Extrude New/Remove preview cells (no harness: start sketches only on planes).
   Known: Shell's wall inner edge lights nothing (live: the removed face) with equal pixels (preview_hides by design).
3. DONE (730f1acd3) EXPORT-MULTI-ROW-INDIVIDUAL: open_part_export parts_only = one row only.
4. Triage after count 13/15, 241/247 (vibe2 r20; before 10/15, 235/247); lead + qa told. plane-* (5) match on
   green (qa uip5-1125): no re-record. Left:
   - copy-paste step 12 FIXED cf0c044af (a clicked, unmoved Transform arrow opens no "0 mm" box; T1
     a_clicked_transform_arrow_opens_no_box); vibe2 run22: scene lib ok, clippy ok, copy-paste 14/14 + menu-transform
     11/11. Landed with dc6768f9/9416858b (re-queued stack 11-26).
   - fully-defined step 13 FIXED (sketch.rs PointerMove by_outline): with entities picked for a dimension, a move
     re-resolves the plane outline whenever the GPU pick is a plane or nothing (it only did with an ellipse picked);
     the runner glides in, so the GPU pick (own plane) stood from 2 mm short and the 1.32 px outline never lit. T1
     a_click_on_the_right_planes_outline_... now follows the scenario (scale, 30 mm dimension, glide; fails without).
     vibe2 r42: 83 hover/dimension/plane tests ok, clippy ok, 10 sketch-dimension scenarios 140/140.
   dc6768f9 "failed" 06:05 = land daemon SIGTERM when skasm's push under it retried (no test failed); re-queued 2de8abe3.
   Item 4 done: 15/15 expected once 2de8abe3 + the fully-defined fix land (qa to confirm).

## TICKETS pass (lead 11-26 06:4x), QUEUED with the fully-defined stack
1. UIS-SELECT-ITEM-ROWS + UIS-CONFIG-BOX-FIELD: marks list_row:<name> (select_item.rs row helper) and field:<input
   name> (config_panel.rs cfgbox-val). Marks exist only under NOSHAPE_UI_SCRIPT: no T1; verify = qa re-runs its
   speed cells 86 / 88 (qa-scratch/hm2/wf, qa's own runner).
2. UIS-STATE-GEOMETRY: both sides done (ours sketch.geometry, comparer same_geometry, live run.mjs since 11-24); no
   take in tests/ui-parity/live has it yet. Marked fixed, to verify.
3. DL-IMPORT-MIXED-STEP-STL-OPTIONS fixed: import_dialog.rs mesh_options (any STL/OBJ: Units, no Join); unit test.
4. EXPORT-ASM-STEP-MBD-FIELDS was already fixed (0207b943d). 5. SKETCH-DRAG-FILLS-HOVER (skasm 50ba91aca),
   IMP-TOAST-QUEUE (1567cf00a): marked fixed, to verify; duplicate union rows dropped.

## P0 (lead 11-26 06:5x): assembly hover in combined states
- Live answered (live/psui/asm-hover-combined-1126, behaviours "Assembly hover in combined states"); the table in
  interaction-feel.md "Combined states in an assembly" has live / ours / status per cell.
- T1s: assembly_feel_tests.rs (combined_hover_*, a_hidden_instances_hovered_row_outlines_it,
  a_re_hovered_second_pick_shows_plain, a_selected_instances_hovered_face_lights_its_boundary,
  after_an_explode_step_its_instance_stays_selected_and_unlit_under_the_pointer) and hover_matrix_tests.rs
  combined_hover_mate_connector_in_a_mate_dialog. add_exploded_view refits the camera and the Explode panel narrows the
  test viewport to 249 px: fit after a settle; clicks at the viewport's top 8 % land on the edit banner.
- Fixed 11-26 (07:1x) Alt no longer picks Isolate / Make transparent ghosts; (08:2x) the 4 P0 rows:
  hidden row hover outline (`asm_scene::outline_hidden`: the hidden instance's mesh under one HIDDEN|HOVER|OUTLINE state
  slot at picks.len()+1, overlay ids moved up by one; fs_hover_mask keeps HIDDEN), a re-hovered second mate pick plain,
  a selected part's hovered face boundary #ffc368 (BODY_HOVER_EDGE), explode edit: the selected instance unlit under the
  pointer and an empty click keeps the selection.
- Rows 1-4 QUEUED 73236c4a (11-26 08:30, full assembly-ui/scene/render + clippy green); with mate preview (4) QUEUED 800c7338. 08:44: mate preview (4) fixed:
  the part a mate's preview moves lights nothing under an arriving pointer (`previewed` in the hover filter and
  `moving` in the inference block; `still` = the pointer still at the second pick's click keeps the old look);
  a_re_hovered_second_pick_shows_plain compares all states away vs over it (fails without the fix). Landing.
- Open: ASM-SELECTED-HOVER-LOOK's arrow (ours: whole triad, hovered handle flat #3e94ff; live c1a shaded #638abf):
  live REQUESTS psui 11-26 08:20. Mate preview (4): the part under it (Part 1's front darkens #4e6876 > #14374c, amber
  #dbad20 > #e5b826) not compared. Explode before a step: #ffc685 = our standard hover (`combined_hover_while_editing_an_
  exploded_view`), done.
- 11-26 07:2x the psui worktree directory was gone (branch agent/psui intact at 54dbd7bc0); recreated with
  `git worktree add /root/projects/noshape-wt/psui agent/psui`.

## Responsiveness audit (lead 11-26 08:5x): DONE, landing
- `tools/perf/ui-audit.sh <app> <display> [rounds]` (scenarios tests/perf/ui-audit: 30-feature part + a 20-instance
  assembly, 60 timed actions); `tools/perf/ui-audit-medians.py` fails on a median first frame > 100 ms or UI CPU > 50 ms.
  Driver (crates/app/src/ui_script.rs): `timing.cpu` (UI-thread CPU input → first frame end), a hover timed from its
  last move, a drag from its release (both untimed before).
- 11-26 09:31 result (vibe2, ui-bench app at ~/agents/psui/target/ui-bench/noshape, Xvfb :151 left running): nothing
  over 100 ms, UI CPU ≤ 28 ms; table in interaction-feel.md "UI responsiveness audit". Nothing to fix; on Xvfb wall
  times are mostly software present / core waits (`ui stall` lines all ≤ 30 ms UI CPU).
- Run: rb.sh on vibe2: `cargo build --profile ui-bench -p noshape --bin noshape`, then from the src root
  `~/bin/slot bash tools/perf/ui-audit.sh $HOME/agents/psui/target/ui-bench/noshape :151 3 ~/tmp/psui/auditN`.

## Drawing triage + asm shard + qa 12:3x tickets (lead 11-26 09:3x / 10:50 / 12:2x): QUEUED 94d158aa 11-26 14:2x (lead + qa told)
- Baseline 1125-1501: drw 382/457, 53 maker-scope failures; asm shard (green bc0fc23b8) 212/225. After: the 87
  failures rerun 79/87 (drw 70/75), the 77 Esc/radial drw 75/77, then verify3-6 (~/tmp/psui/ver3..ver6 on vibe2):
  ver6 13/14 (114/116 states); drawing-ui 214, assembly-ui 228, scene 259, ops e2e 312 all ok.
- In the stack: one Esc ends dimension-type tools; reader_artefacts; radial text halfway for a click within r/2;
  foreshortened radius reads "dimension" (a foreshortened diameter isn't read); ordinate press point; circle
  centerline quadrants (within 3 px, from the hovered circle) + 2↔3 point tool switch (was toggling off); Remove
  foreshorten clears the selection; run.py click_placed_points (2 px, DECISIONS 91aa2f213), button_focused,
  created_feature_owner; Shell too thick blames `entities`; mate: × chip button, row red while dialog red, REST status
  stays OK, × is an Undo step, edited mate keeps its flip icon; asm Where used / hidden tooltip / select other /
  Parasolid 38.1 seed; Import Units; Tangent edges greys the current style; Isometric menu has Rotate view, no
  Suppress alignment (views::is_isometric / views::turns_freely, ops uses it too). 324fcaa20 dropped (skasm bf9f9f32).
- Open: drw-dimension-foreshorten s10: ours reads EDGE+ (the menu-row hover rule, drawing_ui_state ~l.690) where
  live's 11-26 take reads nothing; mistake-extrude-open-sketch s17 (+ mistake-sketch-gap-profile-extrude, qa): our
  Extrude row isn't registered while its dialog is open; drw-geometric-tolerance (REQUESTS 12:24); asm Measure panel
  reader; asm-drag-revolute-turns (REQUESTS 11:33); radial drag rule. SKETCH-CLICK-OFFSET is qa/live's now
  (SKETCH-CLICK-ONE-PIXEL).

## LANDING 11-26 16:1x (lead 14:4x: callout widths + the pixel item), commits after the 94d158aa stack
- f0424bccc callouts: ⌴ ⌵ ↧ advances fitted to live's boxes (⌴/⌵ 2.35 h, ↧ 1.47 h, strokes centred; font.rs),
  HoleCallout lines centred on the widest, pitch 5/3 h (sheet.rs). T1 drawing::tests::hole_callout_boxes_as_live.
  verify8: drawing 188, model-data 47, drawing-ui 214, ops e2e 312 ok; 25/25 callout + hole-table scenarios.
- b68f72d9f pixel (SKETCH-CLICK-ONE-PIXEL): takes recorded >= 2026-11-26T14:05 replay at their own `at`
  (NOSHAPE_UI_CLICKS_ROUND + NOSHAPE_UI_LIVE_AT, app ui_script.rs `whole`); older takes floor(projection) as before
  (lead approved). verify11: 18 new takes + 100 sampled ps/sk/asm: new 111/118, 1000/1022 states; sample new = base
  (99/100, 755/757). Remaining differs pre-existing: extrude-open-sketch s17, dimension-construction s11,
  gap-profile-extrude s18, mirror-no-line s11, offset-negative s16, asm-measure-face-cylinder-panel s4.
- drag-fixed-line s5: real app difference, filed SKETCH-ISO-ORIGIN-ALIGN (TICKETS top).

## LANDING 11-26 17:1x (lead 16:0x items)
- 3d3e59398 drawing: a view under a clicked menu row reads lit only within 6.5 px of its box's edges (inside too;
  drw-dimension-foreshorten s10). T1 drawing_ui_tests::a_view_under_a_clicked_menu_row_reads_lit_only_near_its_edges.
- b4ba08978 EXTRUDE-OPEN-SKETCH-FOCUS: the editing placeholder row is wrapped "feature:<name>" (part_studio mod.rs
  render_editing_placeholder); run.py button_late holds through hover steps (README Focus).
- verify12 (81: every drw take with a context-menu row click + note-move, surface-finish-flip, view-align h/v,
  foreshorten, the two extrude takes from 2ff0dfee checked out then restored): base 78/81 -> new 81/81, 1011/1011.
  drawing-ui lib tests 193 ok.

## LANDING 11-26 19:09, c76091651 replay + 72dfe3dd4 gdt (lead 17:5x: gdt T1 + replay checks, commit, ops e2e, land, report)
- Drawing rerun (verify13, ~/tmp/psui/ver13n|ver13g, names13.txt): this morning 75 failed; green 91c15afe9 81/86
  (814/828); branch 85/86 (830/834). The one left, drw-geometric-tolerance, matches now (below).
- REPLAY APPROACH (lead 17:2x): run.py live_pointer_from (where live's pointer rested before each step: a key step's
  pointer.at, e.g. parked [1889,932] by run.mjs l.49, else the last click's at) -> NOSHAPE_UI_LIVE_FROM; ui_script.rs
  live_from(): jump there first, then 8 moves to a sketch_point (run.mjs l.288), else 3; a menu row is reached from
  our own pointer (the jump swapped the open submenu: folder-menu-collapse-then-expand-all s10, 3/3 runs). No ~1 px
  vertical offset (107 clicks: dy mean +0.08, |dy| <= 0.46 px). A/B (verify15, names11 = 18 takes since 14:05 +
  sample100 + 2 gdt): without 114/120, with 113/120 (only the folder-menu s10, fixed after); drag-fixed-line matches
  both ways. verify17 (menu fix, ~/tmp/psui/ver17n): 116/120; left: 3 sketch mistakes (skasm's e709bc2d1 covers
  mirror-no-line, offset-negative; dimension-construction s11) + asm-measure-face-cylinder-panel (Measure reader).
- Tests: drawing-ui 215, ops e2e 313, clippy (drawing-ui, app) clean. QUEUED 4bfc0d3a; lead told 19:1x.
- 19:29: assembly Measure panel reader (measure_panel::ui_state, other_dialogs; T1 the_panel_reads_as_live_recorder_reads_it):
  asm-measure-face-cylinder-panel 7/7, asm-select-two-faces-measure 3/3, key-bracket-measure 4/4; assembly-ui 233.
- GEOMETRIC TOLERANCE (live 7062): empty slots: the edge lights and reads as selected, its click places nothing and
  starts a leader (the edge stays lit), the next click places ONE ⌖ frame with it, Esc keeps it; edge click + Esc
  places nothing. gdt_empty removed; drawing_ui_state.rs Tool::Gdt lights the hovered / leader edge. T1
  an_empty_geometric_tolerance_places_one_frame_after_an_edge_click (replaces feel::an_empty_..._places_nothing).
  App: drw-geometric-tolerance 6/6, -edge-then-escape 9/9 (with the takes of live's 6b1c85cd4, not on main yet).
  Our listed position (82.9, 93.0) vs live (84.5, 94.1) is within the comparison's tolerance.

## Platform check (lead 11-26 20:5x): DONE (22:45)
- Sample: 100 fixed scenarios (25 drw / asm / sketch, 8 shortcut, 17 ps; /root/psui-scratch/resp/os-sample.txt, seed
  1126), release v0.4.72 on each OS. Linux (vibe2, os-lin.sh): 94/100 (6 drw "tool stays after Esc", fixed after the
  release). Mac (gated: idle >= 600 s, load < 6, one app at a time, <= 175 s; os-mac.sh in ~/noshape-host/psui-keep):
  91/100, the same 6 + 3 Mac-only, all runner artefacts, fixed (commit "ui-parity on a Mac"): redo mod+y -> Shift+Cmd+Z
  (Cmd+Y redoes only in a drawing in live), asm-circular-pattern-angle-count's ctrl+a -> mod+a, run.py's stderr pipe
  filled and stopped the app (now a file). App behaviour: no Mac-only difference found.
- CI (qa-os.yml input `uiparity`, run 45901651578): Windows 78/100, macOS runner 84/100. The 17 differences not on Linux
  all came from the runners' 1024 x 768 screens squeezing live's 1919 x 992 window (drawing toolbar overflow, menus
  and camera moved). At 1920 x 1080 (Set-DisplayResolution / displayplacer; probe runs 45905022971, 45906424638 on
  agent/psui-osci) every one matches: no Windows-only difference. Workflow step landed ("1920 x 1080 screen").
  Rerun: gh workflow run qa-os.yml -f tag=<tag> -f uiparity="$(tr '\n' ' ' < os-sample.txt)"; artefacts qa-<OS>/uip.log.

## Depth batches 1 + 2 (lead 11-26 22:5x: replay live's option-combination takes myself; qa skips them): IN FLIGHT
- 61 takes (d02aa4898 + 9b9058e66 on main; names /root/psui-scratch/resp/depth-names.txt; vibe2 ~/tmp/psui/depth,
  script resp/depth.sh, diff helpers /root/psui-scratch/bin/stepdiff.py, sumdiff.py, focusafter.py): 37/61 on db5d67ef1+.
- Fixed (commit "a Hole's focused icon box"): focused icon box has no label; Hole type pick leaves no focus. T1s
  a_holes_focused_depth_box_has_no_label, a_hole_type_picked_leaves_no_focus. Hole replay (30 takes): 21/30, the other 9 are the remembered values only (incl. tapped depth 5.6 mm).
- Waiting on live (REQUESTS 23:0x): (1) keyboard into the selection field a control shows: all 10 runtime-0adacf98
  takes (since 11-21) move it, the older takes with the same control and start keep it on the control (5 retakes asked).
  If confirmed: in focus_new_empty_field / the `takes` rule (feature_dialog.rs ~690-725) a field the edit shows and
  activates takes the keyboard from any control; covers boolean offset x2, circular / mirror / linear feature type,
  fillet-variable (its vertex pick follows), linear second direction; old takes then need the retakes. (2)
  mistake-extrude-remove-flipped-out ✓ not closing on an INFO. (3) remembered seeds (chamfer 16 mm, fillet 2 mm,
  cbore 9.75, csink 11.2, Metric Tapped M5) for 8 takes.
- core: DEPTH-EXTRUDE-UP-TO-PART-OVERLAP, DEPTH-PATTERN-REMOVE-OWN-PART (TICKETS 23:0x).
- Boss-top pick FIXED, landing (commit "over a face an edge lights from 6 px"): over a face an edge's reach is 6 CSS px
  + half a device px to its drawn pixels (viewport.rs EDGE_OVER_FACE_REACH, was 7; live mc-reach-1122 R4: lit 6, face at
  7; the Ø8 rim is 7.26 px from the top's middle at 0.275 mm/px). T1 a_click_on_a_boss_tops_middle_picks_the_face; render
  offscreen reach tests now 5 / 6. QUEUED 39026a6c (00:16). Scenario rerun on branch: match (10 states).
- GPU-only red assembly-ui a_re_hovered_second_pick_shows_plain FIXED (11-27 01:0x, b9b194fcf): the test's own point was
  wrong since 3e5156291: previewed, Part 2 spans x -40..20, y 10..30, so [5,3,10] is bare Part 1; now [10,15,10] (on
  Part 1's top), asserting Part 2's face is hovered. No app change (a depth-rank tweak for the moved part was not needed:
  A/B passes without it). assembly-ui 233 + scene 268 green on vibe2. 39026a6c failed only on this test (01:18);
  re-landed with the fix on top, plus coverage.py name matches (Part Studio menus 129/131; 2 real gaps for live).
- 11-27 lead order: Part Studio shard (1126) rerun, triage, fix ours. QUEUED 6264e254 (01:25: boss-top + asm test +
  coverage.py). Shard on 80c80a0a-ish build (green 7c9124aac + 26): 1084/1126, 9496/9842 (was 95.8; click offsets gone).
  42 failing, triaged:
  * Focus rule (8): boolean-subtract-offset(-keep-tools)-ok, circular-pattern-feature-sketch-axis-ok,
    fillet-variable-two-vertices-ok, linear-pattern-second-direction-ok, loft-guides-finished, mirror-feature-reapply-on-ok,
    mistake-linear-pattern-feature-boss-no-reapply. UNCOMMITTED in feature_dialog.rs set_value: `takes` per control kind
    removed; any field the edit brought up and activated takes the keyboard, except a tab's same-label twin (live 11-27:
    Onshape changed; 5 retakes in /root/live2-scratch/l24/ar/out/psfk-*.json, land with live's depth batch 4; staged
    copies vibe2 ~/tmp/psui/psfk).
  * Remembered seeds (11): UNCOMMITTED "remembered" blocks added to chamfer-distance-angle-ok (width 16), fillet-conic-ok
    (radius 2), hole cbore x3 (cBoreDiameterV3 9.75), csink x2 (cSinkDiameterV3 11.2), tapped x3 (tappedDepthV3 5.6),
    hole-metric-counterbore-through-ok (isoHoleTableEx Tapped M5 0.80 + 5.6). Live's "(3) done" was other takes.
  * Ours, UNCOMMITTED fix: part-menu-edit-material x2: a selected face/edge/vertex no longer hides Copy Part 1 / Where
    used… (mod.rs `others`; features/sketches still do, featurelist capture). T1 row "menu / a face selected, on its
    part" in list_feel_tests.rs.
  * Ours?, sketch cluster (new since 11-26, likely my e3ffb4cef approach replay acquiring inferences): sketch-parallel(-key),
    sketch-constraint-refusals (extra VERTICAL), sketch-mirror-escape-after-line / -undo-midway (extra HORIZONTAL+MIDPOINT),
    sketch-mirror-construction-line, sketch-dim-parallel-lines, sketch-line-started-on-line-off-midpoint-1mm,
    revolve-dialog-axis, sketch-chamfer-finished. A/B with the approach off running (below).
  * Waiting on live (REQUESTS "psui 11-27 02:5x"): mistake-extrude-remove-flipped-out (✓ stays open 3/3 today vs 2979's
    commit on 11-14; guard tried in accept_edit and reverted), camera-menu-perspective-view-off (recorder's orthographic
    always true: artefact), coverage gaps Disable imported appearances / Release all configurations….
  * Not mine: extrude-up-to-part-overlap-ok, mistake-linear-pattern-remove-own-part (core tickets), gusset x2 (ours error,
    live ok: core), mistake-asm-fastened-same-instance (skasm), sketch-image-edit/-place (dialog "Insert an image": skasm),
    sketch-spline-insert-points(-escape) (skasm).
  * RUNNING (background, local log /root/psui-scratch/resp/focus.log; script resp/focus.sh): part-studio-ui lib with the
    focus rule (expect old-rule T1s to fail: update them to the new rule), retakes 5 via --live psfk, 27 old focus
    scenarios (pre-11-21 takes may now differ = stale), the 11 seeds, sketch cluster with the approach off (run.py sed,
    restored after). Started before the part-menu change: rerun part-studio-ui lib after. Then commit, handoff, land.
- Open, mine: linear-pattern-second-direction-ok s16 field Distance#1 (after the focus rule); mirror s8 focus text.
- qa verified RUNNER-UNLABELLED-FIELD-INDEX on c444ac1d5 (flow 70, 0 runner errors).

## NEXT
- 19:50 interaction-feel sweep (lead order 2): rows 964-966, 987-990, 1003, 1031, 1749 marked = with their tests; 1071 /
  ASM-SELECTED-HOVER-LOOK fixed (70fb1b47b: hovered triad handle = shaded blue stroke, MANIP_HOVER, T1
  a_hovered_triad_handle_is_a_shaded_blue_tube), to verify. Still ≠, not mine: 1100 (unverified), 1662 (decision), 1900 (core),
  FEEL-MC-MARKER-SHAPES (skasm). mirror-no-line → skasm. Row 1662 = superset (lead 11-26), T1 units
  live_accepted_inputs_and_our_superset_of_its_refusals. 19:5x RUNNER-UNLABELLED-FIELD-INDEX fixed (Hole icon rows marked field:#n),
  to verify by qa's flow45 rerun. 20:2x-20:4x drawing triage outside the maker scope (lead): 127 in-scope (section,
  detail, broken-out, BOM / hole tables, centerlines / centermarks, sheets, formats) + 178 other drw (no flat / weld) all
  match on branch (green d3abfe79c + 74595efa): 305/305, 3151/3151 states (vibe2 ~/tmp/psui/drw2, drw3). Next: qa's diffs from live's option-combination batches (order 3) as filed.
- SKETCH-ISO-ORIGIN-ALIGN and asm-drag-revolute-turns are skasm's (lead 17:1x / 17:5x). Radial placed inside a hole: both takes match (verify13, branch and green), closed. Queue empty (19:4x). qa's 70 (Hole rows 'Ø 5 mm THRU' / field
  Diameter): answered 19:4x, a flow-script issue (THRU = the account's remembered seed; live's diameter box is unlabelled).

## Agents

- skasm owns the hover matrix's sk/* cells (lead 11-22); psui the 3D / Part Studio cells.
- Agent ids: ONLY in /root/lead-scratch/roster.md (lead rule 11-19); read it before every message_agent, never copy ids
  into notes. Context cap 200k (user 11-19).
- P0 sketch-vertical-key: matches live on green f6f8069f, 10 of 10 states (qa 11-20).

## Open decisions

- None.
