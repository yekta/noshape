# psui (Part Studio UI): current state

Updated 2026-11-18 23:58 UTC. History up to then: docs/handoff/archive/psui.md (not read by default). Standing rulings:
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

## Now (updated 02:35 UTC 11-19)

1. Landed 02:27 as 1dd4033be (a46fd345: box hold, unit precision, xdev prewarm, view key re-light, Shift+T, rows).
2. Queued 02:3x as 70d7c71d (3 commits): Shift+Y clears the selection; Shift+C in a sketch selects the analysed
   curves; Isolate / Make transparent sliders remembered (`feature_settings` view:isolate / view:makeTransparent);
   the mesher test race (mesh_pixels flipped `set_rust_mesh_everywhere` for every thread; now its BRepMesh meshes are
   made on its own thread): it failed nothing, but showed in a46fd345's check log.
   After it lands: queue 65df5b2ca (rows only: edge vs face reach =, triad and named view restore = with their tests,
   view cube hold / # list to retake, sketch hover timing closed, plane reach on slanted edges asked).
3. Lead 02:3x: the interaction-feel ≠ rows by frequency (hover/select/drag first). All psui ≠ rows are now fixed
   or waiting on live: plane reach on a slanted edge at 6–9 px (asked live 02:3x), section preview colour over a
   shown plane (REQUESTS 22:48), Hole callout glyph size (REQUESTS 23:33); retakes once green has 1dd4033be.
4. Open with live: Hole callout glyph size and weight (REQUESTS 23:33). Dialog spots remembered per dialog
   (live: localStorage, sectionview +150/+100 like ours) = already.
3. Speed rows closed (lead 23:14): rollback 0.24 s and Mass 313 ms are software-render frame cadence (handler < 1 ms,
   frame 8 ms; frame log /root/psui-scratch/lat). Rule: act on a feel row only if our handler + layout + paint
   exceeds ~100 ms or the event order differs from live.
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
