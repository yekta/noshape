# psui (Part Studio UI): current state
Updated 2026-11-28 05:4x UTC. History up to then: docs/handoff/archive/psui.md (not read by default). Standing rulings:
docs/DECISIONS.md (read first; it wins over anything here).

## Setup

- Worktree /root/projects/noshape-wt/psui, branch agent/psui. Commit as yekta (AGENTS.md), subject "psui: …".
- Builds, crate tests and scenario runs on vibe2, one rb.sh at a time: `NOSHAPE_RB_HOST=vibe2 tools/tiers/rb.sh bash -c "$(cat script)"`
  (the script exports OCCT_ROOT=/opt/occt CARGO_PROFILE_DEV_DEBUG=line-tables-only TMPDIR=$HOME/tmp/psui); `git diff --name-only origin/main | xargs touch -c` first. ~/bin/slot
  only for suites, gates, timing pairs (DECISIONS 9e5b481d4). Scripts in /root/psui-scratch/run*.sh. The GPUI dialog matrix is
  `dialogs::the_dialogs_matrix_matches_live` (rows filter: DIALOG_ROW=…).
- Hosts (lead 11-27, vibex6 gone): builds and app/replay runs on vibe2 (with the land queue: one run of mine at a
  time); scenario shards and suites on vibex8 through `~/bin/slot <cmd>` (run-shards.sh points there, an-run.sh at
  vibe2 via slot). Nothing kept on vibex6 survives.
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

## STATE 11-28 05:4x (start here)
- QUEUED 0868ddd7 05:3x (8 commits: batch 10 code, export caret fixes, TICKETS ticks, handoff). Never poll --status;
  the lead reports. Lead + qa told (qa re-scores batches 8/9/10 once green).
- UNCOMMITTED (since 0868ddd7): "Apply export rules to contents": ops ExportSettings/SolidExportPrefs `apply_rules`
  (default true, remembered in keep/give; T1 apply_export_rules_starts_on_and_is_remembered), op param
  `apply_export_rules` (ops/export.rs), dialog row ex-apply-rules under a ticked individual-files box with an
  export_rules_link (export.rs; OPTION_CHECKS now 4), run.py strips live's " View export rules" from checkbox labels.
  The effect on the zip's entry names is NOT implemented: asked in REQUESTS "psui 11-28 05:3x".
- RUNNING ps62 (bg, vibe2, log /root/psui-scratch/resp/ps62.log): clippy pss+ops, ops --lib export, UPDATE_DOCS=1
  coverage:: (regenerates docs/AGENT_API.md ON THE HOST: scp vibe2:agents/psui/src/docs/AGENT_API.md back into the
  worktree before committing), pss --lib export, build, replay export-parts-three-individual-files, -two-selected,
  export-stl-download into ~/tmp/psui/ps62. Then commit "psui: Export's Apply export rules to contents (on,
  remembered, op apply_export_rules)", handoff last, land.
- Verified before: ps55 config 7/19; ps56 33/50 681/828; ps57 export 30/38; ps60 custom-deviation, chordal-zero,
  file-name-blank, stl-download match. Left for live: REQUESTS "psui 11-28 01:5x", "04:1x", "05:3x";
  EXPORT-REMEMBERED-START (Units Meter vs live's remembered Millimeter).
- NEXT: export-step-spline-part s4 ("Include derived tolerances in export" live true / ours false: remembered or
  default?), ps47 open items (mistake-section-view-pick-edge s2, mass-properties-add-part s5 message,
  MEASURE-BRACKET-KEY), the maker set, test-memory P2, remaining batch 8/9 rows.
- Diff per state: scp 'vibe2:tmp/psui/psNN/a/*.json' /root/psui-scratch/psNN/, then
  `python3 /root/psui-scratch/bin/alldiffs.py /root/psui-scratch/psNN <names…>`.
- Commit-order lesson: cherry-picking TICKETS commits onto a newer main union-merges qa's rows twice: grep each ticked
  id's count before land.sh.
- Open (not live-backed): redo of the step before a Properties rename loses the rename; ask live before changing.

## EARLIER
- sketch-tool-insert-image s3: retake asked (REQUESTS "psui 11-27 15:42").
- frame-corner-coped-limit s5 and the other focus disagreements: retakes asked in REQUESTS "psui 11-27 06:2x".

## NEXT
- qa's diffs from live's option-combination batches as filed; interaction-feel rows to verify (archive 11-27).

## Agents

- skasm owns the hover matrix's sk/* cells (lead 11-22); psui the 3D / Part Studio cells.
- Agent ids: ONLY in /root/lead-scratch/roster.md (lead rule 11-19); read it before every message_agent, never copy ids
  into notes. Context cap 200k (user 11-19).
- P0 sketch-vertical-key: matches live on green f6f8069f, 10 of 10 states (qa 11-20).

## Open decisions

- None.
