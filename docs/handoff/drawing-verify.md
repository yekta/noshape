# Handoff: verifying the drawing checklist's 🔧 rows (agent "drw2")

Task: take the 🔧 rows of `docs/parity/drawing.md` one by one, exercise them in the app, compare with the live
reference (/root/onshape-research/live/drawing, behaviours.md "Drawing") and the help pages
(docs-raw/md/Drawing__*.md), fix differences in the drawing code, then mark ✅. asb owns the ❌ rows and the
dimension palette (D31–D37); ui2 owns drawing visuals. Claimed rows say "drw2: checking" in their notes.

Tools: `/tmp/drw2/bin/row.py claim|set|show <ID> …` edits checklist rows. Build: target-behave, binary copied to
/tmp/drw2/noshape, display :111.

## Log
- 10-20: started. Batch 1 claimed: CR12–CR15 (custom template), K22, K24, K25, T1, T23.
- K22 ✅ (notes, table cells, views copy / paste verified in the app).
- CR12–CR15 (Custom template tab) rebuilt to live drawing/02: Standard ANSI / ISO / JIS, Language, Size, Units (7),
  Decimal separator, Projection tiles (new icons `projection-third-angle` / `projection-first-angle`), Border,
  Horizontal / Vertical zones, Start zones, Titleblock. Model: `Sheet.format: SheetFormat`, `DrawingTemplate.units /
  decimal_comma / format`. Open: the dropdown lists (Language, sizes per standard, zone counts, defaults) — live
  request "drw2 (10-20 a)". Test `custom_template_border_zones_and_units`.
- V9 / V11: Shift+click projected view of a section keeps the cut (`cut_from_section`, `views::projects_section`);
  projected views inherit the parent's broken-out sections (`views::broken_out_cuts`); Projected refuses Detail
  parents. Test `projected_views_inherit_broken_out_and_section_cuts`.
- K25: triple-click a note opens it with all text selected (`double_click(triple, ..)`).
- T23: right-panel header as live (34 px, 2 px blue underline, no ×); MBD status empty state (Model ▾, Datums,
  Characteristics, info box). `DrawingEditor.mbd_model`.
- TB10: drag inside a table's cells = rubber-band cell selection (tan, help image), the block outlined blue 2 px;
  Shift+click no longer extends (live). Lines / grips still move / resize the table (TB4).
- T1: redrew centerline / centerline-edge / centerline-circle2/3 / virtual-sharp icons: snap-point tools show open
  (white-filled) circles as live 09.
- Open requests: "drw2 (10-20 b)" general table text entry (K24 / TB1 / TB3 depend on it).
- V58: Show/hide edges... dialog (app/drawing_hide_edges.rs; `DrawingView.hidden_edges`, transient `editing_edges`
  makes build.rs draw marked edges thick grey and occluded edges grey). V55/V59: Show/hide submenu in the live order
  (hidden lines, shaded, threads, sketches..., edges..., auto centermarks, centerlines only for views with axes,
  Show/hide parts... only for multi-part views, center of mass). Faulty parts / part intersections / offset cut
  lines still to do (V56).
- CR7: new views get auto centermarks (live Drawing properties › Views default On); each auto centermark is its own
  pick (`PickTarget::AutoCentermark`, `Sel::AutoCentermark`), Delete removes just it (`DrawingView.deleted_centermarks`),
  Show auto centermarks again restores them. Test `auto_centermarks_delete_one_by_one_and_edges_hide`.
- D4: Dimension ▾ 2 point linear / Point to line / Line to line are their own tools (`DimTool::Point2 / PointLine /
  LineLine`, `typed_linear_click`): points only; a point and a line; two parallel lines.
- Menus: drawing context-menu labels use "..." like Onshape (was "…"). Sheets panel: sheet row Rename... |
  Properties... | Duplicate | Copy (+ Paste of a copied sheet at the bottom, Delete, Move up / Move down;
  `SheetClipboard`, `Drawing::paste_sheet`); reference row Switch to reference | Replace... (all sheets,
  `InsertViewState.replace_part`) + pin; view row Properties... | Replace... | Delete. Request "drw2 (10-20 c)".

## After v0.4.1 (20 Oct afternoon)
- ✅ now: K22, T23, C5, V9, V11, V14, V58, CR7, D4, TB10. Verified but kept 🔧 (pending live answers or missing
  pieces): S6, S7, S8, C3, V55, V59, CR12–CR15, EX10.
- Fix: editors opened by a double / triple click (note, table cell) got no keyboard focus — the canvas takes the
  focus back while the click is handled. `DrawingEditor::textarea` now focuses again in `window.defer`; the
  triple-click select-all is deferred after it (K25). Re-verify K24 (cell Tab / Enter) with that.
- Section geometry: `section_geometry(.., frame, section, parent, ..)` — `section` = the frame of the section view
  whose cut it is (sight normal to the cut), so a Shift+click projected view doesn't hatch the removal tool's caps.
- View properties (app/drawing_props.rs `render_view_props`) redone to live 177 / dom drawing-flat-view-properties:
  reference link + "In progress" badge, Scale combo, Rotation angle 0–360 + reverse button (icon
  `rotation-reverse`), editable only without parent / children or with alignment suppressed; child views get
  "Suppress alignment with parent", projected views of sections "Apply parent view section cut"; Sheet is a list only
  with several sheets; BOM / Cut list table reference rows (read-only). Not yet: View render mode, Show sketch
  appearances (no effect in our renderer yet), Synchronize breaks from parent, multi-view properties (V39).
- Reference row menu no longer shows "Pin view reference (no versions)".
- Live answers a–d applied: Custom template lists / resets / zones (✅ CR13–CR15; CR12 waits for title-block
  translations, request e), Sheet properties › Size › Custom (✅ S12; `SheetSize::A4Portrait`, `standard_list`,
  `list_name`), border off = inner frame only (sheet.rs), Sheets panel rows "<name> (<position>)", click selects /
  Ctrl+click / right-click adds / double-click activates, multi-row menu, live menu order, pasted "<name> (1)"
  (✅ S2, S7, C3; `DrawingEditor.sheet_rows`), ordinate menu (✅ D8, `ordinate_entries`).
- Next: in-place note / table-cell editing (A2, K24, TB1, TB3: live "General table text entry" — floating Note
  panel, caret in the cell, Tab / Enter (cell below) / Shift+Tab / Alt+Enter, Enter in the last cell ends, Esc
  discards); then asb's list of built-but-unchecked rows (D31–D38 palette, A57/A58, V68, V63, V23, D42/A64 …).

## PAUSED by the lead (20 Oct 16:55, after the OOM reboot) — exact state
Nothing of mine is running (my last build was killed by the reboot). /tmp/drw2 scripts were recreated (row.py,
start.sh, shot.sh, x.sh); Xvfb :111 is not running.

**Integrated in dc0da0a (14:43):** everything up to the Sheets panel / ordinate / Custom template / Sheet properties
work (rows marked ✅ above).

**In the working tree, written but NOT yet compiled or checked in the app** (finish these first after RESUME):
1. In-place note editing (A2): new `crates/app/src/drawing_inplace.rs` (untracked, `mod drawing_inplace;` in
   main.rs) — `sheet_to_local`, `note_inplace_box`, `render_note_inplace` (Textarea over the note in its font,
   size and line pitch, dashed box, ruler above). Hooks: drawing_view.rs `inplace_hidden` / `inplace_cell` fields,
   `refresh` blanks the edited note / cell text, `render` keeps them in sync and adds the two overlays;
   drawing_ui.rs `render_panel` → `render_note_panel` (floating "Note" ✓ ✕ above the note / cell);
   drawing_note.rs: the Textarea and ruler left `note_panel_rows`; drawing_tools.rs `on_mouse_down`: a click away
   commits an open note / cell.
2. Drawing fonts registered with GPUI: `noshape_sketch::text::font_files()` (sketch/src/text.rs) loaded in
   main.rs `load_fonts` — touches the sketch crate (full downstream rebuild).
3. In-place table cells (K24, TB1, TB3): `cell_inplace_box`, `render_cell_inplace` (blue cell outline, keys Tab /
   Shift+Tab / Enter = cell below / Alt+Enter; capture_key_down on the overlay); drawing_extra.rs `cell_step`
   (step 2 = down; past the last cell ends); drawing_table_tool.rs `close_table_tool(accept, window, cx)` opens
   cell (0,0) after ✓.
4. View properties text 13.33 px (drawing_props.rs), small.
Possible compile risks: Textarea style methods (`font_family`, `line_height`, `italic`, `p_0`, `h`) on
`gpui_kit::component::input::Textarea`; `Input::text_size` in drawing_props.rs; `window.defer` closures.

**Rows claimed and still open:** A2, K24, TB1, TB3 (in-place editing above), S8 (drag reorder), V55/V56 (faulty
parts, part intersections, offset cut lines), V59 (pattern connecting lines), CR12 (title-block language),
EX10 (strikethrough only from DXF/DWG 2013), V39 (multi-view properties), V45 (render mode).

**Next after RESUME:**
- Build; fix my compile errors (if the shared tree is broken elsewhere, use the private worktree
  /root/noshape-drw2 — rsync from /root/noshape and take HEAD versions of the broken foreign files only).
- Check in the app: in-place note (double-click NOTES in --demo-drawing-full, type, click away), Table tool ✓ →
  typing in cell (1,1), Tab / Enter / Shift+Tab / Alt+Enter, Enter in the last cell ends.
- CR12: the live answer is in behaviours.md "Custom template › Language: title-block texts per language" (+
  dom/drawing-lang-titleblock-texts.json with exact strings and per-language text heights; ANSI: no SIGNATURE
  column, MM/DD/YYYY date, untranslated .XX tolerance block, "PROJECTION USA" in French). Needs a language on the
  sheet (`DrawingTemplate` → `Sheet`) and translated labels in sheet.rs `onshape_title_block`.
- asb's "For drw2" list in drawing-behaviour.md: D31–D38 palette (live drawing/15 for the look), A57 / A58, V68,
  V63, V23, D42 / A64, Units and precision Dual / Fit class, Table tool Rows focus, Detail scale, centermark arms,
  leaders, radial dim lines, ISO ordinate.

## HANDED OFF (20 Oct ~17:30, account change) — exact state for the replacement
Scripts now live in /root/drw2-scratch/bin (row.py edits checklist rows: `python3 row.py claim|set|show ID …`;
start.sh <binary> [args] starts Xvfb :111 + the app; shot.sh name [crop]; x.sh <xdotool args>). Shots go to
/root/drw2-scratch/shots. No app is running. A `cargo build -p noshape` (target-behave) may still be running in
the background from my session; it is harmless (shared target dir) — just build again.
**Don't build /root/noshape-drw2** (deleted; TEAM-RULES forbids private worktrees in shared target dirs. NOTE for
the lead: before that rule I built /root/noshape-drw2 into /root/target-behave once, ~15:20, which may explain
rlib-format errors there).

**Integrated already (in HEAD via 1c4ba5d / later):** in-place note / table-cell editing code
(app/drawing_inplace.rs + hooks), drawing fonts registered with GPUI (`noshape_sketch::text::font_files`), View
properties rework, Sheet properties Size › Custom, Sheets panel, ordinate menu, Custom template lists.
These compile (the lead's integration builds them) but A2 / K24 / TB1 / TB3 are **NOT yet checked in the app**.

**In the working tree (mine, CR12 title-block language) — `cargo build -p noshape` succeeded at ~17:35 with no
warnings; tests, clippy and the app check are still to do:**
- new `crates/model/src/drawing/tb_lang.rs`: `TitleLanguage` (Onshape's 9 languages in list order, `name()`,
  `strings()` = every title-block label from live behaviours.md "Custom template › Language: title-block texts per
  language", per-language height factors, `sheet_field()` "{sheet.number} von {sheet.count}" …).
- mod.rs: `pub mod tb_lang;`, `Sheet.title_language` (serde default English).
- templates.rs: `DrawingTemplate.language`; `apply_template` sets the sheet language and the localized "1 of 1"
  Sheet field.
- sheet.rs `onshape_title_block`: all labels from `tr` (English layout unchanged; other languages wrap the long
  notes in their cell with the new `pub fn wrap_text`); units word mapped (MILLIMETERS / INCHES → the language's).
- app/drawing_create.rs: Language dropdown = `TitleLanguage::ALL`, `CustomTemplate::template` sets `t.language`.
- tests.rs: `custom_template_language_translates_the_title_block`.
Not captured yet (fallback English, needs a live request): third-angle projection label, inch word and
FRACTIONAL for languages other than en / fr (only ANSI en + fr were captured).
**Known limit:** no CJK font is bundled, so 繁體中文 / 简体中文 / 日本語 / 한국어 labels draw as the stroke font's
fallback boxes — lead decision (font size; asb raised it earlier).

**Next steps:**
1. `free -g` (≥ 8 GB available), build, fix any compile errors in the CR12 files above, run
   `cargo test -p noshape-model --lib drawing::` and `cargo clippy --locked --workspace --all-targets -- -D warnings`.
2. In the app (`--demo-drawing-full`, Drawing 1 tab at the bottom): double-click the NOTES note → it is edited in
   place (dashed box, ruler above, "Note" ✓ ✕ panel above it); type, click away commits, Esc discards. Table tool
   (Note ▾ › Table) → place → ✓ → caret in cell (1,1): Tab right, Shift+Tab left, Enter = cell below, Alt+Enter new
   line, Enter in the last cell ends (live behaviours.md "General table text entry"). Then mark A2, K24, TB1, TB3.
3. "+" › Create Drawing › Custom template › Language Deutsch, ISO A3 → compare the title block with live
   drawing/831-*-titleblock.png and 820-e; mark CR12.
4. Remaining open rows: S8 (drag reorder of sheet rows), V39 (multi-view properties, help image
   viewproperties-mult-01), V45 (View render mode), V55 / V56 (faulty parts, part intersections, offset cut lines),
   V59 (connecting lines of patterned auto centermarks), EX10 (strikethrough only from DXF/DWG 2013), then asb's
   "For drw2" list in drawing-behaviour.md (D31–D38 palette vs live drawing/15, A57 / A58, V68, V63, V23, D42 / A64,
   …) and the rest of the 🔧 rows.

## drw2 (new account, 20 Oct ~22:00) — progress
- CR12 compiles; `cargo test -p noshape-model --lib drawing::` green (91) before the font change. App check of the
  German title block still to do.
- **Drawing default font = Noto Sans** (Onshape "Noto Sans ( Note )"): model drawing/font.rs `DEFAULT_FAMILY`,
  `char_width` (Noto advances per cap height, cached; stroke-glyph advances only for chars Noto lacks), `text_width`
  now sums those; `text_strokes` centres each stroke glyph in its Noto slot; `text_fill_font(None)` draws Noto Sans
  (was Liberation Sans squeezed to the stroke widths, which made the in-place editor's text wider than the sheet's).
  font.rs is asb's file — tell asb / lead.
- **Note panel rebuilt to live 225 / dom/drawing-note-edit-dialog.txt** (app/drawing_note.rs, 394 px): Font combo
  "Noto Sans ( Note )" 238 px + height 75 px; B I U S (30 px) + colour swatch + 45 px popups Line spacing,
  Horizontal alignment, List, Vertical alignment, Note order; Ø symbols, Insert drawing / sheet reference property,
  Text format / Date time format (enabled with the caret in a `{property}`, they rewrite it); Flag notes ☐ (the list
  item at the caret) + Flag (Drawing) + N Characters. New icons `note-*` (README). `note_field_fmt` removed.
- In-place note: Textarea xsmall (its own 4 px padding offset), no grips while editing (`note_grips`), Esc discards.
- **Cells as notes** (live 743–750): `Panel::Cell.note: Option<Box<CellNote>>` (style + height) for general
  tables; the same Note panel above the cell (no Note order / Flag row, Line spacing and List disabled); on commit
  `cell_note_format` writes the cell format. Model: `CellFormat.italic / underline / strikethrough / valign`
  (`CellVAlign`), `Table::shown_cell_format` (general tables: header bold centred, body left — live), bold / italic
  via real Noto Bold / Italic font spans (was 3× offset stroke text), `set_cell_format` compares with the cell's
  default.
- Live request "drw2 (10-20 f)": Note panel popup items, mm height text, colour palette, B pressed look.
- Fixed clippy needless `.into()` in drawing_props.rs:663 (asb's named-view code) as the lead asked.
- 21 Oct: **verified in the app**: in-place note (text exactly over the sheet text, no grips while editing, typing,
  click away commits, Esc discards and the sheet redraws at once via `discard_inplace`, new note via Note tool);
  Table tool ✓ → Note panel + caret in cell (1,1), Tab / Enter / Shift+Tab / Alt+Enter, Enter in the last cell
  ends, double-click a filled cell → caret at the click ("aZbc"), Esc discards. Rows ✅: K24, TB1.
  Key gotcha: the Textarea binds tab / shift-tab / enter / escape to actions (IndentInline, OutdentInline, Enter,
  Escape) which GPUI dispatches BEFORE key_down listeners — intercept them with `capture_action`.
- Also done: in-place ruler at the sheet scale (`RulerState.sheet_k`, ◁▷ tail), cell editor sized to its text for
  centred / right cells (multi-line Textarea ignores text_align), trailing empty cell line keeps its height
  (tables.rs `split('\n')`), general table cells edited with the table unselected, note rotate label "Ndeg"
  (`render_rotate_label`, whole degrees). Not yet seen in the app: ruler, rotate label (build blocked by others).
- api moved drawing tool / overlay / pick types to crates/scene/src/drawing_tools/ (pick.rs `SheetPick`): edit
  picking there.
- v0.4.3 freeze (21 Oct): ✅ A2, K24, TB1. Reverted (not started again yet): live Note-panel lists from behaviours.md
  "Note panel popups and values" — Onshape's DRAWING / SHEET_REF property lists (need new keys in build.rs
  `field_value`), the 23 date formats (lowercase `m` = month in `field_format`), height "3.5000" in drawing units,
  popup labels Left/Center/Right/Justify, List without None, Note order Bottom-first, symbol list ° Ø ℄ ⌵ ↧ ⌴ □ ⌒ ±
  Ⓤ Ⓘ ✟ ✝ ▷, Text format / Date only right after an inserted property, B pressed bg #cfdbe5 + 1 px #9aa5af.
  Next: those, then TB3 = the 590 px floating "Cell" panel on a double-click of an EMPTY general-table cell
  (text cells edit in place), then CR12 in the app (renderer was crashing on `section-count`, now fixed).
- 21 Oct after GO: live Note panel answers applied (format.rs DRAWING_FIELDS / SHEET_REF_FIELDS / OTHER_FIELDS,
  `is_field`, `is_date_field`, 23 DATE_FORMATS with `m` = month; build.rs `field_value` keys for all of them;
  drawing_note.rs `note_height_text` / `parse_note_height` = "3.5000" in drawing units; popup labels / order;
  B pressed look) — verified in the app except the date / text format popups. New, not yet compiled against a
  green tree (others' files were broken): app/drawing_cell_panel.rs (TB3 / TB8 floating "Cell" panel,
  `DrawingEditor.cell_panel`, `sync_cell_panel`, icons `cell-*`; double-click an EMPTY general cell selects it),
  app/drawing_note_color.rs (A9 "Note color" panel, live 1137 palette; `DrawingEditor.note_color`),
  `CellFormat.font`. Check them first.
- 21 Oct ~09:00: ✅ TB3, TB8, A3 (grips as live 229b / 234 via `note_grip_overlay`, rotate dotted outline, 14 px
  readout; live has no Flip). Note color panel (A9) and Cell panel verified in the app.
- CR12: title block heights now the live DXF multiples of L = 1.6256 mm (ISO) / 1.2192 mm (ANSI): angular 0.9 L,
  DO NOT SCALE 1.3 L × dns_h, sign-off values 0.84 L, material / finish 1.5 L, TITLE values 2.4 L, SIZE / DWG NO /
  REV / SCALE / WEIGHT / SHEET labels 0.8 L, size / dwg no / rev values 3.0 L, scale / weight / sheet values 1.8 L;
  notes wrap at the cell width minus the left margin; √ after the surface finish label; the "noshape" logo text
  removed (live's top row is empty). Ignored test `dump_german_title_block` prints the texts to compare with
  dom/drawing-lang-titleblock-texts.json. To check in the app: German ISO A3 vs live 820-e.

## D12 (lead, 21 Oct): DXF / DWG export like Onshape's (docs/parity/workflow-drawing.md §4.4)
Reference: live samples/drw2-asbu-aux.dxf (AC1032, "Part 1 Drawing 4", 3 sheets as layouts), asbv-layouts.dxf;
qa5-sm-flat.dxf is a Part Studio flat export (other exporter, R14) but lists Onshape's full layer set. Scripts:
/root/drw2-scratch/bin/dxfstat.py FILE (version, layers, entity types per layer), dxfents.py FILE TYPE N.
Live: CIRCLE / ARC / ELLIPSE / LINE on "Visible" (mixed case in 2000+, VISIBLE in R12/R14), HATCH ANSI31 on
VIEW_HATCHES, MTEXT everywhere (styles Onshape-Standard*, Noto fonts; notes `{\pxql;…}`), DIMENSION (dimstyles
Onshape-Standard$N, anonymous *D blocks) and INSERT (*U blocks) on ANNOTATION_TEXT, ACAD_TABLE on TABLES, layers
only the used ones + 0 + Defpoints, colour 7, lineweights (Visible 40, ANNOTATION_TEXT 25, ANNOTATION_LINES /
CENTERMARKS 13, SECTION_CUTTING_LINE 70 + ONS_PHANTOM, others 15), line types DASHED / ONS_PHANTOM / CENTER /
HIDDEN (inch patterns). Several sheets → ONE file with a layout per sheet (we zip one file per sheet: to do).
Done (stage 1–3, model): `drawing/cad_layers.rs` (CadLayer kinds, names, line types, lineweights; `cad_since`,
`cad_runs`, `cad_line_layer` / `cad_text_layer` / `cad_fill_layer` (fills: the touching line's layer));
`SheetGeometry.cad` spans set in build.rs (hatch, detail border, centermarks, bends, breaks, cutting line /
aux / detail circle, annotations by kind `annotation_cad_layers`, tables, hole origin, sketch, images) and kept
by `export::prepare`; cad_export.rs: live layer table / line types / text styles, MTEXT, curves via
`drawing/cad_fit.rs` (`fit_curve`: circle / arc / ellipse within 1e-4 mm); R12 writer: same layers in capitals.
Done (stage 4, 21 Oct, against live drw2-k-dxf2018 / R14 / R12, request k): layer names "Visible" / "Hidden"
in 2000+, capitals (+ DEFPOINTS) in R12–R14; live layer table order (`cad_export::LAYER_ORDER`); Defpoints only
with dimensions; SECTION_JOG_LINES (broken-out profile, SPLINE). `drawing/cad_objects.rs`: `CadObject`s (set in
`SheetGeometry::add_cad_objects` after the annotations, `add_cad_table`, `add_cad_spline`; members kept by
`export::prepare`): DIMENSION (linear / radius / diameter / 3-point angular / ordinate, *D block with the drawn
lines, SOLIDs, `\A1;` MTEXT in Onshape-Standard-Dimension, Defpoints POINTs; dimstyles Onshape-Standard$0/$2/$3/
$4, Onshape-OrdinateStyle; DSTYLE xdata DIMLFAC = 1 / view scale), TOLERANCE for GD&T frames
(Onshape-Standard-GTolerance, fields split by %%v), hole callout texts as INSERT of a *U block, tables as
ACAD_TABLE over a *T block (2004+; INSERT before), images as OLE2FRAME "Paintbrush Picture" (BMP in
\x01Ole10Native). POLYLINE instead of LWPOLYLINE (view edges 70 = 128). Drawing symbols in MTEXT as
`{\fDrawing Symbols Sans|c0;…}` (no more stroked text). Note leaders on ANNOTATION_TEXT. Release 11-12:
`cad_export::to_dxf_r12` / `r12_geometry` (view edges + sketch only, CIRCLE / ARC / LINE / POLYLINE, line types
used + DASHED). Export dialog / ops: `CAD_VERSIONS`, `DEFAULT_DXF` (R14), `DEFAULT_DWG` (2000), `to_dxf_file` /
`to_dwg_file`. Tests: cad_export::tests::*, drawing::tests::cad_export_dimensions_layers_and_r12_like_onshape.
Still open: (1) all sheets in ONE file (model space side by side + a layout with VIEWPORT per sheet; the offset
rule is unknown → request drw2 10-21 l); (2) per-kind text styles (Onshape-Standard-Fields / -NoteStyle with
`{\pxql;…}` / -View-Label / -Hole-Tag / table row styles; one MTEXT per multi-line view label); (3) the
remaining dimension types (chamfer, arc length, ordinate groups) and the _DotMediumOnshape leader-dot INSERTs;
(4) the full live STYLE / DIMSTYLE tables.

## asb → drw2 (10-21, qa5 findings)
- D12 (DXF entities / layer names): I see you're in model/drawing/cad_export.rs, so D12 is yours. I'm taking D11
  (DXF options: Release 14 default, Sheets ▾) in app/drawing_export.rs only. Note: cad_export.rs:252
  `EntityType::MText(e)` needs `Box::new(e)`, so the model crate doesn't compile until then.
