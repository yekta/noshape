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

| # | Step | Live | Status |
|---|---|---|---|
| 1.1 | Start window › Create › Document… | Dialog with the name "Untitled document" selected | ✅ |
| 1.2 | Type a name, Enter | Creates and opens it | ✅ |
| 1.3 | New document contents | Part Studio 1 (active) + Assembly 1; "Features (4)", Default geometry, rollback bar under Right, "Parts (0)"; isometric | ✅ |
| 1.4 | Start window footer version | (ours) should read the release version | ✗ F27 |

## 2. Sketch on Top (live: "Sketch mode", "Input behaviour" › G)

| # | Step | Live | Status |
|---|---|---|---|
| 2.1 | Sketch, then click Top in the list | Sketch 1 dialog, plane "Top plane", no tool active, view not rotated | ✅ |
| 2.2 | Sketch dialog defaults | ☐ Disable imprinting, **☑ Show constraints**, ☐ Show expressions, ☑ Show errors | ✗ F1 |
| 2.3 | N | Animated view normal to the plane | ✅ |
| 2.4 | L, click, click, click (chain) | Previous segment's length boxed, live length in blue | ✅ |
| 2.5 | Enter mid-chain | The boxed length becomes a driving dimension; the tool stays | ✅ |
| 2.6 | Esc | One Esc ends the chain and exits the tool | ✅ |
| 2.7 | G (corner rectangle), C (circle), A (3-point arc) | As live | ✅ |
| 2.8 | Select two lines, E | Equal; glyphs shown | ✅ |
| 2.9 | D, pick circle, place | Value box opens with the full value selected ("20.479094 mm") | ✅ |
| 2.10 | Bare "25" + Enter | Ø25, box closes | ✅ (but see F3) |
| 2.11 | Double-click the dimension, "1 in" + Enter | Ø25.4, tan background | ✅ |
| 2.12 | "10+5" + Enter | Label "ƒ 15" (expression prefix) | ✗ F4 |
| 2.13 | Re-open it | Box shows "(10+5) mm" | ✗ F5 |
| 2.14 | Type, Esc | Reverts and blurs; box stays | ✅ |
| 2.15 | Unknown "#w" + Enter | "Create variable" popup next to the field (Name "w") | ✗ F6 |
| 2.16 | "abc" + Enter | Pink field, red underline, ✓ disabled, box stays open, hover "Enter a valid expression." | ✗ F7 |
| 2.17 | Arc radius, point-point, line-point dimensions | Created, typed values applied | ✅ |
| 2.18 | Type the value immediately after the placing click | Applied (keys not lost) | ✅ (checked with 20 ms steps) |
| 2.19 | Enter with no tool active | Does **not** accept the sketch (live sketch/311) | ✅ (see F2) |
| 2.20 | ✓ | Sketch 1 in the list, under-defined badge | ✅ |
| 2.21 | Dimension crash (user report, macOS) | – | not reproduced on Linux (all dimension kinds above) |

## 3. Features (live: "Part Studio: features, selection, menus", "Feature dialog basics", "Input behaviour")

| # | Step | Live | Status |
|---|---|---|---|
| 3.1 | Shift+E, click the rectangle region | "Face of Sketch 1", 25 mm default, Part 1 preview | ✅ |
| 3.2 | Depth: 1st click / "30" Enter / Enter | Selects all / "30 mm" re-selected, dialog stays / commits; sketch hidden | ✅ |
| 3.3 | Shift+7 | Isometric | ✅ |
| 3.4 | Shift+F, two edges, "3" Enter Enter | Fillet 1 | ✅ |
| 3.5 | Chamfer on the vertical edge meeting both 3 mm fillets: 5 mm, then 2 mm | 5 mm: **error "Distance of chamfer on face too large."** (live G46: Onshape refuses it too); 2 mm builds Onshape's 10-face shape | (F8 fixed per kern 10-22; re-run) |
| 3.6 | Shell, remove top face, 2.5 / 1 mm | Shells (2.5 mm: 13946.59 mm³, 21 faces, kern 10-22) | (F9 fixed per kern 10-22; re-run) |
| 3.7 | After Enter committed a value, Esc | Cancels the dialog (no pending edit) | ✅ |
| 3.8 | Sketch points on the top face; Hole from the sketch | One chip "Vertices of Sketch 2"; one hole per point | ✗ F10 |
| 3.9 | Hole defaults and callout | Drilled 1/4 (6.35 mm), 12 mm blind, 118 deg; "Ø6.35 ↧ 12" callout; row "Ø 6.35 mm ↧ 12 mm" | ✅ |
| 3.10 | Linear pattern, Part type: Part 1, edge, 40 mm × 3 | Part 2/3, default colour rotation #a5a5a5, #3b61b4 | ✅ |
| 3.11 | Direction field, click near the edge's midpoint | Filtered to the edge ("Edge of Part 1") | ✗ F11 |
| 3.12 | Linear pattern, Feature type: Hole, edge, 10 mm × 2 | Holes copied on Part 1 | ✗ F12 (Part 1 destroyed) |
| 3.13 | Esc while a select's search box is open | Closes the list only | ✗ F13 |
| 3.14 | Mirror, Part type: Part 3 about Right | Part 4 | ✅ |
| 3.15 | Shift+X while a feature dialog is open | ? | ✗ F14 (Section view opens on top of the open Mirror dialog) |
| 3.16 | Boolean union Part 3 + Part 4 | Touching parts: one part. Parts that don't touch (live 10-22): **two unchanged parts, the row stays black with no error**, and while editing a blue info bubble **"Boolean resulted in no geometry change…"** (psb BOOL1) | ✅ touching; disjoint: re-run |

## 4. Edit, roll back, reorder, suppress, delete, undo (live: "Rollback bar", "Right-click menus")

| # | Step | Live | Status |
|---|---|---|---|
| 4.1 | Double-click Extrude 1, depth 25, Enter Enter | Rolled back while editing; model regenerates | ✗ F12 (Part 1 disappears silently) |
| 4.2 | Feature row right-click › Suppress | Struck-through italic; model updates | ✅ |
| 4.3 | Drag Fillet 1 below Chamfer 1 | Drop line + pill; reordered | ✅ (Fillet then fails: F8) |
| 4.4 | Ctrl+Z / Ctrl+Y / Ctrl+Z | Undo / redo the reorder | ✅ |
| 4.5 | Click a row, ↑ ↑ / ↓ ↓ | Rollback bar moves; light blue while focused | ✅ |
| 4.6 | Select a feature row, Delete key | Deletes it | ✗ F15 |
| 4.7 | Right-click › Delete; Ctrl+Z | Deleted; restored | ✅ |
| 4.8 | Error header | At the default panel width live truncates to "Features ..." beside the error icon (part-studio/94, 570); a wider panel shows "Features (N)" | ✅ (F16 withdrawn) |

## 5. Variables, configuration, measure, mass properties

| # | Step | Live | Status |
|---|---|---|---|
| 5.1 | Variable (Plane ▾ dropdown): w, 20 | "#w = 20 mm" | ✅ |
| 5.2 | Plane ▾ dropdown contents | … Variable, Query variable, **Composite part, Tag** | ✗ F17 (last items cut off at the window bottom) |
| 5.3 | Drag the variable to the top; Extrude 1 depth "#w" | Parts 20 mm | ✅ |
| 5.4 | Select a face / two faces | "Area: 856.366 mm²" / a distance | ✅ |
| 5.5 | Mass properties, no material | ? centre of mass shown | ? F18 (centre of mass and inertia blank) |
| 5.6 | Material warning | Goes with the Mass properties dialog | ✗ F19 (stays; later covers the sketch dialog's ✓) |
| 5.7 | Configurations panel › Configure Part Studio; add row "Long" | List input, Default / Long, info box | ✅ |
| 5.8 | Right-click a feature parameter (Variable Value) | "Configure …" | ✗ F20 (only Cut/Copy/Paste/Select All) |
| 5.9 | + Configure features | Yellow banner "Select … to configure [Done]" | ✗ F21 (no banner seen) |
| 5.10 | Strip tooltip after the panel opens | Hides | ✗ F22 (stays over the panel) |
| 5.11 | ☰ › Workspace units… while a sketch is open; Length → Inch, ✓ | Units change; the sketch stays open; geometry unchanged | ✗ F23, F24 |

## 6. Assembly and drawing (live: "Assembly", "Drawing")

| # | Step | Live | Status |
|---|---|---|---|
| 6.1 | Assembly tab, I | Insert dialog | ✅ |
| 6.2 | Click Part 2, click; click Part 3, click | Inserted; **the first instance lands at the origin** | ✗ F25 |
| 6.3 | Instance filter "3" | Filters the list | ✅ |
| 6.4 | Fastened mate (M): two faces | Mate HUD, solves, "Fastened 1" | ✅ |
| 6.5 | + › Create Drawing…, ANSI_A_MM, OK | Model picker, then Insert view following the cursor | ✅ |
| 6.6 | Place front, right and top views; Esc | Projected views | ✅ |
| 6.7 | Dimension tool, pick an edge, place | Dimension "17"; tool stays active | ✅ (two-edge distance not retried) |

## 7. Export, save, reopen

| # | Step | Live | Status |
|---|---|---|---|
| 7.1 | Part Studio tab › Export…, STEP | Dialog (Parasolid default), STEP options, file written | ✅ (3 solids, mm) |
| 7.2 | STEP contents | Products named as in live's export | ✗ F26 |
| 7.3 | Quit, start, Recently opened, open | Thumbnail, all tabs and features back | ✅ (single click selects; double click opens: ? vs live) |

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
