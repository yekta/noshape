**Fixed**
- Windows: the in-app update installs again, also from older versions and while a coding agent (Claude Desktop, Claude Code) keeps noshape running in the background. If an update can't be installed, noshape opens again and says why.
- Flat faces no longer show dark wedge-shaped streaks (since v0.4.32, some triangles were drawn from the inside).
- Reopening a document shows its model at once (within half a second) while it rebuilds in the background.
- Assemblies: screw and rack-and-pinion relations no longer stop after half a pitch, and gear relations no longer jump back; a relation ratio given as a variable works; drawing views keep their exploded view or named position when those are reordered or deleted.
- Enter in the graphics area accepts the open assembly dialog, as in Onshape.

**Closer to Onshape**
- Sketch: a spline's handle arms can be selected, constrained and dimensioned like lines.
- Drawings: Break view works as in Onshape (applied with ✓, its own menu, Onshape's default gap); the empty sheet's menu has Clear selection.
- Assemblies: the menus of hidden, suppressed and isolated instances, Collapse all, Display states and several dialogs as in Onshape; configuration variables work in mates and relations; linked parts show their BOM properties.
- Import: STEP and IGES files read with clean faces and edge rings (O-rings, hoses, motor mounts); multi-part STL files import one part per mesh.
- Many more community documents now rebuild exactly as in Onshape (lofts, shells, Lighten, closed splines).

**Faster**
- Several documents now rebuild faster than in Onshape: a spur gear (1.0 s vs 2.1 s), planetary gears, a circuit board, a sheet-metal louver (4 s vs 5 s; it took 616 s two days ago).
- Display meshing of gears and revolved parts is several times faster.
