**Faster**
- The diamond knurl at 45° tooth and helix angles now builds, in about 20 s (Onshape takes about 2 minutes); 30° tooth knurls take 4–9 s instead of 40–180 s.
- Cancelling a dialog while its preview computes stops the work within a fraction of a second.
- noshape's own fillets now handle fillets that run round smaller corner rounds, about 10–20× faster than before; Draft's reapplied fillets 5.6 s → 0.1 s.

**Fixed**
- A Feature pattern of through holes with Reapply off failed again in v0.4.18.
- A sheet metal enclosure's cut removed slightly too much material.
- Two fillets exactly as wide as their face now make one rounded face, as in Onshape.
- Thin lofts with or without Trim ends give Onshape's shapes.
- Mate connectors on circular edges point out of the material, as in Onshape.

**Closer to Onshape**
- Sketch: the Flow constraint (smooth curvature continuity), Curvature and Flow between splines and arcs at normal sizes, the Constraint manager and Profile inspector as Onshape's, configured sketch dimensions.
- Assemblies: full right-click menus for patterns, mirrors, relations and mates, several mates selected at once, dynamic suppression, groups that show and hide like mates, snap mode places and fastens parts.
- Part Studio: many dialog fields, tooltips, ranges and menus checked against Onshape and matched.
- Drawings: MBD dimensions from the Part Studio, custom properties and property fields in notes and title blocks, BOM tables from other assemblies or data files, datum target directions and custom areas, hole callout precision overrides, the drawing's Properties dialog.
- On Windows, `noshape` commands typed in a terminal print their output; uninstalling disconnects the coding agents it connected.
