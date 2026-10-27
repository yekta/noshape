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
- perf.yml on main dispatched 10-26 23:45: 40421862773 (live-30 edit/undo/redo), 40421870988 (fins-400 orbit 2x frames),
  40421880164 (live-30 cold start). Read the summaries into viewport-performance.md.

## In progress (10-27 04:45, unlanded on agent/psui)

Shift+wheel on number fields (5.22); feature list filter as live (6.10 / 6.11: kept Default geometry rows and parts,
"..." runs in Parts, bare ":" matches nothing, ":type" / ":part" alone don't filter, the error icon hides while
filtering; model-core `filter_row`); paused regeneration as live (6.25: amber banner over an emptied toolbar, bold
italic pending rows, New folder gone, Create version… off the rail); 4.4 the open tool's button again keeps its
dialog. Needs: app check on :102, clippy (model-core, ops, app-core, app), model-core interaction tests, T0.

Drafted in /root/psui-scratch (apply after the running check, then build, look, land): patch-box.py (box selection
takes no part vertices), patch-iso.py (Shift+I clears the selection; quiet over a faded part), patch-recent.py (empty
tool search lists "Recently used"), patch-related.py (parts-list related highlight, 3.14 / 3.16).

Residuals found today: live outlines the Delete part ghost (255,192,76) (ours none); a paused dialog's Depth field
shows a corner mark in live; Shift+wheel under xdotool loses events after the first (GPUI X11 resets the scroll
valuator on XI2 DeviceChanged from XTest's slave switches; real wheels fine); the banner's text is 5 px narrower
than live's Flama.

## TAB-NO-PREVIEW: closed (test-display artifact)

On Xvfb without a window manager the app window never gets X input focus, so GPUI treats it as inactive and sends no
focus / blur events; nothing blurs, so nothing commits. **Always `xdotool windowfocus <window>` after starting the app
on :102** (W=$(xdotool search --name noshape | head -1)). With focus, Tab commits and previews as live. Fixed alongside:
a Tab commit scheduled two regenerations.

## Build times (10-27, load ~30)

App edit → `cargo build -p noshape`: 60–78 s. A vendor gpui-base / gpui-pre-linux edit → app: ~3.5 min. After a rebase
that touches the model: ~12 min.

## Next, in order

1. part-studio.md rows with live specs waiting (SPECS.md "psui: part-studio.md …"): 6.25 paused regeneration, 3.6 box selection, 6.10 / 6.11 filter box, 1.12 Delete key, 6.6 Shift+click range,
   7.4 eye, 4.4 tool clicked again, 2.18 Shift+I with parts selected (isolates them, clears the selection; not yet
   re-checked), 9.6 cube hover. Done today: 1.18, 1.19, 1.22, 1.27, 2.11, 2.13, 2.16, 2.17, 3.11, 9.7, 5.22 (Shift+wheel arrives as a
   horizontal scroll on X11 / macOS: `quantity_field::wheel_delta`). 3.6: live asked whether the crossing box is
   really blue (spec) or our yellow dashed #FFC20E (bundle).
2. Preview blend residuals (behaviours "psui (10-26, P1) Feature preview look"; SPECS CORRECTION 10-27: live's Remove
   cuts the part and draws the grey tool over it, as ours, so Remove / Intersect are right unblended); live draws the active field's picked region outline
   full orange over the preview (ours: 30 % behind the tool); at slider 1 live's top face is lighter than ours.
3. Edge selected + hovered (live: the hover's dark outline round a (255,188,64) core): ours never hovers a selected
   entity (builder `hover.filter(|h| !selection.contains(h))`).
4. Part lighting's specular varies across the screen in live, ours is flat: three blocks (behaviours "spec: psui
   part-studio.md 2.18 / 2.19": Part 3 blue at the right reads top (62,96,171), ours (53,87,161); Part 1 / 2 match
   within 2). Live's Part 3 = 0.905 × colour + ~8 (a specular term), Part 1 none; live notes a highlight near the
   front-right corner of one block. Look at the bundle's lightFragment view vector (eye-space position?) before
   changing shaders.wgsl `shade()`; test `part_faces_are_lit_like_onshape`.
5. Dihedral panel layout (P2; live asked for the full spec): Units ▾ row, Max / Min read-outs, Scale row, grey ✕;
   Curve/surface's "☑ Previewed edges and faces" row over a dialog.
6. Cold start (viewport-performance.md 10-27): per-(source, options) MTLLibrary cache in the vendored wgpu-hal.
7. interaction.md 🔧 rows left: 4.1–4.4, 4.7–4.9, 5.1, 5.3, 6.2–6.8, 2.3, 2.6. Ctrl+M Mate connector defaults
   (live asked: owner entity / attachment with one part).

## Requests to the lead

(none)
