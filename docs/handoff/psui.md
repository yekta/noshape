# psui (Part Studio UI): current state
Updated 2026-11-25 08:3x UTC. History up to then: docs/handoff/archive/psui.md (not read by default). Standing rulings:
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

## QUEUE (11-25 19:5x, lead + live)
1. LOOK-OUTLINE-WEIGHT (qa row l.63): cause = a solid's curved outline drew BOTH the backface band and a centred
   opaque silhouette line (ours 808080 + 27343C: half the face px dark; live ~0.7 px all outside). DONE d9c41793c,
   QUEUED eaa44314 20:2x (with b976bdb5's stack): Instance::backed (params.z bit 1) on silhouette lines of bodies with the band
   (builder.rs `backed`); silhouette_vertex drops them unless face-lit / query / error, pick pass keeps them.
   Now 808080 + 3B4E59. T1 edge_looks::a_curved_outline_at_rest_is_the_band_outside_the_face; back_face_cull test
   allows <= 3 outline px (kept back faces beside the band). hover_matrix 125/125 green with it. Then commit + TICKETS.
2. DONE (fe39cfd77) combined hover P0: 3 tests at the end of hover_matrix_tests.rs (combined_hover_*); fixed: a
   sketch click took the edge under a lit model vertex (sketch.rs lit_vertex_at); harness: Matrix::reset now clears
   SketchEditor::model_selection. interaction-feel.md rows filled; left "? (qa)": Extrude region picked hovered,
   face-selected + sketch line, Extrude New/Remove preview cells (no harness: start sketches only on planes).
   Known: Shell's wall inner edge lights nothing (live: the removed face) with equal pixels (preview_hides by design).
3. DONE (730f1acd3) EXPORT-MULTI-ROW-INDIVIDUAL: open_part_export parts_only = one row only.
4. IN FLIGHT 11-26 00:56: interaction-suite triage (lead). Before 20 failing. r18 (vibex8 ~/tmp/psui/r18, copy in
   /root/psui-scratch/r18): 6/9 rerun scenarios match (offset, ver-protection, coincident, refusals, routing, rclick-menu);
   so 12 of the lead's 15 (gusset x2 skipped) match. Then fixed (commit "psui: interaction suite: ..." after e7fae29e2):
   sketch-fully-defined (the outline pick also runs over no face: sketch.rs sketch_hover `None | Some(plane)`; T1
   a_click_just_outside_the_right_planes_outline...), mate-connector (an edge-end origin reads as the edge, not its
   secondary: feature_dialog.rs query_rows point_of only on a Face; T1 a_shift_click_6_px_inside_an_edge...).
   LEFT: sketch-entity-copy-paste step index 9 (rclick at window (1165,501) = sketch (15,20)): ours opens a CONSTRAINT
   glyph's menu (rows ... "Show all" ... "Delete sketch entity"; Some(cid) from the glyph div's right mouse-down,
   sketch.rs ~5370, or LabelDown/dim_line_hover, both impossible: no dimensions), live the Front plane menu (hover JCC).
   T1 a_right_click_beside_a_copied_line_opens_the_sketch_menu replays it at the scenario's scale and PASSES (one glyph
   box, Horizontal, at origin+(25,-41) px, 20 px square; pointer at origin+(83,-18)): the scenario differs. Next: run
   uipx/run19.sh (copy-paste only) with an eprintln of glyph_boxes / dim at open_sketch_context_menu and the glyph
   right-press, read the run's .stderr (killed once at a compaction).
   NEXT after this step (lead 11-26 00:5x): LOOK-OUTLINE-WEIGHT partly fixed (qa, green 23e5d26f7): vertical
   silhouettes 1 px as live but lighter/greyer (616569 vs live 484c4f, 5a5e60 vs 637783), and a new 1 px 151a1e run
   over 63 rows.
   Original item (lead 23:1x): use live's triage (live/REQUESTS.md "11-25 23:09"). 5 plane-* recordings
   were stale (being re-recorded, abaedadb). The other 17 match live today, so they're OUR fixes, most-used first, each with a T1:
   sketch-entity x4 (Hide rows done in b00920fe; check the rest), sketch-coincident-constraint, sketch-constraint-refusals,
   sketch-fully-defined, sketch-offset, mate-connector-shift-lock-inference, routing-curve x5, ver-protection-enable.
   Skip the 2 gusset ones (sheet metal). Then give qa the after count (qa id from roster; check agent_status first).
   Interaction suite (lead 19:4x). DONE so far: sketch-entity-* Hide other/all sketches (rows only with another
   sketch in the studio, live behaviours "qa4 r5 s6b" (5) + ctx-menus-1117 setup had Sketch 1). NEXT: plane-* (4) to
   live/REQUESTS as stale (re-record), then sketch-coincident, constraint-refusals, fully-defined/offset focus, then report
   before/after (before 20) to the lead.
   Original notes: baseline run 1125-1501-c7944540 (qa-scratch/suites/<run>/uip.log, uip/*.json vs
   tests/ui-parity/live/*.json): 20 PS+sketch failing (skip drw/sm/asm/docs/ver). Found: plane-* (4) = stale live
   (recorder right-clicked the feature-list row at x 89; ours is the viewport menu = live 2255) -> REQUESTS re-record;
   sketch-entity-* (4): ours adds "Hide other sketches / Hide all sketches" before Curve/surface analysis and lacks
   Add comment (cloud): check live. Then sketch-coincident (VERTICAL vs PERPENDICULAR), sketch-constraint-refusals,
   sketch-fully-defined/offset focus, mate-connector inference, gusset titleState, routing-curve Points field.

## Agents

- skasm owns the hover matrix's sk/* cells (lead 11-22); psui the 3D / Part Studio cells.
- Agent ids: ONLY in /root/lead-scratch/roster.md (lead rule 11-19); read it before every message_agent, never copy ids
  into notes. Context cap 200k (user 11-19).
- P0 sketch-vertical-key: matches live on green f6f8069f, 10 of 10 states (qa 11-20).

## Open decisions

- None.
