# Test tiers

From 2026-10-26 (docs/PLAN.md 2.1). Every test is in exactly one tier. Scripts are in `tools/tiers/`.

| Tier | Command | Runs | Blocks |
|---|---|---|---|
| **T0 smoke** | `tools/tiers/t0.sh` | before every push (every agent), every main commit (watcher) | the push |
| **T1 core** | `tools/tiers/t1.sh` (= `cargo test --workspace`, app included) | every main commit (watcher); CI on `last-green` and on release branches, Linux + macOS + Windows | main, releases |
| **T2 deep** | `tools/tiers/t2.sh` | nightly on Linux (watcher), weekly on macOS/Windows (`ci.yml` schedule), on demand | **nothing**: each failure becomes a ticket (docs/TICKETS.md) |

All scripts take extra cargo args (`--locked`, `-p <crate>`, `--release`). Set `CARGO_TARGET_DIR` to your own dir.

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
| model/tests/qa_regressions `shell_that_breaks_the_part_fails` | red on macOS arm64 in every CI run of 10-26 (ticket: core modelling) |
| model/tests/features_c `parting_line_draft_on_curves_as_live` | 44 s |
| model/tests/custom_features `std_sheet_metal_matches_onshape` | 45 s |
| model/src/assembly/tests.rs: 18 standard-content recipe tests (`*_match_live`, `*_regenerates_on_every_head`, `standard_content_parts_regenerate_with_mate_connectors`, …) | standard content long tail (P3) vs live; 7–144 s each |
| model/tests/features_c `face_blend_on_curved_faces_as_live` (core, 10-26) | 30 s of face blends (cylinder, cone, chamfer gap: all match live) |
| model/tests/features_c `face_blend_on_an_elliptic_boss_as_live` (core, 10-26/27) | 5 s of face blends (rolling ball, conic, asymmetric on the elliptic boss: as live since 10-27) |
| model/tests/features_c `face_blend_curvature_on_an_elliptic_boss_as_live` (core, 10-27) | open difference from live: the curvature section fails (OCCT's fuse with the lofted tool comes back empty) |
| kernel/tests/crash_inputs `replay_one` (core, 10-27) | a tool: replays and times one NOSHAPE_DUMP_OPS capture named by NOSHAPE_REPLAY |
