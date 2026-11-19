# psui (Part Studio UI): current state

Updated 2026-11-19 07:50 UTC. History up to then: docs/handoff/archive/psui.md (not read by default). Standing rulings:
docs/DECISIONS.md (read first; it wins over anything here).

## Setup

- Worktree /root/projects/noshape-wt/psui, branch agent/psui. Commit as yekta (AGENTS.md), subject "psui: …".
- Builds and crate tests: plain `NOSHAPE_RB_HOST=vibe2 tools/tiers/rb.sh bash -c "$(cat script)"` (DECISIONS 9e5b481d4;
  ~/bin/slot only for suites, gates, timing pairs). Scripts in /root/psui-scratch/run*.sh. The GPUI dialog matrix is
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
- sbs.py: side-by-side crops (ours offset (242, 90)); mirror-sxs/ holds the Mirror side by sides with README.
- vibe ~/tmp is pruned after 6-12 h: keep results in ~/agents/psui-keep.

## Now (updated 07:50 UTC 11-19)

1. Landed tonight: 1dd4033be, 373dd5b9, 36c58917 (edge 5/7 px), 24792169 (feature-dialogs-1119), 16297c80 (coplanar
   depth rank), test batches. Queued: c9379de5 (+1dfda2a83, config tests), 04edd2ba (lead P0, alone: the depth rank's
   step adds 1/32 px of the face's depth slope (`depth_step_px`, shaders.wgsl) and float-depth faces push by
   `face_push` (override constant) instead of the bias constant; Metal reproduced CI's (14, 14) on the Mac, 0/49 after;
   test covers perspective, ortho, near grazing). DX12 unverified locally: watch the next CI run.
2. Queued as one land (list-1119 / nav-feel-1119 / tools-1119 rows to =):
   - list-1119: `ROW_DRAG_PX` 5.8; no ⚠ under a suppressed dependency (`missing_selections`); a fresh folder clicked
     keeps its features selected (`select_folder`) so its menu is the selection's with Hide other / all sketches
     (live p6); list rows' side tooltip beside the list (`with_list_row_tooltip`, anchor reaches past the 5 px inset).
   - nav-feel-1119: Where used icon (+ `OsMenu::named_icon`, test where_used_has_its_icon); anchored flip (vendor);
     free line ends 4 px; Parts eye not added while its menu is open (run49 ≠, now `.when(!menu_open)`); line vs
     midpoint menus (test a_sketch_lines_menu_and_its_midpoints).
   - tools-1119 (live 07:5x): a part's appearance editor draws the part outlined, untinted
     (`Highlighting::untinted_parts`, refresh on open), a swatch recolours the display at once (Arc::make_mut +
     rebuild_scene), ✓/✕ clear a part's selection (a3-cancelled); dialogs row "Appearance / the part outlined…".
   - live recheck 11-19: Shift+C in a sketch takes the selected curves only (else all); test
     curve_analysis_in_a_sketch_takes_the_selected_line_only.
   run51 green (part-studio-ui 293, scene 179, app-core 112, clippy clean); queued as one land (see --status).
3. After: dx's prewarm on scoped threads (f1e0c74e): once landed, check the big-assembly first open isn't slower
   (asm-xdev-subs cold open, ~4 s on vibe2 after 1dd4033be); qa times it too. Never block a rayon worker on a thread
   that needs rayon.
4. Open with live: Hole callout glyph size (REQUESTS 23:33), section preview over a shown plane (REQUESTS 22:48).
5. Not to build: 3D Sketch, Fault analysis. Cleanup: vibex7 ~/tmp/psui/priv/dock.noshape when the tab work is over.

## Open decisions

- None.
