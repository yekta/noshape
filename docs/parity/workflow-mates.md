# Workflow: mates and mate connectors in depth (dialog, inference, Fastened flip rule, HUD, connector panel, Offset/Limits, mate values, Animate, Mate connector tool, Replace instance)

qa5, 2026-10-22 pass 10, app /tmp/qa5/noshape-0004 (target-shared 00:04, main still 5bd50b2 + uncommitted work).
Doc /tmp/qa5/d24/mate.noshape (setup /root/qa5-scratch/pass10/mkmate.py = live's "skb mate offset": Part Studio "hole plate" 60×40×5 mm with a centred Ø10 through hole;
Assembly 1 = Part 1 <1> fixed at the origin, Part 1 <2> upright at (80, 30, 20) mm). Transforms read with the CLI (assembly_list) on a saved copy.
Shots /tmp/qa5/shots/m*.png (cmp-* = ours | live side by side).
Live: behaviours.md "Mates" (~1320), "Drag cues, mate dialog details…" (~1209), "Fastened mate: default primary-axis alignment (flip)" (~1825), qa5 asm-040…044 (Mate connector), "Replace instance" (~1931), assembly/30–53, 300–318, 573–589.
Status: ✅ matches live, ✗ differs, ? live pending.

| # | Step | Live | Ours | Status |
|---|---|---|---|---|
| 1 | **m** | "Fastened 1" (red), type select, Mate connectors (active), ☐ Offset, ☐ Simulation connection, flip / reorient / ▶ / Solve, ? (assembly/30) | same, minus ☐ Simulation connection (simulation is excluded by the team rules); the **flip-primary-axis icon in the empty dialog points down-left**, live's up-right (it reads up-right only after picks; cmp-flip) | ✗ MT1 (ui, minor) |
| 2 | Mate type select | list replaces the field: plain text rows, **current type highlighted blue**, scrollbar, ~7.5 rows; order Slider, Cylindrical, Revolute, Pin slot, Planar, Ball, Fastened, Parallel (36) | same order, but a popup below the field **with an icon per row** and **no highlight on the current type** (cmp-m03) | ✗ MT2 (ui) |
| 3 | Hover a planar face | orange face outline, white dots at corners / edge midpoints / hole centre, small triad + ring at the nearest point (31) | same (m05) | ✅ |
| 4 | Flip rule (c): top face <2> → top face <1> | coincident, overlapping, no flip; <2> identity at (0,0,0) | identity at (0,0,0) (m06, CLI) | ✅ |
| 5 | Flip rule (a): bottom face <2> → top face <1> | flipped automatically: <2> stacked upright, translation (0,0,5), identity | (0,0,5), identity (m14–m16, CLI) | ✅ |
| 6 | HUD after the 2nd pick | 310×51 panel: drag dots, [type ▾], flip, reorient, Solve, green ✓, ✕ | same (m06) | ✅ |
| 7 | HUD ✓ | commits and opens "Fastened 2" with an empty active field; tool stays active | same; Fastened 1 row grey (hidden) with an expander (m07) | ✅ |
| 8 | Chip clock icon tooltip | "Mate connector of Part 1 <3> (Click to edit)" | **"Edit mate connector"** (m17) | ✗ MT3 (skb) |
| 9 | Click the clock icon | "Mate connector" panel (✕ only): On entity ▾, Origin entity "Face of Part 1 <3>", ☐ Realign, ☐ Move, flip / reorient; the edited chip turns **bold** and the owning **instance row is highlighted** (578) | same panel; chip row only light blue (not bold); instance row not highlighted (cmp-m18) | ✗ MT4 (skb, minor) |
| 10 | Fastened ☑ Offset | X↘ Y↗ Z↑ "0 mm" (regular black), **Rotate about X**, Rotation angle 0 deg (585) | values in **grey italic** (placeholder look); select defaults to **Rotate about Z** (cmp-m19) | ✗ **MT5** (values, ui) / **MT6** (default axis, skb) |
| 11 | Change type to Revolute | title follows ("Revolute 1") | same (m20) | ✅ |
| 12 | Revolute ☑ Limits | Z min "No minimum", Z max "No maximum" in regular text (51) | same fields, grey italic (cmp-m21) | ✗ MT5 |
| 13 | Select the mate row | only the **mated faces amber**, both **instance rows related-highlighted**, connector triad **with X/Y/Z letters**, blue dashed circle with arrowheads + angle label along the arc ("275.326°") hugging the connector (305) | **both whole parts tinted tan**, instance rows not highlighted, triad without letters; circle + arrowheads + "335.387°" label drawn, but the circle is far larger (radius ≈ 135 px vs ≈ 60 px); at 0° only a "0.000°" label away from the connector, no circle (m23, m26) | ✗ **MT7** (skb) |
| 14 | Esc / empty click with the mate selected | Esc keeps the values; an empty click clears them | same (m24, m25) | ✅ |
| 15 | Drag <2> | rotates about the revolute axis only | same (m26) | ✅ |
| 16 | Mate row right-click | Rename, Edit…, Reset, Hide/Hide other/Hide all mates, Isolate…, Make transparent…, Edit named position driver mate…, Suppress, Dynamic suppression ▸, Animate…, (Add comment), Add selection to folder…, Expand ▸, Collapse ▸, Delete | same, with Show / Show all mates for our hidden mate (m27) | ✅ |
| 17 | Animate… | "Animate mate": ⟳ - z - Revolute 1, Start 0 deg, End 360 deg, Steps 300, Playback Reciprocate ▾ + ↻, Current value, ▶ ■; instance rows related-highlighted | same dialog; ▶ animates with Current value updating, ■ stops (m28–m33); rows not highlighted (MT7) | ✅ |
| 18 | Ctrl+M Mate connector, pick, ☑ Realign, ☑ Move | On entity ▾, Origin entity, Realign → Primary / Secondary axis entity; Move → **X↘ Y↗ Z↑ icon rows** "0 mm", Rotate about Z, Rotation angle 0 deg; **no preview slider**; rows ~29 px (qa5/asm-043) | same fields, but Move rows are labelled **"X translation" / "Y translation" / "Z translation"** (grey italic values) and the dialog has a **preview slider** row (cmp-m37) | ✗ **MT8** (skb) |
| 19 | Hidden mate row tooltip | "<name> (Hidden)" + parameter summary | "Revolute 1 (Hidden) / Revolute mate / Offset: off" (m38) | ✅ (summary lines ?) |
| 20 | Replace instances… › pick an unrelated part | panel closes, plain chip "Part 1", title black, orange preview at the instance's position, mates that can't resolve turn red + red badge on the root | same (m44–m46) | ✅ |
| 21 | ✓ | mate kept but **fails**: red row, tooltip "<mate> has error: Mate cannot resolve mate connectors. (Hidden)" | the mate turns back to normal (not red) and **re-attaches to the new part, moving it onto <1>** (the box now sits on the plate) (m47, m48) | ✗ **MT9** (skb) |
