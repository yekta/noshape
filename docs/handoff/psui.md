# psui (Part Studio UI): current state

Updated 2026-11-18 23:00 UTC. History up to then: docs/handoff/archive/psui.md (not read by default). Standing rulings:
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

## Now (lead order after the 22:5x compaction)

1. Landed 22:28 as fea9cbbe (9662a162 + 404f2e1f). Live retakes once /root/green passes it (green is df6be9b6 now).
2. Queued (land 23:xx, see `land.sh --status`): view cube hold ≥ 490 ms no click; view-cube-iso scenarios via the
   view menu; Transform ghost (test a_transforms_old_place_is_a_light_grey_ghost); Hole callout leader 14 px and its
   glyph mesh `on_top` (in-app recheck shot: /root/psui-scratch/mk-hole-shot.py makes the scratch scenario); tooltips
   wrap at 400 px; a hidden sketch's eye only on hover; chip tooltip right; "#" list redraw; configuration names
   spreadsheet keys (Tab out of the add row is the input's IndentInline action, not a key event); X11 press-interval
   double-click; selected cap colour render test. Part Studio lib 273 passed, app-core, render caps, clippy clean.
3. Speed rows (rollback drop 0.24 s, Mass values 313 ms): frame log 23:00 (/root/psui-scratch/lat, green on a local
   Xvfb, NOSHAPE_UI_TIMING): handlers < 1 ms, frame layout+paint 8 ms, but presents 100–150 ms apart (software
   Vulkan): both gaps are two software-rendered frames. Decision asked of the lead: measure on a GPU.
4. live's other recheck ≠ (skasm's): Symmetric on two lines, Trim past a chain's corner.
5. Queue: plane reach decision, asm in-context pane strings when live2's capture lands (live/asm/insert-1118/),
   Roll to here (on hold for live2), live's answer on the cap over a shown plane (REQUESTS 22:48). Not to build: 3D
   Sketch, Fault analysis.
- Agent ids (11-18 20:xx): core d30002ca-d013-4435-b7d9-7da9cb564cec, blend de66e481-56e2-4005-a5d3-9fa897ca5fde.
- Landed today: 270e489b (72 a turn, drill-point fans), 933d8e41 (apex grid), 45f2d798 + 548d795f (box-drag red
  main), 3fa69088 (Section dialog place, arrow dead zone), 9595bbf05 (tab return), e7d9df8c (Windows nav).
- Set aside: /root/psui-scratch/revol-wip.patch (OCCT revolutions → StripAlgo, not needed).
- Cleanup: vibex7 ~/tmp/psui/priv/dock.noshape when the tab work is over.

## Open decisions

- None.
