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

Live check (2026-10-20, `/root/onshape-research/live/dom/cursors.txt` and `cursors-*.json`:
`getComputedStyle(...).cursor` in Chrome) confirms the stack below and settles the ❓ rows.
Chrome truncates fractional hot spots: `crosshair-cursor.svg 7.5 7.5` is applied at 7,7.

Status: ✅ done, 🟡 partly, ❌ not yet. "Hot" = hot spot in CSS pixels.

## Graphics area (viewer cursor stack)

| Context | Cursor | Hot | Status |
|---|---|---|---|
| Nothing selected, Part Studio / Assembly | `default` (arrow) | — | ✅ (X11: GPUI now sets the theme arrow on the first frame; before, a new window showed the root window's cursor) |
| N entities selected (N = 1..5) | image: arrow + digit N (`macSelectionCursor-N`; Windows `winSelectionCursor-N`) | mac 3,3 · win 1,2 | ✅ (read back on X11: 32 px, hot 3,3; Windows art with hot 1,2 in cursor_windows) |
| 6 or more selected | image: arrow + "5+" (`…SelectionCursor-5plus`) | mac 3,3 · win 1,2 | ✅ |
| Any sketch drawing tool active (line, rectangles, circles, arcs, polygons, splines, point, text, slot, fillet, chamfer, trim, extend, offset, use, mirror, dimension, constraints …: the sketch tool base class `getDefaultCursor()`) | image crosshair (`crosshair-cursor.svg`), fallback CSS `crosshair` | 7.5,7.5 (Chrome truncates to 7,7) | ✅ (read back: 15 px, hot 7,7) |
| Sketch pattern tools (linear / circular pattern, transform: they select first) | `default` | — | ✅ |
| Sketch linear / circular pattern, transform (`canCreatePattern`) and offset (preview shown): operation complete, nothing pre-highlighted and no dimension box open ("click to accept"; `showConfirmationCursor`) | image confirmation (`macConfirmation_Cursor` / `winConfirmation_Cursor`); while something is pre-highlighted the stack is empty, so the selection cursor shows | 1,1 | ✅ (read back: 32 px, hot 1,1) |
| Hovering a dimension's value (draggable) | `move` | — | 🟡 (viewport `overlay_cursor`; read-back pending) |
| Hovering a dimension's lines | `default` | — | ✅ |
| Hovering a plane-type manipulator (section plane / move plane) | `move` | — | ❌ |
| Ctrl held during a box selection (box deselect) | image remove-selection (`macRemoveSelectionCursor` / `winRemoveSelectionCursor`) | mac 3,3 · win 1,2 | ✅ (Part Studio and sketch box; Ctrl in a sketch box now also deselects; read back: 32 px, hot 3,3) |
| Zoom to window mode ("Drag to create zoom bounding box.") | `zoom-in` | — | ✅ (theme `zoom-in` on X11; our drawing where the theme has none) |
| Assembly: dragging a fixed instance | `no-drop` | — | ✅ (was not-allowed) |
| Assembly: snapping a mate connector during a drag | image snap-mate (`macSnapMate_Cursor` / `winSnapMate_Cursor`), fallback `nesw-resize` | 7.5,7.5 | 🟡 (shown for the whole snap drag) |
| Analysis color-map legend: over the legend / its color bar / dragging / tick hover / value text | `grab` / crosshair image (7,7) / `grabbing` / `ew-resize` (horizontal legend) / `pointer` | — | ✅ (legend `OpenHand`/`ClosedHand`, color map through the viewport's `overlay_cursor`) |
| Drawing: no tool (sheet, views, dragging a view) | `auto` (arrow) | — | ✅ |
| Drawing: Dimension, Ordinate, Datum, GD&T, Note, Balloon, Table, Centerline, Line, Hatch, Insert view placement | `DrawingCrosshair-01.cur` (the sketch crosshair's 15 px plus) | 7,7 | ✅ (`AppCursor::Crosshair` via `CursorSync`) |
| Drawing: Centermark | `DrawingCrosshairBox3-01.cur` (plus with a 7 px hollow square, arms to the edges) | 7,7 | ✅ (`AppCursor::CrosshairBox`, our drawing) |
| Rotate / pan / zoom drags (right / middle / Ctrl+right, Shift+middle, wheel) and left-drag box select | `default` (live: Onshape never changes the cursor for navigation) | — | ✅ (no hiding during drags) |
| Manipulator arrows, triad, rotate rings, extrude depth arrow (hover and drag) | unchanged: the dialog's current canvas cursor (live) | — | ✅ |
| Feature dialog open (e.g. new Extrude, Mate connector after its first pick) | the selection-count cursor: Onshape counts the whole selection minus rollback-bar items (`min(6, n)`), so the edited feature row makes it "1" | 3,3 | 🟡 (ours shows the arrow while a dialog is open; exact counting asked live, REQUESTS.md 20 Oct follow-up) |
| Selection-count cursor outside the canvas | the same cursor is set on `.navbar` and on `#context-menu-layer` while a context menu is open | — | ❌ |
| Follow mode (another user's pointer) | cloud-only (excluded) | — | — |
| Comment tag placement (`comment-tag.svg`) | cloud-only (excluded) | — | — |

## HTML controls (woolsthorpe.css)

| Context | Cursor | Status |
|---|---|---|
| Buttons, `[role=button]`, toolbar tools and ▾ carets, Search tools…, menu items, inactive tabs, + and tab-list buttons, navbar ☰ / ?, node expanders, Features header icons, left rail / right strip icons, the list toggle, dialog option tabs, checkbox labels, dropdowns, chip ×, (?) help, view cube ▾, camera menu items | `pointer` (live dom/cursors-controls.json) | ✅ (vendored gpui-component `Button`: pointer when clickable; read back with /tmp/ui-tools/cursweep.sh) |
| Active tab, empty navbar, view cube faces / arrows, dialog ✓ / ✗ (enabled or not), panel-dialog header ✓ / ✗, selection chip text, feature and part rows | `default` / `auto` | ✅ |
| Undo / Redo with nothing to undo, disabled camera items (Previous view), disabled buttons | `not-allowed` | ✅ (Undo/Redo; `Button` disabled → not-allowed) |
| Text inputs (the whole box inside the border, padding included), document name and workspace label in the navbar, number fields | `text` | ✅ (vendored gpui-base `frame_hit`; app boxes via `input_frame`) |
| Feature-dialog / panel-dialog header drag area (`.ns-drag-area:hover`, `.osx-panel-dialog-header`, `.ns-subdialog-header`) | `grab` (live: title and drag area) | 🟡 (feature dialog ✅; panel dialogs to audit) |
| Disabled dialog ✓ (`div.ns-dialog-button-ok.disabled`) | `default` (live) | ✅ |
| Feature-list row, rollback bar (hover and drag) | `default` (live) | ✅ |
| Dimension box drag strip (`.dimension-dialog-drag`) | `move` | ❌ |
| Draggable table/section headers (`.draggable-header.is-draggable:hover` / `:active`) | `grab` / `grabbing` | ❌ |
| Layout splitters (`.os-layout-splitter`, x / y axis) and resize handles (`osc-resize-handle.osc-x/.osc-y`) | `ew-resize` / `ns-resize` | ❌ |
| Row / column splitters (`div.os-splitter`): features / parts list (live y 789), Configurations block (y 157), left panel / graphics (x 245) | `row-resize` / `col-resize` (live) | 🟡 (features/parts ✅; noshape has no left-panel width splitter nor a Configurations-block splitter yet) |
| Table column resize grip (`.table-cell--header .drag-handle`) | `col-resize` | ❌ |
| Resizable modals: edges / corner | `ns-resize`, `ew-resize`, `se-resize` | ❌ |
| jQuery dialogs: title bar / edges / corners | `move`; `n/s/e/w/ne/nw/se/sw-resize` | ❌ |
| Drag grips in lists (selection-list drag handle, property lists, enum tables) | `ns-resize` | ❌ |
| Customize toolbar: sled, tool-set drag handle, palette items | `move` | ❌ |
| Color picker area / slider | `crosshair` / `ns-resize` | ❌ |
| Drawing markup content (`.os-markup-content`) | `crosshair` | ❌ |
| `abbr[title]` | `help` | — |
| Loading placeholders | `wait` | ❌ |
| A collapsed layout splitter | `default` | ❌ |

## Still to capture live (REQUESTS.md, 20 Oct)

The computed CSS cursor of the canvas (and of the element under the pointer) in each of these
states, to settle the ❓ rows and the drawing / assembly specifics: every sketch tool hovering
the viewport (and over a sketch entity), during right-drag rotate / middle-drag pan /
Ctrl+right zoom / wheel, over the extrude depth arrow and while dragging it, over the triad
arrows / planes / rotation rings (Transform, Mate connector, Assembly drag), over a dimension
value, over a section plane manipulator, in Zoom to window, in the Measure tool, while picking
in a feature dialog's selection field, in drawing tools (view placement, dimensions, notes,
callouts) and over drawing views, over splitters and dialog headers.
