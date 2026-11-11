**Fixed**
- A plane cut through a wall that runs straight in one direction gives exact straight edges: a user's 10 mm window edges now read 10.000 mm. The v0.4.42 notes listed this too early; this build has it.
- Chamfers that OCCT can't build are now built the way Onshape builds them.

**Closer to Onshape**
- Sheet metal: a sketch on the flat pattern is drawn and edited inside the flat view, and the flat lies where Onshape lays it. Selecting a flat face highlights the part face it came from.
- Assemblies: Edit in context on a part from another document opens that document with the assembly as context. Its bar's "Create version and update in assembly" makes the version and moves the part to it.
- FeatureScript notices: clicking a notice folds its stack. The stack's rows are the links to the code, as in Onshape.
- Compare and Versions: picking and highlighting as in Onshape; Escape closes Edit definition….

**Faster**
- Delete face on a prism's rounds is edited in place: one test document 602 ms → 194 ms.
- Extrudes up to a surface of revolution take its circles exactly and much faster.

**Updates**
- Checked by qa on this exact build: gear and ticket documents at their baseline and one more feature matching Onshape in the model suite. The main-branch tests and the three-platform CI are green.
