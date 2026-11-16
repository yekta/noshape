# Core modelling (agent "core"): current state

Updated 11-16 18:15 UTC. History (dated notes up to 11-15 14:48): docs/handoff/archive/core.md. Standing rulings:
docs/DECISIONS.md (read first; it wins over anything here).

## In flight
- Impeller Extrude 4 model side: closed, nothing to fix. The 12 mm² sliver alone never meets the offset wall: even its
  surface over all of its own bounds only grazes the prism at the sketch plane (replayed dump vibe5
  ~/agents/core-keep/e4dump/prism_until-10, experiment patch /root/core-scratch/uptowiden-exp.patch). Live's Extrude 4
  is the whole region, which we match (impeller 27/48 on main after blend's d7f9264c9; first difference Fillet 3).
- **Committed in core-oc 8deb8156c (rebased on main 18:10; with 1357fddb1 split-face attributes and 11f1335b9
  untrimmed cylinder split), all three in one release pair: a point on an elementary face is classified with its
  parameters moved by whole periods into the face's** (geom.cpp classify_periodic; g++-13 OK). tablet-stand's rolled
  wall faces cross the cylinder's seam, matched no definition piece (part_sources fell back to Face(0)), so std's SM
  Boolean never built the bend's tool: Extrude 1 = live (bool handed it back: the Boolean was right); with main's
  narrowed translator rule c2-sm-tablet-stand is 16/16 to the end.
  T1 kernel core::a_cylinder_face_across_its_seam_contains_its_points. A notch test across a rolled wall (/root/core-scratch/rolled-notch-test-wip.patch) still fails
  "Subtractive tool must cut all the way through ... rolled wall": std wants 2 cap faces per trimmed piece, ours has
  more (the caps split at the seam?).
- **Pair (six core-oc commits: 1357fddb1, 11f1335b9, 8deb8156c, 3ce19f704, 3ace68b3f, bcaf5ffdb): vibe7
  ~/agents/core-keep/sppair.sh -> spres (merge-base d29b5ee81 files in core-keep/base, branch copies core-keep/branch;
  started 18:33 UTC, the last two synced in before its build). Land all six when it shows nothing worse (save the
  probe patch first; 97b778e7d sits on top, see below: land.sh would take it too, so land the six from a branch at
  bcaf5ffdb or after a config-docs check).** (1357fddb1): opSplitFace keeps a split face's attributes on
  every piece** (host ops.rs op_split_face; Parasolid's). std Bend's imprint split the wall and dropped its WALL
  attribute, so c2-sm-rocker-enclosure-sm's Extrude 1 read "Removal destroys sheet metal part" (eachSheetStillExists
  found no WALL face): 6 -> 8/14; 14 sheet metal docs otherwise unchanged. T1
  custom_features::a_split_face_keeps_its_attribute_on_both_pieces (0 pieces tagged without). Face attributes have no
  snapshots (query.rs attribute_snapshot: edges / vertices only), so they follow a face only by its name.
- rocker-enclosure-sm Extrude 2 (next): Sketch 4's four Use circles (PROJECTED, live's external edges at x 45-60 mm,
  not on our part) come out as two: Extrude 2 names one region twice and cuts 2 holes of 4. Sketch translation:
  written into docs/handoff/skasm.md (landed 11-16).
- **Also in that pair: Split part by a cylinder face with Use trimmed off splits by the whole cylinder** (split.rs
  whole_cylinder; the pieces unified): c2-rib-wrap 4 -> 10/18 (Split 1: 2 parts, the disc 3 faces as live). T1
  features_b::an_untrimmed_cylinder_face_splits_off_its_whole_disc. Next there: Wrap 1 (3.8 mm³; its sketch text's
  ascent dimension untranslated). Land both (one commit each) when the pair shows nothing worse.
- c2-surf-workshop-britatap Touch screen split (4/22): our projected split curve (BRepProj section, a degree 7
  B-spline) ends 0.116 mm from the face's corner along the rim, where the sketch edge ends exactly; live ends it on
  the corner (2/6/5 vs ours 2/7/6). Kernel projection precision at a tangent end; parked.
- **Committed in core-oc 3ce19f704 (in the pair): paths-guides Loft 1 dome from a part's face** (features.cpp
  apex_tangent_loft read the seam's pcurves through the REVERSED side face, UpdateEdge wrote them back swapped:
  "unorientable", cone fallback) + a B-spline face's apex is a shown vertex (common.cpp shown_topology, as cone apexes):
  c2-loft-paths-guides 4 -> 8/29 (Loft 1 3/2/1 as live). T1 features_a::a_loft_from_a_parts_face_to_a_point_tangent_to_its_plane.
- **Committed 3ace68b3f (in the pair): two profiles along a straight path square to both = the plain loft**
  (features.cpp straight_path_square_to; Loft 2's path line lies past both profiles, path_sections gave up): Loft 2 =
  live (7/13/9, 5230851 vs 5230790 mm³); 8 -> 16/29. T1 features_a::a_straight_path_square_to_two_profiles_past_them_lofts_them_plainly.
  Three or more profiles on a straight path keep path_sections (unmeasured in live).
- **Committed bcaf5ffdb (in the pair): a path loft merges smoothly meeting sides** (merge_tangent_sides now also when
  `along`; live 1784's 3+-section rule): Loft 3 (two slots along an arc) 20 -> 17 faces, live 15/33/23. Left: two
  Boolean slivers (15.6 and 0.16 mm², 0.012 mm thick) along profile 1's flat side at x=-50.8, y 68-92: the path loft's
  surface there is a hair off Extrude 2's face. Likely cause: path_sections cuts each section with a plane square to
  the path; Loft 3's arc leaves profile 1 ~23 deg off its normal, so the first cut planes slant across the 197 mm slot
  and clip against the carried sweep's start: the surface near profile 1 bends into the part. Parasolid presumably
  places sections by the spine's parameter instead. Next item there.
  Mirror 1 times swing 0.3-1.7 s on loaded vibe5 runs; check on the Mac / in the pair before calling it.
  T1 features_a::a_path_loft_of_two_slots_has_one_side_face.
- c2-config-parts-links (10/35, largest gap after paths-guides): Sketch 1's solve flips its waist arcs to the other
  tangent branch when p goes 5 -> 8 mm, so the translator keeps the stored 5 mm sketch. Solver: in skasm's handoff
  11-16. Not core's.
- **Committed 97b778e7d (core-oc, NOT in the vibe7 pair): the translator builds configured documents at
  sketches.json's capturedConfiguration (the defaults live's lines are at) instead of currentConfiguration** (only the 7
  c2-config-parts docs record it; cf-sheet-metal-model-plus, without it, keeps current). Debug run of the 6 (screws
  out): every score as before (pulleys' Sketch 1 now kept; the rest stop at take-backs, in skasm's handoff). T1
  parity::a_configuration_variable_takes_the_captured_configurations_value. Land after the six, with those 6 docs as
  its pair.
- c2-surf-workshop-cpcurves Main pipe (thin sweep, 7/24): (1) Sketch 4's circle kept 18.8 mm off the path (translator
  take-back: in skasm's handoff). (2) Mine: live merges the faces swept along the path's three collinear lines into
  one per wall (6 faces; ours 10: a face per path edge). **Committed b173dc5bb (core-oc, NOT in the vibe7 pair):
  join_collinear_lines joins a sweep path's lines along one line into one edge (first piece's source)**: Main pipe's
  faces = live (only its box differs, the take-back). T1 features_a::a_sweep_along_collinear_lines_has_one_side_face.
- **Committed d2605e45d (core-oc): a pattern / Mirror with Add joins surface copies to the seeds' and merge scope's
  surfaces they touch** (bshared.rs join_surface split out of surface_result; pattern.rs, only when every copy is a
  surface; copies touching nothing keep the old push): c2-surf-workshop-singularities 2 -> 5/15 (Mirror 3 one sheet
  as live). Next there: "Loft with bad singularity" ERROR: a guided SURFACE loft from a sketch curve to a point (a
  guide's end), start Normal to profile, guides Normal to guide + Match tangent to Extrude 1's cap edge; live makes
  one singular patch (Problem loft 1/3/3). Guided loft to a point: a loft_guided item, not started.
  T1 features_a::a_mirrored_surface_added_joins_the_surface_it_meets.
- **bd8b5f74f (core-oc, on top): face_project_point classified as before** (8deb8156c's period shift there made
  c2-hunting-knife's Fillet 5 a face more, 46 -> 32/52; face_contains_point keeps the shift, tablet-stand 16/16;
  verified by debug runs). Both pairs carry the old change: vibe7's branch will show hunting-knife worse (and any other
  doc the projection touched): recheck each doc vibe7 marks worse with HEAD (.ts3.sh <doc>) before landing.
- **vibe5 pair (97b778e7d, b173dc5bb, d2605e45d) CLEAN 23:2x: 1247 -> 1250 (singularities 2 -> 5, cpcurves
  Main pipe topology = live's), nothing worse. vibe7 pair (six) still running.**
- **Landing plan:** both pairs' binaries are built; the suites run as suites-only scripts with `~/bin/slot timeout 7000`
  (the timeout no longer counts the slot wait, which killed nothing yet but would have): vibe7
  ~/agents/core-keep/spsuites.sh -> spres (sp/base = main 18:10, sp/branch = 1357fddb1..bcaf5ffdb; queued behind
  qa's suites-rest since 21:45), vibe5 ~/agents/core-keep/p2suites.sh -> p2res (p2bin/base = bcaf5ffdb,
  p2bin/branch = + 97b778e7d b173dc5bb d2605e45d; queued behind bool's gate jobs). Land all ten (with bd8b5f74f) when
  both are clean and every doc vibe7 marks worse is rechecked with HEAD.
- c2-surf-workshop-manualblend Sweep 2: Sketch 5's CONCENTRIC rejected by the translator (skasm's handoff). Not core's.
- c2-surf-intro-examples Fill 1 (5/13, parked): 4 G0 boundary edges (2 fit splines, 2 sketch splines; they match
  live's lengths). fair_g1_fill returns nothing for them, so OCCT's plate (MakeFilling) balloons: 61911 mm², box to
  x 198 mm; live 23827.4. A Coons patch (GeomFill_BSplineCurves, CoonsStyle) gives 23669.5 (-0.66 %: outside the
  band), box inside the boundary's. fair_g1_fill declines because the edges share no vertices: ConnectEdgesToWires
  rebuilds them and the IsSame lookup (at < 0) fails. Matching by midpoint (/root/core-scratch/fair-fill-midpoint.patch)
  makes the fair fill run: 22754.7 (-4.5 %). Live's box bulges past the boundary (y -19..157.6 vs 0..152.4), so
  neither is Parasolid's G0 fill; needs a live capture of a plain 4-edge G0 fill (area, a mid-surface point) first.
- c2-surf-workshop-split Boundary surface 1 (6/12, parked): 2x2 profiles, defaults; ours 3803.55 mm², live 3762.61
  (+1.1 %). It goes through k::fill_surface (op_fill) but not its plate: a Coons branch for 4 G0 edges placed first
  changed nothing (edge count != 4 or Coons declined; not chased). Experimental coons_fill saved in
  /root/core-scratch/features-coons.cpp (full file, not a patch).
- c2-surf-workshop-loft Loft 2: 3-profile surface loft 7e-5 off live's area (interpolation detail); skipped.
- c2-sm-rocker-enclosure-body Extrude 6: Sketch 6 taken back by the translator (skasm's handoff 11-16). Not core's.
- c2-hunting-knife Revolve 1 (46/52, parked): HANDLE 10/27/18, live 10/27/19. Our ring between the handle's B-spline
  face and the dome's sphere is two equal 41 mm edges (vertices at (119.65, 0, 2.99): the B-spline's seam end, and
  (104.36, 0, -27.20): the sphere seam's end), both hidden by the seam-join rule. Live's ring RvBB (81.385 mm) has its
  arc-length midpoint exactly on the first, so live keeps one vertex, at the sphere seam's end. Rule unknown (a
  Parasolid sphere has no seam); a live capture of a dome revolved onto a swept handle would settle it.
- **IN FLIGHT (lead 11-16 ~22:10): free-form offset, one kernel `offset_solid` that bool's Shell can call too.**
  Design: every face moved by its own distance (0: stays on its surface, trimmed/extended to meet moved neighbours).
  Surfaces = quad::Surf (shell.cpp; already per-surface `d`, 0 allowed; free-form = Geom_OffsetSurface + foot search);
  edges = quad::march_offsets between the two faces' Surfs from the new end vertices (3D curve + both pcurves, same
  parameter); vertices = quad::solve3 / solve_all of the faces round each vertex; faces = their new wires on the offset
  (or base) surface; sewn, checked, volume. No topology change (a face vanishing, edges crossing): reported, the
  caller keeps its fallback. Limit: a deterministic work budget per face (march steps / halvings), reported by face,
  never a wall clock (DECISIONS 11-15). API sketch (sent to bool):
    std::optional<OffsetSolid> offset_solid(const TopoDS_Shape& solid, const std::vector<double>& face_dist,
                                            int face_budget, OffsetReport& why);   // C++, crates/kernel/cpp/offset.cpp
    k::offset_solid(shape, &distances, budget) -> Result<OpResult>                  // Rust; error names the face
  bool's requirements (11-16 22:2x): (1) face_dist along the face's outward normal, TopAbs_REVERSED applied; (2) faces
  to remove (NaN distance): result open, neighbours' walls ending at the rim's offset (Shell needs it); (3) an OpResult
  with history (Modified face -> its offset, Generated for new walls/edges); (4) poll noshape_should_break in every
  march loop besides face_budget; (5) report the tolerance reached per face; (6) parallel workers take their own
  copies of Geom surfaces (a shared converted B-spline raced in bool's test). bool switches by_quadrics' inner walls
  to it once it matches on the Shell corpora; by_quadrics stays the one direct Shell builder. bool gates step 1.
  Steps (each landed alone, release pair + bool's gate where crates/kernel shell.cpp is touched):
    1. DONE as 148d531e6 (branch core-offset, worktree core-trace): Surf, OffsetRun and the first quad block's
       declarations in crates/kernel/cpp/offset_quad.hpp; both quad blocks out of the anonymous namespace
       (noshape::quad). g++-15/13 build, 473 kernel tests. bool gating it (sent 22:3x); land from core-trace when
       bool says pass. (The make-a-Surf-for-a-face helper stays in by_quadrics: offset.cpp writes its own.)
    2. DONE as 23f44d3d7 (core-offset, stacked on 1; bool told 22:35, gates both): crates/kernel/cpp/offset.cpp
       op_offset_solid / k::offset_solid(shape, faces, distances, face_budget) + k::last_offset_report(); T1
       tests crates/kernel/tests/offset.rs (box exact, Ø10 boss out+up, extruded spline 0.2 mm within 1e-6 of
       its exact offset, a turned-round edge fails) + T2 probe dumped_offset_probe (NOSHAPE_OFFSET_CASE).
       NOSHAPE_OFFSET_DEBUG=/dir dumps each input (offset-N.brep + .txt face index/distance). 477 kernel tests,
       clippy clean, g++-13 builds. Refuses NaN (removed faces) and topology changes.
    2b. DONE as 8583db3fb (offset.cpp + tests only; bool's gate on 23f44d3d7 covers every shell.cpp change): a
       three-sided face whose offset vanishes goes (its neighbours meet at one solved vertex); curves on faces
       sharing one surface read by the edge's way in the face; a staying free-form face with moved corners lies on
       its run-on surface. T1 a_corner_triangle_whose_offset_vanishes_goes. fan-blade's 4 tools offset in 0.29 s
       each, all tolerances 1e-4 mm. Cases: vibe5 ~/tmp/core/offset-cases (probe .kp1.sh / .kp2.sh in core-trace).
    3. DONE as 87c383316 (core-offset): basic.rs Subtract with Offset calls offset_solid for non-analytic picked
       faces (OFFSET_FACE_BUDGET 20000), T1 features_a::subtract_with_offset_moves_a_tools_spline_face. fan-blade
       Boolean 2: volume = live's now, 57/142/86 vs live 56/143/88 (before 56/139/84, 54.7 mm3 off); 736 ms.
       Needs a release pair (model-core) before landing; land 1+2+2b once bool's gate passes (bool gating since
       22:40), then 3 after its pair. Next: the +1 face / -1 edge / -2 vertex difference vs live.
    4. bool's Shell may call it for its inner walls (bool decides).
- c2-fan-blade-sweep Boolean 2 (48/53, parked; lead: core's): Subtract with Offset 0.2 mm on 6 picked tool faces
  (11 per tool after the pattern copies: the sweep's free-form thread flanks among them). basic.rs offsets only analytic
  faces (else the tool as is), so ours removes 54.7 mm³ less (56/139/84 vs live 56/143/88). OCCT's offset_faces on them,
  limit lifted: fails every time after 33-62 s (4 tools). Needs a free-form face offset (Geom_OffsetSurface faces
  trimmed against their neighbours' offsets) in the kernel: a sizeable item, not started.
- **Lofts (mine, from dx's list):**
  c2-loft-guide-curves-wave-washer Loft 1: Sketch 4's rectangles are 63.5 mm off live's (an external COINCIDENT to
  live vertex JHk/JH4 unresolved: in skasm's handoff 11-16), which explains the "83 mm off the ring" below; redo the
  closed-path WIP once skasm fixes that. 4 profiles along a closed path; live closes it (4/4/0, 2080.8 mm³). WIP
  /root/core-scratch/loft-closed-path-wip.patch closes it round the path but comes out −11279 mm³: two profiles sit
  83 mm off the 26 mm ring, so MakePipeShell's carried sections twist. c2-surf-workshop-pipe: guided loft of more than
  two profiles unsupported (a Gordon surface over n profiles: big kernel item).
- **Impeller speed P0 (qa 11-16 04:40): waiting on blend.** Delete face 2 3.9 -> 26.4 s from v0.4.48 to v0.4.53
  (release A/B beside asm's full7: a hint; qa reruns it quiet). Chamfer 26 now builds (Error -> Ok, as live), so OCCT's
  Defeaturing gets a bigger part (trimming faces 13.2 s, FF 6.4 s, EF 6.1 s). Root: Part 1 differs from Fillet 28 on
  (ours 34/89 vs live 35/100; Fillet 29 fails, live 39/111; both hang on Fillet 3 (27), blend's next item), so the face
  "Sketch Stroemungskanal" is imprinted on differs: our sketch's circles are split (4 regions) but Extrude impellerraum
  adds 3 faces (live 20) and Extrude 6's ring is 4 faces (live 21, same 2337 mm³). Move face 1's 4 SWEPT_FACEs of the
  ring's split sides don't exist in ours: it fails ("Could not offset ...", 8 s in the reblend's defeature) and Delete
  face 2 defeatures a part live never has. Turning the fuse's unify off changes nothing there. blend (thread 0e6e9942) tells me when
  27-29 match (blend's Fillet 3 needs f24c11cc on main to measure); then recheck from Extrude impellerraum (the probe now prints a sketch's regions: NOSHAPE_PROBE_SKETCH).
- Extrude 33 (asm-pattern-std 36b90f) P0: 150 ms on main (debug, loaded vibe5); re-time once asm's Boolean feature
  pattern fix lands (Linear pattern 1 still errors upstream).
- **Handed off 11-16:** c2-config-parts-screws memory (14.8 GB): Revolve 9's unify (usd_kept_volume -> OCCT UnifyFaces)
  on the invalid body Sweep 1's OCCT fallback left: bool takes Sweep 1's cut, bool2 the unify bound.
- **Parked: knife Revolve 1's 19th vertex (branch core-pole-v2 in core-bis, not landed).** Our sphere's pole there sits
  where live's vertex is, (114.5, 0, -12.56), inside the dish. "Pole shown where the face has a boundary" regressed
  cf-box-dovetail-joint, prize-wheel-tim, cf-surface-pattern; "boundary not only circles round the pole's axis" still
  regresses prize-wheel-tim's knob (our sphere's poles lie on a horizontal axis, (±4.9, 0, 24.5): our sphere frame,
  not live's) and cf-surface-pattern (259 -> 344 vertices). Needs live's surface type of the knife's dish
  (SPHERE or a spun B-spline: Parasolid may keep a vertex where a spun profile meets the axis) before another try.
- Probe (uncommitted, /root/core-scratch/probe-wip.patch): ops parity::bodies_after_feature_probe (+ NOSHAPE_PROBE_SKETCH: a
  sketch's regions)
  (NOSHAPE_PROBE_DOC = a copy WITH its -wal / -shm, NOSHAPE_PROBE_AFTER, NOSHAPE_PROBE_OUT, NOSHAPE_PROBE_PART,
  NOSHAPE_PROBE_TAB): bodies as .brep, a part's faces (surface, edge indices), edges and vertices with shown flags.
  Saved documents' part names are Part N, not live's.

## Waiting on others
- **c2-sm-rocker-enclosure-body 15/28 (stable now):** Extrude 6's pocket is ~2 mm off live's and differently shaped
  (top face 89.76 vs live 66.95 mm²) because Sketch 6 isn't translated as live: its PROJECTED constraint finds no model
  geometry, its circular pattern isn't kept, and the solve (1.97 mm) is taken back. Sketch translation, not modelling
  (skasm's, paused until 11-17).
- **GearLab 90f78b:** with ae8b7f751 plus asm's unlanded 93346d03d (custom features with a `def` map; std 1777/1793)
  the loft builds; Gear Lab 1 then fails at an opFillet on the 157-face toothed solid ("Failed to fillet
  selections."): blend's. 4.3 s vs live 2.4 s, mostly 2 Booleans (1.7 s, bool's) and 1.2 M edge_point_at calls
  (0.53 s; not looked at). Repro vibe7: `NOSHAPE_FS_TRACE=1 NOSHAPE_PARITY_MODELS=~/agents/asm-keep/m90
  <ops test binary> parity::parity_models --ignored --nocapture`.
- **1dc32b (asm-infento-subs):** Draft 1 = live on dx's corrected input (-1306.543 vs -1306.54 mm³; OCCT 0030 +
  draft work bound landed af81dda05). Left: Fillet 2 / 3 / 12, Chamfer 3 (blend's); Extrude 42 17.6 s (bool's
  Boolean); Mirror 2 2.2 s (unify, bool2's). Hole 4-6 time: f1f212d03 above.
- **c2-impeller-loft:** first difference feature 15 (Boolean Flansch+Gehäuse 19/53/36 vs 19/56/39) is the fuse-then-unify
  merge (eval.rs fuse_targets -> unify_same_domain): bool takes it (11-15 22:40). Move face 1 / Delete face 2 speed only
  after that (they act on a part live never has). OCCT's Defeaturing dumps vibe7 ~/tmp/core/imp/dd.
- **asm-bjr-config 5d8bc1:** Sweep 1 landed 72248180. Kerf Compensation 1: live's corpus line is Manufacture=true
  (3 plain Ø2.2 holes, 3704.237 -> 3745.156 mm³, offset faces 454.835 -> 454.462 mm²); at false live leaves the part
  unchanged; ours offsets at false (dx's translation). Check our offset against the true numbers once dx has that input.
  Shell 1 is dx's. Extrude 55 (3.8 s vs 333 ms) is bool's fuse / unify.
- **e53bba:** bool's slit re-cut; then re-test the band-end patch vibe5 ~/agents/core-keep/e53bba-band-end.patch.
- **c2-hunting-knife feature 24:** live's own lines disagree (16-23 LINE, 24 OTHER, no geometry change); live asked
  in REQUESTS.md (11-15 00:45) to refetch 15-24 in one session.

## Next, in order
0. Impeller Move face 1 / Delete face 2 after blend's Fillet 3 / 28 / 29 (In flight). Left from 11-16's asks: c2-sm-rocker-enclosure-body moved 9 -> 15/28 between runs (A/B it before crediting).
1. Knife's dish vertex: asked live in REQUESTS.md (11-16 01:32) for the face's surface type and its edges / vertices;
   then retry core-pole-v2 (Parked above) on what live says.
2. qa's speed list (P0), what is mine: c2-valve-config Extrude 3 `prism_until.split` 131 ms (a logo up to one revolved
   dome face: OCCT's splitter, 26 face-face intersections; OBB and a (u, v) patch of the dome tried 11-15, no gain; a
   projection-built extrude would be a big change for ~100 ms; valve's other time is bool's / bool2's); c2-rings-loft: Circular pattern 1 913 ms vs live
   767: its 51 prism_until calls (302 ms) seem to run in parallel, bool's Boolean (892 ms) is the path; Shell 2
   `regions.tubes` 55-65 ms (Common of the solid with the hole tubes + fuse) and prism_until (~35 ms a call up to a
   1 m² cylinder face). Not mine there: rings Shell 1 (bool's quadrics), Circular patterns and Extrude 5 / housing
   Rotieren 1 (bool's Rust Boolean), Fillet 1 / Chamfer 1 (blend). The suite's per-feature part checks cost seconds of
   area / volume on offset faces (impeller's -3 mm wall 2.8 s); that is harness time, not regeneration.
3. psui's Move boundary: Up to vertex / face land for picked straight edges (81a182b14); other edge kinds and Up to part
   still open.
4. SHELL-SLOW tail after pockets: hang402 Shell 5's cost is the corner patches' slab fans (archive "Next, in order" 2);
   rounded blocks with pockets still decline regions (an outer fillet's sliver isn't a convex pocket); c2-bottle-split
   Shell 1 declines ("a pocket face isn't a plane or cylinder") and runs bool's quadrics (bool's self-interference
   filter halves that).
5. Live specs not implemented: revolved cones framed at their apex, tori / spheres at their centre (`reframe_cylinder`
   covers cylinders only); extruded circle edges on the sketch circle's frame vs our seam at u = 0; extrude side faces'
   x along their bottom edge (check).

## Parked
- Knife lofted handle: +17 mm³ smooth bulge between the guides vs live (v direction or live's section construction).
- E1 Govde (dock) 539/304 -> 538/303 accepted by the lead (bf26f8759's 0.5 mm seam line).

## How to run
- Builds / tests: `NOSHAPE_RB_HOST=<host> tools/tiers/rb.sh bash <script>` from the worktree (env prefixes don't
  reach the host: put them in the script). Mac: builds and crate tests only, one worktree of mine at a time.
- Heavy runs on a vibe host (suites, gates, pairs) through `~/bin/slot <cmd>` there (one per host).
- Suite pair: `core-base/.gof.sh <tag>` (copy into the worktree; `branch` as tag also builds the kernel with g++-13
  and profiles Lighten 1) builds the ops tests in release and runs parity_models into ~/tmp/core/g/run-<tag>.log.
  Base = `git merge-base HEAD origin/main` checked out detached in core-cache, same host, same session. Compare the
  per-document "features match" lines (SAME-PER-DOC) and the "speed:" lines.
- One document: `NOSHAPE_PARITY_ONLY=<doc>[,<doc>] NOSHAPE_PARITY_MODELS=$HOME/parity/models <ops-bin>
  parity::parity_models --ignored --nocapture` (prints each feature's time vs live); `NOSHAPE_PARITY_SAVE=<dir>` keeps
  the translated document; `NOSHAPE_KERNEL_PROFILE=1` prints each feature's kernel profile (includes the harness's
  own part checks; for regeneration alone profile the CLI's `feature list --doc` on a saved copy).
- Dumps: `NOSHAPE_DUMP_OPS=<dir>` (fillet, draft, shell, prism_until, defeature, project_along, unify ...); replay one
  with `NOSHAPE_REPLAY=<dir>/<name> [NOSHAPE_REPLAY_REPEAT=n] <kernel test bin> crash_inputs::replay_one --ignored
  --nocapture`; probes in kernel tests: features::defeature_dump_probe (NOSHAPE_PROBE_DEFEATURE), unify_apart_probe.
- Traces: NOSHAPE_FS_TRACE=1 (FS ops and their errors), NOSHAPE_PATTERN_TRACE=1, NOSHAPE_WORK_TRACE (draft work
  steps), NOSHAPE_SHELL_DEBUG=1 (shell path chosen), NOSHAPE_DEFEATURE_DEBUG=1, NOSHAPE_GUARD_TRACE=1.
- Private user documents (hats, dock, qa's tickets): only mode-700 copies in ~/tmp/core, deleted after;
  /root/qa-scratch/bin/userdocs.py compares two CLIs on them.
- C++: g++-13 build of noshape-kernel on vibe5 before landing (`CC='ccache gcc-13' CXX='ccache g++-13' cargo build
  -p noshape-kernel`); direct-initialise OCCT handles; never name a variable `near` / `far` (empty macros on some
  platforms); never build a gp_Dir from a vector that can be zero (OCCT 8: std::terminate); signal-handler state needs
  `std::atomic_signal_fence` after it is set.
- OCCT patches (patches/occt, newest 0030): rebuilding a library on a vibe host needs `-march=x86-64` (check objdump
  for ymm / vfmadd); installing it on hosts is the lead's (x86) / mine on the Mac (~/noshape-host OCCT, backups beside).
  OCCT loops must poll `noshape_should_break()` with a deterministic bound, never a clock.
