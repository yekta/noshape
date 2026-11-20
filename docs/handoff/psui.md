# psui (Part Studio UI): current state

Updated 2026-11-20 11:14 UTC. History up to then: docs/handoff/archive/psui.md (not read by default). Standing rulings:
docs/DECISIONS.md (read first; it wins over anything here).

## Setup

- Worktree /root/projects/noshape-wt/psui, branch agent/psui. Commit as yekta (AGENTS.md), subject "psui: …".
- Builds and crate tests: plain `NOSHAPE_RB_HOST=vibex8 tools/tiers/rb.sh bash -c "$(cat script)"` (DECISIONS 9e5b481d4;
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

## Now (11:14 UTC 11-20, fresh thread since 10:00)

1. IN FLIGHT, a local commit "psui: wip" (rebased on main 10:3x; amend it into the real subject before land.sh):
   - Feel-row tests for interaction-feel 574 / 575 / 577: sketch_glyph_feel_tests::a_lines_glyph_row_is_20_px_boxes_22_px_
     apart_20_5_px_off_its_middle; scene sketch_mode a_dimension_arrowhead_is_16_by_7_px, geometry_being_drawn_is_lives_
     preview_blue (run139: the scene two pass). Fill those rows' Test cells when green.
   - qa reopen R8 FEEL-ROLLBACK-DROP-ROW (`row_hover_filled`, mod.rs); R9 FEEL-SECTION-ARROW-FOOT (`SECTION_STEM_PX`,
     test the_section_arrow_reaches_about_100_px_from_its_plane); Configurations box labels (live cfglabel-1120, test
     the_configurations_box_labels_take_their_texts_width); FEEL-SM-PARTIAL-FLANGE-MANIP handle (TANGENT_* consts in
     manipulators.rs: ring 19, heads 24-41 px, live's blue in every state; at-rest asked of live2, REQUESTS 10:40).
   - TICKETS: SETLIB ticked; R8 / R9 / partial-flange "fixed, to verify"; HEAVY-ASM-SOFTWARE-FRAME closed (lead).
   - Uncommitted on top: the frame cache (item 2).
   Check: run141 (vibex7) then run142.sh (clippy render + app-core + part-studio-ui; app-core viewport; sketch_hover_feel
   + the tests above). Green: amend, land.sh, tell qa (R8 / R9 / handle), drop stash "psui-feeltests-1000" (by tag).
2. P0 heavy Part Studio feel (lead 10:3x). Tools: /root/psui-scratch/hf.sh (vibex6 ~/agents/psui-keep/hf2.sh <doc>
   <tab hex> <out> [perf] [NOSHAPE_PERF_SKIP]) + hfeel.py (screen-timed hover / clear / click / Shift+E, 5 takes); docs
   + green 9381b4adc binary in vibex6 ~/tmp/psui (isaac "Studio 850efe" bb49db4ceba0762c, "Studio de4eff"
   874aef1a8b008b1b; impeller 844c39ce20497196; block.noshape cc4a172465de10c6). Medians (ms) hover / clear / select /
   dialog: block 480 / 228 / 128 / 93; 850efe 404 / 157 / 495 / 68; de4eff 412 / 123 / 333 / 104; impeller 568 / 290 /
   199 / 137. Live drift-1120 (software GL): hover 113-259, clear 5-7. Not model size: one llvmpipe frame is 90-120 ms
   (r.gpu ~56 even for a block; no single pass dominates: NOSHAPE_PERF_SKIP "ao" only drops AO from the timer, not the
   pass) + GPUI's present ~30. Fix in progress (uncommitted): viewport.rs `frame_cache` (3 read-back full frames keyed
   by `frame_key` + `Scene::same_as`); a frame equal to one kept is shown at once and the render thread only told
   (`Gpu::adopt` -> `Renderer::adopt_frame`: picks answer for its scene). Test sketch_hover_feel_tests::a_cleared_hover_
   shows_the_frame_drawn_before_without_drawing. Then: hf2.sh A/B on the block + the 3 docs; hover itself (first light)
   stays one frame + the prehighlight wait.
3. Untested "=" rows elsewhere (56) are asm's and studios'. Rows 578-580 stay "–" (calibrated constants).
4. Cleanup: vibex8 ~/agents/psui/target 21 GB: clear target/debug once run140 is done. Not to build: 3D Sketch, Fault
   analysis.

## Agents

- Agent ids: ONLY in /root/lead-scratch/roster.md (lead rule 11-19); read it before every message_agent, never copy ids
  into notes. Context cap 200k (user 11-19).
- P0 sketch-vertical-key: matches live on green f6f8069f, 10 of 10 states (qa 11-20).

## Open decisions

- None.
