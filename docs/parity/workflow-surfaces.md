# Workflow: surfaces & multi-body (a CAD user's session, step by step against live Onshape)

Document "qa5 surfaces" (mm). Setup sketches made with noshape-cli (/root/qa5-scratch/mksurf.py): "Line sketch"
(Top, 40 mm line), "Box sketch" (Top, 40×20 at y 40..60), "Arcs sketch" (Front, two 40 mm arcs 30 mm apart),
"Blocks sketch" (Top, two overlapping 40×20 rectangles). Everything else by mouse/keyboard. Expected values are
computed (exact geometry); live data for the less common features is requested ("qa5 import/surfaces").
Status: ✅ matches expected/live, ✗ differs (finding #), ? live data pending.

**Last run: 2026-10-21 (session 12:05–13:35), qa5**, binary /tmp/qa5/noshape-1206.

| # | Step | Expected / live | Ours | Status | Verified |
|---|---|---|---|---|---|
| 1 | Shift+E, Surface tab, click the line, 20 mm | surface 40×20 | "Line sketch line" chip, preview, Surface 1 in "Surfaces (1)" | ✅ | 0.4.10+ / dd1b5037 10-27 (40 mm line on Top, Surface 1 under Surfaces (1)) |
| 2 | Next Extrude opens on … | (live: last used tab?) | Surface tab again (sticky) | ✅ live behaviours 1497: "the Solid / Surface / Thin choice is sticky per user" (qa 10-27, dd1b5037) |  |
| 3 | Thicken (extrude ▾), click the surface | live dialog: [New\|Add\|Remove\|Intersect] · Faces and surfaces to thicken · ☐ Mid plane · Thickness 1 · Thickness 2 · ☐ Keep tools · ☐ Merge with all · Merge scope | same fields; opens on **Add** with Merge scope prefilled "Surface 1" | ? |  |
| 4 | Thickness 2, ✓ | 40×20×2 = 1600 mm³ solid | Part 1, 1600.0, 6 faces | ✅ | 0.4.10+ / dd1b5037 10-27 (1600 mm³ / 1840 mm², the surface consumed; = live behaviours 6047) |
| 5 | Solid extrude of a region after a picking miss | – | Extrude 2 red "Select sketch curves to extrude" (was still on the Surface tab) | ✅ (my miss) |  |
| 6 | Delete face › Type list | Heal / Cap / Leave open | same | ✅ | 0.4.10+ / dd1b5037 10-27 (Heal / Cap / Leave open) |
| 7 | Delete faces field, click near an edge | face-only | **"Edge of Extrude 2" accepted** (I2 again) | ✗ I2 |  |
| 8 | Leave open on the box top | open surface body | "Part 2" under Surfaces (5 faces, 3800 mm²) | ✅ (name ?) | 0.4.10+ / dd1b5037 10-27 (40×30×20 block's top: "Part 2" under Surfaces) |
| 9 | Tool search "fill" | Fillet, Fill, Finish flex PCB model, Finish sheet metal model, … | no Finish flex PCB model (SM12) | ✗ SM12 |  |
| 10 | Fill: 4 top edges | live dialog ? | chips "[Position] Edge of Extrude 2" + per-edge continuity rows; **no preview while editing**; result Surface 2, 800 mm² | ✗ SF1 (preview) |  |
| 11 | Boundary surface: arcs as U profiles | surface between the arcs | "Add Profile" UI, preview, OK; 1200 mm² (planar region between two coplanar arcs = 40×30) | ✅ | 0.4.10+ / da18f886 10-28 (U profiles rows "[None] Edge of Arcs sketch" ×2, preview, Surface 1 = 1200.000 mm²) |
| 12 | Click in the viewport before activating a field | (Boundary) | notice "Click a selection field in the dialog first." | ? |  |
| 13 | Extrude two adjacent regions (solid) 10 mm | one block, **6 faces** (live merges coplanar faces, G45) | 8000 mm³ but **10 faces** (region seams kept on top/bottom/sides) | ✅ (SF2 fixed) | v0.4.12 CLI 10-28 (bin/sfcheck.py: two adjacent 20×20 regions, 10 mm → 8000, 6 faces) |
| 14 | Extrude with both regions, misclick near the New/Add/Remove row | – | Error "Select parts in the merge scope" (most likely my click landed on Remove; not a finding) | – |  |
| 15 | Transform: type list | Translate by line, by distance, by XYZ, Transform by mate connectors, Rotate, Copy in place, Scale | same | ✅ | 0.4.10+ / dd1b5037 10-27 (Translate by line / by distance / by XYZ / Transform by mate connector / Rotate / Copy in place / Scale) |
| 16 | Transform XYZ x 20, ☑ Copy part | copy | Part 4 at x 80..120 | ✅ | 0.4.10+ / dd1b5037 10-27 (bracket: XYZ x 10 + Copy part → Part 2 overlapping) |
| 17 | Boolean Subtract: tools Part 4, targets Part 3 | 4000 mm³, 6 faces | 4000.0, 6 faces | ✅ |  |
| 18 | Split Part 3 by Plane 1 (Right + 70) | two parts | Part 3 (x 70..80) + **Part 5** (x 60..70), 2000 each | ? (names) |  |
| 19 | Mirror Part 5 about Right | Part 6 | Part 6 at x −70..−60 | ✅ |  |
| 20 | Part colours after Split/Mirror | rotation #9dcfed, #a5a5a5, #3b61b4, #eaeaea, #c4e2f3, #f88701, #7f7f7f, #fab601 | Part 3 and Part 5 orange, Part 6 gold (persist after tab switch) | ? SF4 |  |
| 21 | Move boundary on one straight edge of the planar boundary surface, 25 mm | surface extended | **Error with raw kernel text** "Geometry operation failed: operation failed: wire offset failed (offset too large or non-planar wire?)" | ✅ (SF5 fixed) | v0.4.12 CLI 10-28 (bin/sfcheck25.py: straight side of the arcs boundary surface, 25 mm → 1950 mm² = 1200 + 30×25, 1 face) |
| 22 | (CLI) Move boundary on one edge of the planar Fill surface, 10 mm | 800 + 200 mm² | Error "Move boundary currently supports single planar surfaces only (all boundary edges moved)" | ✅ (SF5 fixed) | v0.4.12 CLI 10-28 (bin/sfcheck.py on a 40×20 extruded-line surface: one edge 10 mm → 1000 mm², x 0..50, 1 face) |
| 23 | (CLI) Move boundary on the boundary surface's bspline edge, 10 mm | 1200 + 300 mm² | **Status OK, surface area −514.305 mm²** (corrupt body) | ✅ (SF6 fixed) | v0.4.12 CLI 10-28 (bin/sfcheck.py: boundary surface between two 3-point arcs 30 apart = 1200 mm²; its straight side 10 mm → 1500 mm², 1 face) |
| 24 | Feature-list row tooltip after picking a plane from the list | hides | lingers over the dialog (I4) | ✗ I4 |  |

## Findings (2026-10-21 run)

| # | Finding | Repro | Owner |
|---|---|---|---|
| SF6 | **Move boundary returns OK with a corrupt surface (area −514.305 mm²)** | boundary surface between two coplanar arcs; Move boundary on its straight (bspline) side edge, 10 mm | kern |
| SF5 | Move boundary can't extend a single edge ("supports single planar surfaces only (all boundary edges moved)"), and in the UI fails with raw kernel text "Geometry operation failed: operation failed: wire offset failed …" | §21–22 | kern / psb (error text) |
| SF2 | Extruding adjacent regions leaves the region seams as separate faces (10 instead of 6); live merges coplanar faces | §13 | kern |
| SF1 | Fill shows no preview while editing | §10 | psb |
| SF4 | Part colours after Split/Mirror don't follow the default rotation (verify live) | §20 | psb |

## Update from live's answer (behaviours.md "qa5 import/surfaces", 10-21)

| # | Row | Live | Verdict |
|---|---|---|---|
| 1–4 | Thicken of the extruded line surface | default direction **−Y** (bbox y −2…0), Surface 1 consumed, Part 1 New | ours: **y 0…2** (opposite side), opened on Add → **SF8** |
| 6 | Delete face top of box | default Heal fails "Could not delete selected faces."; enum HEAL / CAP / VOID | ✅ (ours Heal / Cap / Leave open) |
| 10 | Fill on the open box | rows "[Position] Edge …"; when the loop closes **☑ Merge with all** appears → **knits into one solid, 8000 mm³, 6 faces** | ours: no Merge with all; separate "Surface 2" → **SF7** |
| 11 | Boundary surface | V profiles shown pink/required; U profiles rows "[None] Edge of Sketch 1" | ? look |
| 18 | Split by a plane | Part 1 = +X side, new Part 2 = −X side; **Keep tools switches on by itself** after the plane pick | naming ✅ (original keeps +X); Keep tools auto-on ? |
| 21–23 | Move boundary single edge 10 mm | surface extended in place: 1200 → 1200+…, still 1 face | SF5, SF6 confirmed |
| 17 | Boolean subtract keep tools off | Part 1 only, 6000 | ✅ pattern |

## Re-run pass 3 (2026-10-21 15:10, CLI /tmp/qa5/noshape-cli-1436 = target-engine 14:36)

| # | Row | Ours | Status | Verified |
|---|---|---|---|---|
| 13 | Extrude two adjacent regions 10 mm | 8000, **10 faces** (live 6) | ✅ (SF2 fixed) | v0.4.12 CLI 10-28 (8000, 6 faces) |
| 21 | Move boundary on the planar boundary surface (qa5 surfaces doc) | still Error | ✗ SF5 |  |
| 9 | Tool search "fill" / Sheet metal ▾ | still no Finish flex PCB model | ✗ SM12 |  |

## Re-run on integration 5d28447 (2026-10-21 17:05–17:20, /tmp/qa5/noshape-1659 = /root/target-lead/debug/noshape 16:59; no CLI built from 5d28447 yet, so CLI-only geometry rows stay on the 14:36 CLI result)
| SF2 two adjacent regions | seam kept (top face split in two) | ✗ still |

## Re-run pass 7 (2026-10-21 18:56–19:50, app /tmp/qa5/noshape-1851 = target-shared 18:51 (tree d5745c9 + uncommitted work), CLI /tmp/qa5/noshape-cli-1809 = target-engine 18:09)
| Row | Ours | Status | Verified |
|---|---|---|---|
| SF2 adjacent regions | 8000, 10 faces (CLI); seam visible in the app | ✗ still |  |
| SF5 Move boundary one Fill edge 10 mm (CLI) | OK but **all four edges move**: 2400 mm², bbox −10..50 × 30..70 (expected 1000) | ✗ still (changed) |  |
| SF5 boundary-surface case, 25 mm | error now "Unable to move selected entities." (raw kernel text gone) | ✅ text |  |
| SF6 bspline edge 10 mm (CLI) | OK, area −514.304543 | ✗ still |  |

## Re-run pass 8 on main 5bd50b2 (2026-10-21 23:13–00:10, app /tmp/qa5/noshape-2256 = /root/target-lead/debug/noshape 22:56; no newer CLI than target-engine 18:09, so CLI-only rows stay on pass 7; geometry rows re-read in the app where possible)
| SF2 | seam still visible in the app | ✗ still |

## Re-run pass 12 (2026-10-22 ~01:35, CLI /tmp/qa5/noshape-cli-0107 = target-engine 01:07; main still 5bd50b2)
| Row | Ours | Status | Verified |
|---|---|---|---|
| SF2 adjacent regions (sf2.py) | 8000, 10 faces (live 6) | ✗ still |  |
| SF5 Move boundary one Fill edge 10 mm | OK, 2400 mm², bbox −10..50 × 30..70 (all four edges move; live 1000) | ✗ still |  |
| SF5 boundary-surface case 25 mm | "Unable to move selected entities." | ✅ text (as pass 7) |  |
| SF6 bspline edge 10 mm | OK, area −514.304543 | ✗ still |  |

## Re-run pass 13 (2026-10-22 ~05:20–06:40, main aa75fdf: app /tmp/qa5/noshape-0503 = /root/target-lead/debug/noshape 05:03, CLI /tmp/qa5/noshape-cli-0226 = target-engine 02:26)
| SF2 / SF5 / SF6 | 8000 & 10 faces / 2400 mm² (all edges move) / −514.304543 (surface names now positional "Surface 1/2") | ✗ still |

## Re-run pass 15 (2026-10-22, CLI /tmp/qa5/noshape-cli-0521 = target-engine 05:21; main still aa75fdf)
CLI rows unchanged: SF2 8000/10 faces, SF5 2400 mm², SF6 −514.304543; SM1 8383.1217/20, hem 9254.2367/30, SM3 "Closed corners are not supported yet"; ADV2 5927.754632; EXP1 openNURBS. ✗ still.

## Re-run pass 18 (2026-10-22 ~10:00–11:00, CLI /tmp/qa5/noshape-cli-0811 = target-engine 08:11; main 3ce6952)
CLI rows unchanged: SF2 8000/10 faces, SF5 2400 mm², SF6 −514.304543; SM1 8383.1217/20, hem 9254.2367/30, SM3 "Closed corners are not supported yet"; ADV2 5927.754632; EXP1 openNURBS; SML1 loft Add "not supported yet". ✗ still.

## Re-run pass 21 (2026-10-23 ~17:30–18:30, app = /root/target-lead/debug/noshape 10-23 17:10 (main 88fbf8f), CLI = target-lead noshape-cli 10-23 13:45)
SF2 8000 / 10 faces, SF5 2400 mm² (all edges move), SF6 −514.304543 → ✗ still, routed kern.
| SF1 Fill preview while editing | the fill surface previews (f02) | ✅ **fixed** |
| (feature-sweep Fill #3) Continuity label | items show a bare "Position" select (f02) | ✗ → **qa5 fix**: SHOW_LABEL on Fill "Continuity" and Boundary surface "Boundary condition" (fillSurface.fs / bsurf.fs) |

## Re-run pass 21 (qa5, 2026-10-23, main 40e23871 app /root/target-lead/debug/noshape)
| I2 | face-only fields pick the face when clicking on its edge | ✅ fixed |
| I4 | row tooltip after picking a plane from the list hides | ✅ fixed |
