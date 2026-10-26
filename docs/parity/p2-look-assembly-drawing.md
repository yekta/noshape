# P2 look: assembly and drawing UI vs live (ui2)

ui2, 2026-10-24, from the lead's P2 list (PRIORITIES.md). ui2 owns the look, skb the
assembly behaviour, asb the drawing behaviour. App = the ui2 build of 10-24 13:50 for the
first pass, live = assembly/01–04, 1362, 2219–2226; drawing/12, 20–22, 122, 170a, 726, 1340.
Status: ✅ matches, ✗ differs, 🔧 fixed on agent/ui2 (to check on the next build), ? live
asked, → owner.

## Assembly

| # | Item | Live | Ours | Status |
|---|---|---|---|---|
| AL1 | Insert dialog (open, tabs, document header, Part Studios / Assemblies, search, filters, tree, "Inserted: N", undo row) | assembly/01 | within 1 px | ✅ |
| AL2 | Empty instance list | Assembly 1, Origin, Mate features (0); no hint | an extra grey "Insert parts and assemblies (I) to start." (not an Onshape string) | ✅ removed (checked on the 17:19 build) |
| AL3 | Insert dialog row hover | light-blue row + the name in a tooltip (assembly/03) | row only | ✅ tooltip added |
| AL4 | Insert ghost | the part follows the pointer, amber (assembly/04) | same | ✅ (placement → skb) |
| AL5 | New mate's row while its dialog is open | "Revolute 1" in Mate features, selected, red with the error badge until both connectors are picked; the count includes it (assembly/2219) | no row until ✓ | ✅ (17:19 build: red row + badge, count 1, normal once both are picked) |
| AL6 | Mate connectors box once both are picked | stays active: blue frame, light-blue fill (assembly/2220) | plain | ✅ |
| AL7 | New mate's indicator while its dialog is open | the mate glyph at the connectors (2220) | only the triads | ✅ |
| AL8 | Mate dialog fields, HUD (type select, flip, reorient, Solve, ✓ ✕) | 2219/2220 | within 1 px; "Simulation connection" ⊘ (simulation) | ✅ |
| AL9 | BOM pane (header, BOM type, Add column, ⋯, table) | assembly/1362 | within 1–2 px | ✅ |

## Drawing

| # | Item | Live | Ours | Status |
|---|---|---|---|---|
| DL1 | Insert view tool | opens the empty Insert view panel; [Insert] opens "Select a part or assembly" (drawing/122, 170a; behaviours 1473, 3043) | opened the chooser at once | ✅ panel first |
| DL2 | Empty Insert view panel | fields greyed with grey ▼; View scale focused (2 px blue underline, dark ▼) | all alike, blue carets | ✅ |
| DL3 | Active drawing tool button | `#d1dde6` (drawing/20–22, 127) | `#eaeaea` (the Part Studio's) | ✅ |
| DL4 | Section view dialog label | focused with "A" selected, 2 px underline (drawing/20) | not focused, 1 px, 2 px lower | 🔧 focus + 2 px ✅; the selection and Esc (the focused field swallowed it) fixed after the 17:19 build |
| DL5 | Section view dialog checkboxes | `#ddd` 12 px boxes 2 px up / left | the kit's darker box | ✅ every drawing dialog and panel (`drawing_chrome::DrCheckbox`): the floating dialogs, view and sheet properties the 12 px `#ddd` box (live hides the native input: dom drawing-section/detail/weld/note/flat-view-properties); the Drawing properties and styles panels and the Reference manager Chrome's native 13 px box (`#767676` / `#0075ff`, live 240, 294) |
| DL6 | Dimension "±0.1" launcher | a small button beside a placed / selected dimension (drawing/12) | none | ✅ (live answer "ui2 (10-24 15:25) dimension launcher"): shows 0.8 s after a dimension is selected, bottom-left at the text ink's top-right + (14, −14) (measured +15/−14), 0.6 s hover or click opens the palette at its top-left + (−15, +9), leaving that palette closes it and deselects; our own 32 px icon after live's layout (its anti-aliased strokes not copied: 113 of 1024 px differ) |
| DL8 | Detail view dialog | rows: View scale 3 px lower after the field; checkbox rows 29 px apart (drawing/127) | 3 px higher; the checkboxes 2 px left | ✅ rows at live's 181 / 207 / 233, boxes at 254 / 280 and x 47, labels at x 68 (pixel-checked) |
| DL9 | Drawing properties panel (Units and precision) | tabs 42 px down; selects a white 164 × 25 field on a #ddd line, 2 px below the row top, #555 value ink 7 px in, 8 px caret 6 px from the right; groups 8 px above their first row, 4 px below the last; footer: template row 4 px under the rule, 26 px DWT button 8 px in, 41 px Lock row (drawing/1710) | tabs 1 px low; ghost buttons with a 6 px caret; rows 3 px low, fields 1 px left; footer 5 px high, Lock checkbox 4 px left | ✅ every row edge from the tabs to the last group identical to live (119…772), field / value / caret / label within 1 px, footer exact; checkbox rows (box 9 px in, 13.33 px #666 label) exact but the label 1 px left (pixel-checked). Rows no longer shrink when a tab overflows (Dimensions rows were ~27 px apart) |
| DL10 | Drawing properties, other tabs | each group a table (1710): its controls from its own column (Dimensions: 123 / 153 / 165 px), selects as wide as the column (220 / 197 / 186), rows 30–33 px; number fields white on a #ddd line like the selects (▾ / spinbox carets the selects' #777 ▾), colours a 26 px square, no Bold / Italic (only the note editor and the Views tab's labels have them), Chrome's 15 px scrollbar when the tab overflows | B / I toggles, every control right-aligned at 164 px, boxed rounded number fields, a 28 × 16 swatch with the colour's name, no scrollbar | Dimensions, Annotations ✅ (every select, field, swatch and group rule at live's x / y; values, carets, spinbox at live's pixels; Annotations below Center of mass, past live's capture, keep plain rows). The panel's #d1d1d1 left and #dbdbdb right lines as live. Views: Projection angle and Insert view defaults at live's columns, in live's order, Scale label / Auto centermarks / Auto centerlines as On / Off selects. Tables: All tables' 100 px spinboxes at 181. Inspection: column 105, 253 px selects, Bold / Italic on the Text row (26 px, pressed #cfdbe5 with a #1b5faa line). Formats: a Lock group (Unlocked / Locked select), colours at 111. Construction geometry: Extended lines first, 181 px spinboxes at 149 |
| DL11 | Arrowhead picker | the style's glyph and a ▾ (drawing/1710); its menu lists the glyphs only (drawing/162) | a text select | glyph control ✅ (Dimensions; drawn in code, 18 × 9 px); the menu still lists names: open |
| DL12 | Drawing properties › Views content | All views: Visible edges (thickness select + colour), View labels (font; text height, Bold, Italic, colour); Insert view defaults: Threads (Parts), Bend lines (Parts), Bend notes (Parts); Hidden / Tangent edges: Thickness (select + colour) and Dashes (spinbox) (drawing/1710) | none of these; Hidden / Tangent edges have only a Color row | ✗ behaviour (new drawing properties applied to views): → lead / asb. Same for Tables › BOM / hole / general tables groups (line thickness, font, header and content row text with Bold / Italic / colour) Inspection › Text colour and Solid fill colour, Formats › each group's Thickness, Construction geometry › Created in drawing, Part Studio sketches, Centermarks / Centerlines Thickness and Dashes |
| DL7 | Title block field double-click | Note editor on that note (drawing/726) | nothing (17:19 build too) | ✗ → asb (behaviour) |

## For skb

- A Revolute between the two top faces of two plates (asm1: Part 1 <1> fixed, Part 1 <2>) previews with the dialog title red and "Mate error" on the Mate features folder and the assembly row, also on the 13:51 build (not from the look changes); live shows a valid preview for such picks (assembly/2220).

