# psui (Part Studio UI): current state
Updated 2026-11-27 16:2x UTC. History up to then: docs/handoff/archive/psui.md (not read by default). Standing rulings:
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

## IN FLIGHT 23:2x (after compaction: start here)
- QUEUED ec4c4387 21:4x (edge reach, maker fixes incl. used-curve clicks, arenas, export title/prefs, rel_px,
  Insert view text, TICKETS). Wait for the lead's word; never poll --status. Then: report (<=5 lines: maker 7/13 +
  filed rows, memory 6.76 -> 3.7 GB, reach + export landed), cc skasm on SKETCH-COINCIDENT-FREE-LINE-SLIDES /
  SKETCH-TANGENT-ARC-SPAN + REQUESTS "psui 11-27 16:26".
- Committed 7ca0f099cc + this change (landing next): batch-9 state (part_data, Appearance editor kind "appearance",
  Appearances panel); part_data reads props from before an open colour editor's change; Hide drops hidden parts from
  the selection; Material "- None -" + OK removes it; field marks (Properties Name/Material, Material panel custom);
  face menu: Curve/surface analysis… on a curved face, Section view… on a planar face or a face+edge selection;
  `features::sketch::inner_regions` (port of parity.rs qSketchRegion(…, true)) -> SketchRegionDisplay.inner, left
  out of a sketch row's preselection / region pick (qa EXTRUDE-PRESELECTED-SKETCH-FILLS-HOLE). core uses
  inner_regions for eval.rs Reference::Sketch and lands after mine: TELL CORE when mine is QUEUED (and the name).
- ps43 batch 9: 27/49, 479/538. Left: .focus.selection caret on click in the editor's #/A fields (live 3/2, ours 0);
  colour hex before Enter (appearance-part-hex-enter etc: live keeps 9DCFED in part_data while editing, check step
  6 = after Enter?); mass 2.319 vs 2.32 digits; material-library-dropdown (dropdown:Library not shown);
  mistake-rename-blank message "Name is required."; Material panel Tab order / focus.kind field; Properties Escape;
  mistake-surface-assign-material (surface menu: Hide other surfaces/Hide all surfaces, New sketch… rows);
  PS-MENU-CYLINDER-FACE: live's cylinder (fillet) face menu has View normal to + Section view… + Curve/surface
  analysis… (appearance-maker-plate-r8-face); ours lacks the first two.
- sketch-used 0/19: skasm's (lead: SKETCH-USED-CURVES-NOT-SELECTABLE is skasm's P0; don't duplicate).
- Then (lead 11-27 23:0x): ASM-INSERT-CANCEL (P2: Insert panel ✕ closes, inserts nothing, toast "Assembly insertion
  has been cancelled. Restore") and ASM-INSERT-ROW-ADDS-INSTANCE (P3: row click must not add; only ✓) (TICKETS
  4927fa0759).
- Then qa batch 8 (TICKETS 2d235a33de "qa 11-27 23:2x", scored on green 5128614a4, before ec4c4387): rescore on main
  first; P2 EXPORT-CUSTOM-FIELDS-NO-FOCUS, EXPORT-N-PARTS-DIALOG ("Export 2/3 Parts" + 2 checkboxes),
  SECTION-VIEW-ARROW-FLIP (+ {"world"} drag target), SECTION-VIEW-NEXT-FIELD (focus to Items to exclude; edge pick
  doesn't clear the plane), MEASURE-BRACKET-KEY ("[" opens Measure); P3 PART-MENU-CURVE-ANALYSIS,
  MASS-PROPERTIES-ADD-PART-FOCUS, EXPORT-TOAST-DOCUMENT-NAME (run.py drops live's " Copy N");
  ASM-MASS-PROPERTIES-BUTTON (lead's/asm; mine if only the button name).
- Export open: "Include hidden instances in export" still remembered.
- Hosts: vibe2 = builds/app runs (one at a time, land queue shares it); vibex8 for shards via ~/bin/slot.

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
