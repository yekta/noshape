# Part Studio behaviour parity with Onshape

Checklist of Onshape Part Studio behaviours (selection, dialogs, feature
list, view, shortcuts, menus, evaluation tools) and their state in noshape.
Sketch-mode behaviour is tracked by the sketch work; feature *geometry*
(what each feature computes) is tracked in `research/sections/*-parity.md`.

Sources are Onshape Help pages in `/root/onshape-research/docs-raw/md/`
(named by page, e.g. `Home__selection`), the research section
`A-partstudio-ui.md` (§ numbers) and the web-client bundle
(`bundle-command-actions.txt`, `bundle-i18n-strings.txt`).

Status: ✅ works · 🔧 fixed/added in this pass · ⏳ partial · ❌ missing.
Tested = driven in the app on Xvfb (xdotool) and/or covered by unit tests
(`crates/model/src/interaction.rs`, `crates/app/src/part_studio/view_cube.rs`,
`behaviors.rs`).

## 1. Keyboard shortcuts: General (Home__keyboard_shortcuts_and_hotkeys)

| # | Behaviour | Source | Status |
|---|---|---|---|
| 1.1 | Enter commits the open dialog (in a number field whose text changed, the first Enter applies the value and selects it, the second commits) | keyboard_shortcuts; live | ✅ (two-stage Enter per live capture) |
| 1.2 | Enter works after clicking the feature list/viewport (focus left the dialog) | keyboard_shortcuts | ✅ (app 10-24: list clicked, Enter committed Extrude 2) |
| 1.3 | Enter with a feature selected in the list (no dialog) opens it for editing | A §3.3 (implied by Edit) | ✅ (app 10-24) |
| 1.4 | Shift+Enter commits and reopens the same tool, empty | keyboard_shortcuts; Home__dialogs | ✅ |
| 1.5 | Esc exits the command / cancels the dialog | keyboard_shortcuts | ✅ |
| 1.6 | Esc in a changed dialog field reverts the typed value; the next Esc cancels the dialog | live (behaviours.md: Feature dialog basics) | ✅ |
| 1.7 | Esc closes transient UI first (S toolbar, Select other, zoom-window mode, the context menu), then exits isolate/transparent; it never clears the selection (live 10-27: Space does) | View__isolate; select_other | ✅ |
| 1.8 | Space deselects everything (features and entities) | keyboard_shortcuts | ✅ (app 10-24) |
| 1.9 | Ctrl/⌘+C copies selected features | keyboard_shortcuts | ✅ |
| 1.10 | Ctrl/⌘+V pastes copied features (repeatable, at the rollback bar) | keyboard_shortcuts | ✅ |
| 1.11 | Delete/Backspace removes selected features | keyboard_shortcuts | ✅ |
| 1.12 | Delete with parts selected opens the pre-filled "Delete part N" dialog (Delete composite parts, the parts' chips), the parts go from the list and show as a pale ghost (live 10-27) | A §3.9; Home__selection | ✅ (live outlines the ghost (255,192,76); ours has no outline) |
| 1.13 | Ctrl/⌘+Z / Ctrl+Y / ⇧⌘Z undo/redo; inside a dialog they step the dialog's own edits | keyboard_shortcuts; A §2.1 | ✅ |
| 1.14 | Shift+N renames the selected feature in place | keyboard_shortcuts | ✅ |
| 1.15 | Alt/⌥+C opens Search tools | keyboard_shortcuts; Home__search_tools | ✅ |
| 1.16 | Shift+/ toggles the keyboard shortcut map | keyboard_shortcuts | ✅ |
| 1.17 | Shift+/ opens Onshape's shortcut sheet: tabs General, Part Studio, Assembly, 3D view, Sketch, Feature Studio, Drawing; key chips + action, 🔒 locked; search across tabs; legend | live (shortcuts/, dom/keyboard-shortcuts.txt) | ✅ (the live 114-row list) |
| 1.18 | ` (grave) opens Select other at the cursor | keyboard_shortcuts; Home__select_other | ✅ (psui 10-27, as live "spec: psui part-studio.md … Select other": the list at the pointer + (100, 51), each part after its nearest face, row 2 bold and pre-highlighted) |
| 1.19 | ` again moves down the Select other list, Shift+` up, Enter picks | Home__select_other | ✅ (psui 10-27: ` and ↓ step down and wrap, Enter picks; live's Shift+` does nothing, so ours no longer steps back on it) |
| 1.20 | S opens the shortcut toolbar at the cursor: 5x2 tools (Part Studio: Sketch, Extrude, Revolve, Sweep, Loft / Fillet, Chamfer, Hole, Boolean, Transform) | live (shortcuts/16) | ✅ |
| 1.21 | S toolbar contents customizable in preferences | part_studios | ✅ (Customize toolbar > Shortcut toolbar (S) checklist; saved in the config dir) |
| 1.22 | [ opens the Measure tool | keyboard_shortcuts | ✅ (psui 10-27: [ opens Measure at the bottom right, Esc closes it, as live) |
| 1.23 | Ctrl+M opens the Mate connector dialog | keyboard_shortcuts | ✅ (live 10-27: "Mate connector 1" red, Origin entity active, ☑ Owner entity with an empty owner field, Attachment None, one part or several; a picked origin then fills the owner) |
| 1.24 | K shows/hides mate connectors | keyboard_shortcuts | 🔧 (toggles the Mate connector features' visibility) |
| 1.25 | A flips the primary axis, Q reorients the secondary axis while editing a mate connector | keyboard_shortcuts | ✅ |
| 1.26 | Shift+C Curve/surface analysis (curvature combs on edges, face U/V isocurves, sketch curves; inflection points, minimum radius, extend across face, control point grids, scale), Shift+D Dihedral analysis (combs, Max/Min, thresholds, scale); Analysis tools menu | keyboard_shortcuts; View__curve_surface_analysis; View__dihedral_analysis; View__analysis_tools | ⏳ (Curve/surface, Deviation, Connection, Dihedral, Zebra stripes, Reflection, Curvature color map (Onshape's color bar: Viridis / Blue - > Red / Plasma / Rainbow, Gaussian / Mean / Max radius / Min radius from mesh principal curvatures, flat faces keep their color), Thickness analysis (ray / rolling ball / gradients, color bar, Dim color scale, bounds, Edit / Turn off) ✅ verified in the app; Draft analysis… as Onshape's dialog (Mold split direction + flip, Minimum draft angle, Entities to check, Show undercut regions by ray casting; legend Steep / Undercut / Side 1 / Side 2 at the top, side-1 arrow, angle under the cursor) built; Reflection's library maps are procedural stand-ins until live/env-maps is captured and its "Other documents" tab is empty (no image tabs yet); Flatten surfaces (unfold + relaxation, rips and seams, translucent patch in the Top plane, facets / edges / checkerboard / distortion, Origin, mate connector position, offset / angle, STL / DXF / SVG export) ✅; Interference detection… (pairwise overlaps in red, parts translucent, Interferences list with bounding box and size on hover) built, dialog details await the live capture; menu labels and order as live (with "…"); Lock environment = fixed to the model (live); zebra pattern = latitudes / longitudes of a room around the model (rings and radial stripes on flat faces, live); our own environment maps modeled on Onshape's; Flatten's PARASOLID export writes the flat sheet with the kernel's Parasolid writer; Thickness palettes beyond Viridis are guesses (not on the free account)) |
| 1.27 | Alt/⌥+T tab manager (search, open), Ctrl+Space switches to the previously active tab | keyboard_shortcuts | ✅ (psui 10-27: Alt+T opens and closes the Tab manager; Esc no longer closes it, as live) |
| 1.28 | Ctrl+U feedback, Ctrl+Shift+F FeatureScript search | keyboard_shortcuts | — (not Part Studio: Ctrl+Shift+F finds text in the current Feature Studio (Help: keyboard shortcuts), the Feature Studio editor's job; Ctrl+U is cloud feedback, out of scope) |
| 1.29 | Bare-letter shortcuts do not fire while typing in an input | (Onshape behaviour) | ✅ |

## 2. Keyboard shortcuts: Part Studio and 3D view

| # | Behaviour | Source | Status |
|---|---|---|---|
| 2.1 | Shift+S new sketch ("Select a sketch plane"); with a face/plane selected it fills the sketch plane | keyboard_shortcuts; A §2.3 | ✅ |
| 2.2 | Shift+E extrude; in a sketch it finishes the sketch and takes all of its regions | keyboard_shortcuts | ✅ |
| 2.3 | Shift+W revolve (same rules) | keyboard_shortcuts | ✅ |
| 2.4 | Shift+F fillet (selected edges preselected) | keyboard_shortcuts | ✅ |
| 2.5 | Shift+H shows/hides sketches | keyboard_shortcuts | ✅ |
| 2.6 | Rollback bar selected + ↑/↓ steps the bar | keyboard_shortcuts; features_and_parts_lists | ✅ (= 6.33) |
| 2.7 | Shift+1…7 Front, Back, Left, Right, Top, Bottom, Isometric | keyboard_shortcuts | ✅ |
| 2.8 | N view normal to the hovered/selected plane, face or sketch; N again flips to the reverse | keyboard_shortcuts | ✅ |
| 2.9 | F zoom to fit: the world box of the visible parts/sketches, the origin point when shown and the default planes (150 mm, ±75) when shown, its 8 corners centred, half-height 1.05 × max(h, w/aspect), in both projections (perspective: eye at padded half-height / tan(fov/2) + padded half-depth); a Part Studio opens at Trimetric fitted to its parts and the planes' box even when hidden (live "Zoom to fit (perf 10-23 22:40)"; navigation `zoom_to_fit_frames_like_onshape`) | keyboard_shortcuts, live | ✅ |
| 2.10 | Z zoom out, Shift+Z zoom in (about the view center) | keyboard_shortcuts | ✅ |
| 2.11 | W zoom to window: drag a box to magnify; W again cancels | keyboard_shortcuts; View__zoom_to_fit | ✅ (psui 10-27, as live "spec: psui part-studio.md 2.11": the zoom-in cursor and the "Drag to create zoom bounding box." bar while armed, the box fills the view, W again cancels) |
| 2.12 | ←→↑↓ rotate 15°, Ctrl 5°, Shift 90° | keyboard_shortcuts | ✅ |
| 2.13 | Ctrl+Shift+←→↑↓ pan (150 px per step) | keyboard_shortcuts; A §6.1 | ✅ (psui 10-27: 150 px per press, as live) |
| 2.14 | P shows/hides planes | keyboard_shortcuts | ✅ |
| 2.15 | Shift+P hides all construction geometry (planes, sketches); again shows it | keyboard_shortcuts | ✅ (app 10-24, = sketch 1.17) |
| 2.16 | Y hides the hovered entity's part/sketch/plane (or the selection) | keyboard_shortcuts; View__hide_show | ✅ (psui 10-27: Y hides the part under the pointer, as live) |
| 2.17 | Shift+Y shows hidden entities (selected ones, else all parts) | keyboard_shortcuts; View__hide_show | ✅ (psui 10-27: with nothing selected, Shift+Y shows only the hidden part under the pointer; over empty graphics nothing, as live) |
| 2.18 | Shift+I isolates the hovered/selected parts (others at ~20 %) and clears the selection; over a faded part it does nothing (live 10-27: not cumulative by hover) | keyboard_shortcuts; View__isolate | ✅ |
| 2.19 | Shift+T makes the hovered/selected parts transparent (0.29 of the colour over white, Part 1 top (222,235,243) as live 10-27); cumulative by hover | keyboard_shortcuts; View__make_transparent | ✅ |
| 2.20 | Shift+X opens the Section view dialog (again with a section on: turns it off) | keyboard_shortcuts | ✅ |
| 2.21 | Shift+V named views | keyboard_shortcuts | ✅ |
| 2.22 | Shift+R view in high quality (4x finer tessellation, notice) | keyboard_shortcuts | ✅ 10-27: the bar at live's exact box 866–1298 × 76–103 (notice = live's 433×28 #90cef1 bar at the graphics top centre, 13 px text, white Disable button; part-studio/lod4-fill-shiftR) |
| 2.23 | Alt+click selects through transparent geometry | keyboard_shortcuts | 🔧 (isolate/transparent and translucent part appearances) |
| 2.24 | Perspective view only from the camera menu (no key, like Onshape; O was a noshape extra, removed 10-23); off in every new graphics area, per Part Studio tab for the session (no Set as default, no preference: live 10-23 a2); field of view 45° vertical (Graphics preferences) in Part Studios and assemblies | View__perspective_view | ✅ |

## 3. Selection (Home__selection, Home__select_other, A §5)

| # | Behaviour | Source | Status |
|---|---|---|---|
| 3.1 | Click selects, click again deselects; selection is additive without modifiers | Home__selection | ✅ |
| 3.2 | Click in empty space clears the selection (entities and features) | Home__selection | ✅ (app 10-24) |
| 3.3 | Hover pre-highlights the entity under the cursor | A §5.3 | ✅ |
| 3.4 | Box select left→right = window (fully inside), right→left = crossing | Home__selection | ✅ |
| 3.5 | Window box is solid, crossing box dashed | Home__selection; A §5.2 | ✅ |
| 3.6 | Box select: left→right takes what is wholly inside (faces and edges, no parts or part vertices), right→left what it touches; Ctrl+drag box deselects (live 10-27) | Home__selection | ✅ (live 10-27: window box #1651B0 20 % fill / 60 % solid border, crossing #FFC20E 20 % / 60 % dashed 9 on 8 off, both ~1.5 px; ours within 1 level) |
| 3.7 | Box selection with a dialog open fills the active field (filtered) | A §5.4 | 🔧 |
| 3.8 | Selected entities show quick values in the Measure readout | A §5.1 | ✅ |
| 3.9 | Selection count next to the cursor, 1–5 ("5" for five or more) | Home__selection; live cursors/ | ✅ (Onshape bakes the digit into the cursor image; GPUI has no image cursors, so a black digit with a white outline is drawn at the arrow's lower right) |
| 3.10 | Midpoints of edges show on hover and can be picked (measure, planes) | Home__selection | ✅ (part edges and open sketch curves; with a dialog open when its field takes points. Look as live qa4-r5-s9-6: hovering the edge shows a 9 px dark see-through ring at its midpoint (render point style RING); a selected midpoint is an 11 px #fac012 disc with a #c4960b ring and #ffd65b centre; a hovered midpoint target draws the same marker in orange #f8c488/#c49a6a/#ffd8ac. Within 9 px of the marker the midpoint is the hover target and the edge highlight goes, as live (checked 10-27).) |
| 3.11 | Select other lists faces and edges under the cursor nearest first; hovering a row highlights it; click/Enter picks; list closes | Home__select_other | ✅ (psui 10-27, see 1.18; a row click picks it, Esc closes) |
| 3.12 | Select other picks go to the active dialog field | Home__select_other | 🔧 |
| 3.13 | Select other from the face/edge context menu | Home__select_other | 🔧 |
| 3.14 | Selecting a face or part highlights the features that created/modified it in the feature list, and its part in the Parts list (related-highlight (236,240,244), live 10-27) | features_and_parts_lists | ✅ |
| 3.15 | Hovering a feature in the list highlights its geometry | features_and_parts_lists ("Hover over a feature…") | ✅ (app 10-24) |
| 3.16 | Selecting a feature in the list highlights its geometry (its part amber) and relates its part row (live 10-27) | A §5.3 (FEATURE_HIGHLIGHT) | ✅ |
| 3.17 | Selecting / hovering a part in the parts list highlights the whole part | A §5.3 (PART_HIGHLIGHT) | ✅ (10-27; row to row the hover no longer drops) |
| 3.18 | Double-click a sketch in the graphics area edits it | (Onshape behaviour) | 🔧 |
| 3.19 | Create selection dialog and Select ▸ submenu | Home__create_selection; live part-studio/380–398, dom/dialog-create-selection.txt, context-menus-ps-part-select-submenus.txt | 🔧 (Select ▸: face/region = Protrusion, Pocket, Hole, Fillet, Tangent connected, Bounded faces, Loop/chain connected edges, Create selection…; part edge and sketch curve = Tangent connected, Equal length/radius, Parallel, Create selection…; no separator; each item replaces the selection. Create selection (`part_studio/create_selection.rs`): 220 px, floats at the right-click point, header ✕ only, draggable; Faces / Edges tabs (opens on Faces), native-style select (Protrusion…Bounded faces / Tangent connected, Loop/chain connected, Equal length/radius, Parallel), Selection field (empty at open, picks toggle), ☐ Select patterns (pattern copies via `select::with_pattern_instances`), "N faces selected", [Add selection] (disabled at 0) adds to the selection and resets, staying open; preview seeds orange / results query yellow-green. Feature fields with SHOW_CREATE_SELECTION (Delete/Move/Replace face, Offset surface, Draft parting edges, face patterns / mirror) and Flatten's two fields have the 26 px button (tooltip "Create selection"); it opens the dialog beside the feature dialog, the field turns white with a blue border, Add selection fills the field. Convexity follows qConvexConnectedFaces (exterior fillet edges convex). Verified in the app.) |

## 4. Preselection and tools (A §2.3, PartStudio__feature_basics)

| # | Behaviour | Source | Status |
|---|---|---|---|
| 4.1 | Entities selected before starting a feature fill its first compatible field | A §2.3 | ✅ |
| 4.2 | A sketch selected in the feature list preselects all its regions (Extrude, Revolve…) | features_and_parts_lists | ✅ (app 10-24: "Face of Sketch 2" preselected) |
| 4.3 | Planes / features selected in the list preselect plane / feature fields | A §2.3 | ✅ (10-27: Top in the list → Plane 1's "Top plane"; Sketch 1's row → Extrude's "Face of Sketch 1") |
| 4.4 | Clicking the active toolbar tool again (or its shortcut) leaves its dialog open as it is (live 10-27; only ✓ / ✕ / Esc close it) | Home__selection | ✅ |
| 4.5 | Picking a tool from a toolbar dropdown makes it the group's face for the session | A §2.3 | ✅ (10-27: Face blend from the Fillet ▾ takes the group's button) |
| 4.6 | Toolbar tooltips show the shortcut | A §2.2 | ✅ |
| 4.7 | Customize toolbar (drag tool sets, Reset) | part_studios | ✅ (built by an earlier pass: move tool sets, remove/restore tools, reset) |
| 4.8 | Search tools: typing filters tools, Enter runs the first, Esc closes | Home__search_tools | ✅ |
| 4.9 | Search tools with no query lists recently used tools first (bold "Recently used", newest first, first row selected, description pane); description on hover | Home__search_tools | ✅ |
| 4.10 | REMEMBER_PREVIOUS_VALUE parameters default to the last accepted value of that tool | A §7.2 | ✅ (app 10-24: Fillet 3 opened at the 3 mm just accepted) |
| 4.11 | Search tools box: 162 px #eee box, "alt/⌥" (35 px) and "c" (17 px) keys joined, 19 px, 1 px #aaa with a 2 px bottom edge, 3 px after the label | live part-studio/00-new-doc-initial | ✅ (box, key edges and key text rows at live's pixels) |

## 5. Feature dialogs and selection fields (Home__dialogs, feature_basics, Home__numeric_fields, A §7)

| # | Behaviour | Source | Status |
|---|---|---|---|
| 5.1 | Green check accepts, red X cancels | feature_basics | ✅ |
| 5.2 | Title red while the feature errors; the ✓ is disabled but Enter still commits (the model falls back to the state before the feature) | live | ✅ (was: commit refused) |
| 5.3 | Error/warning message shown in the dialog | Home__error_indicators | ✅ |
| 5.4 | Fields appear/disappear live from their visibility conditions | A §7.1 | ✅ |
| 5.4b | Clicks and right-clicks on the dialog (and other panels over the graphics area) never reach the model beneath | (Onshape behaviour) | 🔧 (clicking a field also picked/deselected geometry under it) |
| 5.5 | First selection field active when the dialog opens | feature_basics | ✅ |
| 5.6 | Click a selection field to make it active; it stays active while a number field has keyboard focus ("two fields are active") | Home__dialogs; feature_basics | 🔧 (typing deactivated it) |
| 5.7 | Picks go to the active field; clicking a picked entity again removes it | Home__selection; A §5.4 | ✅ |
| 5.8 | Picks that don't pass the field filter do nothing: no highlight on hover, no pick, no toast, no flash | live "qa4 round 5 s1 follow-up 2" (1) | 🔧 (`field_takes`: the hover and the click; the invented rejection toast and red flash removed) |
| 5.8b | Clicking a sketch curve while a region field is active takes the region it bounds (the smaller one for a curve between two regions); hovering the curve pre-highlights that region | live (Extrude dialog) | ✅ (was "all regions"; changed per live) |
| 5.9 | List clicks (default planes, parts) obey the field filter | A §5.4 | ✅ (10-27: a part row does nothing in Extrude's regions field) |
| 5.10 | Clicking a sketch in the feature list fills a region field with all its regions, shown as one "Faces of Sketch 1" row; clicking again removes them; feature fields take the feature | features_and_parts_lists; live | ✅ |
| 5.11 | Single-pick fields replace their pick and move focus to the next empty selection field | A §5.4 | ✅ |
| 5.12 | × on a chip removes that pick | feature_basics | ✅ |
| 5.13 | Right-click a pick > Clear all (plus Clear missing references when some are missing); × on hover removes one row | live (Fillet dialog) | ✅ |
| 5.14 | Hovering a pick row highlights its entity in the graphics area, tints the row and shows its name as a tooltip | Home__dialogs; live | ✅ (yellow-green #f2ff80 state QUERY in the shader) |
| 5.15 | Missing references shown red; Clear missing references | feature_basics | ✅ (verified: red rows, accept disabled, Clear missing references) |
| 5.16 | Reorder picks in order-sensitive fields by dragging a row | feature_basics | ✅ (10-27, Loft profiles: ⇅ turns the rows' × into ≡ grips and shows DONE; dragging a grip draws the drop line and moves the pick. The Move up/down menu items were removed: live menus only have Clear all) |
| 5.17 | Tab moves between fields (inputs and checkboxes) | feature_basics | ✅ |
| 5.18 | Space toggles a focused checkbox; arrows/Enter in dropdowns | feature_basics | ✅ (Space toggles a checkbox; Enter/Space opens a dropdown, arrows move, Enter picks — the list opens without the current value highlighted) |
| 5.19 | Numeric fields accept expressions, units and #variables | Home__numeric_fields | ✅ |
| 5.20 | Invalid expression: red outline and message | Home__numeric_fields | ✅ |
| 5.21 | Expressions show their evaluated value | Home__numeric_fields | ✅ (10-27: "10+5" → "15 mm" once unfocused) |
| 5.22 | Mouse wheel over a focused numeric field (mm document, live 10-27): ±1, Shift ±10, Ctrl ±0.1 (integers ±1); only while focused with no uncommitted typed text; a step out of the field's range is ignored (Depth goes below 0) | Home__numeric_fields | ✅ |
| 5.23 | Field shows the converted value after accepting, the original text when focused | Home__numeric_fields | ✅ (fixed: focusing showed the converted text) |
| 5.24 | Final button previews the end of the history; absent for new features and the last feature | Home__dialogs | ✅ (app 10-24) |
| 5.25 | Preview opacity slider (blends the model before and after the feature) | Home__dialogs | ✅ (live previewBlend: both models mixed at the slider, 0.7 at rest; the active field's highlight lines drawn whole over the mix, also at 1, as live part-studio/psui-prv-01 / -05b) |
| 5.26 | Rename the feature from the dialog title | Home__dialogs | ✅ (double-click the title) |
| 5.27 | Dialog movable and resizable; help (?) icon | Home__dialogs | ✅ (move by the title, resize by the bottom-right grip; size kept for the session) |
| 5.28 | Boolean "Convert to expression" / "Convert to checkbox" (evaluated at regeneration with upstream variables) | feature_basics | ✅ (starts as true/false text, focused) |
| 5.29 | Tolerance options on tolerant quantities (± icon "Tolerance options": precision ▾ then tolerance type ▾; Hole diameter adds Fit / Fit with tolerance / Fit (tolerance only) with Standard, Fit type, Hole class, Shaft class) | Home__tolerance_options; live dialogs/hole-30–37 | ✅ (`model::fits`; the hole callout in the graphics area shows "Ø6.350 H7 +0.015/−0.000 THRU") |
| 5.30 | Extrude/Revolve from a part face (or a sketch on one) defaults to Add with that part as the merge scope | (Onshape behaviour, A §7) | ✅ (app 10-24) |
| 5.31 | Choosing Remove/Intersect on such a feature points it into the part | (Onshape behaviour) | ✅ (10-27: Extrude on the block's top face, Remove → the arrow turns down, the block cut through, the tool grey) |
| 5.32 | Drag handles for depth/angle/distance (extrude arrow, revolve arc, plane offset, shell thickness, fillet radius, chamfer distance) | A §7.7 | ✅ (hover outlines the arrow blue) |
| 5.33 | Editing a feature rolls the model back to it | Home__dialogs | ✅ |
| 5.34 | Feature dialog rows on live's pixels: Chrome's fractional rows (26.33 px empty selection fields, 27.33 px checkbox rows) snapped like Chrome (27/28 px steps), 5 px under a field, 28 px select rows (select on the top line), 26 px quantity rows (29 px with a hole icon), 31 px icon selects, 48 px stacked label + select, Hole size block without top padding, inactive field label #666, frame ending 24.5 px under the list | live dialogs/*-01-open, dom/dialog-extrude-new, dialog-fillet, dialog-chamfer, dialog-hole | ✅ (Extrude, Revolve, Fillet, Chamfer, Shell, Hole, Sweep, Linear pattern, Plane: every row's checkbox/underline/label at live's pixel; Revolve and Fillet's frame bottom and ? are 1 px high from Chrome's sub-pixel rounding) |

## 6. Feature list (PartStudio__features_and_parts_lists, A §3)

| # | Behaviour | Source | Status |
|---|---|---|---|
| 6.1 | Feature count in the header | features_and_parts_lists | ✅ |
| 6.2 | Default geometry rows (Origin, Top, Front, Right) | features_and_parts_lists | ✅ |
| 6.3 | Each feature with its tool icon | features_and_parts_lists | ✅ |
| 6.4 | Double-click a feature to edit it | A §3.3 | ✅ |
| 6.5 | Click toggles a feature in the selection (additive) | features_and_parts_lists ("just click to select additional items") | ✅ (app 10-24) |
| 6.6 | Shift+click selects a range, hidden sketches included (live 10-27) | Home__selection | ✅ |
| 6.7 | Drag a feature to reorder | features_and_parts_lists | ✅ |
| 6.8 | Dragging a selected feature moves every selected feature (non-contiguous allowed) | features_and_parts_lists | ✅ (10-27: Sketch 3 + Extrude 3 dragged above Sketch 2: a "2 items" chip, the drop line, both move) |
| 6.9 | A reorder above a feature's dependencies is done (no refusal, no toast); the feature then fails on its missing selection | live qa4 round 5 v3 s8 (REORDER-INVALID) | ✅ (the app no longer checks the order; test `extrude_above_its_sketch_misses_its_region`) |
| 6.10 | Search box filters by name or type: Default geometry rows and parts by name too, each run of dropped rows one "..." (Parts too), "Features (M of N)" counting the kept default rows, no error icon while filtering (live 10-27) | features_and_parts_lists | ✅ |
| 6.11 | Filter commands :errors :warnings :suppressed :hidden :shown :type :name :folder :variable :part | features_and_parts_lists | ✅ (:partnumber / :revision match the parts' Part number / Revision properties; live 10-27: ":" alone matches nothing, ":type" / ":part" alone don't filter, ":hidden" / ":shown" take default rows and parts) |
| 6.12 | Error icon in the header (17 px at x 45, "N feature(s) failed to regenerate"), title "Features …"; click filters ":errors " (live 10-27) | features_and_parts_lists | ✅ |
| 6.13 | Failed features red with the error as tooltip | Home__error_indicators | ✅ |
| 6.14 | Suppressed features struck through | features_and_parts_lists | ✅ |
| 6.15 | Under-defined sketch indicator | features_and_parts_lists | ✅ (blue text) |
| 6.16 | Missing-selection / warning yellow triangle | features_and_parts_lists | 🔧 (12 px #FAB601 badge with a white !, the icon's bottom right, at live's pixels: part-studio/2071; shown for Warning status — missing-selection tracking itself is not modelled) |
| 6.17 | Eye icon on hover hides/shows sketches, planes, mate connectors | features_and_parts_lists | ✅ (app 10-24) |
| 6.18 | Rename in place (context menu / Shift+N) | features_and_parts_lists | ✅ |
| 6.19 | Feature context menu in the live order: Rename | Edit…, Edit <sketch>…, Copy sketch | Add selection to folder… | Show/Hide | Export as DXF/DWG… | Suppress, Configure suppression, Dynamic suppression ▸ | Zoom to selection, View normal to | Show dependencies… | Roll to here (not on the last feature) | sketch/feature appearance | Delete | live (dom/context-menus-partstudio.txt) | ✅ (Create Drawing of Sketch N… via asb's sketch views; Add comment out of scope: cloud comments) |
| 6.20 | Several selected features: Add selection to folder… | Suppress, Dynamic suppression ▸ | Clear selection | appearance | Delete | live | ✅ (Section view… through the selected sketches' / planes' planes) |
| 6.21 | Show dependencies…: "Dependencies of X" panel at the dialog position: parents, ▲, the feature, ▼, children (icons, hover highlights, tooltips) | live (part-studio/100) | ✅ |
| 6.22 | Dynamic suppression ▸ Add expression… (expression editor, ƒx icon, live true/false preview) | live | ✅ |
| 6.23 | Copy sketch / Paste sketch onto a plane or planar face | Home__context_menus | ✅ (verified: Copy sketch, Front plane > Paste sketch) |
| 6.24 | Show dimensions (feature/face/sketch menus): the sketch's dimensions in 3D, parts see-through, hover gold on tan, double-click edits the sketch, click elsewhere or Esc hides; Export as DXF/DWG | Home__context_menus; live part-studio/40–42 | ✅ (Create Drawing of Sketch N… via asb's sketch views) |
| 6.25 | Pause regeneration (banner: Regenerate / Regenerate and exit / Discard) ; regeneration times panel (hover highlights) | features_and_parts_lists | ✅ paused mode as live 10-27: the 343 × 28 amber banner over an emptied toolbar (Undo / Redo stay), ▶\| at 0.4 until something waits, New folder gone, pause at 0.5, Create version… off the rail, edited rows bold italic, dialogs preview nothing (no manipulator, no rollback), ✕ discards without asking. Live's Depth corner mark in a paused dialog not done |
| 6.26 | Collapse the whole list; drag the feature/parts separator | features_and_parts_lists | 🔧 |
| 6.27 | Default plane row menu (live): Rename | New sketch…, Offset plane…, Size to Part Studio (Paste sketch when a sketch was copied) | Hide, Hide other planes, Hide all planes | Zoom to selection, View normal to | Show dependencies…; origin: Rename | Hide | Show dependencies… | Hide, Hide other planes, Hide all planes | ✅ |
| 6.28 | Default planes/origin can be hidden (never deleted: Delete does nothing, no Delete in their menu, live 10-27) | part_studios | ✅ |
| 6.29 | Clicking empty list space clears the feature selection | (Onshape behaviour) | ✅ (app 10-24) |

### Rollback bar

| # | Behaviour | Source | Status |
|---|---|---|---|
| 6.30 | Drag the bar to roll back/forward; model shows up to the bar | features_and_parts_lists | ✅ |
| 6.31 | Right-click a feature > Roll to here / Roll to end | features_and_parts_lists; live "qa4 round 5 s2 rollback follow-up" | ✅ Roll to here unless the bar is right below the feature, Roll to end while the bar isn't at the end (`behaviors::roll_items`, tested on live's four menus). A rolled-back feature: Rename \| Add selection to folder… \| Suppress, Dynamic suppression ▸ \| Roll to here, Roll to end \| Delete (no Edit…, Zoom to selection, Show dependencies…, appearance); with a feature dialog open only Rename \| Suppress, Dynamic suppression ▸ (R5-S3-b). Add comment is cloud |
| 6.32 | (Right-click the bar > Roll to end) | features_and_parts_lists | ✅ removed (live has no menu on the bar) |
| 6.33 | Select the bar, ↑/↓ step one feature | features_and_parts_lists | ✅ (also after clicking a feature row; the bar turns #a0c7dd while focused) |
| 6.34 | Features below the bar greyed; new features inserted at the bar | A §3.2 | ✅ |
| 6.34a | Dragging the bar or a feature row near the list's top/bottom autoscrolls it | live "psb (10-24 k)" | ✅ 70–75 px edge zones, ~30 → 205 px/s toward the edge, faster past it (~370 px/s at 40 px, ~600 at 190 px out), also with the pointer beside the panel (only its height counts); a frame loop keeps scrolling while the pointer rests and stops at the ends (`behaviors::list_autoscroll_speed`, tested against live's speeds) |
| 6.34b | The bar dropped over the Default geometry rows or header lands at the top slot (drop line under the last default row) | live "qa4 round 5 s2 rollback follow-up" (3) | ✅ (app: drop on Origin → every feature rolled back) |

### Folders

| # | Behaviour | Source | Status |
|---|---|---|---|
| 6.35 | Add selection to folder | features_and_parts_lists | ✅ |
| 6.36 | Expand/collapse a folder | features_and_parts_lists | ✅ |
| 6.37 | Folder menu: Rename, Suppress/Unsuppress (all contents), Unpack folder | features_and_parts_lists | 🔧 (Suppress added, "Remove" renamed) |
| 6.38 | Nested folders; Expand all / Collapse all | features_and_parts_lists | 🔧 |

## 7. Parts list (features_and_parts_lists §Parts list, A §3.9)

| # | Behaviour | Source | Status |
|---|---|---|---|
| 7.1 | Parts with colour swatch and count | features_and_parts_lists | ✅ |
| 7.2 | Click selects (or fills a part field in a dialog, filtered) | features_and_parts_lists | ✅ (10-27: Delete part takes the clicked row's part) |
| 7.3 | Hover highlights the part in the graphics area | features_and_parts_lists | ✅ |
| 7.4 | Eye on hover ("Hide Part N", 20 × 20 at x 210) hides/shows the part; hidden parts greyed | features_and_parts_lists | ✅ |
| 7.5 | Parts list menu (live order): Rename, Properties…, Assign material…, Edit appearance… | Create Drawing of Part…, Export… | Hide, Isolate…, Make transparent… | Zoom to selection | Delete… | live | ✅ (Copy Part N after Copy here… as live; Release… and Revision history: see 13.14) |
| 7.6 | Separate Parts / Surfaces / Curves sections (collapsible, eye hides a section); "Composite parts (N)" after Parts (collapsed at first, eye "Hide all composite parts", row tooltip "Composite part 1 (open\|closed) (N parts)"; closed: members leave Parts, stay drawn and selectable) | features_and_parts_lists, live part-studio/470–474 | ✅ |
| 7.7 | Show parts list properties (part number, revision) from the section header menu | features_and_parts_lists | 🔧 |

## 8. Graphics area context menus (Home__context_menus, A §4)

| # | Behaviour | Source | Status |
|---|---|---|---|
| 8.1 | Right-click without drag opens the menu; right-drag rotates | A §4 | ✅ |
| 8.2 | Empty space: Show all | Select ▸ | (Clear selection) | Zoom to fit, Isometric | live | ✅ (Select ▸ with nothing selected = Create selection… only, as in the sketch capture; Part Studio capture requested) |
| 8.3 | Face: Edit <feature>…, Edit <sketch>…, Show dimensions, Show dependencies of <feature>… | New sketch…, Offset plane…, Extrude…, Revolve… (planar) | Hide <part>, Isolate…, Make transparent…, Section view… | Copy <part> | Create Drawing of <part>…, Export as DXF/DWG… (planar), Export… | (Clear selection) Select ▸, Select other… | Zoom to fit, Zoom to selection, View normal to | Delete <part>…, Delete <feature> | Assign material…, Edit appearance…, Add appearance to N faces… | live | ✅ (Add comment n/a; Copy part + Ctrl+V in another Part Studio opens a prefilled Derived, in the same one a Transform copy) |
| 8.4 | Edge: same groups with Fillet…, Chamfer… and Align view with vertical axis (straight edges) | live | ✅ (Curve/surface analysis… added) |
| 8.5 | Plane: New sketch…, Offset plane… | Hide <plane>, Hide other planes, Hide all planes, Section view… | Select ▸, Select other… | Zoom to fit, Zoom to selection, View normal to | live | ✅ |
| 8.6 | Sketch region/curve: Edit <sketch>…, Show dependencies… | Extrude…, Revolve… (region) | Hide, Section view… | Export as DXF/DWG… | Select other… | Zoom to fit, Zoom to selection, View normal to | Delete <sketch> | Edit sketch appearance… | live (dom/context-menus-ps-sketch-entities.txt) | ✅ (verified in the app: region New sketch… / Offset plane… on the region; curve Copy sketch entities (pastes into a sketch), Hide other sketches / Hide all sketches, Select ▸ as live (region: Protrusion / Pocket / Hole / Fillet / Tangent connected / Bounded faces, Loop/chain connected edges; line or circle: Tangent connected, Equal length/radius, Parallel edges; + Create selection…; each replaces the selection), Align view with vertical axis on straight lines; midpoints of sketch curves on hover; Create Drawing of Sketch N… via asb's sketch views; Add comment out of scope: cloud comments) |
| 8.7 | The right-clicked entity stays highlighted while its menu is open | live (menus/rmb-ps-face.png) | ✅ |
| 8.8 | Exit isolate / Exit make transparent first, Turn off section view (in Select's place) when active (live 10-27) | View__isolate; View__section_view | ✅ (one Esc on the menu also ends the isolate, as live) |
| 8.9 | Creation commands carry their tool icon | Home__context_menus | ✅ (creation, hide, section, drawing, delete and appearance entries carry icons) |
| 8.10 | A feature dialog open: Confirm <feature name> · Cancel first, then Show all · Curve/surface analysis… · Select (live 10-27; no "Exit <tool>") | Home__selection | ✅ |
| 8.11 | Create selection / Select ▸ / Add appearance / Section view / Paste sketch entries | A §4 | ✅ (Create selection… is the last entry of Select ▸) |

## 9. View navigation, view cube, camera menu (View__view_navigation_and_the_view_cube, A §6)

| # | Behaviour | Source | Status |
|---|---|---|---|
| 9.1 | Right-drag rotates, middle-drag or Ctrl+right-drag pans, wheel zooms at the cursor | view_navigation | ✅ |
| 9.2 | Double-click the middle button zooms to fit (live 10-27) | View__zoom_to_fit | ✅ |
| 9.3 | View cube in the upper right turns with the camera | view_navigation | ✅ (iso: the Top face fill 1803–1854 × 127–156 = live's pixels) |
| 9.4 | Click a cube face for that standard view | view_navigation | ✅ |
| 9.5 | Click a cube edge or corner for the view along it | view_navigation; A §6.2 | ✅ (a corner takes the pointer up to 0.31 half edges (9 px) from its centre, live's ~8 px beyond its disc) |
| 9.6 | Cube faces/edges/corners highlight on hover, (188,226,247) = #90cef1 at 0.6; no name or tooltip (live 10-27) | A §6.2 | ✅ |
| 9.7 | Cube arrows rotate 15° (Shift 90°, Ctrl 5°); arc arrows roll | A §6.2 | ✅ (psui 10-27, as live "spec: psui part-studio.md 9.7": ▶ brings the left side in view, ▲ the front face, the right arc rolls clockwise; 15° / Ctrl 5° / Shift 90°) |
| 9.8 | Camera menu (live order): Isometric/Dimetric/Trimetric | Named views…, Previous view | Zoom to fit/window | Perspective, Orient normal to sketch on edit | ✓ display ▸, ✓ hidden edges ▸, ✓ tangent edges ▸ | View in high quality | Section view… | live dom/menu-camera-render.txt | ✅ (Graphics preferences… and Highlight boundary edges by ui) |
| 9.9 | Previous view returns to the camera before the last view change | View__previous_view | ✅ (10-27: Shift+5 then Previous view gives the same frame pixel for pixel) |
| 9.10 | Zoom to selection (parts, faces, edges, sketches, planes, features) | View__zoom_to_fit | ✅ (10-27: a face's menu › Zoom to selection frames that face) |
| 9.11 | Display: Shaded with edges / Shaded without edges / Unshaded / Translucent; Hidden edges visible/removed; Tangent edges visible / phantom (dash-dot) / removed | live | ✅ (tangency found from the mesh: `MeshBuffers::smooth_edges`) |
| 9.12 | Orient normal to sketch on edit (camera menu toggle); the previous view returns when the sketch closes | View__orient_normal_to_sketch_on_edit | ✅ (10-27: Sketch 1 opens normal to Top, ✓ brings back the frame pixel for pixel; the toggle isn't persisted across sessions) |
| 9.13 | Named views store orientation/zoom/projection; save, restore, delete | View__named_views | ✅ |
| 9.14 | Named views store the section state | View__named_views | 🔧 |
| 9.15 | Rotation inertia; rotate about the point under the cursor | A §6.1 | ✅ U (`inertia_decays_like_onshape`, `rotation_keeps_the_pivot_fixed_on_screen`, app-core navigation, from the bundle) |

## 10. Visibility, isolate, transparency, section (View__hide_show, View__isolate, View__make_transparent, View__section_view)

| # | Behaviour | Source | Status |
|---|---|---|---|
| 10.1 | Hide/Show parts, sketches, planes, mate connectors, default geometry | View__hide_show | 🔧 (parts only before) |
| 10.2 | Hide other parts / Hide all parts / Show all / Show all parts | View__hide_show | ✅ (10-27: the face menu and the parts list row menu as live; Show all in the empty-space menu) |
| 10.3 | Isolate keeps chosen parts opaque, others transparent | View__isolate | ✅ (see 2.18) |
| 10.4 | Make transparent makes chosen parts transparent | View__make_transparent | ✅ (see 2.19) |
| 10.5 | Isolate/transparent panel with transparency control and × to exit; Esc exits | View__isolate | ✅ (10-27: the 220-px panel at the dialog's place, Esc exits, also from the graphics menu) |
| 10.6 | Select transparent geometry checkbox; Expand by distance | View__isolate | 🔧 |
| 10.7 | Section view dialog (live): Exclude | Include tabs, "Section planes" field (planes, planar faces, mate connectors; × removes) + a view-facing plane button, "Items to exclude/include"; ✓/Enter keeps, ✕/Esc restores; empty-space menu Edit section view… / Turn off section view | live part-studio/70–74 | ✅ |
| 10.7a | Section view manipulator: base circle (slides the manipulator over the plane), offset arrow, and at its tip two rotate rings about the plane's in-plane axes (Top: Y, then X); a ring drag turns the plane about the arrow's base, snapping to 5° within 2° (else 3 decimals) with a "deg" value box that takes a typed angle; a turned plane's chip loses "(Top)"; the hovered handle turns blue | live "skb (10-24) assembly section" (1)–(3) | ✅ (app: ring hover blue, drag → "27.345 deg", tilted cut; `section_manip.rs`, test `rotation_snaps_and_reads_as_live`; look to icons2) |
| 10.8 | Section by several planes (up to 3), drag arrow on the active plane with an offset box, caps | View__section_view | ✅ (caps: the cut solid is capped in the part color with hatching, separate section pipeline; offscreen test `section_caps_cover_the_cut`) |

## 11. Measure, mass properties, appearance (View__measure_tool, View__mass_properties_tool, PartStudio__customizing_appearances)

| # | Behaviour | Source | Status |
|---|---|---|---|
| 11.1 | Measure readout for the selection (length, area, radius, distance, angle, ΔX/ΔY/ΔZ); the first row follows the client's order (bundle `setMeasurements`: Diameter / Radius / Parallel (axis) distance or an angle first): two perpendicular faces read "Perp planes angle: 90.000 deg" (live part-studio/81, assembly/707), parallel faces "Parallel dist", pairs "Min dist", face totals "Area"; values with fixed decimals and "deg" ("90.000 deg", "400.000 mm²") | View__measure_tool, live 81/707 | ✅ (verified in the app; Part Studio and assembly share `measure::measure_shapes`; labels for line/axis pairs inferred from the bundle's measurement types — live request psb 10-20 m) |
| 11.2 | Measure/Mass properties buttons at the bottom right; [ opens Measure with a prompt when nothing is selected | A §1, §6.10 | ✅ (10-27: [ opens Measure with "Select entities to measure"; the bottom-right Mass button opens Mass properties) |
| 11.3 | Measure graphics (dotted min distance with X/Y/Z components) and measure type selector | View__measure_tool | ✅ (10-27: two faces → Parallel dist / Area / Min dist / Max dist with X/Y/Z rows, the dashed line on row hover; Measure type "Show all") |
| 11.4 | Mass properties: volume, surface area, mass (with material), center of mass | View__mass_properties_tool | ✅ |
| 11.5 | Moments of inertia about the center of mass; Face tab; reference mate connector; override mass | View__mass_properties_tool | 🔧 (several parts: each part's inertia moved to the joint centre of mass by the parallel-axis term, test `mass_properties_move_each_part_to_the_joint_centre`; Face tab / reference mate connector / override mass still open) |
| 11.6 | Part appearance colour, transparency, tessellation quality (Appearance editor) | customizing_appearances | 🔧 (material still in Part properties) |
| 11.7 | Appearance panel, face/feature/sketch appearances | customizing_appearances | 🔧 (no custom-color tiles / mixer tab) |

## 12. Variables and configurations (PartStudio__variable_table, PartStudio__configurations)

| # | Behaviour | Source | Status |
|---|---|---|---|
| 12.1 | Variable table panel on the right | variable_table | ✅ |
| 12.2 | Configuration panel and configuration selectors | configurations | ✅ |
| 12.3 | `#variable` in any numeric field; :variable filter finds users | variable_table; features_and_parts_lists | ✅ (quoted = exact) |
| 12.4 | Hover a variable highlights the features that use it; right-click > Filter feature list | variable_table | ✅ (`:variable "name"`, live 564) |

### Found in the live capture

| # | Behaviour | Source | Status |
|---|---|---|---|
| 13.1 | A new feature hides the sketch it consumes | live (selection/02) | ✅ |
| 13.2 | Ctrl+A does nothing in the Part Studio | live (shortcuts/14) | ✅ (not bound) |
| 13.3 | Delete/Backspace on a selected feature deletes it at once (no confirmation, no toast); Ctrl+Z restores it | live (shortcuts/15) | ✅ (Delete and Ctrl+Z verified) |
| 13.4 | Tab skips the End type dropdown; Shift+Tab from Depth reaches the flip button | live | ✅ |
| 13.5 | Depth arrow: hover outlines it blue; dragging updates the Depth field with the value selected; no viewport label | live (dialogs/extrude-05..07) | ✅ (app 10-24 vs dialogs/extrude-05..07: the field is focused with the value selected during the drag too, not only after; the dragged arrow is one solid blue stroke (≈ #2c61a3), the hovered one blue around a light-blue core; values move in 0.1 mm / 0.01 in steps like live's "13.9 mm", "2.06 in") |
| 13.6 | Hovering a selected feature row shows "Extrude 1 <- Sketch 1" | live | ✅ |
| 13.7 | Rollback bar: no right-click menu; after clicking a feature row ↑/↓ move the bar; focused bar `#a0c7dd`; dragging shows a "Rollback bar" pill and a 2 px blue drop line | live | ✅ |
| 13.8 | Picking through planes: a part face behind a transparent plane always wins | live (selection/30–31) | ✅ (verified: the face behind the Right plane is picked) |
| 13.9 | Copy here… opens Transform (Translate by XYZ, Copy part) for the part | live (part-studio/102) | ✅ |
| 13.11 | Deleting a tab: no confirmation and no toast (a "Delete tabs" warning only when other tabs use it); Ctrl+Z doesn't bring it back; Versions and history > right-click an earlier change > Restore restores it. Undo is per tab: Ctrl+Z reverts the active tab's newest edit only | live; Document__document_tabs; Document__versions_and_history | ✅ (`UndoStack::undo_in`/`redo_in`, `commit_not_undoable`) |
| 13.12 | A Variable shows as "#name = value" ("#? = …" until named) in the list and its dialog title; its dialog opens with Name focused | live (Variable dialog) | ✅ |
| 13.13 | Configurations: "+ Configure features", click a feature, then a parameter (outlined in yellow in its dialog) to configure it; a dialog shows and edits the active configuration's values of configured parameters | PartStudio__part_studio_and_assembly_configurations | ✅ (was: the dialog showed and edited the base value, which the table then overrode) |
| 13.10 | Clicking a face/edge/vertex while a parts field is active (Boolean Tools…) takes its part; hovering highlights the whole part | (Onshape behaviour) | ✅ |

| 13.14 | Release management (local, no approvers): parts list Release… opens "Create Release candidate" (items with Revision / State / Part number, needs-attention count and messages, Release name * and notes, Save draft, Apply → "Review Release", Release, Close); Release writes the part numbers, creates a version named after the release and records it (store meta "releases"); revisions A, B, C… skipping I O Q S X Z, never lower than released, auto-obsolete of the previous revision; Revision history (View release, Obsolete revision → "Create Obsolete candidate" with re-releasable option); versions graph ▲ release / ◭ contains obsoletion, legend, Releases ▸ submenu → Review Release; Properties show the released revision | docs-raw Release__*.md (no live capture: free plan) | 🔧 (Add items (+), Release all configurations, the versions-panel release icon and tab-menu Release… still to do) 19 Oct: Release… on Part Studio / Assembly / Drawing / Variable Studio / file tabs (`tab_items`: BOM parts, drawing sources) and the versions panel, Revision history on Assembly / Drawing / Variable Studio tabs, obsoletion marks the released version, Release drafts panel (rail) with double-click → Review Release, Save draft ▴ Save / Delete draft, State "In progress" for workspace items, empty Release name, imported revision kept, Not revision managed, drawing revision tables fed from the log, Revision history User / Document / Version link / View obsoletion. Remaining gaps listed in the handoff. |
| 13.15 | Hole: Up to next / Up to entity show the computed depth read-only ("Multiple" with several points), and the name template follows hole.fs (thread size, ↧ depth / computed depth / THRU, ⌴ / ⌵ parts) with values as the fields show them, also in the list row while editing | hole.fs getFeatureName, setFeatureComputedParameter; live part-studio/436 | ✅ (verified: Multiple, 10 mm, "Ø 6.35 mm ↧ 12 mm" in title and row) |
| 13.16 | Hole dialog open: one callout per feature at the middle hole (no "3X"), in the start face's plane, leader from the rim to the part's edge, shelf, constant screen size; "Ø6.35 ↧5", "Ø6.35 THRU" (also a blind hole deeper than the part, and Up to next), fits stacked | live part-studio/431–439, dialogs/hole-30, hole-35 | ✅ (`part_studio/hole_callout.rs`, `interaction::hole_callout_3d`) |
| 13.17 | Transform ▸ Translate by XYZ / Copy here…: triad at the part's top-face centre, ~65 px, white hollow arrows + three planar handles; hover turns only that handle blue (rgb(63,111,172) / rgb(151,177,217)) and hides the geometry hover; drags update X / Y / Z live with one decimal; other handles translucent while dragging | live part-studio/440–449 | ✅ (`part_studio/triad.rs`; the moved part's ghost of the original is not drawn) |
| 13.19 | Parts list: Rename edits inline (white input, 1 px #1651b0 border, text selected; Enter commits, Esc cancels); Properties… = the metadata modal with Reset all; Assign material… = the floating "Material" panel (Library / Custom, Onshape Material Library with 189 materials, search, read-only properties, ✓) | live part-studio/450, dialogs/assign-material-*, dom/dialog-partstudio-part-properties.txt | ✅ (`start_part_rename`, `open_part_meta`, `part_studio/material_panel.rs`; all 189 materials carry category and the six mechanical properties from live dom/material-library-onshape-full.csv (`material_data.rs`, generated); document / added libraries map their CSV columns by header) |
| 13.18 | Flatten surfaces ▸ Export as Parasolid (planar sheet; one face per flattened face with Export inner edges) | analysis tools | ✅ (`Flattened::loops`, kern's `write_parasolid`; export verified in the app) |
| 13.20 | Feature dialogs' keyboard (live psb 10-19 h): focus starts on the first selection list; Tab order = DOM order: selection lists (focusing one makes it the active field), selects (Onshape's focusser: Enter/Space/↑/↓ or a typed character open it; focused glow), the flip button, number fields, the tolerance (±) button, every checkbox, the `?` help; the tab rows and ✓ / ✕ are skipped; Tab past `?` leaves the dialog for the navbar (logo link), Shift+Tab before the first control to <body> and stays there; focus looks from the bundle CSS (checkbox box #1651b0 + glow, icon buttons a 2 px bottom line, selects .btn-default-focus) | live behaviours.md "Keyboard, full Tab cycle", part-studio/510–513, bundle woolsthorpe.css | ✅ (verified in the app: faces → End type → flip → Depth → … → ? → navbar logo) |
| 13.21 | Release dialog header: "1 item needs attention" / "N items need attention" + the red filter button ("Show only items that need attention"), ⋯ menu "View audit log" (the workflow object's audit JSON in a new tab) and "View workflow" (every candidate has a published workflow): a featherlight lightbox (#333 backdrop, 25 px white frame, ✕) with the workflow diagram, current state filled, pan / zoom / drag nodes, Esc closes only the lightbox | bundle release-dialog.html + osx-workflow-diagram + woolsthorpe.css `.release-process-diagram-lightbox`, Help Release__creating_a_customized_release_workflow (fs-custwkflow3, custom-workflow-ex) | ✅ (verified in the app; release management isn't on the Free account (live "psb (10-19 e)"), so the audit JSON's field names are ours) |
| 13.22 | Numbering schemes (Company settings; here Preferences under Release management): tabs Release objects / Standard content / Items / Release names, Number generation (Manual entry / Sequential part number generation), flow chart Prefix per type → Min length → Next → Preview, Part number uniqueness, Part number propagation, Save changes per tab; release dialog # buttons: header "Generate missing part numbers", per row "Generate next part number", Release name "Generate next release name"; drawings may share their part's number | Help Plans/numbering_schemes (co-numschemes-03.png), bundle strings | 🔧 (implemented: `numbering_prefs.rs`, `NumberingSchemes`; the other tabs' layout and the default prefixes await live "psb (10-19 e)"; Arena generation is a cloud integration) |
| 13.23 | Add Items to Release: Current document / Other documents tabs; the title row's Create version… and Version graph (pick a version of this document); Other documents: filters, Search or paste URL, documents, then the document's newest version (Version graph to change); items from a version carry a linked-version icon; released ones are listed as Released with their revision, others release with this release and the linked document's version is marked Released | Help Release__selecting_parts_for_a_release_candidate, Release__typical_release_workflow (Releases and linked documents), bundle select-item widget | ✅ (verified in the app: `release_add_source.rs`, `ReleaseItem.document`; a document without versions opens at "Start" with "N changes since the last version." + "Create a version in <doc>." and "No <type> available." like the Insert dialog; a linked row's Edit properties / Export open that version in a read-only window of its own (`files::open_version_window`) and run there, for other documents and for versions of the releasing document alike) |
| 13.24 | Tool search (Alt+C) lists only tools: "material", "release", "measure"… give "No items match your search" (42 px row); synonyms offer "Did you mean:" pills ("export" → [Derived]), 24 px above the results; the same in the Assembly search | live menus/tool-search-*.png, bundle CommandSearch synonyms | ✅ (verified in the app; `tool_synonyms.rs`) |
| 13.25 | Assign material picker (ui-select): "- None -" is filtered like the materials, matches in bold, the active row (#b2ddf6) starts at the current material and moves with hover / ↑ ↓, Enter picks it, Esc closes the list, the search's ✕ clears it, the list fits its rows up to 200 px; density lines 11.67 px #555 at 65 % | live dialogs/assign-material-03/04, dom/dialog-assign-material-picker.txt | ✅ (verified in the app; property values right-aligned except Name, live assign-material-05) |
| 13.26 | Interference detection: 220 px dialog at the lower right with only ✕; "Parts to analyze" (picks from the viewport or the parts list), read-only "Interferences" list ("Part 1 and Part 2", tooltip "L x W x H", hover = #f2ff80 box, click = bold + zoom), "No interferences"; picked parts outlined amber; with results all parts light gray see-through, outlined, the overlap solid shaded red; no Scale slider; `?` row | live analysis/80–84, dom/dialog-interference-*.txt | ✅ (verified in the app; default position from live psb 10-19 i: left = W − 600, top = H − 401, kept while open; the Refresh row ↻ "Interference detection not accurate, instances have moved." after a model change) |
| 13.27 | Configuration visibility conditions: the Configurations box's "Modify visibility conditions" button (all inputs), input ⋯ › Visibility conditions…, the input's visibility icon; the dialog (Inputs filter, ⋯ expand/collapse all, always-visible row, Show <input> When [input] [is|is between] [values] And …, option rules Show [options|range] [When|Always] …, summaries, warnings, preview with Show hidden inputs, undo/redo, Save/Cancel); hidden inputs take their defaults, hidden options fall back; repair after deletions with "Visibility conditions for X were updated"; the box hides hidden inputs/options | Help PartStudio__visibility_conditions, bundle visibilityConditionsDialog/Statement/conditionBuilder | ✅ (verified in the app; assemblies too: the assembly's own evaluation, its Configurations box, the Insert dialog's configured rows and the instance's Change configuration panel hide hidden inputs/options and use the effective values; test `assembly_configuration_respects_visibility`) |
| 13.28 | Configurations panel ⋯ menus: input ⋯ = Copy table, Copy input, Move up / Move down (when possible), Rename, Visibility conditions…, Input propagation… (disabled without a matching Derived feature), Delete; panel ⋯ = Copy all inputs, Collapse all inputs / Expand all inputs; Configure Part Studio ▾ / Add configuration input ▾ gain Paste inputs while inputs are copied (kept for the session; existing ids are skipped) | Help PartStudio__managing_configurations | ✅ (verified in the app: Copy table gives tab-separated text "<feature names>" / "Name<TAB><params>" / one row per option, e.g. "\tExtrude 1 / Name\tDepth / Standard\t40 mm / Long\t60 mm"; exact live clipboard format requested as psb 10-20 j) |
| 13.29 | Derived from a configured Part Studio: the dialog shows the source's inputs under the Part Studio select (lists as selects, checkboxes), hidden ones per its visibility conditions; the derived parts follow the chosen configuration | Help PartStudio__derived | ✅ (verified: Pin Length Standard/Long → 40/60 mm pin; live look requested, psb 10-20 j) |
| 13.30 | Input propagation…: "Input propagation for <Part Studio>" dialog, References / Configuration table with header and row checkboxes ("Propagate"), "N new inputs to propagate", Confirm; confirming adds a "<Derived> / Configuration" column mapping each option to the same option (cells are option selects) | Help PartStudio__input_propagation | ✅ (verified in the app; live DOM requested, psb 10-20 j) |
| 13.31 | Export of parts (parts list › Export…, release rows of this or another document/version): File name "<Part Studio> - <Part>" for one part, Format, Export models oriented Y axis up, Version / Use latest version, Options; no "Export unique parts as individual files" / "Include hidden instances in export" (those are the tab export's) | live (lead's round 3 comparison) | ✅ (verified in the app from the parts list, a workspace release row and a V1 release row, which opens the read-only version window first) |
| 13.32 | Search tools popover: 600 px, 10 px under the toolbar at the window's right edge, drawn over the feature list and any open right panel | live menus/tool-search-* | ✅ (was clipped under the feature list while a right panel was open) |
| 13.33 | Variable table editing (live 550–565): add row at the rollback bar with focus to the next Name row; a taken name → `name_0` + "Renamed some new variables to avoid name conflicts."; invalid name pink `#f9ebec`/`#bd3039` with tooltip, Enter does nothing, Esc clears; rename to a taken name reverts; only variables before the rollback bar; types Length/Angle/Number/Any/Measured/From import ("Not set" ✎); row menu without Move up/down at the ends; Description adds Copy; ⋯ = Copy table; a bare number takes the workspace unit | live psb 10-20 o, qa4 13:30, 652–656 | ✅ (incl. Description Convert to expression / text) |
| 13.34 | Measure panel (x) "Add variable for this measurement" on distance rows only; opens a Measured Variable (Distance Min/Max, Along X/Y/Z, Length, Diameter) with the picks, Name empty; panel closes | live 566–572 | ✅ |
| 13.35 | Create variable popup: Value / Default = the field's previous value, type tab from the field; Configuration tab Length/Angle/Integer/Real/Text with Name, Default, Minimum 0, Maximum 100000; name / range errors disable ✓; ✓ keeps `#name` in the focused field, ✕ selects it | live 577–584, bundle CreateVariable | ✅ |
| 13.36 | Filtered feature list: blue funnel, ⊗ clear, "Features (k of N)", Default geometry with "...", one "..." per hidden run, rollback bar kept | live 564 | ✅ |
| 13.37 | Edit FeatureScript IDs: invalid id = tooltip only, duplicate = disabled button only, no spaces; excluded icon between chevron and name and after the value in the Configurations box | live 520–535 | ✅ |
| 13.38 | Configured properties tab: Properties for ▾, Add property ▾, ⋯, per-input + property table, empty-state and "not affecting" info lines | live 526–536, 600–656 | 🔧 (all cell editors, Tab / Enter moves, paste down a column; rows from BOM edits need asb's BOM to call `set_for_configuration`) |
| 13.39 | Inspection › Datum: "Datum ✓ ✕" with the next letter, placed on a planar face as a preview, boxed letter + triangle in the face plane at constant screen size, right-click Show all / Select / Zoom to fit / Delete annotation, Model definitions Datums (n/n) and the Characteristics header | live psb/r-910–922 | 🔧 (drag, Select other…, default tolerances, GTol / Weld / Dimension placement open; partly uncaptured) |
| 13.40 | Variable Studio tables above the Part Studio's own table, stored per Part Studio; lowest wins a clash; header drag with grabbing cursor, pill ghost and blue drop line (own table refused, red); shadowed rows ⓘ "Overwritten in another variable table." | live psb 10-20 t | 🔧 (ⓘ waits on ui2's variable_studio.rs hook) |
| 13.41 | Model definitions: Isolate active part; "Select default tolerances…" picker (ISO 2768-1 grades c/f/m/v or Precision; applies on open), chip with hover-only Remove, "Default tolerances" section; Characteristics: per-type cells, Default rows from the standard, rolled-back rows, Hide default tolerances, Types ▾ (badge), Show/Hide derived tolerances, Reset filters, row hover/click | live qa4 598–599, psb 10-21 b/c | 🔧 (angle band, Fillet-radius type filter: psb 10-21 d) |
| 13.42 | Slow regenerations: newest edit wins (stale runs aborted or finished into the cache), old result kept, working-overlay spinner after 1.2 s ("Opening <feature>…" when a dialog opens), ✓ closes at once with the spinner until the final result, Esc doesn't cancel | live "Pending state" | ✅ |
| 13.43 | Fillet / Chamfer edge fields take part edges only (Onshape `EdgeTopology.TWO_SIDED`): a sketch line lying on a part edge isn't picked, the part edge is; sketch-only fields (Rib profiles, surface Extrude curves) take no part edges | app 10-24 (a chamfer picked "Sketch 1 line") | ✅ `QueryFilter::SOLID_EDGE`, `kind_matches`; the pick pass skips the overlay kinds (sketch curves/points/regions, part edges/vertices) the active field rejects (render state `NO_PICK`), so a click on a shown sketch line lying on a part edge takes the edge (app: Chamfer with Sketch 1 shown → "Edge of Extrude 1"; render test `no_pick_overlay_lets_the_edge_under_it_be_picked`) |
| 13.44 | A Hole whose cut fails (tangent to a face: non-manifold) reports hole.fs's HOLE_CUT_FAIL "Hole creates invalid geometry." and cuts nothing; a Reapply feature pattern of it is OK and silently skips the instances that fail on their own; a hole whose axis is beside the part but whose cylinder crosses it cuts (rays across its disk) | live "qa4 round 5 v2 s1 follow-up" (2) | ✅ test `tangent_hole_fails_and_its_reapplied_pattern_cuts_the_good_copies` (volume = live's 37453.538 notch) |
| 13.45 | A sketch region that no longer resolves (its sketch deleted or moved below) is a missing selection: the feature reports its own empty-input message ("Select face or sketch region to extrude.") and the row tooltip adds "1 missing selection"; a feature that still works with the rest warns "1 missing selection" | live qa4 round 5 v3 s8 (ERR-MISSING-REGION) | ✅ (`drop_missing_regions`, `FeatureState::missing`; test `extrude_above_its_sketch_misses_its_region`) |
| 13.46 | Hole "Sketch points to place holes" takes sketch points and explicit mate connectors only: a face or a part vertex is neither highlighted nor added | live "qa4 round 5 s1 follow-up 2" (1) (HOLE-FACE) | ✅ (`kind_matches`: no implicit mate connector in a sketch-point field; a part vertex isn't a sketch point; test `face_fields_take_no_edges`) |

## Not applicable to a local app

Collaboration (follow mode, comments), release approvers and notifications, App Store,
permissions/view-only toolbar, browser-specific shortcuts (Ctrl+click to open
in a new tab), mobile gestures.

