# psui (Part Studio UI): current state

Updated 2026-11-19 22:06 UTC. History up to then: docs/handoff/archive/psui.md (not read by default). Standing rulings:
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

## Now (updated 22:48 UTC 11-19; this thread replaced 4973e295 at 13:27, Ubeyd's weekly limit)

1. LANDED 13:59 (e87a2b3e): routing Multiple offset, edge double-click / region look / constraint refusal tests,
   section cap, Hole callout size.
2. LANDED 22:04 (01969838): qa's P0 feel rows (menu press closes only the menu; one Esc after a field's menu item;
   no readout under an analysis tool), the untested-row tests, Routing curve Multiple with its value box. qa reruns
   its 7 rows on its green automatically.
3. QUEUED 22:48 b5fce1fd (6 commits): Mass CoM / inertia Override; TOOLBAR-TOOLTIP-AFTER-CLICK (TooltipPressGate +
   Window::tooltip_released_on; TICKETS marked); Routing Multiple centre ball; sketch-vertical-key fix (BAND_HOLD_PX 8);
   row tests. On green: tell qa (TOOLTIP + vertical-key re-run). Follow-up: the ball test's expected centre comes from
   f32 mesh positions (core 22:4x) -> take it exact (DVec3(0.04, 0.015, 0.005)) and compare the text again.
4b. P0 qa STALE-SKETCH-HOVER (TICKETS P1 walk 11-19): outside sketch mode a sketch region's hover outline (after a rim
   click + ✓) and a circle centre point's orange dot stay after the pointer leaves to empty space (also after Shift+7);
   rims and part faces clear fine. Leads (unverified): hover flows viewport.set_hover (emits only on a GPU id change,
   app-core viewport.rs ~1466) -> apply_hover_pick (mod.rs ~1971) -> shown_sketch_hover_id (sketch.rs 843: CPU reach
   8.5 px points / 7.5 curves) ; PointerMove re-checks via ui.shown_sketch_gpu_hover (mod.rs ~2356), which is None
   while sketching. Suspects: (1) after ✓ the viewport's hover id N is re-read against the new built picks (N now the
   region) and nothing re-emits; (2) the scene's HOVER flag set by the viewport outlives self.hover (a None->None
   change never refreshes). Repro first as a GPUI test with explicit Hover events.
   qa 22:3x narrowed: (b) is a ONE-EVENT LAG, in sketch mode too (line, Esc, one xdotool move to empty: the end stays
   lit; a second move clears it). (a) the region outline after ✓ survives several moves: separate path. Read so far:
   sketch-mode PointerMove (sketch.rs ~1190) sets ed.hover from hovered_sketch_entity() = the OLD self.hover, then the
   re-check at ~1222 (sketch_hover_id vs ui.sketch_gpu_hover) calls sketch_hover which does reset ed.hover — so check
   whether that re-check is reached after Esc (Select) and whether viewport pick_at / request_hover re-emits Hover
   when the async pick equals the stale id (viewport.rs ~2575 fast-move branch, set_hover ~1466).
5. Interaction-feel rows that match live with no test (grep "missing" in the Test column; P0 sections first).
   Skipped: the two-block box counts (live's camera: iso + 10 wheel steps about an unknown pointer), the rollback
   bar's hover colour (GPUI group_hover, no test hook), the spinner's 900 ms (`Instant`, not the test clock).
   Asked of live2: the diagonal submenu move's timing (REQUESTS 16:37).
6. Not to build: 3D Sketch, Fault analysis. Cleanup: vibex7 ~/tmp/psui/priv/dock.noshape when the tab work is over;
   vibe2 ~/tmp/psui/hv binaries. Branch psui-backup-1119 (all landed; delete).

## Agents

- Agent ids: ONLY in /root/lead-scratch/roster.md (lead rule 11-19); read it before every message_agent, never copy ids
  into notes. Context cap 200k (user 11-19).
- P0 sketch-vertical-key (lead after compaction: "match live's default view orientation, re-baseline"): checked 21:1x:
  ours already opens in live's start view, Camera::default = Trimetric (-0.433, 0.75, -0.5) = live's recorded start
  camera direction (every live take with a camera); live's Isometric menu item is the true (-1, 1, -1)/√3, as ours
  (view-cube-iso). The (64, -45) vs (61, -42) gap is the zoom of the camera-less 11-06 take (~0.2885 mm/px implied;
  live's empty start is 0.2754 in 17 of 25 takes, 0.2507 in today's). live2 retook it with its camera (cf70e676):
  live shows no Vertical at 0.2507. NEXT: once cf70e676 lands, run sketch-vertical-key on ours (uip-shots.sh); if ours
  infers Vertical on live's camera, the gap is the reach measure (screen distance to the projected plumb line) — with
  skasm. No orientation change, no re-baseline unless that run shows one.

## Open decisions

- None.
