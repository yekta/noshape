# Audit: every drawing and data-exchange UI surface of live against ours (dx, from 2026-11-01 14:24 UTC)

User 11-01: audit whole surfaces, not listed rows: every drawing toolbar tool's dialog field by field, every drawing
right-click menu per entity type, the import / export dialogs and their options per format, and the Notifications
panel. Sources: live DOM dumps (onshape-research/live/dom/: drawing-*, dialog-import-*, dialog-export-*, 127x-*,
context-menus-drawing*), behaviours.md. Method: a script lists every visible text, option, tooltip and placeholder of
the dumps and finds the ones absent from our sources (/root/dx-scratch/audit/strings2.py; texts built at run time and
user content are then sorted out by hand); fields, options and menus are then compared in the code and in the app.
P0 broken / data loss, P1 missing field / option / menu item or wrong behaviour, P2 wrong label / order / missing
message, P3 tooltip / prompt / look. Status: ✗ open, ✅ fixed (commit), – accepted (reason), ? live asked.

## Surfaces

| Surface | Source | Result | Gaps |
|---|---|---|---|
| Drawing dialogs and panels with live dumps (55 dumps: tools, properties, Styles, Inspection, MBD status, Sheets, tables, notes) | dom/drawing-* | 2561 texts and options, 2217 found as is; the rest user content, run-time texts, or the rows below | D1–D… |
| Import / export dialogs per format (16 dumps: Import to Onshape, STEP / SAT / x_t / PVZ / zip / STL / OBJ / 3MF / SLDPRT import; Part Studio / Assembly / Drawing / PVZ / JT / Parasolid / STEP export) | dom/dialog-import-*, dialog-export-*, 127x-* | 357 texts, 327 found; every export text and option present | X1… |
| Drawing right-click menus per entity type (25 types) | context-menus-drawing*.txt, behaviours (menu lines) | in the app 11-01 (w3/menus.noshape, a d6 copy with every annotation type; crops in dx-scratch/audit/rmb): **= live** (Add comment is cloud-only; Paste shows while something is copied, as live): empty sheet, view (projected, detail, section label: Copy on top), extension line, linear dimension, ordinate value, note, callout, hole callout, datum, centermark, surface finish (edge-attached), table, sketch line, section cut line. **Live unknown, asked**: weld, geometric tolerance, centerline, revision table, datum target, note leader line. **Differs**: M1 | M1, M2 |
| Notifications panel | behaviours (EX11 request 1544, import flow) | compared with live's description | N… |

## Gaps

| # | Surface | Item | P | Difference | Status |
|---|---|---|---|---|---|
| D1 | Datum › Datum target | Style list | P1 | live: Inside, Outside, Moveable horizontal inside / outside, Moveable rotational inside / outside (6); ours: the 3 inside ones | ✗ |
| D2 | Styles panel (a dimension) | rows | P1 | live: Font, Text height, Color, Arrowhead, Text justification, Text alignment, Text gap, **Geometry gap, Extension past line, Show first extension line, Show second extension line**, Break dimension; ours lacked the four in bold | ✅ the four rows in live's order (spinboxes in the drawing's unit; the extension-line ticks on linear dimensions); `AnnStyle` geometry_gap / extension / hide_ext; test `dimension_style_extension_lines`; app check 11-01 (mbd: first extension line hidden) |
| D3 | Hole table dialog | Collapse tags | P1 | live has ☐ Collapse tags (after Location precision); ours none | ✗ |
| D4 | Hole table dialog | Title row | P1 | live has ☐ Title row (after the Views list); ours none | ✗ |
| D5 | Hatch region dialog | Apply to | P1 | live: an "Apply to" select ("All instances in all views", other options not dumped); ours none | ? |
| D6 | Inspection table | Type of a linear dimension | P2 | live "Aligned dimension" (an aligned one); ours "Linear dimension" for all | ? (horizontal / vertical names) |
| D7 | Hole table dialog | Sheet row | P2 | live: a select (Sheet1 …); ours the name as text | ✗ |
| D8 | Hole table dialog | messages | P3 | live tooltips "Select views of a common singular part", error "All views in a hole table must be of a common singular part" | ✗ |
| D9 | Section view / Edit section | tooltips | P3 | live: "Vertical cut line", "Horizontal cut line", "Angular cut line", "Edit cutting line" (ours "Cutting line"), "Opposite direction" (ours "Flip direction"), "End type", "Depth", "Offset distance", "Select parts to exclude from cut" | ✅ the mode tooltips, "Edit cutting line", "Opposite direction"; End type / Depth / Offset distance go with D18 |
| D10 | Datum dialog | tooltips and prompt | P3 | live: "Datum style", "Click in space to add a target", "Delete A1", "Select an origin location for datum target", "Select target location for datum target", "Select an edge to define the X / Y direction for this datum target", "Flip measurement from reference line", "Rotate angle measurement", "This field cannot be empty" | ✅ the field, Flip / Rotate, row ✕ ("Delete A1") and Targets tooltips; "Datum style" and the empty-label check open |
| D11 | Hole callout panel | tooltips and «Varies» | P2 | live: "Count type: …", "Tolerance: None", "Toggle stacked visibility" tooltips; «Varies» in a value several selected callouts differ in | ✗ |
| D12 | Hatch region dialog | list rows | P3 | live: rows "Hatch 1" with tooltip "Delete hatch 1", the field tooltip "Select or create closed profile for hatch region" | ✅ "Delete hatch N" and the list's tooltip |
| D13 | Detail view / Replicate region / Adjust linestyle | prompts | P3 | "Create closed profile for detail view boundary", "No closed profile selected", "Select required edges to change their style" | ✅ the three fields' tooltips |
| D14 | Create Drawing | options and texts | P2 | live: the options' tooltips ("Drawing will automatically have recommended views", "Drawing will be empty, ready for views to be added manually"), ☐ Show Onshape drawing templates, "Creating drawing…" while it works, and "Suggested views provide only necessary views when MBD data is present." | ✗ |
| D15 | Note editor | font list | P3 | live labels the style's font "Noto Sans ( Note )" | ✗ |
| D16 | Shortcut toolbar (s) | empty state | P3 | live "No tools added" | ✗ |
| D17 | Hole cell (double-click) | Formulas | P2 | live's cell editor offers "Formulas" | ? |
| X1 | Import to Onshape (documents page) | tiles | P2 | live: "Import to a single document" / "Split into multiple documents" ("Groups subassemblies with their parts in the same document") / Combine to a single Part Studio; ours the import options without the three tiles on the start page | ✗ |
| N1 | Notifications panel | ⋯ menu | P2 | live's header has a ⋯ menu (items not recorded); ours none | ? |
| N2 | Notifications panel | import entry | P2 | live: an import's steps as green ✓ rows (Uploaded, Translated, Post-processing done, Created link to <file>, the link opening it); ours one line | ✗ |
| M1 | Right-click menus | frame / zone / title block line | P2 | live: a format line is selectable alone (orange, end / mid grips) with Paste · Copy · Paste as table · Move to ▸ · Clear selection · Zoom to fit · Delete (behaviours 1993); ours: format lines aren't picks (the empty-sheet menu; only the title block region opens its fields) | ✗ |
| M2 | Right-click menus | weld symbol order | P3 | ours: Edit… · Add leader · Copy · Move to ▸ · Paste as table (not from live; the other symbols put Paste · Copy · Paste as table before Add leader / Move to) | ? |
| A1 | Agent API | drawing_note `leader` | P3 | an edge point (the Top view's right edge) was refused, "not a vertex, circle center or edge point of a view"; a vertex works | ✗ |
| D18 | Section view › Section depth | End type, Offset distance | P1 | live: End type is a select (Up to entity / Blind, tooltip "End type"), a "Depth of section cut" Depth field, and an **Offset distance** field with its own Opposite direction; ours: Blind / Up to entity tabs and no offset | ? (offset's effect asked) |

