**Fixed**
- Feature pattern: a patterned cut whose copy lands partly in empty space now builds.
- Delete face no longer reports success while the face stays in the part; it says "Could not delete selected faces." as Onshape does.
- Extrude Up to next through a row of parts stops at the right faces; a New extrude of touching regions from different sketches keeps them as separate parts.
- Loft: a section made of one closed curve with one connection point lofts as in Onshape.
- Sketch: clicking the Origin adds it to the selection, so a corner and the Origin can be made coincident.
- Drawings: Create Drawing with no views picks a scale that fits the first view, and follow-on views place only along the parent view's axes.
- Drawings: a view whose part no longer exists goes blank and the sheet shows "Failed to resolve part.".
- Drawings: a straight tangent or hidden edge next to a visible line is drawn instead of dropped; DXF no longer writes overlapping copies of one line.
- Assemblies: standard nuts and bolts keep an empty Description in the BOM, as in Onshape.
- Importing a file that can't be translated says "<file> uploaded but failed to translate." in a red message that stays until closed.

**Faster**
- Opening a big assembly: its Part Studios rebuild side by side; a test assembly's first tab opened in 41 s instead of 70 s on the same machine.
- Big assemblies draw small and distant parts at coarser detail, and display meshes use Onshape's density.
- Spur gears and other free-form Boolean cuts are faster again (v0.4.59 had slowed them down).
- Loft along a path, Rib, and Move face offsets of planes run faster.
- Inserting the first drawing view of a large Part Studio through the agent API: 9.7 s to 0.15 s.

**Closer to Onshape**
- Sketch: a shown sketch's lines and points light at Onshape's distances outside sketch mode, while feature dialogs still pick regions inside small circles; Trim and Symmetric act as in Onshape on corners and line pairs.
- Feature dialogs: Esc after Tab has left a dialog's fields keeps the dialog open.
- Selection: a box being dragged lights nothing until released; Y / Shift+Y hide and show at once; a selection chip's × says "Delete <name>".
- Mass properties opens where Onshape puts it; Isolate leaves Default geometry as it is; named views animate as in Onshape.
- Assemblies: mate rows open on double-click, mate error flyouts and faded error rows as in Onshape; A flips a mate about the right axis; Shift+R, W and N keys as in Onshape.
- Assemblies: Insert › Other documents has Folders and Documents sections with Sort menus; Standard content opens on your last category, size and thread length.
- Assemblies: flexible subassemblies inside flexible subassemblies move with the top-level solve.
- Drawings: dimensions, geometric tolerances, datums, crop, auxiliary, section and detail views place and preview as in Onshape; keyboard shortcuts and the Measure panel's units match Onshape's.
- Configurations: the box is one input tall until dragged; an out-of-range value stays in the field, marked pink.
- Variable Studio: Enter on a failing value keeps the cell selected; a lost reference's grip is red-orange.
- Versions panel: tooltips sit beside the panel.
- Error messages for Up to next with nothing ahead, a revolve across its axis and an impossible loft use Onshape's words.
