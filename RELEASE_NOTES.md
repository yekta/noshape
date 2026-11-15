**Closer to Onshape**
- Mirror: the Create selection panel looks and behaves as in Onshape (picking a face lights up its feature, Add selection puts typing in the panel), and the assembly Mirror strategy table matches Onshape's layout.
- Number fields: typing a value with a unit and pressing Tab now applies it, so the preview runs and one ✓ commits (before, the value was only applied by ✓ and the dialog stayed open).
- Drawings: revision tables, note leaders, broken-out section outlines, crop outlines and inspection bubbles export as Onshape's; profile tools snap to edges from the same distance as Onshape.
- Exported STEP and IGES files of sheet-metal parts open in Onshape with the same faces as Onshape's own export.
- Offset curve, fit spline, Move boundary and Face blend mark the field at fault as Onshape does.

**Fixed**
- Noshape no longer crashes on quit after switching between Part Studios and an assembly.
- A sketch on a slow model no longer leaves a spinner over the view until the sketch is closed.
- Several drafts, holes, fillets and chamfers now build as in Onshape.

**Faster**
- Shell and Boolean on curved faces are faster (shells and patterns on tori and B-spline faces, face areas).
- Custom features open faster than Onshape's (first use of a document's custom feature 98 → 42 ms).
- Fillets, chamfers and lofts on free-form faces are faster.
