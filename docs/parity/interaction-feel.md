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
| Edge (over the background or its face) | lights at 6–7 px; stays lit to 9–10 | 6; 9 (a line picks at least 3 px wide, 9 hovered itself; the shaders' `picking`) | matrix; GPU `feel::a_click_beside_an_edge_selects_what_is_lit` | = |
| Vertex | 7 px across, 9 diagonally; stays lit to 7–8 | 7 / 9; 7 (a point picks at most 5 px wide) | matrix | = |
| Sketch line / point, not editing | 5–6, lit to 9 / 7, lit to 8 | 6, 9 / 7, 7 | matrix | = |
| A part in iso with the planes behind and through it (user P0 11-17) | the part lights, never a plane behind it: each face's middle (a face, or a seen edge by it), a top corner 5 px out its vertex; beside it a plane | the same | GPU `feel::part_faces_and_vertices_light_over_the_planes_behind` | = |
| A block wholly behind the Front plane (live psui/hover-pick-1120, 11-17) | a transparent plane never wins over geometry behind it: every face's middle lights the block; an edge with only the plane beyond lights at 5–6 and holds to 9 from the plane side, 4 / 8 from its face; the plane lights only where nothing is behind it | the same (planes see-through in the pick, priority 0) | GPU `feel::a_block_behind_the_front_plane_lights_through_it` | = |
| A sketch point vs its line, not editing (live hover-pick-1120 row 2) | moving along the line inward the point takes over 8 px before the end, the line back at 9; beyond the end / across 6–7 | the point's square: 7 along, 7 across | – | ≈ (1 px along the line) |
| Plane border | 7–8, no hold | 7, no hold (`LineSegment::WIDE_PICK`, `NO_HOLD`) | matrix | = |
| Assembly instance edge / vertex | 6 / 7, no hold | 6, no hold (`NO_HOLD`) / 7 | matrix | = |
| Silhouette edge | lights at 6, stays lit to 9–10 | as an edge | – | ≈ |
| Mate connector | its triad and ring: 23 px across, 14 down | drawn at Onshape's MATE_CONNECTOR_DIAMETER_PX 36 (`MC_R_OUT_PX` 18) with the 5 px pick: 23 across, ~15 down in the iso view | – | ≈ |
| Vertex vs its edge; edge vs edge 5.6 px apart | the vertex anywhere in its reach (a lit edge holds against it to ~4 px); the lit edge holds 3–4 px past the other | the vertex in its reach; the held band wins by distance | – | ≈ |
| Hover timing | a pick on every move below 0.15 px/ms; faster: after a pause of min(5 × the average pick, 250 ms) | the same (`prehighlight_delay`) | app-core `prehighlight_waits_for_a_pause_as_live` | = |
| Edge midpoint target after a rest | live 1954: 9 px square, a short dwell | `update_hover`, `MIDPOINT_DWELL` | existing midpoint tests | = |
| Click | adds; on a selected entity removes; on empty space clears; Shift+click on empty space keeps; Shift+click on an entity as a plain one | the same (`on_viewport_event`'s Click) | – | = |
| Ctrl+click | the context menu of what is under the pointer, selects nothing (every OS; on a Mac Cmd+click is a plain click) | the same | – | = |
| Double-click | a face: shows its feature's sketch drawn as an edited sketch looks and through the part (feel/3d dblclick-face.png: 2 px blue, its corner dots, the back base line seen through), the two clicks select then deselect, its feature and sketch rows related until the next click (an empty click clears both: live2 11-17 sel-feel 2, 3); an edge: nothing | the same (`sketches_shown_for_now`, `SceneOptions::through_sketches`; the next click clears `dblclick_related`). qa 11-18 on 591643fc: drawn depth-tested, 1 px in the inactive colour, it vanished into Part 2's base edges | part-studio-ui `a_face_double_clicked_shows_its_features_sketch`, GPU `a_face_double_clicked_draws_its_sketch_through_the_pointer` (two-blocks, Part 2) | = (fixed 11-18) |
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
| Alignment with another point | only a point hovered ≥ 100 ms first (the origin always); vertical 6 px, horizontal 8 (A) / 4 (B) | a point rested on ≥ 100 ms (`ACQUIRE_REST`), the origin always; 6 / 7 px | scene `a_point_aligns_only_after_a_rest_on_it` | = (horizontal reach: ≈) |
| Perpendicular | through the start, to a woken line (snapped onto) and from the chain's previous segment; 360 / L px (L the line's length); live 1866 (e): also across a line whose direction a constraint holds | the same | sketch `a_woken_line_gives_parallel_and_perpendicular_through_the_start`, `square_to_a_held_line_is_perpendicular`; matrix | = |
| Parallel | through the start, to a sketch line snapped onto (woken); never before (catalogue "wakeSummary") | the same | sketch `a_woken_line_gives_parallel_and_perpendicular_through_the_start`; matrix | = |
| Tangent (from an arc's end; ending on a circle) | 6–7 px; ending on a circle within 6 px of the tangent point (10 px: only Coincident) | from an arc's end 360 / L px; ending on a circle: the true tangent point from the start within the 7.5 px snap radius along the circle, the end placed on it (was a 3° angle: ~2 px on a 36 px circle) | sketch `tangent_hint_from_arc_end`, `a_line_ends_tangent_on_a_circle_near_the_tangent_point`; matrix rows line/arc-end/tangent, line-end/circle/tangent | = |
| Two at once | horizontal + on a line both; a point snap beats horizontal; a point beats its line | the same: H/V where the axis crosses a curve near the pointer takes both (`inference::infer`) | sketch `two_inferences_at_once_as_live` | = |
| The target snapped to last | stays while anything under the pointer is on it, ahead of a stronger snap; not forgotten when nothing is under the pointer (client WT@9146346 getInference lastInferenceTarget; live's reach probes reset it) | the same (`SketchEditor::snap_target`, `Inference::targets`; reset with the tool) | scene `the_line_snapped_to_last_stays_over_a_point_by_it` | = (code; a line slid into its own end asked of live 11-18) |
| An opened sketch an upstream change moved (the user's hat15b Sketch 1, 1.1 mm) | edited where it is shown: its lines light to 7 px either side | the same: the draft takes the regeneration's solve on entering, not an undo step (was the stored place: lit 30 px to one side, none 3 px to the other) | part-studio-ui `an_opened_sketch_is_edited_where_its_solve_shows_it`; checked in hat15b | = |
| Two at once, shown | perpendicular × a lit model edge (live2 combo-perpEdge, 11-14): ⊥ then coincident, 20 px boxes 2 px apart, the first +15 / +25 px from the pointer (a single glyph +20 / +20); the order is the created constraints' (live2 sketch-snaps.json "combined"); a parallel never meets a model edge (parEdge: only COINCIDENT); the reference segment and the line or edge landed on lit, the rubber band dashed and a dashed guide to an aligned point; both constraints; any two zones crossing combine (a direction or an alignment × a curve or an edge, a direction × an alignment; not two directions: perpExt), the crossing nearest the pointer of several ("three") | the same (`sketch_mode::inference_icons`, `glyph_offset`, the editor's crossings; glyphs by the pointer). Glyph order: the constraints' order, as live2's batch 2 | scene `a_combined_snap_shows_each_glyph`, `crossing_zones_combine_as_live`, GPUI `a_square_to_the_last_segment_crossing_a_lit_model_edge_takes_and_shows_both`, matrix `combo/*` | = |
| 3 point arc onto a semicircle | the third click within 7.5 px of the semicircle over the ends (inside or out, anywhere along it) draws that semicircle; MIDPOINT holds its centre at the ends' midpoint (live2 arcmid: 7.5 / 6.8 px yes, 8.0 / 7.9 no) | the same (`ARC3_SEMICIRCLE_PX`, `MidpointOf`) | scene `a_three_point_arc_snaps_to_the_semicircle`; ui-parity sketch-tangent-arc-from-arc | = (fixed) |
| Snap target look | yellow-orange square, 17 px, round a point / midpoint; none on a line | the same (`SNAP_SQUARE_*`) | scene, same | = |
| Inference glyph | ≈ 30 px right, 30 below the pointer (28, 28 mid-drag); same frame as the preview (35–60 ms) | its 20 px box centred 29 right, 31 below the snapped point | – | ≈ (within live2's ±2 px) |
| A dragged end onto an end / a midpoint / a line / a circle's rim | 9 (A) / 7 (B) / 6 / ≤ 7 / 6 (A), 3 (B) px; no H/V during a drag | 8.5 / 6 / 7.5 / 7.5 on screen (`point_drop_target`) | GPUI `a_dragged_point_lands_on_a_line_within_7_px` | = (rim: ≈) |
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
| A point and a circle / ellipse made coincident | the point fixed (a model vertex, a Fix): the curve shifts the shortest way, radii kept; the point free: the point moves onto the curve (live2 11-17, REQUESTS (d)) | the same (`what_moves`: the curve's centre or the point is what the solve moves) | scene `a_circle_coincident_with_a_fixed_point_shifts_and_a_free_point_moves` | = (fixed 11-17) |
| Hover timing (no tool) | the highlight ≈ 270–300 ms after the pointer stops; with the Dimension tool ≈ 30 ms | as the GPU pick arrives | – | – |
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
| Dimension tool | one line: aligned; two points: H / V / aligned by the pointer against their bounding box; two lines: the angle of the pointer's quadrant; a circle: Ø; the text centred on the pointer; the value box 100 × 26, ~33 px left of and 5 above the click | the hovered entity within reach | scene `two_points_dimension_as_live_by_where_the_pointer_is` (live2's 48 pointer positions) | = |

## Drawings (studios, from 2026-11-13)

Live: behaviours.md (lines cited) and live2's feel/drawing.json (D1–D6, F3, F4; written 11-13 03:22). Ours: px are screen px
(`tol_mm(px)` = px / zoom), mm are sheet mm.

| Item | Live | Ours (where) | Test | Status |
|---|---|---|---|---|
| View edge / inside hover reach (Select) | the whole view lights (tan frame ~8 px outside its geometry) from 14–16 px outside an edge, not 16–20; constant in sheet mm (D1) | 3.6 sheet mm round the view box (`VIEW_REACH_MM`, `view_under`) | GPUI feel `a_view_lights_and_is_taken_within_its_reach_in_sheet_mm` | = |
| Dimension line / extension line, centerline, centermark arm | 6 px, not 8 (D1; extension 4 / 6 at zoom B) | 7 px (`SELECT_LINE_REACH_PX`, `hit_reach`) | GPUI feel `a_dimensions_line_and_text_light_at_lives_reach` (dimension line) | = |
| Dimension / note text | 8 px outside the text box, not 10 (D1) | 9 px outside the box (`SELECT_TEXT_REACH_PX`, `text_box_distance`) | GPUI feel `a_dimensions_line_and_text_light_at_lives_reach` (dimension text) | = |
| Note leader | 10 px, not 12; lights the note (D1) | 11 px for its stroke and nodes in Select's hover too (`SELECT_LEADER_REACH_PX`, `leader_reach`; was the 7 px line reach) | GPUI feel `a_note_leader_lights_its_note_within_10_px` | = |
| Priority: annotation vs view | an extension line's foot on a view edge lights the dimension and a click selects it (3 of 3, live2 drw-feel-q (b)); a centerline over a view edge lights both and a click takes the view (D2); on the centerline 3 px inside the view, hover and click take the centerline (followUp (2)) | annotation first (−2.5 px bias); a centerline lit where it runs on a view edge gives way to the edge, which stands for the view (`centerline_on_edge`) | GPUI feel `a_centerline_on_a_view_edge_gives_way_to_the_view` | = |
| Snap reach (Dimension tool, Note leader) | 7 px, not 9, for vertices and circle centres; 5 px along an edge; constant in px (D3) | 8 px (`SNAP_REACH_PX`) | GPUI `a_views_vertex_snaps_within_8_px_on_screen` (7 snaps, 9 doesn't) | = |
| Snap marker, dimension tools | a filled 10 px disc rgb(216,165,102) for every kind, none in Select, none for a midpoint (D3) | the same (`SnapMark::Disc`), also the Dimension tool's sticky centre dots (bm 4062) | GPUI feel `the_dimension_tool_snaps_to_a_vertex_within_its_reach_and_select_marks_none` | = |
| Snap marker, Note tool | a hollow square ~15 px rgb(255,130,63) round the vertex (bm 26796, C11 (a)) | the same (`SnapMark::Box`) | – | = |
| Midpoints / quadrants | only after Shift+Q (D22) | the same | – | = |
| Two points in reach | the nearer, one marker (bm 4092) | the nearer (`snap_near`) | – | = |
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
| VS grid keys | click selects a cell; Enter / typing edits; Backspace clears; arrows move, ↑/↓ wrap; editing: Enter commits and moves down, Tab right (studios.json `variableStudioGrid`) | the same (`grid_key`, `grid_tab_action`, the `VariableGrid` key context) | `arrows_move_a_cell_and_wrap_up_and_down`, `tab_walks_the_rows_and_the_add_row`; ui-parity vs-grid-keys (live take asked) | = |
| VS row drag | a round blue badge of the first letter, a blue drop line with a dot; one undo step | the same (`VsRowDrag`, `row_drop`) | ui-parity vs-row-drag (live take asked); e2e `variable_studio_table` (`by` > 1) | = |
| VS column resize | grab 3–6 px inside a header's right edge; min 40 px | the same | ui-parity vs-column-resize | = |
| Versions rows | 50 px; hover #def1fb; selected #b2ddf6; a version row tooltips "Open version V1"; a single click on its name or date opens it (a double click too); the graph column clicks nothing; "Show changes…" lists the changes without opening the row; Main goes back to the workspace (`versionsPanel`) | the same (`graph_column`) | GPUI feel `version_rows_open_on_a_click_of_their_name_and_the_graph_column_opens_nothing`; ui-parity ver-* | = |
| Compare from a version row's menu | enters compare mode at once, no dialog: the workspace the base (blue row), the version the target (pink) (`versionsPanel.compare`) | the same (`compare_with_current`) | GPUI feel `compare_from_a_version_rows_menu_starts_at_once_against_the_workspace`; ui-parity ver-compare-* | = |
| Release candidate › Add to Release… | the Part Studio's row opens expanded with its parts; a click on a part's row picks it (blue), again puts it back (live2 rel-flow-two-parts) | the same (`AddItemsDialog`) | GPUI `add_items_opens_with_the_parts_listed_and_a_row_click_picks_one`; ui-parity rel-flow-two-parts | = |
| FS editor line keys | Ctrl+/, Ctrl+D, Ctrl+Shift+D, Alt+↑/↓, Alt+Shift+↑/↓ (Mac: Cmd, Cmd+Option) (fs0.json) | the same (`fs_lines`) | `ace_line_keys_comment_move_duplicate_and_remove` and unit tests | = |
| FS editor navigation keys | Ctrl+K / Ctrl+Shift+K (Mac Cmd+G), Ctrl+\\, Alt+E / Alt+Shift+E (Mac F4) | the same | `find_next_and_previous_wrap`, `ctrl_backslash_jumps_to_the_matching_bracket`, the keystroke test | = |
| FS autocomplete | keywords and snippets at the top level, std inside a function; opens 250–500 ms after a letter | the same lists; opens on the keystroke | fs_complete tests | timing asked (REQUESTS 12:58) |

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
| Insert Esc, pending | drops only the instance on the pointer, keeps the dialog | the same (asm e9941770) | asm-insert-esc-pending (live take asked) | = |
| Insert Esc, none pending | closes and removes the session's instances | the same (`cancel_insert`) | – | = |
| Insert ✓ | keeps all, the pending one at identity; one undo step | the same (asm e9941770) | asm-insert-ok-pending (live take asked) | = |
| Ring snap | 5° / 1° / 0.1° by the arc at the pointer's radius ≤ 50 px, within 0.4 × (1 + level/3) of a step | the same (asm 641ab465, `ring_snap`) | asm's tests | = |
| Snap mode ghost | the source part at 0.3 opacity | the same (17:18) | – | = |
| Snap mode lines | mates.json lists the client's INFERENCE_SNAP_* constants (rgb(255,194,14) dashed 4-4, 8 px box) | none | – | ? (client-wide constants, the sketch's qsnap; live's snap-mode capture, behaviours 1211, shows no lines) |

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
| Row tooltip | only for a cut-off name (a row whose name fits has none; the fix icon's "Part is fixed"), live2 11-14 | the same (the name measured against its room), at 160 ms (`ROW_TIP_DELAY`) | – | = (fixed) |
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
| Failed mate's colour | rgb(170,37,45) | the same (17:18; was #bd3039) | `hidden_mate_rows_are_grey` | = |
| Failed mate's marks | red "!" on the icon, "Fastened 1 has error: …", red Mate features icon and root overlay, ⚠ on the fixed instances | the same | – | = |

### 3D in an assembly (3d.json `assembly`): 10 / 10

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
| Double-click a default plane / a part | nothing / two toggles, no rename | the same (a part's name renamed: removed; ui-parity parts-row-dblclick agrees) | = (fixed) |
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
| Hover: the row tooltip | "Extrude 1 <- Sketch 1"; fades in from 160 ms, full by 390 | the text as before; shown at 160 ms with a 230 ms fade (was GPUI's 500 ms, no fade: `ui::ROW_TIP_DELAY`) | = (fixed) |
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
| A row hovered | lights its bend in the 3D and flat views | the same | = |
| Flange | no length arrow, a preview wall | the same | = |
| Flange / Hem edges box hover (live 11-17 sm-feel-1117) | rim and bend edges and a rip's thin side face light; a wall's broad faces and planes never | the same (`pick_muted`: the walls' faces out of the picks while that box is active; were lit) | = (fixed 11-17) |
| A joint row hovered (live 11-17 sm-feel-1117) | lights the rip in the 3D view and both its edges in the flat view | the same (`sm_rip_refs`; the flat edges `flat_view_side_edge`: each side face laid flat with the wall it closes; was nothing) | = (fixed 11-18) |
| Flange's flip (live 11-17 sm-feel-1117) | the opposite-direction arrow button at the right of the [Bend angle ▾] row; nothing in the graphics | the same (a flip after a row joins it) | = |
| Bend relief / Corner boxes hover (live 11-17 sm-feel-1117) | a rip's side face filled orange, rim / bend edges, a bend end's corner as an orange disc; the broad faces, planes never | the same (`SM_CORNER` takes planar faces, the walls' faces muted as Flange's box; was vertices and edges only) | = (fixed 11-18) |
| Type Rip → Bend that can't unfold (live 11-17 sm-feel-1117) | "Modify joint 1" added, the joint a bend (the corner shown bent), the yellow toast "Sheet metal model cannot be unfolded.", the flat view blank | the same (the host's unfold: a wall a second bend path places elsewhere (`Unfolded::clashes`) means no flat, `cannot_unfold`, the feature's warning SHEET_METAL_COULD_NOT_UNFOLD; a consistent loop, a cut splitting the plate between two flanges as qa4 S1, still unfolds; was a flat with the loop cut and no message). live2 read the feature as red: ours shows the warning (std's defineFeature rolls an ERROR back, so the bend wouldn't stay; hole.fs reads this status as a warning) | = (fixed 11-18; the row's colour to recheck) |
| Corner / Bend relief with a rip's side face picked (live skasm sm-rip-1118 (b)) | the click is taken as a chip "Edge of Sheet metal model 1" (the rip's definition edge), ✓ enabled, the title red: Corner "Corner 1 did not regenerate properly: Sheet metal corner not found.", Bend relief "… Selection is not a bend relief."; no toast | the same messages (std's sheetMetalCorner SHEET_METAL_RIP_NO_CORNER; `sheetmetal_ops` bend relief); the chip's text not checked | ≈ (chip unchecked) |
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
| Esc with that menu open | sketch/405–408: the first Esc exits the tool, the menu stays, a second closes it (live2's note reads otherwise: asked, REQUESTS 12:33) | the same as sketch/405–408 | = |
| Shift+F | the sketch fillet | the same | = |
| Trim, a click | removes the piece between the crossings | the same | = |
| Trim, hover | the piece up to the nearest crossings lit tan, orange dots at its ends | the same | = |
| Trim, a drag | every piece the trail crosses goes; one that nothing crosses any more goes whole (tr0) | the same | = |
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
| Sketch Ctrl+C / Ctrl+V of a rectangle (live2 11-18 keys-1118) | the copy at once, its bounding box's centre on the pointer, selected, Transform on with its triad on the lowest-left corner; it doesn't follow the pointer; its constraints come along (4 coincidents → 8); no toast; Ctrl+A selects nothing | the same (`paste_clipboard`, `Sketch::extract`; no Ctrl+A binding) | scene `copy_paste_selection` | = |
| Not yet in the matrix | the toolbar's Undo tooltip (always "Undo (cmd+z/ctrl+z)": ours the same), undo in a feature dialog (an existing test), the paste's 2D triad and kept constraints, the assembly rows (asm's); live editing-gaps2: a sketch paste lands at once centred on the pointer, keeps every constraint wholly inside the copy (dimensions too), drops those to entities left out; live editing-gaps3 (the client's triad): the 2D manipulator (paste and Transform alike) sits on the selection's lowest key point (curve ends, centres; the leftmost of equals), grey double outlines, screen-sized: X / Y arrow heads 68–96 px out (move along that axis), the square 20–45 px (move freely), the rotation ring 108 px out on X with two ticks (rotate about the anchor, an angle box), the scale head from (60, 60) px, 30 long (scale, a factor box); the ring at the anchor moves the triad only (ours the same since 11-16: `transform_triad_sits_and_moves_as_live`; was the bbox centre, a 14 px square, rotate 70 px, scale 45 px, no arrows; the agent API's `sketch_transform` pivot defaults to the same anchor). live editing-gaps4: the rotation and scale show in Onshape's quantity input (75 × 26 px, white, blue frame, 13.33 px text right-aligned: "20 deg" in whole degrees, the factor to 8 decimals), rotate's off the pointer while dragging then off the anchor, scale's left where its drag ended; both stay, their text selected for typing; a double click on the ring or the head does nothing (ours the same since 11-16: `XF_*_BOX_*`, `xf_released`; one take each). Not checked: an arc copy's paste offset (live: centre at pointer + (−2.3, −4.2) mm, unresolved) | – | – |

## Dialogs (matrix)

live2's dialogs.json (11-13 05:40–07:25; rules read from the client bundle). part-studio-ui
`dialogs::the_dialogs_matrix_matches_live` (T1, no GPU) on the block start, and `manipulators::snap_tests`.

**Matched: 31 / 31** (11-18 02:17 UTC: live2 parts-list-1118 and dialog-keys-1118 rows, 2 fixed; 23 / 23 at 11-17 22:59 UTC).

| Row | Live | Ours | Status |
|---|---|---|---|
| Which features have a manipulator | Extrude one arrow; Chamfer, Shell none | the same | = |
| An arrow's grab reach | within 14 px across (16 no), from 4 px behind its base to its tip, nothing past the tip | 8 px of the shaft or 14 px round the tip (past it too); now live's (`Viewport::manipulator_index_at`) | = (fixed) |
| The arrow | shaft 30 + head 30 px, head radius 10 | shaft 29; now 30 | = (fixed) |
| A dragged length's step | 10^(round(log10(100 px in the unit)) − 2) of the unit at the arrow (0.1 mm at 0.25 mm/px, 1 mm from 0.316); Shift: free | always 0.1 mm / 0.01 in, Shift ignored; now live's (`manipulators::linear_step`, our own and std arrows) | = (fixed) |
| A dragged angle's step | 5° within 2° up to a 573 px radius, 1° up to 2865 px, then 0.1°; free between; Shift: free | always 5° within 2°; now by radius (the finer windows divided by 1 + level / 3: asked, REQUESTS 12:48) | = (fixed) |
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
| Material on opening | 280 × 368 at the graphics area's bottom; nothing picked ('- None -') | the same | = |
| Material: a picked row | the list closes, the search keeps the keyboard with its text cleared; typing opens the list again; "No material assigned" stays under the open list (material-search-acetal-ok, material-library-pick-abs-cancel) | the keyboard went nowhere and the line hid while the list was open; now as live (the search stays in the tree, unseen) | = (fixed) |
| Material's search and Esc | filters as you type ("steel": 38 rows; live's read counted its highlighted first row twice), the first highlighted; Esc closes the list only, a second Esc the dialog | the second Esc did nothing (the keyboard was left on the closed list's search); now it cancels | = (fixed) |
| Material keys (live2 parts-list-1118 f2-f4) | "alum": Down from Alumina Oxide to Aluminum, Enter picks it, the part's look unchanged; Esc then closes the dialog unapplied | Esc after a pick did nothing (the keyboard on the closed list's hidden search); now it cancels | = (fixed) |
| Add appearance to face… (live2 parts-list-1118 e1-e4) | "Face appearance 1", 220 × 401 in the feature dialogs' place; "Faces or features" with the chip "Face of Extrude 2" and its ×; a swatch recolours only that face at once; ✓ keeps it, no feature row | 431 px at the graphics' bottom left, "1 face" in the field; now as live | = (fixed) |
| Keyboard on opening (live2 dialog-keys-1118) | a new Extrude / Fillet and an edited Extrude focus their first selection field | the same | = (new) |
| Tab from Depth (live2 dialog-keys-1118 b1) | ± → Direction → Starting offset → Symmetric → Draft → Second end position → help → out of the dialog; End type, the selection field, the tabs and ✓ / ✗ skipped; Shift+Tab back | the same | = (new) |
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
| Not yet in the matrix | Ctrl+Up ×0.1 (an existing test), the wheel over a focused field, the "#" list's New variable… and its popover, Tab taking the highlighted entry | – | – |

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

## Fix queue (biggest feel impact first)
1. ~~Sketch click vs drag 5 px; inferences (parallel, perpendicular, H 7 / V 6, alignment after a rest); arc colour;
   Ctrl+click~~ (psui 11-13).
2. Sketch hover timing: live2's 270–300 ms after the pointer stops (no tool) vs 30 ms with the Dimension tool: asked
   whether it is Onshape's or the recording Chrome's slow redraws before copying a delay.
3. ~~3D reach, priority, press thresholds~~ (psui 11-13). Left: the held edge against its own vertex (mate connector reach ≈, double-click fixed 11-14).
4. ~~Inference glyph position against the pointer (live ≈ 30, 30)~~: within 2 px (psui 11-14).
5. ~~Dimension tool placement rules~~: two points' H / V / aligned by the box, 48 / 48 of live2's positions (psui 11-14); the angle quadrant and Ø have their own tests.
