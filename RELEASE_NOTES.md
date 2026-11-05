**Fixed**
- The parts and feature lists' rows span the panel again: the hover highlight covers the whole row and the eye is back at its right edge (v0.4.29 sized rows to their text).
- Opening a select (Configurations, a dialog's End type…) no longer moves anything around it.
- Windows: an update that can't be installed (a coding agent's noshape connection holding the files, or the permission prompt declined) now closes the other noshape copies first, and if it still fails, noshape starts again and says why instead of nothing.
- Every OS: a failed update reopens the previous version with the reason.
- A noshape Boolean stops at its time limit like OCCT's does.

**Closer to Onshape**
- A gear's full root fillet is one face, as in Onshape (the community Spur Gear now matches Onshape feature for feature).
- A Tab merged into a flange at a ripped corner fills the corner, as in Onshape.
- Drawings: Ctrl+S (sheet menu) and Ctrl+M (max/min dimension) take the Control key, as in Onshape.

**Faster**
- Opening, previewing and cancelling a feature dialog builds its 3D scene in the background: the UI stays responsive (about 1 ms instead of 60–70 ms on a big part).
