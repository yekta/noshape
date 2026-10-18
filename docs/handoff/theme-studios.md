# Restyle outside the Part Studio chrome (agent "ui2")

Goal: everything outside the Part Studio chrome looks exactly like Onshape: sketch visuals, assembly,
drawing, start window / documents page, Variable Studio, versions panel, tab manager, generic dialogs and
toasts. The Part Studio chrome and theme.rs belong to agent "ui" (docs/handoff/theme-layout.md).
Build dir /root/target-ui2, display :105. Side-by-side shots (ours | Onshape) in /tmp/ui2-shots/.

## State (2026-10-18 14:00 UTC)

Taken over at 13:25 by a fresh "ui2" (previous one ran out of quota; nothing was half done except the
sheets-panel patch, which had been applied).
- Start window / documents page: done (first pass).
- Sketch visuals: done (first pass). Assembly: list, toolbar, mate dialog done; mate glyphs, triad, Insert
  dialog (asb is editing it) and the view cube still to do.
- Drawing: sheet, frame, text and Sheets panel done (see below). Toolbar contents are asb's.
- Variable Studio, Versions and history (+ Create version / Create workspace dialogs) and Tab manager: done.
- Next: generic confirmation dialogs and toasts, assembly leftovers (mate glyphs, triad, Insert dialog, view
  cube), then the sketch + documents second pass.

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
- /tmp/ui2-shots/final-tab-manager.png

### Sketch (sketch_mode.rs, part_studio/sketch.rs, scene_builder.rs; surgical)
- Dimension values: 14 px black text with no box (hover `#d9bfa0`, selected `#d9a566` rounded boxes, driven
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

- Sheets panel nests derived views under their parent (20 px per level), like Onshape.

### Variable Studio (variable_studio.rs, rewritten render)
- Onshape's table: undo/redo toolbar, table 42 px lower, centered 561 px; `#eaeaea` 35 px title row
  (caret collapses, 16 px/500 tab name, blue `•••` menu with Copy table), `#eaeaea` bold header row, 140 px
  columns, 34 px rows, `#aaa` rules, borderless inputs (text 7 px in), "Length ▾" type dropdown (Length,
  Angle, Number, Any). Last row = italic "Name" cell: typing a name adds a Length variable = `0 <doc unit>`.
  Values with `#refs` show their result until clicked. Row right-click: Delete, Move up, Move down.
- Not done: "Insert Variable Studio" button and the "Insert into all Part Studios" footer (no model).

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

- ui: the Sketch dialog should not show the "Final"/preview footer; Onshape puts the Show constraints /
  expressions / errors checkboxes in the dialog itself (we draw them in a panel right below it).
- ui: the view cube (part_studio/view_cube.rs) is tied to PartStudioView; Onshape shows it in assemblies too.
  A reusable cube would let the assembly drop its six view buttons.

## For the lead

- New file `crates/app/src/os_ui.rs`; `main.rs` gets `mod os_ui;` and one `t.overlay` line.
- theme.rs: one added constant `viewport::SKETCH_REGION`.
- New files `tab_manager.rs` (main.rs `mod tab_manager;`); workspace.rs: tab manager panel wiring, history
  panel width, rail Create version opens the dialog, tab-search cell toggles the tab manager.
- `drawing_ui.rs:12` unused `TabContent` import comes from asb's edit, not mine.
- model drawing/sheet.rs (border) and drawing/font.rs (`text_fill`, `fill_supported`, a test) are asb's
  files; my edits there are self-contained.
