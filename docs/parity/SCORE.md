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
