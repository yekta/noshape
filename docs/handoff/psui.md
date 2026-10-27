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
- perf.yml on main dispatched 10-26 23:45: 40421862773 (live-30 edit/undo/redo), 40421870988 (fins-400 orbit 2x frames),
  40421880164 (live-30 cold start). Read the summaries into viewport-performance.md.

## In progress

(nothing unlanded)

Residuals found today: live outlines the Delete part ghost (255,192,76) (ours none); a paused dialog's Depth field
shows a corner mark in live; Shift+wheel under xdotool loses events after the first (GPUI X11 resets the scroll
valuator on XI2 DeviceChanged from XTest's slave switches; real wheels fine); the banner's text is 5 px narrower
than live's Flama.

## TAB-NO-PREVIEW: closed (test-display artifact)

On Xvfb without a window manager the app window never gets X input focus, so GPUI treats it as inactive and sends no
focus / blur events; nothing blurs, so nothing commits. **Always `xdotool windowfocus <window>` after starting the app
on :102** (W=$(xdotool search --name noshape | head -1)). With focus, Tab commits and previews as live. Fixed alongside:
a Tab commit scheduled two regenerations.

## Landing checklist addition (10-27)

A new or renamed menu label needs its crates/ops/src/coverage.rs MENUS line: run
`cargo test -p noshape-ops --test all coverage::` with clippy / T0 (79460041's "Cancel" turned main red; skasm fixed
it in 25dc002f). /root/psui-scratch/check22.sh runs it now.

## Build times (10-27, load ~30)

App edit → `cargo build -p noshape`: 60–78 s. A vendor gpui-base / gpui-pre-linux edit → app: ~3.5 min. After a rebase
that touches the model: ~12 min.

## Next, in order

1. Delete part ghost outline (255,192,76). Dihedral panel
   (item 5). part-studio.md rows done 10-27: 1.12, 1.18, 1.19, 1.22, 1.27, 2.11, 2.13, 2.16, 2.17, 2.18, 3.6, 3.11,
   3.14, 3.16, 4.4, 4.9, 5.22, 6.6, 6.10, 6.11, 6.25, 6.28, 7.4, 9.2, 9.6, 9.7. 3.6: live asked whether the crossing
   box is really blue (spec) or our yellow dashed #FFC20E (bundle CROSS_SELECT_COLOR; kept).
2. Preview blend residuals (behaviours "psui (10-26, P1) Feature preview look"; SPECS CORRECTION 10-27: live's Remove
   cuts the part and draws the grey tool over it, as ours, so Remove / Intersect are right unblended); live draws the active field's picked region outline
   full orange over the preview (ours: 30 % behind the tool); at slider 1 live's top face is lighter than ours.
3. (Done 10-27: a hovered selected edge draws the hover's dark outline round the selection colour and a
   (255,188,64) core, as live.)
4. (In the second batch: the highlight from Onshape's orthographic eye, offscreen test
   `the_highlight_moves_across_an_orthographic_view`.) Check perspective views against live too.
5. Dihedral panel: layout as live (Units select, read-only Max / Min, threshold rows with our dihedral-high / -low
   icons, Scale row, grey ✕), teal teeth (yellow above / purple below the thresholds), no envelope. Left: comb length
   vs the Scale slider (live asked: lengths at min / 25 % / mid / 75 % / max) and the "Min: 90 deg" label on the comb.
6. Cold start (viewport-performance.md 10-27): per-(source, options) MTLLibrary cache in the vendored wgpu-hal.
7. interaction.md 🔧 rows left: 4.1–4.4, 4.7–4.9, 5.1, 5.3, 6.2–6.8, 2.3, 2.6. Ctrl+M Mate connector defaults
   (live asked: owner entity / attachment with one part).

## Requests to the lead

(none)
