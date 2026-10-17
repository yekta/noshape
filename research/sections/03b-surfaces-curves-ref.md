## 3B. Part Studio features — surfaces, curves, reference geometry, frames (weldments)

Scope: surface-modeling tools (SF-), curve tools (CV-), reference geometry (RG-), and frame/weldment tools (FR-) in the Part Studio. Solid-creating features that can also create surfaces (Extrude, Revolve, Sweep, Loft, Thicken), Delete/Move/Replace face and Split are specified in section 3A (PS- items); here they are only cross-referenced. Gusset is a frame tool (frame-segment gussets) and is specified here as FR-05.

Conventions used in this section:
- Bounds are written "kind, min .. max, default <metric> / <imperial>" from the FeatureScript bound specs (`extracted/bounds.txt` or the .fs file). Example: LENGTH_BOUNDS = length, -500 m .. 500 m, default 25 mm / 1 in.
- "Surface" = Onshape sheet body (BodyType.SHEET); "Curve" = wire body (BodyType.WIRE) listed in the Parts list "Curves" group; "Part" = solid body.
- Error names are ErrorStringEnum identifiers from the stdlib (message text is not in the local corpus).
- Many features carry `UIHint.REMEMBER_PREVIOUS_VALUE`: the dialog opens with the value last used in the session/user profile (app preference store needed).
- "(inferred)" marks behavior deduced from code/screenshots but not stated in the docs.

Shared predicates expanded once (referenced below):
- **surfaceOperationTypePredicate** -> `surfaceOperationType` "Result body operation type" (horizontal enum NewSurfaceOperationType: New `NEW` / Add `ADD`).
- **surfaceJoinStepScopePredicate** (only when `surfaceOperationType != NEW`) -> `defaultSurfaceScope` "Merge with all" (boolean, default true); if false -> `booleanSurfaceScope` "Merge scope" (Query, Filter: BODY && SHEET && modifiable && mesh allowed && not sketch).
- **booleanStepTypePredicate** -> `operationType` "Result body operation type" (NewBodyOperationType: New/Add/Remove/Intersect). **booleanStepScopePredicate** (when not New) -> `defaultScope` "Merge with all" (default false) and `booleanScope` "Merge scope" (solid bodies).
- **curveApproximationPredicate** -> see CV-01.

### SF-01 Surface modeling overview and surface body semantics
- **Purpose:** Concept item: what surfaces are, which features make them, how they become solids, and the continuity vocabulary (G0-G3) used by all surface/curve tools.
- **Sources:** https://cad.onshape.com/help/Content/PartStudio/surface_modeling.htm ; FS: boolean.fs (`joinSurfaceBodiesWithAutoMatching`, `surfaceOperationTypeEditLogic`).
- **Selection inputs:** n/a (concept).
- **Parameters:** n/a.
- **Behavior / semantics:**
  - A surface is a single-sided sheet body with no mass/thickness. Surfaces appear in the Parts list under "Surfaces"; they can be used to (a) make solids via Thicken (3A) or Enclose (SF-11) or auto-solidify when a joined set closes a volume (see SF-02), (b) act as construction geometry (split tools, loft/fill support, mold split lines), (c) replace faces of parts (Replace face, 3A).
  - Surface-creating sources (cross-reference): Extrude/Revolve/Sweep/Loft with creation type "Surface" (3A PS- items; they use SF-02 New/Add semantics); Thicken input; plus the dedicated tools SF-03..SF-10.
  - Continuity definitions (used by Fill, Boundary surface, Bridging curve, Face blend, Fit spline, Loft):
    - G0 Match position: share a common point/edge.
    - G1 Match tangent: G0 + equal tangent direction / surface normal across the joint.
    - G2 Match curvature: G1 + equal curvature at the joint.
    - G3 Match flow: G2 + equal rate of change of curvature.
    - Control-point implication: G0 needs 1 control point per end, G1 2, G2 3, G3 4; a Bezier constrained G3 at both ends needs >= 8 control points.
  - Quality guidance in docs (non-normative but drives tool design): prefer Bezier curves of degree 3/5/7 (degree = control points - 1, keep <= 7); prefer 4-sided surfaces (Boundary surface) over Fill/Loft/Sweep for clean control nets; avoid building from trimmed edges/projected curves (dense cubic spans) - overbuild and Mutual trim instead; add Face blend/Fillet last.
  - Analysis tools referenced (specified in the analysis/view section, not here): Curve/surface analysis (Shift+C, curvature combs + control-point grid), Connection analysis, Zebra stripes, Curvature color map (Gaussian etc.), Dihedral analysis (Shift+D), Deviation analysis, Reflection analysis, Highlight boundary edges (open edges in red), Face tangent angle measure, View in high quality (Shift+R).
  - Delete face as a surface operation: Delete face (3A) with "leave open" turns a solid into a surface (sheet) by removing faces; cross-reference only.
- **Edge cases & errors:** see individual tools.
- **OCCT mapping:** Sheet bodies = TopoDS_Shell/TopoDS_Face not in a solid; continuity checks via BRepLProp_SLProps / GeomLProp, LocalAnalysis_SurfaceContinuity / LocalAnalysis_CurveContinuity (G0/G1/G2 classification; no G3 support - custom 3rd-derivative check).
- **Priority:** P1

### SF-02 Surface result operation (New / Add) and surface joining ("knitting")
- **Purpose:** Shared post-processing step for every surface-creating feature with a "Result body operation type": either keep the new surface as separate sheet bodies, or join it to adjacent existing surfaces along matching edges (and auto-create a solid if closed).
- **Sources:** surface_modeling.htm, fill.htm Notes, boundary_surface.htm; FS: `surfaceOperationTypePredicate`, `surfaceJoinStepScopePredicate`, `joinSurfaceBodiesWithAutoMatching` (boolean.fs).
- **Selection inputs:** "Merge scope" (sheet bodies, multiple) when Merge with all is off.
- **Parameters:**

| FS id | UI label | Type | Enum values / default / bounds+units | Visible when / notes |
|---|---|---|---|---|
| surfaceOperationType | Result body operation type | enum NewSurfaceOperationType | New (NEW), Add (ADD); default New in most features | always |
| defaultSurfaceScope | Merge with all | boolean | default true | surfaceOperationType != New |
| booleanSurfaceScope | Merge scope | Query | sheet bodies, modifiable, not sketch, mesh allowed | Merge with all = false |

- **Behavior / semantics:**
  - New: created sheet bodies are left as new surfaces (named "Surface N").
  - Add: tools = new surfaces of this feature; targets = all other modifiable surfaces in the Part Studio (Merge with all) or the Merge scope. A boolean UNION of sheets is run with automatic detection of coincident ("matching") edges; matching boundary edges are sewn so bodies merge into one sheet. Imprinted edges created by the join are erased (merged) by default.
  - Some features pass `makeSolid = true` (Fill always; Boundary surface since V2513; Ruled surface passes false): if the joined sheets enclose a volume, a solid part is created instead of a surface. Fill help: "If the operation results in a closed surface (creating a volume), Onshape automatically creates a solid part (if Add is selected). Use New to keep all surfaces."
  - If Add with Merge with all finds no other surfaces -> error BOOLEAN_NO_SURFACE_TO_MERGE_WITH; if the merge scope has none -> BOOLEAN_NO_SURFACE_IN_MERGE_SCOPE. On a failed join, the feature's own result is re-created as red error geometry (reconstructOp) for display, then deleted.
  - Edit logic (`surfaceOperationTypeEditLogic`) may preset Add/New on creation based on whether the input edges belong to existing surfaces (inferred from name; behavior: selecting edges of an existing surface defaults to Add).
- **Edge cases & errors:** BOOLEAN_NO_SURFACE_TO_MERGE_WITH, BOOLEAN_NO_SURFACE_IN_MERGE_SCOPE, BOOLEAN_NO_TARGET_SURFACE (info), feature-specific wrappers (e.g. FILL_SURFACE_ATTACH_FAIL).
- **OCCT mapping:** BRepBuilderAPI_Sewing (tolerance ~1e-6..1e-5 m) on new + target faces, then ShapeFix_Shell/ShapeFix_Solid; if sewn shell is closed (BRepCheck / ShapeAnalysis_Shell free edges == 0) and makeSolid -> BRepBuilderAPI_MakeSolid + ShapeFix_Solid orientation. Alternatively BRepAlgoAPI_Fuse of sheets + ShapeUpgrade_UnifySameDomain for imprint cleanup. Edge matching = coincident-edge detection (BRepExtrema / tolerance compare of edge curves).
- **Priority:** P1

### SF-03 Offset surface
- **Purpose:** Create a new surface by offsetting faces, surfaces, or sketch regions; offset 0 = copy in place.
- **Sources:** https://cad.onshape.com/help/Content/PartStudio/offset_surface.htm ; FS: `offsetSurface` (offsetSurface.fs).
- **Selection inputs:** "Faces, surfaces, and sketch regions to offset" - Filter: (FACE || (SHEET BODY && not sketch)) && not construction && mesh allowed; multiple picks; `UIHint.SHOW_CREATE_SELECTION` (create-selection helper available). Selecting a sheet body = all its faces.
- **Parameters:**

| FS id | UI label | Type | Enum values / default / bounds+units | Visible when / notes |
|---|---|---|---|---|
| surfacesAndFaces | Faces, surfaces, and sketch regions to offset | Query | see filter | always |
| offset | Offset | length | ZERO_INCLUSIVE_OFFSET_BOUNDS: 0 .. 500 m, default 5 mm / 0.25 in; REMEMBER_PREVIOUS_VALUE | always |
| oppositeDirection | Opposite offset direction | boolean (flip arrow) | default false | always |

- **Behavior / semantics:**
  - Signed offset = offset * (opposite ? -1 : 1) along each face's outward normal (sheet face normal / sketch region normal).
  - Calls opExtractSurface(faces, offset, useFacesAroundToTrimOffset=false): extracts the selected faces as new sheet bodies and offsets them; connected selected faces yield one connected surface (doc example: a revolved face + planar face -> one surface).
  - The source is not consumed. Result is always New (no Add option).
  - Linear manipulator (arrow) placed on the last selected non-mesh face (tangent plane at its "direct edit manipulator plane"); dragging sets |value| and flips direction when negative.
  - Feature-pattern aware (remainder transform applied).
- **Edge cases & errors:** empty selection -> DIRECT_EDIT_MOVE_FACE_CREATE_SELECT; no tangent plane for manipulator -> NO_TANGENT_PLANE. Offsets larger than the min concave radius self-intersect (kernel failure, inferred). Mesh faces are allowed.
- **OCCT mapping:** offset 0: BRepBuilderAPI_Copy of faces then BRepBuilderAPI_Sewing. Non-zero: BRepOffsetAPI_MakeOffsetShape::PerformBySimple (BRepOffset_MakeSimpleOffset, robust for sheets, no self-intersection handling) or PerformByJoin(shape, offset, tol, BRepOffset_Skin, intersection=false, selfInter=false, GeomAbs_Arc). Sketch regions: build faces from sketch region first (BRepBuilderAPI_MakeFace on planar wire). Mesh faces: gap (needs mesh offset - custom).
- **Priority:** P1

### SF-04 Fill
- **Purpose:** Create a surface (or a part from surfaces) bounded by a closed chain of edges/curves, with per-boundary continuity (G0/G1/G2) and optional interior guide points/curves; works with any number of boundary segments (not limited to 4 sides).
- **Sources:** https://cad.onshape.com/help/Content/PartStudio/fill.htm ; FS: `fill` (fillSurface.fs).
- **Selection inputs:**
  - Edges array: each item has one "Edges" query (Filter: modifiable && ((EDGE && not construction) || (WIRE BODY && not sketch))), shown as rows labelled "[continuity] entities"; preselected edges become one row each with G0.
  - "Adjacent faces" per row (FACE, multiple) - only for G1/G2 rows.
  - Guides "Vertices or curves": VERTEX || EDGE || (WIRE BODY && not sketch), multiple.
- **Parameters:**

| FS id | UI label | Type | Enum values / default / bounds+units | Visible when / notes |
|---|---|---|---|---|
| preselectedEntities | Preselection | Query | hidden | always hidden; converted to rows |
| surfaceOperationType | Result body operation type | enum | New / Add (default New) | always (SF-02) |
| edges[] | Edges | array of rows | one row per boundary selection | always; SHOW_CREATE_SELECTION |
| edges[].entities | Edges | Query | see filter | hidden label (row driver) |
| edges[].continuity | Continuity | enum GeometricContinuity | Position (G0), Tangency (G1), Curvature (G2); new rows copy last row (MATCH_LAST_ARRAY_ITEM) | always |
| edges[].adjacentFaces | Adjacent faces | Query | FACE | continuity != G0 (and field defined) |
| addGuides | Guides | boolean | default false | always |
| guideEntities | Vertices or curves | Query | see filter | addGuides |
| constraintMode | Mode | enum FillConstraintMode | Precise (PRECISE, default), Sampled (SAMPLED) | addGuides |
| sampleSize | Sample size | integer | SAMPLE_BOUNDS 1 .. 50, default 5 | addGuides && Sampled |
| showIsocurves | Show isocurves | boolean | default false | always |
| curveCount | Count | integer | ISO_GRID_BOUNDS 1 .. 50, default 10 | showIsocurves |
| defaultSurfaceScope / booleanSurfaceScope | Merge with all / Merge scope | | SF-02 (default Merge with all = true) | Add |

- **Behavior / semantics:**
  - Boundary must form a single closed loop of connected edges; while open, the UI shows red dots at the chain's open ends. A chain that completes a loop but overshoots where it meets other entities is still accepted. Edges of wire bodies (curves) are followed back to their laminar source edges (`followWireEdgesToLaminarSource`) so composite curves built from surface edges can carry G1/G2.
  - Continuity per row:
    - Position: G0, no tangency/curvature relationship.
    - Tangency: G1 to the face adjacent to the boundary edge. For sketch edges: implicit tangency relative to the sketch plane ("Onshape respects the direction normal to that sketch plane", "as if you had a reference surface"); works for sketch selections only, not other planar curves.
    - Curvature: G2 to the adjacent surface.
    - Adjacent faces (optional for G1/G2): any face whose underlying geometry is coincident with at least one profile curve (face and edge need not be topologically connected or in the same part).
    - Interior edges, 3D fit splines, projected curves and bridging curves can only match position (no adjacent face); a composite curve made from surface edges can match tangency/curvature.
  - Guides: vertices -> surface interpolates the points; curves -> Precise follows the exact curve (curves must touch but not cross the boundary; where guide curves meet each other they must meet tangentially), Sampled samples `sampleSize` points per curve (loosens constraints, smoother, may ripple; works for guides not touching the boundary).
  - Show isocurves: displays the untrimmed underlying surface as an iso-parametric mesh with Count lines per direction (display only).
  - Add: join with automatic matching, makeSolid = true (closed result becomes a solid part); join failure is reported as FILL_SURFACE_ATTACH_FAIL.
- **Edge cases & errors:** FILL_SURFACE_NO_EDGES (no rows), FILL_SURFACE_OPEN_LOOP, FILL_SURFACE_MULTI_LOOP, FILL_SURFACE_DOUBLE_SELECTION, FILL_SURFACE_G2_FAIL, FILL_SURFACE_FAIL, FILL_SURFACE_ATTACH_FAIL; mesh entities rejected (verifyNoMesh) for edges and guides.
- **OCCT mapping:** BRepOffsetAPI_MakeFilling (BRepFill_Filling over GeomPlate): Add(edge, GeomAbs_C0 | GeomAbs_G1 | GeomAbs_G2, support face, isBound=true) per boundary edge; guide vertices -> Add(gp_Pnt); Precise guide curves -> Add(edge, GeomAbs_C0, isBound=false); Sampled -> GCPnts_UniformAbscissa(sampleSize) points -> Add(gp_Pnt). Parameters (Degree, NbPtsOnCur, NbIter, Tol2d/3d, MaxDeg, MaxSegments) to tune. Sketch-edge implicit tangency: synthesize a support face (BRepPrimAPI_MakePrism of the edge along the sketch normal) (inferred). Isocurves: Geom_Surface UIso/VIso of the plate surface. Gap: GeomPlate G2 is approximate and slower than Onshape's; quality tuning work needed.
- **Priority:** P1

### SF-05 Move boundary (extend / trim surface edges)
- **Purpose:** Move boundary edges of a surface outward (extend) or inward (trim) by a distance or up to a face/part/surface/vertex. (Stdlib feature id is `extendSurface`; feature type name "Move boundary". The curve analog is Trim curve CV-13, stdlib file moveCurveBoundary.fs.)
- **Sources:** https://cad.onshape.com/help/Content/PartStudio/move_boundary.htm ; FS: `extendSurface` (extend.fs).
- **Selection inputs:** "Surface or boundary edges": (SHEET BODY) || (EDGE && EdgeTopology.ONE_SIDED), not construction, not sketch, modifiable; multiple. A body pick = all its laminar (boundary) edges.
- **Parameters:**

| FS id | UI label | Type | Enum values / default / bounds+units | Visible when / notes |
|---|---|---|---|---|
| entities | Surface or boundary edges | Query | see filter | always |
| tangentPropagation | Tangent propagation | boolean | default true | always; adds boundary edges tangent-connected to the selection |
| endCondition | Move end condition | enum ExtendBoundingType | Blind (BLIND, default), Up to face (UP_TO_FACE), Up to part/surface (UP_TO_BODY), Up to vertex (UP_TO_VERTEX) | always |
| oppositeDirection | Opposite direction | boolean | default false | Blind |
| extendDistance | Distance | length | LENGTH_BOUNDS -500 m .. 500 m, default 25 mm / 1 in; negative = trim | Blind |
| targetPart | Target | Query | BODY && (SOLID or SHEET), not sketch, mesh ok; max 1 | Up to part/surface |
| targetFace | Target | Query | (FACE && not sketch) or MATE_CONNECTOR; max 1 | Up to face |
| targetVertex | Target | Query | ALLOWS_VERTEX (vertex/point/mate connector origin); max 1 | Up to vertex |
| hasOffset | Offset distance (checkbox; column "Has offset") | boolean | default false | any Up to ... |
| offset | Offset distance | length | LENGTH_BOUNDS, default 25 mm / 1 in | hasOffset |
| offsetOppositeDirection | Opposite direction | boolean | default false | hasOffset |
| maintainCurvature | Maintain curvature | boolean | default false | always |

- **Behavior / semantics:**
  - Extension shape: Maintain curvature on -> ExtendSheetShapeType.SOFT (curvature-continuous extension); off -> LINEAR (tangent/ruled extension).
  - Blind: signed distance = Distance * (opposite ? -1 : 1). Positive -> opExtendSheetBody (extend outward by distance, measured along the surface from each boundary edge). Negative (or flipped) -> trim: opEdgeChange moves each tracked boundary edge inward on its adjacent face by |distance| (offset curve on the face).
  - Up to target: target is built as a temporary body: Up to face -> mate connector = its XY plane (offset along its Z); planar face = large plane; cylindrical face = large uncapped cylinder (offset changes plane origin or cylinder radius); other surface types = extracted copy of the face (offset face when offset is on); Up to vertex -> plane through the vertex whose normal is the extension direction of the selected boundary (edge with lowest id), offset along that normal; Up to part/surface -> the body itself, or with offset: sheet -> offset copy of its faces, solid -> copy with all faces offset (opOffsetFace). Temporary target bodies are deleted afterward.
  - Per owner body: if the surface does not collide with the target -> extend up to target (opExtendSheetBody EXTEND_TO_TARGET); if it collides -> trim back to the target via opEdgeChange with replaceFace = target face (only single-face targets supported for trim).
  - Linear manipulator for Blind distance.
- **Edge cases & errors:** EXTEND_SHEET_BODY_NO_BODY (nothing to move), EXTEND_SHEET_BODY_NO_TARGET, TRIM_TO_MULTI_FAILED / TRIM_TO_MULTI_FAILED_FOR_UPTO_BODY (trim to multi-face target), TRIM_FAILED, EXTEND_OFFSET_FAILED (negative cylinder radius after offset), EXTEND_TO_FACE_FAILED, EXTEND_TO_VERTEX_FAILED, EXTEND_NON_LAMINAR (non-boundary edge), SHEET_METAL_ACTIVE_MODEL_CANNOT_OFFSET; meshes rejected.
- **OCCT mapping:** Extend: convert face surface to B-spline (GeomConvert::SurfaceToBSplineSurface) and GeomLib::ExtendSurfByLength(surf, length, cont = 1 for LINEAR / 2 for SOFT(curvature), inU, after) on the side corresponding to the boundary; rebuild the face with the extended outer wire (BRepLib_MakeFace + new boundary edges from iso/pcurve offsets); analytic surfaces: just widen parameter range (BRepBuilderAPI_MakeFace with new UV bounds). Up to: extend generously then BRepAlgoAPI_Splitter / BRepAlgoAPI_Section with the target and keep the piece containing the original face (BRepClass/sample-point test). Trim (negative): offset boundary in UV (Geom2d offset of pcurve, or BRepOffsetAPI_MakeOffset for planar faces) and BRepAlgoAPI_Splitter / BRepFeat_SplitShape, delete outer strip. Gap: arbitrary-edge sheet extension with tangent propagation and SOFT shape needs custom code (OCCT only extends whole surface sides).
- **Priority:** P1

### SF-06 Boundary surface
- **Purpose:** Create or add a surface defined by up to two U and two V boundary profiles (four-sided patch), with per-profile boundary conditions (normal/tangent to profile, match tangent/curvature of faces, normal/tangent to a direction).
- **Sources:** https://cad.onshape.com/help/Content/PartStudio/boundary_surface.htm ; FS: `boundarySurface` (bsurf.fs).
- **Selection inputs:** each profile row: "Edges, curves and sketches" - (EDGE || (WIRE BODY && not sketch)), no mesh, not construction; multiple entities per profile (chained). U profiles array and V profiles array each "MaxNumberOfPicks" 2 (i.e. at most 2 profiles per direction; arrays collapsible). "Faces" (FACE, no mesh) for match conditions; "Direction" (ALLOWS_DIRECTION || MATE_CONNECTOR, max 1) for direction conditions.
- **Parameters:**

| FS id | UI label | Type | Enum values / default / bounds+units | Visible when / notes |
|---|---|---|---|---|
| surfaceOperationType | Result body operation type | enum | New (default) / Add | always |
| uProfilesArray[] / vProfilesArray[] | U profiles / V profiles | array (max 2 each) | label "[condition] entities" | always |
| {u,v}ProfileEntities | Edges, curves and sketches | Query | see filter | per row |
| {u,v}Condition | Boundary condition | enum BSurfEndDerivativeType | None (DEFAULT, default), Normal to profile, Tangent to profile, Match tangent, Match curvature, Normal direction, Tangent direction | per row |
| {u,v}Magnitude | Magnitude | real | CLAMP_MAGNITUDE_REAL_BOUNDS -1e5 .. 1e5, default 1 | condition != None |
| {u,v}Reference | Faces | Query | FACE, no mesh | Match tangent / Match curvature (optional; auto = faces adjacent to the profile edges) |
| {u,v}Direction | Direction | Query | ALLOWS_DIRECTION or MATE_CONNECTOR (z-axis); max 1 | Normal direction / Tangent direction |
| showIsocurves | Show isocurves | boolean | default false | always |
| curveCount | Count | integer | 1 .. 50, default 10 | showIsocurves |
| defaultSurfaceScope / booleanSurfaceScope | Merge with all / Merge scope | | SF-02 (default true) | Add |

- **Behavior / semantics:**
  - Requires at least 2 non-empty profiles in total (U+V). Surface interpolates all profiles: U profiles = surface "X" direction, V = "Y"; up to 2 per direction gives a 4-sided Coons/Gordon-type patch; resulting surface complexity follows the input curves (no extra spans added) - key quality advantage vs Loft/Sweep/Fill.
  - Conditions (each applied at that profile as a cross-boundary derivative, scaled by Magnitude; negative magnitude reverses):
    - None: position only (shortest-distance blend).
    - Normal to profile: derivative parallel to the profile's sketch-plane normal (profile must be sketch edges; else LOFT_NO_PLANE_FOR_START/END_CLAMP).
    - Tangent to profile: derivative lies in the profile's sketch plane (tangentToPlane).
    - Match tangent / Match curvature: G1/G2 to the given Faces or, if empty, faces adjacent to the profile's wire edges (LOFT_NO_FACE_FOR_START/END_CLAMP if none).
    - Normal direction: profile boundary normal to the selected direction (tangentToPlane w.r.t. direction vector); Tangent direction: derivative along the selected direction. (LOFT_NO_DIRECTION_FOR_START/END if direction unresolved.)
  - Add: auto-matching join; makeSolid true at FS version >= V2513 (closed result becomes a part).
  - Help video lists only None/Normal/Tangent/Match tangent; Steps list all 7 (help steps match the precondition).
- **Edge cases & errors:** BSURF_2_PROFILES (fewer than 2 profiles); LOFT_NO_* clamp errors above; profiles must intersect/touch to form a consistent network (kernel failure otherwise, inferred).
- **OCCT mapping:** 2 profiles in one direction only: BRepFill::Face / GeomFill::Surface (ruled) or BRepOffsetAPI_ThruSections (2 sections, ruled=false) with GeomFill_AppSurf; 2+2 or 2+1 network: GeomFill_BSplineCurves (GeomFill_CoonsStyle / StretchStyle / CurvedStyle) or GeomFill_ConstrainedFilling with GeomFill_SimpleBound (G0) and GeomFill_BoundWithSurf (G1 to adjacent face, via Adaptor3d_CurveOnSurface) / GeomFill_TgtField-based laws for direction conditions and magnitude scaling. G2 boundary conditions and Gordon interpolation are not in OCCT: port a Gordon surface implementation (e.g. TiGL's CTiglInterpolateCurveNetwork built on OCCT) or use BRepOffsetAPI_MakeFilling as fallback.
- **Priority:** P1

### SF-07 Ruled surface
- **Purpose:** Create a ruled surface from edges/curves: straight rulings of given distance emanating from the path at an angle to reference faces (Normal/Tangent), along a direction, or at an angle from a direction; with per-vertex overrides and corner treatment.
- **Sources:** https://cad.onshape.com/help/Content/PartStudio/ruled_surface.htm ; FS: `ruledSurface` (ruledSurface.fs).
- **Selection inputs:** "Edges and curves for ruled surface path": (EDGE, any) || (WIRE BODY, modifiable, not sketch); not construction; multiple. "Direction": ALLOWS_DIRECTION, max 1. "Reference faces": FACE, not sketch/construction, multiple. Vertex overrides: "Vertex" (VERTEX, not construction, max 1), override "Direction" (ALLOWS_DIRECTION, max 1), "Up to vertex or mate connector" (ALLOWS_VERTEX, max 1).
- **Parameters:**

| FS id | UI label | Type | Enum values / default / bounds+units | Visible when / notes |
|---|---|---|---|---|
| surfaceOperationType | Result body operation type | enum | New (default) / Add | always |
| edges | Edges and curves for ruled surface path | Query | see filter | always |
| ruledType | Ruled surface type | enum RuledSurfaceInterfaceType | Normal (NORMAL, default), Tangent (TANGENT), Aligned with direction (ALIGNED_WITH_VECTOR), Angle from direction (ANGLE_FROM_VECTOR) | always |
| axis | Direction | Query | ALLOWS_DIRECTION, max 1 | Aligned with direction / Angle from direction |
| angleFromVector | Angle | angle | ANGLE_360_90_DEFAULT_BOUNDS -1e5 .. 1e5 deg, default 90 deg (code default map 0) | Angle from direction |
| oppositeAngleFromVector | Opposite direction | boolean (circular flip) | default false | Angle from direction |
| angle | Angle | angle | ANGLE_360_ZERO_DEFAULT_BOUNDS -1e5 .. 1e5 deg, default 0 | Normal / Tangent |
| oppositeAngle | Opposite direction | boolean (circular flip) | default false | Normal / Tangent |
| distance | Distance | length | LENGTH_BOUNDS -500 .. 500 m, default 25 mm / 1 in | always |
| oppositeDirection | Opposite direction | boolean | default false | always |
| referenceFaces | Reference faces | Query | FACE | Normal / Tangent (auto-filled with an adjacent face of each picked edge; user can replace/add the opposite face) |
| vertexOverrides[] | Vertex overrides | array | rows labelled by vertex | always |
| .vertex | Vertex | Query | VERTEX, max 1 | per row |
| .overrideType | Ruled surface type | enum VertexOverrideType | Normal, Tangent, Aligned with direction, Up to vertex; new rows copy last | per row |
| .angleOverride / .oppositeAngleOverride | Angle / Opposite direction | angle / boolean | -1e5 .. 1e5 deg, default 0 | Normal / Tangent override |
| .axisOverride | Direction | Query | ALLOWS_DIRECTION, max 1 | Aligned override |
| .boundaryOverride | Up to vertex or mate connector | Query | ALLOWS_VERTEX, max 1 | Up to vertex override |
| .distanceOverride / .oppositeDirectionOverride | Distance / Opposite direction | length / boolean | LENGTH_BOUNDS default 25 mm | override type != Up to vertex |
| useCubicInterpolation | Use cubic interpolation | boolean | default true (help says linear by default - doc discrepancy) | always |
| showRuledLines | Show ruled lines | boolean | default false | always |
| ruledLineCount | Count | integer | RULED_LINE_COUNT_BOUNDS 1 .. 100, default 40 (per continuous path) | showRuledLines |
| cornerType | Corner type | enum RuledSurfaceCornerType | Spun corner (SPUN, default), Extended corner, Lofted corner, No corner | always |
| defaultSurfaceScope / booleanSurfaceScope | Merge with all / Merge scope | | SF-02 | Add |

- **Behavior / semantics:**
  - Path edges (wire edges followed to laminar sources) are chained into paths; each path edge gets an orientation and reference face(s): user Reference faces plus faces adjacent to one-sided (boundary) edges.
  - Normal/Tangent: kernel type ANGLE_FROM_FACE with angle = user angle * (opposite ? -1 : 1) + (Tangent ? 0 : -90 deg): Tangent = ruling lies in the reference face's tangent plane perpendicular to the edge (angle 0 continues the face), Normal = ruling along the face normal; Angle rotates the ruling about the edge tangent.
  - Aligned with direction: every ruling parallel to the direction vector (angle 0). Angle from direction: ruling at the given angle from the direction (rotated about the path tangent, inferred).
  - Width = Distance * (opposite ? -1 : 1) (signed length of each ruling).
  - Vertex overrides: at a path vertex replace the ruling definition (Normal/Tangent with own angle, Aligned with own direction, Up to vertex = ruling ends at that point/mate connector origin, distance ignored), with distance override; between vertices the ruling definition is interpolated linearly or cubically (Use cubic interpolation gives smoother transitions). Override vertices not on the path are snapped to a coincident path vertex (V2522+) else RULED_SURFACE_BAD_VERTEX.
  - Corners between consecutive edges with diverging rulings: Spun (rotational sweep of the ruling around the vertex), Extended (extend rulings to meet), Lofted (lofted patch), No corner (gap). Error if a corner cannot be built.
  - Manipulators: angle and distance per path plus per-override; directions that cannot produce a surface are drawn red.
  - Add: join with auto-matching, makeSolid false.
- **Edge cases & errors:** RULED_SURFACE_SELECT_EDGES, RULED_SURFACE_SELECT_DIRECTION, RULED_SURFACE_SELECT_UP_TO_ENTITY, RULED_SURFACE_EDGES_NOT_MANIFOLD (branching path), RULED_SURFACE_OVERLAPPING_SKETCH_EDGES, RULED_SURFACE_REFERENCE_FACES_BOTH_SIDES, RULED_SURFACE_EDGE_PARALLEL_REFERENCE (path edge parallel to the direction), RULED_SURFACE_GLOBAL_NORMAL_OVERRIDE, RULED_SURFACE_BAD_VERTEX, RULED_SURFACE_FAILED; self-crossing rulings (angle/distance) -> feature error, surface hidden.
- **OCCT mapping:** Custom construction: for each path edge sample a direction field d(t) (face normal from BRepLProp_SLProps / GeomLProp_SLProps on reference face, tangent from BRepAdaptor_Curve, rotate with gp_Trsf about tangent), build the second rail c2(t) = c1(t) + w*d(t) (exact for line/arc + constant directions, else GeomAPI_PointsToBSpline / interpolation), then GeomFill::Surface(c1, c2) or BRepFill::Face(e1, e2) for the ruled face; vertex-override interpolation of d(t) via linear or cubic law (Law_Interpol). Corners: Spun = BRepPrimAPI_MakeRevol of the end ruling about the vertex axis; Extended = intersect extended rulings (GeomAPI_ExtremaCurveCurve); Lofted = BRepOffsetAPI_ThruSections between end rulings. Sew results with BRepBuilderAPI_Sewing.
- **Priority:** P1

### SF-08 Constrained surface
- **Purpose:** Fit a surface through selected points (optionally with normals) or mesh data within a tolerance, optimized for performance or smoothness, with deviation display.
- **Sources:** https://cad.onshape.com/help/Content/PartStudio/constrained_surface.htm ; FS: `constrainedSurface` (constrainedSurface.fs).
- **Selection inputs:** Points mode: array rows, each "Vertex" (VERTEX, max 1), "Normal" (ALLOWS_DIRECTION || MATE_CONNECTOR, max 1). Mesh mode: "Mesh bodies or faces" ((FACE || BODY) && mesh geometry only).
- **Parameters:**

| FS id | UI label | Type | Enum values / default / bounds+units | Visible when / notes |
|---|---|---|---|---|
| csType | Type | enum ConstrainedSurfaceType (horizontal) | Points (POINTS, default), Mesh (MESH) | always |
| vertices[] | Vertices | array (collapsed, no reorder) | rows labelled by vertex | Points |
| .vertex | Vertex | Query | VERTEX, max 1 | per row |
| .normal | Normal | Query | direction or mate connector z-axis, max 1 | per row (optional) |
| .flipNormal | Flip normal | boolean | default false | per row |
| meshes | Mesh bodies or faces | Query | mesh faces/bodies | Mesh |
| tolerance | Tolerance | length | TOLERANCE_BOUND 1e-8 m .. 1 m, default 0.01 mm / 0.002 in | always |
| optimize | Optimize | enum OptimizationMethod | Performance (PERF, default), Smoothness (SMOOTH) | always |
| computeDeviation | Deviation | boolean | default false | always (deviationParameters) |
| maxDeviation | Maximum deviation | length (read-only) | computed | computeDeviation |
| showDeviation | Show deviation | boolean | default false | computeDeviation |
| deviationType | Deviation type | enum DeviationType | Max deviation (MAX), All deviations (ALL) | showDeviation |
| limit | Limit | length | NONNEGATIVE_ZERO_DEFAULT_LENGTH_BOUNDS 0 .. 500 m, default 0 | showDeviation; deviations below limit not drawn |

- **Behavior / semantics:**
  - Points: needs >= 3 resolved vertices; each point optionally carries a normal constraint (flipped if requested). Surface is created normal to the selected vertex normals (orientation control).
  - Mesh: all mesh vertex positions of the selected mesh faces/bodies are used as points.
  - Larger tolerance -> simpler, larger surface. Smoothness: lower curvature, more control points, slower. Performance: faster, may be lower quality.
  - Deviation: computes point-to-surface distances; Maximum deviation written back as computed (read-only) parameter; Show deviation draws max or all deviation lines above Limit (red lines).
  - Android cannot create (view/edit only) - platform note, ignore for native.
- **Edge cases & errors:** CONSTRAINED_SURFACE_TOO_FEW_POINTS (< 3 points), CONSTRAINED_SURFACE_SELECT_MESH.
- **OCCT mapping:** Point fitting: GeomPlate_BuildPlateSurface with GeomPlate_PointConstraint (G0; with normal -> G1 point constraint) + GeomPlate_MakeApprox(tolerance, maxSeg, maxDeg) ; for structured grids GeomAPI_PointsToBSplineSurface (with smoothing weights for "Smoothness"); scattered mesh points: custom least-squares B-spline fitting (parameterize by projection onto best-fit plane, then AppDef/GeomAPI_PointsToBSplineSurface or a smoothing solver). Deviation: GeomAPI_ProjectPointOnSurf / BRepExtrema_DistShapeShape. Mesh input requires a mesh body type in the noshape model (Poly_Triangulation).
- **Priority:** P2

### SF-09 Face blend
- **Purpose:** Blend two sets of faces (connected or disconnected, same or different bodies) with a rolling-ball or swept-profile cross section of circular, conic, curvature-continuous or chamfer shape, with hold lines, cliff edges, caps, limits, trimming options, and optional detached result.
- **Sources:** https://cad.onshape.com/help/Content/PartStudio/face_blend.htm ; FS: `faceBlend` (faceBlend.fs).
- **Selection inputs:** Side 1 / Side 2: FACE, not construction/sketch, modifiable, not active sheet metal; multiple faces but all from one body per side. Spine: EDGE, max 1. Tangent edges / Inverse tangent edges / Conic edges / Inverse conic edges / Cliff edges: EDGE, not sheet metal/construction/sketch, multiple (must lie on Side 1/Side 2 faces). Cap entity: FACE, max 1 per row. Plane limit / Second plane limit: GeometryType.PLANE, max 1. Face limits: FACE (modifiable, not SM). Edge limit: EDGE max 1 + Side: FACE max 1 per row. Help point: VERTEX || MATE_CONNECTOR, max 1.
- **Parameters:**

| FS id | UI label | Type | Enum values / default / bounds+units | Visible when / notes |
|---|---|---|---|---|
| side1 | Side 1 | Query | see above | always |
| flipSide1 | Flip side 1 | boolean | default false | always |
| side2 | Side 2 | Query | see above | always |
| flipSide2 | Flip side 2 | boolean | default false | always |
| propagation | Propagation | boolean | default true | always (group driver) |
| propagationType | Propagation type | enum FaceBlendPropagation | Tangent (default), Adjacent, Custom | propagation |
| propagationAngle | Maximum angle | angle | PROPAGATION_ANGLE_BOUNDS 0 .. 180 deg, default 0 | propagation && Custom |
| crossSection | Cross section | enum FaceBlendCrossSection | Rolling ball (default), Swept profile; REMEMBER | group "Cross section" |
| spine | Spine | Query | EDGE, max 1 | Swept profile |
| blendControlType | Measurement | enum BlendControlType | Radius (default), Width; REMEMBER | always |
| radius | Radius | length | BLEND_BOUNDS 1e-5 .. 500 m, default 5 mm / 0.2 in; REMEMBER | Radius |
| width | Width | length | BLEND_BOUNDS, default 5 mm / 0.2 in; REMEMBER | Width |
| asymmetric | Asymmetric | boolean | default false | always |
| secondRadius | Second radius | length | BLEND_BOUNDS | asymmetric && Radius |
| widthRatio | Ratio | real | RATIO_BOUNDS 0.0001 .. 10000, default 1 | asymmetric && Width |
| switchAsymmetric | Switch side ("Flip asymmetric") | boolean | default false | asymmetric |
| crossSectionShape | Control | enum FaceBlendCrossSectionShape | Circular (default), Conic, Curvature, Chamfer; REMEMBER | always |
| rho | Rho | real | FILLET_RHO_BOUNDS 0 .. 0.99999, default 0.5 | Conic |
| magnitude | Magnitude | real | FILLET_RHO_BOUNDS 0 .. 0.99999, default 0.5 | Curvature |
| tangentHoldLines | Tangent hold lines | boolean | default false | group "Constraints and limits" (collapsed) |
| tangentEdges / inverseTangentEdges | Tangent edges / Inverse tangent edges | Query | EDGE | tangentHoldLines |
| conicHoldLines | Conic hold lines | boolean | default false | always |
| conicEdges / inverseConicEdges | Conic edges / Inverse conic edges | Query | EDGE | conicHoldLines |
| hasCliffEdges | Cliff edges | boolean | default false | always |
| cliffEdges | Cliff edges | Query | EDGE | hasCliffEdges |
| hasCaps | Caps | boolean | default false | always |
| caps[].entity / caps[].capFlip | Cap entity / Flip cap | Query / boolean | FACE max 1 / default false | hasCaps (array) |
| hasLimits | Limits | boolean | default false | always |
| limitPlane1 / limitPlane1Flip | Plane limit / Flip first plane | Query / boolean | plane, max 1 | hasLimits |
| limitPlane2 / limitPlane2Flip | Second plane limit / Flip second plane | Query / boolean | plane, max 1 | hasLimits |
| faceLimits | Face limits | Query | FACE | hasLimits |
| edgeLimits[].edge / .edgeLimitSide | Edge limit / Side | Query / Query | EDGE max 1 / FACE max 1 | hasLimits (array) |
| hasHelpPoint | Help point | boolean | default false | always |
| helpPoint | Help point | Query | VERTEX or mate connector, max 1 | hasHelpPoint |
| trim | Trim type | enum FaceBlendTrimType | Walls (default), Short, Long, No trim; REMEMBER | always |
| detach | Detached | boolean | default false | always |
| showIsocurves / curveCount | Show isocurves / Count | boolean / integer | false / 1 .. 50 default 10 | always / showIsocurves |

- **Behavior / semantics:**
  - Side orientation: for adjacent concave side faces the flip arrows are aligned with face normals; otherwise flips select which side of each wall the ball rolls on (flipSideNNormal derived from convexity of the shared edge).
  - Propagation: Tangent - extend each side set to tangent-connected faces; Adjacent - to all adjacent faces; Custom - adjacent faces whose dihedral angle < Maximum angle. Tangent propagation is not allowed with Swept profile.
  - Cross section: Rolling ball (constant-radius ball contacting both sides, like fillet); Swept profile - section planes are orthogonal to the Spine edge (spine straight along the blend ~ rolling ball).
  - Measurement: Radius = ball/section radius; Width = constant chord width between the two contact lines. Asymmetric: Second radius (radius mode) or width Ratio (width mode); Switch side swaps radius/secondRadius or inverts ratio.
  - Control: Circular arc; Conic (rho shape factor); Curvature (G2-continuous section to both walls, Magnitude 0..0.999 adjusts tangency strength); Chamfer (straight bevel).
  - Hold lines (radius mode only): Tangent hold lines clamp the blend so its contact line follows the selected edge (symmetric on both sides; Inverse = clamp from the other side); Conic hold lines clamp only on the side of the selected edge (asymmetric; can mix conic and inverse conic per side). Cliff edges: when the blend would run past a face edge, the face is extended by default; cliff edge clamps the blend slope to the edge (alters blend angle). All hold-line/cliff edges must be edges of Side 1/Side 2 faces (no sketch edges; Split faces to create one).
  - Caps: faces/planes that close the blend's open ends (e.g. a perpendicular wall), flip chooses side; multiple caps.
  - Limits: up to two limiting planes (each with side flip), face limits (blend stops at faces), edge limits (edge + side face where blend is kept), multiple.
  - Help point: when several separate blends result, keep only the one closest to the point (vertex or mate connector origin).
  - Trim type: Walls - blend trimmed to the side walls' edges; Short - shortest blend between walls; Long - longest; No trim - blend extends beyond walls. Long/No trim are for support/detached surfaces.
  - Detached: blend created as a new surface (Parts list) instead of being attached/trimmed into the side bodies; useful when attaching fails or blends self-intersect.
  - Remembered values: Cross section, measurement, constraints/limits, trim type.
- **Edge cases & errors:** FACE_BLEND_SELECT_FACES (empty side), FACE_BLEND_LEFT_WALL_MULTIPLE_BODIES / FACE_BLEND_RIGHT_WALL_MULTIPLE_BODIES, FACE_BLEND_SOLID_LONG_ATTACH (Long/No trim on solid without Detached), FACE_BLEND_SELECT_SPINE, FACE_BLEND_SPINE_TANGENT_PROPAGATION, FACE_BLEND_SPINE_ZERO_ANGLE_PROPAGATION (Custom angle 0 with swept), FACE_BLEND_WIDTH_HOLD_LINES (hold lines/cliff edges not allowed with Width), FACE_BLEND_HOLD_LINE_IN_WALLS (hold line not on a side face), FACE_BLEND_EDGE_LIMIT_NEEDS_SIDE, FACE_BLEND_TANGENT_HL_ASYMMETRIC (documented: Asymmetric not allowed with tangent hold lines; note the stdlib check tests an empty query first and so effectively never fires - implement per help).
- **OCCT mapping:** Adjacent-face circular/chamfer cases: BRepFilletAPI_MakeFillet / BRepFilletAPI_MakeChamfer (ChFi3d). General disconnected rolling ball: compute spine as intersection of the two walls offset by R (BRepOffsetAPI_MakeOffsetShape + BRepAlgoAPI_Section), contact curves by projecting spine back (GeomAPI_ProjectPointOnSurf), then GeomFill_Pipe(path, curve1, curve2, radius) for circular section, or Blend_Walking with BlendFunc_ConstRad / BlendFunc_EvolRad / BlendFunc_Chamfer / BlendFunc_ConstThroat (internal ChFi3d API) for marching. Conic & curvature (G2) sections, width mode, hold lines, cliff edges: custom section laws (GeomFill_SectionGenerator / BRepOffsetAPI_MakePipeShell with law-driven section). Trimming/attachment: BRepAlgoAPI_Splitter of walls by contact curves + BRepBuilderAPI_Sewing. Large gap: expect custom implementation.
- **Priority:** P2

### SF-10 Mutual trim
- **Purpose:** Trim two intersecting (or nearly meeting) surfaces against each other by extending their intersection to completion, choosing the kept side of each, optionally merging into one surface.
- **Sources:** https://cad.onshape.com/help/Content/PartStudio/mutual_trim.htm ; FS: `mutualTrim` (mutualTrim.fs).
- **Selection inputs:** "First surface" and "Second surface": BODY && SHEET, modifiable, not sketch/construction, max 1 each. (Help speaks of "sets of surfaces"; precondition allows one sheet body per side - a multi-face sheet body is a contiguous set.)
- **Parameters:**

| FS id | UI label | Type | Enum values / default / bounds+units | Visible when / notes |
|---|---|---|---|---|
| body1 | First surface | Query | sheet body, max 1 | always |
| keepOtherSide1 | Keep opposite side | boolean (arrow, also clickable in graphics) | default false | always |
| body2 | Second surface | Query | sheet body, max 1 | always |
| keepOtherSide2 | Keep opposite side | boolean | default false | always |
| merge | Merge | boolean | default true; REMEMBER | always |

- **Behavior / semantics:**
  - opSplitFace(faceTargets = faces of body1, bodyTools = body2, keepToolSurfaces, extendToCompletion = true, mutualImprint = true): both surfaces are split along their intersection, with the intersection extended so each split runs edge-to-edge.
  - Faces on the non-kept side of each surface (determined from splitting edges and the keep flags) are deleted (opDeleteFace leaveOpen).
  - Merge: boolean UNION of both sheets into one surface (sewn along the trim edge); otherwise two trimmed surfaces remain.
  - If the split changes nothing -> info BOOLEAN_UNION_NO_OP and feature returns.
- **Edge cases & errors:** MUTUAL_TRIM_SURFACE_NOT_SELECTED, MUTUAL_TRIM_SAME_SURFACE_USED, MUTUAL_TRIM_GENERIC_ERROR, SPLIT_FACE_NO_CHANGE.
- **OCCT mapping:** Extend each surface's faces (GeomLib::ExtendSurfByLength) as temporary tools; BRepAlgoAPI_Splitter (arguments = sheet A, tools = extended sheet B, and vice versa) or BRepAlgoAPI_Section + BRepFeat_SplitShape; classify pieces by side of the other surface (sample point + normal sign via GeomAPI_ProjectPointOnSurf); delete unwanted pieces; merge with BRepBuilderAPI_Sewing (+ ShapeUpgrade_UnifySameDomain).
- **Priority:** P1

### SF-11 Enclose
- **Purpose:** Create a solid from the volume bounded by any set of intersecting/connected surfaces, faces, planes and parts, with boolean result options. (Also a 3A topic; included here because it is the primary surface-to-solid tool. If 3A specifies it, treat this as a cross-reference.)
- **Sources:** https://cad.onshape.com/help/Content/PartStudio/enclose.htm ; FS: `enclose` (enclose.fs).
- **Selection inputs:** "Entities": ((BODY && (SHEET || SOLID) && not sketch) || FACE) && mesh allowed; multiple (planes allowed as faces).
- **Parameters:**

| FS id | UI label | Type | Enum values / default / bounds+units | Visible when / notes |
|---|---|---|---|---|
| operationType | Result body operation type | enum NewBodyOperationType | New, Add, Remove, Intersect | always |
| entities | Entities | Query | see filter | always |
| defaultScope | Merge with all | boolean | default false | operationType != New |
| booleanScope | Merge scope | Query | solid bodies | not New && !defaultScope |
| keepTools | Keep tools | boolean | default false | always |

- **Behavior / semantics:** Computes closed region(s) bounded by the selected entities (entities must intersect or meet at boundaries; every edge of the enclosed volume bounds 2 faces). New -> new part(s); Add/Remove/Intersect with merge scope. Without Keep tools, the owning bodies of selections (not sketches or planes) are deleted.
- **Edge cases & errors:** no enclosed volume -> failure (enum names in enclose.fs not enumerated here).
- **OCCT mapping:** BOPAlgo_MakerVolume (builds solids from arbitrary intersecting faces/shells; planes trimmed to a large bounding face first), then BRepAlgoAPI_Fuse/Cut/Common for Add/Remove/Intersect.
- **Priority:** P1

### SF-12 Other surface operations (cross-references)
- **Purpose:** Track surface-relevant tools specified elsewhere so the surface tool palette is complete.
- **Sources:** surface_modeling.htm "Surfacing tools collection".
- **Selection inputs / Parameters:** see referenced items.
- **Behavior / semantics:**
  - Extrude / Revolve / Sweep / Loft with "Surface" creation type -> 3A (use SF-02 New/Add).
  - Thicken (surface -> solid) -> 3A.
  - Fillet (incl. on surfaces), Delete face (heal / cap / leave open; leave open converts solid faces to surface), Move face, Replace face (trim/extend face to a surface), Split (split part/face by surface/plane) -> 3A.
  - Section-surface tools: Onshape has no dedicated "section surface" feature; planar sections are done via Split/Intersection curve (CV-06).
  - Offset curve with "Offset, extend, and split" scope also splits faces (CV-11); Isocline with "Split faces" (CV-07).
- **Edge cases & errors:** n/a.
- **OCCT mapping:** see 3A items.
- **Priority:** P1

### CV-01 Curve bodies and shared "Approximate" parameters
- **Purpose:** Concept item: curve features create wire bodies ("Curves") with common display/selection behavior; most curve features share the curve re-approximation option group (`curveApproximationPredicate`).
- **Sources:** projected_curve.htm, composite_curve.htm, intersection_curve.htm, 3d_fit_spline.htm, offset_curve.htm (identical "Approximate" text); FS: `curveApproximationPredicate`, `approximateResults`, `checkApproximationParameters` (approximationUtils.fs).
- **Selection inputs:** n/a.
- **Parameters (curveApproximationPredicate; used by Projected curve, Composite curve, Intersection curve, 3D fit spline, Offset curve; Edit curve has its own copy, see CV-12):**

| FS id | UI label | Type | Enum values / default / bounds+units | Visible when / notes |
|---|---|---|---|---|
| approximate | Approximate | boolean | default false | always (group driver "Approximation parameters") |
| approximationDegree | Target degree | integer | DEGREE_BOUND 2 .. 15, default 3 | approximate |
| approximationMaxCPs | Maximum control points | integer | 4 .. 100 (MAX_CONTROL_POINTS), default 15 | approximate |
| approximationTolerance | Tolerance | length | TOLERANCE_BOUND 1e-8 m .. 1 m, default 0.01 mm / 0.002 in | approximate |
| keepStartDerivative | Keep start derivative | boolean | default false | approximate |
| keepEndDerivative | Keep end derivative | boolean | default false | approximate |
| approximationShowDeviation | Show deviation | boolean | default false | approximate (sub-group driver) |
| maxDeviation | Maximum deviation | length, read-only | computed | approximationShowDeviation |

- **Behavior / semantics:**
  - Curve features create wire bodies listed in Parts list > Curves; the curve feature itself cannot be shown/hidden - use the eye icon in the Curves list. Most curve features have `UIHint.NO_PREVIEW_PROVIDED` (no live preview of the final body; result appears on commit/regenerate) except where noted.
  - Approximate: each resulting wire body is chained into a path (tolerance 1e-5 m), sampled at APPROXIMATION_SAMPLES parameter values (positions, plus end tangents when Keep start/end derivative), and replaced by a single B-spline from `approximateSpline(degree, tolerance, isPeriodic = path closed, maxControlPoints)`. Original shown orange (some pages: yellow), approximation magenta while editing. Show deviation computes max path deviation (largest over all curves) into the read-only field and draws it.
  - Validation: degree >= 2 else EDIT_CURVE_APPROXIMATION_DEGREE_TOO_SMALL; closed path with keep derivative -> EDIT_CURVE_CLOSED_APPROXIMATION_NO_DERIVATIVE; maxCPs must be >= max(4, degree+1) + (closed ? degree-1 : 0) + (#kept derivatives), else error "Approximation needs at least N control points for a [closed ]curve of degree D."
- **Edge cases & errors:** above.
- **OCCT mapping:** Wire bodies = TopoDS_Wire/Edge compounds. Approximation: sample with GCPnts_UniformAbscissa on BRepAdaptor_CompCurve, then GeomAPI_PointsToBSpline(points, DegMin=degree, DegMax=degree, GeomAbs_C2, tol) with control-point cap enforced by iterating knot count, or AppDef_Variational with tangency constraints for kept derivatives; periodic: GeomAPI_Interpolate periodic / AppDef with periodic flag (custom). Deviation: GeomAPI_ProjectPointOnCurve sampling or BRepExtrema_DistShapeShape.
- **Priority:** P1

### CV-02 Helix
- **Purpose:** Create a helical (cylindrical, conical, or flat-spiral) curve around a cylinder/cone face, an axis (line, edge or mate connector Z), or a circular edge; defined by turns, pitch, or both.
- **Sources:** https://cad.onshape.com/help/Content/PartStudio/helix.htm ; FS: `helix` (helix.fs).
- **Selection inputs:** hidden "Initial Entities" (ALLOWS_AXIS) for preselection routing. "Conical or cylindrical face": FACE && ALLOWS_AXIS, max 1. "Helix axis": ALLOWS_AXIS (line/edge/cylinder axis/mate connector Z), max 1; implicit mate connector creation allowed. "Circular edge": CIRCLE || ARC, max 1. "Start point" / "End point": VERTEX, max 1.
- **Parameters:**

| FS id | UI label | Type | Enum values / default / bounds+units | Visible when / notes |
|---|---|---|---|---|
| initEntities | Initial Entities | Query | hidden | preselection only |
| axisType | Helix axis type | enum AxisType (horizontal) | Cylinder/Cone (SURFACE, default), Axis (AXIS), Circle (CIRCLE) | always |
| entities | Conical or cylindrical face | Query | max 1 | Cylinder/Cone |
| axis | Helix axis | Query | max 1 | Axis |
| edge | Circular edge | Query | max 1 | Circle |
| pathType | Input type | enum PathType | Turns (TURNS, default), Pitch (PITCH), Turns and pitch (TURNS_PITCH) | always |
| oppositeDirection | Opposite direction | boolean | default false | always |
| startType | Start condition | enum StartType | Start angle (default), Start point | always |
| oppositeOrigin | Opposite origin | boolean | default false | Axis |
| startAngle | Start angle | angle | ANGLE_360_ZERO_DEFAULT_BOUNDS -1e5 .. 1e5 deg, default 0 | Start angle |
| startRadius | Start radius | length | NONNEGATIVE_ZERO_INCLUSIVE_LENGTH_BOUNDS 0 .. 500 m, default 25 mm / 1 in | Start angle && Axis |
| startPoint | Start point | Query | VERTEX max 1 | Start point |
| endType | End condition | enum EndType | Height (default), End point | pathType != Turns and pitch |
| height | Height | length | NONNEGATIVE_LENGTH_BOUNDS 1e-5 .. 500 m, default 25 mm / 1 in | endType Height && axisType != Cylinder/Cone (surface uses face height) && pathType != TP |
| endRadToggle | End radius (checkbox) | boolean | default false | Height && Axis |
| endRadius | End radius | length | 0 .. 500 m, default 25 mm / 1 in | endRadToggle |
| endPoint | End point | Query | VERTEX max 1 | endType End point |
| revolutions | Target revolutions | real | HELIX_TURN_BOUNDS 1e-5 .. 1000, default 4 | Turns or Turns and pitch |
| actualRevolutions | Computed revolutions | real, read-only | computed | Turns && End point |
| roundUpTurns | Flip rounding | boolean | default false (round down) | Turns && End point |
| helicalPitch | Target pitch | length | NONNEGATIVE_LENGTH_BOUNDS 1e-5 .. 500 m, default 25 mm / 1 in | Pitch or Turns and pitch |
| actualPitch | Computed pitch | length, read-only | computed | Pitch && End point |
| roundUpPitch | Flip rounding | boolean | default false | Pitch && End point |
| handedness | Handedness | enum Direction | Clockwise (CW, first/default), Counterclockwise (CCW) | always |
| showStartEndProfiles | Show start and end profiles | boolean | default false | always (display circles at ends) |

- **Behavior / semantics:**
  - Local frame: Cylinder/Cone - surface coordinate system, origin moved to the bottom of the face's extent along the axis (bounding box in local cSys); height = face extent along axis; radii = cylinder radius, or cone radius at min/max z (tan(halfAngle)*z). Opposite direction swaps ends (runs top->bottom). Axis - axis origin shifted to one end of the axis entity (half its length; Opposite origin picks the other end); direction flips if oppositeDirection xor oppositeOrigin; start radius = Start radius or distance of Start point from axis. Circle - circle's coordinate system, radius = circle radius; Opposite direction flips Z.
  - The frame is canonically aligned (x-axis via alignCanonically) so Start angle 0 is a deterministic direction (help: "start of the revolve or the x-axis of an extruded circle").
  - Start: Start angle -> start point at angle on start radius in base plane; Start point -> vertex clamped to the start radius circle (must lie on it within tolerance, else HELIX_START_POINT_MISALIGNED); angle derived from it.
  - Turns + Height: revolutions = Target revolutions over the height. Pitch + Height: revolutions = height / pitch. Turns and pitch: height = pitch * revolutions (fixed, independent of surface; on a cone the end radius follows the cone's spiral rate since V2279); no end condition.
  - End point: end radius from the point (Axis) or surface; height from the point's axial position (on Cylinder/Cone, face height); direction flips if point is below start. Revolutions are rounded so the helix ends exactly at the end point's angle: fractional part fixed by start/end angle, integer part rounded down by default (Flip rounding = up); computed revolutions/pitch shown read-only; a zero-revolution result becomes 1. End point in the same plane as a circle -> flat spiral (zero height).
  - Varying radius: Axis with End radius, or conical faces -> conical helix (spiral pitch = (endRadius - startRadius)/revolutions). Direction is enforced toward a cone apex when one radius is zero; both radii zero -> HELIX_BOTH_RADII_ZERO.
  - Output: one wire body via opHelix(direction, axisStart, startPoint, spiralPitch, helicalPitch, interval [0, revolutions], clockwise). Handedness CW/CCW = turning sense viewed along +axis. Does not consume input. Manipulators: height arrow (Axis/Circle with Height) and direction arrow.
  - Typical use: sweep a circle on a Curve-point plane at the helix start to make a spring.
- **Edge cases & errors:** HELIX_INPUT_CONE (face not cone/cylinder), HELIX_INPUT_AXIS, HELIX_INPUT_CIRCLE, HELIX_START_POINT_MISALIGNED, HELIX_END_POINT_MISALIGNED, HELIX_BOTH_RADII_ZERO, FLATTENED_SHEET_METAL_SKETCH_PROHIBITED; mesh rejected. Legacy (pre-V1998) helix types Turns/Pitch/Height and turns/Height and pitch/Turns and pitch exist for old documents (HelixType enum).
- **OCCT mapping:** Constant radius: Geom_CylindricalSurface + Geom2d_Line (direction (2*pi, pitch) in (u,v)) -> BRepBuilderAPI_MakeEdge(line2d, surface, 0, 2*pi*turns) then BRepLib::BuildCurves3d (approximates to B-spline). Conical: same on Geom_ConicalSurface (constant v-rate gives constant axial pitch only if adjusted; compute v from height) or analytic sampling + GeomAPI_Interpolate. Flat spiral / general radius law: sample r(t)=r0+k*t, z(t) and fit GeomAPI_PointsToBSpline (tight tolerance) or use OCCT 7.9 TKHelix (HelixGeom_HelixCurve / HelixBRep_BuilderHelix supports taper) if available.
- **Priority:** P1

### CV-03 Projected curve
- **Purpose:** Create a 3D curve as the intersection of the extrusions of two sketches (Two sketches) or by projecting edges onto faces along a direction or normal to the target (Curve to face).
- **Sources:** https://cad.onshape.com/help/Content/PartStudio/projected_curve.htm ; FS: `projectCurves` (projectCurves.fs).
- **Selection inputs:** First sketch / Second sketch: EDGE && sketch && not construction (edges of one sketch each). Edges: EDGE && not construction. Direction: ALLOWS_DIRECTION, max 1. Targets: FACE || (BODY && (SOLID || SHEET) && not sketch).
- **Parameters:**

| FS id | UI label | Type | Enum values / default / bounds+units | Visible when / notes |
|---|---|---|---|---|
| preselectedEntities | Preselection | Query | hidden | routes preselected edges |
| curveProjectionType | Curve projection type | enum CurveProjectionType (horizontal) | Two sketches (default), Curve to face; REMEMBER | always |
| sketchEdges1 | First sketch | Query | sketch edges | Two sketches |
| sketchEdges2 | Second sketch | Query | sketch edges | Two sketches |
| dropTools | Edges | Query | edges | Curve to face |
| projectionType | Projection direction type | enum ProjectionType | Direction (DIRECTION, first), Normal to target (NORMAL_TO_TARGET) | Curve to face |
| directionQuery | Direction | Query | max 1 | Direction |
| oppositeDirection | Opposite direction | boolean | default false | Direction |
| targets | Targets | Query | faces or bodies | Curve to face |
| approximate ... | Approximate group | | CV-01 | always |

- **Behavior / semantics:**
  - Two sketches: each selection must come from a single sketch (PROJECT_CURVES_DIFFERENT_SKETCHES); sketch planes must not be parallel (PROJECT_CURVES_PARALLEL_PLANES). Sketch 1 edges extruded both ways along its normal by the diagonal of the combined bounding box; sketch 2 extruded through-all both ways; boolean INTERSECTION of the two sheet sets (allowSheets) -> result must be wire bodies only (>= 1) else REGEN_ERROR. Docs: projections must intersect in one contiguous curve.
  - Curve to face: opDropCurve(tools = non-construction edges, targets, projectionType, direction*(opposite?-1:1)); Normal to target projects each point along the target surface normal; results are wire bodies on the target faces. Direction manipulator (flip) shown for Direction mode.
  - Approximate replaces results (CV-01); with approximate on, raw curves are hidden.
  - Result in Curves list; feature has no preview.
- **Edge cases & errors:** CANNOT_RESOLVE_ENTITIES (empty selection fields), PROJECT_CURVES_DIFFERENT_SKETCHES, PROJECT_CURVES_PARALLEL_PLANES, REGEN_ERROR.
- **OCCT mapping:** Two sketches: BRepPrimAPI_MakePrism (both directions) for each sketch wire, BRepAlgoAPI_Section(prism1, prism2) with Approximation(true), chain via ShapeAnalysis_FreeBounds::ConnectEdgesToWires. Curve to face, direction: BRepProj_Projection(wire, targetShape, gp_Dir) (cylindrical projection; returns all solutions - pick/merge); normal: BRepOffsetAPI_NormalProjection(targetShape) Add(wire) with SetParams(tol3d, tol2d, GeomAbs_C2, maxDeg, maxSeg) and Build().
- **Priority:** P1

### CV-04 Bridging curve
- **Purpose:** Create a single Bezier curve connecting two locations (vertices, points, edges, faces, mate connectors) with independent end continuity G0..G3 and shape controls.
- **Sources:** https://cad.onshape.com/help/Content/PartStudio/bridging_curve.htm ; FS: `bridgingCurve` (bridgingCurve.fs).
- **Selection inputs:** Start / End: EDGE || VERTEX || (FACE && not construction) || MATE_CONNECTOR, max 2 picks each (e.g. vertex + edge, vertex + face, point + face).
- **Parameters:**

| FS id | UI label | Type | Enum values / default / bounds+units | Visible when / notes |
|---|---|---|---|---|
| preselectedEntities | Preselection | Query | hidden | |
| side1 | Start | Query | max 2 | always |
| match1 | Match | enum BridgingCurveMatchType | Match position (POSITION), Match tangent (TANGENCY, default), Match curvature (CURVATURE), Match flow (G3); REMEMBER | always |
| flip1 | Opposite direction | boolean | default false | match1 != position |
| side2 | End | Query | max 2 | always |
| match2 | Match (column "Second match") | enum | same, default Match tangent; REMEMBER | always |
| flip2 | Opposite direction | boolean | default false | match2 != position |
| method | Method | enum BridgingCurveMethod | Magnitude/bias (MAGNITUDE_BIAS, legacy), Control points (CONTROL_POINTS, UI default) ; REMEMBER | (match1 or match2 != position) && neither is G3 (G3 forces Control points) |
| editControlPoints | Edit control points | boolean | default false; REMEMBER | Control points method && any non-position match |
| side1Magnitude | Start magnitude | real | POSITIVE_REAL_BOUNDS 0 .. 1e5, default 1 | editControlPoints && match1 != position |
| side1CurvatureOffset | Start curvature offset | real | CLAMP_MAGNITUDE_REAL_BOUNDS -1e5 .. 1e5, default 1 | ... && match1 in {curvature, G3} |
| side1G3Offset | Start flow offset | real | -1e5 .. 1e5, default 1 | ... && match1 == G3 |
| side2Magnitude / side2CurvatureOffset / side2G3Offset | End magnitude / End curvature offset / End flow offset | real | same | mirror conditions on match2 |
| magnitude | Magnitude | real | 0 .. 1e5, default 1 | Magnitude/bias method && any non-position |
| bias | Bias | real | BIAS_BOUNDS 0.0001 .. 0.9999, default 0.5 | Magnitude/bias && both non-position |
| side1HasVertex / side2HasVertex | (hidden) | boolean | computed by edit logic | |
| editEdgePositions | Edit edge positions | boolean | default false; REMEMBER | a side has no vertex (edge-only pick) |
| startEdgeParameter / endEdgeParameter | Start edge position / End edge position | real | EDGE_PARAMETER_BOUNDS 0 .. 1, default 0.5 | editEdgePositions && that side has no vertex |
| side1HasFace / side2HasFace | (hidden) | boolean | computed | |
| editTangencyAngles | Edit tangency angles | boolean | default false; REMEMBER | a side has a face && any non-position |
| startAngle / endAngle | Start angle / End angle | angle | ANGLE_180_MINUS_180_BOUNDS -180 .. 180 deg, default 0 | editTangencyAngles && side has face && side non-position |

- **Behavior / semantics:**
  - Side resolution: position from vertex / mate connector origin / point on edge at Edge position (if edge only, Onshape guesses the end nearest the other side unless edge positions are edited); tangent/curvature frame from the edge (or the single edge at a vertex), from a face (tangent in face's tangent plane, rotated by tangency angle within +/-180 deg about the normal), or mate connector Z-axis. Vertex with several edges requires an explicit edge; the vertex must lie at an end of the chosen edge (position match, not topology).
  - Control-points method (current): curve degree = deg1 + deg2 + 1 where deg = 0/1/2/3 for G0/G1/G2/G3 (G0-G0 line, G1-G1 cubic, up to G3-G3 degree 7). Control points computed from end position, tangent (scaled by magnitude "speed"), curvature offset and G3 (normal derivative, k', etc.) data; G3 control points obtained via degree elevation from the G2 solution. With Edit control points, per-side triad/arrow manipulators and the control polygon are shown.
  - Magnitude/bias (legacy): Magnitude scales Onshape's default tangent length (0 -> straighter, larger -> "shoots out" faster); Bias weights tangent strength between start (near 0) and end (near 1).
  - Result: one wire body (Curves list); no preview hint.
- **Edge cases & errors:** BRIDGING_CURVE_NO_START_SELECTION, BRIDGING_CURVE_NO_END_SELECTION, BRIDGING_CURVE_POSITIONS_IDENTICAL, BRIDGING_CURVE_ONE_EDGE_EACH_SIDE, BRIDGING_CURVE_VERTEX_AT_END_OF_EDGE, BRIDGING_CURVE_VERTEX_BELONG_TO_FACE, BRIDGING_CURVE_EDGE_BELONG_TO_FACE, BRIDGING_CURVE_VERTEX_BOTH_SIDES, BRIDGING_CURVE_VERTEX_OR_EDGE_ON_SIDE, BRIDGING_CURVE_ZERO_SPEED_SCALE, FLATTENED_SHEET_METAL_SKETCH_PROHIBITED; meshes rejected.
- **OCCT mapping:** Geom_BezierCurve(poles) + BRepBuilderAPI_MakeEdge. End data from BRepAdaptor_Curve D1/D2/D3 or GeomLProp_CLProps (tangent, curvature, normal) and BRepLProp_SLProps for faces (normal, principal curvatures, normal curvature in tangent direction). Pole formulas for G0..G2 are standard (Hermite-to-Bezier); G3 (flow) poles need the derivative of curvature and torsion terms - custom math (port of `computeBridgingControlPoints`). Degree elevation: Geom_BezierCurve::Increase.
- **Priority:** P1

### CV-05 Composite curve
- **Purpose:** Represent multiple connected edges/curves as one curve body (one per connected chain), optionally re-approximated to a single B-spline.
- **Sources:** https://cad.onshape.com/help/Content/PartStudio/composite_curve.htm ; FS: `compositeCurve` (compositeCurve.fs).
- **Selection inputs:** "Edges": EDGE || (WIRE BODY && not sketch); multiple.
- **Parameters:**

| FS id | UI label | Type | Enum values / default / bounds+units | Visible when / notes |
|---|---|---|---|---|
| edges | Edges | Query | see filter | always |
| approximate ... | Approximate group | | CV-01 (default off) | always |

- **Behavior / semantics:** Selected wire bodies are dissolved into their edges; opExtractWires copies the edges into new wire bodies, one per connected chain (selections must meet at vertices; non-contiguous selections create multiple curves; curves must not intersect or overlap/branch). Use in Loft/Sweep etc. by picking the Curve in the Curves list. Composite curves built from surface edges keep the link to the underlying faces (used by Fill G1/G2, SF-04). To trim/extend sketch entities with Trim curve, convert them to a composite curve first.
- **Edge cases & errors:** EXTRACT_WIRES_NEEDS_EDGES (inferred shared error); meshes rejected.
- **OCCT mapping:** BRepBuilderAPI_Copy of edges, ShapeAnalysis_FreeBounds::ConnectEdgesToWires(edges, tol, shared=false) to group into wires (one wire body each), BRepBuilderAPI_MakeWire for ordered chains; approximation via CV-01 (or GeomConvert_CompCurveToBSplineCurve for exact concatenation).
- **Priority:** P1

### CV-06 Intersection curve
- **Purpose:** Create curves where faces/surfaces/parts in Group 1 intersect those in Group 2.
- **Sources:** https://cad.onshape.com/help/Content/PartStudio/intersection_curve.htm ; FS: `intersectionCurve` (faceIntersection.fs).
- **Selection inputs:** Group 1, Group 2: FACE || (BODY && (SOLID || SHEET) && not sketch); multiple; planes allowed (as faces).
- **Parameters:**

| FS id | UI label | Type | Enum values / default / bounds+units | Visible when / notes |
|---|---|---|---|---|
| group1 | Group 1 | Query | see filter | always |
| group2 | Group 2 | Query | see filter | always |
| approximate ... | Approximate group | | CV-01 | always |

- **Behavior / semantics:** Bodies expand to their faces. A face may not appear in both groups or be selected both as face and via its body (FACE_INTERSECTION_UNIQUE_SELECTION). opIntersectFaces computes intersections only between Group 1 and Group 2 entities (not within a group); may produce multiple curves (one wire body per connected intersection chain). Curves shown unless Approximate (then the approximations replace them). Patterning note: linear-patterning an intersection curve does not keep it on the surface unless the defining plane is also patterned with "Reapply features".
- **Edge cases & errors:** CANNOT_RESOLVE_ENTITIES, FACE_INTERSECTION_UNIQUE_SELECTION; no intersection -> no curves (failure, inferred).
- **OCCT mapping:** BRepAlgoAPI_Section(compound(group1 faces), compound(group2 faces)) with ComputePCurveOn1/2 and Approximation(true); chain result edges with ShapeAnalysis_FreeBounds::ConnectEdgesToWires; one wire body per wire.
- **Priority:** P1

### CV-07 Isocline
- **Purpose:** Create curves on faces where the surface has a given slope (angle) relative to a reference direction (draft/parting-line analysis), optionally splitting the faces.
- **Sources:** https://cad.onshape.com/help/Content/PartStudio/isocline.htm ; FS: `isocline` (isocline.fs).
- **Selection inputs:** Faces: FACE, not construction/sketch, multiple. Direction: ALLOWS_DIRECTION, max 1.
- **Parameters:**

| FS id | UI label | Type | Enum values / default / bounds+units | Visible when / notes |
|---|---|---|---|---|
| faces | Faces | Query | see filter | always |
| direction | Direction | Query | max 1 | always |
| oppositeDirection | Opposite direction | boolean | default false | always |
| angle | Angle | angle | ANGLE_STRICT_90_BOUNDS 0 .. 89.9 deg, default 3 deg | always |
| split | Split faces | boolean | default false | always |

- **Behavior / semantics:** Direction vector (flipped if requested) and angle passed to opCreateIsocline: the isocline is the locus on the faces where the angle between the surface and the reference (draft angle relative to the direction) equals Angle, i.e. where normal . d = sin(angle) (inferred formula). Result: curves (wire bodies). With Split faces: opSplitByIsocline splits the faces along the isocline instead of creating curves; if nothing splits -> warning ISOCLINE_NO_RESULT.
- **Edge cases & errors:** ISOCLINE_SELECT_FACES, ISOCLINE_SELECT_DIRECTION (missing or zero-length direction), ISOCLINE_NO_RESULT (warning).
- **OCCT mapping:** Contap_Contour (direction + angle variant computes draft-angle contour lines on a surface - used by HLR) per face with BRepAdaptor_Surface, then walking lines -> GeomAPI_PointsToBSpline / Approx; clip to face via BRepAlgoAPI_Section with the face or Geom2dHatch; split: BRepFeat_SplitShape / BRepAlgoAPI_Splitter with the isocline edges. Partial gap: robust multi-face handling custom.
- **Priority:** P2

### CV-08 Isoparametric curve
- **Purpose:** Create curves along a face's U or V parameter direction, equally spaced or at given parameter locations/points.
- **Sources:** https://cad.onshape.com/help/Content/PartStudio/isoparametric_curve.htm ; FS: `isoparametricCurve` (isoparametricCurve.fs).
- **Selection inputs:** Select face: FACE, not construction/sketch, max 1 (help text says faces plural; precondition single). Select point: VERTEX || MATE_CONNECTOR, max 1 per position row.
- **Parameters:**

| FS id | UI label | Type | Enum values / default / bounds+units | Visible when / notes |
|---|---|---|---|---|
| face | Select face | Query | max 1 | always |
| directionType | Direction | enum DirectionType | U direction (default), V direction | always |
| equalSpacing | Equal spacing | boolean | default false (inferred: first-created unchecked) | always |
| nCurves | Instance count | integer | 2 .. 50, default 10 | equalSpacing |
| uvParamList[] | Position (item "Curve", "Add Curve") | array | | !equalSpacing |
| .selectPoint | Select point | boolean | default false | per row |
| .point | Select point | Query | vertex / mate connector, max 1 | selectPoint |
| .uvValue | Location | real | EDGE_PARAMETER_BOUNDS 0 .. 1, default 0.5 | !selectPoint |

- **Behavior / semantics:** U direction curves "run in the X direction of the surface" (DIR1 iso), V in Y (DIR2). Location is a normalized face parameter 0..1 (0.5 = middle of face). Point rows: the point is projected onto the face (closest point, since V2664 the point need not lie on the face) and its U (or V) parameter used. Equal spacing: kernel auto-spaced isos (DIRx_AUTO_SPACED_ISO) with Instance count curves. Curves are clipped to the face boundary; each curve is a separate wire body (named curve0..n). Empty point rows are skipped with warning ISOPARAMETRIC_CURVE_SELECT_POINT. Typical uses: split face with the curves, Delete face, then rebuild with ruled surface/fit splines/loft. Numeric fields: scroll = +/-1.0, Ctrl+scroll = 0.01.
- **Edge cases & errors:** ISOPARAMETRIC_CURVE_SELECT_SINGLE_FACE, ISOPARAMETRIC_CURVE_SELECT_FACE, ISOPARAMETRIC_CURVE_SELECT_POSITION (no rows), ISOPARAMETRIC_CURVE_SELECT_POINT (warning), ISOPARAMETRIC_CURVE_POINT_NOT_ON_FACE (legacy).
- **OCCT mapping:** BRepTools::UVBounds(face) to normalize; Geom_Surface::UIso(u)/VIso(v) (or Adaptor3d_IsoCurve), clipped to the face domain with Geom2dHatch_Hatcher over the face's pcurves (as in DBRep isos / BRepTools isolines) -> BRepBuilderAPI_MakeEdge per in-domain interval; point -> GeomAPI_ProjectPointOnSurf / ShapeAnalysis_Surface::ValueOfUV. Auto spacing distribution: assume i/(n+1) of UV range (inferred; Onshape's exact rule undocumented).
- **Priority:** P2

### CV-09 3D fit spline
- **Purpose:** Create a 3D interpolating spline through ordered vertices (open or closed, optional end tangents/curvature), or a single spline replacing a tangent chain of edges.
- **Sources:** https://cad.onshape.com/help/Content/PartStudio/3d_fit_spline.htm ; FS: `fitSpline` (fitSpline.fs).
- **Selection inputs:** hidden "Entities" (VERTEX || EDGE) for preselection routing. Vertices: VERTEX, ordered (`ALLOW_QUERY_ORDER`, reorder UI "Reorder items" with grip handles). Start/End direction: EDGE || ALLOWS_DIRECTION (lines, edges, curves, planes, mate connectors), max 1. Edges: EDGE (tangent-continuous chain).
- **Parameters:**

| FS id | UI label | Type | Enum values / default / bounds+units | Visible when / notes |
|---|---|---|---|---|
| initEntities | Entities | Query | hidden | preselection |
| fitType | Approximation type (tabs "Vertices"/"Edges") | enum FitSplineType (horizontal) | Vertices (default), Edges | always |
| vertices | Vertices | Query (ordered) | VERTEX | Vertices |
| closed | Closed spline | boolean | default false | Vertices |
| startDirection | Start direction | Query | max 1 | Vertices && !closed |
| startMagnitude | Start magnitude | real | CLAMP_MAGNITUDE_REAL_BOUNDS -1e5 .. 1e5, default 1 | Vertices && !closed |
| oppositeDirectionStart | Opposite direction | boolean | default false | Vertices && !closed |
| matchStartCurvature | Match curvature at start | boolean | default false | Vertices && !closed |
| endDirection / endMagnitude / oppositeDirectionEnd / matchEndCurvature | End direction / End magnitude / Opposite direction / Match curvature at end | | same as start | Vertices && !closed |
| hasStartDirection / hasEndDirection | (hidden) | boolean | computed | |
| edges | Edges | Query | EDGE | Edges |
| approximate ... | Approximate group | | CV-01 | always |

- **Behavior / semantics:**
  - Vertices: points in selection order; closed requires >= 3 points (first point appended at end), open >= 2. Interpolation uses centripetal knot spacing (sum of sqrt distances). If a start/end direction is given, end derivative = direction * magnitude scaled to the parameterization (magnitude sign flipped by Opposite direction; zero magnitude errors); Match curvature uses the curvature of the edge/face selected as direction (needs a direction; FIT_SPLINE_NEED_DIRECTION_FOR_CURVATURE). Without directions, natural end conditions (position only). Magnitude manipulators (arrows) at ends.
  - Edges: opSplineThroughEdges builds one spline approximating the tangent-continuous edge chain.
  - Result is a curvature-continuous curve in Curves list. Uses: sweep paths, loft guides, routing.
- **Edge cases & errors:** CLOSED_SPLINE_THREE_POINTS, SPLINE_TWO_POINTS, FIT_SPLINE_REPEATED_POINT (all points coincident), FIT_SPLINE_ZERO_START_MAGNITUDE, FIT_SPLINE_ZERO_END_MAGNITUDE, FIT_SPLINE_CANNOT_EVALUATE_END_CONDITION, FIT_SPLINE_CANNOT_EVALUATE_CURVATURE_END_CONDITION, FIT_SPLINE_CURVATURE_FACE, FIT_SPLINE_NEED_DIRECTION_FOR_CURVATURE, TANGENCY_ONE_EDGE, FLATTENED_SHEET_METAL_SKETCH_PROHIBITED; meshes rejected for directions.
- **OCCT mapping:** GeomAPI_Interpolate(points, parameters (centripetal, computed by app), periodic=closed, tol) with Load(startTangent, endTangent, scale=false) for directions; curvature end conditions not supported by GeomAPI_Interpolate -> AppDef_Variational / AppDef_MultiPointConstraint with curvature constraints, or solve the B-spline linear system with extra rows (custom). Edges mode: GeomConvert_CompCurveToBSplineCurve (exact join of tangent chain) then optional approximation.
- **Priority:** P1

### CV-10 Routing curve
- **Purpose:** Interactively build a multi-point 3D path (interpolated spline or polyline with bend radii) from triad-placed points referenced to the origin, vertices, curves or previous points; import points from vertices/curves/CSV; edit segments (subdivide, orthogonalize). Used for pipe/wire routing and advanced surfacing curves.
- **Sources:** https://cad.onshape.com/help/Content/PartStudio/routing_curve.htm ; FS: `routingCurve` (routingCurve.fs). Feature name template "Routing curve (#curveLength)".
- **Selection inputs:** hidden preselection (EDGE || WIRE BODY || VERTEX || MATE_CONNECTOR). Input Vertices: VERTEX || MATE_CONNECTOR. Input Edges: EDGE || (WIRE BODY && not sketch). Curve reference: VERTEX || MATE_CONNECTOR, max 1 (default origin). Point reference Vertex: VERTEX || MATE_CONNECTOR, max 1. Point reference Edges: EDGE || WIRE BODY. Derivative direction: ALLOWS_DIRECTION || MATE_CONNECTOR, max 1. Edit reference (multi): VERTEX || MATE_CONNECTOR, max 1. Ortho cSys: VERTEX || MATE_CONNECTOR, max 1.
- **Parameters:**

| FS id | UI label | Type | Enum values / default / bounds+units | Visible when / notes |
|---|---|---|---|---|
| curveType | Curve type | enum RoutingCurveType | Interpolated spline (default, cubic), Polyline | always |
| bendRadius | Bend radius | length | NONNEGATIVE_ZERO_DEFAULT_LENGTH_BOUNDS 0 .. 500 m, default 0 (sharp) | Polyline |
| step | Step | enum CurveStep (horizontal, unconfigurable) | Points (default), Segments | always |
| inputType | Input type (group "Inputs") | enum InputType | Vertex, Curve, CSV file | Points |
| inputVertices | Vertices | Query | | Points && Vertex |
| inputCurve | Edges | Query | | Points && Curve |
| samples | Number of samples | integer | CURVE_SAMPLE_BOUNDS 2 .. 50, default 5 | Points && Curve |
| inputData | Table (Select CSV file) | TableData | CSV from current/other document | Points && CSV |
| startColumn | Data start column (0,1,2,.) | integer | 0 .. 1e5, default 0 | Points && CSV |
| csvUnit | Unit | enum LengthUnitNames | default Meter (Centimeter, Foot, Inch, Millimeter, Meter, Yard) | Points && CSV |
| overwritePoints | Overwrite points | boolean | default true | Points && (Curve or CSV) |
| processInputs | Process | button | | Points && (Curve or CSV) |
| isPeriodic | Closed | boolean | default false | Points |
| curveBaseCSys | Curve reference | Query | default = origin | Points |
| pointsEditType | Point edit type | enum PointsEditType (horizontal) | Single (default), Multiple | Points |
| multiEditReference | Edit reference | Query | max 1 | Multiple |
| selectedIndices[].indexValue | Selected points / Point index | integer array | 0 .. 10000 | Multiple |
| multiDx/Dy/Dz, multiRotationMatrix | (hidden) | length / matrix | triad state | Multiple |
| pointIndex | Selected Point | integer (hidden) | 0 .. 10000 | Single |
| points[] | Points ("Point #index", CLEAR, x delete, Add point) | array (no reorder) | | Single |
| points[].referenceType | Reference | enum ReferenceType | Origin (default), Vertex, Curve, Relative (not for first point) | per point |
| points[].vertex | Vertex | Query | max 1 | Vertex |
| points[].curve / curveParameter | Edges / Parameter | Query / real | 0 .. 1, default 0.5 | Curve |
| points[].alignCSys / flipCSys | Align coordinate system / Flip coordinate system | boolean | default false | Curve / alignCSys |
| points[].dx, dy, dz | X offset / Y offset / Z offset | length | ZERO_DEFAULT_LENGTH_BOUNDS -500 .. 500 m, default 0 | per point (in reference cSys) |
| points[].rotationMatrix | (hidden) | matrix | triad basis | |
| points[].overrideBendRadius / pointBendRadius | Override bend radius / Bend radius | boolean / length | false / 0 .. 500 m default 0 | Polyline |
| points[].hasDerivative | Derivative | boolean | default false | Interpolated spline |
| points[].derivativeAlignment | Derivative alignment | enum DerivativeAlignment | Manipulator X axis, Manipulator Y axis, Manipulator Z axis, Direction | hasDerivative |
| points[].derivativeDirection | Derivative direction | Query | max 1 | Direction |
| points[].magnitude / oppositeDirection | Magnitude / Opposite direction | real / boolean | -1e5 .. 1e5 default 1 / false | hasDerivative |
| resetTriad | Reset triad | button | | Points |
| addPointOnAxisDrag | Add point on axis drag | boolean | default false | Points && Single |
| showCSys | Show reference coordinate system | boolean | default true | Points |
| keepPoints | Keep points | boolean | default false | Points (creates point bodies, hidden by default) |
| segmentIndex | Segment index | integer (hidden) | -1 .. 10000, default -1 | Segments |
| segmentEditType | Segment edit type | enum SegmentEditType | Add points (default), Orthogonal path | Segments |
| numPointsToAdd | Number of points to add | integer | 0 .. 10000, default 1 | Add points |
| orthoCsys | Coordinate system | enum OrthoCoordSystem | World, Curve, Other | Orthogonal path |
| orthoCsysMate | Orthogonal path coordinate system | Query | max 1 | Other |
| solutionIndex | (hidden) | integer | 0 .. 5 (up to 6 axis orders) | Orthogonal path |
| orthoPrevious / orthoNext / confirmOrthoPath | Previous / Next / Confirm | buttons | cycle and apply orthogonal solutions | Orthogonal path |
| hiddenReferences | (hidden) | Query | dependency tracking | Segments |
| curveLength | Curve length | length (hidden, computed) | shown in feature name | |

- **Behavior / semantics:**
  - Triad manipulator (translate 3 axes, rotate 3 axes, translate in 3 planes, origin) positions the current point; drags show distance/angle readout with numeric entry. With Add point on axis drag, each axis drag appends a point.
  - Point position = reference cSys (origin / vertex or mate connector / point on curve at parameter with optional tangent-aligned cSys / previous point for Relative) + (dx,dy,dz). Points are ordered by index; coincident consecutive points removed before building.
  - Inputs processing (button): vertices -> one point each; edges -> `samples` evenly spaced points; CSV -> rows of X,Y,Z starting at the data start column in the chosen unit, relative to the curve reference; Overwrite replaces previously processed points (sketch points on edges not overwritten).
  - Curve: Interpolated spline = cubic interpolation through points with optional per-point derivatives (direction from triad axis or selected direction, magnitude scaled by bounding-box diagonal); Polyline = straight segments with fillet arcs of global or per-point bend radius at corners (0 = sharp). Closed: periodic (spline) / closing segment with bend radii at the seam (polyline).
  - Segments step: segments drawn magenta with midpoint manipulators; Add points subdivides a segment n times; Orthogonal path replaces a non-axis-aligned segment with axis-aligned legs in World/Curve/Other cSys, cycling among up to 6 axis orders (Previous/Next) and Confirm; undo supported.
  - Curve length is computed and displayed in the feature name (#curveLength, removable). Routing curve table panel: lists all routing curves; "LRA" checkbox shows length/rotation/angle data for polylines; column move/hide, copy table.
  - Help mentions a "Target length" option (curve adjusts to reach a total length) that is not in the 2026-05 stdlib precondition - newer than the mirror; treat as P2 extension.
  - Routing curves cannot be driven by variables directly; associate points to variable-driven geometry instead.
- **Edge cases & errors:** ROUTING_CURVE_AT_LEAST_TWO_DISTINCT_POINTS (open), ROUTING_CURVE_AT_LEAST_THREE_DISTINCT_POINTS (closed), ROUTING_CURVE_CSV_INVALID_DATA, ROUTING_CURVE_CSV_NOT_ENOUGH_COLUMNS, ROUTING_CURVE_INVALID_PATH, ROUTING_CURVE_ORTHO_PATH_ALREADY_AXIS_ALIGNED, ROUTING_CURVE_SELECT_CURVE.
- **OCCT mapping:** Spline: GeomAPI_Interpolate(points, periodic) with Load(tangents, flags) for per-point derivatives. Polyline: BRepBuilderAPI_MakePolygon; corner arcs per vertex with ChFi2d_FilletAPI / ChFi2d_AnaFilletAlgo on the plane of the two adjacent segments (or analytic arc construction GC_MakeArcOfCircle), assemble with BRepBuilderAPI_MakeWire. Length: GCPnts_AbscissaPoint::Length. CSV parsing, triad, point table, ortho solutions: app-level code.
- **Priority:** P2

### CV-11 Offset curve (offset curve on face)
- **Purpose:** Create curves on faces offset from selected edges by a geodesic or Euclidean distance, optionally extended to the face boundaries, trimmed, or used to split the faces.
- **Sources:** https://cad.onshape.com/help/Content/PartStudio/offset_curve.htm ; FS: `offsetCurveOnFace` (offsetCurveOnFace.fs).
- **Selection inputs:** Edges: EDGE, not construction/sketch, not wire-body edges; multiple (chains; branching not allowed). Targets: FACE, not construction/sketch, modifiable; multiple (empty = all faces).
- **Parameters:**

| FS id | UI label | Type | Enum values / default / bounds+units | Visible when / notes |
|---|---|---|---|---|
| edges | Edges | Query | see filter | always |
| distance | Distance | length | NONNEGATIVE_LENGTH_BOUNDS 1e-5 .. 500 m, default 25 mm / 1 in | always |
| offsetType | Offset type | enum OffsetCurveType | Geodesic (default), Euclidean | always |
| oppositeDirection | Flip | boolean | default false | always |
| gapFill | Gap fill | enum GapFill | Linear (default), Round | always |
| scope | Scope | enum OffsetCurveScope | Offset (default), Offset and extend, Offset, extend, and split | always |
| trim | Trim | boolean | default false | Offset and extend |
| startTrim | Start trim | length | 0 .. 500 m, default 0 | trim |
| equalTrim | Equal trim | boolean | default false (inferred) | trim |
| endTrim | End trim | length | 0 .. 500 m, default 0 | trim && !equalTrim |
| targets | Targets | Query | faces | always |
| approximate ... | Approximate group | | CV-01 | scope != Offset, extend, and split |

- **Behavior / semantics:** opOffsetCurveOnFace(edges, oppositeDirection, extend = scope != Offset, imprint = scope == split, distance, offsetType, targets, roundedCorners = gapFill Round, trims). Geodesic = distance measured within the 2D space of the target faces; Euclidean = 3D straight-line distance. Flip offsets to the other side of the edge (onto the other adjacent face). Gap fill fills gaps between offset segments at convex corners with a straight edge (Linear) or a tangent arc (Round). Offset and extend: curve ends extended to the face edges; Trim shortens start/end by distances (Equal trim = one manipulator for both). Offset, extend, and split: faces are split, no curve created. Separate offset curves may cross within the same faces. Results: wire bodies (or split faces).
- **Edge cases & errors:** OFFSET_CURVE_ON_FACE_CLOSED_CURVE_NO_TRIM (trim on closed curve), TRIM_FAILED; branching edges -> error.
- **3D offset curve:** Onshape has no standalone 3D (non-face) offset-curve feature; planar offsets are done in Sketch (Offset tool, sketch section).
- **OCCT mapping:** Planar faces: BRepOffsetAPI_MakeOffset(face, GeomAbs_Arc for Round / GeomAbs_Intersection for Linear) on the edge wire, then BRepAlgoAPI_Common/Section with the face. Euclidean on curved faces: section of the face with a pipe/tube of radius d around the edge chain (BRepOffsetAPI_MakePipe of a circle or BRepOffsetAPI_MakeOffsetShape on the edge), BRepAlgoAPI_Section. Geodesic: no OCCT equivalent -> custom geodesic offset (march in UV using surface metric, or BRepOffsetAPI_MakeOffset in UV space with metric correction). Split: BRepFeat_SplitShape / BRepAlgoAPI_Splitter. Extension to boundary: GeomLib::ExtendCurveToPoint + section with face boundary.
- **Priority:** P2

### CV-12 Edit curve
- **Purpose:** Modify an existing curve (or a composite of selected edges) while keeping downstream associativity: re-approximate, elevate degree, planarize, and edit control points (position relative to references, weights).
- **Sources:** https://cad.onshape.com/help/Content/PartStudio/edit_curve.htm ; FS: `editCurve` (editCurve.fs).
- **Selection inputs:** Curves: EDGE || (WIRE BODY && not sketch); multiple (must form a continuous non-self-intersecting path; multiple curves require Approximate, producing a new curve). Reference plane entity: ALLOWS_PLANE (plane, planar face, mate connector XY), max 1. Control point Reference: ALLOWS_VERTEX, max 1.
- **Parameters:**

| FS id | UI label | Type | Enum values / default / bounds+units | Visible when / notes |
|---|---|---|---|---|
| wire | Curves | Query | see filter | always |
| approximate | Approximate | boolean | default false | always |
| approximationDegree | Target degree (column "Approximation target degree") | integer | 2 .. 15, default 3 | approximate |
| approximationMaxCPs | Maximum control points | integer | 4 .. 100, default 15 | approximate |
| approximationTolerance | Tolerance | length | 1e-8 m .. 1 m, default 0.01 mm / 0.002 in | approximate |
| keepStartDerivative / keepEndDerivative | Keep start derivative / Keep end derivative | boolean | default false | approximate |
| elevate | Elevate | boolean | default false | always |
| elevationDegree | Target degree (column "Elevation target degree") | integer | 2 .. 15, default 3 | elevate |
| planarize | Planarize | boolean | default false | always |
| planeReference | Reference plane | enum PlaneReference | Best fit (BEST, default), YZ plane, XZ plane, XY plane, Custom | planarize |
| referencePlane | Entity or mate connector | Query | max 1 | Custom |
| lockEnds | Lock ends | boolean | default false | Best fit |
| editControlPoints | Edit control points | boolean | default false | always |
| editPointMode | Edit mode | enum EditPointMode | XYZ (default), UVN (Tangent), UVN (Control polygon) | editControlPoints |
| selectedIndices[].indexValue | Control point indices | integer array | CONTROL_POINT_INDEX_BOUND 0 .. 99 | XYZ mode (multi-select) |
| selectedIndex | Control point index | integer | 0 .. 99, default 0 | UVN modes |
| controlPointEdits[] | Points overrides ("#index: #x;#y;#z", CLEAR, x) | array (no reorder, collapsed) | one override per control point | editControlPoints |
| .index | Control point index | integer | 0 .. 99 | per override |
| .reference | Reference | Query | vertex / mate connector, max 1 (default = original control point) | per override |
| .x, .y, .z | X offset / Y offset / Z offset | length | -500 .. 500 m, default 0 | per override |
| .weight | Weight | real | SCALE_BOUNDS 1e-5 .. 1e5, default 1 (multiplier) | per override |
| showDetails | Show details | boolean | "Default" true in annotation (code default map false) | always |
| curveDegree / curveNumCPs / curveNumSpans | Degree / Control points / Spans | integer, read-only | computed | showDetails |
| approximationShowDeviation / maxDeviation | Show deviation / Maximum deviation | boolean / read-only length | false | always |

- **Behavior / semantics:** Operations apply in fixed order: Approximate -> Elevate -> Planarize -> Edit control points (create several Edit curve features to change order). Approximate rebuilds the curve to degree/CP/tolerance (geometry approximately preserved). Elevate raises degree to the target (exact shape preserved; more control points). Planarize projects control points onto: best-fit plane through the average of control points (Lock ends keeps end points fixed; not for periodic curves), YZ/XZ/XY world planes, or a custom plane/mate connector XY (e.g. helix -> spiral). Edit control points: triad at the selected control point (index 0 = first); "Add point override" creates an override row (current index or next unused); offsets are relative to the original control point or to the Reference entity (so the point follows referenced geometry across configurations); Weight multiplies the point's rational weight (pulls curve toward it). XYZ mode = world axes; UVN (Tangent) / UVN (Control polygon) = local frames along curve tangent or control-polygon leg (inferred from names). The edited curve replaces the original body in place (downstream references preserved) when a single curve is selected.
- **Edge cases & errors:** EDIT_CURVE_SELECT_WIRE, EDIT_CURVE_MULTIPLE_EDGES (multiple curves without Approximate), EDIT_CURVE_DEGREE_TOO_HIGH (elevation target below current degree / above max), EDIT_CURVE_INDEX_TOO_LARGE, EDIT_CURVE_TOO_MANY_CONTROL_POINTS, EDIT_CURVE_NO_BEST_FIT, EDIT_CURVE_LOCK_ENDS_PERIODIC, EDIT_CURVE_SELECT_PLANE, CANNOT_RESOLVE_PLANE, EXTRACT_WIRES_NEEDS_EDGES, plus CV-01 approximation errors.
- **OCCT mapping:** GeomConvert::CurveToBSplineCurve; approximation as CV-01; Geom_BSplineCurve::IncreaseDegree; planarize: best-fit plane by PCA (math_Jacobi on covariance) or GProp_PEquation, project poles (gp_Pln / ElSLib) with Geom_BSplineCurve::SetPole; control-point edits: SetPole / SetWeight; replace edge geometry in the body with BRepTools_ReShape (keeps topology identity for naming).
- **Priority:** P2

### CV-13 Trim curve (move curve boundary)
- **Purpose:** Trim curves up to bounding entities, or extend curves by a distance or up to entities, with tangent or curvature-continuous extension. (Stdlib id `trimCurve`, file moveCurveBoundary.fs.)
- **Sources:** https://cad.onshape.com/help/Content/PartStudio/trim_curve.htm ; FS: `trimCurve` (moveCurveBoundary.fs).
- **Selection inputs:** Curves to adjust: WIRE BODY && not sketch (sketch entities must first become composite curves); multiple. Up to entity: (BODY && not sketch) || FACE || EDGE || VERTEX || MATE_CONNECTOR; multiple. Help point: VERTEX || MATE_CONNECTOR, max 1 (no new implicit mate connectors).
- **Parameters:**

| FS id | UI label | Type | Enum values / default / bounds+units | Visible when / notes |
|---|---|---|---|---|
| moveBoundaryType | Move curve boundary type | enum MoveCurveBoundaryType (horizontal) | Trim (default), Extend; REMEMBER | always |
| wires | Curves to adjust | Query | wire bodies | always |
| endCondition | End condition | enum CurveExtensionEndCondition | Blind (default), Up to entity; REMEMBER | Extend |
| extensionDistance | Distance | length | NONNEGATIVE_LENGTH_BOUNDS 1e-5 .. 500 m, default 25 mm / 1 in | Extend && Blind |
| extendTo | Up to entity | Query | see filter | Extend && Up to entity |
| extensionShape | Extension shape | enum CurveExtensionShape | Curvature (default), Tangent; REMEMBER | Extend |
| trimTo | Up to entity | Query | see filter | Trim |
| helpPoint | Help point | Query | max 1 | always |
| flipHeuristics | Opposite direction | boolean | default false | always (independent of help point) |

- **Behavior / semantics:** Trim: curve is cut at its intersection(s) with the Up to entities (curve, face, edge, plane); mate connectors need not be intersected (trim at closest point / mate connector plane, inferred); the help point selects which intersection (closest to the help point) and the kept portion; Opposite direction keeps the other side. Extend: the curve end nearest the help point (help point need not be on the curve) is extended - Blind by Distance, or Up to entity (must be reached/intersected; mate connectors need not be intersected); Curvature extension follows the curve's curvature, Tangent extends along the end tangent (straight). Opposite direction extends the other end. Extend manipulator (arrow) anchored at the new end.
- **Edge cases & errors:** MOVE_CURVE_BOUNDARY_SELECT_CURVE, MOVE_CURVE_BOUNDARY_SELECT_TRIM_BOUNDARY.
- **OCCT mapping:** Extend: GeomLib::ExtendCurveToPoint(bspline, point, cont = 1 (tangent) / 2 (curvature), after) with point = end + d*tangent iterated to achieve arc length d (GCPnts_AbscissaPoint), or for Up to: extend long then intersect (GeomAPI_IntCS / BRepExtrema_DistShapeShape / IntTools_EdgeFace) and trim (Geom_TrimmedCurve / BRepBuilderAPI_MakeEdge(curve, u0, u1)). Trim: intersection params via BRepAlgoAPI_Section or GeomAPI_ExtremaCurveCurve / IntTools_EdgeEdge, choose split nearest help point (GeomAPI_ProjectPointOnCurve), keep side.
- **Priority:** P1

### RG-01 Default origin and Top / Front / Right planes
- **Purpose:** Every Part Studio starts with an Origin point and three default planes used as sketch planes and references.
- **Sources:** https://cad.onshape.com/help/Content/PartStudio/part_studios.htm ("Planes and Origin"), plane.htm Notes; FS: defaultFeatures.fs (`addDefaultFeatures`, `qDefaultBodies`, `qTopPlane`, `qFrontPlane`, `qRightPlane`, `qOrigin`).
- **Selection inputs:** n/a (selectable in graphics and Feature list).
- **Parameters:** none user-editable (size derived from document units).
- **Behavior / semantics:**
  - Created in order: Origin (feature id "Origin", opPoint at (0,0,0) flagged origin), Front (id "Front", XZ: origin 0, normal (0,-1,0), x-axis (1,0,0)), Top (id "Top", XY: normal (0,0,1), x (1,0,0)), Right (id "Right", YZ: normal (1,0,0), x (0,1,0)).
  - Default plane display size = PLANE_SIZE_BOUNDS default for the document length unit: 0.15 m (150 mm) metric / 6 in imperial (cm 15, ft 0.5, yd 0.166667).
  - Listed at the top of the Feature list; can be hidden (eye icon / "p" toggles all planes) but never deleted or edited. Feature counts exclude them. Implicit mate connector at the origin selectable; origin usable as point reference everywhere ("select the Origin in the graphics area, or in the Instances list").
  - Planes support context menu: Section view (Part Studio section using the plane), Size to Part Studio (resize display rectangle to enclose all parts), normal-to view ("n").
- **Edge cases & errors:** n/a.
- **OCCT mapping:** gp_Ax3 / gp_Pln constants; display faces BRepBuilderAPI_MakeFace(gp_Pln, -s/2, s/2, -s/2, s/2); origin = TopoDS_Vertex (BRepBuilderAPI_MakeVertex). Persistent ids in the app data model.
- **Priority:** P0

### RG-02 Plane
- **Purpose:** Create a construction plane from references using one of 8 plane types.
- **Sources:** https://cad.onshape.com/help/Content/PartStudio/plane.htm ; FS: `cPlane` (cplane.fs). Hotkey "p" shows/hides all planes.
- **Selection inputs:** "Entities": GeometryType.PLANE || VERTEX || ALLOWS_AXIS || EDGE || MATE_CONNECTOR || mesh allowed; multiple (1-3 depending on type). UIHint PREVENT_CREATING_NEW_MATE_CONNECTORS (existing/implicit inference picks only per dialog). Plane-like: planes, planar faces, mate connectors (XY plane); point-like: vertices, sketch points, mate connector origins, origin; line-like: linear edges, sketch lines, cylindrical faces / circles / arcs (axis).
- **Parameters:**

| FS id | UI label | Type | Enum values / default / bounds+units | Visible when / notes |
|---|---|---|---|---|
| entities | Entities | Query | see above | always |
| cplaneType | Plane type | enum CPlaneType | Offset (OFFSET, default), Plane point (PLANE_POINT), Line angle (LINE_ANGLE), Point normal (LINE_POINT), Three point (THREE_POINT), Mid plane (MID_PLANE), Curve point (CURVE_POINT), Tangent (TANGENT_PLANE); REMEMBER | always |
| offset | Offset distance | length | PLANE_OFFSET_BOUNDS 0 .. 500 m, default 25 mm / 1 in | Offset |
| angle | Angle | angle | ANGLE_360_ZERO_DEFAULT_BOUNDS -1e5 .. 1e5 deg, default 0 | Line angle |
| oppositeDirection | Opposite direction | boolean | default false | Offset or Line angle |
| flipAlignment | Flip alignment | boolean | default false | Mid plane or Tangent |
| flipNormal | Flip normal | boolean | default false | always |
| width | Starting width | length | PLANE_SIZE_BOUNDS 1e-5 .. 500 m, default 150 mm / 6 in (code default 1 m) | always hidden; computed |
| height | Starting height | length | same | always hidden; computed |

- **Behavior / semantics (per type):**
  - Offset: exactly 1 entity: plane/planar face/mate connector, or (since V2885) a circular or elliptical edge (plane of the conic). New plane = reference plane translated by offset along its normal (sign by Opposite direction). Linear manipulator; size from the reference face's bounds (expanded) or default.
  - Plane point: exactly 2 entities: one plane-like (plane/face/mate connector) + one vertex; plane parallel to the reference through the point.
  - Line angle: 1 or 2 entities. With only a line/axis: plane contains the line, normal = perpendicular to the line rotated by Angle about it (info CPLANE_SELECT_LINE_ANGLE_REFERENCE asks for a reference). With line + reference (second line: its direction, or offset between parallel lines; plane: in-plane direction = line x plane normal; point: direction to point): base plane contains the line and the reference, then rotated about the line by Angle (sign by Opposite direction). Order independent (non-axis first is swapped). Angle manipulator.
  - Point normal: exactly 2 entities: an axis (line, edge, circle/arc, cylindrical/revolved face) and a vertex; plane through the point with normal = axis direction.
  - Three point: exactly 3 vertices/mate connector origins; origin = centroid of the 3 points, normal = (p3-p1) x (p2-p1) normalized, x-axis along p2-p1; size = 2 x max distance from centroid x 1.2.
  - Mid plane: 2 points -> plane through midpoint, normal along p1->p2; 1 edge (open) -> plane through the midpoint of its end points normal to the chord; 2 planes (faces/mate connectors) -> bisector: if intersecting, normal = normalize(n1 + n2') where n2' = -n2 unless Flip alignment (gives the perpendicular bisector), origin at midpoint of plane origins, x along intersection line; if parallel, plane midway with normal n1.
  - Curve point: exactly 2 entities: curve/edge + vertex; point projected onto the (extended) curve, plane normal = curve tangent at that parameter, origin at the curve point.
  - Tangent: cylinder face + optional second entity: with only the cylinder an info asks for a reference; + plane/mate connector -> tangent plane parallel to it (plane normal must be perpendicular to the cylinder axis); + vertex -> tangent plane passing through the point (point must be outside or on the cylinder; angle acos(r/d)); Flip alignment chooses the other tangent solution/side.
  - Flip normal reverses the final normal (name label orientation shows normal side).
  - Preselection logic (cPlaneLogic): 1 plane or circular edge -> Offset; 1 line/curve with 2 vertices -> Mid plane; 1 axis -> Line angle; plane+vertex -> Plane point; 2 planes or 2 vertices -> Mid plane; curve+vertex -> Curve point; cylinder+(vertex or plane) -> Tangent; vertex+axis -> Point normal; line+(line/plane/vertex) -> Line angle; 3 points -> Three point.
  - Display size: computed default (fits referenced geometry); user can right-click "Size to Part Studio". Section view from plane context menu.
- **Edge cases & errors:** CPLANE_INPUT_OFFSET_PLANE, CPLANE_INPUT_POINT_PLANE, CPLANE_SELECT_LINE_ANGLE_REFERENCE (info), CPLANE_INPUT_LINE_ANGLE2, CPLANE_INPUT_POINT_LINE, CPLANE_DEGENERATE_SELECTION, TOO_MANY_ENTITIES_SELECTED, CPLANE_INPUT_THREE_POINT, POINTS_COINCIDENT (collinear/coincident), CPLANE_INPUT_MIDPLANE (also closed-loop edge), CPLANE_INPUT_CURVE_POINT, CPLANE_TANGENT_INPUT, CPLANE_TANGENT_SELECT_REFERENCE (info), CPLANE_TANGENT_PLANE_INVALID, CPLANE_TANGENT_POINT_INVALID (point inside cylinder), FLATTENED_SHEET_METAL_SKETCH_PROHIBITED.
- **OCCT mapping:** Pure math with gp_Pln / gp_Ax3 / gp_Vec; evaluation helpers BRepAdaptor_Surface (plane/cylinder detection), BRepAdaptor_Curve + GeomAPI_ProjectPointOnCurve (curve point, extended curve), GeomAPI_IntSS / gp intersections (mid plane line); plane body = Geom_Plane face via BRepBuilderAPI_MakeFace with computed width/height.
- **Priority:** P0

### RG-03 Mate connector (Part Studio feature)
- **Purpose:** Create an explicit local coordinate system entity (origin + Z primary axis + X secondary axis) on or between entities, optionally realigned, moved/rotated, flipped, and owned by a part/surface/curve, for use in assemblies (mates) and as axis/plane/point input to features.
- **Sources:** https://cad.onshape.com/help/Content/PartStudio/mate_connector.htm ; FS: `mateConnector` (mateConnector.fs). Shortcuts: Ctrl+M opens the Mate connector dialog; "k" shows/hides mate connectors; "a" flips the primary (Z) axis while editing; holding Shift locks mate inference.
- **Selection inputs:** Origin entity: ((EDGE || VERTEX) || (FACE && not construction)) && mesh allowed, max 1 (unconfigurable; picked via inference points, an existing mate connector, or the Origin). Between entity: FACE (mesh ok), max 1. Primary / Secondary axis entity: FACE || EDGE, max 1 each. Select owner entity: BODY && (SOLID || mesh || SHEET || WIRE || COMPOSITE), modifiable, max 1.
- **Parameters:**

| FS id | UI label | Type | Enum values / default / bounds+units | Visible when / notes |
|---|---|---|---|---|
| originType | Origin type | enum OriginCreationType | On entity (ON_ENTITY, default), Between entities (BETWEEN_ENTITIES) | always |
| originQuery | Origin entity | Query | max 1 | always |
| entityInferenceType | (hidden) | enum EntityInferenceType | PART_ORIGIN, POINT, CENTROID, CENTER, MID_POINT, TOP_AXIS_POINT, MID_AXIS_POINT, BOTTOM_AXIS_POINT, ORIGIN_X, ORIGIN_Y, ORIGIN_Z, LOOP_CENTER, VIRTUAL_SHARP | set by the inference pick |
| secondaryOriginQuery | (hidden) | Query | second inference entity | |
| originAdditionalQuery | Between entity | Query | FACE max 1 | Between entities |
| realign | Realign | boolean | default false | always |
| primaryAxisQuery | Primary axis entity | Query | FACE or EDGE, max 1 | realign (controls Z) |
| secondaryAxisQuery | Secondary axis entity | Query | FACE or EDGE, max 1 | realign (controls X/Y) |
| transform | Move | boolean | default false | always |
| translationX / Y / Z | X translation / Y translation / Z translation | length | ZERO_DEFAULT_LENGTH_BOUNDS -500 .. 500 m, default 0 | Move |
| rotationType | Rotation axis | enum RotationType | Rotate about X, Rotate about Y, Rotate about Z (default) | Move |
| rotation | Rotation angle | angle | -1e5 .. 1e5 deg, default 0 | Move |
| allowOwnerEntity | Allow owner entity | boolean | hidden, default true | |
| requireOwnerPart | Owner entity | boolean | default true | allowOwnerEntity |
| ownerPart | Select owner entity | Query | max 1 | requireOwnerPart |
| specifyNormal, nx, ny, nz | Specify normal / Normal x,y,z | boolean / real | hidden; NORMAL_PARAMETER_BOUNDS -1 .. 1, default 0 | internal (implicit MC from picks) |
| flipPrimary | Flip primary axis | boolean | default false | always |
| secondaryAxisType | Reorient secondary axis | enum MateConnectorAxisType | +X (default), +Y, -X, -Y | always (button cycles quadrants) |
| isForSubFeature | (hidden) | boolean | default false | implicit MCs inside other features |

- **Behavior / semantics:**
  - Coordinate system computed by evMateConnectorCoordSystem from: origin entity + inference type (+ secondary origin for Between entities: origin placed halfway between the inferred point on the first entity and the Between face, inferred), realign axes (primary entity direction/normal becomes Z, secondary entity defines X), then Move (translate in the connector's local X/Y/Z, then rotate about chosen local axis - order per field order, inferred), then Flip primary (rotate 180 deg flipping Z), then Reorient secondary (rotate X to +X/+Y/-X/-Y, 90 deg steps about Z).
  - Owner: by default the edit logic auto-fills the owner from the origin/axis selections' owning body, or the single part/surface/curve in the studio; empty owner (uncheck) = ownerless connector (still created). Owner determines which part carries the connector into assemblies (part-patterned connectors are only inserted if owner is set). If Owner entity is required but unresolved -> MATECONNECTOR_OWNER_PART_NOT_RESOLVED.
  - Uses: Z-axis as axis input to features (helix, revolve, patterns), XY plane as plane input (planes, sketches, Up to face), origin as point/vertex input.
  - Explicit connectors are top-level Feature list items; hide/show per item, "Hide other Mate connectors", visibility independent between Part Studio and assembly. A new connector can be placed on an existing connector and moved.
  - Implicit mate connectors (created inside another feature's selection field by the mate-connector icon) are subfeatures (isForSubFeature) and are not re-created in feature patterns.
- **Edge cases & errors:** MATECONNECTOR_OWNER_PART_NOT_RESOLVED, FLATTENED_SHEET_METAL_SKETCH_PROHIBITED.
- **OCCT mapping:** gp_Ax3 / gp_Trsf composition (math only); inference evaluation via RG-04 helpers; result stored as app entity (point + frame) or as TopoDS_Vertex with attached frame attribute (TDF/XCAF-like custom attribute).
- **Priority:** P0

### RG-04 Mate connector inference points (implicit connector placement rules)
- **Purpose:** Define the candidate connector locations/orientations shown when hovering geometry in a mate connector selection (explicit dialog, implicit connectors in features and assembly mates).
- **Sources:** mate_connector.htm ("Visualize mate connector points", "Inference points and defaults"); FS enum EntityInferenceType.
- **Selection inputs:** hover over faces, edges, vertices, sketch entities, curves; Shift locks the current inference set (prevents waking up others); clicking a face away from points picks the face centroid.
- **Parameters:** EntityInferenceType values (internal): PART_ORIGIN, POINT, CENTROID, CENTER, MID_POINT, TOP_AXIS_POINT, MID_AXIS_POINT, BOTTOM_AXIS_POINT, ORIGIN_X, ORIGIN_Y, ORIGIN_Z, LOOP_CENTER, VIRTUAL_SHARP.
- **Behavior / semantics:**
  - Planar face: candidates at every vertex, arc/circle center, edge midpoint, face centroid, and centroids of inner regions/loops (holes, slots) (LOOP_CENTER); Z = face normal, connector parallel to face.
  - Cylindrical (incl. partial) face: points on the axis at top, middle, bottom (TOP/MID/BOTTOM_AXIS_POINT), Z along axis ("perpendicular to the face axis at the middle and ends"); hovering a partial cylindrical edge of a planar face offers the axis center.
  - Conical face: top, middle, bottom axis points and the virtual sharp (apex where sides would meet) (VIRTUAL_SHARP).
  - Linear edge / sketch line: points at middle and ends, Z along the line.
  - Circular edge / sketch circle: center (CENTER) and ends/middle, Z along circle axis.
  - Curve end point or control point: Z aligned with global Z.
  - Curve (spline) itself: tangent to the curve at the closest end, control, or midpoint.
  - Vertex: Z normal to the associated face.
  - Origin / part origin: PART_ORIGIN, world-aligned; ORIGIN_X/Y/Z variants align Z with world X/Y/Z (inferred from names).
  - Cross-highlighting of connected entities while hovering.
- **Edge cases & errors:** n/a.
- **OCCT mapping:** Face type via BRepAdaptor_Surface::GetType; centroid via BRepGProp::SurfaceProperties (GProp_GProps::CentreOfMass); inner loop centroids via BRepGProp on face built from inner wire; edge midpoints GCPnts_AbscissaPoint at half length; circle centers gp_Circ::Location; axis extents by projecting face bounding box (Bnd_Box / BRepBndLib) onto axis; cone apex gp_Cone::Apex; tangents GeomLProp_CLProps; picking/hover UI is app code.
- **Priority:** P0

### RG-05 Point (construction point, internal)
- **Purpose:** Construction point feature used internally (e.g. derived/mesh workflows); not exposed as a toolbar tool.
- **Sources:** no help page; FS: `cPoint` (cpoint.fs) - "@internal ... Not exposed through the UI."
- **Selection inputs:** Edge: EDGE, max 1. Mesh: FACE && mesh geometry, max 1.
- **Parameters:**

| FS id | UI label | Type | Enum values / default / bounds+units | Visible when / notes |
|---|---|---|---|---|
| pointType | Point type | enum PointType | Edge point (EDGE_POINT, default), Mesh point (MESH_POINT) | always |
| entities | Edge | Query | max 1 | Edge point |
| parameter | Parameter | real | EDGE_PARAMETER_BOUNDS 0 .. 1, default 0.5 | Edge point |
| mesh | Mesh | Query | max 1 | Mesh point |
| x / y / z | X / Y / Z | length | LENGTH_BOUNDS -500 .. 500 m, default 25 mm / 1 in | Mesh point |

- **Behavior / semantics:** Edge point: point body at normalized parameter on the edge (opPoint). Mesh point: point at coordinates on a mesh face. Sketch points and Routing curve "Keep points" are the user-facing ways to create points.
- **Edge cases & errors:** n/a documented.
- **OCCT mapping:** BRepAdaptor_Curve::Value at normalized arc-length/param (GCPnts_AbscissaPoint) + BRepBuilderAPI_MakeVertex.
- **Priority:** P2

### RG-06 Name entity (internal)
- **Purpose:** Assign a persistent name to a face/edge/body so it can be queried by name (qNamed). Under development, not for general use.
- **Sources:** no help page; FS: `nameEntity` (nameEntity.fs) - "Under development, not for general use!!"; opNameEntity (geomOperations.fs).
- **Selection inputs:** Entity: FACE || BODY || EDGE, max 1.
- **Parameters:**

| FS id | UI label | Type | Enum values / default / bounds+units | Visible when / notes |
|---|---|---|---|---|
| entity | Entity | Query | max 1 | always |
| entityName | Name | string | should be unique in the Part Studio | always; feature name template "Name #entityName" |

- **Behavior / semantics:** Name becomes the entity's identity; qNamed(name) resolves to it or its descendants; historical queries use qNamed as a shortcut. Multiple resolved entities -> warning.
- **Edge cases & errors:** warning when entity resolves to multiple entities.
- **OCCT mapping:** App-level topological naming attribute (TNaming / custom attribute map); no geometry.
- **Priority:** P2

### FR-01 Frame
- **Purpose:** Create a collection of swept solid segments ("frame" / weldment members) sharing one profile sketch, arranged end-to-end along path edges, with profile alignment/rotation/mirror, corner treatments, end limits, and tangent-segment merging into composite parts.
- **Sources:** https://cad.onshape.com/help/Content/PartStudio/frame.htm ; FS: `frame` (frame.fs), frameUtils.fs.
- **Selection inputs:**
  - Sketch profile: PartStudioData (PartStudioItemType.SKETCH), max 1; chosen in the "Select sketch" dialog from Current document, Other documents (must be a version), or Libraries (Onshape Frame Profile Library definition id 65dcc2a02c4ff1c239467ec9, company or personal library; Standard / Type / Size or Configuration, "Generate" applies configuration). REMEMBER_PREVIOUS_VALUE. Profile sketch must live in a different Part Studio than the path.
  - Selections ("Faces, edges, and vertices that define sweep paths"): (FACE && not construction) || EDGE || (VERTEX, no edge points) || (WIRE BODY && not sketch); multiple; contiguous paths.
  - Faces to lock: FACE, not construction. Angle reference: ALLOWS_DIRECTION || MATE_CONNECTOR, max 1. Corner override vertex: VERTEX, not construction, max 1 (picked in graphics; row created automatically). Faces to trim to: planar FACE. Parts to trim to: SOLID bodies.
- **Parameters:**

| FS id | UI label | Type | Enum values / default / bounds+units | Visible when / notes |
|---|---|---|---|---|
| profileSketch | Sketch profile | PartStudioData (sketch) | library/document reference incl. configuration | always |
| selections | Selections | Query | see above | always |
| lockProfile | Lock profile faces | boolean | default false | always (group driver) |
| profileLockFaces | Faces to lock | Query | faces | lockProfile |
| mergeTangentSegments | Merge tangent segments | boolean | default true | always |
| angle | Angle | angle | ANGLE_360_ZERO_DEFAULT_BOUNDS -1e5 .. 1e5 deg, default 0 | always (angle manipulator) |
| mirrorProfile | Mirror across Y axis | boolean (flip arrow) | default false | always |
| angleReference | Angle reference | Query | max 1 | always (optional) |
| defaultCornerType | Default corner type | enum FrameCornerType | Miter (default), Butt, Coped butt, None (hidden TANGENT used internally) | always |
| defaultButtFlip | Flip corner | boolean | default false | Butt or Coped butt |
| cornerOverrides[] | Corner overrides ("#vertex [#cornerType]") | array | | always |
| .vertex | Vertex | Query | hidden label | per row |
| .cornerType | Corner type | enum FrameCornerType | Miter/Butt/Coped butt/None | per row |
| .cornerButtFlip | Flip corner | boolean | default false | Butt/Coped butt |
| trim | Limit frame ends | boolean | default false | always (group "Trimming") |
| trimPlanes | Faces to trim to | Query | planar faces / planes | trim |
| trimBodies | Parts to trim to | Query | solids | trim |
| index | Index | integer (hidden) | FRAME_NINE_POINT_COUNT 0 .. 100, default 4 (center) | alignment point chosen via points manipulator |

- **Behavior / semantics:**
  - Profile: the referenced sketch's region(s) are instantiated (FRAME_PROFILE_REGION if unusable). Alignment points = 9-point grid of the profile bounding box (corners, edge mids, center; default index 4 = center) plus "additional alignment points" from the profile's Frame Tag (FR-03); clicking a magenta point in the graphics area sets `index`, shifting the profile so that point lies on the path.
  - Paths: selected edges + boundary edges of selected faces + edges of selected wire bodies + straight splines created between consecutive selected vertices (in order) are chained with constructPaths; invalid/branching -> FRAME_BAD_PATH. The first selected edge's direction is kept (path not flipped).
  - Profile placement per path start: profile plane normal to the path tangent; Angle rotates the profile about the path tangent (from the default normal-plane orientation or relative to the Angle reference direction); Mirror flips the profile across its Y axis.
  - Sweep: each path edge swept separately (opSweep, profileControl default; Lock profile faces -> ProfileControlMode.LOCK_FACES keeps the profile normal to the selected faces, e.g. twisting fuselage stringers). Each segment = separate solid ("frame segment") with start/end cap faces attributed (frame topology attributes used by Cut list, Gusset, End cap, Frame trim).
  - Corners at consecutive segment junctions (from default or override by vertex position): tangent junctions (sin(angle) ~ 0) get TANGENT (no cut, eligible for compositing); Miter - both segments extended and cut by the bisector plane; Butt - by convention without flip the previous segment is extended to the far extent of the next profile and the next segment butts against its side (flip reverses which member runs through); Coped butt - butt plus the through member is subtracted from the butting member (cope to profile shape); None - segments left square/open at the corner. Closed loops get a corner at the seam.
  - Merge tangent segments: G1-continuous runs of segments become a composite part (one item) instead of individual parts.
  - Limit frame ends: start/end segments are extended/trimmed to the selected planar faces or parts (only free ends; internal trims not allowed - FRAME_NO_INTERNAL_TRIM; multiple planes on one end - FRAME_MULTIPLE_TRIM_PLANES).
  - Feature-pattern aware; parts named per frame segment.
- **Edge cases & errors:** FRAME_SELECT_PROFILE, FRAME_SELECT_PATH, FRAME_PROFILE_REGION, FRAME_BAD_PATH, FRAME_SWEEP_FAILED, FRAME_CANDIDATE_FACES (sweep lacks start/end caps), FRAME_MALFORMED_SEGMENT, FRAME_BAD_CORNER_TYPE, FRAME_ANGLE_REFERENCE_INVALID_ENTITY, FRAME_MULTIPLE_TRIM_PLANES, FRAME_NO_INTERNAL_TRIM, FRAME_TRIM_FAILED, CANNOT_RESOLVE_ENTITIES. iOS: view/apply only (platform note).
- **OCCT mapping:** Per segment BRepOffsetAPI_MakePipeShell(spine wire = single edge) with SetMode(gp_Dir binormal) / SetMode(Standard_True for Frenet) or SetMode(TopoDS_Shape spineSupport) for Lock profile faces (normal to support surface), Add(profile wire located by gp_Trsf: path start frame * angle rotation * mirror * alignment offset), MakeSolid(). Corners: extend segment (BRepPrimAPI_MakePrism of end cap face by extension length) then cut with half-space (BRepPrimAPI_MakeHalfSpace + BRepAlgoAPI_Cut) at miter bisector or butt plane; coped: BRepAlgoAPI_Cut(butting member, through member). Limits: BRepAlgoAPI_Splitter / half-space cut with planes, BRepAlgoAPI_Cut with parts. Composite parts and frame attributes: app data model.
- **Priority:** P2

### FR-02 Frame profile libraries
- **Purpose:** Provide standard and custom profile sketches for Frame: Onshape-maintained library plus one company library and one personal library per user, organized by folder hierarchy.
- **Sources:** https://cad.onshape.com/help/Content/Library/onshape_frame_profiles_library.htm ; frame.htm ("Frame profile libraries" sections).
- **Selection inputs:** Select sketch dialog tabs: Current document, Other documents, Libraries (Library dropdown, Standard / Type / Profile levels, search, Size/Configuration, Generate, Done).
- **Parameters:** n/a (library data).
- **Behavior / semantics:**
  - Onshape frame profiles library (read-only, versioned, maintained by Onshape) standards:
    - ISO: Rectangular bar, Rectangular tube, Round bar, Round tube.
    - ANSI: Rectangular bar, Rectangular tube, Round bar, Round tube.
    - 8020 (T-slot aluminum): 10, 15, 20, 25, 30, 40, 45 Series (e.g. 1001, 1010, 1020, 1030, 2020, 2040, 1515, 1530, 1545, 4545 ... with -S/-LS/-Lite variants).
    - AISC: C, HP, L-angle, M, MC, MT, Pipe, Rectangular (HSS), Round (HSS), S, Square (HSS), ST, W, WT shapes (configurations list sizes, e.g. W44x335 ... ).
    - AS (Australian): Cold formed hollow sections and profiles (channel CC, circular hollow CHS, equal/unequal angle CA, flat, rail hollow, rectangular RHS, square SHS); Hot rolled and structural steel (equal angles EA, unequal angles UA, parallel flange channel PFC, tapered flange beam TFB, universal beam UB, universal bearing pile UBP, universal column UC, welded beam WB, welded column WC).
    - Construction materials (wood): Lumber Inch, Timber Metric.
    - Each profile is a configured sketch (size = configuration) with frame tag properties (Standard, Description) feeding the Cut list.
  - Custom libraries: folders on the Documents page; company library must be a root folder shared to the company with Link + Share (owned by admin); personal library needs no sharing. Folder levels map to Standard / Type / Profile (up to 3 levels unlabeled; more than 3 require "Library label" on every folder; up to 10 primary levels, branching allowed; last folder must contain >= 1 profile document). Profile document: one Part Studio with exactly one profile sketch (Part Studio name = Profile name), versioned. Folder context menu: Set library > My frame profiles; Frame profile library > Update my library (required after changes) / Unset my library. Library folders cannot be deleted or have shares edited until unset. Multiple profiles in one document require a document Library label property.
- **Edge cases & errors:** "Failed to update the library" when >3 levels lack labels; multiple sketches in a profile Part Studio breaks update.
- **OCCT mapping:** n/a - app data (document references, configured sketch evaluation). Ship the Onshape-equivalent standards as noshape-owned parametric profile sketches (data entry work).
- **Priority:** P2

### FR-03 Tag (Frame profile tag)
- **Purpose:** Tag a profile sketch with frame metadata (Standard, Description, extra columns) and extra alignment points so that Frame and Cut list use them. (Tag also has purposes Form - sheet metal form tool, see sheet-metal section - and PCB holes - out of scope.)
- **Sources:** https://cad.onshape.com/help/Content/PartStudio/tag.htm ; FS: `tag` (tag.fs), predicate `defineFrameTagParams`.
- **Selection inputs:** Sketch profile: FeatureList (sketch feature), max 1, no query variable. Sketch points (Additional alignment points group, collapsed): VERTEX && sketch.
- **Parameters:**

| FS id | UI label | Type | Enum values / default / bounds+units | Visible when / notes |
|---|---|---|---|---|
| tagPurpose | Tag purpose | enum TagPurpose | Frame (FRAME), Form (FORM), PCB holes (PCB_HOLES); REMEMBER | always |
| sketch | Sketch profile | FeatureList | max 1 | Frame |
| alignmentPoints | Sketch points (group "Additional alignment points") | Query | sketch vertices | Frame |
| standard | Standard | string | configurable | Frame |
| description | Description | string | configurable | Frame |
| additionalColumns[] | Additional columns ("#header = #value", Add Column) | array (no reorder) | | Frame (not configurable) |
| .header / .value | Header / Value | string | | per row |
| showDefaultAlignmentPoints | Show default points | boolean | default true; REMEMBER | Frame (magenta 9-point grid overlay) |

- **Behavior / semantics:** Attaches a frame-profile attribute to the sketch. Frame uses the alignment points (9 default + additional) for placement; Cut list adds columns Standard, Description and each additional Header with values; blank Standard/Description -> column omitted, or "Not set" for that profile if other profiles define it. Workflow: author profile in its own Part Studio (dimensions/variables configured for sizes), tag, version, reference the version from frames; share library documents (view+link for users, edit+link for maintainers).
- **Edge cases & errors:** n/a documented.
- **OCCT mapping:** n/a - attribute on sketch feature (app data model).
- **Priority:** P2

### FR-04 Frame trim
- **Purpose:** Trim frame segments against each other in prioritized ordered groups, or against a face/plane/mate connector.
- **Sources:** https://cad.onshape.com/help/Content/PartStudio/frame_trim.htm ; FS: `frameTrim` (frameTrim.fs).
- **Selection inputs:** Ordered groups: array of rows, each "Frames" (SOLID bodies). Face: "Parts to trim" (SOLID bodies), "Face to trim to" (FACE || (SHEET BODY, any) || (construction plane) || MATE_CONNECTOR), max 1.
- **Parameters:**

| FS id | UI label | Type | Enum values / default / bounds+units | Visible when / notes |
|---|---|---|---|---|
| trimType | Trim Type | enum FrameTrimType (horizontal) | Ordered groups (GROUP, default), Face (FACE); REMEMBER | always |
| frameGroups[] | Frame groups | array (collapsed rows) | | Ordered groups |
| frameGroups[].frames | Frames | Query | solids | per group |
| targets | Parts to trim | Query | solids | Face |
| tool | Face to trim to | Query | max 1 | Face |
| flipTrim | Opposite side | boolean | default false | Face |
| useToolExtents | Trim to face boundaries | boolean | default false (help says on by default - discrepancy) | Face |

- **Behavior / semantics:**
  - Ordered groups (decreasing priority): group 1 is never modified; each later group i is trimmed (boolean subtract, keep tools) by the union of all earlier groups. E.g. groups [a,b],[c],[d,e]: [d,e] - [a,b,c], then [c] - [a,b]. Groups must touch/collide (FRAME_DISJOINT_GROUPS); lengthwise trims rejected (FRAME_LENGTHWISE_TRIM). By convention the single largest piece of each trimmed beam is kept (FRAME_MULTIPLE_EQUAL_SEGMENTS_AFTER_SPLIT if ambiguous). Frame end-cap attributes are re-applied to new end faces.
  - Face: targets split by the tool (face, plane, sheet, or mate connector XY plane); Opposite side picks the kept side (keep-side manipulator arrow); Trim to face boundaries limits the cut to the face extents, otherwise the face's surface is extended.
- **Edge cases & errors:** FRAME_TRIM_GROUPS, FRAME_DISJOINT_GROUPS, FRAME_LENGTHWISE_TRIM, FRAME_MULTIPLE_EQUAL_SEGMENTS_AFTER_SPLIT, FRAME_MISSING_CAP_FACES, FRAME_CANDIDATE_FACES, FRAME_TRIM_SELECT_TARGETS, FRAME_TRIM_SELECT_TOOL, FRAME_TRIM_FAILED (also when a beam is fully consumed).
- **OCCT mapping:** BRepAlgoAPI_Cut(target, toolsUnion) then explode solids and keep largest by GProp volume (BRepGProp::VolumeProperties). Face mode: BRepAlgoAPI_Splitter with tool face (extended surface via large BRepBuilderAPI_MakeFace on the underlying Geom_Surface when not bounded) and side classification (sample point vs face normal); plane/mate connector -> BRepPrimAPI_MakeHalfSpace + Cut.
- **Priority:** P2

### FR-05 Gusset (frame gusset)
- **Purpose:** Create triangular or rectangular gusset plates between adjacent frame segments (or frame and non-frame planar faces) at selected linear edges.
- **Sources:** https://cad.onshape.com/help/Content/PartStudio/gusset.htm ; FS: `gusset` (gusset.fs).
- **Selection inputs:** Edges ("Edges that define the bases of the gussets"): GeometryType.LINE && SOLID body edges; multiple (one gusset per edge) - edge on the frame segment cut face at the junction. Alignment entity: LINE edge of a solid, max 1.
- **Parameters:**

| FS id | UI label | Type | Enum values / default / bounds+units | Visible when / notes |
|---|---|---|---|---|
| edges | Edges | Query | linear solid edges | always |
| gussetType | Gusset type | enum GussetStyleType | Triangle (default), Rectangle; REMEMBER | always |
| length | Length | length | NONNEGATIVE_LENGTH_BOUNDS 1e-5 .. 500 m, default 25 mm / 1 in; REMEMBER | always (along the frame edges; manipulator) |
| shouldFlipBaseSides | Flip base sides | boolean | default false | Rectangle |
| thickness | Thickness | length | 1e-5 .. 500 m, default 25 mm / 1 in; REMEMBER | always (across the frame width) |
| chamfer | Chamfer | boolean | default false; REMEMBER | Rectangle |
| chamferSize | Chamfer size | length | CHAMFER_SIZE_BOUNDS 1e-5 .. 500 m, default 1 mm / 0.04 in | Rectangle && chamfer |
| gussetPosition | Gusset position | enum GussetPosition | Centered (default), Aligned; REMEMBER | always |
| alignedReference | Alignment entity | Query | max 1 | Aligned |
| shouldFlipAlignment | Opposite alignment | boolean | default false | Aligned |
| offset | Offset distance | length | 0 .. 500 m, default 0; REMEMBER | always (manipulator) |
| shouldFlipOffset | Opposite direction | boolean | default false | always |
| baseSweptFaces | Base faces | Query | hidden; auto-collected planar faces | |

- **Behavior / semantics:** For each edge (processed in reverse order; manipulators on the last selected edge): find the two planar base faces adjacent to the junction that are parallel to the edge direction and intersect it (since V2119 frame-to-non-frame planar faces allowed); gusset plate lies in the plane perpendicular to the edge through the edge midpoint (+ alignment/offset shift), spanning the corner between the two base planes with legs of `length`; Triangle = right-triangle plate; Rectangle = square/rectangular plate with optional chamfer on the outer corner (Flip base sides swaps which base face gets which side, inferred); plate extruded `thickness` symmetric about the plate plane (Centered at edge midpoint) or aligned flush with the alignment edge side (Opposite alignment) then shifted by Offset distance along the edge. Gussets are individual new parts; included in Cut list (noted in Description). Not possible between round-tube segments or where corner type is None.
- **Edge cases & errors:** EMPTY_GUSSET_SELECTION, NON_FRAME_EDGE_SELECTED, SWEPT_EDGE_SELECTED (lengthwise edge picked), AMBIGUOUS_GEOMETRY_FOR_GUSSET_DEFINITION, CANNOT_FIT_A_GUSSET, CHAMFER_SIZE_EXCEED_GUSSET_SIZE, GUSSET_ALIGNED_OFFSET_NOT_PARALLEL, GUSSET_ALIGNMENT_NO_INTERSECTION, GUSSET_EMPTY_ALIGNMENT_SELECTION, GUSSET_OFFSET_NOT_PARALLEL.
- **OCCT mapping:** Build 2D triangle/rectangle (with chamfer) wire in gp_Pln (BRepBuilderAPI_MakePolygon), face (BRepBuilderAPI_MakeFace), BRepPrimAPI_MakePrism symmetric (translate -t/2 then prism t); planar face finding via BRepAdaptor_Surface plane tests and IntTools_EdgeFace / BRepExtrema.
- **Priority:** P2

### FR-06 End cap
- **Purpose:** Add end cap plates to open frame-segment end faces (match profile, rectangle, circle, or internal plug), with thickness, offset/inset, and rectangle corner treatment.
- **Sources:** https://cad.onshape.com/help/Content/PartStudio/end_cap.htm ; FS: `endcap` (endcap.fs).
- **Selection inputs:** Faces: GeometryType.PLANE && not construction; multiple (frame segment end/cap faces).
- **Parameters:**

| FS id | UI label | Type | Enum values / default / bounds+units | Visible when / notes |
|---|---|---|---|---|
| faces | Faces | Query | planar end faces | always |
| profileType | Profile type | enum ProfileType | Match profile (default), Rectangle, Circle, Internal | always |
| thickness | Thickness | length | endcap OFFSET_BOUNDS 1 mm .. 500 m, default 5 mm / 0.25 in | always (manipulator) |
| thicknessDirection | Opposite direction | boolean | default false | profileType != Internal |
| offsetDistance | Offset | length | 0 .. 500 m, default 0; REMEMBER | != Internal (grows/shrinks cap outline) |
| offsetDirection | Opposite direction | boolean | default false | != Internal |
| internalOffsetDistance | Offset | length | 0 .. 500 m, default 0; REMEMBER | Internal (clearance from tube inner wall, inferred) |
| internalShiftDistance | Inset | length | 0 .. 500 m, default 0; REMEMBER | Internal (depth into the tube; inset manipulator) |
| cornersType | Treatment type | enum CornerType | None (default), Chamfer, Fillet; REMEMBER | Rectangle |
| cornerDistance | Length | length | 1 mm .. 500 m, default 5 mm / 0.25 in | Rectangle && Chamfer |
| cornerRadius | Radius | length | 1 mm .. 500 m, default 5 mm / 0.25 in | Rectangle && Fillet |

- **Behavior / semantics:** Per selected end face: Match profile extrudes the face's outer outline (holes filled) by thickness along the face normal (outward by default, flip inward); Rectangle = tight bounding rectangle of the face in its plane; Circle = circle centered at the face bounding-box center with radius = max distance to the face edges; Internal = plug matching the inner loop of a hollow profile (face must have exactly 2 loops), placed inside the tube by Inset, outline offset inward by Offset. Offset (non-internal) offsets the cap's side faces (opOffsetFace) to enlarge/shrink. Rectangle corners chamfered (equal offsets) or filleted on the cap's side edges. Each cap is a new part, listed in Cut list Description.
- **Edge cases & errors:** NO_CAP_FACE_SELECTED_ERROR, INVALID_CAP_FACE_SELECTED_ERROR, CAP_MULTI_FACE_SELECTED_ERROR (multiple faces of one end for non-rectangle), CAP_MULTI_LUMENS_SELECTED_ERROR (Internal needs a single inner loop), CAP_INCLINED_CUT_FRAME_ERROR (mitered/inclined end), CAP_FRAME_AXIS_ERROR, FRAME_MISSING_CAP_FACES.
- **OCCT mapping:** Outer wire via BRepTools::OuterWire, BRepBuilderAPI_MakeFace, BRepPrimAPI_MakePrism; rectangle from Bnd_Box in face cSys (BRepBndLib::AddOptimal after transforming); circle radius via BRepExtrema_DistShapeShape (max distance -> sample edges); inner loop for Internal; BRepOffsetAPI_MakeOffset (2D outline offset) or BRepOffsetAPI_MakeThickSolid-free face offset; corners BRepFilletAPI_MakeFillet / BRepFilletAPI_MakeChamfer on prism lateral edges.
- **Priority:** P2

### FR-07 Cut list
- **Purpose:** Group frame segments (and gussets/end caps) into an open composite part and generate a cut list table (item, quantity, standard, description, custom columns, length, end angles) with column overrides.
- **Sources:** https://cad.onshape.com/help/Content/PartStudio/cut_list.htm ; FS: `cutlist` (cutlist.fs), cutlistMath.fs.
- **Selection inputs:** Frames: (SOLID BODY) || COMPOSITE; multiple (when not Select all frames). Override Frames: same filter.
- **Parameters:**

| FS id | UI label | Type | Enum values / default / bounds+units | Visible when / notes |
|---|---|---|---|---|
| selectAllFrames | Select all frames | boolean | default false | always (includes all gussets and end caps) |
| frames | Frames | Query | solids / composites | !selectAllFrames |
| columnOverrides[] | Overrides ("#column = #value", Add Column Override) | array (no reorder) | | always |
| .isFirst | Is first | boolean (hidden) | set by edit logic | |
| .sameFramesAsPrevious | Same frames as previous | boolean | default true | !isFirst |
| .overrideAllFrames | All frames | boolean | default false | isFirst or !sameFramesAsPrevious |
| .overrideFrames | Frames | Query | | !overrideAllFrames (and frames row shown) |
| .column | Column | string | max length 10000; existing or new column name | always |
| .value | Value | string | max length 10000 | always |

- **Behavior / semantics:**
  - Selection is expanded/decomposed (non-cutlist composites into members). Rows: segments grouped first by identical row data (profile tag properties, overrides), then by geometric congruence (clusterBodies, relative tolerance 0.01); tangent-composite segments are evaluated as one temporary body. Groups sorted by the earliest index of their members. Columns per row: Item (1..n), Qty (group size), Standard, Description (from profile Tag; gussets/end caps noted in Description), custom tag columns, overrides, Length, Angle 1, Angle 2.
  - Length/angles: straight segments - length along swept edges between start/end cap faces, Angle 1/2 = end cut angles (miter/butt angle relative to square cut); spline-path segments - approximate length only (no angles); missing cap attributes -> warning FRAME_MISSING_CAP_FACES and blank cells.
  - Column order: Item, Qty, Standard, Description (if present), then custom columns (map order), then Length, Angle 1, Angle 2. Item, Qty, Length, Angle 1, Angle 2 cannot be overridden. Override with empty value -> column hidden; frames not covered by an override show "Not set".
  - Creates an open composite part (one per cut list) carrying the table attribute; table appears in the right-side tables panel (Cut list icon); table UI: column context menu Move left/right, Hide column; three-dot menu Copy table / Show hidden column. Best practice: insert the composite into assemblies so BOM matches.
  - Not allowed inside feature patterns.
- **Edge cases & errors:** FRAME_CUTLIST_NO_FEATURE_PATTERN, FRAME_CUTLIST_COLUMN_EMPTY, FRAME_BAD_COMPOSITE_SEGMENT, FRAME_MISSING_CAP_FACES (warning).
- **OCCT mapping:** App data model (composite part + table). Geometry: congruence test via GProp (volume, area, principal moments from BRepGProp/GProp_PrincipalProps) with 1% relative tolerance (or BRepExtrema-based shape matching); lengths via GCPnts_AbscissaPoint on swept edges; angles from cap face normals (BRepAdaptor_Surface plane) vs path tangent.
- **Priority:** P2
