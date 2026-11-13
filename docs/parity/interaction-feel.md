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
| Priority | sketch point > sketch line > mate connector > vertex > edge > origin > sketch region > face > plane; the nearest within it | the same (`PickKind::priority`) | matrix (a sketch line over a part edge, a sketch point on a vertex) | = |
| Edge (over the background or its face) | lights at 6–7 px; stays lit to 9–10 | 6; 9 (a line picks at least 3 px wide, 9 hovered itself; the shaders' `picking`) | matrix; GPU `feel::a_click_beside_an_edge_selects_what_is_lit` | = |
| Vertex | 7 px across, 9 diagonally; stays lit to 7–8 | 7 / 9; 7 (a point picks at most 5 px wide) | matrix | = |
| Sketch line / point, not editing | 5–6, lit to 9 / 7, lit to 8 | 6, 9 / 7, 7 | matrix | = |
| Plane border | 7–8, no hold | 7, no hold (`LineSegment::WIDE_PICK`, `NO_HOLD`) | matrix | = |
| Assembly instance edge / vertex | 6 / 7, no hold | 6, no hold (`NO_HOLD`) / 7 | matrix | = |
| Silhouette edge | lights at 6, stays lit to 9–10 | as an edge | – | ≈ |
| Mate connector | its triad and ring: 23 px across, 14 down | – | – | – |
| Vertex vs its edge; edge vs edge 5.6 px apart | the vertex anywhere in its reach (a lit edge holds against it to ~4 px); the lit edge holds 3–4 px past the other | the vertex in its reach; the held band wins by distance | – | ≈ |
| Hover timing | a pick on every move below 0.15 px/ms; faster: after a pause of min(5 × the average pick, 250 ms) | the same (`prehighlight_delay`) | app-core `prehighlight_waits_for_a_pause_as_live` | = |
| Edge midpoint target after a rest | live 1954: 9 px square, a short dwell | `update_hover`, `MIDPOINT_DWELL` | existing midpoint tests | = |
| Click | adds; on a selected entity removes; on empty space clears; Shift+click on empty space keeps; Shift+click on an entity as a plain one | the same (`on_viewport_event`'s Click) | – | = |
| Ctrl+click | the context menu of what is under the pointer, selects nothing (every OS; on a Mac Cmd+click is a plain click) | the same | – | = |
| Double-click | a face: shows its feature's sketch, the two clicks select then deselect; an edge: nothing | a face's second click deselects, its feature lit | – | ? |
| Select other (`) | the "Select other" panel, bottom right | `SelectOther` | – | – |
| Box select | left → right: window (faces and edges wholly inside, hidden ones too, no vertices, no part: 18 round a block, 5 round its right half); right → left: crossing (13 over that half); adds; Ctrl at the start deselects | the same (`box_ids`) | `feel::box_select_takes_what_live_takes` | = |
| Left press → drag (box) | more than 2 px and 50 ms after the press | the same (`press_becomes_drag`) | app-core, GPU feel | = |
| Right press | orbits from the first pixel; the menu still opens on release within 4 px and 1 s | the same (`right_release_opens_menu`) | same | = |

## Sketch (editing a sketch)

Live: qsnap (behaviours "sketch snapping and hover", 11-12) and live2's "feel (sketch)" (11-13 00:55, live/feel/sketch.json;
two zooms, A 0.2754 / B 0.2316 mm/px; where they differ both are given).

| Item | Live | Ours (where) | Test | Status |
|---|---|---|---|---|
| A line's end snaps onto a line | ≤ 7 px, not 8 | 7.5 px (`INFERENCE_PX`) | inference tests; GPUI `a_line_ends_on_the_hovered_point_at_scale_1/2` | = |
| …onto a point / a centre | a 15 px square: 7 px straight out, 9–10 diagonally (catalogue); onto a centre: COINCIDENT, not CONCENTRIC | the square on screen | matrix | = |
| …onto a midpoint | ±5–7 px along the line | 6 px (`MIDPOINT_REACH`) | scene `a_snapped_point_or_midpoint_gets_the_square_and_a_curve_none` | = |
| Horizontal / vertical to the line's start | 15 px bands: 7–8 px each side (catalogue; live2's first take: 7 / 6) | 7.5 px (`inference::HORIZONTAL_REACH`, `VERTICAL_REACH`) | sketch `lines_are_15_px_bands_and_points_15_px_squares`; matrix | = |
| Alignment with another point | only a point hovered ≥ 100 ms first (the origin always); vertical 6 px, horizontal 8 (A) / 4 (B) | a point rested on ≥ 100 ms (`ACQUIRE_REST`), the origin always; 6 / 7 px | scene `a_point_aligns_only_after_a_rest_on_it` | = (horizontal reach: ≈) |
| Perpendicular | through the start, to a woken line (snapped onto) and from the chain's previous segment; 360 / L px (L the line's length); live 1866 (e): also across a line whose direction a constraint holds | the same | sketch `a_woken_line_gives_parallel_and_perpendicular_through_the_start`, `square_to_a_held_line_is_perpendicular`; matrix | = |
| Parallel | through the start, to a sketch line snapped onto (woken); never before (catalogue "wakeSummary") | the same | sketch `a_woken_line_gives_parallel_and_perpendicular_through_the_start`; matrix | = |
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

Live: behaviours.md (lines cited) and live2's feel/drawing.json (D1–D6, F3, F4; written 11-13 03:22). Ours: px are screen px
(`tol_mm(px)` = px / zoom), mm are sheet mm.

| Item | Live | Ours (where) | Test | Status |
|---|---|---|---|---|
| View edge / inside hover reach (Select) | the whole view lights (tan frame ~8 px outside its geometry) from 14–16 px outside an edge, not 16–20; constant in sheet mm (D1) | 3.6 sheet mm round the view box (`VIEW_REACH_MM`, `view_under`) | – | = |
| Dimension line / extension line, centerline, centermark arm | 6 px, not 8 (D1; extension 4 / 6 at zoom B) | 7 px (`SELECT_LINE_REACH_PX`, `hit_reach`) | – | = |
| Dimension / note text | 8 px outside the text box, not 10 (D1) | 9 px outside the box (`SELECT_TEXT_REACH_PX`, `text_box_distance`) | – | = |
| Note leader | 10 px, not 12; lights the note (D1) | 11 px for its stroke and node (`leader_hit`) | – | = |
| Priority: annotation vs view | an extension line's foot on a view edge lights the dimension; a centerline over a view edge lights both and a click takes the view (D2) | annotation first (−2.5 px bias); the centerline case takes the centerline | – | ? (one reading; the extension-line click took nothing) |
| Snap reach (Dimension tool, Note leader) | 7 px, not 9, for vertices and circle centres; 5 px along an edge; constant in px (D3) | 8 px (`SNAP_REACH_PX`) | GPUI `a_views_vertex_snaps_within_8_px_on_screen` (7 snaps, 9 doesn't) | = |
| Snap marker, dimension tools | a filled 10 px disc rgb(216,165,102) for every kind, none in Select, none for a midpoint (D3) | the same (`SnapMark::Disc`), also the Dimension tool's sticky centre dots (bm 4062) | – | = |
| Snap marker, Note tool | a hollow square ~15 px rgb(255,130,63) round the vertex (bm 26796, C11 (a)) | the same (`SnapMark::Box`) | – | = |
| Midpoints / quadrants | only after Shift+Q (D22) | the same | – | = |
| Two points in reach | the nearer, one marker (bm 4092) | the nearer (`snap_near`) | – | = |
| Placement inference (note / dimension text) | 7 px, not 9; pink dotted line, red ✕ (D4) | 8 px (`INFER_REACH_PX`), the same line and ✕ | – | = |
| Box select look | left → right: light-blue fill, solid 1 px blue; right → left: pale-yellow fill, dashed yellow-orange (F3) | the same (`box_overlay`) | – | = |
| Box select, annotations | a note half inside: right → left only; fully inside: both (F3) | the same (`select_in_box`) | GPUI `a_box_right_to_left_takes_what_it_crosses` | = |
| Box select, views | a box over any part of a view takes the whole view, both ways, with the dimensions attached to it (F3) | the views the box meets, both ways (`select_in_box`) | GPUI `a_box_takes_the_views_it_reaches_and_adds_them` | = (the attached dimensions: ? live2 re-checking whether the view or the box takes them) |
| Box select with modifiers | none adds; Shift: nothing changes (F3) | adds; Shift does nothing | the same test | = |
| Click, Ctrl + click, Shift + click | all toggle the item: add if unselected, remove if selected (D6) | toggle (`select_click`) | – | = |
| Click vs drag | a view or a dimension moves from 5 px (4 is a click), a note from 8 (6 is a click); no hold needed (D5) | 4 px past the press for views and dimensions, 7 for notes (`drag_threshold_px`) | – | = |
| Box on empty sheet | shows by 12 px, not seen at ≤ 8 (D5, inconclusive below) | 3 px | – | ? |
| Dragging a projected view | follows its parent's axis (bm 13993); live2 saw a projected Top move freely (F4) | locked to the axis (`MoveViews`) | – | ? (asked) |
| Dragging a view near another's alignment | no snap at 16 down to 1 px, Shift or not (F4) | none (built 11-13, removed on live2's answer) | – | = |
| Double-click | ? | the OS's (GPUI: Linux 400 ms, 5 px) | – | ? |

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
| Tab drag start | ? | GPUI's 2 px | – | ? |
| Tab dropped on a folder | lights (`hovered-group`), needs a dwell (bm 6576, 13328) | lights #def1fb, drops at once | – | ? |
| Tab bar auto-scroll while dragging | ? (asked: F5) | within 30 px of an end, 360 px/s whatever the frame rate (`autoscroll_tabs`) | `a_tab_dragged_to_an_end_of_the_strip_scrolls_it`; app check 11-13 (16 tabs: ~250 px in 0.5 s) | ? (built 11-13; the gap stays where the pointer last moved while the strip scrolls under it) |
| Tab manager drag and drop | ? (asked: F6) | rows into folders, before / after a row (2 px rgb(27,95,170) line), not undoable (tab operations) | `rows_dropped_on_a_folder_go_into_it`, `rows_dropped_on_a_tab_row_land_before_or_after_it` | ? (kept or removed by live2's F6) |
| Ctrl / Shift + click a tab | opens it in a new window (rq 2403) | the same | – | = |

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

**Matched: 55 / 55** (11-13 06:02 UTC). Probes: a point 6.5 px straight out snaps, 8.5 doesn't; 9.5 / 11.5 diagonally;
a band 6.5 / 8.5 across (live's own takes vary by a pixel with where its target sits in its pixel).

| Row (tool / target / inference) | Live | Ours (where) | Status |
|---|---|---|---|
| Line end / a line's end, an arc's end, an arc's / circle's centre, a point, the origin (straight, diagonal) | 7 / 9–10 px (6–8 by target); COINCIDENT, not CONCENTRIC, no alignment | the square (`infer`'s `in_point`; was an 8.5 px circle: 8 px snapped, 10 diagonally didn't) | = (fixed) |
| Line end / a line's midpoint, an arc's midpoint | 9 px along (0.45, 0.89), 6 px straight; MIDPOINT | the square | = |
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
| Curves over lines (a line zone and a curve: the crossing if near, else the curve) | catalogue "priority" | `is_direction_line` + `direction_onto_curve` | = |
| A circular model edge in the plane / its centre (lit or not) | CONCENTRIC, a point zone below the coincident points | the same (`centre_snap`, `ExternalSnap::Centre`, `AutoConstraint::ConcentricTo`) | = (new) |
| Another sketch's line end / its line | COINCIDENT (used), 8 / 7 px | the same (`model_snap_of` takes `SketchEntity`) | = (new) |
| Rectangle, second corner | the full set but no H / V through its first corner; midpoints kept | the same | = |
| 3 point arc, third point | coincident points (and its chord's midpoint) only | the same | = |
| Drags: a free end onto a point (straight, diagonally), a midpoint, a circle's rim; no H / V | 7–9 / 6, not 8 / ≤ 6; COINCIDENT / MIDPOINT on release | the squares and bands (`point_drop_target`; points were an 8.5 px circle) | = (fixed) |
| Not yet in the matrix | crossing lines' midpoint; spline through-points; a woken model edge's perpendicular; the other tools live2 is still filling | – | – |

## Fix queue (biggest feel impact first)
1. ~~Sketch click vs drag 5 px; inferences (parallel, perpendicular, H 7 / V 6, alignment after a rest); arc colour;
   Ctrl+click~~ (psui 11-13).
2. Sketch hover timing: live2's 270–300 ms after the pointer stops (no tool) vs 30 ms with the Dimension tool: asked
   whether it is Onshape's or the recording Chrome's slow redraws before copying a delay.
3. ~~3D reach, priority, press thresholds~~ (psui 11-13). Left: mate connector reach, box select's rules, double-click
   on a face, the held edge against its own vertex.
4. Inference glyph position against the pointer (live ≈ 30, 30).
5. Dimension tool placement rules (H / V / aligned by the bounding box, angle quadrant, Ø): a test each.
