# qa5 routing: open ✗ rows that need a kernel, FeatureScript or big area change

qa5 re-runs every docs/parity/workflow-*.md against current main and fixes local UI rows itself. Rows here are not local:
each has an owner, a repro and the live reference. Status as of the last re-run named in the row.
Scripts live in /root/qa5-scratch (not in the repo); CLI = `noshape-cli call <tool> '<json>'`.

## kern (kernel / import / export geometry) — 9 rows

| ID | Row | Repro | Live | Ours (re-run 10-23, main 88fbf8f, CLI 13:45) |
|---|---|---|---|---|
| SF2 | Extrude of two adjacent sketch regions makes a seam | Sketch on Top: corner rectangles (0,0)–(20,20) and (20,0)–(40,20); Extrude both regions 10 mm (/root/qa5-scratch/pass3/sf2.py) | one part 8000 mm³, **6 faces** | 8000 mm³, **10 faces** (seam faces not merged) |
| SF5 | Move boundary on one edge moves all edges | qa5 surfaces doc (/root/qa5-scratch/run/docs/cli/surf.noshape): "Move boundary 2" on one edge of the planar Fill surface, 10 mm | only that edge moves: 800 + 200 = 1000 mm² | OK but all four edges move: **2400 mm²**, bbox −10..50 × 30..70 |
| SF6 | Move boundary returns OK with a corrupt surface | same doc, "Move boundary 3" on the boundary surface's bspline edge, 10 mm | 1200 + 300 mm² | status OK, **area −514.304543 mm²** |
| ADV2 | Loft square → circle volume | Loft of a 20×20 square at z=0 to a Ø16 circle at z=20 (qa5 advanced doc, Part Studio 1) | **5927.728022** mm³, sides 324.6214685 mm² each | **5927.754632** (+0.027); area matches |
| IM2 | Parasolid/STEP imports of B-spline models are a little off in volume | import samples/g38b-ktane_Bracket.x_t, g38b-psas_Bell_Crank.x_t, g38b-mcmaster-91274A118.x_t, g26-revolved-spline.x_t, g48-types.step, g49-chamfer1/2.x_t, g50-*.x_t (/root/qa5-scratch/pass3/impmatrix16.log) | ktane 19377.868698, Bell Crank 2050.807089, McMaster 254.521984, g26 1726.080764, g48 22595.3688588, g49 44789.850213 / 44833.437797, g50-g46 6052.696796, g50-reorder 5990.519587 | ktane 19378.847 (−0.98), Bell Crank 2050.837, McMaster 254.404 (+0.118), g26 1726.0983, g48 22595.3712, g49 44789.973 / 44833.530, g50-g46 6053.320 (+0.62), g50-reorder 5990.523; face counts all match |
| I1 | g25-b: the same model imported as SAT / STEP / X_T gives three volumes | import g25-b.sat, .step, .x_t | one volume 7582.713024 (Parasolid exact) | SAT 7585.978524 / STEP 7582.713024 / X_T 7583.000869 |
| IM1 | PVZ import: part names and tabs | import samples/pvz-shapes.pvz | Part Studio "pvz-shapes parts" + Assembly "RootAssembly" + BOM tab; parts **Cylinder, Torus, Cone** | one tab "pvz-shapes", parts Part 1/2/3 (no names from the file), no assembly/BOM tab (kern/fs) |
| AN4 | Interference size tooltip uses a loose bounding box | block 30×20×10 + a copy shifted 10 mm; Interference detection; hover the result row | "L x W x H" of the overlap: 30 mm x 20 mm x 10 mm | "35.485 mm x 20.233 mm x 10 mm" (OCCT Bnd_Box with gap/tolerance, not the tight box) |
| EXP1 | Rhino export | Part › Export… › Rhino (or `export_parts` format RHINO) | .3dm written | "unsupported: Rhino export needs openNURBS: build with OPENNURBS_ROOT" (packaging: the shipped build must include openNURBS) |

## fs (sheet metal / FeatureScript features) — 7 rows

Note (10-23): crates/model/tests/sheetmetal_std.rs asserts live's SM1 / hem / SM2 numbers on the **std FeatureScript** path, but the built-in features the app and CLI use still give the old results (re-checked on a CLI built from main 88fbf8f).

| ID | Row | Repro | Live | Ours |
|---|---|---|---|---|
| SM1 | Two flanges on adjacent edges: corner geometry | Sheet metal model Thicken of rect (0,0)–(60,40) on Top, t 2, r 2; Flange 20 mm on the top edges y=0 and x=0 (/root/qa5-scratch/run/docs/cli/sm.noshape, roll back after Flange 1) | **8511.4987 mm³, 24 faces**; walls run to x/y = 0.02 (0.02 mm gap at the corner), bend ends x/y = 2 | **8383.1217, 20 faces**; walls stop at x/y = 2.02 with the bends, two 0.04 mm² sliver faces |
| SM-hem | Hem after SM1 | same doc, roll back after Hem 1 | 9378.1835 mm³, 33 faces | 9254.2367, 30 faces |
| SM3 | Corner › Closed | same doc, Corner 1 cornerStyle CLOSED | OK, 8535.0418 mm³, 23 faces (bends run to the corner, 0.02 mm diagonal gap) | "Closed corners are not supported yet" |
| SM2 | Tab sketched on the definition plane with the plate's top face as Flange to merge | plate as SM1 (not finished); Sketch on **Top plane**: rect (60,10)–(75,30); Tab, Flange to merge = plate top face (/root/qa5-scratch/pass20/smtab3.py) | OK (live "qa5 sm" / sm8) | "The tab must lie in the plane of a sheet metal wall" (works when sketched on the top face itself) |
| SM5 | Bend direction labels | the SM1 doc (flanges bent up + hem): sheet metal table / flat view / DXF | **Up** for all three (DXF layer SHEETMETAL_BEND_LINES_UP) | **Down** for all three (`FlatBendLine.up = Bend::is_up() != flip_direction_up`, is_up = ref_inner): looks inverted, but live's Z-profile case (behaviours ~1365) has one Up + one Down, so check the rule against it before flipping |
| SM6 | Flat outline lacks relief cut-outs that the folded part has | SM1 doc + Bend relief 1 + Corner 1, flat view | (needs a live check of the flat outline) | flat outline has no reliefs |
| SM4 | Sheet metal table and flat view panel is a different design (fs + ui) | any sheet metal part, right strip "Sheet metal table and flat view" | "Sheet metal context: [Sheet metal model 1 ▾]", ⌄ Bends grid (#, Name, Radius, Angle, Bend direction, K Factor; editable → adds "Modify joint N"), ⌄ Other joints grid (Name, Type ▾, Style ▾), lower half a shaded 3D flat viewport with its own view cube (behaviours "qa5 sm" (8), part-studio/541) | "Sheet metal" tab, part name, white line drawing, summary rows, Dir/Angle/Radius/Allow table, Export DXF / Export SVG buttons (tooltip + strip position fixed by qa5 10-23) |

## skb (sketch / assembly behaviour) — 1 row

| ID | Row | Repro | Live | Ours |
|---|---|---|---|---|
| POLY1 | Polygon construction circle while the side count is pending | Inscribed polygon: click centre, click a vertex; look before committing | dash-dot construction circle (sketch/91) | solid circle (crates/scene/src/sketch_mode.rs pushes it to `dash_dot`, but it renders solid at that stage) |

## psb (Part Studio behaviour) — 1 row

| ID | Row | Repro | Live | Ours |
|---|---|---|---|---|
| BOOL1b (tooltip) | Boolean "no geometry change" row hover | Union of two disjoint parts (qa5 bool doc), ✓, hover the row | just "Boolean 1" | "Boolean 1 <- Extrude 1, Extrude 3" |
