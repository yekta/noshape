# psui (Part Studio UI): current state
Updated 2026-11-28 04:5x UTC. History up to then: docs/handoff/archive/psui.md (not read by default). Standing rulings:
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

## STATE 11-28 04:5x (start here)
- 57f48639 landed (as e76d5b079d). This land (rebased on origin/main 04:4x): e895723533 TICKETS ticks of ps47's
  batch 8/9 rows; batch 10 code (Configuration panel/box, VS rows top-down, Variable menu, wrong-type no undo step,
  disabled ✓ focus, sketch owner, start rectangles in live's line order; run.py mass variance, Insert dialog read,
  old reader's null `variables`); TICKETS ticks of qa's batch-10 rows; export fixes (File name field mark, Custom field
  click leaves a caret, run.py: live's "start X Copy N" Part Studio name in the bubble, "Export N Parts" fields by
  label, a clicked field's caret on both sides reads as live's).
- Verified: ps55 config 7/19 411/434; ps56 36 batch-10 + 14 Insert takes 33/50 681/828 (compare after fixes); ps57 38
  export takes 30/38 368/402; ps58 clippy pss clean, undo/wrong-type/export tests ok; ps60 the changed export takes:
  custom-deviation, chordal-zero, file-name-blank, stl-download match (gpui-base `click_places_caret`: the Export
  modal's File name and Custom fields take a click's caret, Tab selects all; T1 export_dialog_keys_as_live).
- What's left is live's (REQUESTS "psui 11-28 01:5x" and "04:1x"): VS "Where used" panel left open on 4 takes,
  Name field read as boolean (qa RECORDER-NAME-FIELD-KIND), sketch null under Create variable, the import toast's
  timing (13 asm takes), live's REST lag of a new instance after ✓ (3 takes), config rclick menu null (10), Undo
  after a box switch, config-suppression-list on a wrong copy; Units Meter vs live's remembered Millimeter
  (EXPORT-REMEMBERED-START).
- NEXT: "Apply export rules to contents" (on, remembered; shown under a ticked "Export unique parts as individual
  files", with a "View export rules" link; live reads the label with the link's text): ExportSettings field + ops +
  prefs + the dialog row (export-parts-three-individual-files s12). Then export-step-spline-part s4 ("Include derived
  tolerances in export" live true / ours false: remembered or default?), ps47 open items
  (mistake-section-view-pick-edge s2, mass-properties-add-part s5 message, MEASURE-BRACKET-KEY), the maker set,
  test-memory P2, remaining batch 8/9 rows.
- Diff per state: scp 'vibe2:tmp/psui/psNN/a/*.json' /root/psui-scratch/psNN/, then
  `python3 /root/psui-scratch/bin/alldiffs.py /root/psui-scratch/psNN <names…>`.
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
