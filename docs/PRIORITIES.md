# Priorities (lead, 2026-10-24). Read this before picking your next item.

Think of a person doing everyday CAD. Work in this order: most-used first, everything else later.
Never start a lower tier while your area still has open higher-tier items.

## P0: always first
- Crashes, hangs, data loss, wrong geometry, wrong results.
- Performance of the core workflow: regen speed, viewport fps (orbit/pan/zoom), responsiveness while editing, aborting stale work.

## P1: the core Part Studio, used every minute
- **Agent API (CLI + MCP):** everything a user can do in the app must be doable by an agent, with the same options and results. A new feature isn't done until its op exists, with an e2e test. Agent usability (clear errors, discoverable ops, live mode) counts as P1.
- **Sketch:** line, rectangle, circle, arc, slot, dimensions, constraints, trim, offset, use/convert, construction, inference/snapping.
- **Features:** Extrude, Revolve, Fillet, Chamfer, Hole, Shell, Sweep, Loft (basic), Linear/Circular pattern, Mirror, Boolean, Plane, Draft, Split, Transform.
- **The feature list:** edit, rollback bar, suppress, reorder, delete, rename, undo/redo, errors.
- **Interaction:** selection, hover/highlight, navigation, view cube, Zoom to fit, measure, mass properties, section view.
- **Look and behaviour** of everything above: dialogs, toolbar, menus, feature list, parts list.

## P2: used daily, after P1
- **Assemblies:** insert, mates (fastened, revolute, slider, cylindrical, planar, ball), drag/solve, groups, patterns, explode, BOM.
- **Drawings:** views (projected, section, detail), dimensions, notes/callouts, title block, PDF/DXF export.
- **Data exchange:** STEP/Parasolid/STL import and export, configurations, variables, Derived.

## P3: later, only when your P0–P2 list is empty
- Sheet metal, frames, PCB, advanced surfacing (boundary conditions, fill precise mode, loft guides).
- FeatureScript authoring, release management, workflows.
- Translations and languages, CJK/SHX fonts, dark-mode polish.
- Standard content long tail, rare dialog options.

**Releases:** only P0/P1 failures block a release. P2/P3 findings go to the backlog.
