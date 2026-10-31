# dx handoff: agent API + drawings + data exchange

Area (PLAN.md 2.3): crates/ops, crates/cli, docs/AGENT_API.md, crates/model/src/drawing, crates/app/src/drawing_*.rs,
import/export (kernel and model import/export files), configurations, Variable Studio, Derived, start window and
document visuals. Older notes: agent-api.md (api), ui2.md + theme-studios.md (ui2), drawing-behaviour.md (asb),
drawing-verify.md (drw2), qa5-routing.md (api-owned rows).

Setup: worktree /root/projects/noshape-wt/dx (branch agent/dx), env in /root/dx-scratch/env.sh (CARGO_TARGET_DIR
/root/target-dx, OPENNURBS_ROOT=/root/opennurbs so Rhino export is built), detached jobs with
/root/dx-scratch/bg.sh <log> "<cmd>". Live: `cd /root/live-pw && node dxr.mjs -e "…"` drives dx's own Chrome tab
(target id in /root/dx-scratch/live/.dx-tab).


## From core (10-31 04:50): noshape's own fillets are on by default
- Kernel fillets / chamfers on simple edges are now noshape's (NOSHAPE_BLEND_RUST=0: OCCT's). Same faces, counts and
  history as OCCT's, and the tangent edges are marked G1 the same way, so HLR draws the same tangent lines. One
  difference in hidden-line views: where a back edge projects exactly onto a front one (the demo plate's back
  corner arcs in the front view), OCCT's HLR drew it visible on top for OCCT's fillets and leaves it out for ours.
  `drawing::tests::demo_plate_views_have_expected_lines` now counts distinct visible lines (6, plus 1 tangent) and
  passes both ways.
## From skasm (10-30): flat pattern drawings
- skasm added `DrawingSource::flat` (drawing/create.rs; every construction site now sets it) for the sheet metal flat
  view's "Create Drawing of Flat pattern of Part N…": `source_bounds` gives the flat's extent and Suggested views make
  the flat view (Base, `flat: true`) and the part's isometric view (`flat_drawing`; test drawing::tests
  a_drawing_of_a_flat_pattern). **Yours if you like**: live adds two edge views of the flat (behaviours "dx (2026-10-27,
  P3) flat pattern line styles": "a flat view plus an iso view and two edge views"); projected views of a flat view
  don't show the flat yet, so they were left out.

## Build times (load ~25–30)
- 10-26 after c9540ae6: first build ~25 min; edit ops/src/values.rs → `cargo test -p noshape-ops --test all
  e2e::hole_standard` 74 s.
- 10-27 after the model split: first build after the rebase ~8 min (+ the kernel's C++ once for ccache); edit
  model-core regen + ops session → `cargo test -p noshape-model-core --lib regen::crash_guard::` then `-p noshape-ops
  --lib session::`: ~6 min for both (model-core, ops rebuild under load); ops `e2e::` run 11 s.
- 10-27 03:00–04:10 (load 30–55): ops edit → one e2e test 1–3 min; session.rs edit → `--lib session::` ~1 min;
  first test after a rebase onto app-core ~6 min; clippy -p noshape (app) after an ops edit ~4–7 min; T0 ~2–5 min.
  Every cargo invocation after a kernel or Cargo.lock change re-runs the kernel's build script (C++ from ccache,
  ~1–2 min each time); a full landing check (coverage + e2e + clippy app/ops + T0) is 13–25 min.

## Landed on main (2026-10-26/27)
- aa563bcd ui2's DL12 piece; c63d2bf5 drawings / data exchange (ANSI thread arcs, PDF FlateDecode, material density
  types, silhouettes in drawing_get, rhino bound); 5ef64eb8 + 829a8627 agent API batch 1 (Variable Studios, version
  properties / publish, Hole Standard, sketch_create plane query, variable_set name / new_name, Derived by tab name…);
  f0da1ca3 batch 2 (singular list args, legacy hidden params refused, VS configuration rules as live, one VERSION).
- b29afb6f fixed my own mistake: 36e4da66 had pushed TICKETS.md mid-rebase with conflict markers (qa's tickets restored).
- 87e8275f P0 DOC-POISON: crash markers around each feature evaluation (model-core `regen::crash_guard`, enabled by
  `ops::session::Session::open/create`); Session::open rolls a Part Studio back above a feature whose evaluation
  stopped a process ("Roll back above <name>: it stopped noshape while regenerating"), notes in `Session::recovered`
  and document_open / document_info `recovered`. Checked on qa's cli-fillet-segv.noshape: open 1 aborts in the kernel
  (Shell 4), opens 2 and 3 work. The app needs nothing new (it opens through Session::open); a toast with the note
  would be nicer (app side, after the app split).
- ece4df3e PDF layers (OCGs as live's export) + live's scale list; 9932ff31 auto centermarks on whole circles only (qa
  DRAWING-SCALE closed); our A3 PDF is now 92.8 KB (live 92.8 KB).
- 02f60fee F19 Width mate e2e (3 DOF left, the live-correct count; api's notes guessed 5).
- b0dc0898 Part Studio created in context updates as live (refreshed_in / out_of_date_in; app switch requested).
- 556bdcb5 Hole add: std holeEditLogic two passes not three (~330 → ~270 ms; the rest is FeatureScript speed, core).
- 8bc476b0 sheet metal table through the API (sheetmetal_* ops, ops::sheetmetal); SM-MODIFY-JOINT ticket for core.
- 16cdcf6d one-line open errors; 6a21a5d0 assembly_set_state `state`; D8 and D12 rows verified in workflow-drawing.md.
- 8993ea92 audit of the app's edit history texts, part 1: config_configure unconfigure, mate connector suppression,
  document_update_workspace, variable_list visible_at_end.
- 3dfa5710 audit part 2: assembly_move_instance (the instance list's drops: Reorder instances / Add to folder /
  Move to subassembly), assembly_exploded_view `move` (Reorder : Explode steps), assembly_pattern mirror `strategy`
  (Derived / Transform + Placement per seed; makes the "-Mirrored" Part Studio like the dialog's Update),
  drawing_image (Move to sheet, Image borders); OPEN-ERROR-SILENT part 2 (the start window's open error stays until ×;
  the Open dialog's failures show it); app's image menu and Mirror dialog call the shared ops functions. Removed the
  stale MENUS line sk-dim-tol (skasm's 4c58bbd4 dropped that menu item; coverage:: failed on main).

- fad02aa1 P0 TWO-PROCESS-LOST-EDIT: a Session remembers the microversion its document was read from; when
  another process moved the head, a commit merges this edit's names three-way onto it (tabs feature by feature,
  noshape_model::merge) and reloads, so neither writer loses an edit (live: one workspace, writers append). Test
  session::tests::two_writers_of_one_file_keep_both_edits. Left: (a) two writers changing different fields of the
  *same* feature: merge_part_studio keeps ours for a feature changed on both sides (core's merge.rs could merge the
  feature's params field by field); (b) live shows B's edit in A's open tab within seconds: `Session::sync_external()`
  loads a head another process moved; DocModel::watch_external polls it every 1.5 s (c7928bb4, app-core, cleared by
  the lead; checked in the app: a CLI rename showed in the open window within 4 s).
- Also 10-27: bd14ff0e material_library_update / _export; 5b4b2831 part_annotation move; da75c032 3MF / OBJ welded
  vertices (EXPORT-MESH-SHARED); 71e8357e PDF Text: Selectable embeds the drawing fonts (TrueType subsets, ToUnicode);
  c7928bb4 API-REMOVE-DIR closed per the lead (the dialog's flip, documented); c30baf56 CRASH-MARKER-COPIES (a marker
  applies only to files its dead process had open, `<pid>.docs`).
- bea661f1 Derived carries a source part's explicit appearance (live: red stays red; a default colour is the
  target's next one, as ours already was); query_parts reports each part's `color`; RHINO-GREEN: RHINO is listed (app
  dialog and export_parts) only in builds with openNURBS (kernel::RHINO_EXPORT from the build script's cfg).
- 3b23c8d1 history_list filters (the history panel's search ▾, ops::history_filter shared with the panel); ui_list on
  Variable Studios and collapse_tables. 872c2e2e release_create / release_revision_history: the Release candidate
  dialog's items, checks and commit moved to ops (release.rs, release_settings.rs, release_conditions.rs; the app's
  modules re-export them). Menu coverage: 0 planned items in every area (was 26).
- 771afc0d Extrude add P1 closed with measurements (7–25 ms once std's prewarm has run); I1 narrowed (x_t blend tube,
  spine, edges exact; live's per-face areas requested); flat-pattern line styles need live (requested).
- Agent benchmark 10-27: part → ISO A3 drawing with suggested views → 80 and Ø16 dimensions → note → Selectable PDF in
  1.2 s through the API (/root/dx-scratch/w/py/bench_dwg.py); pdftotext reads every label.

## Paused 10-27 ~09:50 (lead: app split first)
- Landed since the morning handoff: 7b947277 P1 UNDO-NOOP-SHIFT (store: a no-op commit kept the previous undo step);
  b305c5ca API-MATE-RETYPE-NAME; 56c1ee31 Selectable PDF fonts named as live's (AAAAAA+NotoSans); ba3f2c5e
  DWG-DIM-TWO-EDGES (edge-only drawing tools hit-test edges under annotations; API line_to_line fixed, app half to
  re-check on the next green); b8f58d5b the crash-marker test isolated (Session::open_with_markers; main was red).
- Resumed ~11:30 after the app split: CONFIG-CELL-EDIT was landed by psui (e013107b); my WIP dropped. API-ASM-CONFIG-NAMES
  fixed (configuration by input / option names resolved in assembly_insert / set_configuration / replace and drawing
  sources). I1: live's face data says our x_t = live's x_t exactly; only our STEP reader's blends are off (13.1819 vs
  live's STEP 13.2001): next on I1.
- Landing now only through tools/tiers/land.sh (lead 10-27).
- Afternoon 10-27: 0d75aa8e API-ASM-CONFIG-NAMES; a74853db EXPORT-OBJ-MTL (OBJ zip with .mtl, 3MF in metres as live);
  4a905b47 DWG-DIM-TWO-EDGES verified in the green app; 9c187bd1 drawing_annotation_edit orientation / flip_arrows /
  arc_condition and drawing_view_edit cutting_line_angle (shared, the drawing UI calls them); 7bca8284 assembly_snap
  (Snap mode, shared with the assembly view). I1: STEP surface-curve modes ruled out. FLAT-FORM-STYLES filed (needs
  core's flat-pattern data). CONFIG-INPUT-ID waits on live (REQUESTS dx 10-27 P3).
- Evening 10-27: CONFIG-INPUT-ID Checkbox as live's dialog (name required, id from it, default checked) in
  config_add_input (Part Studio and Assembly); the dialogs are psui's (request below). EXPORT-OBJ-ZIP-NAME (the save
  prompt says .zip; zip entries dated). FLAT-FORM-STYLES done: counterbore / countersink heads in the flat (native
  route; a std-built flat, i.e. a std sheet metal feature after the hole, has no heads yet: check next).
- Audit of edit texts (10-27 afternoon): left uncovered on purpose: "Use view geometry" (a step of the drawing sketch's
  constrain tool), "Edit configured dimension" (= config_set_value), "Copy view label" (Alt+drag copy of a label).
- Live answered I1 (behaviours "dx (2026-10-27, P2 I1 …)") and the flat-pattern line styles (REQUESTS dx 10-27 P3): read the latter next.

## Daily walk 10-27 03:40 (green 8165fca6, :131, /root/dx-scratch/w/o*.png)
- No regressions in my area. Open of a truncated file: the error bubble stays (OPEN-ERROR-SILENT verified). Drawing
  from ISO A3 suggested views: first-angle layout, hole centre mark; Export › PDF: 1.5, A3 landscape, 5 layers, Flate,
  92,359 B (live 92,767). Variable Studio: typed "12" → "12 mm". Part › Export… Parasolid 38.1 then + › Import…: the
  dialog offers material density for x_t, Import 1 rebuilds the same part, "CAD Imports" folder. Derived of Part
  Studio 1 into it: dialog, configuration row and the part as DER1/DER10.
- Live answered the derived colour question: the target's next default colour unless the source part's appearance is
  explicit (then carried); ours now does both.

## Evening 10-30 (lead 16:50 order: A41, A44, D15, D40, then the large drawing features)
- Landed: A41 first pass (snap leader, dimension, edge), A44 circular edges, D40 centermark re-attach, V27 Aligned
  section panel (e7030441: Section id, Center / Viewing / Revolved, Show cut geometry only, Edit aligned section...).
- From live 1624 (A41 / A44): GD&T on a dimension's value (centred under it) or line (left edge on it) follows the
  dimension (`GdtExtra.on`, a `GdtOn` at the line's end at the dimension line); GD&T and surface finish inside a face
  region take a dot leader (`Leader.dot`; two clicks); surface finish on an extension line stands square to it, away
  from the value. Still open: a frame dragged off its dimension slides along the dimension line with a leader
  extension; the extension line lengthening to a surface finish symbol.
- V28 broken-out panel: [Up to entity | Blind], ☐ Offset (`BrokenOut.offset`, API `offset`), Blind 25 mm / 1 in
  default (API default too). Still open: Exclude from cut, the Spline / Spline point dropdown, ✓ header.
- CR5: Suggested views of a part with MBD datums keep only the views they need (`mbd::trim_suggested_views`: Front,
  plus Top else Right per datum). MBD dimensions (CR18 / V5) need Part Studio MBD dimensions first (only datums exist
  in `annotations.rs`): not drawing work.
- From live 1625: D15 ✅ (live doesn't foreshorten in detail / crop views either); D40 lost centermarks now disappear
  and come back with their circle (`build.rs` resolve_annotations). Still open (D40): section lines aren't associative
  (live: red line + section view, red dot grip re-attaches and regenerates), GD&T leaders have no reference.
- GD&T / surface finish hover: only view edges and dimensions (pick.rs), since other annotations can overlap an
  extension line (cl1's annotation 3 did). App checks on :147: w3/t30-*, t31-* (t31.noshape).
- Asked live 18:02: BOM Template list / DWT templates / Update properties from a template, Lock drawing properties.
- Landed 441ee2c5 (V28, A41, A44, CR5, D15, D40). Next batch: EX18 image menu (Copy / Paste as table / Move to ▸ /
  Bring to front / Send to back; images in the drawing clipboard), EX17 two-click image placement, V21 partial by
  dragging a cutting line's end inside the part (all checked in the app :147: w3/ex2-*, v21-*).
- Landed 76d8f633 (EX17, EX18, V21). A43 from live 1628: the full Surface finish dialog and symbol
  (`SfSpec` on the annotation; `model-data/src/drawing/surface_finish.rs`, `drawing-ui/src/drawing_sf.rs`); the
  panel now starts empty (live), not "Ra 3.2".
- From live 1627: TB13 / TB18 BOM Template (landed 9375e089); S22 / CR10 / S20: the DWT export carries the
  drawing properties (units resolved), its tab is listed under Create drawing › Created by me (the new drawing takes
  its formats and properties) and in Drawing properties › Select a DWT (checked in the app :147, w3/dwt1-*).
  Open: the DWT doesn't carry the projection (a drawing from it keeps the base template's), its tables.
- Landed c59e86eb (DWT templates), 160b9daf (V28 Exclude from cut), then A44 (the extension line carried on to a
  surface finish on it) + D40 (GD&T leaders on model points are associative: refs slot 100 + leader; lost → that
  leader red with a red dot; end grip re-attaches). Next batch: SK5 (dragging a spline's own point reshapes it),
  V27 flip button (Viewing plane location; preview follows).
- (Since done: A41 drag-off-dimension, D40 section lines, V35; see Night 10-31.)

## Night 10-31 (landed through c8bd8f01)
- Landed: 4760e41a A44 extension line + D40 GD&T leaders; 77e3df39 V27 flip + SK5; 100550a8 A41 drag-off (frame
  slides along the dimension line, leader to the extension line); c8bd8f01 D40 section lines (anchored on the first
  click's model point: follows it, red + red dot when lost, a drag onto a model point re-attaches; resolution runs in
  `views::compute_sheet_views` and `build::assemble_sheet`, the drag compares with a snapshot taken at drag start,
  `DrawingEditor.cut_drag`).
- Then V35 from live 1630: Synchronize breaks from parent (projected views; parent → child only).
- Then D9 (a dragged ordinate value jogs), V35 (d) (sections of broken views; breaks map through the frames), D40
  (the lost section view red as a whole), A5 (GD&T / surface finish leaders publish their paths: menu, nodes, end
  grip; the D40 GD&T re-attach by end grip needed this). A stale dashed rubber band after placing a GD&T or
  surface finish with a leader is fixed (the preview is rebuilt). Asked live: V70 messages, TB28 Change
  configuration.
- Then TB15 (cut list row hover lights its members), D9 push-apart, A5 node drags (any leader), TB20 (hole table
  Flip X / Y, Holes to exclude), V70 (deleted display state / explode: blank views + info notice; live 1638).
  Live 1638 (b) TB28 Change configuration still open.
- Then V27 (Edit aligned section re-picks its points; Exclude from cut on assembly views), S18 (a field's right-click
  menu with Edit note…, its cell text hidden while edited). Landed through e640df77 (06:20 UTC).
- Asked live: D6 side-view cosmetic threads (06:05). Not checked in the app yet: TB15 / TB27 (need a frame
  drawing), V27 Exclude from cut and V28 Exclude from cut (need an assembly drawing), V35 sync breaks.
- Then D6 (side-view cosmetic threads, live 1644; ee163823) and V3 (a named view's section planes: stored on the
  view when placed, cut + hatch + Edit hatch section..., kept when the named view is deleted; live 3121-3127).
- Then A19 (a leader inside a face region lights the region's boundary), V38 / A59 rows were stale (done). Asked
  live: V49 BOM / cut list table references (open).
- Then V42 from live 1645 (typed scales kept: "2:4", "0.5" → "0.5:1"; family scale labels while they differ from
  the sheet; title block SCALE as typed).
- Live 1647 (V49): callouts read BOM items only (a Part Studio drawing's callout is a red "----", cut list or not),
  so A24's cut list item default is gone; View properties shows the table references as live. For core: live's
  Cut list feature makes "Composite part 1" (ours none; the reference row falls back to "Cut list").
  TB15 / TB27 / A24 app-checked on a frame drawing (/root/dx-scratch/w3/frm.noshape, built by CLI).
- Then TB28 (cut list table › Change configuration..., live 1638 (b); w3/frmc.noshape: a frame on a #L-configured
  rectangle). Live 1649 / 1662 answered: D9 knee grip, C11 inference colours + ✕ (done); TB15 row hover tan;
  D13 needs no guide line (ISO inline radial still open); V56 part intersections draw nothing in live (toggle
  only); V55 shaded section / detail (cut face shaded, hatch on top) open.
- Then V56 (part intersections toggle), D13 ISO inline radial + ISO templates' decimal comma.
- Then V55 (shaded sections / details, live 1649).
- Then TB20: Holes to exclude in Hole table properties, X direction (edge pick; X / Y LOC and the indicator turn);
  V35 / TB15 / A44 rows marked done.
- Then C11 "+" marks (done), EX17 image placement inference.
- Then S18 field tooltip.
- Then qa DWG-DATUM-PASTE-RED fixed (d2ee678f): a dangling annotation's fills are red (the datum's triangle), and a
  datum's triangle grip dropped anywhere on a view edge re-attaches (nearest edge point by geometry).
- Left in drawing.md 🔧 (as of 10-31 evening): MBD dimensions (CR5 / CR18 / V5, need Part Studio MBD dims), V1
  "view released items", V27 aligned Cut from section (edge names after a pre-cut), V57 assembly sketches (assembly
  sources carry no sketch wires), V68 spinner, D6 thread lines as dimension picks, D16 (live couldn't start one),
  D24 ISO placement, D40 anchor grip look, A15 custom properties, A23 chips, A28 (needs a multi-instance assembly
  with BOM), A34 datum target details, A56 / A58 per-value overrides, TB12 rich paste, TB14 other sources, TB20
  Y direction / pointer preview / place on ✓, S13 flyout, S15 (app), S18 note menu / editor chips, SK12, EX4, EX11,
  EX17 other documents, V55 decals, CR10, TB13, V25, V28, S20, EX18 (check their notes; several may be done).
- Open in my rows: V57 assembly Show sketches (assembly sources skip sketch wires), V35 (d) sections of broken views,
  D40 hatch / title red, C11 inference points, D9 ordinate jogs.

## Afternoon 10-30 (landed through b76761b6, 16:49 UTC)
- From live 1613: TB5 grid-line drags (the fixed-corner side's row / column, the table grows) ✅; EX18 image menu
  (Image properties..., Switch to <tab>), a deleted image tab marks the drawing out of date and Update drops the image
  with "Failed to resolve image target tab"; menu coverage got "drawing:Image properties...".
- C6 was stale (leader ends re-attach by grip) ✅; V35 dragging a break line moves the break (related views open:
  breaks are per view). TB27 cut list per composite part (compile-checked only).
- Use tools/tiers/perf.sh (vibe3) for any speed numbers from now on.

## Morning 10-30 (lead 07:05 order; landed through fe843162, 08:42 UTC)
- BOM after import (IM1b): not a gap: live's "BOM : <assembly>" element is hidden BOM data, never a tab; our imported
  assembly's BOM has live's names and quantities (matrix row).
- Selectable PDF vs live's anatomy: compact font subset (drawn glyphs renumbered, live's nine tables, CIDToGIDMap
  stream), no unused base-14 fonts: 18.5 → 11.6 kB (live 10.5) (EX7 note).
- Import matrix: SAT I1 fixed (sat_read.cpp: forward line / conic edges keep the file's parameter range; tolerant
  vertices no longer shorten them; plane 384 as live); import toasts as live (IM3). Left: STEP integral (core), g50 F8
  (live keeps an extruded surface) and the McMaster helices (tolerant edges), both inside live's bands.
- drawing.md: V29 red broken-out spline, A64 chamfer content + greyed panel, S33 hole indicator (live 1609), TB15
  row highlights (BOM part edges, hole circles), A20, S30/S31/S32, TB5 double-click; 62 🔧 left.
- Live requests open: 08:38 EX18 image menu / TB5 cell edges. Most remaining 🔧 rows need live data or are larger
  features (MBD in views, section dialogs, surface finish dialog, DWT templates, BOM templates).

## Night 10-30 (lead 23:45 order: drawing.md 🔧 rows → import matrix → data exchange)
- Landed: the Keyboard shortcuts modal is app-core's `shortcut_sheet` (Drawing / Part Studio / Assembly, start tab
  per studio; psui told in psui.md); SK16 sketch Thickness / Pitch scale; S9 + TB30 drag view / table rows onto
  another sheet's row; S3 continuation sheets (ANSI A from drw2-l, ISO A3 from live 1603: the same strip, scaled with
  the title block height); A50 / A51 centerlines (free clicks, ends snap, end grips, middle-snap move; ops
  centerline_end / move_centerline); S29 (a set scale ticks Scale label; default Off as live); V48 multi-line Prefix /
  Suffix; A55 centermark arm edge snap + red diamond; A58 hole callout shaft hidden; promotions V23, D14, D37, D41,
  A4, A17, A21, A26, TB29, C9, S24, SK13, S28, TB17 n/a.
- Import matrix (docs/parity/workflow-import-matrix.md "Re-run 2026-10-30"): 128 / 143 samples identical to qa's
  10-28 run; the 14 dx-asm-xt files and the NIST assembly now import through the CLI (= live's tabs, names, 36
  instances; IM1b BOM element missing, api); I1 graded per face: x_t ✅, SAT ✗ small, STEP ✗ = the integral of the
  degenerate blend patch (the file's patch integrated independently = live's 13.20006; core's, TICKETS I1).
- Live requests: 1603 answered (ISO continuation, Reset ordinate = nothing, no BOM override in drawings, Scale label
  default); new (03:35) A51 concentric arcs, TB4 table midpoint grips, D16 single-silhouette foreshorten.
- Flaky, for core (10-30 06:17): the watcher reverted c48f3aea (S32 sketch-driven dimension colour, TB5; drawing
  crates only) for `custom_features::std_features_match_onshape` failing on main (custom_features.rs:1873). With the
  commit re-applied on main the test passed 7 of 7 runs (`cargo test -p noshape-model --test all
  custom_features::std_features_match_onshape`, a fresh NOSHAPE_CONFIG_DIR each), so it is the nondeterministic
  result core is chasing in that file; re-landed as is.
- Next: the remaining 🔧 rows that need no live answer (V49 BOM / cut list references, A31 detail-view radial datums,
  SK15 constraints, C10, D40 red grips), then the data-exchange list above (Next 0, 3).

## Night 10-28 (landed through acc43c43; walk on green 52ca0b97, :131, /root/dx-scratch/w3/a*.png)
- Walk 03:15: hole table on a 3-hole part (Ø6.6 THRU ×3): TAG / X LOC / Y LOC / SIZE "⌀6.6 THRU" / QTY 3 merged;
  the tag axis' menu (Hole table properties..., Flip X / Y | Clear selection, Zoom to fit), Flip X negates X LOC;
  table menu › Hole table properties... opens the dialog with the table's values, untick Show location data › ✓
  drops X / Y LOC in one edit (one undo restores). Fixed from it: "Location precision" wrapped (label 98 px).
- Landed: EX16 Insert DXF / DWG text as editable notes (da079ca2); D34 the palette's dual row (per-dimension dual
  precision / units / Show-hide units, DXF too) and D17's wording (0637de45); V48 section / detail labels' own
  Prefix / Letter / Suffix (3062ec50, drawing_get views' `label_text`); qa XT-ASM-SCALE (P1), HOLE-TABLE-FILLETS,
  HOLE-TABLE-MENU (acc43c43; TB21 ✅).
- With this: qa ASM-PARASOLID-FLAT: assemblies exported as Parasolid 38.1 keep ASSEMBLY / INSTANCE / TRANSFORM nodes in
  live's layouts (from tests/perf's NIST x_t); older versions stay flat until live sends an assembly in each
  (REQUESTS dx 10-28). Then BALLOON-WRONG-ITEM, DRAWING-SHIFT-SLASH, the drawing.md rows in P2 order.

## Afternoon 10-29 (landed through 8d3bc80b; 16:14 UTC)
- qa tickets fixed: DWG-CONSTRAINT-PRESELECT, DWG-SHEETS-ROW-RENAME (view rows select; Shift+N "Rename" popup beside
  one row), DARK-DRAWING-PANEL-CONTROLS, DWG-PROPERTY-FIELD (editor shows values, tokens kept: `NoteFields`),
  DWG-GDT-MENU-ORDER, DWG-DATUM-TARGET-MENU (A1, Origin active), DWG-TABLE-SHIFT-EXTEND, DWG-SHEET-DRAG-GHOST,
  DWG-INFO-TOASTS, DWG-LINE-MENU (sketch `SketchStyle.layer`, Formats pen; Linestyle / Color in the Styles panel),
  DWG-PALETTE-VALUE-EMPTY (already fixed by D39), then DWG-CHAMFER-ANGLE-PRECISION, DWG-CELL-MERGE-GREY,
  DRAWING-SCALE-LIST-NARROW.
- Outside dx's crates, small and needed by those: vendor/gpui-component `Theme::global` gives a light copy inside
  `with_light_colors` (`LightTheme`, set by crates/app main.rs `apply_kit_theme`); app-core os_menu greys an empty
  submenu.
- The drawing editor filled its SheetInfo (document / tab names) only on the first edit: properties read "----"
  right after opening; now at once.
- Evening: DWG-NOTE-DIAG-FRACTION, AGENT-ELLIPSE-DIM-VALUE, DWG-LINE-COPY, DWG-GROUP-ANNOTATION-RULE,
  DWG-TOOL-HINTS-PREVIEW (Datum / GD&T previews), DWG-CIRCLE-FACETS, DWG-E2E-SILHOUETTE residual (edge tools hover one
  edge), DWG-SHEET-PROPS-ESC, DWG-TABLE-ESC-GHOST, API-ASM-RELATIONS; D24's dragged value (`Annotation.text_along`).
- Later: DWG-DATUM-PASTE-RED (`Annotation.loose`; a datum's triangle re-attaches by drag), DWG-EDIT-SECTION-DIALOG's
  tool button; drawing.md D17 (foreshortened diameter flip node), D21 (checked), D36 / A58 (fit class visibility
  toggle), V26 / V59 (live needs nothing more), A22 (callout on a centermark), A24 / A23 (cut list item default,
  `{cut.*}`; not checked on a frame part yet), A25 (callout menu order).
- Left in dx's TICKETS: DRAWING-SHIFT-SLASH (asked the lead), VERSION-FOOTER-STALE (release process).
- Local `cargo test` runs get a fresh NOSHAPE_CONFIG_DIR (/root/dx-scratch/env.sh, lead 10-29).
- Night: drawing.md A27 (callouts group onto callouts / symbols / datums, also by clicking while placing; only
  ungrouped targets, else a group cycle drew both at the origin), A28 (stacked instances reorder; not app-checked),
  A31 (datum on hole callouts), A32 (area size precision), A33 (datum target in empty space: a SHEET_VIEW point).
  Live request filed for V60 (view Show/Hide constraints). Then A59 (no replicate onto the source view), A63 (painter
  carries Styles-panel overrides), A64 (Styles panel: Datum size; callouts' Stack direction / gap), A57 (stale note:
  side-by-side dual exists), D13 (ANSI radius / diameter values dropped inside the circle).
- Next: D24's ISO placement (live request asb 10-25 a), A24's frame-part check, A28's app check, V60 when answered.

## Morning 10-29 (landed through 8eabd3f5)
- qa tickets fixed: DWG-ROTATE-VIEW-DIMS-ZERO (edge dimensions turn with the view: `set_view_rotation`),
  DWG-DATUM-PASTE-ATTACHED (pasted datum stands free; not red, as ours can't re-attach a datum by drag yet),
  DWG-ANGULAR-LABEL-ANGLE (`text_angle`, arc carried round), DWG-ARC-REF-FULL-CIRCLE, DWG-RULER-SELECTOR-CLOSES,
  DWG-GROUP-ANNOTATION-MENU (multi-selection Group annotation / Group with a view), DWG-HOLE-CALLOUT-ADD-LEADER (extra
  leaders, live's menu), DWG-PROPERTY-FIELD half (inserted properties All uppercase; the editor still shows the token),
  DWG-NOTE-LEADER-ARROW (= live: arrowheads at sheet size, live's answer 1588).
- drawing.md D23 (placement inference to view vertices / centers; it no longer aligned with itself), D22, D26, D31, D33.
- Next: DWG-PROPERTY-FIELD's editor (fields shown as values), DWG-SURFACE-FINISH-MENU, DWG-CHAMFER-NOTE-LANDING,
  DWG-EDIT-SECTION-DIALOG, DWG-DIM-HOVER-STAYS, DWG-CENTERMARK-ALIGN, the E2E-SILHOUETTE hover residual.

## Night 10-28 / 10-29 (landed through the D13 centermark commit)
- qa tickets fixed: DWG-ANGULAR-SECTOR (reflex opposite, continuation line, sector fixed at placing),
  DWG-DANGLING-WALL (wall thickness references its circles: slots 10 / 11), DWG-FORESHORTEN-JUMP (no Foreshorten in
  the radial menu, as live), DWG-MAXMIN-LINE (max / min to a straight edge, square to it: slots 2 / 3).
- drawing.md D2 (Ø between a cylinder's silhouettes: silhouette lines are pickable), D15 (Diameter on one silhouette),
  D13 (radial / diameter take a centermark).
- D22 (grips snap to midpoints / quadrants), D26 (radial / 3 point angular / ordinate re-attach by their blue grips),
  D33 (live has no Count type: matches), D31 (row was stale: the launcher exists).
- App checks now run on dx's own Xvfb :147 (/root/dx-scratch/xvfb147.pid): another agent's app (noshape-green-new)
  opens windows on :131.
- Next, in order: D23 placement inference lines; D24 a value dragged past an extension line; D21 centre-to-quad for
  the other kinds; V59 / V26; drawing.md ⏳ rows on green.

## Late afternoon 10-28
- D39 on every dimension kind: a typed value replaces the measured one after its symbol (`DimFormat.value`,
  `overridden_text`; drawing_edit_dimension `value`), landed 2e44d382.
- V53: views copied in another document / workspace are refused with live's bubble (stays until ×); EX11's start
  banner closes after 2.5 s; the bell for the finish notification is asked of the lead.
- S6: Sheets panel › reference › Change configuration... (live's panel; every view of the reference; the API's
  drawing_change_configuration), checked on :131.
- S8: sheet rows drag to reorder (one or several; blue drop line; one undo), checked on :131.
- qa TABLE-SHIFT-TAB-BLANK (caret after a cell's text), NOTE-PANEL-UNDER-PROPS (below the table when no room above);
  A35 datum targets: no dimensions until Show dimensions (label menu), boxed, Hide dimension on the dimension's menu.
- D18 arc length conjugate (the side the pointer is on); D11 line to line angular by the placing sector (θ, 180° − θ
  with the continuation line, reflex opposite), from live's answer 1576.

## Afternoon 10-28 (landed through 3476703d)
- Parasolid: 30.1–31.1 ASSEMBLY layouts from live's samples (all 13 versions read); exports carry part names / colours
  and the assembly's name as Onshape's TYSA_NAME / TYSA_COLOUR_2 attributes in every version (5bb31dca).
- drawing.md: S25 dimension Text justification, S19 format-layer thickness for moved entities, D10 (ordinate group red
  as a whole; test only), D39 overridden dimensions keep prefix / suffix / above / below, A44 Surface finish keeps
  placing until ✓ (checked in the app on :131).
- Asked live (REQUESTS dx 10-28): EX11 finish notification, V53 paste across workspaces, S6 Change configuration.
- Next, in order: D39's override on non-linear dimensions (needs an override field beyond LinearDimension);
  S8 sheet row drag (live answer pending); A35 datum-target dimension / label menus (needs their own picks + live's
  menus); TABLE-SHIFT-TAB-BLANK; V59 / V26 once live data exists; drawing.md ⏳ rows to verify on green.

## Midday 10-28 (landed through ec54591b; checks on green 6570fa0d, :131, /root/dx-scratch/w3/b*.png)
- Green: the dimension palette's dual row as live asbdd-9 ([ 3.15 ▾ (in) ▾ 👁 ], the eye shows "[3.15 in]"); in Dark mode
  the drawing toolbar is white over its whole width (DARK-DRAWING-TOOLBAR).
- Landed: DWG-REVOLVE-SEAM (P1), VIEW-LABEL-TOGGLE, DRAWING-BOX-SELECT (qa verified all three), XT-ASM-VERSIONS (all 13
  of live's per-version assemblies read; ours write in every version), DATUM-SCALE-DETACHED, DARK-DRAWING-TOOLBAR,
  OBJ-ZIP-MSG, drawing.md EX10 (strikethrough by DXF version), EX11 (export banner), TB9 (corner double-click).
- Asked live: EX11's finish notification, V53 paste across workspaces. Next: ASM-XT-ROUNDTRIP-NAMES (names as
  Parasolid attributes: ATTRIB_DEF / ATTRIBUTE / CHAR_VALUES per version), drawing.md rows in P2 order.

## Morning 10-28 (landed through 91dec38b)
- qa tickets fixed: BALLOON-WRONG-ITEM (a callout on a silhouette is that part's: the nearest visible edge decides,
  the line of sight's closest body for silhouettes), ASM-PARASOLID-FLAT (verified by qa 06:08), CALLOUT-ON-DIMENSION
  (attach on a dimension's lines / text, leader follows, deleted with it), CALLOUT-ALIGN-MISSING (multi-selection
  menu), DIM-PALETTE-DROPDOWN-COLLAPSE (the launcher's palette stays open while its own list is), NOTE-PANEL-UNDER-PROPS
  (the Note panel stays left of Drawing properties / right panels). drawing.md V43 ✅.
- Open: TABLE-SHIFT-TAB-BLANK (P3): the cell left with Shift+Tab draws blank until editing ends (both keys go through
  `cell_step`, which commits first, so it is in the drawing of the in-place cell; not reproduced yet), and a
  re-entered cell's caret should go after its text (TextareaState has no caret setter used here yet).
  DRAWING-SHIFT-SLASH: request below. Balloon 23 of qa's file: its anchor is 6.4 mm from any edge, so "----" is right
  for where it landed.

## Evening 10-27 (landed through 24a0e15f)
- Speed: Parasolid read 35 → 3 s; x_t assemblies import as live (Assembly + "<file> parts"); live's NIST exports in
  tests/perf; dx_perf::timed_nist_import (STEP 41.8 s / x_t 48.0 s dev, loaded; live 38.7 / 24.5 s) is held back by
  the display meshing lock (request below). Bracket drawing 178 ms vs live 30.1 s.
- Drawings: Adjust linestyle in live's lengths, centred per edge (screen, PDF, SVG) and as LINE + LTYPE in DXF
  ($LTSCALE 0.5); title block as live maps it (drawing's own Part number / Revision, part's Title 1–3, date drawn;
  op drawing_element_properties); template sizes by standard; formats unlocked on new drawings; Detail profile
  lines Phantom; palette select lists widen to their rows; walks of views / dimensions / Drawing properties
  (drawing.md: V36 V37 V39 V51 V66 V67 V72 D28 D32 D35 D38 verified, S29–S33 notes refreshed).
- Live asked (REQUESTS dx 10-27): none open.
- Late 10-27 / early 10-28: Drawing properties defaults vs live's new dumps (template sizes by standard, unlocked
  formats); cut list "Split cut list table below" / "Unsplit cut list table" and Cut list table properties › Sheet;
  hole tables as live (QTY, SIZE / QTY merged per size, SIZE = the Hole feature's callout, Show location data,
  Order), the Hole table dialog and Hole table properties… (drawing.md TB20, TB22, TB28, TB29). Next on tables:
  hole table X / Y direction edges, Holes to exclude, the pointer preview; the tag axis' own menu (TB21).

## Drawing walk 10-27 afternoon (green 0537ac91, :131, /root/dx-scratch/w2/s*.png)
- Verified ✅: V36, V37, V39, V51, V66, V67, V72 (drawing.md). Fixed from it: Rotate view ▸ on views whose alignment
  is suppressed (one rule `Sheet::turns_freely` for the menu and the API), live's rotate guide (no bubble); crop
  profiles close on a click on their first point, and the first point no longer hands the crop to a neighbour view.
- Open: Adjust linestyle's Dashed looks shorter than live 446 (asked live). Move to sheet shows a "Moved to Sheet2"
  bubble and switches sheet (live: not described).

## Speed (PLAN 2.7: ≥ 2× faster than live; crates/ops/tests/dx_perf.rs, `--ignored --nocapture`)
- Drawing of the qa5 bracket with the 4 standard views (drawing_create, Suggested views: every view computed):
  **178 ms** median of 5 (first 360 ms), dev build, 10-27 14:15; one more Front view 13 ms. **Live: 30.1 s** to all
  four views drawn (server part 3.0 s), Insert view 3.0 s (server 1.3 s) (behaviours "dx speed: drawing creation and
  NIST import"; their box was loaded). Way past 2×. Release number: next perf.yml `model` run.
- NIST assembly import, live's own exports (tests/perf/nist-mtc-crada-assembly.step / .x_t): **live STEP 38.7 s,
  x_t 24.5 s** to done (32.5 / 21.3 s to the Assembly tab). Ours, CLI import_files (dev, machine at load ~45):
  STEP 37–44 s, x_t 59–74 s, ~50 s of it display meshing serialized on the kernel's global mesh_mutex (request
  below); target ≤ 19 / 12 s. dx_perf::timed_nist_import (dev, load ~45, Assembly + 8 parts as live, and since
  10-28 the box's 152.4 mm): **10-28 03:50: STEP 46.9 s, x_t 20.9 s** (x_t was 48.0 s: its parts came in 1000× too
  small, qa XT-ASM-SCALE, and meshing them was the slow part; fixed acc43c43). x_t of an assembly now imports as live (Assembly + "<file> parts"). Earlier, on our own export of tests/perf/nist-mtc-crada-assembly.noshape (36 parts, STEP 11.9 MB), kernel only
  (C++ -O2): read STEP 9.3 s, write x_t 8.1 s, read x_t 35 s → **3.0 s** (10-27: attributes indexed once, bodies
  healed in parallel, second fins' SP-curves matched in parallel; same volumes / faces bit for bit). STEP next. Parasolid export of that
  Part Studio crashed the kernel on 3 parts (OCCT 8.0.1 SweptToElementary null face): fixed f2279b3c.

## Next (in order, after landing)
0. Import numbers: the remaining ✗ rows of docs/parity/workflow-import-matrix.md (g50 F8 / F19, McMaster helix) and
   I1 (x_t blend trims). (Configurations C1–C8, Derived DER1–14, Variable Studio VS1–4: all closed in the docs.)
1. Audit done ("Move annotation" = part_annotation move, landed). Covered elsewhere:
   Move configuration input (config_reorder_inputs), Hide all instances (assembly_set_state on every instance), Save
   custom color (any colour is settable), Update material library (material_library_update, this landing).
2. API: benchmarks done for Variable Studio, in-context, sheet metal (/root/dx-scratch/bench/*.py); next: drive a model
   through the MCP server with a real client; render_image of the window camera (image size).
3. Drawings (P2): PDF Selectable text landed 10-27 (embedded TrueType subsets, CID fonts with ToUnicode; same
   placement as the outlines); open: compare its file size / font object layout with a live Selectable export; ui2's
   Tables-end and thread tiles checked vs live 10-27 (match); flat pattern Counterbore / Countersink / Form styles.
4. Re-verify old rows on the green build: workflow-configurations.md C1–C8, workflow-derived.md DER2–DER14 (DER8 works
   through the API; check the app's Insert panel), workflow-drawing.md D14/D16 (D8, D12 verified 10-27).

## Requests to the lead
- (10-29 16:40) DRAWING-SHIFT-SLASH: done 10-30 as decided (the sheet is app-core's; psui told in their handoff).
- **land.sh and the 3964a7cf revert (10-29 06:44)**: land.sh did not miss it. The change touched model-data, so its
  crate list (reverse dependencies) included noshape-ops, and vibe3 ran noshape-ops' full tests (e2e included) green. All
  116 ops e2e also pass here at fd102784 (main with the change). Related risk, not today's cause: the ops e2e tests
  (`--test all`) write the real ~/.config/noshape (graphics.json, *_toolbar.json, release settings, keyboard shortcuts,
  sketch-options.json, recent.json): only tests/prefs.rs sets NOSHAPE_CONFIG_DIR. Two e2e runs on one machine (the
  watcher and an agent) share those files. Setting NOSHAPE_CONFIG_DIR to a per-process temp dir for the `all` binary
  (or in t0.sh / t1.sh / land-checks.sh) would isolate them.
- **v0.4.13 (relayed from bool, 10-29)**: tag v0.4.13 = 5e56477b has the Rust Boolean regression bool 2520afc4 that
  c79eed1d fixes (halfmir2.py variant B: 7869.1 mm³ with a −78.54 mm² face, live 7607.301). bool's options: cut from a
  green with c79eed1d, or ship with NOSHAPE_BOOL_RUST off by default. Details: TICKETS RUST-MIRROR-EXTRA-FACE, handoff
  bool.md. (The lead wasn't a live session; psui and dx couldn't reach it directly.)
- **Notifications bell (drawing.md EX11, P3)**: live reports a finished export only in the Notifications bell
  (bold "Export completed"; expanded "Part 1 Drawing (3).pdf was exported successfully."; no toast; behaviours EX11).
  The app has no bell or notification list. It belongs to the window chrome (crates/app), so yours: a bell with a list
  that editors push to (title, body, time). dx then pushes export results from the drawing editor.
- **Version in non-release builds (qa VERSION-FOOTER-STALE, P3)**: crates/app/build.rs shows CARGO_PKG_VERSION
  (0.4.6) + the commit, so green / CI builds read three releases old. Bump the workspace version with each release
  tag, or let build.rs take the last tag (`git describe --tags --abbrev=0`) when it is newer. Release process, so yours.
- **Shift+/ in a drawing (qa DRAWING-SHIFT-SLASH, P3)**: the Keyboard shortcuts modal (tabs General … Drawing) is
  rendered by the Part Studio view (part-studio-ui view_tools.rs `render_shortcut_sheet` / `toggle_shortcut_sheet`,
  psui's crate) and the Assembly has its own list; the drawing editor has nothing for `ToggleShortcutHelp`. Live
  opens the same modal everywhere, on the current workspace's tab. Proposal: move the modal into app-core as a
  component (state: tab, query, scroll) that each editor shows with its start tab (Drawing for drawings); dx can
  wire the drawing side once it is shared, or do the move if psui agrees.
- App (crates/app doc_dialogs.rs `tab_properties`): a Drawing tab's Properties should also edit the drawing's own
  Part number, Revision, Description and Title 1–3 (`Drawing.meta`; the API's drawing_element_properties). Live: the
  title block's DWG NO. / REV come from them (behaviours "dx title block mappings").
- **disk-loop.sh breaks builds in busy target dirs** (3 of dx's land.sh runs on 10-27 failed: "failed to write
  …/.fingerprint/…/invoked.timestamp", "could not parse/generate dep info …noshape_app_core-….d", "unable to copy
  …/incremental/…-working/….o"): its 30-min pass runs prune-build.py / prune-deps.py / fix-fingerprints.py and the
  incremental `find … -mmin +30 -exec rm -rf` on every target dir, without the `busy` check the floor step has.
  Please skip a dir while a cargo uses it (`busy "$t" && continue`), as the floor does. Also: fix-fingerprints.py
  only checks that some artifact of a crate is left, so an `.rlib` whose `.rmeta` was pruned keeps its fingerprint
  and cargo fails "extern location … .rmeta does not exist" (dx 22:56: 42 crates of target-dx; dx reset them by
  hand). Checking `.rmeta` and `.rlib` separately would catch it.
- **Display meshing runs one part at a time** (kernel mesh.cpp, core's area; PLAN 2.7 import target, also regen):
  mesh_prepare holds the global `mesh_mutex` around BRepMesh and mesh_fill takes it again for BRepTools::Clean, so
  `regen::display::mesh_in_parallel`'s 36 jobs of the NIST import queue on it: NOSHAPE_KERNEL_PROFILE summed
  mesh_prepare 396 s / mesh_fill 311 s (waiting) / mesh.brepmesh 46 s, ~50 s wall of a 60 s import. The lock
  guards triangulations BRepMesh writes into shared TShapes (instances). Proposal (dx can do it if you clear it,
  else core): mesh a private topology copy per job (`BRepBuilderAPI_Copy(shape, false, true)`, same MapShapes
  order, geometry shared read-only) with no lock; the original then never carries a triangulation, so the Clean
  (and its lock) goes too. Expected: the import's meshing ≈ 46 s / cores.
- I1 (P2) is core's now: the kernel's face integrator under-reports our STEP blend faces (Gauss 13.18188 vs a fine mesh
  13.19411 on the same face); dx ruled out the reader and the geometry (TICKETS I1 has the numbers).
- assembly-ui (after your move settles): assembly_mate_dialog.rs set_type renames a default name inline; it can call
  `noshape_model::default_names::retyped(&self.name, old, new)` (the same rule the API now uses), one copy.
- App (after the app split): crates/app/src/part_studio/sheet_metal_panel.rs has the same logic as the new
  `noshape_ops::sheetmetal` (joint_rows, joint_edit_index, insert_joint_edit, bend_faces, bend_row_order, plain, the
  Radius / K factor edit, Convert to rip, Move up / down); its handlers should call that module (dx can do it once the
  app crates are settled). Until then the two copies must change together.
- App (after the app split): crates/app/src/doc_dialogs.rs outdated_references / apply_update are now
  `noshape_ops::workspace_update::{outdated, apply}`, and config_panel.rs's "Unconfigure parameter" is
  `noshape_ops::config::unconfigure_parameter`: switch the app to them (same logic, one copy).
- App (after the app split; 1 line each): crates/app/src/part_studio/in_context.rs:788 Update context should call
  `noshape_ops::in_context::refreshed_in(&old, &result, asm, None, Some(<this Part Studio's tab>))` (a Part Studio
  created in context then updates like live; today the app keeps refusing it), and the app's out-of-date dot should use
  `out_of_date_in(.., Some(tab))`. The API already does (dx, e2e part_studio_created_in_context_updates).
- psui (part-studio-ui config_panel.rs, assembly-ui assembly_config.rs): live's "Checkbox" button opens a create
  dialog (Name, empty and required; Default, checked) and the id comes from the Name ("My flag" → My_flag). Ours adds
  "Checkbox N", unchecked, at once. The Part Studio's dialog should call `noshape_ops::config::add_checkbox_input(ps,
  name, default)` (the API's config_add_input does, CONFIG-INPUT-ID); the Variable Studio's dialog already matches.
- RHINO-GREEN: please build the watcher / green app with `OPENNURBS_ROOT=/root/opennurbs` (release.yml builds with it,
  dx's builds too). Hiding RHINO in builds without it needs a kernel build.rs cfg, i.e. a C++ rebuild for everyone; not
  worth it unless you want it.

## From skasm (10-31): UNDO3, a drawing note takes two undo steps in live

workflow-undo.md UNDO3 / behaviours "qa5 undo across tabs".
- Live: after placing a note, the 1st Ctrl+Z leaves the note and enables Redo; the 2nd removes it.
- Ours removes it in one step.

Live's note creation seems to be two history entries (place, then the text edit). Yours if you want to match it; I didn't touch the drawing code.
