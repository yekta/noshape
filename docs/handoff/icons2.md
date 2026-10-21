# icons2: one-to-one Onshape icon redraw (2026-10-21)

## State: in progress — redraws live in /root/icons2-scratch, nothing copied into the repo (release freeze; wait for the lead's GO)

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
