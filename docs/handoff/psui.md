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
- Stack rebased onto main and re-queued as 5175c04d (19 commits) at 18:2x; audit batch 1 bc241af85 queued after it.
- Lead task 11-24 18:2x: Part Studio feel-coverage audit. Batch 1 done: docs/parity/ps-feel-audit.md (186 specs, 22
  out of scope, covered before 142 of 164, after 153, 11 left). Tests: part_studio/ps_audit_feel_tests.rs (audit::),
  nav row "view cube / a triangle click…", list row "menu / every part…". No app behaviour was off live.
  Next batch = the doc's "Still without a test" table (later rows in /root/psui-scratch/audit/triage.tsv): Show all /
  Show all parts effect, Camera menu disabled rows, context menu 250 px ellipsis, analysis panel position over a
  dialog, Delete part preview look, preview self-pick, analysis panel number fields (Zebra count, Draft angle),
  DCLICK-HEAVY, panel positions, Hole icon rows, checkbox label widths.
  Tests note: a field made active by user_focus_query alone is taken back by the focused field's tab stop on the next
  render: focus `tab_stop_handle("q:<path>")` as a click does. build_start can't name regions inside arrays (pick them).
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
- RE-QUEUED 17:54 as 75404bb7 (whole stack; 8903d37f failed 17:45 on a SIGTERM of the part-studio-ui lib binary, no test failed: earlyoom-like, same second slot 1 finished): depth-0 Extrude region outline red (live errors-1123; builder outline_corners:
  corners = a sketch curve end or a turn > 60°; test a_depth_0_extrudes_circle_region_has_a_red_outline); field
  errors = live (test feature_fields_reject_what_live_rejects); flip buttons (hover 1 px #1651B0 bottom line, arrow
  cursor; test the_flip_button_flips_on_click_takes_the_keyboard_and_ignores_space).
- Corner-break disc (FEEL-CORNER-BREAK-VERTEX-PICK): sheet metal, PARKED by the lead: test is T2, don't debug. Known:
  the scene holds the disc (layer 20 Points, POST, over_lines, mix 0.7); the renderer's preview-post pass drops it.
- IN 75404bb7 (was 8e11c924): FEEL-HOVER-OWNER-SELECTED (3): a hovered vertex at a selected edge's
  end gets a 6 px piece of the edge's band over its disc (behaviors vertex_band_piece, layer 21 after the disc; the ON_TOP
  disc writes near depth, so a depth-tested redraw can't cover it); TICKETS dup rows of SKT-USE-HOVER /
  SKT-POSTCOMMIT-HOVER dropped.
- skasm's 7e9bedfb7 (after its b99589dc) adds LineSegment::dash_from: new LineSegment literals need `dash_from: 0.0`.
  My vertex_band_piece literal (in 8e11c924) predates it: whichever lands second adds the field.
- QUEUED 53737a22 (df1e61e44) + 59a849f1 (empty-Loft title tests), stacked on 75404bb7: FEEL-ROW-TIP-AFTER-DBLCLICK per live row-tip-dblclick-1124: a
  double-click holds the row tip (ui.row_tip_held, cleared on leave or entering another row: the leave is lost
  while the dialog opens); a quick one (row_early_press) shows ui.row_name_tip (app-core ui::row_name_tip,
  typed_name, 300 ms fade) while a dialog is open. Also test an_empty_lofts_title_tooltip_names_its_error: the
  real flow already shows Loft 1's error tooltip (qa ERR-LOFT row 648's "no tooltip" not reproduced; ask qa's flow).
- 8903d37f FAILED 17:45 = SIGTERM of the part-studio-ui lib binary (no test failed): re-queued as 75404bb7.
- Other open rows checked 11-24 17:4x: all "fixed, to verify" (qa), sheet metal (parked), or skasm's.
- Then: PARTLY rows in TICKETS (TRANSFORM-*, PS-BOX-DRAG-HOVER), P2 queue
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
