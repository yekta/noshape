# bool: current state

Owner: bool. noshape's own Boolean (crates/brep, kernel cpp rust_boolean.cpp / boolean.cpp), the Shell builder
(by_quadrics, shell.cpp), the speed of Booleans, Shell and Split. History: docs/handoff/archive/bool.md. Standing
rulings: docs/DECISIONS.md (wins over anything here). bool2's notes: docs/handoff/archive/bool2.md.

Updated 11-19 16:45 UTC (new bool thread on the Motile account since 03:5x). Hosts (lead 11-18/19): builds + probes on the Mac (NOSHAPE_RB_HOST=mac; corpus replays / A-Bs
there only inside an rb.sh slot, else vibex8; Mac paths ~/noshape-host/...), gates on the least-loaded host (vibex8 now;
NOSHAPE_GATE_HOST=vibex8 via /root/bool-scratch/gate-chain-own.sh <sha> as a systemd-run unit; off 02:00-04:00);
land `NOSHAPE_LAND_QUEUE_HOST=vibe2 tools/tiers/land.sh`; private docs vibex7 ~/lead-private (hat8, hat15, hat15b,
AppleWatchDock). Never kill a process tree from a computed pid: `systemctl stop <unit>`; kill only own pids > 1.

## Ruling 11-18 (DECISIONS 9e5b481d4) and order
Our Boolean lands on correctness (identical on corpora + hats/dock); speed counts only >= 100 ms and >= 2x per user
action / document (no ms A/Bs). Order (lead): 1. decline reasons ranked (posted 18:1x, below); 2. work them down: bool
takes Shell + offset (shfall), bool2 free-form sections + B-spline faces; 3. impeller (37.8 s vs live 2.6 s): Extrude
Bohrungssitz fuse (d04c232c8) and Shell Gehaeuse. Daily report: share taken (count) + differing cases.
Ranking on the stack (Mac replay-sph, summed over corpora, so corpus2-6 repeats count again): not tried 212,
non-manifold 42, short section from a corner 30, near two vertices 21, inside-out tool 17, tiny arc 12, piece's point on
another solid's face 12, fuzzy / mesh 11, face's vertex off its plane 11 (7 shfall). Outside shfall and bool2's shared
shape, every reason is now 1-4 distinct cases. Looked at 11-19 and parked (diffs in /root/bool-scratch):
- Shell slabs built tight (parked-shell-tight-slabs.diff: ShapeFix_ShapeTolerance to Confusion, each edge back to
  1.05x its CheckSameParameter deviation, restored when invalid; on slab tools and the fused tool): on the 211-shell
  fuzz corpus (/root/core-scratch/shellcorpus, Mac bool-keep/shellcorpus, /root/bool-scratch/shc-ab.sh) Shell's own
  results change in 13 (4 failures now built, 2 built ones now fail, 1 kernel crash bf37be35, 6 volumes / face counts),
  ours takes 108/156 vs 103/159. Not landable. shfall's 93 are recordings: no source change moves them.
- Short section from a corner, 30 (parity56; parked-stub-corner.diff merges the corner and the branch point, group
  centred on the corner): 7 taken but differing (5 history, 2 with 3 more edges), 15 to "two edges leave a vertex in
  one direction", 7 "seam off a ruling". The section runs along the tool's closed seam edge (tangent to it at the
  branch): tangent contact, OCCT's (ruling 11-15).
- Non-manifold (15 distinct outside shfall; nm-trace.diff, DBGNM): 6 have OCCT results non-manifold too (declined on
  purpose); cdd336b4 a box corner at a sphere's centre (the loop passes through the pole: free-form path, bool2's);
  4a71ecd0 / 62565707 a 31 um tolerant tool only touching; 301238ec / 23a4e56c tools' corners exactly on the part's
  wall with tori around a horizontal axis; 9f262529 / b0c36850 1e-5 tolerant mirror twins (volumes 3e-5 apart).
- "a line and a curved edge" (16) went with bool2's 4b68bdb0b; "sheets that meet" bool-corpus ones are on the stack.
live (e8549719) captures at feature level ("bool truth (11-18)" in behaviours.md): sent impeller Bohrungssitz /
Gehaeuse / Fillet 3, nuts Sweep 1; shfall / gpf / sp-cut differences are internal Booleans (no live analogue).

## In flight
- Share (DECISIONS 15c2c6406, report this way): ours 3121/3477 (89.8%: general Boolean 3026, direct builds 95), OCCT
  356, differing 6 (main 548d795fb's gate replay; local = OCCT on fewer faces, counted as OCCT).
- bool-stack7 (bool-hat, 34 commits on main 0bfc1ed03, tip 50e41e425 = gated 1c8b8f4cf + a test-only type alias for
  clippy) = bool-stack6 without c98c4480b + 6ee804b74: the lies_on change (`other_points` instead of no answer) gave
  cf-sheet-metal-model-plus Extrude 4 70/165/96 vs live's 70/163/94 (bool-stack6's gate FAIL 11:03, recorded case
  1df82433e735264f; with NOSHAPE_BOOL_RUST=0 14/14 match). Without them c2-wire-clip-imported Boolean 2 and both
  wire-clip cases decline as on main (win back later with a narrower lies_on rule). Kernel 516 + brep pass (Mac).
  GATE unit bool-gate-1c8b8f4cf-x8 (vibex8, 13:02) -> /root/bool-hat-scratch/chain-1c8b8f4cf.out; private docs
  bool-udoc-1c8b8f4cf -> udoc-1c8b8f4cf.out. Both clean -> land (lead: arm it and say so in the same turn); then tell
  dx / core / blend / asm / bool2. bool-stack6's gate replay: taken 3044 -> 3139 (+95 -0), differing 6 -> 6.
- bool-pole (on bool-stack7, 6 commits; kernel 518 + brep pass on the Mac before the last two; own gate (with the
  Shell check) + udoc after stack7 lands):
  e4f8a8066 a sphere's pole edge split at the curves ending there (boolean.rs apex_parts for Nurbs, Geo::Apex
  a0/a1; rust_boolean.cpp pole pieces): non-manifold cdd336b4 taken = OCCT (T1 a_spheres_octant_through_its_pole).
  01775a048 new straight edges continuing one another through a vertex share one Geom_Line (rust_boolean.cpp
  row_line), so the unify joins them as OCCT's do (2ea4 / a6c8; T1 a_rulings_pieces_unify_into_one_edge).
  fb3d29a2a Face Generated Vertex for every argument's coplanar face (not only the first's): 0 new differences on
  every corpus (Mac A/B), several equivalents exact (DECISIONS 0e603d4c9: history-only fixed, not declined).
  4b1784e6c the 50 um rule (lead 11-19, DECISIONS 0e603d4c9): a Shell slab Boolean (quick_boolean, t_slab; the
  replay sets NOSHAPE_BOOL_SLAB for path direct*) with an input edge >= 5e-5 tolerance is OCCT's: removes 05d2 /
  11f89 / 219f / 978138 / 9c8c (differing). It drops, TO WIN BACK with a narrower signal: gate corpora 0670c350,
  280af0bc, 2b9c38da, 2c9c42e4 (shfall), 30afb30c, 475df60e, 570dfc37 (shfall), 5fc0970a, 70b3fb42 (shfall),
  818552c9, 83b621a3 (shfall), 8e91c517, 922c847a, 952c2efc (shfall), a1186078, a7bdd15b (shfall), b359e025,
  cb7bb22b, e1dc8c98, f1c35250 (shfall), f302ef40; Shell corpus (correct ones) 1451f7cb, 43a947f0, 96d45f42,
  9bee135f, c25f7a31.
- Shell end to end (LEAD 11-19, DECISIONS-level): Shell's results with ours vs OCCT's Boolean alone over core's 211
  fuzz shells (Mac bool-keep/shellcorpus; hosts ~/agents/bool-corpora/shellcorpus/cases), history compared by place:
  main differs in 10, bool-pole in 9 (13186b07 b64c8b4e 9134b29f 8eebe1e2: +1 face; f815832c: +1 face, volume 5e-4;
  8e657637: +26 edges; b79e508b: -1 edge; 17488d26: volume 5e-5; 99b55ec9: volume 2e-8). The replay hid them (each
  Boolean gets OCCT's inputs). Lead: keep the slab path (>= 100 ms on 17 shells, up to 18x); the gate now runs this
  check (bool-pole 0d465cb70: gate-host.py SHELLS / SHELLS_KNOWN, fails on a new one; take fixed ones off the list);
  get the 9 below 5 by 11-20 18:00 (else tell the lead), ranked by visible difference. Per shell, the Boolean that
  differs (record one shell with ours on: NOSHAPE_RECORD_BOOLEANS, then replay NOSHAPE_BREP_VS_OCCT=1):
  - f815832c: Common 9e2bf7c0 (also on OCCT's inputs): ours 2.719411e-6 vs OCCT 2.720903e-6 m3, 59 vs 58 faces.
    xprobe probe_who_is_right (grid points where the two disagree, against the arguments): ours right at 7 of 8.
    Likely OCCT's error (no live capture for a fuzz shell): decision for the lead.
  - 17488d26 (and Shell-recording 7724 / 86b50d59): a 36-slab union; coincident cylinder faces arg9 Face4 / arg24
    Face2 (r 6 mm, z 0..1 mm about (14, 20)): OCCT says arg9 Face4 Deleted, ours Modified to both halves (and arg14
    Face0 Modified to one). Shell reads face records: real. Next: OCCT's history for a non-representative same-domain
    face (BOPAlgo_Builder_2.cxx FillSameDomainFaces: representative = lowest-index original face, else a piece of
    the lowest-index face; ours: first argument's, last argument's in a fuse of 4+).
  - Unify family (slab union same as OCCT's, unified otherwise): b79e508b = 4dc6b1d4 (a ruling split at z 19.8 in
    OCCT: two face pairs' sections, ours one line), da3ee477 / 71ca (an x-axis cylinder's pieces on another
    argument's surface), f97ca580 (an extra seam split at x 34: pieces on two arguments' cylinders), 3b56cb83,
    75780985. Tried and dropped: lines shared per section only (2ea4 / a6c8 regress, 4dc6 unchanged); first
    argument's piece always (fixes 71ca / d252 / ffa2, breaks 2ea4 / 47763 / bd8e); OCCT's representative rule
    literally (56 commons' history differ); declining coincident cylinders (109 of 159).
  - 99b55ec9: Common df5e46e1 equivalent (1.7e-8 relative volume) but the Shell's volume string differs.
  Mac scripts: /root/bool-scratch/shc-e2e2.sh <bin> <runs> (occtC = OCCT baseline), mac-shell-env.sh <bin> <ENV..>
  (Shell recordings + 9-shell chain + end to end per env), mac-ab-vs.sh <bin> <base-tag> [ENV]. Probes:
  xprobe probe_who_is_right, probe_faces_on_plane, probe_edges_on_line, probe_unified_edges.
- History-only (LEAD 11-19): a Shell's slab union (recorded path *_slab_union, quick_boolean's slab_union) whose
  history differs only in input vertices' records counts as equivalent in the replay (Shell reads only its faces'
  records; bool-pole afcf858f3; brep_replay.rs vertex_records_only). Everywhere else history differences count.
  Re-recorded Shell Booleans (Mac shrec-slabu, 155): 94 taken, 9 differ (the unify family 6, 7724's face, f9b2
  (bool2's rim fix gating), 9e2bf7c0). 2265 / 3130 / 85ac / ffa2 now equivalent; the rounding decline A/B (41 Shell
  cases, ~25 per corpus2-6) is parked: /root/bool-scratch/parked-rnd-decline.diff.
- Speed list (lead 11-19, after stack7 / 50 um / history-only): pulleys Circular pattern 1 152 ms vs live 23 (doc 238
  vs 102), the time in the Boolean (core confirmed).
- Split with bool2 (lead 11-19): mine non-manifold (worked through: cdd336b4 fixed; the rest touching, tolerant,
  invalid or OCCT's own non-manifold results) + the Shell group; bool2's two edges leave a vertex, edge twice in a
  face, tiny arc and the 1-3 case tail. Tell each other before a new reason. Parked for both: short section from a
  corner. Parked (lead): 129100d95, 846db3d0c, 13b8c4b8d's same-surface merge (no live capture).
- Uncommitted, kept out of commits by rule: bool-hat's crates/kernel/tests/all.rs probe line and untracked
  xprobe.rs (PROBE_SURF / PROBE_BOX / PROBE_HDUMP) / sampler.rs. Scratch diffs: traces
  /root/bool-scratch/stack3-traces5.diff (DBGNM / DBGP / DBGF / DBGTE / DBGLOST, env-gated), pair-tolerance touch
  tests /root/bool-scratch/touch-dbg.diff (882bad, parked), tolerant-end split /root/bool-scratch/tilt-split.diff
  (parked).
- Mac rule (lead 11-19): corpus replays and A/Bs on the Mac only inside an rb.sh slot (`NOSHAPE_RB_HOST=mac
  tools/tiers/rb.sh bash -c 'cd ~/noshape-host/bool-keep; ...'`; the Mac runs at most two jobs), else on vibex8. Mac
  paths ~/noshape-host/... (no ~/agents there). vibex7: nothing new 02:00-04:00 (qa's quiet window).
- The OCCT cases left on bool-stack3's tip (Mac replay-bnd, distinct, not tried excluded): short section from a corner
  30 (parity56, bool2), non-manifold 30 (15 shfall), near two vertices 19 (17 shfall), inside-out tool 15 (12 shfall),
  piece's point on another solid's face 12 (shfall), face lost 11 (10 shfall), edge on a free-form face 10 (shfall),
  two edges leave a vertex 8, face's vertex off its plane 7, sheets that meet 6, coincident pieces 6, edge twice in a
  face 5 (input slits), the rest <= 5 each. Shared shape: 3 (sheets) left after the stack.
- shfall (122 declined on the tip; lead: keep in the share, one group): recorded 11-02 03:12 from core's CLI Shell
  fuzz (seeds 3000-3029, vibe4, gone), before dcb40cd48 (03:38: slab tools' UpdateTolerances, which lowers faces
  only). 93 have edges >= 10 um: slabs at OCCT's nominal 1e-4 / 1e-5 (48 slab fuses) and fused slabs fed to the
  commons (42). Source test (scratch /root/bool-scratch/retol-slabs-hook.diff: each slab edge's tolerance set to its
  measured deviation, CheckSameParameter, then vertices updated, kept if valid; probe_retolerance_cases re-records the
  93 into Mac ~/noshape-host/bool-keep/shfall-retol): 12 of 92 taken (8 history only, 2 history differs, 2 volume
  differ: 934181cc OCCT 4.1e-6 vs ours 2.7e-5 m3, a01bc6de 2e-5 relative), the rest decline for other reasons
  (non-manifold 13, inside-out tool 9: fused-slab tools with negative volume (OCCT fuse artifacts) and one inverted
  slab 395715b6, face lost 7, ...). Not one lever. Tight (< 10 um) 29: /root/bool-hat-scratch/shfall-tight.txt.
- two edges leave a vertex: parity56 2ebbd509 / f3936233 are tangent contacts (a tool circle's 25 um arc leaves a
  vertex down a cylinder's ruling): OCCT's (ruling 11-15). 11cc92b2 / dbc28f46 / f516559d (tolerant end vertex 8.7
  um, 2.6 um off): the end-tolerance split reach (tilt-split.diff) moves them to non-manifold / vertex off its plane,
  none taken (Mac A/B 04:4x): parked.
- Looked at and left (11-19): voids in a result of several solids (shfall 29b409 ...: boxes can't tell the holder,
  needs a point-in-piece ray test; /root/bool-scratch/parked-void-boxes.diff); corpus4 63bd60d0 / 91923f72 /
  b70f339a / fda1114a (a 0.1 nm radius circle at a fillet / chamfer corner cone's top: micro on the plane, apex on
  the cone needed; /root/bool-scratch/parked-tiny-arc-micro.diff moves them to "a face lost"); 734a8782 (a 10 mm
  circle touching a 15 mm cylinder inside: tangent), cones meeting at an apex 2 (point contact), inside-out tool 3
  (input defect), internal face 2 (INTERNAL input faces), parity56 89ceca45 (sheet cut whose tools lie on the
  object's extrusion surface: the sheet-cut path knows planes only; 1 case).
- qa's speed backlog (TICKETS a3ff6d481): standoffs Revolve 2 / Boolean 1 / Boolean 2 (OCCT 450-980 ms each, ours
  declines "a piece's point on another solid's face"), wire-clip / gamma Boolean 1 (87-tool fuse, ours "cylinders
  nearly coincident", OCCT 2.0-2.9 s) and Boolean 2 (23 tools, "piece's point on another solid's face"). Cases Mac
  ~/noshape-host/bool-keep/rec-<doc>; trace /root/bool-scratch/dbg-near-on.diff (DBG_ON / DBG_NEAR2, scratch only).
- parting-line Split 3: skasm's fix efd277131 landed (Draft 1 OK again): next.
- core's nut thread cut (c2-config-parts-nuts Sweep 1, vibex8 ~/agents/core-keep/nuts/nutcut{0,}.bin, Mac
  ~/noshape-host/bool-keep/nuts): the thread's crest lies on the bore (coincident, same side): ours declines, OCCT's
  wrong ((a) default options 121.40 mm3 valid, below object - tool = 131.99, main the same; (b) 157.7 invalid). Asked
  core 07:40 for a tool whose crest reaches into the hole; then take the cut, check live's 135.268 mm3 / 29 faces.
- c2-fan-blade-sweep Boolean 2: bool2 takes it into ours (lead 11-18; bool2 in brep boolean.rs / free.rs only); when it
  does, I narrow or remove offset_grazes_a_circle (check rings-loft, sp-1113).
- P0 c2-fan-blade-sweep Boolean 2: our attempt fixed in 13. OCCT's 186 ms (Mac) is offset-surface evaluation; B-spline
  fits of the offsets gave -8 % only (dropped); core works the offset side. Case Mac ~/noshape-host/bool-keep/fan.
- Scratch in bool-hat (not to commit): crates/kernel/tests/all.rs (xprobe), xprobe.rs, .*.sh; the fan DBG_NM diff in
  /root/bool-scratch/dbg-nm-fan.diff. bool-valid: debug branch, reset to origin/main after.
- DECISION ASKED (lead, 22:00): every Boolean's check as made_valid (made / changed faces + shells) instead of
  BRepCheck of the whole result: pulleys' 5-tool cut 13.1 -> 6.2 ms (OCCT 8.2), 4-tool 7.1 -> 3.7.
- Validity in the gate: tools/bool/gate-host.py (landed 3b8bfba0) reads blend's NOSHAPE_PARITY_VALIDITY CHECK lines
  (cfd943d5) and fails on a part valid on main and invalid on the branch.
- blend: c2-impeller-loft Fillet 3's OCCT cut 9602f671f8528c1a (~650-800 ms, PerformEF 481): vibex7
  ~/agents/blend-keep/rec-f3, Mac ~/noshape-host/agents/blend10/rec-f3.
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
