# Workflow: analysis (measure, mass properties, section view, curvature, draft, interference)

Document "qa5 advanced", Part Studio 3 (drafted/filleted block with counterbores, its mirror Part 2, and a copy
Part 3 shifted 10 mm in X). Live reference: behaviours.md "Measure & mass properties", "Analysis tools", "Interference
detection", "psb f (10-21)" (section view / mass without material), dom/dialog-mass-properties.txt.
Status: ✅ matches live/expected, ✗ differs (finding #), ? live data missing.

**Last run: 2026-10-21 (session 13:37–14:29), qa5**, binary /tmp/qa5/noshape-1337.

| # | Step | Live | Ours | Status | Verified |
|---|---|---|---|---|---|
| 1 | Click a circular edge | readout "Diameter: …" | "Diameter: 12.878 mm" | ✅ | 0.4.10+ / 8602df0d 10-27 (bracket24): Diameter 6.000 mm |
| 2 | + Top plane (row) | a two-entity readout (Min dist …) | readout **empty** | ✗ AN1 |  |
| 3 | `[` with a vertex (Midpoint of edge) + Top plane | Measure panel with result rows | panel lists both chips, **no result rows** | ✗ AN1 |  |
| 4 | Two adjacent drafted faces | Angle, Area, Max dist (X/Y/Z), Min dist (X/Y/Z); (x) only on distance rows | same | ✅ | 0.4.10+ / 8602df0d 10-27 (bracket24): wall front + gusset face: Perp planes angle 90, Area, Max / Min dist with X/Y/Z, (x) on distance rows |
| 5 | Measure panel units | Length unit ▾, Angle unit ▾ (live inch account: Inch) | Millimeter / Degree in a mm document | ? |  |
| 6 | Mass properties without material | red part chip, blank mass/CoM/inertia, volume + area filled | same (core F18/F19 fixed) | ✅ | 0.4.10+ / 8602df0d 10-27 (bracket24): red part chip, mass / CoM / inertia blank, volume + area filled, toast |
| 7 | Assign material… dialog | Library / Custom, Onshape Material Library ▾ +, "- None -" ▾, "No material assigned"; bottom-left | same | ✅ | 0.4.10+ / 8602df0d 10-27 (bracket24) |
| 8 | Search "Steel", pick 300 Series Stainless | applied | applied | ✅ | 0.4.10+ / 8602df0d 10-27 (bracket24) |
| 9 | Mass properties | mass, CoM, inertia | 0.0518 kg (= 6603.844 mm³ × 7.85 g/cm³), CoM (20, 9.531, 4.649) mm, inertia "(mm² kg)" | ✅ (unit label: live shows "lb mm²" on the live account, depends on mass units ?) | 0.4.10+ / 8602df0d 10-27 (bracket24): 0.527 kg = 67191.328 × 7.85, CoM X 40 |
| 10 | Shift+X (no dialog open) | Section view panel: Exclude / Include, Section planes, Items to exclude; prompt "Select a plane, planar face, cylinder, **cone,** or mate connector." | same panel; prompt **without "cone,"** | ✗ AN2 |  |
| 11 | Plane 1 row into Section planes | accepted | not accepted (ADV1) | ✗ ADV1 |  |
| 12 | Front row | "Section plane 1 (Front)" + offset box "0 mm" with arrow | same | ✅ / ? | 0.4.10+ / 8602df0d 10-27 (bracket24): "Section plane 1 (Front)" + 0 mm box on the arrow |
| 13 | Type an offset into the box, Enter / Tab | section moves | **value not taken (stays 0 mm)**; Enter closed the dialog; Tab added a stale earlier selection (Plane 1) as "Section plane 2" | ✗ AN3 |  |
| 14 | Shift+X a second time (section committed) | ? | nothing opened | ? |  |
| 15 | Camera menu while sectioned | … ends with "Section view…" | "Turn off section view" | ? |  |
| 16 | Analysis tools menu | Curve/surface analysis…, Deviation…, Connection…, Dihedral…, Interference detection…, Zebra stripes…, Reflection analysis…, Curvature color map, Draft analysis…, Flatten surfaces… (free plan: no Thickness) | same + Thickness analysis… | ✅ (paid plans have it) | 0.4.10+ / 8602df0d 10-27 (bracket24) |
| 17 | Draft analysis: Top row as mold split direction | ? | **not accepted**; the top face works → legend Steep / Undercut / Side 1 / Side 2, colours, hover angle "90.00°" | ✗ ADV1 / ? |  |
| 18 | Curve/surface analysis on a fillet face | combs | Entities to analyze, ☑ Curvature combs, U/V curves 8, ☑ Sketch inflection points, ☐ Minimum radius, ☐ Extend across face, ☐ Control point grids, Scale; magenta combs | ? |  |
| 19 | Interference detection, Part 1 + Part 3 | "Part 1 and Part 3", parts translucent with amber outlines, red volume; dialog snapped right (W−600, H−401) | same, same position | ✅ | 0.4.10+ / 8602df0d 10-27 (bracket + Transform copy +10 X: Part 1 and Part 2, amber outlines, red overlap) |
| 20 | Hover the row | yellow box + "L x W x H" of the overlap | yellow box; tooltip **"35.485 mm x 20.233 mm x 10 mm"** for a 30 × 20 × 10 overlap | ✗ AN4 | 0.4.10+ / 8602df0d 10-27: tooltip "70 mm x 50 mm x 60 mm" = the exact overlap box (tickets/interference-tooltip-8602.png); qa5's block + shifted copy not re-run |

## Findings (2026-10-21)

| # | Finding | Repro | Owner |
|---|---|---|---|
| AN4 | Interference size tooltip uses a loose bounding box (35.485 × 20.233 × 10 for a 30 × 20 × 10 overlap) | block + copy shifted 10 mm, Interference detection, hover row | kern / psb |
| AN1 | Measure gives no result for a vertex/edge + plane pair (status-bar readout and Measure panel both empty) | select an edge midpoint (or circle) + Top plane | psb |
| AN3 | Section view offset box ignores typed values; a stale prior selection is added as an extra section plane | Shift+X, Front, type 10 in the box | psb |
| AN2 | Section view prompt text lacks "cone," | Shift+X | psb |

## Re-run pass 3 (2026-10-21 14:40–15:00, /tmp/qa5/noshape-1433 = target-shared 14:32)

| # | Row | Ours | Status | Verified |
|---|---|---|---|---|
| 2 | edge + Top row | readout still empty | ✗ AN1 |  |
| 10 | Shift+X prompt | still "Select a plane, planar face, cylinder, or mate connector." | ✗ AN2 |  |
| 13 | Click the "0 mm" offset box | **the click passes through to the plane under it: Right is deselected, the section disappears and the box goes away**; Enter (after typing) closed the dialog and no section stayed | ✗ AN3 (worse) |  |

## Re-run on integration 5d28447 (2026-10-21 17:05–17:20, /tmp/qa5/noshape-1659 = /root/target-lead/debug/noshape 16:59; no CLI built from 5d28447 yet, so CLI-only geometry rows stay on the 14:36 CLI result)
| AN1 face/edge + Top readout | empty | ✗ still |
| AN2 Shift+X prompt | no "cone," | ✗ still |
| AN3 offset box | click passes to Plane 1, typed value ignored, Tab adds "Section plane 2 (Plane 1)" | ✗ still |

## Re-run pass 7 (2026-10-21 18:56–19:50, app /tmp/qa5/noshape-1851 = target-shared 18:51 (tree d5745c9 + uncommitted work), CLI /tmp/qa5/noshape-cli-1809 = target-engine 18:09)
| Row | Ours | Status | Verified |
|---|---|---|---|
| AN1 face + Top, edge + Top | "Parallel dist: 10.000 mm" for both | ✅ **fixed** | 0.4.10+ / 8602df0d 10-27 (bracket24): hole edge + Top: Min dist 42.000 |
| AN2 Shift+X prompt | "Select a plane, planar face, cylinder, cone, or mate connector." | ✅ **fixed** | 0.4.10+ / 8602df0d 10-27 (bracket24) |
| AN3 offset box | click takes focus, typed 5 + Enter moves the section 5 mm, dialog stays open | ✅ **fixed** | 0.4.10+ / 8602df0d 10-27 (bracket24): typed 4 + Enter moves it, dialog stays |

## Re-check on main a0e360a3 (qa4, 2026-10-23 19:30–20:40, FRESH=1, /root/qa4-scratch/bin/noshape-main)
| Finding | Result |
|---|---|
| AN2 Section view prompt | ✅ "…cylinder, cone, or mate connector." (section.rs) |
| AN4 interference tooltip | "10 mm x 10 mm x 10 mm" for two extruded boxes overlapping 10 mm ✅; qa5's repro (block + shifted copy) still loose → stays open (kern, qa5 routing) |
| AN1, AN3, ADV1 | fixes cited in code; not re-run this pass |

## Measure with default planes (live behaviours "ui (10-23 a): Measure readouts with default planes"; qa 10-28)

Block 40×20×10 at the origin (qa-scratch/blk40 copy), clicks additive (rows for planes, viewport for the edge / face).

| # | Selection | Live | Ours | Status | Verified |
|---|---|---|---|---|---|
| MP1 | Top + Front | status bar "Perp planes angle: 90.000 deg" (only row) | same | ✅ | 8bb8a6d1 10-28 |
| MP2 | Top + Right | "Perp planes angle: 90.000 deg" | same | ✅ | 8bb8a6d1 10-28 |
| MP3 | Top + vertical edge (10 mm) | "Perp line and plane angle: 90.000 deg" | same | ✅ | 8bb8a6d1 10-28 |
| MP4 | Top + the part's top face | status "Parallel dist: 10.000 mm"; panel Parallel dist 10 (X 0, Y 0, Z 10) · Area 800 mm² · Min dist 10 (X 0, Y 0, Z 10), no Max dist | same rows and values (tickets/measure-top-face-8bb8.png) | ✅ | 8bb8a6d1 10-28 |
| MP5 | Top alone | nothing in the status bar; panel "No measurements available for current selection." | same (blue info box) | ✅ | 8bb8a6d1 10-28 |
| MP6 | Two parallel through-hole walls (wf-block holes 25 mm apart, Ø6.35) | live label rule (behaviours ~401): "Parallel axes dist: … Min dist: … Max dist: …" in one line | "Parallel axes dist: 25.000 mm  Min dist: 18.650 mm  Max dist: 32.906 mm" (max = √(31.35² + 10²)) | ✅ | 8bb8a6d1 10-28 |
