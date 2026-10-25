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
- Closed (api, 10-24): live lists "Part 2, Part 1" too, in the file's order (behaviours.md "api (10-24) IMP2 parts order"). No change.

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

## VC8 — ui — Assembly camera menu
- Source: qa5, pass 21 re-run 2026-10-23 (main 88fbf8f / 40e23871).
- Repro: Assembly tab, view cube ▾
- Expected (live): Isometric, Dimetric, Trimetric | ⚙ Graphics preferences… | Named views…, Previous view | Zoom to fit, Zoom to window | Perspective view | ✓ render mode ▸, ✓ hidden edges ▸, ✓ tangent edges ▸ | View in high quality | Section view… (= the Part Studio menu minus "Orient normal to sketch on edit" and "Highlight boundary edges"; behaviours.md "qa5 assembly camera menu")
- Actual: qa5 removed "Show mates" and added "Section view…"; the assembly view has no render modes, named / previous views, graphics preferences, zoom to window or high quality yet

STATUS IM1 (2026-10-24, qa5): fixed by api (names, "<file> parts", RootAssembly) except the "BOM : RootAssembly" tab → IM1b, api.
STATUS DOC1 / I5 (2026-10-24, qa5): fixed by api in live's layout with noshape's own product strings; only the Parasolid schema minor version differs (SCH_3801000 vs live 3801231).
STATUS RHINO1 (2026-10-24, qa5): still "needs openNURBS" on main c59c5af9.
STATUS MEAS1 (2026-10-24, qa5): answered by live "ui (10-23 a)"; fixed on ready/ui 6f16c117 (not on main yet).
STATUS MEAS1 (2026-10-24 07:20, qa5): fixed on main 78109ebc (Top alone: empty status bar, as live).

## DRW-SIL — api — drawing_get omits silhouette edges; callouts can't target curved parts by script
- Source: qa4 QA round 4 session 2 (2026-10-24, v0.4.7). Assembly drawing iso view of a plate + Ø10 pin: `drawing_get
  edges:true` lists the plate and block lines but nothing for the pin (its silhouettes are drawn on the sheet).
  `drawing_callout` anchored on the pin's centre line attaches to the plate (balloon "1"); in the app a click on the pin's
  silhouette gives "2". Expected: silhouettes listed like other edges (and the anchor resolved to the part under it).
  Evidence /root/qa4-scratch/r4/s2-drawing-bom-balloons.png, s2-balloon-pin-app.png; model by r4/s2.py.

## CFG-MASS — api — CLI: mass properties ignore the active configuration; Derived accepts a tab name and fails
- Source: qa4 QA round 4 session 4 (2026-10-24, v0.4.7), model by /root/qa4-scratch/r4/s4.py.
- (1) After `config_activate {"Size": "L"}` (`active` reports L), `query_mass_properties` still returns 12000 mm³ (the S
  block); M and L should give 27000 / 48000. The app rebuilds L correctly.
- (2) `feature_add importDerived {"partStudio": "Part Studio 1"}` is accepted (a tab name, not an id) and the feature errors
  "Error regenerating."; the app then shows "Missing Part Studio". Expected: resolve tab names (like `tab` arguments do) or
  reject with `invalid_argument`.

## FLAT-ROT — fs — flat pattern is turned 90° against Onshape's
- qa4 round 4 S1 (r4/s1.py): live flat bounding box x 128.9165 × y 129.4650 mm; ours 129.465 × 128.916 (panel "Flat size" and the
  drawing's flat view both turned). Live keeps the base in place: the x = 100 flange extends along +x, the y walls along ±y.

## FLAT-SEG — fs — bend lines split by a cutout
- qa4 round 4 S1: a Ø12 cut through both long bends splits each bend line in two in live (6 "Bend line" bodies, 5 × "UP 90.0°
  R1.5" + "UP 180.0° R0.01" notes in the drawing's flat view); ours keeps one bend line per bend (3 + 1 notes).

## FLAT-SCALE — asb — flat pattern view auto scale
- qa4 round 4 S1: Insert view › Flat pattern of Part 1 on ANSI_A_MM: live auto scale 1:3 (title block SCALE 1:3); ours 1:1.

## LOFT-SEAM / FILL-PLANE — kern — loft keeps the circle's seam; planar fill isn't a plane
- qa4 round 4 S3 (r4/s3.py): surface loft Ø20 circle → 30×16 rectangle: live 4 faces / 12 edges / 6060.1344235 mm² (the 4 matched
  points split the circle, its own seam is not kept); ours 5 faces, 6058.8169 mm². Fill of the circle: live an exact PLANE,
  314.1592653589793 mm²; ours 314.212. Downstream: Enclose live 31743.5253 / 6 faces vs ours 31807.0934 / 7.

## STEP-ASM — api — importing a STEP assembly
- qa4 round 4 S5 (r4/s5.py, samples/qa4-s5-asm.step): live makes an **Assembly tab "Assembly 1 (1)"** (root product name, " (1)" on a
  clash) + Part Studio **"qa4-s5-asm parts"** + the blob in a **CAD Imports** folder + a BOM element; parts listed Part 2, Part 3,
  Part 1, each at its **own product position** (the assembly places the instances). Ours: one Part Studio "qa4-s5-asm", parts
  Part 1, Part 3, Part 2 at the assembly's positions, no assembly. (PVZ: IM1, kern row, is the same gap.)
- Export details (minor): STL binary header "noshape-kernel st…" (live 80 zero bytes); STEP product order; Parasolid header = EXP-HDR.

## IMP-DENS — api — "Import material density" offered for STEP files (main, since 5d5e0403)
- Source: qa4 QA round 4 session 5 in the app (2026-10-24). `crates/app/src/import_dialog.rs` `density` includes "step"/"stp".
- Expected (live): behaviours "qa4 round 4 s5": "For STEP there is **no 'Import material density'** checkbox (an .x_t import
  shows it)" (part-studio/2394); "api (10-23) import parity" only showed it for .x_t (2336). v0.4.7 matched live (4 checkboxes).
- Actual (main): a STEP import shows ☐ Import material density between Import appearances and Orient … Y Axis Up.
STATUS STEP-ASM minor (qa4, 2026-10-24): the STL binary header is fixed by qa4 (80 zero bytes, `kernel/src/io.rs`, test
`binary_stl_header_is_zeros`); "import stays on the open tab" was fixed by api on main (5d5e0403).

## qa4 QA round 5 (2026-10-24; docs/handoff/qa-round-5.md, live "qa4 round 5 s1…s5"; v0.4.7 and v0.4.8 alike unless noted)
- **DRW-CFG** (asb) Create Drawing ignores the Part Studio's active configuration (Size M active → views 60 long = S); live:
  "The drawing takes the Part Studio's current configuration". And the view menu has **Configuration ▸ Size ▸** where live has
  **"Change configuration..."** (between Align views and Switch to Part Studio 1) opening a panel (document/branch, tree with
  Size [M ▾], part thumbnail, ✓ ✕) that changes every view of that part; View properties shows it read-only.
- **DRW-DANGLE-DOT** (asb) a dangling dimension keeps its black arrowhead; live draws a **filled orange-red dot** at the
  attachment point (drawing/qa4-r5-s1-c-dim-2x). The colour itself is fixed by qa4 (#DD5228). Fills carry no pick id, so the
  arrow → dot swap belongs in the sheet builder.
- **DRW-DIMMENU** (asb) diameter-dimension right-click: ours Foreshorten · Show as radius · Edit… · Detach from geometry · Paste
  as table | Clear selection · Zoom to fit · Delete (drawing_tools.rs ~1689, drawing_more.rs Foreshorten); live "Edit... | Paste
  | Paste as table | Unbreak dimension | Add comment | Clear selection | Zoom to fit | Delete" for healthy and dangling alike.
- **DRW-CMARK** (asb) Create Drawing › Suggested views: no automatic centre mark on the hole in the Top view (ANSI_A_MM); live has
  one (ISO_A3 in r5 s1; ANSI too, behaviours ~1537). A CLI-inserted view gets one.
- **DRW-DUAL** (asb, v0.4.8 gate (f)) the linear dimension line runs unbroken through the dual text; live breaks it around the
  text block (asbdd-20).
- **PS-NOTCOMPUTED** (psb) the menu of a row that is still regenerating is the full menu; live reduced "Rename | Add selection to
  folder… | Suppress | Dynamic suppression ▸ | Add comment | Delete"; with a feature dialog open the rolled-back rows have no Delete
  ("Rename | Suppress | Dynamic suppression ▸ | Add comment"). (Delete-during-regen speed = v0.4.8 gate (a).)
- **I18N-DIALOGS** (ui2) German: toolbar tooltip titles ("Extrude (shift+e)", ui.rs:604 formats before the lookup); Extrude
  (Thin, Intersect, Faces und sketch regions…, Direction, Starting offset, Entwurf → Formschräge, Second end position); Fillet
  (Verrunden 1 → Verrundung 1, Full round, Entities auf fillet, Tangent propagation, Measurement, Control → Strg, Asymmetric,
  Partial fillet, Variable fillet, Allow edge overflow, Smooth fillet corners); Mass properties (Parts auf measure, Mate
  connector for reference frame, „calculation variance“ anzeigen); part menu "Drawing of Part 1… erstellen" (live „Zeichnung
  erstellen von „Part 1" …"); Create Drawing title "Drawing: erstellen" (live "Zeichnung erstellen:"), "Owned by me" (Gehört
  mir); Preferences units sentence, heading "Voreinstellungen" (live "Preferences"), "Dezimalstellen" (live "Dezimalstellen
  anzeigen"). Live's strings: behaviours "qa4 round 5 s5" and ~11470.
- **DARK-DRW** (ui2) dark mode, drawing tab: our toolbar row is dark with a white block at its right end; live's capture
  drawing/qa4-r5-s5-12 shows the drawing toolbar light (white, dark icons; live's text says dark: ask live if unsure). Dark
  Create Drawing: the selected "No views" tile's label is unreadable.
- **CTX-USE** (skb) in a Part Studio's assembly context, **Use (u) on the context face the sketch lies on projects nothing** (no
  hover) and Use on its in-plane edges says "That geometry can't be used in this sketch"; an out-of-plane context edge projects.
  Live: Use on that face projects its 4 outer edges. On an ordinary face (CLI sketch_use) the same works.

## CI-TIMING (lead, WORKFLOW 17; qa4 2026-10-24) — wall-clock assertions in ci.yml tests (`cargo test --workspace --exclude noshape`)
These assert on elapsed time, so they can go red on a loaded runner; per WORKFLOW 17 the timing part belongs in perf.yml, the
functional part (volumes, cancelled = true, error message) can stay:
- `crates/model/tests/pattern_second.rs:127` `c < 3.0 * a + 300.0` (LP2 vs LP1 ms; comment: "CI release ×400: 1.9-2.7× (runner
  noise)", already loosened once in c5cf7366) — kern/perf.
- `crates/model/tests/pattern_second.rs:163` abort latency `< 200 ms` release / `600 ms` debug — kern/perf (the gate (a) property;
  keep `out.cancelled` as the deterministic check).
- `crates/model/tests/qa_regressions.rs:114` `t0.elapsed().as_secs() < 30` (1 000 000-instance pattern rejected) — psb; the
  error-message assert above it is the deterministic part.
- `crates/store/src/tests.rs:963` 1 MB commit median `< 250 ms` — owner of store (2b3f0530 area).
Not timing (no action): custom_features.rs:660 only checks the Timer line's format.

## qa4 QA round 5 v2 (P0/P1; live "qa4 round 5 v2 s1 follow-up" and "qa4 round 5 v2 s1 / s2"; v0.4.8 c7d9d454; r5/v2fu.py)
- **FUSE-UNIFY (kern, P1 topology)** Extrude (New) of several touching regions of one sketch keeps the region boundaries as
  separate coplanar faces: live's case (L + Ø10 circle + outer loop offset 3 mm in, 3 regions, Extrude 15 all regions) gives
  **12 faces** on ours vs live **8 faces** (one 2500 mm² top face, no inner-loop edges), same volume 37500. Cause:
  `regen/eval.rs` `fuse_named` (~770) fuses the NEW tools with a plain boolean and never runs `unify_same_domain` (extrude.rs's
  two-sided path does). Downstream faces/edges (fillet picks, face counts, drawings) differ from Onshape.
- **HOLE-MSG (psb)** a Simple Ø6 through hole tangent to an outer face fails on ours with "Hole cut failed: Boolean operation
  would result in non-manifold body."; live: "Hole 1 did not regenerate properly: Hole creates invalid geometry."
- **PAT-REAPPLY-FAIL (psb)** Feature pattern (Reapply ON) of that failing hole, 4 mm × 4 along the x=0 top edge: ours **ERROR
  "Could not pattern selected features."**; live **OK, no message**, the instances that fail individually are silently skipped and
  the one that succeeds cuts (37453.53776879832 mm³, 10 faces: a notch at (3,−2)). (On another part ours did report OK and cut;
  the rule should be live's: skip failing instances, report OK.)
- **ROLLBACK-AUTOSCROLL (psb)** dragging the rollback bar and holding it near the feature list's edge: live autoscrolls the list;
  ours doesn't (the bar can't reach rows below the visible ones in one drag).
- **REGEN-SPEED (perf/kern, P0 info)** per-feature regen vs live (Onshape's server times from its ⏱ panel, similar features):
  s2 bosses (Extrude Add Ø8–Ø12 on a plate) ours 17–71 ms each vs live ≤ 9 ms; s1 hole 129 vs 19–21 ms, linear pattern of it
  152 vs 47–96 ms, 30-feature totals ours 462 ms (14 plain bosses) vs live 171 ms (30 mixed features incl. fillets/chamfers).
  Editing feature 2 and ✓: dialog closes ~0.1–0.2 s in both.

## qa4 QA round 5 v3 s1 in the app (P1; live "qa4 round 5 s1 offset"; v0.4.8 c7d9d454) — details in qa-round-5.md "v3 Session 1"
- **OFF-CONSTR (skb, P1 wrong geometry)** sketch Offset makes Parallel (+ Tangent) constraints only; live makes OFFSET +
  DISTANCE (the "3" dimension). Dimensioning the source afterwards drifts the copy (3 → 2.61–2.82 mm) and changes the part.
- **OFF-BOX (skb)** Enter should create the offset and open the value box ("5 mm" selected, ±, ✓/✕), then Enter applies the
  typed value; ours commits without a box, the label shows only at ≥ 30 px and its box lacks ±/✓/✕.
- **OFF-SIDE (skb)** first side: a line away from the region, a circle outward (ours: left normal / closed inward).
- **OFF-TOAST (skb)** an unconnected second entity: select only + toast "Can only offset a single chain at once." (ours offsets both).
- **HOLE-FACE (psb, pending live follow-up 2)** Hole locations accept a planar face (PickKind::Face matches PLANE_LIKE ⊃ MATE_CONNECTOR).
- **EDIT-TYPING-PREVIEW (psb/ui2)** dialog number fields re-run the preview on each keystroke; live only on commit (Tab/Enter).
  `feature_dialog.rs` on_text_changed → DialogEvent::Changed per InputEvent::Change (v3 s2).
- **ROLL-LAST (psb)** "Roll to here" missing on the last feature while the bar is above it (`part_studio/mod.rs` ~3927 `!is_last`);
  live rolls forward that way (v3 s2).
- **REGEN-SPEED (perf/kern) update** live's exact 30-feature part: ours 585 ms at load / 472 ms after an edit vs live 171 / 132 ms;
  Boss 1 152 ms on load (others 11–18 ms).
- **MATE-FLIP-AXIAL (asb, P2 wrong placement)** Flip primary axis does nothing for Revolute / Cylindrical / Planar / Pin slot /
  Parallel: residual `[z.x, z.y]` (`assembly/solver.rs` ~571–579) accepts anti-parallel Z; result = nearest to the prior pose.
  Repro r5/v3s3-flip.py (identical poses for flip false/true; Fastened differs as it should) (v3 s3).
- **DRW-DIM-2EDGES (asb, P2)** drawing smart Dimension: a second click on a parallel edge in the same view places the first
  edge's length there instead of picking the edge (3/3; Top view left+right edge → "54" on the right edge). smart.rs
  dimension_takes_more / smart_takes (v3 s4).
- **OFF-CORNER (skb)** sketch Offset of a connected chain: live meets the offset lines at **sharp** corners (extended to their
  intersection, outward and inward); ours puts arcs where copies separate (`sketch/src/tools.rs` offset_chain "Outer corners get
  arcs") (live "qa4 round 5 s1 follow-up 2" (3)). OFF-SIDE: live forgets the flip between offsets (ours keeps `offset_flip`).
- **HOLE-FACE confirmed (psb)** live: no hover, nothing added for a face or a body vertex in Hole locations (filter: sketch
  vertices || explicit mate connectors). Ours takes both via PLANE_LIKE / POINT_LIKE ⊃ MATE_CONNECTOR in kind_matches; check the
  other MATE_CONNECTOR fields before narrowing (follow-up 2 (1)).
- **ROLL-LAST widened (psb)** live rolled-back row menu: Rename | Add selection to folder… | Suppress | Dynamic suppression ▸ |
  Roll to here | Roll to end | Delete (Add comment is cloud); the row above the bar adds "Roll to end" (no Roll to here); ours
  keeps Edit… / Zoom to selection / Show dependencies… on rolled-back rows and has no "Roll to end" (s2 rollback follow-up).
- **ROLLBACK-DROP-TOP (psb)** dropping the dragged bar over Default geometry rows/header: live clamps to the top slot; ours
  leaves the bar where it was.

## qa4 QA round 5 v3 s5: sketch constraints + dimension editing (P1; v0.4.8 c7d9d454) — qa-round-5.md "v3 Session 5"
- **DIM-EXPR-UNIT (skb)** bare expression `2*5` should reopen as `(2*5) mm` (ours `2*5`).
- **DIM-AUTOCOMPLETE (skb)** sketch dimension box: no unit/function/variable dropdown (live: "in", "isInteger(x)"; "#var 10 mm",
  "New variable…"); VarCompletion is feature-dialog only.
- **DIM-ESC-UNTOUCHED (skb)** Esc on an unchanged box should close it at once; ours needs two Escs.
- **DIM-UNSOLVED-GEOM (skb)** unsolvable sketch: keep the last solved shape (ours draws a half-way solve, dimension at old
  points); the edited dimension should turn red too.
- **DIM-MENU (skb)** right-click on a sketch dimension should be the standard sketch context menu with "Change to driven
  dimension" on top and "Delete dimension" last; ours is a 2-item custom list with centred labels (sketch.rs render_dim_menu).
- **VAR-KEYWORD (psb/api)** `#var` is a valid live variable name; ours rejects it via FS_KEYWORDS (variable_table.rs is_identifier).

## qa4 QA round 5 v3 s6: every sketch constraint, driven dims, glyph menus, dragging (P1) — qa-round-5.md "v3 Session 6"
- **CON-NORMAL-ORDER (skb)** Normal with the circle picked first fails ("e50 is not a line"): make_constraint (scene/sketch_mode.rs
  ~5946) should order line/curve by kind like Midpoint.
- **CON-MSG (skb)** constraint errors expose internal ids in the status bar and linger after later actions.
- **DRIVEN-LOOK (skb)** driven dims: grey plain "Ø10" (no parentheses); a redundant (auto-driven) dimension still opens the value box.
- **GLYPH-MENU (skb)** glyph right-click = reduced sketch menu ending "Delete sketch entity" (ours: one centred "Delete").
- **MATE-FLIP-AXIAL confirmed by live (asb)** flip swaps the pose for Revolute/Cylindrical/Planar; the default follows the
  connectors' axes, not the instance's pose (live "qa4 round 5 s3 mate flip").
- Pending live (follow-up "qa4 round 5 s6 constraints"): H+V on one line (ours collapses it), Tangent/drag changing the radius,
  line menu items.
- **CON-CURVATURE (skb)** Curvature refuses Spline-tool (interpolated) splines: equations.rs curv_spec has no case for them.

## qa4 QA round 5 v3 s7: feature dialogs in everyday use (P1) — qa-round-5.md "v3 Session 7" (sketch items now go to psb)
- **EXT-DIRECTION (psb/ui2)** ticking Direction: the new field should become active, the title red and the group chevron
  red-brown until picked (live dialogs/extrude-23); ours: white field, black title, grey chevron.
- **EXT-2ND-DEPTH (psb/ui2)** Second end position Depth: no › expander and no ± icon on live.
- **SEL-REJECT-TOAST (psb)** no toast for a rejected pick (ours: "Select sketch regions or planar faces." from filter_description).
- **CHIP-HOVER (psb/ui2)** chip hover should highlight the entity yellow-green #f2ff80 (ours keeps amber).
- **FILLET-OVERFLOW (kern, pending live)** R > face height (21…45 on a 20 mm block edge) reports OK as a clipped quarter
  cylinder; r5/v3s7-fillet.py prints status/volume per radius.
- **Live s6 answers → psb**: **CON-HV-COLLAPSE** (V on an H line must make the sketch unsolvable: toast + red, line unchanged;
  ours collapses it to zero length silently), **CON-TANGENT-APPLY** (applying Tangent line–circle moves the circle, radius kept;
  ours grows the radius), CON-NORMAL-ORDER confirmed, **SK-LINE-MENU** (line menu: + Construction, Extrude…, Revolve…, Align view
  with vertical axis; − Section view…; + Hide other/all sketches only when other sketches exist).
- **FILLET-OVERFLOW retracted** (live reports the same OK results and volumes). **CHAMFER-OVERFLOW (kern)** new: Chamfer 25 on a
  20 mm face edge is OK in live (30000.000 mm³, the leg overflows like a fillet); ours "Failed to chamfer selections.".
  EXT-DIRECTION confirmed with colours (qa-round-5.md s7 live answers).

## qa4 QA round 5 v3 s8: the feature list (P1; published v0.4.8) — qa-round-5.md "v3 Session 8"
- **ERR-MISSING-REGION (kern/psb)** dependent of a deleted sketch: "A selected sketch region no longer exists" (eval.rs ~636) is
  not an Onshape string; use the empty-input "Select face or sketch region to extrude." style (exact text pending live s8 (2)).
- **MULTI-MENU (psb)** multi-row menu: add "Edit sketch appearance…" when a sketch is selected.
- Pending live s8: invalid reorder handling (ours: red toast "Cannot reorder: … depends on …"), folder expand as an undo step,
  geometry-hover → row highlight, folder menu.

## qa4 QA round 5 v3 s9: interaction (P1; v0.4.8) — qa-round-5.md "v3 Session 9"
- **BOX-PREHIGHLIGHT (psb)** crossing box: pre-highlight everything it touches while dragging (live).
- **SELECT-OTHER (psb/ui2)** popup: header ✕, no type column, no preselected row, hovered row bold, "?" footer (live sketch/231).
- Pending live s9: global selection filter, parts-row "Hide other/all parts", Measure panel default unit, Point readout axis
  icons, section manipulator, edge midpoint marker.
- **Live s8 answers → psb**: **REORDER-INVALID** (do the move, dependents go red with "Select face or sketch region to
  extrude." / "1 missing selection"; no refusal toast), ERR-MISSING-REGION confirmed (+ second line "1 missing selection"),
  **SUPPRESSED-MENU** (Rename | Add selection to folder… | Unsuppress | Dynamic suppression | Roll to here | Delete),
  **SKETCH-UNCONSUMED** (a sketch whose consumer is suppressed shows again), **TOOLTIP-PARENT** (no type tag on the parent),
  **FOLDER-NAME-INLINE** (Add selection to folder… names inline in the row), **FOLDER-MENU** (Rename | Add selection to folder… |
  Suppress/Unsuppress | Dynamic suppression | Roll to here | Expand | Collapse | Delete | Unpack folder), **FOLDER-EXPAND-UNDO**
  (expand/collapse is not an undo step).

## qa4 QA round 5 v3 s10: everyday assembly (P2; v0.4.8) — qa-round-5.md "v3 Session 10" (assemblies → skb)
- **DRAG-RUNAWAY (skb, P2 severe)** dragging one instance in a Planar-mated stack moves the other free instances: 10 cubes lean, 19
  cubes are flung ±2.5e6 mm in X/Y (mates still ok). Undragged bodies need a pull toward their current pose / minimal-norm steps.
  Repro r5/v3s10n.py (NCUBES=19). Drag rate 3.9 updates/s at 20 instances vs 10.0 at 7 (software GL; r5/dragfps.sh).
- **CHIP-X-SELECTED (skb)** pattern dialog seed chip × is undone while the instance stays selected (re-captured from selection).
- **Live s9 answers**: **READOUT-AXES (ui/psb)** Point readout needs X↘/Y↗/Z↑ coloured axis arrows; **SECTION-MANIP (psb)** section
  plane manipulator = ring + shaft + arrowhead + two rotate handles; **MIDPOINT-MARKER (psb/ui)** dark hollow ring on edge hover,
  orange hollow ring near the midpoint (edge un-highlighted), yellow-orange ring when selected. No global selection filter,
  parts-row Hide other/all parts and the Measure unit are live-like.

## qa4 QA round 5 v3 s11: everyday drawing (P2; v0.4.8) — qa-round-5.md "v3 Session 11"
- **ORD-BASE (asb, P2 wrong value)** ordinate base measured from the click point, not the highlighted edge (boss centres 14.26 /
  39.26 / 64.26 instead of 15 / 40 / 65; 15.39 when clicked outside).
- **EXPORT-DXF-OPTIONS (asb)** DXF/DWG export: add ☐ Hide inspection items and ☐ Exclude off-sheet content (live).
- Pending live s11: auto-update vs Update button, title block editing, iso angular value, live PDF/DXF for comparison.
- **DRAG-RUNAWAY confirmed by live (skb)**: Onshape moves only the dragged instance; all others keep their exact pose (live REST
  before/after for 10- and 19-cube Planar stacks); server ≈ 21 ms per drag step regardless of stack size.
## PCB-CONVERT — fs — Flex PCB model › Convert of a thin plate makes 6 parts (P3)
- api, 2026-10-24 (e2e `flex_pcb_model_features`): a 60 × 20 × 1 mm plate (sketch rectangle, Extrude 1 mm), then Flex PCB model with
  Process Convert, Parts and surfaces to convert = the plate, other fields default → 6 solid parts "Part 1" … "Part 6"
  (`body:<pcbStart>.2114097152` … `.2114097157`); Finish flex PCB model (Unfold none) then reports its Info note. Expected (live,
  to be checked): one flex PCB part. Priority P3 (lead). Repro: `cargo test -p noshape-ops --test e2e flex_pcb_model_features`
  and print `query_parts` after the pcbStart call.

## FLAT-HOLE — model (lead to assign) — Flatten surfaces of a planar face with a hole is wrong
- api, 2026-10-25: an 80 × 40 × 20 mm block with a Ø10 through hole; Flatten surfaces of its top (or bottom) face → 20 boundary
  loops, flat area 4396 mm² for 3121.5 mm² in 3D, strain up to 24, flat size 59 × 84 mm. The same block without the hole, and every
  hole-free face, flattens exactly (1 loop, areas equal, 40 × 80 mm). Expected: an identical flat face (no strain), 2 loops.
  `noshape_model::flatten::flatten_faces` (moved verbatim from the app's flatten_tool.rs; the app's Flatten surfaces is the same).
  Repro: `analysis_flatten {"faces": [<top face id>]}` on `block_with_hole` (ops e2e).
