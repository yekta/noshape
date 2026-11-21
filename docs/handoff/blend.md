# Blends (agent "blend"): current state

Updated 11-21 21:29 UTC. History: docs/handoff/archive/blend.md. Rulings: docs/DECISIONS.md.

Owns fillets, chamfers, face blends: crates/kernel/cpp/features.cpp (op_fillet, scaled / checked builds,
fillet_by_tools), blend_direct.cpp (the direct build), crates/kernel/src/rim_chamfer.rs, model-core
regen/features/detail.rs, detail_a.rs, blend_section.rs (sections, rolling balls, run-ons, rims), custom host ops.rs
(opChamfer / opFillet).

## Now (11-21 20:11)

- Hosts (user 11-21, DECISIONS a16b39297): builds/tests on rb-hosts (vibex7); the Mac only via its one 4-job nice-19
  slot. My Mac target was deleted. Never `| head` a cargo pipe: it kills the build (filter the kernel warnings, then tail).
- RINGS Fillet 1 (9.6 mm): blend10 commit (amended 0c2e6249b..) = all_closed + fuzzy cut + T1
  features_c::ring_rims_round_one_face_each_where_they_meet_at_the_bottom on data/rings-loft-ring-9_6.brep (our
  ring before Fillet 1, from NOSHAPE_RECORD_BLENDS). Passes with the fix (17 faces, rounds 171.819, bore 153.627,
  chamfers 9.79; live 171.817 / 153.624 / 9.797); fails on main (21 faces). The engine.rs extrude test was dropped
  (main passed it too). Gate: model 596 ok; gaterings.sh hit the 2 h task limit during the vibe2 suites, resumed by bin/gaterings2.sh (suites -> suite-rg.cmp) + bin/gaterings-ud.sh (-> ud-rg.log) ->
  suite-rg.cmp + ud-rg.log must be SAME apart from c2-rings-loft, then `git diff --stat origin/main...HEAD`, land.sh
  from blend10, tell bool: 9.6 exact, 10.4 leaves a 0.774 mm² bore island at the bottom (open).
- SCISSOR (asm P0, asm-scissor-lift; copy vibex7 ~/tmp/blend/scissor.noshape; bin/scissor-x.sh <cli> [tabs],
  scissor-f.sh <cli> <tab> <feature> (profile + records), scissor-e.sh (edit entities), scissor-p.sh (part + def)):
  - 2d82a0 Fillet 13 (R1 on two whole end walls of the Middle Cover slab; live 24/61/40 -> 48/107/62, -11.505 mm³;
    our input 25/63/41, 22549.240 vs live 22549.298): "stops" path -> section_blend general (ellipse edges) lofted
    BOTH walls' outlines as one 64 mm chain -> 23 s invalid Boolean. Worktree blend-sc (branch blend-scissor):
    (1) one loft per connected run; (2) chain_ordered: a later run starts at its free end behind its first edge
    (the slanted wall had split into 2 runs at its top corner: 3 lofts); (3) frame_at: no probe inside -> the face's
    single side + outward normal (wedge at the slanted wall's bottom). Now 2 lofts (5.84 + 4.46 mm³), error in 6.6 s
    (main 18.9): the Cut gives 5 solids. Unit test blend_section::tests::a_later_run_listed_from_its_middle_starts_at_its_free_end.
    Debug ZZCHAIN / ZZ_SBDUMP (dir: body.brep, cut*.brep) / ZZSOLID prints in blend-sc: strip before landing.
    bin/sc-build.sh (build cli-sc vibex7 + run -> logs/sc-run.out). Tangent propagation: either face gives all 16 edges.
  - 9cbb0e Fillet 23, 665cf3 Fillet 4: inputs differ upstream (asm bisecting; 9cbb0e derives from 2d82a0, so Fillet
    13 is upstream of it).
- KNIFEHANDLE Fillet 1: core (11-21 ~20:50) queued 8061866b (branch core-kh): Extend handle 1/2/3 as live, Handle
    204640 mm³ (live 205332.7; rest is P4's ellipse radius, with skasm). Fillet 1 is mine once it lands.
- isaac-ball a04711 Chamfer 1 / Fillet 1: wait on bool's Extrude 14 (+776.4 mm³), then re-check.
- Halter faces (parked): ZZ_NOKEPT + ZZ_INTO=0.05 builds VALID +2.333 mm3 (live 2.4694), 61/162/104 vs 39/111/73,
  108 s. Extra faces: fill lofts' planar sides not merged into body faces, 0.004 mm2 tip slivers, extra B-spline pieces.
  logs/zzh10.out (bin/zzh10.sh -> vibex7 ~/tmp/blend/halter-out.brep; bin/faces2.sh lists faces). ZZ_INTO 0.01 invalid.
- SUSPENSION cusp_staged (blend-cusp, uncommitted): refill + root chain VALID 63/146/85, 393418.75 (live 53/98/46,
  393486.60): the root chain stayed [12,21] (the 2 roots); it should run on along the cylinder/torus edges to the cusps
  (live's patch). Faces of ~/tmp/blend/susp-b2.brep / susp-out.brep: bin/faces2.sh '<paths>' 19,39.681,38.084,3.
- Next after these: Suspension root chain (item 2 below), Halter faces (parked: 61 vs 39, needs the rounds' true
  intersection at the corner), P1b/P1c.

## Earlier (11-21 15:59, thread on the Hello account)

- Landed a9c26c41c isaac Fillet 4 (bool told).
- 1. Impeller P0 (lead: Halter, then Fillet 3's speed, then SUSPENSION-CIRCPAT1).
  - Landed d2efb9fd (11-21 15:15): run-on end face first among exact ties, deferred overflow check (no-op on main; keeps
    Fillet 3 at 34/93/60 once bool's mend db41b6bec lands; bool told).
  - Halter (f29): worktree blend-halter3 = f3end + ce09aaef1 (switch to db41b6bec once landed) + corner_run WIP (7248eb349)
    + tighten_new_vertices (Fillet 3's tip vertices 2.6e-3 -> 6e-6) + corner trim (a fill's run past a corner kept only
    inside the neighbouring round, live's convex ridges): fills 1.29 / 1.29 (x 5) + 1.16 / 1.16 (tip) vs live +2.4694 total.
    Probe bin/zzh3.sh -> logs/zzh5/6.out. The x 5 fills fuse valid when cut by the body first (dv 1.2896 each); both tip
    fills fail every Boolean (Fuse, Cut, glue, fuzzy: BOPAlgo_AlertBuilderFailed); zzh6 = their tolerances + tightened
    retries. 79 s per run (live: fast).
  - 11-21 15:59: without the kept pieces (ZZ_NOKEPT) fills 0.3464 x2 + 0.8313 x2 = 2.355 (live 2.4694) but tip cells
    lose the body in the Fuse; neighbour-cut first (ZZ_KEPTCUT) 1.047 x2 + 1.05 + 1.09 (too much), tip cells again
    break the body (add 2 alone: dv -71062). Next: zzh9 (NOKEPT + corner pushed in by ZZ_INTO 0.05 / 0.01 R).
    Input brep kept: vibex7 ~/agents/blend-keep/halter/in29.brep.
  - Halter input differs upstream: our tip x 0.957, live 0.097 (live's chamfer extension after Fillet 3 12.67 mm2, ours
    12.49); the bevel (192x2, v linear across x 5.175..1.325) holds the tip line at x 1.5025 (probe zz_bevel_ext, blend-speed).
  - Fillet 3 speed (Mac 3.1 s, live 107): op_boolean 640, min_distance 389 (touches), bounding_box 298, split 226 ms.
  - asm isaac-ball Studio a04711 Chamfer 1 / Fillet 1: upstream first (Extrude 14 Info, Sketch 10 untranslated); asm told.
- 2. SUSPENSION-CIRCPAT1 (P1): asm-suspension 805e29 Fillet 3 (R2.5, 17 edges) fails. Live
  live/blend/q-susp-knuckle-f3/: 53/98/46, 393486.60 mm3. Front chain lobe-R40-root(boss R55, concave)-R40 flips
  convexity at 4 tangent cusps. Live's cusp patch (29.602 mm2) = an R2.5 air ball rolling between the R55 cylinder
  (extended down to x 17.5) and the R40 round's torus (42.5/2.5), from the crossing A (20, r57.5 @59.4 deg: the root
  round's end section) to the cusp C (17.5, r55 @43.8), where it vanishes: computed contact lengths 15.869 / 15.649 vs live
  15.877 / 15.651. So: convex rounds first (cusps as ends), the R40 round cut back by the R55 cylinder (notch under the
  boss refilled), then the concave root chain on that body (ball rolls plane -> torus, tangent across A). Probe
  crash_inputs::zz_susp stages (blend-speed, bin/zzsusp.sh -> logs/zzsusp1.out; breps vibex7 ~/tmp/blend/susp-b*.brep).
  Back patches (4 x 25.287 mm2) from OCCT. Notes blend-scratch/susp/notes.md.
- 2b. KNIFEHANDLE Fillet 1 (lead, after the current step; unblocks core's land): c2-surf-workshop-knifehandle on
  origin/core-knifehandle: 1 mm conic (rho 0.5) on the cap face's closed edge; our 64-section closed loft valid, the
  Boolean invalid, 2.3 s (live 6/10/7, 204 ms). Worktree blend-kh; bin/khf1.sh [host] -> logs/khf1.out (dumps vibex7
  ~/tmp/blend/khdump).
- 3. P1b/P1c time limits -> work counts (worktree blend-p1c + p1b/p1c-wip.patch; notes-p1b.md).
- 4. SPEED-MUTUALTRIM-FILLET1 (low). 5. c2-opt-fillet-partial-asym-smooth Fillet 2. 6. P2
  c2-curve-pattern-composite Fillet 2. P2 FILLET-FAIL-SLOW.
- Probe/trace worktrees (never land): blend-speed (c44c0240d + f3time2.patch + zz_susp/zz_faces + BODYDUMP),
  blend-f3time. Cleanup: those, blend-p1c, blend-f850 (uncommitted features.cpp; f850/f3geo/f3end targets removed); vibex7 ~/tmp/blend/{impdump,imd,bd}, ~/agents/blend-speed target; Mac
  blend-models, blend-mtdump; Mac blend10 back to branch blend-p1c-wip.
- Agent ids: only in /root/lead-scratch/roster.md; read it before every message_agent, never copy ids here.

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

1. Done 11-18/19 (history in the archive): full round, impeller Fillet 4 + speed, Fillet 3 speed d6f4e8a8d (gating),
   welding Fillet 5 b47029395 (gating). Fillet 3 2x needs the direct build of the conic rounds' ridge (parked, lead).
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
