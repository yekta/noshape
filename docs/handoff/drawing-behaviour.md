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

## Pinned view references (S5) — API
- Model: `DrawingView.pinned: Option<PinnedVersion { id, name }>` on root base views; `SourceKey.version:
  Option<String>` (the store's version id as text). Build keys with `SourceKey::of_view(&view)` (it carries the pin);
  `referenced_sources` / `view_source` use it. `views::regenerate_sources_with(doc, drawing, &|id| -> Option<Document>)`
  regenerates pinned sources from the document at their version (unknown versions are left out);
  `regenerate_sources(doc, drawing)` = no versions. Test `pinned_views_use_their_version`.
- App: `Session::document_at_version(id) -> Result<Option<Document>>` (session.rs, store `list_versions` +
  `read_all(microversion)` + `Document::from_blobs`; test `document_at_a_version`). `DrawingEditor.version_docs`
  caches them (`version_doc`); `ensure_sources` regenerates pinned keys from it; `studio_changed` ignores pinned keys
  (a version never goes out of date).
- UI: Sheets panel reference rows show a pin and the version name when pinned; right-click: Reference manager…,
  Pin view reference ▸ (the document's versions) / Unpin view reference (`pin_reference`: every base view of that
  source and part, all sheets).
- Center of mass: `DrawingView.show_com`, `ViewGeometry.com`, `PartInfo.density` (only parts with a material),
  `DrawingProps.com_diameter`; indicator drawn in `assemble_sheet`.

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

## 19 Oct (after the reboot): S28 groups, callouts, weld dialog, flat pattern views, pinned thumbnails
- Drawing properties › Annotations now follows the help image's groups (`AnnType::ALL` = Notes, Flag notes,
  Callouts, Datums, Geometric tolerances, Surface finish symbols, Weld symbols, Bend notes, Hole/thread callouts;
  serialized by name, old files keep working). New `DrawingProps`: `hide_property_background`, `flag_shape`,
  `callout_justify` (`TextJustify`), `callout_stack` (`StackDirection`), `callout_stack_gap`, `target_hatch`,
  `hole_indicator`, `fit_iso`; `SheetStyle` carries the callout / target-hatch ones. `Drawing::weld_ansi()`.
  Number fields of tab 2: 0 text height, 1 arrowhead, 2..11 per-type heights, 11 datum size, 12 COM diameter,
  13 stack gap (`sync_drawprops_inputs`).
- Property background: build.rs records every expanded title-block value and every note text with a `{field}`
  (`Env.used`); texts equal to one get a grey box in `SheetGeometry.field_backdrops` (drawn on screen only by
  `drawing_scene::sheet_backdrops`, `PROPERTY_BACKGROUND` #e6e6e6; exports ignore it).
- Callouts: `AnnotationKind::Balloon { .., callout: CalloutStyle }` (construct with `AnnotationKind::balloon`):
  above / below / left / right, `CalloutBorder`, `chars`, `height`, `stacked` (texts; refs slot i+1 = the stacked
  instance; `AnnotationEnv::stacked_text`). Still to do in the app: the full Callout dialog (A23) and
  "Add stacked callouts" (A28) — capture requested.
- Weld symbols: model/drawing/weld.rs (`WeldSpec`, `WeldSide` with depth / pitch / contour / finish / root /
  groove / 2nd fillet size+length, `stagger_right`), app/drawing_weld.rs (dialog in Onshape's graphical layout
  from the help images; joint icons `assets/icons/weld-*.svg` generated by scripts/gen-weld-icons.py (`python3 scripts/gen-weld-icons.py crates/app/assets/icons`) —
  regenerate if the glyphs change; keep the geometry in sync with
  `weld::glyph`). NOT verified in the app yet (shared tree broken by other files at the time).
- Flat pattern views: `DrawingView.flat / hide_bend_lines / hide_bend_notes`, `views::flat_geometry`,
  `ViewGeometry.bends / form_marks`, bend lines + notes in `assemble_sheet`; chooser rows "<part> (Flat)";
  view menu Show/Hide bend lines / bend notes. Test `flat_pattern_view_shows_bend_lines_and_notes`
  (`flat_bracket()` builds a sheet metal L bracket in the demo document).
- Pinned views (S5) also in thumbnails: `views::pinned_versions(doc)`, `Session::pinned_documents()` (test
  `pinned_drawing_views_get_their_version`), `tab_manager::drawing_thumbnail(doc, versions, …)` and
  `release_thumbs::render(doc, versions, …)` (psb's / ui2's files, surgical).
- Live requests filed: Annotations tab DOM, weld dialog rows, flat pattern insert + view, callout dialog.
- Scripts (recreate after a reboot): /tmp/asb/bin/start.sh (copies the shared binary, runs on :104),
  shot.sh, setrow.py (checklist rows from JSON).
- Live answers applied (behaviours.md "Drawing properties › Annotations", "Weld symbol", "Flat pattern view"):
  FlagShape has Onshape's 11 shapes (serde alias "Square" → Rectangle); TextJustify::Auto (`frac(bordered,
  leader_left)`); weld: 14 joints (centered variants removed), no 2nd-fillet row, Finishing names (`FINISHES`
  letter + name), ISO flip identification line / "50(100)" / no spacer or finishing, symmetric = the other side
  mirrored, dialog stays open on the placed symbol until ✓ (`WeldDlg.placed`; ✕ removes it), glyphs at 1.5 × text.
  Flat views: bend region tangent lines + bend line (`FlatBend.width`, `ViewTabProps.bend_up_line /
  bend_down_line / *_color`), notes "UP 90.0° R.091" via `dim_text` / `format_len`.
- Move to layers: `Annotation.layer: SheetLayer` (Drawing / BorderFrame / BorderZones / TitleBlock),
  `SheetGeometry.layers: Vec<LayerSpan>` (also set for the format's own lines and texts), `line_layer` /
  `text_layer` / `fill_layer` for DXF / DWG, `export::prepare` keeps them through explode. Menu in
  drawing_tools.rs (annotation context menu). Tables can't move to a layer yet.
- Notes: `NoteStyle.explode` (menu Explode; drawing_export explodes those texts), `angle` (rotation about the box
  middle; grips in drawing_extra.rs `note_grips` / `note_grip_drag`, `Drag::NoteGrip`), ruler
  (app/drawing_ruler.rs, `DrawingEditor.ruler`), `tabs` / `first_indent` / `left_indent` / `right_indent` in
  `note::layout`; the note textarea uses hard tabs.
- `--demo-fab`: Part Studio with demo_fab's sheet metal U channel and welded frame (for flat pattern / cut list
  checks in the app).
- Callouts (live drawing/180–194): app/drawing_callout.rs — `CalloutDlg` (shown with the Callout tool and for
  Edit… / double-click; `refresh_callout_values` caches the boxes for previews), `StackDlg` (Add stacked callouts…,
  `stack_click`, BOM mode banner). Model: `CalloutBorder` = Onshape's 12 shapes, `CalloutStyle { chars (0 = Tight
  Fit), on_face (dot vs arrow), free (no leader: click in empty space), stacked: Vec<StackedCallout { text, body }> }`;
  `Env::callout_text(a, body, text)` resolves `{part.*}`, `{bom.item|quantity|number|name|material}`, "----" when
  empty. Text above / below are outside the border.
- Tables: BOM columns Item | Quantity | Part number | Description, no title (`tables::BOM_HEADER`; drawing_cells.rs
  checks "Item" and the part number in column 2); cut list Item | Qty | Description | Length | Angle 1 | Angle 2.
  app/drawing_table_dlg.rs: the BOM table / Cut list table dialogs (source = a view of the sheet with that source;
  `table_dlg_click` places). Fonts: Onshape offers 22 fonts (behaviours.md Annotations); we bundle 4 families in
  noshape-sketch — adding them is a binary-size decision for the lead (CJK Noto ~16 MB each).
- Entity copy / paste: `Drawing::copy_entities`, `ViewClipboard` may hold only annotations / tables (`has_views`),
  `Sheet::paste_views` detaches entities whose view is missing. App: `copy_entities` (Ctrl+C without views, menu Copy).
- Grouping: `Annotation.view_group` (Group with a view; moved in the MoveViews drag) and `ann_group` (Group annotation;
  MoveAnnotation drag moves the group unless Shift). Menus in drawing_tools.rs annotation menu.
- Table cells: Tab / Enter / Shift+Tab / Alt+Enter in the cell editor (`cell_step`).
- Threads: `ViewGeometry.threads` (end-on tapped holes), `views::thread_major_diameter`, `DrawingView.hide_threads`.
- Chamfer dimensions: `DrawingProps.chamfer_*`, `ChamferContent`, `sheet::ChamferStyle` via `AnnotationEnv::chamfer_style`.
- New pinned views copy the part's pin (V8). Build helper: /tmp/asb/bin/buildloop.sh retries the shared build.
- Clippy (large_enum_variant, blocking main): `AnnotationKind::Weld { spec: Box<WeldSpec> }` and
  `DrawingProps.view_tab: Box<ViewTabProps>` (keeps `Drawing` under the TabContent threshold; boxing
  `TabContent::Drawing` instead made PartStudio the outlier). My app files are clippy-clean (`-D warnings`).

## Custom tables TB25/TB26 + Part Studio Custom tables panel (20 Oct; on fs's `custom::table`)
- Model (`model/drawing/tables.rs`): `TableKind::Custom { studio, version, fs, name, fs_version, bottom_to_top,
  overrides }` (whole Part Studio, no view; `fs_version` = an opaque key, the app uses `derived::ExternalRef` keys
  "ext:<mv>|<fs tab>|<path>" for tables added from a version). `CustomTableKey::of(kind)`, `custom_table_keys`,
  `CustomTables` map in `SheetInfo.custom_tables`; `table_data_with(.., info, ..)` resolves custom tables (use it
  instead of `table_data` wherever a SheetInfo exists). `evaluate_custom(doc, fs, name, from_version, ctx)`
  (workspace: DocumentScripts; version: `OneStudio` compiled alone), `evaluate_custom_tables` (headless).
  `referenced_sources` includes the custom tables' Part Studios. `TableData.col_align / header_bold` (custom
  tables: bold header, body cells aligned like the columns, Left by default; table cell formats still win).
  Test `custom_tables_tabulate_a_whole_part_studio`.
- App shared (`app/custom_tables.rs`): `TableRef`, "My custom tables" in config_dir/custom_tables.json
  (`my_tables`, `add_my_table`), `workspace_tables`, `version_tables`, `source_document`, `evaluate`, badges
  ("Fs"), `table_list` (grouped dropdown), notices groups (`set_notices` / `notice_groups`, rendered by
  fs_notices.rs; model `custom::run::notice_entries`).
- Part Studio panel (`app/part_studio/custom_tables.rs`, hooks in config_panel.rs / behaviors.rs / mod.rs,
  `UiState.ct`): selector + ⋯ Switch to <FS>, sections with ⋯ Copy table (TSV), grid per live DOM (140 px
  columns, 34/35 px rows, #eaeaea header, #f5f5f5 cells, error #f9ebec + tooltip, ⓘ info), row hover = the part's
  edges (live 635), TABLE_ERRORS + notices group when a table throws, footer Add custom tables → 300 px flyout
  (Current document at a version / version graph / blue "N changes" box / FS › table tree; Other documents:
  categories, local documents under Owned by me / Recently opened / Created by me).
- Drawing (`app/drawing_custom_table.rs`, `DrawingEditor.custom_tables`): toolbar Custom table (after Hole table),
  view menu "Insert custom table of <Part Studio>…", dialog Add / Part Studio ▾, Insert / table ▾ with the pickers
  "Select a Part Studio" and "Select a custom table" (+ Done), Order, fixed corner; click places. Right-click:
  Custom table properties…, Split custom table below, Switch to Custom tables : <PS>, Switch to <FS> / Open Feature
  Studio in linked document. Properties (also double-click a corner): Document, Workspace, Custom table and Feature
  Studio links, version line, Sheet, Order, fixed corner, ✓ / ✕. Double-click a body cell edits it (`overrides`).
  Evaluated in the background per (source hash, FS source hash) in `ensure_custom_tables` (called by `refresh`).
- `--demo-tables`: demo drawing document + the live "fs tables" Feature Studio (FS sizes, FS two tables).
- Open: live request filed (REQUESTS.md asb 00:00) for the drawing table look (title row? error cell, TableArray,
  a thrown table), the Select a Part Studio dialog and the right-click menu of a workspace table. The shared table
  context menu (Format cell / Merge / Fixed corner …) is not Onshape's custom table menu (help custom-tables-14).

## 20 Oct: table and dimension menus, jogs, breaks, Dimensions tab
- Table menus (help custom-tables-14 / bom_table / cut_list_table): BOM, cut list and custom tables use
  `data_table_entries` (app/drawing_custom_table.rs) when the whole table is selected, `cell_size_entries`
  (app/drawing_table_dlg.rs: Size ▸ Columns Equally / Rows Equally, Resize) when cells are (`cell_sel`). General,
  revision and hole tables keep the older menu (hole: needs Onshape's Hole table dialog, TB20). `Table.layer` (Move
  to; colored / locked like annotations in build.rs), `order_table`, `move_table_to_sheet`. `MenuEntry::disabled`.
- BOM / Cut list table properties: `TableDlg.table` (edit mode, `open_table_props`, `apply_table_dlg`,
  `close_table_dlg`; Reference link instead of Insert).
- Linear dimension menu as live (`dimension_entries`, app/drawing_jog.rs; other dimension kinds keep the old
  menu). Jogs: `Annotation.jogs: [Option<ExtJog>; 2]`, drawn by `sheet::linear_frame` (shared with the app),
  `GripKind::Jog`, `Drag::Jog`. Break dimension: `Annotation.unbroken` (new dimensions: `!props.break_dims`, set in
  `pending_annotation`), `SheetGeometry::dimension_crossings` + `cut_polyline`. Test
  `jogged_extension_lines_and_broken_dimensions`.
- Dimensions tab: Text justification, Underline overridden (drawn on screen in drawing_scene `sheet_lines`), Break
  by default, Fit standard, Lock jog 45°, Ordinate style, Show radial dimension lines (`Drawing::dim_fit_ansi /
  ordinate_ansi / radial_dim_lines` default from the sheet standard). Justification / fit / ordinate / radial are
  stored only (live request asb 10-20 d).
- GD&T dialog limits (`limit_gdt_texts`), Add frame up to 5.
- Surgical fix in fs's select_item.rs (borrow of `s.configuration` in the source rows) to unblock the build.

## 20 Oct (later): related highlights, selection, menus, small rows
- app/drawing_related.rs: hovered / selected annotations show their references (vertex refs as blue points, edge refs
  drawn blue; `REFERENCE_BLUE` is provisional until live request asb 10-20 e) and group members get `state::ACCENT`.
- Selection is additive with a plain click (toggle), empty click clears (`select_click`), as Onshape.
- Dimension tool wakes circle centers (`woken_centers`, cleared by `set_tool`).
- `AngularDimension.three_point` + `in_sweep`: the 3 point angular arc on the other side gives 360° − angle.
- `cylinder_silhouettes` (drawing_smart.rs): Ø prefix for line-to-line on a cylinder's silhouette.
- `Drag::Reattach` (drawing_jog.rs `attach_grip_at` / `reattached`): drag a linear dimension's attachment grip.
- Annotation menus end with Clear selection, Zoom to fit, Delete; GD&T adds Paste / Paste as table.
- General table cell menu (`general_cell_entries`) and `Table::duplicate_rows / duplicate_cols`.
- Datum tool stays active; datum target sets A1… / B1… (`datum_target_letter`, `next_target_set`); crops refused on
  detail / break / flat views (`crop_refused`).
- Requests filed: asb 10-20 e (highlight colors), f (dialogs for the remaining ❌ rows), g (general table menus).

## 20 Oct (evening): hatch patterns, Cut from section, replication
- model/drawing/hatch_pat.rs: the live pattern lists (ANSI 9 incl. "ANSI31 general (drawing)" at index 8, ISO 14,
  General 31) as .pat-style line families (angle, origin, delta, dashes; ANSI/General in inches, ISO in mm, AR-
  patterns at 1/48). `lines()` clips them to a profile (even-odd holes). Stored indices of older documents keep their
  meaning (old ISO / General entries first; `display_order` gives the dialog order). `HATCH_PATTERNS` is gone.
- Pattern icons: `cargo test -p noshape-model hatch_icons -- --ignored` writes app/assets/icons/hatch-<group>-<i>.svg.
- app/drawing_hatch.rs rebuilt to live 468–472 (tabs, Closed profiles list, editable Scale / Angle, preview canvas);
  the dialog's hatches preview tan, profiles blue (`hatch_profile_outlines`, drawn in drawing_related).
- Styles panel: hatch regions (`render_hatch_styles`).
- V24 Cut from section: `DrawingView::cut_from_section`, `views::pre_cut`; `section_geometry` chains the parent's
  removal cut through `cut_pieces` so edge names still map.
- V68 app/drawing_replicate.rs: Select for replication ▸ View / Region…, Replicate annotations, Replicate region
  dialog; references re-found by same ref or same-kind snap at the mapped point. os_menu labels "Text\tbadge" draw a
  blue "beta" pill (surgical, os_menu.rs `label_el`). Request asb 10-20 h asks for the notification wording and the
  placing details.
- Restored the broken-out view menu "Edit section" (lost with `view_more_entries`).
- Surgical: `.into_any_element()` on `input_frame` in start_search / assembly_positions / release_dialog /
  drawing_detail (the InputFrame change had left the app uncompilable for an hour).
- qa-round-4 (drawing): app/drawing_toolbar.rs replaces `render_toolbar` (drawing_ui.rs) and `sketch_toolbar`
  (drawing_sketch.rs): tools are `TbTool` descriptors per category; when the full width doesn't fit beside the
  rail, every category collapses into one split tool (first tool's icon + ▾ listing the category; tools with
  their own ▾ contribute their items), as live's narrow Part Studio (no overflow button). Live drawing has no
  "Search tools…". Request asb 10-20 i asks for the live narrow drawing toolbar to confirm faces / lists.
- The open Sheets panel is a flex sibling before the canvas (pushes it); `toggle_sheets_panel` pans by half the
  panel width so the sheet keeps its screen spot (live drawing/30 vs 469: the sheet doesn't move);
  `dialog_left()` is 0; the panel toggle is a canvas child drawn under the dialogs.
- Surgical: assembly_panels.rs:1581 `flex_shrink()` → `flex_shrink_1()` (blocked every app build since 09:08).

## 20 Oct (late): dimension palette, hole callout panel, units, Units and precision (for ui2)
**Dimension palette / hole callout panel — app/drawing_palette.rs** (`DrawingEditor::palette: Option<PaletteDlg>`).
Behaviour is done; the look is ui2's (only touch `render_palette`; the state and actions are stable):
- Open: double-click a dimension or a hole callout, or the dimension's right-click Edit… →
  `open_dim_palette(id)` / `open_hole_callout(id)` (both via `open_annotation_panel`). The old
  "Dimension properties" panel (`Panel::Dimension`) is gone.
- `PaletteDlg` fields: `hole` (callout or dimension), inputs `above / prefix / value / suffix / below /
  tol_a / tol_b` (InputStates; `value` overrides a linear dimension's text when changed), `tol` (the list
  entry name, `DIM_TOLERANCES` / `HOLE_TOLERANCES`), `target` (where symbols go), `cur` (the edited
  Annotation, previewed live), `orig`.
- Actions: `edit_palette(|d| …)` for dropdown / toggle changes (then re-reads the inputs and previews);
  `close_palette(accept)` — Enter or a sheet click accepts (one undo step "Edit dimension" / "Edit hole
  callout"), Esc cancels. Lists: `SYMBOLS`, `PRECISIONS` (with "(Drawing)" = None), `DimUnits::CALLOUT` +
  the "(Drawing)" entry, `HoleCount::ALL`, `FitType::ALL`, `FitStacking::ALL`, `fits::hole_classes /
  shaft_classes`.
- Placement: `palette_at(id)` (beside the annotation text's upper right, editor px).
**Model (DimFormat, D31–D38):** `above`, `below`, `reference` (parentheses), `inspection_frame` (oval),
`dual: Option<bool>`, `count: Option<HoleCount>` (stored; dimensions don't count yet), `fit: FitOpts { mode
(Fit / WithTolerance / ToleranceOnly), fit_type, iso: Option<bool>, stacking, precision, hide_hole,
hide_shaft }`. `format::dim_text` renders parentheses, above / below (`DimText::above/below`), the oval
(`DimText::oval`, drawn by `SheetGeometry::dim_label`), stacked or diagonal fit classes and the fit
deviations (`fits::deviations`, precision `fit.precision` → Drawing properties `fit_tolerance_precision`).
`AnnotationKind::measured(rotation)` gives a linear / radial value for the value box.
**Hole callouts:** `Annotation::hole: Option<Box<HoleCalloutOpts { count, override_tol }>>`; above / below /
prefix / suffix / units / reference / dual come from `format` (shared with dimensions). `build::hole_count`
(None / view / face / pattern / part / face excluding patterns), `build::hole_callout_count`; without
Override the callout ignores `format.precision / tolerance`; with it Symmetrical / Deviation / Limits /
MIN / MAX / Fit apply to the diameter.
**Units:** `DimUnits::{Yard, InchFraction, FeetInches}` (1/64 fractions, `1' 2.5"`), `DimUnits::CALLOUT`,
`DimUnits::imperial()`.
**Drawing properties › Units and precision:** `fit_tolerance_precision`, `dual: DualDims { show,
show_unit, dim_location, hole_location (DualLocation Top/Bottom/Left/Right), units (None: mm on inch
drawings, in on metric), precision, tolerance_precision }` (stored; dual values not drawn yet), footer
`template: Option<String>` ("Update properties from a template…": the DWT list, `templates::template_props`)
and `locked` (`set_drawing_prop` ignores every change but the lock while locked). Rows are in
drawing_drawprops.rs tab 0 and `drawing_props_footer`.
**Other:** Table tool ✓ disabled until placed, Rows focused + selected; Detail view scale blank until the
circle is placed; GD&T type already defaults to Position (the ⌖ vs ⊕ difference is the glyph); toolbar
right-end tools (title block, sheet ▾, zoom to fit; not in live) removed with their menus
(`template_entries`, `title_style_entries`, `sheet_size_entries`, `Panel::Scale`), so the full toolbar fits
at 1920 px and collapses only below ~1790 px.

## For drw2 (asb, 20 Oct): built but not yet checked in the app
- Checked by asb today (no need to redo): SK9 / SK17 hatch dialog and styles, V24 Cut from section, V68 menus
  and Replicate region dialog, toolbar collapse at 1440 px, Sheets panel push.
- Not yet checked (your 🔧 queue, most recent first): D31–D38 dimension palette, A57 / A58 hole callout
  palette, V68 replication result after the snap fix (copy a view, delete a dimension on the copy, Region… →
  Replicate annotations → place over the same hole), V63 Adjust linestyle, V23 Edit section, D42 / A64 Styles
  rows, Units and precision Dual / Fit class / footer, Table tool ✓ / Rows focus, Detail scale blank,
  centermark arms, callout / surface finish leaders, radial dim lines, ISO ordinate.
- Builds: behaviour group in /root/target-behave. asb uses Xvfb :104; please use another display.

## From drw2 (20 Oct): shared functions I touched (docs/handoff/drawing-verify.md has the details)
- `views.rs`: `compute_view` (new Projected arm `projects_section`; the broken-out arm now uses `broken_out_cuts`),
  `view_key` (Projected views hash their parent when they inherit cuts), `broken_out_geometry` now takes
  `&[(BrokenOut, ViewFrame)]` (each cut in the frame it was drawn in). New `projects_section`, `broken_out_cuts`.
  `DrawingView.cut_from_section` also means "Shift+click projected view of a section" now.
- `sheet.rs`: `add_border_and_title_full` split out `add_zones`; `Sheet.format: SheetFormat { border, zones, start }`
  (mod.rs), `SheetSize::standard_zones` (ISO A4 = no zones, live 560), `zone_letter` (skips O).
- `templates.rs`: `DrawingTemplate` gained `units`, `decimal_comma`, `format`; build them with
  `DrawingTemplate::standard(..)`. `units_note_of(DimUnits)`.
- `drawing_tools.rs`: `double_click(triple, ..)` (triple-click a note selects all), Projected refuses Detail parents.

## 20 Oct (after v0.4.1): Datum dialog, palette layout, small rows
- app/drawing_datum.rs: the Datum dialog (`DrawingEditor::datum_dlg`, synced with Tool::Datum / DatumTarget):
  Datum tab label (`label_text`, used by the Datum tool's annotations), Datum target tab with area / style /
  area size and the Targets rows (pending annotations previewed; ✓ `close_datum_dlg(true)` commits). The views
  ▾ "Datum target" item is gone (it's the dialog's tab, as live). Model: `DatumTargetArea::{Dot, Custom}`,
  `Annotation::target: Option<Box<DatumTargetOpts { style, origin, origin_ref, angular, flip_x, flip_y,
  hide_dims }>>`; sheet.rs draws the moveable pointers and the X / Y position dimensions from the origin.
- Dimension palette: the dimension form now follows live 15's sizes (prefix 80 / value 85 + ▾ / tolerance
  icon 45 / units 60 / suffix 80; bottom row Ø ▾, reset, (XX), oval, dual ▾).
- Detail view scale: blank until a profile exists (any shape), then the parent × 2.
- SK8 woken endpoints (`woken_points`, `sketch_aligned`), V26 refusals (`Sheet::has_angular_cut`), V49 view
  toggles (`DrawingView::sketch_appearances / model_annotations`).
- Surgical: assembly_panels.rs 2757–2791 `Rc` → `std::rc::Rc` (left unresolved for 20 minutes).

## From drw2 (20 Oct, ~14:40): I'm taking in-place note / table-cell editing (A2, K24, TB1, TB3)
- Live (behaviours.md "Notes: alignment, rotation, menus" and "General table text entry"): the text is typed in
  place on the sheet (caret in the note / cell, dashed text box) and a floating "Note" ✓ ✕ formatting panel sits
  just above it; Tab / Enter (cell below) / Shift+Tab / Alt+Enter move through table cells.
- I'll change how `Panel::Note` / `Panel::Cell` render (drawing_ui.rs `render_panel`, drawing_note.rs
  `note_panel_rows`: the Textarea moves out of the panel into an overlay at the note's place; the formatting rows
  stay yours). Please avoid restructuring those two functions until I post "done" here; style / model changes
  in `NoteStyle` are fine.
- Thanks for the "For drw2" list. I'll take D31–D38, A57/A58, V68, V63, V23, D42/A64 … after this.
- asb → drw2: your `drawing_inplace.rs::sheet_to_local` duplicated a private one in drawing_sketch.rs (same
  math); I removed mine, so yours is the only one now.

## PAUSED (asb, after the out-of-memory reboot) — exact state
- Nothing of mine is running (no cargo, no app). Work dir /tmp/asb was wiped by the reboot (scripts bg.sh,
  dd.sh, shot.sh, verify.sh gone; recreate from this file's notes: builds use
  `OCCT_ROOT=/root/occt CARGO_TARGET_DIR=/root/target-behave nice cargo …`, app on Xvfb :104).
- Committed in dc0da0a ("Integrate: post-v0.4.1 work"): most of today's drawing work (palette, Datum dialog,
  SK8, V26, V49, detail scale, sheets panel, toolbar collapse, hatch, replication, Cut from section).
- Uncommitted, mine, complete (no half-done edits):
  - drawing_datum.rs: pending-target preview via `datum_rows_drawing`, ids by `zip(first..)` (clippy fix).
  - drawing_sketch.rs: `type Aligned` (clippy fix); my duplicate `sheet_to_local` removed (drw2's in
    drawing_inplace.rs is the one).
  - drawing_tools.rs: tool preview includes the Datum dialog's pending targets; `overlay.infer.extend(...)`
    instead of overwriting (keeps sketch inference lines).
  - drawing_more.rs: target numbering counts the dialog's pending rows.
  - drawing_extra.rs: sheet tabs hide under the Datum dialog and the palette.
  - Other uncommitted drawing files (drawing_ui.rs, drawing_view.rs, drawing_palette.rs, drawing_props.rs,
    drawing_table_tool.rs, drawing_gdt.rs, drawing_note.rs, drawing_inplace.rs) also hold drw2's / ui2's
    work; check `git diff` before assuming they are mine.
- Not yet verified after these last edits: `cargo clippy -p noshape --all-targets -- -D warnings` (the
  lead flagged drawing_datum.rs:219 and drawing_sketch.rs:175; both fixed, run not finished), the model
  drawing tests (last green: 88 passed, before these app-only edits), and the app check of the Datum
  target flow (preview, B1/B2 numbering, origin pick → X/Y dimensions) and SK8 inference lines.
- Next after RESUME: clippy + `cargo test -p noshape-model --lib drawing::`, then the app checks above,
  then the remaining ❌ rows (V15, V52, V54, V70, A30, A65, S13, SK7, EX13, EX19, K27, CR16/CR18).
