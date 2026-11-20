**Fixed**
- Surfaces: a sketch on a mirrored or patterned surface solves again (the workshop "singularities" model's Sketch 2 failed in 0.4.61).
- Fillets that the kernel built inside out (the car model's Fillet 1) are repaired instead of refused; failed fillets and chamfers say "Failed to fillet selections." as in Onshape.
- Chamfers on an arch's rim and on a slot's concave corner come out as in Onshape.
- Loft along a closed path through three or more profiles closes into a ring, and surface lofts along guides take more than two open profiles.
- STEP export of a filleted round keeps its face whole; mesh parts export their outline.
- Sheet metal: a cut across a wall leaves both pieces as sheet metal.

**Closer to Onshape**
- Sketch: a line started square to another line's midpoint is Perpendicular; a line's horizontal / vertical lets go at Onshape's distance.
- Assemblies: dragging an instance by a face moves it without turning it; mate connectors light at Onshape's distance; named positions and display states behave as in Onshape.
- Drawings: the Measure panel, Replicate annotations, note ruler and many dialogs (View properties, Move to sheet, Export, Datum target, Hole table) look and act as in Onshape.
- Toolbar tooltips stay through a click and leave on release; a click beside an open menu only closes it.
- Documents: pinned table columns stay put while scrolling sideways.
- The app's own names (profile, mouse preset, Material Library, BOM template, Labs) say Noshape.

**New**
- Centre of mass and inertia override in the Mass panel.
- Routing curve: the centre ball snaps to points on the hovered face.
- Drawing Export as DXF/DWG has Onshape's units options.
