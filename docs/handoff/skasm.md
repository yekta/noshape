# Handoff: skasm (sketch solver, sheet metal, frames, the parity translator's sketches): current state

Updated 2026-11-28 11:37 UTC. Read docs/DECISIONS.md
first; history is in docs/handoff/archive/skasm.md (read it only when a line here points there).

## FIRST (11-28 15:4x UTC): QUEUED 0b1191aa (land7: USE-MID + ui-parity free spline derivatives + used-line menu test fix; ps-ui lib 698/0 on the Mac). Uncommitted: assembly-ui preview-in-state: a new Slider/Revolute previews in the state; a Fastened or an edited mate reads MateSession.opened_at (solved when the dialog opened); T1 a_new_sliders_preview_reads_in_the_state_a_fastened_does_not passes on the Mac; full asm-ui lib running (~/tmp/skasm/tasm.log), then commit + land8. Replay part 2 on vibex8 (~/tmp/skasm/rep2.log). Report to lead on land: also the 15 sketch-used-* now stop at step 28 (Tangent vs Horizontal hinges on the click pixel: SKETCH-CLICK-ONE-PIXEL, lead's call).
## LOFT (11-28 15:1x): cf-surface-pattern Loft 1 (3 open splines, one face): ours 20482.36 mm², live 20480.53; box within 2 µm. Probe (skloft.patch, reverted): v centripetal 20482.24, uniform 20482.13, LONE pairing 20482.36, no natural_loft 20537.63 -> not v. Left: the sections themselves or u pairing. Asked live (REQUESTS 'skasm 11-28 15:1x': spline lengths ours 193.819617/192.807408/215.860145, face B-spline + 9x9 uv samples) -> live/skasm/loft-1128/.
## REPLAY 1128 part 1 (14:3x): 4/21. 13 sketch-used-* stop at step 13 on ONE setup diff: .sketch.geometry[1].derivatives[0][0] live 17.8292 / ours 18.1242 (the setup spline's start derivative) -> fix first, reruns all 13. Others: sketch-rclick-line-circle-menu step 10 constraints 2 vs 3; sketch-slot-on-arc step 6 points[0][1] -4.9087 vs -6.0534; sketch-used-select-equal-circle / submenu-box: import message null at step 12-13.
## NOW (11-28 13:24 UTC): QUEUED d80d6cdd (slot on a spline) and 7487b30e (USE-MID: a Use click within 11.5 px of a model edge's midpoint makes a point; a used edge's glyph still re-uses the edge). Replay running on vibex8 (part 1 of /root/skasm-scratch/bin/replay1128.txt = words 1-21, log ~/tmp/skasm/rep1.log here; part 2 = words 22-41 after).
NEXT: ASM-SLIDER-FILLET-FACE-ORIENTATION: waiting on live REQUESTS 'skasm 11-28 13:3x' (Slider 1 matedCS + fillet connector rule + why REST shows the move before ✓ in slider/revolute takes but not fastened). Survey of /root/parity/models matedCS: X follows the curve's/surface's own frame (Z=±x gives ±y or ±z), not one rule; ours: world X projected, else Y (assembly eval.rs connector_frame / implicit.rs mkx). ui-state reads stored placements while a mate dialog is open (assembly_ui_state.rs 'previewing').
QUEUE after: cf-surface-pattern Loft 1 (wire loft through 3 profiles) 20482.36 mm² vs live ~20480 (Part 1 +0.02 % at Thicken 1); bool files the TICKETS row; likely the section pairing / parameterisation fixed for surf-intro (lead 11-28).
All three items coded, not committed:
1. Offset: live's (10,-4) and (10,-8) clicks FLIP the side (shots: copy -5 -> +5), they hit the hollow arrow (tail on the
   copy, running to -22). Ours: `offset_arrow_at` (whole arrow, 10 px from the segment) for click + hover look;
   `offset_label_hot` (label hover yields to the arrow); ps-ui sketch.rs label click skips when the arrow is under it,
   both label hover boxes use offset_label_hot. Scene T1 a_click_on_the_offset_arrows_tail_flips_the_side PASSED (Mac).
   ps-ui offset_a_click_beside_the_picked_side_drops_it FAILED (live sketch-offset: a click 7 px off the picked line
   drops it; the arrow took it). Fixed after: the scene arrow branch skips when `hovered` is a pending curve. RERUN
   /root/skasm-scratch/bin/toff4.sh on the Mac (scene+ps-ui offset, clippy), then tnext.sh.
2. Chamfer: sketch tools.rs `chamfer` adds Coincident(startA, startB) (live's 13th COINCIDENT: distanceA/B starts on each
   other; point-point coincidents are union-find, no redundancy row). T1 onshape_ux chamfer_corner_cuts_both_lines_... extended.
3. MC corner: already reconciled by 5637f3887c (corner = straight side's end, VertexEnd Z = -tangent = into the edge).
   tests.rs spline_face_inferences_are_its_edges_points now also checks the top corner (40,30,20) Z -z, X +x (live
   rq-1128). Not yet run: tnext.sh (sketch all, scene lib, assembly spline_face, clippy sketch+assembly).
TICKETS: offset + chamfer rows noted; dropped stale union duplicates of ASM-MC-FRAME / ASM-MC-SPLINE rows (re-grep
before land). Then commit, touch -c, land.sh.
Replay: vibex8's slot busy (qa uip 4 h, bool gate, ops); no wait of mine left there. Re-queue bn02so8gg's set
(sketch-rclick-line-circle-menu, sketch-horizontal-key, sketch-vertical-key, sketch-3pt-arc-reversed-sides,
sketch-slot-on-arc + /root/skasm-scratch/used.txt), add sketch-offset-open-chain, then the asm replay.
- QUEUED f5233e5e (the matrix + trim + pattern); handoff commit c6807d5839 rides with the next land.

## NOW (11-28 10:0x UTC): the sketch hover × state matrix
- docs/parity/sketch-hover-matrix.md: 48 =, 0 ?, 0 ✗, 40 L of 88 (14 new GPU cells in hover_matrix_tests, all green on
  the Mac; glyph colours `constraint_glyph_colors` + T1; used points take the constrained colour). The 40 L asked in
  live/REQUESTS.md "skasm 11-28 09:2x" (batch 12): add a cell per new take when it lands.
- Same commit: live rq-1128 answers: Trim hands a cut-off corner to its Coincident partner (point on the trimmed line,
  origin moves with it; sketch tools.rs `hand_over_trimmed_ends`), Linear pattern with a line selected ignores an empty
  click (asked live: line alone / two circles / circular, REQUESTS 09:3x). OPEN from rq-1128: the too-large chamfer's 9
  CHAMFER COINCIDENTs; Offset's 2nd click must not open the box (where is live's label?); ASM-MC top corner takes the
  VERTICAL edge (Z -z), unlike probe-mcq's spline end: reconcile (TICKETS 50).
- Scripts: tmx.sh (scene look + ps-ui hover_matrix::sk_ + glyph T1 + clippy), ttrim.sh (sketch + scene lib + clippy).

## NOW (11-28 08:45 UTC)
- QUEUED (land.sh, no results yet; each carries the earlier ones): 75280e34, dd0fa834, 8728537c, ff59d97f, ab49ca2c
  (Mirror before its line passes over the Origin), 3196c652 (a free line's end Coincident with a used/fixed edge carries
  the whole line = SKETCH-COINCIDENT-FREE-LINE-SLIDES), then: midpoint start on an older held line is Vertical
  (SKETCH-LINE-ON-LINE-INFERENCE: live's rule = Perpendicular only when no held line along it was drawn after its
  Parallel/Perpendicular group), spline derivative T1 (SKETCH-SPLINE-END-DERIVATIVES: the diff was the old state export;
  ours = live), Enter in Display states' name box saves (ASM-DISPLAY-STATE-CREATE), Create Part Studio in context takes
  the clicked origin glyph (ASM-PART-STUDIO-IN-CONTEXT-ORIGIN). skasm-main: 3bb2265e + 643348c2. Fix forward on any fail.
- TICKETS: union-merge duplicates dropped; checked against qa's 1128-0410 replay: ASM-MEASURE-BRACKET-KEY,
  REPLICATE-MATED-PIN-ROW, MATE-ROTATION-SIGN, PATTERN-CENTERED-ORDER match. Batch 6 rows "to verify by replay".
- Blocked/parked: SKETCH-TRIM-COINCIDENT-AT-CUT (needs live's constraint entities), PATTERN-EMPTY-CLICK (REQUESTS 4),
  OFFSET-DISTANCE-FROM-CLICK (live's preview side after the 2nd click), SLOT-ON-SPLINE 3 OFFSET (spline-spline OFFSET),
  TANGENT-ARC-SPAN (complex), ARC-ENDPOINT-ORDER (P3), ASM-DRAG-REVOLUTE-ANGLE (batch 6, verify), ASM-MC-* (50/51).
- REPLAY bn02so8gg waiting in the vibex8 slot (qa's 471-scenario shards since 06:29): rclick-line-circle-menu,
  horizontal/vertical-key, 3pt-arc-reversed-sides, slot-on-arc + 19 sketch-used retakes (out ~/tmp/skasm/uip). After
  the lands: asm replay (mistake-asm-spline-mate-same-part, asm-mc-on-spline-face, display-states-create, in-context).
- Live REQUESTS 1-4 (skasm 06:5x) answered 08:39 (live/rq-1128): see the 10:0x section.
- Hosts: builds + crate tests on the Mac (rb.sh NOSHAPE_RB_HOST=mac, writes only in ~/noshape-host, CLI only); replays on
  vibex8 via ~/bin/slot (uipr.sh). Mac clippy's 6 cocoa deprecation warnings are vendor/: not ours.
- Scripts /root/skasm-scratch/bin: tal.sh (scene + ps-ui sketch ui), tinf.sh (sketch + scene + ps-ui filter), tds.sh
  (assembly-ui filter + clippy), tve.sh, tasmui.sh, tdoc.sh, uipr.sh, used.txt, cmp.py SCENARIO (newest qa replay vs
  live: dialog/focus/selection/tooltip diffs per step).

## NOW (11-28 05:55 UTC)
- LANDING (skasm, re-queued after 29a170e1's handoff rebase conflict): Q on a mixed selection all construction
  (construction_from_selection), refused dim text selected + 2 Escs leave Dimension, alignment hold at BAND_HOLD_PX 8
  (sketch_mode.rs held_snap: is_alignment(prev); live rclick-line-circle-menu: circle centre 9.4 px off the origin's level
  let go). T1s green (scene 279, ps-ui sketch::ui_tests 609, clippy).
- QUEUED 3bb2265e (skasm-main): a B-spline face's corner = its straight side's end, new Inference::VertexEnd (edge end, Z
  back into the edge; implicit.rs), "Edge of", Z +z at (40,30,0) as live (assembly 18 + assembly-ui 240 green). OPEN: ops
  CONNECTOR_DOC / error text don't list vertex_end (needs UPDATE_DOCS regen of AGENT_API.md).
- REPLAY running (bg, vibex8 slot, ~/tmp/skasm/uip): rclick-line-circle-menu, horizontal/vertical-key, the 19 sketch-used
  retakes (qa: verifies the used-pick fix). Then: the asm replay (13/21 before; 6 should flip with 3bb2265e).
- Offset/slot replay (05:0x): 3/8. offset-open-chain: live's 2nd click (10,-4), 8 px off the line, does nothing; ours hits the
  distance label (box "5 mm"): needs live (flip arrow?); TICKETS SKETCH-OFFSET-DISTANCE-FROM-CLICK annotated. Rest click noise.
- Batch 6: TRIM COINCIDENT short (needs live's constraint list). Parked: SKETCH-DIM-NEGATIVE-KEPT. Next: replay results,
  asm replay, HORIZONTAL-RANGE, ASM-MATE-ERROR-ROW.
- Scripts /root/skasm-scratch/bin: tal.sh (scene + ps-ui sketch ui), tve.sh (assembly + ops refs + clippy), tasmui.sh, uipr.sh.

## NOW (11-28 04:00 UTC)
- QUEUED 1249059f (skasm, 11 commits incl. batch 6 43a610f3; c6081900 FAILED 03:48 on use_on_a_shelled_boxs_rim_uses_its_
  outer_loop: batch 6 let an edge in reach beat a lit face on the Use press; now only a SketchRegion (fix 30abeafc)). Also in
  it: a circle's Offset follows the pointer (offset_follow in sketch_mode.rs; a line's keeps the preview, workflows-1122 22
  s18), Slot on a spline (shapes.rs slot_around: offset sides + caps, 1 OFFSET where live has 3: solver has none between
  splines). T1s green (sketch, scene 278, ps-ui use_/rim/offset/slot/used 27, clippy).
- QUEUED 69a56fd5 (skasm-main): an edge midpoint on a B-spline/Bezier/Extrusion face runs along that face's loop
  (align = the face; resolve_connector_aligned Face arm uses edge_orientation_in). Fits live's 3 readings (Fillet 2 edge on
  the spline face -y, top face's fillet edge +y, spline bottom +y). OPEN: the curved side's top corners (no live data).
  LANDED a9e00c4f, 572c7b23.
- Replays (both bg rb.sh runs, waiting in vibex8's slot behind bool's gate + live's exchange test, as of 03:45):
  bvo7yas7q = 6 sketch-used-* + 8 batch-6 (from skasm), b2jdtmu2r = 21 asm-spline-*/asm-mc-* (from skasm-main). Both write
  ~/tmp/skasm/uip on vibex8 (serialised by the slot; compare lines print in the task output). If gone after compaction:
  `ssh vibex8 ~/bin/slot --status`, rerun uipr.sh. NEVER call `slot --help` (it queues "--help" as a job).
- Third replay queued 04:05 (bg bokbi5vff, from skasm): sketch-rclick-line-circle-menu + 6 sketch-offset-* + sketch-slot-on-spline.
  HORIZONTAL-RANGE analysis: live's click (50,5) is 9.3 px off the horizontal through (30,0) on screen (iso, 3.26 px/mm
  along x): no snap is right; our off_line metric looks right, but ours ends at (50.43, 0): the x shift says another snap or
  path (pointer_rests wake without ctx.screen?). Read the replay's step 9-10 state first.
- Next: replay diffs; SKETCH-INFERENCE-HORIZONTAL-RANGE (above);
  OFFSET open bits (picked-curve look, open chain's Escape); batch-6 P3s; ASM-MATE-ERROR-ROW (P3).
- rb.sh syncs each worktree to ~/agents/<worktree>/src and cds there: scripts carry no cd. Crate T1s via rb.sh without
  ~/bin/slot; slot only for replays/suites. Scripts: /root/skasm-scratch/bin (tasm, tso, tuse, uipr, tb6).

## NOW (11-27 19:50 UTC)
- QUEUED (lead reports): 1748d8ca (skasm-base, ASM-MISTAKE-ERRORS), fdd7c592 (spline Horizontal). Landed: f06aa0b2.
- A/B NOT RUN: the abrun.sh task hit the 2 h background limit at 20:2x still waiting for vibex8's slot (qa's 17:48 suite held it). Restart it step by step after the tests (each abb.sh / abs2.sh call its own background run; skasm-main is on branch skasm-snap-wake2 or detached main, check before building): ~/tmp/skasm/ab-base/score.log vs ab-new/score.log on vibex8; land skasm-snap-wake2 only if new >= base; then rm ~/tmp/skasm/ns-base ns-new.
- UNCOMMITTED in worktree skasm (on top of 39eb9a022a), all with T1 tests, waiting for vibex8's slot (runs: t8s.sh revolute test, tsc.sh "" scene lib, tsk.sh sketch + part-studio-ui the_parity_state_lists_sketch_geometry):
  - ASM-DRAG-REVOLUTE-ANGLE: assembly_view.rs drag start: instance_dof 1 + a Revolute on it -> plane_normal = the mate frame's z (live rev-drag-1127; fit: centripetal angle in the plane square to the axis). T1 a_part_on_a_revolute_is_dragged_in_the_plane_square_to_its_axis. Then replay asm-drag-revolute-turns + live/skasm/rev-drag-1127; close one of the 3 duplicate TICKETS rows.
  - SPLINE-END-DERIVATIVES: our curve already is live's (natural cubic, centripetal params: matches live to 0.01 on 3 takes); only the state reported handle offsets. ui_script.rs now reports nurbs::handle_ends (made pub). Test assert (40,60)/(40,-60).
  - DELETE-CORNER-KEEPS-LINES (delete_selection drops line/arc end points), RECT-ZERO-SIZE-SECOND-CLICK (first corner kept), DIM-ZERO-ACCEPTED (line Length takes 0), SLOT-ON-CIRCLE-REFUSED (circle/ellipse: picked, next click "A slot could not be created for the selected curve."), CHAMFER-CORNER-POINT (sketch tools.rs: sharp stays a plain point, legs on own start points coincident; onshape_ux test expectation corrected per live state), DIM-SAME-ARC-TWICE (picked curve clicked again unpicks), SPLIT-CLOSED-TWO-POINTS (Sketch::split_closed + Split asks "Select a second point to split closed curve."), CIRCLE-RIM-ON-LINE-TANGENT (rim at the perpendicular foot on a line within 6 px, not near its ends: TANGENT).
  - TRIM-COINCIDENT-AT-CUT: test a_trimmed_rectangle_edge_ends_coincident_on_the_crossing_line is a probe (point_at_cut already adds one); if it passes, the live 6 vs ours 5 is elsewhere (origin coincident?).
- PARKED for live captures (REQUESTS.md "skasm 11-27 19:44"): 3PT-ARC-SEMICIRCLE-SNAP (live snapped 14.5 px off vs arcmid 7.5 px), PATTERN-EMPTY-CLICK-COMMITS (click-commit commits, two-entities not), MIRROR-ABOUT-POINT (our pick passes the point to the axis behind; live copies onto the circle with SYMMETRIC).
- After the tests: debug build + uip3.sh replay of the batch-6 takes (sketch-spline-*, slot-on-circle, mistake-delete-rect-corner, rect-zero-size-click, dim-typed-zero, dim-arc-angle, chamfer-too-large, split-circle, circle-rim-on-line, trim-rect-edge-at-crossing) on vibex8, then commit + land.sh.
- Remaining batch-6: SLOT-ON-SPLINE, INFERENCE-HORIZONTAL-RANGE, OFFSET-DISTANCE-FROM-CLICK (3 takes); P3s SLOT-CONSTRAINTS, ARC-ENDPOINT-ORDER, DIM-NEGATIVE-KEPT, DIM-INVALID-TEXT-SELECTED, Q-SELECTION-LEAVES-TOOL. Then Fillet 1 + Move extension, SKETCH-LINE-ON-LINE-INFERENCE, ASM-DISPLAY-STATE-CREATE, ASM-PART-STUDIO-IN-CONTEXT-ORIGIN.
- live 11-27: batch 7 (spline-fillet-asm, 33 takes) and 7b (asm-import-step-*, 13 takes) queued; behaviours "Depth batch 7"/"7b".
- Lead 11-27: SM-LOUVER-PATTERN-SLOWER / KNIFEHANDLE under the 11-18 bar, not P0. Hosts: skasm on vibex8 via ~/bin/slot only.
- Scripts (/root/skasm-scratch/bin): uip3.sh <names>, t8s.sh <assembly-ui test>, tsc.sh <scene filter>, tsk.sh <part-studio-ui test>, t7b/t8b/t9b.sh (skasm-base), abb.sh/abs2.sh (A/B).

## NOW (11-27 06:3x UTC): Fillet 1 (knifehandle) diagnosed, waiting on live (REQUESTS 06:26 KNIFEHANDLE-FILLET1-TIP). The jump checks are NOT the cause: with all checks off (SKJUMPGO) the loft is "inside out". Stations are uniform in edge parameter (pstep 0.65-7.4 mm), so the big centre steps are sparse stations, not jumps. Real geometry (SKN=296 dense run, /root/skasm-scratch/f1st2.txt: p, centre, gap, contacts t0/t1 per station): (1) near the tip the edge bends at R~3.4 mm < ball offset ~5 mm: the centres run BACKWARDS (swallowtail) at y -6.5..-9.3 and +6.6..+9.3; (2) at y -10.9 and +10.6 the contact on support 0 JUMPS 4.3/5.6 mm in a 0.25 mm step (ball touches face 0 at two places: top z~11.3 vs side z~8.9) = a crease; live has 6/10/7. Fix needs live's face layout (trim/crease). Core OK'd jump-check edits (checks are in ball_tools; core WIP core-rf changes the incremental one: keep it reading centres, room for 1 longer end step). Debug: f1dbg.patch + SKST/SKAREA/SKN prints, bin/khblend.sh. SKETCH-LINE-ON-LINE-INFERENCE: 1 mm asked live (REQUESTS 06:12); 3/12 mm: ours' line 4 ((-3,-10) on rect top edge to (-3,0)) gets COINCIDENT+HORIZONTAL+PERPENDICULAR+VERTICAL, live without VERTICAL (likely pt-pt vertical with line 3's start x -3); REPRODUCED headless (/root/skasm-scratch/lineinf-probe2.patch, Esc = ed.cancel(), the runner's 8-step approach at 0.27559 mm/px): the approach to line 4's start passes 6.8 px from line 3's start (-3,10) -> our snap (7.5 px, no rest) wakes it -> line 4's START takes VERTICAL with it. Live's recorder (live2-scratch/uip/run.mjs) sends the 8 steps back to back; our ui_script sleeps 20 ms per step; live feel 11-18: a 10 ms rest wakes, a fast pass doesn't. Decision asked (lead 06:5x): app wake needs the snap held >= ~10 ms + runner steps back to back?
- LEAD 11-27 06:1x: knifehandle Fillet 1 is MINE (blend parked, core busy with the impeller); coordinate with core before touching blend_section.rs (core edits it now). After compaction: 1) Fillet 1 rolling balls near the tip, 2) SKETCH-LINE-ON-LINE-INFERENCE after asking live about the 1 mm contradiction. Fillet 8 stays parked.
- LEAD 11-27 06:4x APPROVED (do after compaction): (1) snap wakes a point only after it is held ~10 ms (sketch_mode.rs pointer_move's `snapped` wake; wake on the next move/click if held >= 10 ms) + a T1 (the probe in /root/skasm-scratch/lineinf-probe2.patch -> a real test), and update psui's docs/parity/interaction-feel.md row "Alignment with another point"; (2) crates/app/src/ui_script.rs approach moves back to back (drop the 20 ms sleep between the 8 steps, as live run.mjs). psui told 06:4x (owns ui_script.rs); psui's 17dc749b6 (not landed yet, within the hour) adds a `rests` closure at the approach's start: rebase onto it and keep it. Before land: rerun the full Part Studio + sketch shard and an assembly sample (the count must not drop). Fillet 1 waits for live (REQUESTS 06:26).
- STATE 11-27 08:08: commit 'skasm: a snapped point wakes ... back to back' in the worktree (not landed): POINT_WAKE_HOLD 10 ms (a click on the point wakes it at once: the bracket UI test), ui_script.rs approach + drag approach back to back (psui's 17dc749b6 `rests` not on main yet: rebase keeps it), feel row 101 updated. Green on vibex7: scene 269, part-studio-ui 658, app 54+5. RUNNING vibex8 (restarted 09:14, task btp04pcm9; check ~/tmp/skasm/ns-new mtime > 09:14 and ns-base exist): bin/abrel.sh with the current wake-code.patch (click fix included); rerun if lost, then bin/abshards.sh <bin> <out> ~/tmp/skasm/ab-names.txt (1138 PS+sketch + 53 asm sample) on ns-new, then the non-matches on ns-base; count must not drop; then land.
- queued (qa 11-27 08:1x, TICKETS 5073bdabd, suites/uipdep4-1127): P2 ASM-REPLICATE-ADDS-MATES (live no mates for replicated pins; ours Revolute 2,3), P2 ASM-PATTERN-SECOND-DIRECTION-COUNT (3x3: live 10 instances, ours 7), P3 ASM-PATTERN-CENTERED-ORDER (live <3> 60, <4> -60), P2 ASM-MISTAKE-ERRORS (realign to other instance's edge: 'Mate connector cannot be defined on multiple instances.'; Replicate nothing picked: 'A target scope must be selected.'; mistake-asm-replicate-no-other-hole HANGS after the check in ours: first).
- WIP 11-27 10:5x (UNCOMMITTED in worktree, on top of the snap-wake commit 5eed842c7): (a) ASM-REPLICATE-ADDS-MATES: assembly_ui_state.rs state mates skip generated ones + T1 a_replicates_own_mates_are_not_among_the_assemblys_mates; (b) ASM-PATTERN-SECOND-DIRECTION-COUNT: app-core quantity_field.rs PressEnter counts typed-over text as a change (T1 enter_after_typing_the_same_value_applies_it_and_keeps_the_pattern_open) + Second direction checkbox puts focus on its Direction when first filled (assembly_panels.rs `moved`); (c) hang + ASM-MISTAKE-ERRORS part 2: accept_pattern adds an unscoped/unmatched Replicate in error (unscoped_replicate helper; matched Err -> Some(empty) for Replicate), eval.rs records matched_targets errors for empty Replicate/part patterns in pattern_errors, analysis.rs scope-empty error = 'A target scope must be selected.', state rows list patterns with no copies (before Mate features) with error state; T1 extended a_replicate_without_its_target_scope_keeps_its_row_red. RUNNING vibex7: bin/t4.sh (app-core, assembly-ui, assembly --lib, part-studio-ui; started BEFORE the focus change: rerun assembly-ui). Then replay the batch-4 asm scenarios with bin/uip2.sh (asm-replicate-parts-block-ok, asm-linear-pattern-two-directions-ok, mistake-asm-replicate-no-other-hole, mistake-asm-replicate-features-nothing). Open: ASM-MISTAKE-ERRORS part 1 (MC realign to other instance's edge error), ASM-PATTERN-CENTERED-ORDER, opposite-direction button ('Opposite direction is not shown' in asm-*-pattern-*-opposite/flipped-ok).
- live 11-27 ~10:3x answered KNIFEHANDLE-FILLET1-TIP (REQUESTS ~7222, live/skasm/knifehandle-fillet1-tip-1127/out.json; Handle 6/10/7): no crease edges; fillet = 4 B-spline faces: main band 487.873 mm2, tip cap 118.459 (y +-8.22, z 6.13-11.41), two 3-edge corner faces 16.270 / 16.311; every vertex/edge in out.json. Next after the asm tickets. SKETCH-LINE-ON-LINE (i)-(iv)+1 mm control being recorded.
- queued (qa 11-27): P2 SKETCH-LINE-ON-LINE-INFERENCE (TICKETS acbf0b6b4): Line started on a line off its midpoint, drawn perpendicular: 1 mm off live VERTICAL (ours PERPENDICULAR); 3/12 mm off live COINCIDENT+PERPENDICULAR only (ours adds VERTICAL). tools/ui-parity/run.py sketch-line-started-on-line-off-midpoint-{1,3,12}mm
- queued: ASM-DRAG-REVOLUTE-ANGLE answered (REQUESTS ~7090, live/skasm/rev-drag-1127): no path dependence (150->40 = straight +40 = -21.95; 150->80 -33.96); r60 -29.23, r100 -37.04, r150 -41.26; up u30 +6.24 u40 +7.84 u60 +10.56 u80 +12.81; down 40 -79.18; left flank +40 -22.71, -40 +20.61. r10-r30 read 0.00 (recorder artefact?; 2 px retakes to come in the same answer)
- queued (qa 11-27, TICKETS 22f824cc5, P2): SKETCH-CHAMFER-KEEPS-CORNER-POINT (live 8 ent/17 cons, ours 7/15), ASM-DISPLAY-STATE-CREATE (typed name not in field, Enter creates nothing), ASM-PART-STUDIO-IN-CONTEXT-ORIGIN (origin click not filling the field; check mark does not open the new Part Studio). run.py sketch-chamfer-finished asm-display-states-create asm-create-part-studio-in-context-finished
- 0b33ec60 (pattern "No merge scope selected." + a WRONG Up to part far-side rule) was CHECKING 02:06. Fix forward in
  the worktree: features.cpp op_prism_until leaves out pieces reaching the long prism's end while one stops at the part
  (Part 3 500 mm³; gap case fails); extrude.rs far_side reverted; T1s + e2e pass (vibex8 02:4x). Corpus A/B queued on
  vibex7 (abp.sh upto2.patch = features.cpp + extrude.rs; rerun 03:2x as task bjzr3476d with the 2 h limit after the 30 min one killed the first; build.cpp/core.rs already parked in extend-straight.patch, `git apply` it after the land). Then suites, land
  ONLY those files (+ tests, TICKETS, handoff): park build.cpp/core.rs first (saved: /root/skasm-scratch/extend-straight.patch).
- KNIFEHANDLE-LOFT1 found: live's Extend handle (maintainCurvature false) = Loft 1 ruled straight on along its v
  derivative (B(u,v) = A(u,0) + v A_v(u,0), exact to 1e-15 in live's net), new edge 20 mm along each ruling (live's edge
  midpoint matched to 0.002 mm). Ours: OCCT ExtendSurfByLength (curved, cut at one v) 69.1307. Live's own Loft 1 edge
  is P1 exactly (135.1709 = our 2 x 67.5857); live's reported lengths run ~0.015 short (67.5691 for 67.5854).
  extend-straight.patch (build.cpp extended_straight + extend_face_at straight path; T1 core::a_moved_boundary_runs_straight_on):
  Mutual trim 1 now matches (31/32), BUT Fillet 1 fails ("Failed to fillet selections.", 15 s; was OK): likely the
  C1 joint (curvature jump at v=0, as live's) under the fillet's trim edge. Next: find where the fillet fails (no
  "fillet" kernel dump with NOSHAPE_DUMP_OPS, and RUST_LOG=debug prints nothing in the parity child: put an eprintln
  of the kernel error in detail.rs blend_failed, or a probe on the extend_face_at-42 dump + Mutual trim), e.g. split the
  face at C1 knots for OCCT and join back. Our loft's end tangent
  is up to 0.04 deg off live's (khtan.py): straight on our loft = 69.1754 vs live's surface 69.1591.
  Scripts: khcmp.py, khtan.py, khmid.py, kstep.py (STEP B-spline surfaces), bin/kx-ns.sh, bin/khf1-ns.sh; dump in
  /root/skasm-scratch/khdump.
- hunting-knife Fillet 8 (16/36/22, live 15/37/25): the fillet is between Loft 1 (face 213 in parts-50.step) and the
  Revolve 1 sphere butt; edge at Extrude 10 one closed edge as live (81.399, live 81.385). fillet_split_closed splits
  it in two -> 2 faces (live 1). Live's loft-side boundary is 6 edges, split at 1.056/3.734/31.796 mm from the bottom
  = our loft's double knots with a curvature jump (±1.022/3.621/31.594 on the rim; hkknots.py), not the smooth one at
  the bottom (0.5); but ours also has one at ±38.73 (jump 0.076|0.173) where live has none. Asked live:
  HUNTING-KNIFE-FILLET8-SPLITS answered (live/skasm/knife-fillet8-1127): live's loft has every interior u knot double
  and curvature jumps at all five span-group boundaries 0.0214/0.0593(small)/0.0719/0.3888/0.4674 (livek.py), but
  splits only at 0.0214/0.0719/0.3888; not a curvature, ratio or radius rule (Fillet 8 r = 1 mm). Rule unknown:
  parked (P2). Next idea: the 3 splits/half = a 6-entity profile (bottom, 2 sides x 2, top; Sketch 13 has 6 segments,
  unchecked which sketch Loft 1 uses) while 0.0593/0.4674 come from the other profile. Dump: /root/skasm-scratch/hkdump (bin/f8dbg-ns.sh).
- SURF-INTRO-CAR answered (behaviours "four geometry requests", 11-27 02:25): edge-by-edge in
  derived/b7635b82…/live-edges.jsonl; Fillet 1 end arc 24.8077 mm, side line top z 15.206.
- vibex7 holds ~/.slot-quiet 02:00-04:00: heavy runs then on vibex8 (~/tmp/skasm/corp/models has knifehandle only).
- skdbg-parity.patch applied in the worktree for the dumps: revert crates/ops/src/parity.rs before committing.
