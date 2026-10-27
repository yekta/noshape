# Sketch mode: Onshape behaviour parity checklist

One line per behaviour. Status: ✅ matches Onshape (verified in the app with xdotool on Xvfb and/or a
sketch-crate test), 🔧 partly there / differs, ❌ missing, ⏳ implemented, not yet verified.

Sources: `H:<page>` = Onshape help page `onshape-research/docs-raw/md/Sketch__<page>.md` (other prefixes
spelled out); `B§n` = `research/sections/B-sketch.md` section; `SK-xx` = `research/sections/02-sketch.md`
item; `i18n` = `docs-raw/bundle-i18n-strings.txt`; `LE` = `onshape-research/live-exploration.md`.
Tests: `T:ux` = `crates/sketch/tests/onshape_ux.rs`, `T:inf` = `crates/sketch/src/inference.rs` tests,
`T:sm` = `crates/app/src/sketch_mode.rs` tests.

## 1. Entering, finishing, cancelling

| # | Behaviour | Source | Status | Verified |
|---|---|---|---|---|
| 1.1 | Sketch button / Shift+S opens the Sketch dialog prompting "Select a sketch plane" | B§1.1, i18n | ✅ | 0.4.10+ / 2d9a275c 10-27 |
| 1.2 | A preselected plane or planar face fills the Sketch plane field | B§1.1 | ✅ | 0.4.10+ / 2d9a275c 10-27 |
| 1.3 | Context menu on a plane/face: **New sketch** | H:sketch_basics, LE | ✅ | 0.4.10+ / 2d9a275c 10-27 |
| 1.4 | Double-click a sketch in the Feature list (or Edit) re-opens it | B§1.1 | ✅ | 0.4.10+ / 2d9a275c 10-27 |
| 1.5 | View is **not** reoriented on entry by default; `n` views normal to the sketch plane, `n` again flips | View__orient_normal_to_sketch_on_edit, B§1.4 | ✅ | 0.4.10+ / 2d9a275c 10-27 |
| 1.6 | "Orient normal to sketch on edit" option in the camera menu | View__orient_normal_to_sketch_on_edit | ✅ (Part Studio camera menu; the sketch view orients on entry) | 0.4.10+ / 2d9a275c 10-27 |
| 1.7 | A new sketch starts with no tool active once the plane is picked; a click in empty space does nothing; the view doesn't rotate | Live sketch/00, 01, 44, 500–503 (confirmed by live capture) | ✅ (checked in the app 10-24: Top picked from the list, no tool active, empty click does nothing, view stays) | 0.4.10+ / 2d9a275c 10-27 |
| 1.7a | The sketch frame (Onshape `SketchPlaneDisplay.updateBounds`): placed once when first drawn, centred on the plane entity's bounds centre (the ray through the graphics centre if off screen), 0.7 × the graphics height tall (`VIEWPORT_HEIGHT_TO_PLANE_HEIGHT_RATIO`) × 1.77777 wide; fixed in the plane afterwards (zoom / geometry don't change it; a new plane re-places it); `#90cef1` 1 px; name 20 px (`PLANE_TEXT_HEIGHT_PX`) in the plane 5 px inside the top-left corner, shrunk to fit | Bundle webgl `oM`, live sketch/500–503 (2.02 × 1.14 plane at the default zoom) | ✅ (app 10-24: frame corners at (388,557) (727,265) (1776,567) (1437,860) vs live (388,557) (728,263) (1776,567) (1435,861)) |  |
| 1.8 | ✓ / Enter accepts the sketch; "Finish sketch" button | H:sketch_basics, B§1.5 | ✅ | 0.4.10+ / 965d3f56 10-27 (✓ accepts; Enter with no tool keeps the sketch open = live behaviours 214 / 4824) |
| 1.9 | ✗ cancels the whole sketch session; a Restore link undoes the cancel | H:sketch_basics | ✅ (✕ shows "Sketch N has been canceled. **Restore** ×" (GUI :103 m3-toast.png); Restore reopens the draft with its geometry (m3-restored.png); × dismisses) | 0.4.10+ / 2d9a275c 10-27 |
| 1.10 | Esc cancels the active tool / chain, never the sketch | H:sketch_tools ("The Escape key exits a tool selection") | ✅ (also when the focus is in the Sketch dialog) | 0.4.10+ / 2d9a275c 10-27 |
| 1.11 | Extrude (Shift+E) / Revolve (Shift+W) from the sketch toolbar accept the sketch and preselect all regions | H:sketch_tools | ✅ | 0.4.10+ / 203c4d6f 10-27 |
| 1.12 | Ctrl+Z / Ctrl+Y inside the sketch step through sketch edits | LE, B§8.2 | ✅ | 0.4.10+ / 2d9a275c 10-27 |
| 1.13 | Sketch dialog options are remembered between sessions (except Show expressions) | H:sketch_basics | ✅ (Show constraints remembered; Show expressions resets) | 0.4.10+ / 2d9a275c 10-27 |
| 1.14 | Disable imprinting checkbox when sketching on a part face | H:sketch_basics | ✅ (feature param) | 0.4.10+ / 203c4d6f 10-27 |
| 1.15 | Feature list shows a blue indicator on a sketch that is not fully defined | PartStudio__features_and_parts_lists, LE | ✅ | 0.4.10+ / 2d9a275c 10-27 |
| 1.16 | Shift+H hides / shows all sketches | H:sketch_basics | ✅ | 0.4.10+ / 203c4d6f 10-27 |
| 1.17 | Shift+P hides construction geometry | B§1.7 | ✅ (Part Studio: hides planes and sketches construction) | 0.4.10+ / 203c4d6f 10-27 |

## 2. Toolbar

| # | Behaviour | Source | Status | Verified |
|---|---|---|---|---|
| 2.1 | Sketch toolbar replaces the feature toolbar while a sketch is open | H:sketch_tools | ✅ | 0.4.10+ / 2d9a275c 10-27 |
| 2.2 | Order: Undo, Redo, Extrude, Revolve, Line▾, Rectangle▾, Circle▾, Arc▾, Polygon▾, Spline▾, Point, Text, Use▾, Construction, Fillet▾, Trim▾, Offset▾, Mirror, Pattern▾, Dimension, 14 constraints | LE (docs toolbar images) | ✅ (groups and dropdown contents match live dom/toolbar-sketch.txt + dropdowns-sketch.json, incl. the Insert DXF or DWG ▾ / Insert image group after Linear pattern) | 0.4.10+ / 203c4d6f 10-27 |
| 2.3 | Groups: Line/Midpoint line; Corner/Center point/Aligned rectangle; Center point/3 point circle/Ellipse; 3 point/Tangent/Center point/Elliptical arc/Conic; Inscribed/Circumscribed polygon; Spline/Bezier/Spline control point; Use/Intersection; Fillet/Chamfer; Trim/Extend/Split; Offset/Slot; Linear/Circular pattern/Transform | LE | ✅ | 0.4.10+ / 203c4d6f 10-27 |
| 2.4 | A group button shows the last tool used from that group | H:sketch_tools | ✅ | 0.4.10+ / 2d9a275c 10-27 |
| 2.5 | The active tool's button is highlighted | UI | ✅ | 0.4.10+ / 2d9a275c 10-27 |
| 2.6 | Tools stay active until Esc or another tool is chosen | H:sketch_tools | ✅ | 0.4.10+ / 203c4d6f 10-27 |
| 2.7 | Pressing a tool's shortcut (or button) again exits it | B§2.1 | ✅ | 0.4.10+ / 203c4d6f 10-27 |
| 2.8 | Constraint buttons not applicable to the current selection are disabled | B§3.1 | ✅ (T:sm) |  |
| 2.9 | Tooltips name the tool and its shortcut | UI | ✅ | 0.4.10+ / 203c4d6f 10-27 |
| 2.10 | Search tools (Alt+C) finds sketch tools | H:sketch_tools | ✅ (sketch tools listed first in a sketch) | 0.4.10+ / 2d9a275c 10-27 |
| 2.11 | S opens the sketch shortcut toolbar at the cursor | H:sketch_basics | ✅ (sketch tools in the S toolbar) | 0.4.10+ / 2d9a275c 10-27 |

## 3. Keyboard shortcuts (sketch scope)

| # | Behaviour | Source | Status | Verified |
|---|---|---|---|---|
| 3.1 | l Line, g Corner rectangle, r Center point rectangle, c Center point circle, a 3 point arc, Shift+S Point | B§8.1 | ✅ | 0.4.10+ / 203c4d6f 10-27 |
| 3.2 | q Construction, Shift+F Fillet, m Trim, x Extend, o Offset, u Use, d Dimension | B§8.1 | ✅ | 0.4.10+ / 203c4d6f 10-27 |
| 3.3 | i Coincident, Shift+O Concentric, b Parallel, t Tangent, h Horizontal, v Vertical, Shift+L Perpendicular, e Equal, Shift+M Midpoint, Shift+K Normal, Shift+G Pierce, Shift+Q Symmetric, Shift+J Fix, Shift+U Curvature | B§8.1 | ✅ | 0.4.10+ / 203c4d6f 10-27 |
| 3.4 | Shift+A switches Line ↔ Tangent arc while chaining (keeps the chain) | H:line | ✅ | 0.4.10+ / 203c4d6f 10-27 |
| 3.5 | Shift (hold) suppresses inferences while drawing / dragging | H:automatic_inferencing | ✅ | 0.4.10+ / dd1b5037 10-27 (Line: Shift drops the horizontal inference) |
| 3.6 | Shift (hold) shows all constraint glyphs while moving | H:working_with_constraints | ✅ | 0.4.10+ / dd1b5037 10-27 (Show constraints off: Shift shows the glyphs) |
| 3.7 | Alt (hold) while drawing a rectangle makes a square | H:corner_rectangle | ✅ | 0.4.10+ / 203c4d6f 10-27 |
| 3.8 | Alt+←/→ switches between on-the-fly dimension boxes | H:sketch_basics | ✅ | 0.4.10+ / 203c4d6f 10-27 |
| 3.9 | Delete / Backspace deletes selected entities, constraints and dimensions | H:dimension, H:working_with_constraints | ✅ | 0.4.10+ / 2d9a275c 10-27 |
| 3.10 | Space clears the sketch selection | B§8.2 | ✅ | 0.4.10+ / 203c4d6f 10-27 |
| 3.11 | Ctrl+C / Ctrl+V copy / paste sketch entities | H:sketch_basics | ✅ (T:ux) | 0.4.10+ / 203c4d6f 10-27 |
| 3.12 | `#` typed without `=` in a sketch value box is the view shortcut; `=#var` enters a variable | H:sketch_basics | ✅ |  |
| 3.13 | Shortcuts are user-customizable | B§2.1 | ✅ (Shift+/ › Customize keyboard shortcuts opens the Preferences window scrolled to Keyboard shortcuts, like live's /user/settings?focusView=customize-keyboard-shortcuts tab (shortcuts/30–47): tabs, Edit / Reset per row, legend, Reset all, Shortcut toolbars; GUI :103 k1–k2) |  |

## 4. Geometry tools: clicks, drags and results

| # | Behaviour | Source | Status | Verified |
|---|---|---|---|---|
| 4.1 | Line: click-click-click chains connected segments | H:line | ✅ | 0.4.10+ / 2d9a275c 10-27 |
| 4.2 | Line: press-drag-release makes a single segment | H:line | ✅ | 0.4.10+ / 2d9a275c 10-27 |
| 4.3 | Line: clicking an existing point closes / ends the chain | B§2.4.1 | ✅ | 0.4.10+ / 2d9a275c 10-27 |
| 4.4 | Line: moving back onto the chain end switches to a tangent arc; after the arc the tool reverts to Line | H:line | ✅ (T:sm) | 0.4.10+ / 2d9a275c 10-27 |
| 4.5 | Midpoint line: click the midpoint, then an end; midpoint constraint | H:midpoint_line | ✅ (live spec 10-27: a normal point at the midpoint, the line from the clicked end; skasm 10-27) | 0.4.10+ / 2d9a275c 10-27 |
| 4.6 | Corner rectangle: click corner, click diagonal corner, or drag | H:corner_rectangle | ✅ (live spec 10-27: Horizontal on the edge at the 2nd click's y, Perpendicular with the edge at the 1st click's x, 2 Parallel; sketch test `corner_rectangle_constraints_follow_the_click_order`) | 0.4.10+ / 2d9a275c 10-27 |
| 4.7 | Center point rectangle: click center, click corner, or drag; a construction point at the center, Midpoint of both diagonals (live spec 10-27: no diagonal lines, the Help's "construction diagonal" isn't stored) | H:center_point_rectangle, live spec-tools | ✅ (skasm 10-27: `MidpointOf` point-to-point midpoints; scene test `center_rectangle_and_three_point_circle_as_live`) | 0.4.10+ / 2d9a275c 10-27 |
| 4.8 | Aligned rectangle: two clicks for the first side, third click for the width | H:aligned_rectangle | ✅ (T:sm) | 0.4.10+ / 2d9a275c 10-27 |
| 4.9 | Center point circle: click center, click rim, or drag | H:center_point_circle | ✅ | 0.4.10+ / 2d9a275c 10-27 |
| 4.10 | 3 point circle: three clicks, or drag 1→2 then click 3; a construction point Coincident with the circle at each click (live spec 10-27) | H:3_point_circle | ✅ (T:sm; points skasm 10-27) | 0.4.10+ / 2d9a275c 10-27 |
| 4.11 | 3 point arc: start, end, then the bulge point; drag start→end then click | H:3_point_arc | ✅ | 0.4.10+ / 2d9a275c 10-27 |
| 4.12 | 3 point arc: snaps to an exact semicircle when the center reaches the chord | H:3_point_arc | ✅ | 0.4.10+ / 2d9a275c 10-27 |
| 4.13 | Tangent arc tool: start at a curve end, click the end; tangent constraint | H:arc_tangent | ✅ (T:sm) | 0.4.10+ / 2d9a275c 10-27 |
| 4.14 | Center point arc: center, start (radius), end; the sweep follows the cursor's direction | H:center_point_arc | ✅ (T:sm) | 0.4.10+ / 2d9a275c 10-27 |
| 4.15 | Ellipse: center, primary axis, secondary radius; a horizontal / vertical major-axis click makes it Horizontal / Vertical (live spec 10-27 "majorAxisSnap0") | H:ellipse | ✅ (T:sm; snap skasm 10-27) | 0.4.10+ / 2d9a275c 10-27 |
| 4.16 | Elliptical arc: center, primary axis, secondary radius + start, end on the construction ellipse | H:elliptical_arc | ✅ (T:sm) | 0.4.10+ / 965d3f56 10-27 (centre, axis, 3rd click = minor radius + start, end) |
| 4.17 | Elliptical arc start/end snap to the axis / quadrant points | H:elliptical_arc | ✅ (axis / quadrant snaps on the construction ellipse) | 0.4.10+ / 2d9a275c 10-27 |
| 4.18 | Conic: start, end, control point; rho value | H:conic | ✅ (T:ux, T:sm; GUI :103 rho field auto-focus and 0.7 Enter verified; /tmp/skb/2026-10-18/conic-rho*.png) | 0.4.10+ / 2d9a275c 10-27 |
| 4.19 | Inscribed / circumscribed polygon: center, circumference; construction circle. Inscribed: the 2nd click is an edge's middle (circle tangent to the edges); circumscribed: a vertex (circle through the vertices) | H:inscribed_polygon; live skasm 10-26 (behaviours "skasm sketch polygons") | ✅ (geometry and constraints as live 10-26 (seed + circular pattern; inscribed Midpoint + Tangent, circumscribed Coincident on the circle; no Ø dimension), test `polygon_second_click_is_an_edge_middle_inscribed_and_a_vertex_circumscribed`; GUI :103 10-27) | 0.4.10+ / 2d9a275c 10-27 |
| 4.20 | Polygon: type the side count before the circumference click (3–50) | H:inscribed_polygon | ✅ | 0.4.10+ / 2d9a275c 10-27 |
| 4.21 | Polygon: after the circumference click the pointer's distance from the centre sets the side count (+1 per 10 px beyond the click, −1 per 10 px in, min 3); Enter or any click commits (no new polygon at the click); Esc discards | Live skasm 10-27 (behaviours "skasm (2026-10-27) sketch polygon side count") | ✅ (T:sm `polygon_sides_typed_and_midpoint_line`, `polygon_spinner_and_next_center_commit_the_previous_polygon`) | 0.4.10+ / 2d9a275c 10-27 |
| 4.22 | Spline: click points, double-click to end | H:spline | ✅ (T:sm) | 0.4.10+ / 2d9a275c 10-27 |
| 4.23 | Spline: closes when the last point lands on the first | H:spline | ✅ (T:sm) | 0.4.10+ / 2d9a275c 10-27 |
| 4.24 | Spline: context menu **Confirm spline** finishes one point behind | H:spline | ✅ (T:sm) | 0.4.10+ / 2d9a275c 10-27 |
| 4.25 | Spline tangent handles (white points) to drag | H:spline | ✅ (T:ux; GUI :103 open spline shows white tangent handles; handles are not deletable) | 0.4.10+ / 2d9a275c 10-27 |
| 4.26 | Bezier: click control points, double-click to end; control polygon shown | H:bezier | ✅ (control polygon shown while drawing and when selected) | 0.4.10+ / 2d9a275c 10-27 |
| 4.27 | Bezier: context menu **Escape bezier** removes the curve in progress | H:bezier | ✅ (T:sm) | 0.4.10+ / 2d9a275c 10-27 |
| 4.28 | Bezier degree label (double-click to edit, max 15) | H:bezier | ✅ (T:ux; GUI :103 double-click opens degree input; 4→6 succeeds, 16 rejected; bezier-degree-six-verified.png) | 0.4.10+ / 965d3f56 10-27 (double-click Deg 3 → 6; curve kept) |
| 4.29 | Spline control point: click a spline / bezier control polygon to add a point | H:spline_control_point | ✅ (T:ux) |  |
| 4.30 | Point: click to place | H:point | ✅ | 0.4.10+ / 2d9a275c 10-27 |
| 4.31 | Text: drag a box, lower-left corner and height set the first line; horizontal constraint on the bottom edge | H:text | ✅ | 0.4.10+ / 2d9a275c 10-27 |
| 4.32 | Text: font, bold, italic, flip; edit later; drag by its box | H:text | ✅ | 0.4.10+ / 2d9a275c 10-27 |
| 4.33 | Text: Convert to expression | H:text | ✅ (GUI :103 Convert to expression evaluates `"A" ~ (2 + 3)` to A5; text-expression.png) |  |
| 4.34 | Slot: select centerline curves (before or after the tool); one shared width dimension | H:slot | ✅ (T:ux) | 0.4.10+ / 2d9a275c 10-27 |
| 4.35 | Slot: a connected chain gives one slot following the chain | H:slot | ✅ (T:ux; GUI :103 selected L-shaped line chain makes one continuous slot with shared width input; chained-slot.png) | 0.4.10+ / 965d3f56 10-27 (L chain selected → one slot) |
| 4.36 | Construction toggle: q with a selection toggles it, without one draws construction geometry | H:construction | ✅ | 0.4.10+ / 2d9a275c 10-27 |
| 4.37 | Construction lines are dashed (dash-dot) in the status color | B§5.2 | ✅ (dashed, status colors) | 0.4.10+ / 2d9a275c 10-27 |
| 4.38 | Insert image / Insert DXF or DWG | H:insert_image, H:insert_dwg_or_dxf | ✅ (DXF ASCII + binary and DWG R13–2018 through cadkit (crates/sketch/src/cad.rs, dwg.rs): block references expanded (placement, scale, rotation, arrays, nesting), mirrored/tilted planes and non-uniform block scales mapped exactly; T:ux dwg.rs fixtures R2000–R2018 + cad.rs tests; GUI :103 m4-dwg.png "Inserted 7 entities from fixture.dwg" incl. the BLOCK1 line. Images: 3D textured quads in any orientation, depth tested, exact colors, visible outside sketch mode (m2-placed / m2-iso / m2-accepted-c.png)) |  |
| 4.39 | Preview geometry while placing is drawn in the sketch preview color | B§2.3 | ✅ | 0.4.10+ / 203c4d6f 10-27 |

## 5. On-the-fly dimensions

| # | Behaviour | Source | Status | Verified |
|---|---|---|---|---|
| 5.1 | Right after creation a box shows the suggested value (line length, circle diameter, arc radius) | H:sketch_basics, H:line | ✅ | 0.4.10+ / 2d9a275c 10-27 |
| 5.2 | Typing a value or expression and Enter adds a driving dimension | H:line | ✅ (OTF-FIRST-KEY fixed 0537ac91) | 0.4.10+ / 6cbb24e0 10-27 |
| 5.3 | Continuing to sketch leaves the geometry undimensioned (box disappears) | H:sketch_basics | ✅ | 0.4.10+ / 2d9a275c 10-27 |
| 5.4 | Rectangles: length box, Enter, width box, Enter | H:corner_rectangle | ✅ | 0.4.10+ / 2d9a275c 10-27 |
| 5.5 | Ellipse / elliptical arc: primary then secondary diameter | H:ellipse | ✅ (T:sm) | 0.4.10+ / 965d3f56 10-27 (readout Ø primary, then Ø secondary) |
| 5.6 | Polygon: no value box and no Ø dimension (live spec 10-26 "Sketch polygon commits": seed + circular pattern, no DIAMETER; the Help's "diameter of the construction circle" is only the readout) | H:inscribed_polygon | ✅ (scene test `polygon_sides_typed_and_midpoint_line`: no box) | 0.4.10+ / 2d9a275c 10-27 |
| 5.7 | Fillet: radius box right after the first fillet | H:sketch_fillet | ✅ | 0.4.10+ / 2d9a275c 10-27 |
| 5.8 | `=#var` uses a variable; expressions with units and functions | H:sketch_basics | ✅ |  |
| 5.9 | Esc closes the box without dimensioning | — | ✅ | 0.4.10+ / 2d9a275c 10-27 |
| 5.10 | Chained lines: the chain continues from the dimensioned end | H:line | ✅ | 0.4.10+ / 2d9a275c 10-27 |

## 6. Inference and snapping

| # | Behaviour | Source | Status | Verified |
|---|---|---|---|---|
| 6.1 | Snap to existing points creates a coincident (shared point) | B§6 | ✅ | 0.4.10+ / 2d9a275c 10-27 |
| 6.2 | Snap to the sketch origin; coincident to the origin | H:automatic_inferencing | ✅ (T:inf) | 0.4.10+ / 2d9a275c 10-27 |
| 6.3 | Horizontal / vertical alignment with the origin and other points: dotted guide, constraint on click | H:automatic_inferencing | ✅ | 0.4.10+ / 2d9a275c 10-27 |
| 6.4 | Midpoint snap on lines and arcs creates a midpoint constraint | B§6 | ✅ | 0.4.10+ / 2d9a275c 10-27 |
| 6.5 | Center snap on circles / arcs; quadrant snap | B§6 | ✅ | 0.4.10+ / 2d9a275c 10-27 |
| 6.6 | Point on curve snap creates a coincident | B§6 | ✅ | 0.4.10+ / 2d9a275c 10-27 |
| 6.7 | New line horizontal / vertical inference | B§6 | ✅ | 0.4.10+ / 2d9a275c 10-27 |
| 6.8 | Parallel / perpendicular to the previous segment | B§6 | ✅ | 0.4.10+ / 203c4d6f 10-27 |
| 6.9 | Wake-up: lines the cursor passed over give parallel / perpendicular inferences | H:automatic_inferencing | ✅ (T:inf) |  |
| 6.10 | Tangent inference from an arc / spline end | B§6 | ✅ (T:inf) |  |
| 6.11 | Inference glyph next to the cursor | H:working_with_constraints | ✅ | 0.4.10+ / 2d9a275c 10-27 |
| 6.12 | Inference line yellow dashed (`INFERENCE_SNAP_LINE_COLOR`) | B§6 | ✅ | 0.4.10+ / 2d9a275c 10-27 |
| 6.13 | Shift suppresses inferences | H:automatic_inferencing | ✅ | 0.4.10+ / 2d9a275c 10-27 |
| 6.14 | Snap to model vertices and edges when sketching on a face; vertices stay linked | H:sketch_basics | ✅ (GUI :103 m10.png: sketch on a part face, the line start snaps to the model vertex with a coincident inference and stays linked (fixed point)) | 0.4.10+ / 965d3f56 10-27 (sketch on wf-block's top face: line from the top face's corner vertex, start black + use glyph) |
| 6.15 | Woken entity / parallel partner highlighted orange, new line drawn dotted when parallel | H:automatic_inferencing | ✅ | 0.4.10+ / 2d9a275c 10-27 |
| 6.16 | Ellipse quadrant (Quadrant constraint) inference | H:working_with_constraints | ✅ (T:ux, Quadrant constraint) |  |

## 7. Dimension tool

| # | Behaviour | Source | Status | Verified |
|---|---|---|---|---|
| 7.1 | d / button activates; stays active for the next dimension | H:dimension | ✅ | 0.4.10+ / 2d9a275c 10-27 |
| 7.2 | Picked entities highlight; the dimension previews at the cursor before placing | H:dimension | ✅ | 0.4.10+ / 2d9a275c 10-27 |
| 7.3 | The placement click places the dimension and opens its value box | H:dimension | ✅ | 0.4.10+ / 2d9a275c 10-27 |
| 7.4 | Single line: length; slanted line dragged up/down or left/right: horizontal / vertical | H:dimension | ✅ (T:sm) | 0.4.10+ / 203c4d6f 10-27 |
| 7.5 | Two points: aligned when dragged at an angle, horizontal / vertical when dragged straight | H:dimension | ✅ | 0.4.10+ / 203c4d6f 10-27 |
| 7.6 | Two parallel lines: distance (implies parallel) | H:dimension | ✅ (T:sm) | 0.4.10+ / 203c4d6f 10-27 |
| 7.7 | Two lines at an angle: angle; the quadrant follows the cursor | H:dimension | ✅ (T:sm) | 0.4.10+ / 203c4d6f 10-27 |
| 7.8 | Circle: diameter; arc: radius | H:dimension | ✅ | 0.4.10+ / 2d9a275c 10-27 |
| 7.9 | Arc length: both end points then the arc | H:dimension | ✅ (T:sm) | 0.4.10+ / 203c4d6f 10-27 |
| 7.10 | Point / line / circle to a construction line, cursor across it: centerline dimension (twice the distance) | H:dimension | ✅ (T:sm, T:ux) |  |
| 7.11 | Ellipse: major / minor diameter | B§4.2 | ✅ (T:sm) | 0.4.10+ / 203c4d6f 10-27 |
| 7.12 | Dimension to the sketch origin | H:dimension | ✅ (T:sm) | 0.4.10+ / 203c4d6f 10-27 |
| 7.13 | Dimension to planes / model edges | H:dimension | ✅ (planes as trace lines, model edges / vertices as linked reference geometry) | 0.4.10+ / 965d3f56 10-27 (point to model edge 24.797 → 20) |
| 7.14 | Over-defining dimensions are made driven: plain grey value ("Ø10", no parentheses) and the value box still opens, no toast | H:dimension; live "qa4 round 5 s5 driven dims" (1) | ✅ (test `driven_labels_and_offset_side_as_live`; GUI :103 skasm 10-26: second Ø on a dimensioned circle opens the box, grey plain "Ø10" after Esc) | 0.4.10+ / 203c4d6f 10-27 (duplicate Ø → driven grey plain; h + v + aligned length all stay driving: asked live 18:26) |
| 7.15 | Driven ↔ driving from the context menu ("Change to driving/driven dimension") | H:dimension | ✅ | 0.4.10+ / 203c4d6f 10-27 |
| 7.16 | Double-click a dimension to edit its value | H:dimension | ✅ | 0.4.10+ / 2d9a275c 10-27 |
| 7.16a | Esc on an untouched value box (new or reopened) closes it at once (a new dimension keeps its measured value); a changed box reverts on the first Esc, closes on the second; a bare expression is stored with the document unit ("2*5" → "(2*5) mm") | live qa4 round 5 v3 s5 (DIM-ESC-UNTOUCHED, DIM-EXPR-UNIT) | ✅ (app; `quantity_text::simple` no longer reads "*5" as a unit; tests `dimension_expression_keeps_the_unit`, `expressions_get_the_unit_around_them`) | 0.4.10+ / 2d9a275c 10-27 |
| 7.17 | Editing moves both dimensioned sides by half (except radius / diameter) | H:dimension | ✅ (T:ux) | 0.4.10+ / 965d3f56 10-27 (free line 16.85 → 30 horizontal: both ends moved, centre kept) |
| 7.18 | Negative values flip direction (distance, angle, line length); not allowed for arc, radius, diameter | H:dimension | ✅ (T:sm) | 0.4.10+ / 203c4d6f 10-27 |
| 7.19 | Drag the dimension label to move it | B§4 | ✅ | 0.4.10+ / 203c4d6f 10-27 |
| 7.20 | Select + Delete removes a dimension; context menu Delete | H:dimension | ✅ | 0.4.10+ / 2d9a275c 10-27 |
| 7.21 | Expression indicator next to values that use expressions; Show expressions shows them (≤70 chars) | H:sketch_basics | ✅ | 0.4.10+ / 203c4d6f 10-27 |
| 7.22 | Errors: "A dimension cannot be created between these items." / "Could not create the dimension." | i18n | ✅ |  |
| 7.23 | Driving dimensions black, driven gray, hover/selected gold, conflicting red | Home__colors, B§4.5 | ✅ | 0.4.10+ / 2d9a275c 10-27 |
| 7.24 | Tolerances, precision, configure dimension | B§4.3 | ✅ (tolerance types, deviations, precision; configure dimension) | 0.4.10+ / dd1b5037 10-27 (± → Tolerance: No tolerance / Symmetrical / Deviation / Limits / Min / Max / Basic; Precision: Workspace precision) |
| 7.25 | Min / max distance to circles | SK-D07 | ✅ (T:sm; rim picks measure min / max) |  |

## 8. Constraints

| # | Behaviour | Source | Status | Verified |
|---|---|---|---|---|
| 8.1 | Select-first: constraint applies immediately to the selection | H:working_with_constraints | ✅ | 0.4.10+ / 2d9a275c 10-27 |
| 8.2 | Multi-selection: Parallel / Equal / Coincident / Tangent chain all entities, Horizontal / Vertical / Fix apply to each | H:parallel, H:equal, H:horizontal | ✅ (T:sm) | 0.4.10+ / 203c4d6f 10-27 |
| 8.3 | Tool-first: each pair picked is constrained; tool stays on until clicked again or another tool | H:coincident | ✅ | 0.4.10+ / 203c4d6f 10-27 |
| 8.4 | Coincident: point-point, point-curve, collinear lines, same circle | H:coincident | ✅ | 0.4.10+ / 965d3f56 10-27 (two line ends + i: both move halfway, coincident glyph) |
| 8.5 | Concentric, Parallel, Tangent, Horizontal, Vertical, Perpendicular, Equal, Fix | constraint pages | ✅ | 0.4.10+ / 203c4d6f 10-27 |
| 8.6 | Midpoint: point + line/arc, or 3 points (start, end, middle) | H:midpoint | ✅ | 0.4.10+ / dd1b5037 10-27 (point + line → Midpoint, point on the middle) |
| 8.7 | Normal: line + curve, in either pick order | H:normal; live qa4 round 5 v3 s6 (3) | ✅ (test `constraint_picks_and_application_as_live`) |  |
| 8.7a | A constraint tool doesn't take a pick it can't use (no message); constraint errors never show internal ids | live qa4 round 5 v3 s6 (CON-MSG) | ✅ (`pick_fits`; failed constraint adds are silent, no ids reach the status bar) |  |
| 8.7b | Vertical on a Horizontal line (anything collapsing a line to a point): "Sketch could not be solved.", the line stays, red | live qa4 round 5 v3 s6 (1) | ✅ (the shape before the solve is kept and the toast raised, `solve_after_constraint`; test `vertical_on_a_horizontal_line_does_not_solve`; GUI :103 skasm 10-26: line stays, red, glyphs red, title red) | 0.4.10+ / 203c4d6f 10-27 |
| 8.7c | Tangent line–circle moves the circle and keeps its radius | live qa4 round 5 v3 s6 (2) | ✅ (only the centre takes the change; test `constraint_picks_and_application_as_live`) | 0.4.10+ / 2d9a275c 10-27 |
| 8.7d | Right-click a dimension or constraint glyph: the empty-space sketch menu, a dimension's Change to driving/driven first, "Delete dimension" / "Delete sketch entity" last | live "qa4 round 5 s5 driven dims" (2), (3) | ✅ (GUI :103 skasm 10-26: dimension menu Change to driven \| Confirm \| Copy sketch \| Show all … \| Delete dimension, no Tolerance item (live has none; dropped 10-27); glyph menu ends Delete sketch entity, glyph yellow + line highlighted) | 0.4.10+ / 203c4d6f 10-27 |
| 8.8 | Pierce: point + off-plane edge | H:pierce | ✅ (GUI :103 m14.png: sketch on Front, point + Pierce (Shift+G) with the cylinder top edge → point moves to the pierce point, fully defined) |  |
| 8.9 | Symmetric: axis line first, then two entities | H:symmetric | ✅ | 0.4.10+ / 203c4d6f 10-27 |
| 8.10 | Curvature: spline/conic + adjacent curve | H:curvature | ✅ |  |
| 8.11 | Constraints to planes / model edges | H:working_with_constraints | ✅ (same reference mechanism as dimensions) |  |
| 8.12 | Glyphs show for the hovered entity (show after hover, hide after 750 ms); all with Show constraints | H:working_with_constraints, B§3.5 | ✅ | 0.4.10+ / 2d9a275c 10-27 |
| 8.13 | Hover a glyph highlights its entities (yellow); click selects it; Delete removes it | H:working_with_constraints | ✅ | 0.4.10+ / 203c4d6f 10-27 |
| 8.14 | Drag glyphs to move them | H:working_with_constraints | ✅ | 0.4.10+ / 203c4d6f 10-27 |
| 8.15 | Conflicting constraints: white glyph on red | H:working_with_constraints | ✅ | 0.4.10+ / 203c4d6f 10-27 |
| 8.16 | External (Use) constraints have a blue glyph background | H:working_with_constraints | ✅ (GUI :103 Equal to projected model circle gives blue glyph backgrounds; external-equal.png) | 0.4.10+ / 965d3f56 10-27 (Use glyphs on #def1fb) |
| 8.17 | Equal: if one curve is dimensioned, its size wins | H:equal | ✅ (solver) | 0.4.10+ / 203c4d6f 10-27 |
| 8.18 | Constraint manager / Profile inspector | H:working_with_constraints | ✅ (both panels from the Sketch diagnostics tools) |  |

## 9. Selection

| # | Behaviour | Source | Status | Verified |
|---|---|---|---|---|
| 9.1 | Click selects; clicking more adds; clicking a selected entity deselects | Onshape | ✅ | 0.4.10+ / 2d9a275c 10-27 |
| 9.2 | Box select left→right: window (entirely inside) | B§5.4 | ✅ | 0.4.10+ / 2d9a275c 10-27 |
| 9.3 | Box select right→left: crossing (anything the box touches) | B§5.4 | ✅ (T:sm) | 0.4.10+ / 203c4d6f 10-27 |
| 9.4 | Hover highlights orange, selection amber, related entities yellow | H:working_with_constraints | ✅ | 0.4.10+ / 2d9a275c 10-27 |
| 9.5 | Not fully constrained points win over fully constrained ones under the cursor | H:sketch_tools | ✅ (T:ux; fresh GUI :103 free point overlapping fixed origin drags away while origin stays fixed; current-point-priority.png) |  |
| 9.6 | Clicking a closed region selects it | B§1.6 | ✅ | 0.4.10+ / 2d9a275c 10-27 |
| 9.7 | Select other (`) | B§5.4 | ✅ (GUI :103 backtick opens candidate popup; select-other-focus.png) | 0.4.10+ / 203c4d6f 10-27 |
| 9.8 | Click in empty space clears the selection | Onshape | ✅ | 0.4.10+ / 2d9a275c 10-27 |

## 10. Dragging and solving

| # | Behaviour | Source | Status | Verified |
|---|---|---|---|---|
| 10.1 | With no tool, under-defined points and curves drag with the solver | H:working_with_constraints | ✅ | 0.4.10+ / 2d9a275c 10-27 |
| 10.2 | Fully defined / fixed geometry does not move | Home__colors | ✅ | 0.4.10+ / 203c4d6f 10-27 |
| 10.3 | Dropping a point on another point merges them; on a curve makes it coincident | B§7.1 | ✅ | 0.4.10+ / 203c4d6f 10-27 |
| 10.4 | Dropping a point on the origin constrains it to the origin | B§7.1 | ✅ (T:sm) | 0.4.10+ / 203c4d6f 10-27 |
| 10.5 | Shift while dragging suppresses the snap | B§7.1 | ✅ (T:sm) | 0.4.10+ / 203c4d6f 10-27 |
| 10.6 | Colors: blue under-defined, black fully defined, red over-defined, per entity | Home__colors | ✅ (theme palette: #0000d0 / black / #cd0000) | 0.4.10+ / 2d9a275c 10-27 |
| 10.7 | Text boxes drag as a whole | H:text | ✅ |  |

## 11. Edit tools

| # | Behaviour | Source | Status | Verified |
|---|---|---|---|---|
| 11.1 | Fillet: click a vertex, radius box; later vertices make equal fillets | H:sketch_fillet | ✅ | 0.4.10+ / 2d9a275c 10-27 |
| 11.2 | Fillet: click two curves | H:sketch_fillet | ✅ (T:sm) | 0.4.10+ / 203c4d6f 10-27 |
| 11.3 | Fillet keeps a virtual sharp with the coincident constraints | H:sketch_fillet | ✅ |  |
| 11.4 | Fillet drag manipulator; too-large preview in red | H:sketch_fillet | ❌ (SKETCH-FILLET-STUCK-RADIUS) (T:sm; red preview, radius manipulator) | 0.4.10+ / 965d3f56 10-27 |
| 11.5 | Chamfer: vertex or two lines, two distance dimensions (45° default) | H:sketch_chamfer | ✅ (T:ux) | 0.4.10+ / 203c4d6f 10-27 |
| 11.6 | Trim: click removes up to the nearest intersections, whole curve when unbounded | H:trim | ✅ | 0.4.10+ / 2d9a275c 10-27 |
| 11.7 | Trim: drag across curves trims everything crossed | H:trim | ✅ (T:sm) | 0.4.10+ / 203c4d6f 10-27 |
| 11.8 | Trim: click a standalone point deletes it | H:trim | ✅ (T:sm) | 0.4.10+ / 203c4d6f 10-27 |
| 11.9 | Extend: click a line or arc to extend to the next boundary | H:extend | ✅ | 0.4.10+ / 203c4d6f 10-27 |
| 11.10 | Extend: "Unable to extend the selected geometry." | i18n | ✅ (GUI :103 extending an unbounded circle reports exact message; extend-error.png; T:sm added) | 0.4.10+ / 203c4d6f 10-27 |
| 11.11 | Split: click a curve where to split | H:sketch_split | ✅ | 0.4.10+ / 203c4d6f 10-27 |
| 11.12 | Offset: select curves / chain, distance and direction, Enter; then click more curves. Live "psb (10-24 j)": the copies get their own points joined by Coincidents, sharp outer corners, own centres; one OFFSET per further pair (all on the first picked pair) and one DISTANCE on that pair (a lone curve: one OFFSET), no Parallel / Concentric; free ends unconstrained; editing the distance moves every copy; a new offset starts away from the region (a circle outward), the flip isn't remembered | H:offset; live psb (10-24 j), qa4 round 5 s1 follow-up | ✅ (GUI :103 skasm 10-26: one edge offsets away from the region with the arrow, Enter creates it and opens the "5 mm" box with ± ✓ ✕; `Sketch::offset_tool`, `ConstraintKind::OffsetCurve`; tests `offset_tool_makes_live_constraints_on_a_chain`, `offset_tool_on_the_closed_loop_and_a_circle`) | 0.4.10+ / 2d9a275c 10-27 |
| 11.12a | Offset takes one chain: a curve not connected to the pending one is only selected (again: deselected) with "Can only offset a single chain at once."; the preview stays | live qa4 round 5 s1 follow-up (5) (OFF-TOAST) | ✅ (test `offset_takes_one_chain`) |  |
| 11.13 | Mirror with preselection: click the mirror line, resolves at once and the tool ends | H:sketch_mirror | ✅ (T:sm) | 0.4.10+ / 965d3f56 10-27 (ellipse + mirror line preselected: resolves at once) |
| 11.14 | Mirror tool-first: "Select a mirror line.", then "Select entities to be mirrored.", Esc exits | H:sketch_mirror, i18n | ✅ (T:sm) | 0.4.10+ / 203c4d6f 10-27 |
| 11.15 | Linear / circular pattern with on-canvas manipulators (count, spacing, direction, angle, center) | H:sketch_linear_pattern | ✅ (T:sm; click in space / Enter accept, Esc cancel; linear instances linked to the seed) | 0.4.10+ / 965d3f56 10-27 (3x / 1x manipulators, count label edit, Enter accepts) |
| 11.16 | Pattern preview limited to 10 instances | H:sketch_linear_pattern | ✅ | 0.4.10+ / 965d3f56 10-27 (count 15: preview 10) |
| 11.17 | Edit pattern from the pattern glyph | H:sketch_linear_pattern | ✅ (T:sm; linear and circular instances are linked to the seed, see 14.1) | 0.4.10+ / 965d3f56 10-27 (right-click glyph › Edit pattern; count 15 → 3) |
| 11.18 | Transform: triad manipulator (move, rotate, scale, free-drag center), Enter commits, Esc cancels | H:sketch_transform | ✅ (click in space / Enter commit, Esc cancel) | 0.4.10+ / 965d3f56 10-27 (paste shows the triad: ring, scale arrow, move square; Enter commits, Esc leaves) |
| 11.19 | Use: click model edges / faces / other sketches to project; linked to the source | H:use | ✅ (planar curves project exactly; non-planar edges project to a fitted spline like Onshape; other sketches; linked); a used circle / arc has a construction centre point, its line ends are not (live "skb (10-25) sketch Use on a face sketch", 4b5ac4fe) | 0.4.10+ / 965d3f56 10-27 (U, block edge → black line + use glyphs) |
| 11.20 | Use: hover preview of the projection | H:use | ✅ (GUI :103 m8.png: hovering the cylinder top edge highlights it and previews the projected circle on the sketch plane; click creates it) | 0.4.10+ / 965d3f56 10-27 (hover preview orange) |
| 11.21 | Use silhouette edges | H:use | ✅ (cylinders, cones, spheres, tori; verified on the demo plate) | 0.4.10+ / dd1b5037 10-27 (Front sketch, U on the pin's cylinder face: two silhouette lines + end edges) |
| 11.22 | Intersection: face crossing the sketch plane | H:intersection | ✅ |  |
| 11.23 | Text cannot be patterned / mirrored in a sketch | H:text | ✅ | 0.4.10+ / 203c4d6f 10-27 |

## 12. Context menus

| # | Behaviour | Source | Status | Verified |
|---|---|---|---|---|
| 12.1 | Sketch entities: Delete sketch entity / Delete N sketch entities | i18n, B§9 | ✅ | 0.4.10+ / 203c4d6f 10-27 |
| 12.2 | Construction, Transform sketch entities, Copy sketch entities | B§9 | ✅ | 0.4.10+ / 203c4d6f 10-27 |
| 12.3 | Paste sketch entities (clipboard holds sketch entities) | B§9 | ✅ | 0.4.10+ / 965d3f56 10-27 (Ctrl+C / Ctrl+V of a line lands centred on the pointer) |
| 12.4 | View normal to sketch plane; Exit tool; Clear selection | B§9 | ✅ | 0.4.10+ / 203c4d6f 10-27 |
| 12.5 | Use (model edge under the cursor) | B§9 | ✅ (GUI :103 right-click unused model edge offers Use; model-context-use.png) | 0.4.10+ / 965d3f56 10-27 (right-click an unused model edge: Use) |
| 12.6 | Spline in progress: Confirm spline; Bezier: Escape bezier | H:spline, H:bezier | ✅ (T:sm) | 0.4.10+ / 2d9a275c 10-27 |
| 12.7 | Dimension: Change to driving / driven dimension, Delete | H:dimension | ✅ | 0.4.10+ / 203c4d6f 10-27 |
| 12.8 | Constraint glyph: Delete | H:working_with_constraints | ✅ | 0.4.10+ / 203c4d6f 10-27 |
| 12.9 | Add comment, Configure dimension | H:dimension | ✅ Configure dimension. Add comment belongs to Onshape comments, a cloud-only feature excluded by TEAM-RULES |  |

## 13. Messages and errors

| # | Behaviour | Source | Status | Verified |
|---|---|---|---|---|
| 13.1 | Tool prompts at the bottom of the view ("Select a mirror line.", …) | i18n | ✅ | 0.4.10+ / 203c4d6f 10-27 |
| 13.2 | "Cannot fillet selected point." | i18n | ❌ (FILLET-POINT-MSG) | 0.4.10+ / 203c4d6f 10-27 |
| 13.3 | "The maximum bezier degree (15) has been reached." | i18n | ✅ (GUI :103 entering degree 16 reports exact message; bezier-max-degree-error.png; T:sm creation limit also added) | 0.4.10+ / 965d3f56 10-27 (degree 16 → exact message) |
| 13.4 | Conflicting constraints reported as a sketch warning | B§7.2 | ✅ | 0.4.10+ / 203c4d6f 10-27 |

## 14. Continuation regressions (2026-10-18)

| # | Behaviour | Source | Status | Verified |
|---|---|---|---|---|
| 14.1 | Circular pattern instances follow seed edits and a shared draggable center | H:sketch_circular_pattern | ✅ (T:ux: seed drag, center drag, circle/ellipse dimension edits, deletion cleanup; GUI recheck pending) |  |
| 14.2 | Bezier creation rejects a seventeenth control point with the degree-15 message and still allows completion | H:bezier, i18n | ✅ (T:sm `bezier_creation_stops_at_degree_fifteen_and_can_still_finish`; live: 14 points → Deg 13, no message) |  |
| 14.3 | Holding/releasing Shift updates inference at a stationary cursor; Shift before release suppresses a pending drag snap | H:automatic_inferencing | ✅ (T:sm; GUI :103 m12.png: Shift down at a stationary cursor drops the horizontal inference, release restores it) |  |
| 14.4 | Reopening an ellipse dimension shows its diameter; reopening conic rho stays unitless; accepting unchanged preserves geometry | H:dimension, H:conic | ✅ (T:sm `reopening_ellipse_and_rho_dimensions_preserves_the_displayed_value`, now also with a stored diameter expression) |  |
| 14.5 | One Esc ends a Line chain and exits Line; double-click ends the chain while keeping Line active | Live sketch/56–57, behaviours.md | ✅ (T:sm `line_clicks_chain_and_one_esc_exits_the_tool`; GUI :103) | 0.4.10+ / 203c4d6f 10-27 |
| 14.6 | Sketch context menu offers Confirm Sketch N, accepting the sketch | Live behaviours.md §Leaving a sketch | ✅ (GUI :103 k1-menu.png, k2-menu-line.png) | 0.4.10+ / 203c4d6f 10-27 |
| 14.7 | Conic creation retains a default driving Rho dimension when its initial value box is dismissed; typing updates the same dimension | Live sketch/84–86 | ✅ (GUI :103 k6-conic-c.png: rho box opens with 0.5 selected; Esc keeps a driving `Rho 0.5`) |  |
| 14.8 | Insert image/DXF fallback file chooser actually loads the selected file | GUI :103 | ✅ (GUI :103 current-image-loaded/current-image-inserted and current-dxf-inserted: image rectangle renders; fixture DXF inserts line + circle) |  |
| 14.9 | Esc dismisses a sketch context menu/Select other popup and restores sketch shortcuts, without canceling the tool underneath | GUI :103 | ✅ (GUI :103 k1-esc.png: Esc closes the menu, tool/selection untouched) |  |
| 14.10 | Fast trim drags crossing the viewport do not skip curves between mouse events | H:trim | ✅ (T:sm; GUI :103 m11.png: one fast two-event drag trims all three crossed lines) |  |
| 14.11 | Esc preserves the sketch selection; Space or an empty-space click clears it | Live sketch/31,36 | ✅ (GUI :103 k1-esc.png: line stays selected after Esc) | 0.4.10+ / 965d3f56 10-27 (Esc keeps a selected line, Space clears) |
| 14.12 | S toolbar has the live ten-tool order, includes Spline, and pressing S again repositions it at the cursor | Live sketch/38–39 | ✅ (GUI :103 current-s-toolbar-focused/current-s-repositioned: ten tools, 2×5, second S moves it) | 0.4.10+ / 203c4d6f 10-27 |
| 14.13 | Sketch context menu offers Select other, Zoom to fit, Zoom to selection, with Confirm sketch first | Live sketch/31,37 | ✅ (GUI :103 k2-menu-line.png) | 0.4.10+ / 203c4d6f 10-27 |
| 14.14 | Polygon side-count spinner adjusts the pending polygon; a click commits it (live 10-27: it does not start the next polygon); Esc discards only the pending preview | Live sketch/87–98 | ✅ (GUI :103 k6-poly1-c.png: `◀ N ▶` spinner after the 2nd click; count follows the pointer distance as in live 97–98) |  |
| 14.15 | Text box creation opens the Text dialog; editing supports confirmation/cancel, live preview and the 250-character limit | Live sketch/132–134 | ✅ (GUI :103 k6-text.png / k6-text-ok-c.png: dialog with preview, selected "Default text", 12/250; ✓ creates. Existing texts: double-click reopens the dialog, ✓ applies, ×/Esc keeps the text; T:sm `text_dialog_is_provisional_and_validates_character_limit`; no side panel for selected text; Convert to expression is in the dialog) |  |
| 14.16 | Fillet first click previews/manipulator; Enter commits and opens the radius value box | Live sketch/110–115 | ✅ (T:sm `fillet_two_curves`: staged until Enter, Enter opens the radius box; GUI :103 k3-fillet.png + k3-enter-c.png; Enter path now syncs the value box) | 0.4.10+ / 203c4d6f 10-27 |
| 14.17 | Linear sketch pattern offers independent X/Y counts and spacing, defaults 3×1 | Live sketch/136–137 | ✅ (two directions with linked grid, T:sm `linear_pattern_second_direction_links_grid_and_round_trips_edit`; GUI :103 k4: `3x`/`1x` + spacing `25` on canvas, typed 40 mm applied as a driving dimension) | 0.4.10+ / 965d3f56 10-27 (3x / 1x, 25 mm) |
| 14.18 | Enter mid-chain (or with any tool's idle value box) places the boxed value as a driving dimension; the tool and chain continue | Live sketch/212–213 | ✅ (T:sm `enter_mid_chain_places_the_boxed_length_as_a_driving_dimension`; GUI :103 e1/e2.png: boxed 18.188 → placed dimension, rubber band continues) | 0.4.10+ / 203c4d6f 10-27 |
| 14.19 | Enter with a drawing tool and nothing pending does nothing (tool stays, sketch stays open) | Live behaviours.md §tool-state | ✅ (Accept capture stops propagation while a drawing tool is active) | 0.4.10+ / 203c4d6f 10-27 |
| 14.20 | Press-drag-release from a Line chain's end makes no arc and places no point; the rubber band keeps following | Live sketch/210–211 | ✅ (T:sm `press_drag_from_the_chain_end_places_nothing`; GUI :103 g2.png matches live 211) |  |
| 14.21 | Spline / Bezier: first Esc commits the curve through the placed points (cursor point dropped) and keeps the tool; second Esc exits | Live sketch/22x-esc-spline, 233 | ✅ (T:sm `first_escape_commits_a_spline_and_the_second_exits`; GUI :103 h123.png) | 0.4.10+ / 203c4d6f 10-27 |
| 14.22 | The sketch's own plane is not pre-highlighted while sketching on it | Live sketch/09, 51 | ✅ (GUI :103 g1/g2.png: no tan plane highlight under the cursor; was highlighted before) | 0.4.10+ / 965d3f56 10-27 (Front sketch: no plane pre-highlight under the pointer) |
| 14.23 | An idle value box shows a bare number like a dimension label (`1.269`), units appear only once typing starts | Live sketch/211–212 | ✅ (T:sm asserts the label; GUI :103 k1-box-c.png shows `18.188`) | 0.4.10+ / 203c4d6f 10-27 |
| 14.24 | Sketch context menu in live order (live 10-24, a selected line, other sketches shown): Confirm \| Copy sketch entities, Transform sketch entities \| Construction \| Extrude…, Revolve… \| Hide other sketches, Hide all sketches (other sketches shown), Curve/surface analysis… \| Clear selection, Select ▸, Select other… \| Zoom to fit, Zoom to selection, View normal to sketch plane, Align view with vertical axis (one line) \| Delete sketch entity; no Section view…; nothing selected: Copy sketch, Show all | Live sketch/31, 37, 204; "qa4 round 5 s5 driven dims" (3); qa4 round 5 v3 s6 (SK-LINE-MENU) | ✅ (SKETCH-LINE-MENU not a bug on a line, qa 10-27) (GUI :103 skasm 10-26, one sketch: Confirm \| Copy sketch entities, Transform sketch entities \| Construction \| Extrude…, Revolve… \| Curve/surface analysis… \| Clear selection, Select ▸, Select other… \| Zoom to fit, Zoom to selection, View normal, Align view \| Delete sketch entity, as live (5)) | 0.4.10+ / daf1f50c 10-27 (line's middle: live's menu) |
| 14.25 | Select ▸ Tangent connected edges / Equal length/radius edges / Parallel edges grow the sketch selection | Live sketch/32 | ✅ (T:ux `select_related_grows_tangent_chains_equal_sizes_and_parallels`; GUI :103 k2-parallel-c.png: Parallel edges selects the slot sides + centerline) |  |
| 14.26 | Default tool sizes follow the document unit: inch docs fillet R0.5, chamfer/offset 0.25, slot Ø0.5, pattern spacing 1 | Live sketch/107–137 | ✅ (live: inch R0.5/0.25/0.25/Ø0.5/1 in; mm R20/5/5/Ø20/25 mm; GUI :103 mm doc slot shows Ø20) | 0.4.10+ / 965d3f56 10-27 (mm: slot Ø20, pattern 25; fillet R5 on a fresh session = live behaviours 7976–7981, the R20 in this row's text was a sticky value) |
| 14.27 | Slot works only on existing curves (empty-space clicks do nothing); the slot is a light-blue preview until the next click / Enter / leaving the tool; the picked centerline stays solid; no value box opens | Live sketch/104–109, 307–311 | ✅ (T:sm `slot_previews_until_the_next_click_and_keeps_its_centerline_solid`, T:ux slot tests updated; GUI :103 k2-slot.png) | 0.4.10+ / 203c4d6f 10-27 |
| 14.28 | Enter with no tool active does not accept the sketch | Live sketch/311 | ✅ (GUI :103 k2-enter.png: sketch dialog stays open) | 0.4.10+ / 203c4d6f 10-27 |
| 14.29 | Show constraints starts off in a new session (live sketch/00, 01, 10) and the dialog remembers the last setting (live 44, 200) | Live sketch/00–10, 44, 200 | ✅ (static default off, remembered via `set_show_constraints`; an earlier entry here wrongly said "on") | not verified: ours starts ☑ on a fresh HOME (1c853f78), as core-workflow 2.2 wants; 14.29 says off (live sketch/00) — conflicting rows, see handoff |
| 14.30 | Pattern instance counts are on-canvas `Nx` labels (click to type); the linear spacing is an on-canvas value (click to type, expressions allowed); no options panel for linear pattern | Live sketch/136–138 | ✅ (T:sm `pattern_count_labels_default_to_live_counts_and_accept_typing`; GUI :103 k4.png, k4b.png) | 0.4.10+ / 965d3f56 10-27 (Nx labels click-to-type, spacing on canvas) |
| 14.31 | No floating options panel for fillet, polygon, conic (they use the manipulator / spinner / rho box) | Live sketch/87–98, 84–86, 110–115 | ✅ (GUI :103 k3-enter-c.png: no panel) | 0.4.10+ / 965d3f56 10-27 (fillet: R5 + arrow, no panel) |
| 14.32 | Circular pattern defaults to 3 instances | Live sketch/138 | ✅ (T:sm) | 0.4.10+ / 203c4d6f 10-27 |
| 14.33 | Circular pattern / Transform still use the floating options panel (live: on-canvas only) | Live sketch/138–139 | ✅ (circular: `3x` label + open-pattern angle value, GUI :103 k5-open-c/k5-90-c.png; transform: triad on the selection, rotation/scale fields appear once dragged, GUI k5-xf-c/k5-rot-c.png; T:sm `transform_triad_follows_the_selection_until_its_center_is_dragged`; no options panels remain for patterns/transform) |  |
| 14.34 | Right-click on a model edge in a sketch: Confirm \| Use \| Hide Part, Curve/surface analysis…, Isolate…, Make transparent… \| Clear selection, Select ▸, Select other… \| zoom/view items incl. Align view with vertical axis | Live sketch/204 | ⏳ (model-edge layout added: Confirm | Use | Hide <part>, Curve/surface analysis…, Isolate…, Make transparent… | Clear selection, Select other… | ⏳ (model-edge layout: Confirm | Use | Hide <part>, Curve/surface analysis…, Isolate…, Make transparent… | Clear selection, Select ▸ (edge/face criteria + Create selection…), Select other… | ✅ (GUI :103 m7-edge-menu.png matches live 204: Confirm | Use | Hide Part 1, Curve/surface analysis…, Isolate…, Make transparent… | Clear selection, Select ▸ (Tangent connected, Loop/chain, Parallel, Equal radius, Create selection…; m7-sub-c.png), Select other… | Zoom to fit, Zoom to selection, View normal (+ Align view for straight edges); Add comment is a cloud-only exclusion) |  |
| 14.35 | Bottom-right one-line readout of the sketch selection: Length / Diameter / Radius / Point X Y Z / Distance / Parallel dist / Angle (deg); nothing for mixed selections | Live behaviours.md §Drawing tools (status bar), sketch/108, 136, 139 | ✅ (T:sm; GUI :103 m5-readout-c.png "Diameter: 50.8 mm") | 0.4.10+ / 203c4d6f 10-27 |
| 14.36 | Typed expressions on ellipse axis (diameter) dimensions are kept and regenerate correctly | H:dimension | ✅ (T:sm + model test `ellipse_diameter_expressions_regenerate_as_radii` pass) |  |
| 14.37 | Used (projected) geometry carries the "use" glyph at its middle and both ends; hovering it highlights the entity; selecting it + Delete unlinks the geometry (stays as free sketch geometry) | Live sketch/202 | ✅ (T:sm; GUI :103 m9-hover-c.png: cube use glyph on hover; select + Delete → m9-unlinked-c.png, the circle becomes free blue geometry) | 0.4.10+ / 203c4d6f 10-27 |
| 14.38 | Used geometry is drawn black (fixed) like live, not purple | Live sketch/202 | ✅ (ui2: `sketch_color` draws external geometry in the constrained color; `SKETCH_EXTERNAL` removed) | 0.4.10+ / 965d3f56 10-27 (Use: black line) |
| 14.39 | Live values next to the rubber band while drawing (line length, rectangle width/height, circle / polygon Ø halfway to the cursor when it fits, else outside (across the centre for a horizontal cursor, below the shape otherwise; live skasm 10-27, T:sm `circle_and_polygon_readout_sits_halfway_or_outside_as_live`), arc R, ellipse axes, polygon Ø) in `#449ccd` 13 px, no box | Live sketch/05, 09, 51, 76–80 | ✅ (QA3; `preview_values`, T:sm `rubber_band_shows_live_values`; GUI :103 q1.png / q3-c.png) | 0.4.10+ / 203c4d6f 10-27 |
| 14.40 | Point tool drops hollow points (white center, colored ring); Bezier control points hollow | Live behaviours.md §Point, sketch/103 | ✅ (QA3; point style bit `PointSprite::HOLLOW` (render shader) for standalone points and spline control points; GUI q1.png) | 0.4.10+ / 203c4d6f 10-27 |
| 14.41 | Vertical (steep) linear dimensions: text beside the unbroken dimension line; horizontal ones break the line around the text | Live sketch/17 | ✅ (QA3; `labels_beside_line`; GUI q3-c.png) |  |
| 14.42 | After a new dimension's value is applied (Enter, ✓, Esc, click away) nothing is selected; editing an existing dimension leaves the selection as it was | Bundle: the sketch Dimension tool's commit ends in `cleanUpAfterCommit` → `selectionList.reset()`, the dimensioned entities are only highlighted while the edit box is open (`highlightDimensionEntities` / `clearDimensionEntityHighlights`); live sketch/15 (applied "4", unselected). sketch/148 (selected 30°, an over-constrain attempt) is not a fresh dimension commit, so it does not contradict this | ✅ (GUI :103 dm4: Enter → "54.565" unselected, tool stays; `DimEdit.created`; `commit_dimension`, `SketchEditor::cancel`; T:sm `a_new_dimension_leaves_nothing_selected`; was: QA3 selected it) | 0.4.10+ / 203c4d6f 10-27 |
| 14.43 | Right-click with a tool active: "Escape line" first (directly above Confirm Sketch N, no separator), no Curve/surface analysis; clicking it exits the tool | Live sketch/400–406, menus/rmb-sketch-linetool-*.png | ✅ (GUI :103 s2 matches rmb-sketch-linetool-empty incl. the separator before Select ▸) | 0.4.10+ / 203c4d6f 10-27 |
| 14.44 | Select ▸ with nothing selected lists only "Create selection…" | Live menus/rmb-sketch-empty-select-submenu.png | ✅ (GUI :103 s3) | 0.4.10+ / 203c4d6f 10-27 |
| 14.45 | Esc with that menu open: first Esc exits the tool (menu stays), second Esc closes the menu | Live sketch/405–408 | ✅ (GUI :103 s4–s5; `OsMenu::on_first_escape`) |  |
| 14.46 | Choosing a drawing tool (L) clears the selection | Live sketch/401–402 | ✅ (T:sm `choosing_a_drawing_tool_clears_the_selection`) | 0.4.10+ / 203c4d6f 10-27 |
| 14.47 | A dimension's value box shows the value with the document unit, selected: lengths with 6 decimals ("3.51041 in", "1.725582 in"), angles with 7 ("35.5376823 deg"), zeros trimmed; a value typed as a plain number reopens with its unit ("80" → "80 mm") | Live sketch/13, 141, 143; QA3 | ✅ (`dimension_edit_text`; Enter on the untouched box keeps the exact value (`commit_dimension`); T:sm `reopening_ellipse_and_rho_dimensions_preserves_the_displayed_value` extended; test passes; GUI :103 dm3 shows "16.825693 mm". Visual left for ui2: our field shows the text left-aligned and clips the unit's last letter, while live right-aligns it inside the 100 px field) | 0.4.10+ / 203c4d6f 10-27 |

## Constraint glyph layout (ui2, 2026-10-25; live sketch/27, 28 vs ours at 1× / 2×)
| Item | Live | Ours before | Status | Verified |
|---|---|---|---|---|
| Line glyphs | a row centred on the line's midpoint, 20.5 px out (below a horizontal line, left of a vertical one), 20 px white boxes 22 px apart | started at the midpoint and ran right, 6 px off the line | ❌ (SKETCH-GLYPH-LAYOUT-LOST) (box 11 px under a horizontal line, as live) | 0.4.10+ / 8602df0d 10-27 |
| Point glyphs | up-left of the point (box top-left −27, −26) with a 1 px `#a8b2bb` leader | up-right, no leader | ❌ (SKETCH-GLYPH-LAYOUT-LOST) | 0.4.10+ / 8602df0d 10-27 |
| Box | plain white square, no border, radius or shadow | radius 2 + shadow | ❌ (SKETCH-GLYPH-LAYOUT-LOST) | 0.4.10+ / 8602df0d 10-27 |
| Icon | spans 14 px of the box | 11 px | ❌ (SKETCH-GLYPH-LAYOUT-LOST) (18 px icon) | 0.4.10+ / 8602df0d 10-27 |
| Hovered glyph `#fcda7f`, toolbar, inference glyph | | | ✅ | 0.4.10+ / 965d3f56 10-27 (Equal glyph bg #fcda7f, both ellipses #ffc685; pattern glyph see SKETCH-PATTERN-GLYPH-HOVER) |
| Rectangle corner coincident glyphs | four, at the corners | none: our corners share points, no coincident constraints | → skb (model) |  |
| Hovered / selected sketch line | `#fec788` / `#feab10` ~80 % over the black line (darker core `#ca9e6b` / `#ca870b`) | the line recoloured, solid | → render owner (the line stays under a translucent highlight) |  |
| Dimension value box (hover / selected) | 27 px high, centred on the value (sketch/24) | 22 px | ❌ (SKETCH-GLYPH-LAYOUT-LOST) | 0.4.10+ / 8602df0d 10-27 |
| Inference glyph box | 20 px of 40 % white behind the icon, seen over a face (part-studio/1032) | none | ❌ (SKETCH-GLYPH-LAYOUT-LOST) | 0.4.10+ / 8602df0d 10-27 |
| Alignment guide | 1 px yellow, 4 px on / 4 off, no dot at the pointer (part-studio/1032) | 7 px `#ffc20e` dash + 8 px yellow dot | ✅ |  |
