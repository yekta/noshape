# Workflow: sketch tools in depth (Trim, Offset, sketch fillet/chamfer, Slot, linear pattern, Mirror, Spline, Polygon, S toolbar)

qa5, 2026-10-22 pass 14, app /tmp/qa5/noshape-0503 (main aa75fdf). Doc /tmp/qa5/d26/sk.noshape (new mm document via the CLI, Part Studio "qa5 sketch"),
Sketch 1 on Top, N for the normal view (scale ≈ 5.5 px/mm). Shots /tmp/qa5/shots/k10–k36.
Live: behaviours.md "Sketch mode" (~116–215: drawing tools, Esc per tool, mm defaults sketch/300–312), screenshots sketch/87–139, 300–309.
Status: ✅ matches live, ✗ differs, ? live pending.

| # | Step | Live | Ours | Status |
|---|---|---|---|---|
| 1 | Sketch on Top + N | normal view, sketch outline + name label | same (k10) | ✅ |
| 2 | Corner rectangle (G) + Line (L) across it | glyphs —/\| , Esc ends | same (k11) | ✅ |
| 3 | Trim (M), click the line's stub outside the rectangle | removed back to the nearest intersection | removed back to the rectangle edge, coincident glyph added (k12, k13); hover highlights the whole line | ✅ (hover ?) |
| 4 | Offset (O), click the rectangle's top edge | picked line amber, light-blue copy at **5 mm**, a large **hollow grey arrow** manipulator; **no value box** (sketch/303) | light-blue copy at 5 mm ✅, but a **small orange arrow** and a **"5 mm" value box opens at once** (k14) | ✗ **OFF1** (skb) |
| 5 | Esc in Offset | cancels | cancels (k15) | ✅ |
| 6 | Sketch fillet (Shift+F), click a corner | preview **R20 mm** arc + "R" leader + **hollow grey arrow** manipulator (sketch/301) | R20 preview with a small **square handle**, no arrow, no R leader (k16) | ✗ **FIL1** (skb, manipulator) |
| 7 | Enter → value box | value box with ±, ✓, ✕ | value box opens with "20 mm" ✅, but the **corner's constraint glyphs draw on top of the box**, covering ± and half the ✓ (k17, k17z) | ✗ **VB1** (ui) |
| 8 | Type 3, Enter | fillet shrinks to R3; the two lines extend back toward the corner, rectangle otherwise unchanged (minimal-change solve) | R3 applied, but the **rectangle shrinks**: 160×120 px → ~123×73 px (left edge moved right ~37 px, bottom edge up ~50 px) (k18, k19) | ✗ **SK5** (skb, solver; live check requested) |
| 9 | Slot, click a line | light-blue slot preview **Ø20 mm** + **"Ø20" dimension with a vertical leader** + 4 tangent points (sketch/304) | Ø20 preview (≈110 px) ✅, no dimension and no tangent points while pending (k20) | ✗ **SLOT1** (skb, minor) |
| 10 | Click elsewhere | commits: dark blue, region filled, centre line stays solid; Ø20 above with a vertical leader; glyphs: coincident ×4 + tangent + offset glyphs (sketch/307) | commits ✅ (solid centre line, filled), but Ø20 on a **diagonal leader to the right arc**; glyphs tangent + **equal "="** pairs, no coincident/offset glyphs (k21) | ✗ SLOT1 |
| 11 | Linear pattern on a circle | 3x along X at **25 mm**, 1x along Y; **hollow grey arrow manipulators**, square handles, "3x"/"1x" with **leaders**, spacing **dimension with extension lines** (sketch/136) | 3x / 25 mm / 1x ✅; but **dashed yellow guide lines + grey dots**, labels without leaders, bare "25" (k22) | ✗ **PAT1** (skb, preview look) |
| 12 | Enter | pattern committed | 3 circles, 25 dimension, pattern glyph (k23) | ✅ |
| 13 | Mirror: pick line, then a circle | line yellow-green #f2ff80; copy at once with symmetric glyph | same (k24, k25) | ✅ |
| 14 | Spline: 3 clicks, Esc | commits through the clicked points (cursor point dropped), tool stays | same, end-tangent handles hollow (k26) | ✅ |
| 15 | Inscribed polygon: centre, vertex | dash-dot construction circle, "Ø…" label, spinner **"◀6▶" rotated along the edge** next to the vertex (sketch/91) | hexagon + Ø14.551 ✅; construction circle **solid**, spinner **horizontal above the polygon** (k28) | ✗ **POLY1** (skb, minor) |
| 16 | Esc | discards the uncommitted polygon | discards (k29) | ✅ |
| 17 | S key at (1200,400) | 175×62 panel at **cursor x+10, y−30**, square corners; Line, Corner rect, Center rect, Circle, 3-pt arc / Spline, Point, Use, Trim, Dimension | same 10 tools ✅; panel 165×68 **above-left of the cursor** (x 1151–1315, y 327–395) with **rounded corners** (k30, k31) | ✗ **STB1** (ui) |
| 18 | Sketch chamfer, click a corner | preview chamfer **5 mm** + horizontal arrow manipulator + two 5 mm dimensions (sketch/302, 116) | **no preview** after the click (k33, k34); Enter commits a 5 mm chamfer and opens the value box "5 mm" with ± ✅ (k35) | ✗ **CH1** (skb) |

Findings: SK5 (solver reshapes geometry on a fillet radius edit), CH1 (no chamfer preview), VB1 (glyphs over the value box), OFF1, FIL1, PAT1, SLOT1, POLY1, STB1.
Live request "qa5 (10-22 pass 14, workflow 26 …)" filed for row 3 hover, row 8 and the committed slot glyphs.

## Re-run pass 19 (2026-10-22 ~11:00–11:40, app /tmp/qa5/noshape-1013 = /root/target-lead/debug/noshape 10:13: the queue's 09:58 batch build = main 3ce6952 + the ready branches of that batch; CLI 08:11)
Doc /tmp/qa5/d29/sk.noshape (same steps as pass 14). Shots z20–z29.
| Row | Ours | Status |
|---|---|---|
| OFF1 Offset after a click | hollow grey arrow, no value box (z20) | ✅ **fixed** |
| FIL1 fillet manipulator | hollow grey arrow + "R" leader (z21) | ✅ **fixed** |
| (new) **FIL2** default sketch-fillet radius | **R5** (z21, z22); live mm default **R20 mm** (sketch/301; pass 14 had R20) | ✗ **FIL2** (skb, regression) |
| VB1 glyphs over the value box | box clear (z22) | ✅ **fixed** |
| SK5 rectangle after R5 → R3 | left edge still moves ~10 px (was ~37 px from R20); bottom edge stays (z23) | ✗ still (smaller; live answer pending) |
| SLOT1 slot preview / committed | unchanged (z24, z25) | ✗ still |
| PAT1 linear pattern preview | unchanged (z26) | ✗ still |
| POLY1 polygon | solid construction circle, horizontal spinner (z27) | ✗ still |
| STB1 S toolbar | at cursor x+10, y−30, square corners (z28) | ✅ **fixed** |
| CH1 chamfer preview | preview chamfer + hollow arrow + two dimension lines ✅, but **no dimension values** (only extension lines and tiny arrowheads; live shows "5" twice) (z29) | ✗ **CH2** (skb, partly) |

## Update from live's answer "qa5 sketch tools" (behaviours.md ~7780, 7983) + lead's notes, 10-22
| Row | Live | Ours | Status |
|---|---|---|---|
| FIL2 fillet default | **R5 on every fresh page load**, then sticky per tab (last radius used) | UI R5 on a fresh doc ✅ (pass 19); pass 14's R20 was a leftover → **FIL2 withdrawn**; sticky: R5 → 3 → the next fillet previews R3 (z40). CLI `sketch_fillet` still documents/uses "20 mm / 0.5 in" as default | ✅ UI; ✗ minor CLI default (api) |
| SK5 radius edit (CLI, exact: rect (0,0)–(30.089,22.065), fillet top-right R5 → set the fillet's R) | R3: top 25.089, bottom 28.089, left 22.065, right 19.065. R8: top 25.089, bottom 33.089, left 22.065, right 14.065 (the edge trimmed first keeps its length, height stays, the far edge moves; a `visualSharp` point coincident with both lines) | R3: top **26.089**, bottom **29.089**, left **21.065**, right **18.065**. R8: top **23.589**, bottom **31.589**, left **23.565**, right **15.565** (the change is split over both edges; no visualSharp point) | ✗ **SK5** confirmed (skb; /root/qa5-scratch/pass20/sk5.py + sk5b.py) |
| (new) corner rectangle constraints | PERPENDICULAR, 2 × PARALLEL, HORIZONTAL + corner coincidents (glyphs ∥ ∥ ⊥ — in sketch/303) | 2 × HORIZONTAL + 2 × VERTICAL (glyphs — — \| \|) | ✗ **RECT1** (skb) |
| Trim hover | only the stub up to the intersection | whole line (k12) | ✗ **TRIM1** (skb) |
| SLOT1 constraints | 14: COINCIDENT ×6, TANGENT ×4, OFFSET ×3, DIAMETER; glyph hover shows no tooltip | tangent + equal glyphs (k21, z25) | ✗ SLOT1 (detail added) |
| CH2 chamfer | preview with two distance dimensions labelled "5", hollow arrow; Enter → box "5 mm", 2nd Enter → second distance box | preview dims without values (z29) | ✗ CH2 |

## Re-run pass 21 (2026-10-23 ~17:30–18:30, app = /root/target-lead/debug/noshape 10-23 17:10 (main 88fbf8f), CLI = target-lead noshape-cli 10-23 13:45)
| SK5 radius edit (pass20/sk5.py + sk5b.py) | R3: top 25.089, bottom 28.089, left 22.065, right 19.065; R8: 25.089 / 33.089 / 22.065 / 14.065 = live exactly | ✅ **fixed** |
| TRIM1 Trim hover | only the stub up to the rectangle edge highlighted (k40) | ✅ **fixed** |
| RECT1 corner rectangle constraints | glyphs ⊥ ∥ — on the top edge, ⊥ ∥ left, ∥ right, ∥ bottom (k40) | ✅ **fixed** |
| SLOT1 slot | pending: Ø20 with a vertical leader + 4 tangent points; committed: coincident ×4, tangent pairs and the 3 offset glyphs at the end cap (k41, k42) | ✅ **fixed** |
| PAT1 linear pattern preview | hollow arrow manipulators, square handles, "3x" with leader, spacing dimension "25" with extension lines (k43) | ✅ **fixed** |
| POLY1 polygon | spinner "◀6▶" rotated along the edge ✅; construction circle still drawn **solid** while the side count is pending (k44z) although sketch_mode pushes it as dash-dot | ✗ partly → routed skb |
| CH2 chamfer preview | two dimensions labelled "5", hollow arrow (k45) | ✅ **fixed** |

## Re-check on main a0e360a3 (qa4, 2026-10-23 19:30–20:40, FRESH=1, /root/qa4-scratch/bin/noshape-main)
| Finding | Result |
|---|---|
| RECT1 corner rectangle | ✅ glyphs ⊥ ∥ — (Perpendicular, Parallel, Horizontal) |
| FIL1 sketch fillet | ✅ R5 preview, "R5" leader, hollow grey arrow (R5 = live mm default, FIL2 withdrawn) |
| CH2 sketch chamfer | ✅ preview with two "5" dimensions and a hollow arrow |
| SLOT1 slot preview | ✅ "Ø20" with a vertical leader, four tangent points |
| TRIM1 trim hover | ✅ only the stub up to the slot lights up |
| PAT1 linear pattern | ✅ hollow arrows, square handles, "3x" with a leader, 25 spacing dimension with extension lines |
| **POLY1** inscribed polygon | ✗ **still** (= qa5 pass 21): construction circle solid while pending (looked before the vertex click, so the spinner wasn't checked) → qa5-routing POLY1 (skb). Evidence /root/qa4-scratch/gate-v047/poly1-main-a0e360a3.png |
| OFF1, VB1, STB1, SK5 | fixed in earlier passes / code (SK5 matches live's numbers, d1a1d056 test) |

| FIL2 CLI default (pass 21, 10-23) | `sketch_fillet` documents and uses 5 mm (0.5 in in inch documents) at first, then the tab's last radius | ✅ fixed |

## Re-run pass 24 (qa5, 2026-10-24 ~23:30, main 538981d5, target-lead 23:22; doc sk24)
| Row | Ours | Status |
|---|---|---|
| 2 corner rectangle | ⊥ ⫽ — glyphs (live: horizontal / vertical / parallel / perpendicular, behaviours ~149) | ✅ |
| 3 Trim the line's stub | hover highlights the stub only; removed back to the rectangle edge, coincident glyph | ✅ |
| 4–5 Offset | light-blue copy at 5 mm, hollow grey arrow, no value box; Esc cancels | ✅ |
| 6–8 sketch fillet | R5 + hollow arrow + R leader; Enter → value box clear of glyphs; R3 applied | ✅ |
| 9–10 Slot on the line | Ø20 preview with vertical leader + 4 tangent points; commits filled, Ø20, coincident / tangent glyphs, centre line solid | ✅ |
| 11 Linear pattern on a circle | 3x / 1x with leaders, hollow arrows, square handles, 25 dimension with extension lines | ✅ |
| 18 sketch Chamfer (tool search "chamfer" → 2nd row) | preview, two "5" dimensions, hollow arrow | ✅ |
| (new) Tool search in a sketch | "chamfer" lists the Part Studio **Chamfer** first and **"Sketch chamfer"** second; Enter on the first confirms the sketch and opens the feature (live names the sketch tool "Chamfer", behaviours ~7819; Part Studio features in sketch-mode search not captured) | ? live requested "qa5 (10-24, sketch tool search)" |
| (new) **SEL1** Fillet / Chamfer take sketch curves | with Fillet or Chamfer open, clicking a sketch line adds "Sketch 1 line" (live: SketchObject.NO in fillet.fs / chamfer.fs) | ✗ SEL1 (cross-area fix proposed to the lead) |

## Release re-check v0.4.8 (qa5, 2026-10-25 02:20, release asset noshape-cli 0.4.8 linux-x86_64)
| Row | Ours | Status |
|---|---|---|
| SK5 fillet radius edit (pass20/sk5.py + sk5b.py) | R3: top 25.089, bottom 28.089, left 22.065, right 19.065; R8: 25.089 / 33.089 / 22.065 / 14.065 = live | ✅ |
| POLY1 dash-dot along the whole polygon preview | not in v0.4.8 (fix eb764d1b is in ready/qa5) | — |
| POLY1 on main 358fcd83 (queue build 12:55) | inscribed polygon pending: the construction circle is dash-dot along its whole length | ✅ **fixed** |
| Tool search in a sketch (green 461aefb0) | "chamfer" → only the sketch **Chamfer** (sketch icon, live description); "fillet" → **Fillet** with "shift f" | ✅ **fixed** |
| SEL1 on green 62524c30 (Fillet, visible Sketch 5 line + plate edge) | the sketch line adds nothing; the part edge adds "Edge of Extrude 1" | ✅ **fixed** (live: fillet.fs SketchObject.NO) |
