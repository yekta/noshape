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
- 11:15: gate71 VERDICT CHECK only for the knurls (vibex7 31 GB; replay 0 new differences, suite 90/90 and 1283
  features, crate tests pass, speed 1.02-1.05x); udoc12 13 tabs SAME; knurls ours-only main vs branch (unit bool2-kn71,
  vibex7 ~/tmp/bool2/kn, outside the slot: core's pair7 holds it); land unit bool2-landxs3 waits for it.
- 09:09 (lead): gate71 moved to vibex7 (main cache hit there), hats/dock as udoc12 on vibex7 (udoc-x7.sh: cli-base
  copied from vibe2, cli-br built in vibex7's slot, private docs mode-700 copies deleted on exit); landxs2 re-armed on
  gate71 + udoc12. vibex8/vibe2 entries cancelled (no tickets, vibe2 copies deleted).
### Fan-blade fix: branch bool2-fanb2 = ebc58e0be (on c9c3759aa), worktree bool2-ab
- c2-fan-blade-sweep Boolean 2 (3e8c1e09) ours: see Gate exceptions. Gate it after the stack lands (rebase), vibex8.
  Then tell bool (narrows offset_grazes_a_circle) and core (nuts3 lands after it). Rebase on bool's bool-shared2 when it lands.
- Nuts (bool's hand-off, core's Sweep 1): ebc58e0be already takes bool's recorded nutcrest cut (135.2676 mm3, live
  135.268; OCCT 143.88 removes nothing); in the document (ops-fanb2) Sweep 1 ERROR -> OK with live's counts (one
  cylinder face is a B-spline: core's input). core's nuts3 T1 (59df02227) passes with and without the fix (vibe2):
  without, ours declines and OCCT cuts (570 ms); with ebc58e0be ours (105 ms). Told core 08:52 (old thread) and again its new thread e8af89fe (nuts3 needn't wait); tell core d30002ca when ebc58e0be lands.
### Branch bool2-spd = 37e33a5a5 on main 125e4d410 (worktree bool2-k25): speed, without the fan fix
- 5735138f5 exact drop_same_regions; a7635126a section ends' corner/crossing checks (Ctx::on_an_edge: face box,
  B-spline pieces' boxes (NCurve.boxes), per-vertex memo; corner's common-vertex lookup by distance first);
  a47578116 same_surface off_patches (not torus/sphere); 37e33a5a5 offset_as_bspline fits cached (64, by surface +
  face uv bounds): impeller Boolean 2 234/168 -> 90/143 ms in the document (vs landed build, load 30: a hint).
- k25-25 sampler: section_from closure 5.7-6.2 % -> 1.8-2.0 % (main ~0.6 %), same_surface ~3 % -> 0.6 %.
- Checks: crate tests pass (spdt.out); udoc14 13 tabs SAME. gate73 moved 17:14 from vibex8 (its builds sat 80 min in
  the slot queue behind qa/asm/dx) to vibex7: unit bool2-gate73b, gate73.out. Then knurls ours-only (kn.sh style) and
  no A/B (DECISIONS 9e5b481d4, 11-18 18:15: our Boolean lands on correctness, identical results on the corpora and
  hats/dock; timing blocks only at >= 20 % and >= 100 ms slower per document, A/B-confirmed). Unit bool2-post73b waits
  for the verdict, copies the gate's binaries and runs the knurls ours-only (vibex7 kn73.out); then land bool2-spd
  from bool2-k25 and tell the lead.
- 19:11 gate73 VERDICT CHECK (knurls left out): corpora 3026 -> 3026 taken, 6 -> 6 differing, 0 new; crate tests
  pass; knurls ours-only main = branch (5 corpora); BUT cf-surface-pattern Surface Pattern 3 129/386/259 -> 129/385/258
  (live 131/387/258), reproduced twice: HELD. Bisecting (bis73.sh <sha>: ops binary on vibex8, the doc on vibex7);
  5735138f5 first (unify drop). udoc15 (fan5 vs main) 13 tabs SAME, dock tab A too.
- 20:45 bisect: 5735138f5 alone gives 129/385/258. Landed without it: bool2-spd2 (bool2-k25) = a7635126a, a47578116,
  37e33a5a5 cherry-picked on 125e4d410, QUEUED f9a71c81 20:50 (land.sh --status). 5735138f5 parked: why its exact drop
  changes Surface Pattern 3's unify (one edge, one vertex fewer; live 131/387/258). f9a71c81 failed 20:53 (batch
  rebase conflict in docs/parity/interaction-feel.md, not mine); 9d59ccc4 failed the same way (21:02): branch was on
  125e4d410; rebased on origin/main, re-queued 21:18 as 7843743e (merge-base diff: my 3 files only).
### Seam-line fix: branch bool2-seam2 (on bool2-spd 37e33a5a5, worktree bool2-ab, uncommitted at 17:40)
- Root cause of the dock DIFF (cut a8b5ac0b7503a7e7, AppleWatchDock tab A; recording vibex7 ~/tmp/bool2/udoc/rec,
  private: delete when the dock work is done): a full-turn face (Surf::Cyl full) whose seam's line is in pieces
  (a slot's side along it, used once): bound() put such a piece at th0 (copy 0) though it belongs at th0 + 2 pi, so
  the loop was open on the far side; inside_with's first ray (0.371 rad) from the section arc 1.8 mm below the face
  crossed 3 edges -> "inside" -> an extra edge (ours 589/362 vs 588/361).
- Fix (boolean.rs, after the full: false block in the face-box loop): those pieces whose loop runs up (along v, with
  the face's flip, as round()'s thread rulings) go to `right` (copy 1). The occurrence order of the seam's two uses
  doesn't tell the side (tried first: right for the dock, wrong for the T1's end-side notch).
- T1 bool_patterns::a_cone_over_a_bore_notched_at_its_seam_cuts_as_occt: a cone wedge (1.354 rad up to the seam's
  angle) over a slotted bore, 1 mm above its wall; without the fix "a section without ends" (path occt), with it ours.
  The dock replay with the fix: 9 of 10 taken, 0 differ (kp-seam4/kp-seam5).
- Committed bba45965f (bool2-seam2: bool_ 140, brep 62, clippy clean, dock replay 0 differ, the dock cut declined as on
  main). bool2-fan5 = b4faa1052 = fan fix a2b90f571 on it: bool_ 141, dock 9 of 10 taken, 0 differ. hats/dock udoc15
  (vibex7, unit bool2-udoc15). Next: rebase on main once bool2-spd lands, gate, knurls; land; then tell bool (narrows
  offset_grazes_a_circle) and core d30002ca (nuts).

## Moved 11-19 01:40
### P0 cf-spur-gear cold +100 ms in v0.4.59 (lead 20:40; studios' Mac profile: per-face sample grids cold)
- Likely my fc3754783: free::cached_grid ran folds() (a radius query per sample) on every new grid; only uv_of's rare
  retry reads it. Branch bool2-folds (bool2-ab, from origin/main, uncommitted 20:55): Grid::folds() lazy (OnceLock).
  Unit bool2-folds (folds.out): bool_/brep/clippy, k-main vs kp-folds cf-spur-gear replay ours-only 5 rounds alternating,
  vs OCCT. 21:00: bool_ 139, brep 63, clippy clean, 14/14 cf-spur-gear cases 0 differ; replay (ours only, whole
  corpus per process) medians k-main 11116 ms vs kp-folds 10959: noise, not shown. Not landed; wait for qa's bisect.
  bool suspects 2c33ec558's uv_first grid/uvs lock.
### Line commons: branch bool2-line = ca1457fee (on origin/main 21:25, worktree bool2-ab)
- line_common crosses B-spline edges (64 samples, golden-section refine) and faces (free::ray_hits, turns on periodic
  faces); declines a line along / grazing a free-form face. bool-corpus 865 -> 881 taken, 0 differ (all 16 "a line and
  a curved edge"). T1 bool_patterns::a_line_through_a_free_form_solid_is_cut_as_occt (recorded 091cf0f95b46fa8a).
- bool2-next (bool2-ab) = ca1457fee + d70d9b0dd (seam) + 314fdcf33 (fan), bool_ 142/142. After 7843743e lands
  (background wait on land.sh --status): rebase on main, gate (vibex7 or vibex8), hats/dock, knurls ours-only; land.
  Then tell bool (narrows offset_grazes_a_circle) and core d30002ca (nuts).
### "a cylinder face without curved edges" (3 bool-corpus: 8658a028, 76c804e8, 34b7e564): a partial cylinder face
  bounded by one closed B-spline round a patch (net turn 0, so `covered` empty, no gap). Fix uncommitted on bool2-next
  (bool2-ab): a curve covers every angle it passes (min..max of the unwrapped angle), same as before for monotonic
  curves. bool-corpus 881 -> 884 taken, parity56 / corpus2 unchanged, 0 differ; bool_ 142, clippy clean. Committed on
  branch bool2-cyl (on bool2-next) with T1 a_patch_of_a_cylinder_in_one_closed_curve_fuses_as_occt (34b7e564). Gate it
  after gate74 (or with the next).


- qa 11-17: in the clean A/B (vibe7, load at most 0.06, v0.4.48 to v0.4.53, median of 3), c2-helical-sweep Sweep 1 took 157 ms on 0.4.48 and 396 ms on 0.4.53 (Ok in both). Your 895c8e3b / e47268f2 are meant to fix this on main. The 11-18 nightly pair (v0.4.55 vs v0.4.56, alternating) will confirm or reopen it.

## Moved 11-19 02:05
## Model-suite speed list (lead 11-17): where each stands
- c2-loft-paths-guides Mirror 1: input differs (Loft 3 17 vs live 15 faces, core); Mirror 1 Ok but wrong (13/33/25,
  5.27e6 mm^3 vs live 27/67/47, 6.33e6): OCCT's fuse with the mirror (2.75 s; ours declines "an edge without its curve
  on a free-form face"). Told core. Case (were on vibe5, lost: record again) ~/agents/bool2-keep/lp-rec/61b92e93c065aeed.
- c2-parting-line-draft-parting-line Split 3: 1829 -> 599 ms on main (core's 85010cc40) but still "does not split"
  (live 13/34/24 x 2): the tool runs through the body's parting edges, which our drafted B-spline walls put a few um
  off it (body 25 mm^3 under live's). OCCT's Splitter with fuzzy 1e-5 splits (13/49/39 + 17/53/39). Draft is core's.
  Dumps (were vibe5 ~/tmp/bool2/spd, lost) (split-2-*), probes probe_split_pair / probe_split_ab / probe_split_fuzzy.
- c2-surf-workshop-knifehandle Split 1: 971 -> 98 ms on main (core's fix), live 7; OCCT's Splitter ~50 ms of it. A
  half-space split by our Boolean (extrusion surface swept across) isn't faster: shelved,
  /root/bool2-scratch/split-extrusion-halfspace.patch.
- c2-config-parts-sprockets: Extrude 2's and Circular pattern 1's cuts ours on bool2-c26 (0ffab0d52, bd9710453).
  Cases (were on vibe5, lost: record again) ~/agents/bool2-keep/spr-rec.
- c2-config-parts-nuts Sweep 1: bool's (nuts thread turn); c2-sm-rocker-enclosure-body: input differs at Extrude 6.


## Moved 11-19 02:50
## Valve Circular pattern 1 (case rec-valve 52970b93111ac42b, ours)
- Cone pairs: the part's 60 deg ring (z axis, r 70 at z 0, two halves u 0-90 / 90-360 deg) x 8 hole cones on parallel
  axes at 71.3 mm (r 1.5 at z 3, -30 deg, 2.3 m long). 9 of 14 pairs have curves; bool's parked-cones-apart.diff
  (mine now, /root/bool2-scratch) proves none of the 5 empty ones (they reach the half's boundary angle): dropped.
  Only lever left: own parallel-axis cone x cone sections, matching OCCT's splits (bool's cyl x cyl parked on that).
- Raw cut: ours 47/143/94, OCCT 47/142/93 (OCCT merges two crossings 36 um apart into one vertex; ours keeps a 71 um
  edge). The document's part after the pattern: 46/122/72, live's exactly (vol 46868.68 vs 46868.59): ours is right.


## Moved 11-19 03:50
## Other
- Impeller 38073a30d2ffbc3e (Boolean Flansch+Gehaeuse fuse): ours 20/57/40 raw, 5.913847e-5 m^3; live 19/56/39,
  5.913789e-5; OCCT 16/47/36, 5.911132e-5 (27 mm^3 off live): ours is right (told bool 11-17). 02f2f58d / 7cb79697 by
  an edge after the replay's unify, not looked at. Cases (were on vibe5, lost: record again) ~/agents/bool2-keep/imp-cur.
- live 18:35 (truth-1118/impeller-b15-ch26-f3.json, rb 15/16/26/27/28): before Chamfer 26 30/83/54, 71495.173 mm3;
  after Chamfer 26 33/90/58, 71375.524; after Fillet 3 34/99/66, 71096.124 (not 34/95/62: my reference was wrong).
  Live keeps 3 edges between fillet SPLINE and plane x=5 (2.6805/0.5599/0.1885 mm) and 2 between it and the
  EXTRUDED face (0.3117 mid (1.5, 2.756, -43.051), 4.5439), each side. Fuse 38073a30: live's 19 face areas listed in
  live's message (behaviours.md "bool truth (11-18)"): compare ours' 20 areas to find the extra face. To do: the
  document's counts at Chamfer 26 / Fillet 3 on main vs these; re-record the fuse case.


## Moved 11-19 09:20
- qa's P0s: helical Sweep 1 0.4.48 -> 0.4.53 2.2x slower = 0.4.53 lacked 895c8e3b (fixed on main; vibe7 same run:
  0.4.48 341-557 ms, 0.4.53 873-977, main 257-326). Dock tab B Puck Huni: fixed by bool's 7ac4debfa + b27a78c40.
- core's housing unify: Rotieren 1 / Boolesch 1 / Bohrung 1 each a USD merge rejected at 4.9e-15 m^3 vs 1e-9 V
  (borderline), three memo-hit tries + one volume: no identical-result cut without moving that threshold.

## Moved 11-19 09:55 (gate exception detail)
  Live (bool 11-18, suite-0545 #48, /root/onshape-research/live/suite-truth-1118/c2-fan-blade-sweep.json): base
  56/143/88, V 87773.8660 mm3; ours/OCCT after unify 56/139/84, 87773.867 (1.4e-8 apart). Live's 4 more edges and
  vertices: created faces 2 B-splines (deg 1x2, 3x121 poles) 60.3612, 2 cylinders r 3.2 31.8332, 4 other 55.2344 /
  60.3844 x2. (The earlier 87773.636 was wrong.)
  live 18:25 (behaviours.md "bool truth (11-18)", truth-1118/fan-boolean2.json): the 4 extra edges are 0.7 um edges,
  two per hole, between the bolt's swept OTHER face (60.384 mm2) and the walls x = +-54.1 / +-60.1 (e.g. mid
  (-60.1, -76.793, 14.270)): live keeps them, ours and OCCT merge each into one vertex. Rims not split, no seams.
  Asked the lead whether that's a target (sub-um slivers: a shortcoming?).


## Moved from the handoff 11-19 09:59
## Landed 11-18 13:52: the stack as 18501ac5 (c26b, so2b, xscan, unify speed, warm start, gate68 fixes, fold)

## Unlanded work (11-18 13:55 UTC)
### Seam-line fix: in bool2-next (c62ad9396): full-turn face's once-used seam-line pieces on the far side where its
  loop runs up them (flip applied). Root cause, T1 and the dock history in the archive.
### P0 cf-spur-gear: not mine (qa's bisect: bool's afe2cbe90); folds/line/cyl history in the archive.
## Landed 11-18 22:27: bool2-spd2 as 303e9d91 (a7635126a, a47578116, 37e33a5a5; 5735138f5 parked)
### bool2-next = efe1dfb48 (queued above): gate74 3026 -> 3044 taken, 0 new differing; knurls 0 diff-lines.
### bool2-cyl (bool2-ab) = bool2-next + e52828f58 (cylinder patch, +3) + a8ffdc637 (sheets, +11) + 04bcdfdee (+-pi)
- 04bcdfdee: arrange.rs counts a vertex's leaving directions round from their widest gap, so two a rounding either
  side of -pi merge (553beda0: a block rounded by a cut tangent to its walls; end face y 20's arc left at 1e-8 - pi,
  the wall at pi; the face kept only its corner piece -> "a non-manifold result"). Now OCCT's 7/15/10. Tests:
  brep tests::an_arc_and_a_side_leaving_a_vertex_either_side_of_minus_pi_are_one_direction, kernel
  bool_patterns::a_rounding_cut_tangent_to_a_blocks_walls_keeps_both_end_faces_as_occt (recorded 553beda0).
  gate75 (vibex7, tip 8c58fd0ac = cyl + touch) running since 04:00; land cyl+touch when it shows 0 differences.
- The other 6 "non-manifold" (4a71ecd0, 62565707, 1d3f6a2c, 58c9a63c, 90d53f7b, cdd336b4) still decline: tubes at x
  2 m (r 2 / 4 mm + a B-spline sweep), not the +-pi cause. Debug: nm5-dbg.py (NMX: edges used unevenly + pieces),
  nmdbg3.patch (NM: a face's curves and regions, BOOL2_NM=faces); cases in vibex8 ~/tmp/bool2/nm/c. Branch
  bool2-nmdbg5 (bool2-ab) has both + NMT (touching check) + BOOL2_NOWHOLE (drops whole_pair); kp-nm6, nm6.sh.
- bool2-touch = bool2-cyl + 8c58fd0ac: rust_boolean.cpp's shared-shape rule: faces of two arguments on different
  planes that only touch a shared shape (a straight edge's planes meet along it; at a vertex nothing past it, 1 um
  probes) keep it (quiet 2) instead of OCCT-remakes; boolean.rs whole_pair spares an edge the inputs share. 788f0e94
  (plates sharing an edge, fused) = OCCT's (2 solids, all Kept); T1 plates_sharing_an_edge_fuse_as_they_were.
  bool-corpus 896 -> 897, 0 differ (vibex8 nm/rp-touch.txt); bool_ 144. Gate it with bool2-cyl after bool2-next lands.
- Still declining "non-manifold": 1d3f6a2c, 58c9a63c (a cylinder cut tangent inside a block face: a solid touching
  itself along a line; ours declines by design, OCCT's 9/20/12), 90d53f7b (3 users at a touching line), 4a71ecd0 /
  62565707 (tubes at x 2 m with a B-spline sweep: section edges used once), cdd336b4 (below).
- GearLab 1e81ca teeth Union (38d872ebc0c6ad9c, vibex8 ~/tmp/bool2/gl/rec): PARKED (lead 08:20, ~1 week for one doc).
  live: Boolean's fault (live cuts neighbouring flanks 0.17 mm up, one root-cone face between teeth; OCCT leaves ~200
  edges on 3-4 faces). Ours: (1) hub's r 18.07 bottom circle grazes each tooth flank's big-end corner in the grey band
  (stretches_over None; BOOL2_GL1 skips it, ~1 day); (2) then 140 edges used unevenly at the small ends, 1.4-2.8 um
  above the hub's top plane / r 9.81 rim: near-coincident B-spline end sections (days); flank x flank seems fine.
  Gate needs a live-capture exception (OCCT's result wrong). Debug branch bool2-ffdbg (FFDBG, BOOL2_GL1, NMX).
- cdd336b4 (box common sphere, the box's corner on the sphere's pole): the arcs reach the pole at u pi and 3pi/2,
  nothing joins them (OCCT: a degenerate edge u pi..3pi/2). Needs a pole's degenerate edge split + pcurves; parked.

## Moved from the handoff 11-19 13:12
### Chamfer 26: branch bool2-ch26 (f8d871570 = 9545d4a03 on the old bool2-so2b 532da2deb; rebase later)
- The own-neighbour rule (a piece on another solid's face takes its own-solid neighbour's states across an edge with
  no section and no other solid's user) is now only for a cut's tool pieces: the shfall commons decline again as on
  main; chamfer 26 still ours (36/95/62, 1.2e-10). Kernel 494/494.
- But the impeller's Fillet 3 still ERROR with it (parity on c2-impeller-loft alone, vibex6: 33/86/54 vs live
  34/95/62). Chamfer 26's records: tool edges 7 and 16 Deleted in ours, Modified in OCCT (output edges at (1.5,
  +-2.759, -43.05) mm) = the two slivers (pieces 37, 45) the own rule calls Out; in the shfall commons too OCCT keeps
  the sliver's own edge. Experiment (uncommitted in the bool2 worktree): BOOL2_OWNIN (own rule gives In), BOOL2_OWNALL
  (commons too). Mac run 20:50 (ch26-mac.out): "In" makes chamfer 26 decline (non-manifold -> OCCT), so the impeller's
  Fillet 3 goes back to main's 34/93/60 (live 34/95/62): no fix, it only undoes chamfer 26. OWNALL: commons decline
  non-manifold too. Dropped.
- Mac vs Linux (lead 11-17): found and fixed. own_sections' crossings2d (rust_boolean.cpp, our C++; OCCT's
  Geom2dAPI_InterCurveCurve) found the profile of arg0 face 3 crossing the tool's circle (cyl faces 37/38) twice
  0.3 mm apart on Linux, once on macOS (shallow crossing at u 0.99999127), so face 3 x 37/38 had no section there.
  Fix: branch bool2-xscan (728c3d989 on bool2-c26so, with T1 a_profile_crossing_a_cylinder_twice_close_by_is_cut_at_both: fails on macOS without the scan, passes on both with it, worktree /root/projects/noshape-wt/bool2-xs): the B-spline's
  signed distance from the circle / line scanned (64 a span), missed sign changes bisected. Mac now 36/95/62 with
  Linux's volume to the last digit; Linux unchanged. Gate69 = xscan + usd + warm start (bd0f00fc6; unit bool2-gate69, vibex6, starts when gate68 ends).
  Debug dumps (BOOL2_DUMP: pool, split edges, pieces, states, sections, OCCT/own sections):
  /root/bool2-scratch/ch26-plat-debug.patch; scratch-docs/ch26-dump.sh runs it (Mac and vibex6).
- Tried and dropped: OCCT's ComputeState (an own edge's middle): those middles lie on the other face too.
- Debug patches: /root/bool2-scratch/ch26-history-dbg.patch (BOOL2_EH=arg,idx: an input edge's pieces, get_sedge
  merges), the DBGOWN prints (in git history of this session's experiments only).


## Moved 11-19 17:25
- Tally (gate71's branch replay, vibex7 runs/bool2-c5a70b8c5-1118-094421): 445 declined of 3477; outside sh* corpora
  the free-form ones: "a short section from a corner" 30 (parity56, e.g. fe9dc06be3eb4f20: cut, 1 object of 4 faces,
  32 tools; still declined on fan5), "a line and a curved edge" 16 (bool-corpus..corpus6). "an edge on another
  argument's free-form face" 10 are all shfall (bool's).
- bool 19:30 (qa TICKETS a3ff6d481): c2-config-parts-standoffs 2.7 s vs live 176 ms: Revolve 2 (common
  8be054bf822b565e, OCCT 790-980 ms), Boolean 1 (cut a769bbc55aaa8a71, 445-576), Boolean 2 (cut 0d14c4b705d11fef,
  547-687) all decline "a piece's point on another solid's face": a B-spline face lying on the other argument's
  cylinder r 1.6211 mm within 2e-9, normals parallel, no section (coincident B-spline vs true cylinder). Cases on the
  Mac ~/noshape-host/bool-keep/rec-c2-config-parts-standoffs, copied to vibex7 ~/agents/bool2-keep/stand. 19:45: main
  already takes all 3 (0 differ; 8be054bf equivalent, 1.1e-6 volume), the doc matches live to the end (1308 ms vs 203):
  the time is ours (Revolve 2 589 ms, Boolean 1 283, Boolean 2 381; probe 397/198/197 ms per Boolean). Speed work
  (gap >= 100 ms and >= 2x), after the short sections; told bool.
  Sample (vibex7 kn/stand-b1.txt, a769bbc5 x10, kp-fan5): occt_ahead 46 % (IntTools_FaceFace: ImpPrm cylinder x
  the coincident B-spline 22 %, ComputeTolerance/CheckCurveOnSurface PSO 16 %), edge_over_free_face/stretches_over
  ~19 % (NurbsSurface::derivs). Tried 20:20: not asking OCCT ahead for same_surface pairs: no gain (ahead runs on
  other threads), reverted. Next lever: edge_over_free_face / stretches_over's derivs.
### "sheets that meet": 11 of 12 fixed on bool2-cyl (commit after e52828f58): a plane's sheet with a cylinder's or
  cone's is crossing (empty common). bool-corpus 884 -> 895, 0 differ; shfall's 3 differing are main's own. T1
  a_cylinder_sheet_and_a_plane_sheet_have_no_common (ec7cc4c1). parity56 1 and shfall 5 still decline (other pairs).

## State at the hand-over (11-19 20:45 UTC), the working handoff as it was

# bool2: current state (11-19 13:29 UTC)

Second agent on noshape's own Boolean (crates/brep, crates/kernel/cpp/rust_boolean.cpp, boolean.cpp's unify). A case
is taken only where it equals OCCT (or live, with a capture); results identical end to end (docs/DECISIONS.md 11-14).
History: docs/handoff/archive/bool2.md. Rulings: docs/DECISIONS.md (read it first).

## Daily share (DECISIONS 19987fc88 + 15c2c6406): "ours X/N (general A, direct B), OCCT C, differing D"; ours = rust +
  on_face + through_holes + fuse_slabs; "local" (OCCT on the reached faces) is OCCT. xprobe::probe_paths (strict, no
  OCCT run) over the gate's corpora: paths.sh <branch> <binary> on vibex8.
- 11-19 03:30 (probe_paths, bool2-next): ours 2688/3013 (general 2594, direct 94: on_face 68, through_holes 24,
  fuse_slabs 2), OCCT 325 (occt 299, local 25, glued 1), differing 6 (main's). Main = general 2576.
- Decline ranking (gate74 tsv, all corpora): shared shape 74 (65 outside shfall; MINE, lead 04:00), non-manifold 31, short
  section 30, point near two vertices 19, sheets 17, line/curved 16 (fixed), inside-out tool 15, ...
### LANDED 06:57 bool2-next as 0bfc1ed0 (bool, core told); 08:06 cyl+pi+touch as cd1acebd (gate75: 3026 ->
  3066 taken, 0 new differing, nuts Sweep 1 ERROR -> OK; knurls kn75 0; hats/dock udoc17 13 SAME). Dock rec deleted.
### bool2-sheets (bool2-ab) = bool2-touch + 5c26e6aeb (bool's 8a9351e4b cherry-picked; lands with bool-stack3) +
  4e4970235: sheets' shared shapes run ours too; coincident sheet faces facing each other keep the first argument's
  (corpus2 1230858b etc.). Shared-shape set (vibex8 ~/tmp/bool2/sh/c, 68 distinct): 65 taken, 0 differ (3 left: 2
  non-manifold, 1 arc seam). T1 coincident_sheets_facing_each_other_fuse_as_the_first_arguments; bool_ 145. After
  bool-stack6 (fb1269481, gate on vibex8) lands: rebase (drop 5c26e6aeb), gate, land.
### Short sections (30): PARKED (lead 09:45: one model, OCCT right, several days). Branch bool2-stub2 (bool2-ab) = bool2-sheets +
  stubmerge (rule 1 grow, rule 2 move 2/3; BOOL2_STUB1 = rule 1 only) + ARR/ARRF prints (BOOL2_ARR). Cases: vibex8
  ~/tmp/bool2/stub/c. 09:00 replay: both rules 0/30 (two edges 15, arc across seam 8, seam off ruling 5, plane at an
  angle to a cone 2); rule 1 only 0/30 (21 need rule 2). 0932bf41 (ARRF): a plate's pattern holes at z 101.6, each
  corner with two 17.7 um sections (176 tol, past the stub band) and 0.78 um arcs where both arguments' input edges
  lie on one another (origins [10, 21]); at the corner a 0.8 um arc and a 28.6 um section leave in one direction.
  ALL 30 are one family: cuts of a 4-face blank (r 31.75 cylinder, 45 deg chamfer cone, planes z 101.6 / 152.4) by
  8/12/20/32 tooth-gap tools standing on its bottom plane (sprocket-like); the 17.7 um sections rise from z 101.6 at
  each tooth corner where a tool's flank meets the blank's bottom circle. Cause: each tool's round tip (cylinder)
  bulges 25 um past the blank's cylinder and crosses it at a shallow angle at the corners: 17.7 um coincident tool
  edges / sections there; OCCT: one 46.7 mm line up, one 16.6 mm B-spline down (probe_case_near2 / probe_case_near).
- Split (lead 10:00): MINE two edges leave a vertex, edge twice in a face, tiny arc, 1-3-case tail; bool: non-manifold
  (18 outside Shell; cdd336b4 sphere pole is bool's) + Shell group. Tell bool before starting a new reason.
- 1-3-case tail (lead 10:00, mine; bool told 13:04), by OCCT time (small.tsv): 50a8ad1c (point near two vertices,
  2.4 s): p on vertex 47 and 0.5 mm from vertex 42 of 1 mm tolerance (just inside half its reach): a 1 mm group,
  declined by design; OCCT has nothing there (inside): parked. ba2e862f / 620253d9 (cones meeting at an apex):
  coaxial cones meeting only at their apex (BOOL2_APEX: no section) -> ba2e862f "an invalid result" (BRepCheck
  InvalidMultiConnexity on the shell: likely the apex degenerate edges merged into one), 620253d9 "coincident
  pieces differ". f516559d/1ba3f24a (vertex off its plane): OCCT's result invalid: parked. gpf ee95f96d / b7a02f84
  (~1 s fuses): now "a point near two vertices" = 4 regroup rounds not enough; BOOL2_ROUNDS=12 -> "two edges leave a
  vertex": vertices 4 and 5 at one place never merged (two identical lines). Plane nearly along a cylinder (gpf
  1b0a04c4 1.3 s): |n.a| < 1e-3, needs the long ellipse; not started. All parked as chains (13:17).
- bool2-rim LANDED 20:20 as 09ad4e81 (d1bbfb8c6 rebased: B-spline-only rim skip; gate85 0 new differing, knurls 0,
  hats/dock SAME; bool told).
- bool2-exact 2a8da63d4 gate86 FAILED 19:58: T1 b997c4c3 taken but 0.036 % off OCCT's volume (the old fault: the
  exact lines alone don't fix it; with split_params allowance + area chords they did), shfall 6bb081c8 / 5101adb8
  (32-tool fuses) newly taken and differ (main: "an edge off a plane past its tolerance"). One doc closer to live
  (c2-parting-line-draft Rib 1 edges 234 -> 225, live 214). PARKED: the off-plane relax needs the whole tail stack.
- bool2-rim (bool2-xs worktree) a5c64eb23 on main d4a5f85e7: edge_in_face skips an edge along the
  face's own rim edge (same pool vertices, on it at 1/4, 1/2, 3/4): bool's Shell common f9b29e1d (ships today via
  quick_boolean; ours 345 edges vs 319) now = OCCT; T1 an_edge_along_another_arguments_face_rim_stays_whole (data
  f9b29e1da054394b). gate77 on vibe2 -> land, tell bool.
- Tail work (bool2-ab, branch bool2-taildbg = debug + real commits; nothing landable yet): 11cc92b2/dbc28f46 (one doc)
  now equal OCCT in volume and history with: split_params ends allowance (b02a9be42), cycle areas with gap chords
  (62e8ccdad), off-plane check by curve ends + vertex tol (43eebd26d), a piece keeps its edge's own end within its
  merged vertex's tol (bridge), new plane-plane lines on the planes' exact line (Output.edge_line + ffi edge_line),
  new arcs end where they cross an input line ending at the tolerant vertex (incl. tangent). b997c4c3 (fuzz, T1
  an_edge_through_a_tolerant_vertex_off_a_plane_declines) is now TAKEN = OCCT: that T1 must become recorded_as_occt.
  LEFT (parked 11:35): unified differently (ours 25 faces, OCCT 27): two 0.052 mm2 cylinder slivers (arg0 Face5,
  arg1 Face4 pieces) merge into the long fillet faces (arg0 Face4 / arg1 Face5 pieces) in ours; OCCT's unify keeps
  them (sliver F7 on the 4.98 mm2 face's cylinder, split by circle E6). Same topology, same surfaces (probe_small_faces), same history; NOSHAPE_UNIFY_RETRY_ALL changes nothing, so
  not face order. 13:45 (BOOL2_USD print): OCCT's own USD gives 27 on OCCT's shape, 25 on ours (same cylinders bit
  for bit; the slivers share B-spline edge E20/E24 with the long face): USD's union step (u-origin relocation of
  pcurves on the periodic cylinder) likely fails on OCCT's pcurves. Ours is likely live's, no capture (no doc).
  bool2-exact (bool2-ab, 5 commits on main: off-plane, own end, exact lines, arc crossings, tangent; no split_params
  allowance / area chords, so 11cc/dbc stay declined) + T1 b997c4c3 -> recorded_as_occt (2a8da63d4): bool-corpus 897/972 0 differ; fz1144 b997 taken; a46cf4d0 differs on
  main too (OCCT a 1-face degenerate). gate80 stopped for the rim gate: re-gate 2a8da63d4 after gate81 (vibex8 or vibe2) -> land. bool-corpus with all this: 917/972, differ = those two only.
- Edge twice in a face (5; lead 10:00: closed B-spline period first, then exact lines; land each with 0 differing):
  4 fuses (2284dd9c, 2a5740b0, 1a2a8447, d0e4644e; one model: a 40x40x5 plate with an elliptic boss (extrusion,
  a 10 b 6) fused with a B-spline ring: flat closed B-spline bottom with a seam, a skirt, an inner wall on the boss).
  LIVE CAPTURE requested 10:36 (lead (a)): live/REQUESTS.md "bool2: Face Blend asymmetric on the elliptic boss"
  (psb NE studio, noshape mm test); if live keeps the edge whole, land the 4 with a gate exception citing it.
  LIVE ANSWERED 11:23: the edge on the boss is ONE closed edge in live (9 faces, 15 edges, 8 vertices; Parasolid,
  no seams): ours wins. bool2-cbs (bool2-k25 worktree) 90e34814d on main d4a5f85e7: the 4 period commits + bridge
  "flat closed B-spline's seam is a slit" (no env) + T1s a_blend_ring_round_an_elliptic_boss_fuses_as_occt (2284dd9c)
  and an_asymmetric_blend_ring_keeps_its_edge_on_the_boss_whole_as_live (2a5740b0: OCCT's counts -3/-3, its volume,
  2 edges on the boss at z 8 between the seams); volumes within 1e-7 rel (replay's area x 1e-7 band). Gate after
  gate78; 2a5740b0 differs from OCCT there: exception citing live/bool2/q-fb-ell-asym-1119.
  On bool2-taildbg: BOOL2_FLATSEAM (bridge ~817: a seam on a face as_plane flattened is allowed; still env-gated) +
  a0d6ca1f8 (FreeSurface::period: closed clamped NURBS / extruded closed curve; free.rs seam_crossings/projected use
  it; whole_turn takes the clamped box) + 857339ae8 (param_on: Newton stuck on a closed NURBS's seam retried from the
  other side) + 0f5e8c0fd + 91f778bc3 (a plane's seam is one curve, a slit). Result: 2284dd9c, 1a2a8447, d0e4644e
  TAKEN = OCCT; 2a5740b0 differs: OCCT splits the ring's top ellipse edge (on the boss) at 3 points with no geometric
  cause found (edge within 0.08 um of the boss; wall within 0.09 um; only odd thing: OCCT's stray 0.2 mm section of
  the boss with the ring's bottom at z 5). bool-corpus 921/972, differ = 2a5740b0 + 11cc92b2/dbc28f46 only. A
  "section along part of an edge" decline was too broad (-18 cases): dropped. c38a6db8 (cut, a real slit (10,10,10)-
  (15,10,10) in a plane face): BOOL2_SLIT passes the bridge, then "a section touching a face's boundary"
  (boolean.rs ~1958): PARKED 17:20: not the slit: the tool ring's r10 cylinder is tangent to the box's y=0 / y=20
  walls (section circle touching the top face's edges at one point each): a tangency, its own reason.
- 17:25 started "coincident pieces differ" (3: gpf 9f0b0c04 / 54630d70, corpus4 3cca316c, gear commons; bool told):
  BOOL2_COINC print (bool2-taildbg, boolean.rs ~7105), tdbg45. bool 17:30: do NOT change `rank` / `tree` in the by_key
  loop without telling bool (moves Shell slab unify and the gate's SHELLS_KNOWN check).
  Cause (9f0b0c04): a drill-tip cone on another argument's: pieces differ only by their apex edge (a half turn vs the
  input's whole, no curve). Fix on bool2-taildbg: piece_key keys an Apex sedge by its vertex -> all 3 past it: 9f0b0c04 /
  54630d70 now "two edges leave a vertex" (BOOL2_ARR tdbg47), 3cca316c "a non-manifold result" (bool's); corpus4 0 differ.
  Next cause: a 0.5 um section arc with both ends merged into one vertex (apex) kept as a closed curve: section_from
  now drops vlo == vhi curves shorter than 2x the vertex tol (was lines only): no regression, but NOT the cause
  (BOOL2_SAME: no such section). The sliver is seg 35 of the cone face (arrange, a Trig from u=0 to vertex 11 at
  u=-5e-7, v -36.456 mm; seg 31 also from u=0): a circle cut at the face's 2D x=0 where vertex 11 lies 5e-7 off it
  (the boundary rulings at u -5.0e-7 / -4.25e-7: skewed). Next: why th0 (gap() mid / round()) leaves x=0 that close
  to a ruling; snap the cut to the vertex. PARKED 17:35 (2 cases); apex key + tiny-section drop stay on taildbg.
- "a plane nearly along a cylinder" (gpf 1b0a04c4 fuse 1.3 s, eee70623 common 0.4 s): |n.a| 6.6e-4, r 25.9 mm, ellipse
  a 39 m, tilt 61 um over the face. BOOL2_LONGELL (taildbg) lets OCCT's ellipse through: 0 differ, but 1b0a04c4 ->
  "two edges leave a vertex" (a plane face's vertex 13: a 3.8 um line to vertex 14 and a 0.5 mm line along the same
  direction, 14 not split onto it), eee70623 -> "no point inside a face piece". PARKED 18:25.
- ce4e3c99 (parity56 cut, "an arc across a cylinder's seam"; BOOL2_SEAMARC taildbg): a whole circle (r 14.35, z 114,
  vertex at angle 2.25pi) on a face whose th0 is pi exactly: the circle isn't cut at that seam. Next: why th0 isn't at
  the face's seam line (round()/gap()). PARKED 18:30.
- Non-manifold tail (16 outside shfall), families: 9f262529/b0c36850/d5cd8b8d (91-face fuse, pattern tools sharing
  edges), 301238ec/23a4e56c (parity56 cut, section edges used once), e7a4c2f6/f6ff1277 (touching check), e72bf433
  (common), e40bb5a1 (tool's top face has two overlapping circle holes: skipped), + the bool-corpus 6 below.
### "a non-manifold result" (19 outside sh*): 788f0e94 (all 6 corpora) is two plates touching along one edge, fused:
  OCCT's valid result keeps both sharing that edge (4 faces on it, 12/23/14); ours declines by design. Taking it =
  the output builder emitting an edge shared by two solids (builder-level; ask the lead / bool before starting).
  bool-corpus 4a71ecd0 / 62565707 (cut, OCCT 8/16/10: 4 cylinder + 4 plane faces), 1d3f6a2c / 58c9a63c (9/20/12, 2
  cyl), 553beda0 (7/15/10), 90d53f7b (fuse), cdd336b4 (common with a sphere): a block and a cylinder, OCCT manifold
  and valid; ours non-manifold (NMDBG print: nmdbg-2350.patch, kp-nm). 553beda0: the tool cylinder's end faces 4 um
  past the block's faces (y -0.004 / 20.004 mm); ours keeps too few block pieces (piece 0 at (0, 10, 2709) mm, 2.24
  mm from the r 1.41 axis, not kept). 553beda0 fixed 11-19 (bool2-cyl 04bcdfdee, see above).
### "a tiny arc" (4: 63bd60d0, b70f339a, fda1114a, 91923f72; commons, OCCT's result invalid): an arc whose ends merged (tinyarc-2310.patch). Parked.
### "an edge twice in a face" (5 bool-corpus): the C++ bridge's input check (rust_boolean.cpp ~815): a slit edge, not a
  closed seam. Needs slit edges in ours. Not started.
### Next area (lead 11-18 18:15): bool ranks the decline reasons by recorded cases blocked; I take free-form sections
  and B-spline faces (bool: Shell and offset). Daily report: share taken + differing cases; no millisecond A/Bs.
- Mine, outside shfall (bool 18:40, replay 118bf5b52eaa636b): short section from a corner 30, non-manifold 19, line
  and curved edge 16, sheets that meet 12, two edges leave a vertex 6, edge twice in a face 5, tiny arc 4, face lost 4,
  cylinder face without curved edges 4. bool's gate 123e6b177 queued on vibex7 behind gate73.
- Short sections (BOOL2_SSDBG print in occt_sections, uncommitted in bool2-ab on bool2-fan5: don't commit; kp-ss on
  vibex7): fe9dc06b / fc32eee2 / b1ccf1f3 are cylinder x cylinder pairs (ctx faces 0 x 132/84), OCCT's section in 3
  curves: two of 15-23 mm meeting at a branch point, and a stub of 8-15 um (40-77 tolerances) from a face corner to
  it. OCCT's builder takes the stub into the corner vertex. Idea: drop the stub and merge the branch point into the
  corner vertex (Pool::merge_with, OCCT's MakeVertex rule); check first what OCCT's result has there (vertex position
  and tolerance near the corner) so ours equals it.
- 18:45: OCCT's result (fe9dc06b) keeps the corner vertex in place, the long branch's B-spline edge running into it
  (tol 5.8e-5), the other branch gone there. Tried: drop the stub + Pool::grow(corner, d + tol)
  (/root/bool2-scratch/stubmerge-1845.patch, reverted): parity56 short-section declines 30 -> 0 but only 7 taken,
  all 7 differ (1abdab7d 120/77 vs 117/74 edges/vertices; history), the rest decline later (two edges leave a vertex
  in one direction 17, a seam off a ruling 5, ...). 21:10 (kp-ss4, stubmerge-2110.patch): 1abdab7d's OCCT moves the
  corner up 10.94 um toward the branch point (16.4 um away; fits OCCT's MakeVertex bounding sphere with the branch's
  tolerance ~5.6 um), fe9dc06b's keeps it in place: no single rule yet. Parked; ours' merge_with also uses the
  midpoint, not OCCT's sphere of both spheres (centre between their far ends).
- 22:10 two rules (stubmerge-2210.patch, Pool::move_to): a branch curve whose tol >= the stub (fe9dc06b: 5.79e-5 vs
  9.3 um): grow the corner, keep it; else (1abdab7d: all 1e-7) corner + both branch ends one vertex at 2/3 toward the
  branch (OCCT's vertex there exactly: mean of 3). parity56: 514 taken, 0 differ (was 7 differing); the 30 now
  decline later: two edges leave a vertex in one direction 17, an arc across a cylinder's seam 9, a seam off a ruling
  5, a plane at an angle to a cone 2. Next: what the 17 see (probably the moved corner vs its input edges' directions).
- Old debug work (dump/IVDBG/probes) saved as /root/bool2-scratch/ab-wip-1740.patch; xprobe.rs etc. stay untracked.
### Fan-blade fix: now bool2-fan5 (above); nuts: ebc58e0be took bool's nutcrest cut (135.2676 mm3, live 135.268), core's nuts3
  T1 passes either way; tell core d30002ca when fan5 lands. Older notes in the archive.
### Exact unify drop: branch bool2-usd2 = 017fa3a86 (on bool2-fanb3 f9f8c11ff = ebc58e0be rebased on c5a70b8c5)
- drop_same_regions again, exact: a group (one non-closed free-form surface, handle + placement + orientation) is
  left out of the volume check only when its edges around use the very same pcurve handles over the same parameter
  stretches (joined where contiguous) on both sides. sp-cut 990256f0 77 faces (main); standoffs unify inputs (vibe2,
  5 rounds alternating, load 22-30) 88 -> 45, 236 -> 139, 103 -> 40, 212 -> 122 ms, identical faces/edges/volumes;
  Revolve 2 (usd-2/5, 250 ms) not dropped: USD approximates one merged end curve (two B-splines -> one) and splits
  the long iso-lines into new pieces. Kernel bool_/usd/unify tests 147 pass; fanb3: bool_ 140/140, brep 62.
- Gate fanb3 + usd2 together (017fa3a86) after the stack lands; scripts scratch/usd2.sh, usd2d.sh (BOOL2_DROPDBG).
- Scratch (uncommitted in bool2-ab): xprobe probe_case_box_edges / probe_case_unify_both / probe_case_diff; debug patch
  fanb2-debug.patch.

### Chamfer 26 (xscan landed 11-18): notes in the archive (moved 11-19 13:12).

### Both
- Uncommitted on purpose: crates/kernel/tests/all.rs registers the scratch probes (copy /root/bool2-scratch/all-with-
  probe.rs), crates/kernel/tests/{xprobe,sampler}.rs, scratch-docs/. Take all.rs out before land.sh, put it back after.
- Landed 11-17: bool2-chamfer as 3844e0a5 (valve Extrude 3 own sections, unify reordered tries).

## Model-suite speed list (11-17): in the archive (moved 11-19 02:05).

## Valve Circular pattern 1: in the archive (moved 11-19 02:50; ours is right, live's 46/122/72).

## Other: impeller 38073a30 notes in the archive (moved 11-19 03:50).

## Agents
- Roster (lead 11-19 13:29; full ids only; stopped threads must not be messaged): core
  d30002ca-d013-4435-b7d9-7da9cb564cec, blend de66e481-56e2-4005-a5d3-9fa897ca5fde, bool
  754a9804-feb1-4153-b32a-acf9b2a6d94d, qa 40e853dc-757f-4498-878f-00b41fd8af75, psui
  7e3f2635-05e1-47c0-8594-9dbc0e9a79d7, live2 d7f4a0c2-c793-48d7-b749-13dbbe1dd3a1, studios
  31c9b9ba-e7f8-40f7-85cb-242bab86dbe6, dx aad5e825-f86c-432d-b4ac-3aab402e7052, asm d8d597e0-675c-497b-a804-544ceab99bbd,
  skasm 18a8ce79-5f2e-4089-9ad8-cc9d1182f0e6, live e8549719-ef9a-42de-960c-e24b29f142a6 (stopped until ~21:00,
  Bbedward limit). Stopped: qa 132756b3, psui 4973e295, live2 79ae5cdb, old studios/dx.

## Gate exceptions (lead 11-18 07:15, this case only; the gate's 1e-9 stays for everything else)
- 3e8c1e09636d4fd7 (c2-fan-blade-sweep Boolean 2, base cut by two bolts; ebc58e0be, T1
  bool_patterns::a_thread_crest_through_countersink_rims_cut_as_occt at 1e-8): ours 87773.867193 mm3, OCCT 87773.867457
  (3e-9 relative). Same 66/174/106, identical history (probe_case_records), every face's area within 1e-6 and centroid
  within 0.1 um; the gap is on the countersink cones, whose OCCT sections are 5 um-tolerance curves (offset surfaces).
  Live: 56/143/88 V 87773.8660 (4 more 0.7 um edges live keeps; archive has the detail). Landed in 0bfc1ed0.

## Open decisions (with the lead)
- None open. parting-line Split 3 went to core (11-17).

## Next items, in order (lead 11-17)
0. Impeller Boolean 2 (lead 14:00): case 24a423fdd4de02f6 (vibex7 ~/agents/bool2-keep/imp-b2), ours, 226-273 ms vs live 13:
   convert 93 (offset_as_bspline refits: cached on bool2-imp2 a67529c4a, unit bool2-imp2b -> imp2.out), sections_ahead
   52, build 43, check 42 (BRepCheck on all faces; made_valid only for tool-by-tool cuts), unify 18.
   Then: qa 11-18: c2-helical-sweep Sweep 1 ours 125-128 ms vs live 27.5 (live/qa/rbstep-1118/tables2.md; speed only).
1. Land bool2-k25 (gate72, hats/dock, knurls ours-only, document A/B); then tell bool and core.
2. Chamfer 26 (bool2-ch26x) stays gated; Fillet 3 is blend's (de66e481).
3. Standoffs (dx 11-17): on c26so all three thread Booleans are ours (32/32, doc 13.5 s -> 3.7 s, hosts differ). Unify
   check speed: fe3d4411f on bool2-xscan (faces_volume per face on all threads, bit-identical sums; the fallback
   passes reuse the memo): Revolve 2 421 -> 237 ms, Boolean 1 204 -> 78, Boolean 2 503 -> 193 (vibex6 alternating,
   probe crates/kernel/tests/usdprobe.rs uncommitted in bool2-xs, inputs vibex6 ~/agents/bool2-keep/usd-stoff,
   scratch/usdab.sh; debug build /root/bool2-scratch/boolean-usd-debug.cpp). Left: Revolve 2's check counts all 30
   faces changed: fixed by 9c7f1e399 (faces on one free-form non-closed surface whose merged face has the parts'
   boundary left out of the integral; Revolve 2's unify ~200 -> ~45 ms, same results, kernel 506/506). Our Boolean
   (standoffs cases, 1 thread, vibex6: dd85 238, 5608 263, 2a11 303 ms): samples ~25 % stretches_over -> edge_crossing
   -> nearest_to_edge -> seg_param (Nurbs param_of, 70 a crossing), ~30 % occt_ahead (IntTools_FaceFace, its
   ComputeTolerance PSO). Tried: edge_crossing's min_by keyed once: no gain (dropped).
   Lead 11-17: keep the bore-edge passes (no skip for cuts; dd85's test stays at 1e-9). Warm start: bd0f00fc6
   (nearest_to_edge's golden steps find the B-spline foot from the last one's, NurbsCurve::param_from): 10-13 %
   faster (dd85 422 -> 379, 5608 452 -> 399, 2a11 533 -> 462 ms loaded), counts/history same, volumes within 1e-11.
   Tried: stretches_over's samples seeded from the last on-surface uv: bit-identical but no gain (dropped).
   A/B binaries vibex6 ~/tmp/bool2/ab (k-old = 9c7f1e399, k-warm = bd0f00fc6), scratch/boolab.sh, warmchk.sh.
4. (loft-paths-guides Mirror 1: moot, core's f824d120 reverted the loft change; back to live's volume.)
5. Sprockets end to end (document A/B) after the land; the next documents from qa's speed report whose inputs match
   live (re-run speed-gaps.py on the newest suite).
6. Parked: valve cone x cone sections (OCCT's splits), helical Sweep 1 (cylinders in analytic_freeform_sections give
   one curve where OCCT gives 10, slower), impeller Extrude 4 (OCCT internals).

## Recent facts
- gate.sh landed: builds through ~/bin/slot with SLOT_MIN_GB=12 (19afd619), models snapshot per run (a2952b32).

## How to run
- Host by load (DECISIONS a67911bef): each run on the lowest 1-min load of vibex7 / vibex8 / vibe2 (or the Mac's free
  slot); vibe2: ~ngyekta only. Private docs (hats/dock) stay on vibex7.
- Builds, crate tests, probes: vibex6 again after gate68 (lead 21:23; vibe2 only finishes gate68 + udoc9): `NOSHAPE_RB_HOST=vibex6 tools/tiers/rb.sh bash -c 'export CARGO_BUILD_JOBS=4;
  cargo test --release -p noshape-kernel --test all <f>'`; rb.sh drops env prefixes (env inside `bash -c`). Don't
  rb.sh from a worktree while a binary another unit still needs is being built from it (rb.sh resyncs the sources).
- Gate (vibex6, one of mine at a time; never kill a process tree from a computed pid, stop units with systemctl): `systemd-run --unit=bool2-gateNN --collect -p WorkingDirectory=/root/projects/
  noshape-wt/bool2 bash -c 'bash tools/bool/gate.sh <sha> --jobs 4 --host vibex6 > /root/bool2-scratch/gateNN.out
  2>&1'`. hats/dock: /root/bool2-scratch/udoc-v7b.sh <branch sha> (cli-base on vibex7 must be the branch's main).
- My own suite pairs: through ~/bin/slot on a `cp -al ~/parity/models` snapshot (NOSHAPE_GATE_MODELS for gate-host.py,
  DECISIONS 11-16); model files change under long runs otherwise.
- Land: clean gate + main green (`/tmp/lead/main-status`), all.rs out, `NOSHAPE_LAND_QUEUE_HOST=vibe2
  tools/tiers/land.sh`; `--status`.
- Probes (crates/kernel/tests/xprobe.rs, `--ignored --exact xprobe::<name> --nocapture`, PROBE_CORPUS=<dir>
  PROBE_CASE=<full id>): probe_case_repeat (PROBE_N, NOSHAPE_KERNEL_PROFILE=1, PROBE_SAMPLE=<file> -> /root/bool2-scratch
  samples.py, callers.py), probe_case_volumes (ours vs OCCT). DBG_AHEAD per-pair OCCT section
  times: /root/bool2-scratch/valve-ahead-dbg.patch. NOSHAPE_OWN_SECTIONS=0 turns our own sections off for A/Bs.
- Recordings: vibex6 ~/agents/bool2-keep/{hel-new (helical 9b68aa2b07dc2986), imp-new (impeller f10ce62e55f0fd49,
  f37371b831c5339f), so-rec2 (standoffs' three)}; vibe5's (rec-valve, spr-rec, imp-cur, lp-rec) are lost.
- Doc timing: qa's saved docs vibex6 ~/qa-parity/saved (copy + -wal/-shm each run); `noshape-cli feature list`,
  regen_ms; under load use user CPU (`/usr/bin/time -f %U`, RAYON_NUM_THREADS=1). Private docs (hats, dock): mode-700
  copies in ~/tmp/bool2, deleted after (udoc-cmp.sh does it). gdb only on a child (ptrace_scope 1): `gdb -batch -ex run
  -ex "thread apply all bt" --args ...` under `timeout -s INT`.
