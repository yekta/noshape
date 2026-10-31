# Dark mode (Preferences › Theming) vs live

ui2 owns it (lead 10-22). Live: behaviours "Dark theme (ui 20 Oct)", dom/dark-tokens-table.md, dom/dark-os-vars-computed.json,
dark/dk-9xx (1:1 captures). **Done = every studio and dialog pixel-close to these captures** (lead condition).

## Mechanism (in main once merged)
- `theme::PALETTE_DARK` + `theme::p()` (palette roles); `theme::DARK_HEX` through the patched gpui `rgb()` / `rgba()`
  (one relaxed atomic load in light mode, binary search in dark, no allocation); `gpui_kit::with_light_colors` for
  parts that stay light (drawing editor); `gpui_kit::rgb_static` for `--static` colours.
- Typing cost (UI thread, 200 keys into the feature filter, --demo): before the hook 4.90 / 4.90 / 4.70 ms per key;
  with the hook, same binary, alternating: light 5.10 / 5.85 / 5.60 / 5.30 / 6.00 / 6.50 (mean 5.73),
  dark 5.60 / 5.15 / 4.60 (mean 5.12). Run-to-run noise (±0.7 ms, shared machine) is larger than any hook cost;
  dark is not slower than light, and light pays only one relaxed load per colour.
- Step 2 (viewport): `noshape_scene::palette::{set_dark, is_dark, pick, viewport_dark, background}`; the
  scene builder picks dark plane fill/outline, origin, sketch full/over/driven, sketch regions, connector axes;
  the viewport background is #333 and the view cube dark (faces #333, frames/arrows #999, labels #fff); plane
  labels white. Measured vs dk-929: bg #333 = #333; one plane #3d = #3d; two #44 vs #46; plane edge peak
  #86–#8a vs #84. Saving the theme in Preferences re-sets the graphics global, so part studios and assemblies
  rebuild their scenes at once (no reload, as live).
- Step 1 state (dk0 shot): chrome, panels, tabs, filter and toolbar background go dark; still open: icons
  (dark glyphs, need the dark asset path from icons2), viewport background and plane colours, then per-capture
  side by side below.

## Side by side (live capture | ours | notes | status)
| capture | ours | notes | status |
|---|---|---|---|
| dk-910-documents | /tmp/ui2/dkdocs.png | page #333, navbar #1c1c1c, left nav, list, details, footer: same colours | ☑ colours |
| dk-911-documents-row-hover | dkdocs | hover = `--os-hover-primary` #0a4780 (the capture shows no pointer on a row) | ☑ |
| dk-912-documents-row-selected |  | selected row `accent_soft` #134e96 = live | ☑ |
| dk-913-create-menu | dkcreate | open Create #769fcc, menu #333 with #d7d7d7 outline; icons are icons2 dark art; "Import from" is cloud | ☑ colours |
| dk-913b-create-menu-hover | psui-scratch d4c | Create keeps #769fcc while its menu is open and the pointer is in it (os_ui::dropdown_open); back to #1651b0 after Esc / click outside | ☑ psui 10-31 |
| dk-914-new-document-modal | dkdocs | the capture is the documents page without a modal (Create #1651b0 closed): = dk-910 | ☑ psui 10-31 |
| dk-915-state | psui-scratch rb | Create's focus look (live request 1669): #769fcc + 2 px #3e94ff ring with its menu open and after Esc closes it, until Tab or a press elsewhere (measured: open / Esc lit, Tab / outside click #1651b0); not themed | ☑ psui 10-31 |
| dk-920-partstudio | dk2/dk3 | viewport #333, planes, cube, navbar, toolbar, rail #2c2c2c, panel #333 = live; toolbar icons = live dk-921 (icons2, re-checked 10-26) | ☑ |
| dk-921-partstudio | psui-scratch d1 | wordmark "noshape" white (was #4a4a4a), tab bar + #ccc | ☑ psui 10-31 |
| dk-922-feature-hover | psui-scratch g2 | the capture shows no hover; live dk-9c0: row #0a4780 = ours (measured 10,71,128) | ☑ psui 10-31 |
| dk-923-feature-selected | psui-scratch g1 | selected row #134e96 = ours (19,78,150) | ☑ psui 10-31 |
| dk-924-feature-rmb | dkrmb2 | menu #333, #ccc outline (static), #ddd separators, Delete ⊗ (masked icons keep their mask) | ☑ |
| dk-925-tool-tooltip | dktip | title #4b5761, body #333, white text; the description text is ours (tool_text.rs policy) | ☑ colours |
| dk-926-feature-dialog | dkext2 | dialog #333, active query box #142434 + #3e94ff border, related row #092046, selected #134e96 = live | ☑ |
| dk-927-variable-table | dkvt5 | title #90cef1 (emphasis) over #3e94ff, bars and headers #555 (table_header), cells #1c1c1c (bg_senary) | ☑ |
| dk-928-configurations | dkcfg | Configure Part Studio button, empty state; strip #1c1c1c with #aaa outlines while a panel is open | ☑ |
| dk-929-face-selected-measure | dk3 | bg, planes 1/2 stacked, plane edges, origin, cube = live within a few levels | ☑ |
| dk-930-face-hover | psui-scratch d2 | right strip with a panel open: the open button #333 joining its panel, the others #1c1c1c (measured 51 / 28) | ☑ psui 10-31 |
| dk-931-sketch-line | dksk2 | under-defined #3e94ff, sketch box #90cef1, white origin; regions #f2f2f2 at 20% (#616161 on a plane) | ☑ |
| dk-932-sketch-dimension | psui-scratch p2 | the edited dimension's lines white (they were black: scene sketch_mode used the light palette), hovered #fcda7f / selected #dba517 / driven #ccc (dark tokens); the box #333 with live's #ccc outline, selected text #0a3ea8, its ✕ #bd3039 (measured = live) | ☑ psui 10-31 |
| dk-933-sketch-after |  | dimension values white, driven #ccc, conflicts #ff6b6b | ☑ |
| dk-940-assembly | dkasm | chrome, list and viewport match; toolbar icons = live (icons2, re-checked 10-26; the Search tools box sits ~2 px right: ui) | ☑ |
| dk-941-mate-dialog | psui-scratch h2 | dialog / title #333, active Mate connectors box #142434 with a #3e94ff border = live (Revolute, no picks) | ☑ psui 10-31 |
| dk-942-bom | psui-scratch h5 | BOM header row and bar #555, cells #1c1c1c = live; the assembly strip as the Part Studio's (open #333 joining the panel, others #1c1c1c; was a blue row with an accent bar) | ☑ psui 10-31 |
| dk-950-drawing | dkdraw | shell dark (navbar, rail, tab bar), toolbar, sheet surround and sheet light | ☑ |
| dk-951-drawing-properties | qa c82818b7 | selects light = live; unticked checkboxes solid #333 and their labels #ccc (live white box, #333 label); Sheet properties select values white on white (DARK-DRAWING-PANEL-CONTROLS) | ☑ fixed dx, verified fbd2536c (qa 10-30 15:07: Sheet properties values dark on white with ▾; Drawing properties checkboxes white boxes, #333-like labels) |
| dk-960-feature-studio | dkfs2 | Ace twilight on #333: tokens, gutter #232323, numbers #e2e2e2 = live computed | ☑ |
| dk-961-variable-studio | psui-scratch v2 | layout, title bar / headers #555, cells #1c1c1c, Insert buttons = live (the capture exists: dark/dk-961); residual: live's ▾ in Variable type is 8 px wide, 6 px from the cell's right, ours 6 px wide a little further in | ☑ psui 10-31 |
| dk-971-sheetmetal-partstudio-toolbar |  |  | ☐ |
| dk-972-sheetmetal-dropdown-open |  |  | ☐ |
| dk-973-sheetmetal-table-flatview |  |  | ☐ |
| dk-974-ctx-part-in-parts-list | psui-scratch pm1 | menu #333, #ccc outline, white items, icons and separators = live; ours has no Add comment / Release Upgrade badge (cloud), so it fits under the click where live's flips above | ☑ psui 10-31 |
| dk-975-ctx-face-graphics | psui-scratch fm3 | items, icons, colours = live (Add comment is cloud); a menu taller than the room under the pointer now opens above it, clamped to the window, as live (it shifted up just enough before; vendor gpui-pre `switch_anchor_or_snap`) | ☑ psui 10-31 |
| dk-976-ctx-sketch-line | psui-scratch sl2 | items and colours = live (Add comment is cloud); Extrude… / Revolve… / Delete sketch entity now carry their icons as live (light too) | ☑ psui 10-31 |
| dk-977-dialog-hole | psui-scratch h2 | dialog, tabs, query boxes, selects, value rows = live; the hole table's level names (Hole type, Drill size…) stay #999 in dark as live (they were #d7d7d7) | ☑ psui 10-31 |
| dk-978-dialog-fillet | psui-scratch o978 | dialog, tabs, query box, selects, value row, checkboxes, slider = live (222 px wide with a fresh config; darkcfg holds a widened dialog) | ☑ psui 10-31 |
| dk-979-dialog-linear-pattern | psui-scratch cdlg | title red, ✓ / ✕, type select, New / Add / Remove / Intersect, active query #142434 + #3e94ff, other fields, rows, slider and ? = live | ☑ psui 10-31 |
| dk-980-dialog-mirror | psui-scratch cdlg | title red, ✓ / ✕, type select, New / Add / Remove / Intersect, active query #142434 + #3e94ff, other fields, rows, slider and ? = live | ☑ psui 10-31 |
| dk-981-dialog-sweep | psui-scratch cdlg | title red, ✓ / ✕, type select, New / Add / Remove / Intersect, active query #142434 + #3e94ff, other fields, rows, slider and ? = live | ☑ psui 10-31 |
| dk-982-dialog-sm-flange |  |  | ☐ |
| dk-989-tab-manager-open / dk-990-tab-manager-detailed / dk-991-tab-manager-sort-open | psui-scratch c989 / c990 / c991 | header icons, search, type filters, Clear, detailed rows with thumbnails (live's default; dk-989 is its persisted list view), sort menu = live | ☑ psui 10-31 |
| dk-992-panel-variable-table | psui-scratch vt | title, section bar, headers #555, cells #1c1c1c, add row = live (live's document has Variable Studios inserted, ours none) | ☑ psui 10-31 |
| dk-993-panel-appearances |  | live's studio has a sketch, no parts: "No parts or surfaces in this document" (ours hides the button only with neither parts nor sketches) | ◐ |
| dk-994-panel-custom-tables | | needs a studio with a FeatureScript custom table | ☐ |
| dk-995-panel-configuration | dkcfg | the same empty state as dk-928 (checked) | ☑ psui 10-31 |
| dk-996-left-document-notes | psui-scratch dn / rail-* | panel, header icons, empty state = live; the active rail button now as live in both themes: icon and bar #1651b0 on #eaeaea (light, document/rail-panel-document-notes), #90cef1 on the rail's #2c2c2c (dark); it was a blue-tinted cell with the icon unchanged | ☑ psui 10-31 |
| dk-997-left-where-used / dk-998-left-performance | psui-scratch cleft | panels, sections, table and icons = live (Where used is ours: live's Free account shows nothing) | ☑ psui 10-31 |
| dk-999-properties-dialog | psui-scratch pr / rs1 | modal, header, fields, footer = live; "Reset all" disabled while nothing differs from what it restores (was always enabled; light too, part-studio/1840). Live's 1172 px width is a resized modal (it resizes and keeps its size; default 800 × 542 as dom/dialog-partstudio-part-properties.txt); our release-create icon is local release management (live's Free account has none) | ☑ psui 10-31 |
| dk-9a0-versions-history-branch / dk-9a1-history-branch-rowmenu | psui-scratch c-9a0 | panel, search, toggles, graph = live; the current workspace row #092046 in dark (it was #134e96; light stays #b2ddf6); the filter ▾ shows once branches exist (live too); row menu not compared | ◐ psui 10-31 |
| dk-990-restored-light | psui-scratch t8 | Preferences › Theming › Light mode › Save theme, no restart: chrome, panels, viewport and planes light at once; the view cube kept its dark faces and axis letters until a hover (fixed: the viewport re-sends it on the theme change; = a fresh light start) | ☑ psui 10-31 |

## Plan notes
# Dark mode (lead 10-22: ui2 owns it; coordinate in ui's handoff theme-layout.md)

Live: behaviours "Dark theme (ui 20 Oct)", dom/dark-tokens-table.{md,json} (287 themed tokens), dark/dk-9xx.
Switch applies immediately (no reload). Drawings: only the shell is dark (toolbar, sheet surround, sheet, Drawing
properties stay light). Feature Studio: ACE twilight colours. `--…--static` tokens keep light values.

## Layers
1. **Palette** (theme.rs, ui's file): `PALETTE_DARK` with each field's dark token (comments name the token),
   `theme::set_dark(bool)` (AtomicBool), `p()` returns the active one. ~46 fields; values from the token table:
   app_bg #333, panel #333, panel_raised #333, panel_hover #666, panel_active #555, tool_hover #666,
   tool_hover_group #555, tool_active #134e96?, rail #2c2c2c (left strip measured (44,44,44)), focus #3e94ff,
   text #fff, text_muted #ccc, text_faint #999?, accent #3e94ff, accent_soft #134e96 (selected row (19,78,150)),
   row_hover #0a4780, tooltip …, tab_bar #555 (measured (81,81,81)), tab_inactive …, menu_outline #ccc,
   dialog_bg #333 (95%?), info_fill #094174 / #142434, border #555, border_strong #888, search_fill (85,88,96)…
   Each from the table, then checked against dk-9xx pixels.
2. **Hard-coded literals** (~1800 `rgb(0x…)` / `rgba(0x…)` across every agent's files): a hook in the patched
   gpui (`vendor/gpui-pre/src/color.rs`): `set_hex_theme(Option<fn(u32) -> u32>)` consulted by `rgb()` / `rgba()`
   (alpha kept), plus `with_light_colors(|| …)` (thread-local depth) for subtrees that stay light (drawing
   canvas + its toolbar/panels, the sheet; images; viewport-matched swatches such as appearance colours).
   The map = the token table's majority light→dark value for each light hex (37 of 70 light hexes are
   ambiguous: #fff is background (→#333) and text-on-blue (stays #fff); #1651b0 accent (→#3e94ff) vs primary
   button fill (stays)). Ambiguous call sites switch to `theme::fixed(hex)` (= Onshape's `--static`), found by
   comparing every dk-9xx capture side by side.
3. **Icons:** icons2 captured dark icon sources (icons2-dark-*.png, -sprite.svg): mono icons recolour via
   their `currentColor` / fill → icon tint from the palette; check icons2's notes (behaviours 6244…).
4. **Viewport:** background stays per live (the graphics area #333? live "graphics background … #333";
   default planes fill (76,76,76) edges (100,100,100)) → scene palette `viewport` dark set (noshape_scene).
5. **Feature Studio** ACE-like colours: twilight token set in feature_studio.rs highlighting.
6. Refresh: `set_dark` + `cx.refresh_windows()`; cached slices must re-render (notify every view: add a global
   theme epoch the Slice cache keys on).

## Steps
a. palette + toggle + Preferences Theming wiring (pref `dark_mode`), b. gpui hook + majority map + light scope for
drawings, c. side-by-side pass over dk-910…961 fixing exceptions per area, d. viewport + FS + icons.
Tell ui in theme-layout.md before touching theme.rs / vendor color.rs.

## Icons (detail)
Live: one sprite for both themes; colours from `--os-icon-*` vars (outline-primary #333→#fff, fill-primary
#fff→#333, accent-primary #1651b0→#3e94ff, accent-tertiary #333→#fff; `--static` keep light; full ~70 vars per
theme in dom/icons2-dark-icon-fills.json). Ours: literal fills/strokes (#333 ×985, #000 ×426, #fff ×196, #1651B0
×149 …). → `AppAssets::load("icons/dark/<name>.svg")` = the light SVG with colours rewritten by that var map
(per-icon exceptions from icons2-dark-icon-fills.json where an icon uses a --static var); `assets::icon()` /
`icon_path()` return the dark path while dark (separate paths keep GPUI's raster cache per theme). Tell icons2.

### Step 3 (built): how dark icons load — for icons2
- `assets::icon()` in dark mode (not inside the drawing editor) rasterises `crates/app/assets/icons/dark/<name>.svg`
  when it exists, else the light SVG with the themed colours flipped (#333/#000 → #fff, #1651b0 → #3e94ff);
  the raster cache is keyed per theme. `svg()` mask icons are tinted by the palette and need nothing.
- The flip is right for icons whose Onshape symbol uses only themed vars. Where a symbol uses `--static` vars
  Onshape keeps the light art in dark (white faces, #333 outline, #999 secondary: dark/icons2-dark-*-2x.png),
  and our art has no white faces, so those need drawn `icons/dark/` art. From icons2's work/map.json and the
  live sprite's var names (scratch /root/ui2-scratch/dark-icon-categories.json):
  - all `--static` (74): appearance, bend-relief, body-draft, bom, chamfer, composite-part, constrained-surface, corner-relief, cut-list, decal, draft, drawing, drawing-no-views, drawing-update, enclose, end-cap, error, external-thread, extrude, face-blend, fillet, flange, flat-pattern, folder, form, frame, gusset, hem, hole, inserted-version-update, instance, intersection-curve, isolate, isoparametric, joint, linear-pattern, list-error-filter, loft, mate-connector, mate-connector-assembly, mate-connector-qlv-item, mc-select, menu-delete, mirror, msg-error, msg-warning, offset-surface, part-mesh, performance-info, performance-warning, regen-error, remove-circle, replace-face, rib, sheet, sheet-metal, sheet-metal-loft, sheet-metal-make-joint, sheet-metal-table, shell, split, standard-content, sub-assembly, sweep, tab, thicken, toast-error, toast-info, toast-success, toast-warning, update-all-button, view-cube, warning, wire-body.
  - mixed static + themed (92): add-appearance, aligned-rectangle, assembly, bezier, boolean, boundary-surface, center-point-arc, center-rectangle, cfg-last-viewed, cfg-reset-default, circle, circular-pattern, config-show, configuration, configuration-visibiliity, conic, corner-rectangle, create-partstudio-incontext, create-variable, curvature, curve-pattern, delete-face, delete-part, derived, derived-mirror, edit-curve, ellipse, elliptical-arc, explode, fastened, featurescript-table, fill, flip, flip-circular, flip-circular-opposite, flip-direction, flip-direction-opposite, frame-trim, fs-snip-op, hole-depth, hole-diameter, hole-drill-angle, hole-table, insert-variable, insp-default-tol, inspection-table, intersection, jog, line, menu-create-drawing, midpoint-line, modify-fillet, move-boundary, move-face, mutual-trim, named-position, part, part-studio, paste, point-sketch, polygon-circumscribed, polygon-inscribed, realign-mate-dialog, replace-part, replicate, revolve, ruled-surface, sheet-insert, sheet-metal-bend, sheet-metal-end, snap-mode, spline, split-sketch, stack-closest, stack-furthest, sub-assembly-rigid, surface-body, tangent-arc, text, three-point-arc, three-point-circle, transform, under-defined, use, user-avatar, variable-studio, variable-table, variables-panel, wrap, xyz-along-x, xyz-along-y, xyz-along-z.
  - literal colours, no vars (27): branch, chevron-right, connectivity-analysis-g0, connectivity-analysis-g1, connectivity-analysis-g2, connectivity-analysis-g3, connectivity-analysis-no-connection, corner-break, cube, f-sphere, history, mbd-status, note, performance, plane, properties, recent, release-drafts, src-created, src-owned, src-public, src-recent, src-shared, tab-search, tm-detailed, tm-sort, tool-caret.
  - no sprite reference (99): check against live dark crops.
  - themed only (256): the flip should match; spot-check.

### Preferences (prefs/dk-904)
Primary buttons stay #1651b0 in dark (kit theme pinned; live Create, Share, Try Professional); a disabled one is
#1651b0 at 40% (#283f65, text #858585: button_disabled roles for the solid ones, static colours under the 40%
opacity ones); "My account" in emphasis #90cef1; links #b2ddf6 (link role); the beta pill static #1651b0.
The (?) help icon: dark/help-gray.svg, solid #d6d6d6 as live (icons2, 4722be7c).

## Lead conditions (10-22)
- Light mode: one relaxed atomic load in rgb()/rgba() (drafts/color_hook.rs: `HEX_THEME_ON` + `#[cold]` slow path).
- Dark: static sorted `(light, dark)` array, binary search, no allocation.
- Measure typing-frame cost before/after: script `typing-bench.sh`: app on --demo, focus the feature filter box,
  `xdotool type --delay 30` 200 chars, read utime+stime of the app (/proc/<pid>/stat, ticks) before/after → CPU ms
  per keystroke; 3 runs each: (a) before the hook (current main), (b) hook in, light, (c) hook in, dark.
  Also NOSHAPE_PERF=1 logs if they carry a UI frame stage.
- Done = every studio and dialog side by side with dark/dk-9xx captures, pixel-close (keep a table in
  docs/parity/dark-mode.md with each capture, our crop, diff notes).
- Baseline (10-22 ~05:40, main 5bd50b2 + WIP, before the hook): UI thread 4.90 / 4.90 / 4.70 ms CPU per key
  (200 keys into the feature filter, --demo; whole process ≈ 120 ms/key = llvmpipe, not comparable).

