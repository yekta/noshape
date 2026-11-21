# Part Studio feature dialogs against live (psui, from 2026-11-21)

Lead 11-21 05:3x: for each Part Studio feature dialog, field order, defaults, units, toggles and error texts against
live; differences fixed. Earlier, narrower passes: p1-dialogs.md (the P1 dialogs by eye and specs, 10-26),
audit-part-studio.md (every dialog's editing logic against std through `std_oracle::print_all`), error-wording.md
(22 failing-feature situations taken in live 11-18).

Method:
- **Specs.** Live's `/featurespecs` (onshape-research/live/p1audit/featurespecs.json, std 3095, 10-26; a fresh dump is
  asked in REQUESTS "psui dlg-1") against our `feature_schema {"type", "all": true}` on green, parameter by parameter:
  ids (live's ALWAYS_HIDDEN ones left out), their order, labels, defaults in mm / deg, booleans, enum options and
  option names. Script: /root/psui-scratch/dlg/specdiff.py <noshape> <out.json>.
- **Empty error.** Each feature added with nothing selected to a 40 × 30 × 10 mm block (`feature add --params {}`):
  its status and message, the dialog title's error. Script: /root/psui-scratch/dlg/empty_errors.py. Live's: by REST,
  asked in dlg-1.
- Table: /root/psui-scratch/dlg/gen_table.py specdiff.json ours-empty.json [live empty-errors.json].

Result on green 052681501 (11-21): 89 feature types. Against the 3095 specs 83 are equal in every field (ids, order,
labels, defaults, toggles, options). The 6 others:
- Hole, Jog, Tag: our std is 3112, live's dump 3095 (Hole's V2 → V3 fields, Jog's bendAngleReadOnly, Tag's Standard
  content purpose): version skew, rechecked on the fresh dump.
- Import: ours lists the import's stored fields (blob, units, …), live hides them; its unit options in another order
  (live Centimeter, Foot, Inch, Millimeter, Meter, Yard; ours Meter, Centimeter, Millimeter, Inch, Foot, Yard); Import
  appearances default off in live, on in ours.
- Recognize (hidden in both toolbars): Thickness and Minimal gap default 25 mm in live's spec, 1.6 / 0.02 in ours.
- Variable: Conversion factor default 0 in live, 1 in ours.
Form tool and Sphere have no live spec (library / custom features).

Status: = same, ≠ differs, pending = live's answer asked.

| Feature | Type | Fields (live spec) | Order | Labels | Defaults, units, toggles, options | Empty error, ours | Empty error, live | Status |
|---|---|---|---|---|---|---|---|---|
| 3D fit spline | fitSpline | 20 (4 sel, 6 val, 9 tog, 1 list); ours only: initEntities, hasStartDirection, hasEndDirection | = | = | = | Select at least two vertices to make a spline. | pending | = spec, error pending (dlg-1) |
| Bend | sheetMetalBend | 15 (4 sel, 4 val, 5 tog, 2 list) | = | = | = | Select a line. | pending | = spec, error pending (dlg-1) |
| Bend relief | sheetMetalBendRelief | 6 (1 sel, 3 val, 1 tog, 1 list) | = | = | = | Select a bend relief to modify. | pending | = spec, error pending (dlg-1) |
| Body draft | bodyDraft | 18 (7 sel, 2 val, 5 tog, 4 list); ours only: preselection | = | = | = | Select parts to draft. | pending | = spec, error pending (dlg-1) |
| Boolean | booleanBodies | 10 (3 sel, 1 val, 5 tog, 1 list) | = | = | = | Cannot resolve entities. | pending | = spec, error pending (dlg-1) |
| Boundary surface | boundarySurface | 7 (1 sel, 1 val, 2 tog, 1 list) | = | = | = | Select edges to specify at least two profiles. | pending | = spec, error pending (dlg-1) |
| Bridging curve | bridgingCurve | 22 (2 sel, 12 val, 5 tog, 3 list); ours only: preselectedEntities, side1HasVertex, side2HasVertex, side1HasFace, side2HasFace | = | = | = | Select a vertex or edge at the start position, and optionally an edge or face with the starting direction/curvature. | pending | = spec, error pending (dlg-1) |
| Chamfer | chamfer | 10 (2 sel, 4 val, 2 tog, 2 list) | = | = | = | Select edges or faces to chamfer. | pending | = spec, error pending (dlg-1) |
| Circular pattern | circularPattern | 16 (4 sel, 2 val, 6 tog, 2 list) | = | = | = | Select parts to pattern. | pending | = spec, error pending (dlg-1) |
| Composite curve | compositeCurve | 9 (1 sel, 4 val, 4 tog, 0 list) | = | = | = | At least one edge is required. | pending | = spec, error pending (dlg-1) |
| Composite part | compositePart | 2 (1 sel, 0 val, 1 tog, 0 list) | = | = | = | Select entities to create a composite part. | pending | = spec, error pending (dlg-1) |
| Constrained surface | constrainedSurface | 10 (1 sel, 3 val, 2 tog, 3 list) | = | = | = | Select at least three distinct points. | pending | = spec, error pending (dlg-1) |
| Copy part | copyPart | 1 (1 sel, 0 val, 0 tog, 0 list) | = | = | = | Select parts to pattern. | pending | = spec, error pending (dlg-1) |
| Corner | sheetMetalCorner | 5 (1 sel, 3 val, 0 tog, 1 list) | = | = | = | Select a corner to modify. | pending | = spec, error pending (dlg-1) |
| Corner break | sheetMetalCornerBreak | 21 (3 sel, 9 val, 4 tog, 5 list) | = | = | = | Select sheet metal edges or vertices to fillet or chamfer. | pending | = spec, error pending (dlg-1) |
| Cube | cube | 1 (0 sel, 1 val, 0 tog, 0 list) | = | = | = | (none: builds) | pending | = spec, error pending (dlg-1) |
| Curve pattern | curvePattern | 16 (5 sel, 2 val, 3 tog, 4 list) | = | = | = | Select parts to pattern. | pending | = spec, error pending (dlg-1) |
| Cut list | cutlist | 3 (1 sel, 0 val, 1 tog, 0 list) | = | = | = | Select frames to create a cutlist | pending | = spec, error pending (dlg-1) |
| Decal | decal | 13 (3 sel, 5 val, 3 tog, 1 list); ours only: imageOriginIndex | = | = | = | Select face for decal | pending | = spec, error pending (dlg-1) |
| Delete face | deleteFace | 3 (1 sel, 0 val, 1 tog, 1 list) | = | = | = | Select faces to delete. | pending | = spec, error pending (dlg-1) |
| Delete part | deleteBodies | 3 (2 sel, 0 val, 0 tog, 1 list) | = | = | = | Select entities to delete. | pending | = spec, error pending (dlg-1) |
| Derived | importDerived | 6 (1 sel, 0 val, 3 tog, 1 list); ours only: newUI, mateConnectorIndex, mateConnectorId, mateConnectorIndexInFeature, parts | = | = | = | Select entities from another Part Studio. | pending | = spec, error pending (dlg-1) |
| Derived mirror | derivedMirror | 1 (0 sel, 0 val, 0 tog, 0 list) | = | = | = | Error regenerating. | pending | = spec, error pending (dlg-1) |
| Draft | draft | 14 (4 sel, 2 val, 6 tog, 2 list); ours only: hintFaces | = | = | = | Select a neutral plane. | pending | = spec, error pending (dlg-1) |
| Edit curve | editCurve | 24 (2 sel, 9 val, 9 tog, 2 list) | = | = | = | Select curves. | pending | = spec, error pending (dlg-1) |
| Enclose | enclose | 5 (2 sel, 0 val, 2 tog, 1 list) | = | = | = | Select faces, parts, or planes to form a solid. | pending | = spec, error pending (dlg-1) |
| End cap | endcap | 11 (1 sel, 6 val, 2 tog, 2 list) | = | = | = | Select frame segment start or end face to create endcap. | pending | = spec, error pending (dlg-1) |
| External thread | externalThread | 11 (1 sel, 5 val, 3 tog, 1 list) | = | = | = | Select one or more cylinder edges. | pending | = spec, error pending (dlg-1) |
| Extrude | extrude | 49 (13 sel, 10 val, 19 tog, 7 list); ours only: domain | = | = | = | Select face or sketch region to extrude. | pending | = spec, error pending (dlg-1) |
| Face blend | faceBlend | 42 (12 sel, 8 val, 15 tog, 5 list); ours only: crossSectionGroup, constraintsAndLimitsGroup | = | = | = | Select faces to blend. | pending | = spec, error pending (dlg-1) |
| Fill | fill | 10 (2 sel, 2 val, 3 tog, 2 list); ours only: preselectedEntities | = | = | = | Select laminar edges or curves as fill surface boundary. | pending | = spec, error pending (dlg-1) |
| Fillet | fillet | 37 (8 sel, 10 val, 12 tog, 5 list) | = | = | = | Select edges or faces to fillet. | pending | = spec, error pending (dlg-1) |
| Finish flex PCB model | pcbEnd | 4 (3 sel, 0 val, 0 tog, 1 list); ours only: initEntities | = | = | = | No part selected to unfold. | pending | = spec, error pending (dlg-1) |
| Finish sheet metal model | sheetMetalEnd | 1 (1 sel, 0 val, 0 tog, 0 list) | = | = | = | Select one or more sheet metal parts. | pending | = spec, error pending (dlg-1) |
| Flange | sheetMetalFlange | 33 (6 sel, 10 val, 11 tog, 6 list); ours only: overallParametersGroup, endConditionsGroup | = | = | = | Select edges to flange. | pending | = spec, error pending (dlg-1) |
| Flex PCB model | pcbStart | 36 (12 sel, 8 val, 13 tog, 3 list); ours only: initEntities, selectionsGroup | = | = | = | Cannot resolve entities. | pending | = spec, error pending (dlg-1) |
| Form | sheetMetalFormed | 4 (2 sel, 0 val, 1 tog, 0 list) | = | = | = | Select a location to place the form feature. | pending | = spec, error pending (dlg-1) |
| Form tool | smFormTool | no live spec | no live spec | no live spec | no live spec | Unknown library form | pending | no live spec, error pending (dlg-1) |
| Frame | frame | 14 (5 sel, 1 val, 5 tog, 1 list); ours only: index | = | = | = | Select sweep paths. | pending | = spec, error pending (dlg-1) |
| Frame trim | frameTrim | 6 (2 sel, 0 val, 2 tog, 1 list) | = | = | = | Select at least two frame groups. | pending | = spec, error pending (dlg-1) |
| Gusset | gusset | 12 (2 sel, 4 val, 4 tog, 2 list); ours only: baseSweptFaces | = | = | = | Select linear edge on the frame segment cut face to create gusset. | pending | = spec, error pending (dlg-1) |
| Helix | helix | 24 (5 sel, 8 val, 6 tog, 5 list); ours only: initEntities | = | = | = | Select a cone or cylinder. | pending | = spec, error pending (dlg-1) |
| Hem | sheetMetalHem | 11 (1 sel, 4 val, 3 tog, 3 list) | = | = | = | Select edges to hem. | pending | = spec, error pending (dlg-1) |
| Hole | hole | 149 (4 sel, 93 val, 6 tog, 33 list); live only: holeDiameterV2, holeDiameterV2Precision, holeDiameterV2ToleranceType, holeDiameterV2ToleranceBoundSymmetrical, holeDiameterV2ToleranceBoundDeviationUpper, holeDiameterV2ToleranceBoundDeviationLower…; ours only: isV2, holeVersion, initEntities, featureName, asVersion, threadStandard, hasClearance, blindInLast, showTappedDepth, majorDiameter, isMultiple, tappedAngle, tableSynced, groupedSketches | ≠ first at unitsSystem / style | = | = | No hole points selected. | pending | ≠ spec, error pending (dlg-1) |
| Import | importForeign | 6 (0 sel, 0 val, 5 tog, 1 list); ours only: dependsOnBlob, blobData, partInfo, foreignId, specifyMaterialData, specifyUnits, originalUnit, flatten, maxAssembliesToCreate, isInContext, joinAdjacentSurfaces, compositeName, fixSingleCompositeEmptyName | = | = | ≠ importAppearances default: False / True; unit options: ['Centimeter', 'Foot', 'Inch', 'Millimeter', 'Meter', 'Yard'] / ['Meter', 'Centimeter', 'Millimeter', 'Inch', 'Foot', 'Yard'] | Part import failed. | pending | ≠ spec, error pending (dlg-1) |
| Intersection curve | intersectionCurve | 10 (2 sel, 4 val, 4 tog, 0 list) | = | = | = | Cannot resolve entities. | pending | = spec, error pending (dlg-1) |
| Isocline | isocline | 5 (2 sel, 1 val, 2 tog, 0 list) | = | = | = | Select faces to create isoclines. | pending | = spec, error pending (dlg-1) |
| Isoparametric curve | isoparametricCurve | 5 (1 sel, 1 val, 1 tog, 1 list) | = | = | = | Select face to create isoparametric curve. | pending | = spec, error pending (dlg-1) |
| Jog | sheetMetalJog | 25 (5 sel, 8 val, 8 tog, 4 list); ours only: bendAngleSetProgrammatically | ≠ first at bendAngleReadOnly / bendAngle | = | = | Select a line. | pending | ≠ spec, error pending (dlg-1) |
| Linear pattern | linearPattern | 21 (5 sel, 4 val, 8 tog, 2 list) | = | = | = | Select parts to pattern. | pending | = spec, error pending (dlg-1) |
| Loft | loft | 34 (7 sel, 7 val, 11 tog, 5 list); ours only: endConditionsGroup, makePeriodic, asVersion | = | = | = | Select profiles in order from start to finish. | pending | = spec, error pending (dlg-1) |
| Make joint | sheetMetalMakeJoint | 5 (1 sel, 1 val, 1 tog, 2 list) | = | = | = | Selected edges should be from the same active sheet metal model. | pending | = spec, error pending (dlg-1) |
| Mate connector | mateConnector | 18 (6 sel, 4 val, 4 tog, 4 list); ours only: entityInferenceType, secondaryOriginQuery, allowOwnerEntity, specifyNormal, nx, ny, nz, isForSubFeature | = | = | = | Cannot resolve entities. | pending | = spec, error pending (dlg-1) |
| Mirror | mirror | 9 (4 sel, 0 val, 2 tog, 2 list) | = | = | = | Select parts to mirror. | pending | = spec, error pending (dlg-1) |
| Modify fillet | modifyFillet | 4 (1 sel, 1 val, 1 tog, 1 list) | = | = | = | Select fillet faces to modify. | pending | = spec, error pending (dlg-1) |
| Modify joint | sheetMetalJoint | 10 (1 sel, 4 val, 2 tog, 3 list); ours only: hasStyle | = | = | = | Select a joint from an active sheet metal model. | pending | = spec, error pending (dlg-1) |
| Move boundary | extendSurface | 12 (4 sel, 2 val, 5 tog, 1 list) | = | = | = | Select a surface or boundary edges to move. | pending | = spec, error pending (dlg-1) |
| Move face | moveFace | 16 (6 sel, 4 val, 4 tog, 2 list); ours only: outputType | = | = | = | Select faces to move. | pending | = spec, error pending (dlg-1) |
| Mutual trim | mutualTrim | 5 (2 sel, 0 val, 3 tog, 0 list) | = | = | = | Select a surface. | pending | = spec, error pending (dlg-1) |
| Name entity | nameEntity | 2 (1 sel, 0 val, 0 tog, 0 list) | = | = | = | (none: builds) | pending | = spec, error pending (dlg-1) |
| Offset curve | offsetCurveOnFace | 19 (2 sel, 7 val, 7 tog, 3 list) | = | = | = | Select edges to offset. | pending | = spec, error pending (dlg-1) |
| Offset surface | offsetSurface | 3 (1 sel, 1 val, 1 tog, 0 list) | = | = | = | Select faces, surfaces, or sketch regions to offset from. | pending | = spec, error pending (dlg-1) |
| Plane | cPlane | 7 (1 sel, 2 val, 3 tog, 1 list); ours only: width, height | = | = | = | Offset plane requires a plane, circle, ellipse, or arc to offset from. | pending | = spec, error pending (dlg-1) |
| Point | cPoint | 7 (2 sel, 4 val, 0 tog, 1 list) | = | = | = | Cannot resolve entities. | pending | = spec, error pending (dlg-1) |
| Projected curve | projectCurves | 16 (5 sel, 4 val, 5 tog, 2 list); ours only: preselectedEntities | = | = | = | Cannot resolve entities. | pending | = spec, error pending (dlg-1) |
| Query variable | queryVariable | 25 (7 sel, 0 val, 5 tog, 9 list) | = | = | = | Variable name must be a letter followed by a string of letters or numbers. | pending | = spec, error pending (dlg-1) |
| Recognize | sheetMetalRecognize | 7 (1 sel, 4 val, 2 tog, 0 list) | = | = | ≠ surfaceThickness default: 25 mm / 1.6 mm; minimalClearance default: 25 mm / 0.02 mm | (none: builds) | pending | ≠ spec, error pending (dlg-1) |
| Refold | sheetMetalRefold | 1 (1 sel, 0 val, 0 tog, 0 list) | = | = | = | Selected parts should be from the same active sheet metal model. | pending | = spec, error pending (dlg-1) |
| Replace face | replaceFace | 5 (2 sel, 1 val, 2 tog, 0 list); ours only: showFlip | = | = | = | Select faces to replace. | pending | = spec, error pending (dlg-1) |
| Revolve | revolve | 37 (12 sel, 7 val, 13 tog, 5 list) | = | = | = | Select face or sketch region to revolve. | pending | = spec, error pending (dlg-1) |
| Rib | rib | 10 (2 sel, 2 val, 5 tog, 1 list) | = | = | = | Select sketch profiles to create the ribs. | pending | = spec, error pending (dlg-1) |
| Rip | sheetMetalRip | 3 (1 sel, 1 val, 1 tog, 0 list) | = | = | = | Selected parts should be from the same active sheet metal model. | pending | = spec, error pending (dlg-1) |
| Routing curve | routingCurve | 31 (5 sel, 5 val, 6 tog, 7 list); ours only: preselectedEntities, curveLength, inputsGroup, multiDx, multiDy, multiDz, multiRotationMatrix, pointIndex, segmentIndex, solutionIndex | = | = | = | Create at least two non-coincident points. | pending | = spec, error pending (dlg-1) |
| Ruled surface | ruledSurface | 18 (4 sel, 4 val, 6 tog, 3 list) | = | = | = | Select edges and curves to create a ruled surface. | pending | = spec, error pending (dlg-1) |
| Sheet metal loft | sheetMetalLoft | 22 (3 sel, 11 val, 3 tog, 4 list); ours only: generalGroup, materialGroup, reliefGroup | = | = | = | Select profiles for sheet metal loft. | pending | = spec, error pending (dlg-1) |
| Sheet metal model | sheetMetalStart | 46 (12 sel, 15 val, 13 tog, 6 list); ours only: initEntities, selectionsGroup, generalGroup, materialGroup, reliefGroup, parameterLibraryPurposeId | = | = | = | Cannot resolve entities. | pending | = spec, error pending (dlg-1) |
| Shell | shell | 5 (2 sel, 1 val, 2 tog, 0 list) | = | = | = | Select faces to remove. | pending | = spec, error pending (dlg-1) |
| Sphere | fSphere | no live spec | no live spec | no live spec | no live spec | Select a center point | pending | no live spec, error pending (dlg-1) |
| Split | splitPart | 13 (5 sel, 0 val, 6 tog, 2 list); ours only: edgeProjectionOptionsGroup | = | = | = | Select parts, surfaces, or curves to split. | pending | = spec, error pending (dlg-1) |
| Sweep | sweep | 30 (8 sel, 7 val, 10 tog, 5 list) | = | = | = | Select entities for sweep path. | pending | = spec, error pending (dlg-1) |
| Tab | sheetMetalTab | 4 (3 sel, 1 val, 0 tog, 0 list) | = | = | = | Select a face for the tab. | pending | = spec, error pending (dlg-1) |
| Tag | tag | 15 (5 sel, 0 val, 2 tog, 2 list); ours only: additionalAlignmentPointsGroup, standardContentPart | = | = | ≠ tagPurpose options: ['FRAME', 'FORM', 'PCB_HOLES'] / ['FRAME', 'FORM', 'PCB_HOLES', 'STANDARD_CONTENT'] | Select a single sketch feature. The selected feature must create sketch regions. | pending | ≠ spec, error pending (dlg-1) |
| Thicken | thicken | 10 (2 sel, 3 val, 4 tog, 1 list) | = | = | = | Select surfaces or faces to thicken. | pending | = spec, error pending (dlg-1) |
| Transform | transform | 23 (7 sel, 9 val, 5 tog, 2 list) | = | = | = | Cannot resolve entities. | pending | = spec, error pending (dlg-1) |
| Trim curve | trimCurve | 9 (4 sel, 1 val, 1 tog, 3 list) | = | = | = | Select a curve to adjust. | pending | = spec, error pending (dlg-1) |
| Unfold | sheetMetalUnfold | 4 (3 sel, 0 val, 1 tog, 0 list) | = | = | = | Select the sheet metal part to unfold | pending | = spec, error pending (dlg-1) |
| Variable | assignVariable | 42 (4 sel, 18 val, 6 tog, 9 list); ours only: initEntities, rowGroup, columnGroup, axisMeasurementsGroup | = | = | ≠ conversionFactor default: 0 / 1 | Variable name must be a letter followed by a string of letters or numbers. | pending | ≠ spec, error pending (dlg-1) |
| Wrap | wrap | 19 (5 sel, 4 val, 8 tog, 2 list); ours only: positionGroup | = | = | = | Select planar faces or sketch regions to wrap. | pending | = spec, error pending (dlg-1) |
