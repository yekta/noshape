# Handoff: psui (Part Studio UI), from 2026-10-26

Worktree /root/projects/noshape-wt/psui, branch agent/psui, target dir /root/target-psui, app display :102.
Scratch: /root/psui-scratch (check2.sh = clippy + covering tests + T0, detached; mkskip.sh builds a Skip-instances test
document with noshape-cli; ink.py measures plane-name ink boxes). Live: my own tab is the old ui tab
(`UI=1 node r.mjs …` in /root/live-pw, target id in /root/ui-scratch/live/.ui-tab); captures in onshape-research/live/psui/.
Run the app at live's size: `xdotool windowmove <w> 0 0 windowsize <w> 1919 1079` (planes then sit on live's pixels).

## From dx (10-30, lead's decision)

- The Shift+/ Keyboard shortcuts sheet moved to app-core (`crates/app-core/src/shortcut_sheet.rs`, `ShortcutSheet`,
  registered hooks for the key chip / Preferences links from crates/app main.rs) so the Part Studio, assemblies and
  drawings share one copy. In the Part Studio: `show_shortcuts` and `ui.shortcut_*` are gone, replaced by
  `shortcut_sheet: Option<(Entity<ShortcutSheet>, Subscription)>`; `toggle_shortcut_sheet` (view_tools.rs) opens it on
  General as before; part_studio_hooks lost `show_keyboard_shortcuts`, `show_shortcuts_page` and `key_chip` (only the
  sheet used them); the test `shortcut_sheet_lists_every_part_studio_key` moved to app-core. Behaviour unchanged
  (checked on :147: tabs, Shift+/ and Esc close, click outside closes). Further sheet changes go in app-core.

## Landed on main

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
| open a document | ≤ 0.5 s | v0.4.11 warm: opening → first viewport frame 130–200 ms ✓; first launch 800 ms |

## In progress

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

N1. Open live answers still pending: interaction 4.9 (Measure asterisk), 6.2 (Section arrow vs Extrude arrow crops).
N2. part-studio 🔧 rows to compare with live captures: 3.19 (Create selection, part-studio/380–398), 7.7 (parts list
    properties: ask live), 10.1, 11.5–11.7.
N3. Completion residuals: Onshape's module order for equal-cost rows (BooleanOperationType sits before
    NewBodyOperationType in ours); Shift+Tab in a snippet (untested live).

0. Live answer pending (REQUESTS psui 10-27, P2 interaction 4.9): when the Measure readout puts an asterisk after
   a value (live saw "Min dist: 0.000* in" with two picks on a mesh, none for mesh + Origin). Apply when answered.
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
4. interaction.md 🔧 rows left: 4.1, 4.2, 4.8, 4.9, 5.1, 5.3, 6.2–6.4, 6.8, 2.3, 2.6.
5. p1-dialogs: checkbox label widths differ by font (Flama vs Roboto); nothing to do short of a font change.

## Requests to the lead
- (10-31 12:01) For skasm: ops e2e `sheet_metal_table_ops` steps (Bend A radius 3 → Move up → Convert Bend B to rip) leave
  the model, in about half the runs, with **no bends and only "Joint A"** (the other half: Bend A + Joint B). The test
  didn't check the table there so it passes either way; ids are random per run, so std's joint matching looks
  order-dependent. Also under std a converted rip's `Rip::faces` is empty (no entity for a new Modify joint).

- (10-31) For skasm (sheet-metal model), from the chrome-sweep rows I own: **SM4i** the Other joints Type ▾ / Style ▾
  need Modify joint to restyle an existing rip (now "The joint is already a rip") and to turn a rip into a bend from
  the table; **SM4k** the flat pattern is one face (live: a face per wall and per bend zone, with the zone edges), so
  hovering lights the whole flat. psui adds the table dropdowns once the model takes them.
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
