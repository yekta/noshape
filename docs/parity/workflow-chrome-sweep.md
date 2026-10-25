# Workflow: Part Studio chrome sweep (ui, from 2026-10-22)

The Part Studio chrome compared with the live captures in /root/onshape-research/live, area by area. Areas: the view
cube and its menu, the right-side panels (Appearances, Mass properties, Measure, Section view, Named views, analysis
tools), the toolbar overflow, the top bar menus, and the tab bar menus. One row per difference; fixes land in small
commits. Status: ✅ fixed (commit), ✗ open, ? asked live (REQUESTS.md).

Method: our screenshot (Xvfb :101, 1919×1079) next to the live screenshot of the same state; positions, colours and
sizes are measured from pixels at 1×, texts from the dom/ captures.

## 1. View cube and Camera and render options menu (live part-studio/21, 50–54; dom/menu-camera-render.txt)

| # | What | Live | Ours | Status |
|---|---|---|---|---|
| VC1 | Items, groups, ✓ marks, separators, icons of the camera menu | 15 rows in 7 groups (dom) | the same | ✅ matches |
| VC2 | Widget position | cube's dark edge y 140; ▾ button box y 218–246; menu border top 246, left 1681, right 1904 | everything 1 px lower; menu 1 px further left | ✅ widget top 14 px (was 15); menu anchored 1 px further right |
| VC3 | ▾ caret next to the cube icon | 8×4 CSS caret `#333` (dark core 6 px wide) | 9 px `ui-caret-down` glyph (core 3 px) | ✅ `drawing-tool-caret` 12 px `#333` |
| VC4 | ▾ button while its menu is open | `#e5e5e5` (229) | stays at the hover `#eaeaea` | ✅ pressed `#e5e5e5` while the menu is open (`os_menu::is_open_menu`) |
| VC5 | Camera menu border | 1 px `#d7d7d7` (215) | `#ccc` (204), the right-click menu colour | ✅ `#d7d7d7` for the camera menu and its submenus |
| VC6 | Submenus (Shaded…, Hidden edges…, Tangent edges…) | ✓ at +14, text at +29 from the border, like the main menu | ✓ +17, text +36 (right-click menu insets) | ✅ submenus inherit the camera menu's insets |
| VC7 | Menu font | Flama 12 px `#555` | our UI font 12 px | ✗ app-wide font (Open Sans / Flama not bundled; see theme-layout.md) |
| VC8 | Assembly camera menu | (not compared yet) | a shorter dropdown (`assembly_view::render_camera_menu`, chevron caret) | ✗ to compare (assembly) |
| VC9 | Esc with the camera menu open | closes it | did nothing: the viewport under the ▾ button took the click and the focus back from the menu | ✅ the button's container occludes the viewport |

## Feature-row tooltips (lead 10-22; live behaviours "Feature-row hover tooltips: Fillet / Chamfer / Shell")

| # | What | Live | Ours | Status |
|---|---|---|---|---|
| RT1 | Parents after "<-" | only feature-level inputs: sketches; a Derived's source studio. Fillet, Chamfer, Shell, Boolean: the name alone | parents through part references too ("Boolean 1 <- Extrude 1", "Fillet 1 <- Extrude 1") | ✅ sketches only, plus "Derived 1 <- <source studio>" |
| RT2 | Sketch, plane and mate connector rows (live ui 10-22 d) | `<name>[ [Type]][ (Hidden)][ is not fully defined \| did not regenerate properly: <msg>]`; e.g. "Sketch a underdefined is not fully defined", "Sketch 2 (Hidden)", "Plane to hide (Hidden)", "MC to hide [Mate connector] (Hidden)", "Sketch d overdefined did not regenerate properly: Sketch could not be solved." | "Sketch 1" for every sketch; no (Hidden); no [Type] tag | ✅ one rule for every row: [Type] when the name doesn't start with the type's name (also gives "<callout> [Hole]"), " (Hidden)", the sketch's " is not fully defined", errors after the head; inputs typed too ("<- Path [Sketch]") |

## 2. Tool search (live ui 10-22 c; menus/600–615)

| # | What | Live | Ours | Status |
|---|---|---|---|---|
| TS1 | "Section", "Reflection", "analysis", "Zebra", "Measure", "Mass" | Part Studio: only "Section" finds anything ("Intersection curve", fuzzy); the rest say "No items match your search". Assembly: nothing for all six | the same (checked in the app 10-22) | ✅ matches (icons2's Round 14 note was wrong) |
| TS2 | Tool descriptions (right pane; also the expanded toolbar tooltip) | e.g. Intersection curve: "Create a curve at the intersection of 2 or more surfaces or faces. The selections must intersect." | `tool_text.rs`, partly paraphrased from the help glossary | ✅ word for word from live (b79c7655; 153 of 157 single-line texts match dom/ui-e-tool-descriptions-live.json verbatim, the 4 frame tools' numbered lists as lists) |


## 3. Keyboard shortcuts modal (Shift+/; live ui 10-22 c, menus/616–619)

| # | What | Live | Ours | Status |
|---|---|---|---|---|
| KS1 | ↗ pop-out next to × | `a.osx-popout-link` (14 px arrow-up-right, no tooltip) opens a full-size "Keyboard shortcuts" page: no tabs or search, every category at once in two columns, the legend bar, a footer; the modal stays open | missing | ✅ `shortcuts_page.rs` (97c4895c): its own window with the two-column page, legend and footer, the modal staying open; now opened at the opener's bounds (live: a new tab, 1919×1079), and the lock right after the name as live (4 px blank, was 8; also in the modal: 616 / 2460-1) |

## 4. Opening documents (qa4, lead 10-23)

| # | What | Live | Ours | Status |
|---|---|---|---|---|
| OD1 | A document next to the -wal/-shm of an earlier file of the same name | (Onshape has no local files; the rule is: never crash, show an error) | panic in main.rs ("could not open document: unknown branch …") | ✅ the store probes the document read-only with and without the log; a log it can't be read with but can be read without is foreign and moved aside (kept as `*-wal.stale`). `create` moves leftovers aside too. Tests: `foreign_side_files_are_moved_aside_on_open`, `a_log_that_belongs_to_the_document_is_kept`, `creating_over_leftover_side_files_moves_them_aside` |
| OD2 | Any document named on the command line that can't be opened | — | `expect()` panic | ✅ the Documents page opens with the error bubble "Could not open document: …" (as for a failed open from the page) |

## 5. Assembly dialogs (qa4)

| # | What | Live | Ours | Status |
|---|---|---|---|---|
| AD1 | Assembly linear/circular pattern: Esc on a focused, unedited field | cancels the dialog (feature-dialog keys) | nothing (the dialog had no "FeatureDialog" key context) | ✅ key context + Esc ✕, Enter ✓, Tab / Shift+Tab through the fields |

## 6. Measure with planes (qa4 R1)

| # | What | Live | Ours | Status |
|---|---|---|---|---|
| R1 | Two default planes (or a plane and an edge) selected | parts unchanged (part-studio/81: "Perp planes angle: 90.000 deg" in the status bar only) | parts see-through with dark outlines | ✅ cause: the measure treats a default plane as a 2 000 000 mm face, so its Min/Max dist lines end ±1e6 mm out; those lines counted in the scene bounds, the fitted depth range grew, and the faces' depth bias let edges and silhouettes show through. Measure lines are now left out of the bounds (`Instance::no_bounds`) |
| MS1 | Measurements of default planes (live ui 10-23 a, dom/ui-measure-default-planes.json) | only angles and distances: Top + Front / Top + Right "Perp planes angle: 90.000 deg" alone; Top + edge: Perp line and plane angle, Curvature deviation, Min dist, the edge's Length; Top + parallel face: Parallel dist, the face's Area, Min dist; Top alone: "No measurements available for current selection." | Top + Front: Perp planes angle 90°, **Area 8e12 mm², Max dist 2 449 490 mm, Min dist 0** | ✅ a plane adds no Area, Length, Max dist or positions; with only planes no Min dist unless parallel; a lone plane measures nothing and the panel says so (test `default_planes_add_only_angles_and_distances`; checked in the app 10-23) |
| MS2 | Dashed measure lines | none in the graphics; a distance row's line shows while that row is hovered in the panel | the Min/Max dist lines of every distance, always | ✅ only the hovered distance row's line |

## 7. Right-side panels (live part-studio/77–78; dom/panel-appearance.txt; measured at 1×)

| # | What | Live | Ours | Status |
|---|---|---|---|---|
| RP0 | Panel strip, closed and open | x 1887 (closed) / 1169 (panel open), button rows 471–636 | the same | ✅ matches |
| RP1 | Appearances: tab row under the title | 4 px top padding: tabs 111–137, rows from 147 | tabs 107–133, rows from 143 | ✅ +4 px |
| RP2 | Configuration (empty): illustration and help text | ink 252–401; lines 428, 449, 469, 490 (20.8 px) | ink 251–406; lines 434…494 (20 px) | ✅ 154 px illustration, 20.8 px lines (checked 10-23: within 1 px) |
| RP3 | Custom tables (empty) | ink from 252; lines 428, 449, 470, 491, 511, 532 | ink from 249; lines from 425, 20 px pitch | ✅ +3 px, 20.8 px lines (checked 10-23: within 1 px) |
| RP4 | Inspection table (empty) | header text 125–134; tool band 154–199 (46 px, full width, 1 px #d7d7d7 line at 198), icons 170–189 from x 1217; illustration ink 304–453 (170 × 149); text from 480 | header 122–131; band 142–177 (36 px, inset), no line; icons 150–169 from 1225; ink 246–359 (128 × 113); text from 389 | ✅ band, header, 172 px illustration (checked 10-23: within 1 px) |
| RP5 | Variable table (empty) | ink 312–461 (168 × 149); two lines from 488, the second ending in the link **"Learn more about variable tables."** | ink 272–429 (178 × 157); text from 468; no link | ✅ 171 px illustration lower, the link (checked 10-23: within 1 px) |
| RP6 | Custom tables illustration art | no inner line | an extra vertical line in front of the cube | ✗ icon art (icons2) |
| RP7 | Help-text width | Flama/Open Sans | our UI font is a little narrower | ✗ app-wide font (VC7) |

## 8. Mass and section properties (live part-studio/80; dom/dialog-mass-properties.txt)

| # | What | Live | Ours | Status |
|---|---|---|---|---|
| MP1 | Dialog rows (borders, underlines) | 107, 133, 141–182, 188, 214, 276, 305, 333, 392, 423, 454, 513, 541, 569 | the same, except the empty mate connector field ends at 213 and its text sits 2 px high | ✅ empty field caption 25 px with 2 px top padding |
| MP2 | Parts to measure prefilled from the selection | Part 1 (red: no material) | the same | ✅ matches |
| MP3 | Text baselines of the labels | — | within 1–2 px (font metrics) | ✗ app-wide font (VC7) |
| MP4 | Face tab (live skb/851–854) | Faces to measure, Mate connector for reference frame, ☐ Show calculation variance; Surface area, Perimeter; Area moment of inertia (mm⁴) Lxx Lxy / Lyx Lyy; Polar moment of inertia (mm⁴) Lzz (left half); no Override boxes; values to 3 decimals, a zero as "0"; faces that aren't coplanar: everything blank, the title red, tooltip "Face selections are not coplanar" | a list of value rows: Faces (count), Surface area, Perimeter, Centroid | ✅ live's layout; moments about the faces' centroid in the face plane's axes (10 × 10 face: 833.333 / 0 / 833.333, Lzz 1666.667; test `rectangle_moments_match_live` with live's 40 × 20 numbers) |

## 9. Measure panel (live part-studio/82; measured at 1×)

| # | What | Live | Ours | Status |
|---|---|---|---|---|
| ME1 | Position | x 1699–1918, bottom 1046 (2 px over the viewport's bottom), over the panel strip and the three bottom-right buttons | bottom 36 px up, right 40; then flush, under the buttons | ✅ docked 2 px up, drawn after the buttons |
| ME2 | Title × | thin `#999` ×, 11 px of ink | the dialogs' red ✕ | ✅ grey panel × |
| ME3 | Field | 5 px under the title line; chip × 7 px `#666` ink in a 20 px cell 5 px from the border, red on hover; every second chip row under a 35 % white overlay (woolsthorpe.css `.os-param-selection-list-entry:nth-of-type(2n)`), no hover background | 2 px; a 5 px text × at 9 px; no stripes | ✅ (the feature dialogs had the overlay on hover instead of on every second row: fixed there too) |
| ME4 | Selects | label 12 px `#666`; the select fills the row (underline to 5 px from the edge), 14 px black, a 6 × 5 px `#777` ▼ 7 px from the end; underlines 31, 57, 83 px under the field | shrink-wrapped, 13.33 px `#333`, a 10 px caret next to the text; 26, 52, 78 | ✅ |
| ME5 | Results | first row 31 px under the checkbox text; labels, values and units `#666` | 26 px; `#333` | ✅ |
| ME6 | Footer | 1 px `#d7d7d7` line, 21 px `#eee` band, 12 px ? | the band only, hidden under the buttons | ✅ |
| ME7 | Chip × | removes that entry from the selection | no action | ✅ |
| ME8 | Assembly Measure panel (`measure_panel.rs`, shared code from icons2 Round 14) | as the Part Studio's | docked 36 px up / 40 px in, the old field, selects and colours | ✗ assembly (reported to the lead) |
| ME9 | Nothing to measure (a lone plane; live part-studio/2244) | a 202 px info box (`#def1fb`, 1 px `#b2ddf6`) 5 px in, 14 px `#094174` on 21 px lines, two lines | a 12 px grey line, cut off at the panel edge | ✅ box exact (x/y to the pixel); the line breaks before "for" instead of after it because our UI font is wider (VC7) |

## qa5 re-run pass 21 (2026-10-23, app = target-lead 17:10, main 88fbf8f)
| Row | Ours | Status |
|---|---|---|
| VC8 Assembly camera menu | Isometric, Dimetric, Trimetric \| Zoom to fit \| Perspective view, Show mates; larger rows, chevron caret; the Part Studio menu (v01) has the full live list | ? live capture requested ("qa5 (10-23 pass 21, ui VC8)") |
| VC7 menu font, KS1 shortcuts pop-out page | not local (app-wide font / new window) | ✗ VC7 → ui (qa5-routing.md); ✅ KS1 (row KS1 above) |
| VC8 (live answered, behaviours "qa5 assembly camera menu") | live = Part Studio menu minus "Orient normal to sketch on edit" / "Highlight boundary edges", no "Show mates". qa5: "Show mates" removed, "Section view…" added; the rest needs assembly features (render modes, named / previous views, graphics prefs, zoom to window, high quality) → qa5-routing ui | 🔧 partly / ✗ ui |

## 10. Sheet metal table and flat view (SM4; live "qa5 sm" (8), qa5/sm-091…105, dark/dk-973, behaviours "ui (10-24 a)")

| # | What | Live | Ours (before) | Status |
|---|---|---|---|---|
| SM4a | Layout | no title tab, no footer; "Sheet metal context:" + a 220 × 28 select of the models; collapsible "⌄ Bends" and "⌄ Other joints" tables (37 px #eaeaea title rows, 16 px / 500); the flat view under a 9 px row-resize splitter (1 px #d7d7d7, 50 %, tables ≥ 140 px, flat ≥ 210 px, kept as a share) | a "Sheet metal" tab, the part name, a white line drawing, summary rows, a Dir / Angle / Radius / Allow table, Export DXF / SVG buttons | ✅ (grid lines and columns at live's pixels) |
| SM4b | Bends table | # (60 px, #eaeaea bold) · Name · Radius(mm) · Angle(deg) · Bend direction · K Factor (140 px; Radius and K Factor white, the rest #f5f5f5); headers 700 14 px; #aaa grid, 34 px rows; Chrome's horizontal scrollbar | — | ✅ |
| SM4c | Joint names | one letter sequence over all the model's joints in feature order, a feature's bends before its rips: Bend / Hem / Joint + letter | — | ✅ (live re-sequences after a joint changes: not reproduced) |
| SM4d | Hover | the cell #eee, its row header and column header #d7d7d7 (dark #666) | — | ✅ |
| SM4e | Editing | double-click Radius / K Factor: 2 px #1651b0 outline, the number selected; Enter / Tab adds "Modify joint N" at the rollback bar, or before the model's Finish sheet metal model when that comes first; a second edit of the bend changes that Modify joint (live "ui (10-24 b)"); Esc cancels | — | ✅ (our Modify joint fails to re-bend with a new radius: "The walls must meet in one sharp corner to make a bend", model / fs) |
| SM4f | Flat view | its own graphics area: the flat as a shaded solid seen from Top, dark centre lines on the bends, a view cube without ▾; wheel zoom, right-drag rotate; empty-space right-click Zoom to fit / Isometric; the main window's Measure and Analysis tools at its bottom right | — | ✅ |
| SM4g | Row selection and hover | "#" click selects the row (2 px #1651b0 outline) and its bend in the graphics and in the flat view (selection orange); a cell click outlines the cell; hovering a row pre-highlights its bend in the 3D view and outlines its zone in the flat view (#FFC685, 3 px) | — | ✅ |
| SM4h | Bends row right-click | Move up / Move down (not on the first / last row; only the rows move, no feature, kept with the document), separator, "Convert Bend A to rip" (a Modify joint Rip / Edge joint) | — | ✅ (PartStudio.bend_order; our model can't rip a bend yet: "The walls must meet in a sharp corner to rip", fs) |
| SM4i | Other joints Type ▾ (Bend / Rip) and Style ▾ (Edge joint / Butt joint - Direction 1 / 2) | custom dropdowns; a change edits the joint | static cells | ✗ needs model support: Modify joint can't restyle an existing rip or turn it into a bend from the table (fs) |
| SM4j | Two models | the context select switches the tables and the flat; "the other part dark grey" is only Part 2's default colour (#a5a5a5, ui-sm4-2c / 2e) | — | ✅ nothing to do |
| SM4k | Flat-face hover | the face under the pointer outlined in #FFC685 (ui-sm4-4a); the flat has a face per wall and per bend zone, with solid edges along both sides of each zone (sm-091) | — | ✅ outline; ✗ our flat pattern is one face without the zone edges (model / fs), so the whole flat lights up |
| SM4l | Flat-part right-click (sm-111) | Edit <model>…, Edit <sketch>…, Show dimensions, Show dependencies of <model>… \| New sketch… \| Copy Flat pattern of <part> \| Create Drawing of Flat pattern of <part>…, Export DXF/DWG of Flat pattern of <part>… \| Select other… \| Add comment \| Zoom to fit, Zoom to selection, View normal to \| Delete <model>; about 250 px wide, long labels cut with … | — | ✅ Edit, Edit sketch, Show dimensions, Show dependencies, Export DXF/DWG, Zoom to fit, View normal to, Delete (Add comment is cloud); ✗ New sketch…, Copy Flat pattern…, Create Drawing of Flat pattern…, Select other…, Zoom to selection (need the flat face's model face), label cut-off |
| SM4m | Joint letters after a change | the changed joint moves to the end and every letter is re-sequenced ("ui (10-24 a)" (1)) | letters by feature order only | ✗ |

## qa5 MEAS1 (2026-10-24)
| MEAS1 / MS1 | Top row alone: ours "Area: 4000000000000.000 mm²" | live (behaviours "ui (10-23 a)", case e): nothing in the status bar; panel "No measurements available for current selection." Fixed on ready/ui 6f16c117 (not on main yet, c59c5af9); re-check when it lands | ui |
| MEAS1 / MS1 (re-check 2026-10-24 07:20, main 78109ebc, target-lead 07:07) | Top row alone: nothing in the status bar (live case e) | ✅ fixed (ui) |

## 11. Context menus in the graphics (live behaviours "Right-click menus", "ui (10-24 c)"; menus/rmb-ps-*.png)

| # | What | Live | Ours (before) | Status |
|---|---|---|---|---|
| CM1 | Face, edge, part (parts list), empty space | item for item as live (Add comment is cloud) | the same | ✅ matches (checked 10-24) |
| CM2 | Default plane in the graphics | … Section view… \| **Select ▸** (the face list) · Select other… \| … | no Select ▸ | ✅ |
| CM3 | Vertex | Edit Extrude 1…, Edit Sketch 1…, Show dimensions, Show dependencies… \| Hide Part 1, Isolate…, Make transparent… \| Copy Part 1 \| Create Drawing…, Export… \| Select ▸ (Create selection…), Select other… \| Zoom to fit \| Delete Part 1…, Delete Extrude 1 \| Assign material…, Edit appearance… (no Section view, no Zoom to selection) | no Edit / Show / Delete Extrude items (no creating feature for a vertex), Section view… and Zoom to selection extra, no Select ▸ | ✅ |
| CM4 | Origin | Hide \| Select ▸ (Create selection…), Select other… \| Zoom to fit | no Hide, no Select ▸ / Select other…, Zoom to selection extra | ✅ |
| CM5 | Mate connector | Edit…, Show dependencies… \| New sketch… \| Hide, Section view… \| Select ▸ (Create selection…), Select other… \| Zoom to fit, View normal to \| Delete Mate connector 1 | the part-entity menu without most items | ✅ (not checked in the app: no connector in the test document) |

## 12. Feature dialog rows (live dialogs/extrude-01, shell-01, dom/dialog-extrude-new; measured at 1x)

| # | What | Live | Ours (before) | Status |
|---|---|---|---|---|
| FD1 | Tolerant number rows (Depth with the ± toggle) | label text 239–248, › chevron 2 px under the row's middle, underline 244 | label 3 px and chevron 2 px higher (underline the same) | ✅ |
| FD2 | Checkbox / group rows | `os-param-checkbox-label` 27.333 px (17.333 line + 2 × 5 px): tops step 28, 27, 27, 28 | 27 px each: the rows below Direction drift 1–2 px up | ✅ rows 27.333 px; within 1–2 px at the dialog's bottom (live's rounding) |
| FD3 | A new dialog whose first field is a single pick the preselection filled (Top selected, then Sketch) | the field inactive: white, grey border (sketch/01) | active (blue), keyboard focus on it | ✅ `release_prefilled_single_pick`; with nothing selected it stays active (sketch/00) |
| FD4 | Chip × in selection fields | a 20 px delete cell 5 px in (woolsthorpe.css): the ink 12–13 px from the field's border (extrude-03 443–450 in 249–462; sketch/01 417–423) | the bare glyph 5 px from the border | ✅ (sketch/01: 417–423, as live) |
| FD5 | Ticking a group checkbox (EXT-DIRECTION, qa4 r5 s7) | the first newly shown empty selection field becomes active, unless the active one is empty or in error (bundle updateParameterVisibility → setNewQLVFocus); an empty Direction fails ("Select Extrude direction.", std extrude.fs): title red, no preview; the group chevron red #bd3039 while a shown field is in error (os-param-group-expander is-error), else #666 ticked / #d7d7d7 not (extrude-22/23) | the field stayed white, title black, chevron grey (#555 when ticked); an empty direction silently used the sketch normal | ✅ generic in `FeatureDialog::set_value`; Fillet › Smooth fillet corners with an edge picked activates "Corners to exclude" (optional, chevron grey) |
| FD6 | A failing tool keeps the automatic operation | std booleanStepEditLogic leaves operation and merge scope alone when the tool throws | Add + "Part 1" flipped to New (no scope) while Direction was empty | ✅ `boolean_edit_logic` skips tool errors (only a failing merge scope is judged) |
| FD7 | Auto-advance after a single pick | the next visible selection field, filled or not (getNextFocusableParameter; live s7: direction picked → Merge scope "Part 1" active); none → the focus stays (Sketch alone lets its plane field go) | only to the next empty field, else nothing active | ✅ |
| FD8 | Second end Depth (EXT-2ND-DEPTH) | no › and no ± (std extrudeCommon.fs: not CAN_BE_TOLERANT; extrude-22) | both | ✅ `end_condition` |
| FD9 | Hovering a chip of a pick the preview consumed (CHIP-HOVER) | the entity yellow-green #f2ff80 | stayed amber / light orange (the consumed-pick overlay ignored the hover) | ✅ `consumed_picks_overlay` draws it #f2ff80 |
| FL1 | Opening / closing a folder (FOLDER-EXPAND-UNDO, qa4 r5 s8) | view state, not an undo step (live s8 (7)) | an undo step ("Toggle folder") | ✅ `UiState::folder_open` over the document's state (a new folder starts closed) |
| FL2 | Type tags in tooltips (TOOLTIP-PARENT) | " [Type]" unless the name contains the type, case-insensitively (bundle: `(" "+name.toLowerCase()+" ").indexOf(type)`): "Boss 2 [Extrude] <- Boss sketch 2", "<- Path [Sketch]" | tagged unless the name *started* with the type ("Boss sketch 2 [Sketch]") | ✅ `needs_type_tag` (test) |
| FL3 | A suppressed feature's menu (SUPPRESSED-MENU) | Rename \| Add selection to folder… \| Unsuppress, Dynamic suppression \| Roll to here \| Delete (+ Add comment, cloud) | the full menu (Edit…, Show dimensions, Zoom to selection, Show dependencies…) | ✅ (separators asked: live "ui (10-25 b)") |
| FL4 | Select other (SELECT-OTHER, qa4 r5 s9; bundle select-other-dialog, dom/select-other-popup.txt) | 220 × 149 at the pointer + (100, −25), in the graphics area; "Select other" 13 px / 500 + red ✕; 1 px #d7d7d7 list (#1651b0 under the pointer), 24 px rows, every second 35 % white, the current one **bold**, slim scrollbar on hover; "?" row. ` opens on the 2nd entry (Shift+`: the last), from a menu on none with a yellow point (rgb(255,194,14), 9 px) while the pointer is out of the 8 px click zone; ↑ / ↓ / ` step; Enter / a click in the zone take it, a click elsewhere only closes; hovering a row pre-highlights its entity; "(No selections found)" | a plain list: grey Edge/Face column, the current row filled blue, hover grey, no ✕ / "?", a toast when nothing is under the cursor, a click elsewhere also selected | ✅ Part Studio (sketch mode's own list is psb's) |
| FL5 | Folders (FOLDER-NAME-INLINE, FOLDER-MENU; live "ui (10-25 b)") | naming in the "Folder name" popup (as ours; the s8 "inline" note was wrong); the folder takes the run from the first to the last selected feature in place, collapsed, its row related-highlighted (#ECF0F4 + 1 px #90CEF1 under it) until the next click; a click selects the folder row, only the chevron opens / closes, a double-click does nothing; menu Rename \| Add selection to folder… \| Suppress (Unsuppress), Dynamic suppression ▸ Add expression… \| Roll to here \| Expand ▸ (Expand selected, Expand all), Collapse ▸ (Collapse selected, Collapse all) \| Delete, Unpack folder; a suppressed folder grey italic struck through | selected features gathered next to the first (reordering); a click toggled, a double-click renamed; menu Expand, Rename \| Suppress, Unsuppress \| Expand all, Collapse all \| Unpack folder | ✅ `PartStudio::new_folder` (run, test), `wrap_folder`, `delete_folder_tree` (test); folder row and menu; a folder's Dynamic suppression ▸ Add expression… writes the expression onto every feature in it, none on the folder (live "ui (10-25 c)": no folder node in REST; the children keep it on unpack); the folder shows only the aggregate (grey struck through, tooltip "New folder (Suppressed)", no glyph) and offers Edit dynamic suppression ▸ Remove expression; the children a grey italic fx (tooltip "Dynamic suppression: {expr}"), row tooltip "… (Suppressed with expression)", Edit dynamic suppression ▸ Edit expression…, Remove expression |
| FL6 | Feature rename (qa4 r5 s8 (5)) | inline input, all selected; Enter or a click elsewhere commits, Esc cancels; an empty name returns to the default name ("Fillet 2"); suppressed rows' tooltips "Extrude 1 (Suppressed)", "Fillet 3 (Suppressed with expression)" | Esc fell through (the blur could commit); an empty name was ignored; no "(Suppressed)" tooltip | ✅ `rename_feature` (empty → `next_name`, test), Esc on the input, row tooltips |
| FL7 | Typing in a dialog number field (EDIT-TYPING-PREVIEW, qa4 r5 s2) | the preview re-runs when the field commits (Tab, a click away, Enter), not per keystroke (live s2: zero redraws while typing) | every keystroke re-ran the preview | ✅ `FeatureDialog::flush_typed` |
| FL8 | Roll to here / Roll to end (ROLL-LAST, qa4 r5 s2 rollback follow-up) | Roll to here unless the bar already sits right after the feature (on the rolled-back last one it brings the bar back); Roll to end while the bar isn't at the end; a rolled-back feature's menu is Rename \| Add selection to folder… \| Suppress, Dynamic suppression ▸ \| Roll to here, Roll to end \| Delete (no Edit…, Zoom to selection, Show dependencies…, appearance) | Roll to here hidden on the last feature whatever the bar; no Roll to end; rolled-back features had the full menu | ✅ features (folders: Roll to here) |
| FL9 | Dropping a feature on a folder row (qa4 r5 s8 (6)) | it becomes the folder's first child, moved to the folder's place; the row lights up during the drag; the folder's tooltip sits beside the row ("New folder" / "New folder (Suppressed)") | no drop target on folder rows; the folder tooltip under the row | ✅ `drop_into_folder` (same order check as a drop between rows) |
