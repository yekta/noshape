# Workflow: sheet metal (a CAD user's session, step by step against live Onshape)

Fresh profile (`HOME=/tmp/qa5/home`), start window → Create › Document "qa5 sheet metal" (mm), keyboard and mouse
only. Geometry numbers come from the saved document via `noshape-cli query_parts / query_entities` (same kernel).
Live reference: `/root/onshape-research/live/behaviours.md` sections "Geometry ground truth › G34" (plate + 2
flanges + Corner), "FS sheet metal, part 3" (sm4–sm8), "FS std reference + sheet metal reliefs", dialog
screenshot dialogs/sheet-metal-model-01-open.png, panel screenshot part-studio/541-g34a-flat-view.png.
Status: ✅ matches live, ✗ differs (finding #), ? live data pending (request "qa5 (10-21 sheet-metal workflow)").

**Last run: 2026-10-21 (session 12:05–13:35), qa5**, binary /tmp/qa5/noshape-1206 (target-shared build 11:50), Xvfb 1600×1000,
window 1440×900.

## 1. Base: sketch + Sheet metal model (Thicken)

| # | Step | Live | Ours | Status | Verified |
|---|---|---|---|---|---|
| 1.1 | Sketch on Top, G, rectangle from the origin; D 60, D 40 | fully defined (black) | same; bare "60" stored as "60" (see core F3) | ✅ | 77870dd4 10-28 (rect from origin; 60 needed retyping: Tab drop, fixed in 1eefebb9 not yet green) |
| 1.2 | Sheet metal ▾ list | 15 items: … Corner break, Finish sheet metal model, **Flex PCB model, Finish flex PCB model**; 199 px wide | 13 items (no Flex PCB pair), 213 px | ✗ SM12 |  |
| 1.3 | Sheet metal model dialog on open | Convert tab, Selections (3 query fields, Clearance 0, ☐ Include bends, ☐ Keep input parts), General, › Material, › Relief; title red + ✓ disabled until a selection | same layout and states | ✅ | 77870dd4 10-28 |
| 1.4 | Thicken tab, click the region | chip "Face of Sketch 1" | same | ✅ | 77870dd4 10-28 |
| 1.5 | Thickness "2" Enter, Bend radius "2" Enter | "2 mm", value re-selected, dialog stays | same | ✅ | 77870dd4 10-28 |
| 1.6 | Material / Relief defaults | K 0.45, rolled K 0.5, min. gap 0.02 mm, corner Simple, bend relief Obround depth scale 2, width scale 1.0625 | same values; width scale shown "1.062" | ? (display precision) |  |
| 1.7 | ✓ | Part 1, plate z 0..2, 4800 mm³ | 4800 mm³, Sketch 1 auto-hidden | ✅ | 77870dd4 10-28 |

## 2. Flange (G34: top edges y=0 and x=0, 20 mm, defaults)

| # | Step | Live | Ours | Status | Verified |
|---|---|---|---|---|---|
| 2.1 | Flange dialog defaults | flangeAlignment INNER, BLIND 25 mm, bend angle 90, auto miter on, model radius, partial off | Inner, Blind, 25 mm, Bend angle 90 deg, ☑ Automatic miter, ☑ Use model bend radius, ☐ Partial flange | ✅ (spec) / ? (look) | 77870dd4 10-28 |
| 2.2 | Click the top edge at y=0 | "Edge of Sheet metal 1", preview | "Edge of Sheet metal model 1", preview | ✅ | 77870dd4 10-28 |
| 2.3 | Pick the second (x=0) edge in Top view at the face boundary | ? (edge expected) | picks the **top face** (hover shows the face outline) → feature red, no preview | ? |  |
| 2.4 | Pick the x=0 side from the Left view | chip | "Edge of Sheet metal model 1", both flanges preview | ✅ | 77870dd4 10-28 |
| 2.5 | Distance "20" Enter Enter | commits | commits | ✅ | 77870dd4 10-28 |
| 2.6 | Geometry | **8511.4987 mm³, 24 faces (PLANE 20, CYL 4)**; walls run to x/y = 0.02 (0.02 mm gap at the corner), bend ends x/y = 2 | **8383.1217 mm³, 20 faces (PLANE 16, CYL 4)**; walls stop at x/y = **2.02** with the bends, two 0.04 mm² sliver faces at (2.01, 2, 1) | ✗ SM1 | 77870dd4 10-28 ✗ SM-FLANGE-CORNER (8383.122) |

## 3. Hem (top edge at x=60)

| # | Step | Live | Ours | Status | Verified |
|---|---|---|---|---|---|
| 3.1 | Dialog defaults | hemType STRAIGHT, useMinimalGap true, length 12.5 mm, OUTER, SIMPLE | Straight, ☑ Flattened, Total length 12.5 mm, Outer, Simple | ✅ spec / ? labels | 77870dd4 10-28 |
| 3.2 | Pick edge, ✓ | ? | OK; 30 faces, 9254.2367 mm³; closed hem r 0.01, obround relief r 1.0625 where it meets the flange | ? |  |

## 4. Tab (Sketch 2 on Top: x 20..40, y 40..50; merge = plate top face)

| # | Step | Live | Ours | Status | Verified |
|---|---|---|---|---|---|
| 4.1 | Dialog | fields tabFaces, booleanUnionScope, booleanOffset, booleanSubtractScope | Tab profile, Flange to merge, Subtraction offset 0 mm, Subtraction scope | ? labels |  |
| 4.2 | Region + top face | **OK** (sm8, sketch on the definition plane Top) | **"Tab 1 did not regenerate properly: The tab must lie in the plane of a sheet metal wall"**; "Subtraction scope" outlined red | ✗ SM2 |  |
| 4.3 | Same with the bottom face (coplanar with the sketch; CLI) | – | OK | (shows the rule we apply) |  |

## 5. Bend relief, Corner

| # | Step | Live | Ours | Status | Verified |
|---|---|---|---|---|---|
| 5.1 | Bend relief dialog | bendReliefStyle SIZED_OBROUND, depth scale 2, width scale 1.0625, depth 5 mm, ☐ extend | "Obround - Sized", Bend relief depth 5 mm, ☐ Extend bend relief | ? SM8 |  |
| 5.2 | Pick the vertex at the y=0 bend end next to the hem, ✓ | ? | chip "**Vertex**"; OK, but volume unchanged (9254.236696), faces 30→31 | ? SM7 |  |
| 5.3 | Corner dialog | cornerStyle SIZED_RECTANGLE (scale-driven: sm6g "scale 2"), scale 1.5 | "Square - Sized" showing **Corner relief width 5 mm** (CLI: cornerStyle SIZED_RECTANGLE, visible squareReliefWidth) | ✗ SM8 |  |
| 5.4 | Top-of-wall vertex | ? | rejected (red) | ? |  |
| 5.5 | Inner corner vertex (2,2,2) from Top view, ✓ | OK | OK, 33 faces, 9245.4674 mm³ | ? |  |
| 5.6 | Corner type Closed (G34a) | **OK, 8535.0418 mm³, 23 faces** | **"Closed corners are not supported yet"** (Error) | ✗ SM3 |  |
| 5.7 | Corner type list | SIZED_RECTANGLE / RECTANGLE / SIZED_ROUND / ROUND / CLOSED / SIMPLE | Square - Sized, Rectangle - Scaled, Round - Sized, Round - Scaled, Closed, Simple | ? labels |  |
| 5.8 | Esc, Esc after closing a select's list | 2nd Esc cancels the dialog (no edit pending) | dialog stays open (3× Esc) | ✗ SM9 |  |
| 5.9 | Select tooltip while its list is open | ? | "Flange alignment" / "Corner relief type" tooltip stays over the list | ✗ SM11 |  |

## 6. Sheet metal table, flat view, DXF

| # | Step | Live | Ours | Status | Verified |
|---|---|---|---|---|---|
| 6.1 | Right strip button | "Sheet metal table and flat view" | tooltip "Sheet metal: flat pattern and bend table" | ? |  |
| 6.2 | Panel layout | "Sheet metal context: [Sheet metal 1 ▾]"; ⌄ **Bends** grid (# · Name "Bend A" · Radius(mm) · Angle(deg) · Bend direction · K Factor); ⌄ **Other joints** grid (Name · Type ▾ Rip · Style ▾ Edge joint); lower half a shaded **3D flat viewport** with its own view cube | "Sheet metal" tab header, "Part 1", white 2D outline drawing, Flat size / Thickness / Area / Bends, a "# Dir Angle Radius Allow." list, **Export DXF / Export SVG** buttons; no names, no K column, no joints/rips, no context selector, no editable cells | ✗ SM4 |  |
| 6.3 | Bend direction of flanges bent up | "Up" | "**Down**" (table and DXF text "DOWN 90° R2") | ✗ SM5 |  |
| 6.4 | Click a table row | highlights (?) | nothing | ? |  |
| 6.5 | Flat outline | includes corner/bend reliefs (?) | plain 12-vertex outline, no relief cut-outs although the 3D part has them | ? SM6 |  |
| 6.6 | Right-click the flat → Export as DXF/DWG… | options dialog (request 10) | no context menu on the flat; Export DXF button → plain path prompt, R12 DXF: layers VISIBLE (1 POLYLINE, 12 vertices), HIDDEN (3 bend LINEs), ANNOTATION (3 TEXT) | ? SM13 |  |
| 6.7 | Part right-click menu | … Create Drawing of Part 1…, Export as DXF/DWG…, Export… | adds "Export flat pattern (DXF)…" and "Export flat pattern (SVG)…" | ✗ SM13 |  |
| 6.8 | "Exported …" notice | ? | never auto-dismisses; draws over the next feature dialog's title and ✓ | ✗ SM10 |  |
| 6.9 | Finish sheet metal model, part, ✓ | ? | OK | ? |  |

## Findings (2026-10-21 run)

| # | Finding | Repro | Owner |
|---|---|---|---|
| SM1 | **Wrong flange corner geometry**: two flanges on adjacent edges stop at the bend ends (x/y = 2.02) instead of running to the 0.02 mm min-gap corner; two 0.04 mm² sliver faces; 8383.1217 vs live 8511.4987 mm³, 20 vs 24 faces | 60×40 Thicken t2 r2; Flange top edges y=0 + x=0, 20 mm | fs |
| SM2 | **Tab fails** with the top face as "Flange to merge" ("The tab must lie in the plane of a sheet metal wall"); live OK. Only the face lying on the sketch plane works | §4 | fs |
| SM3 | **Corner › Closed not implemented** ("Closed corners are not supported yet"); live OK 8535.0418 mm³, 23 faces | §5.6 | fs |
| SM4 | **Sheet metal table and flat view panel is a different design** from live (see 6.2) | right strip button | fs + ui |
| SM5 | Bends bent up reported as **Down** (table + DXF annotation) | §6.3 | fs |
| SM6 | Flat outline has no relief cut-outs that the folded part has (needs live confirmation) | §6.5 | fs |
| SM7 | Bend relief on a bend-end vertex: OK but no geometry change; chip text "Vertex" (other chips say "Edge of Sheet metal model 1") | §5.2 | fs |
| SM8 | Corner default SIZED_RECTANGLE shows the absolute width field; live drives it by the scale (1.5). Same suspicion for Bend relief "Obround - Sized" showing depth 5 mm. Labels "Square - Sized" etc. unverified | §5.1, 5.3 | fs |
| SM9 | Esc after using a dialog select never cancels the dialog | Corner, open/close the type list, Esc ×3 | ui |
| SM10 | "Exported …" notice stays forever and covers the next feature dialog's header/✓ | Export DXF, then Finish sheet metal model | ui |
| SM11 | A select's tooltip stays over its open list | Flange alignment, Corner type | ui |
| SM12 | Sheet metal ▾ lacks Flex PCB model / Finish flex PCB model | toolbar | fs / ui |
| SM13 | Flat export lives in invented places (panel buttons, part menu items); live: flat view right-click › Export as DXF/DWG… with an options dialog (pending live capture) | §6.6–6.7 | fs / psb |

## Update from live's answer (behaviours.md "qa5 sheet metal", 10-21)

| # | Row | Live | Verdict |
|---|---|---|---|
| 1.2 | Sheet metal ▾ | 15 items incl. Flex PCB model / Finish flex PCB model | SM12 confirmed |
| 1.6 | Relief defaults | Bend relief type **Obround - Scaled**, width scale shown **"1.063"** | ours shows "1.062" → **SM15** (rounding 1.0625: live rounds half up) |
| 2.6 | Flange geometry | 8511.4987 mm³, 24 faces | SM1 confirmed || 77870dd4 10-28 ✗ SM-FLANGE-CORNER (8383.122) 
| 3.2 | Hem default | **9378.1835 mm³, 33 faces (27 plane, 6 cyl)**; Closed changes nothing here | ours 9254.2367, 30 faces (23 plane, 7 cyl): ✗ (on top of SM1) |
| 4.2 | Tab | first click on the region **auto-fills "Flange to merge" = plate face and sets Subtraction offset 0.02 mm**; OK 9778.1835, 37 faces | SM2 confirmed + **SM16**: no auto-fill of merge face / offset |
| 5.1–5.2 | Bend relief | Obround - Sized, depth 5 mm (ours matches); picking a bend end gives **ERROR "Bend relief 1 did not regenerate properly: Selection is not a bend relief."** (it only edits existing reliefs) | SM7 updated: ours reports OK and adds a face |
| 5.3 | Corner default | Square - Sized with **Corner relief width 5 mm** (live too) | **SM8 withdrawn** |
| 5.4 | Corner on the miter edge | ERROR "Corner 1 did not regenerate properly: Sheet metal corner not found." | ours: rejected (red) ✅ |
| 5.7 | Corner type labels | Square - Sized, Rectangle - Scaled, Round - Sized, Round - Scaled, Closed, Simple | ✅ |
| 6.1 | Strip button | tooltip "Sheet metal table and flat view" | ours "Sheet metal: flat pattern and bend table" → part of SM4 |
| 6.2–6.4 | Table | "Sheet metal context: [Sheet metal model 1 ▾]"; Bends (# Name Radius Angle Bend direction K Factor; Bend B, Bend C, Hem D), Other joints (Joint A, Rip ▾, Edge joint ▾); no Corners table; **click "#" selects the row and highlights the bend yellow in 3D and flat**; editing a cell adds a "Modify joint 1" feature | SM4 confirmed; SM5 (Up) confirmed |
| 6.5 | Flat | single solid, bend zones as strips, dark dashed centre lines; **bbox x −18.5553…71.3388, y −18.5553…50 (89.8941 × 68.5553)** | ours 89.894 × 68.555 ✅ size, but **rotated 90° and moved to positive coordinates** → **SM17** |
| 6.6 | Flat right-click | Edit Sheet metal model 1…, Edit Sketch 1…, Show dimensions, Show dependencies…, New sketch…, Copy Flat pattern of Part 1, Create Drawing of Flat pattern of Part 1…, **Export DXF/DWG of Flat pattern of Part 1…**, Select other…, Add comment, Zoom to fit, Zoom to selection, View normal to, Delete Sheet metal model 1 | SM13 confirmed (ours: no menu) |
| 6.6 | Export DXF/DWG dialog | File name "qa5 sm - Flat pattern of Part 1", Format DXF/DWG, Version (R14 for DXF), Scope, ☑ Use workspace units, Units, Options, ☐ Export splines as polylines, ☑ Set z-height to zero…, ☑ Include bend centerlines, ☐ Include bend tangent lines, ☐ cbore/csink lines, ☐ form outlines, ☐ form centermarks, ☑ Include visible sketches | ours: no dialog → SM13 |
| 6.6 | DXF content | **AC1014**, 18 LINEs on **SHEETMETAL_CUT_LINES**, 3 on **SHEETMETAL_BEND_LINES_UP** (CENTER), 49 Onshape layers in the table, no TEXT | ours AC1009, POLYLINE on VISIBLE, bend LINEs on HIDDEN, 3 TEXT "DOWN 90° R2" → **SM14** |
| 6.7 | Part right-click | no flat-pattern items | SM13 confirmed |
| 9 | Parts list | only Part 1, no Flat pattern entry | ✅ |
| 11 | Finish | one field "Sheet metal parts", chip "Part 1"; status INFO; table stays populated | ours OK ✅ (table state not checked) |

New findings: **SM14** flat DXF format/layers/text differ (fs/kern); **SM15** 1.0625 shown 1.062 (ui, number formatting); **SM16** Tab doesn't auto-fill the merge face and 0.02 mm offset (fs); **SM17** flat pattern rotated/translated instead of model placement (fs). Withdrawn: **SM8**.

## Re-run pass 3 (2026-10-21 15:10, CLI /tmp/qa5/noshape-cli-1436; UI /tmp/qa5/noshape-1448)

| # | Row | Ours | Status | Verified |
|---|---|---|---|---|
| 2.6 | Flange (rollback after Flange 1) | 8383.1217, 20 faces (live 8511.4987, 24) | ✗ SM1 |  | 77870dd4 10-28 ✗ SM-FLANGE-CORNER (8383.122) |
| 3.2 | Hem default (rollback after Hem 1) | 9254.2367, 30 faces (live 9378.1835, 33) | ✗ |  |
| 5.6 | Corner → Closed | "Closed corners are not supported yet" | ✗ SM3 |  |
| 1.2 | Sheet metal ▾ | 13 items, no Flex PCB pair | ✗ SM12 |  |

## Re-run on integration 5d28447 (2026-10-21 17:05–17:20, /tmp/qa5/noshape-1659 = /root/target-lead/debug/noshape 16:59; no CLI built from 5d28447 yet, so CLI-only geometry rows stay on the 14:36 CLI result)
| SM1 flange (UI roll-back + Mass properties) | 8383.122 | ✗ still |
| SM3 Closed corner | "Closed corners are not supported yet" | ✗ still |
| SM12 Sheet metal ▾ | 13 items | ✗ still |
| SM13 part menu | "Export flat pattern (DXF)…" still there | ✗ still |

## Re-run pass 7 (2026-10-21 18:56–19:50, app /tmp/qa5/noshape-1851 = target-shared 18:51 (tree d5745c9 + uncommitted work), CLI /tmp/qa5/noshape-cli-1809 = target-engine 18:09)
| Row | Ours | Status | Verified |
|---|---|---|---|
| SM1 flange (CLI rollback after Flange 1) | 8383.1217, 20 faces | ✗ still (live 8511.4987) |  |
| 3.2 hem (CLI rollback after Hem 1) | 9254.2367, 30 faces | ✗ still (live 9378.1835, 33) |  |
| SM3 Closed corner | "Corner 1 did not regenerate properly: Closed corners are not supported yet" | ✗ still |  |
| SM12 Sheet metal ▾ | 13 items, no Flex PCB pair | ✗ still |  |
| SM13 part menu | "Export flat pattern (DXF)…" still there | ✗ still |  |

## Re-run pass 8 on main 5bd50b2 (2026-10-21 23:13–00:10, app /tmp/qa5/noshape-2256 = /root/target-lead/debug/noshape 22:56; no newer CLI than target-engine 18:09, so CLI-only rows stay on pass 7; geometry rows re-read in the app where possible)
| SM1 flange (app Mass properties, rollback after Flange 1) | 8383.122 mm³ | ✗ still |
| 3.2 hem (rollback after Hem 1) | 9254.237 mm³ | ✗ still |
| SM3 Closed corner | "Closed corners are not supported yet" | ✗ still |
| SM12 Sheet metal ▾ | 15 items, incl. Flex PCB model + Finish flex PCB model | ✅ **fixed** |
| SM13 part menu | no "Export flat pattern (DXF)…" | ✅ **fixed** |

## Re-run pass 12 (2026-10-22 ~01:35, CLI /tmp/qa5/noshape-cli-0107 = target-engine 01:07; main still 5bd50b2)
| Row | Ours | Status | Verified |
|---|---|---|---|
| SM1 flange (rollback after Flange 1) | 8383.1217, 20 faces | ✗ still (live 8511.4987) |  |
| 3.2 hem (rollback after Hem 1) | 9254.2367, 30 faces | ✗ still (live 9378.1835, 33) |  |

## Re-run pass 13 (2026-10-22 ~05:20–06:40, main aa75fdf: app /tmp/qa5/noshape-0503 = /root/target-lead/debug/noshape 05:03, CLI /tmp/qa5/noshape-cli-0226 = target-engine 02:26)
| Row | Ours | Status | Verified |
|---|---|---|---|
| SM1 flange / 3.2 hem (CLI rollback) | 8383.1217 / 20 faces; 9254.2367 / 30 faces | ✗ still |  |
| SM3 Corner › Closed (CLI feature_edit cornerStyle CLOSED) | "Closed corners are not supported yet" | ✗ still |  |

## Re-run pass 15 (2026-10-22, CLI /tmp/qa5/noshape-cli-0521 = target-engine 05:21; main still aa75fdf)
CLI rows unchanged: SF2 8000/10 faces, SF5 2400 mm², SF6 −514.304543; SM1 8383.1217/20, hem 9254.2367/30, SM3 "Closed corners are not supported yet"; ADV2 5927.754632; EXP1 openNURBS. ✗ still.

## Re-run pass 18 (2026-10-22 ~10:00–11:00, CLI /tmp/qa5/noshape-cli-0811 = target-engine 08:11; main 3ce6952)
CLI rows unchanged: SF2 8000/10 faces, SF5 2400 mm², SF6 −514.304543; SM1 8383.1217/20, hem 9254.2367/30, SM3 "Closed corners are not supported yet"; ADV2 5927.754632; EXP1 openNURBS; SML1 loft Add "not supported yet". ✗ still.

## Re-run pass 21 (2026-10-23 ~17:30–18:30, app = /root/target-lead/debug/noshape 10-23 17:10 (main 88fbf8f), CLI = target-lead noshape-cli 10-23 13:45)
| Row | Ours | Status | Verified |
|---|---|---|---|
| SM1 flange (CLI rollback) | 8383.1217, 20 faces | ✗ still → routed fs (qa5-routing.md) |  |
| hem (CLI rollback) | 9254.2367, 30 faces | ✗ still → routed fs |  |
| SM3 Closed corner | "Closed corners are not supported yet" | ✗ still → routed fs |  |
| SM4 panel design | old design (tab header, line drawing, Export DXF/SVG buttons) | ✗ still → routed ui + fs (big); **tooltip and strip position fixed by qa5** (see below) |  |
| 6.1 strip tooltip / order | was "Sheet metal: flat pattern and bend table", last button | **qa5 fix**: "Sheet metal table and flat view", placed above Variable table (live "qa5 sm" (8)) |  |
| SM5 Up/Down | "Down" ×3 for flanges bent up + hem (live all "Up") | ✗ still → routed fs (sign rule needs a live Down case) |  |
| SM7 chip "Vertex" | Corner chip "Vertex" (live "Vertex of Flange 1") | **qa5 fix**: vertex chips read "Vertex of <latest creating feature>" |  |
| SM8 Corner dialog default | "Square - Sized", Corner relief width 5 mm = live ~5854 | ✅ **withdrawn** |  |
| SM9 Esc after a select list | 1st Esc closes the list, 2nd cancels the dialog | ✅ **fixed** |  |
| SM10 "Exported …" notice | auto-dismisses; next dialog unobstructed | ✅ **fixed** |  |
| SM11 select tooltip over its list | no tooltip while the list is open | ✅ **fixed** |  |
| SM12 / SM13 | (fixed earlier) | ✅ |  |
