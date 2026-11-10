**Fixed**
- Selecting two large parts no longer freezes Noshape (whole parts aren't measured, as in Onshape); measurements, mass properties and center of mass are computed in the background.
- A sketch's Use of a curved (spline) edge shows only its two end points, as in Onshape, not a row of points along it.
- A plane cut through a wall that runs straight in one direction now gives exact straight edges (a 10 mm edge read 10.001 mm).
- Renaming a Variable Studio variable now renames it in Assemblies too; a configuration variable's rename keeps its uses working, as in Onshape.
- Patterns of rounds touching a cylinder are much faster on Windows and Mac (a user's pattern took 10–30 s).
- A user's helical sweeps, fillets running off an edge, and imported STEP edges on small faces now build as in Onshape.

**Closer to Onshape**
- Helices are Onshape's curve, so frames and threads taken from them match Onshape exactly.
- Assemblies: Switch to Part Studio selects the part; a part being placed off a snap returns to how it was inserted.
- Drawings, versions and history: several menus, panels and selections as in Onshape.

**Faster**
- The minimum distance between two parts: 96 s → 0.25 s on a user's model.
- Booleans of parts that share faces or edges run in Noshape's own Boolean (one test document now faster than Onshape).

**Updates**
- Checked by qa on this exact build (gear and ticket documents at their baseline), the land queue, the main-branch tests and the release's own three-platform CI. An earlier v0.4.42 build was stopped before publishing because of a geometry fault in gear-like cuts; this one has the fix.
