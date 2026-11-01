# Test tiers

From 2026-10-26 (docs/PLAN.md 2.1). Every test is in exactly one tier. Scripts are in `tools/tiers/`.

| Tier | Command | Runs | Blocks |
|---|---|---|---|
| **T0 smoke** | `tools/tiers/t0.sh` | before every push (every agent), every main commit (watcher) | the push |
| **T1 core** | `tools/tiers/t1.sh` (= `cargo test --workspace`, app included) | every main commit (watcher); CI on `last-green` and on release branches, Linux + macOS + Windows | main, releases |
| **T2 deep** | `tools/tiers/t2.sh` | nightly on Linux (watcher), weekly on macOS/Windows (`ci.yml` schedule), on demand | **nothing**: each failure becomes a ticket (docs/TICKETS.md) |

All scripts take extra cargo args (`--locked`, `-p <crate>`, `--release`). Set `CARGO_TARGET_DIR` to your own dir.

## Landing: `tools/tiers/land.sh` (lead, 10-27; queued 11-01)

The only way to put work on main. `land.sh` rebases on origin/main, runs clippy on the crates the change edits
through rb.sh in your warm vibe4 dir (seconds to a minute; a clippy error refuses the queue and shows the error;
`--no-precheck` skips it in an emergency), queues HEAD on the land server vibe4 and returns at once with `QUEUED <sha>`: **keep working, don't wait for it.** The land daemon there (`tools/tiers/land-daemon.sh`)
takes whatever is queued whenever one of its three slots frees, in arrival order (no batch waits to fill): one place
per worktree (its newest queued change carries its older ones: one check, one result for all), 4 worktrees, 8 above
12 queued changes, 12 above 24. It stacks them on the batches still being checked (a merge train: it lands right
after them, as checked; if one of them doesn't land, its check is stopped and it is rebuilt on the new base) and
checks the stack once: `cargo check` over the workspace (type errors and warnings
anywhere fail), clippy on the crates the changes edit, the full tests of those crates and of the crates that use them
directly, T0 when the app, ops or cli is affected, ops' menu coverage when a UI crate changed. Green: the stack goes to
main. Red: when the error names the change at fault for certain (a compile, clippy or warning location in a file
only one change edited, or in a crate only one change edited or uses; a failing test's crate, the same way), that
change is reported at once and the others are checked again without it; otherwise (two candidates, T0, menu coverage,
a panic without a crate) the batch is split in halves that are checked next (in parallel slots), down to the change
at fault, which is reported; the others land. When a change fails, the queued changes of its worktree that contain
it (queued before it failed) fail with it unchecked ("superseded"); what you queue after the failure is the fix. A change that doesn't apply on main or on the changes queued before it is reported as a
rebase conflict. Deeper dependents' tests run in the watcher after the push. Docs-only changes push at once, no build.
- `tools/tiers/land.sh --status`: your queued, checking, landed and failed changes, with the failing output. The lead
  also tells you when one fails (`LAND FAILED <worktree> <sha> <reason>`). Each land.sh run also prints the results
  since your last one.
- **A failed land is your next task.** Work you stacked on it in your worktree stays there: fix forward with a new
  commit and run land.sh again (it queues everything not yet on main). A rebase conflict: rebase on origin/main by hand
  (once the change it conflicts with has landed), then land.sh.
- `tools/tiers/land.sh --wait`: the old blocking land (this change's checks alone, then the push), for the lead and
  scripts; land.sh falls back to it when vibe4 is unreachable.
- Pushes go through `tools/tiers/land-relay.sh` on this machine (vibe4 has no GitHub credentials), which also syncs
  the results to `/tmp/lead/land-results/<worktree>.log`; land.sh starts the relay (from `/root/lead-scratch`, log
  `land-relay.log` there) and the daemon when they aren't running. On vibe4: `~/land/results.tsv` (every change), `~/land/history.tsv` (every check: queue wait, slot wait,
  phase times, batch size), `~/land/daemon.log`.
- Measured 11-01 on vibe4 (busy with agents' lands): workspace clippy after a model-core edit 98 s / 53 s, against
  `cargo check --workspace` + clippy on model-core 53 s / 28 s. A queued brep change's check: cargo check 30–190 s,
  clippy 1–5 s, test build 7–37 s, tests 33–74 s. T0 had run on every land (`grep -w` took `noshape-brep` for
  `noshape`): 233–313 s saved on lands that don't touch the app, ops or cli or a crate they use directly.

Main red from a pushed change means land.sh wasn't used. A flaky test (passes alone, fails in parallel, or fails now
and then) goes to T2 at once with a ticket for its owner.

## Building and testing on vibe4 / vibe5: `tools/tiers/rb.sh` (lead, 11-01)

Run builds and tests on vibe4, not on this machine (agents' builds kept it at 2–3× its cores and agents spent ~80 % of
their day waiting on compiles). `tools/tiers/rb.sh <command>` syncs your worktree as it is (uncommitted changes too)
to your build server (tools/tiers/rb-hosts: vibe4 for the kernel agents and the land checks, vibe5 for the UI
agents), runs the command there in your worktree's own warm target dir and streams the output; its exit code is the
command's. `--get <bin>` copies a built binary back into `$CARGO_TARGET_DIR/debug/` to run the app here (GUI checks).
Measured: noshape-model's test build cold 188 s, unchanged 7 s, after a one-line model-core change 11 s.
```
tools/tiers/rb.sh cargo test -p noshape-brep --lib
tools/tiers/rb.sh --get noshape cargo build -p noshape   # then run $CARGO_TARGET_DIR/debug/noshape here
```
Heavy jobs (replay corpora, long probes, sweeps) run there too: vibe4 has 62 GB of memory and 400 GB of disk.

## CI on GitHub (`ci.yml`, cliapp 11-01)

`ci.yml` runs one chain per OS (`ci-os.yml`): **OCCT** (the install of this exact patch set restored in under a
minute; else built through ccache whose cache falls back to the previous patch set's, so a patch change rebuilds in
1-3 minutes, not 22-43), then **Test** (T1 or T2, clippy on Linux) and **Release build** (a compile check without LTO;
release.yml builds the shipped binary) side by side. The big caches (rust-cache, OCCT's ccache) are saved from main
only and `prune-caches` keeps one of each kind, so the 10 GB cache quota holds; branches and tags restore main's. To
measure warm runs on a branch: `gh workflow run ci.yml --ref <branch> -f save_caches=true` (delete the branch's
caches afterwards: `gh cache list --ref refs/heads/<branch>`).

What a warm run compiles (11-02, measured with `cargo --timings`): only noshape's own crates. Three caches make that
so, and each is worth knowing about when a build gets slow again:
- **Dependencies** (rust-cache, keyed on Cargo.lock, the toolchain and the CC/CXX environment).
- **The vendored crates** (vendor/: gpui, wgpu-hal…). They are path dependencies, which rust-cache deletes from
  target/ like the workspace's own, so every run rebuilt them (18 CPU-min on Windows, the chain the UI crates wait
  for). `.github/ci/vendored_builds.py` moves their outputs beside target/ before rust-cache saves and back before
  the build; their files get a fixed date after checkout (cargo decides freshness of path crates by file dates) and
  the cache key includes `hashFiles('vendor/**')`, so a vendor change rebuilds them.
- **The kernel's C++** (crates/kernel/cpp, 3-7 min): CC/CXX go through ccache (`setup-ccache`), keyed by content
  and by the compiler's content (not its date: runner images of the same compiler differ in dates, which made OCCT's
  cache miss every object on a newer Windows image).

Warm run times (job, 11-02): Linux test 9-14 min, release 12; macOS test 16-19, release 17-19; Windows test 20
(compile 15, tests 3.4), release 22-24. Before these three caches: Windows test 30.5, Linux 16.6, macOS 19.9.

Splitting the tests into parallel jobs doesn't pay here: the test steps are 85-90 % compiling and 10-15 % running
(Windows: 15 min building, 3.4 min running), so sharing built binaries across jobs saves at most the run; and
splitting the compile makes each job rebuild most engine libraries (engine 24 CPU-min of libraries, 29 of test
builds; UI 11 in all), for ~4 min at best on Windows, with a second rust cache per OS.

release.yml (the shipped binaries) takes 40-50 min, all of it macOS's / Windows' `cargo build --release` with thin
LTO and one codegen unit: the app's binary alone is 12-19 min of LTO at the end of the crate chain, on 2.7 of 4
cores on average. Its jobs already run per OS side by side and bundling, installers and checks take about a minute.
Neither a parallel layout nor the kernel's ccache (tried: 47.6 -> 46.9 min) changes that; only a warm cache of the
shipped profile's dependencies would, which needs main to build that profile (a nightly release dry run on main
saving it, ~3 GB of cache).

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
  `gpui = { package = "gpui-pre", version = "=0.3.9" }` (for `#[gpui::test]`). part-studio-ui and assembly-ui do so
  far (assembly-ui: `assembly_ui_tests.rs`, a document made through the agent API in a temp dir, then the real
  AssemblyView, `assembly_hooks::register_for_tests()`); others opt in the same way. Never put those features on a normal dependency: `.config/hakari.toml` doesn't traverse
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
