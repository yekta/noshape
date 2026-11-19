**Fixed**
- Drawings: switching Drawing properties › Units to Inches fractional or Feet and inches on a drawing with a Ø, R or ± dimension no longer closes the app.
- Hole: a hole started from the part no longer drills up through material that stands above its start.
- Loft: a path bent tighter than its profiles is refused with a message instead of closing the app; lofts along a path no longer fail at random.
- Surfaces: Sweep, Split, Thicken, Boundary surface, Up to next and fit splines build several workshop models that failed before, as in Onshape.
- Sheet metal: a form beside a hole keeps its shape in the flat; pattern and mirror of sheet metal parts form each copy; the flat lies the right way up.
- Configurations › Add configuration input › Checkbox opens its dialog as in Onshape.
- An integer field given a fraction says "Enter a valid expression." instead of taking it.

**Faster**
- Big assemblies on computers without a graphics card draw at coarser detail while moving: zooming shows in about half the time.
- Fillets on lofted and conic surfaces that took seconds before finish in under a second.

**New**
- Derived mirror of an assembly (Mirror strategy › Derived on a subassembly), as in Onshape.
- Centre of mass and inertia overrides for parts, carried into assemblies.
- Sheet metal: Extrude on a flat pattern (Add or Remove), and Add / Remove errors worded as in Onshape.

**Closer to Onshape**
- Sketch: inferred Perpendicular and Vertical on chains and crossing lines, Tangent arc, Trim and Split of used lines, and conflicting dimensions shown red as in Onshape.
- Sheet metal: the flat pattern's DXF / DWG writes exact arcs, circles, ellipses and splines, splits bend lines around holes and includes visible sketches.
- Drawings: hole callouts, the hole table and the inspection lists, the Note panel and ruler, cutting line hover, dimension + marks and edge-on circles, and Drawing properties' units and precision lists as in Onshape; DXF export matches Onshape's text and labels.
- Assemblies: mate dialog highlights, Insert's search and sorting, Esc in Display states, driver mates' first named position, and Ctrl+V placing the copy under the pointer.
- Documents list: column header menus (sort, pin, autosize, choose columns) and sort arrows; the Merge dialog lists tabs in tab-bar order.
- Feature Studio marks errors about a second after typing, with Onshape's red marker.
- Import: STEP / IGES / ACIS rounds and faces merge as in Onshape; 3MF and OBJ imports give one part per object or group; x_t export keeps closed faces whole.
- Section view caps and coplanar faces of two parts draw as in Onshape.
