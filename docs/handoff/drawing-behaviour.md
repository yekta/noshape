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

## Session 18 Oct (evening): inspection, export modal, smart dimension, QA round 3
- `app/drawing_inspection.rs`: Inspection item dialog (Manual with hover preview / Automatic with Scope +
  Preview, Border shape), Reorder, Edit (double-click the bubble), drag a bubble onto another annotation,
  context entries, "Hide inspection items" toggle. Toolbar end: Inspection item + Hide toggle.
- `model/drawing/inspection.rs`: numbering, eligibility, rows (Item parsing Parameter splits hole callouts and
  chamfers into n.1, n.2), Inspection list / Drawing definition list CSV. `sheet.rs`: bubble at the upper left of
  the annotation's text (own pick: `sheet::inspection_pick`, `PickTarget::Inspection`).
- `app/drawing_panels.rs`: right strip in the live order (Inspection table, Styles, Drawing properties, MBD
  status). Inspection table works (View, ⋯ Export to CSV, hover cross-highlight, Assign number, edit number).
  Styles / MBD status are empty shells: capture requested in live/REQUESTS.md.
- `app/drawing_drawprops.rs`: Inspection tab (Item parsing, Text height, Border shape, Border size, Exclude
  callouts). Other tabs (Dimensions … Tables) still say "set per item": TODO (S22–S33).
- `app/drawing_export.rs`: "Export Drawing" modal from tab right-click › Export… (workspace.rs hook):
  PDF / DXF / SVG / PNG / JPEG / TIFF / Inspection list / Drawing definition list, Version, Sheets (custom
  ranges), Overridden dimensions, Text, Color, DXF Units, checkboxes. Several sheets → stored zip.
  `model/drawing/export.rs`: `ExportOptions`, `prepare`, `to_dxf_units`, `to_pdf_pages_version`, `zip_store`.
  Raster via gpui's svg renderer at 300 dpi (verified: 4960x3507 PNG). Toolbar Export removed (Onshape has none).
  Missing: DWG / DWT (needs a DWG writer), Options "Download and store…" / "Store file in a new tab" (needs
  file tabs in the document model), DXF version list (only R12 written).
- `app/drawing_smart.rs`: smart Dimension (d) — circle → diameter, arc → radius, line, point/line, parallel
  lines, angle, concentric circles (wall thickness), deselect by clicking a picked edge again. Verified (Ø28).
- QA round 3 drawing items: "+" › Create Drawing now opens the Insert chooser; template row `#b2ddf6`; Esc
  ending a tool clears the selection (views no longer orange); Section view clicks within 8 mm of a view cut it;
  Section dialog "Show cut geometry only" and "Section depth" (Blind) work (`DrawingView.cut_only`,
  `section_depth`, views.rs section_geometry, test `section_cut_only_and_depth`).
- Next: Section dialog Cut angle / Opposite direction / Flip / Up to entity / Exclude from cut; Drawing
  properties tabs S22–S33; DWG writer; file tabs for export Options; then plan steps 5–7 below.

## Session 18–19 Oct (night): drawing properties, colors, DWG, sketch tools
- Colors in sheet geometry: `SheetGeometry.colors` (`ColorSpan` ranges, `color_since`, `line_color`…) drawn on
  screen (`drawing_scene::sheet_colored`, per-line colors) and in every export (SVG / PDF / DXF ACI / DWG true
  color). Export › Color: Black and white / Grayscale in `export::prepare`.
- `SheetGeometry.style` (`SheetStyle`): dimension / annotation text height, arrowhead size and type (None, Filled,
  Unfilled, Open, Oblique, Filled dot), color; set from `DrawingProps.dim_*` / `ann_*` (Drawing properties ›
  Dimensions / Annotations tabs). Views tab: projection + Insert view defaults (`ViewDefaults::apply`;
  `DrawingView.auto_centerlines` separate from centermarks). Inspection tab: Border color, Solid fill.
- Section dialog: Cut angle + Opposite direction, Flip (`DrawingView.flip`, view stays in place), Show cut geometry
  only (`cut_only`), Section depth Blind (`section_depth`).
- Snap markers: orange square / triangle / diamond / circle (`drawing_scene::SnapMark`).
- DWG / DWT / DXF 2000+: new dependency **acadrust 0.6.5 (MPL-2.0)** in noshape-model; `drawing/cad_export.rs`
  (test reads every version back with acadrust and with cadkit-dwg). DWT = sheets 1–2 (sheet 2 in a layout),
  no views / annotations / tables, 2013.
- Drawing sketch: `Sheet.sketch: SheetSketch` (noshape_sketch::Sketch in sheet mm + styles + hatches,
  `drawing/sheet_sketch.rs`, picks `SketchEntity` / `Hatch`, `Sel::Sketch` / `Sel::Hatch`). Tools and constraints
  in `app/drawing_sketch.rs` (keys l c g r m h v i Shift+l b t), Hatch region dialog `app/drawing_hatch.rs`,
  Insert DXF or DWG into the sheet sketch. Verified line / circle / rectangle / hatch / Horizontal.
- Next: Insert image; Styles and MBD status panels (captures requested); fonts (Font rows everywhere); remaining
  Drawing properties groups (gaps, chamfer, per-annotation-type, construction geometry, formats, tables); hatch
  in view regions; constraint glyphs on hover (SK15); dimensions of sketch geometry (SK7).

## File tabs (asb owns them; shared files touched: model/document.rs, app/workspace.rs, tab_manager.rs, import_dialog.rs, start_window.rs)
- Model: `TabContent::File(FileTab)` (`model/file_tab.rs`): file name, base64 bytes (`Arc<str>`), size, upload time;
  `kind()` / `type_label()` / `size_label()` / `translatable()`. Self-contained, so Duplicate, Copy to clipboard and
  Move to document carry the bytes; the store dedups unchanged blobs by hash. `Document::tab_users` skips file tabs.
  No store change was needed (tabs are serialized per blob as before).
- App: `app/file_tab_view.rs` — the tab's centred card (live documents/67: icon, name, type, size, "Uploaded …",
  Update… / Download), `add_file_tab`, `cad_imports_folder`, `drawing_from_cad`, `download` / `update`, and
  `pick_file` (Onshape's insert-from-file dialog: search the document's file tabs, Import a local one).
- workspace.rs: `TabView::File`, `kind_of` returns `file-cad` / `file-dwg` / `file-image` / `file-pdf` / `file`
  (also the icon name), tab menu Download / Update… / Translate…, "+" › Import… takes any file (CAD → Import dialog
  + file in "CAD Imports"; DXF / DWG → file tab + Drawing; others → file tab), `translate_file`.
  `import_dialog::open` gained `keep_files: bool`. tab_manager: kind "file" in the filters. start_window
  `import_files`: same routing for new documents.
- Users: Export "Store file in a new tab" / "Download and store…", Insert image, Insert DXF or DWG.
- Not yet: Sheet properties › "Select a DWT" from a DWT file tab; "Other documents" browsing in the pickers.

## Sketch views and Detail dialog (19 Oct)
- Sketch views: `DrawingView.sketch: Option<FeatureId>` (the root base view of a sketch; projected / auxiliary
  children inherit it). `views::sketch_frame` looks at the sketch plane (dir = −normal, up = plane y);
  `views::sketch_geometry` projects the sketch curves (`PlaneFrame::to_world` → `frame.project`) with snap points.
  `create.rs`: `DrawingSource.sketch` (all other callers pass `sketch: None`), `source_frame`, and
  `create_drawing(…, frame)` makes one base view of the sketch. App: `drawing_create::request_create_drawing_of_sketch`,
  `DrawingEditor::insert_sketch` (workspace `create_drawing_tab` calls it for sketch sources).
- Menus (psb's files, surgical): "Create Drawing of Sketch N…" before "Export as DXF/DWG…" in the feature-row menu
  (part_studio/mod.rs) and the sketch-region menu (part_studio/menus.rs).
- Detail view dialog (app/drawing_detail.rs): profile Circle / Rectangle / Spline / Polygon (click points, click the
  first to close), View scale, View label, Profile style In line / Connected / Leader, Show detail view profile, Clip border
  to geometry. Model: `DrawingView.detail_profile` (polygon points), `detail_style`, `detail_hide_profile`,
  `detail_clip_border`, `detail_label_at`; `ViewGeometry.clip_poly`.

## Revision tables (19 Oct; on psb's release log, model/release.rs)
- Model: `TableKind::Revision { see_sheet1, visible_rows, newest_top, hide_callouts, title, headers, descriptions,
  hidden }`; `tables::revision_rows(&ReleaseLog, drawing tab)` (releases oldest first, drafts as pending rows),
  `revision_callout_label`, `see_sheet_data`; `TableData.pending` rows get a blue `SheetGeometry.backdrops` area
  (drawn under the ink in the app, SVG and PDF). `build::SheetInfo.revisions` feeds them;
  `AnnotationKind::RevisionCallout { anchor, revision }`.
- App: app/drawing_revision.rs (dialog, placement, table menu, callouts). **Hook for psb:**
  `drawing_revision::revisions(doc, tab)` returns no rows until the release log is stored with the document; make it
  `tables::revision_rows(&log, tab)` then (drawing_view `update_info` calls it on every reload).

## Drawing properties additions (19 Oct)
- Dimensions › Gaps and extensions: `DrawingProps.dim_text_gap` / `dim_geometry_gap` / `dim_extension` /
  `dim_text_horizontal` → `SheetStyle.text_gap` / `geometry_gap` / `extension` / `text_horizontal` (linear and angular
  dimensions).
- Formats: `formats_locked` (title block not pickable), `border_color` / `zone_color` / `title_block_color` (per-element
  `ColorSpan`s in `assemble_sheet`).
- Tables › All tables: `table_h_padding` / `table_v_padding` (default 0.06 in) → `TableData.padding` (layout and text).
- The number fields of the panel are `DrawingPropsUi.inspection_inputs: Option<(tab, Vec<NumberField>)>`.
- Tool dialogs sit at `DIALOG_TOP` (below the sheet chips) and `dialog_left()` (beside the open Sheets panel).

## 19 Oct (afternoon)
- Revision tables: double-click title / header / row edits `title` / `headers` / `descriptions`
  (`drawing_revision::open_revision_cell`, reusing `Panel::Cell` with row `usize::MAX` = title).
- `RevisionCallout.group`: dropped on a dimension's text it groups (anchor = offset from the text box's upper right;
  drawn after the annotation loop in `add_annotations`); deleting the dimension deletes it.
- Detail: Esc before a profile → `DetailDlg.pick_region`; the click takes the smallest closed sheet sketch region.
- `Annotation.flip_arrows` (selected dimension's diamond node, `flip_node_click`).
- Drawing properties › Annotations per type: `DrawingProps.ann_types: Vec<(AnnType, AnnTypeStyle)>`, `datum_size`,
  `hide_placeholder` (`format::expand_fields_with`). New notes use the Notes text height.
- New drawings: `formats_locked = true`. Table cell Size › Columns / Rows equally and Resize… (`Panel::CellResize`).
- Callouts: Align horizontally / vertically with several selected.

- Views: `DrawingView.shown_sketches` / `hide_sketch_points` (Show/hide sketches dialog, app/drawing_view_sketches.rs;
  overlay in `compute_view`) and `hidden_parts` (Show/hide parts, app/drawing_hide_parts.rs; bodies removed from the
  view's context). `Annotation.flip_arrows` covers linear dimensions only so far.

## 19 Oct (evening): notes, tables, references, views tab, colors
- Notes: `AnnotationKind::Note { text, height_mm, style: NoteStyle }` (model/drawing/note.rs: font, underline,
  strikethrough, color, line spacing, paragraph spacing, align / valign, lists with order and flag notes, box width,
  leaders, fractions `1/2␣` `1#2␣` `1^2␣`); construct with `AnnotationKind::note(text, h)`. Editor rows:
  app/drawing_note.rs (in `Panel::Note`); Note tool leader: `DrawingEditor.note_leader`. Field formats
  `{key|upper}` / `{date|dd.MM.yyyy}` (`format::field_format`).
- Tables: `Table.rows` (split parts), `Table.row_heights`, `TableKind::Bom { structured, bottom_to_top, overrides }`,
  `TableKind::CutList`, `TableKind::Hole { flip_x, flip_y }`, `TableData { header_bottom, overridden }`;
  `SourceModel.bom_structured`. App: drawing_cells.rs (cell selection, copy, paste as table, split / unsplit, BOM
  cell override, BOM row hover highlight), `Drag::TableDivider`, `infer_table_placement`.
- Sheets panel: BOM / cut list rows (Properties, Switch to BOM, Delete, Replace via `InsertViewState.replace`);
  Sort by reference groups by sheet; reference rows open the Reference manager (app/drawing_refs.rs).
- `DrawingProps.view_tab: ViewTabProps` (hidden / tangent colors, hatch spacing, section / detail / aux label
  prefixes and styles, detail defaults, centermark / centerline colors and size); `custom_colors` and the color
  dialog (app/drawing_color.rs, swatches in Drawing properties).
- `Annotation.group`: a callout dropped on a dimension follows it (drawn after the loop in `add_annotations`).
  `flip_arrows` covers linear, radial / diameter and angular dimensions.

## How to test
- Build: the shared tree is often broken by others. `/tmp/asb/bin/mbuild.sh <extra files>` copies only
  my files (assembly_*, drawing_*, model assembly/drawing, icons) into the private worktree
  `/root/noshape-asb` (snapshot of the rest) and builds with `/root/target-asb`. Pass other agents'
  new files my code needs (e.g. `crates/app/src/os_ui.rs crates/app/src/os_menu.rs crates/app/src/main.rs`).
- `/tmp/asb/bin/start.sh` (assembly demo on :104), then `/tmp/asb/bin/newdraw.sh` creates
  "Assembly 1 Drawing 1" with No views; `--demo-drawing-full` via `dstart.sh` opens the showcase.
- Model: `cargo test -p noshape-model --lib drawing::`.

## From ui2 (visual changes in your files, 18 Oct)
- `model/drawing/sheet.rs`: Onshape sheet frame (`add_border_and_title_full`: edge line, inset frame, zone
  dividers/labels in the band, ANSI zone counts) and **`SheetGeometry::grips`** (`Grip { pick, pos, kind:
  Origin | End | Handle }`), emitted by `linear_dimension_text` (attachment points, dimension-line ends,
  flip handles). `build.rs` remaps grip picks like lines/texts. Please emit grips from the other
  dimension builders too (radial, angular, ordinate…) when you touch them.
- `model/drawing/font.rs`: `text_fill` / `fill_supported` (filled Liberation Sans text on screen).
- `drawing_view.rs`: paper unlit; `texts` meshes; `push_annotation_hover` = Onshape's hovered-annotation
  look (tan `#ccb292` lines + label box, `#90cef1` / `#cc925c` grips, diamonds, arrowheads covered);
  `MenuEntry::Item` has `icon: Option<&'static str>` (`.with_icon("…")`); `fill_os_menu`;
  `open_tool_menu` (toolbar carets open `os_menu` lists in Onshape's drawing-toolbar style). The view
  and sheet right-click menus use `fill_os_menu`.
- `drawing_scene.rs`: black ink, thinner on-screen widths, text as filled meshes.
- `drawing_extra.rs`: Sheets panel (closed by default, sheet chips, toggle tab, sort tabs).
- `drawing_ui.rs`: the always-on status line is gone (prompts stay).
- Still yours: the selected-dimension look (not captured yet), dimension editor, toolbar contents.
- Drawing tab › Export… (workspace.rs `open_export`) now exists, as in Onshape's Drawing tab menu. Until your
  "Export Drawing" modal lands it calls `DrawingEditor::export(ExportFormat::Pdf, …)`; please point it at your
  modal (one line, marked "asb:" in `Workspace::open_export`). For the modal's look, `crate::os_ui::export_modal`
  (File name + Format select, 600 px, Export / Cancel) and `os_ui::select_field` (Onshape form select that opens
  an os_menu select list) are available; add the extra rows (Version, Overridden dimensions, Text, Color, Options,
  ☐ Hide inspection items) as more `field_label` + `select_field` pairs.
- (ui2, 19 Oct 02:00) I accidentally overwrote your `crates/model/src/file_tab.rs` for about a minute
  (I didn't see it existed) and restored it byte-for-byte from my sync copy of 01:13 (your last edit).
  Please check it's your latest; nothing else of yours was touched. I'm reusing `file_tab_view::pick_file`
  for the Variable Studio "From import" CSV picker.
