# Commands and options live has that Noshape lacks or only partly has (live2 completeness audit, 11-19)

Method (live 09:55–10:12 UTC, a scratch doc with a plate, a pin, an assembly and a drawing; captures in
`onshape-research/live/audit-1119/`):
- **Commands:** every Part Studio, Assembly and Drawing command from live's Customize toolbar (with "Show items
  already in toolbar"), the toolbar and its dropdowns, and Search tools swept a–z in the Part Studio and in a
  sketch. Context menus: Part Studio rows (Origin, plane, sketch, feature, part, the list headers), graphics
  (face, empty), tabs; Assembly root, Origin, instance, graphics part / empty, tab, with submenus.
- **Dialog options:** live's REST `featurespecs` (Part Studio 97 types, Assembly 23) compared parameter by
  parameter (FeatureScript ids and enum options) with `feature_schema {"all": true}`. Assembly specs, Drawing
  properties (8 tabs, every label and list option), the drawing toolbar and dropdowns, and the sketch tools:
  every label searched in `crates/`.
- **Ours:** `docs/AGENT_API.md`, `crates/ops/src/coverage.rs` (588 items, none `Planned`), the app's strings.

Result: every live command of the three studios and the sketch exists in ours (77/77 Part Studio feature
types, every sketch tool, every drawing tool and dropdown item, every Assembly toolbar item except Simulation's),
and every Part Studio feature parameter and option except the rows below. Earlier audits
(`audit-part-studio.md`, `ps-coverage-audit.md`) had closed the rest.

Excluded: cloud-only or not a user command: "Add custom features" (other documents' / public FeatureScript;
local Feature Studios work), Simulation (Force, Moment, Pressure, Acceleration, Angular velocity, Bearing,
Rigid body), Create task, Add comment, Hole's legacy V2 tolerance parameters, the hole/length/angle tolerance
parameter libraries, and std's internal table/attribute features (cutlistTable, holeTable, routingCurveTable,
computeMass, sheetMetalCornerBreakAttributeBased).

Usage: **often** = most sessions of a user of that area; **sometimes** = a few times a week; **rarely** =
specialist or occasional.

| # | Area | Command | What's missing | How often users reach for it | Owner |
|---|---|---|---|---|---|
| 1 | Assembly | Linear / circular pattern | "Follow seed position" (`syncWithSeeds`): live's pattern instances follow the seed when it moves; no such option in ours. **asm 11-19: no gap**: live's spec hides it always (`BTParameterVisibilityAlwaysHidden`, default true), and ours' copies follow their seed (crates/assembly tests, "copies follow the seed") | sometimes | asm |
| 2 | Assembly | Derived mirror of an assembly (`derivedAssemblyMirror`: "Assembly to mirror", "Mirror strategy") | no assembly-level derived mirror (ours has Assembly mirror and the Part Studio's Derived mirror). asm 11-19: capture asked of live2 (where it's invoked, its dialog, the assembly it makes) | rarely | asm |
| 3 | Assembly / Part | Mass properties override (`editMassProperties`) | live overrides Mass, Centre of mass (x, y, z) and the inertia tensor (Lxx…Lzz), each with its own Override box; ours overrides the mass only. asm 11-19: every live capture so far greys the Centre of mass and inertia boxes (behaviours.md: assembly top row, instances, a part without material), as ours; capture asked of live2 of a case where they enable | rarely | asm |
| 4 | Part Studio | Routing curve | editing several points at once by offset and rotation ("Multi X / Y / Z offset", "Multi rotation matrix"); ours edits one point at a time. **psui 11-19: by offset done**: in Multiple a click on a point toggles it in Selected points, the triad at their centre moves them all (dialogs_feel_tests::a_routing_curves_multiple_triad_moves_the_selected_points; core aeb0cc0a7 the hidden params); **psui 11-19 (live2 routing-multi): rotation and Edit reference done**: the full triad (rings at the tips, a centre ball, live's grey / blue), a ring turns only the triad (5° near a multiple, multiRotationMatrix; the points stay), arrows and planes move the points along the turned axes on the 0.1 mm step, an Edit reference puts the triad on its vertex moved by multiDx/Dy/Dz; the points' rings Ø16 grey / light blue / dark blue (dialogs_feel_tests::a_routing_curves_multiple_ring_turns_only_the_triad_and_its_arrows_follow_the_turn); the drag's value box ("10.2 mm", "45 deg", a typed value + Enter); the centre ball (live2 ball-a/b) snaps to the hovered face's inference points, the triad taking that point's frame, the points translated with it (dialogs_feel_tests::a_routing_curves_multiple_ball_snaps_to_a_faces_inference_point). Done | rarely | core, psui |
| 5 | Drawing | Drawing properties › Units | switching units should reset the precision (fractional / feet-and-inches to 1/256, Millimeters to 0.12); the fraction list goes to 1/256; the chamfer dimension keeps its own precision; feet and inches under a foot as 1 31/32" (drawprops-1119) | often | studios (fixed, 0e754c57); live2 retake 11-19 16:15 on green 41fcfd2c7 (recheck-1119/studios2/dq-*): the precision reset (1/256, 1/256", 0.12), the 0…1/256 list and a stacked Ø101/256 hold; LEFT: the Tolerance precision stays 0.123 (live switches it too: 1/256, 1/256", 0.1), and a linear value stays inline "1 147/256" over its arrow (live stacks it: 3 19/128); chamfer and feet-and-inches over a foot not retaken (no chamfer on that sheet) |
| 6 | Drawing | Drawing properties › Text alignment, Arrowhead | ANSI aligned Ø / R / angle text; the dot arrowhead the arrow's length across (drawprops-1119) | sometimes | studios (fixed, 0e754c57); live2 retake 11-19 16:15 on green 41fcfd2c7: holds, Ø text along its leader, the six arrowheads, the dot ≈ 19 px for a 20 px arrow |
| 7 | Drawing | Dimension tool picks | the + snap marks on every dimension after a pick (text centre, dimension-line end) and placement snapping to them | often | studios (fixed, 0e754c57) |
| 8 | Drawing | Section cut line | a plain click selects the cut line (grips, the blue anchor), a press-drag selects the parent view | sometimes | studios (fixed, 0e754c57) |
| 9 | Documents | Column header ⋮ menu | Sort ascending / descending / Clear sort, Pin column ›, Autosize this / all columns, Remove column, Choose columns… (panel), Reset columns | rarely | studios (fixed, 0e754c57) |
| 10 | Drawing | Note panel | every item present (note-opts-1119). Compared 11-19: Flag notes only on list items, the 11 shapes, the colour popup, inserted properties already as live; fixed (cacd477d): Note order Bottom to top draws the lines in reverse, space before / after 0.20 → 0.38 in (before lowers the first line), list text at a 1.5-height tab, an auto-sized note ignores Vertical alignment, flagged lines spread to fit | sometimes | studios (fixed) |
| 11 | Drawing | Hole/thread callout, Hole table | compared with live holecallout-1119 / holetable-1119 ("studios hole plate"): fixed: a feature-patterned hole counted (4X), a tapped hole's drill line first, no space after ⌴ ⌵ ↧, the countersink angle to 0.1°, ISO's pitch "M6x1"; the table a letter per hole spec in the order made, locations to the fixed precision ("15,00") | often | studios (fixed) |
| 12 | Drawing | Export › Inspection list, Drawing definition list | compared with live export-1119: fixed: live's CSV (header, BOM, CRLF, "--", "Sheet1 (1)", a row per hole callout value, notes, "(overridden)"); DXF sheets, title block properties and the 23 date formats already as live; Note border "Square circle" | rarely | studios (fixed) |
