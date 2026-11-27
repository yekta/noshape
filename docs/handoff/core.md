# Core modelling (agent "core"): current state

Updated 11-27 13:06 UTC. Mac: usable only through its ONE 4-job nice-19 slot (DECISIONS a16b39297, NOSHAPE_RB_HOST=mac waits for it), never overloaded; default builds on rb-hosts (core* -> vibex7). Agent ids: only /root/lead-scratch/roster.md, read before every message (never copy ids here). History (dated notes up to 11-15 14:48): docs/handoff/archive/core.md. Standing rulings:
docs/DECISIONS.md (read first; it wins over anything here). Land NOSHAPE_LAND_QUEUE_HOST=vibe2 tools/tiers/land.sh
(vibe2 is the user's own server: only ~ngyekta and /opt/occt). Never kill a process tree from a computed pid.
## NOW (lead 11-27 10:5x, list approved; one land each with a corpus A/B, fix from the top)
- Suite 1126-2043 (190bd1b8; report vibex8 ~/tmp/qa/report.json, snapshot ~/tmp/qa/models-1126-2043-190bd1b8/<doc>/;
  copy core-scratch/suite/). blend parked: blend_direct.cpp is core's for now (note added to blend.md, acbf4dcd5).
  1 QUEUED 76bdd135: evAxis of a cylinder face follows the face's sense (eval.rs faces_axis; live cpax-1121); corpus
    ALL-SAME, model 664/0. 2 QUEUED fefa2000: harness (LIVE_SCATTER + LIVE_LINES_AS_OTHER; live's asym fillet is a
    B-spline fit). 3b QUEUED 49995530: parity translator reads variableStudios (3 passes; vars retried). Lead told.
  3 QUEUED 65e8a5e8 13:0x (commit 30439ec5e): G2 fill = fair_g1_fill(..., curvature) (after the G1 pass, 3 passes of
    N.S_dd = k_support |S_d|^2, wg2 1e3; op_fill sends orders 0..2) + rebuilt edges matched by curve (edges whose ends
    aren't shared: ConnectEdgesToWires copies them). body-draft Fill 1 120.35 -> 121.21 (live 121.95), vol 38233.07 ->
    38236.65 (live 38239.53): half the gap, rest PARKED (Parasolid's fill-hole surface; spans sweeps = tuning, rejected).
    Fillet 1 row LIVE_LINES_AS_OTHER. bridging Boundary surface 1 Warning -> Ok, 20866 -> 17289 mm2 (live 17559, row
    open). Corpus A/B: only bridging; kernel 621 ok, model 664 ok, g++-13 ok. T1 surfacing::a_g2_fill_of_a_hole_in_a_
    sphere_follows_the_sphere (G2 0.3 um off at r 10 mm, G1 3x). LEAD NOT YET TOLD (lead at 215k at 13:04): report.
  4 IN FLIGHT c2-curve-pattern-composite Fillet 2 (R10; 28/58/32 vs live 26/52/28, vol +10.3 mm3; live JJi OTHER
    326.6444 + JJ6 cyl 1220.5154). UNCOMMITTED blend_direct.cpp (patch also /root/core-scratch/cpc-sliver.patch): a piece
    with no inside point narrower than sew_tol (2 area < sew_tol x boundary) is dropped (were two 0.87 um cuts along one
    curve). The direct build then fails "an invalid solid": the R20 cylinder piece's wire self-intersects (uv u runs to
    1.586 > pi/2: the R10 ball's contact crosses the cylinder/plane tangent line at y=20 onto the bottom/top plane,
    2.66e-3 mm) + BadOrientationOfSubshape. Next: make the contact curve stop at / switch at the tangent line (cut on the
    cylinder must end at u = pi/2, the rest on the plane). Debug: vibex7 ~/tmp/core/bdn/cpc (.cpc.sh: parity + q.err,
    NOSHAPE_BLEND_DIRECT_DEBUG=1).
  5 c2-impeller-loft Fillet 3: edges 93 vs 95; "Adjoining edge not filleted." on Fillet Stevenrohr zum Halter.
  6 psui SPLIT-TRIMMED-PARTIAL-ERRORS (live "Tool entity cannot split the selected part/face." with Trim to face
    boundaries, tool partly across); keep ours if valid + what was asked ("ours does the work"), live's error only for
    non-manifold or a slit. Lead FYI: check qa's ~13:xx verdict clears impeller "Select a sketch plane." (69eae144).
  Not ours: c2-surf-intro-* Tail fin (skasm). QUEUED 45a536cf 10:5x (chamfer run-on + Fillet 2 + full round).
## P0s (lead 11-26; block v0.4.72)
- LEAD 22:1x ORDER: (1) Move face 1 repeated booleans try skipped; (2) sliver rule; land; (3) screws Move face 3.
- **SCREWS MOVE FACE 3: QUEUED 7f88950e (11-27 01:1x)** (commit 7211adcc2; T1 features_a::move_face_translate_slides_
  split_pieces_along_a_bore; corpus A/B .ab8.sh ALL-SAME; model tests 655 ok, clippy clean). Note for bool: MF3 406 ms =
  one boolean on the threaded body.
- **IMPELLER CHAMFER -> FILLET 3: LANDING 11-27 05:3x** (lead 05:3x: zum Halter is already Error in 0.4.72 / main
  1b395a8e per qa, so land). rim_chamfer cubic_through on the edge's parameter; boolean.cpp op_join_edges_at keeps the
  line when its far end lies on it; build.cpp extend_straight for ruled B-splines + own_part Segment 5% past each end
  along a ruled face's sections (live's JeG = face range +-5%); T1 kernel features::a_tube_ends_offset_chamfer_runs_on..
  (+ round tip check). Gates 05:3x: kernel 617 ok, g++-13 ok; fresh import A 39 / B 40 Ok of 48 (Move face 1 and
  Fillet Flansch zum Stroemungskanal Error->Ok, zum Halter Ok->Error in this import), regen 55.0 -> 43.0 s; Fillet 3 =
  live face for face at Fillet Stevenrohr einnschub. Corpus (OLD 11-18 import): Sketch Bohrung Flansch "Select a sketch
  plane." + 3 downstream: its plane is Extrude 6's CapEnd, whose sides Move face 1 offsets; Move face 1 was Error in
  the import's build, now Ok, so the old name is gone (fresh import resolves it). knifehandle -0.02 mm3 (check vs live).
- **P0 FIRST (lead 11-27 08:1x; qa compat on green 6c2c1fe9 vs 0.4.72): OLD-IMPORT REFERENCES IN c2-impeller-loft.**
  Sketch Bohrung Flansch Ok -> Error "Select a sketch plane." (+ Remove/Extrude Bohrungssitz, Fillet Bohrungen Flansch,
  Fillet Flansch zum Stroemungskanal). CAUSE (08:2x): 664abc017 (rim chamfer) made the old import's Move face 1 Ok
  (compat 5e42155d clean, 6c2c1fe9 first bad). Move face 1 offsets Extrude 6's side + Extrude 4's CapStart by OCCT's
  offset (direct.rs name_result with offset_role): every face it touches named Offset(the one it was), so Extrude 6's
  CapEnd (only re-trimmed) -> Face{MF1, Offset(CapEnd)}, then Split by MF2; the saved sketch names the bare CapEnd.
  FIX 65cc51362 (LANDING with f4b54ff9c): direct.rs alias_offset_faces at move_face's end: each Offset(k) this Move face made
  whose k no longer resolves gets an alias k -> Offset(k) (Bodies aliases; edges by those faces follow too). T1
  features_a::a_sketch_on_a_face_a_move_face_before_it_offsets_follows_the_face (.mf1.sh). Corpus A/B .ab12.sh on
  vibex8: ONLY impeller differs; its 5 features back to 0.4.72's exact states (sketch Ok, Remove/Bohrungssitz Info,
  Fillet Bohrungen 'Unfilleted edge overlapped', zum Stroemungskanal Ok); 8 parts (7 seat solids 255 mm3, as 0.4.72's 10). Probes: core-scratch/p0imp/{names,fg,dec}.py
  (vibex7 ~/tmp/core: release vs green face names, decoded ids).
- **RIB-FLIPPED-MESSAGE: COMMITTED f4b54ff9c (core-uip3, lands with the P0 fix)**: solid_a.rs rib one-piece branch: RIB_MISS unless
  the user's "Extend profiles to part" extension misses the part (L22 -> RIB_FAILED); tests features_a::rib_in_l_bracket
  (flipped -> miss), kernel_robustness hole_and_rib R3 no longer OPEN. Rib tests 14/14 ok (.rb1.sh,
  08:1x). FACE-PATTERN-BOSS-LIVE-FAILS (lead 08:4x: match live's error):
  pattern.rs copies_touch after a face pattern (copies vs the part's faces: no common vertex closer than 1e-7, or two
  common vertices, no edge, their midpoint on both); T1 features_a::a_face_pattern_whose_copy_touches_the_part_along_
  a_line_fails (8 along Error, 10 along Ok); model 662 ok; corpus A/B .ab13.sh ALL-SAME. QUEUED 0d26da5f 09:0x (with
  the P0 fix 931a66b5's commits).
- **KNIFEHANDLE SPLIT 1 -> CHAMFER 1: DONE, QUEUED 45a536cf (21f359d02, lead told 10:5x).** Split 1 exact (live's
  5157 = its own area figure). Chamfer 1 curved run-on + 48 sections; Fillet 2 over the tip: build.cpp reface_on keeps
  pcurves (holds()), blend_direct lies_on normals 0.05 rad. A/B .ab15: only knifehandle/hunting-knife; model 663, kernel 620.
- **UIPDEP3 RERUN 09:4x (vibex8 ~/tmp/core/uip)**: mistake-rib-flipped-misses + mistake-linear-pattern-face-boss MATCH.
  fillet-full-round-ok: live marks the first EMPTY selection field error (side1Face, side2Face, centerFaces in turn);
  FIXED uncommitted detail.rs full_round err_at(first empty) + T1 features_c::a_full_round_names_its_first_empty_selection
  (passes; 4 full-round tests ok). sweep-thin-edge-no-trim-ok: final state matches; live's Sweep 1 is "error" with Trim
  ends ON (ours Ok + warning): qa 10:1x VERIFIED both fixes, Trim-ends-on kept under the parity rule (nothing to do). Live also
  focuses "Faces and sketch regions to sweep" after the Thin tab click (ours none): psui's dialog focus, tell the lead.
- **ZUM HALTER CORNER: PARKED (lead 11-27 07:4x; Error as in 0.4.72 and main): native-blend-faces candidate.** WIP
  core-scratch/imp/crease-turn-wip3.patch (notes core-scratch/imp/corner-notes.md; the full STATE text in the archive):
  crease junctions, both crease edges untrimmed, >= 32 stations at creases (0.06 um on the bevel), fuzzy Fuse retry
  (unsafe alone: fuzzy fuses returned valid shapes that lost the body). The crease TURNS break every Boolean.
- **QA 11-27 uipdep3 rows: FULL-ROUND-SPLITS-PART 310cb8cb7 + SWEEP-THIN-EDGE-NO-TRIM f44aeb23f VERIFIED by qa on green
  8cec2b630 (TICKETS d9d4673a0).** Left: nothing (RIB-FLIPPED-MESSAGE and FACE-PATTERN-BOSS queued above).
- **SPLIT-NUMBERING-DUCT-MIRROR: QUEUED** 7774546b0 (core-p0; ids keep v0.4.71's order, names live's; test
  features_a::a_split_part_two_extrudes_made_keeps_its_pieces_ids; qa's userdocs.py duct = 0.4.71). Duct Split 1
  part NAMES still differ from live (later).
- **IMPELLER-LOFT-REGEN-TIME: QUEUED 60689091** (T2 ops parity_impeller_loft_regenerates_in_time < 40 s).
- **LEAD LIST 11-26 06:4x**: (1) bracket03 Chamfer 1: QUEUED 43ad3cc4 (core-spd; Chamfer 1 ~40 ms = 0.4.68; was 87-170;
  7 op_chamfers = 1 + 6 trials; c927 landed 15cff2b3f): chains_overlap takes `together`'s OpResult and
  skips pairs whose chamfer faces (history generated from the chains' edges) share no edge nor end within 8 d on a face along neither chain
  (cli-hist). (2) knifehandle settle fix + T1 settle test: core-rf tip commit (land.sh after 66dba573); slow cuts
  handed to bool 06:5x: vibex7 ~/agents/core-keep/kh-bool (Fillet 3 cut 244416f8 667 ms, Fillet 1 cut 856a411e 3.2 s).
  (3) Scissor Delete face 1 = live with 66dba573's merge; recheck 97ad5f Linear pattern 1 after it lands.
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
  Shell 1 is dx's; Extrude 55 is bool's.
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
00. (KNIFEHANDLE-LOFT1: skasm took it, lead 11-27.) KNIFEHANDLE-LOFT1 station rule when loft-stations3 (J, K: profile off its guides) is in; then Face blend 1 (on top); then projected-curve-enclose Mutual trim 1 (ours
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
- Traces: NOSHAPE_FS_TRACE, _PATTERN_TRACE, _WORK_TRACE, _SHELL_DEBUG, _DEFEATURE_DEBUG, _GUARD_TRACE, _BLEND_DEBUG.
- Private user documents (hats, dock, qa's tickets): only mode-700 copies in ~/tmp/core, deleted after;
  /root/qa-scratch/bin/userdocs.py compares two CLIs on them.
- C++: g++-13 build of noshape-kernel on vibe5 (down 11-17) before landing (`CC='ccache gcc-13' CXX='ccache g++-13' cargo build
  -p noshape-kernel`); direct-initialise OCCT handles; never name a variable `near` / `far` (empty macros on some
  platforms); never build a gp_Dir from a vector that can be zero (OCCT 8: std::terminate); signal-handler state needs
  `std::atomic_signal_fence` after it is set.
- OCCT patches (patches/occt, newest 0030): rebuilding a library on a vibe host needs `-march=x86-64` (check objdump
  for ymm / vfmadd); installing it on hosts is the lead's (x86) / mine on the Mac (~/noshape-host OCCT, backups beside).
  OCCT loops must poll `noshape_should_break()` with a deterministic bound, never a clock.
