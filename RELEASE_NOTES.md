**Faster**
- Diamond knurls at any size: a Ø25 × 25 mm 45°/45° knurl that ran for minutes now builds in about 3 s (Onshape takes about 17 s).
- Shell builds its inner walls directly for parts made of planes, cylinders, cones, spheres and tori, including thin fillets and parts with voids, instead of the slow fallback construction.
- Opening and displaying imported parts and knurls is several times faster: freeform faces are meshed by noshape's own mesher and parts mesh in parallel (the NIST test model 3.2 s → 0.3 s).
- Big assemblies regenerate 3–4× faster (1000 instances 3.7 s → 0.8 s), and dragging parts in them is smoother.
- STEP import of large files about 3× faster; STEP export of 2000-part assemblies about 2× faster.
- An extrude that could run for minutes on a drafted part now finishes at once.

**Fixed**
- Chamfering or filleting one face of a part made by Split changed the matching face of every piece.
- On Windows, opening a document no longer flashes a console window.
- A custom feature's construction planes stayed visible after the feature deleted them; planes a feature keeps can now be shown and hidden like Onshape's.
- Shell: a pocket's floor corners are now mitred as Onshape does (the wall used to thin to a point at each corner where the fallback construction built the shell); some shells of complex parts change volume slightly.
- A Draft that can't succeed fails at once instead of after seconds of work.

**Closer to Onshape**
- Variable table: # autocomplete of variables, values show their result rather than the formula, resizable columns, rows reorder by dragging, the type dropdown converts as Onshape's.
- Drawings: model-based definitions exported to STEP AP242, title block fields as notes, format lines selectable one by one, hole tables across several views, one dimension palette for several selected dimensions, more Styles options, Paste as table on Windows and Wayland, the notifications bell.
- Import: Import files takes several files with Onshape's three import options; a STEP assembly's subassemblies keep their own tabs.
- Documents: Tab › Properties, Document details and Copy workspace / version as Onshape's; more drag handles on features.
- Export: custom mesh resolution for STL and other mesh formats.
