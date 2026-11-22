# psui (Part Studio UI): current state
Updated 2026-11-22 23:58 UTC. History up to then: docs/handoff/archive/psui.md (not read by default). Standing rulings:
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

## Now (23:25 UTC 11-22)
Batch 1 LANDED 613b06bb; batch 2 LANDED b6812aae (live's box-hover / multi-part-menu / see-through in it).
Batch 3 (commit "psui: a picked edge solid and bright under the preview…", rebased on 5e634b35e): FEEL-MC-AXIS-UNDER-DISC,
FEEL-FILLET-FACE-OVER-PICKED (#ffaf25 via `LineSegment::OVER_LIT_FACE` on consumed_picks_overlay's band),
FEEL-PICKED-EDGE-DASHED-UNDER-PREVIEW, FEEL-CORNER-BREAK-VERTEX-PICK (disc only), FEEL-HOVER-DISC-AFTER-COMMIT
(`ui.hover_held_at`), PARTS-ROW-OF-PICK-SHEET-METAL; and the macOS CI P0 (run 45071676527: list × viewport p04
"Top plane outline not lit"): not MSAA (t73 passes at NOSHAPE_PERF_MSAA=4); p04 now samples 28 points along the
plane's 4 edges and prints viewport size + hover on failure. t72: psui lib 504/505 (the one: my test's 3 px move
needed `approach`, fixed), scene, render green. t74 (clippy + both tests, ~/tmp/psui/t74.log) → land.sh → watch macOS CI.
USER P0 (23:3x, lead): Mac mini M6 appearance + union. Live answered (behaviours "psui P0: STEP import colours…").
TICKETS block "psui, 2026-11-22 23:5x": dx IMPORT-SOLID-COLOUR-ON-EVERY-FACE, core BOOLEAN-UNION-FIRST-TOOL-IDENTITY,
psui APPEARANCE-PART-UNDER-FACE-COLOURS (test + app check after dx). Both told. t76 (kernel dump_step_colours on
~/tmp/psui/mm.step on vibex8, log ~/tmp/psui/t76.log) → send dx the per-part face colour counts (live: 152/258, 14/529).
Render side already as live (display.rs: part props > import colour; a face colour over it).
Corner break arrow: solid in our render at f5ebb8b3 (dumped): ask qa to re-check after it lands.
EDIT-BODY-SHADE stays open (ours top (118,127,133), live (132,147,155)).
Queue: (CONFIG-FILTER-HELP-STILL-POINTER landed: app-core `ui::help_tooltip`, title beside the box then the 400 px
help under it after TOOLTIP_EXPAND_DELAY; tell asm, its assembly panel has the same box) P2 FEEL-ROW-TIP-AFTER-DBLCLICK, UIS-LIST-ROW-MENU-ROWS, CUSTOM-FEATURE-ROW-ICON,
CUSTOM-FEATURE-DIALOG-HELP, UIS-FEATURE-INITIALS (row test-named "feature:…" only, mod.rs ~4770).
- CONFIG-FILTER-HELP-STILL-POINTER: we have no hover help on the filter box at all (only the funnel's menu,
  config_panel.rs CONFIG_FILTER_COMMANDS); live: 'Filter inputs' tooltip, then a 'Supported filters' popover. To build.
- FEEL-SKETCH-INF-END-DOT: lead deciding whether it counts; sketch_mode is skasm's (tell skasm first).
- asm (lead ruling): software draws 1x while moving, 4x after 150 ms quiet (worker `Settle`, pipelines_alt); tests
  calling Renderer::render stay 1x. asm sends the sha: rebase, re-run render + hover_matrix.
- PS-RAIL-STOPWATCH-WARNING: the red badge is the software-renderer performance error (workspace.rs rail-performance,
  `perf_error`, live's own badge); replays run on llvmpipe. Tell qa: environment, not a bug.

Open qa P0s (TICKETS "psui, 2026-11-22 …"):
- TRANSFORM-ROTATE-EDGE-SENSE: the sense matches live (qa amended); only the preview look is left (batch 2's opaque moved
  part should cover it; the orange axis edge to check).
- CTX-MENU-FLIP-UP (P1): the vertical flip already exists (anchored SwitchAnchorOrSnap, nav-feel-1119). Ask qa to re-check.
skasm owns FEEL-MC-* Part Studio set (10) + SKETCH-DIM-PLACE-PICKS-PLANE (11-22 18:0x); mate connector inference code is
skasm's. Told them batch 1 carries the Shift lock, ConnectorLook::Placed and the with-layer hover mask.
Then: assembly column of sel-gestures (tell asm first); autocomplete list position. Lead rule: end turns at ~180k context.

## Agents

- skasm owns the hover matrix's sk/* cells (lead 11-22); psui the 3D / Part Studio cells.
- Agent ids: ONLY in /root/lead-scratch/roster.md (lead rule 11-19); read it before every message_agent, never copy ids
  into notes. Context cap 200k (user 11-19).
- P0 sketch-vertical-key: matches live on green f6f8069f, 10 of 10 states (qa 11-20).

## Open decisions

- None.
