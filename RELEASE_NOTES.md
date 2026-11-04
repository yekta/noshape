**Fixed**
- Lengths typed in inches are now exactly Onshape's (an inch is 2.54 cm in FeatureScript too), so imperial features build the same.
- Undo after typing values into a feature dialog goes back to the right state again.
- A sketch whose referenced geometry is gone warns "Some external references are missing.", as in Onshape.
- A hose or tube without its end caps now shells in under half a second.

**Closer to Onshape**
- Fillets on gears, pulleys and knobs make the same faces as Onshape; more fillets are built by noshape's own code.
- Features fail with Onshape's own messages in sheet metal, Revolve, surfaces, curves and Measure too.
- More community FeatureScripts build: sheet-metal Tab and Corner Overcut, Shaped Flange, patterns with Reapply features, hose routing.
- Feature patterns and mirrors of touching features match Onshape.
- Sketches: Show constraints is off by default; spline end handles can be dragged and dimensioned; Offset and Mirror behave as in Onshape; sketch resizes match Onshape in more everyday shapes.
- Assemblies: the mate dialog's Undo / Redo take back connector picks; two connectors on one part are accepted and flagged; a pin with a Revolute mate lands where Onshape puts it.
- Drawings: ANSI, ISO and new JIS title blocks laid out from Onshape's templates; suggested views placed as Onshape places them; selectable PDF text.
- Parasolid import: fan hubs and screw threads come in with Onshape's faces and lengths.

**Faster**
- A planetary gear set regenerates 7× faster; spline-heavy parts about twice as fast.
- Some spring and thickened-surface documents regenerate in under 2 seconds instead of several.
- Failing fillets give up in under a second instead of several.

**Updates**
- If you have v0.4.21 or later, this arrives through the in-app updater.
