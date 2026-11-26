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

## NEXT
1. SKETCH-ISO-ORIGIN-ALIGN: our iso alignment reach (sketch/src/inference.rs off_y, 7.5 px on screen) vs live.
2. Geometric tolerance once live answers (REQUESTS 12:24; live gdt.md: empty frame silent, Position default,
   leader end snaps to a nearby hole centre).
3. Older: asm Measure panel reader, asm-drag-revolute-turns (REQUESTS 11:33), radial drag rule.

## Agents

- skasm owns the hover matrix's sk/* cells (lead 11-22); psui the 3D / Part Studio cells.
- Agent ids: ONLY in /root/lead-scratch/roster.md (lead rule 11-19); read it before every message_agent, never copy ids
  into notes. Context cap 200k (user 11-19).
- P0 sketch-vertical-key: matches live on green f6f8069f, 10 of 10 states (qa 11-20).

## Open decisions

- None.
