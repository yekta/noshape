# Error wording (live vs ours, from 2026-11-18)

Errors must be in Onshape's words: the exact text, its tooltip and its look (colour, icon). Lead 11-18 21:5x: live takes
Part Studio features; live2 takes assemblies, drawings and studios. Live: behaviours.md "error wording sweep"; evidence
live/errors-1118/. Status: **=** same, **≠** differs (owner in brackets).

## Assemblies (live2)

| Situation | Live | Ours | Status |
|---|---|---|---|
| Over-defined mate (a Fastened on top of a Revolute, pin-block) | both mates: "Fastened 1 has error: Mate overdefines the assembly. (Hidden)" / "Revolute 1 has error: …"; rows red, Mate features header red ⬇ circle, the root's icon red ! | the same words (green 548d795fb) | = text |
| …its tooltip | a bold grey-blue title bar with the message, right of the list, then the mate's parameters with checkbox marks | the mate flyout (an error mate too): the message as its title bar, then the parameters with their checkboxes; a Fastened lists Offset and Simulation connection (33d8db32, test a_hidden_mate_in_error_is_faded_red / the flyout assertion in assembly_ui_tests) | = [asm] |
| …a hidden error mate's row text | faded red rgb(196,139,148) | the same (33d8db32, `feature_row_color`) | = [asm] |
| Mate that can't resolve its connectors (Part 1's hole filled by an Add extrude, pin-block; live2 11-19) | "Revolute 1 has error: Mate cannot resolve mate connectors. (Hidden)"; row faded red, Mate features ⬇ red, Part 2 stays where it last solved; flyout right of the list (x 243), title bar bold dark text on grey-blue, the bar as wide as the text, then "Revolute" in grey on a field line (errors-1119-asmdrw/a1-tip-c.png) | the same words (green bd38f88d3); the flyout's title is regular weight and the bar is narrower than the text (the text runs past it), "Revolute" is larger and black with no field line, and the flyout starts at x 192, over the list (oa1-tip-c.png, lo.png live above ours) | ≠ [asm] |
| Part deleted in its Part Studio (instance can't resolve) | instance row red #aa252d with a red ! badge on its icon; tooltip "Part 2 <1> has error: Failed to resolve instance."; gone from the graphics (behaviours E15, l.8741) | the same words, red row, gone from the graphics (green 548d795fb, pin-block: Extrude 2 deleted); its icon carries the red ! badge (10 px at its bottom right), no dot (33d8db32; was a red dot after the DOF icon, errors-1118/oe15.png) | = [asm] |
| …a mate on the lost instance | "Cylindrical 1 has error: Failed to resolve the instance for mate. (Hidden)" then its parameters (E15) | "Revolute 1 has error: Failed to resolve the instance for mate. (Hidden)", Revolute / Offset / Limits / Simulation connection | = |
| Mate between two fixed instances (Part 2 <1> › Fix, pin-block; live2 11-19) | "Revolute 1 has error: Mate is between fixed instances. (Hidden)"; row red rgb(170,37,45) (errors-1119-asmdrw/a2-tip-c.png) | the same words, red row (oa2-c.png) | = |
| …the fixed instances' warning | an amber ⚠ overlaid on the fixed icon's lower right of each fixed instance and of the root row; tooltip "More than one part is fixed" (instances) and "More than one part inside is fixed" (root), grey-blue bubble at the icon (a2-p1warn-c.png) | the ⚠ beside the fixed icon, not on it; none on the root row; tooltip "Part 1 <1> has warning: More than one part is fixed" in a white bordered box (oa2-c.png) | ≠ [asm] |
| Revolute limits with minimum 90° above maximum 10° | both limit fields pink rgb(249,235,236) with a red rgb(189,48,57) underline, title red, ✓ pale and disabled (opacity 0.2); a field's tooltip "Minimum mate limit should be smaller than maximum limit." (a5-bad-c.png) | fields and title the same; ✓ stays full green and enabled; the field's tooltip is "Minimum Z angle" (oa5-c.png) | ≠ [asm] |
| An instance a mate uses is suppressed | the mate row italic grey (no strike-through), tooltip "Revolute 1 (Inactive: One or more instances is suppressed) (Hidden)"; the instance struck through, tooltip "Part 2 <1> (Suppressed)" (a6-c.png) | the mate's words the same, but its row is also struck through; the instance's tooltip is "Part 2 <1>" without "(Suppressed)" (oa6-c.png) | ≠ [asm] |
| An instance a mate uses is deleted (right-click › Delete) | deleted with its mate at once, no confirm, no message; Mate features (0) (a7-0.png) | the same (oa7-c.png) | = |
| The Part Studio has a failing feature (Fillet 1 R50 on Part 2) | nothing in the assembly: instance rows, mates and tooltips as before; the Insert dialog lists that Part Studio and its parts unmarked; an instance inserted from a studio with an error ("start block-broken") is plain too (a3-0.png, a3-ins-pin.png, a3-ins-tip.png) | the same, nothing marked (oa3-ins.png, oa3-tip.png) | = |
| Insert dialog, an empty Part Studio (no parts) | not listed at all; searching for it shows "No Part Studios match current search." in bold dark 12 px at the list's top left (q3.png) | "start empty" is listed (expanded, no parts under it); a search with no match shows "No results." in small grey (q12.png, oa3-ins.png) | ≠ [asm] |

## Drawings (live2)

| Situation | Live | Ours | Status |
|---|---|---|---|
| The part changed (out of date) | the toolbar's Update icon yellow, tooltip "Update from this workspace (ctrl+q)"; views unchanged | the same (green 548d795fb) | = |
| The part gone (its feature suppressed), after Update | a blue info toast "ⓘ Failed to resolve part. ×"; all views vanish (no geometry, no frame) | = (fixed 11-18: the blue info toast "Failed to resolve part."; the views go blank, no box; GPUI feel `a_view_whose_part_is_gone_goes_blank_with_failed_to_resolve_part`) | = |
| A dimension whose reference is lost (the hole it measured suppressed), after Update | kept with its value, drawn red-orange rgb(221,82,40), a filled dot of that colour at the lost circle's centre; no toast or tooltip | = (fixed 11-18: the dot at the lost circle's centre, in the lost colour; GPUI feel `a_lost_diameters_dot_sits_at_the_circles_centre`) | = |
| Insert view of an empty Part Studio (+ › Create Drawing…, No views; live2 11-19) | the "Select a part or assembly" list leaves out Part Studios with no parts; searching for one shows "No parts or sketches match current search." bold 12 px rgb(51,51,51) (errors-1119-asmdrw/d1-ins-c.png, d1-cmp.png left) | empty studios left out (=); a search with no match shows nothing under the filter icons (d1-cmp.png right) | ≠ [studios] |
| A linear dimension whose edge is replaced (Chamfer 1 5 mm on the dimensioned top edge), after Update | the dimension re-attaches to the new outer edge: 1.575 kept, black, no lost marks (d2-now.png, d2-c.png) | drawn as lost: red-orange rgb(221,82,40) with a dot at each witness-line foot (od2-c.png) | ≠ [studios] |
| A linear dimension whose part is gone (pin's .394, Extrude 2 suppressed), after Update | kept with its value, red-orange rgb(221,82,40), a filled dot at each witness-line foot where the edge's ends were; no toast or tooltip (d3-u.png) | the lost look is the same (colour, end dots: od2-c.png); note: on that pin edge (a circle seen edge-on in the Front view) ours' Dimension makes a diameter "Ø.394" with a leader where live makes the linear .394 (od3-dim-c.png) | ≠ [studios] |
| Hole/thread callout on a hole that isn't a Hole feature (a sketch circle cut) | no highlight on hover, a click does nothing, no message (d4-c.png) | the same (od4-c.png) | = |

## Variable Studio (live2)

| Situation | Live | Ours | Status |
|---|---|---|---|
| A cycle (a = #b, b = #a) or an invalid expression ("1 +") | the bad cells (and cells depending on them) pink; hover tooltip "Error regenerating." only (no cycle wording) | the same (green 548d795fb) | = |
| …the cell after Enter | the edited cell stays selected, red border | = (fixed 11-18: Enter on a value that fails keeps its cell selected; GPUI `enter_on_a_failing_value_keeps_its_cell_selected`) | = |

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
| Release… of a part whose Part Studio has an error (company default: "Prevent if a Part Studio feature list contains errors" off) | under Part 1's row an amber box (`alert alert-warning`: bg rgb(255, 248, 230), border rgb(252, 218, 127), text rgb(51, 51, 51)) "Part belongs to a Part Studio that has regeneration errors"; the header "1 item needs attention" | = (fixed 11-18: the amber box with live's sentence under the row, live's colours; the header as live; GPUI `a_part_of_a_failing_part_studio_gets_the_regeneration_errors_notice`) | = |
| …the item's marks | an amber corner triangle rgb(250, 182, 1) on the thumbnail and on the Revision box; the ✕ (tooltip "Remove from release") full red rgb(189, 48, 57) | = (fixed 11-18: the ✕ full red) | = |
| …the footer | "A field required by one or more actions is empty" with a ? whose tooltip is "Submit requires Approvers"; Apply enabled (blue), Save draft disabled; the empty Release name box grey-bordered (only its label's "Required field must not be empty." red rgb(170, 37, 45)) | ≈ (fixed 11-18: the ? (light blue) shows "Submit requires Approvers"; Apply enabled, Save draft disabled until an edit; the name box grey, its label red; "Submit" and "Approvers" bold) | = |
| …the attention filter (red ⬇ circle) | tooltip "Show only items that need attention" | the same | = |
| The same with "Prevent if a Part Studio feature list contains errors" on (live 22:48–22:53 UTC, the same record / restore) | the same words "Part belongs to a Part Studio that has regeneration errors" in a red box instead (bg rgb(249, 235, 236), border rgb(245, 223, 225), text dark rgb(51, 51, 51)), inset under the row like the amber one; the rest as with it off (Apply still enabled) | = (fixed 11-18: live's sentence in the red box, live's colours, inset under the row) | = |

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
| …while editing it | a yellow bubble at the top centre "⚠ Sketch could not be solved. ×" (bg rgb(255, 248, 230), border rgb(252, 218, 127)); the line left as drawn, red; its H and V glyphs white on red boxes | the bubble from the moment it opens (`update_conflicts`: it follows the edited sketch's solve, cleared once it solves; it was cleared on the row's Warning); the glyphs white on red boxes whatever Show errors says (live: the box changes nothing); the line as stored when the sketch arrives over-constrained (live2's ours was built one constraint at a time by CLI, H solved before V failed) | = (fixed 11-18, GPUI `an_open_sketchs_state_follows_its_constraints`) [skasm]; live recheck 11-19 on green b16fb02c: the bubble from opening on, the line red = |
| A redundant dimension typed in the UI (rectangle with H/V, 40 and 30, then 40 on the opposite side) | accepted: the bubble "Sketch could not be solved.", the lines and both 40s red, the row red | the Dimension tool places it driven with its box open; a typed value makes it driving: the bubble, the dimensions red, the shape kept, the row red (`apply_dimension`) | = (fixed 11-18, scene `a_typed_value_makes_an_over_defining_dimension_driving`) [skasm] |
| A conflicting dimension (the same, 50 on the opposite side) | accepted: the bubble, both dimensions red, the row red | the same: driving at 50, the bubble, red, the shape kept | = (fixed 11-18, same test) [skasm] |
| An unsolvable dimension (a triangle 100 / 100, the third side typed 300) | accepted: the bubble, the row red | the value taken (300), the bubble, the row red "Sketch could not be solved.", the last shape kept (regeneration warned only for an over-constrained sketch, not a failed solve) | = (fixed 11-18, model-core `a_sketch_whose_solve_fails_says_so`) [skasm] |
| The same three dimensions by the CLI (`sketch_dimension`, live recheck 11-19 on green b16fb02c, live-scratch/err18/ours2/sk2.py) | as in the UI: taken as driving, the sketch red "Sketch could not be solved." | the same (fixed 11-19, `a_dimension_value_the_sketch_cant_take_is_kept_driving`): driving, the sketch red, the reply's warning "Sketch could not be solved.". (With a sketch already in error, the dimensions were applied all along: `failed` lists every feature in error, as feature replies do, and sk.py read that as a refusal; sketch_get shows them.) | = |
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
| Out of range (500, -5) | the field pink rgb(249, 235, 236) with a red underline rgb(189, 48, 57), reading "500 mm" (unit added), its text selected; no message, no tooltip; the model stays at the last valid value | = (fixed 11-18: pink, "500 mm" kept with its unit and selected, no tooltip; GPUI `the_configurations_box_refuses_a_value_out_of_range_or_not_a_value`) | = |
| Not a value ("abc", "3 kg") | the same pink field, the text kept, the model unchanged | = (fixed 11-18: pink and refused, the model unchanged; the same test) | = |
| The Variable's box at rest | the value right-aligned (text-align right); no spinner (the ▲ ● ▼ in panel.png was the Configurations box's scrollbar: with Len and a List Size the box cut the Size row and scrolled; with Len alone no bar, panel-len-only.png) | = (fixed 11-18: right-aligned; with two inputs the box stays one input tall and scrolls; GPUI `the_configurations_box_stays_one_input_tall_with_two`) | = |
