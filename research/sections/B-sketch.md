# B. Sketch mode

Scope: the 2D sketch editor inside a Part Studio, covering entering/exiting, the Sketch dialog, the toolbar, every geometry/edit tool, constraints, dimensions, inferencing, solver/drag behavior, diagnostics (Constraint manager, Profile inspector), visual conventions and shortcuts.
Tags: [DOCS] = Onshape Help (Oct 2026), [FS] = FeatureScript std v3095, [BUNDLE] = web client JS bundle (appearance constants, CSS tokens, i18n, templates). Nothing was observed live. Where something is inferred, it is marked *(inferred)*.

---

## 1. Entering, editing and exiting a sketch

### 1.1 Entry points
- Feature toolbar **Sketch** button. Opens the Sketch dialog with the "Sketch plane" field active, prompting "Select a sketch plane" [DOCS][BUNDLE i18n].
- Shortcut **Shift+S** opens the Sketch feature with the prompt "Select a sketch plane" (Part Studio scope) [DOCS].
- Context menu on a selected plane/planar face: **New sketch…** (shown with the sketch icon) begins a sketch on that plane [DOCS][BUNDLE label `newSketch = "New sketch…"`].
- Edit an existing sketch: double-click it in the Feature list, or Feature-list context menu > **Edit** [DOCS].
- Command action ID: `CREATE_FEATURE_SKETCH=createFeatureSketch` [BUNDLE].
- Pre-selection: if a valid plane/face/mate connector is selected when Sketch opens, it fills the Sketch plane field *(inferred, standard feature pre-selection behavior)*.

### 1.2 Sketch dialog (feature dialog for a sketch)
Feature Type Name "Sketch", `UIHint.CONTROL_VISIBILITY` [FS]. Standard feature-dialog header: green ✓ (accept), red ✗ (cancel) [DOCS].

| # | Control | Type / default | Notes |
|---|---|---|---|
| 1 | **Sketch plane** | Query, `MaxNumberOfPicks: 1` | Filter: `(GeometryType.PLANE && AllowFlattenedGeometry.NO) \|\| (SheetMetalDefinitionEntityType.FACE && AllowFlattenedGeometry.YES && GeometryType.PLANE) \|\| BodyType.MATE_CONNECTOR` [FS]. Accepts a default/construction plane, a planar part face, a sheet-metal flat-pattern planar definition face, or a mate connector (explicit or implicit) [DOCS][FS]. |
| 1a | Mate connector icon (inside the plane field) | button | Click it to show implicit mate connectors when hovering sketches/parts (circles/squares at vertices, centroids etc.) and pick one as the plane. The sketch then uses the mate connector's coordinate system. Once a mate connector is chosen, clicking the icon in the field opens a dialog to edit that mate connector [DOCS]. |
| 2 | **Disable imprinting** | boolean checkbox, `UIHint.REMEMBER_PREVIOUS_VALUE`, default false | Shown only when the parameter exists (`if (value.disableImprinting != undefined)`) [FS]. Docs: shown when sketching on the face of an existing part. Unchecked means the face's edges are imprinted into the sketch and split its regions. Checked means the sketch is not split by the face boundary [DOCS]. |
| 3 | **Show constraints** | checkbox, unchecked by default (`id="show-constraints"`) | Shows all constraint glyphs permanently. When off, glyphs show only for the hovered entity [DOCS][BUNDLE template]. |
| 4 | **Show expressions** | checkbox, unchecked (`id="show-expressions"`) | Shows each dimension's expression text above its value, truncated to 70 chars. **Not remembered**: it resets to unchecked whenever the dialog is closed and reopened [DOCS][BUNDLE]. |
| 5 | **Show errors** | checkbox, **checked** by default (`id="show-overdefined-constraints"`) | Shows over-defined/conflicting constraints. Older docs call it "Show overdefined" [BUNDLE template][DOCS mention "Show errors"]. |
| 6 | Preview controls container | | Standard `_previewControlContainer` partial (feature preview toggle) [BUNDLE]. |
| 7 | **Show sketch diagnostics tools** button | icon `sketch-analysis-tools`, tooltip "Show sketch diagnostics tools" | Dropdown items: **Profile inspector…** / **Turn profile inspector off**, **Constraint manager…** / **Turn constraint manager off** [BUNDLE][DOCS]. |

- "Onshape remembers your selected options in the Sketch dialog and preserves them the next time you open it" [DOCS]. Show expressions is the exception (see row 4).
- No "Lock" field appears in the docs, FS, or bundle template for the sketch dialog. Do not implement one.
- The FS precondition has only `sketchPlane` and `disableImprinting`. Show constraints/expressions/errors are client-only view options, not feature parameters [FS][BUNDLE].

### 1.3 Sketch coordinate system [FS `newSketch`]
- Plane or face: the plane is canonically re-aligned (`alignCanonically`) and its origin becomes the **projection of the world origin onto the plane**.
- Mate connector: the sketch uses the mate connector's own coordinate system (origin and axes) unchanged.
- No valid plane gives error `SKETCH_NO_PLANE` and falls back to the XY plane. A mesh face gives `MESH_NOT_SUPPORTED`. A flattened sheet-metal face with no active sheet-metal model gives `SHEET_METAL_ACTIVE_MODEL_REQUIRED`.
- `newSketchOnPlane` (FS only) takes an arbitrary Plane object.

### 1.4 View orientation on entry
- **Orient normal to sketch on edit**: a toggle item in the *Camera and render options* dropdown, shown with a checkmark when enabled. When on, entering a sketch orients the view normal to the sketch plane. The setting persists across sessions. Turning it off returns to the previous view [DOCS].
- Manual: **n** = view normal to the hovered/preselected plane or face. Pressing n again flips to the inverse normal [DOCS]. Context menu in a sketch: **View normal to sketch plane** (`normalToSketch`) [BUNDLE]. Feature-list sketch context menu: **View normal to** [DOCS].
- Camera snap animation is 125 ms (`CAMERA_SNAP_ANIMATION_TIME_MS`) [BUNDLE].

### 1.5 Accepting and cancelling
- ✓ accepts the sketch and closes the dialog. **Enter** commits. **Shift+Enter** commits and opens another dialog of the same command (a new sketch) [DOCS keyboard general].
- ✗ (or Esc with no tool active) cancels and discards every action taken while the dialog was open. A message bubble then shows a **Restore** link that reverses the cancel [DOCS]. `SKETCH_RESTORE_TIMEOUT=3` (likely seconds the bubble/restore is offered) [BUNDLE].
- Choosing **Extrude** (Shift+E) or **Revolve** (Shift+W) from the sketch toolbar accepts the open sketch, closes it, and opens the Extrude/Revolve dialog with **all sketch regions pre-selected** [DOCS].
- While loading: "The sketch is being prepared. Edits can be made when it is ready." [BUNDLE i18n].
- The whole sketch session is one undoable feature edit. Insert DXF/DWG is recorded on the document Undo/Redo stack [DOCS].

### 1.6 Regions, imprinting and profiles
- Closed loops of non-construction sketch curves form **sketch regions** (faces of a sketch sheet body), drawn shaded. Region fill uses token `--os-sketch-face-color` = #9aa5af at 20 % alpha (light). Show-through (occluded) fill is `rgba(154,165,175,0.1)`. Dark theme uses noreaster-30 at 20 %/10 % [BUNDLE].
- Body appearance constants: `SKETCH_BODY_OPACITY=.2`, `SHOW_THROUGH_SKETCH_BODY_OPACITY=.1`, ambient rgb(128,128,128). Region faces are offset in depth (`SKETCH_FACES_Z_OFFSET=21`, edges 12, points 3) so they draw over coplanar part faces [BUNDLE].
- Regions are the selectable profiles for Extrude/Revolve etc. ("Faces and sketch regions to extrude", filter `SketchObject.YES && EntityType.FACE`) [FS]. Sketch curves can also be picked directly as "Sketch curves to extrude" (must be `ConstructionObject.NO`) [FS].
- Imprinting: when the sketch plane is a part face and imprinting is on, the face's boundary edges become part of the sketch and split regions. Example: circles overlapping a rectangle face are split into separate regions [DOCS].
- Construction geometry never bounds regions and is not used by features [DOCS].

### 1.7 Sketch in the Feature list
- A sketch is stored parametrically as its own Feature-list entity. The default name is "Sketch N" *(inferred: standard naming; bundle has "New sketch added")* [DOCS][BUNDLE].
- Feature-list context menu for a sketch [DOCS Context Menus]: **Rename**, **Edit**, **Copy sketch**, **Show dimensions** (shows the sketch's dimensions until you click elsewhere), **Add selection to folder**, **Show/Hide**, **Create Drawing of Sketch**, **Export as DXF/DWG**, **Suppress**, **Add comment**, **Zoom to selection**, **View normal to**, **Clear selection**, **Roll to here**, **Delete**. Also **Paste into sketch** when a sketch is on the clipboard [DOCS].
- Inactive sketches (not being edited) stay visible in the graphics area in the inactive style (§5.3). **Shift+H** toggles hide/show of all sketches (`HIDE_SHOW_SKETCHES`) [DOCS][BUNDLE]. **Shift+P** hides all construction geometry (`HIDE_ALL_CONSTRUCTION`) [DOCS].
- Comments: right-click the sketch in the Feature list, or a sketch dimension in the graphics area, > **Add comment**. There is an option to receive email notifications [DOCS].

### 1.8 Copy / paste / derive / transform
- Copy: select the sketch in the Feature list > context **Copy sketch**. Inside an active sketch the bundle label is **Copy sketch entities** [DOCS][BUNDLE].
- Paste within a Part Studio: open a sketch > context **Paste into sketch**, or right-click the target sketch in the Feature list > **Paste into sketch** [DOCS].
- Paste into another Part Studio: select an existing sketch, or create a new sketch and pick its plane, then right-click > **Paste sketch entities** (`pasteActiveSketch`). **Paste sketch** (`pasteSketch`) appears when a plane is selected with no feature being edited and the selection is valid for `newSketch.sketchPlane` [DOCS][BUNDLE]. Pasting across differing versions may error [DOCS].
- After pasting (mobile docs), drag or tap to position the pasted sketch [DOCS].
- **Derived** feature can import sketches from another Part Studio. They are placed on their original plane and update when the source changes [DOCS].
- Rigid transform: context menu **Transform sketch entities** (§2.5.35).

---

## 2. Sketch toolbar

### 2.1 General toolbar behavior
- The Sketch toolbar replaces/extends the Feature toolbar while a Sketch dialog is open [DOCS].
- A small ▾ arrow beside an icon means a **dropdown group**. The group button shows the icon of the **last tool used from that group** [DOCS].
- **Search tools** (Alt/⌥+C) searches sketch tools as well [DOCS].
- **Sketch shortcut toolbar**: press **S** in an active sketch to open a mini toolbar at the cursor. It is customizable under My account > Preferences > Shortcut toolbars [DOCS]. Command `OPEN_MINI_TOOLBAR` [BUNDLE].
- **Esc** exits the current tool. Pressing a tool's shortcut again also exits it ("Selects or exits the … tool") [DOCS]. All sketch tool shortcuts require a sketch plane to have been selected [DOCS].
- Extrude and Revolve are reachable from the sketch toolbar (§1.5) [DOCS].
- All shortcuts are user-customizable in Preferences > Keyboard shortcuts [DOCS].

### 2.2 Toolbar inventory and grouping
The exact on-screen order is server-driven and not in the docs or bundle. The bundle's sketch command enum lists the tools with default hotkeys first (LINE … POINT). It then lists the rest in an order that matches the dropdown groupings described in the docs (MIDPOINT_LINE, ALIGNED_RECTANGLE, THREE_POINT_CIRCLE, ELLIPSE, TANGENT_ARC, CENTER_POINT_ARC, ELLIPTICAL_ARC, CONIC, INSCRIBED_POLYGON, CIRCUMSCRIBED_POLYGON, SPLINE, BEZIER, SPLINE_CONTROL_POINT, TEXT, INTERSECTION, SPLIT, SLOT, MIRROR, LINEAR_PATTERN_SKETCH, CIRCULAR_PATTERN_SKETCH, TRANSFORM, INSERT_DXF_DWG, INSERT_IMAGE, TOGGLE_SKETCH_PROFILE_INSPECTOR, CHAMFER, TOGGLE_SKETCH_CONSTRAINT_MANAGER, CURVATURE_G3). The last few look like later additions [BUNDLE].

Recommended toolbar for the clone. Groupings are confirmed by docs where noted; others are *(inferred)* from the enum adjacency:

| Pos | Group button (dropdown members in order) | Command IDs [BUNDLE] | Default shortcut [DOCS] | Grouping source |
|---|---|---|---|---|
| 1 | **Line** ▾ (Line, Midpoint line) | `line`, `midpointLine` | l / – | DOCS ("dropdown next to the Line tool") |
| 2 | **Rectangle** ▾ (Corner rectangle, Center point rectangle, Aligned rectangle) | `cornerRectangle`, `centerPointRectangle`, `alignedRectangle` | g / r / – | inferred |
| 3 | **Circle** ▾ (Center point circle, 3 point circle, Ellipse) | `centerPointCircle`, `threePointCircle`, `ellipse` | c / – / – | inferred |
| 4 | **Arc** ▾ (3 point arc, Tangent arc, Center point arc, Elliptical arc, Conic) | `threePointArc`, `tangentArc`, `centerPointArc`, `ellipticalArc`, `conic` | a / – / – / – / – | inferred |
| 5 | **Polygon** ▾ (Inscribed polygon, Circumscribed polygon) | `inscribedPolygon`, `circumscribedPolygon` | – | DOCS glossary "Polygon" covers both |
| 6 | **Spline** ▾ (Spline, Bezier, Spline control point) | `spline`, `bezier`, `splineControlPoint` | – | inferred |
| 7 | **Point** | `point` | Shift+S | |
| 8 | **Construction** (toggle) | `toggleConstruction` | q | |
| 9 | **Text** | `text` | – | |
| 10 | **Fillet** ▾ (Sketch fillet, Sketch chamfer) | `fillet`, `sketchChamfer` | Shift+F / – | inferred |
| 11 | **Trim** | `trim` | m | |
| 12 | **Extend** | `extend` | x | |
| 13 | **Split** | `splitSketch` | – | |
| 14 | **Offset** | `offset` | o | |
| 15 | **Use** ▾ (Use, Intersection) | `use`, `intersection` | u / – | DOCS ("Intersection … from the Use icon dropdown") |
| 16 | **Mirror** | `mirrorSketch` | – | |
| 17 | **Pattern** ▾ (Linear pattern, Circular pattern) | `linearPatternSketch`, `circularPatternSketch` | – | inferred |
| 18 | **Transform** | `transformSketch` | – | |
| 19 | **Slot** | `slot` | – | |
| 20 | **Insert** ▾ (Insert image, Insert DXF or DWG) | `insertImage`, `insertDxfDwg` | – | inferred |
| 21 | **Dimension** | `dimension` | d | |
| 22 | **Constraints** (Coincident, Concentric, Parallel, Tangent, Horizontal, Vertical, Perpendicular, Equal, Midpoint, Normal, Pierce, Symmetric, Fix, Curvature [+ Curvature G3]) | see §3 | see §3 | DOCS lists this order for the "toolbar constraints menu" |
| – | Diagnostics (in Sketch dialog, not toolbar): Profile inspector, Constraint manager | `toggleSketchProfileInspector`, `toggleSketchConstraintManager` | unassigned | DOCS |

- Mobile docs mention toolbar categories and an overflow menu for "additional sketch tools", so a narrow toolbar may overflow [DOCS].
- The constraints menu in the docs image appears as a row of constraint icons on the toolbar. Docs call it "the toolbar's constraint menu" [DOCS].

### 2.3 Common behaviors shared by geometry tools
- **Click-click vs click-drag**: most tools accept either discrete clicks or press-drag-release, where the release acts as the next click (Line, rectangles, circles, 3-point arc, tangent arc) [DOCS]. Drag threshold: `LEFT_MOUSE_DRAG_DISTANCE_TOLERANCE=2` px and `LEFT_MOUSE_DRAG_TIME_TOLERANCE=50` ms [BUNDLE].
- **On-the-fly dimensions**: right after an entity is created, a suggested dimension box appears with the current value. Type a value or expression and press Enter to create a driving dimension. If you keep sketching instead, the geometry stays undimensioned [DOCS].
  - Multiple boxes (rectangle length/width, ellipse axes): fill the first, press Enter, then fill the second. **Alt+←/→** toggles between boxes [DOCS].
  - Variables: while sketching type `=#name` (e.g. `=#d`). Typing `#` without `=` is the view hotkey (left view in docs wording). In an existing dimension's edit field, `#name` works without `=` [DOCS].
  - Typing `#` (in an edit field) opens variable autofill. The list includes **New variable**, which opens the Create variable dialog (Feature (default) or Configuration). Arrow keys/Enter pick, Esc closes. Only variables above the feature in the list are offered [DOCS Variable].
- **Construction mode**: with Construction toggled on (q) before choosing a tool, new geometry is created as construction [DOCS].
- **Inferencing** runs during every placement click and drag (§6). Holding **Shift** suppresses inferences [DOCS].
- **Preview rendering** of geometry being created: `SKETCH_PREVIEW_COLOR=rgba(68,156,205,1)` *(inferred use)* [BUNDLE].
- Ending chains: **Esc** ends the current chain or tool. Double-click ends Spline and Bezier [DOCS].
- Snapping to points of existing geometry creates coincident constraints automatically (§6) [DOCS].

### 2.4 Geometry creation tools (detailed)

#### 2.4.1 Line (`l`)
- Click a start point, then click to end the segment. **Chaining continues**: each click ends the current segment and starts the next one at that point, until Esc or a double-click *(inferred end gesture)* [DOCS].
- Alternative: press at the start, drag, release to end one segment [DOCS].
- After each segment, a length box appears for an optional driving length dimension (or `=#var`) [DOCS].
- **Line ↔ Tangent arc switching** while chaining [DOCS]:
  - Press **Shift+A** (`LINE_TANGENT_ARC_SWITCH`) to switch to Tangent arc from the current endpoint, OR
  - move the cursor back near the last endpoint until the cursor icon changes from line to tangent arc.
  - Click to set the arc end. The tool then **automatically reverts to Line**, continuing from the arc end. Shift+A again starts another tangent arc.
- Auto constraints: coincident between chained segments *(inferred, standard)*, plus inferred H/V/parallel/perpendicular/coincident snaps (§6).

#### 2.4.2 Midpoint line (no default shortcut)
- In the Line dropdown. Click to place the **midpoint**, then click to place one end. The line extends symmetrically [DOCS].
- Mobile can only view/edit midpoint lines created on desktop [DOCS].
- Auto constraint: midpoint relationship between the center point and the line *(inferred)*.

#### 2.4.3 Corner rectangle (`g`)
- Click the first corner, then click the diagonal corner. Or press-drag-release [DOCS].
- Then length box, Enter, width box, Enter (optional) [DOCS].
- **Hold Alt while sketching** to make it a square (equal constraint on two sides) [DOCS].
- Auto constraints: FS `skRectangle` creates 4 lines (left/right/top/bottom), 4 corner **coincident**, 2 **parallel** (left∥right, top∥bottom), 1 **vertical** (left), 1 **horizontal** (top) [FS]. The interactive tool should produce the same set *(inferred)*.

#### 2.4.4 Center point rectangle (`r`)
- Click the center, then click a corner. Or drag center→corner [DOCS].
- Length then width boxes. Alt = square [DOCS].
- Auto constraints as for the corner rectangle, plus the center point tied to the diagonals' midpoint *(inferred; a construction diagonal or midpoint construct is typical)*.

#### 2.4.5 Aligned rectangle (no shortcut)
- Click the start of the first side, then click the end of that side (any angle). Move perpendicular to it and click at the desired width [DOCS].
- Length then width boxes. Alt = square [DOCS].
- Auto constraints: coincident corners, parallel/perpendicular sides (no H/V) *(inferred)*.

#### 2.4.6 Center point circle (`c`)
- Click the center, then click a point on the circumference (sets radius). Or press at center, drag, release [DOCS].
- A **diameter** box appears for optional entry [DOCS].

#### 2.4.7 3 point circle (no shortcut)
- Click point 1, click point 2, click point 3 (defines the circle). Or drag 1→2 and click 3 [DOCS].
- A diameter box appears [DOCS].

#### 2.4.8 Ellipse (no shortcut)
- Click the center. Move outward and click to set the **primary axis radius**, which also sets the axis direction (the diameter is symmetric about the center) [DOCS].
- Move and click to set the **secondary axis** radius, perpendicular to the primary [DOCS].
- Primary diameter box, Enter, secondary diameter box, Enter [DOCS].
- Whichever axis is larger is the major axis. Dimensions are MAJOR_DIAMETER / MINOR_DIAMETER constraint types [DOCS][FS].
- Ellipse quadrant points support inference/snap (QUADRANT constraint, §3.3) [DOCS].
- FS entity: `skEllipse {center, majorRadius, minorRadius, majorAxis}` [FS]. Error `SKETCH_ELLIPSE_ZERO_AXIS`, `_TOO_SMALL` [FS].

#### 2.4.9 3 point arc (`a`)
- Click the start point, click the end point, then click the third point, which sets the radius/bulge. Or drag start→end and click the third point [DOCS].
- A radius box appears [DOCS].
- **Semicircle**: after placing the endpoints, move until the endpoints and midpoint align and the inference shows. Click to make an exact semicircle [DOCS].
- FS `skArc {start, mid, end}` [FS].

#### 2.4.10 Tangent arc (no default shortcut, Shift+A from Line)
- Click to start (must be at the endpoint of an existing curve for tangency), then click to end. Or drag start→end [DOCS].
- A radius box appears [DOCS].
- Auto constraints: coincident + **tangent** at the start [DOCS implicit]. Error `SKETCH_TANGENT_ARC_INVALID_START` if the start is not a valid curve end [FS].

#### 2.4.11 Center point arc (no shortcut)
- Click the center, click the start point (sets radius), then click the end point (sweep follows cursor) [DOCS].
- A radius box appears [DOCS].

#### 2.4.12 Elliptical arc (no shortcut)
- Click the center of a **temporary construction ellipse**. Click to set the primary axis radius and direction [DOCS].
- Moving the cursor adjusts the secondary axis. The click both fixes the secondary radius and **places the arc start point** [DOCS].
- Move along the construction ellipse and click the end point. The temporary ellipse is deleted and the arc remains [DOCS].
- Primary and secondary diameter boxes appear [DOCS].
- Start and end can snap to the major/minor axis endpoints or quadrant points. A yellow inference line shows the snap [DOCS].
- FS `skEllipticalArc {center, majorAxis, majorRadius, minorRadius, startParameter, endParameter}` [FS].

#### 2.4.13 Conic (no shortcut)
- Click the start point, then click the end point. A **rho** box is offered immediately (optional) [DOCS].
- Click the control point (the apex of the tangent lines). If rho was not entered yet, the rho dimension box opens automatically [DOCS].
- Rho semantics: rho < 0.5 gives an elliptical arc, rho = 0.5 a parabola, rho > 0.5 a hyperbola. Valid range is 0 < rho < 1. FS definition: rho = x/y, where x = distance from the control point to the start–end chord and y = the max distance from the conic to the chord. Wording in the FS comment is as written [DOCS][FS].
- Rho is unitless and stored as a RHO constraint. `fixedRho` exists in FS [FS].
- Curvature constraint applies to conics (§3) [DOCS].

#### 2.4.14 Inscribed polygon (no shortcut)
Docs label: "polygon on the outside of the drawn circle". The docs' inscribed/circumscribed wording is swapped relative to standard geometry. Copy the docs labels as-is [DOCS].
- Click the center, then move to set the circumference (a **construction** circle is drawn) [DOCS].
- Side count, option A: type the number of sides on the keyboard first, then click to lock the circumference. A sides value field appears, where you can type or drag again. Click or press Enter [DOCS].
- Option B: click to lock the circumference. A sides field appears. Type, or **drag toward the polygon to decrease** / **away to increase** the side count. Click or press Enter [DOCS].
- Sides: **min 3, max 50** [DOCS]. FS: integer > 2 (`SKETCH_POLYGON_BAD_SIDE_COUNT`, `SKETCH_POLYGON_ZERO_RADIUS_FAIL`) [FS].
- After the sides are set, a **diameter** box for the construction circle appears [DOCS].
- Result: a regular polygon of line segments plus a construction circle. Vertices or edges are tied to the circle and segments are equal *(inferred)*. FS `skRegularPolygon {center, firstVertex, sides}` builds a polyline [FS].

#### 2.4.15 Circumscribed polygon (no shortcut)
- Identical interaction. Docs: "polygon on the inside of the construction circle". The circle is again construction. Same 3–50 sides, side-count entry and diameter box [DOCS].

#### 2.4.16 Spline (no shortcut)
- Click to start, click to add each fit point, **double-click to end** [DOCS].
- Right-click context menu while creating > **Confirm spline**. It finishes the spline at the point *before* the cursor (one point behind) [DOCS].
- Close a spline only by connecting the last point to the **first** point [DOCS].
- After creation: drag any fit point to reshape. **Tangent handles** (white points) at the ends can slide along or pull away from the spline to change tangent direction/magnitude. Handles **cannot be deleted** [DOCS]. Errors: `SKETCH_CANNOT_DELETE_SPLINE_HANDLES`, and trim/split/extend/pattern/fillet/mirror/transform cannot act on handles [FS].
- Spline points and handles can be dimensioned and constrained [DOCS].
- Context menu: **Insert spline points**, **Insert spline handles** (`insertSplinePoints`, `insertSplineHandles`) [BUNDLE].
- **Curvature color map** context command visualizes curvature [DOCS]. Command `CURVATURE_COLOR_MAP`. Sketch inflection points can be shown ("Sketch inflection points") [BUNDLE i18n].
- Rendering: spline handle point size 10 px (active), 0 (inactive, so hidden). Handle line rgb(128,128,128), width 1 [BUNDLE].
- FS: `skFitSpline`, `skInterpolatedSpline {splinePointCount}` [FS].

#### 2.4.17 Bezier (no shortcut)
- Click to start, click to add each **control point**, double-click to end. The curve follows the control polygon [DOCS].
- Right-click while creating > **Escape bezier** exits and **removes** the in-progress curve [DOCS].
- Cannot be closed (first control point ≠ last). Use Spline for closed loops [DOCS]. A 2-point Bezier is rejected (`SKETCH_CANNOT_MAKE_2_POINTS_BEZIER`) [FS].
- Edit by dragging control points (white) with the tool deselected [DOCS].
- **Degree**: shown as an editable label. Double-click it, type a value, press Enter. Max degree **15** ("The maximum bezier degree (15) has been reached.") [DOCS][BUNDLE]. Stored as a BEZIER_DEGREE constraint [FS].
- Trim keeps the remaining piece with the original degree and control point count. Split makes two curves, each with the same degree and point count, coincident at the split. Moving one moves the other, but their control points are independent [DOCS].
- Context menu: **Insert control points** [BUNDLE].
- Display: curve (blue/constraint color), control polygon (black), control points (white circles) [DOCS].

#### 2.4.18 Spline control point (no shortcut)
- Click anywhere on a spline or on a Bezier control polygon to add a control point. Drag it to reshape [DOCS].
- Difference from a sketch Point: a control point is part of the spline definition and moves the spline. A sketch Point that is coincident to the spline is not part of it [DOCS].
- Display when not adjusting: sketch points are **white with blue border**, spline control points are **solid blue** [DOCS].

#### 2.4.19 Point (`Shift+S`)
- Click to place a point. It can carry most constraints [DOCS].
- Size: active point 6 px, user point 8 px. Inactive 4/6 px [BUNDLE]. Standalone points can be deleted with Trim [DOCS].

#### 2.4.20 Construction (`q`)
- Toggle. Mode A: select entities, then click Construction (or press q) to **toggle** them between regular and construction. Mode B: activate Construction, then pick a tool to draw new construction geometry [DOCS].
- Context menu checkbox **Construction** (`constructionCheckbox`) on selected entities [BUNDLE].
- Text cannot be construction (`SKETCH_TEXT_CANNOT_BE_CONSTRUCTION`) [FS].
- Polygon helper circles are created as construction automatically [DOCS].

#### 2.4.21 Text (no shortcut)
- Click and drag a box. The **lower-left corner and height** define the position and the size of the first line [DOCS].
- A **horizontal** constraint is applied to the lower edge by default [DOCS].
- The Text dialog opens:
  - Preview box at the top.
  - **Text** field: up to 250 chars, paste allowed, multi-line. Lines after the first render outside (below) the box. The box represents line 1's length and height (baseline to ascender) [DOCS].
  - **Font** dropdown. Choose a font that supports your language [DOCS].
  - Style toggles: **Bold**, **Italic** [DOCS].
  - Flip icons: **Flip about horizontal center**, **Flip about vertical center** (`mirrorHorizontal`/`mirrorVertical`) [DOCS][FS].
  - Right-click the text field > **Convert to expression**. The field becomes an expression line that accepts string, length, angle and number variables (e.g. `#txt`, `roundToPrecision(#Length/in, 3) ~ " in"`, `#Angle/deg ~ " degree"`, `#Number ~ " items"`). Right-click > **Convert to text** reverts it [DOCS]. Default text label "Default text" [BUNDLE].
- Edit later: context **Edit text** [DOCS][BUNDLE].
- Sketch text can be configured: "Configure sketch text" / "Unconfigure sketch text" [BUNDLE].
- Box behavior: dragging an unconstrained box moves it (only the box shows during the drag; text redraws when stationary). To rotate: delete the horizontal constraint, Fix one corner, drag the opposite corner. To resize: drag a corner with another corner fixed. A dimensioned box can't be resized by dragging [DOCS].
- Text can be dimensioned and extruded. It **cannot be sketch-patterned or sketch-mirrored**. Use feature pattern/mirror instead [DOCS].
- Errors: `SKETCH_TEXT_IS_EMPTY`, `SKETCH_TEXT_RECTANGLE_FAILED` ("The text rectangle could not be created with selected points."). Hint: "Drag sketch text by its text box." [FS][BUNDLE].
- FS `skText {text, fontName (.ttf/.otf), firstCorner, secondCorner, mirrorHorizontal, mirrorVertical}` [FS].

#### 2.4.22 Insert image (no shortcut)
- Opens the **Insert image** dialog with tabs **Current document** (search, plus Create a version / Version graph links), **Other documents** (company), and **Libraries** (library dropdown). An **Import** link at the bottom covers the empty case [DOCS].
- Formats: PNG, JPEG, GIF, BMP. Alpha is respected. Max size is the GPU Max Texture Size [DOCS].
- Click **Done**, then click-drag in the graphics area to place. The aspect ratio is locked and shown by a dashed line. The image is semi-transparent while moving. Prompt: "Draw image rectangle." [DOCS][BUNDLE].
- A horizontal constraint goes on the lower edge by default. Rotate/move rules match Text (delete constraints, fix a corner, drag) [DOCS].
- The **first dimension** added to sketch geometry scales the image. Only the first dimension does this [DOCS].
- Context **Edit image** replaces the file. Copy/paste works within and across Part Studios. Show/Hide follows the sketch [DOCS].
- Opacity: active 1.0, inactive 0.5 [BUNDLE].
- Image libraries (company/personal folders) are document-management scope. Mention only.

#### 2.4.23 Insert DXF or DWG (no shortcut)
- Dialog: choose **Units** (bottom) first, optionally check **Use file origin position**, then pick a file from the current document, **Other documents**, or **Import**. **Selecting a file closes the dialog immediately** [DOCS].
- Positioning: with Use file origin position checked, the file origin maps to the Part Studio origin. Otherwise the 2D bounding-box center goes to the origin [DOCS].
- The first dimension applied scales the entire inserted sketch [DOCS].
- Errors: `SKETCH_INSERT_DWG_CONVERSION_FAILURE`, `SKETCH_INSERT_DWG_MAX_ENTITIES_EXCEEDED`. An alert is shown on geometry/format/integrity errors [FS][DOCS].
- Recommended for empty sketches [DOCS].

### 2.5 Edit / modify tools

#### 2.5.24 Sketch fillet (`Shift+F` in sketch; Part Studio Shift+F = 3D fillet)
- Select a **point (vertex)** or **two curves** (lines, arcs, splines) [DOCS].
- Two lines: click the first line (no highlight), then click and **drag on the second line** to estimate the size. Type the radius and press Enter [DOCS].
- Vertex: click the vertex, type the radius, press Enter. **Each later click on another vertex creates an equal-size fillet.** The first value becomes the default, and editing it changes all of them [DOCS].
- Splines: click each spline near the corner, not on the point [DOCS].
- Preview: a radius value box and a drag **manipulator** to resize. A fillet too big for a corner displays in **red** [DOCS].
- Result: an arc tangent to both curves, a radius dimension, and a **virtual sharp** (small open circle at the original corner). The virtual sharp is reference geometry that keeps the coincident constraints and can be used for constraints/dimensions [DOCS].
- Errors: "Cannot fillet selected point.", `SKETCH_FILLET_PARALLEL`, `SKETCH_FILLET_INVALID_RADIUS` [BUNDLE][FS].

#### 2.5.25 Sketch chamfer (no shortcut)
- Select a vertex or two edges (lines highlight yellow on selection) [DOCS].
- Two **distance** fields. The angle defaults to **45°**. Edit either field. A manipulator drag resizes [DOCS].
- Multiple: select more edges/vertices. Editing the first selection's dimensions updates all of them [DOCS].
- Too large shows red. If it can't be created, a **yellow warning bar** appears [DOCS].
- Errors: "Cannot chamfer selected point.", `SKETCH_CHAMFER_PARALLEL`, `_INVALID_INPUTS` [BUNDLE][FS]. i18n "Chamfer distances", "Chamfer angles" [BUNDLE].

#### 2.5.26 Trim (`m`)
- Click a segment to delete the portion up to the nearest intersections/bounding geometry. If nothing bounds it, the **entire curve is deleted** [DOCS].
- **Drag** across multiple entities to trim everything the path crosses. The drag path draws as a 1 px line, `--os-sketch-drag-trim-path-color` #555 light / #f5f5f5 dark [DOCS][BUNDLE].
- Clicking a standalone point deletes it [DOCS].
- Bezier keeps degree and control count [DOCS]. Cannot trim spline handles or control polygons [FS].

#### 2.5.27 Extend (`x`)
- Click the line or arc, then click the new end location, or extend to the first intersecting/bounding geometry. With no bound, it ends at the release point [DOCS].
- Arc: double-click the arc edge, or select and drag it, to auto-complete to bounding geometry (e.g. close an elliptical arc into a full ellipse) [DOCS].
- Error: "Unable to extend the selected geometry." [BUNDLE].

#### 2.5.28 Split (no shortcut)
- Click the curve, then click one or more split locations. An **open curve needs ≥1 point, a closed curve ≥2**. Prompt after the first click on a closed curve: "Select a second point to split closed curve." [DOCS][BUNDLE].
- Bezier behavior as in §2.4.17 [DOCS].

#### 2.5.29 Offset (`o`)
- Select one or more curves, or chain-select a loop. A **direction arrow** and **distance** field appear. Click the arrow to flip, drag the arrow to set an approximate value, or type a value. A negative value also flips. Press Enter [DOCS].
- Then click more curves to offset them by the **same** distance [DOCS].
- Creates an OFFSET constraint (one shared distance) [FS enum]. Errors include `SKETCH_OFFSET_CHAIN_FAIL`, `_BASE_CURVE_CONSUMED`, `_SPLINE_SPLIT`, `_ELLIPSE_SPLIT` [FS].

#### 2.5.30 Use (project/convert) (`u`)
- Click Use, then click an edge, several edges, a face (selects its edges), or a silhouette from another sketch or part. **Hover shows a preview** of the projected curve [DOCS].
- Result: projected curves carry a **Use** constraint (PROJECTED) to the source. Their glyphs have a **blue background** (external reference), darker blue on hover [DOCS].
- **Silhouette edges**: there is no pre-select for silhouettes. Hover a curved face. If the true silhouette is off-plane, two highlights show: the real 3D silhouette, rendered `rgba(255,0,255,0.5)` with `stipple(5,5,5,5)`, and its projection. Both are selectable. Clicking a face selects all its silhouette edges [DOCS][BUNDLE].
- Rules: silhouette ends are not constrained (you choose how to fix them). Only trackable silhouettes are allowed. Supported surfaces: cylinders, cones, tori, spheres, extruded surfaces, any surface with one silhouette. Self-intersecting projections are unusable [DOCS]. Silhouette disambiguation enum: ALIGNED/ANTI_ALIGNED/OUTER/INNER/TORUS_TOP/BOTTOM/LEFT/RIGHT [FS].
- Used edges update with the model but cope poorly with geometry type changes (circle→line) [DOCS].
- Context menu **Use** appears on selections too [BUNDLE].

#### 2.5.31 Intersection (no shortcut, in the Use dropdown)
- Select a surface or face that crosses the sketch plane. The intersection curve is projected into the sketch [DOCS].
- Constraints: open curves get **Pierce** constraints at their ends (on the face edges). Closed curves get **Intersection** constraints (INTERSECTED). Updates with the model [DOCS].
- Errors: `SKETCH_INTERSECTION_FAILED/_MULTIPLE_FAILED/_PARTIAL_FAILED` [FS].

#### 2.5.32 Mirror (no shortcut)
- Pre-selection flow: select entities, click Mirror ("Select a mirror line."), then click a sketch line, plane, part face or linear edge. The mirror resolves immediately [DOCS][BUNDLE].
- Tool-first flow: click Mirror, click the mirror line ("Select entities to be mirrored."). Each entity clicked afterward is mirrored immediately. Esc exits [DOCS][BUNDLE].
- Creates MIRROR (symmetric) constraints. Cannot mirror text, spline handles or control polygons. Offset splines/ellipses have mirror restrictions (`SKETCH_MIRROR_OFFSET_SPLINE/_ELLIPSE`) [FS][DOCS].

#### 2.5.33 Linear pattern (no shortcut)
- Select entities, then click the tool. An initial pattern is created [DOCS].
- On-canvas editors: an **instance count** label (double-click to type), an **arrow** manipulator (drag sets direction/angle and spacing), an **angle value** (double-click; visible after clicking the arrow icon), and a **distance between instances** (double-click). Drag the arrow manipulator's base to angle the pattern [DOCS].
- Commit by clicking white space. The cursor shows a mouse with a green button [DOCS].
- Preview shows max **10** instances (`SKETCH_PATTERN_PREVIEW_LIMIT=10`) [DOCS][BUNDLE].
- Supports one or two directions [DOCS]. `SKETCH_LINEAR_PATTERN_PARALLEL_DIRECTIONS`, `_ZERO_LENGTH`, `SKETCH_PATTERN_TOO_LARGE` ("Cannot make a pattern this large.") [FS][BUNDLE].
- Edit later: in the sketch, hover an instance, right-click the pattern glyph > **Edit pattern** [DOCS][BUNDLE].
- Deleting individual instances is allowed. Changing the count does not restore them; the slot stays empty [DOCS].
- Pattern count is configurable ("Configure pattern count") [BUNDLE].

#### 2.5.34 Circular pattern (no shortcut)
- Same flow and editors. Defaults: **closed 360°**, center **at the origin** (not constrained to it) [DOCS].
- Drag the arrow-head manipulator to set an open angle. The pattern stays open when the count changes. Drag the center icon to reposition and resize [DOCS].
- Pattern about an axis or mate connector [DOCS]. `SKETCH_CIRCULAR_PATTERN_ZERO_ANGLE` [FS].

#### 2.5.35 Transform (no shortcut) / context "Transform sketch entities"
- Pre- or post-select entities (add/remove during the operation). A **manipulator triad** appears [DOCS].
  - Arrow/plane drag translates. Normal drag does **not** snap to inferences.
  - **Center ball free drag** repositions the triad without transforming. Free drag **snaps to sketch inferences**. Use it to put the ball on a snap point, then drag the plane to carry that point onto another sketch point [DOCS].
  - **Angle indicator** drag rotates. An angle field activates; type, press Enter, click in space [DOCS].
  - **Scale triangle** drag scales out/in. A scale field activates [DOCS].
- Commit: click off the sketch or press **Enter**. Cancel: **Esc** [DOCS].
- Constraint rewriting: 0°/180° keeps internal constraints. 90°/270° **swaps horizontal↔vertical**. Construction geometry may be added to keep DOF. **Directed dimensions are deleted**, possibly replaced by construction geometry plus minimum dimensions [DOCS].
- Works on images, text, DWG/DXF [DOCS]. The context menu item hides while another sketch command is active, and when the selection includes spline handles or control polygon entities [BUNDLE].

#### 2.5.36 Slot (no shortcut)
- Select centerline curves (lines, arcs, splines, chains, closed profiles), either before or after picking the tool, individually or by box [DOCS].
- Pre-selected set: slots of **equal size on all curves, linked by one width dimension** (diameter). The dimension is editable before or after placement (double-click) [DOCS].
- Pick-sequence: after the first slot, keep clicking curves to add slots with the same dimension. Editing it changes all [DOCS].
- A **connected chain** gives one slot following the chain shape. Picking chain members one at a time (click empty space between picks) gives separate slots [DOCS].
- Errors `SKETCH_SLOT_FAILURE/_PARTIAL_FAILURE` [FS].

#### 2.5.37 Wrap (listed in the Sketch help area)
- Actually a Part Studio feature (Tools = faces/sketch regions, Target = cylinder/cone, Angle, U shift, V shift; Solid/Surface/Split modes). Documented in the Part-features section, not a sketch tool [DOCS].

---

## 3. Constraints

### 3.1 Applying constraints [DOCS]
- **Select-then-tool**: select entities, then click the constraint, or press its key, to apply at once.
- **Tool-then-select (toggle mode)**: activate a constraint with nothing selected. "Each pair of entities selected are constrained to each other." The tool stays on until you click it again (toggle off) or pick another tool.
- Constraints can reference sketch curves **and planes** (e.g. distance/coincident/tangent to a plane) [DOCS].
- The constraint menu is enabled only when the selection is valid *(inferred)*. Invalid input reports e.g. `SKETCH_CONSTRAINT_TWO_ENTITIES`, `SKETCH_PARALLEL_INPUT_ERROR` [FS].
- Selection priority: non-fully-constrained points (blue/red) win over overlapping fully-constrained (black) points [DOCS].

### 3.2 Constraint catalog (toolbar order per docs)

| Constraint | Key | Selection | Behavior / notes |
|---|---|---|---|
| **Coincident** | i | 2+ entities: point–point, point–curve, curve–curve (same underlying geometry), entity–plane | The **infinite underlying geometry** is made coincident (a point on the extension of a line counts). Line–line gives collinear. Arc–arc gives the same circle [DOCS]. Also in Drawing. |
| **Concentric** | Shift+O | circle/arc + circle/arc, or point + circle/arc | Shared center. A point becomes coincident with the center [DOCS]. |
| **Parallel** | b | 2+ lines (or linear edges) | All parallel [DOCS]. FS alignment ALIGNED/ANTI_ALIGNED [FS]. |
| **Tangent** | t | 2+ curves, or curve + plane | Tangency. Applies to line/arc/circle/spline/ellipse/conic combinations [DOCS]. Cannot be applied between a spline and its own handle (`SKETCH_TANGENT_CONSTRAINT_SPLINE_AND_HANDLE`) [FS]. |
| **Horizontal** | h | 1+ lines, or a set of points | Lines become horizontal in sketch coordinates. Points line up horizontally [DOCS]. |
| **Vertical** | v | 1+ lines, or a set of points | Same, vertical [DOCS]. |
| **Perpendicular** | Shift+L | 2 lines | Right angle [DOCS]. |
| **Equal** | e | 2+ lines (length) or 2+ arcs/circles (radius) | "Equal length/radius" [BUNDLE]. If one is already dimensioned, its size wins [DOCS]. |
| **Midpoint** | Shift+M | point + line, point + arc, or **3 points** (start, end, then middle) | Point at the midpoint of a line/arc, or the third point equidistant between the first two [DOCS]. |
| **Normal** | Shift+K | line + curve, or curve + plane | Line normal to a curve (perpendicular to the tangent at the intersection) [DOCS]. `SKETCH_NORMAL_NEED_LINE` [FS]. |
| **Pierce** | Shift+G | sketch point/curve + an off-plane edge/curve that **passes through** the sketch plane | Point goes to where the edge crosses the plane. Can't pierce with a plane (`SKETCH_CANNOT_PIERCE_WITH_PLANE`) [DOCS][FS]. |
| **Symmetric** | Shift+Q | **first** a line or linear edge (axis), then 2 same-type geometries | Constrains only the **underlying curves** (two arcs make their circles symmetric, not their endpoints) [DOCS]. `SKETCH_MIRROR_NEEDS_LINE_AND_TWO_OTHERS` [FS]. |
| **Fix** | Shift+J | 1+ sketch entities | Grounds the entity at its current location on the plane [DOCS]. |
| **Curvature** | Shift+U | spline/conic + adjacent geometry at a shared end | G2 curvature-continuous transition (EQUAL_CURVATURE) [DOCS][FS]. Not between a spline and its handle [FS]. |
| **Curvature G3** (flow) | unassigned | as Curvature | Present as a command (`curvatureG3`, `TOGGLE_SKETCH_CURVATURE_G3`) and constraint type `EQUAL_CURVATURE_G3`. Not documented in Help. G3 means "Match flow" [BUNDLE][FS][DOCS surface modeling]. Put it in the constraint menu next to Curvature *(inferred)*. |

Drawing mode reuses Coincident, Parallel, Tangent, Horizontal, Vertical, Perpendicular [DOCS "Available in: Sketch, Drawing"].

### 3.3 Constraints created automatically (not on the toolbar)
- **Quadrant**: a point coincident to an ellipse **and** on its major or minor axis. Created by inference, dragging onto, or placing on an ellipse's quadrant points [DOCS].
- **Use** (PROJECTED / SILHOUETTED): from the Use tool [DOCS][FS].
- **Intersection** (INTERSECTED) + **Pierce**: from the Intersection tool [DOCS].
- **Offset** (OFFSET), **Mirror** (MIRROR), **Linear/Circular pattern** (LINEAR_PATTERN / CIRCULAR_PATTERN): from those tools. The pattern glyph's context menu gives **Edit pattern** [FS][BUNDLE].
- **Rho** (RHO), **Bezier degree** (BEZIER_DEGREE): value constraints on conics/Beziers [FS].
- **Freeze** (FREEZE): exists in the FS enum, with no UI documentation (likely internal, e.g. for imported/transformed geometry) [FS].
- From creation tools: coincident at chained/snapped points, tangent for tangent arcs, horizontal on text/image bottom edges, rectangle H/V/parallel/coincident set (§2.4.3), inferred H/V/parallel/perpendicular/midpoint/coincident/tangent (§6) [DOCS][FS].

### 3.4 Full ConstraintType enum [FS] (the model must support all of these)
NONE, COINCIDENT, PARALLEL, VERTICAL, HORIZONTAL, PERPENDICULAR, CONCENTRIC, MIRROR, MIDPOINT, TANGENT, EQUAL, LENGTH, DISTANCE, ANGLE, RADIUS, NORMAL, FIX, PROJECTED, OFFSET, CIRCULAR_PATTERN, PIERCE, LINEAR_PATTERN, MAJOR_DIAMETER, MINOR_DIAMETER, QUADRANT, DIAMETER, SILHOUETTED, CENTERLINE_DIMENSION, INTERSECTED, RHO, EQUAL_CURVATURE, BEZIER_DEGREE, FREEZE, EQUAL_CURVATURE_G3.
Related enums: `DimensionDirection {MINIMUM, HORIZONTAL, VERTICAL}`, `DimensionAlignment {UNSPECIFIED, ALIGNED, ANTI_ALIGNED}`, `DimensionHalfSpace {UNSPECIFIED, LEFT, RIGHT}`, `RadiusDisplay {NONE, RADIAL, DIAMETRAL}`, `FixedParameterPosition {FIXED_AT_START, FIXED_AT_END, FIXED_ON_CURVE, FREE}`, `SketchProjectionType {USE, SILHOUETTE_START, SILHOUETTE_END, USE_END}`. skConstraint takes `length`/`angle` values with units checked. Wrong units put the constraint into an error state [FS].

### 3.5 Constraint glyphs (indicators)
- Small square icons (20 px: `CONSTRAINT_INDICATOR_PIXEL_SIZE`, `_PERMANENT_`, `_INFERENCE_`, max 20) drawn next to the constrained entity, with a leader line to it. Leader color: #9aa5af light / #ccc dark [BUNDLE].
- Visibility: with **Show constraints** off, glyphs appear only for the hovered/highlighted entity (show delay **50 ms**, hide delay **750 ms**). **Hold Shift** while moving the mouse to keep all glyphs visible. Show constraints on means all are visible permanently [DOCS][BUNDLE].
- Colors per docs: **well-defined = black icon on gray**, **problem = white icon on red**. External (cross-sketch, e.g. Use) constraints have a **blue background**, darker blue on hover [DOCS].
- Token values [BUNDLE tokens.css, light theme]:

| State | Internal OK | Internal bad | External OK | External bad |
|---|---|---|---|---|
| normal | `--os-constraint-ok-color` #ffffff (dark theme #555555) | #d66e74 | #def1fb | #d66e74 |
| prehilite (hover) | #fcda7f | #aa252d | #b2ddf6 | #aa252d |
| selected | #fab601 | #bd3039 | #90cef1 | #bd3039 |
| muted | rgba(255,255,255,0.4) | rgba(163,0,0,0.5) | rgba(114,206,241,0.4) | rgba(163,0,0,0.5) |

  - Hovered glyph border: `CONSTRAINT_INDICATOR_BORDER_COLOR=rgb(255,169,10)`, 2 px. Generic indicator tint `rgba(39,170,225,0.3)` [BUNDLE].
- Interactions [DOCS]:
  - **Drag** a glyph or glyph group to move it, for display only.
  - **Hover** a glyph to highlight its entities. Related entities highlight **yellow** when a constraint is selected.
  - **Click** a glyph to select it, then **Delete** key or context **Delete** removes the constraint.
  - Right-click a pattern glyph > **Edit pattern**.
- While drawing, **constraint indicators appear next to the cursor** as curves snap to inferences [DOCS].

---

## 4. Dimension tool (`d`)

### 4.1 Basics
- Click Dimension or press d. Pick one or two entities (or entity + plane). **Move the mouse to choose the dimension type and placement.** Click to place; this click also **opens the value field**. Type a value, expression or variable and press **Enter** [DOCS].
- The selected entities highlight. Highlight constants: primary `rgba(255,198,133,…)` border, secondary `rgba(242,255,128,0.4)` [BUNDLE].
- Types: horizontal, vertical, shortest (aligned) distance, angle, diameter, radius, arc length, plus distance to planes and centerline dimensions. Ellipse major/minor diameters, rho, Bezier degree [DOCS][FS].
- The tool stays active for the next dimension until Esc *(inferred, standard)*.
- Errors: "A dimension cannot be created between these items.", "Could not create a dimension with the selections.", "Could not create the dimension." [BUNDLE i18n].
- Measure tool (`[`) for measuring without constraining [DOCS].

### 4.2 Placement by selection [DOCS]

| Selection | Clicks | Mouse placement result |
|---|---|---|
| Single line | line | Length. "Drag to position" sets which side/offset. Used for length or height. |
| Two lines | line, line | Distance between them. Implies parallelism without showing a parallel glyph. |
| Two lines at an angle | line, line | **Angle**. Drag into/out of the angle, or **into the quadrant** whose angle you want (4 choices). |
| Two points, diagonal | point, point, drag at an angle | **Aligned/direct (shortest) distance**. |
| Two points, linear | point, point, drag straight up/down or left/right | **Horizontal or vertical** distance (cursor position picks the axis; DimensionDirection HORIZONTAL/VERTICAL vs MINIMUM). |
| Circle | edge | **Diameter**. Drag in/out to position. |
| Arc | arc | **Radius**. Drag in/out to position. |
| Arc length | endpoint 1, endpoint 2, then the arc | **Arc/curve length**. |
| Spline points | spline point + spline/sketch point | Distance. Drag axis-aligned for linear, at an angle for direct. |
| Point + plane | point, plane | Distance to plane. Editing moves the sketch geometry; the plane stays fixed. |
| Point/circle/non-construction line + **construction line** | entity, construction line | Normal distance. **Move the cursor across to the opposite side of the construction line to toggle to a centerline dimension** (twice the distance, used for revolve diameters; CENTERLINE_DIMENSION). Crossing back toggles it again. |
| Ellipse | ellipse | Major/minor diameter *(inferred from FS enums; on-the-fly boxes documented)* |

- Angular preview radius 80 px (`PREVIEW_ANGULAR_DIMENSION_RADIUS_PIXEL_SIZE`) [BUNDLE].

### 4.3 Value entry and editing [DOCS]
- Inline value field at the dimension. Accepts numbers, units, expressions and functions (§4.6), plus `=#var` in on-the-fly fields or `#var` in edit fields.
- **Edit a placed dimension**: double-click the value. This opens the **Dimension context dialog** (value field, tolerance icon, ✓, ✗, drag handle to move the dialog). Press Enter or click ✓ to accept, ✗ to discard [DOCS tolerance_options].
  - Tolerance icon, then dropdown: **Precision** (Workspace precision or 0–6 decimals), **Tolerance type** (Default, No tolerance, Symmetrical, Deviation, Limits, Min, Max, Basic; diameters also get Fit / Fit with tolerance / Fit (tolerance only)), with fields Deviation / Upper / Lower / Standard (ANSI/ISO) / Fit type / Hole class / Shaft class [DOCS].
  - Tolerances are allowed on distance between lines/points/plane, diagonal, diameter, angle, direct, linear, radius, centerline. **Not allowed on single-line length or arc length** [DOCS].
- **Negative values** flip direction. Allowed for distance, angle, line length, spline dimension. **Not** allowed for arc, radius, diameter [DOCS].
- **Symmetric solve on edit**: except for diameter/radius, a new value moves **both** referenced geometries by half the change each. For a single line, both endpoints move equally. Constrained geometry may not move [DOCS].
- Delete: select a dimension, then Delete key or context **Delete** ("Delete dimension") [DOCS][BUNDLE].
- Context menu on a dimension: **Change to driving dimension / Change to driven dimension**, **Configure dimension** ("Configure dimension with <config>") / **Unconfigure dimension**, **Add comment**, **Delete** [DOCS][BUNDLE].
- Context menu on a sketch (Feature list) **Show dimensions** shows the dimensions without editing [DOCS].
- Variable autofill and New variable (§2.3).
- Mouse wheel over a numeric field: ±0.1 per notch, Ctrl ±0.01, Shift ±1.0 [DOCS numeric fields].

### 4.4 Driving vs driven [DOCS]
- New dimensions are **driving** by default. Driving dimensions are drawn black and are editable.
- **Driven (reference)** dimensions are drawn **light gray** (token `--os-dimension-driven-line-point-color` #666 light / #ccc dark), show the measured value, can't be edited, and don't move geometry.
- If a new dimension would **over-define** the sketch, it is **automatically made driven**. There is no blocking prompt.
- Driven → driving re-applies the value. If that over-constrains, the usual red indicators appear.
- Driven dimensions can be placed anywhere a driving one can.

### 4.5 Dimension appearance [BUNDLE]
- Value font "Flama", 18 px. **3 decimal places** default display (`DIMENSION_DECIMAL_PLACES=3`). Value background rounded rect radius 4, padding 5, opacity 0.6. Arrow 16×8 px. Line width 1 (selected 2). Witness gap 7 px from model, extension 4 px. Arc-length scale 0.75.
- Colors (light): line/arrow and text #000000. Driving-hovered line #987107. Selected line #6a4f06. Hovered value background #bf9460. Selected value background #bf6900. Driven line #666666.
- **Configured dimension**: dashed border rgb(255,194,14), thickness 3, dash 8, gap 5.
- **Expression indicators** (docs: icon to the left of the value): "Expression without a variable" icon vs "Expression with a variable" icon [DOCS]. With **Show expressions** on, the expression text box appears above the value: Flama 14 px, padding 6, border 2, radius 4.
  - With variables: background #def1fb (hover #b2ddf6, selected #90cef1).
  - Without variables: white background with #90cef1 border (hover #fcda7f, selected #fab601).
  - **Bad expression**: background #dd979b (hover #aa252d, selected #bd3039, white text).
- Inactive/occluded dimension witness: stipple(5,5,5,5), color #666 light.
- Tolerance dims shown in the Inspection table later (MBD, out of scope here).

### 4.6 Numeric field grammar (applies to all sketch value boxes) [DOCS numeric fields]
- Units: mm, millimeter, cm, centimeter, m, meter, in, inch, ft, foot, yd, yard, plus plurals. deg, degree, rad, radian. Unitless input uses the default unit. Display converts to the default unit, and the original text reappears when you click back into the field.
- Operators: + - * / ^ %, parentheses. Functions: ceil floor round exp sqrt abs max min log log10. Trig in **degrees**: cos sin tan acos asin atan atan2 cosh sinh tanh asinh acosh atanh. Constants pi/PI/Pi.
- Arrays `[3,5,6,7][2]` (0-based). Ternary `#w > 5 ? 7 : 4`. `&&`, `||`. Comma decimal separator per account setting, which requires spaces in arrays.
- The result must be unitless or a unit to the first power. Invalid: `3in*3in`, `3+3in`, `sin(30)/deg`, anything giving 1/unit.
- Evaluated as FeatureScript. The evaluated value is shown after acceptance and the expression on re-edit.
- Rho and polygon side counts are unitless.

---

## 5. Visual state conventions

### 5.1 Constraint-status colors (lines and points) [DOCS][BUNDLE]

| State | Light | Dark | Show-through (occluded behind parts) |
|---|---|---|---|
| Under-constrained | **#0000d0** (blue) | #3e94ff | rgb(64,64,255). Token alternative #a2b9df |
| Fully constrained | **#000000** | #ffffff | rgb(64,64,64) |
| Over-constrained / error | **#cd0000** | #ff6b6b | rgb(192,64,64). Token #dd979b |
| Inactive sketch (any state) | **#999999** | #999999 | – |

- Status is computed **per entity/point**, not per sketch. A sketch mixes blue and black geometry. "Black means fully constrained" [DOCS].
- Users can change constrained/underconstrained/underconstrained-occluded colors, inactive color, active/inactive line width (0.1–3 px) and the construction stipple in **Graphics preferences > Sketch** [DOCS].

### 5.2 Line styles [BUNDLE][DOCS]
- Active sketch line width **2 px**. Inactive **1 px**. Show-through width matches.
- Normal lines are solid (`DEFAULT_LINE_STIPPLE stipple(12,0,12,0)`).
- **Construction**: `stipple(4, 6, 30, 6)`, meaning dash 4 px, gap 6, dash 30, gap 6 (a dash-dot pattern). Preference defaults are First dash 4 (1–50), First gap 6 (0–50), Second dash 30 (1–50), Second gap 6 (0–50) [DOCS][BUNDLE]. Same status colors as regular geometry.
- **Show-through** (sketch geometry hidden behind model geometry): `stipple(15,15,15,15)` dashes in lighter color. Docs: sketch lines obstructed by the model still show, with construction lines dashed through parts [DOCS].
- Centerline stipple(5,10,10,10) (axis/centerline display) [BUNDLE].
- Docs line-style matrix: Active / Active-selected / Inactive / Inactive-selected for both sketch and construction lines. Selected lines take the selection highlight color [DOCS].
- Used (projected) edges are drawn as normal sketch curves with an external-constraint glyph [DOCS].

### 5.3 Points [BUNDLE]
- Active: sketch point 6 px, **user Point** entity 8 px, spline handle 10 px. Inactive: 4 / 6 / 0 (handles hidden).
- Fonts: sketch point `point(1,0,50)`, user point `point(2,0,50)` (bigger/different glyph), spline handle `point(2,0,30)`.
- Spline handles: white points. Control points: solid blue. Points on curves: white fill with blue border [DOCS].
- Fillet virtual sharp: small open circle [DOCS].
- Origin glyph `point(3,0,50)` [BUNDLE].

### 5.4 Selection and preselection highlight [BUNDLE][DOCS]
- Hover (preselection) uses an **orange** border, `PRE_HIGHLIGHT_COLOR_BORDER rgba(255,198,133,1)` [DOCS: "Entities are highlighted in orange upon hover"].
- Selected entities use `ENTITY_HIGHLIGHT rgba(255,169,10,0.8)`, border rgba(255,169,10,1) (amber/orange-yellow). Highlight point scale 1.5× (min 12 px), opacity 0.68.
- Related/secondary entities (e.g. the entities of a selected constraint) use **yellow** `rgba(242,255,128,0.4)` [DOCS].
- Hidden highlighted lines use stipple(10,10,10,10).
- Prehighlight is suppressed while the mouse moves fast (`PREHIGHLIGHT_SPEED_THRESHOLD=.15`, paused timeout max 250 ms).
- **Select other** (backtick `` ` ``): lists the entities under the cursor (point glyph rgb(255,194,14) size 9) [DOCS][BUNDLE].
- Box select: left-to-right window `stipple(4,0,4,0)`, crossing `stipple(9,8,9,8)` *(direction semantics inferred)* [BUNDLE].

### 5.5 Sketch plane and telltale [BUNDLE]
- Sketch plane outline/text color `--os-sketch-plane-line-color` #90cef1.
- "Telltale" label: inactive rgba(68,156,203) 14 px. Active telltale rgba(97,107,115) on a white 80 % background, offset 5 px, 50 px switch tolerance. This is likely the floating sketch-name tag shown at the sketch *(inferred)*.

### 5.6 Errors in geometry
- Over-defined entities and glyphs turn red (glyph white-on-red) when **Show errors** is on [DOCS].
- Fillet/chamfer previews too large for the corner draw red. A failed chamfer shows a yellow warning bar [DOCS].
- Profile inspector issue highlight: `rgba(189,48,57,1)` (red). The focused issue is yellow `rgba(242,255,128,0.4)` [BUNDLE].
- A failed sketch feature shows the standard feature error state in the Feature list (FS errors §10).

---

## 6. Automatic inferencing [DOCS unless noted]
- While drawing, placing or dragging, the sketcher **suggests constraints** and snaps to them. On click, the inferred constraint is **created** (it becomes a real constraint).
- **Always-on inferences**: horizontal and vertical alignment between the cursor point and the **origin** and/or other entities' points. Example: hover above the origin and a dotted vertical inference line appears.
- **Wake-up inferences**: some appear only after the cursor **hovers near an entity** to "wake" it. Common ones: horizontal, vertical, midpoint, parallel, coincident. Perpendicular and tangent also exist (docs step: wake a line, then draw perpendicular). Waking highlights the entity, and midpoints/centers/quadrants become snap targets.
- **External wake-up**: coplanar vertices and edges of **other sketches or features** wake up on hover and can be snapped to. This creates references, e.g. a circle constrained to a gear vertex from another sketch [DOCS sketch basics].
- Visual feedback:
  - Inference line: a **dotted/dashed line** drawn `INFERENCE_SNAP_LINE_COLOR rgb(255,194,14)` (yellow), width 1, `stipple(4,4,4,4)`. Snap point box 8 px, stipple(12,0,12,0) [BUNDLE]. Docs: "a dotted line appears", "the yellow inference line appears".
  - When a new line becomes parallel to another, the new line turns **dotted** and the related line is highlighted **orange**. The constraint icon shows next to the cursor.
  - Constraint glyph previews at the cursor (inference indicator 20 px) [BUNDLE].
- **Shift** (held while dragging/placing) **suppresses** automatic inferences (action `suppressInferencing`, scope `sketchEdit`, not editable) [DOCS][BUNDLE].
- 3-point-arc semicircle snap, elliptical-arc axis/quadrant snaps, and ellipse quadrant (QUADRANT) inference are documented special cases.
- Tuning constants: `MAXIMUM_INFERENCE_PICKS=10`, `INFERENCE_POINT_SIZE_PX=15`, `INFERENCE_LINE_WIDTH_PX=15` (pick tolerance), `TEMPORARY_INFERENCE_OFFSET=1.4`, `PERMANENT_INFERENCE_OFFSET=.5` [BUNDLE].
- Error `SKETCH_INFERENCE_FAILED` / `SKETCH_INFER_DIM_FAILED` [FS].

---

## 7. Dragging, solver, diagnostics

### 7.1 Dragging under-defined geometry [DOCS]
- With **no tool active**, press and drag any under-constrained point or curve. The solver moves it while keeping all constraints. "Dragging entities may help you understand what constraints or dimensions you may want to add."
- Fully constrained (black) geometry does not move. Fixed entities don't move.
- Dragging a whole curve (e.g. a line body) translates it. Dragging an endpoint reshapes it *(inferred, standard)*.
- Point-selection priority for overlapping points: blue/red before black.
- Spline: drag fit points/handles. Bezier: drag control points (tool deselected).
- Text/image boxes: drag to move. The content is hidden during the drag (text) or semi-transparent (image) [DOCS].
- Dragging onto a point or ellipse quadrant can create coincident/quadrant constraints (inference while dragging). Shift suppresses this [DOCS].
- Error `SKETCH_DRAG_ERROR` [FS].

### 7.2 Solver
- Each change re-solves the whole sketch. Constraint status (under/fully/over) is computed per entity for coloring [DOCS].
- Over-constraint: an added dimension that over-defines is auto-converted to **driven** (§4.4). Conflicting geometric constraints show **red** entities and **white-on-red** glyphs. The sketch keeps the last valid solution *(inferred)*.
- FS error vocabulary for solve problems: `SKETCH_SOLVE_FAILED`, `SKETCH_UNSOLVABLE_CONSTRAINT`, `SKETCH_EXCEEDS_BOUNDS`, `SKETCH_DIMENSION_LIMIT_ERROR`, `SKETCH_DIMENSION_INFINITY`, `SKETCH_TIMED_OUT` [FS].
- Coincident tolerance is displayed in measurement as "(within coincident tolerance)" and tangent as "(within tangent tolerance)" [BUNDLE i18n].

### 7.3 Constraint manager [DOCS][BUNDLE]
- Open: Sketch dialog > **Show sketch diagnostics tools** > **Constraint manager…**. Close with the **X** at its top right, or the diagnostics menu > **Turn constraint manager off**. Command `TOGGLE_SKETCH_CONSTRAINT_MANAGER`.
- Panel contents (top to bottom):
  - **Filters** header: collapse/expand all sections.
  - **Automatically select constraints** toggle, **on by default**. When on, every constraint matching the filters gets a yellow bounding box and is selected in the sketch. When off, you can box-select part of the sketch first and the filters then apply only to that selection.
  - **Type**: multi-select list of constraint types (Coincident, …).
  - **Mode**: **Internal** (this sketch) / **External** (external sources) / **In context** (in-context references). Enum INTERNAL/EXTERNAL/IN_CONTEXT.
  - **Status**: **Driven** (highlight yellow, or darker blue if under-constrained) / **Solved** (yellow, or darker blue if under-constrained) / **Errors** (darker red).
  - Sort: **Sort by constraint** (constraint, then entity) / **Sort by entity** (entity, then constraint). Enum ByConstraint/ByEntity.
  - **Entities and constraints** list. Hover highlights the item in the sketch. A row selected as an entity highlights light yellow. Each row has **Delete constraint** (trash icon) and **Remove selection** (removes it from the list) icons.
  - Row context menu: **Delete constraint** / **Delete entity**, **Zoom to selection**, **Hide external references** (toggles **Show external references**), **Clear all** (empties the list).
  - **Delete all** button deletes all listed constraints.
- Filters combine freely (any number of types, modes, statuses).
- If Show constraints and Show errors are both off, hovering a row shows only that constraint and its entities.
- Works in the sheet-metal flat pattern view too.

### 7.4 Profile inspector [DOCS][BUNDLE]
- Open: Sketch dialog > diagnostics > **Profile inspector…**. Off: **Turn profile inspector off**. Command `TOGGLE_SKETCH_PROFILE_INSPECTOR`. Shortcut unassigned.
- Purpose: find **gaps / loose ends / unclosed loops** that prevent closed regions.
- Panel: a list of issues (e.g. "unclosed loop" entries). Click an issue to highlight it in the sketch (red `rgba(189,48,57)`, focused yellow). **Previous** / **Next** buttons cycle through issues. **Issues disappear from the list as soon as they are resolved** (live update).
- Error `SKETCH_PROFILE_ANALYSIS_FAILED` [FS]. i18n "Overlapping" exists, so overlapping curves are likely also reported *(inferred)*.

### 7.5 Troubleshooting guidance [DOCS]
- Read the colors (blue = add constraints, red = remove conflicts). Drag blue geometry to discover missing DOF. Use the Constraint manager's **Errors** status filter to list conflicts. Use the Profile inspector for open profiles.
- Curvature visualization: Curvature color map / Curve-surface analysis (Shift+C) and Connection analysis (G0–G3 between curves) are available in Sketch [DOCS].

---

## 8. Keyboard shortcuts in sketch mode

### 8.1 Sketch-scope defaults [DOCS keyboard shortcuts + glossary][BUNDLE]
`*` in the docs marks shortcuts that also appear on the default shortcut map.

| Key | Action (toggle: "Selects or exits") |
|---|---|
| l * | Line |
| Shift+A | Switch Line ↔ Tangent arc while chaining |
| g * | Corner rectangle |
| r * | Center point rectangle |
| c * | Center point circle |
| a | 3 point arc (outside a sketch, `a` = flip primary axis) |
| Shift+S | Point (sketch point). Outside a sketch: new Sketch |
| q | Construction toggle (pre-selected entities or new geometry) |
| Shift+F | Sketch fillet (outside: 3D Fillet) |
| m * | Trim |
| x | Extend |
| o | Offset |
| u | Use (project/convert) |
| d * | Dimension |
| i * | Coincident |
| Shift+O | Concentric |
| b * | Parallel |
| t * | Tangent |
| h * | Horizontal |
| v * | Vertical |
| Shift+L * | Perpendicular |
| e | Equal |
| Shift+M | Midpoint |
| Shift+K | Normal |
| Shift+G | Pierce |
| Shift+Q | Symmetric |
| Shift+J | Fix |
| Shift+U | Curvature |
| Shift (hold) | Suppress inferences while moving/placing. Also keeps all constraint glyphs visible while hovering |
| Alt (hold, while drawing rectangle) | Square (equal sides) |
| Alt+←/→ | Move between on-the-fly dimension boxes |
| Enter | Accept value / commit transform / commit sketch |
| Esc | Exit tool / cancel transform / close autofill |
| Delete / Backspace | Delete selected entities, constraints, dimensions |

Unassigned by default (customizable): Midpoint line, Aligned rectangle, 3 point circle, Ellipse, Tangent arc, Center point arc, Elliptical arc, Conic, Inscribed/Circumscribed polygon, Spline, Bezier, Spline control point, Text, Intersection, Split, Slot, Mirror, Linear/Circular pattern, Transform, Insert DXF/DWG, Insert image, Chamfer, Profile inspector, Constraint manager, Curvature G3 [DOCS glossary][BUNDLE].

### 8.2 General/view keys usable while sketching [DOCS]
- s: shortcut toolbar at the cursor. Alt/⌥+C: search tools. Shift+/: shortcut map.
- Ctrl/⌘+Z undo, Ctrl+Y / Shift+⌘+Z redo, Ctrl/⌘+C / V copy/paste, Space clear selection, `` ` `` select other.
- Shift+E Extrude, Shift+W Revolve (both accept the sketch first). Shift+H show/hide sketches. Shift+P hide construction geometry. p show/hide planes. k show/hide mate connectors.
- n normal-to (press twice to flip). f zoom to fit. w zoom window. z / Shift+Z zoom out/in. Shift+1/3/4/5/7 = front/left/right/top/iso. Arrow keys rotate 15°, Shift+arrows 90°, Ctrl+arrows 5°, Ctrl+Shift+arrows pan. `[` Measure. Shift+C curve/surface analysis. Shift+X section view.
- Caveat: `#` typed in a sketch dimension box without a leading `=` triggers a view hotkey [DOCS]. The clone should route keystrokes to the active value box first, except for this documented quirk.

---

## 9. Context menus in an active sketch [BUNDLE labels][DOCS]
Items shown depend on the selection:
- Any sketch selection: **Delete** (`deleteSketchEntities`; "Delete sketch entity" / "Delete N sketch entities"), **Construction** checkbox, **Transform sketch entities**, **Copy sketch entities**, **Use** (for external selections), **Clear selection**, **Zoom to selection**, **Add comment**.
- Empty space / plane: **Paste sketch entities** (`pasteActiveSketch`), **Paste sketch**, **View normal to sketch plane**.
- Text: **Edit text**, **Configure sketch text** / **Unconfigure sketch text**.
- Image: **Edit image**.
- Spline: **Insert spline points**, **Insert spline handles**, **Curvature color map**. During creation: **Confirm spline**.
- Bezier: **Insert control points**. During creation: **Escape bezier**.
- Pattern glyph: **Edit pattern**.
- Dimension: **Change to driving dimension** / **Change to driven dimension**, **Configure dimension** / **Unconfigure dimension**, **Add comment**, **Delete**.
- Constraint glyph: **Delete**.

---

## 10. Validation and error catalog (FS ErrorStringEnum, 203 SKETCH_* codes) [FS]
Rules a clone should enforce, grouped from the error names:
- Plane: `SKETCH_NO_PLANE`. Not active: `SKETCH_NOT_ACTIVE`.
- Geometry creation: LINE/ARC/CIRCLE/RECTANGLE/ELLIPSE/CONIC/SPLINE/TANGENT_ARC `_FAILED`, `SKETCH_TANGENT_ARC_INVALID_START`, `SKETCH_ELLIPSE_ZERO_AXIS`, `SKETCH_POLYGON_BAD_SIDE_COUNT`, `SKETCH_POLYGON_ZERO_RADIUS_FAIL`, `SKETCH_SPLINE_TOO_FEW_POINTS`, `SKETCH_SPLINE_NEW_POINTS_TOO_CLOSE`, `SKETCH_SPLINE_CANNOT_DELETE_ENDPOINTS`, `SKETCH_CANNOT_MAKE_2_POINTS_BEZIER`, `SKETCH_TEXT_IS_EMPTY`, `SKETCH_TEXT_CANNOT_BE_CONSTRUCTION`, `SKETCH_TEXT_RECTANGLE_FAILED`, `SKETCH_IMAGE_RECTANGLE_FAILED`.
- Constraint input arity/type: `SKETCH_CONSTRAINT_TWO_ENTITIES`, `_DIFF_ENTITIES`, `SKETCH_COINCIDENT_DIFF_POINTS`, `SKETCH_FIX_ONE_ENT`, `SKETCH_NORMAL_NEED_LINE`, `SKETCH_MIDPOINT_NEED_POINT/_NEED_DIFF_POINT/_MISSING_ENDS/_NO_INTERNAL_LINE`, `SKETCH_EQUAL_NO_ENDS`, `SKETCH_POINT_LINE_ONLY`, `SKETCH_ANGLE_TWO_LINES`, `SKETCH_MIRROR_NEEDS_LINE_AND_TWO_OTHERS`, `SKETCH_CANNOT_PIERCE_WITH_PLANE`, `SKETCH_TANGENT_/CURVATURE_CONSTRAINT_SPLINE_AND_HANDLE`, `SKETCH_EQUAL_CURVATURE_INPUT_ERROR`.
- Dimensions: `SKETCH_DIMENSION_TWO_ENTITIES`, `_DIFF_ENTITIES`, `_DIST_ERROR`, `_INFINITY`, `_LIMIT_ERROR`, `SKETCH_LENGTH_DIM_*`, `SKETCH_RADIUS_*`, `SKETCH_RHO_DIM_NOT_FOUND`, `SKETCH_BEZIER_DEGREE_DIM_NOT_FOUND`.
- Tools: FILLET (`_INVALID_POINT`, `_PARALLEL`, `_INVALID_RADIUS`), CHAMFER (same set), OFFSET (`_DISTANCE`, `_BAD_PAIR`, `_CHAIN_FAIL`, `_BASE_CURVE_CONSUMED`), TRIM/SPLIT/EXTEND/PATTERN/FILLET/MIRROR/TRANSFORM `_NO_SPLINE_HANDLES` and `_NO_SPLINE_CONTROL_POLYGON`, `SKETCH_SPLIT_NO_BEZIER`, `SKETCH_TRIM_NO_BEZIER` (internal), MIRROR (`_NEED_VALID_MIRROR_LINE`, `_NEED_ENTITIES_TO_MIRROR`, `_OFFSET_SPLINE`, `_OFFSET_ELLIPSE`, `_CURVE_POINT`), PATTERN (`_TOO_LARGE`, `_NEEDS_SEED`, `_INVALID_VALUE`, linear `_ZERO_LENGTH`, `_PARALLEL_DIRECTIONS`, circular `_ZERO_ANGLE`), USE/PROJ/SIL_PROJECTION (`_FAILED`, `_PARTIAL`), INTERSECTION (`_FAILED`, `_MULTIPLE_`, `_PARTIAL_`), SLOT, INSERT_DWG, TRANSFORM.
- Sheet metal: `SKETCH_CONSTRAINT_FLAT_IN_3D`, `_3D_IN_FLAT`, `SKETCH_REFERENCE_WRONG_FLAT_PART`, `SKETCH_CANNOT_REFERENCE_3D`.
- Solve: `SKETCH_SOLVE_FAILED`, `SKETCH_UNSOLVABLE_CONSTRAINT`, `SKETCH_EXCEEDS_BOUNDS`, `SKETCH_TIMED_OUT`, `SKETCH_EXTERNAL_GEOMETRY_MISMATCH`, `SKETCH_MISSING_LOCAL_REFERENCE`.

## 11. Sketch entity model (for the data layer) [FS]
- Point `skPoint{position}`. Line `skLineSegment{start,end,construction}`, with sub-ids `.start`/`.end`. Circle `{center,radius}`. Ellipse `{center,majorRadius,minorRadius,majorAxis}`. Arc `{start,mid,end}`. EllipticalArc `{center,majorAxis,majorRadius,minorRadius,startParameter,endParameter}`. Interpolated/fit spline `{points,parameters,startDerivative,endDerivative}`. Bezier `{points}`. Conic segment `{start,controlPoint,end,rho,fixedRho}`. Text `{text,fontName,firstCorner,secondCorner,mirrorHorizontal,mirrorVertical}`. Image `{blobInfo,firstCorner,secondCorner}`. Rectangle (4 lines + 8 constraints). Regular polygon (a polyline, optionally constrained). All curves support `construction:boolean`.
- `skSolve` solves. `skSetInitialGuess` seeds positions. Constraints are `skConstraint{constraintType, localFirst, localSecond, length?, angle?, …}` [FS].
- Sketch circle/ellipse radius bounds: min 5e-6 m, default 0.025 m (=25 mm / 1 in), max 500 m [FS `SKETCH_RADIUS_BOUNDS`].

## 12. Gaps / not documented
- Exact toolbar pixel order and which dropdowns exist beyond the documented Line and Use dropdowns. §2.2 groupings marked *inferred* follow the bundle enum adjacency.
- Exact auto-constraints for the interactive aligned/center rectangles, polygons and midpoint line (FS gives only the corner-rectangle set).
- Profile inspector issue wording and categories beyond "unclosed loops".
- Curvature G3 UI placement and glyph.
- Constraint glyph icon artwork (docs images only).
- A "Lock" option in the Sketch dialog does not exist in any source.
