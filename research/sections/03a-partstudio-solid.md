## 3A. Part Studio features — solid modeling, patterns, direct edit

Scope: body-creating and body-modifying Part Studio features (extrude, revolve, sweep, loft, thicken, enclose), edge/face treatments (fillet, chamfer, draft, shell, hole, rib, gusset, wrap), body operations (boolean, split, transform, delete part), patterns/mirror, direct-edit (delete/move/replace face, modify fillet), and assorted internal features (primitives, section part, derived mirror). Plane, sketch, surfacing, sheet metal, frames and mate connectors are covered in other sections.

Conventions used below:
- "FS" = FeatureScript standard library (mirror dated 2026-05). Parameter tables are derived from each feature's `precondition`. UI labels in quotes are the `"Name"` annotation.
- Bounds are written `min .. max, default <metric>/<imperial>`; e.g. `LENGTH_BOUNDS` = -500 m .. 500 m, default 25 mm / 1 in.
- Common bound specs (from valueBounds.fs): `LENGTH_BOUNDS` [-500 m, 25 mm/1 in, 500 m]; `NONNEGATIVE_LENGTH_BOUNDS` [1e-5 m, 25 mm/1 in, 500 m]; `NONNEGATIVE_ZERO_INCLUSIVE_LENGTH_BOUNDS` [0, 25 mm/1 in, 500 m]; `NONNEGATIVE_ZERO_DEFAULT_LENGTH_BOUNDS` [0, 0, 500 m]; `ZERO_DEFAULT_LENGTH_BOUNDS` [-500 m, 0, 500 m]; `BLEND_BOUNDS` [1e-5 m, 5 mm/0.2 in, 500 m]; `SHELL_OFFSET_BOUNDS` [1e-5 m, 2.5 mm/0.1 in, 500 m]; `ZERO_INCLUSIVE_OFFSET_BOUNDS` [0, 5 mm/0.25 in, 500 m]; `ANGLE_STRICT_90_BOUNDS` [0, 3 deg, 89.9 deg]; `ANGLE_STRICT_180_BOUNDS` [0, 30 deg, 179.9 deg]; `ANGLE_360_BOUNDS` [-1e5, 30 deg, 1e5 deg]; `ANGLE_360_FULL_DEFAULT_BOUNDS` [-1e5, 360 deg, 1e5]; `ANGLE_360_ZERO_DEFAULT_BOUNDS` [-1e5, 0, 1e5]; `ANGLE_360_90_DEFAULT_BOUNDS` [-1e5, 90 deg, 1e5]; `ANGLE_180_MINUS_180_BOUNDS` [-180, 0, 180 deg]; `POSITIVE_COUNT_BOUNDS` [1, 2, 1e5]; `SCALE_BOUNDS` [1e-5, 1, 1e5]; `FILLET_RHO_BOUNDS` [0, 0.5, 0.99999]; `CHAMFER_ANGLE_BOUNDS` [0.1, 45 deg, 179.9 deg]; `PRIMARY_PATTERN_BOUNDS` [1, 2, 2500]; `SECONDARY_PATTERN_BOUNDS` [1, 1, 2500]; `CIRCULAR_PATTERN_BOUNDS` [1, 4, 2500]; `CURVE_PATTERN_BOUNDS` [1, 2, 2500].
- "CAN_BE_TOLERANT" UIHint = the numeric field accepts tolerance info (precision + tolerance type) used by MBD/drawings; see the MBD section. Noshape must store an optional tolerance record per such parameter.

### PS-01 Common feature mechanics
- **Purpose:** Shared semantics used by every body-creating / body-modifying feature: creation type, result (boolean) operation type, merge scope, surface joining, extrude-style end bounds, offsets, draft, second direction, manipulators, dialog preview/final, and pattern-instance awareness.
- **Sources:** https://cad.onshape.com/help/Content/PartStudio/feature_basics.htm ; https://cad.onshape.com/help/Content/PartStudio/extrude.htm ("Merge scope", "End type examples"); FS: tool.fs, boolean.fs (`booleanStepTypePredicate`, `booleanStepScopePredicate`, `booleanPatternScopePredicate`, `surfaceOperationTypePredicate`, `surfaceJoinStepScopePredicate`, `processNewBodyIfNeeded`, `joinSurfaceBodiesWithAutoMatching`), booleanHeuristics.fs, extrudeCommon.fs, boundingtype.gen.fs, patternUtils.fs/patternCommon.fs.

#### PS-01.1 Creation type (body type)
- Enums (tool.fs): `ToolBodyType` {SOLID "Solid", SURFACE "Surface"} used by revolve/sweep/loft; `ExtendedToolBodyType` {SOLID "Solid", SURFACE "Surface", THIN "Thin"} used by extrude/revolve(thin)/sweep(thin) where supported. Shown as a horizontal enum "Creation type"; extrude adds UIHint REMEMBER_PREVIOUS_VALUE (the dialog re-opens with the last-used creation type).
- Solid: closed regions / planar faces -> solid bodies, followed by a solid boolean step (PS-01.2).
- Surface: open or closed curves/edges -> sheet bodies, followed by an optional surface join step (PS-01.4). Remove/Intersect are not available for surfaces.
- Thin: open or closed curves are offset into a thin planar region (thickness 1/2 or mid-plane) which is then swept like a solid; result is a solid, followed by the solid boolean step.

#### PS-01.2 Result body operation type (solid boolean step)
- `booleanStepTypePredicate`: field `operationType` is `NewBodyOperationType`, UI "Result body operation type" (horizontal enum, label usually hidden): NEW "New", ADD "Add", REMOVE "Remove", INTERSECT "Intersect". Default NEW.
- `booleanStepScopePredicate` (visible only when `operationType != NEW`):

| FS id | UI label | Type | Default | Visible when / notes |
|---|---|---|---|---|
| `defaultScope` | Merge with all | boolean | false (UI annotation) ; FS function default `true` when missing | operationType != NEW |
| `booleanScope` | Merge scope | Query: BODY && SOLID && modifiable, mesh allowed | empty | operationType != NEW and !defaultScope |

- `booleanPatternScopePredicate` (patterns and mirror): same fields but Merge scope filter also allows non-construction, non-sketch SHEET bodies (surfaces are only valid targets for surface+surface Add and surface-minus-solid; enforced at execution).
- Execution (`processNewBodyIfNeeded`, boolean.fs), run after the feature's own geometry op created bodies under the feature id:
  1. If NEW: nothing more; every created body is a new part.
  2. tools = solid bodies created by this feature (plus `seed` bodies for patterns: the seed solids are added to the tools); targets = (`defaultScope` ? all modifiable solid bodies in the Part Studio (mesh included after the mixed-modeling version) : `booleanScope`) minus tools minus optional `mergeScopeExclusion`.
  3. Map `NewBodyOperationType` -> `BooleanOperationType`: ADD -> UNION; REMOVE -> SUBTRACTION; INTERSECT -> SUBTRACT_COMPLEMENT (hidden enum value: removes from each target the complement of the union of tools, i.e. each target becomes target ∩ (union of tools)).
  4. If targets contain active sheet metal and op is not REMOVE -> error SHEET_METAL_CAN_ONLY_REMOVE ("Only Remove can be used with sheet metal. Add Finish sheet metal model to allow other operations.").
  5. If tools empty -> FEATURE_NO_SOLIDS ("Feature did not create any solids."). If grouping is required and targets empty -> BOOLEAN_NEED_ONE_SOLID ("No merge scope selected.") highlighting the `booleanScope` field.
  6. Special case ADD with empty targets: when the feature has a seed (patterns) grouping is turned off and seed+new bodies are unioned among themselves; otherwise error as in 5.
  7. Calls `booleanBodies` (opBoolean) with `targetsAndToolsNeedGrouping=true`, `keepTools=false` (tools consumed). On error/warning the feature re-runs its geometry op under a temporary id ("reconstructOp") purely to display the would-be tool bodies as red error entities, then deletes them.
- opBoolean semantics (geomOperations.fs doc): UNION merges tools that intersect or abut; when several bodies merge, the identity (name, color, appearance, material) of the body that appears earliest is preserved -> for a feature ADD the existing target part keeps its name and properties. SUBTRACTION removes the union of all tools from every target (targets may be solids or surfaces). INTERSECTION intersects all tools. With grouping (body-creating features), each target is processed against the tools that touch it (inferred: a tool region that touches no target remains a separate new part for ADD).
- No-op outcomes are reported as info/warnings, not errors: BOOLEAN_UNION_NO_OP ("Boolean resulted in no geometry change. The parts either do not intersect or are totally contained."), BOOLEAN_SUBTRACT_NO_OP ("Selected tools and targets do not intersect."), BOOLEAN_INTERSECT_NO_OP. Other errors: BOOLEAN_INVALID, BOOLEAN_NON_MANIFOLD_RESULT ("Boolean operation would result in non-manifold body."), BOOLEAN_SAME_INPUT.
- Part naming (inferred from UI behavior; not specified in FS): every new solid body receives the next free default name "Part N" (N = max existing default index + 1 within the Part Studio), new sheet bodies "Surface N", wire bodies "Curve N"; bodies that survive a union keep the name of the earliest/target body; a split produces new bodies which inherit the parent name for one piece and get new "Part N" names for the others (inferred). Users can rename; names are stable attributes of the body's persistent identity.

#### PS-01.3 Default merge-scope / operation heuristics (booleanHeuristics.fs)
- Runs in the "Editing Logic Function" (dialog-time, not regeneration) of extrude, revolve, sweep, loft, thicken, mirror, etc., via `booleanStepEditLogic`.
- Skipped entirely if the user has touched "Merge with all" or "Merge scope". If the user explicitly chose the operation type, only scope can be auto-filled (and only if NEW is not chosen and scope is still empty). If the user is currently flipping the direction arrow, operation type is not changed.
- Algorithm (`autoSelectionForBooleanStep`): run the feature's geometry op as NEW in a temporary sub-feature; tools = created solids; candidate targets = all modifiable solids minus tools minus hidden bodies. Run `evCollision` tools vs targets and classify each (tool, target) pair as intersection (INTERFERE, TARGET_IN_TOOL, TOOL_IN_TARGET) or abutting (ABUT_NO_CLASS / ABUT_TOOL_OUT_TARGET with face-to-face interference; edge-only touching ignored).
  - If any tool collides with >1 target, or tools collide with different targets -> NEW, empty scope.
  - If exactly one target is touched: if it is not active sheet metal -> ADD with scope = [that target]; if it is sheet metal and the tool is sheet metal -> NEW; if sheet metal target and non-SM tool -> REMOVE with scope = [target].
  - No collisions -> NEW.
  - Sets `defaultScope=false` and `booleanScope` explicitly in all cases.
- Direction-flip coupling (`canSetBooleanFlip`): when the user changes the operation between a "positive" type (NEW/ADD) and a "negative" type (REMOVE/INTERSECT) and has not set the scope, extrude flips `oppositeDirection` automatically (so switching Add -> Remove flips the extrusion into the part).
- Help-page wording differs: help says "By default, Merge all is selected" and "If the geometry touches or intersects with only one part then that part is automatically added to the merge scope; if multiple parts touch, you must select". The FS precondition default is `false` and the heuristic fills an explicit scope; treat FS as authoritative.

#### PS-01.4 Surface operation type and surface join
- `surfaceOperationTypePredicate`: `surfaceOperationType` is `NewSurfaceOperationType` {NEW "New", ADD "Add"}, UI "Result body operation type".
- `surfaceJoinStepScopePredicate` (visible when surfaceOperationType != NEW):

| FS id | UI label | Type | Default | Visible when |
|---|---|---|---|---|
| `defaultSurfaceScope` | Merge with all | boolean | true | surfaceOperationType == ADD |
| `booleanSurfaceScope` | Merge scope | Query: BODY && SHEET && modifiable && !sketch, mesh allowed | empty | ADD and !defaultSurfaceScope |

- Execution (`joinSurfaceBodiesWithAutoMatching`): tools = modifiable sheet bodies created by the feature (+ seed sheets for patterns); targets = all other modifiable sheet bodies (Merge with all) or the selected scope. Errors: BOOLEAN_NO_SURFACE_TO_MERGE_WITH ("No surfaces available to merge.") when merge-with-all finds nothing; BOOLEAN_NO_SURFACE_IN_MERGE_SCOPE when scope empty; warnings BOOLEAN_NO_TARGET_SURFACE / BOOLEAN_NO_SHARED_EDGE_WITH_SURFACE_IN_MERGE_SCOPE when no shared edge. opBoolean UNION with sheets: adjacency is detected automatically (coincident or overlapping laminar edges are sewn), `eraseImprintedEdges=true`, `makeSolid=false` for features (Boolean feature union of surfaces uses `makeSolid=true` so a closed set becomes a solid).
- Edit-logic default (`surfaceOperationTypeEditLogic`): if the user has not chosen, ADD is pre-selected when any input edge is a laminar (one-sided, joinable) edge of an existing modifiable surface, or when a sketch edge coincides with such an edge; otherwise NEW. Extrude with a second direction forces NEW.

#### PS-01.5 End bound types (BoundingType, extrude-style)
- Enum `BoundingType` (boundingtype.gen.fs): BLIND "Blind"; UP_TO_NEXT "Up to next"; UP_TO_SURFACE "Up to face"; UP_TO_BODY "Up to part"; UP_TO_VERTEX "Up to vertex"; THROUGH_ALL "Through all"; UP_TO_FACE (hidden, legacy). "Symmetric" is not an enum value: it is a separate boolean available only for BLIND and THROUGH_ALL.
- Parameters by bound (`extrudeBoundParametersPredicate`; second direction uses the same with `secondDirection*` ids and "Second ..." column names):

| FS id | UI label | Type / filter | Bounds / default | Visible when |
|---|---|---|---|---|
| `depth` | Depth | length, CAN_BE_TOLERANT | LENGTH_BOUNDS (-500..500 m, 25 mm / 1 in) | BLIND |
| `endBoundEntityFace` | Up to face | FACE (non-sketch, mesh ok) or mate connector; max 1 | — | UP_TO_SURFACE |
| `endBoundEntityBody` | Up to surface or part | BODY SOLID or SHEET, non-sketch, mesh ok; max 1 | — | UP_TO_BODY |
| `endBoundEntityVertex` | Up to vertex or mate connector | ALLOWS_VERTEX (vertex, sketch point, implicit/explicit mate connector); max 1 | — | UP_TO_VERTEX |
| `hasOffset` | Offset distance (checkbox) | boolean | false | UP_TO_NEXT/SURFACE/BODY/VERTEX |
| `offsetDistance` | Offset distance | length, CAN_BE_TOLERANT | LENGTH_BOUNDS | hasOffset |
| `offsetOppositeDirection` | Opposite direction | boolean | false | hasOffset |

- Semantics (extrudeCommon.fs `transformExtrudeDefinitionForOpExtrude` + opExtrude):
  - Negative blind depth is normalized: depth = |depth| and `oppositeDirection` toggled (`adjustExtrudeDirectionForBlind`), same for second direction.
  - BLIND: end cap at distance `depth` along direction. Symmetric BLIND: startDepth = -depth/2, endDepth = depth/2 (depth is the total).
  - THROUGH_ALL: "an unspecified distance, guaranteed to be further in the extrude direction than any other solid or surface in the context" (compute from Part Studio bounding box). Symmetric THROUGH_ALL: start bound THROUGH_ALL in the opposite direction.
  - UP_TO_SURFACE: end cap lies on the infinite underlying surface of the chosen face/plane (help: "Up to the infinite face underlying the selected face or plane"; the profile must project fully onto it). If a mate connector is picked, a temporary plane through the MC origin with MC Z normal is created and used, then deleted.
  - UP_TO_BODY: terminate on the selected solid or surface body (the extrusion must be completely bounded by it); help: "the sketch region ... must fall entirely within the target entity".
  - UP_TO_VERTEX: a temporary plane is created through the vertex point with normal = profile plane normal (not the extrude direction if a custom direction is set), and the bound becomes UP_TO_SURFACE on that plane.
  - UP_TO_NEXT: kernel-side (opExtrude). Documented behavior: "Extrude up to the next solid body or surface body in the context"; "Up to the next face or faces encountered in the specified direction. If it doesn't completely terminate, then the Extrude fails" (EXTRUDE_UPTO_NEXT_NO_DIVISION "Up to next termination problem, consider switching it to up to part"). Recommended algorithm (inferred): (1) build the through-all prism of the profile; (2) collect faces of all other solid and sheet bodies (excluding bodies created by this feature; for Remove/Add the target bodies are included); (3) split the prism by those faces (general fuse/splitter); (4) keep the split cell(s) adjacent to the profile (start cap) - i.e. the region between the profile and the first faces hit along the direction; (5) fail if that region is not closed off (some profile ray escapes to infinity without hitting a face). When the profile starts on a face of a body and extrudes into material, the "next" face is the first face reached after leaving the start (exit face or cavity face).
  - Offset (hasOffset, for UP_TO_NEXT/SURFACE/BODY; for UP_TO_VERTEX the offset is applied to the vertex plane): `endTranslationalOffset = offsetDistance`, sign chosen so a positive non-flipped offset stops short of the bound (pulls the cap back toward the profile); `offsetOppositeDirection` pushes the cap past/into the bound. EXTRUDE_OFFSET_TOO_DEEP if the offset collapses the extrusion.
  - Second direction start bound: `startBound` = second bound type; `isStartBoundOpposite = (secondOpposite != firstOpposite)`. If both directions point the same way the second end acts as an offset start (help: "Create an offset from the sketch plane by flipping the extrude second direction").
- Edit-logic auto flip: when the user picks an Up-to entity and has not touched the flip arrow (and no second direction/symmetric), the direction is flipped toward the bound entity (plane intersection with axis behind the profile, vertex behind the profile, or body bounding box entirely behind). Second direction default flip = opposite of first.

#### PS-01.6 Starting offset, direction, draft, second direction (extrude family)
- Starting offset (`extrudeOffsetPredicate`): `startOffset` boolean "Starting offset"; `startOffsetBound` StartOffsetType {BLIND "Blind", ENTITY "Entity"}; BLIND -> `startOffsetDistance` "Depth" (LENGTH_BOUNDS) + `startOffsetOppositeDirection`; ENTITY -> `startOffsetEntity` "Entity" (planar face, edge, vertex or mate connector; max 1). Entity must be a line perpendicular to the profile normal or a plane parallel to the profile (errors EXTRUDE_START_OFFSET_BOUND_NOT_PLANAR, EXTRUDE_START_OFFSET_BOUND_NOT_PARALLEL_TO_EXTRUDED_ENTITIES, EXTRUDE_SELECT_START_OFFSET_ENTITY). Distance for ENTITY = (closest point on entity - profile origin)·n / (dir·n). Implemented as a translation of the profile along the extrude direction before extrusion.
- Direction (`extrudeDirectionPredicate`, not for Thin): `hasExtrudeDirection` "Direction"; `extrudeDirection` "Extrude direction" (ALLOWS_DIRECTION: linear edge, planar face normal, axis, mate connector Z; max 1). Direction is flipped if it points against the profile normal; error EXTRUDE_DIRECTION_COPLANAR if perpendicular to the profile normal; EXTRUDE_DIRECTION_INVALID_ENTITY; EXTRUDE_SELECT_DIRECTION.
- Draft (solid/thin only): `hasDraft` "Draft"; `draftAngle` "Draft angle" ANGLE_STRICT_90_BOUNDS (0..89.9 deg, default 3 deg); `draftPullDirection` "Opposite direction" (false = draft outward, true = draft inward). Implementation: after extrusion, a temporary neutral plane through the profile (normal = plane normal, sign follows flip) is created; side (SWEPT) faces are drafted with opDraft NEUTRAL_PLANE (no tangent propagation, no refillet). With symmetric extrude or a two-sided second direction, the body is first split at the neutral plane, both halves drafted (second half with inverted pull), then re-unioned. Angle 0 = no draft.
- Second end position (`hasSecondDirection`, not shown when symmetric): `secondDirectionBound` "End type" (BoundingType, default BLIND), `secondDirectionOppositeDirection` "Opposite direction" (default true), its bound parameters, and - only for solid/thin when the second direction actually points opposite the first - `hasSecondDirectionDraft`, `secondDirectionDraftAngle` (ANGLE_STRICT_90_BOUNDS), `secondDirectionDraftPullDirection`.

#### PS-01.7 Manipulators (drag handles)
- Features declare a "Manipulator Change Function". Extrude: linear arrow manipulator along the extrude axis at the profile origin (offset = depth, halved for symmetric, negative when flipped; base shifted by start offset) bound to `depth`; dragging past zero flips `oppositeDirection`. Non-blind bounds show only a flip arrow manipulator. Second direction gets SECONDARY-style manipulators. Through-all symmetric shows none. Patterns use toggle-point manipulators for skip instances; revolve uses an angular manipulator; fillet/chamfer/shell/thicken use linear/offset manipulators on the selected geometry.
- Noshape: manipulators are typed (linear, angular, flip, points-toggle) with a parameter binding and a change callback that maps drag value -> parameter(s).

#### PS-01.8 Feature dialog behavior: preview, final, rollback
- Dialog fields: blue = selection query field (first focused), white = text input; Tab cycles fields; Enter commits; Esc cancels. Title turns red when the feature is incomplete or errors (cannot commit).
- Live preview regenerates on every parameter change; result shown with the feature's created/modified geometry highlighted; error entities (tool bodies from reconstructOp) rendered red.
- Preview slider: opacity blend between before-feature (0%) and after-feature (100%) state.
- When editing a feature, the model is rolled back to that feature (later features hidden); a "Final" button shows the end-of-list result while editing (not shown for the last feature).
- Array parameters (e.g. loft profiles) support drag reorder; missing references are shown in red and can be cleared individually or via "Clear missing references".
- Boolean checkboxes can be converted to expressions ("Convert to expression") evaluating to true/false; numeric fields accept expressions, variables and units.

#### PS-01.9 Pattern instance awareness; Feature vs Part vs Face pattern
- Every feature evaluates `getRemainderPatternTransform` on its references: when it is re-executed inside a Feature pattern/mirror, its references are already transformed and the remaining transform is applied to directions/axes so the instance is geometrically consistent (e.g. extrude axes, mirror planes are untransformed).
- Part pattern: copies bodies (opPattern), then optional solid boolean (seed included as tool for Add) or surface join. Face pattern: copies faces and re-integrates them into the owning bodies (fast; ignores feature intent such as end conditions). Feature pattern: either (a) default (Reapply features off) - collect bodies/faces created by the selected features and pattern them as a combination of face/part patterns ("face pattern based feature pattern") or (b) "Reapply features" on - re-run every selected feature per instance with the instance transform pushed on the context (end conditions re-evaluated per instance; slower). See PS-22..PS-25.
- **Edge cases & errors:** see per-feature items; general errors above.
- **OCCT mapping:** booleans: BRepAlgoAPI_Fuse / BRepAlgoAPI_Cut / BRepAlgoAPI_Common (use BRepAlgoAPI_BuilderAlgo / BOPAlgo with SetRunParallel, SetFuzzyValue, SetNonDestructive) with tool/argument lists; INTERSECT-with-grouping = for each target, BRepAlgoAPI_Common(target, fuse(tools)). Surface join: BRepBuilderAPI_Sewing (tolerance ~1e-6 m), then BRepBuilderAPI_MakeSolid / ShapeFix_Solid when closed. Post-boolean cleanup: ShapeUpgrade_UnifySameDomain (eraseImprintedEdges). Collision classification for heuristics: BRepExtrema_DistShapeShape (distance 0) + BRepAlgoAPI_Common volume check (BRepGProp::VolumeProperties) or BRepClass3d_SolidClassifier; abutting = BRepAlgoAPI_Section producing faces. Up to next: custom (BOPAlgo_Splitter of through-all prism by candidate faces + cell selection). Naming/identity: custom persistent naming layer using BRepAlgoAPI history (Modified/Generated/IsDeleted) — mandatory for all features. Manipulators/preview: app layer.
- **Priority:** P0

### PS-02 Extrude
- **Purpose:** Add depth to sketch regions / planar faces (solid), sketch curves (surface) or offset curve walls (thin) along a straight direction; create new parts or add/remove/intersect.
- **Sources:** https://cad.onshape.com/help/Content/PartStudio/extrude.htm ; FS: `extrude` (extrude.fs, extrudeCommon.fs). Shortcut: Shift+E (available in Sketch and Part Studio toolbars).
- **Selection inputs:**
  - "Faces and sketch regions to extrude" `entities` (Solid; also Flat domain): sketch FACE regions (incl. flattened sheet-metal sketch regions) or planar non-sketch FACEs; no construction; multi.
  - "Sketch curves to extrude" `surfaceEntities` (Surface): sketch EDGEs, modifiable, non-construction; multi.
  - "Faces and sketch regions to extrude" `wallShape` (Thin): planar FACEs (their boundary edges are used) or sketch EDGEs (non-construction); multi. All shapes must be on planes parallel to the first.
  - Up-to entities, start-offset entity, direction entity: see PS-01.5/PS-01.6 (max 1 each).
  - When Extrude is launched with a sketch open in Solid mode, all closed regions of the sketch (including nested) are pre-selected.
- **Parameters:**

| FS id | UI label | Type | Enum values / default / bounds | Visible when / notes |
|---|---|---|---|---|
| `domain` | (hidden) | OperationDomain {MODEL, FLAT} | MODEL | Always hidden; edit logic sets FLAT when only flattened sheet-metal regions are selected |
| `bodyType` | Creation type | ExtendedToolBodyType | SOLID "Solid" / SURFACE "Surface" / THIN "Thin"; default SOLID; REMEMBER_PREVIOUS_VALUE | domain != FLAT |
| `operationType` | Result body operation type | NewBodyOperationType | NEW/ADD/REMOVE/INTERSECT; default NEW | domain != FLAT and bodyType != SURFACE |
| `surfaceOperationType` | Result body operation type | NewSurfaceOperationType | NEW/ADD; default NEW | bodyType == SURFACE |
| `flatOperationType` | Creation type | FlatOperationType | ADD "Add" / REMOVE "Remove"; default REMOVE | domain == FLAT |
| `entities` | Faces and sketch regions to extrude | Query | see above | bodyType SOLID (or FLAT) |
| `surfaceEntities` | Sketch curves to extrude | Query | see above | bodyType SURFACE |
| `wallShape` | Faces and sketch regions to extrude | Query | see above | bodyType THIN |
| `midplane` | Mid plane | boolean | false | THIN |
| `thickness1` | Thickness 1 | length, CAN_BE_TOLERANT | ZERO_INCLUSIVE_OFFSET_BOUNDS 0..500 m, default 5 mm / 0.25 in | THIN and !midplane |
| `flipWall` | Flip wall | boolean (opposite-direction arrow) | false | THIN and !midplane |
| `thickness2` | Thickness 2 | length, CAN_BE_TOLERANT | NONNEGATIVE_ZERO_DEFAULT 0..500 m, default 0 | THIN and !midplane |
| `thickness` | Thickness | length, CAN_BE_TOLERANT | ZERO_INCLUSIVE_OFFSET_BOUNDS; FS default 0.25 in | THIN and midplane (split half/half) |
| `endBound` | End type | BoundingType | BLIND (default) / UP_TO_NEXT / UP_TO_SURFACE / UP_TO_BODY / UP_TO_VERTEX / THROUGH_ALL | domain != FLAT |
| `oppositeDirection` | Opposite direction | boolean (arrow) | false | always (model) |
| `depth`, `endBoundEntityFace`, `endBoundEntityBody`, `endBoundEntityVertex`, `hasOffset`, `offsetDistance`, `offsetOppositeDirection` | (see PS-01.5) | | | per endBound |
| `hasExtrudeDirection` | Direction | boolean | false | bodyType != THIN |
| `extrudeDirection` | Extrude direction | Query ALLOWS_DIRECTION or mate connector, max 1 | — | hasExtrudeDirection |
| `startOffset` | Starting offset | boolean | false | always |
| `startOffsetBound` | Starting offset bound | StartOffsetType BLIND "Blind" / ENTITY "Entity" | BLIND | startOffset |
| `startOffsetDistance` | Depth | length | LENGTH_BOUNDS | startOffset and BLIND |
| `startOffsetOppositeDirection` | Opposite direction | boolean | false | startOffset and BLIND |
| `startOffsetEntity` | Entity | planar FACE, EDGE, VERTEX, mate connector; max 1 | — | startOffset and ENTITY |
| `symmetric` | Symmetric | boolean | false | endBound BLIND or THROUGH_ALL |
| `hasDraft` | Draft | boolean | false | bodyType SOLID or THIN |
| `draftAngle` | Draft angle | angle | ANGLE_STRICT_90_BOUNDS 0..89.9 deg, default 3 deg | hasDraft |
| `draftPullDirection` | Opposite direction | boolean (circular arrow) | false = outward | hasDraft |
| `hasSecondDirection` | Second end position | boolean | false | not symmetric |
| `secondDirectionBound` | End type | BoundingType | BLIND | hasSecondDirection |
| `secondDirectionOppositeDirection` | Opposite direction | boolean | true | hasSecondDirection |
| `secondDirectionDepth` / `...BoundEntityFace` / `...BoundEntityBody` / `...BoundEntityVertex` / `hasSecondDirectionOffset` / `secondDirectionOffsetDistance` / `secondDirectionOffsetOppositeDirection` | (Second ...) | as first direction | LENGTH_BOUNDS | per secondDirectionBound |
| `hasSecondDirectionDraft` | Draft | boolean | false | SOLID/THIN and second direction opposite to first |
| `secondDirectionDraftAngle` | Draft angle | angle | ANGLE_STRICT_90_BOUNDS | hasSecondDirectionDraft |
| `secondDirectionDraftPullDirection` | Opposite direction | boolean | false | hasSecondDirectionDraft |
| `defaultScope` / `booleanScope` | Merge with all / Merge scope | | PS-01.2 | SOLID/THIN and op != NEW |
| `defaultSurfaceScope` / `booleanSurfaceScope` | Merge with all / Merge scope | | PS-01.4 (default true) | SURFACE and ADD |

- **Behavior / semantics:**
  - Direction: profile plane normal of the first resolved entity (planar face normal; for sketch edges the owner sketch plane normal at the edge midpoint), flipped by `oppositeDirection`; optionally overridden by `extrudeDirection` (oriented to the same half-space as the normal). Extrude axis origin = first planar entity origin or edge midpoint (used for manipulators).
  - Solid: opExtrude of all selected regions/faces into solid bodies; each disconnected region yields a separate body before the boolean step. Then draft (PS-01.6) per body, then the solid boolean step (PS-01.2).
  - Surface: sketch curves extruded into sheet bodies (one body per connected chain, inferred), then optional surface join (Add).
  - Thin: for each group of coplanar wall edges, opOffsetWire offsets the chain by `offset1`=thickness1 and `offset2`=thickness2 (or thickness/2 each for mid plane) on either side, `flip` swaps sides, producing planar regions (`makeRegions`), which are extruded like a solid and then deleted. Offset side rule: single open chain - side defined by the curve direction of the first selected edge; closed loop - default offset1 points outward of the loop. Thin supports draft and second direction; not supported with custom Direction.
  - Flat domain (sheet-metal flat view): an abbreviated dialog (Add/Remove) delegates to SMFlatOp which adds/removes material from the sheet metal flat and back-propagates to the folded model (see sheet-metal section). Mixing flat and 3D selections errors EXTRUDE_3D_AND_FLAT.
  - Inside a Feature pattern, a Thin extrude whose wall face is modified by the seed feature itself fails (EXTRUDE_INVALID_REF_FACE).
  - Tolerances on depth/offset/thickness produce dimensioning data (start/end planes) registered for MBD; tolerant surface extrudes may warn.
- **Edge cases & errors:** EXTRUDE_NO_SELECTED_REGION "Select face or sketch region to extrude."; EXTRUDE_SURF_NO_CURVE "Select sketch curves to extrude."; EXTRUDE_NO_DIRECTION "Cannot compute extrude direction."; EXTRUDE_FAILED; EXTRUDE_SELECT_TERMINATING_SURFACE / _BODY / _VERTEX; EXTRUDE_UPTO_NEXT_NO_DIVISION; EXTRUDE_OFFSET_TOO_DEEP; EXTRUDE_DIRECTION_* ; EXTRUDE_START_OFFSET_*; THIN_EXTRUDE_NOT_PARALLEL_PLANES (wall shapes on non-parallel planes); OFFSET_WIRE_SELECT_WALL_PATH "Select edges or faces to define the path of the thin wall."; OFFSET_WIRE_DIR1_FAILED / OFFSET_WIRE_DIR2_FAILED (offset too large); OFFSET_WIRE_SHAPES_NON_CONSISTENT; DEFINED_IN_SM_FLAT_CANT_REFERENCE_3D / DEFINED_IN_3D_CANT_REFERENCE_SM_FLAT; mesh inputs rejected for profiles. Up-to bounds require the profile to be fully covered by the bounding face/body.
- **OCCT mapping:** Solid/surface blind: BRepPrimAPI_MakePrism(face/wire, vec) (surface: on wires/edges). Two-sided/symmetric: translate profile to start then MakePrism with total length. Through all: MakePrism with length from Part Studio bbox diagonal (or BRepPrimAPI_MakePrism with infinite flag + BRepAlgoAPI_Common with a big box). Up to face/part: BRepFeat_MakePrism::Perform(Until) when the base is a face of an existing solid (feature-based form), or generic approach: through-all prism split by the bounding face's extended surface (BRepBuilderAPI_MakeFace on the underlying Geom_Surface, unbounded/large) via BRepAlgoAPI_Splitter, keep the piece adjacent to the profile. Up to vertex: plane through point + same. Draft: BRepOffsetAPI_DraftAngle on side faces with neutral plane = profile plane (or BRepFeat_MakeDPrism for drafted prism from a face of a base shape; for standalone, build tapered prism via BRepOffsetAPI_DraftAngle after MakePrism). Thin: BRepOffsetAPI_MakeOffset on the wire (GeomAbs_Arc/Intersection join) for each side, build region face between offsets (BRepBuilderAPI_MakeFace with outer+inner wires), then prism. Boolean step: PS-01. Gap: "Up to next" needs custom cell selection; tolerant dimensions need app-level metadata.
- **Priority:** P0

### PS-03 Revolve
- **Purpose:** Sweep planar faces / sketch regions (solid), edges/curves (surface) or offset wall curves (thin) around an axis; new/add/remove/intersect.
- **Sources:** https://cad.onshape.com/help/Content/PartStudio/revolve.htm ; FS: `revolve` (revolve.fs). Shortcut: Shift+W (Sketch and Part Studio toolbars).
- **Selection inputs:**
  - "Faces and sketch regions to revolve" `entities` (Solid): planar FACEs incl. sketch regions, non-construction; multi. With a sketch open, all closed regions are pre-selected.
  - "Edges and sketch curves to revolve" `surfaceEntities` (Surface): non-construction EDGEs or non-sketch WIRE bodies; multi. Wire edges are followed back to their laminar source edges (for joining).
  - "Faces and sketch regions to revolve" `wallShape` (Thin): planar FACEs (boundary edges used) or sketch EDGEs; must be coplanar with the axis.
  - "Revolve axis" `axis`: ALLOWS_AXIS (linear edge/sketch line, linear curve, cylindrical/circular edge or arc -> its axis, implicit/explicit mate connector Z axis); max 1.
- **Parameters:**

| FS id | UI label | Type | Enum values / default / bounds | Visible when / notes |
|---|---|---|---|---|
| `bodyType` | Creation type | ExtendedToolBodyType | SOLID/SURFACE/THIN; default SOLID; remembers previous | always |
| `operationType` | Result body operation type | NewBodyOperationType | default NEW | SOLID/THIN |
| `surfaceOperationType` | Result body operation type | NewSurfaceOperationType | default NEW | SURFACE |
| `entities` / `surfaceEntities` / `wallShape` | see above | Query | | per bodyType |
| `midplane` | Mid plane | boolean | false | THIN |
| `thickness1` | Thickness 1 | length, tolerant | ZERO_INCLUSIVE_OFFSET_BOUNDS (0..500 m, 5 mm / 0.25 in) | THIN, !midplane |
| `flipWall` | Flip wall | boolean | false | THIN, !midplane |
| `thickness2` | Thickness 2 | length, tolerant | 0..500 m, default 0 | THIN, !midplane |
| `thickness` | Thickness | length, tolerant | ZERO_INCLUSIVE_OFFSET_BOUNDS | THIN, midplane |
| `axis` | Revolve axis | Query | max 1 | always |
| `fullRevolve` | Full revolve | boolean | true | always |
| `endBound` | End type | RevolveBoundingType | BLIND "Blind" (default), UP_TO_NEXT "Up to next", UP_TO_SURFACE "Up to face", UP_TO_BODY "Up to part", UP_TO_VERTEX "Up to vertex" | !fullRevolve |
| `oppositeDirection` | Opposite direction | boolean (circular arrow) | false | !fullRevolve |
| `angle` | Revolve angle | angle, tolerant | ANGLE_360_BOUNDS (-1e5..1e5 deg, default 30 deg) | BLIND |
| `symmetric` | Symmetric | boolean | false | BLIND |
| `endBoundEntityFace` | Up to face | FACE (non-sketch, mesh ok) or mate connector; max 1 | | UP_TO_SURFACE |
| `endBoundEntityBody` | Up to surface or part | BODY SOLID/SHEET non-sketch; max 1 | | UP_TO_BODY |
| `endBoundEntityVertex` | Up to vertex or mate connector | ALLOWS_VERTEX; max 1 | | UP_TO_VERTEX |
| `endBoundHasOffset` | Offset | boolean | false | endBound != BLIND |
| `endBoundOffset` | Offset | angle | ANGLE_360_ZERO_DEFAULT_BOUNDS (default 0) | endBoundHasOffset |
| `endBoundOffsetFlip` | Flip offset | boolean | false | endBoundHasOffset |
| `hasStartBound` | Second end position | boolean | false | !fullRevolve and (endBound != BLIND or !symmetric) |
| `startBound` | Start type | RevolveBoundingType | BLIND | hasStartBound |
| `angleBack` | Start angle | angle | ANGLE_360_REVERSE_DEFAULT_BOUNDS (default 330 deg) | startBound BLIND |
| `startOppositeDirection` | Opposite direction | boolean | false | startBound != BLIND |
| `startBoundEntityFace` / `startBoundEntityBody` / `startBoundEntityVertex` | Up to face / Up to surface or part / Up to vertex or mate connector | as end | | per startBound |
| `startBoundHasOffset` | Start offset | boolean | false | startBound != BLIND |
| `startBoundOffset` | Start offset | angle | ANGLE_360_ZERO_DEFAULT_BOUNDS | startBoundHasOffset |
| `startBoundOffsetFlip` | Flip offset | boolean | false | startBoundHasOffset |
| `defaultScope`/`booleanScope` | Merge with all / Merge scope | | PS-01.2 | SOLID/THIN, op != NEW |
| `defaultSurfaceScope`/`booleanSurfaceScope` | Merge with all / Merge scope | | PS-01.4 | SURFACE, ADD |

- **Behavior / semantics:**
  - Axis from evAxis; `oppositeDirection` reverses the axis direction (right-hand rule sweep direction flips).
  - Full revolve: end = blind 360 deg, start = 0 (closed revolution; seam faces merged).
  - Blind: end angle = `angle` (angles beyond 360 are normalized by `adjustAngle`; negative allowed). Symmetric: end = angle/2 and start = 360 - angle/2 (i.e. -angle/2), so the profile plane is mid.
  - Second end position: independent start bound (blind start angle measured in the reverse sense, default 330 deg, or up-to); start up-to direction is independent of the end direction (FS combines `startOppositeDirection` xor `oppositeDirection`).
  - Up-to bounds: same meaning as extrude but angular; offsets are angles; "Up to face" uses the infinite underlying surface; mate connector -> its XY plane. Up to next fails with REVOLVE_UP_TO_NEXT_FAILED / REVOLVE_UP_TO_NEXT_NO_TARGET.
  - Thin: wall thicknesses (thickness1/2 swapped if flipWall xor oppositeDirection; mid plane = thickness/2 each). For double-blind (or full) revolves the wall curves are first revolved into sheets and then thickened (opThicken) with thickness1/2; for up-to bounds the thin region is built in opRevolve itself (offset wire region on the plane containing the axis, then revolved).
  - Pattern-aware: remaining pattern transform applied to the result (`transformResultIfNecessary`).
  - Then solid boolean step or surface join (PS-01).
- **Edge cases & errors:** REVOLVE_SELECT_FACES, REVOLVE_SURF_NO_CURVE, REVOLVE_SELECT_AXIS "Select the axis to revolve around."; REVOLVE_FAILED "Revolve would create self-intersecting part." (profile crosses the axis); REVOLVE_NOT_PLANAR; REVOLVE_PERPENDICULAR "Revolved face is perpendicular to axis."; REVOLVE_NOT_COPLANAR_WITH_AXIS (thin); REVOLVE_2ND_DIR_FAILED; REVOLVE_UP_TO_FAILED; REVOLVE_UP_TO_THIN_FAILED; REVOLVE_SELECT_TERMINATING_BODY/SURFACE/VERTEX. Profiles touching the axis along an edge are allowed (degenerate edge at axis).
- **OCCT mapping:** BRepPrimAPI_MakeRevol(shape, gp_Ax1, angle) for faces (solid) and wires/edges (surface); two-sided: rotate profile by start angle (gp_Trsf rotation) then revolve by total; full: MakeRevol with 2*PI. Up-to: revolve full (or up to 360) then BRepAlgoAPI_Splitter with the bound face's extended surface / body and keep the angular cell adjacent to the profile in the sweep sense (BRepFeat_MakeRevol::Perform(Until) when based on an existing solid). Thin: BRepOffsetAPI_MakeOffset for the wall region then MakeRevol, or MakeRevol of wires then BRepOffsetAPI_MakeThickSolid::MakeThickSolidByJoin / BRepOffset_MakeSimpleOffset on the sheet. Validate self-intersection with BRepAlgoAPI_Check. Boolean step PS-01.
- **Priority:** P0

### PS-04 Sweep
- **Purpose:** Sweep planar faces/regions (solid), edges/curves (surface) or wall curves (thin) along a connected path, with optional profile control, twist and scale; new/add/remove/intersect.
- **Sources:** https://cad.onshape.com/help/Content/PartStudio/sweep.htm ; FS: `sweep` (sweep.fs), `opSweep` (geomOperations.fs).
- **Selection inputs:**
  - "Faces and sketch regions to sweep" `profiles` (Solid): planar FACEs / sketch regions, non-construction; multi.
  - "Edges and sketch curves to sweep" `surfaceProfiles` (Surface): non-construction EDGEs or non-sketch WIRE bodies; multi (wire edges followed to laminar source).
  - "Faces and sketch regions to sweep" `wallShape` (Thin): EDGEs, FACEs (boundary edges used) or non-sketch WIRE bodies (their edges), non-construction; multi; each connected chain is swept and thickened separately.
  - "Sweep path" `path`: non-construction EDGEs or non-sketch WIRE bodies; multi; any order, must form one connected path (open or closed).
  - "Faces to lock" `lockFaces`: FACEs (non-construction); multi (Lock profile faces).
  - "Direction to lock" `lockDirectionQuery`: ALLOWS_DIRECTION or mate connector; max 1.
- **Parameters:**

| FS id | UI label | Type | Enum values / default / bounds | Visible when / notes |
|---|---|---|---|---|
| `bodyType` | Creation type | ExtendedToolBodyType | SOLID/SURFACE/THIN, default SOLID, remembers previous | always |
| `operationType` | Result body operation type | NewBodyOperationType | default NEW | SOLID/THIN |
| `surfaceOperationType` | Result body operation type | NewSurfaceOperationType | default NEW | SURFACE |
| `profiles` / `surfaceProfiles` / `wallShape` | see above | Query | | per bodyType |
| `midplane` | Mid plane | boolean | false | THIN |
| `thickness1` | Thickness 1 | length | ZERO_INCLUSIVE_OFFSET_BOUNDS (0..500 m, 5 mm / 0.25 in) | THIN, !midplane |
| `flipWall` | Flip wall | boolean | false | THIN, !midplane |
| `thickness2` | Thickness 2 | length | 0..500 m, default 0 | THIN, !midplane |
| `thickness` | Thickness | length | ZERO_INCLUSIVE_OFFSET_BOUNDS | THIN, midplane |
| `path` | Sweep path | Query | | always |
| `profileControl` | Profile control | ProfileControlMode | NONE "None" (default), KEEP_ORIENTATION "Keep profile orientation", LOCK_FACES "Lock profile faces", LOCK_DIRECTION "Lock profile direction"; remembers previous | always |
| `lockFaces` | Faces to lock | Query | | LOCK_FACES |
| `lockDirectionQuery` | Direction to lock | Query, max 1 | | LOCK_DIRECTION |
| `hasTwist` | Twist | boolean | false | profileControl NONE or KEEP_ORIENTATION |
| `twistType` | Twist type | SweepTwistType | TURNS "Turns" (default), ANGLE "Angle", PITCH "Pitch"; remembers previous | hasTwist |
| `turns` | Revolutions | real | SWEEP_TURNS_BOUNDS 0..1000, default 1 | TURNS |
| `angle` | Rotation angle | angle | SWEEP_ANGLE_BOUNDS 0..360000 deg, default 0 | ANGLE |
| `pitch` | Pitch length | length | SWEEP_PITCH_BOUNDS 1e-6 m..500 m, default 25 mm / 1 in | PITCH |
| `ccw` | Opposite direction | boolean (circular arrow) | false | hasTwist |
| `hasScale` | Scale | boolean | false | always |
| `scaleFactor` | Scale factor | real | SCALE_BOUNDS 1e-5..1e5, default 1.0 | hasScale |
| `trimEnds` | Trim ends | boolean | true | THIN |
| merge / surface-merge scope | | | PS-01 | per bodyType |

- **Behavior / semantics:**
  - Profile frame along path: NONE - profile rotates to stay normal to the path (minimal-rotation / corrected Frenet frame); KEEP_ORIENTATION - profile keeps its original orientation (pure translation along path); LOCK_FACES - profile is kept aligned with the normals of the selected faces (frame binormal from face normal at the closest point; e.g. sweep along an edge of a curved face); LOCK_DIRECTION - profile is kept perpendicular to the given direction (fixed binormal).
  - Twist: total twist angle = turns*2*pi (TURNS), = angle (ANGLE), = pathLength/pitch*2*pi (PITCH; error if > 1000 revolutions: "Computed number of revolutions outside limit (between 0 and 1000). Current value is N."). Twist applied linearly along arc length; sign negative unless `ccw` (Opposite direction) is set. Twist and scale require a G1-tangent path (SWEEP_TWIST_REQUIRE_TANGENT_PATH, SWEEP_SCALE_REQUIRE_TANGENT_PATH, SWEEP_TWIST_SCALE_REQUIRE_TANGENT_PATH). Twist hidden when locking faces/direction.
  - Scale: linear scale law from 1 at path start to `scaleFactor` at path end, about the path point (help: "transform the overall size of the sweep profile proportionally at the end of the sweep"). Not allowed for multi-profile sweeps (SWEEP_MULTIPROFILE_SCALE_NOT_ALLOWED).
  - If the profile lies somewhere along the path (not at an end), the path is broken at the profile plane and the sweep runs in both directions over the whole path (help). Profiles need not touch the path (info SWEEP_START_NOT_ON_PROFILE / SWEEP_PATH_PROFILE_NO_INTERSECTION recommend it).
  - Each selected profile region produces its own body; profiles must be single bodies and all edges or all faces (SWEEP_DISJOINT_PROFILE, SWEEP_MIXED_PROFILE, SWEEP_PROFILE_DIM_MISMATCH for multi-profile first/last).
  - Thin: each profile chain is swept into a sheet, then thickened (opThicken) by wallThickness_1/_2 (swapped by flipWall; mid plane = half each). With `trimEnds` and an open path, the thickened side walls are trimmed flush with the planes at the path ends (side geometry rule supplied to thicken); warnings THIN_SWEEP_3D_PROFILE_TRIM_WARNING, THIN_SWEEP_FAILED_TO_FIND_TRIM_PLANES_WARNING; errors THIN_SWEEP_TRIM_FAILED, THIN_SWEEP_THICKEN_FAILED.
  - Pattern-aware result transform; then solid boolean step or surface join (surface join matches cap edges to profile edges and swept edges to path edges).
  - Help lists an "Extend to full path" option that does not exist in the current FS precondition (the both-directions behavior is automatic); treat as doc drift.
- **Edge cases & errors:** SWEEP_SELECT_PROFILE, SWEEP_SELECT_PATH, SWEEP_SURF_NO_CURVE_PROFILE, SWEEP_INVALID_PATH "Sweep path curves are not all connected.", SWEEP_PATH_FAILED "Sweep path is self intersecting.", SWEEP_SELF_INT "Result of sweep intersects itself...", SWEEP_FAILED, SWEEP_PROFILE_FAILED, SWEEP_PATH_NO_CONSTRUCTION, SWEEP_BAD_LOCK_FACES, SWEEP_BAD_LOCK_DIRECTION, SWEEP_SELECT_DIRECTION, SWEEP_TWIST_ANGLE_EXCEEDS_LIMITS, SWEEP_TWIST_PITCH_EXCEEDS_LIMITS.
- **OCCT mapping:** BRepOffsetAPI_MakePipeShell(spine wire) with: NONE -> SetMode(Standard_False) corrected Frenet (GeomFill_IsCorrectedFrenet) or SetDiscreteMode; KEEP_ORIENTATION -> SetMode(gp_Dir) fixed / GeomFill_IsFixed (or SetMode(gp_Ax2)); LOCK_DIRECTION -> SetMode(gp_Dir binormal) (GeomFill_IsConstantNormal); LOCK_FACES -> SetMode(TopoDS_Shape support face) (auxiliary surface, GeomFill "IsGuideAC"/support-surface mode via BRepFill_EdgeOnSurfLaw). Twist + scale -> Add(profile, WithContact, WithCorrection) plus SetLaw(profile, Law_Function scale law, ...) for scale; twist requires custom: GeomFill_LocationLaw with a rotation law (subclass GeomFill_LocationLaw / use SetMode with an auxiliary helical guide wire SetMode(guideWire, CurvilinearEquivalence, KeepContact=false)) — no direct twist API; recommended: build a helix-like auxiliary spine and use guide mode. Solid: MakeSolid() after Build. Multi-region: one PipeShell per face (outer + inner wires swept separately, then BRepBuilderAPI_MakeSolid with inner shells, or sweep the face via BRepOffsetAPI_MakePipe(spine, face) when no law needed). Thin: pipe shell of open wire -> BRepOffsetAPI_MakeThickSolid / BRepOffset_MakeSimpleOffset (thicken) then BRepAlgoAPI_Splitter/Cut with end planes for trim. Path splitting when profile is mid-path: split spine at the profile plane (BRepAlgoAPI_Section) and sweep both halves, then fuse.
- **Priority:** P0

### PS-05 Loft
- **Purpose:** Create a solid, surface or thin solid that smoothly transitions through an ordered list of profiles (regions, faces, edges/curves, or a point at either end), optionally controlled by guide curves, a path, end conditions and vertex connections.
- **Sources:** https://cad.onshape.com/help/Content/PartStudio/loft.htm ; FS: `loft` (loft.fs), `opLoft` (geomOperations.fs).
- **Selection inputs:**
  - "Profiles" array (`sheetProfilesArray` for Solid; `wireProfilesArray` for Surface/Thin), order = loft order, drag-reorderable (UIHint COLLAPSE_ARRAY_ITEMS; item label = selection). Each item:
    - Solid: "Faces and sketch regions" `sheetProfileEntities`: FACE or non-sketch SHEET body (non-construction) or VERTEX. One single face/region per profile (no inner loops).
    - Surface/Thin: "Edges, curves and sketches" `wireProfileEntities`: EDGE, FACE, non-sketch WIRE or SHEET body (non-construction) or VERTEX; multiple tangent-connected curves may form one composite profile (expand the item to add).
    - Point (vertex) profiles only as first or last profile.
  - "Guides" array `guidesArray` (when "Guides and continuity"): each item "Edges, curves and sketches" `guideEntities`: non-construction EDGEs or non-sketch WIRE bodies (composite G1 chains allowed).
  - Path "Edges, curves and sketches" `spine`: non-construction EDGEs or WIRE bodies.
  - "Match connections" array `connections`: each item "Vertices or edges" `connectionEntities`: non-construction EDGEs or VERTEXes (no edge points); one pick per profile.
  - "Adjacent faces" (`adjacentFacesStart`/`adjacentFacesEnd`): FACEs whose underlying geometry is coincident with the profile curve.
  - "Start direction"/"End direction": ALLOWS_DIRECTION or mate connector, max 1.
- **Parameters:**

| FS id | UI label | Type | Enum values / default / bounds | Visible when / notes |
|---|---|---|---|---|
| `bodyType` | Creation type | ExtendedToolBodyType | SOLID/SURFACE/THIN, default SOLID, remembers previous | always |
| `operationType` | Result body operation type | NewBodyOperationType | NEW | SOLID/THIN |
| `surfaceOperationType` | Result body operation type | NewSurfaceOperationType | NEW | SURFACE |
| `sheetProfilesArray[].sheetProfileEntities` | Profiles / Faces and sketch regions | array of Query | | SOLID |
| `wireProfilesArray[].wireProfileEntities` | Profiles / Edges, curves and sketches | array of Query | | SURFACE/THIN |
| `midplane`, `thickness1`, `flipWall`, `thickness2`, `thickness` | Mid plane / Thickness 1 / Flip wall / Thickness 2 / Thickness | as sweep thin | ZERO_INCLUSIVE_OFFSET_BOUNDS / 0-default | THIN |
| `startCondition` | Start profile condition | LoftEndDerivativeType | DEFAULT "None" (default), NORMAL_TO_PROFILE "Normal to profile", TANGENT_TO_PROFILE "Tangent to profile", MATCH_TANGENT "Match tangent", MATCH_CURVATURE "Match curvature", NORMAL_DIRECTION "Normal direction", TANGENT_DIRECTION "Tangent direction" | group "End conditions" |
| `adjacentFacesStart` | Adjacent faces | Query FACE | | startCondition MATCH_TANGENT/MATCH_CURVATURE |
| `startMagnitude` | Start magnitude | real | CLAMP_MAGNITUDE_REAL_BOUNDS -1e5..1e5, default 1 | startCondition != DEFAULT |
| `startDirection` | Start direction | Query, max 1 | | NORMAL_DIRECTION/TANGENT_DIRECTION |
| `endCondition`, `adjacentFacesEnd`, `endMagnitude`, `endDirection` | End profile condition / Adjacent faces / End magnitude / End direction | same as start | | same rules |
| `trimProfiles` | Trim profiles | boolean | false | SURFACE/THIN |
| `addGuides` | Guides and continuity | boolean | false | always |
| `guidesArray[].guideEntities` | Guides / Edges, curves and sketches | array of Query | | addGuides |
| `guidesArray[].guideDerivativeType` | Continuity | LoftGuideDerivativeType | DEFAULT "None", NORMAL_TO_GUIDE "Normal to guide", TANGENT_TO_GUIDE "Tangent to guide", MATCH_TANGENT "Match tangent", MATCH_CURVATURE "Match curvature"; MATCH_LAST_ARRAY_ITEM (new items copy previous) | addGuides |
| `guidesArray[].guideDerivativeMagnitude` | Magnitude | real | CLAMP_MAGNITUDE_REAL_BOUNDS | always hidden |
| `trimGuidesByProfiles` | Trim guides | boolean | true | addGuides and SURFACE/THIN |
| `addSections` | Path | boolean | false | always |
| `spine` | Edges, curves and sketches | Query | | addSections |
| `sectionCount` | Section count | integer | 1..50, default 5 | addSections |
| `matchConnections` | Connections | boolean | false | always |
| `connections[].connectionEntities` | Match connections / Vertices or edges | array of Query | | matchConnections |
| `connections[].connectionEdgeQueries` | Edge queries | Query | hidden, synced | matchConnections |
| `connections[].connectionEdgeParameters` | Edge parameters | any (array of numbers) | hidden; arc-length parameter of the picked point on each edge (set by connection manipulator) | matchConnections |
| `makePeriodic` | Make periodic | boolean | false; always hidden | — |
| `showIsocurves` | Show isocurves | boolean | false | always |
| `curveCount` | Count | integer | ISO_GRID_BOUNDS 1..50, default 10 | showIsocurves |
| `trimEnds` | Trim ends | boolean | true | THIN |
| merge / surface-merge scope | | | PS-01 | per bodyType |

- **Behavior / semantics:**
  - Profiles lofted in array order; >= 2 profiles required unless a closed guide or closed path makes the loft periodic (LOFT_TWO_PROFILES, LOFT_PERIODIC_ERROR needs >= 3 profiles / closed guide / closed path). A closed guide always creates a periodic (closed) loft.
  - Profile alignment: with guides, guides define correspondence; else connections; else Onshape estimates by vertex proximity (info LOFT_ALIGNMENT_INFO). Automatic connections shown in magenta when no user connections exist. Profiles with different vertex counts get internal vertices added (LOFT_VERTEX_ADDITIONS on failure). Each connection must have exactly one pick per profile; a pick may be a vertex or a point on an edge (edge parameter stored, draggable with a manipulator).
  - End conditions (derivative constraints at first/last profile): Normal to profile - loft leaves the (planar) profile with tangents parallel to profile normal; Tangent to profile - tangent to the profile plane; Match tangent / Match curvature - G1/G2 to adjacent faces (auto-detected from profile owner or the "Adjacent faces" picks); Normal/Tangent direction - normal/tangent to a picked vector. Magnitude scales the tangent vector length (negative allowed). Point profiles cannot take end conditions (LOFT_NO_CLAMPS_ON_POINT_PROFILE) except with guides restrictions.
  - Guides: must intersect every profile boundary (LOFT_GUIDE_PROFILE_INTERSECTION) and be G1 smooth (LOFT_GUIDE_NOT_SMOOTH); continuity per guide (Normal/Tangent to guide require planar guides; match tangent/curvature need faces adjacent to the guide; internal guides cannot have continuity). Trim guides/profiles restrict the surface to the first/last profile/guide when extents differ (LOFT_INCONSISTENT_EXTENT otherwise).
  - Path: intermediate sections are generated along the spine (sectionCount extra sections) by interpolating profiles in planes normal to the path; path must be G1 (approximated with warning), contiguous, pass through all profile planes in order; max 3 guides with a path; incompatible with non-planar profiles and with point profiles + guides.
  - Thin: surface loft then thicken (thickness1/2, flip, mid plane) with optional end trimming (THIN_LOFT_* warnings/errors).
  - Surface Add: surface join with makeSolid=true (a closed result becomes a solid).
  - Show isocurves draws `curveCount` iso-parameter curves per direction on each loft face (display only).
- **Edge cases & errors:** LOFT_SELECT_PROFILES, LOFT_PROFILE_SINGLE_FACE, LOFT_PROFILE_SOLID, LOFT_PROFILE_POINT, LOFT_PROFILE_NO_INNER_LOOPS, LOFT_FAILED ("Check profile order, guide/profile intersections or end conditions."), LOFT_INVALID (self-intersecting), LOFT_GUIDE_POINT_INTERSECTION, LOFT_VERTEX_MATCHING, LOFT_CONNECTION_MATCHING, LOFT_DIRECTION_ERROR, LOFT_PROFILE_ALIGNMENT, LOFT_START/END_CONDITIONS_FAILED, LOFT_NO_FACE_FOR_START/END_CLAMP, LOFT_NO_PLANE_FOR_START/END_CLAMP, LOFT_NO_DIRECTION_FOR_START/END, LOFT_SPINE_* (disjoint, self-intersecting, profiles not in order, no intersection, too many guides, nontangent), LOFT_CANT_TRIM_PROFILES_INFO, LOFT_CANT_TRIM_GUIDES_INFO.
- **OCCT mapping:** No guides, no conditions: BRepOffsetAPI_ThruSections(isSolid, ruled=false) with AddWire / AddVertex (point ends), CheckCompatibility(true) for vertex matching, SetSmoothing, SetParType/SetContinuity(GeomAbs_C2). Connections: pre-process wires to equal edge counts and rotate start vertices (custom; BRepFill_CompatibleWires does auto alignment). Guides: no ThruSections support -> use GeomFill (GeomFill_NSections + custom), or build curve network surfaces: BRepOffsetAPI_MakeFilling (BRepFill_Filling) / GeomFill_ConstrainedFilling / Geom surface via skinning (GeomFill_AppSurf) with guide constraints, or BRepOffsetAPI_MakePipeShell with multiple profiles + SetMode(guideWire) for single guide. End conditions (tangent/curvature matching): custom — modify the end rows of the skinned B-spline control net (GeomFill_AppSurf does not expose derivative constraints; consider GeomAPI_PointsToBSplineSurface/approx with constraints or BRepFill_Filling with G1/G2 boundary constraints). Path with sections: generate intermediate wires by sampling the spine (GCPnts_UniformAbscissa) and interpolating profiles, then ThruSections. Solid: ThruSections isSolid or sew + MakeSolid. Thin: thicken (BRepOffset_MakeSimpleOffset / MakeThickSolid). Gap: guides + end conditions + connections need substantial custom surfacing work.
- **Priority:** P0 (basic profile loft), guides/conditions P1

### PS-06 Thicken
- **Purpose:** Give thickness to faces or surface bodies, creating solids (new) or adding/removing/intersecting with parts.
- **Sources:** https://cad.onshape.com/help/Content/PartStudio/thicken.htm ; FS: `thicken` (thicken.fs), `opThicken`.
- **Selection inputs:** "Faces and surfaces to thicken" `entities`: FACEs (of solids or sheets) or non-sketch SHEET bodies, non-construction (sketch regions allowed as faces); multi; mesh rejected.
- **Parameters:**

| FS id | UI label | Type | Enum values / default / bounds | Visible when / notes |
|---|---|---|---|---|
| `operationType` | Result body operation type | NewBodyOperationType | NEW (edit logic switches to ADD when a solid's face is picked, REMOVE when flipped) | always |
| `entities` | Faces and surfaces to thicken | Query | | always |
| `midplane` | Mid plane | boolean | false | always |
| `thickness1` | Thickness 1 | length | ZERO_INCLUSIVE_OFFSET_BOUNDS 0..500 m, default 5 mm / 0.25 in | !midplane |
| `oppositeDirection` | Opposite direction | boolean | false (swaps thickness1/thickness2 sides) | !midplane |
| `thickness2` | Thickness 2 | length | 0..500 m, default 0 | !midplane |
| `thickness` | Thickness | length | ZERO_INCLUSIVE_OFFSET_BOUNDS (FS fallback default 0.5 in) | midplane (half each side) |
| `keepTools` | Keep tools | boolean | false (UI); FS fallback true | always |
| `defaultScope`/`booleanScope` | Merge with all / Merge scope | | PS-01.2 | op != NEW |

- **Behavior / semantics:** thickness1 is applied along the face/surface normal, thickness2 opposite; Opposite direction swaps them; Mid plane = thickness/2 each side. opThicken offsets the selected faces (each connected face set becomes one solid; adjacent selected faces are thickened together with side walls along open edges). Keep tools unchecked deletes the input sheet bodies; solid bodies and sketches are never deleted. Then solid boolean step. Edit logic: if picked faces belong to solids and the user has not set the operation, operation = ADD (or REMOVE when Opposite direction is set) and merge scope = owner parts; toggling Add<->Remove flips direction. Pattern-aware.
- **Edge cases & errors:** THICKEN_SELECT_ENTITIES "Select surfaces or faces to thicken."; THICKEN_FAILED "Failed to thicken all selections." (offset larger than min curvature radius, self-intersection); thickness 0 on both sides invalid (inferred).
- **OCCT mapping:** BRepOffset_MakeSimpleOffset (thick solid from a shell, constant offset; supports two-sided via shifting) or BRepOffsetAPI_MakeThickSolid with MakeThickSolidByJoin on a sheet (no faces removed) for one side; for two-sided: offset sheet by +t1 and -t2 (BRepOffsetAPI_MakeOffsetShape::PerformByJoin) and build side faces between corresponding free boundaries (BRepFill::Face / BRepOffsetAPI_ThruSections ruled), sew (BRepBuilderAPI_Sewing) + MakeSolid. Boolean step PS-01.
- **Priority:** P1

### PS-07 Enclose
- **Purpose:** Create solid(s) from the volume(s) fully bounded by a set of intersecting/touching surfaces, faces, planes and parts.
- **Sources:** https://cad.onshape.com/help/Content/PartStudio/enclose.htm ; FS: `enclose` (enclose.fs), `opEnclose`.
- **Selection inputs:** "Entities" `entities`: non-sketch SHEET or SOLID BODYs, or FACEs (incl. construction planes, part faces), mesh allowed; multi.
- **Parameters:**

| FS id | UI label | Type | Default | Visible when |
|---|---|---|---|---|
| `operationType` | Result body operation type | NewBodyOperationType | NEW | always |
| `entities` | Entities | Query | | always |
| `defaultScope`/`booleanScope` | Merge with all / Merge scope | | PS-01.2 | op != NEW |
| `keepTools` | Keep tools | boolean | false | always |

- **Behavior / semantics:** opEnclose computes all closed cells formed by the selected entities (planes treated as infinite / extended surfaces, faces as their full extent - inferred) and creates a solid for each enclosed region (mergeResults merges adjacent regions into one part, inferred default). If Keep tools is off, the owning bodies of the selections (modifiable, not sketches, not construction planes) are deleted after the boolean step and excluded from the merge scope. Then solid boolean step.
- **Edge cases & errors:** ENCLOSE_NOTHING_SELECTED "Select faces, parts, or planes to form a solid."; ENCLOSE_NO_REGION "Selections do not enclose a region."; ENCLOSE_CANNOT_CREATE_SOLID; ENCLOSE_CANNOT_MERGE_REGIONS; ENCLOSE_UNKNOWN_ERROR.
- **OCCT mapping:** BOPAlgo_MakerVolume (exactly this operation: builds solids from arbitrary intersecting faces/shells/planes; SetIntersect(true), AddArgument; planes as large bounded faces) then optional fuse of adjacent result solids (BRepAlgoAPI_Fuse + ShapeUpgrade_UnifySameDomain). Exclude cells that touch the artificial plane bounds (cells not fully enclosed).
- **Priority:** P2

### PS-08 Fillet
- **Purpose:** Round edges (edge fillet: constant, asymmetric, conic, curvature-continuous, width-controlled, variable, partial) or replace a set of faces with a seamless blend between two side faces (full round).
- **Sources:** https://cad.onshape.com/help/Content/PartStudio/fillet.htm ; FS: `fillet` (fillet.fs, edgeBlendCommon.fs), `opFillet`, `opFullRoundFillet`. Shortcut: Shift+F.
- **Selection inputs:**
  - "Entities to fillet" `entities` (Edge): two-sided EDGEs or FACEs (face = all its edges) of non-active-sheet-metal bodies, or sheet-metal definition-vertex edges (corner edges); modifiable, non-sketch, non-construction; multi; box-select picks edges.
  - Full round: "First side face" `side1Face` (FACE, max 1), "Second side face" `side2Face` (FACE, max 1), "Faces to round" `centerFaces` (FACEs, one contiguous set, same body); active sheet metal prohibited.
  - Partial bound entity "Select an entity or mate connector" (`startPartialEntity`/`endPartialEntity`): FACE (incl. planes, mesh ok) or ALLOWS_VERTEX (vertex / mate connector); max 1.
  - Variable: vertex array items (hidden `vertex` query, picked in viewport: vertices on the fillet chain shown as black dots), point-on-edge items "Edge" (EDGE, max 1).
  - "Edges to keep" `keepEdges`: two-sided EDGEs. "Corners to exclude" `smoothCornerExceptions`: VERTEXes.
- **Parameters:**

| FS id | UI label | Type | Enum values / default / bounds | Visible when / notes |
|---|---|---|---|---|
| `filletType` | Fillet type | FilletType | EDGE "Edge" (default), FULL_ROUND "Full round" | always |
| `entities` | Entities to fillet | Query | | EDGE |
| `side1Face`, `side2Face`, `centerFaces` | First side face / Second side face / Faces to round | Query | | FULL_ROUND |
| `tangentPropagation` | Tangent propagation | boolean | true (UI) | always |
| `blendControlType` | Measurement | BlendControlType | RADIUS "Radius" (default), WIDTH "Width"; remembers previous | EDGE |
| `crossSection` | Control ("Cross sectional control") | FilletCrossSection | CIRCULAR "Distance" (default), CONIC "Conic", CURVATURE "Curvature", CHAMFER (hidden, unsupported: FILLET_CHAMFER_UNSUPPORTED); remembers previous | EDGE |
| `radius` | Radius | length, tolerant, remembers previous | BLEND_BOUNDS 1e-5..500 m, default 5 mm / 0.2 in | EDGE, RADIUS, CIRCULAR |
| `nonCircularRadius` | Radius | length, remembers previous | BLEND_BOUNDS | EDGE, RADIUS, CONIC/CURVATURE |
| `width` | Width | length, remembers previous | BLEND_BOUNDS | EDGE, WIDTH |
| `rho` | Rho | real | FILLET_RHO_BOUNDS 0..0.99999, default 0.5 | CONIC |
| `magnitude` | Magnitude | real | FILLET_RHO_BOUNDS 0..0.99999, default 0.5 | CURVATURE |
| `defaultsChanged` | Defaults changed | boolean | hidden; used to suppress VRFILLET_NO_EFFECT info | — |
| `isAsymmetric` | Asymmetric | boolean | false | RADIUS |
| `otherRadius` | Second radius | length | BLEND_BOUNDS | isAsymmetric |
| `flipAsymmetric` | Flip asymmetric | boolean | false | isAsymmetric |
| `isPartial` | Partial fillet | boolean | false | RADIUS |
| `startPartialType` | End type | EndTypePartialFillet | PERCENTAGE "Position" (default), ENTITY "Entity", OFFSET "Offset" | isPartial |
| `startPartialOffset` | Start offset | length | VR_BLEND_BOUNDS 0..500 m, default 5 mm / 0.2 in | OFFSET |
| `startPartialEntity` | Select an entity or mate connector | Query max 1 | | ENTITY |
| `partialFirstEdgeTotalParameter` | Start position | real | 0..1, default 0.01 (fraction of chain length) | PERCENTAGE |
| `partialOppositeParameter` | Opposite direction | boolean | true | isPartial |
| `useTrimmedFirstBound` | Trim to face boundaries | boolean | false | ENTITY |
| `secondBound` | Second bound | boolean | false | isPartial |
| `endPartialType` | End type | EndTypePartialFillet | PERCENTAGE | secondBound |
| `endPartialOffset` | End offset | length | VR_BLEND_BOUNDS | secondBound, OFFSET |
| `endPartialEntity` | Select an entity or mate connector | Query max 1 | | secondBound, ENTITY |
| `useTrimmedSecondBound` | Trim to face boundaries | boolean | false | secondBound, ENTITY |
| `partialSecondEdgeTotalParameter` | End position | real | 0..1, default 0.99 | secondBound, PERCENTAGE |
| `isVariable` | Variable fillet | boolean | false | RADIUS |
| `vertexSettings[]` | Vertices (item label "[radius] vertex"; no reorder) | array | | isVariable |
| `vertexSettings[].vertex` | Vertex | Query VERTEX max 1 | hidden (picked in viewport) | |
| `vertexSettings[].vertexRadius` | Radius | length | VR_BLEND_BOUNDS 0..500 m (zero allowed only at chain ends), default 5 mm; new items copy previous | |
| `vertexSettings[].vertexOtherRadius` | Second radius | length | BLEND_BOUNDS | isAsymmetric |
| `vertexSettings[].vertexFlipAsymmetric` | Flip asymmetric | boolean | | isAsymmetric |
| `vertexSettings[].variableRho` | Rho | real | FILLET_RHO_BOUNDS | CONIC |
| `vertexSettings[].variableMagnitude` | Magnitude | real | FILLET_RHO_BOUNDS | CURVATURE |
| `pointOnEdgeSettings[]` | Points on edges ("Add point on edge") | array | | isVariable |
| `pointOnEdgeSettings[].edge` | Edge | Query EDGE max 1 | | |
| `pointOnEdgeSettings[].edgeParameter` | Location | real | 0.001..0.999, default 0.5 (edge parameter, non-arc-length) | |
| `pointOnEdgeSettings[].pointOnEdgeRadius` / `...OtherRadius` / `...FlipAsymmetric` / `...VariableRho` / `...VariableMagnitude` | Radius / Second radius / Flip asymmetric / Rho / Magnitude | | as vertex settings | |
| `smoothTransition` | Smooth transition | boolean | false | isVariable |
| `allowEdgeOverflow` | Allow edge overflow | boolean | true | RADIUS and crossSection != CURVATURE (CURVATURE forces true) |
| `keepEdges` | Edges to keep | Query | | allowEdgeOverflow |
| `smoothCorners` | Smooth fillet corners | boolean | false | RADIUS and crossSection != CURVATURE |
| `smoothCornerExceptions` | Corners to exclude | Query | | smoothCorners |

- **Behavior / semantics:**
  - Faces selected = fillet all edges of the face. Tangent propagation adds all edges G1-connected to selected edges (chain). Convex edges remove material, concave edges add material, on the same body (no new body created). Works on solids and surfaces (sheet edges).
  - Cross sections: Distance (circular arc of given radius, rolling ball); Conic - conic section with rho: rho < 0.5 elliptical, 0.5 parabolic, > 0.5 hyperbolic (0.01 nearly chamfer, 0.99 nearly sharp); Curvature - G2 (curvature-continuous) blend with magnitude 0..0.999 controlling the shape (tangency weight); edge overflow always allowed.
  - Width measurement: constant chord distance between the two fillet boundary (tangent) lines instead of radius; converted to radius per cross-section from the dihedral angle (edgeBlendCommon `findRadiusToOffsetRatio`, inferred: r = w / (2 sin(theta/2)) style relation where theta is the angle between surface normals).
  - Asymmetric: two setback distances (radius on one face, second radius on the other); Flip asymmetric swaps sides. For the circular case this yields an elliptical-like section (two different contact distances).
  - Variable: radius (and rho/magnitude/second radius) specified at chain vertices and at arbitrary points on edges; interpolated along the chain (linear, or smooth (C1/C2-like) when Smooth transition). Closed chains need >= 2 vertices. Zero radius allowed only at chain end vertices. Unset vertices fall back to the default radius; VRFILLET_NO_EFFECT info if default unused. Edge point locations have drag manipulators.
  - Partial: fillet only a portion of a single edge or single tangent chain (open path; closed requires second bound). Start bound by Position (fraction of total chain length measured from one end), Offset (distance from the end vertex), or Entity (plane/face/vertex/mate connector where the bound is the intersection with the edge; Trim to face boundaries limits by the actual face extent). Opposite direction chooses which side of the bound is filleted. Second bound limits the other end. Fillet end is capped (cap faces built from bound, with leftover faces deleted via opDeleteFace).
  - Allow edge overflow: the rolling ball may run over adjacent edges, modifying/consuming them to keep the cross-section; "Edges to keep" lists edges that must be preserved (fillet then rolls along them, inferred). Off: fillet stops/conforms at existing edges.
  - Smooth fillet corners: at vertices where 3+ filleted edges meet, build a smooth (G2-ish) corner patch instead of a sharp setback/vertex blend; exceptions excluded.
  - Sheet metal: corner edges of active sheet metal are routed to sheet-metal corner-break handling (sheetMetalAwareFillet); partial fillet not supported in active sheet metal.
  - Manipulator: linear drag arrow on the first selected edge controlling radius/width (not shown for variable); partial bound arrows; variable point arrows.
  - Tolerant radius (MBD) registers fillet faces as radius dimension; warnings TOLERANT_RADIUS_NO_ASYMMETRY / _NO_VARIABLE_RADIUS.
  - Full round: replaces `centerFaces` with a single blend surface tangent to side1 and side2 faces whose radius varies to fit; tangent propagation extends side/center sets along tangent faces. Only one contiguous center face set; all faces from one body.
  - "Setback" (vertex setback distances, as in other CAD) does not exist in Onshape fillet; not required.
- **Edge cases & errors:** FILLET_SELECT_EDGES, FILLET_FAILED, FILLET_PARTIAL_FAIL "Could not fillet selections on some parts.", FILLET_FAIL_SMOOTH, FILLET_EDGES_NOT_MANIFOLD, FILLET_OVERLAP, FILLET_FACE_RANGE_TOO_LARGE ("Radius of fillet on face too large." — on failure Onshape displays a ghost visualization of the requested fillet), FILLET_RANGE_INCONSISTENT_EDGE, FILLET_REQUIRES_SURFACE_EXTENSION, FILLET_PRODUCED_SELF_INT_SURFACE, FILLET_RHO_TOO_LARGE, FILLET_VERTEX_EDGES_COMPLICATED, FILLET_ADJOINING_EDGE_NOT_FILLETED, FILLET_BOUNDARY_INTERSECTS_EDGE, FILLET_BOUNDARY_OVERLAP, FILLET_EDGE_OVERLAPPED_BY_FILLET, FILLET_ILLEGAL_END_BOUNDARY, FILLET_OVERLAPS_EDGE_LOOP; VRFILLET_* (RADIUS/RHO/MAG required at vertex/point, VERTEX_NOT_ON_CHAIN, EDGE_NOT_ON_CHAIN, INVALID_CHAIN, MULTI_SELECTION, INTERNAL_ZERO, ASYMMETRIC_RADIUS_REQUIRED_*, BAD_COEDGE); PARTIAL_FILLET_BAD_INPUT_ERROR "Only one edge or one chain of tangent edges can be partially fillet.", PARTIAL_FILLET_CLOSED_PATH_ERROR, PARTIAL_FILLET_INVALID_BOUNDS_ERROR, PARTIAL_FILLET_INVALID_BOUND_ENTITY, PARTIAL_FILLET_OFFSET_BOUNDARY_TOO_LARGE, CANNOT_USE_PARTIAL_FILLET_IN_SHEET_METAL; FULL_ROUND_SELECT_FACES, FULL_ROUND_ADJACENT_CENTER_FACES, FULL_ROUND_COMMON_BODY.
- **OCCT mapping:** BRepFilletAPI_MakeFillet(shape, ChFi3d_Rational or ChFi3d_QuasiAngular) : Add(radius, edge) for constant; Add(r1, r2, edge) / SetRadius(r1, r2, contour, ind) for linear variable; SetRadius(TColgp_Array1OfPnt2d (param, radius) law, ...) or SetRadius(Handle(Law_Function)) for multi-point variable (Law_Interpol for smooth transition, Law_Linear piecewise otherwise). Tangent propagation is native (contours follow G1 chains; to disable, split contours — ChFi3d always propagates along tangent edges, so non-propagation needs custom handling). Asymmetric (two different contact distances): not supported by ChFi3d (rolling ball only) — custom blend needed (e.g. elliptic/conic section surface between two offset contact curves). Conic/curvature (G2) cross sections: not supported by OCCT ChFi3d (only circular; ChFi3d_Polynomial shape gives approximate) — gap, needs custom blend surface generation (e.g. compute spine + contact curves with BRepBlend/ BlendFunc_* for circular contact lines, then build conic/G2 section surfaces via GeomFill_Generator / GeomFill_NSections with conic profiles, sew and replace). Width: convert to radius per section. Partial fillet: split edge at bound points (BRepFeat_SplitShape / BRepAlgoAPI_Splitter with plane) and fillet sub-edge; end capping custom (OCCT leaves free fillet ends when fillet stops mid-edge -> fails; need custom cap). Full round: custom (BRepFilletAPI_MakeFillet cannot vary to tangency of three faces) — approach: compute variable-radius fillet between side faces with radius law = half distance, or use BRepOffsetAPI_MakeFilling with G1 to sides then remove center faces (BRepAlgoAPI_Defeaturing) and sew. Allow edge overflow / keep edges / smooth corners: partially in ChFi3d (corner handling is automatic; no user control) — gaps. Post: ShapeFix_Shape, ShapeUpgrade_UnifySameDomain.
- **Priority:** P0 (constant/tangent-propagated/variable linear), P1 (asymmetric, partial, conic, full round), P2 (curvature, edge-overflow controls, smooth corners)

### PS-09 Chamfer
- **Purpose:** Bevel edges by equal distance, two distances, or distance and angle; with offset or tangent measurement.
- **Sources:** https://cad.onshape.com/help/Content/PartStudio/chamfer.htm ; FS: `chamfer` (chamfer.fs), `opChamfer`.
- **Selection inputs:**
  - "Entities to chamfer" `entities`: two-sided EDGEs or FACEs (all edges of face) of non-active-sheet-metal bodies, or sheet-metal definition-vertex edges; modifiable, non-sketch, non-construction; multi; box select picks edges.
  - "Direction overrides" `directionOverrides`: subset of the chamfered two-sided EDGEs whose asymmetric direction is flipped individually.
- **Parameters:**

| FS id | UI label | Type | Enum values / default / bounds | Visible when / notes |
|---|---|---|---|---|
| `entities` | Entities to chamfer | Query | | always |
| `chamferMethod` | Measurement | ChamferMethod | FACE_OFFSET "Offset" (default), APEX_RANGE "Tangent"; remembers previous | always |
| `chamferType` | Chamfer type | ChamferType | EQUAL_OFFSETS "Equal distance" (default), TWO_OFFSETS "Two distances", OFFSET_ANGLE "Distance and angle", RAW_OFFSET (hidden); remembers previous | always (precondition guard `!= undefined` only for legacy) |
| `width` | Distance | length, tolerant, remembers previous | BLEND_BOUNDS 1e-5..500 m, default 5 mm / 0.2 in | type != TWO_OFFSETS |
| `width1` | Distance 1 | length, tolerant | BLEND_BOUNDS | TWO_OFFSETS |
| `oppositeDirection` | Opposite direction | boolean | false | OFFSET_ANGLE or TWO_OFFSETS |
| `width2` | Distance 2 | length, tolerant | BLEND_BOUNDS | TWO_OFFSETS |
| `angle` | Angle | angle, tolerant | CHAMFER_ANGLE_BOUNDS 0.1..179.9 deg, default 45 deg | OFFSET_ANGLE |
| `directionOverrides` | Direction overrides | Query | | OFFSET_ANGLE or TWO_OFFSETS |
| `tangentPropagation` | Tangent propagation | boolean | true (UI) | always |

- **Behavior / semantics:**
  - Offset method: distances measured along the adjacent faces from the original edge (offset-curve on face); on non-planar faces result depends on selection order (computation starts at the first selection and progresses along the chain).
  - Tangent method: distances measured from the intersection of the tangent planes of the two adjacent faces (apex), independent of order. Identical to Offset when both faces are planar.
  - Equal distance: symmetric setback `width` on both faces. Two distances: `width1` on the reference face, `width2` on the other; Distance and angle: `width` along the reference face and `angle` measured from the reference face. Reference face per edge chosen by the kernel; `oppositeDirection` flips all, `directionOverrides` flips individual edges (CHAMFER_DIRECTION_OVERRIDE_NO_EFFECT if not a subset). When the input is mirrored by a feature pattern, asymmetric chamfer direction is auto-compensated.
  - Tangent propagation extends along G1-connected edges. Faces as input chamfer all edges of the face.
  - Active sheet metal edges: only Offset + Equal distance allowed; they are converted to a sheet-metal corner break (chamfer style, range = width) with info SHEET_METAL_USE_CORNER_BREAK_INFO; other options -> SHEET_METAL_CHAMFER_OPTIONS_USE_CORNER_BREAK.
  - Tolerances on distances/angle are registered for MBD.
- **Edge cases & errors:** CHAMFER_SELECT_EDGES, CHAMFER_FAILED, CHAMFER_PARTIAL_FAIL, CHAMFER_FAIL_SMOOTH (edge between tangent faces), CHAMFER_FACE_RANGE_TOO_LARGE, CHAMFER_OVERLAP, CHAMFER_RANGE_INCONSISTENT_EDGE, CHAMFER_REQUIRES_SURFACE_EXTENSION, CHAMFER_PRODUCED_SELF_INT_SURFACE, CHAMFER_VERTEX_EDGES_COMPLICATED, CHAMFER_ADJOINING_EDGE_NOT_CHAMFERED, CHAMFER_BOUNDARY_*, CHAMFER_EDGE_OVERLAPPED_BY_CHAMFER, CHAMFER_OVERLAPS_EDGE_LOOP, CHAMFER_HELD_BACK (legacy versions).
- **OCCT mapping:** BRepFilletAPI_MakeChamfer: Add(dis, edge) symmetric; Add(d1, d2, edge, face) / AddDD for two distances with reference face selecting the side; AddDA(dis, angle, edge, face) for distance-angle; SetMode(ChFiDS_ClassicChamfer vs ChFiDS_ConstThroatChamfer / ChFiDS_ConstThroatWithPenetrationChamfer) — "Offset" ~ ClassicChamfer (distance along faces), "Tangent" (apex range) ~ custom or ChFiDS_ConstThroatChamfer approximations (verify). Direction override = choose the other adjacent face (TopExp::MapShapesAndAncestors edge->faces) as reference. Tangent propagation native in ChFi3d (contour follows G1 edges). Gap: exact "Tangent" measurement on curved faces may need custom computation of distances from the tangent-plane apex.
- **Priority:** P0

### PS-10 Draft
- **Purpose:** Tilt faces by a draft angle relative to a pull direction for moldability, using a neutral plane or a parting line.
- **Sources:** https://cad.onshape.com/help/Content/PartStudio/draft.htm ; FS: `draft` (draft.fs), `opDraft` (DraftType NEUTRAL_PLANE / REFERENCE_SURFACE / REFERENCE_ENTITY).
- **Selection inputs:**
  - Neutral plane type: "Neutral plane" `neutralPlane`: ALLOWS_PLANE (planar face, construction plane, implicit/explicit mate connector XY); max 1. "Entities to draft" `draftFaces`: FACEs (modifiable, non-sketch, non-construction); multi.
  - Parting line type: "Pull direction" `pullDirectionEntity`: ALLOWS_DIRECTION (planar face normal, linear edge, axis, mate connector Z); max 1. "Parting edges" `partingEdges`: EDGEs of solids or sheets (modifiable, non-sketch); multi; Create-selection helper shown. Hidden `hintFaces` (faces used to disambiguate sides).
- **Parameters:**

| FS id | UI label | Type | Enum values / default / bounds | Visible when / notes |
|---|---|---|---|---|
| `draftFeatureType` | Draft type | DraftFeatureType | NEUTRAL_PLANE "Neutral plane" (default), PARTING_LINE "Parting line" | always |
| `neutralPlane` | Neutral plane | Query max 1 | | NEUTRAL_PLANE |
| `draftFaces` | Entities to draft | Query | | NEUTRAL_PLANE |
| `pullDirectionEntity` | Pull direction | Query max 1 | | PARTING_LINE |
| `partingEdges` | Parting edges | Query | | PARTING_LINE |
| `hintFaces` | — | Query | hidden | PARTING_LINE |
| `partingLineSides` | Sides | PartingLineSides | ONE_SIDED "One sided" (default), SYMMETRIC "Symmetric", TWO_SIDED "Two sided" | PARTING_LINE |
| `alongPull` | Switch face | boolean | true | PARTING_LINE and ONE_SIDED |
| `angle` | Draft angle | angle, remembers previous | ANGLE_STRICT_90_BOUNDS 0..89.9 deg, default 3 deg | always |
| `pullDirection` | Opposite direction | boolean (circular arrow) | false | always |
| `secondAngle` | Second draft angle | angle, remembers previous | ANGLE_STRICT_90_BOUNDS | PARTING_LINE and TWO_SIDED |
| `secondPullDirection` | Opposite direction | boolean | true | PARTING_LINE and TWO_SIDED |
| `tangentPropagation` | Tangent propagation | boolean | true | NEUTRAL_PLANE |
| `referenceEdgePropagation` | Parting line propagation | boolean | true | PARTING_LINE |
| `reFillet` | Reapply fillet | boolean | false | always |

- **Behavior / semantics:**
  - Neutral plane: pull vector = -(neutral plane normal), negated again unless Opposite direction (so default pull = plane normal). Each draft face is rotated about its intersection curve with the neutral plane (reference surface) so that it makes `angle` with the pull direction; the intersection with the neutral plane stays fixed. Tangent propagation adds tangent-connected faces that are steeper than the draft angle; non-steep fillets are always reapplied. Reapply fillet: off - steep fillet faces are reblended as fillets (cylindrical result); on - they are drafted like other faces (conical).
  - Parting line: parting edges (typically created by Split > face split) divide faces; for each edge the adjacent faces are ordered by steepness relative to pull. One sided: drafts the face on one side (Switch face chooses the face more along/against pull); Symmetric: both sides with the same angle; Two sided: both sides with `angle` and `secondAngle` independently flippable. The parting edge stays fixed (reference entity draft). Parting line propagation extends along tangent-connected parting edges. Reapply fillet on generates cones and preserves the parting edges.
  - Angular manipulator on the neutral plane / parting edges edits the angle.
- **Edge cases & errors:** DRAFT_SELECT_NEUTRAL, DRAFT_SELECT_FACES, DRAFT_NO_NEUTRAL_PLANE, DRAFT_NO_DRAFT_FACE, DRAFT_FAILED, DRAFT_SELECT_PARTING_EDGES, DRAFT_SELECT_PULL_DIRECTION_ENTITY, DRAFT_CONFLICTING_OPPOSITION "Conflicting edges in parting line.", DRAFT_PARALLEL_PARTING_EDGE "Parting edge cannot be parallel to pull direction.". Faces already parallel to pull with angle 0 = no change.
- **OCCT mapping:** BRepOffsetAPI_DraftAngle: Add(face, pullDir gp_Dir, angle, neutralPlane gp_Pln, Flag) per face, then Build; handles neutral plane drafts of planar/cylindrical/conical faces with adjacent face re-intersection. Parting line (reference entity) draft: custom — BRepOffsetAPI_DraftAngle requires a neutral plane; approximate per-face by using a local plane through the parting edge (valid when the parting edge is planar and perpendicular to pull), otherwise build ruled draft surfaces from the parting curve (GeomFill / BRepFill draft: BRepOffsetAPI_MakeDraft creates a draft surface from a wire and direction) and rebuild faces with BRepAlgoAPI + defeaturing. Reapply fillet: remove fillets (BRepAlgoAPI_Defeaturing), draft, re-fillet (BRepFilletAPI_MakeFillet). Tangent propagation: custom face-set expansion (TopExp adjacency + BRepLProp G1 test + steepness check).
- **Priority:** P1

### PS-11 Body draft
- **Purpose:** Add tapered (drafted) material below/above held edges down to a parting entity for whole parts, faces, or edges; always adds material (handles complex geometry such as tori via isocline splitting).
- **Sources:** https://cad.onshape.com/help/Content/PartStudio/body_draft.htm ; FS: `bodyDraft` (bodyDraft.fs), `opBodyDraft` (geomOperations.fs). Mobile: view/edit only.
- **Selection inputs:**
  - Hidden "Preselection" `preselection`: solid BODY/FACE/EDGE (edit logic converts a preselection into the scope fields).
  - "Edges" `topEdges` (Edges to hold): EDGEs of modifiable non-sheet-metal solids.
  - "Faces" `faces` (Faces to draft): FACEs of modifiable solids.
  - "Parts" `bodies` (Parts to draft): solid BODYs; "Faces to exclude" `excludeFaces`: FACEs.
  - "Pull direction" `pullDirection`: ALLOWS_PLANE or ALLOWS_DIRECTION (face, plane, edge, mate connector); max 1. The parting entity is auto-filled with the pull direction selection when applicable.
  - "Parting entity" `partingObject`: non-sketch SHEET body, FACE (incl. plane) or mate connector; max 1.
  - "Bottom edges" `bottomEdges` (symmetric, edge scope): EDGEs.
- **Parameters:**

| FS id | UI label | Type | Enum values / default / bounds | Visible when / notes |
|---|---|---|---|---|
| `preselection` | Preselection | Query | hidden | — |
| `selectionType` | Draft scope | BodyDraftSelectionType | PARTS "Parts to draft", FACES "Faces to draft", EDGES "Edges to hold" (FS default EDGES); remembers previous | always |
| `topEdges` | Edges | Query | | EDGES |
| `faces` | Faces | Query | | FACES |
| `bodies` | Parts | Query | | PARTS |
| `excludeFaces` | Faces to exclude | Query | | PARTS |
| `draftOnSelf` | Draft on self | boolean | false | always |
| `pullDirection` | Pull direction | Query max 1 | | always |
| `flipPullDirection` | Flip pull direction | boolean | false | always |
| `partingObject` | Parting entity | Query max 1 | | !draftOnSelf |
| `angle` | Angle | angle | ANGLE_STRICT_90_BOUNDS 0..89.9 deg, default 3 deg | always |
| `bothSides` | Symmetric | boolean | false | always |
| `bottomEdges` | Bottom edges | Query | | bothSides and EDGES |
| `matchFacesAtParting` | Match faces at parting | boolean | true (UI) | bothSides and !draftOnSelf |
| `matchFaceType` | Match type | BodyDraftMatchFaceType | TANGENT_TO_FACE "Tangent to face" (default), REFERENCE_EDGE "Reference edge" | matchFacesAtParting (collapsed group) |
| `concaveRepair` | Concave repair | BodyDraftConcaveRepairType | NONE "None" (default), RADIUS "Radius", MIX "Mix" | matchFacesAtParting |
| `concaveRepairRadius` | Radius | length | BLEND_BOUNDS 1e-5..500 m, default 5 mm / 0.2 in | concaveRepair != NONE |
| `cornerType` | Corner type | BodyDraftCornerType | EXTEND "Extend" (default), PLANE "Plane" | always |
| `keepMaterial` | Keep material | boolean | false | always |

- **Behavior / semantics:** Parts/Faces scopes are shortcuts for holding all edges of the part/face; faces are split by isoclines (where the face slope equals the draft angle relative to pull) and new tapered faces are swept from the held edges/isoclines along the pull direction at the draft angle until they reach the parting entity (or the part itself with Draft on self), then attached to the body (material is only added). Symmetric drafts both sides of the parting entity; Match faces at parting adds extra material so upper and lower drafts meet at the parting line - Reference edge: draft up to the reference edge then fill to parting; Tangent to face: draft to where it is tangent to the part face. Concave repair resolves self-intersecting draft faces with a radius blend (Radius) or straight-plus-radius (Mix). Corner type: Extend - adjacent draft faces extended until they intersect; Plane - faces kept at their limit and a new planar face fills the corner. Keep material: by default material the draft passes through is removed; checked keeps protruding regions. Flip manipulator on pull direction.
- **Edge cases & errors:** BODY_DRAFT_SELECT_EDGES/FACES/PARTS, BODY_DRAFT_BAD_PULL_DIRECTION, BODY_DRAFT_INVALID_PARTING_ENTITY, BODY_DRAFT_PARTING_SURFACE_SMALL, BODY_DRAFT_TOO_STEEP, BODY_DRAFT_NO_EFFECT, BODY_DRAFT_MITER_NEED_BOTH_SIDES, BODY_DRAFT_MITER_FAILED, BODY_DRAFT_EDGE_SEPARATION_FAILED, BODY_DRAFT_EDGE_SPLIT_FAILED, BODY_DRAFT_ISOCLINE_SPLIT_FAILED, BODY_DRAFT_REFERENCE_LOOP, BODY_DRAFT_INVALID_*_REFERENCE, BODY_DRAFT_FAILED.
- **OCCT mapping:** No direct equivalent. Building blocks: isocline curves (custom: solve n·pull = sin(angle) on faces via Geom surface sampling + IntPatch/approx, or BRepAlgoAPI_Section with a draft cone approximation) and face splitting (BRepFeat_SplitShape / BRepAlgoAPI_Splitter); draft surfaces from edges: BRepOffsetAPI_MakeDraft(wire, dir, angle) with Perform(stopShape / length) to the parting surface; corners via BRepOffsetAPI_MakeDraft SetOptions (transition modes); attach with BRepAlgoAPI_Fuse; concave repair with BRepFilletAPI_MakeFillet. Gap: large custom effort.
- **Priority:** P2

### PS-12 Shell
- **Purpose:** Hollow a solid to a constant wall thickness, opening selected faces (shell) or with no openings (hollow).
- **Sources:** https://cad.onshape.com/help/Content/PartStudio/shell.htm ; FS: `shell` (shell.fs), `opShell`.
- **Selection inputs:** "Faces to remove" `entities` (FACEs of modifiable, non-sheet-metal solids, mesh allowed; multi; may span several parts - each owning part is shelled) or "Parts to hollow" `parts` (solid BODYs; multi).
- **Parameters:**

| FS id | UI label | Type | Enum values / default / bounds | Visible when / notes |
|---|---|---|---|---|
| `isHollow` | Hollow | boolean | false | always |
| `entities` | Faces to remove | Query | | !isHollow |
| `parts` | Parts to hollow | Query | | isHollow |
| `thickness` | Shell thickness | length | SHELL_OFFSET_BOUNDS 1e-5..500 m, default 2.5 mm / 0.1 in | always |
| `oppositeDirection` | Opposite direction | boolean | false | always |

- **Behavior / semantics:** Default: walls are created inward (the original outer faces stay; inner offset by -thickness). Opposite direction: the original faces become the inside and material is added outward (offset +thickness). Selected faces are removed (opened); all remaining faces offset uniformly with adjacent offsets re-intersected (fillets/cylinders offset to concentric surfaces). Hollow: no faces removed; creates an internal void (body with an inner shell) — visible in section view. Thickness direction convention: FS negates thickness unless oppositeDirection, passing a signed offset to opShell.
- **Edge cases & errors:** SHELL_SELECT_FACES "Select faces to remove."; SHELL_SELECT_PARTS "Select parts to hollow."; SHELL_FAILED "Could not shell part with selections." (thickness larger than minimum concave curvature radius, offset self-intersection, faces that vanish).
- **OCCT mapping:** Shell: BRepOffsetAPI_MakeThickSolid::MakeThickSolidByJoin(solid, facesToRemove, offset=-t (inward) or +t (outward), tol, BRepOffset_Skin, intersection=false, selfInter=false, GeomAbs_Arc/Intersection join). Hollow: BRepOffsetAPI_MakeOffsetShape::PerformByJoin(solid, -t) to get an inner shell, reverse it and add as an inner shell (BRepBuilderAPI_MakeSolid(outerShell) + Add(innerShell reversed)), or BRepOffsetAPI_MakeThickSolid with empty face list (supported in recent OCCT: produces a closed thick solid). Outward hollow: offset +t outer shell, original becomes inner. Validate with BRepCheck_Analyzer.
- **Priority:** P0

### PS-13 Hole
- **Purpose:** Cut standard or custom simple / counterbore / countersink holes (drilled, clearance, tapped, pipe-tap, PEM) at sketch points or mate connectors into parts in a merge scope; record hole attributes for callouts, hole tables and cosmetic threads.
- **Sources:** https://cad.onshape.com/help/Content/PartStudio/hole.htm ; https://cad.onshape.com/help/Content/PartStudio/hole_table.htm (drawings); FS: `hole` (hole.fs, defineSheetMetalFeature - works on active sheet metal), holeUtils.fs, holeAttribute.fs, holetables.gen.fs, cosmeticThreadUtils.fs.
- **Selection inputs:**
  - "Sketch points to place holes" `locations`: sketch VERTEXes (points, line ends, spline points, circle/arc centers; modifiable) or mate connectors (implicit/explicit); box select; selecting a sketch in the feature list adds all its non-construction points. Max 500 locations (HOLE_EXCEEDS_MAX_LOCATIONS_500; legacy 100).
  - "Merge scope" `scope`: modifiable solid BODYs (mesh allowed) to cut (auto-filled when only one part).
  - "Hole start plane or mate connector" `startBoundEntity`: planar FACE or mate connector; max 1 (Start from selected plane). All hole axes must be perpendicular to it.
  - "Up to entity or mate connector" `endBoundEntity`: FACE (non-sketch, mesh ok) or ALLOWS_VERTEX; max 1.
  - Hidden `initEntities` (preselection).
- **Parameters (current V2/V3 hole; legacy V1 fields noted):**

| FS id | UI label | Type | Enum values / default / bounds | Visible when / notes |
|---|---|---|---|---|
| `isV2` | Feature version | boolean | true; hidden | — |
| `holeVersion` | Hole version | HoleVersion | LEGACY, V3 (default V3); hidden | V3 uses CAN_BE_TOLERANT fields instead of explicit tolerance sub-fields |
| `initEntities` | Initial Entities | Query | hidden | — |
| `featureName` | Feature Name Template | string | hidden; feature name auto-generated from settings (e.g. size + type), user-renamable; clearing the name restores auto name | — |
| `threadStandard` | Thread standard | ThreadStandard | UNSET/ANSI/ISO; hidden, set from lookup | — |
| `hasClearance` | Has clearance | boolean | false; hidden (set when tapped + fastener fit, i.e. clearance in upper parts) | — |
| `unitsSystem` | Units | UnitsSystem | INCH "Inch", METRIC "Metric"; horizontal, remembers previous, unconfigurable | isV2 |
| `styleV2` / `style` | Style | HoleStyle (c-bore/c-sink field visibility tests `style`; edit logic keeps `style` in sync with `styleV2`, inferred) | SIMPLE "Simple", C_BORE "Counterbore", C_SINK "Countersink"; remembers previous | always |
| `locations` | Sketch points to place holes | Query | | always |
| `scope` | Merge scope | Query | | always |
| `ansiHoleTableEx` / `ansiHoleTable` / `isoHoleTableEx` / `isoHoleTable` | Standard | LookupTablePath (cascading dropdowns) | Ex tables (with PEM) for Simple; non-Ex for C-bore/C-sink; see hole tables below; remembers previous | per unitsSystem/style |
| `showThreadClassV2` | Thread class | boolean | remembers previous | threadStandard != UNSET (tapped) |
| `ansiThreadClassV2` / `isoThreadClassV2` | Thread class | LookupTablePath | ANSI 1B/2B/3B (default 2B); ISO 4H, 5G, 5H, 6G, 6H, 7G, 7H, 8G, 8H (default 6H) | showThreadClassV2 |
| `showTappedDepth` | Tapped details | boolean | hidden; true for tapped/pipe holes (set by table) | — |
| `holeDiameterV3` (`holeDiameterV2`, `holeDiameter` legacy) | Diameter | length, SHOW_EXPRESSION, tolerant (diameter) | HOLE_DIAMETER_BOUNDS 1e-5..500 m, default 5 mm / 0.25 in; filled from table, editable | always |
| `tapDrillDiameterV3` | Tap drill diameter | length, tolerant | HOLE_DIAMETER_BOUNDS | hasClearance |
| `startStyle` | Start plane | HoleStartStyle | PART "Start from part" (default), SKETCH "Start from sketch plane", PLANE "Start from selected plane"; remembers previous | always |
| `startBoundEntity` | Hole start plane or mate connector | Query max 1 | | startStyle PLANE |
| `endStyleV2` (`endStyle` legacy) | Termination | HoleEndStyleV2 | BLIND "Blind" (default), UP_TO_NEXT "Up to next", UP_TO_ENTITY "Up to entity", THROUGH "Through all" (legacy HoleEndStyle adds BLIND_IN_LAST "Blind in last", deprecated); remembers previous | always |
| `oppositeDirection` | Opposite direction | boolean | false | always |
| `endBoundEntity` | Up to entity or mate connector | Query max 1 | | UP_TO_ENTITY |
| `offset` | Offset from tip | boolean | false | UP_TO_NEXT / UP_TO_ENTITY |
| `offsetDistance` | Offset from tip | length | ZERO_INCLUSIVE_OFFSET_BOUNDS 0..500 m, default 5 mm / 0.25 in | offset |
| `oppositeOffsetDirection` | Opposite direction | boolean | false | offset |
| `cBoreDiameterV3` | Counterbore diameter | length, tolerant | HOLE_BORE_DIAMETER_BOUNDS 1e-5..500 m, default 10 mm / 0.5 in | C_BORE |
| `cBoreDepthV3` | Counterbore depth | length, tolerant | HOLE_BORE_DEPTH_BOUNDS 0..500 m, default 5 mm / 0.25 in | C_BORE |
| `cSinkDiameterV3` | Countersink diameter | length, tolerant | HOLE_BORE_DIAMETER_BOUNDS | C_SINK |
| `cSinkAngleV3` | Countersink angle | angle, tolerant | CSINK_ANGLE_BOUNDS 0.1..179.9 deg, default 90 deg | C_SINK |
| `tapDrillDiameter` | Tap drill diameter | length | legacy BLIND_IN_LAST only | legacy |
| `majorDiameter` | Tap major diameter | length | HOLE_MAJOR_DIAMETER_BOUNDS; always hidden (from table) | — |
| `isMultiple` | Multiple | boolean | hidden (several differing depths) | — |
| `holeDepthV3` | Depth | length, tolerant, remembers previous | HOLE_DEPTH_BOUNDS 1e-5..500 m, default 12 mm / 0.5 in (includes c-bore/c-sink depth) | termination BLIND |
| `holeDepthComputedV3` | Depth | length, read-only, tolerant | computed | UP_TO_NEXT / UP_TO_ENTITY or multiple |
| `holeDepthMultiple` | Depth | string read-only "Multiple" | legacy | isMultiple && !V3 |
| `tipAngleStyle` | Tip angle style | TipAngleStyle | DEGREE118 "118 deg" (default), DEGREE135 "135 deg", FLAT "Flat", CUSTOM "Custom" | termination != THROUGH |
| `tipAngleV3` | Tip angle | angle, tolerant | TIP_ANGLE_BOUNDS 0.1..180 deg, default 118 | CUSTOM |
| `isTappedThrough` | Tap through all | boolean | true, remembers previous | showTappedDepth and THROUGH |
| `tappedDepthV3` | Tapped depth | length, tolerant | HOLE_DEPTH_BOUNDS | showTappedDepth and (not THROUGH or !isTappedThrough) |
| `tappedAngle` | Tapped angle | angle | HOLE_TAPPED_ANGLE_BOUNDS 0..9.9 deg (tapered pipe taper); hidden | tapered pipe |
| `tapClearance` | Tap clearance (number of thread pitch lengths) | real | HOLE_CLEARANCE_BOUNDS 0..500, default 3 | showTappedDepth and BLIND |
| `toleranceMatching` | Tolerance matching | any | hidden | — |
| legacy: `standardTappedOrClearance`, `standardBlindInLast`, `showThreadClass`, `ansiThreadClass`, `isoThreadClass`, `cBoreDiameter`, `cBoreDepth`, `cSinkDiameter`, `cSinkAngle`, `holeDepth`, `holeDepthComputed`, `tipAngle`, `tappedDepth` + explicit tolerance fields (defineLengthTolerance/defineAngleTolerance) | | | | V1 / LEGACY holes only |

- **Hole tables summary (holetables.gen.fs, lookup path = cascading dropdowns):**
  - Inch (ANSI): Hole type = Drilled (Drill size: 170 entries #80 (0.0135) .. 1 (1.0), letter/number/fraction drills) | Clearance (Size #0 .. 6, 67 sizes; Fastener fit Close, Free, Close (ASME), Normal (ASME), Loose (ASME); provides hole, c-bore diameter/depth, c-sink diameter/angle) | Tapped: Tap type Straight tap (Size #0 .. 6, 67 sizes; Threads/inch e.g. 1/4: 20 (UNC), 28 (UNF), 32 (UNEF), 24, 27, 36, 40, 48, 56 tpi; Fastener fit None/Close/Free/Close (ASME)/Normal (ASME)/Loose (ASME); provides tap drill, major diameter) or Tapered Pipe Tap (NPT sizes 1/16 .. 24, 24 sizes; tpi; provides hole depth/tapped depth) | PEM (Simple only; Fastener: Self-clinching nuts / standoffs / studs & pins, each with fastener types (S/SS/CLS/CLSS/SP, CLA, S-RT, SL, H/HNL, SH, SMPS/SMPP; SO/SOS/SOA/SO4 threaded/unthreaded, BSO, TSO, DSO, SOSG; FH/FHS/FHA, FH4/FHP, FHL, TFH, HFH, HFE, HFG8, HFLH, SGPC, pins, TPS/TP4), sizes #2 .. 1/2, fit = Clearance).
  - Metric (ISO): Drilled (157 sizes 0.05 .. 38 mm) | Clearance (M1 .. M68, 42 sizes; fit Close, Normal, Loose) | Tapped: Straight tap (M1 .. M68; Pitch coarse/fine e.g. M1: 0.20 (Fine), 0.25 (Coarse); fit None/Close/Normal/Loose), Straight Pipe Tap (Thread type G (BSPP) or RP; sizes 1/16 .. 6, 24 sizes), Tapered Pipe Tap (R/BSPT 1/16 .. 6, 15 sizes) | PEM metric (M2 .. M12 etc.; adds FHX, HF109, TPXS types).
  - Legacy (V1) tables: Standard Custom / ANSI / ISO / DIN / PEM with Clearance / Tapped (% diametric engagement 50%/75%) / Drilled / pipe taps; Blind-in-last tables (Clearance & tapped / Drilled).
  - External thread tables (used by External thread feature): ANSI #0 .. 6 (71 sizes, tpi; major/minor diameters), ISO M1 .. M300 (106 sizes, pitches).
- **Behavior / semantics:**
  - Axis per location: through the point along the owner sketch plane normal (mate connector: its Z axis), pointing into the material: default direction = -normal; Opposite direction flips. Pattern transforms applied.
  - Start: Start from part - for each location, cylindrical rays of the hole's outer profile diameter are cast from far in front/behind the targets along the axis; the furthest contact point of each cylinder is found and the closest of these becomes the hole start (so the full diameter starts at the first part surface encountered in the hole direction; handles curved/irregular surfaces); material between the sketch point and the start is cleared. Start from sketch plane - hole starts exactly at the sketch point (allows overlapping holes; c-bore/c-sink may be partially in air). Start from selected plane - start at the intersection of the axis with the chosen plane/MC.
  - Profile: a revolved tool profile built from (c-bore diameter/depth or c-sink diameter/angle), shaft diameter, depth and tip (118, 135, flat, custom cone angle); Depth is measured from the start to the end of the full-diameter shaft (tip extends beyond). For tapped holes the shaft diameter is the tap drill diameter, major diameter recorded; for clearance+tapped across 2+ parts (fastener fit != None) all but the last part get clearance holes (and c-bore/c-sink) and the last part gets the tapped hole (replaces legacy Blind in last).
  - Termination: Blind (depth), Up to next (first face encountered after start; computed depth shown read-only; offset from tip), Up to entity (full diameter reaches the entity; non-flat tip extends past it; offset from tip), Through all (through all parts in merge scope).
  - Tool bodies for all locations are subtracted from the scope in one boolean (CUT_ALL); attributes (HoleAttribute: style, diameters, depths, tapped depth, thread pitch, class, standard path, isTappedThrough) are attached to hole faces for drawings' hole callouts, hole tables and standard-content autosize; tapped/pipe holes get a cosmetic thread attribute (coordinate system, thread depth, pitch) rendered in the Part Studio and annotated in drawings. Tapped depth larger than the computed depth is clamped (info HOLE_TAP_TOO_DEEP). Holes on active sheet metal also set attributes on the flat pattern.
  - If a hole collides/overlaps or does not lie completely on the target, it is drilled at zero depth relative to the sketch plane (help).
  - If a standard does not provide c-bore/c-sink/tap diameters, out-of-range values reset.
- **Edge cases & errors:** HOLE_NO_POINTS, HOLE_EMPTY_SCOPE, HOLE_NO_HITS, HOLE_DISJOINT, HOLE_CBORE_TOO_SMALL, HOLE_CBORE_TOO_DEEP, HOLE_CSINK_TOO_SMALL, HOLE_CSINK_TOO_DEEP, HOLE_TAP_DIA_TOO_LARGE(_OR_EQUAL), HOLE_DESTROY_SOLID, HOLE_CUT_FAIL, HOLE_INCONSISTENT_TAP_INFO, HOLE_PARTIAL_FAILURE, HOLE_NO_END_BOUNDS, HOLE_START_BOUND_INVALID, HOLE_NO_START_BOUND, HOLE_CANNOT_DETERMINE_TAPPED_BODY, HOLE_FASTENER_FIT_IS_NOT_APPLICABLE, HOLE_CBORE_CSINK_VALUES_NON_STD, HOLE_CSINK_ANGLE_TOO_WIDE/NARROW, HOLE_REVERSED_BOUNDS, HOLE_PARAMS_OVERRIDDEN_INFO, HOLE_EXCEEDS_MAX_LOCATIONS_500.
- **OCCT mapping:** Tool solid per location: build the 2D half-profile (gp/GC_MakeSegment wires) and BRepPrimAPI_MakeRevol 2*PI about the hole axis (or compose BRepPrimAPI_MakeCylinder + BRepPrimAPI_MakeCone); start-from-part ray casting: IntCurvesFace_ShapeIntersector / BRepIntCurveSurface_Inter with lines on the profile circle (sample N rays around the circumference) or BRepExtrema_DistShapeShape with a cylinder; up-to-next/entity: BRepAlgoAPI_Section of axis with faces (IntCurvesFace_ShapeIntersector). Fuse all tools (BRepAlgoAPI_BuilderAlgo) and BRepAlgoAPI_Cut from scope. Cosmetic threads/hole attributes: app-level metadata on faces (persisted in our data model; export via XCAF: XCAFDoc_DimTolTool / STEP AP242 PMI hole callouts where possible). Hole tables: embed as static data (port holetables.gen.fs to JSON). No OCCT equivalent for standards.
- **Priority:** P0 (simple/c-bore/c-sink, blind/through, sketch points), P1 (standards tables, tapped/clearance/fastener fit, up-to, cosmetic threads), P2 (PEM, pipe taps)

### PS-14 Rib
- **Purpose:** Create thin support ribs inside/onto parts from open (or closed, normal-direction) sketch curves, extruded to the part boundary and optionally drafted and merged.
- **Sources:** https://cad.onshape.com/help/Content/PartStudio/rib.htm ; FS: `rib` (rib.fs).
- **Selection inputs:** "Sketch profiles" `profiles`: sketch EDGEs (non-construction); each edge/connected profile -> one rib. "Parts" `parts`: modifiable solid BODYs (mesh allowed) that receive the ribs and bound them.
- **Parameters:**

| FS id | UI label | Type | Enum values / default / bounds | Visible when / notes |
|---|---|---|---|---|
| `profiles` | Sketch profiles | Query | | always |
| `parts` | Parts | Query | | always |
| `thickness` | Thickness | length | SHELL_OFFSET_BOUNDS 1e-5..500 m, default 2.5 mm / 0.1 in | always |
| `ribExtrusionDirection` | Rib extrusion direction | RibExtrusionDirection | PARALLEL_TO_SKETCH_PLANE "Parallel to sketch plane" (default), NORMAL_TO_SKETCH_PLANE "Normal to sketch plane" | always |
| `oppositeDirection` | Opposite direction | boolean | true | always |
| `hasDraft` | Draft | boolean | false | always |
| `draftAngle` | Draft angle | angle | ANGLE_STRICT_90_BOUNDS 0..89.9 deg, default 3 deg | hasDraft |
| `draftOpposite` | Opposite direction | boolean | false | hasDraft |
| `extendProfilesUpToPart` | Extend profiles to part | boolean | false | always |
| `mergeRibs` | Merge ribs | boolean | true | always |

- **Behavior / semantics:** For each profile: (1) optionally extend its ends until they hit the part (lines extended linearly, arcs extended by tangent straight lines); (2) extrude the curve into a sheet in the rib direction (parallel to sketch plane = perpendicular to the curve within the sketch plane, side chosen by Opposite direction; normal to sketch plane = along the sketch normal) for a length exceeding the parts' bounding box; (3) thicken the sheet symmetrically by thickness/2 each side; (4) intersect/trim against the parts: pieces of the rib body outside the part material region (pieces not touching the profile) are discarded so the rib fills only the space bounded by the part; (5) draft: the two rib side (cap) faces are drafted about the rib's top (the profile side) with pull = rib direction (flip with draftOpposite), then re-trimmed; (6) Merge ribs: union ribs with the selected parts (grouped), info if some ribs did not merge; small notch faces between adjacent profile ribs are cleaned up. Unchecked: ribs remain separate new parts. Closed profiles are only allowed for normal-to-sketch-plane direction. Edit logic auto-picks direction/parts (inferred). The rib must be fully bounded by the part, otherwise fails.
- **Edge cases & errors:** RIB_NO_PROFILES, RIB_NO_PARTS, RIB_PROFILE_FAILED, RIB_BODY_FAILED "Selected profile did not produce a rib body. Make sure the rib direction and alignment are correct.", RIB_NO_INTERSECTIONS "None of the ribs intersected a part." (individual non-intersecting ribs are deleted and highlighted), RIB_MERGE_FAILED, RIB_ONLY_OPEN_PROFILES.
- **OCCT mapping:** BRepPrimAPI_MakePrism of the (extended) edge into a face, BRepOffset_MakeSimpleOffset / thicken two-sided (or directly build the rib as a prism of an offset 2D region: BRepOffsetAPI_MakeOffset of the wire with thickness/2 to a closed region, then prism), BRepAlgoAPI_Common with the part's "inside" — better: BRepAlgoAPI_Splitter of rib by part faces, keep cells touching the profile and not inside the part material (classify with BRepClass3d_SolidClassifier) — then BRepOffsetAPI_DraftAngle on side faces, BRepAlgoAPI_Fuse with parts, ShapeUpgrade_UnifySameDomain. Profile extension: GeomLib::ExtendCurveToPoint / straight-line extension.
- **Priority:** P2

### PS-15 Gusset
- **Purpose:** Create triangular or rectangular gusset plates between adjacent frame segments (weldment/frame tool).
- **Sources:** https://cad.onshape.com/help/Content/PartStudio/gusset.htm ; FS: `gusset` (gusset.fs).
- **Note:** Although listed in the Part Studio toolbar, Gusset is designed for frames ("Create gussets between adjacent frame segments"; not for round tube profiles or segments with corner type None). Full specification belongs to the Frames section; parameters summarized here for completeness.
- **Selection inputs:** "Edges" `edges` (linear EDGEs of solids on the frame segment cut faces that define gusset bases); "Alignment entity" `alignedReference` (linear solid EDGE, max 1). Hidden `baseSweptFaces`.
- **Parameters:**

| FS id | UI label | Type | Enum values / default / bounds | Visible when |
|---|---|---|---|---|
| `edges` | Edges | Query | | always |
| `gussetType` | Gusset type | GussetStyleType | TRIANGLE "Triangle", RECTANGLE "Rectangle"; remembers previous | always |
| `length` | Length | length | NONNEGATIVE_LENGTH_BOUNDS 1e-5..500 m, 25 mm / 1 in | always |
| `shouldFlipBaseSides` | Flip base sides | boolean | false | RECTANGLE |
| `thickness` | Thickness | length | NONNEGATIVE_LENGTH_BOUNDS | always |
| `chamfer` | Chamfer | boolean | | RECTANGLE |
| `chamferSize` | Chamfer size | length | CHAMFER_SIZE_BOUNDS 1e-5..500 m, default 1 mm | RECTANGLE and chamfer |
| `gussetPosition` | Gusset position | GussetPosition | CENTERED "Centered", ALIGNED "Aligned" | always |
| `alignedReference` | Alignment entity | Query max 1 | | ALIGNED |
| `shouldFlipAlignment` | Opposite alignment | boolean | | ALIGNED |
| `offset` | Offset distance | length | NONNEGATIVE_ZERO_DEFAULT 0..500 m | always |
| `shouldFlipOffset` | Opposite direction | boolean | | always |
| `baseSweptFaces` | Base faces | Query | hidden | — |

- **Behavior / semantics:** Length measured along the frame edge from the corner, thickness across the frame width; centered on the segment midplane or aligned to an edge plus offset; each gusset becomes an individual part (listed in the cut list). Manipulators for length/offset.
- **Edge cases & errors:** GUSSET_EMPTY_ALIGNMENT_SELECTION, GUSSET_ALIGNMENT_NO_INTERSECTION, GUSSET_OFFSET_NOT_PARALLEL, GUSSET_ALIGNED_OFFSET_NOT_PARALLEL, CHAMFER_SIZE_EXCEED_GUSSET_SIZE.
- **OCCT mapping:** Build planar triangle/rectangle (chamfered) face in the plane bisecting the two segments' faces, BRepPrimAPI_MakePrism by thickness; no boolean with frames (separate part).
- **Priority:** P2 (with Frames)

### PS-16 Wrap
- **Purpose:** Wrap planar faces / sketch regions onto a cylindrical or conical face (length-preserving development), producing surfaces, thickened solids (emboss/deboss) or face splits (imprint).
- **Sources:** https://cad.onshape.com/help/Content/Sketch/wrap.htm (help page lives under Sketch; feature is in the Part Studio toolbar); FS: `wrap` (wrap.fs), `opWrap` (WrapType SIMPLE / TRIM / IMPRINT), wrapSurface.fs.
- **Selection inputs:** "Tools" `source`: planar FACEs / sketch regions (non-construction), all on one plane; multi. "Target" `destination`: one cylindrical or conical FACE. "Tools anchor point" `sourceAnchor` / "Target anchor point" `destinationAnchor`: ALLOWS_VERTEX (vertex, sketch point, mate connector); max 1 each.
- **Parameters:**

| FS id | UI label | Type | Enum values / default / bounds | Visible when / notes |
|---|---|---|---|---|
| `resultType` | Creation type | WrapResultType | SOLID "Solid", SURFACE "Surface" (default), IMPRINT "Split" | always |
| `operationType` | Result body operation type | NewBodyOperationType | NEW | SOLID |
| `source` | Tools | Query | | always |
| `destination` | Target | Query max 1 | | always |
| `flipAlignment` | Flip alignment | boolean | false | always |
| `trim` | Trim to target | boolean | false | resultType != IMPRINT |
| `customAnchors` | Specify anchor points | boolean | false | always |
| `sourceAnchor` / `destinationAnchor` | Tools anchor point / Target anchor point | Query | | customAnchors |
| `angle` | Angle | angle | ANGLE_360_ZERO_DEFAULT_BOUNDS, default 0 | group "Position" (collapsed) |
| `angleOppositeDirection` | Angle opposite direction | boolean | false | Position |
| `uShift` | U shift | length | ZERO_DEFAULT_LENGTH_BOUNDS -500..500 m, default 0 | Position |
| `uShiftOppositeDirection` | U shift opposite direction | boolean | false | Position |
| `vShift` | V shift | length | ZERO_DEFAULT_LENGTH_BOUNDS | Position |
| `vShiftOppositeDirection` | V shift opposite direction | boolean | false | Position |
| `thickness` | Thickness | length | SHELL_OFFSET_BOUNDS 1e-5..500 m, default 2.5 mm / 0.1 in | SOLID |
| `oppositeDirection` | Thickness opposite direction | boolean | false | SOLID |
| `defaultScope`/`booleanScope` | Merge with all / Merge scope | | PS-01.2 | SOLID, op != NEW |

- **Behavior / semantics:** A source plane frame and a destination developable surface frame are matched at anchor points (automatic: source anchor at the bounding-box center or at the intersection of the projected target axis with the source bbox when the plane is parallel to the axis; destination anchor = projection onto the target; or user anchors, with mate-connector X axes used as direction hints). The source geometry is mapped isometrically (unrolled) onto the cylinder/cone: Angle rotates the tools in the source plane about the anchor, U shift moves along the circumferential direction, V shift along the axis direction; Flip alignment mirrors orientation. Surface: wrapped faces become new sheet bodies on the target surface; Trim to target trims them by the target face boundaries. Split (imprint): wrapped edges split the target face (not allowed on active sheet metal). Solid: wrapped surfaces are thickened by `thickness` (outward, or inward with Thickness opposite direction) and booleaned per PS-01.2. Manipulators for angle and U/V shifts.
- **Edge cases & errors:** WRAP_SELECT_TOOLS, WRAP_SELECT_TARGET, WRAP_SOURCE_DIFFERING_PLANES, WRAP_DESTINATION_NO_FACE, WRAP_TRIM_FAILED, WRAP_TRIM_NO_INTERSECTION, WRAP_IMPRINT_FAILED, WRAP_IMPRINT_SHEET_METAL, WRAP_NEEDS_ANCHOR, WRAP_SELECT_ANCHORS, WRAP_NEEDS_DIFFERENT_ANCHOR.
- **OCCT mapping:** No direct API. Custom mapping: transform each tool face's edges (2D in source plane coords) to (u,v) parameters of the target Geom_CylindricalSurface (u = s/R, v = t) or Geom_ConicalSurface (polar development: radius-from-apex/angle mapping), build pcurves (Geom2d curves, approximate via Geom2dAPI_Interpolate/ Approx for non-linear cone mapping) and make faces with BRepBuilderAPI_MakeFace(surface, wire-of-edges-on-surface) + BRepLib::BuildCurves3d. Trim: BRepAlgoAPI_Common with target face. Imprint: BRepFeat_SplitShape / BRepAlgoAPI_Splitter with the wrapped edges. Solid: thicken (BRepOffset_MakeSimpleOffset) + boolean. OCCT has BRepProj_Projection / BRepOffsetAPI_NormalProjection but they are projections, not length-preserving wraps.
- **Priority:** P2

### PS-17 Boolean
- **Purpose:** Union, subtract or intersect existing parts (and union surfaces), with optional tool retention and clearance offset for subtract.
- **Sources:** https://cad.onshape.com/help/Content/PartStudio/boolean.htm ; FS: `booleanBodies` (boolean.fs), `opBoolean`, `opOffsetFace`.
- **Selection inputs:** "Tools" `tools`: BODYs - SOLID, or non-construction non-sketch SHEET (mesh allowed); multi; order matters (UIHint ALLOW_QUERY_ORDER: the first body's identity/name/appearance survives a union). "Targets" `targets` (Subtract only): modifiable SOLID or SHEET bodies. "Faces to offset" `entitiesToOffset`: FACEs of solid tool bodies.
- **Parameters:**

| FS id | UI label | Type | Enum values / default / bounds | Visible when / notes |
|---|---|---|---|---|
| `operationType` | Operation type | BooleanOperationType | UNION "Union", SUBTRACTION "Subtract", INTERSECTION "Intersect", SUBTRACT_COMPLEMENT (hidden; used internally for feature Intersect) | always |
| `tools` | Tools | Query | | always |
| `targets` | Targets | Query | | SUBTRACTION |
| `offset` | Offset | boolean | false | SUBTRACTION |
| `offsetAll` | Offset all | boolean | false | offset |
| `entitiesToOffset` | Faces to offset | Query | | offset and !offsetAll |
| `offsetDistance` | Offset distance | length | ZERO_INCLUSIVE_OFFSET_BOUNDS 0..500 m, default 5 mm / 0.25 in | offset |
| `oppositeDirection` | Opposite direction | boolean | false | offset |
| `reFillet` | Reapply fillet | boolean | false | offset |
| `keepTools` | Keep tools | boolean | false; remembers previous | always |

- **Behavior / semantics:**
  - Union: all tools must be the same type. Solids that intersect or abut (face contact) merge; surfaces merge along coincident/overlapping laminar edges and, if they close a volume, become a solid (makeSolid=true); imprinted edges erased. Mixing solids and sheets -> BOOLEAN_CANNOT_MIX_SOLIDS_AND_SURFACES; sheets as tools allowed only for Union (BOOLEAN_TOOL_INPUTS_NOT_SOLID). Surviving body keeps attributes (name, appearance, material) of the earliest-selected tool.
  - Subtract: the union of all tools is removed from every target (targets may be solids or surfaces; tools must be solids). Keep tools keeps the tool parts (useful for fitted parts). With Offset: a temporary copy of the tools is made, the chosen faces (or all faces with Offset all) are offset outward by offsetDistance (Opposite direction = inward; Reapply fillet re-fillets after offset via opOffsetFace/move-face semantics), the offset copy is subtracted, then deleted; original tools deleted unless Keep tools. Zero offset = plain subtract.
  - Intersect: result = common volume of all tools (all must be solids); tools consumed (unless keep tools).
  - Active sheet metal targets use a sheet-metal-aware subtract that cuts the sheet faces and keeps the flat pattern consistent.
  - No-ops are reported as info (BOOLEAN_UNION_NO_OP, BOOLEAN_SUBTRACT_NO_OP, BOOLEAN_INTERSECT_NO_OP) — help: "Parts must intersect for Boolean to work. It will not work with two self-contained parts."
- **Edge cases & errors:** BOOLEAN_BAD_INPUT "Need at least two parts or surfaces for a Boolean operation.", BOOLEAN_SAME_INPUT, BOOLEAN_INVALID, BOOLEAN_INTERSECT_FAIL, BOOLEAN_INPUTS_NOT_SOLID, BOOLEAN_OFFSET_NO_FACES, BOOLEAN_NON_MANIFOLD_RESULT, BOOLEAN_TOOL_INPUTS_NOT_SOLID, BOOLEAN_CANNOT_MIX_SOLIDS_AND_SURFACES, DIRECT_EDIT_NO_OFFSET (legacy zero offset).
- **OCCT mapping:** BRepAlgoAPI_Fuse (SetArguments/SetTools, then ShapeUpgrade_UnifySameDomain) — group result into connected solids (each connected component = one part); surface union: BRepBuilderAPI_Sewing then BRepBuilderAPI_MakeSolid + ShapeFix_Solid when closed; BRepAlgoAPI_Cut (arguments = each target, tools = all tools); BRepAlgoAPI_Common (first tool vs rest, iteratively or BOPAlgo_BOP with Operation COMMON); subtract complement = Common(target, fuse(tools)). Offset: BRepOffsetAPI_MakeOffsetShape (PerformByJoin) on a copy for offset all; selected faces only -> custom (BRepOffset_MakeOffset with per-face SetOffsetOnFace) or BRepAlgoAPI_Defeaturing-style rebuild. Sheet-metal-aware variant: custom. Identity: BRepAlgoAPI history for naming; attribute inheritance from first tool.
- **Priority:** P0

### PS-18 Split
- **Purpose:** Split parts, surfaces or curves into multiple bodies with a plane, mate connector, surface or face (Split part); or split faces into multiple faces on the same body using planes, faces, surfaces, curves or sketch edges (Split face).
- **Sources:** https://cad.onshape.com/help/Content/PartStudio/split.htm ; FS: `splitPart` (splitpart.fs), `opSplitPart`, `opSplitFace`.
- **Selection inputs:**
  - Part: "Parts, surfaces, or curves to split" `targets`: modifiable SOLID / SHEET / WIRE bodies, non-sketch, mesh ok; multi. "Entity to split with" `tool`: non-sketch SHEET body, FACE (part face, surface face or construction plane) or mate connector (implicit/explicit); max 1.
  - Face: "Faces to split" `faceTargets`: modifiable non-sketch non-construction FACEs; multi. "Entities to split with" `faceTools`: sketch EDGEs, SHEET or WIRE bodies (may be non-modifiable), FACEs (incl. construction planes), mate connectors; multi. "Direction" `directionQuery`: ALLOWS_DIRECTION, max 1.
- **Parameters:**

| FS id | UI label | Type | Enum values / default / bounds | Visible when / notes |
|---|---|---|---|---|
| `splitType` | Split type | SplitType | PART "Part" (default), FACE "Face" | always |
| `targets` | Parts, surfaces, or curves to split | Query | | PART |
| `tool` | Entity to split with | Query max 1 | | PART |
| `keepTools` | Keep tools | boolean | false | PART |
| `useTrimmed` | Trim to face boundaries | boolean | false | PART |
| `keepBothSides` | Keep both sides | boolean | true | PART |
| `keepFront` | Opposite direction | boolean | true | PART and !keepBothSides |
| `faceTargets` | Faces to split | Query | | FACE |
| `faceTools` | Entities to split with | Query | | FACE |
| `projectionType` | Projection direction type | ProjectionType | DIRECTION "Direction" (default), NORMAL_TO_TARGET "Normal to target" | FACE (group "Edge projection options") |
| `useSketchPlaneDirection` | Use sketch plane direction | boolean | true | FACE and DIRECTION |
| `directionQuery` | Direction | Query max 1 | | FACE, DIRECTION, !useSketchPlaneDirection |
| `keepToolSurfaces` | Keep tool surfaces and curves | boolean | true | FACE |

- **Behavior / semantics:**
  - Split part: each target is cut by the tool (planes and mate connector XY planes are infinite; a single face is extended to its underlying surface unless Trim to face boundaries; a full multi-face surface body is used as-is). All resulting pieces become separate bodies (solids, sheets, or wires); one piece keeps the original identity/name and the others get new names (inferred). Keep both sides off: only the pieces on one side of the tool are kept — side chosen by the tool normal (front) or back with Opposite direction; flip manipulator arrow shown. Keep tools off deletes a surface tool body; faces of parts, construction planes and mate connectors are always kept (infos SPLIT_KEEP_TOOLS_WITH_FACE, SPLIT_KEEP_PLANES_AND_MATE_CONNECTORS). Trim with a multi-face tool requires a single face (SPLIT_TRIM_WITH_SINGLE_FACE info). Using a face of another part as tool = "split by part".
  - Split face: faces are imprinted by the intersection with face/surface/plane tools; edge/curve tools are projected onto the target faces either along a direction (sketch plane normal by default, or picked direction) or normal to the target; result = more faces on the same body (topology only, no new bodies). Keep tool surfaces and curves off deletes surface/curve tool bodies (planes/MCs kept). Typical use: create parting lines for Draft.
- **Edge cases & errors:** SPLIT_SELECT_TARGETS, SPLIT_SELECT_TOOL, SPLIT_SELECT_FACE_TARGETS, SPLIT_SELECT_FACE_TOOLS, SPLIT_SELECT_FACE_DIRECTION, SPLIT_FAILED, SPLIT_INVALID_INPUT, SPLIT_NO_CHANGE "Selected entity does not split selected parts.", SPLIT_FACE_NO_CHANGE, SPLIT_OFF_FACE, SPLIT_EDGE_INVALID_SURFACE, TOO_MANY_ENTITIES_SELECTED.
- **OCCT mapping:** Split part: BRepAlgoAPI_Splitter (BOPAlgo_Splitter) with arguments = targets, tools = tool shape (plane -> large/infinite BRepBuilderAPI_MakeFace(gp_Pln); extended face -> BRepBuilderAPI_MakeFace(underlying Geom_Surface, large UV bounds or BRepLib::ExtendFace)); explode result into solids/shells/wires; keep side by classifying each piece's centroid (BRepGProp) against the tool normal (signed distance / BRepClass3d on a half-space solid BRepPrimAPI_MakeHalfSpace). Split face: BRepFeat_SplitShape (Add(edge, face) with edges projected via BRepProj_Projection (direction) or BRepOffsetAPI_NormalProjection (normal to target)); face/plane tools: BRepAlgoAPI_Section to get intersection edges then BRepFeat_SplitShape, or BRepAlgoAPI_Splitter with face targets.
- **Priority:** P0 (part split by plane/face/surface), P1 (face split with curve projection)

### PS-19 Transform
- **Purpose:** Move, rotate, scale or copy parts (and mate connectors) by line, distance, XYZ, mate-connector pair, axis rotation, copy-in-place or scale.
- **Sources:** https://cad.onshape.com/help/Content/PartStudio/transform.htm ; FS: `fTransform` (transformCopy.fs; Feature Type Name "Transform"), `opTransform`, `opPattern` (for copies).
- **Selection inputs:**
  - "Entities to transform or copy" `entities`: BODYs (solid, sheet, wire, point, composite; mesh ok; non-sketch) or explicit mate connectors not in-context; multi. Non-modifiable bodies only allowed with Copy (MODIFIABLE_ENTITY_ONLY otherwise). Selecting a part's mate connector moves it with the part.
  - "Line or points" `transformLine`: VERTEX or EDGE; max 2; ordered (two points -> vector p0->p1; one edge -> start->end vector).
  - "Axis" `transformAxis`: ALLOWS_AXIS; max 1.
  - "Direction" `transformDirection`: ALLOWS_DIRECTION or VERTEX; max 2 (two vertices, an edge, a plane/planar face normal, cylindrical face axis, or mate connector Z).
  - "From mate connector" `baseConnector`, "To mate connector" `destinationConnector`: mate connectors (implicit/explicit); max 1 each.
  - "Point or mate connector" `scalePoint`: ALLOWS_VERTEX; max 1.
- **Parameters:**

| FS id | UI label | Type | Enum values / default / bounds | Visible when / notes |
|---|---|---|---|---|
| `entities` | Entities to transform or copy | Query | | always |
| `transformType` | Transform type | TransformType | TRANSLATION_ENTITY "Translate by line" (default), TRANSLATION_DISTANCE "Translate by distance", TRANSLATION_3D "Translate by XYZ", TRANSFORM_MATE_CONNECTORS "Transform by mate connectors", ROTATION "Rotate", COPY "Copy in place", SCALE_UNIFORMLY "Scale" | always |
| `oppositeDirectionEntity` | Opposite direction | boolean | false | TRANSLATION_ENTITY |
| `transformLine` | Line or points | Query max 2 | | TRANSLATION_ENTITY |
| `transformAxis` | Axis | Query max 1 | | ROTATION |
| `transformDirection` | Direction | Query max 2 | | TRANSLATION_DISTANCE |
| `distance` | Distance | length | NONNEGATIVE_LENGTH_BOUNDS 1e-5..500 m, default 25 mm / 1 in | TRANSLATION_DISTANCE |
| `uniform` | Scale uniformly | boolean | true | SCALE_UNIFORMLY |
| `scale` | Scale | real | SCALE_BOUNDS 1e-5..1e5, default 1 | SCALE_UNIFORMLY and uniform |
| `scaleX` / `scaleY` / `scaleZ` | X scale / Y scale / Z scale | real | SCALE_BOUNDS, default 1 | SCALE_UNIFORMLY and !uniform |
| `baseConnector` / `destinationConnector` | From mate connector / To mate connector | Query max 1 | | TRANSFORM_MATE_CONNECTORS |
| `oppositeDirectionMateAxis` | Flip primary axis | boolean | false | TRANSFORM_MATE_CONNECTORS (when defined) |
| `secondaryAxisType` | Reorient secondary axis | MateConnectorAxisType | PLUS_X "+X" (default), PLUS_Y "+Y", MINUS_X "-X", MINUS_Y "-Y" | TRANSFORM_MATE_CONNECTORS |
| `angle` | Angle | angle | ANGLE_360_BOUNDS -1e5..1e5 deg, default 30 deg | ROTATION |
| `oppositeDirection` | Opposite direction | boolean | false | ROTATION or TRANSLATION_DISTANCE |
| `dx` / `dy` / `dz` | X translation / Y translation / Z translation | length | ZERO_DEFAULT_LENGTH_BOUNDS -500..500 m, default 0 | TRANSLATION_3D |
| `scalePoint` | Point or mate connector | Query max 1 | | SCALE_UNIFORMLY |
| `makeCopy` | Copy part | boolean | false | transformType != COPY |

- **Behavior / semantics:**
  - Translate by line: translation = vector between the two picked vertices (selection order) or the edge's start->end chord; Opposite direction inverts. Translate by distance: direction from the pick (normalized), translation = direction * distance; linear manipulator at the selection's center. Translate by XYZ: world translation vector; triad manipulator. Rotate: about the axis by angle (reduced modulo 360), angular manipulator. Transform by mate connectors: T = World(To') * inverse(World(From)) where To' = To mate connector with optionally flipped Z and reoriented X (+Y = Z x X, -X, -Y) — the From frame is aligned onto the To frame. Copy in place: identity copy. Scale: uniform scale about the point (or MC origin); non-uniform scale along the mate connector's axes (or world axes at the vertex) — non-uniform scaling converts analytic surfaces to B-splines (inferred).
  - Copy part: instead of moving, creates transformed copies (opPattern with one instance named "1"); copies are new parts (new names, inferred "Part N"). Mate connectors cannot be copied (CANNOT_COPY_MATECONNECTORS).
  - Zero translation reports coincident info (reportCoincident). Help tip: for multiple copies use a pattern with distance 0.
- **Edge cases & errors:** CANNOT_RESOLVE_ENTITIES, MODIFIABLE_ENTITY_ONLY, TOO_MANY_ENTITIES_SELECTED, TRANSFORM_TRANSLATE_INPUT "Translation requires two vertices or an edge.", TRANSFORM_TRANSLATE_BY_DISTANCE_INPUT, NO_TRANSLATION_DIRECTION, TRANSFORM_MATE_CONNECTORS, TRANSFORM_SCALE_SELECTION, TRANSFORM_FAILED.
- **OCCT mapping:** BRepBuilderAPI_Transform(shape, gp_Trsf, copy) for rigid motions and uniform scale (gp_Trsf::SetScale / SetDisplacement(gp_Ax3 from, gp_Ax3 to) for mate connectors / SetRotation(gp_Ax1, angle) / SetTranslation); non-uniform scale: BRepBuilderAPI_GTransform(gp_GTrsf with SetVectorialPart diag(sx,sy,sz) in the MC frame: conjugate by frame transform). Copies: TopoDS shape copy (BRepBuilderAPI_Copy) + transform; identity/naming in app layer.
- **Priority:** P0

### PS-20 Copy part (deprecated feature)
- **Purpose:** Legacy feature that copies bodies in place.
- **Sources:** FS: `copyPart` (transformCopy.fs) annotated "Deprecated: Use [opPattern], or the [transform] feature instead."; no help page. Not offered in the UI toolbar; superseded by Transform > Copy in place / Copy part (PS-19).
- **Selection inputs:** "Entities to copy" `entities`: BODYs.
- **Parameters:**

| FS id | UI label | Type | Enum values / default / bounds | Visible when / notes |
|---|---|---|---|---|
| `entities` | Entities to copy | Query (BODY) | | always |

- **Behavior / semantics:** creates an in-place copy of each body (opPattern with identity transform). Keep only for loading legacy documents (inferred).
- **Edge cases & errors:** none specific.
- **OCCT mapping:** BRepBuilderAPI_Copy.
- **Priority:** P2 (legacy compatibility only)

### PS-21 Linear pattern
- **Purpose:** Replicate parts, faces or features along one direction or a 2D grid.
- **Sources:** https://cad.onshape.com/help/Content/PartStudio/linear_pattern.htm ; FS: `linearPattern` (linearPattern.fs), patternUtils.fs (`patternTypePredicate`, `applyPattern`, `processPatternBooleansIfNeeded`), `opPattern`, builtin `@computeLinearPatternTransforms`.
- **Selection inputs (from `patternTypePredicate`):**
  - Part pattern: "Entities to pattern" `entities`: BODYs or explicit (not in-context) mate connectors, mesh ok, non-sketch; multi; no new mate connector creation in the field.
  - Face pattern: "Faces to pattern" `faces`: modifiable non-sketch non-construction FACEs; feature selection allowed and "Create selection" helper (select tangent/pocket/boss face sets).
  - Feature pattern: "Features to pattern" `instanceFunction`: FeatureList picked in the feature list (any order; applied in feature-list order).
  - "Direction" `directionOne` / `directionTwo`: ALLOWS_DIRECTION (linear edge, planar face normal, axis, mate connector Z (implicit/explicit)); max 1.
- **Parameters:**

| FS id | UI label | Type | Enum values / default / bounds | Visible when / notes |
|---|---|---|---|---|
| `patternType` | Pattern type | PatternType | PART "Part pattern" (default), FEATURE "Feature pattern", FACE "Face pattern" | always |
| `operationType` | Result body operation type | NewBodyOperationType | NEW (default)/ADD/REMOVE/INTERSECT | PART |
| `entities` / `faces` / `instanceFunction` | Entities to pattern / Faces to pattern / Features to pattern | Query / Query / FeatureList | | per patternType |
| `directionOne` | Direction | Query max 1 | | always |
| `distance` | Distance | length | PATTERN_OFFSET_BOUND = NONNEGATIVE_ZERO_INCLUSIVE 0..500 m, default 25 mm / 1 in | always |
| `instanceCount` | Instance count | integer | PRIMARY_PATTERN_BOUNDS 1..2500, default 2 (includes seed) | always |
| `oppositeDirection` | Opposite direction | boolean | false | always |
| `isCentered` | Centered | boolean | false | always |
| `hasSecondDir` | Second direction | boolean | false | always |
| `directionTwo` | Direction | Query max 1 | | hasSecondDir |
| `distanceTwo` | Distance | length | 0..500 m, 25 mm / 1 in | hasSecondDir |
| `instanceCountTwo` | Instance count | integer | SECONDARY_PATTERN_BOUNDS 1..2500, default 1 | hasSecondDir |
| `oppositeDirectionTwo` | Opposite direction | boolean | false | hasSecondDir |
| `isCenteredTwo` | Centered | boolean | false | hasSecondDir |
| `defaultScope` / `booleanScope` | Merge with all / Merge scope | boolean / Query (solids or non-construction sheets) | false / — | PART and op != NEW (booleanPatternScopePredicate) |
| `fullFeaturePattern` | Reapply features | boolean | false | FEATURE |
| `skipInstances` | Skip instances | boolean | false | always |
| `skippedInstances[]` | Instances to skip (label "(#index1, #index2)") | array | | skipInstances |
| `skippedInstances[].index1` | First direction | integer | -1e5..1e5, default 1 | |
| `skippedInstances[].index2` | Second direction | integer | -1e5..1e5, default 0 | |

- **Behavior / semantics:**
  - offset vector = unit direction (from the pick) * distance (negated if opposite). Instances at i*offset1 + j*offset2 for i in [0, n1) (or (-(n1-1)) .. (n1-1) when Centered: i.e. n1 instances each side including the seed, total 2*n1-1) and similarly j; seed (0,0) excluded from created instances. Total instances n1*n2 must be <= 2500 (PATTERN_INPUT_TOO_MANY_INSTANCES). Instance names "i" or "i_j" (used for persistent naming of instance geometry).
  - Parallel first/second directions -> info PATTERN_DIRECTIONS_PARALLEL. Missing second direction with count > 1 -> PATTERN_LINEAR_NO_DIR.
  - Skip instances: toggle-point manipulator per instance location (gray points, skipped = light blue); seed cannot be skipped (info PATTERN_SKIPPED_INSTANCES_SEED_INDEX); out-of-range indices reported (PATTERN_SKIPPED_INSTANCES_OUT_OF_RANGE_INDEX) but kept in the list. Box-select toggles multiple.
  - Part pattern: opPattern copies the bodies (properties/attributes copied), then: NEW -> each copy a new part (help warns this creates distinct parts); ADD -> surfaces joined (seed included) then solids unioned with seed + merge scope; REMOVE/INTERSECT -> copies (not the seed) subtracted from / intersected with the merge scope (surfaces not allowed: SURFACES_NOT_SUPPORTED_BY_PATTERN_REMOVE_AND_INTERSECT). Patterned mate connectors are all added to assemblies when the Part Studio is inserted.
  - Face pattern: copies the faces and re-integrates them into the owning body (extending/trimming neighboring faces, like a local feature), must stay on the same body (PATTERN_NOT_ON_BODY, PATTERN_FACE_FAILED). Fastest; ignores feature intent.
  - Feature pattern (Reapply features off): bodies/faces created or modified by the selected features are patterned via combined face + body patterns (sketch regions patterned as sheets), no per-instance re-evaluation (an Up-to-next end condition does not update per instance). On failure suggests PATTERN_SWITCH_TO_PER_INSTANCE. Feature pattern without reapply is not available for sheet metal.
  - Feature pattern (Reapply features on): for each instance transform, re-executes each selected feature (sorted by feature-list order) with the instance transform pushed on the context ("pattern instance data"); features use the remainder transform for their references; per-instance external references (e.g. Up to next) re-evaluate. Sketch dimensions/constraints to the origin or default planes are not reapplied (info LINEAR_PATTERN_SKETCH_REAPPLY_INFO). Patterning a pattern feature patterns its instances but not its seed; patterning boolean features requires selecting the features they depend on.
  - Pattern of 1 instance allowed (no-op copies).
- **Edge cases & errors:** PATTERN_SELECT_PARTS / _FACES / _FEATURES, PATTERN_LINEAR_NO_DIR, PATTERN_INPUT_TOO_MANY_INSTANCES "Cannot have more than 2500 instances in a pattern.", PATTERN_INPUT_TOO_FEW_INSTANCES, PATTERN_FEATURE_FAILED, PATTERN_NO_GEOM_FROM_FEATURES, PATTERN_BODY_FAILED, PATTERN_FACE_FAILED, PATTERN_NOT_ON_BODY, SHEET_METAL_NO_FEATURE_PATTERN, DERIVED_FULL_FEATURE_PATTERN, MESH_NOT_SUPPORTED, CANNOT_USE_MATECONNECTORS_IN_PATTERN.
- **OCCT mapping:** Part pattern: BRepBuilderAPI_Transform (copy) per instance (or TopoDS_Shape::Moved with TopLoc_Location for lightweight instances, then BRepBuilderAPI_Copy when booleaned); booleans per PS-01 (one BRepAlgoAPI_Fuse/Cut with all instances as tools for performance). Face pattern: custom "local feature" engine: copy face set, transform, then re-attach — approach: build a tool solid from the face set (extract shell region + cap via BRepAlgoAPI_Defeaturing-style or by extruding boundary), or use BRepFeat_Gluer / BRepFeat_LocalOperation; realistic route: derive the "feature volume" (difference between body and body-with-faces-removed via BRepAlgoAPI_Defeaturing), pattern that volume and boolean add/cut accordingly. Feature pattern (reapply): pure app-level regeneration loop with a transform context. Skip instances: filter transform list.
- **Priority:** P0

### PS-22 Circular pattern
- **Purpose:** Replicate parts, faces or features around an axis.
- **Sources:** https://cad.onshape.com/help/Content/PartStudio/circular_pattern.htm ; FS: `circularPattern` (circularPattern.fs), builtin `@computeCircularPatternTransforms`.
- **Selection inputs:** pattern-type selections as PS-21; "Axis of pattern" `axis`: ALLOWS_AXIS (linear edge/sketch line, cylindrical/conical face axis, circular edge axis, mate connector Z); max 1.
- **Parameters:**

| FS id | UI label | Type | Enum values / default / bounds | Visible when / notes |
|---|---|---|---|---|
| `patternType`, `operationType`, `entities`/`faces`/`instanceFunction` | (as PS-21) | | | |
| `axis` | Axis of pattern | Query max 1 | | always |
| `angle` | Angle | angle | ANGLE_360_FULL_DEFAULT_BOUNDS -1e5..1e5 deg, default 360 deg | always |
| `instanceCount` | Instance count | integer | CIRCULAR_PATTERN_BOUNDS 1..2500, default 4 | always |
| `oppositeDirection` | Opposite direction | boolean (circular arrow) | false | always |
| `equalSpace` | Equal spacing | boolean | true (UI default; FS fallback false) | always |
| `isCentered` | Centered | boolean | false | always |
| `defaultScope`/`booleanScope` | Merge with all / Merge scope | | booleanPatternScopePredicate | PART, op != NEW |
| `fullFeaturePattern` | Reapply features | boolean | false | FEATURE |
| `skipInstances` | Skip instances | boolean | false | always |
| `skippedInstances[].index` | Instances to skip / Index | integer | -1e5..1e5, default 1 | skipInstances |

- **Behavior / semantics:** angle normalized (adjustAngle), negated when opposite. Equal spacing: step = angle / n if the angle is a full 360 deg (within tolerance), else angle / (n-1) (instances span the angle with first and last at the ends); without equal spacing, `angle` is the step between consecutive instances. Instances at rotations i*step about the axis, i in [1, n) or, Centered, i in [-(n-1), n-1] \ {0}. Skip-instance indices (negative allowed when centered). Rest as PS-21 (booleans, feature/face semantics, CIRCULAR_PATTERN_SKETCH_REAPPLY_INFO).
- **Edge cases & errors:** PATTERN_CIRCULAR_NO_AXIS "Select a pattern axis.", PATTERN_INPUT_TOO_FEW_INSTANCES (equal spacing with < 1), plus PS-21 errors.
- **OCCT mapping:** gp_Trsf::SetRotation(gp_Ax1, angle) + BRepBuilderAPI_Transform; booleans as PS-21.
- **Priority:** P0

### PS-23 Curve pattern
- **Purpose:** Replicate parts, faces or features along a curve/path.
- **Sources:** https://cad.onshape.com/help/Content/PartStudio/curve_pattern.htm ; FS: `curvePattern` (curvePattern.fs).
- **Selection inputs:** pattern-type selections as PS-21; "Path to pattern along" `edges`: EDGEs or non-sketch WIRE bodies; multi; must form one continuous path (open or closed). "Faces to be normal to" `lockedFaces`: non-construction FACEs.
- **Parameters:**

| FS id | UI label | Type | Enum values / default / bounds | Visible when / notes |
|---|---|---|---|---|
| `patternType`, `operationType`, `entities`/`faces`/`instanceFunction` | (as PS-21) | | | |
| `edges` | Path to pattern along | Query | | always |
| `spacingType` | Spacing type | CurvePatternSpacingType | EQUAL "Equal spacing" (default), DISTANCE "Distance" | always |
| `distance` | Distance | length | 0..500 m, default 25 mm / 1 in | DISTANCE |
| `instanceCount` | Instance count | integer | CURVE_PATTERN_BOUNDS 1..2500, default 2 | always |
| `orientationType` | Orientation | CurvePatternOrientationType | DEFAULT "Tangent to curve" (default), LOCK_FACES "Normal to face", KEEP_ORIENTATION "Locked" | always |
| `lockedFaces` | Faces to be normal to | Query | | LOCK_FACES |
| `defaultScope`/`booleanScope` | Merge with all / Merge scope | | | PART, op != NEW |
| `fullFeaturePattern` | Reapply features | boolean | false | FEATURE |
| `skipInstances`, `skippedInstances[].index` | Skip instances / Index | | as PS-22 | |

- **Behavior / semantics:** Path built from the edges (dissolving wires); for closed paths the parameter 0 is remapped to the path point closest to the seed (info CURVE_PATTERN_START_OFF_PATH / _CLOSED_PATH if the seed bbox does not touch the start). Parameters: Equal - open path i/(n-1), closed path i/n; Distance - i*distance/pathLength (closed: pathLength - distance used; error CURVE_PATTERN_DISTANCE_TOO_LARGE if (n-1)*d (open) or n*d (closed) exceeds length). Orientation: Tangent to curve - each instance's frame follows the path (frames computed by sweeping a tiny line segment along the path to get a rotation-minimizing frame/tangent planes; falls back to tangent lines); Normal to face - frames keep the binormal aligned to the normals of `lockedFaces` (CURVE_PATTERN_LOCK_FACES_INTERSECTION_FAILED / CURVE_PATTERN_MISSING_FACE_SELECTION); Locked - pure translation keeping the seed orientation. Transform for instance i maps the frame at parameter 0 to the frame at parameter t_i. Rest as PS-21 (CURVE_PATTERN_SKETCH_REAPPLY_INFO).
- **Edge cases & errors:** PATTERN_CURVE_NO_EDGES, PATH_EDGES_NOT_CONTINUOUS, CURVE_PATTERN_DISTANCE_TOO_LARGE, PATTERN_INPUT_TOO_FEW_INSTANCES, plus PS-21.
- **OCCT mapping:** BRepAdaptor_CompCurve over the path wire; GCPnts_AbscissaPoint for arc-length positions; frames via GeomFill_CorrectedFrenet / GeomFill_LocationLaw (rotation-minimizing) or GeomFill_ConstantBiNormal for locked/face-normal (face normal from BRepLProp_SLProps at the projected point, GeomAPI_ProjectPointOnSurf); gp_Trsf from gp_Ax3 frames (SetTransformation); BRepBuilderAPI_Transform.
- **Priority:** P1

### PS-24 Mirror
- **Purpose:** Mirror parts, faces or features across a plane.
- **Sources:** https://cad.onshape.com/help/Content/PartStudio/mirror.htm ; FS: `mirror` (mirror.fs).
- **Selection inputs:** "Entities to mirror" `entities` (Part mirror): BODYs, mesh ok, non-sketch; "Faces to mirror" `faces` (Face mirror): modifiable FACEs, create-selection helper; "Features to mirror" `instanceFunction` (FeatureList). "Mirror plane" `mirrorPlane`: ALLOWS_PLANE (construction plane, planar face, implicit/explicit mate connector XY); max 1.
- **Parameters:**

| FS id | UI label | Type | Enum values / default / bounds | Visible when / notes |
|---|---|---|---|---|
| `patternType` | Mirror type | MirrorType | PART "Part mirror" (default), FEATURE "Feature mirror", FACE "Face mirror" | always |
| `operationType` | Result body operation type | NewBodyOperationType | NEW | PART |
| `entities` / `faces` / `instanceFunction` | Entities to mirror / Faces to mirror / Features to mirror | | | per type |
| `mirrorPlane` | Mirror plane | Query max 1 | | always |
| `defaultScope`/`booleanScope` | Merge with all / Merge scope | | booleanPatternScopePredicate | PART, op != NEW |
| `fullFeaturePattern` | Reapply features | boolean | UI default false (FS fallback true for legacy) | FEATURE |

- **Behavior / semantics:** single reflection transform `mirrorAcross(plane)` with instance name "1"; mirror plane from mate connector or planar face (not transformed when inside a feature pattern). Part mirror ADD includes the original bodies as tools (seed) so the mirrored half unions with the original (typical symmetric part workflow); surfaces whose laminar edges lie on the mirror plane are joined. REMOVE/INTERSECT use only the mirrored copies. Edit logic runs boolean heuristics (PS-01.3) so a mirrored body touching its seed defaults to Add with the seed as scope. No skip instances. Feature mirror with Reapply re-executes features per mirrored instance (sketch reapply info MIRROR_SKETCH_REAPPLY_INFO). Mirrored geometry has reversed orientation handled by the kernel (asymmetric chamfers auto-compensated).
- **Edge cases & errors:** MIRROR_NO_PLANE "Select a mirror plane.", MIRROR_SELECT_PARTS / _FACES / _FEATURES, MIRROR_BODY_FAILED, MIRROR_FACE_FAILED, MIRROR_INVALID_SEED_ASSEMBLY.
- **OCCT mapping:** gp_Trsf::SetMirror(gp_Ax2 plane) + BRepBuilderAPI_Transform (handles orientation reversal); union with seed via BRepAlgoAPI_Fuse + ShapeUpgrade_UnifySameDomain (removes the seam on the mirror plane); face mirror: as face pattern (PS-21).
- **Priority:** P0

### PS-25 Delete face
- **Purpose:** Direct edit: remove faces from a body and heal by extending neighbors, cap with a planar face, or leave open (solid -> surface).
- **Sources:** https://cad.onshape.com/help/Content/PartStudio/delete_face.htm ; FS: `deleteFace` (deleteFace.fs), `opDeleteFace`.
- **Selection inputs:** "Delete faces" `deleteFaces`: modifiable non-sketch non-construction FACEs (solid or sheet, mesh ok); multi; Create-selection helper (select bosses/pockets/fillet chains).
- **Parameters:**

| FS id | UI label | Type | Enum values / default / bounds | Visible when / notes |
|---|---|---|---|---|
| `deleteFaces` | Delete faces | Query | | always |
| `healType` | Type | DeleteFaceType | HEAL "Heal" (default), CAP "Cap", VOID "Leave open"; remembers previous; edit logic sets VOID when all first-selected faces are sheet faces | always |
| `includeFillet` | Delete fillet faces | boolean | true | always |

- **Behavior / semantics:** Heal: removes faces and extends/intersects the surrounding faces to close the gap (like removing a feature from imported geometry). Cap: if the hole cannot be healed by extension, replaces it with a planar face. Leave open: removes faces leaving an open sheet body. Delete fillet faces: also removes fillets adjacent to the selected faces (and re-blends as needed). Cannot delete all faces of a body.
- **Edge cases & errors:** DIRECT_EDIT_DELETE_SELECT_FACES, DIRECT_EDIT_DELETE_FACE_FAILED, DIRECT_EDIT_DELETE_FACE_ALL_FACES.
- **OCCT mapping:** Heal: BRepAlgoAPI_Defeaturing (AddFaceToRemove, SetRunParallel; extends adjacent faces to fill). Leave open: BRepTools_ReShape remove faces from the shell, result as shell/sheet (ShapeFix_Shell). Cap: if defeaturing fails and the free boundary is planar, BRepBuilderAPI_MakeFace(planar wire from ShapeAnalysis_FreeBounds) + BRepBuilderAPI_Sewing + MakeSolid. Fillet inclusion: detect adjacent fillet faces (custom: cylindrical/toroidal/rolling-ball recognition) and add to the removal set.
- **Priority:** P1

### PS-26 Move face
- **Purpose:** Direct edit: offset, translate or rotate faces of a body; neighbors are extended/trimmed to follow.
- **Sources:** https://cad.onshape.com/help/Content/PartStudio/move_face.htm ; FS: `moveFace` (moveFace.fs), `opMoveFace`, `opOffsetFace`.
- **Selection inputs:**
  - "Faces to move" `moveFaces` (output MOVE): modifiable non-sketch non-construction FACEs (non-sheet-metal, or sheet-metal definition faces/edges); multi; Create-selection helper.
  - "Faces, surfaces, and sketch regions to offset" `surfacesAndFaces` (legacy CREATE output): FACEs or SHEET bodies.
  - "Direction" `direction` (Translate): ALLOWS_DIRECTION; max 1. "Axis" `axis` (Rotate): ALLOWS_AXIS; max 1.
  - "Up to entity" `limitEntity` (Translate up-to): planar FACE or VERTEX; max 1. "Up to entity" `limitQuery` (Offset up-to): VERTEX/EDGE/FACE/BODY (mesh ok); max 1.
- **Parameters:**

| FS id | UI label | Type | Enum values / default / bounds | Visible when / notes |
|---|---|---|---|---|
| `outputType` | Creation type | MoveFaceOutputType | MOVE "Move" (default), CREATE "Create" (legacy offset-surface creation) | always hidden |
| `moveFaces` / `surfacesAndFaces` | Faces to move / Faces, surfaces, and sketch regions to offset | Query | | per outputType |
| `moveFaceType` | Move face type | MoveFaceType | OFFSET "Offset" (default), TRANSLATE "Translate", ROTATE "Rotate" | always |
| `direction` | Direction | Query max 1 | | TRANSLATE |
| `axis` | Axis | Query max 1 | | ROTATE |
| `limitType` | End type | MoveFaceBoundingType | BLIND "Blind" (default), UP_TO_ENTITY "Up to entity" | moveFaceType != ROTATE |
| `translationDistance` | Distance | length | MOVE_FACE_TRANSLATE_BOUNDS = ZERO_INCLUSIVE_OFFSET 0..500 m, 5 mm / 0.25 in | TRANSLATE and BLIND |
| `angle` | Rotation angle | angle | ANGLE_360_ZERO_DEFAULT_BOUNDS, default 0 | ROTATE |
| `offsetDistance` | Distance | length | ZERO_INCLUSIVE_OFFSET 0..500 m, 5 mm / 0.25 in | OFFSET and BLIND |
| `oppositeDirection` | Opposite direction | boolean | false | BLIND or ROTATE |
| `limitEntity` | Up to entity | Query max 1 | | TRANSLATE and UP_TO_ENTITY |
| `limitQuery` | Up to entity | Query max 1 | | OFFSET and UP_TO_ENTITY |
| `hasOffset` | Offset distance | boolean | false | UP_TO_ENTITY |
| `offset` | Offset | length | 0..500 m, default 0 | hasOffset |
| `oppositeOffsetDirection` | Opposite offset direction | boolean | true | hasOffset |
| `reFillet` | Reapply fillet | boolean | true | outputType MOVE |

- **Behavior / semantics:** Offset: each selected face is offset along its normal by the signed distance (typical to change a cylinder radius / wall thickness); up-to-entity computes the distance from the first face to the entity (evDistance) plus optional offset. Translate: faces moved by direction*distance (direction from edge, axis, MC Z, or planar face normal); up-to-entity moves until the first face reaches the plane/vertex (+offset). Rotate: faces rotated about the axis by angle. Adjacent faces are extended or trimmed to stay connected; if topology cannot be maintained the operation fails. Reapply fillet: fillets adjacent to moved faces are removed and re-created at their original radius (otherwise fillets are treated as regular faces). Inside a mirrored feature pattern, translate/rotate directions are auto-flipped. Zero offset = no-op. Sheet-metal-aware: moving sheet-metal faces/edges modifies the sheet-metal definition (e.g. flange length). Linear/angular manipulators on the first face's tangent plane.
- **Edge cases & errors:** DIRECT_EDIT_MOVE_FACE_SELECT, DIRECT_EDIT_MOVE_FACE_CREATE_SELECT, DIRECT_EDIT_MOVE_FACE_FAILED, DIRECT_EDIT_OFFSET_FACE_FAILED, MOVE_FACE_NO_INTERSECTION "First face cannot be moved to limit entity.", NO_TANGENT_PLANE.
- **OCCT mapping:** Offset: BRepOffset_MakeOffset with per-face offsets (SetOffsetOnFace) and mode BRepOffset_Skin on the whole body with unselected faces at 0 (or BRepOffsetAPI_MakeOffsetShape on a face set then re-intersect neighbors); Translate/Rotate: custom local operation — transform the selected faces' surfaces (Geom_Surface::Transformed), re-intersect with adjacent faces' extended surfaces (GeomAPI_IntSS / BOPAlgo), rebuild trimmed faces (BRepLib_MakeFace + ShapeFix); practical approach: BRepAlgoAPI_Defeaturing to remove faces, then rebuild via BRepFeat (BRepFeat_MakePrism for planar translate) or general fill (BRepOffsetAPI_MakeFilling). Fillet reapply: defeature fillets first, re-fillet after. Gap: no generic OCCT "move face" (Parasolid local ops) — significant custom work.
- **Priority:** P1

### PS-27 Replace face
- **Purpose:** Direct edit: replace the geometry of selected faces with the surface of other face(s) (extend or trim to a new surface), optionally offset.
- **Sources:** https://cad.onshape.com/help/Content/PartStudio/replace_face.htm ; FS: `replaceFace` (replaceFace.fs), `opReplaceFace`.
- **Selection inputs:** "Faces to replace" `replaceFaces`: modifiable non-sketch non-construction FACEs (mesh ok); multi; Create-selection helper. "Faces to replace with" `templateFace`: FACEs (one or more adjacent faces of the same body; mesh ok).
- **Parameters:**

| FS id | UI label | Type | Enum values / default / bounds | Visible when / notes |
|---|---|---|---|---|
| `replaceFaces` | Faces to replace | Query | | always |
| `templateFace` | Faces to replace with | Query | | always |
| `showFlip` | Show Flip | boolean | true; hidden (edit logic hides flip when sense is unambiguous) | — |
| `oppositeSense` | Flip alignment | boolean | false (same sense as template surface normal; true = opposite, e.g. use inside vs outside of a cylinder) | showFlip |
| `offset` | Offset distance | length | NONNEGATIVE_ZERO_DEFAULT 0..500 m, default 0 | always |
| `oppositeDirection` | Opposite direction | boolean | false | always |

- **Behavior / semantics:** The selected faces are replaced by the underlying surface(s) of the template faces (offset by the signed offset); adjacent faces are extended/trimmed to meet the new surface. The template must be large enough to cover the replaced region (REPLACE_FACE_SHEET_SMALL) and multiple templates must be adjacent and on one body. The default replacement uses the template sense closest to the face being replaced; Flip alignment chooses the opposite. Linear offset manipulator located at the template face mid-point. Typical use: extend a face up to a surface created for that purpose.
- **Edge cases & errors:** DIRECT_EDIT_REPLACE_FACE_SELECT, DIRECT_EDIT_REPLACE_FACE_FAILED, REPLACE_FACE_FACE_COUNT_CHANGED "Replace face would split or merge faces.", REPLACE_FACE_SHEET_SMALL, REPLACE_FACES_NOT_ADJACENT, REPLACE_FACES_NOT_SAME_BODY.
- **OCCT mapping:** No direct equivalent (gap). Custom: for each replaced face, take template Geom_Surface (offset via Geom_OffsetSurface if needed), recompute edges as intersections with adjacent faces' (extended) surfaces (GeomAPI_IntSS, BRepAlgoAPI_Section), rebuild face/wires and adjacent face trims (BRepBuilderAPI_MakeFace, ShapeFix_Face, BRepTools_ReShape), validate with BRepCheck_Analyzer. Simple cases (planar replace) can use BRepFeat_MakePrism "until" or a split-and-fill approach: cut the body with the template surface (BRepAlgoAPI_Splitter) and keep/fuse cells.
- **Priority:** P1

### PS-28 Modify fillet
- **Purpose:** Direct edit: change the radius of existing (e.g. imported) fillet faces or remove them.
- **Sources:** https://cad.onshape.com/help/Content/PartStudio/modify_fillet.htm ; FS: `modifyFillet` (modifyFillet.fs), `opModifyFillet`, `evFilletRadius`.
- **Selection inputs:** "Fillet faces to modify" `faces`: non-planar modifiable non-sketch non-construction FACEs; multi; Create-selection helper for fillet chains.
- **Parameters:**

| FS id | UI label | Type | Enum values / default / bounds | Visible when / notes |
|---|---|---|---|---|
| `faces` | Fillet faces to modify | Query | | always |
| `modifyFilletType` | Modification type | ModifyFilletType | CHANGE_RADIUS "Change radius" (default), REMOVE_FILLET "Remove fillet" | always |
| `radius` | Fillet radius | length | BLEND_BOUNDS 1e-5..500 m, default 5 mm / 0.2 in; edit logic pre-fills with the measured radius of the first selection (evFilletRadius) | CHANGE_RADIUS |
| `reFillet` | Reapply fillet | boolean | true | CHANGE_RADIUS |

- **Behavior / semantics:** Recognizes the selected faces as constant-radius blends between two support faces (rolling-ball), then removes them (extending the supports to their intersection) or re-blends with the new radius. Reapply fillet re-creates dependent/adjacent fillets so the modified fillet flows into them. Fails if faces are not recognizable as fillets.
- **Edge cases & errors:** DIRECT_EDIT_MODIFY_FILLET_SELECT, DIRECT_EDIT_MODIFY_FILLET_FAILED, DIRECT_EDIT_NO_FILLET_FACES, DIRECT_EDIT_FAILED_TO_IDENTIFY_FILLETS, DIRECT_EDIT_ALL_FILLET_FACES_SELECTED.
- **OCCT mapping:** Fillet recognition: custom (cylindrical/toroidal/spherical faces tangent (G1) to two neighbors; radius from Geom surface). Remove: BRepAlgoAPI_Defeaturing on the fillet faces. Change radius: defeature, then BRepFilletAPI_MakeFillet on the recovered sharp edges (found via BRepAlgoAPI_Defeaturing::Generated/Modified history) with the new radius.
- **Priority:** P1

### PS-29 Delete part
- **Purpose:** Parametrically delete parts, surfaces, curves, or composite parts (undoable feature).
- **Sources:** https://cad.onshape.com/help/Content/PartStudio/delete_part.htm ; FS: `deleteBodies` (deleteBodies.fs, Feature Type Name "Delete part"), `opDeleteBodies`.
- **Selection inputs:** "Entities to delete" `entities`: modifiable non-sketch BODYs incl. composite parts (mesh ok); or `nonCompositeEntities` (same, excluding composites) when composite option is Ignore; multi.
- **Parameters:**

| FS id | UI label | Type | Enum values / default / bounds | Visible when / notes |
|---|---|---|---|---|
| `compositePartOption` | Composite part options | CompositePartDeleteOptions | DELETE "Delete composite parts" (default), DISSOLVE "Dissolve composite parts", IGNORE "Ignore composite parts" | always (irrelevant for non-composites) |
| `entities` | Entities to delete | Query | | option != IGNORE |
| `nonCompositeEntities` | Entities to delete | Query | | option == IGNORE (edit logic keeps both fields synchronized) |

- **Behavior / semantics:** DELETE: deletes the selected bodies and, for composite parts, also their constituent bodies. DISSOLVE: deletes only the composite container, constituents remain as individual parts. IGNORE: composites cannot be selected. Typical use: discard tool bodies after booleans. Pressing Delete with a face/edge selected in the viewport deletes the owning feature (not the part). Automatic part colors are re-sequenced after a deletion (unless custom colors).
- **Edge cases & errors:** DELETE_PARTS_FAILED, DELETE_PARTS_PARTIAL.
- **OCCT mapping:** none (app data model: remove bodies from the Part Studio body list; record deletion in history for naming).
- **Priority:** P0

### PS-30 External thread
- **Purpose:** Apply ANSI/ISO cosmetic threads to convex cylindrical faces (shafts, bolts), optionally with an end chamfer, an undercut groove and a split face at the thread end.
- **Sources:** https://cad.onshape.com/help/Content/PartStudio/external_thread.htm ; FS: `externalThread` (externalThread.fs), holetables.gen.fs (ANSI_ExternalThreadTable, ISO_ExternalThreadTable), cosmeticThreadUtils.fs.
- **Selection inputs:** "Cylinder edges" `entities`: circular or arc (sweep >= 182 deg) two-sided non-sketch EDGEs bounding a convex cylinder, modifiable, not mesh, not active sheet metal; multi (same diameter); unconfigurable. The edge picks the cylinder and the thread start end.
- **Parameters:**

| FS id | UI label | Type | Enum values / default / bounds | Visible when / notes |
|---|---|---|---|---|
| `entities` | Cylinder edges | Query | | always |
| `branchOfStandard` | Standard | LookupTablePath | Standard ANSI (Size #0..6, 71 sizes; Threads/inch) or ISO (Size M1..M300, 106 sizes; Pitch); auto-sized from the cylinder diameter (edit logic); remembers previous; unconfigurable | always |
| `depthType` | End type | DepthType | Blind "Blind", UpToNext "Up to next"; remembers previous | always |
| `threadDepth` | Length | length | EXTERNAL_THREAD_DEPTH_BOUNDS 0.1 mm..200 m, default 10 mm / 0.5 in | Blind |
| `addChamfer` | Add chamfer | boolean | false | always |
| `chamferWidth` | Length | length | BLEND_BOUNDS 1e-5..500 m, default 5 mm / 0.2 in | addChamfer |
| `chamferAngle` | Angle | angle | 0.1..89 deg, default 30 deg | addChamfer |
| `addUndercut` | Add undercut | boolean | false | always |
| `undercutDiameter` | Diameter | length | NONNEGATIVE_LENGTH_BOUNDS; defaults to minor diameter - 0.015 in (diametral), autosized | addUndercut |
| `undercutLength` | Length | length | NONNEGATIVE_LENGTH_BOUNDS; defaults to 2 x pitch | addUndercut |
| `splitFace` | Split face | boolean | true; remembers previous | !addUndercut and Blind |

- **Behavior / semantics:** Thread starts at the selected edge and runs along the cylinder for Length (Blind) or to the next face (Up to next = whole cylinder face). Geometry changes only for options: chamfer (opChamfer distance/angle on the start edge), undercut (split cylinder face with temporary planes at thread end, then offset the groove face inward to the undercut diameter), split face (split the cylinder face at the thread end). A cosmetic thread attribute (coordinate system on axis, depth, pitch, size/standard) is attached to the threaded face: rendered as thread lines in the Part Studio, dashed lines in drawings, hole/thread callouts supported; not included in hole tables. Size auto-selection: changing standard requires re-selecting size for autosizing.
- **Edge cases & errors:** THREAD_DEPTH_BEYOND_CYLINDER "Thread length is greater than cylinder length.", EXTERNAL_THREADS_UNSUPPORTED_ON_SHEET_METAL, UNDERCUT_OFF_FACE.
- **OCCT mapping:** Cosmetic data in app model (and XCAF/STEP AP242 semantic PMI where possible). Geometry options: BRepFilletAPI_MakeChamfer AddDA; face split BRepFeat_SplitShape / BRepAlgoAPI_Splitter with planes; undercut = offset face (BRepOffset_MakeOffset per-face) or cut a ring (BRepPrimAPI_MakeCylinder difference) via BRepAlgoAPI_Cut. Thread lines rendering: draw helix-free circles/lines in the renderer.
- **Priority:** P1

### PS-31 Decal
- **Purpose:** Place an image (from the document, other documents or image libraries) on a planar or cylindrical face as an appearance attribute (no geometry change).
- **Sources:** https://cad.onshape.com/help/Content/PartStudio/decal.htm ; FS: `decal` (decal.fs, UIHint NO_PREVIEW_PROVIDED), decalUtils.fs (DecalData attribute), tabReferences.fs (ImageData).
- **Selection inputs:** "Image" `image` (ImageData: reference to an image tab/blob in current doc, other docs, or library; "Select image" dialog with search/import; max 4k x 4k). "Face" `face`: one modifiable non-sketch non-construction planar or cylindrical FACE (preselected face pre-fills). "Placement origin" `originConnector`: mate connector (implicit/explicit; decal plane = MC XY, origin = MC origin); max 1. "Horizontal reference" `horizontalReference`: ALLOWS_DIRECTION or mate connector; max 1.
- **Parameters:**

| FS id | UI label | Type | Enum values / default / bounds | Visible when / notes |
|---|---|---|---|---|
| `image` | Image | ImageData | | always |
| `face` | Face | Query max 1 | | always |
| `angle` | Angle | angle | ANGLE_360_ZERO_DEFAULT_BOUNDS, default 0 | always |
| `angleOppositeDirection` | Angle opposite direction | boolean | false | always |
| `uShift` | U shift | length | IMAGE_OFFSET_BOUNDS -1e8..1e8 m, default 0 | always |
| `vShift` | V shift | length | IMAGE_OFFSET_BOUNDS, default 0 | always |
| `realign` | Realign | boolean | false | always |
| `originConnector` | Placement origin | Query max 1 | | realign |
| `horizontalReference` | Horizontal reference | Query max 1 | | realign |
| `maintainAspectRatio` | Maintain aspect ratio | boolean | true | always |
| `aspectRatioConstraint` | Aspect ratio constraint | ImageAspectRatioConstraint | WIDTH_DRIVING "Width", HEIGHT_DRIVING "Height" | maintainAspectRatio |
| `width` | Width | length | IMAGE_SIZE_BOUNDS 1e-5..1e8 m, default 25 mm / 1 in (edit logic sets a default size fitting the face once image+face are chosen) | !maintainAspectRatio or WIDTH_DRIVING |
| `height` | Height | length | IMAGE_SIZE_BOUNDS | !maintainAspectRatio or HEIGHT_DRIVING |
| `imageOriginIndex` | Image origin index | integer | 0..8 (9 reference points: corners, edge midpoints, center; default center) ; hidden, set by clicking reference points | — |

- **Behavior / semantics:** Computes a DecalData (image, face, placement frame (origin, U/V axes on the surface; cylinder: wrapped around the axis), width/height) and associates it as an attribute on the face; rendering maps the image onto the face (planar projection, or cylindrical wrap). Maintain aspect ratio derives the other dimension from the image pixel aspect. Realign: origin at the mate connector; the image's chosen reference point (of 9) is placed at the origin; horizontal reference aligns the image X axis. Triad/angle/resize manipulators. Decal must remain on the face (DECAL_PROJECTED_OFF_FACE).
- **Edge cases & errors:** DECAL_NO_IMAGE_SELECTION, DECAL_NO_FACE_SELECTION, DECAL_PROJECTED_OFF_FACE, DECAL_HORIZONTAL_REFERENCE_INVALID_ENTITY, DECAL_IMAGE_TOO_LARGE "Inserted image exceeds 4k x 4k size limit".
- **OCCT mapping:** none for geometry; renderer feature (texture projection in GPUI/wgpu pipeline) + XCAF/glTF export as textures (RWGltf_CafWriter supports textured materials; STEP has no decal concept).
- **Priority:** P2

### PS-32 Derived mirror (internal)
- **Purpose:** Internal feature that builds a mirrored copy of parts from another Part Studio, used by Assembly mirror ("Internal feature for assembly mirror").
- **Sources:** FS: `derivedMirror` (derivedMirror.fs, marked @internal); https://cad.onshape.com/help/Content/Assembly/assembly_mirror.htm (user-facing entry point is in Assemblies).
- **Selection inputs:** none (read-only).
- **Parameters:**

| FS id | UI label | Type | Default | Visible when / notes |
|---|---|---|---|---|
| `partStudio` | Part Studio | PartStudioData (build function + configuration + part query) | | read-only, unconfigurable |

- **Behavior / semantics:** Instantiates the source Part Studio's build in a separate context, derives the selected parts (including flat sheet-metal handling for newer versions; closed composites with active sheet metal fall back to plain derive) and mirrors them, producing new "mirrored" parts in a generated Part Studio. Not inserted manually by users.
- **Edge cases & errors:** IMPORT_DERIVED_NO_PARTS, CANNOT_RESOLVE_ENTITIES.
- **OCCT mapping:** Derive (cross-studio body import, app-level) + gp_Trsf::SetMirror + BRepBuilderAPI_Transform.
- **Priority:** P2 (needed only with Assembly mirror)

### PS-33 Primitive features (Cube, Sphere, fCuboid, fCylinder, fCone, fEllipsoid) — FS-only
- **Purpose:** Programmatic primitive bodies for FeatureScript authors; not exposed in the Part Studio toolbar (no help pages; `cube` and `sphere` have Feature Type Names "Cube"/"Sphere" and are usable from custom features / FS console; `fCuboid`, `fCylinder`, `fCone`, `fEllipsoid` take raw vectors and are internal helpers used by other features, e.g. boolean subtract-complement bounding box).
- **Sources:** FS: primitives.fs (`cube`, `sphere`/`fSphere`, `fCuboid`, `fCylinder`, `fCone`, `fEllipsoid`), opSphere etc.
- **Selection inputs:** `fSphere` "Center" `center`: VERTEX, max 1 (only when defined).
- **Parameters:**

| Feature | FS id | UI label | Type | Bounds / default | Notes |
|---|---|---|---|---|---|
| cube | `sideLength` | Side length | length | NONNEGATIVE_LENGTH_BOUNDS 1e-5..500 m, 25 mm / 1 in | corner at origin, extends +X+Y+Z |
| fSphere | `center` | Center | Query (VERTEX) | | |
| fSphere | `radius` | Radius | length | NONNEGATIVE_LENGTH_BOUNDS | |
| fCuboid | `corner1`, `corner2` | — | 3D length vectors | must differ in every coordinate | axis-aligned box |
| fCylinder | `bottomCenter`, `topCenter`, `radius` | — | vectors, length | radius NONNEGATIVE_LENGTH_BOUNDS | |
| fCone | `bottomCenter`, `topCenter`, `bottomRadius`, `topRadius` | — | vectors, lengths | radii NONNEGATIVE_ZERO_INCLUSIVE (one may be 0) | |
| fEllipsoid | `center`, `radius` | — | 3D length vectors | radius = semi-axes along X/Y/Z | |

- **Behavior / semantics:** Each creates a new solid body (new part); no boolean step; pattern-aware (`cube`).
- **Edge cases & errors:** degenerate dimensions rejected by precondition.
- **OCCT mapping:** BRepPrimAPI_MakeBox, BRepPrimAPI_MakeSphere, BRepPrimAPI_MakeCylinder (gp_Ax2 from bottom->top), BRepPrimAPI_MakeCone, ellipsoid = sphere + BRepBuilderAPI_GTransform non-uniform scale.
- **Priority:** P2 (API/FS-compat only; useful internally)

### PS-34 Section part features (internal, drawings/assemblies)
- **Purpose:** Internal features that cut parts for section views in drawings and assembly section views; not exposed in the Part Studio UI.
- **Sources:** FS: sectionpart.fs — `sectionPart` ("Not exposed in the Part Studio UI"), `planeSectionPart`, `multiplePlaneSectionPart`, `jogSectionPart` / `jogSectionPartInternal` ("Used by drawings"), `sectionTransformedParts`, `multiplePlanesSectionTransformedParts` (assembly sections with occurrence transforms). Drawing-side behavior in the Drawings section.
- **Selection inputs / Parameters (all programmatic, no UI labels):**

| Feature | FS id | Type | Notes |
|---|---|---|---|
| sectionPart | `targets`, `plane` | Query, Plane | split by plane, delete bodies in front (positive side) |
| planeSectionPart | `target`, `plane` | Query, Plane | like sectionPart but keeps bodies entirely behind the plane; non-target bodies deleted |
| multiplePlaneSectionPart | `target`, `sketchPlanes` | Query, array of Plane | remove material on the positive side of each plane |
| jogSectionPart / jogSectionPartInternal | `target`, `sketchPlane`, `jogPoints` (array of 3D points; broken-out: [splinePts..., depthPt, ...]), `bbox` (Box3d, optional), `isPartialSection`, `keepSketches`, `isBrokenOut`, `isCropView`, `brokenOutPointNumbers`, `brokenOutEndConditions`, `offsetPoints`, `isAlignedSection` (+ `excludedPartsQuery` must be undefined when aligned) | | jogged/offset/aligned/partial/broken-out/crop sections: a polyline (or spline for broken-out) section line drawn in `sketchPlane` is extruded normal to it, everything on the +x side of the line removed |
| sectionTransformedParts | `targets` (array of SectionTarget: body query + occurrence transform), `sketchPlane`, `jogPoints`, `bbox`, `isPartialSection`, `isBrokenOut`, `isCropView`, `keepSketches`, `brokenOutPointNumbers`, `brokenOutEndConditions`, `offsetPoints`, `isAlignedSection`, `queriesToTrack` (map Query->array) | | assembly variant |
| multiplePlanesSectionTransformedParts | `targets` (SectionTarget array), `sketchPlanes`, `bbox` | | assembly multi-plane variant |

- **Behavior / semantics:** opSplitPart with the plane / extruded jog-line sheet, then opDeleteBodies of the removed side; hole attributes on cut parts are preserved for callouts; sheet-metal intermediate data cleared.
- **Edge cases & errors:** split failures surface in the drawing view.
- **OCCT mapping:** BRepAlgoAPI_Splitter (or BRepAlgoAPI_Cut with a half-space BRepPrimAPI_MakeHalfSpace / prism of the section polyline) then discard; broken-out: prism of a closed spline region to a depth, BRepAlgoAPI_Cut; section hatching faces from BRepAlgoAPI_Section for drawings (HLRBRep for views).
- **Priority:** P1 (required for drawing section views; owned with Drawings)
