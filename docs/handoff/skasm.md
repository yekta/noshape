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
- b154b333, 507ce0e5, 57cab72a: test of the assembly error texts against live's (assembly sweep rows checked).
- 4ca39911 assembly fuzz test (T2).
- 3bcb5145 SKETCH-DRAG-BIG: sketch regions' hole search by bounding box (regions 38 → 3.5 ms, regen 47 → 12 ms per
  drag step on 1000 entities; perf test scene sketch_perf::big_shape_sketch_drag_step_parts).
- 6329b168 sketch: polygons as live (seed + circular pattern; Inscribed = edge middle, Circumscribed = vertex; no Ø
  dimension), Point tool on the Origin, paste at the pointer + Transform (SKETCH-PASTE), unsolvable constraint keeps
  the shape; API polygon descriptions.

## WIP on agent/skasm
- `WIP skasm (app, after the app split)`: crates/app/src/part_studio/sketch.rs: the constraint shortcut shows the
  unsolvable toast at once (now it shows with the next editor action), dimension menu without "Tolerance and
  precision…" (parity 8.7d 🔧 until then). Land after the lead's app split (rebase, re-find the file).

## Next, in order
1. (Landed 10-27: the app part, toast at once, no Tolerance item.) Polygon commit: live also shows a pattern count label with its glyph (behaviours 160); ours has none. (LIMIT-PLACEHOLDER
   of 10-27 was a false alarm: the limit fields select all on focus; my test's Tab had focused the field and the
   click then placed the caret, as a browser does.) The drag bench is on main (psui c7f26470).
2. Live answers pending (REQUESTS.md, skasm 2026-10-27): polygon side count after the 2nd click (pointer distance?),
   polygon Ø readout position while drawing.
3. Sketch regen per drag step ~11 ms on 1000 entities (dev): eval 6.3 ms (solve 2.1, regions 3.2, wires 0.6), the rest
   regen/display overhead. Inside the frame; a release build has more headroom.
4. GUI-verify the rest of docs/parity/workflow-assembly-sweep.md's 🔧 rows (1–5, 7, 8, 10, 12–18, 25 ✅ 10-27).
5. oracle_asm R3/C1 (live flips the pin about an axis at −78.75° (rz −157.5°), ours about x; equally short flips:
   needs live's rule); M1 is live's own failing case.
6. Assembly fuzz (T2) `random_assembly_edits_never_panic`: `ASM_FUZZ_SEEDS=1..200 ASM_FUZZ_STEPS=120 cargo test -p
   noshape-assembly --lib assembly::tests::random_assembly_edits_never_panic -- --ignored --nocapture` (clean 10-27).
7. End-of-day walk of sketch + assembly workflows in /root/green/noshape against live (~04:00 UTC).

Assembly perf (10-27, dev build, `cargo test -p noshape-ops --test all assembly_perf:: -- --ignored --nocapture`):
200 pins / 300 mates: evaluation again 6.5 ms, adding a mate 16.5 ms, inserting a pin 7.8 ms (was ~50), drag steps
1.9–2.8 ms (frame with the scene 2.2–3.1 ms); 1000 free pins: drag 0.55 ms, insert 3.8 ms. All inside the budgets.

## Build times (10-26, load 30–55, 4 agents building)
- First build of app + cli on this target dir: 60 min. First test build of ops tests: 20 min.
- After rebasing on c9540ae6 (workspace-hack, tests/all.rs): `cargo test -p noshape-ops --test all mate_flip::`
  recompiled everything: 955 s.
- 10-27, noshape-assembly crate: edit → `cargo test -p noshape-assembly --lib assembly::tests::<name>` 23–32 s.
- 10-27, layered crates, load ~30: first build after the rebase 254 s (model-core lib tests). Edit in
  model-core/assembly/solver.rs → `cargo test -p noshape-model-core --lib assembly::solver` 108–122 s; →
  `cargo test -p noshape-model --test all oracle_asm::` 113 s. Sketch-crate edit → `cargo test -p noshape-sketch`
  35 s; scene edit → `cargo test -p noshape-scene --lib` 92 s (first after a sketch change). Clippy of model-core +
  model after an edit 53 s. T0 after a sketch/scene edit 830 s total chain (incl. the tests above).

## Requests to the lead
(none)
