# Audit: every Sketch, Assembly and Sheet metal surface of live against ours

User request 11-01: our checklists only cover what someone listed, so audit each live surface whole.
Sources: live DOM dumps in /root/onshape-research/live/dom/, behaviours.md, and whole-surface live requests.
Method: every label, tooltip, option and menu item of the live dump is checked against our sources (/root/skasm-scratch/audit/extract.py), then the order and states by reading the builder code next to the dump.
Strings that only name document objects ("Part 1 <2>", "Delete X" chips, "Mate connector of …") and cloud items (Add comment, subscriptions) are not counted as gaps.

Priorities: P1 a missing tool, option or menu command; P2 a missing indicator or wrong state; P3 looks and texts (tooltips).

## Counts

| | |
|---|---|
| Surfaces audited | 14 |
| Gaps found | 9 |
| Gaps fixed | 7 |

## Surfaces

| # | Surface | Live source | Gaps |
|---|---|---|---|
| S1 | Sketch toolbar: groups, tools, dropdowns, labels, keys | dropdowns-sketch.json, toolbar-sketch.txt | 0 |
| S2 | Sketch dialog, Text dialog, sketch Linear pattern dialog | dialog-sketch-empty / -linear-pattern / -text.txt, sketch-text-dialog.html | G1 |
| S3 | Sketch right-click menus while editing: empty, a tool active, on an entity, on a model edge, on a dimension (Configure dimension ▸) | context-menus-sketch-linetool.txt, behaviours (sketch/31, 204, 400–406, 1703) | 0 |
| S4 | Part Studio right-click on sketch regions / curves / circles (not editing) and the Select ▸ submenus | context-menus-ps-sketch-entities.txt, context-menus-ps-sketch-select-submenus.txt | 0 |
| S5 | Keyboard shortcuts: the Shift+/ sheet and Preferences › Customize (Sketch 54, Assembly 16) | keyboard-shortcuts.txt, customize-keyboard-shortcuts.txt | 0 |
| S6 | Mate dialogs, all 11 types, empty and filled, offset, limits | dialog-mate-*.txt | 0 |
| S7 | Relation dialogs: Gear, Rack and pinion, Screw, Linear | dialog-relation-*.txt | 0 |
| S8 | Assembly tools: Insert, Replicate, Linear / Circular / Part pattern, Mirror, Group, Section view, Mate connector, Animate, Named position driver, Replace | dialog-asm-*, dialog-assembly-*, assembly-insert-*, assembly-replace-*, qa5-asm-*, ui2-asm-* | G7, G9 |
| S9 | Instance list: filter row, header, rows and their icons | assembly-lists.txt, assembly-replace-all.html | G2, G3, G4 |
| S10 | Assembly right-click menus: instance (list, graphics, 2 selected), empty graphics, top row, origin, mate, revolute, group, folder, explode view and step, named position row | context-menus-assembly*.txt, explode-rclick-*.html, behaviours 1219–1226, 1847, 3138 | G6 |
| S11 | Exploded views and Named positions panels | assembly-panel-exploded-views.txt, explode-panel-*.html, panel-named-positions*.txt, assembly-named-position-editing.txt | 0 |
| S12 | Configuration panel (assembly) | assembly-panel-configuration.txt | G8 |
| S13 | Sheet metal dialogs: model, thicken, flange, hem, tab, bend relief, corner, bend, jog, loft, finish, make joint, modify joint | dialog-sheet-metal-model.txt, qa5-sm-*.html, dialog-sheetmetal-make-joint.txt | 0 |
| S14 | Sheet metal table and flat panel, flat export | qa5-sm-091-smtable.html, qa5-sm-112-flat-export.html | G5 |

Waiting on live (whole-surface requests, one at a time): every sketch tool's option UI (fillet, chamfer, offset, polygon, slot, conic, spline, Use, patterns, transform…), then the exploded views / named positions menus in each state.

## Gaps

| # | P | Surface | Live | Ours | Status |
|---|---|---|---|---|---|
| G1 | P3 | Sketch dialog footer | diagnostics button tooltip "Show sketch diagnostics tools" | "Sketch diagnostics tools" | fixed (11-01) |
| G2 | P3 | Instance list filter row | funnel and search box tooltip "Filter assembly contents" | funnel "Filter", box none | fixed (11-01) |
| G3 | P3 | Instance list header | new folder button tooltip "Create new folder" | "New folder" | fixed (11-01) |
| G4 | P2 | Instance list top row | fix-within icon tooltip "Part(s) inside are fixed" / "More than one part inside is fixed" / "Contains both fixed part(s) and part(s) fastened to origin" (locale table; behaviours 1222, 13414) | "Assembly is grounded" (not a live text) | fixed (11-01) |
| G5 | P3 | Sheet metal table | header tooltips "#", "Name", "Radius (mm)", "Angle (deg)", "Bend direction", "K Factor"; joints "Name", "Type", "Style" | no header tooltips | fixed (11-01) |
| G6 | P1? | Exploded views panel | a view row that is not active: Activate Explode 1 · Rename · Reset starting position · Duplicate · Delete (asb 10-20, behaviours 3139; the active view's menu 814 matches ours) | the active view's menu in every state | to confirm with live |
| G7 | P3 | Insert dialog | search box clear button tooltip "Clear search" | no tooltip (the vendored gpui-component input draws the clear button) | deferred: needs a vendor change |
| G8 | P3 | Configuration panel | "Configure Assembly" tooltip "Add configuration input to create multiple variants of this assembly" | none | fixed (11-01) |
| G9 | P3 | Explode step dialog | step type dropdown tooltip "Explode step type" | none | fixed (11-01) |
