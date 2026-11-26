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

## Agents

- skasm owns the hover matrix's sk/* cells (lead 11-22); psui the 3D / Part Studio cells.
- Agent ids: ONLY in /root/lead-scratch/roster.md (lead rule 11-19); read it before every message_agent, never copy ids
  into notes. Context cap 200k (user 11-19).
- P0 sketch-vertical-key: matches live on green f6f8069f, 10 of 10 states (qa 11-20).

## Open decisions

- None.
