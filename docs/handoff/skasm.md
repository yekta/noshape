# Handoff: skasm (sketch + assembly), from 2026-10-26

Area (PLAN 2.3): crates/sketch, crates/scene/src/sketch_mode.rs, sketch behaviour in the app (part_studio/sketch.rs),
crates/app/src/assembly_*.rs + assembly_view.rs, crates/model/src/assembly, standard content. Older notes:
docs/handoff/sketch-behaviour.md, docs/handoff/assembly-behaviour.md (skb), checklists docs/parity/sketch.md,
docs/parity/assembly.md.

Setup: worktree /root/projects/noshape-wt/skasm (branch agent/skasm), CARGO_TARGET_DIR=/root/target-skasm, env in
/root/skasm-scratch/env.sh, detached jobs via /root/skasm-scratch/run.sh <log> "<cmd>". GUI: /root/skasm-scratch/ui/
(start.sh, sketch_top.sh, shot.sh, stop.sh; DISPLAY=:103). Live: my own Chrome tab via /root/skasm-scratch/live/r.mjs
(copy of live-pw/r.mjs; tab id in .skasm-tab; CDP connect takes ~35 s under load); scratch Part Studio "skasm poly"
(569dabca19a397e932f2db54) in noshape mm test.

## Landed on main
- cdde1c06 F19 Width mate in any pick order + DOF cache across evaluations, per-group DOF length scale.
- c76cdcf4 assembly least change measured at the bodies' mate connectors (oracle_asm R1 matches live; R1 out of KNOWN;
  R3/C1 now keep rz 0, still KNOWN for live's −157.5°).
- 6329b168 sketch: polygons as live (seed + circular pattern; Inscribed = edge middle, Circumscribed = vertex; no Ø
  dimension), Point tool on the Origin, paste at the pointer + Transform (SKETCH-PASTE), unsolvable constraint keeps
  the shape; API polygon descriptions.

## WIP on agent/skasm
- `WIP skasm (app, after the app split)`: crates/app/src/part_studio/sketch.rs: the constraint shortcut shows the
  unsolvable toast at once (now it shows with the next editor action), dimension menu without "Tolerance and
  precision…" (parity 8.7d 🔧 until then). Land after the lead's app split (rebase, re-find the file).

## Next, in order
1. P1 SKETCH-DRAG-BIG (qa): a drag step in a 1000-entity sketch re-evaluates the sketch feature (72–113 ms). Per step
   the feature regen clones the sketch, solves it again (already solved by the drag), computes regions (pairwise
   curve intersections, hole assignment by winding) and wires. Perf test ready to add:
   `python3 /root/skasm-scratch/dragbig_patch.py` (scene tests/sketch_perf.rs `big_shape_sketch_drag_step_parts`,
   times solve / regions / regen apart). Then make the slow part incremental (sketch crate / model-core regen).
2. agent/perf 7594798b assembly drag bench (viewport.rs; app: after the app split). Patch /root/skasm-scratch/bench.patch.
3. Assembly perf: `cargo test -p noshape-ops --test all assembly_perf:: -- --ignored --nocapture`.
4. GUI-verify docs/parity/workflow-assembly-sweep.md's 🔧 rows (their messages are on main in code).
5. oracle_asm R3/C1 (live turns the flipped pin −157.5°); M1 is live's own failing case.
6. End-of-day walk of sketch + assembly workflows in /root/green/noshape against live (~04:00 UTC).

## Build times (10-26, load 30–55, 4 agents building)
- First build of app + cli on this target dir: 60 min. First test build of ops tests: 20 min.
- After rebasing on c9540ae6 (workspace-hack, tests/all.rs): `cargo test -p noshape-ops --test all mate_flip::`
  recompiled everything: 955 s.
- 10-27, layered crates, load ~30: first build after the rebase 254 s (model-core lib tests). Edit in
  model-core/assembly/solver.rs → `cargo test -p noshape-model-core --lib assembly::solver` 108–122 s; →
  `cargo test -p noshape-model --test all oracle_asm::` 113 s. Sketch-crate edit → `cargo test -p noshape-sketch`
  35 s; scene edit → `cargo test -p noshape-scene --lib` 92 s (first after a sketch change). Clippy of model-core +
  model after an edit 53 s. T0 after a sketch/scene edit 830 s total chain (incl. the tests above).

## Requests to the lead
(none)
