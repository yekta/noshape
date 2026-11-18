# psui (Part Studio UI): current state

Updated 2026-11-18 17:50 UTC. History up to then: docs/handoff/archive/psui.md (not read by default). Standing rulings:
docs/DECISIONS.md (read first; it wins over anything here).

## Setup

- Worktree /root/projects/noshape-wt/psui, branch agent/psui. Commit as yekta (AGENTS.md), subject "psui: …".
- Builds and crate tests: `NOSHAPE_RB_HOST=vibex6 tools/tiers/rb.sh bash -c '~/bin/slot …'` (Mac for Mac-only checks).
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

## P0s 11-18 (lead order 13:1x)

1. HEAVY-ASM-SOFTWARE-FRAME (asm-xdev-subs). Live's display rule, final (behaviours "psui (lead 11-18 ~13:55): display
   mesh by surface type, read from the viewer"): 12 / 24 / 72 / 300 segments a turn (Coarse / Medium / Fine=Auto /
   Very fine); cylinders and cones one band (side 2n, pointed side n, caps n-fans); spheres n × n/2; tori n·n·2;
   edges reuse the face boundary points. Viewer counts: _787261002 428,069, EFL673ZZ 17,568, E673ZZ 12,096,
   Gehaeuse 23,466, Finger 1,702.
   - Landed 270e489b (17:09; rides the 05:00 train): 72 a turn + drill-point fans (OCCT StripAlgo). Then (17:50,
     landing with the feel batch): the Rust mesher's grid (brep::mesh) at a cone's apex: the ring radius rounds to
     ±1e-18 and the normal flipped, rows 8 µm apart; collapsed derivatives are a pole now. asm-xdev-subs: 7.76 M →
     2.14 M triangles, headless render 18.1 → 7.3 s; Gehaeuse 24,356 (live 23,466), Finger 1,708 (1,702), bearings
     and screws within 5 %; _787261002 254k (live 428k: ours under).
   - WIP set aside: /root/psui-scratch/revol-wip.patch (OCCT revolutions → StripAlgo): not needed for these faces.
   - Next: asm (d8d597e0) ABABs main; qa's on-screen run; the Coarse-level decision (lead 12:4x conditions).
2. live2's ≠ rows: Translate ghost (live m6-grid: old place light grey #D6DADD see-through + orange silhouette, the
   moved part dark grey see-through; ours: old body colour × 0.3 alpha, moved part unchanged); section preview cap
   (#E3C9A7 vs live #EBBD8A = 80 % toward #FFC685, which our shader does: find what pales it); Hole callout (live
   h1-cbore: text on the top face right beside the hole, short leader; ours runs the leader to the part's edge). All
   three need our frame beside live's: one app session on vibex6.
3. live2 UI speed (green c7d388e8): rollback bar after a drop 0.24 / 0.14 s (live ~0.1: the bar reads the studio at
   once, so it's the SQLite commit on the UI thread + the frame); Mass values 313 ms (live 154; bg_job has no delay).
   Frame-log capture each.
4. Tag (live std 3112): the tooltip text and the "Standard content" purpose with "Standard content part"
   (/root/psui-scratch/tag.patch, set aside from the P0 land: apply, run ops coverage / AGENT_API, land).
5. asm: live's in-context pane strings (Switch to assembly, Change assembly configuration…, Update <context>,
   "Update all out of date contexts") are mine when live2's capture lands (live/asm/insert-1118/).
6. Live 11-18 retakes still to compare: view cube clicks (live/feel/viewcube-1118/vc.json, a 10 px click grid from
   iso with the resulting camera: the hidden bottom face's band ~30 px under the cube, corners and edge bands at 30°
   elevation / azimuth multiples, arrows 15°) against `overlay.rs` CORNER_HIT / SILHOUETTE_REACH_PX; Fillet's
   handle a white-outlined ~16 px triangle on the edge's middle (ours an arrow) and its ~5 px dead zone.
7. Queue: suppress row lag, "#" list 0.7 s, chip tooltip to the chip's right, panel offsets shared between studios
   (store rects), Mass ~12 px shorter, FEEL-EDGE-VS-FACE-HOLD, Roll to here (live2 hold), press-interval patch
   (/root/psui-scratch/press-interval2.patch), live pattern-hole rows / live2 field-keys tests, config table keys,
   chips-expr rows, plane reach decision. Not to build: 3D Sketch, Fault analysis (server flags off).
- Done today: tab return 9595bbf05 (qa: 111 vs 151 ms, in the 17:00 cut); Windows nav e7d9df8c (12:57).
- Cleanup: vibex7 ~/tmp/psui/priv/dock.noshape when the tab work is over; vibex6 ~/tmp/psui/heavy after the mesh work.

## Open decisions

- None.
