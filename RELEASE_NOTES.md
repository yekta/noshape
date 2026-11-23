**Fixed from your reports**
- Double-clicking a .noshape file opens that document in Noshape on macOS, Windows and Linux; macOS lists Noshape for the file type, and a document that is already open comes forward instead of opening twice.
- A document copied or renamed in Finder is named after its file, no longer shown as a copy; leftover side files are cleaned up.
- Import lists .step files (and every other file) on macOS.
- A download that works shows no error toast; only a failure does.
- STEP imports keep their colours as part colours, with face colours staying on their faces; solids are named as in the file.
- A Boolean union keeps the face colours of every part it merges.

**Closer to Onshape: hover, selection and dragging**
- Sketches: the dimension box applies only on Enter, Trim and Extend previews, the length box position, and the Select other key match Onshape; mate connectors are inferred as in Onshape in Part Studios and assemblies.
- Part Studios: plane names keep their size at any zoom, picked edges stay lit under a preview, out-of-range values say their range.
- Assemblies: an over-defining mate moves nothing and turns both mates red; error mates, gears and selection as in Onshape.
- Drawings: dimension drags, table cells, detail views and hover distances as in Onshape.
- Documents page, Variable Studio and Compare: menus, row drags and highlighting as in Onshape.

**Faster**
- Booleans in patterns and imported parts are several times faster (one circular pattern from 15 s to under 1 s).
- Move face on an imported part: about 0.2 s instead of 1.4 s.
- A file tab's Import… opens at once.

**Fixed**
- Shell on free-form faces builds valid parts again.
- Errors match Onshape's for Boolean, Hole, Revolve and Loft.
- Sheet metal: hems at a corner unfold correctly; frames' trims, end caps and gussets match Onshape.
- STL files that close import as solids; SAT edges stay on their faces.
