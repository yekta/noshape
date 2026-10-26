# Handoff: agent ui (interaction, navigation, P1 dialogs)

Worktree /root/wt/ui, branch agent/ui. State at 10-26 evening (halt).

## On main

- Interaction (docs/parity/interaction.md):
  - Selection, hover, measure and mass rows.
  - Section view: snapping, face implicit points, hatched caps.
  - Assembly box and window select.
  - Replay checks (.github/nav/interact.py, interact_check.py) for rows 1.1, 1.2, 1.4, 1.5, 1.8, 1.11 and 1.14.
    - 18/18 pass on Wayland and X11 (run 40306609140 for the first 14).
    - The 1.14 steps are on main but not yet confirmed by a CI run.
- Navigation: the Windows touch replay passes, pan, pinch and roll included (run 40306609140).
  - win-replay.ps1 holds each touch frame with keep-alive updates.
  - Step sizes follow Onshape's per-event limits.
  - The nav check measures camera angles with atan2.
- VAR-INVALID: the Variable dialog's title, row, pink field and ✓ follow the applied value (live ui-var, ui-var2).
- Mass properties keeps the Point readout for the reference connector (row 4.6).
- Curve pattern's path: no ALLOW_QUERY_ORDER, as live (e339bda0).

## On the WIP branch: agent/ui @ 3710caed (not on main)

Status: app clippy passed, the model tests ran green, and the app was checked by eye for Skip instances. Model clippy was not re-run after the last fix (`type Indexed` alias in regen/features/pattern.rs). The P1 dialog audit fixes:

1. **✓ enable rule** (feature_dialog.rs). Live `canSubmit` = the dialog is dirty and no applied value fails validation.
   - Dirty means a user edit, or a regeneration without an error (`note_regenerated`).
   - Regeneration errors and empty selections no longer disable ✓.
2. **Sweep check order** (regen/features/solid_a.rs). Lock direction first, then a thin wall, then the path (sweep.fs).
   - Test: `sweep_checks_the_lock_direction_first`.
3. **`Hints::FOCUS_ON_VISIBLE`** (params.rs, custom/studio.rs). Set on Loft's adjacent faces, guidesArray, spine and connections (features/solid.rs).
   - Honoured in `focus_new_empty_field`.
4. **Pattern Skip instances as live:**
   - `Hints::ALLOW_ARRAY_FOCUS` and `PREVENT_ARRAY_REORDER` (features/common.rs; Fillet arrays in features/core.rs).
   - `display::TogglePoint` and `FeatureState.toggle_points`, filled by the native linear, circular and curve patterns (regen/features/pattern.rs, eval.rs, regen/mod.rs, ops/studio.rs).
   - Dialog field: chips, CLEAR ("Clear all items"), no Add button (feature_dialog.rs).
   - Viewport (part_studio/mod.rs, behaviors.rs):
     - markers: 11 px HOLLOW disc, rim #606a72 idle, #134e96 hovered, #a2b9df skipped;
     - the seed is a 4 px white dot;
     - a click toggles an instance.
   - Test: the toggle points in `linear_part_pattern_new_add_and_skip`.
5. **docs/parity/p1-dialogs.md:** the audit write-up (specs, empty dialogs, title errors, 68 states).

Next steps for the WIP branch:
1. Run `cargo clippy -p noshape-model -p noshape-ops --all-targets -- -D warnings`.
2. Run the covering tests:
   - `cargo test -p noshape-model --test features_a -- linear_part_pattern`;
   - `--test features_c -- sweep`.
3. Merge origin/main and re-run app clippy.
4. Push to main.

## Open items, in priority order

1. **Land the WIP** (steps above). Then click an instance marker in the app:
   - the instance should disappear and the chip "(1, 0)" appear;
   - compare with live `onshape-research/live/p1audit/skip-03-hover.png` and `skip-04-clicked.png`.
2. **Inactive-field highlight.** Live (p1audit skip-01, skip-02) doesn't highlight picks in inactive dialog fields:
   - Part 1 picked in Entities shows no orange while Direction or Instances to skip is active.
   - Ours outlines every field's picks as accent (`refresh_scene`: `with_geometry(d.selected_refs())`).
   - I was capturing a decisive live test (Mirror: pick the part, activate Mirror plane) when we halted. Capture it first; don't change ours until then.
3. **interaction.md rows still 🔧:** verify each in the app against `onshape-research/live/selection/*`, then mark ✅ or fix:
   - looks: 1.12, 3.1–3.3;
   - 3.7: feature-row hover outline plus the tooltip "Extrude 1 <- Sketch 1";
   - measure 4.x, mass 5.x, section 6.x;
   - assembly 2.3 and 2.6.
4. **Confirm the 1.14 replay steps on CI** with `gh workflow run nav.yml -f ref=main`. Expect 18/18 interaction checks on Linux.
5. **P1 audit, open items** (docs/parity/p1-dialogs.md):
   - checkbox label width: a font advance-width difference, so live ellipsises "Use sketch plane direction";
   - the dialog's x (246 vs 248);
   - Draft `INITIAL_FOCUS_ON_EDIT` on edit: not yet checked against live.

## Known failing tests in my area

None known. Main was red on others' tests at the halt: api's STEP assembly e2e, and `flexible_subassembly_moves_in_parent_and_locks` (model assembly).

## Tools

- **Live capture:**
  - `UI=1 node r.mjs <script>` in /root/live-pw uses my own tab (`/root/ui-scratch/live/.ui-tab`). Other runs never pick it.
  - That tab's window is sized for a 1919×1079 viewport.
  - Scripts: p1a.mjs (empty dialogs) and p1b.mjs (states from /root/ui-scratch/audit/states.json, clicks recorded relative to the dialog).
- **Our side:**
  - /root/ui-scratch/audit/ours.sh, ours-tips.sh, ours-states.py (replays the clicks; our dialog origin is 246,79).
  - pair.sh builds the side-by-side images.
- **Spec diff:**
  - /root/ui-scratch/audit/p1cmp.py compares live `/featurespecs` (p1audit/featurespecs.json) with our dump.
  - The dump comes from the example /root/ui-scratch/audit/p1specs.rs: copy it to crates/model/examples to run it, and never commit it.
