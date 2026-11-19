# Commands and options live has that Noshape lacks or only partly has (live2 completeness audit, 11-19)

Method (live 10:00–10:40 UTC, a scratch doc with a plate, a pin, an assembly and a drawing; captures in
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
| 1 | Assembly | Linear / circular pattern | "Follow seed position" (`syncWithSeeds`): live's pattern instances follow the seed when it moves; no such option in ours | sometimes | asm |
| 2 | Assembly | Derived mirror of an assembly (`derivedAssemblyMirror`: "Assembly to mirror", "Mirror strategy") | no assembly-level derived mirror (ours has Assembly mirror and the Part Studio's Derived mirror) | rarely | asm |
| 3 | Assembly / Part | Mass properties override (`editMassProperties`) | live overrides Mass, Centre of mass (x, y, z) and the inertia tensor (Lxx…Lzz), each with its own Override box; ours overrides the mass only | rarely | asm |
| 4 | Part Studio | Routing curve | editing several points at once by offset and rotation ("Multi X / Y / Z offset", "Multi rotation matrix"); ours edits one point at a time | rarely | core |
| 5 | Drawing | Drawing properties › Units | switching units should reset the precision (fractional / feet-and-inches to 1/256, Millimeters to 0.12); the fraction list goes to 1/256; the chamfer dimension keeps its own precision; feet and inches under a foot as 1 31/32" (drawprops-1119) | often | studios (fix queued, 596bcbb10 stack) |
| 6 | Drawing | Drawing properties › Text alignment, Arrowhead | ANSI aligned Ø / R / angle text; the dot arrowhead the arrow's length across (drawprops-1119) | sometimes | studios (fix queued) |
| 7 | Drawing | Dimension tool picks | the + snap marks on every dimension after a pick (text centre, dimension-line end) and placement snapping to them | often | studios (fix queued) |
| 8 | Drawing | Section cut line | a plain click selects the cut line (grips, the blue anchor), a press-drag selects the parent view | sometimes | studios (fix queued) |
| 9 | Documents | Column header ⋮ menu | Sort ascending / descending / Clear sort, Pin column ›, Autosize this / all columns, Remove column, Choose columns… (panel), Reset columns | rarely | studios (fix queued) |
| 10 | Drawing | Note panel | every item present; Flag notes only on a numbered list, Note order reverses the lines, space before/after 0.20 → 0.38 in pitch (note-opts-1119): not yet compared with ours | sometimes | studios (to compare) |
