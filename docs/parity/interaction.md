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

| # | Behaviour | Live / source | Status | Verified |
|---|---|---|---|---|
| 1.1 | Click a face / edge / vertex / plane / sketch entity selects it; a plain click **adds** to the selection (no modifier needed) | L263; P part-studio 3.1 | ✅ R (CI replay 10-25, Wayland and X11: click-top, click-front-adds) | 0.4.10+ / 2a6d0167 10-27 |
| 1.2 | A plain click on a selected entity toggles it off (the pointer still hovers it, so its outline stays) | live "nav2 selection" 1 | ✅ R click-selected-toggles (`pick` toggles; Wayland + X11, run 40306609140) | 0.4.10+ / f16b31c6 10-27 |
| 1.3 | Shift+click and Meta+click act as a plain click (toggle). **Ctrl+click opens the graphics context menu on release and leaves the selection** (`eventCanBeUsedForContextMenu` = ctrlKey or right button, every platform) | live "nav2 selection" 3 | ✅ (viewport.rs: Ctrl+left release → context menu; checked in the app 10-25; R shift-empty-keeps) | 0.4.10+ / 2a6d0167 10-27 |
| 1.4 | Click in empty space clears the selection, **a Shift+click there keeps it**; neither clears while Mass properties or a pinned Measure panel is open | L263, L401; live "nav2 selection" 3 | ✅ (part_studio/mod.rs Click arm; checked in the app 10-25; R shift-empty-keeps) | 0.4.10+ / 2a6d0167 10-27 |
| 1.5 | Space clears the selection; Ctrl+A does nothing | L1199; P 1.8 | ✅ R (space-clears) | 0.4.10+ / 2a6d0167 10-27 |
| 1.6 | **Esc does not clear the selection** in the Part Studio (graphics focused); Space does | live "nav2 selection" 4 | ✅ (part_studio/mod.rs Cancel: isolate only; checked in the app 10-25; R shift-empty-keeps) | 0.4.10+ / 2a6d0167 10-27 |
| 1.7 | A part face is picked through planes; a plane only where no part is under the cursor | L406; P 13.8 | ✅ P | 0.4.10+ / 2a6d0167 10-27 |
| 1.8 | Left→right drag: window box (only what is fully inside); right→left: crossing box (everything touched); a window only while it ends strictly right of its start (`point1.x > point0.x`; a vertical box is crossing), a two-finger touch box always a window. Picks on release, back faces on, so **occluded** faces and edges are taken. A Part Studio box takes **faces and edges only: no vertices, part, default planes (enclosed, crossed or over their border), Origin or mate connector** (window round the block → 6 faces + 12 edges; crossing over a corner → 3 faces + 3 edges incl. the hidden bottom face). A box adds, never toggles, and an empty box keeps the selection | L345–347; live "nav2 selection" 5, 6; B `performBoxSelection` | ✅ (box_select.rs from the projected display geometry; behaviors.rs `box_takes`, test `box_takes_no_planes_origin_or_connectors`; window round two cubes takes their faces and edges, not the planes, in the app 10-25) | 0.4.10+ / 77870dd4 10-28 |
| 1.9 | **No pre-highlight while a box is dragged** (only the plane under the press point keeps its ordinary hover); the selection fills in after release | live "nav2 selection" 5 | ✅ (ours draws only the box) | 0.4.10+ / 2a6d0167 10-27 |
| 1.10 | Box look (BoxSelector.updateGraphics): a 1 px loop from the press point, each side stippled from its first corner, over an infill of the line colour at 0.2. Window `--os-box-select-color` (light #1651b0, dark #769fcc) solid; crossing #FFC20E 9 on / 8 off; zoom window rgba(144,206,241,0.7) 10 / 10. Live over white: fill #d0dcef / #fff3cf (= 0.2 exactly), line #7296cf / #ffda6e (= 0.6 of the colour) | L346–347; B | ✅ (viewport.rs `box_overlay`: the line at 0.5 over the 0.2 infill composites to live's 0.6) | 0.4.10+ / f16b31c6 10-27 |
| 1.11 | Ctrl+drag box deselects what it encloses / touches; with nothing selected it does nothing; looks as a plain box | P 3.6; live "nav2 selection" 6 | ✅ R ctrl-box-deselects, ctrl-box-nothing (run 40306609140); the box's look: P 3.6 | 0.4.10+ / f16b31c6 10-27 |
| 1.12 | Selection colours: face #e5aa38 + orange boundary, selected+hovered #f0bb58, edge #ffa90a ~3 px, part #c09039..#f0b850, plane fill #fdd385 / border #ffa90a | L256–263 | ✅ (psui 10-27, app vs live "psui (10-27, interaction 1.12)" light set: top at rest (140,185,212) vs live (142,187,214), selected (229,163,34) exact, selected + hovered the same as selected; a selected face's boundary a 3 px (255,169,10) band; a selected edge 5 px (255,169,10) → (203,138,15) at the centre) |  |
| 1.13 | Selection-count cursor macSelectionCursor-N, -5plus from 6 | L1100–1111 | ✅ P cursors 35/36 | 0.4.10+ / dd1b5037 10-27 (XFixes read-back: "1", "2", "5+" 32 px hot 3,3) |
| 1.14 | Left drag threshold 2 px / 50 ms (`LEFT_MOUSE_DRAG_DISTANCE_TOLERANCE`, `_TIME_TOLERANCE`) | B | ✅ R (nav.yml run 40339384558 on main: jitter-click-clears, small-drag-keeps; 18/18 on Wayland and X11) |  |

Replay: `.github/nav/linux-replay.sh <noshape> .github/nav/block.noshape <out> wayland|x11 interact` (in the "Navigation replay" workflow's Linux job since 10-25; run 40235651854: 8/8 re-checked locally with the box check as live (no plane); run 40252517286 on CI with that check: 8/8 on Wayland and X11)
(interact.py drives clicks, boxes, hover and keys with uinput; interact_check.py asserts the traced
selection and hover per step, rows in brackets).

## 2. Selection in sketch mode and assemblies

| # | Behaviour | Live / source | Status | Verified |
|---|---|---|---|---|
| 2.1 | Sketch: plain click adds, a second click toggles off, Shift+click adds, empty click clears, Esc does not clear | L189, L14084 | ✅ P sketch 9.x | 0.4.10+ / f16b31c6 10-27 |
| 2.2 | Sketch: window / crossing boxes over sketch entities | P sketch | ✅ P | 0.4.10+ / f16b31c6 10-27 |
| 2.3 | Assembly: plain clicks add across instances; a plain click on a selected face toggles it off (not on the triad's centre); empty click clears | L4599–4644; live "nav2 selection" 7 | 🔧 (assembly_view.rs `click_select` toggles, as live) |  |
| 2.4 | Assembly box (only with nothing selected) selects **whole part occurrences**: a crossing box those it touches, a window only those wholly inside it | live "nav2 selection" 7, ui (10-26) ui-asmbox/03–05 (supersedes L4599–4644's faces-in-window) | ✅ in the app 10-26: crossing over part of <2> → the whole occurrence; window over part of <2> → nothing, around all of <2> → <2> only (assembly_view.rs BoxSelect: every mesh point of the occurrence inside the box) | 0.4.10+ / f16b31c6 10-27 |
| 2.5 | Assembly: with something selected, a left drag from empty space **moves the selected instance(s)** with the pointer (no box) | live "nav2 selection" 7 | ✅ in the app 10-26 (<2> selected, a drag from empty space moved it and it stayed; assembly_view.rs `start_selection_drag`: the selection's movable instances follow the pointer in the view plane through their bounds' centre, solved as drags; a click there still clears) | 0.4.10+ / f16b31c6 10-27 |
| 2.6 | Assembly drag starting on a part moves the instance | L4599–4644 | 🔧 (P) |  |

## 3. Hover pre-highlight

| # | Behaviour | Live / source | Status | Verified |
|---|---|---|---|---|
| 3.1 | Hovered face: boundary only, #FFC685 ~2 px, no fill (light and dark) | L257, L11317 | ✅ (psui 10-27: the face fill unchanged, its boundary a flat 3 px #FFC685 band, as live psui-light-02; shaders.wgsl `face_band`) | 0.4.10+ / 2a6d0167 10-27 |
| 3.2 | Hovered edge: thickened #FFC685; a 9 px dark hollow ring at its midpoint; near the midpoint (a few px) the midpoint becomes the target (11 px orange ring) | L258, L14599–14612; behaviours "icons2 midpoint radius" | ✅ (psui 10-27, app vs live selection/06, 06b: the edge lit #FFC685 with the dark midpoint ring; with no dialog the edge stays the target at the midpoint, as live; the midpoint target only with a field that takes points) | 0.4.10+ / 2a6d0167 10-27 |
| 3.3 | Hovered vertex: a light-orange dot | L259 | ✅ (psui 10-27: the 11 px disc #F8C488, centre #FFD8AC, as palette POINT_HOVER from live part-studio/qa4-r5-s9-6d) | 0.4.10+ / f16b31c6 10-27 |
| 3.4 | Hovered plane / mate connector colours | B only (`SECTION_PLANE…`, `MATE_CONNECTOR_PRE_SELECTED_COLOR`) | ⏳ (psui 10-27, live "psui (2026-10-27, P2) interaction 3.4": the mate connector disc uses Onshape's constants: rest white 0.8, hover / selected amber rgba(255,194,14) 0.4 / 0.5 instead of the white, ring 4 px of a 36 px disc; on :102 over a face rest (232,241,246) / live (232,241,247), hover (186,189,133) / (187,190,134). a selected connector's owner part tinted yellow-green (its colour 0.6 toward MATE_CONNECTOR_SECONDARY): top face (186,211,153) / live (186,214,166), wedge (221,202,83) / (220,204,90); hovered planes #FFC685 3 px as live) |  |
| 3.5 | Timing ("reduce mouse hover highlight" on): below 0.15 px/ms the pick runs on every move; faster, the old highlight clears (unless still under the pointer) and the pick waits min(5 × average pick ms, 250 ms) after the pointer pauses (average = EMA 0.8/0.2 of pick times) | live "nav2 pre-highlight" 1–2 | 🔧 (viewport.rs `prehighlight_delay`, GPU hover-pick time as the average; plain selection only; test `prehighlight_waits_for_a_pause_as_live`) |  |
| 3.6 | Leaving the graphics area clears the pre-highlight at once (next frame) | live "nav2 pre-highlight" 4; B `onMouseLeave` | ✅ R (hover-leave) |  |
| 3.7 | Hovering a feature row outlines its geometry | L260 | ✅ (psui 10-27, app vs live selection/10: the part outlined #FFC685, the row highlighted, tooltip "Extrude 1 <- Sketch 1" right of the row) | 0.4.10+ / f16b31c6 10-27 |
| 3.8 | Sketch hover #ffc685 / mirrored pair secondary #f2ff80 | L8776–8813 | ✅ P sketch |  |

## 4. Measure (status bar readout and panel)

| # | Behaviour | Live / source | Status | Verified |
|---|---|---|---|---|
| 4.1 | Readout per selection: labels and order (Parallel dist, Angle, Perp …, Center dist, Min/Max dist, Diameter, Radius, Center, Point, Area, Length, Curvature deviation) | L399–403; B `setMeasurements` priorityTypes; dom/measure-readouts-final.txt | 🔧 (the order per case follows the live capture, model-core regen::measure, U `readout_rules_for_face_pairs`; app 10-27: two parallel faces → Parallel dist · Area · Min dist · Max dist = live case (1); the other cases not yet side by side); psui 10-27: all 14 "psb measure" cases give live's rows in live's order, after the fix that a perpendicular edge reads Curvature deviation only where it meets the face (5a)) |  |
| 4.2 | Point–point, edge–face, face–face, min/max distance values equal `query_measure` | L4611, L4618 | ✅ (psui 10-27: live's "psb measure" part rebuilt with noshape-cli (/root/psui-scratch/meas/mk*.py), its cases (1)–(10) through `query measure`, the app's measure: every value equals live's (40 mm, 2.981 in², 74.83 mm, 135°, 7.071 mm, 63.44 mm, 30 / 22 / 42.942 mm, 15 / 32.727 / 8 mm, 16 / 20 / 56.877 mm, 8 mm …); X/Y/Z signs differ only where the max distance ties between two pairs) | 0.4.10+ / f16b31c6 10-27 |
| 4.3 | Default planes add only angles and distances; a plane alone: "No measurements available for current selection." | L8938–8960; U `default_planes_add_only_angles_and_distances` | ✅ U | 0.4.10+ / 2a6d0167 10-27 |
| 4.4 | A vertex or edge with a plane (AN1): Top + vertical edge reads Perp line and plane angle · Curvature deviation · Min dist · Length (the edge's); vertex / edge midpoint / circle read their Min dist | L8949 (c); U `edge_vertex_midpoint_and_circle_with_the_top_plane` | ✅ U (edge rows exact, Curvature deviation also for a perpendicular line; vertex / circle rows beyond Min dist not captured live) | 0.4.10+ / dd1b5037 10-27 (pin plate vertical edge + Top: Perp line and plane angle 90, Curvature deviation 0.00 %, Min dist 0, Length 5) |
| 4.5 | Units follow the document (mm document: Millimeter) | L14555 (L401 was an inch document) | ✅ (psui 10-27: mm document → "40.000 mm") | 0.4.10+ / 2a6d0167 10-27 |
| 4.6 | Status-bar Point readout with red / green / blue axis arrows ("Point:  X↘ 40.000 mm  Y↗ 0.000 mm  Z↑ 10.000 mm") | L14555–14574; live part-studio/qa4-r5-s9-4, ui-mass/07 | ✅ (psui 10-27, app vs live part-studio/qa4-r5-s9-4: "Point: X↘ … mm Y↗ … Z↑ …", the same arrows, colours, underlined values) | 0.4.10+ / f16b31c6 10-27 |
| 4.7 | Hovering a distance row draws a black dashed line | L8957 | ✅ (10-27 on :102: Min dist row → the dashed line between the closest points) | 0.4.10+ / 2a6d0167 10-27 |
| 4.8 | Live update when the selection changes; value display restored after 3 s (`MEASUREMENT_VALUE_DISPLAY_RESTORE_TIME_MS`) | B | 🔧 (readout and panel follow the selection: menus.rs `measure_cache` keyed on it). The 3 s constant (3e3) is only defined in the loaded bundles; nothing reads it there, so no behaviour to copy yet |  |
| 4.9 | Mesh parts: an asterisk marks approximate values | L947, L953 | 🔧 |  |

## 5. Mass properties

| # | Behaviour | Live / source | Status | Verified |
|---|---|---|---|---|
| 5.1 | Panel 440 px, Part / Face tabs, values = `query_mass_properties` | L403 | ✅ (psui 10-27 on :102 against live "Assembly Mass and section properties" (skb 10-20): a 40×20×10 steel block (noshape-cli) gives Mass 0.138 lb, Volume 8000 mm³, Surface area 2800 mm², CoM 20 / 10 / 5 mm, Lxx 5.769, Lyy 19.614, Lzz 23.075, off-diagonals 0, exactly live's) | 0.4.10+ / 2a6d0167 10-27 |
| 5.2 | No material: red chip, blank mass / CoM / inertia; a one-time toast "One or more parts do not have a material defined." 3.6 s after opening, hidden 4.6 s later | L6287–6318 | ✅ (psui 10-27: the red "Part 1" chip, mass / CoM / inertia blank, CoM and inertia Override greyed, the toast at the top centre, as live "qa4 F18/F19") | 0.4.10+ / 2a6d0167 10-27 |
| 5.2b | On the Part tab a face / edge / vertex click takes the whole part (shaded part amber, its Parts row selected); on the Face tab the face ("Face of Extrude 1", face amber) | behaviours "psui (10-27, interaction 5.x)" | ✅ Part tab (psui 10-27; a selected part's shading refit: top (239,182,72) / front (192,146,57) / right (227,172,68) vs live (243,183,72) / (192,144,57) / (224,169,66)) | 0.4.10+ / f16b31c6 10-27 |
| 5.3 | Face tab: Surface area, Perimeter, Lxx/Lxy/Lyx/Lyy, Lzz; non-coplanar faces blank and a red title | L2723–2735 | ✅ (psui 10-27 on :102, same block: the 40×20 top face gives 800 mm², 120 mm, Lxx 26666.667, Lxy / Lyx 0, Lyy 106666.667, Lzz 133333.333; adding a side face blanks every value, the title turns red with the tooltip "Face selections are not coplanar"; no Override boxes; all as live) | 0.4.10+ / f16b31c6 10-27 |
| 5.4 | "Mate connector for reference frame": a field of its own (click it, then a connector's row or glyph; × clears); with it the centre of mass reads in the connector's frame (block centre 20/10/5 from a connector at its corner (40,20,10): −20/−10/−5) and the inertia is about the connector's origin in its axes, products as tensor entries (Lxy −m·dx·dy); the CoM and inertia Overrides grey out. Mass ☑ Override: ticking keeps the mass and doesn't focus the box; a typed mass + Enter rescales the inertia at once; ✓ (solid once something changed) stores it on the part, unticking restores the computed mass | live "ui (10-25 d) Part Studio Mass properties" (ui-mass/05–15) | ✅ in the app 10-25 (menus.rs `mass_reference`, `mass_override_edit`, `accept_mass_dialog`; numbers to 3 decimals, trailing zeros dropped, tiny ones in e-notation (`mass_number`); Override 1 → inertia 41.667 / 141.667 / 166.667 as live, kept after ✓). Values in the document's mass unit (Workspace units › Mass; inertia "(lb mm²)"), the typed bare number too; a connector on a vertex is world-aligned as live (refgeom.rs `vertex_csys`, test `mate_connector_on_a_vertex_is_world_aligned`) |  |
| 5.5 | Esc closes Measure and Mass | L6327 | ✅ (10-27 on :102: [ opens Measure, Esc closes it; the Mass properties button opens Mass, Esc closes it) | 0.4.10+ / 2a6d0167 10-27 |

## 6. Section view

| # | Behaviour | Live / source | Status | Verified |
|---|---|---|---|---|
| 6.1 | Dialog: Exclude / Include tabs, "Section plane 1 (Top)" (a chip is named by the plane's nearest default plane, whatever was picked: section.rs `direction_name`), prompt "Select a plane, planar face, cylinder, cone, or mate connector." | L358, L4955 | ✅ P 10.7 | 0.4.10+ / f16b31c6 10-27 |
| 6.2 | Part Studio manipulator: base ring at the picked face's centre, arrow (hollow head), two rotate handles; only the hovered handle blue ~#4573AE; the glyph's centre highlights nothing; cursor default | L14576–14598 | ✅ (look checked 10-27 against live qa4-r5-s9-5: ring at the picked face's centre, shaft, hollow arrowhead, the two-handle rotate glyph at the tip; hover colours not yet; section_manip.rs; the base starts at the picked face's area centroid or the connector's origin, `SectionPlane::base`; a default or feature plane at its origin, live ui-snapv/25) 10-27 psui: hovering the arrow turns only the shaft and head blue, as live; 10-27 23:50 psui, side by side with live qa4-r5-s9-5b–5f on :102: the ring, the arrow and each rotate handle light alone, the glyph's centre lights nothing (`RING_DEAD_PX`, T1 test section_rotate_handles_and_their_dead_centre), no face / plane / edge midpoint pre-highlights under a handle, the rotate glyph as live's (rings ≈14 px touching the apex, axis lines ≈44 px, edges (147,155,160) / core (246,247,247)); the shared arrow's stroke colours asked of live (REQUESTS psui 10-27 6.2)) |  |
| 6.3 | Arrow drag shows a value box (75×26, 1 decimal while dragging, from the drag start); Esc closes the box, a second Esc cancels | L11990–12030 | ✅ (psui 10-27 on :102, lpd Top face: the drag shows the 75×26 box upper right of the tip, "-10.7 mm" selected, 1 decimal, minus downwards, the start mark; Esc closes only the box (was ignored inside the field until this commit), a second Esc cancels the section) | 0.4.10+ / f16b31c6 10-27 |
| 6.4 | Rotate handles: ring 0 about the plane's Y, ring 1 about its X, through the base; 5° multiples catch the drag near them, else free (up to 3 decimals, "deg") | L12023–12028 | ✅ (U `ring_drags_catch_five_degree_multiples`, `rotating_turns_the_plane_about_its_base`; psui 10-27 on :102: hovering a ring lights only it and its axis line blue, its drag shows "40 deg" in the box and turns the plane) |  |
| 6.5 | Dragging the base snaps to circle centres, faces, edges (~30–35 px, over the face) and vertices (~22 px); what it snaps to lights up; on release the plane moves through the snapped point and the base returns to its place | L14009–14028 | ✅ in the app 10-26 (top edge lit with the connector disc under the ring, bottom edge, release → hatched cap at z 0); earlier 10-25: edge (bottom edge lit), vertex (lit) and release through the snapped point (plane z 10 → 0) as live; on-plane geometry is now kept (shaders.wgsl `clipped`), so an in-plane edge lights and the base cap shows; the snapped base draws live's implicit connector disc under the ring (asm_scene `push_connector_glyph`), in the snapped entity's frame as live ui-snapv 10-26: a face's normal (also on a Front section plane the top face's disc lies flat, the x=40 face's stands with Z=+X, X=+Y), a circle's axis, a vertex world-aligned; a default plane's base starts at its origin; while the base is over a face, its corners and edge midpoints show white dots and its centroid a quartered black-and-white disc, all lying in the face (live ui-snapv/26–28) (section_manip.rs `section_snap`, ported from nav2 onto psb's manipulator: hover picks continue during the base drag (`Viewport::tool_hover`), only the snapped entity lights; the snapped look (pale-yellow disc + triad) not drawn yet: needs `asm_scene::push_connector_glyph` public) |  |
| 6.6 | Caps: 0.9 × (part colour + id offset), hatch lines 0.5× the fill, direction and spacing from a hash of that colour (Onshape's endcap shader); while the dialog is open 0.2·cap + 0.8·#FFC685; geometry lying on the plane is kept (plane on a top face: no cap; through the bottom face: capped) | live ui (10-26) (ui-cap/04–10), icons2 caps 3, kern G40/G44 | ✅ shader port (shaders.wgsl, icons2/perf); per-part id 230 + 4·index as live's (reproduces icons2's three caps: 131.0° / 125.9° / 19.4°, 15.9 / 8.7 / 17.6 px); on-plane geometry kept (`clipped`) | 0.4.10+ / f16b31c6 10-27 |
| 6.7 | With a feature dialog open, Shift+X does nothing and the camera menu has no Section view… | L4955–4961 | ✅ (10-27 under Extrude 4: Shift+X opens nothing; the camera menu ends at Highlight boundary edges, Previous view greyed, as live) | 0.4.10+ / f16b31c6 10-27 |
| 6.8 | A plane feature row accepted as a section plane (ADV1) | workflow-analysis ADV1; live 10-27 (behaviours "Section view": "Plane mid" row → "Section plane 1 (Right)") | ✅ (10-27 on :102: Plane 1 (Right + 10 mm) row → chip "Section plane 1 (Right)", ✓ cuts at x = 10 with hatched caps) | 0.4.10+ / dd1b5037 10-27 (Plane 1 row → "Section plane 1 (Right)" = live, cut at x = 20) |
| 6.9 | "Select default plane" (the button beside Section planes): a 308 px popup (drag handle, [Select a plane ▾] Top / Front / Right, Flip plane, green ✓ "Commit current plane", ✕ "Cancel"); a choice adds a section plane at the origin (the next choice replaces it), Flip renames it "(Back)" / "(Bottom)" / "(Left)", ✓ keeps it, ✕ takes it away | behaviours "Select default plane", ui-snapv/25 | ✅ Part Studio (psui 10-27, section_dialog.rs `PlanePopup`; the button used to add a view-direction plane at once); assembly: still adds Front at once | 0.4.10+ / f16b31c6 10-27 |

## Handover (nav2 → psb, 10-25 15:45)

nav2 was paused; psb takes this list. State per area, with the code locations found:

Only the trace hook is in the ready batch. Everything else below (box overlay and box_select.rs, the section
manipulator, the replay suite, the AN1 test) is the **WIP commit on agent/nav2** after it: clippy-clean
(target-qa, 15:35) but its unit tests had not finished running; cherry-pick and test before taking it.

- **Trace hook** (✅ committed): `nav_trace::highlights` (crates/app-core/src/nav_trace.rs), called once in
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

## psb → ui handover (10-25 evening; the list is ui's now)

- Taken from nav2's WIP (agent/psb 086db0dc): box_select.rs, the box look (`box_overlay`), the AN1 test and
  the replay suite files. **Not taken:** nav2's section_manip.rs, its `asm_scene` glyph refactor and
  `Viewport::tool_hover`; the Part Studio section manipulator is psb's own section_manip.rs (base ring, arrow,
  two rotate rings with 2° snapping and the degree box, test `rotation_snaps_and_reads_as_live`).
- Box kinds, the box's additive / no-toggle rule and no pre-highlight while dragging: answered by live "nav2
  selection" (rows 1.8, 1.9, 1.11); the box now drops planes again.
- Ctrl+click → context menu (1.3) and the paused pre-highlight (3.5): viewport.rs.
- Waiting on the lead (part_studio/mod.rs is ui's): Esc keeps the selection (1.6), Shift+click in empty
  space keeps it (1.4). Assembly box / drag-moves-selection (2.5) is assembly_view.rs.
- Section value box (6.3): live's field (75×26, corner +40/−25 px from the rotate glyph's centre, right-aligned,
  updated while dragging), the distance from the drag start in 0.1 mm steps, open after Enter, Esc closes it
  before cancelling; the drag start's "⊥" with a 2 px #2861b6 leader; the base circle faces the viewer
  (section.rs, section_manip.rs). Not yet checked in the app; the rotate drag's "⊢" start mark is not drawn.
- Status-bar measure readout: nothing for three or more entities (measure.rs `quick_line`).
