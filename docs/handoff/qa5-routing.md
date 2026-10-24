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

# qa4 entries (appended; same rules: append only, STATUS lines for changes)

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

## POLY1 — skb — sketch inscribed polygon preview: circle solid, no side spinner
- Source: qa5 workflow-sketch-tools POLY1; re-checked by qa4 2026-10-23 on main a0e360a3 (d1a1d056 says fixed).
- Repro: sketch on Top, Inscribed polygon, click a centre, move out. Expected (live sketch/91): dash-dot construction circle,
  "Ø…" label, "◀6▶" spinner rotated along the edge next to the vertex. Actual: solid circle, no spinner.
- Evidence: /root/qa4-scratch/gate-v047/poly1-main-a0e360a3.png.

## D10 — api — drawing PDF export: isometric view comes out shaded, Detail view circle/clip lost
- Source: qa5 workflow-drawing §4.2 (last seen pass 3, not re-run since). On screen hidden-line; in the PDF shaded.

## D12 — api — drawing DXF export explodes geometry
- Source: qa5 workflow-drawing §4.4: circles/arcs, hatches, dimensions and tables become lines/polylines; layer names differ
  from Onshape's. Not re-run since pass 3.

## DSM2 — fs — flat-pattern bend notes say DOWN where live says UP
- Source: qa5 workflow-drawing-sheet-metal (SM5). "DOWN 90.0° R2" vs live "UP 90.0° …". `sheetmetal/flat.rs:300` prints
  `FlatBendLine.up`; `model/tests/sheetmetal_frames.rs:271` asserts a flange is not up. asb: fixed by fs's std-route switch.

STATUS BOOL1b (qa4, 2026-10-23): qa4's ✅ in workflow-booleans.md was for an overlapping union; qa5's disjoint-union tooltip
"Boolean 1 <- Extrude 1, Extrude 3" (psb row above) stays open.
STATUS AN4 (qa4, 2026-10-23): two extruded boxes overlapping 10 mm give "10 mm x 10 mm x 10 mm"; qa5's repro (block + shifted
copy) is still loose per qa5's kern row above, so AN4 stays open (kern).
STATUS POLY1 (qa4, 2026-10-23): same as qa5's skb row: the solid circle remains; the spinner appears once a vertex is clicked
(qa4 looked before clicking one).
STATUS EXP1 (qa4 entry, 2026-10-23): renamed **EXP-HDR** (api, export header fields) to avoid qa5's EXP1 (Rhino export, kern).
