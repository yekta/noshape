**Updates**
- The update notice is now a small card floating at the bottom of the window instead of squeezed into the top bar. It covers no controls and can be dismissed.

**Fixed**
- A boss added on a drafted block's face no longer turns the part inside out.
- A sketch region split by a line now extrudes.
- Extrude Up to face ends on the face's plane even past the face's edges, as Onshape does.
- Some Shells that gave a nearly empty result now build as in Onshape.
- A fillet that can't build fails in about 3 seconds instead of 30.

**Closer to Onshape**
- Fillets, chamfers and full rounds make the same faces and edges as Onshape (one face along a grooved edge, one sphere at a full round's corner).
- Holes across a tube's seam, and sketches on part faces, line up as in Onshape.
- Features fail with Onshape's own messages, shown at the field at fault.
- More community FeatureScripts compile and build.
- Shift+P hides the origin, planes and sketches as one undo step; Ctrl+click on a feature opens its menu.
- Sketch resizing and drags match Onshape in more cases; a part held by one Ball mate turns about the ball when dragged.
- Drawings: centre marks on holes, spline edges in DXF and DWG, the ANSI title block's projection symbol, and suggested views at the right scale.

**Faster**
- Shell on large parts is about 30% faster; some thickened surfaces compute mass properties in seconds instead of minutes.
- Assembly drags need fewer solves per step.
