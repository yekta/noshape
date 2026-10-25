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
| 1.8 | Left→right drag: window box (only what is fully inside); right→left: crossing box (everything touched, planes included); a box is a window only while it ends strictly right of its start (`point1.x > point0.x`; a vertical box is crossing), a two-finger touch box always a window. B: `Viewer.performBoxSelection` runs on release only, picking the rect again and again with what it found hidden (back faces on), so the box also takes **occluded** entities; a window first hides everything crossing its border; Ctrl with an empty selection does nothing | L345–347; B `performContainmentSelection`, `performBoxSelection` | 🔧 (box_select.rs: from the projected display geometry, occluded entities and planes included; which kinds a box keeps waits for live 10-25 c) |
| 1.9 | Crossing box pre-highlights what it touches while dragging | L347; B: BoxSelector draws only the box while dragging (no pick) | ❓ (live note vs bundle disagree; mid-drag capture requested, 10-25 c) |
| 1.10 | Box look (BoxSelector.updateGraphics): a 1 px loop from the press point, each side stippled from its first corner, over an infill of the line colour at 0.2. Window `--os-box-select-color` (light #1651b0, dark #769fcc) solid; crossing #FFC20E 9 on / 8 off; zoom window rgba(144,206,241,0.7) 10 / 10. Live over white: fill #d0dcef / #fff3cf (= 0.2 exactly), line #7296cf / #ffda6e (= 0.6 of the colour) | L346–347; B | 🔧 (viewport.rs `box_overlay`) |
| 1.11 | Ctrl(+Cmd)+drag box deselects | P 3.6 | 🔧 |
| 1.12 | Selection colours: face #e5aa38 + orange boundary, selected+hovered #f0bb58, edge #ffa90a ~3 px, part #c09039..#f0b850, plane fill #fdd385 / border #ffa90a | L256–263 | 🔧 (P) |
| 1.13 | Selection-count cursor macSelectionCursor-N, -5plus from 6 | L1100–1111 | ✅ P cursors 35/36 |
| 1.14 | Left drag threshold 2 px / 50 ms (`LEFT_MOUSE_DRAG_DISTANCE_TOLERANCE`, `_TIME_TOLERANCE`) | B | 🔧 |

Replay: `.github/nav/linux-replay.sh <noshape> .github/nav/block.noshape <out> wayland|x11 interact`
(interact.py drives clicks, boxes, hover and keys with uinput; interact_check.py asserts the traced
selection and hover per step, rows in brackets).

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
| 4.4 | A vertex or edge with a plane (AN1): Top + vertical edge reads Perp line and plane angle · Curvature deviation · Min dist · Length (the edge's); vertex / edge midpoint / circle read their Min dist | L8949 (c); U `edge_vertex_midpoint_and_circle_with_the_top_plane` | ✅ U (edge rows exact; vertex / circle rows beyond Min dist not captured live) |
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
| 6.2 | Part Studio manipulator: base ring at the picked face's centre, arrow (hollow head), two rotate handles; only the hovered handle blue ~#4573AE; the glyph's centre highlights nothing; cursor default | L14576–14598 | 🔧 (section_manip.rs, the assembly's glyph) |
| 6.3 | Arrow drag shows a value box (75×26, 1 decimal while dragging, from the drag start); Esc closes the box, a second Esc cancels | L11990–12030 | 🔧 |
| 6.4 | Rotate handles: ring 0 about the plane's Y, ring 1 about its X, through the base; 5° multiples catch the drag near them, else free (up to 3 decimals, "deg") | L12023–12028 | 🔧 (U `ring_drags_catch_five_degree_multiples`, `rotating_turns_the_plane_about_its_base`) |
| 6.5 | Dragging the base snaps to circle centres, faces, edges (~30–35 px, over the face) and vertices (~22 px); what it snaps to lights up; on release the plane moves through the snapped point and the base returns to its place | L14009–14028 | 🔧 (snapped look: the mate connector disc in the section plane; live turns it with the entity) |
| 6.6 | Caps: part colour with view-dependent lighting, hatch 0.5× the fill; while the dialog is open caps blend toward #FFA90A @0.8; excluded instances uncut and outlined | L12207–12230, L12291 | ⏳ (flat 0.9×) |
| 6.7 | With a feature dialog open, Shift+X does nothing and the camera menu has no Section view… | L4955–4961 | 🔧 |
| 6.8 | A plane feature row accepted as a section plane (ADV1) | workflow-analysis ADV1 ✗ | ❌ |

## Handover (nav2 → psb, 10-25 15:45)

nav2 was paused; psb takes this list. State per area, with the code locations found:

Only the trace hook is in the ready batch. Everything else below (box overlay and box_select.rs, the section
manipulator, the replay suite, the AN1 test) is the **WIP commit on agent/nav2** after it: clippy-clean
(target-qa, 15:35) but its unit tests had not finished running; cherry-pick and test before taking it.

- **Trace hook** (✅ committed): `nav_trace::highlights` (crates/app/src/nav_trace.rs), called once in
  part_studio/mod.rs `refresh_scene` before `scene_builder::with_highlights`; writes `{"highlights": {studio,
  selection, hover}}` lines on change. The assembly (assembly_view.rs) and menus.rs one-liners are cleared
  but not added.
- **Replay suite** (rows 1.1, 1.4, 1.5, 1.8, 3.1, 3.6): `.github/nav/interact.py` + `interact_check.py`, run
  `linux-replay.sh <bin> .github/nav/block.noshape <out> wayland interact`. **Never run yet**: first run may
  need its click points tuned (`project()` assumes the orthographic default view).
- **Box select** (1.8–1.10): Onshape `Viewer.performBoxSelection` (bundle-raw webgl.*.js) re-picks the rect
  with found ids hidden, so occluded entities are taken too. Ours: viewport.rs `finish_box` → `PickPurpose::Box`
  → `ViewportEvent::BoxSelect { ids, modifiers, crossing, rect }` (the GPU `box_ids` are now ignored by the
  Part Studio); part_studio/mod.rs BoxSelect arm → `box_select.rs::box_ids` (CPU, projected display
  geometry: faces/edges/vertices/planes/sketch curves, points, regions) → behaviors.rs `box_select` (now keeps
  planes; still drops Origin, and Vertex/SketchRegion outside dialogs). Look: viewport.rs `box_overlay`
  (BoxStyle Window/Crossing/Zoom). `PickMap` (scene/src/builder.rs: `get`, `ids_for`, `priority_fn_filtered`)
  maps references ↔ pick ids; `sync_pick_filter` (behaviors.rs) sets the viewport's pick priority from the
  active field's kinds. **Open:** which kinds a box keeps (vertices? the whole part?) and whether a crossing
  box pre-highlights during the drag (bundle: no) — live request "nav2 (P1) (10-25 c)", the live agent was
  mid-run (live/selection/s6*, nav2-sel-*.json). The assembly box (assembly_view.rs ~L2640) is unchanged.
- **Section view** (6.2, 6.4, 6.5): part_studio/section_manip.rs (`SectionManip`, `section_manip_event`,
  `section_snap`, `section_glyph`), hooked as `Manip::Section` in manipulators.rs (`current_manip`,
  `sync_manipulator`, `manipulator_event`, `ManipGeometry::build`); state in `SectionDialog::manip`
  (section.rs); `SectionPlane::x` added; the glyph is the assembly's (`asm_scene::push_section_glyph`, now
  pub with a pick-id closure; `push_connector_glyph` pub for the snapped disc). The viewport keeps hover
  picks during a base drag (`Viewport::tool_hover`). The offset box moved to +40/−35 px from the tip.
  **Open:** live-verify in the app; the value box is always shown (live: only after a drag, 1 decimal, from
  the drag start — the assembly's `SectionValue` does this); the chip keeps "(Top)" after a rotation; the
  snapped disc lies in the section plane (live turns it with the entity).
- **Measure AN1** (4.4): already worked in the model (`measure::resolve_shapes` plane faces, `measure`
  plane filter, model/src/regen/measure.rs); pinned by `edge_vertex_midpoint_and_circle_with_the_top_plane`.
- **Not started:** pre-highlight timing (3.5, live request "(10-25 d)"), mass properties rows (5.x), the
  midpoint hover (icons2's).
