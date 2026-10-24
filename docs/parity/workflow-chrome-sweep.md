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
| TS2 | Tool descriptions (right pane; also the expanded toolbar tooltip) | e.g. Intersection curve: "Create a curve at the intersection of 2 or more surfaces or faces. The selections must intersect." | `tool_text.rs`, partly paraphrased from the help glossary ("Create a curve where two or more surfaces or faces intersect.") | ? exact texts for every tool asked (REQUESTS "ui (10-22 e)"); then replace them all |


## 3. Keyboard shortcuts modal (Shift+/; live ui 10-22 c, menus/616–619)

| # | What | Live | Ours | Status |
|---|---|---|---|---|
| KS1 | ↗ pop-out next to × | `a.osx-popout-link` (14 px arrow-up-right, no tooltip) opens a full-size "Keyboard shortcuts" page: no tabs or search, every category at once in two columns, the legend bar, a footer; the modal stays open | missing | ✗ next: a separate window with that page |

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
| MS1 | Measurements of default planes | ? (asked: REQUESTS "ui (10-23 a)") | Top + Front: Perp planes angle 90°, **Area 8e12 mm², Max dist 2 449 490 mm, Min dist 0** | ? |

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

## qa5 re-run pass 21 (2026-10-23, app = target-lead 17:10, main 88fbf8f)
| Row | Ours | Status |
|---|---|---|
| VC8 Assembly camera menu | Isometric, Dimetric, Trimetric \| Zoom to fit \| Perspective view, Show mates; larger rows, chevron caret; the Part Studio menu (v01) has the full live list | ? live capture requested ("qa5 (10-23 pass 21, ui VC8)") |
| VC7 menu font, KS1 shortcuts pop-out page | not local (app-wide font / new window) | ✗ → ui (qa5-routing.md) |
| VC8 (live answered, behaviours "qa5 assembly camera menu") | live = Part Studio menu minus "Orient normal to sketch on edit" / "Highlight boundary edges", no "Show mates". qa5: "Show mates" removed, "Section view…" added; the rest needs assembly features (render modes, named / previous views, graphics prefs, zoom to window, high quality) → qa5-routing ui | 🔧 partly / ✗ ui |
