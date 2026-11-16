# Core modelling (agent "core"): current state

Updated 11-16 18:15 UTC. History (dated notes up to 11-15 14:48): docs/handoff/archive/core.md. Standing rulings:
docs/DECISIONS.md (read first; it wins over anything here).

## In flight
- **Landed b4c6b38c:** hole Up to next ends the shaft at the deepest exit round its rim (+1 µm when deeper
  than the axis's), as the host's opHole full_exit: asm-tvc-mount 7bc9d8 Hole 10/12/3 -> live 8/8/0 (model copy vibe5
  ~/agents/core-keep/m7b, 8 -> 19/120; next there Extrude 5's Inner Ring 11/27/18 vs 10/24/16, asm's withdrawn item).
  T1 features_a::a_hole_up_to_next_into_a_bore_leaves_no_web_beside_it. Feature 22 (same kind) needs asm's unlanded arc fix.
- Landed fa68e206: translator rule narrowed: 55552e704's whole-region rule (impeller 24 -> 26) turned
  c2-sm-rocker-enclosure-body 15 -> 9/28 (landed without a suite pair: my error); now only when another of the
  feature's queries names the same whole region (`shares_whole_region`). Release pair vibe7 (screws out): only rocker
  changed, 1190 -> 1196/1585. asm handoff note updated.
- Impeller Extrude 4 model side: closed, nothing to fix. The 12 mm² sliver alone never meets the offset wall: even its
  surface over all of its own bounds only grazes the prism at the sketch plane (replayed dump vibe5
  ~/agents/core-keep/e4dump/prism_until-10, experiment patch /root/core-scratch/uptowiden-exp.patch). Live's Extrude 4
  is the whole region, which we match (impeller 27/48 on main after blend's d7f9264c9; first difference Fillet 3).
- Landed 198c2b6d: rolled walls (a cylinder WALL without a BEND between two walls) develop in the flat at
  kFactorRolled: c2-sm-tablet-stand 3 -> 6/16; T2 sheetmetal_std::an_extruded_arc_is_a_rolled_wall_in_the_flat.
- Landed b0e98fa3: Move face on an active sheet metal part runs std's moveFace (sheetMetalAwareMoveFace;
  std_route.rs `moves_active_sheet_metal`): c2-sm-tablet-stand 6 -> 12/16 (only corpus document with it). T2
  sheetmetal_std::moving_a_sheet_metal_end_face_keeps_its_walls_and_bend_apart (fails without: 10 vs 14 faces).
- **Committed in core-oc 8deb8156c (rebased on main 18:10; with 1357fddb1 split-face attributes and 11f1335b9
  untrimmed cylinder split), all three in one release pair: a point on an elementary face is classified with its
  parameters moved by whole periods into the face's** (geom.cpp classify_periodic; g++-13 OK). tablet-stand's rolled
  wall faces cross the cylinder's seam, matched no definition piece (part_sources fell back to Face(0)), so std's SM
  Boolean never built the bend's tool: Extrude 1 = live (bool handed it back: the Boolean was right); with main's
  narrowed translator rule c2-sm-tablet-stand is 16/16 to the end.
  T1 kernel core::a_cylinder_face_across_its_seam_contains_its_points. A notch test across a rolled wall (/root/core-scratch/rolled-notch-test-wip.patch) still fails
  "Subtractive tool must cut all the way through ... rolled wall": std wants 2 cap faces per trimmed piece, ours has
  more (the caps split at the seam?).
- **Pair (six core-oc commits: 1357fddb1, 11f1335b9, 8deb8156c, 3ce19f704, 3ace68b3f, bcaf5ffdb): vibe7
  ~/agents/core-keep/sppair.sh -> spres (merge-base d29b5ee81 files in core-keep/base, branch copies core-keep/branch;
  started 18:33 UTC, the last two synced in before its build). Land all six when it shows nothing worse (save the
  probe patch first; 97b778e7d sits on top, see below: land.sh would take it too, so land the six from a branch at
  bcaf5ffdb or after a config-docs check).** (1357fddb1): opSplitFace keeps a split face's attributes on
  every piece** (host ops.rs op_split_face; Parasolid's). std Bend's imprint split the wall and dropped its WALL
  attribute, so c2-sm-rocker-enclosure-sm's Extrude 1 read "Removal destroys sheet metal part" (eachSheetStillExists
  found no WALL face): 6 -> 8/14; 14 sheet metal docs otherwise unchanged. T1
  custom_features::a_split_face_keeps_its_attribute_on_both_pieces (0 pieces tagged without). Face attributes have no
  snapshots (query.rs attribute_snapshot: edges / vertices only), so they follow a face only by its name.
- rocker-enclosure-sm Extrude 2 (next): Sketch 4's four Use circles (PROJECTED, live's external edges at x 45-60 mm,
  not on our part) come out as two: Extrude 2 names one region twice and cuts 2 holes of 4. Sketch translation:
  written into docs/handoff/skasm.md (landed 11-16).
- **Also in that pair: Split part by a cylinder face with Use trimmed off splits by the whole cylinder** (split.rs
  whole_cylinder; the pieces unified): c2-rib-wrap 4 -> 10/18 (Split 1: 2 parts, the disc 3 faces as live). T1
  features_b::an_untrimmed_cylinder_face_splits_off_its_whole_disc. Next there: Wrap 1 (3.8 mm³; its sketch text's
  ascent dimension untranslated). Land both (one commit each) when the pair shows nothing worse.
- Landed bb61a968 (lead cherry-picks it onto v0.4.55): Rib takes the ribbed part with its history chained (bool's
  suggestion), no second fuse. Was: c2-parting-line-draft-parting-line Rib 1 / 2 0.9 -> 7 s
  since this morning (vibe7 same-host A/B, binaries ~/agents/core-keep/tr/{morning,base}): Rib's merge fuse (part ∪
  ribbed part) is declined "a shared shape OCCT may keep or remake" and OCCT takes 3.5 s. Cases vibe5
  ~/agents/core-keep/plcase. Fusing a copy of the part made Rib 2 decline "a point near two vertices" (reverted).
  The chained history names faces as the fuse did (13 rib tests pass).
- c2-surf-workshop-britatap Touch screen split (4/22): our projected split curve (BRepProj section, a degree 7
  B-spline) ends 0.116 mm from the face's corner along the rim, where the sketch edge ends exactly; live ends it on
  the corner (2/6/5 vs ours 2/7/6). Kernel projection precision at a tangent end; parked.
- **Committed in core-oc 3ce19f704 (in the pair): paths-guides Loft 1 dome from a part's face** (features.cpp
  apex_tangent_loft read the seam's pcurves through the REVERSED side face, UpdateEdge wrote them back swapped:
  "unorientable", cone fallback) + a B-spline face's apex is a shown vertex (common.cpp shown_topology, as cone apexes):
  c2-loft-paths-guides 4 -> 8/29 (Loft 1 3/2/1 as live). T1 features_a::a_loft_from_a_parts_face_to_a_point_tangent_to_its_plane.
- **Committed 3ace68b3f (in the pair): two profiles along a straight path square to both = the plain loft**
  (features.cpp straight_path_square_to; Loft 2's path line lies past both profiles, path_sections gave up): Loft 2 =
  live (7/13/9, 5230851 vs 5230790 mm³); 8 -> 16/29. T1 features_a::a_straight_path_square_to_two_profiles_past_them_lofts_them_plainly.
  Three or more profiles on a straight path keep path_sections (unmeasured in live).
- **Committed bcaf5ffdb (in the pair): a path loft merges smoothly meeting sides** (merge_tangent_sides now also when
  `along`; live 1784's 3+-section rule): Loft 3 (two slots along an arc) 20 -> 17 faces, live 15/33/23. Left: two
  Boolean slivers (15.6 and 0.16 mm², 0.012 mm thick) along profile 1's flat side at x=-50.8, y 68-92: the path loft's
  surface there is a hair off Extrude 2's face. Likely cause: path_sections cuts each section with a plane square to
  the path; Loft 3's arc leaves profile 1 ~23 deg off its normal, so the first cut planes slant across the 197 mm slot
  and clip against the carried sweep's start: the surface near profile 1 bends into the part. Parasolid presumably
  places sections by the spine's parameter instead. Next item there.
  Mirror 1 times swing 0.3-1.7 s on loaded vibe5 runs; check on the Mac / in the pair before calling it.
  T1 features_a::a_path_loft_of_two_slots_has_one_side_face.
- c2-config-parts-links (10/35, largest gap after paths-guides): Sketch 1's solve flips its waist arcs to the other
  tangent branch when p goes 5 -> 8 mm, so the translator keeps the stored 5 mm sketch. Solver: in skasm's handoff
  11-16. Not core's.
- **Committed 97b778e7d (core-oc, NOT in the vibe7 pair): the translator builds configured documents at
  sketches.json's capturedConfiguration (the defaults live's lines are at) instead of currentConfiguration** (only the 7
  c2-config-parts docs record it; cf-sheet-metal-model-plus, without it, keeps current). Debug run of the 6 (screws
  out): every score as before (pulleys' Sketch 1 now kept; the rest stop at take-backs, in skasm's handoff). T1
  parity::a_configuration_variable_takes_the_captured_configurations_value. Land after the six, with those 6 docs as
  its pair.
- c2-surf-workshop-cpcurves Main pipe (thin sweep, 7/24): (1) Sketch 4's circle kept 18.8 mm off the path (translator
  take-back: in skasm's handoff). (2) Mine: live merges the faces swept along the path's three collinear lines into
  one per wall (6 faces; ours 10: a face per path edge). **Committed b173dc5bb (core-oc, NOT in the vibe7 pair):
  join_collinear_lines joins a sweep path's lines along one line into one edge (first piece's source)**: Main pipe's
  faces = live (only its box differs, the take-back). T1 features_a::a_sweep_along_collinear_lines_has_one_side_face.
- **Committed d2605e45d (core-oc): a pattern / Mirror with Add joins surface copies to the seeds' and merge scope's
  surfaces they touch** (bshared.rs join_surface split out of surface_result; pattern.rs, only when every copy is a
  surface; copies touching nothing keep the old push): c2-surf-workshop-singularities 2 -> 5/15 (Mirror 3 one sheet
  as live). Next there: "Loft with bad singularity" ERROR: a guided SURFACE loft from a sketch curve to a point (a
  guide's end), start Normal to profile, guides Normal to guide + Match tangent to Extrude 1's cap edge; live makes
  one singular patch (Problem loft 1/3/3). Guided loft to a point: a loft_guided item, not started.
  T1 features_a::a_mirrored_surface_added_joins_the_surface_it_meets.
- **Check before landing:** core-oc HEAD's debug run of c2-hunting-knife stops at Fillet 5 (32/52; GUARD 23/54/33, live
  22/52/32) where the 16:38 suite had 46/52. Suspects: blend's threaded Fillet 5 work on main (31105b015, 684e77e2e,
  before my 18:10 base) or my 8deb8156c (face_contains_point). The vibe7 pair's base (main 18:10) vs branch line for
  hunting-knife decides; if mine, fix before landing; if main's, tell blend.
- **Landing plan:** the vibe7 pair covers 1357fddb1..bcaf5ffdb. The second pair (97b778e7d + b173dc5bb + d2605e45d vs
  bcaf5ffdb) runs on vibe5, restarted 19:36 UTC: ~/agents/core-keep/p2pair.sh -> p2res (own src copy p2src, target
  p2target, base files p2base; src and target removed at the end). Land all nine when both are clean; or the six from
  a worktree at bcaf5ffdb (core-trace) if the second isn't done when the first comes back clean.
- c2-surf-workshop-manualblend Sweep 2: Sketch 5's CONCENTRIC rejected by the translator (skasm's handoff). Not core's.
- c2-surf-intro-examples Fill 1 (5/13, parked): 4 G0 boundary edges (2 fit splines, 2 sketch splines; they match
  live's lengths). fair_g1_fill returns nothing for them, so OCCT's plate (MakeFilling) balloons: 61911 mm², box to
  x 198 mm; live 23827.4. A Coons patch (GeomFill_BSplineCurves, CoonsStyle) gives 23669.5 (-0.66 %: outside the
  band), box inside the boundary's. fair_g1_fill declines because the edges share no vertices: ConnectEdgesToWires
  rebuilds them and the IsSame lookup (at < 0) fails. Matching by midpoint (/root/core-scratch/fair-fill-midpoint.patch)
  makes the fair fill run: 22754.7 (-4.5 %). Live's box bulges past the boundary (y -19..157.6 vs 0..152.4), so
  neither is Parasolid's G0 fill; needs a live capture of a plain 4-edge G0 fill (area, a mid-surface point) first.
- c2-surf-workshop-split Boundary surface 1 (6/12, parked): 2x2 profiles, defaults; ours 3803.55 mm², live 3762.61
  (+1.1 %). It goes through k::fill_surface (op_fill) but not its plate: a Coons branch for 4 G0 edges placed first
  changed nothing (edge count != 4 or Coons declined; not chased). Experimental coons_fill saved in
  /root/core-scratch/features-coons.cpp (full file, not a patch).
- c2-surf-workshop-loft Loft 2: 3-profile surface loft 7e-5 off live's area (interpolation detail); skipped.
- c2-sm-rocker-enclosure-body Extrude 6: Sketch 6 taken back by the translator (skasm's handoff 11-16). Not core's.
- **Lofts (mine, from dx's list):**
  c2-loft-guide-curves-wave-washer Loft 1: Sketch 4's rectangles are 63.5 mm off live's (an external COINCIDENT to
  live vertex JHk/JH4 unresolved: in skasm's handoff 11-16), which explains the "83 mm off the ring" below; redo the
  closed-path WIP once skasm fixes that. 4 profiles along a closed path; live closes it (4/4/0, 2080.8 mm³). WIP
  /root/core-scratch/loft-closed-path-wip.patch closes it round the path but comes out −11279 mm³: two profiles sit
  83 mm off the 26 mm ring, so MakePipeShell's carried sections twist. c2-surf-workshop-pipe: guided loft of more than
  two profiles unsupported (a Gordon surface over n profiles: big kernel item).
- **Impeller speed P0 (qa 11-16 04:40): waiting on blend.** Delete face 2 3.9 -> 26.4 s from v0.4.48 to v0.4.53
  (release A/B beside asm's full7: a hint; qa reruns it quiet). Chamfer 26 now builds (Error -> Ok, as live), so OCCT's
  Defeaturing gets a bigger part (trimming faces 13.2 s, FF 6.4 s, EF 6.1 s). Root: Part 1 differs from Fillet 28 on
  (ours 34/89 vs live 35/100; Fillet 29 fails, live 39/111; both hang on Fillet 3 (27), blend's next item), so the face
  "Sketch Stroemungskanal" is imprinted on differs: our sketch's circles are split (4 regions) but Extrude impellerraum
  adds 3 faces (live 20) and Extrude 6's ring is 4 faces (live 21, same 2337 mm³). Move face 1's 4 SWEPT_FACEs of the
  ring's split sides don't exist in ours: it fails ("Could not offset ...", 8 s in the reblend's defeature) and Delete
  face 2 defeatures a part live never has. Turning the fuse's unify off changes nothing there. blend (thread 0e6e9942) tells me when
  27-29 match (blend's Fillet 3 needs f24c11cc on main to measure); then recheck from Extrude impellerraum (the probe now prints a sketch's regions: NOSHAPE_PROBE_SKETCH).
- Landed 11-16: 3178c98b (bend strip runs on only where the wall stays inside the bend line), 03a052cf (union tool
  attributes follow, target's kept on merges: side-cuts-tab 18/18), 747c2bf0d (ScopeRule 18: a sketch on a face of
  a part a Transform moved is in the face's frame moved with it; Derived / imported parts keep theirs: hem-plus 6/6;
  live fan-blade-sweep's Sketch 2 shows a Derived part's x is its source's), 806ae96f7 (tool unify), 7bc8e345a
  (sheet cylinder frame; dx told).
- Extrude 33 (asm-pattern-std 36b90f) P0: 150 ms on main (debug, loaded vibe5); re-time once asm's Boolean feature
  pattern fix lands (Linear pattern 1 still errors upstream).
- **Handed off 11-16:** c2-config-parts-screws memory (14.8 GB): Revolve 9's unify (usd_kept_volume -> OCCT UnifyFaces)
  on the invalid body Sweep 1's OCCT fallback left: bool takes Sweep 1's cut, bool2 the unify bound.
- **Parked: knife Revolve 1's 19th vertex (branch core-pole-v2 in core-bis, not landed).** Our sphere's pole there sits
  where live's vertex is, (114.5, 0, -12.56), inside the dish. "Pole shown where the face has a boundary" regressed
  cf-box-dovetail-joint, prize-wheel-tim, cf-surface-pattern; "boundary not only circles round the pole's axis" still
  regresses prize-wheel-tim's knob (our sphere's poles lie on a horizontal axis, (±4.9, 0, 24.5): our sphere frame,
  not live's) and cf-surface-pattern (259 -> 344 vertices). Needs live's surface type of the knife's dish
  (SPHERE or a spun B-spline: Parasolid may keep a vertex where a spun profile meets the axis) before another try.
- Probe (uncommitted, /root/core-scratch/probe-wip.patch): ops parity::bodies_after_feature_probe (+ NOSHAPE_PROBE_SKETCH: a
  sketch's regions)
  (NOSHAPE_PROBE_DOC = a copy WITH its -wal / -shm, NOSHAPE_PROBE_AFTER, NOSHAPE_PROBE_OUT, NOSHAPE_PROBE_PART,
  NOSHAPE_PROBE_TAB): bodies as .brep, a part's faces (surface, edge indices), edges and vertices with shown flags.
  Saved documents' part names are Part N, not live's.
- Landed 11-15: 654611f6f / 8e3d52063, 52433a3d2, 348b6e241 (Kerf: offset retried in mm when empty), 9012904e (Shell by
  regions: rough check, convex pockets).

## Waiting on others
- **c2-sm-rocker-enclosure-body 15/28 (stable now):** Extrude 6's pocket is ~2 mm off live's and differently shaped
  (top face 89.76 vs live 66.95 mm²) because Sketch 6 isn't translated as live: its PROJECTED constraint finds no model
  geometry, its circular pattern isn't kept, and the solve (1.97 mm) is taken back. Sketch translation, not modelling
  (skasm's, paused until 11-17).
- **GearLab 90f78b:** with ae8b7f751 plus asm's unlanded 93346d03d (custom features with a `def` map; std 1777/1793)
  the loft builds; Gear Lab 1 then fails at an opFillet on the 157-face toothed solid ("Failed to fillet
  selections."): blend's. 4.3 s vs live 2.4 s, mostly 2 Booleans (1.7 s, bool's) and 1.2 M edge_point_at calls
  (0.53 s; not looked at). Repro vibe7: `NOSHAPE_FS_TRACE=1 NOSHAPE_PARITY_MODELS=~/agents/asm-keep/m90
  <ops test binary> parity::parity_models --ignored --nocapture`.
- **1dc32b (asm-infento-subs):** Draft 1 = live on dx's corrected input (-1306.543 vs -1306.54 mm³; OCCT 0030 +
  draft work bound landed af81dda05). Left: Fillet 2 / 3 / 12, Chamfer 3 (blend's); Extrude 42 17.6 s (bool's
  Boolean); Mirror 2 2.2 s (unify, bool2's). Hole 4-6 time: f1f212d03 above.
- **c2-impeller-loft:** first difference feature 15 (Boolean Flansch+Gehäuse 19/53/36 vs 19/56/39) is the fuse-then-unify
  merge (eval.rs fuse_targets -> unify_same_domain): bool takes it (11-15 22:40). Move face 1 / Delete face 2 speed only
  after that (they act on a part live never has). OCCT's Defeaturing dumps vibe7 ~/tmp/core/imp/dd.
- **asm-bjr-config 5d8bc1:** Sweep 1 landed 72248180. Kerf Compensation 1: live's corpus line is Manufacture=true
  (3 plain Ø2.2 holes, 3704.237 -> 3745.156 mm³, offset faces 454.835 -> 454.462 mm²); at false live leaves the part
  unchanged; ours offsets at false (dx's translation). Check our offset against the true numbers once dx has that input.
  Shell 1 is dx's. Extrude 55 (3.8 s vs 333 ms) is bool's fuse / unify.
- **e53bba:** bool's slit re-cut; then re-test the band-end patch vibe5 ~/agents/core-keep/e53bba-band-end.patch.
- **c2-hunting-knife feature 24:** live's own lines disagree (16-23 LINE, 24 OTHER, no geometry change); live asked
  in REQUESTS.md (11-15 00:45) to refetch 15-24 in one session.

## Next, in order
0. Impeller Move face 1 / Delete face 2 after blend's Fillet 3 / 28 / 29 (In flight). Left from 11-16's asks: c2-sm-rocker-enclosure-body moved 9 -> 15/28 between runs (A/B it before crediting).
1. Knife's dish vertex: asked live in REQUESTS.md (11-16 01:32) for the face's surface type and its edges / vertices;
   then retry core-pole-v2 (Parked above) on what live says.
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
- C++: g++-13 build of noshape-kernel on vibe5 before landing (`CC='ccache gcc-13' CXX='ccache g++-13' cargo build
  -p noshape-kernel`); direct-initialise OCCT handles; never name a variable `near` / `far` (empty macros on some
  platforms); never build a gp_Dir from a vector that can be zero (OCCT 8: std::terminate); signal-handler state needs
  `std::atomic_signal_fence` after it is set.
- OCCT patches (patches/occt, newest 0030): rebuilding a library on a vibe host needs `-march=x86-64` (check objdump
  for ymm / vfmadd); installing it on hosts is the lead's (x86) / mine on the Mac (~/noshape-host OCCT, backups beside).
  OCCT loops must poll `noshape_should_break()` with a deterministic bound, never a clock.
