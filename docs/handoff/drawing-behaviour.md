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

## RESUMED (asb, 20 Oct evening): checks done and ui2 round 8
- Checked in the app (19:35 build + later): palette opens with the prefix focused (live blue border + 8 px
  glow, deferred focus), typing previews ("2X 100"), Esc reverts (capture_key_down), choosing a tool accepts,
  the palette stays where it opened (`PaletteDlg::at`); Datum dialog targets preview, number B1 / B2
  (count the committed sheet + pending rows), origin pick fills the X / Y distances and draws the 12 / 12
  dimensions, typing 20 moves the target, ✓ commits; SK8 red dashed alignment from a hovered endpoint;
  Detail view scale blank until the profile, then 2:1.
- Fixed: `overlay.infer` was overwritten after the tool overlays (sketch inference lines lost).
- ui2 round 8:
  - Sheet menu "Properties…": ours already matches both live captures — one sheet: Rename… | Properties… |
    Duplicate | Copy; several sheets (drw2's capture drawing/760–772): Rename… · Duplicate · Paste · Copy ·
    Move up / down · Delete, no Properties…. ui2 compared the one-sheet capture with a multi-sheet drawing.
    Fixed alongside: a sheet chip's menu (panel closed) now uses that sheet alone.
  - Inspection table rebuilt to live 95 / 98 (bordered View: select, ⋯, collapsible Datums with the info box,
    Characteristics with Item / Sheet / Type / Units / Nominal value / Tolerance / Upper limit / Lower limit /
    Nominal & Tolerance, "--" items, read-only grey cells). Checked in the app.
  - Styles panel to live 474 (plain values, "number ▾ unit", colour swatch, inline arrow picker, Text gap
    spinbox with ▲▼ — `DrawingEditor::styles_gap`). Checked in the app.
  - Title blocks vs live 156 / 164: DRAWN `{user.name}` (upper case, `SheetInfo::user_name` =
    `noshape_store::local_user_name()`), APPROVED `{release.approver}` / `{release.date}` and TITLE three lines
    `{drawing.title1..3}` (empty → "----"), MATERIAL blank, thin frame, DO NOT SCALE DRAWING sized to its cell,
    multi-line field values. Existing drawings keep their field values; new ones get these defaults. The ISO
    block already is live's ANSI-style block with the logo cell ("noshape", no Onshape assets).
  - Detail view prompt field per shape / "Profile created", shape buttons un-pressed after the profile
    (behaviours.md "Detail view dialog: prompt field text per shape").

## HANDOFF (asb → replacement, 10-20, account switch)

State: no app instance running. Nothing committed (per rules). One background job may still be running:
`/root/asb-scratch/bin/bg.sh mt test -p noshape-model --lib drawing::` → log `/root/asb-scratch/mt.log`,
marker `mt.done`. Check that log first: it is the first run of the new test `datum_on_gdt_frame_follows_it`.

Verified earlier this round (in the app): the fresh-drawing title block (ANSI_A_INCH via + › Create Drawing…)
matches live 164. DRAWN shows "----" only because the test env has no USER.

### V15 (aux view reference indicator): code written, model compiles, app not compiled yet
- model `mod.rs` `AuxRef`: new `offset: Option<f64>` (sheet mm from the edge toward the view) and
  `ext: Option<[f64; 2]>` (line extent on −/+ edge side), both serde default + skip-if-none.
- model `build.rs`: new `pub struct AuxIndicator`, `pub fn aux_indicator(sheet, id, &parent_placement)`,
  consts `AUX_OFFSET` 12, `AUX_EXT` 10, `AUX_LEG` 8. Indicator redrawn the Onshape way (help images
  ui-ref/drawing/drawings__auxiliary-view-02/05/10.png): a chain viewing-plane line between two elbows, a leg
  from each elbow back toward the edge, an arrow at the leg tip pointing at the edge, the letter beyond the tip.
  Offset is clamped to [AUX_LEG, the middle of the aux view].
- app `drawing_aux.rs` (new impl block at the end): `aux_indicator_of`, `aux_grips` (elbows, mid, tips;
  shown via `selection_handles` in drawing_more.rs when the aux view is selected), `aux_grip_at`,
  `drag_aux_grip` (an end grip lengthens/shortens its side; the mid grip slides the offset),
  `aux_edge_highlight` (the parent's edge segments through the anchor, parallel to edge_dir).
- app `drawing_view.rs`: `Drag::AuxIndicator { view, grip, orig }`. `drawing_extra.rs`: start in
  `extra_start_drag` (the selected view's grips first, then the `PickTarget::AuxIndicator` arm), move in
  `extra_drag_move`. `drawing_tools.rs`: added to the move/commit lists, undo name "Edit view reference indicator".
  `drawing_related.rs` `push_related_highlights`: parent edge in REFERENCE_BLUE when the aux view is selected.
- Unverified details asked of live capture (REQUESTS.md "asb (10-20 u)"): default offset and length, letter
  position, snap points and which side an end grip changes, view vs indicator selection. Adjust once answered.
- Row V15 in docs/parity/drawing.md is still ❌ (stale note). Set it to ✅/🔧 after the app check.

### A30 (datum on GD&T frame / dimension lines): code written, uncompiled in the app
- model `mod.rs`: `AnnotationKind::Datum { anchor, label, on: Option<DatumOn> }` and
  `pub struct DatumOn { ann, line, seg, t }`: segment `seg` of the `line`-th polyline that annotation `ann`
  draws (counting only lines with its pick), at `t`. All initializers updated (tests.rs, showcase.rs,
  drawing_extra.rs).
- model `sheet.rs`: `type Starts`, `type OnLine`, `SheetGeometry::attached_point`. The Datum layout:
  when attached, the box = position + (current point − stored anchor), so it follows the target. The triangle
  sits on the attached line square below the box (projection; clamped on closed frames). Past the end of an
  open line (an extension line) a thin line carries it on. The target must come before the datum in
  `sheet.annotations`; otherwise it falls back to the stored anchor.
- app: `DrawingEditor.datum_on` field (drawing_view.rs), cleared in `set_tool` and after placement.
  `datum_attach_at(c)` (drawing_extra.rs, before `open_annotation_panel`) finds the nearest GD&T or dimension
  line segment. The Datum tool's first click uses it. While hovering, a red crosshair overlay (`overlay.tinted`)
  marks the point.
- Not done: "Dimension arrows or lines it covers are hidden"; dragging the attached datum's base grip.
- Test added: model tests.rs `datum_on_gdt_frame_follows_it`.

### Build state
- Last `clippy -p noshape --all-targets -- -D warnings` failed on my `ViewId` import in drawing_aux.rs
  (now fixed, not yet rechecked). It also failed on other agents' in-progress files: part_studio/mod.rs:48
  (unused Button/ButtonVariants), mod.rs:1591 lifetime, toolbar_customize.rs:328 `.small()`. Earlier
  clippy-only leftovers belong to others: cached_slice.rs:22, part_studio/annotations_ui.rs, part_studio/sketch.rs:2568.
- Scripts: `/root/asb-scratch/bin/{bg.sh,dd.sh,stop.sh,shot.sh}` (target /root/target-behave, waits for
  8 GB free). App check: `dd.sh` (demo drawing on Xvfb :104), then `shot.sh NAME`, then `stop.sh`.

### Next steps
1. mt.log result: 89 passed, 1 failed. `datum_on_gdt_frame_follows_it` panics at the first assert with `[5.0, 252.375]`. It matched the border zone letter "D", not the datum, so this is a bug in the test. Use a label that doesn't clash (e.g. "Q") or filter texts by the datum's pick, rerun, then fix the code if it still fails. Rerun clippy once the others' files compile.
2. In the app, check V15 (select the aux view: edge highlight + 5 grips; drag the ends and the middle) and
   A30 (Datum tool over a GD&T frame edge: red crosshair; place; move the GD&T and the datum follows; on a
   dimension extension line past its end). Quit the app after. Update rows V15 and A30 plus this file.
3. V54 (view label Ctrl+C / Alt+drag) needs view labels to be selectable entities. It is waiting on the live
   capture request above.
4. The remaining ❌ rows: V52, V70, A65, S13, SK7, EX13, EX19, K27, CR16 (probably stale: Insert view dialog
   exists; check prefilling), CR18, V5 (no MBD data).

## asb (new account, 10-20 night): state
- Fixed `datum_on_gdt_frame_follows_it` (label "Q", so it no longer matches the zone letter "D"); passes.
- Full `cargo clippy --locked --workspace --all-targets -- -D warnings` was clean at 22:05 (before the last edits below).
- Checked in the app (Xvfb :104, screenshots /root/asb-scratch/c*.png, v*.png, a*.png, s*.png):
  - **V3 / V41 named views**: `DrawingView.named_view` + `views::named_view_frame` (camera → frame; test
    `named_view_frame_is_the_camera`). Insert view panel, view menu › View orientation and View properties list the
    source Part Studio's named views after Isometric; a deleted one stays checked + grayed in the menu; choosing a
    standard orientation clears `named_view` / `custom_frame` (`set_standard_orientation`). Section planes of named
    views: not applied (REQUESTS w).
  - **EX13 Print…** (document menu, live 478): `DrawingEditor::print` → all sheets as PDF in $TMP/noshape-print,
    opened with the system viewer. Part Studio / Assembly print setup: psb / skb (reported to the lead).
  - **EX19** Preferences › Drawings (`drawing_prefs.rs`, `ListPrefs.drawing_light_model_space`), spacing matched
    to live 482 (Toolbars' trailing spacer 31 → 15 px in preferences_window.rs). Applying it waits on REQUESTS v.
  - **K27** Drawing tab in Keyboard shortcuts: present (someone else's work) → ✅.
  - **V15** aux indicator: edge highlight, 5 grips, elbow lengthens its side, middle slides it → ✅.
  - **A30** datum on a GD&T frame edge / extension line: crosshair, attach, follows the frame, line carried past the
    end. Found + fixed: every datum's leader ran through its box to the far edge (`box_exit` was passed `-u`; it
    walks against `d`), also in main. New: a datum on a dimension hides the arrowheads it covers
    (`SheetGeometry::hide_covered_arrows`; test `datum_on_dimension_arrow_hides_it`). Not yet rebuilt/checked.
    Still missing: dragging the attached datum's base along the line.
- Capture requests filed (REQUESTS.md): v (DWG model space), w (named views / assembly insert / prefill: CR16, V6,
  V70, V71), x (cross-sheet linked views V52 — Onshape moves views alone and keeps them linked; ours moves
  descendants and orphans become base views), y (SK7 driving sketch dimensions), z (MBD datums in drawings: V5,
  CR18, A65; psb's Part Studio datums exist in model/annotations.rs), z2 (Select a DWT, S13).
- Noted for ui2 / lead: `file_tab_view::pick_file` (Insert image / DXF / CSV) is a centred 560 px modal; live
  (behaviours.md 1729, drawing/123–125) is a 300 px panel with Current / Other documents tabs and an Import… footer.

### asb, 10-20/21 night (after the live answers v–z2): written, not yet built / checked in the app
- **Drawing S toolbar** (from ui2): `drawing_keys.rs` `SHORTCUT_CATALOGUE` (38 tools, toolbar order) and
  `DEFAULT_SHORTCUT_TOOLS` (provisional until REQUESTS z3), panel to the live sketch/38 spec (175×62, 2×5, cursor
  +10 / −30, S again reopens). `ToolbarPrefs.drawing_shortcut_tools`; Preferences › Shortcut toolbars has
  Sketch | Part Studio | Assembly | Drawing (Drawing = index 3; skb added Assembly = 2 meanwhile).
- **View properties**: read-only "Named view" row (live 1104), a deleted named view still listed (live 1117–1119),
  "View render mode" Best quality / Best performance (`DrawingView.best_quality` → exact HLR; V45).
- **Assembly views (V6 / V70 / V71 / CR16, live w)**: `DrawingView.named_position`, `display_state` (base),
  `own_position: Option<AsmPosition>`, `own_display: Option<DisplayChoice>` (children; None = Follow parent);
  `views::view_source` applies the nearest override; `SourceKey.position / display`; regenerate_source solves the
  named position (start + locks) and filters occurrences by the display state (None = Show all, which now also
  shows instances the assembly hides). A missing display state → no source → blank views + toast "Failed to resolve
  display state." (`failed_sources`, retried on Update). Insert view rows Explode/Position + Display state (no
  "Show annotations from model" for assemblies), prefilled from the open assembly (`AssemblyView::drawing_prefill`:
  `ui.np_applied`, `ui.ds_selected`). View menu Explode/Position ▸ / Display state ▸ (current greyed; sections and
  details change their root). View properties rows with "Follow parent" for children, pink empty field when the
  display state is gone. Test `assembly_views_display_state_and_follow_parent`.
- **Imported DWG / DXF (EX19, live v)**: + › Import… of a DXF / DWG now only adds the file tab. File tabs got their
  own menu (`workspace::file_tab_menu`, live file-tab menus; no Translate…): DXF / DWG › "Create Drawing of <file>…"
  → `create_cad_drawing` → `model::drawing::imported::drawing_from_cad` (sheets "Model" + layouts with colours and
  texts via the new `noshape_sketch::cad::parse_cad_spaces`; R12 paper space locked). Model sheet: no paper / border /
  title block; canvas black (Dark view) or white (Light view) from Preferences › Drawings, foreground ink white on
  dark (`DrawingEditor.model_bg`, a white ColorSpan at the front). Sheets panel: "Model" unnumbered, layouts numbered
  from 1, locked rows grey (greyed menu, can't open), Model has no menu, double-click on the active Model → Sheet
  properties with only Size "Letter (8.50 x 11.00 Inches)" greyed. Toolbar: view tools + model tables greyed
  (`TbTool.disabled`, also in collapsed categories). Tests `spaces_keep_colours_and_texts` (sketch),
  `model_space_sheet_keeps_colours` (model). Not done: layout paper size from the DWG's page setup (extents used),
  the layout icon, start_window.rs's documents-page import still makes a drawing (ui2's file).
- **Sheet switches are undoable** (live z2 (5)): `Drawing.active_sheet`; `commit` stores the sheet on screen;
  `switch_sheet` commits "Switch sheet"; `reload` shows the stored sheet (undo / redo); editors open on it.
- **Datums**: leader stops at the box (box_exit sign bug, also in main); a datum on a dimension hides the covered
  arrowhead (`hide_covered_arrows`, test `datum_on_dimension_arrow_hides_it`).
- Still to do from the live answers: V52 linked views across sheets (x), SK7 driving sketch dimensions (y; sign
  ignored, not flipped), MBD datums V5 / CR18 / A65 (z), S13 Select a DWT (z2), named views with section planes (w).

### asb, 10-21 (v0.4.3 code freeze): state
- Tree state at the freeze: `cargo clippy --locked --workspace --all-targets -- -D warnings` clean; model drawing tests
  99 passed.
- Checked in the app this round: S toolbar, View render mode row, assembly Insert view rows / menu, sheet-switch
  undo, imported DXF (file tab only → Create Drawing of … → black Model sheet, colours, Model row, Size-only Sheet
  properties), SK7 sketch dimensions (line 80 → left end fixed; circle Ø30 about its centre).
- Built but NOT yet checked in the app: SK7 reopen focus (deferred focus on double-click), assembly view menu
  order (Explode/Position + Display state right after View orientation, "Insert BOM for <asm> ...", Rotate view ▸
  Vertical to... / Horizontal to...), model-space zoom to fit, **MBD datums** (`drawing/mbd.rs`, `drawing_mbd.rs`,
  MBD status panel rows / message / badge / Show; test `mbd_datum_goes_to_the_edge_on_view_and_can_be_hidden`),
  **V52 linked views** (`drawing/links.rs`: hidden mirror views with ids encoding sheet uid + view, rebuilt by
  `Drawing::sync_links` in every commit; `move_views_to_sheet_linked` used by Move to sheet / View properties ›
  Sheet; Sheets panel ↗ badge; linked views drag freely; edits to a mirror go to the real view; test
  `moving_a_parent_leaves_its_children_linked`).
- Known gaps: MBD datum A in live sits inside the view (box on −normal) while ours sits outside; MBD characteristics
  rows (needs MBD dimensions placed in views, CR18); S13 Select a DWT not started; live REQUESTS z3 (drawing S
  toolbar defaults) still open.

## asb (new account, 10-21 day): state
- Took over mid-edit. The tree failed on `Sel::AuxIndicator` not covered in `context_entries` (drawing_tools.rs;
  V54 / aux-indicator selection work by the previous asb): the indicator's menu is now
  `DrawingEditor::aux_indicator_entries(id)`, a match arm (multi-selection menu comes first, as for any entity).
- **Rename sheet Esc** (lead): with ui's revert-on-Esc input patch (vendor/gpui-base input `escape`), Esc after an edit is
  consumed by the input's Escape *action*, which GPUI dispatches before key listeners, so the dr-panel's
  `capture_key_down` never saw it and the panel stayed open. `render_panel` (drawing_ui.rs) now also has
  `capture_action::<Escape>`: panel closed, nothing applied, focus back to the canvas. Same for every small dr-panel.
- **"Onshape" strings**: drawing_create.rs SOURCES / template documents read "noshape" (done before me). A grep of
  app/drawing*.rs, model/drawing, scene/drawing_tools finds "Onshape" only in comments (and a FeatureScript import path
  in a test), nothing user-visible.
- **MBD datum side** (known gap): live drawing/1070 + dom/mbd-drawing-json.json put both boxes *below* the edge
  (A inside the Front view under its top edge, B outside under the Top view's bottom edge): the model symbol lies in
  the face, edge-on in the view, so the box goes sheet-down. `mbd::model_datum_points` flips the projected normal
  when it points up; vertical edges keep the outward side (no live sample). Test updated.
- Parity: the only ❌ row left was V54 (view label Ctrl+C / Alt+drag), whose code is in drawing_label.rs (selection,
  drag, Copy, Ctrl+C note, Alt+drag note): needs the app check.
- api's slice-4 freeze (21 Oct): don't edit drawing_tools.rs `pending_view`/view_options, drawing_section.rs
  `apply_section_options`, drawing_detail.rs `apply_detail_options`, drawing_insert.rs `insert_sketch_frame`,
  drawing_hatch.rs `parse_auto`/`impl HatchDlg`, drawing_table_tool.rs COL_W/table_tool_size/table_tool_table,
  drawing_gdt.rs GDT_TYPES/gdt_values until the lead lifts it.
- **Queued for after the freeze (lead, from icons2's live check; keep the Revision table button):**
  - Toolbar pitch: live dom/drawing-toolbar-widths.json (w1920): buttons 30 px wide on a 32 px pitch (Undo @44, Redo @76,
    Update @124; Insert view @172 … Crop view @428; Dimension @477 + caret 12 px @507, Ordinate @523). A separator
    takes 18–19 px between groups. Ours uses ui::TOOL_CELL 32 + 2 gap → make the drawing toolbar 30 + 2.
  - Crop view's ▾ (`views_menu` in drawing_toolbar.rs): live has none; drop it (the jogged / partial / crop-shape
    choices live in the tools' own dialogs).
  - Styles panel empty-state box (live drawing/95-panel-styles; classes IntimationMessageheadingStyle /
    IntimationMessageStyle): box at panel x+4 / y+5, right margin 7, padding 15, 1 px border #BCE8F1, bg #D9EDF7,
    text #31708F; heading bold 14 px, body 13 px, both 20 px line height, no gap.
  - Drawing right rail ~25 px lower than live (live 95: rail icons at y 511 / 545 / 578 / 612, 34 px pitch).
- Model drawing tests: 102 passed after fixing stale `jogged_and_aligned_sections` (labels are live's spaced
  "SECTION C - C" since the view-label work; tests.rs:251 updated) and the MBD test (box under the edge).
- App build blocked (21 Oct) by others' WIP: model custom/host/query.rs (fixed by its owner), then
  app part_studio/sketch.rs (sketch_mode move). App checks still pending: Rename sheet Esc, MBD datums, linked views,
  assembly view menu, V54.
  - Drawing properties footer (lead via icons2; live drawing/160): "Update properties from a template..." larger and
    underlined; "Lock drawing properties" greyed (disabled) with a ? help icon at the right.
- **App checks (21 Oct, asb):** Rename sheet: type "XYZ", Esc → panel closed, name kept "Sheet1" ✅. MBD: datums A (top
  face) and B (front face) via Model definitions › Datum; Create Drawing › Suggested views → B under the Top view's bottom
  edge, A inside the Front view under its top edge (= live 1070), badge ✓ (and red ! in the other drawing), drag only
  along the normal ✅. Linked views: Move to sheet… › Sheet2 moves the section alone at the same place, ↗ badge row,
  Sheet1 keeps the A–A line ✅. Assembly base-view menu = live 1108 except Paste (now shown when the clipboard has drawing
  content, before Copy). V54 label: hover / selected look, Copy menu, Ctrl+C/V note at the cursor, Alt+drag copy ✅.
- Written after those checks, awaiting a build (others' WIP blocked: assembly_panels.rs, assembly_standard.rs):
  - `push_datum_selection` (drawing_datum.rs) + `datum_selection_picks` (skipped in the SELECTED flags, drawing_view.rs):
    live 1073 selected-datum look.
  - Label menu without the view's duplicate Copy (drawing_label.rs); view menu Paste (drawing_props.rs);
    `paste_views` selects pasted annotations too (live u-948).
  - Demo drawing's Sheet field is `{sheet.number} / {sheet.count}` (was literal "1 / 1", copied to new sheets).
- Script: /root/asb-scratch/bin/retry_build.sh (rebuild after a change, max 15 tries, cp --remove-destination);
  ddk.sh = dd.sh keeping ct.noshape (datums / drawings survive restarts).

## asb 10-21 afternoon (freeze lifted): done
- Insert view dialog: ✕ tooltip removed (live dialogs have none; psb's rule).
- Toolbar pitch to live (dom/drawing-toolbar-widths.json): drawing_toolbar.rs consts CELL 30 / CARET_W 12 / GAP 2 /
  SEP_W 14, `split()` (1 px in, 2 px after the caret), `sep()`, toolbar pl 4; `ui::undo_redo_with(doc, cell, titles)`
  (drawing titles "Undo (ctrl+z)" / "Redo (ctrl+y)"). Checked: icons at live x up to the table group; ours then runs
  32 px right because of the Revision table button (kept, lead).
- Crop view: no ▾ (live); new **Crop view dialog** `drawing_crop.rs` (help crop-view images): Circle | Spline ▾ (Add
  spline point) | Polygon, Spline first; "Create closed profile" → "Edit closed profile" + Profile ×; the closed profile
  waits for ✓ (Enter too); view menu **Edit crop** reopens it. `crop_done` replaces the direct commits in drawing_extra
  (circle) / drawing_more (polygon, spline). `Overlay.dots` (scene) for coloured points. Checked in the app (spline,
  ✓, Edit crop). The jogged / partial section entries of the old ▾ are next (Section tool extra snap points, live).
- Styles panel: info box to live 95; header 39 px (37 + 2 underline, live); footer pinned with the refresh button,
  "Revert to drawing properties" #666 and the help icon (`styles_with_footer`, all three states).
- Right rail centred (`top 50% − 66`): icons at y 512 / 545 / 579 / 612 = live.
- Drawing properties footer to live 160 (underlined italic field, 25 px DWT, 12 px Lock + help icon).
- Clippy fix lead asked: drawing_aux.rs `type SegFn`.
- Live request filed: REQUESTS "asb (10-21 a)" (Section tool jogs, Crop view dialog details).

## From ui (21 Oct): drawing fields are Wt widgets, not Part Studio fields (live, lead's input task)
Live ("Input behaviour outside feature dialogs" in behaviours.md, dom/oi-*.json) shows the drawing editor's fields
behave unlike the Part Studio's.

**Done (shared input side):**
- `DrawingEditor::input()` and the direct `InputState::new` sites in the drawing panel files now use
  `.select_on_focus(false).revert_on_escape(false)`: caret at the click, and Esc never reverts the text (it passes
  to your panel).
- Sheet properties width/height (`XeInput`) keep select-all and only drop the revert.
- Not changed: drawing_create.rs, drawing_export.rs (Angular/Bootstrap dialogs, so Part Studio fields) and
  drawing_sketch_dim.rs (unknown; please check).

**Yours (value rules per widget, from live):**
- **Dropdown with a number (`XeDropdown`, e.g. Text height ▾ 0.1200 in):**
  - Commit (Enter, Tab, click away) parses the leading number only, ignores any unit (`3 mm` → 3.0000 in), and shows
    4 decimals.
  - Non-numbers silently revert to the last value (no error look).
  - ↑/↓ and the wheel do nothing.
  - Esc does nothing in Drawing properties. The Note editor closes on Esc (note discarded).
- **Spin box (`XeSpinBox`: Stack gap 0.0500, Text gap 0.0900, Dashes 1.00):**
  - Strict number. Invalid shows pale orange `rgba(221,82,40,0.1)` with the hover "Must be a number".
  - ↑/↓ step by the field's step (0.05 in for gaps, 0.10 for Dashes), clamped at 0. The wheel does nothing. Spinner
    arrow clicks step.
  - Esc does nothing. Commit on Tab or click away. Decimals as shown (4 for in, 2 for Dashes).
- **`XeInput` with a unit (Sheet properties custom width, "11 in"):**
  - Selects all on focus. `12` → "12 in"; `300 mm` is kept; `1+1` → "1 in" (leading number); non-numbers silently
    revert.
  - Esc closes the whole Sheet properties dialog (cancel).
- Section view › Depth wasn't captured.

## asb 10-21 evening: qa5 drawing findings (docs/parity/workflow-drawing.md)
Written; tests added; NOT yet compiled together (the model crate kept failing on others' WIP: units `to_fixed`,
drw2's cad_export.rs). Check first: `cargo test -p noshape-model --lib drawing::tests::`, app check, clippy.
- **D4** assembly title block: `SourceModel.own` (the assembly's Properties) feeds `{part.*}` for assembly views;
  `part_mass` sums parts (or the assembly override). Test `assembly_drawing_fields_are_the_assemblys_own`.
- **D8** callouts on an edge away from snap points: `CalloutStyle.part` (pending.rs `part_near`, pick.rs), used by
  `balloon_text`. Test `callout_on_an_edge_shows_its_parts_bom_item`.
- **D10** PDF: qa5's PDF had a hidden-line iso and a detail border; the detail lines over the circle are probably other
  views under it. Test `exact_export_clips_detail_views_to_their_circle` (exact + fast). Re-check with qa5 if it passes.
- **D11** DXF/DWG/DWT versions (live lists + defaults 2000 / Release 14 / 2013), "Model space only" for DXF R13/R14,
  Sheets ▾ for DWG/DXF only (any sheet count), PDF versions 1.4–1.7 default 1.5. Test `default_versions_are_onshapes`.
- **D7** GTol: Tolerance input focused (deferred) on open; ✓ enabled once anything is entered.
- **D5** Esc (Cancel action) gives the canvas the focus back.
- **D3** hypothesis test `source_hashes_survive_a_store_round_trip` (if it fails: some field serializes differently
  after a store round trip → spurious "Out of date" after undo). Not fixed yet.
- **D1** already done by someone (ANSI_A_INCH preselected); verify the highlight in the app.
- **D13** DRAWN persisted: `Drawing.drawn_by` set at creation (workspace.rs, ops/drawing.rs) and on the first commit
  of older drawings; `{user.name}` / `{drawing.drawn_by}` read it. Test crates/ops/tests/drawing_drawn_by.rs.
- **D2** new `drawing_chooser.rs` (live 170–173 flyout, filters, tree, thumbnails); old modal removed from
  drawing_insert.rs. Not yet done: Other documents tab content, the two document buttons, Import… footer, flat-pattern
  thumbnails (use the part's picture).
- **D12** left to drw2 (they're in cad_export.rs); note left in drawing-verify.md.

## asb 10-21 night (after the freeze): qa5 status
- Integrated in 5d28447: D1–D4, D7, D9, D11 confirmed by qa5. D2 picker re-applied after the freeze (`drawing_chooser.rs`).
- In the tree since (model drawing tests 115 green; app clippy waiting on others' model WIP):
  - **D3 root cause**: serde_json was built without `float_roundtrip` (only sketch's dev-dependency had it), so a
    document reloaded from the store (open, undo) had slightly different floats → new hashes → "Out of date".
    Workspace Cargo.toml now enables it (no Cargo.lock change). Test `source_hashes_survive_a_store_round_trip`.
  - **UNDO2**: `studio_changed` recomputes `out_of_date` over all unpinned sources (an undo back clears it).
  - **D8 (pass 3)**: existing callouts without a ref / part find the part of the view edge at their anchor at render
    time (`Env::part_at`, Env got `views`). Test `old_callout_on_an_edge_finds_its_part`.
  - **D14**: `Sheet.scale_shown` (set by commit when the sheet has views, carried over by `reload`), SCALE shows the
    sheet scale after its views are undone. Test `sheet_scale_stays_after_its_views_are_gone`.
  - **D15**: DXF defaults ☑ z-height to zero, Units "Use drawing units" + live list. Test in drawing_export.rs.
  - **DSM1**: chooser flat-pattern pictures are the flattened plate (`ThumbOf::Flat`: flat_pattern_for → extrude by
    thickness → tessellate).
  - **DSM2**: new drawings get Onshape's template precisions (`templates::template_precisions`: inch 0.123 / 0.1° /
    fit 0.1234, no leading / trailing kept; mm 0.12 / 0.1° / fit 0.123, leading / no trailing); angles keep their
    digits (zero settings are length-only) → "UP 90.0°". Tests `template_precisions_keep_the_angle_decimal`, format.rs.
  - **DSM3**: Show/hide gets "Show faulty parts" (`DrawingView.show_faulty`, no effect yet: no faulty-part concept);
    flat views have no center of mass.
- Still open: D10 (re-check with qa5), D12 (drw2), D2 leftovers (Other documents, document buttons, Import…),
  D6 (verify vs live).
- **App checks (21 Oct late, build after d5745c9):** D2 picker (toolbar Insert view → empty Insert view panel + flyout,
  filters, Part Studio 1 ▾ → Part 1 picture, choosing it opens the Insert view panel with the view on the cursor);
  D8 (Callout clicked on a plain assembly edge → "1" with a dot leader); D5 (Auxiliary view, Esc, d → Dimension);
  D7 (GTol: typed "0.1" lands in Tolerance at once, ✓ green); DSM3 Show/hide order with "Show faulty parts". Fixed
  after: sheet tabs hide under the chooser; an empty filter result shows no "No Part Studios" message.
  `cargo clippy --locked -p noshape --all-targets -- -D warnings` clean; model drawing tests 115 green.

## Moved to the shared drawing tools (api, 2026-10-21 23:40; drawing slice 5)
What the drawing tools commit now lives in `crates/scene/src/drawing_tools/commit.rs`, so the agent API
(`drawing_section_view`, `_detail_view`, `_auxiliary_view`, `_break_view`, `_crop_view`, `_centerline`,
`_virtual_sharp`, `_view_table`) builds exactly what the clicks build. The app calls it:
- `drawing_tools.rs`: `ortho_point`, `extend_cut`, `section_jog_at` (bodies), the Centerline, Edge to edge
  centerline (`silhouette_centerline`, `edge_pair_centerline`, `edge_centerline`), 3/2 point circle centerline
  (`circle_centerline`) and Virtual sharp (`virtual_sharp`, its "The edges are parallel") commits.
- `drawing_extra.rs`: `crop_refused` and the Break refusals (`view_refusal`, same messages), `view_crop`,
  `view_break`, `view_table` (BOM / cut list / hole table + its edit name).
Behaviour unchanged. Fix also landed in `scene/drawing_tools/pick.rs` `hover_at`: Edge to edge centerline and
Virtual sharp now hover view edges (before, they always said "Select a straight edge"). If you change one of
these rules, change it in commit.rs (the app and the API both use it). Backups: /root/api-scratch/drw5/bak_*.rs.

## asb 10-22 (qa5 pass 8 on 5bd50b2): DSM5, D18, D17, DSM2, DSM4, D8
- **D18** drawing BOM = assembly BOM: `views::assembly_source` merges `analysis::bom` rows with skb's rule
  (`assembly::bom::canonical_config`, now `pub(crate)`: values equal to the default, or equal as numbers, are the
  default; excluded inputs don't count); a row's bodies are matched on the canonical configuration.
  Test `drawing_bom_merges_instances_in_the_same_configuration` (two copies at "10 mm" / "0.01 m" of a "10mm"
  default join the original's row).
- **D17** (app, `drawing_view.rs`):
  - (1) the end of *any* regeneration (e.g. the chooser's Part Studios) set `out_of_date = false`, so a stale
    drawing's Update went pale while its views stayed old (reproduced: Part Studio change → yellow → open the
    picker → pale). Now `sources_stale` is recomputed over the views' own sources only.
  - (2) the views keep their geometry after the document is reopened: `Drawing.sources_at` (microversion the views
    were made from) is set by every drawing commit while up to date and by Update (an "Update" step);
    `load_old_state` regenerates the changed sources from `store.read_all(sources_at)` at open, so Update is
    yellow and the old views stay until it. Live "qa5 pass 7" (6).
- **DSM4** picker filters (`drawing_chooser.rs`): 7 independent toggles in live's order (Part Studios, Parts (on),
  Composite parts, Sketches, Surfaces, Curves, Flat patterns; `filter` is a bit set). Studio rows show what the
  toggles list, in body order (flat pattern above its part, then sketches); a studio with nothing listed is
  hidden; Part Studios alone = the studio rows themselves. `views::view_bodies(ctx, Some(part))` takes any
  non-point body (surfaces / curves picked here); the whole studio skips open composites (members twice).
- **DSM5** shaded views (`drawing_shaded.rs`, `drawing_scene.rs`, model `SheetGeometry.view_edges`):
  - a flat view's picture is the flat plate (flat face extruded by the thickness) seen from the flat's reference
    side in its plane coordinates (= the flat view's coordinates), so it lines up with the outline;
  - the picture's white background is transparent (also in the exported PNG);
  - a shaded view with its picture hides its edges: they are a pick-only layer, drawn in ink while the view is
    hovered and in the selection colour while it is selected; bend lines, notes and centermarks stay on top;
    PDF / SVG / raster drop the edges where the picture is embedded (DXF keeps them).
  - "the existing flat view opened shaded": no code shades a new flat view (style defaults to hidden lines
    removed); qa5's view was shaded in the test document (most likely by pass 7's DSM3 menu check).
- **DSM2** UP/DOWN: the notes print `FlatBendLine.up` as is (`sheetmetal/flat.rs:300`,
  `b.is_up() != data.flip_direction_up`). fs's native-route test asserts a flange is *not* up
  (`sheetmetal_frames.rs:271`), against live; fs's std-route rewrite ("flange up = UP",
  /root/fs-scratch/sheetmetal_frames.std-route.rs) lands with the std-route switch. Nothing to change on the
  drawing side (note in featurescript.md "From asb (21 Oct night)").
- **D8** iso cylinder balloon "----": a click on a silhouette lands beside the surface and the line of sight
  missed it (tolerance 1e-7 × size). `body_under(src, frame, p, tol)` takes the pick tolerance in view units
  (`pick.rs part_under`). Test extended (just beside the silhouette: none at 0, found at 0.5 mm).
- **Checked in the app (build of 10-22 ~00:30):** D17 (1) the Update tool stays yellow after the picker regenerates
  other studios; DSM4 the demo shows Part Studios / Parts / Sketches only, Parts + Sketches lists "Part 1" then
  "Sketch 1", Sketches alone hides studios without sketches, Part Studios alone = studio rows; shaded iso view: no
  black edges, transparent background (the through hole shows the sheet), hover = thin ink edges, selection =
  orange edges, the balloon on top; qa5's "qa5 sheet metal" flat view: the plate fill now matches the flat outline,
  bend lines on top. Found there: the bend notes were still hidden by the picture (images are their own render pass,
  drawn after the text meshes, depth-tested "nearer or equal"): the picture now sits at z −0.5 (paper −1, ink 0).
  Re-check pending (build blocked by others' WIP, see below).
- **D2 leftovers** (`drawing_chooser.rs`): an opened other document now has live's header (🏠 "Go to document
  filters", ‹ back with the category as tooltip, name + ⑂ version, Create version… (opens the Create version dialog
  on that document through a temporary `DocModel`; the picker rereads it when the version is made) and Version graph
  (its versions; picking one lists it)), then Part Studios | Assemblies, search, the filter toggles, the blue box with
  the underlined "Create a version in <name>." link, and the Part Studio › parts tree of that version (sources keyed
  by the `ExternalRef`, `chooser_sources` lists them; `choose_external` takes a part). Document rows have the version
  line (latest version or Start, read during the library scan). Import… footer in every state of both tabs.
  Their rows have pictures too (`thumbs` keyed by document: 0 here, `OtherDoc::tag` there).
- **Multi-sheet DXF / DWG** (drw2 l, `cad_export.rs`): several sheets → ONE file. `cad_document_sheets`: every sheet
  in model space left to right, each content box (`content_x_range`: frame + off-sheet content, measured before
  Exclude off-sheet content) starting `SHEET_GAP_MM` (5.019 in) after the previous one (first at x 0); a layout per
  sheet named after it ("Layout1" renamed; repeated names get " (2)") with a viewport (id 2) onto its frame.
  `Sheet.dx` shifts all of a sheet's points. R12 (no layouts): the sheets' R12 geometry side by side
  (`r12_sheets`). `to_dxf_sheets` / `to_dwg_sheets`; the export dialog writes one .dxf / .dwg instead of a zip.
  Test `several_sheets_go_in_one_file_like_live` (viewport targets at w/2, 11+5.019+5+w/2, …; layouts Model,
  Sheet1–3). DWG multi-sheet assumed like DXF (live only checked DXF).
- **qa4 R12 (lead 10-22): drawing Wt widgets** (`drawing_wt.rs`, live behaviours "Drawing Wt widgets"):
  - Drawing properties text heights and arrowheads are Wt dropdowns: "0.1500" four decimals in drawing units with
    the unit as a label beside, only the leading number counts ("3 mm" → 3.0000, "1+1" → 1.0000), no number
    silently reverts, ▾ = the Styles panel's preset heights (live list requested). Gaps (Dimensions text / geometry
    gap / extension, Annotations stack gap) are spinboxes: a plain number or invalid (pale orange
    rgba(221,82,40,0.1), hover "Must be a number", not applied), ↑ / ↓ and the spin arrows ±0.05 clamped at 0,
    applied at once. Other number rows keep the length field (with a unit); their widget is requested (asb 10-22 a).
  - New drawings get live's template sizes (`templates::template_sizes`): inch 0.12 text, 0.18 arrows, gaps 0.09 /
    0.0625 / 0.08, stack gap 0.05, datum 0.12, CoM 0.25 (so Notes › Text height reads 0.1200, not 0.1378); metric
    3.5 / 2.5 / 0.62 / 0.625 / 1.25.
  - Sheet properties › Custom width / height (`XeInput`): on blur / Enter "12" → "12 in", "300 mm" kept, "1+1" →
    "1 in", "abc" reverts (`commit_sheet_len`; test). Select-all on focus and Esc closing the dialog were already so.
- **Build state 10-22 ~01:00:** the app doesn't compile because of others' WIP (`assembly_view.rs:492` and
  `drawing_view.rs:427` use `doc` after moving it into the struct — the new shared studio cache;
  `quantity_field.rs:387` misses `Sizable`); the model test target fails in `assembly/tests.rs:3038`. My files type-check.
  `/root/asb-scratch/bin/test_loop.sh` runs the model drawing tests when it compiles.
- **DSM4 handed to ui2 (lead 10-22).** Already in the tree (uncommitted, `drawing_chooser.rs`): `FILTERS` = 7 toggles
  with the `F_*` bits, `ChooserState.filter: Option<u8>` (None = Parts), studio rows built from the bits (flat pattern
  above its part, then sketches; studios with nothing listed hidden), filters for absent kinds hidden (`present`), and
  `views::view_bodies(ctx, Some(part))` accepting surfaces / curves. Checked in the app (demo + qa5 sheet metal:
  Flat + Parts → "Flat pattern of Part 1", "Part 1"; Flat alone → the flat row only). asb no longer edits it.

## ui2 10-22: took DSM4 (lead)
- ui2 owns DSM4 now (picker filter toggles, `drawing_chooser.rs`, surgical). asb's 7 toggles and the
  "only kinds the document has" rule match live (dom/d2-picker-current: `ng-if="filter.isFilterVisible"`;
  drawing/170 shows 4 icons in a document without composites / surfaces / curves, 1394 all 7). Restyled to
  live's `os-button os-clear`: 30 px, 35 px pitch, on = `os-active` (#eaeaea under a #b6b6b6 top edge).
- **Drawing panel fields after live's widget answer (dom/drawing-wt-widgets.txt, lead 10-22):**
  - dropdowns: every Text height, Arrowhead, Datum size, Inspection "Text"; the ▾ list holds only the current value.
  - spinboxes (`FieldKind::Spin { step_in, step_mm, decimals }`): Text / Geometry gap, Extension past line,
    Stack gap size (mm step 0.05), Center of mass diameter, table padding (3 decimals), Centermarks Mark size / Gap
    size; inch step 0.05, mm 0.1 (live mm: Text gap 0.1, Stack gap 0.05).
  - Inspection: one "Inspection items" group (Item parsing, Font, Text, Border shape, Border size, Border color, Solid
    fill) + "Automatic inspection items"; Border size is a select "2 Characters" (`DrawingProps.inspection_border_chars`,
    0 Tight Fit … 5; the bubble is at least that many characters wide).
  - Views: no Hatch spacing row (live has none). Construction › Centermarks: Mark size, Gap size, Color, ☑ Extended
    lines (`ViewTabProps.centermark_mark_size / _gap_size / _extended`; stored, look not yet applied: request asb 10-22 b).
  - mm template sizes: arrowheads 2.5, gaps 0.625 / 0.625 / 1.25, stack gap 1.25, datum 3.5, CoM 6.35, inspection 2.0
    (inch inspection 0.08 in).
  - These panel inputs never get Focus / Blur events (GPUI's focus path doesn't end at their handle), so "Tab /
    away commits" is done by `track_field_focus` (and `track_sheet_len_focus` for Sheet properties width / height):
    each render compares the focused field with the last one and commits the one left.
  - Not in our panel yet (live has them): the Dashes spinboxes (Hidden / Tangent / Exploded edges, Cosmetic threads,
    Centerlines), line-thickness selects, table row text heights, Detail / Auxiliary arrowheads.
- (api, 2026-10-22 06:30) More moved to `scene/drawing_tools/commit.rs` (same code; the API ops use it too):
  Callout clicks (`free_callout`, `SheetPick::finish_callout`) in drawing_tools.rs; `chamfer_as_dimensions`
  (drawing_tools.rs) and `more_dimension_pick` (drawing_more.rs; its private axis_pick / raw_line_pick / circle_at
  wrappers removed) are now SheetPick methods; Centermark Single + Circular/Linear ✓ (`centermark`,
  `centermark_set`, `CmCircle` / `CentermarkOpts`) in drawing_centermark.rs; Note panel ✓ for a new note
  (`new_note`, incl. the typed height becoming the Notes default) in drawing_ui.rs. Behaviour unchanged; checked in
  the app (callout with/without leader, centermark, max/min dimension; note after the next build).

## asb 10-22 morning (freeze lifted, main aa75fdf)
- **Centermarks** (live asb 10-22 b, behaviours.md "drawing centermarks"): build.rs auto centermarks = a continuous
  cross ±a and, with ☑ Extended lines, four CENTER lines from a + g to r + a + g, where a = Mark size + 0.045 in and
  g = Gap size + 0.045 in (sheet units); ☐ Extended lines = only the cross. `centermark_extension` is no longer used
  (kept for old files). Test `auto_centermarks_follow_mark_gap_and_extended_lines` (defaults 0.045 / 0.09 → r + 0.09;
  Mark 0.2 + Gap 0.1 → 0.245 / 0.39 → r + 0.39; unticked → 2 lines). Checked in the app (demo top view).
- **Multi-sheet DXF / DWG Release 13 / 14:** model space only, no per-sheet layouts (the export dialog says "Model
  space only"; live drw2-k-dxfR14 has no LAYOUT objects but the same side-by-side placement). 2000+ keep a layout per
  sheet. Checked in the app: demo drawing + a second sheet → Export › DXF (Release 14) writes ONE "Drawing 1.dxf"
  (no zip), both sheets, the second sheet's frame 127.483 mm (5.019 in) after the first's content.
- **Panel fields checked in the app (mm demo):** Notes Text height "4 mm" + Tab → 4.0000, "abc" + Tab → reverts,
  ▾ visible; Stack gap "1 mm" + Tab → stays, hover "Must be a number"; 1.25 + Enter, ↑ → 1.3000. The pale orange
  now fills the input itself (it was hidden under the input's white).
- (api, 2026-10-22 15:45) Geometric tolerance click (drawing_gdt.rs), Surface finish ✓ and its "Ra 3.2" start value
  (drawing_extra.rs), Weld symbol target + place (drawing_weld.rs), Revision table placement / settings, latest
  revision and Revision callout (drawing_revision.rs) now call scene/drawing_tools/commit.rs (`gdt`,
  `surface_finish`, `SURFACE_FINISH_DEFAULT`, `weld_target`, `weld`, `RevisionTableOpts`, `revision_table`,
  `revision_table_kind`, `latest_revision`, `revision_callout`) — the same builders the agent API uses. Behaviour
  unchanged; each checked in the app (frames / symbols placed, same history text).
- (api, 2026-10-22 17:30) `view_point`, `ortho_view` and `extend_line_ends` (jogged / aligned section and broken-out
  clicks, drawing_extra.rs) now delegate to scene/drawing_tools/commit.rs (SheetPick::view_point /
  extend_line_ends, ortho_view); the API's drawing_jogged_section_view / drawing_aligned_section_view use the same.
  Checked in the app: Aligned section view about hole A1 (legs extended past the view, SECTION C - C placed).
- (api, 2026-10-22 19:40) drawing_view.rs: load_old_state now calls noshape_ops::drawing::old_state (compares the
  stored states at sources_at and at head: a demo / imported document whose memory copy was never round-tripped
  read as out of date before); reload() remakes the views when the drawing's sources_at changes outside the
  editor (Update or its undo, or the agent API's drawing_update). Table cell ✓ uses scene set_general_cell.
