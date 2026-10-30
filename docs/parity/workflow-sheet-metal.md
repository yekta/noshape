# Workflow: sheet metal (a CAD user's session, step by step against live Onshape)

Fresh profile (`HOME=/tmp/qa5/home`), start window → Create › Document "qa5 sheet metal" (mm), keyboard and mouse
only. Geometry numbers come from the saved document via `noshape-cli query_parts / query_entities` (same kernel).
Live reference: `/root/onshape-research/live/behaviours.md` sections "Geometry ground truth › G34" (plate + 2
flanges + Corner), "FS sheet metal, part 3" (sm4–sm8), "FS std reference + sheet metal reliefs", dialog
screenshot dialogs/sheet-metal-model-01-open.png, panel screenshot part-studio/541-g34a-flat-view.png.
Status: ✅ matches live, ✗ differs (finding #), ? live data pending (request "qa5 (10-21 sheet-metal workflow)").

**Last run: 2026-10-21 (session 12:05–13:35), qa5**, binary /tmp/qa5/noshape-1206 (target-shared build 11:50), Xvfb 1600×1000,
window 1440×900.

## Status of every row, skasm 2026-10-30 (std route, main d3d10e3f and later)
The passes below are history; this is where each row stands now. ✅ = matches live (GUI or a test with live's
numbers), ◐ = partly, ✗ = differs, — = no live data.

| Row | Now | Status | Verified by |
|---|---|---|---|
| 1.1 sketch, D 60 / D 40 | fully defined | ✅ | GUI 10-29 |
| 1.2 Sheet metal ▾ | 15 items with the Flex PCB pair (SM12) | ✅ | GUI 10-29 |
| 1.3 dialog on open | as live's screenshot, slider too | ✅ | GUI 10-29 |
| 1.4 Thicken pick | "Face of Sketch 1" | ✅ | GUI 10-29 |
| 1.5 Thickness / radius | "2 mm", stays open | ✅ | GUI 10-29 |
| 1.6 Material / Relief | K 0.45 / 0.5, gap 0.02, Simple, Obround - Scaled, 2, 1.063 (SM15) | ✅ | GUI 10-29 |
| 1.7 ✓ | 4800 mm³, Sketch 1 hidden | ✅ | GUI 10-28/29 |
| 2.1 Flange defaults | as live | ✅ | GUI 10-29 |
| 2.2 first edge | "Edge of Sheet metal model 1", preview | ✅ | GUI 10-29 |
| 2.3 x = 0 edge picked in Top view on the face boundary | the x = 0 top edge picked, chip "Edge of Sheet metal model 1" (live request 1610: the edge is picked) | ✅ | GUI 10-30 |
| 2.4 second edge | picked, 2 chips (fixed 10-29: preview picks) | ✅ | GUI 10-29, T2 flange_keeps_the_other_edges_references |
| 2.5 Distance 20 | commits | ✅ | GUI 10-29 |
| 2.6 geometry (SM1) | 8511.4987 / 24 faces; part keeps name and colour | ✅ | T2 flanges_on_adjacent_edges_match_live, flanges_keep_the_part_name_and_colour |
| 3.1 Hem dialog | defaults and lists as live | ✅ | GUI 10-30 |
| 3.2 Hem result | 9378.1835 / 33 | ✅ | T2 hem_on_g34_matches_live |
| 4.1 Tab dialog | fields as live | ✅ | GUI 10-30 |
| 4.2 Tab (SM2, SM16) | merges; auto-fills merge face and 0.02 mm (field shows it since 10-30) | ✅ | GUI 10-30, T2 tab_merges_into_the_top_face, tab_editing_logic…, UI test a_value_set_by_editing_logic… |
| 4.3 bottom face as merge | OK, 8911.4987 mm³, 28 faces, as live (request 1614 (a)) | ✅ | T2 tab_merges_into_the_bottom_face |
| 5.1 Bend relief dialog | Obround - Sized, 5 mm, ☐ Extend | ✅ | GUI 10-30 |
| 5.2 bend end vertex (SM7) | "Selection is not a bend relief." | ✅ | GUI 10-30 |
| 5.3 Corner dialog (SM8 withdrawn) | Square - Sized, 5 mm | ✅ | GUI 10-30 |
| 5.4 Corner off a corner | "Sheet metal corner not found." (the dialog also refuses such picks) | ✅ | T2 corner_off_a_corner_fails_as_live |
| 5.5 inner corner vertex | OK | ✅ | T2 closed_corner / corner tests |
| 5.6 Closed (SM3) | 23 faces, 8535.1616 (live regen 8535.1655548) | ✅ | T2 closed_corner_matches_live |
| 5.7 corner type labels | as live | ✅ | GUI 10-30 |
| 5.8 Esc after a list (SM9) | 2nd Esc cancels | ✅ | GUI 10-30 |
| 5.9 select tooltip (SM11) | none while open | ✅ | GUI 10-28 |
| 6.1 strip button | "Sheet metal table and flat view" | ✅ | GUI 10-30 |
| 6.2 panel (SM4) | live's design | ✅ | GUI 10-30 |
| 6.2 row names | letters in the order the joints' definition edges appeared (live request 1604): an edge is renewed when its ends move or a face beside it is cut down or gains a wall (a face that only grows, a Tab merged in, keeps it); renewals in place come before moved and new edges. Live's (a)/(b)/(c) cases, the Hem alone (request 1614 (b)) and qa5 (Flange, Hem, Tab, Corner: Joint A, Bend B (x = 0), Bend C (y = 0), Hem D) exact | ✅ | T2 joint_births_follow_live_letters, qa5_letters_follow_live |
| 6.3 Up (SM5) | Up | ✅ | GUI 10-30 |
| 6.4 row click | row selected, bend lit in 3D and flat | ✅ | GUI 10-30 |
| 6.5 flat (SM6, SM17) | reliefs, live placement and size | ✅ | GUI 10-30, T2 flat_pattern_keeps_the_model_placement |
| 6.6 flat menu (SM13) | live's items (New sketch… while active, Create Drawing, Select other…: the flat's two faces as "Face of <model>", Zoom to selection); clicking the flat selects it and its model face; a bend line selects (orange) and has Zoom to fit / Zoom to selection / Align view with vertical axis. Not built: Copy Flat pattern (live pastes a flat part instance into an assembly; our flats aren't parts), Add comment (cloud), a bend line's Curve/surface analysis… and Select other… | ◐ P3 | GUI 10-30 |
| 6.6 export dialog / DXF (SM14) | live's dialog and options; AC1014, live's layers | ✅ | GUI 10-30, T1 the_flat_export_options…, T2 flat_dxf_is_written_as_live |
| 6.7 part menu | no flat items | ✅ | GUI 10-30 |
| 6.8 notice (SM10) | auto-dismisses | ✅ | GUI 10-28 |
| 6.9 Finish | "Sheet metal parts" Part 1, INFO notice, table stays | ✅ | GUI 10-30 |
| 9 parts list | only Part 1 | ✅ | GUI 10-30 |
| Bend / Jog / Make joint / Tab-beside / cut / hole / counterbore then Flange / partial flange / Butt direction | see the 10-29/10-30 tables below | ✅ | T2s |
| Butt direction after a Convert | the top wall runs through (a body one Extrude made lists its faces in live's order: first side, start cap, end cap, other sides; request 1608) | ✅ | T2 convert_rips_and_bends_like_onshape |
| Sheet metal loft rect → triangle | 49 faces as live: a bend meeting the profile at a slant ends in one ruled face (rulings radial, its definition-side arc on the plane of the two walls' sides through the vertex, eased onto a rip's pulled-back side), and an edge-joint rip pulls each wall back clearance / (1 − cos) (0.018071 mm at the 96.1° seam); 17452.9223 vs live's evVolume 17452.9189 (request 1612) | ✅ | features_c::sheet_metal_loft_rectangle_to_triangle |

## 1. Base: sketch + Sheet metal model (Thicken)

| # | Step | Live | Ours | Status | Verified |
|---|---|---|---|---|---|
| 1.1 | Sketch on Top, G, rectangle from the origin; D 60, D 40 | fully defined (black) | same; bare "60" stored as "60" (see core F3) | ✅ | 41f60e47 10-28 (rectangle from the origin; the 60 typed then dimensioned) |
| 1.2 | Sheet metal ▾ list | 15 items: … Corner break, Finish sheet metal model, **Flex PCB model, Finish flex PCB model**; 199 px wide | 13 items (no Flex PCB pair), 213 px | ✅ (SM12 fixed) | 64997436 10-28 (tool search "Fill"/"Corner" lists Finish flex PCB model; the Flex PCB pair is in the list) |
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
| SM8 Corner dialog default | "Square - Sized", Corner relief width 5 mm = live ~5854 | ✅ **withdrawn** | 64997436 10-28 (GUI: Corner on smf.noshape (bin/smflange.py): Square - Sized, Corner relief width 5 mm) |
| SM9 Esc after a select list | 1st Esc closes the list, 2nd cancels the dialog | ✅ **fixed** | 64997436 10-28 (GUI: Corner's relief list open, Esc closes it, 2nd Esc cancels Corner 1) |
| SM10 "Exported …" notice | auto-dismisses; next dialog unobstructed | ✅ **fixed** | 64997436 10-28 (GUI: Part 1 › Export… › Parasolid › Save: 'Exported …x_t' gone within ~5 s) |
| SM11 select tooltip over its list | no tooltip while the list is open | ✅ **fixed** | 64997436 10-28 (GUI: hover the select 2.5 s with its list open: no tooltip) |
| SM12 / SM13 | (fixed earlier) | ✅ |  |

## skasm 2026-10-29: sheet metal runs by std (Onshape's own sheet metal FeatureScript) by default
The built-in sheet metal features now run std's own features through the FS host (`regen/std_route.rs`,
`NOSHAPE_STD_SHEET_METAL=0` for the old native code). Rows checked by tests on the std route (values from live):
| Row | Ours (std route) | Status | Verified |
|---|---|---|---|
| 2.6 SM1 flange corner | 8511.4987 mm³, 24 faces (T2 sheetmetal_std flanges_on_adjacent_edges_match_live) | ✅ (test) | |
| 3.2 hem on G34 | 9378.1835 mm³, 33 faces (T2 hem_on_g34_matches_live) | ✅ (test) | |
| 4.2 SM2 tab on the top face | OK, +400 mm³ (T2 tab_merges_into_the_top_face) | ✅ (test) | |
| 4.2 SM16 tab auto-fill | std editing logic fills Flange to merge + 0.02 mm (T2 tab_editing_logic_fills_merge_face_and_offset) | ✅ (test) | |
| 6.5 SM17 flat placement | live's (T2 flat_pattern_keeps_the_model_placement) | ✅ (test) | |
| 6.6 SM14 DXF | AC1014, live's layers (T2 flat_dxf_is_written_as_live) | ✅ (test) | |
| 5.6 SM3 Closed corner | 8541.2502 mm³ / 35 faces (live 8535.0418 / 23) | ✗ (T2 closed_corner_matches_live) | |
| Modify joint radius (table cell) | OK (e2e sheet_metal_table_ops) | ✅ (test) | |
A GUI pass of the whole list on a green build is next.

### GUI pass on the std route (skasm 10-29 22:05–23:10, /root/green/noshape f1f3117a, then agent/skasm)
| Row | Ours | Status | Verified |
|---|---|---|---|
| 1.1 | rectangle from the origin, D 60 / D 40: fully defined | ✅ | f1f3117a 10-29 |
| 1.2 | Sheet metal ▾: 15 items incl. Flex PCB model / Finish flex PCB model | ✅ | f1f3117a 10-29 |
| 1.3 | Convert tab first, Selections (3 fields, Clearance 0, ☐ Include bends, ☐ Keep input parts), General, › Material, › Relief, preview slider; title red, ✓ off (= live dialogs/sheet-metal-model-01-open.png) | ✅ | f1f3117a 10-29 |
| 1.6 | Material: K Factor, 0.45, 0.5; Relief: Minimal gap 0.02 mm, Corner Simple, **Obround - Scaled**, depth scale 2, width scale **1.063** (SM15) | ✅ | f1f3117a 10-29 |
| 2.1 | Flange dialog defaults as live | ✅ | f1f3117a 10-29 |
| 2.3 | x=0 edge in Top view at the plate's boundary: live picks the edge (request 1610); ours picks the x = 0 top edge, wall along x = 0 (the 10-29 miss was the click landing off the boundary) | ✅ | GUI 10-30 |
| 2.4 | **second edge not pickable once the first flange previewed** (std rebuild renamed all faces: the preview counted as the feature's own geometry) → fixed: faces lying on the replaced part's keep its names | ✅ after fix | agent/skasm 10-29 (GUI: both edges picked, 2 chips; T2 flange_keeps_the_other_edges_references) |
| 2.6 | two flanges: **part turned orange** (rebuilt part took a fresh palette colour) → fixed: keeps its name and colour | ✅ after fix | agent/skasm 10-29 (GUI: part stays blue; T2 flanges_keep_the_part_name_and_colour) |
| 3.1 | Hem: Straight (+ flip), ☑ Flattened, Total length 12.5 mm, Outer, Simple; lists Straight / Rolled / Tear drop, Outer / In place, Simple / Closed; chip "Edge of Sheet metal model 1" | ✅ | 6fb0344f 10-29 |
| 3.2 | Hem result | 9378.1835 / 33 by T2 hem_on_g34_matches_live | ✅ (test) | |
| 4.1 | Tab: Tab profile, Flange to merge, Subtraction offset, Subtraction scope, preview slider | ✅ | 6fb0344f 10-29 |
| 4.2 | first region click fills Flange to merge "Face of Sheet metal model 1" (SM16) and the offset 0.02 mm, but **the field kept showing "0 mm"** (value right: reopened it reads 0.02 mm) → fixed (psui: editing logic values redraw their fields); Tab OK | ✅ after fix | agent/skasm 10-29 (UI test a_value_set_by_editing_logic_shows_in_its_field) |
| 5.1 | Bend relief: field "Bend relief", Obround - Sized, Bend relief depth 5 mm, ☐ Extend bend relief | ✅ | 6fb0344f 10-29 |
| 5.2 | bend end vertex by the hem: chip "Vertex of Hem 1"; red; **"Bend relief 1 did not regenerate properly: Selection is not a bend relief."** (SM7) | ✅ | 6fb0344f 10-29 |
| 5.3 / 5.7 | Corner: Square - Sized, Corner relief width 5 mm; list Square - Sized, Rectangle - Scaled, Round - Sized, Round - Scaled, Closed, Simple | ✅ | 6fb0344f 10-29 |
| 5.8 | Esc closes the open list, the 2nd Esc cancels Corner | ✅ | 6fb0344f 10-29 |
| 6.1 | strip button tooltip "Sheet metal table and flat view" | ✅ | 6fb0344f 10-29 |
| 6.2 | "Sheet metal context: [Sheet metal model 1 ▾]"; Bends # · Name · Radius(mm) · Angle(deg) · Bend direction · K Factor; Other joints Name · Type [Rip ▾] · Style [Edge joint ▾]; 3D flat view with its view cube (SM4) | ✅ | 6fb0344f 10-29 |
| 6.2 names | ours Bend A, Bend B, Hem C, Joint D; live Joint A, Bend B, Bend C, Hem D (live letters follow Onshape's own joint order: qa4 r4 s1 had Bend A before Joint B) | ✗ minor (P3) | |
| 6.3 | flanges bent up: "Up" (SM5) | ✅ | 6fb0344f 10-29 |
| 6.4 | click "#": row selected, the bend lit in 3D and in the flat | ✅ | 6fb0344f 10-29 |
| 6.5 | flat: bend strips, dashed centre lines, relief notches at the bend ends (SM6) | ✅ | 6fb0344f 10-29 |
| 6.6 menu | flat right-click: Edit Sheet metal model 1…, Edit Sketch 1…, Show dimensions, Show dependencies…, Export DXF/DWG of Flat pattern of Part 1…, Zoom to fit, View normal to, Delete Sheet metal model 1; **missing** New sketch…, Copy Flat pattern of Part 1, Create Drawing of Flat pattern of Part 1…, Select other…, Add comment, Zoom to selection | ✗ (SM13 part) | |
| 6.6 dialog | Export DXF/DWG… opens a plain path prompt "Export flat pattern"; live has the Export DXF/DWG options dialog (file name, format, version, units, options) | ✗ (SM13 part) | |
| 6.6 dialog (fixed) | "Export as DXF/DWG": File name "<studio> - Flat pattern of Part 1" (+ View export rules), Format DXF / DWG (DWG → Version 2000, DXF → Release 14), Version, Scope (3), ☑ Use workspace units (Units grayed), Options (3), the 8 checkboxes with live's defaults; DWG writes AC1015. Scope, units, version and bend centerlines change the file; splines / z-height / tangent / counterbore / form / sketch options have nothing to write yet | ✅ | agent/skasm 10-30 (GUI on qa's smf.noshape; T1 several_flats_go_into_one_file) |
| 6.6 menu (10-30) | "Create Drawing of Flat pattern of Part 1…" added: drawing "Flat pattern of Part 1 Drawing 1", Suggested views = the flat view (bend lines, "UP 90.0° R2" notes) + the part's iso view (live also two edge views: dx). Zoom to selection added (the table's selected bend's strip, else fit); New sketch… (active models: a sketch on the base wall's face, the 3D view not turned, live request 1605). Not built: Copy Flat pattern… (live pastes a flat-pattern part instance into an assembly; our flats aren't parts), Select other… (the flat's two faces), the sketch frame drawn in the flat view, Add comment (cloud, not built anywhere) | ◐ | agent/skasm 10-30 (GUI on smf.noshape) |
| 6.6 export options (10-30) | Include bend tangent lines / counterbore and countersink lines / form outlines / form centermarks write SHEETMETAL_BEND_TANGENT_LI, _CBORE_CSINK_UP/_DOW, _FORM_UP/_DOWN, _FORM_CENTERMARK (T1 the_flat_export_options_write_their_layers) | ✅ (test) | agent/skasm 10-30 |
| 5.6 SM3 | Closed corner (T2 closed_corner_matches_live): 23 faces as live (PLANE 17, CYLINDER 4, OTHER 2), 8535.1616 mm³ vs live's regen evVolume 8535.1655548 (its 8535.0418 is /massproperties, 0.124 low on spline faces: request 1606) | ✅ | agent/skasm 10-30 |
| partial flange | Outer / Inner partial flange (live request 1599 (a)): 6027.5617 / 6204.5617 mm³, 22 faces, wall inside / outside the outline (T2 partial_flange_outer_and_inner_as_live) | ✅ (test) | agent/skasm 10-30 |
| Butt joint | Direction 1 = the wall of the first-picked flange edge (live request 1599 (b); T2 butt_joint_direction_follows_the_first_picked_flange_edge) | ✅ (test) | agent/skasm 10-30 |
| Bend dialog (live "qa5 sm tools") | Bend line (+ flip arrow), Sheet metal face to bend, Bend alignment [Bend line, Hold line, Hold other line, Inner, Outer, Middle], [Bend angle, Align to geometry, Angle from direction] + ↻, 90 deg, ☑ Use model bend radius, ☑ Use model K Factor; the empty Bend line turns pink when the focus leaves it | ✅ | agent/skasm 10-30 (GUI) |
| Jog dialog | Bend's fields + Bounding type [Blind, Up to entity, Thickness], Jog offset 5 mm, Jog offset anchor [Inside, Nominal, Outside], ☑ Preserve material | ✅ | agent/skasm 10-30 (GUI) |
| Make joint dialog | "Edges or side faces to join", Rip, Edge joint | ✅ | agent/skasm 10-30 (GUI) |
| Bend / Jog / Tab results | Bend 4812.5664 / 14 faces; Jog 4825.1327 / 22 faces, raised flange z 7..9 to x 55.889; Tab beside the plate 5400 / 10 faces, without a flange to merge "Select a sheet metal wall to join the tab to." (T2 bend_matches_live, jog_matches_live, tab_beside_the_plate_as_live) | ✅ (test) | agent/skasm 10-30 |
| Cut / Hole then Flange | Extrude Remove and Simple Hole through the plate, then a Flange: the hole stays, in the part and the flat (T2 a_cut_survives_a_later_flange, a_hole_survives_a_later_flange); Extrude Add on the part: "Only Remove can be used with sheet metal. Add Finish sheet metal model to allow other operations." | ✅ (test) | agent/skasm 10-30 |
