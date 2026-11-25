# Core modelling (agent "core"): current state

Updated 11-23 07:11 UTC. All lands through 96180a1c are on main (1d414cce). Mac: usable only through its ONE 4-job nice-19 slot (DECISIONS a16b39297, NOSHAPE_RB_HOST=mac waits for it), never overloaded; default builds on rb-hosts (core* -> vibex7). Agent ids: only /root/lead-scratch/roster.md, read before every message (never copy ids here). History (dated notes up to 11-15 14:48): docs/handoff/archive/core.md. Standing rulings:
docs/DECISIONS.md (read first; it wins over anything here). Land NOSHAPE_LAND_QUEUE_HOST=vibe2 tools/tiers/land.sh
(vibe2 is the user's own server: only ~ngyekta and /opt/occt). Never kill a process tree from a computed pid.

## Now (11-25)
- **P0 FLOW-HOLE-ON-FACE-SKETCH-NO-SCOPE: not a Hole bug.** qa's start "bracket-link" draws its R10 ends with ops
  `sketch_draw center_point_arc [[30,10],[30,0],[30,20]]`: scripted clicks went straight from start to end, a 180° turn
  the tool's sweep reads as clockwise, so both ends bulged in (region 885.8 mm² = 1200 − 100π, x ±30) and the point at
  (30, 10.2) sat in the bite: no part under it. Fix: ops sweeps the pointer through the CCW arc's middle before the end
  click (ops/sketch.rs). Tests: e2e a_hole_at_a_point_on_an_obround_links_top_face (area 1200+100π, Hole Ok, scope 1);
  GPUI a_hole_at_a_point_sketched_on_a_face_merges_into_its_part (the app's dialog fills Merge scope at the first pick).
- **Re-landed 09:0x: QUEUED 86e2d541** (fillet crash fix, docs, ops arc fix, STL fin repair, handoff). The 08:44
  failure (psui a_circles_rim_lights_and_selects_the_circle_not_its_centre_beyond_reach, a 1374 s loaded run) passes
  3/3 on the rebased branch: flaky under load, not ours.
- **STL vs live, c2-housing-draft** (ours .ex5.sh on vibex7; live /root/parity/exchange/c2-housing-draft/live-opt-*.stl;
  core-scratch/bin/stlz.py rings): ours 678 / 1348 / 5428 / Custom 5514 triangles vs live 924 / 1772 / 6644 / 6612,
  all watertight, ~140 ms each. Every circle has live's segment count exactly except two: R13 at z 1.5 and R12.5 at
  z 1.8 (the narrow drafted strip between them), where live has twice ours at every resolution (59/115/285 vs
  29/58/143), unevenly spaced. Our sag there 0.076 mm < 0.24. REQUESTS core 11-25 08:28 (10) asks live that face's
  surface / edge curve types; if SPLINE, mesh spline faces as Parasolid does. Impeller Medium: 0 defects after the
  fin repair.
- **live dialogs-1125:** Fillet r50 (test kernel_robustness::fillet_wider_than_its_faces_overflows_like_onshape, T1:
  y 10-30, z to sqrt(2100)-30, 7458.616 mm³), Chamfer 25 (T2 chamfer_wider_than_its_face_overflows_like_onshape),
  Shell 20 / Extrude 0 messages, Union disjoint Info, Part Studio Circular angle 0 OK: all as live. Open: 0° pattern
  makes 4 coincident parts (REQUESTS (9) live's count). Count 0: the app's field says live's "A parameter value is not
  valid." (feature_dialog VALUE_NOT_VALID); ops names the range. behaviours E13 "angle 0 → must be positive" is the
  assembly pattern.
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
