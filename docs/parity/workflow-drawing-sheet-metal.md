# Workflow: drawing of a sheet-metal part with a flat view

qa5, 2026-10-21 pass 4, /tmp/qa5/noshape-1448, "qa5 sheet metal" (Sheet metal model, Flange, Hem, Bend relief, Corner, Finish).
Live: behaviours.md "Flat pattern view" (asb, ~line 1365). Status: ✅ matches live, ✗ differs, ? live pending.

| # | Step | Live | Ours | Status |
|---|---|---|---|---|
| 1 | Part 1 right-click | Rename, Properties…, Assign material…, Edit appearance… \| Copy here…, Copy Part 1 \| Create Drawing of Part 1…, Export…, Release… \| Hide, Isolate…, Make transparent… \| Add comment \| Zoom to selection \| Delete… | same + **"Export flat pattern (DXF)…"** (the SVG item is gone) | ✗ SM13 (partly fixed) |
| 2 | Create Drawing of Part 1… › ANSI_A_MM, No views | "Part 1 Drawing 1" | same, inserted after Part Studio 1 | ✅ |
| 3 | Insert › picker filter "Flat patterns" | rows "Flat pattern of Part 1" with a **flat thumbnail** | "Flat pattern of Part 1", but the thumbnail shows the **folded** part | ✗ **DSM1** |
| 4 | Place the flat view (1:1, Top) | outline; each bend = two solid bend-region lines + dashed centre line; no view label | same style; no label | ✅ |
| 5 | Bend notes | vertical along each bend, "**UP 90.0° R.091**" (one decimal on the angle, document-unit radius) | "**DOWN 90° R2**" ×2, "DOWN 180° R0.01" (hem); horizontal-bend notes read horizontally above the bend | ✗ SM5 (Down) + **DSM2** (angle "90°" vs "90.0°"; placement ?) |
| 6 | View right-click › Show/hide ▸ | Show hidden lines · Hide bend lines · Hide bend notes · Show shaded view · Hide threads · **Show faulty parts** · Show/hide sketches… · Show/hide edges… · Hide auto centermarks | no "Show faulty parts"; extra "Show center of mass" | ✗ **DSM3** |
| 7 | Hide bend notes | notes hidden | notes hidden; Ctrl+Z restores | ✅ |
| 8 | View right-click (whole menu) | (assembly/part view menus in behaviours.md drawing section) | Show/hide ▸, View orientation ▸, Tangent edges ▸, Adjust linestyle…, Insert custom table of Part Studio 1…, View properties…, Order ▸, Align views ▸, Rotate view ▸, Switch to Part Studio 1, Move to sheet…, Copy, Paste as table, Select for replication [beta] ▸, Replicate annotations [beta], Clear selection, Zoom to fit, Delete | ? |
