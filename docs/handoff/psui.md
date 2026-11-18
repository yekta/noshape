# psui (Part Studio UI): current state

Updated 2026-11-18 13:49 UTC. History up to then: docs/handoff/archive/psui.md (not read by default). Standing rulings:
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

1. HEAVY-ASM-SOFTWARE-FRAME (asm-xdev-subs, 7.76 M triangles, llvmpipe ~17 s/frame; asm: repeats already share one
   upload, instancing gave nothing: it's the triangle count). Live's display rule (behaviours "psui (lead 11-18
   ~13:05): live's display tessellation RULE", live/psui/mesh-1118/rule/): Coarse 30° / Medium 15° / Fine 5° / Very
   fine 1.2° per segment, chord = body diagonal / 2 × (1 − cos(angle / 2)); Auto = Fine everywhere in a Part Studio.
   Live's Fine counts vs ours: Gehaeuse 23,466 vs 1,168,544; Finger 1,702 vs 266,240; _787261002 460,748 vs 690,978;
   EFL673ZZ 18,840 vs 67,716; E673ZZ 13,248 vs 46,112; screw 4,944 vs 18,410.
   - Cones: drill points (apex, a degenerated edge) went to BRepMesh, 275 k triangles per 1 mm cone. mesh.cpp
     StripAlgo now fans rim → apex (test strip_mesh: drill point + spike). Still refused on the gripper: 4 faces have
     a side neither const-u nor const-v (trace "STRIPSIDE"), 2 fail the apex check ("STRIPAPEX"): temporary
     NOSHAPE_STRIP_DEBUG prints in mesh.cpp (/root/psui-scratch/mesh.cpp.fan is the clean version): remove before
     landing.
   - Density: ours was 144 per circle (2× live per direction, tori 4×); live (13:4x) confirmed 72 on the viewer
     (edge fits a 72-gon within 0.36 px). In the worktree: TessQuality 12 / 24 / 72 / 300, live's chord tolerance in
     `display_deflection`; display_tessellation tests updated (side = 144 triangles).
   - Then: re-measure (heavy-probe.py + heavy-run.sh, vibex6 ~/tmp/psui/heavy mode 700, delete after), tell asm
     (d8d597e0) to probe the branch, re-measure on screen with qa, then decide the Coarse level (lead 12:4x: off the
     UI thread after the fine one, only above a triangle threshold, no time-to-first-frame change, memory reported).
2. Land queue: 94d326d2 (feel batch + named view) checking since 12:57; list-matrix "a click away commits a rename"
   failed twice in the queue: 9e1c2b5ee waits for !ui_busy and moved rows now print why. c01c81325 Hole no-point
   tooltip (tested). Land both with the cone fix.
3. live2's ≠ rows: triad at box centre + named view (queued), Hole tooltip (done); left: Translate ghost / preview
   colours (old place grey + orange silhouette, preview dark grey), section preview cap (#E3C9A7 vs live #EBBD8A /
   #E9C497: live = part colour 80 % toward #FFC685, which our shader does: find what pales it), Hole callout placement
   (live lays it on the face beside the hole).
4. live2 UI speed on green c7d388e8: rollback bar after a drop 0.24 / 0.14 s (live ~0.1); Mass Volume / area 313 ms
   after the pick (live 154; dialog 94 ms =). bg_job has no delay: profile with NOSHAPE_FRAME_LOG.
5. Queue: suppress row lag, "#" list 0.7 s, chip tooltip to the chip's right, panel offsets shared between studios
   (store rects), Mass ~12 px shorter, FEEL-EDGE-VS-FACE-HOLD, Roll to here (live2 hold), press-interval patch
   (/root/psui-scratch/press-interval2.patch), live pattern-hole rows / live2 field-keys tests, config table keys,
   chips-expr rows, plane reach decision.
- Done today: tab return 9595bbf05 (qa: 111 vs 151 ms, in the 17:00 cut); Windows nav e7d9df8c (12:57).
- Cleanup: vibex7 ~/tmp/psui/priv/dock.noshape when the tab work is over; vibex6 ~/tmp/psui/heavy after the mesh work.

## Open decisions

- None.
