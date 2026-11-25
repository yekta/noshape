# Handoff: skasm (sketch solver, sheet metal, frames, the parity translator's sketches): current state

Updated 2026-11-23 08:4x UTC (restarted 11-17 09:20). Read docs/DECISIONS.md
first; history is in docs/handoff/archive/skasm.md (read it only when a line here points there).

## NOW (11-25 08:30) — read first
- DROPPED (lead 11-25): the ops center_point_arc P1; core fixed it in db2fd8e7 (my ops edits reverted).
- LANDING Insert image as live G13: `image_box` fits INSIDE the dragged rect (min side, was max); preview
  = dash-dot rect to the pointer + the picture inside (`SketchEditor::image_preview`, psui sketch_overlay draws it via
  thread_local IMAGE_PREVIEW decoded once in load_image_bytes). Scene tests `an_image_fits_inside_its_dragged_rectangle_as_live`,
  `a_spline_closed_on_its_first_point_is_one_periodic_curve_as_live` (both pass, scene 216 ok). psui test
  `a_spline_arm_clicked_reads_its_length_as_live` (2187: arm alone selected, status "Length: x mm") NOT yet run.
  All green (sketch 175, scene 217, psui arm + bracket + trim): LANDING with the trim fix below.
- LANDING circle/trim vs live circle-trim-cons-1125: probe showed ours matches (end/middle trim, Length
  dropped, offspring no H, Radius kept) but for one gap: a circle/arc centre on the curve is now a trim boundary (`cuts`,
  sketch test `a_circles_centre_on_a_line_is_a_trim_boundary_as_live`, bin/ttrim2.sh -> ~/tmp/skasm/ttrim2.log). Circle
  centre snap = Coincident passes (scene). Bracket test hole tie tightened to Coincident only (run with tarm). Not done:
  circle->arc shares the line ends (live: point-point COINCIDENTs; same to the user); link_centre_snap on MODEL edges
  still ConcentricTo (live answer covered sketch circles only).
- NEXT splines/image (lead 11-25): REQUESTS skasm 11-25 07:55 filed (fit point / body / handle drags, curvature handle,
  end onto start, Bezier edits; image look, drags, scale by dimension, right-click opacity/lock). Known live already
  covered: 2175 closing, 2187 arm, G13 placement, Bezier Deg label, Esc commits.
- QUEUED 1290c4f1 (11-25 07:50) bracket everyday check (lead 11-25): psui `a_bracket_drawn_step_by_step_as_live` draws lines, a
  tangent arc, a hole, 3 dimensions, a mirror and a trim step by step against live's tool specs (behaviours 18337-18403,
  15793); all as live but one: a tangent / 3 point arc ended level with its own start took a HORIZONTAL. Fixed in `infer`
  (`own_start`: only Line, Midpoint line, Inscribed polygon align with their own first click, behaviours ~40336); scene
  test `arcs_take_no_alignment_with_their_own_start`. psui sketch suites 534 ok; clippy + ops sketch rerun (tbr4.sh).
  Open: the hole's centre tie (Coincident vs Concentric) and trim's offspring constraints not checked against live.
- QUEUED 9102b99e 11-25 02:40 (3 commits): (1) "free geometry joined to a conflict drags": `Sketch::drag` retries a
  Failed drag with `SolveOptions::unheld` = `unheld_constraints(1e-9)` unless the handle is FullyConstrained (fz211
  stuck free entities 161 -> 3; T1 misc free_geometry_joined_to_a_conflict_drags). The 3 (lines 92, 95, fit spline 215)
  are bodies held by Fix'd points (92 via c51 to 93's Fix c122; 215 has 4 Fix'd fit points): live leaves such a body
  still (REQUESTS 3074 fromOrigin-body); each free point of theirs drags 0.4-6.8 mm (probe zz_probe_fz211_three). Closed.
  (2) Insert DXF/DWG as live (live/skasm/dxf-insert-1125): `insert_drawing` no longer joins ends (0 constraints);
  `DxfDrawing::unrecognized` counts texts/hatches/faces/leaders; scene `insert_dxf` drops POINTs, returns
  (ids, Some(DXF_UNRECOGNIZED)); `DXF_UNREADABLE` for unparseable files; psui insert selects nothing, no message,
  Warning bubble (live's is yellow, text with "\n\n"); op returns `warning`. Tests: psui
  insert_dxf_drops_points_and_texts_and_undoes_in_one_step, e2e sketch_text_and_imports, dwg
  a_drawings_meeting_ends_stay_unconstrained. Parity row G11b (lenient parser noted; open: Units not preselected from
  $INSUNITS). (3) quick solves: `Sketch::set_quick_solves` (cache flag) makes `solve()` skip the DOF analysis; psui
  `with_sketch` sets it while `sketch_solves_slowly`; toggle_driving / add_dimension solve_with(default);
  refresh_analysis always analyzes (begin_drag keeps the drag gate right). fz211 probe (zz_probe_tools.rs, PROBE_QUICK):
  split 124 -> 27, delete 128 -> 35, trim 130 -> 37, Line 2nd click 131 -> 39, circular 135 -> 37 ms.
- QUEUED (11-25 05:26) "linear pattern ... quick": the 172-188 ms was add_dimension's analysed solve (100-178 ms) of
  the spacing dimension; link_linear_pattern now adds that Length driving directly (fresh instances, nothing else fixes
  it). fz211 linear pattern 32-37 ms (all probed tools now < 50 ms). T1 a_linear_pattern_draws_its_direction_line_as_live
  checks driving + label + no redundancy.
- QUEUED (11-25) slot width / conic rho the same way (`add_own_dimension`: measured, driving, no solve): fz211 slot
  preview 240 -> 60 ms, conic 3rd click 172 -> 41 ms. T1 slot_previews_until_... checks the width drives, not redundant.
- QUEUED (11-25, after 9102b99e) "constraints beside a conflict solve" (live sk-conflict-add-1125: everything added
  beside a red conflict is kept and solves): `Sketch::solved_beside_conflicts(before, around)` (unheld constraints
  all elsewhere and already unheld before) keeps solve_after_constraint's and the fillet radius edit's solve.
  T1 scene constraints_beside_a_conflict_solve. Open from live: a dimension on B's top made nothing (live's kit?).
- LEAD 11-24 TASK: sketch speed + robustness as felt. S300 T2 (latency_tests a_300_line_sketch_keeps_up_with_the_hand,
  321 lines, 483 constraints): open 33 ms (live 1.2-1.9 s), drag step to frame 15 (UI 13), release 23, dim edit 55
  (live ~1.1 s), unsolvable value warned 67, ✓ 16 (live 0.24 s). Model sketches <= 5 ms.
- Solver speed-up (commit "a broken sketch's edits 2-4x quicker", landing with the T2 test): failed trial solves
  (no exclusion) skip the DOF analysis; the post-placement solve runs without analysis first (analysed again only
  when kept); hold_the_base returns at once when the piece fails with nothing held; conflict_dims + redundant_nearby
  probes run side by side (par_keep, std::thread::scope; INSIDE set in each thread). Fuzz probes 1-20 identical to
  base; sketch crate, scene lib, part-studio sketch suites green. fz211 (qa fuzz, 438 ents, over-defined), probe
  (tprobe.sh + skasm-scratch/probes/speed-instrumented.diff): edit 1146 -> 225 ms / 431 -> 153 ms; ops T2: open
  457 -> 211 ms, change a dim 0.5-0.9 s -> 253 ms, drag step median 16, worst 452 -> 205 ms.
- QUEUED 80c322df 11-25 00:02 (lead 11-24 "off the UI thread"): a slow sketch (its feature's last regeneration >= 50 ms;
  `sketch_solves_slowly`, tests force it with `ui.slow_sketch`) solves a typed dimension off the UI thread:
  scene `prepare_dimension` / `finish_dimension` (apply_dimension = both + the solve); sketch.rs commit_dimension
  spawns the solve, `ui.sketch_solve` holds the sketch (with_sketch -> None, viewport input but camera/hover and
  sketch keys ignored), pending (live's overlay past 0.9 s); regen_sync of a slow sketch -> schedule_regen (the
  async completion now also runs update_profile_issues). T1 latency_tests
  a_slow_sketch_solves_a_dimension_off_the_ui_thread. fz211 UI probe (skasm-scratch/probes/zz_probe_fz211_ui.rs):
  commit_dimension on the UI thread 627-742 ms -> 3-5 ms, done 0.54-0.65 s later, same warning. psui sketch suites 531 ok, scene 212 ok, clippy clean. Not covered (follow-ups): drags (fz211 worst ~205 ms), other tools' own solves stay on the UI thread.
  Not covered: drags (median 16 ms, worst ~205 ms on fz211), other tools' own solves (sync, regen async).
- LEAD 11-24: sheet metal PARKED; focus = sketching as an individual user meets it. Landed today: ce1aefee (b33ec97e).

## In flight (11-22 17:45 UTC)
- CHECKING 2c5bffa7 (17:29, carries ab6987cc + the BEND-FACE-RECLICK fix): corner breaks in the flat (sm_flat.rs Break::
  Chamfer / ChamferAngle / FilletWidth / Conic(first, second, rho): asymmetric = ellipse quadrant w = sin θ/2, conic w =
  rho/(1-rho), a rational Bezier via make_bspline; first side n·(d1×d2) > 0) + model F (curved rips pulled back by the
  clearance; build_part refuses only curved bends). T2 sheetmetal_frames corner_break_*.
- 2f05e73c4 (committed, not queued yet): parity LIVE_SCATTER + LIVE_LINES_AS_OTHER for corner-break from feature 34
  (lead 11-22: live's conic / asymmetric / curvature corners are B-spline approximations, live/skasm/cb-curves-1122;
  our exact conics stay). Doc 35/37: 35/36 differ on the curvature corner (magnitude 0.7), still a circle on purpose
  (live's curve fits any G2 quartic/quintic to its 1.6 µm noise).
- Lead 18:0x: took from psui SKETCH-DIM-PLACE-PICKS-PLANE (fixed 18:2x, dimension_skips_plane checks the outline at the
  press) and the Part Studio FEEL-MC-* inference P0s. Plan: move asm's chooser (assembly_view.rs ~3170: 47.5 px square,
  midpoint reach, 15 px edge ends, MidAxis, entered -> centroid, Shift lock) into one pure fn by inference_candidates
  (crates/assembly eval.rs), both views call it; AFTER psui 3ff88768b (Shift lock, ConnectorLook::Placed, hover mask
  "with" layer) and asm f028bfc94 (dots on top = CAP-MARKER-HIDDEN, infer_vertex_glyph) land (asm keeps off the chooser
  meanwhile). Then MARKER-SHAPES (midpoints squares), EDGE-TRIAD-HIDDEN (Z on top), PS-VERTEX-LOOK, EDGE-PICK-RADIUS
  (~6 px, rim back half), MARKER-REACH (REQUESTS 18:08), PICKED-OWNER / SHIFT-NEAREST re-scored after psui's land.
- Live sketch-edit-feel-1122 (lead P0 feel set item 1; behaviours "skasm sketch editing feel gaps"), IN PROGRESS:
  S1 dim box: applies only on Enter; a canvas click keeps it open, unfocused, formatted (format_dim_box, commit_text);
  Tab (SketchDimBoxTab in key context "SketchDimBox") formats, stays focused; Esc reverts to dim_box_focus_text;
  underline #1651B0 focused / #d7d7d7; a press on the unfocused box refocuses all selected. Test
  dimension_box_applies_only_on_enter. S2: radial_dim leaders (outside: text→rim, arrow in; inside: Ø whole diameter,
  R centre→arc; off an arc's ends: leader to the nearer end); angles keep the placed sector (Sketch::
  orient_angle_sector at add_dimension, Hints.sector flag so old docs keep the label sector) and the arc runs on to
  text outside it. S3: selected glyph's entities unlit with the pointer away (SketchEditor::lit_picks); #fab601, box
  never picks glyphs, tiles drag: already so. S4: Trim circle pieces already so; Extend two clicks already so, but
  past the boundary: 11-18 (click 42 px past stops at the crossing) vs 11-22 (preview runs past): REQUESTS 18:08 R6.
  Open: the dimensioned line lit light orange while the box is open (check).
- (landed or queued) SK-BOX-FILL (scene SketchEditor::box_ends; psui sketch.rs render_sketch_labels draws app-core
  box_overlay, the polyline box gone; screen_to_plane cfg(test)); test backtick_on_a_clicked_sketch_line_opens_select_
  other_by_the_pointer. qa's SK-CTRL-BOX / SK-SELECT-OTHER were on green dc779d69f (15:58), before 905188129 (16:21)
  fixed both; sketch_boxes_take_whats_inside passes (GPU test, vibex8). Next: sgd.sh run -> commit, TICKETS rows
  "fixed, to verify", land.
- QUEUED ab6987cc (16:32): closed straight hems at a corner unfold off their own edges (rolled_strip tests the axis by
  the edge's curve ends; a closed corner's shared vertex is off the hem line): cov2-sm-hem-jog no false flat collision
  INFO; T2 sheetmetal_std closed_straight_hems_unfold_off_their_edges. SM docs (26): 23/26 match, nothing worse.
  hem-jog left: Part 1 29/56/29 vs live 29/61/34 at the closed corner (live: 2 mitre planes + 4 spline faces,
  behaviours "skasm (11-21 19:26)"), optional.
- FAILED b1998f3d (16:48, handoff rebase conflict; the same commit rides under ab6987cc's job): BEND-FACE-RECLICK-EMPTIES fixed again (held_face_under: kept wherever the preview has ANY
  face at the held face's depth, face_depths; a Flange before the Bend renames the plate's face); sm_feel test now with
  Flange 1 + the field clicked, fails on 049f4e313. TICKETS row "fixed again ... to verify".
- LANDED 16:21 0cc55505 (sketch gesture cells + CI e2e scratch name) as 04cc09dc; 16:26 3af78749 (frames: trim group
  unify, end cap circle at offset radius, gusset offset/aligned as std: cov2-frame-corners 23/23; UIS-ASM-START-FIRST-
  INSTANCE fixed) as a21fba49.
- Report to the lead when the queue is empty: 0cc55505/3af78749 landed, b1998f3d + ab6987cc results, model F state.
- Sketch long press: live = a release selects only if held < ~0.5 s (<= 504 ms selects, >= 533 nothing), same on a
  sketch line. psui adds app-core Viewport `long_press_cancels` (Part Studio 520 ms) for Drag::Pending(Left) clicks;
  sketch ToolDown/ToolUp picks (press_on_capture_pick / tool_captures_left) are mine once psui's change is on main.
- Open: live lists "Face of Sketch 3" twice in sketch Select other (we list once); a circle centred level with the
  Origin adds a construction Origin point a window box takes (not asked of live).
- Model suite 14:2x (vibex6, suite-1122b.log): 130/188 docs, 2021/2680 features; no doc's first difference is a sketch.
- REQUESTS 12:32 ANSWERED (live/skasm/lenbox-1122, behaviours "skasm length box side"): the line's length box sits LEFT of
  the drawing direction, centred on the perpendicular through the middle, near edge ~13-14 px off (A right-to-left: below
  +25 px; B downward: right +41; C 2nd segment: above -27; D down-left: 35 px down-right on the normal); only the last
  finished segment has a box; in iso it lies in the sketch plane. Midpoint: from (20,15.5) the level wins (end (0,15.713),
  HORIZONTAL + VERTICAL to the midpoint, no diamond); from (20,25) diamond + MIDPOINT, end exactly (0,15). Box side +
  offset (13 + 27.5|nx| + 12.5|ny| px) done 18:4x (scene line_length_field). TODO (P1): ours ends level ∩ the EDGE
  (coincident, glyphs horizontal + coincident: apply_external_snap's direction_onto with midpoint: true); live ends level ∩
  the VERTICAL through the edge's midpoint, constraints HORIZONTAL (line) + VERTICAL (end, external midpoint), glyphs
  — and |, dashed guides along the level and through the midpoint (E.json); differs on a slanted edge. Needs an external
  midpoint point; test a_level_line_onto_a_model_edges_midpoint_ends_where_the_level_meets_it to update.
- REQUESTS 14:30 ANSWERED: /root/parity/models/c2-constrained-surface/refs.jsonl (1254 lines): feature 4's 1121 vertices
  refs resolved (id, VERTEX, point, body, createdBy: Linear pattern 1 960, Sketch 1 80, Sketch 2 80, Origin 1 IB) + the
  Sketch 1/2 constraint externals (IB origin, JEC mirror face, JDC angle face); Sketch 2's DISTANCE BEDuJiRx8D0e
  externalFirst = IF, a VERTEX at (0,0,0) on body JFD. TODO after the P0s.
- REQUESTS 18:08 (open, live): mc-reach-1122 (the Part Studio inference reach: ratio vs px margin to the centroid).
- SM firsts left: corner-break model F (in flight), sm-loft-basic apex (archive "Loft apex 11-21"), hem-jog corner counts.
- REVERTED ab958b87b (Bend split-piece naming, bend-twice) by core in bad9c09e (11-24): it broke c2-sm-rocker-enclosure-sm (Bend 2 "Select a planar sheet metal face.", the rest cascades). Parked; redo later with the rocker as a counter-case. Rebase on bad9c09e once it lands.

## P1 TABLET-STAND-FACE-NAMES: DONE 11-22 (acf660cb)
- Cause (not the fuse): a rolled bend's planar end caps with exact arcs get tighter boxes (x 67.999994 vs the B-spline
  arcs' 68.066990), so their geometric `Role::New` key fell before the bend cylinder's (68.000000): New 11/12/13 swapped.
- Fix: rolled_bend also thickens plain and keys each cap by its plain twin's box (NameKey); fresh_part_names uses it.
  Saved tablet-stand copy: all Ok, names identical to the plain build's (probe /root/skasm-scratch/probes/zz_smnames.rs,
  bin/smn.sh). T2 sheetmetal_std a_rolled_walls_faces_keep_their_names_with_exact_arcs.

## Closed 11-19: big-sketch feel (no row >= 100 ms and >= 2x live; live: open 2.6-25 s, edit 2.7-13 s, press 31-56 ms;
ours: press 3-25 ms, drag 4-15 ms/step, edit 14-20 ms). Probe: /root/skasm-scratch/probes/zz_sketch_speed.rs (REDRAW too).
Sketch keys (shortcuts-sk-1118, 6 rows) landed a4b5d946; live confirmed all six on green.

## Next
- 850efe Extrude 6: bool's (P1 for them, 3b6725b2). Extrude 1-5 match live; Extrude 6's union leaves its r4 tool cylinders and
  Extrude 2's coaxial r4 ones unmerged (ours 33, OCCT 31 with a bad merge, live 28). Axes exact to 1e-12 mm: not the sketch.
- Midpoint alignment landed (303e9d91); live has no circle quadrant inference, nor do we: nothing left there.
- Handed: surf-intro-simple Loft 1 (core), surf-intro-examples Fill 1 overshoot (lead to route), welding-config
  Extrude 8 (core, fixing), manualblend Sweep 2 area / Move boundary (core).
- Form speed P0 binaries /root/skasm-scratch/apps: delete (done).

## Waiting on live (REQUESTS)
- skasm 11-24 20:15: sketch dimension text in iso, in the plane or upright (the Offset's "5" is in-plane; ours all upright).
- REQUESTS 6628 (glyph row moved off a taken spot, wf13 s14): live tried 3 runs, no clean boxes yet.
- Answered 11-18 13:xx: wave-washer sketches.json (Sketch 4 / 3 solved inward) and surf-intro Derived 1 sources
  (c2-surf-intro-*/derived/<id>/, live.jsonl by rollback): rerun wave-washer and the surf-intro docs.

## qa tickets waiting
- None open for me: all marked fixed / to verify in TICKETS (11-22 10:2x). Next: whatever qa / the lead files; else the
  "Next, in order" list below (glyph row off taken spots, hem-jog counts are optional).

## Next, in order
00. lead 11-20 SM suite models / qa 00d: all match but c2-sm-loft-basic (apex, above). Mac 12:30: 22/23, 259/260.
00b. qa P2 SUITE-INTRO-TAILFIN-INTERSECTED: LANDED 37638c76: an Intersection lands within 1 µm of live's ends (ours 0.19 µm
   off): no INTERSECTED left / no take-back in surf-intro boundary/fill/mutualtrim/ruled (A/B over the 7 INTERSECTED
   docs: nothing else changed). Left: Tail fin surface area 1.2e-5 off (3567.145 vs 3567.188 mm², ~0.6 µm of profile
   length, cause not found) and 3D fit spline 1's shape (starts x 32.70, live 33.94, same ends and start direction):
   core's (fit spline), told via the lead; Boundary surface 1 fails on it.
00g. A/B 11-21 18:50 (core's asks): asm-scissor-lift (asm P0) and c2-surf-intro-mutualtrim, parity.rs only.
   Studio 0139f2 Extrude 8: region_at tied Sketch 1's regions with Sketch 6's projected ones (same plane, point, area);
   by_geometry now takes the sketch the region query names. b582441a Extrude 1 KICB (as a Derived source, no refs):
   region_from_query's exact-boundary rule took a sliver beside one of three named pieces of the slot line; a region
   must have as many pieces of each named curve. mutualtrim: SWEPT_* of a Ruled surface (`<f>.….patch`) = its part,
   its face on the curve, its edge from the curve's start / end (ruled_piece_from_query), not the sketch edge.
   Plus curved rips with no pull (corner-break model F). A/B on vibe2: skasm-scratch/ab6.sh (~/tmp/skasm/scm/sc-<doc>:
   studio copies + refs.jsonl, mode 700, delete after). Default builds on vibe2; the Mac only via its one 4-job nice-19 slot (DECISIONS a16b39297).
0. USER P0 feel: rows of docs/parity/interaction-feel.md in my areas are "=" now; Hover timing is psui's.
0b. DONE 11-21 13:35: sketch-line-from-midpoint / sketch-bezier-polygon / sketch-spline-handle-arm match with the camera
   (3/3, 40/40 states, green 6241cf347 on vibe2; skasm-scratch/bin/uipg.sh); the 7 asm takes are asm's.
1. LANDING 11-21: a DISTANCE to a model circle in a sketch without live's frame takes our frame (loft-paths-guides
   Sketch 2 solved, Mac suite otherwise identical). loft-paths-guides left: Sketch 4's 3 PROJECTED; Sketch 6's take-back
   (0.5 in to JUV, Extrude 1's seam line at x -50.8, ours elsewhere: 390 mm move; older than this). (Sketch 5's PIERCEs on its
   Intersection's ends are skipped as the tool's own). The other PIERCE take-backs (09:20, sec.log) all sit behind an
   upstream difference: surf-intro tail fins (Intersection face from the unbuilt Derived 1), manualblend Sketch 6 / 7
   (sections on Sweep 2, wrong after Sketch 5's CONCENTRIC), pipe (core's Loft first), knifehandle (Sweep 2 first).
   Unfound PROJECTED (45): mostly qCompressed queries with no ids (hunting-knife Sketch
   12, no feature lost); rocker-enclosure's wait on core's split-face fix.
2. Suite 11-18 05:45 vibex8 (~/agents/skasm-keep/suite-0545.log): 88/126 docs, 1292/1585; untranslated sketch
   constraints 121 (45 PROJECTED model geometry not found, 24 PIERCE rejected: loft-paths-guides, surf-intro tail fin,
   manualblend Sketch 6/7). surf-intro-examples Fill 1 (area 2.6x live) and -simple Extrude 1, -mutualtrim Title next.
3. core's sketch cases below. Rocker-enclosure Sketch 4's projected circles: bodies RXBD.. outside the part box
   (x 45-60 mm), waiting on core's split-face fix.

## core's sketch cases (2026-11-16), queue item 3
- c2-sm-rocker-enclosure-sm Extrude 2 (now its first difference once core's split-face attributes fix lands, 8/14):
  Sketch 4's four circles are PROJECTED (Use) constraints whose live external edges (refs.jsonl
  AjhQtHt4MbMf.{0..3}.project: x 45–60 mm, y ±78, z −91 / −21) aren't on our part. Our sketch comes out with two
  circles (both rows map onto one), so Extrude 2 names one region twice ([0, 19] twice) and cuts 2 holes of live's 4
  (ours 26/52/24, live 28/56/24). Probe: `NOSHAPE_PROBE_SKETCH="Sketch 4" NOSHAPE_PROBE_PARAM=entities` in
  parity::bodies_after_feature_probe (core's probe patch).
- c2-loft-guide-curves-wave-washer Loft 1 ("Could not create loft", 6/7): live says JHk / JH4 are Sketch 2's points
  (POINT bodies JHb / JHv at (0, ±25.4, 3.81) mm); our Sketch 4 now coincides with them but solves its rectangles
  outward (live's stored geometry is inconsistent): waiting on live's sketches.json (REQUESTS 06:2x).
- c2-surf-workshop-manualblend Sweep 2 (11/19): Sketch 5's CONCENTRIC to vertex JNE now kept (anchors_off counts a
  CONCENTRIC in the plane, within 10 µm when the stored was 0.1 mm+ off); our sweep now lies where live's does (x
  −60.9…−26.3), but it takes the path's name "Curve 2" and the wire becomes "Surface 3": Sweep's body identity (core).
- c2-sm-rocker-enclosure-body Extrude 6 (15/28): Sketch 6 now kept (its hexagon's pattern centre COINCIDENT with
  the Use of JRR's centre, which live's refs `center` has; the stored was 2.8 mm off). Extrude 6's counts now match
  (39/97/58); volume 379696 vs live 379830 mm³ (live cuts only the hexagon pocket, 66.81 mm² x 5 = 334.07 mm³; ours
  134 more = 26.8 mm² x 5). Extrude 6's ids JtC / JtG map by text (ids fail): our regions 110.97 + 22.94 mm², live
  84.16 + 22.94. Our first region runs past the hexagon (x -86.01 vs -84.04, y to 43.01 vs 41.04) to the pocket
  floor's imprinted corner arcs (boundary entities >= 2^30): how regions meet imprinted face edges differs.
  Sketch 5's take-back is right: its circles are stored on live's centres, our Use of JZB lands 6.43 mm off (our
  model's edge: core's split-face fix).
- Settled 11-18: config-parts links / pulleys / linear-rail / standoffs / sprockets match to the end; cpcurves has no
  sketch difference left (first: 3D fit spline 1, "Cannot evaluate tangency for end condition.").

## Agents
Thread ids only in /root/lead-scratch/roster.md (lead 11-19 23:00): read it before every message_agent; never copy ids.

## Waiting on others
- psui: told 20:3x the tab row is a pick difference (live hit Flange 1's wall).
- dx: cf-spur-gear's per-host Sketch 2 failure = the near-singular sketch 62893b1ed takes back (told 21:3x).
- Not mine (handed): knife / impeller geometry (core); fan-blade Boolean 2, gamma / wire-clip Boolean 2, shaped-flange
  Booleans, surface-pattern sphere cuts (bool); setback fillet (blend); hex-bookcase mesh Uses (dx).

## Open questions (none blocking)
- hat15b Sketch 1 (inserted in the doc's last microversion 1229, 11-17 ~17:30): its used point 19 (Vertex ref) was
  stored at (33.679, 45.182) mm but resolves to the vertex at x 34.75 in 0.4.43…main; nothing of the model lies at the
  stored x in any build. Not upstream geometry (not core's). Not reproduced: today's snap there stores 34.749999642 (the
  mesh's f32), now the vertex's exact point (`a_model_vertex_snaps_to_its_exact_point`); no edge or vertex of any part
  lies at the stored x. 8b481251 makes the editor take the solve on opening, so the doc edits right.
- s236 (fuzz probes 11): live solves three linked quads at once; per-case placement rules don't converge. Decision
  for the lead if fuzz work resumes there. s228 has its rule (grow_slot); s226 is 3 µm off (live's anchor drifts).
- Jog at its reduced angle: our flat lays two bend strips 0.73 mm over each other (features_c
  jog_preserve_material_off_and_errors' second case; live's doesn't): the jog's definition has two middle faces 0.79
  mm apart, so coplanar_rip doesn't join them.
- Sheet metal "Collision in sheet metal flat pattern." INFO for zero-gap contact at a tear (config-assembly 08590a's
  Flange 1 … Mirror 1): needs a contact rule, not an area check. (tablet-stand's was the curved wall: fixed in A.)

## Current numbers
- Model suite 2026-11-16 00:01 vibex6 (suite-2354.log): 80/111 documents, 1019/1292 features (24 documents new to the
  set). Sketch constraints left to the solved geometry: 586 over all, 74 over the 87 of 2026-11-15 (16:10: 83).
- Sheet metal speed, quiet vibex6 19:00, ops binary of 17:43 (ours vs live ms): corner-break 1637-1782 / 1972,
  shaped-flange 809 / 1026, overcut-dogbone 210 / 270, hem 65 / 95. All four are faster than live; the suite's slower
  figures were host load.
- Exchange: model-plus STEP / IGES import in live as 58/151/94 (6f5b151e snap_coplanar); dx "live imports ours" 43/48.
- Sketch fuzz vs live: 801 of 801 counted (29 live defects, 29 known settles, of 830); probes 20 11/11.

## Hosts and rules (mine; DECISIONS.md has the general ones)
- Builds and crate tests on the Mac: `NOSHAPE_RB_HOST=mac tools/tiers/rb.sh bash -c '…'` (one slot; rb.sh keeps file
  times: touch an edited file inside the command when a result looks cached). No suites, pairs or app runs there.
- Suites and A/B pairs on vibex6 (or vibex7) through `~/bin/slot <cmd>` there. ~/tmp on vibe hosts is pruned after
  6-12 h: lasting binaries in ~/agents/skasm-keep (cli-*, ops-a / ops-b).
- Any change touching projection / Use: userdocs check (hats hat8 / hat15 / hat15b, dock) on vibe5 against 0.4.52
  (~/qa-rel/0.4.52/noshape-cli) before landing: candidate CLI built on vibex6 (buildcli.sh), copied with `scp -3` via
  this host (vibex6 can't reach vibe5). The private documents only as mode-700 copies in ~/tmp/skasm (or userdocs.py's
  ~/tmp/qa/ud-<pid>), deleted after with their outputs; never in the repo.
- Live captures: append to /root/onshape-research/live/REQUESTS.md (timestamps from `date -u`).

## How to run my tools (/root/skasm-scratch/bin)
- fullsuite.sh <log>: the whole model suite through rb.sh on $HOST (default vibex6); wrap the remote command in
  `~/bin/slot`. dropped.py <log>: count the constraints a suite log leaves to the solved geometry.
- ut3t.sh <doc>: one model-suite document with NOSHAPE_PARITY_TRACE (translator TRACE lines, untranslated, DIFFs).
  ut3p.sh <docs,comma> <models dir>: the same from another models dir (a fresh copy), binary kept as skasm-keep/ops-c.
- smab.sh: release ops test binaries for skasm-base (A, /root/projects/noshape-wt/skasm-base: reset it to my
  branch's base first) and skasm (B) on vibex6 -> ~/agents/skasm-keep/ops-{a,b}; vibex6 ~/tmp/skasm/smrun.sh <runs>
  <docs,comma> interleaves them (A/B by the match lines' times).
- buildcli.sh <name>: release noshape-cli -> ~/agents/skasm-keep/<name>.
- Profiling: NOSHAPE_KERNEL_PROFILE=cpu, NOSHAPE_FS_PROFILE=1, NOSHAPE_RECORD_BOOLEANS=<dir> (one .brep + .txt per
  distinct Boolean); Mac `sample` + sample_under.py <sample> <root symbol>.
- Fuzz: fz/cmpprobe.sh <set> <case> (initial / ours / live); probe.sh runs a probes/zz_*.rs test copied into
  crates/ops/tests (zz_sketch_speed, zz_skdump, zz_sksolve; never committed).
- stepadj.py: coplanar adjacent planes in a STEP file. cfg.sh: one assembly studio alone in the model suite
  (archive: "config-assembly's sheet metal studio 08590a").
- Parity translator changes asm made for the assembly corpus: archive section "Parity translator changes by asm".
