# Error wording (live vs ours, from 2026-11-18)

Errors must be in Onshape's words: the exact text, its tooltip and its look (colour, icon). Lead 11-18 21:5x: live takes
Part Studio features; live2 takes assemblies, drawings and studios. Live: behaviours.md "error wording sweep"; evidence
live/errors-1118/. Status: **=** same, **≠** differs (owner in brackets).

## Assemblies (live2)

| Situation | Live | Ours | Status |
|---|---|---|---|
| Over-defined mate (a Fastened on top of a Revolute, pin-block) | both mates: "Fastened 1 has error: Mate overdefines the assembly. (Hidden)" / "Revolute 1 has error: …"; rows red, Mate features header red ⬇ circle, the root's icon red ! | the same words (green 548d795fb) | = text |
| …its tooltip | a bold grey-blue title bar with the message, right of the list, then the mate's parameters with checkbox marks | a plain white popup under the row, no title bar or checkboxes; (the parameter list itself matches live 10-22: Revolute / Offset / Limits / Simulation connection) | ≠ [asm] |
| …a hidden error mate's row text | faded red rgb(196,139,148) | strong red rgb(170,37,45) | ≠ [asm] |
| Mate that can't resolve its connectors | "Fastened 1 has error: Mate cannot resolve mate connectors. (Hidden)" (behaviours l.1946) | not checked yet | – |
| Part deleted in its Part Studio (instance can't resolve) | instance row red #aa252d with a red ! badge on its icon; tooltip "Part 2 <1> has error: Failed to resolve instance."; gone from the graphics (behaviours E15, l.8741) | the same words, red row, gone from the graphics (green 548d795fb, pin-block: Extrude 2 deleted); ≠ no ! badge on the part icon, a red dot after the DOF icon instead (errors-1118/oe15.png) | = text, ≠ badge [asm] |
| …a mate on the lost instance | "Cylindrical 1 has error: Failed to resolve the instance for mate. (Hidden)" then its parameters (E15) | "Revolute 1 has error: Failed to resolve the instance for mate. (Hidden)", Revolute / Offset / Limits / Simulation connection | = |

## Drawings (live2)

| Situation | Live | Ours | Status |
|---|---|---|---|
| The part changed (out of date) | the toolbar's Update icon yellow, tooltip "Update from this workspace (ctrl+q)"; views unchanged | the same (green 548d795fb) | = |
| The part gone (its feature suppressed), after Update | a blue info toast "ⓘ Failed to resolve part. ×"; all views vanish (no geometry, no frame) | no toast; each view shows "No parts to show" in a small box | ≠ [studios] |
| A dimension whose reference is lost (the hole it measured suppressed), after Update | kept with its value, drawn red-orange rgb(221,82,40), a filled dot of that colour at the lost circle's centre; no toast or tooltip | the same colour and value; ≠ the dot at the leader's end on the old circle, not at its centre | ≈ [studios] |

## Variable Studio (live2)

| Situation | Live | Ours | Status |
|---|---|---|---|
| A cycle (a = #b, b = #a) or an invalid expression ("1 +") | the bad cells (and cells depending on them) pink; hover tooltip "Error regenerating." only (no cycle wording) | the same (green 548d795fb) | = |
| …the cell after Enter | the edited cell stays selected, red border | the selection moves to the next row (blue border) | ≠ [studios] |

## Feature Studio (live2)

| Situation | Live | Ours | Status |
|---|---|---|---|
| A custom feature's `throw regenError("…")` | row and icon red, tooltip "err Feature 1 did not regenerate properly: err too deep"; dialog title red; Features header red ⓘ; the parameter field not pink | the same form ("uip Feature 1 did not regenerate properly: uip too deep") | = |
| Compile / parse notices | gutter ✕ + squiggle with the parser's text, {!} navbar icon, notices pane (behaviours "Feature Studio", fs parse cases) | studios' fs-* ui-parity takes | see those takes |

## Release (live2)

Live 2026-11-18 22:39–22:42 UTC: a scratch company-owned document (a block custom feature, then a custom feature that throws), managed
workflows switched on for the take and restored after (behaviours "Error wording sweep (7)"); Part 1 › Release…, then Close, never submitted.
Ours: green faf73238b, fs-custom with uip Feature 1 at 40 mm (error), release_settings.json managed_workflows on. Evidence
live/errors-1118/rel-*.

| Situation | Live | Ours | Status |
|---|---|---|---|
| Release… of a part whose Part Studio has an error (company default: "Prevent if a Part Studio feature list contains errors" off) | under Part 1's row an amber box (`alert alert-warning`: bg rgb(255, 248, 230), border rgb(252, 218, 127), text rgb(51, 51, 51)) "Part belongs to a Part Studio that has regeneration errors"; the header "1 item needs attention" | no box, the message isn't anywhere in ours; the header also says "1 item needs attention" | ≠ [studios] |
| …the item's marks | an amber corner triangle rgb(250, 182, 1) on the thumbnail and on the Revision box; the ✕ (tooltip "Remove from release") full red rgb(189, 48, 57) | the triangles rgb(245, 180, 0); the ✕ pale rgb(212, 119, 125), same tooltip | ≈ / ≠ ✕ colour [studios] |
| …the footer | "A field required by one or more actions is empty" with a ? whose tooltip is "Submit requires Approvers"; Apply enabled (blue), Save draft disabled; the empty Release name box grey-bordered (only its label's "Required field must not be empty." red rgb(170, 37, 45)) | the same sentence, its ⓘ shows no tooltip; Apply disabled, Save draft enabled; the Release name box red-bordered | ≠ [studios] |
| …the attention filter (red ⬇ circle) | tooltip "Show only items that need attention" | the same | = |

## Part Studio features (live)

Live 2026-11-18 21:48–22:01 UTC: a 40 × 30 × 10 block, then each failing feature added by REST to a scratch Part Studio, and each row read in the
UI (hover tooltip, dialog). Ours: the same steps through the CLI (`feature_add`, `sketch_constrain`) on green e355f5286, and the list in the app.
Evidence live/errors-1118/ (live-errors.json, rows.json, list.png, tip-*.png, dlg-*.png; ours-cli.json, ours-*.png). The tooltip text is
"<name> did not regenerate properly: <message>"; a name that doesn't start with its type adds it in brackets
("Pattern no direction [Linear pattern] did not …").

| Situation | Live | Ours | Status |
|---|---|---|---|
| Extrude Up to next, nothing ahead (a circle 100 mm off the part, New) | "Up to next termination problem, consider switching it to up to part." | "Failed to extrude selections, check input." | ≠ [core] |
| Extrude with no region | "Select face or sketch region to extrude." | the same | = |
| Fillet with no edges | "Select edges or faces to fillet." | the same | = |
| Fillet too big (every edge of the block, R6 on 10 mm) | "Failed to fillet selections." | the same | = |
| Fillet R35 on one top edge (Allow edge overflow on) | no error | no error | = |
| Chamfer too big (one top edge, 45 mm) | "Failed to chamfer selections."; the edge's entry in the dialog red | the same, the entry red | = |
| Chamfer 20 mm on that edge | no error (volume 6000 mm³) | no error (6000 mm³) | = |
| Shell too thick (top face, 20 mm on 10 mm) | "Could not shell part with selections." | the same | = |
| Linear pattern of the part, overlapping (10 mm × 3, New) | no error, 3 overlapping parts | the same | = |
| Linear pattern with no direction | "Select a pattern direction." | the same | = |
| Sketch over-constrained (a line Horizontal + Vertical) | "Sketch could not be solved."; row red (REST featureStatus WARNING) | the same text, row red (status Error) | = |
| Hole on a point off every part | "None of the holes intersected a part." | the same | = |
| Revolve with no axis | "Select the axis to revolve around." | the same | = |
| Revolve whose axis crosses the profile | "Revolve would create self-intersecting part." | "Geometry operation failed: operation failed: revolve failed (does the profile cross the axis?)" | ≠ [core] |
| Loft with one profile | "Select at least two profiles, a closed guide curve or a closed path." | the same | = |
| Loft of two coplanar profiles (circle and square on Top) | Error "Could not determine loft direction." | builds a part, no error | ≠ [core] |
| Mirror with no plane | "Select a mirror plane." | the same | = |
| Boolean Subtract with no tools | "Cannot resolve entities." | the same | = |
| Sweep with no path | "Select entities for sweep path." | the same | = |
| Draft with no faces | "Select faces to draft." | the same | = |
| A failed feature's row | name red rgb(170, 37, 45), its type icon unchanged (no badge); the Features header shows a red ⬇ circle | the same | = |
| …its tooltip | right of the list, a box at most **400 px** wide that **wraps** ("Chamfer too big did not regenerate properly: Failed to chamfer / selections." on two lines, 400 × 44); 12 px / 18 px weight 500, text rgb(51, 51, 51) on rgb(208, 219, 229), padding 4 px | the same text and colours, on **one line** however long (≈ 400 px wider) | ≠ [psui] |
| …its dialog | the title red; a selection that no longer resolves red in its box; no message text in the dialog | the same | = |
| Hidden sketches' rows at rest | no icon at the row's right end (an eye only on hover) | a crossed-out eye on every hidden sketch's row at rest | ≠ [psui] |

## Sketch (live)

Live 2026-11-18 22:03–22:13 UTC (sketches by REST, then the third dimension typed in the UI); ours green 2a00b4f3f, CLI and app. Evidence live/errors-1118/sketch/.

| Situation | Live | Ours | Status |
|---|---|---|---|
| Over-constrained (a line Horizontal + Vertical): row, tooltip | row red, "Sketch HV did not regenerate properly: Sketch could not be solved." (REST status WARNING) | the same text, row red | = |
| …while editing it | a yellow bubble at the top centre "⚠ Sketch could not be solved. ×" (bg rgb(255, 248, 230), border rgb(252, 218, 127)); the line left as drawn, red; its H and V glyphs white on red boxes | no bubble; the line moved horizontal (half solved), red; the glyphs plain (white boxes) | ≠ [skasm] |
| A redundant dimension typed in the UI (rectangle with H/V, 40 and 30, then 40 on the opposite side) | accepted: the bubble "Sketch could not be solved.", the lines and both 40s red, the row red | accepted, the sketch "fully defined", no error | ≠ [skasm] |
| A conflicting dimension (the same, 50 on the opposite side) | accepted: the bubble, both dimensions red, the row red | kept at 40 with no message (the CLI's value 50 ignored) | ≠ [skasm] |
| An unsolvable dimension (a triangle 100 / 100, the third side typed 300) | accepted: the bubble, the row red | the box's Enter adds the dimension at its measured value (10) without a word; the CLI refuses: "Sketch could not be solved." (invalid_argument) | ≠ [skasm] |
| A self-intersecting profile (a bowtie of 4 lines) extruded | no error: two triangle regions, both extruded (2000 mm³) | the same (2 regions, 2000 mm³) | = |
| Extrude of an open profile's regions (there are none) | "Extrude open did not regenerate properly: Select face or sketch region to extrude. 1 missing selection"; the extrude icon gets a warning overlay; the dialog's box reads "Missing Faces of Sketch open" in red | the same text (the CLI's selection was empty, so no "missing selection" line) | = text |
| Sketch dialog options (Show constraints / expressions / errors) | remembered: accepting a sketch with one changed opens the next sketch with it (live 22:4x, both Show errors and Show expressions); baseline Show constraints on, the others off. **Show errors changes nothing visible** (over-constrained line, overlapping lines, a 0.3 mm gap, crossing lines, a tiny line; in and out of sketch mode): conflicts are red either way | the same memory (saveSketchOptions); my app's saved file had Show expressions on (withdrawn ≠) | = |

## Sheet metal (live)

Live 2026-11-18 22:13–22:16 UTC; ours CLI, green 2a00b4f3f. Evidence live/errors-1118/sheetmetal/.

| Situation | Live | Ours | Status |
|---|---|---|---|
| Convert a block minus its top face (open box) | status INFO; no message in the row, the tooltip ("Sheet metal 1 [Sheet metal model]") or the dialog | Info "Select edges to add bends." in the CLI; the row and tooltip show none | = |
| A second flange on the flanged edge | "Cannot create flange on an edge that is a joint, fillet, chamfer, or internal to the sheet metal model." | the same | = |
| Corner relief on a vertex that isn't a sheet metal corner | "Sheet metal corner not found." | the same | = |
| Corner with nothing selected | "Select a corner to modify." | the same | = |
| Bend relief on an edge that isn't a relief | "Selection is not a bend relief." | the same | = |
| Bend relief with nothing selected | "Select a bend relief to modify." | the same | = |
| Convert a sphere | "Sheet metal sphere [Sheet metal model] did not regenerate properly: Only planar, cylindrical, conical or extruded faces can be converted to sheet metal." | the same | = |
| Convert with nothing selected | "Cannot resolve entities." | the same | = |

## Import / export (live)

Live 2026-11-18 22:17–22:20 UTC (+ › Import…, the tab menu); ours CLI and app, green 2a00b4f3f. Evidence live/errors-1118/import/.

| Situation | Live | Ours | Status |
|---|---|---|---|
| A broken STEP (header, then text) | the "Import to Onshape" options dialog, then a red bubble "bad.step uploaded but failed to translate. Open document ×"; the file kept as a file tab "bad.step" | a file tab "bad.step" and the error "Import failed: <path>: i/o error: could not read STEP file <path>" | ≠ [dx] |
| An unsupported file (data.xyz) | no options dialog; a green bubble "data.xyz has been imported. Open document ×"; a file tab | a file tab, no error (the app's bubble not checked) | = |
| Export of a Part Studio with no parts | the tab menu has no "Export…" (with parts it has; formats list 200, not throttled) | no "Export…" either; the CLI: "Selected tab contains no supported geometry for export." | = |

## Configurations (live)

Live 2026-11-18 22:20–22:22 UTC: a Variable input Len (Length, 1–100 mm, default 10), Extrude 1 depth #Len; values typed in the Configurations box + Enter. Ours app and CLI, green 2a00b4f3f. Evidence live/errors-1118/config/.

| Situation | Live | Ours | Status |
|---|---|---|---|
| Out of range (500, -5) | the field pink rgb(249, 235, 236) with a red underline rgb(189, 48, 57), reading "500 mm" (unit added), its text selected; no message, no tooltip; the model stays at the last valid value | pink with a red underline, "500" (no unit); hovering shows the tooltip "Enter a value between 1 and 100. Current value is 500." | ≠ [studios] |
| Not a value ("abc", "3 kg") | the same pink field, the text kept, the model unchanged | "abc" accepted as is: the field not marked, the model fails (Features header ⬇ red) | ≠ [studios] |
| The Variable's box at rest | the value right-aligned (text-align right); no spinner (the ▲ ● ▼ in panel.png was the Configurations box's scrollbar: with Len and a List Size the box cut the Size row and scrolled; with Len alone no bar, panel-len-only.png) | left-aligned | ≠ [studios] |
