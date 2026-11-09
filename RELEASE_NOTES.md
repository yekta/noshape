**Fixed**
- A spline's handle arms no longer disappear in sketches made before handle arms existed.
- Chamfers on several pins of one extrude measure from each pin's own sides, so they build as in Onshape.
- An External thread whose size doesn't match its cylinder says so ("Exact match for thread size not found."), as in Onshape.
- Creating a Variable Studio from a Part Studio's table can be undone in that Part Studio.
- Lofts with several-piece guides or spline profiles, and lofts made in older documents, now have Onshape's exact shape.
- Splits a feature makes on purpose (threads, Split, Wrap) stay split after later features.

**Closer to Onshape**
- ☰ › Print… in Part Studios and Assemblies: the Print setup panel with paper sizes, Download image and Print.
- Versions and history: Restore to Main… opens the restore dialog, a merge shows "Revert to before this merge" and its history rows, search runs on Enter, and Show more changes….
- Feature Studio: Onshape's snippets and autocomplete, code folding, adding a custom feature to the toolbar as in Onshape, Ctrl+Z / Ctrl+Y.
- Variable Studio: `#` autocomplete in values, configuration panel filters, the Insert Variable Studio dialog as in Onshape.
- Documents page and tabs: Ctrl+click or Shift+click a tab opens it in a new window, folder sort in New document.
- Clicking a checkbox in a dialog takes the keyboard, as in Onshape; Enter and Esc still work.
- Drawings: clicking an edge in a view selects the edge; Copy takes the view alone.
- Face blend and variable fillet messages as in Onshape.
- Every ? and Learn more opens the offline user guide at the right section.

**Faster**
- STL export matches Onshape's triangle counts at Coarse, Medium and Fine, and runs 5–6 times faster than Onshape's.
- Display meshing about a third faster across our test documents; o-rings and drafted parts much faster.
- Mirrors, feature patterns and Boolean cuts that miss the part are quicker.
