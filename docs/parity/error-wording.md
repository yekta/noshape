# Error wording (live vs ours, from 2026-11-18)

Errors must be in Onshape's words: the exact text, its tooltip and its look (colour, icon). Lead 11-18 21:5x: live takes
Part Studio features; live2 takes assemblies, drawings and studios. Live: behaviours.md "error wording sweep"; evidence
live/errors-1118/. Status: **=** same, **≠** differs (owner in brackets).

## Assemblies (live2)

| Situation | Live | Ours | Status |
|---|---|---|---|
| Over-defined mate (a Fastened on top of a Revolute, pin-block) | both mates: "Fastened 1 has error: Mate overdefines the assembly. (Hidden)" / "Revolute 1 has error: …"; rows red, Mate features header red ⬇ circle, the root's icon red ! | the same words (green 548d795fb) | = text |
| …its tooltip | a bold grey-blue title bar with the message, right of the list, then the mate's parameters with checkbox marks | a plain white popup under the row, no title bar or checkboxes; (the parameter list itself matches live 10-22: Revolute / Offset / Limits / Simulation connection) | ≠ [asm] |
| …a hidden error mate's row text | faded red rgb(196,139,148) | strong red rgb(170,37,45) | ≠ [asm] |
| Mate that can't resolve its connectors | "Fastened 1 has error: Mate cannot resolve mate connectors. (Hidden)" (behaviours l.1946) | not checked yet | – |
| Part deleted in its Part Studio (instance can't resolve) | instance row red #aa252d with a red ! badge on its icon; tooltip "Part 2 <1> has error: Failed to resolve instance."; gone from the graphics (behaviours E15, l.8741) | the same words, red row, gone from the graphics (green 548d795fb, pin-block: Extrude 2 deleted); ≠ no ! badge on the part icon, a red dot after the DOF icon instead (errors-1118/oe15.png) | = text, ≠ badge [asm] |
| …a mate on the lost instance | "Cylindrical 1 has error: Failed to resolve the instance for mate. (Hidden)" then its parameters (E15) | "Revolute 1 has error: Failed to resolve the instance for mate. (Hidden)", Revolute / Offset / Limits / Simulation connection | = |

## Part Studio features (live)

Live 2026-11-18 21:48–22:01 UTC: a 40 × 30 × 10 block, then each failing feature added by REST to a scratch Part Studio, and each row read in the
UI (hover tooltip, dialog). Ours: the same steps through the CLI (`feature_add`, `sketch_constrain`) on green e355f5286, and the list in the app.
Evidence live/errors-1118/ (live-errors.json, rows.json, list.png, tip-*.png, dlg-*.png; ours-cli.json, ours-*.png). The tooltip text is
"<name> did not regenerate properly: <message>"; a name that doesn't start with its type adds it in brackets
("Pattern no direction [Linear pattern] did not …").

| Situation | Live | Ours | Status |
|---|---|---|---|
| Extrude Up to next, nothing ahead (a circle 100 mm off the part, New) | "Up to next termination problem, consider switching it to up to part." | "Failed to extrude selections, check input." | ≠ [core] |
| Extrude with no region | "Select face or sketch region to extrude." | the same | = |
| Fillet with no edges | "Select edges or faces to fillet." | the same | = |
| Fillet too big (every edge of the block, R6 on 10 mm) | "Failed to fillet selections." | the same | = |
| Fillet R35 on one top edge (Allow edge overflow on) | no error | no error | = |
| Chamfer too big (one top edge, 45 mm) | "Failed to chamfer selections."; the edge's entry in the dialog red | the same, the entry red | = |
| Chamfer 20 mm on that edge | no error (volume 6000 mm³) | no error (6000 mm³) | = |
| Shell too thick (top face, 20 mm on 10 mm) | "Could not shell part with selections." | the same | = |
| Linear pattern of the part, overlapping (10 mm × 3, New) | no error, 3 overlapping parts | the same | = |
| Linear pattern with no direction | "Select a pattern direction." | the same | = |
| Sketch over-constrained (a line Horizontal + Vertical) | "Sketch could not be solved."; row red (REST featureStatus WARNING) | the same text, row red (status Error) | = |
| Hole on a point off every part | "None of the holes intersected a part." | the same | = |
| Revolve with no axis | "Select the axis to revolve around." | the same | = |
| Revolve whose axis crosses the profile | "Revolve would create self-intersecting part." | "Geometry operation failed: operation failed: revolve failed (does the profile cross the axis?)" | ≠ [core] |
| Loft with one profile | "Select at least two profiles, a closed guide curve or a closed path." | the same | = |
| Loft of two coplanar profiles (circle and square on Top) | Error "Could not determine loft direction." | builds a part, no error | ≠ [core] |
| Mirror with no plane | "Select a mirror plane." | the same | = |
| Boolean Subtract with no tools | "Cannot resolve entities." | the same | = |
| Sweep with no path | "Select entities for sweep path." | the same | = |
| Draft with no faces | "Select faces to draft." | the same | = |
| A failed feature's row | name red rgb(170, 37, 45), its type icon unchanged (no badge); the Features header shows a red ⬇ circle | the same | = |
| …its tooltip | right of the list, a box at most **400 px** wide that **wraps** ("Chamfer too big did not regenerate properly: Failed to chamfer / selections." on two lines, 400 × 44); 12 px / 18 px weight 500, text rgb(51, 51, 51) on rgb(208, 219, 229), padding 4 px | the same text and colours, on **one line** however long (≈ 400 px wider) | ≠ [psui] |
| …its dialog | the title red; a selection that no longer resolves red in its box; no message text in the dialog | the same | = |
| Hidden sketches' rows at rest | no icon at the row's right end (an eye only on hover) | a crossed-out eye on every hidden sketch's row at rest | ≠ [psui] |
