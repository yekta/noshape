# icons2: one-to-one Onshape icon redraw (2026-10-21)

## State (2026-10-21 07:30, new icons2 on Stablecog): rounds 1-4 in the repo working tree, waiting for the lead to integrate

- Goal: every SVG in crates/app/assets/icons looks like its Onshape counterpart at 20 px and 40 px.
  All drawings are our own hand-authored geometry; `cargo test -p noshape --test no_onshape_assets` must stay green.
- Scratch (outside the repo): `/root/icons2-scratch`
  - `tools/rlib.py` render helpers (resvg); `tools/rank.py` scores every icon against its reference at 20 and
    40 px and writes ranked sheets (`sheets/rank-NN.png`, `work/rank.txt`); `tools/iter.py NAME` one-icon panel
    (pixel grids, diff, mask preview); `tools/guard.py` fast copy check against the repo hash list;
    `tools/show.py` reference browser; `tools/reviewsheet.py` / `automap.py` built the name->reference map.
  - `work/map.json`: our icon -> Onshape reference (`sprite:<symbol>` from ui-ref/icons-svg, or a PNG).
    466 of 644 icons have a sprite/help reference; the rest (hatch swatches, weld glyphs, misc UI) get live crops.
    Per-group corrections in `work/override-<group>.txt`.
  - `before/` = snapshot of the repo icons at the start (2026-10-21); `redraw/` = new versions.
  - `BRIEF.md` = rules + workflow given to the redraw agents; `groups/*.txt` = their icon lists.
- Conventions kept: 24 viewBox with `scale(1.2)` 20-unit grid (other viewBoxes left as they are), no white paint
  (menus render icons as masks), tints as #333 with opacity.

## Next
- Collect the group reports (`work/reports/*.md`), review before/after sheets, fix stragglers.
- On GO: diff `before/` against the repo first (other agents may have changed icons meanwhile), copy `redraw/`
  in, run the guard test and clippy, check in the app (display :115) next to live crops.

## Redraw agents (motile, started 2026-10-21; each writes work/reports/<group>.md)
| group | icons | account | agent id |
|---|---|---|---|
| g1-sketch-assembly | 76 | Ngyekta | 446d3a62-fd39-48ee-af2a-acdc3ef649f3 |
| g2-part-a | 61 | Ngyekta | 94bbb13f-dec5-414f-88c1-4a8787bc4be3 |
| g3-part-b | 60 | Motile | 7bf4f1b7-3674-4263-9a27-ca4b1ff48e3e |
| g4-drawing-a | 45 | Motile | 01538207-c166-4565-9c93-2f3c08d85627 |
| g5-drawing-b | 45 | Unbind | 2dab3866-7c3a-4bf4-b792-ea50c5c340e6 |
| g6-ui-a | 60 | Unbind | 85e8a985-9227-4018-90b8-ce2003b72f63 |
| g7-ui-b | 60 | Stablecog | a56920ac-de08-4905-8c33-9344542ffa37 |
| g8-ui-c | 59 | Stablecog | 1df7f7e6-61a2-4044-9095-7f839123e5bd |
| g9-hatch-weld | 97 | Yekta | d2e48857-70c7-4afd-9635-f3c4e6c70e08 |
| g10-unmapped-misc | 81 | Yekta | ff7f7d52-85ae-4ea1-a2b8-100652b4f4f4 |
- `tools/integrate.sh [--apply]` (on GO only): normalises square 20-viewBox icons to the 24 grid (assets::icon scales intrinsic size by px/24, so 20-unit icons came out at 5/6 size), guard + lint, checks the repo copy is unchanged since `before/`, copies `final/` in.

## Rules added by the lead (2026-10-21), in BRIEF.md items 6-7 and sent to all redraw agents
- No Onshape logos, wordmarks or brand marks, ever (trademark): empty-state/splash art and logos get noshape's own
  mark in the same layout. Before copy-in, review every icon whose reference is a logo/empty-state symbol by eye.
- Every path drawn by hand by our agents; no auto-tracing of Onshape raster or vector; guard stays green.
- After GO: verify in the app that toolbar (20 px), feature-list (17 px), menu and panel icons come out at
  Onshape's exact pixel sizes (measure the crops against live captures), not just that they look right.

## Progress
- g5-drawing-b DONE: 45/45 redrawn, median 22.17 -> 1.64 (all < 4), guard 0. Ref fix: settings -> gear (view-cube
  menu). Known limit: drawing-update (yellow badge + dark arrow) is a solid disc under masks, as before.
- g4-drawing-a DONE: 45/45, median 23.4 -> 2.32 (41 < 5), guard 0. Ref fixes: note -> drawing-note (bold A),
  gdt -> drawing-gtol sprite.
- TODO (post-GO, cross-area, tiny): part_studio/mod.rs:179 maps importDerived and importForeign to "import"
  (the sketch Insert DXF/DWG icon). Onshape uses import-derived-button / import-foreign-button in the feature
  list. Plan: importDerived => "derived" (already redrawn), importForeign => new "import-foreign" icon (to draw).
- g2-part-a DONE: 60 redrawn + reorder-items-button already exact; median 27.8 -> 4.1, guard 0. Ref fixes: zoom-fit,
  mass-properties -> lui-* sprites; wire-body -> curve; filter -> filter-results-button; offset-curve ->
  offset-curve-on-face-button. Weak refs: front/back/left/right/top/bottom-view and transparency only have blurry
  CAM-Studio help crops (scores 8-24); these view icons are not referenced by name in the app code (low impact).
- New icon import-foreign drawn (score 3.24, guard 0; ref import-foreign-button). derived matches
  import-derived-button at 2.67. Both ready for the post-GO mod.rs:179 change.
- g6-ui-a DONE: 57 redrawn + 3 already exact; median 18.05 -> 0.94 (54/60 < 2.5), guard 0. documents-empty's
  "On" lettering replaced by noshape's cube mark (rule 6). Ref fixes: documents-empty -> details-panel-empty,
  custom-feature -> external-fs-feature, image -> sketch-image-rectangle-button, info -> document-details,
  help -> navbar-help, msg-info -> performance-info, toast-error -> performance-error (inferred).
- Checked: the mate dialog's "Flip primary axis" already uses `flip-direction-opposite`
  (assembly_mate_dialog.rs:611, assembly_panels.rs:577); nothing to change.
- g8-ui-c DONE: 57 redrawn + 2 already exact; median 20.45 -> 1.09 (52/59 < 5), guard 0. `document` keeps
  Onshape's page layout with noshape's cube in place of the "On" mark (rule 6; score 18 by design). The
  add-custom-features gear was redone from hand-written shapes after an early scripted attempt (rule 7). White
  fills removed from 7 icons. Ref fixes: show-preview, plus (live crops), document -> sprite:document,
  part-studio -> partstudio, section-view-3d -> lui-section-view.
- g9-hatch-weld DONE: 91 redrawn (54 hatch swatches, hatch-region, 36 weld glyphs), median 29.9 -> 6.5, guard 0.
  11 swatches had line positions first fitted by a script against Onshape pixels; script deleted, values
  re-entered by hand (rule 7). Noisy textures (concrete, wood, ...) match in tone, not pixel for pixel.
  22 weld joint glyphs (+ -below) are sized from the small sprite icons: requested live captures in
  live/REQUESTS.md; refine when they land.
- TODO (cross-area, drawing_weld.rs, for asb/drw2 after GO): the joint picker box should show Onshape's small
  upright icon (our weld-menu-*), not the reference-line glyph (incl. the mirrored -below one); the weld flag
  buttons sit ~7 px lower than Onshape's.
- g7-ui-b DONE: 56 redrawn + 4 already exact; median 20.2 -> 1.5 (51/60 < 5, worst 10.6), guard 0.
  Ref fixes: version -> create-version-button, delete -> clear-field-button (gray circled X; verified in
  live/menus/rmb-asm-list-instance.png), create-variable -> add-variablestudio-button, dialog-ok -> ok-button
  (white check on the green accept button; white kept on purpose, it is only drawn via assets::icon on green).
  menu-section-view moved from its 103-unit frame to the 24 grid.
- g10-unmapped DONE: 60 redrawn (59 with a reference: median 26.97 -> 4.73), 21 left unchanged (no Onshape
  counterpart: noshape-logo, refold/unfold/sheet-metal-rip, camera-menu display modes, suppress/unsuppress/
  rollback, interference, unused leftovers; tab-card-notch keeps its white border cover; ui-corner-mark exact).
  new-document uses noshape's cube instead of the "On" mark (rule 6). Synthetic refs (guesses, not captures):
  note-align-right/center/justify, note-valign-bottom.
- file-pdf: g10 had matched Onshape's Adobe Acrobat ribbon (a third-party mark); replaced by icons2 with a
  neutral hand-drawn "PDF" glyph in the same frame as the other file-tab icons.
- TODO (cross-area, post-GO size check): inspection-illustration shown 130 px wide (Onshape 172); import-arrow
  18 px (Onshape 23-24); anim-reverse glyph 22 (Onshape ~16); limit-* glyphs sit in the cell's right half as in
  Onshape, so the app's separate axis letter should overlap into the cell.
- g1-sketch-assembly DONE: 76/76 redrawn, median 22.6 -> 1.32, worst 4.29, guard 0. Ref fixes: assembly ->
  sprite:assembly, show-mates -> HoverMate, group -> mateGroup. A gear-parameter search script (scored against
  the reference) was deleted and rack-pinion reverted to hand-picked values (rule 7).
- Gotcha: resvg/tiny-skia draws strokes <= 1 device px as hairlines (lighter, 75% corner coverage). Onshape's
  icons are fills, so thin lines are drawn as filled outlines or stroke-width 1.01. The app renders with resvg
  0.45.1 / tiny-skia 0.11.4 (scratch tools use resvg CLI 0.48.1, same tiny-skia line): confirm in the app after GO.

## Copy-in (2026-10-21, after the lead's GO)
- All 608 redraws copied group by group (g1..g10) from `final/` (20-unit icons moved onto the 24 grid); the
  guard test, compiled standalone (`rustc --test crates/app/tests/no_onshape_assets.rs`, it has no deps) passed
  after every group. No repo icon had changed since the `before/` snapshot.
- Correction: GPUI `img()` defaults to ObjectFit::Contain, so the old 20-unit icons under `assets::icon()` were
  rasterised at 5/6 resolution and then stretched to full size (soft/blurry), not drawn at 5/6 size. The 24-grid
  move fixes that (native-resolution raster).
- Debug builds of the app read the icon SVGs from disk at runtime (rust-embed without debug-embed), so a running
  debug binary shows icon changes without a rebuild.
- In-app check vs live captures (Part Studio toolbar, feature list header, rail, assembly toolbar/list, context
  menu): ink extents identical to Onshape (toolbar 18x18 / boolean 20x14, plane 14x15, origin 7x7, add-folder
  18x14, stopwatch 16x18, filter 15x15, rail 16x16); mean pixel difference vs live 2.5-9 / 255.
- Fixes found in context: pause-regen followed a sprite that is the "resume" state; live shows "||" -> redrawn,
  now pixel-exact at 20 px. New icon add-mate-connector (sprite add-mateConnector, hand-drawn, 4.99).
- Code changes (minimal, other areas): part_studio/mod.rs importDerived -> derived, importForeign ->
  import-foreign; assembly_view.rs list-fixed/list-free/list-grounded drawn at 20 px (Onshape's size; at 14 px
  they were 10x8 vs Onshape's 14x12); assembly_behaviors.rs context menu: Fix -> fix-instance (hatched ground),
  Add mate connector -> add-mate-connector, Replace instances without icon (as Onshape), Create Drawing ->
  menu-create-drawing (sheet with blue +), Change to version -> legend-version (plain pin, as Onshape);
  workspace.rs drawing-tab Change to version -> legend-version.

- Verified: `cargo build -p noshape` OK; `cargo test -p noshape --test no_onshape_assets` 2 passed; clippy shows
  nothing from icons2's files (one warning seen once in feature_dialog.rs belongs to another agent's WIP).
- Before/after app screenshots, captured identically (Xvfb :115, 1919x1079 window, demo doc): context/before-[1-4]-*.png
  and after-[1-4]-*.png. "Before" = the pre-copy binary with `before/` bind-mounted over the icon folder in a
  private mount namespace (`unshare -m`), so the repo was never reverted. Side-by-side sheets with live Onshape:
  context/cmp-1..6-*.png (Part Studio toolbar, feature list, assembly toolbar, instance list, context menu,
  sketch toolbar). Capture script: /tmp/icons2/capture.sh (copy in this doc's scratch if /tmp is wiped).
- Totals (618 icons with an Onshape reference): median score 24.32 -> 2.58; 253 < 2, 461 < 5, 587 < 10.
  Ranked lists: work/rank.txt (before), work/all-after.txt (after); sheets/rank-*.png, sheets/all-after-*.png.

## Open items (not icon files)
- Part Studio toolbar layout: at 1920 px Onshape shows every tool ungrouped; ours still groups them into
  dropdowns, and a few header/rail buttons sit 1-3 px off (layout, ui agent).
- Drawing toolbar not compared in the app (the demo document has no drawing tab); drawing icons were checked at
  icon level only.
- Weld joint glyph sizes await the live capture requested in live/REQUESTS.md (icons2 line); weld dialog layout
  items for asb/drw2 listed above.
- Weak references (blurry help crops): front/back/left/right/top/bottom/iso-view, transparency (not referenced by
  name in the app); synthetic refs: note-align-right/center/justify, note-valign-bottom.

## Round 2 (2026-10-21, after the copy-in): drawing, menus, dialogs, documents page, Preferences
- Drawing (created from the demo Part Studio, compared with live drawing/01, 08): toolbar icons match except
  three tools that used Part Studio art. New hand-drawn icons: drawing-aligned-section (3.7),
  drawing-cutlist-table (10.4), drawing-custom-table (6.5), wired in drawing_toolbar.rs + drawing_keys.rs.
  Drawing toolbar carets: Onshape uses a filled 8x4 px blue triangle (#2767ae) -> new glyph drawing-tool-caret and
  ui::tool_caret_blue (drawing toolbar only). Create Drawing tiles: Onshape's art is a 55x40 symbol shown at 66x48;
  ours was 40 px -> drawing-suggested-views / drawing-no-views redrawn on a 55-unit square canvas (white sheet fill:
  only drawn via img on the tile, like dialog-ok), shown at 66 px with -3 px bottom margin (drawing_create.rs).
- Flip buttons: the redraw matched the sprite *names*, which inverted what dialogs show. Live (dialogs/extrude-*,
  circular-pattern-01): unflipped shows the art of flip-direction-opposite / flip-circular-opposite. Fixed the
  mapping in feature_dialog.rs (both places) and the fixed uses (draft_tool, mate HUD reorient, reflection,
  analysis tools) so they look as before. A mirror/rotation scan of all redraws vs before/ found no other
  state-pair inversion (sort, stack, note order, eye, lock, carets checked by eye).
- Menus vs live (feature list: extrude/sketch/plane/origin; parts list; face; edge; tab; view cube; hamburger;
  assembly instance): icons match. Fixed: plane/origin context menu "Hide" lacked the eye-off icon
  (part_studio/mod.rs). Remaining differences are content/layout, not icons (cloud-only "Add comment",
  hamburger offset + extra Print, sketch-menu contents).
- Dialog (Extrude) and Preferences: icons match (help circles identical). Preferences opens as an overlay, Onshape
  as a full page (layout -> ui).
- Documents page + other inline glyphs: 47 SVG constants in os_ui.rs / material_libraries.rs /
  preferences_window.rs (side nav, list views, Feature Studio...) were outside assets/icons. Agent g11 (Stablecog,
  710bbd05-4e7b-4759-95f4-bbaf22b36fde) is redrawing them in scratch (redraw/<file>--<const>.svg; integrate.sh skips
  '--' names) and checking cursor.rs art against live/cursors; icons2 writes the results into the Rust constants.
  material_libraries GLYPH_ONSHAPE imitates Onshape's hexagon logo -> replaced by noshape's mark (rule 6).
- Flag for the lead (not an icon): Create Drawing dialog's template source list is labelled "Onshape"
  (drawing_create.rs SOURCES) next to the noshape mark; under the trademark rule it should probably say noshape.
- Live requests filed (live/REQUESTS.md, icons2 lines): weld joint glyphs; sharp view-orientation, transparency and
  Note-panel alignment icons.
- Live answered both icons2 requests. Regenerated crates/app/tests/onshape_path_hashes.txt with
  /root/icon-guard/make.sh (19211 -> 19402 hashes, strict superset; the live agent saved Onshape mtext SVG sources
  under live/ui/icons2-src-*.svg, now covered). Guard green on repo + all redraws.
- View-orientation + transparency icons: Onshape only shows them in CAM Studio (paid plan), so no sharper
  reference exists; current redraws stay (and they are not referenced by name in the app).
- Weld glyph refinement -> g9 agent (d2e48857...), note-panel icons -> g10 agent (ff7f7d52...), both resumed.

## Round 3 (previous icons2, ~06:10, not logged before it stopped)
- Drawing Sheets panel: new hand-drawn projected/auxiliary/section/detail/broken-view-linked and drawing-standard-views
  (refs in work/override-icons2-r3.txt), wired in drawing_extra.rs (sheet panel tree). Verified 07:00: repo copies == redraw/.
- No half-copied files: every icons2 file in assets/icons equals its redraw/ version. Untracked cell-*.svg and
  aux-format-*.svg are another agent's (drawing cell panel / aux view), not icons2's.

## Round 4 (2026-10-21, new icons2 after the account move)
- g9, g10, g11 had stopped (session limits). g10 had finished (note-panel refs + redraws were already in). g9 and g11
  were finished by fresh agents: g9b 503c7d25-a831-4433-9985-b19c460e53e7 (Yekta), g11b
  b24cf24b-9e28-40e5-bf89-49bb12f2bd9b (Ngyekta). Reports: work/reports/g9-hatch-weld.md (appended),
  work/reports/g11-inline-glyphs.md, inline/cursors/NOTES.md.
- Weld (g9b): 26 joint glyphs (+ -below) refined against the live joint captures (median 48 px diff 0.94 -> 0.65), copied in.
  Picker box: Onshape shows the upright list icon (20 px, never mirrored) -> drawing_weld.rs `picker_icon()` used for the
  picker and the spacer (disabled) box; new weld-picker-spot / weld-picker-seam (smaller circle than the list icons).
  refs/g9-hatch-weld/tools/weld.py is superseded (re-running it would overwrite the refined files).
- Inline glyphs (g11 + g11b): all 47 SVG constants in os_ui.rs / material_libraries.rs / preferences_window.rs replaced
  (median 21.2 -> 4.6) with tools/inline_apply.py (checks each constant still equals inline/before, #333333 ->
  currentColor, br## when the SVG contains `"#`). Rule 6: material_libraries GLYPH_ONSHAPE (hexagon logo) -> noshape cube;
  GLYPH_EMPTY / GLYPH_DOCUMENT keep Onshape's layout with noshape's cube instead of "On".
- Cursors (cursor.rs, g11b NOTES): mac arrow outline and digit outline mitred, darker drop shadow (dx .5 dy 1.4 sd 1.1
  opacity 1, measured L131 vs ours 206), "5+" = "5" plus a thin separate plus. Crosshair / crosshair box / hot spots were
  already exact. Confirmation, RemoveSelection, SnapMate art unverified: live request filed (REQUESTS.md, icons2 line).
- Drawing toolbar vs live drawing/700-toolbar-1920 (side by side: /root/icons2-scratch/context/r4-dtb3-*.png): icons match
  except two tools Onshape draws differently from the Part Studio art -> linear-dim redrawn as Onshape's drawing Dimension
  (filled double arrow, 5.56), new drawing-line (grey hollow end diamonds, 5.24). Dimension ▾ list (live 312): new
  dim-point-to-point / dim-point-to-line / dim-line-to-line / dim-angular-3pt / dim-foreshorten (4.45 / 2.92 / 1.49 / 9.1 /
  5.09), wired in drawing_toolbar.rs and drawing_keys.rs. Overrides: work/override-icons2-r4.txt, refs in refs/icons2/.
- For the lead (not icons, other areas):
  * Drawing toolbar pitch: Onshape's drawing toolbar (Wt iframe) has 30 px buttons + 2 px gaps (32 px pitch, dom
    drawing-toolbar-widths.json); ours uses ui::TOOL_CELL 32 + 2 (34 px), so every drawing tool drifts right (ui2).
  * Crop view carries a ▾ (views_menu: jogged/aligned/partial section, crop shapes) that Onshape's 1920 toolbar doesn't have
    (asb/drw2).
  * Revision table button: on the captured (Free) account Onshape has none (behaviours.md "Revision table"); ours shows
    it between BOM and Cut list (asb decides; paid plans have it).
  * Empty-state art (os_ui GLYPH_EMPTY) is shown 150 px in #d7d7d7; Onshape ~166x150 in #eeeeee (ui2).
- Drawing properties panel (vs live drawing/160): "Update properties from a template" button showed title-block; Onshape
  shows a DWT page with an arrow -> new dwt-template (ref sprite drawing-insert-dwt, 4.06) at 20 px in drawing_drawprops.rs.
- Note panel (vs live ui/icons2-note-panel): icons match; the toolbar ▾ is already the 8x4 drawing-tool-caret; the font /
  height / flag combo carets were 4.8 px wide (ui-caret-down at 9.6) vs Onshape 8x5 -> 16 px in drawing_note.rs.
- In-app check (old 06:04 binary, icons load from disk): new Dimension icon identical to live; weld fillet glyph on the
  reference line identical to live weld-joint-fillet-other-raw. The picker/name changes need a fresh build (the shared tree
  failed in compare_mode.rs, another agent's WIP, ~07:20).
- Guard: `CARGO_MANIFEST_DIR=/root/noshape/crates/app rustc --edition 2021 --test crates/app/tests/no_onshape_assets.rs
  -o /tmp/icons2/guard/g && /tmp/icons2/guard/g` -> 2 passed (~07:25).
- More for the lead: Styles panel info box (live drawing/95-panel-styles) has a bold first line and #2a6496-ish text, ours
  regular (ui2); drawing right rail sits ~25 px lower than live (ui2); Inspection/drawing-props combo carets lighter/larger
  than live 8x5 #555 (ui2, shared dropdown style).
- Tab manager button (bottom-left, 20 px): live (every capture) is Onshape's tab-manager art (dark tab + magnifier), our
  tab-search followed the wrong sprite (tab-search-button, 38 off) -> redrawn by hand vs sprite tab-manager (4.22).
- Configurations (live part-studio/180, 181): the Configure button and its ▾ menu used the drawing "table" icon,
  checkbox-input and variable; Onshape uses its configuration list / checkbox / variable art (our cfg-filter-list /
  -checkbox / -variable, already exact) at 20 px -> config_panel.rs + assembly_config.rs (button, menus, summary filter
  toggles). The ▾ was 4x4 (ui-caret-down at 10) vs live 8x4 -> drawing-tool-caret at 12 px (all four config carets);
  Configure Part Studio caret column 22 -> 24 px (live).
- Checked, icons match: ➕ tab menu (document/01), Variable Studio (document/10), Versions and history (document/22),
  Appearances (part-studio/170), Fastened mate dialog (assembly/30).
- For the lead (not icons): Variable Studio has no right-rail "Variables" panel button (live document/10 x 1903); history
  panel shows an extra wrench button and an enabled Release (live: Release greyed with a lock on Free); the mate dialog
  flip button is drawn pressed (live: not pressed).
- Right rails (live part-studio/g45, 590, 121; assembly/30): the Part Studio "Inspection table" button is Onshape's
  model-based-definition art (sprite match 5.75; our mbd-status), not the checkmark grid -> config_panel.rs rail; the
  assembly "Configuration panel" button is the same blue-grid + cube as the Part Studio (ours showed config-show, the eye)
  -> assembly_view.rs rail uses "configuration". The drawing rail's Inspection table (checkmark grid) is right as it is.

## EXACT STATE at hand-off (2026-10-21 07:36 UTC, moving Stablecog -> Ngyekta)
- App quit; my cargo build stopped (it was queued behind others). No sub-agents running (g9b, g11b finished).
- NOT YET COMPILED by me (the shared tree failed ~07:20 in compare_mode.rs, another agent's WIP; none of the errors were
  in my files). All edits are plain string swaps / one small fn, made so each file still compiles:
  * drawing_weld.rs: new `fn picker_icon(WeldJoint)` (before `fn spacer_icon`), used by the joint picker and spacer box.
  * drawing_toolbar.rs / drawing_keys.rs: Line -> "drawing-line"; Dimension list -> dim-point-to-point, dim-point-to-line,
    dim-line-to-line, dim-angular-3pt, dim-foreshorten.
  * drawing_drawprops.rs: DWT button icon "dwt-template" at 20 px. drawing_note.rs: 3 combo carets ui-caret-down 9.6 -> 16.
  * os_ui.rs, material_libraries.rs, preferences_window.rs: 47 SVG constants (tools/inline_apply.py; br## where needed).
  * cursor.rs: 4 string changes from inline/cursors/NOTES.md.
  * part_studio/config_panel.rs + assembly_config.rs: cfg-filter-* icons, drawing-tool-caret 12 px carets, caret w 22->24
    (line ~726); config_panel.rs rail Inspection table -> "mbd-status". assembly_view.rs rail Configuration -> "configuration".
- Icons copied into the repo this round (all == /root/icons2-scratch/redraw and final/): 26 weld joint glyphs, weld-picker-spot,
  weld-picker-seam, linear-dim, drawing-line, dim-* (5), dwt-template, tab-search. Guard test green ~07:25 (standalone rustc), before the tab-search copy-in; re-run it.
- NEXT for the new icons2:
  1. `OCCT_ROOT=/root/occt CARGO_TARGET_DIR=/root/target-shared nice cargo build -p noshape` (check `free -g`); fix anything in
     the files above; then `cargo clippy -p noshape --all-targets -- -D warnings` for these files.
  2. Copy the binary to /tmp/icons2/nsa; run with `--demo-drawing-full` (drawing) / `--demo` on :115 (tools/app/run.sh etc.,
     copies in /root/icons2-scratch/tools/app/). Verify: weld picker box (upright 20 px icon, ~17.5 px from the box's left),
     drawing toolbar Line + Dimension ▾ list (vs live drawing/312), DWT button (vs drawing/160), note combo carets
     (vs live/ui/icons2-note-panel), tab manager button (vs document/10), Configure ▾ + menu (part-studio/180-181), rails,
     Feature Studio toolbar glyphs (feature-studio/70-toolbar-1920; New feature/import/format are the new inline glyphs),
     documents page side nav + material libraries (rule-6 cube), Preferences shortcut glyphs, count cursors ("5+").
  3. Remaining contexts not yet compared: mass properties / measure panel, sheet metal, import dialog, documents page after
     the inline glyphs, assembly explode/animate panels.
  4. Live request pending (REQUESTS.md, icons2): Confirmation / RemoveSelection / SnapMate cursor captures.

## Round 5 (2026-10-21 ~08:00, icons2 on Ngyekta)
- Build of the round-4 tree OK 07:51 (binary /tmp/icons2/nsa). Guard (standalone rustc) 2 passed.
- In-app vs live, all match: Part Studio rail (mbd-status, configuration), tab manager, Configure button icon, drawing
  toolbar (Line, Dimension), Dimension ▾ list (312), weld picker (fillet, upright 15x15 ink, identical), weld glyph on the
  reference line, DWT button (160), Note panel icons, Feature Studio toolbar glyphs (except import, below), documents side nav.
- Cursors (live request answered: live/cursors/icons2-*-{1x,2x}.png from Onshape's SVGs). cursor.rs redrawn by hand
  (scratch: /root/icons2-scratch/cursors/*.txt + tools/cursors.py, curdiff.py, countcmp.py, winarrow.py, sbs.py):
  * Confirmation: plain arrow at the count-cursor spot (was scaled 1.3), new 19x31 mouse (tapered, #009400 left button,
    wheel, #3c3c3c divider). RemoveSelection: minus 8x1.5 at 16,18 (was 9x2.6 at 14.5,19). SnapMate: new art (disc on a
    neck, stem, cup with socket + pin), arrow untranslated as in Onshape, hot 7.5 kept.
  * Shadow 0,1,1,0.8 (was .5,1.4,1.1,1: too dark under thin shapes). Chosen over 0.6 after a 1x check: best overall at 1x and
    2x (count mac "3" 2x 2.81 -> 1.70, 1x 2.25 -> 2.10; conf 5.6 -> 4.8; snap 6.4 -> 4.9; remove 1.78 -> 0.77).
  * Windows: new WIN_ARROW (thin #191d2f 1 px outline, Onshape's tail), own softer shadow (1,1,1.5,0.4); Windows art sits
    +1,+1 (count label +3,-5) on a 38 px canvas as in Onshape's win files. Count labels left-aligned at x 15 (text_path x0).
  * In app (X cursor read back with tools/xcursor.py, XFixesGetCursorImage; ffmpeg x11grab draws it ~2 px off, don't use it):
    count "1" and RemoveSelection are 32x32, hot 3,3, art in place (xccmp.py vs live 1x: 6.7 / 4.4, mostly AA + shadow).
    Confirmation / SnapMate share the same code path; checked as resvg renders only (triggering them needs a sketch
    pattern / assembly mate-connector drag).
- Note panel combo carets: Onshape's select caret is 8x6 #777 on whole pixels -> new icon ui-select-caret (8 px), used for
  the three combos in drawing_note.rs (#777; size combo pr 4 -> 6 so it sits 10.5 px from the border like live).
- Feature Studio Import (os_ui GLYPH_FS_IMPORT): g11 followed the Part Studio "derived" sprite; live (feature-studio/70) is an
  open box + curved arrow + grey funnel -> redrawn by hand from the 1x live look (6.07 vs live crop).
- Documents "create label" glyph: Onshape's plus is blue #1651b0 -> GLYPH_LABEL_ADD split into tag + GLYPH_LABEL_ADD_PLUS,
  new os::label_add_glyph(size, color), used at both start_window.rs call sites.
- For the lead (not icons):
  * Configure ▾ menu (part-studio/181): Onshape opens a dropdown anchored under the button (left-aligned, button width,
    28.5 px rows, 20 px icons with 16x16 ink); ours is the context OsMenu at the click point (22 px rows, 16 px icons).
  * Weld picker box (drawing_weld.rs dropdown Button): live icon at +10 px from the box's left, caret at +34; ours +6 / +38.
    Weld dialog also lacks Onshape's "Joint spacer" combo row at the top and the "Finishing" row (live weld-joint-*-raw).
  * Drawing properties footer (160): Onshape's "Update properties from a template..." is larger with an underline; "Lock
    drawing properties" is greyed (disabled) with a ? help icon at the right.
- Verified in app (binary built ~08:35): Note carets pixel-identical to live (shape, #777, positions); FS import glyph 6.0 vs
  live in place (14x16 ink); label-add plus renders #1651b0. `cargo check -p noshape` clean (no warnings); guard 2 passed.
- Assembly (vs live assembly/62, 181): toolbar icons, instance list, rail icons and the Explode tree row all match.
- NOT yet done: contexts never compared (mass properties / measure panel, sheet metal, import dialog, animate dialog
  assembly/53); win cursor variants only checked as renders (no Windows machine).
- Live request filed (REQUESTS.md, icons2): assembly rail 5th button (shapes + padlock) - identify it, then draw its icon.
- More for the lead (not icons):
  * Assembly toolbar pitch: Onshape 34 px between button centres, ours 32 (Search tools ends at 1088 vs live 1124).
  * Exploded view edit mode: our "Editing Explode 1" banner sits clipped at the far left (x 40-110, "ode 1"); live centres
    it in the toolbar row (x 868-1090, yellow, with a [Done] button) and keeps Undo/Redo at the left (assembly/62). Ours
    also hides Undo/Redo.
  * Assembly rail: live has 6 buttons (5th unidentified, see the live request); ours 5.
- Mass properties (vs live part-studio/121) and Measure (part-studio/495): the X/Y/Z rows used Unicode arrows (thin, the
  wrong colours). New hand-drawn icons axis-arrow-x/-y/-z (9x9 / 8x9 / 8x12 filled-head arrows, 6.8 / 5.3 / 3.2 vs live),
  measure-min-dist (bars + converging arrows, 5.0) and measure-max-dist (I-beam with outward heads, 4.2). menus.rs: mass
  rows = #999 15 px letter + arrow (#bd3039 / #009400 / #1651b0); measure rows pick the glyph by the parent row (Min dist ->
  converging, Max dist -> I-beam, else arrows), letters #999 15 px like live. Verified in app (~09:45 build): mass and
  measure glyphs match live 121 / 495.
- For the lead (not icons): mass panel X/Y/Z rows are 31 px apart in Onshape (ours 26) and its panel is ~47 px taller;
  analysis tools menu (bottom right) rows are 22 px in Onshape (ours 28) and ours lists an extra "Thickness analysis...".
- add-custom-features: g8 had followed sprite add-toolbar-feature (wrench + gear in the dashed frame); every live Part Studio
  toolbar (part-studio/121, 495) shows add-feature-type, the dashed frame with a 2 px plus ("Add custom features",
  behaviours.md "⊞") -> inner art replaced by hand (frame kept), 1.92 vs live 495. Also used by the [⊞ Custom features] button.
- For the lead: live part-studio/495 has the "Custom features in this workspace" {cube}▾ dropdown AFTER ⊞ (x 1675 vs ⊞ 1637);
  part_studio/mod.rs:1681 puts the document's custom features before ⊞ (citing live 341). Worth a second look (psb/fs).
- Sheet metal document (--demo-fab): Part Studio toolbar (ungrouped) icons all match live 495.
- For the lead (measure panel, live part-studio/495): Onshape lists Min dist before Max dist (ours Max first); its values use a
  monospace face (ours fall back to a proportional font despite font_family("monospace")).

## EXACT STATE at end of round 5 (2026-10-21 ~10:00 UTC)
- Not running: no app, no cargo, no sub-agents. Binary /tmp/icons2/nsa (09:45 build, has everything except the last measure
  letter tweak) can be deleted when not needed (456 MB).
- Files changed this round (all compile; last full build 09:45 + clippy ~10:00):
  * crates/app/src/cursor.rs (cursor art, shadows, Windows layout, text_path x0), docs/parity/cursors.md (rows 39/46 + note).
  * crates/app/src/drawing_note.rs (ui-select-caret, #777, size combo pr 6).
  * crates/app/src/os_ui.rs (GLYPH_FS_IMPORT, GLYPH_LABEL_ADD split + GLYPH_LABEL_ADD_PLUS, fn label_add_glyph),
    crates/app/src/start_window.rs (2 call sites -> os::label_add_glyph).
  * crates/app/src/part_studio/menus.rs (mass + measure X/Y/Z glyphs and letters; `parent` row tracking).
  * New icons: ui-select-caret, axis-arrow-x/-y/-z, measure-min-dist, measure-max-dist. Redrawn: add-custom-features.
    Copies of all in /root/icons2-scratch/redraw and r5/.
- Guard (standalone rustc) 2 passed after the last icon. `cargo clippy --workspace --all-targets -D warnings`: nothing from
  icons2's files; it fails on other agents' WIP (drawing_aux.rs:443, feature_dialog.rs:1202, compare_mode.rs:758,
  sketch.rs:2511, smoke.rs:155, variable_config.rs:48/308) and the bin test target stops at "recursion limit reached while
  expanding #[test]" (moves around: ui.rs:1121 earlier, part_studio/behaviors.rs:2991 now - looks crate-wide, for the lead).
- NEXT: (1) when live answers the rail-5 request (REQUESTS.md icons2 line), draw that icon and tell the lead/asb;
  (2) compare the animate dialog (assembly/53) and the in-Part-Studio compact Configurations block (document/730: blue
  configure button at x 196 y 96); (3) Windows cursor art was only checked as renders.

## Round 6 (2026-10-21 ~10:30-, after the lead's routing)
1. Rail 5th button: live answered - it is **Simulation** (free-plan upsell pane). Simulation is excluded by TEAM-RULES, so no
   icon is drawn; ui2 (who got "6th rail button") should know it is out of scope.
2. Animate mate dialog (vs live assembly/53 + dom/dialog-animate.txt), assembly_behaviors.rs: icons at 20 px (were 22),
   reverse = flip-circular-opposite (live svg-icon-flip-rotation-opposite), Playback select caret = 8x4 #333
   (drawing-tool-caret at 12, was the kit caret), Play/Stop flush with the dialog's left edge, 29 px pitch, reverse button
   flush right; the shared plain × of assembly panel_frame_full: close glyph 21 px at opacity .86 (icon carries .58) = live
   11 px #999. os_ui::param_row labels 13.33 -> 12 px (live dom says 12; used by 4 assembly panels; asb FYI).
   For the lead: live param rows fill a 194 px first column (246-440) with the 26 px right column free; ours span the
   full width (os::param_row, shared).
3. Windows cursors in CI (coordinated via the lead; perf owns perf.yml):
   * cursor.rs `pub fn dump(dir, cursors, cx)` writes every image cursor as a real .cur (`<name>-<win|mac>-<1|2>x.cur`,
     32-bpp straight alpha, hot spot) - platform-independent Rust, nothing Windows-only to compile.
   * smoke.rs (perf's): after the cursor loop, `NOSHAPE_CURSOR_DUMP=<dir>` calls it ("SMOKE cursor files: N").
   * .github/perf/cursors.ps1: inline C# (plain user32/gdi32 P/Invoke + its own PNG writer, no System.Drawing): per file
     LoadImage(LR_LOADFROMFILE) and CreateIconIndirect from the same pixels (= cursor_windows.rs make), GetIconInfo hot
     spots, DrawIconEx on white and grey -> cursors/cursors-sheet.png, <name>.png, cursors.txt; errors when a hot spot reads
     back differently or Windows' drawing is > 2 levels off the file. Warns (exit 0) when no files (older releases).
     Compile-tested with pwsh 7.6 here (`dotnet tool install --tool-path /root/icons2-scratch/pwsh PowerShell`), PNG
     writer checked; the Win32 part only runs on Windows.
   * perf.yml smoke job: Windows step sets NOSHAPE_CURSOR_DUMP=$out\cursor-files; new step "Cursors (Windows)" runs the
     script into the smoke artifact. Needs a release built after this lands (or a `ref` run).
4. Sweep (worst first):
   * tools/rank.py --tag sweep (work/sweep.txt): top entries are by design (blurry view-orientation refs, hatch textures,
     rule-6 cube on document icons, neutral PDF). Map fixes: add-custom-features, create-selection, mc-select.
   * DOM sprite coverage: 460 sprites used in live DOM; the in-scope ones without a counterpart were checked against
     LIVE crops (the ui-ref sprite of the same name can differ, e.g. show-panel = our show-preview, already right):
     - sketch Text dialog (part_studio/sketch.rs): "Flip H"/"Flip V" text buttons -> new sketch-flip-horizontal /
       -vertical icons (4.37 / 4.99 vs live sketch/132), 30x30 buttons; B / I now 25 px bold / italic #1651b0 (live).
     - assembly Insert dialog (assembly_view.rs): Sketches filter -> new insert-sketches (2.92 vs live assembly/120,
       was the pencil), "Insert entire Part Studio as rigid" -> new insert-rigid (6.41, was part-studio).
     - Part Studio Tables panel footer "Add custom tables" -> new add-custom-table (1.62 vs live part-studio/639).
     - Not ours/out of scope: performance "Connection" (server), Import from (cloud), c-plane (a document's custom
       feature). Missing feature for the lead: assembly Variable table "Automatically referenced Variable Studio" header
       icon (dom assembly-panel-variable-table).
   * Context menus (tools/menusweep.py: every live/dom/context-menus-* row vs the icon our code gives that label, live
     crop scored; work/menusweep.txt, sheets/menusweep-*.png). Found: 12 menu items used `.icon(Icon::empty().path(
     icon_path(..)))`, which os_menu draws as a 12 px single-colour kit icon (sketch, plane, check, delete, edit, flip in
     part_studio/mod.rs and assembly_*.rs) -> `.icon_name(..)` (16 px, full colour like Onshape's menus).
   * Icons other agents added that had never been checked against Onshape (all redrawn by hand from 1x live grids):
     - Drawing table Cell panel (drawing_cell_panel.rs, drawing-verify's): cell-insert-row-above/-below, -col-left/-right,
       cell-duplicate (0.6-0.75 vs live drawing/1143, were 20-27), cell-remove-rows/-cols (3.4, were 43-49: Onshape has 3
       rows + a #3e3e3e badge with a white x). Colours: grey #9f9f9f, selected #225ab4 on #96d1f2, arrows #3e3e3e.
       Size columns/rows equally, Merge, Unmerge are only captured disabled -> live request filed (REQUESTS.md icons2).
     - Drawing properties > Auxiliary views "View reference format" tiles (drawing_drawprops.rs): aux-format-line /
       -arrow redrawn on a 48 px canvas (part + bracket / arrow, 2.9 / 5.05 vs live asb/u-930, were 13 / 15), tile icon
       40 -> 48 px.
     - Merge dialog rows (merge_dialog.rs, ui2's): merge-both, merge-replace redrawn (bigger 3.5 px ring, 2.5 px strokes,
       24 px canvas; Onshape's ink is 18x20, ours is shown at 18 -> should be 24); new merge-replace-plain (Replace where
       Main had no changes: thin main line, live row 3) and merge-keep-plain (Keep without discard, live row 4).
       For ui2: Onshape picks svg-icon-replace vs replace-discard and no-merge vs selectivemerge-keep-left-discard per
       row (whether the other side changed); strategy_icon() has one icon per strategy. merge-keep (with x) unchanged.
     - Imported-file tab icons: Onshape has per-format element art (STEP, X_T, X_B, JT, PVZ, STL, IGES, ... cards) but its
       CSV tab shows a plain page, so which art a tab uses is unknown -> live request filed before drawing any.
   * Shared menu / select chrome (vs live analysis/01 camera menu, menus/rmb-*, dialogs/fillet-01):
     - os_menu checked items: gpui-component IconName::Check (9x8, #333) -> new hand-drawn menu-check (11x8, #666; 4.0 vs
       live), drawn in colour at 16 px.
     - menu-delete (circled x, every Delete item): circle 13 px pixel-centred like live (r 6.05 px), x 1.6 units: 11.8 -> 4.1.
     - os_select (feature-dialog enum selects): caret ui-caret-down 10 px (5x3.5) -> drawing-tool-caret 12 px = the 8x4
       bootstrap caret, pr 5 -> 4 so it ends 6 px from the field end (live 426-434 in a 330-440 select).
     - Animate dialog Playback row restructured to live's 84 + 110 + 26 px columns (reverse was 4 px outside the dialog);
       playback button px 0 so its caret sits at the right like live.
     - New sketch / Offset plane in menus: now full colour (see the .icon -> .icon_name fix); remaining 15-21 diffs vs live
       at 16 px are rasteriser differences (plane = 3.07 vs its sprite).
   * Lesson: a perl bulk edit (.icon -> .icon_name) also hit 6 Button calls and broke the tree for ~10 min (reverted
     right away). Always cargo check right after any bulk edit.
   * Live answered both newer requests:
     - Cell panel enabled crops (drawing/icons2-cell-*): the 7 earlier redraws confirmed (0.6-3.4); cell-size-cols-equal,
       cell-size-rows-equal, cell-merge, cell-unmerge redrawn (1.57 / 1.06 / 0.55 / 0.55, were 24-28).
     - File tabs (tabbar/icons2-filetab-*): Onshape draws per-format cards WITH A WHITE FILL (#eaeaea tab behind). New
       hand-drawn file-x_t, -x_b, -jt, -stl, -pvz, -3mf (band letters as mask strokes, like file-cad's STEP), file-geometry
       (generic cube card: IGES, SAT, 3DM, CATIA, Inventor, Solid Edge, NX, OBJ), file-sw (SolidWorks: Onshape shows the SW
       shield logo, a third-party mark -> neutral "SW" band, like the PDF decision), file-dwt (folded page + grid),
       file-txt, file-zip; white fill added to file-cad, -dwg, -image, -pdf, file. Scores vs live 1x/2x: 1.5-3.7 except
       file-geometry 12.9 / 11.6, file-zip 7.6 / 6.1, file-pvz 5.6 (could still be refined).
       file_tab_view::file_icon now picks by extension (falls back to the kind); merge_dialog's tab_icon uses it too.
     - Tab-bar icons are white inside in Onshape too -> new tab-part-studio / tab-assembly / tab-drawing (white fill),
       used only by workspace.rs tab_icon (the shared icons stay unfilled because glyph() masks some of them).
       Variable Studio / Feature Studio / Material library tab icons not checked yet.
     - Camera menu checked submenus (view_cube.rs) used gpui-component's IconName::Check: os_menu::submenu_with_icon now
       takes a named icon (only caller: view_cube) -> menu-check.
   * Verified in app (12:40 build): os_select caret = live (Fillet), Animate Playback row within 1 px of live.
   * File cards: Onshape's card radius is ~2 (ours was 1.6) -> all banded/geometry/txt cards use r 2 (STEP 2x 2.71 -> 1.51).
     file-geometry cube re-measured (x 4.5-15.5, y 4.3-15.3): 12.9 -> 7.9 / 5.4. Final 1x/2x vs live: STEP 2.9/1.5, X_T 2.5/1.4,
     X_B 3.3/1.8, JT 1.6/0.8, STL 2.4/1.0, PVZ 5.2/4.7, 3MF 2.3/1.7, geometry 7.9/5.4, TXT 1.1/0.5, ZIP 7.6/6.1, DWT 3.2/1.1,
     DWG/DXF 1.6/0.8, PNG 2.9/3.1.
   * Verified in app (13:03 build): camera-menu check 10x7 #696969 (live 11x8 #666), tab bar icons white inside (live).
     File tabs themselves not seen in the app (the demo documents have none; the icon art is checked against live crops).
   * Tab manager (tab_manager.rs): live list rows show each file's format icon and the filter row has one button per
     format present (live tabbar/icons2-filetab-tabmanager-listview). kind_of(File) now returns file_icon(f), so rows
     and filters follow (tooltips stay "File"; Onshape's per-format tooltip text isn't captured).

## EXACT STATE at end of round 6 (2026-10-21 ~13:50 UTC)
- No app, no cargo running. /tmp/icons2/nsa = 13:03 build (has everything except the tab_manager change).
- cargo check -p noshape clean (13:45, incl. tab_manager); clippy --workspace --all-targets: nothing from icons2's files,
  fails only on variable_config.rs:304/318/330 (collapsible if, another agent's WIP). Guard test 2 passed.
- Files touched this round (besides icons): assembly_behaviors.rs, assembly_view.rs (plain close x), os_ui.rs
  (param_row label 12 px), part_studio/sketch.rs (text dialog B/I/flips), assembly_view.rs (Insert filters),
  part_studio/custom_tables.rs, part_studio/mod.rs + assembly_*.rs (.icon -> .icon_name, 6 menu items), os_menu.rs
  (menu-check; submenu_with_icon takes a named icon), part_studio/view_cube.rs, os_select.rs (caret),
  drawing_drawprops.rs (aux tiles 48 px), file_tab_view.rs (file_icon by extension), merge_dialog.rs (tab_icon),
  workspace.rs (tab-* icons), tab_manager.rs, cursor.rs (dump), smoke.rs (dump hook), .github/perf/cursors.ps1,
  .github/workflows/perf.yml.
- For the lead to route: param rows' 194 px first column (os::param_row), merge-dialog per-row icon variants (ui2),
  assembly Variable table "Automatically referenced Variable Studio" icon (missing feature), Cell panel owner
  (drawing-verify) FYI on the 11 redrawn icons, perf FYI on the CI step; the CI cursor check needs a build that
  contains cursor::dump (run perf.yml with ref=<integrated commit>, jobs=smoke) - then compare
  smoke-windows-latest/cursors/cursors-sheet.png with live/cursors/icons2-win*-1x.png.
- Rail 5th button = Simulation (excluded; ui2 can drop it).

## Round 7 (2026-10-21 ~14:00-)
- Compact Configurations block (part-studio/183, dom/panel-configuration-left-box): ▾ = Onshape's 10 px dropdown-arrow (10x5,
  #666) -> new ui-dropdown-arrow (1.91 vs live), used in config_panel.rs:1930 + assembly_config.rs:1146 (the Configure
  buttons keep the 8x4 caret, live 181). configuration-visibiliity redrawn from colour-coverage readings 12.27 -> 9.4
  (cells exact; the eye's AA at 1x is the rest). For the lead: that ▾ opens our add-input menu; behaviours.md 371 says
  Onshape's is "View visibility conditions... | Reset to default" (behaviour, config owner).
- Dark theme: live has one (beta; dark/dk-9xx, dom/dark-*). noshape has no dark theme yet (theme.rs is one static palette).
  Onshape's dark icons are NOT a plain recolour (dark Extrude has light-filled faces) -> live request filed for the dark
  sprite sources / computed fills (REQUESTS.md icons2). tools/darkvariant.py has the token-based recolour
  (#333->#fff, #fff->#333, #999->#d7d7d7, #666->#999, #d7d7d7/#eee->#555, colours kept) for when ui adds the theme.
- Sweep to < 3: tools/rank.py --tag sweep2: 293 icons >= 3; 229 after excluding by-design (view-orientation refs blurry,
  hatch textures, rule-6 document marks, neutral PDF, calendar). Snapshot before-r6/. Redraw agents (brief = BRIEF.md +
  round-6 rules in their prompt; they write only /root/icons2-scratch/redraw, I review + copy):
  | group | icons | account | agent id |
  |---|---|---|---|
  | r6-g1 | 58 | Yekta | 034b3fc4-86a9-4c77-9d22-4605b29f0add |
  | r6-g2 | 57 | Stablecog | da7b08a0-9854-4766-96ad-f78a4790c09a |
  | r6-g3 | 57 | Unbind | 50ea5bba-db09-4ae5-b4e3-fba42a7357e2 |
  | r6-g4 | 57 | Motile | d5480772-f2f3-4039-ae3b-b06bcd3050d1 |
  Copy-in checklist: diff before-r6 vs repo first (others may have changed icons), guard, cargo check, in-app look.

## EXACT STATE at handoff (round 7, 10-21, moved for quota)
- **Last edit, build not yet verified**: `crates/app/src/part_studio/mod.rs` ~3188. The feature-row icon is now wrapped in `div().relative().flex_none()` so the under-defined badge is not dimmed. The badge is the new `assets/icons/list-underdefined-overlay.svg` (12 px: #1651b0 circle, white 6×2.4 bar) at left 9.5 / top 8. It replaces the old 8×4 accent bar. Live: dom/ctx-partstudio-left-after-edit-in-context.txt:66 (badge [53,286,12,12], sketch svg [45,278,17,17]); pixels in part-studio/495. A `cargo build -p noshape` was started; **first rebuild and fix if needed**, then check in the app that the badge pixels match 495 (x 53..64, y 286..297, relative to the svg +8,+8).
- Route to ui (layout, not mine): feature-list rows put the icon 1 px too far right and the text 2 px too far right (live: icon ink at x 46, text at 67; ours 47 / 69). Live hidden-row icon opacity is 0.5; `ui::tree_icon` uses 0.4.
- Feature-row icon art matches live: Extrude 4.1, Fillet 3.0 at the best offset (in-app vs 495).
- Card refinement done this round (copied to the repo and redraw/): ZIP 3.48/2.04, PVZ 3.85/2.62, cube `file-geometry` 7.42/5.97 (1x/2x, `tools/ftscore.py`). The cube still needs work.
- `work/override-icons2-r6.txt`: 17 live-crop refs for rank.py (file cards removed; score those with ftscore.py).
- 4 redraw agents (r6-g1..g4, ids in the agent table above) were still running. When they finish: diff `before-r6/` against the repo, review their iter panels, copy in, run guard plus a build, check in the app.
- Dark theme: REQUESTS.md line 437 is still pending. noshape has no dark theme yet (ui's area); `tools/darkvariant.py` holds the token map, which is not enough on its own.
- Perf will send the Windows cursor sheet: compare it with live/cursors/icons2-win*-1x.png.

## From perf (2026-10-21 14:30): Windows cursor check ran in CI
- perf.yml smoke, run 39608471542 (windows-latest; perf-ci = v0.4.4 + smoke + your cursor.rs and cursors.ps1):
  52 cursor files, all "ok" (LoadImage and CreateIconIndirect hot spots equal the file's, max diff 1 level).
- Sheet to compare with live: /root/perf-scratch/ci/cursors/smoke-windows-latest/cursors/cursors-sheet.png (row N =
  line N of cursors.txt; columns LoadImage on white, on grey, CreateIconIndirect on white, on grey); per-cursor PNGs
  and cursors.txt in the same folder.
- Review as perf.yml owner: fine. Assumes one 32-bpp BMP entry per .cur; ZLibStream needs pwsh on .NET 6+ (ok on
  windows-latest).

## Round 7b (2026-10-21 ~14:00-, new icons2 on Yekta, after the move)
- Lead stopped ALL four redraw helpers (r6-g1..g4); icons2 owns all four groups (groups/r6-g*.txt). No r6-g*-status.md was
  written; progress came from work/reports/r6-g4.md and r6-g1's thread (read_thread 034b3fc4...).
- Copy-in #1 (14:30): 46 icons from redraw/ (helpers' finished work + my g2/g3 redraws), all better than before-r6, repo copies
  unchanged since the snapshot, roots (viewBox/size) unchanged, white only inside masks, guard 0/46, standalone guard test 2 passed.
  Copies in /root/icons2-scratch/r7/. Excluded: mc-select, fs-monitor, compare (redraw/ held stale 20-unit files; repo is newer).
  drawing-no-views: g4 removed the white sheet fill (no-white rule) but the Create Drawing tile turns #9ccef0 when selected and
  Onshape's sheet stays white -> white interior restored (score 0.17).
- Gotcha (big one): resvg/tiny-skia draws strokes <= 1 device px as hairlines (alpha-scaled). Onshape's sprite strokes get the same
  treatment, so match Onshape's stroke WIDTHS/centres read from the 40 px grid; where Onshape is crisp/dark at 20 px use 1.01
  (three-point-rectangle 8.73 -> 2.18 just from .75 -> 1.01). Default stroke-width (1) is also a hairline at 20 px.
- New scratch tools: tools/px.py NAME [size] [svg] (luminance grids ref|ours, '<' marks rows that differ), tools/big.py NAME
  (large render with unit grid), tools/at.py NAME SIZE (compare at the size the app shows, e.g. 66 px tiles);
  refs/r6-g1/nat.py NAME (r6-g1's: compare at a PNG ref's native size; use for 24 px weld / 18 px shortcut crops).
- Per-icon log: work/reports/icons2-r7.md.
- For the lead (ui, layout, not icons): feature-list rows put the icon 1 px and the text 2 px too far right (live icon ink x 46,
  text x 67; ours 47 / 69); hidden-row icon opacity is 0.5 in Onshape, ui::tree_icon uses 0.4.
- cargo check -p noshape (14:25): fails only in other agents' WIP (assembly_view.rs:577 private `matched`, assembly_panels.rs:2117
  borrow); part_studio/mod.rs underdefined-badge wrapper raised no errors. Re-check when the tree is green, then the in-app badge look.
- Copy-in #2 (14:45): 33 more icons (my redraws: boundary-surface ... wire-body; weld native fixes; limit-*; mask-stroke fixes).
  All copies kept in /root/icons2-scratch/r7/. Guard test 2 passed after the copy.
- Mask-stroke bug (found in move-face): a <mask> under an element/root with stroke="#333333" inherits that stroke, so the
  mask's white rect and black knockout shapes get a grey 1-unit outline -> grey frames around knockouts and dimmed icon
  borders. Fix = stroke="none" on the <mask>. Applied where it scored better (list in work/reports/icons2-r7.md); eye and
  constrained-surface were tuned around it and score worse with the fix, left as they are.
- Weld glyphs: compare at native 24 px with refs/r6-g1/nat.py (iter's 20/40 resampling of a 4-level dithered 24 px crop has a
  floor ~5-8). 30 weld glyphs are at native diff 0.9-3.1/255 = matching. note-align-*/note-valign-*/limit-* PNG refs are 1x
  crops: tools/natall.py scores them natively (and vs -2x.png if present); most are 0-2/255.
- Stroke 1 -> 1.01 adopted for 27 icons where it scored >= .3 better (list in the report).

## From perf (2026-10-21 15:00): Windows "Cursors" smoke step results
perf-ci (includes 4535176) run 39608471542 (https://github.com/yekta/noshape/actions/runs/39608471542) is green.
The artifact is downloaded at /root/perf-scratch/dl/cursors-39608471542/cursors/: `cursors-sheet.png` (columns: LoadImage
on white, on grey; CreateIconIndirect on white, on grey), `cursors.txt`, one `<name>-{mac,win}-{1x,2x}.png` per cursor;
smoke.log/stdout.log one level up. **52/52 ok:** every hot spot reads back the same as the file through both LoadImage and
CreateIconIndirect; max drawing diff 0–1 level (confirmation, count-1..6/5+, crosshair, crosshair-box, move,
remove-selection, snap-mate, zoom-in).

## Round 8 (2026-10-21 ~15:00-, icons2 on Stablecog; full per-icon log: /root/icons2-scratch/work/reports/icons2-r8.md)
- Copy-in #3 (before the freeze): 44 redraws that sat in redraw/ but had never been copied (r7 log claimed otherwise).
- Copy-in #4 (after "freeze lifted", main 5d28447): 49 files via tools/copyin-r8.sh --apply (guard, repo == before-r8 snapshot,
  backups in scratch r8/*.before.svg). Includes NEW auto-variable-reference.svg (lead/ui2: assembly Variable table header
  "Automatically referenced Variable Studio", live dom assembly-panel-variable-table:9, 20 px; variable-table art + script x +
  green check with a mask-knockout halo, no white). Standalone guard test 2 passed.
  Biggest gains: sync-view 6.29->1.62 (redesign), fit-spline 6.08->2.53 (3 nodes), hem 5.82->2.55, flange 3.53->0.98, treelist
  3.71->1.06, weld-symbol 5.89->2.50, hide/eye-off ->2.54, eye/show ->1.90 (eye had the mask-stroke bug), variable family
  (variable, -studio, -table, create-, insert-) ->2.9-3.6 with Onshape's script x, move-boundary 7.30->3.24,
  section-view-3d/menu-section-view = section-plane art 2.61, add-appearance 3.65, add-mate-connector 2.21, sheet-metal 2.54,
  note-order-up/down 1.38/1.88, composite-curve 2.21, list-free 2.71, fs-monitor 2.23, cube card file-geometry 1x/2x
  7.42/5.97 -> 6.03/3.09 (ftscore; 1x limited: Onshape's 2:1 diagonals are crisp stair steps at 1x).
  stack-furthest has white fills (washer face, nut hole): only drawn in colour via assets::icon (assembly_standard stacks).
- cursor.rs: Windows count label "4" shifted 1 px right instead of 3 (live: Onshape's Windows "4" ink starts 2 CSS px left of
  the other digits; mac "4" matches). Windows CI sheet (run 39608471542) vs live icons2-win*: every cursor aligned at offset 0;
  mean grey diff 2.6-3.9 (counts, remove-selection), 6-6.5 (confirmation, snap-mate: same ink extents, live AA/shadow softer).
  Tool: scratch tools/wincmp.py.
- TOOL FIX (scratch rlib.py): sprites with non-square viewBox (14 refs: add-appearance, compare, sync-view, chevrons, carets,
  illustrations) were rendered stretched and top-aligned; now centred via a square viewBox like the browser. compare is fine
  (1.30); add-appearance/sync-view were refit.
- New scratch tools: sd.py NAME [size] [svg] (signed diff, best guide at 20 px; '+' = Onshape has ink we lack), ov.py
  (continuous colour overlay), natsd.py (png refs at native size), livecmp.py (svg vs a live screenshot at 1x), scan.py
  (contact sheets; SCAN_S/SCAN_C env), q.sh / try.sh (quick per-icon loop), copyin-r8.sh.
- In-app check without touching the tree: bind-mount a merged icon dir over assets/icons in a private mount namespace
  (/tmp/icons2/cap.sh TAG BIN DIR; kill only your own binary copy). context/r8-1..4: toolbars + instance list render right.
- State of the < 3 sweep: ~95 icons in the r6 set are still >= 3. The rest are at a local optimum: the residual is
  anti-aliasing / hairline differences, and sub-pixel nudges made every one I tried worse (hole, analysis-tools,
  isoparametric, surface-body, stack-closest, config-show, mate-connector, composite-part beyond .8). Gains came only from
  structural mismatches (wrong node shapes, missing elements, mask bugs, 1 px hairlines). PNG-ref icons are judged natively
  (natsd.py): 16 were >= 3, mostly dithered live crops with a floor.
- Next: keep scanning for structural mismatches (scan.py sheets), then the compact Configurations block in the app.

## Round 9 (2026-10-21 ~20:30-, icons2 on Unbind; predecessor thread b7f0af4f)
- Verified at takeover: copy-in #4 (49 files) and batch b2 (13 files, 17:13: drawing-styles, appearance, intersection-curve,
  hole-table, realign-mate-dialog, jog, drawing-props, auxiliary/broken/section/projected-view-linked (hollow bracket
  arrowhead), offset-surface, driven-dimension; copies in scratch r8-batches/b2) are in the tree; every changed icon file
  matches a scratch copy (redraw/, r8/, r8-batches/). cursor.rs Windows "4" fix ((1,-5) instead of (3,-5)) is in.
- **Hatch swatches clobbered and restored:** at 17:54 someone ran drawing-behaviour's ignored generator
  (`cargo test -p noshape-model hatch_icons -- --ignored`, model/drawing/hatch_pat.rs:316), which rewrites all 54
  `hatch-<g>-<i>.svg` from the .pat line families. hatch_pat.rs itself was unchanged, so this was a stray run. The
  generated art scored mean 32.8 vs icons2's hand redraws 8.6 (none better) -> restored the 54 files from HEAD.
  For the lead: ask drawing-behaviour to point that generator at a scratch dir (or drop it) so it can't overwrite the
  redrawn swatches again.
- auto-variable-reference refined: table row-gap cells are opaque white and the top cells opaque #999 (Onshape paints
  them; visible on the Variable Studio header's #eaeaea), x stem widened at its lower right, balls/arm/check nudged.
  Sprite score 4.32 -> 3.52; vs the live 1x crop (refs/icons2/auto-variable-reference-1x.png, on #eaeaea) 10.1 -> 7.9
  (Onshape's own sprite: 4.8; the live capture is quantised to ~6 grey levels). White is fine: only drawn via
  assets::icon (variable_studio.rs:853). New tools: tools/live1x.py NAME SVG [LIVE] [REF] (diff vs a 20x20 live crop on
  its own background), tools/try2.sh (try.sh + live diff).
- Gotcha: live crops of header icons sit on grey (#eaeaea), so transparent "white" faces in our art read darker than
  Onshape's painted white there. Check the background before trusting a live diff.

## Round 9b (2026-10-21 ~19:45-, icons2 on Stablecog; predecessor thread 084c1884)
- Copy-ins (backups scratch r8/<name>.before.svg; copies r8-batches/b3..b5; guard test 2 passed after each):
  * b3: body-draft 4.69->1.31, hole 5.47->3.66, end-cap 4.95->2.55, mutual-trim 6.99->3.49, hole-table 4.22->2.86
    (hole-table: arrow halo only along the sides (apex touches the 7.5 line), tapered filled tail, mitred box corner,
    1-unit lower-left polyline).
  * b4: transform 4.10->2.61 (bottom curve now runs into the arrowhead like the top one, box path closed + 1.4 corner,
    hexagon 9.25-17.75 / stroke .85), reset-default 4.58->1.28 (arrow base x 6, bigger x centred 10,11),
    sheet-metal-end 4.81->2.79 (redrawn: 45deg mitred 2.5 check, 3.2 wide grey diagonal band, bend wedge),
    sheet-metal-loft 4.50->2.54 (bottom = two straight segments with a rounded middle, outline 1.08), part-mesh 4.40->2.63.
  * b5: split 4.39->2.94, frame-trim 4.18->1.85 (badge centre 5,15 r5 halo r6; x arms clipped flat to a 2.5-7.5 square),
    cut-list 2.73->1.82 (chamfers H14.8 L18.5 5.2: on pixel centres), modify-fillet 4.09->3.92.
- cursor.rs: Windows count "4" is now a hand-drawn open-top 4 (Onshape's Windows glyph: stem starts below the diagonal
  with a slanted cut), Windows only; mac keeps the font glyph. Verified through the app's own dump path
  (`DISPLAY=:115 NOSHAPE_SMOKE=1 NOSHAPE_CURSOR_DUMP=dir ns --demo-drawing`; needs --demo-drawing, and
  DBUS_SESSION_BUS_ADDRESS=unix:path=/nonexistent avoids a 120 s portal stall): 52 files. Before the glyph change, vs
  live icons2-win*: counts 3.5-3.9, the 4 4.17 (label region 3.54 vs 2.4 for other digits); prototype of the new 4
  (tools/four.py, same SVG as cursor.rs): label 1.43 / whole 2.91. Re-dumped with a 20:35 build that contains it: count-4 vs live 2.91 / 2.85 (1x/2x, was 4.17 / 4.36), now the best
  of the counts, offset (0,0) (tools/curcmp.py /tmp/icons2/curdump).
  cargo check -p noshape clean after the edit (one warning in model/sheetmetal/flat_dxf.rs:132, not mine).
- **Finding: the app rasterises icons at 2x and box-downsamples** (GPUI's svg scale factor): an in-app toolbar crop
  matches ds2(resvg 40 px) at 1.0-1.2 mean diff, a direct 20 px resvg render at 6-8. Chrome (live) sits between the two
  (Onshape sprite vs live: 1x 3.55 mean, ds2 3.59), so no GPUI change is needed, but **d40 predicts the in-app look best**;
  1.01-stroke hairline tricks matter less in the app than at resvg 1x.
- New scratch tools: applive.py NAME SVG LIVE (ours through the app path vs a 20x20 live crop; floor = Onshape's sprite,
  same path), tbslots.py APPSHOT LIVESHOT (every toolbar slot app vs live at the same position; app and live toolbar
  positions coincide), inapp.py, appshot.sh start|shot|stop (one app, killed only by its saved pid), curcmp.py, four.py.
  Live toolbar crops: refs/icons2/<name>-tb-1x.png (from live/part-studio/00-new-doc-initial.png, y 48).
- Hatch swatches: unchanged since round 9's restore (asb's generator output left alone).
- b6: import 4.17->2.92 (DXF page: smaller arrowhead 5..13 apex 9,10, tapered tail, dog-ear = rounded outline, wider X),
  dwt-template 4.06->1.90 (same page/arrow as import + W/T letters), dwg-insert 3.08->2.36 (same art as import, same ref),
  curvature 3.41->1.91 (pointier arch). b7: create-partstudio-incontext 2.82->1.80 (smaller bottom chamfers).
- Toolbar sweep app vs live (tbslots.py, same positions): Part Studio, sketch and assembly toolbars. After the copy-ins (in-app
  crops vs live): split 6.44->4.26, frame-trim 8.67->6.18, cut-list 3.85->1.85, curvature 7.16->5.24, import 8.13->7.92,
  modify-fillet 9.04->8.63, create-partstudio-incontext 4.66->2.63 (applive). Everything else in the three toolbars is
  within ~1.5 of its floor. Left as is (tried, no gain): hole (ring size), gear-relation / rack-pinion (8-tooth params from
  tools/gear.py, hand-picked sets all worse), parallel (live prefers the 1.15 blue line: pixel phase through the 2x path),
  snap-mode (live crop is speckled, floor 7.4), boundary-surface, drawing-suggested-views (66 px tile, see r7).
- Gotcha: a sprite-score gain can be a live loss for thin lines (parallel): check applive.py too when a live crop exists.

## EXACT STATE (2026-10-21 ~21:05, icons2 on Stablecog)
- No app or cargo of mine running. /tmp/icons2/ns-icons2 = 20:35 shared-target build (has the cursor change).
- Tree: my files compile (cargo check -p noshape clean at 20:20). At 21:00 the shared tree failed in another agent's WIP
  (model/custom/host/sheetmetal.rs:713 `fi` not found), not mine. Guard test (standalone rustc) 2 passed after every copy-in.
- Files touched this round: crates/app/src/cursor.rs (Windows "4"), icons of batches b3-b7 (list above; backups in
  scratch r8/*.before.svg, copies r8-batches/b3..b7). r8w/ is empty.
- Next: drawing toolbar icons all match live by eye (checked in toolbar order vs live/drawing/08); remaining sprite >= 3
  icons are at a local optimum or by design (see lists above). Further gains: other live screens with stable positions
  (feature list, context menus via tools/menusweep.py) using applive.py; d40 is the best predictor of the in-app look.
## Round 10 (lead: score against 40 px; feature list, menus, dialogs vs live)
- Feature list: icons render through the same 2x path at 34 px -> 17, at 0.8 opacity (ui::tree_icon); that model
  matches the app's crop at 1.47. tools/d17.py NAME SVG = that metric vs the sprite. Only extrude degraded at 17 px:
  its .3 hairlines -> .5 and a .5 chamfer edge at the front box: d17 4.72->3.69, iter 2.70->2.31, live toolbar
  3.18->2.56, live feature row 6.56->5.67 (copied in, batch b8).
- Context menus (in-app vs live menus/rmb-ps-featurelist-extrude / -sketch): icons present match. Fixed:
  * part_studio/mod.rs feature-list menu: Show/Hide gets eye / eye-off, "Create Drawing of Sketch N…" gets
    menu-create-drawing, "Edit sketch appearance…" shows a colour swatch (the sketch's appearance colour, #999 default;
    live: 17x17 #999, radius 2) instead of add-appearance.
  * part_studio/menus.rs viewport sketch menu: same swatch on "Edit sketch appearance…".
  * os_menu.rs: new MenuIcon::Swatch(Hsla) + OsMenuItem::swatch(); appearance.rs: PartStudioView::sketch_swatch(id, cx).
  Not yet verified in the app (needs a build after the freeze).
- tools/menusweep.py now renders through the app path and parses icon_item(...) / MenuEntry::item(..).with_icon(..)
  (conditional and format! labels). Its "ours has icon, live none" rows (Zoom to fit, View normal to, Export…, Edit…,
  Isometric) are unverified: the parser may be pairing labels from other menus; check in the app before changing code.
- Next: viewport/parts-list/assembly menus in the app vs live (Copy, Delete, Hide rows), then dialogs.
- Freeze (21:25-) passed with: cargo check -p noshape clean, guard test 2 passed, clippy -p noshape --all-targets: nothing
  from icons2's files. All of the above (menus + cursor "4") is in main 5bd50b2. perf's 52/52 Windows cursor result is
  run 39608471542 (15:00), before the hand-drawn "4"; a perf.yml smoke run on 5bd50b2 would confirm it on real Windows.

## Round 10b (2026-10-21 after "freeze lifted", main 5bd50b2)
- Verified in app (23:27 build): feature-list Sketch menu now = live (Hide/Show eye icons, Create Drawing icon, #999 swatch
  17x17). Swatch corner radius 2 -> 3 (live corner pixel is white; os_menu.rs).
- Menus in app vs live, side by side (context/r10b-rmb-*.png vs live/menus/rmb-*): parts-list part, face, PS empty,
  assembly instance row, revolute row, assembly empty -> icons match, except assembly_behaviors.rs had icons where live
  has none: Export… (graphics instance menu), Zoom to fit + View normal to (instance menu), Zoom to fit + Isometric
  (empty menu) -> plain items now (live rmb-asm-two-instances-selected, rmb-asm-empty).
  For ui/lead (layout, not icons): live menus wrap long labels ("Add mate connector to instance origin…" on 2 lines,
  menu max width); our os_menu widens instead.
- Dialogs (Extrude, Fillet, Hole vs live/dialogs): flip, ±, caret, chevron, help, hole diameter/depth/tip-angle icons
  match (2.5-7). Fixed in feature_dialog.rs: ✓ and ✗ drawn at 24 px (live dom adv4-a-dialog: 24 px sprites centred in
  28x24; ink was 14x10 / 8x8, live 16x12 / 12x12); the selection-field chip × (kit IconName::Close 12 px, 6x6 #909aa0)
  -> ui-clear glyph 9 px #666 (live 7x7 #666), red on hover via group_hover; Icon/IconName import dropped.
- Verified (00:04 build, 10-22): cargo check clean; clippy -p noshape --all-targets: nothing from icons2's files (only
  assembly_view.rs:37 unused `Group`, another agent's). In app vs live: ✓ 9.86 -> 4.53, ✗ 16.29 -> 3.28, chip × same
  size/tone (10.9 -> 10.2, pixel phase), assembly empty menu without icons (= live), swatch corner = live.
- Next: other dialogs with their own icons (plane types, sketch text, pattern dialogs), the Hole dialog's mc-select
  button sits ~0.5 px lower than live (layout).

## Round 11 (2026-10-22 ~00:30) — EXACT STATE at the move to Motile
- Checked in app vs live: Plane (types), Linear pattern, Circular pattern, sketch Text dialogs (context/r11-*.png vs
  live/dialogs/plane-types-01, linear-pattern-01, circular-pattern-01, live/sketch/132).
  * Matching: disabled ✓ (2.3), flips, circular mate-connector button, text flip H/V (5.3 / 7.9), I (5.8).
  * Fixed (feature_dialog.rs, both flip button helpers ~1618 and ~1741): flip icons drawn at 19 px instead of 20 (live dom
    dialog-*.txt: every flip svg is 20 x 19, i.e. the sprite at 19 px). Simulated vs live: circular ↻ 13.6 -> 12.4
    (floor 11.3), straight flips 12.8 -> 12.4 (floor ~12.6).
  * NOT YET COMPILED: last cargo check (00:2x) failed only in other agents' files (assembly_view.rs:486 and
    drawing_view.rs:427, moved `doc`). My change is the size literal 20. -> 19. plus a comment. First thing on resume:
    cargo check -p noshape, then clippy, then build and look at an Extrude + Circular pattern dialog (flip ink should be
    ~16x16 at x 444-460 like live).
- For the lead / other owners (not icons):
  * Sketch Text dialog layout (sketch owner): live replaces the sketch dialog in the left panel with Onshape's standard
    header (green ✓ 28 px, red ✗) and font select "Open Sans"; ours is a floating popup with a small blue check, small ×,
    "Liberation Sans" and an extra "Convert to expression".
  * B / I glyph shape differences come from the UI font (DejaVu Sans on Linux here vs Onshape's web font; ui).
  * Dialog body is white at 95% alpha (theme dialog_bg 0xfffffff2) = 252 over our viewport, live 255 (ui to confirm).
  * Pattern dropdown menu (toolbar ▾) shows a scrollbar for 3 items; compare with live/menus/ps-dropdown-* (ui).
- Next (icons2): verify the 19 px flips in the app; then other dialogs with own icons (mirror, sweep, loft, shell,
  hole variants, mate dialogs in the assembly), using the same method: tools/appshot.sh start|shot|stop (one app, killed
  by saved pid), side-by-side crops vs live/dialogs, per-icon template match ±12 px, DOM sizes from live/dom/dialog-*.txt.
- No app running; nothing else half-done. Perf smoke run 39697628442 on 5bd50b2 (Windows cursor "4") pending, lead forwards.

## Round 12 (2026-10-22, on Motile) — dialog icons, batch 1
- 19 px flips (feature_dialog.rs) compile (cargo check 00:5x clean apart from quantity_field.rs warnings, not mine).
- Hole dialog: the counterbore / countersink / tap row icons were never scored (not in work/map.json). Mapped in
  scratch work/override-icons2-r11.txt and redrawn by hand (r11/, backups r11/before/):
  hole-tapped-depth 24.71 -> 0.51, hole-tap-clearance 23.20 -> 0.51, hole-csink-angle 19.86 -> 1.21,
  hole-cbore-diameter 17.92 -> 0.74, hole-csink-diameter 12.66 -> 1.22, hole-cbore-depth 10.48 -> 0.74.
- Mate limits (assembly_mate_dialog.rs limit_field): live shows ONE 26 px icon per limit row with the axis letter in
  it (#999) and the glyph in the axis colour (X red #BD3039, Y green #009400, Z blue), field right after it
  (dom/dialog-mate-revolute-limits.txt). Ours drew a text letter + an 18 px blue glyph for every axis. New icons
  mate-limit-{z-rot,z-lin,x-lin,y-lin}-{min,max} + mate-limit-cone (scratch r11lim/gen.py, scores 0.15-2.0,
  cone 2.90); old limit-angle-*/limit-dist-* removed (only user; backups r11lim/removed/). Guard test 2 passed.
- Dialog icon coverage: every svg-icon in live/dom/dialog-*.txt is mapped except import-*, no-views,
  parametric-part-studio, version-graph-button, assign-variable-button, collapsed-error, sketch-flip-* (next).
- Verified in app (01:2x build, context/r12-*.png): Hole Simple/Counterbore/Countersink rows show the new icons in
  live's order; Revolute mate Limits rows = live (Z grey + blue glyph, 26 px). Then limit row sized as live
  (194 x 29, field ends 26 px before the dialog edge) — not yet rebuilt.
- insert-rigid ("Insert entire Part Studio as rigid", assembly Insert dialog) redrawn from sprite
  parametric-part-studio (dom/dialog-assembly-insert.txt names it): 4.48 -> 2.14. insert-sketches = sprite:sketch (0.72).
- variable_insert.rs (Insert Variable Studio dialog child rows): live "(x) width" -> added the 20 px `variable`
  icon (= sprite assign-variable-button, d 2.2) 14 px before the name, dropped the "#" prefix. Not yet seen in app.
  For the owner: live child rows are 40 px tall (pl 40 + a 70 px thumbnail column), name 13.33 px / 300; the
  studio row's icon is ~30 px (variable-table-button). Ours: 28 px rows, 20 px icon.
- Scratch override file is now work/override-icons2-z11.txt (sorted last so it wins over r6's png refs).
- Mate dialog icons scored: realign 3.88 (local optimum, stroke sweep), qlv-item 1.70, play/stop/reorder 0.
- version-graph.svg was redrawn by theme-layout (DER2) as the dialog "version-graph-button": 8.17 vs that sprite.
- Import modal tiles (import_dialog.rs): live shows ONE 150 px picture per tile (svg-icon-import-single-doc /
  import-flatten); ours composed import-doc 58 + import-arrow 18 + a div box with two 36 px tab icons (arrow too
  small/low, box too small). New own drawings import-tile-single / import-tile-combine (150 viewBox; page, arrow,
  white box, our part-studio/assembly paths at 1.75x, 50% opacity; ~1.4% mean diff vs sprite); old import-doc and
  import-arrow removed (only user). Verified in app vs live document/77 (context/r12-imp.png): tiles identical.
  For ui: our Import modal sits/sizes ~10 px off live (modal-body 588 wide at x 666).
- version-graph.svg: 8.17 -> 1.64 vs version-graph-button (ring r2.5/1.1, stems 2, corner radius 3, dots r2).
- Mate limit rows re-verified after the 194 x 29 change (context/r12-revlim2.png): field right edge = live.
  For the owner: the field text has ~10 px more right inset than live; live also shows "Simulation connection"
  (simulation, excluded).
- State at ~02:00: cargo check -p noshape clean (2 warnings in quantity_field.rs, not mine); clippy --bin noshape:
  nothing from my files; clippy --all-targets blocked by crates/ops/src/live.rs:118 (another agent's WIP).
  Guard test 2 passed. No app running. /tmp/icons2/ns-icons2 = 01:5x build.
- Next: Insert Variable Studio child rows in the app (needs a Variable Studio: + > Create Variable Studio), the
  remaining mate dialog types (slider/cylindrical/planar/ball limits: X red, Y green, cone), sheet-metal dialog
  collapsed-error chevron, drawing Create dialog (no-views / four-views-third-angle 6.64).
- Insert Variable Studio child rows verified in app (context/r12-ivs2.png): "(x) width" = live content. Layout left
  for the owner (see above); also, opened from a Variable Studio our Insert dialog is a centred modal with "Close",
  live uses the same 300 px feature-style dialog as in the Part Studio (behaviours.md ~1571).
- Dialog chevron (expander-right 14 px) vs sprite collapsed (20 px svg, pad 4): ink 7.6x12.4 vs 7.0x12; ok.

## Round 13 (2026-10-22, after lead's batch-1 routing) — limits, sheet metal, dialog sweep
- Mate limits verified in app for Planar (X red / Y green / Z rot blue), Ball (cone), Slider (= live
  qa5/asm-036 exactly): context/r13-planar, r13-ball, r13-slider. Bundle (woolsthorpe) confirms the param -> icon map
  (limitXMin -> XLinearLimitMin, ..., limitAxialZMin -> ZRotateLimitMin, limitEulerConeAngleMax -> ConeAngleLimit).
- Slider flip arrow differs from qa5 capture only by alignment state (asb's flip_icon_name logic), not an icon issue.
- Sheet metal model: red expanded chevron on "Selections" = live (context/r13-smm.png).
- New tool tools/dlgcmp.sh SLUG "Tool": opens a tool by search in the running app, side by side with
  live/dialogs/SLUG-01-open.png -> /tmp/icons2/v/d-SLUG.png. Swept: mirror, sweep, loft, shell, revolve, thicken,
  rib, draft, chamfer, boolean, split, helix, plane, delete-face, move-face, offset-surface, mate-connector,
  transform, delete-part, variable: all icons match. Non-icon: Helix has a "Handedness" label live doesn't show.
- mc-select vs live (dialogs/revolve, refs/r13/mc-revolve-live.png): 11.44, sprite floor 8.97; a stroke/arc sweep
  only got worse -> raster-path difference, left.
- Assign material panel (part_studio/material_panel.rs): + was `plus` (15 px, 3 px stroke, #656565) -> `add-button`
  (12 px, 2 px, #333 = live); ✓/✗ 20/18 -> 24 like feature dialogs (live 28x24 sprites); library select caret ->
  ui-select-caret 8 px #777 (native select arrow, 8x6 = live); "- None -" caret -> drawing-tool-caret 12 px (8x4 =
  live, same as os_select). Building to verify.
- Kit icons in panels -> Onshape sprites (verified in app where noted):
  * Header ✓/✗ (kit Check 18 / Close 16) -> dialog-ok / dialog-cancel 24 px in section.rs (verified = live
    part-studio/71), analysis_tools.rs, behaviors.rs (dyn), assembly_view.rs, assembly_standard.rs,
    assembly_panels.rs (x2), assembly_mate_dialog.rs. Imports trimmed (assembly_view keeps Icon/IconName: its
    child modules assembly_panels/behaviors use them via super::*).
  * Selection-list × (kit Close 11/12) -> ui-clear 9 px #666 in create_selection.rs (red on group hover),
    draft_tool.rs, section.rs, flatten_tool.rs. Animate dialog chip × -> ui-clear 10 px #666 in a 20x24 span.
  * Assign material panel: verified in app = live (context/r13-am2.png).
- (DONE 10-23, verified in app = live part-studio/71: × always shown, plane 20 px = 17x18 ink) Was open: Section view chip × is hover-only in ours (section.rs `.opacity(0.)` + group_hover), live always
  shows it; its add-plane icon is `plane` 16 px, live ~18x18 ink (part-studio/71). Remaining kit icons: Close in
  release_dialog (x2), isolate_ui, reflection, view_tools, behaviors dep-close (x2), assembly_behaviors (shortcuts,
  group x, measure x), assembly_panels mv-x, analysis hq-off; Check ticks in checkboxes (ours ≈ live glyph);
  ChevronDown (flatten, thickness, reflection x2), Loader (thickness), TriangleAlert (start_window), Plus (display
  states). Each needs a live reference before changing.
- FREEZE 04:10: tree green at my last edit (cargo check -p noshape: Finished, 0 warnings), guard test 2 passed,
  tools/guard.py 0/732. No tree edits until "freeze lifted".

## From ui2 (10-22): Onshape Labs flask
- `onshape-labs-blue.svg` (Labs on: Preferences heading, Feature Studio Co-complete switch) and `onshape-labs.svg`
  (grey, Labs off) are ui2's own drawings of live's `svg-icon-onshape-labs(-blue)` (a flask with a cube inside,
  20 px). Redraw them in your style if you like; keep the names.

## Round 14 (2026-10-22, after freeze lifted, main aa75fdf) — kit icons -> Onshape
- Section view (section.rs): chip × always shown (live part-studio/71), side plane icon 16 -> 20 px. Verified r14-sec.
- Bootstrap/ns-dialog close (11 px ×, #333 at .5 = `glyph("close", 21., #333)` at .86, same as Animate's
  panel_frame plain close): Dependencies (behaviors.rs dep-close, pdep-close; verified r14-dep), Isolate (isolate_ui;
  11x11 #999 = live), Reflection env picker (reflection.rs envp-close), keyboard shortcut sheet (view_tools sc-close,
  assembly asm-sc-close), assembly Measure (asm-measure-x). Display states now uses the plain close (live nv-904).
- Select carets -> drawing-tool-caret 12 px (8x4 #333 = live CSS caret): reflection env picker (Library/Type/Image;
  verified r14-envp), flatten export format.
- Selection-list × -> ui-clear 9 #666: assembly Group member rows.
- Display states "+" (was a blue primary kit button): bordered 28 px box, add-button 20, #aaa (.42) until a name is
  typed (live nv-904); verified.
- Dialog header ✗ 18 -> 24 and ✓ 20 / kit Check 18 -> dialog-ok 24 app-wide (live 28x24 sprites): assembly_reflection,
  graphics_prefs_dialog, assembly_mate_dialog, assembly_positions, in_context, assembly_standard, create_selection,
  assembly_view (incl. panel_frame svg close), assembly_mass, assembly_props, assembly_behaviors, appearance,
  assembly_panels, menus. Left: sketch.rs sk-dim-cancel 22 (dimension popup, not a dialog header).
- Dependencies panel rows: mask svg 15 px -> full-colour feature_icon at LIST_ICON (17; 15 px ink = live) at .8,
  so the sketch row shows its blue badge like live (both panels). Building to verify.
- Left as is: thickness.rs (no live reference: Thickness analysis is unavailable on the capture account), release
  dialog × (no live capture), start_window TriangleAlert, mv-x (Explode editing bar; no capture).
- Notes for owners (not icons): tool search finds neither "Section view" nor "Reflection analysis" (live does);
  live's shortcut modal has a ↗ pop-out before × and is ~1000 px wide; assembly Measure panel lacks live's
  query box / Measure type / unit selects; analysis menu has "Thickness analysis…" (live doesn't on this account);
  Display states panel layout (live: name field first, underline input, no Update row); env picker tabs wrap in
  live; Section view header ~7 px higher than live, chip text lighter/smaller.

## Round 15 (2026-10-22, worktree /root/wt/icons2, branch agent/icons2) — sketch manipulators (lead, qa5 wf 26)
Refs: live/sketch/303 (offset), 1933-1934 + 301 (fillet), 136 (linear pattern), 1944b/1952 (slot), 91 (polygon);
behaviours.md "qa5 sketch tools". Code: crates/scene/src/sketch_mode.rs (overlay) + app part_studio/sketch.rs (labels).
- OFF1 (in transition snapshot): hollow arrow (`push_hollow_arrow`: #989fa3 3 px outline + 1.2 px white core; head
  31x23 px) 11 px past the offset copy, 18 px stem; no value box at first (bundle: offset manipulator
  `displayEditView:false`; Enter commits); `offset_value_label` (shown when the gap >= 30 px, live 123 vs 303) opens
  the box on click (`open_offset_input`).
- FIL1 (in snapshot): fillet radius manipulator = hollow arrow from the arc midpoint toward its centre (28 px stem),
  "R<value>" label up-left of a short leader to the arc's upper tangent point (`fillet_value_label`); arrow hit-test
  along the segment.
- PAT1 (uncommitted at the restart): hollow 16 px squares at seed and first instance, count arrow after the square
  (30 px stem), light-blue dash-dot seed->first guide, labels ~95 px diagonal with a leader + arrowhead,
  spacing dimension 101 px to the label's side with extension lines and arrowheads; count drags relative to the
  grab (`pattern_grab`). No grey sprite dots for linear handles.
- POLY1 (uncommitted): dash-dot construction circle; spinner 24 px out on the centre->rim ray, ◀/▶ drawn rotated in
  the overlay (hatched), count label upright (GPUI can't rotate text: lead to decide on a raster glyph).
- SLOT1 (uncommitted): pending slot shows its Ø dimension (dimension_* read the preview via `dimension_source`)
  and 4 tangent points; the diameter goes on the END cap, label at centre + (0, 2 x width) -> vertical radial leader.
  Constraints (coincident x4, tangent x4, offset x3 instead of equal pairs): skb's, note left in sketch-behaviour.md.
- Blocked at the restart: transition's crates/app/src/preferences_window.rs:1143 unclosed delimiter (another agent).
- Verified in app (after main 3ce6952): offset arrow (crisp: #989fa3|white|#989fa3 = live 303), fillet arrow + "R20"
  leader, linear pattern (squares, count arrows, 3x/1x leaders, 25 dimension), slot Ø20 above the end cap with one
  arrow (= live 1952), polygon spinner with the rotated count (= live 91). context/r15-*, r16-*.
- Follow-ups: `SketchEditor.screen_map` (set by the app from viewport rays) + `snap_to_pixel` put arrow starts and
  square centres on pixel centres; pattern leaders 1.5 px into the square with a filled 8 x 5 px arrowhead; count
  labels medium weight; an arc's diameter dimension is one leader to the near side (radial_dim).
- For skb: our fillet default is R20 where the newer live capture (1933) shows R5 on a ~30 mm rectangle (older
  behaviours line 212 says R20: likely geometry-dependent); live highlights the clicked corner point orange; live's
  rectangle shows corner constraint glyphs ours lacks. A new polygon starts with the last side count used.
- 10-22 ~15:xx: merged main (c209b8e), scene tests 61 ok, clippy -p noshape-scene -p noshape --all-targets --no-deps
  clean, guard 2 passed / 0 of 735; checked in app (context/r17-*, r18-slot): polygon spinner (edge after the clicked
  vertex, outward normal, rotated digit), slot Ø20 + join dots = live 1952, chamfer preview arrow. Marked ready/icons2.
  For skb (noted in sketch-behaviour.md): polygon circle radius, chamfer preview dims colour, picked slot line orange.
- Glyph hover (skb note, live sketch/27 + 1956): the hovered constraint's entities draw pre-highlight orange #ffc685
  (3 px) instead of the translucent yellow halo; master entities secondary yellow #f2ff80 (`constraint_masters`:
  superseded: live capture (behaviours "sketch constraint-glyph hover colours", sketch/2203–2217) shows every hover
  orange incl. symmetry/mirror lines; yellow only for a point-pair symmetric's owning lines (Mirror tool pairs) and,
  from sketch/1956, the slot OFFSET's centre line + end (`constraint_masters(kind, sk)`). Pierce / pattern glyph:
  live capture still pending.
  Test a_hovered_glyph_highlights_its_entities_orange_and_its_master_yellow; verified in app (context/r19-ghover).
- Assembly dialogs vs live (r21/r22): Insert dialog icons = live 01; the Composite parts filter now only when a Part
  Studio has a composite (live 01 vs 320; was always shown). Properties modal: ƒ, calculator glyph, undo/redo = live
  132/133; for ui2: live joins the read-only (grey) Mass field and the calculator button into one input group (shared
  border, no gap), ours draws two separate boxes. Named positions panel: no own icons beyond "?".

## Round 16 (2026-10-23) — dark-mode art for Onshape's static-colour icons (lead, from ui2's dark mode)
- Live: one sprite for both themes; `--static` vars keep light values in dark (behaviours "Dark theme icon sources").
  The 74 icons in /root/ui2-scratch/dark-icon-categories.json "static" now have `crates/app/assets/icons/dark/<name>.svg`
  (assets.rs loads them as-is in dark; other icons get the themed #333->#fff flip).
- Recipe (scratch tools/mkdark.py NAME WHITE_D): our light art with #333-at-opacity greys made solid, plus white
  faces under everything (`--os-icon-fill-primary--static` #fff) from OUR outline geometry (one path per subpath:
  overlapping subpaths cancel by winding). Badges (error/warning/info/toasts, update-all, regen) get a white copy of
  their own silhouette so the knocked-out glyph reads white. Hand-drawn: mate-connector-qlv-item (#666 rings + white
  ring/quarter), wire-body (white strokes), hole (#333 bore crescent), the stroked warning triangles.
- Reference: tools/darkref.py renders the LIVE sprite symbol (dark/icons2-dark-sprite.svg) with varsDark substituted
  (the ui-ref copies have vars resolved); tools/darkiter.py NAME [SVG] scores on #333 through the 2x raster path;
  tools/dv.py OUT NAMES shows ref|ours. Scores (d20): median 1.10, worst standard-content 4.1, menu-delete 5.6 (same
  look, slight size), mate-connector-qlv-item 4.4, plus a dozen 2-3.5. guard.py 0/74, guard test 2 passed.
- In-app check: run with XDG_CONFIG_HOME=/tmp/icons2/cfgdark (preferences.json {"dark_mode": true}) so the shared
  ~/.config/noshape isn't touched; compare with live/dark/dk-920-partstudio, dk-940-assembly.
- (fix) the preferences key is camelCase: `{"darkMode": true}`.

## Round 17 (2026-10-23) — dark art for the "mixed" icons (static parts + themed parts), lead: worst-visible first
- 82 more `icons/dark/<name>.svg` (156 total): the mixed list /tmp/icons2/mixed-base.txt (base = our light art through
  the app's themed flip), worst first. Per element, map the live symbol's var to its dark value
  (scratch tools/roles.py OUT NAMES paints each var a colour beside the dark ref and ours): `--static` stays light,
  themed flips (outline-primary/accent-tertiary #333->#fff, outline-secondary ->#bababa, fill-secondary #999->#d7d7d7,
  fill-tertiary #666->#999, fill-primary #fff->#333, accent-primary ->#3e94ff, cancel ->#ff6b6b, success ->#35bc35).
- White faces go on the stroke CENTRE (or the inner edge of a filled outline), not the outer edge: an outer-edge face
  leaves an anti-aliased white halo around the dark outline on #333.
- Helpers: tools/sketchdark.py (sketch tools: curve ->#bababa, point rings ->#3e94ff with white centres, rings r<=2.5
  only), tools/tailconv.py (only static part = #999 fill-secondary), tools/lv.py (live light|dark|ours light|ours dark).
- Not needed: edit-curve, boolean, flip*, curvature, paste, xyz-along-z — the themed flip already gives the live
  colours (#333 at .5 flipped to #fff at .5 on #333 = #999).
- Scores (d20) all <= 3.3 except snap-mode 4.8, stack-closest 2.9; was 9-47. Worst three static: standard-content
  4.1 -> 0.5 (washer face moved to the washer), menu-delete 5.4 (white X instead of a white disc; the rest is our
  light art's size, matched to the live menu), mate-connector-qlv-item stays 4.4 (sub-pixel ring placement).

## Round 18 (2026-10-23) — leftovers, dark sweep of the whole app (lead)
- Block 1: menu-delete redrawn on the sprite's geometry (disc r7 centred, X arms ±3.5 width 2): live's menu disc
  is 11 px at 16 = the sprite's size (live menus/rmb-asm-list-instance.png), ours was 12.2 and 0.5 px off. light 0.07.
  Inspection empty-state illustration 130 -> 172 px (live part-studio/77). Section chip ×/plane were already done.
  anim-reverse = 16x18 ink at 20 (live ~16x17): done. importDerived/importForeign icons: done.
- Kit icons left WITHOUT a live reference: release_dialog Close x2 (Release is "Upgrade" on our live account),
  analysis hq-off, thickness Loader/ChevronDown, start_window TriangleAlert (broken document). Assembly triad value
  box (assembly_panels mv-ok/mv-x): live 2306-2309 is a bare input box with no ✓/× and no label: a layout change
  for the assembly owner, not an icon swap (reported to the lead).
- Block 2 (dark sweep), part 1:
  * assets.rs flip_themed_colors no longer touches `<mask>` contents: it turned the masks' black (cut-out) into
    white, so every masked gap (help "?", gaps around arrows/badges, sketch point holes) vanished in dark. Test
    flip_themed_colors_keeps_mask_cutouts.
  * scratch tools/darksweep.py: every mapped icon AS THE APP DRAWS IT in dark (dark file, else the light art through
    the same flip) vs its live symbol with varsDark; lists "excess" = dark score minus light score. 74 -> 20 left,
    all context-coloured (currentColor: documents-page nav, history, chevrons, tab manager; tinted illustrations;
    tm-detailed's literal #333) — checked in the app / requested live (REQUESTS "icons2 10-23 07:30").
  * 49 more dark files: symbols with LITERAL colours (cube, sphere, plane #d2d2d2 faces, file, mbd-status) or
    static vars that ui2's lists missed (feature-folder, hole-*, import-foreign/derived, show-mates, insert-rigid,
    file-image/cad/dwg/pdf with themed page fill, fill-tertiary #666->#999 icons: settings, chevron-down, home,
    open-*, fs-info; accent-cancel -> #ff6b6b: fs-error, dialog-cancel; ...).
  * delete (assembly/sketch menus) = menu-delete's sprite geometry.
  * Palette.illustration (#eee / #555 = --os-icon-fill-quinary) for the tinted empty-state illustrations (notes,
    documents); notes keep their measured #ebebeb in light.
  * In app (dark): feature list planes = live dk-924; feature menu icons = live; documents nav = live dk-910.
    Not icons, for ui2: our Create button is #3e94ff, live's dark Create is the darker #1651b0-ish blue.
- Block 2, part 2 (in-app dark vs live dk-9xx):
  * Sketch toolbar = live dk-932 except sketch linear pattern (top-left square outline-secondary #bababa, was white):
    fixed. scratch tools/autotry.py tries var-based recolourings (accent red/green flips, outline-secondary grey)
    against the live dark symbol and keeps the best: +13 (check, cancel, mate limits, offset, chevrons, ...);
    tailconv.py (--themed-secondary) for the mate icons (outline-tertiary #888 -> #ccc: revolute, slider, ...),
    link/select-box (fill-secondary themed #d7d7d7); inspection/config illustrations #555 (fill-quinary).
  * Toolbar split-tool caret: the `expanded` sprite is 10x6 and live draws it 1:1; ours was the glyph at 12 px
    (12x8, thicker). Now 10 px = live's 10x6 (light and dark), ui.rs tool_caret + custom features button.
  * Tree expanders: Palette.icon_tertiary (#666 / #999 = --os-icon-fill-tertiary). ui::caret used rgb(0x666666),
    which DARK_HEX sends to #ccc; live dark is #999 (dk-940). Assembly instance/mate row chevrons were the 11 px
    ui-chevron in pal.text (white, 5x10); live = the same 8x12 expander as the headers in #999: now expander-* 14.
  * Selection-list/chip × (ui-clear #666) -> icon_tertiary: live dark #999 (dk-926), was #ccc.
  * Mate dialog / assembly "Solve" buttons were bg(gpui_kit::white()) with inherited text: white-on-white in dark.
    Now rgb(0xffffff) (themed) + pal.text. 51 other bg(white()) remain app-wide (ui2's call; reported).
  * Feature dialog (dk-926): flip arrow, tolerance, ✓/✗ match; live's ± "+" is crisp where ours (and the sprite at
    1:1) is split over two columns: a sub-pixel placement difference, not art.
  * Open for live (REQUESTS icons2 10-23 07:30): sheet metal, more context menus/dialogs, tab manager, history,
    properties panels in dark; glyphs tinted rgb(0x666666) (ui-dropdown-arrow, ui-caret-down, close, info, pin,
    folder, undo, import-file) wait for those captures (sprite vars differ per glyph).
- Block 2, part 3 (live answered dk-971…dk-9a2):
  * Sheet metal dropdown = live dk-972 except Flex PCB model / Finish flex PCB model: pcb-start / pcb-end redrawn
    on the pcb-*-button sprites (curved bend, chip box; 6.4/7.7 -> 2.3/3.4 light); pcb-end's ✓ is white in dark.
  * Context menus (dk-974/975/983/984) = live except "Change to version…": live uses the inserted-version symbol
    (dashed line + dot): new icon change-version (assembly + drawing tab menus). com-indicator ring 0.8 -> 1 (live).
  * Disabled dialog ✓ (15 places): was #cce9cc (light-only colour); live = the whole button at 20% opacity, which
    gives #cce9cc on white and (41,70,41) with a (92,92,92) ✓ on #333 (dk-978). Now bg(accept) + opacity(0.2).
  * Running toolbar tool: live #eaeaea light (dialogs/*-01-open, sketch/1933/1940) and #555 dark (dk-978/932):
    --os-icon-button-fill--selected; was #b2ddf6 / #134e96.
  * Assembly linear/circular pattern flip: showed the flipped art when not flipped (dk-987): now like the Part
    Studio dialogs (*-opposite art when unflipped).
  * "?" help dots (30 places; ui2 reported them darker than live in dark): live = the help-button sprite, a disc
    with the "?" cut out. Now glyph("help-dot") (opaque copy of help-filled: a glyph tint keeps the art's alpha)
    in theme::help_dot(light) = each place's measured light grey, #aeaeae in dark. Measured after: disc 160 light /
    174 dark = live (fillet-01, dk-978); was a bold text "?" on a disc DARK_HEX mapped (#aaa -> #888, darker).
  * Not fixed (ui2's, reported): Insert dialog header is a white bar in dark; mate-type flyout white in dark;
    51 bg(gpui_kit::white()) app-wide; Create button blue on the documents page.
  * Dialogs hole, fillet, linear pattern, mirror, sweep, flange, fastened, revolute, asm linear pattern, replicate:
    icons = live dark.

## Round 18 block 3 (2026-10-23, new icons2 after the approval-prompt hang; predecessor 234b2118)
- Scratch: everything under /root/icons2-scratch (no /tmp: approval prompts hang the agent). App: tools/app3.sh
  start|shot NAME|stop (binary copy /root/icons2-scratch/bin/noshape, state in run/, shots in v/, window by PID);
  tools/sbs3.py OURS LIVE OUT x0 y0 x1 y1 k = live | ours crops.
- Light disabled dialog ✓: `crate::ui::disabled_accept` (light = #cce9cc fill, so the white ✓ stays white;
  dark = opacity 0.2 as live dk-978). 15 sites + Insert dialog (which also drew an 18 px `check` glyph instead of
  the 24 px ok sprite, live dom/dialog-assembly-insert).
- Mate dialog vs live asw-* / dom/dialog-mate-*: Tangent = flip only (26 px row, icon follows the flag alone:
  -opposite empty and filled), Width = no tools (22 px help row); neither has the mate-type select.
  Other types: flip/realign/play/Solve = live (Simulation connection row is simulation, excluded).
- Relations (gear/rack/screw/linear): type select is now the OsSelect (`os-select-bootstrap`, 8x4 caret) like the
  mate select, flush at the dialog edge; help ? on the Reverse direction row (live dom/dialog-relation-*).
- Assembly linear/circular pattern, mirror: connector button = mc-select 20 px in a 26 px button flush against the
  field (was mate-connector 18/24 with a gap); flips 19 px in 26 (was 18/24); Mirror strategy… = mirror icon 20 px,
  27 px row, 2 px pad, 14 px text (was part-studio 18). Replicate scope select = OsSelect. Pattern help ? 4 px from
  the bottom right: over the last checkbox row (linear w/o second direction, circular), else after a 22 px spacer.
- Sheet metal Bend / Sheet metal model: icons = live (no change).
- Not icons, for the assembly owner: our Mirror shows "Create mirrored (opposite-handed) parts" which live's
  dialog doesn't (dom/dialog-asm-mirror-*); Replicate's empty "Parts to find match in" lacks live's pink fill.
- dialog-ok (every dialog ✓) redrawn by hand as a 2.5-unit stroked polyline with a round joint (20-grid
  4.4,8.9 8.9,13.5 15.7,5.7), read off the sprite at 200 px on a grid: alpha diff 0.025 -> 0.009. The old flat polygon's
  arms were ~1 px thinner than live's (assembly/2215, 2262 at 1x). Rule 7: a parameter-search fitter was used first
  and deleted; the values here are hand-picked. resvg draws the sprite's short arm 1 px lower than Chrome does.

## Round 19 (2026-10-23) — whole-app sweep, cursors, feature/parts-list icons (lead)
- Tools (scratch): b4/inuse.json = icons the code uses (string literals + format! prefixes: 677 of 737 files;
  feature-list icons are built from type ids, see b4/feature-icons.txt), tools/dv3.py (ref | ours | diff at 40 px),
  tools/sc.py NAME FILE... (rank.py's score for candidate files), tools/nat4.py (png refs at the crop's native size +
  floor), b4/floor*.json. app3.sh: `CFG=/root/icons2-scratch/run/cfgdark` starts the app in dark with a PRIVATE
  config dir (darkMode in preferences.json; never flip the shared ~/.config/noshape).
- **Metric floor:** rank.py's 20/40 px score of Onshape's own sprite moved 1/4 px. 77 of the 80 sprite-ref icons
  above 3 already beat their own floor (e.g. composite-part 5.7 vs floor 14.2): the rest is sub-quarter-pixel.
  PNG refs scored at native size (nat4.py): all within floor except transparency (blurry ref, CAM-only) and
  tab-card-empty-assembly (+2.1 grey levels, 100 px hover card, line weight).
- Fixed: parallel 4.44 -> 2.85 (bar lengths, blue 1.0); part-configured redrawn to the sprite's design (dark-blue
  table badge top-left, 22.2 -> 1.5; dark file = static colours + white face); dark sheet-metal-end face no longer
  fills the open box top (excess 2.1 -> 1.1); 12 hatch swatches' tone (median |d| 2.3 -> 1.1 of 255);
  drawing-suggested-views inner outlines 1 unit; Create drawing tiles drawn at 63.25 (= live's 1.15 px/unit, ink
  62 px = live); exploded-views-empty 152 -> 170 px, mt 98 -> 81 (live 800: 148x149 ink, ours was 132x131).
- Map: override-b4.txt (in-use icons that had no ref: part-configured, tab-*, list-underdefined-overlay, ...);
  projection-first-angle -> sprite:first-angle (was a 400 px png; 6.57 vs floor 10.1).
- Not fixable / by design: file-pdf, document (rule 6 / third-party marks), calendar, transparency and the
  view-orientation icons (blurry refs), onshape-labs-blue (noshape cube in the flask, rule 6), hatch textures
  (moiré in the live crops: matched in tone), ui-dropdown-arrow / expander-* / ui-chevron-down (tinted glyphs or
  blurry refs; carets match live's collapsed/expanded sprites at 4x to 0.25 px), weld-arrow (+1.5 grey, variants worse).
- Cursors: docs/parity/cursors.md + live dom/cursors.txt already cover every state; the feature-dialog count
  cursor row is stale (implemented: part_studio/mod.rs counts the dialog's picks). Live requests filed for the
  snap-mate drag, fixed-instance drag, busy regen and the Mate connector tool before its first pick.
- Feature list: 88 feature types; all resolve to icons except hidden copyPart / nameEntity / sheetMetalRecognize
  (no file: imported docs only). Light: all at or under the floor; dark: every one within 2 of its light score (sheet-metal-end fixed).
  No live ref for point, refold, unfold, sheet-metal-rip (request filed, with a dark Finish-sheet-metal row).

## Round 20 (2026-10-23) — look parity for ui's backlog: Section view, Named views, Analysis menu, shortcuts pop-out
- Tool: /root/icons2-scratch/tools/runs.py IMG x|y N A B [tol] = colour runs down a column / along a row (edge
  positions, 1x, live vs ours). Dark captures of all four requested (REQUESTS icons2 10-23).
- Section view (part_studio/section.rs), measured against part-studio/70-71: dialog top 3 (as feature dialogs),
  tabs' underline drawn over the label box (2 px blue / 1 px #d7d7d7, labels on one baseline), fields 3 px in and
  7 px under the tabs, 5 px apart, plane button flush, empty field 27 px, chip 22 px bold, help = help-dot glyph
  (#a0a0a0 = live) 4 px from the bottom. Column x=300 now equals live line for line (79..241).
- Caps: the bundle's endcap shader mixes a SELECTED plane (the dialog's) 80 % toward rgb(255,198,133) → live's
  tan #e9c698. New `Scene::clip_selected` (render), `SceneOptions::section_selected` (scene), CapPlane.index.y,
  fs_section_cap. The cap pipeline lost its face depth bias so the coplanar datum plane (strictly-in-front
  transparent pass) no longer tints it (live 73/74). Open: after ✓ ours is 0.9 x part colour (bundle maths:
  selector 0 mixes 10 % to black) where live 74 reads (143,196,228) for #9dcfed: request filed with known colours.
  Assembly section view is a different, simpler panel than Onshape's dialog: capture requested.
- Named views (part_studio/view_tools.rs) vs part-studio/75-76, psb/nv-911/915: close = GLYPH_CLOSE 23 px #333 at
  .5 (11x11, darkest 153 = live), 30 px square buttons, "+" #aeaeae in #ddd until a name is typed, field 1 px lower,
  "No views created" row 26 px (dialog 117 = live), view list = os_ui::select_field_flat (new: native <select>,
  underline only, black text, #777 arrow; Chrome's native list), remove = new os_ui::GLYPH_REMOVE_CIRCLE (opaque,
  our drawing) #333 in #aaa once a view is picked, 40 % in #ddd before (live 51 / 174). The OsSelect it used is gone.
- Analysis tools menu = live (rows, anchor, right edge 1878); 3 px wider only from font metrics (Flama vs ours).
  Thickness analysis is kept: paid-plan item (Help), absent on the free capture account.
- Keyboard shortcuts: ↗ (new icon popout-arrow, hand drawn: 11x11, #333 at .75 = live darkest 102) opens the new
  shortcuts_page.rs window (two columns: General, Sketch, Drawing | 3D view, Part Studio, Assembly, Feature
  Studio; keys right-aligned at 39.6 %, legend bar, our version in the footer — no PTC/Terms, rule 6/cloud).
  The modal is now window-level (deferred/anchored over navbar and panels), 1000 px centred, top 26, height
  window − 154 (live 616). Page section spacing and the popout window size: requested.
