# psui (Part Studio UI): current state

Updated 2026-11-20 03:10 UTC. History up to then: docs/handoff/archive/psui.md (not read by default). Standing rulings:
docs/DECISIONS.md (read first; it wins over anything here).

## Setup

- Worktree /root/projects/noshape-wt/psui, branch agent/psui. Commit as yekta (AGENTS.md), subject "psui: …".
- Builds and crate tests: plain `NOSHAPE_RB_HOST=vibe2 tools/tiers/rb.sh bash -c "$(cat script)"` (DECISIONS 9e5b481d4;
  ~/bin/slot only for suites, gates, timing pairs). Scripts in /root/psui-scratch/run*.sh. The GPUI dialog matrix is
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
  Run: `NOSHAPE_RB_HOST=vibex6 tools/tiers/rb.sh bash -c "$(cat /root/psui-scratch/uip-some.sh)" uip-some <out> <names…>`
  (`~` in remote paths expands locally: pass /home/ngyekta/… paths). Output buffered: the log fills at the end.
- `psui-shards.sh <out> <names file>`: the same in 3 shards, then one `--compare-only` score. The names file must be on
  the host (scp it first). psui-names.txt: the 1021 psui-area scenarios with live takes.
- Compare without rerunning: `python3 tools/ui-parity/run.py --compare-only --out <dir> <names…>` (through rb.sh).
- Runner env: NOSHAPE_UIP_LOGS=1 (app stderr), NOSHAPE_UI_SHOTS=<dir> (a screenshot per step), NOSHAPE_PERF=1,
  NOSHAPE_CACHE_DIR (fresh: cold runs).
- sbs.py: side-by-side crops (ours offset (242, 90)); mirror-sxs/ holds the Mirror side by sides with README.
- vibe ~/tmp is pruned after 6-12 h: keep results in ~/agents/psui-keep.

## Now (updated 03:10 UTC 11-20; this thread replaced 4973e295 at 13:27 11-19, Ubeyd's weekly limit)

1. LANDED: 7c2d7313 as d5472335 (02:40: ERROR-CHIP-RED-NO-FIELD, qa's waiter armed; the 712 px Configurations test).
2. URGENT, NEXT TO LAND: qa P1 ANALYSIS-READOUT-HIDDEN (62ef70f3b hid the bottom-right readout under every analysis
   tool; live only under Interference: `interference_open()` in menus.rs, used by selection_readout and ui_script's
   measure; test measure_feel_tests::only_interference_detection_hides_the_summary; 6 analysis-* scenarios). Tick it in
   TICKETS after the rebase (it is on main only) and tell qa on green. QUEUED alone as 9e20287b (worktree
   psui-readout, --no-precheck, 03:26). qa 04:0x: also edge-menu-curve-analysis step 2 (Length 40.000 mm kept under
   Curve/surface; qa ran a build without 9e20287b): test ::an_edges_curve_analysis_from_its_menu_keeps_its_length in
   the batch (run114, vibex8).
3. NEXT TO LAND (after 7c2d7313): qa P1 EDGE-MIDPOINT-REST-LOOK (TICKETS 5296e9f9c; behaviors `midpoint_rest_edge`:
   rested on the edge, the edge stays lit and the midpoint is a 6 px #ffd3a0 fill in a 7 px dark ring; test
   a_rested_edge_midpoint_keeps_the_edge_lit_with_a_filled_ring; tell qa on green, they re-walk it). Feel-row tests:
   the_working_spinner_shows_900_ms_into_a_regeneration_centred_low (pending.rs "pending-spinner"; the slow case
   drives begin_pending / end_pending: a held Regenerator stalls GPUI's single-threaded test executor),
   switching_configurations_greys_nothing_and_spins_only_past_900_ms, parts_list_properties_put_the_state_pill_on_each_part
   (mod.rs "part-state-{id}"), dialog_field_lists_variables_by_their_word_prefixes (Tab closes the list).
   run100 (vibex8): perspective, mirror, linear pattern, editing matrix pass; fixed since and in run102 (vibe2):
   the skip ring (focus "Instances to skip" after the first draw), the Hole tabs (its value rows have no label: read
   in order), the Hole depth (only "no regeneration" checked), Isolate 198 → 196 px (app-core isolate_ui: the last row
   18 px, no bottom margin, live dom dialog-assembly-isolate; the assembly's too). New in run102:
   dialogs_feel_tests::the_query_variable_dialog_shows_no_count (row 1331), feel_tests::
   a_lit_face_goes_dark_when_the_pointer_leaves_the_window (qa HOVER-LEAVES-WINDOW; GPU, skips without one).
   On landing also tick in TICKETS (fixed earlier, never ticked): TRANSFORM-GHOST-LOOK (row 1148), SECTION-PREVIEW-
   COLOUR (1162), HOLE-CALLOUT-PLACE (1464), FLFEEL-BAR-MENU (list matrix "rollback bar / its menu while rolled back");
   FEEL-FAST-OFF-CLEAR: the clear is on the second fast move, the rest 0-40 ms frame latency, under DECISIONS'
   100 ms bar (11-18): ask qa to close. vibex8 is qa's until 04:00 UTC 11-20 (nightly suite): no builds there.
   Added 02:5x (run105 / run106, vibe2): the chip tooltip right of its chip (dialogs matrix, row 1399), a committed
   failing Fillet remembered (873), the Linear pattern preview blended at 0.7 (1457), the configure toast's × (1274;
   os_ui "bubble-x" selector), the Configuration select at (42, 36 under the graphics) 193 × 26 (1268), the rollback
   bar grey / #c8d9e2 hovered (1045 / 1070: `ui.rollback_hovered` by on_hover instead of group_hover;
   `rollback_bar_colour`), the spinner's arcs and 1.1 s turn (app-core `spinner_arcs`, 1077), Form's Location(s) left
   empty by a face click (1415); row 1412 cites the handle colour test. Added 03:0x (run108): sketch_ui_tests::
   a_hovered_edge_tints_no_list_row (1683), scene builder sketch_look_tests (567 / 572 / 573: sketch colours, widths,
   construction dash-dot).
   Restored: perspective_and_render_modes_switch_in_one_step (Window::dispatch_action is deferred:
   read the state in the next update), a_skip_ring_lights_and_a_click_skips_its_instance_then_keeps_it ("Instances to
   skip" clicked first: an edit dialog opens on Entities). New P0 rows: dialogs_feel_tests
   a_mirror_across_a_side_face_adds_into_its_seed, a_linear_patterns_instance_previews_once_its_direction_is_picked,
   the_hole_tabs_bring_their_rows_and_values, a_typed_hole_depth_waits_for_enter; editing matrix "Shift+Y on a hidden
   part's selected row", "empty-space menu with a part hidden"; list matrix "menu / one sketch"; dialogs matrix row
   click + Delete / Backspace, row hover tooltip text; Isolate's place (behaviors "ps-isolate" selector); the configure
   toast; the section value box + typing (run101). Rows 1208 / 1210 / 1211 now cite existing app-core unit tests.
   Skipped: 1199 (device px needs an app-core refactor), 1221 (no live inch readouts), 835, the speed rows.
3b. live cov-1120 (12 feel rows, behaviours "feel coverage (11-20)", interaction-feel "cov-1120"; run109 on vibe2 03:2x):
   tests FS typing_pairs_nothing_and_enter_after_a_brace_indents_without_closing_it, VS
   a_double_clicked_value_edits_all_selected_esc_reverts_a_click_away_commits / the_value_cells_hash_list_holds_the_variables_above_only,
   model sheetmetal_frames::partial_flange_bound_handles_as_std. S1 fixed: the Flange evaluator adds std's partial bound
   handles (`partial_flange_manipulators`, model-core sheetmetal_flange.rs, skasm's file: tell skasm on landing). S1's look
   (uncommitted after a269779ee, run111): std's TANGENTIAL style as `FeatureManipulator::Linear { tangential }` (custom
   host status.rs reads / writes it), drawn by `tangent_point_polylines` (partial Fillet too). Tickets filed:
   FS-WHEEL-72PX, FS-BRACKET-BOXES [studios], ASM-DRAG-START-15PX [asm]: tell them on landing.
3c. live round 2 (R7–R9, behaviours "feel coverage round 2 (11-20)"): R7 row 1025's live time now 106–151 ms; R8 the row
   under a dragged rollback bar lights (list_hover set from the drag's move; list matrix "rollback bar / while held");
   R9 list matrix "menu / the Top row's Section view… opens on Top" (`section_chips` pub(super)). Run in run114
   (vibex8; vibe2 at load 30 left run112 in the slot queue 22 min).
   Tickets for studios also FS-CLOSE-BRACE-OUTDENT, FS-TRIPLE-CLICK-NEWLINE (run110 found them).
3d. live drift-1120 (behaviours "feel drift retake (11-20)"): toolbar tooltips at 20 ms cold or warm
   (`ui::TOOLBAR_TIP_DELAY` in with_rich_tooltip / with_tool_tooltip; other tooltips keep theirs), longer help 1800 ms
   after showing; chrome matrix rows; hover row = (feel_tests::a_fast_move_off_a_lit_face_clears_it_on_that_move).
3f. qa P0 FEEL-ROLLBACK-DROP-ROW (lit row = R8; tooltip 3 px under the bar, none on the dropped bar:
   `ui.rollback_tip_off`) and FEEL-SECTION-ARROW-FOOT (`SECTION_ARROW_FOOT`), 51d079b3c: tick + tell qa on green.
   The 5 'unrelated' run114 failures were my own 1492bd286 / a269779ee tests: fixed (toast names "Configuration",
   one-sketch menu has Show all sketches with Sketch 2 there, skip field focused after the dialog settles, config place
   check dropped: live's x 42 includes a left rail we may lack — open) and HOVER-LEAVES-WINDOW in gpui-pre
   (`Window::pointer_out`: no hit test after MouseExited until the next move). run116/117 fixes: section test grabs the head
   (45 px), skip ring focuses the dialog as the array field's click, empty new folder collapsed (live
   featurelist-gaps emptyFolder; model-data); run118 (vibex8). Rollback row tooltip after a drop: REQUESTS row 04:51.
3g. Batch fcf678ce FAILED 05:42 on studios 3a462952; rebased on it 06:12 (feature_studio_ui_tests.rs: both sides'
   tests kept) with fa765f055 on top: no tooltip on the dropped-on row (`ui.rollback_drop_row`), feel-row tests (3D
   clicks, sketch Escape/double-click, circle/spline colours, snap glyph (30, 30), line/point reach), live's P2s
   PARTS-LIST-PROPERTIES (PN pill 20 px, matrix row) and TABLES-STICKY-HEADER-HSCROLL (Cut list per feature via
   `table_section`, sticky header from `ct.panel_scroll` + `grid_tops`, `h_scrollbar(16, 18)` pinned; the grid is
   flex_none so wide custom tables scroll sideways at all). run128 green. On green: tell qa (DIM readout, rollback
   drop row, section arrow foot, midpoint rest, leave-window, fast-off clear) and studios/asm/skasm as listed below.
3e. qa P0 FEEL-DIM-PICK-READOUT (15e785522, live R3 cov2-dim-orient): the sketch readout reads the Dimension tool's
   picks (`SketchEditor::readout_entities`); sketch_ui_tests::the_dimension_tools_picks_fill_the_readout. Tick it and
   tell qa on green. run114 (vibex8) also showed 5 part-studio tests failing that this batch doesn't touch (config
   toast "Configuration", config select place, one-sketch menu rows, skip ring, leave-window): run115 re-runs them alone.
4. fillet-chip-clear-missing / fillet-edit-cancel-keeps retaken by live (e4ff71a8): both match on green 08525ecd
   (20/20 states, vibex6 02:4x). Waiting on others: ver-compare-hover-part (history-ui) with the lead.
5. Interaction-feel rows that match live with no test (grep "missing" in the Test column; P0 sections first).
   Skipped: the two-block box counts (live's camera: iso + 10 wheel steps about an unknown pointer), the rollback
   bar's hover colour (GPUI group_hover, no test hook); [skasm]-tagged rows; qa's speed-suite rows.
   Asked of live2: the diagonal submenu move's timing (REQUESTS 16:37).
6. Not to build: 3D Sketch, Fault analysis. Cleanup: vibex7 ~/tmp/psui/priv/dock.noshape when the tab work is over;
   vibe2 ~/tmp/psui/hv binaries. Branch psui-backup-1119 (all landed; delete).

## Agents

- Agent ids: ONLY in /root/lead-scratch/roster.md (lead rule 11-19); read it before every message_agent, never copy ids
  into notes. Context cap 200k (user 11-19).
- P0 sketch-vertical-key: matches live on green f6f8069f, 10 of 10 states (qa 11-20).

## Open decisions

- None.
