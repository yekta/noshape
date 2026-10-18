# Theme and layout restyle (agent "ui")

Goal: the Part Studio must look exactly like Onshape (live reference in
/root/onshape-research/live/, spec in docs/ONSHAPE_UI_SPEC.md). Build dir
/root/target-ui, display :101. Side-by-side shots go to /tmp/ui-shots/.

## State (2026-10-18 20:10 UTC)

**For the lead:** the new disk rule `find …/debug/deps -maxdepth 1 -type f -executable -mmin +60
-delete` also deletes proc-macro `.so` files (they are executable), which forces a rebuild of
every dependency (gpui, rhai, nalgebra…) in that target dir. Use `… -executable ! -name '*.so'
-mmin +60 -delete` instead.

Round 4 (lead's list after the Wayland decision): Wayland count cursor done and tested; Appearance
and Configurations bodies, Graphics preferences (dialog + live settings), Highlight boundary
edges, tool search name matching + Onshape ranking, full tool descriptions: done (see "Round 4").
Open: section interference color, underconstrained-occluded sketch color and mesh line width are
in the dialog but not rendered yet; Measure unit lists wait for a live capture (REQUESTS.md).
The documents-page menus were already switched by ui2 (with `list_no_icons()`).

The shared tree didn't compile for most of this round (other agents' half-done session.rs,
workspace.rs export dialog, reflection.rs, analysis_tools.rs); I built and tested in a private
copy (/root/noshape-ui, rsync of the tree + local fixes of those errors only). All my edits are
in the shared tree.

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
- (older, by the previous agent) /tmp/ui-shots/new-doc-side-by-side.png: new document.

## Measured facts (live 1920x1080 screenshot, viewport 1673x973)

- Default view: **orthographic**, camera 30° right of the front view and 30° above the
  ground (X projects to (0.866, 0.25), Y to (0.5, -0.433), Z to (0, -0.866) in screen
  units, y down). `StandardView::Isometric` now encodes this.
- Default planes are **squares** centered on the origin, side 0.615 of the viewport
  height (598 px), fill `#cedbe5` at 20% unlit (one plane `#f5f8fa`, three `#e7edf2`),
  outline 2 px reading `#aac3de` on white, label 20 px blue at the top-left corner, drawn
  in the plane.
- Renderer blends in linear light; plane fills are now `unlit` (Instance::unlit, params.y bit 1).

## Next

- Appearance and Configurations panel bodies (the frame is done; the bodies still use the older
  layout): Appearances = tabs "Parts and surfaces | Sketches and curves" + rows with swatch
  bars (dom/panel-appearance.txt); Configurations = "Configure Part Studio ▾" + illustration.
- Camera menu: "Graphics preferences…" (gear icon) and "Highlight boundary edges" items (psb
  behaviour first). Section view hatch colors (part-studio/70-74), translucent/hidden-edge
  display mode checks against part-studio/60-68 in the app.
- Rollback drop line: Onshape draws a small circle at its left end.
- Left rail and right strip icon sets vs live (Onshape has 6 rail and 5 strip buttons).

## Gotchas

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
