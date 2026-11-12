**Fixed**
- Documents saved with an earlier Noshape open as before again. v0.4.44 could lose a sketch's plane, or another reference, that pointed at a face merged after a union; a user's model lost two sketches, their extrudes and a fillet. They resolve again, and a reference never moves to a different face.
- A feature that would produce an inside-out solid now reports an error instead of keeping a broken part.
- In the Feature Studio, the first character typed into a field is never lost.
- Show in Explorer on Windows opens the folder with the document selected.

**Closer to Onshape**
- Sketches: each curve and point is its own body, as in Onshape, so deleting, moving or patterning one curve acts on that curve alone. A sketch on a face with a hole has the hole as a region too. A polygon is one circular-pattern constraint.
- Sheet metal: your own forms library (and the other four library types) can be set on a folder, with the Form picker's levels and search as in Onshape.
- Assemblies: new mate connectors on a circular edge point the way Onshape's do (saved ones keep their direction, so saved assemblies solve exactly as before). Parts inside a flexible subassembly can be moved per instance.
- Imports: IGES files with several named solids import one part per solid; STEP, Parasolid and IGES assemblies can be split into documents as in Onshape.
- Chamfers on patterned copies, fillets rolling over a tube's end and the Feature Studio's undo steps match Onshape.

**Faster**
- Long lists stay quick: the Variable Studio with hundreds of variables, the documents page with a thousand documents and long version histories build only the rows on screen (cell clicks 283 → 33 ms; sorting 1,000 documents 350 → 40 ms).
- Escape and Ctrl+A answer immediately; entering a sketch, opening a dialog and clicking a feature in a busy list all fit in one frame.
- Zoom to fit frames curved (B-spline) parts tightly instead of far out.
- Several fillets, chamfers and up-to extrudes are faster.

**Updates**
- Checked by qa on this exact build, now including documents saved by the previous releases and three users' own documents, rebuilt feature by feature against v0.4.43.
