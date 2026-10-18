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
| 1.1 | Enter commits the open dialog | keyboard_shortcuts; Home__dialogs | ✅ |
| 1.2 | Enter works after clicking the feature list/viewport (focus left the dialog) | keyboard_shortcuts | 🔧 |
| 1.3 | Enter with a feature selected in the list (no dialog) opens it for editing | A §3.3 (implied by Edit) | 🔧 |
| 1.4 | Shift+Enter commits and reopens the same tool, empty | keyboard_shortcuts; Home__dialogs | ✅ |
| 1.5 | Esc exits the command / cancels the dialog | keyboard_shortcuts | ✅ |
| 1.6 | Esc while typing in a dialog field cancels the dialog | feature_basics ("cancel at any time … Escape") | 🔧 |
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
| 1.17 | Shortcut map has a search box (and tab-type filter) | keyboard_shortcuts | 🔧 (search; no tab filter) |
| 1.18 | ` (grave) opens Select other at the cursor | keyboard_shortcuts; Home__select_other | 🔧 |
| 1.19 | ` again moves down the Select other list, Shift+` up, Enter picks | Home__select_other | 🔧 |
| 1.20 | S opens the shortcut toolbar at the cursor (feature tools; sketch tools in a sketch) | keyboard_shortcuts; feature_basics | 🔧 |
| 1.21 | S toolbar contents customizable in preferences | part_studios | ❌ |
| 1.22 | [ opens the Measure tool | keyboard_shortcuts | 🔧 |
| 1.23 | Ctrl+M opens the Mate connector dialog | keyboard_shortcuts | 🔧 |
| 1.24 | K shows/hides mate connectors | keyboard_shortcuts | 🔧 (toggles the Mate connector features' visibility) |
| 1.25 | A flips the primary axis while editing a mate connector | keyboard_shortcuts | 🔧 (dialog focused) |
| 1.26 | Shift+C curvature map, Shift+D draft analysis (per-vertex color maps with legend; draft angle and pull direction adjustable) | keyboard_shortcuts | 🔧 (Onshape's Shift+D is dihedral analysis; draft per coordinator; dihedral ❌) |
| 1.27 | Alt/⌥+T tab manager (search, open), Ctrl+Space switches to the previously active tab | keyboard_shortcuts | 🔧 |
| 1.28 | Ctrl+U feedback, Ctrl+Shift+F FeatureScript search | keyboard_shortcuts | ❌ (out of scope) |
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
| 2.20 | Shift+X section view | keyboard_shortcuts | 🔧 (up to 3 planes) |
| 2.21 | Shift+V named views | keyboard_shortcuts | ✅ |
| 2.22 | Shift+R view in high quality (4x finer tessellation, notice) | keyboard_shortcuts | 🔧 |
| 2.23 | Alt+click selects through transparent geometry | keyboard_shortcuts | ❌ |
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
| 3.9 | Cursor badge with the selection count (up to 5+) | Home__selection | 🔧 (re-render throttled to ~30 Hz) |
| 3.10 | Midpoints of edges/sketch curves show on hover and can be picked | Home__selection | ❌ |
| 3.11 | Select other lists faces and edges under the cursor nearest first; hovering a row highlights it; click/Enter picks; list closes | Home__select_other | 🔧 |
| 3.12 | Select other picks go to the active dialog field | Home__select_other | 🔧 |
| 3.13 | Select other from the face/edge context menu | Home__select_other | 🔧 |
| 3.14 | Selecting a face or part highlights the features that created/modified it in the feature list | features_and_parts_lists | 🔧 |
| 3.15 | Hovering a feature in the list highlights its geometry | features_and_parts_lists ("Hover over a feature…") | 🔧 |
| 3.16 | Selecting a feature in the list highlights its geometry | A §5.3 (FEATURE_HIGHLIGHT) | 🔧 |
| 3.17 | Selecting / hovering a part in the parts list highlights the whole part | A §5.3 (PART_HIGHLIGHT) | 🔧 |
| 3.18 | Double-click a sketch in the graphics area edits it | (Onshape behaviour) | 🔧 |
| 3.19 | Create selection panel and Select ▸ submenu (tangent faces, face loop, tangent edges, parallel edges, equal radius, part faces) | Home__create_selection | 🔧 (protrusion/pocket/hole/bounded criteria ❌) |

## 4. Preselection and tools (A §2.3, PartStudio__feature_basics)

| # | Behaviour | Source | Status |
|---|---|---|---|
| 4.1 | Entities selected before starting a feature fill its first compatible field | A §2.3 | ✅ |
| 4.2 | A sketch selected in the feature list preselects all its regions (Extrude, Revolve…) | features_and_parts_lists | 🔧 |
| 4.3 | Planes / features selected in the list preselect plane / feature fields | A §2.3 | 🔧 |
| 4.4 | Clicking the active toolbar tool again closes it | Home__selection | 🔧 |
| 4.5 | Picking a tool from a toolbar dropdown makes it the group's face for the session | A §2.3 | 🔧 |
| 4.6 | Toolbar tooltips show the shortcut | A §2.2 | ✅ |
| 4.7 | Customize toolbar (drag tool sets, Reset) | part_studios | ❌ |
| 4.8 | Search tools: typing filters tools, Enter runs the first, Esc closes | Home__search_tools | ✅ |
| 4.9 | Search tools with no query lists recently used tools first; description on hover | Home__search_tools | 🔧 |
| 4.10 | REMEMBER_PREVIOUS_VALUE parameters default to the last accepted value of that tool | A §7.2 | 🔧 |

## 5. Feature dialogs and selection fields (Home__dialogs, feature_basics, Home__numeric_fields, A §7)

| # | Behaviour | Source | Status |
|---|---|---|---|
| 5.1 | Green check accepts, red X cancels | feature_basics | ✅ |
| 5.2 | Title red while the feature errors; accept is refused with the reason | feature_basics ("prevents you from committing a broken feature") | 🔧 |
| 5.3 | Error/warning message shown in the dialog | Home__error_indicators | ✅ |
| 5.4 | Fields appear/disappear live from their visibility conditions | A §7.1 | ✅ |
| 5.4b | Clicks and right-clicks on the dialog (and other panels over the graphics area) never reach the model beneath | (Onshape behaviour) | 🔧 (clicking a field also picked/deselected geometry under it) |
| 5.5 | First selection field active when the dialog opens | feature_basics | ✅ |
| 5.6 | Click a selection field to make it active; it stays active while a number field has keyboard focus ("two fields are active") | Home__dialogs; feature_basics | 🔧 (typing deactivated it) |
| 5.7 | Picks go to the active field; clicking a picked entity again removes it | Home__selection; A §5.4 | ✅ |
| 5.8 | Picks that don't pass the field filter are rejected: the field flashes red and a message names what it takes | A §5.4 | 🔧 (generic message before) |
| 5.8b | Clicking a sketch curve in the graphics area while a region field is active takes all the sketch's regions | features_and_parts_lists | 🔧 |
| 5.9 | List clicks (default planes, parts) obey the field filter | A §5.4 | 🔧 (were added unfiltered) |
| 5.10 | Clicking a sketch in the feature list fills a region field with all its regions; feature fields take the feature | features_and_parts_lists | ✅ |
| 5.11 | Single-pick fields replace their pick and move focus to the next empty selection field | A §5.4 | ✅ |
| 5.12 | × on a chip removes that pick | feature_basics | ✅ |
| 5.13 | Right-click a selection field > Clear all; right-click a pick > Remove / Clear all | feature_basics | 🔧 |
| 5.14 | Hovering a chip highlights its entity in the graphics area | Home__dialogs | 🔧 |
| 5.15 | Missing references shown red; Clear missing references | feature_basics | 🔧 (missing = creating feature deleted or suppressed) |
| 5.16 | Reorder picks in order-sensitive fields (drag a row, or Move up/down) | feature_basics | 🔧 |
| 5.17 | Tab moves between fields (inputs and checkboxes) | feature_basics | ✅ |
| 5.18 | Space toggles a focused checkbox; arrows/Enter in dropdowns | feature_basics | ⏳ (Space 🔧: it cleared the selection instead; dropdown keys ❌) |
| 5.19 | Numeric fields accept expressions, units and #variables | Home__numeric_fields | ✅ |
| 5.20 | Invalid expression: red outline and message | Home__numeric_fields | ✅ |
| 5.21 | Expressions show their evaluated value | Home__numeric_fields | 🔧 |
| 5.22 | Mouse wheel over a focused numeric field: ±0.1, Ctrl ±0.01, Shift ±1 (integers ±1), clamped to the field range | Home__numeric_fields | 🔧 |
| 5.23 | Field shows the converted value after accepting, the original text when focused | Home__numeric_fields | 🔧 |
| 5.24 | Final button previews the end of the history; absent for new features and the last feature | Home__dialogs | 🔧 (was always shown) |
| 5.25 | Preview opacity slider | Home__dialogs | ❌ |
| 5.26 | Rename the feature from the dialog title | Home__dialogs | ✅ (double-click the title) |
| 5.27 | Dialog movable and resizable; help (?) icon | Home__dialogs | ❌ |
| 5.28 | Boolean "Convert to expression" / "Convert to checkbox" (evaluated at regeneration with upstream variables) | feature_basics | 🔧 |
| 5.29 | Tolerance options on tolerant quantities | Home__tolerance_options | ❌ |
| 5.30 | Extrude/Revolve from a part face (or a sketch on one) defaults to Add with that part as the merge scope | (Onshape behaviour, A §7) | 🔧 |
| 5.31 | Choosing Remove/Intersect on such a feature points it into the part | (Onshape behaviour) | 🔧 |
| 5.32 | Drag handles for depth/angle (extrude arrow, revolve arc) | A §7.7 | ⏳ (extrude, revolve) |
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
| 6.11 | Filter commands :errors :warnings :suppressed :hidden :shown :type :name :folder :variable :part | features_and_parts_lists | 🔧 (:partnumber/:revision ❌) |
| 6.12 | Error icon with count in the header; click filters to errors | features_and_parts_lists | 🔧 |
| 6.13 | Failed features red with the error as tooltip | Home__error_indicators | ✅ |
| 6.14 | Suppressed features struck through | features_and_parts_lists | ✅ |
| 6.15 | Under-defined sketch indicator | features_and_parts_lists | ✅ (blue text) |
| 6.16 | Missing-selection / warning yellow triangle | features_and_parts_lists | 🔧 |
| 6.17 | Eye icon on hover hides/shows sketches, planes, mate connectors | features_and_parts_lists | 🔧 |
| 6.18 | Rename in place (context menu / Shift+N) | features_and_parts_lists | ✅ |
| 6.19 | Context menu: Edit, Rename, Roll to here, Suppress/Unsuppress, Hide/Show, Show dependencies, Copy, Paste, Add selection to folder, Zoom to selection, View normal to (sketches/planes), Clear selection, Delete | context_menus; A §3.10 | 🔧 (Hide/Show, dependencies, zoom, view normal, clear added) |
| 6.20 | Context menu commands act on all selected features | features_and_parts_lists | 🔧 |
| 6.21 | Show dependencies: parents above ↑, children below ↓; hover highlights | features_and_parts_lists | 🔧 |
| 6.22 | Configure suppression; Dynamic suppression (expression editor, ƒx icon, live true/false preview) | features_and_parts_lists | 🔧 |
| 6.23 | Copy sketch / Paste sketch onto a plane or planar face | Home__context_menus | 🔧 |
| 6.24 | Show dimensions, Export as DXF/DWG, Create drawing of sketch | Home__context_menus | ⏳ (Export DXF/SVG 🔧; show dimensions, drawing ❌) |
| 6.25 | Pause regeneration (banner: Regenerate / Regenerate and exit / Discard) ; regeneration times panel (hover highlights) | features_and_parts_lists | 🔧 |
| 6.26 | Collapse the whole list; drag the feature/parts separator | features_and_parts_lists | 🔧 |
| 6.27 | Default geometry context menu: New sketch, Offset plane, Hide/Show, View normal to | context_menus; PartStudio__plane | 🔧 |
| 6.28 | Default planes/origin can be hidden (never deleted) | part_studios | 🔧 |
| 6.29 | Clicking empty list space clears the feature selection | (Onshape behaviour) | 🔧 |

### Rollback bar

| # | Behaviour | Source | Status |
|---|---|---|---|
| 6.30 | Drag the bar to roll back/forward; model shows up to the bar | features_and_parts_lists | ✅ |
| 6.31 | Right-click a feature > Roll to here | features_and_parts_lists | ✅ |
| 6.32 | Right-click the bar > Roll to end | features_and_parts_lists | 🔧 |
| 6.33 | Select the bar, ↑/↓ step one feature | features_and_parts_lists | 🔧 |
| 6.34 | Features below the bar greyed; new features inserted at the bar | A §3.2 | ✅ |

### Folders

| # | Behaviour | Source | Status |
|---|---|---|---|
| 6.35 | Add selection to folder | features_and_parts_lists | ✅ |
| 6.36 | Expand/collapse a folder | features_and_parts_lists | ✅ |
| 6.37 | Folder menu: Rename, Suppress/Unsuppress (all contents), Unpack folder | features_and_parts_lists | 🔧 (Suppress added, "Remove" renamed) |
| 6.38 | Nested folders; Expand all / Collapse all | features_and_parts_lists | ⏳ (Expand/Collapse all 🔧; no nesting) |

## 7. Parts list (features_and_parts_lists §Parts list, A §3.9)

| # | Behaviour | Source | Status |
|---|---|---|---|
| 7.1 | Parts with colour swatch and count | features_and_parts_lists | ✅ |
| 7.2 | Click selects (or fills a part field in a dialog, filtered) | features_and_parts_lists | 🔧 (unfiltered before) |
| 7.3 | Hover highlights the part in the graphics area | features_and_parts_lists | 🔧 |
| 7.4 | Eye on hover hides/shows the part; hidden parts greyed | features_and_parts_lists | 🔧 |
| 7.5 | Context menu: Rename, Properties, Assign material, Edit appearance, Hide/Show, Hide other parts, Hide all parts, Isolate, Make transparent, Zoom to selection, Mass properties, Delete | features_and_parts_lists | 🔧 (no menu before) |
| 7.6 | Separate Parts / Surfaces / Curves sections (collapsible, eye hides a section) | features_and_parts_lists | 🔧 (no Composite parts: not modelled) |
| 7.7 | Show parts list properties (part number, revision) from the section header menu | features_and_parts_lists | 🔧 |

## 8. Graphics area context menus (Home__context_menus, A §4)

| # | Behaviour | Source | Status |
|---|---|---|---|
| 8.1 | Right-click without drag opens the menu; right-drag rotates | A §4 | ✅ |
| 8.2 | Empty space: Show all, Show all parts, Zoom to fit, Isometric, Clear selection | Home__context_menus | 🔧 (Show all, Clear selection added) |
| 8.3 | Face: Select other, New sketch, View normal to, Edit <feature>, Measure, Zoom to selection | A §4 | 🔧 |
| 8.4 | Edge: Select other, Fillet, Chamfer, Edit <feature> | A §4 | 🔧 |
| 8.5 | Plane: New sketch, Offset plane, View normal to, Hide | Home__context_menus | 🔧 |
| 8.6 | Sketch entity/region: Edit sketch, Extrude, Revolve, View normal to, Hide | A §4 | 🔧 |
| 8.7 | Part section: Part properties, Hide <part>, Hide other parts, Isolate, Make transparent, Mass properties | A §3.9 | 🔧 |
| 8.8 | Exit isolate / Exit make transparent / Turn off section view when active | View__isolate; View__section_view | 🔧 |
| 8.9 | Creation commands carry their tool icon | Home__context_menus | 🔧 |
| 8.10 | Exit <tool> entry while a tool is active | Home__selection | 🔧 |
| 8.11 | Create selection / Select ▸ / Add appearance / Section view / Paste sketch entries | A §4 | 🔧 |

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
| 9.8 | Camera and render options menu under the cube: Isometric/Dimetric/Trimetric, Named views, Previous view, Zoom to fit/window/selection, Perspective, display modes, Section view | View__camera_and_render_options | 🔧 |
| 9.9 | Previous view returns to the camera before the last view change | View__previous_view | 🔧 |
| 9.10 | Zoom to selection (parts, faces, edges, sketches, planes, features) | View__zoom_to_fit | 🔧 |
| 9.11 | Display modes: Shaded with edges / Shaded / Hidden line / Wireframe / Translucent | View__shaded_unshaded | ⏳ (Translucent and Hidden edges visible 🔧; tangent-edge options ❌) |
| 9.12 | Orient normal to sketch on edit (camera menu toggle); the previous view returns when the sketch closes | View__orient_normal_to_sketch_on_edit | 🔧 (not persisted across sessions) |
| 9.13 | Named views store orientation/zoom/projection; save, restore, delete | View__named_views | ✅ |
| 9.14 | Named views store the section state | View__named_views | 🔧 |
| 9.15 | Rotation inertia; rotate about the point under the cursor | A §6.1 | ❌ |

## 10. Visibility, isolate, transparency, section (View__hide_show, View__isolate, View__make_transparent, View__section_view)

| # | Behaviour | Source | Status |
|---|---|---|---|
| 10.1 | Hide/Show parts, sketches, planes, mate connectors, default geometry | View__hide_show | 🔧 (parts only before) |
| 10.2 | Hide other parts / Hide all parts / Show all / Show all parts | View__hide_show | 🔧 |
| 10.3 | Isolate keeps chosen parts opaque, others transparent | View__isolate | 🔧 |
| 10.4 | Make transparent makes chosen parts transparent | View__make_transparent | 🔧 |
| 10.5 | Isolate/transparent panel with transparency control and × to exit; Esc exits | View__isolate | 🔧 |
| 10.6 | Select transparent geometry checkbox; Expand by distance | View__isolate | ❌ (transparent parts are skipped by Select other) |
| 10.7 | Section view by plane with offset | View__section_view | 🔧 (offset, flip, remove per plane) |
| 10.8 | Section by faces/mate connectors, several planes, manipulator, caps | View__section_view | ⏳ (faces/MCs/up to 3 planes 🔧; drag manipulator and caps ❌) |

## 11. Measure, mass properties, appearance (View__measure_tool, View__mass_properties_tool, PartStudio__customizing_appearances)

| # | Behaviour | Source | Status |
|---|---|---|---|
| 11.1 | Measure readout for the selection (length, area, radius, distance, angle, ΔX/ΔY/ΔZ) | View__measure_tool | ✅ |
| 11.2 | Measure/Mass properties buttons at the bottom right; [ opens Measure with a prompt when nothing is selected | A §1, §6.10 | 🔧 |
| 11.3 | Measure graphics (dotted min distance, axes) and measure type selector | View__measure_tool | ❌ |
| 11.4 | Mass properties: volume, surface area, mass (with material), center of mass | View__mass_properties_tool | ✅ |
| 11.5 | Moments of inertia about the center of mass; Face tab, reference mate connector, override mass | View__mass_properties_tool | ⏳ (inertia 🔧; the rest ❌) |
| 11.6 | Part appearance colour, transparency, tessellation quality (Appearance editor) | customizing_appearances | 🔧 (material still in Part properties) |
| 11.7 | Appearance panel, face/feature/sketch appearances | customizing_appearances | 🔧 (no custom-color tiles / mixer tab) |

## 12. Variables and configurations (PartStudio__variable_table, PartStudio__configurations)

| # | Behaviour | Source | Status |
|---|---|---|---|
| 12.1 | Variable table panel on the right | variable_table | ✅ |
| 12.2 | Configuration panel and configuration selectors | configurations | ✅ |
| 12.3 | `#variable` in any numeric field; :variable filter finds users | variable_table; features_and_parts_lists | 🔧 (filter) |
| 12.4 | Hover a variable highlights the features that use it; right-click > Filter feature list | variable_table | 🔧 |

## Not applicable to a local app

Collaboration (follow mode, comments), release management, App Store,
permissions/view-only toolbar, browser-specific shortcuts (Ctrl+click to open
in a new tab), mobile gestures.
