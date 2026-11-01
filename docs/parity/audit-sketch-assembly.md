# Audit: every Sketch, Assembly and Sheet metal surface of live against ours

User request 11-01: our checklists only cover what someone listed, so audit each live surface whole.
Sources: live DOM dumps in /root/onshape-research/live/dom/, behaviours.md, and whole-surface live requests.
Method: every label, tooltip, option and menu item of the live dump is checked against our sources (/root/skasm-scratch/audit/extract.py), then the order and states by reading the builder code next to the dump.
Strings that only name document objects ("Part 1 <2>", "Delete X" chips, "Mate connector of …") and cloud items (Add comment, subscriptions) are not counted as gaps.

Priorities: P1 a missing tool, option or menu command; P2 a missing indicator or wrong state; P3 looks and texts (tooltips).

## Counts

| | |
|---|---|
| Surfaces audited | 19 |
| Gaps found | 28 |
| Gaps fixed | 25 |

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
| S15 | BOM table, its menus, copy, extra columns | assembly-bom-*.txt, assembly-panel-bom.txt | G10 |
| S16 | Every sketch tool's option UI (value boxes, dialogs, flyouts; live 1715) | dom/sk-tool-*.txt | G11, G12, G13, G17 |
| S17 | Profile inspector and Constraint manager | sk-tool-profile-inspector-and-constraint-manager.txt | G14, G15 |
| S18 | Assembly error tooltips, mate HUD, standard content, interference, mass | asm-E15-tooltips.json, mate-hud.txt, dialog-insert-standard-content.txt, dialog-interference-assembly-*.txt, dialog-mass-assembly-*.txt | 0 |
| S19 | Remaining assembly menus (live 1723): pattern / mirror / replicate rows and copies, mate connector row and glyph, relation, mate folder, mate indicators, nested rows, headers | dom/skasm-asm-menus-*.txt, behaviours "remaining assembly right-click menus" | G20–G28 |

Live requests answered: 1715 (tool options), 1716 (Flow), 1720 (exploded views / named positions menus, subassembly fix icons).

## Gaps

| # | P | Surface | Live | Ours | Status |
|---|---|---|---|---|---|
| G1 | P3 | Sketch dialog footer | diagnostics button tooltip "Show sketch diagnostics tools" | "Sketch diagnostics tools" | fixed (11-01) |
| G2 | P3 | Instance list filter row | funnel and search box tooltip "Filter assembly contents" | funnel "Filter", box none | fixed (11-01) |
| G3 | P3 | Instance list header | new folder button tooltip "Create new folder" | "New folder" | fixed (11-01) |
| G4 | P2 | Instance list top row | fix-within icon tooltip "Part(s) inside are fixed" / "More than one part inside is fixed" / "Contains both fixed part(s) and part(s) fastened to origin" (locale table; behaviours 1222, 13414) | "Assembly is grounded" (not a live text) | fixed (11-01) |
| G5 | P3 | Sheet metal table | header tooltips "#", "Name", "Radius (mm)", "Angle (deg)", "Bend direction", "K Factor"; joints "Name", "Type", "Style" | no header tooltips | fixed (11-01) |
| G6 | P1? | Exploded views panel | a view row that is not active: Activate Explode 1 · Rename · Reset starting position · Duplicate · Delete (asb 10-20, behaviours 3139; the active view's menu 814 matches ours) | the active view's menu in every state | fixed (11-01, live 1720 confirmed it): an inactive view: Activate <view> (into editing) · Rename · Reset starting position | Duplicate ("Explode 1 (1)") | Delete; Reset is a no-op (our views always start from the solved assembly) |
| G7 | P3 | Insert dialog | search box clear button tooltip "Clear search" | no tooltip (the vendored gpui-component input draws the clear button) | fixed (11-01): every cleanable input\'s clear button has the tooltip "Clear search" (vendor gpui-component input.rs; ours are all search or URL boxes) |
| G8 | P3 | Configuration panel | "Configure Assembly" tooltip "Add configuration input to create multiple variants of this assembly" | none | fixed (11-01) |
| G9 | P3 | Explode step dialog | step type dropdown tooltip "Explode step type" | none | fixed (11-01) |
| G10 | P3 | BOM | an excluded row (Show excluded/suppressed) has the tooltip "Excluded from all BOMs" | none | fixed (11-01) |
| G11 | P2 | Insert DXF or DWG | a select-item dialog "Insert a DXF or DWG file" ✓ (disabled until a pick) ✕: Current document / Other documents, the document's DXF and DWG files with search "Search DXF or DWG files", Import…; Units select Centimeter · Foot · Inch · Meter · **Millimeter** · Yard; ☐ Use file origin position | rows in the Sketch dialog and the system file chooser; Units has "From file" (not live's) and defaults to it | fixed (11-01): the select-item dialog in the Sketch dialog\'s place (Current / Other documents, DXF and DWG file tabs, search, Import… adds the file as a tab), ✓ greyed until a pick, Units (Millimeter default) and Use file origin position under it. Live\'s dialog sat at x 898; ours in the Sketch dialog\'s place like Insert image |
| G12 | P1 | Sketch constraints | a 15th constraint **Flow** (G3, "Create flow continuous transitions between sketch splines or conics and surrounding geometry.", icon flow-continuity, EQUAL_CURVATURE_G3) after Curvature in the toolbar, the Constraint manager and the search | missing | fixed (11-01): ConstraintKind::Flow (tangent + G2 + equal dκ/ds at the joint, numeric FlowSide for lines, arcs, control-point curves incl. conics, fit splines), the toolbar button after Curvature (live's icon), tool search text, sketch_constrain "flow", FS EQUAL_CURVATURE_G3; one glyph on the curve. The glyph sits two-thirds along the curve from the joint, below it on a short grey leader (11-01) |
| G13 | P2 | Insert image | "Insert an image" dialog: Current document / Other documents / **Libraries** (Onshape images: Environment maps, …) | the system file chooser | partly fixed (11-01): the same dialog, titled "Insert an image", with Current / Other documents and Import…; a pick goes straight to drawing the rectangle. Open: live\'s title "Insert an image" and the Libraries tab (Onshape images: environment maps; not in our data) |
| G14 | P2 | Constraint manager | Type filter: all 22 types as icon toggles with tooltips (… Curvature, Flow, Quadrant, Intersection, Offset, Linear pattern, Circular pattern, Projected, Dimension); Mode adds **In context**; list label "Entities and constraints"; Delete all tooltip "Delete all selected constraints" | text chips of the types present only; no In context; label with a count | fixed (11-01): live\'s 220 px dialog right of the Sketch dialog, the 22 type / 3 mode / 3 status icon toggles with tooltips (new icons quadrant, cm-*), the slider, tabs, the "Entities and constraints" list ("Horizontal 1" over "Line 2"; empty until something is selected while automatic selection is on), red Delete all with its tooltip. "In context" lists nothing: our sketches don\'t mark in-context references |
| G15 | P3 | Profile inspector | a "Loose ends" list ("No loose ends found" when none), Previous / Next | a free list of issues, "No issues: every profile is closed." | fixed (11-01): live\'s dialog under the diagnostics button, the "Loose ends" list, "No loose ends found", blue Previous / Next |
| G16 | P2 | Curvature / Flow | a fit spline far from G2 with a 3 point arc solves by moving everything (live 1716 (b): the arc's radius 10 → 26.866) | Curvature and Flow reported the sketch as over-constrained from such a start | fixed (11-01): the G2 / G3 residuals are scaled to lengths (κ·L², dκ/ds·L³, L the curves' size when built); live's case (b) with a drawn spline (handles) now solves, Curvature to an arc solves at 4, 40 and 400 mm (solver_basic::curvature_to_an_arc_solves_at_sketch_size). A spline without end handles (from FeatureScript) still can't take G2 with an arc: its natural ends have no curvature (live gives it end derivatives) |
| G17 | P2 | Text tool | no option UI before its box is drawn (live 1715: no tool shows an option box) | a floating "Text / Height (click)" box while the tool was active | fixed (11-01) |
| G18 | P3 | Instances list | parts fixed inside a subassembly show no fix icon in the parent (live 1720) | the nested rows showed it | fixed (11-01) |
| G19 | P3 | Tool search (sketch) | "Sketch flow" finds nothing though the toolbar has Flow (ui-e-search-live-sketch-retry) | Flow was searchable | fixed (11-01) |
| G20 | P1 | Pattern / Mirror / Replicate rows | Rename · Edit… · Hide · Hide other / all instances (Mirror: · Show center of mass) · Isolate… · Make transparent… · Suppress · Dynamic suppression ▸ | Move to new subassembly (Replicate: Dissolve <name>; Mirror: none) · Release… | Zoom to selection | Add selection to folder… · Expand ▸ · Collapse ▸ | Delete | Edit… · Suppress · Dissolve | Delete | fixed (11-01; UI test the_pattern_and_copy_row_menus_are_lives), but Dynamic suppression (G27) |
| G21 | P2 | A pattern's copy row | the instance menu without center of mass, dynamic suppression, mates, in-context, replace, subassembly, version, folder and Delete items | the full instance menu | fixed (11-01) |
| G22 | P1 | Assembly mate connector | row: Create Part Studio in context… · Rename · Edit… | Hide · Hide other / all mate connectors · Isolate… · Make transparent… · Section view… · Suppress | Add selection to folder… | Delete; glyph: Create Part Studio in context… | Edit <name>… | … · Suppress <name> | Create Drawing of <asm>… | Find in instance list | Select other… | Zoom to fit · View normal to | Delete <name> | Rename · Edit… | Hide · Hide other · Show all · Suppress | Delete, and the mate menu on the glyph | fixed (11-01), but Dynamic suppression (G27) |
| G23 | P3 | Relation row (Gear 1) | Rename · Edit… | Suppress · Dynamic suppression ▸ | Add selection to folder… | Delete | also Isolate…, Make transparent…, Expand ▸, Collapse ▸ | fixed (11-01), but Dynamic suppression (G27) |
| G24 | P1 | Mate indicator in the graphics | Edit <mate>… | Hide · Hide other mates · Hide all mates · Isolate… · Make transparent… · Edit named position driver mate… · Suppress <mate> | Animate <mate>… | Reset | Create Drawing of <asm>… | Find in instance list | Select other… | Zoom to fit | Delete <mate>; a replicate's mate without Edit / Delete | the list row's menu | fixed (11-01) |
| G25 | P3 | Mate folder row | Rename | Hide · Hide other mates · Hide all mates · Isolate… · Make transparent… · Suppress · Dynamic suppression ▸ | Add selection to folder… · Expand ▸ · Collapse ▸ | Delete · Unpack folder | Hide and Show, "… mate connectors and mates", no Dynamic suppression, Expand / Collapse as plain items | fixed (11-01) |
| G26 | P2 | Nested instance row (inside an expanded subassembly) | Properties… | Hide · Hide other / all instances · Isolate… · Make transparent… · Fix · Use best available tessellation | Edit in context · Add mate connector to instance origin… · Switch to … | Copy | Move to new subassembly · Export… · Release… | Zoom to selection | Create Drawing of … | Create empty subassembly | no menu | open: our hide / isolate / fix act on top-level instances only |
| G27 | P2 | Dynamic suppression ▸ on patterns, relations and assembly mate connectors | offered | not modelled (instances, mates and folders only) | open |
| G28 | P3 | Two mates selected | Reset | Show · Show all mates · Isolate… · Make transparent… · Edit named position driver mate… · Suppress · Dynamic suppression ▸ | Clear selection | Add selection to folder… · Expand ▸ · Collapse ▸ | Delete | the clicked mate's menu | open |
