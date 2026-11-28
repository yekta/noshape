# Core modelling (agent "core"): current state

Updated 11-28 07:32 UTC. Mac: usable only through its ONE 4-job nice-19 slot (DECISIONS a16b39297, NOSHAPE_RB_HOST=mac waits for it), never overloaded; default builds on rb-hosts (core* -> vibex7). Agent ids: only /root/lead-scratch/roster.md, read before every message (never copy ids here). History (dated notes up to 11-15 14:48): docs/handoff/archive/core.md. Standing rulings:
docs/DECISIONS.md (read first; it wins over anything here). Land NOSHAPE_LAND_QUEUE_HOST=vibe2 tools/tiers/land.sh
(vibe2 is the user's own server: only ~ngyekta and /opt/occt). Never kill a process tree from a computed pid.
## IN FLIGHT 11-28 07:32
QUEUED (vibe2): 57ac5e13 (core-g2mac: impeller round tol, slanted-end test, britatap Delete face 2), 96b23e35 (Split 3 +
parity cPoint vertex), bc95a9cf (core-g2mac2: G2 sphere-fill T1 band 5 % -> 10 %: Mac arm64 5.8 %, Linux 1.2 %, G1 26 %),
3a62fc14 (core-uip3, 7 commits incl. the 96b23e35 stack: Boundary surface trim_profiles(): in a 2x2 network each profile
split where the other direction's profiles meet it inside, overhangs pruned; T1 features_b::boundary_surface_profiles_
are_trimmed_where_they_meet fails without it; corpus A/B ALL-SAME; model 672/0). No kernel change -> no Mac run.
NEW RULE (lead 11-28, DECISIONS): any crates/kernel or crates/brep change (or their tests) runs that crate's tests once on
the Mac (NOSHAPE_RB_HOST=mac, .mk.sh in core-uip3) before land.sh.
LEAD NOT TOLD (lead at 601k at 07:00): the lands above; Mac rule applied (bc95a9cf); #14 done; #15 trimmed (live's 5/20/16,
area -0.17 %).
britatap (parity 14/22, first diff #12 Extrude 1 PARKED: sketch minimal-move choice). #15 Bridge patch - Top builds;
remaining gap: V1 NORMAL_TO_PROFILE unheld (warning; live's bsurf likely Hermite Coons, bigger item).
WIP UNCOMMITTED core-uip3 (kernel change: Mac run .mk.sh needed): bshared.rs join_surface() after the sew runs
k::merge_coincident_revolutions (NEW boolean.cpp op_merge_coincident_revolutions: neighbouring torus / revolution faces
whose surface coincides (uv_onto: a rigid (u, v) map, shift or mirror, 9 points, onto the group's UNTRIMMED surface) put
on one surface, pcurves mapped, loops reversed where mirrored, whole-period shift to meet a placed neighbour, seams
across the period; unify of only those edges) then k::unify_same_domain (cylinders). britatap Fill - Top 9/17/8 = live
(was 13/31/18); head 11944.27 vs live 11934.85; Fill - Pipe end builds; Fill - Bottom "Select laminar edges" (stale
refs in the 07:02 save? re-save). T1 features_b::a_surface_add_merges_the_faces_of_one_revolved_surface (half ellipse
revolved +-90 deg, Add: 1 face, area 2 x half; fails without: 2 faces; a circle's arc doesn't: OCCT merges tori).
LANDING 08:12: "core: a surface Add merges faces of one revolved surface, torus or cylinder" (merge_coincident_revolutions,
called from join_surface; no general unify there: live keeps coplanar planes apart, c2-surf-intro-mutualtrim). Checks:
britatap Fill - Top 9 faces, kernel 624/0, model 673/0, g++-13 ok, corpus ALL-SAME, Mac kernel 617/1 (only the G2
sphere test, fixed by queued bc95a9cf). Log /root/core-scratch/land-mcr.log.
Probe: vibex7 ~/tmp/core/pbt/{cli, save/ (britatap re-saved 07:02 with the trim, 600)}, ~/tmp/core/lib (fq.py faces
after features with type/area, parts.py, bp.py, fg.py). Live data: vibex7 ~/parity/models/c2-surf-workshop-britatap.
Cleanup when done: vibex7 ~/tmp/core/pbt, vibex8 ~/tmp/core/bt, worktrees core-g2mac/core-g2mac2 after their lands.
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
