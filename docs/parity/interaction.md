# Interaction parity with Onshape (selection, pre-highlight, measure, mass, section view)

Agent nav2 (from 10-25). Same method as navigation (docs/parity/navigation.md):
real OS input into our app (the `.github/nav` replay: uinput under sway on
Wayland and X11, CGEvents on macOS, SendInput / touch injection on Windows)
against live Onshape captures (behaviours.md; Chrome on :107, CDP), each
behaviour with a deterministic replay check.

Status: ✅ verified with evidence · 🔧 implemented, not yet verified here ·
⏳ partial · ❌ missing · ❓ live behaviour not captured yet (requested).
Evidence tags: L = live behaviours.md line, R = replay, U = unit test,
B = Onshape client code (bundle-raw), P = an existing parity row.

## 1. Selection (Part Studio graphics)

| # | Behaviour | Live / source | Status |
|---|---|---|---|
| 1.1 | Click a face / edge / vertex / plane / sketch entity selects it; a plain click **adds** to the selection (no modifier needed) | L263; P part-studio 3.1 | 🔧 (P ✅; replay pending) |
| 1.2 | Click on an already selected entity: removes it? (sketch: a plain second click toggles off, L14084) | L14084 (sketch); Part Studio graphics ❓ | ❓ |
| 1.3 | Shift+click / Ctrl+click (Cmd on macOS) on an entity in the graphics | not captured (only sketch, Parts list, feature list, assembly) | ❓ |
| 1.4 | Click in empty space clears the selection — except while Mass properties or a pinned Measure panel is open | L263, L401 | 🔧 |
| 1.5 | Space clears the selection; Ctrl+A does nothing | L1199; P 1.8 | 🔧 |
| 1.6 | Esc: does it clear the selection in the Part Studio? (it does not in the sketch L189, the assembly L1325, the lists L14045) | ❓ | ❓ (P 1.7 says it clears: unverified) |
| 1.7 | A part face is picked through planes; a plane only where no part is under the cursor | L406; P 13.8 | ✅ P |
| 1.8 | Left→right drag: window box (only what is fully inside); right→left: crossing box (everything touched, planes included); a box is a window whenever it ends right of its start | L345–347; B `performContainmentSelection` | 🔧 (planes excluded from our box: gap) |
| 1.9 | Crossing box pre-highlights what it touches while dragging | L347 | ❌ |
| 1.10 | Box look: window fill/border, crossing border dashed (bundle: `CROSS_SELECT_COLOR #FFC20E`, stipple 9/8; infill opacity 0.2) | L346–347; B | ⏳ (ours #789ad1/#ffdc74 at 0.5) |
| 1.11 | Ctrl(+Cmd)+drag box deselects | P 3.6 | 🔧 |
| 1.12 | Selection colours: face #e5aa38 + orange boundary, selected+hovered #f0bb58, edge #ffa90a ~3 px, part #c09039..#f0b850, plane fill #fdd385 / border #ffa90a | L256–263 | 🔧 (P) |
| 1.13 | Selection-count cursor macSelectionCursor-N, -5plus from 6 | L1100–1111 | ✅ P cursors 35/36 |
| 1.14 | Left drag threshold 2 px / 50 ms (`LEFT_MOUSE_DRAG_DISTANCE_TOLERANCE`, `_TIME_TOLERANCE`) | B | 🔧 |

## 2. Selection in sketch mode and assemblies

| # | Behaviour | Live / source | Status |
|---|---|---|---|
| 2.1 | Sketch: plain click adds, a second click toggles off, Shift+click adds, empty click clears, Esc does not clear | L189, L14084 | ✅ P sketch 9.x |
| 2.2 | Sketch: window / crossing boxes over sketch entities | P sketch | ✅ P |
| 2.3 | Assembly: plain clicks add; Shift / Ctrl+click on a selected face change nothing; empty click clears; double-click a face clears | L4599–4644 | 🔧 (ours toggles off on a plain click: check) |
| 2.4 | Assembly window box: faces / edges / vertices fully inside; crossing box: whole instances | L4599–4644 | 🔧 |
| 2.5 | Assembly box without a modifier: replaces or adds to the selection? (ours clears first) | ❓ | ❓ |
| 2.6 | Assembly drag starting on a part moves the instance | L4599–4644 | 🔧 (P) |

## 3. Hover pre-highlight

| # | Behaviour | Live / source | Status |
|---|---|---|---|
| 3.1 | Hovered face: boundary only, #FFC685 ~2 px, no fill (light and dark) | L257, L11317 | 🔧 |
| 3.2 | Hovered edge: thickened #FFC685; a 9 px dark hollow ring at its midpoint; near the midpoint (a few px) the midpoint becomes the target (11 px orange ring) | L258, L14599–14612 | ⏳ (ours switches to the midpoint only on click, P 3.10) |
| 3.3 | Hovered vertex: a light-orange dot | L259 | 🔧 |
| 3.4 | Hovered plane / mate connector colours | B only (`SECTION_PLANE…`, `MATE_CONNECTOR_PRE_SELECTED_COLOR`) | ❓ |
| 3.5 | Timing: the pick runs at once on every move unless "reduce mouse hover highlight" is on (then after min(5 × average pick time, 250 ms) once the mouse pauses, or at once below 0.15 px/ms) | B `schedulePreHighlightPick`, `onMouseMove` | 🔧 (ours: at once) |
| 3.6 | Leaving the graphics area clears the pre-highlight | B `onMouseLeave` | 🔧 |
| 3.7 | Hovering a feature row outlines its geometry | L260 | 🔧 P |
| 3.8 | Sketch hover #ffc685 / mirrored pair secondary #f2ff80 | L8776–8813 | ✅ P sketch |

## 4. Measure (status bar readout and panel)

| # | Behaviour | Live / source | Status |
|---|---|---|---|
| 4.1 | Readout per selection: labels and order (Parallel dist, Angle, Perp …, Center dist, Min/Max dist, Diameter, Radius, Center, Point, Area, Length, Curvature deviation) | L399–403; B `setMeasurements` priorityTypes | 🔧 (P 11.1 ✅ partly) |
| 4.2 | Point–point, edge–face, face–face, min/max distance values equal `query_measure` | L4611, L4618 | 🔧 |
| 4.3 | Default planes add only angles and distances; a plane alone: "No measurements available for current selection." | L8938–8960; U `default_planes_add_only_angles_and_distances` | ✅ U |
| 4.4 | A vertex or edge with a plane (AN1) | workflow-analysis AN1 ✗ | ❌ |
| 4.5 | Units follow the document (mm document: Millimeter) | L14555 (L401 was an inch document) | 🔧 |
| 4.6 | Status-bar Point readout with red / green / blue axis arrows | L14555–14574 | ❓ (no parity row) |
| 4.7 | Hovering a distance row draws a black dashed line | L8957 | 🔧 |
| 4.8 | Live update when the selection changes; value display restored after 3 s (`MEASUREMENT_VALUE_DISPLAY_RESTORE_TIME_MS`) | B | ❓ |
| 4.9 | Mesh parts: an asterisk marks approximate values | L947, L953 | 🔧 |

## 5. Mass properties

| # | Behaviour | Live / source | Status |
|---|---|---|---|
| 5.1 | Panel 440 px, Part / Face tabs, values = `query_mass_properties` | L403 | 🔧 (P 11.2) |
| 5.2 | No material: red chip, blank mass / CoM / inertia; a one-time toast "One or more parts do not have a material defined." 3.6 s after opening, hidden 4.6 s later | L6287–6318 | 🔧 |
| 5.3 | Face tab: Surface area, Perimeter, Lxx/Lxy/Lyx/Lyy, Lzz; non-coplanar faces blank and a red title | L2723–2735 | 🔧 (P 11.5) |
| 5.4 | Reference mate connector; Mass Override | L8850 | ❌ (P open) |
| 5.5 | Esc closes Measure and Mass | L6327 | 🔧 |

## 6. Section view

| # | Behaviour | Live / source | Status |
|---|---|---|---|
| 6.1 | Dialog: Exclude / Include tabs, "Section plane 1 (Top)", prompt "Select a plane, planar face, cylinder, cone, or mate connector." | L358, L4955 | ✅ P 10.7 |
| 6.2 | Part Studio manipulator: base ring, arrow (hollow head), two rotate handles; hover blue ~#4573AE, cursor default | L14576–14598 | ❌ (arrow only) |
| 6.3 | Arrow drag shows a value box (75×26, 1 decimal while dragging, from the drag start); Esc closes the box, a second Esc cancels | L11990–12030 | 🔧 |
| 6.4 | Rotate handles turn in 15° steps ("15 deg") | L11990–12030 | ❌ Part Studio |
| 6.5 | Dragging the base snaps to circle centres, faces, edges (~30–35 px) and vertices (~22 px) | L14009–14028 | ❌ |
| 6.6 | Caps: part colour with view-dependent lighting, hatch 0.5× the fill; while the dialog is open caps blend toward #FFA90A @0.8; excluded instances uncut and outlined | L12207–12230, L12291 | ⏳ (flat 0.9×) |
| 6.7 | With a feature dialog open, Shift+X does nothing and the camera menu has no Section view… | L4955–4961 | 🔧 |
| 6.8 | A plane feature row accepted as a section plane (ADV1) | workflow-analysis ADV1 ✗ | ❌ |
