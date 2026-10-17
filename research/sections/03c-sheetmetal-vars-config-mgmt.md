## 3C. Part Studio — sheet metal, variables, configurations, feature-tree management, parts & properties

Scope: sheet metal (SM-), variables / expressions / Variable Studio / query variables (VAR-), configurations (CFG-),
Part Studio feature-tree management, part data, materials, appearances, derived, composite parts (PSM-).
Help URLs use the pattern `https://cad.onshape.com/help/Content/<Area>/<page>.htm`. FS = FeatureScript standard library
(mirror 2026-05). Bounds notation: "min .. max, default X mm / Y in" (metric / imperial document default).
Numeric-field entry mechanics (scroll increments, decimal comma) are owned by the Document section; this section owns the
expression language as used by variables and parameters.

### 3C.1 Sheet metal

Common facts for all SM features (from `sheetMetalAttribute.fs` header and help):
- Features defined with `defineSheetMetalFeature` act only on *active* sheet metal models (a model is active from its
  creating `sheetMetalStart` / `sheetMetalLoft` until a `sheetMetalEnd` "Finish" feature).
- Selection filter keywords used below: `SheetMetalDefinitionEntityType.EDGE/FACE/VERTEX` (= entity of the folded or flat
  SM body that maps back to an edge/face/vertex of the hidden master surface), `AllowFlattenedGeometry.YES` (pick may be made
  in the flat-pattern view), `ModifiableEntityOnly.YES` (no derived / in-context read-only geometry), `ActiveSheetMetal.YES/NO`.
- Shared bounds: thickness `SM_THICKNESS_BOUNDS` 2e-5 m .. 500 m, default 1.6 mm / 0.0625 in; bend radius
  `SM_BEND_RADIUS_BOUNDS` 1e-5 m .. 500 m, default 2.3 mm / 0.09 in; minimal gap `SM_MINIMAL_CLEARANCE_BOUNDS`
  2e-5 m .. 1 m, default 0.02 mm / 0.001 in; relief size `SM_RELIEF_SIZE_BOUNDS` 2e-5 m .. 500 m, default 5 mm / 0.25 in;
  K factor `K_FACTOR_BOUNDS` 0 .. 1, default 0.45; rolled K factor 0 .. 1, default 0.5; corner relief scale 1.0 .. 2.0,
  default 1.5; bend relief depth scale 1.0 .. 5.0, default 2.0; bend relief width scale 0.0625 .. 2.0, default 1.0625.
- Many SM fields carry `UIHint.REMEMBER_PREVIOUS_VALUE`: the last used value becomes the default for the next feature
  "across sessions and documents" (help, hem/model pages).
- Sheet metal features cannot be feature-patterned (`SHEET_METAL_NO_FEATURE_PATTERN` "Sheet metal features cannot be
  patterned."); faces/flanges can be patterned with face patterns (see SM-21).

### SM-01 Sheet metal master model (architecture)
- **Purpose:** Data model that makes "simultaneous" sheet metal possible: one hidden definition surface per SM model drives
  both the folded solid and the flat pattern, and the Bend/Rip table.
- **Sources:** `sheetMetalAttribute.fs` header; `sheetMetalStart.fs` (`sheetMetalRecognize` doc comment); help
  PartStudio/sheet_metal_model.htm.
- **Selection inputs:** n/a (concept).
- **Parameters:** n/a. Attribute schema (`SMAttribute`, from `canBeSMAttribute`), each editable parameter stored as
  `{value, canBeEdited, controllingFeatureId, parameterIdInFeature}` so that the table can route an edit back to the feature that owns it:

| objectType | Fields |
|---|---|
| MODEL | frontThickness, backThickness, minimalClearance (+ K factor, radius, relief defaults from sheetMetalStart) |
| WALL | (attribute id) - planar or rolled (cylinder/cone/extruded) face of the master surface |
| JOINT | jointType BEND / RIP / TANGENT; BEND: bendType, radius, angle, `unfolded` (raw bool); RIP: jointStyle EDGE / BUTT / BUTT2, angle, minimalClearance |
| CORNER | cornerStyle (SMReliefStyle), cornerReliefScale, bendReliefScale, cornerBreaks[] (SMCornerBreak) |

- **Behavior / semantics:**
  - The *master body* is a hidden surface (sheet) body "not accessible from other features". SM features edit the master
    body and its attributes directly; `updateSheetMetalGeometry` then rebuilds (a) the 3D folded solid and (b) the flat
    pattern from it after every SM feature, so geometry and errors exist in both views at every step.
  - On the master surface, bends are represented as *sharp edges* between walls (virtual sharps) carrying a BEND joint
    attribute with radius; the recognizer "replaces cylinders with sharp edges, when possible ... cylinders or sharp edges
    replacing them are annotated as Bends preserving original radius". Rolled walls stay as curved faces joined by TANGENT
    joints (no bend line in flat, listed as tangent joint in table).
  - Walls are offset by front/back thickness (Opposite direction flips which side the material lies) to build the solid;
    bends become cylindrical bend regions of inner radius R; rips become gaps of minimal clearance; corners/bend ends get
    reliefs per CORNER attributes.
  - Ordinary geometry operations on active SM bodies throw `SHEET_METAL_PARTS_PROHIBITED` ("Active sheet metal models are
    not allowed."), except operations Onshape explicitly translates back to the master (Extrude-Remove, Hole, Move face,
    Boolean subtract, face patterns, constant fillet/chamfer -> corner break; see SM-21).
  - Multiple SM models can be active at once; one model may consist of several parts.
- **Edge cases & errors:** `SHEET_METAL_REBUILD_ERROR` "Sheet metal model cannot be built."; `SHEET_METAL_COULD_NOT_UNFOLD`;
  `SHEET_METAL_SELF_INTERSECTING_MODEL` "Collision in sheet metal model."; `SHEET_METAL_SELF_INTERSECTING_FLAT` "Collision
  in sheet metal flat pattern."; `SHEET_METAL_ORPHANED_BEND`; `BEND_BAD_CONFIGURATION` (edge/vertex not a precise offset);
  `DEFINED_IN_3D_CANT_REFERENCE_SM_FLAT` / `DEFINED_IN_SM_FLAT_CANT_REFERENCE_3D`.
- **OCCT mapping:** OCCT has no sheet-metal module. (design suggestion) for noshape:
  - `SheetMetalModel { id, master: TopoDS_Shell, attrs: HashMap<PersistentName, SmAttr>, params: ModelParams, active: bool }`
    stored in the Part Studio context like any body but hidden; attributes keyed by persistent topological names (PSM-02),
    not by TopoDS hash.
  - Folded build: for each WALL face build a slab (`BRepPrimAPI_MakePrism` of the face along its normal by
    front/back thickness, or `BRepOffsetAPI_MakeThickSolid` / `BRepOffset_MakeSimpleOffset` for curved walls), trim walls by
    bend setback, build each bend as `BRepPrimAPI_MakeRevol` of the thickness strip about the bend axis (inner radius R,
    angle = dihedral), fuse (`BRepAlgoAPI_Fuse` + `ShapeUpgrade_UnifySameDomain`), cut reliefs (`BRepAlgoAPI_Cut` with
    generated relief tool prisms), leave rips as gaps.
  - Flat build: spanning tree over the wall adjacency graph from a fixed wall; each bend contributes a planar strip of
    length = bend allowance (SM-22); place walls with composed `gp_Trsf` (`BRepBuilderAPI_Transform`); union in 2D
    (`BRepAlgoAPI_Fuse` of planar faces), extrude by thickness for the flat solid; rolled walls unrolled analytically
    (cylinder: arc length at neutral radius; cone: developed sector).
  - Collision checks: `BRepAlgoAPI_Common` / `BRepExtrema_DistShapeShape` between non-adjacent walls (folded) and between
    flat regions (flat).
  - Keep master, folded and flat bodies linked by a face-correspondence map so selection in any view resolves to the master entity.
- **Priority:** P1

### SM-02 Sheet metal model (sheetMetalStart)
- **Purpose:** Create and activate a sheet metal model by Convert (wrap existing parts/surfaces), Extrude (sketch curves),
  or Thicken (faces / sketch regions).
- **Sources:** PartStudio/sheet_metal_model.htm; FS: `sheetMetalStart` (sheetMetalStart.fs), predicate
  `sheetMetalModelParameters`. Feature carries "Parameter Library Purpose Id" (hook for a sheet metal parameter/gauge
  library; no help page in corpus - see Gaps).
- **Selection inputs:** Convert: "Parts and surfaces to convert" (solid or sheet bodies, mesh allowed), "Faces to exclude"
  (faces). Extrude: "Sketch curves to extrude" (sketch edges, non-construction), "Arcs to extrude as bends" (sketch arcs).
  Thicken: "Faces or sketch regions to thicken" (planar, cylindrical, extruded, conical faces). Convert/Thicken: "Edges or
  cylinders to bend" (two-sided linear edges or cylindrical faces, non-sketch). Hidden `initEntities` (pre-selection seed).
- **Parameters:**

| FS id | UI label | Type | Enum / default / bounds | Visible when / notes |
|---|---|---|---|---|
| process | Process | SMProcessType | CONVERT "Convert" (default), EXTRUDE "Extrude", THICKEN "Thicken" | horizontal enum |
| partToConvert | Parts and surfaces to convert | Query | | Convert |
| facesToExclude | Faces to exclude | Query | | Convert |
| sketchCurves | Sketch curves to extrude | Query | | Extrude |
| bendArcs | Arcs to extrude as bends | Query | | Extrude |
| endBound | End type | SMExtrudeBoundingType | BLIND "Blind", UP_TO_NEXT "Up to next", UP_TO_SURFACE "Up to face", UP_TO_BODY "Up to part", UP_TO_VERTEX "Up to vertex" | Extrude |
| oppositeExtrudeDirection | Opposite direction | bool | false | Extrude |
| depth | Depth | length | LENGTH_BOUNDS -500..500 m, default 25 mm / 1 in; tolerance-capable | Blind |
| endBoundEntityFace | Up to face | Query (face or mate connector, 1) | | Up to face |
| endBoundEntityBody | Up to surface or part | Query (solid/sheet body, 1) | | Up to part |
| endBoundEntityVertex | Up to vertex or mate connector | Query (vertex, 1) | | Up to vertex |
| hasOffset / offsetDistance / offsetOppositeDirection | Offset distance / Opposite direction | bool / length / bool | LENGTH_BOUNDS | Up to next/face/part/vertex |
| symmetric | Symmetric | bool | false | Blind |
| hasSecondDirection | Second end position | bool | false | not symmetric |
| secondDirectionBound ... | End type / Depth / Up to ... / Offset (second) | as first direction | secondDirectionOppositeExtrudeDirection default true | Second end position |
| regions | Faces or sketch regions to thicken | Query | | Thicken |
| tangentPropagation | Tangent propagation | bool | false | Thicken |
| bends | Edges or cylinders to bend | Query | | Convert, Thicken |
| clearance | Clearance from input | length | 0 .. 500 m, default 0 | Convert, Thicken |
| bendsIncluded | Include bends | bool | false | Convert, Thicken; include clearance for bends |
| keepInputParts | Keep input parts | bool | false | Convert |
| thickness | Thickness (group General) | length | SM_THICKNESS_BOUNDS, default 1.6 mm / 0.0625 in | remembered |
| oppositeDirection | Opposite direction | bool | false | flips thickness side |
| radius | Bend radius | length | SM_BEND_RADIUS_BOUNDS, default 2.3 mm / 0.09 in | inside radius; remembered |
| flipDirectionUp | Flip direction up | bool | false | reverses model and flat-view orientation (bend up/down) |
| bendCalculationType | Bend calculation (group Material, collapsed) | SMBendCalculationType | K_FACTOR "K Factor" (default), BEND_ALLOWANCE "Bend allowance", BEND_DEDUCTION "Bend deduction" | becomes a table column |
| kFactor | Default bend K Factor | real | 0 .. 1, default 0.45 | |
| kFactorRolled | Rolled K Factor | real | 0 .. 1, default 0.5 | rolled walls |
| minimalClearance | Minimal gap (group Relief, collapsed) | length | SM_MINIMAL_CLEARANCE_BOUNDS, default 0.02 mm / 0.001 in | rip gap |
| defaultCornerStyle | Corner relief type | SMCornerStrategyType | SIZED_RECTANGLE "Square - Sized", RECTANGLE "Rectangle - Scaled", SIZED_ROUND "Round - Sized", ROUND "Round - Scaled", CLOSED "Closed", SIMPLE "Simple" (default) | |
| defaultCornerReliefScale | Corner relief scale | real | 1.0 .. 2.0, default 1.5 | Rectangle/Round - Scaled |
| defaultRoundReliefDiameter | Corner relief diameter | length | SM_RELIEF_SIZE_BOUNDS, default 5 mm / 0.25 in | Round - Sized |
| defaultSquareReliefWidth | Corner relief width | length | SM_RELIEF_SIZE_BOUNDS | Square - Sized |
| defaultBendReliefStyle | Bend relief type | SMBendStrategyType | RECTANGLE "Rectangle - Scaled", OBROUND "Obround - Scaled" (default), TEAR "Tear" | |
| defaultBendReliefDepthScale | Bend relief depth scale | real | 1.0 .. 5.0, default 2.0 | Obround/Rectangle |
| defaultBendReliefScale | Bend relief width scale | real | 0.0625 .. 2.0, default 1.0625 | Obround/Rectangle |

- **Behavior / semantics:**
  - Convert: encloses the selected parts with sheet of given thickness, offset by Clearance (bends optionally included in the
    clearance); selected edges/cylinders become bends, unselected sharp edges become rips; unselected arcs/splines become
    tangent joints (rolled walls). Keep input parts retains the original bodies; otherwise they are consumed.
  - Extrude: extrudes open/closed sketch curves (lines, arcs, splines) into walls; arcs/splines produce rolled walls with
    tangent joints unless picked in "Arcs to extrude as bends", in which case they become bends whose radius comes from the
    sketch (radius cell read-only in table). Line-line corners become bends with model radius.
  - Thicken: thickens faces/regions; edges chosen in "Edges or cylinders to bend" become bends, others rips/tangent joints.
  - Conical parts revolving around a single axis can be converted (`SHEET_METAL_NO_CONE_APEX` if apex present).
  - Help gives relief formulas: bend relief depth = touching-bend depth + (depthScale - 1) * bendRadius; bend relief width =
    thickness * widthScale. Help text for Extrude/Thicken says depth scale "between 1.00 and 2.00" (older text) vs FS bound
    1..5 - FS is authoritative.
  - Each created model is listed in the Bend/Rip table and gets a flat pattern; new parts named "Part N" (inferred, same
    naming as other part-creating features).
- **Edge cases & errors:** `SHEET_METAL_CONVERT_PLANE`, `SHEET_METAL_INVALID_FACE` ("Only planar, cylindrical, conical or
  extruded faces can be converted"), `SHEET_METAL_CANNOT_THICKEN`, `SHEET_METAL_TOO_THICK`, `SHEET_METAL_START_SELECT_BENDS`,
  `SHEET_METAL_NO_0_ANGLE_BEND` ("Edge between tangent walls cannot be a bend."), `SHEET_METAL_CYLINDER_BEND`,
  `SHEET_METAL_THICKEN_IN_CONTEXT_INFO`, `SHEET_METAL_ROLLED_CORNER_RELIF` (only Simple corner relief on rolled walls /
  cylindrical bends).
- **OCCT mapping:** Master surface creation: Extrude -> `BRepPrimAPI_MakePrism` of sketch wire (sheet); Convert ->
  `BRepOffsetAPI_MakeOffsetShape` of selected faces by clearance (+T/2 to mid-surface as chosen) then remove excluded faces;
  Thicken -> copy faces (`BRepBuilderAPI_Copy`) + sew (`BRepBuilderAPI_Sewing`); replace selected cylinders by sharp edges
  (intersect adjacent wall planes, `GeomAPI_IntSS`). Then SM-01 build.
- **Priority:** P1

### SM-03 Flange (sheetMetalFlange)
- **Purpose:** Add a wall to each selected edge, joined by a bend.
- **Sources:** PartStudio/sheet_metal_flange.htm; FS: `sheetMetalFlange` (sheetMetalFlange.fs), predicate `partialFlangePredicate`.
- **Selection inputs:** "Edges or side faces to flange": SM definition edges - linear edges (flat view allowed) or planar
  side faces (folded only); multi-pick. Up-to entity: face or vertex, 1. Parallel to / Direction: any direction-bearing
  entity except mate connector, 1.
- **Parameters:**

| FS id | UI label | Type | Enum / default / bounds | Visible when / notes |
|---|---|---|---|---|
| edges | Edges or side faces to flange | Query | | |
| flangeAlignment | Flange alignment | SMFlangeAlignment | INNER "Inner", OUTER "Outer", MIDDLE "Middle", BEND "Hold line" | |
| limitType | End type | SMFlangeBoundingType | BLIND "Blind", UP_TO_ENTITY "Up to entity", UP_TO_ENTITY_OFFSET "Up to entity with offset" | |
| distance | Distance | length | 1e-5 .. 500 m, default 25 mm / 1 in | Blind |
| limitEntity | Up to entity | Query (face/vertex, 1) | | Up to entity (+offset) |
| offset / oppositeOffsetDirection | Offset / Opposite offset direction | length / bool | 1e-5 .. 500 m | Up to entity with offset |
| angleControlType | Angle control type | SMFlangeAngleControlType | BEND_ANGLE "Bend angle", ALIGN_GEOMETRY "Align to geometry", ANGLE_FROM_DIRECTION "Angle from direction" | |
| oppositeDirection | Opposite side | bool | false | |
| bendAngle | Bend angle | angle | 1 .. 179 deg, default 90 | Bend angle |
| parallelEntity | Parallel to | Query (1) | | Align to geometry |
| directionEntity / angleFromDirection / angleFromDirectionOppositeAngle | Direction / Angle / Opposite angle | Query / angle / bool | angle 0 .. 180 deg, default 90 | Angle from direction (help says 1-359; FS 0-180) |
| autoMiter | Automatic miter | bool | true | |
| miterAngle | Miter angle | angle | 0 .. 179 deg, default 45 | !autoMiter |
| useDefaultRadius | Use model bend radius | bool | true | |
| bendRadius | Bend radius | length | SM_BEND_RADIUS_BOUNDS | !useDefaultRadius |
| isPartialFlange | Partial flange | bool | false | |
| chainType | Chain type | SMPartialFlangeChainType | PER_EDGE "Per edge", PER_CHAIN "Per chain" | partial (group Overall parameters) |
| flipFlangeBounds | Flip sides | bool | false | partial |
| holdAdjacentEdges | Hold adjacent edges | bool | true | partial |
| firstBoundType | Bound type | SMFlangeBoundingType | as End type | partial (group End conditions) |
| firstBoundOffset | Distance | length | 0 .. 500 m, default 25 mm | first bound Blind |
| firstBoundingEntity / firstBoundingEntityOffset / firstBoundingEntityOppositeOffsetDirection | Up to entity / Distance / Opposite offset direction | Query / length / bool | 0 .. 500 m | first bound Up to entity (+offset) |
| hasSecondBound | Second bound | bool | false | partial |
| secondBoundType, secondBoundOffset, secondBoundingEntity, secondBoundingEntityOffset, secondBoundingEntityOppositeOffsetDirection | Second bound type / Second distance / ... | as first | | Second bound |

- **Behavior / semantics:**
  - Blind distance measured from the virtual sharp along the outer edge of the base wall to the flange tip (FS doc + help).
  - Alignment: Inner = inner face of flange aligned with the inner edge of the side lip; Outer = outer face aligned with the
    outer edge; Middle = flange mid-plane halfway; Hold line = bend starts at the selected edge.
  - Bend angle measured from the base wall; Align to geometry = flange parallel to selection; Angle from direction = angle
    from the given direction. Opposite side flips bend side.
  - Auto miter trims/extends adjacent flanges on consecutive edges to meet in a miter; otherwise flange sides rotated by miter angle.
  - Partial flange: flange spans only a portion of the edge, bounded by offsets from edge ends or up to a plane/vertex;
    Per edge vs Per chain applies bounds per individual edge or per connected chain; Hold adjacent edges keeps the flange's
    side edges extended to full edge length (help wording).
  - Result: new WALL face + BEND joint added to master; flat pattern updates. To modify later, use Move face.
- **Edge cases & errors:** `SHEET_METAL_FLANGE_NO_EDGES`, `SHEET_METAL_FLANGE_NON_LINEAR_EDGES`, `SHEET_METAL_FLANGE_INTERNAL`
  (edge is joint/fillet/chamfer/internal), `SHEET_METAL_FLANGE_FAIL_ALIGNMENT`, `..._FAIL_UP_TO`, `..._FAIL_UP_TO_ENTITY`,
  `..._FAIL_LIMIT_OPP_FLANGE`, `..._FAIL_AUTO_MITER`, `..._FAIL_PARALLEL_EDGE/PLANE/DIRECTION`, `..._FAIL_NO_BEND` (0 deg),
  `..._NO_PARALLEL_ENTITY`, `..._NO_DIRECTION_ENTITY`, `..._PARAMETER_BOUND` (partial beyond base edge), `..._ZERO_WIDTH`,
  `..._NO_BOUNDING_ENTITY`, `..._BOUNDING_ENTITY_PARALLEL`, `..._BOUNDING_ENTITY_NOT_SUPPORTED` (vertex or plane only),
  `SHEET_METAL_FLANGE_NEXT_TO_CYLINDER_BEND`, `SHEET_METAL_FLANGE_ADJACENT_CONE` (use Hold line), `SHEET_METAL_MULTI_SM_DEFAULT_RADIUS`.
- **OCCT mapping:** Master edit: new planar face built from edge + direction (`BRepBuilderAPI_MakeFace` on `gp_Pln`),
  trimmed by bounds; miter by intersecting adjacent flange planes (`BRepAlgoAPI_Section`); attach BEND attribute. Rebuild per SM-01.
- **Priority:** P1

### SM-04 Tab (sheetMetalTab)
- **Purpose:** Add planar tab faces to parallel walls (merge), optionally removing interfering material, or bridging two flanges.
- **Sources:** PartStudio/sheet_metal_tab.htm; FS: `sheetMetalTab`.
- **Selection inputs:** "Tab profile" (planar faces / sketch regions, non-construction, multi); "Flange to merge" (SM
  definition faces, flat allowed); "Subtraction scope" (SM faces or non-SM solid bodies).
- **Parameters:**

| FS id | UI label | Type | Enum / default / bounds | Visible when / notes |
|---|---|---|---|---|
| tabFaces | Tab profile | Query | | |
| booleanUnionScope | Flange to merge | Query | auto-filled with parallel walls (editing logic) | |
| booleanOffset | Subtraction offset | length | 0 .. 500 m, default 0 | clearance between tab and subtracted wall |
| booleanSubtractScope | Subtraction scope | Query | | |

- **Behavior / semantics:** tab regions coplanar/parallel with a wall are united into that wall of the master; where a tab
  crosses walls listed in Subtraction scope, those walls are cut with offset clearance; normal parts may be subtracted too.
  Reflected in flat pattern.
- **Edge cases & errors:** `SHEET_METAL_TAB_NO_BEND` ("Tab can only be added to a free edge or rip."), `..._NO_MERGE`,
  `..._NONPLANAR`, `..._NO_WALL`, `..._NO_TAB`, `..._NO_PARALLEL_WALL`, `..._NO_EFFECT`, `..._LOW_CLEARANCE`,
  `..._FAILS_MERGE`, `..._COLLISION`, `SHEET_METAL_TAB_MERGE_AND_SUBTRACT_SAME_FLANGE`.
- **OCCT mapping:** 2D union/cut on the master wall face (`BRepAlgoAPI_Fuse` / `BRepAlgoAPI_Cut` of coplanar faces, then
  `ShapeUpgrade_UnifySameDomain`); offset tool via `BRepOffsetAPI_MakeOffset` (2D wire offset).
- **Priority:** P2

### SM-05 Hem (sheetMetalHem)
- **Purpose:** Fold an edge back on itself (straight, rolled, tear drop).
- **Sources:** PartStudio/sheet_metal_hem.htm; FS: `sheetMetalHem`.
- **Selection inputs:** "Edges or side faces to hem": same filter as flange.
- **Parameters:**

| FS id | UI label | Type | Enum / default / bounds | Visible when / notes |
|---|---|---|---|---|
| edges | Edges or side faces to hem | Query | | |
| hemType | Hem type | SMHemType | STRAIGHT "Straight", ROLLED "Rolled", TEAR_DROP "Tear drop" | remembered |
| oppositeDirection | Opposite direction | bool | false | |
| useMinimalGapForStraight | Flattened | bool | true | Straight; gap = model minimal gap |
| innerRadius | Inner radius | length | SM_BEND_RADIUS_BOUNDS | Rolled, Tear drop, Straight & !Flattened |
| useMinimalGapForTipGap | Use minimal gap | bool | true | Tear drop |
| tipGap | Gap | length | SM_MINIMAL_CLEARANCE_BOUNDS | Tear drop & !Use minimal gap |
| angle | Angle | angle | 180 .. 359.9 deg, default 270 | Rolled |
| length | Total length | length | 1e-5 .. 500 m, default 12.5 mm / 0.5 in | Tear drop, Straight |
| hemAlignment | Hem alignment | SMHemAlignment | OUTER "Outer", IN_PLACE "In place" | |
| hemCornerType | Corner type | SMHemCornerType | SIMPLE "Simple", CLOSED "Closed" | |

- **Behavior / semantics:** Total length measured from the outermost edge of the hem to its end. Outer alignment puts the
  outside of the hem at the original edge (material added inward); In place starts the hem at the original edge. Simple
  corners are straight lines in flat; Closed closes the corner for welding (more complex flat geometry). Hems appear in the
  bend table with radius and angle; may be moved up/down but not edited in the table.
- **Edge cases & errors:** `SHEET_METAL_HEM_NO_EDGES`, `..._NON_LINEAR_EDGES`, `..._FAIL_ALIGNMENT`, `..._TOO_SHORT`
  ("increase total length"), `..._TEAR_DROP_GAP_TOO_LARGE`, `..._INTERNAL`, `..._FAILED`, `..._NEXT_TO_CYLINDER_BEND`,
  `SHEET_METAL_HEM_ADJACENT_CONE` (select In place).
- **OCCT mapping:** Master edit adds 1-2 walls + bends (straight: 180 deg bend with gap; rolled: rolled cylindrical wall of
  given angle; tear drop: bend + angled return wall). Build per SM-01.
- **Priority:** P2

### SM-06 Jog
- **Purpose:** Offset part of a wall with two opposite bends along a line (S/Z shape).
- **Sources:** PartStudio/sheet_metal_jog.htm. No FS precondition in the stdlib mirror (feature newer than mirror) -
  parameters below are from help only; FS ids unknown (names given are descriptive, not FS ids).
- **Selection inputs:** "Bend line" (line/edge, 1; need not lie on the face - projected and extended); "Sheet metal face to
  bend" (one planar SM face per feature); Up to entity (face) for bounded offset.
- **Parameters (help):**

| FS id | UI label | Type | Enum / default / bounds | Visible when / notes |
|---|---|---|---|---|
| (n/a) | Hold opposite side | bool | false | which side moves |
| (n/a) | Bend alignment | enum | Bend line, Hold line, Hold other line, Inner, Outer, Middle | as SM-07 |
| (n/a) | Angle control type | enum | Bend angle (1-359 deg, Opposite angle), Align to geometry (Parallel to), Angle from direction (Direction, Angle 1-180 deg, opposite) | |
| (n/a) | Use model bend radius / Bend radius | bool / length | true | |
| (n/a) | Use model K factor / K Factor | bool / real | true | |
| (n/a) | Bounding type | enum | Blind (Jog offset), Up to entity (+ Offset distance), Thickness (Thickness factor x model thickness) | |
| (n/a) | Jog offset anchor | enum | Inside, Nominal (overall), Outside | how jog height is measured |
| (n/a) | Preserve material | bool | true | unchecked adds material to keep original flat length |

- **Behavior / semantics:** two bends of equal and opposite angle; offset measured per anchor (Inside: top face to inner
  parallel face; Nominal: bottom face to outer face; Outside: top face to outer parallel face). Custom K factor affects only
  the folded model. Bend may not collide with earlier Bend/Corner features.
- **Edge cases & errors:** same family as Bend (`SHEET_METAL_BEND_*`, `SM_BEND_ANGLE_TOO_SMALL`).
- **OCCT mapping:** implement as two SM-07 bends on the master (imprint two parallel lines with `BRepAlgoAPI_Splitter`,
  rotate the far region). 
- **Priority:** P2

### SM-07 Bend (sheetMetalBend)
- **Purpose:** Fold an existing planar wall along a reference line (e.g. bend a flat DXF-derived blank).
- **Sources:** PartStudio/sheet_metal_bend.htm; FS: `sheetMetalBend`.
- **Selection inputs:** "Bend line" (any line, 1); "Sheet metal face to bend" (planar SM definition face, modifiable, 1);
  Parallel to / Direction (direction entities except mate connector, 1).
- **Parameters:**

| FS id | UI label | Type | Enum / default / bounds | Visible when / notes |
|---|---|---|---|---|
| bendReference | Bend line | Query | | |
| holdOtherSide | Hold opposite side | bool | false | |
| face | Sheet metal face to bend | Query | | |
| bendAlignment | Bend alignment | BendAlignment | BEND_LINE "Bend line", HELD_EDGE "Hold line", BENT_EDGE "Hold other line", BENT_INSIDE "Inner", BENT_OUTSIDE "Outer", BENT_MIDPLANE "Middle" | |
| angleControlType | Angle control type | BendAngleControlType | BEND_ANGLE, ALIGN_GEOMETRY, ANGLE_FROM_DIRECTION | |
| oppositeAngle | Opposite angle | bool | false | circular flip |
| bendAngle | Bend angle | angle | 1 .. 359.9 deg, default 90 | Bend angle |
| parallelEntity | Parallel to | Query | | Align to geometry |
| directionEntity / angleFromDirection / oppositeAngleFromDirection | Direction / Angle / Opposite angle | Query / angle / bool | 0 .. 180 deg, default 90 | Angle from direction |
| useDefaultRadius / bendRadius | Use model bend radius / Bend radius | bool / length | true / SM_BEND_RADIUS_BOUNDS | |
| useDefaultKFactor / kFactor | Use model K Factor / K Factor | bool / real | true / 0 .. 1, default 0.45 | |

- **Behavior / semantics:** Bend line need not be on the face, is projected onto it, extended to face edges, may cross
  multiple cuts; first three alignments position the line relative to the flat pattern (middle / start / end of bend
  region), last three relative to the folded wall. The bend does not change flat-pattern dimensions; custom K factor changes
  only the folded length. BA/BD values not editable here (edit in model feature or table).
- **Edge cases & errors:** `SHEET_METAL_BEND_NO_BEND_LINE`, `..._BAD_BEND_LINE`, `..._NO_FACE`, `..._NO_PARALLEL`,
  `..._NO_DIRECTION`, `..._BAD_FACE`, `..._IMPRINT_FAILED`, `..._BAD_DECOMPOSITION` (line must split face into >= 2 parts),
  `..._LINE_PERPENDICULAR_TO_FACE`, `..._ROLL_FAILED`, `SHEET_METAL_BOTH_SIDES_CONNECTED`, `SHEET_METAL_CANNOT_BEND_BUTTS`,
  `SHEET_METAL_BEND_COLLISION`, `SM_BEND_ANGLE_TOO_SMALL`.
- **OCCT mapping:** project line onto face (`BRepProj_Projection` / `GeomAPI_ProjectPointOnSurf`), split the master face
  (`BRepAlgoAPI_Splitter`), rotate the moving sub-graph about the bend axis (`gp_Trsf::SetRotation`), mark new edge BEND.
- **Priority:** P2

### SM-08 Form (sheetMetalFormed)
- **Purpose:** Stamp a form (louver, lance, emboss, countersink ...) from a form Part Studio onto SM walls, in model and flat.
- **Sources:** PartStudio/sheet_metal_form.htm; FS: `sheetMetalFormed`; Library/sheet_metal_form_library.htm.
- **Selection inputs:** "Form Part Studio" (PartStudioData, entire Part Studio, 1, from current doc / other docs / form
  libraries; config inputs shown inline; computed config input `thickness`); "Location(s)" (mate connectors or sketch
  vertices); "Target face(s)" (planar faces of active SM).
- **Parameters:**

| FS id | UI label | Type | Enum / default / bounds | Visible when / notes |
|---|---|---|---|---|
| formPartStudio | Form Part Studio | PartStudioData | library definition id "65dcc2bb2c4ff1c239467eca" | `thickness` config input driven by SM thickness |
| locations | Location(s) | Query | | sketch points use sketch CSys for orientation |
| flipDirection | Flip direction | bool | false | form on opposite side |
| targetFaces | Target face(s) | Query | | |

- **Behavior / semantics:** The form Part Studio must have a configuration variable named `thickness` (type Length) and one
  Form Tag (PSM-30): "Part to add" is united to the sheet, "Part to remove" is subtracted; the tag's Form origin mate
  connector is aligned to each location's CSys. Optional form sketch (construction-only) appears on the flat pattern and in
  flat DXF/drawing (outline + centermarks). Hiding the Form feature hides its flat sketch. Forms referencing a library form keep
  the previous version until the user reselects the updated form.
- **Edge cases & errors:** forms cannot touch side walls, rolled walls, rips, joints or corners (`SHEET_METAL_CANNOT_CUT`);
  corner breaks do not work on form edges; `SHEET_METAL_FORMED_REBUILD_FAILED`.
- **OCCT mapping:** instantiate form bodies (regenerate source Part Studio with config `{thickness: T, ...}`), transform
  (`BRepBuilderAPI_Transform`), `BRepAlgoAPI_Fuse` / `BRepAlgoAPI_Cut` on the folded solid; record the form as a non-unfoldable
  "feature island" on the wall attribute so flat pattern shows only the outline sketch (design suggestion).
- **Priority:** P2

### SM-09 Sheet metal form library (forms content and custom libraries)
- **Purpose:** Provide standard and custom form templates.
- **Sources:** Library/sheet_metal_form_library.htm; PartStudio/sheet_metal_form.htm (Creating a form, form libraries).
- **Selection inputs:** n/a.
- **Parameters:** Standard Onshape forms and their config inputs: Bridge lance (Length, Width, Height, Angle, Radius control
  Double thickness/Thickness/Half thickness/Custom -> Internal & External radius); Circle emboss (Diameter, Height, Angle,
  Radius control); Circle extrusion (Diameter, Depth, Radius); Circle knockout (Diameter); Countersink (Clearance hole,
  Angle); Lance (Profile Flat/Straight, Length of flat, Width, Height, Angle, Radius control); Louver (Height, Length, Width,
  Radius control -> Radius); Slot emboss (Length, Length to Slot centers/Slot tangents, Width, Height, Angle, Radius control);
  Slot extrusion (Length, Length to centers/tangents, Width, Depth, Radius control).
- **Behavior / semantics:** One personal and one company form library (a folder designated as library); folder levels act
  as selector levels (Type, Form; up to 10 primary levels; > 3 levels require Library labels); one valid form tag per Part
  Studio; external forms require a version; library must be "updated" to publish changes.
- **Edge cases & errors:** library folders cannot be deleted or re-shared until unset.
- **OCCT mapping:** app data model (library index over documents); geometry per SM-08.
- **Priority:** P2

### SM-10 Make joint (sheetMetalMakeJoint)
- **Purpose:** Join two walls into a bend or rip by extending/trimming them.
- **Sources:** PartStudio/sheet_metal_make_joint.htm; FS: `sheetMetalMakeJoint`.
- **Selection inputs:** "Edges or side faces to join": SM definition edges, modifiable, max 2 (two edges, two faces, or face + edge).
- **Parameters:**

| FS id | UI label | Type | Enum / default / bounds | Visible when / notes |
|---|---|---|---|---|
| entities | Edges or side faces to join | Query (2) | | |
| joint | Joint type | MakeJointType | BEND "Bend", RIP "Rip" (default) | |
| useDefaultRadius / radius | Use model bend radius / Bend radius | bool / length | true | Bend |
| jointType | Joint style | SMJointStyle | EDGE "Edge joint", BUTT "Butt joint - Direction 1", BUTT2 "Butt joint - Direction 2" | Rip |

- **Behavior / semantics:** walls are extended/trimmed to their intersection; Edge joint extends both to meet at inner
  edges with minimal gap; Butt joint extends outer edges of one wall over the other (Direction 2 flips handedness); butt only
  for 90 deg joints. Creates associativity; result listed in Bend/Rip table; convert rip<->bend later from the table.
- **Edge cases & errors:** `SHEET_METAL_MAKE_JOINT_FAIL`, `SHEET_METAL_JOINT_FAIL_ADJACENT_FACES`, `SHEET_METAL_NON_90_BUTT`,
  `SHEET_METAL_RIP_MULTI_BODY`, `SHEET_METAL_RIP_FAIL_NON_PLANAR`, `SHEET_METAL_RIP_FAIL_INTERNAL_EDGE`.
- **OCCT mapping:** plane-plane intersection (`GeomAPI_IntSS`), extend faces (`BRepLib_MakeFace` on untrimmed plane, trim by
  intersection line), sew into master shell (`BRepBuilderAPI_Sewing`), add JOINT attribute.
- **Priority:** P2

### SM-11 Modify joint (sheetMetalJoint)
- **Purpose:** Change an existing joint's type/style/radius/K factor; auto-created by Bend/Rip table edits.
- **Sources:** PartStudio/sheet_metal_joint.htm; FS: `sheetMetalJoint` ("modifies sheet metal joint by changing its attribute").
- **Selection inputs:** "Joint": SM definition face or edge (flat allowed), modifiable, 1.
- **Parameters:**

| FS id | UI label | Type | Enum / default / bounds | Visible when / notes |
|---|---|---|---|---|
| entity | Joint | Query (1) | | |
| jointType | Joint type | SMJointType | BEND "Bend" (default), RIP "Rip", TANGENT "Tangent" | Tangent only if circular faces involved |
| useDefaultRadius / radius | Use model bend radius / Bend radius | bool / length | true | Bend |
| useDefaultKFactor / kFactor | Use model K Factor / K Factor | bool / real | true / 0 .. 1 | Bend (help: when unchecked also choose Bend calculation K/BA/BD; help claims K range -1.50..1.00 - differs from FS bounds 0..1) |
| hasStyle | Has style | bool | true, always hidden | Rip |
| jointStyle | Joint style | SMJointStyle | EDGE / BUTT / BUTT2 | Rip & hasStyle |

- **Behavior / semantics:** rewrites the JOINT attribute on the master edge; geometry rebuilt; "Convert to rip/bend" in the
  table creates this feature. Not editable after a Corner break exists (SM-13).
- **Edge cases & errors:** `SHEET_METAL_ACTIVE_JOIN_NEEDED`, `SHEET_METAL_FLAT_RIP_NO_EDIT`, `SHEET_METAL_CANT_CHANGE_TO_FLAT`,
  `SHEET_METAL_RIP_STYLE_ERROR`, `SHEET_METAL_NON_90_BUTT`, `SHEET_METAL_NO_0_ANGLE_BEND`.
- **OCCT mapping:** attribute edit only; rebuild per SM-01.
- **Priority:** P1

### SM-12 Corner (sheetMetalCorner)
- **Purpose:** Override the model's default corner relief at one corner.
- **Sources:** PartStudio/sheet_metal_corner.htm; FS: `sheetMetalCorner`.
- **Selection inputs:** "Corner": SM definition vertex or edge (flat allowed), 1. One corner per feature.
- **Parameters:**

| FS id | UI label | Type | Enum / default / bounds | Visible when / notes |
|---|---|---|---|---|
| corner | Corner | Query (1) | | |
| cornerStyle | Corner relief type | SMCornerReliefStyle | SIZED_RECTANGLE "Square - Sized" (default), RECTANGLE "Rectangle - Scaled", SIZED_ROUND "Round - Sized", ROUND "Round - Scaled", CLOSED "Closed", SIMPLE "Simple" | |
| cornerReliefScale | Corner relief scale | real | 1.0 .. 2.0, default 1.5 | Rectangle/Round - Scaled |
| roundReliefDiameter | Corner relief diameter | length | SM_RELIEF_SIZE_BOUNDS | Round - Sized |
| squareReliefWidth | Corner relief width | length | SM_RELIEF_SIZE_BOUNDS | Square - Sized |

- **Behavior / semantics:** writes CORNER attribute on the master vertex; Closed requires exactly two adjacent bends.
- **Edge cases & errors:** `SHEET_METAL_CORNER_SELECT_ENTITIES`, `SHEET_METAL_BEND_END_NOT_A_CORNER` (use Bend relief),
  `SHEET_METAL_NOT_A_CLOSED_CORNER`, `SHEET_METAL_CORNER_UNDER_SIZED`, `SHEET_METAL_RELIEF_FAILURES`.
- **OCCT mapping:** relief tool generation (rectangle/circle prisms at corner, sized or scaled by bend geometry),
  `BRepAlgoAPI_Cut` in flat and folded builds.
- **Priority:** P2

### SM-13 Corner break (sheetMetalCornerBreak)
- **Purpose:** Fillet or chamfer sheet-thickness corners (edges through thickness / flat-pattern vertices).
- **Sources:** PartStudio/sheet_metal_corner_break.htm; FS: `sheetMetalCornerBreak`, predicates `chamferOptions`, `filletOptions`.
- **Selection inputs:** "Entities to fillet or chamfer": linear edges or vertices that are SM definition vertices (flat
  allowed), box select adds edges. Direction overrides (chamfer): same filter. Edges to keep (overflow): two-sided edges of active SM.
- **Parameters:**

| FS id | UI label | Type | Enum / default / bounds | Visible when / notes |
|---|---|---|---|---|
| blendType | Type | EdgeBlendType | FILLET "Fillet", CHAMFER "Chamfer" | horizontal, remembered |
| entities | Entities to fillet or chamfer | Query | | |
| chamferMethod | Measurement | ChamferMethod | FACE_OFFSET "Offset", APEX_RANGE "Tangent" | Chamfer |
| chamferType | Chamfer type | ChamferType | EQUAL_OFFSETS "Equal distance", TWO_OFFSETS "Two distances", OFFSET_ANGLE "Distance and angle" | Chamfer |
| width / width1 / width2 | Distance / Distance 1 / Distance 2 | length | BLEND_BOUNDS 1e-5 .. 500 m, default 5 mm / 0.2 in | |
| oppositeDirection | Opposite direction | bool | false | Offset-angle / Two distances |
| angle | Angle | angle | 0.1 .. 179.9 deg, default 45 | Distance and angle |
| directionOverrides | Direction overrides | Query | | Two distances / Distance and angle |
| blendControlType | Measurement | BlendControlType | RADIUS "Radius", WIDTH "Width" | Fillet |
| crossSection | Control | FilletCrossSection | CIRCULAR "Distance", CONIC "Conic", CURVATURE "Curvature" | Fillet |
| radius / filletWidth | Radius / Width | length | BLEND_BOUNDS | |
| (edgeFilletCommonOptions, asymmetricFilletOption) | Rho / Magnitude / Asymmetric / Second radius / Flip asymmetric | | as edge fillet section | Fillet (shared with Fillet feature, defined in Part Studio features section) |
| allowEdgeOverflow / keepEdges | Allow edge overflow / Edges to keep | bool / Query | false | Fillet, not Curvature |

- **Behavior / semantics:** applied in the flat (2D) and baked into the definition surface: rips, corner and bend reliefs
  become fixed, so afterwards joint Type and Style are no longer editable in the table; recommended as last SM step. Works on
  bend-relief corners too. One break per vertex per feature; stack features for fillet-on-chamfer.
- **Edge cases & errors:** `SHEET_METAL_CORNER_BREAK_FAILED`, `..._NOT_A_CORNER`, `..._NO_WALL`, `..._VERTEX_NOT_FREE`,
  `..._ATTRIBUTE_EXISTS`, `..._SELECT_ENTITIES`.
- **OCCT mapping:** 2D fillet/chamfer on flat wall outline (`BRepFilletAPI_MakeFillet2d` / `ChFi2d_FilletAPI`,
  `ChFi2d_ChamferAPI`) mapped back to master face boundary; conic/curvature profiles need custom 2D curve construction.
- **Priority:** P2

### SM-14 Bend relief (sheetMetalBendRelief)
- **Purpose:** Override default relief at one bend end.
- **Sources:** PartStudio/sheet_metal_bend_relief.htm; FS: `sheetMetalBendRelief`.
- **Selection inputs:** "Bend relief": SM definition vertex or edge of a bend end (flat allowed), 1.
- **Parameters:**

| FS id | UI label | Type | Enum / default / bounds | Visible when / notes |
|---|---|---|---|---|
| bendRelief | Bend relief | Query (1) | | |
| bendReliefStyle | Bend relief type | SMBendReliefStyle | SIZED_RECTANGLE "Square - Sized", RECTANGLE "Rectangle - Scaled", SIZED_OBROUND "Obround - Sized" (default), OBROUND "Obround - Scaled", TEAR "Tear" | |
| bendReliefDepthScale | Bend relief depth scale | real | 1.0 .. 5.0, default 2.0 | Rectangle/Obround - Scaled |
| bendReliefWidthScale | Bend relief width scale | real | 0.0625 .. 2.0, default 1.0625 | Rectangle/Obround - Scaled |
| bendReliefDepth | Bend relief depth | length | SM_RELIEF_SIZE_BOUNDS | Sized |
| extendBendRelief | Extend bend relief | bool | false | not Tear; extends relief to the sheet end (flange collision case) |

- **Behavior / semantics:** writes per-bend-end relief attribute. (Help text gives scaled depth range 1.00-2.00 in one
  place, 1.00-5.00 in another; FS 1-5.)
- **Edge cases & errors:** `SHEET_METAL_BEND_RELIEF_NO_CORNER`, `..._SELECT_ENTITIES`, `SHEET_METAL_CORNER_NOT_A_BEND_END`,
  `SHEET_METAL_BEND_RELIEF_CONE`.
- **OCCT mapping:** as SM-12 (relief tool prisms + cut).
- **Priority:** P2

### SM-15 Rip (sheetMetalRip)
- **Purpose:** Split a wall between vertex pairs, creating rip joints (marked @internal in FS; no help page).
- **Sources:** FS: `sheetMetalRip` (sheetMetalRip.fs, "@internal ... produces a sheet metal rip by splitting a wall").
- **Selection inputs:** "Vertex pairs to split with": vertices of solid SM bodies (flat allowed), even count.
- **Parameters:**

| FS id | UI label | Type | Enum / default / bounds | Visible when / notes |
|---|---|---|---|---|
| vertices | Vertex pairs to split with | Query | | |
| useDefaultGap | Use default minimal gap | bool | true | |
| minimalClearance | Minimal gap | length | SM_MINIMAL_CLEARANCE_BOUNDS | !useDefaultGap |

- **Behavior / semantics:** each pair defines a split line across one planar wall; new edges get RIP attributes with gap.
- **Edge cases & errors:** `SHEET_METAL_RIP_EVEN`, `..._WALL_NOT_FOUND`, `..._SAME_VERTEX`, `..._NEED_MORE_VERTICES`,
  `..._FAIL`, `..._NO_CORNER`.
- **OCCT mapping:** `BRepAlgoAPI_Splitter` of master face by segment edge.
- **Priority:** P2

### SM-16 Sheet metal loft (sheetMetalLoft)
- **Purpose:** Create (or add to) a sheet metal part transitioning between two profiles (transition ducts), tessellated into
  planar facets with bends.
- **Sources:** PartStudio/sheet_metal_loft.htm; FS: `sheetMetalLoft`.
- **Selection inputs:** Profile 1 / Profile 2: edges, faces, wire/sheet bodies (non-sketch), or a vertex (point profile);
  box select edges. Connections: vertices or edges. Merge scope (Add): solid bodies of one active SM model.
- **Parameters:**

| FS id | UI label | Type | Enum / default / bounds | Visible when / notes |
|---|---|---|---|---|
| surfaceOperationType | (operation) | NewSurfaceOperationType | NEW "New", ADD "Add" | surfaceOperationTypePredicate |
| profile1 / profile2 | Profile 1 / Profile 2 | Query | | |
| matchConnections | Connections | bool | false | |
| connections[] | Match connections (connectionEntities "Vertices or edges", isRip "Rip" default false, hidden connectionEdgeQueries / connectionEdgeParameters) | array | | Connections; manipulators drag connection points |
| chordalTolerance | Chordal tolerance | length | 1e-5 .. 0.1 m, default 1 mm | max deviation of facets from the ideal loft surface |
| booleanScope | Merge scope | Query | | Add |
| (sheetMetalModelParameters) | General / Material / Relief groups | | as SM-02 | New |

- **Behavior / semantics:** loft surface between profiles is tessellated within chordal tolerance into planar walls joined
  by bends; closed profiles automatically get a rip; Rip in a connection splits the model there.
- **Edge cases & errors:** `SHEET_METAL_LOFT_PROFILES_TOUCH`, `..._ORTHOGONAL_PROFILE`, `..._DISJOINT_RESULT`,
  `..._MERGE_SCOPE_SHIFT`, `SHEET_METAL_ADD_WRONG_MODEL`.
- **OCCT mapping:** ruled loft (`BRepOffsetAPI_ThruSections` with ruled=true) between profiles after resampling both
  profiles to matched point sets; triangulate/quad-split into planar facets (custom); facets become master WALL faces, shared
  edges BEND (custom).
- **Priority:** P2

### SM-17 Finish sheet metal model (sheetMetalEnd)
- **Purpose:** Deactivate SM models so later features treat them as ordinary solids (post-fabrication operations).
- **Sources:** PartStudio/finish_sheet_metal_model.htm; FS: `sheetMetalEnd`.
- **Selection inputs:** "Sheet metal parts": bodies of active SM models, modifiable, multi.
- **Parameters:** `sheetMetalParts` (Query) only.
- **Behavior / semantics:** after this feature the parts are plain solids; later changes do not affect flat pattern or table
  (info `SHEET_METAL_END_DONE`). Optional if no post-fab ops are needed. Revert by deleting/suppressing it. Parts derived with
  "Preserve active sheet metal models" revert to plain parts if the source adds Finish (see PSM-25).
- **Edge cases & errors:** `SHEET_METAL_SELECT_PARTS`, `SHEET_METAL_INACTIVE_MODEL_SELECTED`, `SHEET_METAL_ACTIVE_MODEL_NEEDED`.
- **OCCT mapping:** app data model (clear `active` flag; folded solid becomes regular body; flat pattern frozen at that state).
- **Priority:** P1

### SM-18 Sheet metal table and flat view (Bend/Rip tables, flat pattern panel)
- **Purpose:** Side panel showing the flat pattern and Bend / Rip tables for one SM model, with in-table editing.
- **Sources:** PartStudio/sheet_metal_table.htm.
- **Selection inputs:** model dropdown at top of panel; rows multi-selectable; cross-highlight with graphics and flat view.
- **Parameters:** table columns (inferred from help text): ID/name, joint Type (Bend/Rip/Tangent), angle, radius, bend
  calculation column (K Factor / Bend allowance / Bend deduction per model setting), rip Style dropdown (Edge / Butt dir 1 /
  Butt dir 2). Hems listed with radius and angle (read-only).
- **Behavior / semantics:**
  - Two collapsible tables (Bends, Rips). Right-click: Move up/down (ordering), Convert to rip / Convert to bend. Double-click
    radius or K/BA/BD cell to edit; out-of-range values turn the cell red with tooltip of valid range; read-only cells raise
    `SHEET_METAL_TABLE_READ_ONLY`. Every edit creates/updates a Modify joint feature (SM-11) or edits the owning feature via
    the attribute's `controllingFeatureId/parameterIdInFeature`.
  - Selecting a rip/bend in graphics selects and scrolls to its row.
  - Flat view: right-click "New sketch" sketches directly on the flat (sketch plane = flat face); extruding a flat sketch opens
    an abbreviated Extrude (add/remove material) that is applied to the master; 3D extrudes cannot be reused as flat extrudes.
  - Flip direction up (model) reverses flat orientation. Locked after Corner break.
- **Edge cases & errors:** `SHEET_METAL_TABLE_UNKNOWN_ERROR`, `SHEET_METAL_TABLE_REGEN_ERROR` (edits not applied),
  `SM_FLAT_OP_*` family (`NO_INTERSECT`, `PARTIAL_INTERSECT`, `ADD_CROSSES_EDGE`, `NON_PLANAR_TOOL`, `NON_PLANAR_TARGET`,
  `LEGACY_MODEL`, `FACES_NOT_COPLANAR`, `CANNOT_JOIN_DIFFERENT_TRANSFORMS`, `FACES_DONT_MATCH`), `SHEET_METAL_SKETCH_DETACHED_FACE`,
  `SHEET_METAL_FLAT_OP_ROLL_FAIL`.
- **OCCT mapping:** UI (GPUI table + secondary 2D viewport); flat body from SM-01; flat-op extrude = 2D boolean on flat
  wall face then mapped back to master via the flat->master face transform map.
- **Priority:** P1

### SM-19 Flat pattern export (DXF/DWG) and flat pattern view
- **Purpose:** Export a flat pattern to DXF/DWG; flat pattern also insertable as a drawing view (owned by Drawing section).
- **Sources:** PartStudio/sheet_metal_table.htm (Export DXF/DWG of flat pattern).
- **Selection inputs:** right-click flat pattern.
- **Parameters:** file name; format DWG/DXF; version; scope (Single flat pattern part only / All flat pattern parts in the
  current model / All flat pattern parts in the Part Studio); download option; Export splines as polylines; Set z-height to
  zero and normals to positive; Include/hide bend centerlines; bend tangent lines; counterbore and countersink lines; form
  feature outlines; form feature centermarks; visible sketches.
- **Behavior / semantics:** honors custom export rules if configured (informational banner).
- **Edge cases & errors:** none documented.
- **OCCT mapping:** OCCT has no DXF writer in the open edition - custom DXF writer (or a Rust DXF crate) over the flat body's
  planar edges (`BRepAdaptor_Curve`, `GCPnts_TangentialDeflection` for polylines); bend lines from attributes.
- **Priority:** P1

### SM-20 Recognize (sheetMetalRecognize)
- **Purpose:** Convert an existing constant-thickness solid (e.g. imported) into an active SM model (marked @internal; no help page).
- **Sources:** FS: `sheetMetalRecognize` (sheetMetalStart.fs).
- **Selection inputs:** "Bodies" (any bodies).
- **Parameters:**

| FS id | UI label | Type | Enum / default / bounds | Visible when / notes |
|---|---|---|---|---|
| bodies | Bodies | Query | | |
| radius | Radius | length | 1e-5 .. 500 m, default 25 mm | radius for originally sharp edges |
| changeThickness / surfaceThickness | Change thickness / Thickness | bool / length | 1e-5 .. 500 m | |
| keepInputParts | Keep input parts | bool | false | |
| kFactor | K Factor | real | 0 .. 1e5 (POSITIVE_REAL_BOUNDS), default 1 | |
| minimalClearance | Minimal gap | length | 1e-5 .. 500 m | |

- **Behavior / semantics:** offset-pair detection extracts the definition sheet; cylinders become sharp bend edges preserving
  radius; planar faces -> walls; original sharp edges -> bends of input radius; rips not recognized (TODO in source).
- **Edge cases & errors:** `SHEET_METAL_CANNOT_RECOGNIZE_PARTS`.
- **OCCT mapping:** offset face-pair detection (custom: pair faces by `BRepExtrema_DistShapeShape` distance = T and opposite
  normals), mid-surface extraction (custom), cylinders -> plane intersections (`GeomAPI_IntSS`).
- **Priority:** P2

### SM-21 Regular features on active sheet metal
- **Purpose:** Define which non-SM features may operate on active SM and how they translate to the master.
- **Sources:** PartStudio/sheet_metal_model.htm (notes); error enum.
- **Selection inputs:** n/a.
- **Parameters:** n/a.
- **Behavior / semantics:**
  - Allowed: Extrude Remove (cut is always perpendicular to the pierced wall), Hole, Move face (side faces; flange edges),
    Boolean subtract, Offset (where noted), Linear/Circular/Curve pattern and Mirror of faces (Pattern type = Face), Fillet
    (constant, circular cross section) and Chamfer (equal distance, chord based) on corners - suggested to use Corner break.
  - Blocked until Finish: Extrude Add / Intersect, other booleans, advanced fillet/chamfer, any custom feature op on SM
    geometry. Fillet/chamfer keep the original construction edge for later features (e.g. a flange on a filleted edge uses
    the original sharp edge).
- **Edge cases & errors:** `SHEET_METAL_CAN_ONLY_REMOVE`, `..._CAN_ONLY_SUBTRACT`, `..._CAN_ONLY_OFFSET`, `SHEET_METAL_CUT_JOINT`
  ("Cut should either not touch a joint or cut through it."), `SHEET_METAL_TOOL_DOES_NOT_CUT_THROUGH`, `SHEET_METAL_SUBTRACT_DESTROYS_SHEET`,
  `SHEET_METAL_MOVE_NOT_PLANAR`, `SHEET_METAL_CANNOT_MOVE_BEND_EDGE`, `SHEET_METAL_MOVE_FACE_NEXT_TO_CYLINDER_BEND`,
  `SHEET_METAL_HOLE_REBUILD_FAILED`, `SHEET_METAL_COUNTER_HOLE_UNSUPPORTED`, `SHEET_METAL_FILLET_NO_CONIC/NO_WIDTH/NO_VARIABLE`,
  `SHEET_METAL_CHAMFER_*`, `SHEET_METAL_FACE_PATTERN_*`, `SHEET_METAL_PATTERN_DISABLED_BOOLEANS`, `SHEET_METAL_ACTIVE_MODEL_CANNOT_OFFSET`.
- **OCCT mapping:** each allowed op implemented as a master-surface edit: project tool onto walls (cut through-thickness =
  2D cut of wall face by projected tool section, `BRepAlgoAPI_Section` + `BRepAlgoAPI_Cut`), hole = circular cut + hole
  attribute for counterbore/countersink re-application on the folded solid.
- **Priority:** P1

### SM-22 Bend calculation and flat-length math
- **Purpose:** Define how bend regions unfold (flat length).
- **Sources:** PartStudio/sheet_metal_model.htm (Bend calculation definitions); SM attributes.
- **Selection inputs:** n/a.
- **Parameters:** model-level bendCalculationType (K Factor / Bend allowance / Bend deduction), kFactor, kFactorRolled;
  per-bend overrides from table / Modify joint / Bend feature.
- **Behavior / semantics:** (standard formulas; exact Onshape implementation inferred)
  - K factor: neutral radius = R + K*T; bend allowance BA = theta * (R + K*T) (theta in rad).
  - Bend allowance mode: BA entered directly per bend = arc length of neutral line between tangent points.
  - Bend deduction mode: BD = 2*OSSB - BA where OSSB = (R+T)*tan(theta/2); flat = sum of flange lengths to apex - BD.
  - Rolled walls: developed length = theta * (Rinner + Krolled*T).
  - Bend radius in table read-only for bends whose radius comes from sketch arcs.
- **Edge cases & errors:** red cell for out-of-range values (SM-18).
- **OCCT mapping:** pure math in the flat builder.
- **Priority:** P1

### 3C.2 Variables, expressions, Variable Studio, query variables

### VAR-01 Variable feature - Assigned (assignVariable)
- **Purpose:** Feature in the feature list that assigns a named value usable as `#name` in any expression-capable parameter
  of features below it.
- **Sources:** PartStudio/variable.htm; FS: `assignVariable` (variable.fs). Feature name template `###name = #value`,
  tooltip `###name = #value #description`; no preview.
- **Selection inputs:** hidden `initEntities` (pre-selection used to seed Measured mode).
- **Parameters:**

| FS id | UI label | Type | Enum / default / bounds | Visible when / notes |
|---|---|---|---|---|
| mode | Mode | VariableMode | ASSIGNED "Assigned" (default), MEASURED "Measured", TABLE "From import" | horizontal enum (help calls TABLE "From table") |
| variableType | Variable type | VariableType | LENGTH "Length", ANGLE "Angle", NUMBER "Number", ANY "Any" | Assigned; UNCONFIGURABLE |
| name | Name | string | identifier, MaxLength 10000 | UNCONFIGURABLE, VARIABLE_NAME hint |
| lengthValue | Value | length | ZERO_DEFAULT_LENGTH_BOUNDS -500 .. 500 m, default 0 | Length |
| angleValue | Value | angle | -1e5 .. 1e5 deg, default 0 | Angle |
| numberValue | Value | real | -1e12 .. 1e12, default 0 | Number |
| anyValue | Value | any FS value | | Any |
| value | (hidden) | any | | always hidden mirror of typed value |
| description | Description | string | MaxLength 256, default "" | all modes |

- **Behavior / semantics:**
  - Executes `setVariable(context, name, value)`; later features read it via `#name` (UI) or `getVariable` (FS).
  - Value fields accept full expressions (VAR-05), including other variables defined above.
  - Any: any immutable FeatureScript value - number with any units, boolean, string, array, map, function
    (e.g. `function(len) { return len * 2 + 2.5 mm; }` then `#Adjust(20mm)`), arrays indexed by variables
    (`[0.25, 0.5, 1][#config]`, zero-based).
  - Editing the value regenerates all dependents. Renaming offers "Update all references" to rewrite `#old` -> `#new`
    in all referencing expressions.
  - The feature is listed in the Variable table (VAR-10) only if the name is a valid identifier.
- **Edge cases & errors:** `VARIABLE_NAME_INVALID` ("Variable name must be a letter followed by a string of letters or
  numbers."), `VARIABLE_NAME_TOO_LONG` (< 10000 chars), `VARIABLE_DESCRIPTION_TOO_LONG` (256),
  `VARIABLE_NAME_ALREADY_USED_IN_QUERY_VARIABLE`, `VARIABLE_CANNOT_EVALUATE`, `PARAMETER_OUT_OF_RANGE`.
- **OCCT mapping:** app data model (expression evaluator + context variable map). No geometry.
- **Priority:** P0

### VAR-02 Variable feature - Measured
- **Purpose:** Driven variable whose value is measured from geometry at its position in the feature list.
- **Sources:** PartStudio/variable.htm (Measured); View/mass_properties_tool.htm (variable icon beside any measurement creates
  a driven/measured variable); FS: `assignVariable` MEASURED branch.
- **Selection inputs:** Distance: "Entities to measure between" (faces, edges, vertices, non-sketch bodies, mate connectors;
  flat allowed; max 2). Length: "Entities" (edges or wire bodies). Diameter: "Entity" (circle, arc, cylinder, sphere; 1).
  Custom direction: direction entity, 1.
- **Parameters:**

| FS id | UI label | Type | Enum / default / bounds | Visible when / notes |
|---|---|---|---|---|
| measurementMode | Measurement | VariableMeasurementMode | DISTANCE "Distance", LENGTH "Length", DIAMETER "Diameter" | Measured |
| entityCouple | Entities to measure between | Query (2) | | Distance |
| minmax | Minimum/Maximum | VariableMinMaxSelection | MINIMUM (default), MAXIMUM | Distance |
| extendEntities | Extend entities | bool | false | Distance & Minimum; lines/arcs/cylinders treated as infinite |
| measureFromAxis | Use axis | bool | false | Distance & Minimum; measure to axes of cylinders/circles/arcs/revolved |
| distance, xOffset, yOffset, zOffset | Distance / Along X / Along Y / Along Z | length (read-only) | >= 0 | group "Axis measurements" (collapsed) |
| componentSelector | Value to record | AxisWithCustom | DISTANCE "Distance", X "Along X", Y "Along Y", Z "Along Z", CUSTOM "Along custom" | |
| customDirection / customOffset | Custom direction / Along custom | Query / length (read-only) | | CUSTOM |
| lengthEntities | Entities | Query | | Length: total length of edges |
| radius | Radius | bool | false | Diameter: record radius instead |
| diameterEntity | Entity | Query (1) | | Diameter |

- **Behavior / semantics:** value recomputed on every regeneration; result type Length; read-only in the Variable table.
  Distance components are absolute deltas along world X/Y/Z or a custom direction.
- **Edge cases & errors:** `VARIABLE_SELECT_FIRST_ENTITY`, `VARIABLE_SELECT_SECOND_ENTITY`, `VARIABLE_SELECT_CUSTOM_DIRECTION`,
  `VARIABLE_SELECT_ENTITIES_TO_MEASURE`, `VARIABLE_SELECT_ENTITY_TO_MEASURE`, `VARIABLE_NO_GEOMETRY_WITH_DIAMETER`,
  `VARIABLE_ONLY_TWO_ENTITIES_ALLOWED`, `VARIABLE_FLATTENED_ENTITIES_MUST_BE_SAME_BODY`,
  `VARIABLE_CANNOT_USE_MAXIMUM_WITH_INFINITE_ENTITIES`, `VARIABLE_NO_AXIS_ENTITIES`.
- **OCCT mapping:** `BRepExtrema_DistShapeShape` (min distance; extend entities by building infinite `Geom_Line`/`Geom_Plane`
  /untrimmed surfaces), max distance via custom sampling or bounding-vertex search, `GProp_GProps` + `BRepGProp::LinearProperties`
  for length, `BRepAdaptor_Curve/Surface` radius for diameter.
- **Priority:** P1

### VAR-03 Variable feature - From import (CSV table)
- **Purpose:** Variable populated from a subset of a CSV table (single value, array, or map).
- **Sources:** PartStudio/variable.htm (From table); VariableStudio/variable_studio.htm; FS: `assignVariable` TABLE branch.
- **Selection inputs:** CSV file (TableData) chosen from current document, other documents, or imported from disk.
- **Parameters:**

| FS id | UI label | Type | Enum / default / bounds | Visible when / notes |
|---|---|---|---|---|
| csv | CSV file | TableData | | TABLE |
| rowAccessType | Access type (Row) | AccessType | INDEX "By index", INDEX_RANGE "By index range", LABEL "By label", ALL "All values" | |
| rowIndex | Index (0-based) | integer | 0 .. 1e5, default 0 | By index |
| rowValue / rowRegexp | Value / Regular expression | string / bool | | By label |
| rowMinIndex / rowMaxIndex | Min / Max index (0-based) | integer | 0 .. 1e5 | By index range |
| rowResult | Result type (Row) | ResultType | SINGLE "Single value", ARRAY "Array", MAP "Map" | |
| rowLabelIndex | Label column index | integer | 0 .. 1e5 | By label or Map |
| columnAccessType, columnIndex, columnValue, columnRegexp, columnMinIndex, columnMaxIndex, columnResult, columnLabelIndex | (Column group, same labels) | | same | Column filters each selected row |
| useConversionFactor / conversionFactor | Conversion factor | bool / any | | multiplies numeric cells (e.g. `1 in`) |

- **Behavior / semantics:** Row options select rows and the outer result type; Column options select cells within each row
  and the inner type. Examples (help): all/array + all/array -> array of arrays; rows Map keyed by column 0 + columns index
  range 1..2 array; rows all/array + column index 0 single -> first column; row index 0 single + columns all/array -> first row.
  Type and Value are read-only in the Variable table; hover feature shows result.
- **Edge cases & errors:** `VALUE_WITH_UNITS_ERROR` ("Conversion factor must be a number or with units.").
- **OCCT mapping:** app data model (CSV parse, regex via Rust `regex`).
- **Priority:** P2

### VAR-04 Variable naming, `#` references and scoping rules
- **Purpose:** Rules for how variable names resolve across feature list, Variable table, Variable Studios and configurations.
- **Sources:** PartStudio/variable.htm; PartStudio/variable_table.htm; Home/numeric_fields.htm; error enum.
- **Selection inputs:** n/a.
- **Parameters:** n/a.
- **Behavior / semantics:**
  - Names: English letters, digits, underscore; case-sensitive; cannot start with a digit (help: "FeatureScript identifier:
    starting with a letter or underscore followed by alphanumeric characters or underscores, and without spaces").
  - Reference syntax `#name` inside any expression field (feature parameters, sketch dimensions, variable values,
    configuration values, suppression expressions, configured-property values where numeric).
  - Feature-order scoping: a Variable feature only affects features after it; autofill lists only variables existing at
    the current position; referencing a later/suppressed variable gives `VARIABLE_NOT_FOUND` ("Variable not found, is
    suppressed, or has not been set."). A later Variable feature with the same name re-assigns it for features below (inferred
    from setVariable semantics).
  - Variable table / Variable Studio variables are available to every feature (inferred: they are evaluated before the
    feature list). Precedence: a feature-list variable with the same name overrides the table variable (table shows override
    icon); with two tables defining one name, the last table in the panel wins.
  - Configuration variables (CFG-04) are referenced the same way (`#name`).
  - Query variables share the namespace check with value variables (`QUERY_VARIABLE_NAME_ALREADY_USED_IN_NON_QUERY_VARIABLE`).
  - Creating a variable on the fly from a field inserts the Variable feature immediately before the feature being edited.
- **Edge cases & errors:** as VAR-01; `PARAMETER_EXPRESSION_UNKNOWN_VARIABLE`.
- **OCCT mapping:** app data model: per-regeneration `VariableScope` = base map (configuration values + Variable Studio
  imports + table) then feature-list assignments applied in order; dependency index (variable -> referencing features) for
  `:variable` filter and highlighting.
- **Priority:** P0

### VAR-05 Expression language - grammar
- **Purpose:** Syntax of parameter expressions ("Parameter expressions are evaluated as FeatureScript").
- **Sources:** Home/numeric_fields.htm; PartStudio/variable.htm; PartStudio/feature_basics.htm (boolean expressions).
- **Selection inputs:** n/a.
- **Parameters:** n/a.
- **Behavior / semantics:**
  - Grammar (precedence low -> high; precedence/associativity follows FeatureScript, inferred where the help is silent):
    ```
    expr      := cond
    cond      := or ( "?" expr ":" expr )?
    or        := and ( "||" and )*
    and       := eq ( "&&" eq )*
    eq        := rel ( ("==" | "!=") rel )*
    rel       := add ( ("<" | "<=" | ">" | ">=") add )*
    add       := mul ( ("+" | "-") mul )*
    mul       := unary ( ("*" | "/" | "%") unary )*
    unary     := ("-" | "+" | "!") unary | pow
    pow       := postfix ( "^" unary )?            // right-assoc
    postfix   := primary ( "[" expr "]" | "(" args? ")" )*
    primary   := NUMBER UNIT? | "(" expr ")" UNIT? | call UNIT?
               | "#" IDENT | IDENT          // constant (pi), function name, unit name
               | "[" (expr ("," expr)*)? "]"  // array literal
               | STRING | "true" | "false"
               | FS function literal / map literal   // Any-type variable values only
    NUMBER    := digits ("." digits)? (exponent)?   // decimal comma accepted when user setting enabled
    UNIT      := length or angle unit keyword (VAR-06), juxtaposed with optional whitespace
    ```
  - Unit juxtaposition: `5mm`, `5 mm`, `(3*pi) in`, `sqrt(16)m` are valid; juxtaposition of two parenthesized groups is not
    multiplication (`(2*3)(1/3)` invalid).
  - Arrays are zero-based: `[3,5,6,7][2]` = 6. With comma decimal separators, array elements must be separated by ", " (space).
  - Ternary and logical operators: `#width>5?7:4`, `(#width > 3in && #width < 5in) ? 3 : 2`, `(#width > 3in || #alwaysReinforce) ? 3 : 2`.
  - Boolean parameters can be converted to expressions (VAR-14) and accept boolean results.
  - After acceptance the field shows the evaluated value in document units; on focus it shows the original expression text
    (expression text is persisted, evaluated value cached).
  - Sketch dimensions show an indicator: "expression without a variable" vs "expression with variable".
- **Edge cases & errors:** `PARAMETER_EXPRESSION_VALIDATION_SYNTAX_ERROR_0ARGS` ("Syntax error in expression."),
  `PARAMETER_SYNTAX_ERROR`, `EXPRESSION_FAILED_VALIDATION`, `PARAMETER_EXPRESSION_UNKNOWN_FUNCTION`,
  `PARAMETER_EXPRESSION_VALIDATION_RESERVED_ID_1ARG`, `..._CANNOT_APPLY_OP_TO_*`, `..._INCORRECT_TYPE`; field outlined red.
- **OCCT mapping:** app data model: hand-written Pratt/recursive-descent parser in Rust producing an AST; evaluator over a
  `Value` enum {Number(f64, UnitDims), Bool, String, Array, Map, Function}; FeatureScript function literals are out of scope
  (store as opaque string and reject, or support a tiny lambda subset) - see Gaps.
- **Priority:** P0

### VAR-06 Expression units and dimensional rules
- **Purpose:** Unit keywords, defaults and dimensional validity of expressions.
- **Sources:** Home/numeric_fields.htm; `units.fs` (STRING_TO_UNIT_MAP).
- **Selection inputs:** n/a.
- **Parameters:**

| Kind | Keywords accepted |
|---|---|
| Length | mm, millimeter, cm, centimeter, m, meter, in, inch, ft, foot, yd, yard (+ plurals, capitalized forms in FS map) |
| Angle | deg, degree, rad, radian (+ plurals) |
| Mass (FS map; Any variables) | kg, kilogram, g, gram, oz, ounce, lb, pound |

- **Behavior / semantics:**
  - Result must be unitless or a unit to the first power matching the field (length field -> length, angle field -> angle;
    unitless fields such as Rho or instance counts -> unitless).
  - Unitless result in a length/angle field takes the document default unit (`3+2` -> 5 default units).
  - Mixing is invalid: `3+2.5in`, `3in+2`, `3mm+2deg`, `3in*3in` (area), anything giving 1/unit, `sin(30)/deg`,
    `3[unit]+1[unit]/2[unit]`. Valid: `3in+2.5in`, `3mm+2.5in`, `(2*3)*(1/3)`, `sqrt(16)m`, `sqrt(2in * 3mm)`, `sqrt(4 in^2)`,
    `cos(30deg)`, `3mm+(2mm*3mm)/(6mm)`.
  - Any unit can be typed regardless of document units; display converts to default units.
  - Fractions supported (e.g. `1/2 in`; exact binding of the unit to the fraction inferred as 0.5 in).
- **Edge cases & errors:** `NO_UNIT` ("Value is missing required units."), `PARAMETER_EXPRESSION_VALIDATION_UNITS_MISMATCH_3ARGS`.
- **OCCT mapping:** app data model: unit dimension vector (length exponent, angle exponent, mass exponent) with SI storage
  (meters, radians, kg); conversion tables.
- **Priority:** P0

### VAR-07 Expression built-in functions and constants
- **Purpose:** Function library available in expressions.
- **Sources:** Home/numeric_fields.htm; `math.fs`; PartStudio/variable.htm (autofill includes FS functions).
- **Selection inputs:** n/a.
- **Parameters:**

| Group | Functions / constants |
|---|---|
| Operators | `+ - * / ^ %` (`5%2` = 1) |
| Math (help) | ceil, floor, round, exp, sqrt, abs, max, min, log, log10 |
| Trig (degrees for unitless args) | cos, sin, tan, acos, asin, atan, atan2, cosh, sinh, tanh, asinh, acosh, atanh |
| Constants | pi, PI, Pi |
| Additional FS math (math.fs, available because expressions are FS) | exp2, hypot, clamp, roundToPrecision, argMax, argMin, range |

- **Behavior / semantics:** `sin(30)` = `sin(30 deg)` = 0.5; inverse trig return angles (`atan(1)` = 45 deg; divide by
  `deg` for unitless); `atan2(4, 5)` = polar angle of (5,4); `ceil(5.667)` = 6, `floor(5.667)` = 5; min/max variadic over
  same-unit args; sqrt of unit^2 gives unit. Autofill without `#` also proposes FS functions with argument placeholders.
- **Edge cases & errors:** unknown function -> `PARAMETER_EXPRESSION_UNKNOWN_FUNCTION`.
- **OCCT mapping:** app data model (Rust f64 math).
- **Priority:** P0

### VAR-08 Variable autofill and inline variable creation
- **Purpose:** Typing assistance in numeric fields and sketch dimension boxes.
- **Sources:** PartStudio/variable.htm (Variable autofill feature, Creating variables on the fly).
- **Selection inputs:** n/a.
- **Parameters:** n/a.
- **Behavior / semantics:** typing `#`, `_` or any letter opens a dropdown of variables in scope (with computed values shown
  gray, not inserted) plus FS functions when `#` not used; type to filter, arrows + Enter to accept, wheel scroll, click;
  Esc or blur closes without inserting. A "New variable" entry opens a Create variable dialog with choice Feature (default:
  inserts a Variable feature before the current feature) or Configuration (creates a configuration variable input, CFG-04).
- **Edge cases & errors:** variables below the current feature are not listed.
- **OCCT mapping:** UI (GPUI popover) over VAR-04 scope query.
- **Priority:** P1

### VAR-09 Expression references from dimensions and parameters (dependency tracking)
- **Purpose:** Track which features use which variables; regenerate dependents on change.
- **Sources:** PartStudio/variable_table.htm (hover highlights, Filter feature list); PartStudio/part_studios.htm (`:variable` filter).
- **Selection inputs:** n/a.
- **Parameters:** n/a.
- **Behavior / semantics:** hovering a variable in the table highlights the defining feature and all using features; context
  menu "Filter feature list" shows the variable and affected features; `:variable name` filter (case-sensitive) shows
  features that use or change a variable.
- **Edge cases & errors:** none documented.
- **OCCT mapping:** app data model (reverse index built from parsed expression ASTs).
- **Priority:** P1

### VAR-10 Variable table (Part Studio)
- **Purpose:** Right-side panel to create, view, edit variables (and imported Variable Studio tables) in a Part Studio/Assembly.
- **Sources:** PartStudio/variable_table.htm.
- **Selection inputs:** n/a.
- **Parameters:** row columns: Name, Variable type (Length/Angle/Number/Any/From import), Value, Description.
- **Behavior / semantics:**
  - Type a name in "New variable name" + Enter/Tab -> type and value cells appear (default type selectable); edits save per cell.
  - Changing a name here updates all referencing features. Changing a type requires a new value.
  - Table-level ellipsis menu: Copy table (to clipboard, paste back from spreadsheet into cells/rows; header rows read-only),
    Switch to Variable Studio, Open linked document, Update linked document (Reference manager), Move up/down (table order),
    Edit reference (choose another Variable Studio or configuration), Remove reference.
  - Cell context menu: Filter feature list, Delete, Move up/down.
  - Bottom dropdown: Create Variable Studio / Insert Variable Studio (VAR-11/12).
  - Variables created by Variable features also appear (if valid identifiers); Measured / From-import types are read-only here.
  - Duplicate names in the same table rejected with a message; overrides across tables / feature list shown with override icon.
- **Edge cases & errors:** inserted tables from other documents are read-only; require a version.
- **OCCT mapping:** UI + app data model (table variables stored as an ordered list evaluated before the feature list).
- **Priority:** P0

### VAR-11 Variable Studio
- **Purpose:** Separate document tab holding a variable table usable by all Part Studios and Assemblies of the workspace, and
  insertable into other documents.
- **Sources:** VariableStudio/variable_studio.htm; FS: trimmed `assignVariable` used by Variable Studios (variable.fs @internal).
- **Selection inputs:** n/a.
- **Parameters:** rows: Name, Type (default Length; Length/Angle/Number/Any/From import), Value, Description; checkbox
  "Insert into all Part Studios and Assemblies" (bottom of tab).
- **Behavior / semantics:** later rows may reference earlier rows (`W1 = #L1 * ...`); reorder by drag or Move up/down;
  collapse/expand tables; when the auto-insert box is checked, variables are available document-wide (uncheck for drafts);
  From import opens the VAR-03 dialog; Variable Studios can be released (except configured ones).
- **Edge cases & errors:** cannot be configured while auto-inserted (CFG-13).
- **OCCT mapping:** app data model (element type VariableStudio = ordered variable list + optional configuration).
- **Priority:** P1

### VAR-12 Inserting / referencing Variable Studios
- **Purpose:** Bring a Variable Studio (whole table or selected variables) from this or another document into a Variable
  table / Variable Studio.
- **Sources:** VariableStudio/variable_studio.htm; PartStudio/variable_table.htm.
- **Selection inputs:** Insert Variable Studio dialog: tabs Current document / Other documents; pick a whole studio or individual variables.
- **Parameters:** for configured studios: configuration inputs + Generate, Set to last viewed / Reset to default.
- **Behavior / semantics:** other-document references pin a version (prompt to create one if none); header shows
  "Studio name + version"; referenced tables are read-only; update via Reference manager; edit reference can configure the
  reference with Part Studio configuration inputs (CFG-13).
- **Edge cases & errors:** `MISSING_EXT_REF`, `ELEMENT_REFERENCE_CYCLE_DETECTED`.
- **OCCT mapping:** app data model (versioned cross-document references).
- **Priority:** P1

### VAR-13 Query variable (queryVariable)
- **Purpose:** Named, parametric selection set reusable in downstream feature query fields.
- **Sources:** PartStudio/query_variable.htm; FS: `queryVariable` (queryVariable.fs), predicates `initialQueryPredicate`,
  `additionalQueryPredicate`. Name template `###name`.
- **Selection inputs:** depend on Selection type (table). Used downstream through a "Variable selection" dropdown in query
  fields or by picking the query-variable feature in the list.
- **Parameters:**

| FS id | UI label | Type | Enum / default / bounds | Visible when / notes |
|---|---|---|---|---|
| name | Name | string | MaxLength 10000 | UNCONFIGURABLE |
| description | Description | string | max 256 | |
| selectionType | Selection type | SelectionType | SELECTION "Selection", CREATED_BY "Created by", OWNED_BY "Owned by", PROTRUSION "Protrusion", POCKET "Pocket", HOLE "Hole", FILLETS "Fillets", BOUNDED_FACES "Bounded faces", LOOP_CHAIN_CONNECTED "Loop/chain connected", PARALLEL "Parallel", TANGENT_CONNECTED "Tangent connected", MATCHING "Matching", ALL_SOLID_BODIES "All solid bodies", EDGE_CONVEXITY "Edge convexity" | help also documents "Adjacent" (newer than FS mirror) |
| seedType | Entity type | SeedType | FACE "Face", EDGE "Edge" | Tangent connected, Matching |
| selectionQuery | Selections | Query (any, mesh & flat allowed) | | Selection |
| createdByFeatures | Created by features | FeatureList | | Created by |
| seedBodies | Entities | Query (bodies) | | Owned by, Edge convexity |
| seedFaces | Faces | Query (faces) | | Protrusion, Pocket, Hole, Fillets, Bounded faces, Tangent/Matching+Face |
| seedEdgesOrFaces | Edges or faces | Query | | Loop/chain connected |
| seedEdges | Edges | Query | | Parallel, Tangent/Matching+Edge |
| entityType | Entity type | EntityType | VERTEX, EDGE, FACE, BODY | Created by, Owned by |
| filletCompareType | Fillet compare type | FilletCompare | EQUAL "Equal", LESS_EQUAL "Less or equal", GREATER_EQUAL "Greater or equal" | Fillets |
| boundedFacesBounds | Bounds | Query (edges/faces) | | Bounded faces |
| edgeConvexityType | Edge convexity type | EdgeConvexityType | CONVEX, CONCAVE, SMOOTH, VARIABLE | Edge convexity |
| filterConstruction | Filter construction entities | bool | true | Created by |
| filterByBodyType / createdByBodyType | Filter by body type / Body type | bool / BodyTypeOptions | false; SOLID "Part", SHEET "Surface", WIRE "Curve", POINT "Point", MATE_CONNECTOR "Mate connector", COMPOSITE "Composite part" | Created by |
| addAdditionalQueries | Add additional queries | bool | false | |
| additionalQueries[] | Additional queries: booleanOperation (UNION "Union", SUBTRACTION "Subtract", INTERSECTION "Intersect") + same selection fields prefixed `addQ` | array | label "#booleanOperation of #addQlowercaseSelectionType" | |
| evaluateOnUse | Evaluate on use | bool | false | re-evaluate at each consumer's position |
| showSelection | Show selection | bool | true | highlight |

- **Behavior / semantics:**
  - Semantics of types: Protrusion = faces connected to seed by convex edges; Pocket = by concave edges; Hole = faces of the
    same round hole; Fillets = constant-radius fillet faces (compare radius); Bounded faces = faces between seed and bounding
    edges/faces (if not closed, all adjacent selected); Loop/chain = edge loops of a face / chains from sketch edge; Parallel
    = edges parallel to seed; Tangent connected = tangent chain; Matching = same-size faces/edges; All solid bodies; Edge
    convexity = edges of bodies with given convexity (variable = both).
  - Additional queries combine left-to-right with union / subtract / intersect.
  - Without Evaluate on use, the set is frozen at the variable's position (e.g. All solid bodies = bodies existing then);
    with it, re-evaluated at the position where consumed.
  - Mixed entity types allowed; consumers take valid entries (first valid one for single-pick fields, e.g. sketch plane).
    Empty query variables allowed per help, but FS has `QUERY_VARIABLE_EMPTY_SELECTION`.
  - Editing a query variable updates downstream features; Final button previews downstream.
- **Edge cases & errors:** `QUERY_VARIABLE_EMPTY_NAME`, `..._NAME_ALREADY_USED_IN_NON_QUERY_VARIABLE`, `..._EMPTY_SELECTION`,
  `..._NAME_CANNOT_START_WITH_POUND`, `..._NAME_CANNOT_CONTAIN_QUOTE`.
- **OCCT mapping:** query engine over topology: adjacency via `TopExp::MapShapesAndAncestors`, convexity via
  `BRepLProp_SLProps` normals + `BRep_Tool::Continuity` / dihedral angle sign, parallel/tangent via `BRepAdaptor_Curve`
  derivatives, hole/fillet detection via `GeomAdaptor_Surface::GetType()` cylinder/torus + radius; persistent query
  representation per PSM-02.
- **Priority:** P1

### VAR-14 Boolean parameter as expression (Convert to expression)
- **Purpose:** Drive checkbox/boolean parameters by expressions.
- **Sources:** PartStudio/feature_basics.htm (Using variables in Boolean parameters); PartStudio/managing_configurations.htm.
- **Selection inputs:** right-click a boolean field (e.g. Extrude "Opposite direction") -> Convert to expression.
- **Parameters:** expression text replacing the checkbox.
- **Behavior / semantics:** expression evaluated to true/false (e.g. `#Length>3`); right-click text -> Convert to checkbox
  restores a checkbox. Same mechanism powers dynamic suppression (PSM-07) and checkbox configuration inputs converted to
  expressions (CFG-06).
- **Edge cases & errors:** non-boolean result -> `PARAMETER_EXPRESSION_VALIDATION_INCORRECT_TYPE` (inferred).
- **OCCT mapping:** app data model (parameter value = Literal | Expression).
- **Priority:** P1

### 3C.3 Configurations

### CFG-01 Configuration model and terminology
- **Purpose:** Variants (part families) of a Part Studio / Assembly / Variable Studio driven by independent inputs instead of
  one large design table.
- **Sources:** PartStudio/configurations.htm; PartStudio/part_studio_and_assembly_configurations.htm.
- **Selection inputs:** n/a.
- **Parameters:** n/a. Data model terms: Configuration panel (one per element, right side) containing *Inputs* (each a
  collapsible table of type List, Checkbox, or Configuration variable); *Options* (rows of a List input; checked/unchecked
  for Checkbox; value range for variables); *Configured features* (column group headers = existing feature or variable,
  name not editable); *Parameters* (columns = specific feature parameters); *Option values* (cells).
- **Behavior / semantics:**
  - A configuration = one choice per input; every permutation of inputs is a configuration.
  - Selected configuration is per user/session ("each sees their own selected configuration"; followers in Follow mode see
    the leader's).
  - Configurations selector appears above the Features list only once at least one input exists; panel "Configure Part
    Studio" button (or dropdown: List / Checkbox / Configuration variable) creates the first input.
  - What can be configured (Part Studio): any feature parameter value or selection, feature suppression, sketch dimensions,
    sketch text, custom-feature parameters, part properties (incl. part number, custom properties), material, part and face
    appearance (CFG-08). Assemblies: mates, instances, patterns (not mate connectors).
- **Edge cases & errors:** `CONFIGURATION_HAS_BAD_PARAMETERS`, `CONFIGURATION_HAS_BAD_CURRENT_CONFIGURATION`,
  `INVALID_CONFIGURATION_SPECIFIED`, `ALL_CONFIGURATION_PARAMETERS_NOT_SPECIFIED`, `FEATURE_CONFIGURED_PARAMETER_NO_VALUES`,
  `FEATURE_CONFIGURED_PARAMETER_INCONSISTENT_TYPES`.
- **OCCT mapping:** app data model. (design suggestion) `ConfigurationDef { inputs: Vec<Input> }`,
  `Input { id(fsId), name, kind, options/default/min/max, columns: Vec<ConfiguredParam{featureId, paramId}>,
  values: Table, visibility }`; a configuration key = ordered map inputId -> value; regeneration substitutes configured
  parameter values before evaluating features; cache regenerated Part Studio results per (element version, configuration key)
  with LRU eviction.
- **Priority:** P1

### CFG-02 List input
- **Purpose:** Named rows (e.g. Short / Medium / Tall), each setting values of chosen parameters.
- **Sources:** PartStudio/part_studio_and_assembly_configurations.htm.
- **Selection inputs:** Configure features mode (CFG-05).
- **Parameters:** input name; rows (option names, first is "Default"); columns (configured parameters); default row.
- **Behavior / semantics:** blue bar marks the active row; switch via selector or row context menu "Switch to"; Set as
  default; Duplicate row; Move up/down / drag; Rename; Delete row/column. Dimensions using expressions populate cells with
  the expression. Multiple List inputs allowed. List inputs can carry option-level visibility rules (CFG-09).
- **Edge cases & errors:** values must match parameter type (`FEATURE_PARAMETER_TYPE_MISMATCH`).
- **OCCT mapping:** app data model + GPUI table.
- **Priority:** P1

### CFG-03 Checkbox input
- **Purpose:** Boolean input (two rows: unchecked, checked) toggling parameters, most often suppression.
- **Sources:** PartStudio/part_studio_and_assembly_configurations.htm; PartStudio/managing_configurations.htm.
- **Selection inputs:** Configure features mode.
- **Parameters:** Name; Default (checked/unchecked).
- **Behavior / semantics:** one column per configured parameter (e.g. feature "Unsuppressed" state, end type, selections);
  can be converted to an expression (Convert to expression / Convert to checkbox), expressions may reference variables and
  other inputs (`#alwaysReinforce` in expressions).
- **Edge cases & errors:** none documented.
- **OCCT mapping:** app data model.
- **Priority:** P1

### CFG-04 Configuration variable input
- **Purpose:** Continuous-valued input usable as `#name` in any expression.
- **Sources:** PartStudio/part_studio_and_assembly_configurations.htm; VariableStudio/variable_studio_configurations.htm.
- **Selection inputs:** n/a.
- **Parameters:** Name (becomes variable `#name`); Variable type: Length, Angle, Integer, Real, Text; Default, Minimum,
  Maximum for all types except Text (Text = any string usable by custom FeatureScript).
- **Behavior / semantics:** apply by typing `#name` in a field or right-click a field -> "Set to #name"; panel lists the
  input's usages; selector shows a numeric/text field constrained to [min, max]. Can also be created from the autofill "New
  variable -> Configuration" option. Form Part Studios require a Length configuration variable named `thickness` (SM-08).
- **Edge cases & errors:** out-of-range value rejected (inferred: clamp/red field).
- **OCCT mapping:** app data model; injected into VAR-04 base scope.
- **Priority:** P1

### CFG-05 Configure features mode (configured feature parameters)
- **Purpose:** Add feature parameters as columns of an input.
- **Sources:** PartStudio/part_studio_and_assembly_configurations.htm; PartStudio/managing_configurations.htm.
- **Selection inputs:** "+ Configure features" enters a mode (yellow banner, Done to exit); click a feature, then hover
  parameters (configurable ones highlight yellow) and click to add.
- **Parameters:** n/a.
- **Behavior / semantics:**
  - Column header = feature name (top) + parameter label (sub). Configured parameters show a dashed yellow outline in the
    dialog; a parameter configured by one input is unavailable to others.
  - Value cells: double-click to edit numbers/expressions; for selections, double-click opens the feature dialog with the
    parameter highlighted blue to reselect per option.
  - Tolerance options can be configured per option value (precision, type).
  - Parameters annotated `UIHint.UNCONFIGURABLE` (e.g. variable Name, variable type) cannot be configured.
- **Edge cases & errors:** `CONFIG_NO_PARAMETERS_FOUND`, `CONFIG_TOO_MANY_PARAMETERS`, `CONFIG_INCORRECT_PARAMETER_TYPE`.
- **OCCT mapping:** app data model + dialog overlay state.
- **Priority:** P1

### CFG-06 Configured suppression and dynamic suppression expressions
- **Purpose:** Suppress features per configuration, or by logical expression.
- **Sources:** PartStudio/managing_configurations.htm (Configuring dynamic suppression, Converting a checkbox to an
  expression); PartStudio/features_and_parts_lists.htm (Dynamic suppression).
- **Selection inputs:** Feature list right-click -> "Configure suppression" / "Configure suppression with..." (adds column
  to an input).
- **Parameters:** suppression state per option or suppression expression per option.
- **Behavior / semantics:** suppression column cells are checkboxes (checked = suppressed); dynamic suppression expressions
  (PSM-07) can be configured from the panel; removing dynamic suppression from the feature converts the configured
  expression back to a checkbox.
- **Edge cases & errors:** `INVALID_SUPPRESSION_EXPRESSION`.
- **OCCT mapping:** app data model.
- **Priority:** P1

### CFG-07 Configuring sketch dimensions and sketch text
- **Purpose:** Make sketch dimension values / text per configuration.
- **Sources:** PartStudio/part_studio_and_assembly_configurations.htm; PartStudio/tag.htm ("right-click on each dimension ...
  select Configure dimension"); PartStudio/managing_configurations.htm (columns renameable for sketch features only); glossary
  (sketch text configurable).
- **Selection inputs:** in Configure features mode click the sketch then the dimension; or right-click a dimension in sketch
  -> Configure dimension.
- **Parameters:** n/a.
- **Behavior / semantics:** each configured dimension is a column; sketch columns can be renamed (other feature columns
  cannot). Using a configuration variable in a dimension expression is the alternative (column appears with Default row).
- **Edge cases & errors:** none documented.
- **OCCT mapping:** app data model (sketch dimension values are feature parameters of the sketch feature).
- **Priority:** P1

### CFG-08 Configured properties
- **Purpose:** Per-configuration part / appearance / custom properties (part number, name, description, material, color ...).
- **Sources:** PartStudio/configured_properties.htm; PartStudio/customizing_appearances.htm (Configuring face colors).
- **Selection inputs:** "Configured properties" button at panel top -> "Properties for" dropdown (parts, appearances; items with
  configured props show a checkmark) -> Add property.
- **Parameters:** property columns x rows = cartesian combination of included inputs.
- **Behavior / semantics:**
  - Cells edited by double-click; dialog-type properties (appearance, material) open their dialog. "Generate next part
    number" in Part number column if numbering scheme enabled.
  - Read-only properties lock table structure (cannot delete table / add properties / remove that column).
  - *Exclude from properties* (input menu): that input is ignored for property tables (treated as default value for releases,
    revision references and BOM identity; parts differing only in excluded inputs are the same part); still affects geometry,
    computed properties and Mass Properties. Excluded inputs may only drive visibility of other excluded inputs below them.
  - Reset all (properties dialog) does not affect configured properties.
- **Edge cases & errors:** none documented.
- **OCCT mapping:** app data model; colors to XCAF on export (`XCAFDoc_ColorTool`).
- **Priority:** P1

### CFG-09 Visibility conditions
- **Purpose:** Conditional logic controlling when inputs (and List options) are shown.
- **Sources:** PartStudio/visibility_conditions.htm.
- **Selection inputs:** input menu -> Visibility conditions (dialog with live preview panel; Inputs dropdown incl. All inputs;
  Show hidden inputs checkbox).
- **Parameters:** input-level conditions: `<controlling input> is <options...>` or `is between <a> and <b>` (AND-combined,
  additive); option rules (List inputs only): Show `<options | range>` When `Always | <input> is/is between ...` with AND-ed conditions.
- **Behavior / semantics:** only inputs *above* can control inputs below; hidden inputs take their default value; an input
  with no visible options is hidden; if a default is hidden, first visible option is used (also for Reset to default);
  conditions apply in all Insert dialogs and propagate to referencing documents; deleting/moving referenced inputs/options
  auto-repairs (removes) conditions with a workspace warning; summaries shown when collapsed.
- **Edge cases & errors:** `CONFIGURATION_PARAMETER_HAS_BAD_VISIBILITY_CONDITION`.
- **OCCT mapping:** app data model (rule evaluation over ordered inputs).
- **Priority:** P2

### CFG-10 Managing configurations (rename, filter, copy/paste, menus, FeatureScript IDs)
- **Purpose:** Editing/maintenance tools for inputs.
- **Sources:** PartStudio/managing_configurations.htm.
- **Selection inputs:** n/a.
- **Parameters:** n/a.
- **Behavior / semantics:**
  - Rename input / sketch column inline (hover I-cursor) or via menu.
  - Filter inputs: `:name` (default), `:part "Part 1"`, combinable filters, quoted exact match; toggle icons refine.
  - Copy input / Copy all inputs -> Paste inputs (between Part Studios, Assemblies, Variable Studios; same or other document;
    clipboard lasts until logout); names, visibility conditions, FS IDs and variable definitions copied; no live link; inputs
    with duplicate FS IDs are not pasted; invalid pasted visibility conditions auto-removed.
  - Copy table -> spreadsheet -> paste values back starting at top-left cell (rows auto-created, extra columns ignored, default
    units added).
  - Panel menu: Copy all inputs, Collapse/Expand all. Input menu: Copy table, Copy input, Move up/down (also drag), Rename,
    Edit FeatureScript IDs (change internal ids; "Break references and change IDs"), Exclude from properties, Visibility
    conditions, Input propagation, Delete (immediate, no warning). Row menu: Switch to, Set as default, Duplicate row, Move
    up/down, Rename, Delete row. Column menu: Delete column; columns resizable.
  - Inputs in Part Studios derived from an Assembly mirror are view-only.
- **Edge cases & errors:** none documented.
- **OCCT mapping:** app data model + clipboard (serialize inputs as JSON + TSV for tables).
- **Priority:** P2

### CFG-11 Input propagation
- **Purpose:** Bulk-drive configuration inputs of references (inserted instances, Variable Studio references, derived) from
  the current element's same-named inputs.
- **Sources:** PartStudio/input_propagation.htm.
- **Selection inputs:** Configuration panel menu -> Input propagation (dialog lists references sharing parameters).
- **Parameters:** checkbox per input to propagate.
- **Behavior / semantics:** propagated inputs are marked next to the reference in the panel; available only when matching
  inputs exist (Copy input helps create them).
- **Edge cases & errors:** none documented.
- **OCCT mapping:** app data model (reference config value = expression bound to parent input).
- **Priority:** P2

### CFG-12 Configuration selection, insertion and change (assemblies, derived, drawings, releases)
- **Purpose:** Choose configurations when consuming a configured element.
- **Sources:** PartStudio/part_studio_and_assembly_configurations.htm; PartStudio/derived.htm; PartStudio/part_studios.htm
  (Release all configurations).
- **Selection inputs:** Insert dialogs (assembly insert, Derived `partStudio` with `UIHint.SHOW_INLINE_CONFIG_INPUTS`,
  drawing insert, release candidate, Form Part Studio).
- **Parameters:** per-input pickers; "Set to last viewed configuration" / "Reset to default"; Generate (refresh list of parts
  for chosen configuration).
- **Behavior / semantics:** instances store their configuration; Change configuration from instance context menu; patterns /
  replicates / mirrors share seed configuration; "Select instances with same part and same configuration"; tooltip shows
  instance configuration. Derived: one Derived feature per source + configuration (`DERIVED_NO_SAME_SOURCE`). Release all
  configurations: up to 100 configurations, only when the element has a single List input (no checkbox/variable inputs).
  Per-user configuration selection inside the element does not affect consumers.
- **Edge cases & errors:** `INVALID_CONFIGURATION_SPECIFIED`, `REFERENCE_REPAIR_MISSING_CONFIGURATION`.
- **OCCT mapping:** app data model (reference = element id + version + configuration key).
- **Priority:** P1

### CFG-13 Variable Studio configurations
- **Purpose:** Configure Variable Studio values/descriptions; choose configuration at insertion.
- **Sources:** VariableStudio/variable_studio_configurations.htm.
- **Selection inputs:** right-click a variable value or description -> Configure / Configure with / Set to... / Unconfigure.
- **Parameters:** List, Checkbox, Configuration variable (Length, Angle, Integer, Real, Text) inputs as in Part Studios.
- **Behavior / semantics:** must uncheck "Insert into all Part Studios and Assemblies" before configuring; configured cells
  highlighted orange; insertion requires picking configurations (Generate); in a Part Studio, Edit reference -> right-click
  configuration -> Configure (with) adds the Variable Studio configuration as a column in the Part Studio's configuration panel.
  Configured Variable Studios cannot be released.
- **Edge cases & errors:** none documented.
- **OCCT mapping:** app data model.
- **Priority:** P2

### CFG-14 Configuration limits and unconfigurable state
- **Purpose:** Record documented limits and what is not configurable.
- **Sources:** pages above.
- **Selection inputs:** n/a.
- **Parameters:** n/a.
- **Behavior / semantics:**
  - Release all configurations limited to 100 configurations and single List input.
  - Parameters with `UNCONFIGURABLE` UI hint cannot be configured; array-parameter fields not configurable (tag.fs comment
    "array parameter fields are not configurable (for now)").
  - Hide/show state of parts is not listed as configurable in the help (docs silent - treat as not configurable); part/face
    appearance *including transparency set in the appearance dialog* is configurable via configured properties (inferred
    that opacity follows appearance).
  - Mobile can only view/select configurations (out of scope).
  - No documented cap on number of inputs/rows (docs silent).
- **Edge cases & errors:** n/a.
- **OCCT mapping:** app data model.
- **Priority:** P2

### 3C.4 Part Studio management and part data

### PSM-01 Part Studio and regeneration model
- **Purpose:** Define what a Part Studio is and how its geometry is computed.
- **Sources:** PartStudio/part_studios.htm; PartStudio/features_and_parts_lists.htm; FeatureScript design (stdlib
  `defineFeature`, `Context`, `Id`, `qCreatedBy`).
- **Selection inputs:** n/a.
- **Parameters:** n/a.
- **Behavior / semantics:**
  - A Part Studio is a design environment (tab) holding one ordered feature list that produces many bodies: parts (solids),
    surfaces (sheets), curves (wires), points, sketches, mate connectors, composite parts, meshes. Default geometry: Origin,
    Top, Front, Right planes (hideable, not deletable). Best practice: only parts sharing references in one studio.
  - Regeneration: start from an empty context (plus default geometry) and execute each non-suppressed feature above the
    rollback bar in order. Each feature is a pure function `(context, id, definition) -> context'` where `definition` = its
    parameter values after configuration substitution and expression evaluation (variables in scope at that point).
  - Every feature gets a stable feature id; operations within a feature get sub-ids (`id + "extrude1"`). Entities created by
    an operation are addressable by `qCreatedBy(id)` and by persistent names derived from the creating op.
  - Selections stored in feature parameters are *queries* (persistent references) resolved against the current context at
    that feature's position, not raw geometry; resolution failure gives a missing reference (PSM-13).
  - Results cached per feature (inferred) so editing feature k re-runs from k onward; edits above the rollback bar regenerate
    downstream features (Final button shows end state while editing).
  - Features may fail (error), partially succeed (warning/info), or be fully defined/underdefined (sketch). A failed feature
    contributes nothing; later features still run (and may fail due to missing references).
  - Part Studio element can be shown as FeatureScript ("Show code").
- **Edge cases & errors:** `REGEN_ERROR`, `SYS_ERROR_REGEN` ("Current part studio cannot regenerate."), `SYS_FEATURE_EXCEPTION`,
  `FEATURE_INVALID_ROLLBACK_INDEX`, `FEATURE_CHANGE_BREAKS_MODEL`, `FEATURE_CONCURRENCY_ERROR`.
- **OCCT mapping:** (design suggestion) `PartStudio { features: Vec<FeatureNode>, rollback_index, config_def }`;
  `RegenContext { bodies: SlotMap<BodyId, Body{shape: TopoDS_Shape, kind, name, props}>, naming: NamingGraph, variables,
  attributes }`; snapshot per feature (structural sharing; OCCT shapes are ref-counted handles so cloning a `TopoDS_Shape`
  is cheap); parallelism not needed initially. Feature implementations call OCCT ops (`BRepAlgoAPI_*`, `BRepPrimAPI_*` ...).
- **Priority:** P0

### PSM-02 Persistent references / topological naming (design suggestion)
- **Purpose:** Keep feature selections attached to "the same" faces/edges/vertices across regenerations and upstream edits.
- **Sources:** Onshape behavior (queries, missing references, Repair/Replace reference - Document/repairing.htm). Onshape's
  internal naming is not documented; this item is a noshape design.
- **Selection inputs:** n/a.
- **Parameters:** n/a.
- **Behavior / semantics (required behavior):** a selection made at feature k must resolve to the corresponding entity after
  dimension changes, upstream feature edits, reordering that keeps ancestry, and configuration changes; if it cannot, report a
  missing reference (red selection) rather than silently picking another entity; split entities resolve to all pieces where
  the consumer accepts multiple, otherwise first valid.
- **Edge cases & errors:** `MISSING_PARAMETER_REFERENCE` ("Reference is missing."), `INSTANCE_CONSUMED_BY_CLOSED_COMPOSITE`.
- **OCCT mapping:** (design suggestion)
  - Option A - OCAF: `TNaming_Builder` (Generated/Modify/Delete) per operation, `TNaming_Selector::Select/Solve` for stored
    selections, `TNaming_NamedShape` history in a `TDocStd_Document`. Pros: built-in; cons: known fragility on complex
    booleans/fillets, heavy document coupling.
  - Option B (recommended) - custom element map: every op records history via `BRepBuilderAPI_MakeShape::Generated/Modified/
    IsDeleted` (all `BRepAlgoAPI_*`, `BRepFilletAPI_*`, `BRepOffsetAPI_*`, `BRepPrimAPI_*` expose it; `BRepTools_History` /
    `BRepAlgoAPI_BuilderAlgo::History()` merges it). Assign each output sub-shape a name = hash(featureId, opTag,
    role (e.g. "cap_start", "side", "lateral of edge X"), sorted source names, occurrence index). Sketch-derived faces named from
    sketch entity ids (stable). Store selections as name + geometric fingerprint (type, centroid, normal/axis, area/length) used
    as a tie-break when a name maps to several candidates or none (heuristic recovery flagged as warning).
  - Resolution happens against the context snapshot at the consuming feature; maintain `TopTools_IndexedDataMapOfShapeListOfShape`
    maps from `TopExp` for adjacency-based names (edge = intersection of two named faces) - this "face-pair" naming is robust
    for edges and vertices.
- **Priority:** P0

### PSM-03 Features list (UI structure)
- **Purpose:** Left panel listing features and parts.
- **Sources:** PartStudio/features_and_parts_lists.htm; PartStudio/part_studios.htm.
- **Selection inputs:** clicking a feature/sketch name supplies it to an active selection field; selecting a part/face
  cross-highlights features that created/modified it (not for composite parts).
- **Parameters:** n/a.
- **Behavior / semantics:** top to bottom: configuration selectors (if any), feature count, search/filter box, default
  geometry, features (each with its tool icon and editable name), rollback bar, separator, Parts list (PSM-15). Separator
  draggable; both areas scroll independently; panel collapsible via Features and parts icon. Linked features show link icon
  (blue when newer version exists). Features with comments show comment icon. Child features shown nested with expand arrows
  (e.g. under Derived). Users with view+copy can open dialogs read-only.
- **Edge cases & errors:** n/a.
- **OCCT mapping:** UI (GPUI tree/list virtualized).
- **Priority:** P0

### PSM-04 Feature dialog conventions
- **Purpose:** Common dialog behavior for creating/editing features.
- **Sources:** PartStudio/feature_basics.htm; PartStudio/feature_tools.htm.
- **Selection inputs:** first query field auto-focused (blue); Tab cycles fields; arrow keys open dropdowns; space toggles
  checkboxes; Enter accepts; Esc cancels.
- **Parameters:** n/a.
- **Behavior / semantics:** title red when incomplete or erroring (cannot commit), black when valid; selection fields list
  picks with x to remove, right-click "Clear all"; missing references shown red with "Clear missing references"; array
  parameters (e.g. loft profiles) reorderable by drag handle; Preview slider (opacity 0-100% before/after); editing a feature
  rolls the model back to that feature, "Final" shows end result (absent when editing the last feature); in-dialog undo/redo;
  after closing, undo/redo reverts/reapplies the whole edit session; help (?) link; feature tool search (alt/opt+c);
  shortcut toolbar (S key). Read-only users see disabled fields.
- **Edge cases & errors:** n/a.
- **OCCT mapping:** UI; preview = regenerate feature into a temporary context.
- **Priority:** P0

### PSM-05 Rollback bar
- **Purpose:** View/edit the model at an earlier point in history.
- **Sources:** PartStudio/features_and_parts_lists.htm; glossary (Rollback, Rollback bar); keyboard shortcuts.
- **Selection inputs:** drag bar; feature context menu "Roll to here"; bar context menu "Roll to end"; select bar + Up/Down arrows.
- **Parameters:** rollback index (persisted in the workspace, shared with collaborators).
- **Behavior / semantics:** features below the bar are temporarily not regenerated (treated as suppressed); new features are
  inserted at the bar position; editing a feature does not move the bar. View-only collaborators who move the bar stop
  receiving live rollback updates until reload.
- **Edge cases & errors:** `FEATURE_INVALID_ROLLBACK_INDEX`.
- **OCCT mapping:** app data model (regenerate prefix only).
- **Priority:** P0

### PSM-06 Suppress / unsuppress features and folders
- **Purpose:** Exclude features from regeneration without deleting them.
- **Sources:** PartStudio/features_and_parts_lists.htm; glossary (Suppress).
- **Selection inputs:** feature or folder context menu Suppress / Unsuppress (multi-select).
- **Parameters:** per-feature `suppressed` flag (configurable, CFG-06), or dynamic expression (PSM-07).
- **Behavior / semantics:** suppressed feature shown with strikethrough; skipped in regeneration; dependents lose references
  and may fail; suppressing a folder suppresses all contained items ("all features associated with the suppressed item become
  inactive"); `:suppressed` filter. Variables defined by suppressed Variable features are not set (`VARIABLE_NOT_FOUND`).
- **Edge cases & errors:** n/a.
- **OCCT mapping:** app data model.
- **Priority:** P0

### PSM-07 Dynamic suppression
- **Purpose:** Suppress a feature when a logical expression is true.
- **Sources:** PartStudio/features_and_parts_lists.htm (Dynamic suppression).
- **Selection inputs:** feature context menu Dynamic suppression > Add expression / Edit dynamic suppression > Edit expression
  | Remove expression; expression icon in list opens editor.
- **Parameters:** boolean expression (VAR-05), e.g. `#depth < 0.5 in`.
- **Behavior / semantics:** evaluated during regeneration with the variables in scope at the feature; when true, feature
  suppressed (strikethrough) and expression icon shown; configurable (CFG-06).
- **Edge cases & errors:** `INVALID_SUPPRESSION_EXPRESSION`.
- **OCCT mapping:** app data model.
- **Priority:** P1

### PSM-08 Reorder features and dependency rules
- **Purpose:** Change feature order by drag.
- **Sources:** PartStudio/features_and_parts_lists.htm; PartStudio/variable.htm (feature order matters for variables).
- **Selection inputs:** drag one or more (non-contiguous) selected features; folders can be reordered and items moved in/out.
- **Parameters:** n/a.
- **Behavior / semantics:** reorder is parametric (model regenerated in new order). A feature cannot be moved above a feature
  it references, nor a parent below its children (inferred; Onshape reports `FEATURE_CHANGE_BREAKS_MODEL` "The change would
  result in the feature list becoming invalid" for invalid structural changes). Moving a Variable below its consumers makes
  them fail (`VARIABLE_NOT_FOUND`). "Show dependencies" (PSM-11) displays parents/children to guide reordering.
- **Edge cases & errors:** `FEATURE_CHANGE_BREAKS_MODEL`.
- **OCCT mapping:** app data model: dependency graph from parameter queries (feature ids referenced via `qCreatedBy` /
  naming graph ancestry) and variable references; validate drop target.
- **Priority:** P0

### PSM-09 Rename features
- **Purpose:** Descriptive feature names.
- **Sources:** PartStudio/features_and_parts_lists.htm.
- **Selection inputs:** context menu Rename (inline edit) or dialog title.
- **Parameters:** name string; some features have name templates (Variable `###name = #value`, Query variable `###name`).
- **Behavior / semantics:** icon still shows feature type; renaming does not affect references (references are by id).
  Default names "<Type> <n>" (e.g. Extrude 1) (inferred from examples).
- **Edge cases & errors:** n/a.
- **OCCT mapping:** app data model.
- **Priority:** P0

### PSM-10 Feature folders
- **Purpose:** Group features/sketches/planes.
- **Sources:** PartStudio/features_and_parts_lists.htm.
- **Selection inputs:** select items -> right-click "Add selection to folder..." or folder icon at list top; name dialog.
- **Parameters:** folder name; nesting allowed.
- **Behavior / semantics:** expand/collapse; Unpack folder restores items; Suppress folder; hide/isolate folder; `:folder` filter.
  Folder contents must stay contiguous in feature order (inferred).
- **Edge cases & errors:** n/a.
- **OCCT mapping:** app data model (folder = start/end markers or parent node in list).
- **Priority:** P1

### PSM-11 Feature list context menu and commands
- **Purpose:** Per-feature commands.
- **Sources:** PartStudio/features_and_parts_lists.htm; PartStudio/feature_basics.htm; PartStudio/customizing_appearances.htm;
  Document/repairing.htm; keyboard shortcuts.
- **Selection inputs:** right-click feature(s).
- **Parameters:** n/a.
- **Behavior / semantics:** Edit (or double-click); Rename; Suppress/Unsuppress; Dynamic suppression; Configure suppression
  (with...); Hide/Show (or eye icon) - hides the feature's sketch/construction output; Show dependencies (dialog: parents above
  up arrow, children below down arrow, hover tooltips); Expand/Collapse selected/all; Add selection to folder / Unpack;
  Roll to here; Add comment (email notifications option); Add appearance to feature / Edit feature appearance; Edit healthy
  moment (repair, owned by Document section); Update linked document (linked/custom features); Delete (dependents keep
  missing references; undoable); copy/paste features (Ctrl/Cmd+C, Ctrl/Cmd+V "Pastes copied feature", repeatable).
- **Edge cases & errors:** `RESTORE_FEATURE_FAILED`.
- **OCCT mapping:** UI + app data model.
- **Priority:** P0

### PSM-12 Feature/parts list search and filters
- **Purpose:** Filter the features and parts lists.
- **Sources:** PartStudio/part_studios.htm; PartStudio/features_and_parts_lists.htm.
- **Selection inputs:** search box + filter dropdown.
- **Parameters:** plain text (name or type) or commands: `:part`, `:type`, `:name`, `:errors`, `:warnings`, `:folder`,
  `:variable` (case-sensitive), `:suppressed`, `:hidden`, `:shown`, `:partnumber`, `:revision` (last two only when parts list
  properties shown).
- **Behavior / semantics:** `:part part 1` matches Part 1, Part 10.. and features that directly affect those bodies.
- **Edge cases & errors:** n/a.
- **OCCT mapping:** UI + app data model (feature->body affect index from naming history).
- **Priority:** P1

### PSM-13 Feature failure states and error indicators
- **Purpose:** Communicate regeneration problems.
- **Sources:** Home/error_indicators.htm; PartStudio/features_and_parts_lists.htm; PartStudio/feature_basics.htm; error enum.
  (Home/error_messages.htm covers only browser 413/431 errors - not applicable.)
- **Selection inputs:** n/a.
- **Parameters:** n/a.
- **Behavior / semantics:**
  - Feature error: feature name and dialog title shown in error color (help: orange text; dialog title red) with hover summary;
    erroring field outlined red; bad selections red in list and graphics (selected red entity turns darker red).
  - Error badge at top of Features list with error count; click filters to errors (`:errors`).
  - Missing selections: yellow triangle on feature with count.
  - Sketch not fully defined: blue indicator; constraint conflicts shown as red constraint icons; driven dims gray, driving black.
  - Warning/info states (e.g. `SHEET_METAL_END_DONE`, `DERIVED_SM_AUTO_INSERT`) do not block regeneration (`:warnings`).
  - Faulty geometry: part context menu "Ignore faulty part warning (for...)" / "Show faulty part warning" (only in the owning
    Part Studio).
  - General system errors: notification bubble at top.
  - Repair tools (View in repair, Edit healthy moment, Replace reference with propagation) - Document section.
- **Edge cases & errors:** generic: `REGEN_ERROR`, `PARAMETER_PRECONDITION_FAILED`, `PARAMETER_OUT_OF_RANGE`,
  `PARAMETER_VALUE_INVALID`, `MISSING_PARAMETER_REFERENCE`, `FEATURE_NO_SOLIDS`, `CUSTOM_FEATURE_DEFINITION_NOT_FOUND`,
  `DERIVED_BODIES_HAVE_FAULTS`, `SYS_FEATURE_EXCEPTION`.
- **OCCT mapping:** feature result `{status: Ok|Warning|Info|Error, message_key, faulty_params, faulty_entities}`;
  geometry validity via `BRepCheck_Analyzer` to flag faulty parts.
- **Priority:** P0

### PSM-14 Pause regeneration and regeneration times
- **Purpose:** Batch parameter edits without regenerating; profile slow features.
- **Sources:** PartStudio/features_and_parts_lists.htm.
- **Selection inputs:** Pause regeneration button; Show regeneration times button.
- **Parameters:** n/a.
- **Behavior / semantics:** while paused, tables, configuration tables, quantities and booleans (non-selection fields) can be
  edited; banner actions: Regenerate Part Studio (stay paused), Regenerate features and exit, Discard unregenerated changes and
  exit, Restore (undo discard). Switching tabs/closing applies changes; sketches not editable; geometry/tables inconsistent
  until regenerated; not available with Repair mode. Regeneration times panel lists per-feature time; hover highlights feature
  geometry; suppress from there.
- **Edge cases & errors:** n/a.
- **OCCT mapping:** app data model (dirty queue); timing via `std::time::Instant` per feature.
- **Priority:** P2

### PSM-15 Parts list
- **Purpose:** List of bodies produced by the Part Studio.
- **Sources:** PartStudio/features_and_parts_lists.htm; PartStudio/part_studios.htm.
- **Selection inputs:** click item to select; multi-select.
- **Parameters:** n/a.
- **Behavior / semantics:** sections with counts and icons: Parts, Surfaces, Surface meshes (mixed modeling), Composite parts,
  Curves (sketches and mate connectors are listed in the features list, not the parts list - inferred from help structure).
  Hover section title -> eye icon hides/shows all items of that section; hovering a hidden item shows its outline. Right-click
  Parts header -> Show/Hide parts list properties (part number, revision, state next to names; preference "Display parts list
  properties"). Released parts show a release indicator when viewing a release version. Mixed-model parts show a special icon.
- **Edge cases & errors:** n/a.
- **OCCT mapping:** UI over context bodies (kind from `TopoDS_Shape` type: SOLID/COMPSOLID -> part, SHELL/FACE -> surface,
  WIRE/EDGE -> curve, VERTEX -> point).
- **Priority:** P0

### PSM-16 Parts list context menu
- **Purpose:** Commands on parts, surfaces, curves.
- **Sources:** PartStudio/features_and_parts_lists.htm.
- **Selection inputs:** right-click item(s) in list or graphics.
- **Parameters:** n/a.
- **Behavior / semantics:** Rename; Properties (PSM-17); Assign material / Edit material (PSM-18); Edit appearance (PSM-19);
  Copy (paste elsewhere); Create drawing; Export; Release / Release all configurations / Revision history (PDM, out of scope);
  Hide / Hide other / Hide all; Hide/Show center of mass (material required); Isolate; Make transparent; Add comment; Zoom to
  selection; Delete (creates Delete part feature, PSM-23); Switch to (source studio for derived); Ignore faulty part warning.
- **Edge cases & errors:** n/a.
- **OCCT mapping:** UI.
- **Priority:** P0

### PSM-17 Part properties (metadata)
- **Purpose:** Name and metadata of parts (and Part Studios, documents).
- **Sources:** PartStudio/part_studios.htm (Properties dialog fields); Home/properties_metadata.htm.
- **Selection inputs:** Parts list context menu Properties (multi-select allowed); Part Studio tab Properties.
- **Parameters:** Part list name (read-only mirror), Name, Description, Category (pro/enterprise), Part number, Revision,
  State (In progress / Pending / Rejected / Released; editable only in a version with release management off), Vendor,
  Project, Product line, Material (read-only display), Title 1/2/3, Not revision managed (bool), Exclude from all BOMs (bool),
  Unit of measure (help: "Default is Inch"), Mass (displayed once a material is applied; may be
  overridden, see PSM-20), custom properties (company-defined) at bottom.
- **Behavior / semantics:** Apply commits; Save stores without applying; Undo/Redo in dialog; Reset all clears all editable
  properties including hidden ones (appearance, material, tessellation quality) and manual overrides so values computed by
  custom features take effect (no effect on configured properties); Generate next part number icon; grayed fields are
  admin-controlled; properties may be configured (CFG-08); derived parts copy properties if "Include properties" (PSM-25).
  Default part names "Part 1", "Part 2" ... (inferred from examples).
- **Edge cases & errors:** n/a.
- **OCCT mapping:** app data model; on export map name/part number to XCAF (`XCAFDoc_ShapeTool` labels + `TDataStd_Name`,
  `XCAFDoc_` metadata / STEP AP242 properties).
- **Priority:** P0

### PSM-18 Materials and material libraries
- **Purpose:** Assign material (density and mechanical properties) to parts for mass properties.
- **Sources:** PartStudio/customizing_part_materials.htm; Library/onshape_material_library.htm.
- **Selection inputs:** part(s) context menu Assign material / Edit material.
- **Parameters:** Library tab: library dropdown (Onshape Material Library default), material dropdown (shows density),
  search by name/category. Custom tab: Name, Density, Poisson's ratio, Young's modulus, Tensile yield strength, Ultimate tensile
  strength, Compressive yield strength, Ultimate compressive strength. Library CSV columns: Category, Name, Density (unit
  selectable per column).
- **Behavior / semantics:** parts without material have zero mass and are omitted from mass calculations; units shown in
  document units; custom material values not saved for reuse; libraries are CSV tabs in documents, added via account
  preferences (or "+" in the dialog), updated by re-importing CSV and versioning; assigned parts are not updated when library
  changes (must reapply).
- **Edge cases & errors:** `NO_MATERIAL_FOR_MASS_PROPERTY` ("No material assigned."), `NO_MATERIAL_FOR_COMPOSITE_PART_COMPUTED_MASS`.
- **OCCT mapping:** app data model; export via `XCAFDoc_MaterialTool` / `XCAFDoc_Material` (name, description, density) and
  visual via `XCAFDoc_VisMaterialTool`/`XCAFDoc_VisMaterial`.
- **Priority:** P0

### PSM-19 Appearances (part, face, feature, sketch, curve colors; transparency; tessellation)
- **Purpose:** Visual properties.
- **Sources:** PartStudio/customizing_appearances.htm.
- **Selection inputs:** part(s) -> Edit appearance; face(s) -> Add appearance (to selected faces); feature -> Add appearance to
  feature / Edit feature appearance; sketch -> Edit sketch appearance; curve -> Edit appearance; Appearance panel (right side).
- **Parameters:** color via Palette / Mixer / RGB / hex; Custom colors (add, delete, update); Part transparency slider 0 .. 1;
  Tessellation quality: Auto (default), Coarse, Medium, Fine, Very fine (never auto-chosen).
- **Behavior / semantics:** automatic part color sequence of 8 colors restarting at the 9th part; deleting a part keeps
  existing colors (Delete part help says colors are re-sequenced unless custom-assigned - the two help pages differ);
  manual colors persist; face appearances override part color on those faces; feature appearance colors faces created by the
  feature; patterned parts/faces inherit appearances; Alt+click selects through transparent parts; Appearance panel lists all
  overrides (double-click to edit; add face appearance); face colors configurable; imported face appearances toggled by the
  Import feature's "Import appearances" option.
- **Edge cases & errors:** n/a.
- **OCCT mapping:** app data model + renderer; tessellation via `BRepMesh_IncrementalMesh` with deflection per quality level;
  export colors via `XCAFDoc_ColorTool` (surface color per shape/face), transparency via `XCAFDoc_VisMaterial` alpha.
- **Priority:** P1

### PSM-20 Mass properties and mass override
- **Purpose:** Mass, volume, area, center of mass, inertia; user override of mass.
- **Sources:** View/mass_properties_tool.htm; PartStudio/part_studios.htm (Mass property field).
- **Selection inputs:** Mass and section properties dialog (bottom-right scale icon): Part tab (parts, composite parts) or Face
  tab (faces, section faces); optional mate connector for inertia reference.
- **Parameters:** Override checkbox + value (select the entire part in the list first); Show calculation variance.
- **Behavior / semantics:** additive over selections (intersecting parts counted fully each); mass/COM/inertia only from
  parts with materials; inertia about the selection's center of mass unless a mate connector is given; face tab: area,
  perimeter, area and polar moments; values approximate (variance bounds); center-of-mass glyph shown while open, persistent
  via "Show center of mass"; click a value to toggle full precision.
- **Edge cases & errors:** `NO_MATERIAL_FOR_MASS_PROPERTY`, `VOLUME_ACCURACY_MUST_BE_HIGH_FOR_REGEN`.
- **OCCT mapping:** `BRepGProp::VolumeProperties` (with density factor), `BRepGProp::SurfaceProperties`,
  `BRepGProp::LinearProperties`, `GProp_GProps::CentreOfMass`, `MatrixOfInertia`, `PrincipalProperties`; error estimate via
  the `Eps` variant of `VolumeProperties`.
- **Priority:** P1

### PSM-21 Hide / show
- **Purpose:** Toggle visibility of bodies, features, sketches, folders.
- **Sources:** View/hide_show.htm; PartStudio/features_and_parts_lists.htm.
- **Selection inputs:** eye icon; context menu Hide / Show / Hide other / Hide all / Show all; hover + `y` hide, `Shift+y` show;
  `p` planes, `Shift+h` sketches, `k` mate connectors, `Shift+p` construction geometry.
- **Parameters:** per-entity visibility (workspace state, not a feature).
- **Behavior / semantics:** hidden items excluded from graphics and picking; hidden-item outline on list hover; derived parts
  always inserted visible. Visibility is not a regenerated feature (not in history).
- **Edge cases & errors:** n/a.
- **OCCT mapping:** UI/renderer state.
- **Priority:** P0

### PSM-22 Isolate / Make transparent
- **Purpose:** Focus on selected entities by making others transparent (or the reverse).
- **Sources:** View/isolate.htm.
- **Selection inputs:** context menu Isolate / Make transparent; `Shift+i` / `Shift+t` cumulative on hover; grave key select other.
- **Parameters:** Select transparent geometry (bool); transparency / internal geometry slider; Component controls Expand:
  distance (Part Studio + Assembly), Expand: connectivity (Assembly only).
- **Behavior / semantics:** dialog-based mode, exit with x / Esc / context "Exit isolate"; settings remembered across sessions;
  slider extremes auto-uncheck Select transparent geometry; works with folders and section view.
- **Edge cases & errors:** n/a.
- **OCCT mapping:** renderer; distance expansion via bounding-box / `BRepExtrema_DistShapeShape` ordering.
- **Priority:** P1

### PSM-23 Delete part (deleteBodies)
- **Purpose:** Parametric deletion of bodies.
- **Sources:** PartStudio/delete_part.htm; FS: `deleteBodies`.
- **Selection inputs:** "Entities to delete": bodies (mesh allowed, modifiable, non-sketch); with Ignore composite parts, non-composite bodies only.
- **Parameters:**

| FS id | UI label | Type | Enum / default / bounds | Visible when / notes |
|---|---|---|---|---|
| compositePartOption | Composite part options | CompositePartDeleteOptions | DELETE "Delete composite parts", DISSOLVE "Dissolve composite parts", IGNORE "Ignore composite parts" | |
| entities | Entities to delete | Query | | option != IGNORE |
| nonCompositeEntities | Entities to delete | Query | | IGNORE (pick constituent bodies) |

- **Behavior / semantics:** creates a "Delete part" feature; deleting from the Parts list (Delete key) also creates it;
  pressing Delete with a face/edge selected deletes the *feature* that created it instead. Dissolve turns a composite into its
  constituent bodies. Typical use: discard tool bodies after booleans.
- **Edge cases & errors:** `DELETE_COMPOSITE_PART_FAILED`.
- **OCCT mapping:** remove bodies from context (no kernel op).
- **Priority:** P0

### PSM-24 Composite part (compositePart)
- **Purpose:** Group bodies to act as one part (BOM line, release, drawing, properties).
- **Sources:** PartStudio/composite_part.htm; FS: `compositePart` (performs `opCreateCompositePart`).
- **Selection inputs:** "Entities": bodies (modifiable, mesh allowed, non-sketch).
- **Parameters:**

| FS id | UI label | Type | Enum / default / bounds | Visible when / notes |
|---|---|---|---|---|
| bodies | Entities | Query | | |
| closed | Closed | bool | false | closed consumes constituents |

- **Behavior / semantics:** open composite: constituents remain listed and selectable; closed: only the composite is listed
  (bodies still visible/actionable graphically). A closed composite selecting another composite consumes it; open composites
  do not consume and may overlap each other. Composite never owns mate connectors. Export emits separate parts. Import
  automatically makes fractured bodies composite. Features (Delete, Transform ...) may treat open/closed differently.
- **Edge cases & errors:** `CREATE_COMPOSITE_PART_FAILED`, `NESTED_COMPOSITE_PARTS`, `COMPOSITE_CLOSED_OVERLAP`,
  `COMPOSITE_OPEN_OVERLAP`, `COMPOSITE_SKETCH`, `COMPOSITE_MATE_CONNECTOR`, `COMPOSITE_PLANE`, `COMPOSITE_NON_MODIFIABLE`,
  `COMPOSITE_PART_ADD_AND_REMOVE_BODY`, `COMPOSITE_PART_SELECT_ENTITIES`, `MODIFY_COMPOSITE_PART_FAILED`.
- **OCCT mapping:** app data model (composite = id + member body ids + closed flag); geometry as `TopoDS_Compound` for
  export/mass (`BRep_Builder::MakeCompound`).
- **Priority:** P2

### PSM-25 Derived (importDerived)
- **Purpose:** Insert parts, surfaces, curves, sketches, planes, mate connectors, active SM models or whole Part Studios from
  another Part Studio (same or other document/version) with one-way associativity.
- **Sources:** PartStudio/derived.htm; FS: `importDerived` (importDerived.fs).
- **Selection inputs:** "Part Studio" (PartStudioData: element + version + configuration + selected entities; Select Part
  Studio dialog with Current document / Other documents, search by properties, released-parts filter, create version,
  version graph; inline configuration inputs); "Locations" (mate connectors in this studio, implicit or explicit; multiple
  allowed -> multiple instances, discouraged).
- **Parameters:**

| FS id | UI label | Type | Enum / default / bounds | Visible when / notes |
|---|---|---|---|---|
| newUI | (hidden) | bool | true | legacy switch |
| partStudio | Part Studio | PartStudioData | | SHOW_INLINE_CONFIG_INPUTS |
| preserveActiveSheetMetal | Preserve active sheet metal models | bool | false | derive SM model + flat pattern + flat sketches, still editable as SM |
| location | Locations | Query (mate connectors) | | |
| placement | Placement | DerivedPlacementType | AT_ORIGIN "Base origin", AT_MATE_CONNECTOR "Base mate connector" | |
| mateConnectorIndex, mateConnectorId, mateConnectorIndexInFeature | (hidden) | integer / any | DERIVED_MATE_CONNECTOR_COUNT | Base mate connector; chosen by clicking point selectors on derived mate connectors |
| includeMateConnectors | Include mate connectors | bool | true | |
| includeProperties | Include properties | bool | true | false keeps only name, material, appearance |
| parts / buildFunction | (hidden) | Query / BuildFunction | | legacy (newUI false) |

- **Behavior / semantics:**
  - Placement: source origin (or chosen source mate connector) aligned to each Location's CSys (default: this studio's origin).
  - Child features listed under Derived: mate connectors, sketches, planes, sketch patterns/mirrors, transforms (copy-part case).
  - Same document: reference workspace (live) or a version; other documents: version only (link icon, blue when newer version
    exists; update via Reference manager). Linked derived icon in list.
  - Restrictions: one source Part Studio per Derived feature (multiple entities allowed); same source + configuration only
    once per studio (`DERIVED_NO_SAME_SOURCE`); no circular references; derived parts always inserted visible; avoid chains of
    derived. Swapping the source by editing the feature is allowed (downstream may fail -> Repair).
  - Preserve active SM: all parts and flat sketches of the model auto-derived (`DERIVED_SM_AUTO_INSERT`); if the source later
    finishes the model, derived reverts to a plain part and later SM features fail.
- **Edge cases & errors:** `DERIVED_NO_PARTS`, `DERIVED_NO_INSTANCING` (warning), `DERIVED_NO_INSTANCING_SM`,
  `DERIVED_NO_SAME_SOURCE`, `DERIVED_FULL_FEATURE_PATTERN`, `DERIVED_NO_ACTIVE_SM_COMPOSITE`,
  `DERIVED_NO_CLOSED_COMPOSITE_WITH_SM_WARNING/ERROR`, `DERIVED_MATE_CONNECTOR_INDEX_OUT_OF_BOUNDS`,
  `DERIVED_MATE_CONNECTOR_NO_LONGER_VALID`, `DERIVED_MATE_CONNECTOR_RESET`, `DERIVED_NO_MATE_CONNECTORS`,
  `IMPORT_DERIVED_NO_PARTS`, `DERIVED_BODIES_HAVE_FAULTS`, `ELEMENT_REFERENCE_CYCLE_DETECTED`, `MISSING_EXT_REF`.
- **OCCT mapping:** regenerate source studio (cached per version+configuration), copy selected bodies (`BRepBuilderAPI_Copy`
  or share shapes), place with `BRepBuilderAPI_Transform` (`gp_Trsf` from source CSys to target CSys); naming: prefix source
  persistent names with the Derived feature id so downstream references stay stable across source updates.
- **Priority:** P1

### PSM-26 Mixed modeling (mesh bodies)
- **Purpose:** Use imported meshes alongside B-rep.
- **Sources:** PartStudio/mixed_modeling.htm.
- **Selection inputs:** features whose filters include `AllowMeshGeometry.YES`.
- **Parameters:** n/a.
- **Behavior / semantics:** import STL, OBJ, GLTF, 3MF, Parasolid mesh/mixed (x_t+xmm_txt / x_b+xmm_bin); meshes listed with
  mesh icon (Surface meshes section), not editable; supported tools include Delete face, Rib, Shell, Enclose, Boolean, Split,
  Offset surface, Move face, Hole, Mate connector, in-context; export mixed models as STL/Parasolid only.
- **Edge cases & errors:** n/a.
- **OCCT mapping:** OCCT `Poly_Triangulation` faces (triangulation-only `TopoDS_Face` via `BRep_Builder::MakeFace(Poly_Triangulation)`);
  booleans with meshes are limited in OCCT (custom or convert mesh to B-rep facets via `StlAPI`/`BRepBuilderAPI_MakePolygon`
  sewing - slow). Recommend P2 / partial.
- **Priority:** P2

### PSM-27 Custom features and extension model
- **Purpose:** User-authored features (Onshape: FeatureScript in Feature Studios) appear in the toolbar and feature list.
- **Sources:** PartStudio/add_custom_features.htm.
- **Selection inputs:** toolbar "+" (Add custom features) dialog: FeatureScript samples, document filters, search/URL.
- **Parameters:** n/a.
- **Behavior / semantics:** custom features are linked from a specific document version (latest by default); behave like
  built-ins (edit, suppress, hide, delete); out-of-date link icon and Reference manager update (Update all / Selective
  update); current-workspace custom features update live; removing from toolbar keeps existing instances usable.
  FeatureScript itself is out of scope for noshape.
- **Edge cases & errors:** `CUSTOM_FEATURE_DEFINITION_NOT_FOUND`.
- **OCCT mapping:** (design suggestion) define every built-in feature through one Rust trait
  (`FeatureDef { spec(): ParamSpec (ids, labels, types, bounds, visibility predicates), regen(ctx, id, params) }`) mirroring
  FS preconditions, so a later plugin system (WASM or scripting) can register features with the same spec format.
- **Priority:** P2

### PSM-28 Custom tables
- **Purpose:** Tables computed from Part Studio data (FeatureScript-defined) in a right-side panel.
- **Sources:** PartStudio/custom_tables.htm.
- **Selection inputs:** Custom table panel: add enterprise table / custom table / samples (from versioned Feature Studio).
- **Parameters:** n/a.
- **Behavior / semantics:** column move left/right, remove/hide, re-show via overflow; Copy table; parameter groups and
  conditional visibility supported; in drawings, table covers entire studio.
- **Edge cases & errors:** n/a.
- **OCCT mapping:** app extension point (same plugin mechanism as PSM-27); built-in tables (Bend/Rip, Cut list, Inspection) use it.
- **Priority:** P2

### PSM-29 Cut list (cutlist)
- **Purpose:** For frames: create an open composite of segments and a table of segment properties.
- **Sources:** PartStudio/cut_list.htm; FS: `cutlist`. (Frames feature itself is covered by the frames writer.)
- **Selection inputs:** "Frames": solid bodies or composite parts (when not Select all).
- **Parameters:**

| FS id | UI label | Type | Enum / default / bounds | Visible when / notes |
|---|---|---|---|---|
| selectAllFrames | Select all frames | bool | false | includes gussets, end caps |
| frames | Frames | Query | | !selectAllFrames |
| columnOverrides[] | Overrides (item "Column Override", label "#column = #value", no reorder) | array | | |
| .isFirst | (hidden) | bool | | set by editing logic |
| .sameFramesAsPrevious | Same frames as previous | bool | true | not first |
| .overrideAllFrames | All frames | bool | false | first or !same |
| .overrideFrames | Frames | Query | | !overrideAllFrames |
| .column | Column | string | max 10000 | Item, Qty, Length, Angle 1, Angle 2 not overridable |
| .value | Value | string | max 10000 | empty -> column hidden; others "Not set" |

- **Behavior / semantics:** table panel on the right (move left/right, hide, copy, show hidden); gussets/end caps noted in
  Description; insert the composite (not individual frames) into assemblies.
- **Edge cases & errors:** `FRAME_CUTLIST_NO_FEATURE_PATTERN`, `FRAME_CUTLIST_COLUMN_EMPTY`, `FRAME_CUTLIST_NO_END_FACE_EDGE_GEOMETRY_PAIR`,
  `FRAME_MISSING_CAP_FACES`, `FRAME_MISSING_SWEPT_EDGES/FACES`.
- **OCCT mapping:** length = distance between cap faces along sweep path (`BRepGProp` / `BRepExtrema`), angles from cap-face
  normals vs path tangent; composite per PSM-24.
- **Priority:** P2

### PSM-30 Tag (tag)
- **Purpose:** Attach metadata: frame profile tags, sheet metal form tags, PCB hole tags.
- **Sources:** PartStudio/tag.htm; FS: `tag`, predicates `defineFrameTagParams`, `defineFormTagParams`, `definePcbHolesTagParams`.
- **Selection inputs:** per purpose (table).
- **Parameters:**

| FS id | UI label | Type | Enum / default / bounds | Visible when / notes |
|---|---|---|---|---|
| tagPurpose | Tag purpose | TagPurpose | FRAME "Frame", FORM "Form", PCB_HOLES "PCB holes" | remembered |
| sketch | Sketch profile | FeatureList (1) | | Frame |
| alignmentPoints | Sketch points (group Additional alignment points) | Query (sketch vertices) | | Frame |
| standard / description | Standard / Description | string | | Frame (cut-list columns) |
| additionalColumns[] (header, value) | Additional columns | array | | Frame; header cannot be "Standard"/"Description" |
| showDefaultAlignmentPoints | Show default points | bool | true | Frame (9-point grid) |
| positivePart | Part to add | Query (solid, 1) | | Form |
| negativePart | Part to remove | Query (solid, 1) | | Form |
| flatFormSketch | Sketch for flat view | FeatureList | | Form |
| cSysMateConnector | Form origin mate connector | Query (mate connector, 1) | default: created at origin | Form |
| (PCB holes params) | PCB entity type, Plated, Base plane, Holes[...] | | | PCB (out of scope) |

- **Behavior / semantics:** one valid form tag per Part Studio; form tag requires at least one of add/remove; frame tags feed
  Cut list columns.
- **Edge cases & errors:** `FORMED_TAG_FORM_*` (NO_FEATURE_PATTERN, SELECT_DIFFERENT_PARTS, SELECT_SKETCH, SELECT_SOMETHING,
  SELECT_SKETCH_WITH_WIRE_POINT, BODIES_ALREADY_TAGGED, POSITIVE/NEGATIVE_PART_NOT_SOLID, POSITIVE/NEGATIVE_PART_CONSUMED,
  ORIGIN_OUTSIDE_TOOLS_BBOX), `FRAME_TAG_PROFILE_*` (SELECT_SKETCH, SPECIFY_COLUMNS, HEADER_EMPTY, VALUE_EMPTY,
  HEADER_RESERVED, NO_FEATURE_PATTERN).
- **OCCT mapping:** app data model (attributes on bodies/sketches).
- **Priority:** P2

### PSM-31 Model-based definition (brief)
- **Purpose:** PMI (tolerant dimensions, annotations) in the Part Studio with an Inspection table.
- **Sources:** PartStudio/model_based_definition.htm.
- **Selection inputs:** Inspection panel; annotation toolbar.
- **Parameters:** n/a (tolerance options on sketch/feature dimensions owned by Sketch/Feature sections).
- **Behavior / semantics:** tolerant sketch/feature dimensions and annotation-toolbar annotations listed in Characteristics
  table; editable in graphics (double-click opens owning sketch/feature or annotation); cross-highlighting; derived
  annotations read-only; flows into drawings via Suggested views.
- **Edge cases & errors:** n/a.
- **OCCT mapping:** app data model; export as STEP AP242 PMI via `XCAFDoc_DimTolTool` (semantic PMI) - optional.
- **Priority:** P2

### PSM-32 Part Studio tab operations
- **Purpose:** Element-level commands for a Part Studio tab.
- **Sources:** PartStudio/part_studios.htm.
- **Selection inputs:** right-click tab.
- **Parameters:** n/a.
- **Behavior / semantics:** Delete (not the last tab), Open in new browser tab, Rename, Properties, Show code, Duplicate (copy
  with no association), Copy to clipboard (Paste tab in another document), Update linked document, Select as document
  thumbnail, Create drawing of Part Studio, Move to document (creates links if used elsewhere), Export, Release / Release all
  configurations, Create task. Toolbar customization (drag tool sets, New toolset, Reset to default).
- **Edge cases & errors:** n/a.
- **OCCT mapping:** app data model / document layer.
- **Priority:** P1
