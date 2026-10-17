# C. Part Studio features I: solid creation and detail features (complete dialog specifications)

Scope: Extrude, Revolve, Sweep, Loft, Thicken, Enclose, Fillet, Chamfer, Face blend, Modify fillet, Draft, Body draft, Rib, Shell, Hole, External thread, Boolean, Split, Delete part.

Sources: [FS] = FeatureScript std v3095.0 preconditions (`fs-features.md`, `fs-std/*.fs`), the authority for field order, labels, options, defaults. [DOCS] = Onshape Help pages `PartStudio__*.md`. [BUNDLE] = web-client JS bundle (command IDs). Nothing here was observed live.

---

## 0. Conventions shared by every dialog

### 0.1 How to read the field tables
- Rows are in **exact dialog order** [FS]. Fields marked `ALWAYS_HIDDEN` in FS are left out of the tables and listed under "Hidden params".
- **Visible when** is the condition the field shows under. It is re-evaluated live on every edit, so the dialog reflows right away.
- **Default (UI)**: the annotation `"Default"` if there is one. Otherwise an enum defaults to its **first value**, a boolean to `false`, a query to empty, and a number to the bound spec's default. Each feature's FS "Defaults" map is only a fallback for programmatic calls, and it sometimes disagrees with the UI default (noted where it matters).
- Length defaults depend on document units. They are written "mm-value / inch-value" (the bound specs also define cm, m, ft, yd).
- "Column Name" annotations [FS] are alternate labels used in configuration/variable tables. They don't show in the dialog. They're mentioned only where they make two identical labels distinguishable.

### 0.2 UIHint legend (how to render) [FS]
| UIHint | Rendering / behavior for the clone |
|---|---|
| HORIZONTAL_ENUM | Segmented button row (tabs) instead of a dropdown. |
| SHOW_LABEL | Show the field label to the left of the dropdown. Without it, enum dropdowns render label-less, with only the selected value visible. |
| REMEMBER_PREVIOUS_VALUE | When the feature is next created, initialize the field from the last committed value (per user, per feature type). Docs confirm this for Creation type, fillet measurement/control/radius, chamfer type/distance, face blend cross section/trim, boolean keep tools, and the hole fields. |
| OPPOSITE_DIRECTION | Flip-arrow toggle button, usually placed at the right end of the preceding field's row. Bound to a boolean. |
| OPPOSITE_DIRECTION_CIRCULAR | Same, but with a circular-arrow icon (angles, twist). |
| DISPLAY_SHORT + FIRST_IN_ROW | A checkbox and the value field that follows it render on **one row**: `[x] Offset distance [ 25 mm ] [flip]`. |
| CAN_BE_TOLERANT / CAN_BE_TOLERANT_DIAMETER | Value field gets a "Tolerance options" icon. It expands Precision + Type (Default, Symmetrical, Deviation, Limits, Min, Max, Basic, and fit classes for diameters). See the tolerance section. |
| SHOW_EXPRESSION | Field shows the expression (e.g. the table-driven value) rather than only the evaluated number. |
| READ_ONLY | Greyed-out display-only value (e.g. the computed hole depth). |
| UNCONFIGURABLE | Field can't be configured or driven by a configuration. |
| MATCH_LAST_ARRAY_ITEM | A new array item copies this field's value from the previous item. |
| COLLAPSE_ARRAY_ITEMS | Array items render collapsed to one summary line ("Item label template"), expand on click, and can be reordered (a "Reorder items" button turns on drag handles [DOCS loft]). |
| PREVENT_ARRAY_REORDER | No drag reordering of array items. |
| FOCUS_INNER_QUERY / FOCUS_ON_VISIBLE / INITIAL_FOCUS / INITIAL_FOCUS_ON_EDIT | Control which query field gets selection focus: on open (INITIAL_FOCUS), on edit (INITIAL_FOCUS_ON_EDIT), when the field appears (FOCUS_ON_VISIBLE), or the inner query of a new array item (FOCUS_INNER_QUERY). |
| ALLOW_QUERY_ORDER | Selection order is preserved and meaningful. |
| SHOW_CREATE_SELECTION | The field offers the "Create selection" helper (select-by-rule). |
| Group Name / Driving Parameter / Collapsed By Default | A collapsible group under a checkbox. The driving checkbox both shows/hides the group and acts as its header (the "dropdown" the docs mention, e.g. "Click the Allow edge overflow dropdown"). |
| Lookup Table (LookupTablePath) | Renders as a **cascade of dropdowns**, one per table level (e.g. Hole type > Size > Threads/inch > Fastener fit). See Hole. |

### 0.3 Common predicate: Result body operation type + merge scope (booleanStep) [FS]
Used by Extrude, Revolve, Sweep, Loft, Thicken, Enclose. Resolved inline in each table too.
- `Result body operation type` (operationType, NewBodyOperationType, HORIZONTAL_ENUM): **New** / **Add** / **Remove** / **Intersect**. Default New.
- Surface variant (surfaceOperationType, NewSurfaceOperationType, HORIZONTAL_ENUM): **New** / **Add**.
- Solid merge scope (booleanStepScopePredicate), shown only when operationType != New:
  - `Merge with all` (defaultScope, boolean, **Default false** [FS]). [DOCS] says "By default, Merge all is selected", which conflicts with FS; follow FS.
  - `Merge scope` (booleanScope, query), visible when Merge with all is off. Filter: modifiable **solid** bodies (mesh allowed).
- Surface merge scope (surfaceJoinStepScopePredicate), shown only when surfaceOperationType != New:
  - `Merge with all` (defaultSurfaceScope, **Default true**).
  - `Merge scope` (booleanSurfaceScope): modifiable **sheet** bodies, non-sketch.
- Both merge fields render only if the definition has `defaultScope` (very old features don't).
- **Auto-selection heuristics** (booleanStepEditLogic) [FS]:
  - While the user hasn't touched Result type, Merge with all, or Merge scope, the feature builds a temporary "New" result and collides it with every visible modifiable solid.
  - Collides with exactly **one** part: **Add** with that part as merge scope. If that part is active sheet metal: Remove, or New when the tool is also sheet metal.
  - Collides with **several** parts, or none: **New**.
  - These heuristics stop once the user explicitly sets the operation or scope. They also skip re-deriving the operation while the user is flipping the direction arrow or toggling sweep "Extend to full path".
  - [DOCS] says the same: "If the geometry touches or intersects with only one part then that part is automatically added to the merge scope... If multiple parts... you must select".
- **Error rule** [DOCS]: Add/Remove/Intersect with an empty merge scope fails. New has no merge scope.
- **Direction auto-flip on op change** [FS canSetBooleanFlip]: switching between a "positive" op (New/Add) and a "negative" one (Remove/Intersect) auto-toggles the direction flip (Extrude, Revolve, Thicken) unless the user set the flip or scope explicitly.

### 0.4 Common "Thin" sub-block (Extrude/Revolve/Sweep/Loft Thin creation type) [FS]
| UI label | param | type | default | visible when |
|---|---|---|---|---|
| Mid plane | midplane | boolean | false | Thin |
| Thickness 1 | thickness1 | length, 0..500 m (ZERO_INCLUSIVE_OFFSET_BOUNDS) | 5 mm / 0.25 in | Thin and not Mid plane |
| Flip wall | flipWall | boolean, OPPOSITE_DIRECTION arrow | false | Thin and not Mid plane |
| Thickness 2 | thickness2 | length, 0..500 m | 0 | Thin and not Mid plane |
| Thickness | thickness | length, 0..500 m | 5 mm / 0.25 in | Thin and Mid plane |
- Thickness fields are CAN_BE_TOLERANT on Extrude and Revolve, but not on Sweep or Loft.
- [DOCS]: Thickness and Thickness 1/2 are independent values, kept separately when Mid plane is toggled back and forth.

### 0.5 Generic dialog behaviors (apply to all features here) [DOCS]
- Feature dialog has a Name field (pencil icon on hover renames), the field list, **Preview slider** (before/after), **Final** button (show the result at end of the feature list), accept (check) and cancel (X).
- On open, the first query field is active (blue), and the first text field takes keyboard input. Tab moves through fields, and arrow keys + Enter drive dropdowns.
- Mate-connector-capable query fields (filters including `BodyType.MATE_CONNECTOR` or `ALLOWS_VERTEX/AXIS/DIRECTION`) show a **mate connector icon button**:
  - Clicking it lets you pick implicit mate connectors on hover.
  - Once one is picked, clicking the connector chip in the field opens a nested Mate connector dialog to edit it.
- **Preselection**: entities selected before launching a tool fill the first matching query field (e.g. Hole picks up preselected sketch points via `initEntities`, and Body draft routes a preselection into Parts/Faces/Edges).

---

## 1. Extrude

- Toolbar: icon "Extrude" (feature icon), tooltip "Extrude". Command `createFeatureExtrude` [BUNDLE]. **Shortcut: Shift+E** [DOCS]. Also on the Sketch toolbar while a sketch is open.
- Available in: **Sketch, Part Studio** [DOCS].
- FS: `extrude` (extrude.fs). Editing logic `extrudeEditLogic`, manipulators `extrudeManipulatorChange`, filter selector "allparts".
- Hidden params: `domain` (OperationDomain, default MODEL). Edit logic sets it to FLAT when all selected regions are sheet-metal flat-pattern sketch regions.

### 1.1 Fields (model domain)
| # | UI label | param | type | options / range (default) | visible when | hints / filter |
|---|---|---|---|---|---|---|
| 1 | Creation type | bodyType | enum | **Solid** / Surface / Thin | domain != FLAT | HORIZONTAL_ENUM, REMEMBER_PREVIOUS_VALUE |
| 2 | Result body operation type | operationType | enum | **New** / Add / Remove / Intersect | bodyType != Surface | HORIZONTAL_ENUM |
| 2' | Result body operation type | surfaceOperationType | enum | **New** / Add | bodyType == Surface | HORIZONTAL_ENUM |
| 3 | Faces and sketch regions to extrude | entities | query | — | bodyType == Solid | Filter: sketch-region faces (flattened allowed) OR planar non-flattened faces; no construction |
| 3' | Sketch curves to extrude | surfaceEntities | query | — | bodyType == Surface | Filter: sketch edges, modifiable, non-construction |
| 3'' | Faces and sketch regions to extrude | wallShape | query | — | bodyType == Thin | Filter: planar faces (incl. non-modifiable, e.g. sketch regions) OR sketch edges (modifiable, non-construction) |
| 4-8 | Thin block (Mid plane, Thickness 1, Flip wall, Thickness 2, Thickness) | see 0.4 | | | Thin | CAN_BE_TOLERANT on thickness |
| 9 | End type | endBound | enum | **Blind** / Up to next / Up to face / Up to part / Up to vertex / Through all | always | dropdown, no label |
| 10 | Opposite direction | oppositeDirection | boolean | false | always | OPPOSITE_DIRECTION (arrow next to End type) |
| 11 | Depth | depth | length | -500..500 m, **25 mm / 1 in** | End type = Blind | CAN_BE_TOLERANT |
| 12 | Up to face | endBoundEntityFace | query (1 pick) | — | End type = Up to face | Filter: any non-sketch face (mesh ok) OR mate connector |
| 13 | Up to surface or part | endBoundEntityBody | query (1 pick) | — | End type = Up to part | Filter: solid or sheet body, non-sketch, mesh ok |
| 14 | Up to vertex or mate connector | endBoundEntityVertex | query (1 pick) | — | End type = Up to vertex | Filter: ALLOWS_VERTEX (vertices, sketch points, mate connectors) |
| 15 | Offset distance (checkbox) | hasOffset | boolean | false | End type in {Up to next, Up to face, Up to part, Up to vertex} | DISPLAY_SHORT, FIRST_IN_ROW |
| 16 | Offset distance | offsetDistance | length | -500..500 m, 25 mm / 1 in | hasOffset | DISPLAY_SHORT, CAN_BE_TOLERANT (same row) |
| 17 | Opposite direction | offsetOppositeDirection | boolean | false | hasOffset | OPPOSITE_DIRECTION |
| 18 | Direction | hasExtrudeDirection | boolean | false | bodyType != Thin | group header "Direction" |
| 19 | Extrude direction | extrudeDirection | query (1 pick) | — | hasExtrudeDirection | Filter: ALLOWS_DIRECTION OR mate connector (lines, axes, planar faces, MC Z axis) |
| 20 | Starting offset | startOffset | boolean | false | always | group header "Starting offset" |
| 21 | Starting offset bound | startOffsetBound | enum | **Blind** / Entity | startOffset | dropdown |
| 22 | Depth | startOffsetDistance | length | -500..500 m, 25 mm / 1 in | startOffset and bound = Blind | |
| 23 | Opposite direction | startOffsetOppositeDirection | boolean | false | startOffset and bound = Blind | OPPOSITE_DIRECTION |
| 24 | Entity | startOffsetEntity | query (1 pick) | — | startOffset and bound = Entity | Filter: planar face, edge, vertex, mate connector |
| 25 | Symmetric | symmetric | boolean | false | End type in {Blind, Through all} | |
| 26 | Draft (checkbox) | hasDraft | boolean | false | bodyType in {Solid, Thin} | DISPLAY_SHORT, FIRST_IN_ROW |
| 27 | Draft angle | draftAngle | angle | 0..89.9 deg, **3 deg** | hasDraft | DISPLAY_SHORT (same row) |
| 28 | Opposite direction | draftPullDirection | boolean | false | hasDraft | OPPOSITE_DIRECTION_CIRCULAR |
| 29 | Second end position | hasSecondDirection | boolean | false | NOT (Symmetric and End type in {Blind, Through all}) | FIRST_IN_ROW. Group "Second end position" (expanded) |
| 30 | End type | secondDirectionBound | enum | same 6 values, **Blind** | hasSecondDirection | |
| 31 | Opposite direction | secondDirectionOppositeDirection | boolean | **true** | hasSecondDirection | OPPOSITE_DIRECTION |
| 32 | Depth | secondDirectionDepth | length | 25 mm / 1 in | 2nd end type = Blind | (not tolerant) |
| 33 | Up to face / Up to surface or part / Up to vertex or mate connector | secondDirectionBoundEntityFace / ...Body / ...Vertex | query (1 pick) | — | 2nd end type = matching Up to | same filters as 12-14 |
| 34 | Offset distance (checkbox) | hasSecondDirectionOffset | boolean | false | 2nd end type is an Up-to type | DISPLAY_SHORT, FIRST_IN_ROW |
| 35 | Offset distance | secondDirectionOffsetDistance | length | 25 mm / 1 in | hasSecondDirectionOffset | DISPLAY_SHORT, CAN_BE_TOLERANT |
| 36 | Opposite direction | secondDirectionOffsetOppositeDirection | boolean | false | hasSecondDirectionOffset | OPPOSITE_DIRECTION |
| 37 | Draft (checkbox) | hasSecondDirectionDraft | boolean | false | bodyType in {Solid, Thin} AND second direction points away from the first (secondDirectionOppositeDirection XOR oppositeDirection) | DISPLAY_SHORT, FIRST_IN_ROW |
| 38 | Draft angle | secondDirectionDraftAngle | angle | 0..89.9, 3 deg | hasSecondDirectionDraft | DISPLAY_SHORT |
| 39 | Opposite direction | secondDirectionDraftPullDirection | boolean | false | hasSecondDirectionDraft | OPPOSITE_DIRECTION_CIRCULAR |
| 40 | Merge with all | defaultScope | boolean | false | bodyType != Surface and op != New | |
| 41 | Merge scope | booleanScope | query | — | above and not Merge with all | modifiable solids |
| 40' | Merge with all | defaultSurfaceScope | boolean | **true** | Surface and op = Add | |
| 41' | Merge scope | booleanSurfaceScope | query | — | Surface, Add, not Merge with all | sheet bodies |

### 1.2 Fields (flat domain: extruding a sheet-metal flat-pattern sketch) [FS][DOCS]
| UI label | param | type | options (default) |
|---|---|---|---|
| Creation type | flatOperationType | enum, HORIZONTAL_ENUM | Add / **Remove** (annotated default Remove) |
| Faces and sketch regions to extrude | entities | query | as row 3 |
- [DOCS]: the "abbreviated Extrude dialog" shows only add/remove. An Extrude defined in the 3D model can't be reused as a flat-pattern extrude; doing so raises a regeneration warning. Mixing flat and 3D regions gives EXTRUDE_3D_AND_FLAT [FS].

### 1.3 Behavior [DOCS][FS]
- **Auto-select regions**: if Extrude (Solid) is launched while a sketch is open, all closed regions of that sketch are preselected, including nested ones [DOCS].
- **Creation type remembered** (Solid/Surface/Thin) [DOCS, REMEMBER_PREVIOUS_VALUE].
- **Manipulators** [FS addExtrudeManipulator]:
  - Blind: a linear **depth arrow** at the profile origin along the extrude axis.
    - Dragging sets depth, and dragging past zero flips oppositeDirection. With Symmetric the arrow shows half the depth, and drag sets 2x.
    - The arrow origin accounts for Starting offset.
  - Non-blind end types: a **flip arrow** manipulator only.
  - Through all + Symmetric: no manipulator.
  - Second end position: a second depth arrow, or flip arrow, in SECONDARY style.
  - No manipulators for flat-pattern regions.
- **Direction auto-flip** [FS]: when no second direction is set and the user hasn't touched the flip, the flip is chosen to point toward the selected Up-to face/part/vertex. Otherwise it follows the op change (0.3). The second-direction flip is auto-set as well.
- End type semantics [DOCS]:
  - Up to next: next face(s) in the direction. Fails if the profile doesn't fully terminate (EXTRUDE_UPTO_NEXT_NO_DIVISION).
  - Up to face: the infinite surface underlying the face/plane.
  - Up to part: next part or surface.
  - Up to vertex: vertex or (implicit) mate connector.
  - Through all: through all parts, one direction, or both with Symmetric.
  - For Up to next/face/part, the profile must fall entirely within the target.
- Offset (Up-to types): the extrude stops short of the target by the distance, and the flip makes it overshoot instead.
- Direction: extrude along an axis or mate connector Z instead of the sketch normal. A direction coplanar with the profile errors (EXTRUDE_DIRECTION_COPLANAR). Not available for Thin.
- Starting offset Entity: must be planar and parallel to the profile plane (EXTRUDE_START_OFFSET_BOUND_NOT_PLANAR / _NOT_PARALLEL_...).
- Draft: neutral plane = sketch plane. Second-direction draft is offered only when the second direction points away from the first.
- Thin extrude requires the selected faces to lie in parallel planes (THIN_EXTRUDE_NOT_PARALLEL_PLANES).
- Merge scope: "Surfaces must be merged with surfaces and parts with parts" [DOCS].
- Errors (ErrorStringEnum) [FS]: EXTRUDE_NO_SELECTED_REGION, EXTRUDE_NO_REGION_IN_SKETCH, EXTRUDE_SURF_NO_CURVE, EXTRUDE_SELECT_TERMINATING_SURFACE/BODY/VERTEX, EXTRUDE_SELECT_DIRECTION, EXTRUDE_DIRECTION_INVALID_ENTITY, EXTRUDE_NO_DIRECTION, EXTRUDE_OFFSET_TOO_DEEP, EXTRUDE_SELECT_START_OFFSET_ENTITY, EXTRUDE_INVALID_REF_FACE/SURFACE, EXTRUDE_FAILED.

---

## 2. Revolve

- Toolbar: "Revolve". Command `revolve` [BUNDLE]. **Shortcut: Shift+W** [DOCS]. Also on the Sketch toolbar.
- Available in: **Sketch, Part Studio** [DOCS].
- FS: `revolve`. Editing logic `revolveEditLogic`, manipulators `revolveManipulatorChange`.

### 2.1 Fields
| # | UI label | param | type | options / range (default) | visible when | hints / filter |
|---|---|---|---|---|---|---|
| 1 | Creation type | bodyType | enum | **Solid** / Surface / Thin | always | HORIZONTAL_ENUM, REMEMBER_PREVIOUS_VALUE |
| 2 | Result body operation type | operationType | enum | **New** / Add / Remove / Intersect | Solid or Thin | HORIZONTAL_ENUM |
| 2' | Result body operation type | surfaceOperationType | enum | **New** / Add | Surface | HORIZONTAL_ENUM |
| 3 | Faces and sketch regions to revolve | entities | query | — | Solid | planar faces/regions, non-construction |
| 3' | Edges and sketch curves to revolve | surfaceEntities | query | — | Surface | edges (non-construction) OR non-sketch wire bodies (curves) |
| 3'' | Faces and sketch regions to revolve | wallShape | query | — | Thin | planar faces (non-modifiable ok) OR sketch edges |
| 4-8 | Thin block | see 0.4 | | | Thin | CAN_BE_TOLERANT |
| 9 | Revolve axis | axis | query (1 pick) | — | always | Filter: ALLOWS_AXIS (lines, linear/circular edges, cylinders, arcs, mate connector) |
| 10 | Full revolve | fullRevolve | boolean | **true** | always | |
| 11 | End type | endBound | enum | **Blind** / Up to next / Up to face / Up to part / Up to vertex | not Full revolve | |
| 12 | Opposite direction | oppositeDirection | boolean | false | not Full revolve | OPPOSITE_DIRECTION_CIRCULAR |
| 13 | Revolve angle | angle | angle | **30 deg** (bounds +-1e5 deg) | End = Blind | CAN_BE_TOLERANT |
| 14 | Symmetric | symmetric | boolean | false | End = Blind | |
| 15 | Up to face | endBoundEntityFace | query (1) | — | End = Up to face | face (non-sketch, mesh ok) OR mate connector |
| 16 | Up to surface or part | endBoundEntityBody | query (1) | — | End = Up to part | solid/sheet body |
| 17 | Up to vertex or mate connector | endBoundEntityVertex | query (1) | — | End = Up to vertex | ALLOWS_VERTEX |
| 18 | Offset (checkbox) | endBoundHasOffset | boolean | false | End != Blind | DISPLAY_SHORT, FIRST_IN_ROW |
| 19 | Offset | endBoundOffset | angle | 0 deg | endBoundHasOffset | DISPLAY_SHORT |
| 20 | Flip offset | endBoundOffsetFlip | boolean | false | endBoundHasOffset | OPPOSITE_DIRECTION_CIRCULAR |
| 21 | Second end position | hasStartBound | boolean | false | End != Blind OR not Symmetric | group "Second direction" (expanded) |
| 22 | Start type | startBound | enum | **Blind** / Up to next / Up to face / Up to part / Up to vertex | hasStartBound | |
| 23 | Start angle | angleBack | angle | **330 deg** (ANGLE_360_REVERSE_DEFAULT_BOUNDS) | Start type = Blind | |
| 24 | Opposite direction | startOppositeDirection | boolean | false | Start type != Blind | OPPOSITE_DIRECTION_CIRCULAR |
| 25 | Up to face / Up to surface or part / Up to vertex or mate connector | startBoundEntityFace/Body/Vertex | query (1) | — | matching Start type | as 15-17 |
| 26 | Start offset (checkbox) | startBoundHasOffset | boolean | false | Start type != Blind | DISPLAY_SHORT, FIRST_IN_ROW |
| 27 | Start offset | startBoundOffset | angle | 0 deg | startBoundHasOffset | DISPLAY_SHORT |
| 28 | Flip offset | startBoundOffsetFlip | boolean | false | startBoundHasOffset | OPPOSITE_DIRECTION_CIRCULAR |
| 29 | Merge with all / Merge scope | defaultScope / booleanScope | | false | Solid/Thin, op != New | see 0.3 |
| 29' | Merge with all / Merge scope | defaultSurfaceScope / booleanSurfaceScope | | true | Surface, op = Add | see 0.3 |

### 2.2 Behavior
- Solid with a sketch open: all closed regions are auto-selected [DOCS]. Creation type remembered.
- Full revolve (default) = 360 deg. Uncheck it to get End type [DOCS].
- **Manipulators** [FS]: an angular manipulator for the Blind angle, a flip manipulator for Up-to types, and a second angular/flip manipulator for the second end.
- Direction auto-flip on op change: only when the first direction is Blind, not symmetric, and there's no second end [FS].
- Surface op auto-logic: Full revolve, symmetric, or second end forces surface result **New** unless the user set it [FS].
- Offset for Up-to types is an **angle** [DOCS: "Offset by a number of degrees"]. Flip offset makes it overshoot.
- Errors [FS]: REVOLVE_SELECT_FACES, REVOLVE_SELECT_AXIS, REVOLVE_NOT_COPLANAR_WITH_AXIS, REVOLVE_NOT_PLANAR, REVOLVE_PERPENDICULAR, REVOLVE_SURF_NO_CURVE, REVOLVE_UP_TO_NEXT_NO_TARGET, REVOLVE_UP_TO_FAILED, REVOLVE_2ND_DIR_FAILED, REVOLVE_SELECT_TERMINATING_SURFACE/BODY/VERTEX.

---

## 3. Sweep

- Toolbar: "Sweep". Command `createFeatureSweep` [BUNDLE]. No default shortcut.
- Available in: Part Studio [DOCS].
- FS: `sweep`. Editing logic `sweepEditLogic` (boolean heuristics, or surface op heuristics).

### 3.1 Fields
| # | UI label | param | type | options / range (default) | visible when | hints / filter |
|---|---|---|---|---|---|---|
| 1 | Creation type | bodyType | enum | **Solid** / Surface / Thin | always | HORIZONTAL_ENUM, REMEMBER |
| 2 | Result body operation type | operationType / surfaceOperationType | enum | New/Add/Remove/Intersect, or New/Add for Surface | always | HORIZONTAL_ENUM |
| 3 | Faces and sketch regions to sweep | profiles | query | — | Solid | planar faces/regions, non-construction |
| 3' | Edges and sketch curves to sweep | surfaceProfiles | query | — | Surface | edges OR non-sketch wire bodies |
| 3'' | Faces and sketch regions to sweep | wallShape | query | — | Thin | edges, faces, or non-sketch wire bodies; non-construction |
| 4-8 | Thin block | see 0.4 | | | Thin | (not tolerant) |
| 9 | Sweep path | path | query | — | always | edges (non-construction) OR non-sketch wire bodies |
| 10 | Opposite direction | pathOppositeDirection | boolean | false | always | OPPOSITE_DIRECTION (arrow beside Sweep path: start from the other end of the path) |
| 11 | Extend to full path | extendToFullPath | boolean | false | always | |
| 12 | Profile control | profileControl | enum | **None** / Keep profile orientation / Lock profile faces / Lock profile direction | always | SHOW_LABEL, REMEMBER |
| 13 | Faces to lock | lockFaces | query | — | Profile control = Lock profile faces | faces, non-construction |
| 14 | Direction to lock | lockDirectionQuery | query (1) | — | Profile control = Lock profile direction | ALLOWS_DIRECTION OR mate connector |
| 15 | Twist | hasTwist | boolean | false | Profile control in {None, Keep profile orientation} | group "Twist" |
| 16 | Twist type | twistType | enum | **Turns** / Angle / Pitch | hasTwist | SHOW_LABEL, REMEMBER |
| 17 | Revolutions | turns | real | 0..1000, **1** | Twist type = Turns | |
| 18 | Rotation angle | angle | angle | 0..360000 deg, **0** | Twist type = Angle | |
| 19 | Pitch length | pitch | length | >=1e-6 m, 25 mm / 1 in | Twist type = Pitch | |
| 20 | Twist opposite direction | ccw | boolean | false | hasTwist | OPPOSITE_DIRECTION_CIRCULAR |
| 21 | Scale (checkbox) | hasScale | boolean | false | always | DISPLAY_SHORT, FIRST_IN_ROW |
| 22 | Scale factor | scaleFactor | real | 1e-5..1e5, **1.0** | hasScale | DISPLAY_SHORT |
| 23 | Trim ends | trimEnds | boolean | **true** | Thin | |
| 24 | Merge with all / Merge scope | | | | per 0.3 (solid or surface variant) | |

### 3.2 Behavior [DOCS][FS]
- Profile control:
  - None: the profile keeps its relationship to the global plane.
  - Keep profile orientation: the profile keeps its relationship along the path (Frenet-like).
  - Lock profile faces: orientation follows the selected faces.
  - Lock profile direction: orientation follows the selected direction.
- Twist (Turns: total rotations; Angle: rotation reached at path end; Pitch: one revolution per pitch length) and Scale (profile scaled proportionally by the end of the path) need a tangent-continuous path: SWEEP_TWIST_REQUIRE_TANGENT_PATH, SWEEP_SCALE_REQUIRE_TANGENT_PATH, SWEEP_TWIST_SCALE_REQUIRE_TANGENT_PATH.
- Twist is hidden when profile faces/direction are locked.
- If the profile sits mid-path rather than at an end, the path is broken there and the sweep runs both ways along the full path [DOCS].
- Extend to full path: sweep along the entire path. Toggling it doesn't re-trigger the boolean op heuristic [FS].
- Thin + Trim ends: when on, the sweep starts and ends at the path endpoints. When off, end faces are tangent to the path [DOCS].
- Creation type remembered [DOCS].
- No manipulators (no manipulator function).
- Errors [FS]: SWEEP_SELECT_PROFILE, SWEEP_SELECT_PATH, SWEEP_PATH_NO_CONSTRUCTION, SWEEP_INVALID_PATH, SWEEP_MIXED_PROFILE, SWEEP_DISJOINT_PROFILE, SWEEP_SURF_NO_CURVE_PROFILE, SWEEP_START_NOT_ON_PROFILE, SWEEP_PATH_PROFILE_NO_INTERSECTION, SWEEP_BAD_LOCK_FACES, SWEEP_BAD_LOCK_DIRECTION, SWEEP_SELECT_DIRECTION, SWEEP_MULTIPROFILE_SCALE_NOT_ALLOWED, SWEEP_TWIST_ANGLE_EXCEEDS_LIMITS, SWEEP_TWIST_PITCH_EXCEEDS_LIMITS, SWEEP_EXTEND_HAS_NO_EFFECT, SWEEP_SELF_INT, THIN_SWEEP_* warnings.

---

## 4. Loft

- Toolbar: "Loft". Command `createFeatureLoft` [BUNDLE].
- Available in: Part Studio [DOCS].
- FS: `loft`. Editing logic `loftEditLogic`, manipulators `loftManipulator` (connection points).

### 4.1 Fields
| # | UI label | param | type | options / range (default) | visible when | hints / filter |
|---|---|---|---|---|---|---|
| 1 | Creation type | bodyType | enum | **Solid** / Surface / Thin | always | HORIZONTAL_ENUM, REMEMBER |
| 2 | Result body operation type | operationType / surfaceOperationType | enum | New/Add/Remove/Intersect, or New/Add | always | HORIZONTAL_ENUM |
| 3 | Profiles (array, item "profile") | sheetProfilesArray | array | — | Solid | COLLAPSE_ARRAY_ITEMS. Item label = its selection |
| 3a | Faces and sketch regions (per item) | sheetProfileEntities | query | — | (in item) | faces OR non-sketch sheet bodies (non-construction) OR a **vertex** (point profile) |
| 3' | Profiles (array) | wireProfilesArray | array | — | Surface or Thin | COLLAPSE_ARRAY_ITEMS |
| 3'a | Edges, curves and sketches | wireProfileEntities | query | — | (in item) | edges, faces, non-sketch wire/sheet bodies, OR vertex |
| 4-8 | Thin block | see 0.4 | | | Thin | |
| — | Group "End conditions" (expanded) | | | | always | |
| 9 | Start profile condition | startCondition | enum | **None** / Normal to profile / Tangent to profile / Match tangent / Match curvature / Normal direction / Tangent direction | always | SHOW_LABEL |
| 10 | Adjacent faces | adjacentFacesStart | query | — | start = Match tangent or Match curvature | faces. FOCUS_ON_VISIBLE |
| 11 | Start magnitude | startMagnitude | real | -1e5..1e5, **1** | start != None | |
| 12 | Start direction | startDirection | query (1) | — | start = Tangent direction or Normal direction | ALLOWS_DIRECTION OR mate connector |
| 13 | End profile condition | endCondition | enum | same as 9 | always | SHOW_LABEL |
| 14 | Adjacent faces | adjacentFacesEnd | query | — | end = Match tangent/curvature | FOCUS_ON_VISIBLE |
| 15 | End magnitude | endMagnitude | real | 1 | end != None | |
| 16 | End direction | endDirection | query (1) | — | end = Tangent/Normal direction | |
| 17 | Trim profiles | trimProfiles | boolean | false | Surface or Thin | |
| 18 | Guides and continuity | addGuides | boolean | false | always | |
| 19 | Guides (array, item "guide") | guidesArray | array | — | addGuides | COLLAPSE_ARRAY_ITEMS, FOCUS_ON_VISIBLE |
| 19a | Edges, curves and sketches | guideEntities | query | — | (item) | edges OR non-sketch wire bodies |
| 19b | Continuity | guideDerivativeType | enum | **None** / Normal to guide / Tangent to guide / Match tangent / Match curvature | (item) | SHOW_LABEL, MATCH_LAST_ARRAY_ITEM |
| 20 | Trim guides | trimGuidesByProfiles | boolean | **true** | addGuides and (Surface or Thin) | |
| 21 | Path | addSections | boolean | false | always | |
| 22 | Edges, curves and sketches | spine | query | — | addSections | edges OR wire bodies. FOCUS_ON_VISIBLE |
| 23 | Section count | sectionCount | integer | 1..50, **5** | addSections | |
| 24 | Connections | matchConnections | boolean | false | always | group "Connections" |
| 25 | Match connections (array, item "connection") | connections | array | — | matchConnections | FOCUS_INNER_QUERY, FOCUS_ON_VISIBLE |
| 25a | Vertices or edges | connectionEntities | query | — | (item) | edges (non-construction) OR vertices (not edge points) |
| 26 | Show isocurves | showIsocurves | boolean | false | always | |
| 27 | Count | curveCount | integer | 1..50, **10** | showIsocurves | |
| 28 | Trim ends | trimEnds | boolean | **true** | Thin | |
| 29 | Merge with all / Merge scope | | | | per 0.3 | |
- Hidden params: guideDerivativeMagnitude (per guide), connectionEdgeQueries, connectionEdgeParameters (per connection), makePeriodic.

### 4.2 Behavior [DOCS][FS]
- **Profile order matters**: select start, then intermediate sections in order, then end. Use "Reorder items" to drag them into order. At least 2 profiles (LOFT_TWO_PROFILES). A point (vertex) may be a start or end profile.
- Each profile must be a **single contour**. A profile with extra contours or nested loops highlights **red** and is unsuitable (LOFT_PROFILE_NO_INNER_LOOPS).
- Composite profile or guide: expand the item (arrow) and keep selecting tangent-connected curves into the same item. The active item is highlighted blue.
- Best results come when profiles have the same segment/vertex count. Otherwise twisting can occur, and Connections fix it.
- **Connections**:
  - Automatic connections show in **magenta**, but only while no Match connection entries exist.
  - Each connection needs one vertex or edge per profile.
  - Once one exists, a **drag manipulator** moves the connection point along its edge (connectionEdgeParameters).
- Guides must be G1-continuous and intersect every profile.
- Match tangent/curvature conditions need adjacent faces. The face's underlying geometry must coincide with a profile curve.
- The Path ("centerline") option isn't compatible with non-planar profiles. More sections follow the path more closely.
- Switching Creation type copies profile selections between the sheet and wire arrays [FS].
- Use **Final** to check the result while editing.
- Errors [FS]: LOFT_SELECT_PROFILES, LOFT_SELECT_GUIDES, LOFT_SELECT_SPINE, LOFT_PROFILE_SOLID, LOFT_PROFILE_SINGLE_FACE, LOFT_GUIDE_NOT_SMOOTH, LOFT_GUIDE_PROFILE_INTERSECTION, LOFT_VERTEX_NOT_ON_PROFILE, LOFT_CONNECTION_MATCHING, LOFT_NO_FACE_FOR_START_CLAMP/END_CLAMP, LOFT_NO_DIRECTION_FOR_START/END, LOFT_SPINE_* (non-tangent, self-intersecting, profiles not in order), LOFT_NO_CLAMPS_ON_POINT_PROFILE, LOFT_START_OR_END_CONDITIONS_MAGNITUDE_NO_EFFECT, LOFT_CANT_TRIM_PROFILES_INFO / _GUIDES_INFO.

---

## 5. Thicken

- Toolbar: "Thicken". Command `createFeatureThicken` [BUNDLE]. Available in: Part Studio.
- FS: `thicken`. Editing logic `thickenEditLogic`.

### 5.1 Fields
| # | UI label | param | type | options / range (default) | visible when | hints / filter |
|---|---|---|---|---|---|---|
| 1 | Result body operation type | operationType | enum | **New** / Add / Remove / Intersect | always | HORIZONTAL_ENUM |
| 2 | Faces and surfaces to thicken | entities | query | — | always | faces (any, incl. sketch regions) OR non-sketch sheet bodies; non-construction |
| 3 | Mid plane | midplane | boolean | false | always | |
| 4 | Thickness 1 | thickness1 | length | 0..500 m, 5 mm / 0.25 in | not Mid plane | |
| 5 | Opposite direction | oppositeDirection | boolean | false | not Mid plane | OPPOSITE_DIRECTION |
| 6 | Thickness 2 | thickness2 | length | 0 | not Mid plane | |
| 7 | Thickness | thickness | length | 5 mm / 0.25 in | Mid plane | |
| 8 | Keep tools | keepTools | boolean | **false** (UI). FS defaults map says true for programmatic calls | always | |
| 9 | Merge with all / Merge scope | defaultScope / booleanScope | | false | op != New | solids |

### 5.2 Behavior
- Selecting a **solid part face** auto-switches the op to **Add** and sets merge scope = that face's owner part. If the direction is flipped, it becomes **Remove** [FS thickenEditLogic][DOCS]. Changing Add to Remove auto-flips the direction arrow [DOCS: "note the Direction arrow is automatically flipped"].
- Keep tools unchecked: the source **surfaces** are deleted and replaced by the new part. Solid bodies and sketches are never deleted [DOCS].
- Errors: THICKEN_SELECT_ENTITIES, THICKEN_FAILED.

---

## 6. Enclose

- Toolbar: "Enclose". Command `createFeatureEnclose` [BUNDLE]. Available in: Part Studio.
- FS: `enclose`, no edit logic.

| # | UI label | param | type | options (default) | visible when | filter |
|---|---|---|---|---|---|---|
| 1 | Result body operation type | operationType | enum | **New** / Add / Remove / Intersect | always | HORIZONTAL_ENUM |
| 2 | Entities | entities | query | — | always | non-sketch sheet or solid bodies, OR faces (incl. planes, construction planes); mesh ok |
| 3 | Merge with all / Merge scope | defaultScope / booleanScope | | false | op != New | solids |
| 4 | Keep tools | keepTools | boolean | false | always | |

- Behavior [DOCS]: the selected boundaries must intersect or connect to bound a closed volume. Without Keep tools, the owning bodies of selections are deleted, except sketches and planes.
- Errors [FS]: ENCLOSE_NOTHING_SELECTED, ENCLOSE_NO_REGION, ENCLOSE_CANNOT_CREATE_SOLID, ENCLOSE_CANNOT_MERGE_REGIONS.

---

## 7. Fillet

- Toolbar: "Fillet". Command `createFeatureFillet` [BUNDLE]. **Shortcut: Shift+F**, which opens the Fillet dialog on Edge type [DOCS].
- Available in: Part Studio.
- FS: `fillet` (fillet.fs + edgeBlendCommon.fs). Editing logic `filletEditLogic`, manipulators `filletManipulatorChange`.
- Hidden: defaultsChanged (set true when the user edits radius/rho/magnitude/second radius, so the server can update remembered defaults); per-vertex `vertex` query (driven by the array item).

### 7.1 Fields
| # | UI label | param | type | options / range (default) | visible when | hints / filter |
|---|---|---|---|---|---|---|
| 1 | Fillet type | filletType | enum | **Edge** / Full round | always | HORIZONTAL_ENUM |
| 2 | Entities to fillet | entities | query | — | Edge | Filter: (non-sheet-metal two-sided edges OR faces) OR sheet-metal corner edges; non-construction, non-sketch, modifiable. Box select picks edges only |
| 3 | First side face | side1Face | query (1) | — | Full round | face, modifiable, non-sheet-metal |
| 4 | Second side face | side2Face | query (1) | — | Full round | same |
| 5 | Faces to round | centerFaces | query | — | Full round | faces |
| 6 | Tangent propagation | tangentPropagation | boolean | **true** | always | |
| 7 | Measurement | blendControlType | enum | **Radius** / Width | Edge | SHOW_LABEL, REMEMBER |
| 8 | Control | crossSection | enum | **Distance** (=CIRCULAR) / Conic / Curvature | Edge | SHOW_LABEL, REMEMBER. Description "Cross sectional control". (Hidden enum value CHAMFER) |
| 9 | Radius | radius | length | 1e-5..500 m, **5 mm / 0.2 in** | Edge, Radius, Control = Distance | REMEMBER, CAN_BE_TOLERANT |
| 9' | Radius | nonCircularRadius | length | same | Edge, Radius, Control = Conic or Curvature | REMEMBER |
| 9'' | Width | width | length | same | Edge, Measurement = Width | REMEMBER |
| 10 | Rho | rho | real | 0..0.99999, **0.5** | Edge, Control = Conic | |
| 10' | Magnitude | magnitude | real | 0..0.99999, **0.5** | Edge, Control = Curvature | |
| 11 | Asymmetric | isAsymmetric | boolean | false | Edge, Measurement = Radius | |
| 12 | Second radius | otherRadius | length | 5 mm / 0.2 in | Asymmetric | REMEMBER |
| 13 | Flip asymmetric | flipAsymmetric | boolean | false | Asymmetric | OPPOSITE_DIRECTION |
| 14 | Partial fillet | isPartial | boolean | false | Edge, Measurement = Radius | group "Partial fillet" |
| 15 | End type | startPartialType | enum | **Position** (=PERCENTAGE) / Entity / Offset. UI first value = Position; FS defaults map also PERCENTAGE | isPartial | SHOW_LABEL, REMEMBER |
| 16 | Start offset | startPartialOffset | length | 0..500 m, 5 mm / 0.2 in | End type = Offset | |
| 16' | Select an entity or mate connector | startPartialEntity | query (1) | — | End type = Entity | faces (mesh ok) OR ALLOWS_VERTEX (vertices, MCs) |
| 16'' | Start position | partialFirstEdgeTotalParameter | real | 0..1, **0.01** | End type = Position | |
| 17 | Opposite direction | partialOppositeParameter | boolean | **true** | isPartial | OPPOSITE_DIRECTION, DISPLAY_SHORT |
| 18 | Trim to face boundaries | useTrimmedFirstBound | boolean | false | End type = Entity | |
| 19 | Second bound | secondBound | boolean | false | isPartial | |
| 20 | End type | endPartialType | enum | **Position** / Entity / Offset | secondBound | SHOW_LABEL, REMEMBER |
| 21 | End offset | endPartialOffset | length | 5 mm / 0.2 in | 2nd End type = Offset | |
| 21' | Select an entity or mate connector | endPartialEntity | query (1) | — | 2nd End type = Entity | |
| 21'a | Trim to face boundaries | useTrimmedSecondBound | boolean | false | 2nd End type = Entity | |
| 21'' | End position | partialSecondEdgeTotalParameter | real | 0..1, **0.99** | 2nd End type = Position | |
| 22 | Variable fillet | isVariable | boolean | false | Edge (any measurement) | group "Variable fillet" |
| 23 | Vertices (array, item "vertex") | vertexSettings | array | [] | isVariable | PREVENT_ARRAY_REORDER. Item label "[radius] vertex". Item created by picking a vertex (Driven query) |
| 23a | Radius | vertexRadius | length | 0..500 m (zero allowed), 5 mm / 0.2 in | (item) | MATCH_LAST_ARRAY_ITEM |
| 23b | Second radius | vertexOtherRadius | length | 5 mm | (item) and Asymmetric | MATCH_LAST |
| 23c | Flip asymmetric | vertexFlipAsymmetric | boolean | false | (item) and Asymmetric | OPPOSITE_DIRECTION, MATCH_LAST |
| 23d | Rho / Magnitude | variableRho / variableMagnitude | real | 0.5 | (item), Control = Conic / Curvature | MATCH_LAST |
| 24 | Points on edges (array, item "point on edge") | pointOnEdgeSettings | array | [] | isVariable | PREVENT_ARRAY_REORDER, FOCUS_INNER_QUERY. Label "[radius] edge". Add button "Add point on edge" [DOCS] |
| 24a | Edge | edge | query (1) | — | (item) | edge, non-sketch, modifiable, non-construction |
| 24b | Location | edgeParameter | real | 0.001..0.999, **0.5** | (item) | |
| 24c | Radius | pointOnEdgeRadius | length | 5 mm | (item) | MATCH_LAST |
| 24d | Second radius / Flip asymmetric | pointOnEdgeOtherRadius / pointOnEdgeFlipAsymmetric | | | (item) and Asymmetric | MATCH_LAST |
| 24e | Rho / Magnitude | pointOnEdgeVariableRho / pointOnEdgeVariableMagnitude | real | 0.5 | (item), Conic / Curvature | MATCH_LAST |
| 25 | Smooth transition | smoothTransition | boolean | false | isVariable | |
| 26 | Allow edge overflow | allowEdgeOverflow | boolean | **true** | Edge, Control != Curvature | group "Edge overflow" (collapsed by default) |
| 27 | Edges to keep | keepEdges | query | — | allowEdgeOverflow (group expanded) | two-sided edges, modifiable |
| 28 | Smooth fillet corners | smoothCorners | boolean | false | Edge, Control != Curvature | group "Smooth fillet corners" |
| 29 | Corners to exclude | smoothCornerExceptions | query | — | smoothCorners | vertices, modifiable |
- Note: rows 26-29 are inside the "Edge" branch (FS nesting), so Full round shows only rows 1, 3-6.

### 7.2 Behavior [DOCS][FS]
- Measurement, Control, and the last radius are **remembered** for the next fillet. Docs: "Onshape remembers the last inputted radius value for circular fillets."
- **Manipulator**: an orange linear drag arrow on the first selected edge, along the bisector of the two face normals, sets radius or width. The drag range is inverted for convex edges.
  - Partial fillet: an orange point-on-edge arrow manipulator for Start/End position.
  - Variable fillet: a point-on-edge manipulator per "point on edge" (circle with arrows at each side).
- Variable fillet:
  - Available vertices show as **black dots** on the model, and selected ones turn **orange**.
  - The first added vertex or point copies the main radius, rho, magnitude, and second radius into the new item [FS updateVariableFilletParameters].
  - A per-point radius of 0 is allowed.
- Partial End type:
  - Position: fraction (shown as a percentage) of the edge from one vertex.
  - Entity: a plane, face, or (implicit) mate connector.
  - Offset: distance from the vertex.
  - Opposite direction picks which side of the start is kept. Second bound adds the end limit.
- Conic Rho: 0.25 elliptical, 0.5 parabolic, ~0.999 hyperbolic. Curvature Magnitude 0..0.999 adjusts tangency [DOCS].
- Allow edge overflow ON: the fillet may consume or modify adjacent edges for a smooth surface. OFF: those edges are preserved. "Edges to keep" exempts specific edges.
- Smooth fillet corners: rounds the vertex where filleted edges meet. "Corners to exclude" exempts some.
- Full round: replaces the "Faces to round" with one seamless blend between the two side faces. Only one contiguous face set at a time [DOCS].
- Sheet metal: picking a sheet-metal corner edge is allowed (filter), but docs recommend Corner break.
- Errors [FS]: FILLET_SELECT_EDGES, FILLET_FAILED, FILLET_PARTIAL_FAIL, FILLET_OVERLAP, FILLET_EDGE_OVERLAPPED_BY_FILLET, FILLET_RHO_TOO_LARGE, FILLET_ADJOINING_EDGE_NOT_FILLETED, FILLET_EDGES_NOT_MANIFOLD, FILLET_FAIL_SMOOTH, FILLET_REQUIRES_SURFACE_EXTENSION, FILLET_LEGACY_ASYMMETRIC_UNSUPPORTED, FILLET_CHAMFER_UNSUPPORTED.

---

## 8. Chamfer

- Toolbar: "Chamfer". Command `createFeatureChamfer` [BUNDLE]. Available in: Part Studio.
- FS: `chamfer`. No edit logic, no manipulators.

### 8.1 Fields
| # | UI label | param | type | options / range (default) | visible when | hints / filter |
|---|---|---|---|---|---|---|
| 1 | Entities to chamfer | entities | query | — | always | as Fillet row 2 (two-sided edges or faces, or SM corner edges). Box select = edges |
| 2 | Measurement | chamferMethod | enum | **Offset** (FACE_OFFSET) / Tangent (APEX_RANGE) | always | SHOW_LABEL, REMEMBER |
| 3 | Chamfer type | chamferType | enum | **Equal distance** / Two distances / Distance and angle (hidden: RAW_OFFSET) | always for new features (legacy features may lack it) | SHOW_LABEL, REMEMBER |
| 4 | Distance | width | length | 1e-5..500 m, **5 mm / 0.2 in** | type != Two distances | REMEMBER, CAN_BE_TOLERANT |
| 4' | Distance 1 | width1 | length | 5 mm / 0.2 in | type = Two distances | CAN_BE_TOLERANT |
| 5 | Opposite direction | oppositeDirection | boolean | false | type in {Distance and angle, Two distances} | OPPOSITE_DIRECTION |
| 6 | Distance 2 | width2 | length | 5 mm / 0.2 in | Two distances | CAN_BE_TOLERANT |
| 6' | Angle | angle | angle | 0.1..179.9 deg, **45 deg** | Distance and angle | CAN_BE_TOLERANT |
| 7 | Direction overrides | directionOverrides | query | — | Distance and angle or Two distances | two-sided edges or SM corner edges |
| 8 | Tangent propagation | tangentPropagation | boolean | **true** (UI annotation; FS defaults map false) | always | |

### 8.2 Behavior [DOCS]
- Measurement only matters for non-planar adjacent faces:
  - Offset: distance measured from the starting edge. Order-dependent: computation starts at the first selection and chains on.
  - Tangent: distance from the intersection of the two faces' tangents. Not order-dependent.
- Remembered: chamfer type, and the last distance for Equal distance and Distance and angle. Angle defaults to 45 deg.
- Distance and angle: distance is measured along one face and the angle from it. The arrow flips which face is used, for all edges. Direction overrides flips individual edges.
- Errors [FS]: CHAMFER_SELECT_EDGES, CHAMFER_FAILED, CHAMFER_DIRECTION_OVERRIDE_NO_EFFECT, CHAMFER_OVERLAP, CHAMFER_HELD_BACK, CHAMFER_PARTIAL_FAIL.

---

## 9. Face blend

- Toolbar: "Face blend". Command `createFeatureFaceBlend` [BUNDLE]. Available in: Part Studio.
- FS: `faceBlend`. Manipulators `faceBlendManipulatorChanges` (flip arrows).

### 9.1 Fields
| # | UI label | param | type | options / range (default) | visible when | hints / filter |
|---|---|---|---|---|---|---|
| 1 | Side 1 | side1 | query | — | always | faces, modifiable, non-sketch, non-sheet-metal |
| 2 | Flip side 1 | flipSide1 | boolean | false | always | OPPOSITE_DIRECTION |
| 3 | Side 2 | side2 | query | — | always | same |
| 4 | Flip side 2 | flipSide2 | boolean | false | always | OPPOSITE_DIRECTION |
| 5 | Propagation | propagation | boolean | **true** | always | group "Propagation" (expanded) |
| 6 | Propagation type | propagationType | enum | **Tangent** / Adjacent / Custom | propagation | |
| 7 | Maximum angle | propagationAngle | angle | 0..180 deg, 0 | type = Custom | |
| — | Group "Cross section" (expanded) | | | | | |
| 8 | Cross section | crossSection | enum | **Rolling ball** / Swept profile | always | SHOW_LABEL, REMEMBER |
| 9 | Spine | spine | query (1) | — | Swept profile | edge |
| 10 | Measurement | blendControlType | enum | **Radius** / Width | always | SHOW_LABEL, REMEMBER |
| 11 | Radius | radius | length | 5 mm / 0.2 in | Radius | REMEMBER |
| 11' | Width | width | length | 5 mm / 0.2 in | Width | REMEMBER |
| 12 | Asymmetric | asymmetric | boolean | false | always | |
| 13 | Second radius | secondRadius | length | 5 mm | Asymmetric and Radius | REMEMBER |
| 13' | Ratio | widthRatio | real | 0.0001..10000, **1** | Asymmetric and Width | REMEMBER |
| 14 | Switch side | switchAsymmetric | boolean | false | Asymmetric | OPPOSITE_DIRECTION |
| 15 | Control | crossSectionShape | enum | **Circular** / Conic / Curvature / Chamfer | always | SHOW_LABEL, REMEMBER |
| 16 | Rho | rho | real | 0..0.99999, 0.5 | Conic | |
| 16' | Magnitude | magnitude | real | 0.5 | Curvature | |
| — | Group "Constraints and limits" (**collapsed** by default) | | | | | |
| 17 | Tangent hold lines | tangentHoldLines | boolean | false | always | |
| 18 | Tangent edges | tangentEdges | query | — | tangentHoldLines | edges (non-SM, non-sketch) |
| 19 | Inverse tangent edges | inverseTangentEdges | query | — | tangentHoldLines | same |
| 20 | Conic hold lines | conicHoldLines | boolean | false | always | |
| 21 | Conic edges | conicEdges | query | — | conicHoldLines | edges |
| 22 | Inverse conic edges | inverseConicEdges | query | — | conicHoldLines | edges |
| 23 | Cliff edges (checkbox) | hasCliffEdges | boolean | false | always | |
| 24 | Cliff edges | cliffEdges | query | — | hasCliffEdges | edges |
| 25 | Caps (checkbox) | hasCaps | boolean | false | always | |
| 26 | Caps (array, item "cap") | caps | array | — | hasCaps | label "#entity" |
| 26a | Cap entity | entity | query (1) | — | (item) | face |
| 26b | Flip cap | capFlip | boolean | false | (item) | OPPOSITE_DIRECTION |
| 27 | Limits | hasLimits | boolean | false | always | |
| 28 | Plane limit | limitPlane1 | query (1) | — | hasLimits | planar geometry |
| 29 | Flip first plane | limitPlane1Flip | boolean | false | hasLimits | OPPOSITE_DIRECTION |
| 30 | Second plane limit | limitPlane2 | query (1) | — | hasLimits | planar |
| 31 | Flip second plane | limitPlane2Flip | boolean | false | hasLimits | OPPOSITE_DIRECTION |
| 32 | Face limits | faceLimits | query | — | hasLimits | faces, modifiable |
| 33 | Edge limits (array, item "edge limit") | edgeLimits | array | — | hasLimits | label "#edge" |
| 33a | Edge limit | edge | query (1) | — | (item) | edge |
| 33b | Side | edgeLimitSide | query (1) | — | (item) | face |
| 34 | Help point (checkbox) | hasHelpPoint | boolean | false | always | |
| 35 | Help point | helpPoint | query (1) | — | hasHelpPoint | vertex OR mate connector |
| 36 | Trim type | trim | enum | **Walls** / Short / Long / No trim | always | SHOW_LABEL, REMEMBER |
| 37 | Detached | detach | boolean | false | always | |
| 38 | Show isocurves | showIsocurves | boolean | false | always | |
| 39 | Count | curveCount | integer | 1..50, 10 | showIsocurves | |

### 9.2 Behavior [DOCS][FS]
- Unlike Fillet, the sides need not touch. Side 1 is one or more faces of one body, and Side 2 can be on another body. The flip arrows choose which side of each face set the blend sits on.
- **Graphics flip-arrow manipulators** for Side 1, Side 2, each Cap, Plane limit, and Second plane limit. Clicking an arrow toggles the matching boolean.
- Propagation: Tangent extends to tangent faces. It doesn't work with Swept profile (FACE_BLEND_SPINE_TANGENT_PROPAGATION). Adjacent extends to all adjacent faces. Custom extends to adjacent faces below the max angle.
- Rolling ball is fillet-like. Swept profile keeps the cross-section orthogonal to the spine edge.
- Control Chamfer = linear bevel. Curvature magnitude 0..0.999.
- Tangent hold lines clamp symmetrically, so they can't be combined with Asymmetric (FACE_BLEND_TANGENT_HL_ASYMMETRIC). Conic hold lines clamp one side only.
- Hold lines and cliff edges must be edges on the Side 1/2 faces, not sketch edges. Split the face first if needed.
- Cliff edges clamp the blend to the edge instead of extending the face.
- Caps close blend ends with a face or plane, and several are allowed.
- Edge limit needs a Side face (FACE_BLEND_EDGE_LIMIT_NEEDS_SIDE).
- Help point keeps only the blend closest to the point when several are generated.
- Trim:
  - Walls: trims to the walls of both sides.
  - Short / Long: the shortest / longest blend between the walls.
  - No trim: extends past both walls.
  - Long and No trim suit support surfaces.
- Detached: the blend becomes a new surface body (in the Parts list) instead of attaching. This resolves self-intersection and attach failures (FACE_BLEND_CANNOT_ATTACH).
- Remembered: cross section, measurement, constraints and limits, trim type [DOCS].
- Errors [FS]: FACE_BLEND_SELECT_FACES, FACE_BLEND_SELECT_SPINE, FACE_BLEND_BAD_SPINE, FACE_BLEND_DIMENSION_TOO_LARGE/SMALL, FACE_BLEND_WRONG_LEFT/RIGHT_WALL_SENSE, FACE_BLEND_LEFT/RIGHT_WALL_MULTIPLE_BODIES, FACE_BLEND_INVALID_CAP, FACE_BLEND_INVALID_PLANE_LIMIT, FACE_BLEND_SELF_INTERSECTION, FACE_BLEND_RHO_TOO_LARGE.

---

## 10. Modify fillet

- Toolbar: "Modify fillet" (Direct-editing group). Command `createFeatureModifyFillet` [BUNDLE]. Available in: Part Studio.
- FS: `modifyFillet`. Editing logic `modifyFilletLogic`.

| # | UI label | param | type | options (default) | visible when | filter |
|---|---|---|---|---|---|---|
| 1 | Fillet faces to modify | faces | query | — | always | **non-planar** faces, modifiable, non-sketch |
| 2 | Modification type | modifyFilletType | enum | **Change radius** / Remove fillet | always | dropdown |
| 3 | Fillet radius | radius | length | 1e-5..500 m, 5 mm / 0.2 in (but auto-filled) | Change radius | |
| 4 | Reapply fillet | reFillet | boolean | **true** (UI) | Change radius | |

- Behavior:
  - When faces are first picked and the user hasn't typed a radius, **Fillet radius auto-fills with the measured radius** of the selected fillet face (evFilletRadius) [FS].
  - Reapply fillet keeps downstream fillets flowing [DOCS].
  - For long fillet chains, use the context-menu "Create selection" to grab all faces [DOCS].
  - Intended for imported or history-less parts.

---

## 11. Draft

- Toolbar: "Draft". Command `createFeatureDraft` [BUNDLE]. Available in: Part Studio. (Draft analysis is a separate tool.)
- FS: `draft`. Editing logic `draftEditLogic`, manipulators `draftManipulatorChange`.
- Hidden: hintFaces (auto-computed faces adjacent to the parting edges).

| # | UI label | param | type | options / range (default) | visible when | hints / filter |
|---|---|---|---|---|---|---|
| 1 | Draft type | draftFeatureType | enum | **Neutral plane** / Parting line | always | HORIZONTAL_ENUM |
| 2 | Neutral plane | neutralPlane | query (1) | — | Neutral plane | ALLOWS_PLANE (planes, planar faces, mate connectors) |
| 3 | Entities to draft | draftFaces | query | — | Neutral plane | faces, modifiable. INITIAL_FOCUS_ON_EDIT |
| 4 | Pull direction | pullDirectionEntity | query (1) | — | Parting line | ALLOWS_DIRECTION |
| 5 | Parting edges | partingEdges | query | — | Parting line | edges of solid/sheet, non-sketch, modifiable. SHOW_CREATE_SELECTION, INITIAL_FOCUS_ON_EDIT |
| 6 | Sides | partingLineSides | enum | **One sided** / Symmetric / Two sided | Parting line | dropdown |
| 7 | Switch face | alongPull | boolean | **true** | Parting line, One sided | OPPOSITE_DIRECTION |
| 8 | Draft angle | angle | angle | 0..89.9 deg, **3 deg** | always | REMEMBER |
| 9 | Opposite direction | pullDirection | boolean | false | always | OPPOSITE_DIRECTION_CIRCULAR |
| 10 | Second draft angle | secondAngle | angle | 3 deg | Parting line, Two sided | REMEMBER |
| 11 | Opposite direction | secondPullDirection | boolean | **true** | Parting line, Two sided | OPPOSITE_DIRECTION_CIRCULAR |
| 12 | Tangent propagation | tangentPropagation | boolean | **true** | Neutral plane | |
| 12' | Parting line propagation | referenceEdgePropagation | boolean | **true** | Parting line | |
| 13 | Reapply fillet | reFillet | boolean | false | always | |

- Behavior [DOCS][FS]:
  - **Manipulators**: an orange angle arrow (angular manipulator) for the draft angle, a second one for Two sided or Symmetric, and a flip manipulator.
  - Tangent propagation adds only tangent faces **steeper than the draft angle**. Non-steep fillets are always reapplied.
  - Reapply fillet ON: steep fillet faces are treated as draft faces and become cones (often undesirable at large angles). OFF: they are re-blended as fillets (cylinders).
  - Parting line needs an existing parting edge, typically made with Split > Face.
  - Edit logic auto-computes hint faces and auto-sets Switch face / flips when sides change.
  - Errors: DRAFT_SELECT_NEUTRAL, DRAFT_SELECT_FACES, DRAFT_SELECT_PULL_DIRECTION_ENTITY, DRAFT_SELECT_PARTING_EDGES, DRAFT_NO_NEUTRAL_PLANE, DRAFT_PARALLEL_PARTING_EDGE, DRAFT_CONFLICTING_OPPOSITION, DRAFT_FAILED.

---

## 12. Body draft

- Toolbar: "Body draft". Command `createFeatureBodyDraft` [BUNDLE]. Available in: Part Studio. Mobile can only view and edit it [DOCS].
- FS: `bodyDraft`. Editing logic `bodyDraftEditLogic`, manipulators `bodyDraftManipulatorChange` (pull-direction flip arrow).
- Hidden: preselection (body, face, or edge preselection routed into the right field on open).

| # | UI label | param | type | options / range (default) | visible when | hints / filter |
|---|---|---|---|---|---|---|
| 1 | Draft scope | selectionType | enum | Parts to draft / Faces to draft / Edges to hold. UI default = first value (Parts to draft). FS defaults map = EDGES. Edit logic sets it from the preselection | always | SHOW_LABEL, REMEMBER |
| 2 | Edges | topEdges | query | — | Edges to hold | solid edges, modifiable, non-sheet-metal |
| 2' | Faces | faces | query | — | Faces to draft | solid faces |
| 2'' | Parts | bodies | query | — | Parts to draft | solid bodies |
| 3 | Faces to exclude | excludeFaces | query | — | Parts to draft | solid faces |
| 4 | Draft on self | draftOnSelf | boolean | false | always | |
| 5 | Pull direction | pullDirection | query (1) | — | always | ALLOWS_PLANE or ALLOWS_DIRECTION |
| 6 | Flip pull direction | flipPullDirection | boolean | false | always | OPPOSITE_DIRECTION |
| 7 | Parting entity | partingObject | query (1) | — | not Draft on self | non-sketch sheet body, OR any face, OR mate connector |
| 8 | Angle | angle | angle | 0..89.9 deg, **3 deg** | always | |
| 9 | Symmetric | bothSides | boolean | false | always | |
| 10 | Bottom edges | bottomEdges | query | — | Symmetric and Edges to hold | edges |
| 11 | Match faces at parting | matchFacesAtParting | boolean | **true** (annotation; FS map false) | Symmetric and not Draft on self | group "Match face options" (collapsed) |
| 12 | Match type | matchFaceType | enum | **Tangent to face** / Reference edge | matchFacesAtParting | SHOW_LABEL |
| 13 | Concave repair | concaveRepair | enum | **None** / Radius / Mix | matchFacesAtParting | SHOW_LABEL |
| 14 | Radius | concaveRepairRadius | length | 5 mm / 0.2 in | concaveRepair != None | |
| 15 | Corner type | cornerType | enum | **Extend** / Plane | always | SHOW_LABEL |
| 16 | Keep material | keepMaterial | boolean | false | always | |

- Behavior [DOCS][FS]:
  - Body draft always **adds** material below the held edges and attaches it back. It handles geometry Draft can't (e.g. a torus).
  - Scope "Parts" = hold all edges of the part. "Faces" = hold all edges of the face. "Edges" = explicit.
  - **Pull direction and Parting entity cross-fill**: picking a plane for one auto-fills the other if it's empty and untouched. Edges are never copied into Parting entity.
  - Draft on self removes the Parting entity field.
  - Symmetric drafts both sides of the parting entity. Match faces adds material so both sides meet at the parting line.
    - Match type Reference edge: draft up to the reference edge.
    - Match type Tangent to face: until tangent to the part face.
    - Concave repair Radius blends tight corners. Mix makes a straight region into a radius.
  - Corner Extend: faces extended until they meet. Plane: a new plane joins the limits.
  - Material the draft passes through is removed unless Keep material is on.
  - Errors: BODY_DRAFT_SELECT_PARTS/FACES/EDGES, BODY_DRAFT_INVALID_PULL_DIRECTION, BODY_DRAFT_INVALID_PARTING_ENTITY, BODY_DRAFT_TOO_STEEP, BODY_DRAFT_NO_EFFECT, BODY_DRAFT_MITER_NEED_BOTH_SIDES, BODY_DRAFT_FAILED.

---

## 13. Rib

- Toolbar: "Rib". Command `createFeatureRib` [BUNDLE]. Available in: Part Studio.
- FS: `rib`. Editing logic `ribEditLogic`.

| # | UI label | param | type | options / range (default) | visible when | hints / filter |
|---|---|---|---|---|---|---|
| 1 | Sketch profiles | profiles | query | — | always | sketch edges, non-construction |
| 2 | Parts | parts | query | — | always | modifiable solid bodies (mesh ok) |
| 3 | Thickness | thickness | length | 1e-5..500 m, **2.5 mm / 0.1 in** | always | |
| 4 | Rib extrusion direction | ribExtrusionDirection | enum | **Parallel to sketch plane** / Normal to sketch plane | always | dropdown |
| 5 | Opposite direction | oppositeDirection | boolean | **true** | always | OPPOSITE_DIRECTION |
| 6 | Draft (checkbox) | hasDraft | boolean | false | always | DISPLAY_SHORT, FIRST_IN_ROW |
| 7 | Draft angle | draftAngle | angle | 0..89.9, 3 deg | hasDraft | DISPLAY_SHORT |
| 8 | Opposite direction | draftOpposite | boolean | false | hasDraft | OPPOSITE_DIRECTION_CIRCULAR |
| 9 | Extend profiles to part | extendProfilesUpToPart | boolean | false | always | |
| 10 | Merge ribs | mergeRibs | boolean | **true** | always | |

- Behavior [FS ribEditLogic][DOCS]:
  - **Auto part selection and direction**: while Parts is untouched, the rib is test-built in both directions against all visible modifiable solids.
    - It hits parts in exactly one direction: Parts is set to those, and the flip to match.
    - Both or neither: Parts stays empty.
    - If the user flips the direction with Parts still empty, Parts is recomputed for that direction.
  - The rib must lie entirely within the part bounds, or it fails.
  - Extend profiles: lines are extended, and arcs get tangent straight extensions until they hit the part.
  - Merge ribs off: each rib becomes a separate new part.
  - Draft tapers both sides.
  - Errors: RIB_NO_PROFILES, RIB_NO_PARTS, RIB_NO_INTERSECTIONS, RIB_ONLY_OPEN_PROFILES, RIB_MERGE_FAILED.

---

## 14. Shell

- Toolbar: "Shell". Command `createFeatureShell` [BUNDLE]. Available in: Part Studio.
- FS: `shell`.

| # | UI label | param | type | options / range (default) | visible when | filter |
|---|---|---|---|---|---|---|
| 1 | Hollow | isHollow | boolean | false | always | |
| 2 | Faces to remove | entities | query | — | not Hollow | solid faces, modifiable, non-sheet-metal, mesh ok |
| 2' | Parts to hollow | parts | query | — | Hollow | solid bodies |
| 3 | Shell thickness | thickness | length | 1e-5..500 m, **2.5 mm / 0.1 in** | always | |
| 4 | Opposite direction | oppositeDirection | boolean | false | always | OPPOSITE_DIRECTION (arrow beside thickness) |

- Behavior [DOCS]:
  - Default: the wall grows inward from the part faces. The arrow makes the part face the inside of the wall, so the shell grows outward.
  - Hollow removes no faces and leaves an internal void, visible with section view or hidden edges.
  - Errors: SHELL_SELECT_FACES, SHELL_SELECT_PARTS, SHELL_FAILED.

---

## 15. Hole

- Toolbar: "Hole". Command `createFeatureHole` [BUNDLE]. Available in: Part Studio. Works on active sheet metal too, but not on flex-PCB (HOLE_NOT_SUPPORTED_FOR_PCB).
- FS: `hole` (hole.fs, holeUtils.fs, holeAttribute.fs, holetables.gen.fs). Editing logic `holeEditLogic`. Feature name template `#featureName`, so the feature-list name auto-updates.
- New features always use isV2 = true and holeVersion = **V3** (V3 parameter set with inline tolerances). The legacy (V1) parameter set is kept only for old features. It is summarized at the end.
- Hidden params:
  - isV2, holeVersion, initEntities (preselected sketch points/MCs moved into Locations on open), featureName.
  - threadStandard: UNSET/ANSI/ISO. Set by logic when the hole type is a straight tap.
  - hasClearance: true when a tapped hole's Fastener fit != None.
  - locationSignatures, showTappedDepth (true when the table entry has a thread pitch), majorDiameter, isMultiple, tappedAngle, toleranceMatching.

### 15.1 Fields (V2/V3, the current dialog), in order
| # | UI label | param | type | options / range (default) | visible when | hints / filter |
|---|---|---|---|---|---|---|
| 1 | Units | unitsSystem | enum | **Inch** / Metric | always | HORIZONTAL_ENUM, REMEMBER, UNCONFIGURABLE |
| 2 | Style | styleV2 (synced to `style`) | enum | **Simple** / Counterbore / Countersink | always | HORIZONTAL_ENUM, REMEMBER, UNCONFIGURABLE |
| 3 | Sketch points to place holes | locations | query | — | always | sketch vertices (modifiable) OR mate connectors. INITIAL_FOCUS. Box select ok |
| 4 | Merge scope | scope | query | — | always | modifiable solid bodies (sheet metal ok, not flex PCB) |
| 5 | Standard | ansiHoleTableEx (Inch+Simple) / ansiHoleTable (Inch+C'bore/C'sink) / isoHoleTableEx (Metric+Simple) / isoHoleTable (Metric+C'bore/C'sink) | lookup table (cascade) | see 15.3 | always (V2) | REMEMBER, UNCONFIGURABLE |
| 6 | Thread class (checkbox) | showThreadClassV2 | boolean | false | threadStandard != UNSET (straight-tap hole type) | REMEMBER. Group "Thread class" |
| 7 | Thread class | ansiThreadClassV2 / isoThreadClassV2 | lookup | ANSI: 1B / **2B** / 3B. ISO: 4H / 5G / 5H / 6G / **6H** / 7G / 7H / 8G / 8H | checkbox on | REMEMBER, UNCONFIGURABLE |
| 8 | Diameter | holeDiameterV3 | length | 1e-5..500 m, **5 mm / 0.25 in**. Overwritten from the table on size change | always | REMEMBER, SHOW_EXPRESSION, CAN_BE_TOLERANT_DIAMETER |
| 9 | Tap drill diameter | tapDrillDiameterV3 | length | from table | hasClearance (tapped hole with Fastener fit != None) | REMEMBER, SHOW_EXPRESSION, CAN_BE_TOLERANT |
| 10 | Start plane | startStyle | enum | **Start from part** / Start from sketch plane / Start from selected plane | always | REMEMBER, SHOW_LABEL |
| 11 | Hole start plane or mate connector | startBoundEntity | query (1) | — | Start from selected plane | planar face OR mate connector |
| 12 | Termination | endStyleV2 | enum | **Blind** / Up to next / Up to entity / Through all | always | REMEMBER, SHOW_LABEL |
| 13 | Opposite direction | oppositeDirection | boolean | false (auto-set by heuristics) | always | OPPOSITE_DIRECTION |
| 14 | Up to entity or mate connector | endBoundEntity | query (1) | — | Up to entity | faces (non-sketch, mesh ok) OR ALLOWS_VERTEX |
| 15 | Offset from tip (checkbox) | offset | boolean | false | Up to next or Up to entity | DISPLAY_SHORT, FIRST_IN_ROW |
| 16 | Offset from tip | offsetDistance | length | 0..500 m, 5 mm / 0.25 in | offset | DISPLAY_SHORT |
| 17 | Opposite direction | oppositeOffsetDirection | boolean | false | offset | OPPOSITE_DIRECTION |
| 18 | Counterbore diameter | cBoreDiameterV3 | length | **10 mm / 0.5 in** (table) | Style = Counterbore | REMEMBER, SHOW_EXPRESSION, CAN_BE_TOLERANT |
| 19 | Counterbore depth | cBoreDepthV3 | length | 0..500 m, **5 mm / 0.25 in** (table) | Counterbore | same |
| 18' | Countersink diameter | cSinkDiameterV3 | length | 10 mm / 0.5 in (table) | Style = Countersink | same |
| 19' | Countersink angle | cSinkAngleV3 | angle | 0.1..179.9 deg, **90 deg** (table: 82 deg ANSI, 90 deg ISO) | Countersink | REMEMBER, CAN_BE_TOLERANT |
| 20 | Depth | holeDepthV3 | length | **12 mm / 0.5 in** | Termination = Blind | REMEMBER, CAN_BE_TOLERANT |
| 20' | Depth (read-only, computed) | holeDepthComputedV3 | length | computed | Up to next / Up to entity (or several locations) | READ_ONLY, CAN_BE_TOLERANT |
| 21 | Tip angle style | tipAngleStyle | enum | **118 deg** / 135 deg / Flat / Custom | Termination != Through all | SHOW_LABEL |
| 22 | Tip angle | tipAngleV3 | angle | 0.1..180 deg, **118 deg** | Tip angle style = Custom | REMEMBER, CAN_BE_TOLERANT |
| 23 | Tap through all | isTappedThrough | boolean | **true** | threaded hole (showTappedDepth) and Through all | REMEMBER |
| 24 | Tapped depth | tappedDepthV3 | length | 12 mm / 0.5 in (adjusted by logic) | threaded and (not Through all, or Tap through all off) | REMEMBER, CAN_BE_TOLERANT |
| 25 | Tap clearance (number of thread pitch lengths) | tapClearance | real | 0..500, **3** | threaded and Blind | REMEMBER |

### 15.2 Behavior [DOCS][FS]
- **Locations**: sketch points, line ends, spline points, circle centers, and (implicit) mate connectors.
  - Clicking a **sketch in the feature list selects all its non-construction points**.
  - Coincident or duplicate vertices are clustered into one location [FS clusterVertexQueries].
  - Limit **500 locations** (HOLE_EXCEEDS_MAX_LOCATIONS_500). Older versions were limited to 100.
- **Merge scope and direction heuristics** [FS holeScopeFlipHeuristicsCall]: Locations, start plane, depth/termination, and flip changes trigger a raycast from each location.
  - The raycast fills Merge scope with the bodies hit and chooses the flip direction.
  - Adding locations only appends targets. Other changes recompute scope and flip from scratch.
  - With one part, the scope auto-populates [DOCS].
- **Start plane**:
  - Start from part: the hole start moves to the part surface, in the hole direction only. Good starting depth on curved surfaces.
  - Start from sketch plane: begins exactly at the sketch plane. Counterbore/countersink depth is measured from there, so it can be shortened.
  - Start from selected plane: plane or MC.
- **Termination**:
  - Blind: depth includes the counterbore/countersink.
  - Up to next: the first face hit, with optional Offset from tip.
  - Up to entity: the full diameter reaches the entity, so a non-flat tip extends past it. Offset from tip compensates.
  - Through all: through every part in Merge scope.
  - Depth is read-only (computed) for Up to next/entity. With multiple locations it reads "Multiple" in legacy.
- **Tapped depth readjustment**: for Up to next/entity, if Tapped depth > computed hole depth, it's clamped to the hole depth and a message shows (HOLE_PARAMS_OVERRIDDEN_INFO style).
- **Multi-part tapped**: with 2+ parts in Merge scope and Tapped > Fastener fit != None, the first parts get **clearance** holes and the **last** part is tapped (replaces the legacy "Blind in last" termination).
- **Table-driven values**:
  - Changing hole type, size, pitch, or fit rewrites Diameter, Tap drill, Counterbore/Countersink values and depths from the table entry [FS updateHoleDefinitionWithStandard].
  - Users may override any value.
  - "If a standard does not provide a counterbore, countersink or tap diameter, these values will be reset if they are out of range" [DOCS].
- **Cosmetic threads**: tapped holes show cosmetic thread display in the Part Studio and can get Hole/thread callouts in drawings. No thread geometry is modeled.
- **Feature name auto-template** [FS generateFeatureNameTemplate], examples:
  - Simple blind: `Ø 5 mm ↧ 12 mm`.
  - Through: `Ø 5 mm THRU`.
  - Tapped: `<size>-<pitch> ...` (e.g. `1/4-20 ...`), with `NPT` or `RC TAPPED HOLE` for pipe taps.
  - Counterbore suffix ` | ⌴Ø <cbore dia> ↧ <cbore depth>`. Countersink suffix ` | ⌵Ø <csink dia> X <angle>°`.
  - Renaming: pencil icon on the Name field. Clearing the name restores the template [DOCS].
- **Hole table panel**: once any Hole exists, a Hole table icon appears in the right-side panel strip.
  - The table lists all holes. Hovering a row highlights those holes, and clicking selects or deselects them.
  - The overflow menu has "Copy table" (to clipboard). The column context menu can move left/right, hide, or restore columns.
  - External threads aren't listed [DOCS hole_table, external_thread].
- Hole callouts in drawings only attach to Hole-feature holes, not extruded cuts. Standard-content autosize reads hole data [DOCS].
- Collision or overlap, or a hole not lying fully on the target, gives a zero-depth hole relative to the sketch plane [DOCS].
- Errors [FS]: HOLE_NO_POINTS, HOLE_EMPTY_SCOPE, HOLE_NO_HITS, HOLE_DISJOINT, HOLE_CBORE_TOO_SMALL/TOO_DEEP, HOLE_CSINK_TOO_SMALL/TOO_DEEP, HOLE_CSINK_ANGLE_TOO_NARROW/WIDE, HOLE_TAP_DIA_TOO_LARGE, HOLE_TAP_TOO_DEEP, HOLE_FASTENER_FIT_IS_NOT_APPLICABLE, HOLE_CBORE_CSINK_VALUES_NON_STD, HOLE_START_BOUND_INVALID, HOLE_DESTROY_SOLID, HOLE_PARTIAL_FAILURE, HOLE_CUT_FAIL.

### 15.3 Hole standard / size tables (holetables.gen.fs) [FS][DOCS]
- A LookupTablePath renders as **one dropdown per table level**. Each level is `{ name, displayName, default, entries }`, and the leaves are value records. Each level's default comes from `"default"`, else the first entry.
- **Top level by Units + Style** (Hole type dropdown, `holeType`):
  - Inch, Simple: Drilled / Clearance / Tapped / **PEM®**.
  - Inch, Counterbore/Countersink: Drilled / Clearance / Tapped (no PEM).
  - Metric: the same, with ISO tables.
  - No explicit default, so the first entry, Drilled.
- **Drilled** to `Drill size`:
  - ANSI: number/letter/fraction drills, e.g. "#80 (0.0135)" ... "1/4 (0.25)" (default) ...
  - ISO: mm sizes "0.05" ... "5" (default) ...
- **Clearance** to `Size` to `Fastener fit`:
  - ANSI sizes "#0" ... "1/4" (default) ...; fits **Close / Free / Close (ASME) / Normal (ASME) / Loose (ASME)**.
  - ISO sizes "M1" ... "M5" (default) ...; fits **Close / Normal / Loose**.
- **Tapped** to `Tap type`:
  - ANSI: **Straight tap** (default) / Tapered Pipe Tap.
  - ISO: Straight tap / **Straight Pipe Tap** (to `Thread type` G (default) / RP) / Tapered Pipe Tap.
  - Straight tap to Size (default 1/4 or M5) to `Threads/inch` (ANSI, e.g. "20 tpi (UNC)", "28 tpi (UNF)") or `Pitch` (ISO, e.g. "0.80 mm (Coarse)", "0.50 mm (Fine)") to `Fastener fit`: **None** (default) / Close / Free / Close (ASME) / Normal (ASME) / Loose (ASME) (ISO: None / Close / Normal / Loose).
  - Docs: Fastener fit for tapped is meaningful only with 2+ parts in scope (HOLE_FASTENER_FIT_IS_NOT_APPLICABLE otherwise).
  - Tapered pipe tap: Size to Threads/inch. The leaf also supplies tappedDepth and holeDepth.
- **PEM®** (Simple only):
  - Inch → Unified tables, Metric → Metric tables.
  - `Fastener`: Self-clinching nuts (default) / Self-clinching standoffs / Self-clinching studs & pins.
  - Then `Fastener type`, e.g. S/SS/CLS/CLSS/SP nuts, CLA, S-RT, SL, H/HNL, SH, SMPS/SMPP.
  - Then size and thread levels. Fit = Clearance.
- **Leaf record fields**: holeDiameter, tapDrillDiameter, cBoreDiameter, cBoreDepth, cSinkDiameter, cSinkAngle (ANSI 82 deg, ISO 90 deg), majorDiameter, and for pipe taps tappedDepth and holeDepth. Values are strings with units, e.g. "0.0635 in".
- **Thread class tables**: ANSI 1B/2B/3B (default 2B). ISO 4H/5G/5H/6G/6H/7G/7H/8G/8H (default 6H).
- **Legacy (V1) table** `tappedOrClearanceHoleTable`:
  - `Standard`: Custom / ANSI / ISO / DIN / PEM®.
  - Then `Hole type`: Clearance (default) / Tapped / Drilled / Tapered Pipe Tap (+ Straight Pipe Tap for ISO).
  - Tapped goes Size to Threads/inch|Pitch to `% diametric engagement` (50% / **75%**).
  - Legacy termination enum adds **Blind in last**, with its own table (Hole type: Clearance & tapped / Drilled).
  - Legacy dialog order: Style (no Units), Locations, Merge scope, Start plane, Termination, Standard, Thread class, Diameter, ..., Depth showing "Multiple" (read-only) for multi-location up-to holes.
- External thread tables (15.3-like): `Standard`: ANSI / ISO, then Size (default 1/4 or M5), then `Threads/inch` (ANSI, "20 tpi") or `Pitch` (ISO, "0.8 mm"). Leaf: holeDiameter, minorDiameter, majorDiameter.

---

## 16. External thread

- Toolbar: "External thread" (usually grouped with Hole). Command `createFeatureExternalThread` [BUNDLE]. Available in: Part Studio.
- FS: `externalThread`. Editing logic `editFeatureLogicExternalThread`.

| # | UI label | param | type | options / range (default) | visible when | hints / filter |
|---|---|---|---|---|---|---|
| 1 | Cylinder edges | entities | query | — | always | circle/arc edges, two-sided, non-sketch, non-construction, modifiable, **no mesh**, non-sheet-metal. UNCONFIGURABLE |
| 2 | Standard | branchOfStandard | lookup table | Standard: ANSI / ISO, then Size, then Threads/inch or Pitch (see 15.3) | always | REMEMBER, UNCONFIGURABLE |
| 3 | End type | depthType | enum | **Blind** / Up to next | always | REMEMBER |
| 4 | Length | threadDepth | length | 0.1 mm..200 m, **10 mm / 0.5 in** | Blind | REMEMBER |
| 5 | Add chamfer | addChamfer | boolean | false | always | group "Chamfer" |
| 6 | Length | chamferWidth | length | auto (see below) | addChamfer | REMEMBER |
| 7 | Angle | chamferAngle | angle | 0.1..89 deg, **30 deg** | addChamfer | REMEMBER |
| 8 | Add undercut | addUndercut | boolean | false | always | group "Undercut" |
| 9 | Diameter | undercutDiameter | length | auto | addUndercut | REMEMBER |
| 10 | Length | undercutLength | length | auto | addUndercut | REMEMBER |
| 11 | Split face | splitFace | boolean | **true** | not Add undercut and End type = Blind | REMEMBER |

- Behavior [FS][DOCS]:
  - The selected edge must bound a **convex cylinder** with arc sweep >= 182 deg. Several edges are allowed if they're the same size.
  - **Auto-size**: when edges are first picked or the Standard changes, the table is searched for the size whose diameter matches the cylinder. After switching standard the user must pick a size manually for auto sizing [DOCS].
  - Size change auto-recomputes:
    - Chamfer length = (major - minor)/2 + 0.0075 in.
    - Undercut diameter = minor diameter - 0.015 in.
    - Undercut length = 2 x pitch (1 mm fallback).
    - Unchecking chamfer or undercut also resets those values.
  - Threads are **cosmetic** (dashed lines in drawings, callout via Hole/thread callout). They're not in Hole tables.
  - Split face splits the cylinder at the thread length.

---

## 17. Boolean

- Toolbar: "Boolean". Command `createFeatureBoolean` [BUNDLE]. Available in: Part Studio.
- FS: `booleanBodies` (boolean.fs). Filter selector "allparts".

| # | UI label | param | type | options / range (default) | visible when | hints / filter |
|---|---|---|---|---|---|---|
| 1 | Operation type | operationType | enum | **Union** / Subtract / Intersect (hidden SUBTRACT_COMPLEMENT) | always | HORIZONTAL_ENUM |
| 2 | Tools | tools | query | — | always | bodies: solid, or non-construction non-sketch sheet; mesh ok. ALLOW_QUERY_ORDER |
| 3 | Targets | targets | query | — | Subtract | same, modifiable |
| 4 | Offset | offset | boolean | false | Subtract | |
| 5 | Offset all | offsetAll | boolean | false | Subtract and Offset | |
| 6 | Faces to offset | entitiesToOffset | query | — | Offset and not Offset all | solid faces |
| 7 | Offset distance | offsetDistance | length | 0..500 m, 5 mm / 0.25 in | Offset | |
| 8 | Opposite direction | oppositeDirection | boolean | false | Offset | OPPOSITE_DIRECTION |
| 9 | Reapply fillet | reFillet | boolean | false | Offset | |
| 10 | Keep tools | keepTools | boolean | false, then **remembered** | always | REMEMBER_PREVIOUS_VALUE |

- Behavior [DOCS]:
  - Union and Intersect use only Tools (all participants). Subtract removes Tools from Targets.
  - Solids can't be mixed with surfaces (BOOLEAN_CANNOT_MIX_SOLIDS_AND_SURFACES). Surface union needs shared edges.
  - Parts must intersect or touch, or you get the *_NO_OP errors.
  - Merged results keep the attributes (name, etc.) of the **earlier-selected** entity, which is why the selection order is preserved.
  - Offset (Subtract) leaves a gap between selected faces (or all faces with Offset all) and the remaining parts.
  - Keep tools keeps the tool bodies, e.g. for fitted parts.
  - Errors: BOOLEAN_NEED_ONE_SOLID, BOOLEAN_SAME_INPUT, BOOLEAN_UNION_NO_OP, BOOLEAN_SUBTRACT_NO_OP, BOOLEAN_INTERSECT_NO_OP, BOOLEAN_OFFSET_NO_FACES, BOOLEAN_NON_MANIFOLD_RESULT.

---

## 18. Split

- Toolbar: "Split". Command `createFeatureSplit` [BUNDLE]. Available in: Part Studio.
- FS: `splitPart` (splitpart.fs). Editing logic `splitEditLogic`, manipulators `splitManipulatorChange` (keep-side flip arrow).

| # | UI label | param | type | options / range (default) | visible when | hints / filter |
|---|---|---|---|---|---|---|
| 1 | Split type | splitType | enum | **Part** / Face | always | HORIZONTAL_ENUM |
| 2 | Parts, surfaces, or curves to split | targets | query | — | Part | solid, sheet, or wire bodies; modifiable, non-sketch, mesh ok |
| 3 | Entity to split with | tool | query (1) | — | Part | non-sketch sheet body, OR any face (incl. planes), OR mate connector |
| 4 | Keep tools | keepTools | boolean | false (auto-true when the tool is a single face) | Part | |
| 5 | Trim to face boundaries | useTrimmed | boolean | false | Part | |
| 6 | Keep both sides | keepBothSides | boolean | **true** | Part | |
| 7 | Opposite direction | keepFront | boolean | **true** | Part and not Keep both sides | OPPOSITE_DIRECTION (+ graphics flip arrow) |
| 8 | Faces to split | faceTargets | query | — | Face | faces, non-sketch, non-construction, modifiable |
| 9 | Entities to split with | faceTools | query | — | Face | edges (non-construction), non-sketch sheet/wire bodies, faces, mate connectors |
| — | Group "Edge projection options" (expanded) | | | | Face | |
| 10 | Projection direction type | projectionType | enum | **Direction** / Normal to target | Face | |
| 11 | Use sketch plane direction | useSketchPlaneDirection | boolean | **true** (auto-set) | Face and Direction | |
| 12 | Direction | directionQuery | query (1) | — | Direction and not Use sketch plane direction | ALLOWS_DIRECTION |
| 13 | Keep tool surfaces and curves | keepToolSurfaces | boolean | **true** | Face | |

- Behavior [DOCS][FS]:
  - Part split takes **one** tool: plane, face, surface, or (implicit) mate connector.
  - When the tool is a single face, Keep tools is forced on, since faces of other parts, planes, and MCs are always kept (SPLIT_KEEP_TOOLS_WITH_FACE / SPLIT_KEEP_PLANES_AND_MATE_CONNECTORS).
  - Trim to face boundaries only applies when a face is the tool. Off means the face's underlying surface is extended. A multi-face surface plus Trim requires choosing a single face (SPLIT_TRIM_WITH_SINGLE_FACE).
  - Keep both sides off: only the side the arrow points to is kept.
  - Face split projects curves onto target faces.
    - Use sketch plane direction is auto-enabled when the edge tools lie in a sketch plane, and auto-disabled otherwise.
    - Normal to target projects along the face normal.
  - Face split is how parting lines for Draft are made.
  - Errors: SPLIT_SELECT_TARGETS, SPLIT_SELECT_TOOL, SPLIT_SELECT_FACE_TARGETS, SPLIT_SELECT_FACE_TOOLS, SPLIT_SELECT_FACE_DIRECTION, SPLIT_NO_CHANGE, SPLIT_FACE_NO_CHANGE, SPLIT_OFF_FACE, SPLIT_FAILED.

---

## 19. Delete part

- Toolbar: "Delete part". Command `createFeatureDeletePart` [BUNDLE]. Available in: Part Studio.
- FS: `deleteBodies` (deleteBodies.fs). Editing logic `deletePartEditLogic`.

| # | UI label | param | type | options (default) | visible when | filter |
|---|---|---|---|---|---|---|
| 1 | Composite part options | compositePartOption | enum | **Delete composite parts** / Dissolve composite parts / Ignore composite parts | always | dropdown |
| 2 | Entities to delete | entities | query | — | option != Ignore | any body (parts, surfaces, curves, composites), modifiable, non-sketch, mesh ok |
| 2' | Entities to delete | nonCompositeEntities | query | — | option = Ignore | same minus composite bodies |

- Behavior [DOCS][FS]:
  - Parametric: it creates a "Delete part" feature, which can be undone and rolled back.
  - The two selection fields mirror each other. Switching to Ignore drops composites from the selection.
  - Dissolve removes the composite wrapper and keeps its constituents. For non-composite parts the dropdown is irrelevant.
  - Use case: delete tool bodies after several Booleans.
  - Part-list Delete of a part creates this feature too. **Deleting a face or edge selection in the graphics area deletes the owning feature**, not the part.
  - Auto colors re-sequence after deletion unless colors are custom-assigned.

---

## 20. Cross-feature implementation checklist

- **Remembered values** (REMEMBER_PREVIOUS_VALUE), keyed per feature type and field:
  - Creation type: Extrude, Revolve, Sweep, Loft.
  - Sweep: Profile control, Twist type.
  - Fillet: Measurement, Control, radius, width, second radius, partial End types.
  - Chamfer: Measurement, Chamfer type, Distance.
  - Face blend: Cross section, Measurement, Radius, Width, Second radius, Ratio, Control, Trim type.
  - Draft: Draft angle, Second draft angle.
  - Body draft: Draft scope.
  - Boolean: Keep tools.
  - Hole: Units, Style, Standard, Thread class, Diameter, Tap drill, Start plane, Termination, C'bore/C'sink values, Depth, Tip angle, Tapped depth, Tap through all, Tap clearance.
  - External thread: Standard, End type, Length, Chamfer, Undercut, Split face.
- **Shortcuts** [DOCS]: Shift+E Extrude, Shift+W Revolve, Shift+F Fillet (Edge). The others have none by default. "S" opens the customizable shortcut toolbar, and Alt/Opt+C opens tool search.
- **Edit-logic heuristics to replicate** [FS]:
  - Boolean-step auto op/scope (Extrude, Revolve, Sweep, Loft; Thicken has its own variant).
  - Extrude/Revolve direction auto-flip toward Up-to targets and on Add<->Remove switches.
  - Thicken face-owner auto-scope.
  - Rib auto-parts/direction.
  - Hole raycast scope/flip, table-driven value fill, name template.
  - Modify fillet radius read-back.
  - Body draft pull/parting cross-fill.
  - Split keep-tools and sketch-direction auto-set.
  - External thread auto-size.
  - Loft connection updates.
  - Draft parting-line hint faces.
- **Manipulators**:
  - Extrude: depth arrows, flip arrows.
  - Revolve: angle arc, flip.
  - Fillet: radius arrow, partial/variable point-on-edge sliders.
  - Draft: angle arcs, flip.
  - Loft: connection sliders.
  - Body draft / Split / Face blend: flip arrows.
  - All manipulator edits write back to dialog fields in real time.
