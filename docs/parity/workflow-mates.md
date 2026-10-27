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
| 8 | Chip clock icon tooltip | "Mate connector of Part 1 <3> (Click to edit)" | **"Edit mate connector"** (m17) | ✅ (skasm 10-27: code assembly_mate_dialog.rs "<chip> (Click to edit)"); was ✗ MT3 (skb) |
| 9 | Click the clock icon | "Mate connector" panel (✕ only): On entity ▾, Origin entity "Face of Part 1 <3>", ☐ Realign, ☐ Move, flip / reorient; the edited chip turns **bold** and the owning **instance row is highlighted** (578) | same panel; chip row only light blue (not bold); instance row not highlighted (cmp-m18) | ✗ MT4 (skb, minor) |
| 10 | Fastened ☑ Offset | X↘ Y↗ Z↑ "0 mm" (regular black), **Rotate about X**, Rotation angle 0 deg (585) | values in **grey italic** (placeholder look); select defaults to **Rotate about Z** (cmp-m19) | ✅ (skasm 10-27: values regular text (GUI green 8165fca6), Fastened defaults to Rotate about X (code)); was ✗ **MT5** (values, ui) / **MT6** (default axis, skb) |
| 11 | Change type to Revolute | title follows ("Revolute 1") | same (m20) | ✅ |
| 12 | Revolute ☑ Limits | Z min "No minimum", Z max "No maximum" in regular text (51) | same fields, grey italic (cmp-m21) | ✅ (GUI green 19555ca3, skasm 10-27: "No minimum" / "No maximum" regular text); was ✗ MT5 |
| 13 | Select the mate row | only the **mated faces amber**, both **instance rows related-highlighted**, connector triad **with X/Y/Z letters**, blue dashed circle with arrowheads + angle label along the arc ("275.326°") hugging the connector (305) | **both whole parts tinted tan**, instance rows not highlighted, triad without letters; circle + arrowheads + "335.387°" label drawn, but the circle is far larger (radius ≈ 135 px vs ≈ 60 px); at 0° only a "0.000°" label away from the connector, no circle (m23, m26) | ✅ (GUI green 8165fca6, skasm 10-27: only the mated face amber, both instance rows related-highlighted, triad with X/Y/Z letters, blue dashed circle + angle label); was ✗ **MT7** (skb) |
| 14 | Esc / empty click with the mate selected | Esc keeps the values; an empty click clears them | same (m24, m25) | ✅ |
| 15 | Drag <2> | rotates about the revolute axis only | same (m26) | ✅ |
| 16 | Mate row right-click | Rename, Edit…, Reset, Hide/Hide other/Hide all mates, Isolate…, Make transparent…, Edit named position driver mate…, Suppress, Dynamic suppression ▸, Animate…, (Add comment), Add selection to folder…, Expand ▸, Collapse ▸, Delete | same, with Show / Show all mates for our hidden mate (m27) | ✅ |
| 17 | Animate… | "Animate mate": ⟳ - z - Revolute 1, Start 0 deg, End 360 deg, Steps 300, Playback Reciprocate ▾ + ↻, Current value, ▶ ■; instance rows related-highlighted | same dialog; ▶ animates with Current value updating, ■ stops (m28–m33); rows not highlighted (MT7) | ✅ |
| 18 | Ctrl+M Mate connector, pick, ☑ Realign, ☑ Move | On entity ▾, Origin entity, Realign → Primary / Secondary axis entity; Move → **X↘ Y↗ Z↑ icon rows** "0 mm", Rotate about Z, Rotation angle 0 deg; **no preview slider**; rows ~29 px (qa5/asm-043) | same fields, but Move rows are labelled **"X translation" / "Y translation" / "Z translation"** (grey italic values) and the dialog has a **preview slider** row (cmp-m37) | ✗ **MT8** (skb) |
| 19 | Hidden mate row tooltip | "<name> (Hidden)" + parameter summary | "Revolute 1 (Hidden) / Revolute mate / Offset: off" (m38) | ✅ (summary lines ?) |
| 20 | Replace instances… › pick an unrelated part | panel closes, plain chip "Part 1", title black, orange preview at the instance's position, mates that can't resolve turn red + red badge on the root | same (m44–m46) | ✅ |
| 21 | ✓ | mate kept but **fails**: red row, tooltip "<mate> has error: Mate cannot resolve mate connectors. (Hidden)" | the mate turns back to normal (not red) and **re-attaches to the new part, moving it onto <1>** (the box now sits on the plate) (m47, m48) | ✗ **MT9** (skb) |

## Re-run pass 13 (2026-10-22 ~05:20–06:40, main aa75fdf: app /tmp/qa5/noshape-0503 = /root/target-lead/debug/noshape 05:03, CLI /tmp/qa5/noshape-cli-0226 = target-engine 02:26)
Docs /tmp/qa5/d24b/m.noshape (mate-base copy), /tmp/qa5/d24b/full.noshape (pass 10 doc). Shots m50–m76.
| Row | Ours | Status |
|---|---|---|
| MT1 flip icon (empty dialog) | up-right, same as live 30 (cmp-flip2) | ✅ **fixed** |
| MT2 mate-type list | plain rows, current type highlighted, overlays the field (cmp-m52) | ✅ **fixed** |
| MT3 chip clock tooltip | "Mate connector of Part 1 <2> (Click to edit)" | ✅ **fixed** (m56) |
| MT4 connector panel | chip bold, instance row highlighted | ✅ **fixed** (m57) |
| MT5 / MT6 Offset values / default axis | regular black "0 mm"; Rotate about X | ✅ **fixed** (m53) |
| MT7 selected Revolute at 0° | mated face amber, instance rows highlighted, triad letters, no circle ✅; label **"0.000°" italic, past the X axis** (live "0°" upright on the X-axis end, trailing zeros dropped) | ✗ still (label only) (m60) |
| MT8 Mate connector tool | no preview slider; Move rows X↘ Y↗ Z↑ icons, Rotate about Z | ✅ **fixed** (m61, m62) |
| MT9 Replace instance → unresolvable mate | stays red, tooltip "Revolute 1 has error: Mate cannot resolve mate connectors. (Hidden)" | ✅ **fixed** (m75, m76) |

## Re-run pass 19 (2026-10-22 ~11:00–11:40, app /tmp/qa5/noshape-1013 = /root/target-lead/debug/noshape 10:13: the queue's 09:58 batch build = main 3ce6952 + the ready branches of that batch; CLI 08:11)
| MT7 label at 0° | small blue "0°" on the X-axis end over the red X (z11) | ✅ **fixed** |

## Re-check on main 88fbf8f5 (qa4, 2026-10-23 18:00–19:00, FRESH=1, /root/qa4-scratch/bin/noshape-m88)
- MT3 tooltip "… (Click to edit)" ✅ in code; MT5 offset values now regular text (QuantityInput, qa4 10-22) ✅; MT5 limit fields,
  MT6 default axis, MT7 label, MT9: fixes cited in code, not re-run (the demo's mates are drawn grey here: checked separately).

## skb P1 mates audit (2026-10-24, agent/skb on main fde24dac; /root/skb-scratch/mates, plate with Ø10 hole + Ø10×30 pin)
| Row | Live | Ours | Status |
|---|---|---|---|
| MT10 HUD position | centred on the 2nd pick, bottom edge on it, kept inside the graphics area (assembly/35, 93; mate-hud.txt) | was +14/+14 below-right, unclamped (✓/✕ cut off at the right edge, mates/s06) | ✅ **fixed** |
| MT11 DOF and drag per type | Fastened 0, Revolute ⟳z, Slider ↹z, Cylindrical ⟳z ↹z, Planar ⟳z ↹x ↹y, Ball 3 rotations (behaviours DOF table) | same (solver test + drags in the app, mates/t-*.png) | ✅ |
| MT12 retype via Edit… | the row is renamed, the solve follows the new type | same (Revolute → Slider → Cylindrical → Planar → Ball) | ✅ |
| MT13 cylinder-face inference | connector on the axis at mid-length, dots at the end centres (assembly/31) | same (mates/s10) | ✅ |
| MT14 MATE-FLIP-AXIAL (qa4 round 5; live "qa4 round 5 s3 mate flip") | Revolute / Cylindrical / Planar: default aligns the connectors' Z (pin hangs through, t (0,0,−30)); Flip primary axis turns it over (t (0,0,50), Z (0,0,−1)); a pin on its side gets the default too | flip did nothing (the residual took Z's x, y only); now a mate whose Z points the wrong way turns its moving part over before solving (`solver::orient_axial_mates`), the tie (perpendicular axes) aligns (`ops::assembly::flip_for_axes`), and the CLI's new mates bring the connectors together like the dialog. Tests: `flip_primary_turns_axial_mates_over`, ops `mate_flip`, `default_flip_ties_align`; app: flip in the floating toolbar stands the pin on the plate (mates/fl02, fl03) | ✅ **fixed** |
| MT15 axial default with the part starting anti-aligned | (live "skb (10-25) axial mate default flip") Revolute / Planar keep an upside-down pin's pose (flip set automatically, t (0,0,50)); parallel or perpendicular starts align, as Fastened | same: least rotation, ties aligned (`flip_for_axes`); ops `mate_flip` covers upside down (Revolute, Planar) and on its side (Revolute, Fastened) | ✅ |
