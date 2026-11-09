# Handoff: psui (Part Studio UI), from 2026-10-26

Worktree /root/projects/noshape-wt/psui, branch agent/psui, target dir /root/target-psui, app display :102.
Scratch: /root/psui-scratch (check2.sh = clippy + covering tests + T0, detached; mkskip.sh builds a Skip-instances test
document with noshape-cli; ink.py measures plane-name ink boxes). Live: my own tab is the old ui tab
(`UI=1 node r.mjs …` in /root/live-pw, target id in /root/ui-scratch/live/.ui-tab); captures in onshape-research/live/psui/.
Run the app at live's size: `xdotool windowmove <w> 0 0 windowsize <w> 1919 1079` (planes then sit on live's pixels).
**Builds and tests run on vibe4 since 11-01 (user; docs/TESTS.md):** `tools/tiers/rb.sh cargo test -p <crate>`; for GUI
checks `CARGO_TARGET_DIR=/root/target-psui tools/tiers/rb.sh --get noshape cargo build -p noshape`, then
/root/psui-scratch/appc.sh runs it on :102. Never cargo on this machine. land.sh as before (it checks on vibe4).

## 2026-11-09 20:34 UTC: custom features on the toolbar (user P0), and the Part Studio speed re-measure

**User P0 (lead 11-09): a custom feature added from another document showed no sign it was added.** Placement is
live's (lead checked live at ~20:00, behaviours "lead (2026-11-09 ~20:00 UTC)"): no button of its own, ⊞ becomes
[⊞][▾], the ▾ lists the features then Add custom features. What makes it visible, now as live:
- the green toast "Custom feature "X" was added to your toolbar from version V1." (5 s, ×), "from published version
  V4" when the version was published with Publish FeatureScript… (`versions::is_published_fs`);
- in the panel, a feature on the toolbar and its Feature Studio's row stay light blue (`SelectItemPanel::set_added`), a
  second click takes it off (no second entry), the studio's row adds all its features (all on: all off; help);
- the list alphabetical, an account setting every open Part Studio follows (`MyCustomFeaturesRev`);
- right-click › Remove on the ▾'s rows too (Open linked document, Remove; plain ⊞ when none are left);
- "Linked custom features in this Part Studio" ▾ (live's `part-studio-linked-feature-tools`, before Custom features in
  this workspace): the studio's features from other documents not on the toolbar.
Tests: part-studio-ui `add_custom_features_toggles_adds_all_and_sorts`, `linked_custom_features_offer_a_used_feature_not_on_the_toolbar`;
checked on Xvfb (/root/psui-scratch/cf-s22c.png, cf-s33c.png). Not done: the ▾ row menu's other live items (Sort custom
features, Update…, Update all…, Customize toolbar…), and a toast per feature when a studio's row adds several (the
last one shows; live not checked).

**Speed re-measure under the feature-time rule (lead 11-09), vibe5, release main 9c5206e7, 87 corpus documents
(qa's saved set):** every scored open is under live (cf-surface-pattern 8.7 s, live 25.2); 67/87 first opens and 84/87
reopens show the model within 0.5 s. Found and fixed:
- **The bench's latency was the bench's own frame cap.** GPUI caps an unfocused window at ~30 fps for frames awaited
  with `on_next_frame`, which is how `ui-latency` finds its frame: unfocused (Xvfb without a window manager, the CI
  Macs) every sample waited 35-40 ms. Focused: row click 4.5 ms p50 / 9.8 p95 action to painted frame, dialog open
  12.8 / 26.5 (UI CPU 2 / 4-7 ms). The script now activates its window (macOS), bench.sh focuses it with xdotool on
  Linux (perf.yml installs it). Earlier "54 ms p95" numbers in this handoff were that cap.
- **A document's Feature Studios compiled on the UI thread before the window's first frame** (~50 ms on the cf-*
  documents): now in the background (`doc_model::compile_custom_features`, regenerations get `LazyScripts`, the toolbar
  reads `custom_features_if_compiled`). Regeneration starts at ~60 ms instead of 110-170; the model's frame is the
  same (the compile is in the regeneration now), reopens 40-80 ms sooner on three of four.
- **Not ours, a P0 for dx:** c2-fan-blade-sweep's Sweep 1 takes 331 s (live 58 ms) since 7e7d69f2b; docs/TICKETS.md
  FAN-BLADE-SWEEP-HANG.
Left in this area: reopens over 0.5 s on llvmpipe are its software draw of big models (cf-fill-pattern), not ours;
dialog open p95 26 ms (one frame over) not looked into yet.

## Now (2026-11-09 01:12 UTC): the Variable Studio and documents areas while studios is paused
- State in docs/handoff/studios.md (psui entries 11-08 22:42 on). Helper: /root/psui-scratch/bin/uipdiff.py <repo>
  <outdir> <scenario> prints every difference per step after run.py's normalisation.
- Per-tab undo can follow the tab an edit was made in (store `Entry::origin`, `Session::set_undo_origin`,
  `DocModel::edit_tab_in`): an inserted Variable Studio's table edited in a Part Studio's panel undoes there.

## From dx (2026-11-08 20:11 UTC, FYI): STL exports through `k::tessellate_export`; the closed-mesh repairs made fast
Lead 11-08: STL presets as Onshape's. The STL export (ops export.rs `mesh_params`) asks BRepMesh for 0.002 × the
dialog's mm (live's facetter takes its chord as about a thousandth) through `tessellate_export`: a copy 1000× up,
`mesh_prepare(exact)`, whose spheres and tori go on live's grid (mesh.cpp `export_grid` / `grid_face`, export only;
your display path is as before), then your three closed-mesh repairs. Those repairs (mesh_repair.rs) give the same
results but now hash with a multiply and only build their maps when a face is folded or an edge used twice: a
4M-triangle export's repairs 21.8 → 1.9 s. OBJ, 3MF and the others still use `tessellate_closed`.

## From studios (2026-11-08 20:29 UTC, lead's call): `var_complete` moved to app-core
The quantity autocomplete (`var_complete.rs`) is now `noshape_app_core::var_complete` (unchanged; part-studio-ui
reaches it as `crate::var_complete` through its app-core glob), so the Variable Studio's Value cell uses it too
(live vs-value-autocomplete). Your hooks (feature dialog, sketch dimensions, Variable table, Dynamic suppression)
are as they were.

## From studios (2026-11-08 20:04 UTC, lead's call): ☰ › Print… is studios' in the Part Studio
The Print setup panel, its dashed frame and Download image / Print live in app-core `print_setup.rs` (geometry in
`noshape_scene::print`, op `render_print`). The Part Studio only hooks it: `PartStudioView::open_print`, `on_print_action`, a `print`
field, the panel and frame drawn in `render_viewport_overlay`, and Esc (Cancel) closing the panel first. Please leave those hooks to
studios; ask if you need them moved.

## From core (2026-11-08 17:23 UTC, lead's P1 CANCEL-LATENCY): the worker's stop time, nothing for the UI to change
- Measured over the corpus (T2 ops parity::cancel_latency_probe: each saved parity document's Part Studios over
  0.3 s cancelled at 10-90 % of their run; vibe5 at load 5-9): cancel -> worker free (result dropped) was up to
  529-586 ms (cf-fill-pattern's sheet metal rebuild, cf-surface-pattern's tool grouping), 124-211 ms (cf-part-draft's
  Part Draft 3), ~100-133 ms (sheet metal flanges / corner breaks). Now every cancel is under ~100 ms, most
  under 50 (fill pattern <= 65, Surface Pattern 3 2-10, Part Draft 3 <= 56).
- Changed (core): the FeatureScript interpreter checks the cancel at every native call; the sheet metal rebuild
  (custom host) between its steps and per face / bend; surface_props between faces; the drafted prism's loops;
  op_boolean's tool grouping waits on a thread. Your side (66cf2194, 6826d5f1) needs nothing new.

## From core (2026-11-06 13:45 UTC, lead's item): the display meshing BRepMesh still does
**Landed (core 11-06 15:10):** extrusions of B-spline / Bezier curves read as exact B-splines (mesh_rust.cpp
bspline_of), and brep::mesh `mesh_ruled`: a B-spline straight across v whose rims are translates, one loop of two rims
and two straight sides, zipped rim to rim (no grid; the grid's 10:1 aspect cap from the smallest u step had put ~1000
rows along the flat way). And a moved instance's nodes on edges another face gives are pinned to that face's points
(RustFace::pins; a prism's moved bottom cap was 1e-18 off its extruded wall at z 0, not bit-welded). cf-drive-train-
generator: meshing 2.29 -> 0.41 s summed over its parts, triangles 556,572 -> 363,248; the corpus's BRepMesh 33.5 ->
14.7 s (12,428 -> 2,471 faces); parity unchanged. T1 kernel rust_mesh::an_extruded_spline_wall_meshes_here_as_a_strip.
What BRepMesh still gets (corpus, serial ms): B-spline 3467, plane 3012, revolution 2564, torus 1932, offset 1098.
**Then (core 11-06 16:00):** the grid meshes across an apex or pole (a degenerate edge: its nodes' u the edge's
parameters, kept apart, their two-node triangles dropped, the normal a hair inside the face; `degenerate_edges_run_along_u`),
the steps' angular rule ignores samples whose first derivative collapses there; surfaces of revolution of a planar
B-spline / Bezier / circle / ellipse curve (`MeshSurface::Revolution`, bridge kind 6; a conic converted, its nodes
then projected). knob-jason's, cf-setback-fillet's and clamp-guillem's 91 such faces and knob-kraut's revolutions
meshed here; the corpus's BRepMesh 14.7 -> 13.1 s (2,471 -> 1,920 faces). Still BRepMesh's: faces with ellipse edges
(cf-o-ring-generator's rings, c2-valve-config, c2-housing-draft): brep::mesh has no ellipse edge. The normals check in
rust_mesh compares with BRepMesh only where BRepMesh matches the surface (its sphere pole normal is turned round).
The lead asked core to find which surface types make up BRepMesh's ~10 s of the corpus's ~16 s of display meshing
(the faces brep::mesh declines or isn't given), biggest first, and to take the biggest into brep::mesh. brep::mesh is
yours: core will measure first (a per-surface-type table of BRepMesh's time over the parity corpus, in core.md), then
work in brep::mesh's dispatch / grid mesher with small, separately landed changes, results checked against BRepMesh's
area as the hose work did. Tell core (handoff or the lead) if you have work in flight there or would rather take a
surface type yourself.

## From dx (2026-11-05 03:44 UTC): cf-hose-routing's display takes ~6 s and ~0.9 GB
**Done (psui 11-05 11:57, lead's request):** the hoses' walls (B-splines closed round u with a seam) were all declined
by our mesher and went to BRepMesh (2.2-6.4 s a hose). Three causes in brep::mesh `mesh_grid`: the loop walked the seam
at one end of the u range and its rim on towards the other (each ring now unwrapped by continuity, then shifted into
the range); the rim's first nodes from the seam's corner were all found on the seam (a clamped iteration: retried from
just inside); the seam's B-spline edge split near a slight crease into nodes nanometres apart (boundary nodes closer
than the triangulation resolves stand for one, the least point's, so both seam uses agree); and the grid's box
widened by the seam nodes' overshoot gave 30000-row grids (clamped to the range). feature_list reopen 7.2-9.2 s /
0.9 GB -> 1.27-1.30 s / 318 MB (vibe5); CLI render the same to 2 pixels over 10 %. T1 kernel
rust_mesh::a_hose_swept_along_a_spline_meshes_here_within_brepmeshs_area (T2 its hollow twin).

The parity corpus's cf-hose-routing (four hoses: a circle swept along fit splines, then shelled; B-spline tubes)
regenerates in ~0.7 s, but `feature_list` (which builds the display) takes 7.05 s and peaks at 930 MB, and the parity
runner's reopen-and-list is 7.8 s against live's 0.49 s regeneration; `document_info` alone is instant. So nearly all
of it is the hoses' display meshes. Repro: `NOSHAPE_PARITY_ONLY=cf-hose-routing NOSHAPE_PARITY_SAVE=<dir>` parity_models
keeps the translated document, then `noshape-cli call feature_list '{"doc":"<dir>/cf-hose-routing.noshape"}'`.

## From skasm (2026-11-04 14:21 UTC)
- **MC-INFERENCE-DOTS** (P3, docs/TICKETS.md): the assembly's mate connector hover now draws your `InferenceMarks`
  (as the Part Studio). Against live 1851 shot 4 (psui/hole-mc-1851/4-hover-face.png) the connector glyph matches
  (~35 px), but the dots are smaller than live's: live's corner dots are ~10-12 px white discs drawn flat in the face's
  plane, its edge-midpoint marks ~8 px; ours are 7 px round dots with a grey rim. Yours to change in
  `scene::builder::InferenceMarks` (both apps take it).
- **Win / Super key** (lead 11-04): `app-core::quantity_field::step_ctrl(&Modifiers)` = Control, or Cmd on macOS only
  (provisional until live's Mac key list). The quantity field and the sketch's dimension stepping use it now;
  `feature_dialog.rs:3909` still takes `e.modifiers.control || e.modifiers.platform` (on Windows / Linux the Windows
  / Super key steps by Ctrl's amount there): yours.

## Mesher (user 11-01 18:30: before the rest of the audit): NIST and the 45/45 knurl
Baselines (release regen_profile with the document's Feature Studios, NOSHAPE_KERNEL_PROFILE=1; docs in the
worktree's untracked .psui-prof/: nist-mtc-crada-assembly.noshape, knurl-45.noshape = core-scratch/profdocs/
knurl-diamond with toothHelixAngle 45 deg): NIST 9.86 s (BRepMesh 9.1 s summed, 104 s summed waits on the mesh lock:
the parts one at a time); knurl 13.7 s, the feature 12.7 s, the display mesh ~1 s (note in bool.md). Work: mesh.cpp
runs BRepMesh on a topology copy of the faces it meshes with the lock released (cache entries copied out first, the
set re-fetched after; NOSHAPE_MESH_LOCKED=1 the old way for A/B), at most NOSHAPE_MESH_CONCURRENT parts at once
(default: no limit, measured best). NIST regeneration on idle vibe5: locked 3.19-3.20 s -> 0.61-0.64 s (loaded vibe4
8.5 -> 2.4 s). Landed 57f2f512.

B-spline faces in brep::mesh (11-01 21:30, this commit; NOSHAPE_MESH_BSPLINE=0 BRepMesh's as before): mesh_rust.cpp
flattens B-spline curves and surfaces (rust_boolean.cpp's push_nurbs layout, trims unwrapped) into `brep_mesh`'s
`nurbs`; mod.rs `mesh_grid` (generic `GridSurface`: B-splines, and cylinders / cones / tori / spheres a strip
declines, e.g. a knurl's lands bounded by helices, with seams) puts the boundary on the parameter plane
(`param_of` from the last node's), steps each way by the curvature (|S''|, |S'|; the deflections) spread over the
knot spans, no step over 10x the other way's smallest, interior grid nodes inside (even-odd per row, a cell hash for
the boundary distance), cdt::triangulate_with; normals Su x Sv. B-spline edges: knot breaks, halving, then merged
greedily where the deflections allow. Faces with an edge not split here are declined before any is meshed, and the
B-spline faces mesh after the others (settled), so a part's thread isn't meshed for nothing. Our mesher's own work
runs with the mesh lock released too (rust_mesh_faces `unlocked`). Results (release regen_profile, vibe5 loaded
~5-8 by others, same runs A/B):
- knurl 45/45: display mesh 0.88-1.35 s (BRepMesh) -> 0.22 s, our mesher takes all 17762 faces; 64k -> 50k triangles.
- NIST: 1.09-1.14 s -> 0.67-0.70 s regeneration; fasteners 26 of 32 faces ours (was 13); 909k -> 1.11M triangles
  (B-spline edges split ~18% finer than BRepMesh's GCPnts; tori 192k -> 301k).
- Accuracy: every grid face within the deflections (worst normal turn 0.53 of the angle; NIST's linear deflection is
  19 mm, so the angle rules). CLI renders (render image, 1400x1000 and NIST front 4096x3072) against BRepMesh's: no
  pixel more than 10% apart on the knurl, 478 of 12.6M on NIST (the threads' tangent-edge dashes).
T1: brep mesh::tests a_bspline_face_is_meshed_within_the_deflection (a dome), a_cylinder_piece_under_a_slanted_cut_*,
a_whole_cylinder_under_a_slanted_cut_*; kernel rust_mesh::a_lofts_bspline_sides_mesh_here_within_brepmeshs_area.
Left: B-spline faces with a seam (periodic B-splines) decline; B-spline edge split could follow GCPnts closer (fewer
triangles); SurfaceOfLinearExtrusion / revolution faces stay BRepMesh's.

## From core (11-01 17:00): the NIST import's display mesh is BRepMesh, one part at a time
Regeneration profile of the perf documents (core, NOSHAPE_KERNEL_PROFILE, release, vibe4; logs
~/agents/logs/prof-*.txt there): opening tests/perf/nist-mtc-crada-assembly.noshape spends ~6.3 s of its 6.5 s in
`mesh_prepare` for the 36 parts: `mesh.brepmesh` 6.1 s (OCCT's BRepMesh: the imported B-spline faces; the own mesher
took 22 ms, `mesh.rust`), and `mesh.maps` 78.7 s summed over the 36 part threads, i.e. each part waits on
`mesh_mutex()` for the others: the parts mesh one at a time. The two levers (yours): the lock (BRepMesh per shape is
re-entrant once the shape's own maps aren't shared?), and the own mesher for B-spline faces. Every other perf
document's display mesh is under 0.5 s.

## From dx (10-30, lead's decision)

- The Shift+/ Keyboard shortcuts sheet moved to app-core (`crates/app-core/src/shortcut_sheet.rs`, `ShortcutSheet`,
  registered hooks for the key chip / Preferences links from crates/app main.rs) so the Part Studio, assemblies and
  drawings share one copy. In the Part Studio: `show_shortcuts` and `ui.shortcut_*` are gone, replaced by
  `shortcut_sheet: Option<(Entity<ShortcutSheet>, Subscription)>`; `toggle_shortcut_sheet` (view_tools.rs) opens it on
  General as before; part_studio_hooks lost `show_keyboard_shortcuts`, `show_shortcuts_page` and `key_chip` (only the
  sheet used them); the test `shortcut_sheet_lists_every_part_studio_key` moved to app-core. Behaviour unchanged
  (checked on :147: tabs, Shift+/ and Esc close, click outside closes). Further sheet changes go in app-core.

## Landed on main

- 11-08 15:30 MESH-GRID-LOCAL (lead: brep::mesh's grid step): a B-spline face's interior nodes are the corners of cells cut
  as fine as the surface bends there (`mesh_grid_why`): a raster over the parameter box (4 cells a knot span, ≤512 a
  way, ≤262k) holds the steps each way from the bending across the surface (S''·N, not |S''|: a draft's flat wall took
  3,792 triangles for BRepMesh's 32), the twist and the normal's measured turn between raster corners (a checkerboard
  tube), a cell taking its neighbours' too, graded (+0.5 per unit distance); cells split by it breadth-first
  (≤400k; over, every step scaled up evenly). Triangulated on the plane stretched so a typical cell is square (a long
  bend: slivers along its flat length as BRepMesh's; tall cells fanned a boundary node across several), cells within
  10x of that shape; corners snapped (two cuts' corner a hair apart failed the CDT). A normal where Su×Sv all but
  vanishes (a blade's tip) is unknown (its noise asked 19k cells). Corpus (fresh copies; BRepMesh reuses a finer
  triangulation already on a shape, which skewed the first counts): ours/BRepMesh 1.17x → 0.68x display, 1.41x →
  0.68x Medium; faces over 2x 239 → 7, worst 1,353x (cf-spur-gear face 207) → 4.3x (c2-hunting-knife's blade at
  display, where BRepMesh's 1,291 miss the angle). Guard: T2 `planar_face_mesh::corpus_triangle_counts_against_brepmesh`
  (no face ≥5x); export meshes all closed.
- 11-08 15:20 EXPORT-MESH-FOLD (lead P1, dx's exchange suite): every export mesh (STL, OBJ, 3MF…) goes through
  `k::tessellate_closed`: the raw mesh without the display's `tidy_triangles` (it turned slivers by their corners'
  normals against their neighbours and dropped area-less ones, opening edges: c2-hunting-knife, cf-setback-fillet,
  learning-center-michael, pipeflange-steven), a pole's pinched triangles dropped, folded faces re-triangulated
  (`repair_folded_faces`: cf-sheet-metal-model-plus Part 3 face 48, BRepMesh's two triangles turned over on a 14×1
  B-spline strip) and triangles wound against their neighbours turned (`orient_by_neighbours`). Guard: T2
  `planar_face_mesh::corpus_export_meshes_are_closed_and_oriented` (340 corpus solids × Coarse / Medium / Fine, each
  edge once each way: green 11-08); T1 `mesh_repair::`. Not ours: 7 bodies regenerate as invalid B-reps with free
  edges (cf-surface-pattern Parts 1-6, c2-impeller-loft Part 1: no closed mesh possible; for core). Open: our grid
  mesher makes ~500k triangles on one B-spline face at export Medium (c2-hunting-knife faces 17 / 37, cf-setback-fillet
  face 5; BRepMesh 1270): one tight spot (the blade edge) sets the whole tensor grid's step, then the 400k cap thins it.
  → fixed 11-08 15:30 UTC (MESH-GRID-LOCAL above).

- 11-05 22:25 queued ABOUT-DISABLED: Help › About / macOS "About noshape" open the About modal (app/src/about.rs; text in
  ops/src/about.rs, op app_about). The notices file crates/ops/assets/THIRD-PARTY-NOTICES.txt is generated:
  `UPDATE_NOTICES=1 cargo test -p noshape-ops --test all notices::` after any Cargo.lock change (its test fails until then; a
  package with no licence file of its own needs its licence's standard text in ops/assets/licenses).
- 11-04 22:30 queued 1dc789e7 + 1d927467: a dialog's preview / close build the scene off the UI thread
  (`apply_display_async`, scene caches behind a mutex, `display_seq` / `scene_gen` guards; UI-thread apply per dialog
  open 57-72 → 1 + 14 ms); a drawing's Ctrl+S / Ctrl+M take the Control key only (drawing_keys::control_key).
- 11-04 21:55 landed: shared shortcuts per OS (actions::ctrl_cmd: Ctrl and Cmd on macOS, Ctrl elsewhere; Redo Ctrl+Y
  + Shift+Cmd+Z on macOS only); UI test binaries get their own settings dir (ui-test-support ctor; crates name it).
- 11-04 20:48 a8c24d3b **P0 fix**: since d2eccf9f (18:16) the cached feature/part rows collapsed in any list taller
  than its panel (cf-straight-spline: all 59 rows drawn on one line): a cached view's content doesn't hold its size,
  so flex-shrink squeezed them to 0. `flex_none` on the rows' cached style; T1 cached_rows_stack_in_a_long_list (120
  features; the small T1 documents never overflowed, which is how it got through).
- 11-04 21:00 queued 3580968e: 27922298 scene pick targets cached per body mesh (s.build p50 22 → 14 ms on
  cf-straight-spline); ef6fbdfb dialog open/close redraws 1 list row, not 73 (vendor gpui-pre: `Window::focus`
  marks only the old/new focus paths dirty instead of `refresh()`; a cached view nested in a reused one carries its
  frame indices along; row keys from what the row shows, not the display pointer). Dialog open UI CPU p50 46/43 →
  26/27 ms, frame 51/40 → 20/21 ms.

- 10-27 00:15 (cdde1c06..8ef8c97b): e6b2a956 dialog audit model side; 396eea65 plane names (within 1 px of live), hollow
  points' thick ring, renderer preview mix (`Scene::preview_mix`, not used by the app yet); c7f26470 dialog audit app side
  (✓ rule, Skip instances field + points, FOCUS_ON_VISIBLE, active-field-only highlight, drag bench); 8ef8c97b offscreen
  preview-mix test. Checked: clippy (7 crates), covering tests, T0.
- 10-27 01:00 (dc3a03f6..606d2d0f): 3b089573 DCLICK-HEAVY (X11/Wayland double clicks timed by event times);
  6db9bb8d DELETE-TAB-DIALOG as live; 606d2d0f Tab commit schedules one preview, TAB-NO-PREVIEW closed (Xvfb artifact).
- 10-27 01:20–02:10: 8bf820e0 feature previews as live's preview blend; 72e41465 LOFT-PROFILE-LIST; 5e4c8ad5
  interaction 3.2/3.3 and p1-dialogs closures; PANEL-BLEED (analysis field empty over a dialog, Esc order) in check7.
- 10-27 02:10–03:30: 67bca14a PANEL-BLEED (analysis field empty over a dialog, Esc order); ac78dfed Section view's
  "Select default plane" popup; fef99ffe highlighted edges as live's light set (face boundary 3 px, edge hover 8 px
  with dark outlines, selected 5 px); 5912b4c1 Mass Part tab takes the whole part; 8165fca6 selected-part shading
  refit; then PERF-SECTION-PANIC + MENU-SKETCH-TAG (check12).
- End-of-day walk 10-27 03:10 on /root/green/noshape (16cdcf6d): live-30 edit Extrude 1 (Enter previews, ✓ regens;
  15 mm swallows the bosses so their chamfers/fillets fail, as the geometry says), undo, rollback drag up and back,
  Suppress from the row menu (dependants red), the Diameter readout, Section view, Mass properties: no regressions
  in my area. Live asked about the feature list header with errors ("Features ..." truncates next to the red icon).
- 10-27 03:30–04:15: PERF-SECTION-PANIC + MENU-SKETCH-TAG; CHIP-HOVER (regions); DEPTH-RANGE; tickets closed as not
  bugs or already working: FOLDER-NAME-INLINE, EXT-DIRECTION, P1-10. psui's open tickets now: P1-8 caveat (Hole chips).
- 10-27 05:30 54ffeab7: paused regeneration as live (6.25), feature list filter with default rows / parts / "..."
  runs (6.10 / 6.11, model-core `filter_row`), Shift+wheel ±10 (5.22), the active tool clicked again keeps its
  dialog (4.4); Roboto Bold Italic. Checked: clippy (4 crates), interaction / quantity_field / ops list tests, T0.
  Verified against live on :102 also: 1.12, 6.6, 7.4, 9.2, 9.6, 6.28.
- 10-27 05:40–06:25: bac382a7 highlight from Onshape's orthographic eye (offscreen test against live), box select
  without part vertices, "Recently used" tool search, parts-list related rows, Shift+I clears the selection;
  bd5ce252 Dihedral panel as live; b6f189d2 no pre-highlight during a box drag (the stray midpoint ring); 5b54b73d a
  hovered selected edge as live; c0bbde51 REGEN-NO-CANCEL (P0, model-core regen::runs: a newer run needing fewer
  features aborts the running one); a9f68288 LOFT-SKETCH-HIDE (every sketch a feature uses is hidden, also on edit).
- 10-27 06:30–06:55: c93f34b4 PS-SECTION-RED (red overlap caps only in assemblies, `Scene::section_interference`);
  79460041 graphics context menu modes as live (8.8 / 8.10: exits first, Confirm / Cancel, Turn off section view;
  second Shift+X keeps the Section view dialog); d97fe394 Esc on the menu also ends an isolate; 793ad99d Make
  transparent at live's 29 %; 57b344ac part-row hover no longer drops going row to row. Verified on :102 too: 2.19,
  3.17, 7.3, 9.3–9.5, interaction 5.5.
- 10e24f53 the feature list's error icon as live (6.12). perf.yml dispatched 10-27 07:02 on main: run 40464678503
  (read its summaries into viewport-performance.md).
- 10-27 07:10–07:50: 39757d3b preview highlights whole over the mix (compare::POST pass); 028f1f2d face menu Hide
  other / all parts; 46aaad69 view cube corner hit; 4e55b01f PREVIEW-NO-PICK (P1 regression of 8bf820e0, the
  preview's pick pass). Verified on :102: 1.7, 3.10, 4.3, 4.5, 5.9, 5.16, 5.21, 5.31, 7.2, 9.9, 9.10, 9.12, 10.2–10.5,
  11.2, 11.3, interaction 4.7 / 6.7.
- 10-27 07:55–08:50: Mate connector opens with the owner empty (1.23, live); box border 1.5 px as live; the
  Extrude preview's end cap and the region tint over the mix (4f768e34); Dihedral comb length (exponential Scale)
  and Max / Min halo labels (99fcfa1b). Verified: 5.16, 6.8, 9.15 (U), navigation 1.19 / 4.5 / 5.2 / 5.3 / 5.6 (U).
- 10-27 10:25 f24ef0f8: **noshape-part-studio-ui split** out of the app (40k lines; the app is 30k), app-core gained
  shortcuts / material_libraries / mass formatting / StaleContexts, the rest are `part_studio_hooks` (docs/CRATES.md
  Progress, with build times: one-line Part Studio edit → app 15–19 s, `cargo check -p noshape-part-studio-ui` 3.7 s).
  Checked: workspace clippy, hakari, full T1, T0, ops coverage, the release smoke on :102. The Part Studio's files
  are now under crates/part-studio-ui/src (part_studio/, feature_dialog.rs, …).
- 10-27 10:50: LP-DIRECTION-PICK (P1, user): custom features' compound edge filters (`EDGE && ALLOWS_AXIS`,
  `EDGE && GeometryType.LINE`) take the part's edges (custom/studio.rs `filter_of`); offscreen test that a preview
  picks edges too. Rule from the lead: a preview / pick change lands only with a T1 test that a dialog field picks a
  face AND an edge.
- 10-27 11:20 f65e888f: the parked WIP, verified on :102 against live's "psui preview look": Delete part outlines the
  removed part's ghost (254,191,76), silhouette only, full colour at any slider position; a blended Add's "with"
  model is the result in the part's colour (boss over the block top at 0.7: ours (96,129,147) / (130,172,197), live
  (104,131,148) / (129,164,186): the 0.7 mix of the part shade over the block, as live).
- 10-27 11:50: part-studio.md 3.7 (a box into a field that takes edges puts in only edges), 3.12 / 3.13 (Select
  other lists only what the active field accepts; the entity menu with a dialog open in live's order), 6.26 (each
  list down to its 30 px header), 6.37 / 6.38 checked as live. Verified on :102.
- 10-27 12:20: MEASURE-MAX-DIST (P1): Max dist refined on the exact geometry (model-core measure `refine_max`).
- 10-27 13:10: EMPTY-COMMIT (P2: ✓ / Enter / Ctrl+Enter only once the dialog can be submitted) and CONFIG-CELL-EDIT
  (P2: configured value cells frame on a click and take typing, as live).
- 10-27 13:40: 69de3163 EDGE-HIGHLIGHT-LOOK (2), the user's black wedge: the image-based silhouette pass could throw
  a back face across the screen (a push divided by a finite step's on-screen length); now analytic, offscreen test
  a_thin_plate_draws_nothing_past_its_outline (1× / 2×). Not reproducible on Linux, so the most likely cause, not a
  confirmed one. 11e7e6aa FS-EDITOR-COMPLETE (1): Ctrl+Space stays in the Feature Studio's code editor.
- 10-27 15:40 89c4d147: EDGE-HIGHLIGHT-LOOK (P1, user) as live's capture: 5 CSS px band, opaque rim, 78 % core, no
  dark outline; hovered + selected = selected; face boundaries 2 device px; plain edges 1 CSS px; lines don't write
  depth (the AO pass shaded the band's lifted depth). Test scene edge_looks. With it a913de0e REGEN-NO-CANCEL (2):
  the 84 s stale run was the document thumbnail, now aborted when the view regenerates. Also landed today:
  8cdd189f PERF-TOTAL-MISMATCH, 8d9246ab CONFIG-INPUT-ID, 7d371f34 OK-DISABLED-HOVER + CONFIG-CELL-EDIT-LOOK.
- 10-27 16:10: GPU-TEST-SEGV (P1, lead): GPU tests crashed in a driver's exit handler after it was unloaded (each
  Renderer::new loaded and dropped its own Vulkan instance + device); one process-wide device now (renderer
  `shared_device`), 90 / 90 clean runs where the old binary died once in 27.
- 10-27 14:10–15:10 (new psui session, taking over from the one moved off its account): ab488e30 perf.yml speed
  targets (the previous session's); 9d216d54 EDGE-ON-PLANE-PICK (P1: a datum plane's border picks as its fill, first
  and without depth writes, `Instance::pick_behind`; offscreen test); d6e49c94 SPACE-CHECKBOX (P1: GPUI's keyboard
  click already presses a focused element with a click handler, the tab stops' own key handler toggled twice);
  9dd786cc PART-HOVER-RED (`accent` never takes the error red; scene test) + PARTS-SHIFT-RANGE; d96076f9
  EXPR-ENTER-COMMITS. BOX-FILLET-FACES is a preselection (box drawn before the dialog opened), DIALOG-OK-STATE has
  bundle notes: both wait for live (REQUESTS psui 10-27). perf.yml on main dispatched 15:01: 40526871683 (bench),
  40526882894 (bench, COLD=1): read their "Speed targets" lines into the table below.
- 10-27 15:10–17:00: 6bff20a7 section value box Esc (interaction 6.3 / 6.4 verified); 5.1 / 5.3 mass properties
  verified against live's numbers; 6016d717 measure: Curvature deviation only where an edge meets the face (4.1 / 4.2
  verified on live's "psb measure" part, rebuilt by /root/psui-scratch/meas/mk*.py); 1c853f78 SHORTCUT-MAP-FOCUS;
  bf088793 REORDER-MISSING-MSG (model-core: a part not yet made is a missing selection); 85a2cb76 BOX-FILLET-FACES
  (planar-face fields take no cylinder, `face_fits`) + DIALOG-OK-STATE (range errors disable ✓, "Syntax error in
  expression."); dea57d5c TAB-EXPORT-MISSING; 2044128a HOLE-SIZE-NO-DIAMETER (the size table applies with std logic
  too; overridden info for unsynced holes); b0a4cab0 VAR-BARE-UNIT + SKETCH-RMB-ITEMS (curve).
- 10-27 17:00–18:20: 08673b8b section overlap flat red in Part Studios (live re-capture) + part hover = its outline
  in hover orange; 1a5a255a PLANE-MENU-SELECTED; 15f1128b SKETCH-S-EXTRUDE; 595549a3 LOFT-PROFILES-PINK (array
  field error look; any set_value makes the dialog dirty); a80013c6 `NOSHAPE_PREWARM=early` switch (off).
- 10-27 18:20–20:00: 8602df0d mate connector disc as Onshape's constants (interaction 3.4 ⏳); df28c320
  PREVIEW-SELF-PICK (`made_by`) + a selected connector's owner tinted yellow-green; MENU-TRUNCATE not a bug (live:
  250 px, "…"); 467c4f21 / dc395f46 indexed line quads measured and left opt-in (`NOSHAPE_PERF_LINES=indexed`).
- 10-27 20:00–22:00: 921bb066 P1-8 caveat (Hole `groupedSketches`); 5840897a a click into a field checks the face's
  surface like hover (Revolve's axis took a planar face); part-studio.md 1.24 / 3.18 / 5.6 verified. Main went red
  20:34 (lead's -Dfar= vs core's `far`): fixed by the lead (bd0948b5) while mine was landing.
- 10-27 21:40–23:30 (new psui session on a fresh account): ecc38c5a BOX-DIALOG-PANIC (P0 release blocker,
  regression of df28c320: `box_select` read the dialog inside its own update; T1 GPUI test
  box_select_with_a_dialog_open, which panics without the fix), SECTION-DIALOG-STACK (starting a feature ends the
  Section view dialog as with ✓, T1 test), TABS-PANEL-FOCUS (the Tab manager doesn't take the focus). bf131a31 the
  part hover look (0d below) as live: render hover_outline.rs. Then interaction 6.2 (Section view manipulator hover
  and rotate glyph as live, T1 test). Experiment branch psui-perf-pulled (lines pulled from a storage buffer,
  `NOSHAPE_PERF_LINES=pulled`, pixel-identical offscreen) measured by perf.yml 40599691793 (variants r1/p1/r2/p2 at
  2x): land it opt-in or as default only if p1/p2 beat r1/r2. Perf 40590825262 (main 27e0f950, Retina orbit):
  macOS 14 base 64.6 / 73.1 fps, no lines 84.4; macOS 15 base 52.0 / 61.3, no lines 96.4.
- 10-27 23:40– (psui session on another account): MISSING-TOPO (P1 feature list errors, found walking live's "(7)
  Dragging feature rows"): a face / edge / vertex of a feature later in the list or consumed before is a missing
  selection (model-core `drop_missing_regions` with `topo`, sketches excluded; `Body::has_exact` first), so a Fillet
  dragged above its Extrude shows the ⚠ triangle and "1 missing selection"; feature_list gives `missing_selections`.
  part-studio.md 5.4b verified (✅), 6.16 note updated. perf.yml 40599691793 (psui-perf-pulled, 2x orbit): macOS 14
  p1 72.8 / p2 67.6 against r1 69.0 / r2 69.8 fps, macOS 15 p 50.3 / 60.6 against r 57.7 / 44.8 (noise ±15): pulled
  lines gain nothing measurable, the branch stays unlanded. Live asked: 2.23 Alt+click, a sketch on a face that is gone.
- 10-28 00:30–01:40: b3bb479b Isolate "Expand: distance" as Onshape's isolate manager (bundle rule in behaviours
  "psui (2026-10-28) Isolate"), part-studio 10.6 / 5.8 verified; FS-EDITOR-COMPLETE (P2): feature-studio-ui
  fs_complete.rs (std index from the doc comments, hump matching, Ace ranking, call snippets) + the vendored completion
  menu's `ace` look (gpui-component completion_menu.rs, caret_anchored.rs `shift_x`; gpui-base: a non-trigger
  keystroke closes the list). Unknowns asked in REQUESTS (psui 10-28): snippets of non-op functions, `;`, placeholders.
- 10-28 01:40–03:20: 7929b13a sketch on a lost face ("Select a sketch plane." + missing, live), named views sorted;
  e3fd499b see-through parts pick nearest-first, Alt through (2.23 ✅); then completion placeholders (vendored
  gpui-base snippet session: `${n:…}` selected in turn by Tab, caret after `;`; every call ends in `;`, predicates
  "predicate") and CTRL-C-PART.
- 10-28 03:20–04:10: 4658e826 completion placeholders + CTRL-C-PART; 4fa9cdf1 no row tooltips while dragging
  (REORDER-MISSING-MSG closed). End-of-day walk 03:35 on green da18f886 (live-30: edit Extrude 1 rolls back with the
  preview and arrow, Esc; Front › Shift+X opens Section view with "Section plane 1 (Front)" and cuts; Shift+1/7 views;
  Alt-pick, completion, lost sketch plane checked on my builds): no regressions in my area. One unreproduced oddity: a
  drag of Boss 2 once moved Boss sketch 2 with it (three retries moved Boss 2 alone).
- perf.yml on main dispatched 10-26 23:45: 40421862773 (live-30 edit/undo/redo), 40421870988 (fins-400 orbit 2x frames),
  40421880164 (live-30 cold start). Read the summaries into viewport-performance.md.

## Speed targets (PLAN 2.7), current numbers

| target | goal | current (where measured) |
|---|---|---|
| orbit / pan / zoom, NIST assembly at Retina | 120 fps | 10-30 (perf 40987999836, main 7523035a, silhouette cull on, 2× frames-driven): presented 79–85 fps, GPU frame mean 12.8–15 ms (before the cull: 48.7–55.7 fps, perf 40973793607). Earlier: v0.4.11 (perf.yml 40528877198, macOS 14, 2388×1588 device px, frame-driven): presented 76.7 / 73.6 / 64.9 fps, UI CPU p95 0.35 / 0.33 / 0.37 ms; GPU time not reported on Metal (r.done_ms lands with ab488e30, after v0.4.11). At 1× the runner presents ~30 fps (its timer) |
| cold start to first frame | ≤ 1 s | 10-28 (perf 40755981437, Metal cache cleared each launch, precompiled metallibs): median 1364 ms (macOS 14) / 1560 ms (macOS 15), shader compile 65–141 ms (was 540–900); the window itself is the rest. Earlier: | v0.4.11: the job's first launch 1356 ms (OS first run: gpui_kit::init 285 ms vs 20 warm; the held shader compile 421 ms starts at the first window draw, 784 ms); warm launches 276–385 ms ✓. COLD=1 run 40526882894 (Metal shader cache cleared each launch) pending |
| hover, pick (pointer / click to the frame showing it) | within a frame | 11-04: viewport benches `NOSHAPE_PERF_BENCH=hover` (new) / `pick`; their summary splits the UI thread's part (to the frame's submission) from the GPU's. cf-straight-spline (3662 faces, 230k triangles, model suite, local Xvfb): hover's pick 1.5 ms p50, scene refresh 1.4 ms; the GPU part is lavapipe's here (~1.1 s), only perf.yml's runners say |
| dialog open, list row click (action to the end of the paint of its first frame, UI thread) | within a frame | **Met. 11-06 on an idle host (vibe5, load 1-8, xvfb-run, release main 3be27d0f): cf-straight-spline dialog open UI-thread CPU (action + its frame) p50 4.9-5.0 ms, p95 5.2-6.1 (action 0.8, frame ~5: prepaint 2-5.5); the first open of each feature type is no outlier (sample 0: frame 4.9 ms); live-30-features p50 4.0-4.5, p95 5.4-5.7, row click p95 1.9. The ~35 ms until the frame starts there is llvmpipe drawing and presenting GPUI's window (frames ~30 fps); a real GPU starts it within a refresh. Gated now: perf.yml's macOS jobs run ui-latency on live-30-features (bench.sh LATENCY_DOC), `--gate-latency-cpu-p95 16.7` and the same-job comparison with the previous release. The numbers below (13-22 ms) were the loaded local machine (load 10-29).** 11-06 (main 5b14f339, cf-straight-spline, :102 release, interleaved A/B, load 10-29): dialog open UI-thread CPU (action + its frame) p50 12.6-13.2 ms (was 14.1-14.6 before 5b14f339; 26-27 on 11-04), p95 15-22 (the first open of a feature type and the machine's load); its frame's layout+prepaint+paint p50 ~10 ms (prepaint 4.3-9 ms, was ~13); row click CPU p50 6.4 ms. Met at p50; p95 over by up to ~5 ms. Per sample: `BENCH SAMPLE` lines. Left on the UI thread around an open: the preview's `ps.apply_display_end` 3.5-12 ms before its first frame (refresh_scene: `with_highlights` and the consumed picks' edges, the pick priority built twice), GPUI's layout/prepaint/paint of the window. Waiting for the frame (~30 ms here) is this machine's (lavapipe draws on the same CPUs). 5b14f339: a replaced display is freed off the UI thread (it was 8 % of the UI thread while dialogs opened and closed).** Earlier 11-04 21:00 (ef6fbdfb): CPU p50 26-27 ms (was 43-46). |
| pins-assembly-1000 on perf.yml's macOS runners (real GPU) | 120 fps / a frame | 11-04 run 41840797973 (main 60f4813f, fixture pins-assembly-1000, variant `hov@hover:`): GPU 8.7 ms a frame mean, 13 ms p95 (over 8.3 ms: the renderer's next item); presented 33 fps (the runners cap a 1x window near 30); hover p50 23-32 ms (UI 10-15 ms), pick p50 106-113 ms (UI 11-15 ms; the rest the runner's pacing) |
| open a document | ≤ 0.5 s | **11-06 23:19, with the display cache (vibe5, release, Xvfb + llvmpipe, median of 3; table "Document open with the display cache" below): a reopen shows the model within 0.5 s for 94/94 Part Studio documents (slowest 377 ms, c2-housing-draft; c2-rings-loft 5178 → 218 ms, cf-surface-pattern 10083 → 261 ms); a first open (no cache) 72/95, unchanged: regeneration-bound (OPEN-SLOW-SURFACE-PATTERN, OPEN-SLOW-SHELL-FILLET).** 11-06 20:44 (vibe5 idle, release, Xvfb + llvmpipe, median of 3; table below): 73/95 show their model within 0.5 s of opening (parity corpus 87 + tests/perf 8; pins-assembly-1000 is an assembly, not measured by the Part Studio script). Every slow one is regeneration (OCCT Shell / Fillet, Booleans of many tools, FeatureScript custom features), not scene build or upload. Floor ~170-200 ms (window + GPU init to the first frame ~120-165 ms).** Earlier: v0.4.11 warm: opening → first viewport frame 130–200 ms (an empty viewport, before the model) ✓; first launch 800 ms |

### Assembly and drawing tabs with the display cache (11-07 02:36)

Lead 11-07: the display cache for assembly and drawing tabs too (app-core `display_cache`: key, files, eviction,
`regenerating_row`; assembly-ui `assembly_open.rs`: a snapshot of the evaluation, each mesh once, view-only until the
real one, mates / connectors / relations / patterns / Insert / Explode wait with WAITING, a drag says so; drawing-ui
`drawing_open.rs`: the opened sheet's `BuiltSheet` (JSON in the cache: the sheet data's serde attributes need it), shown
until the real sheet is complete, a tool waits, a click says so). Measured as the Part Studios
(`tools/perf/open-times.py --runs 3 --reopen 3`, vibe5, release): corpus copies whose first tab is "Assembly 1" with
Part Studio 1 inserted (`-asm`) or a drawing of it with the suggested views (`-drw`), ~/agents/psui/open-tabs. Reopen
within 0.5 s 12/15; over: cf-surface-pattern-asm 685 (its full meshes on llvmpipe: the cached result's frame 553 ms after
it is in), cf-hose-routing-asm 512, pins-assembly-1000 1163 (llvmpipe draws the 1000 pins ~700-900 ms; its evaluation is
5-120 ms and the UI thread's apply 9 ms; perf.yml's macOS runners draw it in ~9 ms a frame: not a cache case).
Frame-level test of the swap (`the_swap_from_the_cached_display_to_the_real_one_changes_no_frame`, a real GPU via
`Gpu::headless_rendering` + `deliver_frames`, skipped without an adapter): every presented frame from the cached model's
first is the model's own picture. It found the opened model's first 0.5 s drawn at navigation's reduced detail (no
outline: the first fit counted as navigation): `Viewport::jump_camera`.

| document | open → model (ms) | reopen → model | open → full | regen starts | regen (preview) | regen (full) | display → frame | live regen |
|---|---|---|---|---|---|---|---|---|
| cf-surface-pattern-asm | 16573 | 685 | 16573 | 192 | 15360 | 15360 | 553 | – |
| cf-surface-pattern-drw | 10589 | 191 | 11713 | 122 | 0 | 0 | 57 | – |
| c2-impeller-loft-asm | 9593 | 383 | 9593 | 104 | 9270 | 9270 | 65 | – |
| c2-impeller-loft-drw | 6771 | 143 | 7019 | 103 | 0 | 0 | 30 | – |
| c2-rings-loft-drw | 5024 | 161 | 5025 | 124 | 0 | 0 | 34 | – |
| c2-rings-loft-asm | 4881 | 229 | 4881 | 101 | 4731 | 4731 | 37 | – |
| pins-assembly-1000 | 1151 | 1163 | 1151 | 172 | 14 | 14 | 914 | – |
| cf-hose-routing-drw | 1021 | 196 | 3552 | 154 | 0 | 0 | 43 | – |
| cf-hose-routing-asm | 856 | 512 | 856 | 150 | 257 | 257 | 278 | – |
| c2-valve-config-asm | 770 | 207 | 770 | 108 | 605 | 605 | 44 | – |
| c2-valve-config-drw | 766 | 177 | 858 | 104 | 0 | 0 | 30 | – |
| c2-hunting-knife-asm | 347 | 187 | 347 | 104 | 196 | 196 | 36 | – |
| c2-hunting-knife-drw | 342 | 154 | 346 | 102 | 0 | 0 | 25 | – |
| bearing-block-takumi-asm | 182 | 183 | 182 | 105 | 16 | 16 | 47 | – |
| bearing-block-takumi-drw | 170 | 148 | 198 | 99 | 0 | 0 | 26 | – |

### Document open with the display cache (11-06 23:19)

Before / after (lead 11-06): `tools/perf/open-times.py --runs 3 --reopen 3` (release 5bb968de, vibe5 idle): each
document opened 3 times on an empty display cache ("open → model": before the cache, what a first open still is), then
3 times on the cache the last open wrote ("reopen → model": the first frame drawing a model, the cached one's or a
regeneration's that came first). "regen starts": opening to the background regeneration's start. The regenerated
display replaces the cached one once its scene is built off the UI thread (`apply_display_async`: the old scene stays
drawn until then). GPUI test `a_cached_display_stands_in_until_the_identical_real_one_lands`: the cached display's
meshes equal the regenerated one's, the view is view-only with "Regenerating…" until it lands, a feature dialog asked
for meanwhile says WAITING at once and opens when it lands. Live's page load: REQUESTS 20:20, not recorded yet.

| document | open → model (ms) | reopen → model | open → full | regen starts | regen (preview) | regen (full) | display → frame | live regen |
|---|---|---|---|---|---|---|---|---|
| pins-assembly-1000 (timeout) | – | – (no cache) | – | – | – | – | – | – |
| cf-surface-pattern | 10083 | 261 | 10400 | 115 | 9880 | 52 | 92 | 4178 |
| c2-impeller-loft | 9810 | 322 | 9999 | 117 | 9620 | 205 | 82 | 2773 |
| c2-rings-loft | 5178 | 218 | 5200 | 103 | 5010 | 6 | 50 | 1557 |
| release-fillet-hang | 5066 | 183 | 7145 | 96 | 4931 | 2096 | 39 | – |
| c2-sm-louver | 3556 | 227 | 3829 | 99 | 3384 | 61 | 76 | 4987 |
| cf-fill-pattern | 3198 | 219 | 3659 | 141 | 2728 | 292 | 327 | 4711 |
| knurl-d25x25-45-45 | 1993 | 369 | 2058 | 168 | 1746 | 74 | 78 | – |
| c2-sm-corner-break | 1760 | 223 | 1874 | 109 | 1590 | 15 | 62 | 1972 |
| cf-drive-train-generator | 1759 | 313 | 2155 | 146 | 1480 | 40 | 126 | 4156 |
| cf-setback-fillet | 1285 | 217 | 1422 | 118 | 1112 | 30 | 49 | 1089 |
| c2-valve-config | 1261 | 370 | 1298 | 157 | 1018 | 54 | 90 | 175 |
| cf-spur-gear | 1244 | 297 | 2174 | 122 | 995 | 136 | 127 | 2112 |
| fins-400 | 1187 | 257 | 1187 | 93 | 1034 | 1034 | 60 | – |
| cf-sheet-metal-shaped-flange | 1076 | 263 | 1079 | 166 | 837 | 3 | 69 | 1026 |
| cf-sheet-metal-model-plus | 1056 | 227 | 1111 | 127 | 877 | 70 | 55 | 917 |
| learning-center-michael | 811 | 214 | 905 | 95 | 661 | 67 | 58 | 96 |
| cf-part-draft | 756 | 214 | 814 | 107 | 589 | 11 | 60 | 1241 |
| cf-fix-pcb | 642 | 265 | 662 | 163 | 434 | 2 | 45 | 618 |
| c2-housing-draft | 590 | 377 | 634 | 151 | 368 | 9 | 69 | 105 |
| c2-hunting-knife | 561 | 328 | 621 | 163 | 318 | 13 | 77 | 408 |
| nist-mtc-crada-assembly | 535 | 208 | 1233 | 126 | 161 | 160 | 247 | – |
| cf-planetary-gears | 517 | 244 | 565 | 129 | 333 | 6 | 53 | 542 |
| cf-sheet-metal-overcut-dogbone | 476 | 290 | 497 | 196 | 244 | 1 | 38 | 270 |
| cf-wave-spring | 457 | 201 | 508 | 103 | 299 | 35 | 52 | 740 |
| cf-hose-routing | 454 | 246 | 638 | 131 | 241 | 42 | 73 | 491 |
| cf-belt | 438 | 356 | 490 | 195 | 150 | 3 | 72 | 35 |
| c2-extension-spring | 434 | 342 | 473 | 162 | 197 | 2 | 69 | 183 |
| cf-circular-pattern | 412 | 222 | 460 | 107 | 257 | 2 | 49 | 237 |
| cf-sheet-metal-hem | 399 | 254 | 421 | 160 | 196 | 1 | 41 | 95 |
| c2-hex-bookcase-derived | 396 | 322 | 417 | 97 | 254 | 12 | 43 | 290 |
| c2-bottle-split | 384 | 361 | 580 | 151 | 144 | 5 | 87 | 40 |
| cf-3d-spline | 364 | 328 | 364 | 170 | 149 | 149 | 44 | 43 |
| c2-helical-sweep | 338 | 209 | 363 | 95 | 199 | 7 | 50 | 40 |
| cf-sheet-metal-flange-plus | 338 | 224 | 357 | 133 | 160 | 0 | 40 | 208 |
| bushing2-0-joshua | 332 | 328 | 384 | 157 | 88 | 16 | 78 | 46 |
| cf-transform-pattern | 330 | 234 | 347 | 134 | 133 | 2 | 65 | 206 |
| pipeflange-steven | 329 | 200 | 366 | 96 | 176 | 29 | 54 | 212 |
| cf-linear-pattern-with-seed-location | 322 | 208 | 363 | 107 | 168 | 2 | 46 | 132 |
| cf-o-ring-generator | 307 | 231 | 805 | 108 | 119 | 288 | 80 | 65 |
| cf-point-pattern | 295 | 238 | 322 | 140 | 103 | 1 | 52 | 53 |
| c2-variable-fillet-faceblend | 294 | 310 | 294 | 156 | 26 | 1 | 113 | 58 |
| bracket120-datenkrake | 293 | 310 | 293 | 154 | 16 | 1 | 123 | 40 |
| c2-sm-exhaust-clamp | 285 | 188 | 303 | 96 | 142 | 1 | 46 | 71 |
| c2-duct-multipart | 278 | 307 | 278 | 157 | 29 | 29 | 89 | 107 |
| big-plate | 278 | 248 | 465 | 93 | 70 | 49 | 104 | – |
| c2-fan-blade-sweep | 275 | 213 | 302 | 103 | 116 | 7 | 53 | 1418 |
| cf-parameter-pattern | 272 | 216 | 272 | 128 | 111 | 111 | 31 | 75 |
| cf-butt-joint-with-pin | 271 | 217 | 271 | 155 | 89 | 89 | 28 | 191 |
| cf-pocket-holes | 271 | 206 | 294 | 106 | 122 | 8 | 42 | 55 |
| cf-parametric-curve | 270 | 231 | 270 | 148 | 95 | 95 | 25 | 315 |
| clamp-hms | 270 | 196 | 296 | 99 | 116 | 2 | 50 | 146 |
| motor-mount-jack | 268 | 192 | 322 | 93 | 123 | 25 | 52 | 53 |
| cf-extrude-vertex | 266 | 214 | 266 | 129 | 98 | 98 | 33 | 27 |
| parts-1000 | 262 | 361 | 696 | 92 | 44 | 13 | 120 | – |
| cf-box-dovetail-joint | 255 | 232 | 281 | 112 | 94 | 1 | 52 | 360 |
| cf-syncro-rotator | 249 | 204 | 268 | 108 | 99 | 1 | 42 | 38 |
| cf-metric-keys | 245 | 201 | 245 | 106 | 95 | 0 | 42 | 25 |
| cf-lighten | 244 | 219 | 244 | 126 | 72 | 0 | 44 | 114 |
| cf-multi-plane | 241 | 197 | 241 | 105 | 93 | 0 | 42 | 102 |
| cf-mortise-and-tenon | 240 | 196 | 254 | 105 | 94 | 0 | 40 | 43 |
| bracket03-mattie | 230 | 201 | 246 | 96 | 92 | 4 | 43 | 67 |
| stef-radijator-nlukic | 230 | 193 | 257 | 92 | 90 | 10 | 47 | 74 |
| cf-snap-hook | 225 | 187 | 225 | 107 | 89 | 89 | 28 | 274 |
| exercise-jan | 216 | 190 | 241 | 93 | 70 | 1 | 48 | 98 |
| knob-jason | 216 | 196 | 299 | 95 | 67 | 5 | 53 | 125 |
| prize-wheel-tim | 216 | 192 | 245 | 95 | 76 | 3 | 42 | 61 |
| clamp-guillem | 209 | 193 | 233 | 95 | 71 | 3 | 43 | 189 |
| exercise-martinl | 209 | 187 | 231 | 93 | 75 | 1 | 42 | 76 |
| c2-tpd-fillet-variable | 199 | 309 | 360 | 95 | 50 | 172 | 47 | 44 |
| live-30-features | 199 | 190 | 246 | 96 | 48 | 2 | 52 | – |
| cf-t-slot-joint | 198 | 218 | 198 | 129 | 12 | 12 | 57 | 222 |
| clamp-abhil | 193 | 188 | 195 | 96 | 32 | 2 | 53 | 89 |
| c2-lofted-basic | 192 | 201 | 192 | 103 | 5 | 0 | 85 | 22 |
| exercise-chino | 188 | 186 | 208 | 93 | 45 | 2 | 47 | 88 |
| cf-wiring | 187 | 184 | 187 | 121 | 0 | 0 | 65 | 11 |
| bearing-block-takumi | 184 | 193 | 184 | 94 | 10 | 1 | 79 | 27 |
| knob-tim | 184 | 183 | 184 | 92 | 11 | 1 | 80 | 41 |
| cf-torus-dome-and-sphere | 181 | 195 | 181 | 105 | 3 | 0 | 73 | 60 |
| flange-walter | 181 | 186 | 181 | 94 | 6 | 3 | 82 | 16 |
| knob-kraut | 180 | 186 | 209 | 93 | 18 | 10 | 70 | 55 |
| learning-center-frank | 179 | 189 | 179 | 92 | 6 | 1 | 80 | 20 |
| learning-center-ivan | 179 | 180 | 203 | 95 | 19 | 9 | 59 | 25 |
| c2-steak-mold-draft | 178 | 187 | 178 | 96 | 10 | 1 | 71 | 30 |
| flange-josh | 178 | 184 | 178 | 92 | 11 | 1 | 73 | 20 |
| exercise-anthony | 175 | 190 | 175 | 92 | 6 | 1 | 77 | 17 |
| learning-center-devon | 175 | 186 | 175 | 92 | 6 | 1 | 77 | 28 |
| cf-curved-text | 174 | 179 | 174 | 108 | 3 | 3 | 64 | 506 |
| pulley02-maxwell | 173 | 184 | 200 | 92 | 19 | 12 | 62 | 34 |
| exercise-evana | 172 | 179 | 172 | 92 | 7 | 1 | 73 | 34 |
| mounting-plate-giana | 170 | 185 | 170 | 95 | 9 | 1 | 67 | 51 |
| pillow-block-maysa | 170 | 181 | 170 | 91 | 5 | 1 | 74 | 13 |
| train-joe | 170 | 182 | 170 | 93 | 14 | 1 | 62 | 100 |
| untitled-document-chris | 170 | 183 | 170 | 92 | 10 | 1 | 68 | 39 |
| exercise-jonathan | 168 | 180 | 168 | 93 | 4 | 0 | 71 | 17 |

72/95 documents show their model within 0.5 s of opening, 94/95 of reopening

### Document open, per document (11-06 20:44)

`tools/perf/open-times.py --app <release noshape> --out <dir> --runs 3 <dirs/documents>` (NOSHAPE_PERF_SCRIPT=open:
the app quits once the full display's frame is up; milestones `open: preview/full display in (regen …)` and
`open: preview/full model frame presented`, armed with `perf::arm_frame_milestone` and logged when the viewport
presents the frame that draws them). Corpus documents from parity_models with NOSHAPE_PARITY_SAVE (main 11-06 20:00).
"open → model": "opening document" to the first frame drawing the preview display (what the user sees first); "open
→ full": to the refined display's frame; "display → frame": the display arriving to that frame (scene build, upload,
the frame; llvmpipe here, so the GPU share is high: NIST's 516 is ~70 ms scene build and the rest the software GPU's
upload and first 4× MSAA draw). "live regen": live.jsonl's full regeneration time (live's page load not recorded yet:
REQUESTS 20:20). Where the slowest go (preview regeneration):
- cf-surface-pattern 10.3 s (live 4.2): the custom Surface Pattern 3's `opBoolean` of the patterned tools, 7.4 s (bool);
- c2-impeller-loft 9.4 s (live 2.8): Shell Gehäuse 2.4 s, two Fillets 1.3 + 1.2 s, Delete face 0.6 s (core);
- c2-rings-loft 5.1 s (live 1.6): Shell 1 4.6 s (core);
- release-fillet-hang 5.1 s (perf fixture): its Fillet; c2-sm-louver 3.5 s (live 5.0); cf-fill-pattern 3.3 s (live 4.7):
  Fill pattern ×2 1.9 + 0.7 s; cf-drive-train-generator 2.1 s (live 4.2): the custom feature, 2.5 s in one call;
  c2-sm-corner-break 1.7 s (Flange 0.6, Extrude 2 0.5, Sheet metal model 0.4); cf-setback-fillet 1.5 s (Fill surface in
  the custom feature); cf-spur-gear 1.4 s.
- Ours: NIST's full display's scene build runs 296 ms on the UI thread at open (apply_display, not the async build).

| document | open → model (ms) | open → full | regen (preview) | regen (full) | display → frame | live regen |
|---|---|---|---|---|---|---|
| pins-assembly-1000 (timeout) | – | – | – | – | – | – |
| cf-surface-pattern | 10269 | 10576 | 10042 | 58 | 100 | 4178 |
| c2-impeller-loft | 9396 | 9535 | 9238 | 142 | 49 | 2773 |
| c2-rings-loft | 5117 | 5142 | 4971 | 6 | 46 | 1557 |
| release-fillet-hang | 5116 | 7184 | 4970 | 2081 | 43 | – |
| c2-sm-louver | 3524 | 3776 | 3335 | 56 | 94 | 4987 |
| cf-fill-pattern | 3254 | – | 2761 | 300 | 343 | 4711 |
| knurl-d25x25-45-45 | 2179 | 2261 | 1898 | 89 | 93 | – |
| cf-drive-train-generator | 2136 | 2539 | 1867 | 41 | 125 | 4156 |
| c2-sm-corner-break | 1690 | 1780 | 1537 | 17 | 54 | 1972 |
| cf-setback-fillet | 1542 | 1846 | 1304 | 36 | 80 | 1089 |
| cf-spur-gear | 1431 | 2380 | 1170 | 159 | 131 | 2112 |
| fins-400 | 1283 | 1283 | 1122 | 1122 | 57 | – |
| cf-sheet-metal-shaped-flange | 1132 | 1132 | 889 | 3 | 72 | 1026 |
| cf-sheet-metal-model-plus | 1074 | 1120 | 888 | 68 | 58 | 917 |
| learning-center-michael | 850 | 935 | 686 | 71 | 61 | 96 |
| nist-mtc-crada-assembly | 827 | 1460 | 172 | 187 | 516 | – |
| c2-valve-config | 822 | 848 | 663 | 34 | 56 | 175 |
| cf-part-draft | 769 | 826 | 597 | 12 | 59 | 1241 |
| cf-fix-pcb | 698 | 719 | 469 | 2 | 43 | 618 |
| cf-planetary-gears | 568 | 614 | 383 | 7 | 55 | 542 |
| cf-hose-routing | 516 | 721 | 245 | 44 | 104 | 491 |
| cf-sheet-metal-overcut-dogbone | 495 | 510 | 250 | 1 | 40 | 270 |
| cf-wave-spring | 472 | 525 | 310 | 33 | 56 | 740 |
| c2-housing-draft | 453 | 472 | 315 | 6 | 42 | 105 |
| pipeflange-steven | 445 | 507 | 227 | 36 | 70 | 212 |
| cf-circular-pattern | 416 | 459 | 257 | 2 | 49 | 237 |
| cf-sheet-metal-hem | 411 | 436 | 201 | 1 | 43 | 95 |
| c2-hex-bookcase-derived | 401 | 423 | 256 | 12 | 43 | 290 |
| exercise-martinl | 345 | 395 | 110 | 2 | 71 | 76 |
| c2-hunting-knife | 343 | 364 | 199 | 7 | 45 | 408 |
| cf-sheet-metal-flange-plus | 342 | 361 | 165 | 0 | 43 | 208 |
| knob-jason | 341 | 441 | 90 | 13 | 85 | 125 |
| c2-helical-sweep | 332 | 357 | 192 | 7 | 46 | 40 |
| flange-josh | 328 | 328 | 17 | 3 | 154 | 20 |
| cf-transform-pattern | 325 | 348 | 137 | 2 | 46 | 206 |
| flange-walter | 324 | 324 | 9 | 2 | 147 | 16 |
| cf-linear-pattern-with-seed-location | 322 | 361 | 167 | 2 | 48 | 132 |
| exercise-jan | 321 | 332 | 91 | 2 | 65 | 98 |
| cf-snap-hook | 320 | 320 | 106 | 106 | 39 | 274 |
| cf-o-ring-generator | 316 | 834 | 119 | 331 | 92 | 65 |
| cf-point-pattern | 305 | 333 | 106 | 2 | 46 | 53 |
| exercise-jonathan | 304 | 304 | 6 | 1 | 138 | 17 |
| motor-mount-jack | 296 | 350 | 133 | 34 | 59 | 53 |
| cf-pocket-holes | 294 | 319 | 130 | 9 | 50 | 55 |
| big-plate | 294 | 492 | 74 | 47 | 116 | – |
| cf-parameter-pattern | 293 | 293 | 119 | 119 | 36 | 75 |
| cf-parametric-curve | 289 | 289 | 106 | 106 | 30 | 315 |
| c2-sm-exhaust-clamp | 278 | 291 | 146 | 1 | 39 | 71 |
| cf-belt | 274 | 298 | 107 | 1 | 42 | 35 |
| parts-1000 | 273 | 774 | 45 | 14 | 125 | – |
| train-joe | 271 | 271 | 19 | 3 | 111 | 100 |
| cf-extrude-vertex | 269 | 269 | 102 | 102 | 35 | 27 |
| clamp-hms | 269 | 297 | 119 | 2 | 46 | 146 |
| c2-fan-blade-sweep | 268 | 292 | 112 | 8 | 48 | 1418 |
| c2-extension-spring | 263 | 280 | 125 | 2 | 43 | 183 |
| cf-butt-joint-with-pin | 257 | 257 | 92 | 92 | 25 | 191 |
| cf-syncro-rotator | 255 | 274 | 102 | 1 | 42 | 38 |
| cf-lighten | 249 | 262 | 79 | 1 | 41 | 114 |
| cf-metric-keys | 249 | 268 | 98 | 0 | 44 | 25 |
| cf-multi-plane | 249 | 267 | 96 | 0 | 42 | 102 |
| c2-bottle-split | 246 | 391 | 94 | 3 | 56 | 40 |
| cf-mortise-and-tenon | 244 | 255 | 93 | 0 | 41 | 43 |
| cf-box-dovetail-joint | 242 | 269 | 88 | 1 | 45 | 360 |
| prize-wheel-tim | 240 | 276 | 87 | 4 | 49 | 61 |
| stef-radijator-nlukic | 235 | 262 | 90 | 12 | 48 | 74 |
| bracket03-mattie | 228 | 250 | 88 | 4 | 43 | 67 |
| cf-3d-spline | 226 | 226 | 93 | 93 | 29 | 43 |
| knob-kraut | 217 | 263 | 19 | 15 | 95 | 55 |
| clamp-guillem | 216 | 244 | 71 | 2 | 49 | 189 |
| knob-tim | 212 | 212 | 12 | 1 | 103 | 41 |
| live-30-features | 211 | 260 | 53 | 2 | 60 | – |
| cf-wiring | 208 | 208 | 0 | 0 | 75 | 11 |
| cf-t-slot-joint | 204 | 204 | 13 | 13 | 60 | 222 |
| bushing2-0-joshua | 201 | 223 | 41 | 7 | 51 | 46 |
| exercise-chino | 199 | 218 | 46 | 4 | 44 | 88 |
| mounting-plate-giana | 199 | 199 | 8 | 1 | 85 | 51 |
| clamp-abhil | 196 | 197 | 23 | 3 | 57 | 89 |
| pillow-block-maysa | 194 | 194 | 5 | 1 | 89 | 13 |
| pulley02-maxwell | 194 | 225 | 20 | 9 | 72 | 34 |
| learning-center-frank | 193 | 193 | 6 | 1 | 82 | 20 |
| bearing-block-takumi | 192 | 192 | 10 | 1 | 87 | 27 |
| c2-tpd-fillet-variable | 191 | 352 | 51 | 169 | 45 | 44 |
| c2-duct-multipart | 190 | 190 | 27 | 27 | 50 | 107 |
| learning-center-ivan | 188 | 209 | 19 | 10 | 72 | 25 |
| untitled-document-chris | 187 | 187 | 11 | 1 | 81 | 39 |
| cf-torus-dome-and-sphere | 184 | 184 | 4 | 0 | 76 | 60 |
| learning-center-devon | 183 | 183 | 6 | 1 | 76 | 28 |
| cf-curved-text | 182 | 182 | 3 | 3 | 63 | 506 |
| bracket120-datenkrake | 181 | 181 | 13 | 1 | 71 | 40 |
| c2-variable-fillet-faceblend | 181 | 181 | 15 | 1 | 67 | 58 |
| exercise-anthony | 180 | 180 | 6 | 0 | 81 | 17 |
| exercise-evana | 175 | 175 | 7 | 1 | 73 | 34 |
| c2-lofted-basic | 172 | 172 | 5 | 0 | 73 | 22 |
| c2-steak-mold-draft | 171 | 171 | 10 | 0 | 67 | 30 |

73/95 documents show their model within 0.5 s of opening

## In progress

**11-08 22:09 UTC, speed area (lead: parity index speed 70.8 %, psui and bool first).** The cases: 85 parity
regenerations + 13 opens (all opens at or under live: 0 lost). Profiled on vibe5 (`NOSHAPE_PARITY_SPLIT=1`
parity::parity_models: open / feature_list / query_parts, then each Part Studio's features against its display):
regenerations 85.9 % as measured on today's tree (the qa 14:11 run was 70.8 % for the area on a slower, niced run).
Every top lost point is feature evaluation (bool's / core's; bool's handoff has the list); psui's share is the
display build: elliptical edges meshed here (ea06cbfe: cf-part-draft display 156 → 35 ms), a circle's surface of
revolution meshed as a torus and a face closed both ways splitting its seams as rows (a0c4bae0: cf-o-ring-generator
Medium 1.2 → 0.18 s, display 1.07x BRepMesh's triangles); corpus display meshing 2.93 → 1.89 s (ours / BRepMesh's
time in the same runs 0.28 → 0.16). Measurement, asked of the lead: ours is open + feature_list + query_parts wall
time (query_parts' mass properties 0.1-1.6 s on some; our display build) where live's `full_regen_ms` is its
Regeneration times panel (features only): 85.9 % as measured, 90.5 % features only. Next: c2-rings-loft's 395 ms of
display (2 faces we don't take, Offset-like, settle all 26 others to BRepMesh), cf-fill-pattern 107 ms.


**11-08 19:20 UTC, early warm-up measured (perf 42504449807 on f57f96b): out.** Ours (caches cleared) vs early:
macOS 26 662 → 715 ms (+54), macOS 15 1180 → 1802 ms (+622: window created 648 → 1652, the warm-up's GPU work on
the one paravirtual GPU held AppKit's window); reverted, bench back to ours / warm. This run's macOS 15 was slow
throughout (ours 1180 ms against 975 in the last; window creation 403 ms against 271): ours over 1 s on that runner.
What is left of ours there: window creation with the Metal caches cleared (GPUI's 8 pipelines compiled for the GPU,
~300 ms over warm) and frame 1's first GPU use (~500 ms from workspace drawn); the lever would be pipelines shipped
compiled for the GPU (Metal binary archives / metal-tt for Apple GPU families), a bigger project. Gate flags of the
same job: texture zoom / orbit, pan, row-click not repeated on re-measure; readback orbit UI CPU p95 repeated
(4.12 → 5.12, again 4.70 → 6.51; macOS 26 flagged readback zoom UI CPU in the last run too): texture mode's UI CPU
unchanged (0.81 → 0.79); readback only, nothing per-frame changed since v0.4.36 in our viewport code; watch it.
WATCH (lead 11-08): readback orbit UI CPU p95. Flagged twice (11-08 macOS 26 readback zoom, macOS 15 readback orbit);
a third flag on a quiet runner (its other rows steady) → bisect it.


**11-08 17:21 UTC, ui-parity leftovers.** key-rollback-bar-arrows matches (the clicked rollback bar reads as a selected
item, live's tree node). config-variable-edit-definition: focus now "Name" as live's; its live recording reads the
popup as a panel (recorder without live2), vs-cfg-variable-dialog as a dialog: re-record asked (REQUESTS 17:21).
sketch-dim-menu-unconfigure step 16: live's toast had timed out (5.5 s between its steps; ours replays in ~1 s): the
runner doesn't pace by live's `t`, not a behaviour. For studios: vs-cfg-variable-dialog step 5, the Variable Studio's
own Variable input (variable_config.rs) reports no focus label ("Name" in live).


**11-08 17:13 UTC, cold start split (perf 42489298913 on 80cf796).** noshape's own cold start (our caches cleared,
the OS warm, medians of three): macOS 26 **697 ms**, macOS 15 **975 ms** (samples 772-1098), target 1000 met on both;
the runner's first launch adds 1572 / 2114 ms (window creation 750-1000 ms of it, the fresh VM's). Ours, macOS 15:
window created 410, workspace drawn 422, frame 1 taken 463 (pipelines 460), rendered 788 (its passes ~125 ms GPU,
the rest the paravirtual GPU's first use: warm launches take ~200 ms over frame 1 too), presented 975 (the UI
thread's GPUI frames queue behind it). Experiment landing: `NOSHAPE_PREWARM=early` now compiles at device ready and
renders a 64x64 frame off screen while the window is created; bench.sh's `early-<n>` launches measure it beside ours
(analyze "with NOSHAPE_PREWARM=early: N ms (±)"); default if it wins on both Macs. Not taken: skipping the empty
first frame's AO (moves its first use into the model's frame, whose open target is already missed). The same run's
macOS 26 gate flagged dialog-open UI CPU p95 6.6 → 9.6 (again 10.3 → 14.0): over all 20 samples the mean is +4 / +9 %
and ours' p50 lower (4.0 vs 4.8); the tail is GPUI layout / prepaint / paint swinging on both binaries; macOS 15 passed.
The sketch toolbar's per-render `doc_edited_in_dialog` now reads the undo top before walking the stack.


**11-08 10:20 UTC (lead: focus differences, face-appearance menu, cap hover in assemblies, cold start ours < 1 s).**
Suite 514/530 (SCORE). Cold start: bench.sh STARTUP (end of each job: our caches cleared, then warm; analyze.py
"Cold start split"); startup fixes landed: font probes without listing every font (065c55f7), the command line's
document read on a thread from process start (fdb1e521). Perf 42468464451 (065c55f7: the split, fonts) and 42470955223
(0fd8cabd: + prefetch) running: read the split and "noshape's own cold start" against 1 s. The 07:25 perf
(42458734521) flagged 2× pan / zoom GPU p95 +10% vs v0.4.35 in the job, confirmed on re-measure; the 00:02 run's
main had the same values (2× pan 8.80 there, 8.55 here): likely the runner, check against these two runs. Coverage:
shortcuts 84/89 (5 new scenarios filed with live, REQUESTS 10:19); menus 115/121 left: Delete annotation, Delete
column, Edit definition…, Open linked document, Revision history, a configured sketch dimension's Unconfigure.


**11-08 08:13 UTC, FRAME-PROFILE-OTHER-STUDIO (lead):** done, see TICKETS (value `<tab>#<sketch>.4294967295`); the
variable table's Filter feature list leaves the parts list alone and its row's hover ends under its menu
(variable-table-row-filter matches). Live asked (REQUESTS 07:57) for the field text / configuration / placement.


**11-08 06:34 UTC.** Part Studio ui-parity: 493/530 on 11-07 19:54's binary (all 530, vibe5, /root/psui-scratch/uip-psui.sh),
498 on main 05:31 (key-backtick-select-other, face-menu-select-other, frame-tool, plane-line-angle,
sketch-circumscribed-polygon now match); this commit +6 (camera-menu ×9 counted once fixed: Previous view never
greyed, live 2212; a view cube submenu reads "camera-submenu" with its checks; in-context ×2: a context made by Create
Part Studio in context was "out of date" at once (the assembly compared it by primary instance; now ops'
`out_of_date_in`), a page dropdown reads its top level and highlights its chosen row, its actions hand the keyboard to
the graphics). Dynamic suppression's box completes like a number field ("true"), ✓ deselects and says "Dynamic
suppression added." / "…updated." / "…removed." (live 2289). Left, not ours: analysis-thickness ×3 (live's free account), external-thread /
sm-form (account memory), fillet-chip ×2 ("Unknown" start), feature-menu-suppress-unsuppress (live hides the sketch
row only after the server's regeneration), dialog-number-rclick-menu (live runner). Live 2289: "Edit dynamic
suppression" is a hover submenu (its old recordings missed it; re-recorded 28a1e9df). 07:45: visibility conditions'
modal recorded as live's recorder reads it, Paste inputs over taken IDs says "No inputs pasted. Existing inputs have
the same internal FeatureScript ID." (ops' `info` too), the configuration ▾ holds the keyboard while open, a field's
Configure leaves it (config-* now match but config-depth-configure: live counts Configure in the open dialog's undo;
ours has it in the document's only). The Appearance editor's ✓ / ✕ clear its selection. 08:30: dialog-checkbox-convert-back
(its field's menu over the input) and key-mod-shift-f (the search pane's Find text noted as "other", Esc closes it)
match; part-menu-release is live's Release paywall; mirror-face-type: live moves the keyboard to the query after
Face mirror and Chamfer's Two distances only (9 other recorded type switches keep it on the select: a DOM detail,
left). Left, ours: (08:59) a face appearance's row
is live 2292's: Rename… (the panel's "Rename" popup, `FaceAppearance::name`), Edit appearance…, Suppress (row struck
through, swatch crossed, faces back to the part colour; then Unsuppress · Delete), Delete; op part_appearance_row;
sketch-row-copy-paste-onto-front (asked live 11-07), variable-table-row-filter (ours marks the row related at the
right-click, live only after Filter feature list).
Perf 42427964353 (main 5660398, all green), macOS 26: cold start 3446 ms (target 1000): 417 ms to the GPUI app,
gpui-kit 470 ms (system fonts), **window creation 1044 ms** (later launches 20-100 ms; GPUI's metallib is precompiled
in release, so it is the fresh VM's first window), first GPU frame 300 ms. Warm start 825 ms (met). Open 555 ms
(target 500): the first frame's GPU 143 ms is the runners' first-frame stall. First frames (camera starts after an
idle) GPU 19-244 ms, steady 6 ms: the paravirtual GPU's idle stall (N000000000). Fixed here: NIST's first model
frame compiled `silhouette-pulled` in the frame (62 ms), now in the deferred set. NIST's 327-372 ms between "full
display in" and its scene: the 36 meshes' uploads (silhouette edges, picks, edge levels) one after another; now on
every core (`upload_bodies_in_parallel`, perf line "scene: N body meshes uploaded in"): ~80 ms on vibe5. Next: frame
3's ~180 ms on macOS beyond its pipeline and color pass (perf on main once this lands).


**11-07 08:58 UTC, lead's order (the 121's final count, then Add custom features, the std tool dialogs, the sketch tool
diffs, focus):** the 121: 65/121 scenarios, 498/589 states; all 530: 460/530, 2937/3069.
- Add custom features (live 2213): ⊞ opens `SelectItemPanel` in its new `PanelMode::CustomFeatures` (300 × 581,
  right-aligned under the toolbar; Current document | Other documents, opening on Other documents with "Search or paste
  URL" focused; FeatureScript samples, Community spotlight, Owned by me, … Public; Current document at its latest
  version with the changes box, Feature Studio rows (fold) and feature rows (initials); a click adds the feature to
  the toolbar at once and stays light blue; only ✕ closes it). Added features live in the toolbar prefs
  (`ToolbarPrefs::custom_features`, ops `ui_toolbar` `set.custom_features`); ⊞ is then a split tool, its ▾ the features
  and Add custom features, the face the one last picked; a feature face's right-click: Open linked document, Remove.
- Model: a custom feature type can name a Feature Studio of another document's version: `custom:ext:<mv>|<tab>|<path>:<name>`
  (`studio_type_id`, `parse_type_id` splits at the last colon), compiled once by `custom::external_studio` (versions
  never change) and found by `features::get`; ops sets the external tab loader too (it didn't: `ext:` imports failed in
  the CLI). Ops `feature_types` takes `document` + `version` (e2e `a_custom_feature_from_another_documents_version`).
  Left: the ▾ menu's Sort custom features, Update… / Update all… (a newer version), the search box's sort button, the
  live face default (live showed the first added feature as the face in other documents), an external studio's own
  imports of its document's tabs by tab id (not resolved). Other documents grey the documents without custom features.
- The faulty field: `field_error` no longer falls back to the first empty selection field (fixed tag-tool and
  finish-flex-pcb-model-tool; Face blend now names its empty side, as live). Compare: tooltips' whitespace collapsed.
- Std tool dialogs (in this change): Ruled surface's Use cubic interpolation off (live 2212); Edit curve's empty Curves
  field faulty (opExtractWires fails with EXTRACT_WIRES_NEEDS_EDGES, which std's Edit curve maps), its Degree / Control
  points / Spans 0 without a curve and read-only fields recorded "disabled"; a dialog's button (Reset triad) recorded as
  live's control; Constrained surface opens with its Vertices array active, not Tolerance. Left: Frame's Sketch profile
  picker (live: a library pick, 8020 › 10 Series › 1001 by default; our profiles are text names, no 8020 profiles),
  Routing curve's Points array (live shows Point 0 expanded), Offset surface's parts.
- Sketch (next change): the Conic's second end takes the rubber band's horizontal / vertical between its ends (it was
  put on the conic and refused); the recorder counts an open spline's / Bezier's / conic's two end Coincidents and a
  Bezier's BEZIER_DEGREE, as live's model holds them; the right-click
  menu's Use takes the right-clicked edge (it used whatever was under the cursor at the row's click: nothing). Parts
  list: Surfaces and Curves start collapsed (live 2176). Asked live (REQUESTS 09:18): the circumscribed polygon's
  constraints, Flow's toolbar state, a pasted sketch's selection owner, Frame's 1001 default.
- Frame's Sketch profile library (lead 11-07, P1): `regen/features/frame_library.rs`, profiles built from the
  published dimensions their size names give (no Onshape geometry): ISO and ANSI (Rectangular bar / tube, Round bar /
  tube; live's 1,076 size names and defaults, frame_library.json; ISO Rectangular tube's Rounded: outer r 2t, inner r t,
  provisional until live measures). `profileSketch` = `frame-library:<standard>/<profile>\tSize=…;Rounded=…`. The
  dialog's field is the Select sketch panel (shows the profile name), opening on Libraries: Library / Type / Profile
  selects (a change updates the field at once), Size (+ Rounded) with Generate preview, Select closes it. Op
  `feature_frame_profiles`. AISC (11-07 16:20): its 14 shapes, 1,461 sizes from the AISC Shapes Database v15.0
  (dimensions only, `frame_library_aisc.json`, smallest first as live's C-shape list), Standard › Type "AISC profiles"
  › Profile; I / C / tee / L sections sharp-cornered, HSS with a 2t outside radius. 8020, AS and Construction materials (11-07 23:45, live2's lists, REQUESTS 2220):
  `frame_library_more.json` (generated by /root/psui-scratch/bin/gen-frame-more.py + gen-8020-rows.py from live's
  lists and the dimension tables in psui-scratch: as-lumber.json (InfraBuild / Liberty / BlueScope catalogues, lumber
  dressed sizes) and 8020-profiles.json (80/20 Catalog 23 drawings: slot opening, lip, cavity, depth, bore per series;
  each profile's slot layout from its title)). Levels: Standard › Type (8020's series, AS's two, "Construction wood")
  › Profile › Size; 8020 profiles have no Size (`frame-library:8020/1001`, no tab). 8020 sections: square T-slots at
  each unit, the bore, quarter-round / 45° backs; not drawn: corner radii (unpublished), align-a-grooves, Lite
  pockets, 40-4093/4/6's arc backs (drawn square). AS hot-rolled sharp (no root radii, as AISC); cold-formed RHS / SHS
  corners 2t / t (2.5t / 1.5t over 3 mm); CA / CC thickness the name's. **A new Frame defaults to 8020 › 10 Series ›
  1001** (live 2216's fresh account; then remembered). Placement (live 2220 (2)): only the tube's centre is measured;
  angles, channels and 8020 placement wait for live (the rest of 2220 went back to live, live2 paused till 11-10).
  Search ("Search by name, folder, or configuration": matches as 55 px rows, earliest match first, Enter takes the
  first) and the preview (`frame_library::preview_svg`: the section from above, pale fill, outline, origin) as live
  D134 f19 / f23; the panel's three tabs on two lines.
- Scenario differences (15:20 UTC 11-07): page dropdowns (a panel's ⋯, the in-context carets and contexts list) are
  live's "menu" and stay open on Escape (`OsMenu::page`, `mark_page_dropdown`); the Variable table's ⋯ has Collapse
  all tables only with an inserted table; select-other and the S toolbar are recorded; a pick whose creator is gone (deleted or
  suppressed) is a red "Missing Edge" with Clear missing references again (live 1993 (c); the "Unknown of" of 11-07's
  morning came from block-fillet's REST-built query, which live reads "Unknown of" untouched: asked live to rebuild
  that start, REQUESTS 15:31); Dynamic suppression before Configure suppression; "Select items to insert" is recorded; the contexts
  list reads "Context 1" with its assembly as a grey note (`OsMenuItem::note`); `[` Measure's field and a feature
  row's right press (out of a panel's input) as live. Not ours to match: Thickness analysis (live's free account has
  none), Release's paywall, the sm-form / thread / frame defaults (account memory), fillet-measurement-list-escape /
  fillet-chip-edit-healthy-moment (live's start builds an "Unknown" query). Asked live: Hide/show instances in context
  (REQUESTS 15:14).
- Speed, PLAN 2.7 rows on macOS (perf 42366320997, 11-07 11:00; lead: take the row furthest from its target): the
  GPU rows pass (2× orbit / pan / zoom p95 ≤ 8.7 ms, NIST orbit mean 6.6 ms). Furthest: **cold start to first frame**
  (the job's first launch: 2.5 s macOS 15, 3.6 s macOS 26, target 1 s; open 1.15 / 1.45 s, target 0.5 s), then the
  dialog-open / row-click latency (action to painted frame p95 54 ms; its CPU gate, 9 ms, passes). The cold launch:
  app created 0.56 s, gpui-kit 0.35 s, then 1.5 s unlogged between the session opening and the first workspace
  render. Milestones added through the window's opening (window, document model, live server, workspace, Part
  Studio view); on Linux the span is the window waiting for the renderer's device (0.5 s first run, 0.07 s warm), the
  workspace builds in 10 ms. macOS bench on main dispatched with them (perf runs 42394224613, then the next).
  11-07 20:00, perf 42395752773 (all green: 42394224613's dialog-open CPU flag was noise): only each VM's first launch
  is slow (window creation 0.45 / 1.28 s, gpui-kit 0.8 s on macOS 26: OS caches); every later launch has the window in
  20–100 ms, the first viewport frame at 0.45–1.2 s. The open row (target 0.5 s) misses on every launch: live-30 opens
  in 480–570 ms with its display in at 524 ms. Two waits: (1) the regeneration's background work started only once
  the UI thread polled its task, 106 ms after it was asked (Linux 84–216 ms): spawned at once now (schedule_regen;
  vibe5: started 0–2 ms after asked); (2) the model's frame presented 380 ms after its display was in, with the GPU
  idle (encode 3 ms, gpu wait 6 ms), and frame 1 took 228 ms from submit to present. Milestones `frame <n> submitted /
  taken by the render thread / rendered / back on the UI thread / scene set / waits for a frame slot / waits for a
  layout` for the first 6 frames find where; on lavapipe it's the one frame in flight and a 150–300 ms CPU render.
  Next: perf.yml on main with them, then fix the Mac's wait.
  Landed 611236c0 (20:10). perf 42411762630 measured v0.4.35, not main (perf.yml takes the latest release unless
  `-f ref=<sha>`): its macOS 26 flag (row-click UI CPU p95 3.0 -> 3.7, again 4.8 -> 9.4) is one outlier sample each
  (p50 equal), and the first click's 4-5 ms more is the build run first: bench.sh ran the latency script new-first
  in both passes (vibe5, alternating: v0.4.34 4.1-4.3 ms, v0.4.35 4.2-6.7). The re-measure now runs the baseline
  first. Main's frame milestones: perf 42414613905 (-f ref=f30e2e65, jobs bench).
- P0 user dock (lead 11-07 20:00, a user's friend's AppleWatchDock; user data, kept in /root/psui-scratch/dock and
  vibe5:~/psui-scratch/dock only). **Corrected 11-07 23:45:** the screenshots are tab "A - Yumusak Kutu" at history
  entry 3 (01a1b627-6bc3-775c-b358-7ba4668a0711, 13:16, "Insert feature : Puck Huni", the lid "Alt Kapak" shown),
  not 01:01: dock-top's ring and channel match only the funnel states with the lid (all 222 states rendered with
  v0.4.33: dock/hs/, hs-sheet*.png). The section's sloped band is the funnel's real wall but hatched across the
  empty space under it, and the dark band is the lid's top face inside the cavity (real). Copy at that state:
  vibe5 dock/c3/s3.noshape (history_restore). On Linux, v0.4.33 and green draw that state right (section caps:
  dock/s3/; one red sliver ~1 mm at the funnel / slot junction, inside Govde alone: a mesh fold, to look at), and
  its meshes pass both checks (faces covered, each body closed) with and without d7a6f2355's tidy, on x86 and on an
  FMA build (OCCT + bridge compiled `-march=x86-64-v3 -ffp-contract=fast`, vibe5 ~/psui-scratch/occt-fma; worktree
  /root/projects/noshape-wt/psui-fma has a NOSHAPE_NO_TIDY switch, not for landing): tidy turns 4 triangles either
  way. So the Mac fault (the lid's top face partly missing: black; the funnel's under-space counted as material)
  doesn't reproduce off a Mac; GCC's contraction isn't Apple clang's. Confirming needs the document on a Mac (the
  user on v0.4.35, or a macOS runner: lead's call, it is user data). **The model is at fault too (00:20 11-08):**
  Govde self-intersects (`Shape::self_intersects`; BRepCheck calls it valid) from feature 70 "Fis Gecidi R" (a fillet
  of the USB-C plug passage) on, in every state since; every feature before it is clean (bisected with rollback on
  c3x/s3.noshape). The red sliver on Linux is that fold; a Mac's OCCT rounds that fillet differently, so the user's
  bigger red and the space counted as material most likely come from it. For core / blend. The corpus guard now
  also checks every face's triangles face its outward normal (`facing_problems`, any surface; the dock passes). Asked live (REQUESTS 2255) for Onshape's section
  of two touching parts and a cavity (a stand-in, nothing of the user's). **Live's answer (11-08 00:20) against ours**
  (the stand-in built with noshape-cli: /root/psui-scratch/s2255/build.py, renders n-*.png): touching parts no red
  (ours 0 red pixels front / iso / bottom), the 0.5 mm overlap flat red (ours #c50000, live #c60000), each cap its
  colour at 90 % hatched, the ceiling its own shade. One difference fixed: the hatch spacing is Onshape's canvas
  pixels, CSS-sized except on a Retina Mac; ours counted device pixels, so the CLI's 2× supersampled renders (and a
  HiDPI Windows / Linux screen) hatched at half spacing (A 7 px, B 10 px; live 16 / 20; ours now 14-16 / 21; the
  angle is the part id's hash, live's changed between two section creations too). The GPU test's period check
  passed at half the period (the autocorrelation's second peak); it now fails on the old code. Cap hover (next
  commit): the caps draw into the pick pass where the last frame's mask holds material (`fs_section_cap_pick`, id
  `SECTION_CAP_PICK + plane`, `section_cap_of_pick`); a hovered plane's caps mix 33 % toward #FFC685
  (`Scene::clip_hovered`, `ui.cap_hover`; kept when the pointer leaves the graphics, as live). GPU test
  section_caps_pick_as_their_plane_and_hover_orange. A click / right-click on a cap is now empty space's (before: the
  face behind it); live's cap menu is the plane's + Turn off section view + View normal to section plane (10-2x
  notes); asked live what a click, a double click, a face-based plane's menu and ` do on a cap (REQUESTS 00:42).
  Assemblies too since 11-08 (`AsmUi::cap_hover`; `overlay_pick` leaves cap ids out: they had fallen in the mates'
  overlay range).
- Focus (in this change): a right-pressed configuration / variable row takes the keyboard ("other"; a left click in
  the panel doesn't), the contexts selector does; the driver reads a state with the keyboard where the open menu returns
  it; Add custom features' search box is recorded, Escape there drops the keyboard. 
  Traced (lead 11-07, focus per press / menu open / frame / state read, env-gated, removed after):
  - A number field's right press did focus it; then the text input's own Cut / Copy / Paste menu (gpui-component's
    NativeMenu, drawn on Linux, native on macOS / Windows) took the keyboard. A browser's native menu leaves it on the
    input: the driver now reads a state with the focus that menu hands its actions to
    (`NativeMenu::open_fallback_context`, vendor gpui-component), as for our own menus. The press focuses the field
    with its text all selected (capture phase); a converted checkbox's expression field likewise before its menu.
  - A checkbox's right press: GPUI's auto-focus of a tracked element runs for any button; a capture handler now
    `prevent_default`s it and leaves the keyboard to the dialog (live: the page body). The earlier blur inside the
    menu's build had lost "Convert to expression": the recorder had never read a converted checkbox (now "number").
  - Mirror: with its line selected, the line's constraint glyphs took the next press (overlay presses come first
    for every tool but drawing ones); Mirror's picks now pass through them, and the line stays selected while it asks.
    A mirrored line's own SYMMETRIC is recorded as live's model holds it.
  - Left: dialog-number-rclick-menu's Escape (live's recorder sends it past the browser's native menu: focus none).

**Lead 11-02 23:35, top items (milestone 11-03 evening):** (1) P1 VAR-AUTOCOMPLETE-ENTER, (2) the interaction parity
suite's noshape side (docs/PARITY-SUITES.md §2): the driver (`NOSHAPE_UI_SCRIPT` / `NOSHAPE_UI_STATE_OUT`), the state
dump, the runner, live's first 20 scenarios run in ours and a first score.
- VAR-AUTOCOMPLETE-ENTER, 11-03 00:10: three causes fixed (ticket closed, details there): the open dialog's variables
  now follow every regeneration (mod.rs apply_display); the sketch dimension box's Enter inserts the highlighted entry
  and stays open unless that changes nothing (`VarCompletion::enter_inserts`, live 11-02 on my tab: psui/wall-*.png);
  a dialog field's Enter insertion leaves the caret after it (`commit_inserted`). qa's first click / Tab caret: an Xvfb
  artifact (window never activated, no focus events): **the driver must activate its window** (`activate_window`).
- Suite format agreed with live and qa (REQUESTS 11-02 23:50): tests/ui-parity/README.md (qa's scenario draft + live's
  state format + `app`, first-text list rows, wait-for-idle). qa's 110 scenarios: /root/qa-scratch/ui-parity/ (to move
  into tests/ui-parity/ by qa); live's recordings will land in tests/ui-parity/live/.
- Driver landed 11-03 00:37 (401675d2): crates/app/src/ui_script.rs (driver), app-core ui_script.rs (named bounds:
  `mark` / `wrap`, zero cost when off; os_menu / os_select / tooltip helpers mark themselves), part_studio/ui_script.rs
  (`ui_state`, `ui_target_point`, `ui_busy`), feature_dialog/ui_state.rs (dialog, Create variable popup, focus),
  ops ui_parity.rs (start documents), tools/ui-parity/run.py (runs, compares, tallies differences by key).
  Run: `python3 -u tools/ui-parity/run.py --app $CARGO_TARGET_DIR/debug/noshape --scenarios tests/ui-parity
  --live /root/parity/ui/live --out /root/psui-scratch/uip-out --display :102` (live records into /root/parity/ui/live
  until it lands them in tests/ui-parity/live). The runner xdotool-focuses the window (Xvfb: no focus events otherwise).
- First score 11-03 01:20: 5/12 scenarios, 35+/68 states (all 5 "vars" scenarios, #wall included, match live step for
  step). Fixed on the way (live's recordings): Esc with the autofill list open only closes it (dialog fields bind Esc to
  Cancel); a commit's late change event no longer opens a unit list ("15" + Enter); the Create variable popup taking
  the focus applies nothing (row and preview stay ok); an invalid applied number previews its last valid value (live
  keeps the part) and turns the row red; the edited feature's hidden sketch row reads black. The other 7 differ only
  by the start: live built "Sketch 1 region 0" as a whole-sketch query ("Faces of Sketch 1"); asked live (REQUESTS
  11-03 00:50) to select the one region; they re-recorded the vars ones already.
- Score 11-03 02:00: **25/29 scenarios, 132/141 states** (all live had recorded). Fixed on the way: a dialog's second
  Enter didn't accept (Enter's own "\n" replaced the selected value: gpui-base patch), Tab's focus readout, autofill
  function rows ("max("). The 4 left are live-runner artifacts (keys after a row click don't reach Onshape's list under
  CDP; sketch dialog kind) — REQUESTS 11-03 02:00. Runner normalizes cloud-only menu rows (Add comment) and -1
  highlights.
- 11-03 04:40: **47/72 scenarios, 270/337 states** (03:20: 37/49) (live re-records as its runner improves). Fixed as live: right-click
  on a row outside the selection opens the menu for the selection and that row; a hidden sketch with other features
  has no Hide other / all sketches; the sketch row menu has Paste into sketch; state: rename / folder name inputs as
  focus, folder rows (the list's own walk, `list_items`), rollbackAfter "Default geometry"; targets: view cube cells,
  the parts list eye. Next differences: a select's or checkbox's focus readout, the empty field of a new Extrude
  (ours "error"), focus after Esc, hidden sketch rows while rolled back / editing a sketch.
- 11-03 08:25 (UTC): **130/184** (822/933 states) on live's x-40 batch (/root/parity/ui/live-x40: recorded with live's
  dialog dragged over the feature list; live is re-recording all into /root/parity/ui/live with the default place,
  behaviours "ui-parity (2026-11-03, P1 request 1850)"). Since then, landed / queued: live 1839 / 1844 / 1849 (row
  Enter opens nothing, ↑ / ↓ only after a click on the bar, the rolled-to hidden sketch shows, ✓ or Shift+Enter with
  typed text only applies it, a dialog's opening defaults are no step; skasm did 1853's sketch Undo); another tool or
  a double-click on another row keeps the open feature; typed text (a Variable's name) shows in title and row once
  committed; an edited Variable opens in Value; Shift+I / T with no part do nothing; state for the tool search,
  Isolate, Section view, shortcut sheet, Named views, appearance and filter; the driver opens toolbar dropdowns and
  types shift+symbol as X11 does. Left with live (REQUESTS 11-03 08:20 and 1850): fillet-* / chamfer-* after a
  preselected edge (live's account drops the edge since 04:10), Hole / Revolve remembered values, Shift+P as an undo
  step, a rolled-back variable's " = 0", Fillet moved above its Extrude reading "ok", the second Enter after an
  autofill insertion (new scenario extrude-depth-variable-enter-twice).
- 2026-11-03 19:01: parity suites check (lead): live's recordings (128) are now in tests/ui-parity/live and the model index lists
  all 58 documents (both had stayed off main: live's commits sat on an unpushed local branch). Copy new recordings
  from /root/parity/ui/live into tests/ui-parity/live when comparing them.
- 11-03 17:30: **114/128 scenarios, 887/932 states** on live's batch (/root/parity/ui/live). Left: 7 fillet-* (live's
  account drops the preselected edge, REQUESTS 1850 / 15:05), 7 sketch (skasm.md), extrude-end-type-up-to-face (live's
  start read before its list drew). Fixed since 15:10: the driver's idle check waits for an open dialog's model before
  the feature and std's editing logic (f0c58b77); and a new feature's preview waits for std's editing logic's first
  run (a new Extrude on a part face showed its New preview, a separate Part 2, ~0.9 s before it turned Add; now its
  first preview is Add; `ui.std_logic_first`, the wait is pending as a regeneration). 18:10: an edge's feature is
  the latest of its two faces' creators (ops `edge_creator(n, ps)`; was the second face by name: a fillet's tangent
  edge lit Extrude 1; x40 measure-fillet-face matches). The x40-only recordings (94 not re-recorded): 81/94, all
  13 known (Hole / Revolve remembered values, preselected edges, "#width = 0", x40 dialog place, revolve-dialog-axis).
- 11-03 15:10: lead P1 (user 17:26 report): the update banner is now a card floating at the bottom centre of every
  window (updates.rs `card`, app-core `os_menu::set_floating`), ✕ hides it for the session; navbar / start window
  slots gone; checked on :102 (start window and a Part Studio, 1280 / 560 / 760). Since 14:00 also: an edge clicked
  at its midpoint lights up its feature (fillet-asymmetric-toggle); the midpoint snaps only after the pointer moves
  (ours turned a still pointer's edge into its midpoint on any hover update: Fillet / Chamfer then opened empty
  about half the time). fillet-* / chamfer-* now differ only by live's account dropping the edge (REQUESTS 15:05).
- 11-03 14:00: driver: the empty-graphics target avoids the open dialog, planes and sketch curves (it hit the Sketch
  dialog); every scenario (274) runs with all steps found; the 76 using `empty` rechecked. Live resumed 13:58; I
  compare each new recording as it lands (a Monitor running run.py per new file on :102).
- 11-03 13:05: x40 recordings not yet re-recorded (111 psui scenarios): **100/111**; fixed since: New folder keeps the
  features selected, folder rows read with their count. Left: live-account / start effects (Hole tapped, Revolve Thin,
  edge preselection, "#width = 0", the x40 dialog place) and revolve-dialog-axis (skasm: the Line tool's first click
  after a rectangle starts nothing on the real input path, instrumented). Live's batch paused 11:59 for request 1870.
- 11-03 13:05: **51/56** on live's new batch (421/436 states); the 5 left are skasm's sketch tools (skasm.md). Since
  12:20: feature rows copy / paste nothing (live, my probe), Shift+P hides as an undo step (live 1863).
- 11-03 12:20: live's new batch (56 recorded, paused 11:59 for request 1870): 35/53 at 11:40; since then the
  autofill rules are live's (an Enter that completes the text only inserts it, the next applies, the next accepts;
  an Enter that changes nothing applies; invalid values are no undo step): all 10 autofill / number-field scenarios
  match. The rest of that batch: sketch (note in skasm.md; sketch-slot-on-line settled as skasm's: our Slot tool
  makes the slot on the click, live's selects the line and makes it on Enter) and feature-row-copy-paste (settled
  12:40 in my own live tab: Onshape copies / pastes nothing for feature rows; ours matches now).
- 11-03 09:10: also landed (the autofill part corrected 12:20, above): Ctrl+click on a feature row opens its
  menu and a click beside an open menu only closes it; flip buttons, toolbar carets and row edges as driver targets;
  the Hole's icon rows and table levels in the state. Onshape's shortcut page (cad.onshape.com/help, Keyboard
  shortcuts) confirms Ctrl+C / Ctrl+V copy / paste features (kept, though live's CDP run pasted nothing), ↑ / ↓ only
  with the bar selected, no Enter-to-edit. Live re-records all 186 scenarios into /root/parity/ui/live (~2 min each,
  started 08:10); compare with `--live /root/parity/ui/live` as it fills, live-x40 for the rest.
- 11-03 07:50: **74/107** (live re-records and drops scenarios as its runner changes; run after every batch). Fixed
  since 05:00 (all landed): the sketch plane bubble stays; cancelling an unchanged existing sketch shows no toast; a
  part row double-click renames only on the name; dialog focus rules (tabs, labelled selects, a new active field takes
  the keyboard, Sketch dialog takes none); folded groups left out of the state. The rest is with live (REQUESTS 11-03
  03:50, 05:40, 07:10): remembered Hole / Chamfer / Fillet values in its account, plane clicks missing in sketch-*,
  key delivery after row clicks, selection kind / owner changes, parts list during a preview, ✓ with uncommitted text.
- 11-03 05:00: 57/101 before the Sketch-dialog focus and "Select a sketch plane" fixes (rerun pending). Landed
  48ed5ca2: overlapping datum planes pick the nearest (planes write a far-scaled depth in the pick pass). Open with
  live (REQUESTS 11-03 03:50): neutral starts (account's remembered Fillet radius 3 mm), select list kind / disabled
  state, mod+click, consumed sketch shown after Roll to here (user-hidden case), selection after New folder; the
  4 key-delivery artifacts (arrow-up, Enter, copy/paste, mod-click) are live-runner issues.
- Open: whole-sketch picks (Onshape's sketch-row pick persists as "Faces of Sketch 1" and follows new regions; ours
  stores the regions): model gap, see Requests to the lead. Targets not resolvable yet: a chip's ×, a row's eye, view
  cube faces, tools only in a toolbar dropdown, "Section view" (not a toolbar tool in ours).
**11-02 23:30 (pinned account): viewport parity 3 landed** (the WIP of 23:00): the assembly instance list no longer
observes the viewport (prepaint per orbit frame 230 -> 7.5 ms on pins-assembly-1000; `a.list_render` perf timer kept);
pick bench timeouts (10 s per click, logged); Reflection analysis as Onshape's shader 4663 (audit-viewport V8): world
reflection of Onshape's eye point (renderer `onshape_near_far`, also orthographic), lit as lightFragment(env, env)
(`light_fragment` in shaders.wgsl, ambient 1 / diffuse 0.5; `shade` is the same with 0.5 / 0.5); Lock environment
keeps the camera at the lock (`Reflection.lock: Option<Camera>`, ui `env_lock`, cleared when the analysis is off; the
renderer puts its eye in the instance normal matrix's w column). Also the flaky UI test name clash p-planes
(p_toggles_every_plane now "p-every-plane").
Then the Part Studio's feature and parts lists stopped observing the viewport too (neither reads its state):
parts-1000 orbit (`BENCH=orbit pickrun.sh`, llvmpipe) GPUI prepaint 194 -> 6.4 ms mean per window render.

Next after the WIP, in order: read the CI pick numbers and the GPU breakdown run; profile the NIST import's GPU time
toward 120 fps (now 62-80 at 2x); first frame and orbit fps on the 2000-instance assembly (~/psui-bigasm.noshape on
vibe5) and NIST (~/psui-nistxt.noshape); if selection is still slow on big assemblies once the list is fixed,
virtualize the instance list (rows are ROW_H high, the mates block would need one item per row); the sheet metal flat
and triad comparisons; then report before / after numbers.

11-02 night (user: "the viewport against live, looks and speed"): docs/parity/audit-viewport.md. Method: the same
B-rep both sides (our live-30 / knurl exported as x_t into live's "noshape mm test"; live's NIST x_t and 2000-instance
STEP imported into ours, ~/psui-nistxt.noshape and ~/psui-bigasm.noshape on vibe5), live's `#canvas` at 1673 x 886
(/root/psui-scratch/live/vp.mjs, modes.mjs) against `noshape-cli render image` at that size (/root/psui-scratch/vpr.sh,
vpm.sh), `compare -fuzz 6%`. Landed V1-V5 (Onshape's SSAO port, Translucent front faces at 25 %, hidden edges faded
85 % and only where hidden, render_image framing, Graphics preferences tooltips). Pending: V6 (assembly / named view
framing), the A30 fix (a cylinder's section plane contains its reference direction), the pick benchmark
(`NOSHAPE_PERF_BENCH=pick`, bench.sh / analyze.py). Speed: perf.yml on main says NIST at 2x orbit 62-80 fps, GPU p95
20-32 ms (the v0.4.21 "regression" is runner noise: no render change in v0.4.20..v0.4.21, v0.4.20 alone 56 and 80 fps);
breakdown run 41527719581 (variants noao / nolines / nosil / msaa1 / nomesh on 6db7d596).

11-02 evening (user: "the audit's remaining items, then Render / appearance, Section view, Measure, menus"): all
landed or queued, audit-part-studio A26-A30 and M11/M12/M15. Measure: every type against live 1709 (14 pairs; pair
rules in model-core measure.rs, one type filter measure::row_type for both studios). Section view: several planes
remove what any plane removes (shader `clipped` / caps, render offscreen test), plane numbers kept, every plane's
manipulator live, cylinder / cone through the axis (ops studio::section_frame_of_ref). Appearance editor: keystroke
filters, Enter accepts, Tab order (app-core AppearanceNextField). Manipulators from native evaluators through
EvalCtx::manipulators with std's keys, a drag running std's change function (std_feature LOGIC_ONLY: change function
only; STD_EDITING_LOGIC decides editing logic): Fit spline, Ruled surface, Wrap, Frame, Gusset, End cap, Decal, Derived
(Derived's click set natively: std's rebuilds the source studio). Points manipulators drawn (mod.rs std_points*, 7 px
dots as live's Frame). Left on core (in the audit M12 row): Offset curve on face trims (Offset and extend scope),
Bridging curve (std's control points), Edit curve (no triad FeatureManipulator), Loft connection sliders
(connectionEdgeParameters). wait_until in UI tests now waits 30 s (dx 11-02: a_shelled_face_... failures were all
from before its 11-01 fix). Render studio is cloud (left out by AGENTS).

11-02: queued e75450f5 (Variable table pass, A24; its MENUS line for "Collapse all tables" came in the next commit:
e75450f5 alone fails ops coverage), the planes P1 part 1 (FS host Ent::PlaneBody) and part 2 (A25: a feature's
planes as `Reference::ConstructionPlane`, listed under it with eyes; live 11-02 on "psui planes 1102 ps" in the mm
test doc). Mesher (queued after them): B-spline edges marched as GCPnts does instead of halved and merged (NIST 1.11M -> 989k
triangles against BRepMesh's 909k, regeneration 343 ms against 616 ms with BRepMesh; knurl 50k against 64k; the
4096 px NIST render within 10% but for 598 px of tangent-edge dashes).
11-02 later: the user's yribs.fs passes on a cylinder (T1); the small differences closed (plane rows grey under the
hover, no plane names, a plane's own state as live, shared widths in Variable Studio tables, no selection on a row
drag). Next: back to the audit (M11 rest, M12, M15 points: P3).

11-01 22:25: landed bf64da4e (B-spline faces in our mesher). Queued 42c306d8 (audit 12: Helix Turns and pitch flip,
interaction::helix_manipulators), c92c5408 (audit 12: Move curve boundary's arrow, interaction::
move_curve_boundary_anchor), 10a94ee6 (audit 12: a Shell preview fills the removed face khaki, as live 11-01; and
a_shelled_face_is_highlighted_as_before made to wait for the regenerations: it raced them and failed on main).
Next: M11 / M12 (P3).

(nothing unlanded) Landed 10-29: 60ca6465 MC-SELECT-NO-READOUT; 6f411558 / edf5dda5 / 5f3d7b20 mesher steps 5–5c
(N06: fins-400's display mesh ≥ 10× BRepMesh's on GitHub's Linux and macOS runners, live-30's 3.3–6×);
32d87597 / 5a695829 / 3cb447c2 perf.yml "Display mesher (release)" step (model job; per run mesher-N.log in the
model-perf artifact; macOS prints its mesh steps summed); 8b2c53ae HOLE-TABLE-BUTTON (verified by qa 16:04);
bc1ce2e2 PREFS-SELECT-LIST-DETACH (os_menu: a native `<select>` list closes on a wheel outside it).
MIRROR-DIALOG-STATE closed (live's answer 1597 = ours). Open in psui's TICKETS: ROW-TOOLTIP-WIDE-PANEL (P2, waiting for qa's pointer
path); API-VAR-REDEFINE (P3, the agent API's variable_create + model-data's variable_table::create: dx / core's
code, not a UI change: left for them).
18:20: d5ae3ded mesher 5d (CDT cache), cc73aaea display off the critical path (N06).
10-30 12:50: 65559a25 CUSTOM-ERROR-HIDDEN (P1, user via lead 11:20): with a new feature's dialog open its row
(the editing placeholder) had no tooltip and the FeatureScript notices panel walked only the document's committed
features; now the placeholder has the rows' error tooltip and the panel lists the open dialog's feature
(custom::run::set_draft, set in apply_display). Committed failing features already showed both. Verified on :102
with a tiny "No-op cut" Feature Studio (/root/psui-scratch/cfail/, built by noshape-cli): don't open the knurl
document in the app (16.7 GB, earlyoom; core's P0).
10-30 08:15: COMMIT-LATENCY (P0, core asked psui to confirm on a perf run): perf 41005801257 had commit within
budget (medians 149 / 122 ms) but undo 200–364 ms in 6 of 9 runs with a 2 ms regen; the regen's 12 ms coalescing
timer fired late (locally ~180 ms). d21f7678: the wait only for bursts → perf 41014326054 undo median 36 ms
(both macOS), redo ~30, commit ~100. Two slow outliers per OS remain (250–350 ms between scheduling and the
result); 62d49da8 logs whether that's the thread start or the lock (`regen #n: started … lock …`); perf
41020683102 dispatched: read it, then close COMMIT-LATENCY or chase the outliers.
10-30 10:50: perf 41036130143 (4b4e2b4a, logs "polled"): the slow undo / redo (166–462 ms, 3–4 of 18 per OS)
are the UI thread first running the spawned regen task 170–430 ms late (regen 1 ms, lock 0, nothing else on the
UI thread): App Nap on the runner's background window. This commit holds gpui's prevent_app_nap activity for the
regen task (schedule → shown). Next: dispatch perf (jobs=bench, live-30, NOSHAPE_PERF_SCRIPT=edit-undo-redo) on it;
if the outliers are gone close COMMIT-LATENCY (commit / undo / redo all within 200 ms), else look at the run loop.
15:10: perf 41058182056 on c383c1e8: no change (one slow step per process, at a varying step: a one-off on the
UI thread). Next commit logs the stages per step and long deferred-pipeline waits; perf on main (ref=main: a short
SHA ref fails the checkout) then read "stages since the last step" / "waited for a deferred pipeline".
16:40: perf 41073528532: the slow steps are the UI thread blocked in gpui.present while the GPU takes 250–500 ms
per frame for the first frames after opening (not pipelines, not App Nap). Next commit: NOSHAPE_PERF_SCRIPT_WAIT_MS;
perf with env NOSHAPE_PERF_SCRIPT=edit-undo-redo,NOSHAPE_PERF_SCRIPT_WAIT_MS=2000. If clean: COMMIT-LATENCY is met
(commit / undo / redo medians ≤ 120 ms), the outliers are the runner GPU's warm-up; propose closing to the lead.
17:30: perf 41092110782 (e10b29fd, WAIT_MS=2000): all steps ≤ 164 ms (medians commit 75 / 74, undo 57 / 28, redo
19 / 33): COMMIT-LATENCY closed in TICKETS. Left open (not a ticket): the macOS runners' first viewport frames after
opening take 250–500 ms on the GPU (paravirtual Metal); if real Macs show it, look at GPU-side first use. Mesher
numbers: use tools/tiers/perf.sh (vibe3, machine to itself) once it's back on main (the lead reverted it at eb85b1ec).
10-30 23:50: mesher on the perf server (tools/tiers/perf.sh, `rust_mesh::bench_f`): the first real numbers were
8.2–8.7× on the fin plate (this machine's / CI's 11–13× were optimistic), and runs before c68adf77 timed other trees
(perf-remote.sh checked out before its lock; fixed in c68adf77, request below). Now f94a0486: **fin plate 12.2–13.5 ms
(median 12.9) against BRepMesh 133–135: 10.0–10.8×, median 10.4× over 5 runs**; filleted boss 1.80–1.85 ms against
2.61–2.80 (1.4–1.6×). Steps (`rust_mesh::bench_profile_fin`, per copy): brep's mesh 5.3, box 1.6 (faces in parallel,
a solid skips the outside-edge walks), reading in 1.8, out 0.7 (polylines in parallel), edge cache 0.55 (16 maps
bound in parallel), fill 0.7. Next if more is wanted: the box's per-face BRepBndLib (1.5), brep's per-face Vecs.
10-31 05:30: live-30 on the perf server (new benches: ops `regen_profile::bench_live30_display_mesh`, cold = scaled
copies so brep's CDT cache has none, as on open, and warm; `bench_profile_live30_display_mesh` by step, the profile
switched on after the regeneration with `set_kernel_profile`). b947eff0: **cold 3.2–3.5 ms against BRepMesh 11.2–11.7:
3.2–3.7×** (was 2.9–3.2× before the parallel output copy), warm 3.0–3.3 against 10.8–11.4: 3.5–3.8×; fin plate still
10.4–10.6×. Per cold mesh: brep's faces 1.25 ms (the tori), its output copy 0.63, mesh_fill 0.77, edges 0.2, box 0.2.
10× would be ~1.1 ms in all, below our face meshing alone: live-30 stays ~3.5× short of a faster strip/torus mesher
(the filleted boss is 1.4× BRepMesh: 1.85 against 2.64 ms). Not pursued further unless the lead asks.
10-31 07:00–09:30: lead's parity sweep (core-workflow, chrome sweep, part-studio, keyboard-mouse, interaction, large
models, featurescript, configurations, variable studios, dark mode). Nearly every open row was a stale log row that a
later pass had fixed; marked so with the evidence (9b1b9b47). Re-checked in the app: KB1, KB3, FS1/FS2/FS4/FS5, ADV3,
VC8. Fixed: ME8 (assembly Measure panel as the Part Studio's, opened on the selection). Fonts (VC7/RP7/MP3) accepted
(Flama is commercial). Open: core-workflow (d) parts-1000 orbit fps (perf 41179398098 running), SM4i / SM4k (skasm).
10-31 11:30: **selection-field auto-advance** (user priority; live request 1650, behaviours "psui (2026-10-31, PRIORITY)
selection-field auto-advance", dom/advance-all-modes.txt; Onshape's bundle autoAdvanceToNextControl / processItemAdd):
FeatureDialog::add_picks (every Part Studio dialog: built-in, sheet metal, surfacing, custom FeatureScript): a field
reaching its MaxNumberOfPicks passes the focus to the next visible query field (non-query controls skipped, a filled
one still takes it, array items within their item); a full field replaces / drops its oldest and advances again;
the last keeps the focus (Sketch lets go); × on a chip focuses its field; after a removal the refill doesn't advance
until the user clicks another field and back (advance_disarmed / user_focus_query); a box selection is one edit;
a preselection filling a limit moves on. Assembly: the mate dialog by fields (Tangent ≤1 ⇒ ≤1, Width tab ≤2 ⇒ width
≤2, others one ≤2 field that keeps the focus, FIFO), the assembly mate connector (Origin ⇒ Between ⇒ axes, last
keeps), Pattern Direction ⇒ second Direction. Limits: T1 query_pick_limits_follow_std (std's 112 MaxNumberOfPicks;
Variable's measure-between was unlimited, now 2); chains in our order = live's table except fields we lack (Fillet
variable Vertices, Routing curve hidden references, Tag base plane: their owners). T1: mate connector, Bend, knurl
(rule 6 included), Tangent, Fastened FIFO. In the app (:102, small knurl doc): three clicks, no manual focusing.
10-31 10:38: landed f6b3af7d (947b7c66 / cce0086e / 2821d5ba auto-advance + 8.x renames, 959ce1d9 dark chrome: Create
#769fcc while its menu is open (os_ui::dropdown_open), wordmark white, the right strip's open button #333). Then (live
request 1663): one Esc cancels every appearance editor, also typing in its fields ("AppearanceEditor > Input" binding,
as FeatureDialog's); Add appearance to feature… opens "Face appearance N" with Faces or features = the feature, black,
the focus on the body (`appearance_heading`); a sketch's editor starts at its #999; "Dependencies of Origin" lists a
sketch constrained to the origin (model-core `features_using`: a sketch's external_refs and its origin point).
Checked on :102 (blk). Residual: live shows the dependent row selected (light blue, also in the feature list); ours
plain. core-workflow section 8: every row checked; no unchecked rows left in the sheet (2.21 / F2 / F16 are notes).
10-31 10:58: dark-mode sheet: dk-914 / 922 / 923 / 941 / 942 checked (= live); the assembly's right strip as the Part
Studio's (skasm's assembly_view.rs render_panel_rail: open button joins its panel, others strip_inactive; was a blue
row with an accent bar; live assembly/181, dk-942). dk-915 (Create #769fcc with a #3e94ff ring) asked live (REQUESTS
psui 10-31 10:52). Left in dark-mode: dk-990 (restored light), dk-961 not captured.
10-31 11:29: landed 59380fbb Ctrl+Space tab switcher (live spec: card while Ctrl is held, MRU thumbnails, release
switches; `switcher_next`, TabThumbs queues). SM4k: the flat view outlines only the hovered wall / bend zone
(model-core flat_view_display names each edge; the assembly's pasted flat keeps its edges naming the part). Asked
live: Shift+H / p semantics (REQUESTS psui 10-31 11:14; ours toggles a global show flag). Next: SM4i (Other joints
Type ▾ Bend / Rip, Style ▾ Edge joint / Butt joint - Direction 1 / 2 → a Modify joint on the rip's wall-end edge).
10-31 12:01: SM4i: Other joints Type ▾ / Style ▾ as live (ops::sheetmetal::edit_rip, API op sheetmetal_edit_joint, T1 e2e
sheet_metal_other_joint_ops); select lists now grow to their longest option (os_menu, Chrome's lists). Under std a
rip a Modify joint made has no `faces` (custom host sheetmetal.rs `named(Def::Edge(e))` empty); edit_rip finds its
Modify joint through the rip's joint id instead (see Requests). Asked skasm via lead: the flake below.
10-31 13:03: landed ff5594ef (SM4i). MODAL-HELP-MARK: os_ui::modal_help ("?" before ✕) on Export Drawing and Export as
DXF/DWG (live drawing/90).
10-31 13:40: landed dbb094e0 (modal "?"). Dark dk-932: an edited / hovered / selected sketch dimension in the dark
palette (scene sketch_mode used the light one: black lines on #333), the dimension box's outline #ccc and ✕ #bd3039
as live, focused-field text selection #0a3ea8 in dark (app main.rs PlainSelectionColors per theme). Origin
dependencies "selected row" residual: live's capture was a hover (tooltip shown), ours does the same: closed.
Asked live 13:03: the feature list's multi-selection menu with a sketch in it (core-workflow 8.3).
10-31 14:12: landed 6a4b510f (dark dimensions). Shift+H / p as live (request 1673): per-item visibility with live's
tie-breaks (sketches: any hidden → show all; planes: any shown → hide all), not a global draw flag. Next: dk-915
answer (Create's #769fcc + #3e94ff ring is its keyboard focus, kept after Esc until Tab / a click elsewhere).
10-31 14:36: landed 18e2a3ad (Shift+H / p). dk-915: the documents page's Create keeps live's focus look (#769fcc + 2 px
#3e94ff ring) while its menu is open and after Esc, until Tab or a press elsewhere (start_window create_focus; the
button is a styled div now).
10-31 15:07: landed 53e1eaaf (Create focus). dk-961 compared (= live but the type ▾ 2 px narrower). API-VAR-REDEFINE fixed.
10-31 15:47: landed 2c19808a (API-VAR-REDEFINE). Feature list multi-selection menu by the selection's mix (request 1675).
10-31 16:22: landed 5132559a. Enter / Space on a focused dropdown button opens its menu (Bootstrap; os_ui OpenDropdown in
the "OsDropdown" context, so it wins over a window's Enter): the documents page's Create after Esc, as live.
10-31 16:41: landed badf80bd. dk-990: after Save theme the view cube (faces, frames, X/Y/Z letters) follows at once
(viewport observes GraphicsGlobal). Dark-mode sheet: every row checked.
10-31 16:58: landed 3c365d12 (Hole level names #999 in dark). Dark sheet now lists dk-971…9a1 (Part Studio captures
not in it before); checked dk-974, 977, 978, 979, 980, 981; open: 971–973 (sheet metal), 975, 976, 982, tab manager,
panels 992–999, history.
10-31 17:10: context menus that don't fit under the pointer open above it when there's more room there, clamped to the
window (live dk-974 / 975); before they shifted up just enough. Vendor gpui-pre Anchored::switch_anchor_or_snap
(NOSHAPE PATCH), used by the window menu host, the Part Studio canvas menu and skasm's assembly canvas menu.
10-31 17:56: landed d8c1b80c. Dark sheet: also checked dk-975, 976 (sketch menu icons added), 992, 995, 999 (Properties'
Reset all disabled until something differs). Open in dark-mode: 971–973 / 982 (sheet metal), 994 (custom table),
tab manager 989–991, left panels 996–998, history 9a0–9a1.
10-31 20:12: landed a2c5b09f (flat view fit). Flange dialog labels as live (skasm's sheetmetal.rs spec SHOW_LABEL).
10-31 21:43: **P1 from the user: instant cancel.** Cancelling a dialog while its preview regenerates (the knurl at
45 / 45°) left the spinner going: cancel_edit scheduled a regeneration of the restored document, which waited on the
regenerator lock behind the cancelled run. Now the committed document's result (display, context, the studio /
upto / config it came from) is kept when a dialog opens (`before_edit`, only when no regeneration is pending); on
cancel, if the document is unchanged, it is shown in the same update, `regen_seq` bumps (late results dropped),
the runs are aborted (`Runs::abort_all`, model-core) and the spinner ends. T1 sketch_ui_tests
cancel_shows_the_document_before_the_edit_at_once. Asked live (REQUESTS psui 10-31 21:24) for Onshape's toast /
list / dialog behaviour on cancel; behaviours 4227 already says Esc doesn't cancel a committed regeneration and there
is no cancel button.
11-01 00:08: landed 66cf2194 (instant cancel, user P1). Live (behaviours request 1688): ✕ / Esc close the dialog at once,
no toast, the old geometry stays drawn and input works, but Onshape's spinner runs until the model has rebuilt the old
values (~18 s on the knurl) and a committed regeneration can't be cancelled at all (Esc does nothing, no Stop button).
Ours now: dialog, list, geometry and spinner back in the same update (the user's request beats live's slower spinner).
Gap closed 11-01 00:58: the view keeps the last finished result of the committed document (`last_done`, recorded
whenever a result is shown with no dialog open); a cancel always shows it at once and drops both runs' late
results; when an earlier edit was still regenerating, its document then regenerates afresh behind it. T1
cancel_during_an_earlier_regeneration_shows_the_last_result. Checked on :102 with kn.noshape
(Diamond knurl 45 / 45): spinner and dialog gone in the first frame after Esc.
10-30 05:05: v0.4.15 (58810bd9) carries all of it (silhouette cull on, mesher 5–5d, display off the critical path,
MESH-EMPTY-LOOP-PANIC); qa's 04:56 list has no blocker in psui's area.
10-30 01:50: 6ca3c756 edge-silhouette GPU cull on by default (N-Retina in 0c2 below); bca7f3d1 MESH-EMPTY-LOOP-PANIC
(P1, qa 10-29 18:51: an edgeless loop is declined, a mesher panic falls back to BRepMesh at the bridge).

Residuals found today: a paused dialog's Depth field
shows a corner mark in live; Shift+wheel under xdotool loses events after the first (GPUI X11 resets the scroll
valuator on XI2 DeviceChanged from XTest's slave switches; real wheels fine); the banner's text is 5 px narrower
than live's Flama.

## TAB-NO-PREVIEW: closed (test-display artifact)

On Xvfb without a window manager the app window never gets X input focus, so GPUI treats it as inactive and sends no
focus / blur events; nothing blurs, so nothing commits. **Always `xdotool windowfocus <window>` after starting the app
on :102** (W=$(xdotool search --name noshape | head -1)). With focus, Tab commits and previews as live. Fixed alongside:
a Tab commit scheduled two regenerations.

## Local tests (lead 10-29): `export NOSHAPE_CONFIG_DIR=$(mktemp -d -t noshape-config-XXXXXX)` for `cargo test`
(UI tests otherwise read / write the shared ~/.config/noshape); the app itself runs with the normal settings.

## Landing (10-27 13:45, lead): only with `tools/tiers/land.sh`

It rebases, runs clippy + the full tests of every touched crate and its dependents (+ T0 for app / ops / cli) and
pushes only if green.

## Landing checklist addition (10-27)

A new or renamed menu label needs its crates/ops/src/coverage.rs MENUS line: run
`cargo test -p noshape-ops --test all coverage::` with clippy / T0 (79460041's "Cancel" turned main red; skasm fixed
it in 25dc002f). /root/psui-scratch/check22.sh runs it now.

## Build times (10-27, load ~30)

App edit → `cargo build -p noshape`: 60–78 s. A vendor gpui-base / gpui-pre-linux edit → app: ~3.5 min. After a rebase
that touches the model: ~12 min.

## Next, in order

- (11-08 13:55, lead from studios) S-TOOLBAR-OVER-TABS: the S shortcut toolbar opens when S is typed with the pointer
  over the tab bar; live opens it only over the graphics (ui-parity doc-tab-rename-escape). After EXPORT-MESH-FOLD.

N000000000000000. (11-06 21:49) **Display cache (lead 11-06: open like Onshape, instantly).** part_studio/display_cache.rs
    (key: blake3 of the whole document's JSON, the tab, configuration values, View in high quality, the build: version,
    the executable's size and time, `noshape_kernel::OCCT_PATCHES` from kernel's build.rs; files
    `paths::cache_dir()/displays/<key>.display` = magic + zstd(bincode(StudioDisplay)), written beside and renamed in,
    least recently used evicted past 1 GiB; feature states, decals, variables, in-context bodies and constraint glyphs
    aren't kept, shared pattern meshes re-linked on load) and part_studio/cached_open.rs (looked up on the studio's first
    regeneration; shown with `show_cached_display` unless the real one came first; `cached_view` until the first real
    display: "Regenerating…" with the spinner at the top of the feature list, `start_feature` / `edit_feature` deferred
    with the message WAITING and run when it lands; the first full display after opening saved at once, later ones 2 s
    after the last full regeneration with no dialog; the coarse (preview) display kept beside it as `<key>-coarse` and
    shown first, the refined one over it when read: a refined cf-fill-pattern took 2.5 s to read and draw). Enabled by the app only (`cached_open::enable`, never under the
    ui-parity driver or in tests; NOSHAPE_CACHE_DIR moves it). GPUI test
    `a_cached_display_stands_in_until_the_identical_real_one_lands`. The opened studio's displays build their scenes off
    the UI thread until its first full one (`first_full_shown`). bench.sh OPEN_DOCS on the macOS perf jobs.

N00000000000000. (11-06 19:58) **Coverage complete for the toolbar and sketch tools; live's batches 5-9 in.** Batches 13
    (the toolbar's last 29) and 14 (15 menu items) filed with live. Menus left (25/117): assembly in-context (Go to
    assembly, Current assembly position…), releases, MBD annotations, the appearance panel's rows, a configured
    dimension's Unconfigure, Paste inputs. From live's recordings today: see SCORE 11-06 evening; open questions in
    REQUESTS 19:27. New: GROUP_OPEN hint (std "Collapsed By Default": false), `selection_feature_menu` (the
    multi-selection feature menu, also a fresh folder's), recorder entries for the Section view panel / appearance
    editor / parts list headings / open submenus.

N0000000000000. (11-06 18:06) **Remembered feature values and batches 10-12 (lead 11-06).** Hole memory is ours as live
    found it (1987: on ✓, per account and type, every document, lengths per document unit; `feature_settings`). The
    suite's two Hole differences were live's account state: a scenario's `"remembered"` (README) seeds the run's config
    (main.rs keeps the memory under the driver when NOSHAPE_UI_REMEMBERED is set); the seeds in hole-metric-tab /
    hole-countersink-tab are inferred from live's dialogs, asked live to confirm (REQUESTS 17:28). A real bug it showed:
    std's holeEditLogic copies the hidden V2 sizes back on a table switch and ours had none (custom
    `hole_v2_synced`). Batches 10 (camera menu, sketch dimension menu), 11 (configurations: targets config_more /
    config_row) and 12 (shortcuts, last sketch tools) filed with live. Coverage: toolbar 47/76, sketch tools 55/55,
    menus 78/117, shortcuts 78/89 (the rest of the shortcuts are tabs, windows, assembly and mate keys). Next: the
    toolbar's remaining 29 (surfaces, curves, sheet metal, frames), menus' remaining 39 (appearance panel rows,
    folders' Rename, expressions on number fields), and live's recordings as they arrive.

N000000000000. (11-06 15:23) **ui-parity coverage and live's batches (lead 11-06: menus are the biggest gap).** Batches
    6-9 (58 scenarios: menus, then the toolbar's uncovered tools) are on main and filed in live/REQUESTS.md (12:39,
    12:55, 13:18, 13:22). Coverage: toolbar 47/76, sketch tools 43/55, menus 62/117, shortcuts 59/89. Live's first
    recordings of batches 5-7 sit in /root/parity/ui/live (not on main yet): compare them with `run.py --live <dir>`
    (copy them to vibe5 first); 23/34 -> see SCORE 11-06 14:47, and the Export as DXF/DWG modal since (flat_export.rs
    is now the modal for flats, sketches and planar faces: EXPORT-DXF-DIALOG, the face's file name is a guess). Left
    from them: account memory (Hole), live runner artefacts (menu pick by prefix: REQUESTS 14:30), Mirror's Face
    mirror focus (asked live). New targets: `{"feature": "Top"}` (a default plane row), `{"feature": "New folder"}`
    (a folder row, also "New folder (2)"), a submenu by `{"do": "hover", "on": {"menu": "Collapse"}}`. The compare
    drops paste rows in scenarios that copy nothing (live's session clipboard). Starts name features as they make them
    (a default name stays default: a paste's next name is "Sketch 2", as live). Menus left: the camera menu (its
    button has no tooltip mark yet), a sketch dimension's menu (no target for a dimension yet), configurations.
    Open: dialog-number-rclick-menu (live: a right-click on a number field gives it the keyboard, text selected;
    ours focused it from a capture handler on the row and stopped the right release, yet the focus was gone when the
    state was recorded and back after Esc: something else holds it, reverted, not found yet).
    Writer scripts: assert no file exists before writing (batch 5 overwrote mirror-feature-type once).

N00000000000. (11-06) **perf.yml's macOS gate (lead P1):** 42162655806 (v0.4.32) failed on macOS 15's baseline
    comparison only, no regression: the same build's rows move 20-100 % between runs there (readback GPU p95 7.2-15.5,
    1x texture 6.6-9.6, 2x fps 65-82; readback UI CPU p95 2.9-7.0 over v0.4.23-v0.4.32 with no trend) and macOS 26
    had every row at or under macOS 15's baseline. Faults: (1) the baseline was the last *passed* run, so the luckiest
    run set the bar (now each metric's median over the last five default-settings runs, passed or failed: Linux,
    Windows, and macOS when no earlier release); (2) one run decided (now a flagged row is measured again, analyze.py
    --confirm); (3) both still compared across jobs, and a job's host and its slowing down through the job decide
    readback UI CPU and 1x fps: 42171045580 measured v0.4.32 against three runs of v0.4.32 and failed, again on
    re-measuring (UI CPU p95 4.33 -> 6.57 -> 6.99; 1x zoom fps 26.3 -> 21.0 -> 19.5). So the macOS jobs measure the
    previous release beside the build, row by row, alternating which goes first (bench.sh AB_BIN / AB_OUT), and judge
    regressions against it, re-measuring both on a flag (--confirm-baseline). Thresholds unchanged (10 %, MIN_CHANGE).

N0000000000. (11-06 05:42) **The first-frame stall on a real Mac (lead 11-06: the user's Mac mini runs these).** CI saw
    the bench's first frame after an idle cost 25-30 ms at 1x and 90-320 ms at 2x on the runners' virtual GPU (above).
    On a real Mac, from a checkout of the repo (Terminal; Python 3 with Pillow for the stills, `pip3 install pillow`):
    1. Install noshape v0.4.33 or later (the first with the first-frames report: the release's macOS .dmg into
       /Applications), or build main (`cargo build --release -p
       noshape`, with OCCT as in docs). `BIN=/Applications/noshape.app/Contents/MacOS/noshape` (or target/release/noshape).
    2. Keep the Mac awake and leave it alone during the runs (each launch waits 60 s, then measures 300 frames; the
       whole set takes about 15 minutes; screencapture may ask for Screen Recording permission once: allow it):
       `caffeinate -d -i -- env DELAY=60 VARIANTS="retina@orbit:NOSHAPE_PERF_SCALE=2,NOSHAPE_PERF_BENCH_DRIVE=frames
       retina@pan:NOSHAPE_PERF_SCALE=2,NOSHAPE_PERF_BENCH_DRIVE=frames retina@zoom:NOSHAPE_PERF_SCALE=2,NOSHAPE_PERF_BENCH_DRIVE=frames"
       .github/perf/bench.sh "$BIN" tests/perf/nist-mtc-crada-assembly.noshape ~/noshape-perf texture readback`
    3. `python3 .github/perf/analyze.py ~/noshape-perf "Mac mini" --stills > ~/noshape-perf/report.md`, then send
       report.md and the ~/noshape-perf folder (its logs have each first frame: `grep "BENCH FIRST" ~/noshape-perf/*.log`).
    Reading it: the "First frames" table's GPU column against the rows' GPU mean. About a steady frame (under ~16 ms
    at 2x): the stall is the runners' and nothing is to fix; 50 ms or more: it's real, and `BENCH FIRST FRAMES` shows
    which pass pays it (r.pass.color / sil-cull / overlay, r.prep, r.encode_submit).
N000000000. (11-06 05:25) **First-frames hitch (lead 11-06): not ours, found on the runners' GPU.** The bench gates the
    steady state (first 30 frames left out); their max per frame is its own row ("First frames", analyze.py;
    `--gate-first-frames-gpu MS` ready, not on). Four diagnosis runs (perf/psui-firstframes: 42137141138, 42145731291,
    42154493047, 42161346296; `BENCH FIRST FRAMES` per frame): only the bench's first frame (sometimes the second) is
    slow, every pass at once (color 25-30 ms at 1x, 90-320 ms at 2x: in proportion to the render size, steady 5 ms at
    both). Not the move (still frames first: same), not the interactive style (NOSHAPE_PERF_DETAIL=full: same), not the
    sil-cull (off: same), no pipeline built and no target / present texture / readback buffer made then, no page reads
    (page faults per frame logged), not idle length past seconds (11 s idle as bad as 58 s) but not after 300 ms (the
    pick bench's frames: p50 8.3 ms), and a Metal residency set holding every buffer and texture changed nothing. So:
    the first GPU frame after seconds of idle on the runners' paravirtualized GPU. A real Mac can't be checked here; in
    use the pointer's hover picks draw before a drag. The gate stays off.

N00000000. (11-06 00:20) **perf 42117840554 (main@aadc61e, first macOS 26 run):** macOS 26 green, no regression; NIST 2×
    orbit / pan / zoom GPU mean 6.86 / 6.79 / 6.19 ms (p95 ≤ 8.01), the 120 fps target met; parts-1000 and
    pins-assembly-1000 ≤ 6.4 ms. macOS 15 failed two false regressions, readback pan / zoom GPU p95 7.20 -> 11.16 /
    7.57 -> 11.32: the whole-run p95 holds the first windows' runner stalls (single frames of 67 and 107 ms GPU), the last
    window's p95 is 7.10 / 7.66 (= baseline); same code on macOS 26 7.21 / 7.05; macOS 15 readback p95 on main today
    swings 7.4-14.4 ms run to run. Lead's call whether analyze.py should drop warm-up windows or report readback rows only.
N0000000. (11-05 13:40) **Landed today, speed:** 5423c54a a closed solid's back faces culled in the color pass
    (live's solid faces are one-sided: webgl bundle `isTwoSided: false`, CULL_FACE BACK by default): NIST 2× orbit GPU
    mean -1 ms (macOS 14, 8.40 -> 7.45 avg) / -0.5 ms (macOS 15), perf 42015953579; byte-identical with edges and in
    hidden line, in plain Shaded only the back faces our depth offsets let through go (scene back_face_cull test).
    3ed86c09 the Regenerator a cancel starts afresh is seeded (Regenerator::snapshot, shared Arcs): cf-straight-spline
    edit after a cancel 17.4 s -> 8-17 ms. 7d4332c8 a cancelled dialog drops all its work (P1 Displacement map).
    655a65fa cf-hose-routing's hoses meshed here (reopen 7.2-9.2 s -> 1.3 s). Next: perf 42036796248 (main, default
    settings) confirms NIST ≤ 8.3 ms mean on both runners -> TARGET_GATE '--gate-target' in perf.yml.
N000000. (11-05 09:08) **Live mid-orbit (lead 11-05: motion-only cuts are parity only if live does them).** Live's
    webgl bundle (bundle-raw/webgl.*.js, the `detail` logger): while the camera moves (frameTimer interaction, 500 ms
    after the last move) live draws its interactive DetailSettings; they start equal to FULL and drop a step only when
    interactive frames average over 1000/DETAIL_REDUCTION_THRESHOLD_FPS (20) = 50 ms: edge silhouettes + section
    edges, then SSAO, then image-based (backface) silhouettes, then lowLevelBodyProportion +0.1 up to 0.9; back up a
    step under 1000/30 ms. Always while moving: extent culling 10 px (2 px at rest). AO off for interactive frames
    only on Mac with an AMD GPU or Apple GPU in Safari (`isInteractiveSsaoEnabled`); edge / backface silhouettes off
    only in Safari 15. Ours does all of this already (app-core viewport.rs `interactive_style`, navigation.rs
    DetailManager). Capture (live NIST Part Studio, SwiftShader, CDP screencast during a right-drag:
    onshape-research/live/psui/orbit-detail/, cast/drag-vs-rest.png): part edges, outlines and AO drawn while
    dragging; only the hover highlight is gone until release. So at our frame rates (~8 ms) live draws full detail
    while moving: no motion-only cut is parity. Savings must change nothing.
    **AO (exact-pixel candidates):** per frame the AO is three passes, ~0.63 ms each on the runners (occlusion and
    filter at half size, apply full size: about the same each, so per-pass cost), and the colour pass stores its 4×
    MSAA depth (2388×1588×4) only for the AO to read sample 0. A/B run 41999792837 (perf/psui-depthstore,
    NOSHAPE_PERF_SKIP=depthstore / ao+depthstore) measures the store's share; if it is large, keep only sample 0
    (Metal depth resolve, MTLMultisampleDepthResolveFilterSample0, via the vendored wgpu-hal).
N00000. (11-05 07:19) **Speed gates landed (lead's decision 11-05, 8a0cf0d9 with cf8d8a95's r.gpu_ms):** perf.yml's
    macOS rows gate PLAN 2.7's floor (`--gate-floor-gpu-p95 16.7`: GPU frame p95 over the scripted orbit at 2×) and
    report the NIST 120 fps target (`--target-gpu-mean 8.3`; set TARGET_GATE to `--gate-target` once the mean is
    under it, so it can't regress after); the big-model rows parts-1000 and pins-assembly-1000 run in their own step
    with the floor gate. A dispatch with its own `variants` has no `texture+retina` rows, so its floor gate says
    "no 2× run" (expected); a release before cf8d8a95 (v0.4.30) reports no GPU time, so the gate skips. **First
    check on main (run 41989113805, main@0a8ec1d built from source), 2× GPU mean / p95 ms:** NIST orbit 8.54 / 8.94
    (macOS 14), 8.26 / 9.51 (macOS 15), pan 7.4-7.75 / ≤ 9.71, zoom 6.3-6.6 / ≤ 7.85; parts-1000 orbit 7.19 / 9.14
    and 6.81 / 7.64; pins-assembly-1000 orbit 6.27 / 7.25 and 5.17 / 5.7. Floor met on every row; the 120 fps
    target met on macOS 15, 0.24 ms over on macOS 14. Its "regression" failures are the old baseline's (below).
    **Baseline fix (a2014f5e):** perf.yml compared every run with the last successful run on main, which was a
    frames-driven experiment branch (41933370375, perf/psui-layer2): every release run since failed on false
    regressions. Now only a release measured with the default settings writes `default-settings` into its artifact
    and only such a run is a baseline; none until the first such run succeeds.
    **Edge-line cut (lead: yes, only if pixel-identical): measured, not landed (no gain).** Branch
    perf/psui-edgecull2: a solid's edge segments whose two faces both turn away (5-bit normal cones + octahedral
    normals in the segment's spare colour / style bits, `vs_line_visible`) left out of the visible lines' pass. Exact
    only with a thin-wall guard: the depth offsets let a back edge through a wall thinner than a few pixels' depth,
    and its antialiased line blends into the front one (a 3 mm plate zoomed out 8×: 1 px). Guard: per segment the
    clearance to the solid's other faces (scene `edge_sides.rs`, grid + power-of-two checks) and a convex side's
    radius; the shader culls only where that is ≥ 8 px and covers the line's width on both sides. With it every view
    of crates/scene/tests/hidden_edge_cull.rs is byte-identical (fitted, 2×, zoomed out 8× and 30×, both
    projections). But the gain is noise: run 41977362854 (unguarded, i.e. the upper bound, interleaved A/B, NIST 2×
    orbit GPU mean) macOS 14 cull 8.47 / 9.24 vs off 9.35 / 8.96 ms, macOS 15 7.46 / 7.59 vs 7.54 / 7.33; colour
    pass p50 −0.3 / −0.1 ms. The culled segments still run their vertex work; the raster they skip was cheap. And
    the per-segment data costs CPU at every body upload: 0.27 s for NIST's 341k segments' sides, 1.1 s with the
    clearance. So not landing; the lines' remaining cost is vertex work per segment (6 vertices each).
N0000. (11-05 03:00) **Runner GPU per pass (run 41950133358, NIST 2388×1588 orbit, frames-driven, steady p50):**
    color 5.4 / 4.7 ms (macOS 14 / 15), AO 0.6 / 0.4, overlay ~0, sil-cull ~0; submit→done 12.6 / 11.2 ms. The empty
    frame (lines + meshes skipped): color 0.9 / 0.6, AO 0.3, but submit→done still 9.3 ms. So our GPU work per NIST
    frame is ~6 ms (under 8.3); the ~9 ms floor is the paravirtualized runner GPU's submission → completion latency,
    not our passes (the frames-driven bench waits for each frame, so it measures that latency). The real cost to cut
    is the color pass's meshes + lines (~4.4 ms). **Ablation (run 41957634951, r.pass.color p50, macOS 14 / 15):**
    full 5.7 / 5.4; no lines 4.3 / 3.9; no meshes 5.0 / 5.5; no edge silhouettes 6.1 / 5.8; no backface
    silhouettes 5.0 / 5.3; lines + meshes off 0.9 / 0.6 (run 41950133358). So lines are ~1.5 ms, and the rest is the
    ~670k triangles processed twice (backface-silhouette pass, then the faces): drop either and the other takes the
    depth work. No parity-safe quick cut found: the vertex format is 16 bytes; drawing the backface pass after the
    faces cost up to 37 % more on these runners (renderer.rs note). Options, each a look decision: the backface pass
    on a coarser display level; line segments cheaper at 4× MSAA. On a real GPU frames pipeline (2 in flight) and
    the runner's completion latency (the ~9 ms floor) isn't there.
N000. (11-05 01:50) **Per-pass GPU time on the runners**: `NOSHAPE_PERF_PASSES=1` (with NOSHAPE_PERF): the
    vendored wgpu-hal (metal::pass_timing) reads every labelled command buffer's GPUStartTime/GPUEndTime; wgpu-core
    records each render / compute pass in a command buffer of its own labelled with the pass (color, ao, sil-cull,
    hover-mask, …), so the renderer logs r.pass.<label> with no change to how it encodes. (A first try split the
    encoder and filtered on `noshape:` labels: wgpu labels pass buffers with the pass's label, so nothing matched,
    run 41942955242.) **UI side measured here (11-05 01:00, cf-straight-spline):** a dialog open's UI CPU p50
    23.5-24.9 ms after f57610ca; its frame is the dialog's own layout, the toolbar (re-rendered on purpose: the open
    feature's tool shows active, mod.rs ~3107) and the edited row; slices re-laid out ~6.6 % of the UI thread. Next
    there would be caching each toolbar button as its own view. Hover over the model and over list rows re-render
    no list (60 viewport hovers, 10 row hovers: only the overlay slice, 0.4 ms).
N00. (11-05 00:40) **Viewport Metal layer: measured, not landed (no gain).** Branch `psui-layer-keep` (and
    perf/psui-layer2) has it: NOSHAPE_VIEWPORT_LAYER=1 presents viewport frames to a CAMetalLayer in an NSView under
    GPUI's view (gpui::UnderlayHole hole sprites, gpui-pre-macos underlay_layer / place_underlays, app-core
    LayerPresenter blit). It works (CI screenshots on macOS 14 / 15 show the model through the hole; GPUI draws 0 ms per
    orbit frame) but doesn't pay: same build, first orbit at 2× frames-driven, GPUI path vs layer: NIST macOS 14
    77.6 vs 72.6 fps, macOS 15 67.0 vs 76.4 (noise); the empty frame (NOSHAPE_PERF_SKIP=lines+meshes) macOS 14 140.4 vs
    89.8 fps, macOS 15 118.2 vs 123.2 (runs 41923148042 / 41932160816 / 41933357955 / 41933370375). So GPUI's present
    isn't the floor; the empty frame is ~6-7 ms of our own fixed passes at 2388×1588 and NIST adds ~4-5 ms. Next for
    the 8.3 ms target: per-pass GPU timestamps (clears, MSAA resolve, AO, overlay, the present copy) on the runners,
    then cut the biggest. Cross-checking macOS code from Linux: `cargo check --target aarch64-apple-darwin -p
    gpui-pre-apple -p gpui-pre-macos` (CARGO_TARGET_DIR=/root/target-psui-mac; touch
    `.../build/gpui-pre-apple-*/out/stitched_shaders.metal` first); app-core's macOS module through the stub crate
    /root/psui-scratch/maccheck.
N0. (11-04 21:00, lead's speed list) (a) Dialog open: the preview's `apply_display` (~30 ms each, scene build ~16)
    is UI-thread work; build the scene off the UI thread with the regen (`scene_builder::build` needs `&mut GpuCache` +
    the uploader: move the cache into the worker or split upload from build), or skip `build_preview_base` when the
    blend base didn't change. (b) The overlay's view cube / right strip / evaluation tools as keyed sub-slices: a
    WIP is in /root/psui-scratch/mod.rs.wip-with-overlay-slices (`PsSlices` cube/toggles/eval), held back because
    the cube key doesn't cover everything `render_camera_menu` reads; the overlay slice is ~2 ms of a dialog open,
    2-5 ms of a row click. (c) Orbit on perf.yml: NIST at 2× has a ~9-10 ms per-frame floor even for an empty
    frame (handoff 0e above: GPUI's compositing / present of our texture at Retina), our scene adds 2-5 ms; next is
    timing GPUI's present on the runners (r.* scopes / Metal capture), not our passes. Run 41885259563's
    texture+variant orbit runs (1×) all present ~19 fps whatever is skipped: the 1× runs are pacing-bound, use 2×
    frames-driven variants (`retina@orbit:NOSHAPE_PERF_SCALE=2,NOSHAPE_PERF_BENCH_DRIVE=frames,...`) for ablations.
    The floor's likely cause (read 11-04 21:30): every orbit frame GPUI's Metal renderer (gpui-pre-apple
    metal_renderer.rs `draw`) takes a new drawable and redraws the whole window scene at 2388×1588 (every UI quad and
    glyph, our texture as one sprite) although only the viewport changed. Proposed fix: the viewport presents
    through its own CAMetalLayer, a sublayer below GPUI's (GPUI's layer non-opaque, transparent over the viewport
    rect; overlays, dialogs and menus stay in GPUI's layer above it), so an orbit frame touches only that layer and
    GPUI draws nothing. Needs: gpui-pre-macos a hook to add/position a sublayer for a view's bounds (z below the
    window layer), app-core gpu.rs presenting into it (CAMetalLayer drawable instead of the shared texture),
    resize/scale/occlusion handling; measure with 2× frames-driven perf.yml variants (base vs the same with the
    sublayer off by env). Linux/Windows keep the current path.

N12. (11-01) Part Studio audit (user 11-01): docs/parity/audit-part-studio.md. Dialogs: tests/std_oracle.rs
     `print_all` diffs every toolbar feature with std (STD_MODULES maps the rest to their std files;
     custom::std_feature::std_dialog_of); /root/psui-scratch/audit_gen.py + audit_doc.py turn its output into the D
     rows (base_rows.json = first run, keep D numbers). Six live requests for whole-surface dumps (panels, menus).
     10:19 audit 5: A rows 12 of 14 fixed (live 1717: config row menus, Variable Configure gating, Edit healthy
     moment, Appearances row menu); open A1 (Tag base plane) and A10 (core ticket). Next: A1, then the open P2 D rows
     (array item names, routing points), helix rows (need live in Axis mode).
     audit 6: std's ranges / hints on Variable, Helix, Routing curve, End cap, Hem, Corner, Corner break, Jog, Flex
     PCB end, Mate connector, Ruled surface, Frame trim: 154 of 199 rows fixed, 56 of 75 dialogs equal std. Asked live
     (1722): Tag Base plane prefill, Helix Axis layout, Routing curve point row titles.
     12:53 audit 8: oracle blind spots closed (custom reader + oracle now read "Driven query" and "Column Name":
     A15 driven arrays, A16 std's 99 column names in the configuration table, asked live 1725 for the toast);
     std's 48 editing logic functions audited (section "Editing logic"): 35 run std's (std_feature STD_EDITING_LOGIC +
     LOGIC_ONLY for types std can't regenerate; T2 tests in std_oracle), 10 native, 2 open (Routing curve, Unfold).
     A preselection goes to std's hidden initEntities / preselectedEntities / preselection where std's logic runs.
     Lookup results of editing logic reach the dialog (External thread sizes). 63 of 75 dialogs equal std.
     13:40 audit 9 (live 1722 / 1725 answered): Helix End radius rows, Routing curve (std's logic incl. copyParameter,
     "Routing curve (30 mm)" name, Point N rows, Single mode with the triad on the active point), configured columns
     in parameter order, out-of-range lengths / reals as live (A18). 64 of 75 dialogs equal std; editing logic 36 std /
     10 native / 1 open (Unfold: std's leaves the hold face empty on our part). Open: A10 (core), 5 reference pickers.
     11-01 G31 (lead) landed 8936d024: Show calculation variance in the Part Studio and the assembly (A21), checked on
     :102 against live 1731 (block 48000 ± 1.328 mm³ vs live 1.321, centre ± 1.000e-3, one inertia bound, Face tab).
     audit 11: manipulators (audit-part-studio "Manipulators", M1–M15: every std addManipulators against ours), landed
     fd90ed90, d2ef77be, then b8d179c5 (flips) queued: Shell / Chamfer arrows removed (std and live have none),
     Extrude's second end, Revolve's arc as live (stem + head past the arc, 5° snap, past 0 flips) and its start arc,
     Offset surface (= live 11-01), Replace face, Move boundary, Helix height, Draft neutral plane arc (live 11-01 look),
     a click flips a flip arrow (live 11-01), non-Blind ends' flip arrows. Next: M7 Transform angle / distance, M6 Plane
     angle, M5 partial fillet points, M11 flips (Split, Projected curve, …), Helix flip, Move curve boundary; M15 by
     catching std's addManipulators in the host for the features std runs (sheet metal).
     17:30: landed M6 / M7 (Plane Line angle, Transform rotate / distance), M11 Split's flip; queued 55802817: M15
     (FeatureState::manipulators from the host's addManipulators, drawn as arrows / arcs, a drag or click runs the
     feature's Manipulator Change Function via std_feature::manipulator_change_in; checked on :102 with Sheet metal
     model's flip) and RUST-EXTRUDE-SCOPE-SLOW's psui part (dialog FS functions under the kernel cancel flag after
     15 s: repro 418 s -> 33 s). Note: a CLI binary older than scope rule 5 makes legacy (native) sheet metal studios;
     rebuild the CLI (`rb.sh --get noshape-cli cargo build -p noshape-cli`) before making test docs.
     Open in this line: M5 partial fillet points; Helix flip; points manipulators. (Custom features' editing logic and
     manipulator change functions do run now: std_feature::editing_logic_in_clicked / manipulator_change_in take a
     Feature Studio's definition first.)

N11. (11-01 04:54) Drawing sheets taken from dx (user 11-01): docs/parity/workflow-drawing.md and
     workflow-drawing-sheet-metal.md. dx keeps drawing.md and the import sheets; notes to dx in docs/handoff/dx.md
     "From psui". workflow-drawing.md: every row re-checked or carried with evidence; the scale lists are live's
     (model-data drawing::sheet::SCALE_LIST); Create Drawing's memory as live 1706 (Recently opened, MRU).
     workflow-drawing-sheet-metal.md: every row re-checked on :102 (smf copy), all ✅ or withdrawn.

N10. (11-01 01:13) Unchecked rows (user 11-01): keyboard-mouse a / q / k, Ctrl+M, w and the empty Verified cells done;
     cursors 53 (count cursor with a dialog), 70 (Isolate / Replace instance / Interference titles), 78, 81, 82 ✅,
     74 ✅ (the Variable Studio pill is live's 60 % #1651b0 with white text, ui::name_pill_on); 46 ✅ (live 1697 + Onshape's bundle: only
     while snapped to a target, as ours). large-models 24: qa5's "Point" was our midpoint target of a full circle, at its seam;
     answered (live 1698: no midpoint on closed edges) and done (behaviors.rs edge_midpoint). part-studio 1.26 ✅ (env maps accepted as ours), 1.28 n/a.
     Dark-mode residual closed 11-01: the Variable type ▾ is live's 8 x 4 caret (os_ui::GLYPH_CARET), checked in both themes.
     dk-993 closed 11-01: the Appearances panel's empty tabs as live 1700 (icons/appearances-empty.svg,
     sketches-empty.svg: our drawings fitted to live's pixels with resvg; /root/psui-scratch/fit/sk.py). core-workflow F18 / F19 = live.

N06. 10-29 own display mesher (lead 10-28 21:05): step 1 landed 020fd2e7 (planes bounded by lines; crates/brep
     `mesh`, cpp/mesh_rust.cpp after core's cache, NOSHAPE_MESH_RUST=0 off); step 2 (this commit): circles split as
     BRepMesh splits them, planes with holes, cylinders and cones between two rims (zipped as the kernel's strips),
     a face on a circle a BRepMesh face shares declined to a fixpoint (watertight), seams' two polygons. T1 kernel
     rust_mesh::* (fins, holes, countersink, an angled hole left to BRepMesh, pattern instances). Next: measure
     (pattern_second fin plate, live-30-features open / edit, release, NOSHAPE_MESH_RUST=0 vs on), then spheres and
     tori (grids), cone apexes, and the bodies the Rust Boolean keeps (no OCCT reads).
     Measured (release, this machine, load ~14, pattern_second fin plate, 1606 faces): the first (cold) display mesh
     BRepMesh 291–361 ms → ours 181–196 ms (step 2) → 73–96 ms (this commit: our meshes and polylines handed to
     mesh.cpp directly instead of set on the shape (no polygons, no clean), loops chained through a vertex index).
     Left (kernel profile): core's shape maps 21–24, reading OCCT's geometry 16–18, our mesher 14–25, building the
     triangulations 7–9, edge polylines 5–8. After the fin (core's cache serves most): 134–153 → ~133 ms (the cache's
     own cost, not meshing). live-30-features edit commit: 99–115 ms either way (no display re-mesh in it).
     Step 2c (10-29): core's maps referenced, not copied (OcMesh), edge → faces from the shape's cached ancestors;
     slices across the bridge; edge nodes made once per body; the mesh test thread's off switch checked before
     the input is built. Bench (kernel `rust_mesh::bench_fin_plate_display_mesh`, release, load 27–30, thread CPU
     time): ours 77–85 ms against BRepMesh 243–255 ms (3–3.3×; wall 3.2–3.9×). perf: our Rust meshing (CDT,
     loops, strips) ~13 ms of it; the rest is the C++ around it, ~10 ms each: reading OCCT geometry, building
     Poly_Triangulations, core's edge loop, mesh_fill, the bounding box. Next for ≥10×: our faces written straight
     into the Tessellation (no Poly_Triangulation / mesh_fill round trip), geometry from the Rust Boolean's bodies.
     Step 2d (10-29): the circle fixpoint runs before meshing too (a face declined there isn't meshed for
     nothing). Fixtures on open (ops `regen_profile::regen_profile`, NOSHAPE_KERNEL_PROFILE=cpu, mesh_prepare +
     mesh_fill, release, load 16–25, NOSHAPE_MESH_RUST=0 → on): fins-400 438–529 → 109–150 ms (3.5–4×; ours
     85–124 of it, no BRepMesh); live-30-features 13–19 → 12–14.5 ms (ours 0.5 ms; BRepMesh 5–8 ms still meshes
     the fillets (tori) and the faces sharing their circles; before 2d ours meshed those too and threw them away:
     19–24 ms). Next: tori and spheres (grids), so live-30's fillets are ours.
     Step 3 (10-29): tori between two rims at constant v (fillets round circular edges): rows at the seam's nodes,
     each split round the axis as a circle there; a plane whose hole loop comes first (OCCT's wire order after a
     fillet) is triangulated with its largest ring as the outer one. T1 rust_mesh::fillets_round_a_boss (concave
     and convex tori, every face ours). Next: spheres, cone apexes (degenerate edges), measure live-30.
     Step 3b (10-29): the trigonometry once per row / column (a torus's rows were 4 sin_cos per node), faces
     meshed in parallel (scoped threads over runs of 4 faces, as BRepMesh runs InParallel; not rayon: under the
     kernel's mesh lock a rayon caller steals another body's tessellate job and deadlocks on that lock: model
     tests hung 1 h on vibe3, land aborted). Node and triangle counts equal BRepMesh's on every
     T1 shape (the filleted boss: 11332 / 21324 both). Fixtures on open (as 2d, load ~24, NOSHAPE_MESH_RUST=0 →
     on): live-30-features par: 9.9–18.7 → 10.3–11.8 ms (all 48 faces ours, ~60k nodes, mostly the bosses'
     fillets; our mesher 5.6 + triangulations 2.2 against BRepMesh 5.1; with rayon it read 8–9.5, with runs of
     16 faces one thread: 17.7), fins-400 488–611 → 100–107 ms (~5×; ours 77–82). Bench (rayon, load ~24): ours
     53 ms (CPU 49) against BRepMesh 235 (218), 4.4×. What's left on fins-400 is OCCT reads, Poly_Triangulation
     building, core's edge loop and mesh_fill (~10 ms each); on live-30 the per-node cost of our output.
     Step 4 (10-29): spheres (a torus with no hole) and degenerate edges (a cone's apex, a sphere's poles): their
     use's u from the edge's parameters (the surface's u there, a whole turn kept), the row there one node, its
     normal the axial part of the next row's. T1 rust_mesh::a_cone_to_its_apex_and_a_sphere (every face ours;
     area within 1e-3 and bounds within 1e-3 of the face's size for doubly curved faces: rows where BRepMesh has
     none, ours nearer the exact area). Every surface the lead listed is done; still declined: B-splines,
     offsets, revolutions / extrusions of free curves, a face with more than two loops on a revolved surface,
     a torus face bounded at constant u.
     10-29 09:16 PSUI-CONE-SHADING (P2, qa against step 4): the apex was one node with an axial normal (a blob on
     a cone's tip); now a node per fan triangle with its side line's normal. T1 checks normals on every mesher face
     (exact surface normal, BRepMesh's, each triangle within 7° at its middle).
     Note for whoever owns the UI test harness: part-studio-ui's sketch ui_tests read the shared
     ~/.config/noshape/left_panel_width.txt (1240 here, written 05:30 by some session's app), so
     section_rotate_handles_and_their_dead_centre and polygon_sides_typed_before_the_rim_click fail locally;
     with XDG_CONFIG_HOME pointing at an empty dir both pass. Tests shouldn't read the user's config.
     Pixel check (10-29, lead's T1 ask): ops `mesh_pixels::live_30_features_look_the_same…` regenerates live-30
     with our mesher and with BRepMesh (kernel `set_rust_mesh_everywhere`, a process-wide switch for tests) and
     renders both shaded with edges from three views, 800×600: 5–14 pixels differ (by > 24 in a channel) of
     75k–318k part pixels; asserts ≤ 0.5 %. NOSHAPE_TEST_OUT writes the images.
     Step 5 (10-29 13:10, 6f411558): our faces go straight into the display buffers (`RustFace`: ranges of the shared
     MeshOut, a pattern instance's moved in mesh_fill; no Poly_Triangulation; core's mesh cache keeps them as
     such, `MeshSet::rust`); OCCT reads (edges, faces) and the edge-polyline loop on OCCT's pool in runs of 64
     (inline when one run), edge samples on scoped threads like the faces (brep `in_parallel`); cached edge
     polylines shared, not copied (reversed while mesh_fill copies them); the output Vecs sized once.
     fins-400 open (ops regen_profile, NOSHAPE_KERNEL_PROFILE=1 wall, load 32 on 16 cores, mesh_prepare):
     BRepMesh 633–841 ms, ours 65–95 ms (≈ 9.5× median; 5.3× before at load 28). live-30: BRepMesh 28–56,
     ours 10–28 (≈ 3×): its time is the fillets' tori (5365 nodes each, ~0.9 ms) and copying ~60k nodes;
     wall times here swing 3× with the box's load (a 144-node plane took 5 ms once: preemption).
     perf.yml's model job now has a "Display mesher (release)" step (ours vs BRepMesh, fins and live-30, 3 each).
     **Runner numbers (perf 40880437816 on 32d87597 = step 5, Linux x86_64, mesh_prepare wall):** fins-400
     BRepMesh 235–242 ms → ours 18–21 ms (**11–12.5×**, + mesh_fill 1.1–1.3 against 2.2–2.4); live-30 12.1–12.6
     → 4.0–4.4 ms (2.8×; + mesh_fill 0.8 against 0.4). macOS: the step found no profile lines (fixed in the
     next perf.yml commit: each run's output kept as mesher-N.log in the model-perf artifact; read it).
     Step 5b (edf5dda5): a torus's normal in closed form (hand · sign(ring) · (cos v radial + sin v axis)), u's
     sines and cosines once per row length; kernel bench rust_mesh::bench_filleted_boss (11332 nodes, process
     CPU, all threads): ours 5.3 → 4.0–4.5 ms against BRepMesh 10–11 (NOSHAPE_BENCH_OURS=1 for perf record).
     Tried and reverted: the bridge's nodes in f32 (a moved instance then rounds twice and a prism's top no
     longer welds to its sides: display_mesh_guard). Left for live-30 (≥ 10× there needs ~1 ms): per node,
     mesh_strip's row tuples copied into points / normals and the zip; mesh_fill; fresh pages for every
     buffer (~12 % of ours in perf: kernel_init_pages / page faults).
     Step 5c (5f3d7b20): faces meshed costliest first, one at a time (brep `mesh_body` order by boundary nodes);
     the planar CDT ~25 % less CPU (legalisation stack kept, in-circle's bound only for det > 0, boundary
     half-edges found once; brep bench_cdt_plate_with_holes, 1444 points: ~1.75 ms; u32 indices tried: no gain).
     **Runner numbers on 5c (perf 40900438407):** Linux fins-400 mesh_prepare 228–233 → 18.9–20.1 ms
     (**11.4–12.3×**), live-30 12.1–12.6 → 3.6–3.8 ms (3.3×); macOS (Apple silicon, the mesh steps summed, its
     bridge names unreadable) fins-400 189–337 → 15–32 ms (10–15×), live-30 7.3–11.4 → 1.7–1.9 ms (4–6×).
     The fin plate meets ≥ 10× on both; live-30 doesn't: its time is the plate's top face (CDT of ~1450 boundary
     nodes, ~1.75 ms alone on one thread) and the fillets' tori. Tried for it, no gain, not landed: ear clipping
     with hole bridges (a mapbox earcut port: 1.5–2.3 ms on the same face, the long bridged slivers defeat its
     z-order hash), u32 half-edges, inserting in ring order instead of BRIO (3× slower). live-30's floor is
     then about that one face plus the fixed per-call costs (~1–2 ms: reads, edges, fill): ~3–6× is near
     what's reachable without splitting a face's triangulation across threads.
     Step 5d (d5ae3ded): a planar face's CDT result cached by its rings, grid points and insertion order (the
     only inputs; 16 kept, faces of 256+ nodes, the final checks re-run on a hit): an edit that moves live-30's
     plate top along its normal takes the last triangulation. live-30 edit commit here (load ~30): ours
     3.5–7 ms per display mesh against BRepMesh 18–75 (was 9–28). fins-400 after the fin (model
     pattern_second::timed_the_fin_plate_redisplays_after_the_fin): the display 52–60 ms (was 133–153), its mesh
     ~20 ms; the rest is display work outside meshing (N02's open item). perf.yml's mesher step now also times
     live-30's edit commits ("live-30 edit" rows).
     **Runner numbers on 5d (perf 40924170859, Linux):** fins-400 open 246–251 → 21–22 ms (11.5–11.8×);
     live-30 open 12.2–12.4 → 3.6–4.4 ms (~3×); live-30 edit_commit_profile's meshes 114.8 → 25–26 ms in all
     (4.5×; that run's "over 1 meshes" was a miscount, fixed next). macOS (Apple silicon) same run: fins-400
     364–451 → 18–23 ms (16–25×), live-30 open 11.5–22 → 4–6 ms, live-30 edits 117–129 → 32–37 ms (3.5×).
     cb540420: a small part's OCCT reads / edge polylines stay on the calling thread (the pool from 1024 items:
     waking it cost up to 6 ms on live-30's 150 edges under load). perf dispatched after it (read its
     model-perf artifacts' display_mesher.log).
     cc73aaea: the display's first touches of a new part off its critical path: shown vertices classified
     beside the tessellation (rayon::join in display.rs tessellate_body; ~12 ms on the fin plate), a big
     shape's bounding box per face on OCCT's pool (kernel geom.cpp, the same box; 14.6 → ~3 ms). fins-400
     after the fin: the display 52–60 → 25–34 ms; editing the fin: the whole edit 154–215 ms (load 10–30).
0. **After the cold-start work: our own display mesher (lead 10-28 21:05, user-approved).** `brep::mesh` in crates/brep
   (bool's crate, its Body / Surface / Curve types): planes (polygons with holes), cylinders, cones, spheres, tori from
   their exact parameterisation, shared edge samples (watertight), the display's deflection rules; anything else
   declines to BRepMesh per face. Hook in crates/kernel/cpp/mesh.cpp behind core's mesh cache, NOSHAPE_MESH_RUST
   (0 = off), on by default once correct; cached brep bodies where the Rust Boolean made them, else read the OCCT face
   geometry directly. T1: area / bounds / max deviation against BRepMesh, offscreen pixel checks. Target ≥ 10× on
   mesh_prepare (400-fin plate, live-30-features open / edit); numbers before/after here. Land planes, then
   cylinders / cones, then spheres / tori.

N05. 10-28 21:00 cold start with the metallibs loading (perf 40755981437 on c2428d9e, COLD=1: Metal cache cleared
     before every launch): "shader compile" (r.prewarm_pipelines) 65–141 ms on macOS 14 and 15, against 540–900 ms
     before (40686409197). First viewport frame, median of 9: macOS 15 1560 ms (1766 before), macOS 14 1364 ms; the
     first launch of a job 2.5–3.3 s, of which the window takes 2.0–2.8 s (OS first-run, gpui_kit::init), not the
     shaders. The build runner (macOS 26): pipelines from source 1039–1228 ms, precompiled 68–93 ms. Left for the
     2 s budget: the first window itself. macOS 14's red is the 1× texture pan (31 → 18.7 fps, the timer-paced
     bimodal path of N00), not this.
N04. 10-28 15:30 cold start, measured: perf 40709590907's build job (macOS 26 runner, Metal cache cleared before each
     run) creates the first-frame pipelines in 868–1033 ms from source and 79–130 ms from the metallibs (34 of 34
     loads hit). The bench of 40686409197 still read 540–900 ms "shader compile" on macOS 15: the libraries were built
     for the build runner's macOS 26 / MSL 3.2 and don't load on 15 (the app compiles from source then); macOS 14 had
     "Parent device is lost" on every launch (new in that run; one sample). Now built with -std=metal2.4
     -mmacosx-version-min=12.0 (naga writes the same source from 2.4 on). Next: perf -f cold=1 on that commit; the
     bench's first-launch "shader compile" should drop by ~0.8 s if they load on 14 / 15.
N03. 10-28 11:00–12:30: item 2 cold start landed as code, to be measured: vendor/wgpu-hal (29.0.4 + src/metal/
     precompiled.rs) loads `<hash of the MSL>.metallib` from noshape.app/Contents/Resources/metallib (or
     NOSHAPE_METALLIB_DIR) before compiling a pipeline's MSL; packaging/macos/metallibs.sh (release.yml and
     .github/actions/noshape-binary) dumps the first-frame MSL with crates/render/examples/prewarm_shaders
     (NOSHAPE_MSL_DUMP), compiles each with `xcrun metal -std=<its // language:> -fpreserve-invariance`, then reruns
     the example and fails if any pipeline still compiles (NOSHAPE_METALLIB_LOG=1 prints hit / miss per source).
     Type-checked locally with `cargo check -p noshape-render --example prewarm_shaders --target aarch64-apple-darwin`.
     Next: perf.yml `-f ref=<full sha> -f cold=1` and compare first launches with 40526882894 (1.28–1.96 s); the
     AIR still goes through the driver's backend compile, so expect part of the 480–670 ms back, not all.
     Also landed: a folder's Roll to here nests the bar at its top (open folder), and live's state 4 (behaviours
     FOLDER-MENU-ROLL-TO-HERE): no Roll to here there; the Parts header's properties toggle is saved per studio
     (PartStudio::parts_list_properties, not an undo step).
N02. 10-28 08:55–10:45, item 1 (display re-mesh): built a kernel face cache, then the lead (10:35) gave the kernel
     side to core; not landed. Findings, the patch and numbers are in core.md "From psui (10-28 10:45)": the
     planar CDT collinear bug (200–400 ms on the fin plate's rebuilt front faces, independent of caching) is the
     big one. My part, only after core's cache lands: measure the display work outside meshing on Extrude 3
     (~150–190 ms here: "display: bodies" minus "tessellate"; with_mesh_lines, shown_vertices, …) with
     `pattern_second::timed_the_fin_plate_redisplays_after_the_fin` and NOSHAPE_PERF=1, act if significant.
     Next: item 2, cold start (precompiled Metal shader libraries in the release job).

N00. Perf 10-28: v0.4.12's 1× texture orbit / zoom at 18 fps (run 40640263048) is **runner noise, not code**:
     v0.4.12's own commit built from source (40648174571) reads 31.1 / 30.5 / 30.7 fps on macOS 14 and 18.7 / 19.1 /
     18.6 on macOS 15 in the same run; bf131a31~1 and bf131a31 read ~31 on both. The 1× path is timer-paced and
     bimodal (one or two 33 ms ticks per frame); compare it only within one run, or use the 2× frame-driven rows
     (unchanged at 67–75 fps). Landed with it: cef2a10a, the hover band only for a whole hovered part.
N01. 10-28 06:00–08:40 landed: a237ae16 count fields two-step Enter (live), c8239e05 folder Roll to here / Roll to
     end and the nested rollback bar, db5db47b the folder guide's end tick, 469aa3c6 parts list properties (7.7),
     a801dcd6 blamed number fields (live "failing parameter marked red"), 3b5ecdee Where used rail panel, cef2a10a
     hover band only for a whole hovered part. part-studio.md parity: 3.19, 6.16, 9.14, 10.1, 11.5, 11.6, 11.7 now ✅;
     only the P3 13.x rows (release management, configured properties, datums, variable studios, model definitions)
     are left 🔧. psui's TICKETS section has no open item. Residuals: a folder rolled to its top draws the bar above
     it (live: nested at its top); the parts-list override isn't saved in the document; cold start first launch
     1.75–2.9 s on the runners (budget 2 s; the lever is precompiled Metal shader archives in the release job).

N0. 10-28 04:20–06:00 landed: 54324599 Create selection's see-through result faces (3.19), 6f4dc5c3 arrow halo as live
    (6.2), 2960a143 Measure asterisk (4.9), aaaa5b4c STRIP-CLICK-THROUGH (P1), 298fbda5 MASS-MC-FIELD (P2, ops names the
    reference without a material), 57ab9256 VAR-NAME-PINK. Waiting for live: 7.7 parts list properties, FOLDER-MENU
    (last folder). P0 COMMIT-LATENCY / undo 392 ms (qa, this machine at load ~30; CI 26–71 ms): profile on the runners
    with core before changing anything; the regen cache already keeps several versions per feature.

N1. (10-31) interaction 4.1 / 4.8 / 4.9 and part-studio 3.19 / 7.7 / 10.1 / 11.5–11.7 all ✅ now.
N3. Completion residuals: Onshape's module order for equal-cost rows (BooleanOperationType sits before
    NewBodyOperationType in ours); Shift+Tab in a snippet (untested live).

0a. P2 P1-8 caveat (Hole chips): a sketch's vertices picked from its row and clicked one by one are stored the same,
   so a reopened Hole can't tell them apart; needs the row pick stored as such (a model change).
0b. Cold start (COLD=1 run 40547478509 compared it: base 1.55–1.93 s, early 1.42–1.59 s, the window drawn later with
   early; default unchanged): with the Metal shader cache cleared (perf.yml 40526882894, COLD=1) the first frame is 1.28–1.96 s:
   our 8 first-frame pipelines (already compiled in parallel; naga 29 already compacts each to its entry point)
   take 480–670 ms and start only after the first window draws (`hold_prewarm`). Experiment ready in
   /root/psui-scratch/prewarm-early.diff (`NOSHAPE_PREWARM=early`: compile at device-ready on utility-QoS threads);
   measure it with a COLD=1 run whose variants set it, before landing as default. Bigger lever: precompiled
   metallibs (wgpu 29 passthrough `metallib`) built by the macOS release job.
0c. Retina orbit 76 fps (target 120) is GPU-bound (submit→done ≈ 17 ms mean, 45 ms p95 at 2388×1588, 4× MSAA):
   diagnostic run 40537102560 at 2×: base 71 fps (done 18.8 / 46.0 ms), MSAA 1 70.7, no AO 69.3, no back
   silhouettes 65.8, **no lines 81.8 fps (13.2 / 24.9 ms)**, no silhouettes 73.4 (p95 26.4). The edge lines are the
   GPU cost at Retina. Indexed quads (4 vertex shaders per segment) were tried (perf.yml 40563115906, alternating
   runs in one job): p95 25 ms against 46, but mean 19.4 against 17.3 ms and 69 fps against 75, so vertex count
   isn't the cost. A coarser line LOD (`NOSHAPE_PERF_LINE_ERROR_PX` 1 / 3 against 0.25, perf.yml 40583226566) changes
   nothing either (66.8–72.5 fps, noise), so segment count isn't it: next look at the per-segment fragment work (the
   line shader's AA band over 4× MSAA, blending) and the per-instance draw overhead (one instance per segment).
0d. Part hover look: done (render hover_outline.rs: mask of the hovered part's visible faces against the color
   pass's depth, then a band over the resolved frame within `dpr` px of the mask's boundary on either side; the hull
   outline no longer draws hover-only parts). Costs one mask pass + one full-screen pass only while a whole part is
   hovered. Verified on :102 (blk.noshape, Part 1 row hovered): right silhouette rest face | (101,126,141) (10,11,12)
   | bg → hover face | FFC685 FFC685 | bg, the bottom the same, as live's crops. Residual: on shallow slants a few
   pixels of the line's antialiasing lie diagonally past the band's 1 px reach and stay dark (a 1.5 px reach
   overshoots onto the background on steep edges instead).
0c2. (10-29) Retina lines, what's ruled out and what's left: lines share the one "color" pass with the faces (no
   extra MSAA load / store); the pick pass runs only on demand (`pick_valid`); segment count (LOD), MSAA level and
   quad shape (indexed 4-vertex) didn't move it. Left: the per-vertex work of `line_vertex` (shaders.wgsl), run 6×
   per segment: `load_inst` copies the 176-byte instance, a `states` lookup, two model and two view_proj
   products, `clip_near`, `in_front_of_faces`, sRGB conversion. Next experiment: WGSL `override` switches set
   from env at pipeline creation (skip states / lift / per-vertex instance load by passing what's needed per
   segment) as perf.yml variants on macOS at 2×, one run.
   Step one landed (7a8c12d2): `NOSHAPE_PERF_LINES=minimal` swaps the edge lines' vertex shader for the bare
   projection `vs_line_min`. Bench perf 40944655118 (jobs=bench, ref 7a8c12d2, variants r1/min1/nol1/r2/min2/nol2
   at 2×, frames-driven): compare the variants' GPU times (done_ms) within the job. min ≈ nol → the cost is
   line_vertex's work (slim it); min ≈ r → it is the raster (look at blending / varyings / AA width).
   **Result (10-29 21:30, done_ms p95 in each job):** macOS 14 r 43–46, min 46–48, nol 27–32; macOS 15 r 43–45,
   min 46–47, nol 20–31: the vertex work is not it. Suspect: fs_line's discards (dashes, section clip) force late
   depth testing, so every hidden line fragment is shaded and blended. Diagnostic `NOSHAPE_PERF_LINES=plain`
   (fs_line_plain, no discard) landed next; bench with variants r / plain / nol. If plain ≈ nol: draw solid
   lines with a discard-free pipeline (dashed / clipped ones keep fs_line).
   **Result (perf 40956037306):** plain ≈ base (macOS 14 done_ms p95 45–49 against 42–48; nol 20–22): not the
   discards. Note `NOSHAPE_PERF_SKIP=lines` also drops silhouettes and back silhouettes, which none of the edge
   variants touched; the older "no silhouettes" run cut p95 46 → 26. Next (landed with this note): vs_silhouette
   decides a candidate isn't a silhouette before line_vertex (same output); bench variants r / nosil
   (NOSHAPE_PERF_SKIP=silhouettes) / nobsil (backsil) / nol on it.
   **Found (perf 40964180293, NIST at 2×, done_ms):** macOS 14 base mean 25–27 / p95 40–41, nosil 13.6–14.5 /
   18.4, nobsil 21–25 / 36–37, nol 13.1 / 17.0; macOS 15 base 24.7–25.7 / 35–38, nosil 14.9–15.1 / 17.5–19.9,
   nol 11.2 / 17.1. **The edge silhouettes are the lines' Retina cost** (~10–12 ms of a ~25 ms GPU frame); the
   early-out (4d75111d) didn't move it, so it's their number, not their math: every interior tessellation edge
   of a curved face is a candidate drawn as 6 vertices each frame (scene builder `silhouette_edges`; coplanar
   pairs already left out), and only a handful per face are silhouettes in any view. Plan: a compute pass per
   frame culls the candidates (facing test with the instance's normal matrix) into a compacted index list +
   indirect draw (vs_silhouette pulling the candidate by index); behind a switch first, measured with the same
   variants, then default.
   Landed with this note: `SilCull` (renderer.rs) + `cs_sil_cull` / `vs_silhouette_pulled` (shaders.wgsl), opt-in
   `NOSHAPE_SIL_CULL=1` or `Renderer::set_silhouette_cull`; needs 6 storage buffers a stage (the renderer's own
   device now asks up to 8 where the adapter has them; Linux's shared GPUI device keeps the old path); runs too
   big for one storage binding draw the old way. T1 render offscreen culled_silhouettes_draw_as_the_plain_ones
   (pixel-equal, two slots); every render test also passes with NOSHAPE_SIL_CULL=1. Bench dispatched with
   variants r / cull / nosil: if cull ≈ nosil, make it the default.
   **Result (perf 40973793607, presented fps at 2×, NIST):** macOS 14 r 48.7–49.1, cull 62.6–65.8, nosil
   77.7–84.5 (done_ms mean 30–32 → 22, nosil 17); macOS 15 r 55.7, cull 62.0–65.5, nosil 74.7–82.4. On by default
   since 10-30 00:40 (NOSHAPE_SIL_CULL=0 turns it off). The gap to nosil is the cull's own per-candidate work:
   next, fewer candidates (per-face normal cones / screen-space bounds to skip whole faces) or a cheaper test.
   The cull reads 56 bytes a candidate (bandwidth-bound at millions of candidates): a packed cull-only copy (oct
   normals + position, ~20 bytes) would cut that. Beyond silhouettes, the frame without any (nosil) is still
   ~17 ms GPU at 2× (faces at 4× MSAA, AO, edges): 120 fps needs that halved too; measure with the existing
   NOSHAPE_PERF_SKIP variants (ao, meshes) the same way (done_ms within one job).
   **Measured 10-30 02:40 (perf 40987999836, main 7523035a, cull on):** NIST at 2× presented 79.4–81.9 fps (macOS
   14) / 71.6–85 (15), done_ms mean 12.8 / 14.4–15.0, p50 ~10.5; noao, msaa1, nol, nomesh all within noise of it
   (11.6–14.7 mean, p50 10.1–12.2). So after the cull nothing in the scene dominates: a ~10 ms p50 floor every
   variant shares, i.e. per-frame fixed cost (MSAA resolve, the texture present / copy into GPUI's frame at
   2388×1588, pacing). Next for 120 fps: time those stages (r.* perf scopes around resolve and present) on the
   runners.
   **Floor measured (perf 40993453087, NOSHAPE_PERF_SKIP=lines+meshes: an empty frame):** done_ms p50 7.0–9.5 ms,
   presented 87–110 fps (4× MSAA) and 113–125 fps with MSAA 1; the full NIST frame in the same jobs 64.7–78.1 fps.
   So ~8 ms of the 2× frame is fixed: MSAA clears / resolve, depth kept for AO and the AO pass, background, the
   texture present. Next: which of those (a variant skipping AO on the empty frame needs `+` support in
   NOSHAPE_PERF_SKIP's `ao` / `backsil` / `silhouettes` tokens, which split on ',' only; bench.sh splits a
   variant's env on ','), then memoryless MSAA attachments (StoreOp::Discard on depth where AO doesn't need it).
   **Isolated (perf 40999839485, deabae0d's `+` tokens):** the empty frame's done_ms p50 is 9.0–9.5 ms with AO
   or without, with 4× MSAA or 1 (presented 91–120 fps); the full NIST frame 10.6–11.1 p50 (75–80 fps). So the
   floor isn't our passes: our submission waits behind the window's own compositing / present at 2× (done_ms is
   submit → completed). What our scene adds is now ~2–5 ms. 120 fps at Retina would need the present path
   itself (GPUI's texture compositing) to get cheaper: that's GPUI's, not the renderer's; left here.
0f. (done 10-28: no gain, see above) Retina orbit, experiment for 0c: line segments are instances of 6 vertices (`draw(slots*6, 0..count)`),
   which packs Apple GPUs' vertex SIMD groups badly; pull segments from a storage buffer by vertex_index / 6 with
   the slot as the instance (one instance per copy) and measure with perf.yml.
0e. P3 LEFT-RAIL-WHERE-USED: live's Free account only shows an upgrade notice for "Where used"; a panel listing
   the tabs that use this element would be ours to design.

1. Base part shading: the offscreen test part_faces_are_lit_like_onshape holds the block top at live's
   (140,185,212) within 4; the preview samples' few units are the preview mix, not the base shading.
3. Cold start first launch 2.5–3.4 s (budget 2 s): wgpu-hal isn't vendored; a per-(source, options) MTLLibrary cache
   needs vendoring wgpu-hal 29, or smaller per-pipeline shader modules (naga's MSL writer emits the whole module per
   stage). perf.yml without `-f ref=` measures the latest release: run 40464678503 (v0.4.8) failed its gates on
   the paravirtual macOS runners (NIST assembly texture "orbit" 33.2 → 19.3 fps; that log is the pan run, kind
   mislabelled; the orbit log reads 30.8 fps). Run 40467324821 (`-f ref=main`, 07:28) measures main: read its
   summaries into viewport-performance.md and compare.
5. p1-dialogs: checkbox label widths differ by font (Flama vs Roboto); nothing to do short of a font change.

## Requests to the lead

- 11-04 21:05 **P0 found and fixed on main**: my d2eccf9f (18:16) collapsed the feature list's rows in any studio
  with more features than the panel shows (all rows drawn on one line); a8c24d3b (landed 20:48) fixes it, with a T1
  on a 120-feature list. Releases cut between 18:16 and 20:48 carry it. Your list's (1) row click is within a frame
  on CPU (9-13 ms p50); (2) dialog open went 43-46 → 26-27 ms CPU p50 (queued 3580968e; it patches vendor
  gpui-pre's focus handling and view cache for every studio: all UI crates' tests pass, focus rings checked by hand),
  the rest is the preview's scene apply (N0 a); (3) orbit: not started today beyond reading 41885259563 (pacing-bound
  at 1×, N0 c).
- 11-04 17:37 lead's list (step_ctrl, MC-INFERENCE-DOTS, Part Studio speed, MISSING-LATER-CHIP): all done. Speed
  state, cf-straight-spline on :102 (box at load ~26 on 16 cores: ±15 ms noise): hover's UI part p50 6.5 ms, pick's
  3.7 ms; a row click's frame 55-59 ms (layout + prepaint + paint), of which the toolbar ~20 ms is gone with
  `Slice::keyed` (renders 23 → 1 in a run) and the two lists ~18 ms remain (they do change); dialog open: the
  lists ~40 ms, the overlays ~17 ms of its frame. Orbit on a real GPU only from perf.yml: the NIST Part Studio's
  GPU 12.8-15 ms a frame at Retina (over 8.3), pins-assembly-1000 8.7 ms; the model-suite documents can't go to CI.
- 11-04 16:04 start window Help ▾ finished (833cae3e): live was silent since 12:24, so I captured Onshape's documents
  page ? menu in my own tab (behaviours "the documents page's ? Help menu", shots live/psui/docs-help-*): ours has its
  offline rows, Keyboard shortcuts (opens the studios' sheet over the page) and About, around the two agent items.
- 11-04 14:05 OS audit (AGENTS.md ad57a47b: the app alike on every OS), Part Studio / start window / Preferences
  and the app-core / app code they use: one behaviour differed, fixed (9c3b3337): closing a document's last window
  showed nothing on macOS; it now shows the start window as elsewhere (macOS only keeps running after the start window
  closes, its convention). The macOS app menu's Install command line tool… / Connect to coding agents… are in both
  Help menus; its Quit is Ctrl/Cmd+Q on every OS (Onshape has no Quit row; closing the windows quits elsewhere).
  The other cfg branches are conventions or platform plumbing: client decorations on Linux, native file dialogs,
  Cmd for Ctrl / no Windows-key modifier, per-OS wheel units (each normalised to Chromium's notch), GPU / cursors,
  Claude Desktop not offered on Linux (none exists). Start window Help ▾: mine (a7fac69b) and the lead's 6418da8a
  were the same menu, the lead's conflicted; its About row ported (2e2a62bf). Onshape's own rows wait for live
  (REQUESTS 13:29).
- 11-04 13:15 (lead: Part Studio speed, then the three P3s): see "Speed targets" (hover / pick / dialog open / row
  click rows). Fixed: a selected feature row's highlight (56 → 2 ms on cf-straight-spline), PickMap hashing. Left
  over target: GPUI prepaint of the list slices after a click (30–50 ms on 117 rows; next: time the views in prepaint),
  `ps.apply_display` 150–390 ms on the UI thread per regeneration (scene build 100–130 ms of it). The model-suite
  documents can't go to perf.yml (not ours); their translations: vibe5 `~/psui-parity-saved/`, here
  /root/psui-scratch/perf/ (NOSHAPE_PARITY_SAVE). Bench runner: /root/psui-scratch/benchrun.sh (own config dir: the
  shared ~/.config/noshape has left_panel_width 1240). cf-straight-spline regenerates in 16.2 s (vibe5 release), live
  7.8 s: told core. P3s: DIALOG-UNDO-ON-CONTROL, FS-SEARCH-BAR fixed; MISSING-LATER-CHIP's look done, its menu waits
  for live (1993 (b)).
- 11-04 11:06: live's 1990 answers (behaviours l.34565) put in part-studio.md: 22 rows match. Corrected: a
  double-click on a sketch's line in the graphics no longer opens the sketch (3.18), a click on empty list space keeps
  the selection (6.29). New P3 tickets MISSING-LATER-CHIP, DIALOG-UNDO-ON-CONTROL, FS-SEARCH-BAR. 9.15 inertia and
  5.15's "Edit healthy moment" asked again (REQUESTS 11-04 11:06). Still waiting: 1991 (hole-* re-recordings).
- 11-04 10:57: **MC-INFERENCE-HOVER** (lead): a field taking mate connectors (Mate connector's Origin entity, its
  nested editor, a field's mate connector button on) shows the hovered face / edge / region's inference points
  (skasm's `inference_candidates`, background task in part_studio/inference.rs) as white dots and the nearest as the
  hovered glyph (scene `InferenceMarks`, shared; skasm told in skasm.md); a click stores live's way (model-data
  `ConnectorOrigin`: face + POINT / MID_POINT / CENTER / LOOP_CENTER + secondary). A vertex hover picks the vertex.
  Checked in the app against psui/hole-mc-1851/4 (same dots and glyph). Note: my app display is :102, not :101.
- 11-04 10:38 (lead's three items): (1) **Hole remembered values** are Onshape's (live 1987: server-side per user and feature
  type, on ✓ only, quantities per document unit). Ours now persists them (app-core feature_settings.json; off under
  NOSHAPE_UI_SCRIPT and in tests); asked live to commit std's default Hole and re-record hole-* (REQUESTS 11-04 10:38).
  (2) **IN-FEATURE-MATE-CONNECTOR done** (model, regen, op, dialog, nested editor, Ctrl+M; note in core.md); its hover
  inference points split to MC-INFERENCE-HOVER. (3) **part-studio.md audit**: 233 of 258 rows cite live; corrected
  1.3 / 1.9 / 1.10 / 5.26 (the dialog title no longer renames); 25 rows asked of live in one session.
- 11-04 08:42: face-rclick-menu's highlighted row is accepted: both windows are 1919 x 992, but live's face menu has the
  cloud-only "Add comment" row, which makes it one row too tall to fit under the pointer (it shifts up and the
  pointer lands on row 20); ours fits exactly below. FS-EDITOR-IMPORT-NOTICES fixed (custom::doc_imports); the
  10-30 mesher fallback ticket closed (verified v0.4.15). IN-FEATURE-MATE-CONNECTOR still waits for core's model side.
- 11-04 06:03: **274/287 scenarios, 1798/1833 states** (full run on main). Left: 6 Hole (live's account remembers
  M6x1.00), chamfer-two-distances (focus after an edit brings a field up: asked live by hand, REQUESTS 04:0x),
  face-rclick-menu's highlighted row (menu placement), measure-fillet-face (stale label, re-record asked),
  extrude-end-type-up-to-face (live's start), asm-fastened-undo-picks / revolve-dialog-axis / sketch-extend-to-line
  (skasm).
- 11-04 03:36: since 01:05 (landed or queued): Mass Override without a material (live 1959), Section view's keyboard after a
  graphics press, the no-material notice waited for by the driver, Rib's Parts empty until a profile, an existing
  feature keeps its direction on Remove, face double-click keeps its features lit, the point's menu, menus highlight
  the row under a resting pointer, the driver waits for the hover to settle; 15 edge scenarios click a quarter of the
  edge (live re-recording them). Full run 02:00: 254/282.
- 11-04 02:59: **edge midpoint rule confirmed by hand (11-04 03:36, live 1981: a 2 s still rest then click takes the midpoint point, Fillet empty, the point's menu; a quick click takes the edge; the sketch side differs)**. Was under review (lead 03:00). Ours (92095b8d): a pointer resting 250 ms on an edge's
  midpoint snaps to the midpoint point (behaviors.rs MIDPOINT_DWELL, schedule_midpoint_dwell; the point's menu; POINT
  of EDGE in the state). Every Part Studio recording agrees (select-edge-then-chamfer-tool 00:56 POINT; edge-rclick-menu
  without Fillet…; fillet-* fields empty), but skasm's sketch version was reverted at 03:00 (live's 02:24–02:40
  re-recordings select the line at its midpoint) and 1954 came from the runner, so I asked live for a by-hand retry
  (REQUESTS 11-04 02:59). If by hand the click takes the edge: drop the dwell (ready = moved only, as d89c51a1 had) and the point
  menu stays only for a moved pointer. The 15 edge scenarios now click a quarter of the edge either way.
- 11-04 01:07: **187/221 scenarios, 1433/1532 states**. Since 23:00 (all landed): the graphics context menu in the state
  (app-core os_menu::menu_state); a pick that fills a field and moves on gives the next field the keyboard; which
  control keeps the keyboard when an edit brings up a field (tab: the field; checkbox: only if the old active field
  went away; dropdown: keeps it); dialog angles read "3 deg" focused; Loft's End conditions group title left out of
  the state; the Mass panel's state (mass_ui_state). Open with live (REQUESTS 00:0x–01:0x): **edge / line midpoint
  after a still hover** (explains fillet-* / chamfer-* / undo-new-fillet / edge-rclick-menu / sketch midpoint
  clicks: if live targets the midpoint, move the scenarios' clicks off it and make ours snap after a still dwell),
  Remove on an existing Extrude flipping (specifiedParameters), Paste sketch with a clipboard, face-dblclick's lit
  rows, Mass Override without material, edit-feature-flip-direction's runner miss.
- 11-03 23:14: **134/150 scenarios, 1101/1142 states** (tests/ui-parity/live, kept in sync with /root/parity/ui/live after each
  live batch). Left: 7 fillet-* (live's account), 8 sketch (notes in skasm.md), extrude-end-type-up-to-face (live's
  runner). Since 19:00: rolled-back rows aren't reported hidden (variable-edit-focus). Live's batch reads scenarios from
  /root/qa-scratch/ui-parity (asked to use main's, REQUESTS 11-03 23:14; its sketch-slot ran old steps: not synced).
- 11-03 18:31 (psui): P2 IN-FEATURE-MATE-CONNECTOR needs core first. Live answered (behaviours "IN-FEATURE-MATE-
  CONNECTOR", request 1851): Hole's mate connector side button on, a face click makes a connector that lives inside
  the Hole (features.json `subFeatures`), chip "Mate connector", nothing in the feature or parts lists. Proposed model
  shape for core: `Feature.sub_features: Vec<Feature>` (serde default, skipped when empty), each a mateConnector
  evaluated just before its feature in regen/mod.rs's loop (in the feature's cache key; failures go to the
  feature's state), its connector keyed by the sub-feature's id so `Reference::MateConnector { feature: sub.id, .. }`
  works unchanged; suppressing / deleting the feature takes its subs; the ops feature add/update params take them
  (and the parity translator then stops building them as features before). Once that's in I do the UI: the side
  button's on state (os-active, rgb 234), face hover inference points + triad, the chip with its icon ("Mate connector
  (Click to edit)") opening the nested "Mate connector" panel (On entity / Origin entity / Realign / Move X Y Z /
  Rotate about Z / angle), the chip's right-click "Clear all".
- 11-03 15:10 (psui, seen while checking the update card): the start window at 560 px wide lays itself over itself:
  the "Details" panel's header and icon cover "Owned by me", "+ Add" and the list (shot
  /root/psui-scratch/start-560-overlap.png). Not from the card (the same without it); start-ui isn't mine.
- 11-03 01:20 (psui, from the ui-parity suite): WHOLE-SKETCH-PICK (P2, core + psui): live's click on a sketch's row in a
  face field stores a whole-sketch region query (`BTMIndividualSketchRegionQuery` with the featureId only): the chip
  reads "Faces of Sketch 1" also with one region and after reopening, and the selection follows regions added to the
  sketch later. Ours stores the regions (`Reference::SketchRegion` each), so a reopened one-region pick reads "Face of
  Sketch 1" and new regions aren't picked up; `sketch:<id>` in an Extrude fails "No valid regions to extrude"
  (EvalCtx::profiles has no Reference::Sketch case). Needs core: Reference::Sketch in face fields = all its regions
  (profiles, queries, naming); then psui stores it for row picks and labels it "Faces of …".
- (11-01 18:10) Part Studio audit A22, for core: 378 of the 529 literal failure texts our native features raise are not std's ErrorStringEnum texts (what live shows in the red row's tooltip and the dialog title). List by file in docs/parity/audit-part-studio-errors.txt. Needs each failure mapped to std's condition; regen features are core's.
- (11-01 07:43) FYI, all agents: an app built on vibe4 with `rb.sh --get` showed **no SVG icons** here (no ✓ / ✕, chevrons,
  checkmarks, carets, tool icons): rust-embed reads its folders at run time in debug builds, from vibe4's absolute
  paths (our app/assets and gpui-kit-assets in vibe4's cargo registry). Fixed by rust-embed's `debug-embed` feature
  (app-core Cargo.toml; one rust-embed 8.13 in the graph, so gpui-kit-assets embeds too). GUI checks of --get builds
  before this landed judged icons that weren't drawn; worth a word to the others.
- (11-01 01:13) For skasm (assembly): (1) with a new Mate connector (Ctrl+M) or mate dialog open, live lists the in-progress
  feature as a red "Mate connector 1" row under Mate features (live 20891, 1693); ours shows no row until ✓.
  (2) The instance list's right-click on 2 selected instances (live behaviours "Multi-select (2 fixed instances) in
  the list", assembly/533) reads "Replace instances…" and ends "… Create empty subassembly · Clear selection ·
  Delete"; ours says "Replace 2 instances…" (live's graphics menu says that) and has no Clear selection.
- (11-01 02:00) The land-slot requests (10-31 13:03, 20:12, 11-01 01:13) are resolved by 0abc635f (slots in arrival
  order); land73–75 went through. The 10-31 12:01 note below was passed to skasm by the user.
- (10-31 12:01) For skasm: ops e2e `sheet_metal_table_ops` steps (Bend A radius 3 → Move up → Convert Bend B to rip) leave
  the model, in about half the runs, with **no bends and only "Joint A"** (the other half: Bend A + Joint B). The test
  didn't check the table there so it passes either way; ids are random per run, so std's joint matching looks
  order-dependent. Also under std a converted rip's `Rip::faces` is empty (no entity for a new Modify joint).

- (10-30 20:40) tools/tiers/perf-remote.sh: the checkout of the shared ~/perf/wt happens before `flock 7` (one perf
  run at a time), so a run waiting on the lock gets its tree replaced by the next run's checkout and builds / times
  that commit while printing its own sha. Seen twice: psui 3494042f's `rust_mesh::bench_profile_fin` ran 0 tests (175
  in the binary; 176 at 3494042f, the run passes locally) while perf/skasm-388555fc was pending. Fix: take `flock 7`
  before the fetch / checkout (or a worktree per name). psui's next commit does the first (perf.sh runs the measured
  commit's own perf-remote.sh, so it holds for a run once its commit has it); please check the baseline job takes the
  same lock before its checkout. The 3494042f / 960b0771 rust_mesh numbers were of other trees (441ee2c5 et al.).
- (FYI 10-30 00:50, no reply needed) 6ca3c756 turns the edge-silhouette GPU cull on by default (compute pass +
  indirect draw; macOS 14/15 NIST orbit at 2× +20–35 % fps). If the release smoke shows missing or extra outline
  lines on curved faces, `NOSHAPE_SIL_CULL=0` restores the old path; Linux (GPUI's shared device, 4 storage
  buffers) keeps the old path anyway.
(none; 4e55b01f is in v0.4.10)
- (FYI from skasm 10-30, lead-approved) `OsSelect::set_pictures(id → RenderImage)`: rows of items with a picture are 40 px with a
  70 × 40 image before the label (live Form ▾ of the form library, request 1622 (d)). No picture set: unchanged.
- skasm 10-31 (in your crate): part_studio/analysis_tools.rs `isocurve_overlay` draws Show isocurves (Loft, Boundary surface, Fill, Fillet …) and Show ruled lines (Ruled surface) while the dialog is open, in vivid magenta #d403f9 (help "colors"). It's cached per preview in `UiState::isocurves`, and the lines come from noshape_model::analysis::feature_isocurves / feature_ruled_lines.
- skasm 10-31, **SM4i / SM4k: the model supports both now** (your Requests to the lead, 10-31):
  - **SM4i:** sheet metal runs by std (the default; the native evaluator, with its "The joint is already a rip", only
    runs for sheet metal saved before std or a regeneration without the FeatureScript host). std's Modify joint restyles
    a rip and turns it into a bend. T2 sheetmetal_std `modify_joint_restyles_a_rip_and_turns_it_into_a_bend`, on G34's
    Joint C:
    - Style: `sheetMetalJoint` with `entity` = an edge of a wall end at the rip (e.g. the y = 0 flange's end edge at
      (0.02, 0, 12) mm), `jointType` RIP, `hasStyle` true, `jointStyle` EDGE / BUTT / BUTT2 → the rip's style changes.
    - Type: the same feature with `jointType` BEND (`useDefaultRadius` true) → the rip becomes a third bend.
    So the Other joints Type ▾ / Style ▾ can add / edit a Modify joint feature.
  - **SM4k:** `FlatPattern.shape` (and so `flat_view_solid()`) now has a face per wall and per bend zone, split along
    both sides of every bend zone. The outline `loops`, DXF and drawings are unchanged. G34's flat has 5 faces; T2 a
    one-flange flat has 3.
    - GUI :103 on G34: the zone edges draw as solid lines beside the dashed bend lines, as sm-091.
    - Your flat hover still outlines the whole flat. Hover / pick by face of the flat view solid to get live's
      one-face #FFC685 outline.
- skasm 10-31, in your crate (part_studio/behaviors.rs, mod.rs Paste), from live (behaviours "skasm (2026-10-31, P3 workflow-derived rows 29 / 30)"):
  - Ctrl+V of a copied part in a Part Studio now does **nothing**. It used to open Derived (DOC3, which was an
    assumption: live shows no Derived dialog and no message).
  - "Copy Part N" / Ctrl+C on a part copy silently (no "Copied …" toast).
  - The Assembly's Ctrl+V inserts the copied part at once (assembly side, mine).
  - workflow-keyboard-mouse.md row "Ctrl+C / Ctrl+V" is updated.

## From core (11-01 16:00): Mass properties › Show calculation variance (skasm audit G31)
- model-core `regen::measure::mass_property_variance(ctx, bodies, densities)` returns a `MassProperties` of bounds
  (± per value: volume, surface area, mass when every part has a density, centre x / y / z, each inertia entry), next
  to `mass_properties`; kernel `mass_property_bounds(shape, PropsKind::{Volume, Surface, Linear})` for faces / edges.
  About two more integrations a part: call it only while the box is ticked.
- The bounds come from a second, coarser integration (at least 1e-12 of each value's scale): on everyday parts they
  sit at that floor. Live's format and magnitude are asked in REQUESTS (core 2026-11-01 15:30, G31); I'll tune the
  scale to live's numbers when they come. The checkbox in part-studio-ui menus.rs / assembly_mass.rs is yours to
  wire: append " ± <bound>" in the dialog's units as live shows it.
- (11-01 19:00) Live's answer (behaviours "core (2026-11-01, P2 audit G31)"): with the box ticked each field reads
  "<value> <unit> ± <bound> <unit>" in the same field, the bound always d.ddde±x ("1.321e+0", "2.912e-9"), the value as
  usual. Parts: Mass, Volume, Surface area, Center of mass X / Y / Z, the inertia grid (one bound for every cell; the
  text cut with "…" in the narrow cells). Face tab (planar faces only): Surface area, Perimeter, Area moments, Polar
  moment. `mass_property_variance` now gives live's magnitudes (Parasolid's: 48000 ± 1.321 mm³ for a 60 × 40 × 20
  block, the centre ± 1.000e-3 mm), and `regen::measure::face_property_variance(faces)` the Face tab's (area,
  perimeter, area moments).

## From skasm (11-03 05:35): parity driver and sketch picks
- (06:50, replaces the 05:35 note) Dimension picks do resolve in driver runs; what failed was the placement click:
  in the start's iso view it lands over a default plane's area or a constraint glyph, which took it (fixed, 60050f25).
  The driver now clicks at whole pixels (a mouse's, and Chrome's under live's runner).
- Lengths drawn by clicks can't match live to the digit: live's viewport is 43 px lower and 1 px is ~0.3 mm at the
  start zoom, so a clicked 30 mm line reads 29.73 in live and 29.99 in ours (sketch-dimension-*, circle-dimension:
  `.sketch.dimensions[].value` and the box's `focus.text`). For qa: type the length (value box) where a scenario
  compares a dimension, or the runner compares those within a pixel.
- (08:40) With your `{"tool": X, "menu": Y}` (d41cb3d1), the sketch toolbar's tools and group carets are marked too
  (`tool:<Onshape name>`, `caret:<face>`; sketch.rs). Your sketch-slot clicks two centres and a width, but live's Slot
  works on an existing curve (behaviours "spec: skasm sketch Slot tool commit"; mine: sketch-slot-on-line), and your
  sketch-tangent-arc's `a` is the 3 point arc (mine: sketch-tangent-arc-from-line, via the dropdown).
- I added two optional state keys (README "undo", "sketch"; recorded by part_studio/ui_script.rs, compared by run.py
  once live records them).
- (05:55) Sketch toolbar tools aren't marked for the driver (only the feature toolbar's `tool:` names), and the driver
  takes `{"tool": X, "menu": Y}` as the tool X itself: sketch tools in dropdowns (3 point circle, Center point arc …)
  can't be targeted yet; my scenarios use the keys meanwhile. Fixed on the way: in a sketch `a` / `q` were the
  feature dialog's Flip primary axis / Reorient secondary axis (skasm, app-core actions `dlg` now `!Sketch`).
