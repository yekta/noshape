# Core modelling (agent "core")

Area: features, regeneration, the kernel (crates/kernel incl. the OCCT bridge and patches/occt, not the
STEP/Parasolid/Rhino/JT/PVZ import-export files), crates/model-core/src/regen, behaviors.rs / manipulators.rs feature
behaviour, crates/featurescript, the FeatureScript host (moving to crates/custom), the Onshape oracle tests. Worktree
/root/projects/noshape-wt/core (branch agent/core), target dir /root/target-core, scratch /root/core-scratch.
Build env: `OCCT_ROOT=/root/occt CARGO_TARGET_DIR=/root/target-core CC="ccache gcc-13" CXX="ccache g++-13"`
(/root/core-scratch/env.sh; long jobs via /root/core-scratch/job.sh <log> "<cmd>").

## Landed on main
- 81c8d9cd psb's face blend WIP (curved sides, live's message rule). Open (T2 `features_c::face_blend_on_an_elliptic_boss_as_live`):
  the elliptic boss gives 10 faces (the boss side in two halves) vs live's 9; the curvature section fails.
- 3f5f69ed P0 CP1: extruded cylinders framed as live (features.cpp `seat_circles` + `frame_cylinders_toward_sketch`).
- bf8aaee2 P0 crashes: OCCT patch 0002 (Draft parallel extrema, Extrema_ExtCS::Points, UnifySameDomain zero lines),
  crash guard (signals inside a bridge call fail that call; `run_guarded` inside ops), fillet_past_end_faces guards,
  NOSHAPE_DUMP_OPS input capture, kernel tests `crash_inputs` (fixtures in crates/kernel/tests/data/crash).
- 7a04ba4d P0 SHELL-WALLS / SHELL-FILLETED-BOX: walls across collapsing convex blends (shell.cpp `collapsing_blend`,
  `blend_sector`, `collapsed_blend_filler`, `collapsed_corner_filler`).

## OCCT patches (patches/occt)
- 0001 kern's congruent-pair reuse (TKBO). 0002 core's crash fixes (TKOffset Draft_Modification_1, TKGeomBase
  Extrema_ExtCS, TKShHealing ShapeUpgrade_UnifySameDomain). Local: patched sources in /root/core-scratch/occt-patch/b,
  built in /root/kern-scratch/occt-build (`cmake --build build --target <TK>`), the .a swapped into /root/occt/lib with
  cp + mv; backups of the replaced libs in /root/core-scratch/occt-lib-backup.
- OCCT 8's gp_Dir / gp_Ax1 constructors are noexcept: a zero vector anywhere is std::terminate (caught by the guard as
  SIGABRT inside bridge calls). Never build a gp_Dir from a vector that can be zero in our C++.

## In progress / next, in order
1. qa's other crash documents with the CLI (cli-delete-crash, FILLET-SEGV-BLEND cli-fillet2-segv): script
   /root/core-scratch/qadocs/run2.sh (running); capture their inputs into crash_inputs fixtures.
2. P0 SHELL-SLOW: slabs take 1–3.5 s per shell in the dev build (NOSHAPE_SHELL_DEBUG=1 prints each path's time);
   qa's matrix 8–17 s. Profile by_slabs (one fuse of ~20–30 slab tools).
3. P0 REGEN-SPEED (live per-feature times: behaviours "spec: core REGEN-SPEED").
4. P0 `qa_regressions::shell_that_breaks_the_part_fails` on macOS arm64 (live spec in behaviours).
5. P1 P1-16 (T2 test kernel shell.rs `shell_block_with_close_holes_like_onshape`): the chamfer-corner cones are
   rejected by `collapsing_blend` (debug "slabs: cone: inner .. centre .. convex .."), 3·π/12 mm³ short, 53 faces vs 47.
6. P1 SHELL-WALLS-CORNER (fillet corners keep more in live), std_oracle hole scope, draft_curve parting line, Extrude add
   308 ms, the elliptic boss face blend.
7. kern's open items: forward-oriented edges, revolve / Hole cylinder frames (live "fs (10-26 b)" f/g), up-to extrudes
   (op_prism_until isn't re-framed), slot arcs (live answered "core (2026-10-26)" in behaviours.md: check it).
- Don't edit crates/model-core/src/custom/ until the lead's crates/custom move lands.

## Edit → test (measured 10-27, this machine, load ~10–20)
- kernel C++ edit → `cargo test -p noshape-kernel --test all <file>::`: ~25 s (ccache; one .cpp recompiles).
- kernel C++ edit → model-core `--lib assembly::`: ~4.5 min (before the assembly split); a model integration T1 run 30–40 s.
- kernel C++ edit → CLI build: ~3 min.

## Requests to the lead
- (none)
