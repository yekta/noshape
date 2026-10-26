# Core modelling (agent "core")

Area: features, regeneration, the kernel (crates/kernel incl. the OCCT bridge and patches/occt, not the
STEP/Parasolid/Rhino/JT/PVZ import-export files), crates/model/src/regen, behaviors.rs / manipulators.rs feature
behaviour, crates/featurescript, crates/model/src/custom, the Onshape oracle tests. Worktree
/root/projects/noshape-wt/core (branch agent/core), target dir /root/target-core, scratch /root/core-scratch.

## Landed on main (2026-10-26)
- 81c8d9cd psb's face blend WIP (curved sides, live's message rule). Open, T2 test
  `features_c::face_blend_on_an_elliptic_boss_as_live`: the elliptic boss gives 10 faces (the boss side in two
  433.959 mm² halves) vs live's 9; the curvature section fails. Probe test: /root/core-scratch/features_c.with_tmp.rs
  (`tmp_core_ellipse_blend_faces`, not committed).

## WIP on origin agent/core (paused by the lead 10-26 ~22:30; not on main)
1. **c717ed23 P0 oracle_asm CP1: extruded cylinder frames as live** (features.cpp `frame_cylinders_toward_sketch`,
   called in op_prism's swept-back path). Checked: kernel extrude_sense (new test
   `an_extruded_cylinder_is_framed_toward_its_sketch`, live's cases a/b/e), model integration T1 (225 passed),
   oracle_asm (CP1 matches; R3/C1 flip the pin as live, the solver's −157.5° turn is skasm's; KNOWN updated).
   Still to run before landing: `cargo clippy -p noshape-kernel -p noshape-model --all-targets -- -D warnings`, model
   `--lib assembly::`, ops assembly e2e (mates on cylinders), oracle_p1 (T2; P6/CP2 use cylinder frames). Then push.
2. **256515e6 (WIP) P0 SHELL-WALLS + qa5's P1-16** (shell.cpp): `collapsing_blend` (convex cylinder/cone/torus whose
   inward offset passes its centre), `blend_sector` (exact primitive sectors: cylinder/cone wedges, torus disc
   revolved), `collapsed_blend_filler` (the neighbours' offsets run on across the collapsed blend: to the removed
   face, or to each other; swept along the blend's spine), revolved_band skips collapsing blends, slabs instead of
   OCCT's offset when a kept blend collapses, the split result kept when Unify breaks it. Debug: NOSHAPE_SHELL_DEBUG=1.
   State (kernel shell.rs tests now call `set_working_scale(1000.0)` like the model; without it P1-16's base solid
   is 13928.602 instead of 13928.664 mm³ and the slabs fail):
   - SHELL-WALLS 2.5 mm: 14950.999 mm³ / 19 faces (live 14951.962 / 19): the walls are back. 1.5 mm: 9609.030 / 28
     (live 9609.949 / 27; that ~0.92 was there before). Asked live for the filleted base solid's volume (REQUESTS
     "core (2026-10-26 b)"): if live's is ~0.95 above ours, the difference is the fillet, not the shell.
   - P1-16: 1 mm 3997.846 (live 3998.627), 2.5 mm 8503.176 / 53 faces (live 8503.961 / 47): exactly 3·π/12 short,
     the three chamfer-corner cones (apex at the top face) get no sector. Next: run the shell tests with
     NOSHAPE_SHELL_DEBUG=1 (prints "slabs: no slab for a face of type N"); probably not Geom cones (B-spline or
     revolution), so `collapsing_blend` / `blend_sector` need that surface type (a cone of revolution via its axis).
   - Then the face counts (P1-16 53 vs 47) and SHELL-SLOW.

## Next, in order
1. Finish and land 1 and 2 above (mark docs/TICKETS.md P0 CP1, SHELL-WALLS, P1-16 with the commits).
2. P0 SHELL-SLOW (8–17 s shells; time the slab path on qa's matrix), P0 REGEN-SPEED (30-feature part edit commit
   1007 ms dev / ~450 ms release vs live 171 ms; profile with ops `regen_profile` in release), P0
   `qa_regressions::shell_that_breaks_the_part_fails` on macOS arm64.
3. P1 std_oracle `hole_scope_follows_depth_as_live` (std hole.fs `holeScopeFlipHeuristicsCall` →
   `probeForViableTargets`: the blind probe's opHole `targetToDepthExtremes` / evCollision give [] at 15 mm),
   P1 kernel draft_curve `level_parting_line_matches_occt`, P1 Extrude add 308 ms (ops' add path; fs probes on
   agent/fs 81ab300e), the elliptic boss face blend.
4. kern's open items: forward-oriented edges proposal, revolve (mid-height) and Hole (start, z along the drill)
   cylinder frames (live "fs (10-26 b)" f/g), extrude up-to cylinders (op_prism_until isn't re-framed yet).

## Notes
- Live requests open: "core (2026-10-26)" (cylinder frames of slots, inclined planes, up-to, reversed revolve axis),
  "core (2026-10-26 b)" (SHELL-WALLS base solid).
- Edit → test time: not measured yet (paused before the first incremental rebuild after the lead's split).
- OCCT probes outside the repo: /root/core-scratch/cpp (`make <name>`; cyl.cpp = the cylinder re-framing).

## Requests to the lead
- (none)
