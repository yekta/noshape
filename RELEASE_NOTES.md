**Sketching**
- Dragging sketch geometry feels as in Onshape: it starts after 3 px, keeps the spot you grabbed, hides region fills while it moves, and one undo takes back one drag.
- An over-defined or redundant dimension fails the sketch as in Onshape, while features built on it keep their last shape.
- In a tilted view, dimension values and the Offset distance lie in the sketch plane as in Onshape.
- Pattern labels, the Text dialog, constraint glyph placement and arc snaps to their own centre match Onshape.

**Closer to Onshape: hover, selection and dialogs**
- Move face's arrows and rotation ring sit and drag as in Onshape (4 px dead zone, 5° ring snaps), and Transform's drag undoes in one step.
- Hovering and picking with a dialog open, under a preview and in a sketch with a model selection match Onshape.
- Part outlines are drawn with Onshape's weight.
- Notices show one at a time and wait their turn, as in Onshape (they stacked in 0.4.69).

**Assemblies**
- Unresolved mate connectors are named as in Onshape.

**Import and export**
- Update… of an imported file re-imports it in the same Part Studio.
- Exporting several parts offers one file per part.
- Derived picks the parts you click in a Part Studio as Onshape does.
- STL meshes: Move face Offset and Translate work, and Extrude can stop on a mesh face.
- STEP export keeps the colours you give single faces.
- Imported face colours have their own row in the Appearances panel.

**Faster**
- Patterns of Booleans fuse up to 4x faster (impeller seats 0.9 s to 0.25 s), and many Booleans on curved parts are quicker.

**Fixed**
- Ribs that end on a part's edge build as in Onshape.
- A typed Hole size shows Onshape's "parameters overridden" note while the dialog is open.
