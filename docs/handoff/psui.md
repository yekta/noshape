# Handoff: psui (Part Studio UI), from 2026-10-26

Worktree /root/projects/noshape-wt/psui, branch agent/psui, target dir /root/target-psui, app display :102.
Scratch: /root/psui-scratch (check2.sh = clippy + covering tests + T0, detached; mkskip.sh builds a Skip-instances test
document with noshape-cli; ink.py measures plane-name ink boxes). Live: my own tab is the old ui tab
(`UI=1 node r.mjs …` in /root/live-pw, target id in /root/ui-scratch/live/.ui-tab); captures in onshape-research/live/psui/.
Run the app at live's size: `xdotool windowmove <w> 0 0 windowsize <w> 1919 1079` (planes then sit on live's pixels).

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
- perf.yml on main dispatched 10-26 23:45: 40421862773 (live-30 edit/undo/redo), 40421870988 (fins-400 orbit 2x frames),
  40421880164 (live-30 cold start). Read the summaries into viewport-performance.md.

## Speed targets (PLAN 2.7), current numbers

| target | goal | current (where measured) |
|---|---|---|
| orbit / pan / zoom, NIST assembly at Retina | 120 fps | presented 30.5–31.3 fps on the paravirtual macOS runners (their timer caps it; perf.yml 40493040035, v0.4.10, 1194×794 device px, not Retina); UI CPU p95 0.4–0.9 ms; GPU time not reported on Metal (no timestamps) |
| cold start to first frame | ≤ 1 s | Metal shader cache cleared: 1.34–2.09 s (pipelines 0.50–0.98 s); warm cache 0.48–0.83 s; a job's first launch 2.5–3.4 s (runs 40421880164 / 40421862773) |
| open a document | ≤ 0.5 s | app to document window 0.34–0.55 s + window to first frame 0.55–0.95 s cold (viewport-performance.md 10-26) |

## In progress

(nothing unlanded)

Residuals found today: a paused dialog's Depth field
shows a corner mark in live; Shift+wheel under xdotool loses events after the first (GPUI X11 resets the scroll
valuator on XI2 DeviceChanged from XTest's slave switches; real wheels fine); the banner's text is 5 px narrower
than live's Flama.

## TAB-NO-PREVIEW: closed (test-display artifact)

On Xvfb without a window manager the app window never gets X input focus, so GPUI treats it as inactive and sends no
focus / blur events; nothing blurs, so nothing commits. **Always `xdotool windowfocus <window>` after starting the app
on :102** (W=$(xdotool search --name noshape | head -1)). With focus, Tab commits and previews as live. Fixed alongside:
a Tab commit scheduled two regenerations.

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

1. Our base part shading differs a few units from live's (block top (140,185,212) vs (145,181,203)), which is what
   the preview samples still differ by.
3. Cold start first launch 2.5–3.4 s (budget 2 s): wgpu-hal isn't vendored; a per-(source, options) MTLLibrary cache
   needs vendoring wgpu-hal 29, or smaller per-pipeline shader modules (naga's MSL writer emits the whole module per
   stage). perf.yml without `-f ref=` measures the latest release: run 40464678503 (v0.4.8) failed its gates on
   the paravirtual macOS runners (NIST assembly texture "orbit" 33.2 → 19.3 fps; that log is the pan run, kind
   mislabelled; the orbit log reads 30.8 fps). Run 40467324821 (`-f ref=main`, 07:28) measures main: read its
   summaries into viewport-performance.md and compare.
4. interaction.md 🔧 rows left: 4.1, 4.2, 4.8, 4.9, 5.1, 5.3, 6.2–6.4, 6.8, 2.3, 2.6.
5. p1-dialogs: checkbox label widths differ by font (Flama vs Roboto); nothing to do short of a font change.

## Requests to the lead

(none; 4e55b01f is in v0.4.10)
