**Faster**
- Parts with curved free-form faces regenerate faster: Booleans reuse work across features, faces that can't meet are skipped sooner, and Split, Revolve, Mirror and Shell spend less time in the geometry kernel (on a Mac, Split and Revolve run up to twice as fast).
- Importing Parasolid files with blended tubes is more than twice as fast (one case went from 7.1 s to 3.1 s).
- Fillets check their result more quickly.

**Fixed**
- A threaded part whose sweep sent the face merge into an endless loop no longer uses up all memory.
- More lofts along a path, sweeps, splits by a cylinder and features on round faces build as in Onshape.
- Parasolid, ACIS and JT exports of round and offset faces read back as the model, and an ellipse with equal radii is a circle, so revolved rings come out as tori.

**Closer to Onshape**
- Drawings: suggested views fit the Top and Right views to the sheet too, so more drawings get Onshape's scale; DXF exports write straight edges as lines and round edges as exact circles and ellipses.
