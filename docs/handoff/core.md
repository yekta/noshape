# Core modelling (agent "core")

Area: features, regeneration, the kernel (crates/kernel incl. the OCCT bridge and patches/occt, not the
STEP/Parasolid/Rhino/JT/PVZ import-export files), crates/model-core/src/regen, behaviors.rs / manipulators.rs feature
behaviour, crates/featurescript, the FeatureScript host (crates/custom), the Onshape oracle tests. Worktree
/root/projects/noshape-wt/core (branch agent/core), target dir /root/target-core, scratch /root/core-scratch.
Build env: `OCCT_ROOT=/root/occt CARGO_TARGET_DIR=/root/target-core CC="ccache gcc-13" CXX="ccache g++-13"`
(/root/core-scratch/env.sh; long jobs via /root/core-scratch/job.sh <log> "<cmd>").

## Landed on main (10-26 / 10-27)
- 81c8d9cd psb's face blend WIP (curved sides, live's message rule). a2b34ade: the elliptic boss as live (a sketch
  ellipse's halves become one edge, so the extruded side is one face; closed lofts start a quarter turn past the seam);
  open (T2 `features_c::face_blend_curvature_on_an_elliptic_boss_as_live`): the curvature section's fuse comes back empty.
- 3f5f69ed P0 CP1: extruded cylinders framed as live (features.cpp `seat_circles` + `frame_cylinders_toward_sketch`).
- bf8aaee2 (+ 0145f35c fixture) P0 crashes DRAFT-SEGV-DOC, SHELL-SLAB-GPF, FILLET-SEGV, FILLET-SEGV-BLEND: OCCT patch
  0002, crash guard (signals inside a bridge call fail that call; `run_guarded` inside ops), fillet_past_end_faces
  guards, NOSHAPE_DUMP_OPS input capture, kernel tests `crash_inputs` (fixtures in crates/kernel/tests/data/crash).
- 7a04ba4d P0 SHELL-WALLS / SHELL-FILLETED-BOX: walls across collapsing convex blends (shell.cpp `collapsing_blend`,
  `blend_sector`, `collapsed_blend_filler`, `collapsed_corner_filler`). 239fa250 P1-16 exactly as live (cone sectors as
  cylinder wedges, invalid slab results healed when the volume holds).
- REGEN-SPEED: 327e611a volume cache on OcShape; d85c3029 no volume per boolean (counts + box first) and
  NOSHAPE_KERNEL_PROFILE; 9281f41a parallel blend validity check; 0de5a209 auto-scope classification before distance.
  Probe: `cargo test -p noshape-ops --test all regen_profile::edit_commit_profile -- --ignored --nocapture` (median of
  5 edit commits on live's 30-feature part): 510 → ~350 ms (dev build, load ~30–40; min 312).
- 3733ef7a SHELL-SLOW: the slab construction's common in parallel (a quarter faster).
- 97081557 P0 FILLET-HANG: fillets/chamfers on a 30 s CPU budget (thread CPU timer, also stops on cancel); kernel test
  `kernel_core::a_kernel_call_over_its_budget_fails`. a117663d P0 DRAFT-HANG: drafts (30 s) and face deletions (10 s) too;
  defeaturing runs on the calling thread (a thread CPU budget doesn't count OCCT's parallel workers: keep any op that
  can hang single-threaded, or its budget never runs out).
- 502416cc REGEN-SPEED: ancestor maps and surface properties cached on the C++ shape (a Mutex inside the Rust Shape
  trips clippy's mutable_key_type wherever Shape is a hash key: keep such caches in OcShape).
- Tickets checked on main 10-27: std_oracle hole scope and draft_curve level parting line pass (Linux).
- 64f6f79a kern's P6 remainder: revolved (mid-height, z against the axis as drawn), Up to (2 um past the far end) and
  Hole (start, z along the drill) cylinder frames as live; `reframe_cylinder` (features.cpp) re-frames any cylinder face
  in place. Full revolves keep their profile where it is (turning it to the new x moved seams the screw recesses rely
  on), so a revolved cylinder's seam sits at u = pi of its new frame; extrudes have it at u = 0 (seat_circles: with u = pi
  the demo Flange's mirror common lost most of the part).

- b0e757f3 / 011aaf43 P0 BOOL-THREAD-CRASH (partly): sweeps copy their profile before the in-place cylinder re-framing
  (a data race on the caller's sketch-region edges); OCCT booleans serial by default (`bop_parallel()`, common.cpp;
  NOSHAPE_BOP_PARALLEL=1 restores). qa's seed-72 replay (`NSCLI=<cli> FZTAG=<tag> python3
  /root/qa-scratch/bin/clifuzz_trace.py 72 106`, ~12–18 min a run): parallel 2/3 complete (crash in
  BOPAlgo_PaveFiller::ForceInterfEF on a pool thread), serial 3/3. Cost: the shell test set 8.1 → 12.9 s (SHELL-SLOW).
  Open: the race itself (OCCT patch 0001's EF/VF changes are the first suspect: BOPAlgo_PaveFiller_5.cxx), and
  OSD_Parallel::For loops that still run several booleans at once (cut_slabs, fuse_slabs, fuse_trimmed).

- cb84d673 SHELL-SLOW (part): plates whose only concave faces are through holes (+ hole-rim fillet tori) shell by
  regions (shell.cpp `by_regions`: hole tube r + t, torus sectors): the kernel matrix's hole plates 0.7–1.1 s → 81–93 ms,
  same volumes. Shells run on the blends' 30 s CPU budget (`op_shell_solid`; qa's 2-min delete-crash shell fails after
  31 s). `BudgetHold` keeps the budget from jumping out of a parallel BRepCheck (pool workers would use the abandoned
  frame): wrap any OSD-parallel work reachable from a budgeted op in one. Tool: `NOSHAPE_REPLAY=<dir>/<name> cargo test
  -p noshape-kernel --test all crash_inputs::replay_one -- --ignored --nocapture` replays a NOSHAPE_DUMP_OPS capture.
  NOSHAPE_SHELL_DEBUG=1 prints why regions/offset decline ("regions: a concave edge" + where).

- 2f1fde83 oracle_p1 LP2 / CP2 as live again (T2, red in the 10-27 nightly; T1 test
  `oracle_p1::overhanging_pattern_copies_merge_with_the_part_as_live`): pattern.rs `overhang_growth` takes the part's
  exact box (the tolerance-padded one left the column's bottom 0.05 µm under the part's, no merge). kernel_robustness
  `patterns_match_onshape` M3 (T2): Up to extrudes no longer seat their circles (seated, the boss's seam fell on the
  target cylinder's seam line and the fuse lost 0.035 mm³); their cylinders are still framed as live.

- cc6c4b54 shell_that_breaks as live: `blind_hole_floor_too_thin` (shell.cpp) fails an inward shell when a blind
  hole's shoulder / flat bottom is less than 2t above the kept face under it (live's 26-case grid; its second clause,
  t <= ~1 tip-cone apex under the cavity floor, rests on 2 points: not ruled). qa_regressions test back in T1 (0.6 s).
  std_oracle's `set_std_route` is per thread now (process-wide it turned oracle_p1's extrudes into std's when both
  ran in one binary: "Select face or sketch region to extrude." under T2). SHELL-WALLS-CORNER void (exact mitre).

- ce7a161a REGEN-SPEED: a blend is checked where it changed (`valid_blend`, features.cpp: new faces fully, the whole
  shape for closure / orientation by edge use counts; BRepCheck's topology-only pass alone cost 6 ms): 7.6 → 2.6 ms a
  blend; the probe's features 278 → 224 ms.
- 9f7459f9 WIN-CRASH-GUARD: cpp/seh.cpp (MSVC only) __try/__except around each bridge call (trycatch) and run_guarded;
  std::terminate inside a guarded call is raised as structured exception 0xE0A50001 (per-thread terminate handler).
  The crash tests run on every platform again. Verifying in CI run 40481396224 (dispatched on main, 09:31).
  Also fixed there: BudgetHold / blend_budget_seconds were declared only on Linux but used everywhere since cb84d673:
  macOS and Windows don't build between cb84d673 and 9f7459f9.

- add45a95 (land.sh) Windows CI green (run 40493410580 on core/win-ci): std::terminate inside a guarded call longjmps
  back (seh.cpp; an SEH exception raised from the handler can't get out: ucrt's terminate() wraps it in __except, then
  aborts, 0xc0000409); NOSHAPE_BOP_PARALLEL goes from Rust to C++ like the memory budget (Windows' CRT getenv doesn't see
  set_var). BUDGET-LONGJMP: the budget's tick jumps only when the interrupted PC is in the executable's own text
  (`in_own_code`, common.cpp), so never out of malloc/free. classify_point keeps its loaded classifier on the shape.
  NOSHAPE_KERNEL_PROFILE=cpu: kernel profile in thread CPU time (use it on this loaded machine).

- e01fe6b3 OCCT patch 0003 (TKMesh BRepMesh_FaceChecker: skip wire pairs whose boxes miss; the display mesh's linear
  deflection is the part's diagonal, so this was quadratic in hole loops); cut_slabs checks slabs in parallel.
  594ea0d7 `cut_through_holes` (boolean.cpp): holes straight through two opposite planar faces built directly.
  203c4d6f SURF-EXTRUDE-SKETCH-EDGES.

- a01dca62 Hole add speed: line_crossings keeps OCCT's loaded line intersector on the shape. Left in std's hole probe:
  min_distance (~140 calls, ~0.8 ms each, BRepExtrema rebuilt per call).

## OCCT patches (patches/occt)
- 0001 kern's congruent-pair reuse (TKBO). 0002 core's crash fixes (TKOffset Draft_Modification_1, TKGeomBase
  Extrema_ExtCS, TKShHealing ShapeUpgrade_UnifySameDomain). Local: patched sources in /root/core-scratch/occt-patch/b,
  built in /root/kern-scratch/occt-build (`cmake --build build --target <TK>`), the .a swapped into /root/occt/lib with
  cp + mv; backups of the replaced libs in /root/core-scratch/occt-lib-backup.
- 0003 TKMesh BRepMesh_FaceChecker: wires whose boxes miss each other skip the tree walk. 0003b (10-28) TKMesh
  BRepMesh_ShapeVisitor::addWire: a wire on a plane that is already a simple loop in order keeps its order
  (ShapeAnalysis_WireOrder compares every edge's ends with every other's: 30 % of meshing the 400-fin plate's
  1604-edge front face; best 67.7 → 51.9 ms, the same triangles: kernel `planar_mesh::mesh_big_face_bench` prints a
  hash). 0003b sorts after 0003 in C order and before it in some locales: they touch different files.
- 0004 (10-28, phase 1) TKBO, on 0001: the pair intersections of PerformEE / PerformEF / PerformFF / PerformVF /
  IntersectVE / TreatVerticesEE on the thread pool with booleans serial (`noshapePairsInParallel`, ≥ 64 pairs),
  **only with NOSHAPE_BOP_PAIRS_PARALLEL=1 since 10-28 18:24** (qa BOP-PAIRS-GPF: with it on by default the app
  and the CLI died in ForceInterfEF / an IntTools_Context map on a pool worker, intermittently, as BOOL-THREAD-CRASH
  did with whole booleans parallel); ReduceIntersectionRange's E/E interferences indexed by new vertex (serial,
  stays on). So the knurl is back near its serial time unless the variable is set.
- 0005 (10-28; v2 22:30: only jobs with worker threads, a job of the calling thread alone is a serial loop the
  budget must still stop: qa DRAFT-BUDGET-POOL, T1 `kernel_core::a_budget_still_stops_a_pool_job_of_its_own_thread`,
  T2 ops `regen_profile::the_draft_pool_hang_document_opens`) TKernel OSD_ThreadPool::Launcher::perform counts, per thread, the pool jobs it runs
  (`noshape_osd_pool_depth()`); the CPU budget's handler (common.cpp on_budget_tick) doesn't jump while it isn't 0
  (the next tick after). That was BOP-PAIRS-GPF's cause: the budgeted ops (Shell, draft, blends) jumped out of a
  pool job the evaluating thread takes part in (it runs a share of the items), the workers went on with its
  abandoned frames. Likely BOOL-THREAD-CRASH's too (whole booleans parallel, same ops). T1 kernel
  `kernel_core::a_budget_never_stops_a_call_inside_an_occt_thread_pool_loop` (crash_inside_kernel(5): a 0.05 s
  budget over an OSD_Parallel loop of arithmetic, then a spin): without the check the process died (SIGSEGV) or
  the loop stopped part way every run; with it 3 of 3 runs finish the loop. (A clock read in the loop hides the
  bug: the pc sits in the vDSO, where the budget never jumps.) Built lib
  kept at /root/core-scratch/libTKBO.a.0004; the TKBO before it in /root/core-scratch/occt-lib-backup.
- OCCT 8's gp_Dir / gp_Ax1 constructors are noexcept: a zero vector anywhere is std::terminate (caught by the guard as
  SIGABRT inside bridge calls). Never build a gp_Dir from a vector that can be zero in our C++.
- Signal-handler state (crash guard, budget) needs `std::atomic_signal_fence` after it is set, or the compiler may drop
  the store before a loop.

## Live specs not yet implemented (behaviours "spec: core revolve/surface frames and edge directions",
## "spec: core Hole surface frames", "spec: core SHELL-SLOW")
- Revolved cones are framed at their apex (z = −axis, "from the apex into the cone"), tori and spheres at their centre;
  `reframe_cylinder` covers cylinders only (cones/spheres/tori need their own (u, v) maps).
- Extruded circle EDGES keep the sketch circle's frame (x +X, t = 0 at +X, ccw); our seat_circles restarts them at the
  face frame's x (−X for an extrude up) so the cylinder's seam sits at u = 0 (needed by the booleans: the Flange's
  mirror common). Matching both needs the seam moved without moving the edge's start (an edge range [pi, 3 pi]?).
- Side faces of an extrude: x = the direction of its bottom (sketch) edge; check ours.
- SHELL-SLOW: live shells qa's plates in 4–7 ms of server time; our slabs take 1–3 s (serial booleans). Needs a
  different construction for prismatic parts (by_regions extended to holes / collapsing fillets), not tuning.

## Speed targets (PLAN 2.7), current numbers
perf.yml model job (release) on main 10-27 ~18:05 (run 40556712238; before ab192d94's square-corner fillets):
| target | macOS (Apple silicon) | Linux x86_64 | goal |
|---|---|---|---|
| live-30-features full regen (`regen_profile::regen_profile`) | **50 ms** | 65 ms | ≤ 80 (live 171) |
| edit commit on it (`regen_profile::edit_commit_profile`, total) | **72 ms** | 90 ms | ≤ 100 |
| big-plate regen | 276 ms | 314 ms | ≤ 250 (live 617) |
| big-plate's Extrude 2 (400 holes; dev here, load ~50, 10-28 after the parallel profiles / prisms) | | 215 ms (was 457; profiles 136-205 → 42, prisms 120-172 → 18-39, the Boolean step ~120) | |
| Shell on qa's filleted plate with a hole (`kernel_robustness::timed_shell_on_a_filleted_plate_with_a_hole`, perf.yml timed checks from 10-28) | | dev here 140 ms (10-28, load ~50; was 177–229) | ≤ 200 |
| fin under the 400-fin plate (Extrude 3; `pattern_second::timed_the_fin_under_the_fin_plate_regenerates_locally`) | | (here, release, best of 3: 324 ms 10-28 05:37) | < 150 (was 2.7 s) |
| editing that fin (`pattern_second::timed_editing_the_fin_under_the_fin_plate`, edit commit, dev here, loaded) | | feature 217 ms, display 261 ms (was 586), whole edit 479 ms (was 802) | |
This machine (load 20–40) runs ~3× slower than the macOS runner; compare there. Landed for these today: patch 0003,
`cut_through_holes`, `fuse_on_face`, on-demand hole tables (the first hole parsed 2.1 MB of FS: 154 → 17 ms),
`fillet_square_corner`, the blend check on changed faces, shells by regions. Measuring: NOSHAPE_KERNEL_PROFILE=1
(wall), =cpu (this thread), =proc (process CPU, pool threads too); regen_profile NOSHAPE_PROFILE_ONLY=<fixture>.

## Boolean speed-up (docs/BOOLEAN-SPEEDUP.md)
Phase 0 (in progress, 10-27 evening):
- Recorder: NOSHAPE_RECORD_BOOLEANS=<dir> (kernel `bool_corpus`, around `boolean()`): `<hash>.brep` + `<hash>.txt`
  (op, options, valid / volume / counts / history hash, ms, path). Corpus /root/core-scratch/bool-corpus: 974 cases
  from the model oracles / features / kernel_robustness / pattern_second / big_plate / qa_regressions / custom
  features, every perf fixture (regen_profile) and the kernel tests: 933 OCCT, 28 on_face, 8 through_holes, 3
  trimmed, 2 fuse_slabs; 948 with < 50 input faces, 17 with 50–1000, 9 over 1000.
- Replay: `NOSHAPE_BOOL_CORPUS=<dir> cargo test -p noshape-kernel --test all bool_replay::bool_corpus_replay -- --ignored
  --nocapture` (NOSHAPE_BOOL_MAX_FACES=n for small ones, NOSHAPE_BOOL_REBASE=1 to take a run as the expected one).
  Replays compare with replays: 3 cases came out otherwise in a regeneration than replayed alone (rebased), and one
  (24e39113, a fuse) changes its history between full replay runs: OCCT itself isn't deterministic there (listed in
  `<corpus>/unstable`, reported, not failed). T1: `bool_replay::bool_corpus_small_cases` (26 cases in
  crates/kernel/tests/data/bool, 0.6 s) and `bool_replay::parallel_booleans_give_the_same_result` (serial vs parallel).
- Answer 1, small parts: of op_boolean's CPU on the 948 cases under 50 faces, OCCT's build is 98.8 % (51.7 of 52.3 s);
  make_result 0.2 %, history 0.1 %, inputs and unify < 0.1 %. Median 10.4 ms a Boolean (dev build, load ~30),
  p90 49 ms; 9 cases over 1 s (helix / B-spline tools) are 51 of the 74 s. So it's the OCCT Boolean itself (Phase 3
  has its go), not overhead around it.
- The 1600-face case (a fin fused to the 400-fin plate, the Extrude 3 of phase 2): op_boolean 1.93 s, OCCT's build
  1.79 s; gdb samples land in whole-part passes (BOPAlgo_Builder::FillInternalShapes' MapShapesAndAncestors,
  BuilderSolid::PerformAreas boxing every face, PostTreat's CorrectWires, the pave filler's teardown): O(faces) at
  ~1 ms a face, however small the tool. The fin is flush with the plate's front and back faces, so `fuse_on_face`
  (which wants the footprint clear of the face's edges) declines it: phase 2 has to merge coplanar faces.
- Answer 2, knurl thread scaling (diamond, NOSHAPE_BOP_PARALLEL=1, NOSHAPE_OCCT_THREADS=n, /root/occt-kern's [bop]
  stats, release, load ~30; /root/core-scratch/knurl.log): 1 thread 451 s, 2: 382 s, 4: 259 s, 8: 239 s, 16: 276 s.
  Seconds per phase (summed over the run's Booleans) at 16 / 8 / 4 / 2 threads: PerformEF 57 / 40 / 80 / 125,
  PerformEE 43 / 37 / 35 / 70, PerformFF 31 / 14 / 13 / 20, MakeBlocks 22 / 11 / 12 / 17, after the last stat
  (BuildBOP etc.) 26 / 34 / 15 / 25, RepeatIntersection 13 / 8 / 20 / 25, VF 8 / 8 / 27 / 21. From 4 threads up
  nothing shrinks: EF, EE, FF, MakeBlocks and the build are serial-bound, phase 1's targets (EF first, as planned).
- Corpus history hash: each output by its place (rounded centroid / size), not its index, so a fast path that
  orders the result's faces otherwise compares equal when the relations agree (corpus rebased with LOCAL=0).

Phase 2 (local Booleans), first step landed:
- boolean.cpp `local_boolean` (NOSHAPE_BOOL_LOCAL=0: off): one tool, a solid target of ≥ 200 faces (and ≥ 8× the
  tool's): the target's faces the tool's box reaches plus one ring, OCCT's general fuse with the tool, pieces kept by
  place (a target piece with a vertex out of the tool's box at once, else a point against the tool; tool pieces by
  ray parity against the target; shared pieces by their normals), the shell rebuilt from the untouched faces and
  the pieces; history from the general fuse's, kept to the result, both input solids Deleted (OCCT's). Declines when
  the region's boundary edges get split.
- Corpus: 0 mismatches over 974 cases; it takes the two fin fuses: 412 faces 117 ms (OCCT ~700 ms dev), 1612
  faces 370–413 ms (OCCT 1.57 s dev). T1: kernel `boolean_groups::a_small_fuse_on_a_big_part_is_done_locally`.
- Extrude 3 of fins-400 (dev, whole feature): 1681 → 1336 ms (local Boolean) → ~1000 ms (merge_planar, below).
  Breakdown (pattern_second ×400, NOSHAPE_PERF): kernel ~580 ms (local Boolean, mostly OCCT's general fuse of the
  two 800-edge faces the fins merge into), naming + unify ~230 ms, split ~50 ms; then the display re-meshes the whole
  part (~1 s, every regeneration).
- Step 3 (10-28): big planar faces the tool only touches (≥ 64 edges, the tool's box on their inner side, no tool
  face on their plane over an area) stay out of the general fuse and out of the ring: the fin fused under the
  400-fin plate went through the plate's two 800-edge front / back faces and, through them, every fin's sides
  (1604 of 1612 faces); their edges the fuse splits are put back in place (`patched`, IsSplitToReverse +
  pcurves on the plane). Tool pieces against the target and kernel `classify_point` on solids of ≥ 200 faces go
  by `RayClassifier` (geom.cpp, NOSHAPE_CLASSIFY_RAYS=0: OCCT's classifier): ray parity over the faces whose
  box the ray passes, OCCT's classifier when no ray is clean. Corpus case a2121c5b (1612 faces): 417 → 163 ms
  (general fuse 180 → 5 ms; OCCT 1108 ms recorded). Extrude 3's kernel CPU (NOSHAPE_KERNEL_PROFILE=cpu, now
  printed per feature): 535 → 395 ms; classify_point (the auto scope, 2 calls) 133 → 22 ms. Left: op_unify 106
  (merge_planar 42, candidates 33), the local Boolean 65 (maps 26, history 19), ancestor maps 85 (8 calls),
  bounding boxes 20 (6 calls), on_face's decline 14. T1: kernel `boolean_groups::points_against_a_big_part_classified_by_rays`.
- Step 4 (10-28): op_boolean sees its inputs' OcShapes (`OcShapeList::sources`, valid during the call): the local
  Boolean and fuse_on_face take the target's cached face map, edge → face map and face boxes (`OcShape::face_boxes`,
  shared with classify_point's rays) instead of rebuilding them; fuse_on_face finds each face's plane once.
  Ancestor maps are built by sorting (sub-shape, ancestor) pairs instead of TopExp::MapShapesAndAncestors (same
  rows). local.maps 26 → 5 ms, on_face's decline 14 → 2 ms; corpus a2121c5b 161 → 110 ms; 0 mismatches.
  `NOSHAPE_KERNEL_PROFILE` shows ancestors.<kind> per map: naming's vertex → face maps are the dear ones (3 a
  feature, ~16 ms each on the 1612-face part).
- Step 5 (10-28): the regen's unify after a local Boolean looks only where the Boolean changed the part.
  `OcShape::myUnifyClean`: a unify found nothing to merge in the shape (op_unify then returns at once, path
  "known"); a local Boolean on such a target attaches a `UnifyScope` to its result (internal.hpp: the new faces,
  the faces across the kept pieces' edges, the faces at the vertices whose edges changed) and op_unify's
  candidate scans and merge_planar run over that compound only (path "scoped"; `last_unify_path()`).
  NOSHAPE_UNIFY_LOCAL=0: the whole shape; NOSHAPE_UNIFY_CHECK=1: every scoped unify is redone whole and compared
  (aborts on a difference); `unify_same_domain_whole` for tests. Extrude 3 of fins-400 (CPU, loaded machine):
  op_unify 187 → 77 ms (merge_planar 76 → 27, candidates_again 56 → 10; the rest is make_result's history over
  the whole part); the feature 670 → 408 ms wall. T1: kernel
  `boolean_groups::the_unify_after_a_local_boolean_matches_the_whole_one` (six fuses and cuts, four of which merge).
  With it, naming: a unify's input gets face names only (its history takes faces to faces; the edge / vertex
  names cost two ancestor maps of the part) and `Body::named` takes the edge / vertex names `names_for` already
  made (the part's edges and vertices were named twice). Extrude 3: ancestor maps 8 → 4 a feature; the feature
  306 ms wall (dev; 1681 at the start of phase 2). Checked: kernel, model-core, model and ops tests and the corpus
  with NOSHAPE_UNIFY_CHECK=1, no difference. Left in Extrude 3 (CPU): op_boolean 100 (local 59), op_unify 72
  (merge_planar 30, make_result's history ~30), classify_point 32 (first: the face boxes), bounding boxes 27
  (6 calls), ancestor maps 28.
- Step 6 (10-28): an op's result keeps the maps its history needed (HistoryBuilder hands its OcShape on instead of
  wrapping the shape again), the history reads the inputs' maps from their OcShapes (op_boolean, op_unify), results
  of the local Boolean and the unify take the face boxes of the faces they kept from their input, and
  `bounding_box` is made once per OcShape. Extrude 3 (CPU): bounding boxes 27 → 14 ms, ancestor maps 28 → 21 ms;
  corpus a2121c5b 110 → 93 ms; 0 mismatches.
- Step 7 (10-28): RayClassifier tries the ray that passes the fewest faces' boxes first (slanted and near-axis
  directions; boxes tested against the half-line): the fin's pieces against the plate 16 → 3 ms. The local
  Boolean hands the result's face / edge / vertex maps to make_result (they were made twice). Timers inside
  merge_planar (merge.maps / groups / rebuild / edges); NOSHAPE_BOOL_DEBUG prints the unify scope's size (Extrude 3:
  11 faces, 16 seeds). Extrude 3 (dev, CPU): op_boolean ~58, op_unify ~65-76 (merge.groups 15-19: the front face's
  800-edge group rebuilt; merge.edges 12-15; make_result's history ~20), classify_point 33 (the target's face
  boxes, made the first time), ancestor maps 16, bounding boxes 12-15. Corpus a2121c5b 74 ms; 0 mismatches.
  T2 timed (perf.yml model job's pattern_second step): `timed_the_fin_under_the_fin_plate_regenerates_locally`.
- Step 8 (10-28): whole-part walks merged. `OcShape::map` makes the face, edge and vertex maps in one walk
  (`map_fev`: MapShapes' order, a sub-shape met again not walked again); edge → face and vertex → face are made
  together (the naming asks for both); `unique_ancestors` replaces TopExp::MapShapesAndUniqueAncestors in
  boolean.cpp (same entries, lists from a bump allocator; small gain: the walk is the cost). perf on the bench
  below: TopoDS_Iterator::updateCurrentShape was 14 % of the time (OCCT has no frame pointers: no callers).
  Bench `boolean_groups::local_fuse_bench` (ignored; NOSHAPE_BENCH_CASE=/root/core-scratch/bool-one/a2121c5b456e7188:
  the fin fuse, the scoped unify and the naming's maps, 20 rounds): median 142 → 105 ms. Per round (CPU): the
  local Boolean 30 (history 8, closed_and_oriented 6.5, general fuse 6), merge_planar 25 (the two 800-edge
  groups 13, the edge phase's maps 5.6), candidates_again 8; the whole-shape unify would add ~50.
- Step 9 (10-28): planar faces bounded by straight edges get their box from their corners (`face_box`; any other
  face BRepBndLib's), and the rays meet a planar face's plane directly and classify the hit with
  BRepClass_FaceClassifier (OCCT's IntCurvesFace_Intersector approximated the 800-edge front face's whole boundary
  first, ~10 ms): classify_point's rays 16.8 → 6.5 ms. boolean_into logs (NOSHAPE_PERF) naming / unify / naming
  the unified separately: on Extrude 3 the naming is 6 + 5 ms; the unify call 70 ms wall (merge_planar's two
  800-edge groups ~20, candidates_again ~10, its make_result ~12: the merged part mapped again and ~10k kept
  records), the split 32 (edge / vertex names of 5000 edges, 3300 vertices). Corpus a2121c5b 49 ms; 0 mismatches.
  Timed test (dev, loaded machine) 236–303 ms. Left for < 150: the two big groups' rebuild, the history records of
  every kept sub-shape (op_boolean and op_unify each), the naming of every edge and vertex, the auto scope's first
  classification (the target's face boxes and rays, ~20 ms; the dialog's `touched`, ops).
- Step 10 (10-28): display meshes of the faces an edit left alone are reused. mesh.cpp `MeshSet`: the display's
  triangulations (with normals) and edge polylines by face / edge (TShape + location) and deflections (a set serves
  absolute deflections within 1 %: the display's scale with the part's diagonal). mesh_prepare meshes only the
  faces it doesn't have, alone, when every edge they share with a cached face is straight (else the whole shape),
  and drops the triangulations of only those. BRepMesh re-meshed all 1612 faces after each edit, and took as long
  again when they all already had a mesh (~340 ms either way), so the cache keeps them out of BRepMesh entirely.
  NOSHAPE_MESH_CACHE=0: off; NOSHAPE_MESH_DEBUG=1 prints how many faces were meshed. `shown_topology` is made
  once per shape from its cached ancestor maps (2 calls a regeneration, each mapping the part's ancestors 3 times:
  137 → 15 ms). Editing the fin (depth 5 → 6, 7, 8 mm; dev, loaded machine, best of 3), before → after: the
  feature 216 → 217 ms (kernel ~200 CPU), mesh_prepare 406 → 193 ms (134 of it meshing the 7 new faces, two of
  them the patched 800-edge front / back faces), the display 586 → 261 ms, the whole edit 802 → 479 ms.
  T1: kernel `boolean_groups::display_meshes_of_kept_faces_are_reused` (a fuse and a hole: the same triangles,
  normals and edge polylines as meshing a fresh copy whole; 97 → 43 and 91 → 15 ms). T2 timed:
  `pattern_second::timed_editing_the_fin_under_the_fin_plate`.
- Step 11 (10-28): psui's planar CDT fix (their 10-28 note below, /root/psui-scratch/planar-cdt-collinear-fix.patch):
  planar_cdt.hpp `recover()` refused a boundary segment when a vertex lay in line *behind* its start; the fin under
  the plate puts edges in line across the 1608-edge front / back faces, so both went to BRepMesh's algorithm every
  time. T1 kernel `planar_mesh::a_vertex_in_line_behind_a_boundary_segment_keeps_the_planar_triangulation` (theirs),
  on `Tessellation::planar_fallbacks` (new: planar faces given to BRepMesh's algorithm; NOSHAPE_MESH_DEBUG prints
  it). The edit (dev, loaded): display 230–265 ms, whole 447–640 ms. Left in the display: BRepMesh's model build of
  the two 1608-edge faces (~150 ms for the 7 new faces; ShapeAnalysis_WireOrder, CurveOnPlane per call: psui), the
  maps (31 ms), the clean (19). psui's larger patch (mesh-reuse-for-core.patch: an edge discretizer reusing the
  kept faces' polygons, display deflection quantised) is not taken: the cache here keeps the same output with the
  1 % rule; the quantising would change every display mesh.
- The regen's unify (boolean.cpp `merge_planar`): the merged face goes on the surface of the group's face with the
  most edges and only edges without a pcurve there get one; a one-shell solid is rebuilt shell-only instead of
  BRepTools_ReShape copying the part: fins-400's 4 unifies 849 → 382 ms. A unify of only the new faces' region
  (UnifySameDomain on a compound, boundary kept) was tried and dropped: 518 ms vs 183 on Extrude 3 (the region holds
  the 800-edge faces). Older notes below: What's left there is around the Boolean: the
  regen's unify_same_domain over the whole 1600-face result after every Boolean (~390 ms for 4 calls), the auto
  scope's classify_point loading a classifier per new body (~140 ms / 4), ancestor maps (4816 calls, 365 ms),
  shown_topology (223 ms), and inside the local Boolean the general fuse of the two 800-edge front / back faces
  (the fins merge into them; ~220–350 ms). Next: unify only the region, no reload of classifiers, then the target.

Phase 2 stopped here (user, 10-28 12:55: big gains, not the last milliseconds). Where it stands, fin under the
400-fin plate (Extrude 3):
- Full regeneration, the feature: 2.7 s (OCCT whole) → ~240 ms dev / 324 ms release under load (best of 3).
  Kernel CPU ~200: the local Boolean ~45 (history 15, closed_and_oriented 10, general fuse 6), its make_result ~6;
  op_unify ~70 (merge_planar's two 1604-edge front / back groups ~20, candidates_again ~10, make_result ~12);
  the auto scope's first classify_point ~20 (the target's face boxes and rays; the dialog's `touched`); ancestor maps
  ~15; bounding boxes ~15. Rust: naming 6 + 5, the split's edge / vertex names ~30.
- Edit commit (depth 5 → 6, 7, 8 mm; dev, loaded): the feature ~217 ms, the display 586 → ~230-260 ms, the whole
  edit 802 → ~450-480 ms. Display left: BRepMesh on the 7 new faces ~150 ms (the two 1604-edge faces' model build;
  0003b took 30 % of it), mesh maps 31, clean 19, shown_topology 15.
- Long tail left (each 5-20 ms): rebuilding the 800-edge groups in merge_planar, the history records of every kept
  sub-shape (op_boolean and op_unify), naming every edge and vertex, the auto scope's first classification, the
  BRepMesh model build of big faces. The bool agent's Rust Boolean (phase 3, NOSHAPE_BOOL_RUST, its hook early in
  op_boolean) may replace the local Boolean for planar parts.

Phase 1 (10-28, in progress; patch 0004 drafted in /root/core-scratch/0004-bop-parallel-pairs.patch, applied on
0001): the diamond knurl (kernel `perf::knurl_like_onshape`, KNURL=diamond, release, production settings: OCCT
booleans serial, BOOL-THREAD-CRASH) runs against /root/occt-core (a copy of /root/occt with the TKBO under test;
/root/target-core-bop; NOSHAPE_EF_STATS=1 prints [ef] / [bop] step times from temporary timers that are not in the
patch). Before: 564 s (load ~30), PerformEF 181 s, of it the "parallel" pair intersections 130 s on one thread and
the serial result loop only 22 s. So the big structural win is the pairs, not the result loop:
- The pair intersections of PerformEE / PerformEF / PerformFF / PerformVF / IntersectVE run on the thread pool even
  with booleans serial (`noshapePairsInParallel`, ≥ 64 pairs; NOSHAPE_BOP_PAIRS_SERIAL=1: never). Each pair into its
  own record with its own context, applied afterwards in order: the same result. ForceInterfEF's / ForceInterfEE's
  and the builder's loops stay serial (the race's sites).
- ReduceIntersectionRange looks up the E/E interferences by new vertex (it scanned all ~10^5 for each vertex
  common part): 6.3 → 0.4 s.
- TreatVerticesEE (patch 0001 had made it one representative per congruent class, still serial) on the pool too:
  75 → 25 s; PerformVF 42 → 12.5, RepeatIntersection 34 → 13.
- Knurl 564 → 264-329 s (instrumented 264-281, the clean lib 329; load 26-36; same 8142 faces, volume 75409.760). Steps left (s, summed over the run's Booleans):
  MakeBlocks 75 (PostTreatFF's sub-filler 37, UpdateFaceInfo 20, the loop 12, PutSEInOtherFaces 5), PerformEF 44
  (pair collection 15 with 0001's congruence keys, pairs 14, result loop 13 of which projections 10), PerformEE 30,
  TreatVerticesEE 25, PerformFF 17, RepeatIntersection 13, PerformVF 12.5, PerformVE 6. Next if worth it:
  PostTreatFF's sub-filler, the pair collections (the keys could be made in parallel), the EF result loop's
  projections (decide in parallel, commit in order).

Big plate (10-28): Extrude 2's 400 sketch regions become faces in parallel (`EvalCtx::profiles`, ≥ 16 regions,
each region's face gets its own edges) and its 400 prisms too (extrude, ≥ 16 profiles all from sketch regions),
kept in order (the first error in order still wins). NOSHAPE_PERF logs extrude's profiles / tools / operation.
Phase 1 stopped at patch 0004 (the remaining steps are 10-40 s each, listed above); CI run 40718671419 built OCCT
with 0003b and 0004 on Linux, macOS and Windows.

## Next, in order
0. **Now: phase 1 (knurl 210 s → < 90 s), EF result loop first (patch 0004); take the big structural win, stop
   when returns flatten (user 10-28).** Phase 2 stopped (above). Phase 3 is the bool agent's (crates/brep, the
   NOSHAPE_BOOL_RUST hook in op_boolean: keep it working when changing op_boolean). Earlier wording:
   **P0 Boolean speed-up, docs/BOOLEAN-SPEEDUP.md (lead, 10-27 19:40; the user's plan, binding).** Phase 0 first
   (recorder, corpus, replay, the small-part flame graph and the knurl thread-scaling run): its small-part answer
   also decides how REGEN-SPEED's per-boolean ~15 ms gets cut, so item 1 continues through it. Then, one agent
   doing both: Phase 2 (local Booleans: Extrude 3 under the 400-fin plate 2.7 s → < 150 ms, the everyday case)
   before Phase 1 (OCCT patch 0004, parallel serial phases: knurl 210 s → < 90 s); Phase 3 only if Phase 0
   supports it. Each phase: same results and history, deterministic at 1/4/16 threads, OCCT fallback with a
   NOSHAPE_BOOL_<NAME>=0 switch, numbers before/after in this file.
1. P0 REGEN-SPEED (open): per regeneration ~15 ms per boolean (OCCT build), fillet/chamfer ~10 ms + ~6 ms check, the
   display meshing ~45 ms. Ideas: check only the blend's new/modified faces instead of the whole part; fewer
   unify passes; profile with NOSHAPE_KERNEL_PROFILE=1 on the probe. Live: 2–9 ms per feature.
2. P0 SHELL-SLOW (open): parts with concave edges still go to slabs (fuse of 20–45 overlapping tools 0.4–5 s; qa's
   shell_that_breaks part now fails at once by live's rule); NOSHAPE_SHELL_DEBUG=1 times each stage. The delete-crash shell input (now fails on the
   budget) is in /root/core-scratch/slow-shells. Next: regions for concave edges between planes (pockets, steps).
3. P1 std_oracle hole scope,
   Extrude add (Hole add: std holeEditLogic is OCCT-bound in its probe), the elliptic boss curvature blend,
   FILLET-HANG's fillet itself. T2 reds of the 10-27 nightly in my area to check: kernel_robustness
   shell_draft_sweep_matches_onshape, sheetmetal_frames (3).
4. kern's open items: forward-oriented edges (proposal: hand out edges FORWARD so start/end, parameters and tangents
   agree; touches every caller of edge_info). Cylinder frames: done (64f6f79a).

## End-of-day walk (10-27 03:45, green 8165fca6)
- Opened qa's qa-1026-ps2-shell.noshape in the green app on :101: Part Studio 1 (live's 30 features) regenerates, Part
  Studio 2 (SHELL-WALLS + chamfer + holes) shells to an open box with full walls and tubes round the holes, Shell 1 OK,
  as live describes. The three crash documents and the hang document open with the CLI built from main. No regressions
  found in my area.

## Edit → test (measured 10-27, this machine, load ~10–40)
- kernel C++ .cpp edit → `cargo test -p noshape-kernel --test all <file>::`: ~25 s (ccache); a header edit ~3–10 min.
- core edit → `cargo test -p noshape-model --test all`: ~1–2 min at load 30; ops test binary rebuild ~2–6 min.

## From psui (10-28 10:45): display re-mesh findings for your mesh cache

Before the lead's 10:35 correction I built a kernel-side face cache too. It is **not landed**; take it or drop it:
/root/psui-scratch/mesh-reuse-for-core.patch (`git apply` on e60fdf6f; mesh.cpp, ffi, mesh.rs, display.rs, tests).
What it found on pattern_second's fin under the 400-fin plate (release, load 20–40):
- **A planar CDT bug, independent of any cache (small):** planar_cdt.hpp `recover()` refuses a constraint when a
  vertex in a's fan is collinear with a-b even when it lies *behind* a. The fin under fin 19 puts edges in line
  across the plate's front / back faces (1608 edges), so PlanarCdtAlgo refuses both on every regeneration and
  BRepMesh's own algorithm takes 200–400 ms (BRepMesh's face phase 32 → 414 ms). Fix: refuse only a collinear
  vertex ahead of a (dot > 0): /root/psui-scratch/planar-cdt-collinear-fix.patch, with a T1 that fails without it
  (planar_mesh `a_vertex_in_line_behind_a_boundary_segment_keeps_the_planar_triangulation`; it reads a
  `planar_fallbacks` count that the big patch adds to MeshCounts).
- Keys miss without this: the display's linear deflection is diag × quality and the fin grows the bounding box, so
  every key changes. The patch quantizes diag in display.rs `display_deflection` (eighths of an octave, up).
- Kept faces must stay *out* of BRepMesh: with every face restored it still took 86 of ~100 ms (model build, edges,
  healer). The patch meshes a compound of the new faces only and gives each edge they share with a kept face that
  face's polygon (a custom edge discretizer doing what BRepMesh's extractor does); an edge that isn't
  same-parameter / same-range, or is a seam, brings its kept face along instead. A face is reused when its edges,
  and per edge the faces' surface / location / orientation / stored pcurves in BRepMesh's order
  (ReverseSortFaces), are unchanged; the faces themselves may be new (the rebuilt front face).
- Exactness: kernel test mesh_reuse (120 and 400 fins, a fin fused under with unify) compares the result with a
  full re-mesh bit for bit: identical, 1867 of 1878 faces kept.
- Numbers (tessellating the part after the fin): main 430–600 ms; CDT fix alone 330–630 (load); fix + cache
  326–357 ms, 1202 of 1610 faces kept (the 400 located fin faces are never cached). Left: BRepMesh's model build
  of the two 1608-edge faces (~200 ms: ShapeAnalysis_WireOrder, BRep_Tool::CurveOnPlane per call), and ~150–190 ms
  of display work outside meshing (mine to measure once your cache lands).
- Landed with this note: model `pattern_second::timed_the_fin_plate_redisplays_after_the_fin` (perf.yml model job)
  prints the display's share of the regeneration.

## From bool (10-28 16:30): noshape's own Boolean in op_boolean
- boolean.cpp: one block (`rust_boolean_on()`) after through_holes, before cut_slabs; **on by default from 10-28
  evening** (NOSHAPE_BOOL_RUST=0: off; docs/handoff/bool.md). Its conversion and result building are in rust_boolean.cpp.
- noshape_occt.hpp: OcShape keeps `myBrep` / `myBrepTol` / `myBrepWhy` (+ `myBrepLock`): the converted body of the
  shape (or why it has none), made once; a result of the Rust Boolean gets its body attached
  (`rust_boolean_keep`), so a chain of Booleans converts a body once. Please keep these when you touch OcShape.
- shell.cpp: the six plain BRepAlgoAPI Fuse / Cut / Common blocks (slabs' fuse and cut, the two draft ones' cut and
  fuse) call `quick_boolean` (rust_boolean.cpp): noshape's Boolean where it takes the inputs, else the same OCCT
  call; recorded with NOSHAPE_RECORD_BOOLEANS (path `direct` / `direct_rust`). features.cpp: Thicken's cut and
  Shell's offset cut too (10-28 20:00). The fillet / chamfer fallbacks' cuts (balls, wedges, chamfer planes) are
  left as they are: their tools are cylinders at angles the Rust Boolean declines anyway.
- Corpus: I added 5c40b5731295e50a to /root/core-scratch/bool-corpus/unstable (with the Rust Boolean one face's
  centroid lands on the other side of a 0.5 µm rounding boundary; all else equal).
- Found while testing (OCCT, not ours, affects `RayClassifier`): BRepClass_FaceClassifier's answer for a point
  0.04 mm from an 800-edge comb face's boundary depended on which edge the face's wire starts with (the fin plate's
  y=20 face after a unify; `classify_point` gave Outside for a point inside). Results of the Rust Boolean now start
  their loops where OCCT's do, which avoids that case, but the classifier may misjudge near-boundary hits elsewhere:
  a second ray when a hit is within ~1e-3 of the face's size from its boundary would make it safe.

## Requests to the lead
- Never swap an OCCT lib into the shared /root/occt before its patch lands (I did 10-28 20:50-22:05 with a "pairs
  on" TKBO, and qa's 21:05 build picked it up): /root/occt-core + /root/target-core-bop is the place to try them.
- /tmp/lead/disk-loop.sh's `prune-exes.py` (every ~30 min) keeps the newest 2 builds of each noshape crate per
  target dir and deletes the rest with their fingerprint dirs, also while a cargo uses the dir: land.sh builds
  each crate in more than two ways (clippy, tests, T0), so the build a running land.sh still needs goes. My lands
  failed at 00:27, 00:59, 03:36 and 04:06 on 10-28, each a minute after a prune ("removed 47 old builds" at 03:35,
  45 at 04:06) on a missing fingerprint / rlib. Please skip target dirs a cargo is running in (the loop's `busy()`)
  or keep 4 builds; until then I start lands right after a prune.
- Corpus case 24e39113fe46e702: OCCT's own result differs from run to run (listed `unstable` in the corpus).
- Main RED 484a185c (09:31) and cb5e9cec (07:55), "ops --test all SIGSEGV": not the kernel. The kernel log has
  `hidden_edges::e[1635930] general protection fault … in libvulkan.so.1.4.341` at the watcher's T1 (09:26), and a
  "pipeline compil" thread faulting in libvulkan 10 min earlier: ops' GPU test `hidden_edges::edges_behind_fins_stay_hidden`
  (renderer, not mine) crashing in the Vulkan loader/driver, intermittently (1 in 3 runs under gdb here). The DOC-POISON
  session test failing in the same run looks like a consequence (a dead test process's crash markers); dx's.
- macOS / Windows don't compile on main between cb84d673 and 9f7459f9 (fixed in 9f7459f9); Windows T1 green from
  add45a95 on (crash guard + boolean_budget). Don't release from before add45a95.
- Forward-oriented edges (kern's proposal, P0 class: EdgeInfo.start/end follow a reversed edge but its parameters,
  tangents and circle frame don't, and many callers pair them): handing every edge out FORWARD (common.cpp
  `sub_shape`/`sub_shapes`, patch in /root/core-scratch/forward-edges.patch) passes every kernel, model, model-core,
  assembly, custom and oracle test unchanged, and ops' 119 others, but breaks two drawing flows: ops e2e
  `drawing_callouts_ordinates_and_datums` (an ordinate "0 39.8 79.8" where 40 is expected) and
  `drawing_section_detail_views_and_annotations` (drawing_virtual_sharp "Select a straight edge"). The drawing code
  (dx) relies on the stored orientation. Not landed: needs dx to adapt those two (or route it to me with leave to edit
  crates/drawing).
