# D. Part Studio features II — patterns, transforms, direct edit, reference geometry, variables/configurations, surfaces, curves, frames, sheet metal

Sources: [FS] = FeatureScript std v3095.0 preconditions (authoritative for field order/labels/defaults); [DOCS] = Onshape Help (Oct 2026); [BUNDLE] = web-client JS bundle strings/action IDs. Nothing here was observed live.

## 0. Conventions used in every table

- Column order = exact dialog order from the FS precondition. "param" = FeatureScript parameter id (what gets stored in the feature definition / configured).
- Types: `Query` = selection box (filter noted); `bool` = checkbox (or flip-arrow if UIHint OPPOSITE_DIRECTION*); `enum` = dropdown, or horizontal tab strip if UIHint HORIZONTAL_ENUM; `length`/`angle`/`real`/`int` = expression-capable numeric box (accepts `#variables`, expressions, units); `FeatureList` = picks from the Feature list; `array` = repeatable list group.
- Default length for bounds `LENGTH_BOUNDS`/`NONNEGATIVE_LENGTH_BOUNDS` = 25 mm / 1 in (metric/imperial doc units); `ZERO_DEFAULT_*` = 0; `BLEND_BOUNDS` = 5 mm / 0.2 in; `SHELL_OFFSET_BOUNDS` = 2.5 mm / 0.1 in; `PLANE_SIZE_BOUNDS` = 150 mm. Angle `ANGLE_360_BOUNDS` default 30 deg; `ANGLE_360_FULL_DEFAULT_BOUNDS` 360 deg; `ANGLE_360_90_DEFAULT_BOUNDS` 90 deg. Counts: linear primary [1, 2, 2500], linear secondary [1, 1, 2500], circular [1, 4, 2500], curve [1, 2, 2500] ([min, default, max]). [FS valueBounds.fs]
- Shared predicate field blocks (expanded inline below as "→ <block>"):
  - **Result body operation type** (`operationType`, enum NewBodyOperationType, HORIZONTAL_ENUM tabs: New | Add | Remove | Intersect; default New). [FS booleanStepTypePredicate]
  - **Merge scope block (patterns/mirror)**: shown only if operationType != New: `defaultScope` "Merge with all" bool default **false**; if not Merge-with-all → `booleanScope` "Merge scope" Query (solid bodies, or non-construction non-sketch sheet bodies, modifiable only, mesh allowed). [FS booleanPatternScopePredicate]
  - **Merge scope block (extrude-style)**: same but default Merge with all false and filter = solid bodies only. [FS booleanStepScopePredicate]
  - **Surface merge block**: if surfaceOperationType != New: "Merge with all" default **true**; else "Merge scope" (sheet bodies). [FS surfaceJoinStepScopePredicate]
  - **Curve approximation block**: "Approximate" bool → group "Approximation parameters": Target degree int [2,3,max]; Maximum control points int [4,15,max]; Tolerance length [1e-8 m, 1e-5 m (0.01 mm), 1 m]; Keep start derivative bool; Keep end derivative bool; Show deviation bool → "Maximum deviation" read-only length. [FS curveApproximationPredicate]
- Generic dialog behaviors (all features) [DOCS feature_tools]: tool can be started before or after preselecting geometry (preselection fills the first focused Query); Escape exits; Preview slider (left = before, right = after); "Final" button shows model at bottom of Feature list while editing an earlier feature; dialog-local Undo/Redo while open; green check accepts, X cancels. Feature shortcut toolbar via **S** key in graphics area (customizable in Preferences); tool search **Alt/Opt+C**.
- Keyboard shortcuts relevant to this section [DOCS keyboard_shortcuts]: **Ctrl+M** opens Mate connector dialog; **K** show/hide Mate connectors; **A** flip primary axis (while editing a mate connector); **P** show/hide planes; **Shift+H** show/hide sketches; **Shift+N** rename selected feature/instance; Ctrl+C / Ctrl+V copy/paste features. No default hotkeys exist for patterns, mirror, transform, plane, variables, surface/curve/frame/sheet-metal tools — all are customizable (bundle action IDs `createFeatureLinearPattern`, `createFeatureCircularPattern`, `createFeatureCurvePattern`, `createFeatureMirror`, `createFeatureTransform`, `createFeatureDeleteFace`, `createFeatureMoveFace`, `createFeatureReplaceFace`, `createFeaturePlane`, `createFeatureOffsetSurface`, `createFeatureBoundarySurface`, `createFeatureFill`, `createFeatureMoveBoundary`, `createFeatureRuledSurface`, `createFeatureMutualTrim`, `createFeatureConstrainedSurface`, `createFeatureHelix`, `createFeatureThreeDFitSpline`, `createFeatureProjectedCurve`, `createFeatureBridgingCurve`, `createFeatureCompositeCurve`, `createFeatureIntersectionCurve`, `createFeatureTrimCurve`, `createFeatureIsocline`, `createFeatureOffsetCurve`, `createFeatureIsoparametricCurve`, `createFeatureEditCurve`, `createFeatureRoutingCurve`, `createFeatureDerived`, `createFeatureVariable`, `createFeatureQueryVariable`, `createFeatureCompositePart`, `createFeatureWrap`, `createFeatureDecal`, `createFeatureFrame`, `createFeatureFrameTrim`, `createFeatureGusset`, `createFeatureEndCap`, `createFeatureCutList`, `createFeatureTagProfile`, `createFeatureSheetMetalModelId`, `createFeatureFlange`, `createFeatureHem`, `createFeatureTab`, `createFeatureBend`, `createFeatureSheetMetalJog`, `createFeatureFormed`, `createFeatureTessellatedLoft`, `createFeatureMakeJoint`, `createFeatureCorner`, `createFeatureBendRelief`, `createFeatureFinishSheetMetalModel`, `createFeatureCornerBreak`, `createFeatureMateConnector` (= Ctrl+M)). [BUNDLE]

---

## 1. Patterns, Mirror, Transform

### 1.1 Shared pattern behaviors [DOCS linear/circular/curve pattern]
- Pattern type (first field) = **Part pattern | Feature pattern | Face pattern** (enum PatternType; default Part pattern). [FS]
  - **Part**: replicates bodies (also non-in-context Mate connectors; sketch bodies excluded; mesh allowed). Has Result body operation tabs New/Add/Remove/Intersect. New instances become separate parts (docs warn: identical parts show as separate parts in BOM).
  - **Feature**: picks features from the Feature list (FeatureList param). Selection order irrelevant — features re-applied in Feature-list order. Selecting a pattern feature patterns the pattern but not its seed (select seed too). Patterning a Boolean/Split requires also selecting the features it was applied to. No Result-operation tabs (feature's own booleans are reapplied). End conditions are carried. Not available for sheet metal unless Reapply features is on.
  - **Reapply features** (`fullFeaturePattern`, Feature type only, default false for linear/circular/curve; **true for Mirror**): regenerates each feature per instance (e.g. extrude "Up to next" re-evaluated per instance; external references regenerate per instance). Sketches in reapplied features: dimensions/constraints to Origin or default planes (Top/Front/Right) are NOT reapplied — used to scope what moves. Slower; use only when needed.
  - **Face**: picks faces (non-construction, non-sketch, modifiable). Selection box has **Create selection** button (`SHOW_CREATE_SELECTION`) to grab related faces (tangent/pocket/boss sets) and allows feature selection (`ALLOW_FEATURE_SELECTION`). Fastest; face patterns ignore end-type modifiers.
- **Skip instances** (all patterns except Mirror): checkbox `skipInstances` → group "Skip instances" (expanded by default) with array "Instances to skip" (`skippedInstances`, labels only, no reordering, initial focus). Every instance shows a gray-outlined selection point in the viewport; click it (or box-select several) to skip → point turns light blue and an item appears in the list; X on list item restores; CLEAR empties list. Linear item label "(#index1, #index2)" = position from 0 in dir 1 / dir 2; circular/curve item label "#index".
- **Merge scope**: only parts in scope are affected (e.g. which parts a patterned pocket cuts). Merge with all = all parts. Add/Remove/Intersect with empty scope = error. If geometry touches only one part it is auto-added to scope [DOCS mirror].
- Instance count includes the seed; minimum 1.
- Part-patterned Mate connectors all appear in the Assembly when Part Studio is inserted (owner must be the part).
- Patterns usable inside an active sheet-metal model (Face pattern or Feature pattern with Reapply).
- Count can be configured; bundle has context-menu strings "Configure pattern count"/"Unconfigure pattern count" (assembly patterns). [BUNDLE]

### 1.2 Linear pattern
Toolbar: "Linear pattern" (pattern dropdown group with Circular/Curve pattern & Mirror). Action `createFeatureLinearPattern`. Manipulator change fn `linearPatternPointChange` (drag arrows in viewport update distance/flip). [FS][BUNDLE]

| UI label | param | type | options | default | visible when |
|---|---|---|---|---|---|
| Pattern type | patternType | enum PatternType | Part pattern / Feature pattern / Face pattern | Part pattern | always |
| Result body operation type | operationType | enum tabs | New / Add / Remove / Intersect | New | Part |
| Entities to pattern | entities | Query (bodies or MC, no sketch bodies) | — | — | Part |
| Faces to pattern | faces | Query faces (+Create selection) | — | — | Face |
| Features to pattern | instanceFunction | FeatureList | — | — | Feature |
| Direction | directionOne | Query ALLOWS_DIRECTION, max 1 | edge, planar face (normal), line, MC (implicit/explicit) | — | always |
| Distance | distance | length ≥0 | — | 25 mm | always |
| Instance count | instanceCount | int [1..2500] | — | 2 | always |
| Opposite direction | oppositeDirection | bool flip arrow | — | false | always |
| Centered | isCentered | bool | — | false | always |
| Second direction | hasSecondDir | bool (group driver) | — | false | always |
| ↳ Direction | directionTwo | Query, max 1 | as above | — | hasSecondDir |
| ↳ Distance | distanceTwo | length ≥0 | — | 25 mm | hasSecondDir |
| ↳ Instance count | instanceCountTwo | int [1..2500] | — | 1 | hasSecondDir |
| ↳ Opposite direction | oppositeDirectionTwo | bool flip | — | false | hasSecondDir |
| ↳ Centered | isCenteredTwo | bool | — | false | hasSecondDir |
| Merge with all / Merge scope | defaultScope / booleanScope | → merge block | — | false | Part & op != New |
| Reapply features | fullFeaturePattern | bool | — | false | Feature |
| Skip instances | skipInstances | bool → array of (First direction int, Second direction int) | — | false | always |

Notes [DOCS]: Distance = spacing between consecutive instances. Planar face/MC direction uses its normal. **Centered**: seed sits in the middle; with Instance count N the pattern places N instances (incl. seed) on each side of the seed along the direction. Second direction makes a 2D grid (skip-instance labels are (i, j) grid indices).

### 1.3 Circular pattern
Toolbar "Circular pattern"; manipulator fn `circularPatternPointChange`. [FS]

| UI label | param | type | options | default | visible when |
|---|---|---|---|---|---|
| Pattern type | patternType | enum | Part/Feature/Face pattern | Part pattern | always |
| Result body operation type | operationType | tabs | New/Add/Remove/Intersect | New | Part |
| Entities / Faces / Features to pattern | entities / faces / instanceFunction | as linear | | | per type |
| Axis of pattern | axis | Query ALLOWS_AXIS, max 1 | circular edge, cylindrical/conical face, linear edge/sketch line, sketched circle, MC (primary Z axis) | — | always |
| Angle | angle | angle | — | 360 deg | always |
| Instance count | instanceCount | int [1..2500] | — | 4 | always |
| Opposite direction | oppositeDirection | bool (circular flip arrow) | — | false | always |
| Equal spacing | equalSpace | bool | — | annotation Default true (Defaults map says false; dialog opens checked) | always |
| Centered | isCentered | bool | — | false | always |
| Merge with all / Merge scope | | merge block | | false | Part & op != New |
| Reapply features | fullFeaturePattern | bool | | false | Feature |
| Skip instances | skipInstances → Instances to skip (Index int) | | | false | always |

Notes [DOCS]: Equal spacing ON → instances evenly distributed **within** Angle (e.g. 3 instances in 60 deg); OFF → Angle is between consecutive instances. Axis field has a **mate-connector button** (implicit MC selection) and, once an MC is chosen, clicking the MC icon in the field opens an inline MC edit dialog (alignment/position/angle). Centered: pattern extends both sides of seed.

### 1.4 Curve pattern
Toolbar "Curve pattern"; manipulator fn `curvePatternPointChange`. [FS]

| UI label | param | type | options | default | visible when |
|---|---|---|---|---|---|
| Pattern type | patternType | enum | Part/Feature/Face pattern | Part pattern | always |
| Result body operation type | operationType | tabs | New/Add/Remove/Intersect | New | Part |
| Entities / Faces / Features to pattern | | as linear | | | per type |
| Path to pattern along | edges | Query: edges or non-sketch wire bodies (multi, ordered chain) | — | — | always |
| Spacing type | spacingType | enum CurvePatternSpacingType | Equal spacing / Distance | Equal spacing | always |
| Distance | distance | length ≥0 | — | 25 mm | spacing = Distance |
| Instance count | instanceCount | int [1..2500] | — | 2 | always |
| Orientation | orientationType | enum CurvePatternOrientationType (label shown) | Tangent to curve / Normal to face / Locked | Tangent to curve | always |
| Faces to be normal to | lockedFaces | Query faces (non-construction) | — | — | Orientation = Normal to face |
| Merge with all / Merge scope | | merge block | | false | Part & op != New |
| Reapply features | fullFeaturePattern | bool | | false | Feature |
| Skip instances | skipInstances → Index list | | | false | always |

Notes [DOCS]: Path = sketch curve or chain of adjacent curves/solid edges/wire edges, used in selection order; if result is unexpected, reselect in different order. Error if path too short for count × Distance. Tangent to curve = instances rotate to follow tangent; Normal to face = keep aligned to selected face(s); Locked = instances keep seed orientation (translate only).

### 1.5 Mirror
Toolbar "Mirror"; editing logic fn `mirrorEditLogic`. No skip instances. [FS]

| UI label | param | type | options | default | visible when |
|---|---|---|---|---|---|
| Mirror type | patternType | enum MirrorType | Part mirror / Feature mirror / Face mirror | Part mirror | always |
| Result body operation type | operationType | tabs | New/Add/Remove/Intersect | New | Part |
| Entities to mirror | entities | Query bodies (mesh ok, no sketch) | — | — | Part |
| Faces to mirror | faces | Query faces (+Create selection) | — | — | Face |
| Features to mirror | instanceFunction | FeatureList | — | — | Feature |
| Mirror plane | mirrorPlane | Query ALLOWS_PLANE, max 1 | plane, planar face, MC (implicit/explicit; XY plane of MC) | — | always |
| Merge with all / Merge scope | | merge block | | false | Part & op != New |
| Reapply features | fullFeaturePattern | bool | | **true** | Feature |

Notes [DOCS]: mirrors parts and surfaces; usable in active sheet metal (Face mirror). Mirror plane field has MC button for implicit MCs; MC icon in field opens inline MC editor. Docs show slider-right instant preview.
- Related hidden feature **Derived mirror** (`derivedMirror`): single read-only, unconfigurable `partStudio` param — created by system (mirrored derived Part Studio), not in toolbar. [FS]

### 1.6 Transform
Toolbar "Transform". Manipulator change fn `transformManipulatorChange` (drag triad/arrow/rotation ring in viewport; positioned at bounding-box center of selected entities). Action `createFeatureTransform`. [FS transformCopy.fs][BUNDLE]

| UI label | param | type | options | default | visible when |
|---|---|---|---|---|---|
| Entities to transform or copy | entities | Query bodies or (non-in-context) MCs, mesh ok, no sketch bodies | — | — | always |
| Transform type | transformType | enum TransformType | Translate by line / Translate by distance / Translate by XYZ / Transform by mate connectors / Rotate / Copy in place / Scale | Translate by line | always |
| Opposite direction | oppositeDirectionEntity | bool flip | — | false | Translate by line |
| Line or points | transformLine | Query vertex/edge, max 2, ordered | 1 line (length = distance) or 2 points (from→to) | — | Translate by line |
| Axis | transformAxis | Query ALLOWS_AXIS, max 1 | — | — | Rotate |
| Direction | transformDirection | Query ALLOWS_DIRECTION or vertex, max 2 | line, face normal, MC, 2 points | — | Translate by distance |
| Distance | distance | length >0 | — | 25 mm | Translate by distance |
| Scale uniformly | uniform | bool | — | true | Scale |
| Scale | scale | real [1e-5..1e5] | — | 1 | Scale & uniform |
| X scale / Y scale / Z scale | scaleX/Y/Z | real | — | 1 | Scale & !uniform |
| From mate connector | baseConnector | Query MC, max 1 | — | — | by MCs |
| To mate connector | destinationConnector | Query MC, max 1 | — | — | by MCs |
| Flip primary axis | oppositeDirectionMateAxis | bool (primary-axis toggle, first in row) | — | false | by MCs |
| Reorient secondary axis | secondaryAxisType | enum MateConnectorAxisType (axis-type button) | +X / +Y / -X / -Y | +X | by MCs |
| Angle | angle | angle | — | 30 deg | Rotate |
| Opposite direction | oppositeDirection | bool flip | — | false | Rotate or Translate by distance |
| X translation / Y translation / Z translation | dx / dy / dz | length | — | 0 | Translate by XYZ |
| Point or mate connector | scalePoint | Query ALLOWS_VERTEX, max 1 | point (center) or MC (coordinate system for non-uniform) | — | Scale |
| Copy part | makeCopy | bool | — | false | type != Copy in place |

Behavior [DOCS][FS]:
- Translate by XYZ shows a drag manipulator along each world axis. Rotate angle normalized internally; negative manipulator drag sets Opposite direction.
- Errors: no entities = error; null transform (coincident points / zero distance) = "points coincident" error (since V74).
- Copy in place: creates independent duplicate at same location (Parts list count increases). For many copies docs recommend Pattern with distance 0. Copy part (makeCopy) = transform a copy, keep original (internally opPattern instead of opTransform).
- Selecting a part's Mate connector along with the part moves it with the part. Scale about MC → X/Y/Z relative to MC axes.
- Deprecated **Copy part** feature (`copyPart`, single field "Entities to copy" Query bodies): hidden/deprecated; replaced by Transform → Copy in place. [FS]

---

## 2. Direct editing: Delete face, Move face, Replace face, Offset surface

All four: "Direct Editing" tools, ideal for imported parts without history [DOCS]. Face selection boxes have the **Create selection** button (UIHint SHOW_CREATE_SELECTION) to add related faces (e.g. all faces of a boss/pocket, tangent chain). Mesh faces allowed. [FS]

### 2.1 Delete face
Editing logic `deleteFaceEditLogic`. [FS]

| UI label | param | type | options | default | visible |
|---|---|---|---|---|---|
| Delete faces | deleteFaces | Query faces (non-construction, non-sketch, modifiable) | — | — | always |
| Type | healType | enum DeleteFaceType (REMEMBER_PREVIOUS_VALUE) | Heal / Cap / Leave open | Heal (then last used) | always |
| Delete fillet faces | includeFillet | bool | — | true | always |

- Heal = extend surrounding faces until they intersect; Cap = place a face over the void; Leave open = remove face, solid becomes a surface (sheet body). [DOCS]
- **Type remembered** across invocations (dialog opens with last-used). Delete fillet faces also removes adjacent fillet faces. [DOCS][FS]

### 2.2 Move face
Manipulator fn `moveFaceManipulatorChange` (drag arrow/rotation handle); editing logic `moveFaceEditingLogic`. [FS]

| UI label | param | type | options | default | visible |
|---|---|---|---|---|---|
| (Creation type) | outputType | enum MoveFaceOutputType, ALWAYS_HIDDEN | Move / Create | Move | hidden |
| Faces to move | moveFaces | Query faces (in active sheet metal only SM definition faces/edges) | — | — | outputType=Move |
| Faces, surfaces, and sketch regions to offset | surfacesAndFaces | Query faces/sheet bodies | — | — | outputType=Create (hidden path) |
| Move face type | moveFaceType | enum MoveFaceType | Offset / Translate / Rotate | Offset | always |
| Direction | direction | Query ALLOWS_DIRECTION, max 1 | MC/axis (parallel), edge (parallel), face (normal) | — | Translate |
| Axis | axis | Query ALLOWS_AXIS, max 1 | axis, MC | — | Rotate |
| End type | limitType | enum MoveFaceBoundingType (label shown) | Blind / Up to entity | Blind | type != Rotate |
| Distance | translationDistance | length ≥0 | — | 5 mm | Translate & Blind |
| Rotation angle | angle | angle | — | 0 deg | Rotate |
| Distance | offsetDistance | length ≥0 | — | 5 mm | Offset & Blind |
| Opposite direction | oppositeDirection | bool flip | — | false | Blind or Rotate |
| Up to entity | limitEntity | Query planar face or vertex, max 1 | — | — | Translate & Up to entity |
| Up to entity | limitQuery | Query vertex/edge/face/body, max 1 | — | — | Offset & Up to entity |
| Offset distance | hasOffset | bool (short, first in row) | — | false | Up to entity |
| Offset | offset | length ≥0 (short) | — | 0 | hasOffset |
| Opposite offset direction | oppositeOffsetDirection | bool flip | — | **true** | hasOffset |
| Reapply fillet | reFillet | bool | — | true | outputType=Move |

- Offset: typically changes radii of non-planar faces. Translate Up to entity: vertex or parallel face. Rotate: degrees about axis/MC; arrows flip rotation. [DOCS]

### 2.3 Replace face
Manipulator fn `replaceFaceManipulatorChange` (offset drag arrow); editing logic `replaceFaceEditLogic`. [FS]

| UI label | param | type | options | default | visible |
|---|---|---|---|---|---|
| Faces to replace | replaceFaces | Query faces (modifiable, non-construction) | — | — | always |
| Faces to replace with | templateFace | Query faces (any, incl. surfaces/other parts) | — | — | always |
| (Show Flip) | showFlip | bool ALWAYS_HIDDEN | — | true | hidden |
| Flip alignment | oppositeSense | bool | — | false | showFlip (set by editing logic) |
| Offset distance | offset | length ≥0 | — | 0 | always |
| Opposite direction | oppositeDirection | bool flip | — | false | always |

- Trims or extends selected faces to the replacement surface (often a helper surface you later hide in Parts list). Flip alignment reverses which side of the template the face aligns to. Multiple faces can be replaced at once. [DOCS]

### 2.4 Offset surface
Manipulator fn `offsetSurfaceManipulatorChange`. [FS]

| UI label | param | type | options | default | visible |
|---|---|---|---|---|---|
| Faces, surfaces, and sketch regions to offset | surfacesAndFaces | Query faces / non-sketch sheet bodies (+Create selection; sketch regions = faces) | — | — | always |
| Offset | offset | length ≥0 (REMEMBER_PREVIOUS_VALUE) | — | 5 mm (then last used) | always |
| Opposite offset direction | oppositeDirection | bool flip | — | false | always |

- Creates a new surface (sheet body); offset 0 = copy in place. Multiple adjacent faces yield one surface. **Offset value remembered**. [DOCS]

---

## 3. Reference geometry & misc Part Studio features

### 3.1 Plane
Toolbar "Plane" (action `createFeaturePlane`). Feature-level UIHint CONTROL_VISIBILITY (plane has eye toggle in Feature list); manipulator fn `cplaneManipulatorChange`; editing logic `cPlaneLogic` (infers plane type from selections). [FS]

| UI label | param | type | options | default | visible |
|---|---|---|---|---|---|
| Entities | entities | Query multi: planes/planar faces, vertices, axes (lines, cyl faces), edges, MCs, mesh | — | — | always |
| Plane type | cplaneType | enum CPlaneType (REMEMBER_PREVIOUS_VALUE) | Offset / Plane point / Line angle / Point normal / Three point / Mid plane / Curve point / Tangent | Offset (then last used; editing logic infers from selection) | always |
| Offset distance | offset | length [0..500 m] | — | 25 mm | Offset |
| Angle | angle | angle | — | 0 deg | Line angle |
| Opposite direction | oppositeDirection | bool flip | — | false | Offset or Line angle |
| Flip alignment | flipAlignment | bool | — | false | Mid plane or Tangent |
| Flip normal | flipNormal | bool | — | false | always |
| (Starting width/height) | width / height | length ALWAYS_HIDDEN | — | 150 mm | hidden |

Plane types [DOCS]:
- **Offset**: plane/planar face/MC XY plane (or circular edge) + distance. Drag manipulator updates distance; click arrow flips. Preselecting a planar face → defaults to Offset.
- **Plane point**: through a point, parallel to plane.
- **Line angle**: through a line at an angle relative to reference (plane/point/axis); angle drag manipulator; arrow flips.
- **Point normal**: axis (line, circle, arc, cyl/revolved face) = normal; point = origin. Preselecting line + point → defaults to Point normal.
- **Three point**: through 3 points.
- **Mid plane**: between two parallel planes (centered) or bisecting two intersecting planes; Flip alignment chooses bisector.
- **Curve point**: normal tangent to curve/edge at a point (vertex/MC) — used for sweep profiles on helices.
- **Tangent**: tangent to cylinder face, through a point (coplanar) or parallel to a plane.
- Inputs recognized: Plane (plane, planar face, MC), Point (vertex, sketch point, MC, Origin), Line (linear edge, sketch line, cylindrical face axis).
- Flip normal: plane name text in graphics area indicates normal side (text mirrored when flipped).
- Context menu on plane: **Size to Part Studio** (resize to enclose everything), **Section view**. Shortcut **P** hides/shows all planes. Default size 150 mm square.

### 3.2 Point (FS-only, not a standard toolbar tool)
`cPoint` "Point": Point type (Edge point / Mesh point; default Edge point) → Edge point: Edge (Query edge, max 1), Parameter (real [0..1], default 0.5); Mesh point: Mesh (mesh face, max 1), X/Y/Z (length, default 0). Docs describe only sketch points. [FS]

### 3.3 Mate connector (Part Studio)
Toolbar "Mate connector"; shortcut **Ctrl+M**; **K** toggles MC visibility. Feature UIHint CONTROL_VISIBILITY; editing logic `connectorEditLogic` (on create: auto-sets Owner entity and Attachment). [FS][DOCS]

| UI label | param | type | options | default | visible |
|---|---|---|---|---|---|
| Origin type | originType | enum OriginCreationType | On entity / Between entities | On entity | always |
| Origin entity | originQuery | Query edge/vertex/non-construction face (or inferred point), max 1; unconfigurable, no query variable | — | — | always |
| (hidden) | entityInferenceType | enum ALWAYS_HIDDEN | PART_ORIGIN, POINT, CENTROID, CENTER, MID_POINT, TOP/MID/BOTTOM_AXIS_POINT, ORIGIN_X/Y/Z, LOOP_CENTER, VIRTUAL_SHARP | set by pick | hidden |
| (hidden) | secondaryOriginQuery | Query | — | — | hidden |
| Between entity | originAdditionalQuery | Query face, max 1 | — | — | Between entities |
| Realign | realign | bool | — | false | always |
| Primary axis entity | primaryAxisQuery | Query face/edge, max 1 (controls Z) | — | — | realign |
| Secondary axis entity | secondaryAxisQuery | Query face/edge, max 1 (controls X/Y) | — | — | realign |
| Move | transform | bool | — | false | always |
| X / Y / Z translation | translationX/Y/Z | length | — | 0 | Move |
| Rotation axis | rotationType | enum RotationType | Rotate about X / Rotate about Y / Rotate about Z | Rotate about Z | Move |
| Rotation angle | rotation | angle | — | 0 deg | Move |
| Owner entity | requireOwnerPart | bool | — | true | (allowOwnerEntity hidden, true) |
| Select owner entity | ownerPart | Query body (solid/mesh/sheet/wire/composite/point), max 1 | — | auto: owner of picked entity, or the single body in studio | requireOwnerPart |
| Attachment | attachmentOption | enum AttachmentOption (label shown) | None / To owner / To selection | auto: To selection if origin is face/body of owner; To owner if owner set; else None | requireOwnerPart |
| Attach to | attachTo | Query face/body/MC, max 1 | — | origin face | Attachment = To selection |
| (Specify normal, Normal x/y/z) | specifyNormal, nx, ny, nz | hidden | — | false/0 | hidden |
| Flip primary axis | flipPrimary | bool flip (first in row) | — | false | always |
| Reorient secondary axis | secondaryAxisType | MC axis-type button | +X / +Y / -X / -Y | +X | always |

Behavior [DOCS]:
- Explicit MCs (toolbar) listed top-level in Feature list; implicit MCs are created inside other feature dialogs (MC button next to axis/direction/plane fields; not selectable from Feature list).
- **Inference points**: with dialog open, hovering a face/edge "wakes up" inference points; nearest highlights as candidate MC. Hold **Shift** to lock the current inference set (stop waking others). Points per entity: planar face → every vertex, arc center, edge midpoint, face centroid, centroids of inner loops (holes/slots), parallel to face; cylindrical face → on axis at top/middle/bottom (perpendicular); linear edge/sketch line → middle & ends; circular edge → center/middle & ends; cone → top/mid/bottom/virtual sharp; curve endpoint/control point → aligned with global Z; curve → tangent at closest end/control/midpoint; vertex → normal to given face. Clicking anywhere on a face places at centroid. Origin selectable (graphics or list). Connected entities cross-highlight on hover.
- Flip primary axis = rotate 180 deg (shortcut **A** while editing); Reorient secondary = rotate X/Y one quadrant (90 deg) per click.
- Owner entity blank → ownerless MC. Part-owned MCs travel with part into Assemblies; hide state in Part Studio is independent of Assembly.
- Feature list context menu: Hide / Hide other Mate connectors / Show / Show all Mate connectors; eye icon per MC.
- MC may be created on an existing MC (then moved/realigned).

### 3.4 Helix
Toolbar "Helix"; manipulator `helixManipulatorChange` (height drag); editing logic `helixEditingLogic` (sets axis type from preselection: cyl/cone face → Cylinder/Cone, circle/arc → Circle, line/MC → Axis). [FS]

| UI label | param | type | options | default | visible |
|---|---|---|---|---|---|
| Helix axis type | axisType | enum AxisType (horizontal tabs) | Cylinder/Cone / Axis / Circle | Cylinder/Cone | always |
| Conical or cylindrical face | entities | Query face, max 1 | — | — | Cylinder/Cone |
| Helix axis | axis | Query ALLOWS_AXIS, max 1 (line/edge/MC Z; implicit MC button) | — | — | Axis |
| Circular edge | edge | Query circle/arc, max 1 | — | — | Circle |
| Input type | pathType | enum PathType | Turns / Pitch / Turns and pitch | Turns | always |
| Opposite direction | oppositeDirection | bool flip | — | false | always |
| Start condition | startType | enum StartType | Start angle / Start point | Start angle | always |
| Opposite origin | oppositeOrigin | bool flip | — | false | Axis |
| Start angle | startAngle | angle | — | 0 deg | Start angle |
| Start radius | startRadius | length ≥0 | — | 25 mm | Start angle & Axis |
| Start point | startPoint | Query vertex, max 1 | — | — | Start point |
| End condition | endType | enum EndType | Height / End point | Height | Input type != Turns and pitch |
| Height | height | length >0 | — | 25 mm | End=Height & axis type != Cylinder/Cone |
| End radius | endRadToggle | bool | — | false | Height & Axis |
| End radius | endRadius | length ≥0 | — | 25 mm | endRadToggle |
| End point | endPoint | Query vertex, max 1 | — | — | End=End point |
| Target revolutions | revolutions | real [1e-5..1000] | — | 4 | Turns or Turns and pitch |
| Computed revolutions | actualRevolutions | real read-only | — | — | Turns & End point |
| Flip rounding | roundUpTurns | bool flip | — | false | Turns & End point |
| Target pitch | helicalPitch | length >0 | — | 25 mm | Pitch or Turns and pitch |
| Computed pitch | actualPitch | read-only length | — | — | Pitch & End point |
| Flip rounding | roundUpPitch | bool flip | — | false | Pitch & End point |
| Handedness | handedness | enum Direction | Clockwise / Counterclockwise | Clockwise | always |
| Show start and end profiles | showStartEndProfiles | bool | — | false | always |

Notes [DOCS]: Cylinder/Cone height = face height (no Height field); Start angle measured from seam/x-axis of the face; Start radius & End radius (Axis type) make tapered helix; End point on circle plane = flat spiral. Helix doesn't consume the face body. Spring workflow: helix → Curve point plane at helix end → sketch circle → Sweep.

### 3.5 Composite part
Toolbar "Composite part". [FS][DOCS]

| UI label | param | type | default |
|---|---|---|---|
| Entities | bodies | Query bodies (modifiable, non-sketch; parts/surfaces/curves/points/mesh) | — |
| Closed | closed | bool | false |

- Closed = consumes constituent bodies (Parts list shows only the composite; constituents still selectable graphically); Open = constituents remain listed too. Acts as one part (BOM line, release, metadata, drawings, features e.g. Delete/Transform). Listed in a separate "Composite parts" section of Parts list; context menu "Hide other composite parts" / "Hide all composite parts".
- Dissolve: **Delete part** feature dropdown → "Dissolve composite parts" (back to bodies) / "Ignore composite parts" (pick an individual body to delete).
- Composites never own MCs (owned by individual bodies). Closed composite consumes selected composites; open does not. Two open composites can overlap. Export → separate parts. Import-fractured bodies auto-composited.

### 3.6 Derived
Toolbar "Derived" (action `createFeatureDerived`); manipulator `onManipulatorChange`. [FS]

| UI label | param | type | options | default | visible |
|---|---|---|---|---|---|
| (newUI) | newUI | bool hidden | — | true | hidden |
| Part Studio | partStudio | PartStudioData (opens "Select Part Studio" picker; configuration inputs shown inline) | — | — | newUI |
| Preserve active sheet metal models | preserveActiveSheetMetal | bool | — | false | newUI |
| Locations | location | Query MCs (explicit or implicit) — "Select or create mate connectors in this Part Studio to position derived instances" | — | — (base origin = this origin) | newUI |
| Placement | placement | enum DerivedPlacementType | Base origin / Base mate connector | Base origin | newUI |
| (MC index/id) | mateConnectorIndex, mateConnectorId, mateConnectorIndexInFeature | hidden | | | Base mate connector |
| Include mate connectors | includeMateConnectors | bool | — | true | newUI |
| Include properties | includeProperties | bool (REMEMBER_PREVIOUS_VALUE) | — | true | newUI |
| (legacy) Parts to import / buildFunction | parts / buildFunction | hidden | | | !newUI |

Behavior [DOCS]:
- Select Part Studio dialog: tabs **Current document** (can pick other version) / **Other documents** (only versioned docs; notice if no/ newer version, with Create version); search box (names, properties incl. custom; multiple words), filter button, released-parts filter, create version, version graph icons. Pick a whole Part Studio or specific parts, surfaces, curves, sketches, planes, active SM models, MCs (one Part Studio per Derived feature). Close with X → inserted at base origin.
- Multiple Locations = multiple instances (docs discourage; use assembly). Placement Base mate connector → point selectors appear on derived MCs; click one to use as base frame.
- Include properties off → only name, material, appearance carried. Choice remembered.
- Preserve active sheet metal → derived SM model, flat pattern, flat sketches stay editable (add flange/hem/joint). If source later uses Finish sheet metal, derived reverts and downstream SM features fail.
- Feature list: link icon for cross-document derive; turns blue when newer version exists (also on tab) → update via linked-document flow. Child features listed beneath (expand arrow): MCs, sketches, planes, patterns/mirrors of sketches, Transforms with Copy part. Eye icon toggles.
- Cannot derive the same Part Studio+configuration twice in one Part Studio; no circular references; one-way associativity; derived parts always visible on insert. Can swap source by editing.

### 3.7 Wrap
Toolbar "Wrap" (action `createFeatureWrap`); manipulator `wrapManipulatorChange`; editing logic `wrapEditLogic`. [FS][DOCS Sketch__wrap]

| UI label | param | type | options | default | visible |
|---|---|---|---|---|---|
| Creation type | resultType | enum tabs WrapResultType | Solid / Surface / Split | Surface | always |
| Result body operation type | operationType | tabs | New/Add/Remove/Intersect | New | Solid |
| Tools | source | Query planar non-construction faces / sketch regions | — | — | always |
| Target | destination | Query cylindrical/conical face, max 1 | — | — | always |
| Flip alignment | flipAlignment | bool flip | — | false | always |
| Trim to target | trim | bool | — | false | type != Split |
| Specify anchor points | customAnchors | bool | — | false | always |
| Tools anchor point | sourceAnchor | Query vertex, max 1 | — | — | customAnchors |
| Target anchor point | destinationAnchor | Query vertex, max 1 | — | — | customAnchors |
| group **Position** (collapsed by default): Angle | angle | angle | — | 0 | always |
| Angle opposite direction | angleOppositeDirection | bool circular flip | — | false | always |
| U shift / U shift opposite direction | uShift / uShiftOppositeDirection | length / flip | — | 0 / false | always |
| V shift / V shift opposite direction | vShift / vShiftOppositeDirection | length / flip | — | 0 / false | always |
| Thickness | thickness | length | — | 2.5 mm | Solid |
| Thickness opposite direction | oppositeDirection | bool flip | — | false | Solid |
| Merge with all / Merge scope | defaultScope / booleanScope | merge block (solids) | — | false | Solid & op != New |

- Solid = emboss/engrave; Surface = wrapped surfaces; Split = imprint/split target face. Trim to target hides tool portions past target borders. Anchor points map one tool point to one target point. Angle tilts, U shift horizontal, V shift vertical. [DOCS]

### 3.8 Decal
Toolbar "Decal" (`createFeatureDecal`); UIHint NO_PREVIEW_PROVIDED; editing logic `onDecalFeatureChange`; manipulator `onDecalManipulatorChange`. [FS]

| UI label | param | type | options | default | visible |
|---|---|---|---|---|---|
| Image | image | ImageData ("Select image" dialog: Current document / Other documents / Libraries; Import link) | — | — | always |
| Face | face | Query planar or cylindrical face, max 1 | — | preselected face | always |
| Angle | angle | angle | — | 0 | always |
| Angle opposite direction | angleOppositeDirection | bool circular flip | — | false | always |
| U shift | uShift | length | — | 0 | always |
| V shift | vShift | length | — | 0 | always |
| Realign | realign | bool | — | false | always |
| Placement origin | originConnector | Query MC (implicit ok), max 1 | — | — | realign |
| Horizontal reference | horizontalReference | Query direction or MC, max 1 | — | — | realign |
| Maintain aspect ratio | maintainAspectRatio | bool | — | true | always |
| Aspect ratio constraint | aspectRatioConstraint | enum | Width / Height | Width | maintainAspectRatio |
| Width | width | length | — | auto-fit face (bound default 25 mm) | !maintain or Width |
| Height | height | length | — | auto-fit | !maintain or Height |
| (Image origin index) | imageOriginIndex | int hidden (set by clicking one of 9 reference points) | — | — | hidden |

- Manipulators: V arrow, U arrow, angle ring (0-360), size triangle (keeps aspect). Realign shows 9 reference points around image; click one to align it to Placement origin. Image auto-fits face on placement. PNG/JPG/GIF/BMP, max 4096x4096; transparency supported. Face changed to non-plane/cylinder → feature error. Visible in drawings only in shaded views. [DOCS]

### 3.9 Tag (brief)
Toolbar "Tag" (`createFeatureTagProfile`). "Tag purpose" enum (REMEMBER_PREVIOUS_VALUE): **Frame / Form / PCB holes** (default Frame). [FS tag.fs]
- Frame: Sketch profile (FeatureList, 1 sketch); group "Additional alignment points" (collapsed): Sketch points (sketch vertices); Standard (string); Description (string); Additional columns (array, "Add Column": Header, Value; label "#header = #value"); Show default points (bool, default true, remembered) → magenta 9-point alignment grid on profile. Values feed Cut list columns.
- Form: Part to add (solid, 1); Part to remove (solid, 1); Sketch for flat view (FeatureList); Form origin mate connector (MC, 1; if none, created at Origin). Only one valid Form tag per Part Studio.
- PCB holes: out of scope (PCB).

### 3.10 Name entity (brief, FS-only)
"Name entity": Entity (face/body/edge, max 1), Name (string). Feature name template "Name #entityName"; no preview. Not documented in Help; not a standard toolbar item. [FS]

---

## 4. Variables, Query variables, Variable table, Configurations, Variable Studios

### 4.1 Variable feature
Toolbar "Variable" (`createFeatureVariable`). Feature name template **"#name = value"** (Feature list shows e.g. `#width = 25 mm`); tooltip "#name = value description"; no preview; editing logic `variableEditLogic`. [FS variable.fs]

| UI label | param | type | options | default | visible |
|---|---|---|---|---|---|
| (Entities) | initEntities | Query ALWAYS_HIDDEN (preselection seeds Measured mode) | — | — | hidden |
| Mode | mode | enum tabs VariableMode | Assigned / Measured / From import | Assigned | always |
| Variable type | variableType | enum tabs (UNCONFIGURABLE) | Length / Angle / Number / Any | Length | Assigned |
| Measurement | measurementMode | enum tabs | Distance / Length / Diameter | Distance | Measured |
| Name | name | string (VARIABLE_NAME, UNCONFIGURABLE, max 10000) | — | empty (feature title shows "?" until named) [FS] | always |
| Value | lengthValue | length (initial focus on edit) | — | 0 | Assigned & Length |
| Value | angleValue | angle | — | 0 | Assigned & Angle |
| Value | numberValue | real [-1e12..1e12] | — | 0 | Assigned & Number |
| Value | anyValue | any FS expression (bool, map, array, string, function, other units) | — | — | Assigned & Any |
| CSV file | csv | TableData ("Select CSV file" picker: Current document / Other documents / Import) | — | — | From import |
| group **Row** (expanded): Access type | rowAccessType | enum AccessType | By index / By index range / By label / All values | By index | From import |
| Index (0-based) | rowIndex | int [0..1e5] | — | 0 | Row By index |
| Value / Regular expression | rowValue / rowRegexp | string / bool | — | — | Row By label |
| Min index / Max index (0-based) | rowMinIndex / rowMaxIndex | int | — | 0 | Row By index range |
| Result type | rowResult | enum ResultType | Single value / Array / Map | Single value | From import |
| Label column index | rowLabelIndex | int | — | 0 | Row By label or Map |
| group **Column** (expanded): same fields — Access type, Index, Value/Regular expression, Min/Max index, Result type, Label row index | column* | | | | From import |
| Conversion factor | useConversionFactor | bool (short, first in row) | — | false | From import |
| Conversion factor | conversionFactor | any (value with units) | — | — | useConversionFactor |
| Entities to measure between | entityCouple | Query face/edge/vertex/body/MC, max 2 | — | — | Measured & Distance |
| Minimum/Maximum | minmax | enum | Minimum / Maximum | Minimum | Distance |
| Extend entities | extendEntities | bool ("Measure to infinitely extended lines, arcs, cylinders, etc.") | — | false | Distance & Minimum |
| Use axis | measureFromAxis | bool ("Measure to axis of cylinders, circles, arcs, or revolved surfaces") | — | false | Distance & Minimum |
| group **Axis measurements** (collapsed): Distance, Along X, Along Y, Along Z | distance, xOffset, yOffset, zOffset | read-only lengths | — | — | Distance |
| Value to record | componentSelector | enum AxisWithCustom | Distance / Along X / Along Y / Along Z / Along custom | Distance | Distance |
| Custom direction / Along custom | customDirection / customOffset | Query direction / read-only | — | — | Along custom |
| Entities | lengthEntities | Query edges / wire bodies (total length) | — | — | Measured & Length |
| Radius | radius | bool (record radius instead of diameter) | — | false | Measured & Diameter |
| Entity | diameterEntity | Query circle/arc/cylinder/sphere, max 1 | — | — | Measured & Diameter |
| Description | description | string max 256 | — | "" | always |

Behavior [DOCS]:
- Name rules: English alphanumerics + underscore, case-sensitive, cannot start with digit (must be FeatureScript identifier to appear in Variable table). Referenced as `#name` in any numeric field / sketch dimension / expression.
- Variables are features: position in Feature list matters; only variables above the current feature are visible.
- **Autocomplete**: typing `#` (or letter/underscore) in a numeric field opens dropdown of variables (computed values in light gray, not inserted) and FeatureScript functions (functions only listed when no `#`). Type to filter, arrows + Enter accept, scroll wheel / click, Escape or blur closes. Last entry "**New variable…**" opens a Create variable dialog with choice **Feature (default) / Configuration** variable; new variable feature is inserted just before the current feature.
- Sketch dimensions show an indicator icon left of value: "expression without variable" vs "expression with variable". Editing reveals the expression.
- Arrays/index pattern: `[0.25, 0.5, 1][#config]`. Any type can hold functions: `function(len) { return len * 2 + 2.5 mm; }` then `#Adjust(20mm)`.
- Renaming a variable prompts checkbox **Update all references**.
- From import: hover feature name → tooltip shows result; in Variable table the type/value become read-only.

### 4.2 Query variable
Toolbar "Query variable" (`createFeatureQueryVariable`). Feature name "#name"; tooltip "#name description"; no preview. [FS queryVariable.fs]

| UI label | param | type | options | default | visible |
|---|---|---|---|---|---|
| Name | name | string (QUERY_VARIABLE_NAME, unconfigurable) | — | empty ("?" shown) | always |
| Description | description | string ≤256 | — | "" | always |
| Selection type | selectionType | enum SelectionType | Selection / Created by / Owned by / Protrusion / Pocket / Hole / Fillets / Bounded faces / Loop/chain connected / Parallel / Tangent connected / Matching / All solid bodies / Edge convexity / Adjacent | Selection | always |
| Entity type | seedType | enum | Face / Edge | Face | Tangent connected or Matching |
| Selections | selectionQuery | Query anything (mesh, flattened ok) | — | — | Selection |
| Created by features | createdByFeatures | FeatureList (flat sketches allowed) | — | — | Created by |
| Entities | seedBodies | Query bodies | — | — | Owned by, Edge convexity |
| Faces | seedFaces | Query faces | — | — | Protrusion/Pocket/Hole/Fillets/Bounded faces/(Tangent/Matching & Face) |
| Edges or faces | seedEdgesOrFaces | Query | — | — | Loop/chain connected |
| Edges | seedEdges | Query edges | — | — | Parallel/(Tangent/Matching & Edge) |
| Entity type | entityType | enum | Vertex / Edge / Face / Body | Vertex | Created by, Owned by |
| Fillet compare type | filletCompareType | enum | Equal / Less or equal / Greater or equal | Equal | Fillets |
| Bounds | boundedFacesBounds | Query edges/faces | — | — | Bounded faces |
| Edge convexity type | edgeConvexityType | enum | Convex / Concave / Smooth / Variable | Convex | Edge convexity |
| Filter construction entities | filterConstruction | bool | — | true | Created by |
| Filter by body type | filterByBodyType | bool | — | false | Created by |
| Body type | createdByBodyType | enum | Part / Surface / Curve / Point / Mate connector / Composite part | Part | filterByBodyType |
| Entities | seedQuery | Query vertex/face/edge (non-sketch) | — | — | Adjacent |
| Shared | adjacencyType | enum | Vertex / Edge | Vertex | Adjacent |
| Select | edgeAdjacentEntityType | enum | Edges and faces / Only edges / Only faces | Edges and faces | Shared=Edge |
| Select | vertexAdjacentEntityType | enum | Edges, faces, and vertices / Only edges / Only vertices / Only faces | Edges, faces, and vertices | Shared=Vertex |
| Add additional queries | addAdditionalQueries | bool → group | — | false | always |
| Additional queries | additionalQueries | array ("Add additional query"; item label "<op> of <selection type>"): Boolean operation (tabs Union / Subtract / Intersect) + same selection-type sub-form | — | [] | addAdditionalQueries |
| Evaluate on use | evaluateOnUse | bool | — | false | always |
| Show selection | showSelection | bool | — | true | always |

Behavior [DOCS]:
- Selected entities shown with yellow highlight + orange border; Created by + body filter: all created entities outlined orange, only matching ones yellow.
- Evaluate on use OFF = frozen to entities at definition point; ON = re-evaluated where used (e.g. "All solid bodies" picks up later bodies).
- Selection boxes in other features show a **Query variable icon** to pick a query variable as input (fields marked NO_QUERY_VARIABLE, e.g. MC origin, lack it). Mixed entity types allowed; empty allowed; first valid entry used where a single entity is required (e.g. sketch plane).
- Edit: double-click in Feature list; **Final** button previews downstream (unavailable if last before rollback bar).
- Criteria: Protrusion = faces connected by convex edges; Pocket = concave; Hole = faces of same round hole; Fillets = constant-radius fillet faces (compare by radius); Bounded faces = seed face region up to bounds; Loop/chain connected; Parallel edges; Tangent connected; Matching = same-size faces/edges.

### 4.3 Variable table (Part Studio)
Right-side panel (icon below View cube; shares the right-panel strip with Configuration panel, Appearance panel, Custom tables). [DOCS variable_table]
- Table columns: **Name | Type | Value | Description**. Bottom row "New variable name": type name + Enter/Tab → row created with type (default Length) and value. Name must be FS identifier. Types same as Variable feature plus **From import** (opens Variable dialog From import tab via pencil icon).
- Edit type via dropdown arrow; each edit saved immediately (works even if variable is used by features).
- Bottom dropdown: **Create Variable Studio** / **Insert Variable Studio** (other-document Variable Studios are read-only references and require a version; prompt to create one).
- Table ellipsis menu: Copy table; Switch to Variable Studio [x]; Open linked document; Update linked document (Reference manager); Move up / down; Edit reference (Update Variable Studio dialog; can pick a configuration); Remove reference.
- Row context menu: **Filter feature list** (show variable + features using it); Delete; Move up/down. Hover a row → Variable feature and dependent features highlight in Feature list.
- Precedence: duplicate names in one table rejected (alert). Same name in two tables → later table wins; overridden row shows override icon. Variable feature in Feature list with same name overrides table variable (override icon). Renaming in table updates all references.
- Variable features (from Feature list) appear in table only if their name is a valid identifier.

### 4.4 Configurations (Part Studio)
Terminology [DOCS configurations]: one **Configuration panel** per Part Studio/Assembly/Variable Studio; contains many **inputs** (each a collapsible table); rows = **options**; columns = **parameters** of **configured features** (top header = feature name, sub-header = parameter name); cells = **option values**. Each permutation of input options = one configuration. Each collaborator sees own selected configuration (Follow mode follows leader).

**Panel UI** [DOCS part_studio_and_assembly_configurations, managing_configurations][BUNDLE]:
- Open via Configuration panel icon on right side. Empty state text: "Add configuration input to create multiple variants of this Part Studio" / "Configurations drive different design variants of a Part Studio using sketch dimensions of parts and surfaces, feature parameters, or suppression states." Button **Configure Part Studio** (arrow beside it → **Paste inputs**).
- Header: Filter icon + "Filter inputs" field (search options `:name` default, `:part "Part 1"`, combinable; quotes = exact) + toggle buttons to show List / Checkbox / Configuration variable inputs; three-dot menu: **Copy all inputs**, **Collapse/Expand all inputs**; **Configured properties** link at top of panel.
- Bottom: **Add configuration input** button/arrow → menu: **List**, **Checkbox**, **Configuration variable** (bundle also has **Text** input type). Default list name "List input N". [BUNDLE]
- Input types:
  - **List**: first column = option names (click first row, type, Tab to next row; pasting multi-line creates multiple values: "Each line is saved as a new list value…Press ENTER when done"). Blue bar left of row = currently active option. Default option shown next to input name.
  - **Checkbox** ("Checkbox input" dialog: Name, Default checked/unchecked): table has 2 rows (unchecked, checked); typically configures Suppressed state. Can be converted: right-click → **Convert to expression** / **Convert to checkbox**.
  - **Configuration variable** ("Variable input" dialog: Name, type **Length / Angle / Integer / Real / Text**, value(s)): creates a real `#variableName`; apply via right-click on a feature field → **Set to #variableName** or typing `#name`. Usages listed in panel.
  - **Text**: text input (convertible to expression).
- **Configuring a feature parameter** (List/Checkbox): click **+ Configure features** → yellow banner mode ("Select sketches or features to configure", "Right click sketch dimensions, features, or feature parameters to configure them") → open a feature from Feature list, hover fields: configurable ones highlight yellow; click one → outlined with broken yellow line, new column added (default header "Extrude 1 / Depth"). Parameters already configured in another input are dashed-yellow and unavailable. Click **Done** in yellow banner to finish. Alternative: right-click a sketch dimension/feature/parameter → **Configure dimension** / **Configure dimension with <input>** / **Configure suppression** / **Configure suppression with <input>** / **Configure sketch text** / **Configure with <param>**; **Unconfigure** / **Unconfigure from <param>** / **Remove configured value**.
  - Non-configurable: array parameters, button parameters, params inside arrays, params with errors ("Errors must be resolved before a parameter can be configured"), and params marked UNCONFIGURABLE in FS (e.g. Variable name/type, MC origin entity). [BUNDLE][FS]
  - Edit numeric cell: double-click → type. Selection-type cell: double-click → feature dialog opens with that parameter highlighted blue; reselect; green check.
  - Configured feature parameters show yellow/configured styling in dialog; feature in list shows "Has configured parameters"/"Configured by <input>" tooltip. [BUNDLE]
  - Tolerance options icon per cell for dimensions (precision/type per option value).
  - **Dynamic suppression expressions**: right-click feature → Configure suppression → edit expression in panel.
- Input table three-dot menu: Copy table; Copy input; Move up/down (or drag); Rename; **Edit FeatureScript IDs** (dialog; "Break references and change IDs"); **Exclude from properties** / Include in properties; **Visibility conditions**; **Input propagation**; Delete (no warning).
- Row context menu: Switch to (option); **Set as default**; **Duplicate row**; Move up/down (or drag); Rename; Delete row. Column context menu: Rename (sketch features only), Delete column. Column edges draggable to resize.
- Rename input/column: hover name → I-cursor → click, type, Enter.
- Copy table → TSV with headers to clipboard; paste spreadsheet block into top-left cell: replaces values, adds rows on the fly, extra columns ignored, default units appended.
- Copy/paste inputs across tabs/documents (names, visibility conditions, FS IDs, variable definitions copied; no live link; clipboard until logout; duplicate FS IDs not pasted; invalid visibility conditions auto-removed).

**Configuration selector** [DOCS]: "Configurations" area at top-left above Feature list, one dropdown per input (checkbox for Checkbox inputs, value box for variable inputs); changing it regenerates model. Button **Manage visibility conditions** (opens dialog for all inputs) with dropdown → **Reset to default** (if default hidden by visibility, first visible option used). Insert dialogs (assemblies/drawings/derived) show same inputs + **Set to last viewed configuration** button (dropdown Reset to default) and **Generate**; assembly instances get configured icon; right-click instance → **Change configuration**; hover shows configuration tooltip. Derived feature shows configuration inputs inline (UIHint SHOW_INLINE_CONFIG_INPUTS). [FS]

**Configured properties** [DOCS configured_properties]: panel top link "Configured properties" → "Properties for" dropdown (parts/appearances; configured items checked) → **Add property** (part properties, appearance, custom properties) → table with one column per property, rows per option combination of all (non-excluded) inputs; double-click cell to edit (Tab to next row). Part number column context: **Generate next part number** (if numbering scheme). Read-only properties lock table structure. **Exclude from properties**: input treated as default for properties/BOM/release (parts differing only in excluded inputs = same part); still affects geometry, computed properties, Mass properties; icon "Excluded from affecting configured properties".

**Input propagation** [DOCS]: three-dot → Input propagation dialog lists references (e.g. derived/inserted configured elements) sharing parameters with this tab's inputs; check inputs to propagate → Confirm; propagation shown next to reference in panel.

**Visibility conditions** [DOCS visibility_conditions]: input three-dot → "Configuration visibility conditions" dialog (left: conditions; right: live preview with selectable options, "Show hidden inputs" checkbox; "Inputs" dropdown incl. All inputs).
- **Add condition**: [controlling input] [is | is between] [option(s) | first..last]. Multiple conditions AND together. Only inputs **above** can control inputs below. Save → input shows visibility icon.
- **Add option rule** (List inputs only): Show [Options | Range] When [Always | When <input> is/is between …]; conditions within a rule AND; overlapping ranges allowed.
- Hidden inputs use default value; input with no visible options is hidden. Excluded-from-properties inputs can control only other excluded inputs. Auto-repair removes conditions referencing deleted inputs/options (warning). Summaries shown when collapsed. Conditions apply in all Insert dialogs; hidden inputs grayed in assembly panels.

**Custom tables** [DOCS custom_tables]: right-panel Custom tables icon; add from a Feature Studio version (own, enterprise, or "Custom table samples"); columns: right-click Move left / Move right / Remove; overflow menu: Copy table, Show removed columns. (FS custom-table authoring = Feature Studio section.)

### 4.5 Variable Studios [DOCS VariableStudio__*]
- Created via Insert new tab (+) → **Create Variable Studio**. Table: Name (alphanumeric) | Type (default Length; Length/Angle/Number/Any/From import) | Value | Description. Tab moves to next row; drag or right-click Move up/down to reorder; later variables may reference earlier ones. Caret collapses; three-dot → Collapse/Expand all tables.
- Bottom-edge checkbox **Insert into all Part Studios and Assemblies** (auto-reference in workspace at default configuration; uncheck while drafting). Configured Variable Studios cannot be auto-referenced or released.
- **Insert Variable Studio** button (top of a Variable Studio, or from Variable table): dialog tabs Current document / Other documents; pick whole studio or individual variables; other documents need a version; inserted table header shows "Variable Studio 1 V1".
- Release: right-click tab → Release.
- Configurations: same panel ("Configure Variable Studio"); right-click a variable value or description → **Configure** / **Configure with** / **Set to…** / **Unconfigure** / **Unconfigure from**; configured cells highlighted orange. Insert dialog → choose options → Generate → click studio or individual variables. To drive by a Part Studio configuration: Variable table three-dot → Edit reference → right-click configuration → Configure → becomes a column in the Part Studio Configuration panel.

---

## 5. Surface tools

Surface-feature common: "Result body operation type" tabs **New | Add** (enum NewSurfaceOperationType; default New) [FS surfaceOperationTypePredicate]; when Add → **Merge with all** (default **true**) else **Merge scope** (sheet bodies) [FS surfaceJoinStepScopePredicate]. Surfaces appear in the Parts list "Surfaces" section. Toolbar surface group (docs "Surfacing tools collection"): Thicken, Enclose, Fillet, Face blend, Delete face, Move face, Replace face, Offset surface, Boundary surface, Fill, Move boundary, Ruled surface, Mutual trim, Constrained surface. [DOCS]

### 5.1 Boundary surface
Editing logic `bsurfEditLogic`. Icon shared with Loft-like glyph. [FS bsurf.fs]

| UI label | param | type | options | default | visible |
|---|---|---|---|---|---|
| Result body operation type | surfaceOperationType | tabs | New / Add | New | always |
| U profiles | uProfilesArray | array (collapsed items; item label "[condition] entities"; max 2 profiles) | — | [] | always |
| ↳ Edges, curves and sketches | uProfileEntities | Query edges / non-sketch wire bodies (no mesh, no construction) | — | — | per item |
| ↳ Boundary condition | uCondition | enum BSurfEndDerivativeType | None / Normal to profile / Tangent to profile / Match tangent / Match curvature / Normal direction / Tangent direction | None | per item |
| ↳ Magnitude | uMagnitude | real [-1e5..1e5] | — | 1 | condition != None |
| ↳ Faces | uReference | Query faces | — | — | Match tangent / Match curvature |
| ↳ Direction | uDirection | Query direction or MC, max 1 | — | — | Normal direction / Tangent direction |
| V profiles | vProfilesArray | array, same sub-fields (v*) | — | [] | always |
| Show isocurves | showIsocurves | bool | — | false | always |
| Count | curveCount | int [1..50] | — | 10 | showIsocurves |
| Merge with all / Merge scope | defaultSurfaceScope / booleanSurfaceScope | surface merge block | — | true | Add |

- U and V = two directions of a four-sided surface; profiles added by selecting in viewport (each selection creates an array item; dropdown arrow on item to add more entities). Magnitude ±. None = position match by shortest distance. [DOCS]

### 5.2 Fill
Editing logic `fillEditLogic`. [FS fillSurface.fs]

| UI label | param | type | options | default | visible |
|---|---|---|---|---|---|
| (Preselection) | preselectedEntities | hidden | — | — | hidden |
| Result body operation type | surfaceOperationType | tabs | New / Add | New | always |
| Edges | edges | array (selecting edges/wire bodies adds items; label "[continuity] entities"; +Create selection) | — | [] | always |
| ↳ Continuity | continuity | enum GeometricContinuity (MATCH_LAST_ARRAY_ITEM: new item copies previous) | Position / Tangency / Curvature | Position | per item |
| ↳ Adjacent faces | adjacentFaces | Query faces | — | — | continuity != Position |
| Guides | addGuides | bool | — | false | always |
| Vertices or curves | guideEntities | Query vertices/edges/wire bodies | — | — | addGuides |
| Mode | constraintMode | enum FillConstraintMode | Precise / Sampled | Precise | addGuides |
| Sample size | sampleSize | int [1..50] | — | 5 | Sampled |
| Show isocurves | showIsocurves | bool | — | false | always |
| Count | curveCount | int [1..50] | — | 10 | showIsocurves |
| Merge with all / Merge scope | | surface merge block | | true | Add |

- Red dots on selected boundary = gaps/open chain. Closed result with Add → automatically becomes solid part; use New to keep surfaces. Precise guides must touch, not cross, boundary; intersecting guides must be tangent-compatible. Sampled tolerates more. Isocurves show untrimmed underlying surface grid. [DOCS]

### 5.3 Move boundary
Manipulator `extendManipulatorChange` (drag arrow on boundary). [FS extend.fs]

| UI label | param | type | options | default | visible |
|---|---|---|---|---|---|
| Surface or boundary edges | entities | Query sheet bodies or one-sided (laminar) edges | — | — | always |
| Tangent propagation | tangentPropagation | bool | — | true | always |
| Move end condition | endCondition | enum ExtendBoundingType | Blind / Up to face / Up to part/surface / Up to vertex | Blind | always |
| Opposite direction | oppositeDirection | bool flip | — | false | Blind |
| Distance | extendDistance | length (negative = trim) | — | 25 mm | Blind |
| Target | targetPart / targetFace / targetVertex | Query body / face or MC / vertex, max 1 | — | — | per up-to type |
| Offset distance | hasOffset | bool (short) | — | false | Up to … |
| Offset distance | offset | length | — | 25 mm | hasOffset |
| Opposite direction | offsetOppositeDirection | bool flip | — | false | hasOffset |
| Maintain curvature | maintainCurvature | bool | — | false | always |

### 5.4 Ruled surface
Manipulator `ruledSurfaceManipulator`; editing logic `ruledSurfaceEditingLogic` (auto-adds an adjacent face as reference face on first edge pick). [FS ruledSurface.fs]

| UI label | param | type | options | default | visible |
|---|---|---|---|---|---|
| Result body operation type | surfaceOperationType | tabs | New / Add | New | always |
| Edges and curves for ruled surface path | edges | Query edges / wire bodies (non-construction) | — | — | always |
| Ruled surface type | ruledType | enum RuledSurfaceInterfaceType | Normal / Tangent / Aligned with direction / Angle from direction | Normal | always |
| Direction | axis | Query direction, max 1 | — | — | Aligned / Angle from direction |
| Angle | angleFromVector | angle | — | 90 deg (Defaults map 0) | Angle from direction |
| Opposite direction | oppositeAngleFromVector | bool circular flip | — | false | Angle from direction |
| Angle | angle | angle | — | 0 | Normal / Tangent |
| Opposite direction | oppositeAngle | bool circular flip | — | false | Normal / Tangent |
| Distance | distance | length | — | 25 mm | always |
| Opposite direction | oppositeDirection | bool flip | — | false | always |
| Reference faces | referenceFaces | Query faces | — | auto adjacent face | Normal / Tangent |
| Vertex overrides | vertexOverrides | array (label "#vertex") | — | [] | always |
| ↳ Vertex | vertex | Query vertex, max 1 | — | — | item |
| ↳ Ruled surface type | overrideType | enum VertexOverrideType (copies last item) | Normal / Tangent / Aligned with direction / Up to vertex | Normal | item |
| ↳ Angle / Opposite direction | angleOverride / oppositeAngleOverride | angle / flip | — | 0 | Normal/Tangent |
| ↳ Direction | axisOverride | Query direction | — | — | Aligned |
| ↳ Up to vertex or mate connector | boundaryOverride | Query vertex/MC | — | — | Up to vertex |
| ↳ Distance / Opposite direction | distanceOverride / oppositeDirectionOverride | length / flip | — | 25 mm | type != Up to vertex |
| Use cubic interpolation | useCubicInterpolation | bool | — | true | always |
| Show ruled lines | showRuledLines | bool | — | false | always |
| Count | ruledLineCount | int [1..100] | — | 40 | showRuledLines |
| Corner type | cornerType | enum RuledSurfaceCornerType | Spun corner / Extended corner / Lofted corner / No corner | Spun corner | always |
| Merge with all / Merge scope | | surface merge block | | true | Add |

- Invalid direction shown red in viewport; self-intersecting surfaces → error (feature red in list). Docs say linear interpolation is default but FS default is cubic = true. [DOCS][FS]

### 5.5 Mutual trim
Manipulator `mutualTrimManipulatorChange` (click arrowheads in viewport to flip a set). [FS]

| UI label | param | type | default |
|---|---|---|---|
| First set of surfaces | body1 | Query sheet bodies (modifiable) | — |
| Keep opposite side | keepOtherSide1 | bool flip | false |
| Second set of surfaces | body2 | Query sheet bodies | — |
| Keep opposite side | keepOtherSide2 | bool flip | false |
| Merge | merge | bool (REMEMBER_PREVIOUS_VALUE) | true |

- Extends intersections as needed; Merge = single resulting surface, else keep separate trimmed surfaces. [DOCS]

### 5.6 Constrained surface
[FS constrainedSurface.fs]

| UI label | param | type | options | default | visible |
|---|---|---|---|---|---|
| Type | csType | enum tabs | Points / Mesh | Points | always |
| Vertices | vertices | array (collapsed, no reorder; label "#vertex") | — | [] | Points |
| ↳ Vertex | vertex | Query vertex, max 1 | — | — | item |
| ↳ Normal | normal | Query direction or MC, max 1 | — | (surface normal to vertex) | item |
| ↳ Flip normal | flipNormal | bool flip | — | false | item |
| Mesh bodies or faces | meshes | Query mesh faces/bodies | — | — | Mesh |
| Tolerance | tolerance | length [1e-8..1 m] | — | 0.01 mm | always |
| Optimize | optimize | enum OptimizationMethod | Performance / Smoothness | Performance | always |
| Deviation | computeDeviation | bool → group | — | false | always |
| ↳ Maximum deviation | maxDeviation | read-only length | — | — | computeDeviation |
| ↳ Show deviation | showDeviation | bool → group | — | false | computeDeviation |
| ↳ Deviation type | deviationType | enum | Max deviation / All deviations | Max deviation | showDeviation |
| ↳ Limit | limit | length ≥0 (deviations below not displayed) | — | 0 | showDeviation |

- Larger tolerance = simpler surface; Smoothness = more control points, slower, lower curvature. Deviations drawn as red lines. [DOCS]

### 5.7 Surface modeling overview & mixed modeling [DOCS surface_modeling, mixed_modeling]
- Surfaces = zero-thickness sheet bodies; become solids via **Thicken** or **Enclose**; used for construction (split tools, Replace face targets).
- Continuity terms used across curve/surface dialogs: G0 Match position, G1 Match tangent, G2 Match curvature, G3 Match flow (control points needed: 1/2/3/4 per end).
- Analysis tools (View section): Curve/surface analysis (**Shift+C**; curvature combs, control point grids), Connection analysis, Zebra stripes, Curvature color map (Gaussian etc.), Dihedral analysis (**Shift+D**), Deviation analysis, Reflection analysis; View options: Highlight boundary edges (open edges red), View in high quality (**Shift+R**), Face tangent angle in Measure.
- Best-practice guidance (not UI): prefer Boundary surface over Loft/Sweep/Fill for control; Beziers over splines (degree 3/5/7); avoid building from trimmed/projected edges (overbuild + Mutual trim/Split); four-sided surfaces.
- **Mixed modeling**: imported meshes (STL, OBJ, GLTF, 3MF, Parasolid mesh/mixed x_t+xmm_txt / x_b+xmm_bin zips) coexist with B-rep. Mesh-capable features (FS `AllowMeshGeometry.YES`): Delete face, Rib, Shell, Enclose, Boolean, Split, Offset surface, Move face, Hole, Mate connector, in-context modeling, plus pattern/transform/mirror entity inputs. Parts list icons distinguish mesh part / mesh surface / mixed part. Meshes are view/reference only (not editable). Export mixed models as STL or Parasolid (zip x_t + xmm_txt).

---

## 6. Curve tools

Common: outputs are wire bodies listed under **Parts list → Curves**; curve features have no eye toggle in Feature list (hide via Curves list). Most curve features have UIHint NO_PREVIEW_PROVIDED (result shown directly, no preview slider). **→ Curve approximation block** (see §0): when Approximate is on, original curve drawn **orange**, approximated curve **magenta**. [FS][DOCS]

### 6.1 3D fit spline
Manipulator `fitSplineManipulatorChange` (drag start/end magnitude arrows); editing logic `fitSplineEditLogic`. [FS]

| UI label | param | type | options | default | visible |
|---|---|---|---|---|---|
| Approximation type | fitType | enum tabs FitSplineType | Vertices / Edges | Vertices | always |
| Vertices | vertices | Query vertices, ordered (ALLOW_QUERY_ORDER) | — | — | Vertices |
| Closed spline | closed | bool | — | false | Vertices |
| Start direction | startDirection | Query edge/direction/MC, max 1 | — | — | Vertices & !closed |
| Start magnitude | startMagnitude | real | — | 1 | same |
| Opposite direction | oppositeDirectionStart | bool flip | — | false | same |
| Match curvature at start | matchStartCurvature | bool | — | false | same |
| End direction / End magnitude / Opposite direction / Match curvature at end | endDirection / endMagnitude / oppositeDirectionEnd / matchEndCurvature | as start | — | —/1/false/false | same |
| Edges | edges | Query edges (tangent-continuous) | — | — | Edges |
| Approximate … | → approximation block | | | false | always |

### 6.2 Projected curve
Manipulator `projectedCurveManipulatorChange`; editing logic `projectCurvesEditLogic`. [FS]

| UI label | param | type | options | default | visible |
|---|---|---|---|---|---|
| Curve projection type | curveProjectionType | enum tabs (REMEMBER_PREVIOUS_VALUE) | Two sketches / Curve to face | Two sketches (then last used) | always |
| First sketch | sketchEdges1 | Query sketch edges (non-construction) | — | — | Two sketches |
| Second sketch | sketchEdges2 | Query sketch edges | — | — | Two sketches |
| Edges | dropTools | Query edges | — | — | Curve to face |
| Projection direction type | projectionType | enum | Direction / Normal to target | Direction | Curve to face |
| Direction | directionQuery | Query direction, max 1 | — | — | Direction |
| Opposite direction | oppositeDirection | bool flip | — | false | Direction |
| Targets | targets | Query faces / solid or sheet bodies | — | — | Curve to face |
| Approximate … | approximation block | | | false | always |

- Two sketches: extrusions of both sketches must intersect in one contiguous curve. [DOCS]

### 6.3 Bridging curve
Editing logic `bridgingCurveEditingLogic`; manipulator `bridgingCurveManipulator` (magnitude arrows, bias circle, control-point arrows, edge-position handles). [FS]

| UI label | param | type | options | default | visible |
|---|---|---|---|---|---|
| Start | side1 | Query edge/vertex/face/MC, max 2 (vertex + edge/face) | — | — | always |
| Match | match1 | enum BridgingCurveMatchType (remembered) | Match position / Match tangent / Match curvature / Match flow (G3) | Match tangent | always |
| Opposite direction | flip1 | bool flip | — | false | match1 != position |
| End | side2 | Query, max 2 | — | — | always |
| Match | match2 | enum (remembered) | as above | Match tangent | always |
| Opposite direction | flip2 | bool flip | — | false | match2 != position |
| Method | method | enum BridgingCurveMethod (remembered) | Magnitude/bias / Control points | Control points (annotation) | any side != position and neither is G3 |
| Edit control points | editControlPoints | bool (remembered) → group | — | false | method = Control points (or any G3) |
| ↳ Start magnitude | side1Magnitude | real [0..1e5] | — | 1 | match1 != position |
| ↳ Start curvature offset | side1CurvatureOffset | real | — | 1 | match1 curvature/G3 |
| ↳ Start flow offset | side1G3Offset | real | — | 1 | match1 G3 |
| ↳ End magnitude / End curvature offset / End flow offset | side2* | real | — | 1 | per match2 |
| Magnitude | magnitude | real >0 | — | 1 | Magnitude/bias & any non-position |
| Bias | bias | real [0.0001..0.9999] | — | 0.5 | Magnitude/bias & both non-position |
| Edit edge positions | editEdgePositions | bool (remembered) → group | — | false | a side lacks a vertex (edge picked) |
| ↳ Start edge position / End edge position | startEdgeParameter / endEdgeParameter | real [0..1] | — | 0.5 | side without vertex |
| Edit tangency angles | editTangencyAngles | bool (remembered) → group "Edit angles" | — | false | a side has a face & any non-position |
| ↳ Start angle / End angle | startAngle / endAngle | angle [-180..180] | — | 0 | side with face, non-position |

- Rules [DOCS]: Match position needs no edge; vertex with single edge needs no edge; vertex with multiple edges needs an edge; edge without vertex → end inferred from other side. MC: tangent along MC Z. Magnitude → 0 straighter; Bias weights tangency toward one side.

### 6.4 Composite curve
Fields: **Edges** (`edges`, Query edges / non-sketch wire bodies) + approximation block. Adjacent edges must meet at vertices, not intersect/overlap; non-contiguous selections create multiple curves. Use as loft profile by picking from Curves list. [FS][DOCS]

### 6.5 Intersection curve
Fields: **Group 1** (`group1`, faces / solid or sheet bodies), **Group 2** (`group2`), approximation block. Only Group1×Group2 intersections create curves (multiple curves possible). Pattern note: to pattern along surface use feature pattern incl. intersecting plane + Reapply. [FS][DOCS]

### 6.6 Trim curve
Manipulator `extendCurveManipulatorChange`; no preview. [FS moveCurveBoundary.fs]

| UI label | param | type | options | default | visible |
|---|---|---|---|---|---|
| Move curve boundary type | moveBoundaryType | enum tabs (remembered) | Trim / Extend | Trim | always |
| Curves to adjust | wires | Query non-sketch wire bodies | — | — | always |
| End condition | endCondition | enum (remembered) | Blind / Up to entity | Blind | Extend |
| Distance | extensionDistance | length >0 | — | 25 mm | Extend & Blind |
| Up to entity | extendTo | Query body/face/edge/vertex/MC | — | — | Extend & Up to entity |
| Extension shape | extensionShape | enum (remembered) | Curvature / Tangent | Curvature | Extend |
| Up to entity | trimTo | Query body/face/edge/vertex/MC (implicit MC ok; MC need not intersect) | — | — | Trim |
| Help point | helpPoint | Query vertex/MC, max 1 | — | — | always |
| Opposite direction | flipHeuristics | bool flip | — | false | always |

- Help point: trim keeps/removes nearest section; extend picks nearest end. Sketch entities must first be converted to composite curves. [DOCS]

### 6.7 Isocline
Fields: **Faces** (Query faces), **Direction** (Query direction, max 1), **Opposite direction** (flip, false), **Angle** (angle [0..89.9], default 3 deg), **Split faces** (bool, false → splits faces along isocline instead of only curve). [FS][DOCS]

### 6.8 Offset curve
Manipulator `offsetCurveOnFaceManipulators` (distance + start/end trim arrows). [FS offsetCurveOnFace.fs]

| UI label | param | type | options | default | visible |
|---|---|---|---|---|---|
| Edges | edges | Query model edges (not sketch/wire) | — | — | always |
| Distance | distance | length >0 | — | 25 mm | always |
| Offset type | offsetType | enum | Geodesic / Euclidean | Geodesic | always |
| Flip | oppositeDirection | bool flip | — | false | always |
| Gap fill | gapFill | enum | Linear / Round | Linear | always |
| Scope | scope | enum | Offset / Offset and extend / Offset, extend, and split | Offset | always |
| Trim | trim | bool → group "Trim control" | — | false | Offset and extend |
| ↳ Start trim | startTrim | length ≥0 | — | 0 | trim |
| ↳ Equal trim | equalTrim | bool | — | false | trim |
| ↳ End trim | endTrim | length ≥0 | — | 0 | trim & !equalTrim |
| Targets | targets | Query faces (empty = all adjacent faces) | — | — | always |
| Approximate … | approximation block | | | false | scope != split |

- Geodesic = distance in face 2D space; Euclidean = 3D. Split scope splits faces, no curve output. Branched edge selections error. [DOCS]

### 6.9 Isoparametric curve
| UI label | param | type | options | default | visible |
|---|---|---|---|---|---|
| Select face | face | Query face, max 1 | — | — | always |
| Direction | directionType | enum | U direction / V direction | U direction | always |
| Equal spacing | equalSpacing | bool | — | false | always |
| Instance count | nCurves | int [2..50] | — | 10 | equalSpacing |
| Position | uvParamList | array ("Add Curve"; X removes, CLEAR all) | — | [] | !equalSpacing |
| ↳ Select point | selectPoint | bool | — | false | item |
| ↳ Select point | point | Query vertex/MC, max 1 | — | — | selectPoint |
| ↳ Location | uvValue | real [0..1] | — | 0.5 | !selectPoint |

- Numeric fields here: scroll wheel ±1.0, Ctrl+scroll ±0.01, Shift+scroll no effect (differs from other dialogs). [DOCS]

### 6.10 Edit curve
Editing logic `editCurveEditLogic`; manipulator `onEditCurveManipulatorChange` (triad on selected control point: 3 axis translations + 3 plane translations). Description: "Edits a curve directly if possible or a composite curve of the selection otherwise." [FS]

| UI label | param | type | options | default | visible |
|---|---|---|---|---|---|
| Curves | wire | Query edges / wire bodies (continuous, non-self-intersecting; multiple → must Approximate) | — | — | always |
| Approximate → Target degree, Maximum control points, Tolerance, Keep start derivative, Keep end derivative | approximate, approximation* | bool + group | — | false | always |
| Elevate → Target degree | elevate, elevationDegree | bool + int [2..max] | — | false / 3 | always |
| Planarize → Reference plane | planarize, planeReference | bool + enum | Best fit / YZ plane / XZ plane / XY plane / Custom | false / Best fit | always |
| ↳ Entity or mate connector | referencePlane | Query plane/MC | — | — | Custom |
| ↳ Lock ends | lockEnds | bool | — | false | Best fit |
| Edit control points | editControlPoints | bool → group | — | false | always |
| ↳ Edit mode | editPointMode | enum | XYZ / UVN (Tangent) / UVN (Control polygon) | XYZ | edit CPs |
| ↳ Control point indices | selectedIndices | array of index (XYZ mode) | — | [] | XYZ |
| ↳ Control point index | selectedIndex | int | — | 0 | UVN modes |
| ↳ Points overrides | controlPointEdits | array (label "index: x;y;z", collapsed; X clears, CLEAR all; "Add point override") | — | [] | edit CPs |
| ↳↳ Control point index / Reference (vertex/MC) / X offset / Y offset / Z offset / Weight | index, reference, x, y, z, weight | int / Query / length ×3 / real [1e-5..1e5] | — | —/—/0/0/0/1 | item |
| Show details → Degree, Control points, Spans (read-only) | showDetails | bool | — | true (annotation) | always |
| Show deviation → Maximum deviation | approximationShowDeviation | bool | — | false | always |

- Order of operations fixed: Approximate → Elevate → Planarize → Edit control points (use two features to reverse). Dragging triad on a CP auto-adds an override row. Weight >1 pulls curve toward CP. Downstream features stay associated. [DOCS]

### 6.11 Routing curve
Feature name template "Routing curve (<length>)"; manipulator `routingCurveManipulator` (triad: 3 translations, 3 rotations, 3 plane drags, origin; dragging shows blue line + inline input box); editing logic `routingCurveEditLogic`. [FS][DOCS]

| UI label | param | type | options | default | visible |
|---|---|---|---|---|---|
| Curve type | curveType | enum | Interpolated spline / Polyline | Interpolated spline | always |
| Bend radius | bendRadius | length ≥0 | — | 0 | Polyline |
| Target length | hasTargetLength + targetLength | bool + length (short) | — | false / 25 mm | Interpolated spline |
| Step | step | enum tabs (unconfigurable) | Points / Segments | Points | always |
| **Points step:** Input type | inputType | enum | Vertex / Curve / CSV file | Vertex | Points |
| ↳ Vertices / Edges + Number of samples / Table (CSV) + Data start column (0,1,2,.) + Unit (Meter default; cm/ft/in/mm/m/yd) ; Overwrite points (true); **Process** button | inputVertices, inputCurve, …, inputData, csvUnit | | | | per input type |
| Closed | isPeriodic | bool | — | false | Points |
| Curve reference | curveBaseCSys | Query vertex/MC ("Defaults to origin") | — | Origin | Points |
| Point edit type | pointsEditType | enum tabs | Single / Multiple | Single | Points |
| Points | points array ("Point #index"; CLEAR; x deletes) | Reference (Origin / Vertex / Curve / Relative), Vertex or Edges+Parameter (+Align/Flip coordinate system), X/Y/Z offset, Override bend radius, Derivative (spline only; Direction option) | — | — | Single |
| Reset triad | resetTriad | button | — | — | Points |
| Add point on axis drag | addPointOnAxisDrag | bool | — | false | Single |
| Show reference coordinate system | showCSys | bool | — | true | Points |
| Keep points | keepPoints | bool ("Create points in addition to the curve") | — | false | Points |
| **Segments step:** Segment edit type | segmentEditType | enum | Add points / Orthogonal path | Add points | Segments |
| ↳ Number of points to add | | int | — | — | Add points |
| ↳ Coordinate system | orthoCsys | enum | World / Curve / Other | World | Orthogonal path |
| ↳ Orthogonal path coordinate system | orthoCsysMate | Query vertex/MC | — | — | Other |
| ↳ Previous / Next / Confirm | buttons cycling orthogonal solutions | | | | Orthogonal path |

- Segments mode shows magenta segment lines with midpoint manipulators; selected non-orthogonal segment turns green while cycling options. Ctrl/Cmd+Z undoes last step inside dialog. Relative reference not allowed for first point. [DOCS]

---

## 7. Frames (weldments)

Toolbar frame group: Frame, Frame trim, Gusset, End cap, Cut list (+ Tag for profiles). Bundle action IDs `createFeatureFrame`, `createFeatureFrameTrim`, `createFeatureGusset`, `createFeatureEndCap`, `createFeatureCutList`. [BUNDLE]

### 7.1 Frame
Manipulator `frameManipulators` (angle drag; 9-point alignment selectors on profile); editing logic `frameEditLogicFunction`. [FS frame.fs]

| UI label | param | type | options | default | visible |
|---|---|---|---|---|---|
| Sketch profile | profileSketch | PartStudioData (sketch; REMEMBER_PREVIOUS_VALUE; library def) → "Select sketch" dialog | — | last used | always |
| Selections | selections | Query faces / edges / vertices / non-sketch wire bodies ("Faces, edges, and vertices that define sweep paths") | — | — | always |
| Lock profile faces | lockProfile | bool → group | — | false | always |
| ↳ Faces to lock | profileLockFaces | Query faces | — | — | lockProfile |
| Merge tangent segments | mergeTangentSegments | bool | — | true | always |
| Angle | angle | angle | — | 0 | always |
| Mirror across Y axis | mirrorProfile | bool flip | — | false | always |
| Angle reference | angleReference | Query direction or MC, max 1 | — | (normal plane) | always |
| Default corner type | defaultCornerType | enum FrameCornerType | Miter / Butt / Coped butt / None / Tangent | Miter | always |
| Flip corner | defaultButtFlip | bool flip | — | false | Butt / Coped butt |
| Corner overrides | cornerOverrides | array (pick vertex in viewport; label "vertex [cornerType]") | — | [] | always |
| ↳ Corner type / Flip corner | cornerType / cornerButtFlip | enum / flip | as above | — | item |
| Limit frame ends | trim | bool → group "Trimming" | — | false | always |
| ↳ Faces to trim to | trimPlanes | Query planar faces/planes | — | — | trim |
| ↳ Parts to trim to | trimBodies | Query solids | — | — | trim |
| (Index) | index | int hidden — chosen alignment point of 9-point grid | — | center | hidden |

Behavior [DOCS]:
- Select sketch dialog tabs: **Current document** / **Other documents** (profile must be in a version; prompt to create) / **Libraries** (Onshape frame profiles library + company/user libraries; Standard / Type / Size / Configuration selectors; **Generate** after changing size/configuration); **Done** closes.
- Path must be contiguous; faces use their edges. Profile centered on path by default; click one of the alignment points shown on profile (9-point grid + Tag-defined points) to realign.
- Merge tangent segments → tangent segments become one composite part. Butt / Coped butt show flip arrow at corner. Corner types: Miter, Butt, Coped butt, None (open).
- Frame profile libraries are folders (Standard/Type/Profile hierarchy, "Library label" for deeper levels); Documents page: Set library > My frame profiles / Frame profile library > Update my library / Unset my library (document management, see other section).

### 7.2 Frame trim
Manipulator `frameTrimManipulatorChange`. [FS]

| UI label | param | type | options | default | visible |
|---|---|---|---|---|---|
| Trim Type | trimType | enum tabs (remembered) | Ordered groups / Face | Ordered groups | always |
| Frame groups | frameGroups | array (collapsed; "Frame segments in earlier groups are used as tools for later groups") → Frames (solids) | — | [] | Ordered groups |
| Parts to trim | targets | Query solids | — | — | Face |
| Face to trim to | tool | Query face / sheet / construction plane / MC, max 1 | — | — | Face |
| Opposite side | flipTrim | bool flip | — | false | Face |
| Trim to face boundaries | useToolExtents | bool | — | false | Face |

### 7.3 Gusset
Manipulator `gussetManipulatorChange` (length/thickness arrows); editing logic `gussetEditLogic`. Many values remembered. [FS]

| UI label | param | type | options | default | visible |
|---|---|---|---|---|---|
| Edges | edges | Query linear solid edges (gusset bases, on segment cut faces) | — | — | always |
| Gusset type | gussetType | enum (remembered) | Triangle / Rectangle | Triangle | always |
| Length | length | length (remembered) | — | 25 mm | always |
| Flip base sides | shouldFlipBaseSides | bool flip | — | false | Rectangle |
| Thickness | thickness | length (remembered) | — | 25 mm | always |
| Chamfer | chamfer | bool (remembered, short) | — | false | Rectangle |
| Chamfer size | chamferSize | length (remembered) | — | 1 mm | chamfer |
| Gusset position | gussetPosition | enum (remembered) | Centered / Aligned | Centered | always |
| Alignment entity | alignedReference | Query linear solid edge, max 1 | — | — | Aligned |
| Opposite alignment | shouldFlipAlignment | bool flip | — | false | Aligned |
| Offset distance | offset | length ≥0 (remembered) | — | 0 | always |
| Opposite direction | shouldFlipOffset | bool flip | — | false | always |

- Gussets are individual parts. [DOCS]

### 7.4 End cap
Manipulator `manipulatorChange`. [FS endcap.fs]

| UI label | param | type | options | default | visible |
|---|---|---|---|---|---|
| Faces | faces | Query planar faces (segment ends) | — | — | always |
| Profile type | profileType | enum | Match profile / Rectangle / Circle / Internal | Match profile | always |
| Thickness | thickness | length [1 mm..] | — | 5 mm | always |
| Opposite direction | thicknessDirection | bool flip | — | false | type != Internal |
| Offset | offsetDistance | length ≥0 (remembered) | — | 0 | type != Internal |
| Opposite direction | offsetDirection | bool flip | — | false | type != Internal |
| Offset | internalOffsetDistance | length ≥0 (remembered) | — | 0 | Internal |
| Inset | internalShiftDistance | length ≥0 (remembered) | — | 0 | Internal |
| Treatment type | cornersType | enum (remembered) | None / Chamfer / Fillet | None | Rectangle |
| Length | cornerDistance | length | — | 5 mm | Chamfer |
| Radius | cornerRadius | length | — | 5 mm | Fillet |

### 7.5 Cut list
Editing logic `cutlistEditLogic`. [FS cutlist.fs]

| UI label | param | type | default | visible |
|---|---|---|---|---|
| Select all frames | selectAllFrames | bool (includes gussets & end caps) | false | always |
| Frames | frames | Query solids/composites | — | !selectAllFrames |
| Overrides | columnOverrides | array ("Add Column Override"; label "column = value") | [] | always |
| ↳ Same frames as previous | sameFramesAsPrevious | bool | true | items after first |
| ↳ All frames | overrideAllFrames | bool | false | first item or !same |
| ↳ Frames | overrideFrames | Query | — | !All frames |
| ↳ Column | column | string (existing or new column) | — | item |
| ↳ Value | value | string | — | item |

- Creates an **open composite part** of the segments + a **Cut list table** in the right-side table panel (Cut list icon). Default columns: **Item, Qty, Standard, Description, Length, Angle 1, Angle 2** (+ Tag additional columns); Item/Qty/Length/Angle 1/Angle 2 not overridable; frames not in an override show "Not set". Gussets/end caps noted in Description. Table: right-click column → Move left / Move right / Hide column; three-dot → Copy table / Show <column>. Insert the composite (not individual frames) into assemblies for correct BOM. [DOCS][FS frameUtils.fs]

---

## 8. Sheet metal (summary)

Concept [DOCS sheet_metal_model]: **Sheet metal model** activates an SM model; while active, every operation is mirrored live in a **flat pattern** and a **Sheet metal table** (bends/rips/joints); folded, flat, and table views update simultaneously. Features piercing an active SM model cut perpendicular to walls. **Finish sheet metal model** deactivates (treat as solid; later changes don't affect flat/table). Most sheet-metal-only tools are enabled only while an active model exists. Model parameters use a "Parameter Library Purpose" (company SM material/gauge libraries). Patterns/Mirror/Transform work on active SM (Face pattern / Reapply). Bundle action IDs §0. [FS][DOCS]

Shared bend fields [FS sheetMetalBendTopPredicate/BottomPredicate]: Bend line (line, max 1); Hold opposite side (flip); Sheet metal face to bend (planar SM face); Bend alignment (Bend line / Hold line / Hold other line / Inner / Outer / Middle; default Bend line); Angle control type (Bend angle / Align to geometry / Angle from direction); Opposite angle; … ; Use model bend radius (default true) → Bend radius; Use model K Factor (true) → K Factor.

| Tool | Purpose | Main fields (FS order; defaults) |
|---|---|---|
| **Sheet metal model** | Create SM from parts, sketch curves, or faces | Process tabs **Convert / Extrude / Thicken** (default Convert). Group *Selections*: Convert → Parts and surfaces to convert, Faces to exclude; Extrude → Sketch curves to extrude, Arcs to extrude as bends, End type (Blind / Up to next / Up to face / Up to part / Up to vertex), Opposite direction, Symmetric, Second end position…; Thicken → Faces or sketch regions to thicken, Tangent propagation (false). Convert/Thicken → Edges or cylinders to bend (unselected edges become rips; arcs/splines → tangent joints), Clearance from input (0), Include bends (false); Convert → Keep input parts (false). Group *General*: Thickness (1.6 mm, remembered), Opposite direction, Bend radius (2.3 mm, remembered), Flip direction up (remembered). Group *Material* (collapsed): Bend calculation (K Factor / Bend allowance / Bend deduction), Default bend K Factor (0.45), Rolled K Factor (0.5). Group *Relief* (collapsed): Minimal gap (0.02 mm), Corner relief type (Square - Sized / Rectangle - Scaled / Round - Sized / Round - Scaled / Closed / Simple; default Simple) + Corner relief scale (1.5) / diameter / width, Bend relief type (Rectangle - Scaled / Obround - Scaled / Tear; default Obround - Scaled) + Bend relief depth scale (1.5), Bend relief width scale (1.0625). All remembered. |
| **Flange** | Wall connected by a bend to each selected edge | Edges or side faces to flange; Flange alignment (Inner / Outer / Middle / Hold line); End type (Blind / Up to entity / Up to entity with offset) + Distance (25 mm) / Up to entity / Offset; Angle control type (Bend angle / Align to geometry / Angle from direction); Opposite side; Bend angle (90°, [1..179]) / Parallel to / Direction+Angle; Automatic miter (true) → Miter angle; Use model bend radius (true) → Bend radius; Partial flange (bool → Chain type, Flip sides, Hold adjacent edges (true), Bound type, Distance…). Manipulator drag for distance. |
| **Hem** | Fold edge back (straight/rolled/tear drop) | Edges or side faces to hem; Hem type (Straight / Rolled / Tear drop; remembered); Opposite direction; Flattened (true, Straight); Inner radius; Use minimal gap (true, Tear drop) → Gap; Angle (270°, Rolled); Total length (12.5 mm); Hem alignment (Outer / In place); Corner type (Simple / Closed). |
| **Tab** | Add planar tab profile to a flange | Tab profile (planar faces / sketch regions); Flange to merge (SM faces); Subtraction offset (0); Subtraction scope. |
| **Bend** | Bend SM along a line | Shared bend top fields; Bend angle (90°, [1..359.9]) / Parallel to / Direction + Angle + Opposite direction angle; shared bottom fields (model radius / K factor). |
| **Jog** | Two opposite bends forming S/Z offset | Shared bend top fields; Bend angle (unconfigurable; read-only when set programmatically) / Parallel to / Direction…; bottom fields; Bounding type (Blind / Up to entity / Thickness) → Jog offset (5 mm) / Up to entity + Offset distance / Thickness factor (1); Jog offset anchor (Inside / Nominal / Outside); Preserve material (true). |
| **Form** | Place a form tool (Part Studio with Form tag) | Form Part Studio (PartStudioData picker: current doc / other docs / sheet-metal forms library; thickness passed as computed configuration input; remembered); Location(s) (MCs or sketch points); Flip direction; Target face(s) (planar active SM faces). Form tag defined via Tag feature (§3.9). |
| **Sheet metal loft** (`createFeatureTessellatedLoft`) | SM model connecting two profiles (faceted) | Result tabs New / Add; Profile 1; Profile 2; Connections (bool → Match connections array: Vertices or edges, Rip (false)); Chordal tolerance; Add → Merge scope (active SM solids) else full SM model parameter groups (General/Material/Relief). |
| **Make joint** | Convert intersection of two walls into joint | Edges or side faces to join (max 2); Joint type (Bend / Rip; default Rip); Bend → Use model bend radius → Bend radius; Rip → Joint style (Edge joint / Butt joint - Direction 1 / Butt joint - Direction 2; default Edge joint). |
| **Modify joint** | Change existing joint (created by table "Convert to rip/bend") | Joint (SM face/edge, max 1); Joint type (Bend / Rip / Tangent; default Bend); Bend → Use model bend radius, Use model K Factor → Bend calculation (K Factor / Bend allowance / Bend deduction) + value; Rip → Joint style (Edge / Butt Dir 1 / Butt Dir 2). Butt only for 90° joints. |
| **Corner** | Override corner relief at a vertex | Corner (SM vertex/edge, max 1); Corner relief type (6 styles above; default Square - Sized); Corner relief scale (1.5) / diameter / width (remembered). |
| **Bend relief** | Override relief where bend meets free edge | Bend relief (SM vertex/edge, max 1); Bend relief type (Square - Sized / Rectangle - Scaled / Obround - Sized / Obround - Scaled / Tear; default Obround - Sized); depth scale (1.5) + width scale (1.0625) for scaled; Bend relief depth for sized; Extend bend relief (false, not Tear). |
| **Corner break** | Fillet/chamfer SM corners (flat-aware) | Type tabs Fillet / Chamfer (remembered); Entities to fillet or chamfer (SM corner vertices/edges; box-select edges); then standard fillet options (radius…) or chamfer options. After a Corner break, joint type/style no longer editable in table — add last. (A second attribute-based variant: Corners to break, Corner break style Chamfer/Fillet, Range.) |
| **Rip** (FS, not toolbar-listed in docs) | Split SM between vertex pairs | Vertex pairs to split with; Use default minimal gap (true) → Minimal gap. |
| **Unfold / Refold** (FS) | Temporarily unfold bends | Unfold: Unfold all (true) → Sheet metal part / else Bend or non-planar faces; Wall face or bounding edge to hold. Refold: Parts or bends. |
| **Recognize** (FS) | Recognize SM from imported bodies | Bodies; Radius; Change thickness → Thickness; Keep input parts; K Factor (0.45); Minimal gap. |
| **Finish sheet metal model** | Deactivate SM model(s) | Sheet metal parts (active SM bodies). |

**Sheet metal table & flat view** [DOCS sheet_metal_table]: toggle icon midway on right side of window; split panel with flat pattern view + tables (Bends, Rips/Joints; collapsible carets); model dropdown at top. Rows cross-highlight with flat view and 3D model (multi-select by clicking rows; picking in graphics scrolls table). Row context menu: Move up/down, **Convert to rip / Convert to bend** (creates Modify joint feature). Rip row dropdown = joint style (Butt only for 90°). Double-click K Factor / Bend allowance / Bend deduction or radius cells to edit (out-of-range → red cell + tooltip with valid range). Flat pattern context menu: **New sketch** (sketch on flat; extrude on flat gets abbreviated Extrude dialog add/remove), **Export DXF/DWG of flat pattern** (file name, DWG/DXF, version, scope: Single flat pattern part only / All flat pattern parts in the current model / All flat pattern parts in the Part Studio; options: splines as polylines, z-height zero, bend centerlines, bend tangent lines, cbore/csink lines, form outlines, form centermarks, visible sketches). Flip direction up (model feature) reverses flat orientation.

---

## 9. Other Part Studio panels / items (brief)

- **Model-based definition (MBD)** [DOCS model_based_definition]: PMI annotations (datums, driven dimensions, geometric tolerances, weld symbols) stored in the Part Studio; driving sketch/feature dimensions shown when Inspection panel open — double-click one → "Dimension context" mini-dialog + owning Sketch/Feature dialog open simultaneously; up/down arrows increment, Tab previews, Enter accepts; select annotation + Delete removes it (and its Tolerance options). Drawings via "Suggested views" import MBD.
- **Visibility conditions**: configuration logic — fully covered in §4.4.
- **Inspection table** [DOCS inspection_table]: right-panel icon below View cube; "Model definitions": Model (part) dropdown, Isolate active part, Select default tolerances; annotation toolbar (Datum, Dimension / Fillet dimension / Minimum / Maximum dimension, Geometric tolerance, Weld symbol, Set coincident, Set parallel, Set text alignment); **Datums** table + **Characteristics** table (Type, Nominal, Tolerance, Upper/Lower limit…; filters: show/hide default tolerances, Types = Distance/Angle/Radius/Diameter/Chamfer/Fillet/Hole/Weld/GTol); exportable. Details belong to MBD/drawing section.
- **Hole table** [DOCS hole_table]: right-panel icon appears only after a Hole feature exists; lists all holes; row hover highlights holes; overflow → Copy table / Show <column>; column context menu (move/hide).
- **Mass properties overrides** [DOCS View__mass_properties_tool]: in Mass and section properties dialog, select the whole part in the Feature/Parts list → **Override** checkbox → enter Mass (recomputes dependent values). Excluded configurations still count in Mass properties (§4.4). Public-part metadata overrides apply company-wide, carry to new versions, flow to BOM [DOCS properties_metadata].
- **Custom tables / Cut list table / Sheet metal table / Variable table / Configuration panel / Appearance panel** all share the right-side panel strip (icons below View cube). [DOCS]
- Out of scope (toolbar mention only): Flex PCB model / Finish flex PCB model, PCB holes tag, Simulation, Render Studio.
