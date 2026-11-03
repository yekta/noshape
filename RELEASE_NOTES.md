**Fixed**
- Typing `#wall` and Enter in a dialog now takes the first match (`#wall_thickness`), also in a dialog opened before the variable existed.
- Deleting a feature in some documents with fillets or chamfers could crash the app.
- In a sketch, `a` is the 3 point arc and `q` toggles construction again; Undo and Redo follow the sketch's own history.
- A second Enter in a dialog accepts it again instead of re-applying an empty value.

**Closer to Onshape**
- Sketches resize and drag as Onshape does in many more everyday cases (rounded corners, chamfers, slots, bolt circles, triangles, hexagons); a drag ends where you let go, whatever path you took.
- Dragging parts in an assembly also lands where Onshape puts them.
- Clicking where datum planes overlap picks the nearest one; the Sketch dialog's "Select a sketch plane" stays until you pick one; right-clicking a feature outside the selection opens the menu for both.
- While a feature is edited the rollback bar stays where it is.
- IGES files keep names, colours and assemblies as Onshape's do.

**Faster**
- Undo and redo far back in the history no longer rebuild the model.
- Sketch drags on big sketches are smoother.
- Large drawing exports (SVG, PNG, JPEG, TIFF) are much faster.

**Updates**
- If you have v0.4.21 or later, this arrives through the in-app updater.
