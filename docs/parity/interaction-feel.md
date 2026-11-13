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

| Item | Live | Ours (where) | Test | Status |
|---|---|---|---|---|
| Edge hover reach, pointer over the background | ? | 9 px (pick footprint: drawn width + `PICK_EXTRA_CSS_PX` 8, nearest within `PICK_RADIUS` 5) | offscreen `our_3d_pick_reach_for_edges_and_vertices`; GPU `feel::a_click_beside_an_edge_selects_what_is_lit` | ? |
| Edge hover reach, pointer over its own face | ? | 5 px (the face wins from 6) | GPU `feel::a_click_beside_an_edge_selects_what_is_lit` | ? |
| Silhouette edge reach | ? | – | – | ? |
| Two edges 6 px apart: which lights | ? | the nearer (nearest within the 5 px search) | – | ? |
| Vertex hover reach | ? | 11 px (6 px sprite) | offscreen `our_3d_pick_reach_for_edges_and_vertices` | ? |
| Vertex vs the edge it ends | ? | the vertex wins within its reach | offscreen, same | ? |
| Edge vs the face it bounds | ? | the edge within its reach (5 px over the face) | GPU, same | ? |
| Mate connector reach | ? | – | – | ? |
| Shown sketch's curve / point reach (not editing) | ? | – | – | ? |
| Datum plane border reach | ? | – | – | ? |
| Edge midpoint target after a rest | live 1954, behaviours "icons2 midpoint radius": 9 px square, a short dwell | `update_hover`, `MIDPOINT_DWELL` | existing midpoint tests | = |
| Hover timing: highlight during a fast move | live "nav2 pre-highlight": none above a speed, then after a pause | `prehighlight_delay`: speed ≥ 0.15 px/ms waits 5 × the average pick, at most 250 ms | – | ? |
| Click = hover (the click takes what is lit) | ? | yes: the click's pick is the hover's at the same pixel (`pick_at`), with the same Alt pick-through, field filter and midpoint target; assemblies map both through `pick` | GPU `feel::a_click_beside_an_edge_selects_what_is_lit` | ? |
| Click on a selected entity | deselects (Onshape) | deselects (`pick`) | – | – |
| Shift / Ctrl + click | ? | – | – | ? |
| Double-click | ? | 450 ms window (`is_double_click`); a face's second click deselects, its feature lit | – | ? |
| Select other | ? | – | – | ? |
| Box select: left to right / right to left | window / crossing (`BoxSelector`) | window / crossing; Ctrl+drag deselects | – | ? |
| Click vs drag threshold | ≈ 5 px from the press, not time-based | 5 px on screen (`DRAG_START_PX`) | scene `a_press_drags_from_5_px` | = |

## Sketch (editing a sketch)

Live: qsnap (behaviours "sketch snapping and hover", 11-12) and live2's "feel (sketch)" (11-13 00:55, live/feel/sketch.json;
two zooms, A 0.2754 / B 0.2316 mm/px; where they differ both are given).

| Item | Live | Ours (where) | Test | Status |
|---|---|---|---|---|
| A line's end snaps onto a line | ≤ 7 px, not 8 | 7.5 px (`INFERENCE_PX`) | inference tests; GPUI `a_line_ends_on_the_hovered_point_at_scale_1/2` | = |
| …onto a point / a centre | 8 px (A), 9 (B); onto a centre: COINCIDENT, not CONCENTRIC | 8.5 px (`inference::POINT_REACH`) | same | = |
| …onto a midpoint | ±5–7 px along the line | 6 px (`MIDPOINT_REACH`) | scene `a_snapped_point_or_midpoint_gets_the_square_and_a_curve_none` | = |
| Horizontal / vertical to the line's start | 7 px / 6 px offset at the pointer (not an angle) | 7 / 6 px (`inference::HORIZONTAL_REACH`, `VERTICAL_REACH`) | sketch `horizontal_reaches_7_px_and_vertical_6` | = |
| Alignment with another point | only a point hovered ≥ 100 ms first (the origin always); vertical 6 px, horizontal 8 (A) / 4 (B) | a point rested on ≥ 100 ms (`ACQUIRE_REST`), the origin always; 6 / 7 px | scene `a_point_aligns_only_after_a_rest_on_it` | = (horizontal reach: ≈) |
| Perpendicular | only when the line starts on a line (its end, a point on it, the chain's previous segment); 6–7 px; live 1866 (e): also across a line whose direction a constraint holds | the same | sketch `a_woken_line_gives_perpendicular_only_when_started_on_and_never_parallel`, `square_to_a_held_line_is_perpendicular` | = |
| Parallel | never | never | same | = |
| Tangent (from an arc's end; ending on a circle) | 6–7 px; ending on a circle within 6 px of the tangent point | 7.5 px | – | – |
| Two at once | horizontal + on a line both; a point snap beats horizontal; a point beats its line | – | – | – |
| Snap target look | yellow-orange square, 17 px, round a point / midpoint; none on a line | the same (`SNAP_SQUARE_*`) | scene, same | = |
| Inference glyph | ≈ 30 px right, 30 below the pointer (28, 28 mid-drag); same frame as the preview (35–60 ms) | its 20 px box centred 29 right, 31 below the snapped point | – | – (pixel check pending) |
| A dragged end onto an end / a midpoint / a line / a circle's rim | 9 (A) / 7 (B) / 6 / ≤ 7 / 6 (A), 3 (B) px; no H/V during a drag | 8.5 / 6 / 7.5 / 7.5 on screen (`point_drop_target`) | GPUI `a_dragged_point_lands_on_a_line_within_7_px` | = (rim: ≈) |
| Drags: free line end / body; held lines; dimensioned; rectangle; circle | see behaviours "feel (sketch)" (2) | – | – | – |
| Click vs drag threshold | ≈ 5 px from the press, not time-based | 5 px on screen (`DRAG_START_PX`) | scene `a_press_drags_from_5_px` | = |
| Hover of a sketch entity = what a click takes | a line lights within 8 (A) / 7 (B) px; sketch-offset hits a line 3 mm off | one reach for the hover, the click and the fallback pick (`sketch_hover_id`, `sketch_pick_at`, `in_sketch_reach`) | GPUI `a_circles_rim_lights_and_selects_the_circle_not_its_centre_beyond_reach`, `a_line_in_reach_lights_and_is_clicked_when_the_gpu_pick_misses_it`; GPU `feel::the_dimension_tool_takes_the_hovered_line` | = (px: –) |
| Hover timing (no tool) | the highlight ≈ 270–300 ms after the pointer stops; with the Dimension tool ≈ 30 ms | as the GPU pick arrives | – | – |
| Nothing recoloured under the pointer mid-draw | the target line doesn't recolour | the same (`SketchEditor::mid_draw`) | – | = |
| Hovered point look | a 10–11 px translucent orange disc, centred | the same; whole on a slanted plane | offscreen `a_hovered_sketch_point_on_a_slanted_face_draws_its_whole_disc`, `a_sketch_point_is_drawn_as_lives_in_each_state` | = |
| Per-entity colour: a line | the line black, its free end blue; black once dimensioned; a construction line through the origin black, ends blue | a line by its own geometry (`line_motion_queries`) | sketch `a_line_with_a_free_length_is_defined_and_its_free_end_is_not`, `a_line_whose_end_moves_off_it_is_under_defined` | = |
| …an arc | centre and radius held: the arc black, its ends blue | by its own circle (`own_motions`) | sketch `an_arc_held_on_its_circle_is_defined_and_its_sliding_ends_are_not` | = |
| …a circle, a spline | radius free: blue, centre black; a spline blue while any point is free | the same | – | = |
| Selection: click / Shift / Ctrl | a click adds, a click on a selected one removes; Shift the same; Ctrl+click opens the context menu | the same (Ctrl+click: `open_sketch_context_menu`) | GPUI `ctrl_click_in_a_sketch_opens_its_context_menu` | = |
| Clearing; double-click | a click on empty space clears, Escape doesn't; a double-click on a line selects then deselects (no chain) | the same (two toggles) | – | = |
| Box select | left → right: window, blue fill and border; right → left: crossing, yellow dashed; it adds | window / crossing (`#789ad1` / `#ffdc74` dashed) | – | – |
| Dimension tool | one line: aligned; two points: H / V / aligned by the pointer against their bounding box; two lines: the angle of the pointer's quadrant; a circle: Ø; the text centred on the pointer; the value box 100 × 26, ~33 px left of and 5 above the click | the hovered entity within reach | GPU `feel::the_dimension_tool_takes_the_hovered_line` | – |

## Drawings (studios, from 2026-11-13)

Live: behaviours.md (lines cited) and live2's feel/drawing.json (asked 11-13 00:40). Ours: px are screen px
(`tol_mm(px)` = px / zoom), mm are sheet mm.

| Item | Live | Ours (where) | Test | Status |
|---|---|---|---|---|
| View edge hover reach (Select) | lights within ~4 px, nothing at 11 (bm 26810, Dimension tool) | 5 px (`hover_at`, `BuiltSheet::hit`) | – | ? |
| Dimension line / extension / arrow reach | ? | 5 px; annotations win ties by 2.5 px (`hit` bias) | – | ? |
| Dimension / note text | ? | inside the text box (+0.15 × height pad) | – | ? |
| View's empty inside | the view's frame lights (bm 23413, 36443) | the smallest view box within 1 mm (`view_under`) | – | ? |
| Priority: edge vs annotation vs view | ? | annotation (−2.5 px) > edge > title block (+1.25 px) > view box | – | ? |
| Snap reach (Dimension tool, Note leader) | 10–13 px, 12 fits best, constant in px (bm 4078) | 12 px (`SNAP_REACH_PX`) | GPUI `a_views_vertex_snaps_within_12_px_on_screen` (two zooms) | = (live2 pinning it down) |
| Snap marker, dimension tools | a filled 10 px disc rgb(216,165,102) for every kind (bm 19501, D21) | the same (`SnapMark::Disc`), also the Dimension tool's sticky centre dots (bm 4062) | – | = |
| Snap marker, Note tool | a hollow square ~15 px rgb(255,130,63) round the vertex (bm 26796, C11 (a)) | the same (`SnapMark::Box`) | – | = |
| Midpoints / quadrants | only after Shift+Q (D22) | the same | – | = |
| Two points in reach | the nearer, one marker (bm 4092) | the nearer (`snap_near`) | – | = |
| Placement inference (note / dimension text) | pink dotted line, red ✕, gone 4 px off (bm 26796) | 3.5 px (`INFER_REACH_PX`), the same line and ✕ | – | = (live2: exact reach) |
| Box select left → right | ? (Part Studio: window, blue) | window (#1651B0 box) | – | ? |
| Box select right → left | ? (Part Studio: crossing, #FFC20E dashed) | crossing, Onshape's crossing box (`select_in_box`, `box_overlay`) | GPUI `a_box_right_to_left_takes_what_it_crosses` | ? (built 11-13) |
| Click on an item | toggles it, no modifier (help, bm 34761) | toggles (`select_click`) | – | = |
| Shift + click | adds views, notes, dimensions (bm 6004, 24332) | as a plain click | – | = |
| Ctrl + click | ? | as a plain click | – | ? |
| Click vs drag | ? (runs used a 0.5 s hold) | 3 px (`drawing_tools.rs` 1269) | – | ? |
| Dragging a projected view | follows its parent's axis (bm 13993) | locked to the axis (`MoveViews`) | – | = |
| Dragging a view near another's alignment | ? (asked: F4) | a free view's centre held on another's centre line within the inference reach, its line shown; Shift free (`align_view_drag`) | GPUI `a_dragged_view_holds_on_another_views_centre_line` | ? (kept or removed by live2's F4) |
| Double-click | ? | the OS's (GPUI: Linux 400 ms, 5 px) | – | ? |

## Documents page and tabs (studios, from 2026-11-13)

| Item | Live | Ours (where) | Test | Status |
|---|---|---|---|---|
| Documents list: click / Ctrl / Shift | ? (asked: F1) | a click selects one row, Ctrl (Cmd) toggles, Shift selects from the anchor, folders and documents in page order (`click_rows`) | `rows_select_alone_with_ctrl_toggles_and_with_shift_ranges_from_the_anchor` | ? (built 11-13) |
| Actions on N selected | "N selection(s)" wording (bm 35) | right-click menu Move to… / Send to trash, Delete key; "N selections will be sent to the trash." | – | ? |
| Documents dragged onto a folder | ? (asked: F2) | rows, folder tiles and parent crumbs take a drop (#def1fb); the selection moves, "N items" ghost | `rows_dropped_on_a_folder_move_into_it`; app check 11-13 (4 moved) | ? (built 11-13) |
| Row hover / selected | #def1fb / #b2ddf6 (bm 20) | the same | – | = |
| Tab drag: ghost, gap | 100 px ghost, a tab-wide gap, no line (bm 6562) | the same | – | = |
| Tab drag start | ? | GPUI's 2 px | – | ? |
| Tab dropped on a folder | lights (`hovered-group`), needs a dwell (bm 6576, 13328) | lights #def1fb, drops at once | – | ? |
| Tab bar auto-scroll while dragging | ? (asked: F5) | within 30 px of an end, 360 px/s whatever the frame rate (`autoscroll_tabs`) | `a_tab_dragged_to_an_end_of_the_strip_scrolls_it`; app check 11-13 (16 tabs: ~250 px in 0.5 s) | ? (built 11-13; the gap stays where the pointer last moved while the strip scrolls under it) |
| Tab manager drag and drop | ? (asked: F6) | rows into folders, before / after a row (2 px rgb(27,95,170) line), not undoable (tab operations) | `rows_dropped_on_a_folder_go_into_it`, `rows_dropped_on_a_tab_row_land_before_or_after_it` | ? (kept or removed by live2's F6) |
| Ctrl / Shift + click a tab | opens it in a new window (rq 2403) | the same | – | = |

## Fix queue (biggest feel impact first)
1. ~~Sketch click vs drag 5 px; inferences (parallel, perpendicular, H 7 / V 6, alignment after a rest); arc colour;
   Ctrl+click~~ (psui 11-13).
2. Sketch hover timing: live2's 270–300 ms after the pointer stops (no tool) vs 30 ms with the Dimension tool: asked
   whether it is Onshape's or the recording Chrome's slow redraws before copying a delay.
3. 3D reach, once live's numbers are in (an edge reaches 9 px over the background but 5 over its face today).
4. Inference glyph position against the pointer (live ≈ 30, 30).
5. Dimension tool placement rules (H / V / aligned by the bounding box, angle quadrant, Ø): a test each.
