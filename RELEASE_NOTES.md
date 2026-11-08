**Fixed**
- Exported STL and other mesh files are always clean closed surfaces (some had folded or missing triangles), and STL files with a fold read back as solids.
- Extrude "up to face" with an offset on both sides now cuts to the right depth.
- A sketch region with no area fails its feature with a message instead of producing a broken part.
- Valve, snap-hook and other community models now rebuild exactly as in Onshape (valve handle cuts, snap-hook fillets).

**Closer to Onshape**
- Spline handle arms: constraints that don't apply now refuse with Onshape's message instead of being greyed out.
- Fillets, chamfers and face blends: full rounds on drafted and tapered ribs, partial fillets ending at a plane, Face blend propagation, Detached, trim options and surface walls, chamfer measurements, as in Onshape.
- Lofts: Normal-to-profile starts and guided lofts match Onshape's shapes.
- Frame can use a sketch from another Part Studio; Cut list tables as in Onshape.
- Drawings: views of another document's version, Switch to reference, Crop and Detail views from an existing closed curve.
- Documents page, Variable Studio, Feature Studio and versions: renames, Move to document… (with referenced tabs), Compare, Help menu with an offline user guide, FeatureScript library and System check; new Feature Studios use Onshape's current standard library (3112).
- Section view: hovering a cut highlights all caps of its plane, in Part Studios and Assemblies.

**Faster**
- Opening noshape: the document loads while the app starts, fonts are found faster, and Mac graphics pipelines compile in parallel.
- Unify after Booleans skips work it already knows can't help (several features 2–4× faster); snap-hook and valve documents much faster.
