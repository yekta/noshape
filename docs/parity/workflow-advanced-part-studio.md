# Workflow: advanced Part Studio (a CAD user's session, step by step against live Onshape)

Document "qa5 advanced" (mm), built by hand (mouse + keyboard). Live ground truth: behaviours.md "Geometry ground
truth" G9/G10/G12 (loft, sweep), G1 D1 (draft), O1 (move face), "Feature dialog basics", dom/dialog-*.txt;
pending request "qa5 (10-21 advanced Part Studio)" for rib, wrap, thread, hole types, intersect.
Status: ✅ matches live/exact, ✗ differs (finding #), ? live data pending.

**Last run: 2026-10-21 (session 13:37–14:29), qa5**, binary /tmp/qa5/noshape-1337 (target-shared build 13:03).

## 1. Loft (Part Studio 1; live G10(a))

| # | Step | Live | Ours | Status | Verified |
|---|---|---|---|---|---|
| 1.1 | Sketch 1: 20×20 square on Top, from the origin | – | fully defined with D | ✅ | 7929b13a 10-28 (corner rectangle from the origin, both dimensions typed; note: since c1b58ff7 Tab no longer applies (live), so the flow is 20 Enter 20 Enter) |
| 1.2 | Plane (Top + 20) | Plane 1 | Plane 1 | ✅ | 7929b13a 10-28 |
| 1.3 | Sketch, Sketch plane field active, click the **Plane 1 row** in the feature list | fills "Plane 1" (default planes work the same way) | **nothing**; clicking Plane 1 in the viewport doesn't fill it either; the Top row works | ✗ ADV1 | 7929b13a 10-28 (Sketch with nothing selected, click the Plane 1 row → "Plane 1"; ADV1 fixed) |
| 1.4 | Select Plane 1 first, then Sketch | sketch on Plane 1 | works | ✅ | 7929b13a 10-28 (Plane 1 selected, Sketch → "Plane 1", field inactive) |
| 1.5 | Circle Ø16 at (10,10) | – | ✅ | ✅ | 64997436 10-28 (GUI: Sketch 2 on Plane 1, Ø16 at (10,10)) |
| 1.6 | Loft dialog | Solid/Surface/Thin, New/Add/Remove/Intersect, Profiles, End conditions, Guides and continuity, Path, Connections, Show isocurves | same | ✅ | f9185639 10-28 (dialog fields) |
| 1.7 | Click the circle region | "Face of Sketch 2" | first click took the circle's **centre point** ("Sketch 2 point" → pyramid to a point); the region took a second click | ? (point vs region priority) | 64997436 10-28 (GUI: a click on the centre picks 'Vertex of Sketch 2' (point first, as any point pick); a click inside the circle off the centre picks 'Face of Sketch 2') |
| 1.8 | Result (square 20 → Ø16, 20 high) | **5927.728022 mm³**, side faces 324.6214685 mm² each | **5927.755 mm³** (+0.027), area 1899.548 (live 4×324.6215+400+201.0619 = 1899.548) | ✗ ADV2 (volume 0.027 off; area exact) |  |

## 2. Sweep (Part Studio 2; live SW rows)

| # | Step | Live | Ours | Status | Verified |
|---|---|---|---|---|---|
| 2.1 | Ø6 circle on Top; path on Front: line 30 + tangent arc | – | built and fully defined | ✅ | main CLI 12:08 (bin/advmatrix.py) (line 30 + tangent arc via tangent_arc_switch) |
| 2.2 | Sweep dialog | Faces and sketch regions to sweep, Sweep path, ☐ Extend to full path, Profile control None, ☐ Twist, ☐ Scale | same | ✅ | f9185639 10-28 (dialog fields) |
| 2.3 | Path field: click the Sketch 2 row | ? | notice "Select edges, sketch curves or curves."; viewport picks work (two chips "Sketch 2 line", "Sketch 2 arc") | ? |  |
| 2.4 | Arc R3 | **981.4696759** | **981.47** | ✅ | main CLI 12:08 (bin/advmatrix.py): 981.469676, 4 faces |
| 2.5 | Arc R2 | ERROR "Sweep R2 did not regenerate properly: Could not create valid swept body, check input." | row tooltip "Could not create valid swept body, check input." (**no "Sweep 1 did not regenerate properly:" prefix**) | ✗ ADV3 |  |
| 2.6 | Arc R6 | **1114.7093353** | **1114.709** | ✅ | main CLI 12:08 (bin/advmatrix.py): 1114.709335 |

## 3. Fillet, draft, holes, mirror (Part Studio 3)

| # | Step | Live | Ours | Status | Verified |
|---|---|---|---|---|---|
| 3.1 | Block 40×20×10; Fillet 4 vertical edges R3 | 7922.743339 | ✅ (G7 value) | ✅ | main CLI 12:08 (bin/advmatrix.py): 7922.743339 |
| 3.2 | Draft: neutral Top, 4 side faces (tangent propagation), 5° | **7428.357160** (D1) | **7428.357** | ✅ | main CLI 12:08 (bin/advmatrix.py): 7428.357160 |
| 3.3 | Edit Extrude 1 depth after the draft | regenerates | regenerates | ✅ | 64997436 10-28 (GUI: Extrude 1 10 → 15 mm with Fillet 1 + Draft 1 after it: all regenerate, no error) |
| 3.4 | Shift+S twice in the Part Studio (sketch, then Point) | ? | worked as Sketch then Point | ? |  |
| 3.5 | Hole dialog | Inch/Metric, Simple/Counterbore/Countersink, Sketch points, Merge scope, Hole type Drilled, Drill size 1/4 (0.25), Ø, Start plane, Termination Blind, depth, 118 deg | same (Simple matches hole-01 screenshot) | ✅ | f9185639 10-28 (dialog fields) |
| 3.6 | Counterbore defaults | Ø10 × 5 mm, 12 mm deep (psb f) | same; 6877.312 mm³ = exact | ✅ | 41f60e47 10-28 (Ø10 × 5, 12 deep) |
| 3.7 | Counterbore / countersink rows | (live look pending) | text labels **truncate the values** ("Counterbore diamet… 1(", "Countersink angle 82 de") and with Tapped the **"Countersink diameter" label overlaps "12.878 mm"** | ✗ ADV4 |  |
| 3.8 | Hole type list | Drilled, Clearance, Tapped | same; Tapped adds Tap type, Size, Threads/inch, Fastener fit, ☐ Thread class, Tapped depth | ✅ (live behaviours ~8515: Tapped → Tap type Straight tap, Size, Threads/inch, Fastener fit None, ☐ Thread class) | 64997436 10-28 (GUI: Drilled / Clearance / Tapped; Tapped shows Tap type, Size 1/4, Threads/inch 20 tpi (UNC), Fastener fit None, ☐ Thread class, tap drill 5.105 mm, tapped depth) |
| 3.9 | Mirror › Part mirror about Right | Part 2 | Part 2 | ✅ | 64997436 10-28 (GUI: Part 1 + Right row → Part 2) |
| 3.10 | Mirror › Feature mirror of the hole about Plane 1 (Right + 20) | dialog: Features to mirror, Mirror plane, ☐ Reapply features | same; **Plane 1 row doesn't fill Mirror plane** (ADV1); viewport pick works; 6326.267 mm³ = exact | ✅ / ✗ ADV1 | 64997436 10-28 (GUI: the Plane 1 row fills Mirror plane: ADV1 fixed for mirror; tickets/adv1-mirror-plane-row-6499.png) |
| 3.11 | Edit the hole to Countersink + Tapped | mirror follows | Mirror 2 error "Could not create all instances as entered. Try selecting "Reapply features" option." (the two countersinks overlap at the plane); ☑ Reapply features fixes it | ? (plausible per G45) |  |

## 4. Helix, thread, rib, wrap

| # | Step | Live | Ours | Status | Verified |
|---|---|---|---|---|---|
| 4.1 | Helix dialog | Cylinder/Cone \| Axis \| Circle; Conical or cylindrical face; Input type Turns; Start condition Start angle 0; End condition Height; Target revolutions 4; Handedness Clockwise; ☐ Show start and end profiles | same (no "Handedness" label: the select stands alone) | ✗ ADV5 (label) |  |
| 4.2 | Helix on the Ø6 × 30 face | curve 4 turns | Curve 1, length **81.147 mm** = exact | ✅ | 64997436 10-28 (GUI: Helix on the Ø6×30 side face → Curve 1, a click on it reads Length 81.147 mm) |
| 4.3 | Space | clears the selection | clears | ✅ | 0.4.10+ / dd1b5037 10-27 |
| 4.4 | Tool search "thread" | External thread (Did you mean: Helix, Hole) | same | ✅ | 41f60e47 10-28 |
| 4.5 | External thread: click the Ø6 cylinder's end edge | chip / size mismatch handling (pending) | **no hover, no pick**; a pre-selected edge is dropped on open; outside the dialog the same edge picks fine ("Diameter: 6.000 mm") | ✗ ADV6 |  |
| 4.6 | Rib dialog | Sketch profiles, Parts, Thickness, Parallel to sketch plane + flip, ☐ Draft, ☐ Extend profiles to part, ☑ Merge ribs | same; default 2.5 mm (live inch doc 0.1 in) | ✅ | f9185639 10-28 (dialog fields) |
| 4.7 | Rib: click the Sketch 2 row for "Sketch profiles" | ? | nothing added → ✓ gives "Select sketch profiles to create the ribs." | ? (same pattern as ADV1) |  |
| 4.8 | Rib 4 mm on the L bracket (line (5,15)→(20,5) on Front) | ~10050 (pending exact) | 10044.179 (my line endpoints approximate) | ? |  |
| 4.9 | Wrap dialog | (pending) | tabs Solid / **Surface** (default) / Split; Tools, Target, ☐ Trim to target, ☐ Specify anchor points, Angle, U shift, V shift | ? |  |
| 4.10 | New Part Studio from + while Part Studio 2 is active | inserted right after the active tab (behaviours.md "Tab bar") | after the active tab | ✅ | 77870dd4 10-28 (Part Studio 3 inserted after the active PS2) |
| 4.11 | N twice in a sketch | second N flips to the back side | flips | ✅ | 0.4.10+ / dd1b5037 10-27 (n twice flips) |

## Findings (2026-10-21)

| # | Finding | Repro | Owner |
|---|---|---|---|
| ADV6 | External thread's "Cylinder edges" never accepts the circular edge of a cylinder (no hover, no pick, pre-selection dropped) | Ø6 cylinder, External thread, click its end edge | psb / fs |
| ADV1 | Plane **feature** rows (Plane 1) can't fill plane fields (Sketch plane, Mirror plane), and Plane 1 in the viewport doesn't fill Sketch plane; default planes work | §1.3, §3.10 | psb |
| ADV2 | Loft square→circle volume 5927.755 vs live 5927.728022 (area matches) | G10(a) | kern |
| ADV3 | Feature error tooltip lacks "<Feature> did not regenerate properly:" prefix | sweep R2 | psb |
| ADV4 | Hole dialog counterbore/countersink rows: values truncated, label overlaps value | Hole › Countersink › Tapped | ui |
| ADV5 | Helix "Handedness" label missing | Helix dialog | ui |

## Update from live's answer (behaviours.md "qa5 advanced", 10-21) + re-run pass 3 (2026-10-21 14:40–15:20)

Binaries: UI rows /tmp/qa5/noshape-1433 (target-shared 14:32); CLI rows /tmp/qa5/noshape-cli-1436 (target-engine 14:36),
rebuilt exactly to live's spec in /tmp/qa5/home/Documents/noshape/qa5-livecheck.noshape (script /root/qa5-scratch/pass3/mklive.py).

| # | Row | Live | Ours (pass 3) | Status | Verified |
|---|---|---|---|---|---|
| 1.3 | Plane 1 into Sketch plane | row or viewport fills "Plane 1" | **viewport pick now fills it**; the feature-list **row still does nothing** | ✗ ADV1 (partly fixed) |  |
| 1.8 | Loft volume | 5927.728022 | 5927.755 | ✗ ADV2 |  |
| 2.5 | Sweep error tooltip | "Sweep 1 did not regenerate properly: …" | still "Could not create valid swept body, check input."; Helix error tooltip also bare ("Select a cone or cylinder.") | ✗ ADV3 |  |
| 3.7 | Hole countersink rows | – | "Countersink diameter" label still overlaps "12.878 mm", "82 de" cut | ✗ ADV4 |  |
| 4.1 | Helix handedness label | live DOM (qa5-adv-helix): the Clockwise select has **no label** | no label | ✅ **ADV5 withdrawn** |  |
| 4.5 | External thread edge pick | picks the edge | **hovers + picks "Edge of Sweep 1"** | ✅ ADV6 fixed | 64997436 10-28 (GUI: hover outlines the Ø6 top edge, click → 'Edge of Extrude 3') |
| 4.5b | Thread dialog "Threads/inch" value | "20 tpi" (dom qa5-adv-thread) | "20 tpi (UNC)" | ✅ (ADV8 fixed) | 64997436 10-28 (GUI: '20 tpi'; ADV8 fixed) |
| 4.5c | Thread result | cosmetic, **cylindrical face split at 10 mm (3 → 4 faces)**, area unchanged | area unchanged 2513.2741, **still 3 faces** (Split face ☑ default) | ✅ (ADV7 fixed) | 64997436 10-28 (GUI: committed, the side face is split 10 mm down: hover lights the lower band only; ADV7 fixed; tickets/thread-split-6499.png) |
| 4.6 | Rib defaults | 2.5 mm, Parallel to sketch plane, Merge ribs ☑; Parts auto-fills "Part 1" | same (UI pass 2) | ✅ | f9185639 10-28 (dialog fields) |
| 4.8 | Rib 4 mm, line (5,25)–(25,5) | **10550.0000, 11 planar faces** | **10550.0, 11 faces** (CLI) | ✅ | 64997436 10-28 (GUI: Sketch 3 row, Part 1, 4 mm → Mass props 10550 mm³) |
| 4.2b | Helix Ø20×30 default | 253.1115 mm | 253.1116 (analytic 253.11157) | ✅ | b504234e 10-28 (GUI: Helix on Ø20×30 side face, edge Length 253.112 mm) |
| 4.9 | Wrap 10×5 rectangle onto Ø20 | Surface tab default; sheet, 1 cylinder face 50.0000 mm² | Surface default; Surface 1, 1 cylinder face 50.0 | ✅ | b504234e 10-28 (GUI: Surface tab default, 10×5 on Front+10 onto Ø20 side face → Surfaces (1) Surface 1, Face Mass props 50 mm², perimeter 30 mm) |
| 3.6b | Hole types, block 40×20×10, point (20,10), Inch | Cbore 7448.9548 (9 f) / Simple 7683.3078 (7) / Csink 7643.7210 (8) | **7448.9548 (9) / 7683.3078 (7) / 7643.7210 (8)** | ✅ | b504234e 10-28 (GUI, Inch tabs on the (20,10) vertex, Mass props: Simple 7683.308 / Cbore 7448.955 / Csink 7643.721) |
| 3.6c | Hole dialog title | the live callout ("Ø 6.35 mm ↧ 12 mm \| ⌴Ø 10 mm ↧ 5 mm") | callout title + the feature row also named by callout ("1/4-20 ↧ 12 mm \| ⌵Ø 12…") | ? (row name: verify live) |  |
| 6 | Boolean intersect two blocks | Part 1, 4000.0000 | Part 1, 4000.0 | ✅ | b504234e 10-28 (GUI: Intersect tab, 40×20×10 ∩ offset 40×20×10 → Parts (1) Part 1, Mass props 4000 mm³) |
| 3.10b | Feature mirror of a 10×10×5 boss about Right+20 | no New/Add tabs; 9000.0000, 16 planar faces | 9000.0, 16 faces (CLI) | ❌ (FEATURE-MIRROR-COPY-FULL-DEPTH) | b504234e 10-28: ✗ GUI Feature mirror (Reapply off) 9500 / 15 faces, copy spans the part's full y |
| – | Switch to another Part Studio tab | parts list shows the tab's parts | **"Parts (0)", Curves group missing, right strip one icon short** until a body is clicked (Part Studio 2/4 here, holeblock in qa5 import; also on the 14:48 build) | ✗ **TAB1** |  |
| – | Sketch dimension edit to an unsolvable value, Enter | ? (live request filed) | box stays open, Esc ignored, its ✕ keeps the new value; red toast "The sketch could not be solved with that value" stays after the sketch closes | ? |  |

## Update from live's answers ("qa5 advanced, addendum: Hole row name", "qa5 dim conflict", 10-21)

| # | Row | Live | Ours | Verdict |
|---|---|---|---|---|
| 3.6c | Hole row name | the callout's **first line only** ("1/4-20 ↧ 12 mm"); tooltip "<callout> [Hole]"; Rename opens with "#featureName" selected; empty name restores the callout | row "1/4-20 ↧ 12 mm \| ⌵Ø 12…" (both lines) | ✗ **ADV12** (rename behaviour to check) |
| – | Dim set to an unsolvable value, Enter | box **closes**; value accepted (R2) but geometry stays; sketch, title and row red; **warning toast "⚠ Sketch could not be solved."** (yellow rgb(255,248,230), top centre), not auto-dismissed, **gone when the sketch closes**; Esc #1 reverts + blurs, Esc #2 closes; box ✕ cancels (stays R6) | box **stays open**, Esc ignored, ✕ keeps R2; **red error toast "The sketch could not be solved with that value"**, still shown after the sketch closed | ✗ **SK1** |

## Re-run on integration 5d28447 (2026-10-21 17:05–17:20, /tmp/qa5/noshape-1659 = /root/target-lead/debug/noshape 16:59; no CLI built from 5d28447 yet, so CLI-only geometry rows stay on the 14:36 CLI result)
| Row | Ours | Status | Verified |
|---|---|---|---|
| ADV1 Plane 1 row → Sketch plane | nothing | ✗ still |  |
| ADV2 loft | 5927.755 | ✗ still |  |
| ADV3 / SK1 dim conflict | box stays open, red "The sketch could not be solved with that value" | ✗ still |  |
| ADV4 countersink rows | icon rows, values fit, no overlap | ✅ **fixed** | 64997436 10-28 (GUI: Inch › Countersink › Tapped: icon rows, 13.487 mm / 82 deg / 12 mm / 12 mm / 3 all fit, no overlap; 20 tpi (UNC), ☐ Thread class = live dom qa5-adv-hole-tapped) |
| ADV7 thread split face | whole cylinder highlights as one face | ✗ still |  |
| ADV8 "20 tpi (UNC)" | unchanged | ✗ still |  |
| ADV12 hole row name | both callout lines | ✗ still |  |
| TAB1 tab switch Parts (0) | PS2 still "Parts (0)" on open (holeblock in qa5 import OK) | ✗ still (partly) |  |
| SK2 F in a sketch | fits the far rectangle (tight margin) | ✅ **fixed** (no "Sketch N" outline yet) |  |

## Re-run pass 7 (2026-10-21 18:56–19:50, app /tmp/qa5/noshape-1851 = target-shared 18:51 (tree d5745c9 + uncommitted work), CLI /tmp/qa5/noshape-cli-1809 = target-engine 18:09)
Pass 6 (17:11 build, d5745c9) had already found ADV8 fixed ("20 tpi"); not re-checked here.
| Row | Ours | Status | Verified |
|---|---|---|---|
| ADV1 Plane 1 row → Sketch plane | field stays empty | ✗ still |  |
| ADV2 loft | 5927.754632 (CLI) | ✗ still (live 5927.728) |  |
| ADV3 error tooltip prefix | "Helix 1 did not regenerate properly: Select a cone or cylinder." | ✅ **fixed** | 64997436 10-28 (GUI: empty Hole committed → row tooltip '1/4-20 ↧ 12 mm | ⌵Ø 13.487 mm X 82° [Hole] did not regenerate properly: No hole points selected.') |
| SK1 dim set to an unsolvable value | Enter closes the box, value R2 kept, geometry red, **yellow "⚠ Sketch could not be solved."** toast, gone when the sketch closes ✅; **dialog title "Sketch 2" and the feature row text stay black** (live red) | ✗ still (mostly fixed) |  |
| SK2 | the "Sketch 2" plane outline now shows while editing | ✅ |  |
| ADV7 thread splits face | still one cylindrical face, 565.487 mm² (whole 30 mm) (CLI) | ✗ still |  |
| ADV12 hole row name | both callout lines; tooltip "Hole 1 <- Sketch 2" | ✗ still |  |
| TAB1 tab switch | Part Studio 2 and 3 open with "Parts (0)" and 4 right-strip icons until a body is clicked | ✗ still |  |
| RD1 stale Volume readout | **readout gone**: nothing at the bottom right with a part selected (live has none) | ✅ **fixed** | 0.4.10+ / dd1b5037 10-27 (no readout with a part selected) |

## Re-run pass 8 on main 5bd50b2 (2026-10-21 23:13–00:10, app /tmp/qa5/noshape-2256 = /root/target-lead/debug/noshape 22:56; no newer CLI than target-engine 18:09, so CLI-only rows stay on pass 7; geometry rows re-read in the app where possible)
| Row | Ours | Status | Verified |
|---|---|---|---|
| ADV1 Plane 1 row → Sketch plane | fills "Plane 1" | ✅ **fixed** | 0.4.10+ / dd1b5037 10-27 (Plane rows fill Sketch plane / Entities) |
| ADV2 loft (Mass properties) | 5927.755 mm³ | ✗ still (live 5927.728) |  |
| ADV7 thread splits face | hover shows two cylinder faces, split ~10 mm up | ✅ **fixed** |  |
| SK1 unsolvable dimension | dialog title and row now red, ✓ disabled | ✅ **fixed** |  |
| ADV12 hole row | tooltip "1/4-20 ↧ 12 mm \| ⌵Ø 12.878 mm X 82° [Hole]" ✅; row still shows both callout lines | ✗ still (partly) |  |
| TAB1 | Part Studio 2 still opens with "Parts (0)" (Part Studio 3 listed its parts) | ✗ still |  |

## Re-run pass 13 (2026-10-22 ~05:20–06:40, main aa75fdf: app /tmp/qa5/noshape-0503 = /root/target-lead/debug/noshape 05:03, CLI /tmp/qa5/noshape-cli-0226 = target-engine 02:26)
| Row | Ours | Status | Verified |
|---|---|---|---|
| ADV2 loft (CLI) | 5927.754632 | ✗ still (live 5927.728022) |  |
| ADV12 hole row name | "1/4-20 ↧ 12 mm \| ⌵Ø 12…" (both parts, ellipsis); tooltip "… [Hole] <- Sketch 2" | ✅ — **withdrawn**: live psb 10-21 p (behaviours ~7448) shows both callout parts; "first only" was a Simple hole (a04, a05) | f8014efa 10-28 (GUI, counterbore on cbd.noshape, default panel: row "Ø 6.35 mm ↧ 12 mm | ⌴…", tooltip "Ø 6.35 mm ↧ 12 mm | ⌴Ø 10 mm ↧ 5 mm [Hole] <- Sketch 2") |
| TAB1 Part Studio 2 on first open | Parts (1), 5 strip buttons | ✅ **fixed** (a01) |  |

## Re-run pass 15 (2026-10-22, CLI /tmp/qa5/noshape-cli-0521 = target-engine 05:21; main still aa75fdf)
CLI rows unchanged: SF2 8000/10 faces, SF5 2400 mm², SF6 −514.304543; SM1 8383.1217/20, hem 9254.2367/30, SM3 "Closed corners are not supported yet"; ADV2 5927.754632; EXP1 openNURBS. ✗ still.

## Re-run pass 18 (2026-10-22 ~10:00–11:00, CLI /tmp/qa5/noshape-cli-0811 = target-engine 08:11; main 3ce6952)
CLI rows unchanged: SF2 8000/10 faces, SF5 2400 mm², SF6 −514.304543; SM1 8383.1217/20, hem 9254.2367/30, SM3 "Closed corners are not supported yet"; ADV2 5927.754632; EXP1 openNURBS; SML1 loft Add "not supported yet". ✗ still.

## Re-run pass 21 (2026-10-23 ~17:30–18:30, app = /root/target-lead/debug/noshape 10-23 17:10 (main 88fbf8f), CLI = target-lead noshape-cli 10-23 13:45)
| Row | Ours | Status | Verified |
|---|---|---|---|
| ADV2 loft | 5927.754632 | ✗ still → routed kern |  |
| ADV4 Hole countersink rows | icon rows, values readable ("12.878 mm", "82 deg") (a12) | ✅ **fixed** | 64997436 10-28 (GUI: Inch › Countersink › Tapped: icon rows, 13.487 mm / 82 deg / 12 mm / 12 mm / 3 all fit, no overlap; 20 tpi (UNC), ☐ Thread class = live dom qa5-adv-hole-tapped) |
| Hole Tapped defaults | Straight tap, 1/4, **20 tpi (UNC)**, Fastener fit None, ☐ Thread class (live ~8515 ☐) (a15); an old doc's stored Threads/inch shows blank (legacy value) | ✅ |  |
| ADV1, ADV3, ADV6, ADV7, ADV8, TAB1 | fixed in earlier passes; ADV5, ADV12 withdrawn | ✅ |  |

## Re-run pass 21 (qa5, 2026-10-23, main 40e23871 app /root/target-lead/debug/noshape)
| PS2 / TAB1 | imported tab shows its parts as soon as it opens | ✅ fixed |

## Re-run pass 22 (qa5, 2026-10-24, agent/qa5 on main 037298c3)
| Row | Ours now | Status | Verified |
|---|---|---|---|
| 2.3 / 4.7 Sketch row into a curve field | Rib: clicking the "Line sketch" row fills Sketch profiles with **"Edges of Line sketch"** (live "Edges of Rib sketch", behaviours ~983); Sweep path the same | 🔧 fixed (qa5) | 64997436 10-28 (GUI: the Sketch 3 row fills 'Edges of Sketch 3') |
| 3.6c Hole row named by the callout | hole.fs "Feature Name Template": "#featureName" → live names it by the callout too | ✅ | f9185639 10-28 (wf-block row "Ø 6.35 mm ↧ 12 mm") |
| 4.9 Wrap default tab | wrap.fs Creation type Default SURFACE | ✅ | f9185639 10-28 (dialog fields) |
| 1.7 point vs region priority | no live evidence yet | ? |  |
