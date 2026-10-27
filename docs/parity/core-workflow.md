# Core workflow: release checklist (a CAD user's session, step by step against live Onshape)

Re-run before every release, on a fresh profile (`HOME=<empty dir>`, no arguments → start window), at the
window's default size, with the keyboard and mouse the way a user works (no API, no demo documents). For each
step compare with live Onshape (`/root/onshape-research/live/behaviours.md`; section names below) and record
keys, focus, units, feedback, errors and crashes. Status: ✅ matches live, ✗ differs (finding #), ? not
verified against live yet, – not run.

Tools: `/root/qa4-scratch/cwstart.sh BIN` (fresh app on :110), `/root/qa4-scratch/s.sh NAME "xdotool steps"
[crop]` (steps + screenshot to /tmp/qa4/cw). Watch out: a chip added to a dialog field moves the fields under
it, so re-take positions after every selection.

**Last run: 2026-10-21, qa4**, shared-tree builds of 07:26 (steps 1–5) and 08:15 (steps 5–7), Linux/Xvfb
1600×1000 (window 1440×900). Findings F1–F27 are listed at the end with owners.

## 1. New document (live: "New document")

| # | Step | Live | Status | Verified |
|---|---|---|---|---|
| 1.1 | Start window › Create › Document… | Dialog with the name "Untitled document" selected | ✅ | 0.4.10+ / 1c853f78 10-27 |
| 1.2 | Type a name, Enter | Creates and opens it | ✅ | 0.4.10+ / 1c853f78 10-27 |
| 1.3 | New document contents | Part Studio 1 (active) + Assembly 1; "Features (4)", Default geometry, rollback bar under Right, "Parts (0)"; isometric | ✅ | 0.4.10+ / 1c853f78 10-27 |
| 1.4 | Start window footer version | (ours) should read the release version | ✅ re-checked 10-23 on main 88fbf8f5: footer "(0.4.6.88fbf8f5fa11)" = release version + commit (was ✗ F27) |  |

## 2. Sketch on Top (live: "Sketch mode", "Input behaviour" › G)

| # | Step | Live | Status | Verified |
|---|---|---|---|---|
| 2.1 | Sketch, then click Top in the list | Sketch 1 dialog, plane "Top plane", no tool active, view not rotated | ✅ | 0.4.10+ / 1c853f78 10-27 |
| 2.2 | Sketch dialog defaults | ☐ Disable imprinting, **☑ Show constraints**, ☐ Show expressions, ☑ Show errors | ✅ re-checked 10-23 (main 88fbf8f5): ☑ Show constraints, ☑ Show errors (was ✗ F1) | 0.4.10+ / 1c853f78 10-27 |
| 2.3 | N | Animated view normal to the plane | ✅ | 0.4.10+ / 1c853f78 10-27 |
| 2.4 | L, click, click, click (chain) | Previous segment's length boxed, live length in blue | ✅ | 0.4.10+ / 1c853f78 10-27 |
| 2.5 | Enter mid-chain | The boxed length becomes a driving dimension; the tool stays | ✅ | 0.4.10+ / 1c853f78 10-27 |
| 2.6 | Esc | One Esc ends the chain and exits the tool | ✅ | 0.4.10+ / 1c853f78 10-27 |
| 2.7 | G (corner rectangle), C (circle), A (3-point arc) | As live | ✅ | 0.4.10+ / 1c853f78 10-27 |
| 2.8 | Select two lines, E | Equal; glyphs shown | ✅ | 0.4.10+ / 1c853f78 10-27 |
| 2.9 | D, pick circle, place | Value box opens with the full value selected ("20.479094 mm") | ✅ | 0.4.10+ / 1c853f78 10-27 |
| 2.10 | Bare "25" + Enter | Ø25, box closes | ✅ (but see F3) | 0.4.10+ / 1c853f78 10-27 |
| 2.11 | Double-click the dimension, "1 in" + Enter | Ø25.4, tan background | ✅ (DIM-AC-ENTER fixed 189663bf) | 0.4.10+ / f16b31c6 10-27 |
| 2.12 | "10+5" + Enter | Label "ƒ 15" (expression prefix) | ✅ re-checked 10-23: "ƒ 15" (was ✗ F4) | 0.4.10+ / 1c853f78 10-27 |
| 2.13 | Re-open it | Box shows "(10+5) mm" | ✅ re-checked 10-23: "(10+5) mm" (was ✗ F5) | 0.4.10+ / 1c853f78 10-27 |
| 2.14 | Type, Esc | Reverts and blurs; box stays | ✅ | 0.4.10+ / 1c853f78 10-27 |
| 2.15 | Unknown "#w" + Enter | "Create variable" popup next to the field (Name "w") | ✅ re-checked 10-23: Create variable popup, Name "w" (was ✗ F6) | 0.4.10+ / 1c853f78 10-27 |
| 2.16 | "abc" + Enter | Pink field, red underline, ✓ disabled, box stays open, hover "Enter a valid expression." | ✅ re-checked 10-23: pink, red underline, ✓ disabled (was ✗ F7) | 0.4.10+ / 1c853f78 10-27 |
| 2.17 | Arc radius, point-point, line-point dimensions | Created, typed values applied | ✅ |  |
| 2.18 | Type the value immediately after the placing click | Applied (keys not lost) | ✅ (checked with 20 ms steps) |  |
| 2.19 | Enter with no tool active | Does **not** accept the sketch (live sketch/311) | ✅ (see F2) | 0.4.10+ / 1c853f78 10-27 |
| 2.20 | ✓ | Sketch 1 in the list, under-defined badge | ✅ | 0.4.10+ / 1c853f78 10-27 |
| 2.21 | Dimension crash (user report, macOS) | – | not reproduced on Linux (all dimension kinds above) |  |

## 3. Features (live: "Part Studio: features, selection, menus", "Feature dialog basics", "Input behaviour")

| # | Step | Live | Status | Verified |
|---|---|---|---|---|
| 3.1 | Shift+E, click the rectangle region | "Face of Sketch 1", 25 mm default, Part 1 preview | ✅ | 0.4.10+ / 1c853f78 10-27 |
| 3.2 | Depth: 1st click / "30" Enter / Enter | Selects all / "30 mm" re-selected, dialog stays / commits; sketch hidden | ✅ | 0.4.10+ / 1c853f78 10-27 |
| 3.3 | Shift+7 | Isometric | ✅ | 0.4.10+ / 1c853f78 10-27 |
| 3.4 | Shift+F, two edges, "3" Enter Enter | Fillet 1 | ✅ | 0.4.10+ / 1c853f78 10-27 |
| 3.5 | Chamfer on the vertical edge meeting both 3 mm fillets: 5 mm, then 2 mm | 5 mm: **error "Distance of chamfer on face too large."** (live G46: Onshape refuses it too); 2 mm builds Onshape's 10-face shape | (F8 fixed per kern 10-22; re-run) |  |
| 3.6 | Shell, remove top face, 2.5 / 1 mm | Shells (2.5 mm: 13946.59 mm³, 21 faces, kern 10-22) | (F9 fixed per kern 10-22; re-run) |  |
| 3.7 | After Enter committed a value, Esc | Cancels the dialog (no pending edit) | ✅ | 0.4.10+ / 1c853f78 10-27 |
| 3.8 | Sketch points on the top face; Hole from the sketch | One chip "Vertices of Sketch 2"; one hole per point | ✅ re-checked 10-23: one chip "Vertices of Sketch 2", one hole per point (was ✗ F10) |  |
| 3.9 | Hole defaults and callout | Drilled 1/4 (6.35 mm), 12 mm blind, 118 deg; "Ø6.35 ↧ 12" callout; row "Ø 6.35 mm ↧ 12 mm" | ✅ |  |
| 3.10 | Linear pattern, Part type: Part 1, edge, 40 mm × 3 | Part 2/3, default colour rotation #a5a5a5, #3b61b4 | ✅ |  |
| 3.11 | Direction field, click near the edge's midpoint | Filtered to the edge ("Edge of Part 1") | ✅ re-checked 10-23: an edge/line, not a midpoint (was ✗ F11) |  |
| 3.12 | Linear pattern, Feature type: Hole, edge, 10 mm × 2 | Holes copied on Part 1 | ✅ re-checked 10-23: holes copied 10 mm along the edge, Part 1 intact (was ✗ F12 (Part 1 destroyed)) |  |
| 3.13 | Esc while a select's search box is open | Closes the list only | ✅ re-checked 10-23: list closes, dialog stays (was ✗ F13) | 0.4.10+ / 1c853f78 10-27 |
| 3.14 | Mirror, Part type: Part 3 about Right | Part 4 | ✅ |  |
| 3.15 | Shift+X while a feature dialog is open | ? | ✅ re-checked 10-23: Shift+X does nothing while the dialog is open (was ✗ F14 (Section view opens on top of the open Mirror dialog)) | 0.4.10+ / 1c853f78 10-27 |
| 3.16 | Boolean union Part 3 + Part 4 | Touching parts: one part. Parts that don't touch (live 10-22): **two unchanged parts, the row stays black with no error**, and while editing a blue info bubble **"Boolean resulted in no geometry change…"** (psb BOOL1) | ✅ touching; disjoint: re-run |  |

## 4. Edit, roll back, reorder, suppress, delete, undo (live: "Rollback bar", "Right-click menus")

| # | Step | Live | Status | Verified |
|---|---|---|---|---|
| 4.1 | Double-click Extrude 1, depth 25, Enter Enter | Rolled back while editing; model regenerates | ✅ re-checked 10-23: rolled back while editing; Part 1 kept (was ✗ F12 (Part 1 disappears silently)) | 0.4.10+ / 1c853f78 10-27 |
| 4.2 | Feature row right-click › Suppress | Struck-through italic; model updates | ✅ |  |
| 4.3 | Drag Fillet 1 below Chamfer 1 | Drop line + pill; reordered | ✅ (Fillet then fails: F8) |  |
| 4.4 | Ctrl+Z / Ctrl+Y / Ctrl+Z | Undo / redo the reorder | ✅ | 0.4.10+ / 1c853f78 10-27 |
| 4.5 | Click a row, ↑ ↑ / ↓ ↓ | Rollback bar moves; light blue while focused | ✅ |  |
| 4.6 | Select a feature row, Delete key | Deletes it | ✅ re-checked 10-23: Delete removes the feature, Ctrl+Z restores (was ✗ F15) |  |
| 4.7 | Right-click › Delete; Ctrl+Z | Deleted; restored | ✅ |  |
| 4.8 | Error header | At the default panel width live truncates to "Features ..." beside the error icon (part-studio/94, 570); a wider panel shows "Features (N)" | ✅ (F16 withdrawn) |  |

## 5. Variables, configuration, measure, mass properties

| # | Step | Live | Status | Verified |
|---|---|---|---|---|
| 5.1 | Variable (Plane ▾ dropdown): w, 20 | "#w = 20 mm" | ✅ | 0.4.10+ / 1c853f78 10-27 |
| 5.2 | Plane ▾ dropdown contents | … Variable, Query variable, **Composite part, Tag** | ✅ (v0.4.6 gate): list scrolls to Composite part / Tag (was ✗ F17 (last items cut off at the window bottom)) | 0.4.10+ / 1c853f78 10-27 |
| 5.3 | Drag the variable to the top; Extrude 1 depth "#w" | Parts 20 mm | ✅ | 0.4.10+ / 1c853f78 10-27 |
| 5.4 | Select a face / two faces | "Area: 856.366 mm²" / a distance | ✅ | 0.4.10+ / 1c853f78 10-27 |
| 5.5 | Mass properties, no material | ? centre of mass shown | ? F18 (centre of mass and inertia blank) |  |
| 5.6 | Material warning | Goes with the Mass properties dialog | ✅ (v0.4.6 gate): page toast ≈3.4 s, gone ≈8 s (= live) (was ✗ F19 (stays; later covers the sketch dialog's ✓)) |  |
| 5.7 | Configurations panel › Configure Part Studio; add row "Long" | List input, Default / Long, info box | ✅ |  |
| 5.8 | Right-click a feature parameter (Variable Value) | "Configure …" | ✅ re-checked 10-23: Depth › right-click › "Configure" (was ✗ F20 (only Cut/Copy/Paste/Select All)) |  |
| 5.9 | + Configure features | Yellow banner "Select … to configure [Done]" | ✅ (v0.4.6 gate): yellow "Select sketches or features to configure [Done]" (was ✗ F21 (no banner seen)) |  |
| 5.10 | Strip tooltip after the panel opens | Hides | ✅ re-checked 10-23: no tooltip left over the panel (was ✗ F22 (stays over the panel)) |  |
| 5.11 | ☰ › Workspace units… while a sketch is open; Length → Inch, ✓ | Units change; the sketch stays open; geometry unchanged | ✅ re-checked 10-23: the Workspace units dialog accepts the open sketch first (Sketch 3 kept), geometry unchanged; live order not captured (was ✗ F23, F24) |  |

## 6. Assembly and drawing (live: "Assembly", "Drawing")

| # | Step | Live | Status | Verified |
|---|---|---|---|---|
| 6.1 | Assembly tab, I | Insert dialog | ✅ | 0.4.10+ / 1c853f78 10-27 |
| 6.2 | Click Part 2, click; click Part 3, click | Inserted; **the first instance lands at the origin** | ✅ re-checked 10-23: first instance at the assembly origin (was ✗ F25) | 0.4.10+ / 1c853f78 10-27 |
| 6.3 | Instance filter "3" | Filters the list | ✅ |  |
| 6.4 | Fastened mate (M): two faces | Mate HUD, solves, "Fastened 1" | ✅ | 0.4.10+ / 1c853f78 10-27 |
| 6.5 | + › Create Drawing…, ANSI_A_MM, OK | Model picker, then Insert view following the cursor | ✅ | 0.4.10+ / 1c853f78 10-27 |
| 6.6 | Place front, right and top views; Esc | Projected views | ✅ | 0.4.10+ / 1c853f78 10-27 |
| 6.7 | Dimension tool, pick an edge, place | Dimension "17"; tool stays active | ✅ (two-edge distance not retried) | 0.4.10+ / 1c853f78 10-27 |

## 7. Export, save, reopen

| # | Step | Live | Status | Verified |
|---|---|---|---|---|
| 7.1 | Part Studio tab › Export…, STEP | Dialog (Parasolid default), STEP options, file written | ✅ (TAB-EXPORT-MISSING fixed dea57d5c) (3 solids, mm) | 0.4.10+ / f16b31c6 10-27 |
| 7.2 | STEP contents | Products named as in live's export | ✅ (v0.4.6 gate): flat products Part 1…n, AP242 header (was ✗ F26) |  |
| 7.3 | Quit, start, Recently opened, open | Thumbnail, all tabs and features back | ✅ (single click selects; double click opens: ? vs live) | 0.4.10+ / 1c853f78 10-27 |

## 8. Context menus: every item clicked once (added 10-22 after qa5's Parts-list Rename crash on aa75fdf)

Each row: right-click the row, click the item once, check the result, then undo / Esc / close so the next item starts
from the same state. **Any crash, hang or console panic fails the gate.** Items that open a dialog or panel: it opens
and closes again with ✕/Esc/Cancel. Expected results follow live (behaviours.md "Right-click menus").

**8.1 Feature row, sketch (Sketch 1)**
| # | Item | Expected | Status | Verified |
|---|---|---|---|---|
| 8.1.1 | Rename | inline name field, all selected; Esc reverts | – |  |
| 8.1.2 | Edit… | Sketch 1 opens for editing | – |  |
| 8.1.3 | Copy sketch | copied (no visible change; Paste in another sketch pastes it) | – |  |
| 8.1.4 | Show dimensions | the sketch's dimensions show in the graphics | – |  |
| 8.1.5 | Add selection to folder… | Folder name popup beside the row | – |  |
| 8.1.6 | Hide / Show | the sketch hides / shows; eye toggles | – |  |
| 8.1.7 | Create Drawing of Sketch 1… | Create Drawing dialog | – |  |
| 8.1.8 | Export as DXF/DWG… | export dialog | – |  |
| 8.1.9 | Suppress / Unsuppress | struck-through italic / back | – |  |
| 8.1.10 | Dynamic suppression ▸ Add expression… | expression popup | – |  |
| 8.1.11 | Zoom to selection | view zooms to the sketch | – |  |
| 8.1.12 | View normal to | view turns normal to its plane | – |  |
| 8.1.13 | Show dependencies… | "Dependencies of Sketch 1" panel | – |  |
| 8.1.14 | Roll to here | rollback bar moves under Sketch 1 | – |  |
| 8.1.15 | Edit sketch appearance… | appearance editor | – |  |
| 8.1.16 | Delete | deleted (Ctrl+Z restores) | – |  |

**8.2 Feature row, other feature (Extrude 1)**
| # | Item | Expected | Status | Verified |
|---|---|---|---|---|
| 8.2.1 | Rename | inline name field; Esc reverts | – |  |
| 8.2.2 | Edit… | Extrude 1 dialog | – |  |
| 8.2.3 | Edit Sketch 1… | Sketch 1 opens | – |  |
| 8.2.4 | Show dimensions | dimensions show | – |  |
| 8.2.5 | Add selection to folder… | Folder name popup | – |  |
| 8.2.6 | Suppress / Unsuppress | struck-through / back | – |  |
| 8.2.7 | Dynamic suppression ▸ Add expression… | expression popup | – |  |
| 8.2.8 | Zoom to selection | zooms | – |  |
| 8.2.9 | Show dependencies… | dependencies panel | – |  |
| 8.2.10 | Roll to here | bar under Extrude 1 | – |  |
| 8.2.11 | Add appearance to feature… | appearance editor | – |  |
| 8.2.12 | Delete | deleted (Ctrl+Z restores) | – |  |

**8.3 Several feature rows (Shift-click two)**
| # | Item | Expected | Status | Verified |
|---|---|---|---|---|
| 8.3.1 | Add selection to folder… | Folder name popup | – |  |
| 8.3.2 | Section view… | Section view dialog | – |  |
| 8.3.3 | Suppress | both struck-through | – |  |
| 8.3.4 | Dynamic suppression ▸ | submenu | – |  |
| 8.3.5 | Clear selection | rows unselected | – |  |
| 8.3.6 | Edit sketch appearance… / Add appearance to feature… | appearance editor | – |  |
| 8.3.7 | Delete | both deleted (Ctrl+Z restores) | – |  |

**8.4 Default geometry: plane (Top) and Origin**
| # | Item | Expected | Status | Verified |
|---|---|---|---|---|
| 8.4.1 | Top › Rename | inline field; Esc reverts | – |  |
| 8.4.2 | Top › New sketch… | Sketch dialog on Top | – |  |
| 8.4.3 | Top › Offset plane… | Plane dialog, Offset, Top picked | – |  |
| 8.4.4 | Top › Size to Part Studio | plane resizes to the parts | – |  |
| 8.4.5 | Top › Hide / Show | plane hides / shows | – |  |
| 8.4.6 | Top › Hide other planes | Front, Right hide | – |  |
| 8.4.7 | Top › Hide all planes | all planes hide | – |  |
| 8.4.8 | Top › Zoom to selection | zooms | – |  |
| 8.4.9 | Top › View normal to | view normal to Top | – |  |
| 8.4.10 | Top › Show dependencies… | dependencies panel | – |  |
| 8.4.11 | Origin › Rename / Hide / Show dependencies… | as above | – |  |

**8.5 Folder row**
| # | Item | Expected | Status | Verified |
|---|---|---|---|---|
| 8.5.1 | Expand / Collapse | folder opens / closes | – |  |
| 8.5.2 | Rename | inline field; Esc reverts | – |  |
| 8.5.3 | Suppress / Unsuppress | its features struck-through / back | – |  |
| 8.5.4 | Expand all / Collapse all | all folders open / close | – |  |
| 8.5.5 | Unpack folder | folder removed, features stay | – |  |

**8.6 Parts list row (Part 1)**
| # | Item | Expected | Status | Verified |
|---|---|---|---|---|
| 8.6.1 | Rename | inline white field; Esc reverts (**crashed on aa75fdf**, qa5) | – |  |
| 8.6.2 | Properties… | Properties dialog (Name focused) | – |  |
| 8.6.3 | Assign material… | material panel | – |  |
| 8.6.4 | Edit appearance… | appearance editor | – |  |
| 8.6.5 | Copy here… | Transform dialog, Translate by XYZ, ☑ Copy part | – |  |
| 8.6.6 | Copy Part 1 | copied (no toast) | – |  |
| 8.6.7 | Create Drawing of Part 1… | Create Drawing dialog | – |  |
| 8.6.8 | Export… | Export dialog | – |  |
| 8.6.9 | Release… | Release dialog | – |  |
| 8.6.10 | Hide / Show | part hides / shows | – |  |
| 8.6.11 | Isolate… | Isolate panel | – |  |
| 8.6.12 | Make transparent… | part translucent | – |  |
| 8.6.13 | Zoom to selection | zooms | – |  |
| 8.6.14 | Delete… | Delete part feature (Ctrl+Z restores) | – |  |

**8.7 Parts list section header ("Parts (N)")**
| # | Item | Expected | Status | Verified |
|---|---|---|---|---|
| 8.7.1 | Show / Hide parts list properties | properties columns show / hide | – |  |
| 8.7.2 | Hide all parts | all parts hide | – |  |

## Findings (2026-10-21 run; owner per item)

| # | Finding | Repro | Owner |
|---|---|---|---|
| F1 | Sketch dialog opens with **Show constraints unchecked** on a fresh profile (live: on) | new doc, Sketch on Top | psb (sketch) |
| F2 | Live: **Enter with no tool active does not accept a sketch** (sketch/311); parity/sketch.md 1.8 says Enter accepts. Check before shipping the "Enter accepts the sketch" fix | – | psb / lead |
| F3 | Bare and expression values typed into **sketch dimensions are stored without a unit**: switching Workspace units to inch re-solves Sketch 1 with huge entities, moves the parts and breaks the Hole (red) and the parts count (3 → 4). Live appends the unit on commit, so a units change never changes geometry. Likely the "sketch dimensions without units" report | mm doc; dims "25", "10+5", "30", "45"; ☰ › Workspace units › Inch | psb |
| F4 | Expression-driven sketch dimension label has no **ƒ prefix** (live "ƒ 15") | dim "10+5" | psb |
| F5 | Re-opened expression dimension shows "10+5", live "(10+5) mm" | same | ui (shared numeric field) / psb |
| F6 | Unknown **#w** in a dimension box: nothing happens; live opens the "Create variable" popup | dim, "#w" Enter | psb |
| F7 | Invalid "abc" in a dimension box: no pink/red state, ✓ stays enabled, and a **raw parser toast** "unknown identifier `abc` (variables are written `#abc`) at 0..3" appears (and stays after the sketch closes). Live: field invalid state + hover "Enter a valid expression." | dim, "abc" Enter | psb (sketch) / ui (field states) |
| F8 | Chamfer 5 mm on the vertical edge meeting two 3 mm fillets fails ("Failed to chamfer selections"); Fillet placed after a chamfer fails too. Needs a live comparison | rectangle extruded 30 mm, fillet two top edges 3 mm, chamfer the front vertical edge between them | kern |
| F9 | **Shell fails** on that box (top face removed, 2.5 mm and 1 mm): "Could not shell part with selections" | after F8's 2 mm chamfer | kern |
| F10 | Hole from a sketch: three "Sketch 2 point" chips instead of one "Vertices of Sketch 2", and the **projected origin** the vertical inference referenced counts as a point, so a third hole is placed off the part | sketch on top face, 2 points (2nd vertically aligned with the origin) | psb (hole) / skb (sketch inference point) |
| F11 | A Direction field accepts "Midpoint of edge" (live filters to the edge) | Linear pattern › Direction, click near an edge midpoint | psb |
| F12 | **Feature pattern of the Hole destroys Part 1**: at creation Part 1 turns into a hollow box with standing cylinders; after editing Extrude 1 (30 → 25 mm) Part 1 vanishes with **no error** on any feature; suppressing the pattern brings it back | Linear pattern › Feature pattern › Hole 1, edge, 10 mm × 2 | psb (pattern) / kern |
| F13 | Esc while a select's search box is open (pattern type) cancels the **whole feature dialog** | Linear pattern, open "Part pattern" select, type, Esc | ui (select) |
| F14 | Shift+X opens Section view **on top of** an open feature dialog (two dialogs stacked at the same place) | Mirror open, Shift+X | psb |
| F15 | **Delete key** on a selected feature row does nothing (live deletes) | click Boolean 1, Delete | psb |
| F16 | **Withdrawn** (10-21): live also shows "Features ..." beside the error icon at the default width (part-studio/94, 570) | – | – |
| F17 | Plane ▾ toolbar dropdown is cut at the window bottom: Composite part and Tag unreachable at 900 px | 1440×900 window | ui |
| F18 | Mass properties without material: centre of mass and inertia blank (verify what live shows; request live) | Part 1, Mass properties | psb |
| F19 | "One or more parts do not have a material defined." stays after Mass properties closes and later covers the sketch dialog's ✓ | close Mass properties, edit a sketch with the Configurations panel docked | psb / ui (notice placement) |
| F20 | Right-click a feature parameter offers only Cut/Copy/Paste/Select All; live offers "Configure …" ("Right click … feature parameters to configure them") | configured studio, Variable › Value | psb (configurations) |
| F21 | "+ Configure features" shows no "Select … to configure [Done]" banner | Configurations panel | psb |
| F22 | The strip's "Configuration panel" tooltip stays over the panel after it opens | click the strip button | ui |
| F23 | **Workspace units › ✓ cancels the open sketch** ("Sketch 3 has been canceled. Restore") | in a sketch, ☰ › Workspace units…, change, ✓ | psb |
| F24 | Workspace units dialog opens over (and hides) the open sketch dialog | same | ui |
| F25 | First instance inserted into an empty assembly lands at the click point, not at the origin (live: at the origin) | new assembly, Insert, click part, click | skb |
| F26 | STEP export: an extra root `PRODUCT('SOLID')` and an Open CASCADE header/description; compare with live's STEP of a 3-part Part Studio | Export › STEP | kern |
| F27 | Start window footer says "noshape 0.1.0" | start window | ui2 |

## Re-run 2026-10-21 14:30–14:50 (qa4, build of 14:32, fresh profile, document "CW rerun")

| # | Result | Notes |
|---|---|---|
| F1 | ✅ fixed | Show constraints on by default (re-confirmed 10-21 16:42 on an empty HOME: ☑ Show constraints, ☑ Show errors) |
| F3 | ✅ fixed | mm → inch with the sketch's bare "40" and "10+5" dimensions: labels 1.575 / ƒ Ø0.591, geometry unchanged |
| F4 | ✅ fixed | "ƒ Ø15" |
| F5 | ✅ fixed | re-opened box shows "(10+5) mm" |
| F6 | ✅ fixed | "#w" opens Create variable (Feature / Configuration, Length…, Name "w") |
| F7 | ✅ fixed | pink field, red underline, ✓ disabled, hover "Enter a valid expression.", no raw toast |
| F8 | ✗ still failing | chamfer 5 mm on the vertical edge next to a 3 mm fillet: red |
| F9 | ✗ still failing | Shell 2.5 mm, top face removed: red |
| F10 | ✅ fixed | one chip "Vertices of Sketch 2", two holes, no stray origin hole |
| F11 | ✅ fixed | click near the edge midpoint gives "Edge of Extrude 1" |
| F12 | ✅ fixed | Feature pattern of the Hole cuts real holes; Extrude 1 30 → 25 mm keeps Part 1 |
| F13 | ✅ fixed | Esc in the type select's search keeps the dialog |
| F14 | ✅ fixed | Shift+X does nothing while a feature dialog is open |
| F15 | ✅ fixed | Delete key deletes the selected feature (Ctrl+Z restores) |
| F17 | ✅ fixed | Plane ▾ dropdown scrolls; Composite part and Tag reachable |
| F18 | ? unchanged | Mass properties without material: centre of mass / inertia still blank (live not captured yet) |
| F19 | ✅ fixed (verify) | no lingering notice; the material notice no longer appears at all (check live) |
| F20 | ✅ fixed | Depth › right-click › Configure adds "Extrude 1 / Depth" column, field framed dashed amber |
| F21 | ✅ fixed | banner "Select sketches or features to configure [Done]", toolbar greyed |
| F22 | ✅ fixed | no stranded strip tooltip |
| F23 | ✅ fixed (verify) | Workspace units no longer cancels the sketch: it **accepts** it first (live behaviour not captured) |
| F24 | ✅ fixed | follows from F23 (no sketch dialog under it) |
| F25 | ✅ fixed | first instance placed at the assembly origin |
| F26 | ✗ still failing | two parts → PRODUCT('Part 1'), ('Part 2') and root ('SOLID'); header still "Open CASCADE Model" |
| F27 | ✗ still failing | footer now "(0.1.0)": still not the release version |
| F2, F16 | – | F2 is a note for psb/lead; F16 withdrawn |

New in this run:
| # | Finding | Repro | Owner |
|---|---|---|---|
| F28 | **Show errors** now opens **unchecked** in the Sketch dialog (live: ☑ Show errors) — regression from the F1 change? | new sketch | psb — **not a code regression** (10-21): the defaults are Show constraints ☑, Show errors ☑ (test `sketch_options_default_as_live`). The qa4 profile has a saved `/tmp/qa4/cwhome/.config/noshape/sketch-options.json` (08:22) with `"show_errors": false`. As in Onshape (UserSettingsService), accepting a sketch whose options changed saves them. Delete that file to test a fresh profile. |
| F29 | After an invalid value in a dimension box (abc + Enter) then Esc, Esc, sketch shortcuts (L) do nothing until a click in the graphics; the plain Esc path works | dim box, abc, Enter, Esc, Esc, L, click, click | psb — fixed (10-21): Esc on the box closes it in every state and gives the focus back to the Part Studio (sketch.rs dim box `capture_key_down`); test `esc_closes_the_dimension_box_then_the_tool` |

**F28 closed (10-21 16:42, qa4): not a bug.** The re-run's profile still had my own saved sketch options (`rm -rf HOME/*` keeps dotfiles); on an empty HOME both defaults are on. Fresh-profile runs now use `FRESH=1 cwstart.sh`, which deletes the whole HOME.


## v0.4.5 gate run: 5d28447 (target-lead build, files = 5d28447), FRESH=1 profile, 2026-10-21 17:05–17:20, qa4

**Result: FAIL** (1 blocking regression path + 3 known kernel/export fails). Everything else passes.

| Section | Rows | Result |
|---|---|---|
| 1 New document | 1.1–1.4 | ✅ (footer "(0.4.4.e4b90e209380)": the hash is the stale HEAD of /root/noshape-lead, not 5d28447; CI builds are fine) |
| 2 Sketch | 2.1–2.21 | ✅ all (fresh defaults ☑ Show constraints / ☑ Show errors; ƒ, (10+5) mm, #w popup, invalid state, F29 Esc path, Enter does not accept) |
| 3 Features | 3.1–3.4, 3.7–3.11, 3.13–3.15 | ✅ |
| | 3.5 chamfer 5 mm next to fillets (F8) | ✗ kern |
| | 3.6 shell 2.5 / 1 mm (F9) | ✗ kern |
| | 3.12 Feature pattern of the Hole **after a Part pattern** (direction = Part 1's bottom edge, 10 mm × 2) | ✗ **new path of F12**: no extra holes, Part 1's filleted top corner turns into twisted facets and a sliver **Part 4** is split off |
| | 3.16 Boolean union | ? (the mirrored part didn't touch Part 3 in this layout; committed without error, parts unchanged) |
| 4 Edit / history | 4.1 Extrude 1 30 → 25 mm | ✗ **Part 1 disappears with no error** (Parts 3) while Linear pattern 2 (the Feature pattern above) is active — data loss |
| | 4.2–4.8 | ✅ (suppressing Linear pattern 2 brings Part 1 back; reorder, undo/redo, rollback arrows, Delete key, menu Delete + undo). Note: plain clicks in the list add to the selection, so Delete removed the two selected rows (verify against live's list selection) |
| 5 Variables … units | 5.1–5.11 | ✅ (5.5 centre of mass blank = F18, live request open; 5.11 mm → inch changes only the Hole row's text) |
| 6 Assembly / drawing | 6.1–6.7 | ✅ (first instance at the origin; "Select a mate connector on a different instance" hint; coincident fastened faces show z-fighting lines: ? vs live) |
| 7 Export / reopen | 7.1, 7.3 | ✅ |
| | 7.2 STEP (F26) | ✗ kern: 4 parts → PRODUCT Part 1–4 plus **three** PRODUCT('SOLID'); header "Open CASCADE Model" |

Repro for the F12 path: new doc; sketch rectangle on Top, extrude 30; fillet two top edges 3 mm; chamfer the vertical edge 2 mm;
sketch 2 points on the top face, Hole; Linear pattern (Part) Part 1, 40 mm × 3; Linear pattern (Feature) Hole 1,
direction = Part 1's bottom-right edge, 10 mm × 2 → sliver Part 4 + facets; then edit Extrude 1 to 25 mm → Part 1 gone.

**Live answers (10-21, behaviours.md "Mass properties without material + the material notice"):**
- **F18 closed, matches live:** without a material live also leaves Mass, Center of mass X/Y/Z and all nine inertia values **empty** (Override boxes disabled), Volume and Surface area filled, the Part chip in red text. Our panel is the same.
- **F19 still differs (psb / ui):** live shows a **page toast** (yellow warning bubble, top centre, "⚠ One or more parts do not have a material defined." ×; bg rgb(255,248,230), border rgb(252,218,127)) **≈3.6 s after** the panel opens with a part lacking material, **auto-hides ≈4.6 s later**, and only **once per page load** (re-opening in the same session shows none; closing the panel doesn't touch it). Ours: first run showed it and it never auto-hid; on the 14:32 build and 5d28447 it never appears.

## v0.4.5 gate re-run: 5bd50b2 (target-lead build, files = 5bd50b2), FRESH=1 profile, 2026-10-21 23:35 – 10-22 00:15, qa4

**Result: FAIL** on F8 and F9 (kern). F12, F19 and F26 now pass. Four new findings (F30–F33) and one palette finding (F34), none blocking on its own.

| Section | Rows | Result |
|---|---|---|
| 1 New document | 1.1–1.4 | ✅ (footer hash again the stale /root/noshape-lead HEAD e4b90e2) |
| 2 Sketch | 2.1–2.20 | ✅ all (☑ Show constraints / ☑ Show errors; chain, Enter mid-chain, Esc; G/C/A; E; D value box selected; 25; 1 in → Ø25.4 tan; ƒ Ø15; (10+5) mm; Esc reverts + blurs; #w → Create variable; abc → pink, ✓ disabled, "Enter a valid expression."; R20 / 40 typed right after the placing click; Enter with no tool keeps the sketch) |
| 3 Features | 3.1–3.4, 3.7–3.11, 3.13–3.15 | ✅ |
| | 3.5 chamfer 5 mm next to the two 3 mm fillets (F8) | ✗ kern (red; 2 mm works) |
| | 3.6 shell 2.5 mm, top face removed (F9) | ✗ kern (red) |
| | 3.10 count field | ✗ **F30** (Enter kept the dialog open; see below) |
| | 3.12 Feature pattern of Hole 1 after the Part pattern, Part 1's bottom-right edge, 10 mm × 2 (F12 path) | ✅ real holes, Parts (3), no sliver or facets |
| | 3.16 Boolean union of the disjoint Part 3 + mirrored Part 4 | ? (no error, parts unchanged; same as last run) |
| 4 Edit / history | 4.1 Extrude 1 → 25 mm with the Feature pattern active (F12) | ✅ rolled back while editing; Part 1 kept |
| | 4.2, 4.4–4.8 | ✅ |
| | 4.3 drag Fillet 1 down onto the Sketch 2 row | ✗ **F33**: the drop line is drawn above Sketch 2, but the feature lands **below** Sketch 2 (dragging up lands where the line is) |
| 5 Variables … units | 5.1–5.11 | ✅ (5.6 F19 now as live: toast ≈3.4 s after opening, gone by ≈8 s, not shown again on reopening; 5.11 accepts the sketch first, live not captured) |
| 6 Assembly / drawing | 6.1–6.7 | ✅ |
| 7 Export / reopen | 7.1–7.3 | ✅ (7.2 F26: flat products Part 4…1, id = name = description, no PRODUCT('SOLID'), AP242 header, METRE, one colour per part = live G47; originating system 'NOSHAPE 0.4.4' vs live 'ONSHAPE BY PTC INC') |

New findings (10-22):
| # | Finding | Repro | Owner |
|---|---|---|---|
| F30 | **Instance count + Enter** in Linear pattern keeps the dialog open and re-selects the value (the next key replaced it). Live ("Input behaviour" › Enter): Instance count Enter **accepts the whole feature dialog** | Linear pattern › Instance count, click, "3", Enter | psb |
| F31 | Fillet ▾ toolbar dropdown (Fillet, Face blend) shows a **scrollbar** and scrolls 2 px; live is a plain two-row list | click the ▾ next to Fillet | ui |
| F32 | A **Create variable popup** left open (from "#wabc" in a dimension box) **outlives the box**: Esc closes the box, but re-opening the box shows the old popup again | dim box "#wabc" Enter, Esc Esc, click away, double-click the dimension | psb (sketch dim box) |
| F33 | Feature drag **down**: the drop line is drawn at the hovered row's top, but `interaction::reordered` puts the feature **after** that row; line and result must agree (live capture requested in REQUESTS.md, qa4 10-21 23:59) | drag Fillet 1 onto the row two below | psb |
| F34 | **Default part colours** (`regen/display.rs PART_COLORS`): ours #9dcfed, #9a9a9a, #4e6fb7, #ebebeb, #c8e6f5, #fd9c28, #787878, #ebb115; live (behaviours.md "Default part colour rotation", dom/default-part-colors.txt, and live's STEP colours) #9dcfed, **#a5a5a5, #3b61b4, #eaeaea, #c4e2f3, #f88701, #7f7f7f, #fab601**. (Row 3.10 was passed by eye earlier.) | Linear pattern of a part ×3 | ui2 |

Routing (lead, 10-22): F30, F32, F33 → psb; F31 → ui; F34 → ui2. 3.16: live request filed (REQUESTS.md, qa4 10-22).

## v0.4.5 gate run: aa75fdf (target-lead build 05:03, files = aa75fdf), FRESH=1 profile, 2026-10-22 05:10–05:35, qa4

**Result: PASS.** Every row matches its expected result. One note: the version footer reads "0.4.4.5bd50b21fc7b" (/root/noshape-lead's HEAD wasn't advanced to aa75fdf).

| Section | Rows | Result |
|---|---|---|
| 1 New document | 1.1–1.4 | ✅ (footer hash stale, see above) |
| 2 Sketch | 2.1–2.20 | ✅ (2.17/2.18: in the first pass the R20 / 40 dimensions after the F32 check didn't get created; not reproducible: re-checked in a second sketch, the same sequence (#z popup, Esc Esc, click away, D, arc, 20 Enter) gives R20 and Ø20) |
| | F32 (Create variable popup left open, box closed with Esc, re-opened) | ✅ fixed: no stale popup |
| 3 Features | 3.1–3.4, 3.7–3.11, 3.13–3.15 | ✅ |
| | 3.5 chamfer 5 mm (F8) | ✅ red, tooltip "Chamfer 1 did not regenerate properly: Distance of chamfer on face too large." (= live G46); 2 mm builds |
| | 3.6 shell 2.5 mm, top face removed (F9) | ✅ builds (this box: 9973.193 mm³; kern's 13946.59 mm³ is for its own sketch size) |
| | 3.10 count + Enter (F30) | ✅ fixed: Enter in Instance count accepts the dialog |
| | 3.12 Feature pattern of the Hole after the Part pattern (F12) | ✅ real holes, Parts (3) |
| | 3.16 Boolean union of the disjoint Part 3 + Part 4 | ✅ blue note "Boolean resulted in no geometry change. The parts either do not intersect or are totally contained.", Parts (4) unchanged, row black after ✓ |
| 4 Edit / history | 4.1 Extrude 1 → 25 mm (F12) | ✅ rolled back while editing; Part 1 kept |
| | 4.3 drag Fillet 1 down onto Sketch 2 (F33) | ✅ fixed: the line is drawn below Sketch 2 and the feature lands there (line and result agree; live capture of the side still requested) |
| | 4.2, 4.4–4.8 | ✅ |
| 5 Variables … units | 5.1–5.11 | ✅ (5.6 F19 checked on a fresh session at 7.3: toast ≈3.4 s after opening, gone by ≈8.2 s) |
| 6 Assembly / drawing | 6.1–6.7 | ✅ |
| 7 Export / reopen | 7.1–7.3 | ✅ (7.2: flat products Part 1–4, no PRODUCT('SOLID'); **F34 fixed**: COLOUR_RGB 0.6157/0.8118/0.9294, 0.6471 (#a5a5a5), 0.2314/0.3804/0.7059 (#3b61b4), 0.9176 (#eaeaea) = live) |

Open items (not blocking): F31 (Fillet ▾ scrollbar, ui) not re-checked; drawing panel fields (asb).

## Section 8 dry run on aa75fdf (2026-10-22 06:40–07:10, qa4; /root/qa4-scratch/menurun.sh, one fresh demo app per item)
73 items (8.1–8.7 on the demo Part Studio: Sketch 1, Extrude 1, multi-select, Top, Origin, folder, Part 1, "Parts (1)"):
**one crash: 8.6.1 Parts list › Rename** (panic at vendor/gpui-pre/src/elements/div.rs:843, "hover style already set"; qa5's
find, ui fixing). The other 72 open what they should (screenshots /tmp/qa4/cm/aa-*.png). Notes:
- 8.1.4 / 8.2.4 Show dimensions: the part turns see-through (live too), but the demo's Sketch 1 has no dimensions: check
  the labels on the gate document (live: the sketch's dimensions drawn in 3D on its plane).
- 8.3.6: with Sketch 1 + Extrude 1 selected ours lists only "Add appearance to feature…"; live lists "Edit sketch
  appearance…, Add appearance to feature…" (psb, minor).
- Dynamic suppression ▸ is a submenu: hover it, then click "Add expression…" (opens the "Dynamic suppression" dialog).
Re-run the whole section on the build with ui's fix (`menurun.sh BIN menus-ps.txt TAG`, then menus-fm.txt).

## v0.4.5 candidate /tmp/lead/noshape-v045 (aa75fdf + CRASH1 fix, branch release/v0.4.5), FRESH=1, 2026-10-22 07:20–07:50, qa4
**PASS** (lead tags v0.4.5). Footer still 0.4.4 (version bump after this build).
- 8.6.1 Parts list › Rename: white inline field, "Part 1" selected, no hover band while the pointer is on the row; "Bracket"
  + Enter renames; no crash (crashed on aa75fdf).
- Section 8, all 73 items (menurun.sh, fresh demo app per item): no crash; screenshots show each item's dialog / action.
- Sections 1–2 smoke: New document, Sketch on Top (☑ Show constraints / ☑ Show errors), L chain + Enter dimension, G,
  D value box selected, 25 Enter, ✓ → Sketch 1.

## v0.4.6 gate: main 8da7e78 (target-lead build 14:40, embeds 8da7e78248bc), FRESH=1, 2026-10-22 15:20–16:40, qa4
**Result: FAIL on (b)** (the failed Extrude's input region isn't drawn red). Everything else passes.

| Row | Result |
|---|---|
| Sections 1–7 | ✅ (as the aa75fdf run; 3.5 5 mm chamfer red, 2 mm builds; 3.6 shell builds; 3.12/4.1 F12; 3.16 blue note; F19 toast ≈3.4 s / gone ≈8.2 s; STEP 4 products, live colours). 2.17: my script's R20/40 after the #w-popup + "abc" steps didn't get created (twice now); the same steps by hand create them every time, so not counted. |
| Section 8 (73 context-menu items) | ✅ all alive, incl. 8.6.1 Parts list › Rename |
| (a) "lp wall over" Feature pattern (CLI-built, live G-URGENT model) | ✅ Reapply off: row and dialog title red, tooltip exactly `Linear pattern 1 did not regenerate properly: Could not create all instances as entered. Try selecting "Reapply features" option.`, ✓ enabled green, seed wall + direction drawn red, no second wall. Reapply on: title black, orange preview, two walls, volume **20042 mm³** (= live). |
| (b) failed Extrude (Up to face on the block's parallel x=0 face) | ✗ title / row red, tooltip exactly `Extrude 2 did not regenerate properly: Failed to extrude selections, check input.`, ✓ enabled, no ghost; the up-to face's edge is red, **but the input sketch region (10×10 square) isn't drawn at all**: live draws it as a red outline with a light red fill. |
| (c) Parts list › Rename | ✅ white field, all selected, no hover band; "Bracket" + Enter renames; no crash |
| (d) import g48-two.step | ✅ names as live: Part 1 = G48 body (1.379 in³ ≈ 22597 mm³), Part 2 = the 10 mm cube. **New finding:** selecting Part 1 highlights the cube too, selecting Part 2 highlights the cube plus a face of the G48 body (selection highlight crosses the imported parts); the Parts list shows Part 2 above Part 1 (live order not captured). |
| (e) noshape-cli live mode (CLI built from 8da7e78) | ✅ with failext.noshape open in the app: sketch_create / sketch_draw / feature_add extrude via the CLI appear in the window at once (Sketch 3, Extrude 3, Part 2); Ctrl+Z in the app undoes the extrude. |

Models: /root/qa4-scratch/mk-models.py CLI DIR (lpwall.noshape, failext.noshape).

## v0.4.6 candidate /tmp/lead/noshape-v046 + noshape-cli-v046 (main 8da7e78, built 17:06, footer `0.4.4.8da7e78248bc`), FRESH=1, 2026-10-22 17:00–17:55, qa4
**Result: FAIL on (b)** (same as the target-lead run). Everything else passes. New finding R1 (below).

| Row | Result |
|---|---|
| Sections 1–4 | ✅ (2.17 scripted miss as before; by hand OK) |
| Section 5 | ✅ #w = 20 drives Extrude 1; area 688.210 mm² / "Parallel dist 0.000 mm"; mass props Part 1 17073.686 mm³ (centre of mass still blank, F18); config row "Long". **Now fixed:** 5.2 Plane ▾ list scrolls to Composite part / Tag (F17); 5.9 yellow "Select sketches or features to configure [Done]" banner (F21). |
| Section 6 | ✅ Insert (configuration picker shown), filter "3", Fastened 1 solves, Drawing ANSI_A_MM, 3 views, dimension "32.81". ? both inserted instances show the fixed icon and Fastened 1 is drawn grey (live not checked). |
| Section 7 | ✅ STEP: 4 MANIFOLD_SOLID_BREP, products Part 1–4; quit / Recently opened (thumbnail) / reopen: all tabs, features, Configurations panel back. |
| Section 8 (73 items, menurun.sh, fresh demo app each) | ✅ all ALIVE, every screenshot shows its dialog / action |
| (a) lp wall over | ✅ red row + title, tooltip exact, ✓ green, seed wall and Right plane red; Reapply on: black title, two walls, **20042 mm³** |
| (b) failed Extrude | ✗ title / row red, tooltip exact (`Extrude 2 did not regenerate properly: Failed to extrude selections, check input.`), ✓ enabled, up-to face edge red; **the input region (x 50–60, y 10–20 on Top) is not drawn** (checked iso and Top view, zoomed) |
| (c) Parts list › Rename | ✅ white field, "Part 1" selected; "Bracket" + Enter; no crash |
| (d) import g48-two.step | ✅ names (Part 1 = G48 body, Part 2 = cube); still: selection highlight crosses parts (Part 1 lights the cube too; Part 2 lights the cube + a G48 face), list order Part 2 above Part 1 |
| (e) CLI live mode | ✅ sketch_create / sketch_draw / feature_add on the open failext.noshape appear at once (Sketch 3, Extrude 3, Part 2); Ctrl+Z in the app removes Extrude 3 |

**R1 (new, render):** select two default planes (Top + Right, or Front + Right; feature list or viewport) → every part is drawn
with black jagged halos and see-through holes (hole cylinders visible through the top faces), plus the green dashed
intersection line; one plane selected draws normally; clearing the selection restores it. Reproduced 3×. **Not a
regression:** /tmp/lead/noshape-v045 shows the same with Top + Right on the demo plate (black halos, see-through holes,
banding on the side faces).

## v0.4.6 final candidate /tmp/lead/noshape-v046b + noshape-cli-v046b (release/v0.4.6 751df66 = 8da7e78 + version, toolchain pin, CI fix, BRepMesh planar meshing), FRESH=1, 2026-10-23 01:38–02:40, qa4
**Result: FAIL on (b) only** (unchanged from 8da7e78; the fix isn't on the release branch). Everything else passes.

| Row | Result |
|---|---|
| Display (new planar meshing) | ✅ demo plate shaded with edges / shaded without edges, zoomed on holes + fillets; Flange (annular planar faces with bolt-circle holes); tests/perf/big-plate (one planar face, hundreds of holes): no cracks, slivers or stray triangles; edges right |
| (a) lp wall over | ✅ red row + title, tooltip exact, ✓ green, seed wall + Right plane red; Reapply on: two walls, 20042 mm³ |
| (b) failed Extrude | ✗ as before: red title / row, tooltip exact, ✓ enabled, up-to face edge red; input region not drawn |
| (e) CLI live mode | ✅ sketch_create / sketch_draw / feature_add on the open failext.noshape appear at once; Ctrl+Z removes Extrude 3 |
| Section 8 (73 items) | ✅ all ALIVE; screenshots as the 8da7e78 run (montage /tmp/qa4/cm/mont46c.png) |

Note: opening a .noshape whose stale `-wal` / `-shm` (from an older file of the same name) sit beside it panics at start
("could not open document: unknown branch …", main.rs:348 `expect`) instead of an error dialog; same code on main.

## v0.4.6 candidate c /tmp/lead/noshape-v046c (release/v0.4.6 + psb 9b27922), 2026-10-23 02:50–03:10, qa4
**PASS** on the two rows asked for.
- (b) failed Extrude (Up to face, failext.noshape): the input region (10×10 square at x 50–60) is drawn with a red outline
  and a light red fill, as live part-studio/2028; title / row red, tooltip exact, ✓ enabled. Still different from 2028:
  live draws the region's **4 vertices as grey dots** and the **extrude direction arrow** on the region; noshape shows
  neither in the failed state (open, psb).
- (a) lp wall over (fresh CLI model): Reapply off red with the exact tooltip, seed wall + Right plane red; Reapply on two
  walls, 20042 mm³.
- Test note: a model accepted with Reapply on in an earlier run keeps it (file saved); rebuild the models for each run.

## v0.4.7 gate: release/v0.4.7 dd1ca034 (built in /root/wt/release, target-lead, 0 warnings; footer 0.4.7.dd1ca0347f4c), FRESH=1, 2026-10-23 13:55–15:30, qa4
**Result: FAIL on (a)** (Direction not red). (b) PASS to the lead's spec (one deviation from live noted), (c) PASS.
Screenshots: /root/qa4-scratch/gate-v047/.

| Row | Result |
|---|---|
| (a) failed Feature pattern, lp wall over, Reapply off, counts 2 / 5 / 10 | ✓ 1 / 4 / 9 ghosts; ghost edges #bd3039 drawn through the block; translucent red ghost fill (#7e2d32 on the overhang strips, live #904348 there); seed keeps its shading, orange outline. **✗ the Direction (block long edge) is drawn orange (#e8962a-ish, selection colour), not red; a Right-plane Direction is orange-tinted too.** Side by side with live 2259/2258/2260: a1-…png |
| (a) flush wall (wall flush with the block's long sides, 1 mm over the top; mk-flush.py) | ✓ fails "Could not create all instances…"; the long side face fills red from the stacked ghosts of the merged side face, red edges, ghost stack at the wall end, as the user's Onshape shot. Exact camera of the user's shot not reproduced (orbit too coarse). A wall flush with the *end* face pattern OK in noshape (not failing). |
| (b) projection | ✓ fresh Part Studio and reopened document (quit with perspective on) orthographic, Perspective view unticked; O in the graphics area: 0 px change; O in a sketch = Offset; camera menu Perspective view toggles with ✓; right-click › Set as default → new Part Studio opens perspective. **Deviation from live (lead's decision):** live has no Set as default and no persistence (behaviours.md "Projection"). Our perspective looks stronger than live's 45° (2268); not compared on equal geometry. |
| (c) failed Extrude (v0.4.6 b) | ✓ red region + light fill, tooltip exact, ✓ enabled; the direction arrow is now drawn (missing on v046c) |
| (c) smoke | ✓ new doc, rectangle sketch, Extrude, Fillet; quit / reopen; section 8: 73/73 ALIVE, screenshots as before |

Notes: Enter in Instance count accepts the dialog (Tab used); Xvfb :110 died once during the run (restarted).

## Re-check of the ✗ rows on main 88fbf8f5 (qa4, 2026-10-23 16:00–17:30, FRESH=1, binary /root/qa4-scratch/bin/noshape-main)
All 21 ✗ rows of sections 1–7 pass now (marked above). The history ✗ (line "4 Edit / history", the 10-21 F12 run) is fixed (4.1).
Open findings re-checked: F31 ✅ (Fillet ▾: plain two-row list), F33 ✅ (dragging Fillet 1 down onto the Hole row: drop line
under the Hole row, lands under it), F8/F9/F18 closed earlier. Still open, routed in docs/handoff/qa5-routing.md:
- IMP1 (kern): imported parts share face names → selection highlight crosses parts (g48-two: Part 1 lights the cube, Part 2
  lights the cube + two G48 faces). `importing.rs import_foreign` names each body with `new_face_names` → `Role::New(ordinal)`
  per body under one feature, so body 0 and body 1 both have New(0…5).
- IMP2 ✅ (api 10-24): Parts list after importing g48-two reads Part 2 above Part 1, as in live (the file's order).
- R1: fixed on main (833bfb0b), re-checked in the app: closed.
- PAT-DIR (psb, v0.4.7 gate): failed Feature pattern's Direction drawn orange (selection) instead of red.
- ASM-ESC: fixed on main (833bfb0b), re-checked in the app: closed.

## v0.4.7 re-gate: release/v0.4.7 30979bc1 (target-lead build 20:09, 0 warnings, footer 0.4.7.30979bc1aae4), FRESH=1, 2026-10-23 20:10–21:30, qa4
**Result: FAIL on (b)** (Zoom to fit in perspective); (a), (c), (d) PASS. Screenshots /root/qa4-scratch/gate-v047b/.

| Item | Result |
|---|---|
| (a) failed Feature pattern | ✅ Direction edge #bd3039, Right-plane Direction red; ×2/×5/×10 → 1/4/9 ghosts, red edges, seed orange outline. Ghost fill (planes hidden) (124,56,68) (112,60,68) (120,40,44) vs live 2258 (128,56,64) (104,60,68) (104,40,44) |
| (b) projection | ✅ right-click Perspective view / Highlight boundary edges: no menu; new Part Studio and reopened document orthographic; perspective per tab. ✗ **Zoom to fit in perspective shrinks the model** (live 2315: the eye moves closer, the model fills the view); our ortho fit is also tighter than live's 2313. Convergence right after ticking Perspective ≈ live 2314 (fins 25 by mk-fins.py, planes hidden) |
| (c) shading, Isometric, faces clear of planes | ✅ front (79,104,119) vs #4e6876; top (142,187,214) vs #8cb9d4; side (118,156,178) = live 2265. A translucent default plane over a coplanar face lightens it (lp wall front on the Front plane reads #788a96) |
| (d) regressions | ✅ failed Extrude red region / tooltip / ✓ enabled; section 8 73/73 ALIVE |

## v0.4.8 gate: release/v0.4.8 5668940b (= 36ed7c7d, tree 57d7290a; target-lead dev build 12:39, footer still 36ed7c7d), FRESH=1, 2026-10-24 12:40–14:00, qa4
**Result: FAIL on (a)** (delete mid-regen ~7–9 s), **(e)** (German tooltip title + Extrude dialog), **(f)** (dual dimension
line through the dual text), **(d)** (1000-pin orbit 19 fps vs 33 on v0.4.7, macOS CI); (b), (c), (g) PASS. Screenshots /root/qa4-scratch/gate-v048/.

| Item | Result |
|---|---|
| (a) user's fins (plate 810×20×10, 1×20×5 wall top + LP1 ×400, wall under + LP2 ×400; mk-fins2.py) | ✅ Reapply on 242000 mm³ / 3206 faces = live; Reapply off both ERROR with live's text, 162200 / 14 = live. Regen panel (dev build): LP1 2.07 s, LP2 5.34 s, Extrude 3 3.10 s (live 55 ms), total 10.60 s (live 9.27 / 24.00 / 33.36 s; same 2.6× ratio). ✗ **Delete LP2 while it regenerates**: row gone at once, spinner 7.0 s (Delete key 4 s into the regen) / 8.8 s (right-click › Delete 1 s in), end state right; Suppress LP2 returns to LP1 in 0.5 s. Open 20 s, undo of the delete 18 s |
| (b) flush wall W1 (live "psb flush W1"; mk-w1f.py) | ✅ count 2 OK 19700 mm³, 6 faces, x 1.5..100; 3/4/5 ERROR "Could not create all instances as entered. Try selecting "Reapply features" option.", 20100; dialog ghosts: whole block failed-red, nested end caps, Direction red, title/row red (live 2306) |
| (c) view | ✅ new Part Studio Trimetric, 150 mm planes; fit = live rule (planes 0.800 of the viewport height vs 0.799 computed, centred, w/h 1.050 vs 1.052); planes hidden: perspective + F moves closer, framing = live 2315 (x 1140–1382 vs 1155–1365, y 67–295 vs 67–297); "p" doesn't re-fit |
| (d) finer mesh | ✅ angle mesh default (9b67d84a; tests/perf/README still says chord); Ø50 cylinder silhouette smooth at fit. Orbit parts-1000: local llvmpipe v0.4.8 6375 ms/frame vs v0.4.7 1738 (instances 11007 vs 6007, load 33–44, not meaningful); ✗ macOS CI, same runner, parts-1000, GPU-presented orbit: **v0.4.8 19.2 fps** (mean 52.1, p95 77.1 ms; macOS 15 19.5) vs **v0.4.7 32.9 fps** (30.4 / 43.5 ms; macOS 15 33.6); display triangles 572k vs 184k (×3.1, the angle mesh); runs 40096334299 / 40096336855 |
| (e) look | ✅ dark: Part Studio, assembly, BOM = live dk-940/942. ✗ German: toolbar tooltip title "Extrude (shift+e)" (live "Linear austragen (shift+e)"; ui.rs:604 formats before the table lookup, de.tsv:2586 has "Extrude"); Extrude dialog: Thin, Inters…, "Faces und sketch regions to extrude", Direction, Starting offset, Entwurf (live Formschräge), Second end position untranslated/wrong; feature menu German except "Sketch 1… bearbeiten" (live „Sketch 1 [Skizze]“ bearbeiten …). Own names stay English ✅, new features German ✅. Off-gate: Preferences units sentence English, heading "Voreinstellungen" (live "Preferences"), "Dezimalstellen" (live "Dezimalstellen anzeigen") |
| (f) drawings | ✅ dual text "[2.36 in]" over "60", "[0.39 in]" / "Ø10" (no Ø). ✗ the linear dimension line runs unbroken through the dual text's bracket bottoms (ANSI and ISO); live asbdd-20 breaks it around the text block, in the gap. ✅ PNG 300 px/in (A3 4961×3508, ANSI B 5100×3300), RGB, 2 colours. ✅ note leaders: leader to a vertex with landing, selection amber, leader menu = live minus Paste (empty clipboard) / Add comment. Note tool placement by synthetic clicks fails on v0.4.7 and v0.4.8 alike (Callout works): not a regression |
| (g) regressions | ✅ section 8: 73/73 ALIVE, no panics (montage g1); smoke: new doc, rectangle (r = Center point rectangle, as live), Extrude, Fillet, quit/reopen intact |

## v0.4.8 re-gate: release/v0.4.8 c7d9d454 (target-lead dev build, footer c7d9d454), FRESH=1, 2026-10-24 19:00–20:30, qa4
Scope (lead): (a), (d), (f), smoke; German out of scope (P3). **Result: FAIL on (d)**; (a), (f), smoke PASS.

| Item | Result |
|---|---|
| (a) fins ×400/×400, delete LP2 while it regenerates | ✅ Delete key: row gone by 0.30 s, spinner gone by 0.92 s; right-click › Delete: row gone by 0.39 s, spinner gone by 1.0 s (was 7–9 s); end state 11 features, LP1 result |
| (d) orbit, macOS CI perf.yml bench, parts-1000, 300 frames (run 40124041877, ref release/v0.4.8 = c7d9d454) | ✗ GPU-presented orbit **18.9 fps** macOS 14 (mean 52.9 ms, p95 76.0) / **20.0 fps** macOS 15, vs v0.4.7 **32.9 / 33.6** (run 40096336855). The display mesh is back to chord (184 000 triangles = v0.4.7), but the draw has **11 007 instances vs v0.4.7's 6 007**: the remaining cost is not triangles (display levels / edge LOD instances?) |
| (f) dual dimension line | ✅ broken around the "[2.36 in] / 60" text block on ANSI B and ISO A3, in the PNG and in the app (= live asbdd-20) |
| smoke | ✅ new doc, corner rectangle (g), Extrude, Fillet, quit/reopen; section 8 73/73 ALIVE, no panics (montage g2) |

Note: perf.yml's `ref` input needs a branch or tag (a bare SHA fails in actions/checkout: run 40121047489 cancelled).
