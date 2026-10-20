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
- Sweep fixes (verified unless noted): parts fields take the part and show it **outlined**
  (render state OUTLINE, verified) and select its parts-list row / cross-highlight its feature
  (compiled); Tab stays inside feature dialogs (`.focus_trap`, compiled); a right-clicked
  feature row and its geometry stay highlighted while its menu is open (`ui.list_menu`) and
  Add appearance / Delete have their icons (compiled); a selection no longer pops the Measure
  panel: the one-line readout sits left of the evaluation tools (`selection_readout`, live
  part-studio/81, verified), the panel opens only with `[` / the button.
- **Material panel** (ui pixel pass): fixed 280×368 flush with the graphics bottom, "?" at the
  bottom right, 36×32 "+", picker 2/4 px insets; the open picker becomes the blue-bordered
  search field over a rounded 200 px list of 44 px rows (current #b2ddf6), the note hidden
  while open (live dom/dialog-assign-material*.txt, assign-material-03). Verified.
- **Tool search**: non-feature commands (`view_tools::COMMANDS`, `PaletteItem::Command`):
  "Assign material" (on the selected parts; nothing selected → info message). Not yet
  confirmed live: REQUESTS.md "psb (10-19 f)" asks which commands Onshape's Part Studio
  search lists; analysis tools stay out (live: not searchable).
- Test `qa_regressions::new_hole_follows_document_units_and_table` now asserts the live
  default (Inch tab, 1/4 = 6.35 mm, in mm and inch documents).
- Verified: Tab stays in feature dialogs (order: fields, then ✓ ✕; checkboxes and selects are
  not tab stops yet, unlike Onshape's DOM order), the right-clicked feature row highlight.
- **Consumed picks** (live dialogs/fillet-02…04): a fillet's / chamfer's edges (and removed
  faces) that the preview consumed are drawn orange from the model before the feature
  (`consumed_picks_overlay`, cached in `ui.consumed_lines`). Verified.
- skb's in-context round: (1) Measure no longer opens on Edit in context (panel only when
  pinned); (2) Ctrl+Alt+V → `CreateVersion` handled in workspace.rs (Create version dialog);
  (3) the pick pass draws unlit transparent meshes (planes) before lit see-through faces
  (context bodies), so faces win (renderer.rs draw_lists); (4) **Composite part**: Closed
  consumes the members (as before), open keeps them (`Body.composite` /
  `BodyDisplay.composite: CompositeInfo { members, closed }`), the open composite's body has
  an empty mesh in the Part Studio and is skipped when a whole Part Studio is inserted; inserted
  alone it is tessellated from its shape. Parts list: a "Composite parts (N)" section
  (composite-part icon). Tests: `qa_regressions::open_composite_keeps_its_members`,
  features_b composite test updated. REQUESTS "psb (10-19 g)" asks live for the Part Studio
  list layout.

## Pass 7 (19 Oct, new psb after the quota handover)

- Inherited half-done edits checked: `feature_dialog.rs` Tab stops (an `#[allow]` had slid onto the
  wrong fn) and `behaviors.rs` doc comments (Measure's comment had moved onto `consumed_picks_overlay`):
  fixed; everything compiles.
- **Tab stops in feature dialogs** (live behaviours.md Extrude "Keyboard" + dom/dialog-extrude-new.txt):
  `FeatureDialog::tab_stop(key)` hands out persistent `FocusHandle`s (`tab_stops` map) and
  `tab_stop_element` makes a div focusable with the `#1651b0` ring (a `.focus()` style) and
  Space (+ Enter for buttons) activation. Stops: checkboxes (`cb:<path>`), tolerance buttons (`tol:`),
  mate connector axis flip / realign (`axisflip:` / `axisturn:`), `?` help; the trailing flip buttons
  are gpui-kit `Button`s (tab stops by default); ✓ / ✕ are `tab_stop(false)`. gpui orders stops by
  render order, which is the DOM order. **Shared dialog components untouched**: all of it lives in
  feature_dialog.rs. Not yet verified in the app.
- **Material library**: `material_data.rs` regenerated from live/dom/material-library-onshape-full.csv
  (category + Poisson / Young / tensile yield / UTS / compressive yield / UCS, values as the CSV text,
  which is what the panel shows). Generator: /tmp/psb/scripts/gen_materials.py (csv + the old list).
  Document / added libraries map extra columns by header (`material_libraries::materials_of` now
  returns the whole `MaterialLibrary`).
- **Release dialog header** (bundle release-dialog.html): "1 item needs attention" / "N items need
  attention" + red filter button (`list-error-filter` icon, ours), ⋯ (`more`) menu with **View audit
  log** (JSON written to the temp dir and opened with `cx.open_url(file://…)`, like Onshape's new tab;
  entries OBJECT_CREATED / TRANSITION_STARTED / TRANSITION_COMPLETED with ordinals in the help's
  order). "View workflow" is not offered (template: only with a workflow definition). Package of a
  review / draft kept in `ReleaseDialog.package`.
- **Numbering schemes**: `model::release::{NumberingSchemes, NumberScheme, Propagation}` (+ tests),
  config_dir/numbering_schemes.json via `release_dialog::{numbering_schemes, set_numbering_schemes,
  next_number}`; `ReleaseLog.drawing_reuse` relaxes DuplicatePartNumber for a drawing + its part.
  Preferences: `numbering_prefs.rs` (embedded in `ReleasePrefsView`, after Release management):
  tabs, Number generation select, the flow chart, uniqueness radios, propagation select + note, Save
  changes (current tab only, Next never goes down). Release dialog: header # "Generate missing part
  numbers" (drawing shares its single shown part's number with reuse on), row # "Generate next part
  number", Release name # "Generate next release name" (not obsoletions); icon `part-number` (ours).
  Strings from the bundle. Unknown until live "psb (10-19 e)": other tabs' exact layout, default
  prefixes (we start empty, Manual entry), propagation notes for One / All workspaces.
- **Add Items › Other documents / versions** (`release_add_source.rs`, a child module of
  release_dialog): Current document | Other documents tabs; title row with Create version… (this
  document: `CreateVersion` action; other: opens its window) and Version graph (Main = workspace for
  this document, versions newest first); Other documents = filters / Search or paste URL / document
  list (same widget as asb's Insert), a picked document opens at its newest version. Items get
  `ReleaseItem.document: Option<LinkedDocument>` (`same_object` compares it); released at that version
  → listed as Released with its revision (`reverted`, reference only); release → our log + the linked
  document's log gets the package at the linked version (`mark_linked_versions`). Pictures render from
  the version snapshot (`thumb_groups`, `release_thumbs::key` includes the version). Edit properties /
  Export are inert on linked rows for now. Titles / tooltips per bundle: "Add Items to Obsoletion",
  "Add to Release…", "Add selected items to Release", "Cancel adding items and resume editing Release".

Queue still open: verify all of the above in the app; tool search (live "psb (10-19 f)" pending);
composite parts list layout (live "psb (10-19 g)" pending); everyday-modelling sweep.

## Pass 8 (19 Oct, third psb after a quota handover)

- Live answers: (e) release management is not on the Free account, so the release dialogs follow the
  bundle + help; (f) tool search lists only tools, so Assign material is out (`COMMANDS` removed;
  "Did you mean:" chips via `tool_synonyms.rs`); (g) the composite list matches live (Parts (N), then
  "Composite parts (N)" collapsed, tooltip "(open|closed) (N parts)", row menu = part menu).
- **⋯ › View workflow** (`release_workflow.rs`, child module `workflow` of release_dialog): the bundle
  shows every candidate has a published workflow (`getPublishedWorkflow(e.workflowId)`), so the item is
  always offered (not once discarded). Featherlight lightbox: solid #333 backdrop, white content with
  25 px padding, ✕ 25 px top right, diagram max(w−300,800)×max(h−300,400); nodes 180×60 (+10 current),
  4 px stroke in the node-type colour (initial #def1fb, inner #90cef1, good #aee28c, bad #dc777d), only
  the current state filled, labels 12/500 (700 current), links 2 px #ccc with Onshape's arrow marker,
  edge labels 12 px #555 on the path at 125 px. Default "Onshape Release" (Setup → Submit → Pending,
  Setup/Pending → Release → Released, Pending → Reject → Rejected) laid out like help image
  enterprise/custom-workflow-ex; obsoletion the same ending in Obsoleted. zoomToFit 95 %, wheel / drag /
  double-click zoom like d3-zoom, nodes draggable. Rendered as SVG through gpui's svg renderer. Esc /
  ✕ / backdrop click close only the lightbox (`release_dialog_modal` turns the modal's Esc off).
- REQUESTS "psb (10-19 h)": where Tab goes after the dialog's `?` (live: "body"); our dialog keeps a
  focus trap until answered.
- The diagram is rasterised with **resvg 0.46** (new direct dependency of the app, same version gpui
  uses; Cargo.lock gains one line) and its own fontdb holding the app's Roboto, because gpui's SVG
  renderer only has system fonts (serif fallback). `release_workflow::rasterize`.
- **Tab stops verified** in the app (Extrude: Depth → ± → Direction → Starting offset → Symmetric → Draft
  → Second end position → Final → ?). Focus looks from the bundle CSS (`tab_stop_element(.., Focus)`):
  checkbox box `#1651b0` border + 3 px `#769fcc` glow (`ui::os_checkbox_focused`, used by
  feature_dialog only), icon buttons a 2 px blue bottom line (inset shadow), `?` a ring.
- **⋯ menu** is deferred (anchored) so the table no longer paints over it.
- **Add Items › Other documents**: `Source::load` now lists Onshape's versions incl. the root
  "Start" (as asb's Insert `OtherDoc`), so a document without versions opens; `Source.changes` +
  `render_changes` = "N changes since the last version." / "Create a version in <doc>."; "No <type>
  available." for an empty version. A linked row's **Edit properties / Export** now work:
  `add_source::run_linked` opens that document in its window (`files::document_window`, new), views
  the version and runs the action two frames later (the new window's tab views exist by then). Rows
  from a version of the releasing document itself stay inert (can't switch this window's version
  under the open dialog). **Superseded in pass 9:** every linked row opens its version in a window of
  its own, so these work too.
- **Tool search**: "Did you mean:" row has 24 px below it, "No items match your search" is 42 px
  (live tool-search-export / -material); the Assembly search (asb's `assembly_search.rs`, small
  change) got the same chips (`tool_synonyms::ASSEMBLY`) and the no-match line.
- **Material picker** = ui-select: `material_rows` (None filtered too), `MaterialPanel.active`,
  hover / ↑ ↓ / Enter / Esc, bold matches (`StyledText` highlights), ✕ clears, list fits its rows;
  property values right-aligned except Name. Verified against assign-material-04/05.
- **Interference detection** (live analysis/80–84): field "Parts to analyze", read-only
  "Interferences" query list ("Part 1 and Part 2", tooltip "L x W x H", click = bold + zoom
  (`ui.interference_picked`)), no Scale slider, opens at the lower right (left = viewport − 347 −
  220, top 571; REQUESTS "psb (10-19 i)" asks whether that is a remembered position). Parts-list
  clicks now go to an open analysis tool (`pick_body` → `analysis_pick`). All analysis dialogs end
  with the 22 px `?` row. Analysis query fields restyled to Onshape's query list (24 px 13.33 px
  #555 rows with a visible ×, 12 px label).
- **Interference look** (verified vs analysis/81–83): picked parts get render state OUTLINE; with
  results `SceneOptions.neutral_bodies` (new) draws solids `#d1d6da` at alpha 0.2 (+ transparent);
  the red overlap `#c71010` is an `Instance::over_transparent()` mesh (new in render: drawn with
  `mesh_opaque` after the transparent list, so the see-through parts don't tint it); hover box
  `#f2ff80` 6 px. **Render changes (ui's renderer, small):** `DrawLists.over_transparent`, and
  outlines for see-through parts: `mesh_outline_depth` (depth-only, `fs_outline_depth`, OUTLINE
  states only) + `mesh_outline` over the transparent list, at the very end of the color pass.
- Build fixes outside my area while others were mid-edit (tell the lead): a stop-gap
  `ParamKind::Reference` arm in feature_dialog.rs (the fs agent then replaced it with theirs), and a
  missing `Sizable as _` import in fs's `fs_reference.rs`.

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

## Pass 9 (19–20 Oct, lead's list after v0.3.0)

- **Feature dialog keyboard = live "psb (10-19 h)"**: focus starts on the first selection list
  (`focus_first_field`); selection lists are Tab stops (`q:<path>`) and focusing one makes it the
  active field; selects are Tab stops (`OsSelect::focus_handle`, Onshape's `os-select-focusser`:
  Enter / Space / ↑ / ↓ open it, a typed character opens it searching for that character, closing
  gives the focus back; focused look = Bootstrap `.btn-default-focus` glow). The focus trap is gone:
  Tab / Shift+Tab are `DialogTab` / `DialogTabPrev` (actions.rs, "FeatureDialog" context) =
  `focus_next` / `focus_prev`, and leaving the dialog emits `DialogEvent::LeaveFocus`: forward →
  `FocusNavbar` (workspace: the navbar logo is now a Tab stop with a focus ring; Enter opens the
  documents page), backward → the Part Studio root (Onshape's <body>; `StudioTabPrev` keeps Shift+Tab
  there). Order verified earlier: … → Depth → ± → checkboxes → ?.
- **Interference detection = live "psb (10-19 i)"**: left = viewport W − 600, top = H − 401 (kept
  while open: `AnalysisTool.anchor_top`); a model change while open shows the 26 px Refresh row (↻,
  "Interference detection not accurate, instances have moved.", `AnalysisTool.stale`), the old
  result stays until it is clicked.
- **Release rows from a version of the releasing document**: Edit properties / Export work now.
  `files::open_version_window(path, microversion)` opens a second, read-only window on the same file
  at that version (not registered as the document's window); `add_source::run_linked` uses it for
  every linked row (other documents too, so their own open windows aren't switched to history); it
  gets the window's `Workspace` back and calls `export_item` (now pub(crate)) after 800 ms, since a
  root-view lookup in a fresh window fails. Verified: Export opens over the version window.
- **Visibility conditions** (ui's report: the box button is "Modify visibility conditions"):
  model `configuration::{Visibility, Condition, ConditionTest, OptionRule, ShownOptions,
  InputVisibility}` in `ConfigInput.visible_when` (same field name, so the many `visible_when: None`
  literals elsewhere still compile; `de_visibility` loads the old `(input, values)` form);
  `Configuration::visibility(values)` / `effective` (hidden input → default; hidden chosen option →
  default or the first shown one; a list with no options showing is hidden; options without a rule
  are hidden once rules exist) — `value_of` goes through it, so regen, store_edit and the release
  dialog follow; `repair_visibility` (called from `edit_config`, info bubble "Visibility conditions
  for X were updated"). Tests: `cargo test -p noshape-model --lib visibility_tests`.
  Dialog `config_visibility.rs` (bundle templates visibilityConditionsDialog / Statement /
  conditionBuilder + their CSS): title, ⋯ Expand/Collapse all inputs, the Inputs filter, ✕; the
  always-visible row; per input "Show <input> When [input] [is|is between] [values]" with And rows,
  Add condition, option rules "Show [options|range] … [When|Always] …", Add option rule, collapsed
  summaries in Onshape's wording, the "Some options do not have an option rule…" warning; preview
  with Show hidden inputs; ?, undo, redo, Save (valid + changed), Cancel. Opened from the Part
  Studio box button, the input ⋯ › Visibility conditions…, the input's visibility icon, and asb's
  assembly box button (small change in assembly_config.rs). The Part Studio box hides hidden inputs
  and options. Assemblies and the panel's ⋯ items: Pass 10.
- Clippy: `UiState.consumed_lines` now uses `ConsumedLines` (type_complexity).
- Build fixes in others' files while they were stuck: `Selectable as _` import in drawing_gdt.rs.

## Pass 10 (20 Oct, lead's round 3)

- **Part export options** (export.rs): `open_export_modal_with(base, assembly, parts_only, …)`;
  exporting parts (parts list, release rows) drops "Export unique parts as individual files" and
  "Include hidden instances in export", and one part is named "<Part Studio> - <Part>", as live.
  `open_export_modal` (tabs) is unchanged. Verified from the parts list, a workspace release row and a
  V1 release row (Add items › Current document › Version graph › V1 › Part 1; the row's Export opens
  the read-only version window, then the dialog). Live request psb (10-20 k): the navbar of a version
  window (ours says "History" + VIEWING HISTORY for every read-only view — workspace.rs:443).
- **Create version… in Add Items to Release** did nothing (an action dispatched from the dialog layer
  never reaches the Workspace): `release_add_source::create_version` now calls
  `history_panel::open_create_version` directly.
- **Configurations panel menus** (config_panel.rs + new input_propagation.rs): input ⋯ = Copy table,
  Copy input, Move up/down, Rename, Visibility conditions…, Input propagation…, Delete; panel ⋯ =
  Copy all inputs, Collapse/Expand all inputs; Paste inputs in the Configure Part Studio ▾ / Add
  configuration input ▾ / box ▾ menus while inputs are copied (`config_visibility::copy_inputs` /
  `copied_inputs`, an app-wide static: Onshape keeps them for the session; same ids are skipped).
  Copy table writes TSV (feature names row, "Name" + parameter labels, one row per option).
- **Derived from a configured Part Studio**: `DerivedSource.configuration`; the Derived dialog shows the
  source's List/Checkbox inputs (visibility-filtered) and stores them as `configuration:<id>` params
  (`importing::DERIVED_CONFIG_PREFIX`; ":" because the dialog reads "a.b" as array paths);
  `DerivedProvider::context_configured` (default = `context`) and `DocStudios::context_configured`
  regenerate the source with those values (cached per tab + values, same circular guard).
- **Input propagation…**: candidates are Derived features whose source has an input with the same id
  and kind (copied inputs keep their id); the dialog ("Input propagation for <studio>", References /
  Configuration checkboxes, "N new inputs to propagate", Confirm) adds a configured column
  `configuration:<id>` mapping each option to itself; its cells are option selects, its header
  "<Derived 1> / Configuration". Verified with Pin (Length Standard/Long) derived into Part Studio 1.
- **Visibility conditions in assemblies** (surgical, noted in assembly-behaviour.md): evaluation already
  went through `effective`; the assembly box, the Insert dialog's configured rows and the instance's
  Change configuration panel now skip hidden inputs/options. Test
  `assembly_configuration_respects_visibility`.
- **Search tools popover** (view_tools.rs): anchored at the window's right edge 10 px under the
  toolbar (`UiState.palette_top` probe) and deferred, so it draws over the feature list and right
  panels (it was clipped under the feature list while a right panel was open).
- Others' files fixed while they blocked the build: `Disableable as _` import in drawing_custom_table.rs.
- Not done: input ⋯ "Edit FeatureScript IDs…" and "Exclude from properties"; the live answers
  psb (10-20 j/k).
- **Measure (lead via skb, 20 Oct)**: `regen::measure::measure_shapes` holds the rules for both Part
  Studios and assemblies (`assembly::analysis::measure_entities` now calls it): single entities as
  before but Radius/Diameter first; pairs get the priority/angular row first like the client's
  `setMeasurements` (bundle: `GBTAngularMeasureDisplayData` rows and Diameter / Radius / Parallel
  (axis) distance move to the front; arc lengths to the back): planes → "Parallel dist" or "Perp planes
  angle", cylinder axes → "Parallel axes dist" / "Perp axes angle", lines → "Parallel dist" / "Perp lines
  angle", line + plane → "Perp line and plane angle"; then "Area" / "Length" totals, "Min dist" (was
  "Distance"), "Center dist" with a circle/sphere. Only the plane labels and Min dist are live-confirmed;
  request psb (10-20 m). skb's `measure_context` routing is untouched.
- **Mass properties**: `mass_properties` now moves each part's inertia to the joint centre
  (`combined_inertia`, parallel-axis term); test against two boxes by hand. The assembly version
  already did this.
- Feature dialog checkboxes: long labels wrap inside the 220 px dialog (label in a shrinkable div;
  rows keep 27 px min height). Live check requested (psb 10-20 l).
