# Plan: make Booleans fast by replacing OCCT's slow parts, one at a time

For the lead. Written 2026-10-27 from a read of main (kernel-robustness handoff, `crates/kernel/cpp/boolean.cpp`,
`patches/occt/`, OCCT 8.0.1's `BOPAlgo_PaveFiller_5.cxx`).

## Why Booleans first

Every Extrude, Revolve, Hole, Pattern, Mirror and Boolean feature ends in `op_boolean`
(`crates/kernel/cpp/boolean.cpp`, called from `regen/eval.rs::boolean_into_with`). The two largest speed gaps against
Onshape are both inside it:

| Case | noshape now | Onshape | Where the time goes |
|---|---|---|---|
| Diamond knurl, 8,142 faces (`KNURL=diamond cargo test -p noshape-kernel --test perf knurl_like_onshape -- --ignored`) | 210 s, ~3.9 GB | 32 s | EF ~50–60 s, MakeBlocks ~60–100 s, MakePCurves ~21 s, VF ~12 s, FF ~10 s |
| Extrude 3 under the 400-fin plate (`model/tests/pattern_second.rs`) | 2.7–3 s | 55 ms | a whole-part boolean that visits all ~1,600 faces to change a few |

The goal is speed with **no change in results**. A full Rust kernel comes later; this plan only swaps out the
slow parts, and each part it adds is also a first piece of that kernel.

## Rules for every phase

1. **Same results.** Same validity, the same face/edge/vertex counts, volume within 1e-9 relative (or live's band
   where the test already uses one), and the same history (Modified / Generated / Deleted / Kept), because
   persistent naming is built on it. A faster path that changes a single name is a regression.
2. **Deterministic.** Output, including `MapShapes` order, must not depend on thread count or scheduling.
   Persistent naming and every index-based test depend on it. Check each case at 1, 4 and 16 threads.
3. **OCCT stays as the fallback.** Every new path checks its own preconditions and declines anything it doesn't
   handle. The existing fast paths in `op_boolean` (`fuse_on_face`, `cut_through_holes`, `cut_slabs`, `fuse_slabs`)
   already work this way; keep that structure. Each new path gets an env switch
   (`NOSHAPE_BOOL_<NAME>=0`) so a mismatch can be checked against plain OCCT immediately.
4. **Measure before and after.** No speed claim without the replay numbers from Phase 0.
5. The usual team rules apply: C++ checked with GCC 13, zero warnings, patched OCCT rebuilt by CI from
   `patches/occt/**` (locally: swap only `lib/libTKBO.a` in `/root/occt`).

## Phase 0: record real Booleans and replay them (do first)

Without this, every later phase is guessing.

- **Recorder.** `NOSHAPE_RECORD_BOOLEANS=<dir>` makes `op_boolean` write each call: input shapes (`.brep`), op,
  `BoolOpts`, and a result summary (valid, volume, counts, a hash of the history records, time, which path ran).
- **Corpus.** Record while regenerating the sample documents, `tests/perf/*.noshape`, the oracle documents, the
  kernel_robustness cases, big_plate / pattern_fins / pattern_second, and the knurls. Deduplicate by input hash. Keep
  the corpus outside the repo (`/root/core-scratch/bool-corpus`) apart from a small T1 subset of kilobyte-sized
  cases.
- **Replay tool.** A kernel test binary (`#[ignore]`, T2) that runs every case through the current code with chosen
  switches and compares against the recorded summary. It prints per-case speedup, mismatches, and the total time
  split by size bucket (<50 faces, 50–1,000, >1,000).
- **Answer two questions before Phase 2 or 3:**
  1. **Small parts.** In an everyday regen (parts under 50 faces), how much of `op_boolean` is the OCCT boolean
     and how much is overhead around it: `check_all` (BRepCheck on every input), `BRepTools_History`,
     `unify`, `make_result` index maps, copies? The notes give a 16–26 ms cut and 3–6 ms unify on a 36-face part;
     get a flame graph (`perf record`, debug info is on in release).
  2. **Thread scaling.** Run the diamond knurl at 1, 2, 4, 8 and 16 threads and record time per phase
     (`NOSHAPE_BOP_STATS` build in `/root/occt-kern`). Anything that doesn't shrink as threads increase is serial.

## Phase 1: make OCCT's serial phases parallel (patch 0004)

OCCT computes each pair intersection in parallel but processes the results one at a time. In
`BOPAlgo_PaveFiller::PerformEF` (`BOPAlgo_PaveFiller_5.cxx`), `BOPTools_Parallel::Perform` intersects all
edge-face pairs on every thread. The loop after it then walks all 815k results on one thread. That loop does real
geometric work per pair (`MakeNewVertex`, point-on-surface projections, `ReduceIntersectionRange`, which scans the
EE interferences), interleaved with writes to the shared `BOPDS_DS` (`aEFs.Appended()`, `AddInterf`,
`ChangeFaceInfo`, pave-block maps). OCCT keeps it serial because those writes go to unsynchronised shared
structures, and because `myContext` (projector caches) is not thread-safe.

**Fix pattern, applied to each serial phase:**
1. **Decide in parallel.** For each pair, compute everything it needs (new vertex positions, projections, range
   checks, on-pave tests) into a per-pair result record. Use a per-thread `IntTools_Context`, as the parallel phase
   already does through `BOPTools_Parallel`'s context functor. No writes to `myDS`.
2. **Commit serially in the original pair order.** Apply the records to `myDS`. This keeps the output identical
   and deterministic. The commit loop should only append and link; any geometry left in it is a bug.
3. **Watch for order dependence.** Some decisions read state that earlier iterations of the same loop write, for
   example `CheckFacePaves` reading face info, `ForceInterfVF` changing vertex tolerances, or a vertex created for
   one pair being reused by the next. List every read of `myDS` in the decide step. Where a read can see an earlier
   write, either keep that check in the commit step, or group pairs by the shared key (edge or face) and run each
   group sequentially in parallel with other groups.

**Order of work** (from the knurl phase times, confirmed by the Phase 0 scaling run):
1. The EF result loop (`PerformEF`): 50–60 s, the largest single serial block.
2. MakeBlocks / `PostTreatFF`'s sub-filler EE (60–100 s, 2 GB). Also do the step already listed in the kern
   handoff: section edges of copied FF pairs as located instances of the representative pair's edges, so most of
   them never get created.
3. MakePCurves (~21 s): check which part is serial; per edge-face it should be fully parallel.
4. Anything else the scaling run shows as flat.

**Target:** diamond knurl under 90 s and under 2 GB, every corpus case identical, small Booleans not slower.

## Phase 2: local Booleans, so a small change stays small

This is the 50× gap (Extrude 3: 2.7–3 s vs 55 ms) and the parked "LP2 = LP1" follow-up. OCCT's Boolean always
processes the whole target. When a tool touches only a few faces of a large part, the result should cost in
proportion to those faces.

- **Generalise** `fuse_on_face` and `cut_through_holes`, which are already special cases of this, into a single
  local Boolean:
  1. Find the target faces the tool reaches (box tree over the target's faces, cached on the shape, then an exact
     check).
  2. Take those faces plus one ring of neighbours as a local shell, and run the Boolean (OCCT) only between that
     local region and the tool.
  3. Sew the result back in place of the touched faces with `BRepTools_ReShape`. Every other face keeps its TShape,
     so it comes out as **Kept** in the history.
- **Preconditions** (otherwise decline and fall back to the full Boolean): the tool's intersection with the target
  lies inside the chosen faces, the region boundary edges are untouched, and the result's volume equals the
  target's ± the local change.
- **Free extra win:** Kept faces can keep their display triangulation. Check that `mesh_prepare` and the scene
  reuse the meshes of Kept faces instead of re-meshing the whole part. On a 1,600-face plate that is most of the
  display cost after the Boolean.
- **Target:** Extrude 3 under 150 ms; LP2 within 1.2× of LP1; `pattern_second` and `big_plate` corpus cases
  identical.

## Phase 3: the first Rust piece, a Boolean for prismatic parts (start only if Phase 0 supports it)

Most everyday mechanical parts are bounded only by planes, cylinders, cones and spheres. A Boolean restricted to
those surfaces can be exact, fast and fully parallel. It is also the first module of the future kernel, so build it
as one.

- **New crate `crates/brep`, pure Rust, with no OCCT types inside.**
  - Topology lives in index-based arenas (no reference-counted object graph).
  - Every edge has one forward orientation (this fixes the start/end-vs-parameter mismatch flagged in the kern
    handoff).
  - Decisions use exact or adaptive-precision predicates (orientation, side-of-plane, point-in-face).
  - Explicit, documented tolerances: no tolerance that grows per vertex.
- **Scope:** intersections of plane/plane, plane/quadric, and coaxial or parallel-axis cylinders. Anything else
  (B-splines, tori, skew cylinders, offsets) declines and goes to the OCCT path.
- **Boundary:** convert TopoDS to the crate's structure on entry and back on exit, including history in the
  kernel's `HistoryRecord` terms. Cache the converted form on `OcShape` (keyed by TShape) so a chain of features
  converts each body once, not once per Boolean. Conversion cost counts against the speedup.
- **Comparison:** run the replay corpus with the Rust path forced on wherever it accepts a case. Match faces by
  surface, area and centroid; then counts, volume and history relations must agree.
- **Go / no-go:** do this phase if Phase 0 shows small-part Booleans are dominated by the OCCT Boolean itself.
  If they are dominated by overhead (BRepCheck, history, unify, copies), cut the overhead first: it's cheaper and
  helps every case.
- **Write it from the geometry,** papers and the corpus, not by porting OCCT source. That keeps the future
  kernel's licence open.

## Ownership, tests, and acceptance

- **Owner:** core modelling (kernel area). Phases 1 and 2 are independent and could go to two agents (PLAN 2.3,
  slot 4: "a second core-modelling agent") if the lead wants them in parallel. Phase 0 comes first either way.
- **Tests:**
  - **T1:** about 30 small corpus cases through every path (≤ 30 s), plus a determinism test (one mid-size case at
    1 and 16 threads, identical `MapShapes` order).
  - **T2:** full corpus replay, knurls, `pattern_second`, `big_plate`.
  - **perf.yml:** knurl and Extrude 3 timings with the targets above.
- **Done for a phase** means:
  - Replay shows zero mismatches.
  - No case is more than 10% slower.
  - The phase target is met on perf.yml (macOS runner) and on this machine.
  - The handoff file is updated with the phase times before and after.

## Separate ticket found during this review (skasm, sketch)

`cargo test -p noshape-sketch --release --test all -- --ignored` fails: `fuzz::random_sketch_edits_never_panic`,
seed 2, step 55. A drag returns `Solved` with `max_residual` 3.6 (the test needs < 1e-6), so the solver reports
success while a constraint is still violated. The test was clean over 100 seeds × 150 steps when 44180351 added it,
so this is a regression. Find the commit with `git bisect` on that test. Then:

- Make the solver never report `Solved` above its tolerance. It should report `Failed` and keep the previous
  geometry instead.
- Move the fuzz test into T1, at a smaller seed count if it's too slow.
