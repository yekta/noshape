# Test tiers

From 2026-10-26 (docs/PLAN.md 2.1). Every test is in exactly one tier. Scripts are in `tools/tiers/`.

| Tier | Command | Runs | Blocks |
|---|---|---|---|
| **T0 smoke** | `tools/tiers/t0.sh` | before every push (every agent), every main commit (watcher) | the push |
| **T1 core** | `tools/tiers/t1.sh` (= `cargo test --workspace`, app included) | every main commit (watcher); CI on `last-green` and on release branches, Linux + macOS + Windows | main, releases |
| **T2 deep** | `tools/tiers/t2.sh` | nightly on Linux (watcher), weekly on macOS/Windows (`ci.yml` schedule), on demand | **nothing**: each failure becomes a ticket (docs/TICKETS.md) |

All scripts take extra cargo args (`--locked`, `-p <crate>`, `--release`). Set `CARGO_TARGET_DIR` to your own dir.

## Landing: `tools/tiers/land.sh` (lead, 10-27)

The only way to put work on main. It rebases on origin/main, runs clippy and the full tests of every crate the change
touches plus every crate that depends on them (T0 too when the app, ops or cli is affected), and pushes only if all
of it is green. Docs-only changes push without a build. Main red from a pushed change means land.sh wasn't used.
A flaky test (passes alone, fails in parallel, or fails now and then) goes to T2 at once with a ticket for its owner.

## T0 smoke

- Builds the app (`-p noshape`) and the CLI.
- `crates/ops/tests/smoke.rs`: the demo document, the demo with its drawing, and live's 30-feature part
  (tests/perf/live-30-features.noshape) open and regenerate with every feature Ok; the 30-feature part's volume matches
  live's 60779.484 mm³.
- 20 core agent-API flows of `crates/ops/tests/e2e.rs` (the `E2E` list in t0.sh): sketch → extrude → fillet → pattern →
  export, mates, configurations, sketch tools, drawing views, analysis, STEP round trip, Feature Studio, menus, undo/redo.

## T1 core

Every test that isn't `#[ignore]`d. A T1 test must be:
- **deterministic and platform-independent:** geometry compared within live's tolerance band (`[min, max]` from live's
  mass properties), never against OCCT's or Parasolid's last digits;
- **isolated:** no dependence on thread or test order; files only through `scratch()`-style unique names; the GPU only
  under `gpu_lock()`;
- **fast:** a test over ~5 s on this machine belongs in T2 unless it guards a P0/P1 path nothing faster covers;
- **free of known-difference lists** (KNOWN in the oracles): progress must never turn T1 red.

## UI keystroke tests

Keyboard and focus behaviour (what qa's OTF-FIRST-KEY-style tickets are about) is tested on GPUI's test platform,
T1, no GPU, no display: the real view in a test window, its `Gpu` a no-op stand-in (`Gpu::headless()`: the render
worker on wgpu's no-op backend), keys through GPUI's dispatch. Example: part-studio-ui
`part_studio/sketch_ui_tests.rs` (Line, click, click, `5 0 Enter` → a 50 mm dimension).
- The crate dev-depends on `noshape-ui-test-support` (turns on GPUI's `test-support` and wgpu's `noop`) and on
  `gpui = { package = "gpui-pre", version = "=0.3.9" }` (for `#[gpui::test]`). Only part-studio-ui does so far; others
  opt in the same way. Never put those features on a normal dependency: `.config/hakari.toml` doesn't traverse
  noshape-ui-test-support, so `cargo hakari generate --diff` stays empty, and
  `cargo tree -p noshape -e features -e normal | grep -c test-support` must print 0.
- Write the test inside the module whose private state it reads (`#[cfg(test)] #[path = "…_ui_tests.rs"] mod
  ui_tests;`), with `#[gpui::test] fn …(cx: &mut TestAppContext)`, and import gpui_kit names one by one (a glob brings
  GPUI's `test` macro in, and `#[gpui::test]`'s own `#[test]` then recurses). Set up: `cx.update(gpui_kit::init)`,
  `cx.bind_keys(actions::default_key_bindings())` (the app's keys without the user's customizations),
  `part_studio_hooks::register_for_tests()` (inert app hooks), a document from `Session::create` in a temp dir, the
  view inside a wrapper with `key_context("Workspace")` (the default bindings' context, e.g. Esc → Cancel) via
  `gpui_kit::open_window`, the view built with `Gpu::headless().unwrap()`, then `window.activate_window()`: focus
  events (an input's select-on-focus) only run in an active window, and without it a test passes where the app
  fails. `sketch_ui_tests.rs`'s `sketch_with` does all of it.
- Drive it as the user does, and draw a frame between steps (`window.draw(cx).clear(cx)`, then
  `cx.run_until_parked()`: a newly focused input registers its text handler when painted). Keys:
  `window.dispatch_keystroke(Keystroke::parse("5"), cx)` with `key_char` set for a character key; clicks in the 3D view
  as the viewport's tool events (`ViewportEvent::ToolDown` / `ToolUp` at viewport pixels through the view's
  `on_viewport_event`), since the viewport's own hit testing needs real frames.
- Assert on the model (the sketch's constraints, the document), not on pixels. Each test well under a second.

## T2 deep

Marked `#[ignore = "T2: <reason>"]`; `tools/tiers/t2-names.py` lists them and `t2.sh` runs exactly those (other
`#[ignore]`d tests are generators, benches and perf.yml's timed tests, which keep their own reasons and don't run
in T2). To move a test: `tools/tiers/mark-t2.py <file> "<reason>" <test names>` (no names = the whole file), or
delete the line to bring it back to T1. Record the move below.

## Measurements

**2026-10-26, this machine (16 cores, idle), after a full build, every test binary run back to back:**
- Before the split: 98 binaries, 616 s of test time. Dominated by single tests: standard-content recipes vs live
  (144 s, 90 s, 34 s, 29 s, 28 s…), std_oracle (69 s), custom_features' std sheet metal (45 s), parting-line draft
  (44 s), NIST/Inventor assembly imports (41 s each), Parasolid import (36 s), planar mesh stress (33 s).
- After the split (T1, `cargo test --workspace`, 108 binaries, all green on d7926161): **118 s of test time**, the
  longest binary 10 s. T0's tests: 3 s.
- Building is the real cost: a cold `cargo test --workspace --no-run` takes 24 min here. Incremental compilation stays
  off, so after an edit in `model` expect several minutes of rebuild. The T0 and T1 budgets count test time on a
  warm build.

## What moved to T2 (2026-10-26) and why

| Test(s) | Reason |
|---|---|
| model/tests: `kernel_robustness`, `std_oracle`, `oracle_p1`, `oracle_asm`, `p2_oracle` (whole files) | oracles vs live: exact values and known-difference lists; red on progress. std_oracle `hole_scope_follows_depth_as_live` was red at the halt. |
| model/tests: `sheetmetal_frames`, `sheetmetal_std` (whole files) | sheet metal / frames (P3), long std regenerations |
| model/tests: `pattern_fins`, `pattern_perf`, `pattern_second`, `bracket_perf`, `open_undo_fins`, `native_assembly` (whole files; `big_plate`, `perf`, `drag_speed` were already all ignored) | big models / perf; NIST and Inventor imports 40 s each |
| ops/tests: `many_parts` (`assembly_perf`, `big_plate_perf`, `regen_profile` were already all ignored) | 900 parts, 19 s |
| kernel/tests: `export` (whole file), `planar_mesh` | export round trips compare last digits across platforms (`rhino_breps_are_valid` red on Windows 10-26); Parasolid import 36 s; planar mesh stress 33 s |
| kernel/tests/draft_curve `level_parting_line_matches_occt` | exact match vs OCCT; red on all platforms 10-26 |
| model/tests/features_c `parting_line_draft_on_curves_as_live` | 44 s |
| model/tests/custom_features `std_sheet_metal_matches_onshape` | 45 s |
| model/src/assembly/tests.rs: 18 standard-content recipe tests (`*_match_live`, `*_regenerates_on_every_head`, `standard_content_parts_regenerate_with_mate_connectors`, …) | standard content long tail (P3) vs live; 7–144 s each |
| model/tests/features_c `face_blend_on_curved_faces_as_live` (core, 10-26) | 30 s of face blends (cylinder, cone, chamfer gap: all match live) |
| model/tests/features_c `face_blend_on_an_elliptic_boss_as_live` (core, 10-26/27) | 5 s of face blends (rolling ball, conic, asymmetric on the elliptic boss: as live since 10-27) |
| model/tests/features_c `face_blend_curvature_on_an_elliptic_boss_as_live` (core, 10-27) | open difference from live: the curvature section fails (OCCT's fuse with the lofted tool comes back empty) |
| kernel/tests/crash_inputs `replay_one` (core, 10-27) | a tool: replays and times one NOSHAPE_DUMP_OPS capture named by NOSHAPE_REPLAY |
| kernel/tests/bool_replay `bool_corpus_replay` (core, 10-27) | the Boolean corpus outside the repo (/root/core-scratch/bool-corpus, 974 cases, ~4 min dev); its 26 small cases run in T1 (`bool_corpus_small_cases`) |

- **Back to T1 (skasm 10-27, lead's SKETCH-SOLVED-WITH-RESIDUAL):** sketch `fuzz::random_sketch_edits_never_panic` at
  seeds 1–3 × 60 steps (1.6 s dev build); the wider default (1–5 × 80) stays T2 as
  `random_sketch_edits_never_panic_wide`.

## Timing on the perf server (vibe3, 10-30)
This machine's load makes local timings meaningless. Time a perf test at your commit with
`tools/tiers/perf.sh [-n RUNS] <cargo test args>` (e.g. `tools/tiers/perf.sh -p noshape-model --test all --
custom_features::knurl_feature_b94_on_a_cylinder --ignored --nocapture --test-threads=1`): the commit goes to a temporary
`perf/` branch, vibe3 builds release and runs it RUNS times (3 by default) with the machine to itself (land checks wait),
and prints each run's wall time, the test's timing lines, best and median. Quote those numbers in handoffs and commits.
vibe3 also times the standard set (knurl, big plate, pattern_second, the timed pattern checks) on every new last-green,
every 6 h; a > 15 % slower median is flagged to the lead. perf.yml on GitHub stays the cross-platform reference.
