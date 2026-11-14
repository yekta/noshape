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
| Mate connector | its triad and ring: 23 px across, 14 down | drawn at Onshape's MATE_CONNECTOR_DIAMETER_PX 36 (`MC_R_OUT_PX` 18) with the 5 px pick: 23 across, ~15 down in the iso view | – | ≈ |
| Vertex vs its edge; edge vs edge 5.6 px apart | the vertex anywhere in its reach (a lit edge holds against it to ~4 px); the lit edge holds 3–4 px past the other | the vertex in its reach; the held band wins by distance | – | ≈ |
| Hover timing | a pick on every move below 0.15 px/ms; faster: after a pause of min(5 × the average pick, 250 ms) | the same (`prehighlight_delay`) | app-core `prehighlight_waits_for_a_pause_as_live` | = |
| Edge midpoint target after a rest | live 1954: 9 px square, a short dwell | `update_hover`, `MIDPOINT_DWELL` | existing midpoint tests | = |
| Click | adds; on a selected entity removes; on empty space clears; Shift+click on empty space keeps; Shift+click on an entity as a plain one | the same (`on_viewport_event`'s Click) | – | = |
| Ctrl+click | the context menu of what is under the pointer, selects nothing (every OS; on a Mac Cmd+click is a plain click) | the same | – | = |
| Double-click | a face: shows its feature's sketch, the two clicks select then deselect; an edge: nothing | the sketch wasn't shown (its row lit only); now shown until the selection changes (`with_dblclick_sketch`; the double click on the viewport's clock) | part-studio-ui `a_face_double_clicked_shows_its_features_sketch` | = (fixed) |
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
| Inference glyph | ≈ 30 px right, 30 below the pointer (28, 28 mid-drag); same frame as the preview (35–60 ms) | its 20 px box centred 29 right, 31 below the snapped point | – | ≈ (within live2's ±2 px) |
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
| Dimension tool | one line: aligned; two points: H / V / aligned by the pointer against their bounding box; two lines: the angle of the pointer's quadrant; a circle: Ø; the text centred on the pointer; the value box 100 × 26, ~33 px left of and 5 above the click | the hovered entity within reach | scene `two_points_dimension_as_live_by_where_the_pointer_is` (live2's 48 pointer positions) | = |

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
| Versions rows | 50 px; hover #def1fb; selected #b2ddf6; single click opens; graph column clicks nothing (`versionsPanel`) | the same (`graph_column`) | ui-parity ver-* | = |
| FS editor line keys | Ctrl+/, Ctrl+D, Ctrl+Shift+D, Alt+↑/↓, Alt+Shift+↑/↓ (Mac: Cmd, Cmd+Option) (fs0.json) | the same (`fs_lines`) | `ace_line_keys_comment_move_duplicate_and_remove` and unit tests | = |
| FS editor navigation keys | Ctrl+K / Ctrl+Shift+K (Mac Cmd+G), Ctrl+\\, Alt+E / Alt+Shift+E (Mac F4) | the same | `find_next_and_previous_wrap`, `ctrl_backslash_jumps_to_the_matching_bracket`, the keystroke test | = |
| FS autocomplete | keywords and snippets at the top level, std inside a function; opens 250–500 ms after a letter | the same lists; opens on the keystroke | fs_complete tests | timing asked (REQUESTS 12:58) |

## Assembly (studios, from 2026-11-13)

Live: live/feel/mates.json, asm-views.json, fields.json `assemblyInstanceList`, 3d.json `assembly` (live and live2,
11-13). The UI is studios'; asm keeps the mate solver and landed the ring snap, the ▶ preview, Insert's Esc / ✓ and the
drag thresholds (their commits below). Menus leave out "Add comment" (comments are cloud). Counts are matched / total
per matrix (17:18 UTC): **mates 26 / 28, views 14 / 15, instance list 21 / 22, 3D 9 / 10**.

### Mate dialog, mate list, Insert (mates.json): 26 / 28

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
| Insert preview's hidden edges | outlined orange through other parts; nothing lit under the pointer | the same (`xray_tops`, hidden-only #ffc04c edges); its own back edges also show faintly through itself (the hidden-only pass can't tell self-occlusion) | – | ≠ (minor) |
| Insert Esc, pending | drops only the instance on the pointer, keeps the dialog | the same (asm e9941770) | asm-insert-esc-pending (live take asked) | = |
| Insert Esc, none pending | closes and removes the session's instances | the same (`cancel_insert`) | – | = |
| Insert ✓ | keeps all, the pending one at identity; one undo step | the same (asm e9941770) | asm-insert-ok-pending (live take asked) | = |
| Ring snap | 5° / 1° / 0.1° by the arc at the pointer's radius ≤ 50 px, within 0.4 × (1 + level/3) of a step | the same (asm 641ab465, `ring_snap`) | asm's tests | = |
| Snap mode ghost | the source part at 0.3 opacity | the same (17:18) | – | = |
| Snap mode lines | mates.json lists the client's INFERENCE_SNAP_* constants (rgb(255,194,14) dashed 4-4, 8 px box) | none | – | ? (client-wide constants, the sketch's qsnap; live's snap-mode capture, behaviours 1211, shows no lines) |

### Exploded, section and named views (asm-views.json): 14 / 15

| Item | Live | Ours (where) | Test | Status |
|---|---|---|---|---|
| Exploded views panel | 3rd of the strip; empty: Add exploded view, illustration, how-to, help link | the same (Simulation left out: cloud) | ui-parity asm-explode-panel, asm-explode-add-view | = |
| Add exploded view | "Explode 1" in edit mode: yellow "Editing Explode 1 / Done", toolbar only undo/redo, the blue how-to, the rollback bar | the same | – | = |
| Selecting an instance | orange outline through other parts, a triad (3 arrows, 3 rings, centre) | the triad; the outline's see-through not compared | – | ? |
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

### Instance list (fields.json): 21 / 22

| Item | Live | Ours (where) | Test | Status |
|---|---|---|---|---|
| Hovered row | light blue, an eye at the right end | the same | – | = |
| Row tooltip | ~70 ms (a 30 fps recording) | 160 ms, as the feature list's rows (`ROW_TIP_DELAY`: live featurelist.json's 160–390 ms) | – | ? (two live numbers) |
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

### 3D in an assembly (3d.json `assembly`): 9 / 10

| Item | Live | Ours (where) | Test | Status |
|---|---|---|---|---|
| Explicit connector's reach | 23 px across, 14 px up and down | the disc picks, in its plane (18 px + the pick square's 5) | `a_picked_glyphs_disc_takes_the_pointer_across_it` | = |
| Explicit connector's priority | over vertex, edge, face | the same (overlay first) | psui's 3D matrix | = |
| Explicit connector's look | a 36 px disc, 4 px ring, Z line 20 px, X/Y 12 px | the Part Studio's disc (`ConnectorGlyphs::picked`, screen size) | `connector_glyph_extent_counts_for_the_zoom_limits` | = |
| Implicit, over a face | the centroid wherever the pointer is; corner dots larger than midpoint dots | the same (17:18; Shift still picks among the points) | `a_faces_connector_is_its_centroid_wherever_the_pointer_is` | = |
| Implicit, over an edge | the midpoint, Z along the edge | the same (17:18) | – | = |
| Vertex / edge over the face | takes over within 6 px, not at 8 | the pick square (psui's 3D matrix: 15 / 15) | psui `the_3d_pick_matrix_matches_live` | = |
| Free drag | moves in the screen plane by the full pointer delta | the same | – | = |
| Drag start | > 2 px and > 50 ms | the same (asm ccbc333f) | asm's tests | = |
| Short press | a click: the face selected, the triad there; its centre ball moves the triad | the same | – | = |
| Constrained drag | a Revolute part turns about its axis, axial position kept | through the solver, grabbed nearest the part's box centre | – | ? |

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

## Feature list and Parts list (matrix)

One row per behaviour live2 measured (live/feel/featurelist.json, 11-13 06:00–06:48), acted out with real clicks,
Shift / Ctrl / Alt, double clicks and drags at the rows of a fresh Part Studio (three parts, Plane 1 between), at a
device pixel ratio of 1 and 2: part-studio-ui `list::the_feature_list_matrix_matches_live` (T1, ~4 s, no GPU; a
canvas click is the viewport's empty pick). Like the snaps matrix it prints matched / total and fails when a row's
status moves. `LIST_ROW=<part of an id>` runs some rows.

**Matched: 57 / 57** (11-13 15:51 UTC; 20 / 42 at the first run; 3 folder rows added 15:51: 1 / 3 matched, 2 fixed). ui-parity: the 78 list and tooltip scenarios 75 / 78 before the last fixes, the 3 left on green too or fixed since (feature-row-drag-above).

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
| Hover: the row tooltip | "Extrude 1 <- Sketch 1"; fades in from 160 ms, full by 390 | the text as before; shown at 160 ms with a 230 ms fade (was GPUI's 500 ms, no fade: `ui::ROW_TIP_DELAY`) | = (fixed) |
| Hover: the geometry | lit 100 ms after the pointer arrives, cleared 100 ms after it leaves; a sweep lights only where it rests | the same (`LIST_HOVER_DEBOUNCE`; was at once) | = (fixed) |
| Folders: New folder | nothing selected: an empty folder above the rollback bar; features and a part: the features; only default geometry: nothing; Escape: none | the same (default geometry alone opened the popover) | = (fixed) |
| Folders: dropping on a collapsed folder | top third before it, middle inside at its end, bottom third after it | the same (every drop went inside, first) | = (fixed) |
| Folders: dropping on an expanded folder | top half before it, bottom half inside as its first child | the same | = |
| Folders: a drop between its children | inside the folder | the moved feature kept its own folder and the folder lost the children before it; now the folder at the drop (`behaviors::drop_folder`) | = (fixed) |
| A drag off the list | the drop line stays at its last spot; released over the canvas: nothing | the line went with the pointer's row (drawn by `drag_over`); now from the drop slot | = (fixed) |
| Folders: sideways | every 19 px sideways in the slot steps a level: a last child's bottom half 19 px left drops after the folder, outside | no levels; now 19 px a level from where the pointer came onto the slot (`LIST_LEVEL_PX`) | = (fixed) |
| Multi-part Properties… | one "2 selected" dialog, both parts listed, Name "Multiple values", shared fields editable (live 08:18) | the first part's Properties | ≠ (not a row yet) |
| Not yet in the matrix | the 6 px drag threshold (Chrome's, not Onshape's); 4 levels of nesting at most; an empty new folder's caret (live2's new-folder-empty, queued); the insertion line's look | – | – |

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
| Not yet in the matrix | a click near an edge's middle takes its midpoint (the 9 px window and the dwell: `midpoint_target_tests`) | – | – |

## Navigation (matrix)

live2's navigation.json (Linux Chrome, the account's default mouse mapping): wheel, right / middle / Ctrl+right /
Alt+right drags, view keys, animations, double middle click, acted out with real events in a Part Studio (a
40 × 30 × 10 block, orthographic) at a device pixel ratio of 1 and 2. part-studio-ui
`nav::the_navigation_matrix_matches_live` (T1, no GPU; the viewport's press and animation clock is the test's,
`Viewport::set_clock`); `NAV_ROW=<part of an id>` runs some rows.

**Matched: 20 / 20** (11-13 14:14 UTC; 9 / 14 at the first run).

| Row | Live | Ours | Status |
|---|---|---|---|
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
| Not yet in the matrix | inertia (a flick keeps turning, ×0.77 per 16.67 ms; none held still 50 ms; none below 20 fps: it reads real time and the frame rate, so not a T1 row; navigation `inertia_decays_like_onshape` checks the decay); Alt+right on a tilted view animates upright first; the view cube (150 px, 15 px margins, face / edge / corner 500 ms, arrows 15° in 250 ms, curved arrows roll, hover fill) and its menu | – | – |

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

**Chrome: 21 / 21** (11-13 15:51 UTC, 18 / 18 at 14:01; 14:01 UTC, was 17 / 17 at 13:33 UTC; 7 / 11 at the first run, then 5 rows added: 1 / 5 matched).

| Row | Live | Ours | Status |
|---|---|---|---|
| Tooltip delay | 400 ms (fully shown 470–545 ms with the fade) | 500 ms; now 400 (vendored gpui `DEFAULT_TOOLTIP_SHOW_DELAY`) | = (fixed) |
| Warm switch | while one shows, or within ~200 ms of it going, the next button's shows at once | always the full wait; now at once (gpui `tooltip_warm`, executor clock) | = (fixed) |
| Cold again | after a pause, the 400 ms wait again | the same | = |
| Longer help | a toolbar tooltip's description 2000 ms after it shows | 1500; now 2000 (`ui::TOOLTIP_EXPAND_DELAY`; a menu row's side tooltip keeps 1500: live fs-ps-ws-feature-tooltip) | = (fixed) |
| Feature dialog place | graphics area's top left, 3 px down | the same | = |
| Feature dialog drag | clamped inside the graphics area; within ~10 px of the top / right / bottom it snaps (3 / 0 / 2 px); the left doesn't | could leave the area by all but 80 × 40 px, no snap; now live's (`behaviors::place_feature_dialog`) | = (fixed) |
| Select other place | pointer + (100, 51) | the same | = |
| A tooltip on a click or the wheel | closes | the same | = |
| A tooltip on a drag's start | closes | the same | = |
| Tooltip fade-in | ~150 ms | none on toolbar tooltips (only list rows' 230 ms); now 150 ms (`ui::TOOLTIP_FADE_IN`); its fade-out isn't drawn (gpui drops a tooltip at once) | = (fixed) |
| Feature dialog place, kept | every feature dialog, in every tab and session, opens where the last one was left | per tab, lost on closing; now one place (`behaviors::feature_dialog_pos`, config feature_dialog_place.json, written when a move ends) | = (fixed) |
| A submenu (▸ row) | opens on hover with no delay (one 10 ms poll); closes ~30 ms after the pointer moves to another row | opens in the frame the row is hovered, closes in the next | = |
| Select other size | 220 px, 24 px rows in a 90 px list; nothing found: 83 px tall | the same, but the empty dialog kept the 92 px list (149 px); now one row (`SELECT_OTHER_EMPTY_H`) | = (fixed) |
| Context menu place | pointer + (0, 2) in the graphics; + (10, 12) on a feature-list row | (0, 2) everywhere; now (10, 12) on feature and folder rows (`os_menu::MENU_AT_ROW`) | = (fixed) |
| Context menu flip | at the right edge its right side 1 px left of the pointer; at the bottom its bottom 4 px below it | flipped round where it opened (pointer + (0, 2)); now round the pointer (vendored gpui `anchored().flip_point`) | = (fixed) |
| Feature list width | remembered per tab, the default 205 elsewhere | one width for every tab; now per tab (`ui::left_panel_w(tab)`, config left_panel_widths.json) | = (fixed) |
| Toolbar dropdowns | on click only, under the toolbar at the tool group's left edge, staying open | under the caret (31 px right of the group's left); now at the group's left (vendored gpui-component `PopupMenu::shift_x`); 4 px under the buttons as live | = (fixed; checked on a runner screenshot and by a row click, no T1 test) |
| Not yet in the matrix | the tooltip's ~100 ms fade-out | – | – |

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
| Not compared | filtered-out entities (no selection filter in the Part Studio for live's account); a selected feature's related part (part-studio/110 compared when the feature list was done) | – | – | – |

Seen on the way: live's start "block" sketch shows all four rectangle lines blue (the rectangle not tied to the
origin); ours draws the two lines from the origin black, as live colours such a line when its corner is at the origin
(qsnap). So the two runners' starts likely differ (whether the rectangle's corner is tied to the origin), not the
colouring: to check.

## Sheet metal (matrix)

live2's sheetmetal.json (11-13 08:10–08:37, a 60 × 40 sheet metal model with two flanges and a tab). part-studio-ui
`sm::the_sheet_metal_matrix_matches_live` (T1, no GPU) on the sm-box start, the Sheet metal table and flat view
panel open, a fresh document per row; cells carry `sm-cell-<table>-<row>-<column>` debug selectors.

**Matched: 11 / 11** (11-13 14:14 UTC; 5 / 7 at the first run). ui-parity: the 11 sm-* scenarios 10 / 11
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
| The flat view | its own camera: the wheel zooms it alone | the same | = |
| The flat view's start | looking straight down (Top) | the same | = |
| A click inside a bend's zone in the flat view | selects that bend in both views and its Bends row | only the centre line (5 px) took a click, else the whole flat face; now the bend's face, line and row (`FlatViewEvent::BendZone`, as Select other's "Face of …") | = (fixed) |
| Not yet in the matrix | the panel's open time (0.9–1.4 s live on a software GPU), the flat view re-fitting, its bend lines (rgb(47,47,47), dashed centre), a flat bend hovered lighting both views and selecting its row, the Make joint / Corner / Bend relief picks, Tab filling its fields from the sketch | – | – |

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
| Extend, hover | the line and its end lit orange, nothing past it; the clicks: the curve, then the boundary (behaviours "psui (2992)"; live2's light blue was the sketch's X axis under its line) | the same (an extension preview added 11-13 from live2's note, taken out 11-14) | = |
| Text | two clicks (corners), then its dialog with "Default text" | the same | = |
| Search tools in a sketch | the sketch tools (Offset o, Linear / Circular pattern, Mirror, Text, Insert image, Trim, Extend) | the same | = |
| Sketch fillet, a click on a corner | a preview, nothing made; Esc cancels | the same | = |
| Sketch fillet, a drag from the corner | made on release (arc, tangents, radius; its radius box opens) | only previewed (a second click made it); now made on release (`SketchEditor::corner_pressed`) | = (fixed) |
| Not yet in the matrix | Search tools' panel (Recently used, the info box, Highlight tool in toolbar), the pattern labels (3x / 1x, spacing, inline boxes), Text's dash-dot rectangle and dialog layout (font list, "Default text" selected, n/250 characters: ours has them), Insert image's tabs and aspect-kept rectangle | – | – |

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
| Not yet in the matrix | the toolbar's Undo tooltip (always "Undo (cmd+z/ctrl+z)": ours the same), undo in a feature dialog (an existing test), the paste's 2D triad and kept constraints, the assembly rows (asm's) | – | – |

## Dialogs (matrix)

live2's dialogs.json (11-13 05:40–07:25; rules read from the client bundle). part-studio-ui
`dialogs::the_dialogs_matrix_matches_live` (T1, no GPU) on the block start, and `manipulators::snap_tests`.

**Matched: 18 / 18** (11-14 00:29 UTC; 1 / 5 at the first run, then 11 rows added: 8 matching, 3 fixed).

| Row | Live | Ours | Status |
|---|---|---|---|
| Which features have a manipulator | Extrude one arrow; Chamfer, Shell none | the same | = |
| An arrow's grab reach | within 14 px across (16 no), from 4 px behind its base to its tip, nothing past the tip | 8 px of the shaft or 14 px round the tip (past it too); now live's (`Viewport::manipulator_index_at`) | = (fixed) |
| The arrow | shaft 30 + head 30 px, head radius 10 | shaft 29; now 30 | = (fixed) |
| A dragged length's step | 10^(round(log10(100 px in the unit)) − 2) of the unit at the arrow (0.1 mm at 0.25 mm/px, 1 mm from 0.316); Shift: free | always 0.1 mm / 0.01 in, Shift ignored; now live's (`manipulators::linear_step`, our own and std arrows) | = (fixed) |
| A dragged angle's step | 5° within 2° up to a 573 px radius, 1° up to 2865 px, then 0.1°; free between; Shift: free | always 5° within 2°; now by radius (the finer windows divided by 1 + level / 3: asked, REQUESTS 12:48) | = (fixed) |
| Properties on opening | no field has the keyboard (live's recordings: a div / ul); typing does nothing | Name focused; now the modal itself (`MetaForm::focus`), document keys still off behind it | = (fixed) |
| A selected entity clicked again | leaves the field (in, out, in) | the same | = |
| Esc in a value field | the first discards the typed value and leaves the field, the dialog stays; a second cancels | the same | = |
| Enter with the keyboard outside a field | commits the dialog | the same | = |
| Typing in a value field | no preview while typing (none 3 s on); Enter, Tab or leaving the field sends it | the same | = |
| A click on Extrude's arrow | flips the direction (the field keeps 27 mm) and regenerates | only a direction-only arrow flipped; a length arrow's click focused its field; now it flips (`manipulators`, a release with no move) | = (fixed) |
| A field's row hovered | its entity lit yellow-green (#f2ff80) while hovered, cleared on leaving (a consumed edge too) | the same | = |
| A field's row clicked (not its ×) | the field keeps its entities | the same | = |
| Tab from Fillet's Radius | Radius's ± button, then Asymmetric, Partial, Variable, Allow edge overflow, Smooth fillet corners; selection fields and dropdowns skipped | the same | = |
| Properties' Enter and Esc | do nothing (no apply, no close) | the same | = |
| Properties' Tab order | Name, Description, Part number, Revision, Vendor, Project, Product line, Title 1–3, Not revision managed, Exclude from all BOMs | Tab stayed in Description (indented it) and the checkboxes took no Tab; now live's order (Description passes Tab on, the two checkboxes are Tab stops) | = (fixed) |
| Appearance on opening | 220 × 400 at the graphics area's bottom left; hex focused and selected, uppercase without '#'; A "1.00" | the same | = |
| Material on opening | 280 × 368 at the graphics area's bottom; nothing picked ('- None -') | the same | = |
| Material's search and Esc | filters as you type ("steel": 38 rows; live's read counted its highlighted first row twice), the first highlighted; Esc closes the list only, a second Esc the dialog | the second Esc did nothing (the keyboard was left on the closed list's search); now it cancels | = (fixed) |
| Not yet in the matrix | the drag's 2–3 px dead zone, no snap to geometry, no value box while dragging, preview timing while dragging (every ~0.4 s);  the Appearance palette's swatch grid and the Material picker's rows (look); Appearance's keys are in `sketch_ui_tests` (escape / Tab / Enter) | – | – |

## Fields (matrix)

live2's fields.json "numericFields" (11-13 10:07, Extrude 1's Depth and Draft angle). part-studio-ui
`fields::the_fields_matrix_matches_live` (T1, no GPU) on the block start.

**Matched: 9 / 9** (11-14 00:33 UTC; 6 / 9 at the first run).

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
| Not yet in the matrix | a failed preview drawing the edge in the error colour; the commit's working overlay (live first seen 1119 ms after ✓, polled every 40–150 ms; ours 1200 ms: within a frame or two, not changed without a finer measure); toasts (studios: success rgb(221,246,204), top centre, 5.1 s, no stacking, hover doesn't pause); empty states | – | – |

## Fix queue (biggest feel impact first)
1. ~~Sketch click vs drag 5 px; inferences (parallel, perpendicular, H 7 / V 6, alignment after a rest); arc colour;
   Ctrl+click~~ (psui 11-13).
2. Sketch hover timing: live2's 270–300 ms after the pointer stops (no tool) vs 30 ms with the Dimension tool: asked
   whether it is Onshape's or the recording Chrome's slow redraws before copying a delay.
3. ~~3D reach, priority, press thresholds~~ (psui 11-13). Left: the held edge against its own vertex (mate connector reach ≈, double-click fixed 11-14).
4. ~~Inference glyph position against the pointer (live ≈ 30, 30)~~: within 2 px (psui 11-14).
5. ~~Dimension tool placement rules~~: two points' H / V / aligned by the box, 48 / 48 of live2's positions (psui 11-14); the angle quadrant and Ø have their own tests.
