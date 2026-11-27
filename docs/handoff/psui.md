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

## IN FLIGHT 20:5x (after compaction: start here)
- 21:3x: ps40 green (readout, export prefs/title, runner rel_px, drawing-ui check); ps39 full psui lib run with two
  arenas: peak 3703 MB (was 6.76 GB), 670 passed, 1563 s. LANDED via land.sh 21:3x (wait for the lead's word; never
  poll --status). After it: report (<=5 lines: maker 7/13 + filed rows, memory 6.76 -> 3.7 GB, reach + export landed),
  cc skasm on its two rows, rerun maker + sketch-used sets on main.
- NEXT (live 21:2x, batch 9 0865c00b, not on main yet): ui_script state pieces per tests/ui-parity/README.md (Part
  Studio panels section): (1) "part_data" when the scenario sets "part_data": true: per part name, part_number,
  description, material, appearance {hex, opacity 0..255, default}, hidden; (2) Appearance editor as dialog kind
  "appearance" (A, #, R, G, B, Tessellation quality; tabs Palette/Mixer; Enter applies and closes, no ✓); (3) the
  Appearances panel in panels (id "appearances", rows). Live facts: behaviours.md "Depth batch 9".
- Branch rebased on origin/main 20:5x, commits (not landed yet): 704e0a91d4 edge reach 5 px over another part's face;
  b3d560e6f1 maker flow fixes (used outline counterclockwise, Intersection angle, Perp lines angle, Use quick click,
  N centre, related sketch, glyph inert, dump); 8f17e5e730 two malloc arenas in UI test binaries; a209af82f9 Export
  "Export N Parts" + unique parts not remembered (DECISIONS 11-27) + runner drag `"to": {"rel_px": [dx, dy]}`
  (the `{"world": [x, y, z]}` target already existed in part_studio/ui_script.rs); f2d903b869 TICKETS.
- 01daa2e25e (qa 21:xx, P3 UIP-DIALOG-TEXT-NATIVE-SELECT): Insert view's chosen source dumped as a select value, so
  run.py's panel_text_form drops it with the options for live2 >= 11-17.3 takes. After the rebase mark qa's row fixed
  (not on my base yet). qa's world-drag note: done (world existed; rel_px added in a209af82f9).
- ps40 (resp/ps40.sh, vibe2 after ps39's memory run): scene selection_readout (the compile fix), ops
  export_remembers + prefs::, psui --lib export, app ui_script::. Green -> land.
- ps39 results: maker 6/13 (7/13 with live's retake 8d0f42b8f on main: tangent-connected-used matches); sketch-used
  0/19 all at s13 constraints 6/5 = skasm's spline Horizontal, fixed on main b73df15bec: rerun both sets after the land.
  Full psui lib run with 2 arenas: see ~/tmp/psui/mem3/hwm.txt on vibe2 / ps39.log T-MEM-DONE (ps38 base 6.76 GB).
- Filed 20:5x: TICKETS "psui 11-27 20:5x: the maker flow" (skasm SKETCH-COINCIDENT-FREE-LINE-SLIDES,
  SKETCH-TANGENT-ARC-SPAN; mine SKETCH-DIM-CIRCLE-RIM-TOP waits for REQUESTS "psui 11-27 20:4x"; lead
  UIP-COMPARE-ITEM-ORDER, UIP-COMPARE-FREE-CLICK-DERIVED). After the land: cc skasm on its two rows + REQUESTS
  "psui 11-27 16:26"; report to the lead (<=5 lines).
- Export open: "Include hidden instances in export" is still remembered (no live take checks it; DECISIONS lists only
  Format, Options, Resolution, Units, Y axis up). Batch 8's export-* and section-view-* takes are on 78dc566b3e, not
  main yet: replay them when live lands them.
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
