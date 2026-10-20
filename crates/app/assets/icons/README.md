# noshape icons

289 icons.

> **Rendering (read this first).** Icons are **multi-color** and the app renders them
> in full color through GPUI `img()` (helper `assets::icon(name, size_px, cx)`), not
> `svg()`/`Icon`, which would reduce them to a single-color mask. Each SVG is rasterized
> at exactly its on-screen device size (toolbar 20 px, feature list 17 px, tabs 20 px,
> rails 20 px), so draw on a 24-unit `viewBox` (or the 20-unit Onshape grid scaled to 24)
> with pixel-aligned strokes. Use Onshape's palette: `#333333` outlines, `#ffffff` lit
> faces, `#999999` result faces, `#1651b0` accents, `#009400`/`#bd3039`/`#fab601` status.
> Menus (`PopupMenuItem::icon`) still use monochrome masks, so keep icons mask-safe:
> the silhouette must still read when every color becomes one tint.

> **Never copy Onshape's files or path data** (the repo is public). Look at Onshape's icon, then draw
> your own geometry on the grid below. `cargo test -p noshape --test no_onshape_assets` fails, naming the
> file, if any SVG in the workspace contains path data from Onshape's icons or any asset file is a copy of
> an Onshape file. It compares against hashes only (`crates/app/tests/onshape_path_hashes.txt`). The list
> is regenerated outside the repo with `/root/icon-guard/make.sh`.

24x24 SVG icons for noshape, in Onshape's style (docs/ONSHAPE_UI_SPEC.md section 6).

Usage: `assets/icons/<name>.svg`.

## Part Studio features (70)

`sketch`, `extrude`, `revolve`, `sweep`, `loft`, `thicken`, `enclose`, `fillet`, `chamfer`, `face-blend`, `draft`, `rib`, `shell`, `hole`, `external-thread`, `boolean`, `split`, `delete-part`, `transform`, `linear-pattern`, `circular-pattern`, `curve-pattern`, `mirror`, `delete-face`, `move-face`, `replace-face`, `offset-surface`, `fill`, `move-boundary`, `boundary-surface`, `ruled-surface`, `helix`, `projected-curve`, `bridging-curve`, `composite-curve`, `intersection-curve`, `fit-spline`, `offset-curve`, `plane`, `mate-connector`, `point`, `variable`, `variable-studio`, `import`, `wrap`, `sheet-metal`, `flange`, `hem`, `tab`, `bend-relief`, `corner-break`, `frame`, `gusset`, `end-cap`, `modify-fillet`, `body-draft`, `isocline`, `isoparametric`, `routing-curve`, `edit-curve`, `trim-curve`, `mutual-trim`, `constrained-surface`, `form`, `joint`, `jog`, `frame-trim`, `cut-list`, `configuration`, `feature-folder`, `feature-studio`, `custom-feature`

## Sketch tools (37)

`line`, `polyline`, `midpoint-line`, `corner-rectangle`, `center-rectangle`, `three-point-rectangle`, `aligned-rectangle`, `polygon-inscribed`, `polygon-circumscribed`, `circle`, `three-point-circle`, `center-point-arc`, `three-point-arc`, `tangent-arc`, `ellipse`, `elliptical-arc`, `conic`, `spline`, `bezier`, `slot`, `point-sketch`, `construction`, `text`, `image`, `sketch-fillet`, `sketch-chamfer`, `trim`, `extend`, `split-sketch`, `offset`, `sketch-mirror`, `sketch-linear-pattern`, `sketch-circular-pattern`, `transform-sketch`, `use`, `intersection`, `dimension`

## Sketch constraints (16)

`coincident`, `concentric`, `parallel`, `tangent`, `horizontal`, `vertical`, `midpoint`, `normal`, `pierce`, `perpendicular`, `equal`, `symmetric`, `fix`, `curvature`, `collinear`, `driven-dimension`

## Assembly (24)

`insert`, `instance`, `fastened`, `revolute`, `slider`, `planar`, `cylindrical`, `pin-slot`, `ball`, `parallel-mate`, `tangent-mate`, `width-mate`, `gear-relation`, `rack-pinion`, `screw-relation`, `linear-relation`, `group`, `replicate`, `explode`, `named-position`, `animate`, `interference`, `fix-instance`, `mate-connector-assembly`

## Drawing (24)

`drawing`, `base-view`, `projected-view`, `auxiliary-view`, `section-view`, `detail-view`, `broken-view`, `crop-view`, `linear-dim`, `angular-dim`, `radial-dim`, `diameter-dim`, `ordinate-dim`, `note`, `balloon`, `centermark`, `centerline`, `gdt`, `surface-finish`, `datum`, `bom`, `hole-table`, `title-block`, `sheet`

## View / app (74)

`view-cube`, `iso-view`, `front-view`, `top-view`, `right-view`, `zoom-fit`, `zoom-window`, `section-plane`, `measure`, `mass-properties`, `shaded`, `shaded-edges`, `wireframe`, `hidden-lines`, `perspective`, `orthographic`, `show`, `hide`, `isolate`, `transparency`, `appearance`, `material`, `part`, `surface-body`, `wire-body`, `part-studio`, `assembly`, `version-graph`, `branch`, `version`, `merge`, `undo`, `redo`, `search`, `settings`, `export`, `export-step`, `export-stl`, `new-document`, `open-document`, `save`, `document`, `folder`, `chevron-down`, `chevron-right`, `close`, `check`, `check-repeat`, `flip`, `flip-circular`, `rollback`, `suppress`, `unsuppress`, `edit`, `rename`, `delete`, `copy`, `paste`, `error`, `warning`, `info`, `under-defined`, `plus`, `minus`, `more`, `select-box`, `filter`, `eye`, `eye-off`, `lock`, `unlock`, `link`, `unlink`, `home`

## Claims: assembly / drawing / document-UI icon agent (Onshape match pass)

The assembly, drawing and document/UI icon agent owns these files. The Part Studio icon agent should leave them alone. Some icons stay with the Part Studio agent:

- The shared pattern icons (`linear-pattern`, `circular-pattern`, `mirror`). Onshape uses the same icon in both toolbars.
- `variable`, `configuration` and `mate-connector`.

- Assembly: `insert`, `instance` (same drawing as `part`), `sub-assembly`*, `fix-instance`, `mate-connector-assembly`, `fastened`, `revolute`, `slider`, `planar`, `cylindrical`, `pin-slot`, `ball`, `parallel-mate`, `tangent-mate`, `width-mate`, `group`, `gear-relation`, `rack-pinion`, `screw-relation`, `linear-relation`, `replicate`, `replace-part`*, `display-states`*, `explode`, `named-position`, `animate`, `interference`, `snap-mode`*, `show-mates`*, `part`
- Drawing: `drawing`, `base-view`, `projected-view`, `auxiliary-view`, `section-view`, `detail-view`, `broken-out-section`*, `crop-view`, `broken-view`, `linear-dim`, `ordinate-dim`, `angular-dim`, `radial-dim`, `diameter-dim`, `chamfer-dim`*, `arc-length-dim`*, `hole-callout`*, `note`, `balloon` (same drawing as `callout`), `callout`*, `gdt`, `datum`, `surface-finish`, `centerline`, `centermark`, `bom` (assembly BOM panel), `drawing-bom`* (drawing BOM table), `hole-table`, `table`*, `title-block`, `sheet`, `drawing-update`*, `export`
- Document and UI: the tab icons `part-studio`, `assembly`, `variable-studio` and `feature-studio`; also `plus`, `minus`, `version-graph`, `history`*, `recent`*, `undo`, `redo`, `search`, `menu`*, `front-view`, `back-view`*, `top-view`, `bottom-view`*, `left-view`*, `right-view`, `iso-view`, `zoom-fit`, `zoom-window`, `view-cube`, `variables-panel`*, `properties`*, `appearance`, `measure`, `mass-properties`, `check`, `cancel`*, `close`, `chevron-down`, `chevron-right`, `eye`, `eye-off`, `folder`, `delete`, `warning`, `error`, `info`, `document`, `new-document`, `open-document`, `settings`, `more`

`*` marks a new file.

### Conventions for this set (until the theme note lands)

- The glyph is drawn on Onshape's 20-unit icon grid inside `<g transform="scale(1.2)">`, in a 24x24 viewBox. At 20 px every 1-unit line falls on the pixel grid, the same as in Onshape. Render toolbar icons at 20 px for the closest match.
- Fixed colors from Onshape's icon tokens: outline `#333333`, accent `#1651B0`, success `#009400`, cancel `#BD3039`, warning `#FAB601`.
- Light tints are written as dark ink with opacity: `#333` at .58 gives `#888`, at .5 gives `#999`, at .2 gives `#D7D7D7`, and `#219DE3` at .5 gives `#90CEF1`. On a white background they match Onshape's values exactly. As GPUI single-color masks they keep their shape and come out as lighter alpha.
- No white fills. Hidden lines are removed with geometry or with a `<mask>` knockout, so a mask render never fills in solid. The exception is solid-color badges (`warning`, `error`, `drawing-update`), which lose their inner glyph or show it as a hole in mask mode.
- Generator script and comparison sheets were kept outside the repo (`/tmp/icons-ui/`).

## Claims: Part Studio icon agent (Onshape match pass)

The Part Studio icon agent owns these files: the Part Studio feature toolbar, the sketch toolbar and constraints, the feature list, the Part Studio right-side panels and the view-menu display modes. It doesn't touch the other set's files.

`aligned-rectangle`, `bend-relief`, `bezier`, `body-draft`, `boolean`, `boundary-surface`, `bridging-curve`, `camera`, `center-point-arc`, `center-rectangle`, `chamfer`, `circle`, `circular-pattern`, `coincident`, `collinear`, `composite-curve`, `composite-part`, `concentric`, `configuration`, `conic`, `constrained-surface`, `construction`, `corner-break`, `corner-rectangle`, `corner-relief`, `cube`, `curvature`, `curve-pattern`, `custom-feature`, `cut-list`, `decal`, `delete-face`, `delete-part`, `derived`, `derived-mirror`, `dimension`, `draft`, `driven-dimension`, `edit-curve`, `ellipse`, `elliptical-arc`, `enclose`, `end-cap`, `equal`, `extend`, `external-thread`, `extrude`, `f-sphere`, `face-blend`, `feature-folder`, `featurescript-table`, `fill`, `fillet`, `fit-spline`, `fix`, `flange`, `form`, `frame`, `frame-trim`, `gusset`, `helix`, `hem`, `hidden-lines`, `hole`, `horizontal`, `image`, `import`, `intersection`, `intersection-curve`, `isocline`, `isolate`, `isoparametric`, `jog`, `joint`, `line`, `linear-pattern`, `loft`, `mate-connector`, `material`, `midpoint`, `midpoint-line`, `mirror`, `modify-fillet`, `move-boundary`, `move-face`, `mutual-trim`, `normal`, `offset`, `offset-curve`, `offset-surface`, `origin`, `orthographic`, `parallel`, `perpendicular`, `perspective`, `pierce`, `plane`, `point`, `point-sketch`, `polygon-circumscribed`, `polygon-inscribed`, `polyline`, `projected-curve`, `query-variable`, `refold`, `replace-face`, `revolve`, `rib`, `routing-curve`, `ruled-surface`, `section-plane`, `section-view-3d`, `shaded`, `shaded-edges`, `sheet-metal`, `sheet-metal-bend`, `sheet-metal-end`, `sheet-metal-loft`, `sheet-metal-make-joint`, `sheet-metal-rip`, `sheet-metal-table`, `shell`, `sketch`, `sketch-chamfer`, `sketch-circular-pattern`, `sketch-fillet`, `sketch-linear-pattern`, `sketch-mirror`, `slot`, `spline`, `split`, `split-sketch`, `surface-body`, `sweep`, `symmetric`, `tab`, `tag`, `tangent`, `tangent-arc`, `text`, `thicken`, `three-point-arc`, `three-point-circle`, `three-point-rectangle`, `transform`, `transform-sketch`, `transparency`, `trim`, `trim-curve`, `unfold`, `use`, `variable`, `variable-table`, `vertical`, `wire-body`, `wireframe`, `wrap`

New in this pass: `composite-part`, `cube`, `decal`, `derived-mirror`, `f-sphere`, `featurescript-table`, `origin`, `query-variable`, `section-view-3d`, `sheet-metal-bend`, `sheet-metal-end`, `sheet-metal-make-joint`, `sheet-metal-rip`, `sheet-metal-table`, `tag`, `variable-table`.

### Conventions for this set

- The grid is the same as the set above. Glyphs sit on Onshape's 20-unit grid inside `<g transform="scale(1.2)">`, in a 24x24 viewBox. Horizontal and vertical 1-unit lines are centred on n+.5, so they land on whole pixels at 20 px. 2-unit constraint strokes are centred on integers.
- Strokes: outlines 1, inner and secondary edges 0.75, curve and sketch-tool strokes 1.5, constraint strokes 2. These are 20-unit values.
- Colors and mask-safety follow the set above. Outline is `#333333` and accent `#1651B0`. Light tints are `#333` (or `#219DE3`) with opacity, and there is no white paint. Hidden geometry and white details (cut-outs, arrow halos, hollow sketch endpoints) are `<mask>` knockouts.
- Bodies use Onshape's oblique view, with the depth going up and to the left. Affected faces are `#999`. Sheet-metal faces are light gray with `#999` bend zones. Sketch endpoints are hollow blue rings, and the geometry underneath them is masked out.
- The generator and comparison sheets live outside the repo, in `/tmp/icons-ps/`.
