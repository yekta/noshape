# Core modelling (agent "core"): current state

Updated 11-17 11:13 UTC. History (dated notes up to 11-15 14:48): docs/handoff/archive/core.md. Standing rulings:
docs/DECISIONS.md (read first; it wins over anything here).

## In flight
- **11-17 08:36 landed:** c85e75070 (Move face offset routing reverted + T1), core-sing2 (21dd0e7ab reapply cylinder,
  ba77957ae bounded per-feature defeature, cd3f1c01b Thicken pinched guard). f197: blend's, re-landed 74d05ad5.
- **Impeller P0 (TICKETS IMPELLER-DELETE-FACE-2-SLOW; qa b1d13282e: not a regression, a speed item vs live (doc ~19 s
  vs 2.8 s); lead 11-17 order): parity of the input first, then the local rebuild on live's faces.**
  - blend: Fillet 3 LANDED be6f0ca8 (10:45), arcs fix 9e40ad95a (34/95/62 = live, Stevenrohr einnschub too) lands ~11:15,
    then Fillet Stevenrohr zum Halter ("Adjoining edge not filleted."). On 9e40ad95a ours matches 29/48.
  - **Extrude impellerraum (mine; bool: not in its queue):** same volume / area as live, topology off (ours +3 faces,
    live +20). Ours (dump vibe5 ~/tmp/core/impdump/unify-29, pre-unify 38 faces = after): outer wall ONE cylinder
    8168.14 mm2, inner ONE 7414.16, cap 584.34, plane 212.49. Live: outer in 4 pieces (3 x 2107.33 + 1846.21), inner in 15
    (1464.7 ... 10.5), cap 584.37: axial splits. Extrude 6 extrudes that CAP_FACE (isStart false) -> live 21/57/38 (19
    arcs), ours 4 faces; Move face 1 then needs the split sides. Live's outer splits at -136/-44/49/131 deg (round
    (y 0, z -34) mm, the circles' real centre) = where our part's edges cross the circles at x = 0 (inner: -146/-82/49/131
    match too; -11/12/57..124 not found by my crude finder, xa.cpp). Reading: Parasolid splits the tool's walls along the
    part's boundary vertices (a Boolean merge rule) -> bool's area per lead: recording running (vibe5
    ~/agents/core-keep/impbool, NOSHAPE_RECORD_BOOLEANS + EXACT; the case = result volume 95981.76 mm3), then send the
    case id to bool 7fa8ceab. Check first whether the inner -11..124 splits need the second direction's walls (x 0..25).
  - Delete face 2 (later): Mac median main 2822 ms, on 4efe95564 1825 ms, live 12. On our part it deletes stand-ins
    (planes x 5 / 6, faces 26 / 29 of 38). OCCT's defeature: ~75 % in its final MakerVolume (df4.cpp scope timer).
    Prototypes vibe5 ~/agents/core-keep/localrm (src /root/core-scratch/brepq/lr.cpp, lr2.cpp): trim + re-sew: planar
    images = OCCT's exactly; the B-spline neighbour's image 1.55 mm2 larger (OCCT trims it against non-neighbours),
    invalid, 342 ms vs 2587: the case to solve. Gap solid fused / cut: wrong (527 vs 650 mm3), Boolean ~850 ms.
  - Probes (vibe5 ~/tmp/core/probefan, src /root/core-scratch/brepq; build: g++-15 -O1 -w -I/opt/occt/include/opencascade
    x.cpp -L/opt/occt/lib -Wl,--start-group <all libs> -Wl,--end-group -lpthread -ldl): df2 (dump faces + OCCT per group),
    df4 (OCCT progress-scope timer), fl (faces in an x range), uc (faces / volume), xa, sp/sp2/sh (split), ang, sw.
- **core-split LANDED 0d7e2fc2 (11-17 09:57):** quick Gauss Split centre, Rib one Boolean, exact B-spline Use,
  sheet halves. parting-line Split 3 edges 30/20 vs live 34/24: live's inner parting edges come in 4 pieces + 0.016 mm
  stubs + duplicates (Parasolid artefacts, same geometry): not a target. Pair note: the FIRST run of a pair sees 7 cf
  docs + extension-spring compared fully, the second not (run-order artefact): compare common documents only.
- **core-pipe2 (core-trace; HELD by lead until Split 1's Surface 1 = live 9608 mm2):** 3e106dca0 N-profile guided loft
  without conditions, 829ac26ef intersection n1 x n2, f6aa8f16f parity pairs by type, bd2a1af82 sheet halves,
  a1142dcfb Move boundary on free-form sheets (extend_face_sides) + Keep front by a sheet piece's area (T1s).
  manualblend now: Sweep 2 8223.1 (live 8224.9), MB2 9102.5 (9104.4), MB1 = live, Split 1 6 parts as live but Surface 1
  9594.6 vs 9607.8. Root: our Intersection curve 1 (Loft 1 x Sweep 1) starts 0.057 mm beside live's (Sketch 5's circle
  centre, stored at live's curve end, Concentric to it: the harness keeps live's position), so the tube is off-centre.
  Next: why the section starts 0.057 mm off (inputs meet at a shallow angle? needs live's surfaces / x_t).
- **P1 after impeller P0: apply loft guide + end conditions** (pipe Loft; singularities Loft with bad singularity +
  Loft 3). Live 06:14 answered (behaviours.md "core (06:14)…", live/core/q-guided-loft/lofts.json): lines P1 (0,0,0)-
  (40,0,0) / P2 (0,60,0)-(40,60,0), arc guides via (-8,30,10) / (48,30,10): defaults OK, 1 bicubic face 4x18 poles,
  3269.1654 mm2, straight across (x 20 / x 4 at y 30 both z 10); both guides Normal to guide (m 1, 2):
  BSURF_INCONSISTENT_CONDITIONS_ERROR; guide 1 Tangent, Start Normal to profile (m 1, 2):
  LOFT_START_OR_END_CONDITIONS_WITH_GUIDES_FAILED. First: our result on these 6 (match the errors), then the suite 3.
- c2-hunting-knife Revolve 1 HANDLE 10/27/19 (live 11-17, behaviours.md "core (01:32): the isolated vertex…", data
  live/core/q-knife-rev1/): live's extra vertex is the sphere's pole (114.5, 0, -12.563), an ISOLATED vertex (no edges)
  kept where the pole touched HANDLE's end plane RjBK (tangent there) that the subtract removed. Block repro: tangent
  top -> 9 vertices, 0.2 mm off -> 8. A Boolean keeping a removed face's tangent touching point; not started.
- mutualtrim Sweep 1 (skasm 11-17, back to core): Sketch 2 = live (Bezier P0 (-53.717,-28.813) -> P3 (-27.212,55.050)
  sketch mm = live JHR; Plane 1 on JHE). Our Surface 2's rails 36.508 mm vs live 94.15: the profile moves (22.13,
  28.81) not (26.50, 83.86): the path cut at world z = 0, y -31.59 (where it crosses Surface 1). trimEnds is Thin-only
  in std and in ours, so not that. Repro vibe6 ~/tmp/skasm/save/q.noshape (query_entities edges).
- Curve point plane sketches (live 05:01, behaviours.md "core (05:01)…", live/core/q-curvepoint/frames.json): the plane
  = ours; a sketch on it measures from the WORLD ORIGIN projected onto it (pipe (-39.2748,0,25.7771), x (0,1,0);
  mutualtrim (0,-21.5364,-44.4163), x (1,0,0)); pipe ellipse centre (-46.0366,0,15.4747), major 5.0451 along Y,
  minor 4.7120. Our frames = live's (traced). pipe: every PIERCE to a guide edge is rejected by the sketch op ("can't
  be referenced in this sketch ..."), so Loft profile 3 stays as stored, 2.69 mm off our guide: sent to skasm 11-17.
  skasm 11-17 (unlanded stack): Pierces made now; Loft profile 3's would move the ellipse 2.69 mm onto our guide and
  are taken back (nothing of live's confirms it), so OUR GUIDE is off live's by 2.69 mm near the ellipse's far end
  (live (-48.6220,0,11.5353)): find which guide sketch / curve (Side profile's Tangent is taken back by 0.179 mm too).
- Fill G0 (live 11-17, behaviours.md, live/core/q-fill-g0/fills.json: poles, knots, 3x3 points + normals): corpus
  Fill 1 (= c2-surf-intro-examples' Surface 1, ours 61910.9 vs 23827.4353 mm2) is ONE bicubic B-spline 14x18 poles,
  non-rational, unnormalised single knots (u -0.08537..0.07992, 10 interior; v -0.07325..0.06120, 14 interior): fitted,
  not Coons; centre (1.6244, 68.5723, 20.9206). A fresh skew-quad G0 fill (Precise) = BILINEAR 1x1 patch of its 4
  corners, 10293.9930 mm2, centre (50,50,7.5). Next after the impeller.
- fan-blade Boolean 2: 56/139/84 with core-next (live 56/143/88): +2 edges +2 vertices per hole need live's x_t
  (now at /root/parity/models/c2-fan-blade-sweep/live-base.x_t, 11-17). Probe vibe5 ~/tmp/core/probefan (src /root/core-scratch/brepq).
- Worktrees: core-trace (core-pipe2; probe /root/core-scratch/probe-wip.patch), core-oc (detached on blend's impeller Fillet 3 for the impeller dumps), core-bis: docs
  lands + Mac timing (.mac.sh, .macdoc = qa's impeller copy).
- Impeller Extrude 4 model side: closed, nothing to fix. The 12 mm² sliver alone never meets the offset wall: even its
  surface over all of its own bounds only grazes the prism at the sketch plane (replayed dump vibe5
  ~/agents/core-keep/e4dump/prism_until-10, experiment patch /root/core-scratch/uptowiden-exp.patch). Live's Extrude 4
  is the whole region, which we match (impeller 27/48 on main after blend's d7f9264c9; first difference Fillet 3).
- rocker-enclosure-sm Extrude 2 (next): Sketch 4's four Use circles (PROJECTED, live's external edges at x 45-60 mm,
  not on our part) come out as two: Extrude 2 names one region twice and cuts 2 holes of 4. Sketch translation:
  written into docs/handoff/skasm.md (landed 11-16).
- c2-surf-workshop-britatap Touch screen split (4/22): our projected split curve (BRepProj section, a degree 7
  B-spline) ends 0.116 mm from the face's corner along the rim, where the sketch edge ends exactly; live ends it on
  the corner (2/6/5 vs ours 2/7/6). Kernel projection precision at a tangent end; parked.
- c2-surf-workshop-cpcurves Main pipe (thin sweep, 7/24): (1) Sketch 4's circle kept 18.8 mm off the path (translator
  take-back: in skasm's handoff). (2) Mine: live merges the faces swept along the path's three collinear lines into
  one per wall (6 faces; ours 10: a face per path edge). **Committed b173dc5bb (core-oc, NOT in the vibe7 pair):
  join_collinear_lines joins a sweep path's lines along one line into one edge (first piece's source)**: Main pipe's
  faces = live (only its box differs, the take-back). T1 features_a::a_sweep_along_collinear_lines_has_one_side_face.
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
- c2-hunting-knife Revolve 1 (46/52, parked): HANDLE 10/27/18, live 10/27/19. Our ring between the handle's B-spline
  face and the dome's sphere is two equal 41 mm edges (vertices at (119.65, 0, 2.99): the B-spline's seam end, and
  (104.36, 0, -27.20): the sphere seam's end), both hidden by the seam-join rule. Live's ring RvBB (81.385 mm) has its
  arc-length midpoint exactly on the first, so live keeps one vertex, at the sphere seam's end. Rule unknown (a
  Parasolid sphere has no seam); a live capture of a dome revolved onto a swept handle would settle it.
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
- **c2-config-parts-nuts Sweep 1 (14/17, parked again 11-17 02:55; mine per bool 01:3x).** The tool's crest strip is
  a B-spline (1 x 11) exactly on the bore's cylinder (r 1.621113885844308 mm); OCCT intersects instead of seeing one
  surface, and its Cut removes more than the tool. Rust Boolean declines. Live 135.268 mm3 (29/59/32), target
  143.883, tool 11.888. WIP /root/core-scratch/sweep-cylinders-wip*.patch (op_pipe_shell faces_on_cylinders: strips
  made exact cylinders, unwrapped pcurves; ShapeFix_Shape makes it valid). Probe (vibe5 ~/tmp/core/probefan/nuts,
  src /root/core-scratch/brepq/nuts.cpp, dumps ~/agents/core-keep/nm-nuts-cyl): with exact cylinders the cut is
  still wrong (144.384 mm3, 18 non-manifold edges): each strip is ONE face spanning ~7 turns (u -21.3..23.6), which
  OCCT's Booleans can't place points on (they project into one period). ShapeUpgrade can't split it
  (SplitSurface::Init clamps a periodic range to one period); BRepAlgoAPI_Splitter by the seam half-plane gives 27
  faces but no separate solids. 11-17 03:15: per-turn split DONE in WIP /root/core-scratch/sweep-cylinder-turns-wip.patch
  (cylinder_turns: strips cut at u = 2 pi k, pcurves in [0, 2 pi], edges split via ReShape; wire reversed where the
  cylinder's normal opposes the B-spline's: valid with no ShapeFix). The cut is still wrong (155.4 mm3, invalid;
  TooSmallEdge, AcquiredSelfIntersection): the r 2.0 crest passes through the chamfer rims (r 2.0 at z 0 / 4.06).
  bool 04:3x: fixed on bool's side (bool-nuts b6466ab04: a turn face's helices 2.6e-8 rad short of a full turn were
  taken as its seam): valid cut, 42 faces, 135.267625 mm3 (live 135.268). After it lands (own gate): unpark the
  exact-cylinder patch and compare the doc's volume with live (tool axes 2-5e-15 off z: make them exact).
- c2-config-parts-standoffs Revolve 1 (13/32): Sketch 3 solved 4 x 22 mm, chamfers on the axis side; live 3 x 24 (Length),
  outer corners chamfered (sketches.json). Already in skasm's handoff (translator takes Sketch 3 back). Variables OK.
- **c2-sm-rocker-enclosure-body 15/28 (stable now):** Extrude 6's pocket is ~2 mm off live's and differently shaped
  (top face 89.76 vs live 66.95 mm²) because Sketch 6 isn't translated as live: its PROJECTED constraint finds no model
  geometry, its circular pattern isn't kept, and the solve (1.97 mm) is taken back. Sketch translation, not modelling
  (skasm's; skasm back 11-17 09:25 as 18a8ce79-5f2e-4089-9ad8-cc9d1182f0e6, on the surface-workshop item incl. mutualtrim).
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
0. Impeller P0 (In flight): Extrude impellerraum's split walls (Boolean case to bool), then Move face 1, Fillet 4,
   Move face 2 on the right input, then Delete face 2's local rebuild on live's faces, re-timed on the Mac vs 12 ms.
1. Guided-loft conditions P1 (live 06:14 answered: match live's errors first), then pipe (our guide 2.69 mm off live's),
   singularities Loft with bad singularity / Loft 3. Then: Fill G0 = intro-examples' Fill 1 (live's fitted 14x18 surface),
   mutualtrim Sweep 1 (path cut at Surface 1), fan-blade Boolean 2 with live's x_t, manualblend Intersection curve 1
   (0.057 mm off live's start; unblocks core-pipe2), knife Revolve 1's isolated vertex (live: Parasolid keeps the
   sphere's pole where a removed tangent plane touched: a Boolean rule, ask bool).
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
- Agent ids (11-17): lead 818534eb-f6b1-4a00-bb48-ebf0f7cdbd8a, blend 0e6e9942-0713-46c1-ae55-132fbbb4a14b, bool
  7fa8ceab-4549-499f-9b01-3a289c2c95b4, qa 132756b3-e48d-4b9f-8a0e-3d4f23629c8b, skasm 18a8ce79-5f2e-4089-9ad8-cc9d1182f0e6,
  live e8549719-ef9a-42de-960c-e24b29f142a6, psui 4973e295-9e92-463f-a13f-2ec5258bc5f9 (old ids stopped; never message a paused agent: DECISIONS 11-17).
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
