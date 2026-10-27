# Core modelling (agent "core")

Area: features, regeneration, the kernel (crates/kernel incl. the OCCT bridge and patches/occt, not the
STEP/Parasolid/Rhino/JT/PVZ import-export files), crates/model-core/src/regen, behaviors.rs / manipulators.rs feature
behaviour, crates/featurescript, the FeatureScript host (crates/custom), the Onshape oracle tests. Worktree
/root/projects/noshape-wt/core (branch agent/core), target dir /root/target-core, scratch /root/core-scratch.
Build env: `OCCT_ROOT=/root/occt CARGO_TARGET_DIR=/root/target-core CC="ccache gcc-13" CXX="ccache g++-13"`
(/root/core-scratch/env.sh; long jobs via /root/core-scratch/job.sh <log> "<cmd>").

## Landed on main (10-26 / 10-27)
- 81c8d9cd psb's face blend WIP (curved sides, live's message rule). Open (T2 `features_c::face_blend_on_an_elliptic_boss_as_live`):
  the elliptic boss gives 10 faces (the boss side in two halves) vs live's 9; the curvature section fails.
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

## OCCT patches (patches/occt)
- 0001 kern's congruent-pair reuse (TKBO). 0002 core's crash fixes (TKOffset Draft_Modification_1, TKGeomBase
  Extrema_ExtCS, TKShHealing ShapeUpgrade_UnifySameDomain). Local: patched sources in /root/core-scratch/occt-patch/b,
  built in /root/kern-scratch/occt-build (`cmake --build build --target <TK>`), the .a swapped into /root/occt/lib with
  cp + mv; backups of the replaced libs in /root/core-scratch/occt-lib-backup.
- OCCT 8's gp_Dir / gp_Ax1 constructors are noexcept: a zero vector anywhere is std::terminate (caught by the guard as
  SIGABRT inside bridge calls). Never build a gp_Dir from a vector that can be zero in our C++.
- Signal-handler state (crash guard, budget) needs `std::atomic_signal_fence` after it is set, or the compiler may drop
  the store before a loop.

## Next, in order
1. P0 REGEN-SPEED (open): per regeneration ~15 ms per boolean (OCCT build), fillet/chamfer ~10 ms + ~6 ms check, the
   display meshing ~45 ms. Ideas: check only the blend's new/modified faces instead of the whole part; fewer
   unify passes; profile with NOSHAPE_KERNEL_PROFILE=1 on the probe. Live: 2–9 ms per feature.
2. P0 SHELL-SLOW (open): slab fuse of 20–45 overlapping tools 0.4–5 s (qa's shell_that_breaks part 9 s);
   NOSHAPE_SHELL_DEBUG=1 times each stage. The 43 s delete-crash shell input is in /root/core-scratch/slow-shells.
3. P0 `qa_regressions::shell_that_breaks_the_part_fails` (T2): live fails too; ours fails only by an invalid OCCT
   result (platform-dependent). Needs Parasolid's rule (TICKETS note).
4. P1 SHELL-WALLS-CORNER (fillets meeting at a sharp corner keep ~0.47·R mm³ more in live), std_oracle hole scope,
   draft_curve parting line, Extrude add 308 ms, the elliptic boss face blend, FILLET-HANG's fillet itself.
5. kern's open items: forward-oriented edges (proposal: hand out edges FORWARD so start/end, parameters and tangents
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
- Forward-oriented edges (kern's proposal, P0 class: EdgeInfo.start/end follow a reversed edge but its parameters,
  tangents and circle frame don't, and many callers pair them): handing every edge out FORWARD (common.cpp
  `sub_shape`/`sub_shapes`, patch in /root/core-scratch/forward-edges.patch) passes every kernel, model, model-core,
  assembly, custom and oracle test unchanged, and ops' 119 others, but breaks two drawing flows: ops e2e
  `drawing_callouts_ordinates_and_datums` (an ordinate "0 39.8 79.8" where 40 is expected) and
  `drawing_section_detail_views_and_annotations` (drawing_virtual_sharp "Select a straight edge"). The drawing code
  (dx) relies on the stored orientation. Not landed: needs dx to adapt those two (or route it to me with leave to edit
  crates/drawing).
