# psui (Part Studio UI): current state

Updated 2026-11-19 12:44 UTC. History up to then: docs/handoff/archive/psui.md (not read by default). Standing rulings:
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

## Now (updated 12:44 UTC 11-19; Ubeyd at 98 % weekly: keep this current, land small)

1. LANDED 10:11 (03a8f126): HEAVY-ASM-SOFTWARE-FRAME renderer side (no 3D frame for a refresh that changes nothing:
   `CameraGeometry::key`, `small_content_key` in `Scene::same_as`; `Style::software` = 1x MSAA + each body's coarsest
   level), the list-1119 / nav-feel-1119 / tools-1119 rows, Shift+C. 1aef7f70 (coplanar on Metal/D3D12) landed 08:32.
   asm owns the LOD meshes / load path.
2. HEAVY-ASM: done. vibe2 llvmpipe asm-xdev-subs: steady frame 1.2 -> 0.11 s, first frame after the tab 5.97 -> 1.95 s,
   wheel notch 3.62 -> 0.72 s; the one 9.2 s evaluate was load (reruns: base 6.2 / 3.0 s, new 3.6 / 3.9 s). asm's
   d7691003 (software proxy at the coarsest level) on top: frames 0.04-0.07 s. Prewarm timing dropped (qa).
3. Appearance swatch: = on vibe2 (next 3D frame, 60-120 ms after the swatch tick on llvmpipe; pxwatch.c / swclick2.sh
   in /root/psui-scratch; app on MY display :394 only, :393 is asm's). Row says retake on a GPU.
4. Routing curve Point edit type Multiple (core's missing-commands row 4), native in triad.rs: a click on a point
   toggles it in selectedIndices, the triad at the selected points' centre moves each one's dx/dy/dz
   (dialogs_feel_tests::a_routing_curves_multiple_triad_moves_the_selected_points). Test note: point items set
   directly are reset by the editing logic; type offsets via set_expr_text. Not done: the triad's rotation rings,
   an Edit reference's triad: asked of live (REQUESTS psui 11-19 13:0x, routing-multi).
   NOW (lead): no ≠ rows of mine left but the two waiting on live (section cap, Hole callout); taking P0 rows that are =
   with no test (interaction-feel.md "missing" in the test column: hover-p0, hover-pick, sel-feel, dialogs first).
   Done: edge double-click (dialogs_feel_tests). Next: 862 sketch region look, 810 refusal texts, 884/885.
   qa's suite rows (P0) arrive ~16:00 UTC: they go first.
5. Open with live: Hole callout glyph size (REQUESTS 23:33), section preview over a shown plane (REQUESTS 22:48).
   live is stopped until ~21:00 UTC: recheck asks go to live2.
6. Not to build: 3D Sketch, Fault analysis. Cleanup: vibex7 ~/tmp/psui/priv/dock.noshape when the tab work is over;
   vibe2 ~/tmp/psui/hv binaries after the reruns. Branch psui-backup-1119 = pre-reset branch (all landed; delete).

## Open decisions

- None.
