| PS-01 | Common feature mechanics (creation type, boolean step, merge scope heuristics, surface join, end bounds, offsets, draft, second direction, manipulators, preview/final, pattern awareness) | P0 | BRepAlgoAPI_Fuse/Cut/Common (+ Common(target, fuse(tools)) for Intersect); BRepBuilderAPI_Sewing for surface Add; ShapeUpgrade_UnifySameDomain; BRepExtrema_DistShapeShape/BRepAlgoAPI_Section for collision heuristics; custom Up-to-next (BOPAlgo_Splitter cell selection); app-level naming/manipulators |
| PS-02 | Extrude | P0 | BRepPrimAPI_MakePrism (solid/surface), BRepFeat_MakePrism / splitter for up-to bounds, BRepOffsetAPI_DraftAngle for draft, BRepOffsetAPI_MakeOffset (wire) for thin |
| PS-03 | Revolve | P0 | BRepPrimAPI_MakeRevol; BRepFeat_MakeRevol / BRepAlgoAPI_Splitter for up-to; thin via offset wire region or MakeThickSolid |
| PS-04 | Sweep | P0 | BRepOffsetAPI_MakePipeShell (SetMode corrected Frenet / fixed / binormal / support face; SetLaw for scale; twist via auxiliary guide - custom); thin = sheet + thicken + trim |
| PS-05 | Loft | P0 (guides/conditions P1) | BRepOffsetAPI_ThruSections (no guides); guides/end conditions/connections custom via GeomFill / BRepOffsetAPI_MakeFilling / PipeShell multi-profile |
| PS-06 | Thicken | P1 | BRepOffset_MakeSimpleOffset / BRepOffsetAPI_MakeThickSolid; two-sided = two offsets + side faces + sewing |
| PS-07 | Enclose | P2 | BOPAlgo_MakerVolume + fuse of adjacent cells |
| PS-08 | Fillet | P0 (P1 asym/partial/conic/full round; P2 curvature/overflow/smooth corners) | BRepFilletAPI_MakeFillet (constant, linear/law variable); conic/G2/asymmetric/full round/partial caps custom |
| PS-09 | Chamfer | P0 | BRepFilletAPI_MakeChamfer Add / AddDD / AddDA, ChFiDS chamfer modes; tangent (apex) measurement custom |
| PS-10 | Draft | P1 | BRepOffsetAPI_DraftAngle (neutral plane); parting-line draft custom (BRepOffsetAPI_MakeDraft + rebuild) |
| PS-11 | Body draft | P2 | custom: isocline split + BRepOffsetAPI_MakeDraft surfaces to parting entity + fuse |
| PS-12 | Shell | P0 | BRepOffsetAPI_MakeThickSolid::MakeThickSolidByJoin; hollow via offset inner shell |
| PS-13 | Hole | P0 (standards/tapped P1, PEM/pipe P2) | revolved tool profiles (BRepPrimAPI_MakeRevol) + ray casting (IntCurvesFace_ShapeIntersector) + BRepAlgoAPI_Cut; hole tables as data; cosmetic thread metadata (XCAF PMI) |
| PS-14 | Rib | P2 | prism of offset profile region + splitter/classify against part + DraftAngle + fuse |
| PS-15 | Gusset | P2 | planar gusset face + BRepPrimAPI_MakePrism (frames section) |
| PS-16 | Wrap | P2 | custom isometric mapping to cylinder/cone pcurves + BRepBuilderAPI_MakeFace; BRepFeat_SplitShape for imprint; thicken for solid |
| PS-17 | Boolean | P0 | BRepAlgoAPI_Fuse/Cut/Common, Sewing+MakeSolid for surface union, offset via BRepOffsetAPI_MakeOffsetShape |
| PS-18 | Split | P0 (face split P1) | BRepAlgoAPI_Splitter (part); BRepFeat_SplitShape + BRepProj_Projection / BRepOffsetAPI_NormalProjection (face) |
| PS-19 | Transform | P0 | BRepBuilderAPI_Transform (gp_Trsf), BRepBuilderAPI_GTransform for non-uniform scale, BRepBuilderAPI_Copy |
| PS-20 | Copy part (deprecated) | P2 | BRepBuilderAPI_Copy |
| PS-21 | Linear pattern | P0 | BRepBuilderAPI_Transform per instance + PS-01 booleans; face pattern via feature-volume extraction (custom); feature pattern = app regeneration loop |
| PS-22 | Circular pattern | P0 | gp_Trsf::SetRotation + BRepBuilderAPI_Transform + booleans |
| PS-23 | Curve pattern | P1 | BRepAdaptor_CompCurve + GCPnts_AbscissaPoint + GeomFill frames (CorrectedFrenet/ConstantBiNormal) + Transform |
| PS-24 | Mirror | P0 | gp_Trsf::SetMirror + BRepBuilderAPI_Transform + Fuse/UnifySameDomain with seed |
| PS-25 | Delete face | P1 | BRepAlgoAPI_Defeaturing (heal); BRepTools_ReShape (leave open); planar cap + sewing |
| PS-26 | Move face | P1 | BRepOffset_MakeOffset per-face (offset); translate/rotate custom local operation (surface transform + re-intersection) |
| PS-27 | Replace face | P1 | custom (template surface substitution + neighbor re-intersection); no OCCT equivalent |
| PS-28 | Modify fillet | P1 | custom fillet recognition + BRepAlgoAPI_Defeaturing + BRepFilletAPI_MakeFillet |
| PS-29 | Delete part | P0 | app data model (body removal, composite handling) |
| PS-30 | External thread | P1 | cosmetic thread metadata (app/XCAF); BRepFilletAPI_MakeChamfer AddDA, face split, offset for undercut |
| PS-31 | Decal | P2 | renderer texture projection + attribute; RWGltf texture export |
| PS-32 | Derived mirror (internal, assembly mirror) | P2 | derive + gp_Trsf::SetMirror |
| PS-33 | Primitive features (Cube, Sphere, fCuboid, fCylinder, fCone, fEllipsoid; FS-only) | P2 | BRepPrimAPI_MakeBox/Sphere/Cylinder/Cone; GTransform for ellipsoid |
| PS-34 | Section part features (internal, drawings/assembly sections) | P1 | BRepAlgoAPI_Splitter / Cut with half-space or section-line prism |
