# psui (Part Studio UI): current state
Updated 2026-11-22 10:51 UTC. History up to then: docs/handoff/archive/psui.md (not read by default). Standing rulings:
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

## Now (09:53 UTC 11-22): batch landing (lead order: batch → land; corner dots; Recognize; wf tickets)
454c71f2 LANDED 09:27 as c45df7d2 (qa re-scores wf29/19/38/02/05 now).
BATCH QUEUED as 9ad9b5d6 (6d72a4e3 failed 10:01 on a rebase conflict at sketch_ui_tests.rs's end with skasm's
u_on_another_sketchs_vertex_projects_it: both kept, rebased, requeued; t35 + t36 green: hover_matrix 85/85, list 112/112, feedback, fields, sketch_hover, measure, feel,
scene lib, ops e2e::sketch; clippy clean). Ticked: FEEL-HELIX-FACE-PICKED-LOOK, FEEL-QUERY-PART-PICK-FILL,
APPEARANCE-RGB-FIELD-SCROLL, SPLIT-KEEP-TOOLS-AUTO, FEEL-SKETCH-RCLICK-PLANE-LIT, MEASURE-VALUE-CLICK-PIN,
MEASURE-READOUT-VARIABLE-ICON, FEAT-MISSING-REF-CHIP, PARTS-ROW-OF-PICK, SKETCH-ON-FACE-EXITED-LOOK. Not ticked:
UIS-WORLD-TARGET (asm's half), EXPR-AUTOCOMPLETE-LOOK (the list's left edge in a sketch dimension box),
MEASURE-PANEL-POSITION (no drag test: PanelOffsets writes the user's config; qa recheck). Model-core changes for core:
FeatureState.missing_refs (eval::drop_missing_regions), SketchDisplay.on_face (serde default).
- PARTS-ROW-OF-PICK: a dialog field's face/edge/vertex relates its part row (#ECF0F4 + 1 px #90CEF1 underline);
  body_of_input falls back to a face next to an edge/vertex the preview consumed (a Fillet's edge).
- SKETCH-ON-FACE-EXITED-LOOK: builder CurveEnds (every curve end dotted at rest, not a closed curve's seam); no rest
  region fill on a part's face (SketchDisplay.on_face).
- MEASURE-PANEL-POSITION: Measure a moved panel ("ps-measure"), remembered in panel_positions.json.
- P0 FEEL-PICK-SKETCH-LINE-THROUGH-FACE taken OUT (patch /root/psui-scratch/pickclass-line.patch; test
  line-8px-test.rs). A reach circle can't satisfy both live cases: inside a shown sketch's rectangle the region's
  pick-only mesh (PICK_PRIORITY_DEPTH_BIAS +4, drawn before line_pick) hides the inner half of the line's 5 px band
  (-7 px needs ~6.5 px reach; wf36 needs < 5.4). Fix the occlusion first (lines' pick depth beating the region), then
  the circle (radius² + 4 at scale 1; pick radii device px, r = 5 × scale). NOTE (t37): a lit line's pick band is
  11 px (vs_line `held`), so measure with the pointer parked between samples. DIAG (uncommitted, never land):
  /root/psui-scratch/tmp-diag.patch (viewport psui_tmp_raw_id + test psui_tmp_line_band_ids; reverted).
- FEEL-SECTION-PLANE-LABEL-LOOK labels half: the section's back-face tint (shaders mesh_shade, accent·0.5) hit unlit
  meshes (plane names, fills) seen from behind; now shaded faces only. Test hover_matrix
  plane_names_keep_their_blue_in_a_section_view. OPEN half: ours outlines the cut part in thick black ~3 px edges, live
  1 px (qa-scratch/tickets/wf28-section-zoom.png).
RECOGNIZE: CLOSED 09:26 (live offers it nowhere; ours is .hidden(); existing features still evaluate and edit).
NEXT: the P0 line pick (above; diag via pointer-parked samples gave None/stale hover: read the pick buffer
directly from a Renderer like Matrix::crop instead), the section's thick edges, CTX-MENU-FLIP-UP (submenu-at-once: needs live
evidence whether a row under the pointer at open lights), the autocomplete list position, tooltip-after-press opt-out.
OPEN: tooltip after a press (gpui-pre div.rs TooltipPressGate needs a per-element opt-out). Helix arrow direction = the
revolve axis (core/skasm). CTX-MENU-FLIP-UP: ours fits below as it lacks live's cloud rows.

## Agents

- skasm owns the hover matrix's sk/* cells (lead 11-22); psui the 3D / Part Studio cells.
- Agent ids: ONLY in /root/lead-scratch/roster.md (lead rule 11-19); read it before every message_agent, never copy ids
  into notes. Context cap 200k (user 11-19).
- P0 sketch-vertical-key: matches live on green f6f8069f, 10 of 10 states (qa 11-20).

## Open decisions

- None.
