**Faster**
- Patterns of cuts and holes regenerate much faster: noshape's own Boolean recognises copies of the same tool and works out their intersections once (a pocket grid about 17× faster, gear slots about 19×, a 999-instance pattern about 15% faster).
- noshape's own Boolean now handles more shapes: surface sheets, edges lying on freeform faces, and patterns that reach into the part.
- Edits, undo and redo respond sooner: a single regeneration starts at once (undo is now about 36 ms).
- Opening and editing large parts is faster: edge meshes are built in parallel.
- Sheet metal rebuilds faster again (a hem's rebuild lost 2 s of wasted work).

**Fixed**
- A diamond knurl no longer fails; it now builds, though it is still slow.
- Custom features that fail now show their error on hover, and the FeatureScript notices panel lists it.
- A sheet metal bend under 45° unfolded the wrong way in the flat pattern.
- Counterbore and countersink holes on sheet metal disappeared after later sheet metal features.
- Mirroring with Add and Merge with all could hang on large imported parts.
- Fillets and chamfers wider than a face fail as in Onshape instead of giving a bad result; a draft of a face parallel to the pull matches Onshape.
- ACIS import no longer shortens some edges.
- Long modelling operations are stopped by their time budget on macOS and Windows too, not just Linux.

**Closer to Onshape**
- Sheet metal: flat view selection, bend line menus and Select other, form tears and outlines in the flat and in drawings, the form library picker with previews, Copy Flat pattern into an assembly, Create drawing of a flat pattern, bend table letters as Onshape's.
- Part Studio: Show isocurves on Loft, Boundary surface, Fill and Fillet; Show ruled lines on Ruled surface; Fill on open edges uses Add with Merge with all; a rib from an open chain makes one rib per edge; Measure reads a partial cylinder's radius.
- Sketch: dimensions between two free conics, Beziers or splines are refused as in Onshape.
- Assemblies: Move to new subassembly and Dissolve subassembly turn Undo off, as in Onshape.
- Drawings: surface finish dialog and symbols, GD&T and surface finish leaders with nodes and grips, broken views carry into section views, aligned and broken-out section panels, Exclude from cut, BOM templates, DWT templates, cut list tables, hole table options, hovered BOM and hole table rows light their parts, table grips and resizing, dimensions and section lines re-attach to model points and turn red when lost.
- Imports show Onshape's messages on success and failure; selectable PDF text matches Onshape's files.
