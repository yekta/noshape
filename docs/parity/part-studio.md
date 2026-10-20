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
| 1.2 | Enter works after clicking the feature list/viewport (focus left the dialog) | keyboard_shortcuts | 🔧 |
| 1.3 | Enter with a feature selected in the list (no dialog) opens it for editing | A §3.3 (implied by Edit) | 🔧 |
| 1.4 | Shift+Enter commits and reopens the same tool, empty | keyboard_shortcuts; Home__dialogs | ✅ |
| 1.5 | Esc exits the command / cancels the dialog | keyboard_shortcuts | ✅ |
| 1.6 | Esc in a changed dialog field reverts the typed value; the next Esc cancels the dialog | live (behaviours.md: Feature dialog basics) | ✅ |
| 1.7 | Esc closes transient UI first (S toolbar, Select other, zoom-window mode), then exits isolate/transparent, then clears the selection | View__isolate; select_other | 🔧 |
| 1.8 | Space deselects everything (features and entities) | keyboard_shortcuts | 🔧 (features were not cleared) |
| 1.9 | Ctrl/⌘+C copies selected features | keyboard_shortcuts | ✅ |
| 1.10 | Ctrl/⌘+V pastes copied features (repeatable, at the rollback bar) | keyboard_shortcuts | ✅ |
| 1.11 | Delete/Backspace removes selected features | keyboard_shortcuts | ✅ |
| 1.12 | Delete with parts selected deletes them (Delete part feature) | A §3.9; Home__selection | 🔧 |
| 1.13 | Ctrl/⌘+Z / Ctrl+Y / ⇧⌘Z undo/redo; inside a dialog they step the dialog's own edits | keyboard_shortcuts; A §2.1 | ✅ |
| 1.14 | Shift+N renames the selected feature in place | keyboard_shortcuts | ✅ |
| 1.15 | Alt/⌥+C opens Search tools | keyboard_shortcuts; Home__search_tools | ✅ |
| 1.16 | Shift+/ toggles the keyboard shortcut map | keyboard_shortcuts | ✅ |
| 1.17 | Shift+/ opens Onshape's shortcut sheet: tabs General, Part Studio, Assembly, 3D view, Sketch, Feature Studio, Drawing; key chips + action, 🔒 locked; search across tabs; legend | live (shortcuts/, dom/keyboard-shortcuts.txt) | ✅ (the live 114-row list) |
| 1.18 | ` (grave) opens Select other at the cursor | keyboard_shortcuts; Home__select_other | 🔧 |
| 1.19 | ` again moves down the Select other list, Shift+` up, Enter picks | Home__select_other | 🔧 |
| 1.20 | S opens the shortcut toolbar at the cursor: 5x2 tools (Part Studio: Sketch, Extrude, Revolve, Sweep, Loft / Fillet, Chamfer, Hole, Boolean, Transform) | live (shortcuts/16) | ✅ |
| 1.21 | S toolbar contents customizable in preferences | part_studios | ✅ (Customize toolbar > Shortcut toolbar (S) checklist; saved in the config dir) |
| 1.22 | [ opens the Measure tool | keyboard_shortcuts | 🔧 |
| 1.23 | Ctrl+M opens the Mate connector dialog | keyboard_shortcuts | 🔧 |
| 1.24 | K shows/hides mate connectors | keyboard_shortcuts | 🔧 (toggles the Mate connector features' visibility) |
| 1.25 | A flips the primary axis, Q reorients the secondary axis while editing a mate connector | keyboard_shortcuts | ✅ |
| 1.26 | Shift+C Curve/surface analysis (curvature combs on edges, face U/V isocurves, sketch curves; inflection points, minimum radius, extend across face, control point grids, scale), Shift+D Dihedral analysis (combs, Max/Min, thresholds, scale); Analysis tools menu | keyboard_shortcuts; View__curve_surface_analysis; View__dihedral_analysis; View__analysis_tools | ⏳ (Curve/surface, Deviation, Connection, Dihedral, Zebra stripes, Reflection, Curvature color map (Onshape's color bar: Viridis / Blue - > Red / Plasma / Rainbow, Gaussian / Mean / Max radius / Min radius from mesh principal curvatures, flat faces keep their color), Thickness analysis (ray / rolling ball / gradients, color bar, Dim color scale, bounds, Edit / Turn off) ✅ verified in the app; Draft analysis… as Onshape's dialog (Mold split direction + flip, Minimum draft angle, Entities to check, Show undercut regions by ray casting; legend Steep / Undercut / Side 1 / Side 2 at the top, side-1 arrow, angle under the cursor) built; Reflection's library maps are procedural stand-ins until live/env-maps is captured and its "Other documents" tab is empty (no image tabs yet); Flatten surfaces (unfold + relaxation, rips and seams, translucent patch in the Top plane, facets / edges / checkerboard / distortion, Origin, mate connector position, offset / angle, STL / DXF / SVG export) ✅; Interference detection… (pairwise overlaps in red, parts translucent, Interferences list with bounding box and size on hover) built, dialog details await the live capture; menu labels and order as live (with "…"); Lock environment = fixed to the model (live); zebra pattern = latitudes / longitudes of a room around the model (rings and radial stripes on flat faces, live); our own environment maps modeled on Onshape's; Flatten's PARASOLID export is missing (no Parasolid writer); Thickness palettes beyond Viridis are guesses (not on the free account)) |
| 1.27 | Alt/⌥+T tab manager (search, open), Ctrl+Space switches to the previously active tab | keyboard_shortcuts | 🔧 |
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
| 2.6 | Rollback bar selected + ↑/↓ steps the bar | keyboard_shortcuts; features_and_parts_lists | 🔧 (clicking the bar now gives the list focus) |
| 2.7 | Shift+1…7 Front, Back, Left, Right, Top, Bottom, Isometric | keyboard_shortcuts | ✅ |
| 2.8 | N view normal to the hovered/selected plane, face or sketch; N again flips to the reverse | keyboard_shortcuts | ✅ |
| 2.9 | F zoom to fit | keyboard_shortcuts | ✅ |
| 2.10 | Z zoom out, Shift+Z zoom in (about the view center) | keyboard_shortcuts | ✅ |
| 2.11 | W zoom to window: drag a box to magnify; W again cancels | keyboard_shortcuts; View__zoom_to_fit | 🔧 |
| 2.12 | ←→↑↓ rotate 15°, Ctrl 5°, Shift 90° | keyboard_shortcuts | ✅ |
| 2.13 | Ctrl+Shift+←→↑↓ pan (150 px per step) | keyboard_shortcuts; A §6.1 | 🔧 (bound but unhandled) |
| 2.14 | P shows/hides planes | keyboard_shortcuts | ✅ |
| 2.15 | Shift+P hides all construction geometry (planes, sketches); again shows it | keyboard_shortcuts | 🔧 (bound but unhandled) |
| 2.16 | Y hides the hovered entity's part/sketch/plane (or the selection) | keyboard_shortcuts; View__hide_show | 🔧 (bound but unhandled) |
| 2.17 | Shift+Y shows hidden entities (selected ones, else all parts) | keyboard_shortcuts; View__hide_show | 🔧 |
| 2.18 | Shift+I isolates the hovered/selected parts (others transparent); cumulative | keyboard_shortcuts; View__isolate | 🔧 |
| 2.19 | Shift+T makes the hovered/selected parts transparent; cumulative | keyboard_shortcuts; View__make_transparent | 🔧 |
| 2.20 | Shift+X opens the Section view dialog (again with a section on: turns it off) | keyboard_shortcuts | ✅ |
| 2.21 | Shift+V named views | keyboard_shortcuts | ✅ |
| 2.22 | Shift+R view in high quality (4x finer tessellation, notice) | keyboard_shortcuts | 🔧 |
| 2.23 | Alt+click selects through transparent geometry | keyboard_shortcuts | 🔧 (isolate/transparent and translucent part appearances) |
| 2.24 | O toggles orthographic/perspective (noshape extra; Onshape uses the camera menu) | View__perspective_view | ✅ |

## 3. Selection (Home__selection, Home__select_other, A §5)

| # | Behaviour | Source | Status |
|---|---|---|---|
| 3.1 | Click selects, click again deselects; selection is additive without modifiers | Home__selection | ✅ |
| 3.2 | Click in empty space clears the selection (entities and features) | Home__selection | 🔧 (features stayed selected) |
| 3.3 | Hover pre-highlights the entity under the cursor | A §5.3 | ✅ |
| 3.4 | Box select left→right = window (fully inside), right→left = crossing | Home__selection | ✅ |
| 3.5 | Window box is solid, crossing box dashed | Home__selection; A §5.2 | ✅ |
| 3.6 | Ctrl+drag box deselects | Home__selection | 🔧 |
| 3.7 | Box selection with a dialog open fills the active field (filtered) | A §5.4 | 🔧 |
| 3.8 | Selected entities show quick values in the Measure readout | A §5.1 | ✅ |
| 3.9 | Selection count next to the cursor, 1–5 ("5" for five or more) | Home__selection; live cursors/ | ✅ (Onshape bakes the digit into the cursor image; GPUI has no image cursors, so a black digit with a white outline is drawn at the arrow's lower right) |
| 3.10 | Midpoints of edges show on hover and can be picked (measure, planes) | Home__selection | 🔧 (part edges and open sketch curves; with a dialog open when its field takes points; regen resolves sketch-curve midpoints) |
| 3.11 | Select other lists faces and edges under the cursor nearest first; hovering a row highlights it; click/Enter picks; list closes | Home__select_other | 🔧 |
| 3.12 | Select other picks go to the active dialog field | Home__select_other | 🔧 |
| 3.13 | Select other from the face/edge context menu | Home__select_other | 🔧 |
| 3.14 | Selecting a face or part highlights the features that created/modified it in the feature list | features_and_parts_lists | 🔧 |
| 3.15 | Hovering a feature in the list highlights its geometry | features_and_parts_lists ("Hover over a feature…") | 🔧 |
| 3.16 | Selecting a feature in the list highlights its geometry | A §5.3 (FEATURE_HIGHLIGHT) | 🔧 |
| 3.17 | Selecting / hovering a part in the parts list highlights the whole part | A §5.3 (PART_HIGHLIGHT) | 🔧 |
| 3.18 | Double-click a sketch in the graphics area edits it | (Onshape behaviour) | 🔧 |
| 3.19 | Create selection dialog and Select ▸ submenu | Home__create_selection; live part-studio/380–398, dom/dialog-create-selection.txt, context-menus-ps-part-select-submenus.txt | 🔧 (Select ▸: face/region = Protrusion, Pocket, Hole, Fillet, Tangent connected, Bounded faces, Loop/chain connected edges, Create selection…; part edge and sketch curve = Tangent connected, Equal length/radius, Parallel, Create selection…; no separator; each item replaces the selection. Create selection (`part_studio/create_selection.rs`): 220 px, floats at the right-click point, header ✕ only, draggable; Faces / Edges tabs (opens on Faces), native-style select (Protrusion…Bounded faces / Tangent connected, Loop/chain connected, Equal length/radius, Parallel), Selection field (empty at open, picks toggle), ☐ Select patterns (pattern copies via `select::with_pattern_instances`), "N faces selected", [Add selection] (disabled at 0) adds to the selection and resets, staying open; preview seeds orange / results query yellow-green. Feature fields with SHOW_CREATE_SELECTION (Delete/Move/Replace face, Offset surface, Draft parting edges, face patterns / mirror) and Flatten's two fields have the 26 px button (tooltip "Create selection"); it opens the dialog beside the feature dialog, the field turns white with a blue border, Add selection fills the field. Convexity follows qConvexConnectedFaces (exterior fillet edges convex). Verified in the app.) |

## 4. Preselection and tools (A §2.3, PartStudio__feature_basics)

| # | Behaviour | Source | Status |
|---|---|---|---|
| 4.1 | Entities selected before starting a feature fill its first compatible field | A §2.3 | ✅ |
| 4.2 | A sketch selected in the feature list preselects all its regions (Extrude, Revolve…) | features_and_parts_lists | 🔧 |
| 4.3 | Planes / features selected in the list preselect plane / feature fields | A §2.3 | 🔧 |
| 4.4 | Clicking the active toolbar tool again closes it | Home__selection | 🔧 |
| 4.5 | Picking a tool from a toolbar dropdown makes it the group's face for the session | A §2.3 | 🔧 |
| 4.6 | Toolbar tooltips show the shortcut | A §2.2 | ✅ |
| 4.7 | Customize toolbar (drag tool sets, Reset) | part_studios | ✅ (built by an earlier pass: move tool sets, remove/restore tools, reset) |
| 4.8 | Search tools: typing filters tools, Enter runs the first, Esc closes | Home__search_tools | ✅ |
| 4.9 | Search tools with no query lists recently used tools first; description on hover | Home__search_tools | 🔧 |
| 4.10 | REMEMBER_PREVIOUS_VALUE parameters default to the last accepted value of that tool | A §7.2 | 🔧 |

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
| 5.8 | Picks that don't pass the field filter are rejected: the field flashes red and a message names what it takes | A §5.4 | 🔧 (generic message before) |
| 5.8b | Clicking a sketch curve while a region field is active takes the region it bounds (the smaller one for a curve between two regions); hovering the curve pre-highlights that region | live (Extrude dialog) | ✅ (was "all regions"; changed per live) |
| 5.9 | List clicks (default planes, parts) obey the field filter | A §5.4 | 🔧 (were added unfiltered) |
| 5.10 | Clicking a sketch in the feature list fills a region field with all its regions, shown as one "Faces of Sketch 1" row; clicking again removes them; feature fields take the feature | features_and_parts_lists; live | ✅ |
| 5.11 | Single-pick fields replace their pick and move focus to the next empty selection field | A §5.4 | ✅ |
| 5.12 | × on a chip removes that pick | feature_basics | ✅ |
| 5.13 | Right-click a pick > Clear all (plus Clear missing references when some are missing); × on hover removes one row | live (Fillet dialog) | ✅ |
| 5.14 | Hovering a pick row highlights its entity in the graphics area, tints the row and shows its name as a tooltip | Home__dialogs; live | ✅ (yellow-green #f2ff80 state QUERY in the shader) |
| 5.15 | Missing references shown red; Clear missing references | feature_basics | ✅ (verified: red rows, accept disabled, Clear missing references) |
| 5.16 | Reorder picks in order-sensitive fields by dragging a row | feature_basics | 🔧 (the Move up/down menu items were removed: live menus only have Clear all) |
| 5.17 | Tab moves between fields (inputs and checkboxes) | feature_basics | ✅ |
| 5.18 | Space toggles a focused checkbox; arrows/Enter in dropdowns | feature_basics | ✅ (Space toggles a checkbox; Enter/Space opens a dropdown, arrows move, Enter picks — the list opens without the current value highlighted) |
| 5.19 | Numeric fields accept expressions, units and #variables | Home__numeric_fields | ✅ |
| 5.20 | Invalid expression: red outline and message | Home__numeric_fields | ✅ |
| 5.21 | Expressions show their evaluated value | Home__numeric_fields | 🔧 |
| 5.22 | Mouse wheel over a focused numeric field: ±0.1, Ctrl ±0.01, Shift ±1 (integers ±1), clamped to the field range | Home__numeric_fields | 🔧 |
| 5.23 | Field shows the converted value after accepting, the original text when focused | Home__numeric_fields | ✅ (fixed: focusing showed the converted text) |
| 5.24 | Final button previews the end of the history; absent for new features and the last feature | Home__dialogs | 🔧 (was always shown) |
| 5.25 | Preview opacity slider (blends the model before and after the feature) | Home__dialogs | 🔧 |
| 5.26 | Rename the feature from the dialog title | Home__dialogs | ✅ (double-click the title) |
| 5.27 | Dialog movable and resizable; help (?) icon | Home__dialogs | ✅ (move by the title, resize by the bottom-right grip; size kept for the session) |
| 5.28 | Boolean "Convert to expression" / "Convert to checkbox" (evaluated at regeneration with upstream variables) | feature_basics | ✅ (starts as true/false text, focused) |
| 5.29 | Tolerance options on tolerant quantities (± icon "Tolerance options": precision ▾ then tolerance type ▾; Hole diameter adds Fit / Fit with tolerance / Fit (tolerance only) with Standard, Fit type, Hole class, Shaft class) | Home__tolerance_options; live dialogs/hole-30–37 | ✅ (`model::fits`; the hole callout in the graphics area shows "Ø6.350 H7 +0.015/−0.000 THRU") |
| 5.30 | Extrude/Revolve from a part face (or a sketch on one) defaults to Add with that part as the merge scope | (Onshape behaviour, A §7) | 🔧 |
| 5.31 | Choosing Remove/Intersect on such a feature points it into the part | (Onshape behaviour) | 🔧 |
| 5.32 | Drag handles for depth/angle/distance (extrude arrow, revolve arc, plane offset, shell thickness, fillet radius, chamfer distance) | A §7.7 | ✅ (hover outlines the arrow blue) |
| 5.33 | Editing a feature rolls the model back to it | Home__dialogs | ✅ |

## 6. Feature list (PartStudio__features_and_parts_lists, A §3)

| # | Behaviour | Source | Status |
|---|---|---|---|
| 6.1 | Feature count in the header | features_and_parts_lists | ✅ |
| 6.2 | Default geometry rows (Origin, Top, Front, Right) | features_and_parts_lists | ✅ |
| 6.3 | Each feature with its tool icon | features_and_parts_lists | ✅ |
| 6.4 | Double-click a feature to edit it | A §3.3 | ✅ |
| 6.5 | Click toggles a feature in the selection (additive) | features_and_parts_lists ("just click to select additional items") | 🔧 (click replaced the selection) |
| 6.6 | Shift+click selects a range | Home__selection | 🔧 (Shift toggled) |
| 6.7 | Drag a feature to reorder | features_and_parts_lists | ✅ |
| 6.8 | Dragging a selected feature moves every selected feature (non-contiguous allowed) | features_and_parts_lists | 🔧 |
| 6.9 | Reorder refused above a feature's dependencies, with an error ("Cannot reorder: X depends on Y.") | (Onshape behaviour) | 🔧 |
| 6.10 | Search box filters by name or type | features_and_parts_lists | 🔧 |
| 6.11 | Filter commands :errors :warnings :suppressed :hidden :shown :type :name :folder :variable :part | features_and_parts_lists | 🔧 (:partnumber / :revision match the parts' Part number / Revision properties) |
| 6.12 | Error icon with count in the header; click filters to errors | features_and_parts_lists | 🔧 |
| 6.13 | Failed features red with the error as tooltip | Home__error_indicators | ✅ |
| 6.14 | Suppressed features struck through | features_and_parts_lists | ✅ |
| 6.15 | Under-defined sketch indicator | features_and_parts_lists | ✅ (blue text) |
| 6.16 | Missing-selection / warning yellow triangle | features_and_parts_lists | 🔧 |
| 6.17 | Eye icon on hover hides/shows sketches, planes, mate connectors | features_and_parts_lists | 🔧 |
| 6.18 | Rename in place (context menu / Shift+N) | features_and_parts_lists | ✅ |
| 6.19 | Feature context menu in the live order: Rename | Edit…, Edit <sketch>…, Copy sketch | Add selection to folder… | Show/Hide | Export as DXF/DWG… | Suppress, Configure suppression, Dynamic suppression ▸ | Zoom to selection, View normal to | Show dependencies… | Roll to here (not on the last feature) | sketch/feature appearance | Delete | live (dom/context-menus-partstudio.txt) | ✅ (Create Drawing of Sketch ❌ needs sketch views in drawings; Add comment out of scope: cloud comments) |
| 6.20 | Several selected features: Add selection to folder… | Suppress, Dynamic suppression ▸ | Clear selection | appearance | Delete | live | ✅ (Section view… through the selected sketches' / planes' planes) |
| 6.21 | Show dependencies…: "Dependencies of X" panel at the dialog position: parents, ▲, the feature, ▼, children (icons, hover highlights, tooltips) | live (part-studio/100) | ✅ |
| 6.22 | Dynamic suppression ▸ Add expression… (expression editor, ƒx icon, live true/false preview) | live | ✅ |
| 6.23 | Copy sketch / Paste sketch onto a plane or planar face | Home__context_menus | ✅ (verified: Copy sketch, Front plane > Paste sketch) |
| 6.24 | Show dimensions (feature/face/sketch menus): the sketch's dimensions in 3D, parts see-through, hover gold on tan, double-click edits the sketch, click elsewhere or Esc hides; Export as DXF/DWG | Home__context_menus; live part-studio/40–42 | ✅ (Create drawing of sketch ❌) |
| 6.25 | Pause regeneration (banner: Regenerate / Regenerate and exit / Discard) ; regeneration times panel (hover highlights) | features_and_parts_lists | 🔧 |
| 6.26 | Collapse the whole list; drag the feature/parts separator | features_and_parts_lists | 🔧 |
| 6.27 | Default plane row menu (live): Rename | New sketch…, Offset plane…, Size to Part Studio (Paste sketch when a sketch was copied) | Hide, Hide other planes, Hide all planes | Zoom to selection, View normal to | Show dependencies…; origin: Rename | Hide | Show dependencies… | Hide, Hide other planes, Hide all planes | ✅ |
| 6.28 | Default planes/origin can be hidden (never deleted) | part_studios | 🔧 |
| 6.29 | Clicking empty list space clears the feature selection | (Onshape behaviour) | 🔧 |

### Rollback bar

| # | Behaviour | Source | Status |
|---|---|---|---|
| 6.30 | Drag the bar to roll back/forward; model shows up to the bar | features_and_parts_lists | ✅ |
| 6.31 | Right-click a feature > Roll to here | features_and_parts_lists | ✅ |
| 6.32 | (Right-click the bar > Roll to end) | features_and_parts_lists | ✅ removed (live has no menu on the bar) |
| 6.33 | Select the bar, ↑/↓ step one feature | features_and_parts_lists | ✅ (also after clicking a feature row; the bar turns #a0c7dd while focused) |
| 6.34 | Features below the bar greyed; new features inserted at the bar | A §3.2 | ✅ |

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
| 7.2 | Click selects (or fills a part field in a dialog, filtered) | features_and_parts_lists | 🔧 (unfiltered before) |
| 7.3 | Hover highlights the part in the graphics area | features_and_parts_lists | 🔧 |
| 7.4 | Eye on hover hides/shows the part; hidden parts greyed | features_and_parts_lists | 🔧 |
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
| 8.6 | Sketch region/curve: Edit <sketch>…, Show dependencies… | Extrude…, Revolve… (region) | Hide, Section view… | Export as DXF/DWG… | Select other… | Zoom to fit, Zoom to selection, View normal to | Delete <sketch> | Edit sketch appearance… | live (dom/context-menus-ps-sketch-entities.txt) | ✅ (verified in the app: region New sketch… / Offset plane… on the region; curve Copy sketch entities (pastes into a sketch), Hide other sketches / Hide all sketches, Select ▸ as live (region: Protrusion / Pocket / Hole / Fillet / Tangent connected / Bounded faces, Loop/chain connected edges; line or circle: Tangent connected, Equal length/radius, Parallel edges; + Create selection…; each replaces the selection), Align view with vertical axis on straight lines; midpoints of sketch curves on hover; Create Drawing of Sketch ❌ needs sketch views in drawings; Add comment out of scope: cloud comments) |
| 8.7 | The right-clicked entity stays highlighted while its menu is open | live (menus/rmb-ps-face.png) | ✅ |
| 8.8 | Exit isolate / Exit make transparent / Turn off section view when active | View__isolate; View__section_view | 🔧 |
| 8.9 | Creation commands carry their tool icon | Home__context_menus | ✅ (creation, hide, section, drawing, delete and appearance entries carry icons) |
| 8.10 | Exit <tool> entry while a tool is active | Home__selection | 🔧 |
| 8.11 | Create selection / Select ▸ / Add appearance / Section view / Paste sketch entries | A §4 | ✅ (Create selection… is the last entry of Select ▸) |

## 9. View navigation, view cube, camera menu (View__view_navigation_and_the_view_cube, A §6)

| # | Behaviour | Source | Status |
|---|---|---|---|
| 9.1 | Right-drag rotates, middle-drag or Ctrl+right-drag pans, wheel zooms at the cursor | view_navigation | ✅ |
| 9.2 | Double-click the middle button zooms to fit | View__zoom_to_fit | 🔧 |
| 9.3 | View cube in the upper right turns with the camera | view_navigation | 🔧 |
| 9.4 | Click a cube face for that standard view | view_navigation | 🔧 |
| 9.5 | Click a cube edge or corner for the view along it | view_navigation; A §6.2 | 🔧 |
| 9.6 | Cube faces/edges/corners highlight on hover (name shown) | A §6.2 | 🔧 |
| 9.7 | Cube arrows rotate 15° (Shift 90°, Ctrl 5°); arc arrows roll | A §6.2 | 🔧 |
| 9.8 | Camera menu (live order): Isometric/Dimetric/Trimetric | Named views…, Previous view | Zoom to fit/window | Perspective, Orient normal to sketch on edit | ✓ display ▸, ✓ hidden edges ▸, ✓ tangent edges ▸ | View in high quality | Section view… | live dom/menu-camera-render.txt | ✅ (Graphics preferences… and Highlight boundary edges by ui) |
| 9.9 | Previous view returns to the camera before the last view change | View__previous_view | 🔧 |
| 9.10 | Zoom to selection (parts, faces, edges, sketches, planes, features) | View__zoom_to_fit | 🔧 |
| 9.11 | Display: Shaded with edges / Shaded without edges / Unshaded / Translucent; Hidden edges visible/removed; Tangent edges visible / phantom (dash-dot) / removed | live | ✅ (tangency found from the mesh: `MeshBuffers::smooth_edges`) |
| 9.12 | Orient normal to sketch on edit (camera menu toggle); the previous view returns when the sketch closes | View__orient_normal_to_sketch_on_edit | 🔧 (not persisted across sessions) |
| 9.13 | Named views store orientation/zoom/projection; save, restore, delete | View__named_views | ✅ |
| 9.14 | Named views store the section state | View__named_views | 🔧 |
| 9.15 | Rotation inertia; rotate about the point under the cursor | A §6.1 | 🔧 |

## 10. Visibility, isolate, transparency, section (View__hide_show, View__isolate, View__make_transparent, View__section_view)

| # | Behaviour | Source | Status |
|---|---|---|---|
| 10.1 | Hide/Show parts, sketches, planes, mate connectors, default geometry | View__hide_show | 🔧 (parts only before) |
| 10.2 | Hide other parts / Hide all parts / Show all / Show all parts | View__hide_show | 🔧 |
| 10.3 | Isolate keeps chosen parts opaque, others transparent | View__isolate | 🔧 |
| 10.4 | Make transparent makes chosen parts transparent | View__make_transparent | 🔧 |
| 10.5 | Isolate/transparent panel with transparency control and × to exit; Esc exits | View__isolate | 🔧 |
| 10.6 | Select transparent geometry checkbox; Expand by distance | View__isolate | 🔧 |
| 10.7 | Section view dialog (live): Exclude | Include tabs, "Section planes" field (planes, planar faces, mate connectors; × removes) + a view-facing plane button, "Items to exclude/include"; ✓/Enter keeps, ✕/Esc restores; empty-space menu Edit section view… / Turn off section view | live part-studio/70–74 | ✅ |
| 10.8 | Section by several planes (up to 3), drag arrow on the active plane with an offset box, caps | View__section_view | ✅ (caps: the cut solid is capped in the part color with hatching, separate section pipeline; offscreen test `section_caps_cover_the_cut`) |

## 11. Measure, mass properties, appearance (View__measure_tool, View__mass_properties_tool, PartStudio__customizing_appearances)

| # | Behaviour | Source | Status |
|---|---|---|---|
| 11.1 | Measure readout for the selection (length, area, radius, distance, angle, ΔX/ΔY/ΔZ); the first row follows the client's order (bundle `setMeasurements`: Diameter / Radius / Parallel (axis) distance or an angle first): two perpendicular faces read "Perp planes angle: 90.000 deg" (live part-studio/81, assembly/707), parallel faces "Parallel dist", pairs "Min dist", face totals "Area"; values with fixed decimals and "deg" ("90.000 deg", "400.000 mm²") | View__measure_tool, live 81/707 | ✅ (verified in the app; Part Studio and assembly share `measure::measure_shapes`; labels for line/axis pairs inferred from the bundle's measurement types — live request psb 10-20 m) |
| 11.2 | Measure/Mass properties buttons at the bottom right; [ opens Measure with a prompt when nothing is selected | A §1, §6.10 | 🔧 |
| 11.3 | Measure graphics (dotted min distance with X/Y/Z components) and measure type selector | View__measure_tool | 🔧 |
| 11.4 | Mass properties: volume, surface area, mass (with material), center of mass | View__mass_properties_tool | ✅ |
| 11.5 | Moments of inertia about the center of mass; Face tab; reference mate connector; override mass | View__mass_properties_tool | 🔧 (several parts: each part's inertia moved to the joint centre of mass by the parallel-axis term, test `mass_properties_move_each_part_to_the_joint_centre`; Face tab / reference mate connector / override mass still open) |
| 11.6 | Part appearance colour, transparency, tessellation quality (Appearance editor) | customizing_appearances | 🔧 (material still in Part properties) |
| 11.7 | Appearance panel, face/feature/sketch appearances | customizing_appearances | 🔧 (no custom-color tiles / mixer tab) |

## 12. Variables and configurations (PartStudio__variable_table, PartStudio__configurations)

| # | Behaviour | Source | Status |
|---|---|---|---|
| 12.1 | Variable table panel on the right | variable_table | ✅ |
| 12.2 | Configuration panel and configuration selectors | configurations | ✅ |
| 12.3 | `#variable` in any numeric field; :variable filter finds users | variable_table; features_and_parts_lists | 🔧 (filter) |
| 12.4 | Hover a variable highlights the features that use it; right-click > Filter feature list | variable_table | 🔧 |

### Found in the live capture

| # | Behaviour | Source | Status |
|---|---|---|---|
| 13.1 | A new feature hides the sketch it consumes | live (selection/02) | ✅ |
| 13.2 | Ctrl+A does nothing in the Part Studio | live (shortcuts/14) | ✅ (not bound) |
| 13.3 | Delete/Backspace on a selected feature deletes it at once (no confirmation, no toast); Ctrl+Z restores it | live (shortcuts/15) | ✅ (Delete and Ctrl+Z verified) |
| 13.4 | Tab skips the End type dropdown; Shift+Tab from Depth reaches the flip button | live | ✅ |
| 13.5 | Depth arrow: hover outlines it blue; dragging updates the Depth field with the value selected; no viewport label | live (dialogs/extrude-05..07) | ✅ (drag end focuses the field with the value selected; hover outlines the arrow blue) |
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

## Not applicable to a local app

Collaboration (follow mode, comments), release approvers and notifications, App Store,
permissions/view-only toolbar, browser-specific shortcuts (Ctrl+click to open
in a new tab), mobile gestures.

