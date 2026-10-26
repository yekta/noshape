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
(nothing yet: paused by the lead 10-26 ~22:40 while the crates are split)

## Parked on origin agent/skasm (on main c9540ae6+, 10-26 ~22:40), in order
1. `skasm: Width mate solves in any pick order …` (F19 + DOF cache, from skb b826d8f8, plus `group_length`: the
   DOF analysis and its cache key use each group's own length scale, so an instance inserted far away no longer
   misses every group's cache entry). **Checked:** `mate_flip::` (new layout) passes, model `--lib
   assembly::solver` 15/15, clippy -p noshape-model -p noshape-ops clean (on the pre-rebase base). **Not yet:** T0
   (the run was stopped by the pause). Next: T0, then push to main.
2. `WIP skasm: …` (sketch, not yet compiled; GUI-checked on :103 against live where noted):
   - tools/tiers/t0.sh (lead's): the "in no test binary" check skips crates that keep cargo's test discovery (no
     `autotests = false`; crates/app has one test file, merge-tests.py leaves it alone). Without it T0 fails on main.
   - Polygons as live (behaviours "skasm (2026-10-26) Sketch polygon commits"): 2nd click Inscribed = an edge's
     middle (circle tangent), Circumscribed = a vertex (circle through it); seed edge + circular pattern about the
     circle centre (`CircularPatternLink` on its ends, one Coincident joint), Inscribed: construction midpoint +
     Midpoint + Tangent, Circumscribed: seed end Coincident on the circle; the click's inference goes to its point;
     no Ø dimension box after the commit (live makes none); spinner by the clicked edge; preview dots at the centre
     and vertices. Tests: scene `polygon_second_click_is_an_edge_middle_inscribed_and_a_vertex_circumscribed`,
     `polygon_sides_typed_and_midpoint_line`, sketch tests/shapes.rs `polygons_and_slot` (DOF 4, area).
   - Unsolvable constraint (V on a Horizontal line): the shape before the solve stays, toast "Sketch could not be
     solved." (`solve_after_constraint` returns false; test `vertical_on_a_horizontal_line_does_not_solve`).
   - Point tool on the Origin (or any point) makes a new point Coincident with it (behaviours "dx (2026-10-26, for
     skasm) Point tool on the Origin"); test `point_tool_on_the_origin_makes_a_point_coincident_with_it`.
   - SKETCH-PASTE (ticket): Ctrl+V lands centred on the pointer, selected, Transform tool active, no prompt
     (test `copy_paste_selection`).
   - Dimension context menu without "Tolerance and precision…" (live has none).
   - docs: parity/sketch.md 7.14, 8.7, 8.7a–d, 11.12, 14.24 ✅ (GUI evidence), 4.19 🔧 until the polygon commit
     lands; TICKETS P1-4 data closed (fixed since 5c94bb51).
   To land: `cargo test -p noshape-sketch`, `cargo test -p noshape-scene --lib sketch_mode`, clippy -p
   noshape-sketch -p noshape-scene -p noshape --all-targets, T0; GUI re-check of the polygon commit (glyphs);
   mark SKETCH-PASTE [x].

## Next, in order
1. Land 1 and 2 above (after the lead's crate split: rebase, fix paths).
2. oracle_asm R1 (−6° turn on a Revolute): least change measured at the body's mate connectors (mean), not its part
   origin. Patch ready: `python3 /root/skasm-scratch/anchor.py` (edits model/assembly/solver.rs least_change, adds
   `anchor`). Baseline oracle passes (/root/skasm-scratch/oracle-base.log: R1 turn 0.1048 rad). After the patch:
   `cargo test -p noshape-model --test all oracle_asm:: -- --ignored --nocapture`; if R1 matches, drop it from KNOWN
   in the same commit; no other case may change status.
3. agent/perf 7594798b assembly drag bench (viewport.rs `BenchKind::Drag` + bench.sh doc; patch in
   /root/skasm-scratch/bench.patch, unbuilt): build, run once (NOSHAPE_PERF=1 NOSHAPE_PERF_ORBIT=300
   NOSHAPE_PERF_BENCH=drag on tests/perf/pins-assembly-1000.noshape), land with a note (psui's file).
4. Assembly perf: `cargo test -p noshape-ops --test all assembly_perf:: -- --ignored --nocapture` (insert into 200
   pins / 300 mates; targets: edits < 100 ms, drag step < 16.7 ms).
5. GUI-verify docs/parity/workflow-assembly-sweep.md's 🔧 rows (all their messages are on main in code).
6. oracle_asm M1 is live's own failing (over-constrained) case: low value. CP1/R3/C1 wait on core's cylinder axis.
7. End-of-day walk of sketch + assembly workflows in /root/green/noshape against live (PLAN: ~04:00 UTC).

## Build times (10-26, load 30–55, 4 agents building)
- First build of app + cli on this target dir: 60 min. First test build of ops tests: 20 min.
- After rebasing on c9540ae6 (workspace-hack, tests/all.rs): `cargo test -p noshape-ops --test all mate_flip::`
  recompiled everything: 955 s. Edit → test time with incremental builds: not measured yet.

## Requests to the lead
- tools/tiers/t0.sh: see parked item 2 (its check fails on main for crates/app's single test file).
