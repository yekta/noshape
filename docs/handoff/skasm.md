# Handoff: skasm (sketch solver, sheet metal, frames, the parity translator's sketches): current state

Updated 2026-11-21 22:39 UTC (restarted 11-17 09:20). Read docs/DECISIONS.md
first; history is in docs/handoff/archive/skasm.md (read it only when a line here points there).

## In flight (11-21 04:30 UTC)
- LANDED 0f86a0f8: flat-face sketches (louver 36/36, modify-joint 11/11) + Rip 1 (c2-cov-sm-rip 5/5); 6a4ac440
  Recognize 6/6. SM 25 docs (Mac): 22/24 docs, 291/295 features.
- QUEUED ca26da57 (06:43): hem-plus Offset surface 1 (Mac-only; OCCT's joined offset fails in metres on the Mac,
  op_offset_shape retries in millimetres: Mac 6/6, other offset docs unchanged; g++-13 build clean on vibex8).
- LANDED bc92cab1 (P1): sketch regions over tiny gaps: an end joins a Use'd curve's end, or a circular-pattern copy's
  of one, within ~20 µm (live use-rule.json); plain ends and text outlines' (fixed curves sharing ends) only 0.2 µm.
  Untested in live: linear-pattern / mirror copies of Use'd curves, pattern copies of plain lines.
- Handed to core 11-21 10:50: infento a3f7a1 Linear pattern 1's direction JMx = Chamfer 2's new edge on Extrude 1's
  ring (ours runs -y, live +y); and P0 c2-genesis-trunk-hanger 20 -> 12/73 = core's 18354ec79 (bisected; fixed by
  8631c5717, Mac 20/73 on that tree; main still 12/73 from a second cause core is bisecting).
- QUEUED b3345866 + 6ef0420c + af26939c (Offset label hover; 22:4x; after they land tell core + qa, report to lead; A/B green: suite 127 -> 129/188 docs, 1986 -> 1999 features, no doc worse; start-extrude 34/34,
  overcut-dogbone 10/10; base ab6-base.out vibe2, new vibex7 ~/agents/skasm-keep/ab6-new.out):
  - parity.rs: ruled_piece_from_query (mutualtrim SWEPT_*), region_query_sketch hint + pieces rule (scissor
    b58244 Extrude 1 KICB 445 mm², 0139f2 Extrude 8 Sketch 6's regions); recheck sc-* in vibe2 ~/tmp/skasm/scm.
  - sheetmetal.rs rolled_bend: k::thicken (exact; start-extrude 29 -> 34/34) on the face `tightened(1e-7)` (new
    kernel Shape::tightened / tighten_tolerances: the hem corner trim leaves 6e-5 m edges, the fuse closed the 0.02 mm
    R0.01 hem slot; hem-jog Part 1 27 -> 29 faces, 18904.9 -> 18842.05 mm³, live 18842.62; edges 56 vs 61, flat
    "Collision" INFO left). SM docs otherwise unchanged (23/27). Curved-rip experiment dropped (no effect).
  - equations.rs PointOnEllipse scaled by b as a constant (core 11-21: knifehandle P4's free minor radius moved
    16.63 -> 15.95); test constraints_more::ellipse_held_at_its_major_ends_keeps_its_minor_radius.
  - inference.rs Perpendicular reach (live "Reach brackets" 11-21, 13 takes): start within 22.5 px of the held line
    (infinite) or 30 px of its midpoint -> Perpendicular; on it off the midpoint / chain from its end: Vertical.
  - kernel common.cpp tighten_tolerances (ShapeFix_ShapeTolerance + SameParameter + UpdateTolerances on a copy).
  - scissor single-docs (vibe2, base -> new2): 0139f2 13 -> 17/36 (Extrude 8 matches); b58244 norefs 2 -> 6/106.
- Sketch-tool feel P0s (qa walk 3, TICKETS 11-21 20:5x), in the same land, scene tests green (203):
  - FEEL-SKFILLET-FLOW: no arc on a hovered corner; the arrow / "R" leader only while pending (`fillet_handle`).
  - FEEL-CHAIN-ORIGIN-DROP: a Line chain goes on through the Origin (not "closed"); FEEL-TANGENT-ARC-AT-ORIGIN: an
    own point on the Origin wins over the Origin inference; Slot takes a hovered end point's one curve.
  - FEEL-SPLINE-PREVIEW-VANISHES: the cursor point equal to the last click isn't added to the preview fit.
  - FEEL-OFFSET-PREVIEW-LOOK (live 20:55 answer): end points drawn; hollow arrow's tail on the preview line; black
    dimension line + 2 filled 5 px arrowheads; "d" label always (in the gap from 13.5 px, else on the picked line
    with the line run past it); hover: arrow #4674af/#9bb2cc, label tan #ccb396 edge #d99c3e, arrowheads hidden
    (`offset_label_hovered`, set by the label's on_hover). Open: the tiny "—" tick by the tail (x4-2-z1-hovers). FEEL-POLYGON-SPINNER-DRIFT closed (live 10-27).
    FEEL-EXTEND-NO-EXTEND closed (two picks as live 2992); the stub's look unknown (live's (8) retake did nothing).
  - FEEL-TEXT-BOX-PREVIEW (qa walk 4, in 6ef0420c): the Text tool draws only the dash-dot box between its two clicks and
    4 corner points, no text, until ✓ (live sketch/132, 134). Live's box looked ~3.3:1 for 4:1 clicks: unconfirmed.
- asm 11-21 22:0x: asm-scissor-lift fed22d (Top Openings 15.5 -> 23.0 mm, Sketch 32 PARALLEL left): not c721b3513
  (draw-time inference only) nor 1d3541912's sketch-curve rule (A/B on vibe2 scm/sc-fed22d: same 28/60, 14.95 mm
  both); its plane face query differs base vs main. Told asm (rerun after b3345866).
- loft-paths-guides: Sketch 4 / 6 match now (16/29; first diff Loft 3 edges, Loft 4 ERROR: loft owner's).
- hem-jog next: edges 56 vs 61 / vertices (live: 4 spline corner faces + mitre planes) and the flat "Collision".
- Next after that (lead 11-21): the next item of "Next, in order"; sketch-solver parity rows if sheet metal has none.
- LANDING: forms 21/21 (c2-sheet-metal-forms 10 -> 21/21; Mac A/B vs 97df380f1: 108 -> 109/174 docs, 1779 -> 1790
  features, no other doc changed): Sketch 3 connector at live's frame; Form 4 fuse skipped when adds add nothing;
  Form 5 region_from_query derived branch; lance bends meeting at asin(1 - h/(rmi+rme)) with no wall, the lance
  turned 90° (strip along the location's y); knockouts too small for their bridges only the hole; parity
  names_the_origin (scissor 0da59a's sketch planes).
- asm-scissor-lift 0da59a: "our parts []" because its 6 Derived sources (other documents) were never fetched:
  live REQUESTS skasm 11-21. Re-run scissor when live installs them.
- P0 11-21 (tablet-stand Sketch 3 warning): f9ad07b50's exact-thicken step; core's 199faacf fixes (verified by
  compat userdocs.py on vibex7). Compat repro: ~/tmp/skasm/compat on vibex7 (userdocs.py copy).
- After landing: re-check 850efe (72fe51df1 now on main).
- LANDED af717ef2: std Jog's two bends (c2-sm-jog-panel 5 -> 12/12; full parity 1723 -> 1730/2315, no other doc).
- LANDED 381554f1: Hem 1 (c2-sm-hem-flange) keeps "Sheet metal model cannot be unfolded." (219 -> 222/295).
- LANDED 9f5250bf: SM loft profiles with different corner counts walk the least total fold (host/loft.rs).
- SM suite (Mac 11-21 12:30, my build after bc92cab1): 22/23 docs, 259/260 features; only c2-sm-loft-basic's apex left.
- Loft apex (c2-sm-loft-basic 38 vs 40 faces): box rule still open after live's 12:32 copies. Samples:
  live/skasm/sm-loft-fans-1121/<name> (chordal 0.1 / 0.05 / 0.035 / 0.022 in = 3-6 bends; "-plane4" = Plane 1 at 4 in,
  two apexes, other differences too; fan-tol0p05 = loft-basic itself; 0p022 / 0p035 -simple / -sized1 / -round1 with
  edge-ends.json). Parity on them: copy the dirs to Mac ~/noshape-host/tmp-skasm/fans, set NOSHAPE_PARITY_MODELS;
  skasm-scratch/loft/ours.sh saves + dumps ours. Scratch: skasm-scratch/loft/flat.py develops live's walls / bends at
  the neutral radius into bend m = floor(n/2)'s plane (u along it, from its cylinder's origin: sized1's origin differs
  by 1.0954 mm, the others agree; canonical u = from the definition corner A (0, 203.2, 0)'s foot on bend m's axis);
  rect.py corners, rnd.py round centres, cen.py centre lines, arc.py, verts.py. FOUND: live cuts the apex by ONE
  rectangle in the flat, open toward the tip: u < uP (square to bend m) and vlo < v < vhi (lines along bend m).
  Rectangle = scale x a box about its centre (0p022 at 1.0 / 1.2 to 1e-4 mm). Round (Scaled 1.0) = one circle centred
  where the first and last bends' centre lines cross (both samples, 1e-4 mm), radius = to the box's far corner
  (umax, vmin) (0p022 5.1083). Simple: every wall ends on one arc in the flat (0p022 centre canonical (-1.4033,
  0.0699), r 5.3194; the bend strips end where their tangent lines cross it). Sized 1 mm: under-sized warning, each
  bend squared at its wall corners (= ours today). Box (canonical mm, scale 1): 0p022 u -0.1395..4.1965, v
  -1.2643..0.7095; 0p035 u -0.1030..3.9320, v -0.7085..0.8526 (u from the round's radius). vmax = the last wall's
  corner v (free edge x its bend's tangent line); umin = bend 0's wall-0 tangent line's point nearest A (0.6 / 0.3 µm,
  float32 captures); umax, vmin unexplained (tried: strip overlaps, square bend ends, A's feet, virtual sharps, arc
  hits, 3D bbox in bend m's frame). Ours: each bend squared at its corners.
- Debug std: throw a custom regenError message (FS println goes to notices, not stderr); restore edited files with a
  fresh mtime (rb.sh rsync keeps the old one and cargo skips the rebuild). NOSHAPE_FS_TRACE=1 names the throw's line.
- LANDED 6d80df66: Unfold unrolls only its bend, Refold puts back the held side (unfold-refold 6/6, 212/295).
- LANDED eca6e645: Thicken / Convert flat lies by the largest wall (`Definition::extracted`); Sized relief ends
  depth + R tan(a/2) in (live 8.3 mm). 201 -> 209/295.
- Rejected: extract surface's faces in body order (hem-flange SM model 2 7 -> 4/8, frames BUTT/BUTT2 swap);
  patch ~/agents/skasm-keep/extract-order.patch (local).
- c2-hgr-riser-weldment: dx's (lead 11-20).
- Rip 1 (c2-cov-sm-rip) 3 -> 5/5: host merge_coplanar_rips builds a wall split by a rip ending at real corners as one
  face (the bends beside it square); not where the rip ends inside a straight run (louver's in-line walls stay pieces).
- Tools: /root/skasm-scratch/pm.sh (ONLY=, SAVE=, build + run), pmrun.sh (TRACE run of the kept binary), smlist.txt
  (24 SM docs), CLI batch on saved docs: ~/tmp/skasm/saved (copy .noshape + -wal), hemq*.sh / jq.sh / jogq.sh.
- Feel stack 4e51cce3 verified by qa (green f380b6928); P2 drawn_failed LANDED 8fd60c7a.
- cf-sheet-metal-model-plus P1 line (SUITE-REGRESSIONS-1108) closed: 318 ms vs live 917.

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
- Answered 11-18 13:xx: wave-washer sketches.json (Sketch 4 / 3 solved inward) and surf-intro Derived 1 sources
  (c2-surf-intro-*/derived/<id>/, live.jsonl by rollback): rerun wave-washer and the surf-intro docs.

## Next, in order
00. lead 11-20 SM suite models / qa 00d: all match but c2-sm-loft-basic (apex, above). Mac 12:30: 22/23, 259/260.
00b. qa P2 SUITE-INTRO-TAILFIN-INTERSECTED: LANDED 37638c76: an Intersection lands within 1 µm of live's ends (ours 0.19 µm
   off): no INTERSECTED left / no take-back in surf-intro boundary/fill/mutualtrim/ruled (A/B over the 7 INTERSECTED
   docs: nothing else changed). Left: Tail fin surface area 1.2e-5 off (3567.145 vs 3567.188 mm², ~0.6 µm of profile
   length, cause not found) and 3D fit spline 1's shape (starts x 32.70, live 33.94, same ends and start direction):
   core's (fit spline), told via the lead; Boundary surface 1 fails on it.
00c. LANDED 5cf3fc94: a feature pattern re-runs each seed once per group: rocker-enclosure-body Mirror 3 200 -> 88 ms.
   Left: nested seeds re-run as tools (Mirror 1 / 2 inside Mirror 3).
00d. LANDED 01d8226a: an edge with no curve of live's type within 5 % of live's length takes the nearest of any type
   within 5 % (live's part first): surf-intro Boundary surface 1 builds (boundary conditions WARNING is core's).
00e. LANDED 8ac743df (Corner+Joint): SIZED_ROUND corner relief under its bend-allowance square warns
   SHEET_METAL_CORNER_UNDER_SIZED, not cut; the translator truncates a selection over the dialog's limit.
00f. LANDED 1d354191 11-21 (Mac A/B 1976 -> 1990/2680, no doc worse): Bounds::trim with a solid start (start-extrude 19 ->
   29/34); one bend's end at rips = BEND_END (corner-break 20 -> 23/37); frame trim by a surface body (frame 16 ->
   17/23); edge-on circle Use (loft-paths-guides Sketch 4) and a constraint's non-part edge = an earlier sketch's
   line (Sketch 6); a part edge between two definition faces with no joint = their shared definition edge (hem-jog's
   TANGENT side); opPattern of a sketch body by a scale scales its geometry (std Hem's arc copy: hem-jog builds, INFO
   flat collision, 27/52/27 vs live 29/61/34).
   Next: corner-break model F convert of a filleted box (live 3 parts; ours "cannot be built"); start-extrude "arcs as
   bends" (2 bend-end arcs not circles: rolled_bend's thicken_plain leaves the r 11.5 offset's end edges B-splines;
   503396456 took the exact rebuild off sheet metal for tablet-stand's vertex refs: try k::thicken in rolled_bend only); frame Trim group 66 vs 64 edges, End cap signal 11, gusset on +y (live -y);
   hem-jog flat collision. Loft 3 (37 vs 33 edges) is core's (TICKETS LOFT3-EDGES).
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
