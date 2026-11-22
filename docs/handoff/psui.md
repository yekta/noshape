# psui (Part Studio UI): current state
Updated 2026-11-22 04:10 UTC. History up to then: docs/handoff/archive/psui.md (not read by default). Standing rulings:
docs/DECISIONS.md (read first; it wins over anything here).

## Setup

- Worktree /root/projects/noshape-wt/psui, branch agent/psui. Commit as yekta (AGENTS.md), subject "psui: …".
- Builds and crate tests on vibe2 or vibex8 (lead 11-22: no waiting on vibe2's one slot): `NOSHAPE_RB_HOST=… tools/tiers/rb.sh bash -c "$(cat script)"`,
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
  Run: `NOSHAPE_RB_HOST=vibex6 tools/tiers/rb.sh bash -c "$(cat /root/psui-scratch/uip-some.sh)" uip-some <out> <names…>`
  (`~` in remote paths expands locally: pass /home/ngyekta/… paths). Output buffered: the log fills at the end.
- `psui-shards.sh <out> <names file>`: the same in 3 shards, then one `--compare-only` score. The names file must be on
  the host (scp it first). psui-names.txt: the 1021 psui-area scenarios with live takes.
- Compare without rerunning: `python3 tools/ui-parity/run.py --compare-only --out <dir> <names…>` (through rb.sh).
- Runner env: NOSHAPE_UIP_LOGS=1 (app stderr), NOSHAPE_UI_SHOTS=<dir> (a screenshot per step), NOSHAPE_PERF=1,
  NOSHAPE_CACHE_DIR (fresh: cold runs).

## Now (06:20 UTC 11-22): lead order: export_solid.json flake, qa P0s (modal hover, vertex dot), workflow P0s, P1 arrow
LANDED 768e0e85 05:29 as c70136f5 (with 2bc17f3e, 0fc65a29, 30d77c0f, 38229ff2); lead told 06:35.
IN WORK (uncommitted, one batch): t27 on vibe2 06:27: 152/155 (3 mine, fixed: modal rule scope, line-end row colour,
config Enter refocus via cfg_enter_next) + scene 200 + clippy clean; t28 (run-t28.sh: feel::, sketch_lineend,
configuration, the two new tests, clippy app-core/psui, app ui_script + clippy -p noshape) on vibe2. Green: commit
"psui: ...", land.sh, tick TICKETS rows, tell qa + lead (matrix score). Lead 06:10: vibe2, don't end the turn on runs.
- Flake (core's ed773d89 land): export-dialog tests shared config_dir/export_solid.json with parallel binaries. ops
  export.rs SolidExportPrefs::use_file (thread-local file); tests' OwnExportPrefs guard (sketch_ui_tests.rs).
- FEEL-PATTERN-FAIL-EDGE-RED + PATTERN-FAIL-PREVIEW: mod.rs failed-pattern branch tints only the seed; pattern.rs
  apply_tools: all copies off the part -> ev.error_shapes = the copies (red ghost). Test hover_matrix
  a_failing_feature_patterns_direction_stays_orange_and_its_instance_shows_red (t19 green).
- FEEL-ROW-HOVER-IN-OWN-DIALOG: behaviors.rs row_hovered_feature; test the_edited_features_own_row_hovered_lights_nothing.
- FEEL-MENU-EDGE-STAYS-LIT + MENU-HIDE-PART-ICON; FEEL-EXTRUDE-ARROW-HOVER-LOOK; MEASURE-VALUE-CLICK-PIN (part).
- FEEL-HOVER-AFTER-MODAL: viewport.rs hover_waits_at: back over the graphics with no move (a cover went) lights
  nothing until a move; was a re-pick (1dd4033be, meant for view keys, which InputModalityIndependent already keeps).
  Test feel a_modal_closed_over_the_graphics_lights_nothing_until_the_pointer_moves (+ the view-key test re-run).
- FEEL-VERTEX-SELECTED-HOVER-DOT: shaders.wgsl: a lit Part Studio sketch dot (style 128, rest 4 px) at least 11 px, as
  centres. Matrix rows sketch_lineend_hovered / _selected_hovered (ring_of 4-5 px).
- EXTRUDE-ARROW-GONE-WITH-VARIABLE: manipulators.rs edit_scope (scope_variables at the edited feature: Variable
  Studio tables) for every manipulator value; test an_extrude_depth_from_a_variable_studio_keeps_its_arrow.
- FEEL-QUERY-PART-PICK-FILL: mod.rs untinted_parts also while a feature dialog is open (outline through, no amber);
  test a_part_in_a_dialog_field_is_outlined_not_tinted.
- SKETCH-LINE-ON-FACE-HIDDEN: builder.rs a shown (not edited, not through) sketch's curves get a dashed hidden_only
  copy; test a_shown_sketch_behind_a_part_is_dashed.
- CONFIG-NAME-FOCUS-BOX: config_panel.rs the add row's focused Name input gets a 2 px #1651b0 box overlay (live
  sampled); test the_configuration_add_rows_name_has_the_focus_box.
- UIS-CONFIG-ROW-MENU: config_panel.rs config_row mark on the Name cell (live clicks there); app ui_script.rs
  menu_row_matches (a quoted name after the menu name matches); test ui_script::tests::a_menu_name_matches_as_lives_runner.
  t24 (run-t24.sh: configuration + app ui_script tests + clippy app) after t23.
- 06:10: vibex8 (no GPU) too slow and 15 failures: t26 found the modal rule blocked hover on first entry and two tests
  wrong (Boolean single tool fails; Extrude had no region). Fixed: viewport `covered` (hover lost with the pointer in
  bounds) + `last_input_move` (window capture listeners) scope the wait to a cover closed by a click/key; tests use
  Mirror and a picked face. t27 = full set on vibe2 (lead: GPU tests ran green there).
- Prepared, apply after the batch commits: /root/psui-scratch/next/helix-tint.py (dialog face SELECTED|TINT + shader,
  tests a_face_in_a_dialog_field_is_lightly_tinted, parts_in_booleans_tools_stay_untinted_under_the_preview),
  rgb-scroll.py (vendor gpui-base on_blur reset_scroll_to_start + test), split-keep.py (SPLIT-KEEP-TOOLS-AUTO test;
  std splitEditLogic is in STD_EDITING_LOGIC: find why it doesn't tick). Helix arrow direction = the revolve axis
  (core/skasm, told lead 06:35); live's "start disc" is the world origin drawn through the part (ours occluded).
  Open: in the Part Studio a click on a sketch line's free end seems to select the line (its band under the lit end):
  check live (hover_matrix sketch_lineend_selected_hovered now checks only the 11 px size).
NEXT (qa wf21, green b5b414278; crops qa-scratch/tickets/wf21-helix.png, wf21-parts.png, live left): P0
FEEL-HELIX-FACE-PICKED-LOOK (a cylinder face picked into Helix: live faint tint (141,152,137), ours tan (182,150,73);
Helix arrow: live mid -> down to a start disc at the bottom, ours up at the top, no disc); P1 PARTS-ROW-OF-PICK (Parts
list: the new part bold in Revolve's preview; a grey row + blue underline while its face/edge is in a field). qa wf28
(live/workflows-1122/28, crops qa-scratch/tickets/wf28-*.png): P0 FEEL-SECTION-PLANE-LABEL-LOOK (Section view, Right
picked: our plane labels pale mauve (138,117,129), live solid blue (39,94,181); our part edges ~3 px dark, live 1 px);
P1 APPEARANCE-RGB-FIELD-SCROLL (R reads "00" after mod+a, 200, Tab; live "200"); note: right-click 7-8 px from Sketch
3's line takes the line, live the face (edge of reach). qa wf29: P1 FEEL-PLANE-LIT-AFTER-MENU (Paste sketch from the
Top row's menu: ours lights Front under the pointer where the menu was, live nothing; wf29-paste.png row 7): likely
the same path as FEEL-HOVER-AFTER-MODAL (hover_waits_at): check once landed. qa wf38: P1 SPLIT-KEEP-TOOLS-AUTO;
FEEL-QUERY-PART-PICK-FILL still on green 984adcc30 (fix not landed; wf38-bool.png steps 5/28/29/35). qa wf36
(wf36-measure-decimals.png, live left): P1 MEASURE-READOUT-VARIABLE-ICON (status readout: live's (x) after the unit);
MEASURE-VALUE-CLICK-PIN in the Measure panel too (click Min dist: live's row light blue, value full precision 39.05125
selected, a chip above; ours nothing). qa wf40 (wf40-*.png): P0 FEEL-SKETCH-RCLICK-PLANE-LIT (right-click empty
sketch space: live lights the sketch plane orange while the menu is open); P1 CTX-MENU-FLIP-UP (a menu that doesn't fit
below opens above the click; ours slides up under the pointer / over the tab bar; drawing view menu opens a submenu at
once). qa wf33: P1 FEAT-MISSING-REF-CHIP (lost reference: live's chip red "Missing Edge of Extrude 1", menu Clear
all / Clear missing references / Edit healthy moment; ours plain, Clear all only; wf33-fillet-fix.png); wf37: P2
EXPR-AUTOCOMPLETE-LOOK (suggestion rows/position, wf37-*.png). Then the
tooltip-after-press opt-out (below), UIS-WORLD-TARGET P2. Crops qa-scratch/tickets.
OPEN: tooltip after a press (row name tip, measure chip, FEEL-ROW-DEPENDENCY-TIP-IN-DIALOG): gpui-pre div.rs
TooltipPressGate needs a per-element opt-out restarting the timer at release. Asked live: SKETCH-ON-FACE-EXITED-LOOK,
MEASURE-PANEL-POSITION. SKETCH-DIM-PLACE-PICKS-PLANE: qa re-scores with 30d77c0f.

## Before (02:54 UTC 11-22): USER P0, the hover x selection look matrix + qa's walk tickets
LANDED: batch 1 (5b7743f97), batch 2 (6be52caf), batch 3 666d9c47 as f411d924 (qa re-score 4/15, matrix 94/125).
QUEUED 0fc65a29 (03:0x, 3 commits: walk 6, batch 3/walk 7, walk 5 + 8 below); lead + qa + live told.
QUEUED 30d77c0f (03:12, carries 0fc65a29; run-t13.sh green): FEEL-PLANE-LIT-WHILE-DIMENSIONING per live dimplane-1122 (behaviours "psui plane
while dimensioning"): with Dimension picks a plane lights by its outline only (sketch.rs sketch_hover -> plane_by_outline,
reach DIM_PLANE_OUTLINE_PX 2.5, 6 with an ellipse); test sketch_hover_feel a_line_picked_for_a_dimension_lights_a_plane_
only_on_its_outline. KEYS-SHEET-DETAILS: "select rollback bar" a keycap (shortcuts.rs locked_row; SheetRow.prefix gone).
Fillet arrow anchor pin test (hover_matrix the_fillet_arrow_stays_at_the_edges_midpoint…; lead re-raised it, told closed).
SPEED-HEAVY-HOVER closed (Mac GPU: plate 22-86 ms, parts-1000 21-69 ms move-to-present vs live 42-53 / 265-270;
kit ~/noshape-host/psui-keep run.sh / run2.sh ps1000, local psui-scratch/mac). TICKETS dup headers dropped.
LANDING (03:36, after 30d77c0f): lead order 11-22 03:2x: (1) qa 1121-0935 full run's Part Studio failures: all 9 left match
on green e97ac7f1c (uip-green.sh on vibe2, ~/agents/psui-keep/noshape = /root/green copy; the 15 Section view rows fixed
earlier); (2) missing-commands-1119: no psui row open; (3) dialog specs vs std 3132: Jog, Tag reordered (82/86; specdiff
e97a in psui-scratch/dlg); Hole = live's legacy/V2 fields (ours is V3), Import, Recognize (live asked), Frame/Tag errors
explained in feature-dialogs.md. qa P2 UIS-EMPTY-NON-PS: empty outside PS/asm = (1560, 960); ace_word in FS.
WALK 5 + 8 BATCH (part-studio-ui hover_matrix/menu/midpoint/snaps 116, list:: + dialogs + ui_script + assembly-ui
green on vibe2, run-t9.sh / run-t10.sh):
- live psui walk 5 (behaviours "psui walk 5", live/psui/walk5-1121/): in a sketch with no tool a part edge's midpoint
  lights first (behaviors.rs update_hover; mod.rs per-move recheck now also in a sketch with the Select tool, else a
  face stayed the midpoint after the approach) and its menu is the short one (sketch.rs `midpoint_of`); off it the
  edge's menu keeps Use, analysis, Zoom to selection, Align view. Part Studio, face selected: the midpoint click joins
  it; the menu adds "Add appearance to face…" (menus.rs: a point with faces selected). Tests hover_matrix
  a_model_edges_midpoint_in_a_sketch…, a_model_face_right_clicked_in_a_sketch… (asserts the face lit),
  a_midpoint_clicked_with_a_face_selected_joins_it.
- walk 8 P1s: FEEL-DIALOG-FINAL-BUTTON (feature_dialog.rs render_final_button, the slider greyed) and
  FEEL-RENAME-INPUT-LOOK (mod.rs rename box); list matrix rows "dialog / Final", "rename / the input's box" (80 / 80).
- qa ask 2: ui-script drag "hold": true + "release" lets go (ui_script.rs `held`, README): tell qa when it lands.
- Ticked: FINAL-BUTTON, RENAME-INPUT-LOOK, MENU-SK-MODEL-ENTITY; dropped FIELD-VERTEX-EDGE's union duplicate row.
FEEL-FILLET-ARROW-ANCHOR: CLOSED by qa as no gap (ours edge_mid; pixel-identical picks on green 04389f02d).
IN 38229ff2 (queued, carried by 768e0e85): FEEL-ORIGIN-HOVER-EDGE, FEEL-CUBE-CORNER-HOVER, FEEL-MC-LOOK,
FEEL-FILLET-PICKED-STRIP, FEEL-NAMED-VIEW-AFTER-SAVE, ui-script drag button/moves/interval (tell qa when green); commit
message has the details. FEEL-FILLET-ARROW-DRAG in 2bc17f3e: qa re-scores when it lands.
FEEL-NAV-INERTIA / FEEL-NAV-ANIM-LOW-FPS (walk 7): live's client (webgl bundle): animateTo jumps and
performStopRotationAnimation skips when isInteractiveFramerateBelowThreshold: average of the last 5 draw-loop CPU times
or rAF intervals > 1000/DETAIL_REDUCTION_THRESHOLD_FPS(20). SwiftShader draws in Chrome's GPU process, so live measures
itself fast and animates; llvmpipe draws in ours (~100 ms), so ours jumps by the same rule. Proposed no change (lead).
Score kit: run-hm7.sh (vibe2 crops ~/tmp/psui/hm3 + b3 cells), compare.py oursN live-b1c|live-b2|live-b3 sxsN.
Later: drop `at_bar` + list matrix row (full1 refusals / copy-paste: both match in qa 1121-0935, closed).

### Earlier today

3714e6ce FAILED 04:40 (row_keys test: the sketch under the bar now shown, not only drawn); fixed forward,
LANDED a071525a as 7de5b356 05:33 with live's Section view rule: the whole fc-1120 batch (item 6 b) + CUTLIST-WHEEL-SIDEWAYS + rollback-bar row menus
(menu_left now any sketch row, also rolled back). Tests green on vibex6: sketch 202, scene sketch_mode 166,
part-studio-ui list/sketch_hover/cut_list 41, ops studio:: 5, clippy. On land: tell qa (CUTLIST to verify).


1. LANDED: 6f48184a (FS notices compile off the UI thread; A/B 6 max prepaint 534 -> 28 ms on 850efe, 549 -> 37 on
   de4eff), 18809b4c (spinner on the executor clock), 1c6e56e4 + 5c3f3bb7 (revert of a5505f50). Worktrees psui-fsn,
   psui-spin removed.
2. LANDED 87ffadda 17:15 (b3c497b9): a5505f50's parts again + alone-only hover-wait averaging + white/blue flange handle +
   Configurations box pane (FEEL-CONFIG-BOX-WIDTH). run156 green (clippy, part-studio-ui 354, app-core 115, offscreen).
   A/B 7/8 (vibex6, load 5-10): select the same at low load (5-7 ms both); under load both slip (main 64-171, re-land
   96-154), waiting for the first window present after the click. Not a re-land regression.
3. Heavy-doc hover. LANDED 51c01c9c 18:35 (0808250d): after 500 ms with no window frame the viewport took itself as
   off screen, so every hover after a rest redrew the whole window (present 100-180 ms on llvmpipe) before its frame;
   now vendor gpui `App::entity_drawn` (NOSHAPE PATCH) decides on screen. Test
   a_hover_after_a_rest_submits_its_frame_without_a_window_redraw asserts `Viewport::last_layout()` unchanged (GPUI's
   test mode draws dirty windows in flush_effects). Land check flaked once on clicks_in_3d_…ctrl_opens_the_menu and
   feature-studio a_double_click_…: 5/5 and 3/3 alone on vibex8 at load 13.
   A/B 11 (vibex6 load 8-11, same-tree baseline): hover medians de4eff 380/237 -> 283/253, impeller 207/180 -> 164/222,
   block 187/183 -> 82/232: mixed under load, clear only at low load.
   DONE (lead 18:5x: software-only frame cost is below the open feel rows): scratch bench /root/psui-scratch/pick_bench.rs
   (never land; run162.sh copies it in on the host), 151k tris + 17k segments 1600x1000 llvmpipe, load 11-13: frame
   71-106 ms with nothing skipped; skipping ao / silhouettes / backsil (alone or together) 63-142, in the noise; meshes
   skipped 28-38; pick 0.1-2 ms. The shaded mesh pass is the cost; no cheap pass win that also helps GPU frames.
   Tools vibex6 ~/agents/psui-keep: ab9/10/11.sh, hf3t.sh, hfeel-t.py, hovtrace.py, hovtrace2.py (+ DIAG lines from
   the trace patch /root/psui-scratch/diag2.py: NOSHAPE_NOTIFY_TRACE, never land); docs in ~/tmp/psui (850efe
   bb49db4ceba0762c, de4eff 874aef1a8b008b1b, impeller 844c39ce20497196, block cc4a172465de10c6).
4. NOW (lead 18:5x): every Part Studio uip scenario that still differs (feature dialogs, feature list, sketch chrome,
   view cube, filters), most-used first; report the count before / after. Sources: qa 14:47 run on 0a0b9a49
   (/root/qa-scratch/suites/1120-1447-0a0b9a49/uip.log; it has a5505f50's select regression, none of the fixes) and
   the night runs (1120-0407-81d23c9b, 1120-night-ui-a06ec927). The 33 Part Studio names: /root/psui-scratch/ps-names.txt;
   rerun on main via uip-some.sh on vibex8 -> ~/tmp/psui/ps1 (log /root/psui-scratch/ps1.log) = the "before":
   4/33 differ on eb8aae4fb (rollback-bar-drag, sketch-dimension-variable, sketch-entity-copy-paste,
   variable-type-angle); the 14:47 run's other 12 + 5 not run all match now. "After": rerun once this batch lands.
   - P0 (lead 20:0x): macOS CI failed a_cleared_hover_shows_the_frame_drawn_before_without_drawing; T2'd in fe8136d5.
     Mac diagnosis 21:4x: every submit had a frame in flight (settled_frame asked every 10 ms; Metal frames take
     longer), and redraws of one view filled the 3-frame cache with copies of it. Fixed (cache one entry per view;
     settled_frame asks only when none is in flight), T2 dropped: LANDED 657a7fad 22:19 (c49692e3).
   - LANDED 5ce123f8 20:46 (c759a122): variable-type-angle ("30 deg"), rollback-bar-drag (driver name marked while the
     tip is off), sketch-entity-copy-paste (a menu paste stays; Transform keeps a picked entity clicked again),
     "Unconfigure suppression" in the row menu (live rollto-menus), FEEL-CONFIG-BOX-WIDTH (cfg-box pr(5)),
     FEEL-FLANGE-EDGE-BEHIND-WALL (selected_only line copies at compare::POST; line shader params.x bit 2).
   - "After" (ps2, main 5ce123f8, vibex8): 32/33 scenarios, 305/308 states (before 29/33, 300/308). Left:
     sketch-dimension-variable (live's take placed no dimension; retake asked).
   - REQUESTS 19:11 answered (live 11-21 01:0x): a feature row's own menu has "Section view…" only while a sketch or
     plane is selected; a sketch row's right-click + Esc leaves the sketch selected unshown. Ours: UiState.menu_left
     (set when a sketch row's menu closes, cleared by any left click), list matrix row "menu / after a sketch's menu".
     LANDED 14d18cb7 01:48 (fe009731). FEEL-FLANGE-EDGE-BEHIND-WALL: qa reopened 00:2x, then could not reproduce
     (4 runs, 9/9 points bright), ticked; its pixel test on qa's model lands with it (no code change).
   - sketch-dimension-variable retaken by live (ca2c8aff; clicks (64,0) pick, (85,-8) place; step 9 value box focus,
     step 10 autocomplete [#width, New variable…], end 30 = #width): ours matches it, 12/12 states (ps5, vibex8, 01:5x,
     with ca2c8aff's files while it was still queued). So all 33 Part Studio uip scenarios match.
   - Sheet metal titleState errors (bend-alignment-hold-line, hem-in-place-closed, bend/jog-new-edit-reedit,
     bend-angle-align-to-geometry) are skasm's features.
6. NOW (lead 22:3x) a) night suite 1120-night-ui-a06ec927 (2145/2172; 26 listed in uip.log): its 14 Part Studio ones
   were in ps1/ps2 and match on main but sketch-dimension-variable (live retake). Not run / flaky there, matched at
   14:47: config-input-menu-copy-table, config-row-move-up, ruled-surface-aligned-direction, ver-compare-hover-part
   (ours "Diameter 10 mm" readout in compare mode when quick; live none: asked in fc-1120 (10)). Rerun x2 on main:
   ~/tmp/psui/ps3 on vibex8, log /root/psui-scratch/ps3.log. The rest are other owners' (drw/doc/asm/vs).
   b) feel coverage: an audit of interaction-feel.md (28 candidates): 21 measured, 7 partly. Batch fc-1120 for live in
   REQUESTS (22:24, 10 items: dense hover, solver drag, region hover, redo, fillet chain, Extrude Enter/Tab preview,
   mate connector points, click-through, panel collapse / resize, compare readout; plus the Section view ETA). Write a
   test per item as its row arrives. LANDED b09eee10 23:09 (a82534f5): latency_tests gained menu / sketch enter / ✓ /
   Undo (Mac release: 8.6 / 17 / 17 / 17 ms vs live 117 / 226 / 117 / 107).
   fc-1120 answered (live behaviours "psui fc-1120", "psui recheck", "psui recheck 2"; shots live/psui/fc-1120/).
   Rows in interaction-feel.md "Live fc-1120". Equal: (1) hover rule, (4) redo, (5) fillet clicked edge only (new test
   a_fillet_lists_only_the_clicked_edge_of_a_tangent_chain), (6) Extrude Enter, (8) Alt click-through (new test).
   Fixed in the batch in flight (Mac run195): (2) glyphs + region fill hidden mid-drag (geometry_dragging,
   SceneOptions::sketch_dragging), (7) mate connector at a face's corner: chip "Vertex of Extrude 1" + TO_OWNER,
   (9) list toggle tooltip "Features and parts" + model kept centred, (3) line reach in sketch mode: inside a region
   7.5 px straight out, outside live's 5 px square (4.9). (2) the dimensioned rectangle drag (ours moved
   nothing, live slides; user 03:2x gave it back to psui, skasm told and dropped it): Sketch::drag, where the
   dimension hold can't reach the pointer, solves without it and keeps each held coordinate that still solves
   (MAX_KEPT 8 points, small components only); skasm: a plain solve alone moves the bottom. (10) compare readout
   -> studios (= 7d64e534). Flaky ctrl_opens_the_menu: 20/20 on vibex7 under load, not flaky (lead told).
   Same batch: CUTLIST-WHEEL-SIDEWAYS (live behaviours of that name 03:11): grid + right-panel body
   restrict_scroll_to_axis (GPUI turned a plain wheel sideways); test a_cut_list_wheel_scrolls_the_panel_and_a_sideways_
   wheel_the_table. FEEL-ROW-MENU-ROLL-TO-HERE (live 03:15, dm/rollback-menus.json): rows below the bar get Section
   view… (menu_left) / a sketch's Hide other + all sketches and (Un)configure suppression; a shown sketch with other
   sketches: Hide, Hide other sketches, Hide all sketches; list row "menu / rolled back under Sketch 1". Section view… on
   a sketch row: back on every plain right-click (live secmenu-rerecord-1121, 13:4x: the 11-06 takes hold; the 04:47
   secmenu probe right-clicked without moving onto the row; qa P1 UIP-SKETCH-ROW-SECTION-VIEW). cov-dm-config's live
   menus lack it: check that take after this lands.
   "After" uip rerun of the 33 (ps-names.txt) on the batch's tree (vibex8 ~/tmp/psui/ps6, log /root/psui-scratch/ps6.log,
   05:2x): 33/33 scenarios, 308/308 states (before 29/33, 300/308).
   Mac: incremental dirs got emptied mid-build twice (11-21 02:4x, 03:1x): runs use CARGO_INCREMENTAL=0.
   c) Sent 22:5x: studios 8 (drw-centermark, drw-inspection-table-iso-scrolled, drw-revision-table-delete; not run:
   drw-quad-mid-snap, doc-folder-drag-to-all-tabs, docs-advanced-search-help, docs-folder-crumb, vs-ps-own-table-menu),
   asm 1 (asm-menu-instance-properties: step 2 dialog fields live 1 / ours 0).
7. LANDED 00c1edad 00:36 (2f9bb4ed). P0 (lead 00:0x, from studios): Routing curve Multiple's centre ball snaps as mate-connector placement (live ball-1119,
   asm-feel3-1118). Ours by the real pointer never snapped: the active field's pick filter masked faces, rehover dropped
   them, and the point was chosen nearest where the drag began (no PointerMove during tool drags). Now: faces + edges
   picked while the ball drags, the drag's position chooses, the 47.5 px square / centroid (edge: 15 px end / midpoint)
   rule in Part Studio inference (also the Mate connector feature), snap as the points arrive. Test (GPU pointer)
   dialogs::the_ball_dragged_onto_a_face_reaches_its_corner_from_45_px_and_turns; part-studio-ui 357 ok on the Mac.
4b. Flange handle vs live flange-handle-1120: the orange inside the heads is the selected edge seen through the hollow
   heads: nothing more to draw.
5. Cleanup done 18:56 (vibex6 A/B binaries and hf/, vibex8 target/release).
   Not to build: 3D Sketch, Fault analysis.

## Agents

- Agent ids: ONLY in /root/lead-scratch/roster.md (lead rule 11-19); read it before every message_agent, never copy ids
  into notes. Context cap 200k (user 11-19).
- P0 sketch-vertical-key: matches live on green f6f8069f, 10 of 10 states (qa 11-20).

## Open decisions

- None.
