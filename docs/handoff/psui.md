# psui (Part Studio UI): current state
Updated 2026-11-22 10:59 UTC. History up to then: docs/handoff/archive/psui.md (not read by default). Standing rulings:
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

## Now (10:59 UTC 11-22): lead order: batch → land; corner dots; Recognize; wf tickets
QUEUED 91ae59a8 (10:53; carries 9ad9b5d6, the batch: 6d72a4e3 failed 10:01 on a rebase conflict at
sketch_ui_tests.rs's end with skasm's u_on_another_sketchs_vertex_projects_it, both kept). Batch: t35 + t36 green
(hover_matrix 85/85, list 112/112, feedback, fields, sketch_hover, measure, feel, scene lib, ops e2e::sketch, clippy).
91ae59a8: section labels (t39: 96/96 incl. hover_matrix 86/86, render lib 17/17). Reported to lead 09:58 (batch);
the section-label land not yet reported. Core and qa told 09:58.
Ticked: FEEL-HELIX-FACE-PICKED-LOOK, FEEL-QUERY-PART-PICK-FILL, APPEARANCE-RGB-FIELD-SCROLL, SPLIT-KEEP-TOOLS-AUTO,
FEEL-SKETCH-RCLICK-PLANE-LIT, MEASURE-VALUE-CLICK-PIN, MEASURE-READOUT-VARIABLE-ICON, FEAT-MISSING-REF-CHIP,
PARTS-ROW-OF-PICK, SKETCH-ON-FACE-EXITED-LOOK. Not ticked: UIS-WORLD-TARGET (asm's half), EXPR-AUTOCOMPLETE-LOOK (the
list's left edge in a sketch dimension box), MEASURE-PANEL-POSITION (qa recheck; note rb.sh sets a temp
NOSHAPE_CONFIG_DIR, so a drag test may be possible there, but not in CI). FEEL-SECTION-PLANE-LABEL-LOOK: labels half
done (annotated, not ticked).
- P0 FEEL-PICK-SKETCH-LINE-THROUGH-FACE: CAUSE CONFIRMED (t38b, offline pick buffer, diag test
  /root/psui-scratch/line-band-diag.patch, rendered like Matrix::crop with pick: true). On the shown-sketch-reach
  rectangle the two FAR lines (screen normal pointing up) lose their inner band pixels (-1, -2 px) to the region's
  pick id; the near lines keep the full -2..+2 band. Tilt: across the band, the tilted plane's depth gains more than
  the line's pick lift (vs_line: 1.5 * min(half, 1.5 * scale) = 2.25 px toward the eye; iso tan ~1.4 x 2 px = 2.8).
  Plan: (a) sketch-line instances carry their plane normal (new Instance field, e.g. in a free params.z bit plus a
  vec; params.z bits 1-7 are free; normal_m is used by silhouette lines, shaders 883) and in picking lift by
  half * tan(tilt) + 1 px, clamped ~20 px; or (b) regions not overwriting a line's band (needs a stencil; the pick
  depth is Depth32Float, no stencil). Prefer (a). Then reapply pickclass-line.patch (PickClass::Line, circle r² ~29 at
  scale 1) and restore line-8px-test.rs. Check sketch_hover a_shown_sketchs_line_and_point_reach_outside_sketch_mode_as_live
  (-7 px inside lit) + the 8 px test (wf36).
- FEEL-SECTION-PLANE-LABEL-LOOK labels half (in 91ae59a8): the section's back-face tint (shaders mesh_shade,
  accent*0.5) hit unlit meshes (plane names, fills) seen from behind; now shaded faces only. Test hover_matrix
  plane_names_keep_their_blue_in_a_section_view. OPEN half: ours outlines the cut part in thick black ~3 px edges,
  live 1 px (qa-scratch/tickets/wf28-section-zoom.png).
- PARTS-ROW-OF-PICK: body_of_input falls back to a face next to an edge/vertex the preview consumed (a Fillet's edge).
- Model-core changes (core told): FeatureState.missing_refs, SketchDisplay.on_face (serde default).
RECOGNIZE: CLOSED 09:26 (live offers it nowhere; ours is .hidden(); existing features still evaluate and edit).
NEXT: the P0 line pick (plan above), the section's thick edges, CTX-MENU-FLIP-UP (submenu-at-once: needs live
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
