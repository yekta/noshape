# Core modelling (agent "core"): current state

Updated 11-26 06:5x UTC. Mac: usable only through its ONE 4-job nice-19 slot (DECISIONS a16b39297, NOSHAPE_RB_HOST=mac waits for it), never overloaded; default builds on rb-hosts (core* -> vibex7). Agent ids: only /root/lead-scratch/roster.md, read before every message (never copy ids here). History (dated notes up to 11-15 14:48): docs/handoff/archive/core.md. Standing rulings:
docs/DECISIONS.md (read first; it wins over anything here). Land NOSHAPE_LAND_QUEUE_HOST=vibe2 tools/tiers/land.sh
(vibe2 is the user's own server: only ~ngyekta and /opt/occt). Never kill a process tree from a computed pid.

## Now (11-25)
- **LANDING core-rf (11-26 06:4x)**: a730d3fd FAILED 02:30 (rebase conflict, parity.rs: main's LiveRef `body`
  beside my `created_by`, kept both); rebased with the zero-length plane merge (f378eeb7b + 714337378) on top, one
  land.sh. Solver/cPlane/translator: models gate 131 -> 132/189 (lora ribs 39 -> 46/46), no doc worse.
- **LEAD LIST 11-26 06:4x**: (1) bracket03 Chamfer 1: QUEUED 43ad3cc4 (core-spd; Chamfer 1 ~40 ms = 0.4.68; was 87-170;
  7 op_chamfers = 1 + 6 trials; c927 landed 15cff2b3f): chains_overlap takes `together`'s OpResult and
  skips pairs whose chamfer faces (history generated from the chains' edges) share no edge nor end within 8 d on a face along neither chain
  (cli-hist). (2) knifehandle settle fix + T1 settle test: core-rf tip commit (land.sh after 66dba573); slow cuts
  handed to bool 06:5x: vibex7 ~/agents/core-keep/kh-bool (Fillet 3 cut 244416f8 667 ms, Fillet 1 cut 856a411e 3.2 s).
  (3) Scissor Delete face 1 = live with 66dba573's merge; recheck 97ad5f Linear pattern 1 after it lands. (4) loft-guide
  Sweep 1: CLOSED not a target: our path within 4 um of live's 801 samples (core-scratch/lg cmp.py/fit.py; centripetal
  natural is the best of 8 fits), live's sections bulge to r 0.0275-0.028 at the 12 tight bends (behaviours 11-18) =
  live's approximation; REQUESTS row withdrawn.
- **BOOL GATE 2 PASSED (06:17)**: 420fd5d01 (/root/core-rf-scratch/gate-420fd5d01.log): replay +0 -0 (parity56
  513 -> 513, 0 differing), suite 132 -> 132, features 2106 -> 2106; its 1 "changed" doc is lora's untranslated note
  gone (translator, 46/46 both); suite total 397 -> 624 s = load (branch ran at ~18, main cached; median doc 1.7x).
  Landing with core-rf. Next: 97ad5f Linear pattern 1 (PART ADD x6) fuses copies live keeps apart: recheck.
- **P0 SPEED**: c927ff4e QUEUED (core-spd): hose, radiator = 0.4.68; profile kit vibex7 ~/tmp/core/spd (m/ models,
  pft.py, spd-prof.sh, ~/qa-rel/<v>/noshape-cli). Impeller: no regression on main (old-base test builds).
- **KNIFEHANDLE FILLET 3**: settle fix (core-rf tip) (1380 -> 1100 ms; live 11). Left: run-on cut (bool's, above), 4 op_split
  ~225 ms (closed/trimmed run-on), 1632 projections ~360 ms. Probe: gdb -x ~/tmp/core/spd/w/k/g.gdb.
- **LEAD 11-26 08:0x: blend FAILURES first (ranked by documents hit), native blend faces only planned, then skasm's
  sweep items.** Impeller's 1432 ms final cut handed to bool (08:0x; vibex7 ~/agents/core-keep/imp-bool/ed1c251b6b5acedd).
  Ranking (gate cache vibex7 ~/agents/bool-gate/cache/9ad08dd0740c5c08-5839ab362e0e-suite.tsv = main + merge, 200 docs;
  qa gaps-1125 models-report.json): each failure hits ONE document; order = own first difference first:
  (1) c2-surfaces-lofts-projected #9 Face blend 2 "Failed to blend faces." (its first difference) <- NEXT;
  (2) c2-body-draft-naming #7 Fillet 1 edges 19 line/2 other vs 15/6, then #8 Fillet 2 fails (BLEND-FAILS e);
  (3) c2-cov2-face-blend-partial-fillet #6 Face blend 1 edges 19/2 vs 17/4, then #15 Fillet 3 fails;
  (4) asm-scissor-lift studios (BLEND-FAILS b-d below); after upstream fixes: genesis #72, impeller Stevenrohr #29.
- **NATIVE BLEND FACES PLAN (for the lead's later decision)**: scope: section_blend's rolling-ball path (blend_section.rs
  ball_tools -> 35-section loft tools -> closed_run_on splits -> Boolean with the body) replaced by the blend face
  itself: a B-spline surface fitted to the ball sections (contact curves exact on the supports), trimmed at the
  contacts, sewn into the body with the supports trimmed there (no tool solid, no split, no Boolean). Removes: the
  final body Boolean (impeller 1.43 s OCCT, knifehandle 0.4-0.67 s), the run-on splits (impeller ~1 s, knifehandle
  ~0.3 s), tight boxes/distances of run-ons (impeller ~1.7 s summed). Cases: conic and circular rolling-ball rounds
  OCCT declines (impeller Fillet 3 5.1 s vs live ~0.1; knifehandle Fillets 1-3 ~1.1-4.5 s vs live 0.01-0.1).
  Risk: trimming the supports at a fitted face's contacts when the run-on ends on free-form faces (the cases
  closed_run_on handles now: walls, corners, ties) and validity of the sewn shell; the section path stays as the
  fallback. Size: ~1-2 weeks; numbers to confirm first: per-phase timings with NOSHAPE_KERNEL_PROFILE on both docs.
  Profiling notes: Booleans via NOSHAPE_RECORD_BOOLEANS; op_split 14 (10 closed_run_on); 133 min_distance =
  closed_run_on's threaded `touches`; tried w/o gain (reverted): reuse tie cuts, vertex side of run-on pieces.
  gdb kit vibex7 ~/tmp/core/spd/w/i g2-g4.gdb, b.jsonl (rollback 30).
- **QUEUE**: blend failures (ranking above), then skasm's sweep items.
- **SCISSOR 264eb3 (11-26 01:21)**: source = live per feature (Chamfer 1 1606.626), suite 942 s, 24/41 mates, 64/87
  BOM; the 90-min stall was 97ad5f's pattern fusing wrong parts while the source was off, not a hang. Delete face 1
  matches live. Saved doc vibex7 ~/tmp/core/sc/new; probes core-scratch/sc pf.py (per feature), fc.py, nm.py (normals).
- **qa 11-25: MESH-UP-TO-FACE landed (fixed, to verify); DL-STL-MESH-MOVE-FACE waits on live/core/mesh-offset-1125
  (REQUESTS 11-25 20:0x: our convex rounds finer than live's: +0.085 % at 1 mm; Translate's message with a plane dir).**
- **then skasm's sweep items (1124-2341, 11-25 16:2x)**: (1) Tail Fin Up-to-vertex depth 67.880676 vs live 67.881480
  (c2-surf-intro-boundary/fill/mutualtrim/ruled; -offset's Offset area): Derived 1 car body's tail vertex x 33.940348 vs
  33.940739; doc vibex6 ~/tmp/skasm/fin/c2-surf-intro-fill.noshape. PARKED 21:0x: it is the source's Fillet 1 (R 0.6 in,
  edge Jdh, ends on the y=254 end face): ours +33.940348 / -33.940329 (live +-33.940739); Loft 4's edge end matches live
  (50.517295). Every build is invalid, retries (1-1e-7..1e-5) too: the kept one is repaired() of the full radius; ChFi3d
  10x tighter SetParams gives the same vertex: OCCT's stripe end on the face, not the walk. Probes ~/tmp/core/fin f2-f4.py. (2) c2-surf-intro-simple Loft
  1: 1 face, live 4 (4-segment profiles). (3) c2-wrap-text-cylinders Wrap keeps lines (86 line; live 6 line/270 other).
  (2) 11-26 02:0x: ours 1/8/8 faces/edges/vertices, area 19194.80 vs live 4/12/8 19485.60 (live one face per
  profile segment: 5922/3910/5492/4161 mm²): our profiles do have 4 segments (8 vertices), the loft makes one face
  through them (kernel ops.rs loft -> features.cpp ThruSections; look for where wire profiles' edges become one
  section curve). Saved doc vibex7 ~/tmp/core/si. (4) c2-hunting-knife Revolve 1 18 vs 19 vertices: shown_topology (common.cpp) shows no vertex on the closed ring the
  sphere cuts in the loft tip; live likely one at the loft's seam (y=0). Doc vibex8 ~/tmp/bool/knife.noshape.
- **DL-STL-MESH-MOVE-FACE Translate (lead 21:1x P1)**: committed 5aab43cd7 (mesh Translate moves the whole mesh; T1 in
  features_b::move_face_offsets_a_mesh_part…). Tessellation (live 0.005 mm chord, 8/12 segments a quarter) is low
  priority: BRepMesh deflection 5e-6 gave 10592.119, angle-driven 10592.355 (live 10591.438): needs own chord control.
  Inward remesh chamfer: not a target (lead).
- **P1 MESH-MOVE-FACE (lead 11-25 14:4x, qa flow 5, live qa/dl-bracket-1125)**: Move face Offset on an STL part (1 mesh
  face) offsets the whole mesh: kernel `Shape::offset_mesh` (mesh_io.cpp: facets unified, OCCT sharp offset, then its
  convex edges filleted at d run by run, meshed back). qa's bracket: 1 mm 25822.709 (live 25800.854), 2 mm 35387.538
  (live 35320.926): the holes' facet rims stay sharp (OCCT can't fillet them). Delete face already removed the part.
  T1 mesh_import::a_mesh_body_offsets_whole_with_rounded_convex_edges, features_b::move_face_offsets_a_mesh_part_….
- **Scissor parity 11-25 14:3x** (core-scratch/pa-now-1125.out, saved doc vibex7 ~/tmp/core/sc/fresh): 2d82a0
  Chamfer 2 fixed. New vs the slabs run, not from the chamfer (it only runs after OCCT fails): fed22d Extrude
  18/28/38 + Fillet 18 + Move face 7, 97ad5f Extrude 2/Boolean 2/Fillet 3/Move face 1, 665cf3 Fillet 2/3, 57725f
  Fillet 2: from main since then; check before working b–d.
- **HOLE vs live hole-1125** (lead 11-25 13:4x): probe core-scratch/hole/hp.py (40x30x20 block, face + Top-plane
  sketches, via MCP). Same as live: lists (T1 hole_tables::tests::lists_are_lives: 170 inch / 157 metric drills,
  42 taps M1-M68, PEM® only Simple, defaults), tap drill M5 4.2, merge scope = the part on face and plane sketches,
  callout title/list name (interaction::hole_name), cbore/csink/tapped volumes. Fixed: Tap type labels (QUEUED
  15da59b1). Live's 8 mm depth was the account's remembered value (live hole-remember-1125: every setting of the last ✓
  Hole, Esc nothing, hidden Blind depth kept): ours already does all of it, no change. Not captured: preview, errors.
- **STEP-EXPORT-FACE-COLOURS: QUEUED f87d9280** (lead 11-25 13:1x; I own export while dx is parked). step_out.rs
  styled_solid(): per face OVER_RIDING_STYLED_ITEM over the solid's STYLED_ITEM (STYLED_ITEM without a part colour);
  StepOptions/ExportSettings.face_colors (per part, MapShapes face index, sRGB) from BodyDisplay.face_colors
  (export::part_face_colors; ops export_parts + Part Studio Export). Mac-mini-2024: 166 + 2 styled items, re-import
  152 + 14 face colours, as live (private copy deleted). Left: face appearances (P2 above); COLOUR_RGB 17 vs live 14.
- **P0 FLOW-HOLE-ON-FACE-SKETCH-NO-SCOPE: not a Hole bug.** qa's start "bracket-link" draws its R10 ends with ops
  `sketch_draw center_point_arc [[30,10],[30,0],[30,20]]`: scripted clicks went straight from start to end, a 180° turn
  the tool's sweep reads as clockwise, so both ends bulged in (region 885.8 mm² = 1200 − 100π, x ±30) and the point at
  (30, 10.2) sat in the bite: no part under it. Fix: ops sweeps the pointer through the CCW arc's middle before the end
  click (ops/sketch.rs). Tests: e2e a_hole_at_a_point_on_an_obround_links_top_face (area 1200+100π, Hole Ok, scope 1);
  GPUI a_hole_at_a_point_sketched_on_a_face_merges_into_its_part (the app's dialog fills Merge scope at the first pick).
- **live dialogs-1125:** Fillet r50 (test kernel_robustness::fillet_wider_than_its_faces_overflows_like_onshape, T1:
  y 10-30, z to sqrt(2100)-30, 7458.616 mm³), Chamfer 25 (T2 chamfer_wider_than_its_face_overflows_like_onshape),
  Shell 20 / Extrude 0 messages, Union disjoint Info, Part Studio Circular angle 0 OK: all as live. Open: 0° pattern
  makes 4 coincident parts, as live (REQUESTS (9) answered). Count 0: the app's field says live's "A parameter value is not
  valid." (feature_dialog VALUE_NOT_VALID); ops names the range. behaviours E13 "angle 0 → must be positive" is the
  assembly pattern.
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

## Waiting on others
- c2-config-parts-standoffs Revolve 1 (13/32): Sketch 3 solved 4 x 22 mm, chamfers on the axis side; live 3 x 24 (Length),
  outer corners chamfered (sketches.json). Already in skasm's handoff (translator takes Sketch 3 back). Variables OK.
- **c2-sm-rocker-enclosure-body 15/28 (stable now):** Extrude 6's pocket is ~2 mm off live's and differently shaped
  (top face 89.76 vs live 66.95 mm²) because Sketch 6 isn't translated as live: its PROJECTED constraint finds no model
  geometry, its circular pattern isn't kept, and the solve (1.97 mm) is taken back. Sketch translation, not modelling
  (skasm's).
- **1dc32b (asm-infento-subs):** Draft 1 = live on dx's corrected input (-1306.543 vs -1306.54 mm³; OCCT 0030 +
  draft work bound landed af81dda05). Left: Fillet 2 / 3 / 12, Chamfer 3 (blend's); Extrude 42 17.6 s (bool's
  Boolean); Mirror 2 2.2 s (unify, bool2's). Hole 4-6 time: f1f212d03 above.
- **asm-bjr-config 5d8bc1:** Sweep 1 landed 72248180. Kerf Compensation 1: live's corpus line is Manufacture=true
  (3 plain Ø2.2 holes, 3704.237 -> 3745.156 mm³, offset faces 454.835 -> 454.462 mm²); at false live leaves the part
  unchanged; ours offsets at false (dx's translation). Check our offset against the true numbers once dx has that input.
  Shell 1 is dx's. Extrude 55 (3.8 s vs 333 ms) is bool's fuse / unify.
- **c2-hunting-knife f24:** live's lines disagree (16-23 LINE, 24 OTHER); refetch 15-24 asked (REQUESTS 11-15 00:45).

## Next, in order
0. **BLEND-FAILS-1125** (lead 11-25 14:1x; blend parked): Fillets/Chamfers that fail in ours and build in live. Sources: model
   suite vibex7 ~/qa-parity/models.log (11:37, qa 1125-0803), asm-scissor-lift studios (my slabs run, vibex7
   ~/tmp/core/sc/pa-slabs.out). Skipped (parked): scissor 9cbb0e F18/F19/F23, impeller "Fillet Stevenrohr zum Halter".
   Own (the blend is its studio's first divergence), most ordinary first:
   a. scissor 2d82a0 Chamfer 2: FIXED, QUEUED 3a19cf4d (features.cpp chamfer_ring_fill: concave circle between a
      plane and a coaxial cylinder, chamfer wider than the face: ring fill clipped by the neighbours' outer sides;
      T1 blend::a_chamfer_wider_than_a_counterbores_step_meets_the_through_hole). Then Fillets 4-7/13/21/25/28/30/31/37,
      Chamfer 6: re-check on the fresh run (vibex7 ~/tmp/core/sc/pa-now.out, saved doc ~/tmp/core/sc/fresh/; the
      core-keep scissor.noshape is stale: Draft 1 / Move face 1 fail only there). Probes: core-scratch/sc st.py
      (failing features per studio), fe.py (a feature's params + edges); NOSHAPE_DUMP_OPS=<dir> NOSHAPE_DUMP_ONLY=chamfer
      dumps blend inputs (.brep + .txt edge indices) for a kernel replay example.
   b. scissor 184ad0 Fillet 5 (+ Fillet 3 "Unfilleted edge overlapped")
   c. scissor b7e381 Fillet 4
   d. scissor 9cbb0e Fillet 2 (then Chamfer 6)
   e. c2-body-draft-naming #7 Fillet 1 (builds, 15→19 lines / 6→2 other edges vs live) then #8 Fillet 2 fails (draft body)
   After an upstream difference (fix upstream first; re-check after):
   f. c2-lora-enclosure-ribs #19 USB C Port Chamfer Base (#16 USB Base Extrude Remove differs)
   g. c2-genesis-trunk-hanger #62 "3.7", #63 "1.5", #67 "3", #72 Fillet 1 (#20 snap-holes Boolean: ellipse edges)
   h. scissor cascades: 0139f2 F1; 665cf3 C1, F5; 264eb3 C1; 37a1a7 F3, F5; 404ddb F1, C1, F5, F14; fed22d F4, F5,
      F24, F25, F30, C2; 4ce5c7 F1; 57725f F3-5, C1; 5aae43 C1-2, F1-7; 97ad5f F1-2; b58244 (17, lost refs after Extrude 4)
   i. surfaces: c2-surf-workshop-knifehandle #31 Fillet 1 (#29 Mutual trim), britatap #21 (#8 Delete face), pipe #22/#31/#32
      (#9 Loft: sheet, live solid)
   j. c2-impeller-loft #38, #42, #47 (after #27 Fillet 3 differs)
0. SPEED-ASM-OPEN-SCISSOR: QUEUED 7d11bf0e 11-25 12:58 (core-rmx branch core-slabs, squashed; NOSHAPE_NO_SLABS switch
   dropped). offset_by_slabs (features.cpp): a moved face between square neighbours = part cut/fused with its slab,
   unify on a copy, every face's images GENERATED from it (as OCCT's offset: "Offset of" names; 665cf3 Extrude 1 ends
   on one). Fresh-import parity_assemblies asm-scissor-lift (vibex7 ~/tmp/core/sc/pa-{slabs,occt}.out): 13 feature
   differences fixed (56d3a6 Delete faces/Fillets/Chamfer 2/Extrudes, 9cbb0e Fillet 6), mates 16 -> 21/38, none new.
   Corpus A/B 127 docs ALL-SAME (~/tmp/core/ud/ab-slab.out). Left: 9cbb0e Boolean 4 / 404ddb Boolean 2 "union lost
   one of its parts" (both modes); unify can't merge a fuse slab's walls with rounded-corner cylinders / hole walls
   (slab path refuses, OCCT fallback runs); Delete face 2 (op_defeature 1.15 s). Probes: core-scratch/sc fn.py/fn2.py,
   dec.py (face ids: urlsafe-b64 raw deflate JSON).
0. ERRORS-1123 rest (live/errors-1123/*.md; audit /root/core-scratch/errors-1123-audit.md). Never add errors for live's
   "succeeds" cases (Revolve 0°, Move face through the part's top, cylinder collapse).
00. KNIFEHANDLE-LOFT1 station rule when loft-stations3 (J, K: profile off its guides) is in; then Face blend 1 (on top); then projected-curve-enclose Mutual trim 1 (ours
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
  reach the host: put them in the script). No Mac builds (user 11-21); rb-hosts picks vibex7 for core*.
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
