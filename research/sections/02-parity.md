| SKB-01 | Sketch feature | P0 | App feature; Geom_Plane from face/mate connector; Geom2d entities -> BRepBuilderAPI_MakeEdge(c2d, plane); custom solver |
| SKB-02 | Sketch plane selection and plane frame | P0 | BRepAdaptor_Surface::Plane / gp_Ax3 from mate connector; custom canonical axis alignment |
| SKB-03 | Sketch editing workflow and toolbar | P0 | App UI/command framework (GPUI) |
| SKB-04 | Sketch dialog display toggles (Show constraints / Show expressions / Show errors / Show dimensions) | P1 | Render-layer flags |
| SKB-05 | Sketch output data model and entity identity | P0 | App data model; stable entity ids; TopoDS_Compound of wire/point/face bodies |
| SKB-06 | Region detection | P0 | BOPAlgo_Builder (split) + BOPAlgo_BuilderFace / BOPAlgo_Tools::WiresToFaces, or custom 2D DCEL + BRepBuilderAPI_MakeFace |
| SKB-07 | Imprinting of the sketch-plane face (Disable imprinting) | P1 | Add face boundary edges (TopExp::MapShapes) as non-output splitters in region arrangement |
| SKB-08 | Constraint solver | P0 | Custom GCS (planeGCS/SolveSpace style: Newton/DogLeg/LM, Jacobian QR/SVD DOF analysis) in Rust (nalgebra/faer) |
| SKB-09 | Constraint status coloring (DOF display) | P0 | From solver null-space analysis per entity |
| SKB-10 | Over-constraint and conflict reporting | P0 | Solver redundancy/conflict detection (rank of J^T), auto-driven dims |
| SKB-11 | Dragging geometry | P0 | Solver drag mode (min-change least squares with soft target) |
| SKB-12 | Automatic inferencing and snapping | P0 | Custom proximity/inference engine; Geom2dAPI_ProjectPointOnCurve; BRepExtrema for external coplanar entities |
| SKB-13 | Dimension while sketching (on-the-fly value entry) | P0 | App UI + expression evaluator |
| SKB-14 | Construction geometry and construction mode | P0 | Entity flag; filtered before edge/region build |
| SKB-15 | Constraint display and interaction | P0 | App UI (icon layout, hit-testing) |
| SKB-16 | Constraint manager | P1 | App UI over solver constraint list |
| SKB-17 | Profile inspector | P1 | Curve-graph endpoint valence analysis; ShapeAnalysis_FreeBounds optional |
| SKB-18 | Sketch visibility | P1 | App visibility state |
| SKB-19 | Copy/paste sketches and sketch entities; derived sketches | P1 | Serialize entities + internal constraints; Derived feature import |
| SKB-20 | Sketch feature context menu commands | P1 | App UI; DXF export via dxf crate/libdxfrw (no OCCT open-source DXF) |
| SKB-21 | Expressions and variables in sketch values | P0 | Shared expression evaluator with units |
| SKB-22 | Sketch failure states and messages | P1 | App error enum mirroring SKETCH_* strings |
| SKB-23 | External references and in-context sketch geometry | P0 | IntAna_QuadQuadGeo plane traces; GeomProjLib::ProjectOnPlane + GeomAPI::To2d |
| SKB-24 | Sketch keyboard shortcuts | P1 | App keymap (customizable) |
| SKE-01 | Line (and polyline chaining) | P0 | GCE2d_MakeSegment -> Geom2d_TrimmedCurve(Geom2d_Line) |
| SKE-02 | Midpoint line | P1 | GCE2d_MakeSegment + midpoint constraint |
| SKE-03 | Corner rectangle | P0 | 4x GCE2d_MakeSegment + coincident/parallel/H/V constraints (skRectangle set) |
| SKE-04 | Center point rectangle | P0 | 4x GCE2d_MakeSegment + construction diagonals + midpoint |
| SKE-05 | Aligned rectangle (3-point rectangle) | P1 | 4x GCE2d_MakeSegment + parallel/perpendicular constraints |
| SKE-06 | Inscribed polygon | P1 | gp_Trsf2d rotation vertices, GCE2d_MakeSegment, construction Geom2d_Circle |
| SKE-07 | Circumscribed polygon | P1 | As SKE-06 (vertices on circle) |
| SKE-08 | Center point circle | P0 | GCE2d_MakeCircle / Geom2d_Circle |
| SKE-09 | 3 point circle | P0 | GCE2d_MakeCircle(p1,p2,p3) / gce_MakeCirc2d |
| SKE-10 | 3 point arc | P0 | GCE2d_MakeArcOfCircle(p1,pMid,p2) |
| SKE-11 | Tangent arc | P0 | GCE2d_MakeArcOfCircle(p1, tangent vec, p2) + tangent constraint |
| SKE-12 | Center point arc | P0 | GCE2d_MakeArcOfCircle(gp_Circ2d, p1, p2, sense) |
| SKE-13 | Ellipse | P1 | GCE2d_MakeEllipse / Geom2d_Ellipse (swap axes if primary < secondary) |
| SKE-14 | Elliptical arc | P1 | GCE2d_MakeArcOfEllipse (u = 2*pi*param) |
| SKE-15 | Conic | P2 | Rational quadratic Geom2d_BezierCurve, middle weight rho/(1-rho) |
| SKE-16 | Spline (interpolated / fit spline) | P1 | Geom2dAPI_Interpolate (+Load end tangents, periodic for closed) -> Geom2d_BSplineCurve |
| SKE-17 | Spline control point tool | P2 | Re-interpolate with inserted point; Geom2d_BezierCurve::InsertPoleAfter/Increase |
| SKE-18 | Bezier curve | P2 | Geom2d_BezierCurve (degree <= 15), Segment for trim/split |
| SKE-19 | Point | P0 | gp_Pnt2d; BRepBuilderAPI_MakeVertex |
| SKE-20 | Slot | P1 | BRepOffsetAPI_MakeOffset(open wire, GeomAbs_Arc, isOpenResult) or Geom2d_OffsetCurve + end arcs |
| SKE-21 | Text | P1 | Font_FontMgr + Font_BRepFont + Font_BRepTextBuilder (bundle listed OFL fonts) |
| SKE-22 | Insert image | P2 | No geometry; textured quad in renderer; 4-corner solver frame |
| SKE-23 | Insert DXF or DWG | P1 | Gap in OCCT OSS: dxf crate/libdxfrw (DXF), LibreDWG/ODA (DWG) -> Geom2d entities |
| SKT-01 | Trim | P0 | Geom2dAPI_InterCurveCurve + Geom2d_TrimmedCurve / BSpline Segment |
| SKT-02 | Extend | P0 | Intersect untrimmed basis curve (Geom2dAPI_InterCurveCurve); splines custom extrapolation |
| SKT-03 | Offset | P0 | BRepOffsetAPI_MakeOffset (chains) / Geom2d_OffsetCurve; Geom2dConvert_ApproxCurve for splines |
| SKT-04 | Sketch mirror | P0 | gp_Trsf2d::SetMirror(gp_Ax2d) + MIRROR constraints |
| SKT-05 | Linear sketch pattern | P1 | gp_Trsf2d translation copies + pattern constraint |
| SKT-06 | Circular sketch pattern | P1 | gp_Trsf2d rotation copies + pattern constraint |
| SKT-07 | Sketch fillet | P0 | ChFi2d_FilletAPI (Ana/general) or Geom2dGcc_Circ2d2TanRad |
| SKT-08 | Sketch chamfer | P1 | ChFi2d_ChamferAPI / custom distance intersections |
| SKT-09 | Use (project / convert) | P0 | GeomProjLib::ProjectOnPlane + GeomAPI::To2d; analytic degenerate-projection recognition |
| SKT-10 | Use silhouette edges | P1 | Analytic silhouettes (cyl/cone/sphere/torus); HLRBRep_Algo outlines / Contap_Contour |
| SKT-11 | Intersection | P1 | BRepAlgoAPI_Section(face, plane) -> 2D curves |
| SKT-12 | Sketch split | P1 | Geom2d_TrimmedCurve / BSpline & Bezier Segment at projected params |
| SKT-13 | Transform sketch entities (translate / rotate / scale) | P1 | gp_Trsf2d + constraint rewrite rules (H/V swap, directed dim removal) |
| SKT-14 | Wrap (sketch-region wrap onto cylinder/cone) | P2 | Custom developable mapping onto Geom_CylindricalSurface/ConicalSurface; BRepBuilderAPI_MakeFace with pcurves |
| SKC-01 | Coincident | P0 | Custom solver residuals (P-P, P-curve, collinear, same circle, plane trace) |
| SKC-02 | Concentric | P0 | Custom solver residual (centers equal) |
| SKC-03 | Parallel | P0 | Custom solver residual cross(d1,d2)=0 with alignment sign |
| SKC-04 | Tangent | P0 | Custom residuals; Geom2dGcc/GccAna for initial tangent construction |
| SKC-05 | Horizontal | P0 | Custom residual (dy=0 / equal y for point sets / ellipse axis) |
| SKC-06 | Vertical | P0 | Custom residual (dx=0 / equal x) |
| SKC-07 | Perpendicular | P0 | Custom residual dot(d1,d2)=0 |
| SKC-08 | Equal | P0 | Custom residual (equal length / equal radius) |
| SKC-09 | Midpoint | P0 | Custom residual (segment midpoint, arc angular midpoint, 3-point internal line) |
| SKC-10 | Normal | P1 | Custom residual with auxiliary contact parameter |
| SKC-11 | Pierce | P1 | GeomAPI_IntCS(curve, plane) fixed point + custom residual |
| SKC-12 | Symmetric (MIRROR) | P0 | Custom residual (mirror about axis line) |
| SKC-13 | Fix | P0 | Remove params from unknowns / equality residuals |
| SKC-14 | Curvature (EQUAL_CURVATURE) | P1 | Geom2dLProp_CLProps2d curvature; custom G2 residual |
| SKC-15 | Quadrant (auto) | P1 | Custom residual (point at ellipse axis end) |
| SKC-16 | Use / Projected (auto) | P0 | Projection binding (see SKT-09) |
| SKC-17 | Silhouetted (auto) | P1 | Silhouette binding (see SKT-10) |
| SKC-18 | Intersected (auto) | P1 | BRepAlgoAPI_Section binding |
| SKC-19 | Offset (auto) | P1 | Geom2d_OffsetCurve evaluation + custom residual |
| SKC-20 | Linear pattern (auto) | P1 | Custom residuals (translation relationships) |
| SKC-21 | Circular pattern (auto) | P1 | Custom residuals (rotation relationships) |
| SKC-22 | Bezier degree (auto) | P2 | Geom2d_BezierCurve::Increase; custom degree reduction |
| SKC-23 | Freeze | P2 | Reserve enum value (undocumented) |
| SKD-01 | Dimension tool (general behavior) | P0 | Solver + annotation renderer; Geom2dAPI_ExtremaCurveCurve for measuring |
| SKD-02 | Line length | P0 | Custom residual |
| SKD-03 | Point-to-point distance (aligned / horizontal / vertical) | P0 | Custom residual with DimensionDirection + alignment sign |
| SKD-04 | Point-to-line distance | P0 | Custom signed residual with half-space |
| SKD-05 | Line-to-line distance | P0 | Custom residual (implies parallel) |
| SKD-06 | Distance to plane / external geometry | P0 | IntAna_QuadQuadGeo trace + custom residual |
| SKD-07 | Dimensions to circles and arcs (center / min / max) | P2 | Custom residual (center distance +/- r) |
| SKD-08 | Angular dimension | P0 | Custom atan2 residual with quadrant senses |
| SKD-09 | Radius | P0 | Custom residual |
| SKD-10 | Diameter | P0 | Custom residual |
| SKD-11 | Ellipse major / minor diameter | P1 | Custom residual |
| SKD-12 | Arc length / curve length | P1 | GCPnts_AbscissaPoint::Length; arc analytic r*theta |
| SKD-13 | Spline point / handle dimensions | P1 | Custom residual (as point distances) |
| SKD-14 | Centerline dimension | P1 | Custom residual 2*dist(point, centerline) |
| SKD-15 | Rho dimension | P2 | Bezier weight rho/(1-rho) |
| SKD-16 | Driven (reference) dimensions | P0 | Excluded from solve; post-solve measurement |
| SKD-17 | Negative values and dimension orientation data | P1 | Stored alignment/half-space orientation x signed value |
| SKD-18 | Dimension values: expressions, variables, configurations | P0 | Shared expression engine |
