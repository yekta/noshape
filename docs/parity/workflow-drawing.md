# Workflow: full drawing of an assembly (a CAD user's session, step by step against live Onshape)

Document "qa5 assembly" (see workflow-assembly.md): Assembly 1 = plate + shaft + pins (patterns) + block +
imported bracket; BOM row 1 (Part 1 = plate) has Part number "PN-001". Drawing created from the Assembly 1 tab.
Live reference: behaviours.md "Drawing" (Create drawing, Insert view, toolbar, Callout, GTol, Datum, Detail view,
Auxiliary view reference indicator, Sheet properties, Export Drawing modal), live DXF samples/drw2-*.dxf.
Status: ✅ matches live, ✗ differs (finding #), ? not verified.

**Last run: 2026-10-21 (session 12:05–13:35), qa5**, binary /tmp/qa5/noshape-1206.

## 1. Create, insert views

| # | Step | Live | Ours | Status | Verified |
|---|---|---|---|---|---|
| 1.1 | Assembly tab › Create Drawing of Assembly 1… | modal "Create Drawing: … Drawing 1", Existing templates / Custom template, filter All/ANSI/ISO/JIS, **ANSI_A_INCH.dwt selected by default** | same layout; **no template row selected** | ✗ D1 |  |
| 1.2 | ANSI_A_MM, No views, OK | new tab right after the source, A sheet, Insert view panel, view follows the cursor | same | ✅ | 0.4.10+ / daf1f50c 10-27 (asm5 Assembly 1 › Create Drawing) |
| 1.3 | Insert view panel (assembly) | Insert + reference, View orientation, View scale, View simplification, ☐ Show sketch appearances | + Explode/Position, Display state; scale list without "(Sheet)" marker | ? |  |
| 1.4 | Insert › picker "Select a part or assembly" | filter icons **Part Studios / Parts / Sketches / Flat patterns**, thumbnails, "Flat pattern of Part 1" rows | tabs Part Studios / Assemblies, **no filter icons, no thumbnails, sketches always listed, no flat patterns** | ✗ D2 |  |
| 1.5 | First view, projected Top/Right/iso by moving away | third-angle placement, iso by diagonal | same | ✅ | 0.4.10+ / daf1f50c 10-27 (asm5 Assembly 1 › Create Drawing) (front, right, top, iso) |
| 1.6 | Undo ×3 (views) | views removed | removed, but an **"Out of date — Update (Ctrl+Q)"** notice appears though nothing changed | ✗ D3 |  |
| 1.7 | Title block DWG NO. of the assembly drawing | the assembly's part number ("----" when empty) | **"PN-001" = Part 1's part number** (CLI: only Part 1 has PN-001) | ✗ D4 |  |
| 1.8 | Section view (Horizontal), click a hole centre, drag down, place, ✓ | "SECTION A-A", arrows, hatching | same | ✅ | 0.4.10+ / daf1f50c 10-27 (asm5 Assembly 1 › Create Drawing) ("SECTION A - A", arrows, hatch; Vertical tab) |
| 1.9 | Detail view: circle centre + radius, place | "DETAIL B / SCALE 1:1" (2× parent), circle + label on the parent | same | ✅ | 0.4.10+ / daf1f50c 10-27 (asm5 Assembly 1 › Create Drawing) ("DETAIL A / SCALE 2:1", circle + label on the parent) |
| 1.10 | Auxiliary view on a straight edge, place | "Auxiliary C - C", indicator arrows at the parent | same | ✅ | c38eb9cf (qa 10:55: Auxiliary view on the front view's top edge, placed above → "Auxiliary A - A" (first aux view), A–A indicator arrows at the parent) |

## 2. Annotations

| # | Step | Live | Ours | Status | Verified |
|---|---|---|---|---|---|
| 2.1 | **d** after closing the Auxiliary view with Esc | Dimension tool | **nothing**; later (after clicking the sheet) d works | ✗ D5 |  |
| 2.2 | Dimension: click an edge 7 px from its midpoint, place | ? (in the Part Studio live picks the midpoint near it, behaviours.md line 258) | picks the **midpoint** and waits for a second entity | ? D6 |  |
| 2.3 | Click the edge away from the midpoint, place | "100" | "100"; tool stays | ✅ | 0.4.10+ / daf1f50c 10-27 (asm5 Assembly 1 › Create Drawing) ("60") |
| 2.4 | Circle in the top view, place | Ø10 with leader | same | ✅ | 0.4.10+ / daf1f50c 10-27 (asm5 Assembly 1 › Create Drawing) (Ø10 with leader) |
| 2.5 | Datum on an edge | filled triangle + leader + framed "A" | same; next label "B" | ✅ | 0.4.10+ / daf1f50c 10-27 (asm5 Assembly 1 › Create Drawing) (A then B) |
| 2.6 | GTol dialog | Upper text, Prefix, Type ⌖▾, Tolerance, 3 Datums, Suffix, + / Ø, Lower text; **✓ enabled once something is entered** | same fields; **typing right after opening is lost** although Tolerance shows focus (needs a click); ✓ stays dimmed with content | ✗ D7 |  |
| 2.7 | Each click places another frame, Esc ends | yes | yes | ✅ | c38eb9cf (qa 10:55: Callout, three clicks → three "----" frames, Esc ends the tool) |
| 2.8 | Note (n), click, type, ✓ | in-place editor with ruler, formatting toolbar | same | ✅ | 0.4.10+ / daf1f50c 10-27 (asm5 drawing) (editor with ruler + formatting toolbar) |

## 3. BOM table, balloons, sheet properties

| # | Step | Live | Ours | Status | Verified |
|---|---|---|---|---|---|
| 3.1 | BOM table, place top right | Item / Quantity / Part number / Description, rows from the assembly BOM | same; quantities 1/1/6/1/1, PN-001 | ✅ | 0.4.10+ / daf1f50c 10-27 (asm5 Assembly 1 › Create Drawing) (Item / Quantity / Part number / Description: 1 / 1, 2 / 2) |
| 3.2 | Callout on a part edge in the assembly view (BOM table present) | balloon shows the **BOM item number** | balloon shows **"----"** | ✗ D8 |  |
| 3.3 | Right-click sheet › Sheet properties… | Scale / Format / Size / Reference | same; scale "1:2" Enter + ✓ updates the title block SCALE | ✅ | 0.4.10+ / daf1f50c 10-27 (asm5 drawing) (1:2 + ✓: SCALE 1:2, views shrink; datums stay put: DATUM-SCALE-DETACHED) |

## 4. Export

| # | Step | Live | Ours | Status | Verified |
|---|---|---|---|---|---|
| 4.1 | Tab › Export…, PDF | File name + **"View export rules" link**, Format, Version 1.5, Overridden dimensions, Text, Color, Options, ☐ Hide inspection items | same, without the "View export rules" link | ✗ D9 (minor) |  |
| 4.2 | PDF content | as on screen | letter landscape, 1 page; **iso view exported shaded** (on screen: hidden-line); **Detail B circle border lost / content not clipped** | ✗ D10 |  |
| 4.3 | DXF options | **Version default Release 14** (+ "Model space only" line), **Sheets ▾ All/Current/Custom** | Version default **2018**, no Sheets ▾ | ✗ D11 |  |
| 4.4 | DXF content | CIRCLE/ARC/ELLIPSE, HATCH, MTEXT, DIMENSION, ACAD_TABLE, INSERT; layers Visible, TANGENT_LINES, VIEW_HATCHES, CENTERMARKS, ANNOTATION_TEXT, ANNOTATION_LINES, SECTION_CUTTING_LINE, TABLES, BORDER_*, TITLE_BLOCK | drw2 10-21 (vs live drw2-k-dxf2018 / R14 / R12): CIRCLE / ARC / ELLIPSE / SPLINE / POLYLINE, HATCH, MTEXT, DIMENSION over *D blocks, TOLERANCE, hole callouts as INSERT *U, ACAD_TABLE (INSERT before 2004), OLE2FRAME; live layer names, case and order, Defpoints with dimensions; Release 11-12 view geometry only. All sheets in one file: the first in model space, the others in layouts "Sheet2"… (`cad_export::to_dxf_sheets`; R13/14 strips layouts, R11-12 side by side as live); per-kind text styles Onshape-Standard-Fields / -View-Label / -NoteStyle / hole tags (`CadTextStyle`) | ✅ D12 (dx 10-31: rechecked, both landed earlier) |  |

## Findings (2026-10-21 run)

| # | Finding | Repro | Owner |
|---|---|---|---|
| D4 | **Wrong data in the title block**: an assembly drawing shows Part 1's part number as DWG NO. | BOM: Part number of row 1; drawing of the assembly | asb |
| D8 | Balloons (Callout) on assembly parts show "----" even with a BOM table | §3.2 | asb |
| D12 | DXF export explodes circles/arcs, hatches, dimensions, tables into lines/polylines; layer names differ from Onshape's | §4.4 | asb / drw2 |
| D10 | PDF: iso view comes out shaded; detail view border/clip lost | §4.2 | asb |
| D2 | Insert view picker: no filter icons (Parts/Sketches/Flat patterns), no thumbnails, no flat-pattern entries | §1.4 | asb |
| D6 | (unverified; may match live) Dimension: a click near an edge's midpoint picks the midpoint, not the edge | §2.2 | asb |
| D5 | Drawing shortcuts (d) dead after closing a view dialog with Esc until the sheet is clicked | §2.1 | asb / ui2 |
| D7 | GTol: focused Tolerance field drops keys typed on open; ✓ stays dimmed with content | §2.6 | asb |
| D3 | Undoing views shows a spurious "Out of date" notice | §1.6 | asb |
| D11 | DXF export options: default version 2018 (live R14), no Sheets selector | §4.3 | asb |
| D1 | Create Drawing: no default template selected | §1.1 | asb / ui2 |
| D9 | Export Drawing: no "View export rules" link | §4.1 | ui2 |

## Re-run pass 3 (2026-10-21 15:25–15:40, /tmp/qa5/noshape-1448 = target-shared 14:48; new "Assembly 1 Drawing 2" + existing Drawing 1)

| # | Row | Ours | Status | Verified |
|---|---|---|---|---|
| 1.1 | Create Drawing dialog | ANSI_A_INCH.dwt selected by default; "No views" option pre-selected (live default ?) | ✅ D1 fixed | 0.4.10+ / daf1f50c 10-27 (asm5 Assembly 1 › Create Drawing) (ANSI_A_INCH selected, No views preselected) |
| 1.4 | Insert picker | filter icons Part Studios / Parts / Sketches / Flat patterns + thumbnails | ✅ D2 fixed | c38eb9cf (qa 10:54: Insert view › Insert → "Select a part or assembly": filter toggles for the kinds present (Part Studios, Parts on, Sketches; seven defined in live order), Part 1 row with its thumbnail, Import… row) |
| 1.6 | Place a view, Ctrl+Z | view removed, no "Out of date" notice ✅; **title-block SCALE changes from "1:1" (before the view) to "----" after the undo** | ✅ D3 fixed / ✗ **D14** (minor) | c38eb9cf (qa 10:52, wf-block Part 1 › Create Drawing, No views: place a Front view (SCALE 1:1), Ctrl+Z → view gone, SCALE "----", no Out of date notice; Ctrl+Y brings it back at 1:1) |
| 1.7 | DWG NO. of an assembly drawing | "----" (both drawings) | ✅ D4 fixed | 0.4.10+ / daf1f50c 10-27 (asm5 Assembly 1 › Create Drawing) ("----") |
| 2.6 | GTol: type right after opening | "0.1" lands in Tolerance, ✓ enabled | ✅ D7 fixed | c38eb9cf (qa 10:54: GTol tool, type 0.1 at once → "0.1" in Tolerance, ✓ green) |
| 3.2 | Callout balloons in the existing drawing | still "----" with a BOM table present | ✗ D8 |  |
| 4.1 | Export dialog | "View export rules" link present | ✅ D9 fixed | 0.4.10+ / daf1f50c 10-27 (asm5 drawing) |
| 4.3 | DXF options | Release 14 + "Model space only", Sheets ▾ "All sheets" ✅; **Units "Drawing units"** (live "Use drawing units"), **☐ Set z-height to zero…** (live ☑ by default, behaviours.md D12 answer) | ✅ D11 fixed / ✗ **D15** | 0.4.10+ / daf1f50c 10-27 (asm5 drawing) (Release 14, Model space only, All sheets, Use drawing units) |
| – | Tab menus (assembly / drawing tab) | no "Open in new browser tab" (browser-only, ok), no "Show code" | ? |  |
| 2.1, 4.2, 4.4 | D5, D10, D13 | not re-run this pass | – |  |

## Re-run on integration 5d28447 (2026-10-21 17:05–17:20, /tmp/qa5/noshape-1659 = /root/target-lead/debug/noshape 16:59; no CLI built from 5d28447 yet, so CLI-only geometry rows stay on the 14:36 CLI result)
| D8 balloons | "----" | ✗ still |
| D15 DXF defaults | z-height ☐, "Drawing units" | ✗ still |
| D14 | not re-run (tab switch missed) | – |

## Re-run pass 7 (2026-10-21 18:56–19:50, app /tmp/qa5/noshape-1851 = target-shared 18:51 (tree d5745c9 + uncommitted work), CLI /tmp/qa5/noshape-cli-1809 = target-engine 18:09)
Pass 6 (17:11) had found D5 fixed (d starts Dimension after Esc).
| Row | Ours | Status | Verified |
|---|---|---|---|
| D8 balloons | the old front-view balloon now shows **3**; a new balloon on a cylinder in the isometric view still shows "----" | ✗ still (partly) |  |
| D15 DXF defaults | ☑ Set z-height to zero…, Units "Use drawing units" | ✅ **fixed** | 0.4.10+ / daf1f50c 10-27 (asm5 drawing) (☑ z-height to zero) |
| D14 SCALE after undo | "----" on the empty sheet, "1:1" with a view, "----" after Ctrl+Z (consistent) | ✅ **fixed** (live value on an empty sheet ?) | c38eb9cf (qa 10:52, wf-block Part 1 › Create Drawing, No views: "----" after the undo, "1:1" with the view (and while the Insert-view preview follows the cursor)) |
| D16 (new) | the "Sheet1" chip draws over the Insert view dialog's title ("Sheet1 ew") | ✗ **D16** (asb / ui) |  |

## Re-run pass 8 on main 5bd50b2 (2026-10-21 23:13–00:10, app /tmp/qa5/noshape-2256 = /root/target-lead/debug/noshape 22:56; no newer CLI than target-engine 18:09, so CLI-only rows stay on pass 7; geometry rows re-read in the app where possible)
Live answered "qa5 pass 7" (3): SCALE "----" before any view, 1:1 after → D14 ✅ confirmed.
| D8 balloon on an isometric-view cylinder | "----" | ✗ still |
| D16 Sheet1 chip over Insert view title | gone | ✅ **fixed** |

## Re-run pass 13 (2026-10-22 ~05:20–06:40, main aa75fdf: app /tmp/qa5/noshape-0503 = /root/target-lead/debug/noshape 05:03, CLI /tmp/qa5/noshape-cli-0226 = target-engine 02:26)
| D8 balloon on an isometric-view cylinder | not re-run (the "qa5 assembly" doc is gone) | – |

## Re-run pass 14 (2026-10-22, main aa75fdf, app /tmp/qa5/noshape-0503; doc rebuilt: /tmp/qa5/d14/asm.noshape via /root/qa5-scratch/pass14/mkasm2.py = mkasm + plate fixed, block, shaft)
| Row | Ours | Status | Verified |
|---|---|---|---|
| D8 Callout on an assembly part in the iso view with a BOM table (Create Drawing › ANSI_A_MM, Suggested views; BOM table top right) | balloon shows the BOM item number "2" (p08, p09) | ✅ (BALLOON-WRONG-ITEM fixed) | 0.4.10+ / daf1f50c 10-27 · verified 6444d892 10-28 (TICKETS BALLOON-WRONG-ITEM) |

## Re-check (qa4, 2026-10-23 20:40–21:10, main a0e360a3; code and the latest qa5 passes)
- Fixed in qa5's later passes (rows marked ✅ above): D2, D3, D5, D7, D8, D9, D11, D14, D15, D16; D1, D4 fixes cited in code.
- Still open, not re-run since pass 3: **D10** (PDF: iso view shaded, Detail circle/clip lost) and **D12** (DXF explodes circles,
  arcs, hatches, dimensions, tables into lines) → qa5-routing D10 / D12 (**api**, owns export).

## Re-run pass 23 (qa5, 2026-10-24, app from agent/qa5 on main c703b0d0, which has api's drawing export b4b803af; d10 doc)
| Row | Ours now | Status | Verified |
|---|---|---|---|
| D10 PDF | tab › Export… › PDF 1.5 Color: the isometric view is **lines** (not shaded); Detail A keeps its circle and the clipped content, "DETAIL A / SCALE 2:1" (pass23/d10pdf-1.png) | ✅ fixed (api) | 0.4.10+ / daf1f50c 10-27 (PDF 1.5, 1 page 792×612, lines, Detail A clipped + SCALE 2:1) |
| D12 DXF | Release 14: CIRCLE ×5, ELLIPSE ×2, MTEXT ×43, LINE, POLYLINE; layers VISIBLE, TITLE_BLOCK, BORDER_ZONES / BORDER_FRAME, DETAIL_VIEW_BORDER / DETAIL_VIEW_PARENT_BORDER, ANNOTATION_TEXT (R14 writes upper-case layer names, behaviours ~6165) | ✅ fixed (api) for this drawing; hatches, dimensions and tables not in it (not checked) | 0.4.10+ / daf1f50c 10-27 (asm5 drawing) (R14 AC1014, INSUNITS 4: DIMENSION 2, LINE 95, CIRCLE 7, ELLIPSE 4, MTEXT 46, POLYLINE 9, SOLID 4) |
| D13 DRAWN name after reopening (pass 24, 2026-10-24) | d10 drawing: "ROOT" in DRAWN before and after closing and reopening the document | ✅ fixed | c38eb9cf (qa 10:56: drawing closed with the app and reopened → DRAWN "ROOT", views, dimension, callouts and the auxiliary view all kept) |
| D6 / 2.2 Dimension near an edge's midpoint (pass 24) | d, hover 7 px off the left view edge's midpoint: the whole edge highlights; click, move, click: length dimension "20" of the edge (live "asb (b)": the edge is picked, never its midpoint) | ✅ | c38eb9cf (qa 10:52, wf-block Part 1 › Create Drawing, No views: d, hover 7 px along the left edge from its midpoint → whole edge orange; click, move, click → the edge's length dimension (at 7 px perpendicular nothing highlights; live measured along the edge too)) |
| D8 (dx 10-27, main 9932ff31, CLI) | Callout on the pin's silhouette in an assembly drawing's isometric view (plate + Ø10 pin) | the pin's BOM item number "2" (drawing_get now lists silhouettes; /root/dx-scratch/d8/d8.py) | ✅ D8 | 78958d0f 10-28 (GUI, asm5 Assembly 1 Drawing 1 with a BOM: Callout tool, click the left pin's silhouette in the isometric, place → circled "2" = its BOM item) |
| D12 entities (dx 10-27, main dbc12611, CLI, demo drawing 4 sheets) | DXF 2018: DIMENSION 6, HATCH 6, ACAD_TABLE 4, MTEXT, LWPOLYLINE; Release 14: DIMENSION 6, HATCH 6, INSERT 4, POLYLINE/VERTEX, MTEXT (live drw2-k-dxfR14: DIMENSION, HATCH, INSERT, POLYLINE/VERTEX, MTEXT) | dimensions, hatches and tables are entities, not exploded | ✅ D12 | v0.4.12 CLI 10-28 (asm5 Assembly 1 Drawing 1 with the BOM + callout, drawing_export DXF (default AC1014 = R14): BOM as 1 INSERT, POLYLINE/VERTEX, MTEXT 41, LINE / CIRCLE / ELLIPSE; this drawing has no dimensions or hatches, the 2018 variant not run) |
