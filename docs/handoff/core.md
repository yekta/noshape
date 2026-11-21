# Core modelling (agent "core"): current state

Updated 11-21 04:48 UTC (lead order 11-21: thicken trim, SPEED-VALVE-CONFIG, KNIFEHANDLE, then the direct thicken ticket). Mac: builds + crate tests via NOSHAPE_RB_HOST=mac tools/tiers/rb.sh, single-model repros in ~/noshape-host/core-keep/models. Agent ids: only /root/lead-scratch/roster.md, read before every message (never copy ids here). History (dated notes up to 11-15 14:48): docs/handoff/archive/core.md. Standing rulings:
docs/DECISIONS.md (read first; it wins over anything here). Hosts: the least-loaded of vibex7/vibex8/vibe2 (DECISIONS a67911bef); land
NOSHAPE_LAND_QUEUE_HOST=vibe2 tools/tiers/land.sh (since 11-17 21:23; vibe2 is the user's own server: only ~ngyekta and
/opt/occt, no heavy runs). Never kill a process tree from a computed pid (11-17 21:00 incident).

## In flight (11-20)
- **LANDED 11-20:** singularities Sketch 2 P0 a2e7c966; fillet orientation d80c2c58 (then reverted, below); manualblend
  Intersection curve 1 5efc58479; T2-NIGHTLY-22 f7b71172; isaac Mirror 1 9c9f2926 (13,277 -> 1,064 ms); arc sense
  6b384aa69 + AGENT_API 2374eb6fc; P0 COMPAT-FILLET-ORIENT-REFS aa5682636 (d80c2c58e's edge turning out: OCCT's offset
  can't join turned edges; if snap-hook's axis is wanted again, orient at the point of use); screws Sweep 1 0f8993ef8
  (unify keeps a USD merge only if its new faces pass BRepCheck; follow-up: live 8 faces there, we 20). Targets removed.
- **LANDED 18:41 771be348 (core-rf, target moved to vibex8 core-rp): c2-replace-face-exercise Replace face 1 (dx):** ERROR -> 85/184/112, 688456.9 mm3 (live
  85/184/104, 688437.672); doc 0 -> 6/8. rounds.rs takes OCCT's free-form corner patches; Replace face rolls its fillets
  again (reblend + replace_by_prism: onto_plane tweak, onto_cylinder = sewn pocket solid on the face's side of the
  cylinder template, Fuse/Cut + unify). Suite vibex8 101/174 vs base: only this doc moved. Follow-up: 8 extra vertices /
  19 mm3 in OCCT's refillet corners (5-edge vertices, asymmetric centroid); Replace face 2 builds (550.5 cm3), unchecked.
- **dx: c2-impeller-loft Delete face 2 (622 ms vs live 12):** comes AFTER the doc's first difference (Fillet 3, blend):
  our input isn't live's. Face 33 goes onto neighbours; face 39 (x=5 plane with a bore) OCCT spends its bound and
  gives up (face left; live 65 -> 63 faces). Revisit when Fillet 3 matches. Moving by_region before OCCT broke kernel
  tests a_boss_and_a_pocket..., delete_and_offset_faces and didn't apply (face 33 has no closed region): reverted
  (diff /root/core-scratch/defeat-reorder.diff).
- **parting-line Draft 1/2 speed (PARKED 17:50):** 128 samples (main): Draft 1/2 ~760/800 ms, live 20/18 (vibex7 load ~15).
  Fewer samples (diff /root/core-scratch/pl-samples.diff): 8 per span -> Split 3 38 edges vs 34 (10/17); 16 per span keeps
  13/17, Split 3 1.8 s -> 0.7 s, but Draft 2 slower (bool's boolean.rust.sections_ahead 92 -> 646 ms on the 67-pole
  sheet; told bool 17:50). Real fix is structural (no slab + Splitter by a sampled ruled sheet). Not landing.
- **P2 SUITE-NEW-INDEX-1120-CORE (TICKETS 7066344c1):** 12 new documents' first differences (Split 2, Sweep 1 invalid,
  Constrained surface, External thread, 3x Frame 1, Rib 2, ribbed enclosure extrude remove, Projected curve 1, Thicken 1,
  Wrap 1). 19:0x in core-rp: Thicken NEW keeps each surface's slab its own part (add_new_apart; live 2 overlapping
  parts); Constrained surface: 1 of 961 points (Linear pattern 1 copy of Sketch 2's rL3agFJlOTPG1.MirrorCS.end,
  instance 2) unmapped -> TRACE COPY added; Projected curve 1 TWO_SKETCHES: 4 edges vs live 2 (guess: closed ellipse
  prism's seam splits the section; join across seams); frames = dx (7aac1279c fixed the path query).
- **LANDED 16:01 core-bis as 6d8e63f8** (fit spline one end, loft to point Coons, Normal to profile, pinched thicken,
  flat fan): singularities 5 -> 8/15. Target removed. screws Sweep 1 valid on main (bool: closed).
- **LANDED (queued 03:02) 199faacf, core-th: release P0 c2-sm-tablet-stand** Sketch 3 "Some external references are
  missing." (skasm): f9ad07b50's exact thicken rebuild gave the SM part's vertices no history. k::thicken_plain (no
  rebuild) for opThicken + both sheetmetal.rs; native Thicken / thin loft keep it. skasm compat SAME as 0.4.63. Root
  fix later: map the old vertices to the rebuilt ones in exact_hist, then use the rebuild everywhere again.
- **LANDED 04:15 1a106f47 as 3bbeb609: qa validity P1s** (LORA unify keeps a valid shape; SURFLOFTS 3-sided Coons;
  WRAP1 BuildCurves3d at 1e-7, walls from the target surface). Suite pair vibex6:
  118 -> 119 / 174 docs, 1840 -> 1848 features, none worse (c2-rib-wrap 10 -> 18). OPEN (P2, mine): a thin wrapped r3
  disc fuse loses the cylinder in OCCT (cut -> 0 volume, common empty); recording /root/core-scratch/wrap-disc-fuse-lost.bin
  (parts[0] objects, parts[1] tools; tool 1 fails alone).
- **SPEED-SPLIT-THICKEN (P0, core-trim queued 11-21):** c2-surf-workshop-split Thicken 1 (Mac: offset 100 ms, ours doc
  202 ms vs live 112). Trim: the sharp path's volume computed once while orienting (-11 ms). Real fix = DIRECT THICKEN
  (lead 11-21, own ticket, after VALVE + KNIFEHANDLE): offset faces on Geom_OffsetSurface, concave sharp edges trimmed by
  GeomAPI_IntSS (2-6 ms an edge), seams by IntCS, ruled walls, sew; only sheets whose sharp edges are concave loops,
  OCCT fallback; lands on the same faces / edges / volume as OCCT on every thicken in suite + corpora, a test per case.
- **SPEED-VALVE-CONFIG (P0, core-th):** Circular pattern 1 (Mac 98-102 ms vs live 67; x86 201): a drafted Through all
  REMOVE ran 2 m (model_extent's floor), its cones met below the 150 mm disc -> rust_cut_one_by_one (7 x ~12 ms).
  Fix: extrude.rs reach() = the parts' extent along the direction + 10% for a drafted Through all cut / intersect
  (EvalCtx::seed_op carries the seed's operation into a pattern's New evaluation); T1 engine
  reapplied_drafted_through_all_holes. Mac timing + crate tests running.
- **KNIFEHANDLE-LOFT1 (qa P1 11-21, in progress):** Loft 1 = SURFACE through P1 (2 splines), P2-P6 ellipses, a point
  (tip 188.575), 2 guides (side profile splines; guide 1 runs on to x=0); live Handle 1/2/3, 21477.23 mm2, box x
  13.09..188.58, y +-19.958, z -39.98..13.80. Work: /root/core-scratch/gordon3.diff (k::loft_gordon = OCCT 8
  GeomFill_Gordon + GORDONDBG). Found: P4's ellipse (x 117.98) misses guide 1 by 8.77 mm and guide 2 by 2.91 mm, as in
  live's own stored sketch (its PIERCE on KTRB at parameter 1 = the guide's END, z -36.651; the guide crosses x 117.98
  at z -27.80). So live lofts with a profile off its guides; Gordon refuses (ContactDiscovery). Gordon without P4
  throws Standard_OutOfRange on the point profile. Next: Gordon over P1..P6 minus P4 with guides trimmed to P1..P6 and
  the apex closed apart (or a guided ThruSections), compare area with 21477.23 mm2.
- core-rp's 11-20 batch LANDED 00:10 as f9ad07b5 (details archived 11-21 02:08). Face blend on top:
  /root/core-scratch/fb-on-top-detail_a.diff + fb-on-top-test.rs (FBDBG to strip).
- **live data-management batch 1 (19:00):** cov-dm-config (5 configurations, configured suppression) and cov-dm-vs
  (VS -> VS -> Part Studio, inch units, w change). Importer already reads configured suppression + VS references;
  suite checks one configuration. Asked live 18:57 (REQUESTS) for c2-dm-config / c2-dm-vs dirs + configurations.json
  rows; then parity_models evaluates each row (config_activate, parts / bbox / volume).
- **bool-land7 e3078f89:** if a model result moves after it lands, tell bool. **Singularity loft / Loft 3 / Fill G0:**
  open (live q-singularity-loft-1119); core-scratch/sing, fill/ (FILLDBG in core-oc).

## Earlier (11-18/19)
- **Lead 11-19:** cpcurves Sweep 5's 8/9/2 (live splits the R16 circle into a CIRCLE arc + an OTHER curve) is live's
  representation, a known difference, not a target. Build hosts: the lowest 1-min load of vibex7/vibex8/vibe2 each
  time (DECISIONS a67911bef); private docs on vibex7 only.
- **LANDED 11-19:** loft sections of different edge counts refused 82d75f4a5 (P0); Thicken one-sided intersection join
  7dd227351 (split 12/12; singularities corpus exact); Routing curve multiDx/Dy/Dz/RotationMatrix aeb0cc0a7 (live2 audit
  row 4; psui told: the multi triad / toggle-points manipulators are UI + host manipulator kinds).
- **nuts (c2-config-parts-nuts):** on main Sweep 1 volume = live (135.2676), faces 29 = live; one face type off: live's
  thread crest is an exact cylinder, ours OCCT's B-spline (also Revolve 2, Part Number 1). nuts3 59df02227 stays
  dropped. WIP core-loft branch core-helixcyl 9c5711eab turns a helical sweep's crest/root into exact cylinders (kernel
  T1 a_helical_teeths_crest_and_root_are_cylinders passes, Pappus volume) but OCCT's Boolean of a cylinder face wrapping
  22 turns is invalid (thread::helical_teeth_add_to_the_core): parked. Probes vibex8 ~/tmp/core/nutsrec (ftypes, cylfit).
- **Wave-washer ring: core-rp branch core-ring5 fc8b33664 (committed, NOT landed):** sections = every profile carried by
  a rotation-minimising frame, mixed with natural-spline weights over the profiles' arc places (first profile again at
  s = total), stations total*i/(N+1) (core-scratch/ring/nat.py: live's sections to 0.06 mm); skin = natural_loft
  periodic (periodic_seam_derivative clamps both ends) at chord-length params; ThruSections iso unsmoothed first.
  Fix inside: natural_loft read a REVERSED face's closed edge pcurves swapped (CurveOnSurface flips, UpdateEdge
  doesn't): now read on the face taken forward (affects every natural loft with reversed faces: suite pair needed).
  Doc: 7/7 match, Loft 1 2079.32 vs live 2080.67 mm3, 109 ms (was error / 1453 ms); vs live's faces max 0.163 / mean
  0.014-0.064 mm (bar 0.1: DECISION ASKED of the lead 23:20). Residual = live's knots (.1722 .2519 .3339 .5 .6702
  .7504 .8319, asymmetric) vs our chord (.1694 .2506 .3322): rule unknown (kn.py / kn2.py). Kernel T1 features::
  a_loft_round_a_closed_path_mixes_its_sections_from_every_profile. SUITE PAIR running 23:20 on vibex8: branch
  ~/tmp/core/g/run-ring5.log, base (core-cache detached at 2f9ab4c4c) run-base5.log; local log
  /root/core-scratch/ring5-suite.out. Compare SAME-PER-DOC + validity, then g++-13 build, then land if the lead OKs.
  Probes: vibex8 ~/tmp/core/{ringl,ringsurf,pcv,chk,spar} (src core-scratch/brepq/).
- **asm isaac a2e4a2 Chamfer 2: CLOSED 11-19 (asm: Part 1 39921.44 mm3 fits live on main; the rest is asm's connectors).**
  PROBE lines `// PROBE` in features.cpp (nl false / ok valid, ring wires, ringloft dump) to strip before landing.
  Tools: vibex8 ~/tmp/core/ringl <wires.brep> ring-live-surf.txt (src core-scratch/brepq/ringl.cpp), .r5.sh. Bar 0.1 mm
  (0.17 would need the lead: decision).
- **11-19 afternoon:** guided-loft conditions e02fe92f (queued/landed: Normal to guide aslant -> BSURF_INCONSISTENT text;
  start/end conditions checked against guides; T1 a_guided_loft_with_guide_and_end_conditions_as_live). f197 closed
  (Mirror 3 splits one B-spline edge at (1.2061,39,20): DECISIONS 11-16). surf-intro Derived 1 +0.1 in = asm's
  90d254008 (handed to asm; live's body differs, can't judge). qa re-saved cpcurves/mutualtrim: left cpcurves Split 5
  Info + extra 1227.58 mm3 part (= Split 3 keeps the wrong piece) -> after pipe.
- **Re-queued 21:20 b9d47a3d** (failed 18:07 on the batch partner's coverage row "partStudio:Row B", not ours).
- **Pipe Loft LANDING 11-19 (lead: within 0.1 mm of live where ours errored lands, DECISIONS 11-19):** core-loft
  core-nloft2 9a112af34 + checklist 2d162d3e8: loft_guided_open through > 2 open profiles: a natural cubic across the
  profiles (live: zero 2nd derivative at the end profiles), each profile paired by its own curve parameter (live's section
  points sit at parameter share u), a guide's derivative ignored (live2: None / magnitude 2 / 10 identical poles); start /
  end conditions there still refused. 0.093 mm max two-way off live's face, area 600.16 vs 596.66 (the tube's curvature
  turns 0.05 mm into 0.6 %); kernel T1 a_surface_loft_through_three_profiles_along_curved_guides_is_live_s_face (live2's
  curves, boundaries < 1 um, interior < 0.1 mm). Parity row stays: live skins ~13 sections of its own rule with C1 cubics
  (rest in v .55-.75, mid u). Tools: vibex7 ~/tmp/core/livesurf, projpt (BRepExtrema glitches 4.7 mm here), devlive.
- **singularities Mirror 3 QUEUED 21:25 072505c2:** a surface mirrored / patterned with Add joins the surfaces it
  touches (merge scope + seeds) into one sheet (live: one 2-face sheet); singularities 2 -> 5/15, 30-doc subset otherwise
  same (vibex7 ~/tmp/core/glm.log vs glsl.log). T1 features_a::a_surface_mirrored_with_add_joins_its_mirror. Next there:
  Loft with bad singularity (ours errors, live 3425 mm2 sheet).
- **asm a2e4a2 Chamfer 2 (isaac):** blend's bb667320d removed its state difference (see asm isaac 23:00 above). a2e4a2 left: Chamfer 4
  "Failed to chamfer selections." (blend's?), Extrude 15/16 (vibex7 ~/agents/asm-keep/suite/h1119-asm-isaac-ball.out).
- **qa compat 11-19 17:0x answered:** split's corpus copy had Split 2's selection saved while Extrude 1 failed (qa re-saved,
  12/12); singularities Thicken 20 084.5 mm3 exact for our input (22 880 - 90 deg concave corner 2 795; 7dd227351).
- **asm 11-19 FYI (asm-suspension, studio geometry, mine when reached):** 805e29 Fillet 3/4 fail -> Knuckle a 0-volume
  sliver; 995636 Body two parts (90 419 + 4 712) vs live one 109 049 mm3, Cup / Cap off. Trace vibex7
  ~/agents/asm-keep/susp.out.
- **Volume measure warning:** on impeller parts OCCT's plain Gauss VolumeProperties is ~1.2 % off (input 97755.8 vs
  mesh-converged ~96555, kernel props 95977.8 after the delete ~= mesh): the old "OCCT 97123.4" targets were plain
  Gauss. Compare with kernel props (replay prints them) or vibex7 ~/tmp/core/imp/mvol (mesh).
- **Fan-blade P0 (fix forward):** main after core-oc 1484 ms vs 0.4.56 730; Boolean 2 877 vs 122 (live 364). bool's
  b5b566884 drops the wasted Rust attempt; my lever: per-tool offsets' edges 52 + check 24 ms (NOSHAPE_OFFSET_DEBUG=1).
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
  (skasm's).
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
00. SPEED-VALVE-CONFIG land; then KNIFEHANDLE-LOFT1 (P1, In flight); then DIRECT THICKEN (own ticket); then Face blend 1 (on top); then projected-curve-enclose Mutual trim 1 (ours
   174 ms): speed ticket if >=100 ms and >=2x live's (lead 23:5x); then the thin wrapped-disc fuse (P2, In flight).
0. Impeller P0 (In flight): land Extrude impellerraum's split walls after the pair, then Delete face 2 on our input and
   the local rebuild, re-timed on the Mac vs 12 ms; then the In-flight queue 1-7.
1. Wave-washer ring (In flight; live2's sections capture), then singularities Loft with bad singularity / Loft 3. Then: Fill G0 = intro-examples' Fill 1 (live's fitted 14x18 surface),
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
- Agent ids: /root/lead-scratch/roster.md only (read it before every message_agent; never copy ids into notes).
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
