# Core modelling (agent "core"): current state

Updated 11-28 05:24 UTC. Mac: usable only through its ONE 4-job nice-19 slot (DECISIONS a16b39297, NOSHAPE_RB_HOST=mac waits for it), never overloaded; default builds on rb-hosts (core* -> vibex7). Agent ids: only /root/lead-scratch/roster.md, read before every message (never copy ids here). History (dated notes up to 11-15 14:48): docs/handoff/archive/core.md. Standing rulings:
docs/DECISIONS.md (read first; it wins over anything here). Land NOSHAPE_LAND_QUEUE_HOST=vibe2 tools/tiers/land.sh
(vibe2 is the user's own server: only ~ngyekta and /opt/occt). Never kill a process tree from a computed pid.
## IN FLIGHT 11-28 04:45
LANDED: 434c2a49 (as 1d0bf5a6: extrude-hole evaluator side, spline-fillet-asm exact volume, blend #4 box-corner balls +
reach + reprojection), macOS G2 sphere-fill T1 (aa19340a: cross-edge curvature 1.2 % vs band 5 %; lead picks it onto
the release).
QUEUED 57ac5e13 (worktree core-g2mac, from main; carries f27f2793): (1) e18766a8 impeller: an as-is OCCT round whose
new vertices are looser than its radius isn't kept (feature 43 Fillet Flansch zum Stroemungskanal, tol 1.14 mm ->
Extrude Bohrungssitz Ansaugflansch's Boolean no longer declines, 2.9 -> 0.25 s; bool's report). Corpus A/B 04:3x: only
c2-impeller-loft differs (Part 1 115536 -> 115591 mm3, live 119670: upstream #29), statuses unchanged. (2) slanted-end
round test asserts edges on faces (de975d18). (3) britatap Delete face 2 (58f84927): direct.rs adjacent_fillets takes
an edge only along a fillet's rails (least-curved way); T1 features_c::delete_face_keeps_a_pipe_bend_tangent_across_it.
Corpus A/B 04:4x: only britatap differs; parity 8/22 -> 12/22. Kernel 622/0, model 670/0, g++-13 ok.
IN FLIGHT 05:2x: BRITATAP Split 3 (#13: ours dropped the whole target, live keeps 1/7/7 5966.7 mm2). Cause: split.rs
sheet_point's far point of the big piece lies past the tool profile's end, nearest tool point on its boundary, so
side_of's sign was wrong (both pieces "behind", Keep front removed both). Fix (UNCOMMITTED split.rs): sheet_side():
if the far point's projection isn't square to the tool (|cos| < 0.9), cut_side(): the tool normal at a cut edge's
middle against the way into the piece. Release cli (vibex8 ~/tmp/core/bt/cli) gives Split 3 = live 1/7/7. T1 (UNCOMMITTED)
features_c::split_keeps_the_front_of_a_sheet_reaching_round_the_tool (U plane + half cylinder; one piece left).
RUNNING (task, local /root/core-scratch/sp-run.sh): T1 with fix, then SWAPS split.rs to HEAD's for the without-run,
restores from core-scratch/sp.F (ends RESTORED). If the run died: cp /root/core-scratch/sp.F to split.rs FIRST.
Then: model tests, clippy, corpus A/B (vibex7, A = HEAD split.rs), parity britatap, commit, land.
britatap rest: #12 Extrude 1 area 5309.019 vs 5309.235 (sketch with G2 + external refs solves 5 um apart; low);
#14 Side blend curve Error "Bridging curve start and end points must be distinct." (corpus cli; parity harness said
"Selected vertex does not belong to selected face."): next after Split 3. Then #15-#21 follow.
Probe setup vibex8 ~/tmp/core/{bt (britatap copy + live.jsonl), lib (sig/nscli/hkf/fdump)}.
PARKED tonight (all blocked upstream / native blend faces): (a) c2-hunting-knife #51 Fillet 8 (16/36/22 vs 15/37/25):
the dome ring is 2 edges (both seams end on it, archive 11-18), OCCT makes one blend face per spine edge -> 2 faces of
57.44 (live one 114.83); joining the edges first (probe vibex8 ~/tmp/core/hk/fp.cpp) fails (pcurves cross a seam).
Needs a two-blend-face merge (surface concatenation) or native blend faces. Live also splits the loft side in 6
(11-16 known class). (b) impeller #47 is downstream of #29 zum Halter (parked): parity diff list from #27 on.
(c) body-draft-naming #8 Fillet 2 = the pinch corner (2.9 mm3). lora-enclosure matches to the end (item f done).
If 434c2a49 fails: fix forward. qa 01:1x asked about 28329.612 in its UI takes: answered (live UI shows massproperties' nominal; compare UI takes within live's variance band). Not yet checked: a_round_over_a_slanted_end_across_a_turn fails without `reach`.
IMPELLER P2 (kernel profiles vibex7 ~/tmp/core/kn/i-new.err / i-old.err): Extrude Bohrungssitz Ansaugflansch 2.9 s
vs 0.5: noshape's Boolean declines in 12 ms (0.4.73: boolean.rust 284 ms), OCCT 2.8 s; with NOSHAPE_BOOL_FALLBACK=0
"a face's vertex off its plane" (boolean.rs ~5625; that run also fails Delete face 2, so maybe not the real reason):
told bool 00:5x (P2, for later; they'll say if an upstream feature of mine is sloppy). Delete face 2 4.1 vs 1.9 s:
OCCT op_defeature on the bigger (live-like) part, 579 vs 85 edge-face intersections: not a regression in code.
## REPORT PENDING (lead at 395k at 20:40: not messaged): pulleys fix QUEUED 96280303. Earlier (16:5x): e8d60855 re-landed docs-only as 09995eb9; 04b6d11a follows the split rule
(errors only when pieces < 2 AND an INTERNAL face/edge remains = a slit). Both said to lead 17:3x in thread.
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
  4 QUEUED 434c2a49 (11-28 01:0x, commit 5e870683) c2-curve-pattern-composite Fillet 2: parity 3/11 -> 9/11 (next diff
    Curve pattern 1 Part 8/9, pre-existing). Details in IN FLIGHT above.
  5 PARKED 13:4x c2-impeller-loft (parity 13:2x on 30439ec5e: 27/48; Fillet 3 34/93/60 vs live 34/95/62, faces and volume
    as live; then zum Halter ERROR "Failed to fillet selections." and the cascade to 47). Fillet 3 edges by type+length
    (.imp3.sh, vibex7 ~/tmp/core/imp3): (a) vertex splits along the y=+-27 run-ons and label kinds (live LINE/SPLINE vs
    our OTHER): 11-16 known-difference class; (b) the conic end at x=5: live 0.19+0.56 each side + a 0.36 edge across
    (5,-0.144,-44.007), ours 2 x 0.75 meeting at y 0, plus live's 0.05 edge at (0.023,4.132,-63.209). (b) is the layout
    zum Halter's 4th reference needs (archive 11-25 note; crease-turn WIP core-scratch/imp/crease-turn-wip3.patch, parked).
  6 QUEUED bf790398 13:5x psui SPLIT-TRIMMED-PARTIAL-ERRORS: split.rs pieces < 2 with an INTERNAL face/edge (a slit)
    -> err "Tool entity cannot split the selected part/face." (ours kept Part 1 whole with an Info = not doing the work).
    T1 features_b::a_trimmed_face_part_way_through_a_part_cannot_split_it. A/B .ab19: ALL-SAME, model 665 ok. Tell psui "fixed".
  7 DONE 14:3x c2-body-draft-naming after Fill 1 (.bd3.sh/.bd4.sh): Intersection curve 1 = Fill's Surface 1 x the part
    face made from it (one surface): live WARNING + no curve, ours their 3-edge boundary (ours does the work, kept).
    Draft 2 12/27/17 vs live 13/28/17: face for face as live except live's 0.1 mm sliver face at the Fill corner (its
    11.778 edge split 11.678 + 0.1, two 0.1 OTHER edges at (50.634,14.575,63.93)): Parasolid artefact, not matched. The
    doc's only real difference left: Fill 1's 2.88 mm3 (#3, parked).
  LEAD NOT YET TOLD (lead 245k at 14:2x): #3 queued 65e8a5e8 (half the gap + bridging Warning->Ok), #4 + #5 parked with the
  findings above, #6 status. Lead FYI still open: qa's ~13:xx verdict on impeller "Select a sketch plane." (69eae144).
  Not ours: c2-surf-intro-* Tail fin (skasm). QUEUED 45a536cf 10:5x (chamfer run-on + Fillet 2 + full round).
## P0s (lead 11-26; block v0.4.72)
- LEAD 22:1x ORDER: (1) Move face 1 repeated booleans try skipped; (2) sliver rule; land; (3) screws Move face 3.
- **ZUM HALTER CORNER: PARKED (lead 11-27 07:4x; Error as in 0.4.72 and main): native-blend-faces candidate.** WIP
  core-scratch/imp/crease-turn-wip3.patch (notes core-scratch/imp/corner-notes.md; the full STATE text in the archive):
  crease junctions, both crease edges untrimmed, >= 32 stations at creases (0.06 um on the bevel), fuzzy Fuse retry
  (unsafe alone: fuzzy fuses returned valid shapes that lost the body). The crease TURNS break every Boolean.
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
   e. c2-body-draft-naming #8 Fillet 2: builds (281ced578), 2.9 mm3 off = the pinch corner (native blend faces)
   After an upstream difference (fix upstream first; re-check after):
   f. DONE: c2-lora-enclosure-ribs matches to the end (gate cache 11-28)
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
0. ERRORS-1123 CLOSED 14:4x: every audit case now matches live or ours does the work (probes, not kept: steep draft
   60/80/89 deg both pulls all Ok and valid (live's crossing faces trimmed: 5 faces); sweep profile in the path's
   plane = live's "Could not create valid swept body, check input."; revolve axis-through, angle0, move-face collapse,
   loft guides, boolean targets/intersect, hole texts already in).
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
