| SF-01 | Surface modeling overview and surface body semantics | P1 | Sheet = TopoDS_Shell/Face; continuity checks LocalAnalysis_SurfaceContinuity/CurveContinuity (G3 custom) |
| SF-02 | Surface result operation (New / Add) and surface joining | P1 | BRepBuilderAPI_Sewing + ShapeFix_Shell; closed shell -> BRepBuilderAPI_MakeSolid + ShapeFix_Solid; UnifySameDomain |
| SF-03 | Offset surface | P1 | BRepOffsetAPI_MakeOffsetShape::PerformBySimple / PerformByJoin(Skin); offset 0 = BRepBuilderAPI_Copy |
| SF-04 | Fill | P1 | BRepOffsetAPI_MakeFilling (GeomPlate) with per-edge C0/G1/G2 + support faces; guide points/curves; tune for G2 quality |
| SF-05 | Move boundary (extend / trim surface edges) | P1 | GeomLib::ExtendSurfByLength (cont 1/2) + face rebuild; up-to via BRepAlgoAPI_Splitter; trim via UV offset split (custom) |
| SF-06 | Boundary surface | P1 | GeomFill_BSplineCurves / GeomFill_ConstrainedFilling (+BoundWithSurf for G1); Gordon/G2 custom (port TiGL) |
| SF-07 | Ruled surface | P1 | Custom direction field -> second rail; GeomFill::Surface / BRepFill::Face; corners via MakeRevol/ThruSections |
| SF-08 | Constrained surface | P2 | GeomPlate_BuildPlateSurface point constraints + GeomPlate_MakeApprox; GeomAPI_PointsToBSplineSurface; scattered-fit custom |
| SF-09 | Face blend | P2 | BRepFilletAPI_MakeFillet/MakeChamfer for adjacent; GeomFill_Pipe(path,c1,c2,R) / BlendFunc_* walking; conic/G2/hold lines custom |
| SF-10 | Mutual trim | P1 | Extend surfaces (GeomLib) + BRepAlgoAPI_Splitter both ways, side classification, BRepBuilderAPI_Sewing merge |
| SF-11 | Enclose | P1 | BOPAlgo_MakerVolume + BRepAlgoAPI_Fuse/Cut/Common for Add/Remove/Intersect |
| SF-12 | Other surface operations (cross-references) | P1 | See 3A items (extrude/revolve/sweep/loft surface, thicken, delete/move/replace face, split) |
| CV-01 | Curve bodies and shared Approximate parameters | P1 | Wire bodies; GCPnts sampling + GeomAPI_PointsToBSpline / AppDef_Variational with end tangents; deviation via projection |
| CV-02 | Helix | P1 | Geom2d_Line on Geom_Cylindrical/ConicalSurface + BRepBuilderAPI_MakeEdge + BuildCurves3d; spiral via B-spline fit or OCCT 7.9 TKHelix |
| CV-03 | Projected curve | P1 | Two sketches: MakePrism x2 + BRepAlgoAPI_Section; curve-to-face: BRepProj_Projection (dir) / BRepOffsetAPI_NormalProjection |
| CV-04 | Bridging curve | P1 | Geom_BezierCurve with computed poles (deg = d1+d2+1); G0-G2 Hermite math; G3 flow math custom |
| CV-05 | Composite curve | P1 | ShapeAnalysis_FreeBounds::ConnectEdgesToWires + BRepBuilderAPI_MakeWire; GeomConvert_CompCurveToBSplineCurve |
| CV-06 | Intersection curve | P1 | BRepAlgoAPI_Section(group1, group2) + ConnectEdgesToWires (one wire body per chain) |
| CV-07 | Isocline | P2 | Contap_Contour (direction + draft angle) per face + approximation; split via BRepFeat_SplitShape |
| CV-08 | Isoparametric curve | P2 | Geom_Surface::UIso/VIso clipped with Geom2dHatch_Hatcher to face domain; point via GeomAPI_ProjectPointOnSurf |
| CV-09 | 3D fit spline | P1 | GeomAPI_Interpolate (centripetal params, end tangents, periodic); curvature ends custom (AppDef); edges via CompCurveToBSpline |
| CV-10 | Routing curve | P2 | GeomAPI_Interpolate w/ per-point tangents; polyline BRepBuilderAPI_MakePolygon + ChFi2d corner arcs; triad/CSV/table app code |
| CV-11 | Offset curve (offset curve on face) | P2 | Planar: BRepOffsetAPI_MakeOffset (Arc/Intersection joins); Euclidean: tube section; geodesic custom; split BRepFeat_SplitShape |
| CV-12 | Edit curve | P2 | Geom_BSplineCurve IncreaseDegree/SetPole/SetWeight, approximation, PCA plane projection; BRepTools_ReShape in place |
| CV-13 | Trim curve (move curve boundary) | P1 | GeomLib::ExtendCurveToPoint (cont 1 tangent / 2 curvature); trim via IntTools/BRepAlgoAPI_Section + help-point side pick |
| RG-01 | Default origin and Top / Front / Right planes | P0 | gp_Pln constants (Top XY +Z, Front XZ -Y, Right YZ +X); display face 150 mm / 6 in; app data model |
| RG-02 | Plane | P0 | gp_Pln/gp_Ax3 math for 8 plane types; Geom_Plane display face via BRepBuilderAPI_MakeFace |
| RG-03 | Mate connector (Part Studio feature) | P0 | gp_Ax3/gp_Trsf composition (realign, move, flip, reorient); owner + attribute in app data model |
| RG-04 | Mate connector inference points | P0 | BRepGProp centroids, GCPnts midpoints, gp_Circ/gp_Cone axis/apex, Bnd_Box axis extents; hover UI app code |
| RG-05 | Point (construction point, internal) | P2 | BRepAdaptor_Curve value at parameter + BRepBuilderAPI_MakeVertex |
| RG-06 | Name entity (internal) | P2 | App-level topological naming attribute; no geometry |
| FR-01 | Frame | P2 | BRepOffsetAPI_MakePipeShell per segment (SetMode support for lock faces); half-space cuts for miter/butt, BRepAlgoAPI_Cut for cope |
| FR-02 | Frame profile libraries | P2 | App data: library folders + configured profile sketches; author standard profile content (ISO/ANSI/8020/AISC/AS/wood) |
| FR-03 | Tag (Frame profile tag) | P2 | Attribute on sketch feature (Standard, Description, columns, alignment points) |
| FR-04 | Frame trim | P2 | BRepAlgoAPI_Cut ordered groups, keep largest solid (BRepGProp); face mode BRepAlgoAPI_Splitter / half-space |
| FR-05 | Gusset (frame gusset) | P2 | Planar triangle/rectangle wire + BRepPrimAPI_MakePrism symmetric; base-plane detection via BRepAdaptor_Surface |
| FR-06 | End cap | P2 | Outer wire / bbox rect / circle -> BRepPrimAPI_MakePrism; 2D offset; BRepFilletAPI fillet/chamfer corners |
| FR-07 | Cut list | P2 | App data model (composite part + table); congruence grouping via GProp with 1% tolerance; lengths/angles from geometry |
