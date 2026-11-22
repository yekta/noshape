# psui (Part Studio UI): current state

Updated 2026-11-22 02:54 UTC. History up to then: docs/handoff/archive/psui.md (not read by default). Standing rulings:
docs/DECISIONS.md (read first; it wins over anything here).

## Setup

- Worktree /root/projects/noshape-wt/psui, branch agent/psui. Commit as yekta (AGENTS.md), subject "psui: …".
- Builds and crate tests on vibe2 (user 11-21): `NOSHAPE_RB_HOST=vibe2 tools/tiers/rb.sh bash -c "$(cat script)"`,
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
- sbs.py: side-by-side crops (ours offset (242, 90)); mirror-sxs/ holds the Mirror side by sides with README.
- vibe ~/tmp is pruned after 6-12 h: keep results in ~/agents/psui-keep.

## Now (02:54 UTC 11-22): USER P0, the hover x selection look matrix + qa's walk tickets
LANDED: batch 1 (5b7743f97), batch 2 (6be52caf), batch 3 666d9c47 as f411d924 (qa re-score 4/15, matrix 94/125).
QUEUED 0fc65a29 (03:0x, 3 commits: walk 6, batch 3/walk 7, walk 5 + 8 below); lead + qa + live told.
IN TREE (03:12, next land; run-t13.sh): FEEL-PLANE-LIT-WHILE-DIMENSIONING per live dimplane-1122 (behaviours "psui plane
while dimensioning"): with Dimension picks a plane lights by its outline only (sketch.rs sketch_hover -> plane_by_outline,
reach DIM_PLANE_OUTLINE_PX 2.5, 6 with an ellipse); test sketch_hover_feel a_line_picked_for_a_dimension_lights_a_plane_
only_on_its_outline. KEYS-SHEET-DETAILS: "select rollback bar" a keycap (shortcuts.rs locked_row; SheetRow.prefix gone).
Fillet arrow anchor pin test (hover_matrix the_fillet_arrow_stays_at_the_edges_midpoint…; lead re-raised it, told closed).
SPEED-HEAVY-HOVER closed (Mac GPU: plate 22-86 ms, parts-1000 21-69 ms move-to-present vs live 42-53 / 265-270;
kit ~/noshape-host/psui-keep run.sh / run2.sh ps1000, local psui-scratch/mac). TICKETS dup headers dropped.
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
IN 38229ff2 (render 17, scene 200, part-studio-ui 453 green on vibe2; run-full5.sh):
- FEEL-ORIGIN-HOVER-EDGE: renderer.rs pick_reaching's corner rule reaches 3 px (d2 <= 10): the origin glyph (a 5x5
  pick id under the edges' priority) covers the corner, the edges' nearest pixels were 3 px off. b3_cell aims at
  `m.exact` (unrounded, as the ui-parity runner); the rounded point hid it.
- FEEL-CUBE-CORNER-HOVER (walk 7): overlay.rs view_cube::build draws a hovered corner's highlight over the near faces
  (no white fill: live #B1D9F0 over a gap); test render `the_cubes_near_corner_lights_a_whole_disc` (CPU composite).
- FEEL-MC-LOOK: builder.rs connector_origin_lit, a 10 px #F2FF80 point under a lit connector whose originQuery is a
  vertex (connector_look.rs connector_on_vertex); cells check 30+ lit px within 5 px (live 39, ours was 18). The crisp
  rim / navy block at the ring's front = no MSAA under llvmpipe (style.software), not ours to fix (lead: software GL
  isn't a target).
- FEEL-FILLET-PICKED-STRIP: LineSegment::BAND (1024) + CORE: an overlay line drawn as a lit edge's 5 px band, no plain
  line under the core (shaders.wgsl own_edge); behaviors.rs consumed_picks_overlay uses it for picked edges of a Fillet
  edit only (a flange's edge under its wall stays the bright 3 px line: sm test, the band's 78% core is dim over the
  blend). Cell b3_field_edge_picked: 4-6 amber px across (live 4, ours was 3).
- FEEL-NAMED-VIEW-AFTER-SAVE (walk 7, P1): a saved view is the picked one (part studio view_tools.rs, assembly
  assembly_view.rs); test sketch_ui_tests a_saved_named_view_is_the_picked_one.
- qa ask: ui-script "drag" takes "button" right|middle, "moves", "interval_ms" (crates/app/src/ui_script.rs drag_plan,
  README); right/middle let go right after the last move (a flick). Tell qa when green.
- TICKETS: tick the 3 batch-3 ones (done) + after a rebase onto main CUBE-CORNER and NAMED-VIEW. NAV-INERTIA /
  ANIM-LOW-FPS: qa re-ranked them P2 software-GL-only (qa checks the Mac); nothing of ours.
- FEEL-FILLET-ARROW-DRAG: in 2bc17f3e (queued), qa re-scores when it lands.
FEEL-NAV-INERTIA / FEEL-NAV-ANIM-LOW-FPS (walk 7): live's client (webgl bundle): animateTo jumps and
performStopRotationAnimation skips when isInteractiveFramerateBelowThreshold: average of the last 5 draw-loop CPU times
or rAF intervals > 1000/DETAIL_REDUCTION_THRESHOLD_FPS(20). SwiftShader draws in Chrome's GPU process, so live measures
itself fast and animates; llvmpipe draws in ours (~100 ms), so ours jumps by the same rule. Proposed no change (lead).
Score kit: run-hm7.sh (vibe2 crops ~/tmp/psui/hm3 + b3 cells), compare.py oursN live-b1c|live-b2|live-b3 sxsN.
Later: drop `at_bar` + list matrix row (full1 refusals / copy-paste: both match in qa 1121-0935, closed).

### Earlier today

LANDED c2e4094c as 9ffded3f 14:13 (3dd4f04b's circle-rims + ellipse rule, parity ellipse labels, plane-face rule,
Section view… back on a sketch row's plain right-click; qa verified the 15 UIP-SKETCH-ROW-SECTION-VIEW scenarios on
28d58d209, 75/75). Kept off: the sketch right above a rolled-back bar (rollback-menus; live REQUESTS 13:49 to recheck).
IN TREE (to land): real-app fixes from vibex8 shots (shots-run.sh, ~/agents/psui-keep/shots6):
- circle-rims step 22 (27/27 now): a glyph takes a press only when it is lit (its tile, or the hovered constraint); the
  pointer-move path no longer lights a glyph from the lagging glyph_boxes while the sketch's entity is under its margin.
- ellipse-axes step 12: with faces off, the plane lit is the nearest outline within 6 px (plane_by_outline), whichever
  face is under; re-resolved on every move (the GPU id doesn't change along one face). Test probes live's (40, 5).
- ellipse-axes step 18: ui_script exports an ellipse axis dimension as live's MAJOR_/MINOR_DIAMETER constraint.
sk12d (the 21 sk12 takes, sk12-names.txt, uip-some.sh on vibex8 -> ~/agents/psui-keep/sk12d): 21/21, 344/344.
Not done: Front's face not picked 3.3 mm above the sketch plane (live picks it at 13-17 mm).
split-at-crossing PERPENDICULAR: skasm's queue (crates/sketch square_at_mid). Shots: vibex8
~/agents/psui-keep/shots2/<take> (shots-run.sh in psui-scratch: build + one app, frames every 0.4 s).

qa P1 FEEL-ROW-DBLCLICK-DISTANCE: closed by qa (xev-measured gaps on vibe2: 80 px opens to ~380 ms, not ~420, as one spot).

std 3132 vs 3112 (reported to lead 06:5x): our crates/featurescript/std is exactly 3112; 14 files / 181 lines differ
(live/std-3132, live/std-3132-vs-3112.diff, stddiff.py in psui-scratch/std). Geometry ones (SM pattern collapsed
walls V3128, hole error tool axes on start plane, 7/8"-14 UNF tap drill 0.8125, thin extrude @setThinWallOneSided
host builtin) -> core with the suite gate. Hole V2 diameter is in 3112 too (legacy V2 holes only): no gap.

8. NOW (lead 05:3x) a) Sketch tool coverage: all 55 sketch tools have >= 1 uip take (coverage.py; per tool
   /root/psui-scratch/sk/tools.json, union 169 in sk/names.txt). The 169 on main (vibex8 06:2x): 169/169, 1946/1946
   states. sk-1 (10 dimension kinds): live has 7 takes good, parallel-lines / circle-rims / circle-centres miss a
   pick (live checking). sk-2 (10 second takes for the thin tools) recording. When they land in tests/ui-parity: run
   them (psui-shards.sh on vibex8), fix the differences, most-used first. skasm: tell before a crates/sketch or
   sketch_mode fix.
   b) Feature dialog audit DONE 10:46: docs/parity/feature-dialogs.md regenerated on green 4553fc38a against live's
   dlg-1121 specs (std 3132) and empty errors: 81/86 specs, 85/87 errors; the rest explained there (Hole = std skew).
   Scripts /root/psui-scratch/dlg/ (specdiff.py, empty_errors.py, gen_table.py). Open: Variable conversionFactor.


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
