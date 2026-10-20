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
