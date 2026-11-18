# Blends (agent "blend"): current state

Updated 11-18 21:05 UTC (blend thread 2fc420a3 since 20:05, replacing 0672fa20). History: docs/handoff/archive/blend.md. Rulings: docs/DECISIONS.md.

Owns fillets, chamfers, face blends: crates/kernel/cpp/features.cpp (op_fillet, scaled / checked builds,
fillet_by_tools), blend_direct.cpp (the direct build), crates/kernel/src/rim_chamfer.rs, model-core
regen/features/detail.rs, detail_a.rs, blend_section.rs (sections, rolling balls, run-ons, rims), custom host ops.rs
(opChamfer / opFillet).

## Now (11-18 21:05, new thread)

- Background runs MUST be systemd units (`systemd-run -E HOME=/root -E PATH=$PATH`): `( ... &)` jobs die at turn end.
  Units: blend-seq-x8 (bin/seq-x8.sh: f3d dump -> spring -> suspension 995636, vibex8, blend10 binary), blend-himp
  (bin/halter-imp.sh: blend8 = blend-halter, impeller with NOSHAPE_BLEND_DEBUG + ZZ_DUMP -> vibex7 ~/tmp/blend/hdump).
  Logs /root/blend-scratch/logs/*.out. Mac: ONE slot (noshape-env.sh), dx held it 19:52-; I build on vibex7/8.
- Order (lead 20:05): impeller Fillet 3 speed, Halter, c2-extension-spring Fillet 1, asm-welding-config Part 6
  fillets; + asm-suspension 995636 Fillet 3 ("Unfilleted edge overlapped by fillet.", live OK; tell asm when landed;
  805e29e8 Fillets 3/4 after skasm's Sketch 2). Dropped (9e5b481d4): snap-hook Fillet 6, motor-mount Fillet 2 ms items,
  and the full-round land (more code, not simpler).
- Fillet 3 (Mac release, f3t logs): ~2.3 s = before_direct 85 + failing direct 146 + closed_run_on 840 (met 280,
  corners 380) + reface 55 + Boolean 830 + unify 55. Direct build on the trimmed tools (ZZ_DIRECT2) = "an invalid
  solid" in 395 ms; dumping it (ZZ_DUMP_DIRECT -> vibex8 ~/tmp/blend/f3d2) to replay with bin/f3d-replay.sh <n>.
  blend10 uncommitted: cached face boxes + threaded touches / corner splits in closed_run_on (met 413 -> 280).
- Halter: the old dump (Mac zzdump body-36 + add-36-*) is the tighten_tolerances variant: body invalid; tip fills'
  sides 0.04-0.115 mm off their supports (x 5 fills 2.4e-5) and every fuse invalid. Suspected root upstream: our
  tip x 1.502 = Chamfer Halterung's foot; live's Fillet 3 extends the chamfer face (6.18 -> 12.67 mm2) down to the
  tip x 0.097 (live/blend/q-imp-halter). Probes kernel blend.rs zz_fill / zz_desc (blend8 uncommitted).
- spring Fillet 1: two 0.5 mm CURVATURE rounds meeting in the middle of the slot's 1 mm end (x -11.7): live keeps two
  faces JMG / JMC (0.8113 mm2) with an edge at y -1.0 (11/27/18); ours 10/24/16 = likely one face. Run pending.
- live's Fillet / Chamfer pick rows (feel dialogs 11-18 batch 1): green CLI counts = live (R8 block 7; propagation on
  10, off 8, face 12, chamfer defaults 10); chips / highlight / timing are psui's.

## In flight (older)

- Speed is judged on the Mac (release, quiet, median of 3) against live (DECISIONS 11-16); vibe numbers are hints.
  Done: robot-relations a4e419 (Mac 65 vs live 78), cf-part-draft Spur Gear (76-81 vs 227).
- Knife blends left (Mac): Fillet 8 65 (live 12: OCCT's working-scale round 45 after the as-is fails 7; split-line
  join 5.8 needed), Fillet 3 45 (direct build 26: classify.tool 7 = ray intersectors on lofted tools; lofts; conic
  rebuild), Chamfer 2 35, Fillet 5 21. Probes: parked/zz-*.patch; Mac A/B: bin/ab-mac.sh <base> <cand> <doc> <regex>.
- Backlog (lead: parked, >1 day): Fillet 5 to live's 9 ms needs a contact-curve build: blend faces made straight from
  the ball sections and contact curves (no solid tool), supports trimmed along the contact curves in 2D (no lies_on,
  no IntSS on supports), the shell from known adjacency, only new faces checked; est. Mac ~8-10 ms, 3-4 days incl.
  every rolling-ball fillet identical on the suite.
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

1. NOW (lead 11-18 17:xx): (a) land the full round (gating), (b) Halter = Fillet Stevenrohr zum Halter (P1, impeller's
   first difference; blocks core's Move face 2 / Delete face 2), (c) c2-extension-spring Fillet 1 (10/24/16 vs live
   11/27/18), then Fillet 3's direct-build end trimming (multi-day).
   Landed 11-18: 7c60d5ea impeller Fillet 4 (cliff rounds on curved edges; T1 features_c::a_round_wider_than_a_
   curved_flange...), 51510232 its face reach by projection (Mac 612 -> 370 ms), 270e489b by-tools direct build checked
   once (snap-hook Fillet 6 25 -> 23 ms; the rest is 40 cone x torus IntSS 5.9 ms, axes parallel 50-230 mm apart: left).
   (a) blend-fullround 16a7b76ae (blend10 branch; blend-base detached at it): op_full_round_coaxial (blend_direct.cpp):
   a bore's FULL_ROUND by face replacement (half torus revolved from the profile arc, the sides' loops replaced by its
   circles, only new faces + shells checked), torus renamed as the Boolean's tool face; motor-mount-jack Fillet 2 Mac
   9 -> 2 ms x5 (live 3), 10/10; T1 kernel_robustness::a_full_round_of_a_bore_or_a_boss_is_a_half_torus passes.
   Gate: unit blend-seq-fr2 (bin/seq-fr.sh): suites blend4 (main) / blend-base on vibex7 -> suite-x7-fr{base,}.log,
   .cmp, g13-fr.log, fr.done. Land when only motor-mount... differ in speed (identical results).
   (b) Halter: branch blend-halter (WIP 663 lines on af331f85e); first look upstream: Chamfer Halterung's bevel (f26)
   foot at 0.21 mm tolerance, bevel faces 6.097 vs live 6.18 mm2, tip x 1.502 vs live 0.097.
   qa 11-18 Mac re-time (main): snap-hook Fillet 6 25/8, motor-mount Fillet 2 9/3 (now 2), others within 1.5x.
   (b) Fillet 3 on main (Mac, 3 runs): 2269-2277 ms (live 103). Profile (logs/imp3-prof.log): op_boolean x3 884 ms
   (OCCT edge-face 594), min_distance x149 339, bounding_box x339 251, op_split x10 223, blend.direct 132 (declined:
   open shell, 20 free edges at the rounds' ends x 0 / 78.9 and x 1.5-5), unify 73. Path: the conic's untrimmed ball
   tools go to the direct build (fails), then closed_run_on + Booleans. Next: make the direct build take the
   closed_run_on-trimmed tools (or close its ends) so the Booleans go. Same run: Chamfer Stevenrohr Halterung 272 (live
   16-20), Fillet Bohrungssitz 938 (242), Flansch zum Stroemungskanal 137 (66.5), Fillet 1 74 (39), einnschub 55 (30),
   Delete face 2 226 (12, core's?).
   bool 11-18: the Boolean's 680 of 720 ms are 4 edge/face pairs (135-150 ms each): the housing's 141 / 144 mm B-spline
   edges round its offset face F2 against each tool's strip face lying on F2's surface (another handle and basis);
   bool's Rust Boolean declines for the same reason. Mac cases ~/noshape-host/bool-keep/imprec. The direct build avoids
   it: its dump replays in 180 ms (blend10 patch parked/zz-dump-direct.patch: ZZ_DUMP_DIRECT=<dir> on a run,
   crash_inputs::zz_direct with ZZ_DIRECT=<file>; Mac dumps ~/noshape-host/agents/blend10/f3d/direct-1.brep). Open
   shell: the tools run x -1.5..78.97 untrimmed; sections blend x part faces 8 / 18 (the 6.10 mm2 chamfer faces live's
   rounds end on) and the (8 x 6) / (9 x 0) pairs give 0 lines. Lead 12:4x: setback fillet -> core (sent); order: land
   f4 + speed, then Halterung (rim_chamfer_cut 230 ms: tool 70 = 3 lofts + 3 Booleans, cut 95, unify 31, 2 checks 21;
   needs the same end trimming for live's 16-20), Bohrungssitz (938 ms all OCCT fallbacks failing on a wrong input:
   3 parts vs live 1 upstream: skip), then this end trimming.
   Direct-build ends (blend10 branch blend-direct-ends 00e3410af = dump patch + probes; parked/direct-ends-wip.patch):
   pairing every face a blend's box meets (ZZ_ALLNEAR; today only faces beside its supports) + keeping a ridge section
   whose run-on to a vertex won't project gets to the sew, still open. But the untrimmed tools run on to the part's end
   (x 79) while live's rounds stop at x 54.7-61 on the first faces met (closed_run_on's walls): the design is to trim
   each tool's run-on at its first walls (the wall's surface extended, analytically, no op_split / Booleans), then
   hand the trimmed tools (end caps on the walls) to the direct build. Chamfer Halterung: cheap trims (one Common with
   slab ∩ stop, one validity check) measured 270 -> 268 ms and the doc +150 ms: dropped (zz-blend-chspeed-dropped).
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
