# F. Drawings

Sources: Onshape Help `Drawing__*.md` (66 pages, Oct 2026 build) = [DOCS]; client bundle strings = [BUNDLE]. Nothing here was observed live. The drawing editor is a separate app inside the web client: its toolbar command IDs are **not** in `bundle-command-actions.txt` (only Part Studio/Assembly IDs are), so toolbar order below is pieced together from the docs (see the note in §3).

Out of scope (one line each): **MBD** (model-based definition annotations pulled into views, MBD status panel), **Inspection items / Inspection table** (numbered balloons + CSV export), **Revision table / release management** (needs Release management turned on), **mobile** (iOS/Android can only rename, delete or export a drawing). Each gets a short stub in §16.

---

## 1. Drawing element: fundamentals

### 1.1 What a drawing is
- A tab type ("Drawing") in a document. Drawings are built on the **DWG** database; **DXF** is also supported. [DOCS]
- Three ways to make one: (a) from a part/surface/curve/Part Studio/assembly, (b) empty, (c) by importing a .DWG/.DXF. [DOCS]
- **No simultaneous editing.** If another user already has the drawing tab active, you get a message naming who is editing it. [DOCS]
- All views of one part in a drawing come from the **same version** of that part. New views (drawing/projected/auxiliary/section) use the version the existing views use. [DOCS]
- When the source model changes, the drawing goes **out of date** and the Update button becomes active (§5.9). [DOCS]

### 1.2 Drawing tab context menu (right-click the tab) [DOCS]
| Item | Behavior |
|---|---|
| Delete | Deletes the tab even if it is active. The last remaining tab can't be deleted. |
| Open in new browser tab | |
| Rename | Opens the rename dialog. |
| Properties | Metadata dialog for the drawing (part number, revision, description, title 1–3, ...). Fields defined by Company properties are grayed out. Edits flow into the title block. |
| Duplicate | Copies the tab within the document and keeps the references to the original Part Studio. |
| Copy to clipboard | Then use "Paste tab" from the + menu in another document. The source Part Studio is pasted too, and no references to the original document are kept. |
| Change to version... | Re-points the drawing to a different version/revision of its source (latest, or a hand-picked earlier one). Referencing a version/revision makes the drawing open much faster. |
| Select as document thumbnail | |
| Update linked document... | |
| Move to document | Moves the drawing to a new or existing document. Referenced Assembly and Part Studio tabs move too, and a link is created. Blocked if it would leave a document with no tabs. |
| Export | Opens the Export Drawing dialog (§15). [BUNDLE] also has "Export DXF/DWG of __name__…" and "Export as DXF/DWG". |
| Release | Release candidate (out of scope). |
| Revision history | (out of scope) |
| Create task | (out of scope) |

### 1.3 Screen layout [DOCS]
- **Top toolbar**: drawing tools (§3).
- **Left**: **Sheets panel** (toggle with Ctrl+S). The current sheet name shows at the upper left of the drawing space.
- **Right side icon strip** (each icon toggles a panel): **Properties** (drawing properties, §12), **Styles** (§13), **MBD status** (out of scope), **Inspection table** (out of scope).
- Background color (Dark/Light) is set in My account > Preferences > Drawings, then "Save drawing settings". [DOCS] [BUNDLE] "Drawing settings updated." / "Warning: White is a difficult color to see on an Onshape drawing."
- Navigation (pan/zoom mouse mapping) can be set to match traditional CAD systems in account settings. [DOCS]

### 1.4 Cursors [DOCS]
- **Plus-sign cursor**: the command needs a **position** pick.
- **Box cursor**: the command needs an **entity** pick.

### 1.5 Grip points [DOCS]
- **Circular** grips **move** an entity. **Square** grips **resize** it. **Red** grips mean an **error** (dangling reference, §5.10).

| Grip type | Action | Cursor |
|---|---|---|
| Normal resize | drag to resize | resize arrows |
| Rotate | drag to rotate | cross |
| Flip | click to flip orientation | small turn arrow |
| Point attachment | drag to reattach to another point | 4-way arrow |
| Edge attachment | drag to reattach to another edge | 4-way arrow |
| Dimension text | the whole dimension is draggable, no grip needed | 4-way arrow |

### 1.6 Snap points and inferencing [DOCS]
- Snap points are **orange**. Once a snap is visible you can click; you don't have to hit the point exactly.
- Dimension snap shapes: **square** = endpoint, **triangle** = midpoint, **diamond** = quadrant point on a circle/arc, **circle** = arc/circle center. A center snap can be dragged out to a quadrant point.
- Midpoints and quad points are **off while dimensioning**. **Shift+Q** toggles them on/off for the current command. They are always available when you edit a placed dimension. Dimensions never snap to midpoints by default.
- Callouts snap to any entity (point, midpoint, line). Dimensions snap only to references that make sense for the active dimension type (e.g. the 2-point linear tool only offers points).
- **Inferencing**: thin dashed lines appear while you place notes, dimensions, tables, images or symbols; clicking while one shows aligns to it. Red inference points sit on edges and midpoints; pink horizontal/vertical inference lines come off views, lines, dimensions and annotations. **Hold Shift to turn inferencing off** during placement. Dimension text centers are inferenced too, so you can line up a second dimension's text with the first.
- Snapping is **not** associative. After placement, every entity drags independently.
- Sketch inferencing: only horizontal/vertical constraints are inferred. Hovering one snap point and then another builds a point at the intersection of the two axes (red inference lines). Inference lines start from any snap point.

### 1.7 Copy and duplicate [DOCS]
- **Alt+LMB drag** on a hovered entity (cursor shows "+") drops a copy. Works for notes, GD&T frames, datums, hole callouts, BOM tables, surface finish symbols, weld symbols, balloons, tables, and views.
- Ctrl+C / Ctrl+V for views, notes, sheets, tables. Entities can be pasted across sheets, drawings, workspaces and documents (views only within the same workspace).
- Copying a view label with Ctrl+C applies the default Note properties. Alt+drag keeps the label's own properties.

### 1.8 Selection highlight colors [DOCS]
- Hovering a dimension, note or callout turns its referenced edges **blue**.
- Annotations grouped with the selected one highlight **yellow**.
- Hovering a constraint gives it a yellow background and turns the referenced entities orange.
- Hatch hover rules: §10.3.
- Hovering a BOM, cut list or hole table row cross-highlights the matching callouts (blue) and geometry in all views.

---

## 2. Creating a drawing

### 2.1 Entry points [DOCS]
1. **Part/surface/curve** in a Part Studio Parts list, or an **instance/subassembly** in the Assembly Instances list: right-click > **Create drawing of <name>**. [BUNDLE] wording: "Create Drawing of __name__…".
2. **+ (Insert new tab) menu**, lower left > **Create Drawing**. This starts an empty drawing with no source.
3. **Import** a .DWG/.DXF from the Documents page (Create > Import files..., which makes a new document named after the file) or from + > Import inside a document (makes a new tab named after the file). See §15.3.

### 2.2 Create drawing dialog [DOCS]
- **Name field** at the top, pre-filled from the part/curve/surface name. Click it to edit. (The empty-drawing path is described the same way.)
- **Custom template** button, top left. Opens the Custom template dialog (§2.4).
- **Filters (left column)**:
  - **Recently opened**: templates in most-recently-used order.
  - **Onshape**: Onshape-supplied public templates ("Onshape ANSI Drawing Templates", "Onshape ISO Drawing Templates", ...). Also reachable via "Show Onshape drawing templates".
  - **My templates**, with sub-filters **Created by me** and **Shared with me**: any `.DWT` file in documents you can read.
  - **Public**: templates other users made public.
  - **Teams and companies**: one entry per team/company you belong to (listed as the team/company name under Existing templates).
- **Standards bar (top/right)**:
  - **All**
  - **ANSI**: third-angle projection, inch or mm units.
  - **ISO**: first-angle projection, mm units.
  - **JIS**
  - Only templates matching **both** the selected standard and the selected filter are listed.
- **Template list**: one entry per standard/sheet size. [BUNDLE] size strings include A0–A9 and "ANSI C", "ANSI D", "ANSI E" (ANSI A/B are presumably plain letters).
- **Views option (bottom)**:
  - **Suggested views**: places up to **6 orthographic views** (front, top, left, back, bottom, right). The help also says "four views of the part are inserted"; Views.md says the 4 standard views are **top, front, right, isometric**. The implementer should treat the default set as Front/Top/Right(+Iso) laid out per the template's projection angle.
    - When the part has MBD data, the views on planes holding MBD annotations are prioritized (top/right/front bias, adapted for first-angle).
  - **No views**: empty sheet.
  - Drawing Basics adds that after creation, "a view is attached to your cursor"; **Esc** cancels it, a click places it.
- **OK / Cancel**.
- The template sets the drawing's starting system variables, sheet size, border entities, units, standard and other properties.
- New templates have **centermark extensions on** by default. Select a centermark and press Delete to remove it.
- If a named view, exploded view/named position or display state is active when you first create the drawing, the Insert view fields are pre-filled with it.

### 2.3 Source selection and configuration [DOCS]
- From a part's context menu, the source is that part (in its current configuration) at the current workspace.
- Otherwise the source is picked in **Insert view** (§5.1): Current document or Other documents, plus version/workspace, configuration, and entity type filter (Parts / Part Studio / Assemblies / surfaces / sketches / curves / composite parts / sheet metal flat patterns).
- Change configuration later via Sheets panel > reference > **Change configuration** (§11.4). Change version via tab > **Change to version...**.

### 2.4 Custom template dialog (from the Create drawing dialog) [DOCS]
| Field | Options / defaults |
|---|---|
| Standard | ANSI, ISO, JIS |
| Language | language used for title block text |
| Size | list depends on Standard |
| Units | Millimeters, Centimeters, Meters, Inches, Feet and inches, Foot, Yards (default comes from the standard) |
| Decimal separator | Period / Comma (default comes from the standard) |
| Projection | Third angle / First angle (default comes from the standard) |
| Border | Include / do not include border |
| Horizontal zones | number |
| Vertical zones | number |
| Start zones | which corner zone labeling starts from |
| Titleblock | Include / do not include title block |
| Views | suggested views or empty (no views) |
| OK / Cancel | |

---

## 3. Drawing toolbar

**Order caveat:** the docs never print the full left-to-right order, and the drawing command IDs aren't in the bundle. The order below follows Onshape's documented groupings: Update, then views, then dimensions, then annotations, then construction, then sketch, then tables/inserts, then utilities, with inspection at the far right ("Inspection item ... to the right side of the Drawings toolbar"). [DOCS]

Some tool icons carry **open circles**. Those tools pick **snap points**; the rest pick **edges**. [DOCS]

Behavior shared by all tools:
- **Click the active tool again (or press its shortcut again) to exit it.** The shortcuts are documented as "Selects or exits ...".
- **Esc** ends the tool.
- Projected view stays active after use and is also active by default right after the first view is placed.

| # | Tool (tooltip) | Group / dropdown | Shortcut | Notes |
|---|---|---|---|---|
| 1 | **Update** (Update drawing) | standalone | Ctrl+Q | Grayed when up to date, active when out of date (§5.9). |
| 2 | **Insert view** | Views | – | Insert view dialog (§5.1). |
| 3 | **Projected view** | Views | P | §5.3 |
| 4 | **Auxiliary view** | Views | – | §5.4 |
| 5 | **Section view** | Views | – | §5.5. [BUNDLE] "Section view…" |
| 6 | **Aligned section view** | Views | – | §5.6 |
| 7 | **Broken-out section** | Views | – | §5.7 |
| 8 | **Detail view** | Views | – | §5.8 |
| 9 | **Break view** | Views | – | §5.8b |
| 10 | **Crop view** | Views | – | §5.8c |
| – | (Flat pattern view) | n/a | – | Not a toolbar button. Insert via the Insert view dialog's flat-pattern filter. |
| 11 | **Dimension** (smart) | Dimension dropdown (default) | D | §6.2 |
| 12 | **2 point linear dimension** | Dimension dropdown | – | point–point (snap-point tool) |
| 13 | **Point to line dimension** | Dimension dropdown | – | |
| 14 | **Line to line dimension** | Dimension dropdown | – | parallel lines only |
| 15 | **Ordinate dimension** | Dimension dropdown | – | |
| 16 | **Line to line angular dimension** | Dimension dropdown | – | |
| 17 | **3 point angular dimension** | Dimension dropdown | – | |
| 18 | **Radial dimension** | Dimension dropdown | Shift+R | |
| 19 | **Diameter dimension** | Dimension dropdown | Shift+D | |
| 20 | **Foreshorten dimension** | Dimension dropdown | Shift+H | |
| 21 | **Arc length dimension** | Dimension dropdown | – | |
| 22 | **Chamfer dimension** | Dimension dropdown | – | |
| 23 | **Maximum or minimum dimension** | Dimension dropdown | Ctrl+M | |
| 24 | **Hole/thread callout** ("Hole callout" [BUNDLE]) | Dimension dropdown | – | Docs: "applied ... through the Dimensions dropdown menu, Hole callout tool". |
| 25 | **Note** | Annotations | N | §7.1 |
| 26 | **Callout** (balloon) | Annotations | – | §7.2 |
| 27 | **Geometric tolerance** | Annotations | – | §7.4 |
| 28 | **Datum** | Annotations | – | §7.5 (tabs Datum / Datum target) |
| 29 | **Surface finish** | Annotations | – | §7.6 |
| 30 | **Weld symbol** | Annotations | – | §7.7 |
| 31 | **2 point centerline** | Centerline dropdown | – | §7.8 |
| 32 | **Edge-to-edge centerline** | Centerline dropdown | – | |
| 33 | **3 point circle centerline** | Centerline dropdown | – | |
| 34 | **2 point circle centerline** | Centerline dropdown | – | |
| 35 | **Centermark** | Construction | – | dialog: Single / Circular / Linear |
| 36 | **Virtual sharp** | Construction | – | |
| 37 | **Line** | Sketch | L | §10 |
| 38 | **Center point circle** | Sketch (circle dropdown) | C | |
| 39 | **3 point circle** | Sketch (circle dropdown) | – | |
| 40 | **Tangent arc** | Sketch | – | |
| 41 | **Corner rectangle** | Sketch (rectangle dropdown) | G | |
| 42 | **Center point rectangle** | Sketch (rectangle dropdown) | R | |
| 43 | **Spline** | Sketch (spline dropdown) | – | |
| 44 | **Spline point** | Sketch (spline dropdown) | – | |
| 45 | **Sketch point** | Sketch | – | |
| 46 | **Trim** | Sketch | M | |
| 47 | **Hatch region** | Sketch | – | |
| 48 | **Constraints menu**: Horizontal (H), Vertical (V), Coincident (I), Perpendicular (Shift+L), Parallel (B), Tangent (–) | Constraint dropdown | see column | §10.4 |
| 49 | **Table** | Table dropdown | – | §8.1 |
| 50 | **BOM table** (Insert BOM) | Table dropdown | – | §8.2 |
| 51 | **Revision table** | Table dropdown | – | needs release management (out of scope) |
| 52 | **Hole table** | Table dropdown | – | §8.4 |
| 53 | **Cut list** table | Table dropdown | – | §8.5 |
| 54 | **Custom table** | Table dropdown | – | §8.6 |
| 55 | **Insert image** | Insert | – | §9.1 |
| 56 | **Insert DXF or DWG** ([BUNDLE] "Insert a DXF or DWG file") | Insert | – | §9.2 |
| 57 | **Format painter** | Utility | – | §9.3 |
| 58 | **Inspection item** | right end | – | out of scope |
| 59 | **Hide inspection items** (toggle) | right end | – | out of scope |

Other drawing-mode keys: F (zoom to fit), W (zoom window), Shift+Z (zoom in), Z (zoom out), [ (Measure), S (shortcut toolbar), Delete, Ctrl+S (Sheets panel), PgDn/PgUp/Home/End (sheet navigation), Shift+Q (mid/quad points), Shift+N (rename selected sheet/view). [DOCS]

---

## 4. Keyboard shortcuts (drawing) [DOCS]

| Key | Action |
|---|---|
| f | Zoom to fit |
| w | Zoom window |
| Shift+z / z | Zoom in / zoom out |
| d | Dimension (select/exit) |
| Shift+r | Radial dimension |
| Shift+d | Diameter dimension |
| Shift+h | Foreshorten dimension |
| Ctrl+m | Maximum/minimum dimension |
| Shift+q | Toggle midpoints and quad points during the current dimension command |
| n | Note |
| p | Projected view |
| Ctrl+q | Update drawing ("Updates all views and properties on each sheet") |
| l | Line |
| c | Center point circle |
| g | Corner rectangle |
| r | Center point rectangle |
| m | Trim |
| h / v / i | Horizontal / Vertical / Coincident constraint |
| Shift+l | Perpendicular |
| b | Parallel |
| s | Shortcut toolbar (if enabled; Esc closes it) |
| [ | Measure tool |
| Ctrl+s | Show/hide Sheets panel |
| PgDn / PgUp / Home / End | Next / previous / first / last sheet |
| Delete | Delete selection |
| Shift+n | Rename selected sheet or view (single selection in the Sheets list) |
| Alt+LMB drag | Copy the selected annotation(s) to the drag location |
| Ctrl+c / Ctrl+v | Copy/paste (views, entities, table cells) |
| Shift (held) | Turn off inferencing during placement. Show all constraints while hovering. Move one entity of a group (Shift+drag). Move one centermark extension line. Multi-select table cells. |
| Esc | End tool, cancel a pending view, cancel hatch creation |
| Enter / Tab / Shift+Tab / Alt+Enter | Table cell nav: next cell / previous cell / new line inside a cell |

Shortcuts are user-customizable (Preferences > Keyboard shortcuts, with a Drawing category). [DOCS]

---
## 5. Views

### 5.1 Insert view [DOCS]
Places a view of a part, surface, assembly, sketch, curve, composite part or flat sheet metal pattern on the active sheet. **Label and scale are off by default.**

1. Click **Insert view**. The Insert view dialog opens with an **Insert** button (part/assembly icon).
2. Click **Insert** to open the **Select a part or assembly** dialog:
   - **Current workspace** lists the Part Studios and Assemblies in this document.
   - **Other documents** uses the standard document filters, then a **Version graph** to pick a version or workspace. Also offers a **released items only** filter and **Create version**.
   - **Entity filter icons**: **Parts** (default), Part Studio (whole), Assemblies, Surfaces, Sketches, Curves, Composite parts, Sheet metal flat patterns. An icon only shows when the selected Part Studio/Assembly contains that type.
3. Insert view dialog fields once a source is chosen:

| Field | Part Studio source | Assembly source | Options / default |
|---|---|---|---|
| View orientation | ✓ | ✓ | Top, Left, Right, **Front (default)**, Back, Bottom, Isometric, plus any **Named views** (with/without section planes) |
| View scale | ✓ | ✓ | dropdown of scales |
| View simplification | ✓ | ✓ | None, Absolute, Ratio to studio, Ratio to part, **Auto (default)**. Applies only to newly inserted views. Default comes from Properties > Views > Insert view defaults. |
| Show sketch appearances | ✓ | (✓) | checkbox, off |
| Show annotations from model | when a **part** is selected | | checkbox, **on**. MBD data shown in the view. Inherited by child views. |
| Explode/Position | | ✓ (if any) | exploded views / named positions |
| Display state | | ✓ (if any) | display states |

4. Move the cursor over the sheet. A **preview** follows it; click to place.
- If the drawing is out of date when a new view is placed, it **updates automatically** first.

### 5.2 View properties dialog [DOCS]
Open it by **double-clicking** a view, or right-click > **View properties**. It opens at the top left of the drawing. With several views selected, fields that differ show "multiple values".

| Field | Details |
|---|---|
| Document | source document name |
| Workspace / Version | workspace name if the source is in this document, otherwise the version name (moving tabs to another document auto-versions it) |
| Type | Part or Assembly |
| Reference | part/assembly name, linked to open the source tab |
| Display state | (assemblies) dropdown; the view updates |
| View orientation | Top, Left, Right, Front, Back, Bottom, Isometric, Named. A named view deleted from the source shows grayed out. |
| Scale | Format N:N or N/N. User-entered values always have denominator 1. Double-click the Scale label to edit. Choices include **Parent scale** (link to the parent; the default for Projected/Auxiliary/Section views) and **Sheet scale** (link to the sheet scale). |
| Rotation angle | Degrees, 0–360. Arrow reverses direction. New views start at 0°. Editable only if the view has no parent and no children, or its parent alignment is suppressed. A view with children can never be rotated, even with alignment suppressed. |
| Tangent edges | Hidden / Solid / Phantom |
| View render mode | **Best performance (default)** / Best quality. Use Best quality if edges render wrong. Also applies to export. |
| View simplification | Auto (default) / Absolute (length in drawing units; parts smaller than the threshold disappear) / Ratio to studio (% of Part Studio/Assembly size; small parts may disappear) / Ratio to part (% of part size; keeps all parts, simplifies detail). Children inherit, but can be changed independently later. |
| Sheet | dropdown; moves the view to another sheet |
| Name | default `<view perspective>-<part name>`. Renaming doesn't change orientation or part. Shift+N renames from the Sheets list. |
| View label | Checkbox + label text. Applied automatically to Detail and Section views; Projected/Auxiliary need it checked. Detail/Section also get optional **prefix** and **suffix** (multi-line label). Changing the letter also changes the parent's reference letter (detail circle / cutting line). |
| Scale label | checkbox; shows the scale under the view |
| Show sketch appearances | checkbox |
| Show annotations from model | checkbox (MBD; default on; inherited) |
| Move to sheet | opens a dialog with a sheet dropdown |
| BOM table reference | the assembly a BOM table references |
| Cut list table reference | |
| Apply parent view section cut | Only for a Projected view made from a Section view. Toggles whether the parent's cut is carried over. |

Rotation and alignment rules:
- A view rotates about the center of its rectangle; the rectangle resizes. A detail view rotates about its circle center, keeping the same geometry and circle size.
- At a non-zero rotation, **reconnect alignment** controls and commands are disabled. Set rotation back to 0° first.
- Dimensions follow rotation. Horizontal/vertical linear dimensions stay horizontal/vertical; aligned/rotated ones stay aligned to their geometry.
- The scale and label stay centered below the rotated view rectangle or detail circle.

### 5.3 Projected view (P) [DOCS]
- Steps: click an existing view, then drag in a direction (the possible projections preview as you move). **Click** places the view without the parent's cuts; **Shift+click** places it **with the parent's section cuts**.
- Label and scale are off by default. Scale = Parent.
- The tool stays active until you click its icon again. It is auto-activated after the first view is placed.
- Projection follows the drawing's **Projection angle** (Properties > Views > All views): first-angle (ISO) or third-angle (ANSI).
- A projected view inherits its source view's settings. A projection of a broken view is also broken.

### 5.4 Auxiliary view [DOCS]
An orthographic view folded 90° out from a selected edge (usually a slanted one), or about a sketch line or centerline.

Dialog fields:
- **Label**: auto-assigned A–Y, skipping I, O, Q, S, X, Z. Editable.
- **Type**:
  - **Normal**: perpendicular to a linear edge.
  - **In line**: about a sketch line or centerline.
  - **Axis**: looking down a hole axis.
- **Edge/line selection** field: fills when you click the edge or line.
- **Reverse** (flip) icon.
- **Show view reference indicator**: checkbox, **on** by default.
- Then drag the view and click to place it. The view is placed and the dialog closes.

Behavior:
- Scale is off by default and = Parent.
- Selecting the aux view highlights the source edge in the parent.
- Reference indicator: the snap at the arrow "elbow" changes its length; the middle snap slides it between the selected edge and the middle of the aux view.
- Reference indicator context menu: **Edit auxiliary view**, **Flip direction**, **Clear selection**, **Zoom to fit**, **Delete**.

### 5.5 Section view [DOCS]
Section, jogged section or partial section of an existing view (a section view can be the source). **Label on, scale off** by default.

Dialog:
- **Orientation**: Vertical | Horizontal | Angular.
- **Label**: auto-assigned A–Y, skipping I, O, Q, S, X, Z.
- **Show cut geometry only**: checkbox.
- **Cut angle** (Angular only): numeric, with an **Opposite direction** toggle.
- **Section depth**:
  - **Up to entity**: then select the entity on the drawing.
  - **Blind**: depth value.
- **Exclude from cut** (assembly views only): place the cut line, check the box, then click the parts to exclude.
- **Cut from section** (when the source is a section view): cut the already-sectioned geometry. Unchecked = cut the base geometry.
- Check (accept) / X (cancel).

Creation:
1. Hover the part to show snap points.
2. Click once to place the dotted cutting line (Angular: click a second snap to set the angle).
3. Click more snaps to make a **jogged** section (max 4 points for angular jogged).
4. Drag the section view away and click. **The side you drag toward sets the viewing direction.**

Editing:
- Select the cutting line. The **elbow** snap lengthens or shortens it; pull the line inside the part for a **partial section**.
- Drag a placed snap point to a new snap to move the line; the view updates.
- Right-click the line:
  - **Flip direction**: labels swap sides and the view regenerates.
  - **Edit section**: reopens the dialog (check / red X).
- Add a segment: drag a snap point to a new location and click the new snap. Remove a segment: click a snap point that shows an orange box. Click away to finish.
- Selecting a section view highlights its cut line in the parent.
- Hatch: auto-applied. Edit via view context menu **Edit hatch** or Properties > Views. Hatching never overlaps annotations.

Interactions:
- From a view with a broken-out section, the new cut uses the base geometry.
- Horizontal/vertical sections and breaks can be combined (even jogged). Moving a break updates all related views.
- **Angular** sections are not allowed on views with breaks, and breaks are not allowed on views with angular cutting lines.

### 5.6 Aligned section view [DOCS]
Part of the cut is rotated onto the view plane. Typical use: cylindrical, partly symmetric parts.

Dialog fields:
- **Label**
- **Center**: click the center of the cutting line.
- **Viewing plane location**: point to project from.
- **Revolved plane location**: point that is revolved into alignment.
- **Show cut geometry only**
- **Exclude from cut** (assembly only)
- Click to place. Dragging across the source view flips the viewing-plane arrows.

Behavior:
- **Cut from section** is checked by default when the source is a section or aligned section.
- Edit via right-click > **Edit**. Dangling references show as **red markings** in the dialog.

### 5.7 Broken-out section [DOCS]
Dialog:
- Closed profile drawn with the **Spline** tool; the **Spline point** tool adds edit points.
- **End type**:
  - **Blind**: depth.
  - **Up to entity**: a point or edge, possibly in another view.
- **Offset**: optional, with distance.
- **Exclude from cut** (assembly only).
- Accept with ✓.

Behavior:
- Right-click: **Edit section**, or **Remove section** (removes the cut and leaves the spline for reference).
- The depth point is associative to the model. On update the spline and depth move with it. If the update fails, the **spline turns red**; redefine the depth point to fix it.

### 5.8 Detail view [DOCS]
Dialog fields:
- **Profile tool**: Circle | Rectangle | Spline | Polygon.
- **View scale**
- **View label**
- **Profile style**:
  - **In line**: label sits in the source view.
  - **Connected**: dashed line to the source; label hidden.
  - **Leader**: label with line and arrow.
- **Show detail view profile**: checkbox; outline around the detail view matching the source outline.
- **Clip border to geometry**: show only geometry, without the view border.

Creation:
- Circle: click the center, then click the radius.
- Spline/polygon: click successive points until the region closes.
- **Esc, then click** picks an existing closed region.
- Then drag and click to place the detail view.

Editing:
- Hover the detail circle on the source view. The grip **between the arrows** resizes it; the **center grip** moves it.
- Right-click the circle > **Add leader** to move the detail name on a leader.
- Selecting the detail view highlights its circle in the parent.

### 5.8b Break view [DOCS]
Dialog fields:
- **Orientation**: Horizontal | Vertical.
- **Break line type**: Zig zag, Small zig zag, Curve, Straight.
- **Gap distance**

Creation and editing:
- Click twice in the view to place the two break lines (the gap).
- To remove a break, select it and press Delete.

### 5.8c Crop view [DOCS]
Dialog fields:
- **Shape**:
  - **Circle**: click center, click circumference.
  - **Spline**: click points, double-click closes; Spline dropdown > **Add spline point**.
  - **Polygon**: click corners, double-click closes.
- Accept with ✓.

Right-click options: **Edit crop**, **Remove crop** (leaves the boundary; select it and press Delete to remove it).

### 5.8d Flat pattern view [DOCS]
- Can only be inserted from the Insert view dialog (sheet metal flat pattern filter).
- Shows bend lines (up/down styles from Properties), form outlines (cut outline plus top/bottom surface outlines), form centermarks, and the large diameter of counterbore/countersink holes on the top/bottom faces.
- Bend lines and bend notes can be toggled.

### 5.8e View-type creation matrix [DOCS]
| New view ↓ / can be made from → | Base | Projected | Isometric | Auxiliary | Section | Aligned sect. | Broken-out | Detail | Break | Crop | Flat pattern |
|---|---|---|---|---|---|---|---|---|---|---|---|
| Projected | ✓ | ✓ | – | ✓ | ✓ | ✓ | ✓ | ✗ | – | – | ✓ |
| Auxiliary (from linear edge) | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | – | ✗ | ✗ | ✓ | ✓ |
| Section | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |
| Aligned section | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | – | ✓ | ✓ |
| Broken-out section | – | ✓ | – | ✓ | ✗ | ✗ | ✓ | ✗ | ✓ | ✓ | – |
| Detail | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | – | ✓ | – | ✓ | ✓ |
| Break | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | – | ✗ | – | – | ✓ |
| Crop | – | ✓ | – | ✓ | ✓ | ✓ | ✓ | ✗ | ✗ | – | ✗ |
| Flat pattern | only from the Insert view dialog | | | | | | | | | | |

(✗ = explicitly not allowed, – = not stated.) Section and aligned section also can't be made when the **cut line is tangent to a cylindrical face**.

### 5.9 Updating a drawing (Ctrl+Q) [DOCS]
- Any change to the source makes the Update icon active (gray when up to date). Even trivial changes count, e.g. moving a sketch dimension or hiding a plane. Document History shows why.
- Update refreshes **only drawing views** (and properties). It does not check links to other documents.
- You can keep working on an out-of-date drawing. **Versioning the document freezes the drawing.**
- Placing a new view on an out-of-date drawing triggers an automatic update.
- After an update, check for dangling entities.
- MBD default tolerance changes flow in on update.
- Callout numbers re-sync to a reordered BOM on update.
- [BUNDLE] related strings: "Failed to update out-of-date drawings.", "This drawing was not updated prior to version creation.", "Update drawings".

**Reference manager** (reference icon in the Sheets panel): **Update all**, or **Selective update**, tick references, then **Update selected**. [DOCS]

### 5.10 Dangling entities [DOCS]
- An entity that has lost one or more references after an update. Candidates: dimensions, section lines, note leaders, callouts, centermarks, geometric tolerances.
- Shown with a **red grip point** on the lost reference (dimensions are drawn red).
- Fix: drag the red grip onto a new reference point.
- Ordinate dimensions whose feature was removed stay in place, turn red, and can be deleted.
- Broken-out splines turn red when their depth point fails.

### 5.11 Moving, copying, deleting and replicating views [DOCS]
- **Delete**: select, then Delete key or context menu > Delete.
- **Move**: drag. Child views stay aligned to the parent (projection axis) unless **Suppress alignment with parent** is used.
- **Move to another sheet**, three ways:
  - context menu **Move to sheet**
  - View properties > **Sheet** dropdown
  - drag the view onto a sheet in the Sheets panel

  Related labels and dimensions move with the view. Parent and aux/child views move **independently**. A view linked to a view on another sheet shows an **arrow badge** on its Sheets-panel icon. Updating a reference updates all linked views. Views can't be unlinked (delete and re-add instead).
- **Copy**: Ctrl+C / Ctrl+V or context Copy/Paste, or Alt+drag. Works within a sheet, across sheets, and across drawings in the same workspace (not across workspaces/documents).
- **Replicate annotations**:
  - Context menu **Select for replication > View** copies all of the view's annotations.
  - **Select for replication > Region** opens the **Replicate region** dialog: resize the box with corner handles, place it with a center-point rectangle, then Enter/✓.
  - Then right-click the target view > **Replicate annotations**. For a region, place a solid-edged box, adjust it, and confirm with Enter/✓.
  - A notification reports how many annotations were replicated.
  - Views don't need to be linked, but geometry must match. Annotations can't be replicated back onto the source view.

### 5.12 View context menu [DOCS]
Right-click a view. Items depend on the view type and its content.

**Show/hide ▸** (each entry toggles between Show X / Hide X):

| Item | Default | Notes |
|---|---|---|
| Hidden lines | hidden | Can then dimension to hidden lines. |
| Bend lines | shown | flat pattern views |
| Bend notes | – | sheet metal flattened views |
| Shaded view | hidden | Detail inherits the parent's shading but can be toggled independently. Decals only show when shaded. |
| Threads | shown | cosmetic thread lines |
| Faulty parts | shown in part drawings, hidden in assembly drawings | |
| Part intersections | hidden | Virtual edges where parts intersect. Also restores parts dropped because of intersections. Advised for assemblies over 20 parts that render wrong. |
| Offset cut lines | – | section views |
| Show/hide sketches (Part Studio) | – | Dialog listing all Part Studio sketches, multi-select, ✓. Also used to insert a sketch into a section view. |
| Sketches (Assembly) | hidden | toggles all sketches inserted in the assembly |
| Sketch points | shown | Size/style set in Properties > Construction geometry. |
| Show/hide edges | – | Dialog: click edges to mark hidden (thick gray). Hidden lines also show gray. Click again to unmark (black). ✓ applies. |
| Centerlines | shown (auto) | Only automatic centerlines on holes/cylinders/spheres. Manually placed centerlines aren't affected. Not offered if there is no circular geometry. |
| Auto centermarks | shown | Shown when the view is normal to the hole face. Patterned/mirrored holes get centermarks and, for circular/linear patterns, connecting lines. |
| Show/hide parts | all shown | Dialog with a **Hidden parts** selection field, ✓. |
| Constraints | hidden | sketch constraints |
| Center of mass | hidden | Parts/composites with a material. Dimensionable. Size set in Properties > Annotations. |

Other items:
- **View orientation ▸**: Top, Left, Right, Front, Back, Bottom, Isometric. The current one is grayed.
- **Tangent edges ▸**: Hidden / **Solid (default)** / Phantom.
- **Adjust linestyle**: dialog. Click edges (they go into the **Edges** list), then set **Linestyle** (dropdown), **Dashes** (for dashed lines), **Thickness**, **Color**. ✓ applies.
- **Insert custom table**
- **View properties**
- **Order ▸**: Bring to front / Send to back.
- **Align views ▸**: Vertically / Horizontally. The cursor becomes a selector; click a second view and the view centers align.
- **Rotate view vertically / Rotate view horizontally**: click a straight edge to make it vertical/horizontal. Only for rotatable views.
- **Switch to...**: opens the source Part Studio/Assembly.
- **Move to sheet**, **Copy**, **Paste**, **Paste as table** (from Excel/Sheets).
- **Group / Remove view from group**: e.g. a hatch or note grouped with the view moves with it.
- **Select for replication ▸ View | Region**; **Replicate annotations**.
- **Edit [view]**:
  - section/detail/crop: edit the shape, add spline points, drag
  - auxiliary: label, source edge, flip, view-plane visibility
  - broken-out: reopens its dialog
- **Display state ▸**: list of states. Parent default "show all", children default "follow parent". Section/Detail views always follow the parent. A deleted state gives "Failed to resolve exploded view" on update and leaves the view empty.
- **Explode/Position ▸**: list of explode/position states. A deleted explode gives "Failed to resolve exploded view". The exploded-tree rollback bar is ignored; every step shows.
- **Suppress alignment with parent**: frees a child to move independently. Doesn't break alignment to its own children.
- **Remove section / Remove crop**
- **Edit hatch** (section) / **Edit hatch region**: dialog with type (ANSI, ISO, General), pattern, scale, angle, color.
- **Change configuration**: mentioned for views in the cut list docs.
- **Clear selection**, **Zoom to fit**, **Delete**.

---
## 6. Dimensions

### 6.1 Common behavior [DOCS]
- **Associative and driven.** Drawing dimensions measure the model; they never drive it. Editing the **value** turns the dimension into an **Overridden** (non-associative) dimension. Its text is always **underlined** (underline means "not to scale"). Clear the value field and exit to restore associativity.
- Workflow: pick 1–2 references (orange snaps or highlighted edges), and the dimension previews attached to the cursor. Click to place the text.
- **Text placement during preview**:
  - drag up/down, away from the picks → **horizontal** dimension
  - drag sideways → **vertical**
  - drag perpendicular to the pick line → **aligned**
  - text can be dragged outside the extension lines
  - horizontal/vertical projected snaps line text up with other text and dimensions (hover a marker to "wake" alignment)
- **Clicking a selected edge again deselects it** (even mid-command) so you can pick another.
- Hovering a placed dimension turns its references **blue**.
- **Re-attach**: drag any grip onto another edge, point, arc, circle or center. The other grips keep their associations.
- **Text drag**: the whole dimension text drags, no grip needed.
- **Flip arrows**: selecting a dimension shows a node near each arrow/tick; click it to flip. Dragging the value also flips arrows. Ordinate dimensions have circular grips at arrow bases.
- **Jogged extension line**: right-click the extension line > **Add jog**. The jog starts at the midpoint; drag its snaps to adjust. Properties can lock jog angles to 45°.
- **Break dimension / Unbreak dimension** (context menu): break a crossing dimension line. Only one of two crossing lines can be broken. The default is set by Properties "Break dimension by default".
- **Center marks on hover**: with the dimension tool active, hovering a circular edge shows an orange center snap plus a vertical center marker, which stays visible.
- Hidden lines can be dimensioned once shown (Show hidden lines).
- Thread lines are dimensionable (hidden-line view of a threaded hole).
- **Dimension context menu**: Edit (opens palette), Paste, Clear selection, Zoom to fit, Delete. Type-specific extras:
  - Break/Unbreak dimension
  - Add jog
  - Foreshorten / Remove foreshorten (radial)
  - Display as direct diameter / Display as foreshorten diameter
  - Add to ordinate dimension group / Reset ordinates (ordinate)
- Deleting a dimension also deletes the callout attached to it.

### 6.2 Dimension types [DOCS]
| Tool | Shortcut | Picks | Placement behavior |
|---|---|---|---|
| **Dimension** (smart) | D | Circle, arc, circle center, line, centermark, point or thread; 1–2 picks; the type is inferred | Like the sketch dimension tool. **Two edges of a cylinder's silhouette in a projected view** give a linear dimension **with Ø auto-added** (picking corner points instead gives a plain linear). **Two concentric circles/arcs** give a **wall-thickness** linear dimension. |
| **2 point linear** | – | exactly 2 points (lines not allowed) | Horizontal, vertical or rotated (aligned) depending on drag. |
| **Point to line** | – | 1 point + 1 line | Horizontal, vertical or rotated. |
| **Line to line** | – | 2 lines; **only parallel lines highlight** after the first pick | Also auto-adds Ø on cylinder silhouettes; works in projected and section views. |
| **Ordinate** | – | First click = datum (0,0) (point, edge, line, arc, centermark, manual centermark), then each point in one direction; Esc ends | Move the cursor around the datum to set the 0 orientation. Each run makes one **ordinate group** (one direction, one datum); run again for the other axis (same datum allowed). |
| **Line to line angular** | – | 2 lines | Preview shows the inner angle. Drag the arc grip across the extension of a leg to switch to the supplementary/vertical angle; the conjugate/reflex angle is also available. |
| **3 point angular** | – | vertex, then a point on each leg | Arc grip toggles between the angle and 360° minus the angle. |
| **Radial** | Shift+R | arc, circle or circular centermark | Context **Foreshorten** adds a jogged leader; drag and click to set it. **Remove foreshorten** undoes it. Works on circles behind other geometry (a guide line is overlaid). |
| **Diameter** | Shift+D | arc, circle, centermark, silhouette edges | Ø symbol added. On a **detail or crop view** it becomes a **foreshortened** dimension automatically. |
| **Foreshorten** | Shift+H | silhouette edge of a cylinder, **or** a centerline + arc/circle edge | Half-diameter: one extension line, one arrow. Centerline-based versions snap to the edge's Min/Max point while dragging. Context **Display as direct diameter**: on a silhouette the value is kept; on centerline-based the value is recomputed and both arrows shown. **Display as foreshorten diameter** reverts. Hover the grip until a flip arrow shows, then click to flip sides. |
| **Arc length** | – | an arc | Dragging to the opposite side gives the conjugate/reflex. |
| **Chamfer** | – | chamfer edge + an adjacent edge | Shows as a Note or Dimension depending on Properties > Dimensions > Chamfer. Double-click to add a prefix. |
| **Maximum or minimum** | Ctrl+M | one arc/circle + any other geometry (no curves) | Max or min is chosen by **where you click on the circle** (near side = min, far side = max). |
| **Hole/thread callout** | – | circle of a Hole feature, or thread of an External thread feature | See §7.3. |

**Ordinate group behavior**:
- Dragging a dimension's **middle snap** moves the whole group (dashed lines show the members). The **outermost snap** moves a single dimension.
- Drag the **jog point** to reposition the leader jog. Aligned jog points stay aligned (red x shows the alignment).
- Drag a datum point to get inference guides for jogging/unjogging.
- Context menu: **Add to ordinate dimension group** (next click adds a value), **Edit**, **Reset ordinates** (back to aligned positions), Clear selection, Zoom to fit, Delete.
- Style comes from Properties > Dimensions > Ordinate: **ANSI** = leader lines only; **ISO** = leader lines plus arrows chaining from the origin. The default follows the template standard.

### 6.3 Dimension palette (properties) [DOCS]
Selecting a dimension with no tool active shows a **palette icon** next to it. Hover the icon, or **double-click the dimension**, to open the palette. Fields depend on the dimension type:

| Field | Options |
|---|---|
| **Above text** | text/symbol above the value |
| **Prefix text** | |
| **Count type** | dropdown (e.g. hole count) |
| **Precision** | 0–8 decimals. Choosing the entry marked **"(Drawing)"** links it to the Properties panel precision (link/unlink per dimension). |
| **Dimension units** | Millimeters, Millimeters (Drawing), Centimeters, Meters, Inches, Inches fractional, Feet and inches, Feet, Yards. A per-dimension override; pick "(Drawing)" to inherit again. Sets both Units and Fractional display. Feet and inches is drawings-only. **Show/Hide units** toggle. |
| **Suffix text** | |
| **Below text** | |
| **Symbol dropdown** | ° Degree (%%d), ⌀ Diameter (%%c), ℄ Centerline, ⌵ Countersink, ↧ Depth, ⌴ Counterbore, □ Square, ⌒ Arc length, ± Plus/Minus (%%p), Ⓤ Unequally disposed profile, Ⓘ Independency, Continuous feature, Spot face, ▷ Translation |
| **Inspection dimension** | toggle; oval frame around the dimension |
| **Dual dimension** | single unit / single unit per drawing properties / dual unit |
| **Tolerance** | None, Symmetrical, Deviation, Limits, Basic, MIN, MAX, Fit, Fit with tolerance, Fit (Tolerance only) |
| ↳ Upper / Lower | for Symmetrical, Deviation, Limits. **+ / −** buttons make a Deviation/Limits pair +/+ or −/−. |
| ↳ Fit type, Shaft class, Hole class, Fit standard | for Fit variants |
| ↳ Fit tolerance precision | 0–8, for Fit with tolerance and Fit (Tolerance only) |
| **Reset text position** | toggle |
| **Parentheses** | toggle; reference dimension |
| **Toggle hole class visibility** | Fit / Fit with tolerance: Stacked without line / Stacked diagonally / Stacked with line |
| **Toggle Radius/Diameter dimension** | switch R ↔ Ø |

Copy/paste works in all text boxes. Styles (font, height, color, arrowheads, gaps, extension-line visibility) are in the Styles panel (§13), not the palette.

---

## 7. Annotations

### 7.1 Note (N) [DOCS]
**Create**:
- **With leader**: hover a view, region or face to get a snap, click to anchor, click again to place the text. A small text box plus the Note dialog appears. Type (the box grows), format, then ✓.
- **Without leader**: click empty space.
- Add more leaders: right-click > **Add leader**. The leader starts at the note grip nearest the mouse; click the target.
- The leader has a movable **node** on its horizontal segment; drag it to lengthen or shorten that segment.

**Manipulate** (no tool active):
- **Rotate** (leaderless notes only): drag the center grip in a circle; a numeric angle readout appears. Rotating an edge-aligned note detaches it.
- **Resize**: square side grips.
- **Explode** (context): becomes polylines on export.
- **Flip** (context, edge-aligned notes only): rotates 180° and moves to the other side of the edge.
- Drag the middle-bottom grip to detach from or reattach to an edge.
- Move text and leader together by dragging the text (the leader re-snaps). Move only the leader by dragging a leader grab point.
- Leader nodes: right-click the leader > **Add node**; right-click a node > **Remove node** or **Add leader** (from that node).
- **Remove leader** (context), or select the leader/text and press Delete.
- **Move to ▸** Title block / Border zones / Border frame / (back to) Drawing. While Format is **locked**, notes on non-Drawing layers can't be edited.
- **Copy/Paste**: across drawings and documents you have permission for.
- **Group annotation** (with callouts) / **Remove annotation from group**. **Group** (with a single view) / **Remove from group**:
  - A note attached to a view entity is grouped with that view automatically.
  - One note connects to one view, but many notes can group with one view.
  - Moving the view moves the note; moving the note doesn't move the view.
  - **Shift+drag** moves one member of a multi-selection relative to the others (grip ignored; no effect if a view is in the selection).

**Text editing**:
- Double-click opens the editor; triple-click opens with all text selected.
- In the editor: Ctrl+A select all, double-click a word, triple-click a line, system clipboard paste.
- Unicode via `\U+00AE`. Codes: `%%d` °, `%%c` ⌀, `%%p` ±.
- Fractions: `n#n ` diagonal, `n/n ` horizontal, `n^n ` stacked (converted on the trailing space; to avoid conversion type another character first, then delete it).

**Note formatting toolbar**:
- **Ruler**: tab stops (Left/Center/Right/Decimal selector; click the ruler to add, drag along to move, drag off to remove), first-line indent, left indent, right indent, bounding-box width handle. No indents/tabs by default. Settings made before typing apply to the whole note.
- **Font**: SHX or TrueType.
- **Text height**: cap height; **becomes the new default**.
- **Bold / Italic / Underline**: TrueType only.
- **Strikethrough**
- **Color**: palette, hex, RGB, mixer.
- **Line spacing**: 1.0/1.5/2.0/2.5/3.0; add/remove space before paragraph; add/remove space after paragraph.
- **Horizontal alignment**: left/right/center/justify. Disabled for leader notes; the standard decides.
- **List**: numbers, bullets, uppercase letters, lowercase letters. Restart numbering by leaving a gap and reselecting.
- **Vertical alignment**: Top/Middle/Bottom. Disabled for leader notes.
- **Note order**: Bottom to top / Top to bottom.
- **Symbols**: same list as the dimension palette.
- **Insert drawing property**: Drawing created by, Drawing created date, Drawing description, Drawing last changed by, Drawing last changed date, Drawing name, Drawing part number, Drawing revision, Drawing Title 1/2/3, Sheet number, Total sheets, Sheet scale, Sheet size.
- **Insert sheet reference property**: properties of the referenced model: Name, Description, Part Number, Revision, State, Vendor, Project, Product Line, Title 1–3, Material, Mass, plus active custom properties (alphabetical, at the bottom).
- **Text format** (for an inserted property): All uppercase / All lowercase / Sentence case / Title case / Unmodified.
- **Date time format**: yyyy-M-d, yyyy-MM-dd, M.d.yyyy, M/d/yyyy, M/d/yy, MM-yy, MM/dd/yyyy, MMM.d, yy, MMMM yy, MMMM d,yyyy, d-MMM-yy, d MMMM yy, dd.MM.yyyy, dd/MM/yyyy, dddd,MMMM d,yyyy, dd-mm-yyyy, d-MMM-yyyy, dd/mm/yy, dd/mm/yyyy, dd.mm.yy, dd.mm.yyyy. Dates use **UTC**.
- **Flag notes**: for list-formatted items only. Check **Flag notes** per item, then pick a flag type and a character size (default **2 Characters**).

**Display rules**:
- Inserted properties show with a **gray background**. Unset ones show **"----"** placeholders. Both can be hidden in Properties > Annotations > Notes. Exports drop placeholders by default.
- Selecting a note turns its target blue and its grouped annotations yellow.

### 7.2 Callout (balloon) [DOCS]
- Associative to part metadata, the BOM table, and the cut list table. **With a BOM or cut list present, it defaults to Item Number.** Reordering the table needs an Update to renumber.
- **Placement**:
  - With leader: snap on a view or face, click the anchor, click the balloon.
  - Without leader: click white space.
  - Leaders on faces end in a **solid dot**, not an arrowhead.
  - Can also attach to centermarks, manual centermarks, linear dimensions and angular dimensions.
  - The leader node on the horizontal segment only exists when Border shape = None.
- **Dialog fields** (left to right):
  - **Part property** dropdown
  - **BOM table property** (table columns)
  - **Instance property** (Reference designator)
  - **Flag note property**
  - **Cut list table property**
  - **Text fields**: above, below, left, right
  - **Insert symbol**
  - **Text height**
  - **Border shape**
  - **Border size** (characters wide)
  - ✓ accept
- **Context menu**:
  - Edit, Copy, Paste
  - Add leader, Remove leaders
  - Group annotation / Remove annotation from group (callouts group with callouts, dimensions, welds, surface finishes, datums)
  - **Move to ▸** Border frame / Border zones / Title block (the layer's formats then apply)
  - Clear selection, Zoom to fit, Delete
  - With several callouts selected: **Align horizontally / Align vertically** (one-shot, not locked)
  - **Add stacked callouts** (assemblies with a BOM): Stacked callouts dialog with a **Stacked instances** field (click instances), or **Add instances from BOM**, which shows a yellow banner; click the BOM's first column, then **Done**; ✓.
- **Grouping**:
  - Drag the callout onto a callout, dimension, weld, surface finish or datum until it highlights.
  - During creation, click the target entity as the placement point.
  - Group to a view: select both > **Group**.
  - Moving the view moves its grouped callouts; moving a callout doesn't move the view.
  - Drag a grouped callout outside its grip to move the whole group.
- Leader nodes: same as notes.
- Style: Font, Text height, Color, Text justification, Stack direction, Stack gap size, Arrowhead.

### 7.3 Hole/thread callout [DOCS]
- Pick a hole-feature circle or an external-thread thread, then drag to place. Metadata fills in automatically, including precision, tolerance, fit class and shaft class from the feature.
- **Add leader** (context) to other holes **of the same size**.
- Leader has a node. Red dashed inference lines show while placing.
- **Edit panel** (double-click):
  - Above text, Prefix text
  - **Count type** (counted holes highlight). "Count by face (Excluding patterns)" is unavailable on patterned holes.
  - Units, Suffix text, Below text, Symbol dropdown
  - Inspection dimension
  - Single/Dual dimension (dual top/bottom or side-by-side)
  - **Override precision/tolerance** checkbox, which unlocks:
    - Precision 0–8
    - Tolerance (same list as dimensions)
    - Upper/Lower
    - **Fit type**: User defined / Clearance / Transition / Interference
    - **Hole class**: None or the available classes
    - **Fit tolerance precision** 0–8
    - **Fit standard**: ANSI (Drawing) / ANSI / ISO
    - **Toggle shaft class visibility** button + dropdown (shaft class hidden by default)
- Thread display:
  - Tapped holes: dashed lines (ANSI) or the 3/4-circle outline (ISO).
  - Occluded threads aren't shown.
  - 45° taper at the thread runout.
  - External thread chamfer and undercut are shown.
  - Toggle via view context menu Show/Hide threads.
- Replicate: view context menu **Replicate annotations**.
- Style: font, height, color, arrowhead, text justification.

### 7.4 Geometric tolerance [DOCS]
**Attach to**:
- edges, holes, dimensions (with or without leader), dimension extension lines, extension lines, surface regions, centermarks, or free space
- dropped on a dimension it snaps and follows it; dragging it off unsnaps it, and dragging away from a dimension adds a leader and keeps it inline

**Dialog**:
- **Upper text** (≤50 chars), **Lower text** (≤200)
- **Prefix / Suffix** (≤20 each)
- **Tolerance frame** rows:
  - **Symbol**: Position, Concentricity, Symmetry, Parallelism, Perpendicularity, Angularity, Cylindricity, Flatness, Roundness, Straightness, Profile (Surface), Profile (Line), Circular runout, Total runout. Plus **Composite** on the 2nd+ frame (spans both frames).
  - **Tolerance** value, with left/right symbol fields (≤7 symbols each)
  - **Datums** ×3: each must start with an uppercase letter; symbols and hyphens allowed
- **Add frame (+)**: up to 5 rows. Right-click a row: **Move up/down**, **Remove** (added rows only). The video mentions a "Remove last frame" icon.
- **Leader** dropdown: Leader / All around / All over.
- **Show/Hide symbols (ø)**: palette. In text fields every symbol is offered; inside frame cells the set depends on the symbol type and cell.
  - Modifiers: MMC Ⓜ, LMC Ⓛ, RFS Ⓢ, Free state Ⓕ, Tangent plane Ⓣ, Unequally disposed Ⓤ, Projected tolerance zone Ⓟ, Statistical, Continuous feature, Spot face, Translation.
  - Diameter-type: Ø, SØ, R, SR, CR.

**Placement**:
- Click to place. The tool stays active for repeated placement until ✓.
- With leader: snap, then drag and click.
- On an edge: place first, then drag onto the edge until no leader shows.
- Along an extension line: drag away from the snap until an extension line appears.

**Grouping**:
- Drag onto a dimension or hole callout (highlight) to group. Moving the tolerance **breaks** a plain dimension group; drop it back on to regroup. Inside a callout group it stays grouped when moved by its grip.
- Moving the dimension moves the group.

**Context menu**: Edit, Paste, Copy, **Paste as table**, Add leader, Remove leaders, Move to ▸ (Border frame/Border zones/Title block), Clear selection, Zoom to fit, Delete.

Leader nodes work as for notes. Style: Font, Text height, Color, Arrowhead.

### 7.5 Datum [DOCS]
The dialog has two tabs: **Datum** and **Datum target**.

**Datum tab**:
- **Label**: auto-increments letters; editable.
- Click an edge (linear or circular silhouette), drag away to set the datum line, click to set the symbol. The tool stays active for repeats.
- Also attaches to:
  - any of the 4 edges of a GD&T frame (red crosshair)
  - dimension leader lines and linear dimension extension lines (can drag beyond them)
  - hole callouts and radius/diameter dimensions, **except** ones convertible to foreshortened (e.g. in detail views)
- Occluded dimension arrows and lines are hidden.
- Change the label: select, then double-click the highlighted square.
- The triangle style comes from Properties > Annotations > Arrowhead.

**Datum target tab**:
- **Target area**: Point (X), Point (dot), Circle, Rectangle, Custom (any closed area).
- **Style** (callout style).
- **Area size**: optional for points (diameter); required for Circle (diameter) and Rectangle (side lengths); not available for Custom. Each has a precision dropdown.
- Click to place the target, then click to place its callout.
- **Label**: added targets increment the digit (A1, A2...); new targets increment the letter (B1, C1...).
- **Origin** (not for Custom):
  - **Length**: click the origin, X-Position / Y-Position distances + precision, X/Y flip arrows.
  - **Angle**: origin at a circle/arc center, Diameter distance (target sits at radius), Angle from horizontal, precision dropdowns, **Radial lines** option, **Reference line** (edge to measure the angle from), Flip/Rotate, **Instance** count (evenly spaced over 360°), Opposite direction (CCW).
- **Targets** field: click in space to add more targets with the same letter. ✓ to finish.

**Context**: right-click a target dimension > **Hide dimension**; right-click the target label > **Show dimensions ▸ This target / All targets**.

Highlight: the selected target and its face turn blue; other targets in the series turn yellow.

Style: datum = Font, Text height, Color, Datum size. Target = Font, Text height, Color, Hatch pattern, Hatch scale. Datums group with callouts.

### 7.6 Surface finish [DOCS]
**Dialog**:
- **Type**: Basic, Material removal required, Material removal prohibited, ISO Basic, ISO material removal required, ISO material removal prohibited.
- **All around**: checkbox.
- **Parenthesis**: checkbox.
- **Direction lay**: dropdown.
- Data cells.
- **Add row**: up to 5 extra rows.

**Placement**:
- Click white space, a point on a circular-view edge, or a face region. Can associate with a dimension extension line.
- The tool stays active for more symbols; ✓ or ✗ ends it.

**Leaders**: Add leader via context; the leader has a node; leader nodes work as for notes.

**Context menu**: Edit, Copy, Add leader, Remove leaders, Move to ▸, Clear selection, Zoom to fit, Delete.

Style: Font, Text height, Color, Arrowhead. Groups with callouts.

### 7.7 Weld symbol [DOCS]
- The dialog layout follows the drawing's **standard (ANSI or ISO)**. The weld standard can be changed separately in Properties > Annotations > Weld symbols.
- **Joint spacer** types: Double V groove, Double bevel groove, Double U groove, Double J groove, Double flare V, Double flare bevel.
- **Joint types**: Square groove, V groove, U groove, J groove, Bevel groove, Single bevel butt broad root, V flare groove, Bevel flare groove, Bead joint, Fillet joint, Plug or Slot joint, Seam joint, Spot joint, Seam centered joint, Spot centered joint.
- **Options**:
  - **Stagger**: when Symmetric is on, or top and bottom joints match.
  - **Symmetric**: all joint types, but not ANSI joint spacers.
  - **Second fillet**: certain joints.
  - **Reference**: enables a notes text box.
  - **All around**
  - **Flag**: field weld.
- Place on a line or vertex. Add more leaders via context; leader nodes work as for notes. ✓ or ✗ ends it.
- Style: font, height, color, arrowhead. Groups with callouts.

### 7.8 Construction tools: centerlines, centermark, virtual sharp [DOCS]
These are sheet geometry representing model features. Drawing constraints can't be applied to them.

- **2 point centerline**: pick two points (snaps optional; endpoints of another centerline allowed).
- **Edge-to-edge centerline**: a single cylindrical/conical **silhouette** edge, or **Shift+select** two edges or two concentric arcs.
- Centerline editing (no tool active): select it, drag end grips to resize (it may be shorter than the references), drag the snap point to move, Delete to remove.
- **3 point circle centerline**: 3 points (hole centers, end, mid or quad points) give a bolt circle. Dimensionable.
- **2 point circle centerline**: center point (any snap), then a circumference point.
- **Centermark** dialog:
  - **Single | Circular | Linear**
  - Click a circle/arc edge.
  - Circular/Linear mark options: **Circular**: Reference center, Bolt circle, Radial lines, Pattern centermark. **Linear**: Connecting lines.
  - ✓ accepts.
  - Context **Align centermark**: click a line or edge to orient it.
  - Delete removes it.
  - Can be added manually to round or elliptical faces not normal to the view.
  - Automatic and implicit centermarks in Projected/Auxiliary views copy along with the view (duplicate sheet, copy/paste, move to sheet).
  - **Extended lines** (Properties): drag a centermark-line grip and all lines move together; **Shift+drag** moves one line. While dragging all lines, a red diamond snap appears on edges.
- **Virtual sharp**: pick two linear edges. Fully associative. Dimensions go to the cross intersection. Display style (Centermark or Edge extension) is set in Properties > Construction geometry.

---
## 8. Tables

### 8.0 Behavior shared by all tables [DOCS]
- **Fixed corner** (anchor) picker at the bottom of every insert dialog: 4 corner boxes, default **upper right**. Placement inferencing works as for other entities.
- **Grab points** (on hover):
  - **mid-points** (top/bottom/sides) resize the whole table proportionally; **double-click a mid-point** auto-fits to content
  - **corner points** move the table and snap to sheet corners (red snap)
  - **double-click the fixed corner** (or any corner) opens Table properties (change the fixed corner)
- **Cell edge drag**:
  - top/right edge: resizes the cells on both sides, table size unchanged
  - bottom/left edge: resizes only the selected row/column; the table grows or shrinks
  - double-click a divider: fit to text
- **Context on cells**:
  - **Size ▸** Columns equally / Rows equally
  - **Resize**: dialog with Column width and Row height; can't exceed sheet size
  - **Duplicate ▸** Rows / Columns
- **Selection**:
  - single click opens the **Cell formatting panel**
  - Shift+click multi-selects
  - click the first and Shift+click the last cell to select a row or column
  - drag-select shows a yellow outline while dragging, then blue
- **Copy/paste**: Ctrl+C a cell range, paste into Excel/Google Sheets or as a new Onshape table.
  - Doesn't work in Safari. Firefox needs Ctrl+V; the context-menu Paste isn't supported there.
  - Can't paste into an existing table. The clipboard empties after one paste.
  - Merged cells are kept, as are text size, bold/italic/underline/strike (only if the whole cell has them) and cell width/height. Background, text color and hyperlinks are lost. Pasted content is center-aligned.
- **Paste as table** (drawing context menu): pastes cells from Excel/Sheets. The top-left corner follows the cursor; click to place.
- Date properties in cells use **UTC**.
- Table formatting defaults: Properties > Tables (§12.7).

### 8.1 Table (generic) [DOCS]
**Dialog**:
- **Rows**, **Columns**
- **Title row** checkbox (spans all columns), **Header row** checkbox (below the title)
- Fixed corner
- Click to place, then ✓. The **Note panel** opens for filling cells: Tab/Enter goes to the next cell, Shift+Tab to the previous, **Alt+Enter** for a new line in a cell. Double-click a cell to edit it later.

**Cell formatting panel**:
- Insert row above / below, Insert column left / right
- Remove rows / columns
- Duplicate
- Size columns equally / Size rows equally
- **Merge cells** (horizontal, vertical or all), **Unmerge cells**
- plus all Note text formatting

**Table properties** (context "Table properties..." or double-click a corner): fixed corner.

### 8.2 BOM table [DOCS]
**Insert dialog**:
- **Source dropdown**: lists the BOM tables in this document (e.g. the master assembly). The **Insert** icon opens the standard insert dialog, with:
  - Current document / **Other documents**
  - **Assemblies**: the assembly's "Subassembly BOM behavior" property decides how subassemblies appear
  - **BOM data**: an uploaded file, or Import
  - **BOM app**: App Store
  - **Release filter** when released parts exist
  - Create version / version graph
- **Template**: only if a company admin has added BOM templates. [BUNDLE] "Default Drawings BOM table template updated."
- **BOM type**: **Flattened** / **Structured - Top level** / **Structured - Multi-level** (item numbers like 1, 1.1, 1.2).
- **Order**: Top to Bottom / Bottom to Top.
- Fixed corner, then click to place.

**Structured numbering** works even when the view reference differs from the BOM reference, as long as one contains the other.

**Row hover** highlights the matching callouts (blue) and the geometry in all views.

**Table context menu** (whole table selected):
- **BOM table properties**
- **Split BOM table below** (at the right-clicked row)
- Copy
- **Move to ▸** Border frame / Border zones / Title block / Drawing
- Bring to front / Send to back
- **Switch to <Assembly tab>**
- **Move to sheet**
- Clear selection, Zoom to fit, Delete

**Sheets panel entry** for the BOM: Properties, Switch to BOM, **Replace** (pick another assembly; the views referencing it are replaced too), Delete.

**Cell formatting**:
- Size columns/rows equally
- **Override** checkbox: replaces the cell content. Shows a black triangle in the cell's bottom-left corner and a tooltip. Needs a Part Number on the row; disabled when several cells are selected.

**BOM table properties dialog**:
- Reference link
- BOM type: Flattened / Structured
- Order: Top to bottom (default) / Bottom to top
- Template
- Fixed corner

### 8.3 Revision table (out of scope)
Release-management driven.
- Dialog: Multi-sheet (Linked / See sheet 1), Visible rows, Order, "Hide revision callouts no longer shown in the table", fixed corner.
- Context items: **Add revision callout**, Hide/Show revision callouts.
- A pending revision row is highlighted blue, with a `*` suffix on the label.

[DOCS]

### 8.4 Hole table [DOCS]
Only for views of a **single part** (not a whole Part Studio).

**Dialog**:
- **Show location data in views**: checkbox.
- **Precision** for coordinates.
- **Views**: click views; click the field again to add more. A live preview fills in as you pick.
- **Origin**: pick it.
- **X or Y axis**: click an edge.
- **Excluded holes**: click holes.
- **Order**: alphanumeric top-to-bottom / bottom-to-top.
- Fixed corner, then place.

**Hole tag/axis context menu**: Hole table properties, **Flip X direction**, **Flip Y direction**, Clear selection, Zoom to fit.

**Table context menu**: Hole table properties, **Split hole table above/below**, Move to ▸, Bring to front, Send to back, Move to sheet, Clear selection, Zoom to fit, Delete.

**Hole table properties dialog**: show location coordinates, **collapse tags**, views, tag order, fixed corner.

Row hover cross-highlights. Shaft class info is hidden by default.

### 8.5 Cut list table [DOCS]
- Needs a composite part plus a Cut list created in the Part Studio (Frames workflow).
- Dialog: **Insert** (pick the cut list), fixed corner, then place. Row hover cross-highlights.
- **Context menu**: Cut list table properties, **Split cut list** (after the row) / "Split cut list table below" and **Unsplit cut list table**, **Change configuration** (affects the table only, not the views), Copy, Move to ▸, Bring to front, Send to back, Move to sheet, Clear selection, Zoom to fit, Delete.
- **Properties**: Sheet, Order (Top to bottom / Bottom to top), fixed corner.

### 8.6 Custom table [DOCS]
- Built from a **FeatureScript custom table definition** (Feature Studio) evaluated on a **Part Studio**. It covers the whole Part Studio, not just the parts in the drawing, and updates on drawing update.
- **Dialog**:
  - **Part Studio**: dropdown of Part Studios already in the drawing, plus **Add** (opens "Select a Part Studio").
  - **Insert**: opens "Select a custom table" with a table dropdown, then **Done**.
  - **Order**: Top to bottom / Bottom to top.
  - Fixed corner, then click to place.
- Also reachable from the view context menu **Insert custom table**.
- **Custom table properties** (context, or double-click a corner): Document, Workspace, Custom table (link to the Part Studio), Feature Studio (link, plus version), Sheet, Order, fixed corner.

---

## 9. Inserts and utilities

### 9.1 Insert image [DOCS]
- Supports JPG, PNG and GIF (logos, QR codes, barcodes).
- **Dialog**: search box plus a list of image tabs in the document; **Browse documents**, which copies the image from another document; **Import** at the bottom.
- Click and drag to place and size (aspect ratio kept). Drag to move it. Inferencing works.
- **Context menu**: **Show image borders / Hide image borders** (applies to **all** images; none are shown by default), **Properties** (sheet, references), **Open linked document**.
- Deleting the source Image tab removes the image on the next update. Images inserted from Documents-page imports are not linked.

### 9.2 Insert DXF or DWG [DOCS]
- **Dialog**: "Search files" plus a list; **Other documents** (copies the file); **Import**.
- Only **model space** content comes in, and only wireframe (lines, arcs, polylines) and notes; no 3D.
- On import:
  - colors are removed (layer default color used)
  - blocks and polylines are exploded
  - simple text becomes editable Notes
- Use **Move to ▸** Title block / Border frame / Border zones to put content on format layers. This is the main way to build templates from legacy CAD drawings.
- [BUNDLE] "The DXF/DWG imported with the following error(s):".

### 9.3 Format painter [DOCS]
**Dialog checkboxes**: **Paste style**, **Paste tolerance and precision**, **Paste content**.

1. Click the **source** dimension or annotation.
2. Click the **destinations**; each updates immediately.
3. ✓ to finish.

Rules:
- Only formats from defaults, Drawing properties or the Styles panel are painted. Overrides made in the Note dialog are not.
- Works dimension ↔ annotation; only styles both share are painted.

---

## 10. Drawing sketch tools and constraints

### 10.1 Sketch tools (sheet geometry) [DOCS]
| Tool | Key | Interaction |
|---|---|---|
| Line | L | Click start, then click each vertex. Each segment is a **separate entity**. H/V inferencing. Snaps to existing objects. Esc ends. |
| Center point circle | C | click center, click radius |
| 3 point circle | – | 3 clicks on the circumference |
| Tangent arc | – | click a line/edge end (tangent start), click the end point |
| Corner rectangle | G | corner, opposite corner |
| Center point rectangle | R | center, corner |
| Spline | – | click points; double-click or Esc ends; drag points to reshape |
| Spline point | – | click on a spline to add points; Esc exits |
| Sketch point | – | click to place; drag to move |
| Trim | M | Click a curve segment to trim to the nearest intersection/boundary. With no intersection the whole curve is deleted. Clicking a point deletes it. |
| Hatch region | – | §10.2 |

- Negative dimension values flip the direction, as in Part Studio sketches. [DOCS]
- Sketch geometry style (Styles panel): **Linestyle** Solid / Phantom / Center / Dashed / Dotted; **Pitch scale** 0.01–5.00 for non-solid styles; **Thickness**; **Color**. Defaults are in Properties > Construction geometry > Sketch geometry. Driven sketch dimension color is set there too.

### 10.2 Hatch region [DOCS]
**Dialog**:
- Sketch tools at the top, for drawing a closed profile; or select an existing closed sketch profile.
- **Standard**: ANSI / ISO / General.
- **Pattern**
- **Scale**: Auto, or 1.0–10.0 in integer steps.
- **Angle**: Auto, or 0–180° in 15° steps.
- **Color**: "Drawing properties" checkbox on uses the global color; uncheck for a custom color, which only shows after ✓.
- Preview at the bottom of the dialog.
- ✓ accepts. A spinner may show while it computes; Esc cancels.

Editing: right-click > **Edit hatch**; Delete key or context Delete.

**Where hatches can be applied/edited**:
| View | Apply/edit |
|---|---|
| Projected | ✓ |
| Auxiliary | ✓ |
| Crop | ✓ |
| Section, Aligned section, Broken-out | ✓ (hatch is **automatic** on creation) |
| Detail | ✗ |
| Break | ✗ |

### 10.3 Hatch highlight rules [DOCS]
- Tool-drawn hatch, idle: hatch color orange, yellow surround.
- Hover near its edge: hatch turns yellow, surround and nodes orange.
- Hovering any hatch highlights the view bounding box yellow.
- Profile-based hatch on hover: hatch orange, profile light blue.
- Hovering elsewhere in the view: view box orange, tool-drawn hatch surrounds yellow, profile hatch color yellow.

### 10.4 Drawing constraints [DOCS]
- Constraints link **sketch entities to each other** or **sketch entities to view geometry** (e.g. perpendicular to a part edge in the Top view). **Not** allowed on construction tools.
- Toolbar constraint menu:
  - **Horizontal** (H): one or more lines, or a set of points
  - **Vertical** (V)
  - **Coincident** (I): 2+ entities
  - **Perpendicular** (Shift+L): 2 lines
  - **Parallel** (B): 2+ lines
  - **Tangent**: line/curve
- Two workflows: tool first, then pick entities; or pre-select, then click the constraint.
- Auto-constraints come from inferencing while sketching (H/V only). Constraint indicators show next to the cursor.
- **Display**:
  - Constraints show only for the hovered entity; **hold Shift** to keep all visible.
  - View context menu **Show/hide > Show constraints / Hide constraints** toggles them; hidden by default.
  - On hover, the constraint icon gets a yellow background and its entities turn orange.
- **Delete**: click the icon and press Delete, or context Delete.
- **Unsolvable** constraints are shown in **red**.

---
## 11. Sheets

### 11.1 Sheets panel (Ctrl+S) [DOCS]
- Left-side panel. Once opened it stays open, with the displayed sheet selected.
- The current sheet is shown in blue with a vertical blue bar on its left. Multi-select is allowed.
- **Double-click** a sheet to display it. Double-clicking the displayed sheet's name opens its Properties.
- Keys: **PgDn / PgUp / Home / End** for next / previous / first / last sheet.
- Templates contain a **main sheet** plus **continuation sheet(s)**. The main sheet shows on creation.
- **Tree contents**: sheets, then their views (Detail/Section views show their identifiers, e.g. "Detail A"), then tables (BOM, ...).
  - Released parts show part number and state.
  - **Latest revision** icon when a view shows the latest revision of a released part.
  - **Linked-view arrow badge** when a view is linked to a view on another sheet.
  - **Pin icon** plus version name for pinned references.
- **Two sort modes**:
  - **Sort by sheet**: sheet, then referenced entities, then views.
  - **Sort by reference**: referenced entity, then sheets, then views.
- Hovering a configured part's reference shows a **tooltip with the configuration**.

### 11.2 Sheet operations [DOCS]
| Operation | How |
|---|---|
| Add sheet | **Create sheet** icon (top right of panel). Inserted **after the highlighted sheet** and displayed immediately. |
| Duplicate | Sheet context > **Duplicate**. The copy goes right after the original (e.g. one sheet per configuration). |
| Copy / Paste | Sheet context > **Copy**, then in the target drawing's panel > **Paste**. Lands at the **end**. Across workspaces the source must be a **version**. |
| Delete | Context > **Delete**. Undo restores it. |
| Reorder | Drag and drop in the panel, or context **Move up / Move down**. |
| Rename | Context > **Rename** (name pre-selected), or **Shift+N**. Renames only the panel entry, not the title block. |
| Move views between sheets | Select views in the panel, then drag onto a sheet, or context **Switch to sheet ▸ target** (also "Move to sheet"). Children don't follow parents automatically. |
| Properties | Right-click empty drawing space > **Sheet properties**, or panel context > **Properties**, or double-click the displayed sheet name. |

### 11.3 Sheet properties dialog [DOCS]
| Field | Details |
|---|---|
| Scale | Sheet scale. Shown in the title block and used by views set to "Sheet scale". |
| Size | Paper size; linked to the title block "Size" field. **Custom** = Width + Height (max **1800 in × 1800 in**). |
| Border | include or not |
| Horizontal zones | count |
| Vertical zones | count |
| Start zones | **bottom right** or **top left** |
| Select a DWT (icon) | **Replace the sheet format** from a .DWT (per sheet) |
| References | All parts, Part Studios and Assemblies used on the sheet, each with a link icon to open it. |
| OK / Cancel (✓) | |

**Replacing the format**:
- **Select a DWT** dialog: Current document / Other documents, search, Import from your computer, Current Documents tab.
- On resize or format replacement the **title block is not replaced**; border frame and zones are **redrawn**; **Drawing-layer content is never touched**.

### 11.4 Reference actions (Sheets panel, right-click a reference) [DOCS]
- **Switch to reference**: opens the source tab.
- **Replace**: opens Insert view and swaps the entity **on every sheet**.
- **Change configuration**: dialog with a Configuration dropdown. Started from a parent-child set under Sort by sheet, it changes **only that set**; under Sort by reference, it changes **all views of that reference**.
- **Pin view reference / Unpin view reference**: for versioned/revisioned references; stops updates.
- **Reference manager** (reference icon): Update all / Selective update, then Update selected.

### 11.5 Title block auto-fields (Onshape templates) [DOCS]
| Drawing property | Title block cell |
|---|---|
| Documentation name, else Nickname, of the creator | Drawn: Name. Only applied at creation; later profile changes don't propagate. |
| Created date (UTC) | Drawn: Date |
| Part Number | DWG No |
| Description | Title |
| Revision | Rev |
| (auto) | Sheet number, Total number of sheets |

- Unset properties show "----". These print; delete the note, or hide placeholders in Properties.
- Title block fields are notes, so they can be dragged, copied and pasted. Double-click a field to edit it in the Note panel.
- Title block labels are plain multi-line notes. Lines can be moved or created.
- Gray entities in the title block/border inherit from document data. Context **Edit note**; hover shows the bound property.

### 11.6 Drawing layers [DOCS]
- Four layers: **Drawing**, **Title block**, **Border frame**, **Border zones**.
- The **Move to ▸** context command is available on notes, callouts, GTOLs, surface finishes, tables and inserted DXF content.
- **Format locked** (the default for new drawings): the three format layers can't be selected or edited.
- DWG/DXF export layer names: `TITLE_BLOCK`, `BORDER_FRAME`, `BORDER_ZONES`.
- DWT export drops the Drawing layer.

---

## 12. Drawing Properties panel

Opened from the right-side icon, or right-click empty space > **Drawing properties**. These are drawing-wide **defaults**; individual entities can be overridden through the Styles panel (§13). A tab icon strip runs across the top: **Units and precision, Dimensions, Annotations, Views, Construction geometry, Formats, Tables, Inspection**.

At the panel bottom: **Update properties from a template...** (Select a DWT icon; dialog as in §11.3, plus Other documents filters Owned by me / Enterprise / Recently opened / Created by me / Shared with me, and View released items / Create version / Version graph), and a **Lock drawing properties** checkbox. [DOCS]

Color pickers everywhere: **Palette** (swatches, hex, RGB) and **Mixer**; "+" under **Custom colors** saves a color. [DOCS]

### 12.1 Units and precision [DOCS]
- **Primary**:
  - **Units**: Millimeters, Centimeters, Meters, Inches, Inches fractional, Feet and inches, Feet, Yards. Separate from workspace units.
  - **Decimal separator**: Comma / Period.
  - **Precision**: 0–6 decimals, or 0–1/256 fractional.
  - **Tolerance precision**: same range.
  - **Angular precision**: 0–6.
  - **Angular tolerance precision**: 0–6.
  - **Fit class tolerance precision**: 0–6.
- **Dual**:
  - **Show dual dimensions**: checkbox.
  - **Show dual unit**: checkbox.
  - **Dimension location**: Top / Bottom.
  - **Hole callout location**: Top / Bottom / Left / Right.
  - **Units**: any except the primary.
  - **Precision**, **Tolerance precision**.
- **Leading and trailing zeros**:
  - Length leading zeros
  - Length trailing zeros
  - Length tolerance leading zeros
  - Length tolerance trailing zeros

### 12.2 Dimensions [DOCS]
- **All dimensions**:
  - **Font**, **Text height**, **Color**
  - **Arrowhead** length (0.004–393.7 in)
  - **Arrowhead type**: none / filled / unfilled / open / oblique marks / filled dot
  - **Text justification**: Left / Center / Right / Auto
  - **Underline overridden dimensions**: checkbox, on
  - **Break dimension by default**: checkbox, on; affects new dimensions only
- **Gaps and extensions**:
  - **Text alignment**: with dimension line / horizontal
  - **Text gap**, **Geometry gap**, **Extension past line**
  - **Lock the angle of all jogged dimensions to 45 degrees**
- **Chamfer dimension**:
  - **45 degree style**: Note / Dimension
  - **45 degree content**: Angle x Length / Length x Angle / Length x Length / C Length
  - **Non 45 degree style**: Dimension / Note
  - **Non 45 degree content**: Angle x Length / Length x Angle / Length x Length
  - **Length precision**, **Angle precision**
- **Ordinate dimensions**: ANSI / ISO (default from the template).
- **Diameter and radial dimensions**: **Show dimension lines by default** (dimension line inside the circle). On for ISO templates, off for ANSI. New dimensions only.

### 12.3 Annotations [DOCS]
Common fields: Font, Text height, Color.
- **All annotations**: Arrowhead size.
- **Notes**: Font, Text height, Color; **Hide property placeholder** ("----"); **Hide property background** (gray).
- **Flag notes**: Font, Border shape, Color.
- **Callouts**: Font, Text height, Color, Text justification, Stack direction, Stack gap size.
- **Center of mass**: indicator diameter.
- **Datums**: Font, Text height, Datum size (triangle), Color.
- **Geometric tolerances**: Font, Text height, Color.
- **Surface finish symbols**: Font, Text height, Color.
- **Weld symbols**: weld **standard** (ANSI/ISO), independent of the drawing standard.
- **Bend notes**: font, height, color.
- **Hole callouts**: font, height, color, indicator.

### 12.4 Views [DOCS]
Common fields: Line (style/thickness/color), Arrowhead (size/style), Labels (font/size/format/color).
- **All views**:
  - **Projection angle**: First / Third.
  - **Visible edges**: thickness, color.
  - **View labels**: font, size, treatment, color.
- **Insert view defaults** (applied to newly inserted views; derived views copy their parent):
  - **View simplification**: None / **Auto (default)** / Absolute / Ratio to studio / Ratio to part
  - **Auto centermarks (Parts)**: on/off
  - **Auto centerlines (Parts)**: on/off
  - **Hidden lines**: Hidden / Shown
  - **Threads (Parts)**: Hidden / Shown
  - **Bend lines (Parts)**: on/off
  - **Bend notes (Parts)**: only if Bend lines is on
  - **Tangent edges**: Hidden / Solid / Phantom
  - **Scale label**: on/off
- **Hidden edges**: thickness, color, dash spacing.
- **Tangent edges**: thickness, color, dash spacing.
- **Exploded lines**: thickness, color, dash spacing.
- **Section views**:
  - **Hatch pattern** (type + pattern)
  - **Hatches** (line size, color)
  - **Cutting lines** (size, color)
  - **Arrowhead** (size; filled/unfilled/open)
  - **Cutting line label** (font, size, treatment, color)
- **Detail views**:
  - **Detail profile lines** (size, color, style Phantom/Dashed/Solid)
  - **Arrowhead**
  - **Detail profile label**
  - **Detail profile style**: In line / Connected / Leader
  - **Show detail view profile**
  - **Clip border to geometry**
- **Auxiliary views**:
  - **Show view reference indicator**
  - **View reference format**: line style ↔ arrow style. Arrow style can be used as a reference for angular dimensions.
  - **Viewing lines** (thickness, color)
  - **Arrowhead**
  - **Viewing line label**
  - **View label prefix**
  - **View label style**: "A-A" / "A"
  - **View label suffix**
- **Break views**: break line thickness, color.
- **Cosmetic threads**: thread style, thickness, color (internal/external); ANSI dash length scale.
- **Flat pattern views**:
  - All bend lines: thickness
  - Up bend lines and Down bend lines: style Solid/Dashed/**Center (default)**, color
  - Bend tangent edges: thickness, color
  - Counterbore/Countersink up and down: **Solid (default)**/Dashed/Center, color
  - Form tear: Solid default
  - Form up and Form down: Center default
  - Form centermarks: Solid default

### 12.5 Construction geometry [DOCS]
- **Created in drawing**:
  - **Virtual sharps**: Centermark / Edge extension
  - **Sketch geometry**: line size, color
  - **Sketch driven dimensions color**
- **Part Studio sketches**: format of Part Studio sketches shown in views (line, color).
- **Centermarks**: line color and thickness, **mark size**, **gap size**, **Extended lines**; circular: show bolt circle, radial lines, pattern centermark.
- **Centerlines**: line size, color, dash spacing.
- Sketch point appearance and size (referenced from the view context menu).

### 12.6 Formats [DOCS]
- **Locked**: checkbox. **Default: Locked** for new drawings. Locked means the title block, border and zones can't be selected; field values still update.
- **Border frame**: color, thickness.
- **Border zone**: label color, thickness.
- **Title block**: line and content color, thickness.

### 12.7 Tables [DOCS]
- **All tables**: Horizontal padding (default **0.06 in**), Vertical padding (default **0.06 in**).
- **BOM / Custom / Cut list tables**: Line thickness, Font, Header row text (size/treatment/color), Content row text.
- **Hole tables**: the same, plus **Indicator style** ANSI/ISO, **Indicator arrowhead** size, **Indicator lines** (thickness, color), **Tags** (font, size, bold/italic/none, color).
- **Revision tables**: Line thickness, Font, Title row text, Header row text, Content row text, **Revision callout** (shape, number of characters, font, size, color, treatment).
- **Tables** (generic): Line thickness, Font, Title/Header/Content row text.

### 12.8 Inspection (out of scope; list only) [DOCS]
- Item parsing: Parameter (default) / Row
- Font, Text, Border shape, Border size, Border color
- Solid fill + color
- Automatic inspection items: Exclude callouts

---

## 13. Styles panel [DOCS]
- A right-side panel for **per-entity overrides** of the Properties defaults.
- Select the entity first, or open the panel first and then select.
- **Revert to drawing properties** icon at the bottom.
- **Multi-select** across types shows only the shared fields (e.g. note + dimension = Font, Text height, Color, Arrowhead). If any selected entity is unsupported, the panel is unavailable.

**Fields by entity class**:
- **Dimension styles**:
  - Font, Text height, Color
  - Arrowhead: thickness and type, **per arrow**; a left and right arrow are shown when there are two
  - Text justification (L/C/R/Auto)
  - Text alignment (with line/horizontal)
  - Text gap, Geometry gap, Extension past line
  - **Show first extension line**, **Show second extension line** (on by default; hovering the checkbox turns the line blue)
  - Break dimension
  - Show dimension line
- **Annotation styles**: Font, Text height, Color, Arrowhead, Text justification, Text gap, **X degree content** (chamfer), **Stack direction** (Auto/Right/Left/Up/Down; callouts), **Stack gap size** (mm), **Datum size**.
- **Hatch styles**: Hatch pattern (type/style/size, 3 dropdowns), Color, Scale, Angle.
- **Sketch geometry styles**: Linestyle (Solid/Phantom/Center/Dashed/Dotted), Pitch scale (0.01–5.00), Thickness, Color.

**Availability matrix, dimensions** (✓ = available):
| Style | Chamfer | Max/min | 2pt linear | Pt-line | Line-line | Line-line ang. | 3pt ang. | Radial | Diameter | Foreshorten | Ordinate |
|---|---|---|---|---|---|---|---|---|---|---|---|
| Font/Text height/Color/Text justification/Text gap/Break dimension | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |
| Arrowhead, Text alignment | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | – |
| Geometry gap, Extension past line, Show 2nd ext. line | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | – | – | – | – |
| Show first extension line | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | – | – | ✓ | – |
| Show dimension line | – | – | – | – | – | – | – | ✓ | ✓ | – | – |

**Availability matrix, annotations**:
| Style | Chamfer | Hole/thread callout | Datum | GTOL | Surface finish | Weld | Note | Callout |
|---|---|---|---|---|---|---|---|---|
| Font/Text height/Color | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |
| Arrowhead | ✓ | ✓ | – | ✓ | ✓ | ✓ | ✓ | ✓ |
| Text justification | ✓ | ✓ | – | – | – | – | – | ✓ |
| Text gap, X degree content | ✓ | – | – | – | – | – | – | – |
| Stack direction / gap | – | – | – | – | – | – | – | ✓ |
| Datum size | – | – | ✓ | – | – | – | – | – |

Sketch geometry (Line, circles, rectangles, Spline, Tangent arc): Linestyle, Thickness, Color.

---

## 14. Templates (brief) [DOCS]
- A template (.DWT) holds the format (title block, border, zones), properties and styles.
- **Customize a public template**:
  1. New document, then + > Create Drawing, Onshape filter, standard, size, OK.
  2. Edit it.
  3. Tab > Export, Format **DWT**, Options **Store file in a new tab**, Export.
  4. Optionally download the .dwt tab.
  5. Use it later via the **Created by me** / **Shared with me** filters (or + > Import a .dwt).
- **DWT export rules**:
  - Only the **first and second** sheets are exported (first = primary template, second = continuation template). [BUNDLE] "Primary template is the first sheet." / "Continuation template is the first or, if it exists, second sheet."
  - All views are deleted; only non-view geometry and text remain.
  - DWT version is 2013.
  - The Drawing layer is excluded.
- **Legacy CAD format as a template**:
  1. Custom template with no border and no title block.
  2. Import the DWG/DXF, then **Insert DXF/DWG**.
  3. Replace fonts. Add tables, an image logo, and drawing-property fields.
  4. **Move to** the Title block / Border frame / Border zones layers.
  5. Export as DWT.
- **Update properties from a template**: bottom of the Properties panel. **Replace format per sheet**: Sheet properties > Select a DWT.

---

## 15. Export, print, import

### 15.1 Export Drawing dialog [DOCS] [BUNDLE]
- Opened from tab right-click > **Export**.
- **Batch export**: in the Tab manager, Ctrl+click several drawings, then right-click > **Export drawings** (produces a zip).
- **Formats**: **PDF, DWG, DXF, DWT, SVG, PNG, JPEG**, plus CSV-type formats "Inspection list" and "Drawing definition list" (out of scope).

| Field | Applies to | Options |
|---|---|---|
| File name | all | text; export rules can auto-name ([BUNDLE] warns when a rule name is truncated) |
| Format | all | list above ([BUNDLE] "Export format") |
| Version | DWG/DXF/DWT | file-format version (e.g. 2013; strike-through text needs 2013) |
| Sheets | all | All sheets / Current sheet / **Custom sheets** with a range like `1-3, 5, 7-9` ([BUNDLE] errors: "Invalid sheet range, use e.g. 1-3, 5, 7-9", "Sheet range cannot be empty", "Sheet range is out of bounds"). Raster/SVG: "Selected format only supports exporting the active sheet." [BUNDLE] |
| Overridden dimensions | all | show / hide underline |
| Text | PDF | **Normal** / **Selectable** |
| Text | DWG/DXF/DWT | **Export as text** / **Explode into polylines (notes)** / **Explode all into polylines** (notes, table text, dimensions, annotations) |
| Color | PDF | Color / **Black and white** / **Grayscale** |
| Units | DWG/DXF | **Use drawing units** or a chosen unit |
| Options (destination) | all | Download / **Store file in a new tab** / "Download and store file in a new tab" / "Email with file download link" / cloud drive "Store file in __drive__" [BUNDLE] |
| Apply export rules to contents | batch | checkbox, plus a "View export rules" link |
| Export splines as polylines | all except PDF | checkbox |
| Set z-height to zero and normals to positive | DWG/DWT/DXF | checkbox |
| Hide inspection items | all | inspection items are shown by default |
| Exclude off-sheet content | DWG/DWT/DXF | checkbox |

- After export: a blue banner "[name] is being prepared for export. You will receive a notification...", then a notification when done.
- [BUNDLE] warning: "In the exported file, dimensions added to a view with a scale different from the active sheet may be out of scale."
- Format layers export as `TITLE_BLOCK` / `BORDER_FRAME` / `BORDER_ZONES`. Placeholders "----" are dropped by default. The view render mode applies to export.

### 15.2 Print [DOCS]
Document menu (top left) > **Print...** opens a print-friendly browser tab; print or save from its controls.

### 15.3 Import a drawing [DOCS]
- **Documents page** > Create > **Import files...**: makes a new document named after the file.
- **In a document**: + > **Import...**: makes a file tab **plus** a drawing tab, both named after the file.
- DWG/DXF are translated into an Onshape drawing. Model space background color is set under Preferences > Drawings.
- Import errors are listed in a message ([BUNDLE] "The DXF/DWG imported with the following errors:").
- Imported drawings are not associative to any model.

---

## 16. Out-of-scope stubs (one line each)
- **MBD status panel**: right-side icon. Lists model annotations not shown in any view (Model: Current sheet / All Models / Part; Datums and Characteristics tables). Icon badge shows the missing count, or a checkmark when nothing is missing. Deleted annotations go gray and come back via **Show**. [DOCS]
- **Inspection item / Inspection table / Hide inspection items**: numbered balloons placed manually or automatically (scope: drawing/sheet/view), with Reorder, Remove and CSV export. [DOCS]
- **Revision table / revision callouts**: release management. [DOCS]
- **Watermarks**: company-level ("Edit watermarks"). [BUNDLE]

---

## 17. Key implementation notes (clone checklist)
1. **Associativity model.**
   - Annotations reference model topology through the view: edge, vertex, circle center, silhouette, thread line.
   - After an update, a lost reference makes the annotation **dangling**: a red grip, fixed by dragging it onto a new reference.
   - Editing a dimension's value makes it **overridden** and underlined; clearing the value restores it.
   - Snapping and inference never create associations between annotations (except explicit groups).
2. **Parent/child view graph.**
   - Projected, aux, section, detail and aligned views keep a parent link: scale = Parent by default, alignment on the projection axis, settings inherited (Insert view defaults, simplification, MBD visibility, shading for detail).
   - Rotation is only possible when the view has no children and no alignment.
   - Section/Detail labels auto-increment A–Y, skipping I O Q S X Z.
   - Breaks propagate across related views. Angular sections and breaks are mutually exclusive.
3. **Out-of-date state.**
   - Track source-model change, then light up Update (Ctrl+Q). Update re-projects views only and auto-runs before a new view is placed.
   - Pinned or versioned references never go stale.
4. **Defaults cascade**: Template/DWT, then Drawing Properties (8 tabs), then Styles panel per-entity overrides (Revert button), then per-dimension palette (precision/units "(Drawing)" link).
   - Note text height edits **become the new default**.
   - Format painter copies only property/style-level formatting.
5. **Layers and format.**
   - Four layers: Drawing, Title block, Border frame, Border zones. Format is locked by default.
   - Changing sheet size redraws the border and zones, leaves the title block alone, and never touches Drawing-layer content.
   - DWT export keeps sheets 1–2 and drops views and the Drawing layer.
6. **Placement UX.**
   - Every placement tool shows a live preview with orange snaps, red inference points and pink H/V inference lines. Shift disables inference.
   - Alt+drag duplicates.
   - Tools stay active for repeated placement until ✓ or Esc (datum, GTOL, surface finish, projected view).
