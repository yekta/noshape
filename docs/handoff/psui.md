# psui (Part Studio UI): current state

Updated 2026-11-21 00:22 UTC. History up to then: docs/handoff/archive/psui.md (not read by default). Standing rulings:
docs/DECISIONS.md (read first; it wins over anything here).

## Setup

- Worktree /root/projects/noshape-wt/psui, branch agent/psui. Commit as yekta (AGENTS.md), subject "psui: …".
- Builds and crate tests on the Mac (lead 11-20 23:10): plain `NOSHAPE_RB_HOST=mac tools/tiers/rb.sh bash -c "$(cat
  script)"` (headless, ~/noshape-host only, 2 slots shared with bool/blend/core); vibex8 when it is busy. ~/bin/slot
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

## Now (11:01 UTC 11-21)

LANDED 4553fc38 10:28 (sk batch + empty-feature errors + Routing curve buttons), 0cd43606 10:52 (Construction readout),
70ab4b581 10:53 (dialog table, docs). QUEUED 17848cd1 11:01: Variable conversionFactor default 0 as live (lead decision;
std variable.fs reads it only with useConversionFactor; custom_features::variable_from_import test both ways).
Running: sk12c rerun of the 21 takes on vibex8 (psui-shards.sh -> ~/agents/psui-keep/sk12c).

sk-1 + sk-2 (21 takes, in tests/ui-parity with f90a1ef9). Rerun sk12b (vibex8, copy /root/psui-scratch/sk/sk12b):
12/21, 306/344. In 4553fc38 (not in sk12b): polygon drop (curves held at the drag's start), 3pt circle Origin tie;
a sketch point on the Origin picked before it (sketch_pick_at; dim hv-two-points, circle-centres); Split by a line's
middle cuts at the midpoint + MIDPOINT of its ends (live geo-list6); an ellipse and a plane refused with
DIMENSION_FAILED (dim-ellipse-axes); tests in scene sketch_mode + sketch_ui_tests. Open: transform-drag-copy (live's
arrow drag leaves the box focused "9.9 mm"; ours no focus: arrow hit in iso view?), circle-rims step 22 (our click on
circle 1's rim at (-5, 0) picks nothing), split-at-crossing step 11 (live PERPENDICULAR: the line crosses the
held line at its midpoint; crates/sketch square_at_mid, sent to skasm 10:1x), cparc end off-circle (REQUESTS 08:40). Shots of 3 takes: vibex8 ~/agents/psui-keep/bin/shots.sh (frames every 0.4 s)
-> ~/agents/psui-keep/shots (waiting on slot).

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
   sketches: Hide, Hide other sketches, Hide all sketches; list row "menu / rolled back under Sketch 1". Open: live's
   sketch menus in cov-dm-config have no Section view… before Suppress, ours (keys-1118) do: live answered 04:5x
   (behaviours "psui 11-21 04:12", secmenu-1121): only while the selection already holds a planar item (plane, sketch,
   planar face), the same sketch included (its own right-click + Esc, or a row click). Ours now: a sketch row's
   Section view… only when it was selected before (menu_left or selected_features); list row "menu / one sketch".
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
