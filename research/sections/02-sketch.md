## 2. Sketch

Scope: the Part Studio Sketch feature and everything inside an active sketch: plane selection, solver and DOF
status, inferencing, dragging, regions, display/diagnostic options (SKB-), sketch entities (SKE-), sketch tools
(SKT-), geometric constraints (SKC-) and dimensions (SKD-). Help URL base: `https://cad.onshape.com/help/Content/Sketch/<page>.htm`
(abbreviated below as `Sketch/<page>`). FS = FeatureScript std library `sketch.fs` unless noted. Sketch entity creation
in Onshape is interactive (client-side solver); the FS functions (`skLineSegment`, `skArc`, ...) are the
parameter-level data model for each entity, and the `ConstraintType` enum (constrainttype.gen.fs) is the
complete list of persisted constraint/dimension kinds:
`NONE, COINCIDENT, PARALLEL, VERTICAL, HORIZONTAL, PERPENDICULAR, CONCENTRIC, MIRROR, MIDPOINT, TANGENT, EQUAL, LENGTH,
DISTANCE, ANGLE, RADIUS, NORMAL, FIX, PROJECTED, OFFSET, CIRCULAR_PATTERN, PIERCE, LINEAR_PATTERN, MAJOR_DIAMETER,
MINOR_DIAMETER, QUADRANT, DIAMETER, SILHOUETTED, CENTERLINE_DIMENSION, INTERSECTED, RHO, EQUAL_CURVATURE, BEZIER_DEGREE, FREEZE`.
Related internal enums: `SketchToolType {NONE, EXTEND, MIRROR, FILLET, OFFSET, USE, TRIM, PATTERN, SLOT, SPLIT, SECTION, CHAMFER}`,
`DimensionDirection {MINIMUM, HORIZONTAL, VERTICAL}`, `DimensionAlignment {UNSPECIFIED, ALIGNED, ANTI_ALIGNED}`,
`DimensionHalfSpace {UNSPECIFIED, LEFT, RIGHT}`, `RadiusDisplay {NONE, RADIAL, DIAMETRAL}`,
`SketchProjectionType {USE, SILHOUETTE_START, SILHOUETTE_END, USE_END}`,
`SketchSilhouetteDisambiguation {NONE, ALIGNED, ANTI_ALIGNED, OUTER, INNER, TORUS_TOP, TORUS_BOTTOM, TORUS_LEFT, TORUS_RIGHT}`,
`FixedParameterPosition {FIXED_AT_START, FIXED_AT_END, FIXED_ON_CURVE, FREE}`.

ConstraintType -> spec item: COINCIDENT SKC-01; CONCENTRIC SKC-02; PARALLEL SKC-03; TANGENT SKC-04; HORIZONTAL SKC-05;
VERTICAL SKC-06; PERPENDICULAR SKC-07; EQUAL SKC-08; MIDPOINT SKC-09; NORMAL SKC-10; PIERCE SKC-11; MIRROR (Symmetric) SKC-12;
FIX SKC-13; EQUAL_CURVATURE (Curvature) SKC-14; QUADRANT SKC-15; PROJECTED (Use) SKC-16; SILHOUETTED SKC-17;
INTERSECTED SKC-18; OFFSET SKC-19; LINEAR_PATTERN SKC-20; CIRCULAR_PATTERN SKC-21; BEZIER_DEGREE SKC-22; FREEZE SKC-23;
LENGTH SKD-02/SKD-12; DISTANCE SKD-03..06 (+SKD-07); ANGLE SKD-08; RADIUS SKD-09; DIAMETER SKD-10;
MAJOR_DIAMETER/MINOR_DIAMETER SKD-11; CENTERLINE_DIMENSION SKD-14; RHO SKD-15; NONE = placeholder.

Common 2D geometry kernel for all entities (OCCT): `gp_Pnt2d/gp_Vec2d/gp_Dir2d/gp_Ax2d/gp_Ax22d`, `Geom2d_Line`,
`Geom2d_Circle`, `Geom2d_Ellipse`, `Geom2d_TrimmedCurve`, `Geom2d_BSplineCurve`, `Geom2d_BezierCurve`, `Geom2d_OffsetCurve`,
constructors `GCE2d_MakeSegment`, `GCE2d_MakeCircle`, `GCE2d_MakeArcOfCircle`, `GCE2d_MakeEllipse`, `GCE2d_MakeArcOfEllipse`,
`gce_MakeCirc2d`; intersection `Geom2dAPI_InterCurveCurve`; projection `Geom2dAPI_ProjectPointOnCurve`; extrema
`Geom2dAPI_ExtremaCurveCurve`; tangency solving `Geom2dGcc_*` / `GccAna_*`; conversion to 3D via `GeomAPI::To3d(curve2d, gp_Pln)`
and `BRepBuilderAPI_MakeEdge(Handle(Geom2d_Curve), Handle(Geom_Plane))`.

---

### SKB-01 Sketch feature
- **Purpose:** Part Studio feature that holds a planar set of 2D curves, points, text, images, constraints and dimensions; output is wire/point bodies and region surface bodies used by downstream features.
- **Sources:** Sketch/sketch_basics, Sketch/sketch_tools; FS: `newSketch`, `newSketchOnPlane`, `skSolve` (sketch.fs). Shortcut: Shift+s (Part Studio, "Opens the Sketch feature. Prompts Select a sketch plane").
- **Selection inputs:** "Sketch plane": `(GeometryType.PLANE && AllowFlattenedGeometry.NO) || (SheetMetalDefinitionEntityType.FACE && AllowFlattenedGeometry.YES && GeometryType.PLANE) || BodyType.MATE_CONNECTOR`; MaxNumberOfPicks 1. I.e. a default/construction plane, a planar part face, a planar face of a flattened sheet metal definition, or a mate connector (explicit or implicit).
- **Parameters:**

| FS id | UI label | Type | Enum values / default / bounds+units | Visible when / notes |
|---|---|---|---|---|
| sketchPlane | Sketch plane | Query | 1 planar entity or mate connector | always |
| disableImprinting | Disable imprinting | boolean | default false; UIHint REMEMBER_PREVIOUS_VALUE | shown (only defined when present in the definition; newer sketches). Help: shown when sketching on a face of an existing part |
| (UI only) | Show constraints | checkbox | - | see SKB-04 |
| (UI only) | Show expressions | checkbox | resets to unchecked when dialog reopened | see SKB-04 |
| (UI only) | Show errors | checkbox | - | mentioned in Constraint manager notes; see SKB-04 |
| (UI only) | Mate connector icon | button | toggles display of implicit mate connectors for plane picking | see SKB-02 |
| (UI only) | Show sketch diagnostic tools | menu | Profile inspector, Constraint manager | see SKB-16/17 |

- **Behavior / semantics:**
  - Created from the Feature toolbar Sketch button or Shift+s, or context menu "New sketch" on a selected plane/face. Dialog remembers its options between uses ("Onshape remembers your selected options in the Sketch dialog").
  - Sketch coordinate system (from `newSketch`): if the plane reference is a plane/face, the sketch plane is re-aligned canonically (`alignCanonically`) and its origin is the projection of the world origin onto the plane; if a mate connector, the sketch uses the mate connector's coordinate system (origin and X axis) unchanged.
  - When the feature is inside a feature pattern/mirror, the plane is transformed by the pattern transform (sketch drawn in original frame, then transformed).
  - Solve: all constraints solved (`skSolve`); then each non-construction edge becomes a `WIRE` body, each enclosed region becomes a planar `SURFACE` body, and each standalone vertex (points, circle centers) becomes a `POINT` body. Construction geometry is displayed but not output as feature geometry (see SKB-14).
  - Accept (checkmark) records the feature; Cancel (red X) discards all actions taken while the dialog was open; a "Restore" link in the message bubble undoes the cancel.
  - Opening Extrude (Shift+e) or Revolve (Shift+w) from the sketch toolbar accepts the sketch and opens the feature with all closed regions pre-selected.
  - Multiple sketches per Part Studio; one plane per sketch.
- **Edge cases & errors:** `SKETCH_NO_PLANE` "Select a sketch plane." (falls back to XY plane for evaluation); `MESH_NOT_SUPPORTED` when plane is a mesh face; `SHEET_METAL_ACTIVE_MODEL_REQUIRED` when picking a flattened face of a non-active sheet metal model; `FLATTENED_SHEET_METAL_SKETCH_PROHIBITED`, `FLATTENED_SHEET_METAL_SKETCH_ONE_FACE`, `SHEET_METAL_SKETCH_DETACHED_FACE`, `SKETCH_REFERENCE_WRONG_FLAT_PART`, `SKETCH_CONSTRAINT_WRONG_SHEET_METAL_BODY` (flat-pattern sketches); `SKETCH_SOLVE_FAILED`, `SKETCH_EVALUATION_FAILED`, `SKETCH_UPDATE_FAILED`, `SKETCH_SOLVER_NOT_INITIALIZED`, `SKETCH_EXCEEDS_BOUNDS` "creates geometry that is beyond the system limit".
- **OCCT mapping:** `gp_Pln`/`Geom_Plane` from `BRepAdaptor_Surface` of the picked face (check `GeomAbs_Plane`), or from the mate connector `gp_Ax3`; entities stored in 2D (`Geom2d_*`), converted to edges with `BRepBuilderAPI_MakeEdge(c2d, plane)` + `BRepLib::BuildCurves3d`; wire bodies via `BRepBuilderAPI_MakeWire`/`TopoDS_Compound`; regions via SKB-06.
- **Priority:** P0

### SKB-02 Sketch plane selection and plane frame
- **Purpose:** Define where and in which 2D frame a sketch lives.
- **Sources:** Sketch/sketch_basics; FS `newSketch` (sketch.fs); PartStudio mate connector pages.
- **Selection inputs:** see SKB-01 filter. One pick.
- **Parameters:** `sketchPlane` (see SKB-01).
- **Behavior / semantics:**
  - Allowed: Front/Top/Right default planes, Plane feature results, planar part faces, planar surface faces, implicit mate connectors (hover a face/edge with the Mate connector icon active: circle centers, square corners, centroids "wake up"), explicit mate connectors; after choosing a mate connector, clicking the icon in the field opens an inline Mate connector editor (offset/rotate it).
  - Frame rule: planes/faces -> canonical axes, origin = projection of world origin onto the plane (so the Part Studio origin projects into the sketch as a reference point); mate connector -> its own origin/X/Y.
  - "Sketch on face offset": Onshape has no offset parameter on the sketch feature; to sketch at an offset the user creates an offset Plane (cPlane) first or uses a mate connector with an offset (inferred from the precondition, which has no offset field).
  - Changing the sketch plane on edit re-solves all entities in the new frame; external references (Use/Pierce) are re-evaluated (inferred).
  - Flattened sheet metal faces are allowed only on an active sheet metal model (sketch on flat pattern).
- **Edge cases & errors:** `SKETCH_NO_PLANE`; non-planar face rejected by filter; mesh faces rejected (`MESH_NOT_SUPPORTED`).
- **OCCT mapping:** `BRepAdaptor_Surface::Plane()`, `gp_Ax3` for mate connector; canonical alignment = choose X axis among world axes most aligned with the plane (custom, mirrors `alignCanonically`); `ElSLib::Parameters` / `gp_Pln::Location` projection for origin.
- **Priority:** P0

### SKB-03 Sketch editing workflow and toolbar
- **Purpose:** Interaction model for creating/editing sketches.
- **Sources:** Sketch/sketch_basics, Sketch/sketch_tools, Home/keyboard_shortcuts_and_hotkeys, Home/context_menus.
- **Selection inputs:** n/a.
- **Parameters:** n/a (UI behavior).
- **Behavior / semantics:**
  - Edit: double-click the sketch in the Feature list, or context menu "Edit". Edited sketches re-solve and downstream features regenerate on accept. "Roll to here" rolls the feature list back to the sketch.
  - Sketch toolbar groups tools into drop-downs; the group button shows the last tool used from the group.
  - Esc exits the current tool. Most tools stay active for repeated use (e.g., Line chains segments; constraint tools toggle on and apply to each picked pair until toggled off or another tool is chosen).
  - Pre-selection or post-selection: constraints/tools can be applied by selecting entities then clicking the tool, or clicking the tool then selecting entities.
  - Shortcut toolbar: press `s` in an active sketch to open a customizable radial/shortcut toolbar at the cursor. Search tools: alt/opt+c.
  - Shortcut keys require a sketch plane to be selected (full table in SKB-24).
  - Undo/redo (ctrl+z / ctrl+y) operate on sketch edits inside the open sketch; DXF insert is recorded in the undo stack.
  - Context menu in the graphics area of an open sketch gives Transform sketch entities, Copy sketch, Paste into sketch, Edit text, Edit image, Edit pattern, Change to driving/driven dimension, Add comment, Confirm spline / Escape bezier, Curvature color map.
  - Selection priority: when points overlap, non-fully-constrained points (blue/red) are picked before fully constrained (black) points.
  - Measure tool and Mass properties are available while sketching.
- **Edge cases & errors:** `SKETCH_NOT_ACTIVE` "There is no active sketch."; `SKETCH_MODIFICATION_FAILED`.
- **OCCT mapping:** none (app UI / command framework).
- **Priority:** P0

### SKB-04 Sketch dialog display toggles (Show constraints / Show expressions / Show errors / Show dimensions)
- **Purpose:** Control annotation display in the active sketch.
- **Sources:** Sketch/sketch_basics, Sketch/working_with_constraints, Sketch/dimension, Home/context_menus.
- **Selection inputs:** n/a.
- **Parameters:**

| FS id | UI label | Type | Enum values / default / bounds+units | Visible when / notes |
|---|---|---|---|---|
| (UI) | Show constraints | checkbox | default off (inferred) | shows all constraint icons; when off, constraint icons are shown only for the hovered entity |
| (UI) | Show expressions | checkbox | default off; auto-unchecked when the dialog is closed and reopened | displays the dimension's expression text above its value; display truncated at 70 characters |
| (UI) | Show errors | checkbox | - | shows constraint-error indicators (named in Constraint manager notes; behavior inferred) |
| (context menu on sketch in Feature list) | Show dimensions | command | - | temporarily shows a closed sketch's dimensions; clicking elsewhere hides them |

- **Behavior / semantics:**
  - Without Show constraints: hovering an entity shows only its constraints; holding Shift while moving the mouse keeps all encountered constraints visible.
  - Dimension decorations: an "expression without variable" icon or "expression with variable" icon is drawn left of a dimension value whose value is an expression.
- **Edge cases & errors:** none documented.
- **OCCT mapping:** none (render layer).
- **Priority:** P1

### SKB-05 Sketch output data model and entity identity
- **Purpose:** Define what a solved sketch produces and how downstream features reference it.
- **Sources:** sketch.fs module doc; query.fs `qSketchRegion`, `qSketchFilter`, `SketchObject`; FsDoc `sketchEntityQuery`, `startTracking(context, sketchId, sketchEntityId)`; evaluate.fs `evOwnerSketchPlane`.
- **Selection inputs:** n/a.
- **Parameters:** n/a.
- **Behavior / semantics:**
  - Each sketch entity has a stable string id (e.g. `"line1"`, composite entities use dotted sub-ids such as `rectangle1.left`, `rectangle1.left.start`, `polyline1.line0`, `polygon1.line3`). Point sub-ids: `.start`, `.end`, `.center` (returned as `startId`, `endId`, `centerId`).
  - Every solved non-construction curve -> one wire body edge (sketch edges are queryable with `sketchEntityQuery(featureId, EntityType.EDGE, entityId)`); regions -> surface bodies (`qSketchRegion(featureId, filterInnerLoops)`); isolated points and circle/arc/ellipse centers -> point bodies.
  - `qSketchFilter(q, SketchObject.YES/NO)` distinguishes sketch vs non-sketch entities (used e.g. by selection filters).
  - `evOwnerSketchPlane` returns the sketch plane of any sketch-created entity (used by extrude direction, hole, rib, loft).
  - Identity must survive edits so downstream references (extrude regions, edges) stay attached: region identity is derived from bounding sketch entity ids (inferred).
  - Persisted solved state: `skSetInitialGuess(sketch, map entityId -> number[])` stores the last solved parameter values so re-solve starts from the previous configuration (initial guess).
- **Edge cases & errors:** `SKETCH_MISSING_ENTITY`, `SKETCH_ENTITY_NOT_FOUND`, `SKETCH_ENTITY_PARAMETER_NOT_FOUND`, `SKETCH_CONSTRAINT_NOT_FOUND`, `SKETCH_CONSTRAINT_PARAMETER_NOT_FOUND`.
- **OCCT mapping:** app data model; `TNaming`/custom naming keyed by entity ids; `TopoDS_Compound` of wire bodies.
- **Priority:** P0

### SKB-06 Region detection
- **Purpose:** Compute closed planar regions (faces) from sketch curves for Extrude/Revolve/Sweep/Loft/etc.
- **Sources:** query.fs `qSketchRegion` doc; Home/glossary ("Sketch region: A finite area in a sketch defined by a bounding set of sketch curves"); PartStudio/extrude ("automatically selects all the closed regions in the sketch, and if present, nested entities").
- **Selection inputs:** n/a (downstream features pick regions).
- **Parameters:** `filterInnerLoops` (qSketchRegion): default false; when true excludes regions fully contained in other regions ("A region whose border has a vertex or edge on the outside boundary is not considered contained").
- **Behavior / semantics:**
  - Input curves: all non-construction curves (lines, arcs, circles, ellipses, conics, splines, beziers, text glyph outlines, imprinted plane-face edges unless Disable imprinting, Used/projected curves that are not construction).
  - Algorithm: build the planar arrangement of all curves (split every curve at every intersection/touch point), then every bounded face of the arrangement is one region. Overlapping closed shapes produce all sub-faces (e.g. two overlapping circles -> 3 regions). Nested loops: the annulus between an outer and inner loop is a region; the inner area is a separate region. Dangling/open curves do not bound regions but do split regions they cross completely (inferred from standard behavior).
  - Points, construction curves, images are ignored.
  - Regions are recomputed on every solve.
- **Edge cases & errors:** `EXTRUDE_NO_REGION_IN_SKETCH` "Selected sketch contains no region (has only open curves)"; self-intersecting curves: `SELF_INTERSECTING_CURVE_SELECTED` / `SKETCH_BAD_SPLINE`; tiny gaps -> no region (Profile inspector SKB-17 helps find).
- **OCCT mapping:** Option A (OCCT only): make 3D edges on the plane, `BOPAlgo_Builder` (General Fuse) to split all edges at intersections, then `BOPAlgo_BuilderFace` with a large bounding planar face (or `BOPAlgo_Tools::EdgesToWires` + `BOPAlgo_Tools::WiresToFaces`), discard the unbounded outer face; `BRepBuilderAPI_MakeFace(plane, wire)` for each loop, `ShapeAnalysis_FreeBounds` for loop detection. Option B (recommended for interactive speed, design suggestion): custom 2D DCEL arrangement using `Geom2dAPI_InterCurveCurve` for splitting, then build faces with `BRepBuilderAPI_MakeFace`; classify nesting with `BRepClass_FaceClassifier` / signed area (`BRepGProp::SurfaceProperties`).
- **Priority:** P0

### SKB-07 Imprinting of the sketch-plane face (Disable imprinting)
- **Purpose:** When sketching on a part face, the face's boundary edges participate in region splitting unless disabled.
- **Sources:** Sketch/sketch_basics; FS `newSketch` `disableImprinting` ("Prevents sketchPlane from imprinting on the sketch").
- **Selection inputs:** n/a.
- **Parameters:** `disableImprinting` boolean, default false, remembers previous value.
- **Behavior / semantics:**
  - false: the edges of the selected planar face (the face boundary) become part of the sketch's region computation, so sketch curves crossing the face boundary are split into separate regions (e.g. circles crossing the face edge -> two regions each).
  - true: only sketch curves define regions (whole circles remain single regions).
  - Imprinted edges are not editable sketch entities (inferred).
- **Edge cases & errors:** no effect when sketch plane is a datum plane or mate connector (planeReference is nothing for mate connectors).
- **OCCT mapping:** include `TopExp::MapShapes(face, TopAbs_EDGE)` edges as extra non-output splitters in the SKB-06 arrangement.
- **Priority:** P1

### SKB-08 Constraint solver
- **Purpose:** Solve the geometric constraint system (constraints + driving dimensions) to place all entity parameters; drives DOF status, drag, and dimension edits.
- **Sources:** sketch.fs (`skSolve`, `skConstraint`, `skSetInitialGuess`); Sketch/working_with_constraints; Sketch/troubleshooting_sketch_geometry; Sketch/dimension. Onshape's solver internals are not documented.
- **Selection inputs:** n/a.
- **Parameters:** `skConstraint` value map: `constraintType` (ConstraintType), `localFirst`, `localSecond` (sketch entity ids, incl. point sub-ids), `length` (ValueWithUnits, length units), `angle` (ValueWithUnits, angle units), `direction` (DimensionDirection MINIMUM/HORIZONTAL/VERTICAL; for DISTANCE), `alignment` (DimensionAlignment, used to fix the sign/orientation of a directed distance or line alignment). Wrong units -> value treated as undefined, constraint in error.
- **Behavior / semantics (Onshape-observable):**
  - Solve starts from the previous solution (initial guess) and converges to the nearest configuration; geometry not pinned by constraints keeps its current position as much as possible.
  - When a dimension value changes, both dimensioned entities move by half the delta (except radius/diameter), unless constrained otherwise (see SKD-01).
  - Over-constraining additions: new dimensions that would over-define are automatically made driven (SKD-16); conflicting constraints are shown red (SKB-10).
  - Constraints to external geometry (Use, Pierce, plane references) treat the external geometry as fixed.
- **Implementation note (design suggestion):**
  - Parameterization: points (x,y) are the primitive unknowns; line = 2 points; circle = center point + r; arc = center + r + start/end points with on-circle residuals (planeGCS style) or center + r + 2 angles (SolveSpace style); ellipse = center + major-axis focus/direction + major/minor radius; conic = start, end, control point + rho; interpolated spline = through-points + end tangent handle vectors (+ internal tangent/handle params); bezier = control points; text/image = 4 corner points with rectangle constraints.
  - Residuals per constraint as in SKC-xx/SKD-xx items; solve with Newton/Gauss-Newton + Powell DogLeg (planeGCS default) with Levenberg-Marquardt fallback; scale variables by sketch size.
  - Decompose the constraint graph into connected components (and optionally rigid clusters) and solve each independently; reuse the factorization during drag.
  - DOF analysis: Jacobian J (m x n) at the solution; rank via column-pivoted QR (or SVD for small systems). DOF = n - rank(J). Redundant constraints = rows dependent on others (QR of J^T); if a redundant row has nonzero residual -> conflicting (red), else redundant-but-consistent (Onshape treats as over-defined when added; reject or make driven). Per-entity "fully defined": an entity is black iff all its parameter columns have zero component in the null space of J (null-space basis from SVD/QR); external/fixed geometry excluded from unknowns.
  - Drag: solve min ||x - x_prev||^2 subject to constraints, with the dragged point's target as a strong soft constraint (SolveSpace "dragged params" approach); fall back to last good state on failure (`SKETCH_DRAG_ERROR`).
  - Rust crates: `nalgebra`/`faer` for dense/sparse QR/SVD; port planeGCS (LGPL, FreeCAD) or SolveSpace (GPLv3; license implications) as references; Onshape-like commercial option is a D-Cubed 2D DCM-style solver (not open).
- **Edge cases & errors:** `SKETCH_SOLVE_FAILED`, `SKETCH_UNSOLVABLE_CONSTRAINT` "Some constraints are not applicable to the current external references and have not been solved", `SKETCH_EXTERNAL_GEOMETRY_MISMATCH` "External geometry included in the sketch has changed types", `SKETCH_DIMENSION_INFINITY`, `SKETCH_DIMENSION_LIMIT_ERROR`, `SKETCH_ADD_CONSTRAINT_FAILED`, `SKETCH_CONSTRAINT_NEEDS_SKETCH_ENTITY` "A constraint must involve something from the sketch", `SKETCH_CONSTRAINT_UNKNOWN`.
- **OCCT mapping:** none in OCCT (no 2D GCS); custom solver. OCCT used only to evaluate geometry for residuals of curve-based constraints (`Geom2d_Curve::D1/D2`, `Geom2dAPI_ProjectPointOnCurve`).
- **Priority:** P0

### SKB-09 Constraint status coloring (DOF display)
- **Purpose:** Visual feedback of definition state.
- **Sources:** Sketch/troubleshooting_sketch_geometry, Sketch/working_with_constraints, Sketch/sketch_basics.
- **Selection inputs:** n/a.
- **Parameters:** n/a.
- **Behavior / semantics:**
  - Entity colors: blue = under-constrained; black = fully constrained; red = constraint problem (over-constrained/conflicting).
  - Constraint icon colors: black glyph on gray = well-defined; white glyph on red = problem.
  - Driving dimensions black; driven dimensions light gray.
  - Constraints referencing other sketches/external geometry (e.g. Use) have a blue icon background, darker blue on hover.
  - Hover highlight orange; related entities of a selected constraint highlighted yellow.
  - Points: when not adjusting a spline, sketch points are white with a blue border; spline control points are solid blue.
  - Line styles: active sketch lines solid; inactive (other) sketches lighter; construction lines dashed; sketch lines hidden behind model drawn lighter; used (projected) edges distinct style (exact colors are user-configurable in visualization settings: active/inactive sketch line width and colors, construction dash/gap lengths).
- **Edge cases & errors:** none.
- **OCCT mapping:** none (derive from SKB-08 DOF analysis).
- **Priority:** P0

### SKB-10 Over-constraint and conflict reporting
- **Purpose:** How problems are reported and resolved.
- **Sources:** Sketch/working_with_constraints, Sketch/dimension, Sketch/troubleshooting_sketch_geometry.
- **Selection inputs:** n/a.
- **Parameters:** n/a.
- **Behavior / semantics:**
  - Adding a dimension that over-defines the sketch: the dimension is automatically created as driven (reference).
  - Adding a constraint that conflicts, or switching a driven dimension to driving such that the sketch is over-constrained: involved entities turn red and conflicting constraint icons show white on red; the sketch feature reports an error state in the Feature list (inferred: feature shown red with error tooltip) while still outputting the last solvable geometry.
  - Constraint manager "Status: Errors" filter lists constraints in error (SKB-16).
  - The user resolves by deleting constraints/dimensions (icon click + Delete) or toggling dimensions driven.
- **Edge cases & errors:** `SKETCH_SOLVE_FAILED`, `SKETCH_UNSOLVABLE_CONSTRAINT`.
- **OCCT mapping:** none; requires redundancy/conflict detection from the solver (SKB-08).
- **Priority:** P0

### SKB-11 Dragging geometry
- **Purpose:** Direct manipulation of under-constrained geometry.
- **Sources:** Sketch/troubleshooting_sketch_geometry ("Dragging entities may help you understand what constraints or dimensions you may want to add"); Sketch/text; Sketch/insert_image; Sketch/spline; Sketch/sketch_basics (transform free drag); Sketch/sketch_tools (iOS tip: with no tools active, drag points or entities).
- **Selection inputs:** with no tool active, press-drag a point, curve, spline handle, text box corner, image corner, constraint icon, or dimension.
- **Parameters:** n/a.
- **Behavior / semantics:**
  - Dragging a point moves it subject to all constraints; dragging a curve translates it (or reshapes it, e.g. circle edge drag changes radius) while satisfying constraints; fully defined geometry does not move.
  - Inferences fire while dragging (point can snap/create coincident on release); hold Shift to suppress automatic inferences while dragging.
  - Spline: drag through-points and tangent handles (white) along/away from the spline to change tangency direction/magnitude.
  - Text box: dragging an unconstrained text box moves it; during drag only the box is displayed, text appears when stationary; resizing requires one corner fixed; dimensioned text cannot be resized by drag; rotation requires removing the default horizontal constraint and fixing a corner.
  - Images: become semi-transparent during placement/move.
  - Constraint icons can be dragged to reposition; dimensions dragged to reposition text/witness.
- **Edge cases & errors:** `SKETCH_DRAG_ERROR` "Could not dynamically update sketch during drag.", `SKETCH_DRAG_NO_SKETCH`.
- **OCCT mapping:** none (solver drag mode, SKB-08).
- **Priority:** P0

### SKB-12 Automatic inferencing and snapping
- **Purpose:** Create constraints automatically while sketching/dragging based on cursor proximity.
- **Sources:** Sketch/automatic_inferencing, Sketch/sketch_basics, Sketch/working_with_constraints, Sketch/3_point_arc, Sketch/elliptical_arc, Home/glossary ("Inference: ... appears as an orange dotted line as well as orange boxes highlighting related entities"), Home/keyboard_shortcuts (Shift).
- **Selection inputs:** cursor hover.
- **Parameters:** n/a.
- **Behavior / semantics:**
  - Always-on inferences: horizontal and vertical alignment of the cursor/new point with the sketch origin and with other entity points (dotted line shown), horizontal/vertical orientation of a line being drawn.
  - "Wake-up" inferences: only after the cursor has hovered near an entity: commonly horizontal/vertical alignment to that entity's points, midpoint, parallel, perpendicular, coincident; tangent when starting an arc from a curve end; quadrant points of ellipses (Quadrant constraint); circle/arc centers (concentric/coincident).
  - Snap targets: sketch points (endpoints, centers, midpoints), curves (on-curve coincident), origin, coplanar vertices and edges of other sketches and part features (creates an inferred constraint to that external entity, e.g. a circle snapped to a gear vertex in another sketch), ellipse major/minor axis and quadrant points.
  - Visual: when the drawn line becomes parallel to another, it turns dotted and the related line highlights orange with the constraint icon at the cursor; constraint indicator icons appear next to the cursor while snapping.
  - Semicircle: in 3 point arc, moving until endpoints and midpoint align snaps to a semicircle inference.
  - On click/release, each displayed inference becomes a persisted constraint (coincident, horizontal, vertical, parallel, perpendicular, tangent, midpoint, quadrant, etc.).
  - Suppression: hold Shift while placing or dragging to suppress automatic inferences.
- **Edge cases & errors:** `SKETCH_INFERENCE_FAILED` "Could not add inferences."
- **OCCT mapping:** custom (2D proximity queries; candidate generation with tolerance in screen pixels; `Geom2dAPI_ProjectPointOnCurve` for on-curve snaps; external coplanar edges/vertices via `BRepExtrema_DistShapeShape` or projecting to plane and checking distance to plane).
- **Priority:** P0

### SKB-13 Dimension while sketching (on-the-fly value entry)
- **Purpose:** Type dimension values immediately after creating geometry.
- **Sources:** Sketch/sketch_basics; every entity page ("Optionally, immediately after ... input a numerical value (or type "=#" to input a variable)").
- **Selection inputs:** n/a.
- **Parameters:** per tool: Line -> length; Corner/Center/Aligned rectangle -> length then width; Circle (center, 3-point) -> diameter; arcs -> radius; polygons -> number of sides then diameter of construction circle; ellipse/elliptical arc -> primary then secondary axis diameter; conic -> rho; slot -> width (diameter); fillet -> radius; chamfer -> distances; offset -> distance; patterns -> count/spacing/angle.
- **Behavior / semantics:**
  - A suggested value appears in a box; typing a value or expression and pressing Enter creates a driving dimension; continuing to sketch leaves it undimensioned.
  - Alt+arrow keys toggle between multiple value boxes (e.g. rectangle length/width).
  - Variable syntax while sketching: "=#name" (a bare "#" while sketching is the view shortcut and changes to Left view). In an existing dimension's edit field: "#name" without "=". Typing "#" in a dimension field shows a list of existing variables plus "New variable" (PartStudio/variable).
- **Edge cases & errors:** `SKETCH_INFER_DIM_FAILED` "Could not infer sketch dimension value."
- **OCCT mapping:** none.
- **Priority:** P0

### SKB-14 Construction geometry and construction mode
- **Purpose:** Reference geometry that participates in constraints but not in features.
- **Sources:** Sketch/construction; FS `construction` boolean on every entity function. Shortcut: q.
- **Selection inputs:** optional pre-selected sketch entities.
- **Parameters:**

| FS id | UI label | Type | Enum values / default / bounds+units | Visible when / notes |
|---|---|---|---|---|
| construction | Construction | boolean | default false | field exists on skLineSegment, skArc, skCircle, skEllipse, skEllipticalArc, skFitSpline, skBezier, skRectangle, skPolyline, skRegularPolygon, skConicSegment, skText (not on skPoint/skImage) |

- **Behavior / semantics:**
  - With entities selected, q (or the tool button) toggles their construction state.
  - With nothing selected, q toggles construction mode on; subsequent tools create construction geometry until toggled off.
  - Construction curves are drawn dashed, excluded from regions and output wire bodies; still constrainable/dimensionable.
  - Tools auto-create construction helpers: polygon circle, elliptical-arc temporary ellipse (deleted after), fillet virtual sharp point, transform-generated helpers, text/image boxes (inferred construction-like frames).
  - Construction lines are the reference for centerline dimensions (SKD-14).
- **Edge cases & errors:** `SKETCH_TEXT_CANNOT_BE_CONSTRUCTION` "Cannot set construction on sketch text." (help-UI; FS skText still accepts the field); `SKETCH_CONSTRUCTION_POINT_FAILED`.
- **OCCT mapping:** app data flag; filter before edge/region generation.
- **Priority:** P0

### SKB-15 Constraint display and interaction
- **Purpose:** How constraints are shown, selected, moved, deleted.
- **Sources:** Sketch/working_with_constraints.
- **Selection inputs:** constraint icons.
- **Parameters:** n/a.
- **Behavior / semantics:**
  - Constraint icons are drawn near constrained entities; groups of icons can be dragged to a new location.
  - Hover an icon to highlight constrained entities; select an icon and press Delete (or context menu Delete) to remove the constraint.
  - Hovering an entity shows only that entity's constraints (unless Show constraints); Shift keeps them visible while moving.
  - Constraints applied while sketching (inferences) and manually are identical objects.
  - External-reference constraints: blue background; hover darker blue.
  - Auto-created constraint kinds not on the toolbar: Quadrant, Use (Projected), Intersection (Intersected), plus Offset, pattern, silhouette (see SKC-15..23).
- **Edge cases & errors:** none.
- **OCCT mapping:** none.
- **Priority:** P0

### SKB-16 Constraint manager
- **Purpose:** List, filter and delete constraints/dimensions to diagnose errors.
- **Sources:** Sketch/working_with_constraints (Constraint manager section).
- **Selection inputs:** optional box selection of entities in the sketch to limit the list.
- **Parameters:**

| FS id | UI label | Type | Enum values / default / bounds+units | Visible when / notes |
|---|---|---|---|---|
| (UI) | Filters | expand/collapse all | - | sections Type, Mode, Status |
| (UI) | Automatically select constraints | toggle | default on | on: picked filter results get yellow bounding boxes in the sketch (selected); off: filters apply only to the current entity selection |
| (UI) | Type | multi-select | each constraint type (Coincident, ...) | |
| (UI) | Mode | multi-select | Internal (current sketch), External (external sources), In context (in-context references) | |
| (UI) | Status | multi-select | Driven, Solved, Errors | Driven/Solved highlight yellow (darker blue if under-constrained); Errors highlight darker red |
| (UI) | Sort | radio | Sort by constraint / Sort by entity | |

- **Behavior / semantics:** Entities-and-constraints list; hover highlights in sketch; per-row Delete constraint and Remove selection; row context menu: Delete constraint/entity, Zoom to selection, Hide external references, Clear all; "Delete all" deletes all listed constraints. When Show constraints and Show errors are off, hovering a row shows only that constraint. Works in sheet metal flat pattern sketches. Close with X or diagnostics menu "Turn constraint manager off".
- **Edge cases & errors:** none.
- **OCCT mapping:** none (UI over solver data).
- **Priority:** P1

### SKB-17 Profile inspector
- **Purpose:** Find open loops/gaps preventing regions.
- **Sources:** Sketch/troubleshooting_sketch_geometry; glossary ("easily identify loose ends in a sketch").
- **Selection inputs:** n/a.
- **Parameters:** n/a (dialog with issue list, Previous/Next buttons).
- **Behavior / semantics:** Opened via Sketch dialog > Show sketch diagnostic tools > Profile inspector. Lists issues such as "unclosed loop" (dangling endpoints); clicking an issue highlights it; resolved issues disappear live.
- **Edge cases & errors:** none.
- **OCCT mapping:** endpoint valence analysis on the curve graph (custom); optionally `ShapeAnalysis_FreeBounds`, `ShapeAnalysis_Wire::CheckConnected`.
- **Priority:** P1

### SKB-18 Sketch visibility
- **Purpose:** Show/hide sketches in the Part Studio.
- **Sources:** Sketch/sketch_basics, Home/keyboard_shortcuts (Shift+h), Home/context_menus (Show/Hide), PartStudio/derived.
- **Selection inputs:** sketch in Feature list.
- **Parameters:** n/a.
- **Behavior / semantics:** Shift+h toggles hide/show of all sketches; per-sketch eye icon / context menu Show/Hide; images in a sketch hide with the sketch; consumed sketches are auto-hidden after use by a feature (inferred, common Onshape behavior; not in local docs); "Hide all construction" hides sketches, planes and origin.
- **Edge cases & errors:** none.
- **OCCT mapping:** none.
- **Priority:** P1

### SKB-19 Copy/paste sketches and sketch entities; derived sketches
- **Purpose:** Reuse sketch content within/between Part Studios.
- **Sources:** Sketch/sketch_basics, Sketch/sketch_tools, Home/context_menus, PartStudio/derived.
- **Selection inputs:** sketch in Feature list (copy), target sketch/plane (paste).
- **Parameters:** n/a.
- **Behavior / semantics:**
  - Feature list context menu "Copy sketch"; "Paste into sketch" on an open sketch, or on a sketch in the Feature list, or on a selected plane/planar face (creates a sketch). After pasting, drag/tap to position the pasted geometry.
  - Images (as sketch entities) can be copied between Part Studios.
  - Derived feature imports a sketch from another Part Studio (associative, placed on its original plane; eye icon toggles visibility).
- **Edge cases & errors:** `PASTE_SKETCH_METRICS_FAILURE` "Highlighted entities have changed due to differing feature libraries", `PASTE_SKETCH_LIBRARY_MISMATCH`, `PASTE_SKETCH_CLIPBOARD_EMPTY`, `COPIED_SKETCH_NOT_FOUND`; copying across Part Studios of differing versions may error.
- **OCCT mapping:** app data model (serialize entities + internal constraints; external references dropped).
- **Priority:** P1

### SKB-20 Sketch feature context menu commands
- **Purpose:** Commands on a sketch in the Feature list.
- **Sources:** Home/context_menus; Sketch/sketch_basics.
- **Selection inputs:** sketch feature.
- **Parameters:** n/a.
- **Behavior / semantics:** Rename; Edit; Copy sketch; Paste into sketch; Show dimensions; Add selection to folder; Show/Hide; Create Drawing of Sketch; Export as DXF/DWG; Suppress; Add comment (also available on a sketch dimension); Zoom to selection; View normal to (camera normal to sketch plane); Clear selection; Roll to here; Delete.
- **Edge cases & errors:** none.
- **OCCT mapping:** DXF export not in open-source OCCT (use `dxf` Rust crate / libdxfrw); view normal from plane normal.
- **Priority:** P1

### SKB-21 Expressions and variables in sketch values
- **Purpose:** Numeric field grammar used by every sketch dimension and tool value.
- **Sources:** Home/numeric_fields, Sketch/dimension, Sketch/sketch_basics, PartStudio/variable.
- **Selection inputs:** n/a.
- **Parameters:** n/a.
- **Behavior / semantics:**
  - Accepts integers, decimals (comma decimal separator if configured), fractions, units (length: mm, millimeter, cm, centimeter, m, meter, in, inch, ft, foot, yd, yard, plurals; angle: deg, degree, rad, radian), math (+ - * / ^ %, ceil, floor, round, exp, sqrt, abs, max, min, log, log10), trig in degrees (cos, sin, tan, acos, asin, atan, atan2, cosh, sinh, tanh, asinh, acosh, atanh), constants pi/PI/Pi, arrays with 0-based lookup `[3,5,6,7][2]`, ternary `?:`, logic `&& ||`, comparisons, variables `#name` (Variable feature/table/Variable Studio, configuration inputs).
  - Unit rules: result must be unitless or a unit to the first power; no mixing incompatible types (3mm+2deg invalid; 3+2in invalid); value without units uses document default units; display converts to default units but re-shows original text when edited.
  - Scroll wheel on a numeric field: default +/-0.1, Ctrl 0.01, Shift 1.0.
  - Expressions evaluated as FeatureScript; dimension shows expression icon (SKB-04).
  - Rho and pattern counts are unitless; variables are not supported for sketch pattern counts (`CANNOT_USE_VARIABLES_IN_SKETCH_PATTERNS`).
- **Edge cases & errors:** invalid expression -> field error; `SKETCH_DIMENSION_DIST_ERROR` "Distance dimension value is not a length".
- **OCCT mapping:** none (expression evaluator, shared with feature parameters).
- **Priority:** P0

### SKB-22 Sketch failure states and messages
- **Purpose:** Catalog of sketch error conditions to reproduce.
- **Sources:** errorstringenum.gen.fs (SKETCH_* entries); Sketch/sketch_fillet, Sketch/sketch_chamfer, Sketch/insert_dwg_or_dxf.
- **Selection inputs:** n/a.
- **Parameters:** n/a.
- **Behavior / semantics:**
  - Feature-level: no plane (`SKETCH_NO_PLANE`), solve failure, unsolvable external constraints, external geometry type changed, beyond system limits (`SKETCH_EXCEEDS_BOUNDS`).
  - Tool-level (transient, shown as message bubble/yellow warning bar, operation not applied): fillet (`SKETCH_FILLET_INVALID_POINT`, `SKETCH_FILLET_PARALLEL`, `SKETCH_FILLET_FAIL`, `SKETCH_FILLET_INVALID_RADIUS`), use/projection (`SKETCH_USE_FAILED`, `SKETCH_USE_PARTIAL`, `SKETCH_PROJ_FAILED`, `SKETCH_PROJ_PARTIAL`, `SILHOUETTE_USE_FAILED`), offset (`SKETCH_OFFSET_FAILED`, `SKETCH_OFFSET_DISTANCE`, `SKETCH_OFFSET_CHAIN_FAIL`, `SKETCH_OFFSET_BASE_CURVE_CONSUMED`, `SKETCH_OFFSET_SPLINE_SPLIT`, `SKETCH_OFFSET_ELLIPSE_SPLIT`), trim/extend/split (`SKETCH_TRIM_FAILED`, `SKETCH_EXTEND_FAILED`, `SKETCH_SPLIT_FAILED`), mirror (`SKETCH_MIRROR_*`), patterns (`SKETCH_PATTERN_*`, `SKETCH_LINEAR_PATTERN_*`, `SKETCH_CIRCULAR_PATTERN_*`), entity creation (`SKETCH_LINE_FAILED`, `SKETCH_ARC_FAILED`, `SKETCH_CIRCLE_FAILED`, `SKETCH_RECTANGLE_FAILED`, `SKETCH_ELLIPSE_FAILED`, `SKETCH_ELLIPSE_FAILED_TOO_SMALL`, `SKETCH_ELLIPSE_ZERO_AXIS`, `SKETCH_CONIC_FAILED`, `SKETCH_SPLINE_*`, `SKETCH_TEXT_*`, `SKETCH_IMAGE_RECTANGLE_FAILED`, `SKETCH_POLYGON_*`, `SKETCH_SLOT_*`, `SKETCH_INSERT_DWG_*`, `SKETCH_TRANSFORM_FAILED`), deletion (`SKETCH_DELETE_PTS_FAILED` "Cannot delete points used by curve", `SKETCH_DELETE_FAILED`), constraint/dimension input errors (see each SKC/SKD item).
  - Visual failure states: red entities/icons (over-constrained), red fillet/chamfer preview when too large, yellow warning bar when chamfer cannot be created.
- **Edge cases & errors:** as listed.
- **OCCT mapping:** none (error enum in app).
- **Priority:** P1

### SKB-23 External references and in-context sketch geometry
- **Purpose:** Behavior of sketch geometry tied to geometry outside the sketch.
- **Sources:** Sketch/use, Sketch/intersection, Sketch/pierce, Sketch/working_with_constraints, Sketch/dimension (dimension to plane).
- **Selection inputs:** external vertices, edges, faces, planes, mate connectors, other sketches' entities.
- **Parameters:** n/a.
- **Behavior / semantics:**
  - External entities can be referenced directly by constraints (coincident, tangent, symmetric about a plane/linear edge, mirror about a plane/face/edge, pierce, dimensions to planes) or converted into sketch geometry via Use/Intersection/silhouette (SKT-09..11).
  - External geometry is treated as fixed by the solver; when it changes upstream, the sketch re-solves; type changes (e.g. circle -> line) are poorly handled (`SKETCH_EXTERNAL_GEOMETRY_MISMATCH`).
  - In-context references (assembly context) appear as "In context" mode in Constraint manager.
  - Dimension between sketch geometry and a plane moves the sketch geometry, the plane stays.
- **Edge cases & errors:** `SKETCH_UNSOLVABLE_CONSTRAINT`, `SKETCH_EXTERNAL_GEOMETRY_MISMATCH`, `SKETCH_CANNOT_PIERCE_WITH_PLANE`.
- **OCCT mapping:** plane traces: `GeomAPI_IntSS` / `IntAna_QuadQuadGeo` plane-plane intersection -> 2D line; edge projection `GeomProjLib::ProjectOnPlane` + `GeomAPI::To2d`.
- **Priority:** P0

### SKB-24 Sketch keyboard shortcuts
- **Purpose:** Default key bindings (all customizable; require a sketch plane to be selected).
- **Sources:** Home/keyboard_shortcuts_and_hotkeys (Sketch section), entity pages.
- **Selection inputs:** n/a.
- **Parameters:**

| Key | Action |
|---|---|
| l | Line |
| shift+a | Switch Line <-> Tangent arc during line creation |
| a | 3 point arc |
| c | Center point circle |
| r | Center point rectangle |
| g | Corner rectangle |
| shift+s | Sketch point (in sketch); Sketch feature (Part Studio) |
| d | Dimension |
| i | Coincident |
| shift+o | Concentric |
| b | Parallel |
| t | Tangent |
| h | Horizontal |
| v | Vertical |
| shift+l | Perpendicular |
| e | Equal |
| shift+m | Midpoint |
| shift+k | Normal |
| shift+g | Pierce |
| shift+q | Symmetric |
| shift+j | Fix |
| shift+u | Curvature |
| m | Trim |
| x | Extend |
| o | Offset |
| u | Use (Project/Convert) |
| shift+f | Sketch fillet (in sketch); Fillet feature (Part Studio) |
| q | Construction toggle |
| shift (hold) | Suppress automatic inferences when moving/placing |
| s | Shortcut toolbar at cursor |
| alt/opt+c | Search tools |
| shift+e / shift+w | Extrude / Revolve from open sketch |
| shift+h | Show/hide all sketches |
| Alt (hold, rectangle tools) | Constrain rectangle sides equal (square) |
| Alt+arrow | Cycle on-the-fly dimension boxes |
| Esc | Exit tool / cancel transform |
| Enter | Accept value / commit transform |

Unassigned by default: Midpoint line, Aligned rectangle, polygons, 3 point circle, Tangent arc, Center point arc, Ellipse, Elliptical arc, Conic, Spline, Bezier, Spline control point, Slot, Text, Image, DXF/DWG, Mirror, patterns, Chamfer, Split, Transform, Intersection, Profile inspector.
- **Behavior / semantics:** pressing a tool key again exits the tool.
- **Edge cases & errors:** none.
- **OCCT mapping:** none.
- **Priority:** P1

---

#### Sketch entities

Conventions for all entity items: every entity accepts the construction flag (SKB-14) unless noted; every creation
click participates in automatic inferencing (SKB-12; Shift suppresses); "click-click" and "press-drag-release" input
are both supported where stated; on-the-fly dimension entry per SKB-13. Created points (endpoints/centers) are
separate solver points shared via implicit coincidence.

### SKE-01 Line (and polyline chaining)
- **Purpose:** Line segment or chain of connected segments.
- **Sources:** Sketch/line; FS: `skLineSegment`, `skPolyline`. Shortcut: l.
- **Selection inputs:** clicks in graphics area (start, end, next...).
- **Parameters:**

| FS id | UI label | Type | Enum values / default / bounds+units | Visible when / notes |
|---|---|---|---|---|
| start | (click 1) | 2D point (length) | - | `startId` returned |
| end | (click 2) | 2D point (length) | - | `endId` returned |
| construction | Construction | boolean | false | |
| (UI) | length box | length expression | suggested from cursor | after each segment |
| skPolyline.points | - | array of 2D points (>1; if closed need >2) | first==last -> closed | FS-only helper |
| skPolyline.constrained | - | boolean | default false | creates COINCIDENT between consecutive segment ends (and closing pair) |

- **Behavior / semantics:**
  - Click to start, click to end each segment; chaining continues from the last endpoint (each new segment start coincident with previous end). Press-drag-release creates a single segment.
  - Horizontal/vertical inference while drawing creates Horizontal/Vertical constraints; endpoint snaps create Coincident.
  - Line -> Tangent arc switching: Shift+a, or hover back near the last endpoint until the cursor icon changes to tangent arc; the next click ends a tangent arc (tangent to the previous segment), then the tool returns to Line automatically.
  - Double-click or Esc ends the chain (inferred standard).
- **Edge cases & errors:** `SKETCH_LINE_FAILED` "The line could not be created between these two points." (zero length).
- **OCCT mapping:** `GCE2d_MakeSegment(p1,p2)` -> `Geom2d_TrimmedCurve` on `Geom2d_Line`.
- **Priority:** P0

### SKE-02 Midpoint line
- **Purpose:** Line drawn symmetrically from its midpoint.
- **Sources:** Sketch/midpoint_line (desktop only; mobile can view/edit).
- **Selection inputs:** click 1 = midpoint, click 2 = one endpoint.
- **Parameters:** UI only (midpoint, endpoint, length box). FS representation: `skLineSegment` + a midpoint point with MIDPOINT constraint (inferred).
- **Behavior / semantics:** Creates a line whose midpoint is at click 1 and one end at click 2 (other end mirrored); the midpoint is a point constrained with Midpoint to the line (inferred) so the line remains symmetric when edited.
- **Edge cases & errors:** zero length -> `SKETCH_LINE_FAILED` (inferred).
- **OCCT mapping:** `GCE2d_MakeSegment(2m - p, p)`.
- **Priority:** P1

### SKE-03 Corner rectangle
- **Purpose:** Axis-aligned rectangle from two diagonal corners.
- **Sources:** Sketch/corner_rectangle; FS: `skRectangle`. Shortcut: g.
- **Selection inputs:** click corner, click diagonal corner (or drag).
- **Parameters:**

| FS id | UI label | Type | Enum values / default / bounds+units | Visible when / notes |
|---|---|---|---|---|
| firstCorner | (click 1) | 2D point | - | |
| secondCorner | (click 2) | 2D point | - | |
| construction | Construction | boolean | false | |
| (UI) | length, width boxes | length | - | Alt+arrow to switch |

- **Behavior / semantics:** Creates 4 line segments with sub-ids `left`, `right`, `top`, `bottom` and constraints: 4 corner COINCIDENTs (`corner0..3`), PARALLEL left/right and top/bottom, VERTICAL on left, HORIZONTAL on top (exactly as `skRectangle`). Remaining DOF = 4 (position x/y, width, height). Holding Alt while sketching adds an EQUAL constraint between two adjacent sides (square).
- **Edge cases & errors:** `SKETCH_RECTANGLE_FAILED` (degenerate: zero width/height).
- **OCCT mapping:** 4x `GCE2d_MakeSegment`.
- **Priority:** P0

### SKE-04 Center point rectangle
- **Purpose:** Axis-aligned rectangle from center and corner.
- **Sources:** Sketch/center_point_rectangle. Shortcut: r.
- **Selection inputs:** click center, click corner (or drag).
- **Parameters:** center (2D point), corner (2D point), construction, length/width boxes; Alt for square.
- **Behavior / semantics:** Same 4-line constraint set as SKE-03 plus construction diagonal(s) with a center point coincident to the diagonal midpoint (inferred: Onshape creates construction diagonal lines and a midpoint constraint so the center stays centered).
- **Edge cases & errors:** `SKETCH_RECTANGLE_FAILED`.
- **OCCT mapping:** segments as SKE-03.
- **Priority:** P0

### SKE-05 Aligned rectangle (3-point rectangle)
- **Purpose:** Rotated rectangle: first edge by two points, then perpendicular width. This is Onshape's "3-point rectangle" (no separate 3-point rectangle tool exists).
- **Sources:** Sketch/aligned_rectangle.
- **Selection inputs:** click 1 = start of base edge, click 2 = end of base edge, click 3 = perpendicular distance.
- **Parameters:** 3 points, construction, length/width boxes; Alt for square.
- **Behavior / semantics:** 4 lines with corner coincidences, two PARALLEL pairs and one PERPENDICULAR (no horizontal/vertical), leaving 5 DOF (position, rotation, width, height) (inferred constraint set).
- **Edge cases & errors:** collinear third point -> `SKETCH_RECTANGLE_FAILED` (inferred).
- **OCCT mapping:** segments.
- **Priority:** P1

### SKE-06 Inscribed polygon
- **Purpose:** Regular polygon around a construction circle ("polygon on the outside of the drawn circle": the circle is inscribed in the polygon, i.e. tangent to the edges).
- **Sources:** Sketch/inscribed_polygon; FS: `skRegularPolygon` (unconstrained helper).
- **Selection inputs:** click center, drag to set circle size, click; then set side count.
- **Parameters:**

| FS id | UI label | Type | Enum values / default / bounds+units | Visible when / notes |
|---|---|---|---|---|
| center | (click 1) | 2D point | - | |
| firstVertex | (drag) | 2D point | != center | FS: distance to center = circumradius |
| sides | number of sides | integer | min 3, max 50 (help + `SKETCH_POLYGON_BAD_SIDE_COUNT` "fifty sides or fewer"); FS requires integer > 2 | typed or dragged (drag toward polygon decreases, away increases) |
| construction | Construction | boolean | false | |
| (UI) | diameter box | length | - | diameter of construction circle |

- **Behavior / semantics:** Creates a construction circle and N line segments; interactive tool constrains vertices/edges so the polygon remains regular (equal sides, tangent edges to circle or vertices on circle - inferred) and the circle diameter is the driving size. Side count can be typed before locking the circle. Note: the help's "inscribed" naming is inverted relative to math convention; follow the help: inscribed = circle inside polygon.
- **Edge cases & errors:** `SKETCH_POLYGON_BAD_SIDE_COUNT`, `SKETCH_POLYGON_ZERO_RADIUS_FAIL`.
- **OCCT mapping:** vertices by rotating (`gp_Trsf2d::SetRotation`), `GCE2d_MakeSegment`, `Geom2d_Circle`.
- **Priority:** P1

### SKE-07 Circumscribed polygon
- **Purpose:** Regular polygon inside a construction circle (vertices on the circle).
- **Sources:** Sketch/circumscribed_polygon; FS `skRegularPolygon`.
- **Selection inputs / Parameters:** as SKE-06 (sides 3..50, diameter box).
- **Behavior / semantics:** As SKE-06 but polygon vertices lie on the construction circle (circle circumscribes the polygon).
- **Edge cases & errors:** as SKE-06.
- **OCCT mapping:** as SKE-06.
- **Priority:** P1

### SKE-08 Center point circle
- **Purpose:** Circle from center and radius point.
- **Sources:** Sketch/center_point_circle; FS: `skCircle`. Shortcut: c.
- **Selection inputs:** click center, click radius point (or drag).
- **Parameters:**

| FS id | UI label | Type | Enum values / default / bounds+units | Visible when / notes |
|---|---|---|---|---|
| center | (click 1) | 2D point | - | `centerId` returned |
| radius | (click 2) / diameter box | length | SKETCH_RADIUS_BOUNDS: 5e-6 m .. 500 m, default 25 mm / 1 in | on-the-fly box asks diameter |
| construction | Construction | boolean | false | |

- **Behavior / semantics:** Full circle; center point is a separate selectable point (exported as point body). Solver DOF 3.
- **Edge cases & errors:** `SKETCH_CIRCLE_FAILED`.
- **OCCT mapping:** `GCE2d_MakeCircle(gp_Ax2d, r)` / `Geom2d_Circle`.
- **Priority:** P0

### SKE-09 3 point circle
- **Purpose:** Circle through three points.
- **Sources:** Sketch/3_point_circle.
- **Selection inputs:** click p1, click p2 (or drag p1->p2), click p3.
- **Parameters:** three 2D points; diameter box; construction. Stored as `skCircle` (center, radius) (inferred) with the picked points only inferring coincidences where snapped.
- **Behavior / semantics:** Circumcircle of three points.
- **Edge cases & errors:** collinear points -> `SKETCH_CIRCLE_FAILED`.
- **OCCT mapping:** `GCE2d_MakeCircle(p1,p2,p3)` / `gce_MakeCirc2d`.
- **Priority:** P0

### SKE-10 3 point arc
- **Purpose:** Arc by two endpoints then a point on the arc.
- **Sources:** Sketch/3_point_arc; FS: `skArc`. Shortcut: a.
- **Selection inputs:** click start, click end (or drag start->end), click third point (sets radius/side).
- **Parameters:**

| FS id | UI label | Type | Enum values / default / bounds+units | Visible when / notes |
|---|---|---|---|---|
| start | (click 1) | 2D point | - | `startId` |
| end | (click 2) | 2D point | - | `endId` |
| mid | (click 3) | 2D point on arc | - | defines radius and bulge side |
| construction | Construction | boolean | false | |
| (UI) | radius box | length | - | |

- **Behavior / semantics:** Arc through start, mid, end; has center point (selectable). Semicircle inference when midpoint aligns (SKB-12). Solver DOF 5.
- **Edge cases & errors:** collinear -> `SKETCH_ARC_FAILED` "The arc could not be created through these three points."
- **OCCT mapping:** `GCE2d_MakeArcOfCircle(p1, pMid, p2)`.
- **Priority:** P0

### SKE-11 Tangent arc
- **Purpose:** Arc starting tangent at the end of an existing curve.
- **Sources:** Sketch/arc_tangent, Sketch/line (Shift+a switching).
- **Selection inputs:** click start (must be an endpoint of a curve), click end (or drag).
- **Parameters:** start point (curve endpoint), end point, radius box, construction.
- **Behavior / semantics:** Arc start coincident with the curve endpoint, TANGENT constraint to that curve; end at click. Direction of departure follows the curve's end tangent; cursor side decides arc orientation (inferred). Within Line tool, Shift+a or hovering back near the endpoint switches to tangent arc for one segment.
- **Edge cases & errors:** `SKETCH_TANGENT_ARC_FAILED`, `SKETCH_TANGENT_ARC_INVALID_START` "Start entity was not at the end of a curve.", `SKETCH_TANGENT_NOT_FOUND`.
- **OCCT mapping:** `GCE2d_MakeArcOfCircle(p1, tangentVec, p2)`.
- **Priority:** P0

### SKE-12 Center point arc
- **Purpose:** Arc from center, start, end.
- **Sources:** Sketch/center_point_arc.
- **Selection inputs:** click center, click start (sets radius), click end (sets sweep).
- **Parameters:** center, start, end points; radius box; construction.
- **Behavior / semantics:** Radius = |start - center|; end point projected onto that circle; sweep direction follows the cursor motion (inferred).
- **Edge cases & errors:** `SKETCH_ARC_FAILED`.
- **OCCT mapping:** `GCE2d_MakeArcOfCircle(gp_Circ2d, p1, p2, sense)`.
- **Priority:** P0

### SKE-13 Ellipse
- **Purpose:** Full ellipse from center and two axis radii.
- **Sources:** Sketch/ellipse; FS: `skEllipse`.
- **Selection inputs:** click center, click primary axis radius point (sets direction + radius), click secondary radius (perpendicular).
- **Parameters:**

| FS id | UI label | Type | Enum values / default / bounds+units | Visible when / notes |
|---|---|---|---|---|
| center | (click 1) | 2D point | - | `centerId` |
| majorRadius | primary axis (diameter box) | length | SKETCH_RADIUS_BOUNDS 5e-6..500 m | either axis may be major |
| minorRadius | secondary axis (diameter box) | length | SKETCH_RADIUS_BOUNDS | |
| majorAxis | (direction of click 2) | unitless 2D direction | optional | |
| construction | Construction | boolean | false | |

- **Behavior / semantics:** Axis that is drawn larger is the major. Ellipse exposes center and quadrant (axis end) points for snapping (Quadrant constraint). Horizontal/Vertical can be applied to an ellipse (orients its major axis). Dimensions: MAJOR_DIAMETER / MINOR_DIAMETER (SKD-11). Solver DOF 5.
- **Edge cases & errors:** `SKETCH_ELLIPSE_FAILED`, `SKETCH_ELLIPSE_FAILED_TOO_SMALL`, `SKETCH_ELLIPSE_ZERO_AXIS`.
- **OCCT mapping:** `GCE2d_MakeEllipse(gp_Ax22d, Rmaj, Rmin)` (OCCT requires major >= minor: swap axes if the primary is shorter).
- **Priority:** P1

### SKE-14 Elliptical arc
- **Purpose:** Portion of an ellipse.
- **Sources:** Sketch/elliptical_arc; FS: `skEllipticalArc`.
- **Selection inputs:** click center, click primary radius, move to set secondary radius and click arc start on the temporary ellipse, move along ellipse and click end.
- **Parameters:**

| FS id | UI label | Type | Enum values / default / bounds+units | Visible when / notes |
|---|---|---|---|---|
| center | (click 1) | 2D point | - | |
| majorAxis | (click 2 direction) | 2D direction | - | |
| majorRadius | primary diameter box | length, non-negative | SKETCH_RADIUS_BOUNDS | need not exceed minor |
| minorRadius | secondary diameter box | length, non-negative | SKETCH_RADIUS_BOUNDS | |
| startParameter | (click 3) | number | period 1: 0 at major axis, 0.25 at minor axis | |
| endParameter | (click 4) | number | same convention | arc runs counterclockwise start->end |
| construction | Construction | boolean | false | |

- **Behavior / semantics:** Temporary construction ellipse is deleted after creation; start/end can snap to axis/quadrant points (yellow inference line). `startId`/`endId` returned.
- **Edge cases & errors:** as SKE-13.
- **OCCT mapping:** `GCE2d_MakeArcOfEllipse(gp_Elips2d, u1, u2, Standard_True)` with u = 2*pi*parameter.
- **Priority:** P1

### SKE-15 Conic
- **Purpose:** Conic section (elliptical, parabolic, hyperbolic) by start, end, control point and rho.
- **Sources:** Sketch/conic; FS: `skConicSegment` (@internal).
- **Selection inputs:** click start, click end, (optionally type rho), click control point.
- **Parameters:**

| FS id | UI label | Type | Enum values / default / bounds+units | Visible when / notes |
|---|---|---|---|---|
| start | (click 1) | 2D point | - | |
| end | (click 2) | 2D point | - | |
| controlPoint | (click 3) | 2D point | - | intersection of end tangents (shoulder control) |
| rho | rho box | unitless number | 0 < rho < 1; <0.5 elliptical, =0.5 parabolic, >0.5 hyperbolic | box opens automatically if not entered before click 3 |
| lowParameter / highParameter | - | number | optional | segment trimming |
| fixedRho | - | boolean | optional | fix rho during solve |
| construction | Construction | boolean | false | |

- **Behavior / semantics:** rho = x/y where x = distance from controlPoint to chord start-end and y = max distance from conic to chord (FS doc; i.e. shoulder ratio). Tangent at start points to control point, at end from control point. Rho is held by a RHO constraint/dimension that cannot be deleted (`CANNOT_DELETE_RHO_DIMENSION`). Curvature constraint applies (SKC-14).
- **Edge cases & errors:** `SKETCH_CONIC_FAILED`, `SKETCH_RHO_DIM_NOT_FOUND`, `CANNOT_EDIT_FIXED_CONIC`.
- **OCCT mapping:** rational quadratic `Geom2d_BezierCurve` with poles (start, control, end) and weights (1, w, 1), w = rho/(1-rho); or `Geom2d_BSplineCurve` equivalent. Optionally convert to exact `Geom2d_Parabola/Hyperbola/Ellipse` trimmed.
- **Priority:** P2

### SKE-16 Spline (interpolated / fit spline)
- **Purpose:** Smooth curve through clicked points with end tangent handles.
- **Sources:** Sketch/spline; FS: `skFitSpline` (public), `skInterpolatedSpline`, `skInterpolatedSplineSegment`, `skSpline`, `skSplineSegment` (@internal).
- **Selection inputs:** click start, click each through-point, double-click to end; closing by clicking the first point.
- **Parameters:**

| FS id | UI label | Type | Enum values / default / bounds+units | Visible when / notes |
|---|---|---|---|---|
| points | (clicks) | array of 2D points, size > 1 | first == last -> closed (periodic) | |
| parameters | - | array of numbers | optional | parameter per point |
| startDerivative | start tangent handle | 2D vector (length units) | optional | ignored if closed |
| endDerivative | end tangent handle | 2D vector (length units) | optional | ignored if closed |
| splinePointCount | - | number | internal | skInterpolatedSpline |
| construction | Construction | boolean | false | |

- **Behavior / semantics:**
  - Interpolating cubic spline through points; each spline point is a sketch point that can be dragged, dimensioned and constrained.
  - Tangent handles (white points) at ends: move along/away from spline to change direction and magnitude; handles cannot be deleted; handle endpoints can be dimensioned/constrained (e.g. Horizontal, distance).
  - Closed only by connecting the last point to the first.
  - Add more points with Spline control point tool (SKE-17). Interior (non-end) points can be deleted; endpoints cannot.
  - Right-click "Confirm spline" ends creation at the previous point (drops the point under the cursor).
  - Curvature color map context command visualizes curvature.
  - Tangent/curvature constraints at ends to adjacent curves (SKC-04, SKC-14).
- **Edge cases & errors:** `SKETCH_SPLINE_FAILED`, `SKETCH_BAD_SPLINE` "likely self-intersecting", `SKETCH_SPLINE_NEW_POINTS_TOO_CLOSE`, `SKETCH_SPLINE_CANNOT_DELETE_ENDPOINTS`, `SKETCH_SPLINE_POINT_TO_DELETE_NOT_FOUND` "Cannot delete spline tangent handles", `SKETCH_SPLINE_TOO_FEW_POINTS` "Closed splines must have at least three spline points", `SKETCH_SPLINE_NOT_INTERPOLATED_SPLINE`, `SKETCH_SPLINE_POINTS_NOT_DELETED`.
- **OCCT mapping:** `Geom2dAPI_Interpolate(points, periodic, tol)` + `Load(startTangent, endTangent, scale=false)` -> `Geom2d_BSplineCurve`; chord-length parameterization default (Onshape parameterization undocumented).
- **Priority:** P1

### SKE-17 Spline control point tool
- **Purpose:** Add points to an existing spline or bezier control polygon.
- **Sources:** Sketch/spline_control_point.
- **Selection inputs:** click on a spline (adds a through-point) or on a bezier control polygon (adds a control point).
- **Parameters:** none.
- **Behavior / semantics:** Added point becomes part of the curve definition (moving it reshapes the curve), unlike a sketch Point merely coincident to the spline (which only constrains the curve to pass through it). Adding to a spline should preserve shape at insertion as far as possible (inferred); adding to a bezier increases control point count (degree elevation; inferred).
- **Edge cases & errors:** `SKETCH_SPLINE_NOT_INTERPOLATED_SPLINE` "Cannot add/delete points on curve."
- **OCCT mapping:** spline: re-interpolate with inserted point; bezier: `Geom2d_BezierCurve::InsertPoleAfter` or `Increase` (degree elevation preserves shape).
- **Priority:** P2

### SKE-18 Bezier curve
- **Purpose:** Single-segment Bezier from clicked control points.
- **Sources:** Sketch/bezier; FS: `skBezier`.
- **Selection inputs:** click start, click control points, double-click to end.
- **Parameters:**

| FS id | UI label | Type | Enum values / default / bounds+units | Visible when / notes |
|---|---|---|---|---|
| points | (clicks) | array of 2D points, size > 1 | degree = n-1 | control polygon drawn black, control points white |
| construction | Construction | boolean | false | |
| (UI) | degree field | integer | max 15 | double-click degree label to edit (BEZIER_DEGREE constraint) |

- **Behavior / semantics:** Curve follows its control polygon; control points dimensionable/constrainable (e.g. vertical between two control points). Cannot be closed (first/last control point cannot coincide); use Spline for closed. Right-click "Escape bezier" aborts creation (removes current bezier). Trim keeps original degree and control-point count on the remaining piece; Split yields two curves of the same degree and control-point count, coincident at the split point, shape unchanged, control points independent.
- **Edge cases & errors:** closed loop not allowed.
- **OCCT mapping:** `Geom2d_BezierCurve(poles)`; trim/split via `Geom2d_BezierCurve::Segment(u1,u2)` (keeps degree); degree change via `Increase` (elevation) or least-squares reduction (custom).
- **Priority:** P2

### SKE-19 Point
- **Purpose:** Standalone sketch point.
- **Sources:** Sketch/point; FS: `skPoint`. Shortcut: shift+s (in sketch).
- **Selection inputs:** click.
- **Parameters:** `position` (2D point, optional). Returns `pointId`. No construction flag.
- **Behavior / semantics:** Exported as POINT body (used e.g. by Hole locations). Most constraints apply (coincident, midpoint, horizontal/vertical sets, symmetric, pierce, fix, dimensions). A point on a spline does not modify the spline definition (contrast SKE-17).
- **Edge cases & errors:** standalone points can be deleted by Trim.
- **OCCT mapping:** `gp_Pnt2d`; 3D `BRepBuilderAPI_MakeVertex`.
- **Priority:** P0

### SKE-20 Slot
- **Purpose:** Slot outline around selected centerline curves.
- **Sources:** Sketch/slot.
- **Selection inputs:** pre- or post-select sketch curves: lines, arcs, splines, chains, closed profiles (individually or box-select).
- **Parameters:** single width value (displayed as a diameter dimension); editable before or after placement.
- **Behavior / semantics:**
  - Creates offset curves on both sides of each centerline at width/2 plus round end caps (arcs) for open chains; closed profiles produce inner and outer offset loops.
  - Multi-chained selection (contiguous curves selected together) -> one slot following the chain shape; selecting individual curves of a shape (with the tool active, clicking empty space between picks) -> separate slots per curve.
  - All slots created in one tool session share one linked width dimension (editing it changes all).
  - Centerlines are converted to construction (inferred, typical). Implementation constraints: OFFSET constraints between centerline and each side ("circle and point" offset pairs for end caps per `SKETCH_OFFSET_BAD_PAIR` wording) (inferred).
- **Edge cases & errors:** `SKETCH_SLOT_FAILURE` "A slot could not be created for the selected curve.", `SKETCH_SLOT_PARTIAL_FAILURE`.
- **OCCT mapping:** `BRepOffsetAPI_MakeOffset(wire, GeomAbs_Arc, isOpenResult=Standard_True).Perform(width/2)` gives the rounded slot loop around an open wire directly; closed wires: offset +/- width/2; or analytic per-curve `Geom2d_OffsetCurve` + end arcs.
- **Priority:** P1

### SKE-21 Text
- **Purpose:** Sketch text whose glyph outlines become sketch curves/regions.
- **Sources:** Sketch/text; FS: `skText`.
- **Selection inputs:** click-drag to define text box (lower-left corner and height define first-line position and size).
- **Parameters:**

| FS id | UI label | Type | Enum values / default / bounds+units | Visible when / notes |
|---|---|---|---|---|
| text | Text | string (or expression) | max 250 characters per box; newlines allowed (`\n`); empty not allowed | can be converted to an expression (right-click "Convert to expression"; back with "Convert to text") |
| fontName | Font | string | `OpenSans-Regular.ttf` (default if no match), `AllertaStencil-Regular.ttf` (no bold/italic), `Arimo-Regular.ttf`, `DroidSansMono.ttf` (no bold/italic), `NotoSans-Regular.ttf`, `NotoSansCJKjp-Regular.otf` (no italic), `NotoSansCJKkr-Regular.otf` (no italic), `NotoSansCJKsc-Regular.otf` (no italic), `NotoSansCJKtc-Regular.otf` (no italic), `NotoSerif-Regular.ttf`, `RobotoSlab-Regular.ttf` (no italic), `Tinos-Regular.ttf` | Bold/Italic encoded by replacing `-Regular` with `-Bold`, `-Italic`, `-BoldItalic` |
| (UI) | Bold / Italic | toggles | off | map to fontName suffix |
| mirrorHorizontal | Flip about horizontal center | boolean | false | |
| mirrorVertical | Flip about vertical center | boolean | false | |
| firstCorner / secondCorner | (drag) | 2D points | optional | text starts at left, overflows right; first line fills the box height; subsequent lines below the box (above if mirrored vertically) |
| construction | - | boolean | optional in FS | UI disallows (`SKETCH_TEXT_CANNOT_BE_CONSTRUCTION`) |

- **Behavior / semantics:**
  - Box height = baseline to ascender of the first line. Horizontal constraint applied to the box's lower edge by default.
  - Text expressions: string, Length, Angle, Number variables, `~` concatenation, e.g. `roundToPrecision(#Length/in, 3) ~ "in"`, `#Angle/deg ~ " degree"`, `#Number ~ " items"`; text is configurable via configurations.
  - Box can be dimensioned; dragging moves an unconstrained box; resize by corner drag with another corner fixed; rotate by deleting the horizontal constraint, fixing one corner and dragging another. Context menu "Edit text".
  - Glyph outlines form regions (extrudable); text can be dimensioned/constrained via its box.
  - Text cannot be patterned or mirrored inside a sketch (use feature patterns/mirror instead). Transform tool supports text.
- **Edge cases & errors:** `SKETCH_TEXT_RECTANGLE_FAILED`, `SKETCH_TEXT_IS_EMPTY`, `SKETCH_TEXT_CANNOT_BE_CONSTRUCTION`.
- **OCCT mapping:** `Font_FontMgr` (register bundled fonts), `Font_BRepFont` + `Font_BRepTextBuilder::Perform(font, NCollection_String, gp_Ax3, halign, valign)` -> planar faces/wires; scale to box height via ascender metric; mirror via `gp_Trsf`; extract 2D outline curves (B-spline/line) for display and regions.
- **Priority:** P1

### SKE-22 Insert image
- **Purpose:** Place a raster image in a sketch as a tracing reference.
- **Sources:** Sketch/insert_image; FS: `skImage`.
- **Selection inputs:** image chosen from current document, other documents, or an image library (or Import); then click-drag to place.
- **Parameters:**

| FS id | UI label | Type | Enum values / default / bounds+units | Visible when / notes |
|---|---|---|---|---|
| blobInfo | (image picker) | map (blob reference) | PNG, JPEG, GIF, BMP | |
| firstCorner / secondCorner | (drag) | 2D points | aspect ratio locked (dashed preview) | returns `imageId` |

- **Behavior / semantics:** Default horizontal constraint on lower edge; semi-transparent while moving; rotate = delete horizontal, fix a corner, drag another; move = remove constraints and drag. The first dimension applied to sketch geometry also scales the image (only the first). Alpha channel respected. Hidden/shown with the sketch. Context menu "Edit image" to swap image; image tab "Update" propagates source changes. Max size limited by GPU max texture size. Copy/paste supported; Transform supported. Image libraries: one per company (admin) and one per user, set via folder context menu.
- **Edge cases & errors:** `SKETCH_IMAGE_RECTANGLE_FAILED`.
- **OCCT mapping:** no geometry; textured quad in renderer (wgpu/GPUI); 4 corner points + rectangle constraints in solver; image decoding via `image` crate.
- **Priority:** P2

### SKE-23 Insert DXF or DWG
- **Purpose:** Convert DXF/DWG file contents to sketch entities.
- **Sources:** Sketch/insert_dwg_or_dxf; File/supported_file_formats.
- **Selection inputs:** a DXF/DWG previously imported into the document (or in another accessible document), or Import from the dialog.
- **Parameters:**

| FS id | UI label | Type | Enum values / default / bounds+units | Visible when / notes |
|---|---|---|---|---|
| (UI) | Units | enum | document length units list | select before choosing file (choosing a file closes dialog) |
| (UI) | Use file origin position | checkbox | default off | on: keep file origin at sketch origin; off: center of 2D bounding box of all entities placed at sketch origin |

- **Behavior / semantics:** Entities (lines, arcs, circles, ellipses, splines, polylines, text, inferred) become regular sketch entities; recommended into an empty sketch; recorded in undo stack; the first dimension applied scales the entire inserted geometry; Transform supported.
- **Edge cases & errors:** alert on geometry/format/integrity error; `SKETCH_INSERT_DWG_CONVERSION_FAILURE` "Some entities could not be converted properly.", `SKETCH_INSERT_DWG_MAX_ENTITIES_EXCEEDED`.
- **OCCT mapping:** gap: open-source OCCT has no DXF/DWG reader. Use Rust `dxf` crate or libdxfrw (DXF), LibreDWG (GPL) / ODA Teigha (commercial) for DWG; map entities to `Geom2d_*` (LWPOLYLINE bulge -> arcs, SPLINE -> `Geom2d_BSplineCurve`).
- **Priority:** P1

---

#### Sketch tools

### SKT-01 Trim
- **Purpose:** Remove the portion of a curve between nearest intersections/bounds.
- **Sources:** Sketch/trim; `SketchToolType.TRIM`. Shortcut: m.
- **Selection inputs:** click curve segments to trim, or press-drag a path through multiple entities; standalone points can also be deleted.
- **Parameters:** none.
- **Behavior / semantics:**
  - Find the picked parameter on the curve; compute all intersections of the curve with other sketch curves (including construction curves and bounding geometry); remove the sub-segment containing the pick between the nearest intersection on each side (or curve end). If no intersection/bounding geometry is found, the entire curve is deleted.
  - Closed curves (circle/ellipse/closed spline) need two intersections to trim, becoming arcs/open curves.
  - The remaining pieces keep constraints on retained ends; new endpoints get Coincident (point-on-curve) constraints to the bounding curve (inferred, standard).
  - Bezier trim keeps degree and control-point count on the remainder.
  - Drag-trim trims every segment crossed by the drag path.
- **Edge cases & errors:** `SKETCH_TRIM_FAILED` "Could not trim this selected entity."
- **OCCT mapping:** `Geom2dAPI_InterCurveCurve` (all pairs) -> sorted parameters; `Geom2d_TrimmedCurve` / `Geom2d_BSplineCurve::Segment`; `Geom2dAPI_ProjectPointOnCurve` for the pick parameter.
- **Priority:** P0

### SKT-02 Extend
- **Purpose:** Extend a line or arc to the next intersecting geometry.
- **Sources:** Sketch/extend; `SketchToolType.EXTEND`. Shortcut: x.
- **Selection inputs:** click the line/arc near the end to extend; then click a new location or bounding geometry; or double-click / drag the curve to auto-complete.
- **Parameters:** none.
- **Behavior / semantics:**
  - Extends the end nearest the pick along the curve's natural continuation (line along its direction; arc along its circle) to the first intersecting sketch curve/point/edge; if none found, the end stops at the release/click point (a point is created there).
  - Double-click or drag an arc edge to auto-complete to bounding geometry (e.g. elliptical arc completes to a full ellipse when no bound).
- **Edge cases & errors:** `SKETCH_EXTEND_FAILED` "Unable to extend the selected geometry."
- **OCCT mapping:** underlying `Geom2d_Line`/`Geom2d_Circle` (untrimmed basis) intersected with `Geom2dAPI_InterCurveCurve`; splines: `GeomLib::ExtendCurveToPoint` (3D) or custom 2D extrapolation.
- **Priority:** P0

### SKT-03 Offset
- **Purpose:** Create curves offset from a curve or chain at a distance, kept associative.
- **Sources:** Sketch/offset; `SketchToolType.OFFSET`; `ConstraintType.OFFSET`. Shortcut: o.
- **Selection inputs:** one or more curves (single entity or chain-selected loop); additional curves can be clicked after to get offsets at the same distance.
- **Parameters:** distance (length; negative flips side), direction arrow (click to flip; drag arrow to set distance approximately, then type).
- **Behavior / semantics:**
  - Offset of a connected chain is computed as a whole (joined offset with trimmed/extended corners; arcs inserted at convex corners, inferred); each offset curve is linked to its base by an OFFSET constraint driven by one distance dimension; further curves clicked in the same session share the distance.
  - Offset of a circle is a concentric circle; of a line a parallel line; of an arc a concentric arc; splines/ellipses produce offset curves (approximations).
- **Edge cases & errors:** `SKETCH_OFFSET_FAILED`, `SKETCH_OFFSET_DISTANCE` "Offset could not be created at this distance.", `SKETCH_OFFSET_CHAIN_FAIL` "Can only offset a single chain at once.", `SKETCH_OFFSET_BASE_CURVE_CONSUMED` "No corresponding offset for reference curve", `SKETCH_OFFSET_SPLINE_SPLIT` / `SKETCH_OFFSET_ELLIPSE_SPLIT` (one curve generated multiple offset curves), `SKETCH_OFFSET_BAD_PAIR`, `SKETCH_OFFSET_INPUT_ERROR`, `SKETCH_OFFSET_CONSTRAINT_FAILED`.
- **OCCT mapping:** `BRepOffsetAPI_MakeOffset` on the planar wire (`GeomAbs_Arc` or `GeomAbs_Intersection` join) for chains; `Geom2d_OffsetCurve` for single analytic curves (exact for lines/circles); approximate splines/ellipses with `Geom2dConvert::ApproxCurve` / `Geom2dConvert_ApproxCurve` to keep B-spline representation; track base->offset correspondence via `BRepOffsetAPI_MakeOffset::Generated`.
- **Priority:** P0

### SKT-04 Sketch mirror
- **Purpose:** Reflect sketch entities about a line, plane, part face or edge.
- **Sources:** Sketch/sketch_mirror; `SketchToolType.MIRROR`; `ConstraintType.MIRROR`.
- **Selection inputs:** pre-selected entities then tool then mirror line; or tool, mirror line, then entities one by one (mirrored as picked; Esc to exit). Mirror line: sketch line, plane, part face, or edge.
- **Parameters:** none.
- **Behavior / semantics:** Creates mirrored copies and SYMMETRIC (MIRROR) constraints between each original and copy about the mirror line, so copies follow edits. Internal constraints of the originals are mirrored (inferred).
- **Edge cases & errors:** `SKETCH_MIRROR_NEED_VALID_MIRROR_LINE`, `SKETCH_MIRROR_NEED_ENTITIES_TO_MIRROR`, `SKETCH_MIRROR_CONSTRAINT_FAILED`, `SKETCH_MIRROR_FAILED`, `SKETCH_MIRROR_OFFSET_SPLINE` "Cannot mirror offset splines.", `SKETCH_MIRROR_OFFSET_ELLIPSE`, `SKETCH_MIRROR_CURVE_POINT` "Cannot mirror curve points."; text cannot be mirrored in sketch.
- **OCCT mapping:** `gp_Trsf2d::SetMirror(gp_Ax2d)`, `Geom2d_Geometry::Mirrored`; plane/face mirror line = trace of plane on sketch plane (`IntAna_QuadQuadGeo`).
- **Priority:** P0

### SKT-05 Linear sketch pattern
- **Purpose:** Repeat sketch entities in one or two directions with associativity.
- **Sources:** Sketch/sketch_linear_pattern; `SketchToolType.PATTERN`; `ConstraintType.LINEAR_PATTERN`.
- **Selection inputs:** pre-selected sketch entities, then tool.
- **Parameters (on-canvas manipulators):** instance count per direction (double-click to enter; integer; no variables), spacing distance (double-click), direction angle (drag arrow base / double-click angle value), optional second direction.
- **Behavior / semantics:** Initial pattern previewed; click in empty space to accept. Instances tied by LINEAR_PATTERN constraint. Deleting an instance leaves a gap (count change does not reinstate). Preview shows at most first 10 instances for performance. Edit later: hover an instance, right-click the pattern icon, "Edit pattern".
- **Edge cases & errors:** `SKETCH_PATTERN_UNKNOWN_FAILURE`, `SKETCH_PATTERN_TOO_LARGE`, `SKETCH_LINEAR_PATTERN_ZERO_LENGTH`, `SKETCH_LINEAR_PATTERN_PARALLEL_DIRECTIONS`, `CANNOT_USE_VARIABLES_IN_SKETCH_PATTERNS`; text cannot be patterned.
- **OCCT mapping:** `gp_Trsf2d::SetTranslation`; copies of `Geom2d` curves.
- **Priority:** P1

### SKT-06 Circular sketch pattern
- **Purpose:** Repeat sketch entities about a center.
- **Sources:** Sketch/sketch_circular_pattern; `ConstraintType.CIRCULAR_PATTERN`.
- **Selection inputs:** pre-selected entities; center manipulator (initially at sketch origin, not constrained to it; drag to reposition/resize); axis or mate connector reference per description.
- **Parameters:** instance count (double-click), total angle (default 360 closed; drag arrow head to open pattern; open stays open when count changes), angle value field, spacing.
- **Behavior / semantics:** As SKT-05 with rotation; closed pattern divides 360 deg by count; open pattern uses given angle between first/last (inferred: angle field is total span).
- **Edge cases & errors:** `SKETCH_CIRCULAR_PATTERN_FAILED`, `SKETCH_CIRCULAR_PATTERN_ZERO_ANGLE`, `SKETCH_PATTERN_TOO_LARGE`.
- **OCCT mapping:** `gp_Trsf2d::SetRotation(center, angle)`.
- **Priority:** P1

### SKT-07 Sketch fillet
- **Purpose:** Round a corner between two curves with a tangent arc.
- **Sources:** Sketch/sketch_fillet; `SketchToolType.FILLET`. Shortcut: shift+f.
- **Selection inputs:** a shared vertex (point) or two curves (lines, arcs, splines); click second curve and drag to estimate size.
- **Parameters:** radius (length; first value becomes default for subsequent picks in the same session; editing the first value changes all of them).
- **Behavior / semantics:**
  - Trims both curves back and inserts an arc tangent to both, with TANGENT and COINCIDENT constraints and a radius dimension; fillets made in one session share an EQUAL/linked radius.
  - A "virtual sharp" construction point is left at the original intersection, retaining the coincident constraints of the two curves (usable as a dimension reference).
  - If the radius is too large for a corner the preview is red; manipulator drag resizes.
- **Edge cases & errors:** `SKETCH_FILLET_INVALID_POINT`, `SKETCH_FILLET_PARALLEL` "Cannot fillet parallel edges.", `SKETCH_FILLET_FAIL`, `SKETCH_FILLET_INVALID_RADIUS`.
- **OCCT mapping:** `ChFi2d_FilletAPI` (auto-selects `ChFi2d_AnaFilletAlgo` for line/arc or `ChFi2d_FilletAlgo` for general curves) on 3D edges in the plane; or `Geom2dGcc_Circ2d2TanRad` with `Geom2dGcc_QualifiedCurve` to pick the correct solution nearest the corner.
- **Priority:** P0

### SKT-08 Sketch chamfer
- **Purpose:** Bevel a corner between two curves.
- **Sources:** Sketch/sketch_chamfer; `SketchToolType.CHAMFER`.
- **Selection inputs:** a vertex or two edges (lines highlighted yellow); multiple selections in a session.
- **Parameters:** distance 1, distance 2 (lengths; angle defaults to 45 deg i.e. equal distances until second distance edited), applied via value fields; editing the first selection's values updates all selections in the session.
- **Behavior / semantics:** Trims the two curves and inserts a line between points at the given distances from the corner; leaves a virtual sharp (inferred by analogy with fillet); manipulator drag resizes.
- **Edge cases & errors:** too large -> red preview; cannot create -> yellow warning bar.
- **OCCT mapping:** `ChFi2d_ChamferAPI` (line-line) or custom intersection of circles of radius d about the corner with each curve (`Geom2dAPI_InterCurveCurve`).
- **Priority:** P1

### SKT-09 Use (project / convert)
- **Purpose:** Project external edges, sketch entities, faces' edges into the active sketch as associative sketch geometry.
- **Sources:** Sketch/use; `SketchToolType.USE`; `ConstraintType.PROJECTED`; `SketchProjectionType {USE, USE_END, ...}`. Shortcut: u.
- **Selection inputs:** edges, faces (all edges of face), vertices, sketch entities from other sketches or features; pre-select supported for edges (not for silhouettes); hover shows projection preview.
- **Parameters:** none.
- **Behavior / semantics:** Each selected entity is orthogonally projected onto the sketch plane (e.g. a circular edge viewed edge-on projects to a line); the resulting sketch curve is bound by a Use (PROJECTED) constraint, shown with blue background; it updates when upstream geometry changes. Used geometry is drawn in the "used edge" style and can be toggled construction. Partial results when some edges cannot be used.
- **Edge cases & errors:** `SKETCH_USE_FAILED` "The face could not be used in the sketch.", `SKETCH_USE_PARTIAL`, `SKETCH_PROJ_FAILED`, `SKETCH_PROJ_PARTIAL`, `SKETCH_PROJECTION_UNKNOWN`, `SKETCH_PROJECTION_FAILED`, `SKETCH_EXTERNAL_GEOMETRY_MISMATCH` (type change, e.g. circle -> line, not handled well).
- **OCCT mapping:** `GeomProjLib::ProjectOnPlane(curve3d, plane, dir=normal, keepParametrization)` then `GeomAPI::To2d`; analytic recognition of degenerate projections (circle edge-on -> segment: detect via `GeomLib_IsPlanarSurface`/normal dot) to emit `Geom2d_Line`; ellipse from tilted circle. `BRepProj_Projection` (cylindrical) for edge sets.
- **Priority:** P0

### SKT-10 Use silhouette edges
- **Purpose:** Project the silhouette (outline as seen normal to the sketch plane) of curved faces.
- **Sources:** Sketch/use (Using silhouette edges); `ConstraintType.SILHOUETTED`; `SketchProjectionType.SILHOUETTE_START/END`; `SketchSilhouetteDisambiguation`.
- **Selection inputs:** with Use active (no pre-selection), hover a face: highlighted silhouette candidates appear (if the real silhouette is out of plane both the 3D silhouette and its projection are shown and selectable); click a face to take all its silhouettes or click an individual highlight.
- **Parameters:** none (disambiguation stored internally: ALIGNED/ANTI_ALIGNED, OUTER/INNER, TORUS_TOP/BOTTOM/LEFT/RIGHT).
- **Behavior / semantics:** Supported faces: cylinders, cones, tori, spheres, extruded surfaces, and any surface with one silhouette edge; only "trackable" silhouettes; ends of silhouette lines are not constrained (user constrains them); no automatic sewing of face outlines; silhouettes self-intersecting after projection are rejected.
- **Edge cases & errors:** `SILHOUETTE_USE_FAILED`, `SKETCH_SIL_PROJECTION_INPUT_ERROR` "A silhouette projection requires a point and a segment.", `SKETCH_SIL_PROJECTION_MISSING_POINT`.
- **OCCT mapping:** analytic silhouettes: cylinder/cone -> two rulings tangent to view direction (compute from axis and view normal), sphere -> great circle, torus -> outer/inner circles or offset curves; general: `HLRBRep_Algo` / `HLRBRep_PolyAlgo` with `HLRAlgo_Projector` along plane normal and extract `OutLineVCompound`; `Contap_Contour` for contour lines on a surface for a direction.
- **Priority:** P1

### SKT-11 Intersection
- **Purpose:** Project the intersection curve of faces/surfaces with the sketch plane.
- **Sources:** Sketch/intersection (in Use dropdown); `ConstraintType.INTERSECTED`, `PIERCE`.
- **Selection inputs:** surfaces or faces intersecting the sketch plane (multiple).
- **Parameters:** none.
- **Behavior / semantics:** Creates sketch curves along face-plane intersection; open curve end points get Pierce constraints onto the intersected face's edges; closed curves get Intersection (INTERSECTED) constraints; updates when the model changes.
- **Edge cases & errors:** `SKETCH_INTERSECTION_FAILED`, `SKETCH_INTERSECTION_MULTIPLE_FAILED`, `SKETCH_INTERSECTION_PARTIAL_FAILED`.
- **OCCT mapping:** `BRepAlgoAPI_Section(face, gp_Pln)` -> edges -> `BRep_Tool::CurveOnPlane` / `GeomAPI::To2d`.
- **Priority:** P1

### SKT-12 Sketch split
- **Purpose:** Split a curve into segments at picked points.
- **Sources:** Sketch/sketch_split; `SketchToolType.SPLIT`.
- **Selection inputs:** click a curve, then click one or more locations along it.
- **Parameters:** none.
- **Behavior / semantics:** Open curves need >= 1 split point; closed curves need >= 2. Pieces are coincident at split points (endpoints joined by Coincident) so dragging one moves the other; shapes unchanged. Bezier: both pieces keep degree and control-point count; control points independent.
- **Edge cases & errors:** `SKETCH_SPLIT_FAILED`.
- **OCCT mapping:** `Geom2d_TrimmedCurve` / `Geom2d_BSplineCurve::Segment` / `Geom2d_BezierCurve::Segment`; split parameters by `Geom2dAPI_ProjectPointOnCurve`.
- **Priority:** P1

### SKT-13 Transform sketch entities (translate / rotate / scale)
- **Purpose:** Rigidly move, rotate, or scale selected entities.
- **Sources:** Sketch/sketch_transform, Sketch/sketch_basics (Transforming sketches); context menu "Transform sketch entities".
- **Selection inputs:** at least one sketch entity (pre/post selection; add/remove during operation); works for images, text, DWG/DXF content.
- **Parameters:** manipulator: drag translation (arrows/plane), rotation (angle indicator; angle field), scale (triangle handle; scale factor field), ball point (center free drag repositions the triad without transforming; free drag snaps to inferences).
- **Behavior / semantics:**
  - Commit by clicking off the sketch or Enter; Esc cancels.
  - Constraint maintenance: 0 or 180 deg rotation -> internal constraints unchanged; 90/270 deg -> Horizontal and Vertical constraints swap; other angles -> constraints that become invalid are handled: construction geometry may be added to maintain DOF; directed (horizontal/vertical) dimensions are deleted and may be replaced by construction geometry plus minimum (aligned) dimensions.
  - No "copy" option is documented for sketch transform (copy is via copy/paste or pattern) (gap noted).
- **Edge cases & errors:** `SKETCH_TRANSFORM_FAILED`.
- **OCCT mapping:** `gp_Trsf2d` (SetTranslation / SetRotation / SetScale), `Geom2d_Geometry::Transform`.
- **Priority:** P1

### SKT-14 Wrap (sketch-region wrap onto cylinder/cone)
- **Purpose:** Listed in the Sketch help area but is a Part Studio feature: wrap planar faces/sketch regions onto a cylinder or cone.
- **Sources:** Sketch/wrap; FS: `wrap` (wrap.fs). Full spec belongs with Part Studio features; summary here.
- **Selection inputs:** Tools: `EntityType.FACE && GeometryType.PLANE && ConstructionObject.NO` (planar faces / sketch regions); Target: `EntityType.FACE && (GeometryType.CYLINDER || GeometryType.CONE)`, max 1; anchor points (vertex) when custom anchors.
- **Parameters:**

| FS id | UI label | Type | Enum values / default / bounds+units | Visible when / notes |
|---|---|---|---|---|
| resultType | Creation type | WrapResultType | Solid / Surface (default) / Split (IMPRINT) | |
| operationType etc. | Result (booleanStepTypePredicate) | enum | New / Add / Remove / Intersect | Solid only |
| source | Tools | Query | planar faces | |
| destination | Target | Query | cylinder/cone face, 1 | |
| flipAlignment | Flip alignment | boolean | false | |
| trim | Trim to target | boolean | false | not Split |
| customAnchors | Specify anchor points | boolean | false | |
| sourceAnchor / destinationAnchor | Tools anchor point / Target anchor point | Query vertex | max 1 each | customAnchors |
| angle | Angle | angle | ANGLE_360_ZERO_DEFAULT_BOUNDS -1e5..1e5 deg, default 0 | group Position |
| angleOppositeDirection | - | boolean | false | |
| uShift / vShift | U shift / V shift | length | ZERO_DEFAULT_LENGTH_BOUNDS -500..500 m, default 0 | |
| uShiftOppositeDirection / vShiftOppositeDirection | - | boolean | false | |
| thickness | Thickness | length | SHELL_OFFSET_BOUNDS 1e-5..500 m, default 2.5 mm / 0.1 in | Solid only |
| oppositeDirection | - | boolean | false | Solid only |
| defaultScope / booleanScope | Merge with all / Merge scope | boolean / Query | | Solid only |

- **Behavior / semantics:** length-preserving mapping of the planar tool onto the developable target surface (arc length along circumference = U); see Part Studio Wrap item for details.
- **Edge cases & errors:** `WRAP_DESTINATION_NO_FACE`.
- **OCCT mapping:** no direct OCCT wrap; custom: map 2D curves (u, v) -> cylinder `(R cos(u/R), R sin(u/R), v)` and build faces with `BRepBuilderAPI_MakeFace(Geom_CylindricalSurface, wire-with-pcurves)`; solid via `BRepOffsetAPI_MakeThickSolid`/`BRepPrimAPI_MakePrism` along normals or `BRepOffsetAPI_ThruSections`.
- **Priority:** P2

---

#### Geometric constraints

Conventions: P = point, L = line (segment; constraint acts on its infinite underlying line unless stated), C = circle,
A = arc (constraint acts on its underlying full circle unless stated), E = ellipse/elliptical arc, S = spline/bezier/conic,
X = external geometry (edge, vertex, plane, face, other-sketch entity; treated as fixed). Residuals are a design suggestion
for the solver (SKB-08). All toolbar constraints support "toggle" mode: activate the tool, then every picked pair is
constrained; select another tool or click the tool again to exit. Multi-selection + tool applies to all selected
(pairwise or chained). Errors `SKETCH_CONSTRAINT_DIFF_ENTITIES` "Cannot add constraint between an entity and itself",
`SKETCH_CONSTRAINT_TWO_ENTITIES` apply generally. Storage: `skConstraint` with `constraintType`, `localFirst`,
`localSecond` (external references use additional external fields, not exposed in FS docs).

### SKC-01 Coincident
- **Purpose:** Make entities share location / underlying geometry.
- **Sources:** Sketch/coincident (also Drawing); `ConstraintType.COINCIDENT`. Shortcut: i.
- **Selection inputs:** two or more entities; includes a sketch entity and a plane/external entity.
- **Parameters:** none.
- **Behavior / semantics (per pair):**
  - P-P: same position (2 eq): x1-x2=0, y1-y2=0.
  - P-L: point on infinite line (1 eq): cross(p - a, b - a)/|b-a| = 0.
  - P-C/A: point on underlying circle (1 eq): |p - c| - r = 0.
  - P-E: point on underlying ellipse (1 eq; with ellipse parameter as auxiliary unknown or implicit form).
  - P-S: point on curve (auxiliary curve parameter t, 2 eq: p - S(t) = 0) (`FixedParameterPosition.FREE`/`FIXED_ON_CURVE` style).
  - L-L: collinear (2 eq): both endpoints of L2 on infinite L1.
  - C/A-C/A: same underlying circle: centers equal and radii equal (3 eq) ("The infinite, underlying geometry of the two entities is made coincident").
  - E-E: identical underlying ellipse (inferred).
  - P-X(plane): point lies on the plane's trace line in the sketch plane; L-X(plane): line collinear with trace; P-X(vertex/edge): point coincident with projection of the external vertex / on projected edge (inferred).
  - Two curves of different types: error. Two points of the same curve: error.
- **Edge cases & errors:** `SKETCH_COINCIDENT_FAILED`, `SKETCH_COINCIDENT_INPUT_ERROR` "cannot be applied to two curves of different types", `SKETCH_COINCIDENT_DIFF_POINTS` "cannot be added to two points from the same curve", `SKETCH_CONSTRAINT_COINCIDENT_TWO_ENTITIES`.
- **OCCT mapping:** custom residuals; external traces via `IntAna_QuadQuadGeo`/`GeomProjLib`.
- **Priority:** P0

### SKC-02 Concentric
- **Purpose:** Share a center.
- **Sources:** Sketch/concentric; `ConstraintType.CONCENTRIC`. Shortcut: shift+o.
- **Selection inputs:** circles, arcs, ellipses, points (sketch or external circular edges, inferred).
- **Parameters:** none.
- **Behavior / semantics:** C/A/E - C/A/E: centers coincide (2 eq). P - C/A/E: point coincides with center (2 eq). Radii stay independent.
- **Edge cases & errors:** `SKETCH_CONCENTRIC_INPUT_ERROR` "only ... circles, arcs, ellipses and points", `SKETCH_CONCENTRIC_FAILED`, `SKETCH_CONSTRAINT_CONCENTRIC_TWO_ENTITIES`.
- **OCCT mapping:** custom.
- **Priority:** P0

### SKC-03 Parallel
- **Purpose:** Make lines parallel.
- **Sources:** Sketch/parallel (also Drawing); `ConstraintType.PARALLEL`; `DimensionAlignment`. Shortcut: b.
- **Selection inputs:** two or more lines (sketch lines, external linear edges; ellipse axes inferred via directional group).
- **Parameters:** internal `alignment` ALIGNED / ANTI_ALIGNED (direction sense captured at creation so the solver does not flip; inferred from enum doc "how a constraint between lines or axes (such as parallel or coincident) should align the two constrained axes").
- **Behavior / semantics:** L-L: cross(d1, d2) = 0 (1 eq); alignment sign keeps dot(d1,d2) sign. More than two: chained.
- **Edge cases & errors:** `SKETCH_PARALLEL_INPUT_ERROR` "requires two lines", `SKETCH_PARALLEL_CONSTRAINT_FAILED`.
- **OCCT mapping:** custom.
- **Priority:** P0

### SKC-04 Tangent
- **Purpose:** G1 contact between curves, or a curve and a plane.
- **Sources:** Sketch/tangent (also Drawing); `ConstraintType.TANGENT`. Shortcut: t.
- **Selection inputs:** two or more curves (at least one per pair non-linear), or a curve and a plane/external edge.
- **Parameters:** none.
- **Behavior / semantics:**
  - L-C/A: distance(center, line) = r (1 eq), on infinite line/full circle.
  - C/A-C/A: |c1-c2| = r1 + r2 (external) or |r1 - r2| (internal); branch chosen from current configuration (1 eq).
  - A-L or A-A sharing an endpoint (coincident ends): tangent at the shared point (endpoint G1): tangent directions parallel (inferred; standard for tangent arcs).
  - S-L/A/S: tangent at the spline end (handle direction aligned with the other curve's tangent at the contact point); arbitrary interior tangency needs an auxiliary parameter (inferred).
  - E-L/C: tangency with auxiliary contact parameter (inferred).
  - Curve-plane: tangent to the plane's trace line.
  - Multiple curves: pairwise chain.
- **Edge cases & errors:** `SKETCH_TANGENT_INPUT_ERROR` "requires two curves, one of which must not be a line", `SKETCH_TANGENT_FAILED`.
- **OCCT mapping:** residuals custom; initial construction of tangent geometry `Geom2dGcc_Circ2d2TanRad`, `Geom2dGcc_Lin2d2Tan`, `GccAna_Circ2d2TanRad`.
- **Priority:** P0

### SKC-05 Horizontal
- **Purpose:** Align to sketch X axis.
- **Sources:** Sketch/horizontal (also Drawing); `ConstraintType.HORIZONTAL`. Shortcut: h.
- **Selection inputs:** one or more lines; or two or more points (incl. external vertices/origin); or an ellipse (major axis).
- **Parameters:** none.
- **Behavior / semantics:** L: dy = 0 (1 eq). Points set: all y equal (n-1 eq; implemented via an internal hidden line between points per `SKETCH_DIR_INTERNAL`). E: major axis direction horizontal. Selection must be all points or all lines/ellipses. Multiple lines -> each horizontal.
- **Edge cases & errors:** `SKETCH_HORIZONTAL_FAILED`, `SKETCH_DIR_INPUT` "A directional constraint requires one line or ellipse or two points.", `SKETCH_DIR_INTERNAL`, `SKETCH_POINT_LINE_ONLY`, `SKETCH_DIRECTIONAL_GROUP_INPUT` "The entities must be all points or all lines and ellipses."
- **OCCT mapping:** custom.
- **Priority:** P0

### SKC-06 Vertical
- **Purpose:** Align to sketch Y axis.
- **Sources:** Sketch/vertical (also Drawing); `ConstraintType.VERTICAL`. Shortcut: v.
- **Selection inputs / Parameters / errors:** as SKC-05 (`SKETCH_VERTICAL_FAILED`).
- **Behavior / semantics:** L: dx = 0. Points: all x equal. E: major axis vertical.
- **OCCT mapping:** custom.
- **Priority:** P0

### SKC-07 Perpendicular
- **Purpose:** Right angle between two lines.
- **Sources:** Sketch/perpendicular (also Drawing); `ConstraintType.PERPENDICULAR`. Shortcut: shift+l.
- **Selection inputs:** two lines (sketch or external linear edges).
- **Parameters:** none.
- **Behavior / semantics:** dot(d1, d2) = 0 (1 eq), infinite lines (need not touch).
- **Edge cases & errors:** `SKETCH_PERPENDICULAR_INPUT_ERROR` "requires two lines", `SKETCH_PERPENDICULAR_FAILED`.
- **OCCT mapping:** custom.
- **Priority:** P0

### SKC-08 Equal
- **Purpose:** Equal lengths or radii.
- **Sources:** Sketch/equal; `ConstraintType.EQUAL`. Shortcut: e.
- **Selection inputs:** two or more lines, or two or more arcs/circles (curves of like type).
- **Parameters:** none.
- **Behavior / semantics:** L-L: |b1-a1| = |b2-a2| (1 eq). C/A-C/A: r1 = r2 (1 eq). E-E: equal major and minor radii (inferred). If one of the curves is dimensioned, its size drives the others. Multiple -> all equal (chained n-1 constraints).
- **Edge cases & errors:** `SKETCH_EQUAL_INPUT_ERROR` "only ... between two lines or two curves", `SKETCH_EQUAL_NO_ENDS`, `SKETCH_EQUAL_FAILED`, `SKETCH_CONSTRAINT_EQUAL_TWO_ENTITIES`.
- **OCCT mapping:** custom.
- **Priority:** P0

### SKC-09 Midpoint
- **Purpose:** Place a point at the middle of a line/arc or between two points.
- **Sources:** Sketch/midpoint; `ConstraintType.MIDPOINT`. Shortcut: shift+m.
- **Selection inputs:** point + line, point + arc, or three points (start, end, middle in that order).
- **Parameters:** none.
- **Behavior / semantics:** P-L: p = (a+b)/2 (2 eq) on the segment. P-A: point at angular midpoint of the arc (on arc, equidistant from both ends) (2 eq). 3 points: middle point = midpoint of start/end (an internal construction line is created between start and end and the point is constrained to its midpoint, per errors). Cannot use the segment's own endpoint.
- **Edge cases & errors:** `SKETCH_MIDPOINT_INPUT_ERROR`, `SKETCH_MIDPOINT_NEED_POINT`, `SKETCH_MIDPOINT_NEED_DIFF_POINT`, `SKETCH_MIDPOINT_MISSING_ENDS`, `SKETCH_MIDPOINT_MISSING_PTS`, `SKETCH_MIDPOINT_NO_INTERNAL_LINE`, `SKETCH_MIDPOINT_NO_COINCIDENT`, `SKETCH_MIDPOINT_FAILED`, `SKETCH_CONSTRAINT_MIDPOINT_TWO_ENTITIES`.
- **OCCT mapping:** custom.
- **Priority:** P0

### SKC-10 Normal
- **Purpose:** Make a line normal to a curve, or a curve normal to a plane.
- **Sources:** Sketch/normal; `ConstraintType.NORMAL`. Shortcut: shift+k.
- **Selection inputs:** line + curve (circle, arc, ellipse, spline), or curve + plane.
- **Parameters:** none.
- **Behavior / semantics:** L-C/A: infinite line passes through the center (1 eq). L-E/S: line perpendicular to the curve tangent at their intersection (auxiliary contact parameter: point on both; 1-3 eq) (inferred). Curve-plane: curve meets the plane's trace at a right angle (inferred).
- **Edge cases & errors:** `SKETCH_NORMAL_NEED_LINE` "Normal constraint requires a line.", `SKETCH_NORMAL_INPUT_ERROR`, `SKETCH_NORMAL_INPUT_NEEDED` "requires a line and a curve".
- **OCCT mapping:** custom; `Geom2dAPI_ProjectPointOnCurve` for initial contact parameter.
- **Priority:** P1

### SKC-11 Pierce
- **Purpose:** Constrain a sketch point/curve to the point where an out-of-plane curve passes through the sketch plane.
- **Sources:** Sketch/pierce; `ConstraintType.PIERCE`. Shortcut: shift+g.
- **Selection inputs:** a sketch point or curve + an external edge/curve that crosses the sketch plane (not a plane).
- **Parameters:** none.
- **Behavior / semantics:** Compute 3D intersection of the external curve with the sketch plane -> fixed 2D point Q. P: p = Q (2 eq). Curve: Q lies on the curve (1 eq). Used for sweep/loft guide setups; also auto-created by Intersection tool for open-curve ends. Edge must actually pass through the plane.
- **Edge cases & errors:** `SKETCH_CONSTRAINT_PIERCE_TWO_ENTITIES` "requires a curve or point from the sketch and an external edge", `SKETCH_PIERCE_FAILED`, `SKETCH_CANNOT_PIERCE_WITH_PLANE`.
- **OCCT mapping:** `GeomAPI_IntCS(curve3d, Geom_Plane)` or `IntAna_IntConicQuad`; choose nearest of multiple solutions.
- **Priority:** P1

### SKC-12 Symmetric (MIRROR)
- **Purpose:** Two same-type geometries symmetric about a line.
- **Sources:** Sketch/symmetric; `ConstraintType.MIRROR`. Shortcut: shift+q.
- **Selection inputs:** first a line (sketch line, linear edge, or plane, per toolbar description), then two geometries of the same type.
- **Parameters:** none.
- **Behavior / semantics:** P-P: midpoint on axis and segment perpendicular to axis (2 eq). L-L: endpoints pairwise symmetric (or underlying lines mirror; inferred: underlying lines). C/A-C/A: centers symmetric and radii equal; arc endpoints NOT constrained ("Symmetric only constrains the underlying curve"). Also used by Sketch mirror (SKT-04).
- **Edge cases & errors:** `SKETCH_MIRROR_NEEDS_LINE_AND_TWO_OTHERS`.
- **OCCT mapping:** custom.
- **Priority:** P0

### SKC-13 Fix
- **Purpose:** Ground a sketch entity.
- **Sources:** Sketch/fix; `ConstraintType.FIX`; sheetMetalHem.fs usage. Shortcut: shift+j.
- **Selection inputs:** one or more sketch entities (one per constraint).
- **Parameters:** none.
- **Behavior / semantics:** P: position fixed (2 eq). Curve: underlying geometry fixed, not its endpoints: e.g. Fix on an arc fixes center and radius, ends can still slide along the circle (stdlib comment "Fix the center and radius of the arc... This does not fix the end position of the arc"); Fix on a line fixes the infinite line (inferred by analogy); ellipse fixes center, axes, radii. Fix values are the current solved positions.
- **Edge cases & errors:** `SKETCH_FIX_ONE_ENT` "A fix constraint requires one entity.", `SKETCH_FIX_FAILED`.
- **OCCT mapping:** custom (remove parameters from unknowns or add equality residuals).
- **Priority:** P0

### SKC-14 Curvature (EQUAL_CURVATURE)
- **Purpose:** G2 (curvature-continuous) transition.
- **Sources:** Sketch/curvature; `ConstraintType.EQUAL_CURVATURE`. Shortcut: shift+u.
- **Selection inputs:** two curves meeting at an endpoint, at least one a conic or spline (other may be line, arc, spline, conic).
- **Parameters:** none.
- **Behavior / semantics:** At the shared point: tangent continuity plus equal signed curvature (curvature of a line = 0, of an arc = 1/r) (inferred: implies G1; may be combined with coincident).
- **Edge cases & errors:** `SKETCH_EQUAL_CURVATURE_FAILED`, `SKETCH_EQUAL_CURVATURE_INPUT_ERROR` "requires two curves one of which must be a conic or spline".
- **OCCT mapping:** curvature from `Geom2dLProp_CLProps2d`; residual custom.
- **Priority:** P1

### SKC-15 Quadrant (auto)
- **Purpose:** Point on an ellipse at an axis end.
- **Sources:** Sketch/working_with_constraints (Constraints created automatically); `ConstraintType.QUADRANT`.
- **Selection inputs:** created by inference, dragging, or placing onto an ellipse's axis points; not on toolbar.
- **Parameters:** none.
- **Behavior / semantics:** Point coincident with the ellipse and lying on its major or minor axis (one of 4 quadrant points; branch stored) (2 eq).
- **Edge cases & errors:** `QUADRANT_CONSTRAINT_INPUT` "requires a point and an ellipse", `SKETCH_QUADRANT_FAILED`.
- **OCCT mapping:** custom.
- **Priority:** P1

### SKC-16 Use / Projected (auto)
- **Purpose:** Bind a sketch entity to the projection of external geometry.
- **Sources:** Sketch/working_with_constraints, Sketch/use; `ConstraintType.PROJECTED`.
- **Selection inputs:** created by Use tool (SKT-09).
- **Parameters:** internal projection type (`SketchProjectionType.USE`, `USE_END`).
- **Behavior / semantics:** Sketch entity's parameters equal the projected external geometry (all DOF removed; entity black); re-evaluated on regen; shown with blue icon background. Deleting the constraint frees the geometry as a normal sketch entity (inferred).
- **Edge cases & errors:** `SKETCH_EXTERNAL_GEOMETRY_MISMATCH`, `SKETCH_PROJECTION_FAILED`.
- **OCCT mapping:** see SKT-09.
- **Priority:** P0

### SKC-17 Silhouetted (auto)
- **Purpose:** Bind a sketch line/curve to a face silhouette.
- **Sources:** Sketch/use; `ConstraintType.SILHOUETTED`; `SketchProjectionType.SILHOUETTE_START/END`; `SketchSilhouetteDisambiguation`.
- **Selection inputs:** created by silhouette Use (SKT-10).
- **Parameters:** disambiguation enum (which of multiple silhouettes: ALIGNED/ANTI_ALIGNED, OUTER/INNER, TORUS_TOP/BOTTOM/LEFT/RIGHT).
- **Behavior / semantics:** Sketch curve lies on the silhouette (underlying line/circle), ends free (user constrains ends).
- **Edge cases & errors:** `SILHOUETTE_USE_FAILED`, `SKETCH_SIL_PROJECTION_*`.
- **OCCT mapping:** see SKT-10.
- **Priority:** P1

### SKC-18 Intersected (auto)
- **Purpose:** Bind closed sketch curves to a face/plane intersection.
- **Sources:** Sketch/working_with_constraints, Sketch/intersection; `ConstraintType.INTERSECTED`.
- **Selection inputs:** created by Intersection tool (SKT-11).
- **Parameters:** none.
- **Behavior / semantics:** Closed intersection curves are fully bound; open intersection curve ends use Pierce constraints instead.
- **Edge cases & errors:** `SKETCH_INTERSECTION_*`.
- **OCCT mapping:** `BRepAlgoAPI_Section`.
- **Priority:** P1

### SKC-19 Offset (auto)
- **Purpose:** Keep an offset curve at a distance from its base.
- **Sources:** Sketch/offset; `ConstraintType.OFFSET`; errors `SKETCH_OFFSET_BAD_PAIR`, `SKETCH_OFFSET_INPUT_ERROR`.
- **Selection inputs:** created by Offset (SKT-03) and Slot (SKE-20).
- **Parameters:** distance (length dimension, linked across the offset set).
- **Behavior / semantics:** Valid pairs: two curves of the same type (line-line: parallel at distance; arc/circle: concentric with r2 = r1 +/- d; spline/ellipse: offset curve) or a circle and a point (circle centered on point with radius d). One or two pairs of entities per constraint (offset of a curve also binds its endpoints, inferred).
- **Edge cases & errors:** `SKETCH_OFFSET_BAD_PAIR`, `SKETCH_OFFSET_INPUT_ERROR`, `SKETCH_OFFSET_CONSTRAINT_FAILED`.
- **OCCT mapping:** `Geom2d_OffsetCurve` for evaluation.
- **Priority:** P1

### SKC-20 Linear pattern (auto)
- **Purpose:** Maintain linear sketch pattern relationships.
- **Sources:** Sketch/sketch_linear_pattern; `ConstraintType.LINEAR_PATTERN`.
- **Selection inputs:** created by SKT-05.
- **Parameters:** count(s) (integer, no variables), spacing, direction angle.
- **Behavior / semantics:** Each instance = seed translated by i*spacing*dir (+ j*spacing2*dir2); deleting an instance leaves a gap.
- **Edge cases & errors:** see SKT-05.
- **OCCT mapping:** custom residuals.
- **Priority:** P1

### SKC-21 Circular pattern (auto)
- **Purpose:** Maintain circular sketch pattern relationships.
- **Sources:** Sketch/sketch_circular_pattern; `ConstraintType.CIRCULAR_PATTERN`.
- **Selection inputs:** created by SKT-06.
- **Parameters:** count, angle (closed 360 default or open), center point.
- **Behavior / semantics:** Instance i = seed rotated by i*step about center.
- **Edge cases & errors:** see SKT-06.
- **OCCT mapping:** custom residuals.
- **Priority:** P1

### SKC-22 Bezier degree (auto)
- **Purpose:** Persist/edit the degree of a bezier sketch entity.
- **Sources:** Sketch/bezier; `ConstraintType.BEZIER_DEGREE`.
- **Selection inputs:** double-click degree label of a bezier.
- **Parameters:** degree (integer, max 15).
- **Behavior / semantics:** Changing degree rebuilds the control polygon (elevation adds control points; reduction approximates; inferred).
- **Edge cases & errors:** none documented.
- **OCCT mapping:** `Geom2d_BezierCurve::Increase`; reduction custom least squares.
- **Priority:** P2

### SKC-23 Freeze
- **Purpose:** Enum value `ConstraintType.FREEZE`; no help documentation.
- **Sources:** constrainttype.gen.fs only.
- **Selection inputs:** unknown.
- **Parameters:** unknown.
- **Behavior / semantics:** Undocumented; likely locks an entity's full shape (e.g. inserted DXF/DWG or text/image frames) beyond Fix (inferred, unverified). Reserve the enum value for file compatibility.
- **Edge cases & errors:** none.
- **OCCT mapping:** n/a.
- **Priority:** P2

---

#### Dimensions

Conventions: dimensions are constraints with a value (`skConstraint` `length` or `angle`). Type is chosen from the
picked entities and the placement drag position (horizontal/vertical/aligned zones; angle quadrant; centerline side).

### SKD-01 Dimension tool (general behavior)
- **Purpose:** Single smart tool creating all sketch dimension types.
- **Sources:** Sketch/dimension; Sketch/sketch_basics. Shortcut: d.
- **Selection inputs:** 1-3 picks among points, lines, arcs, circles, ellipses, splines/spline points, bezier control points, text/image boxes, external planes/edges/vertices; then a placement click.
- **Parameters:** value (length/angle expression); driving/driven state.
- **Behavior / semantics:**
  - Pick entities, drag to position, click to place; the value field opens immediately; Enter accepts. Esc keeps the measured value (inferred).
  - Edit later: double-click the value; Delete key or context menu Delete removes; right-click "Add comment"; right-click "Change to driving/driven dimension"; drag to reposition.
  - Value change distribution: except for radius/diameter, when a new value is entered both dimensioned geometries are translated by half of the change so they move equally (e.g. both endpoints of a dimensioned line move by half); constrained geometry may not move (solver decides).
  - Dimension to a plane: the sketch geometry moves, the plane stays (external geometry fixed).
  - Between two lines the distance dimension implies parallelism without a visible parallel constraint.
  - The first dimension applied to an inserted image or inserted DXF/DWG scales that content.
  - Dimension shows expression icons (SKB-04).
- **Edge cases & errors:** `SKETCH_ADD_DIMENSION_FAILED`, `SKETCH_POSITION_DIMENSION_FAILED`, `SKETCH_MODIFY_DIM_FAILED`, `SKETCH_DIMENSION_DIFF_ENTITIES` "A dimension can only be added between two different geometries.", `SKETCH_DIMENSION_TWO_ENTITIES`, `SKETCH_DIMENSION_INPUT_ERROR` "A dimension cannot be added between a curve and one of its points.", `SKETCH_DIMENSION_INFINITY`, `SKETCH_DIMENSION_LIMIT_ERROR` "The dimension value is outside its limits.", `CANNOT_EVALUATE_DIMENSION`.
- **OCCT mapping:** none (solver + annotation renderer); measurement via `Geom2dAPI_ExtremaCurveCurve`, `GCPnts_AbscissaPoint::Length`.
- **Priority:** P0

### SKD-02 Line length
- **Purpose:** Length of a single line.
- **Sources:** Sketch/dimension (Single line distance); `ConstraintType.LENGTH`.
- **Selection inputs:** one line.
- **Parameters:** length (length; negative allowed: flips the line direction).
- **Behavior / semantics:** |b - a| = L (1 eq), or directed: drag the dimension to choose aligned (true length), horizontal, or vertical extent (horizontal/vertical extent stored as DISTANCE between endpoints with direction, inferred).
- **Edge cases & errors:** `SKETCH_LENGTH_DIM_INPUT_ERROR` "Length dimensions require a single line, spline or conic.", `SKETCH_LENGTH_DIM_MISSING_ENDS`, `SKETCH_LENGTH_DIM_NOT_FOUND`, `SKETCH_LENGTH_DIM_FAILED`.
- **OCCT mapping:** custom.
- **Priority:** P0

### SKD-03 Point-to-point distance (aligned / horizontal / vertical)
- **Purpose:** Distance between two points.
- **Sources:** Sketch/dimension (Diagonal distance, Direct distance, Linear distance); `ConstraintType.DISTANCE` with `direction` `DimensionDirection.MINIMUM | HORIZONTAL | VERTICAL` and `alignment`.
- **Selection inputs:** two points (endpoints, centers, sketch points, spline points, external vertices, origin).
- **Parameters:**

| FS id | UI label | Type | Enum values / default / bounds+units | Visible when / notes |
|---|---|---|---|---|
| length | value | length | negative allowed (flips side) | |
| direction | (placement) | DimensionDirection | MINIMUM (drag at an angle -> direct/diagonal), HORIZONTAL (drag up/down), VERTICAL (drag left/right) | |
| alignment | - | DimensionAlignment | ALIGNED / ANTI_ALIGNED | sign convention: which point is first along the measured axis (stdlib comment "use ANTI_ALIGNED when localFirst is ... higher than localSecond") |

- **Behavior / semantics:** MINIMUM: |p2 - p1| = d. HORIZONTAL: (x2 - x1)*s = d. VERTICAL: (y2 - y1)*s = d, with s = +/-1 from alignment (directed dimension; keeps order, so negative value flips).
- **Edge cases & errors:** `SKETCH_DIMENSION_DIST_ERROR`, `SKETCH_DIMENSION_FAILED`.
- **OCCT mapping:** custom.
- **Priority:** P0

### SKD-04 Point-to-line distance
- **Purpose:** Perpendicular distance from a point to a line.
- **Sources:** Sketch/dimension; `ConstraintType.DISTANCE`; `DimensionHalfSpace`.
- **Selection inputs:** point + line (either may be external).
- **Parameters:** length (negative allowed: puts the point on the other side); internal half-space LEFT/RIGHT records which side of the line the point is on (inferred from `DimensionHalfSpace`).
- **Behavior / semantics:** signed distance cross(p - a, d)/|d| = s*D where s from half-space; to the infinite line.
- **Edge cases & errors:** as SKD-03.
- **OCCT mapping:** custom.
- **Priority:** P0

### SKD-05 Line-to-line distance
- **Purpose:** Distance between two (parallel) lines.
- **Sources:** Sketch/dimension (Distance between 2 lines).
- **Selection inputs:** two lines.
- **Parameters:** length (negative allowed).
- **Behavior / semantics:** Implies parallelism (no visible parallel constraint): both endpoints of L2 at signed distance D from infinite L1 (2 eq) (inferred residual form).
- **Edge cases & errors:** non-parallel lines become parallel on solve.
- **OCCT mapping:** custom.
- **Priority:** P0

### SKD-06 Distance to plane / external geometry
- **Purpose:** Dimension from sketch geometry to a plane or external edge/vertex.
- **Sources:** Sketch/dimension (Between sketch geometry and plane); Sketch/sketch_tools ("apply constraints (including dimensions) between sketch curves and planes").
- **Selection inputs:** sketch point/line + plane (seen as its trace line), external edge or vertex.
- **Parameters:** length.
- **Behavior / semantics:** Plane treated as fixed trace line in the sketch; sketch geometry moves to satisfy the value. If the plane is parallel to the sketch plane it cannot be a planar trace (inferred: such picks not allowed).
- **Edge cases & errors:** `SKETCH_UNSOLVABLE_CONSTRAINT` when reference becomes inapplicable.
- **OCCT mapping:** trace via `IntAna_QuadQuadGeo(gp_Pln, gp_Pln)`.
- **Priority:** P0

### SKD-07 Dimensions to circles and arcs (center / min / max)
- **Purpose:** Distance involving circles/arcs.
- **Sources:** `DimensionMeasurementReferenceType {AXIS, MINIMUM, MAXIMUM}` (FsDoc: how to measure distance between cylindrical faces or a cylindrical face and a plane; used by `setDimensionedEntities`); Sketch/sketch_transform ("minimum dimensions"); Sketch/dimension (centerline dimension from a circle). Sketch help does not document min/max circle dimensions explicitly.
- **Selection inputs:** circle/arc + point/line/circle.
- **Parameters:** length; reference type (center default; min/max tangent variants inferred).
- **Behavior / semantics:** Default: distance measured to the circle center (inferred). Min/max: distance to nearest/farthest tangent point = center distance -/+ r (inferred implementation, P2).
- **Edge cases & errors:** none documented.
- **OCCT mapping:** custom.
- **Priority:** P2

### SKD-08 Angular dimension
- **Purpose:** Angle between two lines.
- **Sources:** Sketch/dimension (Angle); `ConstraintType.ANGLE`.
- **Selection inputs:** two lines (sketch or external linear edges). Three-point angle (vertex + two points) is not documented; `SKETCH_ANGLE_TWO_LINES` "Angle dimensions require two lines." (gap: construct with construction lines).
- **Parameters:** angle (angle units; negative allowed: flips direction).
- **Behavior / semantics:** Placement quadrant chooses which of the four angles between the two infinite lines is measured (theta, 180-theta supplementary, and their vertical opposites): drag into/out of the angle or into the desired quadrant before clicking. Residual: atan2(cross(d1,d2), dot(d1,d2)) = theta with the line direction senses chosen by the quadrant (inferred implementation).
- **Edge cases & errors:** `SKETCH_ANGLE_TWO_LINES`.
- **OCCT mapping:** custom; `gp_Dir2d::Angle`.
- **Priority:** P0

### SKD-09 Radius
- **Purpose:** Radius of an arc (or circle).
- **Sources:** Sketch/dimension (Radius); `ConstraintType.RADIUS`; `RadiusDisplay {RADIAL, DIAMETRAL}`.
- **Selection inputs:** one arc/circle.
- **Parameters:** radius (length, positive only; bounds SKETCH_RADIUS_BOUNDS 5e-6..500 m).
- **Behavior / semantics:** r = R (1 eq). Default for arcs. Center stays put on value change (no half-split rule).
- **Edge cases & errors:** `SKETCH_RADIUS_INPUT_ERROR` "Radius dimensions require a single circle or arc.", `SKETCH_RADIUS_DIM_FAILED`; negative not allowed.
- **OCCT mapping:** custom.
- **Priority:** P0

### SKD-10 Diameter
- **Purpose:** Diameter of a circle (or arc).
- **Sources:** Sketch/dimension (Diameter); `ConstraintType.DIAMETER`.
- **Selection inputs:** one circle (default diameter) or arc.
- **Parameters:** diameter (length, positive only).
- **Behavior / semantics:** 2r = D. Drag into/away from the circle to place. Radius/diameter display toggle via RadiusDisplay (inferred: context command).
- **Edge cases & errors:** negative not allowed.
- **OCCT mapping:** custom.
- **Priority:** P0

### SKD-11 Ellipse major / minor diameter
- **Purpose:** Size an ellipse.
- **Sources:** Sketch/ellipse, Sketch/elliptical_arc; `ConstraintType.MAJOR_DIAMETER`, `MINOR_DIAMETER`.
- **Selection inputs:** ellipse/elliptical arc (choose axis by placement; inferred).
- **Parameters:** primary axis diameter, secondary axis diameter (length).
- **Behavior / semantics:** 2*Rmaj = D1; 2*Rmin = D2 ("primary"/"secondary" = first/second drawn axis; either can be the longer).
- **Edge cases & errors:** `SKETCH_ELLIPSE_RADIUS_INPUT_ERROR` "Radius dimensions require a single circle, arc or ellipse."
- **OCCT mapping:** custom.
- **Priority:** P1

### SKD-12 Arc length / curve length
- **Purpose:** Length along an arc (or spline/conic).
- **Sources:** Sketch/dimension (Arc length / curve length); `ConstraintType.LENGTH` (error text lists line, spline or conic).
- **Selection inputs:** arc: click first endpoint, second endpoint, then the arc; spline/conic: the curve (inferred).
- **Parameters:** length (positive only for arcs per Negative dimensions note; "Arc dimension" not negative).
- **Behavior / semantics:** Arc: r * sweepAngle = L. Spline/conic: integral of |S'(t)| = L (numeric) (inferred).
- **Edge cases & errors:** as SKD-02.
- **OCCT mapping:** `GCPnts_AbscissaPoint::Length(Geom2dAdaptor_Curve)`; derivative w.r.t. parameters numerically.
- **Priority:** P1

### SKD-13 Spline point / handle dimensions
- **Purpose:** Dimension spline through-points, handles, bezier control points.
- **Sources:** Sketch/dimension (Spline), Sketch/spline, Sketch/bezier.
- **Selection inputs:** spline point / tangent handle end / control point + any sketch point.
- **Parameters:** length (negative allowed for spline dimensions).
- **Behavior / semantics:** Same as SKD-03 (horizontal/vertical/direct by drag direction). Handle length/direction can be dimensioned (angle between handle and a line via construction, inferred).
- **Edge cases & errors:** none documented.
- **OCCT mapping:** custom.
- **Priority:** P1

### SKD-14 Centerline dimension
- **Purpose:** Diametral dimension across a construction centerline (for revolve profiles).
- **Sources:** Sketch/dimension (Centerline dimensions); `ConstraintType.CENTERLINE_DIMENSION`.
- **Selection inputs:** circle, point, or non-construction line + a construction line.
- **Parameters:** length (= twice the perpendicular distance to the centerline).
- **Behavior / semantics:** Start as a distance dimension; dragging the placement to the opposite side of the construction line toggles to centerline dimension (and back). Residual: 2*dist(p, centerline) = D.
- **Edge cases & errors:** requires construction line.
- **OCCT mapping:** custom.
- **Priority:** P1

### SKD-15 Rho dimension
- **Purpose:** Shape parameter of a conic.
- **Sources:** Sketch/conic; `ConstraintType.RHO`.
- **Selection inputs:** created with the conic.
- **Parameters:** rho (unitless, 0 < rho < 1).
- **Behavior / semantics:** Always present; cannot be deleted; may be driven by expression/variable; `fixedRho` keeps it constant during solve.
- **Edge cases & errors:** `CANNOT_DELETE_RHO_DIMENSION`, `SKETCH_RHO_DIM_NOT_FOUND`.
- **OCCT mapping:** weight w = rho/(1-rho) on a rational quadratic Bezier.
- **Priority:** P2

### SKD-16 Driven (reference) dimensions
- **Purpose:** Display-only measured dimensions.
- **Sources:** Sketch/dimension (Driven dimensions); Sketch/working_with_constraints (Constraint manager Status: Driven).
- **Selection inputs:** any dimension; right-click "Change to driving/driven dimension".
- **Parameters:** driving (default) / driven state.
- **Behavior / semantics:** Driving: black, editable, constrains geometry. Driven: light gray, not editable, value tracks the current geometry, no effect on the solve. A dimension that would over-define the sketch is automatically created driven. Driven dimensions can be added anywhere a driving one can. Switching driven -> driving applies the value; if over-constrained, red indicators appear. Useful for design intent checks (clearance/wall thickness).
- **Edge cases & errors:** none.
- **OCCT mapping:** none (solver excludes; evaluate measurement after solve).
- **Priority:** P0

### SKD-17 Negative values and dimension orientation data
- **Purpose:** Sign semantics of directed dimensions.
- **Sources:** Sketch/dimension (Negative dimensions); `DimensionAlignment`, `DimensionHalfSpace`, `DimensionDirection` enums; sheetMetalHem.fs comments.
- **Selection inputs:** n/a.
- **Parameters:** stored alignment (ALIGNED/ANTI_ALIGNED), half-space (LEFT/RIGHT), direction (MINIMUM/HORIZONTAL/VERTICAL).
- **Behavior / semantics:** Negative values allowed for distance, angle, line length and spline dimensions: geometry flips to the other side/direction (e.g. a rectangle dimensioned -x from a line moves to the other side). Not allowed for arc length, radius or diameter. Implementation: store orientation at creation; value sign multiplies the orientation.
- **Edge cases & errors:** `SKETCH_DIMENSION_LIMIT_ERROR`.
- **OCCT mapping:** none.
- **Priority:** P1

### SKD-18 Dimension values: expressions, variables, configurations
- **Purpose:** Parametric dimension values.
- **Sources:** Sketch/dimension, Sketch/sketch_basics, Home/numeric_fields, PartStudio/variable, PartStudio/configurations (sketch dimensions configurable).
- **Selection inputs:** dimension value field.
- **Parameters:** expression string.
- **Behavior / semantics:** Accepts full numeric-field grammar (SKB-21); variable reference `#name` (when editing an existing dimension) or `=#name` (in the on-the-fly box while sketching); "#" lists variables + "New variable". Expressions re-evaluate on regen when variables change; a dimension with an expression shows the expression icon; Show expressions displays the text (70-char limit). Sketch dimensions and sketch text can be configuration parameters.
- **Edge cases & errors:** unit mismatch rejected (e.g. angle value in a length dimension -> `SKETCH_DIMENSION_DIST_ERROR`).
- **OCCT mapping:** none (expression engine).
- **Priority:** P0

