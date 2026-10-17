# A. Part Studio environment & general UI

Sources: [DOCS] = Onshape Help (Oct 2026), [FS] = FeatureScript std v3095.0, [BUNDLE] = web-client JS bundle (action IDs, appearance constants, i18n strings). Nothing here was observed live. Items marked *[inferred]* come from reading between sources (e.g. ordering/adjacency) and are not stated outright anywhere; treat them as best guesses.

---

## 1. Overall Part Studio layout

Screen regions [DOCS: part_studios, user_interface_basics, document_toolbar, variable_table, model_evaluation_tools]:

| Region | Position | Contents |
|---|---|---|
| Document toolbar | Top row, level with the Onshape logo | Logo (goes to the Documents page), **Document menu** (hamburger: Workspace units, Print, ...), document name (bold, click to rename), workspace name (light gray), Copy link, folder link, FeatureScript notices, Notifications, App Store, Learning Center, Share, Help menu (?), user account menu. Public docs also show Public, Copies, Links, Likes. |
| Feature toolbar | Second row, full width | Undo/Redo, then feature tools (sec. 2), custom features, **Search tools** at the far right. Swapped for the Sketch toolbar while a sketch is being edited. |
| Document panel | Thin vertical strip on the far left, under the logo | Versions and history, Comments, Document notes, Where used, Performance, AI Advisor, Explore Onshape, Action items, Tab manager (mostly out of scope; Performance is in sec. 10). |
| Feature list + Parts list | Left pane (collapsible) | Sec. 3. |
| Graphics area | Center | Model, default planes and origin, **View cube** in the upper right with the *Camera and render options* dropdown under it. |
| Right side-panel icon strip | Right edge of the graphics area, below the View cube | Appearance panel, Configuration panel, Custom tables, Inspection table, Sheet metal table and flat view (only when sheet metal is present), Variable table; a Hole table icon shows up once a Hole feature exists [DOCS]. |
| Model evaluation tools | Bottom-right corner | **Measure** (`[`), **Analysis tools** (menu), **Mass properties** (scale icon). |
| Refinement indicators | Bottom-left corner of the graphics area | Small icons while tessellation refines or the camera moves; they disappear when done [DOCS: graphics_area_display_data]. |
| Tab bar | Bottom of the window | Element tabs (draggable, each with a right-click menu), **Tab manager** icon, **+ Insert new element** (Part Studio, Assembly, Drawing, Feature Studio, folder, import, Paste tab...). |

- When the window is narrow, the Sketch tool shrinks to icon-only [DOCS].
- The Part Studio is a container (one Feature list drives many parts); it is "not a model" [DOCS].
- Default geometry: **Origin, Top, Front, Right** planes. They can be hidden but never deleted. To resize a plane, select it, then drag its handles (RESIZE_MANIPULATOR_RADIUS=10 px [BUNDLE]). Plane right-click menu: *Size to Part Studio* [DOCS: plane].
- Theme: Light/Dark [DOCS]. Light-theme background #ffffff.

### 1.1 Part Studio tab context menu (right-click a tab) [DOCS]
Delete (the last remaining tab can't be deleted) · Open in new browser tab · Rename · Properties · Show code (FeatureScript panel) · Duplicate (an unassociated copy) · Copy to clipboard (paste into another doc via + > Paste tab) · Update linked document · Select as document thumbnail · Create drawing of Part Studio *x* · Move to document · Export · Release / Release all configurations (out of scope) · Create task (out of scope).

---

## 2. Feature toolbar

### 2.1 Fixed left area
- **Undo / Redo** (Ctrl/Cmd+Z; Ctrl+Y or Shift+Cmd+Z). Scope: per user, per tab, per session [DOCS].
  - While a dialog or sketch is open, undo/redo steps through actions inside that edit session.
  - After the dialog closes, undo/redo reverts or reinstates the whole editing session as one step.
  - Actions taken while a dialog was open are removed from the undo list once the dialog is accepted or rejected [DOCS: dialogs].

### 2.2 Default tool order
Order taken from the Icon Glossary "Part Studio" table [DOCS], which matches the `CREATE_FEATURE_*` order in the bundle [BUNDLE]. The only bundle difference: Fillet appears right after Extrude there, which looks like shortcut-registration order (Shift+E, Shift+F).

**Grouping.** The UI basics page says vertical divider lines separate tool groups, and some tools carry a ▾ arrow that opens a dropdown of related tools. Only three dropdowns are confirmed in the docs:
- Thicken▾ contains Enclose.
- The Frame group has a dropdown.
- There is a "Custom features" dropdown.

Every other grouping below is *[inferred]* from glossary adjacency. Make grouping data-driven.

| # | Tool (tooltip name) | Bundle action ID | Default shortcut | Group / dropdown | Tooltip text [DOCS glossary, condensed] |
|---|---|---|---|---|---|
| 0 | **Sketch** | createFeatureSketch | Shift+S | own group | Start a sketch on a plane or planar face ("Select a sketch plane"). |
| 1 | Extrude | createFeatureExtrude | Shift+E | Solid group | Create/add/subtract/intersect parts, surfaces or thin parts by extruding sketch regions, planar faces, lines or curves. |
| 2 | Revolve | revolve | Shift+W | Solid group | Same verbs; revolve about an axis. |
| 3 | Sweep | createFeatureSweep | – | Solid group | Sweep along a path; optional profile control, twist, scale. |
| 4 | Loft | createFeatureLoft | – | Solid group | Smooth face between ordered profiles. |
| 5 | Thicken | createFeatureThicken | – | **Thicken▾** [DOCS] | Thicken surfaces or faces of solids. |
| 6 | Enclose | createFeatureEnclose | – | under Thicken▾ [DOCS] | Create parts from faces/surfaces/parts that enclose a region. |
| 7 | Fillet | createFeatureFillet | Shift+F | Fillet▾ *[inferred]* | Constant, variable, partial, asymmetric, or full round; radius/width; distance/conic/curvature. |
| 8 | Face blend | createFeatureFaceBlend | – | Fillet▾ *[inferred]* | Blend 2+ faces; propagation, cross section, symmetry, control, trim. |
| 9 | Chamfer | createFeatureChamfer | – | – | Offset/tangent; equal, two distances, distance-angle. |
| 10 | Draft | createFeatureDraft | – | Draft▾ *[inferred]* | Draft faces by neutral plane or parting line. |
| 11 | Body draft | createFeatureBodyDraft | – | Draft▾ *[inferred]* | Draft a part by edges, faces, or the part. |
| 12 | Rib | createFeatureRib | – | – | Ribs from sketch curves; thickness; normal/parallel to sketch plane. |
| 13 | Shell | createFeatureShell | – | – | Thin-walled part by removing faces. |
| 14 | Hole | createFeatureHole | – | Hole▾ *[inferred]* | Simple/countersink/counterbore holes at sketch points or mate connectors. |
| 15 | External thread | createFeatureExternalThread | – | Hole▾ *[inferred]* | Thread on cylindrical faces from leading edges. |
| 16 | Linear pattern | createFeatureLinearPattern | – | **Pattern▾** *[inferred]* | Pattern parts/faces/features along a direction. |
| 17 | Circular pattern | createFeatureCircularPattern | – | Pattern▾ | About an axis. |
| 18 | Curve pattern | createFeatureCurvePattern | – | Pattern▾ | Along a path. |
| 19 | Mirror | createFeatureMirror | – | – | Mirror about a plane or planar face. |
| 20 | Boolean | createFeatureBoolean | – | – | Union, subtract, intersect parts or surfaces. |
| 21 | Split | createFeatureSplit | – | – | Split parts, surfaces, or faces. |
| 22 | Transform | createFeatureTransform | – | – | Move, rotate, copy, scale parts. |
| 23 | Wrap | createFeatureWrap | – | – | Wrap faces or sketch entities onto a cylinder or cone. |
| 24 | Decal | createFeatureDecal | – | – | Apply an image onto a flat or cylindrical face. |
| 25 | Delete part | createFeatureDeletePart | – | – | Delete parts or surfaces; composite handling. |
| 26 | Modify fillet | createFeatureModifyFillet | – | Direct-edit▾ *[inferred]* | Remove or resize fillets (imported parts). |
| 27 | Delete face | createFeatureDeleteFace | – | Direct-edit▾ | Heal, cap, or leave open. |
| 28 | Move face | createFeatureMoveFace | – | Direct-edit▾ | Translate, rotate, offset faces. |
| 29 | Replace face | createFeatureReplaceFace | – | Direct-edit▾ | Trim or extend a face to a surface. |
| 30 | Plane | createFeaturePlane | – | – | Construction plane. |
| 31 | Offset surface | createFeatureOffsetSurface | – | **Surface▾** *[inferred]* | Offset a face/surface/region; distance 0 = copy. |
| 32 | Boundary surface | createFeatureBoundarySurface | – | Surface▾ | Surface from boundary profiles. |
| 33 | Fill | createFeatureFill | – | Surface▾ | Surface from a closed boundary of edges/curves with continuity. |
| 34 | Move boundary | createFeatureMoveBoundary | – | Surface▾ | Extend or trim surface boundary edges. |
| 35 | Ruled surface | createFeatureRuledSurface | – | Surface▾ | Ruled surface from an edge, line or curve. |
| 36 | Mutual trim | createFeatureMutualTrim | – | Surface▾ | Trim two surfaces against each other. |
| 37 | Constrained surface | createFeatureConstrainedSurface | – | Surface▾ | Fit a surface to points or mesh. |
| 38 | Helix | createFeatureHelix | – | **Curve▾** *[inferred]* | Helix from a cone/cylinder, axis, or circular edge. |
| 39 | 3D fit spline | createFeatureThreeDFitSpline | – | Curve▾ | Spline through vertices. |
| 40 | Projected curve | createFeatureProjectedCurve | – | Curve▾ | Project a curve onto faces, or intersect 2 sketches. |
| 41 | Bridging curve | createFeatureBridgingCurve | – | Curve▾ | Curve between 2 points with continuity. |
| 42 | Composite curve | createFeatureCompositeCurve | – | Curve▾ | Join connected edges/curves. |
| 43 | Intersection curve | createFeatureIntersectionCurve | – | Curve▾ | Curve at a face intersection. |
| 44 | Trim curve | createFeatureTrimCurve | – | Curve▾ | Trim or extend curves. |
| 45 | Isocline | createFeatureIsocline | – | Curve▾ | Isocline curves or split lines. |
| 46 | Offset curve | createFeatureOffsetCurve | – | Curve▾ | Offset edges on faces. |
| 47 | Isoparametric curve | createFeatureIsoparametricCurve | – | Curve▾ | U/V curves on a surface. |
| 48 | Edit curve | createFeatureEditCurve | – | Curve▾ | Approximate, change degree, move CVs, planarize. |
| 49 | Routing curve | createFeatureRoutingCurve | – | Curve▾ | Spline or polyline through points/MCs/CSV. |
| 50 | Mate connector | createFeatureMateConnector | Ctrl+M (general) | – | Mate connector on or between parts. |
| 51 | Derived | createFeatureDerived | – | – | Insert entities from another Part Studio (associative). |
| 52 | Variable | createFeatureVariable | – | Variable▾ *[inferred]* | Create a #variable. |
| 53 | Query variable | createFeatureQueryVariable | – | Variable▾ | Named query for later features. |
| 54 | Composite part | createFeatureCompositePart | – | – | Open or closed composite of bodies. |
| 55 | Tag | createFeatureTagProfile | – | Frame group *[inferred]* | Tag entities for frames, forms, PCB holes. |
| 56 | Frame | createFeatureFrame | – | **Frame▾** [DOCS] | Sweep a profile along paths. |
| 57 | Frame trim | createFeatureFrameTrim | – | Frame▾ | – |
| 58 | Gusset | createFeatureGusset | – | Frame▾ | – |
| 59 | End cap | createFeatureEndCap | – | Frame▾ | – |
| 60 | Cut list | createFeatureCutList | – | Frame▾ | – |
| 61 | Sheet metal model | createFeatureSheetMetalModelId | – | **Sheet metal▾** *[inferred]* | Convert, extrude, or thicken into sheet metal. |
| 62 | Flange | createFeatureFlange | – | Sheet metal | – |
| 63 | Hem | createFeatureHem | – | Sheet metal | – |
| 64 | Tab | createFeatureTab | – | Sheet metal | – |
| 65 | Bend | createFeatureBend | – | Sheet metal | – |
| 66 | Jog | createFeatureSheetMetalJog | – | Sheet metal [BUNDLE only] | – |
| 67 | Form | createFeatureFormed | – | Sheet metal | Library form tools. |
| 68 | Sheet metal loft | createFeatureTessellatedLoft | – | Sheet metal | – |
| 69 | Make joint | createFeatureMakeJoint | – | Sheet metal | – |
| 70 | Corner | createFeatureCorner | – | Sheet metal | – |
| 71 | Bend relief | createFeatureBendRelief | – | Sheet metal | – |
| 72 | Corner break | createFeatureCornerBreak | – | Sheet metal | – |
| 73 | Finish sheet metal model | createFeatureFinishSheetMetalModel | – | Sheet metal | – |
| 74 | Flex PCB model / Finish flex PCB model | createFeatureFlexPcbModel / ...Finish... | – | PCB (OUT OF SCOPE) | – |
| 75 | **Add custom features** (+ icon) | addCustomFeature | – | right end | Opens the custom-feature browser (sec. 2.6). |
| 76 | Custom features ▾ (current-workspace FS features) / "Linked custom features in this Part Studio" ▾ | – | – | right end | Shown only when relevant [DOCS]. |
| 77 | **Search tools** | openCommandSearch | **Alt/Opt+C** | far right | Sec. 2.5. |

Other Part Studio commands that are not on the toolbar [BUNDLE action IDs]: hideShowSketches (Shift+H), hideShowPlanes (P), hideAllConstruction (Shift+P), hideShowMateConnector (K), expandAll / collapseAll (feature folders), copyOccurrences / pasteOccurrences (Ctrl+C / Ctrl+V on features), renameSelection (Shift+N), deleteSelection (Del/Backspace), clearSelection (Space), selectOther (`), openMiniToolbar (S), openElementTabSwitcher (Ctrl+Space), openTabManager (Alt+T), openKeyboardShortcuts (Shift+/).

### 2.3 Toolbar behaviors [DOCS: user_interface_basics]
- **Last-used promotion:** picking a tool from inside a dropdown moves it to the top (visible face) of that group. Example: after using Enclose, it shows in place of Thicken. Reopening the document resets groups to their default faces.
- **Responsive collapse:** at low resolution or high browser zoom, more tools fold into dropdown groups.
- Clicking a tool toggles it: click again, Esc, or context menu *Exit <tool name>* deselects it [DOCS: selection].
- Tools work either way round: pick the tool and then the geometry, or preselect geometry and then pick the tool. Preselections fill the dialog's first selection field [DOCS: feature_tools].
- Users without edit permission get a **View only toolbar** [BUNDLE i18n "Previewing view only toolbar."].

### 2.4 Customize toolbar [DOCS: part_studios]
- Right-click anywhere on the toolbar to get **Customize toolbar...** and **Hide "Search tools"** / **Show "Search tools"**. Enterprise accounts also see *Show / Hide / Restore enterprise toolset* [BUNDLE].
- Edit mode:
  - Tools are highlighted in their tool sets.
  - Drag a whole tool set to a new spot.
  - Drag a tool onto the **New toolset** box (it appears once a drag starts) to create a set.
  - Drag a tool to the **Tools** box to remove it. It can be dragged back later.
- Buttons: **Save**, **Cancel**, **Reset to default**. Reset restores the original order and content for that toolbar type only (Part Studio, Assembly, or Feature Studio).
- Grouped tools move as a whole group. Once a group is removed, its tools can be re-added one at a time. Only Reset re-forms the group.
- Messages: "Toolbars reset." / "Toolbars failed to reset." [BUNDLE].

### 2.5 Search tools (command search) [DOCS: search_tools]
- Open with the toolbar button at the far right or **Alt/Opt+C**; the cursor lands in the search field. Works in Part Studio, Assembly, and Sketch.
- With no query, the dialog shows recently used tools.
- Typing shows suggestions on the left. Results include Onshape-vetted custom features.
- Hovering a result shows its description on the right.
- Checkbox **Highlight tool in toolbar** (bottom right, on by default): while hovering a result, its toolbar icon is highlighted.
- Tools removed from the toolbar still appear, in a **Removed from toolbar** section, and can be launched.
- Clicking a result starts the tool.
- Closing: x clears the query; the close button or Esc closes the dialog. It also closes when a toolbar tool is clicked.

### 2.6 Custom features [DOCS: add_custom_features]
- **+ (Add custom features)** opens a dialog:
  - Filters: *FeatureScript samples*, the standard document filters, and a search box (accepts a name or pasted URL).
  - Clicking a document lists its features. Pick the top-level icon to add all of them, or pick one. Clicking a selected feature again toggles it off.
  - A version-graph icon picks the version; the latest is the default.
  - X closes the dialog.
- Toolbar toasts: "Custom feature *name* was added to your toolbar (from version *v*)." [BUNDLE].
- Features defined in the current workspace go automatically into a **Custom features ▾** dropdown and update when the Feature Studio is committed.
- Right-click a custom-feature toolbar icon: **Remove** · **Open linked document**.
- If the Feature list uses a custom feature that is not on your toolbar, a **Linked custom features in this Part Studio** icon/dropdown appears so it stays usable.
- Feature list indicators:
  - Linked custom features get a link icon.
  - The link highlights blue when a newer version exists, and the same icon shows on the tab.
  - An out-of-sync icon appears when the feature's version differs from the toolbar version.
  - Click the icon, or right-click > **Update linked document**, to open the Reference manager (Update all / Selective update).
- Custom icons and description images come from SVG blob tabs.

### 2.7 Shortcut toolbar
- **S** with the cursor in the graphics area and no dialog open shows a mini toolbar at the cursor. It shows feature tools in a Part Studio and sketch tools in a sketch [DOCS]. Action: openMiniToolbar [BUNDLE].
- Its contents are set in Account Preferences; the defaults are not documented.

---

## 3. Feature list and Parts list [DOCS: features_and_parts_lists]

### 3.1 Left pane, top to bottom
1. **Configuration selector(s)**: one dropdown per configuration input. Present only if configurations exist.
2. Header row:
   - **Feature count**.
   - **Filter icon** (opens a filter dropdown).
   - **Search box**: "Filter by feature name or type name" [BUNDLE].
   - **Folder icon**: creates a folder from the current selection.
   - **Pause regeneration** button and **Show regeneration times** button.
   - **Error icon** (upper left, only when errors exist): the tooltip gives the count ("*n* feature(s) failed to regenerate" [BUNDLE]); clicking filters to errors.
   - **Features and parts** icon (right side): collapses or expands the whole pane.
3. **Default geometry**: Origin, Top plane, Front plane, Right plane.
4. **Features**, in creation order. Each row has:
   - the creating tool's icon;
   - the name (default "*Tool* *n*", e.g. "Extrude 1" — standard Onshape naming, not stated in the docs);
   - an eye toggle that shows on hover;
   - status icons (3.6).
5. **Rollback bar**: a thick horizontal bar.
6. **Separator bar**: drag it to resize the Feature area against the Parts area. The two areas scroll independently.
7. **Parts list**, in collapsible sections with counts [BUNDLE "Parts", "Surfaces (n)", "Curves (n)", "Composite parts (n)", "Meshes (n)"]:
   - **Parts** (solid icon), **Composite parts**, **Surfaces**, **Meshes** (surface mesh), **Curves**.
   - Each item is listed whether it was made here or derived.
   - Released parts show a release indicator when viewing a release version.

The pane can be collapsed entirely. In narrow layouts it opens and closes via a handle (mobile).

### 3.2 Rollback bar
- The graphics area shows the model **up to the bar**. Features below it are temporarily suppressed (not computed).
- Ways to move it:
  - drag it up or down;
  - right-click a feature > **Roll to here**;
  - right-click the bar > **Roll to end**;
  - select the bar and press **↑/↓** to step one feature at a time.
- New features are inserted directly above the bar.
- A folder that contains the bar is labelled "*name* (Contains rollback bar)" [BUNDLE].
- A view-only collaborator who moves the bar stops receiving live updates of others' bar moves until reload.
- Failure message: "Rollback failed. Consider opening a previous version instead." [BUNDLE].

### 3.3 Selection, cross-highlighting, reordering
- Clicking a feature or sketch name selects it. If a dialog is open, the click fills the active selection field (whole sketches, features for patterns, etc.).
- **Shift+click** selects a range; plain click toggles items in and out (non-contiguous).
- **Cross-highlight:**
  - Selecting a part, surface or curve in the Parts list highlights the features that created or modified it.
  - Selecting a face or part in the graphics area highlights the related features too.
  - Composite parts are not cross-highlighted.
- **Reorder:** drag one or more selected features to a new position. Onshape regenerates parametrically. Items can be dragged into and out of folders.
- **Double-click** a feature to open it for editing (standard behavior, implied by "Edit" in the menu).
- With View + Copy permission, dialogs can still be opened read-only.

### 3.4 Folders
- Select items, then right-click > **Add selection to folder...** (or click the folder icon). A *Folder name* prompt appears; ✓ to accept.
- Folders nest when created inside an existing folder. Limit message: "Cannot exceed *max* levels of folder nesting." [BUNDLE].
- Folder right-click menu: **Unpack folder** (removes the folder, keeps its items), **Suppress** (suppresses everything inside), Rename, plus the generic items.
- Expand/collapse:
  - right-click > **Expand** > *Expand selected* / *Expand all*;
  - right-click > **Collapse** > *Collapse selected* / *Collapse all*;
  - actions expandAll / collapseAll [BUNDLE].
- A collapsed folder rolls up the status of its contents: "Folder contains error", "Folder contains *n* missing selection(s)" [BUNDLE].
- Validation messages [BUNDLE]: "Folder name is required.", "Folder name cannot exceed *n* characters.", "Folder name cannot contain HTML."

### 3.5 Suppression
- **Suppress / Unsuppress** from the context menu. Suppressed features show **strikethrough** names and are skipped during regeneration. Dependent features become inactive or fail.
- **Dynamic suppression:**
  - right-click > *Dynamic suppression* > **Add expression** opens a dialog for a boolean expression, e.g. `#depth >= .5 in`; ✓ saves;
  - an **expression icon** appears on the row; clicking it edits the expression;
  - right-click > *Edit dynamic suppression* > **Edit expression** / **Remove expression**;
  - tooltips "*name* (Suppressed)" and "*name* (Suppressed with expression)" [BUNDLE].
- **Configure suppression** / **Unconfigure suppression** (with configurations) [BUNDLE].

### 3.6 Feature status indicators

| State | Visual | Source |
|---|---|---|
| Error (regen failed) | Name text red (#bd3039) in the list and the dialog title; error icon on the row; error count icon at the top of the list; hover gives a summary "*Feature* did not regenerate properly: *message*". (The Error-indicators page says "orange text"; the Colors page says moderate red #bd3039. Use red.) | [DOCS], [BUNDLE] |
| Missing selections (warning) | **Yellow triangle** on the row; hover shows "*n* missing selection(s)". | [DOCS], [BUNDLE] |
| Warning-level status | Feature still computes. A warning (custom message) shows as a notification. | [FS reportFeatureWarning] |
| Info-level status | Notification only. | [FS reportFeatureInfo] |
| Sketch not fully defined | **Blue indicator** on the sketch row. | [DOCS] |
| Suppressed | Strikethrough; tooltip "(Suppressed)". | [DOCS], [BUNDLE] |
| Dynamic suppression | Expression icon. | [DOCS] |
| Linked / custom / derived out of date | Link icon, blue "update available" variant, out-of-sync icon. | [DOCS] |
| Commented | Comment icon (while the Comments panel is open). | [DOCS: feature_basics] |
| Hidden | The eye icon shows the hidden state; hovering a hidden part shows its outline in the graphics area. | [DOCS] |

- FS semantics: an error on a top-level feature **rolls that feature back**. Its geometry is not applied, and downstream features run on the pre-feature state [FS error.fs].
- Faulty geometry: right-click the part > **Ignore faulty part warning for *part*** / **Show faulty part warning for *part*** (only in the originating Part Studio) [DOCS], [BUNDLE].

### 3.7 Search / filter
- Typing plain text matches feature and part names or types.
- The filter-icon dropdown offers checkbox filters equivalent to the colon commands below [BUNDLE "Filter by ..."].

| Command | Shows |
|---|---|
| `:part str` | Bodies matching the name, plus features that directly affect those bodies. |
| `:type str` | Features and bodies of a matching type. |
| `:name str` | Features and bodies with a matching name. |
| `:errors` | Features and bodies with errors. |
| `:warnings` | Features and bodies with warnings. |
| `:folder str` | Part Studio folders by name. |
| `:variable str` | Features that use or change a variable (case-sensitive). |
| `:suppressed` | Suppressed features. |
| `:hidden` / `:shown` | Hidden / non-hidden features and bodies. |
| `:partnumber str` / `:revision str` | Only when parts-list properties are shown. |

Example: `:part part 1` matches Part 1, Part 10, Part 11 ...

### 3.8 Pause regeneration and regeneration times
- **Pause regeneration:**
  - a *Paused regeneration* banner shows;
  - non-selection edits are allowed (tables, configuration tables, quantities, booleans); sketches cannot be edited;
  - banner buttons: **Regenerate Part Studio** (stay paused) · **Regenerate features and exit** · **Discard unregenerated changes and exit** · **Restore** (undoes a discard);
  - switching tabs or closing the document auto-applies the changes;
  - not available in Repair mode.
- **Show regeneration times:**
  - a panel lists each feature's regeneration time ("Server regeneration time: *t* (*level*)", "Total regeneration time" [BUNDLE]);
  - hovering a row highlights that feature's geometry;
  - the feature context menu still works inside the panel.

### 3.9 Parts list behaviors
- Hovering a section header (Parts, Composite parts, Curves, Meshes, Surfaces) shows an eye that hides or shows **all** items in that section.
- Right-click the Parts header > **Show parts list properties** / **Hide parts list properties** adds part number, revision and state next to each name. The choice persists across reloads. Default set in Preferences > Part Studio settings > *Display parts list properties*.
- **Part context menu** (Parts list, or a part in the graphics area) [DOCS + BUNDLE wording]:
  - Rename
  - Properties... (dialog in sec. 9.2)
  - **Assign material** (becomes **Edit material** once assigned; plural form "Assign material for *n* parts...")
  - **Edit appearance** ("Edit appearance for *name*...")
  - Copy
  - Create drawing
  - Export
  - Release / Release all configurations / Revision history (out of scope)
  - **Hide** / **Hide other** ("Hide other parts") / **Hide all**
  - **Show/Hide center of mass** (only once a material is assigned)
  - **Isolate**
  - **Make transparent**
  - **Section view**
  - Curve/surface analysis
  - Add comment
  - **Zoom to selection**
  - **Delete** (creates a Delete part feature *[inferred]*)
  - Ignore/Show faulty part warning (when applicable)
- Curves have their own **Edit appearance**.

### 3.10 Feature and sketch context menus
**Sketch (in Feature list)** [DOCS: context_menus]:
- Rename · **Edit**
- **Copy sketch** (paste via a plane's right-click > **Paste sketch** in another Part Studio)
- **Show dimensions** (until the next click elsewhere)
- Add selection to folder · Show/Hide
- Create Drawing of Sketch · Export as DXF/DWG
- **Edit sketch appearance**
- Suppress · Add comment · Zoom to selection · **View normal to**
- Clear selection · **Roll to here** · Delete

**Feature (in Feature list)** [DOCS various, BUNDLE]:
- Edit · Rename · Roll to here
- Suppress/Unsuppress · Dynamic suppression ▸ (Add expression | Edit expression | Remove expression) · Configure suppression (when configured)
- Hide/Show
- **Show dependencies of *name*...** — a "Dependencies of *name*" dialog: parents above an ↑ arrow, children below a ↓ arrow; hovering an arrow gives a tooltip
- Add selection to folder... · Expand ▸ / Collapse ▸ (for rows with arrows)
- **Add appearance to feature** / **Edit feature appearance**
- Copy (Ctrl+C) / Paste (Ctrl+V pastes the copied feature, repeatable; insertion at the rollback bar *[inferred]*)
- Add comment · Zoom to selection · Clear selection · Delete
- Update linked document (derived/custom) · Ignore faulty part warning

**Origin / planes:**
- New sketch · Offset plane (each with its icon) · Hide/Show · View normal to · Section view · Size to Part Studio (planes) · Paste sketch (if a sketch is on the clipboard) · Rename · Zoom to selection [DOCS context_menus, plane].
- The default planes and origin cannot be deleted.

**Rollback bar:** Roll to end.

### 3.11 Right side panels (brief)
- **Variable table:**
  - The icon sits below the View cube on the right.
  - Rows have Name (must be a valid FS identifier), Type, Value and Description. A "New variable name" row adds a variable.
  - The ellipsis menu has Copy table, Switch to Variable Studio, Open/Update linked document, Move up/down, Edit/Remove reference. The bottom dropdown has Create Variable Studio / Insert Variable Studio.
  - Cell right-click: **Filter feature list** · Delete · Move up/down.
  - Hovering a variable highlights the features that use it.
  - Override icon: when the same name exists elsewhere, the later definition wins.
  - Renaming a variable updates every feature that references it [DOCS variable_table].
- **Configuration panel:** right side. Its inputs drive the selector dropdowns at the top of the Feature list [DOCS configurations] (detail in another section).
- **Appearance panel:** sec. 9.3.

---

## 4. Context menus (graphics area) [DOCS: context_menus + scattered]

General rules:
- Right-click on an entity or empty space.
- The menu is split into vertical sections and shows only the sections relevant to the current selection or hover.
- Creation commands carry their tool icon.
- Right-click vs right-drag: a context menu needs mouse movement under 4 px (CONTEXT_MENU_DISTANCE_THRESHOLD=4 [BUNDLE]); a larger drag rotates the view.
- On mobile: two-finger tap.

| Context | Items |
|---|---|
| **Empty space** (Part Studio) | Show all (parts, sketches, planes) · Show all parts · **Create selection** · **Zoom to fit** · **Isometric**; plus Clear selection when something is selected; Exit *tool* when a tool is active; Turn off / Edit section view when one is active. |
| **Plane** (e.g. Top) | **New sketch**, **Offset plane** (icons), Hide, View normal to, Section view, Size to Part Studio, Paste sketch, Zoom to selection, Clear selection. |
| **Face** | **Select other** (`) · **Select ▸** (Tangent connected faces; Loop/chain connected edges ...) · **Create selection** · New sketch (planar face) · View normal to (planar) · **Add appearance** ("Add appearance to *n* faces...") · **Edit appearance for *part*...** · Assign material · Hide *part* / Hide other parts · Isolate · Make transparent · Section view · Curve/surface analysis · Zoom to selection · Add comment · Clear selection. |
| **Edge** | Select other · **Select ▸** Tangent connected edges, **Loop/chain connected edges**, **Parallel edges**, Equal length/radius edges (plus pattern variants) · Create selection · Measure-related items · Hide/Isolate/Transparent for the owning part · Zoom to selection · Clear selection. |
| **Vertex / point** | Select other · Zoom to selection · Clear selection · (Section view needs planes, faces or MCs). |
| **Part (body)** | Same as the Parts-list menu (3.9). |
| **Sketch / sketch entity** (not editing) | Edit sketch, Show dimensions, Hide, View normal to, Curve/surface analysis, plus the sketch menu (3.10). |
| **Inside a dialog selection field** | **Clear all** (empties the field) · **Clear missing references** · **Convert to expression** (boolean fields) / **Convert to checkbox** [DOCS feature_basics]. |
| **Triad / manipulator center** | **Move to origin**. Axis arrow: **Align with Z** / **Anti-align with Z**. Rotation arc: **Rotate 90 degrees** / **Rotate 180 degrees** [DOCS triad_manipulator]. |
| **Isolate / Transparent active** | Exit isolate / Exit make transparent. |
| **Camera menu items** | *Set as default* (sec. 6.4). |
| **Toolbar** | Customize toolbar..., Hide/Show "Search tools"; on a custom feature icon: Remove, Open linked document. |

*The Select ▸ submenu wording follows the Create-selection criteria (5.6). The exact submenu list per entity type is not fully documented.*

---

## 5. Selection

### 5.1 Basic rules [DOCS: selection]
- **Click toggles:** click to select, click again to deselect. Selection is always additive, as if Ctrl were held; no modifier is needed.
- The cursor badge shows the selected count, up to 5; beyond that it reads "5+".
- **Clear selection:** click empty space, press **Space**, or choose context menu *Clear selection*.
- **Shift+click** selects a range in lists.
- **Ctrl+drag** across entities deselects them.
- **Alt/Opt+click** selects through transparent geometry ("Select through transparent" [BUNDLE]), e.g. a part made transparent in the Appearance dialog.
- **Delete/Backspace** deletes the selected features, sketch entities or parts.
- Pick radius 5 px (MOUSE_PICK_RADIUS_PX) [BUNDLE].
- A left drag starts only after more than 2 px **and** 50 ms (LEFT_MOUSE_DRAG_DISTANCE_TOLERANCE=2, LEFT_MOUSE_DRAG_TIME_TOLERANCE=50) [BUNDLE].
- Selecting with no dialog open shows quick measurements in the bottom-right Measure readout.
- **Preselection:** entities selected before a tool is opened fill the tool's first matching field.
- **Midpoint hover:** with no sketch or feature edit open, hovering sketch entities or model edges shows their midpoints. These can be selected for Measure, Plane creation, or Use in a sketch. In an active sketch, midpoints show only for entities outside the active sketch.

### 5.2 Box selection [DOCS: selection, colors] [BUNDLE]

| Drag direction | Mode | Look |
|---|---|---|
| Left to right | **Window**: only entities fully inside the box | Solid outline #789ad1, fill #d0dcef (BOX_SELECT_COLOR, stipple(4,0,4,0) = solid, infill opacity 0.2) |
| Right to left | **Crossing**: anything the box touches | Dashed outline #ffdc74 / #FFC20E (CROSS_SELECT_COLOR, stipple(9,8,9,8)), fill #fff3cf |

- Box selection works in Part Studios and Assemblies (and in Interference detection).
- Box zoom (W) uses a dashed rgba(144,206,241,0.7) box.

### 5.3 Hover pre-highlight and selection highlight [BUNDLE appearance constants, DOCS colors]
- Each highlight has COLOR1 (face fill / interior overlay) and BORDER (edge/outline).
- Interior overlay opacity is 0.5, or 0.25 during a preview.

| Constant | Fill (COLOR1) | Border | Use |
|---|---|---|---|
| PRE_HIGHLIGHT | rgba(255,198,133,**0**) | rgba(255,198,133,1) (#ffc685, "very soft orange") | Hover: outline/edge only, no face fill |
| ENTITY_HIGHLIGHT | rgba(255,169,10,0.8) | rgba(255,169,10,1) (#ffa90a) | Selected faces, edges, vertices |
| FEATURE_HIGHLIGHT | rgba(255,169,10,0.8) | same | Geometry of a selected feature |
| PART_HIGHLIGHT | rgba(255,192,76,1) | same (#ffc04c) | Whole selected part |
| RELATED_PART_OR_OCCURRENCE | rgba(255,205,112,0.5) | 0.8 | Related bodies |
| UI_HIGHLIGHT / UI_RELATED_ENTITY | rgba(255,194,14,0.8) | rgba(255,194,14,1) (#ffc20e) | Highlight driven from UI hover (list rows, dialog fields) |
| SECONDARY_HIGHLIGHT | rgba(242,255,128,0.4) | 0.8 | Secondary/automatic selections (pale yellow) |
| SECONDARY_PRE_HIGHLIGHT | rgba(255,198,133,0) | 0.8 | – |
| SMART_SELECTION | rgba(242,255,128,0.4) | rgba(242,255,128,1) | Create-selection automatic picks |
| ACCUMULATED_SMART_SELECTION | rgba(255,194,14,1) | same | Picks already accumulated |
| ERROR_HIGHLIGHT | rgba(189,48,57,1) | same (#bd3039) | Entities in error; selecting one shows a darker red [DOCS] |
| FEATURE_ERROR_GRAPHIC_COLOR | rgba(189,48,57,0.6) | – | Error graphics of a failing feature |
| FILTERED_ENTITIES | rgba(255,29,5,0.5) | 0.8 | Entities rejected by a filter |
| COMMENT_HIGHLIGHT | rgba(66,139,202,0.5) | 0.8 | Entities tagged in comments |
| INCONTEXT_REFERENCE | rgba(212,3,249,0.3) | 0.6 | In-context references (magenta) |
| SECTION_PLANE_PRE_HIGHLIGHT | rgba(255,198,133,0.33) | – | Section-plane hover |
| REPAIR_HIGHLIGHT / REPAIR_HOVER | (255,169,10,.8) / (242,255,128,.4) | – | Repair mode |

- Docs give the selected color as "vivid yellow #ffc107" (Part Studio selection) and the sketch-edge hover as #f3bf8e.
- Highlight geometry params:
  - edges: min 5 px, opacity 0.78;
  - hidden (occluded) highlighted edges: opacity 0.18, stipple(10,10,10,10);
  - points: ×1.5, min 12 px, opacity 0.68.
- Pre-highlight throttle:
  - suppressed while the mouse moves faster than PREHIGHLIGHT_SPEED_THRESHOLD=0.15;
  - after a pause it appears within at most 250 ms (PAUSED_MOUSE_PREHIGHLIGHT_TIMEOUT_MAX_MS) [BUNDLE].

### 5.4 Dialog-driven selection filtering [FS, DOCS]
- Onshape has no global selection-filter toolbar. Each dialog **selection field** (a Query parameter) has a `"Filter"` expression, e.g. `EntityType.EDGE && BodyType.SOLID && ModifiableEntityOnly.YES`, and an optional `"MaxNumberOfPicks"` (often 1).
- While a field is active, only entities that pass its filter can be pre-highlighted or picked.
- With `MaxNumberOfPicks : 1`, a new pick replaces the old one *[inferred]*.
- Clicking a different field moves focus. Picks always go to the focused field.

### 5.5 Select other [DOCS: select_other] [BUNDLE]
- Key: the grave accent **`** (layout-specific: UK ', FR @, DE ^, ES º, IT-Mac Alt+\, IT-Win ò). Also in the context menu.
- Hover or select a face or edge, then press `. A list opens of all faces and edges under the cursor, ordered from the nearest one into the part.
- Navigation:
  - press ` again to move down the list; **Shift+`** moves up;
  - **Enter** or a click selects the highlighted entry; the list then closes.
- Highlighted entries are added to the current selection.
- Works with a feature dialog open; the result goes to the active field.
- Limits [BUNDLE]: SELECT_OTHER_PICK_RADIUS_PX=8, SELECT_OTHER_MAXIMUM_ELEMENTS=20, marker point color rgb(255,194,14).
- Also adds entities cumulatively inside Isolate / Make transparent.

### 5.6 Create selection / Select ▸ [DOCS: create_selection]
- **Select ▸ submenu** (right-click with a seed selected) adds related entities right away, without a dialog. Examples: *Parallel edges*, *Loop/chain connected edges*.
- **Create selection dialog:**
  - opened from the context menu or the arrow icon inside a feature dialog field (fields with UIHint.SHOW_CREATE_SELECTION [FS]);
  - pick seeds; Onshape propagates the selection;
  - **Add selection** moves the result into the active feature field (or into the global selection); the dialog then clears for another selection.
  - Face criteria:
    - **Protrusion**: faces connected through convex edges.
    - **Pocket**: through concave edges.
    - **Hole**: faces of the same round hole.
    - **Fillets**: constant-radius fillet faces.
    - **Tangent connected**: through tangent edges.
    - **Bounded faces**: from a seed face up to bounding faces or edges (picked second). If no closed boundary forms, all adjacent faces are taken.
    - **Select patterns** checkbox: also take the matching faces of patterned instances.
  - Edge criteria:
    - **Tangent connected**.
    - **Loop/chain connected**: picking a face takes its edge loop; picking a face and then an edge takes the loop along that face; add adjacent faces to extend the chain.
    - **Equal length/radius**.
    - **Parallel**.
    - **Select pattern** checkbox.
  - Colors: the seed shows orange, automatic picks pale yellow (SMART_SELECTION).

---

## 6. View

### 6.1 Mouse / keyboard / touch navigation (defaults) [DOCS: view_navigation] [BUNDLE]

| Action | Mouse | Keyboard | Touchpad |
|---|---|---|---|
| 3D rotate | **RMB drag**. Alt (Win) snaps with animation to the nearest "floor-down" view (no roll). Alt+RMB: horizontal move orbits, vertical move pitches. | **Arrows** = 15°, **Shift+Arrows** = 90°, **Ctrl+Arrows** = 5°; hold an arrow to rotate continuously | Right-button drag (Win); two-finger click-drag (Mac) |
| Pan | **MMB drag** or **Ctrl+RMB drag** | **Ctrl+Shift+Arrows** (DEFAULT_PAN_AMOUNT_PX=150) | Ctrl+right-drag |
| Zoom | Wheel up = in, down = out (direction configurable in Preferences); zooms about the cursor | **Z** = out, **Shift+Z** = in (factor 1.75 per step, DEFAULT_ZOOM_FACTOR) | Pinch / two-finger scroll |
| Zoom to fit | **Double-click MMB** (500 ms window) | **F** | – |
| Zoom to window | – | **W** (toggle), then drag a box | – |
| View normal to | – | **N** on a hovered or selected plane or face; press N again for the reverse normal | – |
| Standard views | View cube faces | Shift+1 Front, Shift+2 Back, Shift+3 Left, Shift+4 Right, Shift+5 Top, Shift+6 Bottom, Shift+7 Isometric | – |

Camera constants [BUNDLE]:
- ROTATE_ANGLE=15, PRECISION_ROTATE_ANGLE=5, COARSE_ROTATE_ANGLE=90. The naming is inverted relative to the docs, which call 15° "coarse".
- Snap animation 125 ms; short animation 250 ms; long animation 500 ms.
- Zoom-to-fit padding ×1.05.
- ROTATION_INERTIA=0.77 (the view keeps spinning briefly after release).
- The rotation center is depth-averaged under the cursor (ROTATION_CENTER_SEARCH_RADIUS=3).
- Touch: 1-finger drag rotates, 2-finger drag pans, pinch zooms, 2-finger rotate rolls, 2-finger tap opens the context menu, 2-finger long-press starts box select, double-tap clears the selection.

### 6.2 View cube [DOCS] [BUNDLE]
- Upper-right corner of the graphics area. Screen size 150 px; margins 15 px left/right/top and 40 px bottom.
- Front face scale 0.75, back face 0.97, corner size 0.2. Labels in the "Flama" font at 12 px.
- Shows X/Y/Z axis lines; hidden axes are dashed at 0.3 opacity; axis labels fade near edge-on.
- Interactions:
  - **Click a face** for that plane view (Top/Bottom/Front/Back/Left/Right).
  - **Click a corner** for a trimetric view [DOCS].
  - Edges also highlight (VIEW_CUBE_EDGE_HIGHLIGHT rgba(27,95,170,0.6)), so they are clickable.
  - **Arrows** around the cube: click = 15°, Shift+click = 90°, Ctrl+click = 5°. They include arc arrows for in-plane roll.
  - Hovering faces or edges highlights them; the text color changes on hover.
- The **dropdown beneath the cube** opens the *Camera and render options* menu (6.3).

### 6.3 Camera and render options menu (in documented order) [DOCS: camera_and_render_options] [BUNDLE actions]

| Item | Type | Shortcut | Notes |
|---|---|---|---|
| Isometric / Dimetric / Trimetric | command | Shift+7 (Iso) | Axonometric projections. |
| (Front/Back/Left/Right/Top/Bottom) | command | Shift+1..6 | Also on the cube faces. |
| Graphics preferences... | dialog | – | Sec. 6.6. |
| Named views... | dialog | **Shift+V** | Sec. 6.5. |
| Previous view | command | – | Reverts to the previous camera (previousView). |
| Zoom to fit | command | F | – |
| Zoom to window | toggle mode | W | – |
| Zoom to selection | command | – | Listed only when something is selected. |
| Perspective view | toggle | – | Off by default. Settable default. Field of view lives in Graphics preferences. |
| Orient normal to sketch on edit | toggle (✓) | – | Persists across sessions. Entering sketch edit orients normal to the plane; turning it off returns to the previous view. |
| Shaded with edges (default) / Shaded without edges / Unshaded / Translucent | radio | – | With Translucent, the hidden-edge options are unavailable. TRANSLUCENT_PASS_ALPHA=0.25. |
| Hidden edges removed (default) / Hidden edges visible | radio | – | Combines with Shaded or Unshaded. |
| Tangent edges visible (default) / phantom / removed | radio | – | Phantom = dashed, stipple(30,4,8,4), rgba(35,35,35), 0.5 px. |
| View in high quality | toggle | **Shift+R** | Forces the highest tessellation (Extra fine). Blue notice "Viewing in high quality. Performance can decrease." |
| Highlight boundary edges | toggle | – | Off by default, settable default. Laminar or open edges show solid red when visible and dashed red when hidden. |
| Section view / Turn off section view / Edit section view | toggle + dialog | **Shift+X** | Sec. 6.7. |

The mobile View-cube menu also lists "Shaded with hidden edges" and "Hidden edges visible/removed" as discrete entries [DOCS].

### 6.4 "Set as default" [DOCS]
- **Perspective view** and **Highlight boundary edges**: left-click to set the state, then right-click the item > *Set as default*.
- **Shaded / Hidden / Tangent edge sub-options**: right-click a disabled sub-option > *Set as default*. This enables it and stores it as the default.
- **Orient normal to sketch on edit**: just toggling it persists.

### 6.5 Named views and previous view [DOCS]
- Named views store orientation, zoom, perspective and section state, **per workspace**.
- Dialog:
  - a name field plus **+** to save;
  - a *Select a view* dropdown to restore;
  - **X** deletes the chosen view.
- **Shift+V** opens the dialog next to the cube, or where it was last moved.
- If the current camera matches a saved view, that view is preselected.

### 6.6 Graphics preferences dialog [DOCS] [BUNDLE]
Two sections: General and Sketch. **Reset to default** restores everything. Toast: "Graphics preferences saved successfully."

| Group | Setting | Range / default |
|---|---|---|
| Viewport | Ambient occlusion | 0–100 %, default 37.5 % |
| Viewport | Field of view | 20–120° (only while Perspective is on) |
| Shaded bodies | Line width | 0.1–3 px (DEFAULT_LINE_WIDTH=1) |
| Shaded bodies | Shininess | 1–100 % (DEFAULT_SHININESS=30) |
| Shaded bodies | Specular color | swatch (default rgb(153,153,153)) |
| Shaded bodies | Section interference color | swatch (default red, rgba(220,0,0)) |
| Phantom edges | Phantom tangent edge width | 0.1–3 px, default 0.5 |
| Phantom edges | Phantom tangent edge color | swatch (rgba(35,35,35)) |
| Mesh bodies | Mesh line width | 0.1–3 px, default 0.3 |
| Active sketches | Line width (default 2) · Constrained line color (#000) · Underconstrained color (#0000d0 / docs #0000ff) · Underconstrained occluded color | swatches |
| Inactive sketches | Line width (default 1) · Line color (#999999 / docs #949ba0) | – |
| Construction lines | First dash 1–50 (4) · First gap 0–50 (6) · Second dash 1–50 (30) · Second gap 0–50 (6) px | stipple(4,6,30,6) |

Swatches open the shared **Color picker**:
- Palette tab (checkmark on the current color) and Mixer tab (spectrum plus H/S/B square).
- `#` hex field and R/G/B fields (0–255).
- Optional Transparency slider (0.00–1.00).
- Custom colors with + (tile right-click: Delete / Update color).
- Optional Tessellation quality.

### 6.7 Section view [DOCS] [BUNDLE]
- Valid inputs: one or more **planes, mate connectors, planar, cylindrical or conical faces**. Pick them before or after turning the mode on.
- Ways in:
  - menu > Section view;
  - right-click a part > Section view;
  - **Shift+X**.
- Dialog:
  - **Section planes** list.
  - Tabs **Exclude** (*Items to exclude*) / **Include** (*Items to include*).
  - **Select default plane** icon: a dropdown of default planes plus a **Flip plane** icon; its own ✓.
  - ✓ closes the dialog but keeps the section active.
- Manipulator:
  - Drag the open-circle ball to move it; it snaps to inference points (e.g. cylinder centroids).
  - The arrow sets depth; clicking the manipulator flips the view side.
  - Angle handles tilt the plane.
  - A numeric field in the graphics area accepts depth or angle.
- Clicking another location adds a new section plane and manipulator. "Maximum number of section planes reached" [BUNDLE].
- Interfering solids show in the section interference color (red).
- Cut faces get end caps with a hatch-like pattern (SECTION_PLANE_PATTERN_* constants).
- Close and reopen: double-click the section plane, or use *Edit section view*.
- Exit: *Turn off section view* (menu or context menu).
- N gives a view normal to the section. Measure works on section faces. Named views capture the section state.

### 6.8 Hide / Show / Isolate / Make transparent [DOCS]
- **Hide (Y)** hides the entity under the cursor or the selection. **Shift+Y** shows it. Eye icons in the lists do the same.
- Commands come in triples: *Hide/Show*, *Hide/Show all* (all of the same type), *Hide/Show other*. BUNDLE templates: "Hide all *label*", "Hide other *label*", "Show *n* planes", "Hide *n* planes".
- **P**: hide/show all planes. **Shift+H**: hide/show sketches. **Shift+P**: hide all construction geometry. **K**: mate connectors.
- **Isolate (Shift+I)**: the selection stays opaque and everything else turns transparent.
- **Make transparent (Shift+T)**: the reverse.
- Both are cumulative when pressed on hovered entities in turn, and both open a dialog:
  - **Transparency slider**:
    - far left: outer edges only, internal geometry invisible;
    - middle: internal geometry visible;
    - far right: fully invisible.
  - **Select transparent geometry** checkbox: allows picking, measuring and editing transparent geometry. Moving the slider fully left or right auto-unchecks it; moving it again re-checks it.
  - **Component controls**: *Expand: distance* slider (Part Studio). *Expand: connectivity* exists in Assembly only.
  - Exit with **x** (restores everything; selections made inside are kept), Esc, or right-click > *Exit isolate* / *Exit make transparent*.
  - The checkbox and slider settings persist across sessions.
- BUNDLE values: ISOLATE_CONTEXT_FACE_TRANSPARENCY=0.2, EDGE=0.1, 300 ms transition.

### 6.9 Rendering details useful for a clone [BUNDLE]
- Construction planes:
  - fill alpha 0.2, outline width 2;
  - default aspect 1.7778;
  - default plane height = 0.7 × viewport height (VIEWPORT_HEIGHT_TO_PLANE_HEIGHT_RATIO);
  - label 20 px Flama in the plane corner, 5 px margin.
- Origin marker 10 px. Point bodies 8 px.
- Edges are drawn over faces using z-offsets (part faces 27, edges 15, sketch edges 12, sketch points 3 ...).
- Hidden-line stipple (show-through) is stipple(15,15,15,15). Hidden lines fade 0.85.
- Sketch region fill #9aa5af at 20 % alpha; show-through 10 %.
- Mesh facet edges 0.3 px, rgba(70,70,70).
- Display-data LOD levels: Bounding box > Coarse > Medium > Fine > Extra fine. Data size roughly ×10 per level. Parts outside the view or occluded can drop back to bounding boxes [DOCS graphics_area_display_data].
- Performance auto-reduces detail below 20 fps and enhances above 30 fps (DETAIL_REDUCTION/ENHANCEMENT_THRESHOLD_FPS).

### 6.10 Model evaluation tools (bottom-right) — brief
- **Measure** (`[`):
  - Selecting anything shows quick values; *Show measure details* expands to the full dialog.
  - **Measure type**: Show all, Position, Center position, Min distance, Max distance, Length, Diameter, Radius, Angle, Face tangent angle, Curvature deviation, Surface area.
  - **Length unit**: cm/ft/in/m/mm/yd. **Angle unit**: deg/rad.
  - Results:
    - a single entity gives length, area, diameter, radius, center, point;
    - two entities give center, min and max distances with ΔX/ΔY/ΔZ, parallel distance, perpendicular-axes angle;
    - more than two give total area and total length.
  - Graphics:
    - bold dotted lines: black for center/min/max, red X, green Y, blue Z;
    - thin dotted lines for angles;
    - ✱ marks a value inside the coincident/tangent tolerance;
    - clicking a value pins its line.
  - *Reference coordinate system* option (pick an MC).
  - Variable icon beside a value creates a measured Variable feature (Part Studio).
  - Click a value once to copy full precision; click again for workspace precision.
- **Mass properties** (scale icon):
  - Tabs **Part** / **Face**.
  - Optional MC as the inertia reference.
  - Shows mass (material-assigned parts only), volume, surface area, perimeter (faces), center of mass, moments of inertia about the centroid.
  - **Show calculation variance** checkbox.
  - **Override** a mass by selecting the Part Studio or part in the list.
  - Center of mass glyph shows while the dialog is open (persistent via *Show center of mass*).
  - Parts without material are excluded, with a note.
- **Analysis tools** menu, in this order:
  - Curve/surface analysis (Shift+C)
  - Deviation analysis
  - Connection analysis (G0–G3)
  - Dihedral analysis (Shift+D)
  - Interference detection: interference shows red; Include standard content / Show top level only (assembly)
  - Zebra stripes: Stripe count (default 35), Flip stripes, Show edges
  - Reflection analysis
  - Curvature color map: Viridis / Blue→Red / Plasma / Rainbow; Gaussian / Mean / Max radius / Min radius
  - Draft analysis
  - Thickness analysis (cloud-computed, progress stages)
  - Flatten surface
  - Opening from the menu starts with empty selections. Opening by shortcut keeps the previous selections.

---

## 7. Dialog conventions (feature dialogs) [DOCS: dialogs, feature_basics, numeric_fields] [FS]

### 7.1 Anatomy (top to bottom)
- **Title bar:**
  - feature name;
  - an edit (pencil) icon on hover renames the feature;
  - **✓ Accept** and **✗ Cancel** buttons at top right.
  - Title color: **red** while required input is missing or the feature errors (accept is blocked), **black** when valid.
- **Parameters**, in the order of the FS precondition. Conditional parameters appear and disappear as controlling values change.
- **Footer:**
  - **Preview slider** (opacity, 0 % = before the feature, 100 % = after). Hidden for features with UIHint.NO_PREVIEW_PROVIDED [FS].
  - **Final** button: only when editing a feature that is not last; shows the end-of-list result with the edit applied.
  - **Help (?) icon**, lower right: opens the help topic for that feature.
- The dialog is movable and resizable with the mouse.
- Selection lists auto-grow. Shrinking one manually turns auto-grow off; growing it again turns auto-grow back on.
- Dialog widgets for numeric edits on screen: MANIPULATOR_EDIT_LINEAR_DECIMALS=2, ANGULAR=2 [BUNDLE].

### 7.2 Field types (mapping FS parameter types to widgets)

| FS type | Widget | Visual / behavior |
|---|---|---|
| `Query` | **Selection field (QLV)** | **Solid blue / light-blue fill** (#dbf1fa) = expects graphics or list picks; a gray prompt line (e.g. "Faces and sketch regions to extrude"). Each pick is a row with an **×**. Missing (deleted) references show red; × each one or use right-click > *Clear missing references*. Right-click > *Clear all*. Optional Create-selection arrow (SHOW_CREATE_SELECTION). Reorder via the ⇅ icon when ALLOW_QUERY_ORDER. States: healthy / suppressed-before-opening / suppressed-after-opening. Error background #f8eaeb. |
| `ValueWithUnits` / `isLength` / `isAngle` / `isReal` / `isInteger` | **Numeric field** | **Outlined in blue** (keyboard input). Accepts expressions, units, `#variables` (7.3). Invalid entry = **red outline**. Optional tolerance toggle (CAN_BE_TOLERANT, SHOW_TOLERANCE). |
| `boolean` | **Checkbox**; with OPPOSITE_DIRECTION a **flip-arrow toggle** beside the previous field (OPPOSITE_DIRECTION_CIRCULAR = circular arrows, PLUS_MINUS = ±) | Space toggles a focused checkbox. Right-click > **Convert to expression** turns it into a text field taking a boolean expression (e.g. `#Length > 3`); *Convert to checkbox* reverts. |
| `enum` | **Dropdown** (arrow keys navigate, Enter picks); with HORIZONTAL_ENUM a **horizontal list of clickable text** where the selected item is **blue text with blue underline** (e.g. Solid / Surface / Thin; New / Add / Remove / Intersect) | SHOW_LABEL puts a label above. |
| `string` | Text field | – |
| array parameter | Repeating group: add button, collapsible items, ⇅ reorder unless PREVENT_ARRAY_REORDER; MATCH_LAST_ARRAY_ITEM copies the last item's values; COLLAPSE_ARRAY_ITEMS starts items collapsed | – |
| `FeatureList` | Selection field taking features from the Feature list | – |
| `LookupTablePath` | Cascading dropdowns (e.g. hole standard, size) | – |
| `PartStudioData` / `ImageData` / `CSVData` | Reference picker (opens an element/document browser) | – |

- Two consecutive DISPLAY_SHORT booleans or quantities can share a row. FIRST_IN_ROW forces a new row [FS].
- **Initial focus** when a dialog opens:
  - the first selection field becomes active (for graphics picks) and the first text field gets keyboard focus;
  - INITIAL_FOCUS / INITIAL_FOCUS_ON_EDIT override this [DOCS feature_basics] [FS].
- **REMEMBER_PREVIOUS_VALUE** (used on ~120 parameters): a new feature defaults to the value the user set last time for that tool [FS]. Session or per-user persistence is not documented.

### 7.3 Numeric expression entry [DOCS: numeric_fields]
- Accepts integers, decimals (a comma decimal separator is allowed when set in Preferences), fractions, and parentheses.
- Unit keywords:
  - length: `mm, millimeter, cm, centimeter, m, meter, in, inch, ft, foot, yd, yard`;
  - angle: `deg, degree, rad, radian`;
  - plurals allowed.
- No unit means the workspace default. The field displays the converted value in default units; clicking back into it shows the original text (or with SHOW_EXPRESSION, always the expression).
- Operators and functions:
  - `+ - * / ^ %`;
  - `ceil floor round exp sqrt abs max min log log10`;
  - trig `sin cos tan asin acos atan atan2 sinh cosh tanh asinh acosh atanh`. Trig works in **degrees** when unitless (`sin(30)` = 0.5); inverse trig returns angles (divide by `deg` for a number).
  - constants `pi/PI/Pi`;
  - arrays `[3,5,6,7][2]` = 6 (0-based; with comma decimals, put spaces after the commas);
  - ternary `#w > 5 ? 7 : 4`;
  - logic `&&`, `||`;
  - variables `#name`, plus configuration checkboxes.
- Unit rules: the result must be unitless or unit^1. Invalid examples: `3in*3in`, `3+3in`, `sin(30)/deg`, anything giving 1/unit, `3mm+2deg`, `(2*3)(1/3)` (needs `*`).
- Scroll wheel over a numeric field:
  - plain wheel ±0.1; **Ctrl** ±0.01; **Shift** ±1.0;
  - fields limited to 0..1: wheel ±1.0, Ctrl ±0.01, Shift has no effect.
- Expressions are evaluated as FeatureScript.
- Precision display follows workspace units; more digits show while editing.
- Sketch/feature dimension badges show an "expression without variable" or "expression with variable" indicator to the left of the value [DOCS glossary].

### 7.4 Keyboard in dialogs [DOCS]
- **Tab** moves to the next field; arrow keys navigate a dropdown; **Enter** picks the menu item.
- **Space** toggles a checkbox.
- **Enter** accepts and closes. **Shift+Enter** accepts, closes, and reopens the same tool with an empty dialog.
- **Esc** cancels the feature (or exits the tool).
- Switching tabs commits any open feature (mobile docs).

### 7.5 Preview behavior
- The preview is live: the model updates as inputs change.
- Editing an existing feature **rolls the model back** to that feature, hiding later features, unless **Final** is toggled on.
- Preview colors: the new geometry is drawn as a before/after blend. Fast-preview geometry is gray rgba(128,128,128) [BUNDLE FEATURE_FAST_PREVIEW_COLOR].
- When input is incomplete or the feature errors, the title turns red, offending fields get red outlines, offending selections show red in the list **and in the graphics area**, and error graphics are drawn at rgba(189,48,57,0.6).

### 7.6 Tolerance options [DOCS: tolerance_options]
- A tolerance icon sits to the right of tolerant quantities. Feature examples: Extrude depth / offset / thickness; Revolve angle / thickness; Fillet radius; Chamfer distances / angle; Hole diameters / depths / angles.
- Clicking the icon makes the value tolerant; a ▾ caret to the left opens:
  - **Precision**: Workspace precision, or 0–6 decimal places.
  - **Tolerance type**: Default, No tolerance, Symmetrical (*Deviation*), Deviation (*Upper / Lower*), Limits (*Upper / Lower*), Min, Max, Basic.
  - Diameters also offer Fit, Fit with tolerance, Fit (tolerance only), with *Standard* ANSI/ISO, *Fit type* (User defined / Clearance / Transition / Interference), *Hole class*, *Shaft class*.
- Sketch dimensions: double-click opens the Dimension context dialog with the same controls, plus ✓/✗ and a drag handle.

### 7.7 Manipulators [DOCS: triad_manipulator] [FS manipulator.fs] [BUNDLE]
- FS manipulator types: **LINEAR_1D** (arrow), **LINEAR_3D**, **ANGULAR** (arc handle), **FLIP** (direction toggle arrow), **POINTS** / **TOGGLE_POINTS** (choose among candidate points), **TRIAD_FULL** (3 axes, 3 planes, 3 rotations).
- Styles: DEFAULT, SECONDARY, SIMPLE, TANGENTIAL.
- Dragging a manipulator edits the bound dialog parameter (via the feature's Manipulator Change Function) and shows an **on-screen numeric field** for exact typing.
- Linear drags snap with resolution 100 px; angular drags snap at 50 px [BUNDLE].
- Handles fade when viewed edge-on (linear fade 10→5°, planar 75→82°, angular 80→85°) [BUNDLE].
- Colors are CSS vars with hover, drag and disabled variants. Disabled opacity 0.1. Line width 1.5.
- Sizes:
  - linear: shaft 30 px, head 30 px long, radius 10 px;
  - triad: axis shaft 60 px, plane squares 25 px offset 20 px.
- Triad (Transform, Mate connector, assembly):
  - Drag the **origin ball** to reposition the triad without moving geometry. It snaps to inferred or explicit mate connectors.
  - Arrows translate along an axis; squares translate in a plane; arcs rotate.
  - Right-click: center > *Move to origin*; arrow > *Align with Z* / *Anti-align with Z*; arc > *Rotate 90 degrees* / *Rotate 180 degrees*.

---

## 8. Error indicators and messages [DOCS: error_indicators, colors, error_messages] [FS] [BUNDLE]

| Where | Indicator |
|---|---|
| Feature list | Failing feature name shown in red/orange; hover gives a summary ("*Feature* did not regenerate properly: *message*", optionally "A parameter value is not valid."). The error count icon at the list top filters on click. Yellow triangle = missing selections. Blue badge = under-defined sketch. Folder roll-up states. |
| Dialog | Red title; invalid numeric field outlined red; bad selection rows red; list-field error background #f8eaeb. |
| Graphics area | Bad selected entities and errored part entities red (#bd3039); a darker red when an error entity is selected; feature error graphics at 60 % alpha. |
| Sketch | Under-defined blue, defined black, over-defined or error red (#bd3039 docs / #cd0000 CSS). Driving dimension black, driven dimension light gray. Constraint glyph: gray square with black icon normally; **red square with white icon** when in error. |
| Notifications / alerts | Bubble at the top of the UI. Colors [DOCS colors]: **Info** outline #b2ddf6, fill #def1fb, text #094174 · **Warning** #fcda7f / #fff8e6 / #333 · **Error** #dd979b / #f9ebec / #333 · **Success** #a2db7d / #ddf6cc / #333. Edit-mode notices (configurations, named views) use vivid yellow #ffc107. |
| Whole-studio failure | "Part Studio failed to regenerate." [BUNDLE] |
| Faulty geometry | Part flagged faulty. Right-click > *Ignore faulty part warning* puts it in a warning state; *Show faulty part warning* reverts. |
| Long regen | "*element* contains features that took a long time to regenerate." [BUNDLE] |
| Browser | HTTP 413/431: the fix is clearing browser cache (not relevant to native). |

Regeneration semantics [FS]:
- An error on a feature rolls that feature back, so the model continues without it, and downstream features evaluate.
- Warnings and info keep the geometry and show a notification.
- Errors can name specific faulty parameters (`faultyParameters`); those fields get the red outline.

---

## 9. Units, properties, materials, appearance (brief)

### 9.1 Units [DOCS: setting_default_units_per_workspace] [BUNDLE]
- The global default is **inch / degree**. Account Preferences set the default for new documents.
- **Document menu > Workspace units** sets units per workspace:
  - Length, Angle and Mass units, each with a decimal-places setting [BUNDLE labels Length/Angle/Mass, "Workspace precision"].
  - Other unit rows exist (Area, Volume, Time, Force, Pressure, Moment, Energy), mostly used by simulation.
  - ✓ to save, ✗ to cancel.
- Workspace units apply to feature dialogs, sketch dimensions, manipulator fields, Measure and Mass properties.
- Decimal places affect **display only**; computation is unaffected. Values are rounded for display; full digits show while editing. Measure falls back to scientific notation when precision is too low.
- New workspaces created from a version inherit that version's units.
- *Feet and inches* is available only in Drawings.

### 9.2 Properties (metadata) [DOCS: properties_metadata, part_studios]
- Right-click a part > **Properties**. Fields:
  - Part list name (read-only)
  - Name · Description · Category
  - Part number (has a *Generate next part number* icon) · Revision · State (In progress / Pending / Rejected / Released)
  - Vendor · Project · Product line
  - **Material** (read-only, "Unassigned" if none)
  - Title 1/2/3
  - Not revision managed · Exclude from all BOMs
  - Unit of measure (default Inch)
  - Mass (needs a material; can be overridden)
  - Custom properties at the bottom
- Buttons: **Apply**, **Save**, **Reset all** (also clears manual appearance, material and tessellation overrides), Undo/Redo at bottom-left, Create release.
- Gray fields are admin-locked. The dialog is resizable.
- Part Studio–level properties come from the tab menu.

### 9.3 Appearance [DOCS: customizing_appearances, colors]
- **Default part colors:** a fixed **8-color rotation** assigned at creation; the 9th part restarts the cycle. Deleting a part does not recolor the others. *Hex values not in sources.*
- **Edit appearance** (part, multi-part, curve), **Add appearance** (faces, features), **Edit sketch appearance**.
- Dialog: color picker (6.6) plus **Part transparency** slider 0–1 plus **Tessellation quality** (Auto default / Coarse / Medium / Fine / Very fine; Very fine is never chosen automatically).
- Face appearance dialog: a *Faces* selection field (× to remove) plus palette.
- **Appearance panel** (right side): lists every custom appearance. Double-click a row to edit. The *Add face appearance* icon opens the dialog with preselected faces filled in. Tabs: parts / faces / *Sketches and curves*.
- Appearances follow patterns. Face colors can be configured. Imported face colors are controlled by the Import feature's *Import appearances* option.
- To pick through a part made transparent here, hold **Alt**.

### 9.4 Materials [DOCS: customizing_part_materials]
- Right-click > **Assign material**. Material dialog:
  - **Library** tab: library dropdown (Onshape Material Library is the default), material dropdown with densities shown, search by name or category, **+** to add another library.
  - **Custom** tab: Name, Density, Poisson's ratio, Young's modulus, Tensile yield strength, Ultimate tensile strength, Compressive yield strength, Ultimate compressive strength.
  - ✓ to accept.
- Without a material, mass is **zero**. Values display in document units.
- Once assigned, the menu item becomes **Edit material**, and *Show/hide center of mass* appears.
- Custom libraries are CSV-based Material Library tabs (Category, Name, Density columns), added via Preferences.

---

## 10. Printing and Performance panel (one line each)
- **Print** [DOCS]: Document menu > Print opens a dialog with a dotted page-border preview over the graphics area (the model can still be positioned with normal navigation), paper size, Portrait/Landscape, **Print** (hands off to the OS print dialog) or **Download image**.
- **Performance panel** [DOCS]: a Document-panel icon (left strip) with Tab performance (total regeneration time, slowest features over 10 s, etc.), Tab graphics (primitive counts, tessellation), Document performance and System details. Tables are sorted by count, highest first.

---

## 11. Part Studio keyboard shortcut master list (defaults; all user-customizable in Preferences)

Shortcuts are customizable per category: General, Part Studio, Assembly, 3D View, Sketch, Feature Studio, Drawing.
- Each can be disabled or reset, with a *Reset all* button.
- Conflict handling:
  - Onshape-controlled conflicts are errors;
  - browser or system conflicts give an info message;
  - overriding another shortcut gives a warning.
- **Shift+/** opens the shortcut map, which has a search box and tab-type filter [DOCS].

| Key | Action |
|---|---|
| Enter / Shift+Enter | Commit / commit and reopen the same tool |
| Esc | Exit command / cancel dialog |
| Space | Clear selection |
| Ctrl/Cmd+Z, Ctrl+Y / Shift+Cmd+Z | Undo / Redo |
| Ctrl/Cmd+C / V | Copy / paste feature (repeatable) |
| Delete / Backspace | Delete selection |
| Shift+N | Rename selected feature |
| ` / Shift+` | Select other (cycle down / up) |
| S | Shortcut toolbar at the cursor |
| Alt/Opt+C | Search tools |
| Alt/Opt+T | Tab manager |
| Ctrl+Space | Switch between recent tabs |
| Shift+/ | Keyboard shortcut map |
| `[` | Measure |
| Shift+C | Curve/surface analysis |
| Shift+D | Dihedral analysis |
| Ctrl+M | Mate connector dialog |
| K | Show/hide mate connectors |
| A | Flip primary axis (MC/triad editing) |
| Shift+S | Sketch |
| Shift+E | Extrude |
| Shift+W | Revolve |
| Shift+F | Fillet |
| Shift+H | Show/hide sketches |
| P | Show/hide planes |
| Shift+P | Hide all construction geometry |
| Rollback bar selected + ↑/↓ | Step the rollback bar |
| Y / Shift+Y | Hide / show the entity under the cursor or selected |
| Shift+I / Shift+T | Isolate / Make transparent (cumulative) |
| Alt+click | Select through transparent |
| Shift+1..7 | Front, Back, Left, Right, Top, Bottom, Isometric |
| N (again = reverse) | View normal to |
| F | Zoom to fit |
| W | Zoom to window |
| Z / Shift+Z | Zoom out / in |
| Arrows / Shift / Ctrl | Rotate 15° / 90° / 5° |
| Ctrl+Shift+Arrows | Pan |
| Shift+V | Named views |
| Shift+X | Section view |
| Shift+R | View in high quality |

---

## 12. Implementation notes and open questions for noshape
- **Toolbar data model:** an ordered list of groups, each holding a list of tools. A group shows its *face* tool, which swaps to the most recently used tool for the rest of the session. Groups collapse into overflow by available width. Tools can be removed (still reachable through search). Reset restores defaults per toolbar type. Dropdown membership beyond Thicken▾/Enclose and Frame▾ is unverified.
- **Feature-list status** is a 4-level model (OK / info / warning / error) plus orthogonal flags (suppressed, dynamic expression, missing selections, under-defined sketch, out-of-date link, commented, hidden). Folders aggregate the worst child state.
- **Regeneration on error:** a failed top-level feature is rolled back, and later features keep evaluating. The UI shows red but does not halt.
- **Selection:** always toggle-additive. The focused dialog field decides both the filter and the destination of picks. Pre-highlight is outline-only (fill alpha 0); selection fill is orange rgba(255,169,10,0.8). Left-to-right box = window (solid blue); right-to-left = crossing (dashed yellow).
- **Edit-in-place:** editing a feature rolls the view back to that feature; *Final* shows the end state. The live preview blends before and after via the opacity slider.
- **Remembered defaults:** parameters tagged REMEMBER_PREVIOUS_VALUE take their default from the user's last use of that tool. Camera toggles have an explicit "Set as default"; Orient-normal-to-sketch persists automatically.
- **Unknowns:**
  - exact 8 default part colors (hex);
  - default shortcut-toolbar contents;
  - full per-entity *Select ▸* submenu lists;
  - Workspace-units dialog field order;
  - the full mass-unit list (likely kg, g, lb, oz).
