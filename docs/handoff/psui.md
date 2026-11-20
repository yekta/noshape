# psui (Part Studio UI): current state

Updated 2026-11-20 16:30 UTC. History up to then: docs/handoff/archive/psui.md (not read by default). Standing rulings:
docs/DECISIONS.md (read first; it wins over anything here).

## Setup

- Worktree /root/projects/noshape-wt/psui, branch agent/psui. Commit as yekta (AGENTS.md), subject "psui: …".
- Mac off limits until 18:05 UTC 11-20 (lead). Builds and crate tests: plain `NOSHAPE_RB_HOST=vibex8 tools/tiers/rb.sh bash -c "$(cat script)"` (DECISIONS 9e5b481d4;
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

## Now (16:30 UTC 11-20)

1. LANDED: 6f48184a (FS notices compile off the UI thread; A/B 6 max prepaint 534 -> 28 ms on 850efe, 549 -> 37 on
   de4eff), 18809b4c (spinner on the executor clock), 1c6e56e4 + 5c3f3bb7 (revert of a5505f50). Worktrees psui-fsn,
   psui-spin removed.
2. THIS WORKTREE = main + 729fcf7c3 "psui: wip re-land" (a5505f50's parts again: kept picks, pick area, rollback drop
   row, section arrow, config labels) + uncommitted: hover wait averaged only from picks asked alone (pick_requests
   5th field `alone`), flange handle white at rest / blue lit, Configurations box a title over a 45 px scroll pane
   (FEEL-CONFIG-BOX-WIDTH), feel test a_pick_held_up_behind_a_frame reworked. run156 (vibex8) verifies; then squash
   into ONE commit "psui: …" (check rebase clean + diff --stat; TICKETS union duplicates) and land.sh.
   A/B 7/8 (vibex6, load 5-10, S = main+fix, B = re-land): select the same at low load (5-7 ms both); under load both
   slip (S 64-171, B 96-154): the click's change waits for the first GPUI window present, which competes with the
   llvmpipe frame. Not a re-land regression.
3. Heavy-doc hover (lead: de4eff 466, impeller vs live 113-259): hover = fast-move wait (5 x avg pick, cap 250 ms) +
   pick 2-10 + frame ~80-90 ms (llvmpipe, block) + GPUI present ~40-60. First hovers sat at the 250 cap (avg inflated
   by a pick queued behind the first frame): fixed by alone-only averaging (uncommitted). Next levers: the frame's
   llvmpipe time and the present contending with it. Tools on vibex6 ~/agents/psui-keep: ab7.sh, ab8.sh, hfeel-t.py
   (move times "t", clicks "tc"), hf3t.sh, hovtrace.py (NOSHAPE_FRAME_LOG trace); binaries ~/tmp/psui/noshape-s/-b;
   docs in ~/tmp/psui (850efe bb49db4ceba0762c, de4eff 874aef1a8b008b1b, impeller 844c39ce20497196, block
   cc4a172465de10c6).
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
