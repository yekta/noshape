# psui (Part Studio UI): current state
Updated 2026-11-27 13:2x UTC. History up to then: docs/handoff/archive/psui.md (not read by default). Standing rulings:
docs/DECISIONS.md (read first; it wins over anything here).

## Setup

- Worktree /root/projects/noshape-wt/psui, branch agent/psui. Commit as yekta (AGENTS.md), subject "psui: …".
- Builds, crate tests and scenario runs on vibe2, one rb.sh at a time: `NOSHAPE_RB_HOST=vibe2 tools/tiers/rb.sh bash -c "$(cat script)"`
  (the script exports OCCT_ROOT=/opt/occt CARGO_PROFILE_DEV_DEBUG=line-tables-only TMPDIR=$HOME/tmp/psui); `git diff --name-only origin/main | xargs touch -c` first. ~/bin/slot
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

## IN FLIGHT 13:2x (after compaction: start here)
- LANDED 1b6506ae as 6c2f0fe4d (Windows click fix, assembly focus, Sweep Thin test); qa told 13:2x (db8fa95d3,
  1e07dfe6c; asm-mate-connector-realign retake).
- QUEUED 13:2x: Replicate scope/mates (ps21), fillet Vertices + before_vertex_under (ps22/ps23), Insert image
  Libraries search box, image report (2 HORIZONTAL + other geometry), TICKETS (SPLIT-TRIMMED-PARTIAL-ERRORS for core;
  9 duplicate rows from the union merge dropped: check main again after the land). ps23: part-studio-ui 664 ok;
  fillet-variable-two-vertices-ok, sketch-image-place, sketch-text-font-typed-ok match; sketch-image-edit s17
  .dialog.fields[1].checked live false / ours true (not looked into yet).
- LEAD 13:2x (P0 audit, skasm has the sketch side): Part Studio side of B-spline picking on an imported part (corpus
  STEP): GPUI tests for click, shift/ctrl multiselect, box select, Select › Tangent connected edges/faces and the other
  Select items, hover in each selection state; fix any missed pick or early-stopping chain. Report <= 5 lines.
- Edge reach (PICK-FACE-OVER-NEAR-EDGE), held, uncommitted (stash psui-reach while landing): renderer.rs pick_reaching
  `body`, builder.rs body_fn, viewport.rs pick_body, behaviors.rs, offscreen.rs, feel_tests.rs (2 edge tests). TEMP
  trace in renderer.rs (NOSHAPE_PICK_TRACE, never commit). Land only when split-face-ok s7 matches AND live answers
  REQUESTS "psui 11-27 08:46". Backups resp/reach-trace.patch, resp/wip-1150.patch.
- frame-corner-coped-limit s5 and the other focus disagreements: our rule is today's runtime (live 04:19 confirmed);
  the takes are pre-11-21, retakes asked in REQUESTS "psui 11-27 06:2x": wait for them, no code change.
- Edge reach: ps18 trace: Part 1's edge (id 15, body 1, prio 8) has a pick pixel at d2 = 25 (5.0 px, the line's width)
  from the split-face-ok click over Part 2's face (prio 2): kept at radius 5, so it wins. Needs live's cutoff over
  another part's face: REQUESTS "psui 11-27 08:46" extended 11:2x (5.5 / 6 px). Don't land before the answer.
- live 11-2x: "Release all configurations…" takes queued (6362d352, start config-ps); "Disable imported appearances"
  is the Appearances panel row's item (needs recorder work: not asked for).

## NEXT
- qa's diffs from live's option-combination batches as filed; interaction-feel rows to verify (archive 11-27).

## Agents

- skasm owns the hover matrix's sk/* cells (lead 11-22); psui the 3D / Part Studio cells.
- Agent ids: ONLY in /root/lead-scratch/roster.md (lead rule 11-19); read it before every message_agent, never copy ids
  into notes. Context cap 200k (user 11-19).
- P0 sketch-vertical-key: matches live on green f6f8069f, 10 of 10 states (qa 11-20).

## Open decisions

- None.
