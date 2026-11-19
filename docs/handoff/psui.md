# psui (Part Studio UI): current state

Updated 2026-11-19 12:06 UTC. History up to then: docs/handoff/archive/psui.md (not read by default). Standing rulings:
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

## Now (updated 12:06 UTC 11-19; Ubeyd at 97 % weekly: keep this current, land small)

1. LANDED 10:11 (03a8f126): HEAVY-ASM-SOFTWARE-FRAME renderer side (no 3D frame for a refresh that changes nothing:
   `CameraGeometry::key`, `small_content_key` in `Scene::same_as`; `Style::software` = 1x MSAA + each body's coarsest
   level), the list-1119 / nav-feel-1119 / tools-1119 rows, Shift+C. 1aef7f70 (coplanar on Metal/D3D12) landed 08:32.
   asm owns the LOD meshes / load path.
2. HEAVY-ASM timing, vibe2 llvmpipe, release, asm-xdev-subs Assembly 1 tab (/root/psui-scratch/hv.sh, hvparse.py;
   out dirs in /root/psui-scratch/out-{base,new,pre}): steady frame 1.2 s -> 0.11-0.12 s; first frame after the tab
   5.97 -> 1.95 s; a wheel notch on screen 3.62 -> 0.72 s. Assembly evaluate 4.1 s (base) vs 9.2 s (new) in one run:
   not touched by the change; rerunning base/new x2 (hve.sh -> vibe2 ~/tmp/psui/hv/e-{b1,n1,b2,n2}, queued in slot
   behind asm) to confirm it is load. Report to the lead once in. Prewarm timing: dropped (qa: no regression).
3. live tools-1119 swatch recheck (484-678 ms on loaded vibex6): measured on vibe2 llvmpipe with a 2 ms X pixel
   watcher (/root/psui-scratch/pxwatch.c, swclick2.sh; app on MY display :394 only, :393 is asm's): the part
   recolours 60-120 ms after the swatch's own tick = the next software 3D frame. Row updated "to retake on a GPU".
4. Next feel rows (≠ in my area, by frequency): section view preview caps (row 1160), Hole callout (1463), then
   core's Routing curve Point edit type Multiple (needs FeatureManipulator TOGGLE_POINTS / FULL_TRIAD kinds,
   custom/host/status.rs; std routingCurve.fs 787-1002; aeb0cc0a7 added the params; low priority).
5. Open with live: Hole callout glyph size (REQUESTS 23:33), section preview over a shown plane (REQUESTS 22:48).
   live is stopped until ~21:00 UTC: recheck asks go to live2.
6. Not to build: 3D Sketch, Fault analysis. Cleanup: vibex7 ~/tmp/psui/priv/dock.noshape when the tab work is over;
   vibe2 ~/tmp/psui/hv binaries after the reruns. Branch psui-backup-1119 = pre-reset branch (all landed; delete).

## Open decisions

- None.
