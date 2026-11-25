**Sketching**
- Constraints, fillets and dimensions you add stay and solve even while another part of the sketch has a conflict, as in Onshape.
- Big sketches stay responsive: drags, dimension edits and tool edits no longer freeze the window while the sketch solves.
- Insert DXF/DWG works as in Onshape: placed at once, centred on the origin (or at the file's coordinates), unsupported text and points reported, one undo removes it all; large files insert much faster.
- Dimensions light up and can be picked from their lines too, not only their value; construction circles, arcs and splines are dashed; the Offset box and preview, the centre point arc guides and two-line text look as in Onshape.
- Fillet and chamfer between two picked lines, the chamfer's second value box, Text's Enter for a new line and many smaller tool behaviours match Onshape.

**Closer to Onshape: hover, selection and dialogs**
- Hovering edges, faces and vertices next to a selected face or edge looks as in Onshape; the move manipulator, transform triad and fixed-part glyph are drawn smoothly.
- Feature dialogs: wrong values and wrong picks are shown as in Onshape (only the input at fault turns red, "missing selection"), flip buttons, field limits and the feature-row tooltip after a double-click match Onshape.
- Notices stack instead of replacing each other.

**Modelling**
- Split pieces, patterns and mirrors keep their parts' names and colours; faces cut by a Subtract take the target part's colour.
- STEP imports keep their face colours.
- More fillets build where they used to fail (rounds that turn from convex to concave, notched faces); a coiled wire sweeps.
- Changing an Extrude's depth on complex parts no longer takes up to 15 s to decide Add or New.

**Faster**
- Several Booleans and Move/Split features on screws, sprockets, threads and patterns are 2-10x faster; a nut's thread cut now beats Onshape's time.

**Fixed**
- The Shell of some lofted parts on Mac.
