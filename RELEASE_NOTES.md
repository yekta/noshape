**Fixed**
- Hovering in a sketch now lights what is under the pointer when it stops, never something it passed over on the way: no more lines staying lit, a plane lighting instead of a line, or nothing lighting when you come back to a line.
- Snapping while drawing a sketch line works as in Onshape: model edges and their midpoints offer a coincident, points of other sketches snap without being hovered first, and resting on a point offers its horizontal and vertical alignment.
- Double-clicking a face shows its sketch, clicking empty space clears the related highlight in the feature list, and a hidden sketch lights when you hover its row.
- Measuring the angle between two faces gives the same angle as Onshape (for example 33.69°, not 146.31°).
- Pressing a mate's icon in an assembly now picks the mate, never the part behind it.
- Drawings with mesh parts no longer show error boxes for their views; mesh parts are drawn by their outlines.
- A face pattern regression from the last release that broke sketches on patterned pockets is undone.

**Closer to Onshape**
- Picking, snapping and drag thresholds now use Onshape's own pick sizes and tie rules; hover outlines and selected parts are drawn with Onshape's colours and widths.
- Sketch: value boxes while drawing, arc and ellipse drags, Fix on a line, the unsolved-sketch message and badge, Extend and Offset steps, one constraint icon selected at a time.
- Feature list: renaming commits when you click in the graphics, the rollback bar lands where Onshape puts it, the "working" spinner appears after 900 ms.
- Right-click menus: parts have "Where used…", and a face's sketch and feature items only show when nothing else is selected.
- Section view: Shift+X brings back the last section; hidden parts' row menu has Show and Show all parts.
- Configurations: a plain click on a row no longer switches configuration; duplicate row names are stored as Onshape does.
- Versions panel: clicking a change under a version opens it.
- Variable Studio: invalid names are marked red as you type.
- Drawings: hover and selection look, dragging views, centre mark grips, weld symbols and the hole callout palette as in Onshape; DXF export writes shared edges as Onshape does.
- Export shows a "being prepared" message and puts "Export completed" in the notifications bell; Import keeps you on your current tab.
- Parasolid and STEP import keep cones, cylinders and spheres as such, read Onshape's mesh parts, and split a solid of separate pieces into one part per piece.

**Known issue**
- Switching back to an already-open tab in a large document can take about 0.7 s (since v0.4.54). The fix is in and comes in the next release.
