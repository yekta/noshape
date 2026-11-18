# Blends (agent "blend"): current state

Updated 11-18 11:50 UTC (new blend thread 0672fa20-964d-4fde-a19c-c5b4363729ba, account Bbedward). History: docs/handoff/archive/blend.md. Rulings: docs/DECISIONS.md.

Owns fillets, chamfers, face blends: crates/kernel/cpp/features.cpp (op_fillet, scaled / checked builds,
fillet_by_tools), blend_direct.cpp (the direct build), crates/kernel/src/rim_chamfer.rs, model-core
regen/features/detail.rs, detail_a.rs, blend_section.rs (sections, rolling balls, run-ons, rims), custom host ops.rs
(opChamfer / opFillet).

## In flight

- Speed is judged on the Mac (release, quiet, median of 3) against live (DECISIONS 11-16); vibe numbers are hints.
  Done: robot-relations a4e419 (Mac 65 vs live 78), cf-part-draft Spur Gear (76-81 vs 227).
- Landed 11-16: 684e77e2 (samples / ball tools ahead), 534c6e30 (lies_on skip, loft orientation by rays, run-on trims),
  ce61a597 (scale-first), 35c63214 (Fillet 3's direct build beside OCCT's try, loft probe two-sided per core's review,
  sliver check by 1% areas). Mac x3: knife Fillet 2 95 -> 85, Chamfer 1 117 -> 112, Fillet 3 63 -> 47, Fillet 5 23 ->
  21, Fillet 6 20 -> 7 (live 7); knife ~1020 ms (live 622). Lead split the knife's non-blend time: bool (Split 4,
  Mirror 1, Chamfer 1 / Fillet 2 Booleans, Revolve 1, Extrudes 2/3/8/9), core (Extrude 10, Lofts 1/3); both told.
- Landed 11-17 00:42: 0aad993d (367cf3ae) conic contact edges checked by the faces around them (Fillet 3 47 -> 45).
- cf-spur-gear (11-17, Mac release, main 5b23c028f): every Spur Gear is under live (117/56/34/98/77 vs 955/95/75/283/
  343); its opFillet is 0.3-0.4 ms, the time is the Rust op_boolean (112/46/26/78/53 ms). Sent to bool. Nothing for blend.
- Landed 11-17: blend-necks ba7a8d755 (isaac-ball 9 -> 32/33) broke rust-zero-solid-f197's Mirror 3 (4 slivers: the
  retry's necks cut on a result already valid changed Chamfer 3); reverted (b596da51b), re-landed 07:05 as 74d05ad5e:
  a valid retry keeps its necks unless cutting them lets a full round merge. T1 blend::a_valid_chamfers_necks_stay_and_
  its_mirror_adds_whole (data blend/f197-chamfer3-input.brep). Before landing: userdocs + f197 + Hole docs + every
  ~/qa/tickets doc + qa's compat v0443 docs SAME (bin/ud-nk3.sh, the release-candidate set), g13 OK. GearLab
  (b2e0cbd71) landed.
- qa P0 speed (11-17, vibex7 A/B v0.4.48 -> v0.4.53): knife Chamfer 2 10.7 -> 67.9, Fillet 5 18.8 -> 95, Fillet 3 136
  -> 188 ms; impeller Stevenrohr zum Halter 0.6 -> 2610 ms (fixed by 29c1841df, after .53; Mac main 2 ms). v0.4.53
  predates the 11-16 speed lands; to do: Mac A/B v0.4.48 vs main on those three (bin/ab-mac.sh; commit WIP first).
- Knife blends left (Mac): Fillet 8 65 (live 12: OCCT's working-scale round 45 after the as-is fails 7; split-line
  join 5.8 needed), Fillet 3 45 (direct build 26: classify.tool 7 = ray intersectors on lofted tools; lofts; conic
  rebuild), Chamfer 2 35, Fillet 5 21. Probes: parked/zz-*.patch; Mac A/B: bin/ab-mac.sh <base> <cand> <doc> <regex>.
- Backlog (lead: parked, >1 day): Fillet 5 to live's 9 ms needs a contact-curve build: blend faces made straight from
  the ball sections and contact curves (no solid tool), supports trimmed along the contact curves in 2D (no lies_on,
  no IntSS on supports), the shell from known adjacency, only new faces checked; est. Mac ~8-10 ms, 3-4 days incl.
  every rolling-ball fillet identical on the suite.
- Landed 11-16: d7f9264c9 impeller chamfer end edge joined (impeller 26 -> 27/48 on main; strict suite 127/127, userdocs
  SAME). Core told.
- Scale-first blend6 4b2f94289 (base 3178c98b5): release suite strict-identical 127/127, userdocs ALL-SAME; assemblies
  (vibex6, asm-sf-base.log / asm-sf.log, unit blend-seq-sf3) running; land when identical.
- Live's knife times (11-16): Fillet 1-8 8/9/10/6/9/27/7/12 ms, studio 622; 11-17 (live/blend/knife-regen-1117.json):
  Chamfer 1 19, Chamfer 2 7, Fillet 2 10, Fillet 3 10, Fillet 8 12, studio 597-693. Lead: Fillet 5 at or below live is P0.
- impeller Fillet 3: 4efe95564 landed (be6f0ca8, 34/93/60). 3df3608b1 landed 13:58 as f6274d94 (core told):
  kept edges along faces the convex rounds end on (the front circle's 3 arcs, 34/95/62 as live; impeller 29/48, first
  difference Halter), merge_on_surfaces without the volume integrals (3.4 s; adaptive 15 % off), mark_kept_edges takes
  `ends` (imprint_kept false: fan-blade's Derived 1 Poti2 went 48 -> 0/53 with ends). Gate: strict suite only the
  impeller differs (27 -> 29/48), userdocs/tickets/qa docs/f197 SAME, g13 OK (bin/seq-imp3.sh, ud5-imp3.sh). Fillet 3 Mac ~4-5 s (live 0.1): speed P0 after Halter (hotspots op_boolean, props_surface x24,
  reface_on x4 whole-shape BRepCheck, min_distance x268 = closed_run_on's touches()).
- Halter (P0, impeller f29, R2 on edges 55/56/59/60 of 35 faces, tangentPropagation off, overflow off):
  live/blend/q-imp-halter (11-17): the round on Jfp runs past the tip along JfV and ends against the wall; tip moves to
  x 0.704; JfV -> 2.4405, chamfer 7.9946 mm2; the x 5 rounds run to x 7; rounds meet in convex edges. Ours: refused by
  curved_edge_left_open; skipping concave edges there (/root/blend-scratch/halter-convex.patch) it fails in frame_at:
  every probe at the tip classifies OnBoundary. Cause upstream: Chamfer Stevenrohr Halterung (f26, OFFSET_ANGLE 3.5 mm
  20 deg FACE_OFFSET) leaves its bevel foot on the wall (x 1.5) at 0.21 mm tolerance, bevel faces 6.097 mm2 vs live
  6.18; Fillet 3's Cut then makes chamfer-extension x wall edge (x 1.5 to the tip) at 2.33 mm tolerance; our tip
  x 1.502 vs live 0.097 (live JfR curves to -x). GeomLib C2 extension changes nothing. Dumps Mac
  ~/noshape-host/agents/blend10/zzdump/sections-{31,34,35}.brep (+.txt edges); probes kernel blend.rs zz_halter /
  zz_extdev; ZZ_TOL / ZZ_DUMP / ZZ_FRAME prints in blend_section.rs (blend10, uncommitted). Next: f26's bevel.
- Agent ids (11-18 11:45): asm d8d597e0-675c-497b-a804-544ceab99bbd, skasm
  18a8ce79-5f2e-4089-9ad8-cc9d1182f0e6, bool2 e98465f5-5cc9-4d4f-b67e-aae1d5e6d2e0, studios
  58e9bca8-346c-4847-8012-e15006ae61ef, live e8549719-ef9a-42de-960c-e24b29f142a6, live2
  79ae5cdb-42a4-4025-b43a-da61a4f92ad7, bool 7fa8ceab-4549-499f-9b01-3a289c2c95b4, core e8af89fe-d460-49b3-b0bb-9572da230fed,
  qa 132756b3-e48d-4b9f-8a0e-3d4f23629c8b, psui 4973e295-9e92-463f-a13f-2ec5258bc5f9, dx 2108c2b7-2f48-43a7-87a0-3f652e3cda6d.
- Queued from asm (after the current items): asm-gearbox-std 6a912b / a470167 Fillet 2 (EDGE fillet of 2 faces' edges,
  tangent, 0.125 in: ours 38/92/56, live 38/88/52, -12.1 mm3; repro vibex6/7 NOSHAPE_PARITY_MODELS=~/agents/asm-keep/mgb
  NOSHAPE_PARITY_ONLY=m6a912b); asm-scissor-lift fed22d Fillet 3 (Top - Side 2 45/110/64 vs 48/116/64; Motor Mount Live (q-gearbox-f2/corner.json): each corner a torus (27.55 mm2, its edge on the vertical round the
  whole r 6.35 quarter) + one 3-sided blend face (29.11 mm2); no 2.158 arc, no vertex along that edge (ours 61.4 mm2 vs
  56.66).
  69/128/50 vs 66/121/46), then Fillets 4, 5, 15 fail (repro vibex7 NOSHAPE_PARITY_MODELS=~/agents/asm-keep/ms
  ~/agents/dx-keep/ops-gB3; on main + cf49196a it differs at feature 6 Mirror 1 already: parked until dx's translator
  work lands). Gearbox: live's corner topology asked (REQUESTS 14:02); ours at each of 4 corners an OCCT 48.7 mm2
  B-spline patch + a 4-sided 12.72 mm2 torus.

## Open questions / waiting

- infento 1dc32b Chamfer 3 (live 3169: 23 mm on the cylinder at both R65 arcs, by volume 87,057.04). Ruled out: a
  merged face ranking as the later Add's (live G60: both variants on the top, as our rule), qAdjacent / qOwnedByBody /
  shell order (the two arcs, congruent under Transform 12's rotation, come out opposite). x_t (live q-x38, read with
  our reader + ZZ_XT_FINS debug, blend3 uncommitted): at both arcs the edge's first (forward) fin is the cylinder's;
  on G37's block the first fin matches G30 only 4/6 ((a) front, (e) right fail), our class rule 6/6. Fits all: class
  order when both faces are one feature's, else the forward fin (left face) (G30 (i) too). Needs Parasolid's edge
  direction, which our OCCT edges needn't share. Decision asked of the lead. Parked: parked/ch3-alias-rule.patch.
- c2-impeller-loft Fillet 3 (feature 27, now the first difference with 12b25c47e): CONIC R 3 rho 0.5 on the two
  joined 59.4 mm lines at y = +-2.6104, z = -43.106; each is the lip of a lens-shaped wall: face 0 the 5.2 mm bottom
  strip of a 9.47 mm bore (concave cylinder, axis z = -34), face 1 / 3 Extrude's surface below it, 9 deg apart.
  frame_at's 45 deg probes all classify outside ("could not orient a face at the edge", 10 ms). With the face's own
  normal as fallback (parked/f3-wedge-frame-wip.patch) the conic cut leaves 2 solids, 2.1 s; live removes 280 mm3,
  33/86/54 -> 34/95/62. Fillet Stevenrohr zum Halter (29) follows from it (its edges are Fillet 3's). Probe
  parked/zz_probe_f3.rs, dumps vibex7 ~/tmp/blend/im3 (fillet-16 = the part after the chamfer). Live (live.jsonl
  26 -> 27): removes the bore strip Jee, both lips' Extrude faces trimmed (840 -> 604 mm2, x to 54.7), the chamfer's
  two 6.18 mm2 tool faces -> 12.68 mm2; two conic faces JfC / JfG (177 mm2, y 0..+-1.02, z -43.9..-46.77) meet in a
  ridge JfZ along y = 0 at z -43.9 (below the bore's bottom -43.47): each round overflows the strip, touches its wall
  far down and is clipped by the other. Needs a two-round overflow build (Parasolid's), not section_blend per edge.
  core waits on 27-29 (Extrude impellerraum on depends on them). The chamfer takes 0.6-1 s (live 20): 4 Booleans of
  rim_chamfer_cut's tool after OCCT's chamfer fails twice and section_blend once.
- Boolean sections (bool's): the knife's Chamfer 1 58 ms / Fillet 2 17 ms are OCCT on pairs that touch within the
  knife's ~21 um tolerances (a disjointness test proves 4 of 96; parked hulls-wip.patch).
- cf-snap-hook (dx): Mac release Fillet 6 26 ms (live 8), Fillet 3 14 (7); the document 243 vs live 274. Low.

- PARKED (lead 11-17 04:10, impeller P0 first): knife Fillet 8 join, blend10 `blend-f8split` c7fe2dc61 (rebased on
  main; parked/f8-ring-join-wip.patch). Fillet 8 15/37/24 vs live 15/37/25 (the vertex: Revolve 1's closed-edge vertex,
  core's). Pieces: join_ring_rounds (round faces of edge pieces shown as one edge: rational B-splines end to end, the
  other cross edge the seam, then loop_pieces_joined, a wrapper of core's join_loop_pieces), split_at_support_joints
  (across-curvature jump > 20 % at C1 knot lines), a new KeepCutsMark (kept as kept_ends by joins / unify, not shown
  where a seam runs through), marks carried both ways through Scaled, join_split_lines never between two faces the
  blend left (it had joined the handle's far-end lines). To do: remove the 3 join_rings timers, cut its ~10 ms
  (whole-solid checks / shown_topology), T1 test, strict suite + userdocs + g13, core reviews common.cpp / boolean.cpp.
  Then Fillet 8's speed (67 vs live 12).

## Next, in order

1. NOW (lead 11-18), in order: (a) impeller Fillet 4 (gating), (b) Fillet 3 speed, (c) knife re-time, (d) Halter.
   (a) Fillet 4: branch blend-f4 (blend9, f9df34212 on main ab6da5d97): after OCCT fails with overflow off, section
   blend with cliff (detail.rs); the cliff face is decided once per edge at its middle (the floor turns away at the
   rim's ends: reach 0.0009 mm there), its reach found per station; no cliff face on a curved chain -> declined. T1
   features_c::a_round_wider_than_a_curved_flange_takes_its_side_as_live (data impeller-fillet4-input.bin = core's
   dump fillet-32, 43 faces; live's input 64): 268.31 mm3 off (live 270.11), round 371.22 mm2 (372.45) z -4..0, sliver
   kept. Mac full doc: Fillet 4 OK valid 105929.52 mm3 (live 105931.13); Move face 2 still ERROR (core told 11:45: input
   differs from Boolean 3, 43 vs 64 faces). GATE running: unit blend-seq-f4 (bin/seq-f4.sh): suites blend4 (base) /
   blend9 on vibe2 -> suite-r-f4{base,}.log, .cmp, f4-suite.done; documents on vibex7 -> seq-f4.log, ud-f4.done. Rust
   only (no g13). DON'T edit blend9 / blend4 / blend-base until it's done. Land when identical but impeller.
   Speed: Fillet 4 Mac 602 ms (live 102): Boolean 293, face_reach bisection 153 (1915 distances), OCCT try 83.
   face_reach by projection: blend10 branch blend-f4-speed ac197ab11 (A/B running: bin/ab-mac.sh -> logs/ab-f4.out);
   land it after (a) (patch parked/f4-reach-speed.patch).
   (b) Fillet 3 on main (Mac, 3 runs): 2269-2277 ms (live 103). Profile (logs/imp3-prof.log): op_boolean x3 884 ms
   (OCCT edge-face 594), min_distance x149 339, bounding_box x339 251, op_split x10 223, blend.direct 132 (declined:
   open shell, 20 free edges at the rounds' ends x 0 / 78.9 and x 1.5-5), unify 73. Path: the conic's untrimmed ball
   tools go to the direct build (fails), then closed_run_on + Booleans. Next: make the direct build take the
   closed_run_on-trimmed tools (or close its ends) so the Booleans go. Same run: Chamfer Stevenrohr Halterung 272 (live
   16-20), Fillet Bohrungssitz 938 (242), Flansch zum Stroemungskanal 137 (66.5), Fillet 1 74 (39), einnschub 55 (30),
   Delete face 2 226 (12, core's?).
   (b2) Merged-body Fillet 3 (bool2's fillet-26) recheck: still to do.
   (e) lead 11-18: Fillet Bohrungssitz Ansaugflasch (f48, last feature) text 'Unfilleted edge overlapped by fillet.'
   (v0.4.58) -> 'Failed to fillet selections.' (main): its input moves with every upstream change; the text is whichever
   fallback fails last (fillet_failed maps OCCT's "unfilleted edge overlapped"). Live builds it; both are wrong.
   (f) live 11-18: cf-setback-fillet Setback Fillet 1 (custom: opSplitFace x2 + opDeleteFace + std fill() G1, 3 corners):
   ours A 4117.54 vs live 4116.42 mm2 (V 18858.78; corner B-splines 26.07 / 26.98 / 34.46 mm2). Likely the Fill
   (core's surfaces.rs) -> measure our 3 patches, then hand to core or take it. live/suite-truth-1118/cf-setback-fillet.json.
2. Halter (multi-day, lead; land in gated steps; tests: Fillet Stevenrohr zum Halter, core's Extrude impellerraum
   input). Branch blend-halter (blend10). tighten_tolerances does NOT land (one more raw face behind equal shown
   counts); keep it only if Halter needs it.
   State (blend10 uncommitted on 1aabb5b68, 21:54): corner_run (spec.run_on + cornered chain): one loft per edge;
   at a corner end the section runs straight on 2R; at a free end (the tip) a run of its own: continued_on_faces
   marches the supports' crossing (Newton on both tangent planes: OCCT's section of the two extended B-splines at
   ~20 deg came back empty), sections spaced as the edge's stations, lofted apart from the edge's loft (one loft
   through the tip's turn bulged to 200,000 mm3), outward (negative volume reversed), fused to it, handed to
   closed_run_on as the run (no plane split for fills). Runs 0.031 mm3, tools 3.31 mm3; each run meets only the wall
   (faces 1 / 12, as live). Fails at the final Fuse (now caught: "the result lost the body", a volume guard for corner
   runs). In situ (ZZINSITU): fills 0/1 (x 5 rounds) and 2 fuse valid alone; fill 3 (tip, mirror of 2) alone invalid
   -19 mm3 even on the unrefaced body; fill 2 overlaps the body 0.40 mm3, fill 3 only touches. Tried, no: re-loft
   instead of reversed() (kept), fuzzy 1e-7/1e-6, glue shift/full (fills stay separate solids), corners pushed into the
   part (ZZ_INTO=0.3: x 5 rounds fine, tip fills lose the body: the tip is a sliver). Next idea: build the tip fills
   so they cross the supports transversally only where the part is thick, or trim fill 3 against the body first.
   Debug prints ZZ* / ZZ_INTO to strip before landing. Paused 11-18 05:50 for the lead's order (item 1). Branch
   blend-halter in blend10, last commit 2a74a13b1 (+ handoff commits).
3. P2 asm: GearLab 1e81ca opFillet FILLET_FAILED (repro vibex7 NOSHAPE_FS_TRACE=1 NOSHAPE_PARITY_MODELS=
   ~/agents/asm-keep/m1e ~/agents/asm-keep/bin/ops-c03 parity::parity_models); isaac-ball a2e4a2 f23 Chamfer 2
   (one edge, TWO_OFFSETS 5.5/6, oppositeDirection false, tangentPropagation true; ours 51/124/68 40986.25 vs live
   53/126/68 41594.02: 608 mm3 more removed, 2 faces fewer; Extrude 12 then leaves 403 of 40911, ~24 isaac mates lost
   incl. Fastened 33; repro vibex8 NOSHAPE_PARITY_MODELS=~/agents/asm-keep/ma2 ~/agents/asm-keep/bin/ops-c19
   parity::parity_models --ignored --nocapture, trace ma2/tr19.log l.105); asm 21:5x: c3fdd4 (isaac Part 2 <7>) f11
   Fillet 1 "Failed to fillet selections." (live OK), reachable after asm's Extrude 7 fix; repro vibex8
   ~/agents/asm-keep/mc3. Also e54410 f51 Chamfer 1 (Part 1, one edge, TWO_OFFSETS 2/5, tangentPropagation true)
   "Failed to chamfer selections." (live OK; before 46/116/68, live after 47/119/70, ~138 mm3 off): likely the same
   TWO_OFFSETS cause as a2e4a2; costs Ball 13/14, Revolute 17; repro vibex8 ~/agents/asm-keep/me5 (bin/ops-c21,
   trace me5/tr.log).
3a. After Halter (lead 11-18): impeller Fillet 3 on bool2's merged body (33 faces, the two r 10 cylinder halves one face,
   as live after Chamfer 26) fails "unfilleted edge overlapped by fillet" in OCCT's path (live 34/99/66, 71096.12 mm3).
   Input vibex8 ~/agents/bool2-keep/fillet3/fillet-26.{brep,txt}: edge 81 (full circle r 6, centre (120,0,-34) mm, axis
   X, on plane x = 120), r 2. Replay: NOSHAPE_REPLAY=<dir>/fillet-26 cargo test --release -p noshape-kernel --test all
   crash_inputs::replay_one -- --ignored --nocapture; occt-chamfer-fillet-28 (35 faces, unmerged) succeeds. Lead: the
   face cleanup stays; the fillet must take the merged face.
3b. core 11-18 (main 55fc929c): c2-surf-workshop-knifehandle: Fillet 3 Blade 7/15/10 vs live 6/12/8, then Chamfer 1
   20/36/21 vs 7/15/10, 9285.8 vs 10423.8 mm3 returned OK (wrong geometry), Fillet 2 ERROR. Mine (Chamfer 1 likely
   follows Fillet 3). Repro NOSHAPE_PARITY_ONLY=c2-surf-workshop-knifehandle.
4. Low (asm): asm-robotic-arm eed029 "nắp đế" Fillet 1 (f21) 44/93/44 vs live 44/95/46 (area -0.16 mm2), Draft 1
   then fails; repro vibex7 ~/agents/asm-keep/mra/run.sh <bin tag> (NOSHAPE_PARITY_MODELS=~/agents/asm-keep/mra).
4a. dx 11-18 (exchange suite, DXF counts; files vibex8 ~/tmp/dx/exs-m5/<doc>/{ours,live}-drawing.dxf):
   c2-variable-fillet-faceblend Fillet 1/3 (FULL_ROUND, tangent): our full round's boundary edges are exact circles,
   live's B-splines (sheet ELLIPSE 25/13, SPLINE 8/23). c2-extension-spring Fillet 1: ours 10/24/16 vs live 11/27/18
   (live splits each hook-end swept edge into curve + short straight end: LINE 120/117, SPLINE 37/47).
4c. qa 11-18 (main 7db466c62, vibex8 relative; raw /root/qa-scratch/feat3/ab.jsonl) features >5x live: impeller Fillet 3
   12364 vs 103, Chamfer Stevenrohr Halterung 1430 vs 16, Fillet Bohrungssitz Ansaugflasch 3388 vs 242 (ours Error),
   Fillet Flansch zum Stroemungskanal 1140 vs 66.5, Fillet 1 322 vs 39, Fillet Stevenrohr einnschub 156 vs 30; knife
   Fillet 2 188 vs 8, Fillet 8 156 vs 11, Chamfer 1 203 vs 17.5, Fillet 3 109 vs 10, Chamfer 2 72 vs 7.5, Fillet 5 54 vs 9.
4b. live 11-18 (behaviours.md "per-feature cost by rolling the bar forward", live/qa/rbstep-1118/): live.jsonl
   per_feature_ms is a valid per-feature target. Live: impeller Fillet 3 103 ms; knife Chamfer 1 17.5, Fillet 2 8,
   Fillet 3 10. (qa's "ours" there, 39 s / 492 / 344 / 316 ms, are loaded-host numbers; judge on the Mac.)
5. qa's speed A/B (knife Chamfer 2 / Fillet 3 / Fillet 5), then the parked knife Fillet 8 join.

## Parked (with where)

- Known sections API: /root/blend-scratch/parked/knownsec-wip.patch (bool agreed to park; tangent support pairs aren't
  the cost).
- Debug patches: parked/f8-spring-split-wip.patch (split + ZZ_KNOTS report), parked/zz-ahead-dbg.patch (per-pair occt_ahead time), knot-report-dbg.patch (round-edge crossings),
  hulls-wip.patch (pole-hull disjointness + true distances), chordfoot-wip.patch (superseded by chordnear),
  impf3-wip.patch (impeller Fillet 3: 1.8 mm-loose vertices; the conic loft builds, the cut is invalid).
- L-joint Fillet 2 (asm, no mate impact): live's corners sphere + torus + B-spline where OCCT fills one B-spline;
  needs a corner builder (data blend/q-ljoint).
- RING-BLEND-SPEED (rings-loft Fillet 1 / Chamfer 1, cut by tools on the part's faces), RABC (setback fill patch),
  the dock's "outer arc + step" subset: see archive.
- asm suite blend failures f0eedd Fillet 3/4, Chamfer 2, a3f7a1 Chamfer 3, 74d3d6 Fillet 5/10: upstream differences
  (Loft 1 core's; Import 1); f0eedd's build standalone.

## How to run

- Hosts (11-18): builds on vibex7 or vibex8 (whichever the lead names; vibex7 off 02:00-04:00 for qa), suites/gates
  on vibe2 via ~/bin/slot; land NOSHAPE_LAND_QUEUE_HOST=vibe2. Scripts: bin/model-x8.sh <models dir under ~> <only>
  [ENV..] with BLEND_HOST / BLEND_WT (one parity model, release, validity + differing, via slot with 14 GB free ->
  /root/blend-scratch/model.log); bin/imp-x8.sh = the impeller. Kill only own pids (never from a computed pid).
- Builds and crate tests on vibex7 (lead 21:23; rb-hosts), from blend10: bin/imp-x7.sh <ENV=..> runs the impeller
  with debug env (log -> /root/blend-scratch/imp.log). The Mac (bin/imp-mac.sh) for speed numbers only. vibe2: land
  queue (NOSHAPE_LAND_QUEUE_HOST=vibe2) and the user's own server: only ~ngyekta there, no more heavy runs.
- Never kill a process tree from a computed pid (11-17 21:00 a ppid of 1 took the main machine down): systemctl stop
  own units, kill only own $! pids.
- Kernel C++ with g++-13 before landing: /root/blend-scratch/bin/g13.sh <worktree> <log> (vibex7).
- Pairs / asm on vibex7 (lead 19:50), through `~/bin/slot` there (my scripts call it): bin/host-pair.sh <tag> <host> <base wt>
  <new wt> (logs suite-<tag>-{base,new}.log, .cmp, .walls), bin/asm-one.sh <wt> <log> <host>. Long runs as systemd-run
  units with flags /root/blend-scratch/<tag>.done.
- Release timing on the Mac: `NOSHAPE_KERNEL_PROFILE=1 NOSHAPE_PARITY_ONLY=<doc>` parity_models --release; macOS
  `sample` via bin/sample-mac.sh (DOC=, REL=1, SECS=) and /root/skasm-scratch/bin/sample_{children,parents}.py.
- An assembly studio as a model: copy studios/<id>.json, .sketches.json, .live.jsonl, .refs.jsonl to
  <dir>/st-<id>/{features,sketches,live,refs}.json(l), NOSHAPE_PARITY_MODELS=<dir> (Mac: ~/noshape-host/agents/
  blend10/sg/st).
- Debug env: NOSHAPE_BLEND_DEBUG (sections, rolling balls), NOSHAPE_BLEND_DIRECT_DEBUG (direct build, per-section
  times), NOSHAPE_FREE_DEBUG (bool's per-pair outcomes).
- Worktrees: blend10 (Mac builds), blend4 / blend6 / blend8 (vibex6 gate copies; blend8's target deleted), blend9
  (handoff), blend-ch3-wip branch.
