# Core modelling (agent "core")

Area: features, regeneration, the kernel (crates/kernel incl. the OCCT bridge and patches/occt, not the
STEP/Parasolid/Rhino/JT/PVZ import-export files), crates/model/src/regen, behaviors.rs / manipulators.rs feature
behaviour, crates/featurescript, crates/model/src/custom, the Onshape oracle tests. Worktree
/root/projects/noshape-wt/core (branch agent/core), target dir /root/target-core, scratch /root/core-scratch.

## Landed on main (2026-10-26/27)
- (none yet)

## In progress
- psb's WIP face blends (agent/psb 2845b418) cherry-picked onto agent/core: model clippy, elliptic boss 10 vs 9
  faces, curvature section on the ellipse.

## Next, in order
1. Land the psb face blend WIP.
2. P0 oracle_asm CP1 / R3 / C1: extruded cylinder frame as live ("fs (10-26 b)": origin at the far end, z toward the
   sketch, x = gp_Ax2's default for that z). Draft: /root/core-scratch/cylflip.cpp.
3. qa5 P1-16 Shell (agent/qa5 24e4766c).
4. docs/TICKETS.md core items; kern's open items (forward-oriented edges, revolve/Hole frames).

## Requests to the lead
- (none)
