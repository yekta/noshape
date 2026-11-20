# psui (Part Studio UI): current state

Updated 2026-11-20 18:04 UTC. History up to then: docs/handoff/archive/psui.md (not read by default). Standing rulings:
docs/DECISIONS.md (read first; it wins over anything here).

## Setup

- Worktree /root/projects/noshape-wt/psui, branch agent/psui. Commit as yekta (AGENTS.md), subject "psui: …".
- Mac available again 18:06 UTC 11-20 (NOSHAPE_RB_HOST=mac, headless only). Builds and crate tests: plain `NOSHAPE_RB_HOST=vibex8 tools/tiers/rb.sh bash -c "$(cat script)"` (DECISIONS 9e5b481d4;
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

## Now (18:04 UTC 11-20)

1. LANDED: 6f48184a (FS notices compile off the UI thread; A/B 6 max prepaint 534 -> 28 ms on 850efe, 549 -> 37 on
   de4eff), 18809b4c (spinner on the executor clock), 1c6e56e4 + 5c3f3bb7 (revert of a5505f50). Worktrees psui-fsn,
   psui-spin removed.
2. LANDED 87ffadda 17:15 (b3c497b9): a5505f50's parts again + alone-only hover-wait averaging + white/blue flange handle +
   Configurations box pane (FEEL-CONFIG-BOX-WIDTH). run156 green (clippy, part-studio-ui 354, app-core 115, offscreen).
   A/B 7/8 (vibex6, load 5-10): select the same at low load (5-7 ms both); under load both slip (main 64-171, re-land
   96-154), waiting for the first window present after the click. Not a re-land regression.
3. Heavy-doc hover. FOUND (trace diag1, gpui notify backtraces): after 500 ms with no window frame the viewport's
   request_frame took itself as off screen and notified: every hover after a rest redrew the whole window first (present
   100-180 ms on llvmpipe) and submitted its frame from that draw, the frame then sharing the CPU with it. FIX
   (landing 18:13): vendor gpui `App::entity_drawn` (NOSHAPE PATCH: the entity in a window's last frame) decides on
   screen; last_layout on the viewport's clock. Test feel_tests a_hover_after_a_rest_submits_its_frame_without_a_window_redraw
   asserts `Viewport::last_layout()` unchanged (GPUI's test mode draws dirty windows inside flush_effects, so only that
   tells the paths apart); run159 vibex8: passes fixed, fails with the old line; run158 part-studio-ui 355 + app-core 115.
   A/B 11 (vibex6: noshape-b = same tree without the fix,
   noshape-f) -> /root/psui-scratch/ab11.log. A/B 9 hover main -> 87ffadda: block 416-644 -> 271-431, impeller
   415-712 -> 385-600, de4eff 641-770 -> 224-748 (load 13). de4eff frame r.gpu_ms 120-250 (147k tris); A/B 10 per-pass
   skips too noisy under falling load to attribute (none 227 then 82). Fast-move wait = 5 x pick (30-60 ms on de4eff).
   Tools vibex6 ~/agents/psui-keep: ab9/10/11.sh, hf3t.sh, hfeel-t.py, hovtrace.py, hovtrace2.py (+ DIAG lines from
   the trace patch /root/psui-scratch/diag2.py: NOSHAPE_NOTIFY_TRACE, never land); docs in ~/tmp/psui (850efe
   bb49db4ceba0762c, de4eff 874aef1a8b008b1b, impeller 844c39ce20497196, block cc4a172465de10c6).
4. Flange handle vs live flange-handle-1120: the orange inside the heads is the selected edge seen through the hollow
   heads: nothing more to draw.
5. Cleanup: vibex6 ~/tmp/psui/noshape-b, noshape-s, hf/ab7*, ab8* after the A/Bs; vibex8 ~/agents/psui/target size.
   Not to build: 3D Sketch, Fault analysis.

## Agents

- Agent ids: ONLY in /root/lead-scratch/roster.md (lead rule 11-19); read it before every message_agent, never copy ids
  into notes. Context cap 200k (user 11-19).
- P0 sketch-vertical-key: matches live on green f6f8069f, 10 of 10 states (qa 11-20).

## Open decisions

- None.
