| MT-01 | Measure tool (overview, UI and interaction) | P0 | GPUI panel; worker-thread recompute on selection change |
| MT-02 | Measure: results by selection combination | P0 | BRepExtrema_DistShapeShape (min), custom sampled+BFGS max, GCPnts_AbscissaPoint, BRepGProp, BRepAdaptor_Curve/Surface |
| MT-03 | Measure: units, precision, copying values, visualization | P0 | UI; Precision::Confusion/Angular for tolerance star |
| MT-04 | Measure: reference coordinate system | P1 | gp_Trsf from mate connector gp_Ax3 (inverse) |
| MT-05 | Measure: create variable from measurement | P1 | Measured variable evaluated at regen using MT-02 computations |
| MT-06 | Measure: assemblies (dynamic update), sheet metal flat view | P1 | cached BRepExtrema per part + TopLoc_Location per frame |
| MT-07 | Measure in drawings | P1 | Geom2dAPI_ExtremaCurveCurve on HLRBRep output |
| MT-08 | Mass properties tool — Part tab | P0 | BRepGProp::VolumeProperties + GProp_GProps::Add(density), MatrixOfInertia, GProp_PrincipalProps |
| MT-09 | Mass properties tool — Face (section) tab | P1 | BRepGProp::SurfaceProperties/LinearProperties on (section) faces; in-plane inertia transform |
| MT-10 | Mass properties: materials, override, calculation variance, center-of-mass display, copy | P1 | error estimate from BRepGProp Eps as variance; app override field |
| MT-11 | Section view | P0 | GPU clip planes + stencil capping; BRepAlgoAPI_Section / Cut with BRepPrimAPI_MakeHalfSpace for exact cut |
| MT-12 | Named views | P1 | app data model (camera + section state per workspace) |
| MT-13 | View cube & standard views | P0 | app camera presets; custom view cube widget (AIS_ViewCube as reference) |
| MT-14 | Isometric, dimetric, trimetric projections | P0 | app camera presets |
| MT-15 | Perspective view | P1 | app perspective projection |
| MT-16 | Zoom to fit, zoom to window, zoom to selection, zoom in/out | P0 | BRepBndLib::Add/AddOptimal bounding boxes -> camera fit |
| MT-17 | Previous view | P2 | app camera history stack |
| MT-18 | View normal to / Orient normal to sketch on edit | P0 | BRepAdaptor_Surface plane normal -> camera |
| MT-19 | Render modes: Shaded with edges, Shaded without edges, Unshaded, Translucent | P0 | BRepMesh_IncrementalMesh + renderer passes (OIT for translucent) |
| MT-20 | Hidden edges visible / removed | P1 | GPU edge pass with inverted depth test, dashed |
| MT-21 | Tangent edges visible / phantom / removed | P1 | BRep_Tool::Continuity / normal comparison to flag tangent edges |
| MT-22 | Camera and render option defaults | P2 | user preferences store |
| MT-23 | View in high quality, tessellation levels and display refinement | P1 | BRepMesh_IncrementalMesh LOD levels + disk cache + screen-size LOD selection |
| MT-24 | Graphics preferences | P2 | renderer uniforms, SSAO pass |
| MT-25 | Highlight boundary edges | P1 | TopExp::MapShapesAndAncestors free-edge count / ShapeAnalysis_FreeBounds |
| MT-26 | Hide / Show | P0 | app visibility flags |
| MT-27 | Isolate | P1 | renderer per-part alpha; BRepExtrema/AABB proximity ordering; mate-graph BFS |
| MT-28 | Make transparent | P1 | same as Isolate (inverse) |
| MT-29 | Reference geometry visibility toggles | P1 | renderer filters |
| MT-30 | Analysis tools menu (common behavior) | P1 | app analysis-tool framework |
| MT-31 | Curve/surface analysis | P1 | GeomLProp_CLProps combs, Geom_Surface::UIso/VIso, BSpline Poles/Knots, Geom2dLProp_CurAndInf2d |
| MT-32 | Deviation analysis (curve and surface) | P2 | GeomAPI_ProjectPointOnCurve/Surf sampling + math_BrentMinimum; color-map shader |
| MT-33 | Connection analysis | P2 | GeomLProp_CLProps D0-D3 / BRepLProp_SLProps along shared edge; BRep_Tool::Continuity |
| MT-34 | Dihedral analysis | P2 | BRepLProp_SLProps normals of adjacent faces along edge samples |
| MT-35 | Interference detection | P1 | Bnd_OBB/BVH broad phase + BRepAlgoAPI_Common + BRepGProp volume; BRepExtrema_ShapeProximity |
| MT-36 | Zebra stripes | P2 | fragment shader stripes on exact normals |
| MT-37 | Reflection analysis | P2 | env-map shader |
| MT-38 | Curvature color map | P2 | GeomLProp_SLProps curvatures per mesh vertex + LUT shader |
| MT-39 | Draft analysis | P2 | normal vs pull-direction shader + BVH ray cast for undercuts |
| MT-40 | Thickness analysis | P2 | custom: BVH ray casting (ray method), shrinking-ball medial axis (rolling ball) |
| MT-41 | Flatten surfaces | P2 | custom LSCM/ARAP mesh flattening; GeomAPI_PointsToBSpline + BRepBuilderAPI_MakeFace for output |
| MT-42 | Create selection and Select > (smart selection) | P1 | TopExp adjacency + BRepOffset_Analyse convexity; app pattern history |
| MT-43 | Context menus (graphics area) | P0 | GPUI context menus |
| MT-44 | Appearance (color and opacity per part, face, feature, sketch, curve) | P0 | app appearance attributes; XCAFDoc_ColorTool/VisMaterialTool on export |
| MT-45 | UI color conventions | P1 | theme constants |
| MT-46 | Print / Download image | P2 | offscreen render -> PNG / OS print API |
| IO-01 | Import/export model overview | P0 | XCAF TDocStd_Document -> app Part Studio/Assembly; file tabs as blobs |
| IO-02 | Format support matrix (authoritative list from Supported File Formats) | P0 | format registry (see per-format rows) |
| IO-03 | Import dialog: processing options | P0 | BRepBuilderAPI_Sewing, ShapeFix_*, BRepCheck_Analyzer, gp_Trsf Y-up, XCAFDoc_MaterialTool |
| IO-04 | Import feature (`importForeign`) | P0 | reader -> TopoDS bodies + XCAF names/colors; scale via BRepBuilderAPI_Transform for meshes |
| IO-05 | Assembly / multi-file import (zip rules) | P2 | STEPCAFControl_Reader external refs from zip (design) |
| IO-06 | Updating an imported file | P1 | custom geometric re-matching of faces/edges for persistent naming |
| IO-07 | Repair panel (View in repair, Edit healthy moment, Replace reference) | P2 | app: evaluate past microversion in second context; reference remap |
| IO-08 | Working with imported CAD (faults, fractures, direct editing) | P1 | ShapeFix_Shape, ShapeAnalysis_FreeBounds, BRepBuilderAPI_Sewing, BRepAlgoAPI_Defeaturing |
| IO-09 | Mesh import and mesh referencing | P1 | triangulation-only TopoDS_Face (Poly_Triangulation); RWStl/RWObj/RWGltf readers, lib3mf |
| IO-10 | Export dialog (parts, Part Studios, instances, assemblies) | P0 | IMeshTools_Parameters (Deflection/Angle/MinSize); per-format writers; gp_Trsf Y-up |
| IO-11 | Export naming, export rules, multi-tab export | P2 | app templating over metadata |
| IO-12 | Sketch, planar face and flat-pattern export to DXF/DWG | P1 | 2D projection of face/sketch edges; custom DXF writer; LibreDWG/ODA for DWG |
| IO-13 | Drawing export | P0 | HLR output -> pdf-writer/svg/image crates; custom DXF |
| IO-14 | Drawing import (DWG/DXF/DWT) | P2 | libdxfrw (GPL) or custom DXF reader; LibreDWG/ODA for DWG |
| IO-15 | Download and non-CAD files | P2 | app file tabs |
| IO-16 | Parasolid (.x_t / .x_b; mesh and mixed models) | P2 | commercial only (Parasolid / OCC component); defer, use STEP |
| IO-17 | STEP (.step / .stp) | P0 | STEPCAFControl_Reader/Writer (write.step.schema/unit), ShapeFix/ShapeUpgrade/ShapeCustom preprocessing |
| IO-18 | IGES (.igs / .iges) | P1 | IGESCAFControl_Reader/Writer, write.iges.brep.mode for compatibility |
| IO-19 | ACIS (.sat) | P2 | commercial (Spatial/OCC ACIS component) |
| IO-20 | STL | P0 | BRepMesh_IncrementalMesh + StlAPI_Writer/RWStl (ascii/binary); RWStl::ReadFile |
| IO-21 | OBJ | P1 | RWObj_CafWriter/RWObj_CafReader |
| IO-22 | glTF / GLB | P1 | RWGltf_CafWriter (GLB, Draco) / RWGltf_CafReader |
| IO-23 | 3MF | P1 | lib3mf (BSD) or custom zip+XML writer |
| IO-24 | JT | P2 | commercial (JT Open / OCC JT component) |
| IO-25 | Rhino (.3dm) | P2 | openNURBS <-> Geom_BSplineSurface conversion (custom) |
| IO-26 | PVZ (Creo View) | P2 | infeasible (proprietary PTC format) |
| IO-27 | URDF (assembly export) | P2 | custom URDF XML + mesh writers + BRepGProp link inertia |
| IO-28 | Proprietary native CAD imports (CATIA V5, SOLIDWORKS, Inventor, Pro/E-Creo, NX, Solid Edge) | P2 | infeasible open; optional commercial SDK plug-in; recommend STEP AP242 |
| IO-29 | DWG / DXF / DWT (2D) | P1 | custom DXF writer; libdxfrw/LibreDWG (GPL) or ODA (commercial) for DWG |
| IO-30 | PDF / SVG / PNG / JPEG / CSV outputs | P0 | pdf-writer/printpdf, svg, image crates |
| IO-31 | Flattened-surface export | P2 | StlAPI_Writer; custom DXF/SVG; STEP/BREP instead of Parasolid |
| IO-32 | noshape native exchange format (design suggestion, not in Onshape) | P1 | BRepTools/BinTools Write/Read; BinXCAFDrivers XBF |
| IO-33 | Lidar scan import (mobile) | P2 | out of scope (mobile); use GLB/OBJ mesh import |
| KB-01 | Keyboard shortcuts map (in-app reference) | P1 | GPUI modal generated from action registry |
| KB-02 | Customizing keyboard shortcuts | P1 | action registry + user override map; per-context conflict detection |
| KB-03 | Shortcut tables: conventions and context dispatch | P0 | keymap with context stack (text field > sketch > tab type > 3D view > general) |
| KB-04 | General (feature/tool dialogs): Commit the command and immediately open another (empty) dialog of the same command (Shift+Enter) | P0 | app action registry + keymap (context-dispatched) |
| KB-05 | General: Commit (accept) the command in use (Enter) | P0 | app action registry + keymap (context-dispatched) |
| KB-06 | General: Exit/cancel the command in use (Esc) | P0 | app action registry + keymap (context-dispatched) |
| KB-07 | General: Deselect selected feature(s)/entities (clear selection) (Space) | P0 | app action registry + keymap (context-dispatched) |
| KB-08 | General: Copy selected feature (also copies a selected Measure/Mass value) (Ctrl+C) | P0 | app action registry + keymap (context-dispatched) |
| KB-09 | General: Paste copied feature (Ctrl+V) | P0 | app action registry + keymap (context-dispatched) |
| KB-10 | General: Undo recent activity in the document session (repeatable (Ctrl+Z) | P0 | app action registry + keymap (context-dispatched) |
| KB-11 | General: Redo recently undone activity (repeatable) (Ctrl+Y) | P0 | app action registry + keymap (context-dispatched) |
| KB-12 | General: Remove selected features/entities (Delete / Backspace) | P0 | app action registry + keymap (context-dispatched) |
| KB-13 | General (Sketch, Part Studio, Assembly): Open/close Curve/surface analysis (in sketch: auto-selects all sketch curves (Shift+C) | P1 | app action registry + keymap (context-dispatched) |
| KB-14 | General (Sketch, Part Studio): Open/close Dihedral analysis (selections persist) (Shift+D) | P1 | app action registry + keymap (context-dispatched) |
| KB-15 | General / Feature Studio: FeatureScript search: find text in the current Feature Studio (Ctrl+Shift+F) | P1 | app action registry + keymap (context-dispatched) |
| KB-16 | General: Feedback / report a bug (out of scope for noshape) (Ctrl+U) | P2 | app action registry + keymap (context-dispatched) |
| KB-17 | General (dialog with axis): Flip primary axis (flips the Z axis when editing, e.g. mate connector orientation) (A) | P0 | app action registry + keymap (context-dispatched) |
| KB-18 | General: Show or hide mate connectors (K) | P1 | app action registry + keymap (context-dispatched) |
| KB-19 | General: Toggle the Keyboard shortcuts map (Shift+/) | P1 | app action registry + keymap (context-dispatched) |
| KB-20 | General (Assembly, mate connector): Lock mate inference (while placing a mate connector / mating) (Shift (hold)) | P1 | app action registry + keymap (context-dispatched) |
| KB-21 | General (Part Studio, Assembly): Open the Mate connector dialog (Ctrl+M) | P1 | app action registry + keymap (context-dispatched) |
| KB-22 | General (Part Studio, Assembly, Drawing): Open the Measure dialog ([) | P0 | app action registry + keymap (context-dispatched) |
| KB-23 | Documents page: Open the selected document in a new browser tab (noshape: new tab/window) (Ctrl+click (document)) | P2 | app action registry + keymap (context-dispatched) |
| KB-24 | Documents page: Open the selected document in a new browser window (Shift+click (document)) | P2 | app action registry + keymap (context-dispatched) |
| KB-25 | General: Switch between recently opened tabs in the session (drawings, assemblies, Part Studios) (Ctrl+Space) | P1 | app action registry + keymap (context-dispatched) |
| KB-26 | General: Prompt for a new name of the selected instance or feature (rename) (Shift+N) | P1 | app action registry + keymap (context-dispatched) |
| KB-27 | General / Sketch: Select/exit Construction tool (requires active sketch (Q) | P0 | app action registry + keymap (context-dispatched) |
| KB-28 | General (Part Studio, Assembly, Sketch): Open Search tools (top right) with cursor in the search field (Alt+C) | P1 | app action registry + keymap (context-dispatched) |
| KB-29 | General (Sketch, Part Studio, Assembly): Select other: list of entities selectable relative to cursor position (through depth) (`` ` `` (grave accent)) | P1 | app action registry + keymap (context-dispatched) |
| KB-30 | General (Select other open): Select other: cycle back up the list (Shift+`` ` ``) | P1 | app action registry + keymap (context-dispatched) |
| KB-31 | General: Open the shortcut toolbar at the mouse cursor (S) | P1 | app action registry + keymap (context-dispatched) |
| KB-32 | General: Open the Tab Manager (Alt+T) | P1 | app action registry + keymap (context-dispatched) |
| KB-33 | Part Studio: Extrude (from open sketch uses it (Shift+E) | P0 | app action registry + keymap (context-dispatched) |
| KB-34 | Part Studio: Fillet (Edge fillet dialog) (Shift+F) | P0 | app action registry + keymap (context-dispatched) |
| KB-35 | Part Studio: Show or hide sketches (Shift+H) | P1 | app action registry + keymap (context-dispatched) |
| KB-36 | Part Studio: Move the rollback bar up/down through the Features list (↑ / ↓ (rollback bar selected)) | P1 | app action registry + keymap (context-dispatched) |
| KB-37 | Part Studio: Revolve (from open sketch (Shift+W) | P0 | app action registry + keymap (context-dispatched) |
| KB-38 | Part Studio: New Sketch (prompts "Select a sketch plane") (Shift+S) | P0 | app action registry + keymap (context-dispatched) |
| KB-39 | Assembly: Show or hide mates (J) | P1 | app action registry + keymap (context-dispatched) |
| KB-40 | Assembly: Open Insert parts and assemblies dialog (I) | P1 | app action registry + keymap (context-dispatched) |
| KB-41 | Assembly: Open Fasten mate dialog (M) | P1 | app action registry + keymap (context-dispatched) |
| KB-42 | Assembly: Toggle Show mates mode (toolbar) (H) | P1 | app action registry + keymap (context-dispatched) |
| KB-43 | Assembly: Toggle Snap mode (toolbar) (Shift+S) | P1 | app action registry + keymap (context-dispatched) |
| KB-44 | 3D View: Front view (Shift+1) | P0 | app action registry + keymap (context-dispatched) |
| KB-45 | 3D View: Back view (Shift+2) | P0 | app action registry + keymap (context-dispatched) |
| KB-46 | 3D View: Left view (Shift+3) | P0 | app action registry + keymap (context-dispatched) |
| KB-47 | 3D View: Right view (Shift+4) | P0 | app action registry + keymap (context-dispatched) |
| KB-48 | 3D View: Top view (Shift+5) | P0 | app action registry + keymap (context-dispatched) |
| KB-49 | 3D View: Bottom view (Shift+6) | P0 | app action registry + keymap (context-dispatched) |
| KB-50 | 3D View (also Repair panel): Isometric view (Shift+7) | P0 | app action registry + keymap (context-dispatched) |
| KB-51 | 3D View: View normal to preselected/hovered plane or face (screen-parallel) (N) | P0 | app action registry + keymap (context-dispatched) |
| KB-52 | 3D View, Drawing, Repair panel: Zoom to fit (largest view showing the whole design) (F) | P0 | app action registry + keymap (context-dispatched) |
| KB-53 | 3D View, Drawing: Zoom to window: drag a bounding box to magnify (W) | P0 | app action registry + keymap (context-dispatched) |
| KB-54 | 3D View: Zoom out (Z) | P0 | app action registry + keymap (context-dispatched) |
| KB-55 | 3D View: Zoom in on the center of the current view (Shift+Z) | P0 | app action registry + keymap (context-dispatched) |
| KB-56 | 3D View: Coarse rotate 15° left/right/up/down (← → ↑ ↓) | P0 | app action registry + keymap (context-dispatched) |
| KB-57 | 3D View: Rotate 90° (Shift+← → ↑ ↓) | P1 | app action registry + keymap (context-dispatched) |
| KB-58 | 3D View: Rotate 5° (precision) (Ctrl+← → ↑ ↓) | P1 | app action registry + keymap (context-dispatched) |
| KB-59 | 3D View: Pan the design up/down/left/right (Ctrl+Shift+← → ↑ ↓) | P1 | app action registry + keymap (context-dispatched) |
| KB-60 | 3D View: Open Section view dialog (multiple planes selectable) (Shift+X) | P0 | app action registry + keymap (context-dispatched) |
| KB-61 | 3D View: Open Named views dialog (prompts for a new view name) (Shift+V) | P1 | app action registry + keymap (context-dispatched) |
| KB-62 | 3D View: Toggle View in high quality (Shift+R) | P1 | app action registry + keymap (context-dispatched) |
| KB-63 | 3D View: Hide or show planes (P) | P1 | app action registry + keymap (context-dispatched) |
| KB-64 | 3D View: Hide all construction geometry (Shift+P) | P1 | app action registry + keymap (context-dispatched) |
| KB-65 | 3D View: Hide selected or hovered part/instance(s) (Y) | P0 | app action registry + keymap (context-dispatched) |
| KB-66 | 3D View: Show hidden (hovered/selected) instance(s) (Shift+Y) | P0 | app action registry + keymap (context-dispatched) |
| KB-67 | 3D View: Isolate selected or hovered part/instance(s) (cumulative while Isolate is active) (Shift+I) | P1 | app action registry + keymap (context-dispatched) |
| KB-68 | 3D View: Make selected or hovered part/instance(s) transparent (cumulative) (Shift+T) | P1 | app action registry + keymap (context-dispatched) |
| KB-69 | 3D View: Select through transparent geometry (parts made transparent via appearance) (Alt+click) | P1 | app action registry + keymap (context-dispatched) |
| KB-70 | Sketch: Line tool (L *) | P0 | app action registry + keymap (context-dispatched) |
| KB-71 | Sketch (line tool active): Switch between Line and Tangent arc while drawing (Shift+A) | P1 | app action registry + keymap (context-dispatched) |
| KB-72 | Sketch: 3 point arc tool (A) | P0 | app action registry + keymap (context-dispatched) |
| KB-73 | Sketch: Center point circle tool (C *) | P0 | app action registry + keymap (context-dispatched) |
| KB-74 | Sketch: Corner rectangle tool (G *) | P0 | app action registry + keymap (context-dispatched) |
| KB-75 | Sketch: Center point rectangle tool (R *) | P0 | app action registry + keymap (context-dispatched) |
| KB-76 | Sketch: Sketch point tool (Shift+S) | P0 | app action registry + keymap (context-dispatched) |
| KB-77 | Sketch: Sketch fillet tool (Shift+F) | P0 | app action registry + keymap (context-dispatched) |
| KB-78 | Sketch: Offset tool (O) | P0 | app action registry + keymap (context-dispatched) |
| KB-79 | Sketch: Extend tool (X) | P1 | app action registry + keymap (context-dispatched) |
| KB-80 | Sketch: Trim tool (M *) | P0 | app action registry + keymap (context-dispatched) |
| KB-81 | Sketch: Use (project/convert) tool (U) | P0 | app action registry + keymap (context-dispatched) |
| KB-82 | Sketch: Construction toggle (Q) | P0 | app action registry + keymap (context-dispatched) |
| KB-83 | Sketch: Dimension tool (D *) | P0 | app action registry + keymap (context-dispatched) |
| KB-84 | Sketch: Coincident constraint (I *) | P0 | app action registry + keymap (context-dispatched) |
| KB-85 | Sketch: Concentric constraint (Shift+O) | P1 | app action registry + keymap (context-dispatched) |
| KB-86 | Sketch: Curvature constraint (Shift+U) | P1 | app action registry + keymap (context-dispatched) |
| KB-87 | Sketch: Equal constraint (E) | P0 | app action registry + keymap (context-dispatched) |
| KB-88 | Sketch: Fix constraint (Shift+J) | P1 | app action registry + keymap (context-dispatched) |
| KB-89 | Sketch: Horizontal constraint (H *) | P0 | app action registry + keymap (context-dispatched) |
| KB-90 | Sketch: Vertical constraint (V *) | P0 | app action registry + keymap (context-dispatched) |
| KB-91 | Sketch: Midpoint constraint (Shift+M) | P1 | app action registry + keymap (context-dispatched) |
| KB-92 | Sketch: Normal constraint (Shift+K) | P1 | app action registry + keymap (context-dispatched) |
| KB-93 | Sketch: Parallel constraint (B *) | P0 | app action registry + keymap (context-dispatched) |
| KB-94 | Sketch: Perpendicular constraint (Shift+L *) | P0 | app action registry + keymap (context-dispatched) |
| KB-95 | Sketch: Pierce constraint (Shift+G) | P1 | app action registry + keymap (context-dispatched) |
| KB-96 | Sketch: Symmetric constraint (Shift+Q) | P1 | app action registry + keymap (context-dispatched) |
| KB-97 | Sketch: Tangent constraint (T *) | P0 | app action registry + keymap (context-dispatched) |
| KB-98 | Sketch: Suppress automatic inferences while moving or placing sketch entities (Shift (hold)) | P1 | app action registry + keymap (context-dispatched) |
| KB-99 | Feature Studio: Return cursor to its previous position in navigation history (Back reference) — as docu... (Ctrl+]) | P2 | app action registry + keymap (context-dispatched) |
| KB-100 | Feature Studio: Return cursor to its next position in navigation history (Forward reference) — as docum... (Ctrl+[) | P2 | app action registry + keymap (context-dispatched) |
| KB-101 | Feature Studio: Commit current Feature Studio (updates other tabs) (Ctrl+S) | P2 | app action registry + keymap (context-dispatched) |
| KB-102 | Feature Studio: Commit all Feature Studios with uncommitted changes (Ctrl+Shift+S) | P2 | app action registry + keymap (context-dispatched) |
| KB-103 | Feature Studio: Dismiss autocomplete (Esc) | P2 | app action registry + keymap (context-dispatched) |
| KB-104 | Feature Studio: Open Top level symbols (outline) dropdown (Ctrl+Shift+O) | P2 | app action registry + keymap (context-dispatched) |
| KB-105 | Drawing: Copy selected annotation(s) and place where dragged (Alt+LMB drag) | P1 | app action registry + keymap (context-dispatched) |
| KB-106 | Drawing: Diameter dimension tool (Shift+D) | P1 | app action registry + keymap (context-dispatched) |
| KB-107 | Drawing: Foreshorten dimension tool (Shift+H) | P1 | app action registry + keymap (context-dispatched) |
| KB-108 | Drawing: Place a radial dimension on an arc or circle (Shift+R) | P1 | app action registry + keymap (context-dispatched) |
| KB-109 | Drawing: Maximum or minimum dimension tool (Ctrl+M) | P1 | app action registry + keymap (context-dispatched) |
| KB-110 | Drawing (dimension active): Activate midpoint and quadrant points on arcs/circles for selection during dimension co... (Shift+Q) | P1 | app action registry + keymap (context-dispatched) |
| KB-111 | Drawing: Note tool (N) | P0 | app action registry + keymap (context-dispatched) |
| KB-112 | Drawing: Projected view tool (P) | P0 | app action registry + keymap (context-dispatched) |
| KB-113 | Drawing: Hide or show Sheets panel (Ctrl+S) | P1 | app action registry + keymap (context-dispatched) |
| KB-114 | Drawing: Open first sheet (Home) | P1 | app action registry + keymap (context-dispatched) |
| KB-115 | Drawing: Open last sheet (End) | P1 | app action registry + keymap (context-dispatched) |
| KB-116 | Drawing: Open previous sheet (Page Up) | P1 | app action registry + keymap (context-dispatched) |
| KB-117 | Drawing: Open next sheet (Page Down) | P1 | app action registry + keymap (context-dispatched) |
| KB-118 | Drawing: Update all views and properties on each sheet (Ctrl+Q) | P0 | app action registry + keymap (context-dispatched) |
| KB-119 | Assignable actions without default keys | P2 | action registry entries without default chords |
| KB-120 | Default mouse navigation (Onshape preset) | P0 | app camera controller (trackball + turntable), wheel zoom about cursor |
| KB-121 | Mouse control presets (other CAD systems) | P0 | input mapping table per preset; libspnav/3Dconnexion SDK for 3D mice |
| KB-122 | Touchpad and touchscreen navigation | P1 | GPUI pinch/scroll gesture events -> camera |
| KB-123 | Selection model (click, toggle, clear, ranges) | P0 | GPU ID-buffer or BVH ray picking; topology id -> entity map |
| KB-124 | Box selection (window vs crossing) and box deselect | P0 | frustum (window/crossing) selection over mesh BVH |
| KB-125 | Pre-highlight (hover) behavior | P1 | throttled hover pick (speed threshold, <=250 ms pause) |
| KB-126 | Select other (grave accent key) | P1 | all-hits ray pick sorted by depth (BVH / IntCurvesFace_ShapeIntersector) |
| KB-127 | Selecting through transparency | P1 | pick filter skipping alpha<1 unless Alt |
| KB-128 | Selection in dialogs and pre-selection | P0 | dialog field focus + Filter predicates (TopAbs/GeomAbs/body type) |
| KB-129 | Selection filters, chain selection and query-based selection | P1 | smart selection algorithms (MT-42); query variable |
| KB-130 | Search tools (command search) | P1 | fuzzy matcher over action registry |
| KB-131 | Shortcut toolbar (S key) | P1 | per-context customizable mini toolbar |
| KB-132 | Triad manipulator | P0 | gizmo rendering + ray/plane math; assembly solver for mated drag |
| KB-133 | Numeric field input behaviors | P0 | shared expression parser/evaluator with units |
| KB-134 | Context menu invocation and hover-targeted commands | P0 | GPUI context menus; hover-target resolution |
| KB-135 | Undo / redo scope | P0 | app command stack; dialog-local actions collapsed on commit |
