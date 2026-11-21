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

## Now (00:22 UTC 11-21)

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
