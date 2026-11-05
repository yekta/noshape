**Fixed**
- Export dialogs (part, Part Studio, assembly, drawing) open on your last export's format and options, each format keeping its own; Cancel saves nothing.
- A cut tangent to a face fails as in Onshape; two sketch regions side by side extrude to the right faces again.
- A part mirrored over a hole merges its halves into one face, as in Onshape.
- Custom features use the Part Studio's kernel settings.

**Closer to Onshape**
- Spline fits use Onshape's knot spacing, so imported and FeatureScript splines match Onshape to 1e-4 mm.
- Sheet metal: Make joint, Flange alignment and Move face can extend a sheet up to a surface; rips on rolled walls; folds keep their corner breaks.
- Mate connectors at an edge's midpoint or a face's centre sit where Onshape puts them.
- Stretched (non-uniformly scaled) parts and sheets keep their flat faces and straight edges.
- More community FeatureScripts and models match Onshape (49 of 56 test documents to the end).

**Faster**
- The community spur gear regenerates about twice as fast; hose routing 1.1 s → 0.7 s; a fill of a sheet-metal fuse twice as fast.
- Opening a feature dialog redraws only its own toolbar button.
