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
- QUEUED a2341555 (13:5x): skasm's stack (user's Offset fix) rebased again on main (kept both sides: sketch_ui_tests, mod.rs
  ghost_outline_only(.., &selection), eval.rs refused + error_refs), 11 doubled TICKETS rows dropped. Worktree
  psui-skrebase; after it lands: `git worktree remove` psui-skrebase and psui-strebase, rm ~/agents/psui-*rebase on
  vibex7/8. eaa20b28 LANDED as 9d6b9fc61.
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
- QUEUED dae14f97 (14:3x): TICKETS cleanup + list-tooltip row + FEEL-EDIT-BODY-SHADE fix (above).
- IN PROGRESS: FEEL-PREVIEW-BODY-BLUE (P1 regression, workflow 04 s2 Shell top removed / 38 s16 Intersect: ours the
  plain rest top (152,194,218), live (172,184,172)). A gpui repro (block, Shell, top clicked) does NOT reproduce: ours
  floor (151,161,139) at 0.7, (143,151,127) at 1, (169,186,166) at 0, the picked top tinted in both. So the workflow
  shot likely caught no pick / no preview yet: read qa's out-wf* step 2 (frame + state) and the workflow script next.
- Next: PARTLY rows in TICKETS (CORNER-BREAK-VERTEX-PICK, TRANSFORM-*, PS-BOX-DRAG-HOVER), P2 queue
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
