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

- a01dca62 Hole add speed: line_crossings keeps OCCT's loaded line intersector on the shape. Left in std's hole probe:
  min_distance (~140 calls, ~0.8 ms each, BRepExtrema rebuilt per call).

## OCCT patches (patches/occt)
- 0001 kern's congruent-pair reuse (TKBO). 0002 core's crash fixes (TKOffset Draft_Modification_1, TKGeomBase
  Extrema_ExtCS, TKShHealing ShapeUpgrade_UnifySameDomain). Local: patched sources in /root/core-scratch/occt-patch/b,
  built in /root/kern-scratch/occt-build (`cmake --build build --target <TK>`), the .a swapped into /root/occt/lib with
  cp + mv; backups of the replaced libs in /root/core-scratch/occt-lib-backup.
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

## Next, in order
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

## Requests to the lead
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
