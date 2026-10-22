# Theme and layout restyle (agent "ui")

Goal: the Part Studio must look exactly like Onshape (live reference in
/root/onshape-research/live/, spec in docs/ONSHAPE_UI_SPEC.md). Build dir
/root/target-ui, display :101. Side-by-side shots go to /tmp/ui-shots/.

## State (2026-10-19 03:30 UTC)

Round 4 (lead's list after the Wayland decision) is done: Wayland count cursor, Appearance and
Configurations bodies, Graphics preferences (dialog + live settings), Highlight boundary edges,
tool search name matching + Onshape ranking, full tool descriptions for Part Studio **and sketch**
tools, toolbar tooltips placed and styled like live (see "Round 4" and "Round 5"). Still open from
it: section interference color and mesh line width are in the dialog but not rendered (no section
interference / mesh bodies yet); Measure unit lists and the configured Configurations table wait
for live captures (REQUESTS.md 18:21, 18:31). The documents-page menus were switched by ui2.

Now on the QA round 3 list (docs/handoff/qa-round-3.md, "ui" sections), queue under "Next";
items 1-6 and 8 are done and part of 9 ("Round 6"); 7 (toolbar overflow) waits for a capture.

The shared tree keeps breaking in other agents' files (model best_tessellation, BodyId no longer
Copy in analysis_tools/thickness, menus.rs v9, workspace.rs focus_find / export dialog,
start_window open_dialog); I build and test in a private copy (/root/noshape-ui) synced by
/tmp/ui-tools/sync-private.sh, which patches only those errors locally. All my edits are in the
shared tree.

## Done this round (uncommitted)

- theme.rs: `tool_hover` is `#eaeaea` (measured live), new `tool_hover_group` `#d7d7d7`.
- ui.rs: tool cells 32 px (pitch 34), split tools (`split_tool_button` + `tool_caret`
  18 px in a `tool_group` with group hover), separators flush, "Search tools" 32 px,
  `icon_button`, `shortcut_chip(s)`, `menu_tool_item` (32 px dropdown rows with
  full-color icons and shortcut chips), left panel 206 px.
- Feature list (part_studio/mod.rs): filter row first (funnel + 30 px box), then the
  "Features (N)" row (13.33 px / 500) with new-folder, pause and timer icon buttons;
  26 px rollback row; folder children indented 18 px with the guide line at 10 px;
  list panel has `#ccc`-style top/bottom borders; collapse toggle is Onshape's 33x34
  tab at the viewport's left edge (`render_list_toggle`), the panel collapses to 0.
- Right strip (config_panel.rs): 33x34 buttons with 1 px `#aaa` borders.
- Quick tools (menus.rs `render_evaluation_tools`): 110x30 strip flush bottom-right,
  white 75%, Measure / Analysis tools (dropdown) / Mass properties.
- View cube (view_cube.rs): 150 px widget 15 px from the top-right, triangle and roll
  arrows as tinted glyphs (`cube-*.svg`), no outlines on cells, camera menu button
  44x28 at the widget's bottom-right.
- Origin: 11 px ring with a center dot (point style bit 4 in shaders.wgsl).
- Plane labels: drawn in-plane (glyph outlines from noshape_sketch::text projected and
  painted as a path, blue), like Onshape's skewed labels.
- Navbar: 139 px logo block, 14 px padding. Tab bar: tab-search cell in rail color,
  plus glyph, 5 px before the first tab.
- New icons: new-folder, pause-regen, regen-timer, analysis-tools, tab-search, treelist,
  cube-up/down/left/right, cube-roll-left/right; history and version redrawn after
  Onshape's rail icons.

## Done after the 12:56 handoff (found in the diff, by the previous agent)

- ui.rs: `RichTooltip` / `rich_tooltip` / `with_rich_tooltip`: toolbar tools show the title,
  then expand in place (400 px) with the feature description after 2 s (0.5 s gpui delay +
  1.5 s timer). Used by part_studio/mod.rs toolbar tools (`ty.description`).
- ui.rs `menu_tool_item` now used by the toolbar dropdowns (mod.rs) and ui2's mate dialog.
- shaders.wgsl `shade()`: lighting refit to Onshape's face colors (one key light above-right
  of the viewer, ambient 0.284, broad spec), see cmp-hover-sel.png.
- scene_builder.rs: the sketch being edited fills closed regions with `SKETCH_REGION`
  `#9aa5af` at 26% (theme.rs).
- feature_dialog.rs (visual part; psb edits the behaviour in the same file): segmented
  rows at 12 px with flex-grow + 8 px padding, selected 500 blue with a 2 px underline,
  others a 1 px `#d7d7d7` underline; enum dropdowns 14 px with `ui-caret-down`; selection
  rows hover with a white 35% overlay and a tooltip.
- /tmp/ui-shots/cmp-*.png (13:14) are the previous agent's ours-vs-Onshape comparisons for
  dialog, rmb, hover/selection, tooltip, dropdown; st-*.png are our raw states.

## Done since takeover (13:25-14:50)

- **Onshape right-click menus** (`os_menu.rs`, new, mine): `OsMenu` / `OsMenuItem` with the kit
  `PopupMenu` builder API (`item`, `separator`, `submenu`, `.icon`, `.icon_name` for full-color app
  icons, `.on_click`). Live DOM style: white, 1 px `#ccc`, radius 4, shadow 0 5 10 .2, 1 px padding,
  22 px rows, 12 px `#333` text at x=36, 16 px icon at x=11, hover `#def1fb`, `#ddd` separators,
  submenus beside the row with a ▸ (`ui-caret-right.svg`). Keyboard: up/down/right/left/enter/esc.
  `OsContextMenuExt::os_context_menu` (right-click on any element; drawn by `os_menu::render_host`,
  which workspace.rs adds at the window root). Switched: the viewport menu (menus.rs
  `open_context_menu`), the sketch menus (sketch.rs, both builders), and every `.context_menu` in
  part_studio/mod.rs (feature rows, planes, parts, rollback bar, toolbar). Other areas (assembly,
  drawing, dialogs) still use the kit menu; switching one is an import change
  (`use crate::os_menu::OsContextMenuExt as _` and `.os_context_menu`).
  New icons (our own artwork): `menu-delete.svg` (gray circled ×), `menu-section-view.svg`.
  Shot: /tmp/ui-shots/final-context-menu.png. Missing vs Onshape (behaviour, psb): "Show
  dimensions", "Copy Part 1", "Export as DXF/DWG…" in the face menu.
- **ui2 request (a)**: the Sketch dialog shows Show constraints / Show expressions / Show errors
  inside the dialog under "Disable imprinting", and the diagnostics button in the footer left of
  `?`; no preview slider for sketches. Mechanism: `FeatureDialog::extra: Option<DialogExtra>`
  (rows + footer element from the owner), set in part_studio/mod.rs for `type_id == "sketch"` from
  `PartStudioView::sketch_dialog_extra` (sketch.rs; replaces the old floating
  `render_sketch_options` panel). The "Final" checkbox still shows only when later features exist
  (Onshape shows a "Final" button then). Shot: /tmp/ui-shots/cmp-sketch-dialog.png.
- **ui2 request (b)**: the view cube is reusable: `crate::part_studio::view_cube::view_cube(rotation,
  hover, menu, CubeHooks { state, rotation, click, rotate, roll }, cx)` for any view `V`
  (`part_studio::view_cube` is now `pub mod`). The Part Studio calls it through
  `render_view_cube`. The assembly owner must keep `Option<CubeCell>` + `Option<Point<Pixels>>`
  fields and pass its camera moves and its camera menu element.
- `ui::os_checkbox` (Onshape dialog checkbox: 27 px row, 12 px box at x=4, label 13.33 `#666`),
  now used for every boolean in feature dialogs and the sketch options; group expanders are 20 px
  wide, expanded `⌄` red-brown `#a0522d`, collapsed `›` `#d7d7d7`.
- **Feature dialogs** (feature_dialog.rs, visual parts only): number rows = Onshape's (tolerant
  quantities get the 20 px `›` expander; label 12 px `#666`; field fills the 194 px first column;
  20 px tolerance toggle with the new `tolerance.svg`; flips in the 26 px right column). Flip icon
  `flip-direction.svg` (feature dialogs only; the mate dialog keeps `flip`). Header ✓/✕ are bold
  `dialog-ok.svg` / `dialog-cancel.svg` (`#bd3039`). Own preview slider (`render_preview_slider`:
  112 px, 2 px `#777` track, 11 px white handle with a `#888` ring) replacing the kit slider; footer
  22 px (25 for the Sketch dialog), 7 px left inset. Group fix: a collapsed/unchecked group header
  now hides only the params that depend on it, so Extrude shows Symmetric and Draft after
  "Starting offset" again (they were hidden). Shot: /tmp/ui-shots/final-feature-dialog.png.
  Left: the Depth value shows left-aligned while its field has focus (gpui Input can't
  right-align); Onshape's slider handle rests at ~69% of the track (unknown meaning; ours at 100%).
- Fixed leftovers of the previous agent: `.flex_grow()` without its argument (feature_dialog.rs),
  removed the unused `Palette::dialog_footer` and `ui::panel_title`.

## Round 3 (15:00-18:00, lead's list; all verified in the app on X11)

- **Selection colors** (shaders.wgsl, scene_builder.rs): shaded faces no longer tint on hover;
  their boundary edges light up instead (`face_edges`: the body edges whose points are all
  vertices of the face's triangles get the face's HOVER/SELECTED flag). Selected faces use
  `FACE_SELECTED` (linear 0.80, 0.444, 0.07 → top face reads `#e4b049` like live) with half the
  light contrast. Edges, curves and points use `highlight_stroke`: fully `#ffc685` on hover,
  fully `#ffa90a` selected. Unlit meshes (planes) still tint on hover.
- **Selection-count cursor**: a real image cursor, like Onshape's (`cursor.rs` + per-OS
  backends `cursor_x11.rs`, `cursor_macos.rs`, `cursor_windows.rs`). The image is our own
  drawing (SVG built at runtime: slim arrow, digit from Liberation Sans Bold, white outlines,
  soft shadow; 32 px, hot spot 3,3, digit 1-5, "5" for more), rasterized with gpui's SVG
  renderer at the window scale. X11: ARGB RENDER cursor set on the window from our own
  x11rb connection; macOS: `NSCursor` from the image (`[cursor set]`); Windows: icon cursor
  via `CreateIconIndirect` + a `SetWindowSubclass` hook answering `WM_SETCURSOR`. GPUI only
  re-sets its cursor when the hovered element's style changes, so `Viewport::sync_cursor`
  applies ours when the count / hover / drag state changes and again 40 ms later (after
  GPUI's end-of-frame cursor update); leaving the viewport restores the arrow. Count source:
  part_studio/mod.rs `refresh_scene` (sketch selection while sketching, else the Part Studio
  selection; 0 while a feature dialog is open). Verified on X11 with XFixes
  (`/tmp/ui-tools/getcursor.py`); the macOS and Windows backends pass `cargo check` and
  clippy for aarch64-apple-darwin / x86_64-pc-windows-msvc in a scratch crate
  (/tmp/ui-tools/curcheck) but were not run. **Wayland** (lead chose option a): gpui-pre-linux
  0.3.9 is vendored in `vendor/gpui-pre-linux` (license kept, Cargo.lock dropped) and wired
  with `[patch.crates-io]` + `exclude = ["vendor"]` in the workspace Cargo.toml; the app depends
  on it directly (linux) for the hook. Patch (all marked `NOSHAPE PATCH`): new
  `src/custom_cursor.rs` (`set_custom_cursor(Option<CustomCursor>)`, premultiplied ARGB8888 +
  hot spot + buffer scale; thread-local image and an apply hook), `wayland/cursor.rs`
  `Cursor::set_custom` (memfd → `wl_shm` pool → `Argb8888` buffer on the cursor surface,
  `wl_pointer.set_cursor`), `wayland/client.rs` (hook registration, `apply_cursor`, and the
  custom image wins in `set_cursor_style`, pointer enter and restore-after-hide). App side:
  `cursor_x11.rs` `set_wayland`. `cargo metadata --locked` passes; Cargo.lock only drops the
  registry source/checksum of gpui-pre-linux. **Tested under Wayland**: headless sway
  (`WLR_BACKENDS=headless WLR_RENDERER=pixman`, config /tmp/ui-wl/sway.conf), the app with
  `WAYLAND_DISPLAY`, a persistent virtual pointer (/tmp/ui-tools/vptr, our own test tool;
  `wlrctl` re-creates its device per call, which drops clicks) and `grim -c`: selecting a face
  shows the "1" cursor (`WAYLAND_DEBUG`: `wl_shm.create_pool` → `create_buffer 32x32` →
  `wl_pointer.set_cursor(…, 3, 3)`), leaving the viewport goes back to `set_shape(default)`
  (final-cursor-wayland.png). weston, sway, wlrctl and grim were installed with apt for this.
  New deps (all already in Cargo.lock): raw-window-handle 0.6; x11rb 0.13 (linux, features
  render/cursor/resource_manager); objc2 0.6, objc2-app-kit 0.3, objc2-foundation 0.3
  (macos); windows 0.62 (windows). Cargo.lock only gains dependency edges.
- **os_menu.rs**: Esc now closes (own `OsMenu` key context with MenuUp/Down/Left/Right/Confirm/
  Dismiss actions, which win over the window's Cancel and arrow-key view rotation); Left leaves a
  submenu; `checked`, `disabled`, `submenu_with_icon`, `open_anchored`; submenus open to the left
  near the window's right edge. The camera menu (view_cube.rs) is now an `OsMenu` opened under
  its button (right-aligned), with psb's items unchanged.
- **Sketch dialog**: Show constraints defaults to **off** (sketch_mode.rs `SHOW_CONSTRAINTS`,
  skb's file; live sketch/00, 01, 10 show it off and no glyphs); `mc-select.svg` 26 px "Select
  mate connector" button right of every field that accepts a mate connector (it activates the
  field); diagnostics button uses `sketch-diagnostics.svg`.
- **Feature dialog**: numbers stay right-aligned while typing (`TextAlign::Right`); the preview
  slider rests at 70% like live (`PREVIEW_REST`; full preview from there to the right end).
- **Feature list**: suppressed rows italic too; error rows only turn red (no red dot on the
  icon, live part-studio/94).
- **Tool search** (view_tools.rs, rewritten): live layout (600 px under the toolbar's right end,
  white input with `#3e94ff` border, `#f5f5f5` body: 198 px results with 34 px rows and shortcut
  chips, selected `#b2ddf6`; description pane with 28 px icon + 18 px name; `#eee` footer with
  "Return to this dialog…" and the "Highlight tool in toolbar" checkbox). Arrow keys move the
  selection, Enter runs it, Esc / outside click close. The toggle reads "Close search [escape]"
  while open (`ui::search_tools_toggle`). With highlighting on, the selected tool's toolbar
  button turns yellow `#fcda7f` / `#fab601` (`palette_found_tool`). New UiState fields
  `palette_sel`, `palette_no_highlight`, `palette_closed` (behaviors.rs); `palette_hover` removed.
- **Display modes**: labels "Shaded without edges" / "Unshaded"; translucent alpha 0.48 (linear,
  = live's ~35% sRGB).
- **Right panels** (config_panel.rs): 712 px frame, title as one full-width blue tab, `#eee`
  footer with `?`, no ×. Variable table rewritten as Onshape's table (section header, 4 columns,
  "Name" placeholder row and the faint illustration when empty).

- **Feature dialog groups**: a checked, expanded group's params are indented 24 px with a
  `#d7d7d7` guide line (live extrude-24); expanders are `#555` (`⌄`) / `#d7d7d7` (`›`).
- **Feature list**: rolled-back rows `#999` (was `#ccc`), hidden (consumed) features `#999`;
  the rollback bar turns light blue only after ↑/↓ or a click on it (`ui.rollback_keyboard`,
  behaviors.rs), not when a row is selected; the drag pill sits right of the pointer.
- **Tooltips** (ui.rs `RichTooltip`): centered under the tool, title box 9 px below it, the
  expanded one (after 2 s) 3 px below, 400 px, `#d0dbe5` title band + white body (live 10, 11).
- **Measure** (menus.rs): Onshape's 220 px panel (header + ×, selection field, Measure type ▾,
  Length unit, Angle unit, Reference coordinate system, result rows with monospace values,
  `#eee` footer with ?). **Mass and section properties**: Onshape's 440 px dialog at the
  feature-dialog place (tabs Part | Face, Parts to measure, Mate connector for reference frame,
  Show calculation variance, Mass + Override, Volume, Surface area, Center of mass X/Y/Z with
  axis glyphs, Mass moments of inertia 3x3; without a material: part red, center of mass empty
  and the yellow toast "One or more parts do not have a material defined.").
- **Toasts** (mod.rs): Onshape's message bubble: 18 px icon (`toast-info/warning/error.svg`),
  14 px text, padding 4 25 4 10, radius 5.25, bold × that closes it.
- Context menu icons: `menu-copy.svg` (two pages) and `menu-create-drawing.svg` (sheet with a
  blue +) for psb's new Copy / Create Drawing items (final-context-menu.png).

### Open for the lead

- Wayland count cursor: done with the vendored, patched gpui-pre-linux (lead's option a).
  When gpui-pre-linux is upgraded, re-apply the `NOSHAPE PATCH` hunks (3 files + module line).
- Measure: "Length unit" / "Angle unit" show the document units but don't open a list yet
  (behaviour, psb); the model's feature descriptions (tooltips, tool search) are shorter than
  Onshape's texts (model, psb).

### Menus switched to os_menu.rs (for ui2 and asb)

- asb already moved the assembly menus and the drawing menus are on `OsMenu` too (whoever did it).
- Switched now (import swap, `.context_menu(` → `.os_context_menu(`, `PopupMenuItem` →
  `crate::os_menu::OsMenuItem` inside those closures): feature_dialog.rs (pick rows, Convert to
  expression), part_studio/appearance.rs, variable_studio.rs, workspace.rs (tab menu),
  history_panel.rs. Not switched: start_window.rs (documents page; Onshape's row menu there is a
  Bootstrap dropdown, ui2's call) and toolbar/field **dropdowns** (`dropdown_menu`), which in
  Onshape are different widgets (toolbar dropdowns: 32 px rows, `#eaeaea` hover).
- To switch another menu: `use crate::os_menu::OsContextMenuExt as _;` then `.os_context_menu(
  |m: OsMenu, window, cx| …)`; items are `OsMenuItem::new(label).icon_name("app-icon")
  .checked(b).disabled(b).on_click(…)`; `m.separator()`, `m.submenu(label, window, cx, f)`,
  `m.submenu_with_icon(Some(icon), …)`. A menu opened from a button:
  `os_menu::open_anchored(point, Anchor::TopRight, OsMenu::build(window, cx, f), window, cx)`.

## Round 4 (18:20-20:10)

- **Tool search**: matches tool **names** only, letters in order (Onshape: "fil" → Fillet, Fill,
  Finish flex PCB model, Finish sheet metal model, 3D fit spline, Modify fillet), ranked by the
  first matched letter, then the span of the match, then catalog order; nothing until something is
  typed ("Start typing to search tools", italic "Search tools" placeholder). Views and display
  modes are no longer search results (Onshape lists tools only). final-tool-search.png.
- **Tool descriptions** (`tool_text.rs`): every Part Studio tool, after the live tooltip and the
  help icon glossary, in our own words; used by the expanded toolbar tooltip and the tool search
  pane (fallback: the model's description).
- **Appearances panel**: tabs "Parts and surfaces | Sketches and curves", one row per part / sketch
  (bold name on `#eaeaea` | swatch bar, checkerboard under translucent colors), face/feature
  appearances after them, double-click edits, "Add face appearance" icon (`add-appearance.svg`)
  in the title row. Default part color is now live's `#9dcfed` (model regen/display.rs
  `PART_COLORS[0]`); the shader's diffuse/ambient were scaled by 0.745 so the shaded part looks
  the same as before (render tests pass). The other 7 rotation colors still come from the help
  page; live capture requested. final-appearance.png.
- **Configurations panel** (empty state = live): "Configure Part Studio ▾" split button (main adds a
  List input "Configuration"; ▾: List, Checkbox, Variable; "Text" removed, Onshape has no such
  input kind), gray illustration (`config-illustration.svg`), Onshape's text in its four lines with
  the help link (opens the Onshape help page). With inputs, the older editor still shows (live
  capture of the configured panel requested). final-configurations.png.
- **Highlight boundary edges** (camera menu, checked item): laminar edges (only one face's
  triangles contain all their points; `scene_builder::laminar_edges`, cached per mesh) in red
  `#e00000`, 2 px solid plus 1.5 px dashed on top for the hidden parts.
- **Graphics preferences…** (camera menu, gear icon): `graphics_prefs.rs` (global, saved to
  `<config>/noshape/graphics.json`, Onshape defaults sampled from the help images) and
  `graphics_prefs_dialog.rs` (feature-dialog frame, General | Sketch tabs, collapsible groups,
  value + slider rows, color swatches with a picker, ⓘ hints, Reset to default; live preview, ✓
  saves, ✕ restores). Applied live via `PartStudioView::apply_graphics_prefs` (observes the
  global): field of view, edge line width (new per-instance `Instance::line_scale` in the
  renderer), shininess + specular color (new `Style::specular` uniform), phantom tangent edge
  width/color, active sketch line width / constrained / underconstrained colors, inactive sketch
  width/color, construction line dash/gap pattern (new `LineSegment::CONSTRUCTION` style and
  `Style::construction_dash`), ambient occlusion (new screen-space pass `crates/render/src/ao.rs`
  + `ao.wgsl`: after the color pass, reads the depth buffer (now `TEXTURE_BINDING`, stored only
  when AO is on), 12-sample spiral, multiplied into the resolved image; `Style::ambient_occlusion`).
  Not yet rendered: section interference color, underconstrained occluded color (needs a
  hidden-only line pass), mesh line width (no mesh bodies yet). final-graphics-prefs.png, final-graphics-prefs-sketch.png.
- Removed the Wireframe display mode (Onshape has none) and the unused `DisplayMode`
  ALL/label/icon, `theme::SKETCH_UNDER`.

## Round 5 (20:10-21:30)

- **Underconstrained occluded sketch color**: while a sketch is edited, its underconstrained
  entities also draw where geometry hides them, in the Graphics preferences color (`#4040ff`):
  new `Instance::hidden_only` (renderer `line_hidden`/`point_hidden` pipelines, depth test Less,
  no depth write). Render test `hidden_only_lines_show_only_behind_geometry`.
- **Sketch toolbar tooltips**: `ui::with_tool_tooltip` (used by `render_sketch_toolbar`) shows
  Onshape's title, `ui::onshape_title` (lowercase shortcuts, "Center point rectangle", "Use
  (Project/Convert) (u)", Undo "(cmd+z/ctrl+z)", Redo "(ctrl+y/shift+cmd+z)" as in
  dom/toolbar-sketch.txt), and expands with `tool_text::sketch_description` (every sketch tool and
  constraint, our own words). final-sketch-tooltip.png.
- **Toolbar tooltip fixes** (`ui::RichTooltip`): it was 32 px too low (the bounds probe canvas was
  `absolute()` without insets, so in a block parent it sat *below* the tool) and in the system
  font (tooltips are drawn outside the window's element tree, so they don't inherit the UI font).
  Now placed with `anchored()` in window coordinates 3 px under the tool, centered, both states
  (live: y=77, title 26 px, 12 px/500, 4 px padding; description 12 px on 18 px lines, 4 px
  padding), UI font set explicitly. Same inset fix in os_menu.rs, feature_dialog.rs,
  graphics_prefs_dialog.rs. Moving straight from one tool to the next shows the next tooltip
  (QA3 item not reproduced). final-tooltips.png.
- Undo/Redo are 2 px apart like live (Undo @42, Redo @76), which moved every Part Studio tool 2 px
  right onto live's x positions.
- Removed `Session::undo_description`/`redo_description` (only the old tooltip text used them).

## Round 6 (21:30-22:45, QA round 3)

- **Onshape select** (`os_select.rs`, new): feature dialog dropdowns (every non-tab enum) and the
  Hole lookup rows. Closed: 14 px text over a 1 px `#d7d7d7` underline, 26 px (Hole rows 22 px),
  `#def1fb` hover. Open: a search input with a 2 px `#1651b0` underline in its place (text 4 px
  in, blue 9 px × to clear) and the list under it (field width, 1 px border, 5/4 px padding,
  26 px rows of 14 px `#666`, highlight `#1651b0`/white starting on the current item); ↑/↓ and
  the pointer move it, typing filters (substring, matched letters bold), Enter/click picks, Esc
  closes only the list (bound in the select's key context and on `OsSelect > Input`, which wins
  over the dialog's `FeatureDialog > Input` Esc = Cancel), Tab/click elsewhere close; focus goes
  back where it was. Pixel-identical to dialogs/extrude-13/15. final-dialog-dropdown.png.
- Dialog rows: the select fills the first 194 px column flush with the dialog edge (flip in the
  26 px column); select labels ("Measurement", "Start plane") 12 px `#666` with 5 px padding.
  Hole: no "Standard" header row; levels are "Hole type ▾", "Drill size ▾" rows (88 px
  right-aligned 12 px `#999` labels). final-hole-dialog.png (title spec and the
  diameter/depth/tip icon rows are still to do).
- Dialog frame: header 29 px (28 + border), selection fields 3 px from the edges with 5 px above
  and below (4 px above a select), empty field 26 px (label padding 6/5, 13.33 px line), picked
  field caption 10 px at the top; chip × always shown. Header/tabs/field rows now land on live's
  pixels.
- **Fillet dialog** (model `features/core.rs`, surgical, ids follow Onshape's FeatureScript):
  Edge | Full round; Entities to fillet; Tangent propagation; Measurement ▾ (Radius, Width);
  Control ▾ (Distance, Conic, Curvature); Radius ± / Width / Rho / Magnitude; Asymmetric (Second
  radius + flip); › Partial fillet (End type ▾ Position, Start position + flip, Second bound, End
  position); › Variable fillet (Vertices, Points on edges array "Add point on edge", Smooth
  transition); › Allow edge overflow (Edges to keep); › Smooth fillet corners (Corners to
  exclude); Full round: First side face, Second side face, Faces to round, Tangent propagation.
  Groups that are on by default start folded (live "› ☑ Allow edge overflow"); checking a group
  unfolds it. **Regen still ignores** asymmetric, conic/curvature, partial, variable, edge
  overflow and smooth corners (kernel work for psb). Partial fillet End type options and the
  Second bound fields are guesses until the requested capture (REQUESTS.md 22:40).
  final-fillet-dialog.png.

- **Modals** (QA3 item 3): theme `modal_bg` (opaque white) and `modal_backdrop` (black 50%,
  measured on drawing/01 and assembly/100); Create Drawing and "Select a part or assembly" use
  them (drawing_create.rs / drawing_insert.rs, colour swap only). Sketch name labels and the list
  toggle now draw under feature dialogs (mod.rs overlay order).
- The +, tab right-click and ☰ menus were already Onshape's (QA tested an older commit).
- **Keyboard focus** (item 6): os_menu remembers the focus it took and gives it back when it
  closes (before running the item, so a prompt the item opens returns there too; deferred for
  outside clicks unless the click focused something); the workspace refocuses the active tab
  whenever focus is lost (`on_focus_lost`); Enter in a Configurations/Variable cell hands focus
  back to the Part Studio. Verified: right-click Fillet 1 › Suppress, then Alt+C "shell" Enter
  opens Shell.
- **Configurations panel** (item 8, live part-studio/182-186, dom/panel-configuration-*.txt):
  configured Part Studios get tabs Configurations | Configured properties, a filter row
  (funnel, "Filter inputs", List/Checkbox/Variable "show only" toggles, ⋯), and per input a
  `#eaeaea` header bar (⌄, 16 px name (double-click/⋯ Rename), "Default: …" 14.67 px `#999`,
  "+ Configure features", ⋯ Rename/Delete) over a `#aaa`-grid table: frozen 140 px Name column
  (current configuration blue/500 with a 3 px blue bar; click a row to switch; double-click to
  rename; right-click Delete), one 140 px column per configured parameter (feature name over
  the parameter; checkboxes for Suppressed), the italic "Name" row adds a configuration, and the
  blue "No features are configured" box. Footer "[▦ Add configuration input][▲]". A new List has
  only "Default" (live). The feature list gets the "Configurations [▦👁][▾]" box with one row per
  input (List = Onshape select). The old "Configure a parameter" menu is gone (Onshape uses
  "+ Configure features" and right-click › Configure in dialogs). Configured properties tab:
  a Part | Name table (no capture yet). final-configurations-table.png,
  final-configurations-box.png.
- **Right strip**: 5 buttons like live (Appearance, Configuration, Custom tables, Inspection
  table, Variable table); Custom tables and Model definitions panels show Onshape's empty states
  (own illustrations); custom tables (FeatureScript) are implemented by asb
  (part_studio/custom_tables.rs); inspection tools are not implemented.
- Item 9: region hover is a light-orange boundary only (selected: amber fill + amber boundary;
  `scene_builder::region_outline`); "regenerating…" removed from the Features header.

- **Fillet ids = Onshape's fillet.fs** (docs-raw/fs-std/fillet.fs, lead/kern request): radius |
  nonCircularRadius (conic/curvature) | width, rho, magnitude, isAsymmetric/otherRadius/
  flipAsymmetric (Measurement = Radius only), isPartial group (startPartialType Position/Entity/
  Offset with partialFirstEdgeTotalParameter / startPartialEntity / startPartialOffset,
  partialOppositeParameter (default on, flip), useTrimmedFirstBound, secondBound + end*),
  isVariable group: `vertexSettings` array {vertex, vertexRadius, vertexOtherRadius,
  vertexFlipAsymmetric, variableRho, variableMagnitude} shown as Onshape's driven "Vertices"
  selection field (new `Hints::DRIVEN_QUERY` in model params.rs: a pick adds/removes an item,
  match-last-item values copied), `pointOnEdgeSettings` array {edge, edgeParameter "Location",
  pointOnEdgeRadius, ...OtherRadius/FlipAsymmetric/VariableRho/VariableMagnitude} in Onshape's
  bordered box with "Add point on edge", smoothTransition; allowEdgeOverflow (+ keepEdges) and
  smoothCorners (+ smoothCornerExceptions) hidden for Curvature. Array items now see the
  feature's values when their visibility is evaluated (render_params overlay), like FS loops.
  Arrays in every dialog use the bordered "Add <item>" box (live fillet-12).
- **Part Studio Export modal** (`part_studio/export.rs`, item 5; tab › Export… and part ›
  Export…): live layout to ±1 px (document/80-81): File name + "View export rules" + ?, Format
  (Onshape's 12 formats, PARASOLID default), Y axis up, per-format rows (Parasolid/Rhino Version
  grayed while "Use latest version"; STEP Preprocessing, Version AP242/AP214/AP203, custom units,
  derived tolerances, MBD text height; IGES compatibility; PVZ grouping; GLTF/GLB Resolution +
  Compress; OBJ/3MF Resolution; STL Format/Units/Resolution), Options, unique parts as
  individual files, hidden instances; Bootstrap checkboxes (14 px box, 14 px `#333`). Writers:
  STEP/IGES (kernel), STL (text/binary, units), OBJ (meters), GLTF/GLB and 3MF (our own, in
  export.rs, CRC32/base64/stored-zip helpers with tests), Y-up rotation, per-part files, hidden
  parts only when asked. Not available yet (message shown): PARASOLID, ACIS, PVZ, JT, RHINO; STEP
  is always AP214 and in mm (kernel `write_step` has no schema/unit option); GLTF "Compress"
  (Draco) and "Custom" resolution fields, Options other than Download are not implemented.
  `os_ui::modal` titles are now 18 px `#666` regular like live (all Onshape modals sampled).
  final-export-dialog.png.

## Round 7 (lead's list after QA3; 02:00-03:30)

- **Toolbar collapse** (live part-studio/220, sketch/420, dom/toolbar-narrow-900*.txt): no
  overflow button; when the tools don't fit, every group after Sketch becomes one split tool
  (live faces: Extrude, Hole, Linear pattern, Boolean, Modify fillet, Plane, Frame, Sheet metal
  model) listing all of its tools; the sketch toolbar collapses to Line ▾ (drawing tools, Use,
  Intersection, Construction), Sketch fillet ▾, Dimension, Coincident ▾. TOOLBAR now follows the
  live dump (dom/toolbar-partstudio.txt + dropdowns-partstudio.json). "Search tools…" follows the
  last tool; ⊞ "Add custom features" ends the toolbar (own icon).
- **Feature preview**: bodies a feature creates show see-through slate (`FEATURE_PREVIEW`
  `#4f6577` at 55%) while its dialog is open (live part-studio/230); the "before" model is now
  computed for every dialog. **Depth arrow**: live's screen-sized white arrow with a hollow head
  (gray outline, blue when hovered) at the end of the length; hit area follows it
  (`Viewport::manipulators` carry a glyph length).
- **Hole**: title/list name "Ø 5 mm ↧ 12 mm" (`interaction::display_name`), 26 px icons
  instead of the Diameter / Depth / Tip angle labels (own artwork hole-diameter, hole-depth,
  hole-drill-angle), tip angle options "118 deg", "135 deg". final-hole-dialog.png.
- **Variable** (fs-std variable.fs ids): Assigned | Measured | From import; Measured Distance
  (min/max, axis components in a "› Axis measurements" group, custom direction), Length, Diameter
  (radius option) are evaluated (`regen::measure::entity_distance/total_length/entity_diameter`)
  and the read-only Distance/Along X/Y/Z values are shown (`FeatureDialog::set_computed`);
  default value 0; Name has focus on open; no preview slider (live variable-01). From import
  (CSV tables) is laid out but its regen reports "not available yet" (needs file-tab access).
  New hints: `GROUP_LABEL` (collapsible title without a checkbox), INITIAL_FOCUS on text.
- **View animations** 500 ms (live ≈0.5 s; QA's 1.2 s was llvmpipe frame latency).
- **Section caps rebuilt** (renderer): per-part stencil counting (front faces −1, back faces +1 of
  the kept surfaces), caps only where the count is odd, at the plane depth; faces lying on the
  plane count as cut; hatch = 1 px lines at 45° in the plane's own axes, 11 px apart, half the cap
  color (live part-studio/74). Fixes caps painted over uncut side faces and caps of parts lying
  wholly behind the cut. Depth format is now depth+stencil (Depth32FloatStencil8, else
  Depth24PlusStencil8; AO samples a depth-only view). New test
  `section_leaves_uncut_faces_unhatched`; `section_caps_cover_the_cut` now catches any red tint.
- **Rollback drop line**: hollow blue circle at its left end while dragging.
- **Pixel pass vs part-studio/00 at 1919x1079** (Xvfb :101 now 1920x1080): parts list takes the
  lower 35% of the panel (live), filter box 30 px with italic placeholder and thin funnel, bottom
  tabs fixed 198 px / 3 px apart, help button = filled "?" + solid caret, view cube redrawn
  bevelled (rounded white faces in 72% of each side, corner caps, live-size roll arcs that only
  take clicks on their band), camera menu caret solid.

## Round 8 (05:00-, pixel pass continued)

- **View cube rebuilt from Onshape's own geometry** (webgl bundle, view manipulator; constants
  VIEW_CUBE_*): widget 150 px, cube half edge 29.5 px centred in it; each near face a white
  rounded square of 0.75 of the edge, each far face a 0.97 square in `#cedbe5` at 60% seen
  through the gaps (back-face culling makes that frame and the white lines at the far edges); a
  white disk (radius 0.24 of the half edge) under each near corner; corner radius 0.2 of the edge.
  Only faces and corners are clickable (no edge cells); a far face's frame selects that face. A
  corner goes to its trimetric view (30° steps about Z, 30° above/below). Arrows are drawn shapes:
  triangles 15 x 9.75 px with tips on the widget edge, roll arcs centred on the cube at radius
  67.5 px, 6 px thick (45°-72° and 108°-135°) with heads at the outer ends; hover `#90cef1` at 60%;
  no tooltips (they're GL meshes in Onshape). Triad from 1 px outside the back-bottom-left corner,
  2 half-edges long, hidden parts at 30%; labels past the tip, fading out end-on (8° to 2°), 30%
  when behind the cube. Arrows act through the same hit test; while hovered they live in the hover
  slot as pseudo cells (`Arrow::cell`), so `CubeHooks` didn't change.
- **Standard views fixed** (render/camera.rs): Onshape opens every graphics area in **Trimetric**
  (30° right of front, 30° up; `getDefaultStandardView`), which is what part-studio/00 shows.
  Isometric is the true (-1, 1, -1) diagonal, Dimetric 45° right and 30° up. `Camera::default()`
  and the assembly's first fit now use Trimetric (assembly_view.rs line ~598, one line).

- **Left rail panels** (doc_panels.rs; live document/rail-panel-*.png + dom/rail-panel-*.txt):
  rail order Versions and history, Create version…, Document notes, Performance (Comments, AI
  Advisor are cloud-only; Release drafts stays last). **Document notes**: header ↻ Refresh notes,
  ✎ Edit notes, ✕; empty state = 140 px pale notepad + "Instructions, description, or notes for
  this document"; edit mode Apply · Cancel · Show preview with a borderless textarea (Onshape's
  placeholder); notes are Markdown (headings, lists, **bold**, *italic*) stored in the store meta
  key `notes`; gray footer with the help link. **Performance**: Tab performance (Total
  regeneration time + stopwatch = "Show regeneration times", › Features (n) table by feature type),
  Tab graphics (› Primitives: faces, edges, vertices, sketch entities, planes), Document
  performance (Tabs table), System (› Browser and platform: noshape, version, platform, GL vendor =
  wgpu backend, GL renderer = adapter, red ⓘ for software renderers; › WebGL and extensions ✓).
  Tables: 250 px, sortable by either column (Count ▾ by default). A software renderer also puts
  the red badge on the rail icon ("Performance errors detected"). Connection is cloud-only.
  Open left panels (all of them) now run the window's full height with the gray footer level
  with the tab bar; the tab bar starts right of the panel and the Tab manager cell ends the rail
  (live). Panel background `#f5f5f5`.
  **Where used** is not done: Onshape's is the paid, version-based cross-document search
  (free plan only shows a lock); a local version (assemblies/drawings using this tab's parts)
  needs its own design.

- **Pixel pass vs part-studio/00** (1919x1079): text is grayscale anti-aliased like Chrome
  (`cx.set_text_rendering_mode(Grayscale)` in main.rs; subpixel left colour fringes). Navbar: ☰
  16 px wide, 3 px bars 6 px apart, centred at x 155; name 6 px after it; "Main" on the name's
  baseline 11 px after it. The `{✓}` FeatureScript notices button is in every document (own icons
  fs-notices / fs-notices-error). No Appearance button in an empty Part Studio (parts or sketches
  bring it). Toolbar dropdown carets are a 10x5 px, ~2 px caret (tool-caret.svg). The Default
  geometry guide line ends in a 9 px tick.
- **Remove / Intersect previews** (live 234-237, final-remove-preview.png): a tool that reaches no
  part (every part unchanged) shows translucent dark red (90%) with the blue info toast "Selected
  tools and targets do not intersect." (cleared when it cuts or the dialog closes); when it cuts,
  the cut is live and the tool shows gray at 60%; the tool has no edges. The Extrude depth arrow
  now also appears for a picked part face (not only sketch regions).
- Sketch start (live sketch/01, final-sketch-start.png): a filled single-pick field with no
  empty selection field after it lets go (the Sketch plane field is no longer active once "Top
  plane" is picked); the evaluation tools (Measure, Analysis, Mass properties) stay at the bottom
  right while a sketch or feature dialog is open (live 01, 230, 234).
- For the sketch agent (sketch_mode.rs, not changed): live 01 shows **no tool active** after the
  plane is picked (sketch-behaviour.md says Line starts active), and the empty sketch's frame is
  about 2.3x ours at the default zoom (ours: half height = 150 px; "Sketch 1" at its top-left).
- Open: the New preview's shading differs (live top face darker than the lit side, ours the
  other way); needs a preview-specific lighting in the renderer, not just a colour.

- **Feature preview lighting** (renderer): `Instance::preview()` (params.y bit 4, value 16) shades flat
  with ambient 0.5 + 0.74 x a light from the viewer's right and slightly below, no highlight, so a
  box's top and front read alike and the right side lighter (live 230). Preview bodies (what the
  open dialog creates; not Remove/Intersect tools) come from `PartStudioView::preview_body_ids` via
  `SceneOptions::preview_bodies`; color `FEATURE_PREVIEW` #92aec5 at 70%. Offscreen test
  `feature_preview_shading_matches_onshape` (top/front (129,145,158) vs live (132,148,158), right
  (160,183,203) vs (160,186,202)). final-feature-preview.png.
- **Feature dialogs vs live/dialogs (27 dialogs; final-dialog-*.png)**: rows sit flush with the
  dialog edge like live (tabs, checkboxes, group titles, guide lines -2 px); number fields: 14 px
  value, field 5 px lower (underline on the row's bottom line), label without right padding,
  tolerance rows 3 px left; labelled selects: label on the value's baseline; closed selects: text
  and caret 2 px lower, underline 1 px lower; query fields 27 px, placeholder `#555`, no gap before
  their icon, a 4 px margin when a query field ends the dialog; expander chevrons 12x6 thin
  (expander-down/right.svg); help disc 12 px `#a0a0a0`; dialog shadow blur halved (GPUI blurs twice
  as far as CSS); "= value" hints only for expressions; dialog angles show "3 deg"; a hidden param
  between a row and its flip no longer orphans the flip (Plane's Opposite direction is the arrow on
  Offset distance again); flip buttons swap their icon (flip-direction(-opposite),
  flip-circular(-opposite)) instead of a pressed background; a filled single-pick field lets go
  when nothing is left to pick; driven arrays (Loft's Profiles) take the initial focus; mate
  connector button redrawn as a double ring.
- For other owners: **Mate connector** dialog (spec): live has no mate-connector button on Origin
  entity and shows Flip primary axis / Reorient secondary axis as one row of two icon buttons
  instead of our "☐ Flip primary axis" and "+X" rows. **Assign material** dialog is 179 px tall vs
  live 368 (empty body + help icon missing), and isn't in tool search.

- **Display modes** (final-display-modes.png vs live 60-63): Unshaded draws the faces unlit white
  (they were lit gray). **Section view** (final-section-view.png vs live 70): opened from the camera
  menu with nothing selected it starts empty with the info toast "Select a plane, planar face,
  cylinder, or mate connector." (cleared by the first plane / closing); clicking a default plane in
  the feature list adds it as a section plane. **Mass properties** with a part selected matches
  live 80 (final-mass.png). Labelled selects wrap like live: a select is its value + 22 px and at
  least ~90 px wide (Loft's end conditions wrap, Hole's Start plane and Helix's Start condition
  don't).

- **Right-click menus vs live menus/rmb-ps-*** (final-context-menu.png, final-parts-menu.png):
  context menus have no inner padding (rows/separators edge to edge), text 36 px in with icons and
  17 px without, 10 px right padding, 120 px minimum width, shadow blur halved; submenu ▸ 5 px `#666`
  9 px from the edge, submenus sit on the parent's border and open left only without 200 px of room;
  menus open 2 px under the pointer and flip above when they don't fit (GPUI's SwitchAnchor instead
  of snapping). Content: edge menu without Section view… and with icon-less Fillet…/Chamfer…; planes
  get Size to Part Studio and the eye-off Hide icon; sketch region: no icon on Edit Sketch, Hide
  other/all sketches only with other sketches shown, Create Drawing icon; no Export as DXF/DWG… with
  several faces; no Export as SVG… on sketch rows; parts menu icons (Copy, Create Drawing,
  Hide/Show, Delete); Section view icon redrawn; Add appearance to face uses add-appearance.
  "Add comment" (comments are cloud) and the Release "Upgrade" pill (subscriptions) stay out.
- **Parts list**: rows span the list (content 18 px in), 24 px tall, 5 px short of the edge, eye
  9 px in; double-click renames; while renaming no hover tint or eye, the input fills the row.
  Tree carets (Parts / Default geometry headers) are the thin 12x6 `#666` chevron. **Rollback bar**:
  x 45-234, `#c8d9e2` on hover, tooltip "Rollback bar".

- **renderer.rs `draw_lists`** takes `DrawMode { pick, section, outline }` instead of three bools
  (clippy too_many_arguments; the outline flag is psb's part-outline pass, noted in
  part-studio-behaviour.md). New render flags go into `DrawMode`.

- **Mate connector dialog** (final-dialog-mate-connector.png vs live dialogs/mate-connector-01):
  new hints `PRIMARY_AXIS` (1<<21) and `MATE_CONNECTOR_AXIS_TYPE` (1<<22) in model params.rs
  (FeatureScript UIHint names); flipPrimary is a PRIMARY_AXIS bool and secondaryAxisType a
  MATE_CONNECTOR_AXIS_TYPE enum, drawn as one 28 px row of two 26 px icon buttons
  (`FeatureDialog::render_axis_buttons`: the flip arrow shows the direction; "Reorient secondary
  axis" steps +X, +Y, -X, -Y); Origin entity has NO_MATE_CONNECTOR_BUTTON. Own icon
  realign-mate-dialog.svg.

- **Selection count only in the cursor image** (lead/psb): removed the Part Studio's overlay digit
  (`render_selection_badge` + `badge_at`) and the viewport's fallback `count_badge`. The count is
  drawn only by the platform cursor (cursor.rs: X11 RENDER cursor, Wayland via the patched pointer,
  macOS/Windows native); where a platform can't set it, no count shows.

## Round 9 (2026-10-19 12:10-, new "ui" agent after the reboot; /tmp was wiped)

- Verified done and compiling (from the previous agent): `DrawMode` for `draw_lists` (clippy clean in
  crates/render), `Viewport::pointer_inside()` removed (field still used), the selection count only in
  the cursor image (no overlay digit / `count_badge` anywhere), Mate connector one-row axis buttons,
  preview lighting + `feature_preview_shading_matches_onshape` (all 13 offscreen tests pass).
- Mate connector axis buttons sit 2 px lower (row `items_end`), matching live mate-connector-01.
  final-dialog-mate-connector.png (ours | live).
- Icons redrawn after Onshape's art: `menu-section-view.svg` = the cut hollow cylinder with hatched
  cut faces (icons-master__lui-section-view@3x / section-view-icon@1x), `add-appearance.svg` = blue +
  with the flat swatch fan over a 3-cell bar (add-appearance-button@1x), both sized to fill live's
  pixels at the 16 px menu slot. final-context-menu.png (ours | live rmb-ps-face).
- Sketch toolbar: split tools are Onshape's split tool (`split_tool_button` + caret, no gap, group
  hover; dropdown = `menu_tool_item` rows with shortcut chips), pitch 52 like live
  (dom/toolbar-sketch.txt); no separator between Text and Use (live).
- Sketch frame name ("Sketch 1") is drawn in the sketch plane like the plane names (shared helpers
  `part_studio::in_plane_text` + `paint_paths`, used by the plane labels too), ascender 2.5% of the
  frame height (live sketch/01 skewed, sketch/03 normal view).
- Sketch frame: an empty sketch's frame is 202 x 113.7 mm (`EMPTY_SKETCH_BOX_HALF_HEIGHT` 0.05685,
  live sketch/03 measured line to line; skb's file, constant only).
- Corner rectangle (sketch_mode.rs, surgical): the rubber band's values and the on-the-fly boxes sit on
  the two edges through the **first** corner (live sketch/05-06); the rubber band shows all four
  corner dots; preview lines 2 px. On-the-fly boxes (part_studio/sketch.rs): only the active one is a
  box (24 px, white 90%, 1 px `#788188`, gray 14 px text); the others are plain 13 px `#449ccd`
  values. Edited-sketch vertices are 7 px dots (scene_builder.rs); the origin stays on top of the
  edge-on planes while sketching. final-sketch-rectangle.png (ours left, live right; top = rubber
  band, bottom = after the second click).
- Right panels (config_panel.rs, live 182/183/164): 1 px `#d7d7d7` divider + 5 px white gutter + 712 px
  pane (`#ccc` top line), so the pane starts at x 1207 like live; strip buttons white while no panel
  is open, `#eaeaea` with one open and the open one white without a right border (no blue bar); tabs
  share the width by label (`flex_grow`). Configurations: 44 px filter row (funnel, 30 px box, three
  window-style toggles `cfg-filter-list/checkbox/variable.svg` 36 px apart, ⋯). Left Configurations
  box: 35x28 + 20x28 halves, white while the panel is open, icon `config-show.svg` redrawn after live
  (blue grid + eye), 19 px under the last input. Variable table: flush with the pane, the first header
  2 px under the underline, gray only on the header cells, the italic "Name" row after the Part
  Studio's variables too, ⋯ icon. final-configurations-list.png (ours top, live bottom),
  final-variable-table.png.
- Feature dialog value rows (feature_dialog.rs): labels start at the dialog edge with 5 px padding on
  both sides (live dom/dialog-variable, dialog-thicken: label x = dialog x, field = label end).
  Verify Extrude/Thicken against live after the next build (the old note said "no right padding").
- For the lead / psb: the left Configurations box button is "Modify visibility conditions" in live
  (tooltip and action); ours opens the Configurations panel. (The Variable dialog "Enter doesn't close" note
  is withdrawn: I pressed Enter once, which applies the value; the second Enter commits, as designed.)
- Feature dialog value rows, verified: Variable (Name/Value/Description) and Extrude Depth (label 31,
  field 68-170, ± at 176.5) now land on live's x positions; the tolerance toggle has 4 px after it
  (live 5 px gap before the right column). Rows are still 1-2 px lower than live (open).
- Region hover fixes (scene_builder.rs, part_studio/mod.rs): hover/selection outlines of sketch
  regions are drawn back on the plane (the region mesh is 1 µm above it), which made the edited
  sketch's own curves count as occluded and turn `#4040ff`; while a drawing tool is active, regions
  under the cursor don't light up (live sketch/08-09).
- Circle tool preview (sketch_mode.rs, surgical): circle + center point, no dashed radius line, no
  point at the cursor (live sketch/09). final-sketch-circle.png.
- Toolbar tooltips re-verified against live 10/11 (final-tooltips.png).
- Open (small): the Front/Right planes seen edge-on in a sketch's normal view read darker in live
  (both outline edges blend; our second edge fails the depth test); live's "Top" plane label looks
  bolder (Onshape's font); dimension preview text is 13 px in live's font, ours slightly narrower.

### Side-by-side shots of this round (/tmp/ui-shots, ours | live unless noted)

final-context-menu.png (face menu with the new Section view / Add appearance icons),
final-dialog-mate-connector.png, final-sketch-start.png (ours top, live bottom),
final-sketch-rectangle.png, final-sketch-circle.png, final-configurations-list.png (ours top),
final-variable-table.png (ours top), final-dialog-variable.png, final-feature-dialog.png (Extrude),
final-tooltips.png, final-sketch-dimension.png (ours left).

- **os_menu in every window** (lead/nav request): the open menu remembers its window
  (`OsMenuHost` holds the `WindowId`); every app window is opened with `os_menu::open_window`
  (files.rs, start_window.rs, preferences_window.rs), which wraps the content in a `MenuLayer` view
  that draws that window's menu; the old `render_host` calls in workspace.rs / start_window.rs are
  gone. `os_menu::content_view(window, cx)` returns a window's content view (used by workspace.rs
  and release_add_source.rs to find the Workspace; they used `Root::view()` before). A new window
  must use `os_menu::open_window`, not `gpui_kit::open_window`. New `OsMenu::native_select(width)`
  (browser `<select>` lists); nav's Mouse controls preset select uses it (navigation.md).
- Dimension tool (sketch): the dimension being placed is drawn like a placed one (black, line broken
  around a 16 px value at the cursor; sketch_mode.rs `dim_preview`, sketch.rs label); arrowheads
  16 x 7 px like live (all dimensions); the value box is centered on the label (live 13).
  final-sketch-dimension.png (ours left: placing, box, applied). Open: live highlights the measured
  edge while placing; live 15 shows the applied dimension unselected while skb keeps it selected
  (their sketch/148 evidence); the value in the box isn't shown selected/right-aligned like live.
- Clippy (lead): `os_menu::OpenMenu` alias for the host tuple; offscreen.rs depth-sample test uses
  `Range::contains`. renderer.rs's "very complex type" at :201 was already gone when I got to it (its
  author changed the depth readback). Remaining app warnings (19 Oct 18:30) are other agents':
  history_panel.rs, numbering_prefs.rs, part_studio/behaviors.rs:192 (`consumed_lines`, psb),
  part_studio/in_context.rs, session.rs, tab_manager.rs.
- Tools: /tmp/ui-tools/build.sh (shared build + copy to /tmp/ui/noshape), start.sh [file|--demo]
  (starts on :101 and resizes the window to 1919x1079 like the live shots), x.sh <name> [xdotool…]
  (screenshot to /tmp/ui/<name>.png). Icon scratch work in /tmp/ui/icon.
- Note: the demo document's toolbar is collapsed at 1919 px because its custom features don't fit
  (Onshape collapses the same way); compare toolbars on a new document.

## Round 10 (2026-10-19 evening, lead's follow-ups)

- **Dialog rows = live**: Variable dialog underlines at live's 188-189 / 215 / 241, Extrude at 194 /
  225 / 253 (empty query field 26 px with its border; non-tolerance value rows 26 px via a 4 px field
  top padding; tolerance rows keep 5). The focused field's 2 px blue underline grows upward (1 px
  border + 1 px inset line) like live's inset shadow.
- **Dimension tool**: the measured entity shows selected (amber) while the dimension is placed, not
  the yellow "related" glow (`SketchEditor::tool_picks`, sketch.rs `sketch_overlay`). The value box's
  text *is* selected; it didn't show on Xvfb because GPUI paints input selections only in active
  windows (start.sh now gives the window X focus). Selection colors: see the gpui-base patch below.
- **Hidden-line pass** (renderer.rs): hidden-only lines/points are drawn right after the meshes, so
  only faces hide them; overlays on top of the same curves (hover outlines, accents, dimension
  lines) no longer turn underconstrained curves `#4040ff`.
- **Edge-on planes** (theme `PLANE_OUTLINE` `#4191c8` at `PLANE_OUTLINE_ALPHA` 0.6, linear blend):
  one edge reads like live on white and on the plane fill, the two edges of a plane seen edge-on add
  up to (121,167,209) vs live (121,161,204).
- **Input selection = Chrome's** (lead/skb): vendored, patched **gpui-base 0.7.3** (vendor/gpui-base,
  `[patch.crates-io]`, Cargo.lock drops its registry source/checksum): new global
  `gpui_base::input::PlainSelectionColors { background, foreground }`; for single-line, unmasked,
  non-code inputs with a visible selection the element fills it with `background` and recolors the
  selected text runs with `foreground` (`plain_selection_colors`, `recolor_runs`, prepaint field
  `plain_selection`; every change marked NOSHAPE PATCH in src/input/mod.rs and
  src/input/base/element.rs). main.rs sets `#3367d1` / white; code editors keep `t.selection`.
  When gpui-base is upgraded, re-apply those hunks.
- **Onshape tooltips placed like live** (ui.rs `TipSide`, `PlacedTooltip`, `with_tip`, `.tip()` via
  `TipExt`): feature-list rows and Default geometry rows (beside the row: error, else
  "Extrude 1 <- Sketch 1", else the name; was only on selected rows), left rail (right of the
  button; "Versions and history" without the shortcut), right strip and camera menu button (left),
  Measure / Analysis tools / "Display mass and section properties" (above), rollback bar (under,
  centered). Feature-list rows span x 40-239 like live (5 px right margin; Default geometry indent
  moved into the rows; eye 19 px from the row end). Right strip centered 8 px above the viewport
  middle (live 488 for 4 buttons).
- **Tab hover card** (tab_thumbs.rs, new): an inactive Part Studio / Assembly / Drawing tab shows a
  314 x 184 card above it with the element's picture (rendered off screen per content hash,
  `TabThumbs` owned by the Workspace), notch at the tab, Onshape's gray cube for an empty Assembly
  (`tab-card-empty-assembly.svg`). An empty Part Studio stays blank until the requested capture
  (REQUESTS.md). (tab_manager.rs, ui2's, keeps its own thumbnails; both could share `TabThumbs`.)
  final-hover-tooltips.png (ours left, live right: row, tab card, rail, strip, rollback, camera,
  measure).

- **Feature-list collapse toggle** (lead/ui2): centered 8 px above the viewport middle like the right
  strip (glyph rows 547-562 = live); `treelist.svg` redrawn from live's pixels (chevron, 2 px gray
  bar, four 1 px lines), glyph 2.5 px right of the box center. The assembly uses the same icon.
- **Feature dialog open** (live 570, 94): an invalid open dialog makes its row red (new-feature
  placeholder too) and counts as an error; the Features header shows Onshape's red circle with a
  down arrow (`regen-error.svg`, click: show only errors, tooltip beside it) before the title, which
  truncates to "Features …"; Add folder and Pause regeneration are grayed and inert while a dialog is
  open. `os_checkbox` boxes 8 px from the row top like live (were 7). final-fillet-new.png.

## Round 11 (2026-10-20, cursor parity, lead's priority)

- **Inventory**: docs/parity/cursors.md (from the bundle: the viewer's cursor stack, the image
  cursors with their hot spots and mac/Windows variants, the CSS cursor rules by control). Live
  capture of the ❓ states requested (REQUESTS.md 20 Oct). Onshape's cursor files can be looked at
  for reference in /tmp/ui/cursor-ref (fetched from cad.onshape.com, never copied into the repo).
- **Live answers** (REQUESTS.md 20 Oct, live/dom/cursors.txt + cursors-*.json) are folded into
  cursors.md: navigation never changes the cursor; manipulators keep the current cursor; rollback
  bar and feature rows `default`; disabled dialog ✓ `default`; splitters `row-resize` /
  `col-resize`; drawing tools a 15 px crosshair (Centermark a boxed one), hot 7,7. Follow-up asked
  (which selection the count cursor counts while a feature dialog is open).
- **Infrastructure** (cursor.rs): `AppCursor` = `Count(1..=6)` (6 = "5+"), `Crosshair` (15 px,
  white caps, hot 7.5 → Chrome truncates to 7,7, and so do we), `CrosshairBox` (drawing
  Centermark), `Confirmation` (hot 1,1), `RemoveSelection`, `SnapMate`, and CSS standard `Move`
  (X11/Wayland: `all-scroll`/`fleur`, as Chrome; Windows `IDC_SIZEALL`), `ZoomIn`, `NoDrop`. Art is
  ours. Backends: X11 (RENDER image or theme cursor, `cursor_x11::theme_names`), Wayland (patched
  gpui-pre-linux `set_named_cursor` / `set_custom_cursor`), macOS (`NSCursor`), Windows (`HCURSOR` /
  `IDC_*`). `CursorSync` (cursor.rs) applies an app cursor for a view other than the viewport
  (drawing sheet, workspace navbar).
- **Vendored GPUI patches (this round)**: gpui-pre-linux X11 `set_cursor_style` treats a window
  whose cursor was never set (it showed the root window's X cursor) and a cursor the app set
  (`custom_cursor::invalidate_style`) as "unset", so GPUI's own style is applied again;
  gpui-pre `Window::hovered_cursor_style()` (the style the hovered elements asked for).
- **Viewport**: `tool_cursor` (top of Onshape's stack) wins over the selection count;
  `overlay_cursor` applies over GPUI overlays drawn on the graphics area (dimension value `move`,
  analysis color map crosshair); Zoom to window `zoom-in`; Ctrl during a box select
  remove-selection (Part Studio box and, new, the sketch box, which now also deselects with Ctrl).
  Clearing our cursor refreshes the window so GPUI puts its own back.
- **Wired and read back on X11** (/tmp/ui-tools/curtest.sh, curtest2.sh, curtest3.sh;
  getcursor.py): arrow on a fresh window; sketch tools crosshair 15 px hot 7,7; count 1 and 5+
  hot 3,3; pattern / transform / offset "click to accept" confirmation hot 1,1 (count cursor
  while something is pre-highlighted); dimension value all-scroll; Zoom to window zoom-in; Ctrl box
  remove-selection; drawing Dimension crosshair and Centermark boxed crosshair hot 7,7, no tool
  arrow. Also: feature dialog title and Create selection title grab; analysis legend grab /
  grabbing, ticks ew-resize; features/parts splitter row-resize; rollback bar and disabled ✓ the
  arrow; navbar shows the count cursor over its empty area (Onshape sets it on `.navbar`).
- Not yet: the count cursor while a feature dialog is open (waiting for the live follow-up),
  the context-menu layer's count cursor, no left-panel / Configurations-block splitters exist in
  noshape to give `col-resize` / `row-resize`, disabled-control `not-allowed` audit (feature dialog
  params, `.disabled-tool`), see the ❌ rows in cursors.md.

## Round 12 (2026-10-20, lead: input hit area; qa4: button cursors, notices layout)

- **Inputs: the whole box is the hit target** (vendor/gpui-base, NOSHAPE PATCH,
  `src/input/base/frame_hit.rs`, plus `base/mod.rs` probes and `base/state.rs` `FrameHitArea` /
  `press`). The kit frame (`InputBase`) publishes its padding box while its children prepaint; the
  text element claims the outermost frame; the frame inserts a hitbox over itself in its own
  content mask (the text element clips its overflow, and hitboxes are clipped to the mask they are
  made in), asks for the I-beam before its children paint (a clear button keeps its cursor), and
  forwards presses outside the text element: focus, caret at the nearest position (`press`, which
  skips the "default prevented" bail-out because the frame's own focus handling marks the event),
  drag-select (window-wide already), right-button release for the context menu.
  App boxes drawn around an `Input::new(..).appearance(false)`: wrap the box in
  `gpui_kit::base::input::input_frame(..)`. Done for 38 one-line sites (script
  /tmp/ui-tools/wrapframe.py) plus the feature-list filter, the feature dialog value field, the
  sketch dimension box and the Variable Studio cells.
- **Buttons: pointer when clickable, not-allowed when disabled** (vendored gpui-component 0.7.3,
  NOSHAPE PATCH in `src/button/button.rs`; an app `.cursor(..)` still wins). Arrow overrides where
  Onshape keeps it: feature dialog ✓/✗ and panel-dialog header ✓/✗ (the `dialog-ok` /
  `dialog-cancel` icon buttons; not the sketch dimension box's, which are pointer live). Also:
  Undo/Redo with nothing to do `not-allowed`, active tab default, navbar name and workspace label
  I-beam, expander chevrons and the filter funnel pointer. Read back over the controls of
  live/dom/cursors-controls.json (/tmp/ui-tools/cursweep.sh): all match.
- **Features / parts split** is now a share of the list panel (35% until dragged), so the
  FeatureScript notices panel no longer squeezes the feature tree to nothing.
- Open: the funnel's filter dropdown (Onshape opens a feature-type filter menu) isn't
  implemented.

## Round 13 (2026-10-20 evening, user's caret bug, tab bar)

- **Caret in empty inputs** (user, Retina Mac): the caret sat a few px right of the placeholder's
  first glyph. Cause: about 16 inputs drew their own italic placeholder as an absolute overlay at
  Onshape's text position, while the kit input's real text origin was further in (the kit frame keeps
  its own `input_px` padding, 4–12 px, even with `appearance(false)`). **Rule now: never draw a
  placeholder over an input.** Give the state its placeholder (`InputState::placeholder(..)` at
  creation, or `ui::input_with_placeholder(st, text, cx)` / `ui::ensure_placeholder`); the input
  draws it from the same origin as typed text and the caret, in the app-wide
  `gpui_base::input::PlaceholderStyle` (NOSHAPE PATCH: italic, #999, set in main.rs; Onshape's
  `input::placeholder { font-style: italic; font-weight: 300 }`, we ship no light Roboto face).
  Place the text origin with `.pl(px(..))` on the `Input` (it overrides the kit padding). Don't put
  `.italic()` on an input or its box: typed text is upright in Onshape. Converted: Part Studio and
  assembly filters, Variable table / Configurations "Name" rows and "Filter inputs", Search tools
  (Part Studio + assembly), New view name, Insert dialog searches, GD&T upper/lower text, select-item
  URL / search; italic removed from the documents search, advanced search, tab manager, FS import,
  FS notices, history search and the Variable Studio add row. Crops before/after at scale 1 and 2
  (:113, GPUI_X11_SCALE_FACTOR=2): /tmp/ui-shots/final-caret-1x.png, final-caret-2x.png
  (/tmp/ui-tools/caretcrops.sh, caretsheet.py).
- **Text insets checked against Onshape** (live dom/caret-inputs.json: computed padding/border and the
  caret's x; ours read back at scale 1 and 2, caret column minus the box's left border, logical px):
  Part Studio filter 6 / 6 (border 1 + padding 5; ours box border 1 + px 1 + `pl(4)`); Variable table
  Name cell 7 / 7 (cell padding 0, input padding 6; ours `pl(4)` after the cell's `px(2)`); Variable
  Studio cell 6.5 / 7 (Onshape's table sits on a half pixel); documents search 13 / 13 (border 2 +
  padding 11; ours `pl(12)`); feature-dialog values (`.os-param-number/-text`, padding 3 at the sides):
  number text ends 3 / 3 px from the right edge, the right-aligned caret 1 px further in than Chrome's
  (the kit keeps it inside the text bounds). Caret width 1 px like Chrome (NOSHAPE PATCH
  `CURSOR_WIDTH`). Open: Onshape's input text is 13.33 px (ours 14 in the filter / Variable Studio) and
  its placeholder #666 weight 300 (we ship no light Roboto; ours #999 regular).
- Scripts now live in /root/ui-scratch/tools (caretcrops.sh, caretnum.py, caretsheet.py, start.sh,
  start2x.sh, build.sh, retry.sh); /tmp is wiped on reboot.
- **Tab bar scroll buttons** (workspace.rs `render_tab_scroll_buttons`, `scroll_tabs`): ‹ › 24x30 at
  the right end only on overflow, dimmed at the ends, a press scrolls a page, a right press to the
  first/last tab, tooltips as live; the active tab scrolls into view on activation and when the Tab
  manager opens; the wheel already scrolls the strip (GPUI maps vertical wheel to x). Not yet
  verified at 1280 px.
- **"Missed clicks" in input text** (qa4): not lost; at startup under load the app processes the
  press and the keys late (a capture 0.6 s after typing missed the text, 3 s later it was there,
  8/8). qa4's latency work addresses it.
- Feature Studio toolbar clipping at 1280/1440: waiting for the live capture (REQUESTS.md, 20 Oct).

## Round 14 (2026-10-20 night, typography, tab bar, FS toolbar, select style)

- **Placeholders**: Roboto Light Italic bundled (assets/fonts/Roboto-LightItalic.ttf, v2.138 hinted, same
  source as the other faces, Apache 2.0); `PlaceholderStyle` = italic, weight 300, `#666` (live
  dom/caret-inputs.json).
- **Input text sizes** per live/dom: audit in /root/ui-scratch/input-sizes.md (188 sites: 36 OK, 48
  changed, 104 unknown → REQUESTS.md "input font sizes"). Note: theme rem = 14 px, so the kit presets are
  12.25 px (default/small) and 10.5 px (xsmall); a parent's `text_size` doesn't reach the input: set it on
  the `Input`. Applied with /root/ui-scratch/tools/apply_sizes.py.
- **Tab bar scroll buttons** verified at 1280 (/root/ui-scratch/tools/tabtest.sh): with the tab bar a
  cached slice (qa4), the bar row needs `w_full()` (else the strip sizes to its content and never
  overflows), and a watcher canvas re-renders the slice when overflow / ends change after layout.
- **Feature Studio toolbar** (feature_studio.rs `ToolbarFit`): fit-to-width stages like live (3 full; 2
  snippets → "<last snippet> ▾" split + Import ▾; 1 New feature icon-only; 0 Undo ▾), measured from the
  last layout with probes, re-rendered on resize by a watcher; clipped below stage 0. Snippet ▾ = flat list
  of all 70 snippets, a pick inserts and becomes the face (config_dir/feature_studio_snippet.txt); Undo ▾ =
  Undo / Redo with key chips; Import ▾ = Import / Format Studio. Monitor a fixed 247 px (live). Onshape's
  breakpoints also count the Co-complete toggle (an AI Labs feature, not in noshape).
- **Native select** (`os_ui::select_field`, a Bootstrap form-control `<select>`): the list is Chrome's
  (`os_menu::native_select`, highlight `#1967d2`, was `#1e63d0`), the field gets the 2 px `#3e94ff` ring
  while open (`OsMenu::focus_ring`, drawn by the menu host), caret `#777`. Checked against live 182.
- Tools: /root/ui-scratch/tools (a subagent once deleted that folder while cleaning up: tell subagents not
  to delete anything outside their own files).

## Round 15 (2026-10-20 night, cursor rows, live input fonts, splitters)

- **Cursor rows** (docs/parity/cursors.md, live behaviours.md "Remaining cursor rows"):
  - `CursorStyle::Move` (all-scroll / fleur / size_all; macOS `_moveCursor`) and `Wait` added to vendored
    gpui-pre and its platforms.
  - Dimension box: a □□□ drag strip (`move`) moves the box (`ui.dim_box_offset`, reset per box).
  - Viewport: while another element's drag runs over the graphics (a dialog title, the dimension strip),
    the viewport drops hover, tool moves and its own cursor (`viewport.rs on_mouse_move`, `has_active_drag`).
  - Feature dialog count cursor = picks in the dialog's params (`count_picks`). The snap-mate cursor
    shows only while a snap target exists.
  - Panel dialogs move by their title: `ui::panel_title(key, title)` makes the drag area (`grab`);
    `ui::PanelOffsets` (`ui.panels` in both studios) holds the offsets, dropped when the panel closes;
    `ui::moved_panel` / `ui::nudged_panel` place them.
  - Feature dialog resize = the 3 px right edge, width only (`FeatureDialog::width`, `ui.dialog_width`),
    as live; the corner grip is gone.
  - Splitters:
    - left panel: `ui::left_splitter` (9 px into the graphics, ≥ 206, persisted
      config_dir/left_panel_width.txt; `ui::left_panel_w()` replaces `LEFT_PANEL_W` everywhere);
    - right panels: `ui::right_resizable(panel, inset)` (4 px, ≥ 235, persisted right_panel_width.txt;
      `ui::right_panel_w()` in the Part Studio panel, the assembly panes and `dock_w`);
    - Configurations block: `cfg-box-splitter`, `ui.cfg_box_h`.
  - Configurations panel: drag an input header to reorder (`CfgInputDrag`, a `#1651b0` pill, `grabbing`
    through `set_window_cursor_style`, a drop line with an open circle; undo "Reorder configuration inputs").
  - Properties modal: resizable (right / bottom / corner, ≥ 800×542, centred, persisted
    properties_modal_size.txt).
  - All of the above were checked in the app on :101.
  - Gotcha: GPUI drag values are rebuilt on every render, so keep drag start state in a thread-local or
    the view, not in an `Rc` made at render. The same goes for drag ghosts: GPUI lays them out at min-content
    at pointer − grab offset, and a root margin is lost (pad a wrapper instead).
  - Fixed: the assembly Isolate sliders followed any left drag that crossed them; now only a press that
    started on the slider drags it (`ui.iso_slider_drag`). The other mouse-move sliders (feature dialog
    Final, in-context, explode, graphics prefs) now ignore moves while another element is being dragged.
- **Still open** (cursors.md):
  - row 64: table column grips (asked live);
  - row 67: Onshape's "Reorder items" mode for ordered query lists, which our direct `PickDrag` doesn't
    match (asked live);
  - row 69: the colour Mixer (asked live);
  - the Variable table's inserted-table reorder (needs an order in `VariableStudioRefs`, which changes
    variable precedence: model owner);
  - the Part Studio Customize toolbar (still the old arrow mode, not the assembly's Tools panel);
  - the Part Studio Isolate panel (an old strip, not Onshape's dialog);
  - moving Replace instance and Interference detection.
- **Input fonts from live** (dom/input-fonts.json; mapping /root/ui-scratch/input-fonts-apply.md):
  - 40 sites applied: search insets 6, renames 13.33/14 with inset 4, `os-param-text` cells 13.33 inset 3,
    titles 16/500, on-canvas value boxes (13.33, right-aligned, blue bottom line), and every drawing input's
    text `#555`;
  - per-input placeholder styles: `InputState::placeholder_style` (gpui-base patch) for the auxiliary
    view, GD&T text and New-document labels search.
  - Not done: drawing_palette.rs (another agent was editing it) — its `field` helper needs
    `.text_color(rgb(0x555555)).text_center()`.
  - 65 sites are still unknown.
- **Menus**: `OsMenu::min_width / row_height / text_inset` per menu; documents Create menu = 150 px,
  29.5 px rows, text 48 (live dom/documents-create-menu.txt).
- **Name modal field**: Bootstrap form-control. At rest 1 px `#aaa`, radius 3, 34 px tall. Focused: 2 px
  `#3e94ff` with no glow (the live crop shows none), text kept 11 px in. This is specific to that field, not
  the shared focus style (the Details labels search is 1 px).

## Round 16 (2026-10-20 late, Customize toolbar, Isolate dialog, panel positions)

- **Customize toolbar** is shared by both studios: `toolbar_customize.rs` (live part-studio/340–347,
  dom/customize-toolbar-*.txt).
  - Tool sets are `#f5f5f5`, with a `deferred` 10 px "•••" sled hanging under the bar (cursor `move`;
    drop a sled on a set or on its sled to move the set).
  - Tools take a cover that drags them (a split tool with its dropdown) and keeps clicks off; a dragged
    tool leaves the bar meanwhile (`DRAGGED` thread-local: GPUI drag values aren't readable from views).
  - The 600 px Tools panel: header 43, body `#f5f5f5` (bold empty note, or 3 × 186 px columns of 34 px rows,
    greyed when in the bar), footer 42 (☐ Show items · Save · Reset to default · Cancel, 25 % when idle).
    While a toolbar tool is dragged it is only the red (`#bd3039`) dashed drop zone.
  - Part Studio: `ui.tb_custom`, sets = the 9 groups + ⊞ "Add custom features"; `ToolbarPrefs` gained
    `hide_search` (right-click › Hide / Show "Search tools"). The old arrow mode, "Removed (n)",
    "Shortcut toolbar (S)" and the "Reset toolbar to default" menu item are gone (the S toolbars are in
    Preferences).
  - Assembly: `AsmToolbarPrefs` stays and the old panel and drag types are gone.
  - Both toolbar slices render uncached while customizing (the dragged tool must leave the bar).
- **Part Studio toolbar fit** (`ToolbarFit` in part_studio/mod.rs): collapse when the measured tools
  (natural width, measured when shown in full; `toolbar_full_width` only keys a change) don't fit beside
  "Search tools…". A watcher re-renders when a resize changes the answer. The toolbar row needed
  `w_full().min_w_0()` (it sized to its content and pushed the view 32 px wide).
  - Gap found: our custom-features group (up to 4 buttons + a menu button, before ⊞) differs from live
    641 (one icon + ▾ split after ⊞). Owner: Part Studio toolbar (me): next.
- **Isolate / Make transparent** dialog shared: `isolate_ui.rs` (live behaviours.md "Part Studio
  Isolate…", dom/ui-isolate-*).
  - 220 px, ✕ only; the Expand select with disabled options (`os_ui::select_field_with`).
  - The transparency slider has live's static legend: `#777` to 10 %, then `#3e94ff` → white, inside a
    2 px `#777` ring. The expand slider is a 2 px line; tracks 183 px, 11 px in. Sliders drag only from
    their own press (`*_slider_drag`).
  - Part Studio semantics (`iso_look`): under 10 % light opaque ghosts; from 10 % see-through
    (opacity 0.66 → 0.05 by 90 %); hidden from 95 %.
  - Expand snaps in 10 % steps and joins parts within 0.815 × diagonal × step² of the chosen ones
    (fits live's 20 / 40 / 50 / 70 %).
  - The Part Studio Isolate opens at 0 (ghosts), like live.
- **Panel positions** are now app-wide and persisted (config_dir/panel_positions.json), keyed by panel and
  shared by both studios. `ps-` / `asm-` prefixes are stripped: live remembers a panel's rect per panel id.
- **Kit patch**: `Input::h()` now sizes single-line inputs too (gpui-component input.rs, NOSHAPE PATCH).
  It was multi-line only, so every `.h(px(N))` rendered at 28. This covers the ~40 sites listed in
  theme-studios.md Round 9 at once; `os_ui::input_h` is now equivalent. Still to do: check those sites
  against live heights in the input-size audit.
- **Next (lead's list, in order)**:
  1. table column grips (row 64; live answered: 5 px `.drag-handle`, only that column, min 40, no max,
     scrollbar, persists per table);
  2. Reorder items mode for ordered query lists (row 67; live answered);
  3. colour Mixer (row 69; live answered: 152 SV square crosshair, 50×152 hue strip ns-resize, alpha in
     the Appearance editor);
  4. the custom-features toolbar group;
  5. the input-height audit;
  6. **Dark theme**.
- **Dark theme plan** (REQUESTS.md 20 Oct "DARK THEME"):
  - Ask live for every `--os-*` CSS variable in light and dark, plus per-area computed colours.
  - `theme::Palette` already centralises most colours, but there are about a thousand hard-coded
    `rgb(0x…)` in the app crate. Grep them, map each light value to its `--os-*` token (light values are
    unique enough), and add those tokens to the palette with light/dark values.
  - Replace the literals file by file, with a lint script listing the remaining literals.
  - The Preferences Theme select switches the palette (a global; views re-render) and persists it.
  - Then pixel-check each area against the dark captures.
  - Kit components (gpui-component theme) need their dark colours too: set `Theme` from the palette.
  - Graphics background, view cube and selection colours come from the viewport palette, now in
    noshape_scene (coordinate with perf/api).
  - Estimate: large; best done when the app is quiet (it touches every view).

## Render regression test `plate_edges` (fixed 2026-10-18 ~14:00, for the lead)

- It failed with "13 of 315 edge samples covered by faces", luminance 102-109, after the lighting
  refit in shaders.wgsl `shade()`. Cause: **not** faces winning MSAA samples. The failing samples are
  the same positions that read 84-96 at baseline 75869cc (checked in a private worktree): the
  periodic spots where the 1.5 px line straddles two pixel rows, so no pixel is fully covered and the
  darkest probe is an anti-aliased mix of black and the adjacent surface. The refit makes faces
  brighter on purpose (Onshape's top face reads `#9bc1d9`, luminance ~185; the old shading was
  darker), so those mixes now land just above the absolute limit of 100.
- Fix (crates/render/tests/offscreen.rs): the limit is now relative to the darker surface 5 px on
  either side of the edge, `max(0.7 * beside, 100)`, so 100 is still the floor. A sample the faces
  won reads like that surface and still fails. Verified: with `FACE_DEPTH_BIAS` set to zero, the test
  still fails (27/315, in runs); with the bias, all 5 offscreen tests pass.

## Side-by-side shots for the user (ours | Onshape)

- /tmp/ui-shots/final-context-menu.png: right-click on a part face (with the Select submenu open).
- /tmp/ui-shots/final-feature-dialog.png: new Extrude dialog.
- /tmp/ui-shots/final-sketch-dialog.png: Sketch dialog with its options inside.
- /tmp/ui-shots/final-selection.png: face hover / face selected (ours top, Onshape bottom).
- /tmp/ui-shots/final-cursor.png: selection-count cursor 1 and 2 (ours, Onshape's SVG).
- /tmp/ui-shots/final-cursor-wayland.png: the count cursor under Wayland (sway, grim -c).
- /tmp/ui-shots/final-feature-list.png: suppressed / selected / hover rows.
- /tmp/ui-shots/final-tool-search.png, final-camera-menu.png, final-right-panel.png.
- /tmp/ui-shots/final-measure.png, final-mass.png.
- /tmp/ui-shots/final-appearance.png, final-configurations.png, final-graphics-prefs.png,
  final-graphics-prefs-sketch.png, final-tool-search.png (updated: "fil").
- /tmp/ui-shots/final-tooltips.png (expanded Extrude tooltip), final-sketch-tooltip.png (ours only).
- /tmp/ui-shots/final-dialog-dropdown.png (End type open | Onshape, typed "thr" | Onshape),
  final-fillet-dialog.png, final-hole-dialog.png, final-export-dialog.png,
  final-configurations-table.png, final-configurations-box.png.
- /tmp/ui-shots/final-viewcube.png (4x), final-doc-notes.png, final-performance.png,
  final-remove-preview.png (round 8).
- (older, by the previous agent) /tmp/ui-shots/new-doc-side-by-side.png: new document.

## Measured facts (live 1920x1080 screenshot, viewport 1673x973)

- Default view: **orthographic**, camera 30° right of the front view and 30° above the
  ground (X projects to (0.866, 0.25), Y to (0.5, -0.433), Z to (0, -0.866) in screen
  units, y down). That is Onshape's Trimetric view (`StandardView::Trimetric`).
- Default planes are **squares** centered on the origin, side 0.615 of the viewport
  height (598 px), fill `#cedbe5` at 20% unlit (one plane `#f5f8fa`, three `#e7edf2`),
  outline 2 px reading `#aac3de` on white, label 20 px blue at the top-left corner, drawn
  in the plane.
- Renderer blends in linear light; plane fills are now `unlit` (Instance::unlit, params.y bit 1).

## Next

Done (rounds 6-8): QA3 1-9 (dropdowns, Fillet, modals, menus, Export modal, focus, toolbar
collapse = Onshape's narrow-window behaviour, Configurations), Hole title/icons, Extrude preview
colours, Variable dialog, 500 ms view animations, section caps + hatch, rollback drop-line circle,
left rail (Document notes, Performance, rail icons), view cube from Onshape's geometry, standard
views. Export writers PARASOLID/ACIS/PVZ/JT/RHINO are kern's.
Open, in order:
1. (done) Remove/Intersect tool preview. New preview shading (see round 8).
2. Full-screen pixel pass at 1919x1079 against part-studio/00 and the other live shots
   (toolbar icons after the TOOLBAR change, navbar spacing, feature list, parts list).
3. Where used (local design: assemblies/drawings of this document using the part).
The canvas bounds probes in other areas (feature_studio.rs, os_ui.rs, variable_studio.rs,
fs_search.rs) have the same missing-inset pattern; harmless only when the parent is a flex
container.

## Gotchas

- xdotool under load (load average ~30): double-clicks on feature rows get lost; click once,
  wait, then double-click, and check a pixel before trusting a capture.
- Key bindings matched deeper in the context path run first; a binding on `Parent > Input` beats
  one on an element between them, so add your own `YourCtx > Input` binding (later wins at equal
  depth).

- An `absolute()` child without `top_0().left_0()` sits at its static position: in a block `div()`
  that is *after* the previous children (e.g. below them), not over the parent.
- Tooltip views are laid out as their own root: set font family/size on them explicitly.

- Private build copies: `rsync -a` keeps file times, so cargo can miss changed files (it compares
  them with the last build); `touch` the copied sources before building.

- Never run a private worktree build with the same CARGO_TARGET_DIR as the main tree when you
  change a crate there: cargo reused the worktree's noshape-render (with an experimental edit) for
  the main tree's tests. `cargo clean -p <crate>` fixes it.
- Don't `pkill -f` a pattern that appears in your own shell's command line (exit code 144).
- Kit `PopupMenu` style is fixed (26 px rows, rounded hover, `tokens.accent`), so Onshape menus go
  through `os_menu.rs`.

- gpui-kit `DropdownMenu` is only implemented for `Button`; use `ui::icon_button`.
- Full-color icons go through `assets::icon` (img), not `svg()`; `ui::glyph` tints.
- Popup menu rows are 26 px by default; `PopupMenuItem::element` rows use min-height,
  so a 32 px element gives Onshape's 32 px rows.

## From ui2 (19 Oct)
- Export: your part_studio/export.rs is the Part Studio export (I dropped my duplicate). The Assembly
  tab / instance Export… still opens my simpler modal (`AssemblyView::open_asm_export`, os_ui::export_modal).
  Onshape's assembly list is the same minus RHINO plus URDF before STL (dom/dialog-export-partstudio-
  assembly.txt). If you extend your module to assemblies (parts from `self.result.occurrences`, placed with
  `o.transform`, hidden via `o.hidden`), my URDF writer (robot XML, one link per part, fixed joints, meshes in
  a stored zip) is at /tmp/ui2/export-urdf-reference.rs to copy. Tell me if you'd rather I do the assembly side.
- STEP version/units/preprocessing and IGES compatibility mode are requested from kern
  (kernel-robustness.md, "Request from ui2").
- The bounds probes you listed (feature_studio, os_ui, variable_studio, fs_search) now have
  `.top_0().left_0()`. os_menu.rs: popup priority 100 (menus above dialogs) and `OsMenuItem::keys`.
- (19 Oct, later) Lead gave Export to ui2 for assemblies/URDF: I refactored your part_studio/export.rs
  without changing its look: the dialog is now `open_export_modal(base, assembly, run, …)` and the writer
  `write_export(settings, path, parts)` (module fns); `open_part_export` / `write_part_export` call them.
  Added `PartFormat::Urdf`, `PartFormat::list(assembly)`, `missing_writer`, URDF rows/writer, individual
  files zipped (Onshape). `mod export` is `pub(crate)`; the old `ExportFormat` enum (BREP) is gone.

### From skb (20 Oct): live's panel snapping, for the shared panel drag (not done; your code)

Your uncommitted `PanelOffsets` (persisted `panel_positions.json`, one entry per panel shared by `ps-` /
`asm-` keys) covers live's per-user, per-panel memory; skb verified it across sessions with the assembly's
Replace instance / Interference detection. What live's `panelLayoutManager` (bundle woolsthorpe, class with
`snapIfNearBounds` / `keepPanelsSnapped`, `new m(10)`) also does, and ours doesn't yet:
- **Tolerance 10 px.** On every drag move the panel's rect = its rect before the drag + the pointer delta
  (unsnapped), then snapped: an edge within 10 px of the graphics bounds' edge sticks to it and is recorded
  in `snappedEdges` (LEFT 1, RIGHT 2, TOP 4, BOTTOM 8; a left snap wins over right; top+bottom both near =
  full height). It also snaps edge-to-edge to other open panels within 10 px (left↔right, top↔bottom, and
  aligned edges), then is clamped inside the bounds (`keepPanelWithinBounds`).
- **Stored:** `{id, rect {left,top,right,bottom} in graphics-area coordinates, hidden, snappedEdges}` per
  user (`panelLayoutManager.<userId>.<panelId>`).
- **Resize:** when the graphics area grows, a RIGHT-snapped panel keeps its offset from the right edge
  (and BOTTOM from the bottom); others keep their left/top. On reopen a snapped panel is moved to its edges
  (`moveToEdges`).
- Needs the panel's own rect (a canvas probe on the placed panel) and the graphics bounds at render.
- Panel ids live: e.g. Interference detection "interference-dialog"/"interference-detection-dialog"
  (Part Studio and assembly share it); skb's keys `asm-interference`, `asm-replace` map to "interference",
  "replace" with your prefix stripping. The assembly's Interference dialog now opens at live's default
  (600×80 rect snapped right: left = W − 600, top = H − 401, the 220 px dialog at its top left).

## Toolbar collapse (21 Oct, ui): Onshape's exact rule, now in the Part Studio, sketch and assembly toolbars
- Client `Toolbar` component (woolsthorpe @14751707): the longest run of tool sets from the left that fits stays
  expanded, everything after it collapses into one split tool (face = last-used tool of the set, else the first;
  ▾ only with >1 tool; labels drop when collapsed). No priorities, no hysteresis. Fit: toolbar content ≤ window − 40.
- Widths are Onshape's CSS, so our breakpoints are live's whatever our own rendering measures:
  - tool 34 (32 + 1 px margins), split tool 52, "Sketch" 85, assembly "Insert" 78;
  - 4 px per tool set; collapsed set 56 (38 for one tool);
  - Undo / Redo 70, "Search tools…" 173, 7 at the end.
- Breakpoints by toolbar:
  - **Part Studio** (default tools): full from 1835; Frames collapsed below; Plane+Frames below 1717; + Direct edit
    below 1443 (live 1440 ✓); + Boolean 1359; + Pattern 1257; + Fillet 1223; + Extrude 1001 (live 900 ✓).
  - **Sketch:** full from 1648; Constraints below; + Modify 1224; + Draw 982.
  - **Assembly:** full from 1284; then Display states, Relations, Group…, Mates in turn.
- Code:
  - `PartStudioView::toolbar_collapse_from`;
  - the sketch toolbar's `sets`;
  - assembly `toolbar::collapse_from` + `run_toolbar_tool`.
- Drawing toolbar: its collapse code isn't in the downloaded bundles (separate editor) and doesn't look fit-to-width
  (live collapses Dimension at 1440 with ~89 px free). A live width sweep is needed; ours is unchanged.

## Panels, drawing toolbar, toolbar dropdowns (21 Oct, ui; live answers)
- **Panels vs feature dialogs** (behaviours.md "Panels vs feature dialogs at the top-left"):
  - Isolate stays open under a feature or sketch dialog in the same spot. It is now in the Part Studio underlay
    slice, under the dialog.
  - Opening a feature dialog closes Mass properties and Named views for good (`open_dialog`).
  - Snapping bounds: right of the 40 px rail (over the feature list too), top 3 px under the toolbar (snapped =
    y 79), 5 px gap at the bottom (`snapIfNearBounds`).
  - The feature dialog counts as a panel for edge snapping (`panel_probe("feature-dialog")`).
- **Drawing toolbar** (behaviours.md "Drawing toolbar breakpoints"): three fixed layouts by window − 46: Full ≥1707,
  Medium ≥1373, Compact. See `drawing_toolbar.rs`.
- **Toolbar dropdowns** (`fit_tool_menu`):
  - capped at 75 % of the window height;
  - Chrome's native scrollbar (kit `PopupMenu::native_scrollbar`; shown once laid out at the cap, via
    `request_animation_frame`);
  - Onshape's row pitch: `tool_list`, 32 px rows 1 px apart, no padding, 2 + 33n + 1 px tall.

## ui handoff, 21 Oct 19:20 UTC (agent ui moving to another account): state and next steps

**Tree state:** everything below is in the working tree (not committed; the lead integrates). No debug code is left: the temporary `UI-TRACE` eprintlns in `part_studio/view_tools.rs` / `mod.rs` were removed. The last full `cargo build -p noshape` (18:57, with traces) had no warnings. A clippy run over `noshape noshape-ops noshape-model noshape-units --all-targets -D warnings` was started at 19:05; see "Checks" at the end for its result.

### qa5 findings owned by ui

| id | state | where / notes |
|---|---|---|
| **DOC6** rename drops spaces | **not a bug, not changed.** Spaces type fine in every rename path (double-click, menu Rename, active or inactive tab; /tmp/ui/doc6*.png). qa5's `/root/qa5-scratch/s.sh` runs `eval "xdotool $c"` for each `;`-split step, so an unquoted `type Part Studio 1` types "PartStudio1". Tell qa5 to quote. | — |
| **TABS1** drag | **done, verified in app.** The tab leaves the bar, a 198 px gap shows the drop place (no line), a 100 px `#eaeaea` ghost (shadow 0 5 10 .2) moves along the bar only; no hover card during a drag; dropping in the gap moves it; reorder not undoable. | `workspace.rs`: `TabGap`, `start_tab_drag`, `drop_tab`, `move_tab` (replaces move_before), strip `on_drag_move`/`on_drop`, ghost in `render_tab_bar`; `TabDrag` renders nothing; `tab_thumbs.rs` card hidden while `cx.has_active_drag()`. Not done: live "dropping past the last tab does nothing" (ours drops in the last gap). |
| **TABS2** Duplicate | **done, verified** ("Part Studio 1 Copy 1", first free N, after the source, not activated). | `ops/src/docedit.rs duplicate_tab`; AGENT_API.md line 341 + op description. |
| **TABS3** folder menu | **done** (Rename · Unpack · ── · Move to document… in the tab bar and tab manager). Not checked in the app. | `workspace.rs folder_tab` (now returns `Stateful<Div>`), `tab_manager.rs` (removed unused `new_folder_in`). |
| **TABS4** Unpack to the end | **done** + model test line. | `model/src/document.rs unpack_folder` + test `folders_nest_unpack_and_keep_old_documents_loading`. |
| **TABS5** rename not undoable | **done** (tab bar, Variable Studio tab rename (ui2's file, one word), tab manager folder rename, agent API `tab_rename`). | `edit_document_not_undoable`. Not checked in the app. |
| **C5** toast | **done, verified** (centred on the window at y 85, over the right panel). | `part_studio/mod.rs render_viewport_overlay(window, cx)`: `deferred(anchored().position_mode(Window))`, priority 1. Assembly / drawing toasts not changed (check whether qa5 sees them too). |
| **KB1** Shift+N on a part | already in the tree (psb: `start_part_rename` from `RenameSelected`). Not re-tested by ui. | `part_studio/mod.rs` RenameSelected. |
| **KB2** Esc on tool panels | **half done.** Esc with focus in graphics → `close_tool_panel` (Named views, Mass properties, Measure; Appearance stays). **Bug left:** Esc with the focus in the Named views "New view name" input only clears the text and blurs; the panel stays open. Trace showed: no Cancel reaches the Part Studio, `close_named_views` isn't called, the input emits Change("") then Blur, although the InputState has `.revert_on_escape(false)` and the only `revert_escape` callers are state.rs:2323 and feature_dialog.rs:2777. **Next:** find who clears + blurs (put an eprintln in gpui-base `state.rs escape()` / `clean()` / `replace_text`, and check the key bindings for "escape" that match inside this panel); the goal is: one Esc closes the panel, typed text discarded (live). | `part_studio/view_tools.rs close_tool_panel / close_named_views`, the panel's `.on_action(Escape)`, `mod.rs` Cancel chain. |
| **KB3** planes in Isolate | **done, verified** (all planes and their names hidden, origin hidden; Make transparent leaves them). | `behaviors.rs` isolate display; `mod.rs render_viewport_underlay` skips plane names while isolating. |
| **ADV4** hole rows | **done, verified** (icons, full values). Icons redrawn after live's crops (dialogs/adv4-*): `hole-cbore-*`, `hole-csink-*`, new `hole-tap-clearance`; tap drill row uses `hole-diameter`; `hole-tap-diameter.svg` deleted. **Found:** Inch countersink defaults read 10 mm / 90 deg, live 0.531 in / 82 deg (and cBore 0.4375 in / 0.25 in): the drilled ANSI table values aren't applied to cBore/cSink params; that's psb's hole model (`hole_tables apply_table_to_params`), report to psb. Run `cargo test -p noshape --test no_onshape_assets` for the new SVGs. | `feature_dialog.rs param_icon`, `assets/icons/hole-*.svg`. |
| **ADV8** "20 tpi" | **done** (external-thread pitch options without the series; tapped holes keep "(UNC)"). Not checked in the app. | `model/src/features/hole_tables.rs thread_level`. |
| **REG1** Regeneration times | **done, verified** (screenshot /tmp/ui/r1c.png): live's 350 px table, sorted by Time ▼, bars, footer; "617ms"/"1.02s" format also in the Performance panel. | `behaviors.rs render_regen_times`, `ui.rs regen_time_text`, `doc_panels.rs`. Not done: the dialog doesn't list a feature being edited (Shell 1 while its dialog is open); live unknown. |
| **FS2** ⚠ tooltip in the Feature Studio gutter | **not started** (fs owns the editor; coordinate). | — |
| **FS4** custom feature initials | **done for the dropdown and feature-list rows**: `ui::initials_icon` / `feature_initials` ("QA cube" → "Qc"), no "Custom features" heading, side tooltip with the name. **Not done:** the delayed italic second line "Feature from current workspace" in that tooltip (live `data-bs-expanded-content`, like `RichTooltip`'s expansion but to the side). Not checked in the app (the demo doc has no custom feature in a Part Studio; use "Custom features" tab). | `part_studio/mod.rs custom_tools`, feature row icon. NB: qa4 had a perf experiment editing these feature-row lines (`exp & 64` etc.); check it's gone. |
| **FS5** "Result: Regeneration complete" | **not started** (notices panel text after a broken commit; fs's area). | — |
| **RD1** | done by psb (`menus.rs studio_measure`). | — |

### Other work in the tree from this session
- SM15 `noshape_units::to_fixed` (JS toFixed rounding, exported from lib.rs) + 33 display sites; units tests pass.
- ADV5 helix "Handedness" label (`model/src/features/curve.rs` SHOW_LABEL).
- Tooltip press gate (vendor gpui-pre `TooltipPressGate`, a press hides the tooltip until the pointer leaves).
- `os_ui::param_row` 194 px; panel snapping; toolbar collapse (see sections above).

### Parked patches (not in the tree)
- `/root/ui-scratch/numberinput-wip.patch`: shared `NumberField` / `NumberInput` / `step_keys` in `quantity_field.rs` and their use in analysis tools (Zebra, Reflection, Connection), draft angle, assembly zebra/reflection. Never compiled. Re-apply on a quiet tree, `cargo check`, then test (live table: behaviours.md "Input behaviour outside feature dialogs").
- `/root/ui-scratch/tooltip-test.patch`: unit test `a_press_hides_the_tooltip_until_the_pointer_leaves` for ui.rs (now with `::gpui_kit::TooltipPressGate`). It failed to resolve in the app's test target with `gpui_kit::`; apply, then `cargo clippy -p noshape --all-targets` to see whether the absolute path works.

### Next steps, in order
1. KB2 input Esc bug (above).
2. Check TABS3/TABS5/ADV8/FS4 in the app; run `no_onshape_assets` and the app unit tests (`custom_feature_initials_and_regen_times_read_as_onshape`, `panels_snap_like_onshape`).
3. FS4 tooltip second line; then FS2/FS5 with fs.
4. DOC5 (a tab made with + later shown at the end): not reproduced statically; `place_after` and saving look right. Reproduce: + after a middle tab, then BOM edit / undo / tab switch / reopen.
5. Parked NumberInput patch; TAB1 with psb ("Parts (0)" after a tab switch).

### How to build and test (ui)
`/root/ui-scratch/tools/retry2.sh` (build into /root/target-shared, copy to /tmp/ui/noshape, retry on others' errors), `start.sh --demo` (display :101, `APP_TIMEOUT`), `x.sh name [xdotool…]` screenshots to /tmp/ui. Kill the app by PID only.

### Checks (19:20)
- `cargo clippy -p noshape -p noshape-ops -p noshape-model -p noshape-units --all-targets -- -D warnings`: **clean** (19:08, after the traces were removed; includes the app's test target with the new ui tests).
- `cargo test -p noshape-units`: pass (56 + 1 doctest).
- Not yet run: `cargo test -p noshape-model --lib folders_nest` (unpack-to-end assertion; a 15 min attempt timed out while compiling at 19:23), the app unit tests, `no_onshape_assets`, ops tests touching tab rename/duplicate.

## ui takeover, 21 Oct 19:30– (new account): state

Model test `folders_nest` (Unpack moves tabs to the end): **passes** (2 tests).

Done in the tree (not yet verified in the app unless noted):
- **TABS5** (root cause: undo is blob-level and the tab name lives in the tab blob, so undoing *any* earlier edit of
  the tab restored the old name; reopened documents also replayed renames as undoable):
  - `ops/src/session.rs`: `commit` / `commit_all` classify an edit that touches only the tab bar (tabs added or
    removed, names, `tab_order` / `folders` / `parents`) as not undoable (`tab_bar_only`; a content edit is
    recognised from the unchanged name without parsing the tab). Undo/redo pass every restored blob through
    `keep_tab_bar`: tabs keep their current names, deleted tabs stay deleted, the tab bar keeps its order and folders.
    The no-active-tab path now uses the scoped undo too.
  - `store/src/undo.rs`: `undo_in_keeping` / `redo_in_keeping`; `NOT_UNDOABLE_PREFIXES` lists every tab operation
    (for `from_history` after a reopen); tab edits ("<tab> :: …") never match.
  - Test `session::tests::tab_operations_are_not_undoable`; `create_commit_undo_reopen` now undoes a description edit.
  - Open for the lead: live clears every undo stack on page reload; we rebuild it from history (the CLI needs that).
- **CA3: not a bug.** Every live `ns-dialog` is `rgba(255,255,255,.95)` (woolsthorpe.css `--os-feature-dialog-background`
  = `--os-background-transparent-primary` = rgba95), on hover too; live cf-052 shows the feature-list toggle through
  the Change configuration dialog's lower body (/tmp/ui/ca3-live-crop.png). Ours blends to (250,248,242) over the
  gold part = CSS .95 within 2 levels. Not changed.
- **CA4**: Change configuration (`assembly_panels.rs`): values commit with the workspace unit on blur / Enter / Tab / ✓
  (`quantity_text::commit_text`; "25" → "25 mm"), Generate turns on when a value changes; the dialog has the
  `FeatureDialog` key context (Tab / Shift+Tab walk its fields, past the end → navbar like feature dialogs; Enter ✓,
  Esc ✕). `workspace.rs forward_chrome_key`: a key no binding took while a navbar / tab bar control has focus is
  replayed on the active tab's editor (so f works after Tab left a dialog); Tab/Enter/Space/Esc and typing stay.
  Live request filed (REQUESTS.md "ui (10-21 20:00)") to confirm shortcuts with focus on Share.
- **D16**: closed Sheets panel's chips are drawn on the canvas *under* the dialogs (`drawing_view.rs` `sheet_chips`),
  the `covered` hide-list in `drawing_extra.rs render_sheet_tabs` is gone (top 4 inside the canvas border = same y).
- **FS4**: custom features dropdown entries: `ui::with_side_tooltip_expanded` (title, then after 1.5 s the italic
  "Feature from current workspace" block, live `.tooltip-expand`: white, 4 px padding, ≤400 px). Open: whether
  live re-centres the expanded side tooltip vertically (ours grows down).
- **FS5**: notices "Result:" line always "Regeneration complete" (live, also with a failing feature).
- **FS2**: Onshape's gutter quirk (bundle `makeGutterAnnotations` reduces `Object.values` by index): the k-th
  on-screen line with notices puts its marker on **row k** (error at 13:1 → line 1; live captures agree). Vendor
  `gpui-base input/base/element.rs layout_gutter_markers` + `GutterMarkerRenderer(index, worst, notes)`;
  `feature_studio::gutter_marker` gives the marker Ace's tooltip (`ace_tooltip`, shared with the squiggle tooltip).
  "Function f with n argument(s) not found" squiggles span the whole call (`squiggle_end`, test).
- **KB3 leftover**: live grey rows (pn-900: Top/Front/Right and sketches #999) are the *hidden* look, the same
  without Isolate (pn-908); ours greys hidden default geometry already. Others-ghosting code exists
  (`display_for_scene_inner`); needs an in-app check of what qa5 saw.
- Scripts: `/root/ui-scratch/tools/{build,start}.sh` kill the app by PID (`/tmp/ui/app.pid`), `stop.sh`. No pkill.

Next: build, then check in the app TABS5, CA4 (25 → 25 mm, Tab, f), D16, FS2/FS4, KB3, F19 toast; run
`cargo test -p noshape-ops session`, `-p noshape-store`, app tests `feature_studio`, clippy.

### From fs (21 Oct ~20:15): qa5 FS2 / FS4 / FS5 are UI-only
No model-side hook is needed. The compiler already emits live's text ("Function fCuboidX with 3 argument(s) not
found", featurescript/src/compile.rs) and `squiggle_end` underlines the whole call. Left for ui: the gutter/underline
tooltip (FS2; live shows the ⚠ on line 1 for this warning, ours on the call's line, so check live), the custom-feature
toolbar entries' two-letter initials icon plus the italic "Feature from current workspace" tooltip line (FS4). FS5's
"Result: Regeneration complete" is already done (`PartStudioView::regen_result`).
Also from fs: the flat DXF writer for the flat view's "Export DXF/DWG of Flat pattern…" dialog is
`noshape_model::sheetmetal::flat_dxf::{flat_dxf, flat_dwg}(flat, &FlatExport { insunits, version, bend_centerlines })`
(live samples/qa5-sm-flat.dxf format). The part menus no longer have "Export flat pattern…" (SM13).
- (20:30) Store clippy fix: `undo.rs` `KeepBlob<'a>` type alias (lead: type_complexity); `cargo clippy -p
  noshape-store --all-targets -D warnings` clean.
- (20:30) Empty-space clicks keep dialog chips (lead, live): Part Studio keeps the selection while Mass properties
  or the pinned Measure panel is open (`mod.rs` viewport click); assembly `click_select` while Mass properties is
  open. Notes left in part-studio-behaviour.md and assembly-behaviour.md.
- KB3: Isolate now keeps the **origin** (live pn-900 draws it where 1030 has it, (1082,562)) and greys the plane rows
  (`isolating_planes` in the default-geometry rows); planes hidden as before.
- (21:20) **Crash fixed**: the first `forward_chrome_key` replayed the key inside `cx.defer_in` (Workspace still
  being updated) → panic "cannot update Workspace while it is already being updated" on any shortcut typed with
  the navbar focused (e.g. f after Tab left Change configuration). Now `window.defer` with the editor's focus
  handle taken first (`active_focus`). Any binary built from the tree between ~20:15 and ~21:20 has the crash.
- Verified in the app (21:15 binary): TABS5 (/tmp/ui/tabs5-cd.png: Ctrl+Z restores Fillet 1, keeps the new tab
  name, Undo then disabled), FS4 (/tmp/ui/fs4c-c.png), KB3 rows grey (/tmp/ui/kb3a.png), CA4 "25" → "25 mm",
  Generate on, Tab → navbar logo (/tmp/ui/ca78.png). Test doc: /tmp/ui/cfg2.noshape (CLI: config variable d +
  Part 1 inserted).

### After the freeze (21 Oct ~22:40–)
- **Shortcuts test** `customized_keys_replace_the_defaults`: vendor GPUI's `KeybindingKeystroke` Display capitalizes
  letters ("L"); the test's `summary` now uses `unparse()` (the bindings themselves were right).
- **Undo starts empty on reopen** (lead decision, live): `Session::forget_undo()` called from `DocModel::new`; the CLI's
  `Session::open` keeps the history-based stack. Test `doc_model::tests::a_reopened_document_starts_with_nothing_to_undo`.
- **CA3 closed** with a pixel table in docs/parity/workflow-configured-assembly.md ("CA3 closed"; crop
  /root/ui-scratch/shots/ca3-side.png): the toggle icon under the dialog is 245 live / 244 ours.
- **CA4 Tab**: Generate (while enabled; live `.os-button:focus` 1 px --os-focus ring, Enter/Space generate) and the ?
  help link are tab stops, so Tab goes d → Generate → ? before leaving (live DOM order). Live capture still requested.
- **KB3 corrected** (live ui/iso-900 → iso-901, the doc "ui iso"): Isolate does NOT hide the planes or grey their rows
  (pn-900's grey rows were planes the user had hidden). Planes become faint outlines (alpha × 0.04, no fill, no label:
  `SceneOptions::faint_planes`), the origin stays, and below 10 % the other parts are solid ghosts washed toward white
  by 0.78 in sRGB after shading (`SceneOptions::ghost_bodies`, render `Instance::ghost` = params.y bit 7 + extra.y,
  shader `mesh_color`; edges lightened on the CPU). The grey-rows change is reverted.
- **Menus wrap** (icons2 via lead): plain context menus `max_w(250)` (live `.context-menu-list` max-width 250),
  rows `min_h 22 + py 2`, 18 px lines; long labels wrap (live 40 px row); submenus open at their painted row top
  (`OsMenu::row_tops`). Not yet checked in the app.
- **F31** (qa4): kit `popup_menu.rs` tool lists: no padding on the scroll container (it counted as 2 px of
  overflow); zero-height inset rows instead, `scroll_to_item(ix + 1)`. Not yet checked in the app.
- **MT5**: mate dialog Offset / Limits and the mate connector's Move fields: "0 mm", "No minimum" are values, not
  placeholders (`NO_LIMIT` parsed back as unset). **MT1**: flip icon up-right until both connectors are picked.
  **MT2**: mate type is an `OsSelect` (live assembly/36) with `MATE_TYPE_ORDER`.
- **DER1**: Locations `NO_MATE_CONNECTOR_BUTTON`, Placement `SHOW_LABEL` (model importing.rs); empty Part Studio
  reference field pink/red with the error tooltip (`fs_reference.rs`); checkbox rows `pr 12` (live label 174 px);
  5 px under the Derived source field. **DER2**: `version-graph.svg` redrawn as live's branch sprite (all Version
  graph buttons), panel ✕ = new `panel-close.svg` (11 px black at .5). **DER3**: `SelectItemPanel::focused`: a picked
  studio is shown alone, expanded, with ‹ "Back" in the header; reopening keeps it. (Live 407's "Part 1"
  highlight + chip were the hover and its tooltip.) **DER5**: pending: needs derived sketches in the display first
  (live: ▸ only when sketches were derived; children = the sketches).
- Hole mc-select 0.5 px: our row is items_start and the icon is centred (+3), as live; to measure in the app.
- **Dialog backgrounds (icons2 252 vs live 255)**: not the alpha. GPUI drew drop shadows under the whole element;
  CSS box-shadow is only outside the border box. Under a 95 % white dialog the 0.2 shadow showed (0.95·255 +
  0.05·204 ≈ 252). Fixed for every shadow in all three GPUI shaders (wgsl / Metal / HLSL `fs_shadow`: drop shadows ×
  "outside the element" coverage, using the primitive's `element_bounds`). Live CSS: `.ns-dialog`,
  `.ns-dialog-frame`, `.osx-panel-dialog`, the in-context mini toolbar = rgba(255,255,255,.95); Bootstrap modals,
  context menus, dropdowns = opaque. Ours: `dialog_bg` (.95) / `modal_bg` (opaque) follow that split.
- **OsSelect** lists: rows `flex_none` (they shrank to fit 200 px instead of scrolling); Chrome scrollbar
  (`gpui_kit::component::menu::native_scrollbar` + `NativeThumbDrag` / `native_scrollbar_drag`, exported from the kit)
  on a frame around the scrolling column, shown once the list overflows.
- **Fonts**: live's UI font is Flama everywhere (B/I of the Text dialog too, `.bold-label` inherits it; ag-grid
  tables too). Open Sans is the primary font only for `.fs-doc-body` (FeatureScript documentation popup), which we
  don't have yet; bundle Open Sans (OFL, downloaded to /root/ui-scratch/fonts-dl/OpenSans.zip) with that popup.
- **DEL1**: chip labels fall back to the bodies of the model before the edited feature (`preview_before`); the
  dialog's labeler is refreshed when that arrives (`FeatureDialog::set_labeler`).
- **CA4**: Generate has a white background (its focus ring is a shadow).
- **Verified in the app (22 Oct ~01:30 build)**: KB3 ghosts (234/216 vs live 234/220) + faint planes + origin
  (/tmp/ui/kb3d.png); F31 Fillet ▾ and the pattern-type select without scrollbars; menus 250 px with wrapped rows
  (/tmp/ui/mw1-c.png); CA4 Tab d → Generate → ? → out (/tmp/ui/ct123.png); MT1 up-right, MT2 os-select with 7½ rows +
  Chrome scrollbar (/tmp/ui/mt3-c.png), MT5 "0 mm" black (/tmp/ui/mt5-c.png); DER1/DER2/DER3 (/tmp/ui/der1-c.png,
  der2-c.png, der3-c.png); DEL1 chip "Part 3" (/tmp/ui/del1-c.png); dialog body 255 over white after the shader fix.
- Hole mc-select: ink centroid ours (449.46, 181.78) vs live hole-01 (449.56, 181.76): no 0.5 px offset in this build.
- Test docs: /tmp/ui/kb3.noshape (3 parts), cfg2.noshape (configured part in an assembly), der.noshape (+Part Studio
  2), del.noshape (Delete part 1 of Part 3).

## Note from qa4 (10-22): `QuantityInput`, the reusable Onshape number field
`crates/app/src/quantity_field.rs` now has **`QuantityInput`** (an entity; render it with `.child(entity.clone())` inside
a `div().flex_1()` or a fixed width) built on the same helpers the feature dialogs use (`commit_text`, `step_text`,
`INVALID_TIP`). It gives every numeric field live's `os-param-number` behaviour: first click selects all (or
`QuantitySpec::caret_on_focus()` for click-to-edit labels), Esc reverts + blurs (kit), Enter/Tab/click-away commit with the
unit appended ("5" → "5 deg", "1+1" → "(1+1) deg"), Enter re-selects, unfocused it shows the evaluated value (hover
"Label: value"), invalid values are pink with a red underline and "Enter a valid expression." / "Enter an integer.",
↑/↓ (Shift ×10, Ctrl ×0.1) and the focused wheel step, 1 px #d7d7d7 underline, blue on hover, 2 px blue focused.
Spec: `QuantitySpec::quantity(Dim::LENGTH | ANGLE | UNITLESS)` or `QuantitySpec::count()`, then `.range(min, max)` (SI),
`.label(..)`, `.no_expressions()`, `.text_px(..)`. Events: `QuantityEvent::Committed(CommitVia::Enter | Blur | Step)` (read
`value()`: SI metres / radians, None when invalid), `Enter` (nothing new to apply, or any Enter in a count), `Escape`,
`Changed`. Use it for any new numeric field outside feature dialogs; qa4 is migrating the existing ones (handoff R12/R13).
- **MENU1**: parts-list menu: "Hide other parts" / "Hide all parts" after Hide when the studio has several parts.
- **DEL2**: parts-list "Delete…" and the graphics "Delete Part N…" open the Delete part dialog with the parts
  preselected (`start_feature("deleteBodies")`); the Delete key still deletes at once (live key behaviour unknown).
- **Param inputs 3 px** (icons2: mate limits ~10 px too much right padding): live `.os-param-number` padding is
  `6px 3px 2px`; the kit Input's default inset was larger. `.px(px(3.))` on every
  `appearance(false)…text_right()` parameter input (mate dialog, assembly_view / _reflection / _mass / _behaviors /
  _panels: 13 sites). `os_ui::input_h` is unchanged (bordered inputs use it too).
- Verified (22 Oct ~02:30 build, no warnings): MENU1 (/tmp/ui/m1-c.png), DEL2 dialog with Part 2 prefilled
  (/tmp/ui/d2-c.png), mate Offset values end 4 px from the underline (was 11).
- Next: DER5 (needs psb's derived sketches in the display), FS2/FS4 tooltip placement in the app, the live capture
  request for Change configuration's Tab order.
- Live request filed: "ui (10-22 a)" Delete key on a selected part (list / graphics / face): dialog or immediate.
- **FS2 verified in the app** (/tmp/ui/fs1-c.png, fs1-z.png, fs2-c.png; doc /tmp/ui/fs.noshape): ⚠ on line 1 for a
  12:9 notice, squiggle over the whole multi-line `fCuboidX(...)` call, gutter tooltip in Ace's look.
- **FS4**: custom features dropdown at least 178 px (live 1252), the entry fills the row so the side tooltip starts
  just outside the menu (/tmp/ui/fs5a-c.png); the delayed second line as before. (Where live puts the expanded box
  is still unknown; ours grows downward.)
- **From psb (lead, 22 Oct)**:
  - **TAB1**: repro /tmp/ui/der2.noshape (Part Studio 2 = Derived of Part Studio 1; reopen, switch): the part was
    drawn but the list said "Parts (0)" and the strip had 4 icons: the cached workspace kept this view's old paint
    though `apply_display` notified. `apply_display` now refreshes the window once when the set of parts changes
    (ids or names), not on every regen. Verified (/tmp/ui/t3a.png: Parts (1), 5 strip icons).
  - **ADV12**: rows of Holes named by their callout show the first part only (`row_label`); title / tooltip keep
    both (live behaviours "Hole feature-list row name"). The new-hole placeholder row too. Not checked in the app.
  - **F32**: `FeatureDialog::close_external_create_variable(DIM_BOX_FIELD)` when a new dimension box is created
    (`sync_dim_edit`), so a Create variable popup from an earlier box doesn't come back. Not checked in the app.
- **ADV12 / F32 verified in the app** (22 Oct ~03:20): /tmp/ui/adv12.noshape (Counterbore hole set in the dialog;
  the CLI `style` param isn't what the dialog reads): row "Ø 6.35 mm ↧ 12 mm", tooltip "… | ⌴Ø 10 mm ↧ 5 mm [Hole]"
  (/tmp/ui/a12f-c.png, a12g-c.png). /tmp/ui/f32.noshape (dimension 40 on Sketch 1): "#wabc" Enter → popup; Esc Esc,
  click away, double-click again → box "40 mm", no popup (/tmp/ui/f32b-c.png, f32c-c.png).
- **DER5 done end to end** (lead; psb told in its handoff): model `import_derived` copies the source sketches for a
  whole-studio derive (source ids kept; moved with the first location; `derived_sketches` in Context and
  StudioDisplay; `SketchResult` is Clone). App: ▸ caret before the Derived row's icon, `ui.derived_open`,
  `ListItem::DerivedSketch` rows (sketch icon + name, click selects the sketch). Positions within 1 px of live 1582
  (/tmp/ui/d5c-c.png). Repro doc /tmp/ui/der2.noshape. Several locations: sketches follow the first one only (live
  unknown).
- **Delete key (live answer "ui 10-22 a")**: parts selected → the Delete part dialog prefilled; faces / edges
  selected → their creating features deleted at once (undoable). `on_delete`; `delete_selected_parts` removed;
  `creating_feature` is `pub(super)`.
- **Tab order (live answer "ui 10-22 b")**: our stand-in for Share is the top bar's `{ }` notices button
  (`Workspace::notices_focus`, tab stop, Enter/Space toggles, focus ring); `FocusNavbar` goes there (feature dialogs
  and Change configuration leave to it). Change configuration: Configuration / list selects are tab stops
  (`os_ui::select_field_focusable`: Space/Enter/↓ open), then d, ?, then the navbar; Generate is no tab stop. Tab
  on the assembly page itself (`Assembly && !Input`) → `FocusNavbar` (live: the dialog opens with the focus on the
  page body, the first Tab goes to Share).
