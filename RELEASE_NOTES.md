**Fixed**
- A saved document that crashed noshape on open (in v0.4.33–v0.4.35) opens again, and a Shell that never finished now ends.
- Documents are where the documents page says: the page shows the real folder (on Windows `C:\Users\…\Documents\noshape`) as a link, each document has "Show in Explorer / Finder" and its location in the Details panel, and a `.noshape` file alone always has every change (no more `-wal`/`-shm` files left beside it).
- Coding agents (Claude, Codex) start without failed calls: plain names and `~/…` paths work, and errors say what to do next.
- A sketch's spline handle tips show clearly when hovered or selected, as in Onshape.
- Fillets that folded into the part (a user's dock) now build valid bodies; fillets on boss rims, snap-hook lips and variable rounds build as in Onshape.
- Drawings: merging table cells no longer moves the columns.

**Closer to Onshape**
- Draft and Extrude with draft work on gear and spline profiles.
- Sheet metal: flanges beside short steps, corner breaks on active sheet metal, part patterns keep their bends.
- Frame: AS, 80/20 and construction-material profile libraries; a new Frame starts on 80/20 1001.
- Sketches on faces and planes take Onshape's direction; hovering a section cap highlights the whole cut, as in Onshape.
- Drawings: BOM tables, the Sheets panel, Foreshorten and suggested views as in Onshape.
- Many more community documents now rebuild exactly as in Onshape (snap hook, housing, duct, mortise and tenon, 3D spline, T-slot joints).

**Faster**
- A Part Studio starts rebuilding the moment it opens (about 0.1 s sooner on a Mac).
- A pattern of 51 cutting tools, drafts on gears and several Shells are much faster.
