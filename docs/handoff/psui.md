# Handoff: psui (Part Studio UI), from 2026-10-26

Worktree /root/projects/noshape-wt/psui, branch agent/psui, target dir /root/target-psui, app display :102.
Scratch: /root/psui-scratch (check2.sh = clippy + covering tests + T0, detached; mkskip.sh builds a Skip-instances test
document with noshape-cli; ink.py measures plane-name ink boxes). Live: my own tab is the old ui tab
(`UI=1 node r.mjs …` in /root/live-pw, target id in /root/ui-scratch/live/.ui-tab); captures in onshape-research/live/psui/.
Run the app at live's size: `xdotool windowmove <w> 0 0 windowsize <w> 1919 1079` (planes then sit on live's pixels).

## Landed on main

(nothing yet: paused by the lead 10-26 ~23:00 before the landing check finished)

## WIP: origin agent/psui (5 commits on main 7ac3edb9, not on main)

Clippy passed for model, ops, scene, render, noshape (all targets). Covering tests and T0 had not run when paused.
To land: rebase, `cargo test -p noshape-model --test all -- features_a::linear_part_pattern features_c::sweep`, scene and
render lib tests, `tools/tiers/t0.sh`, push.
1. agent/ui's P1 dialog audit (✓ = live canSubmit, Sweep lock direction first, Loft FOCUS_ON_VISIBLE, pattern Skip
   instances: toggle points, array-focus field with CLEAR, no reorder). Checked in the app against live skip-02..04:
   hover blue, click hides the instance, chip "(1, 0)", CLEAR.
2. agent/icons2's plane-name placement, then tuned: baseline 1.0 ascent from the corner (Front, Top, Right within 1 px of
   live psui/m-01 at the same camera).
3. agent/perf's assembly drag bench (NOSHAPE_PERF_BENCH=drag); the NIST probe left out.
4. Dialogs highlight only the active field's picks (viewport, parts list, feature rows), as live (behaviours "psui
   2026-10-26: feature dialogs highlight only the active field's picks"; Mirror / Linear pattern captures).
5. Pattern instance points 12 px with a 3 px ring (`PointSprite::THICK`), no geometry hover under a hovered point;
   interaction.md 1.14 ✅ (nav.yml run 40339384558).

## TAB-NO-PREVIEW (P1 ticket) — trace so far

- Reproduced in the app (tests/perf/live-30-features.noshape, edit Extrude 1, Depth 10 → 14, Tab): the field shows
  "14 mm" but no `schedule_regen` follows. Enter works.
- The "14 mm" is only the unfocused field's formatted overlay (`converted` in feature_dialog.rs number rows); the input's
  text stays "14". `commit_number` never runs: with debug prints, `FeatureDialog::changed` is never called on Tab, nor on
  a click-away, and gpui's `invoke_subscriber_in` never drops an event. So the input's `InputEvent::Blur`
  (vendor/gpui-base/src/input/base/state.rs `on_blur`, registered with `cx.on_blur(&focus_handle…)`) never reaches the
  dialog, or `on_blur` itself never runs.
- Next: print at the top of InputState `on_focus` / `on_blur` and in the dialog's Blur arm. Suspects: the focused handle
  isn't the input's own (the NOSHAPE PATCH frame focus, `set_frame_focus` / `focus()` with `focus_requested`), so
  `on_blur`'s `previous_focus_path.last() == focus_id` test fails. If so, a fix on our side: also subscribe with
  `on_focus_out` on the input's handle in `ensure_input`, or commit on DialogTab before `window.focus_next`.

## Next, in order

1. Land the WIP (above). Then TAB-NO-PREVIEW.
2. Feature preview as live's preview blend (behaviours "psui 2026-10-26: what the feature dialog's preview slider is"):
   both models drawn, mixed at the slider (0.7). Patch scripts ready, not applied: /root/psui-scratch/pmix/p1.py
   (render + scene: `Scene::preview_mix`, reusing the compare layers and compare-mix pass) and p2.py (app: base scene
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
