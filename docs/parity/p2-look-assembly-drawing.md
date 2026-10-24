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
| DL5 | Section view dialog checkboxes | `#ddd` 12 px boxes 2 px up / left | the kit's darker box | ✗ (a drawing checkbox style; next) |
| DL6 | Dimension "±0.1" launcher | a small button beside a placed / selected dimension (drawing/12) | none | ? REQUESTS "ui2 (10-24 15:25)" |
| DL7 | Title block field double-click | Note editor on that note (drawing/726) | nothing (17:19 build too) | ✗ → asb (behaviour) |

## For skb

- A Revolute between the two top faces of two plates (asm1: Part 1 <1> fixed, Part 1 <2>) previews with the dialog title red and "Mate error" on the Mate features folder and the assembly row, also on the 13:51 build (not from the look changes); live shows a valid preview for such picks (assembly/2220).
