# Handoff: Drawing behaviour parity (agent "asb")

Checklist (kept current): `docs/parity/drawing.md` — 365 Onshape drawing behaviours from the help
pages, each audited against the code (first audit 2026-10-18: 28 ✅, 142 🔧, 188 ❌, 7 n/a).
Live captures win over help pages: `/root/onshape-research/live/drawing/`, `live/dom/*drawing*`,
behaviours.md "Drawing". Agent "ui2" restyles drawing visuals in the same files and another agent is
migrating the drawing menus to `os_menu` (`fill_os_menu`): keep edits surgical, re-read before editing.

## Plan (priority order)
1. Create drawing flow and dialog — done (see below).
2. Insert view panel and View properties — done.
3. View context menu — done except Adjust linestyle…, Insert custom table, Paste as table,
   Select for replication / Replicate annotations, Suppress alignment with parent.
4. Tool behaviour and shortcuts — in progress: dimension dropdown in the live order, same-kind
   shortcut exits the tool, `w` zoom window, Ctrl+S Sheets panel, Shift+N rename. Still: `s`
   shortcut toolbar, `[` measure, Shift+Q, Alt+drag copy, Shift (no inference), 3 point angular.
5. Section / detail / auxiliary dialogs; sheets panel and sheet properties — Sheet properties panel
   and Drawing properties (Units and precision) written; Sheets panel: projected views named by
   orientation ("Right"), "In progress" badge, view row menu (Properties…, Delete).
6. Dimension palette and editing (grips, jog, break), note editor, property picker.
7. Sketch tools, hatch region, tables (revision, custom), export dialog.

## Done (files)
- `app/drawing_create.rs`: Create Drawing dialog (live): name "<part> Drawing 1", Existing templates
  (sources Onshape / Owned by me / Recently opened / Created by me / Shared with me / Public, filter
  All/ANSI/ISO/JIS, table Template | Document | Owner incl. ANSI A portrait, preselected row scrolled
  into view), Custom template tab, Options Suggested views / **No views (default)**.
  `workspace.rs` (`open_create_drawing`, `create_drawing_tab`): new tab right after its source tab.
- `model/drawing/create.rs`: `create_drawing` (template, suggested views laid out for the projection).
- `model/drawing/mod.rs`: `SheetSize::AnsiAPortrait`, `DrawingView.{tangent_phantom, name, scale_label}`,
  `Drawing.props: DrawingProps` (units, decimal comma, precisions, leading/trailing zeros) used by
  `format.rs` (`finish`, `number`; test `drawing_properties_shape_dimension_text`).
- `model/drawing/clipboard.rs`: `ViewClipboard`, `Drawing::copy_views`, `Sheet::paste_views` (test).
- `model/drawing/views.rs`: `standard_orientation` (projected views named Right/Top...).
- `app/drawing_insert.rs`: Insert view panel (Insert + source dropdown, View orientation, View scale,
  View simplification, two checkboxes) shown while `Tool::InsertView`; "Select a part or assembly"
  chooser (toolbar Insert view and the panel's Insert button). `DrawingEditor.insert`.
- `app/drawing_props.rs`: view context menu in Onshape's order (+ Copy / Paste, Rotate view
  vertically / horizontally by edge), View properties panel, `view_clipboard`, `default_view_name`.
- `app/drawing_keys.rs`: zoom window (`DrawingEditor.zoom_window`), Ctrl+S, Shift+N.
- `app/drawing_sheetprops.rs`: Sheet properties panel (Scale, Format, Size, Reference + link).
- `app/drawing_drawprops.rs`: Drawing properties panel (right dock, 8 tabs; Units and precision works).
- `drawing_tools.rs`: empty-sheet menu (Paste, Sheet properties…, Drawing properties…, Zoom to fit /
  window / sheet), `set_tool` same-kind dimension exits, Ctrl+S, zoom-window hooks, Esc cancels modes.
- `drawing_ui.rs`: dimension dropdown = live list; Insert view button opens the chooser.

## State
- Verified in the app (screenshots in /tmp/asb): create dialog, new tab + Insert view panel, base +
  projected view placement, chooser, view scale in the panel, view menu, view copy/paste.
- Written but NOT yet verified (build blocked for a long time by other agents' files and a full
  dependency rebuild): Rotate view to edge, zoom window, Ctrl+S, Shift+N, Sheet properties,
  Drawing properties, Sheets-panel names/badge/menu, dimension dropdown, same-kind exit. Verify first.

## How to test
- Build: the shared tree is often broken by others. `/tmp/asb/bin/mbuild.sh <extra files>` copies only
  my files (assembly_*, drawing_*, model assembly/drawing, icons) into the private worktree
  `/root/noshape-asb` (snapshot of the rest) and builds with `/root/target-asb`. Pass other agents'
  new files my code needs (e.g. `crates/app/src/os_ui.rs crates/app/src/os_menu.rs crates/app/src/main.rs`).
- `/tmp/asb/bin/start.sh` (assembly demo on :104), then `/tmp/asb/bin/newdraw.sh` creates
  "Assembly 1 Drawing 1" with No views; `--demo-drawing-full` via `dstart.sh` opens the showcase.
- Model: `cargo test -p noshape-model --lib drawing::`.
