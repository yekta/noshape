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

- QA round 3 (psb section) fixes:
  - picks follow the active field's filter (`behaviors.rs::pick_kinds` / `sync_pick_filter`;
    the viewport priority returns `i32::MIN` for kinds the field can't take, and the render
    pick skips those), so a short edge's vertex or midpoint no longer swallows a Fillet click,
    and a datum plane no longer blocks a face for analysis tools; the midpoint snap only
    applies when the field takes points;
  - Hole: a sketch clicked in the list (or preselected) gives all its points (`sketch_points`);
  - rows after the edited feature show rolled back while it's open (unless Final);
  - the row selection is cleared when a dialog opens and when it's accepted, and a list click
    the active field can't take flashes the filter message instead of selecting the row;
  - Enter (apply) then Enter (commit) re-checked: works. All verified in the app; the
    dimension-box item is skb's.
- Thickness analysis (`part_studio/thickness.rs`, geometry in `model::thickness`: refined
  samples, BVH ray casts, rolling ball by occupied-cell search, gradients) with the color bar
  (Colors, Dim color scale, Method, editable / draggable bounds, movable legend), progress
  messages, Edit / Turn off in the menu.

- Menus and selection: region New sketch… / Offset plane… (sketch and plane features accept
  regions), curve Copy sketch entities (`copy_sketch_entities`, the sketch-mode clipboard JSON),
  Select ▸ on sketch entities with the live item sets (`sketch_related`; part faces use the same
  face set incl. Bounded faces, `select::select_bounded`; the part edge set awaits the live capture),
  Align view with vertical axis on sketch lines, sketch-curve midpoints (app + regen
  `implicit_connector`), multi-feature Section view… (`section_through_all`), parts list Copy
  Part N, feature filters :partnumber / :revision (`interaction::filter_features`).
- `model::fits`: ISO 286 hole / shaft deviations (H7, g6, K7, p6…) for the Fit tolerance types;
  the Hole dialog side waits for the live capture (REQUESTS.md).

- Release management: `model::release` (ReleaseLog, revision scheme, revision history,
  obsoletion, validation; JSON in store meta "releases"), `app/src/release_dialog.rs` (Create
  Release candidate / Review Release / Revision history / Create Obsolete candidate), parts list
  Release… / Revision history, versions graph triangles + legend + Releases ▸. asb's drawing
  revision tables read `ReleaseLog::revisions(part_number)`.

- Per-dialog behaviour pass (lead, 19 Oct; diffs of the 25 captured dialogs vs ours):
  * Error states: `FeatureError.faulty` / `err_at(&["param"], msg)` (regen/eval.rs) →
    `FeatureState.faulty` → `FeatureDialog::set_faulty`. A faulty, non-active selection field is
    pink (#f9ebec, border #bd3039) with the message as tooltip (fallback: the first empty
    selection field); fields otherwise show their label as tooltip, selects too. Messages use
    Onshape's exact text (docs-raw/fs-std/errorstringenum.gen.fs has every one). Templated
    names get "[Type]" in the title tooltip ("Ø 0.25 in ↧ 0.5 in [Hole] did not regenerate
    properly: …"). An empty sketch shows the blue "Select a sketch plane" bubble, no tooltip.
    Mate connector with no Origin fails ("Cannot resolve entities.").
  * Dialog layout: SHOW_LABEL selects wrap to label row + full-width select when they don't
    fit 194 px (measured with `ui::text_width`); groups nest (stack in render_params); plain
    group titles (GROUP_LABEL, `common::group()`) are 13.3 px, red when a field inside is in
    error, contents indented 20 px. Sheet metal model: Selections / General / Material /
    Relief groups. Loft: driven-query Profiles / Guides / Connections, "End conditions" group,
    NORMAL_DIRECTION / TANGENT_DIRECTION ids. Helix, Fillet (single side faces, overflow only for
    Radius, partial bound "Select an entity or mate connector"), Mate connector Move checkbox,
    Thickness 2 tolerant, MC button only on fields that create connectors (not part fields,
    not Linear pattern Direction; `Hints::NO_MATE_CONNECTOR_BUTTON`), tooltip "Select mate
    connector(s) (ctrl+m)", Transform XYZ axis icons (our own SVGs), Hole opens on Inch.
  * Sweep / Thicken auto Add + merge scope (sweep's `profiles`; thickenEditLogic).
  * Part Studio dynamic suppression is "Unsuppressed if" (suppressed while false), Onshape's
    220 px dialog, Unsuppress / Edit dynamic suppression once set.
  * Import feature: "Import N", Allow faulty parts on.
  * Plane (port of cplane.fs) and tolerance options / fits were done by helpers; see below.
  * Still open: Create selection side button + dialog (capture requested), Copy here triad and
    Move face arrow manipulators, Hole computed depth ("Multiple"), sketch Text dialog (skb).

## Pass 5 (19 Oct, after the quota handover)

- Create selection rebuilt to the live dialog (`part_studio/create_selection.rs`, replaces the old
  panel in behaviors.rs): floats at the right-click point (`ui.menu_pos`, saved by
  `close_context_menu`), Faces / Edges tabs, a native-style select (no search field), Selection
  field (picks toggle; `cs_pick` takes clicks before everything else), ☐ Select patterns
  (`select::with_pattern_instances`, pattern copies traced through `Role::Copy`), result line,
  [Add selection]. Target `CsTarget::{Selection, Field(path), Flatten(i)}`. Feature fields with
  `Hints::SHOW_CREATE_SELECTION` (deleteFaces, moveFaces, surfacesAndFaces, replaceFaces,
  partingEdges, pattern/mirror "faces") get the 26 px button (`DialogEvent::CreateSelection`,
  `FeatureDialog.cs_target` makes the field white + blue); Flatten's two fields have it too.
  Fill surface's "Edges" array (fillSurface.fs has the hint) still lacks the button.
- Select ▸: part edges get the live 3-item set (`Criterion::SKETCH_EDGE`), no separator before
  Create selection…, empty-space Select ▸ = Create selection… only (the invented "All parts…"
  list is gone, capture requested). Hole/Fillets on a non-matching seed find nothing; Bounded
  faces without a closed boundary = the seed (live); Loop/chain needs a face (`select_loops`).
  Convexity follows qConvexConnectedFaces: tangent edges count as convex next to an exterior
  fillet, concave next to an interior one (`curvature_sign`).
- Hole: `FeatureState.computed` / `EvalCtx.computed` (Onshape's setFeatureComputedParameter):
  Up to next / Up to entity report holeDepthComputedV3 ("Multiple" for several points). Names use
  `interaction::display_name_computed(f, shown)` with hole.fs's full template; `shown` gives
  computed values and lengths converted to workspace units (`feature_dialog::shown_length`), so
  a mm document reads "Ø 6.35 mm ↧ 12 mm" like live.
- Hole callout in the graphics area while the Hole dialog is open (`part_studio/hole_callout.rs`,
  text via `interaction::hole_callout_3d`; `HoleRecord.exits` makes a blind hole that breaks
  through read THRU, like live). Filled sans glyphs in the face plane, stroked ↧ ⌴ ⌵.
- Copy here / Translate by XYZ triad (`part_studio/triad.rs`, `Manip::Triad`).
- Flatten ▸ Export PARASOLID: `Flattened::loops` + `make_face` / `sew` + kern's `write_parasolid`.
- Release (audit against the 13 Release__ help pages, 19 Oct): tab menu Release… (last, after
  Export…; help image Release_highlighted2) and Revision history for Assembly / Drawing / Variable
  Studio tabs; `release_dialog::tab_items` (Part Studio → its solid parts; Assembly → itself +
  BOM parts / subassemblies, recursive; Drawing → itself + what its base views show; Variable
  Studio / file → the tab); versions panel "Create release candidate" button (`release-create`
  icon, active tab's objects); obsoletions mark the version of the release they obsolete (no new
  version, `ReleaseLog::release_version_of`); the State column is "In progress" for workspace
  items; Release name starts empty; imported part revisions kept on first release; Not revision
  managed items get no revision (`ReleaseItem.not_revision_managed`); tab part numbers: Assembly
  Properties, others store meta "tab_part_number:<id>"; drawing revision tables now read the log
  (`drawing_revision::revisions(doc_model, tab)` → `tables::revision_rows`); Revision history
  rows show User (version author), Document, a clickable Version (opens it read-only) and
  "View obsoletion". Parts list Properties… now opens the shared metadata modal
  (`open_part_meta`; Rename / Assign material… still use the old dialog, capture requested).
  Also added: Release drafts panel (rail icon `release-drafts`, `ReleaseDraftsPanel`,
  double-click → `open_draft`; Save draft bottom-left with a ▴ Save / Delete draft menu),
  "Add Items to Release" (`open_add_items`: Current document, type tabs, search, Part Studio
  tree, picked list), Include associated drawings (`associated_drawings`), search box + type
  filter, item names open their tab, ⟳ Refresh, Review Release ▸ Clone (workspace only),
  Properties modal release icon (`properties_dialog::open_properties_with_release`, used by
  the parts list's Properties…).
  Still missing (local scope): per-row Edit properties / Export icons, Revert to last
  revision, form undo/redo + yellow "modified" markers, configurations, Add Items "Other
  documents" tab, Add to Obsolete candidate, warnings for obsoleted references, revision
  schemes (numeric/custom), assemblies' Properties release icon (assembly_props).
- Item label templates (`ParamSpec::item_template`, FS "Item label template"): driven rows and
  array item headers; set for variable fillet vertices / points and Fill edges (Fill's edges
  are now a driven query with the Create selection button); custom features parse it.
- Verified in the app: Select ▸ menus, Create selection (Protrusion/Hole, Add selection to the
  selection and to Delete face's field), hole computed depth / Multiple, hole callout.
  Later verified too: triad drag (X 18 mm), Flatten → Parasolid (.x_t written), the Hole title
  in mm, tab Release… → draft → Release drafts panel → Release, Add Items to Release.
- Open: the feature list row of a Hole being edited reads "Hole 1" (live: the templated name
  while editing); the tab menu puts "Create Drawing of …" last, the help image
  (ui-ref/context-menus/Release_highlighted2.png) has it after Copy to clipboard (lead / asb).

## Pass 5b (19 Oct, live answers)

- Hole callout: one per feature at the middle hole, no count, "↧5" without a space, leader to
  the part's edge (bbox exit along the leader), 11 px cap height. List row of a new / edited
  Hole shows the templated name (`render_editing_placeholder`, `render_feature_row` draft).
- Triad: top-face centre, 45 + 20 px, live hover colours, translucent other handles while
  dragging, one-decimal values; manipulator hover hides the geometry hover.
- Parts list: inline Rename (`ui.part_renaming`), Properties… with Reset all
  (`open_properties_with_release(spec, reset_to, …)`), Assign material… panel
  (`material_panel.rs`, data in `material_data.rs` generated from
  live/dom/material-library-onshape.txt; mechanical properties known only for Aluminum - 6061,
  `known_properties`). The old generic part dialog (`open_part_properties`, MATERIALS,
  SWATCHES) is gone. (The Add material library modal and the Transform ghost are done in pass 6.)

## Pass 6 (19 Oct, lead's "still open" list)

- **Add material library** (`material_libraries.rs`): 800 px modal, 195 px left nav (Onshape,
  Owned by me, Recently opened, Created by me, Shared with me, Public), Library | Document |
  Owner table of the local documents' Material Library tabs, Add (disabled until a row is
  picked) saves to config_dir/material_libraries.json; the Material panel's library select lists
  them (`material_panel::Source`). Account preferences gained **Material libraries** (Remove) and
  **Release management** (Revision scheme, custom labels, Unreleased revision suffix, Save
  release settings) in `release_prefs.rs` (config_dir/release_settings.json).
- **Revision schemes** (`model::release::RevisionScheme`): Alphabetical (skips I O Q S X Z),
  Numeric 1/01/001, Custom list; `ReleaseLog.scheme` (not saved with the log; set by
  `release_dialog::release_log`). asb: the unreleased suffix is
  `release_dialog::release_settings().unreleased_suffix` (default "*").
- **Transform ghost** (live 445–446): while a Transform moves parts (not Copy part / Copy in
  place) and they moved, the originals are drawn faded (`ui.transform_ghost`, computed before
  each scene build) and carry the selection; the moved copies are display-only
  (`SceneOptions::display_only`: no picks, pick base past the state buffer). The ghost stays
  after the drag ends, as live.
- **Parts fields take the part**: `kind_matches` ignores the mate-connector / point-body bits for
  part faces, edges and vertices when the field also takes bodies (Transform, Pattern, Mirror
  "entities" now read "Part 1" like live 445, not "Face of Extrude 1").
- **Part outline in dialog fields** (live 440): new render state `OUTLINE` (128): a back-face pass
  extruded 2 px in screen space (`vs_outline` / `fs_outline`, `Pipelines::mesh_outline`, only
  drawn when some state has OUTLINE) gives the amber silhouette with untinted faces.
  `refresh_scene` flags the faces of Body refs of the open dialog. (Built after a shader fix;
  verify in the app.)
- **Release dialog** (`release_dialog.rs`), restyled to the help image
  savedraftbutton-relcandidatedialog: own 50 px title row (20 px title, bordered Refresh, the
  "needs attention" link at the right), no × bar (`release_modal`, 820 px); a gridded table
  (item 270 | Revision 140 | State 140 | Part number 150 | actions) 212 px tall with the item
  column line running down; rows 46 px with a 54×40 **picture** (`release_thumbs.rs`, rendered
  offscreen when listed and on Refresh only, as the help describes), name over italic type, red ✕;
  bordered 30 px Revert / Edit properties / Export buttons; two-column form with bold labels;
  footer bar with bordered undo, faded redo, Save draft, Apply, Release, gray Close.
  Extras: per-row **Edit properties** (the part's own Properties: material and computed mass
  too) and **Export** (opens the Export modal over the dialog: `workspace::request_export` now
  calls the window's Workspace through `Root` because actions don't reach it from a modal);
  **Revert to last revision** (row button and State header button; "Item has not changed since
  its latest revision." / "Only the item's properties have changed…"; `since_release` compares
  the head and the release version both loaded from the store); **undo/redo** in the dialog
  (one step per field edit: `record_typing`); **configurations** (Add Items configuration
  inputs + Generate, configured part names, `DocStudios::context_in`); **Add to Obsolete
  candidate**; Revision history rows have **Export** (views the released version, then Export).
- **Add Items to Release**: own title row, thumbnails ("Generating preview" on gray while
  rendering), branch row, picked list 300 px rows with pictures and red ✕, gray Cancel.
- Verified in the app: release → reopen → Revert, Export over the dialog, Edit properties,
  undo/redo, Obsolete candidate, Add Items pictures, Transform ghost.
- Open: the ⋯ menu of the release dialogs (View audit log, View workflow; Help
  Release__creating_a_customized_release_workflow), **Other documents** tab and the version /
  branch buttons of Add Items (release items from other documents need a document reference in
  `ReleaseItem`), Numbering schemes (sequential part numbers and the # buttons), Release
  management's Workflows section. Full material properties wait for
  live/dom/material-library-onshape-full.csv.

## Lint notes (19 Oct)

- `model::fits::tables` ISO deviations are whole/decimal micrometres (`limits` scales by 1e-6;
  ANSI stays in inches); the mm values like 0.318 tripped clippy's approx_constant (1/π).
- Fixed in my area: cplane.rs literal precision, detail.rs `then_some`, type aliases in
  properties_dialog / release_dialog / flatten_tool / analysis / thickness, config_panel entry
  API, thickness test FRAC_1_SQRT_2. `cargo clippy -p noshape --all-targets` shows nothing in
  my files; the remaining warnings are in assembly_*, drawing_*, scene_builder, session,
  viewport, workspace.rs:937, export.rs, in_context.rs, model custom/host, blend_section,
  detail_a.
- The feature list row of the feature being edited now shows the draft's name (compiled, not
  yet seen in the app).

## For asb: release data of drawings (revision tables)

The release log is saved with the document (store meta "releases", outside the undo history;
it persists in the .noshape file and survives reopening). Read it with
`crate::release_dialog::release_log(&DocModel) -> noshape_model::release::ReleaseLog`.
`drawing_revision::revisions(doc_model, tab)` is already hooked up:
`noshape_model::drawing::tables::revision_rows(&release_log(doc), tab)` (one row per
release of the drawing tab, pending rows for saved drafts, obsoletions marked). Other useful
calls: `ReleaseLog::revisions(part_number)`, `latest_revision`, `state`, `at_version(version)`,
`release_version_of(part_number, revision)`; a drawing's part number is
`release_dialog::tab_part_number(doc, tab)` (store meta "tab_part_number:<id>"). Drawings are
released from the tab menu (Release…; the drawing's parts / assemblies come along) or with
Include associated drawings.

## Next (priority order) — no deviations from Onshape are accepted

1. Analysis tools (`part_studio/analysis_tools.rs`, geometry in `model::analysis`): Curve/surface
   (Shift+C), Deviation, Connection (G0–G3 on face edges / curve joints, Onshape's colors and
   dialog, `analysis::face_connections` / `curve_connections`), Dihedral (Shift+D), Zebra
   stripes (instance flag → `zebra()` in shaders.wgsl, striped "room" sphere `Globals.room`)
   and Reflection (`part_studio/reflection.rs`: Select image dialog, procedural library maps
   in `environment_maps.rs`, `fs_mesh_reflect` with the map bound as an image) are done and
   verified in the app; offscreen tests `zebra_stripes_replace_shading`,
   `reflection_mirrors_the_environment`. Zebra / Reflection have ✓ (stay on) and ✗ (off).
   Thickness and Flatten surfaces (`part_studio/flatten_tool.rs`, `model::flatten`) are done
   too, as are Interference detection (`model::interference`), the curvature color map on the
   shared color bar (`thickness.rs::render_color_bar(Bar)`, `model::analysis::principal_curvatures`)
   and the Draft analysis dialog (`part_studio/draft_tool.rs`). Flatten can't export Parasolid
   (no writer). The environment maps are our own procedural ones modeled on Onshape's
   (live/env-maps is reference only, never commit those images). Still to do: verify Draft
   analysis and Interference detection in the app (needs two overlapping parts), and align the
   Interference dialog with the live capture once REQUESTS.md is answered.
2. Remaining ❌ rows in the checklist (Highlight boundary edges, Graphics preferences…, region
   New sketch/Offset plane, multi-feature Section view…, Create Drawing of Sketch,
   sketch-curve midpoints, :partnumber/:revision filters, FeatureScript search, fit tolerances).
3. Tell the lead: the tab context menu doesn't match live (Delete | Open in new browser tab,
   Rename, Properties…, Show code | Duplicate, Copy to clipboard | Select as document
   thumbnail | Move to document…, Export…).

## How to test

- Build (TEAM-RULES, 19 Oct): `/tmp/psb/build.sh` builds /root/noshape with
  `CARGO_TARGET_DIR=/root/target-shared` and copies the binary to `/tmp/psb/noshape`
  (other agents' builds overwrite the shared one). Batch edits; prefer `cargo check` and single
  tests. No private target dir or mirror any more.
- `/tmp/psb/start.sh` starts `/tmp/psb/noshape --demo` on `:102` (900 s timeout);
  `/tmp/psb/x.sh <name> [xdotool args]` acts and screenshots.
- The first double-click after launch is often missed (slow first frame). Open
  features with click + Enter, or click once somewhere first.
- Tests: `cargo test -p noshape-model --lib interaction` / `select` / `flatten` / `release`.

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

## From ui2 (19 Oct): Import feature (Onshape's "Import to Onshape" modal, live document/77)
The modal (ui2, import_dialog.rs) sets the importForeign parameters importAppearances, yAxisIsUp,
createComposite and a new hidden joinAdjacentSurfaces (ui2 added the sew in regen/features/importing.rs).
Still needed in regen, if it's yours:
- **Import appearances**: STEP colours (`k::read_step` → `ImportedShape.color`) are lost because the app
  stores imports as BREP text in `blobData`. Suggest storing the colours per body in a hidden param at import
  time (ui2 can write it) and applying them as part appearances in `import_foreign`.
- **Create composite**: the parameter exists but isn't applied.
- Onshape's "Import to this document" also creates an **Assembly** for multi-part files (one instance per
  part); part instances need the bodies' ids, i.e. after the first regeneration.

**From ui (render owner), 2026-10-19:** `draw_lists` in crates/render/src/renderer.rs now takes its
flags as `DrawMode { pick, section, outline }` (clippy too_many_arguments blocked integration). The
outline pass is unchanged; pass `DrawMode { outline: true, .. }` when you add more modes. Please put
new render flags into `DrawMode` rather than new arguments.
