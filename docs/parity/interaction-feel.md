# Interaction feel (psui, from 2026-11-12)

How picking, hovering, snapping, selecting and dragging feel, number by number, against live Onshape. User-approved
P1 (lead 11-12). Live measures the 3D side (live/feel/3d.json, behaviours.md "feel (3D)"), live2 the sketch side
(live/feel/sketch.json, "feel (sketch)"); psui keeps this list, ours, and a test per number.

**Rules**
- Every number has a test that runs on every build, at a device pixel ratio of 1 and 2:
  - offscreen (crates/render/tests/offscreen.rs) or GPUI without a GPU where it can be (T1);
  - through the real pointer and the GPU's own pick otherwise (part-studio-ui `feel_tests`), under the GPU lock; they
    skip, saying so, where no adapter or no working readback exists (CI's Linux runner, macOS CI's white readback).
- Distances are CSS px on screen unless stated; "reach" is the largest offset that still lights / snaps / takes.
- Status: **=** matches live, **≠** differs (fix queued, biggest feel impact first), **?** live not measured yet,
  **–** ours not measured yet.

## 3D (Part Studio and assemblies, outside sketch mode)

Live: live/feel/3d.json and behaviours "feel (3D)" (11-13 02:10; DPR 1, two zooms, the same in px). Onshape's client:
`MOUSE_PICK_RADIUS_PX` 5, the pick a 10 × 10 px square round the pointer (×DPR, so CSS px at any ratio), sorted by
priority, then the front-most, then the nearest.

Reach, hold and priority are one table-driven offscreen test, render `the_3d_pick_matrix_matches_live` (the pick
buffer as the viewport picks it, at DPR 1 and 2; each target measured both ways and averaged, as where it sits in its
pixel moves one side a pixel against the other, live's too): **matched 15 / 15** (11-13 06:02 UTC); box select 3 / 3
(part-studio-ui `feel::box_select_takes_what_live_takes`); presses 5 / 5 (app-core `presses_drag_and_open_the_menu_as_live`,
GPU `feel::presses_orbit_box_and_open_the_menu_as_live`).

| Item | Live | Ours (where) | Test | Status |
|---|---|---|---|---|
| Pick area | a square of 5 px each way | the same (`PickBuffer::pick`, `PICK_RADIUS`) | matrix | = |
| A part hidden behind another, selected or hovered | its orange silhouette (selected) or light-orange outline (list-row hover, a face's hover) is drawn through the part in front; the fill only where it shows (live2 11-14, feel/hidden-part-selection.png) | the same: selected, `outline_through` on the part's meshes (studios' renderer flag); hovered, the hover mask takes the part's whole image, depth ignored | GPUI `a_selected_part_outlines_through_parts_in_front`, offscreen `a_hovered_part_is_outlined_through_the_part_in_front` | = |
| Priority | sketch point > sketch line > mate connector > vertex > edge > origin > sketch region > face > plane; the nearest within it | the same (`PickKind::priority`) | matrix (a sketch line over a part edge, a sketch point on a vertex) | = |
| Edge (over the background or its face) | lights at 7 px; stays lit to 9–10 (retaken 11-18 16:29, live/feel/reach-1118; 6–7 on 11-13) | 7; 10 (a line picks at least 5 px wide, 11 hovered itself, was 9: held to 8 in live's recheck; the shaders' `picking`) | matrix; GPU `feel::a_click_beside_an_edge_selects_what_is_lit` | = (fixed after live recheck 11-18: was lit at 7 =, let go at 9) |
| Vertex | 7 px across, 9 diagonally; stays lit to 7–8 | 7 / 9; 7 (a point picks at most 5 px wide) | matrix | = |
| Sketch line / point, not editing | 7, lit to 9–11 / 7–8, lit to 7–8 (retaken 11-18; the line 5–6 on 11-13) | 7, 9 / 7, 7 | matrix | = |
| A part in iso with the planes behind and through it (user P0 11-17) | the part lights, never a plane behind it: each face's middle (a face, or a seen edge by it), a top corner 5 px out its vertex; beside it a plane | the same | GPU `feel::part_faces_and_vertices_light_over_the_planes_behind` | = |
| A block wholly behind the Front plane (live psui/hover-pick-1120, 11-17) | a transparent plane never wins over geometry behind it: every face's middle lights the block; an edge with only the plane beyond lights at 5–6 and holds to 9 from the plane side, 4 / 8 from its face; the plane lights only where nothing is behind it | the same (planes see-through in the pick, priority 0) | GPU `feel::a_block_behind_the_front_plane_lights_through_it` | = |
| A sketch point vs its line, not editing (live hover-pick-1120 row 2) | moving along the line inward the point takes over 8 px before the end, the line back at 9; beyond the end / across 6–7 | the point's square: 7 along, 7 across | – | ≈ (1 px along the line) |
| Plane border | 7–8, no hold | 7, no hold (`LineSegment::WIDE_PICK`, `NO_HOLD`) | matrix | = |
| Assembly instance edge / vertex | 6 / 7, no hold | 6, no hold (`NO_HOLD`) / 7 | matrix | = |
| Silhouette edge | lights at 7, stays lit to 9–11 (retaken 11-18) | as an edge: 7, 9 | – | ≈ |
| Mate connector | its triad and ring: 22–23 px across, 14–15 down on a face's centre; on a vertex 22 across, 16–21 down, and it wins over the vertex throughout (retaken 11-18) | drawn at Onshape's MATE_CONNECTOR_DIAMETER_PX 36 (`MC_R_OUT_PX` 18) with the 5 px pick: 23 across, ~15 down in the iso view | – | ≈ |
| Vertex vs its edge; edge vs edge 5.6 px apart | the vertex anywhere in its reach (a lit edge holds against it to ~4 px); the lit edge holds 3–4 px past the other | the vertex in its reach; the held band wins by distance | – | ≈ |
| Hover timing | a pick on every move below 0.15 px/ms; faster: after a pause of min(5 × the average pick, 250 ms) | the same (`prehighlight_delay`) | app-core `prehighlight_waits_for_a_pause_as_live` | = |
| Edge midpoint target after a rest | live 1954: 9 px square, a short dwell; a shown sketch's line with no tool open: never its midpoint, the line lit across its middle (live2 rr/ln.png, qa FEEL-SKETCH-REACH-OUTSIDE) | `update_hover`, `MIDPOINT_DWELL`; a sketch curve arms its midpoint only with a feature dialog open (was: always) | existing midpoint tests; sketch_hover `a_shown_sketchs_line_and_point_reach_outside_sketch_mode_as_live` | = (fixed) |
| Click | adds; on a selected entity removes; on empty space clears; Shift+click on empty space keeps; Shift+click on an entity as a plain one | the same (`on_viewport_event`'s Click) | – | = |
| Ctrl+click | the context menu of what is under the pointer, selects nothing (every OS; on a Mac Cmd+click is a plain click) | the same | – | = |
| Double-click | a face: shows its feature's sketch drawn as an edited sketch looks and through the part (feel/3d dblclick-face.png: 2 px blue, its corner dots, the back base line seen through), the two clicks select then deselect, its feature and sketch rows related until the next click (an empty click clears both: live2 11-17 sel-feel 2, 3); an edge: nothing | the same (`sketches_shown_for_now`, `SceneOptions::through_sketches`; the next click clears `dblclick_related`). qa 11-18 on 591643fc: drawn depth-tested, 1 px in the inactive colour, it vanished into Part 2's base edges; qa on 850ae7f3: through but grey (a sketch not edited carried no constraint states), its back dot hidden: now its solve's colours (blue under-defined) and its dots on top too | part-studio-ui `a_face_double_clicked_shows_its_features_sketch`, GPU `a_face_double_clicked_draws_its_sketch_through_the_pointer` (two-blocks, Part 2), model `a_sketch_not_edited_carries_its_constraint_states` | = (fixed 11-18) |
| Select other (`) | the "Select other" panel at the pointer + (100, 51), 220 px wide (live2's chrome.json) | `SelectOther` | chrome matrix "select other / its place", "its size"; list `select_other_row` | = |
| Box select | left → right: window (faces and edges wholly inside, hidden ones too, no vertices, no part: 18 round a block, 5 round its right half); right → left: crossing (13 over that half); adds; Ctrl at the start deselects | the same (`box_ids`) | `feel::box_select_takes_what_live_takes` | = |
| Left press → drag (box) | more than 2 px and 50 ms after the press | the same (`press_becomes_drag`) | app-core, GPU feel | = |
| Right press | orbits from the first pixel; the menu still opens on release within 4 px and 1 s | the same (`right_release_opens_menu`) | same | = |

## Sketch (editing a sketch)

Live: qsnap (behaviours "sketch snapping and hover", 11-12) and live2's "feel (sketch)" (11-13 00:55, live/feel/sketch.json;
two zooms, A 0.2754 / B 0.2316 mm/px; where they differ both are given).

| Item | Live | Ours (where) | Test | Status |
|---|---|---|---|---|
| "Orient normal to sketch on edit" (live2 sketch-entry-1118) | editing a sketch from iso turns the view to its normal; ✗ leaves it there | turned, then ✗ went back to the view before; now it stays | sketch_ui_tests `orient_on_edit_turns_to_the_sketch_and_exit_keeps_the_view` | = (fixed) |
| A line's end snaps onto a line | ≤ 7 px, not 8 | 7.5 px (`INFERENCE_PX`) | inference tests; GPUI `a_line_ends_on_the_hovered_point_at_scale_1/2` | = |
| …onto a point / a centre | a 15 px square: 7 px straight out, 9–10 diagonally (catalogue); onto a centre: COINCIDENT, not CONCENTRIC | the square on screen | matrix | = |
| …onto a midpoint | ±5–7 px along the line | 6 px (`MIDPOINT_REACH`) | scene `a_snapped_point_or_midpoint_gets_the_square_and_a_curve_none` | = |
| Horizontal / vertical to the line's start | 15 px bands: 7–8 px each side (catalogue; live2's first take: 7 / 6) | 7.5 px (`inference::HORIZONTAL_REACH`, `VERTICAL_REACH`) | sketch `lines_are_15_px_bands_and_points_15_px_squares`; scene `drawing_a_line_snaps_in_px_bands_and_shift_snaps_nothing_as_lives` (7 px at 40 / 80 / 160 / 320 px from the start, live 11-17 sketch-snap-1117: no angle rule; a woken point's vertical the same at 60 / 150 px); matrix | = |
| Alignment with another point | a point wakes when a COINCIDENT / MIDPOINT / QUADRANT snap lands on it (its wakeupInferenceSource: no rest time, a 10 ms rest or a slow pass woke it, a 30 ms rest with no snap didn't) and stays woken for the line; the origin always; the alignment band 15 px (±7) both ways (live feel retaken 11-18) | a point wakes on a snap onto it (Endpoint / Center inference, an external vertex snap), no rest (was `ACQUIRE_REST` 100 ms, fixed 11-18); the bands ±7.5 px. A line's midpoint snapped onto wakes too: the click on its alignment makes a point there, MIDPOINT on the line, the new end HORIZONTAL / VERTICAL with it (live align-mid-1118: `<new>.endSnap0`; it follows the line); live offers no QUADRANT inference on a circle, so none there | scene `a_point_aligns_once_a_snap_lands_on_it`, `a_passed_midpoint_gives_alignment_and_a_point_there` | = (live recheck 11-18 on green b5b664aaa: a pass with no rest aligns, none without the pass; midpoint sources fixed 11-18) |
| Perpendicular | through the start, to a woken line (snapped onto) and from the chain's previous segment; 360 / L px (L the line's length); live 1866 (e): also across a line whose direction a constraint holds | the same | sketch `a_woken_line_gives_parallel_and_perpendicular_through_the_start`, `square_to_a_held_line_is_perpendicular`; matrix | = |
| Parallel | through the start, to a sketch line snapped onto (woken); never before (catalogue "wakeSummary") | the same | sketch `a_woken_line_gives_parallel_and_perpendicular_through_the_start`; matrix | = |
| Tangent (from an arc's end; ending on a circle) | 6–7 px; ending on a circle within 6 px of the tangent point (10 px: only Coincident) | from an arc's end 360 / L px; ending on a circle: the true tangent point from the start within the 7.5 px snap radius along the circle, the end placed on it (was a 3° angle: ~2 px on a 36 px circle) | sketch `tangent_hint_from_arc_end`, `a_line_ends_tangent_on_a_circle_near_the_tangent_point`; matrix rows line/arc-end/tangent, line-end/circle/tangent | = |
| Two at once | horizontal + on a line both; a point snap beats horizontal; a point beats its line | the same: H/V where the axis crosses a curve near the pointer takes both (`inference::infer`) | sketch `two_inferences_at_once_as_live` | = |
| The target snapped to last | stays while anything of its rank under the pointer is on it; a point or the line's own end coming into reach takes over (live 11-18 sticky-snap, real pointer: the line stays 2–4 px under a point 12 px above it, the point from 5; a line's end lights 5.6 px before it); not forgotten when nothing is under the pointer (client WT@9146346 lastInferenceTarget) | the same (`SketchEditor::snap_target`, `Inference::targets`; reset with the tool) | scene `the_line_snapped_to_last_stays_against_its_rank_not_a_point` | = |
| An opened sketch an upstream change moved (the user's hat15b Sketch 1, 1.1 mm) | edited where it is shown: its lines light to 7 px either side | the same: the draft takes the regeneration's solve on entering, not an undo step (was the stored place: lit 30 px to one side, none 3 px to the other) | part-studio-ui `an_opened_sketch_is_edited_where_its_solve_shows_it`; checked in hat15b | = |
| Two at once, shown | perpendicular × a lit model edge (live2 combo-perpEdge, 11-14): ⊥ then coincident, 20 px boxes 2 px apart, the first +15 / +25 px from the pointer (a single glyph +20 / +20); the order is the created constraints' (live2 sketch-snaps.json "combined"); a parallel never meets a model edge (parEdge: only COINCIDENT); the reference segment and the line or edge landed on lit, the rubber band dashed and a dashed guide to an aligned point; both constraints; any two zones crossing combine (a direction or an alignment × a curve or an edge, a direction × an alignment; not two directions: perpExt), the crossing nearest the pointer of several ("three") | the same (`sketch_mode::inference_icons`, `glyph_offset`, the editor's crossings; glyphs by the pointer). Glyph order: the constraints' order, as live2's batch 2 | scene `a_combined_snap_shows_each_glyph`, `crossing_zones_combine_as_live`, GPUI `a_square_to_the_last_segment_crossing_a_lit_model_edge_takes_and_shows_both`, matrix `combo/*` | = |
| 3 point arc onto a semicircle | the third click within 7.5 px of the semicircle over the ends (inside or out, anywhere along it) draws that semicircle; MIDPOINT holds its centre at the ends' midpoint (live2 arcmid: 7.5 / 6.8 px yes, 8.0 / 7.9 no) | the same (`ARC3_SEMICIRCLE_PX`, `MidpointOf`) | scene `a_three_point_arc_snaps_to_the_semicircle`; ui-parity sketch-tangent-arc-from-arc | = (fixed) |
| Snap target look | yellow-orange square, 17 px, round a point / midpoint; none on a line | the same (`SNAP_SQUARE_*`) | scene, same | = |
| Inference glyph | ≈ 30 px right, 30 below the pointer (28, 28 mid-drag); same frame as the preview (35–60 ms) | its 20 px box centred 29 right, 31 below the snapped point | – | ≈ (within live2's ±2 px) |
| A dragged end onto an end / a midpoint / a line / a circle's rim | 6–7 px every side / ±7 along the line / the 15 px band / ≈ 15 px across the rim (5–6 out, 8–9 in) (live feel retaken 11-18, 1 px steps); no H/V with its own start, but once the drag has snapped onto a point, VERTICAL / HORIZONTAL alignment to it | the 15 px square for an end and a midpoint, 7.5 px to a line and a rim, on screen (`point_drop_target`); alignment with a point the drag snapped onto (`point_drop_target`, woken on the snap; fixed 11-18 f1a1c756) | GPUI `a_dragged_point_lands_on_a_line_within_7_px`, scene `a_drag_aligns_with_a_point_it_snapped_onto` | = (live re-measured f1a1c756: VERTICAL kept after release) |
| Drags: free line end / body; held lines; dimensioned; rectangle; circle | see behaviours "feel (sketch)" (2); a horizontal line's end takes its start along, level; a line from the origin doesn't move by its body (live2 recheck) | the same (a body drag with a held point leaves the curve: `Sketch::drag`) | scene `drags_move_lines_as_live` | = (live2 11-17: a corner rectangle's corner drag keeps the opposite corner, its side drag moves along its normal: scene `a_rectangles_corner_and_side_drag_as_lives`) |
| Drags of curves and constrained geometry (live2 sketch-drags 11-17, 40 px = 10.795 mm) | circle rim resizes, centre moves; arc end keeps r and the other end (the centre moves) until the chord passes 2r, then a semicircle on it; arc middle keeps the centre; arc centre keeps the end, the start onto the new circle; a shared end takes both lines; a LENGTH line's end translates it, its start fixed it swings; a fixed point stays, no message; an ellipse's rim scales it about its centre; an arc's end released level with its other end takes HORIZONTAL (the glyph during the drag); a dragged point shows its drop's glyph (coincident, midpoint) by the pointer | the same (`Sketch::lives_drag`, the ellipse's rim target, `Drop::Level`, `snap_icons` while dragging; dragged points showed no glyph); was: the arc's radius and other end moved, a LENGTH line turned, an ellipse translated | scene `curves_drag_as_lives` (within 0.27 mm) | = (fixed; live2 run4 re-took the ellipse's minor rim (it follows the pointer, as ours) and a chain's L2 dragged by its middle: it keeps its direction and moves across itself through the pointer, the shared end sliding along L1, which keeps its direction; ours translated L2, now `lives_line_body_drag`) |
| Click vs drag threshold | ≈ 5 px from the press, not time-based | 5 px on screen (`DRAG_START_PX`) | scene `a_press_drags_from_5_px` | = |
| Hover of a sketch entity = what a click takes | a line lights to 7 px, a point to 10 (live2 11-17 hover-p0; 8 (A) / 7 (B) before); sketch-offset hits a line 3 mm off | one reach for the hover, the click and the fallback pick: 7 / 10 px (`sketch_hover_id`, `sketch_pick_at`, `in_sketch_reach`) | GPUI `a_circles_rim_lights_and_selects_the_circle_not_its_centre_beyond_reach`, `a_line_in_reach_lights_and_is_clicked_when_the_gpu_pick_misses_it`; GPU `feel::the_dimension_tool_takes_the_hovered_line` | = |
| A sketch line or point over a plane (planes shown) | the line or point always wins, whether the plane is behind or in front on screen; the sketch's own plane never hovers (live2 11-17 hover-p0) | what is in reach under the pointer is asked again on every move, also while the GPU's pick stays the plane (`sketch_hover`, `ui.sketch_gpu_hover`); the GPU pick's own priority: psui | GPUI `a_line_lights_over_the_plane_the_gpu_pick_keeps` | = (fixed 11-17; the user's recording) |
| Planes, faces, the region while a drawing tool is active | nothing of them pre-highlights, before the first click and mid-shape (Line, Circle); sketch entities, the origin and snap targets do (live2 11-17 tools-planes) | the same (`sketch_hover`: a plane, face or region pick is dropped while `Tool::draws`) | GPUI `no_plane_lights_while_a_line_is_drawn` | = (fixed 11-17) |
| Constraint from a selection (two lines) | applies at the click, the selection clears, no tool stays; the first-picked line stays and the second moves: Parallel about its midpoint, Perpendicular about where the lines cross, Coincident onto the first's line (lengths kept), Equal keeps its start and direction; H / V one per line, both lines turned level / plumb about their midpoints, lengths kept (live2 11-17 part 2b) | the same (`solve_after_constraint`: `place_second_line`, `level_lines`; `what_moves` lights what moves, the rest held) | scene `the_second_picked_line_moves_as_lives`, `horizontal_and_vertical_turn_both_lines_about_their_midpoints` (live2's numbers, 0.02 mm) | = (fixed 11-17) |
| Constraint toolbar with two lines selected (live2 11-17 part 2b, p2b2.json) | all 13 buttons stay enabled (nothing greyed); Parallel, Perpendicular, Equal, Coincident add one; Horizontal, Vertical, Fix one per line; Tangent, Concentric, Midpoint, Normal refuse in std's words; Symmetric arms its tool with both lines kept; the selection clears and no tool stays otherwise | the same (no button is ever disabled; `constraint_command`) | scene `two_lines_through_every_constraint_button_as_lives` | = |
| Sketch state while editing (live2 sketch-state 11-17) | a free line: blue, the row's blue "−" badge; Fix on the line: the line black, its ends blue (free along it), the badge stays; a HORIZONTAL on that slanted line: the row red "Sketch could not be solved.", no badge, the yellow bubble, the line where it was; deleting it: back to fixed (the bubble gone); deleting the Fix: back to free | the same (Fix on a line holds the line, not its ends; the message was "Sketch has conflicting constraints") | part-studio-ui `an_open_sketchs_state_follows_its_constraints` | = (fixed) |
| Constraint glyph selection (live2 glyph-select 11-17) | one glyph at a time: a click (Shift too) on another replaces it, Delete takes only it; a glyph click drops selected curves, a curve click drops the glyph; Shift+click on the selected glyph or an empty click deselects | the same (were additive, kept with curves) | scene `constraint_glyphs_select_one_at_a_time_as_lives` | = (fixed) |
| Constraint glyph hover (live2 sktools-1118) | its 20 px tile turns yellow rgb(252, 218, 127), its line lit; lit to 14 px from the tile's centre (≈ 4 px past its edge), off at 15–16; a continuous slide holds it a little longer (16 px), a jump there misses it | the tile #FCDA7F, its line lit; a 4 px clear margin round the tile takes the hover (`GLYPH_MARGIN`): lit to 14 px, off at 15; once lit, 6 px (`GLYPH_HOLD_MARGIN`): a slide holds it to 16 | GPUI `a_hovered_lines_glyphs_wait_by_the_pointer` (13 lit, slid to 16 held, 17 off, jumped to 15 off) | = (fixed 11-18) |
| A point and a circle / ellipse made coincident | the point fixed (a model vertex, a Fix): the curve shifts the shortest way, radii kept; the point free: the point moves onto the curve (live2 11-17, REQUESTS (d)) | the same (`what_moves`: the curve's centre or the point is what the solve moves) | scene `a_circle_coincident_with_a_fixed_point_shifts_and_a_free_point_moves` | = (fixed 11-17) |
| Hover timing (no tool) | the highlight ≈ 270–300 ms after the pointer stops; with the Dimension tool ≈ 30 ms: live's rule (live2 hover-time-1118: `alwaysPreHighlightOnMouseMove` only while a sketch creation tool is active) | the same (`Viewport::pick_on_move`, set while a sketch tool is active) | sketch_hover_feel_tests::only_a_sketch_tool_picks_on_every_move | = (fixed 11-18) |
| Nothing recoloured under the pointer mid-draw | the target line doesn't recolour | the same (`SketchEditor::mid_draw`) | – | = |
| Hovered point look | a 10–11 px translucent orange disc, centred | the same; whole on a slanted plane | offscreen `a_hovered_sketch_point_on_a_slanted_face_draws_its_whole_disc`, `a_sketch_point_is_drawn_as_lives_in_each_state` | = |
| Per-entity colour: a line | the line black, its free end blue; black once dimensioned; a construction line through the origin black, ends blue | a line by its own geometry (`line_motion_queries`) | sketch `a_line_with_a_free_length_is_defined_and_its_free_end_is_not`, `a_line_whose_end_moves_off_it_is_under_defined` | = |
| …an arc | centre and radius held: the arc black, its ends blue | by its own circle (`own_motions`) | sketch `an_arc_held_on_its_circle_is_defined_and_its_sliding_ends_are_not` | = |
| …a circle, a spline | radius free: blue, centre black; a spline blue while any point is free | the same | – | = |
| Selection: click / Shift / Ctrl | a click adds, a click on a selected one removes; Shift the same; Ctrl+click opens the context menu | the same (Ctrl+click: `open_sketch_context_menu`) | GPUI `ctrl_click_in_a_sketch_opens_its_context_menu` | = |
| Clearing; double-click | a click on empty space clears, Escape doesn't; a double-click on a line selects then deselects (no chain) | the same (two toggles) | – | = |
| Box select | left → right: window, blue fill and border; right → left: crossing, yellow dashed; it adds | window / crossing (`#789ad1` / `#ffdc74` dashed) | scene `a_box_selects_on_the_screen`, `a_box_adds_to_the_selection` | = |
| Hover, the user's setup (iso, planes behind; real pointer + GPU pick) | each rectangle line lights on it and lets go 160 px off; ±6 px in, ±9 out; a corner 8 px out is its point; inside, the region; Top never; a circle lights on its rim and ±5 px, not 12 out, its centre the centre point (live2 hov10) | the same | GPU `sketch_hover::a_sketch_line_lights_over_the_planes_behind_it_and_lets_go`, `sketch_hover_reach_and_priority_as_live`, `a_sketch_circle_and_its_centre_light_over_the_planes` | = (11-17) |
| Midpoint, glyph and plane clicks (no tool, iso) | a line's midpoint lights the line; a glyph click selects its constraint, a second replaces it (Shift adds); a click on a plane selects the plane (live2 Part 2) | the same (`toggle_constraint_selection` after a clear, editor `model_press`) | GPU `sketch_box::midpoint_glyph_and_plane_clicks_in_a_sketch_as_live` | = (fixed 11-17) |
| Clicks with the real pointer (iso) | click adds, again removes, Shift the same, Ctrl+click the menu without toggling | the same | GPU `sketch_box::sketch_clicks_toggle_and_ctrl_opens_the_menu_as_live` | = |
| Box select in iso over the planes | a press on a plane draws the box; left → right round the rectangle: its 4 lines (no region, planes or points); right → left over its right part: the 3 lines it touches | the same | GPU `sketch_box::a_box_in_iso_over_the_planes_takes_lines_as_live` | = |
| A hovered line's glyph row (Show constraints off) | placed once ~40 px off the line by the spot where the hover started, stays while the pointer slides along it; after leaving, the line's orange goes and the glyphs 0.4–0.7 s later; onto a glyph keeps the row and the line lit (live2 feel-1417 + follow-up) | the same: `glyph_row_at`, GLYPH_LINGER 550 ms with a redraw when it ends (they stayed until the next move), lingering again off any of the row's glyphs; a selected glyph keeps its row place | GPU `sketch_glyph::a_hovered_lines_glyphs_wait_by_the_pointer`, `a_left_lines_glyphs_go_when_the_linger_ends` | = (fixed 11-17) |
| Dimension label: hover, click, double-click, drag | tan / selected orange / the value box; a press and drag moves it from 5 px, the text centred on the pointer, no snap, never selected (live2 Part 2, 2b, feel-1417) | the same (`DragState::Label`; once past 5 px it follows wherever the pointer goes: it froze back within 5 px) | GPU `sketch_label::a_dimension_label_hovers_tan_selects_on_a_click_and_opens_on_a_double_click`, `a_dimension_label_follows_the_pointer_without_a_selection` | = (fixed 11-17) |
| A sketch dimension's edit box (live dimbox-1118, 11-18) | a small palette (▫▫▫ strip; ›, the 100 × 26 field, ±, ✓, ✗) at the label − (83, 18) for the field, its text all selected with the keyboard; a double-click opens it ~650–680 ms after the first click; a second single click 0.7 / 1.5 / 3 s later deselects the dimension (no box); a 20 px drag of the selected label moves it, no box, still selected; Esc closes it, "25" Enter applies | the same (the box at the label − (104, 30), the field 100 px; the OS double-click opens it; a click on a selected label toggles it off; a drag moves it without a toggle) | scene `a_selected_labels_second_click_deselects_and_its_drag_keeps_it`; GPU `sketch_label::a_dimension_label_hovers_tan_selects_on_a_click_and_opens_on_a_double_click` | = (live recheck 11-18 on green b5b664aaa: double-click opens, a 1.5 / 3 s second click deselects, a 20 px drag moves the label with no box); the value drawn selected (blue) with the window focused, as a feature dialog's field (green 3c4e4c9f3; unfocused on a bare Xvfb neither is lit) |
| Dimension tool | one line: aligned; two points: H / V / aligned by the pointer against their bounding box; two lines: the angle of the pointer's quadrant; a circle: Ø; the text centred on the pointer; the value box 100 × 26, ~33 px left of and 5 above the click | the hovered entity within reach | scene `two_points_dimension_as_live_by_where_the_pointer_is` (live2's 48 pointer positions) | = |

## Drawings (studios, from 2026-11-13)

Live: behaviours.md (lines cited) and live2's feel/drawing.json (D1–D6, F3, F4; written 11-13 03:22). Ours: px are screen px
(`tol_mm(px)` = px / zoom), mm are sheet mm.

| Item | Live | Ours (where) | Test | Status |
|---|---|---|---|---|
| View edge / inside hover reach (Select) | the whole view lights (tan frame ~8 px outside its geometry) from 14–16 px outside an edge, not 16–20; constant in sheet mm (D1) | 3.6 sheet mm round the view box (`VIEW_REACH_MM`, `view_under`) | GPUI feel `a_view_lights_and_is_taken_within_its_reach_in_sheet_mm` | = |
| Dimension line / extension line, centerline, centermark arm | 6 px, not 8 (D1; extension 4 / 6 at zoom B) | 7 px (`SELECT_LINE_REACH_PX`, `hit_reach`) | GPUI feel `a_dimensions_line_and_text_light_at_lives_reach` (dimension line) | = |
| Dimension / note text | 8 px outside the text box, not 10 (D1) | 9 px outside the box (`SELECT_TEXT_REACH_PX`, `text_box_distance`) | GPUI feel `a_dimensions_line_and_text_light_at_lives_reach` (dimension text) | = |
| Note leader | 8 px, not 10; lights the note (live2 batch 1, 11-18 16:15; 11-13 had 10 / 12) | 9 px for its stroke and nodes in Select's hover too (`SELECT_LEADER_REACH_PX`, `leader_reach`; was 11) | GPUI feel `a_note_leader_lights_its_note_within_8_px` | = (fixed 11-18) |
| Priority: annotation vs view | an extension line's foot on a view edge lights the dimension and a click selects it (3 of 3, live2 drw-feel-q (b)); a centerline over a view edge lights both and a click takes the view (D2); on the centerline 3 px inside the view, hover and click take the centerline (followUp (2)) | annotation first (−2.5 px bias); a centerline lit where it runs on a view edge gives way to the edge, which stands for the view (`centerline_on_edge`) | GPUI feel `a_centerline_on_a_view_edge_gives_way_to_the_view` | = |
| Snap reach (Dimension tool, Note leader) | 7 px, not 9, for vertices; a circle centre 9 px, not 11, on the diagonal and from inside the hole (live2 batch 1, 11-18 16:15; 11-13 had 7 / 9); 5 px along an edge; constant in px (D3) | 8 px (`SNAP_REACH_PX`), a centre 10 px (`CENTER_SNAP_REACH_PX`; was 8) | GPUI `a_views_vertex_snaps_within_8_px_on_screen` (7 snaps, 9 doesn't), feel `the_dimension_tool_snaps_to_a_circle_centre_within_9_px` | = (centre fixed 11-18) |
| Snap marker, dimension tools | a filled 10 px disc rgb(216,165,102) for every kind, none in Select, none for a midpoint (D3) | the same (`SnapMark::Disc`), also the Dimension tool's sticky centre dots (bm 4062) | GPUI feel `the_dimension_tool_snaps_to_a_vertex_within_its_reach_and_select_marks_none` | = |
| Snap marker, Note tool | a hollow square ~15 px rgb(255,130,63) round the vertex (bm 26796, C11 (a)) | the same (`SnapMark::Box`) | – | = |
| Midpoints / quadrants | only after Shift+Q (D22) | the same | – | = |
| Two points in reach | the nearer, one marker (bm 4092) | the nearer (`snap_at_with`) | – | = |
| Placement inference (note / dimension text) | 7 px, not 9; pink dotted line, red ✕ (D4) | 8 px (`INFER_REACH_PX`), the same line and ✕ | GPUI feel `placing_a_note_lines_up_with_another_within_8_px` | = |
| Box select look | left → right: light-blue fill, solid 1 px blue; right → left: pale-yellow fill, dashed yellow-orange (F3) | the same (`box_overlay`) | – | = |
| Box select, annotations | a note half inside: right → left only; fully inside: both (F3) | the same (`select_in_box`) | GPUI `a_box_right_to_left_takes_what_it_crosses` | = |
| Box select, views | left → right: a view only with a whole model edge of it inside (part of an edge, or a box past its corner: nothing); right → left: when the box meets its frame (~7 px out); a dimension left → right with its whole text, an arrowhead or an attachment point inside (an extension line's foot alone: nothing), right → left when touched; a press within a view's reach drags it (live2 box-select-rule 11-16) | the same (`select_in_box`, `BOX_VIEW_FRAME_PX` 7) | GPUI `a_box_takes_a_view_by_a_whole_edge_or_by_crossing_its_frame`, `a_box_takes_a_dimension_by_its_text_an_arrowhead_or_an_attachment_point` | = |
| Box select with modifiers | none adds; Shift: nothing changes (F3) | adds; Shift does nothing | the same test | = |
| Hover and selection look (colours, live 2068, D1) | a hovered view: a tan rgb(204,178,146) frame 2 px + rim with a 7 px clear gap to its geometry, its edges as they are; selected: its geometry orange rgb(204,146,92) at 3 px, framed in that orange once the pointer leaves; a hovered dimension: tan lines and label box, orange ends and diamonds, blue attachment dots; a selected one: orange, its ±0.1 launcher after 0.8 s (live2 drw-centermark: d-sel-view's "handle" is the hole's centre mark) | the same (`view_frames`, `push_annotation_hover`; was also a second orange frame 1 mm out from `push_aux_states`), side by side with live2's crops | GPUI feel `a_hovered_view_is_framed_in_tan_and_a_click_turns_its_edges_orange` | = |
| Centre mark hover and selection (live2 drw-centermark) | hovered (its circle, centre or an arm): the hole circle light blue rgb(144,206,241) at 3 px, the arms solid tan, 10 px orange grips at the arm ends and an 11 px blue centre dot, the view not framed; selected: the same with orange arms; its view selected: the grips and dot over black arms, the circle the view's orange; the view hovered alone: none. A click on the circle takes the mark, a press on it drags the view | the same (`centermark_of_edge`, `hole_press_view`, `centermark_looks`; automatic marks got grips; was: the circle lit the view, no grips) | GPUI feel `a_holes_centre_mark_lights_from_its_circle_and_a_press_there_drags_the_view` | = |
| Centre mark grip drags (live2 drw-centermark t1–t6, follow-up) | from hover (no selection needed): a grip outward lengthens all four arms, following the pointer; an automatic mark never gets shorter than its default (pulled short: a red ⊘, nothing changes; a lengthened one comes back), keeps its length on reload and stays automatic (Drawing properties untouched); a Single mark shortens below the default too, even inside its hole (no snap to its own circle); sideways, the centre, the arm inside the circle: nothing | the same (`auto_centermark_grip_at`, `Drag::AutoCentermarkArm`, `DrawingView::centermark_extensions`; `centermark_grip_at` from hover; op `drawing_centermark_resize`); was: automatic marks didn't drag, Single only once selected | GPUI feel `a_centre_marks_grips_drag_from_hover_and_an_automatic_one_never_gets_shorter`, `a_single_centre_marks_grip_shortens_it_from_hover`; e2e `centermark_arms_resize_as_their_grips_drag` | = |
| Edge hover in a tool (Dimension, centerlines, centermark…) | the edge alone pre-highlights orange rgb(204,146,92) (behaviours "drawing Dimension tool"; a circle and a centermark's arms too, drw/dact-aa-d3) | the same (`tool_picks_edges`: the hover colour is the orange in those tools) | GPUI feel `the_dimension_tool_lights_a_hovered_edge_alone_in_orange` | = (fixed 11-17: was tan) |
| Highlighted line width | hovered and selected lines, frames: 2 px solid with an antialiased pixel each side, whatever the line's own width (dh0 dimension line, dsel3 selected dimension, d-sel-view edges and frame) | 3 px (`LIT_LINE_PX`, `Style::lit_line_px`; was the 3D band ~4.5 px for flagged lines and 1.5 px for a hovered annotation's) | the same test (the style) | = (fixed 11-17) |
| Click, Ctrl + click, Shift + click | all toggle the item: add if unselected, remove if selected (D6) | toggle (`select_click`) | GPUI feel `click_ctrl_click_and_shift_click_toggle` | = |
| Click vs drag | a view or a dimension moves from 5 px (4 is a click), a note from 8 (6 is a click); no hold needed (D5) | 4 px past the press for views and dimensions, 7 for notes (`drag_threshold_px`) | GPUI feel `a_view_moves_from_5_px_and_4_px_is_a_click`, `a_dimension_moves_from_5_px_and_4_px_is_a_click`, `a_note_moves_from_8_px_and_6_px_is_a_click` | = |
| Box on empty sheet | shows past 7 px from the press: none at 3–7 px along an axis or (4, 4) px, a box at 8 px and at (5, 5) px; D5's "none at ≤ 8" was a late repaint (live2 drw-feel-q (a)) | past 7 px (`BOX_START_PX`) | GPUI `a_box_on_the_empty_sheet_starts_past_7_px` | = |
| Dragging a view | a view aligned with its parent follows the parent's move only across its axis (Front dragged (+40, −20): Top +40 in x, Right −20 in y); a projected view dragged across its axis takes its parent along (Right 40 px down: Front down too, Top stays) (followUp (3)) | the same (`dragged_view_positions`; was: the whole family moved, a projected view locked to its axis) | drawing `a_dragged_view_takes_its_aligned_family_as_live`; e2e `drawing_view_move_takes_the_aligned_views_along` | = (fixed 11-16) |
| Dragging a view near another's alignment | no snap at 16 down to 1 px, Shift or not (F4) | none (built 11-13, removed on live2's answer) | GPUI feel `a_dragged_view_lands_where_released_with_no_alignment_snap` | = |
| Sheet fit on open and Zoom to fit | the same fit both ways: ANSI A / B and ISO A3 paper 927.5 / 928 / 923 px of a 973 px canvas (1919 × 992 window), centred 14.75 px low; ISO A4 fits its frame (live2 sheet-fit: A3-start, B-start, the 11-15 fits) | the same (`camera_ops::drawing_fit_sheet`: 926 px, 14.75 px low); a smaller window fits smaller (the 3.45 px/mm seen 11-17 was a 1440 × 900 window) | `camera_ops::a_sheet_fits_as_live`, `an_a4_sheet_fits_its_frame`; GPUI feel `a_drawing_opens_fitted_and_zoom_to_fit_puts_it_back` | = (within 3 px) |
| Window resize (live2 drw-window-resize) | a sheet at its Zoom to fit is fitted again on every resize; zoomed or panned it keeps its width across the canvas (the canvas 1879 → 1400 px: the sheet 1300 → 969 px) and its place along it; resizing back restores it exactly | the same (`resize_canvas`; was: the zoom kept either way) | GPUI feel `a_resized_window_refits_a_fitted_sheet_and_scales_a_zoomed_one_with_its_width` | = (y not captured: ours keeps the centre) |
| Double-click | the OS's (live2's Linux: 400 ms, 2 px); a dimension's palette only within ~200 ms and 3 px (live2 11-13) | the OS's (GPUI's Linux constant: 400 ms, 5 px); a dimension's palette within 200 ms and 3 px (`quick_double_click`) | `a_dimensions_palette_needs_a_quick_close_double_click` | = (Linux distance 5 vs 2 px: GPUI's constant) |

## Documents page and tabs (studios, from 2026-11-13)

| Item | Live | Ours (where) | Test | Status |
|---|---|---|---|---|
| Documents list: click / Ctrl / Shift | click one row; Ctrl toggles; Shift **adds** anchor..row; the anchor is the last plain or Ctrl click (feel/documents.json `multiSelect`, three sequences) | the same (`click_rows`) | `rows_select_alone_with_ctrl_toggles_and_with_shift_ranges_from_the_anchor` (live2's sequences) | = |
| Ctrl+A / Esc / click on empty list | all rows (the panel shows no details) / keeps / clears | the same (`select_all_rows`, `doc-list` click) | – | = |
| Right panel for N | "⌄ N selections", the rows with ✕, Trash, Deselect all, "⌄ Labels" (docs-f1-shift-range) | the same (`render_selection_panel`) | – | = |
| Menu for N | Details, Labels…, Move selected (N) to…, Send to trash | the same (`selection_menu`) | – | = |
| Move to… for N | the folder tree, "N items will be moved." + Show details, New folder / Move here / Cancel (docs-move-dialog) | the same browser (Home, Back, the place, New folder, Sort), the count bar and details; Move here off where the rows already are; a new folder's Enter makes and opens it | app check 11-13 (3 moved into a new folder) | = |
| Send to trash for N | "N selections will be sent to the trash."; toast "N items have been sent … Restore" | the same, Restore brings all back | – | = |
| Row drag start | 5 px (4 is a click) | 4 px threshold (`DOCS_DRAG_PX`, gpui `drag_threshold`) | – | = |
| Drag ghost | light-blue pill rgb(178,221,246), 48 px, 10 px left / 8 px above the pointer, ✥ over a target, ⊘ elsewhere, bold name or "N items"; source rows faded | the same (`DocsDrag`) | – | = |
| Drop targets | folder tiles (2 px rgb(22,81,176) border, rgb(232,238,247) fill; toast "Selected items moved."), left panel Trash (Send to trash dialog); not: document rows, "Owned by me" crumb or nav; no dwell-open | the same; folder rows too (list view) | `rows_dropped_on_a_folder_move_into_it`; app check 11-13 (4 moved) | = |
| Row hover / selected | #def1fb / #b2ddf6 (bm 20) | the same | – | = |
| Tab drag: ghost, gap | 100 px ghost, a tab-wide gap, no line (bm 6562) | the same | – | = |
| Tab drag start | 6 px (5 is a click), the tab lifts and leaves a grey slot (feel/documents.json `tabBar.dragStartPx`) | 6 px (`TAB_DRAG_PX`, gpui `drag_threshold`) | – | = |
| Tab dropped on a folder | where it's released decides, not a dwell: the folder tab's middle 20–80 % puts it in (lit), near its ends it reorders (live2 11-13) | the same (`drops_into_folder`, `TabGap.into`) | `a_tab_goes_into_a_folder_released_over_its_middle` | = |
| Tab bar auto-scroll while dragging | from 45 px of an end (not 60), faster nearer it: 13–24 / 51–59 / 66–75 / 94–119 / 108–119 px/s at 45 / 30 / 20 / 10 / 3 px; the tab lands where the pointer is in the scrolled bar (`tabBar.autoScroll`, `dropWhileAutoScrolled`) | the two ends' mean, interpolated (`TAB_AUTOSCROLL`), whatever the frame rate; the pointer's place re-sent each tick so the gap follows the tabs scrolling under it | `a_tab_dragged_to_an_end_of_the_strip_scrolls_it`; app check 11-13 07:02 (held 10 px from the end 2 s: ~250 px, landed under the pointer) | = |
| Tab manager drag and drop | 6 px start; the pressed row selected; a dark-blue pill (white bold name or "N items") below-right of the pointer; between rows a ~2 px dark-blue line with a ring at its left end; a folder row's thirds: above / into (light-blue fill, 1 px dark-blue frame, » at its left) / below; out of a folder to the root; no dwell opens a folder; Esc cancels (`tabManager`) | the same (`TmDrag`, `tm_zone`, `tm_drop_line`; folder rows now laid out as tab rows, 54 px, "Folder" under the name); not undoable (tab operations) | `a_folder_row_takes_drops_in_its_middle_third`, `rows_dropped_beside_a_folder_land_before_or_after_it`, `rows_dropped_on_a_folder_go_into_it`, `rows_dropped_on_a_tab_row_land_before_or_after_it`; app check 11-13 07:02 (ghost, line, Esc) | = |
| Click on a document's name / double-click on a tab | a single click on the name opens the document; a double-click on a tab does nothing (live2 11-13) | the same (`doc-name` click; the tab bar ignores a second click) | – | = |
| Ctrl / Shift + click a tab | opens it in a new window (rq 2403) | the same | – | = |

## Variable Studio, Versions panel, Feature Studio editor (studios, from 2026-11-13)

| Item | Live | Ours (where) | Test | Status |
|---|---|---|---|---|
| VS grid keys | click selects a cell; Enter / typing edits; Backspace clears; arrows move, ↑/↓ wrap; editing: Enter commits and moves down, Tab right (studios.json `variableStudioGrid`; re-taken by live2 11-18 batch 4: unchanged, a commit on the last row keeps it) | the same (`grid_key`, `grid_tab_action`, the `VariableGrid` key context) | `arrows_move_a_cell_and_wrap_up_and_down`, `tab_walks_the_rows_and_the_add_row`; ui-parity vs-grid-keys (live take asked) | = |
| VS row drag | a round blue badge of the first letter, a blue drop line with a dot; one undo step | the same (`VsRowDrag`, `row_drop`) | ui-parity vs-row-drag (live take: recorded 11-14, REQUESTS 3006); e2e `variable_studio_table` (`by` > 1) | = |
| VS column resize | grab 3–6 px inside a header's right edge; min 40 px | the same | ui-parity vs-column-resize | = |
| Versions rows | 50 px; hover #def1fb; selected #b2ddf6; a version row tooltips "Open version V1"; a single click on its name or date opens it (a double click too); the graph column clicks nothing; "Show changes…" lists the changes without opening the row; Main goes back to the workspace (`versionsPanel`) | the same (`graph_column`) | GPUI feel `version_rows_open_on_a_click_of_their_name_and_the_graph_column_opens_nothing`; ui-parity ver-* | = |
| Compare from a version row's menu | enters compare mode at once, no dialog: the workspace the base (blue row), the version the target (pink) (`versionsPanel.compare`) | the same (`compare_with_current`) | GPUI feel `compare_from_a_version_rows_menu_starts_at_once_against_the_workspace`; ui-parity ver-compare-* | = |
| Release candidate › Add to Release… | the Part Studio's row opens expanded with its parts; a click on a part's row picks it (blue), again puts it back (live2 rel-flow-two-parts) | the same (`AddItemsDialog`) | GPUI `add_items_opens_with_the_parts_listed_and_a_row_click_picks_one`; ui-parity rel-flow-two-parts | = |
| FS editor line keys | Ctrl+/, Ctrl+D, Ctrl+Shift+D, Alt+↑/↓, Alt+Shift+↑/↓ (Mac: Cmd, Cmd+Option) (fs0.json) | the same (`fs_lines`) | `ace_line_keys_comment_move_duplicate_and_remove` and unit tests | = |
| FS editor navigation keys | Ctrl+K / Ctrl+Shift+K (Mac Cmd+G), Ctrl+\\, Alt+E / Alt+Shift+E (Mac F4) | the same | `find_next_and_previous_wrap`, `ctrl_backslash_jumps_to_the_matching_bracket`, the keystroke test | = |
| FS autocomplete | keywords and snippets at the top level, std inside a function; opens ~230 ms after a letter (live fsvs-feel-1118; 250–500 ms on 11-13) | the same lists; opens 230 ms after the key (49cfe99b) | fs_complete tests, feature_studio_ui_tests `the_completion_list_shows_230_ms_after_the_key` | = |

## Assembly (studios, from 2026-11-13)

Live: live/feel/mates.json, asm-views.json, fields.json `assemblyInstanceList`, 3d.json `assembly` (live and live2,
11-13). The UI is studios'; asm keeps the mate solver and landed the ring snap, the ▶ preview, Insert's Esc / ✓ and the
drag thresholds (their commits below). Menus leave out "Add comment" (comments are cloud). Counts are matched / total
per matrix (17:18 UTC): **mates 26 / 28, views 14 / 15, instance list 21 / 22, 3D 10 / 10**.

### Mate dialog, mate list, Insert (mates.json): 27 / 28

| Item | Live | Ours (where) | Test | Status |
|---|---|---|---|---|
| M | a new mate dialog, type Fastened, focus in Mate connectors | the same (`FastenMate`, `start_mate`) | ui-parity asm-m-fastened-escape | = |
| A / Q | flip primary axis / reorient secondary 90° | the same (`flip`, `reorient`) | ui-parity asm-revolute-pin-hole-flip | = |
| A / Q: what moves (live2 mate-keys-1118, Revolute on pin-block) | only the free instance (the first chip's); Q turns it −90° about the mate's Z round the connectors' origin (twice: 180°); A turns it 180° about the connector's Y there (the pin's z −5 → +25 mm about the z = 10 connector); the first A right after opening an edit did nothing (one run) | the same: only the free instance moves; Q a quarter turn about the connectors' Z, −90° on pin-block (d9fce0c0); A in a fresh edit 180° about the connector's Y (the flip and half a turn of the held free angle, assembly_behaviors `flip_or_reorient`) | q_in_an_edited_revolute_turns_the_free_part_a_quarter, a_in_an_edited_revolute_turns_the_free_part_over_about_y, an_edited_revolutes_q_and_a_turn_the_free_part_as_live | = |
| I / J / H / K | Insert / hide-show mates / Show mates mode / hide-show connectors | the same (actions.rs) | ui-parity asm-h-key-show-mates-mode, asm-j-shows-mates, asm-i-key-insert-escape | = |
| Ctrl+M / Shift+S | Mate connector / Snap mode | the same | – | = |
| Dialog on open | 220 px at the graphics' top left, "Fastened 1", the row in the list at once, red | the same (`assembly_mate_dialog`) | ui-parity asm-m-fastened-escape | = |
| Fields | type ▾; Mate connectors with Reorder items; chips "… (Click to edit)" ×; Offset; Simulation connection; Flip / Reorient / ▶ / Solve | the same | – | = |
| Type order | Slider, Cylindrical, Revolute, Pin slot, Planar, Ball, Fastened, Parallel | the same (`MATE_TYPE_ORDER`) | – | = |
| Which part moves | the first pick's part moves onto the second; a fixed part never | the same (`new_mate_stays`) | `a_mate_gets_its_heads_up_check_after_the_second_pick` | = |
| After the second pick | the part jumps to its mated place, no animation | the same | – | = |
| Mate axes | X/Y/Z 50 px at the connector (screen size) | the same (`ScreenTriads`, camera geometry) | `mate_axes_stay_50_px_at_any_zoom` | = |
| Floating toolbar | grip, type, flip, reorient, Solve, ✓, ✗ | the same (`render_mate_hud`) | ui-parity asm-fastened-two-faces | = |
| Toolbar's place | top left ~12 px right of and 14 px below the second pick (live2 18:20) | the same (`render_mate_hud`) | – | = |
| Toolbar's grip (live2 place-1118) | a press-drag on the top grip moves the toolbar with the pointer; it stays where released; Esc ×2 still cancels the mate | the same (green 548d795fb, pin-block: dragged (−300, +400), stayed, Esc ×2 cancelled) | – | = |
| Type switch | connectors, Offset, flip and reorient kept; the name follows the type; offset fields follow it | the same (`set_type`, `offset_axes`) | – | = |
| Connector chip | Click to edit: the Mate connector sub-dialog beside (Realign adds the axis fields) | the same | – | = |
| ▶ preview | 90 frames out and back from the mate's zero (a turn, ¼ of the view's width) | the same (asm 43736c55, e9941770) | asm's tests | = |
| Mate row hover | both parts outlined, the marker, a flyout with the name and parameters | the same (`list_hover_mate`, `mate_tooltip`) | – | = |
| Mate menu | Rename … Delete | the same | `two_selected_mates_have_lives_menu`, ui-parity | = |
| Animate… | … Playback type Reciprocate ▾ and an icon button "Reverse playback" (live2 18:30) | the same | – | = |
| Named positions, empty | the how-to text and its help link, Add named position | the same (17:18) | – | = |
| Insert dialog | tabs, Part Studios / Assemblies, search, "Undo to remove instances", "Inserted: N" | the same | ui-parity asm-insert-* | = |
| Insert preview | the instance follows the pointer, solid orange; a click drops it | the same | – | = |
| Insert preview's hidden edges | outlined orange through other parts; nothing lit under the pointer | the same (`xray_tops`, hidden-only #ffc04c edges, `Instance::see_through_self`: not through the preview itself) | render `see_through_self_lines_skip_their_own_mesh` | = (fixed) |
| Insert Esc, pending | drops only the instance on the pointer, keeps the dialog | the same (asm e9941770) | asm-insert-esc-pending (live take: REQUESTS 2985, answered) | = |
| Insert Esc, none pending | closes and removes the session's instances | the same (`cancel_insert`) | – | = |
| Insert ✓ | keeps all, the pending one at identity; one undo step | the same (asm e9941770) | asm-insert-ok-pending (live take: REQUESTS 2985, answered) | = |
| Ring snap | 5° / 1° / 0.1° by the arc at the pointer's radius ≤ 50 px, within 0.4 × (1 + level/3) of a step | the same (asm 641ab465, `ring_snap`) | asm's tests | = |
| Snap mode ghost | the source part at 0.3 opacity | the same (17:18) | – | = |
| Snap mode lines | none: no dashed line or 8 px box in 3 snap-mode drags over 23 pointer stops (live2 snapmode-1118); the client's INFERENCE_SNAP_* constants aren't drawn there | none | – | = |
| Snap mode drag (live2 snapmode-1118) | a press-drag on the hovered face's connector: a translucent ghost carries it, the connector follows the pointer and jumps onto a target dot (a corner, an edge midpoint, a face centre) when the pointer is within 50–72 px of it depending on the face under the pointer and the direction (not a square: live2 snap-diag-1118, which corrects the earlier "57 px in x and in y"); release: connector on connector, frames aligned (not flipped), then an uncommitted "Fastened 1" mate dialog with both chips and the HUD; Esc closes it with no mate and the part stays snapped | the same: press-drag, the connector jumps onto a dot of the face under the pointer within 51 px in x and in y (c7d388e8, live2 snap-diag-1118; qa on green: 49 px straight up, 71 px at 45°, live 50–51 / 72), off every part it stays frozen and a release moves nothing; release on a target aligns the frames and opens the uncommitted Fastened dialog (Esc: no mate, the part stays; Ctrl+Z in it does nothing) (assembly_panels `snap_target` / `finish_snap_drag`) | a_snap_drags_reach_is_51_px_in_x_and_y, a_snap_mode_drag_lands_aligned_and_opens_an_uncommitted_fastened | = |

### Exploded, section and named views (asm-views.json): 15 / 15

| Item | Live | Ours (where) | Test | Status |
|---|---|---|---|---|
| Exploded views panel | 3rd of the strip; empty: Add exploded view, illustration, how-to, help link | the same (Simulation left out: cloud) | ui-parity asm-explode-panel, asm-explode-add-view | = |
| Add exploded view | "Explode 1" in edit mode: yellow "Editing Explode 1 / Done", toolbar only undo/redo, the blue how-to, the rollback bar | the same | – | = |
| Selecting an instance | orange outline through other parts (in explode edit mode too, live2 11-14), a triad (3 arrows, 3 rings, centre) | the same (render `Instance::outline_through`: the silhouette's outline outside its own image, no depth test) | render `an_outline_through_shows_past_the_part_in_front` | = (fixed) |
| Arrow drag | moves live, no value box; on release the Explode step dialog; only that arrow stays | the same | – | = |
| Ring drag | a Rotation step, 5° steps, Specify axis | the same (`ring_snap`, 17:18; was 1°) | `explode_drag_values_round` | = |
| ✓ / Esc on the step | keeps / removes the step | the same | – | = |
| Done | back to mated positions, toolbar back | the same | – | = |
| Row click | selects (blue), tooltip with its name, no explode | the same (17:18: the tooltip) | – | = |
| Row menus | collapsed: Activate, Rename, Reset starting position, Duplicate, Delete; active: Collapse, Rename, Show / Hide all explode lines, Delete | the same | ui-parity asm-explode-row-menu, -row-menu-editing | = |
| Activate | the exploded state, editing again; a hovered step row outlines its instances | the same (17:18: the outline, `step_hover`) | – | = |
| Camera menu | Isometric … Section view… | the same (camera_menu.rs) | – | = |
| Section view… | opens empty with the blue hint "Select a plane, planar face, cylinder, cone, or mate connector." (live2 18:20); Exclude / Include; Items to exclude | the same (the Part Studio's hint) | – | = |
| Section: a face click | adds a plane through the face (live2 18:20) | the same (`section_pick`) | – | = |
| Section Esc | cancels | the same | – | = |
| Named views | New view name + "+", No views created, Select a view + ⊗ | the same (named_views_dialog.rs) | – | = |

### Instance list (fields.json): 22 / 22

| Item | Live | Ours (where) | Test | Status |
|---|---|---|---|---|
| Hovered row | light blue, an eye at the right end | the same | – | = |
| Row tooltip | only for a cut-off name (a row whose name fits has none; the fix icon's "Part is fixed"), live2 11-14 | the same (the name measured against its room), at 80 ms (`ROW_TIP_DELAY`) | – | = (fixed) |
| 3D highlight on hover | ~0.85 s after the pointer arrives on live2's software-GL host; its graphics hover takes ~0.73 s too (live2 18:20 (6)): the host, not a delay of Onshape's | at once | – | = |
| Plain click | toggles the row | the same (`list_click`) | ui-parity asm-select-mate-row | = |
| Ctrl+click | opens the context menu (live2 18:20; behaviours "skb (10-24)" (3)) | the same | – | = |
| Shift+click | the range from the last clicked row, added; after a deselecting click from the top row | the same (17:18: the top row) | – | = |
| Esc / empty graphics | Esc keeps the selection; an empty graphics click clears | the same | – | = |
| Double-click an instance | its Part Studio, the part selected, "<assembly> displayed" bar | the same (17:18, `show_in_context`) | – | = |
| Rename | none for instances; Instance properties… (Reference designator) | the same | ui-parity asm-menu-instance-properties | = |
| One instance's menu | Properties… … Delete | the same | `the_instance_and_top_row_menus_are_lives` | = |
| Two instances' menu | … Dynamic suppression ▸, Check interference… \| Show / Hide mates, Replace instances…, … Clear selection, Delete | the same (17:18) | `two_selected_instances_have_lives_list_menu` | = |
| Mate menu / folder menu | Rename … Delete; Rename … Unpack folder | the same | ui-parity asm-menu-* | = |
| Mixed selection menu | instances, the Origin and a mate: Hide … Clear selection (no Suppress, Fix, Copy, Export…, Delete) | the same | `instances_with_a_mate_selected_have_lives_reduced_menu` | = |
| Right-click an unselected row | acts on both rows (live2 18:20; behaviours "audit 1943" (2)) | joins the selection | – | = |
| Add selection to folder… | a floating "Folder name" panel; the folder row only after Enter (live2 18:20) | the same | ui-parity asm-menu-add-selection-to-folder | = |
| Folders | collapsed; double-click does nothing; one Ctrl+Z | the same | ui-parity asm-menu-unpack-folder | = |
| Row drag look | a blue pill with the name; a blue line with a ring at its left end | the same (17:30, `InstanceDrag`, `list_drop_line`, as the Tab manager's) | – | = |
| Row drop | reorders, one undo step | the same ("Reorder instances") | – | = |
| Hidden mate | grey name and icon | the same | `hidden_mate_rows_are_grey` | = |
| Suppressed mate | grey italic struck through; "Fastened 1 (Suppressed)" | the same (17:18: the tooltip) | – | = |
| Failed mate's colour | rgb(170,37,45); hidden, rgb(196,139,148) (live2 errors-1118 tips) | the same (17:18; was #bd3039; hidden faded red since 11-18, was the strong red) | `hidden_mate_rows_are_grey`, `a_hidden_mate_in_error_is_faded_red` | = |
| Failed mate's marks | red "!" on the icon, "Fastened 1 has error: …", red Mate features icon and root overlay, ⚠ on the fixed instances | the same | – | = |

### 3D in an assembly (3d.json `assembly`): 11 / 11

| Item | Live | Ours (where) | Test | Status |
|---|---|---|---|---|
| Explicit connector's reach | 23 px across, 14 px up and down | the disc picks, in its plane (18 px + the pick square's 5) | `a_picked_glyphs_disc_takes_the_pointer_across_it` | = |
| Explicit connector's priority | over vertex, edge, face | the same (overlay first) | psui's 3D matrix | = |
| Mate glyph's reach (live2 asm-feel2 (b)) | the drawn glyph (~26 px across face-on) plus 6–7 px wins over the face behind it (Slider 1 on a slanted face: left 12, up / down 20, right ≥ 24 along its arrow); hovered: a ~32 px translucent white square behind it, its face outlined light orange; each type its own ~25 px icon (live2 mate-glyphs 11-17) | one drawing per type on an 18 px unit (`MateGlyphs`, `MATE_GLYPH_UNIT_PX`; ~27 px across), no triad at rest, picked by a 15 px disc + the 5 px pick square; the white square, the face outlined | GPUI `a_mate_glyph_wins_the_hover_within_its_reach_and_its_click_selects_the_mate` (19 px the glyph, 24 the face) | = |
| Mate glyph click (4) | selects the mate: row blue, its instances' rows grey, mated faces amber, a dashed line between the connectors | the same | the same test | = |
| Part hover (3b, asm-feel2 (c)) | the face's boundary edges (every loop) #FFC685, the part's silhouette #FFC20E, the fill unchanged; inside a subassembly only that part (its face with a two-level path), a click takes that face and the collapsed sub's row goes grey | the same (`Highlights::hover_part`, `Scene::hover_outline_rgb`, `picked_inside`) | GPUI `a_part_inside_a_subassembly_hovers_and_is_taken_alone` | = |
| Instance row tooltip (asm-feel2 (a)) | every row's label shows the instance name, cut off or not, right of the list, fading in ~0.1–0.27 s; the next row's at once; the icon columns their own tips | the same (`with_row_tip`; warm switching for any delay up to the default, GPUI patch) | GPUI `every_instance_row_tooltips_its_name` | = |
| Graphics → list (2) | nothing lights in the list, no flyout | the same | – | = |
| Box select (5) | left → right over half a part: nothing; round it: the instance; right → left over half: the whole instance; never faces | the same | GPUI `a_box_takes_whole_instances_as_live` | = |
| Explicit connector's look | a 36 px disc, 4 px ring, Z line 20 px, X/Y 12 px | the Part Studio's disc (`ConnectorGlyphs::picked`, screen size) | `connector_glyph_extent_counts_for_the_zoom_limits` | = |
| Implicit, over a face | the centroid wherever the pointer is; corner dots larger than midpoint dots | the same (17:18; Shift still picks among the points) | `a_faces_connector_is_its_centroid_wherever_the_pointer_is` | = |
| Implicit, over an edge | the midpoint, Z along the edge | the same (17:18) | – | = |
| Vertex / edge over the face | takes over within 6 px, not at 8 | the pick square (psui's 3D matrix: 15 / 15) | psui `the_3d_pick_matrix_matches_live` | = |
| Free drag | moves in the screen plane by the full pointer delta | the same | – | = |
| Drag start | > 2 px and > 50 ms | the same (asm ccbc333f) | asm's tests | = |
| Drag feel on a software-GL host (live2 drag-1118, lead (c)) | two-blocks: first moved frame 1.7–2.0 s after the press, then a frame per input step (~0.3 s), the last 0.8–1.5 s after the last move, release settled 0.9–1.2 s; RB 18 (1177 occurrences): the drag is lost, the press answered 12.7 s later as a click (selection) | qa 21:5x, green 7302f8bd on vibex8 Xvfb (software GL, load 8-13), live2's method with ImageMagick grabs every ~0.15 s (no ffmpeg there; qa-scratch/af/dragq.py): two-blocks (free block, 3 runs): first moved frame 0.81-1.02 s, a frame every 0.32-0.45 s, the last 0.44-1.07 s after the last move, settled at the release; asm-xdev-subs (asm's RB 18-class save, 1 run): the press lights the part at 0.58 s, no frame during the drag, the part's move drawn 13.3 s after the press (3.9 s after the release), redraws at 21.9 / 29.9 s (live loses the drag; ours applies it late: the HEAVY-ASM-SOFTWARE-FRAME P0) | – | = (two-blocks); RB 18: ours applies the drag, live doesn't |
| Triad drag steps (live2 triad-step-1118; client constants 11-18) | an arrow: 10^(round(log10(the length of 100 px at the handle)) − 2), always rounded; a ring: 5° while 5° spans ≥ 50 px of arc, else 1° / 0.1°, snapping within 0.4·(1 + level/3) of a step; an explode arrow ~0.1 mm a step at a close zoom (120 px → 8.1 mm) | the same: arrow `arrow_snap_step_for`; ring `ring_snap` (the coarsest of 5° / 1° / 0.1° whose arc at the pointer's radius is ≤ 50 px, the bundle's getSnapAngle, behaviours 10-27 (1)); explode 0.1 mm (`explode_drag_round`) | assembly_view `arrow_drags_snap_to_a_step_that_follows_the_zoom`, `ring_drags_snap_to_a_step_that_follows_the_pointer_radius`, `explode_drag_values_round` | = (checked 11-18) |
| Short press | a click: the face selected, the triad there; its centre ball moves the triad | the same | – | = |
| Constrained drag | a Revolute part turns about its axis, axial position kept; the turn depends on the grab point (live2 11-14, 100 px right: 53.1° from the top face's centre, 42.0° / 44.5° 4 mm off-axis) | the same, grabbed where the pointer meets the part (`ray_hit`; was the box centre, which turned pin-block's pin the wrong way) | app check 11-14 (pin-block: +24.8° toward the pointer, z −5 kept) | = |

## Sketch snaps (matrix)

One row per target × inference × tool: live's reach and the constraints its click creates, ours, at a device pixel
ratio of 1 and 2. The rows are data in one table-driven test, part-studio-ui `snaps::the_sketch_snaps_matrix_matches_live`
(T1, ~13 s, no GPU: a model target is lit as the hover pick would light it); it prints matched / total, fails when a
matched row stops matching, and when a gap starts to (so this table moves with it). Live: live2's catalogue
(live/feel/sketch-snaps.json, behaviours "sketch snaps (catalogue)", 11-13 04:16), else live/feel/sketch.json and qsnap.

Live's model (from its client): a point (an end, a centre, a midpoint, the origin, a model vertex or edge midpoint) is a
15 px square round it; a curve (a sketch curve, a model edge) and an inference line (H / V, alignment, parallel,
perpendicular, a tangent line) a 15 px band. Priority: coincident points > midpoints and crossings > curves > lines;
a line crossing a curve near the pointer gives the crossing with both constraints. Waking (snapping onto) adds: a
point → H + V through it; a sketch line → parallel + perpendicular through the start; a model edge → perpendicular;
a circle → the two tangent lines from the start; a spline → nothing. Shift: no snapping. In-plane model entities only.

**Matched: 67 / 67** (11-17 16:47 UTC: the spline rows remeasured by live2 on a straight spline; 65 / 67 at 16:18: a woken model edge's no-parallel and end alignment rows, live2 wake-parallel; 63 / 65 at 11-16 04:23 UTC: three woken model edge rows (live sketch-gaps 1c), matched once model edges wake; 60 / 62 at 03:36 with three spline rows from live2's sketch-snaps reachZoomA / B, the through-point along matched, across and the curve gaps; 59 / 59 at 01:57 with sketch-gaps' crossing row; 55 / 55 at 11-13 06:02 UTC). Probes: a point 6.5 px straight out snaps, 8.5 doesn't; 9.5 / 11.5 diagonally;
a band 6.5 / 8.5 across (live's own takes vary by a pixel with where its target sits in its pixel).

| Row (tool / target / inference) | Live | Ours (where) | Status |
|---|---|---|---|
| Line end / a line's end, an arc's end, an arc's / circle's centre, a point, the origin (straight, diagonal) | 7 / 9–10 px (6–8 by target); COINCIDENT, not CONCENTRIC, no alignment | the square (`infer`'s `in_point`; was an 8.5 px circle: 8 px snapped, 10 diagonally didn't) | = (fixed) |
| Line end / a line's midpoint, an arc's midpoint | 9 px along (0.45, 0.89), 6 px straight; MIDPOINT | the square | = |
| Line end / a line's midpoint 10.1 px from where another line crosses it (live sketch-gaps 1a) | no intersection point where sketch lines cross (on-curve to one of them); the midpoint wins wherever it is in reach: 6 px towards the crossing, the crossing line's on-curve from 7 | the same | = |
| Line end / a line, a circle's rim (both sides) | 7, not 8; COINCIDENT | 7.5 band | = |
| Line end / a lit model vertex (straight, diagonal) | 6–7 / 9 px; COINCIDENT (used) | the square (was 16 px round); in-plane only | = (fixed) |
| Line end / a lit model edge (both sides) | 7, not 8; COINCIDENT (used); a direction kept where it meets the edge | the band (was 10 px); in-plane only | = (fixed) |
| Line end / a lit model edge's midpoint | 6–7 px; MIDPOINT (used) | the square (`ExternalSnap::Edge { midpoint }`; also a dragged point's drop) | = (new) |
| Horizontal / vertical through the start (both sides) | 7–8 px each side | the band (was 7 / 6) | = (fixed) |
| Line from an arc's end / tangent; from inside a line / perpendicular (two lengths) | 360 / L px (L: the line's length in px): 100 px long 2, not 4; 150 px 2, not 3; 200 px 1, not 2; 300 px 1, not 2 | the same (`DIRECTION_REACH_PX2`; was a fixed 3°) | = (fixed) |
| Line end / a circle at its tangent point | 6 px on along the tangent, not 10 (then on the circle only); TANGENT + COINCIDENT | the crossing of the tangent line with the circle, a point zone (`direction_onto_curve`) | = (fixed) |
| Woken sketch line / parallel through the start | 150 px long: 2 px off, not 3; PARALLEL | the same (was never) | = (fixed) |
| Woken circle / a tangent line from the start | TANGENT (the end on the line) | the same (new) | = (new) |
| Circle tool, radius point / a line's midpoint or the line | nothing (coincident points and tangent points only) | the same (was midpoint / on-curve / H-V) | = (fixed) |
| Shift / a line's end 3 px off | no snap, nothing made | the same | = |
| Two at once: horizontal + on a free line; a point over horizontal; a point over its line; two acquired points' H × V | both; the point; the end point; H + V | the same | = |
| Two zones crossing (live2 combo.json 11-14): square to the last segment × a sketch line, along the line / along the square; square × a woken point's horizontal | both while the pointer is in both zones: 6 px holds, 9 only on the line; 9 holds, 12 only square; holds at 3-6 (9: only square) | the same (`combo/perpendicular+line/along`, `/across`, `combo/perpendicular+alignment`; a tangent line touching its circle keeps the 15 px point: `line-end/circle/tangent`) | = |
| Curves over lines (a line zone and a curve: the crossing if near, else the curve) | catalogue "priority" | `is_direction_line` + `direction_onto_curve` | = |
| A circular model edge in the plane / its centre (lit or not) | CONCENTRIC, a point zone below the coincident points | the same (`centre_snap`, `ExternalSnap::Centre`, `AutoConstraint::ConcentricTo`) | = (new) |
| Another sketch's line end / its line | COINCIDENT (used), 8 / 7 px | the same (`model_snap_of` takes `SketchEntity`) | = (new) |
| Rectangle, second corner | the full set but no H / V through its first corner; midpoints kept | the same | = |
| 3 point arc, third point | coincident points (and its chord's midpoint) only | the same | = |
| Drags: a free end onto a point (straight, diagonally), a midpoint, a circle's rim; no H / V | 7–9 / 6, not 8 / ≤ 6; COINCIDENT / MIDPOINT on release | the squares and bands (`point_drop_target`; points were an 8.5 px circle) | = (fixed) |
| Line end / a spline's through-point (along); across it; its curve between points | 7 px along, then on the curve; across and between points as a line (live2 spline-reach 11-17, a straight spline: 8 up / 6 down; the catalogue's 12 px was a bulging spline probed at 45°) | the square (7.5) and band (7.5) | = (live remeasured) |
| Woken model edge / a perpendicular through the start (live sketch-gaps 1c) | a straight model edge wakes after a 60 ms rest (not 0 or 30); then PERPENDICULAR through the start, a line zone 7–9 px either side (not an angle), the edge used | woke nothing (model edges never woke); now `SketchEditor::woken_edge` (`EDGE_WAKE_REST` 60 ms, `woken_edge_perpendicular`, the click uses the edge: `link_woken_edge`); the bench's triangle prism (`triangle_part`) | = (fixed) |
| Woken model edge / no parallel; its ends' alignment (live2 wake-parallel 11-17) | no PARALLEL along it 0.2–3° off, woken or not; woken, an end level with or plumb below its end vertex takes HORIZONTAL / VERTICAL with it | the ends were never acquired; now `InferenceContext::woken_at` (the woken edge's ends, `link_woken_edge` uses the edge's end) | = (fixed) |
| Let-go: a lit snap held as the pointer moves out (live 11-17 sketch-snap-1117) | lights at 7 px; a point lets go at 9–10 px straight out, 11–12 diagonally; curves, edges and the H / V bands at 8–10 | the same (`held_snap`: [`HOLD_PX`] 9.5, 11.5 diagonally; it let go where it lit, 7.5) | scene `a_lit_snap_holds_to_its_let_go_reach` | = (fixed) |
| Shift held while drawing (live 11-17 sketch-snap-1117 (4)) | no inference: no square, no glyph, no H / V (a line 2 px off level stays tilted), the end at the raw pointer, the click adds no constraint (1 px from a point: no coincident) | the same (`infer(.., suppress)` with `shift_held`) | scene `drawing_a_line_snaps_in_px_bands_and_shift_snaps_nothing_as_lives`, `shift_suppresses_inference_and_drop_snap_without_pointer_movement` | = |
| Value boxes while drawing (live2 11-17 value-box) | a digit opens the box on the last finished segment (never the rubber band), centred 26 px above a horizontal one, 39 px left of a vertical one; Enter dimensions it, the chain going on from its moved end; a digit after that, or Tab with no box: nothing; Esc in the box: no dimension, the tool on; a canvas click drops the value and places the next point; "1abc" Enter: the box stays, pink rgb(249,235,236), no message; a circle's / arc's box only once it is finished | the same (`line_length_field`; the click closes an open box first; `otf_invalid`; was: 26 px left, the box kept over a click, a message) | GPUI `value_boxes_mid_chain_as_lives`, scene `a_vertical_lines_value_box_goes_left_and_short_arrows_stay_inside`, `value_boxes_where_live_puts_them` | = (fixed 11-17) |
| Dimension tool flow (live2 11-17 sketch-dim-sel) | a pick previews at once; the placement click creates the dimension and opens its box, the measured value pre-selected; Esc in the box keeps it, the tool on; "1abc" Enter: the box stays invalid (an expression list offering abs( by the first letter doesn't take the Enter), the first Esc restores the measured text, the second closes; Esc after a pick: nothing added, the tool off; Delete on a side: the line and the constraints on it | the same (`VarCompletion::enter_inserts`: was inserting abs()) | GPUI `the_dimension_tool_flow_as_lives`, app-core `enter_inserts_only_an_entry_matching_the_word` | = (fixed 11-17) |
| A model edge on the sketch plane (live 11-17 sketch-snap-1117 rows 1, 3; qa SKSNAP-MODEL-EDGE) | takes a line's end from 7 px, its middle the midpoint, with no hover of it (in a normal view the block's top edge drawn over it is the one lit); COINCIDENT (end, edge) | the same (`plane_edge_snap` when the hovered model geometry gives no snap; was: lit, no snap) | GPUI `a_model_edge_on_the_sketch_plane_snaps_unhovered` | = (fixed 11-17) |
| Another sketch's point rested on (live 11-17 sketch-snap-1117 row 2; qa SKSNAP-ALIGN-WOKEN-POINT) | wakes as a point of the sketch: its vertical / horizontal a ±6–7 px band at 60 and 150 px, the yellow dashed guide to it; the click aligns with it (used) | the same (`woken_point`: an external vertex snap held `ACQUIRE_REST`; was: only the sketch's own points woke) | scene `a_rested_point_of_another_sketch_offers_its_alignment` | = (fixed 11-17) |
| Another sketch's point under the pointer (live 11-17 sketch-snap-1117 row 1; qa SKSNAP-REACH) | its 15 px square: lights at 6–7 px along x, a circle's centre at 5–8, no hover needed; a point beats an edge | the same (`plane_point_snap` via `unhovered_snap`; was the GPU's 5 px point pick: 5 / 4 px) | GPUI `another_sketchs_point_snaps_from_its_square` | = (fixed 11-17) |
| A line's end snapped onto another sketch's point or a model vertex (live2 11-18 use-glyph; qa SKSNAP-SHIFT-ICON) | one COINCIDENT to the point, no PROJECTED: the coincident glyph only (light-blue tile), the end a hollow grey circle; no use glyph; Use on the point adds it | the same (`take_vertex_snap` keeps the point reference-only; the end is a point of its own; was a used point with the cube glyph) | GPUI `another_sketchs_point_snaps_from_its_square` | = (fixed 11-18) |
| Sketch right-click menus (live 11-17 ctx-menus-1117, menus-sk.json) | a line / a selected line / a line's end / a line and its end: Confirm Sketch N first, then the entity groups (Construction, Extrude… / Revolve… only for curves; "Delete 2 sketch entities"); with Line on, "Escape line" above | the same (Add comment left out: cloud) | GPUI `sketch_entity_menus_as_lives`, sketchtools "line / the menu mid-chain" | = |

## Feature list and Parts list (matrix)

One row per behaviour live2 measured (live/feel/featurelist.json, 11-13 06:00–06:48), acted out with real clicks,
Shift / Ctrl / Alt, double clicks and drags at the rows of a fresh Part Studio (three parts, Plane 1 between), at a
device pixel ratio of 1 and 2: part-studio-ui `list::the_feature_list_matrix_matches_live` (T1, ~4 s, no GPU; a
canvas click is the viewport's empty pick). Like the snaps matrix it prints matched / total and fails when a row's
status moves. `LIST_ROW=<part of an id>` runs some rows.

**Matched: 78 / 78** (11-18 00:40 UTC, live2 featurelist-feel / doc-level-1117 rows added; 57 / 57 11-13 15:51 UTC; 20 / 42 at the first run; 3 folder rows added 15:51: 1 / 3 matched, 2 fixed). ui-parity: the 78 list and tooltip scenarios 75 / 78 before the last fixes, the 3 left on green too or fixed since (feature-row-drag-above).

| Row | Live | Ours | Status |
|---|---|---|---|
| Click / Alt+click | toggles, others kept | the same | = |
| Shift (down, up) | the range from the anchor (the last row clicked, Shift or not), sketches and planes between | the same | = |
| Shift on a selected row | deselects that range (Ext 1, Shift Ext 3, Shift Ext 2 leaves Ext 1, Sketch 2; Parts 1–3 then Shift Part 1: nothing) | the same (`behaviors::list_shift_click`; was always adding) | = (fixed) |
| Shift, nothing selected | from the top: Origin, Top, Front, Right, Sketch 1 … | the same (was the row alone) | = (fixed) |
| Shift on a part, a feature the anchor / no part clicked yet | nothing / nothing (live 08:18) | the same: one anchor across both lists (was a plain click) | = (fixed) |
| Ctrl+click a feature / a part | its menu, selection kept / adds, no menu | the same | = |
| Clearing | an empty canvas click clears; Escape and empty list space don't | the same | = |
| Double-click a feature (name or blank right side) / a sketch | its dialog / sketch mode | the same | = |
| Hover a hidden sketch's row / click a default plane | the sketch drawn in place, light orange, while hovered / the plane's row selected, the Default geometry folder related (live2 11-17 sel-feel 4, 2, 5) | the same (`sketches_shown_for_now`, `default_geometry_related`; were: nothing drawn / the folder unlit, qa 208ca8549) | = (fixed 11-17) |
| Double-click a default plane / a part | the plane left selected (retaken 11-18, live/feel/list-1118) / two toggles, no rename | the same (a part's name renamed: removed; ui-parity parts-row-dblclick agrees) | = (live recheck 11-18 on green b5b664aaa: Top stays selected) |
| Rename | only the menu's: inline, the whole name selected; Escape cancels | the same | = |
| Y over a part row, nothing selected (after its double-click) | hides that part | the same | = (new) |
| Menus: one feature; two features; two selected and a third right-clicked (counts 3) | live's rows (Add comment left out: cloud) | the same | = |
| Menu: two hidden sketches | … Show, Show all sketches … | the same (Show all sketches was missing) | = (fixed) |
| Menu: a sketch and a feature, on the feature | … Hide other sketches, Hide all sketches … | the same (were left out for hidden sketches; still left out when the right-clicked row isn't selected, ui-parity feature-row-rclick-other-selected / -mod-click-toggle, and on folders) | = (fixed) |
| Menu: a feature and a part, on the feature / on the part | Clear selection added / the part's menu without Copy Part N | the same (Copy Part N was kept) | = (fixed) |
| Menu: a feature and a default plane, on the feature | Size to Part Studio, Hide other planes, Hide all planes, Section view…, Clear selection, Add appearance to feature…; no Delete | the same (was the two-feature menu) | = (new) |
| Menu: two parts; one part; the rollback bar | for N parts…, Boolean, Copy N items, … Delete / the part's / none | the same (several parts had the one-part menu: `several_parts_menu`) | = (new) |
| Drag a row: top half / bottom half | before / after the row; the selection as it was (live 08:18 corrected its catalogue: a drop never selects) | the same | = |
| Drag with a selection | the selection and the pressed row move, in list order, the selection kept; "N items" | the same (only a selected pressed row carried the selection) | = (fixed) |
| Undo step of a move | "Move Extrude 3", "Move 2 features" | the same (`noshape_ops::studio::move_description`, also the feature_reorder op; was "Reorder feature(s)") | = (fixed, not a matrix row) |
| Drag: no refusal; over the canvas; Escape; default geometry; onto default geometry; parts | moves (then fails) / cancelled / cancelled / no drag / nothing / no drag | the same | = |
| Rollback bar: the whole 26 px row grabs; top / bottom half | before / after the row | the same (its drop ignored the half) | = (fixed) |
| Rollback bar held (live2 featurelist-feel 11-17) | the "Rollback bar" chip and the insertion line; the bar stays at its old place, nothing greys until the release | the same | = (new) |
| Rollback bar released | the rows below grey, the Parts list follows (Parts (0) above Extrude 1); above the first feature: everything; back at the end all again | the same | = (new) |
| Rollback bar over Default geometry | released over Origin, a plane or the header (expanded or collapsed) it lands at the top, everything rolled back; the drop line keeps the last legal slot (live2 rb-followup 11-18, correcting featurelist-feel and qa 11-18) | the same | = |
| Rollback bar look and menu | #b8b8b8, rgb(200,217,226) only while hovered (live2 rb-followup); right-click: "Roll to end" alone while rolled back, no menu at the end | the colour the same; no menu (qa 11-18) | = (menu fixed; the colour has no matrix row) |
| A reorder the history can't take | accepted, then the row red rgb(170,37,45) with a warning triangle; no message | the same | = (new) |
| Rename keys | Enter commits; a click elsewhere commits; F2 does nothing; the renamed row's tooltip "Bar [Extrude] <- Sketch 1" | a click in the graphics left the input open (it kept the keyboard); now any press outside commits | = (fixed) |
| Suppress / Unsuppress from the menu | grey italic struck through, the part gone; Unsuppress restores it | the same | = (new) |
| Edit dialog place | Extrude 1's dialog right of the list, 220 px wide (live (246, 79) 220 × 352; qa 11-18: ours (246, 80)) | the same | = (new) |
| Undo steps (live2 doc-level-1117) | a rename and a part's Hide are one step each, with no toast; a selection and Shift+5 are none; Ctrl+Y redoes, Ctrl+Shift+Z doesn't (app-core `actions` tests); Ctrl+Z in a dialog's field does nothing (`undo_does_nothing_while_a_dialog_control_has_the_keyboard`) | the same | = (new) |
| Graphics menu over a selection (live ctx-menus-1117) | the face clicked selected: Clear selection above Select; two faces: "Add appearance to 2 faces…", no New sketch… / Offset plane… / Extrude… / Revolve… / Export as DXF/DWG…; a face and an edge: the faces' menu without View normal to, Delete Part 1…, the part's material and appearance rows, Fillet… / Chamfer… or analysis ; a face with Part 1's row selected: no New sketch… … Revolve…, no Export as DXF/DWG…; a vertex: no Section view…, Zoom to selection; the Top plane and Origin rows | the same (sketch_ui_tests `graphics_menus_over_a_selection_as_live`, list matrix rows; a face + an edge gave the edge's menu, a face with the part selected its creation rows) | = (fixed) |
| Parts list: Cmd+click | toggles like Ctrl (metaKey or ctrlKey on every OS) | the same | = (new) |
| Hover: the row tooltip | "Extrude 1 <- Sketch 1"; fades in from 65–100 ms, opaque at 166–209 (retaken 11-18, live/feel/list-1118, 3 runs; 11-13: from 160, full by 390) | the text as before; shown at 80 ms, its 110 ms fade starting at a fifth (`ui::ROW_TIP_DELAY`, `ROW_TIP_FADE`: from nothing its first frame drew nothing, a frame or two late on a slow lane) | = (live retake 11-18 23:27 UTC on green 5c9229069 at load 15, 10 hovers, tip2.mp4: first visible 83–117 ms (one at 150), at a fifth; opaque 183–333, median ~260 against live 166–209, under 100 ms off; the fade lands in 2–3 steps because the app draws ~10 fps on this Xvfb) |
| Hover: the geometry | lit 100 ms after the pointer arrives, cleared 100 ms after it leaves; a sweep lights only where it rests | the same (`LIST_HOVER_DEBOUNCE`; was at once) | = (fixed) |
| Folders: New folder | nothing selected: an empty folder above the rollback bar; features and a part: the features; only default geometry: nothing; Escape: none | the same (default geometry alone opened the popover) | = (fixed) |
| Folders: dropping on a collapsed folder | top third before it, middle inside at its end, bottom third after it | the same (every drop went inside, first) | = (fixed) |
| Folders: dropping on an expanded folder | top half before it, bottom half inside as its first child | the same | = |
| Folders: nesting | four levels: under a fourth-level folder New folder greys (0.5, no message, a click does nothing); a feature still drops into the fourth; a folder chain dropped deeper than four only shows the line and nothing happens (live2 11-14, feel/nest-*.png) | the same: New folder (`PartStudio::new_folder_fits`; the agent API's feature_add_folder says why); a folder row drags with its features, into another folder's middle or between rows (`move_folder`), a drop past four levels a red line and nothing (`folder_fits_in`) | = (list matrix "folders / a folder dragged onto another", "… past four levels …"; e2e `feature_folders_dependencies_and_dynamic_suppression`) |
| Folders: a drop between its children | inside the folder | the moved feature kept its own folder and the folder lost the children before it; now the folder at the drop (`behaviors::drop_folder`) | = (fixed) |
| A drag off the list | the drop line stays at its last spot; released over the canvas: nothing | the line went with the pointer's row (drawn by `drag_over`); now from the drop slot | = (fixed) |
| Folders: sideways | every 19 px sideways in the slot steps a level: a last child's bottom half 19 px left drops after the folder, outside | no levels; now 19 px a level from where the pointer came onto the slot (`LIST_LEVEL_PX`) | = (fixed) |
| Multi-part Properties… | one "2 selected" dialog, both parts listed, Name "Multiple values", shared fields editable (live 08:18, 2962) | the first part's Properties; now live's dialog (`open_parts_properties`; list `several_parts_share_one_properties_dialog`) | = (fixed 11-13) |
| Parts list rename (live2 parts-list-1118) | row menu › Rename: the whole name selected; Esc cancels, Enter commits, a click away commits; Shift+N on a selected part renames, F2 nothing | the same, but a click away left the box open; now it commits (`on_mouse_down_out`, as the feature rename) | = (fixed) |
| A part hovered in the graphics (live2 parts-list-1118 c1) | its row gets no hover or related tint (list → graphics only) | the same | = (new) |
| While a sketch is edited (live2 sketch-entry-1118) | the rows after it grey italic, Parts (2) -> Parts (1), ✗ brings them back; another feature row clicked does nothing | the rows and parts the same; a row click selected the feature; now nothing | = (fixed) |
| Delete on a used sketch (live2 keys-1118) | no dialog; its user red, the tooltip "Extrude 1 did not regenerate properly: Select face or sketch region to extrude." + "1 missing selection" | the same | = (new) |
| Tab from the graphics (live2 keys-1118 (2)) | after a click on empty graphics: Show measure details → Show analysis tools → Display mass and section properties → out of the page, then Share; Shift+Tab back; Space / Enter press the focused button; no list row or toolbar button is a stop | Tab did nothing; now those three stops, then the top bar (`StudioTab`, `eval_tab`), Space / Enter their click | = (fixed) |
| Not yet in the matrix | the 6 px drag threshold (Chrome's, not Onshape's); an empty new folder's caret (live featurelist-gaps 11-16: shown, collapsed, on "New folder (0)"); the insertion line (live featurelist-gaps: rgb(22, 81, 176), 2 px, the row's width, a ~6 px ring filled rgb(202, 216, 237) at its left end, centred on the boundary above the hovered row in its upper half, below in its lower half; ours the same, the ring's fill was white) | – | – |

## Measure (matrix)

live2's measure.json (11-13 06:50–06:56) on live's own document, rebuilt (Part 1 40 × 30 × 10 with an r4 hole at
(20, 15), a 2 mm fillet on its x = 0 top edge, a 1 mm chamfer on its x = 40 one; Part 2 x 60–80 × 20 × 15; Part 3
100–110 × 10 × 5; live's "Center dist 43.875" from (60, 0, 0) places the hole): a selection and the summary bar it
gives, at a device pixel ratio of 1 and 2. part-studio-ui `measure::the_measure_matrix_matches_live` (T1, no GPU,
entities found with `query_entities` by a point on them); `MEASURE_ROW=<part of an id>` runs some rows.

**Matched: 23 / 23** (11-13 15:51 UTC; 15 / 17 at the first run; 5 rows added 15:51: 2 / 5 matched, 3 fixed). ui-parity: the 12 measure scenarios 12 / 12.

| Row | Live | Ours | Status |
|---|---|---|---|
| One vertex / straight edge / circular edge / face | Point (X, Y, Z) / Length 40.000 / Diameter 8.000 / Area 398.642 mm² | the same | = |
| Two vertices / a vertex and a circle | Min dist 20.000 / Min 39.990, Max 47.778, Center 43.875 | the same | = |
| Two circles | Min dist, Max dist, Center dist | the same (was Center, Max, Min: `measure::quick_line`) | = (fixed) |
| Parallel faces / a face and a parallel line / collinear edges | Parallel dist 5.000 / 10.000 / 0.000 | the same | = |
| Perpendicular edges / faces / a face and a plane | Perp lines angle 90.000 deg / Perp planes angle 90.000 deg (×2) | the same | = |
| Three vertices, a part, a feature | nothing | the same | = |
| The details panel open (`[`) | the summary bar empty; `[` again closes it | the same | = |
| A value clicked | 5 decimals ("20.00000"); again: 3 | the same (was nothing; `ui.measure_precise`, per selection) | = (fixed) |
| A value clicked: Ctrl+C | its text selected: Ctrl+C copies "20.00000" | nothing copied; now the number shown (`ui.measure_text`) | = (fixed) |
| A value hovered | its 5 decimals' tooltip and its measurement's dashed line | the tooltip only (the line only from the open panel's rows); now both (`MeasureHover`, the hover on a wrapper: on the value its redraw kept the tooltip shut) | = (fixed) |
| The panel's rows hovered | a distance row: the dashed line; an X / Y / Z row: the legs dashed in X red, Y green, Z blue, zero ones left out | the line only; now the legs too (`measure_hover_lines`) | = (fixed) |
| The panel's size | 220 × 345 with a distance and its X / Y / Z rows | 220 × 347 (6 px above the footer); now 4 px, 345 | = (fixed) |
| A second entity hovered | no preview: the first one's readout | the same | = |
| Not yet in the matrix | a click near an edge's middle takes its midpoint: live measure-gaps (11-16) 8–9 px either side along the edge (10 px takes the edge), the mark from the first shot (~100 ms, no dwell needed); ours the 9 px window (`midpoint_target_tests`) and a 60 ms dwell (was 250; 60 ms is Onshape's hover wake, live sketch-gaps) | – | – |

Added 11-18 (live2 measure-mass-1118; part-studio-ui `measure::measure_pairs_on_lives_wedges_and_cylinder`, 22 pairs
on live's own studio, all as live): two faces or two edges read the angle between the directions from where they meet
toward each one's middle (a sloped face and y = 0: 33.690°, was 146.310°; confirmed by live2 on the first wedge 11-18: its sloped face and x = 40 read 56.310°, where π − the normals' angle would give 123.690°); a cylinder and a face across it read
"Perp axes angle" alone (was with Max / Min dist); a circle and its face keep the distances after "Face tangent angle".
Mass properties (`mass_stays_open_and_bracket_opens_an_empty_measure`): its button again leaves it open (was a toggle);
`[` over it opens Measure empty, the parts deselected (the selection was kept).
Number display (live numfmt-1118, live's `formatValue`; units `live_format_tests`): a value rounds as
`Math.round(v·10^p)/10^p` (12.3455 → "12.346", −2.5 at 0 → "-2"; ours rounded half away from zero), trailing zeros
dropped (the measure readouts keep `toFixed`'s, nudged by 1e-10); |v| ≥ 1e6, ≤ 1e-5 or rounding to 0 reads in exponent form ("4.643e+7"); compound units sorted and spaced
("deg mm", "mm⁴"); a dragged angle's box at the angle precision (3); a Mass properties value clicked shows the unit's
highest precision (mm 5, cm / in 6, ft / yd 7, m, areas, volumes and kg 8, g 5), clicked again as before
(`a_clicked_mass_value_toggles_the_highest_precision`). Before: no exponent form, "mm*deg", no toggle.
Working spinner (live2 11-18, live's JS and CSS): 900 ms after the request (was 1.1 s), a 50 × 50 box white at 75 %,
radius 10, 40 px above the bottom, the 30 px ring rgb(215,215,215) with its blue quarter turning in 1.1 s (was 0.9 s).

Configurations (live2 cfg-1118): a second row named like another is accepted, stored as "A0" (ours "A_2": fixed,
`fs_option_identifier`); a value a row can't take waits until that row is active, then its feature fails
(`a_duplicate_configuration_name_and_a_bad_value_wait_for_their_row`). Already as live: the 712 px docked panel, the
feature list's Configuration box, the row menu (Switch to / Set as default / Duplicate row / Move up / Move down /
Delete row), a row click doesn't switch, switching greys no rows.

Manipulators (live manip-feel-1118 and live2's 11-13 "feel (feature dialogs): manipulators"; checked 11-18): a plain
click on Extrude's arrow flips the direction (dialogs matrix); mid-drag nothing under the pointer lights (the viewport
asks no hover pick during a tool drag, and the handle hides geometry hover); the Depth field keeps the keyboard with its
text selected during and after the drag (`focus_field_selected`); only Shift changes a drag (no step), Ctrl and Alt
nothing; Esc with the button held cancels the whole dialog (dialogs matrix, new row); Transform › Translate by XYZ:
a planar handle drags both of its axes, the other handles fade to faint grey while dragging, the moved parts' old
place shows as a faded ghost with the selection outline (triad.rs, behaviors `transform_ghost`). All as live.

View tools (live2 view-tools-1118): Shift+Y shows the selected hidden parts only (ours already: `show_hidden`, the
action named ShowAll; nothing selected, only a hidden part under the pointer); a hidden part's row menu has Show, Show
all parts (no Hide other / all parts: fixed); the empty-space menu Show all, Show all parts (as ours); render mode and
Perspective switch in one step, a named view's restore animates (ours 500 ms; live's 3 frames over ~240 ms on a slow
lane); Section view: the arrow's value box, "Select default plane", the dialog's caps 80 % toward #FFC685 (E9C698 on
the blue block, EBBD8A on the grey cylinder, as ours); Turn off then Shift+X brings the last plane and offset back
(fixed: `section_last`; `shift_x_after_turning_off_restores_the_last_section`).

## Navigation (matrix)

live2's navigation.json (Linux Chrome, the account's default mouse mapping): wheel, right / middle / Ctrl+right /
Alt+right drags, view keys, animations, double middle click, acted out with real events in a Part Studio (a
40 × 30 × 10 block, orthographic) at a device pixel ratio of 1 and 2. part-studio-ui
`nav::the_navigation_matrix_matches_live` (T1, no GPU; the viewport's press and animation clock is the test's,
`Viewport::set_clock`); `NAV_ROW=<part of an id>` runs some rows.

**Matched: 20 / 20** (11-13 14:14 UTC; 9 / 14 at the first run).

| Row | Live | Ours | Status |
|---|---|---|---|
| Pick ranking and reach (live's client code, behaviours "pick and snap tolerances", 11-18) | the square round(5·dpr) px each side of the pointer's pixel; priority first, then (1 + distance)·(1 − coverage) | nearest pixel after priority, the radius truncated (7 at 1.5×); now live's ranking and rounding (render `picks_rank_by_priority_then_distance_and_coverage_as_live`) | = (fixed) |
| Device-px thresholds (live's unsuffixed constants) | drag start > 2 device px after 50 ms; the right-press menu cancelled past 4 device px of summed move path; hover picks at once below 0.15 device px/ms | CSS px, and the menu used the straight distance; now device px and the summed path (app-core viewport press tests) | = (fixed) |
| Dragged angle snap (live W@480991) | within 0.4·(1 + level/3) steps: 2° at the 5° level; whole 1° and 0.1° steps finer | ÷ instead of × at the finer levels (free in between); now always rounded there | = (fixed) |
| Wheel notch in / out | the view scale × 2^(∓1/12) on Linux and Mac, 2^(∓1/4) on Windows (×3 there) | the same | = |
| Wheel toward the pointer | the point under the pointer stays (≤ 1 px), zooming out too | the same | = |
| Sideways wheel | ignored | the same | = |
| Right drag | 3 turns per canvas diagonal (0.57049 °/px on 1673 × 886), about the part's point under the press (≤ 0.4 px), no world-up lock | the same; a drag released before the GPU's depth sample came lost its moves (now applied when it comes, or after 250 ms about the fallback centre) | = (fixed) |
| Right drag from empty space | about the view centre (its point moved 2 px) | the same: the visible depths' average, else the model's centre (≤ 2 px) | = |
| Shift + right drag | rotates as a plain right drag | the same | = |
| Alt + right drag | turntable: an upright view's direction keeps its z | the same | = |
| Ctrl pressed / released mid-drag | pan while held (1:1), rotate again after | the same | = |
| Middle drag, Ctrl + right drag | pan 1:1 | the same | = |
| Arrows / Shift / Ctrl | 15° / 90° / 5° in 250 ms, every key repeat turns (Ctrl stays Control on a Mac) | the same | = |
| Ctrl+Shift+arrows | pan 150 px in 250 ms | the same | = |
| Shift+1 / 5 / 7, F | 500 ms, zoom to fit | the same | = |
| Shift+1..6 named views | direction / up: Front (0,1,0)/+Z, Back (0,−1,0)/+Z, Left (1,0,0)/+Z, Right (−1,0,0)/+Z, Top (0,0,−1)/+Y, Bottom (0,0,1)/−Y | the same | = |
| N | normal to the selected face in 500 ms; again: 180° | the same | = |
| Easing | smootherstep t³(6t² − 15t + 10) | the same | = |
| A new animation while one runs | ignored until it ends | replaced the running one; now ignored | = (fixed) |
| Double middle click | two middle releases within 500 ms fit (150 ms did, 600 ms didn't) | the OS's double click count; now live's own 500 ms timer | = (fixed) |
| Wheel, Ctrl+wheel, pinch (live nav-1117) | one wheel event = one 2^(1/12) step about the pointer whatever its deltaY (1–240), not animated; Ctrl+wheel the same; a pinch one step per update (5 updates ×1.335 either way); a precise trackpad proportional (0.5 · wheelDelta / 120, ×2^(x/6)) | the same (`wheel_zoom_amount`, `pinch_zoom_amount`) | = |
| View changes below 20 fps (live nav-1117) | an average frame over 50 ms: the view jumps, no animation (500 ms views / F / cube / 180, 250 ms arrows and keys, 125 / 250 ms levelling otherwise, smootherstep) | animated at any rate; now jumps (`navigation::view_change_duration`) | = (fixed) |
| View cube clicks from iso (live viewcube-1118, a 10 px click grid) | faces their views; a corner the Trimetric view turned to its quarter (azimuth 15° short of the corner's, ±30°: the front-right-top corner Trimetric (−0.433, 0.75, −0.5)); the hidden faces' bands past the silhouette theirs (the bottom ~30 px under the cube); the arrows' click areas larger than drawn (a triangle's 61–79 px out, ±12.5; a roll's 59–78 px over 41–73°), 15° each | a corner looked down its diagonal (iso: read from a ui-parity step whose "Isometric" click wasn't on the cube) and the arrows took only their drawn shapes; now as live (`CubeCell::rotation`, `Arrow::hit_poly`); from other views asked of live (REQUESTS 21:30) | = (fixed) |
| View cube press and drag (live viewcube-1118) | a press released within 2 px is a click; 3 or 5 px nothing; a 40 px drag neither orbits nor clicks (the cube moves); a press held 1.2 s without moving nothing; **the limit ≈ 0.5 s (0.2–0.48 s Top, 0.5 / 0.6 / 0.8 / 1.0 s nothing; live viewcube-1118 hold-fine)** | the same (`CLICK_TOLERANCE`, the cube's own drag) except the hold: any hold still clicks (c3d3470d2: 1.2 s → Top) | ≠ (hold) |
| View cube clicks from iso (live viewcube-1118, a 10 px click grid) | faces their views; a corner the Trimetric view turned to its quarter (azimuth 15° short of the corner's, ±30°: the front-right-top corner Trimetric (−0.433, 0.75, −0.5)); the hidden faces' bands past the silhouette theirs (the bottom ~30 px under the cube); the arrows' click areas larger than drawn (a triangle's 61–79 px out, ±12.5; a roll's 59–78 px over 41–73°), 15° each | a corner looked down its diagonal (iso: read from a ui-parity step whose "Isometric" click wasn't on the cube) and the arrows took only their drawn shapes; now as live (`CubeCell::rotation`, `Arrow::hit_poly`); the same from Front, Top and Trimetric (live 11-18 21:5x) | = (fixed) |
| View cube press and drag (live viewcube-1118) | a press released within 2 px is a click; 3 or 5 px nothing; a 40 px drag neither orbits nor clicks (the cube moves); a press held 500 ms or more without moving nothing (480 ms still clicks) | the same (`CLICK_TOLERANCE`, `CLICK_HOLD` 490 ms, the cube's own drag; was: any hold clicked) | = (fixed) |
| Not yet in the matrix | inertia (a flick keeps turning, ×0.77 per 16.67 ms; none held still 50 ms; none below 20 fps: it reads real time and the frame rate, so not a T1 row; navigation `inertia_decays_like_onshape` checks the decay); Alt+right on a tilted view animates upright first; the view cube (150 px, 15 px margins, face / edge / corner 500 ms, arrows 15° in 250 ms, curved arrows roll, hover fill) and its menu; live navigation-gaps2 (client source): Alt+right levels the view on the first drag move over 125 ms (tilt ≤ 45°) or 250 ms (> 45°), smootherstep, drag moves ignored until it ends (ours the same: `level_view`, `CAMERA_SNAP_ANIMATION` / `CAMERA_SHORT_ANIMATION`, `AxisTag::Levelling`) | – | – |

## Chrome (matrix)

live2's chrome.json and chrome-shortcuts.json (11-13 07:18–07:31). Two tests: app-core
`actions::shortcut_map_tests::the_shortcut_map_matches_lives` (T1: every key of live's map for the `all`, `partStudio`,
`sketchEdit` and `assembly` scopes looked up in our keymap in that context, the Windows / Linux keys and the Mac keys
both checked on every OS) and part-studio-ui `chrome::the_chrome_matrix_matches_live` (T1, no GPU; tooltips on the
executor's clock).

**Shortcut map: 196 / 196** (11-13 11:20 UTC; 194 / 196 at the first run: the mate connector dialog's A / Q, bound
in its dialog as live acts on them). Live's Feedback / Report a bug (Ctrl+U) and Open Debug Panel are cloud /
debugging rows, left out. Ours binds besides live's map, in those scopes: the desktop shell's New / Open document,
Close window, Quit (secondary-n / o / w / q) and the dialog Tab / Enter / Shift+Enter keys live's client handles
outside its map (its "client side entries").

**Chrome: 23 / 23** (11-18 04:04 UTC: live2 tooltips-1118's placement row, fixed; 22 / 22 at 11-16 07:11 UTC: the tooltip's fade-out from chrome-gaps; the toolbar states row (11-16) checked by screenshot, not in the matrix; 21 / 21 at 11-13 15:51 UTC, 18 / 18 at 14:01; 14:01 UTC, was 17 / 17 at 13:33 UTC; 7 / 11 at the first run, then 5 rows added: 1 / 5 matched).

| Row | Live | Ours | Status |
|---|---|---|---|
| Tooltip delay | 400 ms (fully shown 470–545 ms with the fade) | 500 ms; now 400 (vendored gpui `DEFAULT_TOOLTIP_SHOW_DELAY`) | = (fixed) |
| Warm switch | while one shows, or within ~200 ms of it going, the next button's shows at once | always the full wait; now at once (gpui `tooltip_warm`, executor clock) | = (fixed) |
| Cold again | after a pause, the 400 ms wait again | the same | = |
| Longer help | a toolbar tooltip's description 2000 ms after it shows | 1500; now 2000 (`ui::TOOLTIP_EXPAND_DELAY`; a menu row's side tooltip keeps 1500: live fs-ps-ws-feature-tooltip) | = (fixed) |
| Toolbar tooltip place (live2 tooltips-1118) | 3 px under the button, centred on it; expanded 400 px wide, centred, the same top, clamped at the page edge only if needed; no image; stays while hovered (12 s) | the same for middle tools, but centred through a 400 px strip snapped to the window: a left tool's short tip sat off-centre; now centred by its measured width (`RichTooltip::width`) | = (fixed) |
| Feature dialog place | graphics area's top left, 3 px down | the same | = |
| Feature dialog drag | clamped inside the graphics area; within ~10 px of the top / right / bottom it snaps (3 / 0 / 2 px); the left doesn't | could leave the area by all but 80 × 40 px, no snap; now live's (`behaviors::place_feature_dialog`) | = (fixed) |
| Select other place | pointer + (100, 51) | the same | = |
| A tooltip on a click or the wheel | closes | the same | = |
| A tooltip on a drag's start | closes | the same | = |
| Tooltip fade-in | ~150 ms | none on toolbar tooltips (only list rows' 230 ms); now 150 ms (`ui::TOOLTIP_FADE_IN`); its fade-out isn't drawn (gpui drops a tooltip at once) | = (fixed) |
| Tooltip fade-out | Bootstrap's 0.15 s fade after the pointer leaves, the node gone 165–200 ms after (chrome-gaps 11-16) | gone at once (gpui dropped a tooltip on leave); now faded out over 150 ms (vendored gpui: `TOOLTIP_FADE_OUT`, the paint's element opacity) | = (fixed) |
| Toolbar button states (user P1 11-16; live2 toolbar-states, live's addendum) | hover #eaeaea; a running tool #eaeaea with `inset 0 1px 1px rgba(0,0,0,0.247)`, hovered flat #dfdfdf (a sketch tool joined to its ⌄ #cecece); hovering a split tool's ⌄ lights the tool #eaeaea and the arrow #d7d7d7, an open ⌄ #d7d7d7 with the inset; Search tools' highlight #fcda7f with a 2 px #fab601 border; icons' light faces opaque white in every state | the running tool had no inset and stayed #eaeaea hovered; a split tool and its ⌄ lit #d7d7d7 together; the highlight's border 1 px; ~100 icons' light faces see-through (the yellow / grey showed through) and their greys translucent; now as live (`ui::pressed_shadow`, `caret_variant`, `search_highlight`, vendored Button `pressed_inset` / `flat_hover`; icons `data-face` underlays, app-core `icons_with_faces_are_opaque_inside`) | = (fixed) |
| Feature dialog place, kept | every feature dialog, in every tab and session, opens where the last one was left | per tab, lost on closing; now one place (`behaviors::feature_dialog_pos`, config feature_dialog_place.json, written when a move ends) | = (fixed) |
| A submenu (▸ row) | opens on hover with no delay (one 10 ms poll); closes ~30 ms after the pointer moves to another row | opens in the frame the row is hovered, closes in the next | = |
| Select other size | 220 px, 24 px rows in a 90 px list; nothing found: 83 px tall | the same, but the empty dialog kept the 92 px list (149 px); now one row (`SELECT_OTHER_EMPTY_H`) | = (fixed) |
| Context menu place | pointer + (0, 2) in the graphics; + (10, 12) on a feature-list row | (0, 2) everywhere; now (10, 12) on feature and folder rows (`os_menu::MENU_AT_ROW`) | = (fixed) |
| Context menu flip | at the right edge its right side 1 px left of the pointer; at the bottom its bottom 4 px below it | flipped round where it opened (pointer + (0, 2)); now round the pointer (vendored gpui `anchored().flip_point`) | = (fixed) |
| Feature list width | remembered per tab, the default 205 elsewhere | one width for every tab; now per tab (`ui::left_panel_w(tab)`, config left_panel_widths.json) | = (fixed) |
| Toolbar dropdowns | on click only, under the toolbar at the tool group's left edge, staying open | under the caret (31 px right of the group's left); now at the group's left (vendored gpui-component `PopupMenu::shift_x`); 4 px under the buttons as live | = (fixed; checked on a runner screenshot and by a row click, no T1 test) |
| Not yet in the matrix | the view cube's hover targets: live chrome-gaps' 3 px map (faces and corners only, the hidden faces' bands reaching ~6 px past the drawn silhouette, round knobs 35–60 cells) against ours 65 % of covered cells alike, now 84 % (`SILHOUETTE_REACH_PX` 6, `CORNER_HIT` 0.37; render `cube_hover_map::the_cube_hover_map_matches_live`) | – | – |

## Look

live2's look.json (11-13 07:34–07:47, light theme, DPR 1; constants in live/feel/client-constants-raw.json, screenshots
in live/feel/look/). Ink is measured as darkness summed down a pixel column through a line against the darker surface
beside it, times its slope (/root/psui-scratch/chrome/ink.py, ink2.py on screenshots of ours from the runner,
`shot.sh` on vibe6; render `edge_ink` offscreen). Measured 11-13 11:44–11:57 UTC.

**Matched: 16 / 16 rows compared** (2 fixed).

| Row | Live | Ours | Status | Evidence / test |
|---|---|---|---|---|
| Part edges | 1 px, ~0.84 px of ink at 1×, never solid black (darkest L ~60) | was 1.25 px (~1.15–1.2 px, solid black); now 1 px (`PART_EDGE_WIDTH`): 0.81 offscreen, 0.85–1.0 in the app | = (fixed) | render `part_edges_carry_lives_ink` |
| Back-face silhouettes | 1.2 × the edge (BACKFACE_SILHOUETTE_WIDTH_FACTOR) | was 1.25 px; now 1.2 × | = (fixed) | scene `cylinder_sides_show_silhouettes` |
| Inactive sketch lines | 1 px | was 1.5 px; now 1 px | = (fixed) | – |
| Active sketch lines | #0000d0, 2 px (1.73 px of ink on sketch-underconstrained.png) | #0000d0, 1.79 px | = | screenshots |
| Sketch region | #9aa5af 0.2 (212,220,227 over the plane) | 210,219,225 | = | screenshots |
| Sketch points | ~9 × 8 px blue dots | 9 × 9 | = | screenshots |
| Highlight colours | hover #ffc685, selected #ffa90a, selected part #ffc04c, secondary #f2ff80, error #bd3039 | the same (`palette.rs`; the selected part in shaders.wgsl, linear) | = | scene `edge_and_face_highlights_look_as_live` |
| Sketch colours | under #0000d0 (#4040ff seen through), fully #000, over #cd0000, inactive #999 | the same (`palette.rs`, `graphics_prefs.rs`) | = | – |
| Construction | the entity's colour, dash-dot 4-6-30-6 px | the same (graphics preferences) | = | – |
| Drawing preview | rgb(68,156,205) | #449ccd (`SKETCH_PREVIEW_COLOR`) | = | – |
| Constraint glyphs | 20 px white tiles 2 px apart, centre 20 px off the entity inside the shape, corners 17 px diagonally out | 20 px tiles on a 22 px pitch, 20.5 px off, corners centred (-17, -16) | = | – |
| Dimension values | Flama 18 px: digits 11 px tall, 8 px wide, 10 px pitch | 12 / 8 / ~10 px | = | screenshots (glyphs-show-constraints-x3.png) |
| Dimension arrows | 16 × 8 px (measured 16 × 7) | 16 × 7 | = | – |
| Planes, origin, axes | fill #cedbe5 0.2, outline #1d60aa 0.35, text #1651b0; origin #000; X #cd0000, Y #009400, Z #0000d0 | the same | = | – |
| Mate connectors | 36 px, 4 px ring | MATE_CONNECTOR_DIAMETER_PX 36, RING 4 | = | – |
| In-context references | rgba(212,3,249,0.3) / 0.6 | calibrated on live's screenshots (assembly/504–706: #c573e4 fill, #dc62f7 outline) | = | – |
| A whole part hovered (its Parts-list row; lead 11-18 pixel parity) | no tint, its edges as at rest, its silhouette outline #FFC685 2 device px at any ratio (live's overlay looks at the ±1 device px neighbours, unscaled; face hover measured 2 device px at DPR 1 and 2) | 2 px at 1×, 4 device px at 2× (the band's reach scaled with the ratio); now 1 at any ratio | = (fixed) | scene `edge_and_face_highlights_look_as_live` |
| A whole part selected (live2 hl-pixels-1118) | lit #FFC04C, no specular, #FFC04C over it at 50 %: top F2B648, front CD9A3D; its internal edges the plain dark edge under the tint (9D772F at 1×, 806026 = half #FFC04C at 2×); its silhouette a #FFC04C outline, 2 device px at any ratio, through the parts in front | top F0B648, front CB9A3D; internal edges were #FFA90A face borders (its faces were passed as selected faces) and the outline 3.5 px at 1× / 7 at 2× (pushed-out back faces); now the part's faces take the tint directly, its edges half amber, and the outline is the hover band's mechanism over the part's mask (state BAND) | = (fixed) | scene `edge_and_face_highlights_look_as_live` |
| A face selected with its part | the face E5A222 with its #FFA90A border, the part's silhouette #FFC04C | the same | = | the same test |
| Sketch entities (live2 hl-pixels-1118) | a line ≈2 px blue (4 device px at 2×); hovered a 5 px band (9 device px) with an opaque #FFC685 rim and a 0.78 core, selected the same in #FFA90A; a point ~5–6 px (ACTIVE_SKETCH_POINT_SIZE 6; a Point entity 8), hovered an ~11–12 CSS px disc with an #FFC685 rim and the dot inside, selected the same in #FFA90A | line 2 px (4 device px), its band 6 px at 1× (one rim falls between rows) and 9 device px at 2×; the corner dot 6.7 px (under-defined); the disc 10–11 px (21 device px) | = | part-studio-ui `sketch_hover::sketch_lines_and_points_light_as_live` (GPU) |
| Not compared | filtered-out entities (no selection filter in the Part Studio for live's account); a selected feature's related part (part-studio/110 compared when the feature list was done) | – | – | – |

Seen on the way: live's start "block" sketch shows all four rectangle lines blue (the rectangle not tied to the
origin); ours draws the two lines from the origin black, as live colours such a line when its corner is at the origin
(qsnap). So the two runners' starts likely differ (whether the rectangle's corner is tied to the origin), not the
colouring: to check.

## Sheet metal (matrix)

live2's sheetmetal.json (11-13 08:10–08:37, a 60 × 40 sheet metal model with two flanges and a tab). part-studio-ui
`sm::the_sheet_metal_matrix_matches_live` (T1, no GPU) on the sm-box start, the Sheet metal table and flat view
panel open, a fresh document per row; cells carry `sm-cell-<table>-<row>-<column>` debug selectors.

**Matched: 12 / 12** (11-16 02:59 UTC: the flat bend hover row from sheetmetal-gaps2; 5 / 7 at the first run). ui-parity: the 11 sm-* scenarios 10 / 11
(sm-form-tool: live's Form dialog opens on "Circle emboss", ours "Select Part Studio…"; the same on green).

| Row | Live | Ours | Status |
|---|---|---|---|
| Bends columns | #, Name, Radius(mm), Angle(deg), Bend direction, K Factor | the same | = |
| A row click | selects the row (cell) only, nothing in 3D | also selected the bend in 3D; now the row only (the table's highlight still lights it) | = (fixed) |
| Enter on a selected Radius / K Factor cell | edits it | only a double-click edited; now Enter too (`sm_enter_edits`) | = (fixed) |
| A Radius edit | adds "Modify joint 1" at the end of the list | the same | = |
| A bend row's menu | the first row: Move down, Convert Bend A to rip | the same | = |
| Convert to rip | the row moves to Other joints (Joint A), the flange its own part (Parts (2)) | the same | = |
| Convert to rip right after a Radius edit; a second Radius edit (live2 sm-1118, live ui (10-24 b)) | the bend's Modify joint 1 becomes the rip / takes the new radius: no Modify joint 2 | the same (`sheetmetal::bend_joint_edit`: the Modify joint its faces are named by once it has rebuilt the bend; ours added Modify joint 2) | ops e2e `a_bends_second_edit_and_its_rip_reuse_its_modify_joint` | = (fixed 11-18) |
| A row hovered | lights its bend in the 3D and flat views | the same | = |
| Flange | no length arrow, a preview wall | the same | = |
| Flange / Hem edges box hover (live 11-17 sm-feel-1117) | rim and bend edges and a rip's thin side face light; a wall's broad faces and planes never | the same (`pick_muted`: the walls' faces out of the picks while that box is active; were lit) | = (fixed 11-17) |
| A joint row hovered (live 11-17 sm-feel-1117) | lights the rip in the 3D view and both its edges in the flat view | the same (`sm_rip_refs`; the flat edges `flat_view_side_edge`: each side face laid flat with the wall it closes; was nothing) | = (fixed 11-18) |
| Flange's flip (live 11-17 sm-feel-1117) | the opposite-direction arrow button at the right of the [Bend angle ▾] row; nothing in the graphics | the same (a flip after a row joins it) | = |
| Bend relief / Corner boxes hover (live 11-17 sm-feel-1117) | a rip's side face filled orange, rim / bend edges, a bend end's corner as an orange disc; the broad faces, planes never | the same (`SM_CORNER` takes planar faces, the walls' faces muted as Flange's box; was vertices and edges only) | = (fixed 11-18) |
| Type Rip → Bend that can't unfold (live 11-17 sm-feel-1117) | "Modify joint 1" added, the joint a bend (the corner shown bent), the yellow toast "Sheet metal model cannot be unfolded.", the flat view blank | the same (the host's unfold: a wall a second bend path places elsewhere (`Unfolded::clashes`) means no flat, `cannot_unfold`, the feature's warning SHEET_METAL_COULD_NOT_UNFOLD, listed as failed as live's (red, "did not regenerate properly", live 11-18 sm-rip-1118 (a)); a consistent loop, a cut splitting the plate between two flanges as qa4 S1, still unfolds; was a flat with the loop cut and no message). live2 read the feature as red: ours shows the warning (std's defineFeature rolls an ERROR back, so the bend wouldn't stay; hole.fs reads this status as a warning) | = (fixed 11-18; the row's colour to recheck) |
| Corner / Bend relief with a rip's side face picked (live 11-18 sm-rip-1118 (b)) | taken as the rip's edge, chip "Edge of Sheet metal model 1", ✓ enabled; the feature fails "Sheet metal corner not found." / "Selection is not a bend relief." | the same (the face's long edge goes in, `side_face_edge`) | part-studio-ui `a_rips_side_face_in_corner_or_bend_relief_is_its_edge_and_fails_as_lives` | = |
| The flat view | its own camera: the wheel zooms it alone | the same | = |
| The flat view's start | looking straight down (Top) | the same | = |
| A click inside a bend's zone in the flat view | selects that bend in both views and its Bends row | only the centre line (5 px) took a click, else the whole flat face; now the bend's face, line and row (`FlatViewEvent::BendZone`, as Select other's "Face of …") | = (fixed) |
| A bend zone hovered in the flat view | lights that bend in both views rgb(255, 198, 133), not its table row; on the dashed centre line nothing (sheetmetal-gaps2) | lit nothing (only the flat face's outline); now as live (`FlatViewEvent::BendHover`) | = (fixed) |
| Not yet in the matrix | the panel's open time (0.9–1.4 s live on a software GPU), the flat view re-fitting, its bend lines (rgb(47,47,47), dashed centre), the Make joint / Corner / Bend relief picks, Tab filling its fields from the sketch; live sheetmetal-gaps2: flat bends' tangent lines solid 1 px, the centre line dashed 10 / 10 / 5 / 10 px at every zoom (ours now `LineSegment::CENTER_LINE`, was the 16 px dash-dot); a click selects the row (frame rgb(22, 81, 176)), the zone filled rgb(214, 151, 30) with an rgb(255, 169, 10) outline (ours the selection colour) | – | – |

## Sketch tools (matrix)

live2's sketchtools.json (11-13 09:12, Sketch 2 on Top of a block copy). part-studio-ui
`sketchtools::the_sketch_tools_matrix_matches_live` (T1, no GPU); Search tools' ranking through
`noshape_ops::tool_search::studio_rank` over the toolbar's feature types.

**Matched: 18 / 18** (11-13 19:30 UTC, 17 / 18 at 14:14 UTC; 10 / 10 at the first run, then 8 rows added: 6 / 8 matched, 1 fixed).

| Row | Live | Ours | Status |
|---|---|---|---|
| Search "fil" | Fillet, Fill, Finish flex PCB model, Finish sheet metal model, 3D fit spline, Modify fillet | the same | = |
| Search fillet / chmf / lin pat / ext / sk / xyz | Fillet, Modify fillet; Chamfer; Linear pattern; Extrude, External thread; Sketch; nothing | the same | = |
| Line chains | each click after the second continues from the last end | the same | = |
| Circle stays | two circles without re-picking the tool | the same | = |
| One Esc mid-chain | exits the tool, the sketch stays open | the same | = |
| Space mid-chain | nothing; the chain continues | the same | = |
| Enter mid-chain | the length box becomes a driving dimension on the segment; the chain continues | the same | = |
| The menu mid-chain | Escape line, Confirm Sketch 1, …; the tool stays | the same | = |
| Esc with that menu open | sketch/405–408: the first Esc exits the tool, the menu stays, a second closes it (live2's earlier note read otherwise: REQUESTS 12:33 answered, sketch/405–408 stands) | the same as sketch/405–408 | = |
| Shift+F | the sketch fillet | the same | = |
| Trim, a click | removes the piece between the crossings | the same | = |
| Trim, hover | the piece up to the nearest crossings lit tan, orange dots at its ends | the same | = |
| Trim, hover reach (live2 sktools-1118) | the piece lights tan rgb(214, 189, 149) with the pointer 3–6 px from it, not at 7; the other piece never | the piece to 6.5 px (`in_sketch_reach`, 7.5 for other tools), drawn `TRIM_PIECE` #D6BD95, its end dots orange; only the piece under the pointer | sketchtools_feel_tests "trim / hover reach" | = (fixed 11-18) |
| Trim, a drag | every piece the trail crosses goes; one that nothing crosses any more goes whole (tr0) | the same | = |
| Trim past a chain's corner (live sktools-1118, recheck b5b664aa) | a line passing ~1 px by a chain's corner, outside it or across it (0.1 mm = 0.6 px, crossing both segments 0.12 mm from the vertex), doesn't cross there: a click trims it up to its crossing with the next segment | the same (fixed 11-18: a curve passing within 1.5 px of a corner where two curves end doesn't cross them near it, `trim_near`; ours stopped at the corner when the line crossed it) | scene `a_trim_past_a_chains_corner_goes_to_the_next_crossing` | = |
| Extend, hover | the line lit, its nearer end marked; no extension yet (live 11-18 sketch-tools) | the same (the nearer end's dot, `nearer_line_end`) | = |
| Extend, a line's end picked | the click picks the nearer end (the line yellow-green); a light-blue extension follows the pointer along the line, shorter too when the pointer is back over it; past a crossing curve it stops on it (orange square), and the second click ends there, COINCIDENT, even 42 px beyond (live 11-18 sketch-tools t4-ext, t7) | the same (`extend_end`, `extend_target`); was: the curve, then a boundary curve's infinite line | = (fixed 11-18) |
| Tangent arc | always tangent: the pointer's path chooses nothing, the end point alone sets the arc; from a line's start it continues backwards; an end nearly in line gives a huge radius (R1575.9 mm) (live 11-18) | the same (`Sketch::tangent_arc`) | = |
| Spline, Enter | nothing: the pending spline and the tool stay; Esc commits it (live 11-18) | the same (`enter_target` Nothing) | = |
| Sketch fillet, the corner's reach and two lines | the corner lights at 10 px (not 14); a click on a line selects it, a second line sharing a corner with it previews the fillet there (R5) (live 11-18) | the same (`corner_click`: 10 px, the two picks) | = |
| Offset | outward from a closed loop by default; a digit typed over the preview does nothing; Enter commits at 5 mm and opens the box "5 mm"; a 27 px drag of the arrow (5.08 mm) from 5 mm gives 10.1 mm, on 0.1 mm steps (live 11-18) | the same (`OffsetArrow { from }`, `drag_step`: the feature arrows' zoom rule, 0.1 mm at that scale); the drag took the pointer's distance from the curve, unstepped | = (fixed 11-18) |
| Text | two clicks (corners), then its dialog with "Default text" | the same | = |
| Search tools in a sketch | the sketch tools (Offset o, Linear / Circular pattern, Mirror, Text, Insert image, Trim, Extend) | the same | = |
| Sketch fillet, a click on a corner | a preview, nothing made; Esc cancels | the same | = |
| Sketch fillet, a drag from the corner | made on release (arc, tangents, radius; its radius box opens) | only previewed (a second click made it); now made on release (`SketchEditor::corner_pressed`) | = (fixed) |
| Sketch fillet, the radius from the drag (live2 sktools-1118) | the arc passes through the pointer: R = the distance from the corner along the bisector / (√2 − 1) at a right angle (20 px at 0.162 mm/px → R 8.19759, 40 px → 15.93494), no rounding; too large: nothing made, the bubble "The current fillet radius produces invalid geometry." | the arc's middle follows the pointer along the bisector: R = d / (1 / cos(θ/2) − 1), √2 − 1 at a right angle, unrounded; too large: red preview, released nothing made and that message | sketch_mode `a_default_radius_too_large_for_the_corner_previews_red`, `fillet_overdrag_turns_red_and_drags_back` | = |
| Not yet in the matrix | Search tools' panel (Recently used, the info box, Highlight tool in toolbar), the pattern labels (3x / 1x, spacing, inline boxes), Text's dash-dot rectangle and dialog layout (font list, "Default text" selected, n/250 characters: ours has them), Insert image's tabs and aspect-kept rectangle; live sketch-gaps2 / 3: pattern labels near-black 16 px regular on opaque canvas-colour boxes (30 × 28 "3x", 30 × 26 the spacing), placed ~100 px off at the zoom the tool started at and then fixed in the model (the spacing dimension 99.8 px to the side, "3x" (+97, +99.8) px from the 2nd instance, a 1 px leader to it, no arrowhead), a circular "3x" 0.4 × the seed's radius beyond the seed; live sketch-gaps4: the circular handle arc at 1.2 × the radius from the seed to the last instance with its open arrow ~20 px on, and below 360° an angle dimension (black arc with filled arrowheads at 1.4 × the radius, blue rays to the handle arc, black extension lines, its text "180°" ~0.64 of the way along: one take), none at 360° ; live sketch-gaps5: the pattern runs clockwise (closed 3x: the arc's end on the first instance after the seed, 240° round; open 4x at 180°: 78.69°, 18.69°, −41.31°, −101.31°), the arrow drags in 5° steps and can't turn it the other way (back onto the seed it closes) (ours the same since 11-16: `pattern_count_labels_sit_where_live_puts_them`, `circular_handle_arc`, `circular_way`; was counter-clockwise, a dashed arc 18 px out, the angle 34 px out at mid-span) | – | – |

## Editing (matrix)

live2's editing.json (11-13 09:27). part-studio-ui `editing::the_editing_matrix_matches_live` (T1, no GPU; outside a
dialog the workspace takes Ctrl+Z, so the document's own undo stands in for it there).

**Matched: 9 / 9** (11-13 14:14 UTC; nothing to fix).

| Row | Live | Ours | Status |
|---|---|---|---|
| Undo in a sketch | one step per chained segment (3 undos for 3) | the same | = |
| Undo a rectangle | one step (4 lines and their constraints) | the same | = |
| After ✓ | one undo removes the whole sketch feature, redo brings it back | the same | = |
| Ctrl+C / Ctrl+V of a feature | nothing (no feature, dialog or toast) | the same | = |
| Delete a sketch another feature uses | no warning; the extrude turns red, Parts (0) | the same | = |
| Delete a part | "Delete part 1" (behaviours 1.12: its dialog opens pre-filled) | the same | = |
| Y / Shift+Y with nothing selected or under the pointer | nothing (Shift+Y isn't "show all") | the same | = |
| Sketch Ctrl+C, Ctrl+V | the copy's centre under the pointer at once, selected; one undo removes it | the same | = |
| Eye icons | a row's eye toggles it (Part 1 hidden, shown; a hidden sketch shown); the selection stays | the same | = |
| A hidden sketch's row at rest (live errors-1118 list.png) | no icon at its right end; the eye shows while the row is hovered | a crossed-out eye on every hidden sketch's row at rest; now hover-only as a shown one's | = (fixed) |
| A long tooltip (live errors-1118 tip-chamfer-full) | at most 400 px wide, wrapped: "Chamfer too big did not regenerate properly: Failed to chamfer / selections." 400 × 44 (12 px, 18 px lines, weight 500, 4 px padding, #333 on #D0DBE5) | one line however long; now wraps at 400 px (`PlacedTooltip`) | = (fixed) |
| Sketch Ctrl+C / Ctrl+V of a rectangle (live2 11-18 keys-1118) | the copy at once, its bounding box's centre on the pointer, selected, Transform on with its triad on the lowest-left corner; it doesn't follow the pointer; its constraints come along (4 coincidents → 8); no toast; Ctrl+A selects nothing | the same (`paste_clipboard`, `Sketch::extract`; no Ctrl+A binding) | scene `copy_paste_selection` | = |
| Not yet in the matrix | the toolbar's Undo tooltip (always "Undo (cmd+z/ctrl+z)": ours the same), undo in a feature dialog (an existing test), the paste's 2D triad and kept constraints, the assembly rows (asm's); live editing-gaps2: a sketch paste lands at once centred on the pointer, keeps every constraint wholly inside the copy (dimensions too), drops those to entities left out; live editing-gaps3 (the client's triad): the 2D manipulator (paste and Transform alike) sits on the selection's lowest key point (curve ends, centres; the leftmost of equals), grey double outlines, screen-sized: X / Y arrow heads 68–96 px out (move along that axis), the square 20–45 px (move freely), the rotation ring 108 px out on X with two ticks (rotate about the anchor, an angle box), the scale head from (60, 60) px, 30 long (scale, a factor box); the ring at the anchor moves the triad only (ours the same since 11-16: `transform_triad_sits_and_moves_as_live`; was the bbox centre, a 14 px square, rotate 70 px, scale 45 px, no arrows; the agent API's `sketch_transform` pivot defaults to the same anchor). live editing-gaps4: the rotation and scale show in Onshape's quantity input (75 × 26 px, white, blue frame, 13.33 px text right-aligned: "20 deg" in whole degrees, the factor to 8 decimals), rotate's off the pointer while dragging then off the anchor, scale's left where its drag ended; both stay, their text selected for typing; a double click on the ring or the head does nothing (ours the same since 11-16: `XF_*_BOX_*`, `xf_released`; one take each). Not checked: an arc copy's paste offset (live: centre at pointer + (−2.3, −4.2) mm, unresolved) | – | – |

## Dialogs (matrix)

live2's dialogs.json (11-13 05:40–07:25; rules read from the client bundle). part-studio-ui
`dialogs::the_dialogs_matrix_matches_live` (T1, no GPU) on the block start, and `manipulators::snap_tests`.

**Matched: 33 / 33** (11-18 21:28 UTC: live dialogs-1118 Tab orders =, Esc after Tab-out fixed; 31 / 31 at 11-18 02:17 UTC: live2 parts-list-1118 and dialog-keys-1118 rows, 2 fixed; 23 / 23 at 11-17 22:59 UTC).

| Row | Live | Ours | Status |
|---|---|---|---|
| Which features have a manipulator | Extrude one arrow; Chamfer, Shell none | the same | = |
| Handles beyond Extrude (live manip-1118b, 11-18 17:33) | Fillet: a small white-outlined triangle (~16 px) on the edge's middle; Move face, Offset surface: an arrow up from the face centre; Chamfer, Shell: none; Draft: none with only the neutral plane picked | Fillet a 16 px hollow triangle at the handle's point (where the round meets the faces' bisector, std addFilletControlManipulator; was an arrow); Move face / Offset surface arrows; Chamfer, Shell none | = (fixed) |
| Fillet's handle drag | unstepped (R5 → 23.41787, 28.97056, 41.52447 mm 5 px apart), a ~5 px dead zone, no value box; toward the edge no less than 0.02414 mm (std BLEND_BOUNDS 0.01 mm of offset); Ctrl+Z in the dialog doesn't undo a drag | unstepped to 5 decimals, the same floor (`fillet` in the arrow drag), the 5 px dead zone (dialogs matrix "manipulators / Fillet's handle drags unstepped") | = |
| Offset surface's arrow | the length step rule (4 → 18.2 mm over 40 px, 0.1 mm steps), no value box | the length step rule | = |
| A feature dialog closing (live section-1118, rAF-polled) | 62–102 ms after Esc or Enter with the keyboard outside a field, 71–82 ms after Esc following an edit | the next frame | = (faster) |
| The Section view dialog's place (live 10-27 and section-1118) | (396, 179), 220 wide: 150 px right of and 100 px below the feature dialogs' (246, 79) | was at the feature dialogs' spot; now there (measure_feel_tests `the_section_dialog_opens_right_of_and_below_the_feature_dialogs_spot`) | = (fixed) |
| Dragging the section arrow's head | nothing for 5 px; from 10 px a 75 × 26 value box 22–30 px right of and 32–39 px below the pointer, focused, its text selected, 0.1 mm steps; the arrow blue; after release the box stays focused and selected | the box was there from the press, 40 px right of and 25 above the rotate glyph; now 5 px dead zone, the box from the first move past it at the head + (26, 35) (a ring drag's by the glyph as live asm/2370) | ≈ (the arrow's blue: unchecked) |
| An arrow's grab reach | within 14 px across (16 no), from 4 px behind its base to its tip, nothing past the tip | 8 px of the shaft or 14 px round the tip (past it too); now live's (`Viewport::manipulator_index_at`) | = (fixed) |
| The arrow | shaft 30 + head 30 px, head radius 10 | shaft 29; now 30 | = (fixed) |
| A dragged length's step | 10^(round(log10(100 px in the unit)) − 2) of the unit at the arrow (0.1 mm at 0.25 mm/px, 1 mm from 0.316); Shift: free | always 0.1 mm / 0.01 in, Shift ignored; now live's (`manipulators::linear_step`, our own and std arrows) | = (fixed) |
| A dragged angle's step | 5° within 2° up to a 573 px radius, 1° up to 2865 px, then 0.1°; free between; Shift: free | always 5° within 2°; now by radius (the finer windows divided by 1 + level / 3: REQUESTS 12:48 answered) | = (fixed) |
| Properties on opening | no field has the keyboard (live's recordings: a div / ul); typing does nothing | Name focused; now the modal itself (`MetaForm::focus`), document keys still off behind it | = (fixed) |
| A selected entity clicked again | leaves the field (in, out, in) | the same; a consumed edge keeps a pick-only line under the preview (`builder::add_consumed_edge_picks`): it had none once the preview was built | = (fixed) |
| Enter on a retyped, unchanged value | an undo step all the same ("5" over "5 mm": Undo turns on; live external-thread-new-edit-reedit, move-face-tool) | the same (`DraftHistory::step_unchanged`); was no step | = (fixed; ui-parity) |
| A picked array's entries (Fill's Edges, Fillet's variable Vertices) | a row each: chevron, "[Position] Edge of Extrude 1", ×, its fields inside, open at first; a title or chevron click folds it; with COLLAPSE_ARRAY_ITEMS (Constrained surface, Loft) folded at first (live2 11-14, feel/fill-rows.png, array-rows-var-fillet-constrained.png) | the same (was a chip row in the field and the fields under it); the recorder reads each open entry's fields | = (fixed; ui-parity fill-new-edit-reedit) |
| Esc in a value field | the first discards the typed value and leaves the field, the dialog stays; a second cancels | the same | = |
| Enter with the keyboard outside a field | commits the dialog | the same | = |
| Typing in a value field | no preview while typing (none 3 s on); Enter, Tab or leaving the field sends it | the same | = |
| A click on Extrude's arrow | flips the direction (the field keeps 27 mm) and regenerates | only a direction-only arrow flipped; a length arrow's click focused its field; now it flips (`manipulators`, a release with no move) | = (fixed) |
| A field's row hovered | its entity lit yellow-green (#f2ff80) while hovered, cleared on leaving (a consumed edge too) | the same | = |
| A field's row clicked (not its ×) | the field keeps its entities | the same | = |
| Tab from Fillet's Radius | Radius's ± button, then Asymmetric, Partial, Variable, Allow edge overflow, Smooth fillet corners; selection fields and dropdowns skipped | the same | = |
| Tab with the unit list open ("6 mm" typed), then ✓ | Tab takes the entry and applies the field, the keyboard stays there; ✓ then commits (behaviours "Tab, then ✓ commits") | Tab took the entry only: ✓ then applied the value and left the dialog open (qa's hat15 fillet edit); now it applies too (`completion_keys`) | = (fixed) |
| Properties' Enter and Esc | do nothing (no apply, no close) | the same | = |
| Properties' Tab order | Name, Description, Part number, Revision, Vendor, Project, Product line, Title 1–3, Not revision managed, Exclude from all BOMs | Tab stayed in Description (indented it) and the checkboxes took no Tab; now live's order (Description passes Tab on, the two checkboxes are Tab stops) | = (fixed) |
| Appearance on opening | 220 × 400 at the graphics area's bottom left; hex focused and selected, uppercase without '#'; A "1.00" | the same | = |
| Appearance colour picking (live2 mass-appear-1118) | Palette: 18 px swatches, a click recolours the part at once (hex "FF0000"), ✓ keeps it; Custom: a 152 px saturation / value square and a 50 px hue strip: a press alone changes nothing, a drag updates the hex and recolours the part live, a hue click keeps saturation / value; Esc reverts the part exactly | the same (a press in the square set the colour: now only a drag does; sketch_ui_tests::the_appearance_mixer_changes_on_a_drag_not_a_press, the_appearance_editor_commits_as_you_type) | = (fixed 11-18) |
| Material on opening | 280 × 368 at the graphics area's bottom; nothing picked ('- None -') | the same | = |
| Material: a picked row | the list closes, the search keeps the keyboard with its text cleared; typing opens the list again; "No material assigned" stays under the open list (material-search-acetal-ok, material-library-pick-abs-cancel) | the keyboard went nowhere and the line hid while the list was open; now as live (the search stays in the tree, unseen) | = (fixed) |
| Material's search and Esc | filters as you type ("steel": 38 rows; live's read counted its highlighted first row twice), the first highlighted; Esc closes the list only, a second Esc the dialog | the second Esc did nothing (the keyboard was left on the closed list's search); now it cancels | = (fixed) |
| Material keys (live2 parts-list-1118 f2-f4) | "alum": Down from Alumina Oxide to Aluminum, Enter picks it, the part's look unchanged; Esc then closes the dialog unapplied | Esc after a pick did nothing (the keyboard on the closed list's hidden search); now it cancels | = (fixed) |
| Add appearance to face… (live2 parts-list-1118 e1-e4) | "Face appearance 1", 220 × 401 in the feature dialogs' place; "Faces or features" with the chip "Face of Extrude 2" and its ×; a swatch recolours only that face at once; ✓ keeps it, no feature row | 431 px at the graphics' bottom left, "1 face" in the field; now as live | = (fixed) |
| Keyboard on opening (live2 dialog-keys-1118) | a new Extrude / Fillet and an edited Extrude focus their first selection field | the same | = (new) |
| Tab from Depth (live2 dialog-keys-1118 b1) | ± → Direction → Starting offset → Symmetric → Draft → Second end position → help → out of the dialog; End type, the selection field, the tabs and ✓ / ✗ skipped; Shift+Tab back | the same | = (new) |
| Tab orders of the other dialogs (live dialogs-1118 batches 1–3) | from the first number field: Chamfer Distance → ± → Tangent propagation → help; Shell thickness → its flip arrow → help; Hole's size → ± → Start plane → Termination (dropdowns are stops) → flip → depth → ± → tip angle → help; Revolve (Full revolve off) angle → ± → Symmetric → Second end position → help; Linear Distance → Instance count → flip → Centered → Second direction → Skip instances → help; Circular Angle → Instance count → flip → Equal spacing → Centered → Skip instances → help; then out | the same | = (new) |
| Esc after Tab has left the dialog (live dialogs-1118 batch 3) | does nothing (twice); a click on empty graphics, then Esc, cancels | Esc cancelled it; now the dialog remembers Tab took the keyboard out (`tabbed_out`) and Esc leaves it open until a press in the graphics or the keyboard is back in it | = (fixed; qa 23:2x on green df6be9b62, vibex8: Extrude, Tab out past help, Esc ×2 leaves it open, a click on empty graphics + Esc cancels) |
| Space, dropdown keys, Ctrl+Enter (live2 dialog-keys-1118 c1-c3) | Space toggles a focused checkbox; End type clicked: a focused search box, Down moves the highlight, Esc closes it unchanged; Ctrl+Enter in a field is Enter | the same | = (new) |
| An empty click with a box active | the box's entries and the dialog stay as they are (no deselect, no close; live feature-dialogs-1117) | the same | = |
| Radius focused, an edge clicked | the Entities box stays active: the edge is added and the keyboard leaves Radius | the same | = |
| Boolean Subtract | picks stay in Tools (no auto-advance to Targets); a click on Targets makes it active | the same | = |
| What lights while a box is active (GPU, real pointer: `feel::a_dialogs_active_box_filters_what_lights`) | Extrude: an edge lights the face beside it; Fillet / Chamfer: a vertex an edge at it; Boolean: a face the whole part; a plane's empty spot nothing | the same (the field's pick filter, `highlighted_hover`'s part for a parts box) | = |
| Not yet in the matrix | the drag's 2–3 px dead zone, no snap to geometry, no value box while dragging, preview timing while dragging (every ~0.4 s);  the Appearance palette's swatch grid and the Material picker's rows (look); Appearance's keys are in `sketch_ui_tests` (escape / Tab / Enter); live dialogs-gaps2 (11-16): the Depth arrow moves from 4 px with the full displacement (its readout updates every ~3 px, so the first tick may be the throttle; ours from 3 px), sideways drags nothing, no value box, no snap to geometry, preview cadence host-bound (450–850 ms there) | – | – |

## Fields (matrix)

live2's fields.json "numericFields" (11-13 10:07, Extrude 1's Depth and Draft angle). part-studio-ui
`fields::the_fields_matrix_matches_live` (T1, no GPU) on the block start.

**Matched: 17 / 17** (11-18 02:17 UTC: live2 dialog-keys-1118 rows, all as ours; 14 / 14 at 11-16 00:47 UTC: live's fields-gaps rows added (wheel, Tab after a bare "#", Create variable's place), 2 fixed; 11 / 11 at 11-15 20:35 UTC with the open list row; 10 / 10 at 20:06 UTC with the right press row; 9 / 9 at 11-14 00:33 UTC; 6 / 9 at the first run).

| Row | Live | Ours | Status |
|---|---|---|---|
| Enter | the expression, a bare one wrapped with the field's unit: "10+5" → "(10+5) mm", "10.123456" → "10.123456 mm"; "1 in" stays | the same | = |
| Blurred | the value in the document unit, 3 decimals: "15 mm", "25.4 mm", "10.123 mm" | the same | = |
| Refocused | the typed expression again | the same | = |
| Invalid ("abc" + Enter) | pink, the text kept, the title red, ✓ disabled | the same | = |
| Negative length | "-5" → "-5 mm", accepted | the same | = |
| Angle | "5" → "5 deg"; "-5" → "-5 deg", pink | "-5" stayed bare (an out-of-range value was left as typed whole); now the unit too | = (fixed) |
| Arrow keys | Up +1 unit, Down −1, Shift+Up +10, Alt+Up as Up | Alt+Up did nothing (the text field's own Alt move); now a step | = (fixed) |
| Autocomplete | "1 m": m, max( x , y ), min( x , y ), mm; "1 i": in, isInteger( x ) | the same (its rows as drawn) | = |
| Autocomplete's Esc | closes the list only | the same | = |
| Right press (live2 11-15, dialog-number-rclick-menu) | no menu, Chrome's or Onshape's; the field takes the keyboard, all selected; one Esc cancels the dialog | the input's Cut / Copy / Paste opened and took that Esc; now no menu | = (fixed) |
| Autocomplete open (lead P1 11-15; live2 ac-menu) | the "#" and "asin" lists overlay the rows below, which keep their rects; the list's top is the input's bottom, its right edge the input's; 5.25 px corners, no shadow, the rows fill the 1 px border | the rows below moved 2 px down (the list hung from a box in the field's column, which took its gap); the list was left-aligned, inset 1 px with square highlighted corners and a shadow; now as live | = (fixed) |
| Wheel over the focused field (live fields-gaps 11-16) | +1 mm per notch, −1 down, Shift ×10, Ctrl ×0.1 | the same | = |
| Tab after a bare "#" (live fields-gaps) | only New variable…, nothing highlighted (with a name typed, "#nope", it is highlighted: live extrude-depth-new-variable); Tab closes the list, the field keeps "#", the keyboard moves on | New variable… highlighted, Tab opened Create variable; now as live (`VarCompletion::highlighted`) | = (fixed) |
| Create variable's place (live fields-gaps) | top-left ~4 px right of the field's right end, top level with the field's | 107 px right of the field's left, 12 px above its top; now as live | = (fixed) |
| Arrow keys, more (live2 dialog-keys-1118) | Ctrl+Up +0.1; a committed "1 in" Up "26.4 mm" (a workspace unit, rewritten in it); typed text not yet Entered: arrows do nothing | the same | = (new) |
| "5+" (live2 dialog-keys-1118 a1-a2) | no error while typing; Enter: the box pink, the title red, so after blur too | the same | = (new) |
| An unknown variable (live2 dialog-keys-1118 a3) | "#zz" + Enter opens Create variable | the same | = (new) |
| Not yet in the matrix | Ctrl+Up ×0.1 (an existing test), the wheel over a focused field, the "#" list's New variable… and its popover, Tab taking the highlighted entry (live2 field-keys-1118 / chips-expr-1118: all measured, rows below) | – | – |

## Feedback (matrix, Part Studio rows)

live2's feedback.json (11-13 09:39, for psui and studios). part-studio-ui `feedback::the_feedback_matrix_matches_live`
(T1, no GPU; the row's tooltip read 500 ms after the pointer comes onto it).

**Matched: 9 / 9** (11-13 19:05 UTC, 8 / 8 at 17:09 UTC; 1 / 2 at the first run, then 6 rows added: 4 / 6 matched).

| Row | Live | Ours | Status |
|---|---|---|---|
| A failing feature's tooltip | "<Feature> did not regenerate properly: <message>", then "1 missing selection" | the same | = |
| A warning feature's tooltip | "Fillet 1", then "1 missing selection" | "Fillet 1 did not regenerate properly: 1 missing selection"; now live's | = (fixed) |
| A failing feature's row | an error row (rgb(170,37,45) name, red icon, warning overlay; the header's failures icon) | the same (state "error") | = |
| A warning's row | only lost selections: the name stays rgb(51,51,51), the icon's overlay only; no failures icon (one with a message of its own reads red, live L27) | red like an error; now black (`only_missing_selections`) | = (fixed) |
| A warning's dialog | the title black with no tooltip, no bubble or toast; the lost entry a grey italic "Edge of Chamfer 1" | the title red with "… did not regenerate properly: 1 missing selection" and a yellow bubble; now live's (`status_reads_red`) | = (fixed) |
| The red title's tooltip | "<Feature> did not regenerate properly: <message>" | the same | = |
| Fillet R100 on the block's vertical edge | builds (5 faces), no error (behaviours "blend (2986)": every radius up to R100) | the same | = |
| A failed preview | Fillet 100 mm on the block's top edge: the title red, the entry red (the field not marked), "… Failed to fillet selections." | regenerated "Ok" into a body with no faces: blend fixed it 11-13; the entry wasn't red, now it is for an error naming no field (`FeatureDialog::entry_failing`) | = (fixed) |
| Its dialog | Sketch 1 deleted under Extrude 1: the title red, the lost region a red "Missing Face" entry, ✓ not disabled (an edge whose feature is only below reads "Edge of Extrude 1": live fillet-chip-edit-healthy-moment) | "Face of deleted feature"; now "Missing Face" | = (fixed) |
| Not yet in the matrix | a failed preview (live feedback-gaps 11-16, Shell 20 mm on the 10 mm block: every edge of the part red rgb(189, 48, 57) ~5 px, faces tinted (rgb(125, 106, 119) on the dark sides), the title red, the face chip and the feature row red, ✓ still enabled; ours the same since 11-16 (core's `FeatureState::error_parts` 601df0ac, `scene_builder::error_geometry(…, ghost: false)`: the part's shading under rgb(189,48,57) at 25 %, edges 5 px): top face (162,156,176) as live, front (124,107,120) vs (125,107,120); a Feature pattern's ghosts keep the dark 60 % style); the commit's working overlay (live first seen 1119 ms after ✓, polled every 40–150 ms; ours 1200 ms: within a frame or two, not changed without a finer measure); toasts (studios: success rgb(221,246,204), top centre, 5.1 s, no stacking, hover doesn't pause); empty states | – | – |

## Captures 11-17 / 11-18 (audit: one row per feel finding)

Every feel finding recorded in onshape-research/live/behaviours.md on 11-17 and 11-18 for the Part Studio, with the
test that checks it (`file::fn`, or a matrix and its row) or **missing**. Owner other than psui in brackets. "=" ours as
live (by its test or the code), "≠" a known difference, "?" not checked yet. P0 marks the lead's P0 captures.

### hover-p0 (USER P0, sketch hover vs planes in iso, live2 11-17 12:35)

| Finding | Ours | Test |
|---|---|---|
| A sketch line wins over any plane, in front of it or behind it on screen | = | sketch_hover_feel_tests::sketch_hover_reach_and_priority_as_live, a_sketch_line_lights_over_the_planes_behind_it_and_lets_go |
| A sketch point beats its lines within its reach | = | sketch_hover_feel_tests::sketch_hover_reach_and_priority_as_live |
| The region beats the planes behind it | = | sketch_hover_feel_tests::sketch_hover_reach_and_priority_as_live |
| Over a constraint glyph nothing else hovers (its entities in the secondary pre-selection) [skasm] | = (fixed: a row's span, gaps included, takes the hover from the geometry under it) | sketch_box_feel_tests::midpoint_glyph_and_plane_clicks_in_a_sketch_as_live |
| The sketch's own plane never hovers in sketch mode; other planes do | = | sketch_hover_feel_tests::sketch_hover_reach_and_priority_as_live |
| Reach in sketch mode: line 7 px, point 10 px | = | sketch_hover_feel_tests::sketch_hover_reach_and_priority_as_live, a_sketch_circle_and_its_centre_light_over_the_planes |
| A plane hovers over its whole interior plus 6 px outside its edge (8 px: nothing) | ≠ (6 px and the inside as live; ours still lit at 8 px across iso's slanted edges: the 5 px pick square's corner + the 5 px border band; live's 3d.json level border lights at 7–8) | feel_tests::a_plane_lights_6_px_past_its_edge (prints the reach) Live recheck 11-19 (iso, the Top plane's lower-left slanted edge, the pointer stepped out along its normal, 2 rounds, live/feel/plane-reach-1119): live lit at 5.8 and 7.2 px, not at 8.05 / 9.4 / 10.3; ours (green 88545e5d7) lit to 9.1, not at 9.9 ≠ (≈2 px more) |
| Reach outside sketch mode: line lights at 7 px and holds to 9–11 (live "feel (3D) retaken 11-18"), point 9 px drawn alone as a ~10 px light orange disc, with no tool a line's middle lights the line (live2 11-18 ln / pt); Top hovers like any plane; the nearer plane wins where planes overlap | = for the reach and the point, ≠ at a line's middle until psui's midpoint change lands (fixed 11-18; on green 548d795f live2 / qa: the midpoint ring took a line's middle [psui's midpoint rule, narrowed by psui], a corner showed a 1–2 px speck): a shown sketch's point within 8.5 px wins (lights at 9: qa 11-18 measured 10 at 9.5), drawn as a 10 px #f8c488 disc; a curve lights within 7.5 px and holds to 10.5, what lies under it otherwise; a click takes the same (`shown_sketch_hover_id`, `midpoint_overlay`) [skasm]. Top / overlapping planes: psui | sketch_hover_feel_tests::a_shown_sketchs_line_and_point_reach_outside_sketch_mode_as_live |
| Hover timing: under 0.15 px/ms picks at once; faster clears and picks min(5 × average pick, 250 ms) after the last move | = | app-core viewport::prehighlight_tests::prehighlight_waits_for_a_pause_as_live |
| No stale hover: only what is under the pointer's last spot | = | sketch_hover_feel_tests::a_quick_sweep_off_a_line_leaves_nothing_lit, the_first_pick_at_rest_lights_what_is_under_the_pointer |
| With no tool a line's midpoint hovers the line; a point hovers as a light-orange disc | = | sketch_box_feel_tests::midpoint_glyph_and_plane_clicks_in_a_sketch_as_live, sketch_hover_feel_tests::sketch_lines_and_points_light_as_live |
| A glyph click selects the constraint, a second glyph click replaces it; a plane click in sketch mode selects the plane | = | sketch_box_feel_tests::midpoint_glyph_and_plane_clicks_in_a_sketch_as_live |
| A dimension label hovers tan, a click fills it orange, a double-click opens its value box [skasm] | = | sketch_label_feel_tests::a_dimension_label_hovers_tan_selects_on_a_click_and_opens_on_a_double_click |
| Corner and line drags with no tool: the rectangle stays one, a line moves along its normal [skasm] | = | sketch_mode `a_rectangles_corner_and_side_drag_as_lives` (live2's (30, -20) -> (39.19, -14.22) and the right line to x 39.20) |
| A press on a plane and a drag draws a box; window over the rectangle takes its 4 lines, crossing over the right half 3; press-release on a plane selects it | = | sketch_box_feel_tests::a_box_in_iso_over_the_planes_takes_lines_as_live |
| With a drawing tool active no plane or region hovers, only snap targets [skasm] | = | sketch_ui_tests::no_plane_lights_while_a_line_is_drawn |
| Constraint buttons on a two-line selection: applied at the click, selection cleared, the second-picked line moves; refusals in a bubble; Symmetric arms [skasm] | = | scene the_second_picked_line_moves_as_lives, horizontal_and_vertical_turn_both_lines_about_their_midpoints (refusal texts: missing) |
| A dimension label dragged follows the pointer, no snap, no selection needed [skasm] | = | sketch_label_feel_tests::a_dimension_label_follows_the_pointer_without_a_selection |

### feel-1417 (P0, glyph linger and label drag threshold, live2 11-17 14:16)

| Finding | Ours | Test |
|---|---|---|
| Show constraints off: a hovered line shows its glyph row; the line turns orange [skasm] | = | sketch_glyph_feel_tests::a_hovered_lines_glyphs_wait_by_the_pointer |
| Leaving into empty canvas: the line's orange goes first, the glyphs ~0.4–0.7 s later | = | sketch_glyph_feel_tests::a_left_lines_glyphs_go_when_the_linger_ends |
| Moving onto a glyph within the linger keeps the row; the glyph hovers yellow, the line stays orange | = | sketch_glyph_feel_tests::a_hovered_lines_glyphs_wait_by_the_pointer |
| The row is placed once where the hover starts (~40 px off) and doesn't follow the pointer | = | sketch_glyph_feel_tests::a_hovered_lines_glyphs_wait_by_the_pointer |
| A dimension label starts moving 5 px from the press [skasm] | = | sketch_label_feel_tests::a_dimension_label_follows_the_pointer_without_a_selection |

### wake-parallel (woken model edge, live2 11-17 14:30) [skasm]

| Finding | Ours | Test |
|---|---|---|
| A woken straight model edge gives no PARALLEL (0.2–3° off, woken or not) | = | snap_matrix_tests::the_sketch_snaps_matrix_matches_live / "wake/model-edge/no-parallel" |
| It gives its end vertices' alignments (VERTICAL with its end) | = | the_sketch_snaps_matrix_matches_live / "wake/model-edge/end-alignment" |

### show-constraints (Show constraints remembered, live2 11-17 14:25)

| Finding | Ours | Test |
|---|---|---|
| The sketch dialog's Show constraints is per user, written when a sketch is committed (✓), not on ✗; off by default | = (`save_options_if_changed` on ✓ only, to the user's config file; `SketchOptions::default` off) | sketch_mode `sketch_options_default_as_live` (the save on ✓: missing, the options are process-wide in tests) |

### hover-pick-1120 (P0, hover reach and priority in iso with the planes shown, live 11-17 20:41)

| Finding | Ours | Test |
|---|---|---|
| A part edge vs the face behind / below it: lights at 5–6 px, stays to 8–10; the face never beats the edge inside reach | ≠ (green 4a69d55b, live2 ours-1118): the edge lights at 6 px but gives way to the face at 8 (held only to 7) | feel_tests::a_click_beside_an_edge_selects_what_is_lit (reach in iso: missing) |
| A sketch point takes over from its line 8 px before the end; beyond the end 6 px; a line 5–7 px, the sketch's plane beyond | = | sketch_hover_feel_tests::sketch_hover_reach_and_priority_as_live |
| A mate connector wins across its drawn triad: 22 px horizontally, 13–14 px vertically, over the face | = (fixed: the Part Studio picked fixed 4 mm axis stubs; now the drawn disc, as the assembly) | feel_tests::a_mate_connector_wins_over_its_face_across_its_disc |
| Through a transparent default plane geometry behind it always wins; the plane lights only where nothing is behind | = | feel_tests::a_block_behind_the_front_plane_lights_through_it, part_faces_and_vertices_light_over_the_planes_behind |
| A block edge behind the Front plane: 5–6 px from the plane side, 4 px from the face side | = | feel_tests::a_block_behind_the_front_plane_lights_through_it |
| Timing: a fast jump lights after the pause (min(5 × average pick, 250 ms)) then one pick | = | app-core viewport::prehighlight_tests::prehighlight_waits_for_a_pause_as_live |

### sel-feel (P0, Part Studio selection, live2 11-17 22:12)

| Finding | Ours | Test |
|---|---|---|
| During a box drag nothing lights; on release the result is selected at once | = | ≠ (live retake 11-18 23:17 UTC on green 5c9229069, 60 fps x11grab, live/feel/recheck-5c9229069, 3 drags: each has frames with the box gone and the part not yet selected: 83, 150, 317 ms, then the selection; at load 28, so the gap is longer than unloaded, but live never shows that frame). Fixed psui 11-18 23:23: the 3D frame renders off the UI thread, so the box overlay went a frame or more before the frame with the selection; the released box now stays until a frame submitted after the release is presented (`Viewport::box_held`; `a_box_drag_lights_nothing_and_selects_at_release` checks it stays, then goes) |
| Window box counts on two blocks (28 / 21); a block's 18 / 5 / crossing 13 | = | feel_tests::box_select_takes_what_live_takes (one block; the two-block counts missing) |
| A press whose first move ends over a default plane lights the plane and draws no box until the next move | = (fixed 11-18: was the box on the first move, live2 ours-1118) | feel tests `a_box_whose_first_move_ends_on_a_plane_waits_a_move` |
| A click on a default plane selects it, a second adds; a face then a plane gives both; an empty click clears all | = | sketch_box_feel_tests::midpoint_glyph_and_plane_clicks_in_a_sketch_as_live (sketch mode); list matrix "click / a default plane: Default geometry related" |
| A double-click on an edge selects then deselects, no chain | = | list matrix "double-click / a part: two toggles" (edge in the viewport: missing) |
| A double-click on a face shows its feature's sketch, rows related, nothing kept selected | = | sketch_ui_tests::a_face_double_clicked_shows_its_features_sketch, sketch_hover_feel_tests::a_face_double_clicked_draws_its_sketch_through_the_pointer |
| A feature row hovered: rgb(222,241,251), tooltip "Extrude 1 <- Sketch 1", its part outlined rgb(255,198,133) | = | list matrix "hover / the row's tooltip", "hover / the geometry lit 100 ms after, cleared 100 ms after leaving" |
| A part row hovered: the same plus its eye icon and a "Part 2" tooltip | = | list matrix "hover / the row's tooltip" |
| A hidden sketch row hovered: drawn in place, lit; "Sketch 1 (Hidden) is not fully defined" | = | list matrix "hover / a hidden sketch: drawn in place, lit" |
| A feature row clicked: selected rgb(178,221,246), its part related rgb(236,240,244), the part drawn selected | = | list matrix "click / toggles, others kept" (the colours: Look rows) |
| A viewport face / edge clicked relates its feature and part rows, never selects them | = | list matrix "click / a face or edge in the graphics relates its feature and part rows" |
| A sketch region on a face relates only its sketch; its look a pale fill with an orange outline | = | list matrix "click / a sketch region relates only its sketch" (the look: missing) |
| A default plane clicked: its row selected, Default geometry related | = | list matrix "click / a default plane: Default geometry related" |
| Space or an empty click clears both sides | = | list matrix "clearing / a click on empty canvas" |

### dialog-memory (USER P0, feature dialogs remembering the previous value, live 11-17 12:53)

| Finding | Ours | Test |
|---|---|---|
| Fillet Radius remembered across documents as typed (plain or a variable-free expression), metric and inch kept apart | = | sketch_ui_tests::a_remembered_value_never_carries_a_variable |
| A value with a #variable is never remembered: the earlier memory stays | = | sketch_ui_tests::a_remembered_value_never_carries_a_variable |
| A cancelled dialog isn't remembered; a committed one that fails is | = | sketch_ui_tests::only_radius_and_measurement_carry_and_a_cancel_remembers_nothing (the failed one: missing) |
| Fillet carries Radius and Measurement; not Tangent propagation, overflow, smooth corners, Asymmetric / Partial / Variable | = | sketch_ui_tests::only_radius_and_measurement_carry_and_a_cancel_remembers_nothing |
| Extrude Depth is never remembered (25 mm / 1 in default) | = | sketch_ui_tests::only_radius_and_measurement_carry_and_a_cancel_remembers_nothing |
| A Hole's remembered values: Units / Style everywhere, the diameter only in documents of the same length unit | = | sketch_ui_tests::remembered_hole_values_follow_the_document_units |

### feature-dialogs-1117 (P0, selection boxes, live 11-17 22:52)

| Finding | Ours | Test |
|---|---|---|
| Every dialog opens with its first box active (border rgb(22,81,176), fill rgb(222,241,251)); empty and required: red title | = | dialogs matrix "keys / on opening the first selection field has the keyboard" (the look: missing) |
| Hover filtered by the active box: Extrude an edge lights its face; Fillet / Chamfer a vertex lights an edge; Boolean a face lights the part; planes nothing | = | feel_tests::a_dialogs_active_box_filters_what_lights |
| Click adds, click again removes, a third adds back; an empty click changes nothing, the dialog stays | = | dialogs matrix "fields / a selected entity clicked again leaves the field", "fields / an empty click changes nothing" |
| An entry hovered lights its geometry #f2ff80 121–181 ms after, with its name in a tooltip | = | dialogs matrix "fields / a row hovered lights its entity" (the delay: missing) |
| Preview: asked 113–330 ms after the pick | = | dialogs matrix "preview / not while typing; Tab sends it" (the delay: missing) |
| Enter with the keyboard outside a field commits; Esc with a pick cancels and removes the feature | = | dialogs matrix "fields / Enter with the keyboard outside a field commits", "manipulators / Esc with the button held cancels the dialog" |
| Radius focused: the box keeps its active look, an edge click goes into it and takes the keyboard out of Radius | = | dialogs matrix "fields / Radius focused, an edge click goes into the box" |
| Boolean Subtract: picks stay in Tools (no auto-advance); a click on Targets makes it active | = | dialogs matrix "Boolean / Subtract: picks stay in Tools" |

### featurelist-feel (feature list, live2 11-17 23:25)

| Finding | Ours | Test |
|---|---|---|
| Rollback bar held: a chip follows, the insertion line shows, the bar stays, nothing greys | = | list matrix "rollback bar / while held nothing moves or greys" |
| On release the bar moves, the rows below grey (rgb(154,154,154) italic), the Parts list follows | = | list matrix "rollback bar / on release the rows below grey and the Parts list follows" |
| The bar is #b8b8b8 away from the pointer (rolled back or at the end), #c8d9e2 hovered | = | missing |
| It drops between any features or above the first, never into Default geometry | = | list matrix "rollback bar / above the first feature", "rollback bar / onto default geometry: the top" |
| A reorder the history can't take is accepted, then fails red rgb(170,37,45) with a warning triangle, no message | = | list matrix "drag / a reorder the history can't take: red, no message" |
| A sketch dragged onto Default geometry: nothing | = | list matrix "drag / onto default geometry: nothing" |
| Double-click opens the dialog at (246, 79), 220 wide, right of the list; its rows edited / related | = | list matrix "double-click / the dialog right of the list, 220 px wide" |
| Rename: 178 × 24 inline, all selected; Enter and a click elsewhere commit; F2 nothing; the tooltip gains "[Extrude]" | = | list matrix "rename / Enter commits", "rename / a click elsewhere commits", "keys / F2 on a selected row: nothing", "hover / a renamed feature's tooltip has its type" |
| Suppress: grey italic struck through; the menu then offers Unsuppress | = | list matrix "suppress / from the menu, then Unsuppress" |
| Autoscroll (fl-autoscroll-1118): a 60 px zone, ~200 px/s, ~390 past the edge, running with the pointer still | = | behaviors::tests::autoscroll_speeds_as_live |
| The drop slot is picked only by a move: a list autoscrolled under a still pointer carries the line away and a release drops there | = (fixed 11-18: ours dropped at the row under the pointer) | list_feel_tests::a_list_scrolled_under_a_still_drag_drops_at_its_line |

### doc-level-1117 (tab switch, undo / redo, regeneration feedback, live2 11-17 23:36)

| Finding | Ours | Test |
|---|---|---|
| A tab switch: the old element goes at once, the list fills, the canvas last, no spinner [studios] | = (green c7d388e8, live2 ours-1118: block-drawing, drawing → Part Studio 1: list, canvas and toolbar change together in one frame at 0.17 s, no spinner; live's list-then-canvas order is its loading, not a target) | app workspace_tests `a_tab_switch_shows_the_new_tab_in_its_first_frame` |
| Ctrl+Y redoes, Ctrl+Shift+Z doesn't (Shift+⌘Z on a Mac); no toast on undo or redo | = | app-core actions::tests::shared_keys_follow_lives_list_per_os; list matrix "undo / a rename is one step, no toast" |
| A rename is one step; hiding a part is one step; a view change or a selection is none | = | list matrix "undo / a rename is one step, no toast", "undo / hiding a part is one step", "undo / a view change or a selection is no step" |
| In a dialog with the keyboard in a value field Ctrl+Z does nothing; Esc then cancels | = | sketch_ui_tests::undo_does_nothing_while_a_dialog_control_has_the_keyboard |
| Regeneration: the old model stays drawn; the working spinner from ~0.75 s (900 ms in live's code), centred low; rows ungrey when done | = | sketch_ui_tests (the spinner's state around a cancelled run, ~2563); its delay and place: missing |

### rb-followup (rollback bar follow-ups, live 11-18 00:27)

| Finding | Ours | Test |
|---|---|---|
| Released over Default geometry (expanded or collapsed) it lands at the last legal slot the line showed: the top for a normal drag | = | list matrix "rollback bar / onto default geometry: the top" |
| Its colour: #b8b8b8 away (rolled back or at the end), rgb(200,217,226) hovered | = | missing |

### spinner (the working spinner, live's client code, 11-18 00:20)

| Finding | Ours | Test |
|---|---|---|
| One spinner, shown 900 ms after a call starts (none if it finishes sooner); body loading and box select at once | = | missing (`pending::OVERLAY_DELAY`) |
| Its look: 50 × 50 rgba(255,255,255,0.75), radius 10, 40 px above the bottom; a 30 px ring rgb(215,215,215) with a blue quarter, 1.1 s a turn, no pulse | = | missing |

### parts-list-1118 (Parts list and appearance, live 11-18 01:47)

| Finding | Ours | Test |
|---|---|---|
| Part rename: whole name selected; Esc cancels, Enter commits, a click away commits | = | list matrix "Parts list / Rename: whole name selected, Esc cancels, Enter commits", "Parts list / a click away commits a rename" |
| Shift+N on a selected part renames; F2 nothing | = | list matrix "Parts list / Shift+N renames a selected part, F2 nothing" |
| A part hovered in the graphics leaves its row untinted | = | list matrix "Parts list / a part hovered in the graphics leaves its row as is" |
| Dragging a part row does nothing | = | list matrix "Parts list / parts can't be dragged" |
| A face's menu ends Assign material / Edit appearance / Add appearance to face… | = | menus tests (ctx-menus-1117 rows) |
| Add appearance to face…: "Face appearance 1", 220 × 401 at (246, 79), chip "Face of Extrude 2"; a swatch recolours only that face; no feature row | = | dialogs matrix "Appearance / a face's: a feature-style dialog that recolours the face at once, no feature row" |
| Material search: Down moves, Enter picks; no appearance preview; Esc discards | = | dialogs matrix "Material / Down and Enter pick the next match, no preview, Esc discards" |

### dialog-keys-1118 (keyboard in feature dialogs, live 11-18 01:52)

| Finding | Ours | Test |
|---|---|---|
| On opening (new or edit) the first selection field has the keyboard | = | dialogs matrix "keys / on opening the first selection field has the keyboard" |
| "10 in" kept while focused, "254 mm" on blur; "2*#w" → "10 mm" on blur | = | fields matrix "Enter: the expression, its unit added", "blurred: the value in the document unit, 3 decimals" |
| "5+": fine while typed; on Enter pink rgb(249,235,236), title red rgb(170,37,45), no tooltip | = | fields matrix "invalid: '5+' fine while typed, pink with a red title on Enter" |
| "#zz" + Enter opens Create variable; the field stays pink | = | fields matrix "unknown variable: '#zz' + Enter opens Create variable" |
| ↑ +1, ↓ −1, Shift +10, Ctrl +0.1, Alt +1; "1 in" ↑ gives "26.4 mm"; uncommitted text not stepped | = | fields matrix "arrow keys step the value", "arrow keys: Ctrl a tenth, another unit stepped in the workspace's, typed text not at all"; app-core quantity_field inch steps |
| Tab from Depth: ± → the five checkboxes → help → out to the header; End type, the field, tabs, ✓ / ✗ skipped; Shift+Tab back | = | dialogs matrix "keys / Tab from Depth, then out of the dialog; Shift+Tab back" |
| Space on a focused checkbox toggles it | = | dialogs matrix "keys / Space toggles a focused checkbox" |
| End type by keyboard: search box focused, ↓ moves, Esc no change | = | dialogs matrix "keys / a dropdown: its search box focused, Down moves, Esc changes nothing" |
| Ctrl+Enter in a value field is plain Enter (macOS too since 11-18 06:xx) | = | dialogs matrix "keys / Ctrl+Enter in a value field is Enter" |

### keys-1118 (keys in lists, Delete with dependents, Tab from the graphics, live 11-18 02:56)

| Finding | Ours | Test |
|---|---|---|
| Delete on a used sketch: no dialog, no toast; its users red, the header ⓘ; the tooltip "… did not regenerate properly: Select face or sketch region to extrude." / "1 missing selection" | = | list matrix "keys / Delete on a used sketch: no dialog, its user fails" |
| The sketch row's menu, in live's order | = | list matrix "menu / two sketches" (the one-sketch order: missing) |
| Sketch Ctrl+C / Ctrl+V: the copy centred on the pointer, selected, the Transform tool on it, constraints copied [skasm] | = | editing matrix "copy / sketch entities paste under the pointer" |
| Ctrl+A in a sketch selects nothing [skasm] | = (no graphics binding) | sketch tools matrix "keys / Ctrl+A selects nothing" |
| Tab from the graphics: Measure → Analysis tools → Mass → out; Shift+Tab back; no row or toolbar button a stop | = | list matrix "keys / Tab from the graphics: Measure, Analysis tools, Mass properties, then out; Shift+Tab back" |

### nav-1117 (P0, navigation gaps, live 11-17 23:13)

| Finding | Ours | Test |
|---|---|---|
| One wheel event is one step (2^(1/12)) whatever its delta, about the pointer | = | nav matrix "wheel / a notch in", "wheel / a notch out", "wheel / in and out toward the pointer" (delta independence: navigation::tests::wheel_notch_zoom_per_platform, a notch's 5.3 lines still one step) |
| Ctrl+wheel is the same zoom; a pinch zooms one step per event | = | app-core navigation::tests::wheel_notch_zoom_per_platform (Ctrl), pinch_updates_zoom_one_notch_each |
| Animated view changes: smootherstep, 500 ms for named / standard views, F and the cube; 250 ms for arrows, pan and zoom keys | = | nav matrix "animation / smootherstep", "keys / Shift+1, 5, 7 and F: 500 ms", "keys / arrows: 15°, Shift 90°, Ctrl 5°, in 250 ms", "keys / Ctrl+Shift+arrows pan 150 px in 250 ms" |
| A new animation while one runs is ignored | = | nav matrix "animation / a new one while one runs" |
| Below 20 fps (frames over 50 ms on average) a view change jumps | = | app-core navigation::tests::a_view_change_jumps_below_20_fps |

### ctx-menus-1117 (right-click menus and Shift+/, live 11-17 23:23)

| Finding | Ours | Test |
|---|---|---|
| "Where used for Part 1…" in its own group after Export… on every menu naming a part | = | sketch_ui_tests::where_used_for_the_part_has_its_own_group_after_export, graphics_menus_over_a_selection_as_live |
| The Parts list row: "Where used…" after Export… (Create task left out) | = | list matrix "menu / one part" |
| The Top plane row: "Section view…" in its own group | = | list matrix "menu / the Top plane row" |
| Two selected: "Clear selection" above Select ▸; "Add appearance to 2 faces…"; a face + an edge drops the part rows | = | sketch_ui_tests::graphics_menus_over_a_selection_as_live |
| Sketch mode: every menu starts "Confirm Sketch 2" ("Escape line" above it with a tool); a line's, a line + end's, a face's "Use" [psui + skasm] | = | sketch_ui_tests::sketch_entity_menus_as_lives, a_plane_behind_the_sketch_gets_its_menu |
| Open time: rows 66–95 ms, one entity 150–270 ms; place pointer + (0, 2) / rows + (10, 12), flipped at the bottom | = | chrome matrix "context menu / at the pointer + (0, 2)", "context menu / a feature-list row's at + (10, 12)", "context menu / flips at the window's right and bottom" (open time: missing) |
| Shift+/ sheet: 114 entries in 7 tabs, as ours | = | app-core shortcut_sheet::tests::shortcut_sheet_lists_every_part_studio_key |

### manip-feel-1118 (P0, manipulator drags, live 11-18 01:20)

| Finding | Ours | Test |
|---|---|---|
| A plain click on Extrude's arrow flips the direction; again flips back | = | dialogs matrix "manipulators / a click on Extrude's arrow flips it" |
| While a feature arrow is dragged nothing lights under the pointer; Depth is the pointer projected on the arrow's line; the arrow blue | = | dialogs matrix "manipulators / an arrow drag lights nothing; Depth has the keyboard, its text selected" (nothing lit; the projection and the blue: missing) |
| During the drag Depth has the keyboard with its text selected, rewritten each update; on release it stays focused and selected | = | dialogs matrix "manipulators / an arrow drag lights nothing; Depth has the keyboard, its text selected" |
| Ctrl or Alt held changes nothing; Shift unrounded | = | manipulators::tests::a_dragged_length_steps_with_the_zoom_as_live (Shift); Ctrl / Alt: missing |
| Esc during an arrow drag, button held, cancels the whole dialog; the release then does nothing | = | dialogs matrix "manipulators / Esc with the button held cancels the dialog" |
| The length step 10^(round(log10(100 px in mm)) − 2); angles 5° near a multiple, 0.1° between (0.4·step·(1 + level/3)) | = | dialogs matrix "drag / the length step follows the zoom", "drag / the angle step"; manipulators::tests::a_dragged_angle_snaps_by_its_radius_as_live |
| Transform › Translate by XYZ: the triad at the part's bounding-box centre; a plane square moves both its axes, each on the 0.1 mm step; an arrow only its own | ≠ (green e503ca2f, live2 ours-1118 manip): the triad sits on the parts' top-face centre (`triad.rs` current_triad, from the 10-24 note), live and std's findCenter the bounding box's centre; a plane square moves both its axes on the 0.1 mm step (YZ square (30, 20) px: Y 10.6, Z −9.4) and an arrow only its own (X 6.2, Y / Z unchanged): = | missing |
| During any triad drag only the dragged handle is drawn (blue), the others faint grey; the old place a light ghost with an orange outline | = (fixed 11-18, was ≠ on green e503ca2f): the dragged handle blue, the others faint; the old place light grey #C9CED2 at 0.6 with no edges, its selection only the orange band (`ghost_outline_only`); the moved part dark grey at 0.6 (`display_for_scene_inner`) | `a_transforms_old_place_is_a_light_grey_ghost` |
| Assembly triad: rings at the arrow tips, arcs with a degree box, 5° only near a multiple, plane squares without a box; Esc drops the box and keeps the place [asm] | = (89eca69c): rings, plane squares, centre ball, the ring's arc and degree box (5, 15, 20, 25, 27.958, 30 deg over 60 px), no box on a plane square, Esc drops the box and keeps the place; an arrow drag keeps its grab point (0 mm at the press, live m9) and no handle stays lit after release | a_triad_arrow_drag_keeps_its_grab_point |

### view-tools-1118 (view tools, live 11-18 01:41)

| Finding | Ours | Test |
|---|---|---|
| Keys: Y hide selected, Shift+Y show selected, P, Shift+P, Shift+H, K, Shift+I, Shift+V, Shift+X | = | app-core shortcut_sheet::tests::shortcut_sheet_lists_every_part_studio_key |
| Shift+Y with nothing selected does nothing; with a hidden part's row selected shows it | = | editing matrix "Y / Shift+Y with nothing selected or under the pointer" (the selected-row case: missing) |
| A hidden part's row menu: Show, Show all parts; the empty-space menu with hidden parts: Show all, Show all parts | = | list matrix "menu / a hidden part" (the empty-space menu: missing) |
| Render-mode and perspective switches change the picture in one step, not animated; perspective 45° | = (green e503ca2f, live2 ours-1118 view: one changed frame at 60 fps for Perspective on / off at 224 / 264 ms, Translucent / Shaded at 367 / 288 ms; Field of view default 45°) | missing |
| Named views: a 237 px panel; a restore animated (~240 ms on live's slow lane) | ≠ (green e503ca2f): the panel 265 px wide as live's (live's screenshot n1-dialog: 240–505; the 237 px was its title element), "Select a view" + ⊗ after a save: =; a restore is one step (one changed frame at 0.24 / 0.40 s), live animates it (3 frames over ~240 ms) | missing |
| Section view: the plane's icon is "Select default plane"; one drag arrow, a value box at it while dragging (0.1 mm steps), typing in it moves the plane | = | sketch_ui_tests::another_section_planes_arrow_drags_it (the box and typing: missing) |
| While the dialog is open the cut faces are a flat light-orange preview; after ✓ the caps are hatched in the part's colour | ≠ (green e503ca2f): flat preview with faint lines and a black outline, the committed caps hatched in the part's colour (=); but our preview is paler, #E3C9A7, live #EBBD8A / #E9C497; psui 11-18 22:4x: the cap shader gives live's colours (render `section_caps_pick_as_their_plane_and_hover_orange`: (230, 193, 146) vs #E9C497 on a #91BEDE part; #EBBD8A is live's grey #999A9E part), the pale reading is a cut lying on the shown Front plane, whose fill blends over it (live's take of that case asked, REQUESTS 22:48) | render offscreen (the colour) |
| Turn off section view; Shift+X again reopens with the last plane and offset | = | sketch_ui_tests::shift_x_after_turning_off_restores_the_last_section |

### sketch-entry-1118 (sketch entry and exit, live 11-18 02:06) [psui + skasm]

| Finding | Ours | Test |
|---|---|---|
| Top selected then Shift+S: the sketch opens on Top at once; nothing selected: an empty plane field; a face selected: "Face of Extrude 1"; the row added at the rollback bar, selected [skasm] | = | sketch_ui_tests::a_preselected_sketch_plane_is_not_an_undo_step, picking_the_sketch_plane_is_an_undo_step (the face label: missing) |
| A new sketch changes nothing on screen: nothing greys or hides, the view stays [skasm] | = | missing |
| Another feature row clicked while sketching: nothing | = | list matrix "sketching / another row clicked does nothing" |
| Editing a sketch rolls back: later rows grey italic, Parts (2) → (1); "Final"; ✗ brings them back | = | list matrix "sketching / later rows grey and the Parts list follows, ✗ brings them back" ("Final": missing) |
| A hidden later sketch stays hidden while editing | = | missing |
| Drawing on a face: the light-blue preview, then the solved lines [skasm] | ≈ faster: the solved lines in the closing click's frame (live's 0.58 s preview is its server solve) | missing |
| Orient normal to sketch on edit: an animated turn once the edit opens; ✗ leaves the view normal | = | sketch_ui_tests::orient_on_edit_turns_to_the_sketch_and_exit_keeps_the_view |

### toolbar-highlight (Search tools' highlight, live 11-17 09:43)

| Finding | Ours | Test |
|---|---|---|
| The found tool #FCDA7F with `inset 0 0 0 2px #FAB601`, nothing moves; a dropdown member lights the ⌄; steady | = | toolbar_highlight_tests::a_highlighted_toolbar_button_keeps_its_size_and_content |

### tooltips-1118 (toolbar tooltips, live 11-18 03:53)

| Finding | Ours | Test |
|---|---|---|
| 3 px under the button, centred on it; expanded 400 px wide, centred, the same top, clamped only at the page edge; no image; stays while hovered | = | chrome matrix "tooltip / under its tool, centred; 400 px expanded; stays while hovered" |

### pick-constants (live's client code: pick and snap tolerances, 11-18 02:39)

| Finding | Ours | Test |
|---|---|---|
| Pointer to canvas in device px, unrounded | = | the hat15b offset run (no test: input → viewport is GPUI's) |
| Pick reach round(5·dpr) device px: an 11 × 11 window at 1×, 21 × 21 at 2×, 8 at 1.5× | = | missing (pick_radius_px) |
| Hits ranked by type priority, then (1 + distance)·(1 − coverage), then id | = | render renderer::tests::picks_rank_by_priority_then_distance_and_coverage_as_live |
| Sketch inference: 15 px squares and bands under the pointer's pixel, points > curves > H / V lines [skasm] | = | snap matrix (the_sketch_snaps_matrix_matches_live) |
| Mate connector inference 50 px [asm]; Select other 8 px; the triad's free-drag ball 8 px | dots: ≈ (a 47.5 px square; live asm-feel3-1118 ±48, live2 place-1118 21:05 ±49–50 on two face sizes: midpoint taken at 50, not 51.5, vertex at 48, not 50.6; kept at 47.5 until the two agree), entering a face shows the centroid and the dots come on the next move as live (fixed 11-18); Select other: ours the pointer's ray only, the ball its disc + the 5 px pick window: ? until live2 measures both (REQUESTS asm 11-18 17:02) | `a_faces_connector_takes_a_dot_within_a_48_px_square_else_its_centroid` (dots); the rest missing |
| Hover speed 0.15 device px/ms; the pause min(5 × average pick, 250 ms) | = | app-core viewport::prehighlight_tests::prehighlight_waits_for_a_pause_as_live (device px: missing) |
| A left drag after > 2 device px and > 50 ms; the menu on right-up only within a 4 device px move path | = | app-core viewport tests::presses_drag_and_open_the_menu_as_live |
| Middle double-click within 500 ms fits | = | nav matrix "double middle click / zoom to fit" |

### nav-constants (live's client code: navigation, inference, manipulators, 11-18 02:44)

| Finding | Ours | Test |
|---|---|---|
| Orbit 3 turns per canvas diagonal, speeds averaged, inertia 0.77 a frame | = | nav matrix "right drag / orbits 3 turns per canvas diagonal"; app-core navigation::tests::inertia_decays_like_onshape |
| Wheel 2^(1/12) a notch (Windows ×3), about the pointer; pinch as Ctrl+wheel | = | nav matrix "wheel / a notch in" (Windows, pinch: missing) |
| Zoom refused past 100 px model size or 2^24 px per unit; perspective eye 0.05–50 diameters | = (qa 23:4x, code: `can_zoom` with MIN_MODEL_PIXEL_SIZE 100, MAX_PIXELS_PER_METRE 2^24, PERSPECTIVE_MIN / MAX_SCALE 0.05 / 50) | app-core navigation::tests::zoom_limits_like_can_zoom, perspective_zoom_dollies_by_a_sixth_of_the_distance |
| Z / Shift+Z ×1.75; Ctrl+Shift+arrows 150 px; drag zoom −dy/10; arrows 15° / 5° / 90°; fit padding 1.05; FOV 45 | = | nav matrix "keys / Ctrl+Shift+arrows pan 150 px in 250 ms", "keys / arrows: 15°, Shift 90°, Ctrl 5°, in 250 ms" (Z keys, fit padding, FOV: missing) |
| Detail drops under 20 fps, back over 30 fps | = | app-core navigation::tests::a_view_change_jumps_below_20_fps (the 30 fps return: missing) |
| Inference keeps the previous target while it is still under the pointer [skasm] | = (`infer`'s held `snap_target`, live's lastInferenceTarget; live 11-18 sticky-snap) | scene `the_line_snapped_to_last_stays_against_its_rank_not_a_point` |
| Linear step 10^(round(log10(100 px)) − 2), always rounded; angles 5° / 1° / 0.1° by a 50 px arc, ±0.4·(1 + level/3); Shift free | = | manipulators::tests::a_dragged_length_steps_with_the_zoom_as_live, a_dragged_angle_snaps_by_its_radius_as_live |

### measure-mass-1118 (P0, measure readouts and Mass properties, live 11-18 00:14)

| Finding | Ours | Test |
|---|---|---|
| 22 readouts on live's studio: a sloped edge's length, edge and face angles toward each one's middle (56.310 / 146.310 / 123.690 / 33.690), a cylinder's Diameter, Perp axes angle, Center axes dist, Face tangent angle first, vertex + face Min dist | = | measure_feel_tests::measure_pairs_on_lives_wedges_and_cylinder |
| The summary: the old text stays until the answer, then refills | = (fixed: it blanked while measuring) | measure_feel_tests::the_summary_keeps_its_old_text_until_the_answer |
| The Measure panel (`[`): 220 px at the right; two faces Area the sum; Length unit → Inch re-measures the panel only, kept in the session; Measure type filters the panel only | = | measure matrix "the details panel: 220 × 345 for a distance and its components" (the unit and type rows: missing) |
| Mass properties opens top-left over the list, (214, 79), 440 × 521; inertia in the account's mass unit | = (fixed 11-18: over the list, 32 px left of the graphics, 3 px under the toolbar; it was clipped to the graphics area, under the list; live2 fresh config: beside the list at (246, 80)) | measure_feel_tests::mass_opens_over_the_lists_right_part |
| Mass and section properties timing (live2 mass-appear-1118, software GL): panel +466 ms, values +555; a part added: chip +494, totals +1026, the old totals shown until then (no blank or spinner); removed: chip +96, totals +210 | local, faster; the old totals stay until the new ones come (they blanked: fixed) | measure_feel_tests::mass_keeps_the_old_totals_until_an_added_parts_come |
| A face click adds its part, a measured part clicked again leaves; volume / area summed; trailing zeros dropped | = | app-core properties_dialog mass_number tests (the add / remove: missing) |
| While Mass is open: no bottom-right summary, an empty click keeps its parts; Esc closes it keeping the selection | = (fixed: the summary showed) | measure_feel_tests::mass_hides_the_summary_keeps_its_parts_on_an_empty_click_and_esc_keeps_the_selection |
| Its button again leaves it open; `[` over it opens Measure empty, the parts deselected | = | measure_feel_tests::mass_stays_open_and_bracket_opens_an_empty_measure |
| A clicked Mass value shows the unit's highest precision (numfmt-1118) | = | measure_feel_tests::a_clicked_mass_value_toggles_the_highest_precision |

### measure-wedge1 (the angle rule on live's first wedge, 11-18 01:33)

| Finding | Ours | Test |
|---|---|---|
| Sloped face + x = 40 face 56.310 (the middles rule, not π − the normals' angle); + y = 0 33.690; + y = 30 146.310; the two top edges 56.310 | = | measure_feel_tests::measure_pairs_on_lives_wedges_and_cylinder; model-core regen::measure::tests::readout_rules_for_face_pairs |

### sketch-readouts (sketch selection readouts, live 11-17 09:57 / 10:10) [skasm]

| Finding | Ours | Test |
|---|---|---|
| A circle alone: Diameter (no Radius); an arc alone: Radius | = | scene sketch_mode::tests::pair_readouts_follow_lives_kinds_and_arcs |
| Pair order by kinds: with a point or a line Min · Max · Center; two circular curves Center · Max · Min | = | scene sketch_mode::tests::pair_readouts_follow_lives_kinds_and_arcs |
| Arcs count as their arc for Min / Max; Center dist to a line is to the infinite line; a point on a line only Min 0 | = | scene sketch_mode::tests::pair_readouts_follow_lives_kinds_and_arcs |
| Three decimals, the value's tooltip five | = | measure matrix "a value hovered: its 5 decimals' tooltip and its dashed line" |

### timings-1117 (click and double-click timings on live's 30-feature part, 11-17 10:17)

| Finding | Ours | Test |
|---|---|---|
| A face click: selected and drawn 57–67 ms (software GL); its Area readout 167–185 ms | = | latency_tests::everyday_interactions_beat_live_on_the_30_feature_part (T2, "click: the face selected and drawn") |
| Extrude 1 double-clicked: the dialog 322–448 ms, its preview 0.73–2.2 s | = | latency_tests::everyday_interactions_beat_live_on_the_30_feature_part (T2, "dialog: Extrude 1 open on its preview") |

### ui-speed-1118 (UI speed of everyday actions, live medians, 11-18 02:05) [psui + qa]

| Finding (live median, settled) | Ours | Test |
|---|---|---|
| Extrude dialog from the toolbar: shown 221 ms, settled 1.3 s; first preview after a pick 2.0 s; ✓ → regenerated 1.1 s | = | latency_tests::everyday_interactions_beat_live_on_the_30_feature_part (T2: "dialog", "✓" targets) |
| Open a document: shaded 7.6 s, list 11.4 s | = (faster; qa 13:31, green c7d388e8, live's speed document by CLI, Recently opened → click the name, 10 rounds, screen crops at 139 ms on Xvfb: shaded 1.22 s (0.93–1.27), list 0.73 s (0.67–0.80); qa-scratch/wf/spdlat.py) | missing (qa's speed suite measures it) |
| Tab A → B: list 1.7 s, settled 2.7 s; back 1.5 / 2.3 s | = (faster; qa 13:31, same run: A → B list 0.34 s, settled 0.49 s; back list 0.34 s, settled 0.34 s; medians of 10) | missing as a psui test (qa's dock flow; workspace a_clicked_tabs_card_picture_redraws_nothing guards the trailing frames) |
| Enter a sketch 226 ms / 1.3 s; exit (✓) 117 ms / 0.9 s; undo 107 ms / 1.0 s | = (green c7d388e8, live2 ours-1118 speed: block, xdotool input → 60 fps screen frames, one round each: enter (face selected, Sketch) first change 31 ms, graphics settled 0.58 s; exit ✓ the list at 0.12 s, settled 0.79 s; undo the list at 0.10 s, settled 0.42 s) | missing |
| Rollback bar drop: in place ~0.1 s, settled ~1.0 s | not a target (lead 11-18 23:14): software-render frame cadence; handler < 1 ms, frame 8 ms (green c7d388e8, live2 ours-1118 speed: block, xdotool input → 60 fps screen frames, one round each: the bar in place 0.24 s after an up-2-rows drop, 0.14 s back to the end, live ~0.1 s; the model settled at 0.35 / 0.27 s, live ~1.0 s). psui 11-18 23:00 frame log (green df6be9b6, Xvfb, software Vulkan): each of our frames is presented ~100–150 ms after the one before it, so two frames take 0.24 s; the drop's handler isn't the cost (the commit is WAL `synchronous=NORMAL`, no fsync). | – |
| Context menu on a face: shown 117 ms | = (green c7d388e8, live2 ours-1118 speed: block, xdotool input → 60 fps screen frames, one round each: shown 134 ms after the right-click, within a capture frame of live's 117) | missing |
| Mass properties: the dialog 161 ms; values 154 ms after the pick | not a target (lead 11-18 23:14): software-render frame cadence; handler < 1 ms, frame 8 ms (green c7d388e8, live2 ours-1118 speed: block, xdotool input → 60 fps screen frames, one round each: the dialog 94 ms (=); Volume / Surface area 313 ms after picking the part, live 154 ms; psui 11-18 23:00 frame log, green df6be9b6 on Xvfb: the pick's handler 0.35 ms and its frame's layout and paint 8 ms, but presents land at +128 / +300 / +394 ms (software Vulkan, ~100–150 ms a frame): the values come two frames after the pick, so the cost is the software renderer's frame time) | missing (sketch_ui_tests::mass_properties_dont_hold_up_the_frame checks only that the frame isn't held) |

### cfg-1118 (Configurations, live 11-18 01:03) [psui + studios]

| Finding | Ours | Test |
|---|---|---|
| The panel docks at the right, ~700 px, the graphics shrink; the empty state ~346 ms after the click | = | missing (the 712 px panel: no test) |
| The feature list's "Configurations" box with "Configuration [A ▾]" 193 × 26 | = | sketch_ui_tests::opening_the_configuration_select_moves_nothing (the place: missing) |
| A duplicate row name is accepted, stored as "A0" | = | sketch_ui_tests::a_duplicate_configuration_name_and_a_bad_value_wait_for_their_row |
| A row's menu: Switch to / Set as default / Duplicate row / Move up / Move down / Delete row | = | missing |
| A plain click on a row name doesn't switch | = | sketch_ui_tests::a_click_on_a_configuration_row_doesnt_switch_to_it |
| A bad configured value stays plain until its row is active; then pink and bold, its feature red, its part gone | = | sketch_ui_tests::a_bad_configured_value_is_pink_only_on_the_active_row, a_duplicate_configuration_name_and_a_bad_value_wait_for_their_row |
| Switching: no greying; the spinner only past 900 ms | = | missing |
| Configure from a field: the menu just "Configure", the toast with ×, the amber dotted outline until ✓ | = | sketch_ui_tests::escape_on_a_dialog_takes_its_configured_field_away (the toast: missing) |

### reverify-1118 (older feel facts retaken, 11-18 02:10)

| Finding | Ours | Test |
|---|---|---|
| 18 older facts retaken, 16 unchanged (documents rows, tab drag, sketch tool keys, Esc / empty-click selection, Offset arrow, Spline Esc, Trim trail…: their owners' existing rows) | = | their matrices' rows (Documents [studios], sketch tools [skasm]) |
| The Extrude arrow: a plain click flips the direction (was "does nothing") | = | dialogs matrix "manipulators / a click on Extrude's arrow flips it" |
| A drawing view's empty inside double-clicked opens View properties [studios] | = (green c7d388e8, block-drawing: a double-click inside the Top view's empty rectangle opens View properties with the view selected) | studios' |

### prefs-1118 (Preferences that change in-app behaviour, live 11-18 02:50) [psui + studios]

| Finding | Ours | Test |
|---|---|---|
| Use comma as decimal separator: a dialog's Depth reads "10,25 mm" | = (green c7d388e8, live2 ours-1118 prefs: set in My account › Preferences, then the block reopened: Extrude Depth "10,25" + Enter reads "10,25 mm", "10.5" reads "10,5 mm") | missing |
| 24 hour: H:mm with no leading zero ("2:47 Today", "0:00 Nov 7") | = | app-core start_library::tests::names_and_times |
| Display parts list properties: each part with the grey "In progress" pill | = (green c7d388e8, live2 ours-1118 prefs: set in My account › Preferences, then the block reopened: Part 1 with the grey "In progress" pill) | missing |
| Constrained rotate as default swaps the Alt prefix in the mouse table | = (green c7d388e8, live2 ours-1118 prefs: set in My account › Preferences, then the block reopened: ticking it swaps the table at once: Rotate "Alt + Right mouse button drag", Constrained rotate "Right mouse button drag") | preferences-ui `mouse_prefs::tests::constrained_rotate_as_default_swaps_the_alt_prefix` |
| Export with "Store file in a new tab": "<file name> is being prepared for export. …" at 0.55 s; the tab after the source tab; the view stays | = | export_feel_tests::a_stored_export_says_it_is_prepared_and_rings_the_bell |

### numfmt-1118 (number and unit display, live's client code, 11-18 02:56)

| Finding | Ours | Test |
|---|---|---|
| formatValue: Math.round(v·10^p)/10^p (12.3455 → "12.346", −2.5@0 → "-2"), trailing zeros dropped | = | units format::live_format_tests::values_round_as_live |
| Exponent form at ≥ 1e6, ≤ 1e-5 or rounding to 0 ("4.643e+7"); below the zero tolerance "0" | = | units format::live_format_tests::exponents_and_compound_units_as_live |
| Compound units sorted and spaced ("deg mm", "mm⁴") | = | units format::live_format_tests::exponents_and_compound_units_as_live |
| Measure readouts (v ± 1e-10).toFixed(p): zeros kept, ties away from zero | = | units format::live_format_tests::readouts_round_ties_away_as_live |
| A compound unit takes its units' largest precision | = (qa 23:4x, code: `format_quantity` uses the angle precision only for a pure angle, the length precision for everything else, so "deg mm" with length 2 / angle 4 shows 2 decimals where live shows 4; equal precisions (the default 3 / 3) look the same); fixed psui 11-18 23:23: a unit of lengths and angles takes the larger precision, a pure length or angle its own | units format::live_format_tests::a_compound_unit_takes_its_units_largest_precision |
| A dragged angle written at the workspace precision (3) | = | missing (manipulators.rs `format_number(deg, angle_precision)`) |
| A clicked Mass value: the unit's maximum precision, again back | = | measure_feel_tests::a_clicked_mass_value_toggles_the_highest_precision |
| Value-only parameters: {mm 5, in 6, cm 6, m / ft / yd 8, deg / rad 7} decimals, zeros dropped | n/a (qa 23:4x): every quantity in ours is stored as an expression (`ParamValue::Expr`) and shown as typed; the parity translator refuses a BTMParameterQuantity with no expression ("no expression"), so a live value-only parameter would fail to import rather than show | missing |
| Drawing PICKBOX 7 for every tool; grips 6 px [studios] | = (ef2dfdfd, 9f629e6f: every tool's pick, the detail loop, the hole table's X direction; the centre mark, detail circle, aux indicator and leader anchor grips) | GPUI feel `a_tool_picks_an_edge_within_7_px` |

### server-numfmt (server number formats by experiment, live 11-18 03:05) [core + studios]

| Finding | Ours | Test |
|---|---|---|
| Accepted: "3/8 in", "(1 + 1/2) in", "1 ft + 6 in", "1/2" (document unit), "1 inch" / "2 inches" / "1 foot" / "1 feet", "30 degrees", "0.5 radian"; rejected: "1 1/2 in", "3/8\"", "1'6\"" | ≠ | ours accepts a superset (inch / foot marks): the lead's decision, no test |
| Sketch dimension text, deg-min-sec and drawing fractional / feet-and-inches units are the server's [skasm / studios] | drawings = (ef2dfdfd, 1e21f28e: fractional inches to the precision, "2 3/4", `1'-2 1/4"`, drawn stacked); sketch: skasm's | drawing sheet `a_fractional_value_is_drawn_stacked` |

### hl-pixels-1118 (P0, highlight pixel columns, live 11-18 01:13)

| Finding | Ours | Test |
|---|---|---|
| The plain edge: dark, ~1 CSS px; the hovered edge band: opaque rim, 0.78 core, no dark border | = | scene tests edge_looks::edge_and_face_highlights_look_as_live |
| A part selected from its row: faces tinted (top F2B648, front CD9A3D), internal edges the plain dark edge under the tint, the silhouette a 2 device px #FFC04C outline at 1× and 2× | = | scene tests edge_looks::edge_and_face_highlights_look_as_live |
| A Parts row hovered: no tint, only the silhouette outline #FFC685, 2 device px | = | scene tests edge_looks::edge_and_face_highlights_look_as_live |
| A face selected with its part: the face E5A222 with its #FFA90A border on its side, the silhouette #FFC04C | = | scene tests edge_looks::edge_and_face_highlights_look_as_live |
| A sketch line: ~2 px blue at rest; hovered a #FFC685 band 5 px (9 device px at 2×), rim opaque, core 0.78; selected the same in #FFA90A | = | sketch_hover_feel_tests::sketch_lines_and_points_light_as_live |
| A sketch point: a ~5–6 px dot at rest; hovered an 11–12 px disc (21 device px at 2×), rim #FFC685, inside 0.78; selected #FFA90A | = | sketch_hover_feel_tests::sketch_lines_and_points_light_as_live |

### qv-fillets (Query variable Fillets / Matching, circle-point Coincident, live2 11-17 13:43) [core + skasm]

| Finding | Ours | Test |
|---|---|---|
| Fillets: only fillet faces of the seed's body, by compare type (a torus by its minor radius); Matching: the seed's body only, same size and shape | = | model tests features_b::query_variable_fillets_and_matching_as_live |
| The dialog shows no count | = (green c7d388e8, block: the Query variable dialog shows no count with a selection) | missing |
| Coincident of a circle / ellipse with a fixed point translates the curve the shortest way, size kept; a free point moves onto it [skasm] | = (fixed 11-18, constructive.rs) | onshape_ux `a_curve_onto_a_held_point_moves_whole_with_its_size_kept` |

### consts-1118 (feel constants: context-menu timing, double-click windows, live 11-18 06:48)

| Finding | Ours | Test |
|---|---|---|
| A feature row's double-click is the list's own: the same row again within ≈ 400 ms anywhere on the row (its far end, its icon; 420 ms or another row never) | = (fixed 11-18: ours needed the OS's 5 px) | list matrix "double-click / the list's own: a second click 80 px along within 400 ms" |
| A list row's menu opens on the right press; the graphics menu on the release within 4 device px | = | list matrix "menu / one feature"; app-core viewport tests::presses_drag_and_open_the_menu_as_live |
| The graphics double-click is the OS's (2 px, ≈ 400 ms on Linux) | = | GPUI's click count (no test) |

### hover-time-1118 (P0, hover timing with no tool and why a sketch tool hovers at once, live 11-18 06:11)

| Finding | Ours | Test |
|---|---|---|
| 3D, no tool, any entity: a fast sweep lights at stop + min(5 × average pick, 250 ms) + one pick | = | app-core viewport::prehighlight_tests::prehighlight_waits_for_a_pause_as_live |
| A fast move off clears at once (remove-only pick); nothing new until the pause | = | sketch_hover_feel_tests::a_quick_sweep_off_a_line_leaves_nothing_lit |
| Slow (< 0.15 px/ms): the highlight follows the pointer, one pick per move | = | app-core viewport::prehighlight_tests::prehighlight_waits_for_a_pause_as_live |
| Every move picks at once only while a sketch creation tool (Line, Dimension) is active; a sketch with no tool, feature dialogs, mates and Measure keep the pause | = (fixed: `Viewport::pick_on_move` only while a sketch tool is active; it was `tool_captures_left`, set for the whole sketch, a plane ring and assembly parts) | sketch_hover_feel_tests::only_a_sketch_tool_picks_on_every_move |

### keys-1118 mac (macOS key parity, live 11-18; live/keys-1118)

| Finding | Ours | Test |
|---|---|---|
| The shortcut list, key map and tooltips are the same under Mac, Linux and Windows user agents | = (bindings) | – |
| The Sketch button's tooltip "Create new sketch (shift+s)" | = (fixed: was "Sketch (shift+s)") | part_studio tool_tooltip_tests::toolbar_tooltips_read_as_live |
| The Tab manager button's tooltip "Tab manager (alt/⌥+t)" | = (fixed: was "Tab manager (Alt+T)") | – (a literal) |

### ps-feel2-1118 (configuration keys, hide / isolate, suppress, rollback drag, live2 11-18 08:46) [psui]

| Finding | Ours | Test |
|---|---|---|
| Configuration table: name + Tab in the add row adds the row, the selection jumps to the first row's Name cell | = (fixed: Tab moved to the next add row; now the first row's name is framed, `cfg_frame_pending`) | sketch_ui_tests::configuration_names_take_spreadsheet_keys |
| Typing on a selected configuration cell replaces it (no Enter first); Return commits and moves down; Esc restores the old text; Down / Up move the selection (Down from the last row: the add row) | = (fixed: row names framed as the value cells (`option_name_cell`); typing replaces, Return after typing commits and frames the row below (below the last: the add row), Up / Down move the frame on names and values; Return on a framed cell with nothing typed opens it, live not taken) | sketch_ui_tests::configuration_names_take_spreadsheet_keys, enter_in_a_configured_value_moves_down_a_row |
| A row name: a click selects the cell, a double-click edits the name inline (no switch), Esc cancels | = (fixed: a click frames the name cell; the double-click and Esc as before) | sketch_ui_tests::configuration_names_take_spreadsheet_keys (the frame) |
| A configuration row's menu (corrected 08:55: the earlier row was taken with another row current): the current row has no Switch to and no Delete row, the default row no Set as default, the first no Move up, the last no Move down; the default row can be deleted while another is current | = (`config_panel` row menu) | missing |
| Switching configuration (dropdown pick or the row menu's Switch to): the model changes in one frame ~1.0 s after the click; both lists stay as they are, no spinner; the dropdown takes Down + Return | = (green c7d388e8, live2 ours-1118 cfg: config-blocks with Extrude 1 Depth configured 20 / 40 mm: a dropdown pick changes the model in one frame at 0.30 s, both lists unchanged, no spinner; Up / Down + Return switch it) | missing |
| Y: the Parts row to opacity 0.5 at 73 ms, the part gone at 436 ms; Shift+Y back: 108 / 562 ms | = (fixed 11-18: the part is hidden / shown in place at the key; it waited for the regeneration: 269 / 669 ms on live2's lane) | list matrix "keys / Y and Shift+Y: the part goes and comes back with the key, not the regeneration" |
| Shift+I with a part selected: the Isolate dialog at (248, 79) 220 × 196 and the other parts translucent (1.4 s); the selection cleared (a Parts row or a face alike; 08:55); nothing selected: the part under the pointer; the lists don't change; Esc closes it and ends isolate | = (the lists no longer grey Default geometry while isolating: fixed 11-18, live2 ours-1118 ISOLATE-DEFAULT-GREY; the part under the pointer, the cleared selection and Esc as live) | missing |
| Suppress from the row menu: the row italic, struck through, opacity 0.5 at 208 ms; Parts (N−1) at 332 ms; its consumed sketch turns black; the parts left renumber (Part 1, Part 2); the part gone at ~2.0 s. Unsuppress: 141 / 234 ms, graphics 1.0 s | = (local, faster; live2 ours-1118 on green 4a69d55b: the row 253 ms, Parts 403 ms (live 208 / 332), the part gone 0.95 s; Unsuppress 232 / 432 ms, 0.85 s: the list rows ~50–200 ms behind live's, the graphics ahead) | list matrix "suppress / the parts left renumber, the consumed sketch drawn", "suppress / from the menu, then Unsuppress" |
| Two features suppressed from one menu: one step, Ctrl+Z restores both | = (live2 ours-1118 on green 4a69d55b: one menu Suppress, Ctrl+Z restores both) | list matrix "suppress / two features from one menu: one undo step" |
| Rollback bar drag: starts after 6–9 px; the drop line snaps to the gaps between rows, switching at each row's middle; nothing regenerates while held (10 s) | = (the line and "nothing until release"; the 6–9 px start fixed: it dragged from 2 px) | list matrix "rollback bar / while held nothing moves or greys", "rollback bar / the drag starts past 6 px" |
| Rollback bar released: the rows below italic at 0.5, not struck through, 0.96 s; the graphics 1.11 s | = (struck through only when suppressed; local, faster; live2 ours-1118 on green 4a69d55b: rows 187 ms, graphics 537 ms) | list matrix "rollback bar / on release the rows below grey and the Parts list follows" (the look: render_feature_row) |

### sk-feel2-1118 (sketch Offset across the curve, dimension texts, live2 11-18 08:46) [skasm]

| Finding | Ours | Test |
|---|---|---|
| Offset arrow dragged back across the curve: the value goes through 0 and the preview jumps to the other side, the arrow flipping; the value shown has no sign; no stop at 0 or at the opposite edge; release keeps the preview, Enter commits with the palette ("24.8 mm") | = (the drag adds to the offset at the press on the arrow steps; over the curve it goes to the other side, the size unsigned; Enter opens the distance box) | sketch_mode `offset_preview_arrow_and_repeat` |
| A dimension label placed level with another dimension: follows the pointer 1:1 across it, no snap, no alignment line (sketch dimensions have no text inference; drawings do, D4) | = (the label at the pointer, stored where clicked) | sketch_mode `a_new_dimensions_label_snaps_to_no_other_dimension` |

### asm-feel3-1118 (instance / mate list, Insert, mate connector dots, live2 11-18 09:00) [asm]

| Finding | Ours | Test |
|---|---|---|
| Instance row hover → its outline in the graphics 1.07–1.25 s on live2's software-GL lane (the graphics' own hover 0.86 s there: the list adds ~0.2–0.4 s) | the outline on the next frame, no wait (live's 1.07–1.25 s is its software-GL lane plus its list's 0.2–0.4 s: a delay, not a target) | a_hovered_instance_row_outlines_the_part_without_a_tint (look only) |
| Mate row double-click opens its edit dialog | the same (`feature_row`: click_count ≥ 2 → `edit_mate`; a single click only selects) | assembly_feel_tests `a_mate_rows_double_click_edits_the_mate` |
| Mate Suppress / Unsuppress from the row menu: the row italic struck through at 198 ms / plain at 150 ms | the same, italic struck through and faint on the next frame (`toggle_mate_suppressed`, `suppressed_text`) | suppressed_rows_are_italic_and_struck_through (look) |
| Mate rename (menu): the whole name selected; Esc cancels; Enter, Tab and a graphics click commit; F2 and Shift+N do nothing | the same: whole name selected, Esc cancels, Enter / leaving the field (Tab, a graphics click) commit; F2 unbound; Shift+N does nothing on a mate (26b5b87ff; an instance still renames) | shift_n_renames_a_selected_instance_but_not_a_mate |
| Instance row drag starts after 8 px; the ghost 5 px below the pointer; the drop line switches at a row's middle; the drop reorders, Ctrl+Z restores | the same (26b5b87ff): past 7 px, the pill 5 px below the pointer, the line above / below the row by its middle; the drop is one Undo step | a_dragged_instance_row_lands_by_the_rows_middle |
| Insert (I): the shell at 363 ms, the list at 633 ms, the search not focused; filtering 353 ms after the last key; a Part Studio row inserts all its parts ("Inserted: 2"); the preview 1.1–1.8 s after the click, a graphics click places it (1.2 s) | the same flow: no search focus, a Part Studio row inserts all its parts ("Inserted: N"), the preview follows the pointer, a click places; it filters on each key (no 353 ms wait: a delay, not a target) | closing_the_insert_dialog_after_esc_leaves_no_redo (part) |
| Mate connector tool: a hovered face's boundary, white dots at its vertices, edge midpoints and centre; no dot in reach: the triad at the face's centroid | the same: white dots at the face's corners (larger), edge midpoints and centre; none in reach: the centroid | a_faces_connector_takes_a_dot_within_a_48_px_square_else_its_centroid (in the mate dialog) |
| Mate connector dot reach: a square of about ±48 px (Chebyshev; diagonal kept at (25, 46), lost at (26, 48); off an edge midpoint kept at (47, 40), lost at (49, 42)) | a 47.5 px square (26b5b87ff; was a 50 px circle): live's four points kept / lost alike | a_faces_connector_takes_a_dot_within_a_48_px_square_else_its_centroid |

### chips-expr-1118 (dialog chips, expressions, the #variable list, live2 11-18 09:00) [psui]

| Finding | Ours | Test |
|---|---|---|
| A chip hovered: its tooltip with its name to the right; that entity yellow-green, the others still orange | ≠ (live2 ours-1118, green 4a69d55b): the edge yellow-green, the others orange as live, but the name tooltip showed under the pointer over the next chip (live: to the chip's right); fixed psui 11-18 22:38: to the chip's right (`with_user_side_tooltip`) | dialogs matrix "a field's row hovered" (the tooltip: missing) |
| A chip's × hovered: red, tooltip "Delete Edge of Extrude 1"; a click removes that chip | ≠ (live2 ours-1118, green 4a69d55b): the × turns red and a click removes the chip, but its tooltip reads the chip's name, not "Delete Edge of Extrude 1"; = since 11-18 (verified live2 20:2x) | missing |
| A click on a chip's body changes nothing; Delete and Backspace then do nothing | = (live2 ours-1118, green 4a69d55b) | dialogs matrix "a field's row clicked" (the keys: missing) |
| A bare expression keeps its form with the default unit: "10+5" → "(10+5) mm", "(10+5)/3" → "((10+5)/3) mm"; "25.4mm" → "25.4 mm"; "1 in", "10 mm + 1 in", "2*#wall" as typed; "7 xyz" kept, pink (invalid) | = (live2 ours-1118, green 4a69d55b): "(10+5) mm", "((10+5)/3) mm", "25.4 mm", "1 in", "7 xyz" pink | missing |
| "#" opens the variable list on the next frame: the matching variables (icon, name, grey value) and "New variable…", the first highlighted; it filters per key; a plain letter opens it too | ≠ slower (live2 ours-1118, green 4a69d55b): the same list (variables, grey values, New variable…, the first highlighted) but ~0.7 s after the key (live: the next frame); fixed psui 11-18 22:4x: the dialog redraws when the list opens (a bare "#" changed no value, so the list waited for the caret's blink) | missing |
| Enter on "New variable…": the "Create variable" panel beside the dialog, the name prefilled from the typed text; its Enter adds "#wa = <the field's value>" before the feature | = (green c7d388e8, vars: "#zz" + Enter on New variable…: the Create variable panel beside the dialog, Name "zz", Value "10 mm" selected; its Enter adds "#zz = 10 mm" before Extrude 1, Depth reads "#zz") | missing |

### sm-feel2-1118 (sheet metal panel, Sheet metal model arrow, Tab, Form, live2 11-18 09:12) [skasm]

| Finding | Ours | Test |
|---|---|---|
| Sheet metal table and flat view: graphics shrink 0.40 s, panel 0.75 s, table text 2.6 s, flat part drawn 2.9 s; the strip moves to the panel's left edge; close in one frame at 1.0 s | ≈ faster: the panel and table in the first frame (`toggle_panel`), the flat part computed in the background (`ensure_flats`); the strip sits at the graphics' right edge, so at the panel's left; the same button closes it in one frame | missing (strip position, one-frame close) |
| Bend row hover: the bend lit in 3D and flat at 0.59 s, the row tint at 0.69 s | = (`sm_hover_refs` in 3D, the bend zone outlined in the flat view), at once | sheet metal matrix "table / a row hovered lights its bend" (3D only; the flat side and timing: missing) |
| Sheet metal model edit: a white arrow at the plate's middle (blue when hovered); a click flips it and ticks "Flip direction up" | = std's own flip manipulator (`addFlipDirectionUpManipulator`), a click sets flipDirectionUp; at the plate's middle pointing up: inside the feature std's qCreatedBy(id, BODY) is the definition sheet, not the part built at its end (live smflip-1118; was a side face of the part, fixed 11-18) | model `sheet_metal_model_adds_std_s_flip_manipulator` (place and direction; colours: missing) |
| Flange edit: no manipulator in the graphics (only the Bend angle flip button) | = (std adds manipulators only for a partial flange) | sheet metal matrix "Flange" |
| Tab: a picked profile fills "Flange to merge" by itself ("Face of Sheet metal model 1"), Subtraction offset 0.02 mm; the flat view shows the tab at once; no arrows | = std's editing logic fills the merge face from the profile and the offset with the model's minimal gap (0.02 mm); no manipulators; the flat as soon as its background recompute ends | model `tab_editing_logic_fills_merge_face_and_offset` (T2) |
| Form: "Preparing form…" with a spinner at the graphics' bottom centre while loading; Location(s) takes points, a face click leaves it empty | ≈ Location(s) takes sketch points and mate connectors only (std's filter), so a face is refused; no "Preparing form…": ours has no load to wait for (the library thumbnails load in the background) | missing (a face click) |

### asm-views2-1118 (exploded, named positions, section, live2 11-18 09:19) [asm]

| Finding | Ours | Test |
|---|---|---|
| Explode arrow drag: the step's Distance = the pointer's travel at the screen scale, rounded to 0.1 mm (13 px → 1 mm … 60 px → 4.4 mm at ≈ 13.6 px/mm); 4–10 px drags give a 0 mm step | the same: the travel at the screen scale, rounded to 0.1 mm (0.01 in imperial) (`explode_drag_round`); a short drag makes a 0 mm step (asm, 11-18) | explode_drag_values_round (rounding; the 0 mm step: missing) |
| Explode step: Enter keeps it; Esc removes it and ends the explode editing | the same (asm, 11-18): Enter keeps the step; Esc on a step a drag just made removes it and ends the editing | esc_on_a_new_explode_step_removes_it_and_ends_the_editing |
| No explode player or slider: Activate shows the exploded state in one jump (1.3–1.7 s); the steps' rollback bar moves the parts in one frame (1.4–1.7 s), not animated | the same: no player; Activate jumps to the state and to fit in one frame (asm, 11-18: was an animated zoom); the rollback bar moves the parts in one frame on release | activating_an_exploded_view_jumps_to_it_and_to_fit |
| Named position row menu: Apply / Edit / Duplicate / Update named position, Delete; a click or double-click on the row applies nothing | the same menu (Move up / down only with 2+ positions, Apply left out on the applied row: live 156–161); a click applies nothing (it edits the cell) | missing |
| Apply named position: the parts jump in one frame (0.57 s), no animation; the row then selected | the same: one frame, the row marked applied; plus the "<name> is applied." toast (live extras-1118) | missing (model: named_position_restores_driving_mate_values) |
| Display states: a toolbar tool, not a right-strip panel | the same: a toolbar tool (`tb-display-states`), its panel on the left | missing |
| Assembly section arrow drag: a focused value box at the arrow during the drag (0.1 mm steps), tan hatched caps on every instance; the box stays after release, text selected | the same (asm, 11-18: the 0.1 mm steps and tan hatched caps while dragged were missing): the value box at the arrow during the drag, after release focused with its text selected | missing |

### live recheck 11-18 (today's landed Part Studio and sketch rows, green b5b664aaa, live 21:11–21:20 UTC)

Our app on its own Xvfb against live's values, by live's probes (pointer steps, screenshots, a 60 fps x11grab for timings; behaviours "live recheck 11-18";
live/feel/recheck-b5b664aaa/). Sketch reach outside sketch mode and the plane-first-move box were taken by live2 / qa today (TICKETS ce74247af, c19796e45).

| Row | Ours vs live |
|---|---|
| Edge hover reach | lights at 7 px = ; held to 8, lost at 9 (live 9–10) ≠ → fixed (a held edge picks 11 px wide); **retake c3d3470d2: lights at 7, held to 9, lost at 10 =** |
| Default plane row double-clicked | the plane stays selected = |
| Row tooltip | first visible 150–167 ms after the row lights, opaque ~250 (live 65–100 / 166–209) ≠ → fixed (the fade starts at a fifth, 110 ms) |
| Alignment on a snap (no rest) | = |
| Alignment with a passed midpoint (retake c3d3470d2, d2e0556f3) | the click level with L's midpoint makes a point at the midpoint (MIDPOINT on L) and the new end HORIZONTAL with it, the guide from the midpoint, as live (align-mid-1118) = |
| View cube corners from Front / Top (retake c3d3470d2) | the front-right-top corner gives Trimetric from Front and from Top, front-left-top the quarter-turned one, the Top face from iso Top = |
| View cube press held on a face (retake c3d3470d2) | 0.40, 0.48, 0.55, 0.70 and 1.2 s holds all go to Top; live: ≤ 0.48 s Top, ≥ 0.5 s nothing ≠ [psui] |
| Symmetric on two lines | ≠: B lands on A's mirrored line by its midpoint (86.165, −4.247)→(72.297, 16.554); live by its start (81.538, 2.692)→(67.671, 23.494) (live2 mirror-sym-1118; ours by the CLI, select-first); **retake c3d3470d2 (e8a6d531c): B (81.538, 2.692)→(67.671, 23.494) =** |
| Trim past a chain's corner | ≠: stops at the corner (the line crosses the chain 0.12 mm either side of it); live trims on to the next segment; unchanged on green 3c4e4c9f3 (f6ea6d5c3); **retake c3d3470d2 (0d069c5a8): trims through the corner to the next crossing, as live =** |
| Dimension edit box | double-click, slow second click, label drag = ; the value drawn selected = once the window has focus (`xdotool windowfocus`; a bare Xvfb leaves the window inactive, so neither the box nor a feature dialog's field paints its selection) |
| Box drag | nothing lights during it = ; selection 67–83 ms after the box clears ≠ → fixed (selected at the release, no readback) |

### pattern-hole-1118 (P0, Linear / Circular pattern, Hole, Mirror dialogs, live2 11-18 09:27 + live 11:17)

behaviours "psui (lead 11-18 ~09:21, P0 feel): Linear / Circular pattern, Hole, Mirror" and "psui (lead 11-18 ~11:15, P0
feel): Hole dialog follow-up"; times on live's software-GL lane (preview times are its regeneration, ±150 ms polling).

| Finding | Ours | Test |
|---|---|---|
| Linear and Circular pattern: no manipulator, no distance / count / angle arrow in the graphics | = (green e503ca2f, live2 ours-1118 pattern: neither draws an arrow or a value box) | missing |
| A typed count or distance changes nothing until Enter (3–4 s watched); Enter: the preview at 0.8–1.5 s | = (green e503ca2f, live2 ours-1118 pattern: "3" and "40" typed: no change for 3.5 s; Enter: the preview at 0.32 s) | missing |
| Linear pattern: the preview instance (translucent) after the Direction edge is picked (3.0 s on live's lane) | = (green e503ca2f, live2 ours-1118 pattern: translucent instance 0.57 s after the edge pick) | missing |
| Skip instances ☑: "Instances to skip" and a small white ring at each instance's centroid (the seed a dot); a hovered ring turns blue | = (green e503ca2f, live2 ours-1118 pattern: white rings at the instances' centroids, the seed a dot, the hovered ring blue) | missing |
| A ring click adds the chip "(1, 0)" with a CLEAR link and drops the instance from the preview; a second click on it removes the chip | = (green e503ca2f, live2 ours-1118 pattern: chip "(1, 0)" with CLEAR, the instance gone at 0.74 s; a second click removes it at 0.24 s) | missing |
| Circular pattern fields: Entities, Axis of pattern, Angle 360 deg, Instance count 4, Equal spacing ☑, Centered, Skip instances | = (green e503ca2f, live2 ours-1118 pattern: the same fields and defaults; a vertical edge as axis: the preview at 0.35 s, no arrows) | missing |
| Hole: the dialog title (and the feature-list row) is the size summary "Ø 6.35 mm ↧ 12 mm", "… \| ⌴Ø 10 mm ↧ 5 mm" (Counterbore), "… \| ⌵Ø 10 mm X 90°" (Countersink); with no point its tooltip "[Hole] did not regenerate properly: No hole points selected." | ≠ (green e503ca2f, live2 ours-1118 pattern, start holes: block + sketch "Hole points"): the titles and list rows as live (Simple, Counterbore, Countersink, Metric "Ø 5 mm ↧ 12 mm"); but the no-point tooltip appends "A parameter value is not valid." after "No hole points selected." | missing |
| Hole picks: only sketch points light (0.31–0.47 s after the pointer stops) and take; a part vertex or a face never lights and adds nothing | = (green e503ca2f, live2 ours-1118 pattern: a sketch point lights 0.28–0.51 s after the pointer arrives; a top corner never lights) | missing |
| Hole: each point click adds a chip "Vertex of Hole points" (with the click), a click on a picked point removes it; the first pick fills Merge scope "Part 1" | = (green e503ca2f, live2 ours-1118 pattern: "Vertex of Hole points" 0.20 s after the click, Merge scope "Part 1", the preview at 0.85 s; a click on it again removes the chip) | missing |
| Hole callout: one "Ø6.35 THRU" label on a leader to the first hole, in the face's plane (a second line "⌴Ø10 ↧5" with Counterbore); it covers points behind it | ≠ (green e503ca2f, live2 ours-1118 pattern): "Ø6.35 THRU" in the face's plane, but on a long leader with the text along the face's far edge (overhanging it); live's lies on the face by the hole, over the other points. Fixed psui 11-18 22:4x: a short leader (14 px past the rim, live h1-p0 / h1-depth5 / h1-cbore), the shelf and text on the face beside the hole (scratch shot hole-sketch + Counterbore); the glyph mesh drawn through everything as the leader (2a6b696b); still ≠ (shot 23:2x): the glyphs are filled but small and thin (11 px cap in the face's plane, ~6 px in Isometric; live's read ~10 px and heavier, text ~1.1× longer): size and weight asked of live (REQUESTS 23:33) | – |
| Hole Simple / Counterbore / Countersink and Inch / Metric tabs: the title changes at 0.8–0.9 s, the preview at 2.5–2.8 s; Counterbore adds diameter 10 mm and depth 5 mm rows; Metric makes Ø 5 mm | = (green e503ca2f, live2 ours-1118 pattern: the title and list row at 0.13–0.28 s, the preview at 0.73–1.48 s; Counterbore 10 mm / 5 mm rows, Countersink 10 mm / 90 deg, Metric Ø 5 mm) | missing |
| Hole depth typed: nothing until Enter; Enter: the title at 0.5 s, the preview at 2.8 s | = (green e503ca2f, live2 ours-1118 pattern: "5" typed: nothing for 4 s; Enter: the title at 0.19 s, the preview at 0.79 s) | missing |
| Mirror: a side face in the Mirror plane box ("Face of Extrude 1"); the preview at 1.8 s; the operation turns to Add with "Merge with all" and Merge scope "Part 1" when the mirror touches the seed | = (green e503ca2f, live2 ours-1118 pattern: "Face of Extrude 1", the preview at 0.93 s, the operation Add with "Merge with all" and Merge scope "Part 1") | missing |

### asm-dstates-1118 + sm-form-1118 (Display states, Form placement, live2 11-18 09:25) [asm, skasm]

| Finding | Ours | Test |
|---|---|---|
| Display states: a toolbar tool; a floating panel at (246, 79) with "New display state name" focused, "+", "No display states created"; name + Enter saves the current visibility; then a native select "Select a display state" with ↻ / ✎ / ⊗; Esc closes the panel | the same panel (toolbar tool, name box + "+", "No display states created", then the select with update / rename / delete; Esc closes); the name box focused on opening (asm, 11-18) | display_states_opens_with_its_name_box_focused |
| Applying a display state: the instance rows at ~0.3 s, the graphics in one frame at ~0.85 s, not animated | the rows and the graphics in the same frame, not animated | missing |
| Form: the dialog at 0.95 s, "Preparing form…" with a spinner at the graphics' bottom centre for ~4.4 s; a sketch point fills Location(s) and Target face(s) by itself; the emboss preview ~5 s after the first point, ~1.5 s after the next; a placed location can't be dragged | = (green e503ca2f, live2 ours-1118 form: start sm-box + Sketch 4 points (30, 20), (45, 20)): Form Part Studio is remembered (std REMEMBER_PREVIOUS_VALUE: a fresh config opens empty, after one committed Circle emboss Form 2 opens with it, as live's account); the dialog at 0.10 s, no wait, a sketch point puts "Vertex of Sketch 4" in Location(s) at 0.19 s and fills Target face(s) by itself (0.64 s), the preview at 3.2 s, the next point's at 1.3 s, a placed location doesn't drag; the Location(s) row's two buttons as live's: the mate connector toggle and Flip direction (live form-buttons-1118: its "pencil" is flip-direction) [skasm] | features_c::form_editing_logic_fills_the_target_face (timings, buttons: missing) |

### studios-1118 answers (drawing feel, live2 11-18 11:22) [studios]

| Finding | Ours | Test |
|---|---|---|
| BOM table properties closed by Esc, ✕ or ✓: the table stays selected (grips, tan outline) | = (green e503ca2f, live2 ours-1118 drawing, start pin-block-asm-drawing: a placed BOM, right-click › BOM table properties…, closed by Esc, ✓ and ✕: grips and tan outline stay each time) | ui-parity drw-bom-table-properties (reader) |
| Drawing toolbar: the title tooltip at ~0.47 s, then the help popover at ~3.1 s (400 px wide at the frame's left, 37 px down, title bar + text, stays while resting); every tool but Undo / Redo has one | = (fixed 708cf617: the title tooltip at 450 ms, and 11-18 the kit's fade starting at 0.25 opacity, not 0: qa DRW-TOOLBAR-TOOLTIP-DELAY's extra ~150 ms was that invisible first frame on a software-GL display; the popover at 3.1 s, 400 px at the frame's left under the toolbar, title bar + numbered steps, covering the tooltip; texts our own words, DECISIONS 11-18) | GPUI feel `a_toolbar_tools_help_shows_after_3_s_under_the_toolbar` (440 ms none, 460 ms the tooltip, 3.2 s the popover) |
| Hatch Pattern list: under the field (1 px), its width + 2, 30 px rows, at most 341 px then scrolls; opens upward when the field is low; a second click on the field closes it and keeps the value | = (green e503ca2f, live2 ours-1118 drawing, start pin-block-asm-drawing, Hatch region: under the field, its width + ~2, 30 px rows; ANSI's 9 rows fit, ISO's list stops at ~340 px and scrolls; a second click on the field closes it, the value kept; the upward case not tried) | ui-parity drw-hatch-panel |
| Sheet properties › Custom: zones 4 × 4 (ANSI) / 6 × 4 (ISO) whatever the size before; width / height fields take the previous size's dimensions | = (green e503ca2f, live2 ours-1118 drawing, start pin-block-asm-drawing, ANSI A → Custom: zones 4 × 4, width / height 11 in × 8.5 in; ISO not tried) | ui-parity drw-sheet-properties-custom-zones |

### snap-diag-1118 (assembly Snap mode reach by direction, live2 11-18 11:40) [asm]

| Finding | Ours | Test |
|---|---|---|
| Snap mode corner reach is not a square: over the top face 45° up-left 72 px (51, 51), straight up 50; over the side face along the face diagonal 56 (26, 49), 45° down-right 49 (both land side-face aligned); off the part 50, the connector frozen at its last face spot and a release there moves nothing (corrects "57 px square") | a dot of the face under the pointer within 51 px in x and y (up-left (51, 51), straight up 50); off every part the connector stays and a release moves nothing (asm, 11-18) | a_snap_drags_reach_is_51_px_in_x_and_y, a_snap_mode_drag_keeps_its_move_through_esc_and_stays_in_snap_mode |

### field-keys-1118 + search-tools-1118 (value field keys, the "#" list's Tab, Search tools, live2 11-18 11:55) [psui, skasm]

| Finding | Ours | Test |
|---|---|---|
| A focused length field: wheel ±1 unit per notch, Up / Down ±1, Ctrl ±0.1, Shift ±10, Alt as plain, PageUp nothing; the text stays selected | = (green e503ca2f, live2 ours-1118 fields: Extrude Depth 10 → Up 11, Down 10, Ctrl+Up 10.1, Shift+Up 20.1, Alt+Up 21.1, PageUp 21.1, wheel up 22.1 / down 21.1, the text selected throughout) | missing |
| The wheel over an unfocused value field changes nothing | = (green e503ca2f: two notches over Depth unfocused, 21.1 mm unchanged) | missing |
| The "#" list: Tab takes the highlighted entry (as Enter), closes the list, the keyboard stays in the field | = (green e503ca2f: "#w" + Tab → "#wall_thickness", the list closed, "+1" typed into the field) | missing |
| Search tools (Alt+C): a 600 px panel under the toolbar box (which turns "Close search [escape]"), the input focused, "Start typing to search tools", footer hint and ☑ Highlight tool in toolbar (on) | = (green e503ca2f: x 1320–1919 from y 86, "Close search escape", the input focused, "Start typing to search tools", the footer and ☑ Highlight tool in toolbar) | missing [skasm] |
| Search tools filtering: every key, ~60–90 ms; rows icon / name / shortcut badge; the first highlighted with an info pane (icon, name, help text); its toolbar button lit yellow while Highlight tool is on | = (fixed 11-18; green e503ca2f, live2 in Sketch 1: each key filters in 81–99 ms (f, i, l), the rows (Fillet shift f, Fix shift j, Flow, Offset o, Chamfer, Transform, Insert DXF or DWG), the first highlighted with its info pane; no sketch toolbar button lit with Highlight tool on: now the found tool, its group's ⌄ for a member, a constraint, Construction, Extrude / Revolve) [skasm] | sketchtools "search / the found tool lit in the sketch toolbar"; ops tool_search::a_sketchs_tool_search_ranks_as_lives |

### place-1118 (drawing placement: projected view, dimension, live2 11-18 17:19) [studios]

| Finding | Ours | Test |
|---|---|---|
| Projected view (P) preview: the new view's box, a 1 px frame rgb(154,165,175) with its geometry in grey rgb(159,159,159), centred on the pointer | = (fixed 11-18: `placing_view` drawn grey (159,159,159) by `ghost_picks`, its box in a 1 px (154,165,175) frame 8 px out, as a selected view's) | GPUI feel `a_projected_views_preview_is_grey_in_a_thin_frame` |
| Projected view direction: within 22.5° of the parent's horizontal / vertical axis the orthographic view (third angle), locked to the parent's row / column; past 22.5° the isometric, free on the pointer (dx 173: dy 60 locked, 80 iso; dx 373: 140 / 160) | = (green 1000e2a3f: the same grid and sweep, 20° Right, 25–65° iso, 70° Top, 165° Left) | GPUI feel `a_projected_view_locks_within_22_5_degrees_and_turns_isometric_past` (live2's grid) |
| Projected view timing on live's lane: preview 0.51 s after the parent click, 0.30–0.36 s after a pointer jump (0.31–0.66 s into the iso band) | a delay, not a target: the next frame | missing |
| Dimension placement (D, one edge): the line at the pointer's height (text middle on the pointer) either side of the edge and inside the view; the text follows the pointer's x everywhere, inside the extension lines too | = (fixed 11-18: ANSI, the value's `text_along` from the pointer as it is placed) | GPUI feel `a_placed_dimensions_value_follows_the_pointer_and_crowds_its_arrows_out` |
| Dimension placement arrows: text clear inside: arrows inside; text against an extension line (pointer 13 / 19 px inside an end): both arrows outside pointing in (53 / 59 px: inside); text past an end: arrows inside, the line carried out under the text | = (fixed 11-18: a value within its gap and an arrow of an extension line flips both arrows outside, `crowded`; past an end the line carried out, as before) | the same test |
| Section view (Vertical): the first click only picks the parent view; then a dash-dot cut line follows the pointer's x 1:1 and a click fixes it only on an inference point (an edge midpoint, a ~14 px orange dot within ~7 px; a point on an edge within 4 px), elsewhere a click does nothing | ≈ (fixed 11-18: `section_inference`: a vertex / centre, an edge midpoint within 7.5 px (14 px dot), a point on an edge within 4.5 px; elsewhere the first click picks the view, later ones do nothing; no line before the view is picked; live2 re-checked the flow and reach on green 548d795fb: =); the following line grey rgb(127,127,127), a long dash and two short ones (83 / 15 / 15 px, 16 px gaps, live2 rr/slc.png; was blue-violet short dashes): fixed 11-18 | GPUI feel `a_vertical_sections_cut_is_fixed_only_on_an_inference_point`, drawing_tools `dash_tests` |
| Section view preview: a grey box with grey hatch, x centred on the pointer, y locked to the parent's row; the arrows point away from the preview's side | = (fixed 11-18: the section being placed drawn grey with its hatch, no label, in the thin frame; live2 re-checked on green 548d795fb) | `placing_view` / `ghost_picks` (the projected test) |
| Detail view: the first click is the circle's centre, then a solid black circle follows with the pointer's distance as its radius; the second click fixes it | = (fixed 11-18: the sizing circle solid black, 1 px; was the blue-violet dotted overlay) | GPUI feel `a_detail_views_preview_is_grey_with_the_parents_circle_plain` (the placing circle) |
| Detail view preview: centred on the pointer, free (no row or column lock), at 2:1; drawn grey rgb(164,164,164) in a thin square frame 12 px clear of its circle, no label; the parent's circle stays plain solid while placing | = (fixed 11-18: `placing_view` drawn grey (164,164,164), no label, `detail_hide_profile` on the previewed view so the parent keeps the overlay's plain solid circle, the frame 12 px clear) | GPUI feel `a_detail_views_preview_is_grey_with_the_parents_circle_plain` |
| Auxiliary view: after the edge click a dash-dot fold line with arrows and the preview follow along the edge's normal only (row-locked); the preview a grey framed box centred on the pointer, no letters or label; the fold line midway between the parent's edge and the preview's frame; over the parent the preview sits on it | = (fixed 11-18: the view being placed drawn grey in the thin frame, no label, no letters at the arrows, the fold line midway to the frame, previewed over the parent too; row lock, centring and the flip as before) | GPUI feel `an_auxiliary_views_preview_is_grey_framed_without_letters_and_shows_over_its_parent` |
| Break view: a zigzag break line at the pointer over the view while hovering; a click places each; after the second the view breaks at once, the gap shown with two zigzag ends | = (fixed 11-18: the break line in the dialog's line type, black, at the pointer over the view and at the first click; after the second the view drawn broken until ✓) | GPUI feel `a_break_line_follows_the_pointer_and_two_clicks_break_the_view_at_once` |
| BOM table placement: the fixed corner (top-right by default) on the pointer, the preview in black lines; near the frame's inner corner it snaps there with a 14 px hollow orange square (5 and 8.6 px taken, 12.8 not) | = (fixed 11-18: the frame's corners take the fixed corner within 10 px, the 14 px hollow orange square on it; the table under the pointer no longer lit, so black lines; its texts kept: live2 notes this take drew the cells without their texts) | GPUI feel `a_placed_bom_tables_corner_snaps_to_the_frames_corner_within_10_px` |
| Crop view drawing: after the first point a red ✕ and pink dotted inference line; the spline drawn open and black through the points to the pointer; the loop closes on the first point within ~5 px (5 / 3 px closed, 8 / 12 open), a click there ends it (orange, blue dots) | = (fixed 11-18: the profile being drawn black, open through the points to the pointer, the closed loop within 6 px of the first point where a click ends it; the red ✕ and dotted line only while the pointer is in line with the first point within 8 px (not 12), from it to the ✕ where the point snaps, horizontal through it just after its click (live2 wake-1118; live's stale leftovers not copied)) | GPUI feel `a_crop_spline_is_drawn_open_and_closes_within_6_px_of_its_first_point` |
| GTOL placement: the frame preview on the pointer (by its left edge's middle) with a pink alignment line; an edge lights orange from 4 px (8: not); a click on it attaches: the frame follows on a leader with an arrow at the clicked point (elbowed near, slanted far); a second click places it | = (fixed 11-18: a click on the lit edge anchors a leader with its arrow at the clicked point and the frame follows; a second click places it; the ⊕ a digit tall; the pink line is live2's stale woken point (wake-1118): a hovered vertex now wakes with a red ✕ that stays, and the frame lines up with it within 8 px (not 12) on a dotted line, snapping there) | GPUI feel `a_geometric_tolerance_clicked_on_an_edge_follows_on_a_leader_until_the_second_click`, `a_gtol_lines_up_with_a_woken_vertex_within_8_px` |
| Datum placement: the edge hovers orange with the symbol previewed; after the edge click the box follows on a solid black leader kept perpendicular to the edge (the foot slides under the pointer, within the edge's extent) | = (fixed 11-18: the hovered edge orange; after the click the leader square to the edge, its foot under the pointer and held at the edge's ends, no rubber band) | GPUI feel `a_datums_leader_stays_square_to_its_edge_within_the_edges_ends` |
| Surface finish placement: hovering an edge previews the symbol at the pointer on it (pink line, ✕); a click places it there at once and the tool stays on with the next preview on the pointer | = (fixed 11-18: the symbol previewed at the pointer, standing on the hovered edge, before and after each click) | GPUI feel `the_surface_finish_symbol_follows_the_pointer_before_and_after_a_click` |
| 2 point centerline snap: a hollow orange-red square rgb(255,89,0) / (255,130,63) on a vertex from 9 px straight-line along a 45° approach (≈6.4 px per axis; 10: none), as the Note tool's | = (fixed 11-18: the Note tool's hollow square, reach 9.5 px; the Dimension tool keeps its disc and 8 px) | GPUI feel `the_two_point_centerline_frames_a_vertex_from_9_px` |
| Note text: a click on empty sheet while typing commits the text and starts a new note there (placeholder "NOTE"); Esc drops the empty one | = (green 548d795fb, the same take) | missing |

### asm-fixed-drag-1118 (dragging a fixed instance, live icons2 10-23 + green 87edba241, live2 11-18) [asm]

| Finding | Ours | Test |
|---|---|---|
| A press-drag on a fixed instance: the cursor `no-drop` and a small fix glyph at the pointer, nothing moves, no toast; hovering it (no press) keeps the default cursor (behaviours "icons2 (10-23)" (b)) | = (the arrow with ⊘ and the fix glyph during the drag, the block stays; the default cursor at the release: the release refreshes the viewport's cursor, which ours had kept until a later hover) | `a_fixed_instances_drag_drops_the_no_drop_cursor_at_the_release` |
| A free instance's drag keeps the default cursor, no snap without Snap mode (icons2 (a)) | = (the default arrow) | missing |

### session-1118 (live's 10-minute modelling session, live/qa/session-1118/SCRIPT.md, replayed by qa 11-18 22:44 on green 87edba24) [skasm, core, studios]

Replayed in ours with the UI-script runner (qa-scratch/session/mksession.py, steps 2a-14c; the drawing from a seeded recent ISO_A3.dwt, the runner can't click a template row). Of live's 14
behaviours, 1 (no turn to the plane), 3 (box with the full-precision value selected), 4 (Enter keeps the field focused, text selected),
5 (the extruded sketch hidden), 6 (Add + Merge scope on a face sketch), 8 (Part pattern first), 9 (Mirror turns to Add), 10 (Sketch 1
edit rolls later features back, Parts (0)), 11 (undo takes back the bar move) and 12 (Rename selects the whole name) are =; 7's fillet
3 mm is live's remembered radius (spec default 5 mm, live 23:00): ours opens at the fresh 5 mm and, after an R3 Fillet ✓, the next
Fillet opens at 3 mm (qa-scratch/session/fillet-mem.json), =. Final part: ours 30133.35 mm³ /
30650.90 mm² against live's 29505.03 / 30273.90, from the pattern row below.

| Finding | Ours | Test |
|---|---|---|
| 2c: with no tool, click a rectangle corner, then the sketch origin: both selected; i adds COINCIDENT and the corner jumps to the origin | = (fixed, skasm 7a8ca3b20; qa 00:00 on green 99d2787e: corner selected, the origin click adds it (2 selected), i adds COINCIDENT (4 → 5)). Was ≠: the origin click cleared the selection | GPUI `a_corner_and_the_origin_selected_take_a_coincident` |
| 2d: in iso a dimension placement click at (40, −8) under the bottom line places the dimension (live retake 23:1x, 3 of 3; the session's plane pick was a click at (−15, 25) on the Right plane's face, a misfire) | = (places it, box open) | missing |
| 14a/b: Create Drawing of Tray, ISO_A3.dwt, No views, OK: the front view follows at View scale 1:2 (the title block SCALE 1:2) | = (fixed 11-18 [studios]: No views starts Insert view through the source pick, so the first view's scale is fitted per sheet: 1:2 for the 200 × 10 mm front on A3; the panel names the part by its property name, Tray, not its body name) | GPUI `workspace_tests::create_drawing_with_no_views_fits_the_first_views_scale` |
| 14b: after the front view and one projected view (below), further clicks right and diagonal place nothing | = (fixed 11-18 [studios]: Insert view's continuation places along the parent's axes only, a diagonal click places nothing; a P-started projected view still goes isometric past 22.5°) | GPUI feel `insert_views_continuation_places_no_diagonal_view` |
| 11: roll back to Extrude 2, insert Chamfer 1 (the hole's top edge, 1 mm), roll to the end: everything rebuilds (Linear pattern 1 = a Feature pattern of Extrude 2, 3 × 20 mm) | ≠ [core]: Linear pattern 1 fails "Could not create all instances as entered. Try selecting "Reapply features" option."; with Chamfer 1 suppressed it rebuilds (29378.70 mm³). TICKETS PATTERN-AFTER-INSERTED-CHAMFER | missing |

### shortcuts-1118 (assembly + drawing default keys, live2 11-18 23:32) [asm, studios]

Every key of live's `assembly` (56) and `drawing` (55) maps (live/keys-1118/linux/capture.json; the Mac map is the same
keys) pressed in live and in ours in the same state: an automated pass over all keys, then each key with a difference
redone from a fresh state (live page reloaded on a fresh lane copy, ours relaunched). Ours: green faf73238b (the
assembly pass) and f09f8eca8 (the drawing pass and every redo). Evidence live/keys-1118/live2/ (vp-asm, vp-drw: live
before / +1.5 s and ours before / +1.5 s for each key). Mac: drawing Ctrl+S / Q / M stay Control on a Mac, and
⌘C / ⌘V and Ctrl+C / Ctrl+V both copy and paste (live's Mac sheet); ours binds the same (`drawing_keys::control_key`,
`ctrl_cmd`) = (checked in the code; no app is run on the Mac).

| Key (scope) | Live | Ours | Status |
|---|---|---|---|
| Shift+R View in high quality (asm) | a blue banner at the top "Viewing in high quality. Performance can decrease. [Disable]" | the Part Studio's bar, shared (`os_ui::high_quality_bar`); Disable turns it off (GPUI feel `the_assembly_keys_act_as_live`) | = [asm] |
| W Zoom to window (asm) | a blue bubble "Drag to create zoom bounding box. ×" | the same blue bubble while armed; a box zoomed or Esc ends mode and bubble (`ZOOM_WINDOW_PROMPT`; `the_assembly_keys_act_as_live`) | = [asm] |
| W Zoom to window (drawing) | no bubble | = (fixed 11-18: no bubble; the next drag zooms) | = [studios] — GPUI feel `drawing_keys_as_lives_shortcut_sweep` |
| N View normal to (asm, a block face selected) | the view turns normal to the face (the Right view of the block) | normal to the last selected planar face, else the hovered one (`view_normal`; `the_assembly_keys_act_as_live`) | = [asm] |
| Ctrl+C (asm, Part 1 selected) | no toast | no toast (`the_assembly_keys_act_as_live`) | = [asm] |
| Ctrl+V after Ctrl+C (asm) | the copy comes in beside the original, see-through, following the pointer to be placed | the copy at the pointer at once, unselected, following it with its hidden edges drawn through (`place_at_pointer`; `the_assembly_keys_act_as_live`) | = [asm] (the see-through look: recheck) |
| Shift+S Snap mode (asm) | the toolbar button turns on; no bubble; a selection stays | the button turns on; no banner; the selection stays (`the_assembly_keys_act_as_live`) | = [asm] |
| Shift+P Hide all construction (asm) | hides the assembly's origin (its dot goes, the Origin row greys) | hides the origin (its row greys), a second press keeps it hidden as the Part Studio's; the origin is the Part Studio's 11 px ring with a dot (the triad only while picking it for a mate connector) (`the_assembly_keys_act_as_live`) | = [asm] |
| H Show mates mode (asm) | the toolbar button turns on and the Revolute's glyph (a pale square with the mate icon) shows at the mate | the hovered (else selected) instance's mates' glyphs, hidden ones too (GPUI feel `show_mates_mode_shows_the_hovered_instances_mates`); ours' capture had no hover (its before shot has no hover outline, live's has) | = [asm] (recheck with a pointer move first) |
| Y Hide (asm, the block selected) | the block hidden, its edges still drawn in yellow while it stays selected | the block gone entirely | ≈ [asm]: live's yellow lines are the block's hover outline left until the pointer moves (Part 1's row is no longer selected in live's +1.5 s shot), not kept |
| Shift+I / Shift+T Isolate, Make transparent (asm, a face selected) | the dialog; the face no longer highlighted | the same dialog; the selection cleared (`isolate_occurrences`; `the_assembly_keys_act_as_live`) | = [asm] |
| Shift+arrows (drawing: Coarse rotate) | pan the sheet 187 px a press on a 1919 × 1079 window (Shift+Down moves it down, Shift+Left left; measured on the full-size shots, live2 correction 11-18: "to the right" was wrong) | = (fixed 11-18: as Ctrl+Shift+arrows, a tenth of the canvas width toward the arrow: 187 px on live2's 1919 px window, its canvas ~1870 px) | = [studios] — GPUI feel `drawing_keys_as_lives_shortcut_sweep` |
| Shift+N Rename (drawing, a view selected, Sheets panel closed) | nothing visible (with the panel open: the view's row becomes a Rename box with ✓ ✕) | = (fixed 11-18: nothing; with the panel open the row's Rename as before) | = [studios] — GPUI feel `drawing_keys_as_lives_shortcut_sweep` |
| [ Show measure (drawing, a view selected) | the Measure panel at the top left ("Select entities to measure"), the selection cleared | = (fixed 11-18: the Measure panel at the top left, "Select entities to measure", the units dropdown (the drawing's unit "(Drawing)" first, then Inches, Millimeters, Centimeters, Feet, Meters, Yards; tooltip Units, live2 wake-1118 mu1c.png), Show dual unit; the selection cleared; no bubble) | = [studios] — GPUI feel `drawing_keys_as_lives_shortcut_sweep` |
| Space Clear selection (drawing, a view selected) | the selection cleared | = (fixed 11-18) | = [studios] — GPUI feel `drawing_keys_as_lives_shortcut_sweep` |
| Ctrl+S Sheets (drawing) | the Sheets panel opens over the left; the sheet stays where it is | = (fixed 11-18: the panel lies over the canvas's left, the sheet doesn't move; tool panels open beside it) | = [studios] — GPUI feel `drawing_keys_as_lives_shortcut_sweep` |
| The rest: asm 1–7 views, arrows, Ctrl/Ctrl+Shift arrows, Z / Shift+Z / F, I Insert, M Mate, Ctrl+M Mate connector, Shift+V Named views, Shift+X Section, ` Select other, J, K, S, Delete / Backspace; drawing D, Shift+D/R/H, Ctrl+M, L, C, G, R, M, H, V, I, Shift+L, B, T, N, P, S, Z / Shift+Z, F, arrows, Ctrl+arrows, Ctrl+Shift+arrows, Page Up/Down, Home, End, Shift+Q, Ctrl+Q, Ctrl+C / Ctrl+V (a view: the copy pasted beside it), Delete / Backspace | the same | the same | = |

### shortcuts-ps-1118 (Part Studio default keys, live 11-18 23:00–23:40) [psui]

Each of the 56 keys in live's `partStudio` map (live/keys-1118/linux/capture.json; the Mac map has the same keys), pressed once in live and once in ours from the same
state. Live reloads the page for every key, on a fresh "q keys" studio when the key changes the document; ours relaunches the green app (5c9229069 / c3d3470d2) on a
fresh copy of a block document. The pointer rests in the graphics at (1300, 700); keys that need a selection have Part 1 picked in the Parts list first. A
screenshot 1.3–1.5 s after the key (live/keys-1118/live/ps-pairs/). All 56 keys do the same thing in both apps: the same view, rotation, pan, zoom, dialog, tool,
toggle or bubble. Esc closes Curve/surface analysis in both. Mac: live's Mac and Linux maps match; ours binds Ctrl/⌘ pairs, Control-only keys and Redo as
live (`ctrl_cmd`, `shared_keys_follow_lives_list_per_os`) =. Feature-dialog widths and the sketch dialog's Show expressions differed only because of saved settings.

| Key (state) | Live | Ours | Status |
|---|---|---|---|
| A view key with the pointer resting on the model (Shift+1/2/3/4/5/6, Shift+Z, Ctrl+Shift+Down/Right) | whatever comes under the resting pointer lights after the view moves (a plane's or the part's orange outline, an edge) | nothing lights until the pointer moves | ≠ [psui] (retaken with a clean config and a 1 px move after launch: still unlit; a 3 px move after the key lights the Front plane, r-front2*.png) |
| Shift+T Make transparent (Part 1 selected) | the dialog; the part drawn as a pale see-through ghost, its row no longer selected | the dialog; the part stays selected, see-through orange, its row highlighted | ≠ [psui] |
| Make transparent's translucency slider | remembered across reloads (dragged to the left, it reopened there after a reload; restored to mid-way after); Isolate's own slider sits apart, near the left | starts at 0 like Isolate | note: live's mid-way was a remembered value, its fresh-account default not seen |
| A dragged dialog's spot | kept across reloads per dialog (browser localStorage `panelLayoutManager.<user>.<dialog id>`: `sectionview-dialog` at (356, 103) = 150 / 100 px from the graphics corner, `feature-dialog` at (206, 3), `select-other-dialog` at (1360, 675), …) | kept in panel_positions.json | = |
| Delete / Backspace (Part 1 selected) | the Delete part 1 dialog; the part drawn as a see-through ghost with orange edges | the same dialog; the part drawn solid, tan | = (withdrawn 11-19 01:28: at load 4 ours shows the ghost with orange edges by 0.5 s; live by 0.5 s too (orange at 0.15 s); the 1.3 s tan shot was at load 28) |
| Shift+X Section view | the dialog at the list's side (~ (396, 192)) | the same with a clean config dir (the low-left spot was a 11-04 panel_positions.json "section" entry on this machine) | = (withdrawn 11-18 23:58) |
| Shift+Y Show selected parts (Part 1 selected) | the selection cleared | Part 1 stays selected | ≠ [psui] (retaken 11-19 from a visible, selected Part 1 in live: the selection cleared) |

### shortcuts-sk-1118 (sketch default keys, live 11-19 00:00–01:15) [skasm, psui]

Each of the 81 keys in live's `sketchEdit` map, pressed once in live and once in ours from the same state, inside Sketch 2 (one line on Top) of the block document,
which is opened by double-clicking its row. Live reloads the page for every key. Ours relaunches green 5c9229069 for every key, with a clean config dir. A
screenshot 1.3–1.5 s after the key (live/keys-1118/live/sk-pairs/). Every key but Ctrl+M, Shift+N, Shift+X, ` and Shift+V starts the same tool, dialog, view or toggle in both apps (two of those also differ in look, rows below); Ctrl+V was not compared (the clipboards differed). Live's studio picked up a pasted second line from
Ctrl+V on, which doesn't change the keys after it.

| Key (in a sketch) | Live | Ours | Status |
|---|---|---|---|
| Ctrl+M | nothing; the sketch stays open | the same (fixed 11-19, `sketch_keys_as_lives`) | = |
| Shift+N Rename | the sketch dialog's name becomes an edit box with "Sketch 2" selected | the same; Enter names the sketch (fixed 11-19) | = |
| Shift+X Section view | the Section view dialog over the sketch (the sketch dialog greyed behind it) | the same; its Esc leaves the sketch open (fixed 11-19) | = |
| ` Select other | the Select other box "(No selections found)" at the pointer | the same (fixed 11-19) | = |
| Shift+V Named views | the sketch closes (Part Studio toolbar back) and the Named views box opens | the same (fixed 11-19, the sketch kept as ✓) | = |
| C, R, G, L, A, Shift+A, Shift+S (a draw tool from the key, pointer still) | nothing drawn at the pointer until it moves | the same (fixed 11-19, `a_draw_tool_from_its_key_draws_nothing_until_the_pointer_moves`) | = |
| Shift+C Curve/surface analysis | "Edge of Sketch 2" in the field, the line drawn orange, "Length: 36.856 mm" at the bottom right | the same field; the line stays blue, no length | ≠ [psui] |
| The rest: views 1–7, arrows, Shift / Ctrl / Ctrl+Shift arrows, Z / Shift+Z / F / W / N, constraints (I, Shift+O, E, H, V, B, Shift+L, Shift+K, T, Shift+M, Shift+Q, Shift+G, Shift+J), D, X, M, O, U, Q, Shift+A, Shift+U, Shift+F, S mini toolbar, Shift+E / Shift+W (close the sketch, open the feature), Shift+D, [, Shift+R, Shift+P / P / Shift+H / K / Y / Shift+Y / Shift+I / Shift+T, Delete / Backspace, Space, Ctrl+C | the same | = |
| A view key with the pointer resting on the model (Shift+1/2/3/4/5/6, Shift+Z, Ctrl+Shift+Down/Right) | whatever comes under the resting pointer lights after the view moves (a plane's or the part's orange outline, an edge) | nothing lit until the pointer moved: GPUI drops an element's hover at any key until the mouse moves; fixed psui 11-19: the viewport keeps its hover through keys (`InputModalityIndependent`) and picks again when hovered with no move | = (test a_view_key_lights_what_comes_under_the_resting_pointer; vibe2 repro) |
| Shift+T Make transparent (Part 1 selected) | the dialog; the part drawn as a pale see-through ghost, its row no longer selected | the dialog; the part stayed selected (orange); fixed psui 11-19: Make transparent clears the selection as Isolate does | = (test make_transparent_clears_the_selection; the slider's start mid-way: live default or saved, asked) |
| Delete / Backspace (Part 1 selected) | the Delete part 1 dialog; the part drawn as a see-through ghost with orange edges | the same dialog; solid tan in live's shot (1.3–1.5 s after the key at load 28); on the current build a pale ghost with the orange silhouette once the preview is in | open (live retakes with a 3 s wait) |
| Shift+X Section view | the dialog at the list's side like other feature dialogs (top left ~ (396, 192)) | the dialog low on the left (~ (336, 620)): a moved spot saved on 11-04 in the run's panel_positions.json | withdrawn (live 11-19: = with a clean config) |
| Shift+Y Show selected parts (Part 1 selected) | the selection cleared | Part 1 stays selected | ≠ [psui] (live's studio had Part 1 hidden first; to retake from one state) |

### reach-px-1119 (hover reach read from full-page screenshots, live 11-19 02:40–02:57) [psui, asm]

Lane 4. The pointer stepped 1 px at a time (450 ms per step) toward the target and back out, and each step was read from a full-page screenshot of what
live draws. Clip screenshots on this lane miss the hover overlay, so they're not used. The distances are px from the drawn edge pixel to the pointer.
These are what the user sees; reach-1118 (16:29) read the viewer's own pick and got more for the silhouette. Evidence: live/feel/reach-px-1119/.

| Target (iso, fit) | Live, drawn: lights at / still lit to | Ours | Status |
|---|---|---|---|
| A block's silhouette edge over the background (vertical, "start two-blocks" Part 1) | **5 / 5** (6 out: nothing), 2 rounds; a click 4 px out selects the edge, at 5 nothing (2 rounds) | 7 / 10 (green 88545e5d7, clean config, the same pixel method) | ≠ [psui]: live's drawn reach is 2 px shorter with no hold (reach-1118's pick-based 7 / 9–11 doesn't match what live draws or clicks) |
| The same edge in the assembly ("start two-blocks asm") | 5 lit, 6 not, 2 rounds | the Part Studio's (the assembly hovers through the shared Viewport pick, `pick_radius_px`; by the code, not measured): 7 / 10 until psui's change | ≠ [psui]: follows the shared pick |
| An edge between two faces, approached over its own face (the side face, at a quarter of the edge, away from its midpoint ⊙) | **7 / 7**: the face is hovered (thin boundary) to 8, the edge (thick) from 7 in, and back at 8 on the way out, 3 rounds (one at the midpoint, 2 at the quarter). A click 6 px inside the face takes the edge, 8 px takes the face (2 clean rounds) | – | [psui] over a face 7, over the background 5 |
| A vertex, approached along the screen diagonal over the background | lit at 8.5, not 9.9; held to 8.5 | – | (3d.json's 8–10 holds) |
| A plane's vertical outer edge (Front / Right in iso, over the background) | **6 / 6** (7: nothing) | – | (3d.json's "level border 7–8" read 1–2 px long) |
| A plane's slanted outer edge (Top in iso) | 7.2 lit, 8.05 not (row "A plane hovers…") | 9.1 / 9.9 | ≠ [psui] |
| Assembly hover timing ("start two-blocks asm", the pointer jumped onto Part 1's top face, 10 rounds) | the first draw 93–261 ms after the pointer stops (median 255), a second at ~515 ms more; the Part Studio the same (99–263, median 260) | the Part Studio's: the assembly's hover is the shared Viewport's (`prehighlight_delay`, app-core `prehighlight_waits_for_a_pause_as_live`; by the code) | = [asm] |
| Drawing: a click beside a view selects it (Top view of "start two-blocks", 1:2, left edge, 2 rounds) | selected out to 13 px, not at 14; hover lights at 13, not 14, with no hold (1 px steps, 450 ms, full-page shots, 2 rounds; row "View edge / inside hover reach" has 14–16) | – | [studios] |

## Fix queue (biggest feel impact first)
1. ~~Sketch click vs drag 5 px; inferences (parallel, perpendicular, H 7 / V 6, alignment after a rest); arc colour;
   Ctrl+click~~ (psui 11-13).
2. Sketch hover timing: 270–300 ms with no tool vs 30 ms with the Dimension tool is live's rule (hover-time-1118); ours
   picks at once in a sketch with no tool (psui, in progress).
3. ~~3D reach, priority, press thresholds~~ (psui 11-13). Left: the held edge against its own vertex (mate connector reach ≈, double-click fixed 11-14).
4. ~~Inference glyph position against the pointer (live ≈ 30, 30)~~: within 2 px (psui 11-14).
5. ~~Dimension tool placement rules~~: two points' H / V / aligned by the box, 48 / 48 of live2's positions (psui 11-14); the angle quadrant and Ø have their own tests.
