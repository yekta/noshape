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

## Now (lead order after the 22:5x compaction)

1. Landed 22:28 as fea9cbbe (9662a162 + 404f2e1f). Live retakes once /root/green passes it (green is df6be9b6 now).
2. Queued (land 23:xx, see `land.sh --status`): view cube hold ≥ 490 ms no click; view-cube-iso scenarios via the
   view menu; Transform ghost (test a_transforms_old_place_is_a_light_grey_ghost); Hole callout leader 14 px and its
   glyph mesh `on_top` (in-app recheck shot: /root/psui-scratch/mk-hole-shot.py makes the scratch scenario); tooltips
   wrap at 400 px; a hidden sketch's eye only on hover; chip tooltip right; "#" list redraw; configuration names
   spreadsheet keys (Tab out of the add row is the input's IndentInline action, not a key event); X11 press-interval
   double-click; selected cap colour render test. Part Studio lib 273 passed, app-core, render caps, clippy clean.
2b. HOLD re-queues until the lead's land-daemon fix 58d20676 lands (~00:50, lead says so), then ONE stack.
   Re-queued 23:55 as 65f89ac4 (failed 00:00, daemon bug) (5 commits: box hold, unit precision, 2a6b696b's rows; the 23:38 failure was the land
   queue's empty-file-list glitch). Hole callout glyphs: filled but small / thin, size and weight asked of live
   (REQUESTS 23:33).
2c. Lead 23:4x: xdev-subs cold first open (qa 40.7 s). Profile (qa's app.log, vibe2 A/B): click -> regen of the
   inserted Part Studios one at a time (7.5–8 s at load 17–25), the Insert catalog's studios one at a time (3–4 s),
   apply 0.6–1 s, then one llvmpipe frame ~5 s (not ours). In the worktree: `AssemblyEvaluator::prewarm` regenerates
   the uncached studios side by side (rayon) before `prepare_inner`: evaluate 3.4–3.6 s, full display ~5 s sooner.
   The catalog's prewarm doesn't take yet (DBG counts in run34); T2 `dx_perf::timed_cold_assembly_evaluation`
   (NOSHAPE_ASM_PERF=<copy>); NOSHAPE_AB_NO_PREWARM is a scratch A/B switch: remove before landing. Doc copy
   vibe2 ~/tmp/psui/asm/xdev.noshape (asm's rebuild, not private). Live's open: 83–94 s.
2d. Live shortcuts-ps-1118 (5 ≠): Shift+X withdrawn (a saved panel spot); view keys don't re-light under the resting
   pointer (feel test a_view_key_lights_what_comes_under_the_resting_pointer, running); Shift+T clears the selection
   (done, test); Delete part: ours solid tan, live a ghost with an orange silhouette (scratch shot mk-del-shot.py);
   Shift+Y and the slider default wait for live's lane 4.
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
