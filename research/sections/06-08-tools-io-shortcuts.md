## 6. Measurement, view & analysis tools

Scope: the "Model evaluation tools" (Measure, Mass properties, Analysis tools) that sit at the bottom-right corner of the
Part Studio / Assembly (Measure also in Drawings), the "Camera and render options" menu (view cube dropdown, top-right),
per-part visibility tools (Hide/Show, Isolate, Make transparent), smart selection (Create selection / Select >),
graphics-area context menus, appearance (colors/opacity), UI color conventions and printing.
Selection mechanics, Select other, box select and all shortcut keys are in section 8 (KB-); this section cross-references them.

Source tags: [DOCS] = help page, [FS] = FeatureScript std library, [BUNDLE] = strings/constants from the Onshape web-client JS bundle
(`/root/onshape-research/docs-raw/bundle-*.txt`). Help URL prefix: `https://cad.onshape.com/help/Content/`.

Common architecture notes (design suggestion):
- All visualization modes are renderer state, not model state (except named views, which are stored per workspace, and appearances, which are part of the Part Studio model).
- Analysis overlays (curvature maps, zebra, draft, thickness) are computed on the display tessellation per vertex/fragment in GPU shaders, with exact B-rep evaluation (OCCT `BRepLProp_SLProps`/`GeomLProp_SLProps`) done per mesh vertex using the triangulation's UV nodes (`Poly_Triangulation::UVNode`) — do the exact evaluation on a worker thread and upload as vertex attributes.
- Measurements and mass properties must use exact B-rep geometry (not tessellation), except for meshes.

### MT-01 Measure tool (overview, UI and interaction)
- **Purpose:** live measurement panel showing every measurement applicable to the current selection.
- **Sources:** View/measure_tool.htm [DOCS]; FS `evDistance`, `evLength`, `evArea` (evaluate.fs) define the underlying computations [FS]. Shortcut `[` (opens Measure dialog) [DOCS]. Bundle action `SHOW_MEASURE_DIALOG=showMeasure`; tooltip "Show measure details (__measureShortcutKeys__)" [BUNDLE].
- **Selection inputs:** any vertices, sketch points, edges (sketch curves, part edges, curves), faces (incl. mesh faces and section-view faces), mate connectors (implicit/explicit), origin, planes; selections from graphics area or Feature/Parts/Instances lists. Hover midpoints of edges/sketch entities are selectable for measuring [DOCS Home/selection]. No max count.
- **Parameters (dialog controls):**

  | Control | Type | Values / default | Notes |
  |---|---|---|---|
  | Measure type | enum dropdown | Show all (default), Position, Center position, Min distance, Max distance, Length, Diameter, Radius, Angle, Face tangent angle, Curvature deviation, Surface area | filters which result rows are shown |
  | Length unit | enum | Centimeter, Foot, Inch, Meter, Millimeter, Yard; default = document/workspace unit | change any time |
  | Angle unit | enum | Degree, Radian; default = workspace unit | |
  | Reference coordinate system | checkbox | off (frame = world origin) | desktop only; reveals "Mate connector for reference coordinate system" selection field (1 mate connector; press K to show all mate connectors) |
  | Part Studio / Flat View tabs | tabs | shown only when the sheet metal table/flat view panel is open | measure 3D model vs flat pattern |

- **Behavior / semantics:**
  - Quick measure: selecting entities shows a compact readout in the bottom-right corner without opening the dialog; clicking the icon ("Show measure details") opens the full dialog. Dialog updates live as selection changes.
  - Messages: "Select entities to measure", "No measurements available for current selection.", "No measurements of this type available for current selection." [BUNDLE].
  - Hovering a result row draws the measured quantity in the graphics area (see MT-03 visualization rules). Clicking a distance row pins its dotted line (stays visible while working); clicking again reverts to hover-only.
  - When a sketch entity is selected in a Part Studio, a sketch-entity-type icon is shown in the dialog.
  - Works while Section view is active (on faces/edges/vertices of the cut).
  - With >2 selections: total surface area (faces), total length (edges); in assemblies also min distance, max distance, center-axis distance (see MT-02).
- **Edge cases & errors:** measurements of meshes: point/area/distance supported; face tangent evaluation on mesh not available (FS error `EVALUATE_FACE_TANGENT_FOR_MESHES` "Cannot evaluate face tangent planes for meshes.") [FS].
- **OCCT mapping:** see MT-02 per quantity. UI: GPUI floating panel; results recomputed on a worker thread on every selection change (debounce ~50 ms, inferred).
- **Priority:** P0.

### MT-02 Measure: results by selection combination
- **Purpose:** define exactly which values the Measure tool reports for each selection set.
- **Sources:** View/measure_tool.htm (option list + video transcript) [DOCS]; result labels from bundle i18n: "Arc length", "Center distance", "Center axis distance", "Parallel distance", "Parallel axis distance", "Perpendicular distance", "Axis perpendicular distance", "Perpendicular axis angle", "Perpendicular line angle", "Perpendicular plane angle", "Perpendicular line and plane angle", "Min distance", "Max distance", "Min radius", "Max radius", "Center position", "Position", "Point", "Face tangent angle", "Curvature deviation", "Surface area", "Length", "Diameter", "Radius", "Angle" [BUNDLE]. Docs do not publish a full matrix; rows below marked (inferred) combine the documented labels with standard Onshape behavior.
- **Result glossary (documented):** Diameter (arc/circle); Radius (arc); Center Dist. (center-to-center between two entities, with X/Y/Z components); Perp axes angle; Area; Face tangent angle (tangent angle between a surface and a line that meets it); Curvature deviation (between a surface and a line that meets it); Center (X,Y,Z of arc/circle center); Min Dist. (X,Y,Z); Max Dist. (X,Y,Z); Min Rad Pt. / Max Rad Pt. (minimum/maximum elliptical radial point X,Y,Z); Length (line length, circle circumference, arc length); Point (X,Y,Z of point/vertex/mate connector).
- **Matrix:**

  | Selection | Reported values |
  |---|---|
  | 1 vertex / sketch point / mate connector / origin | Point/Position X, Y, Z (in reference frame) |
  | 1 straight edge / sketch line | Length (inferred: also endpoint positions not shown) |
  | 1 circle (edge) | Length (circumference), Diameter, Radius, Center X/Y/Z |
  | 1 arc | Length (= arc length; "Arc length" label), Radius, Diameter, Center X/Y/Z |
  | 1 ellipse / elliptical arc | Length, Center, Min Rad Pt. and Max Rad Pt. (X,Y,Z) ("Min radius"/"Max radius") (inferred for labels) |
  | 1 spline / other curve | Length |
  | 1 planar face | Surface area |
  | 1 cylindrical / conical / spherical face | Surface area, Radius, Diameter (inferred: radius at base for cone not shown), Center/axis position for circular edges (documented: "When measuring to the center of a circle, you can select a planar face, edge, or edge of a cylinder") |
  | 2 vertices / points | Distance (Min = Max = Center), deltas X, Y, Z |
  | point + edge or face | Min distance (+ dX/dY/dZ), Max distance (+ dX/dY/dZ); Perpendicular distance when point projects onto a line/plane (inferred) |
  | 2 straight lines | Angle; if parallel: Parallel distance; Min distance and Max distance with X/Y/Z components ("Select two lines to check if they are parallel") |
  | line + planar face | Angle ("Perpendicular line and plane angle" when perpendicular, inferred), Min/Max distance; parallel → Parallel distance |
  | 2 planar faces | Angle (Perpendicular plane angle when 90°), Parallel distance if parallel, Min/Max distance (+XYZ), total area |
  | 2 circular edges / arcs | Center distance (+X,Y,Z), Min distance, Max distance (+XYZ), total length |
  | 2 cylindrical faces (or fillet faces) | Center axis distance / Parallel axis distance (if axes parallel), Perpendicular axis angle / axis angle, Min/Max distance, total area ("Select two faces to determine the distance between their parallel axis, total area, and minimum and maximum distances") |
  | face + line meeting the face | Face tangent angle, Curvature deviation (plus Min/Max distance) |
  | >2 entities | Total surface area of faces, total length of edges; in assemblies also Min distance, Max distance, center axes distance, length |
- **Behavior / semantics:**
  - Min distance = global minimum of Euclidean distance between the two entity sets; Max distance = global maximum distance between any point of A and any point of B (FS `evDistance` with `maximum: true`).
  - Center distance: between centers (circle/arc/sphere centers, or point positions); for cylinders the axis-to-axis distance.
  - All X/Y/Z values are expressed in the active reference frame (MT-04).
  - Star (✱) next to a value = non-zero but within coincidence/tangency tolerance (MT-03).
- **Edge cases & errors:** Max distance undefined if either side is infinite (planes/lines with `extend`) — FS disallows `maximum` with lines or extended sides [FS evDistance doc].
- **OCCT mapping:**
  - Min distance: `BRepExtrema_DistShapeShape` (Value(), PointOnShape1/2, SupportOnShape) — set `SetDeflection`, use `Extrema_ExtFlag_MIN`.
  - Max distance: no direct OCCT API (`Extrema_ExtFlag_MAX` exists in some `Extrema_*` classes but is unreliable for B-rep); implement: closed forms for analytic pairs (point–circle, point–sphere, segment–segment endpoints), else sample both shapes (`GCPnts_UniformAbscissa`, face UV grid) then refine with `math_BFGS`/Newton on the distance function.
  - Length: `GCPnts_AbscissaPoint::Length(BRepAdaptor_Curve)` or `BRepGProp::LinearProperties`.
  - Area: `BRepGProp::SurfaceProperties`.
  - Radius/diameter/center: `BRepAdaptor_Curve::GetType()==GeomAbs_Circle → Circle()`; `BRepAdaptor_Surface` (Cylinder/Cone/Sphere/Torus); ellipse `gp_Elips` major/minor radius → Min/Max Rad points.
  - Angles: `gp_Dir::Angle` between line directions / plane normals; face tangent angle via `BRepLProp_SLProps` normal at the intersection point (`IntCurvesFace_ShapeIntersector`).
  - Parallel test with angular tolerance `Precision::Angular()`.
- **Priority:** P0 (distance/length/area/radius/angle/position); P1 (face tangent angle, curvature deviation, ellipse radial points).

### MT-03 Measure: units, precision, copying values, visualization
- **Purpose:** how values are displayed, copied and drawn.
- **Sources:** View/measure_tool.htm, View/mass_properties_tool.htm [DOCS].
- **Behavior / semantics:**
  - Click a result once → selects the value at full precision (copyable with Ctrl/Cmd+C); click again → toggles to the workspace display precision.
  - Graphics overlay: bold dotted black line = center, min or max distance; bold dotted red/green/blue lines = X/Y/Z components (change in X red, Y green, Z blue); thin dotted line = angle.
  - Star (✱) = value non-zero but within coincident/tangent tolerance (note: still may affect downstream geometry quality).
- **OCCT mapping:** n/a (UI). Tolerance threshold for ✱: use kernel confusion (1e-7 m-scale, design suggestion: `Precision::Confusion()` for length, `Precision::Angular()` for angles).
- **Priority:** P0.

### MT-04 Measure: reference coordinate system
- **Purpose:** report coordinates/deltas relative to a mate connector instead of world origin.
- **Sources:** View/measure_tool.htm [DOCS]; bundle "Mate connector for reference coordinate system" [BUNDLE].
- **Selection inputs:** 1 mate connector (explicit or implicit/inferred).
- **Behavior:** all Point, Center, delta X/Y/Z values are transformed into the mate connector frame; desktop only.
- **OCCT mapping:** `gp_Trsf` from `gp_Ax3` of the mate connector, apply inverse to points/vectors.
- **Priority:** P1.

### MT-05 Measure: create variable from measurement
- **Purpose:** turn any measurement (incl. quick measure) into a driven ("Measured") variable that updates with the model.
- **Sources:** View/measure_tool.htm [DOCS]; FS `variable.fs` enums `VariableMode` (Assigned/Measured/From import), `VariableMeasurementMode` (Distance/Length/Diameter), `AxisWithCustom` (Distance/Along X/Along Y/Along Z/Along custom), `VariableMinMaxSelection` (Minimum/Maximum) [FS].
- **Behavior:** Part Studios only. Click the variable icon right of a result → opens the Variable feature dialog pre-populated (mode Measured, selections, min/max, axis). Variable feature itself is specified in section 3C (VAR-).
- **OCCT mapping:** same computations as MT-02 executed during regeneration.
- **Priority:** P1.

### MT-06 Measure: assemblies (dynamic update), sheet metal flat view
- **Purpose:** context-specific measure behavior.
- **Sources:** View/measure_tool.htm [DOCS].
- **Behavior:**
  - Assemblies: while the Measure dialog is open, values update live while dragging instances: position of a selected circular edge / mate connector relative to origin; distance between origin and a selected entity; distance between an entity on a fixed part and one on a moving part. Not while Section view is on.
  - Sheet metal: with the sheet metal table/flat view open, the dialog has "Part Studio" and "Flat View" tabs measuring the folded model or the flat pattern.
- **OCCT mapping:** re-run MT-02 with per-frame instance transforms (`TopLoc_Location`) — cache `BRepExtrema_DistShapeShape` BVH per part and only update the transform.
- **Priority:** P1.

### MT-07 Measure in drawings
- **Purpose:** measure projected geometry in a drawing.
- **Sources:** View/measure_tool.htm (Drawings) [DOCS]. Shortcut `[`.
- **Selection inputs:** points, lines, arcs, circles in drawing views (projected 2D geometry).
- **Parameters:** unit dropdown (document default marked "(Drawing)"); "Show dual units" checkbox (second value beside first).
- **Behavior:** displays the measurement of projected geometry, axes indicated, and the relation type (e.g., point-to-point). Close with red x. Useful for view-only users.
- **OCCT mapping:** 2D computations on HLR output (`HLRBRep_PolyAlgo`/`HLRBRep_Algo` edges as `Geom2d` curves; `Geom2dAPI_ExtremaCurveCurve`).
- **Priority:** P1.

### MT-08 Mass properties tool — Part tab
- **Purpose:** mass/volume/area/center of mass/inertia of selected parts or instances.
- **Sources:** View/mass_properties_tool.htm [DOCS]; FS `evApproximateMassProperties` (evaluate.fs) [FS]; bundle "Mass and section properties", "Display mass and section properties (__massPropertyShortcutKeys__)" (no default key documented), "Mass moments of inertia (__units__)", "Computing center of mass…", "Mass properties failed to calculate for the current selections.", "Mass calculation skipped items excluded/suppressed from BOM" [BUNDLE]. Bundle action `MASS_PROPERTIES=massProperties` (assignable shortcut).
- **Selection inputs:** "Parts/faces/instances to measure": parts, composite parts, instances, sub-assemblies (surfaces contribute surface area only, inferred), or (by selecting the whole Part Studio/assembly in the Features/Instances list) everything. Optional mate connector (implicit or explicit) as reference for moments of inertia.
- **Outputs (top to bottom):**

  | Output | Definition |
  |---|---|
  | Mass | sum over selected parts that have a material (density) |
  | Volume | sum over all selected parts |
  | Surface area | sum over all selected parts or faces |
  | Center of mass | X, Y, Z (world, or mate connector frame if chosen), parts with material only; marked in graphics with a CoM glyph while dialog open |
  | Mass moments of inertia | 3x3 tensor (Lxx Lxy Lxz / Lyx Lyy Lyz / Lzx Lzy Lzz) about the center of gravity of the selection with world-aligned axes (not the origin), or about the selected mate connector |

- **Behavior / semantics:**
  - Additive: each added part's properties are added. Intersecting parts are NOT unioned: each whole part is computed and summed.
  - Parts without material are omitted from mass/CoM/inertia (a note is shown); if none has material, no mass calculation. FS: density units select dimension — mass = density·volume (3D), density·area (2D), density·length (1D), mass per vertex (0D); "Only entities of the highest dimensionality will be considered" [FS].
  - Assemblies: mass for each instance, selected instances, or whole assembly; items excluded/suppressed from BOM are skipped (message).
  - Not shown by Onshape (per docs and bundle strings): principal moments, principal axes, radius of gyration, products of inertia sign convention. noshape may add them (P2) via `GProp_PrincipalProps`.
  - Values are approximate; see variance option (MT-10).
- **Edge cases & errors:** "Mass properties failed to calculate for the current selections." Mesh solids: mass properties supported for closed meshes [DOCS Document/working_with_imported_cad].
- **OCCT mapping:** `BRepGProp::VolumeProperties(shape, props, Eps, OnlyClosed=true)` (returns error estimate) per part → `GProp_GProps::Add(props, density)` to aggregate with density weighting; `Mass()`, `CentreOfMass()`, `MatrixOfInertia()` (about CoM, world axes); about a mate connector: `MomentOfInertia(gp_Ax1)` or parallel-axis transform of the tensor; principal: `GProp_GProps::PrincipalProperties()` → `GProp_PrincipalProps` (moments, axes, `RadiusOfGyration`). Meshes: divergence-theorem sums over `Poly_Triangulation` (custom).
- **Priority:** P0.

### MT-09 Mass properties tool — Face (section) tab
- **Purpose:** area properties of a face, coplanar face set or section-view cut face.
- **Sources:** View/mass_properties_tool.htm, measure tool video transcript [DOCS]; bundle "Area moment of inertia (__units__)", "Polar moment of inertia (__units__)", "Face selections are not coplanar", "Mate connector Z axis is not normal to face selections" [BUNDLE].
- **Selection inputs:** one face or a set of coplanar faces (incl. faces created by Section view); optional mate connector whose Z axis must be normal to the faces.
- **Outputs:** Surface area, Perimeter, (centroid, inferred), Area moments of inertia (second moments of area, in-plane), Polar moment of inertia.
- **Edge cases:** non-coplanar faces → "Face selections are not coplanar"; mate connector Z not normal → "Mate connector Z axis is not normal to face selections".
- **OCCT mapping:** `BRepGProp::SurfaceProperties` (area, centroid, `MatrixOfInertia` with unit density = 2nd moment of area in 3D) → express in the plane frame: Ixx, Iyy, Ixy in-plane, J = Ixx + Iyy; Perimeter: `BRepGProp::LinearProperties` over all face wires. Section faces from MT-11 geometry (`BRepAlgoAPI_Section` + `BRepBuilderAPI_MakeFace`, or the cap faces of a half-space cut).
- **Priority:** P1.

### MT-10 Mass properties: materials, override, calculation variance, center-of-mass display, copy
- **Purpose:** supporting behaviors of the mass properties tool.
- **Sources:** View/mass_properties_tool.htm [DOCS].
- **Parameters:**

  | Control | Type | Default | Notes |
  |---|---|---|---|
  | Part / Face tabs | tabs | Part | |
  | Mate connector (reference) | selection, 1 | none (CoM, world axes) | |
  | Show calculation variance | checkbox | off | shows value and the difference between lower and upper bound; does not change the computation |
  | Override (Mass) | checkbox + numeric | off | visible when the whole part/assembly is selected in Features/Instances list; user mass replaces computed; dependent values recomputed |

- **Behavior:**
  - Materials assigned via part context menu "Assign material" (material → density; spec in 3C).
  - Center of mass glyph shown while dialog open; persistent via context menu "Show center of mass" on part/composite part/instance; CoM is referenceable in drawings, dimensions and mate connectors.
  - Clicking a value: first click = full precision selected, second = workspace precision (copy with native shortcut).
- **OCCT mapping:** variance = error estimate returned by `BRepGProp::VolumeProperties(..., Eps)` / `SurfaceProperties(..., Eps)`; bound = value ± error.
- **Priority:** P1 (override P2).

### MT-11 Section view
- **Purpose:** clip the model with one or more planes (or cylinder/cone-defined planes) to see inside; persistent until turned off.
- **Sources:** View/section_view.htm, View/graphics_preferences.htm [DOCS]; shortcut Shift+X ("Opens the Section view dialog. Multiple planes can be selected.") [DOCS shortcuts]; bundle "Items to exclude", "Items to include", "Section view…", "Controls the default solid parts interference color in section view.", `SECTION_PLANE_PRE_HIGHLIGHT_COLOR=rgba(255,198,133,0.33)` [BUNDLE].
- **Selection inputs:**
  - Section plane(s): planes, mate connectors, cylindrical faces, conical faces, planar faces (may preselect or pick after opening; each click on a new location adds another section plane with its own manipulator).
  - Items to exclude (Exclude tab): parts/surfaces/instances not to be cut.
  - Items to include (Include tab): restrict sectioning to these items.
- **Parameters:** per plane: depth offset (length) and rotation angle(s) (angle) entered by manipulator drag or graphics-area numeric field; flip direction; "Select default plane" sub-dialog: dropdown Front/Top/Right with "Flip plane" toggle (front/back, top/bottom, right/left).
- **Behavior / semantics:**
  - Entry: Camera and render options > Section view, part/assembly context menu > Section view, or Shift+X. Can be turned on before any selection.
  - Manipulator appears at the last selected location: center ball (open circle) drags/snaps the manipulator origin to inference points (incl. cylinder centroids, shown as white marks); arrow drags depth; angle indicators rotate the plane; click the manipulator to flip which side is kept.
  - Multiple section planes combine (each clips; union of removed half-spaces, inferred).
  - Intersecting (interfering) parts are rendered in the Section interference color (default red; Graphics preferences).
  - Press N / "View normal to" to look normal to the section plane.
  - Click outside the dialog to close it (section stays) and navigate; double-click the section plane, or menu "Edit section view", to reopen; "Turn off section view" from Camera and render options or context menu.
  - Section state can be saved in a Named view (MT-12). Measure works on cut faces/edges; mass properties Face tab accepts section faces. Isolate/Make transparent work in section view. Exposed internal parts are refined for display (MT-23).
  - Cut caps are rendered filled (inferred: in each part's color; Onshape does not document cap color options — only interference color).
  - Cylindrical/conical face selection: section plane through the face's axis (inferred).
- **Edge cases:** dynamic measure updates in assemblies are disabled while sectioned. Repair panel shows sections but not exclusions [DOCS Document/repairing].
- **OCCT mapping:** viewport: GPU clip planes in the GPUI/wgpu renderer (`gl_ClipDistance`/fragment discard) + stencil-buffer capping (front/back face parity) with per-part cap color; interference: parts whose caps overlap → compute with `BRepAlgoAPI_Common` on the section plane faces or 2D boolean of section outlines. Exact cut geometry for measure/mass: `BRepAlgoAPI_Section(shape, gp_Pln)` for outlines; cap faces via `BRepBuilderAPI_MakeFace` on the section wires (`ShapeAnalysis_FreeBounds::ConnectEdgesToWires`), or `BRepAlgoAPI_Cut` with `BRepPrimAPI_MakeHalfSpace`. (OCCT's own `Graphic3d_ClipPlane` capping is the reference implementation if using AIS.)
- **Priority:** P0.

### MT-12 Named views
- **Purpose:** save and restore camera views per workspace.
- **Sources:** View/named_views.htm [DOCS]; Shift+V; bundle error "Name contains invalid characters. New view not saved." [FS errorstringenum/BUNDLE].
- **Parameters:** Name (text) + "+" to add; "Select a view" dropdown; X icon deletes selected view.
- **Behavior:** captures perspective on/off, zoom scale, orientation, and section view state (optional). Unlimited per workspace. Dialog opens next to the view cube dropdown, or at its last repositioned location. If the current camera matches a saved view when opening, that view is preselected. Named views notification uses vivid yellow (#ffc107) [DOCS colors]. Recommended to avoid display refinement churn in large assemblies.
- **Edge cases:** invalid characters in name → not saved.
- **OCCT mapping:** app data model: `{name, eye, target, up, ortho_scale|fov, perspective, section_state}` stored in the workspace (versioned with document).
- **Priority:** P1.

### MT-13 View cube & standard views
- **Purpose:** orientation widget (top-right) and standard orthographic views.
- **Sources:** View/view_navigation_and_the_view_cube.htm, View/camera_and_render_options.htm [DOCS].
- **Behavior / semantics:**
  - Click a cube face → plane view (Top, Bottom, Front, Back, Left, Right). Click a cube corner → trimetric view. Click edges → view along the edge bisector (inferred).
  - Cube arrows: click = rotate 15°; Shift+click = 90°; Ctrl+click = 5°.
  - The view cube dropdown opens "Camera and render options" menu: Isometric/Dimetric/Trimetric, Named views, Previous view, Zoom to fit/window/selection, Perspective, Orient normal to sketch on edit, Shaded/Shaded without edges/Unshaded/Translucent, Hidden edges visible/removed, Tangent edges visible/phantom/removed, View in high quality, Highlight boundary edges, Section view, Graphics preferences.
  - Standard view keys: Shift+1 Front, Shift+2 Back, Shift+3 Left, Shift+4 Right, Shift+5 Top, Shift+6 Bottom, Shift+7 Isometric.
  - World convention (inferred from default planes Top=XY, Front=XZ, Right=YZ, Z up): Front = camera on −Y looking +Y, up +Z; Back = camera on +Y; Right = camera on +X; Left = camera on −X; Top = camera on +Z looking −Z, up +Y; Bottom = camera on −Z.
  - View transitions are animated (inferred).
- **OCCT mapping:** camera math is app-side (no OCCT); a view cube can be drawn by the custom renderer (OCCT's `AIS_ViewCube` is the AIS reference).
- **Priority:** P0.

### MT-14 Isometric, dimetric, trimetric projections
- **Purpose:** standard axonometric orientations.
- **Sources:** View/isometric_dimetric_trimetric_projections.htm [DOCS]; bundle actions `DIMETRIC`, `TRIMETRIC`, `ISOMETRIC_VIEW` [BUNDLE].
- **Behavior:** Isometric: three axes equally inclined, 120° between projected axes. Dimetric: two axes share a scale, third differs. Trimetric: all three different. Exact Onshape angles are not documented. Design suggestion (inferred, tune against screenshots): isometric = camera direction from (1,−1,1) (front, right and top visible, Z up; azimuth 45°, elevation 35.264°); dimetric = azimuth 45°, elevation ≈ 20.7° (two equal foreshortenings); trimetric = azimuth ≈ 30°, elevation ≈ 20° (three different foreshortenings). Shift+7 = isometric; trimetric also from a cube corner click.
- **OCCT mapping:** app camera presets.
- **Priority:** P0 (iso), P2 (dimetric/trimetric).

### MT-15 Perspective view
- **Purpose:** toggle perspective vs orthographic camera.
- **Sources:** View/perspective_view.htm, View/graphics_preferences.htm [DOCS].
- **Parameters:** toggle (default off; can "Set as default" via right-click). Field of view 20–120° (Graphics preferences; only when perspective on; default not documented).
- **Behavior:** vanishing point; camera approaching the model increases distortion. Saved in named views.
- **OCCT mapping:** app camera (perspective projection matrix); ensure picking uses the same projection.
- **Priority:** P1.

### MT-16 Zoom to fit, zoom to window, zoom to selection, zoom in/out
- **Purpose:** frame the model.
- **Sources:** View/zoom_to_fit_window_and_selection.htm [DOCS].
- **Behavior:**
  - Zoom to fit: F or double-click scroll wheel; frames entire Part Studio/Assembly/Drawing (also context menu with nothing selected).
  - Zoom to window: W toggles a mode; click+drag a box to zoom to it.
  - Zoom to selection: menu/context-menu item visible only when something is selected; frames selection.
  - Z = zoom out, Shift+Z = zoom in on the center of the current view; mouse wheel zooms (direction reversible in preferences; zoom about cursor, inferred).
- **OCCT mapping:** bounding boxes `BRepBndLib::Add` / `AddOptimal` on visible shapes (or cached mesh AABBs); compute camera distance/scale to fit.
- **Priority:** P0.

### MT-17 Previous view
- **Purpose:** revert to the previous camera.
- **Sources:** View/previous_view.htm [DOCS]; bundle `PREVIOUS_VIEW` (assignable, no default key) [BUNDLE].
- **Behavior:** restores the camera before the last view change (single-level or stack, inferred: stack).
- **OCCT mapping:** app camera history stack.
- **Priority:** P2.

### MT-18 View normal to / Orient normal to sketch on edit
- **Purpose:** align camera normal to a planar face, plane, sketch or section plane.
- **Sources:** shortcut N [DOCS shortcuts]; View/orient_normal_to_sketch_on_edit.htm; context menu "View normal to" [DOCS context_menus].
- **Behavior:** N with a preselected or hovered plane/face → view normal (face toward viewer); pressing N again flips to the inverse-normal view. "Orient normal to sketch on edit" (menu toggle, check mark, persists across sessions): entering sketch edit orients normal to the sketch plane; disabling returns to previous view.
- **OCCT mapping:** plane normal via `BRepAdaptor_Surface::Plane()` / `BRepLProp_SLProps` at face center; camera up = closest world axis projection (inferred).
- **Priority:** P0.

### MT-19 Render modes: Shaded with edges, Shaded without edges, Unshaded, Translucent
- **Purpose:** global display mode for all parts.
- **Sources:** View/shaded_unshaded__and_translucent.htm, camera_and_render_options.htm [DOCS]; bundle `TRANSLUCENT_PASS_ALPHA=.25` [BUNDLE].
- **Behavior:** Shaded with edges (default); Shaded without edges; Unshaded (wireframe, combined with hidden edges visible/removed); Translucent (all parts semi-transparent; disables Hidden edges visible/removed). "Shaded with hidden edges" also listed (shaded + hidden edges dashed). Right-click a sub-option > "Set as default".
- **OCCT mapping:** renderer passes: shaded triangles from `BRepMesh_IncrementalMesh`; edges from `BRep_Tool::Polygon3D`/`PolygonOnTriangulation`; translucent via order-independent transparency or sorted alpha.
- **Priority:** P0.

### MT-20 Hidden edges visible / removed
- **Purpose:** show/hide edges occluded by geometry.
- **Sources:** View/hidden_edges_visible_and_removed.htm [DOCS].
- **Behavior:** Hidden edges removed (default) / Hidden edges visible, each combinable with Shaded with edges or Unshaded. Hidden edges drawn dashed/lighter (inferred).
- **OCCT mapping:** GPU: second edge pass with depth test inverted (GREATER) and dash pattern. Not HLR (HLRBRep only for drawings).
- **Priority:** P1.

### MT-21 Tangent edges visible / phantom / removed
- **Purpose:** treatment of smooth (tangent-continuous) edges, e.g. fillet boundaries.
- **Sources:** View/tangent_edges_visible_phantom_and_removed.htm, graphics_preferences.htm [DOCS].
- **Behavior:** Visible (solid, default); Phantom (dashed, width 0.1–3 px default 0.5 px and color from Graphics preferences); Removed. Docs: only for newly created models, browser only.
- **OCCT mapping:** classify edge as tangent if `BRep_Tool::Continuity(edge, f1, f2) >= GeomAbs_G1` or normals agree within angular tolerance at sampled points (`BRepLProp_SLProps` on both faces via `BRep_Tool::CurveOnSurface`); store flag per edge in display data.
- **Priority:** P1.

### MT-22 Camera and render option defaults
- **Purpose:** persist preferred view options.
- **Sources:** View/camera_and_render_options.htm [DOCS].
- **Behavior:** Perspective view and Highlight boundary edges default off; enable, then right-click > "Set as default" (and same to revert). Orient normal to sketch on edit persists automatically. Shaded/Hidden/Tangent edge sub-options: right-click a disabled option > Set as default (enables + sets default).
- **OCCT mapping:** user preferences store.
- **Priority:** P2.

### MT-23 View in high quality, tessellation levels and display refinement
- **Purpose:** level-of-detail tessellation management.
- **Sources:** View/view_in_high_quality.htm, Home/graphics_area_display_data.htm, PartStudio/customizing_appearances.htm (tessellation quality) [DOCS]. Shortcut Shift+R.
- **Behavior / semantics:**
  - Levels: Unloaded (bounding box), Coarse, Medium, Fine, Extra fine. Data size ~1x / 10x / 100x. Coarse/Medium/Fine generated by default (complex studios may be limited to Coarse+Medium or Coarse); Extra fine only in View in high quality mode (generated on the fly from viewing conditions).
  - Refinement loop: list parts visible to camera; sort by size and distance; selected parts get Fine regardless of distance; increase levels pass by pass until desired level or memory limit; never downgrade loaded parts unless memory needed; ensure all visible parts at least Coarse before refining any further. Triggered by history changes, camera moves, selections. Parts outside frustum/occluded may be unloaded and drawn as bounding boxes. Indicators bottom-left: "refinement in progress", "paused while camera moves".
  - Per-part tessellation quality (Appearance dialog, Part Studio): Auto (default), Coarse, Medium, Fine, Very fine (never auto-chosen). Assemblies: context menu "Use best available tessellation" / "Use automatic tessellation setting"; icon in Instances list.
  - Transparent (isolated/make transparent) parts are refined.
- **OCCT mapping:** `BRepMesh_IncrementalMesh` with `IMeshTools_Parameters` per level (design suggestion: relative deflection; Coarse 0.5% bbox diag/ 30°, Medium 0.1%/ 20°, Fine 0.02%/ 12°, Extra fine 0.005%/ 6°); cache meshes per (part, level) on disk; LOD selection by projected screen size. Native desktop has more memory than a browser tab, so simplify (design suggestion).
- **Priority:** P1 (LOD), P0 (single good tessellation).

### MT-24 Graphics preferences
- **Purpose:** viewport rendering parameters.
- **Sources:** View/graphics_preferences.htm [DOCS]; bundle `OPEN_GRAPHICS_PREFERENCES` [BUNDLE].
- **Parameters:**

  | Group | Setting | Range / default |
  |---|---|---|
  | Viewport | Ambient occlusion | 0–100 %, default 37.5 % |
  | Viewport | Field of view | 20–120° (perspective only) |
  | Shaded bodies | Line width | 0.1–3 px |
  | Shaded bodies | Shininess | 1–100 % |
  | Shaded bodies | Specular color | color picker |
  | Shaded bodies | Section interference color | color picker (default red) |
  | Phantom edges | Phantom tangent edge width | 0.1–3 px, default 0.5 px |
  | Phantom edges | Phantom tangent edge color | color picker |
  | Mesh bodies | Mesh line width | 0.1–3 px, default 0.3 px |
  | Active sketches | Line width | 0.1–3 px |
  | Active sketches | Constrained line color / Underconstrained color / Underconstrained occluded color | color pickers (defaults black / blue / — ) |
  | Inactive sketches | Line width; Line color | 0.1–3 px; color |
  | Construction lines | First dash / First gap / Second dash / Second gap | 1–50 px def 4 / 0–50 def 6 / 1–50 def 30 / 0–50 def 6 |

  "Reset to default" resets all general and sketch options.
- **OCCT mapping:** renderer uniforms; SSAO pass for ambient occlusion.
- **Priority:** P2 (AO P2, line widths/colors P1).

### MT-25 Highlight boundary edges
- **Purpose:** show laminar (free) edges / holes needing repair, especially after import.
- **Sources:** View/highlight_boundary_edges.htm; Document/working_with_imported_cad.htm [DOCS]; bundle `HIGHLIGHT_BOUNDARY_EDGES` [BUNDLE].
- **Behavior:** toggle (Part Studio); boundary edges in solid red when visible, dashed red when hidden. Default off; can be set as default.
- **OCCT mapping:** edges with exactly one adjacent face: `TopExp::MapShapesAndAncestors(shape, TopAbs_EDGE, TopAbs_FACE, map)` count==1 (excluding seam/degenerate edges via `BRep_Tool::IsClosed`/`Degenerated`); or `ShapeAnalysis_FreeBounds`.
- **Priority:** P1.

### MT-26 Hide / Show
- **Purpose:** per-entity visibility.
- **Sources:** View/hide_show.htm [DOCS]. Shortcuts: Y hide (selected or hovered), Shift+Y show; P planes; Shift+H sketches; K mate connectors; J mates (assembly); Shift+P hide all construction geometry.
- **Behavior:** commands in Parts list / Instances list / graphics-area context menus: Hide/Show; Hide/Show all <type> (all entities of the same type in the tab, e.g. "Hide all instances"); Hide/Show other <type> (all of same type except selection, e.g. "Hide other parts"). Entities: sketches, parts/instances, surfaces, mates, folders, groups, mate connectors. Eye icon in lists. Empty-space context menu: "Show all" (instances, mate connectors, mate features) / "Show all instances" / "Show all parts" (incl. hidden) / "Show all parts, sketches, and planes". Box select works. No in-context needed.
- **OCCT mapping:** app visibility flags (Part Studio visibility is per-workspace UI state; assembly instance visibility is assembly model data, inferred).
- **Priority:** P0.

### MT-27 Isolate
- **Purpose:** keep selection opaque, make everything else transparent.
- **Sources:** View/isolate.htm [DOCS]. Shift+I (cumulative on hovered/selected).
- **Selection inputs:** parts/instances, points, edges, faces, mate connectors, folders, groups.
- **Parameters (Isolate dialog):** Transparency slider ("Show or hide internal geometry"): far left = only outer edges of transparent geometry, internals hidden; moving right reveals internal geometry; further right fades internals and outer edges until fully invisible. "Select transparent geometry" checkbox (allows select/measure/analyze/edit transparent geometry; also from lists). Component controls: "Expand: distance" slider (successively isolate parts by physical proximity) / "Expand: connectivity" slider (assembly only; by mate connectivity).
- **Behavior:** Settings persist across dialog sessions and document sessions. Moving the slider fully left/right auto-unchecks Select transparent geometry; moving it again re-checks it. Exit: dialog x, Esc, or context menu "Exit isolate"; selections made are retained. Works in context and in section view. Inverse of Make transparent.
- **OCCT mapping:** renderer per-part alpha; proximity ordering by `BRepExtrema_DistShapeShape` min distance from selection (or AABB distance for speed); connectivity = BFS on mate graph.
- **Priority:** P1.

### MT-28 Make transparent
- **Purpose:** make selection transparent, rest opaque.
- **Sources:** View/make_transparent.htm [DOCS]. Shift+T.
- **Parameters / behavior:** identical controls to Isolate (Transparency slider, Select transparent geometry, Expand: distance / Expand: connectivity (assembly only) — sliders make more parts transparent). Exit via x / Esc / "Exit make transparent".
- **OCCT mapping:** as MT-27.
- **Priority:** P1.

### MT-29 Reference geometry visibility toggles
- **Purpose:** quick global visibility of non-part entities.
- **Sources:** shortcuts page [DOCS].
- **Behavior:** P = hide/show planes; Shift+H = hide/show sketches (Part Studio); K = show/hide mate connectors; J = show/hide mates (assembly); H = Show mates mode (assembly toolbar); Shift+P = hide all construction geometry. Default planes and origin toggled via eye icon in Feature list; planes resizable by drag handles.
- **OCCT mapping:** renderer filters.
- **Priority:** P1.

### MT-30 Analysis tools menu (common behavior)
- **Purpose:** container for all analysis tools.
- **Sources:** View/analysis_tools.htm [DOCS].
- **Behavior:** "Show analysis tools" icon (bottom-right) opens a menu: Curve/surface analysis, Deviation analysis, Connection analysis, Dihedral analysis, Interference detection, Zebra stripes, Reflection analysis, Curvature color map, Draft analysis, Thickness analysis, Flatten surface. Opening a tool from the menu = non-persistent (empty selections); opening via its shortcut = persistent (previous selections kept). Tools without default keys can be assigned shortcuts (bundle actions: `DEVIATION_ANALYSIS`, `CONNECTION_ANALYSIS`, `SURFACE_FLATTEN`, `INTERFERENCE_DETECTION`, `ZEBRA_STRIPES`, `CURVATURE_COLOR_MAP`, `DRAFT_ANALYSIS`, `REFLECTION_ANALYSIS`, `OPEN_THICKNESS_ANALYSIS`). Only one surface-shading analysis (zebra / color map / draft / reflection / thickness) active at a time (inferred from "Selecting Color map ... also turns off the zebra stripes").
- **OCCT mapping:** app framework.
- **Priority:** P1.

### MT-31 Curve/surface analysis
- **Purpose:** curvature combs, isocurves, control polygons, knots, inflections and minimum radius on curves/faces.
- **Sources:** View/curve_surface_analysis.htm [DOCS]. Shift+C (toggle; in sketch auto-selects all sketch curves). Colors: U combs/border #ff005a, V combs/border #0000ff, edge combs #70a65c, edge comb border #b275de [DOCS colors].
- **Selection inputs:** edges (sketch or part), faces; preselection opens with only that selection; selections persist between invocations; tool stays open after leaving the sketch (selection fields inactive).
- **Parameters:**

  | Option | Type | Default / range | Notes |
  |---|---|---|---|
  | Curvature combs | bool | on (inferred) | combs at evenly spaced isolines (not control points), evaluates up to G3 shape |
  | Show U curves | bool | | U combs red/pink |
  | Show V curves | bool | | V combs blue |
  | U curves / V curves | integer | 2–64, default 8 each | number of isolines |
  | Sketch inflection points | bool | | sketch only; black "bow ties" |
  | Minimum radius | bool | | edges only |
  | Extend across face | bool | | combs continue across adjacent faces |
  | Control point grids | bool | | magenta poles/polygon of underlying B-spline |
  | Knot points | bool | | spline entities only |
  | Details | bool | | on hover: degree, spans, number of control points (spline only) |
  | Comb size | slider | | comb magnitude scale |
  | Show for previewed edges | bool | | when opened from a feature dialog's context menu: combs on the edges of the feature preview |

- **Behavior:** dragging a sketch curve updates combs live. Available from context menu on sketch/part/surface and during feature creation (right-click in graphics area).
- **OCCT mapping:** curve curvature/normal: `GeomLProp_CLProps` / `BRepLProp_CLProps` (comb = point + k·n·scale); surface isolines: `Geom_Surface::UIso/VIso` trimmed by face (`BRepAdaptor_Surface` + `Geom2dHatch_Hatcher` or `BRepTools_WireExplorer` trimming); normal curvature of the isoline on the surface; poles/knots/degree: `Geom_BSplineCurve/Surface::Poles, Knots, Degree, NbPoles` (via `GeomConvert` not needed if native spline); inflection points & min radius for 2D sketch curves: `Geom2dLProp_CurAndInf2d` (direct: inflections + curvature extrema); 3D min radius: sample + `math_BrentMinimum` on 1/k.
- **Priority:** P1.

### MT-32 Deviation analysis (curve and surface)
- **Purpose:** min/max distance between two curve chains or two surfaces.
- **Sources:** View/deviation_analysis.htm [DOCS]; bundle `DEVIATION_ANALYSIS` [BUNDLE].
- **Selection inputs:** Curve mode: Chain 1 (one or more adjacent continuous curves), Chain 2. Surface mode: Source (1 surface), To measure (1 surface). Selecting exactly 2 curves before opening auto-fills Chain 1/Chain 2 and computes.
- **Parameters:** mode Curve/Surface; Scale slider (moves Min/Max labels inward/outward); Surface only: Absolute values (bool), Colors (Viridis default, Blue→Red, Plasma, Rainbow), editable lower/upper legend bounds (click to type or drag; asterisk marks edited value; Reset icon), Dim color scale (only out-of-range regions colored: upper brown, lower pink); legend draggable/dockable to any side.
- **Behavior / algorithm (curves):** if chains share coincident endpoints, deviation over full lengths; otherwise pair endpoints by lower minimum distance; for each endpoint compute min distance to the other chain; trim the computation range at the intersection point of the lowest minimum distance; compute min (blue) and max (red) deviation over the trimmed range; show extrema points (one shown if ties). Surfaces: single-sided (signed) deviation from Source to To measure; color scale dark blue (least) to yellow (most); min/max may be negative unless Absolute values.
- **Edge cases:** chains must be continuous; surface mode: exactly 2 surfaces, no chains.
- **OCCT mapping:** sample chain 1 (`GCPnts_QuasiUniformDeflection`) and project onto chain 2 (`GeomAPI_ProjectPointOnCurve` / `BRepExtrema_DistShapeShape` vertex-to-wire) then refine extrema with `math_BrentMinimum`; surfaces: per-mesh-vertex `GeomAPI_ProjectPointOnSurf` / `BRepExtrema_DistShapeShape` with sign = dot(Δ, source normal); upload scalars to color-map shader.
- **Priority:** P2.

### MT-33 Connection analysis
- **Purpose:** report G0/G1/G2/G3 continuity between curves and/or faces.
- **Sources:** View/connection_analysis.htm; colors: no connection #932229, G0 #fcda7f, G1 #028502, G2 #440154 [DOCS colors]; bundle `CONNECTION_ANALYSIS` [BUNDLE].
- **Selection inputs:** two or more curves, faces, or mixed (examples show 2 faces, multiple faces, multiple curves, curves+faces). Endpoints with multiple connections are not analyzed.
- **Behavior:** results populate automatically; surface connections color-code the shared edge, curve connections color-code the connection point, by highest continuity met (G0 position, G1 tangent, G2 curvature, G3 curvature rate). Tolerances not documented.
- **OCCT mapping:** curves: compare `GeomLProp_CLProps` D0–D3 at shared endpoint (position distance, tangent angle, curvature difference, curvature derivative); faces: sample shared edge (`BRep_Tool::CurveOnSurface` on both faces) and compare positions (G0), normals (G1), normal curvatures in edge-normal direction (`BRepLProp_SLProps`, G2); also `BRep_Tool::Continuity`/`BRepLib::EncodeRegularity` as a quick test. Design suggestion tolerances: G0 ≤ 1e-5 m (Onshape min length bound), G1 ≤ 0.5°, G2 relative curvature diff ≤ 5%.
- **Priority:** P2.

### MT-34 Dihedral analysis
- **Purpose:** display dihedral angle combs along edges.
- **Sources:** View/dihedral_analysis.htm [DOCS]. Shift+D (toggle; selections persist; preselection replaces previous selection). Colors: low threshold #440154, mid #21918c, high #fde725 [DOCS colors].
- **Selection inputs:** edge(s).
- **Parameters:** unit (degree/radian); results Max and Min dihedral angle; "Enable threshold display" (bool) → high/low threshold fields; Scale slider (comb length).
- **OCCT mapping:** sample edge parameters; at each sample get normals of the two adjacent faces (`BRepLProp_SLProps` using `BRep_Tool::CurveOnSurface` → UV) → angle = acos(n1·n2) (signed by convexity via edge tangent cross product); comb = vector proportional to angle.
- **Priority:** P2.

### MT-35 Interference detection
- **Purpose:** find overlapping volumes between parts/instances.
- **Sources:** View/interference_detection.htm [DOCS]; color Moderate red #bd3039 [DOCS colors]; bundle `INTERFERENCE_DETECTION` [BUNDLE].
- **Selection inputs:** two or more parts/instances (box select allowed: left-to-right fully enclosed, right-to-left touching).
- **Parameters:** Include standard content (bool, optional, assembly); Show top level only (bool; hides interferences internal to selected subassemblies).
- **Behavior:** interfering mass shown in red; Interferences list shows part pairs; hover a name = cross-highlight; focus on Interferences field = zoom to fit selected interference; hover = bounding box drawn with length/width/height shown in dialog.
- **OCCT mapping:** broad phase AABB/OBB (`Bnd_OBB`, `BVH`), narrow phase `BRepAlgoAPI_Common` per candidate pair → volume via `BRepGProp::VolumeProperties` > tol; fast mesh-based alternative `BRepExtrema_ShapeProximity` (triangle-set overlap) for preview; bounding box of the common solid via `BRepBndLib::AddOBB` / `Add`.
- **Priority:** P1.

### MT-36 Zebra stripes
- **Purpose:** reflection-line continuity check.
- **Sources:** View/zebra_stripes.htm [DOCS]; bundle `ZEBRA_STRIPES` [BUNDLE].
- **Parameters:** Stripe count (integer, default 35 per the example images); Flip stripes (bool); Show edges (bool, default on).
- **Behavior:** striped-room reflection on all faces (Part Studio/Assembly/Sketch). Stripes aligned across edge (break direction but touch) = G1; continuous without kink = G2. Accepting (check) keeps stripes; turn off by reopening and clicking X; opening Curvature color map also turns zebra off.
- **OCCT mapping:** fragment shader: stripe = step(fract(k · f(reflect(view, n)))) using a fixed world/camera-locked stripe direction; requires smooth per-vertex normals (exact normals from `BRepLProp_SLProps`, fine tessellation).
- **Priority:** P2.

### MT-37 Reflection analysis
- **Purpose:** render model with reflective material and environment map.
- **Sources:** View/reflection_analysis.htm [DOCS].
- **Parameters:** Environment image (Select image: from current document, other documents, Libraries; built-in Onshape library environment maps: Blue sky, Ceiling hexagonal, Ceiling square, Chromatic, Daylight road, Horizontal stripe, Light tunnel, Metro station, Nightfall, Pedestrian bridge, Wall stripes); Rotation angle (+ Opposite direction); Show edges; Blur environment; Lock environment (lock reflection relative to world/camera).
- **OCCT mapping:** shader env-map lookup (equirectangular) with optional mip blur.
- **Priority:** P2.

### MT-38 Curvature color map
- **Purpose:** color faces by curvature.
- **Sources:** View/curvature_color_map.htm [DOCS]; bundle "Loading gaussian/mean/max radius/min radius curvature data" [BUNDLE].
- **Parameters:** Colors: Viridis (deep purple→light green), Blue -> Red, Plasma (deep blue→light yellow), Rainbow. Curvature graph: Gaussian (product of principal curvatures; shown as inverse radius), Mean, Max radius, Min radius. Scale endpoints draggable or typed (refresh icon to recalc). Legend draggable anywhere (reorients vertical near sides); dropdowns stay at top.
- **Behavior:** applies to faces/surfaces in the graphics area (Sketch, Part Studio). Flat faces get no color (model keeps its color).
- **OCCT mapping:** per mesh vertex `GeomLProp_SLProps(surface, u, v, 2, tol)` → `MaxCurvature`, `MinCurvature`, `MeanCurvature`, `GaussianCurvature`; radius = 1/|k|; upload as vertex attribute, shader maps through 256-entry LUT texture with user min/max.
- **Priority:** P2.

### MT-39 Draft analysis
- **Purpose:** check moldability (draft angle, undercuts).
- **Sources:** View/draft_analysis.htm [DOCS]; bundle "Minimum draft angle" [BUNDLE].
- **Selection inputs:** Mold split direction (plane, face or edge; manipulator arrow points to side one; flip arrow); entities to check (parts/faces).
- **Parameters:** Minimum draft angle (angle; default not documented — design suggestion 1°); Show undercut regions (bool, default on).
- **Behavior:** legend at top. Light blue = meets draft on side one (positive), dark blue = side two (negative), yellow = too steep (|draft| < minimum), red = undercut. Hovering shows exact angle at cursor (signed). Stays active while editing; works with section view.
- **OCCT mapping:** per-fragment/vertex draft = 90° − angle(n, pull) using exact normals; undercut = face region whose ray along ±pull direction hits the same part again (BVH ray cast on mesh, or `IntCurvesFace_ShapeIntersector`) — precompute per vertex.
- **Priority:** P2.

### MT-40 Thickness analysis
- **Purpose:** wall thickness map of parts.
- **Sources:** View/thickness_analysis.htm [DOCS]; bundle `OPEN_THICKNESS_ANALYSIS` [BUNDLE].
- **Selection inputs:** part(s) (native parts and meshes; closed composites unsupported).
- **Parameters:** Method: Rolling ball, Rolling ball gradient (%), Ray, Ray gradient (%); Colors palette; editable lower/upper bound (type value; refresh resets to field min/max; drag tick marks); Dim colors scale (3-color view: below/in/above range); legend draggable.
- **Behavior:** computed asynchronously (stages: Preparing → Processing → Calculating → Refining → Postprocessing; progress spinner bottom-left); initial results then refined; auto bounds until user edits them. Stays active while editing (auto-updates). Menu items "Edit thickness analysis", "Turn thickness analysis off".
  - Rolling ball: diameter of largest inscribed sphere tangent at the point and ≥1 other point; continuous.
  - Ray: distance along inward normal to first hit; discontinuous near sharp corners.
  - Gradient: max rate of thickness change per geodesic distance, dimensionless, shown as %.
- **OCCT mapping:** no OCCT equivalent. Ray: BVH ray cast on fine mesh (`BVH_Tree` / `BRepIntCurveSurface_Inter` for exact). Rolling ball: shrinking-ball medial-axis algorithm on mesh (iterate: sphere along −n, nearest-point query via BVH `BRepExtrema_TriangleSet`/custom kd-tree, shrink until empty). Gradient: finite differences over mesh neighbors / edge length. Run on background threads.
- **Priority:** P2.

### MT-41 Flatten surfaces
- **Purpose:** minimum-strain-energy planar development of contiguous non-planar faces (decals, masks, plies).
- **Sources:** View/flatten_surfaces.htm [DOCS]; bundle "Faces to flatten", "Flattened angle", `SURFACE_FLATTEN` [BUNDLE].
- **Selection inputs:** Faces to flatten (contiguous faces, incl. mesh imports); Rip edges (edges to cut along); Flattened position (mate connector, 1); Origin (vertex, 1).
- **Parameters:** Show facets; Show edges (default on); Show checkerboard + Checkerboard scale (lower = bigger blocks); Show distortion + Distortion scale (higher = more sensitive; blue = tension/stretch, red = compression; doc also says magenta); Show flattened; Flattened offset (length along MC Z, ±); Flattened angle (about MC Z, ±); Show export controls → Export format PARASOLID / STL / DXF / SVG, Export inner edges (bool), Export button; Flatten button.
- **Behavior:** results appear only after pressing Flatten; adding rip edges or origin requires pressing Flatten again. No part is created (export and re-import via Derived/Import if needed). Ignores material; approximate (not for critical measurement). Default position below the selected surfaces.
- **OCCT mapping:** none in OCCT. Mesh faces with `BRepMesh_IncrementalMesh`, cut along rip edges (duplicate vertices), parameterize with LSCM/ARAP (custom Rust implementation or port of libigl algorithms), distortion = per-triangle singular values of the Jacobian; output boundary as polylines/B-spline (`GeomAPI_PointsToBSpline`) → planar face (`BRepBuilderAPI_MakeFace`) for STL/BREP export; DXF/SVG via IO-29/IO-30 writers.
- **Priority:** P2.

### MT-42 Create selection and Select > (smart selection)
- **Purpose:** select related geometry by rule and feed it to a feature dialog.
- **Sources:** Home/create_selection.htm [DOCS]; bundle `SMART_SELECTION_HIGHLIGHT_COLOR`, `ACCUMULATED_SMART_SELECTION_HIGHLIGHT_COLOR` [BUNDLE].
- **Entry points:** context menu "Select >" submenu (immediate select, no dialog), context menu "Create selection", or the Create selection arrow icon in a feature dialog selection field.
- **Face criteria:** Protrusion (faces connected by convex edges), Pocket (connected by concave edges), Hole (faces of the same round hole), Fillets (constant-radius fillet faces), Tangent connected (connected by tangent edges), Bounded faces (seed face + boundary edges/faces; all faces between seed and boundary; if boundary not closed, all adjacent faces/edges selected). Option "Select patterns" (also select faces on the part matching the same criteria/pattern).
- **Edge criteria:** Tangent connected (edges tangent-chained), Loop/chain connected (face → its edge loop; face + edge → loop along that face; sketch edges → adjacent chain; multiple loops kept/unselected individually), Equal length/radius, Parallel; "Select pattern" checkbox (all edges of a pattern instance set).
- **Behavior:** seed selection highlighted orange, automatic selections yellow; "Add selection" transfers to the open feature dialog field, dialog clears for another selection; selection made before opening a feature pre-populates it.
- **OCCT mapping:** adjacency graph from `TopExp::MapShapesAndAncestors`; convexity per edge via normals and edge tangent (`BRepOffset_Analyse` gives concave/convex/tangent classification directly — `ChFiDS_TypeOfConcavity`); hole: cylindrical/conical faces sharing axis (`BRepAdaptor_Surface`); fillets: `ShapeAnalysis` + Onshape-like recognizer (cylinder/torus faces with tangent neighbours, or `TopOpeBRep`-free heuristic; OCCT `BRepAlgoAPI_Defeaturing` does not recognize); equal length via `GCPnts_AbscissaPoint`; parallel via line directions; pattern instances via feature history (app data).
- **Priority:** P1 (loop/tangent P0 for fillet workflows).

### MT-43 Context menus (graphics area)
- **Purpose:** right-click command menus.
- **Sources:** Home/context_menus.htm [DOCS].
- **Behavior:** RMB on entity or blank space; menu sections vary by selection/context (feature edit open, entity selected, list selection). Empty-space Part Studio/Assembly items include: Show all, Show all parts, Create selection, Zoom to fit, Isometric (plus Clear selection, Section view, etc.). With a plane selected: creation commands with icons (New sketch, Offset plane). Sketch in Feature list: Rename, Edit, Copy sketch / Paste sketch (to a plane in another Part Studio; cross-version may error), Show dimensions, Add selection to folder, Show/Hide, Create Drawing of Sketch, Export as DXF/DWG, Suppress, Add comment, Zoom to selection, View normal to, Clear selection, Roll to here, Delete. Other entries referenced elsewhere: Select other, Select >, Isolate, Make transparent, Hide/Show (all/other), Edit appearance, Assign material, Show center of mass, Export, Curve/surface analysis, Exit <tool>.
- **OCCT mapping:** GPUI context menu; RMB press-release without drag = menu, drag = rotate (inferred, KB).
- **Priority:** P0.

### MT-44 Appearance (color and opacity per part, face, feature, sketch, curve)
- **Purpose:** display color/transparency stored in the Part Studio (summary; full spec in PSM- section 3C).
- **Sources:** PartStudio/customizing_appearances.htm, Home/colors.htm [DOCS].
- **Behavior:**
  - Default part colors: rotating palette of 8 colors (repeats on 9th part; deleting a part keeps others' colors).
  - Part: context menu "Edit appearance" (multi-select allowed): color picker (Palette swatches; Mixer with spectrum + HSB square; hex #; R/G/B 0–255; Custom colors list with + / right-click Delete / Update color), Part transparency/opacity slider 0–1 (shows in assemblies too), Tessellation quality (Auto/Coarse/Medium/Fine/Very fine; see MT-23).
  - Faces: "Add appearance" to selected faces (Face appearance dialog with Faces selection list); Features: "Add appearance to feature" / "Edit feature appearance" (colors faces created by the feature); Sketch: "Edit sketch appearance"; Curves: "Edit appearance" in Curve list.
  - Appearance panel (right side) lists all altered appearances; double-click to edit; "Add face appearance" button. Appearances are patterned with parts and configurable. Import option "Import appearances" controls imported face colors (IO-03/IO-04).
  - Selecting beneath a transparent part: hold Alt while clicking.
- **OCCT mapping:** stored as feature/part attributes in the app model; on export written via `XCAFDoc_ColorTool` (surface color per shape/face) and `XCAFDoc_VisMaterialTool` (alpha).
- **Priority:** P0 (part color), P1 (face/feature/sketch colors, transparency).

### MT-45 UI color conventions
- **Purpose:** reference palette for highlight/state colors (light theme).
- **Sources:** Home/colors.htm [DOCS]; bundle-appearance-constants [BUNDLE].
- **Values:**

  | Context | Color |
  |---|---|
  | Selected part/surface/edge/vertex/sketch entity | vivid yellow #ffc107 (bundle highlight rgba(255,169,10,0.8) border rgba(255,169,10,1); part highlight rgba(255,192,76,1)) |
  | Pre-highlight (hover) | border rgba(255,198,133,1); sketch edge hover #f3bf8e |
  | Error (feature/part/entity) / Interference | moderate red #bd3039 |
  | Box select left-to-right (window) | outline #789ad1, fill #d0dcef (solid outline) |
  | Box select right-to-left (crossing) | outline #ffdc74, fill #fff3cf (dotted outline) |
  | Sketch underconstrained / fully constrained / overdefined / inactive | #0000ff / #000000 / #bd3039 / #949ba0 (docs page); bundle CSS: #0000d0 / #000000 / #cd0000 / #999999 |
  | Closed sketch region fill | #E3E6E9 |
  | Sketch dimension hover / selected | #d9bfa0 / #d9a566 |
  | Sketch plane line and text | #90cef1 |
  | Ruled lines / isocurves preview | #d403f9 |
  | Mate connector axes X/Y/Z | #cd0000 / #009500 / #0000d0 |
  | Mate indicator good / problem / suppressed | #90cff1 / #bd3039 / #999999 |
  | Alerts: info / warning / error / success | outline #b2ddf6 / #fcda7f / #dd979b / #a2db7d |
  | List field accepting entries / error | #dbf1fa / #f8eaeb |
  | Edit configurations, named views, exploded views notification | #ffc107 |
- **Priority:** P1.

### MT-46 Print / Download image
- **Purpose:** print or save an image of a Part Studio, Assembly or Drawing view.
- **Sources:** Home/printing_part_studios_and_assemblies.htm [DOCS].
- **Behavior:** Document menu > Print: dotted page border preview in the graphics area; user positions model with normal navigation; options: paper size, Portrait/Landscape; "Print" → OS print dialog (printer, pages, copies, layout, color/BW, margins, pages per sheet, scale, duplex); "Download image" saves an image.
- **OCCT mapping:** offscreen render to texture at page resolution → PNG (`image` crate) or OS print API; vector PDF optional via HLR (P2).
- **Priority:** P2.

## 7. Import / export

Scope: everything that moves data in/out of a document: CAD import (translation into Part Studios/Assemblies with an
Import feature), mesh import, drawing (DWG/DXF) import, non-CAD file tabs, export of parts/Part Studios/assemblies/
sketches/faces/flat patterns/drawings, raw download, update of imported files, and repair after re-import.
Per-format items (IO-16 onward) give direction, Onshape options and OCCT feasibility.

Feasibility legend used below:
- **Native** = open-source OCCT (LGPL-2.1 w/ exception) covers it: `STEPControl`/`STEPCAFControl`, `IGESControl`/`IGESCAFControl`, `StlAPI`/`RWStl`, `RWObj`, `RWGltf`, `RWPly`, `BRepTools`/`BinTools`, `VrmlAPI`, XCAF (`XCAFDoc_*`) for names/colors/assembly structure.
- **Open lib** = needs a permissive or copyleft third-party library (license noted).
- **Commercial** = only via paid SDKs: Open Cascade SAS commercial data-exchange components (ACIS, Parasolid, JT, DXF, Rhino, native CAD readers), or third-party SDKs (CAD Exchanger SDK, Datakit, HOOPS Exchange, Spatial 3D InterOp, ODA Drawings SDK, Siemens Parasolid/JT Open licensing).
- **Infeasible (open)** = proprietary format with no legal open implementation; recommend users export STEP (or Parasolid via the source system) instead.

Design suggestion: noshape should ship STEP (AP242/AP214 with colors+names+assemblies), IGES, STL, OBJ, glTF/GLB, 3MF, DXF/SVG/PDF as P0/P1 using OCCT + open libs, use OCCT BREP as its lossless native exchange format, and treat Parasolid/ACIS/JT/native CAD as optional commercial plug-ins.

### IO-01 Import/export model overview
- **Purpose:** define the data model for imported/exported data.
- **Sources:** File/importing_and_exporting_files.htm, Document/importing_files.htm, File/downloading_files.htm, File/exporting_files.htm [DOCS].
- **Behavior / semantics:**
  - **Import:** any file can be uploaded; it always becomes a file tab (named with the original file name, type shown in tab icon) — CAD files are placed in a "CAD Imports" folder of tabs and additionally translated into Part Studio tab(s) (and Assembly tab(s) for assemblies) containing an **Import feature** (`importForeign`) that references the file tab's data. Imported CAD has no feature history.
  - **Download:** copies a file tab out in its original format (any tab representable as a file).
  - **Export:** writes a part / Part Studio / assembly / sketch / planar face / flat pattern / drawing to another format; exported data never contains features/history.
  - Upload limit 4 GB per file. Sources: local disk, drag-and-drop onto Documents list, integrated cloud drives (Dropbox, Google Drive, OneDrive) — cloud is out of scope for noshape (local filesystem only, design suggestion).
  - Progress, warnings and errors reported in Notifications (import queue with retry link).
- **OCCT mapping:** file tab = blob stored in document; translation produces an XCAF document (`TDocStd_Document` + `XCAFDoc_ShapeTool`) → app Part Studio bodies + Assembly instances.
- **Priority:** P0.

### IO-02 Format support matrix (authoritative list from Supported File Formats)
- **Sources:** File/supported_file_formats.htm; File/exporting_files.htm (dialog format list adds GLB) [DOCS].

  | Format | Extensions | Import versions | Import (part / assembly) | Export versions | Export from | noshape feasibility |
  |---|---|---|---|---|---|---|
  | Parasolid B-rep | .x_t, .x_b | v10.0–v38.1 (preferred import format) | part + assembly | v25.0–v37.0 | parts, Part Studios, assemblies | Commercial |
  | Parasolid mesh | zip(.x_t+.xmm_txt), zip(.x_b+.xmm_bin), .x_b | v28.0–v37.0 (view/reference only) | part | v28.0–v37.0 (zip x_t+xmm_txt or single .x_b) | parts, Part Studios | Commercial |
  | Parasolid mixed model (mesh+B-rep) | same containers | v32.0+ | part | (unversioned) | parts, Part Studios, assemblies | Commercial |
  | ACIS | .sat | up to 2023 1.0 | part + assembly | Version 5 | parts, Part Studios, assemblies | Commercial |
  | STEP | .stp, .step | AP203, AP214, AP242 (geometry and face color only) | part + assembly | AP203, AP214, AP242 (parts: geometry, MBD data, face color; assemblies: geometry and face color) | parts, Part Studios, assemblies | Native |
  | IGES | .igs, .iges | up to 5.3 | part (not listed for assemblies) | 5.3 | parts, Part Studios, assemblies | Native |
  | CATIA V5 | .CATPart, .CATProduct (zip*) | R7–R33 (V5-6R2024), V5-6R2026 | part + assembly* | — | — | Commercial / Infeasible (open) |
  | SOLIDWORKS | .sldprt, .sldasm (Pack and Go zip*) | 1999–2026 | part + assembly* | — | — | Commercial / Infeasible (open) |
  | Inventor | .ipt, .iam (zip*) | 9–2026 | part + assembly* | — | — | Commercial / Infeasible (open) |
  | Pro/ENGINEER, Creo | .prt, .asm (zip*) | Pro/E 2000i – Creo Parametric 12.0 | part + assembly* | — | — | Commercial / Infeasible (open) |
  | NX (Unigraphics) | .prt (zip*) | UG 15.0 – NX 12.0.2.9; NX 1847…1996, 2007 series, 2206, 2212, 2306 (≤2306.7000), 2312 (≤2312.7000), 2406 (≤2406.7000), 2412 (≤2412.7000), 2506 (≤2506.4000), 2512 (≤2512.4000) | part + assembly* | — | — | Commercial / Infeasible (open) |
  | Solid Edge | .par, .psm, .asm (zip*) | 10–2026 (.psm imports as plain geometry, not sheet metal) | part + assembly* | — | — | Commercial / Infeasible (open) |
  | JT | .jt | up to 11.0 | part + assembly | up to 11.0 | parts, Part Studios, assemblies | Commercial (JT Open / OCC JT component) |
  | Rhino | .3dm | v2–v8 | part + assembly | v2–v8 | parts, Part Studios (not listed for assemblies) | Open lib (openNURBS, permissive) |
  | STL | .stl | (mesh; view/reference only) | part | — | parts, Part Studios, assemblies | Native |
  | OBJ | .obj | (mesh; view/reference only) | part | (zip .obj+.mtl, meters) | parts, Part Studios, assemblies | Native |
  | glTF | .gltf (also .glb on export) | 2.0 | part + assembly | glTF/GLB | parts, Part Studios, assemblies | Native |
  | 3MF | .3mf | — | part + assembly | — | parts, Part Studios, assemblies | Open lib (lib3mf, BSD-2) |
  | PVZ (Creo View) | .pvz | — | part (zip*) | 8.0 | parts, Part Studios, assemblies | Infeasible (open) |
  | URDF | .urdf (+ meshes) | — | — | — | assemblies only | Native-ish (custom XML writer + mesh export) |
  | AutoCAD DWG | .dwg | up to 2018 | drawing | R11–14, 2000, 2004, 2007, 2010, 2013, 2018 | drawings; sketches, planar faces, flat patterns | Open lib (LibreDWG GPL-3 / libdxfrw GPL-2) or Commercial (ODA) |
  | DXF | .dxf | up to 2013, 2018 | drawing (also sketch Insert DXF/DWG) | R11–14, 2000, 2004, 2007, 2010, 2013, 2018 | drawings; sketches, planar faces, flat patterns, flattened surfaces | Custom writer (easy) / open lib reader |
  | DWT (AutoCAD template) | .dwt | 2013, 2018 | drawing | 2013, 2018 | drawings | as DWG |
  | PDF | .pdf | non-CAD tab (viewable) | — | — | drawings | Custom (Rust PDF writer) |
  | SVG | .svg | non-CAD tab | — | — | drawings; flattened surfaces | Custom |
  | PNG / JPEG | .png, .jpg/.jpeg | non-CAD tab | — | — | drawings; print/download image | Native renderer + image crate |
  | CSV | .csv | non-CAD tab | — | — | drawing inspection items / drawing definition list | Custom |
  | Non-CAD viewable | .pdf .mp4 .png .jpg/.jpeg .svg .gif .txt .md .csv .json | stored + viewable | — | download | — | app file tabs |
  | Non-CAD stored only | .py .java .js, .mov (browser-dependent), .doc/.docx, any other | stored, not viewable | — | download | — | app file tabs |

  `*` = assembly (and some parts) must be uploaded as a zip containing all parts and the assembly; zip name must equal the top-level assembly name (IO-05).
- **Discrepancies:** the Supported formats page does not list GLB, but the export dialog does (GLTF and GLB). Rhino export listed for Parts/Part Studios only, while the generic export format list includes RHINO. STL listed in assembly export but not import-as-assembly.
- **Priority:** P0 (format registry and dispatch; per-format priorities in IO-16..IO-33).

### IO-03 Import dialog: processing options
- **Purpose:** options presented when a recognized CAD file (by extension) is imported.
- **Sources:** Document/importing_files.htm [DOCS]; bundle "Import to a single document", "Split into multiple documents" [BUNDLE].
- **Parameters:**

  | Option | Type | Default | Notes |
  |---|---|---|---|
  | Import to a single document | radio | default (inferred) | Part Studios and Assemblies created as needed, named from the file; recommended for small/medium assemblies (a few hundred parts) |
  | Split into multiple documents | radio | | document + subdocuments; if an assembly level has >100 unique parts, extra documents hold their Part Studios; checkbox "one document per part or Assembly" (placed in a folder named after the file; links preserved; top-level assembly has Download only, linked part documents have Update and Download); parts-only files: each part own document + generated top-level assembly; assembly without subassemblies → single document; with subassemblies → one document each. For large multi-level assemblies |
  | Combine to a single Part Studio (mobile: "Import file to Part Studios only (flatten)") | radio/toggle | off | assembly flattened to parts in one Part Studio; multiply-instanced parts duplicated |
  | Import appearances | bool | on | face appearances imported; changeable later in Import feature |
  | Import material density | bool | off | density → custom material (SolidWorks, Solid Edge, CATIA V5, Pro/E, Creo, NX) |
  | Orient imported models with Y Axis Up | bool | off | for Y-up source systems; not supported for Autodesk .ipt and CATIA .catpart; changeable later ("Source is 'Y Axis Up'") |
  | Create a composite part when importing multiple or non-solid bodies | bool | off | all parts in the imported Part Studio become one closed composite part; not available with Combine to a single Part Studio; changeable later ("Create composite") |
  | Join adjacent surfaces | bool | off (inferred) | attempts to sew adjacent surfaces meeting at an edge into parts; B-rep imports only |
  | IGES postprocessing | bool | off | IGES only; repairs linear edge translation issues; ~50% longer processing |
  | Units | enum | (file/meter) | STL and OBJ (mesh) imports: centimeter, foot, inch, meter, millimeter, yard; also changeable in the file tab units dropdown |
  | Allow import of parts with faults | bool | (always allowed on desktop per docs) | faulty parts imported with red names + notification; mobile exposes toggle |
  | Owner (company plans) | choice | | out of scope |
- **Behavior:** parts failing validation are still imported "with faults" (red in Feature list and Parts list; notification "Imported parts contained faults" `IMPORT_BODY_FAILED_CHECK`). Shattered bodies detected as fractured: all surfaces/curves of the same body are automatically turned into one closed composite part. Point bodies are filtered out by the Import feature [FS].
- **OCCT mapping:** Y-up: `gp_Trsf::SetRotation(gp::OX(), +π/2)` (maps +Y to +Z) applied via `BRepBuilderAPI_Transform` / XCAF location; Join adjacent surfaces: `BRepBuilderAPI_Sewing` (tolerance ~1e-6..1e-4 m) → `ShapeFix_Shell` → `BRepBuilderAPI_MakeSolid` + `ShapeFix_Solid` if closed; IGES postprocessing: `ShapeFix_Shape`, `ShapeFix_Wireframe::FixSmallEdges/FixWireGaps`, `ShapeUpgrade_UnifySameDomain` for split linear edges; fault check: `BRepCheck_Analyzer` (store per-body fault flag, keep body); material density: XCAF `XCAFDoc_MaterialTool` (STEP) or vendor reader metadata.
- **Priority:** P0 (single document, appearances, Y-up, faults, sewing), P1 (composite, units), P2 (split into multiple documents).

### IO-04 Import feature (`importForeign`)
- **Purpose:** the Part Studio feature that instantiates imported geometry and holds post-import options.
- **Sources:** FS `importForeign` (importForeign.fs) [FS]; Document/working_with_imported_cad.htm [DOCS].
- **Selection inputs:** none (references a file tab blob).
- **Parameters:**

  | FS id | UI label | Type | Default / values | Visible when / notes |
  |---|---|---|---|---|
  | `dependsOnBlob` | Depends on blob | boolean | false | hidden |
  | `blobData` | Blob Data | CADImportData | — | hidden; if dependsOnBlob |
  | `foreignId` | Foreign Id | ForeignId (string, dataId of imported tab) | — | hidden; if !dependsOnBlob |
  | `importAppearances` | Import appearances | boolean | true | → operation param `importFaceColors` |
  | `specifyMaterialData` | — | boolean | false | hidden |
  | `importMaterialDensity` | Import material density | boolean | false | if specifyMaterialData |
  | `yAxisIsUp` | Source is 'Y Axis Up' | boolean | false | |
  | `allowFaultyParts` | Allow faulty parts | boolean | false | |
  | `specifyUnits` | — | boolean | false | hidden (true for mesh imports, inferred) |
  | `unit` | Unit | LengthUnitNames: Centimeter, Foot, Inch, Millimeter, Meter, Yard | Meter | if specifyUnits |
  | `originalUnit` | Original unit | LengthUnitNames | Meter | hidden; if specifyUnits |
  | `flatten` | Flatten assembly | boolean | false | hidden |
  | `maxAssembliesToCreate` | Maximum number of assemblies created | integer, POSITIVE_COUNT_BOUNDS (1..1e5) | 10 | hidden |
  | `isInContext` | IsInContext | boolean | false | hidden |
  | `createComposite` | Create composite | boolean | false | |
  | `compositeName` | Composite name | string | — | hidden; if createComposite |
  | `fixSingleCompositeEmptyName` | — | boolean | false | hidden |

- **Behavior / semantics:**
  - Scale = unit/originalUnit (mesh scaling); non-mesh data cannot be scaled (`IMPORT_SCALING_NON_MESH_DATA` "Scaling a non-mesh import is not allowed.").
  - `isModifiable = !isInContext`; in-context imports are skipped inside feature patterns.
  - After the import operation, point bodies created by the import are deleted; result transformed by remaining pattern transform (supports being patterned).
  - Context menu on the Import feature: Edit, Update (choose a new file; see IO-06).
- **Edge cases & errors:** `IMPORT_PART_FAILED` "Part import failed.", `IMPORT_ASSEMBLY_FAILED` "Assembly import failed.", `PARASOLID_IMPORT_FAILED`, `IMPORT_BODY_FAILED_CHECK` "Imported parts contained faults.", `NO_COMPOSITE_FOR_SINGLE_SOLID` "No composite part created when importing a single solid.", `CANNOT_IMPORT_MESH` "Cannot import mesh files to your current document.", `SIMPLIFY_BODY_FAILED` "File cannot be imported. Invalid parts." [FS errorstringenum].
- **OCCT mapping:** blob → reader (per format) → `TopoDS_Shape` per body + XCAF names/colors → app bodies; create composite = app-level composite part; units via `XSControl` unit handling (`xstep.cascade.unit`, OCCT 7.8 `DE_Wrapper`/`DESTEP_Parameters`) or explicit `BRepBuilderAPI_Transform` scale for meshes.
- **Priority:** P0.

### IO-05 Assembly / multi-file import (zip rules)
- **Purpose:** importing multi-file native assemblies.
- **Sources:** Document/importing_files.htm, File/supported_file_formats.htm [DOCS].
- **Behavior:** zip-importable: SOLIDWORKS, Inventor, Pro/Engineer, Creo/PVZ, NX, CATIA, Parasolid. Rules: zip contains top-level assembly + all parts and subassemblies; flatten folders (or zip a whole directory); zip name must equal top-level assembly name minus extension; no special characters; extension only `.zip` (not `.sldasm.zip`); do not rename. SOLIDWORKS: Pack and Go with "Flatten to single folder"; for configurations/lightweight components: Set Lightweight to Resolved, Rebuild on Save Mark > Add Mark for All Configurations, save. Alternative: save SOLIDWORKS assembly as Parasolid.
- **OCCT mapping:** for noshape's supported neutral formats (STEP/IGES/glTF/3MF) assemblies come in one file via XCAF; zip handling only needed for STEP external references (`STEPCAFControl_Reader` resolves external files) — design suggestion: support zip of STEP external-reference assemblies.
- **Priority:** P2.

### IO-06 Updating an imported file
- **Purpose:** replace imported geometry with a newer export while keeping downstream features.
- **Sources:** Document/working_with_imported_cad.htm, Document/importing_files.htm [DOCS].
- **Behavior:** Part Studio: right-click Import feature > Update → pick file; or CAD Imports file tab > Update button. Imported blob replaced; Import feature regenerates; features applied in Onshape after the import are re-applied "where possible" (failures shown in error state). Only Part Studios update; assemblies created by import are not updated. "Split into multiple documents" top-level assembly cannot be updated. Non-CAD file tabs also have Update/Download. Legacy (pre-1.93 documents): Update creates a new Part Studio.
- **OCCT mapping:** re-import → persistent naming must re-match faces/edges between old and new imported shapes (no history available): geometric matching heuristics (same surface type + bbox + area + centroid within tolerance; `BRepAlgoAPI_*` not applicable). Custom work.
- **Priority:** P1.

### IO-07 Repair panel (View in repair, Edit healthy moment, Replace reference)
- **Purpose:** guided fixing of broken references after an Import/Derived base change or any upstream edit.
- **Sources:** Document/repairing.htm [DOCS].
- **Behavior:**
  - Open via Versions and history panel: right-click a change > "View in Repair" (or Repair icon then pick a healthy change), or via failing feature context menu "Edit healthy moment of [Feature]" / broken reference field > "Edit healthy moment".
  - Repair panel shows a read-only past state side-by-side; synchronized camera (toggle "Synchronize view"), rollback bar synchronized always; hovering a broken reference highlights where it pointed in the panel (cross-highlight yellow, also mate connectors). Panel context menu: Zoom to fit (F), Isometric (Shift+7). Configuration switch propagates.
  - Replace reference: hover the field with the broken reference > Replace reference icon → pick replacement; "Propagate changes" (default on) pushes the new reference to all downstream features using it (read-only list "Propagating changes to features"); accept both dialogs with the feature checkmark. Fix top-down.
  - Limitations: Repair panel must be open; 1-to-1 replacement only; not for implicit/explicit mate connectors, in-context references, external sketch references; references that split into multiple (e.g., fillet edge later split) cannot propagate; non-healthy base change → blue notification. Measurements/analysis/mass properties/comments not shown in panel; section exclusions not shown; closes if active branch deleted.
- **OCCT mapping:** app-level: evaluate the Part Studio at a past microversion in a second context; reference remapping in the feature parameter store.
- **Priority:** P2.

### IO-08 Working with imported CAD (faults, fractures, direct editing)
- **Purpose:** post-import cleanup workflow.
- **Sources:** Document/working_with_imported_cad.htm [DOCS].
- **Behavior:** imported parts may be faulty, history-less, or fractured into surfaces. Tools: Highlight boundary edges (MT-25); repair with Enclose, Fill, Move boundary, Bridging curve, Composite curve; composite parts (open/closed) to group fractured bodies; direct editing (Delete face, Move face, Replace face, etc.). Replace duplicated imported components with references: export part → import into its own document → delete original → Replace instances in assemblies. Deleting an imported file tab warns which Part Studios are affected.
- **OCCT mapping:** `ShapeFix_Shape`, `ShapeAnalysis_FreeBounds`, `BRepBuilderAPI_Sewing`, `BRepAlgoAPI_Defeaturing` (Delete face), `BRepOffsetAPI_MakeFilling` (Fill).
- **Priority:** P1.

### IO-09 Mesh import and mesh referencing
- **Purpose:** faceted data for visualization, reference and mixed modeling.
- **Sources:** Document/working_with_imported_cad.htm, File/supported_file_formats.htm, File/importing_a_lidar_scan.htm [DOCS]; FS errors `MESH_NOT_SUPPORTED`, `NO_MESH_FOUND`, `MIXED_MODEL_LAYOUT` [FS].
- **Behavior:** mesh formats: Parasolid mesh, STL, OBJ, glTF, 3MF. Imported into a Part Studio, listed under "Meshes" (below Surfaces) in the Parts list (or as a mesh composite part). View and reference only (no editing of the mesh itself; mixed-modeling features in section 3 may operate on meshes). Mesh vertex selections are non-parametric (location recorded; transforming the mesh does not update them). Possible: 3-point plane from mesh points; measure area, distances, mass properties of closed meshes; Use (project) mesh points in sketch; mate connectors at mesh points; "Up to vertex" end conditions. Units chosen at import and changeable from the file tab units dropdown.
- **OCCT mapping:** mesh body = `TopoDS_Face`(s) carrying only `Poly_Triangulation` (no surface; `BRep_Builder::MakeFace(face, triangulation)`), or a custom mesh body type in the app; readers `RWStl::ReadFile`, `RWObj_CafReader`, `RWGltf_CafReader`, lib3mf; mass properties via `BRepGProp` on triangulation-only faces (OCCT 7.6+ supports `BRepGProp_MeshProps`).
- **Priority:** P1.

### IO-10 Export dialog (parts, Part Studios, instances, assemblies)
- **Purpose:** common export UI and options.
- **Sources:** File/exporting_files.htm [DOCS]; bundle strings "Chordal tolerance", "Angular deviation", "Minimum facet width", "Distance tolerance", "Angular tolerance", "Maximum chord length", "Binary", "Text" [BUNDLE].
- **Entry points:** select parts/surfaces in Parts list or graphics area (or instances/subassemblies in an assembly) > context menu Export; tab context menu Export (whole Part Studio — includes hidden parts — or assembly/subassembly); Tab manager multi-select of several Part Studios or several Assemblies (not mixed) > Export; Revision history > Export (out of scope); Documents page list-view export.
- **Parameters:**

  | Field | Type | Values / default | Visible when |
  |---|---|---|---|
  | File name | text | tab name, or "<tab> - <part>" for single part (inferred format) | always; characters `\ / * : \| > < ? "` replaced by `-`; Date properties default YYYY-MM-DD |
  | Format | enum | PARASOLID, ACIS, STEP, IGES, PVZ, JT, RHINO, GLTF, GLB, OBJ, 3MF, URDF, STL | URDF only for assembly export |
  | Export models oriented Y axis up | bool | off | always |
  | Preprocessing | enum | None (default), Automatic, Remove small entities only (advanced), Remove small entities only (basic), Remove small entities only (advanced) and convert all surfaces to b-surfaces | STEP only |
  | Geometry format | enum | STL, GLTF, GLB, OBJ | URDF only |
  | Use latest version | bool | on | PARASOLID, RHINO, STEP |
  | Version | enum | per format (Parasolid v25–v37; Rhino 2–8; STEP AP203/AP214/AP242) | PARASOLID, RHINO, STEP; editable when Use latest version off |
  | Use custom units for export | bool + enum | off (STEP written in meters); Centimeter, Foot, Inch, Meter, Millimeter | STEP only |
  | Include derived tolerances in export | bool | off (inferred) | STEP only (MBD) |
  | Use custom annotation text height for MBD export | bool + length "Annotations text height" | off | STEP only (MBD) |
  | Use compatibility mode for surfaces and curves | bool | off | IGES only (older IGES for legacy CAD/CAM) |
  | Resolution | enum | Coarse, Medium, Fine, Custom (default not documented; Medium inferred) | GLTF, GLB, OBJ, 3MF, STL, URDF |
  | Distance tolerance / Angular tolerance / Maximum chord length | length / angle / length | — | Resolution = Custom, for GLTF/GLB/OBJ/3MF |
  | Chordal tolerance / Angular deviation / Minimum facet width | length / angle / length | — | Resolution = Custom, for STL |
  | Compress | bool | off (inferred) | GLTF, GLB (meshes compressed; non-mesh not) |
  | STL Format | enum | Text, Binary | STL |
  | Units | enum | Centimeter, Foot, Inch, Meter, Millimeter, Yard | STL |
  | Options | radio | Download; Download and store file in a new tab; Email with file download link; Store file in a new tab | always |
  | Export unique parts as individual files | bool | off (multiple parts → one file); on → zip of one file per part | multiple parts selected (docs: STL and Parasolid support one vs individual files) |
  | Apply export rules to contents | bool | on | multiple Part Studios/Assemblies selected; "View export rules" link |
  | Include hidden instances in export | bool | off | assembly export |

- **Behavior:** composite parts export as separate parts. Suppressed parts/instances never exported (hidden instances excluded unless checkbox; mobile: hidden and suppressed excluded). OBJ export = zip with .obj + .mtl, .obj in meters only (re-import requires extracting and importing only the .obj). Exported data has no history. Progress/errors in Notifications.
- **OCCT mapping:** per-format writers (IO-16..IO-30); tessellation params → `IMeshTools_Parameters` {Deflection = chordal/distance tolerance, Angle = angular deviation/tolerance, MinSize = minimum facet width}; "Maximum chord length" has no direct BRepMesh parameter → post-split long triangle edges (custom) or set `Deflection` relative; Y-up via `gp_Trsf` rotation −90° about X before writing (Z-up → Y-up); "Options: store in new tab" = add file tab to document; email = out of scope.
- **Priority:** P0.

### IO-11 Export naming, export rules, multi-tab export
- **Purpose:** batch export and metadata-driven file names.
- **Sources:** File/exporting_files.htm; Plans/my_account_preferences.htm (Export rules, fetched online) [DOCS].
- **Behavior:** Export rules (Account > Preferences): Add export rule → Scope (Part, Assembly, Drawing, Part Studio, File), optional Category (one rule per category), file format (or all), file-name convention built from properties (default and custom properties); applied when exporting; multi-tab export produces a zip with "Apply export rules to contents" controlling per-file naming. Special characters replaced with `-`.
- **OCCT mapping:** app templating engine over part metadata.
- **Priority:** P2.

### IO-12 Sketch, planar face and flat-pattern export to DXF/DWG
- **Purpose:** 2D export of 3D model data.
- **Sources:** File/exporting_files.htm, Home/context_menus.htm [DOCS].
- **Entry points:** sketch in Feature list > context menu "Export as DXF/DWG"; face or plane in graphics area > "Export as DXF/DWG"; sheet metal flat view > export (flat pattern; specified in SM- section).
- **Parameters:** File name; Format DXF/DWG; Version (R11–14, 2000, 2004, 2007, 2010, 2013, 2018); Units or "Use workspace units" (face export; sketches export in document default units); Options (download/store); "Export splines as polylines" (default off: splines exported as splines); "Set z-height to zero and normals to positive".
- **Behavior:** planar faces export outer solid geometry only (no dimensions or interior geometry) — inner loops are boundary edges of the face (inferred: holes included as loops; "interior geometry" = sketch/annotation content).
- **OCCT mapping:** face/sketch edges → 2D in face plane frame (`BRep_Tool::CurveOnSurface` on the plane, or project with `gp_Trsf`), lines/arcs/circles/ellipses as native DXF entities, B-splines as DXF SPLINE (degree, knots, control points from `Geom2d_BSplineCurve`) or discretized polylines (`GCPnts_QuasiUniformDeflection`); writer custom (DXF ASCII is simple) — DWG via LibreDWG/ODA (IO-29).
- **Priority:** P1 (DXF), P2 (DWG).

### IO-13 Drawing export
- **Purpose:** export drawing tabs.
- **Sources:** Drawing/exporting_a_drawing.htm, File/exporting_files.htm [DOCS].
- **Entry points:** drawing tab context menu > Export; Tab manager multi-select drawings > "Export drawings".
- **Parameters:**

  | Field | Formats | Values |
  |---|---|---|
  | File name | all | text |
  | Format | — | PDF, DWG, DXF, DWT, SVG, PNG, JPEG (+ CSV for inspection items/definition list via separate commands) |
  | Version | DWG/DXF/DWT | R11–14, 2000, 2004, 2007, 2010, 2013, 2018 (DWT 2013/2018); choose 2013 for strike-through text |
  | Sheets | all | all / current / Custom sheets (range or numbers) |
  | Overridden dimensions | all | Show underlines / Hide underlines |
  | Text | PDF | Normal / Selectable |
  | Text | DWG/DXF/DWT | Export as text / Explode into polylines (notes) / Explode all into polylines (annotations, notes, table text, dimensions) |
  | Color | PDF | Color / Black and white / Grayscale |
  | Units | DWG/DXF | drawing units or another unit |
  | Options | all | Download / Download and store in new tab / Email link / Store in new tab |
  | Apply export rules to contents | multi-drawing | bool |
  | Export splines as polylines | all except PDF | bool |
  | Set z-height to zero and normals to positive | DWG/DWT/DXF | bool |
  | Hide inspection items | all | bool (shown by default) |
  | Exclude off-sheet content | DWG/DWT/DXF | bool |

- **Behavior:** simplified DWG output readable by most readers; max custom sheet 1800 in × 1800 in; images on imported drawings have no image properties. Blue message bubble on start, notification on completion.
- **OCCT mapping:** drawing geometry is already 2D (HLR output, see DR- section); PDF via Rust PDF writer (e.g. `printpdf`/`pdf-writer`) with embedded fonts (selectable text) or text-as-paths; SVG custom; PNG/JPEG via offscreen raster; DXF/DWG as IO-29.
- **Priority:** P0 (PDF), P1 (DXF, SVG, PNG), P2 (DWG/DWT).

### IO-14 Drawing import (DWG/DXF/DWT)
- **Purpose:** bring 2D AutoCAD drawings in as Onshape drawing tabs.
- **Sources:** Drawing/importing_a_drawing.htm [DOCS]; FS DXF errors [FS].
- **Behavior:** from Documents page → new document; within a document (+ > Import) → file tab + drawing tab, both named after the file. Model-space background color set in Account > Preferences > Drawings. (Sketch "Insert DXF/DWG" into a sketch is specified in section 2.) Related errors: `DXF_UNKNOWN_ENTITY` "Could not import unrecognized entity.", `DXF_NONUNIFORM_SCALING` "Only uniform block scaling is supported.", `DXF_COULD_NOT_READ_FILE`, `DXF_COULD_NOT_CONVERT_HATCH` "Hatching is not supported.", `DXF_COULD_NOT_CONVERT_BODY` "Cannot import solid bodies into sketches.", `SKETCH_INSERT_DWG_MAX_ENTITIES_EXCEEDED`.
- **OCCT mapping:** DXF reader: libdxfrw (GPL-2+) or custom (DXF ASCII/binary parse); DWG reader: LibreDWG (GPL-3) or ODA (commercial) — license choice impacts noshape distribution.
- **Priority:** P2.

### IO-15 Download and non-CAD files
- **Purpose:** raw file tabs.
- **Sources:** File/downloading_files.htm, Document/importing_files.htm [DOCS].
- **Behavior:** any file tab → context menu Download (original name and type). Non-CAD imports become tabs (viewer for pdf/images/video/text/markdown/csv/json; info panel otherwise) with Update and Download buttons.
- **OCCT mapping:** app.
- **Priority:** P2.

### IO-16 Parasolid (.x_t / .x_b; mesh and mixed models)
- **Direction:** import (B-rep v10.0–v38.1, mesh v28–v37, mixed v32+; parts and assemblies) and export (B-rep v25–v37, mesh v28–v37, mixed; parts/Part Studios/assemblies).
- **Onshape options:** export Version / Use latest version; Export unique parts as individual files (one file vs zip); Y-axis up. Import is "preferred" because Onshape's kernel is Parasolid (lossless).
- **OCCT feasibility:** Commercial only (OCC "Parasolid import" component or Siemens Parasolid licensing/third-party SDK). No open reader/writer. Recommendation: do not implement for MVP; tell users to exchange STEP AP242; optional plug-in interface for a licensed translator.
- **Priority:** P2.

### IO-17 STEP (.step / .stp)
- **Direction:** import (AP203, AP214, AP242; geometry and face color only; parts and assemblies) and export (AP203/AP214/AP242; parts: geometry, MBD data, face color; assemblies: geometry and face color).
- **Onshape options (export):** Version + Use latest version; Preprocessing (None / Automatic [centering if outside valid box + HealAndSew] / Remove small entities only (basic: small edges, small & sliver faces, spikes + periodic splitting) / (advanced: + split ring edges, reparameterize split faces, force trimming loops, trim fin SP-curves) / (advanced) and convert all surfaces to b-surfaces); Use custom units for export (cm, ft, in, m, mm; default meters); Include derived tolerances in export (MBD); Use custom annotation text height for MBD export; Y axis up; single file (assembly structure preserved). Import: appearances, Y-up, composite, join adjacent surfaces, faults.
- **OCCT feasibility:** Native.
  - Read: `STEPCAFControl_Reader` (SetColorMode, SetNameMode, SetLayerMode, SetPropsMode, SetGDTMode for AP242 PMI) → XCAF document; `STEPControl_Reader` for geometry only. Assembly structure via `XCAFDoc_ShapeTool` (labels, references, locations).
  - Write: `STEPCAFControl_Writer` (colors, names, assembly) with schema via `Interface_Static::SetCVal("write.step.schema", "AP203"|"AP214IS"|"AP242DIS")` (OCCT 7.8: `DESTEP_Parameters`/`StepData_ConfParameters::WriteSchema`); units `write.step.unit` (MM, INCH, M, CM, FT); product name `write.step.product.name`.
  - MBD/PMI export: OCCT can write AP242 semantic GD&T from `XCAFDoc_DimTolTool` (partial); presentation annotation text height custom.
  - Preprocessing equivalents: `ShapeFix_Shape`; `ShapeUpgrade_RemoveInternalWires`/`ShapeFix_Wireframe::FixSmallEdges`/`ShapeFix_FixSmallFace` (small entities, slivers, spikes); `ShapeUpgrade_ShapeDivideClosed` + `ShapeUpgrade_ShapeDivideClosedEdges` (periodic splitting/ring edges); `ShapeCustom::ConvertToBSpline` (convert all surfaces to B-surfaces); "centering" via translation if outside ±500 m box.
- **Priority:** P0.

### IO-18 IGES (.igs / .iges)
- **Direction:** import up to 5.3 (parts); export 5.3 (parts, Part Studios, assemblies).
- **Onshape options:** export "Use compatibility mode for surfaces and curves" (older IGES for legacy CAD/CAM); import "IGES postprocessing", join adjacent surfaces.
- **OCCT feasibility:** Native: `IGESCAFControl_Reader/Writer` (colors, names, layers) or `IGESControl_Reader/Writer`. Compatibility mode ↔ `write.iges.brep.mode` = 0 ("Faces": trimmed surfaces, entity 144/142, legacy-friendly) vs 1 ("BRep": MSBO entity 186 solids); IGES 5.3 output standard. Postprocessing ↔ `ShapeFix_Shape` + `BRepBuilderAPI_Sewing` (`read.iges.bspline.continuity`, `read.precision.mode`).
- **Priority:** P1.

### IO-19 ACIS (.sat)
- **Direction:** import up to 2023 1.0 (parts, assemblies); export Version 5 (parts/Part Studios/assemblies).
- **Onshape options:** none specific (best format for AutoCAD solids).
- **OCCT feasibility:** Commercial (Spatial or OCC ACIS component). SAT v5 ASCII is documented-ish and an open writer for analytic+spline B-rep is possible but costly. Recommendation: P2, defer.
- **Priority:** P2.

### IO-20 STL
- **Direction:** import (mesh, view/reference; units chosen at import) and export (parts, Part Studios, assemblies).
- **Onshape options (export):** Resolution Coarse/Medium/Fine/Custom (Custom: Chordal tolerance, Angular deviation, Minimum facet width); STL Format Text/Binary; Units cm/ft/in/m/mm/yd; Export unique parts as individual files (zip) vs one file; Y-axis up.
- **OCCT feasibility:** Native: `BRepMesh_IncrementalMesh(shape, deflection, isRelative=false, angle)` with `IMeshTools_Parameters::MinSize` = minimum facet width; `StlAPI_Writer` (`ASCIIMode` true/false) or `RWStl::WriteAscii/WriteBinary` from a merged `Poly_Triangulation`; scale to units before writing. Read: `RWStl::ReadFile` → `Poly_Triangulation` (merge duplicate nodes). Preset suggestion (inferred): Coarse = 0.5 mm / 30°, Medium = 0.1 mm / 15°, Fine = 0.02 mm / 5° scaled by part size.
- **Priority:** P0.

### IO-21 OBJ
- **Direction:** import (mesh) and export (parts, Part Studios, assemblies).
- **Onshape options:** Resolution (Coarse/Medium/Fine/Custom: Distance tolerance, Angular tolerance, Maximum chord length); output zip with .obj + .mtl, meters only; import units selectable.
- **OCCT feasibility:** Native: `RWObj_CafWriter` (OCCT 7.6+, writes .obj + .mtl with colors from XCAF) and `RWObj_CafReader`/`RWObj_Reader`. Zip via `zip` crate.
- **Priority:** P1.

### IO-22 glTF / GLB
- **Direction:** import glTF 2.0 (parts and assemblies; mesh); export GLTF and GLB (parts, Part Studios, assemblies).
- **Onshape options:** Resolution (as OBJ); Compress (meshes only).
- **OCCT feasibility:** Native: `RWGltf_CafWriter` (binary GLB or glTF+bin, node hierarchy from XCAF, materials via `XCAFDoc_VisMaterialTool`; Draco compression supported in OCCT 7.7+ via `RWGltf_DracoParameters` when built with Draco), `RWGltf_CafReader`. Coordinate system: glTF is Y-up — writer converts (`RWMesh_CoordinateSystemConverter`).
- **Priority:** P1.

### IO-23 3MF
- **Direction:** import (parts, assemblies; mesh) and export (parts, Part Studios, assemblies).
- **Onshape options:** Resolution (as OBJ).
- **OCCT feasibility:** Open lib: lib3mf (BSD-2-Clause, C++ with C ABI) or a custom writer (3MF = zip + XML mesh, straightforward); tessellate via `BRepMesh_IncrementalMesh`; colors from XCAF → 3MF base materials.
- **Priority:** P1.

### IO-24 JT
- **Direction:** import up to 11.0 (parts, assemblies); export up to 11.0.
- **OCCT feasibility:** Commercial (Siemens JT Open toolkit or OCC JT component). Recommendation: P2/defer.
- **Priority:** P2.

### IO-25 Rhino (.3dm)
- **Direction:** import v2–v8 (parts, assemblies); export v2–v8 (parts, Part Studios).
- **Onshape options:** Version / Use latest version.
- **OCCT feasibility:** Open lib: openNURBS (McNeel, permissive license) — read `ON_Brep`/`ON_Extrusion`/`ON_NurbsSurface`/meshes and convert to OCCT (`Geom_BSplineSurface`, trimming curves → `BRepBuilderAPI_MakeFace` + `ShapeFix_Face`); write reverse (OCCT surfaces → `GeomConvert::SurfaceToBSplineSurface` → `ON_NurbsSurface`). Moderate custom effort.
- **Priority:** P2.

### IO-26 PVZ (Creo View)
- **Direction:** import (zip-capable) and export version 8.0 (parts, Part Studios, assemblies).
- **OCCT feasibility:** Infeasible (open) — proprietary PTC visualization format. Skip.
- **Priority:** P2 (won't do).

### IO-27 URDF (assembly export)
- **Direction:** export only, assemblies only.
- **Onshape options:** Geometry format STL / GLTF / GLB / OBJ; Resolution; other common options.
- **Behavior (inferred):** writes a URDF XML (links per rigid group/instance, joints from mates — revolute/slider/fastened mapped to revolute/prismatic/fixed) plus mesh files for link geometry, zipped.
- **OCCT feasibility:** custom XML writer (quick-xml crate) + mesh writers above; mass/inertia per link from MT-08 (`BRepGProp`).
- **Priority:** P2.

### IO-28 Proprietary native CAD imports (CATIA V5, SOLIDWORKS, Inventor, Pro/E-Creo, NX, Solid Edge)
- **Direction:** import only (parts; assemblies via zip). Versions per IO-02 table. Import material density supported for SolidWorks, Solid Edge, CATIA V5, Pro/E, Creo, NX. Y-up not supported for .ipt and .catpart. Solid Edge .psm → plain geometry (thicken to make sheet metal).
- **OCCT feasibility:** no open readers exist (proprietary, encrypted/undocumented formats). Options: (a) commercial SDK plug-in (OCC native-CAD readers, CAD Exchanger SDK, HOOPS Exchange, Datakit); (b) rely on users exporting STEP AP242/Parasolid from the source system (recommended). Some formats embed Parasolid (SOLIDWORKS, Solid Edge, NX) — still requires a Parasolid kernel to read.
- **Priority:** P2 (commercial plug-in interface only).

### IO-29 DWG / DXF / DWT (2D)
- **Direction:** import drawings (DWG ≤2018, DXF ≤2013/2018, DWT 2013/2018); also sketch Insert DXF/DWG (section 2); export from drawings, sketches, planar faces, flat patterns (versions R11–14 … 2018), flattened surfaces (DXF).
- **Onshape options:** see IO-12 and IO-13 (version, units, splines as polylines, z-height zero/normals positive, text handling, exclude off-sheet content).
- **OCCT feasibility:** DXF: no open OCCT DXF (OCC sells a DXF component); implement a custom DXF writer (R12/R2000/R2018 ASCII: LINE, ARC, CIRCLE, ELLIPSE, SPLINE, LWPOLYLINE, TEXT/MTEXT, DIMENSION as blocks, layers) — low effort; reader: libdxfrw (GPL-2+) or custom subset parser. DWG: LibreDWG (GPL-3; read/write partial) or ODA Drawings SDK (commercial). DWT = DWG with template extension. Normal-positive option: ensure extrusion direction (210/220/230 group codes) = (0,0,1) and Z = 0.
- **Priority:** P1 (DXF export), P2 (DXF import, DWG).

### IO-30 PDF / SVG / PNG / JPEG / CSV outputs
- **Direction:** export only (drawings; SVG also from flattened surfaces; PNG/JPEG from print/download image).
- **Onshape options:** PDF text Normal/Selectable, Color/Black and white/Grayscale; sheet selection.
- **OCCT feasibility:** none in OCCT (OCCT `Image_AlienPixMap` can save PNG/JPEG from offscreen rendering if using OCCT viewer); use Rust crates (`pdf-writer`/`printpdf`, `svg`, `image`); line weights and fonts from drawing standard.
- **Priority:** P0 (PDF from drawings), P1 (SVG/PNG).

### IO-31 Flattened-surface export
- **Direction:** export only from the Flatten surfaces tool (MT-41): PARASOLID, STL, DXF, SVG; option "Export inner edges".
- **OCCT feasibility:** STL native; DXF/SVG custom writers; Parasolid commercial → substitute STEP/BREP in noshape (design suggestion).
- **Priority:** P2.

### IO-32 noshape native exchange format (design suggestion, not in Onshape)
- **Purpose:** lossless round-trip of noshape geometry and documents without Parasolid.
- **Behavior:** export/import OCCT BREP (`BRepTools::Write/Read` ASCII, or `BinTools::Write/Read` binary) per part; document archive = zip of feature data (JSON) + cached BREP + meshes; optionally XCAF `XBF` (`BinXCAFDrivers`) to keep names/colors/assembly.
- **OCCT mapping:** Native.
- **Priority:** P1.

### IO-33 Lidar scan import (mobile)
- **Purpose:** iOS-only environment/object scanning (GLB/OBJ mesh into a "Scan"/"Capture" Part Studio with Import feature + mesh composite part).
- **Sources:** File/importing_a_lidar_scan.htm [DOCS].
- **Note:** mobile-only; out of scope for noshape. Equivalent desktop path = import GLB/OBJ mesh (IO-09).
- **Priority:** P2 (out of scope).

## 8. Keyboard shortcuts, mouse navigation & selection

Scope: complete default shortcut map (Onshape "Keyboard Shortcuts and Hotkeys" page, every entry), shortcut customization,
mouse/touchpad navigation and mouse presets, selection mechanics (click, box, select other, through-transparent,
hover-targeted commands), the shortcut toolbar, Search tools, the triad manipulator and numeric-field input behaviors.
Sources: Home/keyboard_shortcuts_and_hotkeys.htm, Home/user_interface_basics.htm, View/view_navigation_and_the_view_cube.htm,
View/camera_and_render_options.htm, Home/selection.htm, Home/select_other.htm, Home/search_tools.htm, Home/triad_manipulator.htm,
Home/numeric_fields.htm, Home/dialogs.htm [DOCS]; Plans/my_account_preferences.htm (fetched online 2026-10-17, Mouse controls,
Keyboard shortcuts, Shortcut toolbars) [DOCS]; client bundle command-action ids, mouse-preset templates and highlight constants [BUNDLE].
Shortcut table IDs: one ID per shortcut row; every documented shortcut appears exactly once per context it is listed in.

### KB-01 Keyboard shortcuts map (in-app reference)
- **Purpose:** discoverable list of all shortcuts.
- **Sources:** Home/keyboard_shortcuts_and_hotkeys.htm [DOCS]; bundle `OPEN_KEYBOARD_SHORTCUTS` [BUNDLE].
- **Behavior:** opened from Help menu > Keyboard shortcuts, or Shift+/ (toggle). Filter by tab type (General, Part Studio, Assembly, 3D View, Sketch, Feature Studio, Drawings) or by "Search shortcuts" text field. Shows the user's customized bindings.
- **OCCT mapping:** n/a (GPUI modal with filterable table generated from the action registry).
- **Priority:** P1.

### KB-02 Customizing keyboard shortcuts
- **Purpose:** user-remappable hotkeys.
- **Sources:** Home/keyboard_shortcuts_and_hotkeys.htm, Plans/my_account_preferences.htm [DOCS]; bundle "Reset shortcuts to defaults?" [BUNDLE].
- **Behavior:**
  - Account > My account > Preferences > Keyboard shortcuts section; customizable categories: General tools, Part Studio tools, Assembly tools, 3D View tools, Sketch tools, Feature Studio, Drawing tools.
  - Hover a row > pencil icon > type a new single key or key combination > checkmark or Enter to accept.
  - Validation: **Onshape-controlled shortcut conflict** (system-reserved combination; error, cannot use); **Browser shortcut conflict** (combination used by browser/OS; info message); **Override shortcut conflict(s)** (already bound to another action; warning, accepting reassigns).
  - "Disable shortcut" checkbox (unbinds action); untick to re-enable. "Reset to default" per shortcut; "Reset all" (top right) for all.
  - Customizations sync to all clients (web, iOS, Android) for the user; on iOS a new built-in shortcut colliding with a custom one shows a "used by more than one action" warning.
  - Any action in the command registry can be bound, including tools without default keys (e.g. analysis tools) — see "Assignable actions without default keys".
- **OCCT mapping:** app: action registry (`action_id → default chord, context`) + user override map persisted in preferences; conflict detection per context; reserved list (OS/system shortcuts such as Cmd+Q, Alt+F4, Ctrl+W — design suggestion for the native app instead of "browser conflicts").
- **Priority:** P1.

### KB-03 Shortcut tables: conventions and context dispatch
- **Purpose:** how to read the tables and resolve keys bound in several contexts.
- **Behavior / semantics:**
  - Columns: Keys (Win/Linux) / Mac. Where the docs write `ctrl/⌘` the Mac key is ⌘; `alt/⌥` → ⌥ Option. Where the docs write plain `ctrl+…` (Ctrl+U, Ctrl+M, Ctrl+Space, Ctrl+S/Q in drawings, Ctrl+arrows) the same physical Ctrl key is listed for Mac (docs give no ⌘ variant; navigation page confirms Ctrl+arrow on macOS).
  - Letter keys are case-insensitive; "Shift+letter" is the shifted variant.
  - Asterisk (*) after some sketch keys reproduces the source page marker (unexplained in docs; likely marks shortcuts present in the default shortcut toolbar or older defaults — treat as informational).
  - Context precedence (inferred from overlapping bindings): active text field (numeric/text input consumes keys) > active sketch (Sketch table) > active tab type (Part Studio / Assembly / Drawing / Feature Studio table) > 3D View > General. Examples of overlaps: Shift+S (Part Studio: new Sketch; Sketch: Point; Assembly: Snap mode), A (General: flip primary axis while editing an axis-bearing dialog; Sketch: 3-point arc), Shift+F (Part Studio: Fillet; Sketch: sketch fillet), I (Assembly: Insert; Sketch: Coincident), M (Assembly: Fasten mate; Sketch: Trim), H (Assembly: Show mates mode; Sketch: Horizontal), Shift+H (Part Studio: hide/show sketches; Drawing: Foreshorten dimension), Shift+D (General: Dihedral analysis; Drawing: Diameter dimension), Ctrl+M (Mate connector; Drawing: Max/min dimension), N (View normal to; Drawing: Note), P (planes; Drawing: Projected view), Shift+R (View in high quality; Drawing: Radial dimension), Ctrl+S (Feature Studio commit; Drawing: hide/show sheets), Shift+Q (Sketch: Symmetric; Drawing: midpoint/quad points), Esc (exit command; Feature Studio: dismiss autocomplete).
  - Hover-targeted commands (Y, Shift+Y, Shift+I, Shift+T, N, grave accent) act on the pre-highlighted entity under the cursor if nothing (or additionally to what) is selected.
  - Tool toggles: pressing a tool's key again (or Esc) exits the tool.
- **OCCT mapping:** app keymap with context stack.
- **Priority:** P0.

#### General

| ID | Keys (Win/Linux) | Mac | Action | Context |
|---|---|---|---|---|
| KB-04 | Shift+Enter | Shift+Return | Commit the command and immediately open another (empty) dialog of the same command | General (feature/tool dialogs) |
| KB-05 | Enter | Return | Commit (accept) the command in use; in Select other list: select highlighted entry; in shortcut editor: accept | General |
| KB-06 | Esc | Esc | Exit/cancel the command in use; also closes Search tools, shortcut toolbar, Isolate/Make transparent dialogs, deselects active tool | General |
| KB-07 | Space | Space | Deselect selected feature(s)/entities (clear selection) | General |
| KB-08 | Ctrl+C | ⌘+C | Copy selected feature (also copies a selected Measure/Mass value) | General |
| KB-09 | Ctrl+V | ⌘+V | Paste copied feature; may be repeated | General |
| KB-10 | Ctrl+Z | ⌘+Z | Undo recent activity in the document session (repeatable; per user, per tab, per session) | General |
| KB-11 | Ctrl+Y | Shift+⌘+Z | Redo recently undone activity (repeatable) | General |
| KB-12 | Delete / Backspace | Delete | Remove selected features/entities | General |
| KB-13 | Shift+C | Shift+C | Open/close Curve/surface analysis (in sketch: auto-selects all sketch curves; selections persist) | General (Sketch, Part Studio, Assembly) |
| KB-14 | Shift+D | Shift+D | Open/close Dihedral analysis (selections persist) | General (Sketch, Part Studio) |
| KB-15 | Ctrl+Shift+F | ⌘+Shift+F | FeatureScript search: find text in the current Feature Studio | General / Feature Studio |
| KB-16 | Ctrl+U | Ctrl+U | Feedback / report a bug (out of scope for noshape) | General |
| KB-17 | A | A | Flip primary axis (flips the Z axis when editing, e.g. mate connector orientation) | General (dialog with axis) |
| KB-18 | K | K | Show or hide mate connectors | General |
| KB-19 | Shift+/ | Shift+/ | Toggle the Keyboard shortcuts map | General |
| KB-20 | Shift (hold) | Shift (hold) | Lock mate inference (while placing a mate connector / mating) | General (Assembly, mate connector) |
| KB-21 | Ctrl+M | Ctrl+M | Open the Mate connector dialog | General (Part Studio, Assembly) |
| KB-22 | [ | [ | Open the Measure dialog | General (Part Studio, Assembly, Drawing) |
| KB-23 | Ctrl+click (document) | ⌘+click | Open the selected document in a new browser tab (noshape: new tab/window) | Documents page |
| KB-24 | Shift+click (document) | Shift+click | Open the selected document in a new browser window | Documents page |
| KB-25 | Ctrl+Space | Ctrl+Space | Switch between recently opened tabs in the session (drawings, assemblies, Part Studios) | General |
| KB-26 | Shift+N | Shift+N | Prompt for a new name of the selected instance or feature (rename) | General |
| KB-27 | Q | Q | Select/exit Construction tool (requires active sketch; applies to preselected entities or entities being created) | General / Sketch |
| KB-28 | Alt+C | ⌥+C | Open Search tools (top right) with cursor in the search field | General (Part Studio, Assembly, Sketch) |
| KB-29 | `` ` `` (grave accent) | `` ` `` | Select other: list of entities selectable relative to cursor position (through depth); repeated grave moves down the list | General (Sketch, Part Studio, Assembly) |
| KB-30 | Shift+`` ` `` | Shift+`` ` `` | Select other: cycle back up the list | General (Select other open) |
| KB-31 | S | S | Open the shortcut toolbar at the mouse cursor | General |
| KB-32 | Alt+T | ⌥+T | Open the Tab Manager | General |

#### Part Studio

| ID | Keys (Win/Linux) | Mac | Action | Context |
|---|---|---|---|---|
| KB-33 | Shift+E | Shift+E | Extrude (from open sketch uses it; otherwise prompts for face/sketch) | Part Studio |
| KB-34 | Shift+F | Shift+F | Fillet (Edge fillet dialog) | Part Studio |
| KB-35 | Shift+H | Shift+H | Show or hide sketches | Part Studio |
| KB-36 | ↑ / ↓ (rollback bar selected) | ↑ / ↓ | Move the rollback bar up/down through the Features list | Part Studio |
| KB-37 | Shift+W | Shift+W | Revolve (from open sketch; otherwise prompts for face/sketch) | Part Studio |
| KB-38 | Shift+S | Shift+S | New Sketch (prompts "Select a sketch plane") | Part Studio |

#### Assembly

| ID | Keys (Win/Linux) | Mac | Action | Context |
|---|---|---|---|---|
| KB-39 | J | J | Show or hide mates | Assembly |
| KB-40 | I | I | Open Insert parts and assemblies dialog | Assembly |
| KB-41 | M | M | Open Fasten mate dialog | Assembly |
| KB-42 | H | H | Toggle Show mates mode (toolbar) | Assembly |
| KB-43 | Shift+S | Shift+S | Toggle Snap mode (toolbar) | Assembly |

#### 3D View (Part Studio and Assembly; most also in Render Studio)

| ID | Keys (Win/Linux) | Mac | Action | Context |
|---|---|---|---|---|
| KB-44 | Shift+1 | Shift+1 | Front view | 3D View |
| KB-45 | Shift+2 | Shift+2 | Back view | 3D View |
| KB-46 | Shift+3 | Shift+3 | Left view | 3D View |
| KB-47 | Shift+4 | Shift+4 | Right view | 3D View |
| KB-48 | Shift+5 | Shift+5 | Top view | 3D View |
| KB-49 | Shift+6 | Shift+6 | Bottom view | 3D View |
| KB-50 | Shift+7 | Shift+7 | Isometric view | 3D View (also Repair panel) |
| KB-51 | N | N | View normal to preselected/hovered plane or face (screen-parallel); press again to flip to inverse-normal | 3D View |
| KB-52 | F | F | Zoom to fit (largest view showing the whole design); also double-click scroll wheel | 3D View, Drawing, Repair panel |
| KB-53 | W | W | Zoom to window: drag a bounding box to magnify; key toggles the mode | 3D View, Drawing |
| KB-54 | Z | Z | Zoom out | 3D View |
| KB-55 | Shift+Z | Shift+Z | Zoom in on the center of the current view | 3D View |
| KB-56 | ← → ↑ ↓ | ← → ↑ ↓ | Coarse rotate 15° left/right/up/down; press and hold = continuous rotation | 3D View |
| KB-57 | Shift+← → ↑ ↓ | Shift+arrows | Rotate 90° | 3D View |
| KB-58 | Ctrl+← → ↑ ↓ | Ctrl+arrows | Rotate 5° (precision) | 3D View |
| KB-59 | Ctrl+Shift+← → ↑ ↓ | Ctrl+Shift+arrows | Pan the design up/down/left/right | 3D View |
| KB-60 | Shift+X | Shift+X | Open Section view dialog (multiple planes selectable) | 3D View |
| KB-61 | Shift+V | Shift+V | Open Named views dialog (prompts for a new view name) | 3D View |
| KB-62 | Shift+R | Shift+R | Toggle View in high quality | 3D View |
| KB-63 | P | P | Hide or show planes | 3D View |
| KB-64 | Shift+P | Shift+P | Hide all construction geometry | 3D View |
| KB-65 | Y | Y | Hide selected or hovered part/instance(s) | 3D View |
| KB-66 | Shift+Y | Shift+Y | Show hidden (hovered/selected) instance(s) | 3D View |
| KB-67 | Shift+I | Shift+I | Isolate selected or hovered part/instance(s) (cumulative while Isolate is active) | 3D View |
| KB-68 | Shift+T | Shift+T | Make selected or hovered part/instance(s) transparent (cumulative) | 3D View |
| KB-69 | Alt+click | ⌥+click | Select through transparent geometry (parts made transparent via appearance) | 3D View |

#### Sketch (all require an active sketch / selected sketch plane; pressing again exits the tool)

| ID | Keys (Win/Linux) | Mac | Action | Context |
|---|---|---|---|---|
| KB-70 | L * | L | Line tool | Sketch |
| KB-71 | Shift+A | Shift+A | Switch between Line and Tangent arc while drawing | Sketch (line tool active) |
| KB-72 | A | A | 3 point arc tool | Sketch |
| KB-73 | C * | C | Center point circle tool | Sketch |
| KB-74 | G * | G | Corner rectangle tool | Sketch |
| KB-75 | R * | R | Center point rectangle tool | Sketch |
| KB-76 | Shift+S | Shift+S | Sketch point tool | Sketch |
| KB-77 | Shift+F | Shift+F | Sketch fillet tool | Sketch |
| KB-78 | O | O | Offset tool | Sketch |
| KB-79 | X | X | Extend tool | Sketch |
| KB-80 | M * | M | Trim tool | Sketch |
| KB-81 | U | U | Use (project/convert) tool | Sketch |
| KB-82 | Q | Q | Construction toggle | Sketch |
| KB-83 | D * | D | Dimension tool | Sketch |
| KB-84 | I * | I | Coincident constraint | Sketch |
| KB-85 | Shift+O | Shift+O | Concentric constraint | Sketch |
| KB-86 | Shift+U | Shift+U | Curvature constraint | Sketch |
| KB-87 | E | E | Equal constraint | Sketch |
| KB-88 | Shift+J | Shift+J | Fix constraint | Sketch |
| KB-89 | H * | H | Horizontal constraint | Sketch |
| KB-90 | V * | V | Vertical constraint | Sketch |
| KB-91 | Shift+M | Shift+M | Midpoint constraint | Sketch |
| KB-92 | Shift+K | Shift+K | Normal constraint | Sketch |
| KB-93 | B * | B | Parallel constraint | Sketch |
| KB-94 | Shift+L * | Shift+L | Perpendicular constraint | Sketch |
| KB-95 | Shift+G | Shift+G | Pierce constraint | Sketch |
| KB-96 | Shift+Q | Shift+Q | Symmetric constraint | Sketch |
| KB-97 | T * | T | Tangent constraint | Sketch |
| KB-98 | Shift (hold) | Shift (hold) | Suppress automatic inferences while moving or placing sketch entities | Sketch |

#### Feature Studio

| ID | Keys (Win/Linux) | Mac | Action | Context |
|---|---|---|---|---|
| KB-99 | Ctrl+] | ⌘+] | Return cursor to its previous position in navigation history (Back reference) — as documented | Feature Studio |
| KB-100 | Ctrl+[ | ⌘+[ | Return cursor to its next position in navigation history (Forward reference) — as documented (bracket direction appears swapped vs. common editors; verify) | Feature Studio |
| KB-101 | Ctrl+S | ⌘+S | Commit current Feature Studio (updates other tabs) | Feature Studio |
| KB-102 | Ctrl+Shift+S | ⌘+Shift+S | Commit all Feature Studios with uncommitted changes | Feature Studio |
| KB-103 | Esc | Esc | Dismiss autocomplete | Feature Studio |
| KB-104 | Ctrl+Shift+O | ⌘+Shift+O | Open Top level symbols (outline) dropdown | Feature Studio |

#### Drawings

| ID | Keys (Win/Linux) | Mac | Action | Context |
|---|---|---|---|---|
| KB-105 | Alt+LMB drag | ⌥+drag | Copy selected annotation(s) and place where dragged | Drawing |
| KB-106 | Shift+D | Shift+D | Diameter dimension tool | Drawing |
| KB-107 | Shift+H | Shift+H | Foreshorten dimension tool | Drawing |
| KB-108 | Shift+R | Shift+R | Place a radial dimension on an arc or circle | Drawing |
| KB-109 | Ctrl+M | Ctrl+M | Maximum or minimum dimension tool | Drawing |
| KB-110 | Shift+Q | Shift+Q | Activate midpoint and quadrant points on arcs/circles for selection during dimension commands | Drawing (dimension active) |
| KB-111 | N | N | Note tool | Drawing |
| KB-112 | P | P | Projected view tool | Drawing |
| KB-113 | Ctrl+S | Ctrl+S | Hide or show Sheets panel | Drawing |
| KB-114 | Home | Home (Fn+←) | Open first sheet | Drawing |
| KB-115 | End | End (Fn+→) | Open last sheet | Drawing |
| KB-116 | Page Up | Page Up (Fn+↑) | Open previous sheet | Drawing |
| KB-117 | Page Down | Page Down (Fn+↓) | Open next sheet | Drawing |
| KB-118 | Ctrl+Q | Ctrl+Q | Update all views and properties on each sheet | Drawing |

### KB-119 Assignable actions without default keys
- **Purpose:** actions that exist in the command registry (bindable in preferences) but have no documented default key.
- **Sources:** bundle-command-actions.txt [BUNDLE] (ids), cross-checked against the documented defaults above.
- **List (by group):**
  - General/tools: Deviation analysis, Connection analysis, Flatten surfaces, Interference detection, Zebra stripes, Curvature color map, Draft analysis, Reflection analysis, Thickness analysis, Mass properties, Flip secondary axis, Copy/Paste occurrences (instances), Create version, Expand all / Collapse all feature folders, Open contact support, Open debug panel, Element tab switcher.
  - Part Studio features: Sweep, Loft, Thicken, Enclose, Face blend, Chamfer, Draft, Body draft, Rib, Shell, Hole, External thread, Linear/Circular/Curve pattern, Mirror, Boolean, Split, Transform, Wrap, Decal, Delete part, Modify fillet, Delete face, Move face, Replace face, Plane, Offset surface, Boundary surface, Fill, Move boundary, Ruled surface, Mutual trim, Constrained surface, Helix, 3D fit spline, Projected/Bridging/Composite/Intersection/Trim/Isocline/Offset/Isoparametric/Edit/Routing curve, Derived, Variable, Query variable, Composite part, Frame, Frame trim, Gusset, End cap, Cut list, Tag profile, Sheet metal model, Flange, Hem, Tab, Bend, Jog, Form, Sheet metal loft, Make joint, Corner, Bend relief, Finish sheet metal, Corner break, Add custom feature.
  - Assembly: Mate (generic), Tangent mate, Width mate, Group, Replicate, Replace instance, Linear/Circular pattern, Mirror, Mate relation, Display states, Create Part Studio in context.
  - 3D View: Dimetric, Trimetric, Perspective view, Shaded with edges, Shaded without edges, Unshaded, Translucent, Hidden edges removed, Tangent edges visible/phantom/hidden, Highlight boundary edges, Previous view, Graphics preferences (zoom in/out and pan/rotate actions also exist as ids).
  - Sketch: Midpoint line, Aligned rectangle, 3 point circle, Ellipse, Tangent arc, Center point arc, Elliptical arc, Conic, Inscribed/Circumscribed polygon, Spline, Bezier, Spline control point, Text, Intersection, Split, Slot, Mirror, Linear/Circular pattern, Transform, Insert DXF/DWG, Insert image, Sketch profile inspector, Chamfer, Constraint manager, Curvature G3.
- **OCCT mapping:** app action registry.
- **Priority:** P2.

### KB-120 Default mouse navigation (Onshape preset)
- **Purpose:** 3D camera control with mouse.
- **Sources:** View/view_navigation_and_the_view_cube.htm [DOCS]; bundle onboarding template "Set your mouse controls" [BUNDLE].
- **Mappings (Windows and macOS identical unless noted):**

  | Action | Input |
  |---|---|
  | 3D rotate | Right mouse button drag |
  | Turntable rotate (Windows doc) | Alt + RMB drag: horizontal motion orbits about the model, vertical motion pitches over the model |
  | Level the view | Alt key (during rotate, Windows doc) animates to the nearest "floor down" view (nearest view without roll) |
  | 2D pan | Ctrl + RMB drag, or middle mouse button drag |
  | Zoom | Scroll wheel up = zoom in, down = zoom out (direction reversible in preferences); zoom centers on cursor (inferred) |
  | Zoom to fit | Double-click scroll wheel |
  | Select | Left click (toggle) |
  | Box select | Left drag in graphics area (direction-dependent, see Box selection) |
  | Context menu | Right click without drag (inferred: RMB release without motion beyond a few pixels) |
  | View cube arrows | Click = 15°, Shift+click = 90°, Ctrl+click = 5° |
  | View cube face / corner | Plane view / trimetric view |

- **Behavior:** rotation center = model/selection center (inferred; not documented). Zoom not supported on all browser/device combinations (web-only caveat).
- **OCCT mapping:** app camera controller (arcball/trackball + constrained turntable).
- **Priority:** P0.

### KB-121 Mouse control presets (other CAD systems)
- **Purpose:** let users keep their muscle memory.
- **Sources:** Plans/my_account_preferences.htm ("Keep Onshape's default settings for mouse mappings, or select another CAD system's default settings. These settings also control mouse mappings for Drawings.") [DOCS]; preset templates `isViewSettingOnshape/SW/NX/Creo/AutoCAD` with labels [BUNDLE].
- **Presets:**

  | Preset | Rotate | Pan | Zoom |
  |---|---|---|---|
  | Onshape (default) | RMB drag | Ctrl+RMB drag or MMB drag | Scroll wheel |
  | SOLIDWORKS | MMB drag | Ctrl+MMB drag | Scroll wheel or Shift+MMB drag |
  | NX | MMB drag | Shift+MMB drag | Scroll wheel or Ctrl+MMB drag |
  | Creo | MMB drag | Shift+MMB drag | Scroll wheel or Ctrl+MMB drag |
  | AutoCAD | Shift+MMB drag | MMB drag | Scroll wheel |

  (Bundle also contains a combined pan string "Ctrl + Right mouse button drag or Middle mouse button drag or Right mouse button drag or Ctrl + Middle mouse button drag", likely used for 2D drawing pan where rotate is not applicable — inferred.)
- **Other mouse preferences:** Reverse scroll wheel zoom direction; Set default rotation behavior to constrained rotate (bundle modes "Rotate", "Constrained rotate", "Precision rotate", "Coarse rotate" — constrained = turntable keeping world Z up, inferred); Use pen input as mouse (stylus treated as mouse, not touch). Save with "Save mouse controls" (toasts "Mouse controls updated." / "Failed to update mouse controls."). 3Dconnexion devices incl. SpaceMouse supported (via vendor driver).
- **OCCT mapping:** app input mapping table keyed by preset; 3D mouse via 3Dconnexion SDK / `libspnav` (Linux, open) — design suggestion.
- **Priority:** P0 (Onshape + SOLIDWORKS presets), P1 (others, 3D mouse).

### KB-122 Touchpad and touchscreen navigation
- **Purpose:** trackpad gestures.
- **Sources:** View/view_navigation_and_the_view_cube.htm [DOCS].
- **Mappings:**

  | Action | Windows touchpad | macOS trackpad |
  |---|---|---|
  | 3D rotate | Right touchpad button click, then drag | Two-finger click, then drag |
  | Zoom | Two-finger press, pinch in = zoom in, pinch out = zoom out (as documented) | Two-finger scroll down = zoom out, up = zoom in |
  | 2D pan | Ctrl + right touchpad button click, then drag | Hold Ctrl + two-finger click, then drag |

  Touchscreen (iOS/Android/browser touch): tap select, double-tap clear selection, one-finger drag rotate, two-finger tap context menu, two-finger long press box select, pinch zoom, two-finger drag pan, two-finger rotate = 2D rotate, precision selector on touch-and-hold, 3D Rotate Lock button — mobile is out of scope for noshape (listed for completeness).
- **OCCT mapping:** GPUI gesture events (pinch/magnify, scroll with phase) → camera.
- **Priority:** P1.

### KB-123 Selection model (click, toggle, clear, ranges)
- **Purpose:** core selection semantics in graphics area and lists.
- **Sources:** Home/selection.htm, Home/user_interface_basics.htm [DOCS].
- **Behavior:**
  - Click = toggle: click to select, click again to deselect; clicking other entities adds to the selection (no modifier needed — equivalent to Ctrl+click elsewhere).
  - Cursor shows a selection count badge (accurate up to 5, then "5+").
  - Clear: click empty space, Space, or context menu "Clear selection".
  - Lists (Feature list, Parts list, etc.): Shift+click selects a contiguous range.
  - Toolbar tools also toggle: click again, context menu "Exit <tool>", or Esc.
  - Selectable entity kinds: sketch curves, sketch regions, faces, edges, vertices, midpoints (hover-visualized midpoints of sketch entities and model edges when not editing a sketch/feature; usable for measuring, plane creation, Use in sketch — midpoints of inactive sketches can be selected for Use while sketching), parts/bodies, planes, mate connectors, origin.
  - Highlight colors per MT-45.
- **OCCT mapping:** picking by GPU ID buffer (entity id per triangle/edge/vertex render) or CPU BVH ray pick (`BVH` over tessellation; OCCT `SelectMgr_ViewerSelector` is the AIS reference); topology id → app entity (body/face/edge/vertex) map.
- **Priority:** P0.

### KB-124 Box selection (window vs crossing) and box deselect
- **Purpose:** area selection.
- **Sources:** Home/selection.htm, View/interference_detection.htm, Home/colors.htm [DOCS].
- **Behavior:**
  - Drag left-to-right: window select — only entities entirely inside the box; solid blue outline (#789ad1), blue fill (#d0dcef).
  - Drag right-to-left: crossing select — any entity the box touches; dotted orange/yellow outline (#ffdc74), pale fill (#fff3cf).
  - (Interference detection docs phrase it as top-left→lower-right = fully enclosed; lower-right→top-left = touched; horizontal direction is what matters per Selection page.)
  - Ctrl + drag across entities = deselect them.
  - Works in Part Studios and Assemblies, also inside analysis dialogs and Hide/Show.
  - Which entity types a box selects depends on context (inferred: in Part Studio idle → parts/sketch entities; in sketch → sketch entities; in dialogs → the active field's filter).
- **OCCT mapping:** frustum selection against mesh bounding volumes + exact containment test of tessellated geometry (OCCT reference: `SelectMgr_SelectingVolumeManager` rectangle selection, "overlap" vs "inside" policies).
- **Priority:** P0.

### KB-125 Pre-highlight (hover) behavior
- **Purpose:** hover feedback before selection.
- **Sources:** Home/selection.htm, Home/colors.htm [DOCS]; bundle constants `PREHIGHLIGHT_SPEED_THRESHOLD=.15`, `PAUSED_MOUSE_PREHIGHLIGHT_TIMEOUT_MULTIPLIER=5`, `PAUSED_MOUSE_PREHIGHLIGHT_TIMEOUT_MAX_MS=250`, `PRE_HIGHLIGHT_COLOR_BORDER=rgba(255,198,133,1)` [BUNDLE].
- **Behavior:** entity under cursor highlighted (orange border); pre-highlight suppressed while the mouse moves fast (speed threshold) and appears after a short pause (≤250 ms) (inferred from constants). Hovering a dialog's selection list item cross-highlights the entity; hovering an entity highlights list rows.
- **OCCT mapping:** throttled pick on mouse-move.
- **Priority:** P1.

### KB-126 Select other (grave accent key)
- **Purpose:** pick entities hidden behind others without rotating.
- **Sources:** Home/select_other.htm [DOCS]; bundle `SELECT_OTHER` [BUNDLE].
- **Behavior:** hover/select an entity then press the grave accent key `` ` `` (or context menu "Select other"): a list of all faces/edges under the cursor ordered from the highlighted one into the depth. Navigate with mouse, or grave = down / Shift+grave = up; Enter selects; dialog closes on selection. Selections flow into a subsequently opened feature dialog; also works with a feature dialog already open. Also usable cumulatively with Isolate/Make transparent. Key per layout: `` ` `` (US), ' on the @ key (UK), @ (French), ^ (German), º (Spanish), Alt+\ (Italian Mac), ò (Italian Windows).
- **OCCT mapping:** ray pick returning all hits sorted by depth (`IntCurvesFace_ShapeIntersector` on B-rep or BVH all-hits on mesh, plus edges within pixel tolerance).
- **Priority:** P1.

### KB-127 Selecting through transparency
- **Purpose:** reach geometry beneath transparent parts.
- **Sources:** shortcuts page (Alt+click), PartStudio/customizing_appearances.htm, View/isolate.htm [DOCS].
- **Behavior:** parts transparent through appearance opacity: hold Alt while clicking to select entities beneath. Isolate/Make transparent: "Select transparent geometry" checkbox decides whether transparent geometry is pickable.
- **OCCT mapping:** pick filter skipping entities with alpha < 1 unless Alt held.
- **Priority:** P1.

### KB-128 Selection in dialogs and pre-selection
- **Purpose:** how selections feed feature/tool dialogs.
- **Sources:** Home/dialogs.htm, Home/create_selection.htm, View/analysis_tools.htm [DOCS].
- **Behavior:**
  - Selection fields highlighted solid blue = filled by graphics/list selection; blue outline = keyboard input. Clicking a field sets focus; the active field receives picks and filters by its allowed entity types (FS `Filter`), with a grey hint line describing the expected type. States shown: healthy, suppressed-before-open, suppressed-after-open (missing).
  - Entities preselected before opening a feature populate its first (matching) selection field. Select other / Create selection results transfer as well.
  - Enter accepts; Shift+Enter accepts and reopens the same feature empty; dialog-internal actions are removed from Undo/Redo once accepted/cancelled.
  - Selection lists auto-expand; manual resize disables auto-expansion until resized long again. Dialog movable/resizable.
  - Analysis tools: opened from menu = fresh selection; via shortcut = previous selections persist (preselection replaces them).
  - Feature dialog field context: "Create selection" arrow icon opens smart selection (MT-42).
- **OCCT mapping:** app; filter predicates map to `TopAbs_ShapeEnum` + `GeomAbs_SurfaceType/CurveType` + body type checks.
- **Priority:** P0.

### KB-129 Selection filters, chain selection and query-based selection
- **Purpose:** clarify what exists for filtered/chained selection.
- **Sources:** Home/create_selection.htm, PartStudio/query_variable.htm (referenced), shortcuts page [DOCS].
- **Behavior:**
  - No global selection-filter toolbar is documented (unlike SOLIDWORKS); type filtering is implicit through the active dialog field's Filter (inferred).
  - Chain/loop/tangent selection: context menu "Select >" (Loop/chain connected edges, Tangent connected, Parallel, Equal length/radius; face rules Protrusion/Pocket/Hole/Fillets/Tangent connected/Bounded faces) — see MT-42. No double-click chain selection is documented.
  - Query-based selection: Query variable feature (spec in VAR- section) stores a selection rule reusable in features.
  - Long-press: touch-only (precision selector); no long-press selection with mouse.
- **OCCT mapping:** MT-42 algorithms.
- **Priority:** P1.

### KB-130 Search tools (command search)
- **Purpose:** find and launch any tool/feature by name.
- **Sources:** Home/search_tools.htm [DOCS]; bundle `OPEN_COMMAND_SEARCH` [BUNDLE]. Shortcut Alt/⌥+C.
- **Behavior:** button right of the toolbar (Part Studio, Assembly, Sketch; not Document or Drawing toolbars). Empty query shows recently used tools. Typing shows suggestions/results on the left (incl. vetted custom features); hover shows description on the right; click launches. Tools removed from the toolbar appear under "Removed from toolbar". Option "Highlight tool in toolbar" (checkbox bottom-right) highlights hovered tool's toolbar icon. x clears; Esc or x closes; selecting a toolbar tool closes it. Toolbar context menu: Hide/Show "Search tools" button.
- **OCCT mapping:** app fuzzy matcher over action registry.
- **Priority:** P1.

### KB-131 Shortcut toolbar (S key)
- **Purpose:** cursor-local mini toolbar.
- **Sources:** shortcuts page, Plans/my_account_preferences.htm (Shortcut toolbars) [DOCS]; bundle `OPEN_MINI_TOOLBAR` [BUNDLE].
- **Behavior:** S opens a toolbar near the mouse pointer; Esc closes. Separate customizable toolbars for Sketch tools, Feature tools, Assembly tools, Drawing tools (Preferences: check tools to include; order = list order; unlimited tools; once customized, user's list overrides future default changes; "Reset to defaults" for main toolbars does not reset shortcut toolbars). Main toolbars: last used tool in a dropdown group moves to the group top (reset on reopen); toolbars collapse into dropdowns at low resolution.
- **OCCT mapping:** app.
- **Priority:** P1.

### KB-132 Triad manipulator
- **Purpose:** direct 3D drag/rotate of instances (assemblies) and of geometry in transform-like dialogs.
- **Sources:** Home/triad_manipulator.htm [DOCS].
- **Behavior:**
  - Assembly instance: free drag (click-drag the part) or manipulator drag (click to show triad). Fixed instances: no triad, drag shows a "fixed" cue.
  - Triad origin (center ball): drag to move the manipulator only (not the part); snaps to inferred/explicit mate connectors; then manipulations are relative to that point. Right-click origin: "Move to origin" (moves part so the reference point is at origin; no mate/fix).
  - Axis arrows: drag translates along axis; right-click: "Align with Z", "Anti-align with Z".
  - Plane squares: drag translates within that plane.
  - Rotation arcs: drag rotates about triad X/Y/Z; right-click: "Rotate 90 degrees", "Rotate 180 degrees".
  - While dragging a numeric field appears (distance or angle) for exact entry.
  - Unmated, unfixed instances move exactly; mated instances move within their DOF (solver may fail to find a solution; try another handle/position).
  - Section view manipulator uses the same idiom (ball/arrow/angle indicators).
- **OCCT mapping:** app gizmo rendering + ray/plane intersection math; mates via the assembly solver (AS- section).
- **Priority:** P0.

### KB-133 Numeric field input behaviors
- **Purpose:** keyboard/mouse behaviors of numeric inputs (full expression grammar is in section 1, DOC-).
- **Sources:** Home/numeric_fields.htm [DOCS].
- **Behavior:**
  - Scroll wheel over a numeric field: default ±0.1; Ctrl+wheel ±0.01; Shift+wheel ±1.0. For fields bounded 0..1: wheel ±1.0, Ctrl+wheel ±0.01, Shift+wheel no effect.
  - Accepts integers, decimals (comma decimal separator if enabled; then array values need ", " spacing), fractions, units (mm, millimeter, cm, centimeter, m, meter, in, inch, ft, foot, yd, yard; deg, degree, rad, radian; plurals OK), operators + − * / ^ %, functions ceil floor round exp sqrt abs max min log log10, trig (degrees by default) cos sin tan acos asin atan atan2 cosh sinh tanh asinh acosh atanh, constants pi/PI/Pi, variables (#name), arrays with index `[3,5,6,7][2]`, ternary `?:`, logic `&&` `||`, comparisons.
  - Result must be unitless or a unit to the first power; no mixing incompatible units (3+2.5in invalid). After accept the evaluated value is shown in default units; on refocus the original expression is shown. Invalid → red outline, accept disabled.
  - Default unit applies when none typed.
- **OCCT mapping:** app expression parser/evaluator (shared with variables).
- **Priority:** P0.

### KB-134 Context menu invocation and hover-targeted commands
- **Purpose:** mouse/keyboard access to context commands.
- **Sources:** Home/context_menus.htm, View/hide_show.htm, View/isolate.htm [DOCS].
- **Behavior:** RMB click on entity/list item/tab/blank space opens a context-sensitive sectioned menu (MT-43). Y / Shift+Y / Shift+I / Shift+T operate on the entity under the cursor (highlighted on hover) or on the current selection; repeated use is cumulative for Isolate/Make transparent. N uses hovered or preselected face/plane.
- **OCCT mapping:** app.
- **Priority:** P0.

### KB-135 Undo / redo scope
- **Purpose:** semantics of Ctrl+Z / Ctrl+Y.
- **Sources:** Home/user_interface_basics.htm, Home/dialogs.htm, shortcuts page [DOCS].
- **Behavior:** undo the last successful action, redo the last undone action; available per user, per tab, per session; repeatable. Actions performed inside an open dialog are dropped from the undo list when the dialog is accepted or cancelled (the feature commit is one undo step).
- **OCCT mapping:** app command stack over the document model (document history is separate; see DOC- section).
- **Priority:** P0.
