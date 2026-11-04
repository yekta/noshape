**Fixed**
- A chamfer, fillet or other pick of one face that a Split (or a Remove) cut through several parts now changes only that part. Before, picking one ring's cut face picked every ring's.
- Replace face works with a dense surface: the Displacement map with Replace face on builds on a cylinder's side in about 3.5 s and on its top in about 6.6 s (Onshape: 11–12 s), with Onshape's volumes.
- Replace face that can't be done stops with Onshape's "Could not replace faces as requested." instead of running for minutes.
- UI tests no longer depend on the computer's saved panel sizes (for developers).

**Faster**
- Large B-spline faces (like a displacement map's) mesh in a fraction of a second instead of minutes.
- More Booleans in community documents use noshape's own Boolean: a spur gear's tooth cut 8× faster than before, a fill pattern's region cut 5× faster.
