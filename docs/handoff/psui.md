# psui (Part Studio UI): current state

Updated 2026-11-19 08:28 UTC. History up to then: docs/handoff/archive/psui.md (not read by default). Standing rulings:
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

## Now (updated 08:28 UTC 11-19)

1. Queued: 04edd2ba (lead P0 coplanar on Metal/D3D12, alone), 56e21c9e (list-1119 / nav-feel-1119 / tools-1119 rows +
   Shift+C), two docs-only duplicate-row cleanups on top. DX12 for 04edd2ba: watch the next CI run.
2. IN THE WORKTREE: TICKETS P0 HEAVY-ASM-SOFTWARE-FRAME, renderer side (lead 08:1x; split with asm: asm owns the LOD
   meshes / load path, psui crates/render + viewport scheduling):
   (1) no 3D frame for a refresh that changes nothing: `CameraGeometry::key` (content hash, `key_of`) on ScreenTriads,
       MateGlyphs, ConnectorGlyphs, InferenceMarks; `Viewport::set_camera_geometry` compares keys (was Arc pointers:
       every assembly refresh with a mate glyph shown drew a frame); and small line / point uploads (≤ 4096) always
       carry their content hash (`small_content_key`), which `Scene::same_as` compares (asm's overlay lines / connector
       dots were uploaded anew each refresh: a new scene each time). Tests count `Viewport::frames_requested` (UI tests'
       Gpu::headless never returns frames, so submits stop at 1): assembly-ui
       a_refresh_with_the_same_mate_glyphs_submits_no_frame (5 refreshes: 0 requests; pointer compare 5; neither 10),
       part-studio-ui ui_only_redraws_submit_no_3d_frame (+ a face selected does request). UI tests run on the noop
       adapter (not Cpu): the software path is covered by the render tests only.
   (2) `Style::software` (set by the viewport from `Gpu::is_software` = adapter DeviceType::Cpu; print keeps full):
       1× MSAA (`msaa_of`) and each body at its coarsest display level (DrawLists.coarsest). asm-xdev-subs: finest
       2.1 M triangles, Coarse 462 k (asm). Tests: render a_software_rasteriser_draws_each_body_at_its_coarsest_level,
       lib a_software_rasteriser_draws_without_msaa. Render tests keep Style::default (software false): real-GPU
       look unchanged.
   Next: run52 (/root/psui-scratch/run52.sh), then measure: release app on Xvfb with asm-xdev-subs (copy of vibex8
   ~/agents/asm-keep/qa-save/asm-xdev-subs.noshape; qa's /root/qa-scratch/heavy/hvs.sh), before/after frame time and
   frames per view change; tell asm the llvmpipe frame time (asm adds a level below Coarse if still too heavy).
3. After: dx's prewarm on scoped threads (f1e0c74e): once landed, check the big-assembly first open isn't slower
   (asm-xdev-subs cold open, ~4 s on vibe2 after 1dd4033be); qa times it too. Never block a rayon worker on a thread
   that needs rayon.
4. Open with live: Hole callout glyph size (REQUESTS 23:33), section preview over a shown plane (REQUESTS 22:48).
5. Not to build: 3D Sketch, Fault analysis. Cleanup: vibex7 ~/tmp/psui/priv/dock.noshape when the tab work is over.

## Open decisions

- None.
