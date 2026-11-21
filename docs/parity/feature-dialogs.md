# Part Studio feature dialogs against live (psui, from 2026-11-21)

Lead 11-21 05:3x: for each Part Studio feature dialog, field order, defaults, units, toggles and error texts against
live; differences fixed. Earlier, narrower passes: p1-dialogs.md (the P1 dialogs by eye and specs, 10-26),
audit-part-studio.md (every dialog's editing logic against std through `std_oracle::print_all`), error-wording.md
(22 failing-feature situations taken in live 11-18).

Method:
- **Specs.** Live's `/featurespecs` (onshape-research/live/psui/dlg-1121/featurespecs.json, std 3132; the first pass
  used p1audit's std 3095 dump of 10-26) against our `feature_schema {"type", "all": true}` on green, parameter by parameter:
  ids (live's ALWAYS_HIDDEN ones left out), their order, labels, defaults in mm / deg, booleans, enum options and
  option names. Script: /root/psui-scratch/dlg/specdiff.py <noshape> <out.json> [<live specs>].
- **Empty error.** Each feature added with nothing selected to a 40 × 30 × 10 mm block (`feature add --params {}`):
  its status and message, the dialog title's error. Script: /root/psui-scratch/dlg/empty_errors.py. Live's: by REST
  (dlg-1121/empty-errors.json).
- Table: /root/psui-scratch/dlg/gen_table.py specdiff.json ours-empty.json [live empty-errors.json].

Result on green 4553fc38a (11-21 10:4x), against live's fresh dump (live/psui/dlg-1121/featurespecs.json, std 3132)
and live's empty errors by REST (dlg-1121/empty-errors.json): 88 feature types, 86 with a live spec. 80 equal in every
field (ids, order, labels, defaults, toggles, options); 85 of 87 empty errors equal (Form tool: no live answer). Left:
- Hole: field order from unitsSystem on (live's V2 diameter / tolerance fields): our std is 3112, live's 3132 (hole.fs
  is one of the 14 files that differ; reported to lead 11-21).
- Jog (bendAngleReadOnly / bendAngle) and Tag (pcbEntityType / standardContentPart): order of fields that are never
  shown together; the dialogs look the same.
- Import: Import appearances on in ours and in live's dialog (live's spec default False is not what its UI shows).
- Recognize (in neither toolbar): Thickness / Minimal gap 25 mm in live's spec, 1.6 / 0.02 mm in ours.
- Variable: Conversion factor default 0 in live's spec, 1 in ours (to judge: hidden, used only for unit conversion).
- Frame empty error: live's REST "Select profile." comes from the spec skipping the library profile default; live's
  dialog says ours, "Select sweep paths.".
- Tag empty error: std (tag.fs) names the faulty field instanceFunction, which Tag doesn't have; live's dialog marks no
  field, as ours.
Form tool and Sphere have no live spec (library / custom features).

Status: = same, ≠ differs.

| Feature | Type | Fields (live spec) | Order | Labels | Defaults, units, toggles, options | Empty error, ours | Empty error, live | Status |
|---|---|---|---|---|---|---|---|---|
| 3D fit spline | fitSpline | 20 (4 sel, 6 val, 9 tog, 1 list) | = | = | = | Select at least two vertices to make a spline. | Select at least two vertices to make a spline. | = spec, = error |
| Bend | sheetMetalBend | 15 (4 sel, 4 val, 5 tog, 2 list) | = | = | = | Select a line. | Select a line. | = spec, = error |
| Bend relief | sheetMetalBendRelief | 6 (1 sel, 3 val, 1 tog, 1 list) | = | = | = | Select a bend relief to modify. | Select a bend relief to modify. | = spec, = error |
| Body draft | bodyDraft | 18 (7 sel, 2 val, 5 tog, 4 list) | = | = | = | Select parts to draft. | Select parts to draft. | = spec, = error |
| Boolean | booleanBodies | 10 (3 sel, 1 val, 5 tog, 1 list) | = | = | = | Cannot resolve entities. | Cannot resolve entities. | = spec, = error |
| Boundary surface | boundarySurface | 7 (1 sel, 1 val, 2 tog, 1 list) | = | = | = | Select edges to specify at least two profiles. | Select edges to specify at least two profiles. | = spec, = error |
| Bridging curve | bridgingCurve | 22 (2 sel, 12 val, 5 tog, 3 list) | = | = | = | Select a vertex or edge at the start position, and optionally an edge or face with the starting direction/curvature. | Select a vertex or edge at the start position, and optionally an edge or face with the starting direction/curvature. | = spec, = error |
| Chamfer | chamfer | 10 (2 sel, 4 val, 2 tog, 2 list) | = | = | = | Select edges or faces to chamfer. | Select edges or faces to chamfer. | = spec, = error |
| Circular pattern | circularPattern | 16 (4 sel, 2 val, 6 tog, 2 list) | = | = | = | Select parts to pattern. | Select parts to pattern. | = spec, = error |
| Composite curve | compositeCurve | 9 (1 sel, 4 val, 4 tog, 0 list) | = | = | = | At least one edge is required. | At least one edge is required. | = spec, = error |
| Composite part | compositePart | 2 (1 sel, 0 val, 1 tog, 0 list) | = | = | = | Select entities to create a composite part. | Select entities to create a composite part. | = spec, = error |
| Constrained surface | constrainedSurface | 10 (1 sel, 3 val, 2 tog, 3 list) | = | = | = | Select at least three distinct points. | Select at least three distinct points. | = spec, = error |
| Copy part | copyPart | 1 (1 sel, 0 val, 0 tog, 0 list) | = | = | = | Select parts to copy. | Select parts to copy. | = spec, = error |
| Corner | sheetMetalCorner | 5 (1 sel, 3 val, 0 tog, 1 list) | = | = | = | Select a corner to modify. | Select a corner to modify. | = spec, = error |
| Corner break | sheetMetalCornerBreak | 21 (3 sel, 9 val, 4 tog, 5 list) | = | = | = | Select sheet metal edges or vertices to fillet or chamfer. | Select sheet metal edges or vertices to fillet or chamfer. | = spec, = error |
| Cube | cube | 1 (0 sel, 1 val, 0 tog, 0 list) | = | = | = | (none: builds) | (none: builds) | = spec, = error |
| Curve pattern | curvePattern | 16 (5 sel, 2 val, 3 tog, 4 list) | = | = | = | Select parts to pattern. | Select parts to pattern. | = spec, = error |
| Cut list | cutlist | 3 (1 sel, 0 val, 1 tog, 0 list) | = | = | = | Select frames to create a cutlist | Select frames to create a cutlist | = spec, = error |
| Decal | decal | 13 (3 sel, 5 val, 3 tog, 1 list) | = | = | = | Select face for decal | Select face for decal | = spec, = error |
| Delete face | deleteFace | 3 (1 sel, 0 val, 1 tog, 1 list) | = | = | = | Select faces to delete. | Select faces to delete. | = spec, = error |
| Delete part | deleteBodies | 3 (2 sel, 0 val, 0 tog, 1 list) | = | = | = | Select entities to delete. | Select entities to delete. | = spec, = error |
| Derived | importDerived | 6 (1 sel, 0 val, 3 tog, 1 list) | = | = | = | Select entities from another Part Studio. | Select entities from another Part Studio. | = spec, = error |
| Derived mirror | derivedMirror | 1 (0 sel, 0 val, 0 tog, 0 list) | = | = | = | Failed to recreate derived objects. | Failed to recreate derived objects. | = spec, = error |
| Draft | draft | 14 (4 sel, 2 val, 6 tog, 2 list) | = | = | = | Select a neutral plane. | Select a neutral plane. | = spec, = error |
| Edit curve | editCurve | 24 (2 sel, 9 val, 9 tog, 2 list) | = | = | = | Select curves. | Select curves. | = spec, = error |
| Enclose | enclose | 5 (2 sel, 0 val, 2 tog, 1 list) | = | = | = | Select faces, parts, or planes to form a solid. | Select faces, parts, or planes to form a solid. | = spec, = error |
| End cap | endcap | 11 (1 sel, 6 val, 2 tog, 2 list) | = | = | = | Select frame segment start or end face to create endcap. | Select frame segment start or end face to create endcap. | = spec, = error |
| External thread | externalThread | 11 (1 sel, 5 val, 3 tog, 1 list) | = | = | = | Select one or more cylinder edges. | Select one or more cylinder edges. | = spec, = error |
| Extrude | extrude | 49 (13 sel, 10 val, 19 tog, 7 list) | = | = | = | Select face or sketch region to extrude. | Select face or sketch region to extrude. | = spec, = error |
| Face blend | faceBlend | 42 (12 sel, 8 val, 15 tog, 5 list) | = | = | = | Select faces to blend. | Select faces to blend. | = spec, = error |
| Fill | fill | 10 (2 sel, 2 val, 3 tog, 2 list) | = | = | = | Select laminar edges or curves as fill surface boundary. | Select laminar edges or curves as fill surface boundary. | = spec, = error |
| Fillet | fillet | 37 (8 sel, 10 val, 12 tog, 5 list) | = | = | = | Select edges or faces to fillet. | Select edges or faces to fillet. | = spec, = error |
| Finish flex PCB model | pcbEnd | 4 (3 sel, 0 val, 0 tog, 1 list) | = | = | = | No part selected to unfold. | No part selected to unfold. | = spec, = error |
| Finish sheet metal model | sheetMetalEnd | 1 (1 sel, 0 val, 0 tog, 0 list) | = | = | = | Select one or more sheet metal parts. | Select one or more sheet metal parts. | = spec, = error |
| Flange | sheetMetalFlange | 33 (6 sel, 10 val, 11 tog, 6 list) | = | = | = | Select edges to flange. | Select edges to flange. | = spec, = error |
| Flex PCB model | pcbStart | 36 (12 sel, 8 val, 13 tog, 3 list) | = | = | = | Cannot resolve entities. | Cannot resolve entities. | = spec, = error |
| Form | sheetMetalFormed | 4 (2 sel, 0 val, 1 tog, 0 list) | = | = | = | Select a location to place the form feature. | Select a location to place the form feature. | = spec, = error |
| Form tool | smFormTool | no live spec | no live spec | no live spec | no live spec | Unknown library form | — | no live spec, no live answer |
| Frame | frame | 14 (5 sel, 1 val, 5 tog, 1 list) | = | = | = | Select sweep paths. ≠fields | Select profile. (fields profileSketch; ours selections) | = spec, ≠ error |
| Frame trim | frameTrim | 6 (2 sel, 0 val, 2 tog, 1 list) | = | = | = | Select at least two frame groups. | Select at least two frame groups. | = spec, = error |
| Gusset | gusset | 12 (2 sel, 4 val, 4 tog, 2 list) | = | = | = | Select linear edge on the frame segment cut face to create gusset. | Select linear edge on the frame segment cut face to create gusset. | = spec, = error |
| Helix | helix | 24 (5 sel, 8 val, 6 tog, 5 list) | = | = | = | Select a cone or cylinder. | Select a cone or cylinder. | = spec, = error |
| Hem | sheetMetalHem | 11 (1 sel, 4 val, 3 tog, 3 list) | = | = | = | Select edges to hem. | Select edges to hem. | = spec, = error |
| Hole | hole | 149 (4 sel, 93 val, 6 tog, 33 list); live only: holeDiameterV2, holeDiameterV2Precision, holeDiameterV2ToleranceType, holeDiameterV2ToleranceBoundSymmetrical, holeDiameterV2ToleranceBoundDeviationUpper, holeDiameterV2ToleranceBoundDeviationLower…; ours only: asVersion, blindInLast, tableSynced, groupedSketches | ≠ first at unitsSystem / style | = | = | No hole points selected. | No hole points selected. | ≠ spec, = error |
| Import | importForeign | 6 (0 sel, 0 val, 5 tog, 1 list); ours only: partInfo, joinAdjacentSurfaces | = | = | ≠ importAppearances default: False / True | Parasolid data import failed. | Parasolid data import failed. | ≠ spec, = error |
| Intersection curve | intersectionCurve | 10 (2 sel, 4 val, 4 tog, 0 list) | = | = | = | Cannot resolve entities. | Cannot resolve entities. | = spec, = error |
| Isocline | isocline | 5 (2 sel, 1 val, 2 tog, 0 list) | = | = | = | Select faces to create isoclines. | Select faces to create isoclines. | = spec, = error |
| Isoparametric curve | isoparametricCurve | 5 (1 sel, 1 val, 1 tog, 1 list) | = | = | = | Select face to create isoparametric curve. | Select face to create isoparametric curve. | = spec, = error |
| Jog | sheetMetalJog | 25 (5 sel, 8 val, 8 tog, 4 list) | ≠ first at bendAngleReadOnly / bendAngle | = | = | Select a line. | Select a line. | ≠ spec, = error |
| Linear pattern | linearPattern | 21 (5 sel, 4 val, 8 tog, 2 list) | = | = | = | Select parts to pattern. | Select parts to pattern. | = spec, = error |
| Loft | loft | 34 (7 sel, 7 val, 11 tog, 5 list); ours only: asVersion | = | = | = | Select profiles in order from start to finish. | Select profiles in order from start to finish. | = spec, = error |
| Make joint | sheetMetalMakeJoint | 5 (1 sel, 1 val, 1 tog, 2 list) | = | = | = | Selected edges should be from the same active sheet metal model. | Selected edges should be from the same active sheet metal model. | = spec, = error |
| Mate connector | mateConnector | 18 (6 sel, 4 val, 4 tog, 4 list) | = | = | = | Cannot resolve entities. | Cannot resolve entities. | = spec, = error |
| Mirror | mirror | 9 (4 sel, 0 val, 2 tog, 2 list) | = | = | = | Select parts to mirror. | Select parts to mirror. | = spec, = error |
| Modify fillet | modifyFillet | 4 (1 sel, 1 val, 1 tog, 1 list) | = | = | = | Select fillet faces to modify. | Select fillet faces to modify. | = spec, = error |
| Modify joint | sheetMetalJoint | 10 (1 sel, 4 val, 2 tog, 3 list) | = | = | = | Select a joint from an active sheet metal model. | Select a joint from an active sheet metal model. | = spec, = error |
| Move boundary | extendSurface | 12 (4 sel, 2 val, 5 tog, 1 list) | = | = | = | Select a surface or boundary edges to move. | Select a surface or boundary edges to move. | = spec, = error |
| Move face | moveFace | 16 (6 sel, 4 val, 4 tog, 2 list) | = | = | = | Select faces to move. | Select faces to move. | = spec, = error |
| Mutual trim | mutualTrim | 5 (2 sel, 0 val, 3 tog, 0 list) | = | = | = | Select a surface. | Select a surface. | = spec, = error |
| Name entity | nameEntity | 2 (1 sel, 0 val, 0 tog, 0 list) | = | = | = | Invalid input selections. | Invalid input selections. | = spec, = error |
| Offset curve | offsetCurveOnFace | 19 (2 sel, 7 val, 7 tog, 3 list) | = | = | = | Select edges to offset. | Select edges to offset. | = spec, = error |
| Offset surface | offsetSurface | 3 (1 sel, 1 val, 1 tog, 0 list) | = | = | = | Select faces, surfaces, or sketch regions to offset from. | Select faces, surfaces, or sketch regions to offset from. | = spec, = error |
| Plane | cPlane | 7 (1 sel, 2 val, 3 tog, 1 list) | = | = | = | Offset plane requires a plane, circle, ellipse, or arc to offset from. | Offset plane requires a plane, circle, ellipse, or arc to offset from. | = spec, = error |
| Point | cPoint | 7 (2 sel, 4 val, 0 tog, 1 list) | = | = | = | Cannot resolve entities. | Cannot resolve entities. | = spec, = error |
| Projected curve | projectCurves | 16 (5 sel, 4 val, 5 tog, 2 list) | = | = | = | Cannot resolve entities. | Cannot resolve entities. | = spec, = error |
| Query variable | queryVariable | 25 (7 sel, 0 val, 5 tog, 9 list) | = | = | = | Variable name must be a letter followed by a string of letters or numbers. | Variable name must be a letter followed by a string of letters or numbers. | = spec, = error |
| Recognize | sheetMetalRecognize | 7 (1 sel, 4 val, 2 tog, 0 list) | = | = | ≠ surfaceThickness default: 25 mm / 1.6 mm; minimalClearance default: 25 mm / 0.02 mm | Cannot resolve entities. | Cannot resolve entities. | ≠ spec, = error |
| Refold | sheetMetalRefold | 1 (1 sel, 0 val, 0 tog, 0 list) | = | = | = | Selected parts should be from the same active sheet metal model. | Selected parts should be from the same active sheet metal model. | = spec, = error |
| Replace face | replaceFace | 5 (2 sel, 1 val, 2 tog, 0 list) | = | = | = | Select faces to replace. | Select faces to replace. | = spec, = error |
| Revolve | revolve | 37 (12 sel, 7 val, 13 tog, 5 list) | = | = | = | Select face or sketch region to revolve. | Select face or sketch region to revolve. | = spec, = error |
| Rib | rib | 10 (2 sel, 2 val, 5 tog, 1 list) | = | = | = | Select sketch profiles to create the ribs. | Select sketch profiles to create the ribs. | = spec, = error |
| Rip | sheetMetalRip | 3 (1 sel, 1 val, 1 tog, 0 list) | = | = | = | Selected parts should be from the same active sheet metal model. | Selected parts should be from the same active sheet metal model. | = spec, = error |
| Routing curve | routingCurve | 33 (5 sel, 5 val, 6 tog, 7 list) | = | = | = | Create at least two non-coincident points. | Create at least two non-coincident points. | = spec, = error |
| Ruled surface | ruledSurface | 18 (4 sel, 4 val, 6 tog, 3 list) | = | = | = | Select edges and curves to create a ruled surface. | Select edges and curves to create a ruled surface. | = spec, = error |
| Sheet metal loft | sheetMetalLoft | 22 (3 sel, 11 val, 3 tog, 4 list) | = | = | = | Select profiles for sheet metal loft. | Select profiles for sheet metal loft. | = spec, = error |
| Sheet metal model | sheetMetalStart | 46 (12 sel, 15 val, 13 tog, 6 list); ours only: parameterLibraryPurposeId | = | = | = | Cannot resolve entities. | Cannot resolve entities. | = spec, = error |
| Shell | shell | 5 (2 sel, 1 val, 2 tog, 0 list) | = | = | = | Select faces to remove. | Select faces to remove. | = spec, = error |
| Sphere | fSphere | no live spec | no live spec | no live spec | no live spec | (none: builds) | (none: builds) | no live spec, = error |
| Split | splitPart | 13 (5 sel, 0 val, 6 tog, 2 list) | = | = | = | Select parts, surfaces, or curves to split. | Select parts, surfaces, or curves to split. | = spec, = error |
| Sweep | sweep | 30 (8 sel, 7 val, 10 tog, 5 list) | = | = | = | Select entities for sweep path. | Select entities for sweep path. | = spec, = error |
| Tab | sheetMetalTab | 4 (3 sel, 1 val, 0 tog, 0 list) | = | = | = | Select a face for the tab. | Select a face for the tab. | = spec, = error |
| Tag | tag | 16 (6 sel, 0 val, 2 tog, 2 list) | ≠ first at pcbEntityType / standardContentPart | = | = | Select a single sketch feature. The selected feature must create sketch regions. ≠fields | Select a single sketch feature. The selected feature must create sketch regions. (fields instanceFunction; ours none) | ≠ spec, ≠ error |
| Thicken | thicken | 10 (2 sel, 3 val, 4 tog, 1 list) | = | = | = | Select surfaces or faces to thicken. | Select surfaces or faces to thicken. | = spec, = error |
| Transform | transform | 23 (7 sel, 9 val, 5 tog, 2 list) | = | = | = | Cannot resolve entities. | Cannot resolve entities. | = spec, = error |
| Trim curve | trimCurve | 9 (4 sel, 1 val, 1 tog, 3 list) | = | = | = | Select a curve to adjust. | Select a curve to adjust. | = spec, = error |
| Unfold | sheetMetalUnfold | 4 (3 sel, 0 val, 1 tog, 0 list) | = | = | = | No part selected to unfold. | No part selected to unfold. | = spec, = error |
| Variable | assignVariable | 42 (4 sel, 18 val, 6 tog, 9 list) | = | = | ≠ conversionFactor default: 0 / 1 | Variable name must be a letter followed by a string of letters or numbers. | Variable name must be a letter followed by a string of letters or numbers. | ≠ spec, = error |
| Wrap | wrap | 19 (5 sel, 4 val, 8 tog, 2 list) | = | = | = | Select planar faces or sketch regions to wrap. | Select planar faces or sketch regions to wrap. | = spec, = error |
