# psui (Part Studio UI): current state
Updated 2026-11-28 11:4x UTC (qa's sketch-used rows). History up to then: docs/handoff/archive/psui.md (not read by default). Standing rulings:
docs/DECISIONS.md (read first; it wins over anything here).

## Setup

- Worktree /root/projects/noshape-wt/psui, branch agent/psui. Commit as yekta (AGENTS.md), subject "psui: …".
- Crate tests (part-studio-ui, scene, assembly-ui) on the Mac's 6-job slot when free: `NOSHAPE_RB_HOST=mac tools/tiers/rb.sh …`
  (lead 11-28 07:4x; keeps load off vibe2's land queue). App builds and replays stay on vibe2.
- Builds and scenario runs on vibe2, one rb.sh at a time: `NOSHAPE_RB_HOST=vibe2 tools/tiers/rb.sh bash -c "$(cat script)"`
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

## STATE 11-28 12:3x (start here)
- 3ff853aa LANDED (main 3a9f10842e..4bd4ec52d9). QUEUED now (12:3x, rebased on main): Ctrl+click beside an open menu
  reopens it there (os_menu dismisses and lets the press through; the sketch menu and a Ctrl+press take the used curve
  under the cursor via `used_curve_under_cursor`); used spline menu without Insert spline points; line + spline
  "Intersection angle"; ui-parity Variable table tables (hook variable_table_state); Extrude… from a sketch with only
  curves selected = Surface with those curves (`selected_sketch_curves`, start_feature; live ctrl-multiselect s85);
  ui-parity reports the renamed Variable's "Update all references" box (`update_references_shown`). Five qa rows ticked.
  Mac ps91 (614 sketch ui tests) + ps92 green. Lead reports; never poll.
- ps90 replay: ctrl-multiselect s83-84 match; s82 rows differ only because ours used an arc at s62 where live used a
  point (sketch geometry); s85 fixed after ps90 (not replayed yet: next replay batch). vs-ps-variable-table-after-edit
  step 1: live's "Where used" panel is stale live state: told qa/live (12:3x).
- Open from qa's list: P3 ASM-INSERT-PANEL-PART-LIST (asm-insert-spline-part-ok s3-5; with ASM-INSERT-DIALOG-LIST).
- Then: Apply export rules (waits REQUESTS 05:3x), Realign retake (REQUESTS 09:3x), PICK-FACE-OVER-NEAR-EDGE (P3).
- Section view (fd74228711): dialog fields are ui-script `field:` targets; a Parts list row goes into Items when
  active (selected till ✓), ignored with Section planes active; T1 a_part_row_goes_into_the_section_dialogs_items_field.
  ps78: section-view-exclude-part, -include-tab-part 7/8 (s2 hint: live 09:2x: a pick within ~0.6 s keeps it; a take race); SECTION-VIEW-NEXT-FIELD ticked.
- ps79 (to-verify rows): 12/17; ticked SWEEP-PART-FACE-PROFILE-ADD, MEASURE-READOUT-SKETCH-REGION,
  FULL-ROUND-EMPTY-FIELDS-RED, LOFT-PROFILE-PICK-SKETCH-REGION, ASM-DIALOG-FLIP-TOGGLE-NAME. The other 5 (09:3x):
  pattern boss takes = core's evaluator: FEATURE-PATTERN-BOSS-NOREAPPLY (core) filed; Sweep thin s7 (Trim ends on: ours
  sweeps, live fails) = SWEEP-THIN-EDGE-NO-TRIM's kept difference (parity rule); asm-mate-connector-realign s4 = an odd
  11-14 take (every other take focuses the revealed query field, as ours): retake in REQUESTS 11-28 09:3x.
- 10:1x: EXPORT-REMEMBERED-START done in the runner (per-format "STL"/"STEP" seed blocks, export toast copy name; ps82
  5/6, the 6th = Apply export rules). DIALOG-TAB-CLICK-KEEPS-FIELD-FOCUS verified (ps79). PICK-FACE-OVER-NEAR-EDGE: f558a0cb64
  on main; split-face-ok s7 still picks the edge (our edge end shows ~1.3 px into Part 2 under the edge depth bias, the
  rounded click lands 5.0 px off); a strict reach breaks live's 5 px hold (render test): renderer-depth fix, P3, open.
- Crate tests now on the Mac slot (NOSHAPE_RB_HOST=mac, /root/psui-scratch/resp/ps75.sh pattern); replays on vibe2.
- 4562f16f's content: Export remembers individual files; Measure panel reported to ui-parity; no-material notice once
  per measured set; flash_seq toast timers (ps68 rerun 30/30, 124/124; tickets ticked).
- ps69 maker set: 7/13 match; the 6 that differ all have TICKETS rows (SKETCH-COINCIDENT-FREE-LINE-SLIDES,
  SKETCH-TANGENT-ARC-SPAN (skasm), SKETCH-DIM-CIRCLE-RIM-TOP (psui, waits REQUESTS 11-27 20:4x), UIP-COMPARE-*,
  extrude-offset-ring recorder owner). Nothing new.
- Export left open: Apply export rules live off (REQUESTS 05:3x/06:0x), EXPORT-REMEMBERED-START (Millimeter),
  export-step-spline-part s4 derived tolerances (same remembered class), import toast timing (REQUESTS 04:1x).
- Verified before: ps55 config 7/19; ps56 33/50 681/828; ps57 export 30/38; ps60 custom-deviation, chordal-zero,
  file-name-blank, stl-download match. Left for live: REQUESTS "psui 11-28 01:5x", "04:1x", "05:3x";
  EXPORT-REMEMBERED-START (Units Meter vs live's remembered Millimeter).
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
