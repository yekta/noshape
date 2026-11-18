# Error wording (live vs ours, from 2026-11-18)

Errors must be in Onshape's words: the exact text, its tooltip and its look (colour, icon). Lead 11-18 21:5x: live takes
Part Studio features; live2 takes assemblies, drawings and studios. Live: behaviours.md "error wording sweep"; evidence
live/errors-1118/. Status: **=** same, **≠** differs (owner in brackets).

## Assemblies (live2)

| Situation | Live | Ours | Status |
|---|---|---|---|
| Over-defined mate (a Fastened on top of a Revolute, pin-block) | both mates: "Fastened 1 has error: Mate overdefines the assembly. (Hidden)" / "Revolute 1 has error: …"; rows red, Mate features header red ⬇ circle, the root's icon red ! | the same words (green 548d795fb) | = text |
| …its tooltip | a bold grey-blue title bar with the message, right of the list, then the mate's parameters with their checkbox states (Revolute: only "Revolute") | a plain white popup under the row, no title bar or checkboxes; Revolute also lists Offset, Limits, Simulation connection | ≠ [asm] |
| …a hidden error mate's row text | faded red rgb(196,139,148) | strong red rgb(170,37,45) | ≠ [asm] |
| Mate that can't resolve its connectors | "Fastened 1 has error: Mate cannot resolve mate connectors. (Hidden)" (behaviours l.1946) | not checked yet | – |

## Part Studio features (live)
