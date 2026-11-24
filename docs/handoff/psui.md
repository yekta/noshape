# psui (Part Studio UI): current state
Updated 2026-11-24 11:5x UTC. History up to then: docs/handoff/archive/psui.md (not read by default). Standing rulings:
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

## RESUME HERE (11-24 14:2x UTC; brief /root/lead-scratch/prompt-psui-1124.txt)
- a2341555 (skasm's stack) FAILED an ops parity test (Text letters' counter): skasm owns that land now (lead 11-24
  16:xx); don't touch psui-skrebase. eaa20b28 LANDED as 9d6b9fc61.
- Landing (14:2x, land-edb.out): TICKETS one row per psui ticket; list matrix row "a row clicked into a dialog's field
  keeps its tooltip" (passes: FEEL-ROW-DEPENDENCY-TIP-IN-DIALOG to verify); FEEL-EDIT-BODY-SHADE fix: an edited
  feature's (EditSession !is_new) preview bodies keep their color and rest shade (Instance::preview_lit: preview
  layering, part lighting; SceneOptions.preview_lit; preview_keeps_part_look), the end cap #595959 at 0.5 drawn in
  both frame and "with" layer (= over the mix). Test an_edited_extrudes_region_shows_through_its_sides_as_live (plain
  block, live edit-body-slider-1124 points). Left P2 in the TICKETS row: y=30 region edge outline through the right face
  (live none), datum planes darker at slider 0, region tint B -9 on the right.
- Windows CI: a_hover_after_a_rest_ + triad hardened (landed 9d6b9fc61). a_cleared_hover_...: read the next Windows
  main run's frame_diff message (run after 9d6b9fc61). the_pointer_lights_...: never failed on main CI; release run
  45375844269 job 138469739575 still in progress 13:5x (`gh api --allow-escape-sequences .../jobs/<id>/logs`).
  asm's the_second_pick_outlines_... failed once on Windows (told the lead).
- Told the lead (13:5x): qa score-a910 b3x-mc-radius / b1-midapp-sel-face mispaired.
- LANDED 0923c1fd as ad784f89 (16:43): owner-selected hover cells, Fillet handle bisector step, sm tab test T2.
- LOCAL db768533d (not queued yet): depth-0 Extrude region outline red (live errors-1123): grey corner dots were drawn
  at every vertex of the tessellated circle over the red line; corners now = a sketch curve end or a turn > 60°
  (builder outline_corners); test a_depth_0_extrudes_circle_region_has_a_red_outline. Field errors (live
  field-errors-1124): ours = live on every value, no code change; test feature_fields_reject_what_live_rejects +
  interaction-feel row. Scene suite green; part-studio-ui suite green except the corner-break disc (below).
- UNCOMMITTED: flip buttons (lead 16:5x, live flip-buttons-1124): hover adds the 1 px #1651B0 bottom line, arrow
  cursor (was a hand) in feature_dialog.rs; test the_flip_button_flips_on_click_takes_the_keyboard_and_ignores_space
  passes (click flips + focuses, Space nothing, Enter commits: already as live). Commit with the sm_feel_tests assert
  message (pixels across the corner). REMOVE the temporary NOSHAPE_DIAG_CONSUMED eprintlns first: behaviors.rs
  consumed_picks_overlay (2 blocks) and mod.rs before `let scene = Arc::new(scene)`, and the "DIAG corner" eprintln
  in sm_feel_tests a_corner_breaks_picked_vertex_keeps_its_disc.
- REGRESSION (my P0 FEEL-CORNER-BREAK-VERTEX-PICK): a_corner_breaks_picked_vertex_keeps_its_disc fails whenever a GPU
  is there (the land host skips it): 0 orange px in the whole frame. Not db768533d, not the owner-selected change, not
  the disc's compare::POST. consumed_picks_overlay does build the disc at [0,0,0.002] (vertex consumed, found in
  preview_before), and the scene handed to set_scene holds it (layer 20 Points, compare POST, over_lines, preview_mix
  0.7): the RENDERER drops it. Next: the preview-post pass (renderer.rs post_frame / draw_lists points_over_lines,
  Lists::build only_post culling) for a POST Points instance; suspects since 3cfb09978: 37f7de4cf, 7e017ac17, core a4804bf07/35dceca06.
  Run: /root/psui-scratch/run-cb2.sh via rb.sh on vibex8 (NOSHAPE_DIAG_CONSUMED=1).
- FEEL-HOVER-OWNER-SELECTED case (3) open: vertex hovered with its edge selected (live: the band over ~1/4 of the disc;
  ours draws the disc after every line).
- Then: PARTLY rows in TICKETS (CORNER-BREAK-VERTEX-PICK, TRANSFORM-*, PS-BOX-DRAG-HOVER), P2 queue
  (PS-PARTS-ROW-TOOLTIP-PITCH, CUSTOM-FEATURE-ROW-ICON). Sheet metal parked (ERR-SM-BAD-SELECTION-RED waits).
- USER P0 Mac mini: APPEARANCE-PART-UNDER-FACE-COLOURS once dx lands; user's doc only as ~/tmp/psui/macmini-m6.noshape
  (700), delete it and /root/psui-scratch/Mac-mini-2024.step after.
- qa's hover-matrix score: /root/qa-scratch/hm2/score-a910/sc8.txt; ascii maps score-a910/bin/ascii.py.

## Agents

- skasm owns the hover matrix's sk/* cells (lead 11-22); psui the 3D / Part Studio cells.
- Agent ids: ONLY in /root/lead-scratch/roster.md (lead rule 11-19); read it before every message_agent, never copy ids
  into notes. Context cap 200k (user 11-19).
- P0 sketch-vertical-key: matches live on green f6f8069f, 10 of 10 states (qa 11-20).

## Open decisions

- None.
