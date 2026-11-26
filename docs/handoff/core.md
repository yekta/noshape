# Core modelling (agent "core"): current state

Updated 11-26 19:56 UTC. Mac: usable only through its ONE 4-job nice-19 slot (DECISIONS a16b39297, NOSHAPE_RB_HOST=mac waits for it), never overloaded; default builds on rb-hosts (core* -> vibex7). Agent ids: only /root/lead-scratch/roster.md, read before every message (never copy ids here). History (dated notes up to 11-15 14:48): docs/handoff/archive/core.md. Standing rulings:
docs/DECISIONS.md (read first; it wins over anything here). Land NOSHAPE_LAND_QUEUE_HOST=vibe2 tools/tiers/land.sh
(vibe2 is the user's own server: only ~ngyekta and /opt/occt). Never kill a process tree from a computed pid.

## P0s (lead 11-26; block v0.4.72)
- **9cbb0e FILLET 18 (lead 19:2x; IN PROGRESS 19:5x, uncommitted in core-rf)**: live's geometry IS in the corpus
  (studios/9cbb0e….live.jsonl line 71, faces with surface+box, edges with curve+mid). Face 0 is a TOP face (normal +z),
  wall 80 normal +y: edge 1 (0/80) CONVEX, 40/24 (80/81, 80/58) CONCAVE. Live: concave rounds 24/40 run to the tip
  (STBCC cyl, STBqB torus); edge 1's convex round STBKE ends at x -19.12 (wall height = 2r); from there the convex
  ball rolls between face 0 and the concave round: 2 OTHER faces (STBSE over the cyl, STBOE over the torus, ellipse
  edges); wall 80 trimmed to x -19.21. Mirror at y +11 (wall 49). Probe zz_probe_f18f (kernel crash_inputs, not to
  commit): stage 1 fuse a box above face 0 on the wall's material side (wall raised) + OCCT concave rounds: valid;
  stage 2 cut a box above face 0 over wall+round: valid (108 faces); stage 3 OCCT on edge 1 + creases FAILS (crease
  face0/torus vanishes tangentially at the tip (-15.09,-10.2)). Code: new model-core features/cusp_blend.rs
  (cusp_wall_round, hooked in detail.rs fillet Err chain before fillet_deferred_overflows; mod.rs) doing stages 1-2
  then OCCT, then rolling_ball_blend on the rest; build+run .bdc.sh (core-rf, prints Foot volume/faces after Fillet 18
  with NOSHAPE_BLEND_DEBUG) 19:54: STILL 'Failed to fillet selections.', 95 faces, NO 'cusp round' line: the hook isn't
  reached or cusps() finds none -> check first (that message may come before the Err chain: round_overrun?). If rolling balls fail too: own tool = cut
  box from J (ball touching face 0, wall and the concave round's wall contact line) to past the tip so OCCT ends edge
  1's round on a square plane; fill back common(S2, box) less a loft of exact ball sections (offset plane ∩ offset
  cyl r 2r / torus minor 2r), 2 lofts (cyl part, torus part), stop just short of the tip; fuse + unify. Target 141
  faces, 64650.834. Then Fillet 19/23, MF7, Chamfer 11.
- **TRANSFORM-TRANSLATE-DISTANCE (lead 11-26 19:0x): QUEUED 01c87462 19:16** (core-rf, carries Fillet 17; qa + lead told): my a5e0075bb sent Move faces
  of planes facing several ways down the one-way-at-a-time tweak (neighbours keep names) instead of OCCT's offset
  (every face Offset(old)): the derived source's Move face 1 vertices renamed, Part Studio 1's Transform 1 by two
  vertices lost them. Now several ways only after OCCT's offset fails (before the booleans); reblend fallback as was.
  T1 features_a::move_face_offset_of_planes_facing_two_ways_keeps_occts_vertex_names (fails without). Both docs
  ALL-SAME vs 0.4.71, corpus A/B only those 2 change (vibex7 ~/tmp/core/tr, mode 700: delete when landed).
- **SPLIT-NUMBERING-DUCT-MIRROR: QUEUED** 7774546b0 (core-p0; ids keep v0.4.71's order, names live's; test
  features_a::a_split_part_two_extrudes_made_keeps_its_pieces_ids; qa's userdocs.py duct = 0.4.71). Duct Split 1
  part NAMES still differ from live (later).
- **IMPELLER-LOFT-REGEN-TIME: QUEUED 60689091 18:2x** (core-p0): unchanged() by volume_change_estimate first,
  boolean.rs glued fuse volumes only for a solid (estimate then exact), face_reach probes "on" by the normal offset;
  T2 ops parity::parity_impeller_loft_regenerates_in_time (< 40 s). vibex7 load 3: 24.2 -> 23.0 s (0.4.71 ~22 s);
  corpus 127 ALL-SAME. A settle tol 1e-10 try changed knifehandle Fillet 2 (reverted). Left (profile 18:3x, DIST_T
  instrumentation not committed): Gehaeuse 6.4 s = OCCT op_fillet retries + project_point_near fallbacks (267 x 4 ms,
  blend_section Support::Face when Newton's foot leaves the face) on Move face 2's new (live) geometry; live does
  Move face 1, Delete face 2 and both fillets OK -> the real fix is making them succeed (parity work, not speed).
## Now (11-25)
- **LANDING core-rf 11-26**: 2b8aa8ed LANDED 15:11. QUEUED 0b615541 15:54: 56131f3c (9cbb0e MF2, b58244 MF6, tweak trim fix) + be8307ce4 (speed: inside-out fillet repaired before the retries; tool_only seed re-runs).
- **FILL 1 DONE (1916587c4)**: blend_section.rs pinch_corner + fill_pinched_corner (live's 3-edge corner). Left: our G1
  fill 120.347 vs live's G2 121.947 mm2 (OCCT plate declines G2); a fair G2 fill fixes our Fill feature too. Parity docs
  need a FRESH parity run (save/ copies keep old references): .bd2.sh.
- **body-draft doc now** (fresh .bd2.sh 13:2x): all 14 OK; Fillet 1 edge kinds 19/2 vs live 15/6 (vol 39073.008 vs
  39072.989); Fillet 2 -6.46 mm3 (G1 fill); Draft 2 12 faces/27 edges vs live 13/28 (one face short): next in this doc.
- **SCISSOR 11-26 15:0x (in 56131f3c)**: (a) detail_a.rs blends_to_reapply: the spread
  to "blends touching removed blends" skips a LARGER cylinder met along a line (the round's support): 9cbb0e MF2 now
  live's 44 faces, 109400.892 (live .89). (b) direct.rs tweak_planes_in_turn: planes facing several ways (all
  neighbours planar) tweaked one way at a time; also the fallback in move_face's reblend closure when OCCT's offset is
  invalid (tweak_planes composes histories with new kernel History::then): b58244 MF6 OK (was Error). (c) features.cpp
  FaceTweak::prepare: a staying edge with a moving end on a TRIMMED unbounded/periodic curve (an earlier tweak's
  edge) gets its basis curve back -> re-trimmed (box top then side: 1440 vs 1560 mm3 before). Tests: features_a
  move_face_offset_rolls_a_round_again_on_the_arc_wall_it_runs_along (fails on old code), move_face_offset_of_floors_
  facing_two_ways (guard only: synthetic OCCT offset stays valid), kernel offset::histories_followed_through_two_tweaks.
- **Fresh scissor run 15:3x** (.pa2.sh compares d-prev = 14:37 with d-now): only change b58244 MF6 fixed; NO new rows,
  so the downstream rows are real (the import matches refs against our build). Tab ids now: 404ddb 544d4be22b1974ab,
  9cbb0e ba43fd52006aef05, b58244 ef415f8715d2b8a5, 2d82a0 0e42b5e59678dc59, 92138f c945c4d3bdad506d, b7e381
  f44c9b9bedd247e3 (st.sh / mfq.sh / dmp.sh; ~/tmp/core/fb/cli = cli-new 15:40). 404ddb Fillet 2 = live now
  (73853.40 vs .06; lead item 1 done). Sorted:
  (a) b58244 (~70 rows): IMPORT GAP, not geometry: live Sketch 3's sketchPlane is a qCompressed query with no
  deterministicIds and no refs.jsonl row; we fall back to Front (y 0) where live's is a face at y -2 mm, so Extrude 4
  (+0.5 mm boss, live +61.3 mm3) is contained (Info) and Plane 1.. cascade. Ours = live through Move face 1 (21721.72).
  Reported to the lead (capture/importer owner). live 16:0x added 24 refs rows: rerun 16:21: Extrude 4 = live now
  (Sketch 3 on its face; ours = live through Transform 1, 56055.30); first failure now Plane 1: ref KZGB box at y = -2
  (point (52.516,-2,25.800)) but our Extrude 4 cap at that x,z is at y = -2.5 (boss -2..-2.5, volume = live): import
  says "no face of ours". live 17:0x: KZGB NOT stale (recaptured = 11-21): in live by Plane 1 Extrude 4's cap is a 1.6 mm
  rim at -2.5 (JZG 314 mm2) around KZGB (1516 mm2) at -2: a 0.5 mm recess; ours one cap at -2.5 -> look at Move face
  2/3/4/6, Fillet 1/2 (idx 8-13) for the face that moves the inner region (mine, after the P0s).
  New b58244 tab 22490dcfa787b5a5.
  (b) 404ddb Hole 2 (then Mirror 1, Split 1, Extrude 5-8, Booleans): Ø8.2 x 42 blind coaxial with the r14 bar from its
  x 59 end face; the input = live (73840.04 vs .83). Ours "Hole creates invalid geometry": the profile shows only the ray
  common (boolean.rust) and op_revol, no cut recorded or OCCT build: looks like the own Boolean's cut errors with no
  fallback. Handed to bool: vibex7 ~/agents/core-keep/hole2-bool (README, mode 600 copy).
  (c) 9cbb0e: Foot = live through Fillet 7 (64907.55); Fillet 8 -0.017, Fillet 12 -0.017; FILLET 17 is the first real
  diff: r 8 on ONE line edge (x -15.54, z -27.68 mm, along y, 16 mm), live 79 -> 80 faces / -154.8 mm3, ours 79 -> 83 /
  -250.7 (vibex7 ~/tmp/core/sc/foot-live.txt; dump ~/agents/core-keep/f17-9cbb0e/fillet-16.brep+txt = edge 177 r 0.008).
  FILLET 17 LANDING 11-26 18:5x (fillet_over_smooth_faces in features.cpp, after square_corner/rust_blend in the
  checked chain): ball on the kept face's plane EXTENDED + the next face, prism of the corner cut + unify touched:
  80 faces, 64593.2110 (live .228). T1 kernel blend::a_round_running_off_a_nose_onto_its_tangent_hollow_rolls_on_it
  (main: 210.6 mm3 taken vs 164.5). Corpus 127 ALL-SAME.
  9cbb0e now (cli 18:42, tab 53427fa107a5c69d): Fillet 17 = live, Fillet 16 = live (64528.94 vs .96), input of
  FILLET 18 = live (95 faces, 64657.748 vs .765). FILLET 18 (r 0.8, 42 edges; live 95 -> 141 faces, -6.93 mm3) fails.
  Per edge (probe core-scratch/f18-probe.patch on vibex7 ~/tmp/core/rt/fillet-18.brep+txt, output
  core-scratch/f18-per-edge.out): the y +-8 chains OK alone (100 faces, -4.754), y +-11 edge 11/39 OK (98, -1.84),
  31/33 OK (+3.49); the y +-11 chain through Fillet 17's round (x -78..-15, edges 7,8,9,10,12-17 / -11 side 1,6,20-24,
  35,40) FAILS. Probe 2 (core-scratch/f18b-probe.patch): all 42 edges are 90 deg, walls y +-11 = faces 49/80
  (planes), the chains on them split at sharp corners: -11 side {1,6,35,40} "fillet failed", {20,21,22,24}
  "invalid solid", 39 OK. Edge 40/9 = short arc (x -15..-17, z -28) on cylinders 81/52 (y -8.25..-11 / 7.75..11,
  Mirror 6/3's copies of the Fillet 17 region), meeting line 1/10 and line 24/8 there.
  19:2x probes: core-scratch/f18cde-probe.patch (f18c/d/e.out); state in the P0 entry above.  (d) 92138f MF1 / Replace face 1 (skasm 15:4x: geometric, mine). (e) Drafts 2d82a0 D1,
  b7e381 D2. skasm 15:4x: 2d82a0 "Position" was missing imports (ac4efa6a), not a regression.
- **REPLACE FACE 1 (bool's, c2-replace-face-exercise)**: landed 83082091 (reblend adds curved edges first: 4 spheres).
  Left -2 mm3: live keeps the RIM; our blends_to_reapply propagates floor fillets -> spheres -> vertical fillets -> rim
  and rolls it again. Stopping at the vertical fillets needs their r 12.7 = floor r degenerate case. Probe:
  core-scratch/refillet-probe.patch (kernel crash_inputs zz_probe_refillet on NOSHAPE_DUMP_OPS fillet-3).
- **LEAD LIST 11-26 06:4x**: (1) bracket03 Chamfer 1: QUEUED 43ad3cc4 (core-spd; Chamfer 1 ~40 ms = 0.4.68; was 87-170;
  7 op_chamfers = 1 + 6 trials; c927 landed 15cff2b3f): chains_overlap takes `together`'s OpResult and
  skips pairs whose chamfer faces (history generated from the chains' edges) share no edge nor end within 8 d on a face along neither chain
  (cli-hist). (2) knifehandle settle fix + T1 settle test: core-rf tip commit (land.sh after 66dba573); slow cuts
  handed to bool 06:5x: vibex7 ~/agents/core-keep/kh-bool (Fillet 3 cut 244416f8 667 ms, Fillet 1 cut 856a411e 3.2 s).
  (3) Scissor Delete face 1 = live with 66dba573's merge; recheck 97ad5f Linear pattern 1 after it lands. (4) loft-guide Sweep 1: CLOSED, not a target (live's approximation; archive).
- **KNIFEHANDLE FILLET 3**: settle fix (core-rf tip) (1380 -> 1100 ms; live 11). Left: run-on cut (bool's, above), 4 op_split
  ~225 ms (closed/trimmed run-on), 1632 projections ~360 ms. Probe: gdb -x ~/tmp/core/spd/w/k/g.gdb.
- **LEAD 11-26 08:0x: blend FAILURES first (ranked by documents hit), native blend faces only planned, then skasm's
  sweep items.** Impeller's 1432 ms final cut handed to bool (08:0x; vibex7 ~/agents/core-keep/imp-bool/ed1c251b6b5acedd).
  Ranking (gate cache vibex7 ~/agents/bool-gate/cache/9ad08dd0740c5c08-5839ab362e0e-suite.tsv = main + merge, 200 docs;
  qa gaps-1125 models-report.json): each failure hits ONE document; order = own first difference first:
  (1) DONE (6ea74721): c2-surfaces-lofts-projected Face blend 2/3 (9 -> 18/45); next diff #18 Sweep 2 (sweep item);
  (2) c2-body-draft-naming Fillet 2: LANDED 281ced578; Fill 1 DONE 1916587c4 (above). Old notes: live's corner is 3-sided
  (A (50.634,14.575,63.877), B (50.634,21.818,71.12), C (59.634,23.575,62.12)): the middle round (along Fillet 1's
  end curve on the right face, setback 9 > its 5.2 radius) PINCHES to C, where the neighbours' contact lines meet;
  BC / CA lie on the neighbours' extrusions (live's mid points: x,z (resp. x,y) on their profiles, the third
  coordinate linear). Ours clips at 0.9 rho: 4-sided corner + 0.17 mm2 sliver + a crease to C. TRIED 11-26 (patch
  core-scratch/pinch-probe.patch): middle sections pinched to C (pinch_point = neighbours' contact lines' closest
  point; exact C) -> loft valid but tolerance 0.12 mm at the apex, every cut order open/invalid; fuzzy 1e-5 cut =
  live's 12/27/17, 38249.2 (live 38239.5) but tol 0.12 mm; a hair off C: open, or 1.3 s and Fill 1 still fails.
  => needs the corner as its own face (native blend face work). Doc's first diff: Fillet 1's edge kinds (live 'other').
  (3) c2-cov2-face-blend-partial-fillet: QUEUED f8890c9d (Fillet 3 OK; left FB1/FB3/asym volumes = live's fits, not
  targets; F3 17 faces vs 16: sliver at the curve/top-edge joint). T2 std_oracle boolean/loft/transform fail on error
  texts ("Cannot resolve entities." / guides Info), not blends: check on main. Probes vibex7 ~/tmp/core/fb (pf.sh, qp.sh).
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
  fallback. Size: ~1-2 weeks; confirm per-phase timings first (NOSHAPE_KERNEL_PROFILE; gdb kit ~/tmp/core/spd/w/i).
- **QUEUE**: Replace face rim (above), blend failures (ranking above), then skasm's sweep items.
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
   a-d. 2d82a0 C2 landed 45db5d099; 184ad0 F3/F5, C6 build now. Ranking now from the 11-26 run (Now: SCISSOR RUN). Probes:
      core-scratch/sc st.py (failing features per studio), fe.py (params + edges); NOSHAPE_DUMP_OPS=<dir>
      NOSHAPE_DUMP_ONLY=chamfer dumps blend inputs. Saved doc: vibex7 ~/tmp/core/sc/fresh/.
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
