# Core modelling (agent "core"): current state

Updated 11-18 21:10 UTC (new thread 20:05 after the account limit; ids: see How to run). History (dated notes up to 11-15 14:48): docs/handoff/archive/core.md. Standing rulings:
docs/DECISIONS.md (read first; it wins over anything here). Hosts: core builds on vibex7 (rb-hosts); land
NOSHAPE_LAND_QUEUE_HOST=vibe2 tools/tiers/land.sh (since 11-17 21:23; vibe2 is the user's own server: only ~ngyekta and
/opt/occt, no heavy runs). Never kill a process tree from a computed pid (11-17 21:00 incident).

## In flight
- **LANDED 11-18:** up-to-next nearest-first 0a9645a56, welding NEW extrude 168f5bdb3, shown-edge joins 81deb44af +
  T1 5c36379c6, Delete face errors when a face stays 67a475987 (impeller Delete face 2 Ok -> Error on purpose, qa told).
- **Delete face onto (core-p0 branch core-delface, 4 WIP commits on ce74247af, NOT landed):** a planar face deleted
  onto its one non-"along" neighbour T (FaceTweak move_onto T's surface, B-spline extended by the face's box diagonal
  as OCCT), then onto_overlaps: where the new edge runs on along T's edge with the same neighbour (impeller housing
  bulges 0.68 mm past the flange end) the two become one edge, the face and T rebuilt as one face on the extended
  surface (edges of unknown sense turned to chain). Impeller face 24: valid, 95977.85 vs OCCT 95977.84 mm3 (kernel
  measure), ~0.2 s vs OCCT 5 s. T1 kernel features::a_flanges_end_is_deleted_onto_the_housing_below_it (repo's
  impeller-delete-face-2.brep face 27: 34 faces, 96645.148). g++-13 clean. Compat A/B vibex7 ~/tmp/core/imp/ab-df.out
  (cli-dfb base vs cli-dfn): land when only intended rows move; squash the 4 commits first.
- **Volume measure warning:** on impeller parts OCCT's plain Gauss VolumeProperties is ~1.2 % off (input 97755.8 vs
  mesh-converged ~96555, kernel props 95977.8 after the delete ~= mesh): the old "OCCT 97123.4" targets were plain
  Gauss. Compare with kernel props (replay prints them) or vibex7 ~/tmp/core/imp/mvol (mesh).
- **Face 30 (x=5 groove floor, cone 37 + boss cylinder 34 + rib notch B-splines 17/18/29/31):** OCCT can't delete it
  even unbounded (UnableToRemoveTheFeature). Needs cone and cylinder run on to meet (~x 10.4) through the rib faces.
  Our input to Delete face 2 isn't live's (44 vs 65 faces: walls held). Decision asked of the lead (build now or after
  the input matches).
- **Rib in parallel (core-sj branch core-ribpar on ce74247af, uncommitted):** each profile's rib (slab cut against the
  part) on rayon, in order; parting-line Rib 1 / 2 make 25 op_boolean each (1.2-1.4 s on loaded vibex7). CLI cli-rib
  building; A/B vs cli-dfb next.
- **Loft path sections (core-cache, uncommitted, with probe timers):** loft.path_sections 718 of op_loft 827 ms on
  loft-paths-guides Loft 3 (2 calls: loft_impl rebuilds without natural ends). WIP: thread_local memo of the last call +
  the plane sections cut in parallel (NonDestructive). CLI cli-lt2; then move to a clean branch without the timers.
- **qa speed backlog (TICKETS a3ff6d481) routed to bool 21:00:** sprockets Circular pattern 1 op_boolean 422/499 ms,
  loft-paths Mirror 1 op_boolean 1120/1136 + Loft 3's 942, parting-line Rib's fuses. Nuts waits on bool2. Split 3
  op_split 1.45 s (OCCT splitter section edges 729) not started. Profiler: vibex7 ~/tmp/core/sp.sh <cli> <doc> "<feature>".
- **live 11-18 21:00, feature-dialog pick rows (Shell, Hole, Revolve; behaviours "feel (feature dialogs) 11-18, batch
  2", live/feel/dialogs-1118/):** UI, not modelling: passed to the lead to route (psui).
- **Walls (core-rw branch core-walls2, d010a6d78 = 27b7836e0 re-applied, HELD with bool):** with them Move face 2
  turns Ok, Delete face 2 48 faces / 95977.8 mm3 (main 37 / 96246.4), then Extrude Bohrungssitz Ansaugflansch's Add
  (OCCT fuse 1 object 65 faces + 9 seat prisms, all valid) comes out INVALID 1992 mm3 / 55 faces, accepted (16 parts).
  Plain OCCT (fz) loses it only with NonDestructive/OBB/history/parallel all off; every other setting gives valid
  96485.5 mm3 / 98 faces. bool's op_boolean (619037c8c, bool-hat) loses it too and now refuses a union that lost its
  body (retry fuzzy, else error); bool looks for the trigger (asked: NOSHAPE_BOP_PAIRS_PARALLEL=0). Re-land walls when
  the fuse is whole + compat-on.sh clean. Case vibex7 ~/tmp/core/imp/rec47/51a35c85013e91e7.brep, probe ./fz.
- **Pair 7 (e9caeead0 offset_planes) REGRESSES:** c2-sm-hem-plus / cf-sheet-metal-hem Derived 1 loses a part (26/72/48
  -> 14/36/24), cf-mortise-and-tenon Mortise Tenon 2 19 -> 10 faces. Not landable as is.
- **c2-housing-draft (dx's DXF rows):** model = live at every feature up to Rotieren 1 (lengths; live measures circles
  polygonally, 96.093 vs 2pi r 96.133). Rotieren 1's torus exact (R 15.2, r 20.72); only difference: 8 floor lines
  0.144 / 0.337 mm where the torus is within 0.24 um of z=0 (r 15.2-15.3): ours split the side edges there (7.941 +
  0.144 vs live 8.084; 12.711 + 0.337 vs 13.047). Boolean tangency (bool's, minor). dx's rows are drawing projection
  (dx). Scripts /root/core-scratch/hdcmp2.sh, brepq/tor.cpp.
- **surf-intro-simple Loft 1 (skasm): de3b6e082 (core-loft, held):** surface loft takes sketch regions (T1 4/12/8 on
  squares). skasm saw 1 face on the real doc: check against live 4/12/8 once skasm's translator lands, then land.
- **qa 11-18 rollback-step list (speed):** fan-blade Extrude 8 120 vs 10, Derived 1 160 vs 51; motor-mount-jack
  Extrude 2 72-133 vs 9; bottle-split Revolve 2 / Split 1 50 / 17 vs 6.5 / 3; rings-loft patterns (live/qa/rbstep-1118).
- **Flansch verbreitern (core-rp branch flansch, on e9caeead0, uncommitted):** offset_planes takes linear extrusions too
  (the profile projected across the direction, run on 10|d| + 10 % past its ends, offset as Geom_OffsetCurve,
  extruded); extrusions move before planes so a plane moved later still meets them. T1 features_a::
  move_face_offset_of_an_extruded_spline_wall_and_a_plane (wall exactly 1 mm off, 6 faces). Next: impeller time A/B
  vs pair-mt (core-rp .imp.sh), g++-13 build, pair 8, land after pair 7's e9caeead0. First A/B (loaded): Flansch
  (vorne) 12 -> 983 ms with this branch: find why before anything else.
- **DRIVEN-PATTERN-SWEEP (qa P1) is REAL:** main fails Sweep 1 on the suite's own translation too (the suite says
  "matches to the end" only because live.jsonl's Sweep 1 line is unfetched). OCCT MakePipeShell status 1 at our
  tolerance (path ~4 mm -> 4.3e-9 m); with SetMaxSegments(1000) it builds 3 faces, valid, ~0.5 s, but keeps the 21.7°
  tilt: V 0.07543 / side 6.169 vs live 0.07985 / 6.347 (live tilts 21.7° at both caps yet less in between). Probe
  vibex7 ~/tmp/core/dp/swt3 profile.brep path.brep (src /root/core-scratch/brepq/swt*.cpp). Asked live (REQUESTS
  11:18) for ROBK's sections. Lead: keep the error. Live 12:4x: tilt stays 21.5°; live's extra V/area comes from its
  one smooth face approximating 12 sharp bends (radius to 0.05 mm, sections non-circular there): matching needs
  Parasolid's approximation. Retry parked on core-sweepseg (core-sw, 20f02b960).
- **cf-setback-fillet (blend 11-18):** Setback Fillet 1's 3 std fill() corner patches 87.52 mm2 in live, ours +1.14
  (~1.3 % large); live/suite-truth-1118/cf-setback-fillet.json. With Fill 1.
- **Nuts: waits for bool2's graze fix ebc58e0be** (not landed; bool2 tells me). Alone it makes Sweep 1 OK with live's
  counts (one face a B-spline where live has a cylinder). Then re-run nuts on main vs live 135.268 mm3 / 29 faces. My
  crest move (core-rp nuts3 59df02227) and exact cylinders (nuts2 7fec58496) are DROPPED (wrong OK without bool2's fix).
  Cases vibex8 ~/agents/core-keep/nuts/ (cutp).
- **Live truth at the suite's first differences (live 11-18, for the restart):** behaviours.md "core + bool + skasm (lead
  11-18): live truth at the model suite's first differences", per-face JSON live/suite-truth-1118/<doc>.json. Mine:
  knifehandle Sweep 2 area, singularities Mirror 3 (one sheet), pipe Loft (>2-profile guides), cpcurves fit spline
  end tangent = -n of plane JMC, Boundary surface 1, loft-paths Loft 3, rib-wrap Wrap 1, manualblend / mutualtrim
  Sweeps, surf-workshop-fill Fill 1 (B-spline 21x22, 5910.87 vs ours 31625). Triage with the lead first.
- **Fan-blade P0 (fix forward):** main after core-oc 1484 ms vs 0.4.56 730; Boolean 2 877 vs 122 (live 364). bool's
  b5b566884 drops the wasted Rust attempt; my lever: per-tool offsets' edges 52 + check 24 ms (NOSHAPE_OFFSET_DEBUG=1).
- **Extrude 4 (impeller, 490 ms vs 41):** op_prism_until's split on one face, a -3 mm offset of a 117x10 bicubic B-spline
  (footprint_patch declines: the face's boundary runs inside the footprint box; probe vibex8 ./pup). Parked.
- knifehandle left: Fillet 3 7/15/10 vs 6/12/8 -> Chamfer 1 wrong as Ok (9286 vs 10424 mm3): blend's (agreed 11-18 03:0x).
  Sweep 2 area 18225.81 vs 18227.26 mm2 (mine, small). TRANSLATE's rigid branch through the tweak; Delete face 2.
- **(4) local Delete face rebuild** once Delete face 2 sees live's faces (blend's Halter build feeds it); src
  /root/core-scratch/brepq/lr*.cpp (B-spline neighbour 1.55 mm2 off). Projection WIP parked:
  /root/core-scratch/proj-tight.patch.
- **qa 11-18 speed list (live ms):** impeller Flansch (vorne) verbreitern fixed (land1); Flansch verbreitern (pair 7 note);
  Extrude 4 690 vs 41 = prism_until split on a -3 mm offset B-spline (mine, parked); Extrude 3 / 1 = bool's Boolean;
  valve Extrude 1/2, knife Extrude 8 / Revolve 1: not looked at. Raw /root/qa-scratch/feat3/ab.jsonl.
- **skasm 11-18:** surf-intro-simple Extrude 1 fixed (land1); Fill 1 parked (above); manualblend Sweep 2 naming = parity pairing (above).
- **Queue after those:**
  1. qa: c2-fan-blade-sweep Boolean 2 (~1.2 s loaded vs live 364; 75-88 ms before my exact offsets, which fixed the
     volume): offsets 2 x ~176 ms in parallel (edges 73, volume check 62 -> ~17 with adaptive GProp: core-oc 482163a60), noshape Boolean decline ~300 ms + OCCT ~1.3 s on the exact offset faces (bool's). B-spline
     approximation of the offsets: slower, dropped.
  2. pulleys Circular pattern 1 speed (273 ms unloaded vs live 23): bool took the Boolean (arc across a seam).
  3. c2-loft-paths-guides: see 4c (centripetal path loft, pair waiting).
  4b. asm a04711 Extrude 4 "non-manifold" (asm's c22): our op_boolean's OCCT path cuts Part 3 to 422.23 mm3 (13 faces,
     non-manifold) where plain BRepAlgoAPI_Cut on the recorded case gives live's 213.964: sent to bool 22:40 (case
     vibex8 ~/tmp/core/a04rec/ad565563c0275193, probe ~/tmp/core/probefan/cutp, src brepq/cutp.cpp).
  5. asm (P2, on its unlanded asm-partorigin): robot-relations 0c3466 Thicken 1 of live's sheet JFD's 28 faces gives 11
     tiny parts (8.5 mm3) vs live one part 223.99 mm3 (silent wrong geometry until one part or an error); repro vibex7
     ~/agents/asm-keep/m0c with ~/agents/asm-keep/bin/ops-c05.
  6. dx (P2): GearLab 8f5622 faces fixed in core-oc (074b9b7ed); its speed 12.9 s vs live 1.15 s is next after it.
  7. asm (low): asm-pattern-std 36b90f Extrude 12 (feature 41; ADD, both directions UP_TO_SURFACE to RCBW, draft 3 deg;
     twin Extrude 11 matches): AR-S1/S2-108.2 17/43/28 vs live 14/36/24, volume/area = live (3 faces split where live's
     caps merge; not the Boolean per bool). Repro vibex8 ~/agents/asm-keep/mps/run.sh <bin tag> (mps/36b90f).
  8. asm (low): asm-robotic-arm 21ab69 Extrude 2 523/1504/992 vs live 540/1574/1032; repro vibex7 ~/agents/asm-keep/mra/run.sh.
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
- c2-hunting-knife Revolve 1 HANDLE 10/27/19 (live 11-18 second batch: the new face is a SPHERE r 60, 364.68 mm2, 1 edge; live 11-17, behaviours.md "core (01:32): the isolated vertex…", data
  live/core/q-knife-rev1/): live's extra vertex is the sphere's pole (114.5, 0, -12.563), an ISOLATED vertex (no edges)
  kept where the pole touched HANDLE's end plane RjBK (tangent there) that the subtract removed. Block repro: tangent
  top -> 9 vertices, 0.2 mm off -> 8. A Boolean keeping a removed face's tangent touching point; not started.
- mutualtrim Sweep 1 (skasm 11-17, back to core): Sketch 2 = live (Bezier P0 (-53.717,-28.813) -> P3 (-27.212,55.050)
  sketch mm = live JHR; Plane 1 on JHE). Our Surface 2's rails 36.508 mm vs live 94.15: the profile moves (22.13,
  28.81) not (26.50, 83.86): the path cut at world z = 0, y -31.59 (where it crosses Surface 1). trimEnds is Thin-only
  in std and in ours, so not that. Repro vibex6 ~/tmp/skasm/save/q.noshape (query_entities edges).
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
  (now at /root/parity/models/c2-fan-blade-sweep/live-base.x_t, 11-17). Probe vibe5 (down 11-17) ~/tmp/core/probefan (src /root/core-scratch/brepq).
- Worktrees: core-bis (docs lands, Mac timing), core-oc (movepl / msx: offset_planes), core-rv (pairtype parity),
  core-rp (free: nuts dropped), core-tw (Loft 4 + uncommitted fill dump), core-trace (core-pipe2).
- rocker-enclosure-sm Extrude 2 (next): Sketch 4's four Use circles (PROJECTED, live's external edges at x 45-60 mm,
  not on our part) come out as two: Extrude 2 names one region twice and cuts 2 holes of 4. Sketch translation:
  written into docs/handoff/skasm.md (landed 11-16).
- c2-surf-workshop-cpcurves Main pipe (thin sweep, 7/24): (1) Sketch 4's circle kept 18.8 mm off the path (translator
  take-back: in skasm's handoff). (2) Mine: live merges the faces swept along the path's three collinear lines into
  one per wall (6 faces; ours 10: a face per path edge). **Committed b173dc5bb (core-oc, NOT in the vibex7 pair):
  join_collinear_lines joins a sweep path's lines along one line into one edge (first piece's source)**: Main pipe's
  faces = live (only its box differs, the take-back). T1 features_a::a_sweep_along_collinear_lines_has_one_side_face.
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
- Probe (uncommitted, /root/core-scratch/probe-wip.patch): ops parity::bodies_after_feature_probe (+ NOSHAPE_PROBE_SKETCH: a
  sketch's regions)
  (NOSHAPE_PROBE_DOC = a copy WITH its -wal / -shm, NOSHAPE_PROBE_AFTER, NOSHAPE_PROBE_OUT, NOSHAPE_PROBE_PART,
  NOSHAPE_PROBE_TAB): bodies as .brep, a part's faces (surface, edge indices), edges and vertices with shown flags.
  Saved documents' part names are Part N, not live's.

## Waiting on others
- c2-config-parts-standoffs Revolve 1 (13/32): Sketch 3 solved 4 x 22 mm, chamfers on the axis side; live 3 x 24 (Length),
  outer corners chamfered (sketches.json). Already in skasm's handoff (translator takes Sketch 3 back). Variables OK.
- **c2-sm-rocker-enclosure-body 15/28 (stable now):** Extrude 6's pocket is ~2 mm off live's and differently shaped
  (top face 89.76 vs live 66.95 mm²) because Sketch 6 isn't translated as live: its PROJECTED constraint finds no model
  geometry, its circular pattern isn't kept, and the solve (1.97 mm) is taken back. Sketch translation, not modelling
  (skasm's; skasm back 11-17 09:25 as 18a8ce79-5f2e-4089-9ad8-cc9d1182f0e6, on the surface-workshop item incl. mutualtrim).
- **GearLab 90f78b:** with ae8b7f751 plus asm's unlanded 93346d03d (custom features with a `def` map; std 1777/1793)
  the loft builds; Gear Lab 1 then fails at an opFillet on the 157-face toothed solid ("Failed to fillet
  selections."): blend's. 4.3 s vs live 2.4 s, mostly 2 Booleans (1.7 s, bool's) and 1.2 M edge_point_at calls
  (0.53 s; not looked at). Repro vibex7: `NOSHAPE_FS_TRACE=1 NOSHAPE_PARITY_MODELS=~/agents/asm-keep/m90
  <ops test binary> parity::parity_models --ignored --nocapture`.
- **1dc32b (asm-infento-subs):** Draft 1 = live on dx's corrected input (-1306.543 vs -1306.54 mm³; OCCT 0030 +
  draft work bound landed af81dda05). Left: Fillet 2 / 3 / 12, Chamfer 3 (blend's); Extrude 42 17.6 s (bool's
  Boolean); Mirror 2 2.2 s (unify, bool2's). Hole 4-6 time: f1f212d03 above.
- **asm-bjr-config 5d8bc1:** Sweep 1 landed 72248180. Kerf Compensation 1: live's corpus line is Manufacture=true
  (3 plain Ø2.2 holes, 3704.237 -> 3745.156 mm³, offset faces 454.835 -> 454.462 mm²); at false live leaves the part
  unchanged; ours offsets at false (dx's translation). Check our offset against the true numbers once dx has that input.
  Shell 1 is dx's. Extrude 55 (3.8 s vs 333 ms) is bool's fuse / unify.
- **e53bba:** bool's slit re-cut; then re-test the band-end patch vibe5 (down 11-17) ~/agents/core-keep/e53bba-band-end.patch.
- **c2-hunting-knife feature 24:** live's own lines disagree (16-23 LINE, 24 OTHER, no geometry change); live asked
  in REQUESTS.md (11-15 00:45) to refetch 15-24 in one session.

## Next, in order
0. Impeller P0 (In flight): land Extrude impellerraum's split walls after the pair, then Delete face 2 on our input and
   the local rebuild, re-timed on the Mac vs 12 ms; then the In-flight queue 1-7.
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
  live e8549719-ef9a-42de-960c-e24b29f142a6, live2 79ae5cdb-42a4-4025-b43a-da61a4f92ad7, asm
  d8d597e0-675c-497b-a804-544ceab99bbd, dx (ask the lead), psui 4973e295-9e92-463f-a13f-2ec5258bc5f9 (old ids stopped; never message a paused agent: DECISIONS 11-17).
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
- C++: g++-13 build of noshape-kernel on vibe5 (down 11-17) before landing (`CC='ccache gcc-13' CXX='ccache g++-13' cargo build
  -p noshape-kernel`); direct-initialise OCCT handles; never name a variable `near` / `far` (empty macros on some
  platforms); never build a gp_Dir from a vector that can be zero (OCCT 8: std::terminate); signal-handler state needs
  `std::atomic_signal_fence` after it is set.
- OCCT patches (patches/occt, newest 0030): rebuilding a library on a vibe host needs `-march=x86-64` (check objdump
  for ymm / vfmadd); installing it on hosts is the lead's (x86) / mine on the Mac (~/noshape-host OCCT, backups beside).
  OCCT loops must poll `noshape_should_break()` with a deterministic bound, never a clock.
