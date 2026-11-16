**Closer to Onshape**
- Toolbar: pressed and hovered buttons look like Onshape's, icons keep their white faces over the highlight, and the toolbar dropdown lists have Onshape's width and spacing. Shadows on menus, lists, dialogs and tooltips now match Onshape's softness.
- Sketches: the move/rotate handles appear where Onshape puts them, clicking an arrow's shaft grabs it, circular patterns run clockwise as in Onshape, pattern labels sit where Onshape's do, and snapping to a model edge and to spline points behaves like Onshape.
- Drawings: the title block's TITLE has three separately editable lines, view dragging keeps aligned views together, and drawing DXF exports have Onshape's layouts, tangent edges and arcs.
- Sheet metal: corner breaks, bend strips, rolled walls in the flat pattern and the flat view's bend lines and hover match Onshape more closely.
- Holes "Up to next", lofts to a point and along a path, Offset surface and several Mirror and Extrude cases build as in Onshape.
- Standard content: bolts and screws offer Onshape's thread lengths and are named with them.
- A failed feature's part is highlighted in place, as in Onshape.

**Fixed**
- A document with a face split by several sketch curves no longer freezes while regenerating.
- Exporting a mesh part to Parasolid, ACIS, IGES, JT, Rhino or PVZ no longer crashes the app.
- Parasolid, ACIS, STEP and IGES exports and imports read back correctly in more cases, an OBJ zip from Onshape or Noshape imports with its colours and units, and a failed import now says so.
- Expressions can compare text and use configuration checkboxes.

**Faster**
- Fillets that fall back to the slower path are much faster (one case went from 2.2 s to 0.35 s).
