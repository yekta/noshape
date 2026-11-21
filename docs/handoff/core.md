# Core modelling (agent "core"): current state

Updated 11-21 12:42 UTC (lead order 11-21: P0 trunk-hanger, then knife loft + infento chamfer sense, then THICKEN-DIRECT). Mac: builds + crate tests via NOSHAPE_RB_HOST=mac tools/tiers/rb.sh, single-model repros in ~/noshape-host/core-keep/models. Agent ids: only /root/lead-scratch/roster.md, read before every message (never copy ids here). History (dated notes up to 11-15 14:48): docs/handoff/archive/core.md. Standing rulings:
docs/DECISIONS.md (read first; it wins over anything here). Hosts: the least-loaded of vibex7/vibex8/vibe2 (DECISIONS a67911bef); land
NOSHAPE_LAND_QUEUE_HOST=vibe2 tools/tiers/land.sh (since 11-17 21:23; vibe2 is the user's own server: only ~ngyekta and
/opt/occt, no heavy runs). Never kill a process tree from a computed pid (11-17 21:00 incident).

## Now (11-21)
- **Move boundary of a curved surface (MANUALBLEND-BOUNDARY1 part, queued 11-21 06:5x as 175c325c):** k::extend_face_at
  (GeomLib::ExtendSurfByLength past the picked edges' uv sides; refuses faces trimmed inside their surface).
  manualblend 11 -> 12/19, Move boundary 1/2 +879.46 mm2 (live +879.47). Boundary surface 1 still ERROR: its U arcs miss
  our V edges by ~0.06 mm. Suspect: Intersection curve 1's approximated end tangent 0.04 deg off n1 x n2 -> Plane 3 tilts
  (live normal -0.304035,0.884882,0.352912) -> Sketch 6/7 shift. Probe: /root/core-scratch/icdbg.py (NOSHAPE_ICDBG, temporary).
  Coons fallback for the fill didn't help (parked mb-coons2.diff). Then Split 1's extra edge (1/5/5 vs 1/4/4).
- **SUITE-IMPELLER-FILLET3 -> blend (agreed 11-21):** 9c9f29267's merge is right (the chamfer's x=120 stop cylinder one
  face, kernel test a_tube_ends_offset_chamfer_..._as_live = live's 33). No-copy only "matched" because a rejected merge
  rewrote the shared surface's curves (chamfer 34, Fillet 3 volume off, next fillet ERROR). Fillet 3 on the one-face
  cylinder is blend's; they'll say if our seam (lowest vertex pair) must move. Experiments: core-scratch/cylmerge-modes.cpp.
- **SUSPENSION-CIRCPAT1 -> blend (lead 11-21):** root is 805e29's Fillet 3 (OCCT refuses R2.5 on 7 of 17 edges);
  dump vibex7:~/agents/core-keep/susp/fillet-19.
- **VALID-SURFLOFTS-LOFT1:** first difference is Face blend 1 ERROR ("Cannot attach blended face"); Loft 1's invalid
  sheet is downstream (Bridging curves on wrong geometry). Goes with the Face blend 1 item.
- **SPEED-VALVE-CONFIG LANDED c17f6eaab.** Impeller Delete face 2 / Extrude 4 speed PARKED (lead 11-21): from feature 29
  our impeller is a wrong model (Halter, Extrude impellerraum 38 vs 59 faces); wait for blend's cascade fix.
- **P0 TRUNK-HANGER (lead 11-21, skasm bisect): 18354ec79's region dedupe broke c2-genesis-trunk-hanger's Sweep 1**
  (profiles A, B, A; live sweeps A twice; 20 -> 12/73). First cause fixed: LANDED c30ff0e4 (picked_regions as before,
  EvalCtx::distinct_regions only for Extrude). A SECOND cause landed after 9612b4685 (9612b4685 + fix = 20/73 on vibex7;
  main 12/73, Sweep 1 100/200/100 vs live 73/152/80). Left: f3f4d2334, a9c26c41c, c62392284, c17f6eaab, 43dc96575,
  89b884427, 7d1b596f9. Bisect in core-th on vibex7 (/root/core-scratch/bis-run.sh <commits> -> bis.out).
- **INFENTO-CHAMFER-SENSE: queued alone 11-21 (blend_edge_senses + kernel_robustness test).** asm-infento-subs on the Mac
  with it: 157/167, no a3f7a1 feature differences (Extrude 3 / Chamfer 3 OK).
- **KNIFEHANDLE-LOFT1 (core-bis2, branch core-thd, not queued):** k::loft_guided_multi (loft_guided.cpp): Handle 1/2/3 as
  live, area -0.42 %; grid 12 per guide x 24 rows (Mutual trim 338 -> 180 ms). Loft must not land before Mutual trim /
  Boolean 1 / Fillet 1 are right (now they build and fail: doc 0.7 -> 3.8 s on the Mac). Live normals
  (live/core/q-knifehandle-normals-1121): Mutual trim keeps qSplitBy's BACK side on both bodies; live's Handle faces
  OUTWARD (+z top, +y side; its profiles' loops run either way), Surface 2 = profile tangent x path (-x). Ours: Handle
  faced inward. In core-bis2 (uncommitted): the loft face turned outward from the ring centre; mutual_trim keeps the
  groups behind the other set (behind_of: sample points whose foot is inside a tool face, nearest the cut), larger-side
  fallback. Checking on the 4 mutual-trim docs (Mac .kmt3.sh; workshop-mutualtrim must stay 8/8).
- **THICKEN-DIRECT next:** core-bis2's .thv.sh (vibex7) dumps every thicken input of the 20 thicken docs
  (NOSHAPE_DUMP_ONLY=thicken) into ~/agents/core-keep/thdump/<doc>; then the direct builder (offset faces trimmed at
  concave sharp edges, ruled walls, sewn; OCCT fallback; same faces/edges/volume as OCCT on every dump).
- Note: loft_guided.cpp was once corrupted by an empty-pattern replace (140 MB); recovered by de-interleaving.

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
0000. (lead 11-21, P1, after SUSPENSION-CIRCPAT1) STD-3132: bundled std 3112 -> 3132 (live/std-3132 +
   live/std-3132-vs-3112.diff, 14 files / 181 lines: SM pattern keeps collapsed walls V3128, hole error preview on a start
   plane, 7/8"-14 UNF tap drill 0.8125 in, thin extrude's new builtin @setThinWallOneSided: no-op unless annotations need
   it). Gate: model suite none worse + skasm's 25 SM docs; tell skasm when landed.
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
