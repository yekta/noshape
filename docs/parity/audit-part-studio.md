# Audit: every Part Studio UI surface of live against ours (psui, from 2026-11-01)

User 11-01: the checklists only cover what someone listed. This sheet audits whole surfaces: toolbar and overflows, every
menu and right-click menu, every feature dialog's fields and options, the keyboard shortcuts, the panels, the view cube and
view menu. Sources: live DOM dumps (onshape-research/live/dom/), behaviours.md, and for dialogs Onshape's FeatureScript std
(crates/featurescript/std, the oracle tests/std_oracle.rs: `print_all` diffs every toolbar feature's dialog with its std
precondition in every visibility state). Status: ✗ gap open, ✅ fixed (commit), – accepted (reason), ? live asked.

## Surfaces

| Surface | Source | Result | Gaps |
|---|---|---|---|
| Toolbar, every group and overflow (75 tools + Add custom features) | behaviours "Toolbar" (Customize toolbar list), dom/toolbar-partstudio.txt | every live tool in live's group, in order (Move boundary = extendSurface) | 0 |
| Toolbar right-click, Customize toolbar, narrow-window collapse | behaviours "Toolbar", "Narrow window" | checked earlier (cursors 81, part-studio 4.x, chrome-sweep) | 0 |
| Feature dialogs, fields and options (75 toolbar features); their editing logic (48 std functions) | std preconditions via std_oracle::print_all | 64 same as std now (37 at first); the rest differ only in accepted rows or the 10 open below | rows below, A15 |
| Keyboard shortcuts (live's Shift+/ list, 121 entries) | dom/keyboard-shortcuts.txt | every applicable entry in workflow-keyboard-mouse.md, all ✅ (11-01) | 0 |
| View menu (camera / render ▾) | dom/menu-camera-render.txt | 16 items = live (part-studio 9.8) | 0 |
| Context menus: face, edge, vertex, empty space, part, plane, origin, mate connector, sketch region / curve, feature rows (sketch, extrude, fillet, multi), parts-list part, tabs, Select ▸ submenus | dom/context-menus-partstudio.txt, -ps-*.txt | compared in core-workflow §8 and chrome-sweep CM1–CM5 (✅) | 0 |
| Context menus: mesh / composite / surface / curve rows and graphics, folders, rollback bar, suppressed / failed / hidden-sketch rows, view cube | live 1714 (dom/psui-audit-rmb-*.txt) | compared in the app 11-01 | A5–A10 |
| Panels: Measure, Mass and section properties, Section view (1709–1711) | live dumps | = live (measure types, units, reference frame, Add variable; Select default plane HUD, cut-face menu) | 0 |
| Right-strip panels: Configurations, Variable table, Appearances, Custom tables, Model definitions (1712) | live dumps | compared 11-01 | A11–A14 |
| Render Studio / Display states (1713) | live | live: no Display states in a Part Studio; Render Studio only as an Upgrade-gated tab type (cloud plan) | 0 |
| View cube (right-click, arrows, hover) | live 1714 | every face / edge / corner / arrow gives the empty-space menu, no tooltips: = ours | 0 |
| Feature manipulators (graphics handles of every dialog) | std's addManipulators (39 files) + live psb 10-24, 11-01 | section "Manipulators" | M1–M15 |
| Toolbar tooltips and tool search descriptions | live tooltips-en_US / tool descriptions | word for word (app-core tool_text.rs) | 0 |
| Feature failure texts (row tooltip, dialog title) | std's ErrorStringEnum | 378 of 529 native texts not std's | A22 (core) |

**Counts (11-02, audit 12):** surfaces audited 19 (13 + Variable table, a feature's construction planes, graphics menus of curves / composite members / mate connectors / empty space / view cube, the appearance editor, Measure in every type, Section view with several planes). Dialogs: 199 field rows + 52 visibility states; fixed 183 rows and 39 states; accepted 14 rows and 13 states; open 1 row (Frame's profile picker); 64 of 75 dialogs equal std. Editing logic: 48 std functions, 36 run std's, 10 native, 1 no-op, 1 partly; custom features' own now run too. Manipulators: 15 rows, 13 fixed (M11's flips all in audit 12), 2 = std; open M12 (P3: Loft, Wrap, Ruled surface, Bridging curve, Fit spline, Edit curve, Offset curve on face) and M15's points (End cap, Gusset, Frame, Decal, Derived). Found outside the oracle (A rows): 30; fixed 27, = live 1 (A1), open 2 (A10, A22: core's).

## Found outside the oracle (in the app, std's code, live)

| # | Surface | Item | P | Difference | Status |
|---|---|---|---|---|---|
| A1 | Tag dialog | basePlane | P2 | std pre-fills Base plane with the Top plane (tag.fs defaults `qTopPlane`); ours starts empty | – = live (1722): live's Base plane starts empty too (std's defaults map isn't a dialog prefill) |
| A2 | Variable (Measured) | componentSelector | P1 | Along custom recorded the plain distance, and X / Y / Z only while the Axis measurements group was open; std records the chosen component always and Along custom along the picked direction ("Select custom direction." when empty) | ✅ ce91c9c7 |
| A3 | Query variable dialog | additionalQueries | P1 | an additional query offers only Selection / Created by / Owned by / Protrusion / Pocket / Hole style types with 5 seed fields; std's item has 20 fields (entity type, fillet compare, bounded faces, edge convexity, construction / body-type filters, adjacency) and ids in std's case (addQselectionType, …) | ✅ (audit 3): std's 20 item fields with std ids (old documents renamed on load); the evaluator now does Protrusion, Pocket, Hole, Fillets (Equal), Tangent connected, Parallel, Bounded faces, Loop/chain connected and Edge convexity for the main query and each additional one, with each item\'s own entity type. Still to do: Matching, Adjacent, Fillets Less / Greater than (warnings) |
| A4 | Mutual trim | body1 / body2 | P1 | std trims a set of surfaces against a set ("First set of surfaces", "Second set of surfaces"); ours takes one surface a side; with nothing to trim std reports "no geometry change" as info, ours errors | ✅ (audit 3): sets of surfaces each side, std's texts ("Select a surface.", "Same surface cannot be used twice."), no change = info (T1 features_b offset_curve_move_boundary_mutual_trim) |
| A5 | Parts list rows (live 1714) | composite / surface / curve rows | P2 | ours added Copy <name>, Hide other parts, Hide all parts (and Export… on a curve); live: none on these rows | ✅ (audit 4) |
| A6 | Graphics menu (live 1714) | surface face | P2 | ours added Hide other parts / Hide all parts after Hide Surface 1 (live: only on solid parts) | ✅ (audit 4) |
| A7 | Feature list (live 1714) | hidden sketch row | P2 | missing Show all sketches (after Show) and Section view… (before Suppress) | ✅ (audit 4): Show all sketches while other sketches are hidden, Section view… before Suppress |
| A8 | Feature list (live 1714) | failed feature row | P2 | missing "Edit healthy moment of <feature>…" (after Edit…); ours adds Zoom to selection | ✅ (audit 5): Zoom to selection dropped; Edit healthy moment of <feature>… after Edit… opens the normal edit dialog (live 1717 (c)) |
| A9 | Feature list (live 1714) | folder row, selected | P3 | live adds Clear selection (own group after Dynamic suppression) when the folder row is selected | ✅ (audit 4) |
| A10 | Model (core) | composite part with later features | P1 | in kinds.noshape (mkkinds + Fillet on a Part 1 edge + 100 mm Chamfer) the composite lists under Parts, and the Fillet adds a stray "Part 3" (4000 mm³, Fillet 1's body .20000); without the later features the Composite parts section is right | ✗ core (TICKETS) |
| A11 | Configuration panel (live 1712) | filter button | P2 | live's filter button opens a menu (:name, :featurescriptid, :featurename, :part, :hasvisibilitycondition, :excludedfromproperties); ours opens nothing | ✅ (audit 4): the six commands, each puts its prefix in the box; the box filters by them |
| A12 | Configuration panel (live 1712) | row right-click | P2 | live (first row): Duplicate row · Move down; ours: Delete only | ✅ (audit 5): by position as live 1717 (a): Switch to "X" (not current) · Set as default (not default; the current configuration stays) · Duplicate row ("Copy of X" under it, values copied) · Move up · Move down · Delete row (not default); `config_edit_option` set_default / move / duplicate in Part Studios |
| A13 | Variable table (live 1712) | row right-click | P2 | live: Filter feature list · Delete · Move down · Configure; ours lacks Configure | ✅ (audit 4–5): Configure adds the value to the first List / Checkbox input (as a dialog parameter); absent without one (live 1717 (b)) |
| A14 | Appearances (live 1712) | part row right-click | P2 | live: Rename… · Edit appearance… · Configure appearance; ours has no menu on part rows | ✅ (audit 5): Rename… (the small "Rename" ✓ ✕ popup beside the row) · Edit appearance… · Configure appearance (Configured properties for the part, a row per configuration and an Appearance column; offered with configuration inputs); Remove appearance stays after them on a part with its own |
| A15 | Feature dialogs | arrays std drives by a query | P2 | std's "Driven query" arrays are one selection field (a pick per item); Frame's Corner overrides, Frame trim's groups, Face blend's Caps and Edge limits, Constrained surface's Vertices, Ruled surface's Vertex overrides and Sheet metal loft's connections were item lists with an Add button; the oracle didn't see it (the custom-feature reader ignored "Driven query", the oracle masked the hint) | ✅ (audit 7): all 13 std driven arrays are driven queries; custom features read "Driven query"; the oracle compares the hint |
| A16 | Configuration panel | configured parameter column headers | P2 | std names 99 parameters differently in the configuration table ("Column Name": the second direction's Depth is "Second depth", Variable's row Index "Row index (0-based)"); ours showed the dialog label, so two "Depth" columns; the oracle didn't read it | ✅ (audit 8): `features::columns` holds std's 99, the table header uses them, custom features read "Column Name", the oracle compares them; the Configure toast keeps the label until live answers (REQUESTS psui A16) |
| A17 | Routing curve dialog (live 1722) | points list | P2 | live opens with "Point 0" at the origin; Single mode shows only the active point's row (click a point to switch); a new point copies the active one's coordinates; the title carries the length ("Routing curve (61.825 mm)"); under two points the error ("Create at least two non-coincident points.") is on the title and the Points label. Ours: no point at first, every row shown, new points at the origin, plain title, the error on Vertices. Rows are titled "Point N" as live (audit 9) | ✅ (audit 9): std's editing logic runs (Point 0 to start, a new point copies the active one, std's error on Points); the name \"Routing curve (30 mm)\"; Single mode shows the active point's row only, with the triad on that point (dragging sets its offsets) and a click on another point making it active |
| A18 | Feature dialogs (live 1722) | a length / real value out of range | P2 | live: the field turns pink with no tooltip, ✓ stays enabled, the title reads "…properly:  A parameter value is not valid." (the feature's message, then the text); ours: "Enter a valid expression." on the field, ✓ disabled. Integers out of range stay a validation error (DIALOG-OK-STATE: ✓ disabled, "Enter an integer value between …") | ✅ (audit 9) |
| A19 | Variable dialog | From import (CSV) | P1 | ours failed every From import variable ("Importing variables from a table is not available yet") and its CSV file was a plain text field | ✅ (audit 10): std's variable.fs runs the From import mode (rows / columns by index, range or label, conversion factor) on the CSV tab; std features read reference parameters through the document; the CSV file is a reference field with live's picker (test custom_features::variable_from_import_reads_the_csv_tab) |
| A20 | Routing curve dialog | std's buttons (Process, Reset triad, Previous / Next / Confirm of the orthogonal path) | P2 | ours had none (the CSV and curve inputs could never become points); the oracle didn't list them (the custom reader ignored `isButton`) | ✅ (audit 10): `isButton` is a button kind (Hints::BUTTON); a click runs std's editing logic with `clickedButton`; editing logic reads reference parameters (CSV) through the document; Table is a CSV reference field (test custom_features::routing_curve_process_reads_csv_points) |
| A21 | Mass and section properties (Part Studio, Assembly) | Show calculation variance | P2 | the box did nothing; live appends "± <bound>" to every value ("28919.247 mm³ ± 9.422e-1 mm³", inertia cells "56.187 ± 9.046e-5") | ✅ (audit 10, user 11-01): core's `mass_property_variance` / `face_property_variance` (the assembly's through the same function, parts placed where they sit), only while ticked, cached per parts and model, unticked on reopening as live; live's format (`quantity_field::plus_minus`), one bound for the whole inertia grid, none on an overridden mass. Checked on :102 against live 1731's 60 × 40 × 20 block: 48000 mm³ ± 1.328e+0 (live 1.321), 8800 mm² ± 3.637e-2 (3.634e-2), centre ± 1.000e-3 mm, inertia ± 1.057e-4 (1.058e-4), its top face 2400 mm² ± 1.796e-2 (1.793e-2), perimeter ± 1.330e-4 (1.339e-4), moments ± 2.400e+3 |
| A22 | Feature error tooltips and dialog titles (every native feature) | the failure texts | P2 | of the 529 literal texts our native features fail with, 378 are not among std's ErrorStringEnum texts (errorstringenum.gen.fs comments, 1767; live shows std's): docs/parity/audit-part-studio-errors.txt lists them by file (curves 61, sheet metal ~140, hole 19, pattern 15, fillet sections 16 internal). Many are internal fall-back reasons or conditions live has none for; the rest need each failure mapped to std's condition and text | ✗ core's (regen features): request to the lead 11-01 |
| A23 | Feature previews (Fillet, Chamfer, Shell against live 11-01 on the 40 × 20 × 10 block) | a picked face the feature changes | P2 | live highlights a pick as it was before the feature: Shell's removed face is its whole outline (the rim unlit); ours lit the rim, which keeps the removed face's name. Fillet / Chamfer previews = live (radius and distance are remembered values) | ✅ (audit 11): picked faces the feature changes (area or box) are drawn from the model before (changed_picks → consumed_picks_overlay), whole over the preview blend; T1 a_shelled_face_is_highlighted_as_before. Audit 12: the removed face filled too, as live 11-01 (khaki #bcb36e at 0.47 over the opening and the rim; ours rim (169,186,166), walls (165,180,160) / (151,161,139) against live's (173,187,174), (167,179,164) / (146,151,132)) |
| A24 | Variable table (user 11-02, live 11-02) | Value cells, columns, rows | P1 | `#` in a Value cell listed nothing; an expression showed as text, cut off; columns fixed at 140 px; rows not draggable; ⋯ lacked Collapse all tables | ✅ (audit 12): `#` / word autofill as live (the variables above the row, sorted, values grey italic, no New variable…; ↑↓, Enter inserts and commits, Tab inserts, Esc closes); not edited, a value shows its result ("120 mm", "40mm" → "40 mm") with the expression as tooltip, a failing one pink as typed with "Error regenerating."; a header's right edge (5 px, col-resize) drags its width with a guide line, at least 40 px, kept while the app runs (live: across tabs, not a reload); a row drags by its Name (blue pill, blue drop line) and moves its Variable feature; ⋯ Collapse all tables. Type ▾ = live (a type keeps its own value: Angle "0 deg", back to Length the old value). Inserted Variable Studio tables (and the Variable Studio's own) use the same widths; a dragged row leaves its name unfocused. T1 a_value_cell_completes_a_variable, a_variable_expression_shows_its_result_until_edited, variable_table_columns_resize, a_variable_row_drags_to_reorder; model variable_table a_dragged_row_moves_its_feature |
| A25 | Feature list (user 11-02, live 11-02) | planes a FeatureScript feature leaves | P1 | the user's "Y Ribs on Faces" left 3 planes in the graphics, in no list and untoggleable: opDeleteBodies(qCreatedBy(planeId, BODY)) missed them (the FS host typed a plane as a face only), and planes a feature keeps carried an op marker no row knew | ✅ (audit 12): a construction plane is also a body (custom host Ent::PlaneBody), so qCreatedBy / opDeleteBodies take it; the planes a feature keeps are its own (`Reference::ConstructionPlane`, "Plane 1", "Plane 2"): its row gets ▸ and an eye (hides them with it), each plane a grey child row with its eye and a plane's menu (New sketch…, Size to Part Studio, Hide / Show, Hide other planes, Hide all planes, Section view…, Zoom to selection, View normal to), as live. T1 custom_features::op_plane_bodies_are_deleted_by_op_delete_bodies, a_features_planes_are_its_own_and_hide_alone_or_with_it. Then (live 11-02): the rows stay grey under the hover, the planes carry no name, each plane's eye / tooltip ("Plane 1 (Hidden)") / menu follow its own state while it is drawn only with its feature shown; the user's script itself (tests/fs/yribs.fs) on a cylinder: ribs built, no plane left (T1 y_ribs_on_a_cylinder_leave_no_planes) |
| A26 | Graphics context menus (live 1714 dumps) | a curve, a composite member, a mate connector, empty space, the view cube | P2 | a curve (Composite curve of a sketch's edges) listed Fillet…, Chamfer…, Curve/surface analysis…, Copy, Export…, Zoom to selection, Align view, Delete Curve 1…, materials, and no Edit Sketch / Show dimensions; live: Edit Composite curve 1… · Edit Sketch 4… · Show dimensions · Show dependencies… | Hide Curve 1 · Isolate… · Make transparent… | Create Drawing of Curve 1… | Select ▸ · Select other… | Zoom to fit | Delete Composite curve 1 | ✅ (audit 12): a curve's menu as live (its source sketch also from its edges' sketch); the composite member (names the member part), mate connector, empty graphics and view cube menus = live (Add comment is cloud). T1 a_curves_graphics_menu_as_live (os_menu OsMenu::labels for whole-menu checks) |
| A27 | Appearance editor (live: behaviours "Appearance editor R / G / B / A / hex", "Colour Mixer") | the colour fields | P2 | ours took any text: R "300" left R at 30 with "00" shown, the hex kept "0000ff" lower case and applied 3-digit forms; live filters each keystroke | ✅ (audit 12): R / G / B digits only, 255 at most as typed; hex hex digits only, no #, at most six, upper case once six, applied only at six; A digits and "." only, 1 at most. Layout = live (Palette / Mixer, swatch, transparency slider, A, # R G B, Custom colors, Tessellation quality + "1 fine part"), Esc reverts = live. T1 appearance::tests::hex_and_srgb_round_trip (the filters). Then: Enter in a field accepts and closes the editor (ours only applied the field), Tab / Shift+Tab go A → # → R → G → B (ours: nothing), an emptied R / G / B / A shows its value again when left; ↑↓ and the wheel do nothing = live. T1 the_appearance_editors_fields_tab_and_enter_as_live |
| A28 | Measure, every type (live 1709, 11-02: a 40 x 30 x 20 block, a Ø16 cylinder; 14 selections, each type) | pair rules | P2 | a face square to a cylinder's axis, a straight edge square to it, a vertex and a cylinder, a circle and a plane parallel to it (apart or on the face), a circle on its cylinder, and an edge and a circle fell to Min / Area / Max: no Perp axes angle, Center axes dist, Center dist, Face tangent angle; a disc's Max dist to its rim read 16.001 (16.000) | ✅ (audit 12): each as live (Perp axes angle · Area · Max · Min; Perp axes angle · Min · Area · Center axes dist · Max · Length; Min · Area · Center axes dist · Max; Min · Max · Area · Center dist · Length, on the face Face tangent angle · Curvature deviation first; Face tangent angle · Curvature deviation · Area · Center axes dist · Max · Min · Length; Min · Max · Center dist · Length; an edge along a cylinder Min · Max · Area · Center axes dist · Length; an edge standing on a face Perp line and plane angle · Curvature deviation · Area · Max · Min, in its plane Parallel dist · Curvature deviation · Area · Min · Max, parallel apart Parallel dist · Max · Area · Min); the type filter keeps Face tangent angle (one filter for both studios, model-core measure::row_type); Max dist kept on the face within 1 nm. Single faces / edges and every type's filtering of them = live. T1 measure::tests::a_block_and_a_standing_cylinder_measure_as_live |
| A29 | Section view options (live 1709, 11-02: Top / Front / Right, faces, an offset plane, a 4th pick) | several planes | P1 | with two or three planes ours removed only what every plane removed (Top with Front, which alone cuts nothing, cut nothing); live removes what any plane removes. Chips renumbered on a removal ("Section plane 1 (Front)" for live's "Section plane 2 (Front)"); only the active plane had a manipulator; a 4th plane read "Maximum number of section planes reached" | ✅ (audit 12): the renderer removes what any plane removes, each cap drawn where the other planes keep the model; a plane keeps its number, a new one takes the lowest free (both studios, app-core section_dialog::next_plane_number); every plane's manipulator is drawn and lit under the cursor, grabbing one makes its plane the one driven; live's "Maximum number of section planes reached. Remove a section plane to add another.". Exclude / Include, the 3-plane limit, faces named by their nearest default plane, the value box = live. T1 render offscreen::section_planes_each_remove_their_side, UI section_planes_keep_their_numbers, another_section_planes_arrow_drags_it |
| A30 | Section view (live 1709, 11-02) | a cylinder or cone as the plane | P2 | the prompt asks for "a plane, planar face, cylinder, cone, or mate connector" but a cylinder pick only repeated it; live cuts through the axis ("Section plane 1 (Right)" for a cylinder from a circle on Top, wherever picked; the manipulator on the axis at mid-height) | ✅ (audit 12): a cylinder or cone face gives the plane through its axis across its reference direction (ops studio::section_frame_of_ref, also render_image's `section`), the base at the face's centroid. e2e part_studio_menu_ops (a section through the hole) |

## Manipulators (std's addManipulators)

The graphics handles a feature dialog shows are what its std feature adds with `addManipulators` (39 std files; their
"Manipulator Change Function"s turn a drag into parameter values). Live psb 10-24 (behaviours "psb (10-24 i) feature
manipulators") confirmed the std reading for seven dialogs: only Revolve and Extrude of them have handles. Ours are
drawn natively (part_studio/manipulators.rs, triad.rs, section_manip.rs): our FeatureScript host ignores
`addManipulators`, and most of these features regenerate natively, so std's own calls don't run for them.

| # | Feature | std's manipulators | Ours | P | Status |
|---|---|---|---|---|---|
| M1 | Shell, Chamfer | none (live: "no thickness arrow", "no arrow") | a thickness arrow, a distance arrow | P2 | ✅ (audit 11): removed |
| M2 | Extrude | depth (linear) or flip; second end: depth (SECONDARY) or flip | depth / direction arrow; no second end | P2 | ✅ (audit 11) second end's depth: the double chevron at the second cap (live ex2), dragged on its own; other second end types its double-chevron flip arrow; a click on a flip arrow flips it (live 11-01, Through all: ours ignored clicks) |
| M3 | Revolve | angle (angular) or flip; second end: start angle (SECONDARY) or flip | an arc in dark blue with a dot, whole degrees, never past 0 | P2 | ✅ (audit 11) live's look (white in a grey halo, open head along the turn, blue on hover), snap to 5° within 2°, four decimals, dragged back past 0 flips the direction; the start angle's double chevron arc; other end types a flip arrow at the profile across the turn (double chevron for the start), flipped by a click |
| M4 | Move face | offset / translate (linear), rotate (angular) | = std, but the rotate arc never went past 0 | P3 | ✅ (audit 11): dragged back past 0 it flips the direction as std's moveFaceManipulatorChange |
| M5 | Fillet | radius (linear); partial fillet: a point-on-edge (linear) per bound | radius | P2 | ✅ (audit 11) the bounds' arrows (Position and Offset bounds) as std places them, tip on the bound point, std's filletManipulatorChange applies the drag (interaction::partial_fillet_manipulators; checked on :102: 0.25 dragged to 0.6) |
| M6 | Plane | offset (linear), Line angle (angular) | offset | P2 | ✅ (audit 11) Line angle's arc: about the line, a quarter of the plane's size out along where the angle is measured from (std's own projection), dragged as Revolve's |
| M7 | Transform | triad (translate by XYZ), angle (rotate about axis), distance (translate by distance) | triad | P2 | ✅ (audit 11) Rotate's arc and Translate by distance's arrow from the parts' box centre (std findCenter) |
| M8 | Draft | angle (angular, two for a parting line), pull direction flip | none | P2 | ✅ (audit 11) Neutral plane: std's arc (about the first face's line on the neutral plane, from its middle; live 11-01: radial line, a 26 px stem past the arc, a 16 px open head; every angle handle now draws that stem); its drag in tenths of a degree up to 89.9°, past 0 flips Opposite direction (live's snapping not measured); Parting line (One sided, Symmetric, Two sided): std's arcs and Switch face flip from its ordered face data, applied by std's draftManipulatorChange (interaction::draft_parting_manipulators; checked on :102) |
| M9 | Helix | height (linear), direction flip | none | P2 | ✅ (audit 11) height arrow for Axis and Circle with Height (not Turns and pitch) and Cylinder / Cone's direction flip, as std's helix.fs places them, applied by std's helixManipulatorChange (look not checked on live); Turns and pitch (any axis): the direction flip at the helix's end as std's getTurnsAndPitchData places it (model-core interaction::helix_manipulators on the regen's own frame; T1 features_c::helix_turns_and_pitch_flip_as_std) |
| M10 | Offset surface, Replace face, Move boundary, Move curve boundary | distance (linear) | none | P2 | ✅ (audit 11) Offset surface (= live 11-01: the arrow at the offset face's middle), Replace face (on the face to replace with), Move boundary Blind (on the last edge, out of its face; `select::edge_extend`); Move curve boundary (audit 12: Extend, Blind, with a help point, as std: from the first curve's end that grows along its tangent, the tip at the extended end where std anchors it, past the end held at the least; interaction::move_curve_boundary_anchor on the regen's own end choice; T1 features_c::move_curve_boundary_arrow_at_the_growing_end) |
| M11 | Split, Projected curve, Mutual trim, Body draft, Frame trim, Face blend | flips | none | P3 | ✅ (audit 11) Split (Part, not Keep both sides): the keep-side flip arrow on the tool's plane by the parts, a click flips it; audit 12: Projected curve (Curve to face, Direction: the flip at the direction's plane or axis origin along its normal), Body draft (along the pull direction, at the parting object's middle by its parameters or a connector's / plane's origin; Draft on self, at the first drafted entity's point nearest the pull entity), as std places them (T1 projected_curve_direction_flip_as_std, body_draft_flip_as_std); Frame trim (Face: on the tool where nearest the parts, along its normal; a connector's Z), Face blend (each side's flip at its face's middle, along its normal unless the side is aligned as wallsNormalAlignment; caps and limits not built here), Mutual trim (both at the middle of the surfaces' section, across it in each surface, away from the other's normal), a click through std's change functions (T1 frame_trim_flip_as_std, features_c::face_blend_side_flips_as_std, features_b::mutual_trim_flips_as_std) |
| M12 | Loft, Wrap, Ruled surface, Bridging curve, Fit spline, Edit curve, Offset curve on face | lengths / angles / points | none | P3 | (audit 12) Fit spline (startManipulator / endManipulator: at each end with a direction, offset = magnitude × the points' box diagonal / 3) and Ruled surface (depthManipulator.T.-1 at the first edge's free end along the ruling, angleManipulator.T.-1 about its tangent but for Aligned with vector) as std, from the evaluators (EvalCtx::manipulators), a drag running std's change function; T1 features_c::fit_spline_magnitude_arrow_as_std, ruled_surface_manipulators_as_std. Wrap (vShiftManipulator from the U-shifted anchor, uShiftAngleManipulator about the axis, angleManipulator about the normal at the shifted anchor, a tenth of the tools' box diagonal out; each the way its value moves our development; T1 wrap_manipulators_as_std). Offset curve on face's trims wait on its Offset and extend scope (core: only Offset is built); Bridging curve's wait on std's control points (ours approximates curvature and flow with tangency); Edit curve's need a triad manipulator kind (FeatureManipulator has none). ✗ Loft (connection points) |
| M13 | Routing curve | full triads, points | triad, points | – | = std (audit 9) |
| M14 | Linear / Circular / Curve pattern | instance points (Skip instances) | points | – | = std (p1 dialogs) |
| M15 | Sheet metal: Flange (length), Sheet metal model (flip), Corner break; End cap, Gusset, Frame, Decal, Derived (points) | std's | none | P3 | ✅ (audit 11) for every feature FeatureScript runs (std's sheet metal, Tag, PCB): the host keeps `addManipulators` (FeatureState::manipulators), the dialog draws lengths / flips as arrows and angles as arcs, and a drag or a click runs the feature's own "Manipulator Change Function" (std_feature::manipulator_change_in); checked on :102 (Sheet metal model's flip ticks Flip direction up). custom (Feature Studio) features too: their own "Editing Logic Function" (never run in dialogs before) and "Manipulator Change Function" (CustomFeatureDef::editing / manipulator_change; T1 custom_features::custom_feature_manipulators_and_editing_logic_run). Points manipulators (audit 12, live 1709 Frame): drawn as 7 px white dots ringed grey, the chosen one light blue, the hovered one blue; a click on another runs the feature's change function (FeatureScript features and native ones alike). Frame's own (native): its nine alignment points around the chosen one at the first segment's middle and the angle arc a profile width out (std createFrameManipulators; T1 sheetmetal_frames::frame_alignment_points_as_std). ✗ End cap, Gusset, Decal, Derived |

## Editing logic (std's dialog reactions)

std's 48 "Editing Logic Function"s (36 run std's, 10 native, 1 no-op, 1 open) change a dialog's fields as the user edits (Add into the touched part, the flip, a type
from the picks). The dialog runs std's own function where `std_feature::has_editing_logic` says so (std runs the feature, or it is in
`STD_EDITING_LOGIC`; T2 tests in std_oracle), or ours does the same natively (`behaviors::direct_edit_logic`, `boolean_edit_logic`).

| # | std function | Feature | Status |
|---|---|---|---|
| E1 | extrudeEditLogic | Extrude | ✅ std's runs |
| E2 | filletEditLogic | Fillet | ✅ std's runs |
| E3 | loftEditLogic | Loft | ✅ std's runs |
| E4 | revolveEditLogic | Revolve | ✅ std's runs |
| E5 | sweepEditLogic | Sweep | ✅ std's runs |
| E6 | mirrorEditLogic | Mirror | ✅ std's runs |
| E7 | cPlaneLogic | Plane | ✅ std's runs |
| E8 | draftEditLogic | Draft | ✅ std's runs |
| E9 | splitEditLogic | Split | ✅ std's runs |
| E10 | holeEditLogic | Hole | ✅ std's runs |
| E11 | thickenEditLogic | Thicken | ✅ native: Add / merge scope from the solids' faces, the flip (`auto_operation`, live L18); std's agrees (T2 thicken_and_modify_fillet_editing_logic_runs_std) |
| E12 | modifyFilletLogic | Modify fillet | ✅ std's runs (audit 8, T2 thicken_and_modify_fillet_editing_logic_runs_std) |
| E13 | sheetMetalStartEditLogic | Sheet metal model | ✅ std's runs |
| E14 | flangeEditLogic | Flange | ✅ std's runs |
| E15 | hemEditLogic | Hem | ✅ std's runs |
| E16 | sheetMetalTabEditingLogic | Tab | ✅ std's runs |
| E17 | onBendChange | Bend | ✅ std's runs |
| E18 | onJogChange | Jog | ✅ std's runs |
| E19 | formedEditLogic | Form | ✅ std's runs |
| E20 | tessLoftEditLogic | Sheet metal loft | ✅ std's runs |
| E21 | makeJointEditLogic | Make joint | ✅ std's runs |
| E22 | sheetMetalJointEditLogic | Joint | ✅ std's runs |
| E23 | pcbStartEditLogic | Flex PCB model | ✅ std's runs |
| E24 | pcbEndEditLogic | Finish flex PCB model | ✅ std's runs |
| E25 | deleteFaceEditLogic | Delete face | ✅ native: Leave open for surface faces (live L10) |
| E26 | replaceFaceEditLogic | Replace face | ✅ native: flip alignment (live L8) |
| E27 | ribEditLogic | Rib | ✅ native: parts from the sketch |
| E28 | connectorEditLogic | Mate connector | ✅ native: owner part |
| E29 | moveFaceEditingLogic | Move face | ✅ native: direction / offset |
| E30 | fillEditLogic | Fill | ✅ native: edges from the preselection, surface operation (live SF7) |
| E31 | onDecalFeatureChange | Decal | ✅ native: width / height from the image |
| E32 | bodyDraftEditLogic | Body draft | ✅ std's runs (audit 8): the draft scope from the preselection (a part: Parts), a parting plane is the pull direction too (T2 body_draft_editing_logic_runs_std) |
| E33 | bridgingCurveEditingLogic | Bridging curve | ✅ std's runs (audit 8): the hidden side flags its fields show by (Start edge position hidden for a vertex, tangency fields for faces; ours never set them), sides from the preselection (T2 bridging_and_edit_curve_editing_logic_runs_std) |
| E34 | bsurfEditLogic | Boundary surface | ✅ std's runs (audit 8): Add when a profile is a surface's open edge (T2 surface_operation_editing_logic_runs_std) |
| E35 | cutlistEditLogic | Cut list | ✅ std's runs (audit 8): only the first override hides "Same frames as previous" (T2 cutlist_and_delete_part_editing_logic_runs_std) |
| E36 | deletePartEditLogic | Delete part | ✅ std's runs (audit 8): Entities and the composite-free list in step (same T2) |
| E37 | editCurveEditLogic | Edit curve | ✅ std's runs (audit 8): a control point edit's offsets reset when its reference changes (same T2) |
| E38 | editFeatureLogicExternalThread | External thread | ✅ std's runs (audit 8): the size nearest the picked cylinder in the chosen standard (8 mm: 5/16\"-18, M8 × 1.25), chamfer and undercut from it; std's lookup results reach the dialog (T2 external_thread_editing_logic_runs_std) |
| E39 | fitSplineEditLogic | 3D fit spline | ✅ std's runs (audit 8): a preselected edge opens on Edges with it; the preselection goes to std's hidden initEntities wherever std's logic runs (Hole, Sheet metal model, Flex PCB too) |
| E40 | frameEditLogicFunction | Frame | ✅ std's runs (audit 8): a new corner override takes the default corner type and flip (T2 frame_editing_logic_runs_std) |
| E41 | gussetEditLogic | Gusset | – std's returns at once at the current version (V2745_QUERY_VARIABLE_CHANGES); nothing to do |
| E42 | helixEditingLogic | Helix | ✅ native: the tab follows the preselection (live L34, checked 11-01) |
| E43 | projectCurvesEditLogic | Projected curve | ✅ std's runs (audit 8): the preselection into the fields, the Curve to face flip (same T2) |
| E44 | routingCurveEditLogic | Routing curve | ✅ std's runs (audit 9): Point 0 to start, new points copy the active one (std's copyParameter now honoured), indices in step (T2 routing_curve_editing_logic_runs_std) |
| E45 | ruledSurfaceEditingLogic | Ruled surface | ✅ std's runs (audit 8): Add on a surface's open edge, reference faces, vertex overrides cleaned (same T2) |
| E46 | unfoldEditLogic | Unfold | ◐ std's only reuses the hold face of an earlier Unfold of the part (wall attribute holdFaceForUnfold); without one it leaves Wall face to hold empty, as ours. Left: the second Unfold of a part (our native Unfold records no hold face) |
| E47 | variableEditLogic | Variable | ✅ native: a preselection in Measured mode fills the visible field std's would (Entities to measure between / Entities / Entity); Assigned ignores it as std |
| E48 | wrapEditLogic | Wrap | ✅ std's runs (audit 8): Merge scope = the target's part, Add / Remove from the thickness direction, the flip; default anchors found (T2 wrap_editing_logic_runs_std) |

## Feature dialogs against std

Generated from `std_oracle::print_all` (first run 11-01; rows not in a later run are marked fixed). P1: a missing option or a
wrong default / range that changes what a user gets; P2: what the dialog shows (labels, option labels, groups, row labels,
which fields show); P3: internal (option ids under the same labels, configurability, remembered values, tooltips). The
oracle reads std's annotations, not its feature defaults map: a default std sets there (Ruled surface's cubic interpolation)
is checked by hand.

Field rows: 199 (fixed 184, accepted 14, open 1); visibility states differing now: 13.

| # | Feature | Field | P | Difference (std = live) | Status |
|---|---|---|---|---|---|
| D1 | editCurve | curveDegree | P1 | value: std `Integer 0..15 =0.000000000000 / 0.000000000000` / ours `Integer 0..1000 =3.000000000000 / 3.000000000000` | – read-only counts std's editing logic fills from the picked curve; ours shows the same once picked (P3) |
| D2 | editCurve | curveNumCPs | P1 | value: std `Integer 0..99 =0.000000000000 / 0.000000000000` / ours `Integer 0..100000 =4.000000000000 / 4.000000000000` | – read-only counts std's editing logic fills from the picked curve; ours shows the same once picked (P3) |
| D3 | editCurve | curveNumSpans | P1 | value: std `Integer 0..99 =0.000000000000 / 0.000000000000` / ours `Integer 0..100000 =1.000000000000 / 1.000000000000` | – read-only counts std's editing logic fills from the picked curve; ours shows the same once picked (P3) |
| D4 | mutualTrim | body1 | P1 | value: std `query max None` / ours `query max Some(1)` | ✅ (audit 3, 11-01) |
| D5 | mutualTrim | body2 | P1 | value: std `query max None` / ours `query max Some(1)` | ✅ (audit 3, 11-01) |
| D6 | ruledSurface | useCubicInterpolation | P1 | value: std `bool=false` / ours `bool=true` | – std's defaults map sets true (ruledSurface.fs); ours true |
| D7 | sheetMetalJog | jogLimitDistance | P1 | value: std `Length 0..500 =0.005000000000 / 0.006350000000` / ours `Length 0..500 =0.000000000000 / 0.000000000000` | ✅ (audit 1, 11-01) |
| D8 | sheetMetalJog | thicknessFactor | P1 | value: std `Real 0..100000 =1.000000000000 / 1.000000000000` / ours `Real 0.00001..100000 =3.000000000000 / 3.000000000000` | ✅ (audit 1, 11-01) |
| D9 | sheetMetalLoft | chordalTolerance | P1 | value: std `Length 0.00001..0.1 =0.001000000000 / 0.001270000000` / ours `Length 0.00001..0.1 =0.001000000000 / 0.001016000000` | ✅ (audit 1, 11-01) |
| D10 | tag | basePlane | P1 | missing: std has "Base plane" | ✅ (audit 1, 11-01) |
| D11 | tag | flatFormSketch | P1 | value: std `query max None` / ours `query max Some(1)` | – std takes a list but errors on more than one sketch (tag.fs doTagForm); ours allows one |
| D12 | tag | holes | P1 | missing: std has "Holes" | ✅ (audit 1, 11-01) |
| D13 | tag | pcbEntityType | P1 | missing: std has "PCB entity type" | ✅ (audit 1, 11-01) |
| D14 | tag | plated | P1 | missing: std has "Plated" | ✅ (audit 1, 11-01) |
| D15 | assignVariable | anyValue | P2 | extra: ours has "Value", std has no `anyValue` | – std has it (`isAnything`, variable.fs); the oracle's extraction skips isAnything fields |
| D16 | assignVariable | axisMeasurementsGroup | P2 | missing: std has "Axis measurements" (a group) | ✅ (audit 2, 11-01) |
| D17 | assignVariable | columnGroup | P2 | missing: std has "Column" (a group) | ✅ (audit 2, 11-01) |
| D18 | assignVariable | conversionFactor | P2 | extra: ours has "Conversion factor", std has no `conversionFactor` | – std has it (`isAnything`, variable.fs); the oracle's extraction skips isAnything fields |
| D19 | assignVariable | minmax | P2 | hints: std adds [], ours adds ['HORIZONTAL_ENUM'] | ✅ (audit 5, 11-01) |
| D20 | assignVariable | rowGroup | P2 | missing: std has "Row" (a group) | ✅ (audit 2, 11-01) |
| D21 | bridgingCurve | match1 | P2 | options: std ['Match position', 'Match tangent', 'Match curvature', 'Match flow (G3)'] / ours ['Match position', 'Match tangent', 'Match curvature', 'Match flow'] | ✅ (audit 2, 11-01) |
| D22 | bridgingCurve | match2 | P2 | options: std ['Match position', 'Match tangent', 'Match curvature', 'Match flow (G3)'] / ours ['Match position', 'Match tangent', 'Match curvature', 'Match flow'] | ✅ (audit 2, 11-01) |
| D23 | compositeCurve | approximationShowDeviation | P2 | hints: std adds ['GROUP_HEADER'], ours adds [] | ✅ (audit 2, 11-01) |
| D24 | constrainedSurface | showDeviation | P2 | hints: std adds ['GROUP_HEADER'], ours adds [] | ✅ (audit 2, 11-01) |
| D25 | constrainedSurface | vertices | P2 | array row label: std Some("#vertex"), ours None | ✅ (audit 2, 11-01) |
| D26 | cutlist | columnOverrides | P2 | array row label: std Some("#column = #value"), ours None | ✅ (audit 2, 11-01) |
| D27 | editCurve | approximationShowDeviation | P2 | hints: std adds ['GROUP_HEADER'], ours adds [] | ✅ (audit 2, 11-01) |
| D28 | editCurve | controlPointEdits | P2 | array row label: std Some("#index: #x;#y;#z"), ours None | ✅ (audit 2, 11-01) |
| D29 | editCurve | selectedIndices | P2 | array row label: std Some("Control point index #indexValue"), ours None | ✅ (audit 2, 11-01) |
| D30 | editCurve | showDetails | P2 | hints: std adds ['GROUP_HEADER'], ours adds [] | ✅ (audit 5, 11-01) |
| D31 | faceBlend | caps | P2 | array row label: std Some("#entity"), ours None | ✅ (audit 2, 11-01) |
| D32 | faceBlend | edgeLimits | P2 | array row label: std Some("#edge"), ours None | ✅ (audit 2, 11-01) |
| D33 | fill | addGuides | P2 | hints: std adds [], ours adds ['GROUP_HEADER'] | ✅ (audit 2, 11-01) |
| D34 | fill | edges | P2 | array item fields differ: std [entities "Edges" query max None hints 0x8000 "" None, continuity "Continuity" enum[("G0", "Position"), ("G1",… ours [entities "Edges" query max None hints 0x0 "" None, continuity "Continuity" enum[("G0", "Position"), ("G1", "T… | ✅ (audit 7) |
| D35 | fitSpline | approximationShowDeviation | P2 | hints: std adds ['GROUP_HEADER'], ours adds [] | ✅ (audit 2, 11-01) |
| D36 | frame | cornerOverrides | P2 | array item fields differ: std [vertex "Vertex" query max Some(1) hints 0x8000 "" None, cornerType "Corner type" enum[("MITER", "Miter"), ("B… ours [vertex "Vertex" query max Some(1) hints 0x0 "" None, cornerType "Corner type" enum[("MITER", "Miter"), ("BUTT… | ✅ (audit 7) |
| D37 | frame | cornerOverrides | P2 | array row label: std Some("#vertex [#cornerType]"), ours None | ✅ (audit 2, 11-01) |
| D38 | frame | selections | P2 | hints: std adds [], ours adds ['ALLOW_QUERY_ORDER'] | ✅ (audit 5, 11-01) |
| D39 | frameTrim | frameGroups | P2 | array item fields differ: std [frames "Frames" query max None hints 0x0 "Frame segments are trimmed against preceding groups and are used as… ours [frames "Frames" query max None hints 0x0 "" None]… | ✅ (audit 6) |
| D40 | frameTrim | frameGroups | P2 | array row label: std Some("#frames"), ours None | ✅ (audit 2, 11-01) |
| D41 | gusset | chamfer | P2 | hints: std adds [], ours adds ['FIRST_IN_ROW'] | – std pairs a DISPLAY_SHORT boolean with the DISPLAY_SHORT field after it; ours marks the pair's first field FIRST_IN_ROW for the same one row |
| D42 | helix | endRadToggle | P2 | hints: std adds [], ours adds ['DISPLAY_SHORT', 'FIRST_IN_ROW'] | ✅ (audit 9) |
| D43 | helix | endRadius | P2 | hints: std adds [], ours adds ['DISPLAY_SHORT'] | ✅ (audit 9) |
| D44 | importDerived | mateConnectorId | P2 | extra: ours has "Base mate connector", std has no `mateConnectorId` | – std has it too, declared `isAnything` (importDerived.fs:98), which the oracle cannot read |
| D45 | intersectionCurve | approximationShowDeviation | P2 | hints: std adds ['GROUP_HEADER'], ours adds [] | ✅ (audit 2, 11-01) |
| D46 | isoparametricCurve | directionType | P2 | default option: std U_DIRECTION / ours U | ✅ (audit 1, 11-01) |
| D47 | mateConnector | flipPrimary | P2 | hints: std adds ['FIRST_IN_ROW', 'OPPOSITE_DIRECTION'], ours adds ['PRIMARY_AXIS'] | – drawn as live draws it: the 26 px flip button in one row with Reorient secondary axis |
| D48 | mutualTrim | body1 | P2 | label: std "First set of surfaces", ours "First surface" | ✅ (audit 3, 11-01) |
| D49 | mutualTrim | body2 | P2 | label: std "Second set of surfaces", ours "Second surface" | ✅ (audit 3, 11-01) |
| D50 | offsetCurveOnFace | approximationShowDeviation | P2 | hints: std adds ['GROUP_HEADER'], ours adds [] | ✅ (audit 2, 11-01) |
| D51 | pcbStart | hasSecondDirection | P2 | hints: std adds [], ours adds ['GROUP_HEADER'] | ✅ (audit 2, 11-01) |
| D52 | pcbStart | offsetDistance | P2 | hints: std adds ['CAN_BE_TOLERANT'], ours adds [] | ✅ (audit 5, 11-01) |
| D53 | pcbStart | secondDirectionDepth | P2 | hints: std adds [], ours adds ['CAN_BE_TOLERANT'] | ✅ (audit 5, 11-01) |
| D54 | pcbStart | secondDirectionOffsetDistance | P2 | hints: std adds ['CAN_BE_TOLERANT'], ours adds [] | ✅ (audit 5, 11-01) |
| D55 | projectCurves | approximationShowDeviation | P2 | hints: std adds ['GROUP_HEADER'], ours adds [] | ✅ (audit 2, 11-01) |
| D56 | queryVariable | additionalQueries | P2 | array item fields differ: std [booleanOperation "Boolean operation" enum[("UNION", "Union"), ("SUBTRACTION", "Subtract"), ("INTERSECTION", "… ours [booleanOperation "Boolean operation" enum[("UNION", "Union"), ("SUBTRACTION", "Subtract"), ("INTERSECTION", "… | ✅ (audit 3, 11-01) |
| D57 | queryVariable | additionalQueries | P2 | array row label: std Some("#booleanOperation of #addQlowercaseSelectionType"), ours None | ✅ (audit 3, 11-01) |
| D58 | queryVariable | edgeAdjacentEntityType | P2 | default option: std ALL / ours EDGES_AND_FACES | ✅ (audit 1, 11-01) |
| D59 | routingCurve | csvUnit | P2 | options: std ['Centimeter', 'Foot', 'Inch', 'Millimeter', 'Meter', 'Yard'] / ours ['Meter', 'Centimeter', 'Millimeter', 'Inch', 'Foot', 'Yard'] | ✅ (audit 1, 11-01) |
| D60 | routingCurve | hasTargetLength | P2 | hints: std adds [], ours adds ['FIRST_IN_ROW'] | – std pairs a DISPLAY_SHORT boolean with the DISPLAY_SHORT field after it; ours marks the pair's first field FIRST_IN_ROW for the same one row |
| D61 | routingCurve | inputsGroup | P2 | missing: std has "Inputs" (a group) | ✅ (audit 2, 11-01) |
| D62 | routingCurve | points | P2 | array item fields differ: std [index "Point index" Integer 0..10000 =0.000000000000 / 0.000000000000 hints 0x8000 "" None, referenceType "Re… ours [referenceType "Reference" enum[("ORIGIN", "Origin"), ("VERTEX", "Vertex"), ("CURVE", "Curve"), ("RELATIVE", "… | ✅ (audit 9) |
| D63 | routingCurve | points | P2 | array row label: std Some("Point #index"), ours None | ✅ (audit 9) |
| D64 | routingCurve | segmentEditType | P2 | default option: std ADD / ours ADD_POINTS | ✅ (audit 1, 11-01) |
| D65 | routingCurve | selectedIndices | P2 | array row label: std Some("Selected point #indexValue"), ours None | ✅ (audit 7) |
| D66 | ruledSurface | vertexOverrides | P2 | array item fields differ: std [vertex "Vertex" query max Some(1) hints 0x2 "" None, overrideType "Ruled surface type" enum[("NORMAL", "Norma… ours [vertex "Vertex" query max Some(1) hints 0x2 "" None, overrideType "Ruled surface type" enum[("NORMAL", "Norma… | ✅ (audit 6) |
| D67 | ruledSurface | vertexOverrides | P2 | array row label: std Some("#vertex"), ours None | ✅ (audit 2, 11-01) |
| D68 | sheetMetalBend | oppositeAngleFromDirection | P2 | hints: std adds ['OPPOSITE_DIRECTION'], ours adds ['OPPOSITE_DIRECTION_CIRCULAR'] | ✅ (audit 2, 11-01) |
| D69 | sheetMetalBend | oppositeAngleFromDirection | P2 | label: std "Opposite direction angle", ours "Opposite angle" | ✅ (audit 2, 11-01) |
| D70 | sheetMetalBendRelief | bendReliefStyle | P2 | hints: std adds ['SHOW_LABEL'], ours adds [] | ✅ (audit 2, 11-01) |
| D71 | sheetMetalCorner | cornerStyle | P2 | hints: std adds ['SHOW_LABEL'], ours adds [] | ✅ (audit 2, 11-01) |
| D72 | sheetMetalCornerBreak | allowEdgeOverflow | P2 | hints: std adds ['GROUP_HEADER'], ours adds [] | ✅ (audit 2, 11-01) |
| D73 | sheetMetalCornerBreak | angle | P2 | hints: std adds [], ours adds ['CAN_BE_TOLERANT'] | ✅ (audit 5, 11-01) |
| D74 | sheetMetalCornerBreak | width | P2 | hints: std adds [], ours adds ['CAN_BE_TOLERANT'] | ✅ (audit 5, 11-01) |
| D75 | sheetMetalCornerBreak | width1 | P2 | hints: std adds [], ours adds ['CAN_BE_TOLERANT'] | ✅ (audit 5, 11-01) |
| D76 | sheetMetalCornerBreak | width2 | P2 | hints: std adds [], ours adds ['CAN_BE_TOLERANT'] | ✅ (audit 5, 11-01) |
| D77 | sheetMetalFlange | angleFromDirectionOppositeAngle | P2 | hints: std adds ['OPPOSITE_DIRECTION'], ours adds ['OPPOSITE_DIRECTION_CIRCULAR'] | ✅ (audit 2, 11-01) |
| D78 | sheetMetalFlange | endConditionsGroup | P2 | missing: std has "End conditions" (a group) | ✅ (audit 1, 11-01) |
| D79 | sheetMetalFlange | overallParametersGroup | P2 | missing: std has "Overall parameters" (a group) | ✅ (audit 1, 11-01) |
| D80 | sheetMetalFlange | secondBoundingEntityOffset | P2 | label: std "Second distance", ours "Distance" | ✅ (audit 2, 11-01) |
| D81 | sheetMetalHem | hemAlignment | P2 | hints: std adds ['REMEMBER_PREVIOUS_VALUE', 'SHOW_LABEL'], ours adds [] | ✅ (audit 2, 11-01) |
| D82 | sheetMetalHem | hemCornerType | P2 | hints: std adds ['REMEMBER_PREVIOUS_VALUE', 'SHOW_LABEL'], ours adds [] | ✅ (audit 2, 11-01) |
| D83 | sheetMetalJog | oppositeAngleFromDirection | P2 | hints: std adds ['OPPOSITE_DIRECTION'], ours adds ['OPPOSITE_DIRECTION_CIRCULAR'] | ✅ (audit 2, 11-01) |
| D84 | sheetMetalJog | oppositeAngleFromDirection | P2 | label: std "Opposite direction angle", ours "Opposite angle" | ✅ (audit 2, 11-01) |
| D85 | sheetMetalLoft | connections | P2 | array item fields differ: std [connectionEntities "Vertices or edges" query max None hints 0x0 "" None, isRip "Rip" bool=false hints 0x0 "" … ours [connectionEntities "Vertices or edges" query max None hints 0x0 "" None, isRip "Rip" bool=false hints 0x0 "" … | – the two extra std fields (Edge queries, Edge parameters) are ALWAYS_HIDDEN storage std's editing logic fills; nothing is shown |
| D86 | sheetMetalLoft | connections | P2 | array row label: std Some("#connectionEntities"), ours None | ✅ (audit 2, 11-01) |
| D87 | sheetMetalStart | hasSecondDirection | P2 | hints: std adds [], ours adds ['GROUP_HEADER'] | ✅ (audit 2, 11-01) |
| D88 | sheetMetalStart | offsetDistance | P2 | hints: std adds ['CAN_BE_TOLERANT'], ours adds [] | ✅ (audit 5, 11-01) |
| D89 | sheetMetalStart | secondDirectionDepth | P2 | hints: std adds [], ours adds ['CAN_BE_TOLERANT'] | ✅ (audit 5, 11-01) |
| D90 | sheetMetalStart | secondDirectionOffsetDistance | P2 | hints: std adds ['CAN_BE_TOLERANT'], ours adds [] | ✅ (audit 5, 11-01) |
| D91 | tag | additionalAlignmentPointsGroup | P2 | missing: std has "Additional alignment points" (a group) | ✅ (audit 1, 11-01) |
| D92 | tag | additionalColumns | P2 | array row label: std Some("#header = #value"), ours None | ✅ (audit 1, 11-01) |
| D93 | assignVariable | angleValue | P3 | hints: std adds ['INITIAL_FOCUS_ON_EDIT'], ours adds [] | ✅ (audit 6) |
| D94 | assignVariable | angleValue | P3 | value: std `Angle -1745.3292519943295..1745.3292519943295 =0.000000000000 / 0.000000000000` / ours `Angle -6.283185307179586..6.283185307179586 =0.000000000000 / 0.000000000000` | ✅ (audit 6) |
| D95 | assignVariable | axisMeasurements | P3 | extra: ours has "Axis measurements", std has no `axisMeasurements` | ✅ (audit 2, 11-01) |
| D96 | assignVariable | columnIndex | P3 | value: std `Integer 0..100000 =0.000000000000 / 0.000000000000` / ours `Integer 0..1000000 =0.000000000000 / 0.000000000000` | ✅ (audit 6) |
| D97 | assignVariable | columnLabelIndex | P3 | value: std `Integer 0..100000 =0.000000000000 / 0.000000000000` / ours `Integer 0..1000000 =0.000000000000 / 0.000000000000` | ✅ (audit 6) |
| D98 | assignVariable | columnMaxIndex | P3 | value: std `Integer 0..100000 =0.000000000000 / 0.000000000000` / ours `Integer 0..1000000 =0.000000000000 / 0.000000000000` | ✅ (audit 6) |
| D99 | assignVariable | columnMinIndex | P3 | value: std `Integer 0..100000 =0.000000000000 / 0.000000000000` / ours `Integer 0..1000000 =0.000000000000 / 0.000000000000` | ✅ (audit 6) |
| D100 | assignVariable | csv | P3 | kind: std `reference Csv` / ours `text=""` (our own picker) | ✅ (audit 10) |
| D101 | assignVariable | customOffset | P3 | value: std `Length 0..500 =0.000000000000 / 0.000000000000` / ours `Length -500..500 =0.000000000000 / 0.000000000000` | ✅ (audit 6) |
| D102 | assignVariable | distance | P3 | value: std `Length 0..500 =0.000000000000 / 0.000000000000` / ours `Length -500..500 =0.000000000000 / 0.000000000000` | ✅ (audit 6) |
| D103 | assignVariable | extendEntities | P3 | tooltip: std "Measure to infinitely extended lines, arcs, cylinders, etc.", ours "" | ✅ (audit 5, 11-01) |
| D104 | assignVariable | lengthValue | P3 | hints: std adds ['INITIAL_FOCUS_ON_EDIT'], ours adds [] | ✅ (audit 6) |
| D105 | assignVariable | measureFromAxis | P3 | tooltip: std "Measure to axis of cylinders, circles, arcs, or revolved surfaces", ours "" | ✅ (audit 5, 11-01) |
| D106 | assignVariable | measurementMode | P3 | tooltip: std "Select distance to measure the linear distance between two entities./nSelect len", ours "" | ✅ (audit 5, 11-01) |
| D107 | assignVariable | minmax | P3 | tooltip: std "Choose to measure minimum or maximum distance between entities", ours "" | ✅ (audit 5, 11-01) |
| D108 | assignVariable | numberValue | P3 | hints: std adds ['INITIAL_FOCUS_ON_EDIT'], ours adds [] | ✅ (audit 6) |
| D109 | assignVariable | rowIndex | P3 | value: std `Integer 0..100000 =0.000000000000 / 0.000000000000` / ours `Integer 0..1000000 =0.000000000000 / 0.000000000000` | ✅ (audit 6) |
| D110 | assignVariable | rowLabelIndex | P3 | value: std `Integer 0..100000 =0.000000000000 / 0.000000000000` / ours `Integer 0..1000000 =0.000000000000 / 0.000000000000` | ✅ (audit 6) |
| D111 | assignVariable | rowMaxIndex | P3 | value: std `Integer 0..100000 =0.000000000000 / 0.000000000000` / ours `Integer 0..1000000 =0.000000000000 / 0.000000000000` | ✅ (audit 6) |
| D112 | assignVariable | rowMinIndex | P3 | value: std `Integer 0..100000 =0.000000000000 / 0.000000000000` / ours `Integer 0..1000000 =0.000000000000 / 0.000000000000` | ✅ (audit 6) |
| D113 | assignVariable | xOffset | P3 | value: std `Length 0..500 =0.000000000000 / 0.000000000000` / ours `Length -500..500 =0.000000000000 / 0.000000000000` | ✅ (audit 6) |
| D114 | assignVariable | yOffset | P3 | value: std `Length 0..500 =0.000000000000 / 0.000000000000` / ours `Length -500..500 =0.000000000000 / 0.000000000000` | ✅ (audit 6) |
| D115 | assignVariable | zOffset | P3 | value: std `Length 0..500 =0.000000000000 / 0.000000000000` / ours `Length -500..500 =0.000000000000 / 0.000000000000` | ✅ (audit 6) |
| D116 | boundarySurface | uProfilesArray | P3 | array item "profile" (std) vs "Profile" (ours) | ✅ (audit 8) |
| D117 | boundarySurface | vProfilesArray | P3 | array item "profile" (std) vs "Profile" (ours) | ✅ (audit 8) |
| D118 | constrainedSurface | vertices | P3 | hints: std adds ['PREVENT_ARRAY_REORDER'], ours adds [] | ✅ (audit 7) |
| D119 | cutlist | columnOverrides | P3 | array item "Column Override" (std) vs "Column override" (ours) | ✅ (audit 7) |
| D120 | cutlist | columnOverrides | P3 | hints: std adds ['PREVENT_ARRAY_REORDER'], ours adds [] | ✅ (audit 7) |
| D121 | editCurve | controlPointEdits | P3 | array item "point override" (std) vs "Point override" (ours) | ✅ (audit 8) |
| D122 | editCurve | controlPointEdits | P3 | hints: std adds ['PREVENT_ARRAY_REORDER'], ours adds [] | ✅ (audit 7) |
| D123 | editCurve | editPointMode | P3 | option ids: std ['XYZ', 'UVN_TANGENT', 'UVN_CONTROL_POLYGON'] / ours ['XYZ', 'UVN_TANGENT', 'UVN_POLYGON'] | ✅ (audit 1, 11-01) |
| D124 | editCurve | planeReference | P3 | option ids: std ['BEST', 'YZPLANE', 'XZPLANE', 'XYPLANE', 'CUSTOM'] / ours ['BEST', 'YZ', 'XZ', 'XY', 'CUSTOM'] | ✅ (audit 1, 11-01) |
| D125 | editCurve | selectedIndices | P3 | array item "index" (std) vs "Index" (ours) | ✅ (audit 8) |
| D126 | editCurve | selectedIndices | P3 | hints: std adds ['PREVENT_ARRAY_REORDER', 'X25'], ours adds [] | ✅ (audit 7) |
| D127 | endcap | cornerDistance | P3 | hints: std adds ['REMEMBER_PREVIOUS_VALUE'], ours adds [] | ✅ (audit 6) |
| D128 | endcap | cornerRadius | P3 | hints: std adds ['REMEMBER_PREVIOUS_VALUE'], ours adds [] | ✅ (audit 6) |
| D129 | faceBlend | caps | P3 | array item "cap" (std) vs "Cap" (ours) | ✅ (audit 8) |
| D130 | faceBlend | crossSectionShape | P3 | tooltip: std "Cross sectional control", ours "" | ✅ (audit 5, 11-01) |
| D131 | faceBlend | edgeLimits | P3 | array item "edge limit" (std) vs "Edge limit" (ours) | ✅ (audit 8) |
| D132 | fill | edges | P3 | array item "edge" (std) vs "Edge" (ours) | ✅ (audit 8) |
| D133 | frame | cornerOverrides | P3 | array item "vertex" (std) vs "Corner" (ours) | ✅ (audit 7) |
| D134 | frame | profileSketch | P3 | kind: std `reference PartStudio` / ours `text=""` (our own picker) | ✗ |
| D135 | frame | selections | P3 | tooltip: std "Faces, edges, and vertices that define sweep paths", ours "" | ✅ (audit 5, 11-01) |
| D136 | frame | trimBodies | P3 | tooltip: std "Parts to use as trim tools", ours "" | ✅ (audit 5, 11-01) |
| D137 | frame | trimPlanes | P3 | tooltip: std "Planes and planar faces to use as trim tools", ours "" | ✅ (audit 5, 11-01) |
| D138 | frameTrim | frameGroups | P3 | array item "group" (std) vs "Group" (ours) | ✅ (audit 8) |
| D139 | frameTrim | frameGroups | P3 | tooltip: std "Frame segments in earlier groups are used as tools for later groups", ours "" | ✅ (audit 5, 11-01) |
| D140 | frameTrim | targets | P3 | tooltip: std "Parts to trim using trim tool", ours "" | ✅ (audit 5, 11-01) |
| D141 | frameTrim | tool | P3 | tooltip: std "Face, plane, mate connector, or planar face to use as trim tool", ours "" | ✅ (audit 5, 11-01) |
| D142 | gusset | edges | P3 | tooltip: std "Edges that define the bases of the gussets", ours "" | ✅ (audit 5, 11-01) |
| D143 | helix | actualPitch | P3 | value: std `Length 0.00001..500 =0.025000000000 / 0.025400000000` / ours `Length 0..500 =0.025000000000 / 0.025400000000` | ✅ (audit 6) |
| D144 | helix | actualRevolutions | P3 | value: std `Real 0.00001..1000 =4.000000000000 / 4.000000000000` / ours `Real 0..1000000000 =4.000000000000 / 4.000000000000` | ✅ (audit 6) |
| D145 | importDerived | partStudio | P3 | kind: std `reference PartStudio` / ours `text=""` (our own picker) | – drawn by the same reference field live's reference parameters use (`render_reference_field`: type icon, "Select Part Studio…", the 300 px picker); the value is the tab id as a reference's is |
| D146 | isoparametricCurve | directionType | P3 | option ids: std ['U_DIRECTION', 'V_DIRECTION'] / ours ['U', 'V'] | ✅ (audit 1, 11-01) |
| D147 | mateConnector | originAdditionalQuery | P3 | hints: std adds ['UNCONFIGURABLE'], ours adds [] | ✅ (audit 6) |
| D148 | offsetCurveOnFace | scope | P3 | option ids: std ['OFFSET', 'OFFSET_AND_EXTEND', 'OFFSET_EXTEND_AND_SPLIT'] / ours ['OFFSET', 'OFFSET_EXTEND', 'OFFSET_EXTEND_SPLIT'] | ✅ (audit 1, 11-01) |
| D149 | pcbEnd | entitiesToUnfold | P3 | hints: std adds ['FOCUS_ON_VISIBLE'], ours adds [] | ✅ (audit 6) |
| D150 | pcbStart | bendsIncluded | P3 | tooltip: std "Check to include the clearance for bends", ours "" | ✅ (audit 5, 11-01) |
| D151 | queryVariable | additionalQueries | P3 | array item "additional query" (std) vs "Additional query" (ours) | ✅ (audit 8) |
| D152 | queryVariable | edgeAdjacentEntityType | P3 | option ids: std ['ALL', 'EDGE', 'FACE'] / ours ['EDGES_AND_FACES', 'EDGES', 'FACES'] | ✅ (audit 1, 11-01) |
| D153 | queryVariable | vertexAdjacentEntityType | P3 | option ids: std ['ALL', 'EDGE', 'VERTEX', 'FACE'] / ours ['ALL', 'EDGES', 'VERTICES', 'FACES'] | ✅ (audit 1, 11-01) |
| D154 | routingCurve | addPointOnAxisDrag | P3 | hints: std adds ['UNCONFIGURABLE'], ours adds [] | ✅ (audit 6) |
| D155 | routingCurve | csvUnit | P3 | hints: std adds ['UNCONFIGURABLE'], ours adds [] | ✅ (audit 2, 11-01) |
| D156 | routingCurve | csvUnit | P3 | option ids: std ['Centimeter', 'Foot', 'Inch', 'Millimeter', 'Meter', 'Yard'] / ours ['Meter', 'Centimeter', 'Millimeter', 'Inch', 'Foot', 'Yard'] | ✅ (audit 1, 11-01) |
| D157 | routingCurve | curveBaseCSys | P3 | tooltip: std "Point or mate connector used as reference for the curve. Defaults to origin.", ours "" | ✅ (audit 5, 11-01) |
| D158 | routingCurve | inputCurve | P3 | hints: std adds ['UNCONFIGURABLE'], ours adds [] | ✅ (audit 2, 11-01) |
| D159 | routingCurve | inputData | P3 | kind: std `reference Csv` / ours `text=""` (our own picker) | ✅ (audit 10) |
| D160 | routingCurve | inputType | P3 | hints: std adds ['UNCONFIGURABLE'], ours adds [] | ✅ (audit 2, 11-01) |
| D161 | routingCurve | inputVertices | P3 | hints: std adds ['UNCONFIGURABLE'], ours adds [] | ✅ (audit 2, 11-01) |
| D162 | routingCurve | keepPoints | P3 | tooltip: std "Create points in addition to the curve.", ours "" | ✅ (audit 5, 11-01) |
| D163 | routingCurve | numPointsToAdd | P3 | hints: std adds ['UNCONFIGURABLE'], ours adds [] | ✅ (audit 6) |
| D164 | routingCurve | orthoCsys | P3 | hints: std adds ['UNCONFIGURABLE'], ours adds [] | ✅ (audit 6) |
| D165 | routingCurve | orthoCsysMate | P3 | hints: std adds ['UNCONFIGURABLE'], ours adds [] | ✅ (audit 6) |
| D166 | routingCurve | overwritePoints | P3 | hints: std adds ['UNCONFIGURABLE'], ours adds [] | ✅ (audit 2, 11-01) |
| D167 | routingCurve | points | P3 | array item "point" (std) vs "Point" (ours) | ✅ (audit 8) |
| D168 | routingCurve | points | P3 | hints: std adds ['PREVENT_ARRAY_REORDER'], ours adds [] | ✅ (audit 7) |
| D169 | routingCurve | pointsEditType | P3 | hints: std adds ['UNCONFIGURABLE'], ours adds [] | ✅ (audit 6) |
| D170 | routingCurve | pointsEditType | P3 | option ids: std ['SINGLE', 'MULTI'] / ours ['SINGLE', 'MULTIPLE'] | ✅ (audit 1, 11-01) |
| D171 | routingCurve | samples | P3 | hints: std adds ['UNCONFIGURABLE'], ours adds [] | ✅ (audit 2, 11-01) |
| D172 | routingCurve | segmentEditType | P3 | hints: std adds ['UNCONFIGURABLE'], ours adds [] | ✅ (audit 6) |
| D173 | routingCurve | segmentEditType | P3 | option ids: std ['ADD', 'ORTHOGONAL'] / ours ['ADD_POINTS', 'ORTHOGONAL_PATH'] | ✅ (audit 1, 11-01) |
| D174 | routingCurve | selectedIndices | P3 | array item "index" (std) vs "Point" (ours) | ✅ (audit 7) |
| D175 | routingCurve | selectedIndices | P3 | hints: std adds ['PREVENT_ARRAY_REORDER', 'X25'], ours adds [] | ✅ (audit 7) |
| D176 | routingCurve | showCSys | P3 | tooltip: std "Show coordinate system of the reference used by the current vertex.", ours "" | ✅ (audit 5, 11-01) |
| D177 | routingCurve | startColumn | P3 | hints: std adds ['UNCONFIGURABLE'], ours adds [] | ✅ (audit 2, 11-01) |
| D178 | routingCurve | targetLength | P3 | value: std `Length -500..500 =0.025000000000 / 0.025400000000` / ours `Length 0.00001..500 =0.025000000000 / 0.025400000000` | ✅ (audit 6) |
| D179 | ruledSurface | cornerType | P3 | option ids: std ['SPUN', 'EXTENDED', 'LOFTED', 'NO_CORNER'] / ours ['SPUN', 'EXTENDED', 'LOFTED', 'NONE'] | ✅ (audit 1, 11-01) |
| D180 | ruledSurface | vertexOverrides | P3 | array item "arrayItem" (std) vs "Vertex" (ours) | ✅ (audit 8) |
| D181 | sheetMetalCorner | cornerReliefScale | P3 | hints: std adds [], ours adds ['REMEMBER_PREVIOUS_VALUE'] | ✅ (audit 6) |
| D182 | sheetMetalCornerBreak | crossSection | P3 | tooltip: std "Cross sectional control", ours "" | ✅ (audit 5, 11-01) |
| D183 | sheetMetalCornerBreak | filletWidth | P3 | hints: std adds ['REMEMBER_PREVIOUS_VALUE'], ours adds [] | ✅ (audit 6) |
| D184 | sheetMetalCornerBreak | otherRadius | P3 | hints: std adds ['REMEMBER_PREVIOUS_VALUE'], ours adds [] | ✅ (audit 6) |
| D185 | sheetMetalFlange | partialEndGroup | P3 | extra: ours has "End conditions", std has no `partialEndGroup` | ✅ (audit 1, 11-01) |
| D186 | sheetMetalFlange | partialOverallGroup | P3 | extra: ours has "Overall parameters", std has no `partialOverallGroup` | ✅ (audit 1, 11-01) |
| D187 | sheetMetalFormed | formPartStudio | P3 | kind: std `reference PartStudio` / ours `text=""` (our own picker) | – drawn by the same reference field live's reference parameters use (`render_reference_field`: type icon, "Select Part Studio…", the 300 px picker); the value is the tab id as a reference's is |
| D188 | sheetMetalHem | angle | P3 | hints: std adds ['REMEMBER_PREVIOUS_VALUE'], ours adds [] | ✅ (audit 6) |
| D189 | sheetMetalHem | innerRadius | P3 | hints: std adds ['REMEMBER_PREVIOUS_VALUE'], ours adds [] | ✅ (audit 6) |
| D190 | sheetMetalHem | length | P3 | hints: std adds ['REMEMBER_PREVIOUS_VALUE'], ours adds [] | ✅ (audit 6) |
| D191 | sheetMetalHem | tipGap | P3 | hints: std adds ['REMEMBER_PREVIOUS_VALUE'], ours adds [] | ✅ (audit 6) |
| D192 | sheetMetalHem | useMinimalGapForStraight | P3 | hints: std adds ['REMEMBER_PREVIOUS_VALUE'], ours adds [] | ✅ (audit 6) |
| D193 | sheetMetalHem | useMinimalGapForTipGap | P3 | hints: std adds ['REMEMBER_PREVIOUS_VALUE'], ours adds [] | ✅ (audit 6) |
| D194 | sheetMetalJog | bendAngle | P3 | hints: std adds ['UNCONFIGURABLE'], ours adds [] | ✅ (audit 6) |
| D195 | sheetMetalJog | bendAngle | P3 | value: std `Angle 0.017453292519943295..6.281439977927592 =1.570796326795 / 1.570796326795` / ours `Angle 0.017453292519943295..3.12413936106985 =1.570796326795 / 1.570796326795` | ✅ (audit 6) |
| D196 | sheetMetalLoft | connections | P3 | array item "connection" (std) vs "Connection" (ours) | ✅ (audit 8) |
| D197 | sheetMetalStart | bendsIncluded | P3 | tooltip: std "Check to include the clearance for bends", ours "" | ✅ (audit 5, 11-01) |
| D198 | tag | additionalColumns | P3 | hints: std adds ['PREVENT_ARRAY_REORDER'], ours adds ['UNCONFIGURABLE'] | ✅ (audit 1, 11-01) |
| D199 | tag | cSysMateConnector | P3 | tooltip: std "If none selected, this feature will create one at Origin", ours "" | ✅ (audit 5, 11-01) |

New rows since the first run:

| # | Feature | Field | P | Difference | Status |
|---|---|---|---|---|---|
| N1 | routingCurve | orthoPrevious | P2 | hints: std adds [], ours adds ['FIRST_IN_ROW'] | – std pairs a DISPLAY_SHORT boolean with the DISPLAY_SHORT field after it; ours marks the pair's first field FIRST_IN_ROW for the same one row |

### Visibility states that differ (current run)

| # | Feature | P | States | Status |
|---|---|---|---|---|
| V1 | importDerived | P2 | shown fields differ in ["newUI", "placement=AT_MATE_CONNECTOR", "includeMateConnectors", "includeProperties"]: shown only by std [], only by builtin ["mateConnectorId"] | – the field is std's too, declared `isAnything` (variable.fs 200 / 291, importDerived.fs 98), which the oracle cannot read |
| V2 | assignVariable | P2 | shown fields differ in ["mode=ASSIGNED", "variableType=ANY", "measurementMode=DISTANCE", "rowGroup", "rowAccessType=INDEX", "rowResult=SINGLE", "columnGroup", "columnAccessType=IND …: shown only by std [], only by builtin ["anyValue"] | – the field is std's too, declared `isAnything` (variable.fs 200 / 291, importDerived.fs 98), which the oracle cannot read |
| V3 | assignVariable | P2 | shown fields differ in ["mode=TABLE", "variableType=LENGTH", "measurementMode=DISTANCE", "rowGroup", "rowAccessType=INDEX", "rowResult=SINGLE", "columnGroup", "columnAccessType=IND …: shown only by std [], only by builtin ["conversionFactor"] | – the field is std's too, declared `isAnything` (variable.fs 200 / 291, importDerived.fs 98), which the oracle cannot read |
| V4 | assignVariable | P2 | shown fields differ in ["mode=TABLE", "variableType=LENGTH", "measurementMode=DISTANCE", "rowGroup", "rowAccessType=LABEL", "rowResult=ARRAY", "columnGroup", "columnAccessType=ALL" …: shown only by std [], only by builtin ["conversionFactor"] | – the field is std's too, declared `isAnything` (variable.fs 200 / 291, importDerived.fs 98), which the oracle cannot read |
| V5 | assignVariable | P2 | shown fields differ in ["mode=TABLE", "variableType=LENGTH", "measurementMode=LENGTH", "rowGroup", "rowAccessType=INDEX_RANGE", "rowResult=ARRAY", "columnGroup", "columnAccessType= …: shown only by std [], only by builtin ["conversionFactor"] | – the field is std's too, declared `isAnything` (variable.fs 200 / 291, importDerived.fs 98), which the oracle cannot read |
| V6 | assignVariable | P2 | shown fields differ in ["mode=TABLE", "variableType=NUMBER", "measurementMode=LENGTH", "rowGroup", "rowAccessType=INDEX", "rowRegexp", "rowResult=MAP", "columnAccessType=INDEX_RANG …: shown only by std [], only by builtin ["conversionFactor"] | – the field is std's too, declared `isAnything` (variable.fs 200 / 291, importDerived.fs 98), which the oracle cannot read |
| V7 | assignVariable | P2 | shown fields differ in ["mode=TABLE", "variableType=LENGTH", "measurementMode=LENGTH", "rowAccessType=INDEX", "rowRegexp", "rowResult=MAP", "columnGroup", "columnAccessType=ALL", " …: shown only by std [], only by builtin ["conversionFactor"] | – the field is std's too, declared `isAnything` (variable.fs 200 / 291, importDerived.fs 98), which the oracle cannot read |
| V8 | assignVariable | P2 | shown fields differ in ["mode=TABLE", "variableType=NUMBER", "measurementMode=DISTANCE", "rowGroup", "rowAccessType=INDEX", "rowRegexp", "rowResult=SINGLE", "columnAccessType=INDEX …: shown only by std [], only by builtin ["conversionFactor"] | – the field is std's too, declared `isAnything` (variable.fs 200 / 291, importDerived.fs 98), which the oracle cannot read |
| V9 | assignVariable | P2 | shown fields differ in ["mode=TABLE", "variableType=ANY", "measurementMode=DIAMETER", "rowGroup", "rowAccessType=INDEX_RANGE", "rowRegexp", "rowResult=SINGLE", "columnGroup", "colu …: shown only by std [], only by builtin ["conversionFactor"] | – the field is std's too, declared `isAnything` (variable.fs 200 / 291, importDerived.fs 98), which the oracle cannot read |
| V10 | assignVariable | P2 | shown fields differ in ["mode=TABLE", "variableType=ANY", "measurementMode=LENGTH", "rowAccessType=INDEX_RANGE", "rowRegexp", "rowResult=ARRAY", "columnAccessType=INDEX", "columnRe …: shown only by std [], only by builtin ["conversionFactor"] | – the field is std's too, declared `isAnything` (variable.fs 200 / 291, importDerived.fs 98), which the oracle cannot read |
| V11 | assignVariable | P2 | shown fields differ in ["mode=TABLE", "variableType=NUMBER", "measurementMode=DISTANCE", "rowGroup", "rowAccessType=INDEX", "rowResult=SINGLE", "columnGroup", "columnAccessType=IND …: shown only by std [], only by builtin ["conversionFactor"] | – the field is std's too, declared `isAnything` (variable.fs 200 / 291, importDerived.fs 98), which the oracle cannot read |
| V12 | assignVariable | P2 | shown fields differ in ["mode=TABLE", "variableType=ANGLE", "measurementMode=DISTANCE", "rowGroup", "rowAccessType=INDEX_RANGE", "rowResult=SINGLE", "columnAccessType=INDEX_RANGE", …: shown only by std [], only by builtin ["conversionFactor"] | – the field is std's too, declared `isAnything` (variable.fs 200 / 291, importDerived.fs 98), which the oracle cannot read |
| V13 | assignVariable | P2 | shown fields differ in ["mode=TABLE", "variableType=ANGLE", "measurementMode=LENGTH", "rowAccessType=LABEL", "rowResult=MAP", "columnGroup", "columnAccessType=LABEL", "columnRegexp …: shown only by std [], only by builtin ["conversionFactor"] | – the field is std's too, declared `isAnything` (variable.fs 200 / 291, importDerived.fs 98), which the oracle cannot read |
