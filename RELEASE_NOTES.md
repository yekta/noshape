**Faster**
- The diamond knurl now regenerates in about 9 s (Onshape takes 49 s; it took over 2 minutes before). Any patterned cut (knurls, radial holes, pocket grids, gear slots) gets the same speed-up: the pattern's repeating unit is worked out once and reused.
- Fillets and chamfers in new Part Studios use noshape's own engine: a plate's 400 rounded holes fillet in 0.2 s instead of 8.6 s, and a typical part's fillets are about 2.5× faster. Documents made before keep the old engine and regenerate exactly as before.
- Edits on parts with many overlapping freeform bodies no longer hang.

**Fixed**
- A sketch fillet that shrinks a neighbouring line to a point now solves, as in Onshape.
- Thin lofts of open profiles and Trim ends give Onshape's results.
- Butt joints on closed sheet metal corners, and the flat pattern at closed and butt corners, match Onshape.
- Context menus too tall for the space under the pointer open above it instead of off screen.
- Switching the theme repaints the view cube at once.

**Closer to Onshape**
- Selection fields move on by themselves when they're full, as in Onshape (for example, the printable knurl's two edges, then Targets).
- Ctrl+Space tab switcher with thumbnails; Shift+H and P toggle sketch and plane visibility as in Onshape.
- Copy and paste of parts: Ctrl+V in a Part Studio does nothing, in an assembly it inserts the part at once.
- Sheet metal: the joint table's Type and Style dropdowns, one flat face per wall and bend.
- Curve/surface analysis with knot points, control point grids and details; Body draft results as Onshape's.
- The assembly's Measure panel looks and works like the Part Studio's; mates have the Simulation connection option.
- Drawings: hole, BOM and cut list tables follow the pointer while placed, hole table X direction and exclusions, shaded section views, cosmetic threads from the side, named views with section planes, title block field tooltips and editing, ISO radial dimensions, scales kept as typed.
- Dark mode matches Onshape in many more dialogs and in the sketch dimensions.
- New app icon and favicon.
