# Parity score (qa, daily; docs/PARITY-SUITES.md)

One line per day (UTC date, the green it was measured on): models matched / total and features matched / total
(dx's T2 `parity_models`), scenarios matched / total (psui's runner against live's recordings), then the five biggest
causes of difference, each with its ticket.

| date | green | models | features | scenarios | biggest causes (tickets) |
|---|---|---|---|---|---|
| 2026-11-03 | dx 21:29 (run 45; 3 dead-studio documents unscored; prize-wheel-tim refetched, its Hole as live) | 26/55 | 303/480 (custom 7/109) | – |
| 2026-11-03 | dx 22:49 (run 47: valuesSortedById in feature order, cf-parameter-pattern 9/9; old studios' lowercase types, id-only faces by tessellated area: cf-o-ring-generator 10/11) | 27/55 | 308/480 (custom 12/109) | – |
| 2026-11-03 | dx 23:17 (run 48: pinned studio imports, opSplitFace edges / leaveOpen / makeSolid in the host: cf-setback-fillet runs; custom-failing documents 8 → 6) | 27/55 | 308/480 (custom 12/109) | – |
| 2026-11-04 | dx 05:56 (run 53: a pattern instance id past the pattern's last is its seed (stef-radijator 3 -> 5/6); speed now compares features' times) | 29/55 | 321/480 (custom 17/109) | – |
| 2026-11-04 | dx 08:12 (run 56: live 1903's sketch wire bodies and REST ids for the 29 cf-* documents; a sketch line by id is that line (exercise-chino 9/9); the host runs cf-fill-pattern's Fill patterns (custom-failing documents 10 -> 8)) | 29/56 | 331/482 (custom 18/111) | – |
| 2026-11-04 | dx 08:28 (run 57: on main with core's latest (knob-jason 11/11); qCreatedBy's face creators cached, attributes indexed) | 30/56 | 332/482 (custom 18/111) | – |
| 2026-11-04 | dx 08:50 (run 58: a mate connector on a sketch region, a written CAP_VERTEX, a part pattern of a sketch region (cf-transform-pattern 14 -> 20/21); opThicken joins a sheet's faces (cf-belt 3/3)) | 32/56 | 340/482 (custom 22/111) | – |
| 2026-11-04 | dx 11:29 (run 63: attribute-based corner breaks, a localized Boolean's edges and vertices, attributes go with their entity (cf-sheet-metal-overcut-dogbone 2 -> 9/10, cf-fill-pattern 8/11); the qCreatedBy and name lookup caches) | 33/56 | 355/482 (custom 26/111) | – |
| 2026-11-04 | dx 12:19 (run 65: opPattern of a solid's faces repeats them in the body; ties between equal faces go to live's part (cf-fill-pattern 11/11)) | 34/56 | 358/482 (custom 28/111) | – |
| 2026-11-04 | dx 12:54 (run 67: each studio compiles against the std of its own pin (54 releases shipped, FS-OLD-STD-COMPAT)) | 35/56 | 369/482 (custom 33/111) | – |
| 2026-11-04 | dx 15:37 (run 69: 93 std releases shipped (39 more from live; TYPED_SINCE 2473); std's units / vector operators natively from 2506 (cf-o-ring-generator 11/11)) | 36/56 | 370/482 (custom 34/111) | – |
| 2026-11-04 | dx 15:56 (run 70 on main 539b4d6f6 + FeatureScript maps as short vectors: cf-circular-pattern 9 -> 13/13, cf-linear-pattern-with-seed-location 8 -> 12/12, cf-curve-pattern-normal 4 -> 5 (main's); pulley02-maxwell 7 -> 2 (Fillet 1), clamp-hms 14 -> 11 (Extrude 5): from main since 14:53, not dx's (the same without it; core.md)) | 37/56 | 371/482 (custom 38/111) | – |
| 2026-11-04 | b3acf8a5 (dx daily 16:48, live's corpus resynced; every document as run 71; 7 of 41 slower than live, run 70 had 9) | 37/56 | 371/482 (custom 38/111) | – | 4× extrude: faces / edges / vertices (clamp-hms Extrude 5 since main ~14:53, core); 2× fillet: faces / edges / vertices (pulley02-maxwell Fillet 1 since main ~14:53, core); 1× cf-spur-gear #teeth teeth: parts; 1× custom feature not built (cf-curve-pattern-normal Surface Tangent Plane 1, PARITY-CUSTOM-RUN-FAILURES); 1× Could not split with selected edge (overcut Fold SM 1 on std 1188, FS-OLD-STD-COMPAT) |
| 2026-11-05 | dx 04:23 (run 97: query results in live's order, Up to Part's end in one edge, std 1431-2559 evaluateQuery, CENTER on a rounded corner, the translator's blended faces and caps: cf-sheet-metal-overcut-dogbone 9 -> 10/10, clamp-hms 14 -> 15/15; cf-fix-pcb's Fix PCB 2 runs) | 42/55 | 393/471 (custom 47/109) | – | 1× driveTrainGen: volume; 1× extrude: faces / edges / vertices; 2× fillet (faces / edges, status ERROR "Unfilleted edge overlapped by fillet."); 1× fixPCB: face types |
| 2026-11-05 | dx 05:05 (run 101: MID_POINT with a secondary edge (live 2050), qBodyType of qEverything keeps only those bodies, an affine map of a planar body keeps its planes: cf-transform-pattern 20 -> 21/21, cf-fix-pcb 11 -> 12/12) | 44/55 | 395/471 (custom 49/109) | – | 1× driveTrainGen: volume; 1× extrude: faces / edges / vertices; 2× fillet (faces / edges, status ERROR "Unfilleted edge overlapped by fillet."); 1× hoseRouting: centroid (live's approximate, REQUESTS 04:41) |
| 2026-11-05 | dx 06:10 (run 105: opExtendSheetBody up to a surface, opFitSpline's centripetal knots, the UI fetch's boxes as outer ones and live's centroid bands; live's corrected model-plus counts and a new document, train-joe: cf-sheet-metal-shaped-flange 5 -> 9/10, cf-hose-routing 12 -> 16/16, cf-sheet-metal-model-plus 8 -> 9/14) | 45/56 | 415/483 (custom 54/109) | – | 2× newSketch: status (a missing external reference); 1× driveTrainGen: volume; 1× extrude: faces / edges / vertices; 2× fillet (faces / edges, status ERROR "Unfilleted edge overlapped by fillet.") |
| 2026-11-05 | dx 06:40 (run 107: opEdgeChange joins run-on edges, a Derived feature's source built from derived/<element>/, part patterns' Add with an empty merge scope: cf-sheet-metal-hem 4 -> 5/6, cf-sheet-metal-shaped-flange 66/152/88 of live's 65/149/86) | 45/56 | 416/483 (custom 54/109) | – | 2× newSketch: status (a missing external reference); 1× driveTrainGen: volume; 1× extrude: faces / edges / vertices; 2× fillet; 1× transform: status ERROR (a derived part's vertex unmapped) |
| 2026-11-05 | dx 06:59 (run 109: bend reliefs beside a tapered wall, missing external references as live's: cf-sheet-metal-shaped-flange 9 -> 10/10, train-joe 11 -> 12/12, clamp-guillem 23 -> 24/25) | 47/56 | 419/483 (custom 55/109) | – | 1× driveTrainGen: volume; 1× extrude: faces / edges / vertices; 3× fillet; 1× transform: status ERROR (a derived part's vertex unmapped); 1× sheetMetalFlange: parts |
| 2026-11-05 | dx 09:27 (run 115: Flange 1 as live (a moved sheet edge keeps its name on its old segment, a curved bend stops at its wall's pulled side, free ends inside a neighbouring bend's pull-back go with it), packed CAP_VERTEX queries, opThicken's caps, opCreateOutline on curved faces and as the tool's shadow on planes, live's refs on the build host: cf-sheet-metal-model-plus 9 -> 13/14, cf-sheet-metal-hem 5 -> 6/6) | 49/56 | 425/483 (custom 56/109) | – | 1× driveTrainGen: volume; 1× extrude: faces / edges / vertices; 1× extrude: area (SM-CURVED-OUTLINE-SHADOW); 1× fillet: faces / edges / vertices; 1× fillet: status ERROR "Unfilleted edge overlapped by fillet." |
| 2026-11-05 | dx 10:34 (run 119: a curved sheet metal outline as the tool's shadow; tracking queries 3x faster: cf-sheet-metal-model-plus 13 -> 14/14, 2.5 -> 1.2 s) | 50/56 | 426/483 (custom 56/109) | – | 1× driveTrainGen: volume (live's blank, REQUESTS 09:59); 1× extrude: faces / edges / vertices; 1× fillet: faces / edges / vertices; 1× fillet: status ERROR "Unfilleted edge overlapped by fillet." |
| 2026-11-05 | dx 10:51 (run 120: a part's volume also passes within 1e-6 of live's exact one (volume_high, evVolume HIGH: massproperties' band misses it on the drive train's 62- and 50-tooth gears, live 2058): cf-drive-train-generator 1 -> 2/2) | 51/56 | 427/483 (custom 57/109) | – | 1× extrude: faces / edges / vertices; 1× fillet: faces / edges / vertices; 1× fillet: status ERROR "Unfilleted edge overlapped by fillet."; cf-setback-fillet / cf-straight-spline (core) |
| 2026-11-05 | dx 15:58 (run 125: sheet metal's face point queries cheaper, results unchanged (a bend's inward side and edge convexity by the face classifier, a thickened cap's foot on its sheet, cached split-piece tests, a box check before Up to Part's distances): cf-sheet-metal-model-plus 1153 -> 870 ms, live 917) | 51/56 | 427/483 (custom 57/109) | – | as run 120 |
| 2026-11-06 | dx 10:55 (run 139 on main cf7d3c6c + thicken caps by history: live's c2 REST features.json (2132 partly in), cf-fill-pattern 4.5 s / live 4.7, cf-wave-spring 335 ms / live 740, faces_cross bounded by work) | 58/72 | 530/753 (custom 54/55) | – | 2× fillet: #/#/# faces/edges/vertices; 2× translation incomplete: status NOT BUILT; 1× deleteFace: #/#/# faces/edges/vertices; 1× extrude: #/#/# faces/edges/vertices; 1× extrude: centroid [#, #, #] mm |
| 2026-11-06 | dx 13:05 (exchange suite, PARITY-SUITES §3, batch 1 of 12 documents: ours imports live's STEP / Parasolid / IGES / STL 19/36, live imports ours 19/33 (15 not imported yet), ours reads ours back 43/48; fixed today: multi-part Parasolid export (Onshape rejected every one), swept surfaces and G1 rings in Parasolid, STEP rings split at a seam; drawings: 16/16 drw-* (94/94 states), coverage tools 30/58, menu items 4/119, shortcuts 5/14, export formats 4/10 with batch 3's 20 scenarios queued for live) | 58/72 | 530/753 (custom 54/55) | – | exchange: IGES faces split (ours 62 for live's 48 on clamp-hms), cf-spur-gear STEP volume −0.3 %, cf-o-ring-generator spun surfaces +0.06 %, sheet metal STEP faces |
| 2026-11-06 | 7031aa22 (psui 08:30: the whole interaction suite, tools/ui-parity/shards.sh on vibe5; models / features: dx's next run) | – | – | 308/310 (1960/1962 states) | 2× drw-*: a hovered view live's recorder reports as selected (drw-dimension-radius, drw-hole-table; dx, live's recorder). Measured with the keys live records that ours didn't (dialog ✓ state, Parts heading, focus dialog, autocomplete row texts, sketch dimension expressions): they showed 3 differences, fixed on main after this green (the value box's typed length kept as the dimension's expression, psui) |
| 2026-11-06 | 70e75ac4 (qa 21:20: both suites on this machine, qa-scratch/bin/qa-suites.sh, every 4 h from now; /tmp/lead/suites-status) | 58/86 | 593/924 (custom 56/113) | 462/486 (2966/3001 states) | models: 4× custom feature not built, 4× translation incomplete (Extrude without its region), 2× custom feature failed to run, 2× extrude and 2× fillet face / edge / vertex counts; scenarios: 3× drw-* (dx: export formats, hole callout, note delete), 5× sketch-* / sketch row (skasm: circumscribed polygon, horizontal key, mirror, offset, copy-paste onto Front), 2× sm-* (bend, form tools), 11× part studio dialogs and menus (psui: analysis-thickness, the dialog right-click menus, external thread, suppress menu, fillet measurement list, loft, mirror face type, offset surface, part menu release, line-angle plane) |
| 2026-11-07 | 727f5b09 (qa 09:57: the 09:40 run of qa-suites.sh; /tmp/lead/suites-status) | 59/88 | 638/936 (custom 62/113) | 619/762 (4091/4361 states) | models: 4× custom feature not built, 4× translation incomplete (not built), 2× custom feature failed to run, 2× fillet counts, 2× fillet failed; scenarios: 21× drw-* start state (view scale not ready in the 6-shard run, UIP-DRW-START-FLAKY, dx), 15× other drw-* (panels not opened, sheet duplicate naming, selection after note edits: dx), face-menu-select-other (UIP-SELECT-OTHER-LIST, psui) |
| 2026-11-07 | dx 15:01 (2132 rerun: the 20 c2-* by REST with sketch bodies; vibe5, main 46e48dc6 + dx's queue) | 59/88 | 638/936 (custom 62/113) | – | c2 first differences in docs/handoff/dx.md (skasm: c2-duct-multipart Extrude 3 non-manifold, c2-sm-louver Louver / Sheet metal model 1; core: fillets, sweep, shell, loft; dx: Derived / Import translations) |

dx 11-03 18:15: three documents are left out of the model score (tests/parity-models/unscored.json, printed as "not
scored" by `parity_models`): cf-ansi-washers, cf-socket-head-cap-screw-creator and cf-thread-creator. Their Feature
Studios import studios of documents that are no longer public (live gets 403 fetching them, REQUESTS 1885), so their
custom features can't be built in noshape or anywhere else; they measure nothing about us.

qa 11-03 05:55: the scenario column counts the scenarios with a live recording at compare time (112 of 162 runs; live was
re-recording); the evening run replaces today's numbers. Scenario differences are psui's (its runner reports its own
score), first-difference keys today: dialog field kinds / labels (toggle vs boolean, "Distance 1" vs "Distance"), feature
row states, selection after a step.
qa 11-03 08:04: scenarios on green 7d35f2cb against live's 184 recordings: **101/184 scenarios, 720/945 states**
(qa-scratch/uip-score3.log). First-difference causes: the sketch plane pick as an undo step (33 scenarios, SKETCH-UNDO-REDO-STATE
reopened), dialog field kinds (enum vs boolean), feature row states, a new dialog's Undo (NEW-DIALOG-UNDO-STEP).
qa 11-03 14:30: on green f5707754 against live's 63 re-recorded scenarios: **61/63 scenarios, 463/465 states**; the two left
are SKETCH-TOOL-PICK-SELECTION (P3).
qa 11-03 19:22: on green c4d5d8af against live's 128 recordings: **116/128 scenarios, 892/932 states** (psui's 17:30
run 114/128). Left, all already owned: 7 fillet-* (live's runner leaves Entities to fillet empty: REQUESTS 1850 / 15:05),
sketch-circle-at-origin-coincident (now VERTICAL vs our COINCIDENT at step 5), sketch-construction-toggle, sketch-fully-defined,
sketch-extend-to-line (stale recording, re-record asked) — skasm.md; extrude-end-type-up-to-face (live's start) — psui.md.
qa 11-03 21:16: the 12 left re-run on green ece6c4c8 against live's re-recordings (tests/ui-parity/live): sketch-construction-toggle
matches now (→ 117/128); sketch-fully-defined now differs only at step 13 (focus: live none, ours other); the 7 fillet-* re-recorded at
20:25 still open with Entities to fillet empty in live (REQUESTS 1850 / 15:05 stays open).
qa 11-03 23:02: on green 9cb98b67 against live's 136 recordings in tests/ui-parity/live: **124/136 scenarios, 967/997 states**.
Left: 7 fillet-* and extrude-end-type-up-to-face (live's recordings, as before), sketch-extend-to-line (stale, re-record asked),
sketch-fillet-corner (fixed by skasm 2e0485c2, after this green), sketch-delete-line, sketch-equal (psui's notes to skasm).
qa 11-04 04:14: on green ad60edf9 against live's 286 recordings: **259/286 scenarios, 1734/1818 states** (undo-rename
counted after a re-run: the first run's app couldn't connect to the X display at start, a harness hiccup; it matches, 6/6). Left:
fillet / chamfer "error" titles (10, incl. new-fillet-then-dblclick-other, undo-new-fillet: live's preselected edge, REQUESTS
1850), hole-* (6: live's account remembers M6x1.00 ↧ 15 mm, behaviours 1849 item 1, a neutral start pending), measure-fillet-face
(POINT vs EDGE), sketch-delete-line / -vertical-constraint / -extend-to-line (selection), revolve-dialog-axis, face-rclick-menu,
mass-properties-part (Mass override enabled), section-view-front (focus), extrude-end-type-up-to-face (live's start).
dx 11-05 16:35: the 8 drawing scenarios (drw-*) against live's recordings (2063, /root/parity/ui/live): **2/8 scenarios, 37/45 states** (first run 0/8, 2/45). Left: the Insert view's scale on a No views sheet (2:1 live, ours 1:1: REQUESTS 16:12), what the selection pixels mean (hover or selection: REQUESTS 16:27, 4 states), drw-create-of-part's step 4 recorded while live still created the drawing (re-record asked).
dx 11-05 20:43: drw-* against live's re-recordings (2068): **7/8 scenarios, 42/45 states**; left: Insert view's default scale (drw-insert-front-and-right, REQUESTS).
dx 11-06 01:08: all 16 drw-* against live's recordings (batches 1-2, 2131 / 2133 / 2134): **14/16 scenarios, 92/94 states**; left: two hovered frames live's recorder reports as VIEW (drw-dimension-radius step 4, drw-hole-table step 3).

psui 11-06 12:08: the whole suite on main 0d395f78 (vibe5, shards.sh) against live's recordings of coverage batches 1-4:
**377/382 scenarios, 2450/2463 states**. Coverage (tests/ui-parity/README.md, coverage.py): toolbar 25/76, sketch tools
43/55, menus 27/117, shortcuts 59/89; batch 5 (17) is with live. Left: sketch-horizontal-key, sketch-offset
(SKETCH-SCREEN-SPACE-REACH, skasm), sketch-circumscribed-polygon (SKETCH-POLYGON-CONSTRAINTS, skasm), part-menu-properties
(our Properties modal isn't in the recorded state yet, psui), sketch-mirror-tool (one entity reads selected after a mirror,
psui).

psui 11-06 14:47: the whole suite on 5ab5b13a (queued; vibe5, shards.sh): **378/382 scenarios, 2452/2463 states**
(the Properties modal in since 12:45; paste rows are compared only in scenarios that copy something: live's session
clipboard carried a sketch into 6 older recordings). Live's first recordings of batches 5-7 (not on main yet,
/root/parity/ui/live): **23/34 scenarios, 201/222 states** (9/18 before today's fixes); left: account memory (Hole
settings, 2), live runner artefacts (a menu pick by prefix, 3: REQUESTS), Mirror's Face mirror focus (asked live),
Export as DXF/DWG's modal (EXPORT-DXF-DIALOG), the server round trip after Suppress, a measurement's chip "Unknown of".
Coverage: toolbar 47/76, sketch tools 43/55, menus 62/117, shortcuts 59/89 (batches 6-9 with live).

psui 11-06 16:49: the whole suite with live's batches 5-9 on main (447 recordings), on 21064380 (queued): **417/447
scenarios, 2770/2849 states**. Left in the Part Studio: account memory (Hole, 2), live runner artefacts (menu pick by
prefix, 3; a new sketch's owner, 1), the server round trip after Suppress, Mirror's Face mirror focus (asked live), a
number field's right-click focus (psui handoff), "Unknown of" chips, skasm's three sketch ones, the mirror line
(SKETCH-MIRROR-LINE-SELECTED), the flaky asm-fastened-undo-picks (UIP-ASM-FASTENED-FLAKY); the rest are drw-* (dx).

psui 11-06 19:58: the whole suite with live's batches 5-9 (487 recordings) on ffe5d877: **451/487 scenarios, 2946/3027
states** (432 before today's fixes from them); since, abe55ca3 makes fill-tool, boundary-surface-tool,
part-menu-copy-here and the two folder submenu scenarios match too (5 more, checked alone). Left in the Part Studio:
account memory (Form's Circle emboss, External thread's 13/16), the free plan (Thickness analysis, Release's upsell),
questions with live (Surfaces collapsed?, Bend's title, Face mirror focus, a checkbox's right-press focus), the number
field's right-click focus, the server round trip after Suppress, "Unknown of" chips, a new sketch's owner, skasm's
three sketch scenarios, SKETCH-MIRROR-LINE-SELECTED. Coverage: toolbar 76/76, sketch tools 55/55, menus 92/117,
shortcuts 78/89 (batches 10-14 with live).

## Own Boolean's share (bool, daily; `tools/bool-share.sh`)

Of the recorded Booleans (each distinct call once), the share noshape's own Boolean takes instead of OCCT, by count and
by the OCCT time those calls cost (OCCT run on vibe4, each once). Parity: the 56 parity documents' 594 Booleans
(recorded 11-05 17:25 from `parity_models` on main, vibe4 ~/agents/bool-corpora/parity56). All: those and bool-corpus,
corpus5, the shell recordings (shfall, sh-new2, shv, shell6) and the knurl sweep (7 cases, 360 s of OCCT's 790 s).

| date | main | parity: count | parity: OCCT time | all: count | all: OCCT time | all but knurls: OCCT time |
|---|---|---|---|---|---|---|
| 2026-11-05 | 38f9abc52 (17:29) | 318/594 (53.5 %) | 43.3/73.0 s (59.4 %) | 1855/2498 (74.3 %) | 495/790 s (62.7 %) | 135/430 s (31.3 %; shfall's 191 Shell fuses, 266 s, 4 % taken) |
| 2026-11-06 | 6d2434999 (00:42) | 350/594 (58.9 %) | 78.2/102.4 s (76.4 %) | 1887/2498 (75.5 %) | 520/824 s (63.1 %) | 186/490 s (38.0 %; shfall 46/191, 12 of 277 s) |
| 2026-11-06 | 628204762 (18:22) | 350/594 (58.9 %) | 60.9/80.1 s (76.0 %) | 1893/2498 (75.8 %) | 532/790 s (67.4 %) | 143/401 s (35.7 %; shfall 51/191, 10 of 240 s; vibe4 loaded, OCCT's times ±20 %: the counts are the measure) |
| 2026-11-06 | dx 21:39 on 8a073492 (exchange suite, 12 documents: ours imports live's STEP / Parasolid / IGES / STL 40/48 (34 at 20:20), live imports ours 23/48 (the five multi-part x_t re-import asked, P0 fix 9b48a753), ours reads ours back 44/48; fixed: multi-part Parasolid attribute chains, STL / OBJ / 3MF a part per mesh with hollows, IGES revolved faces as tori, seam-split B-spline copies one edge (motor-mount STEP); drawings: batch 3 30/35 scenarios, coverage tools 42/58, menu items 14/119, shortcuts 10/14, sheets 10/13 with batches 4-5 (22 scenarios) queued for live) | – | – | – | exchange: IGES edges / vertices of merged periodic faces (pulley 8/0, live's IGES 10/4), the o-ring's B-spline torus quarters (live 1 face), hose STEP 12/12/0 (live's STEP import 12/16/8, its model 12/12/0), cf-spur-gear STEP / IGES volume within the files' precision; drawings: export formats (re-record), hole callout and note selection (asked) |

## Assemblies (asm, daily; T2 `parity_assemblies` over /root/parity/models/asm-*, docs/PARITY-SUITES.md §4)

Per assembly: rebuilt through crates/ops, each occurrence placed where live has it and held while its mates are added,
then freed: matched when ours places it within 1e-6 m / 1e-6 of live's and our mates don't move it; each mate's status
as live's; the BOM rows (once live's corpus has bom.json). `cargo test -p noshape-ops --test all
parity::parity_assemblies -- --ignored --nocapture` (NOSHAPE_PARITY_ASSEMBLIES for another folder).

| date | main | assemblies as live | occurrences | mates | BOM rows | biggest causes (tickets) |
|---|---|---|---|---|---|---|
| 2026-11-06 | 0dfa4c2da (23:54) | 3/3 | 8/8 | 5/5 | – | only live's 3 small built cases so far; the public corpus (15–20 assemblies) asked of live |
| 2026-11-07 | 2cbc4192e (08:00) | 4/24 | 181/1731 | 19/23 | – | live's corpus is in (24); 9 big public ones lose their Part Studios' features in translation (no mates rebuilt, "0/0 mates"): the translator's gaps, next; the suite now names parts from the assembly when live sent `.mass.json` only (it skipped every such studio before) |
| 2026-11-07 | d987386b5 (08:45) | 4/24 | 530/1731 | 51/58 | – | live's part names map to ours (by name, only part, or `.mass.json` volume band): scissor-lift 167/242, pattern-std 76/162, isaac-ball 71/121; what's left is Part Studio translation (variables, patterns, external references: dx's translator) and xdev-subs' subassemblies (4/286) |
| 2026-11-07 | 0adb0644d + translator batch (11:08) | 4/24 | 600/1731 | 56/81 | – | asm's translator fixes without live geometry (standard-content configurations, part ids by creation order, id-only regions by `.mass.json` volume or the outer region, merge scopes by part count, revolve axes on the profile's side): robot-relations 9 → 66, config-assembly 1 → 16, gearbox-std 23/23, scissor-lift 140/242 (a studio with one part no longer takes every instance by luck); Part Studio suite unchanged (59/88, 638/936). Left: blob imports, id-only face/edge queries (~900), variables, studios live didn't fetch (REQUESTS) |
