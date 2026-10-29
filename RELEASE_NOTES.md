**Faster**
- noshape's own Boolean (written in Rust) is now on by default for parts made of flat faces, cylinders and coaxial cones: each Boolean on a small part is about 25× faster than before, so features after an edit regenerate faster (an edit on a 30-feature part about 35–45% faster). Anything it doesn't handle still uses the old engine, with the same results.
- Shell runs many of its internal steps through the faster Boolean.
- A small feature on a large part (like a fin on a 400-fin plate) rebuilds only the area it touches, and faces an edit leaves alone keep their display meshes.
- Extrudes of sketches with many regions build in parallel.
- First launch on macOS is faster: shaders are precompiled.

**Fixed**
- A Boolean could crash the app in rare cases, and some documents with drafts never finished opening.
- Fillets that roll over an unfilleted edge and overlap another round now fail like Onshape's instead of giving a wrong result.
- Renaming a part in its Part Studio renames its assembly instances too.
- The right panel strip no longer lets clicks through to the model under it.

**Closer to Onshape**
- Sketch: F fits the sketch, Tab between value boxes, Use with preselected edges, line inference from an arc's end, fillets keep their own radius, dimension placement next to constraint glyphs, no hint text.
- Part Studio: folders and the rollback bar, parts list properties, Where used panel, Measure's asterisk for near-zero values, feature dialogs mark the parameter an error blames, count fields' two-step Enter.
- Assemblies: Select other, "Mate overdefines the assembly." and its error badges, "No interferences", Move to new subassembly carries patterns, mate limits shown as one value, triad menu.
- Drawings: dimensions re-attach by their grips, diameter from a cylinder's silhouettes, angular and arc length dimensions follow the pointer, typed dimension values, datum targets, view and scale labels, Sheets panel reordering, export banner.
- Parasolid export keeps part names and colours.
