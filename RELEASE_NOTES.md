**Faster**
- noshape's own Boolean now also handles tilted holes and cuts (elliptical edges), cones, hyperbolic and parabolic sections, and freeform faces (B-spline, torus, sphere, swept): more of every model regenerates through it, 10–20× faster per Boolean than before.
- noshape's own display mesher is on: flat, cylindrical, conical and spherical faces are meshed for display about 3.5–4× faster when opening or editing a part.
- Sheet metal rebuilds about 4× faster.
- Edges render faster: silhouette candidates are culled on the GPU.

**Fixed**
- A Feature mirror of a boss copied a column the part's full depth instead of the boss.
- Mirroring blind holes and half cones left extra faces; split cylinder and cone faces now merge into one, as in Onshape.
- Two overlapping parts that a union treated as separate are merged.
- A crash inside a modelling operation can no longer freeze the app.
- Sheet metal: a second flange edge couldn't be picked; older documents lost their sheet metal; cuts and holes vanished after a later flange.
- A Rib from a closed profile gives Onshape's error message.

**Closer to Onshape**
- Sheet metal: partial flanges, butt joints, closed corners, Jog and Tab match Onshape; the flat pattern exports through Onshape's DXF/DWG dialog.
- Standard content: Onshape's whole catalogue, including all 52 PEM® fasteners and NAS self-locking nuts.
- Sketch: Coincident glyphs where curves meet, sketch text in Onshape's Open Sans.
- Drawings: hole callouts and dimensions follow Onshape's rules, Styles panel options for callouts and datums, the Edit section dialog, continuation title blocks, dragging views and tables between sheets, light panels in dark mode.
- Selecting a mate connector shows its position in the status bar.
