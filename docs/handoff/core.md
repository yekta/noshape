# Core modelling (agent "core"): current state

Updated 11-23 07:11 UTC. All lands through 96180a1c are on main (1d414cce). Mac: usable only through its ONE 4-job nice-19 slot (DECISIONS a16b39297, NOSHAPE_RB_HOST=mac waits for it), never overloaded; default builds on rb-hosts (core* -> vibex7). Agent ids: only /root/lead-scratch/roster.md, read before every message (never copy ids here). History (dated notes up to 11-15 14:48): docs/handoff/archive/core.md. Standing rulings:
docs/DECISIONS.md (read first; it wins over anything here). Land NOSHAPE_LAND_QUEUE_HOST=vibe2 tools/tiers/land.sh
(vibe2 is the user's own server: only ~ngyekta and /opt/occt). Never kill a process tree from a computed pid.

## Now (11-24 13:50 UTC)
- **35dceca0 LANDED** (lead 11-24): errors-1123 blamed inputs, part colours (split/pattern/mirror/copy), Subtract cut
  faces, Split names+order, hose joints. TODO: tell qa (roster id, agent_status first) to rescore errors-1123 core rows
  (ERR-SEL-RED-ALL-INPUTS, ERR-HOLE body, APPEAR-*) and the 11-24 "fixed, to verify" rows; ask dx to rerun the hose x_t.
- **Lead 11-24: scissor slab Move-face work PARKED** (WIP core-rmx cb5c5ec61, branch core-slabs). Focus: open core
  P0/P1 TICKETS for everyday features, wrong geometry/errors before speed, small land batches.
- **Committed, NOT landed (core-rf):** 85f167b15 Loft: an error naming a list of selections (Profiles) adds no
  " A parameter value is not valid."; an empty guide entry reads "No selections" (feature_dialog entry_label, ui_state);
  TICKETS marks rows fixed-to-verify (Move face/Thicken/failed bodies landed 11-23 05:04 = 435354603, c276c745f,
  26657ac62; union 1f317d4df; thread on revolved cylinder and surface loft guide OK on green CLI). psui full suite 532
  passed + the new test, clippy clean.
- **In worktree, uncommitted (core-rf): DRIVEN-PATTERN-SWEEP fix.** features.cpp sweep: MakePipeShell retried at
  1e-7/1e-6/1e-5 m where the path-sized tolerance fails (c2-loft-guide-curves-driven-pattern Sweep 1: 0.001 in wire
  coiled on the routing curve); solid_a.rs may_self_intersect: profile gap = 2x sagitta (was chord), path steps <= size/4
  (Sweep 1 1316 -> 69 ms). T1 kernel core::a_thin_wire_coiled_on_a_routing_curve_sweeps + crates/kernel/tests/data/sweep/coiled-wire.brep (untracked: git add it).
  Verified: kernel 592 + model 628 + sweep tests pass (before the pre-check change), 25 sweep corpus docs none worse than
  nightly pm-1122. Final run .core-check11.sh (clippy kernel/model-core/psui, model-core, model, kernel) -> log
  /root/core-scratch/run/check11.log: if green, commit ("core: a thin wire coiled on a routing curve sweeps ...") with
  TICKETS + handoff, then land.sh (stack 85f167b15 + this). Left gap: volume 0.0754 vs live 0.0799 (our routing curve's
  start direction ~12 deg off live's) -> next item.
- Remaining open core P0/P1 everyday rows after these: ERR-LOFT-GUIDES-EMPTY tip order (one capture, no rule), then
  speed rows (SPEED-NUTS Sweep 1 556 ms, SPEED-VALVE-CONFIG / SPROCKETS Circular pattern ~200 ms).
- Probe kit: /root/core-scratch/bin/mcp.py (MCP client), mf/th.py, mf/sl.py, mf/st.py, dp/fl.py (green CLI checks).
- **PARKED projected-curve-enclose Fill 1 (branch core-fill1-wip f4f3d55d, on core-rf):** live one cylinder face
  5072.2167 mm2, ours a plate 5017.6. k::edges_on_faces (boolean.cpp: all edges copied once, each face's surface made
  analytic by GeomConvert_SurfToAnaSurf, p-curve where within max(tol, edge tol)); curves.rs puts the two-sketch
  projection's joined edges on its prisms' faces. Still fails: BRepBuilderAPI_MakeFace(wire) finds no surface (T1
  kernel core::a_curve_on_a_cylinder_fills_with_that_cylinder). Next: check each edge got its p-curve (FindSurface
  wants the SAME handle and location on every edge, then Is2DClosed); else build the face on the cylinder directly
  (fill: all edges on one analytic face -> BRepBuilderAPI_MakeFace(surface, wire)).
- **KNIFEHANDLE LOFT 1 (open at +0.13 %, lead 11-22: parity is the target):** 472eddd5: each loop runs by its own
  parameter from guide 0, each guide by its own parameter (Pchip through (v_i, t_i)): 21504.09 mm2 vs live 21477.23
  (was 21517.64), P1-P4 width within 0.02 mm, P5-tip +0.38 mm. T1 kernel the_knifehandle_loft_through_six_profiles_has_lives_area.
  Remaining gap = station v rule. Stations now = chord of the profiles' centres (replica +20.0 mm2; live v -5.7).
  live loft-stations (live/core/loft-stations, README): B/C/D = cumulative chord of the guide points, E (ellipse) =
  0.4000 not 0.4007; all mirror guides. On knifehandle (two different guides) every guide-chord variant is worse
  (replica: guide0 -79.9, guide1 +66.0, seg mean -60.9, frac mean -57.6, side u.25 -49.4); only a surface u .3275 /
  .6725 chord fits (4e-4), unexplained. Scripts /root/core-scratch/kh: rep3.py (area per rule), st2-st7.py.
  live loft-stations2 (F..I): all 9 captures = chord fractions of the profile-point row with the shortest total chord;
  knifehandle's shortest row is u .5 (err .018, replica +66): no fit (st8-st10.py). Its P4 misses both guides, so lead
  (b): keep centre chord. loft-stations3 J/K (profile 3 mm off its guides): live REFUSES (LOFT_FAILED), so P4 "off its
  guides" was a sketch misread; the shortest-row test already used live's own surface loops (err .018 stands). PARKED at
  +0.13 % (lead 11-22). J/K: ours refused them with "All guides should intersect..."; now a guide beside a MIDDLE profile is
  LOFT_FAILED as live (an end profile missed keeps "All guides": psb 10-24 g). Asked live2 (REQUESTS 11-22 23:08) for a guide
  running past an end profile to tell end-vs-middle from short-vs-beside.
- **THICKEN-DIRECT + SPEED-SPLIT-THICKEN closed (11-22 14:10):** gate on 20 thicken docs, direct vs OCCT (temporary
  NOSHAPE_THICKEN_OCCT in core-rf branch core-thg, reverted): same matches everywhere (253/368); workshop-split 140-143 ms
  vs live 112 (A/B 10 runs; first gate's 239-253 was host load). Analytic offsets (Geom_OffsetSurface::Surface()) tried:
  no change there (no cylinders), dropped. Logs vibex7 ~/agents/core-keep/thg*/.
- **Persist:** a result the regeneration cache can't encode is now logged (persist.rs warn, in 472eddd5).
- Probe kit for saved docs (vibex7 ~/tmp/core/l5): ts.py <cli> <doc> (feature statuses + Sketch 3), tn.py <cli> <doc>
  <copy> (faces/vertices with decoded names after Tab 1); face names are urlsafe-b64 raw-deflate JSON.
- **REGEN-SPEED isaac:** isaac-ball open is level with live (TICKETS SPEED-ASM-OPEN: 13.9 s vs 8.3-13.1). Extrude 27's
  unify.reordered is 1.1 s of 76 s serial kernel time: not worth a change. Kernel totals over the open: op_boolean
  12.9 s (bool), op_fillet 11.9 s (blend), op_unify 7.0, volume_change 3.8, min_distance 2.4.
  its profiles BY PARAMETER (as ours), ruled 3x1, 120x2 CPs, knots doubled at 1/64. So our 1 mm2 gap is the profiles'
  own parametrisation (live's projected curves: 64 spans, doubled knots); compare ours with the dump next.
  Ours: Loft 1 12748.85 vs live 12749.86 mm2 (car body, derived b7635b82); scripts vibex7 ~/agents/core-keep/sib (ar.py
  areas per feature, dv.py, ed2.py).
- **Surf-intro Mutual trim 1 / Ruled surface 3 -> skasm (sent 11-21):** translator gives the sketch entity for Ruled
  surface 1's SWEPT_EDGE/FACE/BODY of edge Ao0Xs1GgygP800; repro vibex7 ~/agents/core-keep/mt.
- Surf-intro builder side still mine: Boundary surface 1 WARNING (live OK), Fill 1 continuity.
- **Queue after:** Face blend 1 (+VALID-SURFLOFTS-LOFT1), MANUALBLEND-BOUNDARY1, dx's
  EDGE-CENTRE-SEAM (P2: props_linear centre of a seam-crossing ellipse rim; probe vibex6 ~/agents/dx-keep/sr/rlmid.py),
  live's 6 cov2 models (c2-cov2-*; error rules in behaviours "lead (11-21 13:59): coverage batch 2").
- **MANUALBLEND-BOUNDARY1: Move boundary of a curved surface (MANUALBLEND-BOUNDARY1 part, queued 11-21 06:5x as 175c325c):** k::extend_face_at
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
- **VALID-SURFLOFTS-LOFT1 (in progress, core-rf branch core-sfl):** Face blend 1 is fixed (ca9c74200); first difference
  now Loft 1 (feature 9, between Bridging curves 1/2 at z +-12.5): ours 4 spline edges, live 2 LINE (25 mm) + 2 SPLINE.
  Change: features.cpp straight_edges_as_lines after op_loft (the loft's own straight B-spline edges -> Geom_Line in place,
  SameParameter; sections' edges untouched; NOSHAPE_LOFT_SPLINE_EDGES=1 = old). T1 kernel core::
  a_loft_between_two_open_curves_has_straight_lines_between_their_ends. Next: kernel loft tests (.sl.sh), then a model
  suite pair (lofts in many docs change edge kinds) + NOSHAPE_PARITY_VALIDITY=1 on surfaces-lofts-projected.
- **SPEED-VALVE-CONFIG LANDED c17f6eaab.** Impeller Delete face 2 / Extrude 4 speed PARKED (lead 11-21): from feature 29
  our impeller is a wrong model (Halter, Extrude impellerraum 38 vs 59 faces); wait for blend's cascade fix.

## In flight (11-20)
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
0. QUEUED 318c67d5 (03:23; carries 0c582c74 + ba3d71cf): Thicken centre-of-curvature refusal + guided-loft LOFT_FAILED,
   Move face (part offset to nothing fails; boss offset to its axis removed), and ERR-FACE-PATTERN-FAILED-PREVIEW +
   ERR-SWEEP-FAIL-BODY: failed face patterns/mirrors (off copies), holes off their part (tool at its Depth from the
   point) and too-tight sweeps fill error_shapes, drawn dark red. T1s features_c boss_block()/face_nearest() helpers,
   kernel_robustness a_sweep_round_too_tight_a_bend_draws_its_body_red; hole/pattern/sweep tests 90/90 on vibex8.
   Also queued: ERR-BUBBLE-ON-ERROR (behaviors.rs tool_missed: the tool preview runs as New, so the studio display's
   Error counts too; UI test a_remove_missing_with_no_merge_scope_shows_only_its_error) and ERR-ADD-PATTERN-PREVIEW-TINT
   (part pattern Add with NO_GEOMETRY_CHANGE -> error_shapes = the instances; regen tessellates them on Info; mod.rs
   draws an Info feature's error_mesh while open; T1 an_add_pattern_changing_nothing_draws_its_instances_red).
   Probes out of git: ~/core-keep/probes2.patch.
0. QUEUED 35dceca0 (11-24, 19 commits = every core commit since 11-20): error_refs (only blamed inputs red; an error
   without explicit refs reddens its err_at params' inputs, none when it has an error_mesh: psui's flange/pattern UI
   tests), APPEAR-SPLIT/PATTERN-COPY part colours, SUBTRACT-CUT-FACES (ScopeRule 20, legacy_cut_face_names), Split
   names + order (extrude_face_order), hose joints (ring_sweep). The 11-23 failed queues' base carried unlanded
   psui/studios commits: rebased with --onto (only core's). Green on vibex7: model 628, model-core, model-data, scene,
   custom, psui 526, ops e2e. After it lands: tell qa to rescore errors-1123 core rows + APPEAR-SPLIT/PATTERN; ask dx
   to rerun the hose x_t.
0. P1 SPEED-ASM-OPEN-SCISSOR (WIP, core-rmx branch core-slabs, commits 7a9c1af8 + cb5c5ec6; diff also
   /root/core-scratch/slabs-wip2.diff is older). offset_by_slabs now unifies a COPY (the null OCCT fallback was unify
   editing the input's shared edges), history = compose(inputs, h, copy+unify) with moved faces/edges/vertices ->
   their slab copies' images (ReplaceModified) and solid -> solid. T1 kernel core::a_face_moved_between_square_
   neighbours_is_the_part_and_its_slab passes. Temporary NOSHAPE_NO_SLABS switch in op_offset_faces (drop before
   landing) for A/B. Scissor open (vibex7, load 13): 71 s slabs vs 75 s OCCT. BLOCKER: slab mode breaks downstream
   refs: Studio 665cf3 features 0-7 same status, Extrude 1 (feature 8) "Select a face to terminate" (OCCT Ok); also
   9cbb0e/56d3a6/fed22d Mirror/Fillet "Select ..." cascades; assembly 4 instances fail to resolve. Next: find which
   offset differs before 665cf3's Extrude 1 (its Derived 1 source studio's Move faces?), compare decoded face names
   (tn.py probe kit) slab vs OCCT. Tools on vibex7 ~/tmp/core/sc (pruned after 12 h): fd.py (per-studio feature/part
   diff of both modes, raw fd-slabs/fd-occt.jsonl), b.jsonl, od.out; scripts .sc*.sh/.ab.sh/.fd.sh in core-rmx.
   Unify can't merge a fuse slab's walls with rounded-corner cylinders / hole walls (unowned faces, slab path refuses,
   OCCT fallback runs, gives a Shell there as main): not axis direction, not tolerance, DirectModification breaks the
   solid; maybe u-range offset by the period. Then Delete face 2 (op_defeature 1.15 s).
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
