# Audit: every Part Studio UI surface of live against ours (psui, from 2026-11-01)

User 11-01: the checklists only cover what someone listed. This sheet audits whole surfaces: toolbar and overflows, every
menu and right-click menu, every feature dialog's fields and options, the keyboard shortcuts, the panels, the view cube and
view menu. Sources: live DOM dumps (onshape-research/live/dom/), behaviours.md, and for dialogs Onshape's FeatureScript std
(crates/featurescript/std, the oracle tests/std_oracle.rs: `print_all` diffs every toolbar feature's dialog with its std
precondition in every visibility state). Status: ✗ gap open, ✅ fixed (commit), – accepted (reason), ? live asked.

## Surfaces

| Surface | Source | Result | Gaps |
|---|---|---|---|
| Toolbar, every group and overflow (75 tools + Add custom features) | behaviours "Toolbar" (Customize toolbar list), dom/toolbar-partstudio.txt | every live tool in live's group, in order (Move boundary = extendSurface) | 0 |
| Toolbar right-click, Customize toolbar, narrow-window collapse | behaviours "Toolbar", "Narrow window" | checked earlier (cursors 81, part-studio 4.x, chrome-sweep) | 0 |
| Feature dialogs, fields and options (75 toolbar features) | std preconditions via std_oracle::print_all | 47 same as std now (37 at first); 28 differ | rows below |
| Keyboard shortcuts (live's Shift+/ list, 121 entries) | dom/keyboard-shortcuts.txt | every applicable entry in workflow-keyboard-mouse.md, all ✅ (11-01) | 0 |
| View menu (camera / render ▾) | dom/menu-camera-render.txt | 16 items = live (part-studio 9.8) | 0 |
| Context menus: face, edge, vertex, empty space, part, plane, origin, mate connector, sketch region / curve, feature rows (sketch, extrude, fillet, multi), parts-list part, tabs, Select ▸ submenus | dom/context-menus-partstudio.txt, -ps-*.txt | compared in core-workflow §8 and chrome-sweep CM1–CM5 (✅) | 0 |
| Context menus not compared yet: mesh part row, composite part row, Surfaces / Curves rows, folder, rollback bar, Configurations / Variable table rows | behaviours 219, 937; live asked for the rest | in progress | – |
| Panels: Measure, Mass and section properties, Section view, Appearances, Variable table, Configurations, Render / Display states | live asked for whole-panel dumps (REQUESTS 11-01) | in progress | – |
| View cube (right-click, arrows, hover) | menus/625-proj-viewcube-rightclick*.png | in progress | – |

**Counts (11-01, audit 2):** surfaces audited 5 of 9 (4 waiting on live dumps); dialog gaps found 199 field rows + 52 visibility states; fixed 73 rows and 39 states, accepted 7, open 119 rows + 13 states. 47 of 75 dialogs now equal std (37 at first).

## Feature dialogs against std

Generated from `std_oracle::print_all` (first run 11-01; rows not in a later run are marked fixed). P1: a missing option or a
wrong default / range that changes what a user gets; P2: what the dialog shows (labels, option labels, groups, row labels,
which fields show); P3: internal (option ids under the same labels, configurability, remembered values, tooltips). The
oracle reads std's annotations, not its feature defaults map: a default std sets there (Ruled surface's cubic interpolation)
is checked by hand.

Field rows: 199 (fixed 73, accepted 7, open 119); visibility states differing now: 13.

| # | Feature | Field | P | Difference (std = live) | Status |
|---|---|---|---|---|---|
| D1 | editCurve | curveDegree | P1 | value: std `Integer 0..15 =0.000000000000 / 0.000000000000` / ours `Integer 0..1000 =3.000000000000 / 3.000000000000` | – read-only counts std's editing logic fills from the picked curve; ours shows the same once picked (P3) |
| D2 | editCurve | curveNumCPs | P1 | value: std `Integer 0..99 =0.000000000000 / 0.000000000000` / ours `Integer 0..100000 =4.000000000000 / 4.000000000000` | – read-only counts std's editing logic fills from the picked curve; ours shows the same once picked (P3) |
| D3 | editCurve | curveNumSpans | P1 | value: std `Integer 0..99 =0.000000000000 / 0.000000000000` / ours `Integer 0..100000 =1.000000000000 / 1.000000000000` | – read-only counts std's editing logic fills from the picked curve; ours shows the same once picked (P3) |
| D4 | mutualTrim | body1 | P1 | value: std `query max None` / ours `query max Some(1)` | ✗ |
| D5 | mutualTrim | body2 | P1 | value: std `query max None` / ours `query max Some(1)` | ✗ |
| D6 | ruledSurface | useCubicInterpolation | P1 | value: std `bool=false` / ours `bool=true` | – std's defaults map sets true (ruledSurface.fs); ours true |
| D7 | sheetMetalJog | jogLimitDistance | P1 | value: std `Length 0..500 =0.005000000000 / 0.006350000000` / ours `Length 0..500 =0.000000000000 / 0.000000000000` | ✅ (audit 1, 11-01) |
| D8 | sheetMetalJog | thicknessFactor | P1 | value: std `Real 0..100000 =1.000000000000 / 1.000000000000` / ours `Real 0.00001..100000 =3.000000000000 / 3.000000000000` | ✅ (audit 1, 11-01) |
| D9 | sheetMetalLoft | chordalTolerance | P1 | value: std `Length 0.00001..0.1 =0.001000000000 / 0.001270000000` / ours `Length 0.00001..0.1 =0.001000000000 / 0.001016000000` | ✅ (audit 1, 11-01) |
| D10 | tag | basePlane | P1 | missing: std has "Base plane" | ✅ (audit 1, 11-01) |
| D11 | tag | flatFormSketch | P1 | value: std `query max None` / ours `query max Some(1)` | – std takes a list but errors on more than one sketch (tag.fs doTagForm); ours allows one |
| D12 | tag | holes | P1 | missing: std has "Holes" | ✅ (audit 1, 11-01) |
| D13 | tag | pcbEntityType | P1 | missing: std has "PCB entity type" | ✅ (audit 1, 11-01) |
| D14 | tag | plated | P1 | missing: std has "Plated" | ✅ (audit 1, 11-01) |
| D15 | assignVariable | anyValue | P2 | extra: ours has "Value", std has no `anyValue` | – std has it (`isAnything`, variable.fs); the oracle's extraction skips isAnything fields |
| D16 | assignVariable | axisMeasurementsGroup | P2 | missing: std has "Axis measurements" (a group) | ✅ (audit 2, 11-01) |
| D17 | assignVariable | columnGroup | P2 | missing: std has "Column" (a group) | ✅ (audit 2, 11-01) |
| D18 | assignVariable | conversionFactor | P2 | extra: ours has "Conversion factor", std has no `conversionFactor` | – std has it (`isAnything`, variable.fs); the oracle's extraction skips isAnything fields |
| D19 | assignVariable | minmax | P2 | hints: std adds [], ours adds ['HORIZONTAL_ENUM'] | ✗ |
| D20 | assignVariable | rowGroup | P2 | missing: std has "Row" (a group) | ✅ (audit 2, 11-01) |
| D21 | bridgingCurve | match1 | P2 | options: std ['Match position', 'Match tangent', 'Match curvature', 'Match flow (G3)'] / ours ['Match position', 'Match tangent', 'Match curvature', 'Match flow'] | ✅ (audit 2, 11-01) |
| D22 | bridgingCurve | match2 | P2 | options: std ['Match position', 'Match tangent', 'Match curvature', 'Match flow (G3)'] / ours ['Match position', 'Match tangent', 'Match curvature', 'Match flow'] | ✅ (audit 2, 11-01) |
| D23 | compositeCurve | approximationShowDeviation | P2 | hints: std adds ['GROUP_HEADER'], ours adds [] | ✅ (audit 2, 11-01) |
| D24 | constrainedSurface | showDeviation | P2 | hints: std adds ['GROUP_HEADER'], ours adds [] | ✅ (audit 2, 11-01) |
| D25 | constrainedSurface | vertices | P2 | array row label: std Some("#vertex"), ours None | ✅ (audit 2, 11-01) |
| D26 | cutlist | columnOverrides | P2 | array row label: std Some("#column = #value"), ours None | ✅ (audit 2, 11-01) |
| D27 | editCurve | approximationShowDeviation | P2 | hints: std adds ['GROUP_HEADER'], ours adds [] | ✅ (audit 2, 11-01) |
| D28 | editCurve | controlPointEdits | P2 | array row label: std Some("#index: #x;#y;#z"), ours None | ✅ (audit 2, 11-01) |
| D29 | editCurve | selectedIndices | P2 | array row label: std Some("Control point index #indexValue"), ours None | ✅ (audit 2, 11-01) |
| D30 | editCurve | showDetails | P2 | hints: std adds ['GROUP_HEADER'], ours adds [] | ✗ |
| D31 | faceBlend | caps | P2 | array row label: std Some("#entity"), ours None | ✅ (audit 2, 11-01) |
| D32 | faceBlend | edgeLimits | P2 | array row label: std Some("#edge"), ours None | ✅ (audit 2, 11-01) |
| D33 | fill | addGuides | P2 | hints: std adds [], ours adds ['GROUP_HEADER'] | ✅ (audit 2, 11-01) |
| D34 | fill | edges | P2 | array item fields differ: std [entities "Edges" query max None hints 0x8000 "" None, continuity "Continuity" enum[("G0", "Position"), ("G1",… ours [entities "Edges" query max None hints 0x0 "" None, continuity "Continuity" enum[("G0", "Position"), ("G1", "T… | ✗ |
| D35 | fitSpline | approximationShowDeviation | P2 | hints: std adds ['GROUP_HEADER'], ours adds [] | ✅ (audit 2, 11-01) |
| D36 | frame | cornerOverrides | P2 | array item fields differ: std [vertex "Vertex" query max Some(1) hints 0x8000 "" None, cornerType "Corner type" enum[("MITER", "Miter"), ("B… ours [vertex "Vertex" query max Some(1) hints 0x0 "" None, cornerType "Corner type" enum[("MITER", "Miter"), ("BUTT… | ✗ |
| D37 | frame | cornerOverrides | P2 | array row label: std Some("#vertex [#cornerType]"), ours None | ✅ (audit 2, 11-01) |
| D38 | frame | selections | P2 | hints: std adds [], ours adds ['ALLOW_QUERY_ORDER'] | ✗ |
| D39 | frameTrim | frameGroups | P2 | array item fields differ: std [frames "Frames" query max None hints 0x0 "Frame segments are trimmed against preceding groups and are used as… ours [frames "Frames" query max None hints 0x0 "" None]… | ✗ |
| D40 | frameTrim | frameGroups | P2 | array row label: std Some("#frames"), ours None | ✅ (audit 2, 11-01) |
| D41 | gusset | chamfer | P2 | hints: std adds [], ours adds ['FIRST_IN_ROW'] | ✗ |
| D42 | helix | endRadToggle | P2 | hints: std adds [], ours adds ['DISPLAY_SHORT', 'FIRST_IN_ROW'] | ✗ |
| D43 | helix | endRadius | P2 | hints: std adds [], ours adds ['DISPLAY_SHORT'] | ✗ |
| D44 | importDerived | mateConnectorId | P2 | extra: ours has "Base mate connector", std has no `mateConnectorId` | ✗ |
| D45 | intersectionCurve | approximationShowDeviation | P2 | hints: std adds ['GROUP_HEADER'], ours adds [] | ✅ (audit 2, 11-01) |
| D46 | isoparametricCurve | directionType | P2 | default option: std U_DIRECTION / ours U | ✅ (audit 1, 11-01) |
| D47 | mateConnector | flipPrimary | P2 | hints: std adds ['FIRST_IN_ROW', 'OPPOSITE_DIRECTION'], ours adds ['PRIMARY_AXIS'] | ✗ |
| D48 | mutualTrim | body1 | P2 | label: std "First set of surfaces", ours "First surface" | ✗ |
| D49 | mutualTrim | body2 | P2 | label: std "Second set of surfaces", ours "Second surface" | ✗ |
| D50 | offsetCurveOnFace | approximationShowDeviation | P2 | hints: std adds ['GROUP_HEADER'], ours adds [] | ✅ (audit 2, 11-01) |
| D51 | pcbStart | hasSecondDirection | P2 | hints: std adds [], ours adds ['GROUP_HEADER'] | ✅ (audit 2, 11-01) |
| D52 | pcbStart | offsetDistance | P2 | hints: std adds ['CAN_BE_TOLERANT'], ours adds [] | ✗ |
| D53 | pcbStart | secondDirectionDepth | P2 | hints: std adds [], ours adds ['CAN_BE_TOLERANT'] | ✗ |
| D54 | pcbStart | secondDirectionOffsetDistance | P2 | hints: std adds ['CAN_BE_TOLERANT'], ours adds [] | ✗ |
| D55 | projectCurves | approximationShowDeviation | P2 | hints: std adds ['GROUP_HEADER'], ours adds [] | ✅ (audit 2, 11-01) |
| D56 | queryVariable | additionalQueries | P2 | array item fields differ: std [booleanOperation "Boolean operation" enum[("UNION", "Union"), ("SUBTRACTION", "Subtract"), ("INTERSECTION", "… ours [booleanOperation "Boolean operation" enum[("UNION", "Union"), ("SUBTRACTION", "Subtract"), ("INTERSECTION", "… | ✗ |
| D57 | queryVariable | additionalQueries | P2 | array row label: std Some("#booleanOperation of #addQlowercaseSelectionType"), ours None | ✗ |
| D58 | queryVariable | edgeAdjacentEntityType | P2 | default option: std ALL / ours EDGES_AND_FACES | ✅ (audit 1, 11-01) |
| D59 | routingCurve | csvUnit | P2 | options: std ['Centimeter', 'Foot', 'Inch', 'Millimeter', 'Meter', 'Yard'] / ours ['Meter', 'Centimeter', 'Millimeter', 'Inch', 'Foot', 'Yard'] | ✅ (audit 1, 11-01) |
| D60 | routingCurve | hasTargetLength | P2 | hints: std adds [], ours adds ['FIRST_IN_ROW'] | ✗ |
| D61 | routingCurve | inputsGroup | P2 | missing: std has "Inputs" (a group) | ✅ (audit 2, 11-01) |
| D62 | routingCurve | points | P2 | array item fields differ: std [index "Point index" Integer 0..10000 =0.000000000000 / 0.000000000000 hints 0x8000 "" None, referenceType "Re… ours [referenceType "Reference" enum[("ORIGIN", "Origin"), ("VERTEX", "Vertex"), ("CURVE", "Curve"), ("RELATIVE", "… | ✗ |
| D63 | routingCurve | points | P2 | array row label: std Some("Point #index"), ours None | ✗ |
| D64 | routingCurve | segmentEditType | P2 | default option: std ADD / ours ADD_POINTS | ✅ (audit 1, 11-01) |
| D65 | routingCurve | selectedIndices | P2 | array row label: std Some("Selected point #indexValue"), ours None | ✗ |
| D66 | ruledSurface | vertexOverrides | P2 | array item fields differ: std [vertex "Vertex" query max Some(1) hints 0x2 "" None, overrideType "Ruled surface type" enum[("NORMAL", "Norma… ours [vertex "Vertex" query max Some(1) hints 0x2 "" None, overrideType "Ruled surface type" enum[("NORMAL", "Norma… | ✗ |
| D67 | ruledSurface | vertexOverrides | P2 | array row label: std Some("#vertex"), ours None | ✅ (audit 2, 11-01) |
| D68 | sheetMetalBend | oppositeAngleFromDirection | P2 | hints: std adds ['OPPOSITE_DIRECTION'], ours adds ['OPPOSITE_DIRECTION_CIRCULAR'] | ✅ (audit 2, 11-01) |
| D69 | sheetMetalBend | oppositeAngleFromDirection | P2 | label: std "Opposite direction angle", ours "Opposite angle" | ✅ (audit 2, 11-01) |
| D70 | sheetMetalBendRelief | bendReliefStyle | P2 | hints: std adds ['SHOW_LABEL'], ours adds [] | ✅ (audit 2, 11-01) |
| D71 | sheetMetalCorner | cornerStyle | P2 | hints: std adds ['SHOW_LABEL'], ours adds [] | ✅ (audit 2, 11-01) |
| D72 | sheetMetalCornerBreak | allowEdgeOverflow | P2 | hints: std adds ['GROUP_HEADER'], ours adds [] | ✅ (audit 2, 11-01) |
| D73 | sheetMetalCornerBreak | angle | P2 | hints: std adds [], ours adds ['CAN_BE_TOLERANT'] | ✗ |
| D74 | sheetMetalCornerBreak | width | P2 | hints: std adds [], ours adds ['CAN_BE_TOLERANT'] | ✗ |
| D75 | sheetMetalCornerBreak | width1 | P2 | hints: std adds [], ours adds ['CAN_BE_TOLERANT'] | ✗ |
| D76 | sheetMetalCornerBreak | width2 | P2 | hints: std adds [], ours adds ['CAN_BE_TOLERANT'] | ✗ |
| D77 | sheetMetalFlange | angleFromDirectionOppositeAngle | P2 | hints: std adds ['OPPOSITE_DIRECTION'], ours adds ['OPPOSITE_DIRECTION_CIRCULAR'] | ✅ (audit 2, 11-01) |
| D78 | sheetMetalFlange | endConditionsGroup | P2 | missing: std has "End conditions" (a group) | ✅ (audit 1, 11-01) |
| D79 | sheetMetalFlange | overallParametersGroup | P2 | missing: std has "Overall parameters" (a group) | ✅ (audit 1, 11-01) |
| D80 | sheetMetalFlange | secondBoundingEntityOffset | P2 | label: std "Second distance", ours "Distance" | ✅ (audit 2, 11-01) |
| D81 | sheetMetalHem | hemAlignment | P2 | hints: std adds ['REMEMBER_PREVIOUS_VALUE', 'SHOW_LABEL'], ours adds [] | ✅ (audit 2, 11-01) |
| D82 | sheetMetalHem | hemCornerType | P2 | hints: std adds ['REMEMBER_PREVIOUS_VALUE', 'SHOW_LABEL'], ours adds [] | ✅ (audit 2, 11-01) |
| D83 | sheetMetalJog | oppositeAngleFromDirection | P2 | hints: std adds ['OPPOSITE_DIRECTION'], ours adds ['OPPOSITE_DIRECTION_CIRCULAR'] | ✅ (audit 2, 11-01) |
| D84 | sheetMetalJog | oppositeAngleFromDirection | P2 | label: std "Opposite direction angle", ours "Opposite angle" | ✅ (audit 2, 11-01) |
| D85 | sheetMetalLoft | connections | P2 | array item fields differ: std [connectionEntities "Vertices or edges" query max None hints 0x0 "" None, isRip "Rip" bool=false hints 0x0 "" … ours [connectionEntities "Vertices or edges" query max None hints 0x0 "" None, isRip "Rip" bool=false hints 0x0 "" … | ✗ |
| D86 | sheetMetalLoft | connections | P2 | array row label: std Some("#connectionEntities"), ours None | ✅ (audit 2, 11-01) |
| D87 | sheetMetalStart | hasSecondDirection | P2 | hints: std adds [], ours adds ['GROUP_HEADER'] | ✅ (audit 2, 11-01) |
| D88 | sheetMetalStart | offsetDistance | P2 | hints: std adds ['CAN_BE_TOLERANT'], ours adds [] | ✗ |
| D89 | sheetMetalStart | secondDirectionDepth | P2 | hints: std adds [], ours adds ['CAN_BE_TOLERANT'] | ✗ |
| D90 | sheetMetalStart | secondDirectionOffsetDistance | P2 | hints: std adds ['CAN_BE_TOLERANT'], ours adds [] | ✗ |
| D91 | tag | additionalAlignmentPointsGroup | P2 | missing: std has "Additional alignment points" (a group) | ✅ (audit 1, 11-01) |
| D92 | tag | additionalColumns | P2 | array row label: std Some("#header = #value"), ours None | ✅ (audit 1, 11-01) |
| D93 | assignVariable | angleValue | P3 | hints: std adds ['INITIAL_FOCUS_ON_EDIT'], ours adds [] | ✗ |
| D94 | assignVariable | angleValue | P3 | value: std `Angle -1745.3292519943295..1745.3292519943295 =0.000000000000 / 0.000000000000` / ours `Angle -6.283185307179586..6.283185307179586 =0.000000000000 / 0.000000000000` | ✗ |
| D95 | assignVariable | axisMeasurements | P3 | extra: ours has "Axis measurements", std has no `axisMeasurements` | ✅ (audit 2, 11-01) |
| D96 | assignVariable | columnIndex | P3 | value: std `Integer 0..100000 =0.000000000000 / 0.000000000000` / ours `Integer 0..1000000 =0.000000000000 / 0.000000000000` | ✗ |
| D97 | assignVariable | columnLabelIndex | P3 | value: std `Integer 0..100000 =0.000000000000 / 0.000000000000` / ours `Integer 0..1000000 =0.000000000000 / 0.000000000000` | ✗ |
| D98 | assignVariable | columnMaxIndex | P3 | value: std `Integer 0..100000 =0.000000000000 / 0.000000000000` / ours `Integer 0..1000000 =0.000000000000 / 0.000000000000` | ✗ |
| D99 | assignVariable | columnMinIndex | P3 | value: std `Integer 0..100000 =0.000000000000 / 0.000000000000` / ours `Integer 0..1000000 =0.000000000000 / 0.000000000000` | ✗ |
| D100 | assignVariable | csv | P3 | kind: std `reference Csv` / ours `text=""` (our own picker) | ✗ |
| D101 | assignVariable | customOffset | P3 | value: std `Length 0..500 =0.000000000000 / 0.000000000000` / ours `Length -500..500 =0.000000000000 / 0.000000000000` | ✗ |
| D102 | assignVariable | distance | P3 | value: std `Length 0..500 =0.000000000000 / 0.000000000000` / ours `Length -500..500 =0.000000000000 / 0.000000000000` | ✗ |
| D103 | assignVariable | extendEntities | P3 | tooltip: std "Measure to infinitely extended lines, arcs, cylinders, etc.", ours "" | ✗ |
| D104 | assignVariable | lengthValue | P3 | hints: std adds ['INITIAL_FOCUS_ON_EDIT'], ours adds [] | ✗ |
| D105 | assignVariable | measureFromAxis | P3 | tooltip: std "Measure to axis of cylinders, circles, arcs, or revolved surfaces", ours "" | ✗ |
| D106 | assignVariable | measurementMode | P3 | tooltip: std "Select distance to measure the linear distance between two entities./nSelect len", ours "" | ✗ |
| D107 | assignVariable | minmax | P3 | tooltip: std "Choose to measure minimum or maximum distance between entities", ours "" | ✗ |
| D108 | assignVariable | numberValue | P3 | hints: std adds ['INITIAL_FOCUS_ON_EDIT'], ours adds [] | ✗ |
| D109 | assignVariable | rowIndex | P3 | value: std `Integer 0..100000 =0.000000000000 / 0.000000000000` / ours `Integer 0..1000000 =0.000000000000 / 0.000000000000` | ✗ |
| D110 | assignVariable | rowLabelIndex | P3 | value: std `Integer 0..100000 =0.000000000000 / 0.000000000000` / ours `Integer 0..1000000 =0.000000000000 / 0.000000000000` | ✗ |
| D111 | assignVariable | rowMaxIndex | P3 | value: std `Integer 0..100000 =0.000000000000 / 0.000000000000` / ours `Integer 0..1000000 =0.000000000000 / 0.000000000000` | ✗ |
| D112 | assignVariable | rowMinIndex | P3 | value: std `Integer 0..100000 =0.000000000000 / 0.000000000000` / ours `Integer 0..1000000 =0.000000000000 / 0.000000000000` | ✗ |
| D113 | assignVariable | xOffset | P3 | value: std `Length 0..500 =0.000000000000 / 0.000000000000` / ours `Length -500..500 =0.000000000000 / 0.000000000000` | ✗ |
| D114 | assignVariable | yOffset | P3 | value: std `Length 0..500 =0.000000000000 / 0.000000000000` / ours `Length -500..500 =0.000000000000 / 0.000000000000` | ✗ |
| D115 | assignVariable | zOffset | P3 | value: std `Length 0..500 =0.000000000000 / 0.000000000000` / ours `Length -500..500 =0.000000000000 / 0.000000000000` | ✗ |
| D116 | boundarySurface | uProfilesArray | P3 | array item "profile" (std) vs "Profile" (ours) | ✗ |
| D117 | boundarySurface | vProfilesArray | P3 | array item "profile" (std) vs "Profile" (ours) | ✗ |
| D118 | constrainedSurface | vertices | P3 | hints: std adds ['PREVENT_ARRAY_REORDER'], ours adds [] | ✗ |
| D119 | cutlist | columnOverrides | P3 | array item "Column Override" (std) vs "Column override" (ours) | ✗ |
| D120 | cutlist | columnOverrides | P3 | hints: std adds ['PREVENT_ARRAY_REORDER'], ours adds [] | ✗ |
| D121 | editCurve | controlPointEdits | P3 | array item "point override" (std) vs "Point override" (ours) | ✗ |
| D122 | editCurve | controlPointEdits | P3 | hints: std adds ['PREVENT_ARRAY_REORDER'], ours adds [] | ✗ |
| D123 | editCurve | editPointMode | P3 | option ids: std ['XYZ', 'UVN_TANGENT', 'UVN_CONTROL_POLYGON'] / ours ['XYZ', 'UVN_TANGENT', 'UVN_POLYGON'] | ✅ (audit 1, 11-01) |
| D124 | editCurve | planeReference | P3 | option ids: std ['BEST', 'YZPLANE', 'XZPLANE', 'XYPLANE', 'CUSTOM'] / ours ['BEST', 'YZ', 'XZ', 'XY', 'CUSTOM'] | ✅ (audit 1, 11-01) |
| D125 | editCurve | selectedIndices | P3 | array item "index" (std) vs "Index" (ours) | ✗ |
| D126 | editCurve | selectedIndices | P3 | hints: std adds ['PREVENT_ARRAY_REORDER', 'X25'], ours adds [] | ✗ |
| D127 | endcap | cornerDistance | P3 | hints: std adds ['REMEMBER_PREVIOUS_VALUE'], ours adds [] | ✗ |
| D128 | endcap | cornerRadius | P3 | hints: std adds ['REMEMBER_PREVIOUS_VALUE'], ours adds [] | ✗ |
| D129 | faceBlend | caps | P3 | array item "cap" (std) vs "Cap" (ours) | ✗ |
| D130 | faceBlend | crossSectionShape | P3 | tooltip: std "Cross sectional control", ours "" | ✗ |
| D131 | faceBlend | edgeLimits | P3 | array item "edge limit" (std) vs "Edge limit" (ours) | ✗ |
| D132 | fill | edges | P3 | array item "edge" (std) vs "Edge" (ours) | ✗ |
| D133 | frame | cornerOverrides | P3 | array item "vertex" (std) vs "Corner" (ours) | ✗ |
| D134 | frame | profileSketch | P3 | kind: std `reference PartStudio` / ours `text=""` (our own picker) | ✗ |
| D135 | frame | selections | P3 | tooltip: std "Faces, edges, and vertices that define sweep paths", ours "" | ✗ |
| D136 | frame | trimBodies | P3 | tooltip: std "Parts to use as trim tools", ours "" | ✗ |
| D137 | frame | trimPlanes | P3 | tooltip: std "Planes and planar faces to use as trim tools", ours "" | ✗ |
| D138 | frameTrim | frameGroups | P3 | array item "group" (std) vs "Group" (ours) | ✗ |
| D139 | frameTrim | frameGroups | P3 | tooltip: std "Frame segments in earlier groups are used as tools for later groups", ours "" | ✗ |
| D140 | frameTrim | targets | P3 | tooltip: std "Parts to trim using trim tool", ours "" | ✗ |
| D141 | frameTrim | tool | P3 | tooltip: std "Face, plane, mate connector, or planar face to use as trim tool", ours "" | ✗ |
| D142 | gusset | edges | P3 | tooltip: std "Edges that define the bases of the gussets", ours "" | ✗ |
| D143 | helix | actualPitch | P3 | value: std `Length 0.00001..500 =0.025000000000 / 0.025400000000` / ours `Length 0..500 =0.025000000000 / 0.025400000000` | ✗ |
| D144 | helix | actualRevolutions | P3 | value: std `Real 0.00001..1000 =4.000000000000 / 4.000000000000` / ours `Real 0..1000000000 =4.000000000000 / 4.000000000000` | ✗ |
| D145 | importDerived | partStudio | P3 | kind: std `reference PartStudio` / ours `text=""` (our own picker) | ✗ |
| D146 | isoparametricCurve | directionType | P3 | option ids: std ['U_DIRECTION', 'V_DIRECTION'] / ours ['U', 'V'] | ✅ (audit 1, 11-01) |
| D147 | mateConnector | originAdditionalQuery | P3 | hints: std adds ['UNCONFIGURABLE'], ours adds [] | ✗ |
| D148 | offsetCurveOnFace | scope | P3 | option ids: std ['OFFSET', 'OFFSET_AND_EXTEND', 'OFFSET_EXTEND_AND_SPLIT'] / ours ['OFFSET', 'OFFSET_EXTEND', 'OFFSET_EXTEND_SPLIT'] | ✅ (audit 1, 11-01) |
| D149 | pcbEnd | entitiesToUnfold | P3 | hints: std adds ['FOCUS_ON_VISIBLE'], ours adds [] | ✗ |
| D150 | pcbStart | bendsIncluded | P3 | tooltip: std "Check to include the clearance for bends", ours "" | ✗ |
| D151 | queryVariable | additionalQueries | P3 | array item "additional query" (std) vs "Additional query" (ours) | ✗ |
| D152 | queryVariable | edgeAdjacentEntityType | P3 | option ids: std ['ALL', 'EDGE', 'FACE'] / ours ['EDGES_AND_FACES', 'EDGES', 'FACES'] | ✅ (audit 1, 11-01) |
| D153 | queryVariable | vertexAdjacentEntityType | P3 | option ids: std ['ALL', 'EDGE', 'VERTEX', 'FACE'] / ours ['ALL', 'EDGES', 'VERTICES', 'FACES'] | ✅ (audit 1, 11-01) |
| D154 | routingCurve | addPointOnAxisDrag | P3 | hints: std adds ['UNCONFIGURABLE'], ours adds [] | ✗ |
| D155 | routingCurve | csvUnit | P3 | hints: std adds ['UNCONFIGURABLE'], ours adds [] | ✅ (audit 2, 11-01) |
| D156 | routingCurve | csvUnit | P3 | option ids: std ['Centimeter', 'Foot', 'Inch', 'Millimeter', 'Meter', 'Yard'] / ours ['Meter', 'Centimeter', 'Millimeter', 'Inch', 'Foot', 'Yard'] | ✅ (audit 1, 11-01) |
| D157 | routingCurve | curveBaseCSys | P3 | tooltip: std "Point or mate connector used as reference for the curve. Defaults to origin.", ours "" | ✗ |
| D158 | routingCurve | inputCurve | P3 | hints: std adds ['UNCONFIGURABLE'], ours adds [] | ✅ (audit 2, 11-01) |
| D159 | routingCurve | inputData | P3 | kind: std `reference Csv` / ours `text=""` (our own picker) | ✗ |
| D160 | routingCurve | inputType | P3 | hints: std adds ['UNCONFIGURABLE'], ours adds [] | ✅ (audit 2, 11-01) |
| D161 | routingCurve | inputVertices | P3 | hints: std adds ['UNCONFIGURABLE'], ours adds [] | ✅ (audit 2, 11-01) |
| D162 | routingCurve | keepPoints | P3 | tooltip: std "Create points in addition to the curve.", ours "" | ✗ |
| D163 | routingCurve | numPointsToAdd | P3 | hints: std adds ['UNCONFIGURABLE'], ours adds [] | ✗ |
| D164 | routingCurve | orthoCsys | P3 | hints: std adds ['UNCONFIGURABLE'], ours adds [] | ✗ |
| D165 | routingCurve | orthoCsysMate | P3 | hints: std adds ['UNCONFIGURABLE'], ours adds [] | ✗ |
| D166 | routingCurve | overwritePoints | P3 | hints: std adds ['UNCONFIGURABLE'], ours adds [] | ✅ (audit 2, 11-01) |
| D167 | routingCurve | points | P3 | array item "point" (std) vs "Point" (ours) | ✗ |
| D168 | routingCurve | points | P3 | hints: std adds ['PREVENT_ARRAY_REORDER'], ours adds [] | ✗ |
| D169 | routingCurve | pointsEditType | P3 | hints: std adds ['UNCONFIGURABLE'], ours adds [] | ✗ |
| D170 | routingCurve | pointsEditType | P3 | option ids: std ['SINGLE', 'MULTI'] / ours ['SINGLE', 'MULTIPLE'] | ✅ (audit 1, 11-01) |
| D171 | routingCurve | samples | P3 | hints: std adds ['UNCONFIGURABLE'], ours adds [] | ✅ (audit 2, 11-01) |
| D172 | routingCurve | segmentEditType | P3 | hints: std adds ['UNCONFIGURABLE'], ours adds [] | ✗ |
| D173 | routingCurve | segmentEditType | P3 | option ids: std ['ADD', 'ORTHOGONAL'] / ours ['ADD_POINTS', 'ORTHOGONAL_PATH'] | ✅ (audit 1, 11-01) |
| D174 | routingCurve | selectedIndices | P3 | array item "index" (std) vs "Point" (ours) | ✗ |
| D175 | routingCurve | selectedIndices | P3 | hints: std adds ['PREVENT_ARRAY_REORDER', 'X25'], ours adds [] | ✗ |
| D176 | routingCurve | showCSys | P3 | tooltip: std "Show coordinate system of the reference used by the current vertex.", ours "" | ✗ |
| D177 | routingCurve | startColumn | P3 | hints: std adds ['UNCONFIGURABLE'], ours adds [] | ✅ (audit 2, 11-01) |
| D178 | routingCurve | targetLength | P3 | value: std `Length -500..500 =0.025000000000 / 0.025400000000` / ours `Length 0.00001..500 =0.025000000000 / 0.025400000000` | ✗ |
| D179 | ruledSurface | cornerType | P3 | option ids: std ['SPUN', 'EXTENDED', 'LOFTED', 'NO_CORNER'] / ours ['SPUN', 'EXTENDED', 'LOFTED', 'NONE'] | ✅ (audit 1, 11-01) |
| D180 | ruledSurface | vertexOverrides | P3 | array item "arrayItem" (std) vs "Vertex" (ours) | ✗ |
| D181 | sheetMetalCorner | cornerReliefScale | P3 | hints: std adds [], ours adds ['REMEMBER_PREVIOUS_VALUE'] | ✗ |
| D182 | sheetMetalCornerBreak | crossSection | P3 | tooltip: std "Cross sectional control", ours "" | ✗ |
| D183 | sheetMetalCornerBreak | filletWidth | P3 | hints: std adds ['REMEMBER_PREVIOUS_VALUE'], ours adds [] | ✗ |
| D184 | sheetMetalCornerBreak | otherRadius | P3 | hints: std adds ['REMEMBER_PREVIOUS_VALUE'], ours adds [] | ✗ |
| D185 | sheetMetalFlange | partialEndGroup | P3 | extra: ours has "End conditions", std has no `partialEndGroup` | ✅ (audit 1, 11-01) |
| D186 | sheetMetalFlange | partialOverallGroup | P3 | extra: ours has "Overall parameters", std has no `partialOverallGroup` | ✅ (audit 1, 11-01) |
| D187 | sheetMetalFormed | formPartStudio | P3 | kind: std `reference PartStudio` / ours `text=""` (our own picker) | ✗ |
| D188 | sheetMetalHem | angle | P3 | hints: std adds ['REMEMBER_PREVIOUS_VALUE'], ours adds [] | ✗ |
| D189 | sheetMetalHem | innerRadius | P3 | hints: std adds ['REMEMBER_PREVIOUS_VALUE'], ours adds [] | ✗ |
| D190 | sheetMetalHem | length | P3 | hints: std adds ['REMEMBER_PREVIOUS_VALUE'], ours adds [] | ✗ |
| D191 | sheetMetalHem | tipGap | P3 | hints: std adds ['REMEMBER_PREVIOUS_VALUE'], ours adds [] | ✗ |
| D192 | sheetMetalHem | useMinimalGapForStraight | P3 | hints: std adds ['REMEMBER_PREVIOUS_VALUE'], ours adds [] | ✗ |
| D193 | sheetMetalHem | useMinimalGapForTipGap | P3 | hints: std adds ['REMEMBER_PREVIOUS_VALUE'], ours adds [] | ✗ |
| D194 | sheetMetalJog | bendAngle | P3 | hints: std adds ['UNCONFIGURABLE'], ours adds [] | ✗ |
| D195 | sheetMetalJog | bendAngle | P3 | value: std `Angle 0.017453292519943295..6.281439977927592 =1.570796326795 / 1.570796326795` / ours `Angle 0.017453292519943295..3.12413936106985 =1.570796326795 / 1.570796326795` | ✗ |
| D196 | sheetMetalLoft | connections | P3 | array item "connection" (std) vs "Connection" (ours) | ✗ |
| D197 | sheetMetalStart | bendsIncluded | P3 | tooltip: std "Check to include the clearance for bends", ours "" | ✗ |
| D198 | tag | additionalColumns | P3 | hints: std adds ['PREVENT_ARRAY_REORDER'], ours adds ['UNCONFIGURABLE'] | ✅ (audit 1, 11-01) |
| D199 | tag | cSysMateConnector | P3 | tooltip: std "If none selected, this feature will create one at Origin", ours "" | ✗ |

### Visibility states that differ (current run)

| # | Feature | P | States | Status |
|---|---|---|---|---|
| V1 | importDerived | P2 | shown fields differ in ["newUI", "placement=AT_MATE_CONNECTOR", "includeMateConnectors", "includeProperties"]: shown only by std [], only by builtin ["mateConnectorId"] | ✗ |
| V2 | assignVariable | P2 | shown fields differ in ["mode=ASSIGNED", "variableType=ANY", "measurementMode=DISTANCE", "rowGroup", "rowAccessType=INDEX", "rowResult=SINGLE", "columnGroup", "columnAccessType=INDEX", "columnResult=SINGLE", "minmax=MINIMUM", "componentSele | ✗ |
| V3 | assignVariable | P2 | shown fields differ in ["mode=TABLE", "variableType=LENGTH", "measurementMode=DISTANCE", "rowGroup", "rowAccessType=INDEX", "rowResult=SINGLE", "columnGroup", "columnAccessType=INDEX", "columnResult=SINGLE", "useConversionFactor", "minmax=M | ✗ |
| V4 | assignVariable | P2 | shown fields differ in ["mode=TABLE", "variableType=LENGTH", "measurementMode=DISTANCE", "rowGroup", "rowAccessType=LABEL", "rowResult=ARRAY", "columnGroup", "columnAccessType=ALL", "columnResult=SINGLE", "useConversionFactor", "minmax=MAXI | ✗ |
| V5 | assignVariable | P2 | shown fields differ in ["mode=TABLE", "variableType=LENGTH", "measurementMode=LENGTH", "rowGroup", "rowAccessType=INDEX_RANGE", "rowResult=ARRAY", "columnGroup", "columnAccessType=INDEX_RANGE", "columnResult=ARRAY", "useConversionFactor", " | ✗ |
| V6 | assignVariable | P2 | shown fields differ in ["mode=TABLE", "variableType=NUMBER", "measurementMode=LENGTH", "rowGroup", "rowAccessType=INDEX", "rowRegexp", "rowResult=MAP", "columnAccessType=INDEX_RANGE", "columnResult=SINGLE", "useConversionFactor", "minmax=MA | ✗ |
| V7 | assignVariable | P2 | shown fields differ in ["mode=TABLE", "variableType=LENGTH", "measurementMode=LENGTH", "rowAccessType=INDEX", "rowRegexp", "rowResult=MAP", "columnGroup", "columnAccessType=ALL", "columnRegexp", "columnResult=ARRAY", "useConversionFactor",  | ✗ |
| V8 | assignVariable | P2 | shown fields differ in ["mode=TABLE", "variableType=NUMBER", "measurementMode=DISTANCE", "rowGroup", "rowAccessType=INDEX", "rowRegexp", "rowResult=SINGLE", "columnAccessType=INDEX_RANGE", "columnRegexp", "columnResult=SINGLE", "useConversi | ✗ |
| V9 | assignVariable | P2 | shown fields differ in ["mode=TABLE", "variableType=ANY", "measurementMode=DIAMETER", "rowGroup", "rowAccessType=INDEX_RANGE", "rowRegexp", "rowResult=SINGLE", "columnGroup", "columnAccessType=INDEX", "columnRegexp", "columnResult=SINGLE",  | ✗ |
| V10 | assignVariable | P2 | shown fields differ in ["mode=TABLE", "variableType=ANY", "measurementMode=LENGTH", "rowAccessType=INDEX_RANGE", "rowRegexp", "rowResult=ARRAY", "columnAccessType=INDEX", "columnResult=MAP", "useConversionFactor", "minmax=MAXIMUM", "axisMea | ✗ |
| V11 | assignVariable | P2 | shown fields differ in ["mode=TABLE", "variableType=NUMBER", "measurementMode=DISTANCE", "rowGroup", "rowAccessType=INDEX", "rowResult=SINGLE", "columnGroup", "columnAccessType=INDEX", "columnRegexp", "columnResult=MAP", "useConversionFacto | ✗ |
| V12 | assignVariable | P2 | shown fields differ in ["mode=TABLE", "variableType=ANGLE", "measurementMode=DISTANCE", "rowGroup", "rowAccessType=INDEX_RANGE", "rowResult=SINGLE", "columnAccessType=INDEX_RANGE", "columnRegexp", "columnResult=ARRAY", "useConversionFactor" | ✗ |
| V13 | assignVariable | P2 | shown fields differ in ["mode=TABLE", "variableType=ANGLE", "measurementMode=LENGTH", "rowAccessType=LABEL", "rowResult=MAP", "columnGroup", "columnAccessType=LABEL", "columnRegexp", "columnResult=SINGLE", "useConversionFactor", "minmax=MAX | ✗ |
