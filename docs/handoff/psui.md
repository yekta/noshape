# psui (Part Studio UI): current state

Updated 2026-11-20 09:57 UTC. History up to then: docs/handoff/archive/psui.md (not read by default). Standing rulings:
docs/DECISIONS.md (read first; it wins over anything here).

## Setup

- Worktree /root/projects/noshape-wt/psui, branch agent/psui. Commit as yekta (AGENTS.md), subject "psui: …".
- Builds and crate tests: plain `NOSHAPE_RB_HOST=vibex8 tools/tiers/rb.sh bash -c "$(cat script)"` (DECISIONS 9e5b481d4;
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

## Now (09:57 UTC 11-20; this thread moves to a fresh one at 10:00, Motile's weekly limit)

1. LANDED 09:29 as 37741d38: the whole 11-20 batch (EDGE-MIDPOINT-REST-LOOK, FEEL-DIM-PICK-READOUT,
   FEEL-ROLLBACK-DROP-ROW, FEEL-SECTION-ARROW-FOOT, HOVER-LEAVES-WINDOW, FEEL-FAST-OFF-CLEAR, PARTS-LIST-PROPERTIES,
   TABLES-STICKY-HEADER-HSCROLL, MEASURE-CIRCLE-DEFAULT-PLANE; the four never-ticked TRANSFORM-GHOST-LOOK /
   SECTION-PREVIEW-COLOUR / HOLE-CALLOUT-PLACE / FLFEEL-BAR-MENU ticked "to verify"). qa, studios, asm, skasm told
   09:3x. Nothing queued. (620709c4 had failed 07:55 on two tests: the 3D click rows now `pointer.settle` after
   each click for its GPU pick; the FS triple-click test expects live's newline after studios' vendor change.)
2. IN FLIGHT, uncommitted in the worktree (a backup in stash "psui-feeltests-1000": drop it, found by tag, once
   committed; if the worktree lacks them, apply it by SHA from `git stash list --format='%H %gs'`): tests for the last untested Part Studio feel rows (interaction-feel 574 / 575 / 577):
   - part-studio-ui sketch_glyph_feel_tests::a_lines_glyph_row_is_20_px_boxes_22_px_apart_20_5_px_off_its_middle
     (Show constraints on, rectangle; reads `glyph-{cid}-{eid}` debug bounds). run136 failed only because one rectangle
     line has no glyphs; fixed (skips empty rows, needs a row of 2+); run138 reruns it (log psui-scratch/run138.log).
   - scene sketch_mode tests a_dimension_arrowhead_is_16_by_7_px (`arrow`), geometry_being_drawn_is_lives_preview_blue
     (overlay of Line / Circle / CornerRect, rgb(68,156,205) 2 px) + clippy on scene and part-studio-ui: run137.
   Both runs started before the stash: if their logs are empty or stale, rerun after applying the stash (touch the two
   files). When green: commit, fill the Test cells of rows 574 / 575 / 577, land.
   Rows 578-580 (planes/axes colours, mate connector size, in-context colours) stay "–": constants calibrated on live's
   screenshots; a test would only restate them.
3. Untested "=" rows elsewhere in interaction-feel (56) are asm's (mates, instance list, asm views, 3D in an assembly)
   and studios' (documents page, drawings) sections: not psui's. No psui row is ≠ (1313 units is the lead's decision).
4. Waiting on live: the Configurations box's label column (ours 80 px fixed, config_panel.rs `div().w(px(80.))`; live
   ~32 px for "Size"): REQUESTS 06:40. No change until live answers. Also the diagonal submenu timing (REQUESTS 16:37).
5. Cleanup: vibex8 ~/agents/psui/target is 21 GB (over the 20 GB rule): clear target/debug once no run uses it. vibe2's
   psui target cleared 07:5x; branch psui-backup-1119 deleted (all landed). Not to build: 3D Sketch, Fault analysis.

## Agents

- Agent ids: ONLY in /root/lead-scratch/roster.md (lead rule 11-19); read it before every message_agent, never copy ids
  into notes. Context cap 200k (user 11-19).
- P0 sketch-vertical-key: matches live on green f6f8069f, 10 of 10 states (qa 11-20).

## Open decisions

- None.
