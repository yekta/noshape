# bool2: history (archived from docs/handoff/bool.md 11-15 19:25 UTC; current state in docs/handoff/bool2.md)

## bool2 (second agent, from 11-11 23:40 UTC; worktree /root/projects/noshape-wt/bool2, scratch /root/bool2-scratch)
- **11-15 12:13 UTC (landed 7eedbae5: the 10:21 stack), helical decline fixed, valve rows; gate54 on vibe5 (bb02343ea,
  708fba52d):**
  - c2-helical-sweep's new cut 9b68aa2b on ours again (bb02343ea): 290 ms in OCCT -> 48-60, OCCT's 13/47/36, valid,
    volume 1.2e-9 apart. Three things: (1) the thread cutter's two flank sections ended where they cross the bar's rim
    2.2 µm apart (the cylinder's section and the top plane's), each a new vertex of 0 tolerance: now a section's
    End::Point takes another new vertex within its tolerance lying on the same edge, merged (end_on_edge); (2) its
    seam crossings were OCCT's curve's points, 0.3-1.7 µm off the cylinder, past the seam's 2e-7 reach, so the seam
    wasn't split there: the existing "on an edge lying on the other face" rule (a vertex on the edge reaching the
    curve's end) now takes the face's seam too; (3) the made cylinder faces' curves then span 2π + 1e-4 rad (OCCT's
    curve on the face ends off the seam by its own error): to_body's "more than a turn" allows the edges' tolerance
    over the radius.
  - c2-valve-config's Circular pattern 1 (d561e545, one cut by 7 tools, ours one at a time since they meet below the
    part): each step BRepCheck'd the whole result; past the first only the made faces and the shells now
    (RustTargetChecked, made_valid; 708fba52d): its checks 50-75 -> 13-22 ms of ~230-320. Left there: OCCT's cone x
    cone sections, 8-10 ms a step (7 steps); the copies' sections could be shared (moved, as within one Boolean) but
    each step is its own Boolean; all 7 at once declines "a non-manifold result" (pieces of the last two copies
    unclassified where the copies meet), even with the tools clipped to the part's box first (6 ms for all 7).
  - c2-valve-config's Extrude 3 (e5811c1b, 80-85 ms): 59 ms is OCCT's sections ahead, 23 pairs of the body's revolved
    B-spline profile (axis z) x the tool's B-spline extrusion (along -y), ~33 ms CPU each, all OCCT's walk (our
    refinement 0). Would need our own revolution x extrusion section. Not started.
  - c2-impeller-loft's Extrude 4 fuse (f10ce62e) now ~200 ms, 115 of it OCCT's sections ahead; OCCT's per-pair
    IntTools_TopolTool sampling of the same B-spline face could only be shared by an OCCT patch (not started).
  - Chamfer 26's cut (f37371b8, OCCT 0.9 s; ours 70-140 ms to the decline): the piece is the run-on chamfer tool's
    end cap, a whole plane face 0.3 mm long (an arc and a B-spline edge, no section splits it) standing across the
    corner of the body's strip face (B-spline, face 3) and the face the arc was drawn on: the arc lies on that face and
    9-12 um outside the strip, the B-spline edge on the strip and 9-12 um inside the other, the point 8.5 um off the
    strip, all inside the Boolean's 16.5 um. OCCT's result has no such face (Out). Signs mix, so tangent_side can't
    say. Tried: ray parity at a thousandth of the tolerance for such a wedge piece gives Out (as OCCT), then the
    result is non-manifold: the tool's arc (on the drawn-from face) and its B-spline edge (on the strip) are each
    used once, neither face split along them; OCCT's result looks like the arc, the B-spline edge and the faces'
    common edge (all within 12 um) taken as one. Reverted; needs that common block.
- **11-15 10:21 UTC (landed 897b690f: the 08:30 stack), more speed rows; gate49 queued (dd20c8d4a, 943e3c90d):**
  - c2-housing-draft (qa: Bohrung 1 unify 62 ms, Rotieren 1's 24): the first unify of each result tried three other
    face orders (unify_in's reordered retries) after usd_kept_volume had found the same merge rejected in every start;
    now skipped then (t_usd_order_moot), the pairs still marked stuck: 81 / 119 -> 22 ms (dd20c8d4a).
  - c2-impeller-loft after chamfer 26 builds: Extrude impellerraum (31) and Boolean 3 (34) declined "a closed edge":
    a circle round from its one vertex 1.2e-7 of a turn short. Now a whole turn where the shortfall is within the
    vertex's tolerance (943e3c90d): 1.49 -> 0.5 s, 0.47 -> 0.1 s, OCCT's counts, volumes 1.4e-8 apart (GProp's own
    variance bound there 3.5e-10 m³, 270x that).
  - Exact face boxes (ExactBoxes: apart_across_slab / resting_on_plane, OCCT's PSO on offset faces) kept on the face's
    OcShape (myExactBoxes), so a chain of Booleans on one part finds each once: the seat fuse 1.09 -> 0.96 s (18703bac3).
  - free::ray_hits on NURBS faces of several spans (and extrusions): Newton only from the Bézier pieces (each span
    halved twice, control-point boxes, the piece path's clear-of test) the ray meets, where the grid's grown boxes
    let it meet ~45 a ray (472bb861e): c2-helical-sweep's cut 2695 -> 136 patches met, 35k -> 3.9k Newton steps,
    best 149 -> 82 ms. Capped at 256 pieces a face (the grid past that): cf-surface-pattern's base surface with 4096
    ran 9 starts a piece and lost 8 %; with the cap level.
  - Not done: c2-impeller-loft's Extrude 4 fuse (f10ce62e, 0.5-0.7 s): extrusion x B-spline sections 340 ms each in
    OCCT, a third of it IntTools_TopolTool::SamplePnts (per pair); Extrude Flansch's fuse (e05c4cf9, 0.23 s) declines
    "a shared shape OCCT may keep or remake".
  - c2-helical-sweep regressed from main's input, not ours: Sweep 1's cut is now 9b68aa2b (OCCT's result 13/47/36,
    the recorded 2e082aa1 14/34/22; likely core's 608e3b402, helices / circles starting at the sketch's x) and ours
    declines "two edges leave a vertex in one direction" (arrange.rs; an older binary too): a 2.1 µm section piece
    along a B-spline face's iso boundary at v 63.5 mm, from the same point as that edge, not split into it. OCCT
    then takes the cut (~290 ms against ours ~80). Fixed 12:13 (above). Recording vibe5/vibe6 ~/agents/bool2-keep/hel-new.
  - Left: chamfer 26's own cut (f37371b8, OCCT 0.9 s) declines "a piece's point on another solid's face": the run-on
    tail lies along the strip's tolerant B-spline face (8.5 µm). Delete face 2 (core's OCCT defeaturing) is 17-26 s on
    the chamfered body (was 4-7 s); told core and the lead (decision needed). Recordings vibe6
    ~/agents/bool2-keep/imp-new; CLIs cli-old / cli-new and ~/tmp/bool2/ab_docs.sh (old vs new alternating, regen_ms).
- **11-15 08:30 UTC, speed rows (qa / lead) and the gate's grooves; stack 65a2dc4e2..bfb5b2910 gating on vibe5:**
  - Gate45 failed two knurl tests (grooves_crossing_just_above_the_end / _37_um_): the segmented volume check in
    merge_cylinder_groups (742a166f5) merged pieces OCCT's GProp keeps apart. Now opt-in (c8319d982): only
    op_unify_precise (`unify_same_domain_precise`, rim_chamfer_cut's unify) sets it. The chamfer test checks the merge
    (input + 3 faces, live's 30 -> 33: our input has one face more than live after Boolean 1, upstream).
  - c2-helical-sweep (bedaeda88, seam_first): the long cylinder's section at once, 96 -> 78 ms.
  - cf-surface-pattern d6500b16 (baa0daad4, sphere_sections): two spheres' exact circle, cut at either's seam (one
    piece a turn round where it rounds a pole), pcurves projected to 1e-7 and moved into the face's turn; OCCT's where
    the circle comes within 10 µm of a seam's plane or a face's latitude bounds (the Boolean calls such a pass a touch:
    a 2.3 µm one in d6500b16), or the face isn't a whole turn. ~0.7 s CPU less a cut (same binary, toggled), the
    volume nearer OCCT's than the recorded one (T1 balls_cut_from_a_ball_off_its_axis). Left there (DBG_AHEAD,
    /root/bool2-scratch/dbg-ahead.patch): sphere x B-spline 272 pairs, 178 real sections at ~9 ms each (1.7 s CPU; the
    lever is our own marcher), 94 empty at ~2 ms; sphere x offset 85 empty pairs, 0.26 s (bool's offset work).
  - c2-impeller-loft's Extrude Bohrungssitz Ansaugflansch (qa, 1271 ms): (1) its checked-glue fuse of nine seats
    declines the plain fuse ("pattern instances overlapping") and goes to rust_fuse_trimmed, which converted the part
    once per seat: now the plain fuse's converted shapes are reused (8904a17f8), 2.42 -> 1.45 s at load 30. (2) Its
    unify: merge_cylinder_bands integrated and BRepCheck'd the whole shape twice per candidate pair; now only the faces
    it changed (their shares after less before, one GProp call) and the shells (BRepCheck_Shell / _Solid)
    (bfb5b2910): 1070 -> 100 ms, the same 72 faces. Recording: vibe5/vibe6 ~/agents/bool2-keep/imp46; probes
    xprobe probe_case_repeat / probe_case_unify_sample, bin/callers.py (a frame's callers from a sampler profile).
  - qa's c2-valve-config / c2-helical-sweep rows are cb65f775a / bedaeda88 above.
- **11-15 06:25 UTC, the impeller's chamfer 26 and two speed rows:**
  - Chamfer Stevenrohr Halterung (OCCT's chamfer fails on the stern tube end's two 0.3117 mm spline edges): new
    fallback (crates/kernel/src/rim_chamfer.rs, blend_section::rim_chamfer_cut, after OCCT's retry in detail.rs's
    chamfer): an Offset chamfer of a plane face's rim whose sides run along its normal, as a cut; each edge's chamfer
    ruled d down its side to its offset e in; at an open chain's ends the chamfer runs on along the edge's own curve
    to the face across the plane's next edge (live q2985c). Feature 26: 35/90/56, live 33/90/58, volume live's within
    5e-6; mitres and run-on ends within 3 µm. Left: Part 3's cylinder stays three faces (the two patches the cut
    leaves): USD won't merge them even on f0's own surface handle, and merge_cylinder_groups' one-loop merge was
    rejected; fixed (cfd2b71d7): merge_cylinder_groups takes the cut's 6 µm edge gap and, where GProp disagrees, the
    segmented integrals (GProp was 2.9 mm³ off): feature 26 is 33/88/56, live 33/90/58.
    Probe and dump: vibe5 ~/agents/bool2-keep/imp-chamfer26, xprobe probe_rim_chamfer (untracked, all.rs patch
    /root/bool2-scratch/all-with-probe.rs).
  - c2-valve-config's pattern cut 0d1cff8f (bool's row): its cost was OCCT's empty cone x cone sections (35-95 ms a
    pair, most 0 curves). New free-of-OCCT test (cb65f775a): round faces on parallel axes whose cross-section circles
    never meet over their shared stretch are apart (brep cyl_cyl and the sections-ahead pairing); about one empty pair
    per tool is left (angularly apart, not radially). vibe5 at load 49, main's binary and ours alternating, best of
    5: 404.6 -> 264.6 ms, the result the same (47 faces).
  - c2-helical-sweep's cut 2e082aa1 (bool's row), 140 ms alone: by samples (xprobe PROBE_SAMPLE, bin/samples.py)
    IntTools_FaceFace 45 % (ComputeTolReached3d's PSO 21 %, IntPatch 17 %), sections_past_seam's second section 16 %,
    our ray_state on the sweep's B-spline faces 17 %, pools spinning 17 %. The PSO tolerance feeds our edge
    tolerances and refinement, so skipping it isn't free; the marcher is dropped. Left as it is for now.
  - Fillet 3 (27) handed to blend (lead 06:20): section_blend's frame_at finds no probe inside at ±20 µm on Je1 /
    Je5, likely at their x 1.5 end by the chamfer's run-on face and Jeh (6 µm tolerance).
- **11-15 01:55 UTC (from 11-14 23:35), c2-impeller-loft from feature 12 (lead's item; core has no WIP there):**
  - Feature 12 "Extrude Flansch": our flange's geometry is live's at its points; the 0.07 mm³ is live's
    integration (core's handoff reached the same).
  - Feature 15's missing vertices: JJM / JJU are the points Loft Gehäuse's vertex matching split the circle's long
    arc at (its 278.6° arc in thirds; addGuides is off, the loft is unguided). Live keeps them for good; our unifies
    joined the arcs (one side face, one end face). Fix (landing with the next): the loft marks those edges kept_ends;
    OwnCopy (the Shell's copy) and every op's result history carry kept marks. They now last to feature 30. The other
    three (KOMB / JOY / KOQB) are the flange wall's near-tangent sections with the housing (2.3 µm apart):
    ill-conditioned, left.
  - Feature 24's Part 3 (5/9/6, live 5/10/7): live's 0.047 mm piece is an up-to-surface artefact; left.
  - Feature 25 Boolean 1 (32 faces, live 30; live q2985b): the stern tube's two 0.3117 mm strips are extrusions of
    pieces of the sketch spline whose rest is Part 3's side; live merges each into that side (JdK / JdG). Fix
    (landed with it): share_mirrored_extrusions takes pieces meeting end to end either way along the
    curve whose join loses its knot (pattern copies keep theirs), one surface on the joined spline; usd_kept_volume
    takes merges on those surfaces as unchanged (its fixed-rule integrals put them at 1.8e-12 m³; exempting every
    same-surface merge took cf-surface-pattern 129 -> 116 faces, so only these). Now 30/79/50 (live 30/83/54); every
    edge around the strips is live's; the rest is the near-tangent y +-27 set and Extrude 4's 0.047 mm piece.
  - Gates 39 / 40 CHECK (landed so): cf-surface-pattern's Surface Pattern 3 (already differing) 129/384/257 ->
    129/383/256: two arcs at (-56.155, -44.024, 11.192) mm joined; live has neither form there.
  - Next: feature 26 Chamfer Stevenrohr Halterung fails in ours (OCCT's chamfer of the tube's end: the arc and the
    two 0.3117 mm strips' edges, 3.5 mm at 20°; either strip edge alone fails, the arc alone builds invalid).
    Reproducer vibe6 ~/agents/bool2-keep/imp-chamfer26 (edges 16 19 81, faces 2 3 28; xprobe probe_chamfer_dump).
  - Boolean Flansch+Gehäuse 0.7-0.9 s alone (live 154 ms): OCCT's sections ahead, mostly the shell's offset faces
    against the flange's planes (40-160 ms each; offset surfaces have no FreeSurface, so free::apart can't rule a
    pair out) and the near-tangent wall pair (305 ms). Per-pair timing patch: /root/bool2-scratch/dbg-ahead.patch.
  - Move face 1 (5-6.5 s, live 26 ms) and Delete face 2 (3-4 s, live 12 ms) are OCCT's Defeaturing on a part that
    already differs (Extrude impellerraum 35 faces, live 59; its sketch is the x = 0 face of a part whose fillets
    26-29 fail): their time follows the upstream.
  - Saved documents: a regeneration in a fresh process recomputes the marks; nothing stores them (bodies aren't
    cached on disk), so a reopened document is the same.
- **11-14 19:5x UTC, the marcher revived and measured (branch bool2-march2, pushed: 0434839f4), not landed:**
  plane x B-spline only, OCCT's section where the march finds none or a curve is closed or doesn't end on a face's
  boundary. On rings-loft's cut 3c0b53fe's 187 pairs outside the Boolean: 4.3x OCCT's time, OCCT's curves on ours
  within 1.2e-6. In the Boolean: gate FAIL. The replay is clean (0 differing), but c2-sm-corner-break's "Sheet metal
  model 1" goes 42 -> 41 faces (its six 1-tool cuts come out equivalent, 1-3e-8 off in volume, and the sheet metal
  model is that sensitive); a closed section (features_c::thin_sweep_as_live, 0.27 mm3) needed the closed-curve rule.
  rings-loft and cf-surface-pattern alone on vibe6 (load 12) were no faster: rings 4.9-7.6 s against main's 3.5-6.9
  in all three rounds (rejected pairs lose their place in OCCT's parallel batch and are asked one by one).
  **Dropped for good (lead 11-14 ~20:0x):** its 4.3x on isolated sections didn't reach the documents, and within-
  tolerance differences broke a sheet metal model; don't revive it. The branch stays only as a record.
  - The impeller's last vertices (live 2985, /root/onshape-research/live/bool/q2985/out.json): JJM / JJU at x 0
    (0, +-23.550655, -56.3968) and IO, JOE, JOk on the z 0 intersection curves are the five our feature 15 lacks.
- **11-14 11:10 UTC: landed 8bb1a09d (fan-blade bolt cut, impeller mirror fuse, arcs in the result check). Fresh
  per-document recordings (vibe6 ~/agents/bool2-keep/bydoc2/rec, today's main): 875/945 ours, 22 OCCT.** Left:
  - c2-gamma-imported / c2-wire-clip-imported f68f4393 (Boolean 1, 87 tools, 7.4-7.7 s) and 678f4bf2 (Boolean 2, 1.9
    s), parked (patch with debug prints: /root/bool2-scratch/gamma-wip.patch). Peeled so far: (1) "cylinders nearly
    coincident": coaxial 2.10 / 2.11 mm holes 10 um apart, the Boolean's tolerance 4.9 um from one vertex elsewhere,
    the faces' own 1e-7: the grey band by the two faces' own tolerances (their edges' and vertices') lets them be
    apart; (2) "two edges leave a vertex in one direction": a 1 mm hole's seam is a straight B-spline (imported),
    the ruling test compared its middle's angle without the turn (a rounding put it 2 pi on): compare modulo a turn;
    (3) 678f4bf2 then "a piece's point on another solid's face": cylinder pieces 1e-9-3e-7 off coaxial cylinders of
    other bodies (near-coincident, under the 4e-7 tolerance) and one cylinder piece tangent to a plane at its point;
    a retry at the piece's other points (state the same all over a piece) isn't enough. f68f4393 after (1)-(2):
    "an arc across a cylinder's seam": a neighbour body's hole edge (r 1 mm, tilted 5 um over 2 mm, off centre) ends
    10 um past the cylinder's seam. Needs: coincidence of cylinder pieces by the faces' own tolerances, and arcs
    within the tolerance band of a seam split there.
  - c2-impeller-loft 1d7fc3da (6.7 s) / 8d00ba15 (1.8 s): "a face's vertex off its plane", the upstream 1.14 mm vertex
    (core).
  - cf-setback-fillet 9d08366e (0.7 s, "a section touching a face's boundary"); small ones under 0.2 s.
- **Sheet metal's result check and glued fuses (lead 11-14 ~03:00: the flat-face check, then skasm's walls-and-bends
  fuse), 11-14 05:2x UTC, gating a9a785004 on vibe6 (vibe5 out of memory: earlyoom killed two gate builds):**
  - 53cc4779d: our result's planar faces bounded by lines are checked by `planar_lines_valid` (wires closed, no two
    segments crossing, one outer wire the face's way round, holes inside it, line ends on their vertices and in the
    plane) instead of BRepCheck's wire pairs; anything it can't say valid goes to the whole BRepCheck. Louver's slotted
    sheet check 12-19 -> 0.3 ms; c2-sm-louver 5.33 -> 5.06 s (live 4.99; vibe5 at load 45, best of 2).
  - 95bc10032: `NurbsSurface::grid` sums each u's poles once (ray patch boxes: model-plus's degree 14 x 1 wall 2.8 ->
    0.7 ms); `off_shared_edges` boxes fb's edges before projecting; the per-face checks on all threads.
  - a9a785004: a shell my edge-use rule can't pass (a cylinder's seam a plane face also uses: the louver's glued
    fuses) gets BRepCheck_Shell / BRepCheck_Solid alone, not the whole analyzer.
  - Glued fuses, warm, alternating on quiet vibe3: model-plus 59 / 55 / 46 / 6.3 -> 33 / 31 / 23.5 / 4.8 ms; corner-break
    82 / 113 / 94 / 81 -> 73 / 90 / 83 / 72; louver's 17 at 8.5 -> 6.2. Ours against BRepCheck on every corpus and
    recording (X_CHECKCMP scratch): 0 disagreements.
  - Left in corner-break's fuses: ~41 of 91 ms are OCCT sections of its rational ruled corner pieces (degree 1 x 2,
    four edges) against each other, the bends and the 68-edge plate, 0 or 1 lines each, 5-19 ms a pair. The pieces of
    neighbouring flanges are 20-28 um apart, inside the grey band (4 x their 10 um tolerance), so `free::apart`
    rightly asks OCCT. Taking them ourselves needs a near-touching ruled-surface test.
  - Then c77138414 (to gate): c2-fan-blade-sweep's blade cut by its two bolts (fb562f41, OCCT 1.1 s, "a face lost")
    is ours, 0.18 s, as OCCT's: each thread root's cylinder strip between two whole-turn helices covered the circle
    up to a rounding and `gap` found a gap of 0 (or one across the circle on the mirrored bolt). T1
    bool_patterns::bolts_with_whole_turn_thread_roots_cut_as_occt. Its 05cd5358 is taken too; 3bf5867d (35 ms, "an
    internal face") is left.
  - 867a4238b (gating, vibe6): `planar_valid` takes arcs too (chords at most r/1000 off, two edges' chords apart by
    the tolerance and their sags, a hole's point clear of the wires by as much; anything closer goes to BRepCheck).
    Louver's glued fuses' check 3.6 -> 1.3 ms, corner-break's 10 -> 4.5 ms (vibe3, alternating). Ours against
    BRepCheck over every corpus and recording: 11,417 results, 0 disagreements. T1
    bool_patterns::round_holes_and_slots_through_a_plate_as_occt.
  - Left on OCCT from my recordings: c2-impeller-loft 9522310b (2.1 s): an input vertex of 1.14 mm tolerance 0.4 mm
    off plane face 7 (upstream: core's), and with the 1 mm cap raised it declines "a face lost"; b4a6c1dc (1.5 s, "a
    piece shorter than its vertices' distance": the mirrored halves' fuse at x 0).
  - b4a6c1dc, looked at (11-14 07:5x UTC, lead): the halves' r 32.5 mm circle on the mirror plane is two arcs in one
    argument and a whole circle in the other. `edge_in_face`'s `past_face` trim finds the face edge it leaves across
    by the distance to each edge's curve, but takes the point clamped to the edge's range: an arc of that same
    circle is 0 from it anywhere, its clamped point its far end, so the circle's piece ended 1.6 um past v1 named v0
    (a 0.8 um piece from v1 to v0). Measuring to the clamped point makes ours take it in 0.35 s (OCCT 1.5 s), as
    OCCT's today: 33 faces, 94,807.060968 mm3 (the recording's OCCT volume, 0.0076 mm3 less, was an older OCCT path's).
    Gating; T1 bool_patterns::mirrored_halves_sharing_a_circle_fuse_as_occt.
- **asm-hand-drill-pinslot's Sweep 1 (lead 11-13 00:35, 03:30), 11-13 05:33 UTC, queued to land (e23bc68f; gate CHECK, knurls only):**
  the thread's inner face is a B-spline on the shaft's r 4.1 mm cylinder (12 nm inside). Ours can't take it (read as
  that cylinder it winds round three times; our cylinder faces cover one turn); OCCT's plain fuse cuts the shaft in
  three. A fuse that comes out invalid where an argument's B-spline face lies on another's cylinder (`as_cylinder`,
  rust_boolean.cpp) facing it from outside runs again with OCCT's GlueShift, kept when valid and the volumes add up (run
  up front it changed 4 parity56 fuses: glue keeps faces a plain fuse merges): one valid shaft, 4896.37 mm³ (live 4896.29). The studio: 21 -> 23 of 35 features, first
  difference Split 1 (21/29/10 against live's 20/28/10: OCCT splits the thread's exposed band of the cylinder into a
  face a turn, live's is one face). 13.1 s (Sweep 1 11.2 s: OCCT's glued fuse, as slow as main's plain one; live 28 ms).
  T2 bool_patterns::a_thread_on_its_shafts_cylinder_fuses_into_one_solid. The per-turn band split is only the raw
  fuse's: Sweep 1's own unify merges it (21 faces as live). Left in the studio: Split 1 (21/29/10, live 20/28/10), Chamfer
  1 and Fillet 1 errors, Sweep 2 failing (its helix half a turn off: the circle frame, to core above).
- **To core (11-13 03:48 UTC), two things the hand-drill showed:**
  - The kernel's `self_intersects()` (common.cpp, used by solid_a.rs's sweep check) now stops after a CPU second
    (run_budgeted) and then finds nothing: Sweep 2's check of a 20-turn helical slot ran for minutes, now 1.06 s (live
    138 ms). solid_a.rs is unchanged.
  - Sweep 2 reached that check because our Helix 2 starts half a turn from live's: it is a helix on the bore's circle
    (r 2.25 mm, Extrude 7's) with oppositeDirection, start angle 0. helix.fs keeps the circle's coordSystem x and
    flips only z; live's helix 2 midpoint is (+2.25, 0, -32.47) mm, so live's circle x is +X. Ours is -X:
    op_prism's `seat_circles` restarts every extruded circle at gp_Ax2's default x for the swept-back z (the cylinder
    face frame live "fs (10-26 b)" (a) has). Live's circle edge keeps the sketch circle's frame (Sketch 7 on the end
    cap, x +X, the circle's xDir +1) while its cylinder face takes the default. So a circle edge's frame and its
    cylinder's differ in live; ours ties them. Sketch 11's slot profile sits at +X, the tool at -X, and core's
    `helical_sweep_clear` (b5da62f83) then can't place the tool's corners on the profile. Not changed by me.
- **Completion (11-12 23:03 UTC, main 909e48460, all 98 suite documents recorded on vibe5, each distinct Boolean once a
  document; /root/bool2-scratch/tally.py, replay-docs.sh):** 928 Booleans, 656 ours (70.7 %), 44 more on our other
  shortcuts (on_face, through_holes), 228 OCCT; without cf-fill-pattern's 140 empty probes 656 / 788 (83.2 %). 48 of
  the 72 documents with Booleans all ours. 70 of 72 documents' Booleans faster ours than OCCT's (45.9 s against 81.1 s;
  not: cf-setback-fillet, cf-parameter-pattern's single on_face). With bool2-cov landed: +16 (rings 2, setback 5,
  sheet metal 8, knife 1) and the 140 probes (8139e7607, gating): 812 / 928 (87.5 %); setback and fill-pattern all ours.
- **Landed (11-13):** 770dac17 (sheets of shells; plane / round sections), 91856886 (cf-fill-pattern's probes),
  b908c662 (coincident sheets: the sheet metal's 42 coplanar fuses and commons). With them main's 227 OCCT Booleans of
  the suite are down to 29 by the replay (854 / 928 ours).
  Real documents, main vs branch alternating on vibe5: c2-rings-loft 9.8-15.6 s -> 4.0-4.3 s (live 1.6), same 22/32;
  cf-setback-fillet ~1.0 s both (live 1.09); c2-hunting-knife the same (1.8-2.0 s both).
- **Next:** the knife's 343b971e (514 ms): the two chamfer tools meet end to end (caps coincident at x -50 mm), their
  B-spline faces nearly tangent to the blade's; with near-on edges taken (X_NEARON) one section edge there has six
  pieces of faces 4, 21 and 25 (non-manifold). A classification problem; parked for the probes.
- **On now (lead 11-12 14:00 UTC):** Boolean coverage by real-document time: the documents whose OCCT Booleans cost
  the most seconds over live's time for the whole document, biggest first. Candidates: c2-impeller-loft (8 of 13 on
  OCCT), c2-sm-corner-break (23), c2-sm-louver (20), cf-sheet-metal-model-plus (7), cf-setback-fillet (5/5),
  cf-o-ring-generator (4). bool: you're on the sections-ahead speed group. I take declines (cases OCCT runs) and stay
  off our own time where we already take a case. If a decline I take is one you have WIP on, say so here.
- **c2-fan-blade-sweep's Sweep 1 (lead's priority 11-12 15:30; landed be39e2c4 11-12 17:33):** OCCT's section of the
  bolt's shaft cylinder (a whole one, its seam an edge) with each thread flank stopped at the last crossing of the
  seam, a turn short of the shaft's end. For flank 1 it left a curve of no length there; for flank 2 nothing. So both
  Booleans lost the overlap: OCCT's common came out empty and its fuse two solids. Ours declined "a non-manifold
  result". Now, when such a section has a curve ending on the seam inside the face that nothing continues, it is asked
  again of the free-form face and the cylinder made long enough to hold it (`sections_past_seam`, rust_boolean.cpp);
  the Boolean cuts the curves to the faces. Ours: fuse one solid 2,156.49 mm3 (live 2,156.42), overlap 26.20 mm3
  (live 26.27). The document goes 42 -> 44 of 53, Sweep 1 244 ms (live 58).
  - T1 bool_patterns::a_thread_sunk_into_a_shaft_fuses_as_live (the two STEP solids from core's repro).
  - **To core:** the next difference is feature 44, Mirror 3 (PART, NEW): its copy of the Bolt, Part 11, has
    25/60/38 faces/edges/vertices, live 16/33/20, while the Bolt itself matches live at features 42 and 43.
- **Coverage by document (11-12 14:40 UTC; model suite recorded per document on vibe4, main 54ffc204, 6 documents at a
  time; each distinct Boolean counted once).** OCCT seconds / ours / our regeneration / live:
  - c2-impeller-loft: 8.4 s on OCCT (10 Booleans), 1.0 s ours, regen 25.9 s, live 2.8 s.
  - c2-rings-loft: 1.8 s OCCT (2), 3.4 s ours (31), 11.6 s, live 1.6 s.
  - c2-hunting-knife: 1.6 s OCCT (2), 1.9 s ours (7), 3.8 s, live 0.4 s.
  - cf-setback-fillet: 0.7 s OCCT (5), regen 2.2 s, live 1.1 s.
  - c2-fan-blade-sweep 0.54 s, c2-extension-spring 0.53 s, c2-sm-corner-break 0.24 s (23, coplanar sheets),
    cf-sheet-metal-model-plus 0.22 s (7), c2-sm-louver 0.13 s (20), cf-o-ring-generator 0.03 s (4).
  - Across all 98 documents: 14.9 s on OCCT (230 Booleans), 51.3 s ours (652).
- **The impeller, case by case (exact recording, vibe4 ~/tmp/bool2/impy):**
  - **Fixed (landed 8c5d60fa1, 11-12 16:25):** the flange fuse 1556cc26 declined "an invalid result". The Shell's offset wall
    carries two edges inside it, apart from each other; in one wire they were BRepCheck's NotConnected. Now one wire per
    connected run. That late decline had also put the part studio's later fuses of the feature on OCCT (op_boolean's
    DeclineMemo: e9caded0, dc8a265f, 136cf961, 7de2dcc1, ~1.4 s); they're ours now, identical. T1
    bool_patterns::a_face_with_two_edges_inside_it_apart_fused_as_occt.
  - **Left, upstream of the Boolean:**
    - The 9-tool fuse and cut (70fd0254, be1d58fd: 3.9 + 1.4 s) get a target with vertices of 1.145 mm tolerance,
      0.4 mm off the plane z = -0.004. They come from a non-Boolean feature before them (Fillet 1 or Delete face 1:
      core / blend). With the 1 mm cap raised they then decline "an arc across a cylinder's seam" (bool's motor-mount
      one), so no gain yet.
    - 8b9436aa (2.4 s) is the Shell's inlet rim: a 0.24 m B-spline edge that lies on the flange's free-form face but
      doesn't project within 4 tol (bool: your Shell rim, JKF, up to 50 um off live's).
- **vibe4 removed (user, 11-12 19:55 UTC); everything moved to vibe5.** Until 522b51a6 lands, export
  NOSHAPE_LAND_QUEUE_HOST=vibe5 NOSHAPE_LAND_HOSTS="vibe5 vibe3" NOSHAPE_RB_HOST=vibe5 for rb.sh, land.sh and gate.sh.
  gate.sh's host is now NOSHAPE_GATE_HOST, else NOSHAPE_RB_HOST, else vibe5.
  - The corpora vibe5 lacked (valve2, snap, sp-cut, kall, k10-25, k10-25-12, gears) and mf2 / lcm were copied from
    vibe4 before it went; copies in /root/bool2-scratch/v4copy. My recordings (impy, hk, rings, es, fan, fan2) are in
    vibe5 ~/tmp/bool2.
  - Lost with vibe4: gate.sh's main-result cache and OCCT cache. Both are rebuilt on vibe5 by the first gate (or
    `gate.sh --main`). Gate run 12 (bool2-rings) died with it: rerun on vibe5.
  - **vibe5 has 29 GB.** A knurl's OCCT cut takes 35-50 GB a process, and vibe5 runs earlyoom. So on a host under 48 GB,
    gate-host.py leaves the knurl corpora (kall, k10-25, k10-25-12, k25-15, k25-25, k25-71-12) out and the verdict says
    CHECK: "the knurls not run". The timing rounds (ours alone, small) still run. Memory waits are a share of the host
    (30 %), never longer than 10 minutes.
  - **The kall nightly stays off** (kall needs 49.5 GB; vibe3 has 31). The hourly `--main` cron runs against vibe5.
- **To core / blend (11-12 22:40 UTC): c2-impeller-loft's 9-hole cut and 9-seat fuse (Remove Bohrung Flansch /
  Extrude Bohrungssitz Flansch, 0.27 + 1.0 s on OCCT) get a flange body with 9 vertices of 1.1-1.8 mm tolerance (one
  0.4 mm off the plane z = -4 mm). The tools are plain blind extrudes; the Booleans before (Boolean 3's) see nothing over
  0.1 mm. So one of features 35-38 makes them: Fillet 4, Move face 2, Delete face 2, Fillet Ansaugflansch zum Gehaeuse.
  Live has no such tolerances; ours declines them ("a face's vertex off its plane"). Recordings vibe5
  ~/tmp/bool2/bydoc-imp/rec/c2-impeller-loft (0ea6317b, 6add1c3f).
  → **core (11-13 00:03 UTC):** the op dumps' vertex tolerances in order (NOSHAPE_DUMP_OPS, main 11-12 23:40): up to
  Delete face 2's defeature input 0.0055 mm; its result 0.092 mm (OCCT's defeaturing); the 9 vertices over 0.1 mm (1.14 mm,
  then 1.76 mm in the next op) come out of the fillet after it (fillet-33 -> unify-34: Fillet Ansaugflansch zum Gehaeuse,
  feature 38), so **blend's**. Delete face 2's 0.09 mm is mine to look at once the impeller's part matches live there.
- **To bool (11-12 22:37 UTC): your plane_tube_sections (78ef7ff85) missed sections in c2-rings-loft, gating a fix in
  it (branch bool2-cov 27205c4eb).** The slot tools' planes lie nearly along the ring's rounds' arcs (meridian
  slabs), so a section crosses a 68 mm round within one of its 64 samples, or caps a fold between two, or a thin end
  face clips it to 0.3 mm: the samples found no run and it said "no section" (34 such pairs; OCCT has one each). Our
  Boolean then declined "a non-manifold result" on the 51-slot cut (5.1-5.5 s on OCCT; the 14:40 table had no such
  OCCT cut in the rings, so likely since 78ef7ff85) and on a 3-slot cut (0.4-0.6 s). The fix: samples four across the
  plane face's width (area over box diagonal), up to 4096; a pair is OCCT's where a side of the round is crossed in a
  step whose roots in range are as many at both ends, where a side crossing inside the plane face's box ends no run,
  or where a step has a fold whose two roots span part of the range. Both cuts ours: 316 ms against 3.4 s, 42 against
  350, equivalent. c2-tpd-fillet-variable's cut 80 ms against main's 95 (ours only). T1
  bool_patterns::slots_across_a_rings_rounds_cut_as_occt. Say here if you'd rather shape it otherwise.
- **Sheets of shells (11-12 21:45 UTC, branch bool2-cov 32a0b112f, gating on vibe5):** ours now builds a sheet
  result's shells as OCCT's BOP does (BOPAlgo_BOP::BuildShape: each input shell's pieces in connected groups, a shell
  each, OCCT's own MakeConnexityBlocks / OrientFacesOnShell), so "a sheet of shells" is gone ("a sheet of wires" stays).
  cf-setback-fillet's five 25+1-face fuses then declined "an invalid result": the corner patch's edges were replaced by
  the fillet's arcs, their curves on the patch taken from its own coincident edges, which lie on it only within their
  tolerance (40 um). Such an edge's tolerance (and its new vertices') is now raised to its measured distance, as OCCT's
  CorrectTolerances (`fit_tolerance`, after coincident_pcurve / parallel_pcurve / projections). The 19 corpus cases
  that declined "a sheet of shells": 17 taken, 0 differ, 3 equivalent (volume 5e-9 relative, 0 records apart); the
  replay now also compares a sheet's shells with OCCT's. Setback's fuses 14-18 ms against OCCT's 28-37. T1
  bool_patterns::sheets_of_shells_fused_into_shells_as_occt.
- **Coverage on vibe5 (11-12 21:35 UTC, main 909e4846, recorded per document; OCCT ms / regen / live):**
  cf-setback-fillet 284 (5) / 907 / 1089; c2-hunting-knife 551 (2) / 1437 / 408; c2-fan-blade-sweep 724 (3) / 1281 /
  1418; c2-extension-spring 179 (1); c2-sm-corner-break 103 (23) / 2591 / 1972; cf-sheet-metal-model-plus 55 (7) / 1126
  / 917; c2-sm-louver 52 (20) / 5897 / 4987; cf-o-ring-generator 21 (4) / 63 / 65. Impeller and rings being re-recorded.
- **Restart state (11-12 21:45 UTC):**
  - **Worktrees:** /root/projects/noshape-wt/bool2 (branch bool2-cov). bool2-gate and bool-gate-base are gate.sh's.
    bool2-nightly is the crons' checkout. bool2-doc is for docs-only lands. Targets on vibe5: bool2, bool2-gate,
    bool-gate-base.
  - **Host: vibe5 again (lead 11-14 ~03:00 UTC: vibe6 at load 56; NOSHAPE_RB_HOST / NOSHAPE_GATE_HOST=vibe5). vibe6 from 11-13
    13:20 (knurls out there too).** vibe6 got the corpora, the gate's bin and cache and my recordings (~/agents/bool2-keep) at 13:25,
    copied through this machine (vibe5 can't reach vibe6 by name); its OCCT cache starts empty. vibe7 has the corpora and the gate's caches (copied 05:55 from vibe5's
    ~/agents/bool-corpora and ~/agents/bool-gate); vibe5 can't reach vibe7 by name, so copies go through this machine.
  - **Session cron, gone with the session; recreate it:** every hour at :17: `git -C
    /root/projects/noshape-wt/bool2-nightly fetch -q origin && git -C /root/projects/noshape-wt/bool2-nightly checkout
    -q -f --detach origin/main && bash /root/projects/noshape-wt/bool2-nightly/tools/bool/gate.sh --main`, skipped while
    another gate runs. The kall nightly stays off (vibe5 has 29 GB).
  - **Recordings (exact) on vibe5 ~/tmp/bool2/:** bydoc/rec/<document> (the 8 above), bydoc-imp (impeller, rings), sos
    (the sheet-of-shells cases); ops-main is main's ops test binary that recorded them. vibe4's (impy, hk, es, bydoc)
    went with it. /root/bool2-scratch/bool-by-doc2.py (DOCS=..., JOBS=...) makes the per-document table.
- **To bool (11-13 05:42 UTC): c2-fan-blade-sweep's fb562f41 (a 2-tool cut, 386-546 ms on OCCT), your seam area:**
  its tools have one-turn helical strips on r 3 mm cylinders (each helix exactly 2π from its rulings' angle). `gap()`
  found a gap of width 0 there and put both rulings at u = 0, so the strips' outlines didn't close: "a face lost". With
  `gap` returning None under 1e-9 rad (/root/bool2-scratch/fan-gap-fix.patch, one line, not landed: it takes nothing
  alone) the faces go round as threads should and the cut stops at "an arc across a cylinder's seam" (your
  motor-mount one). Recording: vibe5 ~/tmp/bool2/bydoc-all/rec/c2-fan-blade-sweep.
  The suite's two other "a face lost" (11-13 06:34 UTC) are the same family, cylinder faces going exactly once round:
  clamp-hms 1309375f (a fuse, 13 ms) has its seam line at u -0.078 on one side and 0.1215 on the other (r 31.75 mm,
  one turn apart), so the outline doesn't close; cf-lighten a28978f4 (a 4-tool cut, 40 ms) loses tool face 26 likewise.
  The gap fix doesn't help them. Cases: vibe5 ~/tmp/bool2/lost.
- **Landed 09:56 UTC as 82cfec7d (4 commits; first land try 30032d61 failed on bool's new parts_of_one_torus test: a past point must be off the face's whole surface too; regated CHECK, knurls only):**
  - 5c810e4fc the knife (below). Its first gate (2a78dd7f, vibe7) FAILED: 4 new differences, 47 cases declined, 6
    crate tests, clamp-hms worse. Two bugs, fixed: the "section along an edge" test measured distance to the edge's
    infinite line (seg_param), so sections running on straight past an edge's end were dropped; now within its range,
    and for B-spline sections only (exact ones don't stray: two shfall slab fuses were taken and differed). And
    "past the face's edge" counted points past it inside an edge (a circle on a torus leaving one face and coming back)
    or with one point on; now only at its ends, three points or more on, and asked only of points clearly off (it
    had cost 9 % of the rings' wedge cut). All bool_patterns pass; the gate's 4 cases as main.
  - c0a44f936 cf-setback-fillet's last two fuses: OCCT's section passes 0.5-4 um from the face's edge, which lies on
    the other face's surface all along (OCCT: edge on face, no vertex), so not "a section touching a face's
    boundary". An edge touching at one point (bool-corpus 890acd4d, 0.1 nm) still declines: OCCT puts a vertex there.
    All 5 of its fuses ours, as OCCT's, 1.1-1.5x. T1 a_section_beside_an_edge_lying_on_the_other_face_fused_as_occt.
  - d511a9ca5 sheet metal: a sheet cut by flat sheets on one of its planes, its other faces' planes past the tools'
    reach: corner-break 1, louver 2, as OCCT's. T1 a_sheet_cut_by_a_sheet_on_one_of_its_faces_as_occt.
  - b0cec4ed7 **c2-rings-loft's wedges (user 07:20, blend: OCCT's Boolean faster there).** The wedge cut 78cd8309 (137
    faces, 2 tools; Chamfer 1's) took 200-225 ms ours against OCCT's 90: two thirds of it was apart_across_slab's
    exact face boxes, BRepBndLib::AddOptimal's search on torus faces (PSO + Extrema against six planes). A torus
    face's box is now worked out in closed form over its UV bounds (torus_box; within 1.3e-10 of OCCT's on the inside,
    up to 0.19 mm larger where a face is trimmed off its UV rectangle: only more cautious). Ours 66-85 ms against
    OCCT's 78-118 on the same host, same result; every rings-loft Boolean now ours at OCCT's time or less (all 99:
    ~1.8 s ours against ~7.4 s). T1 a_rings_wedge_cut_through_its_tori_as_occt. The saving (~150 ms) is blend's
    Chamfer 1 gap (534 -> 380). **To blend (08:10 UTC, from your /root/blend-scratch/rings-prof*.log):** Chamfer 1's 5
    Booleans were 314-362 ms ours against OCCT's 225-257; their 214 ms call is this cut, now ~70: ~210 ours. Fillet
    1's 4 Booleans are 30 ms ours against 24; its 223 -> 127 is op_blend_direct (160 -> 78 ms), yours, not the Boolean.
- **c2-hunting-knife after 8e04e97c (14:02 UTC, vibe6 at load 13, kernel profile):** 3298 ms, live 408. Fillet 3 1306 ms
  (op_loft 803 x6, blend.direct 407: blend's), Chamfer 1 732 (its cut 349: 225 of it OCCT's IntTools_FaceFace of the
  lofted tools' faces with the blade's in sections_ahead, bool's; ours 268-380 ms against OCCT's whole Boolean 788-991),
  Fillet 2 364 (Boolean 87), Mirror 1 349 (op_unify 259: bool's row), Chamfer 2 172 (Booleans 74). Boolean time left
  ~600 ms, mostly OCCT's sections; the rest of the gap is blend's.
- **c2-impeller-loft's Boolean Flansch+Gehäuse (user 12:00 UTC; landed 14:18 as df95cac8):** the flange's inner
  wall is extruded from the shell's inner wall's top edge; where the shell's wall slopes away under it the flange's
  wall stands in the cavity (0.04-0.27 mm proud at x 30-65 mm each side, 3 mm at the far tip). We declined (the
  flange's top edge, the same curve as the shell's own edge e15, is 2.3 um off the wall: that edge is off its own face
  by that much, tolerance 1e-7, upstream) and OCCT's fuse lost the strip (16 raw faces, 26.6 mm³ short). Now:
  `twin_pcurve` (an edge that is one of the face's own edges takes that edge's curve on it), `off_shared_edges`
  (rust_boolean.cpp: OCCT's walking lines run along the shared edge, then turn off down the walls' crossing: the
  stretch along it dropped, each turn-off a trimmed section from its back-extrapolated meeting with the edge), and a
  section end on an edge of one face lying on the other within the section's tolerance gets a vertex on that edge
  reaching it. Ours 59,138.42 mm³ (live 59,137.89), slivers 6.75 / 6.82 mm² (live 6.8 / 6.7), flange wall pieces
  135.66 / 136.74 (live 135.6 / 134.3), inner wall 13,216.16 (live 13,219.1). Feature 15: 19/51/34, live 19/56/39
  (faces and volume as live; the 5 edges / vertices asked of live, REQUESTS 12:35). T1
  bool_patterns::a_flange_drawn_from_a_shells_wall_fused_as_live. Kernel 404/404.
  - **Before / after (vibe6, main 82a88125 vs df95cac8, two alternating rounds, load 25-40):** features 15-23 had faces
    and volume off (15/41/30 at 15, 26.6 mm³ short), now faces, volume and area as live (5 edges / vertices short);
    feature 25 volume 71,468.31 -> 71,495.45 (live 71,495.17). From feature 24 on the same failures (Chamfer, Fillet
    3, Fillet zum Halter, Move face 1; Boolean 3 9,000 mm³ short). The suite's count stays 12/48 (first difference
    feature 12, upstream). Times: Boolean Flansch+Gehäuse 2964/2461 -> 1479/1605 ms (live 154), Boolean 3 347/204 ->
    168/156 (live 94), Fillet Stevenrohr einnschub 640/612 -> 65/70; Move face 1 ~6 s and Delete face 2 ~3-4 s either
    way (core's defeaturing); the document 26-32 s either way under that load (live 2.8).
  - **Live's feature 15 (REQUESTS 2985, live/bool/q2985/out.json; read 00:55 UTC):** 19/56/39, ours 19/51/34. Of live's 39
    vertices ours lacks: JJM / JJU (0, ±23.55, -56.40), between the input's three circle arcs at x 0, which our unify
    joins into one edge (live keeps a body's own vertices where the Boolean changed nothing; the feature's unify runs
    after op_boolean (recorded unify 0), so keeping them needs a mark on untouched vertices that unify honours: not done,
    feature 15 would still be 53/36 against 56/39); and live's turn-off edges are short verticals stopping part-way (x 29.409: 0.69 mm; 61.988: 2.31;
    63.503: 0.35), ours run to the flange's bottom at x 30.08 / 63.86 / 64.34: a different crossing of the walls,
    upstream (the flange 0.07 mm³ and the Shell 7 mm³ off live).
  - **To core:** the next difference downstream is yours: feature 24 Extrude 4's Part 3 5/9/6, live 5/10/7, 0.009 mm³
    off (up to the inner wall); feature 25 Boolean 1 is then 32/81/50 (live 30/83/54) with OCCT's result identical to
    ours on its inputs (a1af84e6, 3998eb43), then Chamfer Stevenrohr Halterung fails. Recording of the whole document
    with this branch: vibe5 ~/agents/bool2-keep/rec-imp2; per-feature diff ~/agents/bool2-keep/imp-diff.log.
- **Our own sections: SHELVED (lead 11-14 00:45 UTC: slower than OCCT's sections (helical sweep 131 vs 83 ms), corner cases open-ended). Branch bool2-march kept. What it would need: sections ending where OCCT's walks end at faces' corners (vertices where two faces of one argument meet on the other: c2-rings-loft 78cd8309 (OCCT's 2 at y 0), bd8a6b96 (live JwQ: OCCT's one)), a curve form OCCT's 2D intersections take whole (C2), and the touch probe not sending most pairs back to OCCT. (Was: lead 15:40 UTC;:** 936 distinct Booleans, 25 on OCCT (97.3 % ours or our
  shortcuts); the replay declines 22. Left: c2-impeller-loft 4 (7519a27a 2.5 s, 9522310b 2.1 s, b4a6c1dc 1.5 s,
  84f92d10 0.6 s; upstream tolerances, blend / core), c2-fan-blade-sweep 2 (fb562f41, 05cd5358: helical strips on
  whole cylinders, bool's seam area; /root/bool2-scratch/fan-wrap-gap.patch), "a shared shape OCCT may keep or remake"
  8 (o-ring 4, impeller 2, hex-bookcase, duct: OCCT's rounding decides, deliberate), single others. The dovetail /
  shaped-flange "non-manifold" ones of the 07:00 list were the knife commit's bug, gone with its fix.
  Our time against OCCT's on the 860 taken (same run, alternating per case): 10.7 s against 42.7 s. Slower than OCCT
  by over 10 % and 3 ms, six: **cf-snap-hook 45eceb50 131 ms against 51 (bool: your snap hooks' row)**, sheet metal
  0067e217 (model-plus) 14 / 3.5, 446d711a (louver) 13 / 3.6, f90a60d9 (corner-break) 22 / 17, 271bc46c 12 / 8,
  dfe6d971 5 / 2.
  - **Why the louver's 446d711a is slow (08:55 UTC):** our Boolean 1.6 ms, the result's BRepCheck_Analyzer 12 ms (thread
    CPU, measured; the sampler misses it): 3 planar faces with 276 line edges (a sheet with its slot holes), the
    checker's wire-against-wire tests. OCCT's own path doesn't check (5 ms all told). Not changed: the check is the
    guard against invalid results of ours (fuzz seed 1144's crashes). A cheaper equivalent for planar faces with line
    edges only (or the wire pairs by boxes) would take 10 ms off each such sheet-metal Boolean: lead, worth it?
- **The knife's 343b971e (11-13 05:33 UTC, branch bool2-knife2):** a cut by two chamfer tools
  end to end along the blade, their B-spline faces all but tangent to the grind face 4. Tool 0's edge 52 lies on face 4
  within 0.2 um but runs on past the blade's tip (0.2 mm off the surface there); OCCT's sections of tool 0's faces
  with face 4 come in pieces with gaps (open ends, not taken). Now: an edge within the grey band all along is on the face
  (was "an edge nearly on another argument's face"); a sample past the face's own edge (`past_face`: the surface's
  nearest point outside the face or on its boundary) counts neither way, and edge_in_face cuts such an edge where it
  leaves the face, a vertex on the face's edge there; a section between two faces that runs along an edge of one lying
  on the other (within 1000 tolerances, 0.1 mm: nearly tangent sections stray 1-40 um off it) is that edge. Taken:
  faces and volume as OCCT's (5e-8), two vertices fewer (OCCT's section pieces' ends); the document's Chamfer 1 matches
  live either way. c2-hunting-knife 1.76 -> 1.24 s (Chamfer 1 907 -> 590 ms; live 0.41 s). T1
  bool_patterns::a_chamfer_tool_along_a_blades_face_cuts_as_occt.
- **c2-extension-spring's 84884726 (0.23-0.53 s):** "a short section from a corner" is bool's deliberate decline.
  OCCT's builder takes a 10-100 tol stub from a corner to a branch point into the vertex; ours doesn't model that.
  Recording vibe4 ~/tmp/bool2/es.
- **c2-fan-blade-sweep after the thread fix (recorded vibe4 ~/tmp/bool2/fan2):** 07831a00 is the thread fuse itself, ours
  = live, OCCT different (not in any corpus). Left on OCCT:
  - 05cd5358 (Revolve 1's cut of the threaded Bolt, 205 ms, "a section without ends"): the shaft cylinder is now a
    helical strip (face 14). A coaxial cone's circle crosses it at x -51.75 mm and runs on across the circle's own
    start angle, the strip having no edge there at that height. Its interval ends Open at t1 on both faces and would
    have to join the first one across the wrap; neither intervals_on nor section_from does that. WIP (the open-end
    probe, not the join): /root/bool2-scratch/fan-revolve-wrap-wip.patch.
  - fb562f41 (a 2-tool cut, 891 ms, "a face lost"): not looked at.
  - 3bf5867d (26 ms, "an internal face").
- **After those:** cf-setback-fillet's five sheet fuses ("a sheet
  of shells"), the sheet-metal documents' "coplanar sheets" (many, under 15 ms each).
- **Done before:** (1) a fast gate, `tools/bool/gate.sh <branch>` (below); (2) spheres and tori in our Boolean:
  learning-center-michael's ring cut (lcm 41701bb2) and Fillet 2's ball / torus cut (f71e381d, vibe4
  ~/agents/bool-corpora/mf2), which `fillet_by_tools` (features.cpp) sends straight to OCCT (`others`). cf-surface-pattern's
  Surface Pattern 3 is bool's (c7c9d0ff).
- **gate.sh** runs bool's gate as one command and prints one verdict line: VERDICT PASS / CHECK / FAIL. It covers every
  corpus in whyall.sh plus shfall, kg.sh's knurls and gears, and the model suite. It also times k25-15, k25-25 and
  cf-spur-gear on our side only (`NOSHAPE_BREP_OURS_ONLY`), main's binary and the branch's alternating, best of 3.
  - Each distinct case is replayed once: the corpora hold 5,327 case files but 2,896 distinct cases (same inputs, op
    and expected result).
  - The cases run in shards (directories of links) over a job pool. The heavy corpora (shfall, knurls, gears: up to
    16 GB a process) get a pool of two; any shard starts only with memory free. Eight at once were OOM-killed.
  - OCCT's results and the facts the replay reads of them (records, validity, volume, unify) are kept by case
    (`NOSHAPE_BREP_OCCT_CACHE`), keyed by what OCCT's path is built from: crates/kernel's src and cpp, brep's
    mesh / nurbs / lib.rs, the lock file, the patches, /opt/occt. A crates/brep branch pays OCCT once. k25-15's
    replay goes 88 -> 37 s.
  - Main's results are cached on the host (~/agents/bool-gate/cache). The key is the git trees the test binary is
    built from, plus the corpora, OCCT's libraries and the models. A branch's results are cached too: once it lands,
    they serve as main's.
  - Host side: tools/bool/gate-host.py. Full log: /root/<caller>-scratch/gate-<sha>.log. One gate per host at a time.
- **Measured (vibe4, load 25-100):** the old serial gate is about 2.5 h: corpora + parity56 13 min x2 (branch and
  main), kg + kt about 35 min x2, the model suite 30 min x2.
  - First sharded runs, OCCT cache cold, both sides at once: the light shards (about 170) take 10 min.
  - The knurl shards are OCCT's cuts: k25-25's single case 918 s, kall shards 1,100-1,600 s. One process reached
    35-40 GB, past the 16 GB budget, and vibe4's earlyoom SIGTERMed it. So the knurls run one at a time with 36 GB free,
    and a killed shard reruns alone.
  - The sharded model suite takes 601 s (serial 30 min). Per-document times are kept for balancing next time
    (~/agents/bool-gate/suite-times.json).
  - Once warm (the OCCT cache for main's key filled, main's results cached), a crates/brep branch replays only its own
    side, without OCCT.
- **Warm gate (11-12 12:44, vibe4, a crates/brep-only change, main 54ffc2047 cached): 40 min, PASS.**
  - Build of the kernel and ops tests: 12.6 min. Crate tests: 11 min, now beside the replay, so about 29 min.
  - The branch's replay and suite: 11.5 min. The suite's floor is c2-impeller-loft, about 11 min alone.
  - Timing: 4.5 min.
  - Main cold too (the first gate after main's kernel moves): 61 min.
  - `gate.sh --main` (hourly, bool2's session cron) works out main's side ahead of the next gate.
  - Timing under vibe4's load: main's own rounds 11-85 % apart. Slower beyond that spread is CHECK, so smaller
    regressions show only as a hint.
- **Host:** vibe4 sits at load 25-100 from the land queue's slots, psui, blend and dx suites and bool's replays, so
  16 gate threads get a fraction of 16 cores. Proposal to the lead: a gate lane on vibe4 (rb.sh pins other jobs to cores
  0-9, gates to 10-15), or gates on vibe5 at night.
- **kall (lead 11-12):** out of the per-change gate. `gate.sh --nightly-kall` replays it on main alone, one case at a
  time, against the night before (bool2's session cron, 04:30 UTC). Verdict line: /tmp/lead/kall-status; a FAIL is
  also appended to /root/bool-scratch/KALL-FAIL, so bool sees it. kall 324c79d1 took 49.5 GB alone on vibe4 and
  earlyoom ended it: until vibe4 has that free, it is reported, not checked.
- **lcm: landed c054329b (11-12 09:41).** OCCT's section of a round and a ring, both tangent to the top plane at a
  corner, crosses the round's straight edge at about 50 degrees 23 um before the corner, runs 8 um out and hooks back
  into the corner (OCCT's own pcurve on the round too). OCCT puts no vertex there, the edge only touching the ring. Ours:
  - no crossing there;
  - the section's 2D curve on that face starts at the crossing, so it leaves the corner inside;
  - a curve's start merged into a crossing within its tolerance (OCCT's pave);
  - a made cylinder face may pass its input's angles by 1000 tolerances.
  Cut taken, identical to OCCT: the app's Boolean 0.9-1.2 s (our decline + OCCT) -> 0.6 s. Gate run 6: CHECK only for
  the 4 knurl cases neither side could finish for memory; model suite identical (77/98, 817/935). T1
  bool_patterns::a_section_hooking_over_an_edge_into_a_corner_cut_as_occt.
- **Fillet 2 (branch bool2-seam, gating):** bool's b4c45d3f2 (an open edge's end across a free-form face's seam takes
  its own parameter; a section's touch near a face's vertex is that vertex) takes the ball / ring cut and fixed shfall
  29f3147a. My "nearest turn" rule is dropped, and 29f3147a needs nothing more: shfall is 58 taken, its known 2 differ.
  What lands: `fillet_by_tools` asks our Boolean first for the balls' and rings' cut (143 ms, OCCT's 406), T1
  bool_patterns::a_fillets_ball_and_ring_cut_as_occt.
- Don't touch: bool's seam-end / touching work (bool-main2, bool-touch); Shell (by_quadrics).
