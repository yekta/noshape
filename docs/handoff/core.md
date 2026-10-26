# Core modelling (agent "core")

Area: features, regeneration, the kernel (crates/kernel incl. the OCCT bridge and patches/occt, not the
STEP/Parasolid/Rhino/JT/PVZ import-export files), crates/model/src/regen, behaviors.rs / manipulators.rs feature
behaviour, crates/featurescript, crates/model/src/custom, the Onshape oracle tests. Worktree
/root/projects/noshape-wt/core (branch agent/core), target dir /root/target-core, scratch /root/core-scratch.

## Landed on main (2026-10-26/27)
- 81c8d9cd psb's face blend WIP (curved sides, live's message rule). Open, T2 test
  `features_c::face_blend_on_an_elliptic_boss_as_live`: the elliptic boss gives 10 faces (the boss side in two
  433.959 mm² halves) vs live's 9; the curvature section fails. Probe test for it: /root/core-scratch/features_c.with_tmp.rs
  (`tmp_core_ellipse_blend_faces`, not committed).

## In progress
- P0 CP1 / R3 / C1: `features.cpp::frame_cylinders_toward_sketch` (op_prism, swept-back path) re-frames extruded
  cylinders as live; kernel test extrude_sense `an_extruded_cylinder_is_framed_toward_its_sketch`.
- P0 SHELL-WALLS + P1-16 (qa5's WIP applied): shell.cpp `collapsing_blend` / `blend_sector` (cylinder, cone, torus
  sectors as primitives), slabs instead of OCCT's offset when a kept convex blend collapses, keep the split result when
  Unify breaks it. Tests kernel shell.rs `shell_of_a_filleted_plate_with_a_hole_keeps_its_walls`,
  `shell_block_with_close_holes_like_onshape`.

## Next, in order
1. Land the psb face blend WIP.
2. P0 oracle_asm CP1 / R3 / C1: extruded cylinder frame as live ("fs (10-26 b)": origin at the far end, z toward the
   sketch, x = gp_Ax2's default for that z). Draft: /root/core-scratch/cylflip.cpp.
3. qa5 P1-16 Shell (agent/qa5 24e4766c).
4. docs/TICKETS.md core items; kern's open items (forward-oriented edges, revolve/Hole frames).

## Requests to the lead
- (none)
