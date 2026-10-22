# Workflow: exploded views and named positions (assembly)

qa5, 2026-10-22 pass 17, app /tmp/qa5/noshape-0503 (main aa75fdf). Doc /tmp/qa5/d28/x.noshape (copy of the pass 10/13 two-plate mate doc with Revolute 1).
Live: behaviours.md "Exploded views: steps, clicks, menus, rollback, Done" (~1833, assembly/800–826) and "Named positions panel in full" (~1340, assembly/150–161).
Status: ✅ matches live, ✗ differs, ? live pending. Shots /tmp/qa5/shots/x01–x14.

| # | Step | Live | Ours | Status |
|---|---|---|---|---|
| 1 | Right strip | BOM Table, Configuration panel, Exploded views, Named positions, (Simulation), Variable table | same minus Simulation (excluded) (x01) | ✅ |
| 2 | Exploded views panel, empty | 712 px, title, centred "Add exploded view" on a **light-grey full-width band**, illustration (triad over a cube), text + "Learn more about exploded views." (800) | same width/texts; button on **white**, a different illustration (two cubes + dashed arrow) (x02) | ✗ **EXV1** (ui, minor) |
| 3 | Add exploded view | "Editing Explode 1 [Done]" banner, toolbar → undo/redo, "⌄ Explode 1" blue + 4 px bar, indent guide lines, rollback bar, info box "Build steps by dragging components…" (801) | all present; **no indent guide line**, rollback bar lighter and not indented, info text wraps to 2 lines (cmp-x03) | ✅ / ✗ **EXV2** (ui, minor) |
| 4 | Select Part 1 <2> | triad: 3 **grey-outlined** arrows with cone heads + a rotation ring at each tip (802) | triad at the hole centre, **white**, plus **3 planar square handles** (x04) | ✗ **EXV3** (ui) |
| 5 | Drag the Z arrow | part follows; "Explode step" ✓✕: Translation ▾, Instances, ☐ Specify direction, Distance, ☑ Explode lines, "Edges, vertices, or faces"; single arrow in the graphics; row "↗ Explode step 1" selected (light blue); info box gone (805) | dialog identical (Distance 12.436 mm), dashed explode line, row "↗ Explode step 1", info box gone ✅; row **not highlighted**, the full triad stays (no single arrow) (x05) | ✅ / ✗ minor (EXV3) |
| 6 | Right-click Explode 1 / a step | Collapse Explode 1 · Rename · — · Show all explode lines · Hide all explode lines · — · Delete / Edit… · Rename · — · Delete (814, 815) | identical (x06, x07) | ✅ |
| 7 | Done | banner gone, toolbar back, unexploded; **tree stays expanded** (Explode 1 plain + its steps), nothing selected (823) | unexploded ✅, toolbar ✅; tree **collapsed** to "› Explode 1"; Part 1 <2> **stays selected with the triad** (x08) | ✗ **EXV4** (skb) |
| 8 | Double-click Explode 1 | re-enters editing, fully exploded (824) | same (x09) | ✅ |
| 9 | Named positions panel | Name column, italic "Name" add-row, info box "Capture variants of the assembly as named positions", footer "Add named position" (150) | same (x10) | ✅ |
| 10 | Add named position | row added at once + "Editing Named position N [Done]", toolbar → undo/redo (151) | same (x11) | ✅ |
| 11 | Mate row right-click in that mode | Hide, Hide other mates, Hide all mates, Isolate…, Make transparent… \| Add comment \| Add selection to folder… \| Expand ▸, Collapse ▸ (no driver item, **no Delete**) (152) | Show, Show all mates (mate hidden), Isolate…, Make transparent… \| Add selection to folder… \| Expand ▸, Collapse ▸ \| **Delete** (x12) | ✗ **NP1** (skb, minor: Delete in editing mode) |
| 12 | Row right-click | Apply, Edit, Duplicate, Update, (Move down/up with 2+ rows), Delete Named position N (159) | Apply, Edit, Duplicate, Update, Delete Named position 1 (x13) | ✅ |
| 13 | Apply | green toast "✓ Named position N is applied. ×"; applied row blue with a 2 px left bar (160) | same (x14) | ✅ |

Not covered yet: rotation step (ring drag), rollback-bar drag in the explode tree, driving-mate selector and value cells (needs an unhidden Revolute; next pass).
