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

1. Land DCLICK-HEAVY, DELETE-TAB-DIALOG and the Tab double regen (checked in the app; check4 running).
2. Feature preview as live's preview blend (behaviours "psui 2026-10-26: what the feature dialog's preview slider is"):
   both models drawn, mixed at the slider (0.7). The render + scene half is on main (396eea65); the app half is
   /root/psui-scratch/pmix/p2.py (app: base scene
   from `preview_before`, raw slider alpha, opaque FEATURE_PREVIEW bodies). After applying: update the offscreen test
   `feature_preview_shading_matches_onshape` to the one-layer model (box TARGET-tagged, preview_mix 0.7) and refit
   FEATURE_PREVIEW + PREVIEW_AMBIENT/DIFFUSE: live 132,148,158 top/front, 160,186,202 right over white ⇒ the preview
   layer itself ≈ (79,102,116) / (119,156,179). Live asked (REQUESTS "psui (10-26, P1 feature preview look)") how
   changed parts (Add, Remove, Fillet, Shell) look. Mix is off for sketches, Remove/Intersect tool previews and Transform
   ghosts (their looks were fitted for direct drawing).
3. perf: re-measure commit latency, orbit fps, cold start, 1000-pin drag via perf.yml (viewport-performance.md "Open, in
   order"). Cold start: wgpu already compacts each pipeline's shader to its entry point (naga process_overrides) and
   Metal has no pipeline cache in wgpu, so "smaller shaders" is not a lever; get second-launch numbers first.
4. LOFT-PROFILE-LIST (P2): Loft profiles need live's per-item › chevron, dividers, no details list below; live asked
   (REQUESTS "psui (10-26): Loft Profiles").
5. interaction.md 🔧 rows: 1.12, 3.1–3.3, 3.7, 4.x, 5.x, 6.x, 2.3, 2.6.
6. p1-dialogs.md open items: checkbox label width; Draft INITIAL_FOCUS_ON_EDIT on edit (live asked). Dialog x: 246 is
   right (live's own dialog had been dragged to x 40 on the shared account).
7. DELETE-TAB-DIALOG (P2).

## Requests to the lead

(none)
