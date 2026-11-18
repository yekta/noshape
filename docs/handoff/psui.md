# psui (Part Studio UI): current state

Updated 2026-11-18 21:30 UTC. History up to then: docs/handoff/archive/psui.md (not read by default). Standing rulings:
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

## Now (lead order after the 21:1x compaction)

1. Watch 404f2e1f (queued 21:13, #1): Coarse level for big assemblies (> 1 M triangles; xdev-subs A/B: first frame
   unchanged, orbit 10.5 → 13 frames / 4 s, RSS +0.4 GB; the regen result key includes coarser_levels), Fillet's
   triangle handle + 5 px dead zone, no sketch-line midpoint with no tool, Tag "Standard content".
2. In the worktree, tests run on vibe2, to land next (WIP commit eb1ef8894 + later edits):
   - live dialogs-1118: Tab orders of Chamfer / Shell / Hole / Revolve / Linear / Circular all = (new matrix row);
     Esc after Tab left the dialog now does nothing (`FeatureDialog::tabbed_out`, set on `LeaveFocus`, cleared by a
     graphics press; the studio's Cancel skips it while the focus is outside the dialog). Checked = earlier: a tool
     click commits the open dialog, no auto-advance between boxes, first box focused, Hole title = its callout,
     Revolve's Full revolve checkbox.
   - live recheck 11-18 (b5b664aaa) ≠ fixed: a held edge picks 11 px wide (was 9; matrix now 10 px, live 9–10); the
     row tooltip's fade starts at 0.2 over 110 ms (`ROW_TIP_FADE`; it showed 150–167 ms after the row lit); a Part
     Studio box selects at the release (`Viewport::box_by_pick` off: it waited for a GPU readback it never used).
   - View cube corners: live's Trimetric view turned to the corner's quarter (azimuth corner − 15°, elevation ±30°;
     vc.json from iso), was the diagonal (misread ui-parity step: its "Isometric" was the home button). New test
     `cube_hover_map::the_cube_click_grid_from_iso_matches_live` (vc.json's 15 × 15 grid, centre ~(1828, 167)).
3. Next: view cube press/drag rows (≤ 2 px a click, 3+ nothing, a 40 px drag neither orbits nor clicks, a 1.2 s hold
   nothing); the Section arrow turning blue while dragged.
4. live2's ≠ rows: Translate ghost (old place light grey #D6DADD see-through + orange silhouette, the moved part dark
   grey see-through); section preview cap (#E3C9A7 vs live #EBBD8A); Hole callout placement (text by the hole). Our
   frame beside live's: one app session on vibe2 Xvfb.
5. live2 UI speed: rollback bar after a drop 0.24 / 0.14 s (live ~0.1); Mass values 313 ms (live 154). Under the
   11-18 ruling only gaps ≥ 100 ms and ≥ 2× on a whole action count: check against it first.
6. live's other recheck ≠ (not mine unless the lead says): Symmetric on two lines, Trim past a chain's corner (skasm);
   the dimension box's selected text not drawn highlighted (skasm).
7. Queue: suppress row lag, "#" list 0.7 s, chip tooltip to the chip's right, panel offsets shared between studios,
   Mass ~12 px shorter, FEEL-EDGE-VS-FACE-HOLD, Roll to here, press-interval patch
   (/root/psui-scratch/press-interval2.patch), config table keys, chips-expr rows, plane reach decision, asm in-context
   pane strings when live2's capture lands (live/asm/insert-1118/). Not to build: 3D Sketch, Fault analysis.
- Agent ids (11-18 20:xx): core d30002ca-d013-4435-b7d9-7da9cb564cec, blend de66e481-56e2-4005-a5d3-9fa897ca5fde.
- Landed today: 270e489b (72 a turn, drill-point fans), 933d8e41 (apex grid), 45f2d798 + 548d795f (box-drag red
  main), 3fa69088 (Section dialog place, arrow dead zone), 9595bbf05 (tab return), e7d9df8c (Windows nav).
- Set aside: /root/psui-scratch/revol-wip.patch (OCCT revolutions → StripAlgo, not needed).
- Cleanup: vibex7 ~/tmp/psui/priv/dock.noshape when the tab work is over.

## Open decisions

- None.
