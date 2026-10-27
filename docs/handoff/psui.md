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
- perf.yml on main dispatched 10-26 23:45: 40421862773 (live-30 edit/undo/redo), 40421870988 (fins-400 orbit 2x frames),
  40421880164 (live-30 cold start). Read the summaries into viewport-performance.md.

## TAB-NO-PREVIEW: closed (test-display artifact)

On Xvfb without a window manager the app window never gets X input focus, so GPUI treats it as inactive and sends no
focus / blur events; nothing blurs, so nothing commits. **Always `xdotool windowfocus <window>` after starting the app
on :102** (W=$(xdotool search --name noshape | head -1)). With focus, Tab commits and previews as live. Fixed alongside:
a Tab commit scheduled two regenerations.

## Build times (10-27, load ~30)

App edit → `cargo build -p noshape`: 60–78 s. A vendor gpui-base / gpui-pre-linux edit → app: ~3.5 min. After a rebase
that touches the model: ~12 min.

## Next, in order

1. End of day (~04:00): walk the Part Studio workflows in /root/green/noshape against live; regressions to TICKETS.
2. Preview blend residuals (behaviours "psui (10-26, P1) Feature preview look"): Remove / Intersect tools not
   blended yet (live: the part uncut, the tool grey through it); live draws the active field's picked region outline
   full orange over the preview (ours: 30 % behind the tool); at slider 1 live's top face is lighter than ours.
3. interaction 1.12 / 3.1: live asked for a fresh light capture (REQUESTS "psui (10-27, interaction 1.12)"). Our
   face-hover band is ~4.5 px against live's ~3 px (selection/03); the selected top (229,163,34) vs the old capture's
   (229,170,56). Change nothing until the fresh set arrives.
4. Dihedral panel layout (P2; live asked for the full spec): Units ▾ row, Max / Min read-outs, Scale row, grey ✕;
   Curve/surface's "☑ Previewed edges and faces" row over a dialog.
5. Cold start (viewport-performance.md 10-27): per-(source, options) MTLLibrary cache in the vendored wgpu-hal.
6. interaction.md 🔧 rows left: 1.12, 3.1, 3.7, 4.x, 5.x, 6.x, 2.3, 2.6.

## Requests to the lead

(none)
