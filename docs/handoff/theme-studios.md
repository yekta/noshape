# Restyle outside the Part Studio chrome (agent "ui2")

Goal: everything outside the Part Studio chrome looks exactly like Onshape: sketch visuals, assembly,
drawing, start window / documents page, Variable Studio, versions panel, tab manager, generic dialogs and
toasts. The Part Studio chrome and theme.rs belong to agent "ui" (docs/handoff/theme-layout.md).
Build dir /root/target-shared (binary copy /tmp/ui2/noshape), display :105. Side-by-side shots (ours | Onshape) in /tmp/ui2-shots/.

## State (2026-10-19 17:45 UTC) — summary for the lead (third ui2 of the day)

**Finished (all built and checked in the app, 17:14 build green, no warnings; clippy: nothing from my files):**
- Queue 0: empty-state "Learning Center." opens https://github.com/yekta/noshape#readme. Queue 2: clippy
  `sort_by_key` (material_library_view.rs) and `BoundsCell` alias (feature_studio.rs).
- Documents page: advanced search (new `start_search.rs`; ▾ in the search box, 360 px panel: Type
  multiselect, Name, Description, Part number, State, Revision, Add criteria, Results from; content
  criteria read off the UI thread into `DocFacts`); right strip Properties (Name *, Description,
  Category, Library label, ↶↷, Apply; `lib::set_doc_properties`, store meta `library_label`) and
  Document notes (read-only, "No notes available"); Details spacing as live 24; italic search placeholder.
- Assembly: filter row like the Part Studio's (+ ≡ list button), rows / Origin row / header at live
  metrics, list-collapse toggle at the viewport edge (`ui.list_collapsed`), bottom-right Measure /
  Analysis / Mass properties buttons.
- Drawing: Create Drawing dialog as live drawing/01 (drawing_create.rs, visual only); ISO sheet zones
  numbered from the right / lettered from the bottom, A3 6x4, 7 mm labels (model drawing/sheet.rs).
- Versions and history: search ▾ filter panel (Tab, Tab types, Modified by, Date modified, Reset/Search;
  tab changes from store diffs), Onshape's Hide merges / Hide automatic versions toggles (old non-Onshape
  toggles removed), legend + icons as live.
- Tab manager: search toggle, sort menu (live's compact list), filter-row toggle, list / detailed views,
  footer preview toggle + ?, bold selected row.
- Feature Studio: Onshape snippet icons (fs-snip-*.svg), pale Commit at 50 %, editor 13/16 px with the
  first line under the toolbar, Monitor box sized to text.
- Tab menus open upward from the tab's left edge (live menus/rmb-tab-*), ⊗ Delete icon, Part Studio menu
  without Release… / Create Drawing…, Assembly "Create Drawing of …" in Onshape's place.
- Toasts centred on the window (Part Studio, assembly, config, named positions). Sketch region alpha 0.34.
- Variable Studio, Export modal, "+" menu already matched (shots only).
- Shots (ours | live) in /tmp/ui2-shots/final-*.png: documents, documents-advanced-search,
  documents-properties, documents-notes, sketch, assembly, drawing, create-drawing, variable-studio,
  versions, tab-manager, tab-manager-sort, feature-studio, export-dialog, menus-tab, toast.

**Round 2 (19:00–21:45, lead's follow-up; all built green and checked in the app):**
- Live answers applied: tab manager header per dom/panel-tab-manager-header (search toggle has no
  tooltip; "Sort tabs" / "Filter tabs" toggles with the #eaeaea + inset-shadow active look, blue funnel,
  36 px pitch; bare 20 px List view / Detail view pair, inactive at 0.2; 20 px ×; header with #d7d7d7
  top/bottom rules), sort menu 120 px with 22 px rows, list-view rows 30 px with a 2 px bar, footer
  "Toggle preview" (#666, faded when off). History toggles use the same active look. Documents page:
  the selected row shows Send to trash / Labels icons at its right end (live documents/112; share is
  cloud). Variable Studio inserted tables were already as live (editable, ⋯ menu).
- Import modal (import_dialog.rs) as live document/77: blank page + ↓ arrow + pale 36 px tab icons,
  tiles 12 px under the header, checkbox rows 34 px apart. final-import-modal.png.
- Clippy (lead): fixed tab_manager.rs:663 (useless conversion), history_panel.rs:247 (`BoundsCell`
  alias), and session.rs:117 (`contains`, not mine but one line). App crate clippy now only shows
  numbering_prefs.rs:240 and config_visibility.rs:299 (others' in-progress files).
- **viewport.rs (nav's), surgical:** the new touch code called `window.insert_hitbox` in the canvas
  paint closure, which panics ("only during request_layout, or prepaint") — every document window
  crashed at start. The hitbox is now inserted in prepaint and passed to paint.
- New windows: none of my files call `gpui_kit::open_window` (start_window uses os_menu::open_window).
- Shots refreshed: final-tab-manager.png, final-versions.png, final-import-modal.png.

**Left / known differences:**
- Feature Studio gutter 10 px wider than Ace's 42 px: gpui-base hard-codes an 18 px fold hitbox (kit
  patch needed; lead's call).
- Assembly ≡ button inert; tab manager ? button inert; ISO A4/A2/A1/A0 zone counts guessed — all waiting
  for live answers (REQUESTS.md).
- Advanced search "Results from: Versions / All" searches only the latest state; Details "Document labels"
  chevron doesn't collapse.

**Cross-area edits (surgical):** ui's part_studio/mod.rs (toast centring); asb's drawing_create.rs,
model drawing/sheet.rs, drawing_callout.rs (`Selectable as _` import that broke the tree); skb's
assembly_view / assembly_behaviors / assembly_panels / assembly_config / assembly_positions.
No Cargo.lock changes. Nothing half done.

**Requests for other agents (please forward):**
- asb: ISO templates use the ANSI-style title block in Onshape (live drawing/156); ours draws an ISO 7200
  block. Drawing tab menu needs "Change to version…" (give me a fn, I wire the row).
- skb: behaviour for the assembly Analysis tools / Mass properties buttons (`asm-eval-analysis`,
  `asm-eval-mass`).
- ui: list toggle at y 563, live 554.
- Clippy `-D warnings` currently fails in others' code: render/renderer.rs:201, model document.rs:30,
  drawing/mod.rs:471.

## State (2026-10-19 12:40 UTC)

Taken over at 12:35 by a fresh "ui2" (previous one ran out of quota mid pixel pass). Uncommitted ui2 work found in
the tree: Learning Center → noshape README (queue item 0 done), clippy `sort_by_key` (material_library_view.rs)
and `BoundsCell` type alias (feature_studio.rs) (queue item 2 done), documents page pixel tweaks (nav 14.67 px /
20 px glyphs, search centred, 267x68 "Last opened" tiles, 10 px section toggles, workspace chip on rows),
Variable Studio footer `?▴` popover (live 42/45) and checked-checkbox icon. Now: pixel pass (queue item 1).

## Earlier state (06:00 UTC)

Taken over at 04:35 by a fresh "ui2" (previous one ran out of quota). Nothing was half done: the tree built.
The previous agent had already finished (not yet in this doc): "From import" variables (variable_import.rs,
model variables.rs `CsvImport`), Document labels (start_window.rs: nav Labels, Create ▾ › Label…, row menu
Labels…, Details "Document labels", New document "Document labels"; `start_library::Labels`), start-window and
workspace toasts on `os_ui::message_bubble`, Export writers wired (Parasolid, ACIS, Rhino from kern; GLTF, GLB,
3MF, URDF ours; only PVZ / JT still end in `missing_writer`).

This round (all checked in the app, shots in /tmp/ui2-shots/final-*):
- **Toasts everywhere on `os_ui::message_bubble`** (+ `message_bubble_with_link`, `BubbleKind` incl. green
  Success, per-kind × colours from live DOM, ui's toast-*.svg icons + new toast-success.svg): Part Studio
  (ui's part_studio/mod.rs, surgical; "Sketch 2 has been canceled. **Restore**"), assembly config / named
  position toasts, documents page (top 8 px, over the top bar). final-toast-part-studio.png,
  final-toast-success.png, final-documents-trash-toast.png.
- **Documents page Trash** (new start_trash.rs: `<config>/trash/<id>/<file>` + index.json, 30-day purge on
  load): "Send to trash" confirm modal (Show details), info toast "**name** has been sent to the trash… Restore",
  nav Trash, notice + "Empty trash…", row/folder menu Details / Restore / Delete, "Delete forever?" (red
  `os_ui::danger_button`, item box), "**name** has been permanently deleted.", "Nothing in trash.". Delete key
  in the Trash = Delete forever. final-documents-trash.png, -send-to-trash.png, -delete-forever.png.
- **Nested folders** (real directories): Folders section = the place's subfolders (`ScanResult.dirs`),
  267x48 tiles (click selects, double-click opens), breadcrumb Owned by me › A › B, folder lists only its own
  documents, folder menu Open / Rename / Details / Move to… / Send to trash (old "Show in file manager /
  Remove from list" and `files::reveal_in_file_manager` removed). Folder… no longer adds to folders.json
  (only folders outside the documents folder live there) and shows "Folder created." (green).
  `Location` browser now navigates folders (`Location::new`, `exclude`; doc_dialogs uses it).
  final-documents-nested-folders.png.
- Nav "Created by me" (owner == local user). Grid / list buttons moved to the right of the Folders row
  (live documents/69); a 36 px row above the table where Onshape's "+ Add" (add column) goes.
- Assembly mate type select → `os_menu` select list with icons (os_menu.rs select style: text after icon,
  surgical). Dead kit `PopupMenu` field removed from drawing_view.rs.
- Lead review round (08:05): Details panel open by default (`Prefs.panel`, remembered) with the ⓘ strip on
  its left and the blue bar on the strip's right (live 69); folder Details (name, Owner, Created by, Last
  modified by); folders area = Onshape's split pane (39 % of the list area as live dom/documents-folder-row
  357/908, draggable divider, ^ when open / ⌄ when collapsed), "Last opened by me" tiles in Owned by me (live
  45), caption = folder name inside a folder, breadcrumb parents `#666`/400 with blue ›. "me" = any of
  Preferences name / OS full name / login (`my_names`), also in Details and Versions. The top-right name
  is `local_user_name()` (Preferences → OS full name → login): on this server the account is `root` with
  GECOS "root", so it shows "root" unless a name is set in Preferences (test home has "Alex Morgan").
  Fixed a crash: `label_checklist` read the StartView during its own render (Details of a document).
  Shots: final-documents-owned.png, -trash.png, -doc-details.png, -folder-details.png.
- Builds now go to the shared /root/target-shared (lead rule); my binary copy is /tmp/ui2/noshape
  (run.sh uses it). /root/target-ui2 and the /root/noshape-ui2 worktree are deleted.
- **D1 answers applied (live documents/80–100, behaviours.md "Documents page: column chooser…"), 08:45:**
  "+ Add" column panel (194 px under the button's right edge: tri-state all box + "Search...", rows
  checkbox · grip (drag to reorder) · name; Modified, Modified by, Owned by on, Type ("Document"), Description
  off; global, `Prefs.columns`; the table scrolls sideways when wider than the pane). Empty trash… = "Empty
  trash?" + "This will permanently delete all items from your trash; you can't undo this action." + red
  **Empty trash**, no toast; the notice line hides when the trash is empty. Restore toast "**name** has been
  restored." (fades after 3 s, `Toast.brief`) and the document's Modified becomes the restore time (store meta
  `modified`). Empty states: Created by me "Get started by clicking the **Create** button, or follow the
  self-paced courses in the **Learning Center.**" (link opens noshape's README, https://github.com/yekta/noshape#readme), "No documents in <folder>.",
  folders "No folders owned by you." / "No folders in <folder>." / "No folders in Trash."; the table header stays
  above the empty state. Folders row: sort ▾ (✓ Name: A → Z / Name: Z → A / Last modified; grid only), grid
  (tiles, default) / list (Name | Modified | Modified by table, headers sort); they act on folders only
  (`Prefs.view`, `Prefs.folder_sort`), and **the documents table is always a list** (doc grid/cards removed).
  Top-level Folders is collapsed by default; a folder's own section is open (`sub_open`).
  os_menu.rs (ui's, surgical): dropdown text moves right when an item is ✓ checked.
  Shots: final-documents-add-columns, -type-column, -empty-trash, -trash-empty, -restore-toast,
  -folder-sort, -folder-list.png.
- Cargo: `trash = "5"` removed from crates/app/Cargo.toml (Cargo.lock updated by the build).

## Next (when resumed): full-screen pixel pass of every non-Part-Studio screen against live (documents
page incl. Created by me empty state not yet seen in the app, Variable Studio, versions, tab manager,
Feature Studio, import/export, assembly, drawing, sketch), at 1919x1079 like the live shots.

Earlier state (16:50 UTC):

Taken over at 13:25 by a fresh "ui2" (previous one ran out of quota; nothing was half done except the
sheets-panel patch, which had been applied).
- Start window / documents page: done (first pass).
- Sketch visuals: done (first pass). Assembly: list, toolbar, mate dialog done; mate glyphs, triad, Insert
  dialog (asb is editing it) and the view cube still to do.
- Drawing: sheet, frame, text and Sheets panel done (see below). Toolbar contents are asb's.
- Variable Studio, Versions and history (+ Create version / Create workspace dialogs) and Tab manager: done.
- Toasts: `os_ui::message_bubble` (Onshape's `.osx-message-bubble`), used by the assembly. Assembly now uses
  ui's shared view cube (+ its own camera menu) instead of the six view buttons.
- os_menu: **ui2 took the assembly and drawing right-click menus** — both switched (drawing
  `fill_os_menu`; assembly `assembly_behaviors::fill_menu` now builds an `OsMenu`, rows use
  `os_context_menu`, graphics-area menus `OsMenu::build` in `ui.ctx_menu`). Kit dropdowns stay.
- Assembly mate glyphs and triad restyled (see Assembly below). Insert dialog not touched (asb).
- Verified in the app (16:50 UTC build): drawing, Variable Studio, versions, tab manager, assembly cube /
  triad / mate glyph / menus, documents times, sketch dimensions. Clippy: nothing from my files.
- Next (not started): drawing toolbar/annotation polish against drawing/10-15 (dimension hover grips are
  asb's), the Insert dialog (asb is editing it), Part Studio toast → `os_ui::message_bubble` (ui's file).

## Done

- `os_ui.rs` (new, mine): Onshape modal dialogs (`modal`, `modal_body`, `modal_footer`, `modal_ok`,
  `modal_cancel`, `field_label`), buttons (`primary_button`, `cancel_button` gray `#d7d7d7`,
  `secondary_button`), `info_box`, `menu_item` (30 px rows with an 18 px glyph), `row_states`, and the
  single-color documents-page glyphs (`GLYPH_*`, drawn on the 20-unit grid, our own artwork).
- `main.rs`: `mod os_ui;` and `t.overlay` = 50% black (Onshape/Bootstrap modal backdrop).
- `start_window.rs`: Onshape documents page. 40 px `#eaeaea` top bar with a centered 390 px search box
  ("Search in <place>") and square search button; 220 px left nav with the blue **Create ▾** (150x34;
  Document…, Folder…, Open file…) and nav rows "Owned by me" / "Recently opened" (selected: blue
  semibold with the 4 px blue bar); breadcrumb header; collapsible "Folders" section with `#f5f5f5` folder
  tiles and the round collapse handle on the `#d0dbe5` divider; list/grid toggles; table header
  Name | Modified | Location | Size (14 px semibold, 2 px `#d7d7d7` column dividers, `#aaa` bottom rule);
  42 px rows (hover `#def1fb`, selected `#b2ddf6`); empty state = gray outline illustration + "No
  documents owned by you."; 28 px footer. No ⋮ buttons (right-click menu, like Onshape).
- New document dialog = Onshape's: "New document", Document name (prefilled "Untitled document"),
  Document location browser (home / back / "Owned by me", folder rows or "No additional folders"),
  **Create document** + gray **Cancel**. Rename uses the same modal style.
- `start_library.rs`: "Untitled document", "Untitled document (1)"…; thumbnails' white background becomes
  transparent (they sit on hovered/selected rows like Onshape's).

Shots: /tmp/ui2-shots/documents-page.png, /tmp/ui2-shots/new-document-dialog.png.

## Final side-by-sides (ours | Onshape) for the lead
- /tmp/ui2-shots/final-drawing.png, final-drawing-sheets-panel.png
- /tmp/ui2-shots/final-variable-studio.png
- /tmp/ui2-shots/final-versions.png, final-version-dialogs.png (ours above Onshape)
- /tmp/ui2-shots/final-tab-manager.png (with real thumbnails)
- /tmp/ui2-shots/final-workspace-units.png, final-document-menu.png
- /tmp/ui2-shots/final-drawing-dimension-hover.png (ours above Onshape), final-drawing-toolbar-dropdown.png
- /tmp/ui2-shots/final-assembly-triad-glyph.png (triad top, mate glyph bottom), final-assembly-menu.png
- /tmp/ui2-shots/final-documents.png (Recently opened, Onshape time stamps), final-documents-menu.png,
  final-documents-details.png
- /tmp/ui2-shots/final-sketch.png (16 px dimension values, region fill)
- /tmp/ui2-shots/final-documents-columns.png, final-documents-create-menu.png (QA round 3)
- /tmp/ui2-shots/final-export-dialog.png (vs Onshape's Export Drawing modal layout)
- /tmp/ui2-shots/final-feature-studio-search.png, final-feature-studio-commit.png (vs help images)

### Sketch (sketch_mode.rs, part_studio/sketch.rs, scene_builder.rs; surgical)
- Dimension values: 16 px black text (live: 12 px digits, black cores; second pass) with no box (hover `#d9bfa0`, selected `#d9a566` rounded boxes, driven
  `#666`), lengths without the document unit (Onshape shows "4", "Ø1"); dimension lines are broken around the
  value (`clip_outside_box`); arrowheads are filled narrow triangles; selected dimension lines 2 px.
  `overlay()` now takes `&UnitPrefs` (one caller in sketch.rs).
- Dimension edit box = Onshape's: drag strip "◻◻◻", ">" expander, 100 px underlined value, ±, green ✓ and
  bordered red ✗ (✓ commits, ✗ = Esc).
- Constraint glyph boxes: 20 px white, no outline (hover `#fcda7f`, selected `#fab601`, external blue tints,
  conflicts red).
- Closed regions of the edited sketch are filled `#9aa5af` at 26% (theme `SKETCH_REGION`).
- The light-blue `#b2ddf6` frame around the edited sketch with its name ("Sketch 1") at the top-left
  (`SketchEditor::sketch_box`).
- Sketch options panel = Onshape's checkbox rows + the 22 px diagnostics button.
- Shot: /tmp/ui2-shots/sketch-dimensions-constraints.png.

### Assembly (assembly_view.rs, assembly_mate_dialog.rs)
- Instance list in Onshape's order: filter row, "Instances (N)" + new-folder button, one tree ending with the
  collapsible "Mate features (N)" folder (collapsed state = `MATE_FEATURES_COLLAPSED` in `expanded`). Left
  panel uses `ui::LEFT_PANEL_W`.
- Toolbar: "Insert" text button; fastened / linear pattern / relations are split tools (`ui::tool_group`).
- Mate dialog in the feature-dialog frame: 28 px header (title red while invalid, green ✓, red ✗),
  underlined mate-type dropdown, Onshape selection box(es) for the connectors, Onshape checkboxes, underlined
  offset/limit fields, bottom row flip / reorient / animate / Solve / ?.
- View cube: ui's shared `part_studio::view_cube::view_cube` (hover/origin in `cube_hover`/`cube_origin`),
  with an assembly camera menu (Isometric/Dimetric/Trimetric, Zoom to fit, Perspective view, Show mates)
  replacing the six view buttons.
- Mate glyph (assembly_scene `with_highlights`): Onshape's flat square in the connector plane (half size
  `triad_size * 0.17`): white piece + `#90cef1` L (gray when inactive), dark outline (red on error, amber
  when hovered/selected). Filled with dense ON_TOP strokes (meshes can't draw over parts), pickable. `Highlights::mate_glyphs` now carries the frame.
- Triad: hollow strokes (`#f7f7f8` core, `#969da3` edge), shafts with outlined triangle heads, small
  rotation rings + ticks at the tips (Ring handles), parallelogram plane handles near the origin (faint
  pickable fill), ring center; hover `#3e94ff`. Same `ManipHandle` picks as before.
- Shots: /tmp/ui2-shots/mate-dialog.png, /tmp/ui2-shots/assembly-window.png.

### Drawing (drawing_view.rs, drawing_scene.rs, drawing_extra.rs, drawing_ui.rs; model drawing/sheet.rs, font.rs)
- Paper is `unlit` (pure white on the `#dcdcdc` surround), ink pure black, on-screen line widths
  `width_mm * zoom * 0.45` (hairlines at the fitted zoom like Onshape, thicker when zoomed in).
- Sheet frame (`SheetGeometry::add_border_and_title_full`): thin line on the sheet edge, frame inset 10 mm,
  zone dividers across the band and labels centered in it on all four sides. ANSI: zones numbered from the
  right and lettered from the bottom, A = 2x2, B = 4x2, C = 4x4, D/E = 8x4, 5 mm labels; ISO 50 mm zones,
  3.5 mm labels.
- Sheet text is drawn as filled Liberation Sans glyphs (`font::text_fill`, earcut; narrowed to the stroke
  font's `text_width` so layouts still fit), pickable meshes (`DrawingEditor::texts`). Texts with symbols
  outside Latin/Greek (GD&T) keep the stroke font (`font::fill_supported`). Exports still use strokes.
- Sheets panel = Onshape's: closed by default; "Sheet1" chips (`#c7c7c7` active) at the canvas top-left and
  the 33x34 toggle tab at the left edge (`sheets_open`). Open: 258 px `#fafafa` overlay, blue `#1b5faa`
  18 px "Sheets (n)" + white Insert sheet button, 2 px blue rule, "Sort by sheet | Sort by reference" tabs
  (`sheets_by_ref`), tree sheet › reference (part / tab name) › views in `#666`; active sheet `#e9eef2`
  with a 4 px blue bar and a `#90cef1` bottom line. New icons `sheet-page.svg`, `sheet-insert.svg`.
- The always-on status text (sheet · size · scale · cursor x/y) is gone; tool prompts / computing / out of
  date / dangling still show bottom-left when present.
- Hovered annotation = Onshape's look (drawing/14): tan `#ccb292` lines and label box, `#90cef1`
  attachment dots, `#cc925c` end dots and flip diamonds, arrowheads covered (`push_annotation_hover`;
  grips from `SheetGeometry::grips`, emitted for linear dimensions so far). Selected look not captured.
- Toolbar dropdowns open `os_menu` lists in the drawing-toolbar style (`OsMenu::toolbar_list()`, 26 px
  rows, 18 px tool icons via `MenuEntry::with_icon`).
- Sketch 14.38 (skb): used geometry is drawn in the constrained color (black), not purple;
  `SKETCH_EXTERNAL` removed.

- Sheets panel nests derived views under their parent (20 px per level), like Onshape.

### Variable Studio (variable_studio.rs, variable_insert.rs; model variables.rs) — Onshape's model
- **Model (crates/model/src/variables.rs, mine for this):** `VariableStudio { variables, insert_into_all
  (default true, serde default true so old documents keep every studio visible everywhere), inserted:
  Vec<StudioRef> }`; `StudioRef { studio, variables: Option<Vec<String>> }` (whole table or single
  variables). Part Studios and Assemblies have `variable_studios: VariableStudioRefs { inserted, removed }`.
  `element_studios(doc, element)` = auto-inserted (checked studios, doc order, not removed there) + the
  element's explicit references; `element_variables(doc, element)` = their variables, later tables win;
  `studio_variables` / `studio_evaluation` evaluate a studio with the tables inserted into it in scope
  (cycles cut). `insert_studio`, `remove_studio`, `insert_into_studio`. Tests: `studio_scope_tests`.
  `document_variables` is gone: every regen call site (Part Studio regen, preview, sketch, derived,
  thumbnails, evaluated-value hints) uses `element_variables(doc, tab)`, so a `#name` from a studio that
  isn't inserted doesn't resolve (the expression evaluator's unknown-variable error; exact Onshape error
  text requested in REQUESTS.md).
- **Variable Studio tab:** Onshape's table (undo/redo toolbar; centered 561 px; `#eaeaea` title and header
  rows; 140 px columns, 34 px rows, `#aaa` rules; borderless inputs; "Length ▾" types; italic "Name" add
  row → Length `0 <doc unit>`). Plain values show as typed; expressions show their result until clicked
  (`is_expression`, `expression_unit`). Row right-click: Delete, Move up, Move down. "…": Copy table,
  Rename (inline title edit). Inserted studios show as read-only tables above (title, "…": Switch to <studio>,
  Move up/down, Remove reference). "Insert Variable Studio" opens the dialog. Footer (live DOM): 40 px
  `#eee` bar, "☑ Insert into all Part Studios and Assemblies" (toggles `insert_into_all`) + `?▾` (explains
  checked/unchecked; exact menu requested).
- **Insert Variable Studio dialog (variable_insert.rs):** modal, tabs Current document | Other documents;
  each studio row inserts/removes the whole table, its caret lists `#variables` to insert singly; checks
  show the real insertion state of the target (a studio, or a Part Studio / Assembly).
  **Other documents is not implemented yet** (needs cross-document, versioned references) — next.
- **Part Studio Variable table panel (config_panel.rs, ui's file; surgical):** after the Part Studio's own
  table, one editable table per inserted studio (`VariableStudioView::new_in_panel`, kept in
  `ui.var_tables`), "…": Copy table, Switch to, Move up/down, Remove reference. Footer split button
  "Create Variable Studio | ▴" (menu: Create / Insert Variable Studio) = `variable_insert::panel_footer`;
  Create adds a studio tab right after the element. **Assembly Variable table panel: next.**

### Versions and history (history_panel.rs, rewritten)
- 380 px panel (`PANEL_W`, workspace uses it): header "Versions and history" + create version / compare /
  close (dispatches `ToggleVersionGraph`), "Search history" box + search button, graph / show-all-changes
  toggles, Name | Modified header (`#d0dbe5` rule), rows 50 px: workspaces (hollow node, name in a bordered
  chip `#d0dbe5`/`#fff4d6`), versions (filled node), Start (bar); graph painted on a canvas (3 px lanes,
  branch lanes amber curving into the parent at the version they came from). "Show changes…" lists the
  microversions (26 px rows, small dots). Click opens (read-only view), double-click another workspace
  switches; right-click: version → Open, Branch to create workspace…, Compare, Restore to <ws>…; workspace →
  Open, Merge into <current>…. Legend at the bottom. Times "1:08 PM Today" (UTC).
- `open_create_version` = Onshape's "Create version from **Main**" modal (Name V<n>, Description, Note;
  Create / Cancel); the rail's Create version button opens it directly. Branch = "Create workspace from
  **V1**" (Name B<n>, ☑ Open new workspace). Both guard against Enter creating twice.
- New icons compare.svg, auto-version.svg.

### Toasts and menus
- `os_ui::message_bubble(id, kind, msg, on_close, cx)`: 14 px text, radius 5.25, padding 4/37/4/10,
  18 px icon (msg-info/-warning/-error.svg; info = `#769fcc` disc with a white i), bold 21 px `#467bb6` ×.
  Used by the assembly (top center, 9 px down). The Part Studio toast is ui's (`ui::alert`); it could use
  the same helper.
- Drawing right-click menus (canvas, sheet rows) open `os_menu` menus (`drawing_view::fill_os_menu`);
  toolbar/panel dropdowns still use the kit `fill_menu` (they are dropdowns, not context menus).

### Documents page (second pass)
- Row and folder right-click menus are `os_menu` menus in Onshape's icon-less list style
  (`OsMenu::list_no_icons()`: 13 px text, 17 px inset; added to ui's os_menu.rs). Row menu = Onshape's
  minus cloud items: Open, Rename, Copy workspace… | Details, Versions and history | Move to… | Remove from
  recently opened, Send to trash. The start window root now draws `os_menu::render_host`.
- Right strip (Details, Versions and history) and 340 px panels: Details (name, thumbnail, Owner "me",
  Document description, Created by, Last modified by; empty state "Select a document or folder…"),
  Versions and history (workspaces/versions/Start, author + time) from `start_library::doc_details`.
- "Move to…": Onshape modal with the folder browser (`Location`), `start_library::move_document`.
- **Still to do here:** Document labels (Details section + nav "Labels"), the toasts (kit `Notification`
  → `os_ui::message_bubble`), user menu name/avatar done.

### Documents page (QA round 3, 18 Oct evening)
- Create ▾ is Onshape's: Document…, Folder… | Import files…, Label…. "Import from ▸" lists cloud drives
  (Dropbox, Google Drive…), so it's left out as a cloud-only feature. "Open file…" is gone: **Import files…** opens a
  `.noshape` file, and turns a STEP/IGES/BREP/STL/OBJ file into a new document (named after the file, in the
  folder being shown) with one Part Studio holding the import feature. Ctrl+O = Import files….
- Folder… now creates a named folder in the current place (Onshape) instead of picking an existing
  directory; it's added to the folder tiles.
- List columns: Name (350) | Modified | Modified by | Owned by (200 each), as live documents-list DOM; the
  user is written "me". Authors come from the store: new `Document::owner_and_last_author()` /
  `noshape_store::owner_and_last_author(conn)` (first and last microversion author).
- Verified in the app: final-documents-columns.png, final-documents-create-menu.png; Folder… and Import
  files… (screw.step → new "screw" document with Part Studio "screw") tested.

### Import / Export (workspace.rs, os_ui.rs, assembly_behaviors.rs, part_studio/menus.rs)
- "+" › Import… creates a new Part Studio tab named after the file and opens it (Onshape), instead of
  importing into the active Part Studio. Helpers `workspace::{read_foreign, import_names,
  add_import_feature, IMPORT_EXTS}` are shared with the documents page.
- `os_ui::export_modal`: Onshape's Export modal (600 px, File name, Format select, Export / Cancel). It's used by the tab
  menu Export… (Part Studio and Assembly tabs; Assembly via `AssemblyView::open_export_all`), the
  assembly instance Export… and the Part Studio part Export… (ui's menus.rs, surgical). The file name is
  carried into the save dialog. Drawing tab › Export… calls the PDF export until asb's "Export Drawing" modal
  lands (asb is building it; told in drawing-behaviour.md). Format-specific option rows (STEP version, STL
  resolution…) wait for the capture requested in live/REQUESTS.md line 68. Verified: final-export-dialog.png.
- os_menu (ui's file, surgical): menus now paint at the kit's popup priority (100, was 2) so a select list
  opened inside a dialog draws above it; `OsMenuItem::keys(&[..])` adds shortcut chips. Bounds probes
  (`canvas` overlays) need `.top_0().left_0()`: without insets they start after the parent's padding.

### Feature Studio (feature_studio.rs rewritten view; fs_search.rs new; model custom/mod.rs)
- Onshape's model: edits are **saved as you type** (`FeatureStudio.draft`, serde-default, skipped when None)
  but Part Studios use the **committed** `source` until **Commit** (ctrl+s) / **Commit all** (shift+ctrl+s;
  one `edit_tab` per studio so Part Studios regenerate). `FeatureStudio::{new, text, save, commit,
  has_uncommitted}`; struct literals in demo.rs, assembly/standard.rs and tests/custom_features.rs now use
  `FeatureStudio::new`.
- Toolbar: undo/redo | blue Commit split button (▾: "Commit ctrl s", "Commit all (n) shift ctrl s" with key
  chips; help image fs-commit-01) | Search tools… (Feature Studio commands). Header row: Module outline ▾
  (popover with "Search symbols…", "n symbols", function rows → jump), ← → (jump history), search button.
  Notices (compile problems) list under the code. The old "FEATURES" side panel and help line are gone.
- Code editor: Onshape colors (keywords #0000ff, numbers #0000cd), #f0f0f0 gutter, #ededed current line
  (set on the kit highlight theme in main.rs `apply_theme`; only Feature Studio uses the code editor).
- Right-click in the code: Copy, Paste, Cut, Find usages in workspace, Rename in workspace (kit menu off).
- **Search Feature Studios pane** (ctrl+shift+f or the search button; workspace panel under the tab bar, as
  help image fs-search-pane-01): Toggle Replace ›/⌄, mode ▾ (Search for text / symbol / standard library
  import), find field (symbol mode: "Search for symbol "x" at Feature Studio 1 - Line 19, Column 13"), Match
  case, Regex, Feature Studio name filter (`*` wildcard), Find, ×; Replace with + Replace all / Rename;
  expand/collapse all; "All n occurrences of 'x' found in all 1 tabs" (Onshape's wording); results by tab,
  matches bold, click jumps; matches are marked yellow in the code. New Cargo dependency `regex = "1"`
  (already in Cargo.lock).
- Verified in the app (final-feature-studio-search.png, final-feature-studio-commit.png). Not captured
  live yet (REQUESTS.md line 68): exact toolbar/gutter metrics, the notices UI, the Search tools contents.

### Tab folders, tab order, Material Library, drawing thumbnails (18 Oct, late)
- **New tabs go right after the active tab** and into its folder (behaviours.md tab bar): "+" creations,
  Import…, Paste; Duplicate lands after its original (`workspace::place_after`).
- **Tab folders** (help: Document tabs › Organizing tabs with folders). Model (document.rs, additive, serde
  default): `DocumentSettings.folders` / `.parents`, `Document::{add_folder, folder, is_folder, parent_of,
  ancestors, set_parent (no cycles, ≤ 10 levels), children, tabs_in, unpack_folder, delete_folder_tree,
  move_in_order, next_folder_name, rename_folder}`, test `folder_tests`. "+" › Create folder (name field
  active). Tab bar: folder tabs; click opens (bar shows only its tabs, "All tabs" home cell first, "…" lists
  parent folders when deeper); drag tabs to reorder, drop on a folder to file, on Home for the root; folder
  menu Rename, Move to parent folder, Unpack folder, Move to document… (moves the folder with its contents;
  `doc_dialogs::move_to_document` now takes several tabs). Activating a tab shows its folder.
- **Tab manager**: with no search/filter, folders show as a tree (chevron, indent 20); folder menu Rename
  (inline), Add selected to a folder, New folder, Unpack folder; Ctrl+click multi-select; tab rows: Add
  selection to folder | Move to document…, Delete. Drawings now get real thumbnails (sheet rasterized with
  tiny-skia off the UI thread; new dependency `tiny-skia = "0.11"`, already in Cargo.lock via resvg).
- **Material Library** ("+" › Create Material Library): `TabContent::MaterialLibrary` (model
  `material_library.rs`: CSV in Onshape's std.mat export format, `from_csv` / `to_csv` / `sample`,
  `libraries(doc)`, test). View `material_library_view.rs` (help image custom-mat-library-1: info box,
  Download sample, table with Filter by name, # | Category | Name ▴▾ | Density + unit select). Tab menu
  Export… (CSV) and Update… (pick CSV → contents modal → OK). Icon `material-library.svg` (ours).
- Verified in the app: Create folder, drag a tab onto a folder, open folder (home + folder + its tabs),
  Tab manager tree + folder menu, drawing thumbnails, Material Library create / Update / unit select.
  Shots: final-tab-bar-folder.png, final-tab-manager-folders.png, final-tab-manager-drawing.png,
  final-material-library.png. Model tests for folders / material library are written but not yet run
  (asb's in-progress drawing/sheet_sketch.rs test code doesn't compile at the moment).
- Gotcha: the in-app file prompt (NOSHAPE_NO_NATIVE_DIALOGS) calls back inside its window's update, so
  `window_handle.update` there fails silently: wrap it in `cx.defer` (done for Material Library Update,
  "+" Import tab activation, documents-page import errors).

### Feature Studio, second pass (19 Oct, live document/70–75 now captured)
- One toolbar row as live: undo/redo | New feature | Feature parameters, Query, Evaluate, Math, Sketch,
  Operation functions (letter + ≡ + ▾, 50 px; items from dom/feature-studio-toolbar-menus.txt; each
  inserts FeatureScript) | Import (the Import panel, fs_import.rs; see "Round 3"), Format Feature Studio (re-indent) | Commit ▾ (pale while clean) | Search tools… | right:
  Monitor <studio> ▾ (Monitor / Profile per Part Studio and Variable Studio; Profile disabled), Module
  outline ▾ (shows the current symbol; "0 symbols / No top-level symbols found"), ←▾ →▾, "Line n , Column m".
  Strings #036a07, comments Ace green. Co-complete (Onshape Labs AI) left out. The search pane stays on
  ctrl+shift+f. Language gap: see "For the lead".
- Tab menu per kind as live: Feature Studio / Variable Studio / Material Library have no "Select as
  document thumbnail".
- Snippet menus use the new `OsMenu::snippet_list()` (33 px rows, 14 px #333 text 14 px in; os_menu.rs).
- Verified in the app: final-feature-studio.png (vs document/70), final-feature-studio-snippets.png
  (vs 71), final-import-modal.png (vs 77); Import adds "screw 1" after the active tab and opens it.

### Feature Studio, FeatureScript content (lead decision 19 Oct: agent "fs" writes the interpreter)
- New studios start with Onshape's template (`feature_studio::FS_TEMPLATE`: `FeatureScript 3095;` +
  `import(path : "onshape/std/common.fs", version : "3095.0");`, caret on line 4). New feature inserts the
  captured `annotation {…} export const myFeature = defineFeature(function(context is Context, id is Id,
  definition is map) precondition {…} {…});`. All six snippet menus insert FeatureScript (std-library calls
  with example arguments; exact texts not captured yet: live/REQUESTS.md); none are disabled.
- Highlighting: FeatureScript keywords #0000ff, numbers #0000cd, strings #036a07, `path`/`version` of
  `import(…)` #318495 (Ace's variable.language), comments Ace green, identifiers plain. Outline: `export const
  x = defineFeature(function(a is A, …))` → "x(a, …)", `function f(…)`, `predicate p(…)`.
- **Where fs plugs in** (crates/app/src/feature_studio.rs):
  - `pub fn studio_notices(tab, src) -> Vec<Notice>` is the single compile hook (it maps the FeatureScript compiler's; formerly Rhai's
    `noshape_model::custom::compile` diagnostics). Replace its body with the FeatureScript parser/checker:
    `Notice { line, column (1-based), message, error }`. It feeds the editor squiggles + hover message
    (`FeatureStudioView::recompile`, on every save) and the FeatureScript notices panel / navbar `{!}`
    (`fs_notices::document_notices`, on the committed source).
  - Part Studios use the committed source through `noshape_model::custom` (provider.rs `compile`,
    `DocumentScripts::from_document`); swap the FeatureScript evaluator in there.
  - The Monitor ▾ state is `fs_notices::monitored` / `set_monitored`; the "Result: …" line comes from
    `PartStudioView::regen_result()` (Part Studio console output when fs adds `println`).
- **FeatureScript notices panel** (fs_notices.rs new): navbar `{ }` / red `{!}` (workspace title bar),
  bottom panel as live document/75: title, filter, ×; Feature Studio sections (boxed red ✕, message,
  "line:col  Feature Studio n", click jumps); Part / Variable Studio rows (gray) with Monitor ▾; the
  monitored one shows "Result: Regeneration complete" (monospace #aaa). The Feature Studio's own notices
  strip is gone (Onshape has none).
- The gutter's red ✕ marker isn't drawn (the kit editor has no gutter markers); squiggles + hover are.

### Export (lead decision: full Onshape list; part_studio/export.rs is ui's file, refactored by ui2)
- `open_export_modal(base, assembly, run, …)` and `write_export(settings, path, parts)` are now module
  functions shared by the Part Studio (ui's `open_part_export`) and the Assembly (`AssemblyView::
  open_asm_export`, tab and instance Export…). Assembly list: no RHINO, URDF before STL; URDF rows Geometry
  format (STL/GLTF/GLB/OBJ), STL Format, Resolution, Options, ☑ Include hidden instances (Onshape's URDF
  default). URDF writes `<name>.urdf` (one link per part, fixed joints, colours) + `meshes/` in a zip.
  "Export unique parts as individual files" now zips the files (help: "zipped together").
- Formats without a writer yet (PARASOLID, ACIS, PVZ, JT, RHINO; kern) go through the normal flow and
  end in the error message (`missing_writer`); wire each in `write_parts` as kern lands it. STEP already
  uses kern's `write_step_with` (schema, units). The old `part_studio::ExportFormat` (with BREP) and
  `os_ui::export_modal` are removed.

### Import modal (import_dialog.rs new; files::request_open_paths)
- "+" › Import… opens the picker for several files, then "Import to Onshape" (live document/77): tiles
  Import to this document / Combine to a single Part Studio, ☑ Import appearances, ☐ Y Axis Up, ☐ composite,
  ☐ Join adjacent surfaces (? tooltips with Onshape's texts). Each file becomes a Part Studio after the
  active tab (combined: one Part Studio with one Import feature per file). The options are the Import
  feature's parameters; "Join adjacent surfaces" is a new hidden `joinAdjacentSurfaces` parameter, sewn in
  regen (model features/importing.rs + regen/features/importing.rs, surgical).
- Not yet: Import appearances (STEP colours are lost: imports are stored as BREP text) and the Assembly
  that "Import to this document" makes for multi-part files — both need regen work (told psb).

### Document ☰ menu and workspace dialogs (workspace.rs, doc_dialogs.rs new)
- ☰ = Onshape's Bootstrap dropdown (`OsMenu::dropdown()`; icon-less → text 20 px in): Rename document,
  Move to…, Document details…, Restore deleted workspaces… | Copy workspace…, Update workspace…,
  Workspace units…, Workspace properties…, Switch to view only mode / Switch to edit mode | Close document.
  Each is implemented (doc_dialogs.rs): rename (store name), move (close, move file, reopen), details
  (name + description in the store, owner, created / modified), restore deleted branches
  (`Document::restore_branch`, new in the store), copy (new document, opened;
  `start_library::copy_document_named`), update (external Derived / Variable Studio references with newer
  versions), units (panel below), workspace properties (branch name + description in store meta
  `workspace_description:<branch>`), view only (`Session::view_only`; `read_only()` covers it).
- Our old ☰ items moved to Onshape's places: logo click → documents page; Export… → Part Studio tab menu
  (Export dialog with the format list); Import… → "+" menu. New/Open/recent are on the documents page
  (Ctrl+N / Ctrl+O still work).
- **Workspace units panel** (`doc_dialogs::WorkspaceUnits`): Onshape's 338 px feature-dialog-style panel
  at the feature dialog's place: Length (length, linear acceleration), Angle (angle, angular velocity),
  Mechanical (mass, density, force, frequency, moment, pressure, energy); default unit + Display decimals
  (0 … 0.12345678); ✓ commits, ✕ cancels. `UnitPrefs` gained `other: [(unit index, decimals); 9]` over
  `noshape_units::OTHER_UNITS` (serde default; `UnitPrefs::other_unit_name`). Option lists requested.
  **For ui:** mass properties could display in `other_unit_name(2)` (mass) etc.
- "+" menu = Onshape's dropdown (253 px, 14 px `#555`, 27 px rows, icons): Create Feature Studio | Create
  Part Studio, Create Assembly, Create Variable Studio, Create Drawing…, Import…. **To do:** Create
  Material Library and Create folder (tab folders) need new element types — not started.

### Element tab menu (workspace.rs, doc_dialogs.rs)
- Onshape's order: Delete | Rename, Properties…, Show code (Part Studios) | Duplicate, Copy to clipboard |
  Select as document thumbnail | Move to document…, Export… (Part Studios) — plus asb's "Create Drawing of
  <tab>…" (not in the live tab menu; asb's call).
- Rename = inline field in the tab (also double-click); Properties… = name + description (store meta
  `tab_description:<id>`); Show code = FeatureScript-style listing of the features (read-only modal; our
  features aren't FeatureScript, so this is a generated approximation — **lead: confirm acceptable**);
  Copy to clipboard → "+" shows "Paste <name>"; Select as document thumbnail → store meta `thumbnail_tab`
  (start_thumbnail uses it); Move to document… → another (closed) document gets the tab, deleted here.
- **To do:** Export… for Assembly (asb's `open_asm_export`) and Drawing tabs.

### Tab manager thumbnails
- Part Studios and Assemblies render real pictures (rows 70x40, preview 340x170) off the UI thread with an
  `AssemblyEvaluator`, cached per content hash (`Thumbs` in tab_manager.rs); type icons only for
  Variable / Feature Studios. **To do:** drawings (sheet picture).

### Author name (store, start_window)
- `noshape_store::local_user_name()` = Preferences name (`set_default_author`, from
  `~/.config/noshape/user.json`), else the OS full name (`os_user_name`: GECOS / `id -F` / `net user`), else
  the login. Documents page top-right: Onshape's account button (avatar, name ▾) → Preferences… (Name).
  New name applies to documents opened afterwards.

### Menus (os_menu styles added in ui's os_menu.rs, all opt-in)
- `list_no_icons()` (documents page), `dropdown()` (Bootstrap: "+", ☰, Variable Studio "…" / `?▾`,
  Variable table footer ▴; icon-less → text 20 px in; 7 px divider margins), `toolbar_list()` (drawing
  toolbar dropdowns), `select_list(width)` (Onshape select lists: Variable type).
- The Variable type select opens under its cell (bounds probed with a canvas).
- Workspace "Delete tabs" warning uses the Onshape modal.

### Dialogs
- Assembly Export and the in-app file path prompt (files.rs, used with NOSHAPE_NO_NATIVE_DIALOGS) use the
  Onshape modal (`os_ui::modal` + body + footer: blue primary, gray Cancel).
- Documents list Modified column: Onshape's "1:15 PM Today" format.

### Tab manager (tab_manager.rs, new; workspace.rs wiring)
- Docked 380 px left panel (shares the slot with Versions and history): "Tabs" header with search / ×,
  "Search tabs", type filter toggles for the kinds present + Clear, 54 px rows (70x40 icon cell, icon + name,
  italic type), selected `#b2ddf6` with a 6 px blue bar and 1 px blue outline, preview area `#f5f5f5`,
  footer "tab or ↑ ↓ to navigate ↵ to open". ↑/↓/Tab move, Enter opens, Esc closes. Alt+T and the
  bottom-left tab-search cell toggle it (that cell's old dropdown list is gone).

## Gotchas

- The shared tree often doesn't compile (other agents mid-edit). I verify in a private worktree
  `/root/noshape-ui2` (detached HEAD) by copying my files in, then build with my target dir. My edits are
  made in /root/noshape first; the worktree is only a build sandbox (delete it when done:
  `git worktree remove --force /root/noshape-ui2`).
- Kit `DialogAction` / `DialogClose` are `size_full`: wrap them (`os::modal_ok/modal_cancel`).
- Kit `Button::label` ignores `text_size`; put a sized `div` child instead.
- `pkill -f` patterns match your own shell when the pattern text is in the command line: the run script
  kills by PID file (/tmp/ui2/app.pid). Scripts: /root/ui2-scripts/{run.sh,shot.sh,sbs.sh}.
- `/root/ui2-scripts/wtbuild.sh` syncs crates/ into the worktree and builds there;
  `/root/ui2-scripts/drawing.sh [name]` opens `--demo-drawing-full` fullscreen on the Drawing tab;
  `vs.sh` (Variable Studio with two variables), `history.sh` (V1 + versions panel). wtbuild.sh also patches
  other agents' half-done lines in the sandbox copy only.
- Test HOME /tmp/ui2/home (docs in Documents/noshape, extra folders in .config/noshape/folders.json).

## Requests to other agents

- ui: (done) the Sketch dialog checkboxes and the reusable view cube — thanks; the assembly uses the cube.
- ui: Material dialog (your Part Studio Assign material): Onshape lists libraries in its first dropdown;
  the document's Material Library tabs are `noshape_model::material_library::libraries(&doc)` →
  (tab, name, `MaterialLibrary { materials: [LibraryMaterial { category, name, density kg/m³ }] }`).
  Please add them next to the built-in list (help image custom-mat-library-3: library ▾ + "+", material ▾).
- ui: surgical edits in your os_menu.rs (popup priority 100; `OsMenuItem::keys`; struct-literal `keys` field
  in drawing_view.rs / assembly_behaviors.rs) and main.rs `apply_theme` (code editor colors).
- ui: surgical edit in your part_studio/menus.rs: `open_export_dialog` now builds Onshape's Export modal
  through `os_ui::export_modal` (same as the tab menu's Export…); the export logic is unchanged.
- ui: the Part Studio toast could use `os_ui::message_bubble` (Onshape's bubble: icon, 14 px, bold × ).
- asb: clippy in your files: `drawing_ui.rs:170` redundant pattern matching, unused `TabContent` import in
  drawing_ui.rs, and `model/src/drawing/clipboard.rs:78/89` loop counters (these stop
  `clippy -D warnings`).

## For the lead

- Disk: free space fell to 11 GB (rule: >15 GB) during other agents' builds; my target dir is 5.3 GB
  after cleaning (incremental removed, stale test binaries deleted, /tmp/ui2 trimmed to ~280 MB).

- **Needs your decision (truly blocked):**
  1. *Feature Studio language* — RESOLVED: Rhai is removed; FeatureScript (agent fs, crates/featurescript) is
     the only custom-feature language. The Feature Studio UI matches live (see the Feature Studio sections).
  2. *Export formats.* Onshape: PARASOLID (default), ACIS, STEP, IGES, PVZ, JT, RHINO, GLTF, GLB, OBJ, 3MF,
     STL (+ URDF for assemblies). Parasolid, ACIS, JT, PVZ and Rhino need proprietary kernels/SDKs; we can
     write STEP, IGES, STL, OBJ now and I'm adding GLTF, GLB, 3MF, URDF. STEP version/units/preprocessing
     and IGES compatibility mode are requested from the kernel agent (kernel-robustness.md). Our "BREP"
     entry is not an Onshape format and goes away.

- New file `crates/app/src/os_ui.rs`; `main.rs` gets `mod os_ui;` and one `t.overlay` line.
- theme.rs: one added constant `viewport::SKETCH_REGION`.
- **Cargo:** crates/app/Cargo.toml adds `chrono = { version = "0.4", default-features = false, features =
  ["clock", "std"] }` (already in Cargo.lock as a transitive dep; the lock only gains it in noshape's
  dependency list). Used by `start_library::format_onshape_time` ("1:15 PM Today", local time) for the
  documents list and the versions panel; the old relative `format_modified` / `capitalize` /
  `civil_from_days` are gone.
- New files `tab_manager.rs` (main.rs `mod tab_manager;`); workspace.rs: tab manager panel wiring, history
  panel width, rail Create version opens the dialog, tab-search cell toggles the tab manager.
- `drawing_ui.rs:12` unused `TabContent` import comes from asb's edit, not mine.
- model drawing/sheet.rs (border) and drawing/font.rs (`text_fill`, `fill_supported`, a test) are asb's
  files; my edits there are self-contained.
- New icons (our own artwork): sheet-page, sheet-insert, compare, auto-version, msg-info, msg-warning,
  msg-error (.svg in crates/app/assets/icons).
- Verified only in my sandbox worktree `/root/noshape-ui2` (the shared tree had other agents' half-done
  edits); the shared tree has all my changes. The worktree can be removed when the lead no longer needs it.
