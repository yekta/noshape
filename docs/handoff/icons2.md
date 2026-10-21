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
