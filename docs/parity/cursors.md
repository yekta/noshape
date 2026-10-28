# Cursors (parity checklist, agent "ui")

Source: Onshape's client bundle (`/root/onshape-research/bundle-raw/`, webgl + woolsthorpe JS and
`woolsthorpe.css`), read 2026-10-20. The graphics viewer keeps a **cursor stack**
(`pushCursor`, `popCursor`, `removeTopOccurrenceOfCursor`): the top entry is the canvas's CSS
cursor, and with an empty stack the canvas shows the **selection cursor**
(`setSelectionCountCursor`: `default`, or the count image for 1..5 / 6+ selected). Everything
outside the canvas is plain CSS on the HTML controls.

Platform variants: the client picks the "WINDOWS" images when `isWindows()`, else the
"GENERIC" (mac) ones; Linux Chrome therefore shows the mac art. Custom images are redrawn by
us (never copied); CSS standard names use the platform's own cursor (X11/Wayland theme name,
macOS `NSCursor`, Windows `IDC_*`; where the OS has none, Chrome draws its own and so do we).

**No private OS APIs (rule since v0.4.4, 21 Oct).** v0.4.3 aborted on macOS 27 when GPUI showed
`CursorStyle::Move` through the private `+[NSCursor _moveCursor]` (now raises). Chrome on macOS shows the
system's private CoreCursor art for `move`, `nwse-resize`, `nesw-resize` and `wait` (busy-but-clickable: the
arrow with the spinning pinwheel); we draw look-alikes ourselves (`vendor/gpui-pre-macos/src/cursors/`,
sources and the generator in `cursors/src/`, 24 pt with 1×/2×/3× bitmaps; hot spots: centre, centre, centre,
arrow tip 4,3) and load them with public AppKit only (`NSBitmapImageRep` reps on one `NSImage`). Not yet
compared with a real Mac (no Mac capture exists; the user's Mac is the check); `wait` is one still frame
(the system animates the pinwheel). The viewport's own `move` (cursor.rs) uses the same drawing.

Live check (2026-10-20, `/root/onshape-research/live/dom/cursors.txt` and `cursors-*.json`:
`getComputedStyle(...).cursor` in Chrome) confirms the stack below and settles the ❓ rows.
Chrome truncates fractional hot spots: `crosshair-cursor.svg 7.5 7.5` is applied at 7,7.

Status: ✅ done, 🟡 partly, ❌ not yet. "Hot" = hot spot in CSS pixels.

## Graphics area (viewer cursor stack)

| Context | Cursor | Hot | Status | Verified |
|---|---|---|---|---|
| Nothing selected, Part Studio / Assembly | `default` (arrow) | — | ✅ (X11: GPUI now sets the theme arrow on the first frame; before, a new window showed the root window's cursor) | 0.4.10+ / dd1b5037 10-27 (XFixes read-back, bin/xcur.py): 24 px arrow hot 3,1 |
| N entities selected (N = 1..5) | image: arrow + digit N (`macSelectionCursor-N`; Windows `winSelectionCursor-N`) | mac 3,3 · win 1,2 | ✅ (read back on X11: 32 px, hot 3,3; Windows art with hot 1,2 in cursor_windows) | 0.4.10+ / dd1b5037 10-27 (XFixes read-back, bin/xcur.py): "1" / "2" images 32 px hot 3,3 |
| 6 or more selected | image: arrow + "5+" (`…SelectionCursor-5plus`) | mac 3,3 · win 1,2 | ✅ | 0.4.10+ / dd1b5037 10-27 (XFixes read-back): "5+" 32 px hot 3,3 after a box over the part |
| Any sketch drawing tool active (line, rectangles, circles, arcs, polygons, splines, point, text, slot, fillet, chamfer, trim, extend, offset, use, mirror, dimension, constraints …: the sketch tool base class `getDefaultCursor()`) | image crosshair (`crosshair-cursor.svg`), fallback CSS `crosshair` | 7.5,7.5 (Chrome truncates to 7,7) | ✅ (read back: 15 px, hot 7,7) | 0.4.10+ / dd1b5037 10-27 (XFixes read-back): Line tool 15 px plus hot 7,7 |
| Sketch pattern tools (linear / circular pattern, transform: they select first) | `default` | — | ✅ | 0.4.10+ / 4c97692e 10-28 (xcur: circular pattern active, nothing picked: arrow 24 px hot 3,1) |
| Sketch linear / circular pattern, transform (`canCreatePattern`) and offset (preview shown): operation complete, nothing pre-highlighted and no dimension box open ("click to accept"; `showConfirmationCursor`) | image confirmation (`macConfirmation_Cursor` / `winConfirmation_Cursor`); while something is pre-highlighted the stack is empty, so the selection cursor shows | 1,1 | ✅ (read back: 37 px as Onshape's 36×37 file, hot 1,1; art compared with live cursors/icons2-*) | 0.4.10+ / 4c97692e 10-28 (xcur: circular pattern after picking a circle, empty canvas: 37 px confirmation art hot 1,1; over the picked circle the count cursor 32 px hot 3,3; feature list arrow) |
| Hovering a dimension's value (draggable) | `move` | — | ✅ (qa 10-27 dd1b5037: read back over a sketch dimension's value, 24 px fleur hot 11,11) | 0.4.10+ / dd1b5037 10-27 (XFixes read-back) |
| Hovering a dimension's lines | `default` | — | ✅ | 0.4.10+ / dd1b5037 10-27 (XFixes read-back): arrow on the dimension line |
| Hovering / dragging the section view arrow (and other 3D manipulators) | `default`: live (behaviours.md "Remaining cursor rows" (1)) never sets `move` or a grab cursor on 3D manipulators; there is no draggable plane surface | — | ✅ (ours keeps the canvas cursor) | 0.4.10+ / 1b81b7ca 10-28 (ps2 Section view: the offset arrow lights blue on hover, cursor the default arrow, byte-identical to the plain canvas cursor (bin/xcur.py)) |
| Ctrl held during a box selection (box deselect) | image remove-selection (`macRemoveSelectionCursor` / `winRemoveSelectionCursor`) | mac 3,3 · win 1,2 | ✅ (Part Studio and sketch box; Ctrl in a sketch box now also deselects; read back: 32 px, hot 3,3) | 0.4.10+ / dd1b5037 10-27 (XFixes read-back): arrow + "−" 32 px hot 3,3 |
| Zoom to window mode ("Drag to create zoom bounding box.") | `zoom-in` | — | ✅ (theme `zoom-in` on X11; our drawing where the theme has none) | 0.4.10+ / dd1b5037 10-27 (XFixes read-back): zoom-in 24 px hot 11,10 |
| Assembly: dragging a fixed instance | `no-drop` | — | ✅ (was not-allowed) | 0.4.10+ / dd1b5037 10-27 (XFixes: no-drop while dragging asm5's fixed plate) |
| Assembly: snapping a mate connector during a drag | image snap-mate (`macSnapMate_Cursor` / `winSnapMate_Cursor`), fallback `nesw-resize` | 7.5,7.5 | 🟡 (shown for the whole snap drag; art redrawn against live cursors/icons2-macSnapMate_Cursor: arrow at the count-cursor spot, 35 px) |  |
| Analysis color-map legend: over the legend / its color bar / dragging / tick hover / value text | `grab` / crosshair image (7,7) / `grabbing` / `ew-resize` (horizontal legend) / `pointer` | — | ✅ (legend `OpenHand`/`ClosedHand`, color map through the viewport's `overlay_cursor`) |  |
| Drawing: no tool (sheet, views, dragging a view) | `auto` (arrow) | — | ✅ | 0.4.10+ / dd1b5037 10-27 (XFixes read-back): arrow over the sheet |
| Drawing: Dimension, Ordinate, Datum, GD&T, Note, Balloon, Table, Centerline, Line, Hatch, Insert view placement | `DrawingCrosshair-01.cur` (the sketch crosshair's 15 px plus) | 7,7 | ✅ (`AppCursor::Crosshair` via `CursorSync`) | 0.4.10+ / dd1b5037 10-27 (XFixes read-back): Dimension (d): 15 px plus hot 7,7 |
| Drawing: Centermark | `DrawingCrosshairBox3-01.cur` (plus with a 7 px hollow square, arms to the edges) | 7,7 | ✅ (`AppCursor::CrosshairBox`, our drawing) | 0.4.10+ / dd1b5037 10-27 (XFixes read-back): plus with the hollow square, 15 px hot 7,7 |
| Rotate / pan / zoom drags (right / middle / Ctrl+right, Shift+middle, wheel) and left-drag box select | `default` (live: Onshape never changes the cursor for navigation) | — | ✅ (no hiding during drags) | 0.4.10+ / 41f60e47 10-28 (xcur: arrow hot 3,1) |
| Manipulator arrows, triad, rotate rings, extrude depth arrow (hover and drag) | unchanged: the dialog's current canvas cursor (live) | — | ✅ | 0.4.10+ / 1b81b7ca 10-28 (section arrow hover: cursor unchanged; triad / rings not run) |
| Feature dialog open (e.g. new Extrude, Mate connector after its first pick) | the selection-count cursor: Onshape counts the whole selection minus rollback-bar items (`min(6, n)`), so the edited feature row makes it "1" | 3,3 | 🟡 (ours shows the arrow while a dialog is open; exact counting asked live, REQUESTS.md 20 Oct follow-up) |  |
| Selection-count cursor outside the canvas | the same cursor on `.navbar`; while a context menu is open the layer and the canvas are `default` (live dom/cursors-count.json) | — | ✅ (navbar via `CursorSync`) | 8627322e 10-28: navbar count cursor ✓ (32 px hot 3,3 over the title bar with 1 selected); ✗ with a context menu open the canvas keeps it → CURSOR-COUNT-UNDER-MENU |
| Follow mode (another user's pointer) | cloud-only (excluded) | — | — |  |
| Comment tag placement (`comment-tag.svg`) | cloud-only (excluded) | — | — |  |

Image cursor art (icons2, 2026-10-21) was compared with Chrome renders of Onshape's own cursor files (live/cursors/icons2-*-{1x,2x}.png):
the shared drop shadow is `dx 0, dy 1, σ 1, opacity 0.8`; the Windows white arrow has its own softer shadow (`1, 1, σ 1.5, 0.4`), and the Windows
art beside the arrow sits +1,+1 (the count label +3,−5) on a 38 px canvas, as in Onshape's `win*` files.

## HTML controls (woolsthorpe.css)

| Context | Cursor | Status | Verified |
|---|---|---|---|
| Buttons, `[role=button]`, toolbar tools and ▾ carets, Search tools…, menu items, inactive tabs, + and tab-list buttons, navbar ☰ / ?, node expanders, Features header icons, left rail / right strip icons, the list toggle, dialog option tabs, checkbox labels, dropdowns, chip ×, (?) help, view cube ▾, camera menu items | `pointer` (live dom/cursors-controls.json) | ✅ (vendored gpui-component `Button`: pointer when clickable; read back with /tmp/ui-tools/cursweep.sh) | 0.4.10+ / dd1b5037 10-27 (XFixes read-back, bin/xcur.py): toolbar tools, Search tools…, Undo: hand hot 7,5 |
| Active tab, empty navbar, view cube faces / arrows, dialog ✓ / ✗ (enabled or not), panel-dialog header ✓ / ✗, selection chip text, feature and part rows | `default` / `auto` | ✅ | 0.4.10+ / dd1b5037 10-27 (XFixes read-back, bin/xcur.py): view cube, dialog ✓: arrow |
| Undo / Redo with nothing to undo, disabled camera items (Previous view), disabled buttons | `not-allowed` | ✅ (Undo/Redo; `Button` disabled → not-allowed) | 0.4.10+ / dd1b5037 10-27 (XFixes read-back, bin/xcur.py): Redo with nothing to redo: not-allowed |
| Text inputs (the whole box inside the border, padding included), document name and workspace label in the navbar, number fields | `text` | ✅ (vendored gpui-base `frame_hit`; app boxes via `input_frame`) | 0.4.10+ / dd1b5037 10-27 (XFixes read-back, bin/xcur.py): filter box: I-beam hot 11,12 |
| Feature-dialog / panel-dialog title (`.ns-drag-area` inside `.ns-dialog-header`, which is `auto`) | `grab`, also while dragging; the dialog moves (live: feature dialogs, Mass and section properties, Appearance, Isolate, Make transparent, Material, Named views, Section view, Replace instance, Interference detection; Measure is docked and does not move) | 🟡 (feature dialog, Create selection; Part Studio Mass, Section view, Material, Named views, Appearance; assembly Mass, Section view, Isolate / Make transparent: `ui::panel_title` + `PanelOffsets`; checked: Material moved exactly by the pointer's travel, reopens in place, and assembly Isolate moves too. Still to do: Replace instance, Interference detection, and the Part Studio Isolate panel, which isn't Onshape's dialog yet) |  |
| Disabled dialog ✓ (`div.ns-dialog-button-ok.disabled`) | `default` (live) | ✅ | 0.4.10+ / 41f60e47 10-28 (xcur: arrow hot 3,1) |
| Feature-list row, rollback bar (hover and drag) | `default` (live) | ✅ | 0.4.10+ / dd1b5037 10-27 (XFixes read-back, bin/xcur.py): feature / part rows: arrow |
| Dimension box drag strip (`.dimension-dialog-drag`) | `move` (Chrome shows all-scroll) | ✅ (read back: 24 px all-scroll; dragging moves the box, and the sketch tool under it no longer follows the pointer) | 0.4.10+ / dd1b5037 10-27 (XFixes read-back): all-scroll (fleur) on ▫▫▫ |
| Configurations panel input headers (`.draggable-header.is-draggable`): drag to reorder | `auto` at rest, `grabbing` while dragging; a `#1651b0` pill with the name follows the pointer; the drop line is 2 px `#1651b0` from a 6 px open circle (live cursors/670–671) | 🟡 (Part Studio panel checked: pill at the pointer, drop line, closed hand read back, Ctrl+Z restores; the Variable table's inserted-table reorder (pale pill, red line where it can't drop) is still to do) |  |
| Feature dialog right edge (`osc-resize-handle.osc-x`, 3 px inside the edge; `.osc-y` is 0×0) | `ew-resize`, width only, also at the bottom-right corner (live: no corner resize) | ✅ (was a 10 px corner grip that resized both ways; checked: ew-resize read back, 220 → 320 px) | 0.4.10+ / dd1b5037 10-27 (XFixes read-back, bin/xcur.py): ew-resize |
| Row / column splitters (`div.os-splitter`): features / parts list, Configurations block (least = its title row; grows past its inputs), left panel / graphics (9 px zone in the graphics; ≥ default width, persists; no double-click), right panel / graphics (4 px zone; ≥ 235 px, persists) | `row-resize` / `col-resize` (live) | ✅ (`ui::left_splitter`, `cfg-box-splitter`, `ui::right_resizable` on the Part Studio panel and the five assembly panes; checked: col-resize / row-resize read back, left stops at 206, right 712 → 510, the block collapses to its title and grows past its inputs) | 0.4.10+ / dd1b5037 10-27 (XFixes read-back, bin/xcur.py): features / parts splitter ns-resize |
| Table column resize grip (`.table-cell--header .drag-handle`, 5 px inside the header cell's right edge) | `col-resize`; only that column changes (the others shift), at least 40 px, no maximum, a horizontal scrollbar, no double-click; widths persist per table (live behaviours.md "Table column resize") | ✅ BOM (checked: col-resize read back, 140 → 340, stops at 40, persists), named positions, assembly and Part Studio Configurations tables, custom tables (`ui::col_grip`). The Variable table is still to do (psb's area). | 0.4.10+ / 41f60e47 10-28 (BOM: col-resize, Item 140→240, others shift) |
| Resizable modals (`resizableModal`: Properties…, BOM / Inspection table template): bottom 5 px / right 5 px / 10 px corner | `ns-resize`, `ew-resize`, `se-resize`; at least 800×542, stays centred horizontally, size persists | 🟡 (Properties checked: ew / ns / se read back, the corner drag gave 1190×687 centred (live 1173×687); the table-template dialog isn't in noshape) |  |
| jQuery dialogs: title bar / edges / corners | `move`; `n/s/e/w/ne/nw/se/sw-resize` | ✅ n/a (live: shipped in the bundle but never used) | 78958d0f 10-28 (asm5 Mass and section properties: edges and corners read the plain arrow (no resize cursors, = live never uses them); its title reads the open hand (grab, row 70)) |
| Selection-list reorder mode (Loft profiles, `ALLOW_QUERY_ORDER` lists): the ≡ grip (`.os-param-selection-list-drag-handle`, 20 x 20) in place of the × | `ns-resize`; the ↑↓ "Reorder items" button / "DONE"; the name pill and the drop line while dragging (live ui/960–968) | ✅ query lists and driven-query arrays (checked in a Loft: the button with 2 profiles, DONE and grips, read back ns-resize, pill, line, swapped). Plain arrays (`.os-param-array-item`) are not done yet. Property lists / enum tables are company settings (cloud, excluded). | 0.4.10+ / 1b81b7ca 10-28 (Boundary surface U profiles › ⇅: rows get ≡ grips and "DONE"; over a grip the cursor is the vertical double arrow, hot (12,13)) |
| Customize toolbar: sled, tool-set drag handle, palette items | `move` | 🟡 (assembly Tools panel ✅ with `CursorStyle::Move`; the Part Studio's Customize toolbar is still the old arrow mode and needs the same Tools panel) | 78958d0f 10-28 (asm5 Customize toolbar…: over a ••• sled and over a tool in customize mode XFixes reads the theme's move cursor (24 px four-way arrows, hot 11,11); palette items not checked) |
| Colour Mixer: saturation/value square / hue strip | `crosshair` / `ns-resize` (live behaviours.md "Colour Mixer") | 🟡 `color_mixer.rs`. In the drawing colour dialog, checked: both cursors read back, the markers, hex/RGB follow. The Part Studio Appearance editor has no Palette / Mixer tabs yet, and the drawing dialog itself is not yet live's "Dimensions color" panel. |  |
| Drawing markup content (`.os-markup-content`) | `crosshair` | — (Comments panel › Markup: comments are cloud-only, excluded) |  |
| `abbr[title]` | `help` | — |  |
| Loading placeholders | `wait` | ✅ n/a (live: the `.placeholder` rule is never used) |  |
| A collapsed layout splitter | `default` | ✅ n/a (live: a collapsed Configurations block keeps `row-resize`; no other collapsible splitter) |  |

## Still to capture live (REQUESTS.md, 20 Oct)

The computed CSS cursor of the canvas (and of the element under the pointer) in each of these
states, to settle the ❓ rows and the drawing / assembly specifics: every sketch tool hovering
the viewport (and over a sketch entity), during right-drag rotate / middle-drag pan /
Ctrl+right zoom / wheel, over the extrude depth arrow and while dragging it, over the triad
arrows / planes / rotation rings (Transform, Mate connector, Assembly drag), over a dimension
value, over a section plane manipulator, in Zoom to window, in the Measure tool, while picking
in a feature dialog's selection field, in drawing tools (view placement, dimensions, notes,
callouts) and over drawing views, over splitters and dialog headers.
