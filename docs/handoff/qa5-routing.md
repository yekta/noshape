# QA routing (qa4 + qa5)

Findings handed to their owner. **Append only**: add new entries at the end, never edit or delete another agent's
entry; to change a status, append a line `STATUS <id>: …` with date and agent. One owner per entry.

Format: `## <id> — <owner> — <one-line summary>` then repro, expected (live), actual, evidence.

## IMP1 — kern — imported parts share face names: selection highlight crosses parts
- Source: qa4, core-workflow v0.4.6 row (d), re-checked 2026-10-23 on main 88fbf8f5.
- Repro: + › Import… /root/onshape-research/live/samples/g48-two.step (defaults). Click Part 1 in the Parts list, then Part 2.
- Expected (live): only the clicked part is highlighted.
- Actual: Part 1 also highlights the whole cube (Part 2); Part 2 also highlights two faces of the G48 body.
- Cause: `crates/model/src/regen/features/importing.rs` `import_foreign` names every body on its own with
  `new_face_names(ev, &p)` → `naming.rs name_faces` → `TopoName::face(feature, Role::New(ordinal))`, the ordinal counted per
  body. Two bodies of one Import feature both get New(0…), so `Reference::Face` names collide and `PickMap::ids_for` returns
  faces of both parts. A per-feature running ordinal (or a body index in the role) fixes it; names of the first body can stay.

## IMP2 — live request — Parts list order after a two-part STEP import
- Source: qa4, v0.4.6 (d). noshape lists Part 2 above Part 1 after importing g48-two.step; live's order not captured.

## R1 — render (lead to assign) — two default planes selected: parts drawn with black halos and see-through holes
- Source: qa4, v0.4.6 gate 2026-10-22; also on v0.4.5 (not a regression).
- Repro: demo document, click Top then Right in the feature list (or Front + Right). One plane selected draws normally;
  clearing the selection restores it. 3/3.
- Expected (live): parts unchanged, both planes highlighted.

## PAT-DIR — psb — failed Feature pattern: the Direction is drawn orange, not red
- Source: qa4, v0.4.7 gate dd1ca034 (a). "lp wall over", Reapply off, Direction = block's long edge (or Right plane).
- Expected (live 2258, behaviours.md "URGENT … failed Feature pattern"): Direction as a red #bd3039 line (a face: red outline +
  light red fill). Actual: selection orange (edge) / orange-tinted (plane). Ghosts, seed outline and edges are right.
- Likely cause: 83afe3c2 adds `state::ERROR` to the Direction's pick ids but leaves `SELECTED`, which wins.
- Evidence: /root/qa4-scratch/gate-v047/a2-direction-edge-orange.png.

## ASM-ESC — ui (asb for assembly dialogs) — assembly pattern dialog: Esc in an unedited focused field does nothing
- Source: qa4, 2026-10-22 (QuantityInput migration).
- Repro: assembly, Linear pattern, click Instance count (no edit), Esc. Expected (live, feature dialogs): the dialog cancels.
  Actual: nothing; Esc with the focus outside the fields closes it.
- Cause: the pattern dialog root has no `key_context("FeatureDialog")`, so the "FeatureDialog > Input" Esc → Cancel binding
  doesn't apply. Adding the context also binds Tab → DialogTab, which the pattern dialog doesn't handle yet.

STATUS R1: fixed on main (833bfb0b, in 88fbf8f5); re-checked in the app 2026-10-23 17:50 (qa4): Top + Right selected, parts draw normally. Closed.
STATUS ASM-ESC: fixed on main (833bfb0b); re-checked 2026-10-23 17:52 (qa4): unedited Instance count, Esc → dialog cancels. Closed.

STATUS IMP1: owner now **api** (owns import/export, lead 2026-10-23).
STATUS PAT-DIR: fixed on release/v0.4.7 (lead 2026-10-23); qa4 re-checks it in the v0.4.7 re-gate.

## EXP1 — api — export headers still name Open CASCADE / noshape
- Source: qa5 workflow-document DOC1 / I5; re-checked 2026-10-23 with noshape-cli v0.4.7 (export_parts iges / parasolid).
- IGES global section "Open CASCADE IGES processor 8.0", file name "Filename.iges", author "root"; Parasolid header
  `APPL=noshape`, `SCH_1200000_12006`. Expected: as live's exports (header fields as Onshape writes them).

## SF2 — kern — Extrude of two adjacent sketch regions makes a seam
- Source: qa5, pass 21 re-run 2026-10-23 (main 88fbf8f / 40e23871).
- Repro: Sketch on Top: corner rectangles (0,0)–(20,20) and (20,0)–(40,20); Extrude both regions 10 mm (/root/qa5-scratch/pass3/sf2.py)
- Expected (live): one part 8000 mm³, **6 faces**
- Actual: 8000 mm³, **10 faces** (seam faces not merged)

## SF5 — kern — Move boundary on one edge moves all edges
- Source: qa5, pass 21 re-run 2026-10-23 (main 88fbf8f / 40e23871).
- Repro: qa5 surfaces doc (/root/qa5-scratch/run/docs/cli/surf.noshape): "Move boundary 2" on one edge of the planar Fill surface, 10 mm
- Expected (live): only that edge moves: 800 + 200 = 1000 mm²
- Actual: OK but all four edges move: **2400 mm²**, bbox −10..50 × 30..70

## SF6 — kern — Move boundary returns OK with a corrupt surface
- Source: qa5, pass 21 re-run 2026-10-23 (main 88fbf8f / 40e23871).
- Repro: same doc, "Move boundary 3" on the boundary surface's bspline edge, 10 mm
- Expected (live): 1200 + 300 mm²
- Actual: status OK, **area −514.304543 mm²**

## ADV2 — kern — Loft square → circle volume
- Source: qa5, pass 21 re-run 2026-10-23 (main 88fbf8f / 40e23871).
- Repro: Loft of a 20×20 square at z=0 to a Ø16 circle at z=20 (qa5 advanced doc, Part Studio 1)
- Expected (live): **5927.728022** mm³, sides 324.6214685 mm² each
- Actual: **5927.754632** (+0.027); area matches

## IM2 — kern — Parasolid/STEP imports of B-spline models are a little off in volume
- Source: qa5, pass 21 re-run 2026-10-23 (main 88fbf8f / 40e23871).
- Repro: import samples/g38b-ktane_Bracket.x_t, g38b-psas_Bell_Crank.x_t, g38b-mcmaster-91274A118.x_t, g26-revolved-spline.x_t, g48-types.step, g49-chamfer1/2.x_t, g50-*.x_t (/root/qa5-scratch/pass3/impmatrix16.log)
- Expected (live): ktane 19377.868698, Bell Crank 2050.807089, McMaster 254.521984, g26 1726.080764, g48 22595.3688588, g49 44789.850213 / 44833.437797, g50-g46 6052.696796, g50-reorder 5990.519587
- Actual: ktane 19378.847 (−0.98), Bell Crank 2050.837, McMaster 254.404 (+0.118), g26 1726.0983, g48 22595.3712, g49 44789.973 / 44833.530, g50-g46 6053.320 (+0.62), g50-reorder 5990.523; face counts all match

## I1 — kern — g25-b: the same model imported as SAT / STEP / X_T gives three volumes
- Source: qa5, pass 21 re-run 2026-10-23 (main 88fbf8f / 40e23871).
- Repro: import g25-b.sat, .step, .x_t
- Expected (live): one volume 7582.713024 (Parasolid exact)
- Actual: SAT 7585.978524 / STEP 7582.713024 / X_T 7583.000869

## IM1 — kern — PVZ import: part names and tabs
- Source: qa5, pass 21 re-run 2026-10-23 (main 88fbf8f / 40e23871).
- Repro: import samples/pvz-shapes.pvz
- Expected (live): Part Studio "pvz-shapes parts" + Assembly "RootAssembly" + BOM tab; parts **Cylinder, Torus, Cone**
- Actual: one tab "pvz-shapes", parts Part 1/2/3 (no names from the file), no assembly/BOM tab (kern/fs)

## AN4 — kern — Interference size tooltip uses a loose bounding box
- Source: qa5, pass 21 re-run 2026-10-23 (main 88fbf8f / 40e23871).
- Repro: block 30×20×10 + a copy shifted 10 mm; Interference detection; hover the result row
- Expected (live): "L x W x H" of the overlap: 30 mm x 20 mm x 10 mm
- Actual: "35.485 mm x 20.233 mm x 10 mm" (OCCT Bnd_Box with gap/tolerance, not the tight box)

## DOC1 — kern — IGES export header (crates/kernel/cpp/io.cpp write_iges)
- Source: qa5, pass 21 re-run 2026-10-23 (main 88fbf8f / 40e23871).
- Repro: Part › Export… › IGES (or `export_parts` IGES); compare with samples/qa5-iges-header.igs (behaviours.md "qa5 iges header")
- Expected (live): Start section 1 text line; Global: 24-hex product id (fields 3 and 12), file name, system id + version strings, **32,24,8,56,15**, scale 1., units **2 / MM**, line weights **1000 / 1.**, date **13H261023.180059** (YYMMDD.HHNNSS, UTC), resolution **1D-05**, max coord **1000000.**, author and organization **empty**, IGES version flag **9** (5.1), drafting 0, mod date = date; sequence numbers space-padded ("G      1"); extension .iges
- Actual: empty Start line; "Open CASCADE IGES processor 8.0", "Filename.iges", 32,308,15,308,15, line weights 1 / 0.01, 15H20261023.175701 (4-digit year), 0.0001, 80.0001 (bbox), author "root", version 11, "G0000001" zero-padded. The system/product strings need a decision (we shouldn't claim to be HOOPS Exchange): same as I5

## I5 — kern — Parasolid export header
- Source: qa5, pass 21 re-run 2026-10-23 (main 88fbf8f / 40e23871).
- Repro: Part › Export… › Parasolid
- Expected (live): `**PART1;` MC/MC_MODEL/MC_ID/OS/OS_RELEASE=unknown, `FRU=Parasolid 38.1.231`, `APPL=Onshape`, SITE/USER=unknown, FORMAT=text, GUISE=transmit, KEY/FILE=BTGapi::writeModels_.xmt, DATE=…; `**PART2; SCH=SCH_3801231_38101` (samples/g25-a.x_t)
- Actual: `MC=unknown;APPL=noshape;FORMAT=text;GUISE=transmit;` and `SCH=SCH_1200000_12006` (needs a decision: product/schema strings)

## RHINO1 — kern — Rhino export (qa5 EXP1)
- Source: qa5, pass 21 re-run 2026-10-23 (main 88fbf8f / 40e23871).
- Repro: Part › Export… › Rhino (or `export_parts` format RHINO)
- Expected (live): .3dm written
- Actual: "unsupported: Rhino export needs openNURBS: build with OPENNURBS_ROOT" (packaging: the shipped build must include openNURBS)

## SM1 — fs — Two flanges on adjacent edges: corner geometry
- Source: qa5, pass 21 re-run 2026-10-23 (main 88fbf8f / 40e23871). Note for the fs rows: crates/model/tests/sheetmetal_std.rs gets live's SM1 / hem / SM2 numbers on the std FeatureScript path (STD_SHEET_METAL on), but the built-in features the app and CLI use still give the old results.
- Repro: Sheet metal model Thicken of rect (0,0)–(60,40) on Top, t 2, r 2; Flange 20 mm on the top edges y=0 and x=0 (/root/qa5-scratch/run/docs/cli/sm.noshape, roll back after Flange 1)
- Expected (live): **8511.4987 mm³, 24 faces**; walls run to x/y = 0.02 (0.02 mm gap at the corner), bend ends x/y = 2
- Actual: **8383.1217, 20 faces**; walls stop at x/y = 2.02 with the bends, two 0.04 mm² sliver faces

## SM-hem — fs — Hem after SM1
- Source: qa5, pass 21 re-run 2026-10-23 (main 88fbf8f / 40e23871).
- Repro: same doc, roll back after Hem 1
- Expected (live): 9378.1835 mm³, 33 faces
- Actual: 9254.2367, 30 faces

## SM3 — fs — Corner › Closed
- Source: qa5, pass 21 re-run 2026-10-23 (main 88fbf8f / 40e23871).
- Repro: same doc, Corner 1 cornerStyle CLOSED
- Expected (live): OK, 8535.0418 mm³, 23 faces (bends run to the corner, 0.02 mm diagonal gap)
- Actual: "Closed corners are not supported yet"

## SM2 — fs — Tab sketched on the definition plane with the plate's top face as Flange to merge
- Source: qa5, pass 21 re-run 2026-10-23 (main 88fbf8f / 40e23871).
- Repro: plate as SM1 (not finished); Sketch on **Top plane**: rect (60,10)–(75,30); Tab, Flange to merge = plate top face (/root/qa5-scratch/pass20/smtab3.py)
- Expected (live): OK (live "qa5 sm" / sm8)
- Actual: "The tab must lie in the plane of a sheet metal wall" (works when sketched on the top face itself)

## SM5 — fs — Bend direction labels
- Source: qa5, pass 21 re-run 2026-10-23 (main 88fbf8f / 40e23871).
- Repro: the SM1 doc (flanges bent up + hem): sheet metal table / flat view / DXF
- Expected (live): **Up** for all three (DXF layer SHEETMETAL_BEND_LINES_UP)
- Actual: **Down** for all three (`FlatBendLine.up = Bend::is_up() != flip_direction_up`, is_up = ref_inner): looks inverted, but live's Z-profile case (behaviours ~1365) has one Up + one Down, so check the rule against it before flipping

## SM6 — fs — Flat outline lacks relief cut-outs that the folded part has
- Source: qa5, pass 21 re-run 2026-10-23 (main 88fbf8f / 40e23871).
- Repro: SM1 doc + Bend relief 1 + Corner 1, flat view
- Expected (live): (needs a live check of the flat outline)
- Actual: flat outline has no reliefs

## SM4 — fs — Sheet metal table and flat view panel is a different design (fs + ui)
- Source: qa5, pass 21 re-run 2026-10-23 (main 88fbf8f / 40e23871).
- Repro: any sheet metal part, right strip "Sheet metal table and flat view"
- Expected (live): "Sheet metal context: [Sheet metal model 1 ▾]", ⌄ Bends grid (#, Name, Radius, Angle, Bend direction, K Factor; editable → adds "Modify joint N"), ⌄ Other joints grid (Name, Type ▾, Style ▾), lower half a shaded 3D flat viewport with its own view cube (behaviours "qa5 sm" (8), part-studio/541)
- Actual: "Sheet metal" tab, part name, white line drawing, summary rows, Dir/Angle/Radius/Allow table, Export DXF / Export SVG buttons (tooltip + strip position fixed by qa5 10-23)

## POLY1 — skb — Polygon construction circle while the side count is pending
- Source: qa5, pass 21 re-run 2026-10-23 (main 88fbf8f / 40e23871).
- Repro: Inscribed polygon: click centre, click a vertex; look before committing
- Expected (live): dash-dot construction circle (sketch/91)
- Actual: solid circle (crates/scene/src/sketch_mode.rs pushes it to `dash_dot`, but it renders solid at that stage)

## ADV9 — psb — Feature mirror (Reapply features off, the default) of a gusset Add-extrude that touches two faces
- Source: qa5, pass 21 re-run 2026-10-23 (main 88fbf8f / 40e23871).
- Repro: /root/qa5-scratch/pass3/mkbracket2.py (qa5 bracket, step 8: Mirror › Feature pattern, the gusset extrude, Plane 1 = Right + 40 mm)
- Expected (live): OK, 67191.3275 mm³ (live "qa5 bracket", Mirror 1 8 ms)
- Actual: "Could not create all instances as entered. Try selecting \"Reapply features\" option." (OK with Reapply ☑)

## BOOL1b — psb — Boolean "no geometry change" row hover
- Source: qa5, pass 21 re-run 2026-10-23 (main 88fbf8f / 40e23871).
- Repro: Union of two disjoint parts (qa5 bool doc), ✓, hover the row
- Expected (live): just "Boolean 1"
- Actual: "Boolean 1 <- Extrude 1, Extrude 3"

## FSW-draft — psb — Parting line draft: any parting line
- Source: qa5, pass 21 re-run 2026-10-23 (main 88fbf8f / 40e23871).
- Repro: Draft › Parting line, parting edges not in one plane perpendicular to the pull, or on two parts
- Expected (live): each edge's faces drafted about the edge (draft.fs), hint faces
- Actual: "Parting lines must lie in a plane perpendicular to the pull direction" / "…must belong to one part"

## FSW-move — psb — Move face › Reapply fillet
- Source: qa5, pass 21 re-run 2026-10-23 (main 88fbf8f / 40e23871).
- Repro: Move face on a face next to a fillet, Reapply fillet ☑
- Expected (live): fillets touching the moved faces are removed and re-rolled
- Actual: option ignored

## FSW-replace — psb — Replace face with several replace-with faces
- Source: qa5, pass 21 re-run 2026-10-23 (main 88fbf8f / 40e23871).
- Repro: Replace face, two adjacent template faces
- Expected (live): all used (adjacent, one body)
- Actual: only the first

## FSW-sweep — psb — Thin sweep
- Source: qa5, pass 21 re-run 2026-10-23 (main 88fbf8f / 40e23871).
- Repro: Sweep › Thin, Trim ends off, scale
- Expected (live): sweep a sheet, thicken it, trim ends to planes; "Failed to thicken swept body, check thickness."
- Actual: wall offset first, then swept; own texts

## FSW-loft — psb — Loft guides, path, end conditions, connections, trims
- Source: qa5, pass 21 re-run 2026-10-23 (main 88fbf8f / 40e23871).
- Repro: Loft with guides / path / end conditions / 2 connections
- Expected (live): supported, with their errors ("Select a point on each profile to define a connection.")
- Actual: "Loft guides are not supported yet", "Loft path is not supported yet", end conditions only warn, first connection only, trims ignored

## FSW-fill — psb — Fill guides
- Source: qa5, pass 21 re-run 2026-10-23 (main 88fbf8f / 40e23871).
- Repro: Fill with guide curves; Fill on surface edges
- Expected (live): guide errors ("Select guide entities." …), exact curves in Precise mode; Add automatic on surface edges (SF7)
- Actual: not checked; sampled; no Add

## FSW-bsurf — psb — Boundary surface profile arrays and conditions
- Source: qa5, pass 21 re-run 2026-10-23 (main 88fbf8f / 40e23871).
- Repro: Boundary surface, U/V profiles with conditions
- Expected (live): driven-query rows "[None] Edge of Sketch 1", max 2 picks; conditions applied with their errors
- Actual: "Add Profile" UI, no cap; conditions warn only

## FSW-helix — psb — Helix start/end point rules, start/end profiles
- Source: qa5, pass 21 re-run 2026-10-23 (main 88fbf8f / 40e23871).
- Repro: Helix with Start point off the axis plane; Show start and end profiles ☑
- Expected (live): errors on startPoint/endPoint unless coplanar with the axis start/end, snapped onto the radius; magenta circles
- Actual: moves the plane, own errors; nothing drawn

## FSW-mc — psb — Mate connector owner / attachment at regen
- Source: qa5, pass 21 re-run 2026-10-23 (main 88fbf8f / 40e23871).
- Repro: Mate connector on a face, move the face
- Expected (live): connector follows its owner/attachment
- Actual: not stored

## FSW-pattern — psb — Pattern infos and fallbacks (L40/42/43/45 live pending)
- Source: qa5, pass 21 re-run 2026-10-23 (main 88fbf8f / 40e23871).
- Repro: Feature pattern with a failing instance; Face pattern off the part; Curve pattern start point
- Expected (live): geometric fallback rules, off-part instances fail, start-point info, skip-instance infos
- Actual: own behaviour

## FSW-hole — psb — Hole merge scope and infos (L56/L57 live pending)
- Source: qa5, pass 21 re-run 2026-10-23 (main 88fbf8f / 40e23871).
- Repro: Hole with an empty scope / fastener fit < 2 parts / tapped depth readjusted
- Expected (live): scope auto-filled with the parts reached, direction flipped; "No target parts selected for hole."; infos
- Actual: empty = every solid; infos missing

## FSW-unsupported — psb — Features still "not supported yet"
- Source: qa5, pass 21 re-run 2026-10-23 (main 88fbf8f / 40e23871).
- Repro: Body draft; Decal; Rib draft; asymmetric / conic / curvature face blends; Face blend between faces without a common edge
- Expected (live): supported
- Actual: "… is not supported yet"

## VC7 — ui — Menu font app-wide
- Source: qa5, pass 21 re-run 2026-10-23 (main 88fbf8f / 40e23871).
- Repro: any menu
- Expected (live): Flama 12 px `#555`
- Actual: our UI font (Flama / Open Sans not bundled; theme-layout.md)

## KS1 — ui — Keyboard shortcuts pop-out
- Source: qa5, pass 21 re-run 2026-10-23 (main 88fbf8f / 40e23871).
- Repro: Help › Keyboard shortcuts, the ↗ next to ×
- Expected (live): `a.osx-popout-link` opens a full-size "Keyboard shortcuts" page (no tabs/search, every category in two columns, legend bar, footer); the modal stays open
- Actual: missing

## SM4-ui — ui — Sheet metal table and flat view panel
- Source: qa5, pass 21 re-run 2026-10-23 (main 88fbf8f / 40e23871).
- Repro: see fs SM4
- Expected (live): see fs SM4
- Actual: old design

## VC8 — ui — Assembly camera menu
- Source: qa5, pass 21 re-run 2026-10-23 (main 88fbf8f / 40e23871).
- Repro: Assembly tab, view cube ▾
- Expected (live): Isometric, Dimetric, Trimetric | ⚙ Graphics preferences… | Named views…, Previous view | Zoom to fit, Zoom to window | Perspective view | ✓ render mode ▸, ✓ hidden edges ▸, ✓ tangent edges ▸ | View in high quality | Section view… (= the Part Studio menu minus "Orient normal to sketch on edit" and "Highlight boundary edges"; behaviours.md "qa5 assembly camera menu")
- Actual: qa5 removed "Show mates" and added "Section view…"; the assembly view has no render modes, named / previous views, graphics preferences, zoom to window or high quality yet

## D12 — asb — DXF export explodes entities; layer names differ
- Source: qa5, pass 21 re-run 2026-10-23 (main 88fbf8f / 40e23871).
- Repro: any drawing with circles, a hatch, dimensions and a table; Export › DXF
- Expected (live): CIRCLE/ARC/ELLIPSE, HATCH, MTEXT, DIMENSION, ACAD_TABLE, INSERT; layers Visible, TANGENT_LINES, VIEW_HATCHES, CENTERMARKS, ANNOTATION_TEXT, ANNOTATION_LINES, SECTION_CUTTING_LINE, TABLES, BORDER_*, TITLE_BLOCK (live drw2-*.dxf)
- Actual: circles/arcs/hatches/dimensions/tables as lines/polylines; other layer names
