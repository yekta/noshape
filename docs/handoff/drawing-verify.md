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
