# Sketch mode: Onshape behaviour parity checklist

One line per behaviour. Status: ✅ matches Onshape (verified in the app with xdotool on Xvfb and/or a
sketch-crate test), 🔧 partly there / differs, ❌ missing, ⏳ implemented, not yet verified.

Sources: `H:<page>` = Onshape help page `onshape-research/docs-raw/md/Sketch__<page>.md` (other prefixes
spelled out); `B§n` = `research/sections/B-sketch.md` section; `SK-xx` = `research/sections/02-sketch.md`
item; `i18n` = `docs-raw/bundle-i18n-strings.txt`; `LE` = `onshape-research/live-exploration.md`.
Tests: `T:ux` = `crates/sketch/tests/onshape_ux.rs`, `T:inf` = `crates/sketch/src/inference.rs` tests,
`T:sm` = `crates/app/src/sketch_mode.rs` tests.

## 1. Entering, finishing, cancelling

| # | Behaviour | Source | Status |
|---|---|---|---|
| 1.1 | Sketch button / Shift+S opens the Sketch dialog prompting "Select a sketch plane" | B§1.1, i18n | ✅ |
| 1.2 | A preselected plane or planar face fills the Sketch plane field | B§1.1 | ✅ |
| 1.3 | Context menu on a plane/face: **New sketch** | H:sketch_basics, LE | ✅ |
| 1.4 | Double-click a sketch in the Feature list (or Edit) re-opens it | B§1.1 | ✅ |
| 1.5 | View is **not** reoriented on entry by default; `n` views normal to the sketch plane, `n` again flips | View__orient_normal_to_sketch_on_edit, B§1.4 | ✅ |
| 1.6 | "Orient normal to sketch on edit" option in the camera menu | View__orient_normal_to_sketch_on_edit | ✅ (Part Studio camera menu; the sketch view orients on entry) |
| 1.7 | A new empty sketch starts with the Line tool | LE (Onshape default) | ✅ |
| 1.8 | ✓ / Enter accepts the sketch; "Finish sketch" button | H:sketch_basics, B§1.5 | ✅ |
| 1.9 | ✗ cancels the whole sketch session; a Restore link undoes the cancel | H:sketch_basics | 🔧 cancel works, no Restore bubble (feature dialog, other agent) |
| 1.10 | Esc cancels the active tool / chain, never the sketch | H:sketch_tools ("The Escape key exits a tool selection") | ✅ |
| 1.11 | Extrude (Shift+E) / Revolve (Shift+W) from the sketch toolbar accept the sketch and preselect all regions | H:sketch_tools | ✅ |
| 1.12 | Ctrl+Z / Ctrl+Y inside the sketch step through sketch edits | LE, B§8.2 | ✅ |
| 1.13 | Sketch dialog options are remembered between sessions (except Show expressions) | H:sketch_basics | ✅ (Show constraints remembered; Show expressions resets) |
| 1.14 | Disable imprinting checkbox when sketching on a part face | H:sketch_basics | ✅ (feature param) |
| 1.15 | Feature list shows a blue indicator on a sketch that is not fully defined | PartStudio__features_and_parts_lists, LE | ✅ |
| 1.16 | Shift+H hides / shows all sketches | H:sketch_basics | ✅ |
| 1.17 | Shift+P hides construction geometry | B§1.7 | 🔧 binding exists (Part Studio) |

## 2. Toolbar

| # | Behaviour | Source | Status |
|---|---|---|---|
| 2.1 | Sketch toolbar replaces the feature toolbar while a sketch is open | H:sketch_tools | ✅ |
| 2.2 | Order: Undo, Redo, Extrude, Revolve, Line▾, Rectangle▾, Circle▾, Arc▾, Polygon▾, Spline▾, Point, Text, Use▾, Construction, Fillet▾, Trim▾, Offset▾, Mirror, Pattern▾, Dimension, 14 constraints | LE (docs toolbar images) | ✅ (no Insert DXF/image group) |
| 2.3 | Groups: Line/Midpoint line; Corner/Center point/Aligned rectangle; Center point/3 point circle/Ellipse; 3 point/Tangent/Center point/Elliptical arc/Conic; Inscribed/Circumscribed polygon; Spline/Bezier/Spline control point; Use/Intersection; Fillet/Chamfer; Trim/Extend/Split; Offset/Slot; Linear/Circular pattern/Transform | LE | ✅ |
| 2.4 | A group button shows the last tool used from that group | H:sketch_tools | ✅ |
| 2.5 | The active tool's button is highlighted | UI | ✅ |
| 2.6 | Tools stay active until Esc or another tool is chosen | H:sketch_tools | ✅ |
| 2.7 | Pressing a tool's shortcut (or button) again exits it | B§2.1 | ✅ |
| 2.8 | Constraint buttons not applicable to the current selection are disabled | B§3.1 | ✅ (T:sm) |
| 2.9 | Tooltips name the tool and its shortcut | UI | ✅ |
| 2.10 | Search tools (Alt+C) finds sketch tools | H:sketch_tools | 🔧 Part Studio palette (feature tools) |
| 2.11 | S opens the sketch shortcut toolbar at the cursor | H:sketch_basics | ✅ (sketch tools in the S toolbar) |

## 3. Keyboard shortcuts (sketch scope)

| # | Behaviour | Source | Status |
|---|---|---|---|
| 3.1 | l Line, g Corner rectangle, r Center point rectangle, c Center point circle, a 3 point arc, Shift+S Point | B§8.1 | ✅ |
| 3.2 | q Construction, Shift+F Fillet, m Trim, x Extend, o Offset, u Use, d Dimension | B§8.1 | ✅ |
| 3.3 | i Coincident, Shift+O Concentric, b Parallel, t Tangent, h Horizontal, v Vertical, Shift+L Perpendicular, e Equal, Shift+M Midpoint, Shift+K Normal, Shift+G Pierce, Shift+Q Symmetric, Shift+J Fix, Shift+U Curvature | B§8.1 | ✅ |
| 3.4 | Shift+A switches Line ↔ Tangent arc while chaining (keeps the chain) | H:line | ✅ |
| 3.5 | Shift (hold) suppresses inferences while drawing / dragging | H:automatic_inferencing | ✅ |
| 3.6 | Shift (hold) shows all constraint glyphs while moving | H:working_with_constraints | ✅ |
| 3.7 | Alt (hold) while drawing a rectangle makes a square | H:corner_rectangle | ✅ |
| 3.8 | Alt+←/→ switches between on-the-fly dimension boxes | H:sketch_basics | ✅ |
| 3.9 | Delete / Backspace deletes selected entities, constraints and dimensions | H:dimension, H:working_with_constraints | ✅ |
| 3.10 | Space clears the sketch selection | B§8.2 | ✅ |
| 3.11 | Ctrl+C / Ctrl+V copy / paste sketch entities | H:sketch_basics | ✅ (T:ux) |
| 3.12 | `#` typed without `=` in a sketch value box is the view shortcut; `=#var` enters a variable | H:sketch_basics | ✅ |
| 3.13 | Shortcuts are user-customizable | B§2.1 | ❌ |

## 4. Geometry tools: clicks, drags and results

| # | Behaviour | Source | Status |
|---|---|---|---|
| 4.1 | Line: click-click-click chains connected segments | H:line | ✅ |
| 4.2 | Line: press-drag-release makes a single segment | H:line | ✅ |
| 4.3 | Line: clicking an existing point closes / ends the chain | B§2.4.1 | ✅ |
| 4.4 | Line: moving back onto the chain end switches to a tangent arc; after the arc the tool reverts to Line | H:line | ✅ (T:sm) |
| 4.5 | Midpoint line: click the midpoint, then an end; midpoint constraint | H:midpoint_line | ✅ |
| 4.6 | Corner rectangle: click corner, click diagonal corner, or drag | H:corner_rectangle | ✅ |
| 4.7 | Center point rectangle: click center, click corner, or drag; construction diagonal with midpoint at the center | H:center_point_rectangle | ✅ |
| 4.8 | Aligned rectangle: two clicks for the first side, third click for the width | H:aligned_rectangle | ✅ (T:sm) |
| 4.9 | Center point circle: click center, click rim, or drag | H:center_point_circle | ✅ |
| 4.10 | 3 point circle: three clicks, or drag 1→2 then click 3 | H:3_point_circle | ✅ (T:sm) |
| 4.11 | 3 point arc: start, end, then the bulge point; drag start→end then click | H:3_point_arc | ✅ |
| 4.12 | 3 point arc: snaps to an exact semicircle when the center reaches the chord | H:3_point_arc | ✅ |
| 4.13 | Tangent arc tool: start at a curve end, click the end; tangent constraint | H:arc_tangent | ✅ (T:sm) |
| 4.14 | Center point arc: center, start (radius), end; the sweep follows the cursor's direction | H:center_point_arc | ✅ (T:sm) |
| 4.15 | Ellipse: center, primary axis, secondary radius | H:ellipse | ✅ (T:sm) |
| 4.16 | Elliptical arc: center, primary axis, secondary radius + start, end on the construction ellipse | H:elliptical_arc | ✅ (T:sm) |
| 4.17 | Elliptical arc start/end snap to the axis / quadrant points | H:elliptical_arc | ✅ (axis / quadrant snaps on the construction ellipse) |
| 4.18 | Conic: start, end, control point; rho value | H:conic | ✅ (rho dimension, box opens after the third click) |
| 4.19 | Inscribed / circumscribed polygon: center, circumference; construction circle | H:inscribed_polygon | ✅ |
| 4.20 | Polygon: type the side count before the circumference click (3–50) | H:inscribed_polygon | ✅ |
| 4.21 | Polygon: drag after the circumference click to change the side count | H:inscribed_polygon | ✅ (T:sm) |
| 4.22 | Spline: click points, double-click to end | H:spline | ✅ (T:sm) |
| 4.23 | Spline: closes when the last point lands on the first | H:spline | ✅ (T:sm) |
| 4.24 | Spline: context menu **Confirm spline** finishes one point behind | H:spline | ✅ (T:sm) |
| 4.25 | Spline tangent handles (white points) to drag | H:spline | ✅ (T:ux; white handles, drag to change the end tangents, not deletable) |
| 4.26 | Bezier: click control points, double-click to end; control polygon shown | H:bezier | ✅ (control polygon shown while drawing and when selected) |
| 4.27 | Bezier: context menu **Escape bezier** removes the curve in progress | H:bezier | ✅ (T:sm) |
| 4.28 | Bezier degree label (double-click to edit, max 15) | H:bezier | ✅ (T:ux; "Degree N" label, double-click to edit) |
| 4.29 | Spline control point: click a spline / bezier control polygon to add a point | H:spline_control_point | ✅ (T:ux) |
| 4.30 | Point: click to place | H:point | ✅ |
| 4.31 | Text: drag a box, lower-left corner and height set the first line; horizontal constraint on the bottom edge | H:text | ✅ |
| 4.32 | Text: font, bold, italic, flip; edit later; drag by its box | H:text | ✅ |
| 4.33 | Text: Convert to expression | H:text | ⏳ (model evaluation + toggle in the text options) |
| 4.34 | Slot: select centerline curves (before or after the tool); one shared width dimension | H:slot | ✅ (T:ux) |
| 4.35 | Slot: a connected chain gives one slot following the chain | H:slot | ✅ (T:ux) |
| 4.36 | Construction toggle: q with a selection toggles it, without one draws construction geometry | H:construction | ✅ |
| 4.37 | Construction lines are dashed (dash-dot) in the status color | B§5.2 | 🔧 rendering (scene builder) |
| 4.38 | Insert image / Insert DXF or DWG | H:insert_image, H:insert_dwg_or_dxf | 🔧 DXF (lines, arcs, circles, polylines, ellipses, splines, units, file origin) and images (drawn when the view is normal to the box); no DWG reader |
| 4.39 | Preview geometry while placing is drawn in the sketch preview color | B§2.3 | ✅ |

## 5. On-the-fly dimensions

| # | Behaviour | Source | Status |
|---|---|---|---|
| 5.1 | Right after creation a box shows the suggested value (line length, circle diameter, arc radius) | H:sketch_basics, H:line | ✅ |
| 5.2 | Typing a value or expression and Enter adds a driving dimension | H:line | ✅ |
| 5.3 | Continuing to sketch leaves the geometry undimensioned (box disappears) | H:sketch_basics | ✅ |
| 5.4 | Rectangles: length box, Enter, width box, Enter | H:corner_rectangle | ✅ |
| 5.5 | Ellipse / elliptical arc: primary then secondary diameter | H:ellipse | ✅ (T:sm) |
| 5.6 | Polygon: diameter of the construction circle | H:inscribed_polygon | ✅ |
| 5.7 | Fillet: radius box right after the first fillet | H:sketch_fillet | ✅ |
| 5.8 | `=#var` uses a variable; expressions with units and functions | H:sketch_basics | ✅ |
| 5.9 | Esc closes the box without dimensioning | — | ✅ |
| 5.10 | Chained lines: the chain continues from the dimensioned end | H:line | ✅ |

## 6. Inference and snapping

| # | Behaviour | Source | Status |
|---|---|---|---|
| 6.1 | Snap to existing points creates a coincident (shared point) | B§6 | ✅ |
| 6.2 | Snap to the sketch origin; coincident to the origin | H:automatic_inferencing | ✅ (T:inf) |
| 6.3 | Horizontal / vertical alignment with the origin and other points: dotted guide, constraint on click | H:automatic_inferencing | ✅ |
| 6.4 | Midpoint snap on lines and arcs creates a midpoint constraint | B§6 | ✅ |
| 6.5 | Center snap on circles / arcs; quadrant snap | B§6 | ✅ |
| 6.6 | Point on curve snap creates a coincident | B§6 | ✅ |
| 6.7 | New line horizontal / vertical inference | B§6 | ✅ |
| 6.8 | Parallel / perpendicular to the previous segment | B§6 | ✅ |
| 6.9 | Wake-up: lines the cursor passed over give parallel / perpendicular inferences | H:automatic_inferencing | ✅ (T:inf) |
| 6.10 | Tangent inference from an arc / spline end | B§6 | ✅ (T:inf) |
| 6.11 | Inference glyph next to the cursor | H:working_with_constraints | ✅ |
| 6.12 | Inference line yellow dashed (`INFERENCE_SNAP_LINE_COLOR`) | B§6 | ✅ |
| 6.13 | Shift suppresses inferences | H:automatic_inferencing | ✅ |
| 6.14 | Snap to model vertices and edges when sketching on a face; vertices stay linked | H:sketch_basics | ⏳ (edge snaps link a coincident to a projected edge) |
| 6.15 | Woken entity / parallel partner highlighted orange, new line drawn dotted when parallel | H:automatic_inferencing | ✅ |
| 6.16 | Ellipse quadrant (Quadrant constraint) inference | H:working_with_constraints | ✅ (T:ux, Quadrant constraint) |

## 7. Dimension tool

| # | Behaviour | Source | Status |
|---|---|---|---|
| 7.1 | d / button activates; stays active for the next dimension | H:dimension | ✅ |
| 7.2 | Picked entities highlight; the dimension previews at the cursor before placing | H:dimension | ✅ |
| 7.3 | The placement click places the dimension and opens its value box | H:dimension | ✅ |
| 7.4 | Single line: length; slanted line dragged up/down or left/right: horizontal / vertical | H:dimension | ✅ (T:sm) |
| 7.5 | Two points: aligned when dragged at an angle, horizontal / vertical when dragged straight | H:dimension | ✅ |
| 7.6 | Two parallel lines: distance (implies parallel) | H:dimension | ✅ (T:sm) |
| 7.7 | Two lines at an angle: angle; the quadrant follows the cursor | H:dimension | ✅ (T:sm) |
| 7.8 | Circle: diameter; arc: radius | H:dimension | ✅ |
| 7.9 | Arc length: both end points then the arc | H:dimension | ✅ (T:sm) |
| 7.10 | Point / line / circle to a construction line, cursor across it: centerline dimension (twice the distance) | H:dimension | ✅ (T:sm, T:ux) |
| 7.11 | Ellipse: major / minor diameter | B§4.2 | ✅ (T:sm) |
| 7.12 | Dimension to the sketch origin | H:dimension | ✅ (T:sm) |
| 7.13 | Dimension to planes / model edges | H:dimension | ⏳ (plane traces / projected edges as linked reference geometry) |
| 7.14 | Over-defining dimensions are made driven, shown in parentheses and gray | H:dimension | ✅ |
| 7.15 | Driven ↔ driving from the context menu ("Change to driving/driven dimension") | H:dimension | ✅ |
| 7.16 | Double-click a dimension to edit its value | H:dimension | ✅ |
| 7.17 | Editing moves both dimensioned sides by half (except radius / diameter) | H:dimension | ✅ (T:ux) |
| 7.18 | Negative values flip direction (distance, angle, line length); not allowed for arc, radius, diameter | H:dimension | ✅ (T:sm) |
| 7.19 | Drag the dimension label to move it | B§4 | ✅ |
| 7.20 | Select + Delete removes a dimension; context menu Delete | H:dimension | ✅ |
| 7.21 | Expression indicator next to values that use expressions; Show expressions shows them (≤70 chars) | H:sketch_basics | ✅ |
| 7.22 | Errors: "A dimension cannot be created between these items." / "Could not create the dimension." | i18n | ✅ |
| 7.23 | Driving dimensions black, driven gray, hover/selected gold, conflicting red | Home__colors, B§4.5 | ✅ |
| 7.24 | Tolerances, precision, configure dimension | B§4.3 | ⏳ (tolerance types, deviations, precision; configure dimension) |
| 7.25 | Min / max distance to circles | SK-D07 | ⏳ (rim picks measure min / max) |

## 8. Constraints

| # | Behaviour | Source | Status |
|---|---|---|---|
| 8.1 | Select-first: constraint applies immediately to the selection | H:working_with_constraints | ✅ |
| 8.2 | Multi-selection: Parallel / Equal / Coincident / Tangent chain all entities, Horizontal / Vertical / Fix apply to each | H:parallel, H:equal, H:horizontal | ✅ (T:sm) |
| 8.3 | Tool-first: each pair picked is constrained; tool stays on until clicked again or another tool | H:coincident | ✅ |
| 8.4 | Coincident: point-point, point-curve, collinear lines, same circle | H:coincident | ✅ |
| 8.5 | Concentric, Parallel, Tangent, Horizontal, Vertical, Perpendicular, Equal, Fix | constraint pages | ✅ |
| 8.6 | Midpoint: point + line/arc, or 3 points (start, end, middle) | H:midpoint | ✅ |
| 8.7 | Normal: line + curve | H:normal | ✅ |
| 8.8 | Pierce: point + off-plane edge | H:pierce | ⏳ (pierce point of an off-plane edge) |
| 8.9 | Symmetric: axis line first, then two entities | H:symmetric | ✅ |
| 8.10 | Curvature: spline/conic + adjacent curve | H:curvature | ✅ |
| 8.11 | Constraints to planes / model edges | H:working_with_constraints | ⏳ (planes and model edges as linked reference geometry) |
| 8.12 | Glyphs show for the hovered entity (show after hover, hide after 750 ms); all with Show constraints | H:working_with_constraints, B§3.5 | ✅ |
| 8.13 | Hover a glyph highlights its entities (yellow); click selects it; Delete removes it | H:working_with_constraints | ✅ |
| 8.14 | Drag glyphs to move them | H:working_with_constraints | ✅ |
| 8.15 | Conflicting constraints: white glyph on red | H:working_with_constraints | ✅ |
| 8.16 | External (Use) constraints have a blue glyph background | H:working_with_constraints | ⏳ |
| 8.17 | Equal: if one curve is dimensioned, its size wins | H:equal | ✅ (solver) |
| 8.18 | Constraint manager / Profile inspector | H:working_with_constraints | ✅ (profile inspector verified in the app; constraint manager ⏳) |

## 9. Selection

| # | Behaviour | Source | Status |
|---|---|---|---|
| 9.1 | Click selects; clicking more adds; clicking a selected entity deselects | Onshape | ✅ |
| 9.2 | Box select left→right: window (entirely inside) | B§5.4 | ✅ |
| 9.3 | Box select right→left: crossing (anything the box touches) | B§5.4 | ✅ (T:sm) |
| 9.4 | Hover highlights orange, selection amber, related entities yellow | H:working_with_constraints | ✅ |
| 9.5 | Not fully constrained points win over fully constrained ones under the cursor | H:sketch_tools | ⏳ |
| 9.6 | Clicking a closed region selects it | B§1.6 | ✅ |
| 9.7 | Select other (`) | B§5.4 | ⏳ |
| 9.8 | Click in empty space clears the selection | Onshape | ✅ |

## 10. Dragging and solving

| # | Behaviour | Source | Status |
|---|---|---|---|
| 10.1 | With no tool, under-defined points and curves drag with the solver | H:working_with_constraints | ✅ |
| 10.2 | Fully defined / fixed geometry does not move | Home__colors | ✅ |
| 10.3 | Dropping a point on another point merges them; on a curve makes it coincident | B§7.1 | ✅ |
| 10.4 | Dropping a point on the origin constrains it to the origin | B§7.1 | ✅ (T:sm) |
| 10.5 | Shift while dragging suppresses the snap | B§7.1 | ✅ (T:sm) |
| 10.6 | Colors: blue under-defined, black fully defined, red over-defined, per entity | Home__colors | 🔧 colors come from the scene builder palette |
| 10.7 | Text boxes drag as a whole | H:text | ✅ |

## 11. Edit tools

| # | Behaviour | Source | Status |
|---|---|---|---|
| 11.1 | Fillet: click a vertex, radius box; later vertices make equal fillets | H:sketch_fillet | ✅ |
| 11.2 | Fillet: click two curves | H:sketch_fillet | ✅ (T:sm) |
| 11.3 | Fillet keeps a virtual sharp with the coincident constraints | H:sketch_fillet | ✅ |
| 11.4 | Fillet drag manipulator; too-large preview in red | H:sketch_fillet | ⏳ (red preview, radius manipulator) |
| 11.5 | Chamfer: vertex or two lines, two distance dimensions (45° default) | H:sketch_chamfer | ✅ (T:ux) |
| 11.6 | Trim: click removes up to the nearest intersections, whole curve when unbounded | H:trim | ✅ |
| 11.7 | Trim: drag across curves trims everything crossed | H:trim | ✅ (T:sm) |
| 11.8 | Trim: click a standalone point deletes it | H:trim | ✅ (T:sm) |
| 11.9 | Extend: click a line or arc to extend to the next boundary | H:extend | ✅ |
| 11.10 | Extend: "Unable to extend the selected geometry." | i18n | ⏳ |
| 11.11 | Split: click a curve where to split | H:sketch_split | ✅ |
| 11.12 | Offset: select curves / chain, distance and direction, Enter; then click more curves | H:offset | ⏳ (preview, arrow drag / click, distance box, Enter / click in space) |
| 11.13 | Mirror with preselection: click the mirror line, resolves at once and the tool ends | H:sketch_mirror | ✅ (T:sm) |
| 11.14 | Mirror tool-first: "Select a mirror line.", then "Select entities to be mirrored.", Esc exits | H:sketch_mirror, i18n | ✅ (T:sm) |
| 11.15 | Linear / circular pattern with on-canvas manipulators (count, spacing, direction, angle, center) | H:sketch_linear_pattern | ⏳ (click in space / Enter accept, Esc cancel) |
| 11.16 | Pattern preview limited to 10 instances | H:sketch_linear_pattern | ✅ |
| 11.17 | Edit pattern from the pattern glyph | H:sketch_linear_pattern | ⏳ (pattern glyph → Edit pattern; instances not linked to the seed) |
| 11.18 | Transform: triad manipulator (move, rotate, scale, free-drag center), Enter commits, Esc cancels | H:sketch_transform | ⏳ (click in space / Enter commit, Esc cancel) |
| 11.19 | Use: click model edges / faces / other sketches to project; linked to the source | H:use | ⏳ (any edge: exact or sampled projection; other sketches; linked) |
| 11.20 | Use: hover preview of the projection | H:use | ⏳ |
| 11.21 | Use silhouette edges | H:use | ⏳ (cylinders, cones, spheres, tori) |
| 11.22 | Intersection: face crossing the sketch plane | H:intersection | ✅ |
| 11.23 | Text cannot be patterned / mirrored in a sketch | H:text | ✅ |

## 12. Context menus

| # | Behaviour | Source | Status |
|---|---|---|---|
| 12.1 | Sketch entities: Delete sketch entity / Delete N sketch entities | i18n, B§9 | ✅ |
| 12.2 | Construction, Transform sketch entities, Copy sketch entities | B§9 | ✅ |
| 12.3 | Paste sketch entities (clipboard holds sketch entities) | B§9 | ✅ |
| 12.4 | View normal to sketch plane; Exit tool; Clear selection | B§9 | ✅ |
| 12.5 | Use (model edge under the cursor) | B§9 | ⏳ |
| 12.6 | Spline in progress: Confirm spline; Bezier: Escape bezier | H:spline, H:bezier | ✅ (T:sm) |
| 12.7 | Dimension: Change to driving / driven dimension, Delete | H:dimension | ✅ |
| 12.8 | Constraint glyph: Delete | H:working_with_constraints | ✅ |
| 12.9 | Add comment, Configure dimension | H:dimension | 🔧 Configure dimension ⏳; Add comment: no comment system in noshape |

## 13. Messages and errors

| # | Behaviour | Source | Status |
|---|---|---|---|
| 13.1 | Tool prompts at the bottom of the view ("Select a mirror line.", …) | i18n | ✅ |
| 13.2 | "Cannot fillet selected point." | i18n | ⏳ |
| 13.3 | "The maximum bezier degree (15) has been reached." | i18n | ⏳ |
| 13.4 | Conflicting constraints reported as a sketch warning | B§7.2 | ✅ |
