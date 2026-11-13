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
| Click vs drag threshold | ? | 3 px (`navigation::DRAG_THRESHOLD_PX`) | – | ? |

## Sketch (editing a sketch)

| Item | Live | Ours (where) | Test | Status |
|---|---|---|---|---|
| A line's end snaps onto a line | ≤ 7 px, not 8 (behaviours "sketch snapping and hover", qsnap) | 7.5 px (`INFERENCE_PX`) | inference tests; GPUI `a_line_ends_on_the_hovered_point_at_scale_1/2` | = |
| …onto a point | ≤ 8–9 px | 8.5 px (`inference::POINT_REACH`) | same | = |
| …onto a midpoint | ±5–7 px along the line | 6 px (`MIDPOINT_REACH`) | scene `a_snapped_point_or_midpoint_gets_the_square_and_a_curve_none` | = |
| A dragged point onto a line | ≤ 7 px, not 9 | 7.5 px on screen (`point_drop_target`) | GPUI `a_dragged_point_lands_on_a_line_within_7_px` | = |
| Snap target look | yellow-orange square, 17 px, round a point / midpoint; none on a line | the same (`SNAP_SQUARE_*`) | scene, same | = |
| Inference glyph position | ≈ 30 px right, 27–29 px below the snapped point (qsnap crops) | its 20 px box centred 29 right, 31 below | – | – (pixel check pending) |
| Other inference types (H/V, alignment, tangent, perpendicular, parallel, quadrant): reach and glyph | ? (feel (sketch)) | 7.5 px for each | – | ? |
| Hover of a sketch entity = what a click takes | live reach: a point 10 px, a curve 6 px (SKETCH-SCREEN-SPACE-REACH) | the GPU hover held to it (`sketch_hover_id`) | GPUI `a_circles_rim_lights_and_selects_the_circle_not_its_centre_beyond_reach`; GPU `feel::the_dimension_tool_takes_the_hovered_line` | = |
| Nothing recoloured under the pointer mid-draw | the target line doesn't recolour | the same (`SketchEditor::mid_draw`) | – | = |
| Hovered point look | a 10–11 px translucent orange disc, centred | the same; whole on a slanted plane | offscreen `a_hovered_sketch_point_on_a_slanted_face_draws_its_whole_disc`, `a_sketch_point_is_drawn_as_lives_in_each_state` | = |
| Per-entity colour (a free end point) | the line black, its free end blue; black once dimensioned | the same: a line is under defined only if a free motion moves the infinite line (`line_motion_queries`, `dof::analyze_component_with`); points, arcs, circles by their own unknowns | sketch `a_line_with_a_free_length_is_defined_and_its_free_end_is_not`, `a_line_whose_end_moves_off_it_is_under_defined` | = (arcs / circles: ? live) |
| Sketch selection: click, Shift, box | ? | – | – | ? |
| Dimension tool: what a click takes, placement | ? | the hovered entity within reach | GPU `feel::the_dimension_tool_takes_the_hovered_line` | ? |
| Point and line drags | ? | – | – | ? |
| Click vs drag threshold | ? | 3 px (sketch drags) | – | ? |

## Fix queue (biggest feel impact first)
1. ~~Per-entity colour~~ (psui 11-12: lines by their own geometry). Arcs and circles: ask live2 whether an arc whose
   ends slide along its circle stays black.
2. 3D reach, once live's numbers are in (an edge reaches 9 px over the background but 5 over its face today).
3. Inference glyph position: a pixel check against qsnap (ours 29 / 31 px from the point, live ≈ 30 / 27–29).
