**Fixed**
- Switching back to an already-open tab in a large document is fast again (it took about 0.7 s since v0.4.54).
- Sheet metal: a model with a rolled (curved) wall joined by a bend unfolds again and has its flat pattern.
- macOS: Ctrl+Enter in a feature dialog's value field applies the value, as Enter does.
- Windows: double-clicking a face and stepping the view with the arrow keys work as on the other systems.
- Documents saved with v0.4.58 that use Move face next to other features keep their references (a fillet after it no longer loses its edges).
- Exporting or importing a large file no longer freezes the window while the file is written or read.
- Fillets: a round wider than its face along a curved edge now builds instead of failing.
- A model vertex on the sketch plane snaps to its exact position (34.75 mm, not 34.7499996 mm).

**Faster**
- Parasolid (x_t) import of models with blends is about twice as fast (one test model: 17.5 s to 7.9 s; Onshape 8.6 s).
- More Boolean operations (threads, gear teeth, sprockets, patterned cuts) run on Noshape's own Boolean, several times faster.
- Agent API drawing operations on big drawings are about 2.5 times faster.

**Closer to Onshape**
- Sketch: a lit snap target stays lit while the pointer is still on it, tangent snaps onto circles reach as far as in Onshape, Trim and constraint icons light at Onshape's distances, a double-clicked face's sketch shows its constraint colours.
- Feature list: the rollback bar drags from 6 px, a row clicked twice opens its dialog, the summary and Mass properties keep their old values until the new ones arrive.
- Numbers are shown with Onshape's rounding and unit spacing.
- Assemblies: snap mode, triad drags (step sizes, Shift to move freely), exploded and section views, mate limits and named positions behave as in Onshape.
- Drawings: toolbar tooltips and help popovers, hatch pattern list, custom sheet sizes with zones, fractional inch values stacked, table hover and selection, section view labels, centre marks only on whole holes, DXF export writes tori and free-form outlines as splines.
- Export offers the Inspection list format (CSV); importing a bare assembly file (SolidWorks, Inventor, CATIA, Creo) explains what to send instead.
- Variable Studio, Feature Studio, configurations, versions and workspaces: timing of error marks, tooltips and dialogs as in Onshape; two workspaces may share a name.
- Surface Extrude makes one surface per chain of curves; Loft along a path and Sweep make Onshape's faces.
