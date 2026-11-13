**Closer to Onshape**
- Sketch snapping rebuilt from Onshape's own rules: points snap inside a 15 px square, lines within 7 px, perpendicular and tangent reach follow the line's length, model edges and vertices only in the sketch plane, Shift turns snapping off.
- 3D picking, the feature list, the Parts list, the measure tool, navigation (wheel steps per OS, view animations, double middle-click fit), menus, tooltips (400 ms) and feature dialogs (they reopen where you left them) behave as in Onshape.
- Part edges are drawn at Onshape's 1 px.
- Properties… on several parts opens one dialog for all of them, with "Multiple values" where they differ.
- A sketch fillet or chamfer dragged from its corner is made on release; dialog arrows grab within 14 px and step with the zoom.
- Documents open their window at once and load in the background, with Onshape's loading steps.
- Variable Studio grid keys and row drag, Feature Studio editor keys, Versions panel and configuration tables as in Onshape.
- Import defaults as Onshape's (STL in millimetres, IGES postprocessing).

**Fixed**
- A Remove extrude picked quickly (Remove, then Through all) cuts into the part again.
- An oversized fillet or chamfer fails as in Onshape instead of leaving an empty part.
- A very fine Custom STL export no longer needs gigabytes of memory (a 0.002 mm export: 35 s / 12.5 GB → 4.3 s / 380 MB).
- STL imports no longer merge a part into another's hollow; STEP and IGES curved edges read exactly.
- Several assemblies place more mates as Onshape does.

**Faster**
- Sheet metal regenerates up to 3× faster (the louver test part now as fast as Onshape).
- Fillets, chamfers and sweeps faster; more Booleans run on Noshape's own code.
