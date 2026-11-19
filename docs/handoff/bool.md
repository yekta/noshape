# bool: current state

Owner: bool. noshape's own Boolean (crates/brep, kernel cpp rust_boolean.cpp / boolean.cpp), the Shell builder
(by_quadrics, shell.cpp), the speed of Booleans, Shell and Split. History: docs/handoff/archive/bool.md. Standing
rulings: docs/DECISIONS.md (wins over anything here). bool2's notes: docs/handoff/archive/bool2.md.

Updated 11-19 02:58 UTC. Hosts (lead 11-18/19): builds + probes on the Mac (NOSHAPE_RB_HOST=mac; corpus replays / A-Bs
there only inside an rb.sh slot, else vibex8; Mac paths ~/noshape-host/...), gates on vibex7
(NOSHAPE_GATE_HOST=vibex7 via /root/bool-scratch/gate-chain-own.sh <sha> as a systemd-run unit; off 02:00-04:00);
land `NOSHAPE_LAND_QUEUE_HOST=vibe2 tools/tiers/land.sh`; private docs vibex7 ~/lead-private (hat8, hat15, hat15b,
AppleWatchDock). Never kill a process tree from a computed pid: `systemctl stop <unit>`; kill only own pids > 1.

## Ruling 11-18 (DECISIONS 9e5b481d4) and order
Our Boolean lands on correctness (identical on corpora + hats/dock); speed counts only >= 100 ms and >= 2x per user
action / document (no ms A/Bs). Order (lead): 1. decline reasons ranked (posted 18:1x, below); 2. work them down: bool
takes Shell + offset (shfall), bool2 free-form sections + B-spline faces; 3. impeller (37.8 s vs live 2.6 s): Extrude
Bohrungssitz fuse (d04c232c8) and Shell Gehaeuse. Daily report: share taken (count) + differing cases.
Ranking (replay 118bf5b52eaa636b, 3477 distinct: same 2926, declined 378, differ 6, equivalent 119, relabelled 46):
not tried 120 (95 our own direct builds: on_face 67, through_holes 24; local 23, slabs 2), non-manifold 33 (shfall 14),
short section from a corner 30, point near two vertices 19 (shfall 17), sheets that meet 17, line + curved edge 16,
inside-out tool 15 (shfall 12), a face lost 14 (shfall 10), coincident pieces differ 11, piece's point on another
solid's face 11 (shfall 11), two edges leave a vertex 10, edge on another argument's free-form face 10 (shfall 10).
shfall's "near two vertices" 17: fuses of a slab + 84-93 tools (most OCCT results invalid) and 6 commons; probing with
a DBG_NEAR2 trace (scratch) in Pool::find_tol / boolean_with's 4th round.
live (e8549719) captures at feature level ("bool truth (11-18)" in behaviours.md): sent impeller Bohrungssitz /
Gehaeuse / Fillet 3, nuts Sweep 1; shfall / gpf / sp-cut differences are internal Booleans (no live analogue).

## In flight
- Share (DECISIONS 15c2c6406, report this way): ours 3121/3477 (89.8%: general Boolean 3026, direct builds 95), OCCT
  356, differing 6 (main 548d795fb's gate replay; local = OCCT on fewer faces, counted as OCCT).
- LANDING: the fold fix alone (spur-gear P0, qa's bisect on afe2cbe90): bool-p0 cherry-picks 9301d9bc3 + fb602bac0
  on main ce88e6672, QUEUED bd009a26 (lead 02:45: on the vibex7 gate's evidence, reasoning in the commit body). On
  land: send qa the main sha (re-time vs v0.4.58).
- GATE unit bool-gate-2c6b920fc on vibex8 (02:43, slot queue) -> /root/bool-hat-scratch/chain-2c6b920fc.out: branch
  bool-stack3 (bool-hat) = main ce88e6672 + fold fix + 34 commits: the old stack without the two reorder commits
  (87df02e54 local after ours, bf73854d9 on_face / through_holes after ours: same geometry, slower (on_face 2.2 vs
  ours 19.5 ms), history differs from the recorded direct builds; lead 02:20 agreed to drop them) and without
  bb10c3fe5 (main has an offset-fit cache already); wire-clip layers (WIP prefixes removed; lies_on, narrow strip,
  coaxial cylinders, seam arc, cylinder other points, fuse strip covered: 6f38f as live, exception in the wire-clip
  corpus's `live` file (live/bool/wire-clip-sliver.txt), corpus added to gate-host.py); and 4 new: ±π vertex
  directions (arrange.rs; corpus2 553beda0 in 6 corpora, corpus5 6a8804ab, 4 shfall), cone apex places within tol
  (corpus4 e72bf433), line commons past distant B-spline edges / faces (16 bool-corpus), plane vs curved sheets'
  common empty (11 bool-corpus). Mac A/B of the ±π fix: no new differences. Land when green (lead).
  The failed vibex7 gate of a6a83073f (the old stack): /root/bool-tube-scratch/gate-a6a83073f.log.
- The 356 by reason (main 548d): shared shape 74 (the stack's e9b6efecc / 8a9351e4b take 46), non-manifold 31
  (mine done: 2 fixed; 7 have B-spline faces (bool2); 4 truly touch along an edge (a cylinder tangent inside a
  face, edge-to-edge fuses), 2 an invalid tool), short section from a corner 30 (bool2), near two vertices 19
  (shfall 17), sheets that meet 17 (11 fixed; 5 shfall, parity56 89ceca45 extrusion), line + curved edge 16 (fixed),
  inside-out tool 15 (shfall 12), face lost 14, two edges leave a vertex one way 7: 11cc92b2 / dbc28f46 / 2ebbd509 /
  f3936233 / f516559d: a tolerant input vertex (2.6 um off its face) tilts the edge piece from it, so a coincident
  tool edge's vertex on the true line is out of split reach; fix idea: split_params measures against the origin
  edge's own line. Edge twice in a face 5 (slit faces, input). Traces: /root/bool-scratch/stack3-traces3.diff
  (DBGNM / DBGP / DBGF / DBGTE).
- shfall (129 declines): no parity-suite Shell reaches slabs now (34 Shells: quadrics 20, smooth 8, regions 6), and
  16 of its 17 "near two vertices" have input vertex tolerances 0.2 mm..1 m: low value. Its 11 "piece's point on
  another solid's face": zero-area input faces (two coincident edges), a 0.2 mm triangle tangent to a cylinder.
- qa's speed backlog (TICKETS a3ff6d481): standoffs Revolve 2 / Boolean 1 / Boolean 2 (OCCT 450-980 ms each, ours
  declines "a piece's point on another solid's face"), wire-clip / gamma Boolean 1 (87-tool fuse, ours "cylinders
  nearly coincident", OCCT 2.0-2.9 s) and Boolean 2 (23 tools, "piece's point on another solid's face"). Cases Mac
  ~/noshape-host/bool-keep/rec-<doc>; trace /root/bool-scratch/dbg-near-on.diff (DBG_ON / DBG_NEAR2, scratch only).
- Earlier gates of the stack: /root/bool-hat-scratch/chain-{4cf4ce0ca,8086f188b}.out. bool-hat rebased on main 1e21f28e8: the 13 below +
  14. Shell race fix (on main as 7b8bb249) + 15. param_of cache + 16. Shell add_pcurve analysis cache + 17. offset fit
  cache + 18. volume_possible's cut lower bound + 19. project_along tangent-corner sliver (britatap) + 20. 4cf4ce0ca
  Shell foot: bounded_foot within two walls before OCCT's global search (impeller Shell Gehäuse 199 -> 183 ms Mac). And
  + 14. 05e180ea5 the Shell race fix (as 7378bd2c) + 15. 4d499486d NurbsCurve::param_of keeps its sample points per span
  run (rings-loft 51-slot cut 331fd52c 246 -> 211 ms Mac, same result) + 16. c0a72a695 Shell's add_pcurve keeps a
  surface's ShapeAnalysis_Surface per thread (impeller Shell Gehäuse 235 -> 199 ms, same result). Then private docs
  (udoc-ab8.sh c0a72a695), list relabelled / seam cases per condition (c), knurls ours-only, land, tell dx (2108c2b7-2f48-43a7-87a0-3f652e3cda6d),
  core (d30002ca-d013-4435-b7d9-7da9cb564cec since 11-18 ~11:00), blend (de66e481-56e2-4005-a5d3-9fa897ca5fde), asm. Branch bool-shared2 (bool-hat) on main f6274d94e, 13 commits, all g++-13 OK:
  17. offset_as_bspline keeps its last 32 fits (impeller Boolean 2 55 -> 35, Boolean 3 41 -> 21 ms Mac; doc's fits
  587 -> 330 ms; kernel T1 496/496).
  1-10. as before: e61278b1d usd room; 8a9351e4b shared shapes (lead conditions (a) suite identical, (b) hats/dock
     SAME, (c) replay label-only by case, (d) split_names T1); a1d7ca0b4 + 65befe285 NURBS piece boxes; 51ee7c1cb unify
     room order; 0b17fee66 par_map_by cancel; 26ce2dffa fuse_glued mirror normal; b11d72be6 gap() (pulleys seam
     decline); 29bcd92a8 pattern clearance once + seam_cuts hull spans; 07ce00475 moved curves keep span lengths;
     934af8abb located mirror images not fused glued.
  11. 26dcc8ec7 (was f88eb782e) a04711 Extrude 4: non-manifold cut retried on text-round-tripped copies (lead 11-18:
     land; commit body names live's JT6 sliver, P3 A04711-TOOL-SLIVER in TICKETS).
  12. 8ba7c6dcc strips (lead 11-18: match live's 1e-5): drop_strip_faces / collapse_short_edges / without_strips on
     every op_boolean path; unify keeps merges whose pieces lie on the merged surface (pieces_within, 1e-7) within 1e-8
     of the volume. e54410 Extrude 22 14 faces as live; 850efe Extrude 5 36 as live, doc 300 s -> 1.7 s, 12/18.
     Not matched: q-union-merge's overlap variant (face coincidence, not a strip; ours 8/18/12 tops 90/100, live
     8/22/16 tops 100/90). OCCT fuzzy 1e-5 gives live's faces there, but over the corpora (5873 cases) parallel
     planes 1e-7..1e-5 apart overlapping occur in 7 only, and fuzzy there drops faces (2), leaves 7 faces unmerged
     (3) or empties a parity56 cut (ce4e3c99): not taken. Detector: /root/bool-scratch/near-planes-detector.diff. 850efe Extrude 6 33 vs 28 (live
     merges with 2e-6 of volume change: not ours to take). Kernel T1 496/496 on the Mac.
  13. b5b566884 fan-blade P0: an offset face grazing another argument's circle declined first (offset_grazes_a_circle,
     rust_boolean.cpp): 36 -> 0.4 ms on the Mac; not where one of its edges lies on the circle (impeller halves).
  cf-surface-pattern 385 vs 386 edges: its Booleans identical branch vs main (the gate's same-base pair decides).
- a04711 notes: Part 3 = live's faces / volume / area but 8/17/11 vs 9/22/15 (live's 10 um JT6 sliver; the rounded
  retry's other firings, Extrude 23 / 27, are rejected and stay errors). The doc's 19.9 s vs live 1.93: fillets on
  parts off from Extrude 1 (asm's). Repro vibex8 ~/agents/asm-keep/ma04; dumps ~/tmp/bool/a04-box.log.
- Re-timed on the Mac vs live's rbstep-1118 (lead 12:30: qa's list first): mine > 5x: impeller Extrude Bohrungssitz
  Ansaugflansch (op_boolean 1050 of 1115 vs live 154.5: fuse 87647d5a, 9 ring bosses on the 48-face B-spline housing,
  glue shift_checked; ours "a piece's point on another solid's face" (coincident faces), OCCT 930 ms of EF on the big
  B-spline faces; OBB no help), Fillet 3's cut ed9c35dc (750 ms; ours "an edge on another argument's free-form face"),
  Chamfer Stevenrohr (op_boolean 156 vs 16), fuse 23659e13 (340 ms; ours "a face lost"). local_boolean would take them
  (only the faces the tools reach) but needs: < 200-face targets with free-form faces, several tools, reach tighter
  than face boxes (no(5): the B-spline faces' boxes reach > 1/4). Knife Split 4 5x: project_along's plane sections
  (OCCT 5-10 ms each, half ComputeTolReached3d PSO); the marcher declines the grind face (grid > 257 a way): parked
  /root/bool-scratch/parked-own-plane-section.diff (identical results where it ran). Cases Mac ~/noshape-host/bool-keep/{imprec,kdump-p}.
- 11-18 13:4x impeller rest: Fillet 3's cut ed9c35dc (OCCT 680 of 720 ms): 4 EF pairs of the housing's 141/144 mm
  B-spline edges against each tool's offset face lying on the housing's offset surface (geometrically, not the same
  handle or basis): 135-150 ms each, no common part; ours declines the same coincidence. Told blend (tool oversize past
  F2) ; our side would be coincident free-form faces in brep (big). 23659e13 (fuse, "a face lost"): ours 57 + OCCT 220;
  e14e43ba (cut): ours 45 + OCCT 62. Scratch probes: xprobe probe_case_reach (box vs true contacts), PROBE_HIST /
  PROBE_GLUE_OFF in probe_case_timed; Mac A/B binaries ~/noshape-host/bool-keep/bin/{base,new}, ab.sh, one.sh.
- parting-line Split 3: skasm's fix efd277131 landed (Draft 1 OK again): next.
- qa 11-18 (5x live): impeller Shell Gehäuse (Mac 199 after 16.; left: CheckerSI 26 %, quad::foot / ProjectPointOnSurf
  Extrema grid scans per point, meshed copies 12 %), Flansch+Gehäuse 4d5e785f (ours 202 vs OCCT 3302; critical path
  one OCCT offset x extrusion section 135 ms of it, OCCT's walk + tolerance PSO), Boolean 1 (unify 66: usd.check 38 =
  made_valid decision, usd.volume 27 borderline-only), knife Split 4 (project_along 41). Cases Mac
  ~/noshape-host/bool-keep/{imprec,impdump,rlrec2}.
- live's suite-truth-1118 first differences (behaviours "core + bool + skasm (lead 11-18)"): rocker-enclosure-body
  Extrude 6 not the Boolean (ours = OCCT; the tool's hex pocket centred (-79.16, 36.16) vs live's (-77.85, 34.85): the
  sketch/placement, skasm/core); britatap fixed (19.); parting-line Split 3: the Mac stops at Draft 1 ("Select parting
  edges", same model files: platform), Linux run on vibe2 (.spx.sh); fan-blade #48 sent to bool2.
- core's nut thread cut (c2-config-parts-nuts Sweep 1, vibex8 ~/agents/core-keep/nuts/nutcut{0,}.bin, Mac
  ~/noshape-host/bool-keep/nuts): the thread's crest lies on the bore (coincident, same side): ours declines, OCCT's
  wrong ((a) default options 121.40 mm3 valid, below object - tool = 131.99, main the same; (b) 157.7 invalid). Asked
  core 07:40 for a tool whose crest reaches into the hole; then take the cut, check live's 135.268 mm3 / 29 faces.
- c2-fan-blade-sweep Boolean 2: bool2 takes it into ours (lead 11-18; bool2 in brep boolean.rs / free.rs only); when it
  does, I narrow or remove offset_grazes_a_circle (check rings-loft, sp-1113).
- P0 c2-fan-blade-sweep Boolean 2: our attempt fixed in 13. OCCT's 186 ms (Mac) is offset-surface evaluation; B-spline
  fits of the offsets gave -8 % only (dropped); core works the offset side. Case Mac ~/noshape-host/bool-keep/fan.
- 850efe: cases Mac ~/noshape-host/bool-keep/m85 (Extrude 5 110173579c7126ff, Extrude 6 cc1019ddbd364035), vibex8
  ~/tmp/bool/m85acc. Blend told of the invalid Fillet 2 earlier; with 8ba7 Fillet 2 gets live's Extrude 5.
- e54410: cases Mac ~/noshape-host/bool-keep/me5 (Extrude 22 6dc4464f40a8b0b2, Extrude 17 10c21d285eec6c3a). Live's
  merge distance: live/bool/q-union-merge (behaviours.md "bool (11-17 22:22)").
- Scratch in bool-hat (not to commit): crates/kernel/tests/all.rs (xprobe), xprobe.rs, .*.sh; the fan DBG_NM diff in
  /root/bool-scratch/dbg-nm-fan.diff. bool-valid: debug branch, reset to origin/main after.
- DECISION ASKED (lead, 22:00): every Boolean's check as made_valid (made / changed faces + shells) instead of
  BRepCheck of the whole result: pulleys' 5-tool cut 13.1 -> 6.2 ms (OCCT 8.2), 4-tool 7.1 -> 3.7.
- Validity in the gate: tools/bool/gate-host.py (landed 3b8bfba0) reads blend's NOSHAPE_PARITY_VALIDITY CHECK lines
  (cfd943d5) and fails on a part valid on main and invalid on the branch.
- asm pattern-std 36b90f Extrude 12 (17/43/28 vs live 14/36/24): not the Boolean (same with NOSHAPE_BOOL_RUST=0):
  the up-to-surface extrude's caps; told asm 11-17. Logs /root/bool-scratch/mps-51ee{,-off}.log.
- Pulleys recordings: vibex8 ~/tmp/bool/pul (6 cases), Mac ~/noshape-host/bool-keep/pul; Mac scripts in bool-hat
  .samp.sh / .t6.sh / .t7.sh (untracked).
- GearLab herringbone (dx, asm-isaac-ball 8f5622, 14.8 s vs live 1.15): fuses 726897a5 (teeth on blank, ours "two
  edges leave a vertex in one direction"; glue changes history: no) and 68f32ae1 (half + mirror, ours "a piece's point
  on another solid's face"; OCCT 949 ms, glue shift 294 identical history). fuse_glued now sees the mirror (6.) but
  the half's z=5 cap is 41 faces (core's 5e-9 rad tilted sweep caps leave teeth end faces unmerged): with core's fix
  it should go glued. ShiftChecked's volumes_add_up (1e-7) rejects B-spline solids (integration noise ~1e-5).
  Cases Mac ~/noshape-host/agents/bool-hat/gear, vibex6 ~/agents/dx-keep/gearlab/booleans.
- blend: c2-impeller-loft Fillet 3's OCCT cut 9602f671f8528c1a (~650-800 ms, PerformEF 481): vibex7
  ~/agents/blend-keep/rec-f3, Mac ~/noshape-host/agents/blend10/rec-f3.
- SCISSOR-LIFT-BOM-ROWS: culprit core's 41f62884a; core's revert ddfea0eb restores 59/87. Done on my side.
- impeller 02f2f58d (bool2's): Linux "unified differently" only; on the Mac history differs with or without my change
  (platform). 7cb79697: seam placement only.
- vibe3's ~/agents/bool-cases copy went to vibe5 (gone): lost.
- Not mine (11-17): impeller 38073a30 (bool2: live wins, OCCT's the outlier), impeller Extrude impellerraum (core).
- Landed 11-17: bool-knife (ecf8c33a), OCCT 0033 (9ece8f66), bool-knife2 (cf5a9aff), bool-march (d3f24a83), nuts
  (43335ce3: thread turn helix gap; gate clean, hats/dock/hat15b SAME), MSVC fix (01c5c13e: fitted's p static
  constexpr; CI 44039545068 green on all three OSes).
- bool-march's 4 replay cases "equivalent" (74acee2ff54886eb, e001fa8eb5dbfe24): ours the exact one (cap area x height =
  our volume exactly; OCCT's caps disagree): accepted per DECISIONS 11-15; told the lead.
- Mac 11-17 (loaded, hints): every qa-list document of mine under live (louver 1195/4987, rings 1387/1557, snap-hook
  206/274, housing 46/105, lighten 61/114, bottle 29/40, shaped-flange 455/1026); over live only features: rings Extrude 5
  71/39 (boolean.rust.brep 33: mapped-curve chords), knife Fillet 2 74-81/9 (op_split 18 OCCT EF on the grind B-spline;
  sections ahead 16), michael Fillet 2, tpd Fillet 1 (blend's). Parked: /root/bool-scratch/parked-offedge-boxes.diff
  (off_shared_edges box pre-check, identical, no gain).
- GATE GAP: the knurl corpora need a 48 GB host (none); ours-only replay instead (lead 11-17 ok):
  NOSHAPE_BOOL_CORPUS=~/agents/bool-corpora/<k> NOSHAPE_BREP_OURS_ONLY=1 <kernel tests> brep_replay::brep_corpus_replay.
- Knife cases: vibe5 ~/tmp/bool/knrec, Mac ~/noshape-host/agents/bool-hat/knrec (17b8 c1d9 Fillet 2, a3f9 Chamfer 1, 682a
  Mirror 1); knife Fillet 2's split dump Mac ~/noshape-host/agents/bool-hat/kdump2/split-11.

## Parked (branches / diffs, not landing as they are)
- Plane x B-spline marcher: un-parked 11-17, now bool-march (In flight above); its knife regression found and fixed.
- Circle-square-circle loft sides (creased at the square's corners) stall that fit in the face's parameters: why 10.
- Torus x cylinder sections: /root/bool-scratch/torus-cylinder-wip.diff (per u the tube circle against the cylinder,
  math_TrigonometricFunctionRoots, branches by Newton). michael: 5 more pairs, but two all but flat curves end
  2.5 um from OCCT's ends and the cut turns non-manifold (lead: a non-manifold cut is a no). 8 such pairs finding
  nothing still cost OCCT 75 ms.
- Offset-surface approximation cache (/root/bool-scratch/offset-cache.diff): 1-2 % CPU on cf-snap-hook's cuts: dropped.
- blend's pole-hull disjointness test: withdrawn by blend (the knife's costly empty pairs touch; tangent contacts).
- blend's known sections entry point: parked (few pairs are tangent contacts).
- Rings-loft Shell 1: skipping OCCT's global foot search where Newton from a 25 x 25 grid's local minima finds no
  square foot (/root/bool-scratch/parked-own-foot.diff): identical, only ~3 %; evidence (160/160), not a proof.
- (Handed to bool2 11-17.) Parallel-axis cone / cylinder disjointness (/root/bool-scratch/parked-cones-apart.diff: circles at each height,
  angular ranges as cosine bounds, quadratics in z): cf-snap-hook fuse 96de626280206723's 30 cone x cone pairs
  (OCCT ~30 ms each, none found) share one height where the circles are internally tangent (4e-7): only 1 proven.
  Tangent contacts stay OCCT's (lead 11-15); likewise housing's 4 cylinder x revolution pairs (20-27 ms each, none).

## Next, in order
1. qa's list (docs/handoff/qa.md "Speed"), mine: knife Split 4 (no exact lever: OCCT's plane x B-spline sections and a
   1 mm line to BRepProj), bottle Shell (566b1015) / Split (no single lever), cf-lighten and c2-sm-louver at or under
   live, CP2 and housing Rotieren 1 (bool-cp2-ahead above). Housing's other cuts: 6e0cb4e02a2efe47 (~7 ms quiet)
   is half OCCT's cylinder x cylinder walk (IntCyCy + WLApprox + PSO tolerance). Own cyl x cyl sections parked:
   /root/bool-scratch/parked-cyl-cyl.diff (quadric_across over a Ruled view) matches OCCT's curves but not its
   splits: OCCT breaks its lines at 12 critical points (CriticalPointsComputing) and re-joins some (JoinWLines:
   IsSeamOrBound, CheckArgumentsToJoin); probe cases in notes-snap.txt. Matching both exactly is the work left.
   CP2 remainder: its 8 tool cuts run in parallel (rust_fuse_trimmed), then the fuse; nothing single.
2. c2-rings-loft Shell 1 (~510-590 ms, live 235): repair 251 (traces 95, split 78, crossings 26, joins 17),
   self-interference 67 (its EF time in ~23 pairs on free-form faces; quadric pairs are cheap), edges 44, check 35,
   thin 34. Points with no square foot: OCCT's global search found none wherever a Newton search from the own 25 x 25
   grid's local minima found none (160 of 160), so the global search may be skipped there (needs the gate).
   Rings-loft profiles 11-16 15:2x (vibe5 ~/agents/bool-keep/rlb, xprobe probe_case_sample): cut 275ec41827174a35
   (51 slot tools, 488 ms) 43 % IntTools_FaceFace ImpPrm walking (plane / cylinder x rational B-spline), the main thread
   waiting on it (wait_until_cold 34 %): the plane x B-spline marcher (item 3) is the lever. Fuse 4fa72ce21506c5f7
   (8 tools, 210 ms): BRepCheck 20 % (cuts' results 8.5, the fuse's 10; the fuse could trust the cut pieces' checked
   faces as made_valid does, but `a`'s are unchecked), ::at 19 % (chords of free curves 50 % of it, piece_box 20 %).
3. The off-edge pre-check (In flight), then the knife Fillet 2 op_split and min_distance.
4. learning-center-michael (363 vs 96): cylinder x torus (22 pairs ~220 ms CPU), cylinder x sphere 24 pairs ~60 ms,
   cylinder x cylinder 10 pairs.
5. skasm's cf-sheet-metal-shaped-flange: vibe6 ~/agents/skasm-keep/sf-bool-cases (40 cases): ~15 corner-relief cuts of a
   6-face bend solid, 7-24 ms each; the part's final fuses 27-83 ms.
6. c2-rings-loft Circular pattern 1: 51 prism_until splits 254 ms (features.cpp, OCCT's Splitter; history and naming
   there: decision needed with the lead before touching).
7. core's FYI (qa's speed list 11-15, vibe7 loaded): c2-housing-draft Rotieren 1 77 ms (live 13; boolean.rust.brep 70,
   sections_ahead 41); c2-rings-loft Extrude 5 295 ms (live 39; boolean.rust 128-216, ahead 25-60); Shell 1 dump
   vibe7 ~/tmp/core/rh/dd/shell-9 (crash_inputs::replay_one).

## Measured (ours vs live, ms; vibe5 under load, candidates per DECISIONS 11-15)
c2-rings-loft 1880 / 1557; c2-sm-louver 4008 / 4987; c2-housing-draft 126 / 105; cf-lighten 158 / 114;
c2-tpd-fillet-variable 149 / 44 (blend's stack on main: 47 / 44 with the parked marcher); learning-center-michael 363 / 96;
cf-snap-hook 866 / 274 (suite run 18:5x); c2-bottle-split Split 1 ~12 ms in release.

## Open questions / decisions
- None open.

## How to run
- Worktrees: bool-split (builds via rb.sh on vibe5, target ~/agents/bool-split; kernel test binary
  target/release/deps/all-71797ea96afb1f39, ops test binary all-a79b487bf0dd0b06), bool-tube (docs / handoff),
  bool-hat (Mac builds: NOSHAPE_RB_HOST=mac). Probes parked outside the tree while branches rebase: copy
  /root/bool-scratch/probes/{xprobe,sampler}.rs into crates/kernel/tests and `git apply
  /root/bool-scratch/probes/all-rs.patch`; undo before a commit. Mac binaries: ~/noshape-host/agents/bool-hat/oc33.
- Gate: `systemd-run --unit=bool-gate-<sha> --collect /root/bool-scratch/gate-chain.sh <sha>` (gate-chain-own.sh: the
  branch's own gate.sh / gate-host.py, for changes to them; the branch's gate only;
  NOSHAPE_GATE_HOST to change host, default vibex8). Output /root/bool-hat-scratch/chain-<sha>.out (VERDICT,
  "CHAIN DONE rc N") and gate-<sha>.log; read with grep. gate.sh runs its replays through the host's ~/bin/slot
  (bool2's ff43a8c95). A crates/brep or Boolean change needs a clean gate and identical results end to end.
- Land: `NOSHAPE_LAND_QUEUE_HOST=vibe2 tools/tiers/land.sh` after
  `git diff --stat $(git merge-base HEAD origin/main) HEAD`; `tools/tiers/land.sh --status`.
- g++-13: /root/bool-scratch/gcc13-split.sh / gcc13-hat.sh <file.cpp> (name only, e.g. rust_boolean.cpp).
- A/B binaries: /root/bool-scratch/kb5.sh <worktree> <name> (kernel release test binary to vibe5 ~/tmp/bool/<name>).
- Private documents (hats, dock): vibe5 ~/agents/bool-keep/private (mode 700); compare main vs branch with
  /root/bool-hat-scratch/udoc-ab.sh <commit> (builds in bool-split, returns to its branch; copies in ~/tmp/bool/udoc, deleted after).
- Probes (xprobe.rs): probe_case_timed (PROBE_CORPUS, PROBE_CASE, PROBE_RUNS; NOSHAPE_SAMPLE_OUT samples all runs),
  probe_case_sample (PROBE_ONLY rust/occt, PROBE_SAMPLE), probe_case_unify, probe_shell_dump (PROBE_DUMP=vibe7
  ~/agents/bool-keep/rl/shell-9; NOSHAPE_SHELL_DEBUG for stage times), probe_split_dump, probe_project_dump.
- Profiles: vibe5 ~/agents/bool-keep/pu.py <log> [stack filter] [n] (inclusive and self; SAMPLE_WALL=1 logs need a
  filter such as probe_case_timed), ~/agents/profile-funcs.py; vibe7 ~/tmp/bool/c2
  pu.py, callers.py (caller chains), lines.py / lines2.py (shell.cpp stage buckets).
- Recordings: NOSHAPE_RECORD_BOOLEANS=<dir> on a parity run (ops binary, NOSHAPE_PARITY_MODELS=vibe5
  ~/tmp/bool/ab/models, NOSHAPE_PARITY_ONLY=<doc>). Corpora: vibe5 ~/agents/bool-corpora, ~/agents/bool-keep,
  ~/agents/blend-keep/known-sections (tpd 6d3bca8654a6d02d, michael 1447eaf22e0cd418), vibe7 ~/tmp/bool/c2.
- Debug switches: NOSHAPE_FREE_SECTIONS=0 / NOSHAPE_FREE_DEBUG (marcher branch), NOSHAPE_OWN_SECTIONS=0,
  NOSHAPE_PROJECT_OCCT, NOSHAPE_SPLIT_DEBUG, NOSHAPE_SHELL_DEBUG; per-pair section timer for occt_ahead:
  /root/bool-scratch/torus-and-ahead-debug.diff (NOSHAPE_AHEAD_DEBUG).
- Notes for the snap-hook work: /root/bool-scratch/notes-snap.txt.
