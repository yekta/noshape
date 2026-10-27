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

1. Land the preview blend (2 commits on agent/psui after 606d2d0f; check5 = clippy render/scene/app, offscreen tests,
   scene lib, T0). Checked in the app (/root/psui-scratch/prv-add.noshape: Extrude 2 is an Add; skip.noshape):
   - Add at 0.7 vs live psui-prv-01: boss over white (132,148,158) exact, right face over the block (126,164,185) vs
     live (129,164,186); left / top over the block 7–15 darker than live. Slider 0: the model before (live also shows a
     faint grey cap at the boss top); slider 1: opaque preview tone (live's top at 1 is lighter: live's slider isn't a
     plain alpha at the top face; unknown why).
   - Linear pattern: the instance in the part's own colour at 70 %, as live skip-02.
   - Remove / Intersect: unchanged (cut result + see-through tool, no blend). Live: the part uncut, the tool grey
     through it (walls (118,135,144) over the block top, interior (69,80,83)); needs the tool drawn over the parts in
     the preview frame (on-top without draw-order artifacts).
   - Live draws the active field's picked region outline full orange over the preview (ours: the edges behind the
     boss at 30 %).
2. LOFT-PROFILE-LIST (P2), spec in behaviours "psui (10-26) Loft Profiles / Guides entries": each entry 24 px with a
   20×20 › expander at +6 px, no item details under the field; › opens the item's fields inside the field under the
   row (indent 20 px, nested query field becomes the active one), several can be open; divider 1 px rgba(0,0,0,0.2)
   full inner width. Fillet Variable: the same expanders, open by default, titles prefixed "[2 mm]".
3. PANEL-BLEED (P2), behaviours "psui (10-27, PANEL-BLEED)": the feature dialog stays; the analysis panel opens as a
   second panel at the graphics area's left edge (x 246, y 79, 220 wide), background rgba(255,255,255,0.95), its pick
   field empty and active; Curve/surface analysis gets an extra first row "☑ Previewed edges and faces"; the 1st Esc
   closes the panel, the 2nd cancels the dialog. Live asked where it goes with the default dialog position.
4. perf (viewport-performance.md "Open, in order"): runs 40421870988 (fins-400 orbit 2x) and 40421880164 (cold) were
   still running at 01:30; read their artifacts (`gh run download <id>`; perf-macos-14/15/*.log, "BENCH SUMMARY",
   "perf script"). 40421862773: commit 315–635 ms (ticket COMMIT-LATENCY for core), undo/redo 26–71 ms.
5. interaction.md 🔧 rows: 1.12, 3.1–3.3, 3.7, 4.x, 5.x, 6.x, 2.3, 2.6.
6. p1-dialogs.md open items: checkbox label width. Draft INITIAL_FOCUS_ON_EDIT: live matches ours (behaviours "psui
   (10-26) Draft edit focus"): close it in p1-dialogs.md.

## Requests to the lead

(none)
