# Theme and layout restyle (agent "ui")

Goal: the Part Studio must look exactly like Onshape (live reference in
/root/onshape-research/live/, spec in docs/ONSHAPE_UI_SPEC.md). Build dir
/root/target-ui, display :101. Side-by-side shots go to /tmp/ui-shots/.

## State (2026-10-18 14:50 UTC)

Taken over by a fresh "ui" agent at 13:25 (the previous one ran out of quota). Baseline
75869cc; all work is uncommitted in the shared tree. Context menus, Sketch dialog options, reusable
view cube, render test: done (see "Done since takeover"). Next: feature-dialog pixel pass, tool
search popover, tooltips check, selection-count cursor, right panels, row states.

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

- Switch the remaining kit context menus to `os_context_menu` (assembly, drawing, feature-dialog
  chips, tabs; the tab and "+" menus are Bootstrap dropdowns in Onshape: 14 px `#555`, 27 px rows,
  hover `#def1fb`, radius 5.25).
- Tool search popover (600 px, results + description pane, yellow toolbar highlight).
- Tooltip check vs part-studio/11-tooltip-extrude-expanded.png (RichTooltip exists).
- Selection-count cursor (cursors/: N = 1..5, drawn into the pointer).
- Selection colors check vs selection/*.png (face hover = boundary only `#fec788`, selected
  `#e5aa38`), box select colors, display modes, section view hatch.
- Right panels (Appearance, Variables, Configurations; 712 px, blue centered title), Measure and
  Mass properties panels, rollback bar drag pill, feature-list row states (part-studio/90-95).
- Select-mate-connector 26 px button next to MC-capable fields (Sketch plane, etc.).

## Gotchas

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
