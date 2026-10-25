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
| Points aligned with the origin | a 3rd point entity at (0,0) + Horizontal constraints | no entity (request 10-25 p1 (1)) | ✗ P1-4 (skb) |
| Hole on 2 picked points | "Vertices of Sketch 2" (one chip); 2 holes, −633.385 mm³; title "Ø 6.35 mm ↧ 12 mm" | chip per point? (request (2)) | ? |
| Linear pattern › Feature pattern | order Part / Feature / Face ✅; chip "Hole 1"; Direction pink until picked ✅ | chip text? (request (3)) | ✅ / ? |
| Shell 2.5 mm (top face) | 8960 mm³, 59 faces | — | ✅ |
| Edit Extrude 1 10 → 15 mm | everything regenerates Ok but Shell returns 4 slivers | — (request (5)) | ✗ **P1-5** (psb + kern) |
| Undo / Redo / Undo | 8960 ↔ slivers ↔ 8960 | — | ✅ |
| Rollback bar above Shell 1 | Shell greyed italic, part unshelled (14220.74) | same | ✅ |
| Suppress Fillet 1 (row menu) | italic strike-through, sharp corners, rest Ok (8446.23) | same | ✅ |
| Row menus | Extrude: Rename, Edit…, Edit Sketch 1…, Show dimensions, … ✅; Fillet: no "Show dimensions" | Fillet menu? (request (4)) | ? |
