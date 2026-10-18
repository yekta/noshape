# Restyle outside the Part Studio chrome (agent "ui2")

Goal: everything outside the Part Studio chrome looks exactly like Onshape: sketch visuals, assembly,
drawing, start window / documents page, Variable Studio, versions panel, tab manager, generic dialogs and
toasts. The Part Studio chrome and theme.rs belong to agent "ui" (docs/handoff/theme-layout.md).
Build dir /root/target-ui2, display :105. Side-by-side shots (ours | Onshape) in /tmp/ui2-shots/.

## State (2026-10-18 16:50 UTC)

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
- Not yet verified in the app (build waiting on ui's in-progress analysis_tools/thickness edits).

### Import / Export (workspace.rs, os_ui.rs, assembly_behaviors.rs, part_studio/menus.rs)
- "+" › Import… creates a new Part Studio tab named after the file and opens it (Onshape), instead of
  importing into the active Part Studio. Helpers `workspace::{read_foreign, import_names,
  add_import_feature, IMPORT_EXTS}` are shared with the documents page.
- `os_ui::export_modal`: Onshape's Export modal (600 px, File name, Format select, Export / Cancel). It's used by the tab
  menu Export… (Part Studio and Assembly tabs; Assembly via `AssemblyView::open_export_all`), the
  assembly instance Export… and the Part Studio part Export… (ui's menus.rs, surgical). The file name is
  carried into the save dialog. Drawing tab › Export… calls the PDF export until asb's "Export Drawing" modal
  lands (asb is building it; told in drawing-behaviour.md). Format-specific option rows (STEP version, STL
  resolution…) wait for the capture requested in live/REQUESTS.md line 68.

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
- ui: surgical edit in your part_studio/menus.rs: `open_export_dialog` now builds Onshape's Export modal
  through `os_ui::export_modal` (same as the tab menu's Export…); the export logic is unchanged.
- ui: the Part Studio toast could use `os_ui::message_bubble` (Onshape's bubble: icon, 14 px, bold × ).
- asb: clippy in your files: `drawing_ui.rs:170` redundant pattern matching, unused `TabContent` import in
  drawing_ui.rs, and `model/src/drawing/clipboard.rs:78/89` loop counters (these stop
  `clippy -D warnings`).

## For the lead

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
