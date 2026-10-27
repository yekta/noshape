# Tickets (lead, from 2026-10-26)

T2 failures and QA findings, by area and priority (docs/PRIORITIES.md). The area agent owns its tickets; when one is
fixed, mark it `[x]` with the commit, and move the test back to T1 if it is now deterministic and fast
(docs/TESTS.md).


## Release v0.4.9 (qa, 2026-10-27 02:55)
- [ ] P0 release/v0.4.9 recut 7ee597e0: CI run 40452492018 **Windows x86_64 (MSVC) failed in "Build (release)"**:
  `crates/kernel/cpp/common.cpp(1273): fatal error C1083: Cannot open include file: 'cxxabi.h'` (the kernel profile's
  `#include <cxxabi.h>`, GCC/Clang only, unguarded; blame d85c3029, REGEN-SPEED). Still on main, so main's Windows build
  is broken too (the watcher builds Linux only). Guard it (`#if !defined(_MSC_VER)`) or demangle another way. Linux and
  macOS jobs succeeded (06:29) (qa 07:50)
  Fixed on main by 2d6719c2 (qa 08:20: not yet on release/v0.4.9, still 7ee597e0); with a117663d (DRAFT-HANG) and 011aaf43
  (BOOL-THREAD-CRASH, serial booleans) it is what a recut needs
  core 10-27: the lead's 2d6719c2 guards it on main (the release branch needs it too).
- [ ] P0 the recut ships DRAFT-HANG: on the release's Linux artifact (run 40452492018, 0.4.9.7ee597e05af9) a copy of
  tickets/cli-draft-hang.noshape never finishes its opening regen (> 150 s, spinner); the draft budget a117663d came after
  the release's base 502416cc. Cherry-pick a117663d (and c30baf56 for markers) or recut. Same artifact: the five crash
  documents open (6.7–32 s regen; cli-delete-crash 148 s because of a 120 s Shell 3), cli-fillet-hang opens in 68 s,
  version string right (qa 08:05)
- [ ] P0 the recut likely ships BOOL-THREAD-CRASH (core): the watcher CLIs 04:06 and 04:48 (≥ 502416cc, the release's
  base) segfault in OCCT booleans on pool threads three times in one fuzz sequence; v0.4.8 runs it through. To confirm
  on the release's Linux artifact when CI finishes (qa 05:55)
- [x] resolved by the recut (qa 05:05: release/v0.4.9 is now 7ee597e0 = main 502416cc + "Release v0.4.9", which has all of them; CI artifact check pending) P0 release/v0.4.9 (2650df69 = 34ab98f7 + CI fix) predates bf8aaee2 (kernel crash guard + OCCT patch 0002), dx's
  crash markers / open rollback (DOC-POISON) and 7a04ba4d (SHELL-WALLS / SHELL-FILLETED-BOX). As cut it ships the Shell
  and Fillet segfaults that leave documents unopenable (SHELL-SLAB-GPF, FILLET-SEGV: regressions since v0.4.8; the green
  app 4d2f6b0c crashed on them, 19555ca3 doesn't). Cherry-pick those fixes onto the release branch (PLAN 2.2) or cut from
  19555ca3. **Confirmed on the release CI's Linux build (run 40434076347, artifact noshape-Linux-X64): it segfaults opening
  cli-shell-gpf, cli-fillet-segv, cli-fillet2-segv and cli-draft-segv.noshape** (/root/qa-scratch/tickets/; green 19555ca3
  opens all five).
## Core modelling (core)
- [ ] P2 MASS-INERTIA-APPROX inertia and centre of mass aren't exact on curved parts: a Steel tube (Ø40 / Ø20 × 30,
  revolved; /root/qa-scratch/rev.noshape Part 1, part_set_material Steel 7850.021 kg/m³) → query_mass_properties: mass
  0.221954 kg and Izz 55.488606 right, but Ixx 44.391247 / Iyy 44.390476 (both exactly m(3(R²+r²)+h²)/12 = 44.39082),
  Ixy −0.006832 (exactly 0) and CoM x 5.8e-5 (0). At the panel's 3 decimals that reads Lxy −0.007, Lxx ≠ Lyy; live
  (Parasolid) is exact there. Looks mesh- or sample-based like MEASURE-MAX-DIST (qa 2026-10-27, watcher CLI 08:12)
  A 40×30×10 Steel box is exact (CoM (20, 15, 5), Ixx / Iyy / Izz 7.850 / 13.345 / 19.625), so only curved faces drift.
- [ ] P0 BOOL-THREAD-CRASH the process dies inside OCCT booleans on a worker thread, nondeterministically: CLI fuzz seed 72 on
  the watcher CLI 04:06 (≥ bcf401ca, crash guard and blend budget in): run 1 died at step 105 (delete Extrude 8; marker:
  Extrude 10 evaluating), kernel log "general protection fault … in noshape-cli[b13430a]" =
  `BOPAlgo_PaveFiller::ForceInterfEF` on thread 768668 (main pid 768652); the replay of the same seed died earlier, at
  step 81 (suppress; marker: Chamfer 5 evaluating), "segfault at 0 … [b0d43a5]" = `BOPAlgo_Builder::FillInternalShapes`.
  Neither document crashes when reopened (tickets/cli-delete-gpf.noshape: 6 opens on two CLIs, all OK), so it looks like
  a race in OCCT's parallel boolean (thread pool) or earlier memory corruption in the session; the crash guard only
  covers the evaluating thread, so a signal on a pool thread takes the whole process down (the app would vanish, the
  CLI's session is lost). Logs cli/clifuzz-72z.log, clifuzz-72zr.log ("START" lines name each op); markers kept in
  tickets/markers/. Try: run booleans with RunParallel off (or a guarded pool), ASan / TSan on the seed-72 replay
  (`NSCLI=<cli> FZTAG=r python3 bin/clifuzz_trace.py 72 106`) (qa 2026-10-27)
  Third occurrence on the watcher CLI 04:48 (≥ 502416cc): the same seed's replay died at step 84 (draft; marker: Shell 3
  evaluating), "segfault at 10 … [b0db1d8]" = `BOPAlgo_Builder::FillInternalShapes` again (log cli/clifuzz-72xr.log).
  Three runs of one sequence, three different crash points: frequent enough to hit in a session.
  The v0.4.8 release CLI runs the same seed to the end (106 steps, no crash; some op results differ there, so this only
  hints at a regression since v0.4.8).
  Fourth occurrence (qa 07:55), watcher CLI 06:09 (≥ 4756f9be): fuzz seed 93 step 114 (draft; marker: Extrude 9
  evaluating), "segfault at 410 / general protection fault … [b154ced]" = `BOPAlgo_PaveFiller::ForceInterfEF` on pool
  threads 1005390 / 1005403 (log cli/clifuzz-93y.log). Two of the last 24 seeds die this way.
  After 011aaf43 (serial booleans; watcher CLI 08:12): replay A of seed 72 ran to the end, replay B died at step 60
  (shell; cli/clifuzz-72sb.log) with no kernel segfault line and no OOM kill this time (an abort or a panic; stderr not
  captured). Replays with stderr saved are running (qa 09:10).
  core 10-27: b0e757f3 stops one data race (in-place pcurves written into the caller's profile edges by the cylinder
  re-framing); the seed-72 replay with it: 2 of 3 runs complete all 106 steps, the 3rd still died at step 81 in
  `BOPAlgo_PaveFiller::ForceInterfEF` on a pool thread (BOPAlgo_PaveFiller_5.cxx, also changed by OCCT patch 0001, after
  v0.4.8). Serial (every OCCT boolean's RunParallel off): 3 of 3 runs complete. So OCCT booleans now run serial
  by default (NOSHAPE_BOP_PARALLEL=1 for the parallel mode; the seed-72 replay is ~35% slower). Left: the race itself
  (patch 0001's EF/VF changes or OCCT), and OSD_Parallel::For loops that run several booleans at once (cut_slabs,
  fuse_slabs, fuse_trimmed) still do.
- [ ] P0 COMMIT-LATENCY edit commit on tests/perf/live-30-features.noshape is over budget on main (perf.yml run
  40421862773, abac9266, 10-27): "commit shown in" 315–378 ms (macOS 14) and 241–635 ms (macOS 15), budget ≤ 200 ms
  (PLAN 2.6; perf measured 174 / 149 ms on 10-26 before the later landings). The time is the model: regen[preview]
  285 ms for Extrude 1's edit (Boss 1–6 8–25 ms each, Linear pattern 1 45 ms, Fillet 3 19 ms, Hole 1 18 ms, Fillet 2
  15 ms), then refine 35 ms. Undo / redo are fine (26–71 ms). Logs: the run's perf-macos-14/15 artifacts (psui
  2026-10-27)
- [x] 3f5f69ed (core: extruded cylinders framed as live, z toward the sketch; CP1 matches, out of KNOWN; R3/C1 now flip the pin as live, the solver's −157.5° turn is skasm's) P0 `model/tests/oracle_asm::assemblies_match_live` CP1 regressed by 339de774 (cylinder frame); fs suspects the
  cylinder axis should point toward the sketch (also CP1, R3, C1). Red at the halt.
- [x] cc6c4b54 (core: live's rule, a blind hole's bottom within 2t of the floor fails the shell before OCCT runs; back in T1) P0 `model/tests/qa_regressions::shell_that_breaks_the_part_fails` red on macOS arm64 in every CI run of 10-26 (T2
  since then): the shell result differs on arm64.
  core 10-27: live fails too (spec "core qa_regressions::shell_that_breaks_the_part_fails": "Could not shell part with
  selections.", part unchanged 21211.1623 mm³ / 24 faces). Ours on Linux fails only because OCCT's slab union comes out
  invalid (NOSHAPE_SHELL_DEBUG: 45 tools, fuse 5.4 s, common 3 s, "result valid 0"): an accident, so arm64 can land
  elsewhere. Needs Parasolid's rule (likely: the blind holes' tip cones reach into the floor's wall; the same part
  10 mm high with through holes shells in live, P1-16) — open.
- [x] (core 10-27: passes on main, Linux, 1.4 s; the whole std_oracle file stays T2) P1 `model/tests/std_oracle::hole_scope_follows_depth_as_live`: "Merge scope at depth 15 mm: []" (Linux, Windows).
- [x] (core 10-27: passes on main, Linux, 6 s; T2 for the other platforms' last digits) P1 `kernel/tests/draft_curve::level_parting_line_matches_occt` red on all platforms 10-26 (exact match vs OCCT).
- [x] verified 19555ca3 (qa): all five crash documents open in the green app 19555ca3 and the CLI (the failing features
  report errors); cli-delete-crash.noshape still regenerating after 120 s in the app, slow but no crash (qa 2026-10-27)
- [x] verified green app bcf401ca (qa 04:25: the document opens, no crash) bf8aaee2 (core: OCCT patch 0002 fixes the Draft's parallel-extrema read; crash guard: a kernel crash fails its feature; the document opens, Draft 7 "Selected faces could not be drafted.") P0 DRAFT-SEGV-DOC a document whose features include a certain Draft crashes the app and the CLI on every open
  (SIGSEGV, null deref in OCCT `Extrema_ExtCS::Points` ← `Draft_Modification::Perform` ← `BRepOffsetAPI_DraftAngle::Build`
  ← noshape::op_draft, cpp/features.cpp:3815 ← model regen/features/detail_a.rs:96 draft_robust). The user can't open the
  document any more (data loss). Found by the CLI fuzzer (/root/qa-scratch/bin/clifuzz.py seed 1 on the watcher's main
  CLI ~20:18; the same document also kills v0.4.8): after step 81 the document still opened fine in a fresh process
  (checked after every step by clifuzz_bisect.py); step 82's feature_add of a Draft segfaulted, and that Draft ("Insert
  feature : Draft 7", the last history entry) had already been **committed before its regeneration ran**, so the saved
  document now segfaults on every open in the green app and both CLIs. Doc: /root/qa-scratch/tickets/cli-draft-segv.noshape; backtrace tickets/cli-draft-segv-bt.txt; the
  fuzz log /root/qa-scratch/cli/clifuzz-1.log has every step's args. Needs (1) the null guard / OCCT patch, (2) a crash in
  one feature must never make the document unopenable (run kernel ops so a signal fails the feature, or open with the
  failing feature marked). Frequent: seed 5 died the same way (same crash address) at its step 102 draft, i.e. 2 of 5
  fuzz runs of ~120 random ops (qa 2026-10-26, build 4d2f6b0c + main CLI)
- [x] verified green app bcf401ca (qa 04:25: the document opens, no crash) bf8aaee2 (core: patch 0002 UnifySameDomain zero-length lines + crash guard; cli-shell-gpf opens, its shells fail with an error; captured inputs in kernel tests crash_inputs) P0 SHELL-SLAB-GPF (regression since v0.4.8: the green app 4d2f6b0c segfaults / aborts opening both documents below,
  v0.4.8 opens them) a Shell
  crashes the CLI with a general protection fault inside OCCT's boolean (`BOPDS_DS::ShapeInfo` ← PaveFiller ←
  BRepAlgoAPI_BooleanOperation::Build ← noshape `by_slabs` (shell.cpp) ← op_shell_solid shell.cpp:1429, t = 2 mm, 2 faces
  removed). Deterministic: the saved document segfaults on every open with the main CLI (3/3); v0.4.8 opens it fine.
  Found by clifuzz_dbg.py seed 7 step 29 (args in /root/qa-scratch/cli/clifuzz-7.log "29 shell-args"); doc
  /root/qa-scratch/tickets/cli-shell-gpf.noshape, backtrace tickets/cli-shell-gpf-bt.txt. Same path, second failure: an
  OCCT exception (gp_Dir ← ShapeUpgrade_UnifySameDomain::MergeSubSeq ← UnifyEdges ← `by_slabs` shell.cpp, t 25 mm) thrown
  through a noexcept frame → std::terminate → SIGABRT (qa5 warned about exactly this); hit by deleting Chamfer 3 under a
  Shell (seed 11 step 42); doc tickets/cli-delete-crash.noshape (unopenable on main, fine in v0.4.8), backtrace
  tickets/cli-delete-crash-bt.txt (qa 2026-10-26, main CLI ~20:18)
- [x] verified green app bcf401ca (qa 04:25: the document opens, no crash) bf8aaee2 (core: fillet_past_end_faces skips an invalid extended part and a crashing fillet, the next fallback runs: the document gives v0.4.8's fillet results; its later Shell abort fixed by patch 0002) P0 FILLET-SEGV (regression since v0.4.8: the green app 4d2f6b0c segfaults opening the document; v0.4.8 opens it) a
  Fillet segfaults the CLI in OCCT
  (`BRep_Tool::CurveOnSurface` ← `ChFi3d_Builder::PerformIntersectionAtEnd` ← PerformFilletOnVertex ← BRepFilletAPI_MakeFillet
  ← noshape `fillet_past_end_faces` ← op_fillet features.cpp:2920), and the saved document then segfaults on every open
  (DOC-POISON). clifuzz_dbg.py seed 9 step 93 (args in /root/qa-scratch/cli/clifuzz-9d.log "93 fillet-args"); doc
  /root/qa-scratch/tickets/cli-fillet-segv.noshape, backtrace tickets/cli-fillet-segv-bt.txt (qa 2026-10-26, main CLI ~20:18)
- [x] verified green app bcf401ca (qa 04:25: the document opens, no crash) bf8aaee2 + this commit (core: the crash guard fails the feature, "Failed to fillet selections."; the document opens; input in kernel crash_inputs. OCCT's surface/restriction blend still crashes on it: making it fillet is open) P0 FILLET-SEGV-BLEND (also in v0.4.8) a Fillet segfaults in OCCT's surface/restriction blend (null 2D curve:
  `Geom2dAdaptor_Curve::EvalD1` ← Adaptor3d_CurveOnSurface ← `BRepBlend_SurfRstConstRad::Values` ← SurfRstLineBuilder ←
  ChFi3d_Builder::ComputeData / PerformSetOfSurf ← BRepFilletAPI_MakeFillet::Build ← features.cpp:2735), leaving an
  unopenable document (DOC-POISON). clifuzz_nodraft.py seed 32 step 46 (log /root/qa-scratch/cli/clifuzz-32.log); doc
  /root/qa-scratch/tickets/cli-fillet2-segv.noshape, backtrace tickets/cli-fillet2-segv-bt.txt. Needs a guard (validate
  the pcurves before the blend, or run the fillet where a signal can't take the process down) (qa 2026-10-27, main CLI
  ~20:18 and v0.4.8)
- [x] verified green app bcf401ca (qa 04:25: the five crash documents open; DRAFT-HANG's doc opens rolled back above the stuck Draft) (dx 10-27: crash markers around each feature evaluation (model-core regen::crash_guard); Session::open rolls a Part Studio back above a feature whose evaluation stopped a process, one named history entry, notes in `recovered` (document_open); app and CLI alike since both open through ops::session; checked on cli-fillet-segv.noshape. The kernel crashes themselves stay open for core) P0 DOC-POISON a feature whose regeneration crashes the process is already committed to the document, so the
  document then crashes on every open (DRAFT-SEGV-DOC, SHELL-SLAB-GPF, FILLET-SEGV, FILLET-SEGV-BLEND). Whatever the kernel bug, one bad feature must not
  make a document unopenable: commit after a successful regen, and/or run kernel ops so a crash only fails that feature
  (subprocess / signal guard), and/or open with the last feature rolled back after a crash on open (qa 2026-10-26)
- [x] verified green app 502416cc (qa 04:56: cli-fillet-hang.noshape opens in 47 s, Fillet 2 red after 33.5 s, Shell 1 10.4 s (SHELL-SLOW); no hang. Residual: each failing blend still stalls ~30 s where live fails at once; fuzz seed 95 step 96 (suppress) on the watcher CLI 06:09 sat > 5 min re-evaluating the features after it, last in Fillet 7: several failing blends at ~30 s each add up; doc tickets/cli-suppress-stall.noshape) checked on watcher CLI 04:06 (qa 04:40): cli-fillet-hang.noshape opens in 42 s, Fillet 2 "Failed to fillet selections."; the chamfer repro (bin/chrepro.py) fails after 30.8 s "Failed to chamfer selections." (core 10-27: a 30 s CPU budget on fillets/chamfers, cancellable; the hang document: Fillet 2 fails after the budget, the regeneration finishes; making that fillet succeed (OCCT's approximation) is open) P0 FILLET-HANG a Fillet runs for > 15 min (stopped by hand) in OCCT's blend approximation (`BRepBlend_AppFuncRoot::
  SearchLocation` / `NCollection_BaseSequence::Find` ← Approx_SweepApproximation ← BRepBlend_AppSurface ←
  ChFi3d_Builder::CompleteData … ← noshape checked_blend / scaled_blend ← op_fillet features.cpp:3049, `checked`, size 5)
  with no timeout or abort; the app would freeze on it. Opening the saved document in a fresh process hangs too (> 30 min,
  stopped). Second occurrence: fuzz seed 33 (main CLI ~20:18) sat in the same BRepBlend approximation for > 1 h
  (stack tickets/cli-hang2-bt.txt, op_fillet features.cpp:2912).
  clifuzz_dbg.py seed 40 step 9 on the watcher CLI 19555ca3 (log /root/qa-scratch/cli/clifuzz-40n.log "9 fillet-args"); doc
  /root/qa-scratch/tickets/cli-fillet-hang.noshape (history ends "Insert feature : Fillet 2"); stack
  tickets/cli-fillet-hang-bt-tail.txt. Needs a time / iteration budget on blends (fail the feature) and the regen must stay
  abortable (qa 2026-10-27, main CLI 19555ca3)
  Chamfer too (qa 2026-10-27, CLI 19555ca3): a 0.1 mm chamfer of 4 edges spins in `ChFi3d_ChBuilder::PerformSurf` →
  ChFi3d_Builder::CompleteData → StoreData (Geom2d_BSplineCurve eval) ← BRepFilletAPI_MakeChamfer::Build ← checked_blend /
  scaled_blend ← op_chamfer features.cpp:3643 (> 10 min in the fuzzer, > 80 s standalone). Repro: open a copy of
  /root/qa-scratch/tickets/cli-chamfer-hang-pre.noshape, feature_add chamfer with entities / width from
  tickets/cli-chamfer-hang-args.json (bin/chrepro.py); stack tickets/cli-chamfer-hang-bt.txt; fuzz seed 62 step 81.
- [x] verified green app 4756f9be (qa 06:07: cli-draft-hang.noshape opens in 27 s, Draft 8 stops after 11.3 s; cli-draft-hang2.noshape in 45 s, Drafts 4 / 5 / 7 stop after 10.2 / 11.9 / 11.0 s; no hang. Residual: each failing draft still stalls ~11 s, three of them make a 42 s open; live fails at once) (core 10-27: drafts and face deletions run on CPU budgets (draft 30 s, face deletion 10 s, defeaturing on the calling thread so the budget counts its work; cancelled regenerations stop them too); the hang document regenerates to the end in 45 s with Draft 8 "Selected faces could not be drafted.") P0 DRAFT-HANG a Draft spins for > 20 min in OCCT's defeaturing (`BOPAlgo_PaveFiller::MakeBlocks` /
  PutPavesOnCurve → `IntTools_Context::IsVertexOnLine` and PerformEF ← BOPAlgo_Builder ← FillGap::TrimExtendedFaces ←
  BOPAlgo_RemoveFeatures ← BRepAlgoAPI_Defeaturing::Build ← noshape op_defeature features.cpp:3984 ← ops.rs delete_faces ←
  model-core detail_a.rs `draft_reblend` / reblend:247). Different root cause from FILLET-HANG (draft, defeaturing), same
  missing budget. clifuzz_dbg.py seed 60 step 76 on the watcher CLI 19555ca3 (log /root/qa-scratch/cli/clifuzz-60n.log
  "76 draft-args"); doc /root/qa-scratch/tickets/cli-draft-hang.noshape (history ends "Insert feature : Draft 8"), stack
  tickets/cli-draft-hang-bt.txt. The CLI hangs on opening the doc too (feature_list > 3 min, same stack). In the app
  (green 16cdcf6d) the doc opens rolled back above Draft 8 (crash marker: good), but dragging the bar below Draft 8 starts a
  regen that never ends (2 workers in Defeaturing, spinner for > 6 min); dragging the bar back above Draft 8 schedules
  regen #3, which queues behind the stuck one: the spinner stays, the Part Studio never updates again until the app is
  killed. Still hangs on the watcher CLI 04:06, which has the fillet/chamfer budget (feature_list > 200 s): the budget
  doesn't cover the draft's defeaturing. Second occurrence (qa 05:05): fuzz seed 71 step 39 on the same CLI (log
  cli/clifuzz-71z.log "39 draft-args", doc tickets/cli-draft-hang2.noshape), the same PerformEF stack under
  `trycatch_timed` (it times but doesn't stop the call); 2 of the last 12 fuzz seeds hit it. Live: a draft either builds or fails at once. Needs a budget on delete_faces / defeaturing (fail the feature,
  "Selected faces could not be drafted.") and a regen that a newer one can supersede (qa 2026-10-27, CLI 19555ca3, app
  16cdcf6d)
- [x] verified green app 4756f9be (qa 06:08: c7928bb4, the other window shows a Suppress made in the first within 2.5 s, no edit needed) verified green app bcf401ca (qa 04:30: two app processes on one copy of the 30-feature part, A suppresses Boss 1, B renames Boss 2 → "RenamedInB": B shows both at once, the file keeps both (feature_list: Boss 1 Suppressed, RenamedInB); A still shows "Boss 2", the known leftover) (dx 10-27: a session merges its edit onto a head another process moved, three-way, then reloads; both edits kept in arrival order as live; session::tests::two_writers_of_one_file_keep_both_edits. Left: same-feature two-field edits keep ours (core merge.rs), and the other window shows the change only at its next edit) P0 TWO-PROCESS-LOST-EDIT the same .noshape opened by two app processes (e.g. `noshape doc.noshape` twice, or a
  file-manager double-click while it's open) gives two independent windows; edits from both are written, but the later
  writer's microversion takes the earlier one as parent while carrying its own stale state, so the earlier edit silently
  vanishes from the head. Repro: open /root/qa-scratch/l30.noshape twice; in A suppress Boss 1, in B rename Boss 2 →
  "RenamedInB"; close both; history_list shows "Suppress" then "Rename : RenamedInB", but Boss 1 is unsuppressed
  (feature_get suppressed: false). Inside one process, opening an open doc from Documents focuses its window (OK). Needs a
  per-file lock / single-instance handoff, or the writer must rebase onto the head it didn't see (Onshape merges
  concurrent edits) (qa 2026-10-26, build 4d2f6b0c; owner: core or dx — document store)
- [x] verified 19555ca3 (qa: 96-case matrix, walls present in every case, live's case 14950.999 vs 14951.962) 7a04ba4d (core: walls across collapsing fillets; live 2.5 mm 14951.96 / 19 faces, ours 14951.00 / 19; the rest is the fillet corners, see SHELL-WALLS-CORNER) P0 SHELL-WALLS Shell silently drops a filleted plate's outer walls (status OK, wrong solid). Repro (live's exact
  case): Sketch 1 on Top = rectangle 80 × 50 + Ø20 circle at its centre; Extrude 1 = the outer region, 10 mm; Fillet 1 =
  the top face (its 4 outer edges + the hole rim), R2; Shell 1 = remove the top face, 2.5 mm.
  live (behaviours "qa (2026-10-26, P0 wrong geometry check)", live/qa/qa-p0-shell-*.png): an open box (floor, four
  full-height walls, a tube around the hole), **2.5 mm 14951.9619 mm³ / 19 faces; 1.5 mm 9609.9491 mm³ / 27 faces**.
  ours: **2.5 mm 10220.907 mm³: only the floor and the hole's tube, no outer walls**; 1.5 mm 9609.03 mm³, walls there.
  Variants that also lose the walls: + Chamfer 1 mm on 2 bottom edges + 2 Ø6.35 holes, at 2.5 and at 1.5 mm (10887.66 mm³);
  a centred 80 × 50 plate with one free Ø~14 hole at (−20, −10) (only a stub of one wall left). Without the fillet, or
  without the hole, it shells right. Docs: /root/qa-scratch/tickets/qa-1026-ps2-shell.noshape (tab Part Studio 2:
  suppress Chamfer 1 and the hole for live's case) and qa-1026-ps3-shell-hole.noshape (tab Part Studio 3); shots
  tickets/shell-fillet-walls-missing-iso.png, -front.png, shell-walls-ps3-iso.png, -front.png. Probably the same root as
  qa5's P1-16 (fillet offsets collapsing in shell.cpp; WIP on agent/qa5). Not a regression: the v0.4.8 release gives
  the same 10887.660 / 10220.907 mm³. **Pattern** (v0.4.8 release CLI, 96 cases, /root/qa-scratch/tickets/shellmatrix.py
  + shellmatrix-v048.txt: plates 80×50×10/30, no hole / Ø20 centred / Ø10 off-centre, top-face fillet R 0/1/2/4, shell
  t 1/1.5/2.5/4): the walls vanish exactly when there is a hole **and t is just above R** (R1 t1.5, R2 t2.5, all 8 such
  cases); t ≤ R, t ≥ 2R and every plate without a hole are fine (qa 2026-10-26, build 4d2f6b0c). **On main** (the
  watcher's CLI build of ~20:18, not green yet) those cases now fail "Could not shell part with selections." instead of
  returning the wrong solid: no longer silent, but live shells them OK (14951.9619 mm³ at R2 t2.5)
- [x] verified 19555ca3 (qa: hollowmatrix cases R1 t1.5 / t2 exact, 6941.0826 / 8948.0826) 7a04ba4d (core: sharp cavity, live's 8948.0826 / 32, 7076.0826 / 31, 15476.0826 / 32 exactly) P0 SHELL-FILLETED-BOX a box whose 12 edges are filleted R, shelled with t > R, is wrong (status OK); live (behaviours
  "qa (2026-10-26, P0 shell check 2)"): the inner offsets of the fillets collapse and the cavity is a **sharp** box, (a)
  8948.0826 / 32 faces, (b) 7076.0826 / 31, (c) 15476.0826 / 32 = the hand math; ours keeps rounded inner edges. 40×30×20, R1: Hollow t2 → 9016.378 mm³ (outer 23924.083 −
  sharp 36×26×16 cavity = 8948.083); open top t2 → 6680.0 (expected 7076.083); Hollow t4 → 16022.136 (15476.083). t ≤ R
  and R = 0 match to 4 decimals (40 cases). Same in the green app (9016.378). Matrix: /root/qa-scratch/tickets/
  hollowmatrix.py + hollowmatrix-v048.txt (qa 2026-10-26, v0.4.8 release CLI + build 4d2f6b0c). On main (watcher CLI
  ~20:18, tickets/hollowmatrix-main.txt): the hollow cases are still wrong (same volumes) and every open-top case with
  t > R now fails "Could not shell part with selections." (live OK)
- [x] cc6c4b54 void (live 'core (2026-10-27)': live's corner is the exact mitre OCCT builds, to 0.002 mm³; the 1 mm³ was live's LOW-precision mass properties; kernel test blend::top_edge_fillets_mitre_at_sharp_corners_as_live) P1 SHELL-WALLS-CORNER (core, from live's shell oracle "spec: core SHELL-WALLS / SHELL-FILLETED-BOX" and "core (2026-10-26 b)")
  top-edge fillets meeting at a sharp vertical corner: live keeps ~1.008 mm³ more per R2 corner than OCCT's mitre (4
  corners 4.031 mm³; no corner patch face), so every filleted shell of qa's matrix is 0.47·R mm³ under live
- [ ] P0 SHELL-SLOW Shell takes seconds, also in the v0.4.8 release: same matrix, plates with a hole + top fillet R ≥ 2 and
  t ≤ R: **8–17.6 s per model** (R2 t1.5 + Ø20 hole: 17.2 s; R4 t1.5/2.5: 8–12 s), R1 cases 2.5–5 s, no-hole cases 0.1–0.7 s
  (whole CLI run, load ~25); at load ~5: 8.2 s (80×50×10, Ø20 hole, R2, t1.5), 9.4 s (same 30 high), 5.4 s (R4 t2.5),
  0.2 s without the hole. In the app (dev build): the same plate (+ chamfer, 2 holes), Shell 1 2.5 mm → Shell eval 3354 ms; edit to
  1.5 mm → 8759 ms; without the fillet 2502 ms (NOSHAPE_PERF log; box load ~35; the green is the watcher's dev build:
  our crates opt-level 1, deps 3). v0.4.8 release CLI on the same doc: query_mass_properties 0.70 s with Shell 1 vs
  0.09 s with it suppressed, i.e. the shell alone ≈ 0.6 s at load ~25; live
  shells such parts in well under a second; ours freezes the preview for seconds (qa 2026-10-26, build 4d2f6b0c)
  qa 2026-10-27 04:45 (load ~22, bin/shelltime.py: 80×50 plate, Ø20 hole, top face R, bottom removed; time of the
  shell's feature_add): watcher CLI 04:06 h10 R2 t1.5 1.77 s, h30 R2 t1.5 1.98 s, h10 R4 t2.5 1.67 s, no hole 0.16 s;
  v0.4.8 release 1.62 / 1.94 / 1.17 / 0.15 s (same volumes). Better than the 8–17 s above, still 10× the no-hole case.
  Worst case (fuzz seed 66 step 80, 3 mm shell of 2 faces): 52.4 s on the watcher CLI 04:06, then "Could not shell part
  with selections." (~2 min on 19555ca3; > 5 min inside the fuzzer under load); samples: shell.cpp by_slabs →
  valid_solid (BRepCheck_Analyzer), ShapeFix_Solid::SolidFromShell, IntTools_BeanFaceIntersector. Repro: copy
  tickets/cli-shell-hang-pre.noshape, `python3 bin/shrepro.py <cli> <copy>` (deletes Shell 5, re-adds it from
  tickets/cli-shell-hang-args.json). A failing shell should fail fast (budget like the blends').
  Release v0.4.9 Linux artifact (7ee597e0, a release build): tickets/cli-delete-crash.noshape opens in 148 s, of which
  Shell 3 119.9 s and Shell 2 21.6 s (NOSHAPE_PERF); the seed-66 shell still 58.7 s on the watcher CLI 06:09. Shells have
  no budget yet (qa 08:00).
  qa 08:50, watcher CLI 08:30 (has cb84d673, shells by regions): bin/shelltime.py h10 R2 t1.5 1.26 s, h30 R2 t1.5 0.99 s,
  h10 R4 t2.5 0.84 s (was 1.6–2.0 s), no hole 0.38 s, same volumes; cb84d673 says ~90 ms for such plates, so the feature as
  a whole is still ~1 s here (load 26). The seed-66 failing shell: 35–40 s (was 52–59); opening that document 74–87 s on
  this box (load 25–39, the 06:09 parallel-boolean CLI the same at equal load).
  core 10-27 (in progress): shells run on the blends' 30 s CPU budget (qa's delete-crash shell input fails after 31 s,
  "shell took too long"); plates whose only concave faces are through holes (and hole-rim fillets) take the regions
  path: the hole plates of the kernel matrix test 0.7–1.1 s → 81–93 ms, same volumes. Parts with concave edges still
  take slabs (1–5 s).
- [ ] P0 REGEN-SPEED (core 10-27, in progress: probe `ops regen_profile::edit_commit_profile` (ignored) and
  NOSHAPE_KERNEL_PROFILE=1 (per kernel call / step); landed 327e611a volume cache, d85c3029 no volume per boolean,
  9281f41a parallel blend check: the edit commit 510 → 364 ms median in a dev build at load ~30 (quiet, min 312). Left
  per regeneration: booleans ~15 ms each (OCCT build), fillet/chamfer ~10 ms + their check ~6.6 ms, Extrude auto-scope
  min_distance ~4 ms each, display meshing ~45 ms) edit commit on the 30-feature part: tests/perf/live-30-features.noshape, edit Extrude 1 10 → 12, ✓ →
  28 features re-evaluated in 1007 ms on the first edit, then 362–584 ms (5 more commits, load ~30); at load ~6:
  281 / 441 / 316 / 309 ms (green dev build); v0.4.8 release CLI full regen ≈ 0.3–0.35 s (0.48 s process − 0.15 s start) (Linear pattern 1 125, Boss 6 97, Chamfer 5 81, Boss 5 75, Hole 1 67, Chamfer 4 66;
  load ~35 on 16 cores, dev build with our crates at opt-level 1); budget ≤ 200 ms, live's server 171 ms for the whole
  part; undo/redo of it 7 / 5 ms (cached, OK). Also big-plate.noshape: regen 3.0 s (Extrude 2 2458, Sketch 2 535 ms) vs
  live 617 ms (491 / 116). v0.4.8 release CLI: a full regen of the 30-feature part (query_mass_properties, process
  0.62–0.76 s, ~0.2 s of it start-up) ≈ 0.45–0.55 s at load ~25, still 2.5–3× the budget
  (qa 2026-10-26, build 4d2f6b0c)
  qa 2026-10-27 green 16cdcf6d (load 30–42): edit Extrude 1 depth to a new value each time (alternating 10/12 is served
  from a result cache in 2–3 ms, so it can't be timed): 11 → 1046 ms, 13 → 1681 ms (29 features evaluated; Boss 1/2
  52–65 ms, Hole 1 30, Linear pattern 1 27, fillets ~11 each); at 14–17 the block swallows the bosses so later times
  (597 / 861 / 360 ms) are on a degenerate model. Undo: the last two states come back cached (2–3 ms), the third undo
  re-evaluates all 29 features (886 ms), far over the 50 ms undo budget; keep more states (or every undo target) cached
  qa 04:49 green bcf401ca at load ~20: commits (fresh depths 11 / 9 / 13 / 8 / 12.5 / 9.5) 223 / 393 / 856 / 325 / 398 /
  305 ms (28 features re-evaluated, median ~360); undo: 1st and 2nd 1 ms (cached), 3rd and 4th 266 / 282 ms.
  qa 08:18 green 4854cad4 (booleans serial since 011aaf43) at load ~27: commits 300 / 153 / 267 / 255 / 248 / 210 ms
  (median ~250, better; still over 200), open regen 471 ms.
- [x] not a bug (live answered 10-26, behaviours "qa (2026-10-26, P0 check)"): PATTERN-OFFPART-PARTS — live also keeps
  each fully-off copy of a Reapply feature pattern as its own floating part (OK, no message), so ours matches (qa)
- [x] verified watcher CLI 04:06 (≥ green bcf401ca, has 239fa250; qa5 pass28/build.py: 2.5 mm 8503.961 mm³ / 47 faces, 1 mm 3998.630 / 33, live 8503.961 / 47 and 3998.627 / 33) (core 10-27: live's numbers exactly, 1 mm 3998.632 / 33 faces, 2.5 mm 8503.961 / 47; kernel test shell.rs shell_block_with_close_holes_like_onshape back in T1) P1 P1-16 / P1-6 (qa5, carried over) Shell 1 mm on live's p2 model fails "Could not shell part with selections."
  (live OK 3998.627 mm³ / 33 faces), 2.5 mm 8487.861 / 53 faces (live 8503.961 / 47); cause, WIP branch agent/qa5 and test
  in docs/handoff/qa5-routing.md "qa5 open items at the 2026-10-26 stop" (qa5 2026-10-26, green a08e3eda)
- [ ] P2 I1 (qa5, re-checked) the same model imported three ways gives three volumes: live/samples/g25-b.x_t 7583.001,
  .sat 7582.996, .step 7582.693 mm³; live (Parasolid) 7582.713024 for all three. On 10-23 STEP was exact (7582.713) and
  SAT 7585.979: SAT got much closer, STEP drifted by −0.02 (qa 2026-10-26, build 4d2f6b0c)
  dx 10-27 (main 71e8357e, CLI): still 7583.001 / 7582.996 / 7582.693. Per face the 14 planes and cylinders agree to
  1e-4 mm²; only the 4 B-spline patches differ: x_t 13.2016 mm² each, SAT 13.2006–13.2011, STEP 13.1819. STEP is
  nearest live's volume, so the x_t patches (our NURBS tube for a rolling-ball blend, xt_read.cpp blend_surface /
  blend_boundary, trimmed by the spine projected onto the supports) come out ~0.15% too big: check the trim curves
  first (the tube itself is O(h^4)). Files /root/dx-scratch/i1/{in.*,d_*.noshape}.
  dx 10-27 later (debug build, not landed): the 4 faces are XT rolling-ball blends (type 56) of radius 2 mm between two
  cylinders, spine an ellipse (type 32) exactly 2.000000 mm from both supports at 17 samples; 4× denser tube stations
  change nothing; a fine mesh of each face gives the same area (13.2012) as the integral (13.2016). So tube, spine
  and contact curves are exact; the +0.072 mm³ per patch (≈ the surface 5 µm outside) must come from the patch's
  other edges (where the blend ends on the neighbouring faces) or its pcurves. Its edges, read: two exact ellipses
  (the contact curves, 6.65917 mm each) and a quarter circle R2 (3.14159 mm), a triangle tapering to a point where
  the cylinders turn tangent; all exact too. Live's per-face areas are asked (REQUESTS dx 10-27 I1) to tell whether
  the excess is ours or the STEP approximation's.
- [ ] P2 BOOL-TOUCH-INFO Boolean Intersect of two blocks that only touch face to face: both parts deleted (= live), but
  live reports INFO "The parts either do not intersect or are totally contained." (dialog bubble); ours OK with no message.
  A disjoint pair: live OK, no message (= ours) (behaviours "qa (2026-10-26, P1 boolean edge cases)") (qa 2026-10-26)
- [ ] P2 ADV2 (qa5 kern row, re-checked) Loft of a 20×20 square on Top to a Ø16 circle 20 mm up (Plane 1): 5927.755 mm³
  in the green app; live 5927.728022 (docs/handoff/qa5-routing.md "kern" table) (qa 2026-10-26, build 4d2f6b0c)
- [ ] P2 HOLE-MISS-MSG a Hole whose points are on the part's Top-plane face but drilled away from the part (sketch on
  Top at z 0 under a 40×30×20 block, default direction): ours "No target parts selected for hole."; live "None of the holes
  intersected a part." with Merge scope flagged (behaviours L56/L57 and "H1 flipped"); live keeps "No target parts…" for
  a cleared scope only (qa 2026-10-26, v0.4.8 + main CLI)
- [ ] P2 PATTERN-REAPPLY-CUT-ONLY Feature pattern with ☑ Reapply features of a Remove extrude alone (its sketch not in
  the pattern; 40×30×10 block, Ø6 cut through all at x 10 drawn on the top face, direction = a bottom edge, 15 mm, count 4):
  ours moves the cut to each instance (3 holes, 706.858 mm³ removed, OK); live L40 (behaviours ~8484): the reapplied cut
  resolves its region on the unpatterned sketch, so every instance re-cuts the original hole → **INFO, no geometry
  change** (with Reapply off ours = live: OK, copies moved, the off-part copy silently does nothing). Check against psb's
  f202f693 / FSW-pattern (qa 2026-10-26, main CLI ~20:18)
- [x] (dx 10-27 08:20, CLI, load ~22: with the document open 5 s the first Extrude add is 9 ms, then 7–25 ms; only an add sent right at open takes 232 ms, because std's editing logic is still loading on the prewarm thread (ops doc.rs and the app's viewport start it at open; Extrude is first in its order). The app's first user add comes after that, so the budget holds; /root/dx-scratch/w/py/t_add.py) P1 Extrude add 308 ms (std logic 55 ms, ~250 ms in ops' add path regens): target ≤ 150 ms (PLAN 2.6).
  dx 10-27 re-timed through the CLI (load ~25): the first Extrude add of a process 230–330 ms (one-time std warm-up),
  later ones 8–23 ms; Hole add 250–290 ms (was ~330: the API's add path stopped running holeEditLogic a third time),
  i.e. two passes of std's holeEditLogic at ~100–150 ms of FeatureScript each; Hole edit 3–60 ms.

- [ ] P3 SM-MODIFY-JOINT (dx 10-27) a Modify joint that changes a flange bend's radius fails: "The walls must meet in one
  sharp corner to make a bend". Repro (CLI, /root/dx-scratch/bench/sm.py): Sheet metal model THICKEN of a 60×40 rectangle
  on Top, 2 mm, R2; Flange 20 mm on the edge along X at y 0 and on the edge along Y at x 0; then sheetmetal_edit_bend
  {"bend": "Bend A", "radius": 3} (the panel's Radius cell, which adds "Modify joint 1": jointType BEND, entity = the
  bend's inner face, useDefaultRadius false, radius 3 mm). Live applies the radius (behaviours "ui (10-24 b)"). Same
  for the app's sheet metal table (shared logic).

## Part Studio UI (psui)
- [ ] P1 MEASURE-MAX-DIST Max dist between curved faces comes from sampled points, not the exact maximum: two tubes
  (Ø40 / Ø20 × 30, axes 50 mm apart; /root/qa-scratch/rev.noshape, Part 1 / Part 2 outer cylinders) → ours "Max dist
  94.850 mm" (app status bar) / 94.849984 (query_measure, from (−19.975, −0.997, 30), a mesh vertex, to (70, 0, 0)); exact
  √(90² + 30²) = 94.868 (from (−20, 0, 30)). Parallel axes 50.000 and Min 10.000 are exact. Live measures on the exact
  geometry (Parasolid); a 0.018 mm error shows at 3 decimals (qa 2026-10-27, green 4854cad4)
  Same on circular edges (query_measure, watcher CLI 08:12): the two tubes' top outer circles Max dist 89.9807 (exact 90),
  tube 1 vs tube 3 139.9787 (exact 140); Min dist / Center dist exact (10 / 50, 60 / 100).
- [ ] P2 EMPTY-COMMIT a feature with an empty required selection can be committed: new Extrude, nothing picked, Enter,
  Enter → ours commits a red "Extrude 1" (green 525c619b). Live keeps the dialog open: the ✓ is disabled (`button-ok
  disabled`), and Enter, ✓ or Ctrl+Enter do nothing until a region is picked (behaviours "qa (2026-10-27, P2 batch 2)"
  (5)) (qa 2026-10-27, green 4854cad4)
- [ ] P2 CONFIG-CELL-EDIT a configured value can't be edited in the Configuration panel's table: Configure Part Studio,
  add row "Big", Linear pattern 1 › Instance count › right-click › Configure (column "Linear pattern 1 / Instance count",
  3 / 3, toast as live); a click, double-click or typing on Big's "3" cell does nothing (no blue frame, no edit), while
  the Name cell takes typing. Setting it through the feature dialog with Big active works (Big 5, Parts (5)). Live's
  table cells are editable like its other panels' (single click frames the cell, typing edits; live asked, qa batch 2
  (6)) (qa 2026-10-27, green 4854cad4)
- [x] c0bbde51 P0 REGEN-NO-CANCEL fixed (psui 10-27): a newer run that evaluates fewer features (the rollback bar above the running feature, a dialog on an earlier feature) aborts the running one; its finished features stay cached, so the rollback shows at once (app on cli-fillet-hang.noshape: 0 ms, 7 cached, while Fillet 2 ran). Was: a newer regeneration waits for a stale one instead of cancelling it: open a copy of
  /root/qa-scratch/tickets/cli-fillet-hang.noshape in green 502416cc (NOSHAPE_PERF=1); while the opening regen is still
  in Fillet 2 (it fails after the 30 s blend budget), drag the rollback bar above Fillet 2: schedule_regen #2 at 05:16:57,
  but its result (7 features, all cached, 1 ms) only arrives at 05:17:30, when Fillet 2's budget ran out; the spinner
  stays for 33 s for a rollback that costs nothing. The blend budget is cancellable (core 97081557: "jumps back when the
  budget is spent or the regeneration is cancelled"), but scheduling a new regen doesn't cancel the running one. Same
  shape as DRAFT-HANG's stuck app (there forever). PRIORITIES P0 "aborting stale work"; live never makes a cheap edit wait (qa 2026-10-27,
  green 502416cc)
  Likely cause (docs/handoff/kernel-robustness.md, gate (a)): `schedule_regen` aborts each running run "it doesn't
  extend"; a rollback above the running feature is a prefix of the running run, so it is treated as extended and waits
  for the whole run instead of taking the cached prefix at once.
- [x] a9f68288 P2 LOFT-SKETCH-HIDE fixed (psui 10-27): every sketch a feature uses is hidden (a loft both profiles), also one picked in a later edit (model-core `used_sketches`, ops::studio). Was: a Loft hides only its first profile's sketch: Sketch 1 = 40×40 square on Top, Plane 1 = Top
  offset 40, Sketch 2 = Ø24 circle on it (/root/qa-scratch/loft2.noshape); Loft, pick the circle then the square, ✓: Sketch 2
  goes grey with the eye-slash, Sketch 1 stays shown (its menu says Hide) and its outline stays drawn; pick the square first
  and it's the other way round (only Sketch 1 hidden, the circle stays drawn). Live hides every sketch a feature uses
  (behaviours "The used sketch is hidden automatically"); a loft uses both (qa 2026-10-27, build 16cdcf6d)
  Same root, green 525c619b: an Extrude committed with an empty region field (Enter twice, red), then edited to pick
  Sketch 1's region and ✓: Sketch 1 stays shown (its menu says Hide). It looks like only the first region picked while a
  feature is being created hides its sketch; hiding should follow every sketch a committed feature uses (live asked, qa
  batch 2 (1)/(5)). Live confirmed 10-27 (behaviours "qa (2026-10-27, P2 batch 2)" (1)): after ✓ both Sketch 1 and
  Sketch 2 are hidden; live hides every sketch the feature uses.
- [x] verified 19555ca3 (qa: double-click on fins-400 opens Extrude 1) 3b089573 P1 DCLICK-HEAVY fixed (psui 10-27): the X11 and Wayland backends counted clicks by when each press was handled
  (`Instant::now()`), so a second press waiting behind the first click's work (fins-400 here: ~0.9 s) started a new
  click, and the row was selected and deselected. Now the presses' own event times (X server / compositor ms) decide
  (vendor/gpui-pre-linux, NOSHAPE PATCH). fins-400: Extrude 1 double-clicked 150 ms apart opens at once (the old build:
  no dialog, the row deselected). macOS and Windows take the OS's click count already. The Wayland change compiles but wasn't run here (no Wayland
  session on this machine).
  Original report: on a heavy part a double-click on a feature row doesn't open the feature: tests/perf/fins-400.noshape,
  double-click Extrude 1 (or Linear pattern 1) in the feature list, clicks 1 ms or 150 ms apart → only the row gets
  selected, no dialog (5 tries); right-click › Edit… opens it. The same double-click on a light part opens the dialog at
  once. On fins-400 the first click's selection highlight shows ~0.9 s later (software GL, load ~25), so the click count
  is probably measured when the event is handled instead of from the event's own timestamp; live opens the dialog on any
  double-click (qa 2026-10-26, build 4d2f6b0c)
- [x] verified dc3a03f6 with X focus (qa: Tab → preview regen; scheduled twice until 606d2d0f is green) 606d2d0f P1 TAB-NO-PREVIEW: not an app bug, a test-display artifact (psui 10-27). On Xvfb without a window manager the
  noshape window never gets X input focus, so it is never "active" and GPUI dispatches no focus / blur events at all
  (`Window::draw` sends empty focus paths for an inactive window): no input ever blurs, so nothing commits on Tab or a
  click away, while keys still arrive (xdotool sends them to the window under the pointer). With the window focused
  (`xdotool windowfocus <window id>` after the app starts) Tab commits and the preview regenerates at once, as live.
  **qa: give the app window X focus before testing anything focus-related** (blur, Tab, click-away commits, focus rings).
  Fixed alongside: a Tab commit scheduled the preview twice (psui, this commit).
  Original report: a number typed into a feature dialog field and committed with **Tab** never reaches the preview:
  edit Extrude 1 of tests/perf/live-30-features.noshape, Depth 10 → 14, Tab → the field reads "14 mm" but no
  schedule_regen in the perf log and the preview stays at 10 mm (tickets/tab-no-preview-extrude.png); the same in Linear
  pattern 1 (Instance count, Distance + Tab: nothing until Enter or a checkbox change, which then shows all the pending
  values; tickets/tab-no-preview-pattern.png). Enter previews fine. live (confirmed 10-26, behaviours "qa (2026-10-26, P0
  check)" (3)): Distance 12 → 20 + Tab moves the preview at once and focus goes to Instance count; Instance count + Tab
  the same; feature_dialog.rs
  InputEvent::Blur → commit_number + flush_typed looks right, so the Changed event seems lost on the way (qa 2026-10-26,
  build 4d2f6b0c)
- [x] 4e55b01f P1 PREVIEW-NO-PICK (psui found 10-27, regression of 8bf820e0 in v0.4.8 / green ≥ 136c6738): with a feature dialog previewing (new Extrude, any edit), nothing in the graphics hovered or picked: the pick pass drew only the unpickable copy of the model before the feature. Fixed: the preview's pick pass picks the model with the feature (offscreen::a_preview_picks_the_model_with_the_feature). Check: Shift+E on a block, hover its top face (orange boundary), click it (chip "Face of Extrude 1").
- [ ] P2 P1-8 caveat (qa5, carried over) a reopened Hole whose points were clicked one by one shows one grouped chip
  "Vertices of Sketch N"; live keeps one "Vertex of Sketch N" chip per click (the grouping isn't stored; details in
  docs/handoff/qa5-routing.md "qa5 open items at the 2026-10-26 stop") (qa5 2026-10-26)
- [x] P2 P1-10 works on main (psui 10-27: Extrude 2 as Remove flipped away from the block: the tool translucent dark
  red, the part uncut) (qa5, carried over) Extrude › Remove tool preview pointing away from the part is grey; live translucent dark
  red (live part-studio/234–235) (qa5 2026-10-26)
- [x] verified 19555ca3 (qa) 6db9bb8d P2 DELETE-TAB-DIALOG fixed (psui 10-27): live's 598 px "Delete tabs" with the outlined warning triangle, "Deleting
  this tab will result in failed or missing features in N tab(s)", one row per tab with its icon; within 1 px of
  part-studio/1584. Original report: deleting a Part Studio used by an assembly: ours shows plain text "Deleting "Part Studio 1" may
  cause errors in the tab that use it: Assembly 1." (tickets/delete-tab-dialog-ours.png); live: a yellow warning box "⚠
  Deleting this tab will result in failed or missing features in 1 tab" and below it one row per affected tab (tab icon +
  name), [Delete] [Cancel] (live part-studio/1584-p9-tab-delete-confirm.png, behaviours "(7) Delete the "qa5 src" tab")
  (qa 2026-10-26, build 4d2f6b0c)
- [x] verified 16cdcf6d (qa: two profiles list "› Face of Sketch 2 ×" / "› Face of Sketch 1 ×" with ↑↓, nothing under the field; › opens "Faces and sketch regions" inside the entry) 72e41465 P2 LOFT-PROFILE-LIST fixed: each entry "› name ×", 24 px entries 30 px apart with live's 1 px
  rgba(0,0,0,0.2) divider, nothing under the field; › opens the entry's fields inside the field (its query field active,
  the array's border stays blue), as live psui-loft-00/01. Original report: Loft dialog with two profiles: ours lists the chips without live's › expander and repeats the
  names in a second, grey, indented list under the field ("Face of Sketch 2 / Face of Sketch 1",
  tickets/loft-profiles-ours.png); live: one list, each entry "› Face of Sketch 1 ×" with its chevron, ↑↓ reorder button,
  nothing below the field (live ui/961-loft-two-profiles.png) (qa 2026-10-26, build 4d2f6b0c)
- [x] verified bcf401ca (qa 04:25: NOSHAPE_PERF=1, 30-feature part, Shift+X › Plane 1 cuts the bosses: capped, no wgpu error, frames keep coming) 51689c87 P2 PERF-SECTION-PANIC fixed: a section view's colour pass with ambient occlusion on asked for
  timestamp writes with neither index (renderer.rs `GpuTimer::writes` now gives none). Reproduced on green 16cdcf6d, gone
  here. Original report: with `NOSHAPE_PERF=1` the viewport render thread panics as soon as a section view cuts a
  part (Shift+X, pick Front plane): "wgpu error: Validation Error … In a CommandEncoder, label = 'frame' / In a pass
  parameter / no begin or end indices were specified for pass timestamp writes" (thread 'noshape-render'), after which the
  viewport is frozen for the rest of the session (the UI keeps working). Without NOSHAPE_PERF the section renders fine, so
  users don't hit it, but every perf run / perf.yml benchmark with a section does. Log tickets/perf-section-panic.log,
  doc tickets/qa-1026-main.noshape (tab Part Studio 6) (qa 2026-10-26, build 4d2f6b0c)
- [x] P2 EXT-DIRECTION not a bug on main (live behaviours "psui (10-27, EXT-DIRECTION)": ticking Direction with nothing
  picked keeps the empty regions field active in live too; with a region picked first the new field takes the focus,
  as ours) (psui 10-27: on main the captured case works: a face picked, ticking Direction makes "Extrude
  direction" the active field, title and chevron red, as live; with nothing picked yet ours keeps the empty regions field
  active, live asked (REQUESTS "psui (10-27, EXT-DIRECTION)")) (qa4 round 5, still open on green 4d2f6b0c) ticking Extrude › Direction leaves the new "Extrude
  direction" field inactive (white) and the regions field active, chevron grey; after a region pick the field turns pink
  with a red chevron but never gets focus. live (behaviours "qa4 round 5 s7 dialogs"): on ticking, Extrude direction is
  the active field (border rgb(22,81,176), fill rgb(222,241,251)), title red rgb(170,37,45), chevron rgb(189,48,57);
  after a pick focus moves on (qa 2026-10-26)
- [x] verified green 502416cc (qa: hovering "Face of Sketch 1" in Extrude 1 lights the region yellow-green) be87bda5 P2 CHIP-HOVER fixed for sketch regions (a picked region is drawn from the sketch, so its
  QUERY state lit nothing; now its fill turns #f2ff80 while the chip is hovered; faces / edges already did) (qa4 round 5,
  still open on green) hovering a selection chip in a feature dialog (e.g. "Face of
  Sketch 1" in Extrude 1) shows the chip's tooltip but the viewport doesn't change at all (pixel-identical); live turns the
  hovered chip's entity yellow-green #f2ff80 (qa 2026-10-26)
- [x] P2 FOLDER-NAME-INLINE not a bug (psui 10-27): live behaviours "(1) Add selection to folder… → a popup dialog, not
  inline" supersedes the s8 note: a 220 × 69 "Folder name" dialog beside the list, "New folder" selected, as ours
  (qa4 round 5, still open on green) feature rows › Add selection to folder… opens a separate
  "Folder name" box (tickets/folder-name-box-ours.png); live names the new folder inline in its row (input prefilled
  "New folder"); the separate box belongs to the header's New folder button (behaviours "qa4 round 5 s8") (qa 2026-10-26)
- [x] verified green 502416cc (qa 04:58: "1e6 mm" kept, red title, tooltip "Extrude 1 did not regenerate properly: A parameter value is not valid." as live; small residual: ours turns pink only on blur, live is pink right after Enter while still focused) 338b18f5 P2 DEPTH-RANGE fixed: a value outside the parameter's range (Depth above 500 m) makes the
  field pink and stays as typed ("1e6 mm"), in and out of the field; valid values in other units still go to document
  units on blur ("500 m" → "500000 mm"). (live answered, behaviours "qa (2026-10-26, P2) Extrude Depth out of range") a length above 500 m
  (e.g. "1e6 mm", "600 m"): live turns the field pink (bg rgb(249,235,236)), keeps the text as typed and the title tooltip
  says "… A parameter value is not valid."; ours shows the message but never makes the field pink and rewrites "1e6 mm" to
  "1000000 mm". A valid value in other units is rewritten into document units on blur ("500 m" → "500000 mm") in both.
  0 mm matches ("Failed to extrude selections, check input.", no pink) (qa 2026-10-26, build 4d2f6b0c)
- [x] verified bcf401ca (qa: Boss 1 row menu "Edit Boss sketch 1…") 51689c87 P2 MENU-SKETCH-TAG fixed ("Edit Boss sketch…": the tag only when the name doesn't contain
  the type, as the tooltips). Original report: feature row menu: ours "Edit Boss sketch 1 [Sketch]…" for a custom-named sketch; live "Edit Boss
  sketch 1…" (no type tag; behaviours "qa (2026-10-26, P2 batch)" (2)) (qa 2026-10-26, build 4d2f6b0c)
- [x] 67bca14a P2 PANEL-BLEED: live bleeds through the same way (behaviours "psui (10-27, PANEL-BLEED
  follow-up)": both panels at x 246, y 79, the analysis panel on top at rgba(255,255,255,0.95), the dialog's rows
  faint under it). Fixed what differed: the analysis field starts empty over a feature dialog, and with the focus in
  the dialog the 1st Esc closes the analysis panel, the 2nd cancels the dialog; Dihedral's field "Edges to show
  dihedral". Open (P2): the Dihedral panel's layout (live: Units [Degrees ▾], Max / Min read-out rows, Scale above a
  full-width slider, a grey ✕; part-studio/psui-pb-default-newD-1-after.png) and Curve/surface's extra
  "☑ Previewed edges and faces" row over a dialog. Original report: with a feature dialog open (Revolve, Shift+W), Shift+D opens the Dihedral analysis panel on top of it,
  and the Revolve dialog's rows ("Faces and sketch regions to revolve", the clock button, "Full revolve") show through
  the analysis panel's body (tickets/dihedral-over-revolve.png); the panel also comes back with the previous run's pick
  ("Edge of Chamfer 4"). Same for Curve/surface analysis (Shift+C) over an open Extrude dialog
  (tickets/curve-analysis-over-extrude.png). Either the panel must be opaque or the analysis tool must replace the feature dialog as live
  does for tool switches (live: Shift+E with a dialog open replaces it, behaviours 7371) (qa 2026-10-26, build 4d2f6b0c)

- [x] c93f34b4 P2 PS-SECTION-RED fixed (psui 10-27): `Scene::section_interference` (assemblies only) gates the red overlap in the cap shader; offscreen test overlapping_caps_are_red_only_in_assemblies. Was: Part Studio section caps fill overlapping parts' material solid red; live's Part Studio doesn't
  (behaviours "skb (10-24) assembly section" (5): "overlapping parts' caps are grey-hatched with no red overlap fill";
  only the assembly shows the red interference, part-studio/2406–2414). Repro: /root/skasm-scratch/qa/ps2.noshape (block
  + a pin as a new part through it, made by qa/ps2.py), view menu › Section view…, pick Front, ✓: the pin's cap inside
  the block is red (/root/skasm-scratch/ps-section-red-overlap.png). The shader paints any 2-part count red
  (crates/render shaders.wgsl `fs_section_cap`, `parts > 1.0`) with no per-scene switch; the assembly needs it on
  (checked 10-27, as live) (skasm 2026-10-27, main 98cb32d1)

## Sketch + assembly (skasm)
- [ ] P2 REPLICATE-EDGE-DEFAULT Replicate on a mate whose connectors sit on circular edges: ours lists the right options
  (Match edges on plane / on face / individual edges) but starts on **Match edges on plane** and keeps the last field as
  "Parts to find match in"; live starts on **Match edges on face** (the select's tooltip "Edge match scope") and renames
  the last field **"Faces to find match in"** (behaviours "qa (2026-10-27, P2 batch 2)" (4)); repro asm5.noshape-like
  plate + pin, Cylindrical on edge centres, Replicate › Instances = the pin (qa 2026-10-27, green 16cdcf6d)
- [x] 3bcb5145 P1 SKETCH-DRAG-BIG dragging a corner in a 1000-entity sketch (10 × 10 rectangles + 100 circles, made with
  sketch_draw shapes; /root/qa-scratch/cli/bigsketch.noshape) re-evaluates the sketch feature per drag step in 72–113 ms
  (green dev build, load ~25; NOSHAPE_PERF "regen: 1 features evaluated"); budget: under one 60 Hz frame. Re-time on a
  release build; the step looks like a full sketch regen (regions included) rather than a solver-only update (qa
  2026-10-26, build 4d2f6b0c)
  skasm 10-27: the step was the sketch regions' hole search (~40 ms); now regions 3.5 ms, the sketch regen ~12 ms (dev
  build under load); perf test scene sketch_perf::big_shape_sketch_drag_step_parts. Re-time on a release build.
  qa 19555ca3: in the green app the drag step's sketch regen is still 50–185 ms (load 41); re-time on a quiet box /
  release build before closing (qa 2026-10-27)
  skasm 10-27 (e5567eca): the app's steps also redid the DOF analysis (the loaded sketch had none): now computed once before
  the drag; qa's bigsketch in the app (dev, load ~40): 42–125 → 13–18 ms per step (spikes ~40 ms).
- [x] verified bcf401ca (qa 04:20: Sketch 2 circle, Ctrl+C, pointer elsewhere, Ctrl+V: the copy is centred on the pointer, selected, Transform active with its manipulator on the copy, no prompt) ac0645bc (triad on the copy) reopened (qa 19555ca3: the copy lands at the pointer, selected, Ø dimension copied, no prompt = live; but the
  Transform manipulator sits on the original circle / sketch origin, not on the copy's centre as live;
  tickets/sketch-paste-manipulator-on-original.png) 6329b168 P2 SKETCH-PASTE Ctrl+C / Ctrl+V of a dimensioned circle in a sketch: ours pastes the copy exactly on top of the
  original (prompt "Pasted; drag or transform the selection into place"); live (behaviours "qa (2026-10-26, P2 sketch)")
  pastes it centred on the pointer, selected, with the sketch Transform tool active and its manipulator on the copy, no
  prompt; the Ø dimension is pasted too (as ours) (qa 2026-10-26, build 4d2f6b0c)
- [x] 5c94bb51 P2 P1-4 data (qa5, carried over) sketches keep a hidden construction "Origin stand-in" point at (0,0) carrying the
  Horizontal/Vertical constraints (sketch_get lists it); live constrains to the external Origin (qa5 2026-10-26)
  skasm 10-26: fixed on main since 5c94bb51 (in green 4d2f6b0c): sketch_get leaves reference-only geometry out and a
  constraint names its reference ("origin"); e2e `sketch_get_names_the_origin_not_a_hidden_point`. The stored point is
  an external, reference-only entity standing for the Origin (live: external id IB), never drawn, picked or listed.

## Drawings / API / data exchange (dx)
- [ ] P1 UNDO-NOOP-SHIFT a no-op operation pushes an undo step without a history entry, so undo/redo shift by one and
  say the wrong thing: copy of tests/perf/live-30-features.noshape, feature_edit Boss 4 depth 5 → 2 mm, then
  feature_suppress Boss 2 suppressed:false (already unsuppressed: no history entry); history_undo → "Edit : Boss 4" but
  Boss 4 stays 2 mm; history_undo again → "Insert feature : Chamfer 5" but it is the Boss 4 edit that comes back (5 mm);
  history_redo → "Insert feature : Chamfer 5". Found by bin/undochk.py (seed 5: undoing 11 entries ends at 60784.58,
  not the original 60779.48). Related, app (green 4854cad4): opening Extrude 1 and ✓ without a change records "Edit :
  Extrude 1", so the next Ctrl+Z seems to do nothing (live asked whether an unchanged ✓ makes a history entry, qa batch 2
  (7)) (qa 2026-10-27, watcher CLI 08:12)
- [ ] P2 DWG-DIM-TWO-EDGES drawing Dimension (d): click a view's left vertical edge, then its right vertical edge: ours
  places the left edge's length (82) at the second click; live switches to the horizontal line-to-line distance between
  the two edges after the second click and places it on the third (behaviours "qa (2026-10-27, P2 batch 2)" (2)); repro
  /root/qa-scratch/dwg82.noshape Top view (qa 2026-10-27, green bcf401ca)
- [ ] P3 EXPORT-OBJ-MTL OBJ export: live downloads a zip with `<name>.obj` + `<name>.mtl`, header
  `# OBJ generated by ONSHAPE BY PTC INC, 1.222 Units = meters`; ours writes one .obj with "# noshape-kernel OBJ export"
  and no material file (units metres = live) (behaviours "qa (2026-10-27, P2 batch 2)" (3)) (qa 2026-10-27)
- [x] verified green app 4756f9be (qa 06:09: tp2 killed -9 while Hole 1 evaluated; a copy of the same fixture opens with all 30 features and leaves the marker; tp2 then opens rolled back above Hole 1 and the marker is gone. Note: binaries older than c30baf56, e.g. CLIs from earlier builds, still consume any marker) (dx 10-27: each process lists the document files it has open beside its markers (`<pid>.docs`); a dead process's marker rolls back only a file on its list, so copies and other processes' files are left alone, and the list goes with its last marker; the note now reads "noshape stopped while regenerating it" (crash or kill). Test session::tests::a_feature_that_crashed_is_rolled_back_on_open opens a copy first) P2 CRASH-MARKER-COPIES a crash marker rolls back whichever document next opens with that feature id, not the file whose
  process died: copy tests/perf/live-30-features.noshape to kd2.noshape and bur.noshape; open kd2 in the app, Boss 1 ›
  Suppress and kill -9 the app during that regeneration; open bur.noshape (never touched): it opens with the rollback bar
  above Boss 2 and a new history entry "Roll back above Boss 2: it stopped noshape while regenerating", and the marker is
  gone, so kd2 (the file that was regenerating) opens unprotected. Markers live in the shared `<temp>/noshape-regen`
  (crash_guard.rs) and match by feature id only (session.rs recover_crashed_features), so copies of a document, the CLI
  and every agent's process on the box consume each other's markers; /tmp/noshape-regen here holds markers of fuzz CLIs
  killed by `timeout` (Chamfer 3 / 4, Draft 5, Shell 1). Key the marker by the document file (path or store id + head)
  too. Also: a kill -9 or power cut mid-regen reads as "it stopped noshape" (qa 2026-10-27, green 502416cc)
- [x] (dx 10-27, lead's decision (a): won't change. The agent API gives the app's results for the same inputs, so feature_add keeps the open dialog's flip as live's dialog makes it; feature_add's description now says so and that `oppositeDirection` given explicitly is kept, which is how an agent picks the side. qa's P1 note (regression: on v0.4.8 the same calls cut; qa's featmatrix "linear pattern of hole" cases now fail "Selected features do not create any geometry that may be patterned." because the seed hole cuts nothing) is the same choice: v0.4.8's feature_add didn't run the dialog's logic yet, the app's dialog with these inputs doesn't cut either; scripts that relied on the old default pass oppositeDirection: false) P2 API-REMOVE-DIR feature_add extrude operationType REMOVE without oppositeDirection stores oppositeDirection: true
  (feature_get), although feature_schema gives its default as false: a 40×30×10 block on Top, a Ø10 circle sketched on Top,
  REMOVE Through all / Blind 5 / no depth → all "Info: Selected tools and targets do not intersect.", volume 12000 (the cut
  goes −Z, away from the block); with oppositeDirection: false passed explicitly it cuts (11214.602). The app's dialog flips
  Remove away from the part as live's dialog does (behaviours 309, same red tool + toast), but live's REST API keeps the
  parameter default, along the normal (behaviours 6494): the API should store what the caller gave, else the schema
  default (qa 2026-10-27, watcher CLI 04:06 ≥ bcf401ca)
  dx 10-27: the flip is std's extrudeEditLogic (canSetBooleanFlip) run by feature_add as the open dialog runs it, on
  purpose (T0 e2e::feature_add_makes_the_dialogs_choices: a Remove sketched on the part's top face flips *into* the part
  and cuts; live's REST default `false` there points away and cuts nothing). Storing the schema default would fix this
  case (sketch on the Top plane under the part) and break that one. Decision for the lead: (a) keep the dialog's choice
  (live UI), (b) the REST default (live API), or (c) dx's proposal: keep the dialog's flip only when the preview then
  touches the merge scope, else the default (an agent can't see the red tool). Lead 10-27: (a).
- [x] verified green 502416cc (qa 04:55: 3MF 2495 vertices = distinct positions, 0 unmatched edges, 60774.111; OBJ welded, normals through vn; OBJ still in metres, asked live) (dx 10-27: a body's vertices are welded by position: 3MF one closed mesh by index, test export::tests::three_mf_bodies_are_manifold_by_index; OBJ one `v` per position, each face keeping its `vn` normals; OBJ stays in metres) P2 EXPORT-MESH-SHARED 3MF and OBJ exports don't share vertices between faces: tests/perf/live-30-features.noshape
  › Part 1 › Export… › 3MF (Medium) gives 3700 vertices for 2495 distinct positions and 4994 triangles, and 2332 of 14982
  directed edges have no opposite by vertex index (every face is its own island), although by position the mesh is
  closed (volume 60774.111 vs exact 60779.484, as the STL). 3MF Core requires a manifold mesh by index (each edge used by
  exactly two triangles), so slicers report open edges / "repaired errors"; OBJ ("# noshape-kernel OBJ export") has the same
  per-face vertices (and is written in metres, v 0.003 = 3 mm). Weld vertices per body (keep sharp normals through vn
  in OBJ; 3MF carries no normals). Files /root/qa-scratch/exp/l30.{3mf,obj,stl} (qa 2026-10-27, build 16cdcf6d)
- [x] verified dc3a03f6 (qa: name accepted, Derived OK 12000 mm³) (dx 5ef64eb8: a tab name or id is resolved, a non-Part-Studio tab refused with `params.partStudio`) P2 API-DERIVED-BY-NAME feature_add importDerived with `partStudio: "Part Studio 1"` (the tab's name) is accepted and
  the feature fails "Error regenerating."; only the tab id works. Accept the name as tab_* ops do, or refuse it as an
  invalid_argument naming the expected id (main CLI ~20:18) (qa 2026-10-26)
- [x] verified bcf401ca (qa 04:12: the bubble stays 3–30 s after launch and goes on its ×; qa's reopen was wrong, 16cdcf6d lacked 3dfa5710) 3dfa5710 (dx 10-27 04:10: on green 8165fca6, which has 3dfa5710, the bubble for bad.noshape is still shown 25 s after the window appears; the reopen below was on 16cdcf6d, before it; /root/dx-scratch/w/o30.png) reopened 16cdcf6d (qa 2026-10-27: the one-line message is right, "Could not open document: opening notdoc.noshape: not a noshape document ×" / "…: sqlite: database disk image is malformed", but the bubble still goes away by itself: shown at 3 s and 6 s after launch, gone at 9 s with no click on ×, while the documents list finishes loading; tickets/oe3-*.png, oe2.png) 16cdcf6d + 3dfa5710 (dx: one-line reason; the error bubble now stays until its × instead of timing out before a slow start shows the window, and File › Open failures show it too) P2 OPEN-ERROR-SILENT `noshape bad.noshape` with a truncated file ("sqlite: database disk image is malformed") or a
  non-noshape file ("not a noshape document") opens the Documents window with no message; the error is only in the log.
  Show it (an error dialog/toast naming the file and the reason) (qa 2026-10-26, build 4d2f6b0c)
- [x] verified 16cdcf6d (qa: 82×82×20 block, R20 corner, Ø20 hole: Create Drawing of Part 1 › ANSI_A_MM › Suggested views gives SCALE 1:2, one centre mark on the hole, none on the R20 arc; tickets/drawing-scale-verified-16cdcf6d.png) (dx 10-27: the scale list is live's, and automatic centre marks go on whole circles only, not on a rounded corner's arc) P2 DRAWING-SCALE Create Drawing › ANSI_A_MM › Suggested views of an ~82×82×20 part: ours picks scale 0.75 and the
  title block reads "1:1.333"; live picks **1:2** from its list (1:1, 1:2, 1:3, 1:4, 1:5, 1:6, 1:8, 1:10, 1:12, 1:16, 1:20,
  1:25, 1:32, 1:40, 1:50, 1:80, 1:100, 2:1, 3:1, 4:1, 5:1, 6:1, 8:1, 10:1, 12:1, 16:1, 20:1, 32:1, 50:1, 100:1; no 3:4,
  1.5, 2.5 or 0.4). model/drawing/create.rs SCALES has 0.75, 1.5, 2.5, 0.4, 0.04, 0.025… Also: live puts automatic centre
  marks on full circles only; ours also marks the R20 partial arc in the Top view (behaviours "qa (2026-10-26, P2 batch)"
  (1)) (qa 2026-10-26, build 4d2f6b0c)
- [x] verified watcher CLI 19555ca3 (qa: `style` refused, "use `styleV2`") (dx 10-27: changing a hidden legacy parameter that has a visible twin is refused with the name to use, `style` → `styleV2`; feature_get's own values pass back) P1 API-HOLE-STYLE (agent usability; v0.4.8 CLI, re-check on main) feature_schema "hole" lists both `style` and
  `styleV2` (same enum, same description "Style"); feature_add with `style: C_BORE` (or C_SINK) is accepted, status Ok, but
  the hole stays Simple (removed volume = plain hole); only `styleV2` works. Either hide the legacy params the dialog doesn't
  show (style, isV2, holeVersion, …) or map them; an ignored parameter should at least warn. Hole geometry itself checked
  exact for Simple / Counterbore / Countersink × Through / Blind (/root/qa-scratch/bin/holematrix.py) (qa 2026-10-26).
  On main (watcher CLI ~20:18) the schema no longer lists `style`, but feature_add still accepts `style: C_BORE` silently
  and makes a simple hole: an unknown parameter should be an invalid_argument error
- [x] verified watcher CLI 04:06 (qa: booleanScope auto-fills the part; see API-REMOVE-DIR for the direction) fixed on main (watcher CLI ~20:18, verify on green): P2 API-REMOVE-SCOPE (v0.4.8 CLI; please re-check on main) feature_add extrude with operationType REMOVE and no
  booleanScope → "Error: No merge scope selected." (faulty booleanScope); the dialog auto-fills Merge scope = Part 1
  when Remove is picked (live behaviours "Remove … Merge scope = Part 1 (auto-filled)"). AGENT_API.md on main describes
  the auto choices for Add and the Remove direction but not the Remove scope (qa 2026-10-26, v0.4.8 release CLI)
- [x] (dx 10-27: a build without openNURBS no longer lists RHINO, in the Export dialog or export_parts (kernel::RHINO_EXPORT from the build script; test export::tests::rhino_is_listed_only_when_built_in); building the watcher with OPENNURBS_ROOT so QA tests what ships stays the lead's) P2 RHINO-GREEN the green build (/root/green/noshape, the lead's watcher build) has no openNURBS: Part › Export… ›
  RHINO → toast "Export failed: unsupported: Rhino export needs openNURBS: build with OPENNURBS_ROOT
  (scripts/build-opennurbs.sh)", while RHINO stays offered in the format list. Releases build it (release.yml); the
  watcher's build should match the release configuration so QA tests what ships (lead: watcher build env); and a build
  without openNURBS shouldn't list RHINO (qa 2026-10-26, build 4d2f6b0c)
- [x] verified (qa 10-27: USER_GUIDE.md:13 "Undo/redo … cover the edits of the current session") (dx 10-26) P3 docs/USER_GUIDE.md says "Undo/redo … work across sessions"; they don't (the undo history starts empty after a
  reopen, which is also live's behaviour): fix the sentence (qa 2026-10-26, build 4d2f6b0c)
- [x] (dx 10-27: exports and the MCP server report `noshape_kernel::VERSION`, the release tag release.yml stamps (NOSHAPE_VERSION), else the crate's, as the app already did; releases built before cut-release.sh bumped Cargo.toml reported 0.4.6 in files) P2 VERSION-0.4.6 exported files and the app report version 0.4.6 (STEP header `originating_system 'NOSHAPE 0.4.6'`)
  while the releases are v0.4.7 / v0.4.8: the workspace `version` in Cargo.toml was never bumped; the daily release cut
  should bump it (lead's release script) (qa 2026-10-26, build 4d2f6b0c)
- [x] (dx c63d2bf5: bound 1e-4, live's spline bands are ±0.1 %) P2 `kernel/tests/export::rhino_breps_are_valid` red on Windows 10-26 (sphere volume 4e-5 off).
