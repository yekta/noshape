**Sketching**
- Splines and images drag as in Onshape: dragging a spline's body bends it, handles and points snap level or plumb with the origin; an inserted image fits the rectangle you drag.
- Use works on the sketch's own face and on an edge's midpoint; a used edge's glyph sits at its middle, and a sketch on a used edge or face follows the model when it changes.
- A point dragged onto a model edge lands on it; arcs no longer take a stray horizontal/vertical through their own start.
- Big sketches: slots, conics and linear patterns respond several times faster (pattern 180 → 35 ms).
- Variables: renaming one offers Update all references, and wrong values and failing dimension expressions are reported in Onshape's words.

**Part Studios**
- Named views, display modes, Measure, feature-list drags, reorder/rollback/delete undo and every Onshape shortcut behave as in Onshape on Windows, macOS and Linux.
- A sketch region on a face under a feature's preview is picked before the face; list-row tooltips and the filter box help appear as in Onshape.
- Hole's Tap type lists Onshape's pipe taps.

**Assemblies**
- Mates: Tangent and other mates the assembly can't hold turn red as in Onshape, a one-pick Tangent is made in error with Onshape's message, and Insert thumbnails follow the chosen configuration.
- Change configuration, subassembly rows, zoom to fit beside Insert, row rename on Tab and the pattern's flip key as in Onshape.

**Import and export**
- STEP export keeps each face's colour, so a re-import shows them; an assembly's STEP export no longer shows Part Studio-only fields.
- STL export no longer leaves a broken edge on some curved parts.
- Linux: install.sh makes Noshape the app for .noshape files on a stock desktop.

**Faster**
- Opening a document from the start page is quicker, and a big impeller's first open went from 43 to 11 s; Move face on screws ~100x faster.

**Fixed**
- Shell of a filleted part, Move face between square neighbours, and a round that used to crash now fails with a message.
