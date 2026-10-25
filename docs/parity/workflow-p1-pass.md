# P1 Part Studio workflow pass (qa5, 2026-10-25)

App: /root/green/noshape (main ee654502, last green). Live: behaviours.md / live screenshots cited per row. Document built in the
app: sketch rectangle on Top → Extrude → Fillet → Chamfer → sketch points on the top face → Hole → Linear (feature) pattern → Shell,
then edits, undo/redo, rollback and suppress.

| Step | Ours | Live | Status |
|---|---|---|---|
| Sketch on Top (Sketch → Top row) | dialog Top plane, ☑ Show constraints, view stays iso, header "Features (4)" | same, header "Features (5)" (sketch/501) | ✗ P1-1 (ui); header icons grey ✗ P1-2 (ui) |
| Corner rectangle | width/height values stay while the tool is active | same (sketch/06) | ✅ |
| Extrude right after ✓ | field empty and active, 25 mm, ✓ disabled | same (part-studio/1290) | ✅ |
| Enter in Depth | applies, dialog stays; 2nd Enter commits | same (behaviours 252) | ✅ |
| Fillet (Shift+F), 3 vertical edges, R2 | chips "Edge of Extrude 1" ×3, preview, 15270.061 mm³ | radius arrow on the **last** pick | ✗ P1-3 (psb): ours on the first pick |
| Chamfer (tool search), 1 mm, tangent propagation | 7-edge chain chamfered (faces 9 → 16) | — | ✅ |
| Sketch on a preselected face | "Face of Extrude 1" in Sketch plane | same | ✅ |
| Points aligned with the origin | a 3rd point entity at (0,0) + Horizontal constraints | no entity; a Horizontal constraint from each point to the **external** Origin | ✗ P1-4 (skb) |
| Hole on 2 picked points | "Vertices of Sketch 2" (one chip); 2 holes, −633.385 mm³; title "Ø 6.35 mm ↧ 12 mm" ✅ | **two chips "Vertex of Sketch 1"** (one per click; "Vertices of …" only when the sketch row is picked) | ✗ **P1-8** (ui) |
| Linear pattern › Feature pattern | order Part / Feature / Face ✅; chip **"Hole 1"**; Direction pink until picked ✅ | chip **"Ø 6.35 mm ↧ 12 mm"** (the feature's shown name) | ✗ **P1-9** (ui) |
| Shell 2.5 mm (top face) | 8960 mm³, 59 faces | — | ✅ |
| Edit Extrude 1 10 → 15 mm | everything regenerates Ok but Shell returns 4 slivers, status Ok | **Shell 1 ERROR "Could not shell part with selections."**, part stays unshelled (behaviours "qa5 (10-25 p1)" (5)) | ✗ **P1-5** (psb + kern) |
| Undo / Redo / Undo | 8960 ↔ slivers ↔ 8960 | — | ✅ |
| Rollback bar above Shell 1 | Shell greyed italic, part unshelled (14220.74) | same | ✅ |
| Suppress Fillet 1 (row menu) | italic strike-through, sharp corners, rest Ok (8446.23) | same | ✅ |
| Row menus | Extrude: Rename, Edit…, Edit Sketch 1…, Show dimensions, … ✅; Fillet: no "Show dimensions" | Fillet: no "Show dimensions" either (+ Add comment, out of scope) | ✅ |
| Mirror › Feature mirror of the Hole about the Right plane | dropdown Part / Feature / Face mirror; chips "Hole 1", "Right plane"; −158.35 mm³ (two holes through the 2.5 mm floor) | — | ✅ |
| Edit Fillet 1 R2 → R1 (dialog: later features greyed, "Final" ☐) | R1 applied; **Shell 1 Error "Could not shell part with selections."** at any thickness 1–2.5 mm | live? (request (6)) | ✗? **P1-6** (kern, needs live) |
| Undo the R1 edit | back to 8801.649 mm³ | — | ✅ |
| Rename (row menu) "Edge break" | stored "Edge break" | — | ✅ |
| Row menus of Fillet / Chamfer | no "Show dimensions" (Extrude has it) | same | ✅ |
| Delete Extrude 1 (dependents) | Fillet, Chamfer, Sketch 2, Hole, Pattern, Shell, Mirror error, Parts (0); errored rows red, **but hidden Sketch 2 stays grey** | failing sketch row red (behaviours 6432, 6491) | ✗ **P1-7** (ui) |
| Undo the delete | back to 8801.649 mm³ | — | ✅ |
| Extrude › Remove from a sketch under the part (default flips away, as live) | tool grey, no message | red translucent tool + toast "Selected tools and targets do not intersect." (part-studio/234) | ✗ **P1-10** (psb + icons2) |

## P2 assembly core (same session, /root/green/noshape ee654502; plate 60×20×5 with 3 Ø10 holes + Ø10×30 pin from Part Studio 1)
| Step | Ours | Live | Status |
|---|---|---|---|
| Insert Part 1 (first instance) | placed, view fits, free DOF glyph (not fixed) | same (assembly/80, free glyph) | ✅ |
| Insert Part 2, click to place, ✓ | Instances (2) | same | ✅ |
| Cylindrical mate (tool search), pin face then hole face | pin moves into the hole, HUD at the 2nd pick, two connector chips | same (MT10, MT13) | ✅ |
| HUD ✓ | commits Cylindrical 1 (hidden, light grey row) and opens "Cylindrical 2" | "Commit current mate and start a new mate" (behaviours 1329) | ✅ |
| Drag the pin | slides along the axis; nothing fixed, so a sideways drag moves both | — | ✅ (plausible) |
| Edit the mate, type list | Slider, Cylindrical, Revolute, Pin slot, Planar, Ball, Fastened, Parallel; retype renames the row ("Pin slot 1") | same order (behaviours 5941), MT12 | ✅ |
| Undo the retype | back to Cylindrical 1 | — | ✅ |

## P2 drawing (same session)
| Step | Ours | Live | Status |
|---|---|---|---|
| Create Drawing of Assembly 1… (suggested views) | 4 views, pin seen through hole 1, edges correct | — | ✅ |
| Part Studio tab menu | Delete, Rename, Properties…, Show code, Duplicate, Copy to clipboard, Select as document thumbnail, Move to document…, Export… | same (+ Open in new browser tab, browser only) | ✅ |
| Part row › Create Drawing of Part 1… | "Part 1 Drawing 1" tab right after Part Studio 1, 4 views | — | ✅ |
| Dimension on a straight edge (assembly drawing) | "2.362" placed | — | ✅ |
| Dimension on a hole circle | part drawing "Ø.394" ✅; **assembly drawing: click not taken** | — | ✗ **P2-D1** |
| BOM panel (right strip) | "Bill of materials", BOM type ▾ Flattened, Add column ▾, ⋯; Item / Quantity / Part number / Description; rows 1 | 1, 2 | 1 | same columns (behaviours 1270) | ✅ |
