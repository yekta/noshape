**Faster**
- Booleans on curved and free-form faces do more of their work in Noshape's own code: cuts across cones and revolved faces, and planes slicing free-form faces, are quicker (one valve feature went from about 2 s to under 1 s over ten runs).
- Splitting and up-to extrudes against large faces only work on the part of the face they touch, and Rib builds all its ribs in one step.
- Closing a document now releases its memory, and opening and closing documents repeatedly no longer grows the app's memory.
- On a shared Mac, Noshape's own background builds run at the lowest priority.

**Fixed**
- In a sketch, hovering a line or point now highlights it even with a plane behind it, and the highlight follows the pointer without going stale. While a drawing tool is active, nothing pre-highlights before the first click.
- A new Fillet in a new document no longer opens with a radius from another document that uses a variable from there (the field and the edges turned red). Plain values are still remembered, separately for metric and inch documents.
- Search tools' "Highlight tool in toolbar" no longer shifts the toolbar.
- Patterns and mirrors of surfaces no longer close up into extra solid parts (an assembly's BOM showed an extra row).
- A sketch on a part's face that was moved within its plane stays where it belongs.
- More fillets, rounds across strips, guided lofts, Move face offsets, Thicken and Delete face build as in Onshape.

**Closer to Onshape**
- Sketch: picking two entities for a constraint moves the second and holds the first; Pierce keeps a circle on the curve; selection readouts measure arcs as arcs.
- Drawings: suggested views pick Onshape's scale on more sheets; DXF exports write more edges as exact lines, circles and ellipses.
- Toolbar dropdown lists are spaced as in Onshape; Query variables' Fillets and Adjacent select as in Onshape.
- ACIS exports of round faces read back as the model.
