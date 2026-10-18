# Handoff: Part Studio behaviour parity with Onshape (agent "psb")

Item-by-item status: `docs/parity/part-studio.md` (section 13 = new items found
in the live capture). Live reference: `/root/onshape-research/live/behaviours.md`
(organised by area) plus `dom/*.txt` (menus, dialogs, shortcut sheet) — live beats
the help docs.

## State right now (pass 3)

- Everything the previous pass built but never tried has been exercised in the
  app: selection count, missing references, Convert to expression, dynamic
  suppression, Copy/Paste sketch, sketch DXF export, multi-plane section,
  Select ▸, Create selection, 5.23 unit display. Fixed on the way: count position,
  5.23 (focusing showed the converted text), Convert to expression (now "true"/
  "false", focused), status message wrapping.
- Changed to match live:
  - two-stage Enter/Esc in dialog fields (`baselines`, `apply_on_enter`,
    `enter_applied` in `feature_dialog.rs`);
  - sketch curve under a region field picks the bounded region
    (`interaction::region_for_curve`, `curve_region` in behaviors.rs);
  - all regions of a sketch show as one "Faces of Sketch 1" row
    (`sketch_region_count`);
  - pick row menu = Clear all (+ Clear missing references);
  - all context menus rebuilt in the live order (menus.rs graphics area;
    mod.rs feature rows / default geometry / parts list). Agent "ui" then moved
    them onto its `os_menu` component (same content);
  - right-clicked entity stays highlighted (`ui.menu_target`);
  - Show dependencies panel restyled;
  - shortcut sheet = live tabs and 114 rows (`SHORTCUT_TABS` in view_tools.rs);
  - S toolbar = live 5×2 set, and its contents are editable in Customize toolbar;
  - Q reorients a mate connector's secondary axis;
  - an erroring feature commits with Enter (✓ disabled);
  - a new feature hides the sketch it consumes;
  - the selection count is drawn like Onshape's cursor digit.
- New helpers: `interaction::source_sketch` (Edit Sketch 1… entries), `export_bodies` +
  `open_export_dialog` (Export…), `align_view_vertical`, `section_part`,
  `hide_planes`, `clipboard_has_sketch`.

## Done since (pass 3, continued)

- Section 13: rollback bar (no menu, ↑/↓ after a row click, `#a0c7dd` focus, "Rollback bar" drag pill),
  selected-row tooltip "X <- Y", Tab skips dropdowns, manipulator drag ends with the value
  selected, Copy here… (Transform preset), Delete/Ctrl+Z verified.
- 9.8/9.11: camera menu in the live order with Display / Hidden edges / Tangent edges submenus;
  tangent edges phantom (dash-dot, new `LineSegment::DASH_DOT`) and removed
  (`MeshBuffers::smooth_edges`, cached per mesh in `GpuCache.tangent`).
- 6.24: Show dimensions (`part_studio/shown_dims.rs`).
- Section view dialog (`section.rs` rewritten: `SectionDialog`, arrow via `section_arrow` in
  the manipulator path, `SceneOptions.section_skip` for excluded parts).
- Face menu complete: Copy Part (`copy_part`; Ctrl+V → `paste_part`: Derived in another
  studio, Transform copy in the same), Export as DXF/DWG (`export_face`, `MeshBuffers::face_edges`).
- Parts fields take the part of a clicked face/edge/vertex (`part_for_field`).

## Done in pass 4 (lead's list + first-hour sweep)

- Section caps: separate `mesh_section` pipeline (`fs_mesh_section` writes the cap depth on
  back faces, part color hatched); offscreen test `section_caps_cover_the_cut`.
- Dialog row hover = state `QUERY` (#f2ff80) via `ui.query_hover`; arrow hover outline
  (`ui.manip_hover`); dialog resize grip (`DialogResize`, `ui.dialog_size`).
- Default planes: Rename in place (`default_names`), Size to Part Studio (`plane_sizes`),
  Show dependencies (`features_using`, `ui.plane_dependencies`).
- Select ▸ criteria Protrusion/Pocket/Hole/Fillets/Loop (select.rs) and empty-space Select ▸.
- First-hour fixes: Variable rows/titles "#name = value" + Name focused
  (`interaction::display_name`, `focus_first_field`); "+ Configure features" pick flow
  (`ui.configure_pick`, dialog `configure_mode`); dialogs show/edit the active configuration's
  values (`Configuration::for_editing` / `store_edit`, also in the preview).
- Tabs exactly like Onshape: delete with no prompt unless other tabs use it ("Delete tabs"
  warning, `Document::tab_users`), not undoable; undo is per tab (`UndoStack::undo_in` /
  `redo_in`, `Session.active_tab`); Versions and history > Restore to Main brings it back.
  Verified in the app.

## Next (priority order) — no deviations from Onshape are accepted

1. Analysis tools menu with Onshape's 11 tools; Shift+C = Curve/surface analysis (combs),
   Shift+D = Dihedral analysis (today Shift+C/Shift+D are curvature map/draft: wrong).
2. Remaining ❌ rows in the checklist (Highlight boundary edges, Graphics preferences…, region
   New sketch/Offset plane, multi-feature Section view…, Create Drawing of Sketch,
   sketch-curve midpoints, :partnumber/:revision filters, FeatureScript search, fit tolerances).
3. Tell the lead: the tab context menu doesn't match live (Delete | Open in new browser tab,
   Rename, Properties…, Show code | Duplicate, Copy to clipboard | Select as document
   thumbnail | Move to document…, Export…).

## How to test

- The shared tree often fails to build for a few minutes on another agent's
  half-done file. `/tmp/psb/sync.sh` mirrors `/root/noshape` into `/root/psb-src`,
  applies sed patches for those transient breaks **in the copy only**, and
  builds with `CARGO_TARGET_DIR=/root/target-psb`. Drop patches that no longer
  apply.
- `/tmp/psb/start.sh` starts the app on `:102` (900 s timeout);
  `/tmp/psb/x.sh <name> [xdotool args]` acts and screenshots.
- The first double-click after launch is often missed (slow first frame). Open
  features with click + Enter, or click once somewhere first.
- Tests: `cargo test -p noshape-model --lib interaction` (13 tests).

## Gotchas

- Concurrent edits: "ui" restyles dialogs/menus in the same files
  (`feature_dialog.rs`, `menus.rs`, `mod.rs`). Re-read before editing; use
  exact-match replacements.
- GPUI Input propagates Enter after emitting PressEnter, so the Part Studio's
  Accept binding fires too; the dialog's capture handler for Accept on each
  field is what makes "Enter applies" work.
- InputState doesn't reliably emit `Focus`; the field baseline is refreshed on
  every render while the field is unfocused.
- Overlays need `.occlude()` or mouse-downs start box selections underneath.
