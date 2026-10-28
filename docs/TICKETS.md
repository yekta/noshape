# Tickets (lead, from 2026-10-26)

T2 failures and QA findings, by area and priority (docs/PRIORITIES.md). The area agent owns its tickets; when one is
fixed, mark it `[x]` with the commit, and move the test back to T1 if it is now deterministic and fast
(docs/TESTS.md).


## Release blockers for the 2026-10-28 05:00 release (qa, 2026-10-27 20:43)
- [x] verified c5e53806 (qa 00:12: Shift+F, box over the part → app alive, 6 edges in the field; tickets/box-dialog-fixed-c5e5.png) (psui 10-27 22:40: fixed, the dialog read before its update; T1 test box_select_with_a_dialog_open) P0 BOX-DIALOG-PANIC (psui, regression of df28c320, in greens dd1b5037, daf1f50c, c9c66d1c, acd6acce and d2826142 (re-checked 21:55)): any box selection with a feature dialog open crashes the app. See the psui ticket below. Not in v0.4.11.

## Release v0.4.9 (qa, 2026-10-27 02:55)
- qa 09:35: release/v0.4.9 is now fcd301e2 (7ee597e0 + 819c2b47 Windows include fix + fcd301e2 crash_inputs unix-only);
  CI run 40480068401 running. Still not in it: a117663d (DRAFT-HANG budget: the draft-hang document never finishes on the
  CI artifact), c30baf56 (CRASH-MARKER-COPIES), cb84d673 (SHELL-SLOW). It contains 97081557, whose signal-longjmp budget
  is BUDGET-LONGJMP (P0, malloc deadlock seen; likely BOOL-THREAD-CRASH's cause).
  qa 09:50, CI artifact of fcd301e2 (run 40480068401, Linux): the five crash documents open; cli-fillet-hang opens in
  43 s; **cli-draft-hang still never finishes (> 200 s)**: DRAFT-HANG ships unless a117663d is taken. Note for testers:
  this build lacks c30baf56, so markers left by any killed process roll back copies of the same document.
- [x] resolved (qa 10:30: v0.4.9 was published 09:41 from tag 819c2b47 = 7ee597e0 + the lead's own `<cxxabi.h>` guard, with Linux, macOS and Windows assets; same code as the fcd301e2 artifact checked at 09:50 except crash_inputs.rs) P0 release/v0.4.9 recut 7ee597e0: CI run 40452492018 **Windows x86_64 (MSVC) failed in "Build (release)"**:
  `crates/kernel/cpp/common.cpp(1273): fatal error C1083: Cannot open include file: 'cxxabi.h'` (the kernel profile's
  `#include <cxxabi.h>`, GCC/Clang only, unguarded; blame d85c3029, REGEN-SPEED). Still on main, so main's Windows build
  is broken too (the watcher builds Linux only). Guard it (`#if !defined(_MSC_VER)`) or demangle another way. Linux and
  macOS jobs succeeded (06:29) (qa 07:50)
  Fixed on main by 2d6719c2 (qa 08:20: not yet on release/v0.4.9, still 7ee597e0); with a117663d (DRAFT-HANG) and 011aaf43
  (BOOL-THREAD-CRASH, serial booleans) it is what a recut needs
  core 10-27: the lead's 2d6719c2 guards it on main (the release branch needs it too).
- [ ] P0 (qa 10:30: published v0.4.9 = 819c2b47 ships it, as do BUDGET-LONGJMP and BOOL-THREAD-CRASH; v0.4.10 from e8858e61 has a117663d / 011aaf43) the recut ships DRAFT-HANG: on the release's Linux artifact (run 40452492018, 0.4.9.7ee597e05af9) a copy of
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
- [ ] P0 FILLET-OVERSIZE-OK — **live answered** (behaviours "qa (2026-10-27, P0) FILLET-OVERSIZE-OK"): two vertical edges at x = 40 R10 / 10.5 / 12 / 20 are **OK in live with exactly our volumes** (7570.796327 / 7526.842646 / 7384.178285 / 6456.739397: overlapping rounds are intersected, a ridge); **R50 fails in live** "End boundary intersects unfilleted edge." (ours Ok 2154.600, 5 faces); three edges (40,0), (40,20), (0,20): R10 / 12 / 20 OK (7356.194490 / 7075.151640 / 5598.332051), **R25 and R50 fail** with the same message (ours: R25 a D-shaped Ok part on wf-block, R50 an empty part with Fillet Ok). Rule: a feature fails when a round's tangent boundary would run past the far end of an adjacent face into an unfilleted edge. Chamfer cases not answered yet. Original report: an edge fillet too big for its faces reports **Ok** and leaves an impossible solid: block 40×20×10 (Top rectangle (0,0)–(40,20), Extrude 10), Fillet the two vertical edges at x = 40: R10 = full round, 7570.796 mm³ (exact); **R10.5 Ok 7526.843 / 7 faces, R12 Ok 7384.178, R20 Ok 6456.739, R50 Ok 2154.600 / 5 faces** (main CLI 12:08; the 06:11 and 13:04 CLIs the same; tickets/fillet-oversize-min.noshape). On wf-block (its Fillet 1 = the 3 vertical edges) R25 commits Ok as a D-shaped part (tickets/fillet-r25-dshape-8602.png) and **R50 leaves no part at all (volume 0, 0 faces) with Fillet 1 Ok** (Chamfer 1 / Sketch 2 / Hole / pattern then fail); in the app the R50 edit preview never resolves (translucent, no error, > 3 min). Live (behaviours F2, part-studio/134–135): fillets that overrun their faces fail "Fillet 1 did not regenerate properly: Failed to fillet selections." (40×20×10, 4 top edges R10 OK, R12 fail; all 12 edges R5 OK, R6 fail); this exact case asked in REQUESTS; **still on green dd1b5037** (qa 20:06: fmin opens with Fillet 1 R50 Ok, a wedge-shaped part). A single oversize edge is right: live's own case (block 40×30×20, one top front edge R50) gives 7458.616 mm³ = live's 7458.6158 on dd1b5037 (R2: 23965.664, exact), so it's fillets / chamfers of **two edges whose rounds overlap on the face between them** that pass when live fails. **Chamfer too** (bin/oversize.py, main CLI 12:08): the same two vertical edges chamfered 10 mm → 7000.001 (exact, they meet), **10.5 → Ok 6900, 15 → Ok 6000, 30 → Ok 3000 mm³** (a 30 mm chamfer on a 20 mm face of a 40 mm block); live F1: "Distance of chamfer on face too large.". Two top long edges filleted R10.5 / R12 also Ok (live: 4 top edges R12 fail). Fine on the same matrix: one vertical edge R19.9 / R20 exact, larger single-edge radii roll over (live rolls a single 50 mm edge fillet too), Shell top 4.9 / 5 / 6 exact and 12 fails (qa 2026-10-27 19:53, green 8602df0d / CLIs above)
- [x] not a bug, = live (qa 20:00: live behaviours "qa (2026-10-27, P1) PATTERN-EDGE-DIR-FLIPPED": the back bottom edge runs −X in live too, the copies land off the part with the same error; the direction is the edge's own start → end. The bracket file was built relying on v0.4.8's wrong +X; qa5's original live note already flipped it) P1 PATTERN-EDGE-DIR-FLIPPED a document that matched live on v0.4.8 now fails: qa5's bracket (tickets/bracket24.noshape, built 10-24 by mkbracket2.py; workflow-bracket.md) opens in green 965d3f56 with **Linear pattern 1 red** "Could not create all instances as entered. Try selecting "Reapply features" option." (tickets/bracket24-pattern-red-965d.png); on 10-24 / v0.4.8 the same file gave 63671.3275 → 67191.3275 mm³ = live. The pattern is a Feature pattern of Extrude 4 (Ø6 Remove Through all in the wall) × 3, 25 mm, directionOne = the base's **back** bottom edge (y = 50, z = 0), oppositeDirection false. On the 12:08 main CLI: the stored edge now sends the copies **−X** (off the part, hence the error); oppositeDirection true → Ok, 67191.3275, 20 faces; re-picking the **front** bottom edge (y = 0) with no flip → Ok, 67191.3275. So the stored edge's direction reversed between v0.4.8 and v0.4.9+ (candidates: 3789e2aa pattern sweep 10-24 12:12, edge re-identification after Fillet 1 on the y = 50 corners). Existing documents must keep their geometry; which way is live's for that edge is asked in REQUESTS (qa 2026-10-27 19:47, build 965d3f56; CLI main 12:08)
- [x] 203c4d6f (core: a sketch picked whole stands for its non-construction curves in the Surface extrude; T1 e2e::surface_extrude_of_a_whole_sketch) P1 SURF-EXTRUDE-SKETCH-EDGES a Surface extrude of a whole sketch fails: new Sketch 2 on Top with one circle, ✓; Shift+E › Surface, click **Sketch 2's row** in the feature list → the field reads "Edges of Sketch 2", the circle turns red and the feature fails "Extrude 3 did not regenerate properly: No valid curves to extrude" (same with the 30-feature part's Boss sketch 1, and when a Thin extrude of the curve is switched to Surface, which turns its pick into "Edges of Sketch 2"). Picking the circle itself in the graphics area ("Edge of Sketch 2") extrudes the tube fine. Live: a sketch picked in the list fills a curves field with all its curves and they extrude (behaviours: Fill/Boundary/Rib REST note — the sketch-edge query needs `qBodyType(qCreatedBy(sketch, EDGE), BodyType.WIRE)`; the UI does that). Owner core (query resolution in the extrude op) or psui (the list pick's query). Shots tickets/surf-extrude-sketch-pick-2d9a.png (fails), surf-extrude-curve-pick-2d9a.png (curve pick works) (qa 2026-10-27 14:45, green 2d9a275c) — **verified 203c4d6f** (qa 17:58: circle sketch on Top, Shift+E › Surface, click the Sketch 1 row → "Edges of Sketch 1", Surface 1 built, "Surfaces (1)" section; tickets/surf-extrude-fixed-203c.png); re-verified fa61ba6f (qa 19:13: live-30-features, Shift+E › Surface, Boss sketch 1 row → "Edges of Boss sketch 1", the tube previews, Surfaces (1); tickets/surf-extrude-fa61.png)
- [x] no change, as live (core 10-27: live's answer, behaviours "qa (2026-10-27) FEATURE-PATTERN-ADD-Y": the plate's left edge points −Y, off the plate; as picked live fails the same way and drops a second direction's off-plate copies silently, status OK; Opposite direction gives all of them, in ours too. T1 test kernel_robustness::feature_pattern_of_an_added_boss_off_the_plate_as_live) P1 FEATURE-PATTERN-ADD-Y a Feature pattern of an **Add** extrude loses the instances along some directions:
  steps (bin/fp2var.py): plate 300 × 200 × 3 (Top), sketch Ø10 on its top face at (20,20), Extrude Add 10 mm; Linear
  pattern › Feature pattern of it, 40 mm. Along the plate's X edge 3 instances: right. Along its Y edge (direction
  either way), 3 instances: **Error "Could not create all instances as entered. Try selecting "Reapply features""**,
  only the seed boss; with Reapply features on: right. With Second direction on, the second-direction instances are
  dropped **silently, status OK**: 2 × 2 → 2 bosses, 7 × 5 → 7 (should be 35). New-body extrude 2 × 2 → 4 parts
  (right), Part pattern 3 × 2 → 6 (right). Same on CLI 09:50 and v0.4.10. Live patterns an added boss in any in-plane
  direction without Reapply (confirming in REQUESTS "qa (2026-10-27, FEATURE-PATTERN-ADD-Y)") (qa 2026-10-27 11:13,
  v0.4.10 artifact / CLI 09:50)
- [ ] P3 SM-FACES Sheet metal model › Extrude of an open L ((0,0)–(50,0)–(50,30), Blind 20, thickness 2, R3; live "fs (10-23
  05:00)"): volumes exact (3211.3274 / flipped 3051.3274) and the folded boxes = live, but ours has **10 faces**, live **14**
  (12 PLANE + 2 CYLINDER): live keeps each end face (z 0 and z 20) split into wall / bend / wall pieces, ours merges each end
  into one face, so picking "the end of the 30 mm wall" selects the whole end (watcher CLI 09:50) (qa 2026-10-27)
- [ ] P3 PLDRAFT-VOLUME curved parting-line drafts build as live (4 faces, all OK) but a little large: live's "psb (10-25 m)
  curved parting line draft" cases (cylinder Ø20×20, Plane 1 Line angle 120° on a Front line at z 10, Split face, Draft ›
  Parting line from Top, bin/pldraft.py, watcher CLI 08:30): A1 one sided 5° 5974.3559 (live 5974.3441, +2.0e-6), A2 other
  side 6616.0612 (6616.0459), A3 symmetric 6948.9371 (6948.9073, +4.3e-6), A4 5°/10° 7312.7271 (7312.6950, +4.4e-6).
  Shows at 3 decimals in Mass properties; live's drafted faces are exact ruled B-splines (deg 1 × 3, 2 × 66 poles)
  (qa 2026-10-27)
- [x] verified watcher CLI 12:59 (≥ 4e568f3f) (qa 13:45, clifuzz_trace logs cli/clifuzz-{120,131,72}L*.log: seed 120 ran all 150 steps, through step 28 (suppress Draft 1) where it deadlocked twice before; seed 131 passed step 124 (delete Chamfer 7, which hung > 1500 s before; it took ~4 min, re-running Shell 8 under its budget); seed 72's replay: see BOOL-THREAD-CRASH) add45a95 (core: the budget only jumps when the interrupted instruction is the program's own code (OCCT and the bridge are static), never inside a shared library: malloc / free, libstdc++, libm, the loader; the next tick 100 ms later. Kernel test a_budget_never_stops_a_call_inside_the_allocator: the old jump deadlocked 2 of 5 runs. Still a jump: an OCCT-internal lock held by a frame above the interrupted one would stay held (none known in budgeted ops). qa: please re-run seeds 120 / 131 and the seed-72 replay) P0 BUDGET-LONGJMP the CPU budgets (97081557 blends, a117663d drafts / face deletion) jump out of OCCT with
  `siglongjmp` from a timer-signal handler (crates/kernel/cpp/common.cpp `on_budget_tick`, SIGRTMIN+3 on a
  CLOCK_THREAD_CPUTIME_ID timer, SA_RESTART); the jump can land inside malloc/free or any non-reentrant OCCT state.
  Seen: fuzz seed 120 on the watcher CLI 08:30 **deadlocked** at step 28 (feature_suppress → ops regen_report →
  model-core regen/mod.rs:420 `regenerate` → free → `__lll_lock_wait_private(main_arena)`), 0 % CPU for > 10 min, no
  crash marker (no feature evaluating), no other thread holding the arena lock: a lock left held by a jump.
  Backtrace tickets/cli-hang120-bt.txt, core tickets/core-hang120.1599386 (2.6 GB, gcore). This likely explains
  BOOL-THREAD-CRASH too: those segfaults inside BOPAlgo (and the later "silent" deaths, i.e. glibc heap aborts) only
  began on CLIs that have the budget (04:06 on; v0.4.8 and 19555ca3 run seed 72 through), and serial booleans didn't stop
  them. Make the budget cooperative (OCCT's Message_ProgressIndicator::UserBreak / progress ranges, checked between
  steps) or run budgeted kernel calls in a child process; never longjmp from a signal across arbitrary code (the
  crash guard's `on_crash` siglongjmp has the same hazard, but it only runs once something has already crashed)
  (qa 2026-10-27 09:35)
  `BudgetHold` (which defers the jump) is used at two call sites only (features.cpp:2411, 2646); OCCT's own
  allocations (Standard::Allocate → malloc, NCollection allocators) inside a budgeted call are never held.
  A/B (qa 10:45, watcher CLI 09:50, clifuzz_trace seeds 130–132, budget on vs NOSHAPE_BLEND_BUDGET=100000): seed 120
  hung again in batch 9 (CLI 08:30, 1500 s timeout, 2 of 2 runs); seed 131 **budget on hung at step 124** (feature_delete,
  1500 s timeout) after the budget had cut a Shell at step 50; budget off, the same Shell at step 50 ran unbounded
  for > 20 min (killed at 2400 s), so the "off" arm can't reach the later steps and the A/B can't separate the two
  causes. 130 and 132 clean both ways. Logs /root/qa-scratch/cli/clifuzz-13{0,1,2}{on,off}.log.
- [x] add45a95 (core: verified on Windows CI run 40493410580: crash_inputs incl. fillet-blend-segv and all four crash kinds return; std::terminate comes back by longjmp, the C runtime runs terminate handlers under its own __except) P1 WIN-CRASH-GUARD the kernel crash guard (common.cpp, sigsetjmp/sigaction) is POSIX-only: on Windows a crash inside
  OCCT (e.g. kernel/tests/data/crash/fillet-blend-segv, FILLET-SEGV-BLEND) still kills the process (the release/v0.4.9
  CI run 40469839948 died with STATUS_ACCESS_VIOLATION in crash_inputs; that test is now cfg(unix)). Add the Windows
  equivalent (SEH: __try/__except around the bridge call, or a vectored exception handler + longjmp-safe unwinding)
  and turn the test back on for all platforms (lead 2026-10-27)
  core 10-27: cpp/seh.cpp (MSVC): __try/__except around every bridge call and run_guarded, std::terminate inside a
  guarded call raised as a structured exception; crash_inputs and kernel_core::a_crash_inside_the_kernel_fails_only_that_call
  on all platforms again (crash_inside_kernel makes real faults on Windows). To verify on Windows CI.
- [ ] P2 MASS-INERTIA-APPROX inertia and centre of mass aren't exact on curved parts: a Steel tube (Ø40 / Ø20 × 30,
  revolved; /root/qa-scratch/rev.noshape Part 1, part_set_material Steel 7850.021 kg/m³) → query_mass_properties: mass
  0.221954 kg and Izz 55.488606 right, but Ixx 44.391247 / Iyy 44.390476 (both exactly m(3(R²+r²)+h²)/12 = 44.39082),
  Ixy −0.006832 (exactly 0) and CoM x 5.8e-5 (0). At the panel's 3 decimals that reads Lxy −0.007, Lxx ≠ Lyy; live
  (Parasolid) is exact there. Looks mesh- or sample-based like MEASURE-MAX-DIST (qa 2026-10-27, watcher CLI 08:12)
  A 40×30×10 Steel box is exact (CoM (20, 15, 5), Ixx / Iyy / Izz 7.850 / 13.345 / 19.625), so only curved faces drift.
  bin/massmatrix.py (revolved Steel solids, watcher CLI 08:30): volumes and areas exact to 1e-10 in all; inertia sphere
  −1.0e-6, cone ±3e-6 (invisible), tube Ixx +1.0e-5 and Ixy −0.0068 (exactly 0), torus R30 r5 Ixx 54.11200 vs 54.11261
  (−1.1e-5: reads 54.112 vs 54.113). OCCT's BRepGProp default precision; ask it for a tighter epsilon (or Gauss order)
  for the inertia tensor.
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
  captured); replay C hung > 25 min at step 105 (delete); seed 93's replay died at step 114 (draft), its original crash
  step, again without a kernel line. So serial booleans turned the segfaults into silent deaths or hangs, not fixed.
  Seed 93 on the watcher CLI 08:30 then ran all 120 steps (stderr saved, nothing): still nondeterministic. Fuzz batch 9
  runs with stderr saved to catch the next one (qa 09:21).
  See BUDGET-LONGJMP (qa 09:35): a likely root cause.
  core 10-27: b0e757f3 stops one data race (in-place pcurves written into the caller's profile edges by the cylinder
  re-framing); the seed-72 replay with it: 2 of 3 runs complete all 106 steps, the 3rd still died at step 81 in
  `BOPAlgo_PaveFiller::ForceInterfEF` on a pool thread (BOPAlgo_PaveFiller_5.cxx, also changed by OCCT patch 0001, after
  v0.4.8). Serial (every OCCT boolean's RunParallel off): 3 of 3 runs complete. So OCCT booleans now run serial
  by default (NOSHAPE_BOP_PARALLEL=1 for the parallel mode; the seed-72 replay is ~35% slower). Left: the race itself
  (patch 0001's EF/VF changes or OCCT), and OSD_Parallel::For loops that run several booleans at once (cut_slabs,
  fuse_slabs, fuse_trimmed) still do.
  Still there with add45a95 (qa 13:45, watcher CLI 12:59): seed 72 run 1 died at step 63 (delete, marker "Mirror 8" evaluating), **no kernel trap logged** this time (so an abort, e.g. a heap check, not a segfault); the saved document opens fine (tickets/cli-72L-died.noshape + -wal, 190695.576 mm³); the replay with stderr captured ran all 106 steps clean, two more replays (FZTAG L3 / L4, stderr in cli/err-72L{3,4}.txt) running. So the budget's longjmp wasn't the (only) cause.
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
  core 10-27 later: the regions path builds the planes' cavity as one polytope (no booleans), fillet / rim regions from
  sections with an arc, cuts the rim torus first (cut last it took 270–360 ms against the hole's tube), and checks only
  the faces the cut made: qa's shelltime plates (h10/h30 R2 t1.5, h10 R4 t2.5) 420–870 → 177–229 ms a Shell feature at
  load ~35 (dev build), same volumes; probe `kernel_robustness::shell_feature_time_probe` (ignored).
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
  qa 12:50 green e598c3a5 (load 45–68): open 1183 ms; Extrude 1 → 12 / 11 / 13: 28 features re-evaluated in 460 / 1056 / 1327 ms (the ✓ itself is cached, 1 ms); undo 1 / 2 back 1–2 ms, the 3rd back re-evaluates 29 features in 883 ms (unchanged since 16cdcf6d)
  qa 21:11 (green c9c66d1c, app, load 29–36): live-30-features opens in 648 ms; Extrude 1 depth edits commit in 562 / 300 /
  394 / 313 / 163 ms (median 313 ms, budget 200); Ctrl+Z ×3: 2.4 / 3.7 ms (cached), the 3rd 137 ms (budget 50).
- [x] not a bug (live answered 10-26, behaviours "qa (2026-10-26, P0 check)"): PATTERN-OFFPART-PARTS — live also keeps
  each fully-off copy of a Reapply feature pattern as its own floating part (OK, no message), so ours matches (qa)
- [x] verified watcher CLI 04:06 (≥ green bcf401ca, has 239fa250; qa5 pass28/build.py: 2.5 mm 8503.961 mm³ / 47 faces, 1 mm 3998.630 / 33, live 8503.961 / 47 and 3998.627 / 33) (core 10-27: live's numbers exactly, 1 mm 3998.632 / 33 faces, 2.5 mm 8503.961 / 47; kernel test shell.rs shell_block_with_close_holes_like_onshape back in T1) P1 P1-16 / P1-6 (qa5, carried over) Shell 1 mm on live's p2 model fails "Could not shell part with selections."
  (live OK 3998.627 mm³ / 33 faces), 2.5 mm 8487.861 / 53 faces (live 8503.961 / 47); cause, WIP branch agent/qa5 and test
  in docs/handoff/qa5-routing.md "qa5 open items at the 2026-10-26 stop" (qa5 2026-10-26, green a08e3eda)
- [ ] (dx 10-27, live's face data (behaviours "dx (2026-10-27, P2 I1 import volumes)"): our x_t import = live's x_t exactly (blend 13.20156 vs 13.2015552, part 7583.001 vs live HIGH 7583.000048; 7582.713 was live's LOW accuracy). Only our STEP is off: its 4 blend faces 13.18190 vs live's STEP 13.2000619–13.2000916 (volume 7582.693 vs live 7582.977). Left: the STEP reader's B-spline blend faces. dx 10-27 later: OCCT's read.surfacecurve.mode 0/2/3/-2/-3 change nothing (13.18187 each, the 4 identical as the file's surfaces are), so not the trims; live's 4 STEP blends differ slightly from each other (13.20006–13.20009), i.e. live's translator rebuilds them rather than taking the file's B-spline surfaces as given. dx 10-27 15:00, for core: on our STEP import each blend face's segmented Gauss area (common.cpp face_moments / surface_props) is 13.18188 but a fine mesh of the very same face (tessellate 1e-7 m, 5424 triangles) gives 13.19411 (an inscribed mesh can only be under), so the integral is ≥ 0.012 mm² short per face: the integrator, not the geometry or the reader; suspect the boundary integral at the triangle's tip (an edge whose pcurve is null is skipped, or a degenerate edge). x_t faces agree mesh vs integral. Owner: core (kernel integrator)) P2 I1 (qa5, re-checked) the same model imported three ways gives three volumes: live/samples/g25-b.x_t 7583.001,
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
  qa 09:27 (watcher CLI 08:30): now INFO "Boolean resulted in no geometry change. The parts either do not intersect or are
  totally contained." but **both parts are kept** (query_parts: 2); live deletes both and its text is only "The parts either
  do not intersect or are totally contained." Result and wording both differ now.
- [ ] P2 ADV2 (qa5 kern row, re-checked) Loft of a 20×20 square on Top to a Ø16 circle 20 mm up (Plane 1): 5927.755 mm³
  in the green app; live 5927.728022 (docs/handoff/qa5-routing.md "kern" table) (qa 2026-10-26, build 4d2f6b0c)
  qa 20:30 (main CLI 12:08): live's other loft (behaviours pvz-shapes "Loft": 20×20 square (110..130, −10..10) to a circle r8 at
  (120, 0, 25)) gives 7409.693222 / 6 faces, live 7409.706328752699 (−0.013 mm³, −1.8e-6): same direction as ADV2.
- [ ] P2 HOLE-MISS-MSG a Hole whose points are on the part's Top-plane face but drilled away from the part (sketch on
  Top at z 0 under a 40×30×20 block, default direction): ours "No target parts selected for hole."; live "None of the holes
  intersected a part." with Merge scope flagged (behaviours L56/L57 and "H1 flipped"); live keeps "No target parts…" for
  a cleared scope only (qa 2026-10-26, v0.4.8 + main CLI)
  qa 09:30 (watcher CLI 08:30): the same hole is now flipped into the part by itself (feature_get oppositeDirection true,
  scope filled) and cuts (23793.819 = block − hole), status INFO "Parameters have been overridden." Live's dialog doesn't
  auto-flip a hole (L57: "Opposite direction does not flip itself"); it reports "None of the holes intersected a part."
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
- [x] verified c5e53806 (qa 00:16: Shift+X, Front, Shift+E → only the Extrude dialog; Esc cancels it; camera menu still offers Turn off section view; tickets/section-dialog-c5e5.png) (psui 10-27 22:40: starting a feature ends the Section view dialog as with ✓; T1 test a_feature_started_over_the_section_dialog_accepts_it) P2 SECTION-DIALOG-STACK starting a feature while the Section view dialog is open stacks the two dialogs: Shift+X, pick Front ("Section plane 1 (Front)"), then start a feature (Search tools › Transform, or Shift+E) → both dialogs stay open, the Section view panel on top covering the feature dialog's first rows; the 1st Esc closes Section view (tickets/section-over-transform-8602.png). Live (behaviours "qa (2026-10-27, P2) Section view dialog open, then a feature is started"): the Section view dialog closes by itself like an implicit ✓, the section stays on, only the feature dialog is open at the normal spot; the 1st Esc cancels the feature (qa 2026-10-27 20:44, green 8602df0d)
- [x] verified c5e53806 (qa 00:17: Alt+T, Shift+W → Revolve 1 opens, search box empty; tickets/tabs-focus-c5e5.png) (psui 10-27 22:40: the panel no longer focuses Search tabs on open) P3 TABS-PANEL-FOCUS Alt+T puts the keyboard focus in "Search tabs", so shortcuts typed next go into the search box: Part Studio, Alt+T, Esc, Shift+W, Shift+T, Shift+V → the search box reads "WTV" and nothing opens (tickets/tabs-panel-focus-dd1b.png, right column). Live (behaviours "qa (2026-10-27, P3) Tabs panel (Alt+T): focus and Esc"): the focus stays on the page (BODY), typing goes nowhere until the user clicks into Search tabs, Shift+W opens Revolve with the panel open; Esc does nothing to the panel (= ours); Alt+T again or ✕ closes it (qa 2026-10-27 20:44, green dd1b5037)
- [x] verified c5e53806 (qa 00:12: Shift+F, box over the part → app alive, 6 edges in the field; tickets/box-dialog-fixed-c5e5.png) (psui 10-27 22:40: fixed: own feature id and the fitting picks read before dialog.update; T1 test box_select_with_a_dialog_open) P0 BOX-DIALOG-PANIC (regression of df28c320 PREVIEW-SELF-PICK; greens dd1b5037, daf1f50c) a box selection with a feature dialog open crashes the app: wf-block (or any part), Shift+F, left-drag a box over the part → the process dies with "thread 'main' panicked at vendor/gpui-pre/src/app/entity_map.rs:156:32: cannot read noshape_part_studio_ui::feature_dialog::FeatureDialog while it is already being updated" (every time; tickets/box-dialog-panic-daf1.log). Backtrace (tickets/fuzz41-replay-backtrace-daf1.log): PartStudioView::box_select, part-studio-ui behaviors.rs:1051–1052: `dialog.update(cx, |d, cx| { let own = dialog.read(cx).feature.id; … })` reads the entity it is updating; take `own` from `d` (or read it before the update). Found by the UI fuzzer (bin/fuzz.py seed 41: died after 88 actions, replay after 93). Box selection into a dialog field is everyday use (Fillet edges, Shell faces), so this blocks the next release (qa 2026-10-27 20:42, green daf1f50c)
- [x] verified dd1b5037 (qa 20:08: wf-block, Shift+F on the bottom front edge, R1.9 preview; hovering / clicking the preview's new band highlights nothing and adds no chip; tickets/preview-self-pick-fixed-dd1b.png) P2 PREVIEW-SELF-PICK in a feature dialog, a click on geometry the feature's own preview created adds a red chip "Face of deleted feature" / "Edge of deleted feature" (a reference into the uncommitted feature, which can never resolve): qa-scratch/wf-block.noshape, Shift+F, pick a top edge (the preview rounds it), then click the preview's new fillet face / its tangent edge near the corner → red chip (tickets/preview-self-pick-203c.png; hover before the click highlighted the pre-existing edge, preview-self-pick-hover-203c.png). Since PREVIEW-NO-PICK (4e55b01f) the pick pass uses the model with the feature. Live: asked (REQUESTS qa 18:12); likely the feature's own output isn't pickable (picks go to the model before the feature), and in any case the chip should not say "deleted feature" Easy to hit: pick the front top edge, then click the right top edge, which tangent propagation has already rounded in the preview → the red chip; ✓ commits (row black) and the reopened Fillet 2 lists **"Edge of Fillet 2"**, a reference to itself stored in the document (tickets/preview-self-pick-stored-203c.png). (qa 2026-10-27 18:12, green 203c4d6f)
  - psui (landed with this note): the open feature's own geometry (a face it made, an edge or vertex touching one)
    is neither hovered nor picked in its dialog, by click or box (`made_by`, test
    a_feature_does_not_pick_its_own_geometry). Block on :102: Fillet on the top front edge, a click on the preview's
    fillet face adds nothing. If live's answer (REQUESTS qa 18:12) says the click goes to the model before the feature
    instead, pick from there.
- [x] P2 LOFT-PROFILES-PINK a new Loft with ☑ Guides and continuity or ☑ Path and nothing picked: live turns the empty "Profiles" field pink (error) while the newly shown field becomes the active one; ours leaves Profiles plain grey (p1-dialogs.md Loft row says this was done: "Profiles, empty, turns pink"). Replay of live's recorded clicks, tickets/loft-profiles-pink-1c85.png (loft-guides, loft-path: live left | ours right) (qa 2026-10-27 16:19, green 2a6d0167) — **verified f16b31c6 (qa 18:43: live-click replay, loft-guides / loft-path: Profiles pink as live; tickets/loft-profiles-pink-fixed-f16b.png)**
  - psui (landed with this note): the Profiles array field takes the error look when the error names it and it isn't
    active (it never did: only query fields did); and a tab click, tick or select now makes the dialog dirty, so ✓
    turns green as in qa's live shots (live DIALOG-OK-STATE rule 2: "a tab click (Solid / Surface)" counts). lpd on
    :102: new Loft, tick Guides and continuity → Profiles pink, Guides active, ✓ green.
- [x] P1 TAB-EXPORT-MISSING the Part Studio and Assembly tab menus have no "Export…" (green 1c853f78: Delete | Open in new browser tab, Rename, Properties…, Show code | Duplicate, Copy to clipboard | Select as document thumbnail | Move to document…; the Assembly one adds Create Drawing / Release… but no Export…), so core-workflow 7.1 ("Part Studio tab › Export…, STEP") can't be done. `crates/app/src/workspace.rs` ~1233 adds Export… for Drawings only, citing "live 10-24: parts export from their own menu". Live: Export… is the last item of the Part Studio tab menu (menus/rmb-tab-partstudio-2.png, dom/context-menus-partstudio.txt rmb-tab-partstudio) and of the Assembly tab menu (rmb-tab-assembly); behaviours 5732 "Part Studio › Export… (tab menu)" documents its dialog. The only capture without it is behaviours 3238, when live's export API was throttled (429): "The Part Studio tab menu also omits Export… while this is throttled." Shot tickets/tab-export-missing-1c85.png Note: two 10-21 live captures (behaviours 6561, 7153) list the menu without Export…, which workflow-p1-pass rows 49/95 followed; behaviours 705 says the item is dropped while live's export API is rate-limited, and 10-21 was such a day (3238). Fresh capture asked (REQUESTS qa 16:19); fix once it is in. (qa 2026-10-27 16:15, green 1c853f78) — **verified f16b31c6 (qa 18:40: Part Studio and Assembly tab menus end with Export…; Export › STEP › Save writes an AP242 file with product "Part 1"; tickets/tab-export-fixed-f16b.png)**
  - psui (landed with this note): Part Studio and Assembly tab menus end with Export… as live's DOM capture
    (rmb-tab-partstudio: … Move to document…, Export…; rmb-tab-assembly: … Move to document…, Export…, Release…);
    it opens the existing Export modal (lpd.noshape on :102: Part Studio 1 › Export… → File name "Part Studio 1").
- [x] P1 HOLE-SIZE-NO-DIAMETER the Hole dialog's Drill size doesn't drive the diameter: (1) new Hole on an empty mm document (Inch tab, 1/4 → 6.35 mm), Drill size › 3/8 (0.375) → the diameter stays **6.35 mm** and the title "Ø 6.35 mm ↧ 12 mm"; Metric tab shows Drill size "5" with 6.35 mm, size 0.8 → still 6.35 mm; (2) the 30-feature part's "Ø 6 mm THRU" (Drill size 1/4 next to a 6 mm diameter, no bubble): Drill size › 3/8 → the hole stays Ø6 (callout "Ø6 THRU"). Live (behaviours L52 / L61 ~8549–8550, 10664): the size sets the diameter (1/4 → Ø6.35; ISO default 5 → Ø5), and a diameter that differs from the size's shows the blue bubble "Parameters have been overridden. Check input." Picking a size from the table is how users choose a hole, so the geometry is wrong. Shots tickets/hole-size-no-diameter-2a6d.png, hole-size-no-diameter-new-2a6d.png (qa 2026-10-27 15:56, green 2a6d0167) — **verified f16b31c6 (qa 18:40: wf-block Hole, Drill size 3/8 → Ø9.525 and title "Ø 9.525 mm ↧ 12 mm"; Metric tab → size 5, Ø5; tickets/hole-size-fixed-f16b.png)**
  - psui (landed with this note): the dialog applied the size table only when std's Hole editing logic was absent,
    but holeEditLogic only switches standards (Onshape's client applies the lookup table); it is applied on every Drill
    size / Standard change and units / style switch now. Empty mm document on :102: Inch 1/4 → 3/8 gives Ø9.525,
    Metric Ø5, 0.8 → Ø0.8. (2) A hole never synced from the table (REST / CLI built, the 30-feature part's Ø6 with
    Drill size 1/4) now shows live's "Parameters have been overridden. Check input." (model-core hole; a dimension
    holding the schema default in either unit system counts as unset, test hole_overridden_parameters_info).
- [x] P2 VAR-BARE-UNIT a variable created from a field's "New variable…" popup with a bare number keeps it unitless in its name: Extrude › Depth `#ht` Tab → Create variable (Length) Value `12` Enter → list row and dialog title read **"#ht = 12"** while the dialog's Value shows "12 mm" and the Variable table "12". Editing the value in the Variable table to `7` gives "7 mm" (right). Live (behaviours 4279 "Bare number: the unit is appended", 1761 "(x) #width = 0 mm"): the stored value gets the unit, the row reads "#ht = 12 mm". Shot tickets/var-bare-unit-2d9a.png (qa 2026-10-27 15:00, green 2d9a275c) — **verified f16b31c6 (qa 18:42: Extrude › Depth #ht › Create variable 12 → row "#ht = 12 mm"; tickets/var-bare-unit-fixed-f16b.png)**
  - psui (landed with this note): the popup's value is committed like a field's (a bare number gets the unit, "1+1"
    → "(1+1) mm"). lpd.noshape on :102: Extrude › Depth `#ht` Tab, Value `12` Enter → row "#ht = 12 mm".
- [x] P2 SKETCH-RMB-ITEMS graphics-area menus of a visible sketch (Sketch 2 = a circle on Front, closed) miss live entries: **circle** right-click has no "Create Drawing of Sketch 2…" (live behaviours 285: … | Hide, Curve/surface analysis… | **Create Drawing of Sketch 1…** | Select ▸ …); **region** right-click has Hide, Section view… but not live's "Hide other sketches, Hide all sketches" (behaviours 292: Hide, Hide other sketches, Hide all sketches, Section view…). Shot tickets/sketch-rmb-2d9a.png (curve left, region right) (qa 2026-10-27 14:48, green 2d9a275c) — **verified f16b31c6 (qa 18:42: circle menu has Create Drawing of Sketch 3…; region menu without Hide other / all sketches here because no other sketch is shown, as live behaviours 14360; tickets/sketch-rmb-fixed-f16b.png)**
  - psui (landed with this note): a sketch curve's menu has "Create Drawing of Sketch N…" after Curve/surface
    analysis… in its own group (no DXF export there), as live. The region's "Hide other sketches, Hide all sketches"
    are shown when another sketch is visible (live's capture had Sketch 1 and Sketch 2 both shown; with one visible
    sketch the menus captured have only Hide); reopen with the document if qa's case had a second sketch shown.
- [x] P3 MENU-TRUNCATE context-menu items are cut with an ellipsis instead of the menu growing: right-click an edge of Linear pattern 1 → "Show dependencies of Linear patter…" (menu ~250 px). Live menus are as wide as their longest item (dom/context-menus-partstudio.txt: 143–198 px boxes sized to content). Shot tickets/menu-truncate-2d9a.png (qa 2026-10-27 14:48, green 2d9a275c)
  - psui: not a bug. Live (behaviours "psui (2026-10-27, P3) MENU-TRUNCATE"): context menus are at most 250 px wide,
    nothing wraps, long items end in "…" ("Show dependencies of Linear pattern…"), as ours.
- [x] P2 PLANE-MENU-SELECTED the default-plane row menu with the plane **selected** and a sketch copied: ours Rename | New sketch…, Offset plane…, Size to Part Studio, Paste sketch | Hide… | Zoom to selection, View normal to | Show dependencies…. Live (behaviours "6.28 Default planes can't be deleted", 19354): Rename, **Paste sketch** (second, before New sketch…), New sketch…, Offset plane…, Size to Part Studio, Show/Hide…, **Section view…**, Add comment, Zoom to selection, View normal to, **Clear selection**, Show dependencies…. Steps: right-click Boss sketch 1 › Copy sketch, click Top, right-click Top. Shot tickets/plane-menu-selected-6011.png (qa 2026-10-27 14:40, green 6011065d) — **verified f16b31c6 (qa 18:42: Copy sketch, Top selected, right-click: Rename, Paste sketch, New sketch…, Offset plane…, Size to Part Studio | Hide, Hide other planes, Hide all planes, Section view… | Zoom to selection, View normal to, Clear selection | Show dependencies…; tickets/plane-menu-fixed-f16b.png)**
  - psui (landed with this note): the default-plane row menu as live's: Paste sketch first in its group; a hidden
    plane offers Show, Show all planes; a selected plane adds Section view… after the visibility items and Clear
    selection after View normal to (Add comment is a cloud feature, left out). lpd.noshape on :102 checked selected,
    unselected and hidden; MENUS line for "Show all planes".
- [x] P2 SKETCH-S-EXTRUDE Preferences › Shortcut toolbars › Sketch: ticking **Extrude** or **Revolve** (the list's first two entries) is saved (part_studio_toolbar.json lists "Extrude") but the sketch S toolbar never shows them, also after a restart; sketch tools work (Slot added → 11th button). `shortcut_cmds::sketch_shortcut_toolbar` keeps only names found in `all_sketch_tools()`, so the two feature tools are dropped. Live (behaviours 3242–3250): the Sketch list starts with Extrude, Revolve and the S toolbar shows what's ticked, in list order. Shot tickets/sketch-s-extrude-6011.png (Slot shown, Extrude not) (qa 2026-10-27 14:35, green 6011065d) — **verified f16b31c6 (qa 18:46: fresh profile, Preferences › Shortcut toolbars › Sketch, tick Extrude, Save → S in a new sketch shows Extrude first, then the default ten in list order; tickets/sketch-s-extrude-fixed-f16b.png)**
  - psui (landed with this note): the sketch S toolbar takes Extrude and Revolve (they finish the sketch and open the
    feature with its regions, as the toolbar's buttons do) and shows the ticked tools in the Preferences list's order,
    as live ("in list order, not tick order"). On :102 with Line, Corner rectangle, Center point circle, Trim,
    Dimension, Slot, Extrude ticked: Extrude, Line, rectangle, circle, Trim | Slot, Dimension.
- [x] P1 SHORTCUT-MAP-FOCUS (parity part-studio.md 1.16, was ✅) the Keyboard shortcuts modal (Shift+/) opens with the "Search shortcuts" box focused, and a second Shift+/ types "?" into it (list empties) instead of closing. Live (behaviours "qa (2026-10-27) Keyboard shortcuts modal", request 1493): focus goes to the shortcut list container (no outline), the search box stays empty; Shift+/ again closes the modal; Esc closes too. Shot tickets/shortcut-map-focus-6011.png (qa 2026-10-27 14:25, green 6011065d) — **verified 1c853f78 (qa 16:02: Shift+/ opens the map with the search box empty, Shift+/ again closes it, Esc closes it; tickets/shortcut-map-fixed-1c85.png)**
  - psui (landed with this note): the list takes the keyboard on open, not the search box (empty); the Part Studio's
    bare-key shortcuts stay quiet while it has it; Shift+/ and Esc close the map. lpd.noshape on :102: Shift+/ opens
    with an empty search box, E does nothing, Shift+/ closes; reopened, Esc closes.
- [x] P1 DIALOG-OK-STATE (parity part-studio.md 5.2, was ✅) the dialog ✓ follows the wrong rule: (a) a new feature with an empty required selection opens with ✓ disabled, but after any parameter edit (Extrude: Shift+E, Depth 25 → Enter) the ✓ turns green and clicking it commits an empty red "Extrude 2" ("Select face or sketch region to extrude"); Mirror with Mirror plane picked and "Entities to mirror" empty/pink also shows a green ✓; (b) an invalid expression (Fillet Radius "abc" / "1+") greys the ✓. Live (behaviours 231, 4290–4294): ✓ disabled (opacity .2) until the required selections are made; invalid input → pink field, "Enter a valid expression.", **✓ stays enabled**. A feature that fails in geometry (fillet radius too big, live's Derived 2) keeps ✓ enabled in both. Shots tickets/dialog-ok-empty-extrude-6011.png, dialog-ok-empty-mirror-6011.png, dialog-ok-invalid-expr-6011.png (qa 2026-10-27 14:20, green 6011065d)
  - psui (10-27): Onshape's bundle (woolsthorpe 425850a4): the dialog's `canSubmit()` is `!haveParamValidationError
    && !isExpressionValidating && isDirty && !createVariableDialogOpen`; the generic feature controller sets isDirty
    on every sendTreeEdit (any parameter edit) and on a regeneration without an error, which reads as (a) enabled
    after the Depth edit, as ours. Quantity parameters propagate their errors to the feature
    (`getDoParameterErrorsPropagateToFeature` true), so an invalid quantity would disable ✓, but behaviours 4290 says
    ✓ stays enabled for "abc". Both cases asked live (REQUESTS psui 10-27 DIALOG-OK-STATE (a), (b)); changed once
    answered.
  - qa (15:42, green 2a6d0167): live's own p1audit crops (onshape-research/live/p1audit/st-*.png, 68 states clicked in live) back psui's canSubmit reading for (a): **new Revolve, Surface tab clicked, nothing picked → live ✓ green**, ours pale (tickets/dialog-ok-revolve-surface-live-ours.png, live left); Revolve › Thin → both pale (dialog-ok-revolve-thin-live-ours.png). So my (a) was wrong as written: live does enable ✓ on an empty dialog once it's dirty; the difference left is that a tab switch doesn't make ours dirty (a Depth edit does). (b) stays with psui's live request.
  - psui (landed with this note): live (behaviours "psui (2026-10-27, P1) DIALOG-OK-STATE") = ours for (a): any edit
    makes the dialog dirty, ✓ enabled, a failing feature commits red; and for (b): an invalid value disables ✓ ("abc",
    "1+", "10 deg"). Fixed the differences: "1+" reads "Syntax error in expression." (was "Enter a valid
    expression."); a value out of its range is a parameter error too (✓ disabled, title "A parameter value is not
    valid.", a count's tip "Enter an integer value between 1 and 2500. Current value is 0."). Checked on :102.
- [x] P1 SPACE-CHECKBOX (parity part-studio.md 5.18, was ✅) Space on a focused dialog checkbox does nothing: Fillet › Radius field, Tab (± button), Tab → Asymmetric has the focus ring, Space (×3) leaves it unticked; same for Partial fillet. Live: Space toggles the focused checkbox (Tab order Depth → ± → checkboxes, behaviours 237–245). Enter/Space on a select still opens it, arrows + Enter pick (OK). Shot tickets/space-checkbox-6011.png (qa 2026-10-27 14:20, green 6011065d) — **verified 2a6d0167** (d6e49c94): Fillet › Radius, Tab, Tab → Asymmetric focused, Space ticks it and shows Second radius (tickets/space-checkbox-fixed-2a6d.png) (qa 15:25)
  - psui (landed with this note): Space toggled the checkbox twice: GPUI turns Space on a focused element with a click
    handler into a keyboard click, and the dialog's tab stops also pressed themselves from their own key handler. The
    tab stops now rely on the keyboard click only (checkboxes, flip / realign buttons, Tolerance options, ?). On :102
    (tubes6, Fillet): Radius, Tab, Tab, Space ticks Asymmetric (Second radius shows); a click on Partial fillet ticks
    it, Space unticks it; one toggle each.
- [x] P2 EXPR-ENTER-COMMITS a boolean converted to an expression commits the whole dialog on the first Enter: Extrude from a sketch, right-click Symmetric › Convert to expression, type `true`, Enter → the dialog closes and Extrude 2 is committed (also with nothing selected: an empty red Extrude 2). Live (behaviours 252, 320): Enter in a value field applies it and keeps the dialog open; a second Enter commits. Shot tickets/expr-enter-commits-6011.png (qa 2026-10-27 14:20, green 6011065d) — **verified 1c853f78** (qa 16:02: Symmetric › Convert to expression, `true`, Enter → applied, dialog open; Enter → committed; tickets/expr-enter-fixed-1c85.png)
  - psui (landed with this note): Convert to expression focuses the new field before it is drawn, so it had no Enter
    baseline and the first Enter fell through to the dialog's commit; the baseline is set when converting. lpd.noshape
    on :102: Extrude, Symmetric › Convert to expression, `true`, Enter → applied (text selected, dialog open); Enter
    again commits. (Whether an empty Extrude may be committed once dirty is DIALOG-OK-STATE (a), asked live.)
- [x] P1 BOX-FILLET-FACES (parity part-studio.md 3.7, was ✅) a window box with Fillet open puts faces in "Entities to fillet": tickets/tubes6.noshape, iso, Shift+F, L→R box around the bottom-right tube → chips 4 × "Face of Linear pattern 1" + the edges (tickets/box-fillet-faces-8357.png). Live (behaviours "psui (10-27) part-studio.md 3.7 box selection with a feature dialog open"): the box result is filtered by the active field and Fillet gets **only edges** (13 edges, no faces), although a click can add a face; Shell / Extrude get only faces (qa 2026-10-27 14:25, green 83577e88) — **verified 203c4d6f** (qa 17:59: tubes6, Fillet open, window box round the bottom-right tube → 4 × "Edge of Linear pattern 1", no faces; box first, then Shift+E → only the 2 planar faces, Add, ✓ green; tickets/box-fillet-edges-203c.png, box-extrude-planar-203c.png); re-verified fa61ba6f (qa 19:15: tubes6, Fillet open, L→R box round the bottom-right tube → 4 × "Edge of Linear pattern 1", no faces; tickets/box-fillet-edges-fa61.png)
  - psui (10-27, this commit): not the in-dialog box: on main a box with Fillet open gives only edges (tubes6 iso,
    L→R round the bottom-right tube: 4 edges; R→L: 6 edges, no faces), as live. qa's 4 faces + edges come from a
    **preselection**: the same box drawn before the dialog is open (selection: 4 faces + 4 edges), then Shift+F,
    gives exactly qa's chips (the 8 entities, the list scrolled after 6 rows), so qa's Shift+F likely reached the app
    after the box. Whether live's Fillet takes the faces of such a preselection is asked in REQUESTS (psui 10-27,
    BOX-FILLET-FACES, preselection); left open until then.
  - psui (landed with this note): live (behaviours "psui (2026-10-27, P1) BOX-FILLET-FACES"): a box drawn with no
    dialog selects the tube's 4 faces and 4 edges; Shift+F then takes all 8 (as ours: qa's 4 faces + edges were this
    case, not a bug), Shell the 4 faces, Extrude only the 2 planar faces. Ours took the cylinders into Extrude too: a
    field of planar faces now takes no cylinder (preselection, box into a field, hover and click; `face_fits`, test
    a_planar_face_field_takes_no_cylinder). tubes6 on :102: box, Shift+E → 2 chips, Add, ✓ green.
- [ ] P3 REORDER-MISSING-MSG after dragging a feature above the one that made its input, its error doesn't name the missing selection: tickets/tubes6.noshape + Split 1 (Part 1 by Front) + Transform 1 (Part 7, Translate by XYZ); drag Transform 1 above Split 1 → red, tooltip "Transform 1 did not regenerate properly: Cannot resolve entities." (tickets/transform-missing-msg-eb43.png). Live (behaviours "(7) Dragging feature rows"): the moved feature fails with its own empty-selection message plus the count, e.g. "Chamfer 1 did not regenerate properly: Select edges or faces to chamfer.\n1 missing selection" (ERR-MISSING-REGION did this for Extrude; it should hold for every feature whose query resolves to nothing). Also: right after the drop the tooltip still showed the row that was under the pointer before ("Split 1" over the Transform 1 row, tickets/drag-stale-tooltip-eb43.png) until the pointer left the row (qa 2026-10-27 13:47, green eb4338a8)
  - psui (landed with this note, model-core regen): a part of this studio that doesn't exist at the feature's place in
    the list is a missing selection like a missing sketch region: left out, so the feature fails with its own
    empty-selection message and "1 missing selection" (test engine::transform_above_its_part_misses_its_part).
    Derived parts (another studio's) are left alone. Still open: the stale row tooltip right after the drop.
- [x] P1 EDGE-ON-PLANE-PICK a sketch line lying in an edge-on default plane can't be hovered or picked: tickets/axp/axis-line.noshape (Sketch 1 on Front: a rectangle and a vertical line on the vertical axis, i.e. in the Right plane), view cube › Front, Revolve, region picked, Revolve axis active, pointer on the line: no pre-highlight and the click adds nothing (tickets/axp/front-hover-eb43.png, zoom front-hover-zoom-eb43.png), while the rectangle's edges hover and pick normally (rect-edge-hover-eb43.png); orbit a few degrees and the line picks. (At rest the line is now drawn grey over the plane's blue edge line, as live; on e598c3a5 it was hidden under it.) Live (behaviours "qa (2026-10-27) Revolve axis on a sketch line lying on the Right plane, Front view (request 1489)"): the line pre-highlights at once (5 px #FFC685 band only along the line) and the click gives "Edge of Sketch 1"; the plane's edge line never takes the hover. Likely the edge-on plane quad wins the pick (or a depth tie) over the coincident edge; edges must win (qa 2026-10-27 13:40, green eb4338a8) — **verified 2a6d0167 (qa 15:35: axis-line.noshape, Front, Revolve › Revolve axis: the line hovers orange, picks "Edge of Sketch 1", the revolve builds; tickets/axp/fixed-2a6d.png)**
  - psui (landed with this note): the Right plane edge-on in Front view has a border segment along its near side,
    nearer the camera than the line in it, and the pick pass drew lines with depth writes, so that segment hid the
    line in the pick buffer (the fill quad has no pixels edge-on). A plane's border now picks as its fill does:
    first, without depth writes (`Instance::pick_behind`), so what lies behind it wins. axis-line.noshape on :102:
    the line hovers (orange band along the line only, the plane's line above it stays blue), the click gives
    "Edge of Sketch 1" and the Revolve builds. Test offscreen::a_line_in_an_edge_on_plane_picks_over_the_plane
    (fails on the old pass).
- [x] P2 PART-HOVER-RED hovering a part in a part-level dialog field outlines it in **red**: lpd.noshape, Search tools › Split (first field "Parts, surfaces, or curves to split" active), pointer on the block's top face → the whole part (silhouette and all edges) is drawn dark red (tickets/phr/split-hover-4e56-e598.png: green 4e568f3f left, e598c3a5 right, same); Transform's "Entities to transform or copy" the same (tickets/phr/transform-lp-hover-4e56.png, left). The click picks Part 1 normally. Live's part hover is the yellow / orange part outline (behaviours "qa4 r5 s10" hover: "the yellow/orange part outline (the whole part's silhouette and edges)"; "psui (10-27) Delete part preview": rgb(255,192,76)); red is live's colour for errors / interference, not hover (qa 2026-10-27 13:25, green 4e568f3f) — **verified 2a6d0167 (qa 15:40: lpd.noshape, Split, hovering the block: grey tint, no red; the orange-outline question stays with live)**
  - psui (landed with this note): a hovered part's entities go to the scene's `accent` list, which was drawn in the
    error red whenever the edited feature failed (an empty Split always does); the red is only for the failing
    feature's inputs now (test edge_looks::a_failing_feature_leaves_the_hovered_part_unred). lpd.noshape on :102:
    Split, hovering the block gives our usual part hover (a light grey tint), no red. Whether live outlines the part in
    orange instead is asked in REQUESTS (psui 10-27 PART-HOVER-RED look).
  - psui (landed with this note): live's look (behaviours "psui (2026-10-27, P2) PART-HOVER-RED look"): a hovered
    whole part (parts field, Parts list row) shows its outline in #FFC685, no tint, inner edges at rest. Ours now
    draws the part's outline in hover orange instead of the grey tint (`with_highlights`: a hovered Body flags its
    faces HOVER | OUTLINE). Left: live's orange replaces the dark silhouette line (2–3 px on the part side, nothing
    outside), ours runs just outside it.
  - psui 10-27 23:30: now as live: a mask of the hovered part's visible faces and a band pass over the frame
    (render hover_outline.rs) put the hover orange on the silhouette line's own pixels (2 at 1×, the part's boundary
    pixel and the one outside), faces and inner edges as at rest; offscreen test
    a_hovered_part_is_outlined_on_its_silhouette.
- [x] verified eb4338a8 (qa 13:40: gone again in green eb4338a8, which has 05ddce60: tubes6 Front view row x675… `F5F8FA… 3B3C3D 3A4D58 4E6877` = 0d75aa8e's line, tickets/sil/front-eb4338a8.png; only greens e598c3a5 / 4e568f3f had it) P1 SIL-AXIS-VIEW-GONE (regression in green e598c3a5, likely 69de3163 "image-based silhouettes can't throw a back face"): in an exact standard view a cylinder's side silhouettes are no longer drawn: tickets/tubes6.noshape (6 revolved tubes Ø40/Ø20 × 30), view cube › Front: each tube is a shaded rectangle with dark top / bottom edges but **no left / right outline**; row through the middle, left side x675…686: e598c3a5 `F5F8FA… D2DADF 546E7D 4E6877` (no line), green 0d75aa8e `F5F8FA… 3B3C3D 3A4D58 4E6877` (the 1 px dark line) (tickets/sil/front-e598c3a5.png, front-0d75aa8e.png, zoom-e598-vs-0d75.png). Iso views still have them. Live (behaviours "perf (10-22 01:00): silhouettes of a cylinder and a sphere", Front view, DPR 1): cylinder side silhouettes are a 1 px #343434-class line with one AA neighbour, like model edges (qa 2026-10-27 13:10, green e598c3a5)
  - psui (landed with this note; qa's line back in eb4338a8 depended on draw details, this makes it certain): not 69de3163 (it is already in the good 0d75aa8e, and nothing render-side changed
    between the two builds); the edge-based silhouette test in `vs_silhouette` (`dot(n1, v) * dot(n2, v) >= 0`
    hides a segment) dropped both tessellation edges round a facet seen exactly edge-on, which an exact standard view
    of a cylinder can give, so whether the sides had a line depended on the tessellation. A segment now hides only
    when both facets clearly face the same way. tubes6 in Front view on :102: left side a 1 px #000000 line, right
    #1F292F + AA. Test offscreen::a_silhouette_beside_an_edge_on_facet_is_drawn (fails on the old test).
- [x] P2 PARTS-SHIFT-RANGE Shift+click in the Parts list adds only the clicked part, not the range: 6 parts, click Part 2, Shift+click Part 5 → Part 2 + Part 5 selected (tickets/parts-shift-click-e598.png; Parts 3, 4 not); live selects the whole range (behaviours "qa4 round 4 s5 multi-part menu" (1): click Part 2, Shift+click Edited → all 3, like the feature list); ours does ranges in the feature list (Revolve 1 → Linear pattern 1 OK). Also: right after the Shift+click the list and viewport still showed the old state (Part 5 only hovered, its tooltip up) until the pointer moved again (qa 2026-10-27 13:00, green e598c3a5) — **verified 2a6d0167 (qa 15:38: tubes6, Part 1 then Shift+Part 3 → Parts 1–3 in the list and amber in the viewport)**
  - psui (landed with this note): Shift+click on a part row selects the rows from the last clicked part to it, as
    the list shows them (filtered rows left out), with the feature list's rule; the viewport updates at once (tubes6
    on :102: Part 2, Shift+Part 5 → Parts 2–5 amber).
- [x] P1 GPU-TEST-SEGV (lead 2026-10-27) GPU test binaries crash now and then with SIGSEGV inside libvulkan (lavapipe) on
  this machine: ops `--test all` (hidden_edges, per core) at 09:31 and render `--test all` (offscreen) at 12:13; the next
  run passes. The watcher retries T1 once on it (/tmp/lead/flaky.log counts them). Find the cause (Renderer/Instance
  creation per test vs one shared device, lavapipe thread-safety, a missing device wait before drop) and fix it so a
  GPU test never crashes the process.
  - psui (landed with this note): reproduced: the render test binary in three parallel loops died with SIGSEGV once
    in 27 runs (rc 139). Under gdb the crash is on the main thread in `__run_exit_handlers` after every test passed,
    jumping to unmapped code (`?? ()`) while a pipeline-compile thread is still up: an exit handler / destructor of a
    driver library (lavapipe's ICD or Mesa's EGL, both loaded per wgpu Instance) after that library was unloaded.
    Every `Renderer::new` made its own Instance + device and dropped it, so the loader loaded and unloaded the
    drivers for each test. Now `Renderer::new` / `new_async` use one process-wide adapter, device and queue
    (`renderer::shared_device`), made once and never dropped (the app's Metal-matching path keeps its own). After:
    90 runs in three parallel loops, no crash; the render tests take 24 s instead of 45. Stress test
    offscreen::renderers_on_many_threads_share_the_device (8 threads × 4 renderers). The ops crash (hidden_edges)
    renders through scene::png::Offscreen → Renderer::new, the same path.
- [x] verified e598c3a5 (qa 12:46: plate.noshape new Extrude: ✓ #CCE9CC with the pointer on it while nothing is picked, #74CE3A hovered once a face is picked) P3 OK-DISABLED-HOVER a disabled ✓ lights up on hover: new Extrude with nothing picked, ✓ is the disabled pale green
  #CCE9CC, but with the pointer on it it turns the enabled #74CE3A (and stays inert), so it looks clickable
  (tickets/empty-commit-disabled{,-hover}-0a25.png). Live's ✓ is `button-ok disabled` and a disabled button has no hover
  look (qa 11:43, watcher build 0a25dd1a (11:32; not a green))
  psui (landed with this note): the feature dialog's ✓ keeps its disabled colour under the pointer and pressed (the
  other ✓ buttons already skipped their hover when disabled); checked on :102: #CCE9CC hovered while disabled,
  #74CE3A hovered once a face is picked.
- [x] verified e598c3a5 (qa 12:48: rev doc, Configure Part Studio, row Large, Linear pattern 1 Distance configured: click → blue frame, typing 70 keeps the frame, Enter → "70 mm"; double-click → frame + value selected; config Large regenerates at 70 mm) P3 CONFIG-CELL-EDIT-LOOK the configured value cell in the Configuration panel shows no edit state: after a click
  (framed) and typing, or after a double-click, the cell loses its blue frame and shows the typed text plainly, with no
  input box, caret or selection highlight (tickets/config-cell-edit-0a25.png, config-cell-dblclick-0a25.png). Live edits
  in an `input.os-param-number` with the value selected on a double-click (behaviours "qa (2026-10-27, P2 batch 2)" (6)) (qa 11:43, watcher build 0a25dd1a (11:32; not a green))
  psui (landed with this note): editing keeps the cell's blue 2 px frame round the in-place input. qa's shots show no
  caret or selection because the window had no X focus (GPUI draws neither in an inactive window; `xdotool
  windowfocus` after starting the app); with focus, a double-click shows the value selected (checked on :102).
- [x] verified eb4338a8 (qa 13:38: plate.noshape iso, column through the top-front edge, 1×: rest 1 px #000000; hover E2C399 E6C497 CFA574 D2AA78 FFC685 (+ one AA px A7977E onto the dark side) vs live E3C399 E5C192 CFA473 D7AF80 FFC685; selected E2AD3D E6AD37 CF8F14 D29319 FFA90A vs live E3AE3D E5AA31 CF8E13 D79920 FFA90A; hovered + selected = selected; no dark outline. The rest line stays #000000 vs live #212C32 (1 CSS px either way)) P1 EDGE-HIGHLIGHT-LOOK (user-reported, v0.4.10 macOS) a hovered model edge is a light band with a dark border on
  both sides; hovered and selected edges must match live exactly (width at 1× and 2×, colours, outline or not). User
  shot: /root/qa-scratch/tickets/ehl/user-report.png (zoom user-edge-zoom.png). Reproduced on the v0.4.10 Linux
  artifact (tickets/ehl/plate.noshape = bin/platedoc.py: 300 × 200 × 3 plate, patterned Ø10 bosses; iso, hover the
  top-front edge away from its midpoint; column profiles across the edge, top → bottom):
  | state | ours 1× (v0.4.10) | live 1× (behaviours 18762, psui 1456) |
  |---|---|---|
  | at rest | 1 px **#000000** | 1 px (20,27,30) + AA (99,131,150) / (64,85,97) |
  | edge hovered | dark 1 px #171F23, **6 px** band FFC685 FFC685 FFD3A0 FFD3A0 FFD3A0 FFC685, dark 1 px #1A2328 | dark 2 px (26,35,40) (29,29,29), **5 px** band (253,198,135) (255,211,160)×2 (241,197,146) (255,198,133), dark 1 px (15,19,22) |
  | edge selected (pointer on background) | 5–6 px BDA96A FFA90A D99C23 CB8A0F D99C23 F8A40A, no outline | 5 px (251,170,18) (221,161,41) (203,138,15) (213,151,29) (255,169,10), no outline |
  | selected + hovered | outline + FFA90A / FFBC40 core | outline + (255,188,64) core |
  - **2×** (GPUI_X11_SCALE_FACTOR=2, tickets/ehl/v0410-2x-hover*.png): hover = 2 px #1A2328, 11 px band (3 FFC685 +
    5 FFD3A0 + 3 FFC685), 2 px #1A2328: every width doubled. The user's 2× macOS shot has exactly this structure (2–3 px
    #1C2327, 3 + 5 + 3 band in display-profile colours #F6C88E / #F8D5A7), so ours is consistent across platforms.
  - So against live's 1× capture: the dark outline on hover **is** live's (the user's "unlike live" conflicts with psui
    1456 — the pending capture REQUESTS "psui (2026-10-27, P1, from the user) EDGE-HIGHLIGHT-LOOK" should settle it,
    especially at 2× / Retina); the differences already measurable are the hover band (6 px with a 3 px core vs 5 px
    with 2), the outline split (1/1 vs 2/1) and the rest line (#000000 vs (20,27,30)). At 2× the question is whether
    live doubles the band and outline (ours: 15 px in all).
  - Hovering the top face while its edge is selected draws the edge with the hover outline (face hover lights its
    boundary); live draws a hovered face's boundary 3 px (255,198,133) with no outline (behaviours 18762 "face
    hovered"). To compare once live's capture is in.
  - The black region at the bottom right of the user's shot did **not** reproduce on v0.4.10 Linux (1× and 2×): iso,
    zoom into the plate's corner down to its vertex, tilts above and below the top face, bottom view. Not filed as
    BLACK-REGION; needs the user's document (or it is macOS / Metal-only) (qa 2026-10-27 11:13, v0.4.10 artifact)
  - psui 69de3163: the user's black wedge was the image-based silhouette pass throwing a back face across the screen
    (`vs_backsil` divided by a finite step's on-screen length, which can come out as nothing); its push is now
    analytic, test a_thin_plate_draws_nothing_past_its_outline (1× and 2×). The edge looks wait for live's 2×
    capture.
  - psui (landed with this note): the looks follow live's capture (behaviours "psui (2026-10-27, P1)
    EDGE-HIGHLIGHT-LOOK"): a hovered / selected part edge is a 5 CSS px band in one colour (#FFC685 / #FFA90A), its
    rim opaque and its core at 78 % over the scene, no dark outline; hovered + selected = selected; a face's boundary
    is a 2 device px line; plain edges 1 CSS px. Also gone: a faint shadow beside a hovered edge (the ambient
    occlusion pass read the band's lifted depth; lines no longer write depth). Measured against live's rows at 1× and
    2× (within ±4), test scene edge_looks::edge_and_face_highlights_look_as_live (fails on the v0.4.10 look).
    Not followed: live renders at the real pixel ratio only on Macs without an Intel GPU (we do everywhere).
- [x] verified watcher CLI 12:08 (≥ green 0d75aa8e; qa 12:10: config_add_input List "Size" → List_mRB7Pn8k5H0Vmv, a 2nd List → List_c5l6FBA6sJtcRi; a Checkbox still gets "Checkbox_3", live's form for checkboxes not captured; dx 569b14e6: live's Checkbox dialog: Name required, id from it ("My flag" → My_flag), Default checked; the app's dialogs are psui's, handoff request) 8d9246ab P3 CONFIG-INPUT-ID a Part Studio's new configuration inputs get FeatureScript ids from their names: config_add_input
  List → id "Configuration" (renamed "Size", the id stays "Configuration"), Checkbox → "Checkbox_2" (ops/config.rs
  add_input → fs_identifier(name)). Live gives a List a random internal id `List_8iFesnytxapQpg` (behaviours 5915, 6098:
  config string `List_YC7FakBBQ6r7FX=Default;…`), which our Variable Studio already copies (ops/vs_config.rs:35). The id
  shows in Edit FeatureScript IDs… and in configuration strings (Derived, API, export names), so Part Studio and
  Variable Studio disagree. Use the List_… form (and live's Checkbox form, unknown: one capture of Edit FeatureScript
  IDs for a new Checkbox settles it). Rest of the configuration matrix fine: List × Checkbox (suppression) × config
  Variable (#h as Extrude 2 depth), 8 / 8 volumes exact, activation leaves history alone, reset right
  (bin/cfgmatrix.py, CLI 09:50) (qa 2026-10-27 10:44, CLI 09:50)
  psui: a new List input's id is live's random List_… form (`vs_config::list_id`, as the Variable Studio's);
  Checkbox and Variable keep FeatureScript ids from their names until a capture shows live's Checkbox form.
- [ ] (qa 12:10, green 0d75aa8e: Ctrl+Space in the editor now stays in the Feature Studio (11e7e6aa verified); no completion list yet, waits for live) P2 FS-EDITOR-COMPLETE the Feature Studio editor has no code completion, and Ctrl+Space in it leaves the tab. Steps:
  /root/qa-scratch/tickets/cfs2.noshape › Feature Studio 1 › end of the source, type `opExt` (wait), then Ctrl+Space.
  Ours (green e8858e61): nothing drops down while typing; Ctrl+Space runs the document-wide "back to the previous tab"
  (app-core actions.rs:185 SwitchRecentTab) and jumps to Part Studio 1 mid-edit (tickets/fs-ctrl-space-before.png /
  -after.png). Live: the FS editor has an autocomplete list (its command list has DISMISS_AUTOCOMPLETE, Esc;
  live-exploration.md 5.11); what opens it (typing, Ctrl+Space) and what it lists are asked in REQUESTS "qa (2026-10-27,
  FS-EDITOR-COMPLETE)". At least Ctrl+Space shouldn't switch tabs while the code editor has focus (qa 2026-10-27 10:40,
  green e8858e61)
  - psui (this commit): Ctrl+Space no longer switches tabs from the code editor (binding context
    `actions::RECENT_TAB_CONTEXT`, "Workspace && !(FeatureStudio > Input)"; checked on cfs2.noshape). The completion
    list waits for live's answer (REQUESTS "qa (2026-10-27, FS-EDITOR-COMPLETE)").
- [x] verified watcher build 0a25dd1a (11:32; not a green) (qa 11:43: cfs2.noshape A `EDGE && ALLOWS_AXIS`, B `EDGE && LINE`, C `EDGE && SOLID`, D `ALLOWS_AXIS` all pre-highlight and take the part's edge; A, B, D also the sketch line; tickets/lp-compound*-0a25.png. Not in v0.4.10) 1e3c419c P1 LP-DIRECTION-PICK (user-reported) hover / pick read nothing in feature dialogs. Repro doc /root/qa-scratch/lpd.noshape
  (Sketch 1 rectangle 60×40, Extrude 1 10; Sketch 2 on the top face 10×10, Extrude 2 Add 8). Linear pattern › Feature
  pattern › Features to pattern = Extrude 2 › click Direction › hover / click the front top edge.
  - **v0.4.9 release** (Linux tarball, 7ee597e0 + fixes): the edge at its middle shows no pre-highlight and the click picks
    nothing (Direction stays empty; tickets/lp-pick-v049-feature-midpoint.png); the same in Part pattern (Entities = Part 1;
    lp-pick-v049-part.png); a click inside the top face picks the **Front plane** behind it instead of the face
    (lp-pick-v049-face-falls-to-plane.png); a plane from the feature list (Top) is taken. (One edge pick away from the edge's
    middle did go in, so it isn't every hover, but the user's steps fail.) Cause per the lead: 8bf820e0 (preview mix);
    fixed by 4e55b01f, which is in green but not in v0.4.9; v0.4.10 (from e8858e61) carries it.
  - **Green e8858e61** (has 4e55b01f), same steps, same pixels: Feature / Part / Face pattern: the edge pre-highlights and
    goes in as "Edge of Extrude 1"; the top face goes in as "Face of Extrude 1"; Direction filled first, then Features, also
    works (lp-pick-green-feature / -part / -face / -direction-first.png); Fillet edges and Shell faces hover + pick
    (pick-green-fillet.png, pick-green-shell.png).
  - **Still broken on green: a custom feature whose query uses a compound filter.** Feature Studio (cfs.noshape) with
    `annotation { "Name" : "Edges", "Filter" : EntityType.EDGE && QueryFilterCompound.ALLOWS_AXIS, "MaxNumberOfPicks" : 2 }`:
    the Edges field is active but straight edges neither pre-highlight nor pick (two different edges tried); the same
    feature with `"Filter" : EntityType.EDGE` alone hovers and picks (pick-green-custom-allows-axis.png, left plain, right
    compound). The API takes the same two edges for the compound feature (feature_add OK), so it is the dialog's pick
    filter that rejects ALLOWS_AXIS. Live: the exact entity types per mode and for the custom query are asked in
    REQUESTS "qa (2026-10-27, P1 LP-DIRECTION-PICK)" (A)–(F) (qa 2026-10-27 10:10)
  - Likely cause of the compound-filter part (code read, qa 10:15): crates/custom/src/custom/studio.rs `filter_of`
    `Ann::And` intersects the two sides' bits. `EntityType.EDGE` = ANY_EDGE = EDGE | SKETCH_EDGE (params.rs:189), and
    `ALLOWS_AXIS` = LINEAR_EDGE | CIRCULAR_EDGE | SKETCH_EDGE | CYLINDRICAL_FACE | MATE_CONNECTOR (studio.rs:738), so the
    intersection is **SKETCH_EDGE only** and every model edge is rejected. The same holds for `EDGE && GeometryType.LINE`
    / `.CIRCLE` and `EDGE && ALLOWS_DIRECTION`: the "EDGE && LINE -> linear edges" refinement in the comment never
    yields model edges, because `EDGE` doesn't carry LINEAR_EDGE / CIRCULAR_EDGE (or SOLID_EDGE). The
    `std_edge_filters_match_builtin` test only covers std features that have a built-in twin, so it misses custom code.
  - Confirmed in the app (green e8858e61, qa 10:20; doc /root/qa-scratch/tickets/cfs2.noshape: the lpd part plus a line
    in Sketch 3 on Front, four custom features with 2-pick "Edges" fields). Clicking the part's top front edge and the
    sketch line:
    | Filter | part edge | sketch line |
    |---|---|---|
    | A `EDGE && ALLOWS_AXIS` | nothing | "Edge of Sketch 3" |
    | B `EDGE && GeometryType.LINE` | nothing | (not tried) |
    | C `EDGE && BodyType.SOLID` | "Edge of Extrude 1" | — |
    | D `ALLOWS_AXIS` alone | "Edge of Extrude 1" | — |
    B (`EDGE && GeometryType.LINE`) is probably the most common edge filter in custom features; `EDGE && CIRCLE`
    reduces the same way. Face filters are fine (`FACE && PLANE` keeps PLANAR_FACE, `FACE && CYLINDER` keeps
    CYLINDRICAL_FACE) since EntityType.FACE carries those bits. Screenshots:
    tickets/pick-green-custom-{A-axisedge-model-none,A-axisedge-sketch-only,B-edge-line-none,C-edge-solid-ok,D-allows-axis-ok}.png
  - psui: the v0.4.9 part is PREVIEW-NO-PICK (fixed 4e55b01f; render test a_preview_picks_the_edges_of_the_model_with_the_feature
    now covers edges too). The compound filters: custom/studio.rs `filter_of`'s `&&` takes any model edge (EDGE) as
    covering the linear and circular ones, so `EDGE && ALLOWS_AXIS` takes linear, circular and sketch edges, `EDGE &&
    GeometryType.LINE` linear and sketch edges, `EDGE && CIRCLE` circular ones (model test
    custom_features::compound_edge_filters_take_model_edges).
- [x] verified green 0d75aa8e (qa 12:10: live-30-features.noshape just opened: panel "Total regeneration time: 1.13s", Regeneration times dialog "Total regen time: 1.13s"; tickets/perf-total-green-0d75.png) 8cdd189f P3 PERF-TOTAL-MISMATCH the Performance panel and its Regeneration times dialog give two totals for the same studio:
  live-30-features.noshape just opened (regen from cache) → panel "Total regeneration time: 46ms", stopwatch › dialog footer
  "Total regen time: 314ms" (the per-feature column sums to ~314). Live reads one number in both places (behaviours 17899);
  show the same (tickets/perf-total-mismatch.png) (qa 2026-10-27, green dacce5e7)
  psui: the Performance panel's total is the feature times' sum, as the Regeneration times dialog's footer (live:
  one number, the sum, behaviours "spec: core SHELL-SLOW").
- [ ] P3 LEFT-RAIL-WHERE-USED the Part Studio's left rail is Versions and history, Create version…, Document notes, Performance
  errors detected, Release drafts; live's is Versions and history, Create version…, Comments, Document notes, **Where used**,
  Performance errors detected, AI Advisor (behaviours 1559 / 1563). Comments and AI Advisor are cloud; "Where used" (which
  tabs use this element) isn't there; "Release drafts" isn't on live's rail (a Free account has no release management)
  (qa 2026-10-27, green dacce5e7)
- [x] verified watcher build 0a25dd1a (11:32; not a green) CLI 11:30 (qa 11:43: rev.noshape outer cylinders Max dist 94.86833 from (−20, 0, 30) = √(90² + 30²); top circles 90.0, tube 1 vs 3 140.0; Min / axes / centres exact) 1cc65def P1 MEASURE-MAX-DIST Max dist between curved faces comes from sampled points, not the exact maximum: two tubes
  (Ø40 / Ø20 × 30, axes 50 mm apart; /root/qa-scratch/rev.noshape, Part 1 / Part 2 outer cylinders) → ours "Max dist
  94.850 mm" (app status bar) / 94.849984 (query_measure, from (−19.975, −0.997, 30), a mesh vertex, to (70, 0, 0)); exact
  √(90² + 30²) = 94.868 (from (−20, 0, 30)). Parallel axes 50.000 and Min 10.000 are exact. Live measures on the exact
  geometry (Parasolid); a 0.018 mm error shows at 3 decimals (qa 2026-10-27, green 4854cad4)
  Same on circular edges (query_measure, watcher CLI 08:12): the two tubes' top outer circles Max dist 89.9807 (exact 90),
  tube 1 vs tube 3 139.9787 (exact 140); Min dist / Center dist exact (10 / 50, 60 / 100).
  psui: the farthest sampled pair is refined on the exact geometry (model-core measure `refine_max`: each end moves to
  its faces' / edges' locally farthest point from the other, a (u, v) or parameter search kept on the bounded face);
  rev.noshape's tubes read 94.86833 from (−20, 0, 30); test measure::max_dist_is_exact_on_curved_faces_and_circles
  (faces √(90² + 30²), top circles 90).
- [x] verified watcher build 0a25dd1a (11:32; not a green) (qa 11:43: new Extrude, nothing picked: Enter and a click on ✓ keep the dialog open, ✓ drawn disabled #CCE9CC; but see OK-DISABLED-HOVER) 4cc84ae1 P2 EMPTY-COMMIT a feature with an empty required selection can be committed: new Extrude, nothing picked, Enter,
  Enter → ours commits a red "Extrude 1" (green 525c619b). Live keeps the dialog open: the ✓ is disabled (`button-ok
  disabled`), and Enter, ✓ or Ctrl+Enter do nothing until a region is picked (behaviours "qa (2026-10-27, P2 batch 2)"
  (5)) (qa 2026-10-27, green 4854cad4)
  psui: ✓, Enter and Ctrl+Enter commit only a dirty dialog (live `canSubmit`: an edit or a regeneration without an
  error; `FeatureDialog::can_submit`, checked in `accept_edit`); verified on :102: a new Extrude with nothing picked
  stays open through Enter, Enter, Ctrl+Enter; a picked face then commits; an existing Extrude opened for edit
  commits on Enter.
- [x] verified watcher build 0a25dd1a (11:32; not a green) (qa 11:43: cfg.noshape Size › Large › Extrude 1 Depth: a click frames the cell (blue 2 px), typing 35 + Enter → "35 mm", Large regenerates 84800 exact; Esc cancels a typed 99; a double-click edits with the value selected (typing replaces it); the edit state has no visible input, see CONFIG-CELL-EDIT-LOOK) e013107b P2 CONFIG-CELL-EDIT a configured value can't be edited in the Configuration panel's table: Configure Part Studio,
  add row "Big", Linear pattern 1 › Instance count › right-click › Configure (column "Linear pattern 1 / Instance count",
  3 / 3, toast as live); a click, double-click or typing on Big's "3" cell does nothing (no blue frame, no edit), while
  the Name cell takes typing. Setting it through the feature dialog with Big active works (Big 5, Parts (5)). Live's
  table cells are editable like its other panels' (single click frames the cell, typing edits; live asked, qa batch 2
  (6)) (qa 2026-10-27, green 4854cad4)
  Live confirmed (behaviours "qa (2026-10-27, P2 batch 2)" (6)): a single click frames the value cell (blue 2 px,
  `os-table-cell-selected`), typing replaces the value in place (`input.os-param-number`), Esc cancels, a double-click opens
  the input with the value selected; each cell shows "›" and a ± tolerance icon.
  psui: a click frames the configured value cell (blue 2 px, the keyboard on it, Part Studio shortcuts quiet), typing
  replaces the value in place, Enter opens it, Esc unframes, a double-click edits with the value selected; a plain
  number in a length / angle cell commits with the workspace unit ("40" → "40 mm"). Verified on :102 (Extrude 1 /
  Depth, Big).
- [x] verified eb4338a8 (qa 13:36: fresh copies of cli-fillet-hang.noshape, NOSHAPE_PERF=1; with the opening regen in Fillet 2, dragging the bar above it gives "0 evaluated, 7 cached, 4 ms" at once; Suppress Fillet 1 right after regenerates in 1 ms and the view run's Fillet 2 marker is gone within seconds, so the stale run is aborted. Left over, by design (a913de0e): once idle, the document thumbnail re-regenerates the head in the background ("7 features evaluated, 0 cached, 47455 ms", Fillet 2 burning its budget) and stops when the view regenerates again) reopened in part (qa 11:53, green e8858e61, NOSHAPE_PERF=1, fresh copy of cli-fillet-hang.noshape): the visible part
  is fixed: with the opening regen in Fillet 2 (crash marker "Fillet 2", +55 s), dragging the bar above Fillet 2 gives
  schedule_regen #2 → "0 evaluated, 7 cached, 1 ms" at once and the rolled-back model stays (tickets/regen-no-cancel-
  green-rolled.png). But the running run is **not aborted**: its marker stays and it finishes "8 features evaluated,
  0 cached, 84919 ms" 84 s later (tickets/regen-no-cancel-green-perf.txt), burning a core on a result nobody wants, and
  any edit that needs the kernel meanwhile shares it. The fix text says the newer run aborts the running one; the
  blend budget's cancel jump (97081557) evidently doesn't fire. Side note: a SIGTERM'd app mid-regen leaves its marker
  and the next open rolls the document back before that feature (DOC-POISON guard); Ctrl+Q clears it (reopens 8 / 8).
- [x] c0bbde51 P0 REGEN-NO-CANCEL fixed (psui 10-27) — **verified 2a6d0167** (qa 15:27: cli-fillet-hang.noshape copy, NOSHAPE_PERF=1, bar dragged above Shell 1 while the opening regen ran → "0 features evaluated, 6 cached, 0 ms" and the model at once; dropped above Fillet 2 instead it waits for Shell 1 itself, 47 s at load ~38, as slow on v0.4.10 (62.9 s): SHELL-SLOW, not this): a newer run that evaluates fewer features (the rollback bar above the running feature, a dialog on an earlier feature) aborts the running one; its finished features stay cached, so the rollback shows at once (app on cli-fillet-hang.noshape: 0 ms, 7 cached, while Fillet 2 ran). Was: a newer regeneration waits for a stale one instead of cancelling it: open a copy of
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
  - psui (10-27 14:50): the 84 s run in qa's reopen was the document thumbnail, not the view: the opening document had
    no thumbnail, the thumbnailer waited its 60 s for the busy view, then regenerated the whole head in parallel with
    its own Regenerator (0 cached) and no cancel. A thumbnail regeneration now stops when one of the document's Part
    Studio views starts regenerating (`start_thumbnail::regen_started` aborts it) and is made again once things settle.
- [x] verified green e8858e61 (qa 11:56: loft2.noshape, old Loft deleted, Sketch 1 shown again; new Loft picking the circle first then the square, ✓: both Sketch 1 and Sketch 2 get the eye-slash and their outlines go; tickets/loft-sketch-hide-green.png) a9f68288 P2 LOFT-SKETCH-HIDE fixed (psui 10-27): every sketch a feature uses is hidden (a loft both profiles), also one picked in a later edit (model-core `used_sketches`, ops::studio). Was: a Loft hides only its first profile's sketch: Sketch 1 = 40×40 square on Top, Plane 1 = Top
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
- [x] P2 P1-8 caveat (qa5, carried over) a reopened Hole whose points were clicked one by one shows one grouped chip
  "Vertices of Sketch N"; live keeps one "Vertex of Sketch N" chip per click (the grouping isn't stored; details in
  docs/handoff/qa5-routing.md "qa5 open items at the 2026-10-26 stop") (qa5 2026-10-26)
  - psui (landed with this note): a Hole records the sketches whose points were picked from their row (hidden
    parameter `groupedSketches`); reopened, only those show as "Vertices of Sketch N", points clicked one by one stay
    one chip each. wf-block on :102: a new Hole from Sketch 2's row reopens grouped; the older Hole reopens with two
    "Vertex of Sketch 2" chips.
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
- [x] verified green e8858e61 (qa 11:59: lpd.noshape, Shift+W then Shift+D: "Edges to show dihedral" starts empty; 1st Esc closes the Dihedral panel, 2nd cancels the Revolve; the panel now has Units / Max / Min / threshold / Scale; tickets/panel-bleed-green.png) 67bca14a P2 PANEL-BLEED: live bleeds through the same way (behaviours "psui (10-27, PANEL-BLEED
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

- [x] reopened (qa 11:58, green e8858e61 and watcher build 0a25dd1a): the red is gone, but the overlap now renders as a
  **hole**: ps2.noshape › Section view › Front › ✓: where the pin passes through the block, the cap is blank white with
  the Top plane's line showing through it (parity of 2 parts reads as "outside"); live hatches the overlap grey
  (behaviours "skb (10-24) assembly section" (5)). tickets/ps-section-overlap-white-green.png, ps-section-green.png
  - psui (landed with this note): live re-captured (behaviours "psui (2026-10-27, P2) PS-SECTION overlap"): a Part
    Studio's overlap IS flat interference red (198,0,0), unhatched, whatever the parts' colours (the earlier note
    saw no overlap). Part Studio scenes turn `section_interference` on: ps2.noshape on :102, Front section → the
    overlap reads (198,0,0) on every pixel, the rest hatched in each part's colour.
- [x] c93f34b4 P2 PS-SECTION-RED fixed (psui 10-27): `Scene::section_interference` (assemblies only) gates the red overlap in the cap shader; offscreen test overlapping_caps_are_red_only_in_assemblies. Was: Part Studio section caps fill overlapping parts' material solid red; live's Part Studio doesn't
  (behaviours "skb (10-24) assembly section" (5): "overlapping parts' caps are grey-hatched with no red overlap fill";
  only the assembly shows the red interference, part-studio/2406–2414). Repro: /root/skasm-scratch/qa/ps2.noshape (block
  + a pin as a new part through it, made by qa/ps2.py), view menu › Section view…, pick Front, ✓: the pin's cap inside
  the block is red (/root/skasm-scratch/ps-section-red-overlap.png). The shader paints any 2-part count red
  (crates/render shaders.wgsl `fs_section_cap`, `parts > 1.0`) with no per-scene switch; the assembly needs it on
  (checked 10-27, as live) (skasm 2026-10-27, main 98cb32d1)

## Sketch + assembly (skasm)
- [x] 1ca0cf14 P2 ASM-RENAME-ESC Esc doesn't cancel an assembly instance's inline rename: Assembly, select Part 2 <1>, Shift+N (or Rename) → the row's name box opens selected; Esc (pointer over the graphics or the list) → the box stays open, and the next keys (y, Shift+Y…) type into it instead of running their shortcuts (tickets/asm-rename-esc-c9c6.png: three Shift+N / Esc rounds, box open after each Esc); only a click elsewhere closes it. The Part Studio's part / feature inline rename closes on Esc and keeps the old name. Live: Esc in an inline rename reverts and closes it (behaviours "Rename": an empty name resets, Esc cancels) (qa 2026-10-27 21:16, green c9c66d1c)
  skasm 10-27 (1ca0cf14): Esc in an assembly row's inline rename (instance, mate, folder, tab) closes it with the old name;
  the next keys run their shortcuts. Checked on :103 (pointer over the graphics and over the list).
- [x] 1ca0cf14 P2 J-KEY-NO-EFFECT `j` (show / hide mates) does nothing on new mates: assembly with one Revolute (new mates are hidden: grey row, "(Hidden)"), pointer on the graphics, j → no pixel changes, the row stays grey; j again, same (tickets/j-key-no-effect-c9c6.png: row before / after j / after 2nd j). Cause: ToggleMates flips the view's global `show_mates`, but every new mate carries its own Mate.hidden, which wins (assembly-ui assembly_view.rs with_actions). Live (behaviours ~1268): a new mate is hidden by default, "j shows/hides mates"; the mate's Show / Show all mates menu items work in ours (qa 2026-10-27 21:13, green c9c66d1c)
  skasm 10-27 (1ca0cf14): j shows every mate when any is hidden, else hides them all (the mates' own Hidden, as Show all
  mates / Hide all mates). Checked on :103: the Revolute row black after j, grey again after the second j.
- [x] verified c5e53806 (qa 00:15: H+V line made, another line dragged → the H+V line stays whole and red, title / header keep the error; tickets/conflict-no-collapse-c5e5.png) dc9db0a4 P1 CONFLICT-COLLAPSE-ON-DRAG a line with conflicting Horizontal + Vertical collapses to a point (and the sketch reads solved) as soon as another entity is dragged: sketch on Top, Line (450,700)→(650,700) (Horizontal inferred), select it, V → "Sketch could not be solved.", the line red with red H / V glyphs, title red (= live; tickets/hv-line-before-drag-c9c6.png). Now draw a second free line elsewhere and drag one of its ends → the first line's ends jump together (zero length at its old midpoint), its glyphs turn black, the sketch title and the Features header lose their error (tickets/hv-line-collapse-c9c6.png). Same on green daf1f50c (before fb339902) and c9c66d1c (after), so not the SOLVED-WITH-RESIDUAL fix's doing. Live (behaviours "qa4 round 5 s6 constraints" (1)): the conflicting line **does not collapse**, stays red and unchanged, the feature commits WARNING. A drag must not move geometry of an unsolvable component, let alone satisfy both constraints by degenerating it (qa 2026-10-27 21:08, green c9c66d1c)
  skasm 10-27 (dc9db0a4): a drag solves only the components holding dragged geometry; the others are evaluated where they
  are (the H + V line stays, red, the sketch keeps its error), and a drag that would collapse a line doesn't move
  (dragging the H + V line itself). Test solver_basic::a_conflicting_line_does_not_collapse_on_drag. Drag step on 200
  entities 5.2 -> 3.5 ms.
- [x] 07ead631 P3 MATE-ANGLE-360 (regression of MT7, workflow-mates.md) a selected Revolute at 0° draws the full dashed circle with "360°": qa-scratch/mate.noshape (qa5's mkmate.py: two hole plates), Revolute between the two hole centres on the top faces (plates coincident), ✓, click the Revolute 1 row → a full blue dashed circle with arrowheads and a "360°" label on the far side of the X axis (tickets/mate-angle-360-daf1.png); same after a Fastened retyped to Revolute. Live (behaviours ~1324, qa5 z11 / MT7): at 0° no circle, a small blue "0°" on the X-axis end over the red X; the arc and its label only for a non-zero angle (qa 2026-10-27 20:51, green daf1f50c)
  skasm 10-27 (07ead631): an angle within the label's 3 decimals of 0 or 360 is 0: no arc, "0°" at the X axis end.
  Checked on :103 with qa's mate doc (Revolute between the hole centres).
- [x] 5bc2e08e P3 ORIGIN-COINCIDENT-GLYPH a sketch point held on the Origin shows the wrong coincident glyph, or none: (a) a Line started with a click on the Origin (the end is held there: the line rotates about it when dragged) shows **no coincident glyph**, also on hover (tickets/origin-line-no-glyph-dd1b.png); (b) a line end dragged onto the Origin gets a glyph with the **blue external tint** (#def1fb, tickets/dup-coincident-fixed-dd1b.png). Live (behaviours "qa4 r5 s6" (4), s6-4b-editing; REST: COINCIDENT start-to-Origin): the coincident glyph at the origin is the usual **grey-on-white** box, for a line drawn from the Origin (qa 2026-10-27 20:05, green dd1b5037)
  skasm 10-27 (5bc2e08e): a point placed on the Origin gets its own point and a Coincident to the Origin (no longer merged
  into it), so the glyph shows; the Origin's glyphs are grey on white. Test
  a_line_from_the_origin_shows_its_coincident_glyph; checked on :103 (line from the Origin, end dragged onto it).
- [x] verified d2826142 (qa 21:57: line glyphs centred below / left of the line, point glyph up-left with a leader, 27 px dimension hover box, inference icon on a white box; tickets/glyph-h-d282.png, glyph-v-d282.png, glyph-point-d282.png, dim-hover-box-d282.png, inference-box-d282.png) 3cc99587 P1 SKETCH-GLYPH-LAYOUT-LOST (regression; parity sketch.md look rows Line glyphs, Point glyphs, Box, Icon, Dimension value box, Inference glyph box) the sketch's constraint glyphs, dimension hover box and inference glyph are back to the pre-10-25 look: a horizontal line 200 px long gets its "—" glyph 16 px right of the midpoint and 16 px below (tickets/sketch-glyph-h-8602.png), a vertical line's "|" sits **right** of the line and 16 px below its middle (tickets/sketch-glyph-v-8602.png); boxes have radius 2 + shadow and 14 px icons; the inference glyph has no 40 %-white box. Live (sketch/28, sketch/24, part-studio/1032; the rows' own text): a line's row is centred on the midpoint 20.5 px out (below a horizontal line, **left** of a vertical one), points' glyphs up-left with a 1 px #a8b2bb leader, plain 20 px boxes with 18 px icons, a 27 px dimension hover box, the inference icon on a 20 px 40 %-white box. Cause: 45690459 and the app half of 74dc1f8c (both 10-25, crates/app/src/part_studio/sketch.rs) were dropped by psb's merges a7913bbe / 8439c48c ("sketch overlaps resolved to skb's versions"); today's part-studio-ui/src/part_studio/sketch.rs:3143 still has `(6.0 + 19.0 * k, 6.0)`, `rounded(2)`, `shadow_xs`, `icon_px(.., 14.0, ..)`. 74dc1f8c's scene half (yellow 4/4 alignment guide) survived. The same merges also dropped 7fa4f33d's UI half (SKV1, 10-24: the inference glyph as the bare #333 18 px icon ~30 px right / 31 px below the line end; sketch picks at the pointer's pixel centre, `ray(pos + 0.5)`). An audit of every commit touching the old app sketch.rs (`git log --full-history --no-merges`, added lines searched in today's part-studio-ui sketch.rs + scene sketch_mode.rs) found only these three lost; 27c287e7's menu lines were reworked by 39cc2b76. Re-apply the three commits' UI parts. Also: a constraint glyph draws over the dimension box's Tolerance / Precision popup (the Midpoint glyph on top of "No tolerance ▾", tickets/glyph-over-tolerance-popup-dd1b.png); live's glyphs sit under its popups (qa 2026-10-27 19:45, build 965d3f56; same code in 8602df0d)
  skasm 10-27 (3cc99587): the three commits' UI parts re-applied to part-studio-ui sketch.rs (glyph rows 20.5 px
  off a line's midpoint, below horizontal / left of vertical lines; point glyphs up-left with the #a8b2bb leader;
  plain 20 px boxes, 18 px icons; 27 px dimension hover box; inference icon #333 on a 20 px 40 %-white box 30 / 31
  px off; picks at the pointer pixel's centre); the tolerance / precision popup now draws over the glyphs. Checked
  on :103 (horizontal and vertical lines, the inference glyph while drawing).
- [ ] reopened (qa 00:05, green d2826142: half fixed) d5bd5b6b P2 SKETCH-FILLET-STUCK-RADIUS dragging the sketch fillet's arrow manipulator past what the corner allows makes the preview vanish and leaves that radius as the tab's sticky fillet radius, so every later fillet is refused: sketch, two lines meeting at a corner (~23 × 15 mm), Shift+F, click the corner (R5 preview + arrow), press on the arrow and drag it well out past the corner → the preview disappears (tickets/sketch-fillet-overdrag-gone-965d.png); release. From then on **any** corner click, even legs of 250 mm, does nothing but "Cannot fillet selected point. operation failed: fillet radius too large" (tickets/sketch-fillet-stuck-radius-965d.png), and there's no preview to drag or type into, so the tool stays unusable in that tab. Cause: DragState::FilletRadius (scene sketch_mode.rs) writes every drag step's r into self.fillet_radius, which ops keeps per tab (od.sketch_fillet_radius). Live (sketch.md 11.4, behaviours 169): the over-large preview turns red and can be dragged back; a too-large default just previews red; only a committed radius becomes sticky. The message's internal tail is FILLET-POINT-MSG (qa 2026-10-27 19:33, build 965d3f56)
  skasm 10-27 (d5bd5b6b): a drag step whose radius flips the trimmed legs (or doesn't solve) keeps the last good
  radius, so the preview stays and drags back; the tab's default radius changes only when a fillet is committed (or a
  committed fillet's arrow is released). Test sketch_mode::tests::fillet_overdrag_keeps_the_preview_and_the_default.
  qa 00:05 (green d2826142): the over-drag now keeps a preview and Enter commits the largest radius that fits (R37.27 on
  that corner, tickets/fillet-overdrag-commit-d282.png). But that radius becomes the sticky default, and a new smaller corner
  (legs ~15 / 11 mm) is then refused outright: "Cannot fillet selected point.", no preview, nothing to drag or type
  (tickets/fillet-sticky-refused-d282.png). Live (sketch.md 11.4): a radius too large for the corner previews red and can be
  dragged / typed smaller; the tool is never locked out by the sticky value.
- [x] bf1edd44 P3 SKETCH-PATTERN-GLYPH-HOVER hovering a sketch linear pattern's ⊞ glyph doesn't light the pattern: sketch on Front, ellipse, Linear pattern (3x, 25 mm), Enter; hover the ⊞ glyph → its background stays white and only the instance whose curve passes under it turns orange (tickets/sketch-pattern-glyph-hover-965d.png); right-click there does give "Edit pattern". Live (behaviours "icons2 … Pierce and sketch Linear pattern", sketch/2222): the glyph background turns #fcda7f, **all instances' curves yellow #f2ff80 (seed included)**, their centre points and the dashed construction line orange #ffc685; live also draws the glyph left of the seed, ours sits right of the seed centre on the next instance's curve. Other glyphs are right: the Equal glyph turns #fcda7f and both ellipses orange (tickets/sketch-equal-glyph-hover-965d.png) (qa 2026-10-27 19:31, build 965d3f56)
  skasm 10-27 (bf1edd44): hovering the glyph: #fcda7f, all instances' curves yellow (seed included), centre points
  orange; the glyph a plain 20 px box left of the seed; right-click still Edit pattern. Checked on :103. (We draw
  no dashed construction line through the centres: the pattern is held by PatternLink constraints.)
- [x] fb339902 P0 SKETCH-SOLVED-WITH-RESIDUAL the solver reports `Solved` while a constraint is still violated:
  `cargo test -p noshape-sketch --release --test all -- --ignored`, `fuzz::random_sketch_edits_never_panic` seed 2,
  step 55: a drag returns Solved with max_residual 3.6 (the test needs < 1e-6). Clean over 100 seeds × 150 steps when
  44180351 added it, so a regression: `git bisect` on that test. Fix: never report Solved above the solver's
  tolerance (report Failed and keep the previous geometry, as live keeps the sketch); move the fuzz test into T1 at a
  seed count that fits (docs/BOOLEAN-SPEEDUP.md last section; lead 10-27)
  skasm 10-27 (fb339902): bisected to ee23767c (drags beside conflicting constraints): such a drag reported
  Solved with the failing component's residual. Now Solved only when every equation is within the tolerance, for
  drags too; the drag still moves its geometry (SolveReport::drag_solved) and reports the sketch's status. Release
  fuzz 100 seeds × 150 steps clean (642 s); fuzz::random_sketch_edits_never_panic is T1 again at seeds 1–3 × 60
  steps (1.6 s, covers seed 2 step 55), the wider run T2 (random_sketch_edits_never_panic_wide).
- [x] verified d2826142 (qa 00:08: plate edge selected, double-click Linear pattern 1 → Instances "Part 2 <1>" only; leftover P3: after ✕ the pin and the pattern row stay selected, live clears the selection; tickets/asm-pattern-edit-d282.png) fe348558 P2 ASM-PATTERN-EDIT-TAKES-SELECTION opening an existing assembly pattern for edit replaces its Instances with whatever is selected: asm5 (qa-scratch/asm5.noshape: plate Part 1 <1> fixed, pin Part 2 <1>, Linear pattern 1 of the pin), click the plate's front bottom edge (Part 1 <1> row highlights), double-click Linear pattern 1 → Instances reads **"Part 1 <1>"** instead of the stored "Part 2 <1>" and the preview patterns the plate 5 × (tickets/asm-pattern-edit-takes-selection-fa61.png); ✓ would save it. With nothing selected the same edit shows "Part 2 <1>". Mate edits (Cylindrical 1) and Part Studio edits (Fillet 1 with a face selected) keep their stored selections, so it's the assembly pattern dialog's preselection fill running on edit. Live: an existing feature opens with its stored selections (preselection only seeds a new feature) — **live confirmed** (behaviours "qa (2026-10-27, P2) ASM-PATTERN-EDIT-TAKES-SELECTION"): an edit opens with the stored queries, ignores the selection and clears it; after ✕ the selection stays empty (ours keeps Part 1 <1> and the pattern row selected after ✕) (qa 2026-10-27 19:18, build fa61ba6f)
  skasm 10-27 (fe348558): an edit opens with the stored Instances, the selection becomes them while the
  dialog is open, and cancelling an edit clears it (and the row). Checked on :103 with asm5 (plate edge selected,
  double-click Linear pattern 1 → "Part 2 <1>"; Esc → nothing selected).
- [x] not a bug on a line; point menus verified acd6acce (qa 21:18: a line's endpoint → Confirm Sketch 3 | Clear selection, Select ▸, Select other… | Zoom to fit, View normal to sketch plane | Delete sketch entity, = e52e8015's live spec; tickets/sketch-point-menu-acd6.png) (qa 21:00, green daf1f50c: pointer on the line's middle, line hovered orange → Confirm | Copy / Transform sketch entities | Construction | Extrude…, Revolve… | Curve/surface analysis… | Clear selection, Select ▸, Select other… | Zoom to fit, Zoom to selection, View normal to sketch plane, Align view with vertical axis | Delete sketch entity: no Section view…, as live; tickets/sketch-line-menu-daf1.png; my original shot had the pointer on the endpoint, the point menu is skasm's open live question) P2 SKETCH-LINE-MENU (parity sketch.md 14.24, was ✅; regression since skasm's 10-26 check) right-click a line in an open sketch (one sketch): ours Confirm | Copy sketch entities, Transform sketch entities | Construction | Extrude…, Revolve… | Curve/surface analysis…, **Section view…** | Clear selection, Select ▸, Select other… | Zoom to fit, Zoom to selection, View normal to sketch plane | Delete sketch entity. Live (behaviours 14309, row 14.24): no Section view… in the sketch line menu, and **Align view with vertical axis** after View normal to sketch plane for a line (Add comment is cloud). Shot tickets/sketch-line-menu-203c.png (qa 2026-10-27 18:33, green 203c4d6f)
  skasm 10-27: not reproduced on a line: in the shot the pointer is on the line's **endpoint** (orange), so the menu
  is a point's (Section view…, no Align view). Right-clicking the line itself gives live's menu exactly (no Section
  view…, Align view with vertical axis after View normal; tickets/sketch-line-menu-skasm-line.png, my build 18:58).
  Live's menu for a sketch point isn't recorded: asked (REQUESTS "sketch point menus"); fixed to match once known.
  skasm 10-27 (e52e8015): the point's menu from the shot now matches live too (behaviours "skasm (2026-10-27)
  sketch point menus"): an endpoint gets no Copy / Transform, Construction, Extrude / Revolve, Curve/surface
  analysis, Section view, Zoom to selection or Align; a standalone point adds Copy / Transform; checked on :103.
- [x] verified dd1b5037 (qa 20:05: Point, Shift+F, click it → "Cannot fillet selected point." alone; tickets/fillet-point-msg-dd1b.png) P3 FILLET-POINT-MSG sketch fillet on a standalone point: the prompt reads "Cannot fillet selected point. operation failed: corner must join exactly two curves" (internal error text appended). Live / i18n (sketch.md 13.2): "Cannot fillet selected point." only. Steps: sketch, Point, Shift+F, click the point. Shot tickets/fillet-point-msg-203c.png (qa 2026-10-27 18:31, green 203c4d6f)
- [x] verified dd1b5037 (qa 20:04: free line, its end dragged onto the Origin (one glyph), then dragged away and released twice → still one glyph; tickets/dup-coincident-fixed-dd1b.png) P2 DUP-COINCIDENT dragging a sketch point that is already coincident with the origin (so it can't move) adds **another** Coincident to the origin on every release: line end dropped on the origin (one ✕ glyph), then press-drag that end away and release on empty space → the point stays (right) but a 2nd, then a 3rd identical coincident glyph appears (tickets/dup-coincident-203c.png). Duplicate constraints pile up in the sketch (they're redundant, so no red yet). Live: not recorded for this exact case; asked (REQUESTS qa 18:29). A duplicate of an existing constraint is never useful, so ours should not add it either way. Steps: sketch on Top, line, drag one end onto the origin, then drag that end twice (qa 2026-10-27 18:28, green 203c4d6f)
- [x] 05dab13c P2 ASM-DIR-NO-HOVER assembly Linear pattern › Direction: hovering a part edge gives no pre-highlight (the edge stays black), although a click on it takes "Edge of Part 1 <1>"; once (pin.noshape, Direction active, click on the plate's top front edge ~95 px from its end) the click took **"Vertex of Part 1 <1>"** at the plate corner instead (triad drawn there). Live: the Direction field pre-highlights the edge on hover and takes it (Part Studio fixed in LP-DIRECTION-PICK; workflow-p1-pass row 59 live: pattern along the plate edge). Steps: qa-scratch/pin.noshape (bin/mkpin.py), Assembly 1 with Part 1 + Part 2, Search tools › Linear pattern, click Direction, hover the plate's top front edge. Shots tickets/asm-dir-no-hover-1c85.png (pointer on the edge, nothing highlighted), asm-dir-vertex-1c85.png (vertex pick) Same for Circular pattern › Axis of pattern: a hole rim gets no pre-highlight, the click takes it (green 6cbb24e0). (qa 2026-10-27 16:29, green 1c853f78) — **verified f16b31c6 (qa 18:46: pin.noshape assembly, Linear pattern › Direction active: the plate edge pre-highlights orange on hover and the click takes "Edge of Part 1 <1>"; tickets/asm-dir-hover-fixed-f16b.png; the Circular pattern axis case not re-run)**
  skasm 10-27 (05dab13c): the linear pattern's Direction picked in mate connector inference mode (no edge
  highlight; a click near a corner took the inferred vertex). It picks the entity now: on :103 (plate + pins) the
  edge highlights on hover and the click takes "Edge of Part 1 <1>" along it. Circular / Mirror unchanged (live's
  Mirror plane reads "Mate connector").
- [x] 189663bf P2 DIM-AC-ENTER the sketch value box needs two Enters when the unit autocomplete is open: double-click a dimension, type `1 in` → the list "in" / "isInteger( x )" opens with "in" highlighted; Enter only accepts the suggestion and the box stays open with "1 in"; a second Enter applies (Ø25.4). A user who types the next value at once gets it appended (see EXPR-PARSE-JUNK). Live (behaviours 4781–4784, sketch dimension box): "Enter commits the typed text in one press. It doesn't need a second Enter to pick from the dropdown"; in feature dialogs too (behaviours 7040: the first Enter closes the list and commits). The list component is shared with the feature dialogs (psui). Shot tickets/dim-ac-enter-1c85.png (list open | after one Enter) (qa 2026-10-27 16:06, green 1c853f78). Same in feature dialogs: Extrude 1 › Depth `#w` (list "(x) #w 20 mm" / "New variable…") → Enter #1 only picks the item, Enter #2 applies, Enter #3 commits; live: Enter #1 applies, Enter #2 commits (behaviours 7040) (tickets/dim-ac-enter-dialog-1c85.png) — **verified f16b31c6 (qa 18:40: sketch Ø dimension, "1 in" with the list open + one Enter → Ø25.4; tickets/dim-ac-enter-fixed-f16b.png)**
  skasm 10-27 (189663bf): Enter commits the typed text in one press with the list open (Tab / arrows / click
  still pick from it). UI test part_studio::sketch::ui_tests::dimension_box_enter_commits_with_the_unit_list_open.
- [x] 189663bf P2 EXPR-PARSE-JUNK a value box accepts `1 in26` as **126 in** (Ø3200.4 mm dimension) instead of rejecting it: sketch dimension Ø25 › double-click, `1 in`, Enter (left open by DIM-AC-ENTER), then `26` typed into the same box → "1 in26" + Enter → Ø3200.4. Live: a malformed expression is invalid (pink box, "Enter a valid expression.", behaviours 4290–4293); `1 in26` isn't a valid FeatureScript expression. Likely the quantity parser drops the unit token between digits. Shot tickets/expr-parse-junk-1c85.png (qa 2026-10-27 16:06, green 1c853f78) — **verified f16b31c6 (qa 18:40: "1 in26" + Enter → pink box, ✓ pale, "Enter a valid expression."; tickets/expr-parse-junk-fixed-f16b.png)**
  skasm 10-27 (189663bf): the parser already rejects `1 in26` ("an operator or end of expression"); the 126 in
  came from the box left open by DIM-AC-ENTER with its caret inside the text. Fixed with DIM-AC-ENTER (same UI
  test: `26` after the one Enter leaves 25.4 mm).
- [x] 0537ac91 P1 OTF-FIRST-KEY (parity sketch.md 5.2, was ✅) the first key typed into a sketch value box right after creating geometry is lost: Line, click, click, type `5` → the box still reads 28.025; `0` → the box opens with "0"; Enter keeps the invalid 0 (no dimension). Circle: click, click, type `25` Enter → Ø5 dimension. Corner rectangle: `30` Enter is lost entirely, the next `20` Enter dimensions the width. Live (behaviours 148, 179): the value box takes the typed digits from the first key, Enter applies ("50" → 50 mm). Also in v0.4.10, a little differently: the first digit opens the box **selected**, so the second digit replaces it ("25" → 5; tickets/otf-first-key-v0410.png). Shots tickets/otf-first-key-line-2d9a.png (after 5 / 0 / Enter), otf-first-key-circle-2d9a.png (qa 2026-10-27 15:10, green 2d9a275c) — **verified 6cbb24e0 (qa 16:33: circle "25" Enter → Ø25; line "50" → 50; corner rectangle "30" Enter "20" Enter → 30 × 20; tickets/otf-first-key-fixed-6cbb.png)**
  skasm 10-27 (0537ac91): open_otf_input focused the box, then set_cursor_position focused it again (focus not
  landed yet), which reset the input's "caller placed the caret" flag, so select-on-focus selected the typed key and
  the next key replaced it. Now focus, then set_selected_range(caret). Checked on :103: Line 5 0 Enter → 50 mm;
  Circle 25 Enter → Ø25; corner rectangle 30 Enter 20 Enter → 30 × 20. T1 keystroke tests (c8d26735, GPUI test
  platform, docs/TESTS.md "UI keystroke tests"): part-studio-ui part_studio::sketch::ui_tests::
  line_value_box_takes_the_first_key and rectangle_value_boxes_take_width_then_height (both fail on the old code with
  qa's symptoms).
- [x] f34c32be P0 SKETCH-SOLVE-SPLINE-SLOW one constraint or dimension in a sketch with fit splines takes **3–5 minutes** — **verified** (qa 15:31, lead's CLI 15:14 at main ≥ f34c32be, debug build, load ~40: bin/sk211time.py → Coincident 0.28 s (was 175.6 s), dimension −10 0.48 s (was 295.1 s); same outcomes: "over defined (conflicting constraints)" / "Sketch could not be solved."; log cli/sk211-verify-1530.log): repro
  /root/qa-scratch/tickets/sk211-step89.noshape (Sketch 1: 273 entities — 209 points, 45 lines, 7 circles, 7 arcs, 3
  ellipses, **2 splines** — 97 constraints, dof 280; made by bin/skfuzz.py 211 89), bin/sk211time.py: Coincident
  point `:257` on spline `:215` → **175.6 s**, ends "over defined (conflicting constraints)"; then a dimension −10
  between points `:137` / `:48` → **295.1 s**, fails "Sketch could not be solved." (watcher CLI 12:08 ≥ green
  0d75aa8e, load ~45; the fuzz runs on CLI 09:50 hit 123–290 s on the same steps, 59 of 198 steps over 5 s). A
  backtrace taken mid-solve (tickets/sk211-solver-bt.txt): solver.rs solve_comp → eval → equations.rs:402
  `numeric_grad` → :327 eval → :317 `fit_curve` → nurbs.rs:374 `interpolate` (an LU solve): the numeric gradient
  re-interpolates the fit spline for every perturbed variable, so each Jacobian costs O(vars × spline solve). Live
  solves such edits at once; in the app this would freeze the sketch for minutes (P0 "responsiveness while editing").
  Analytic derivatives for the fit-spline equations (or re-fit once per Newton step, not per variable), and a solve
  time budget that reports failure fast (qa 12:30, CLI 12:08; v0.4.10 timing running)
  skasm 10-27 (f34c32be): the fit spline equations (point on spline, end directions, curvature) have their own
  gradients from the fit's factored, banded system (O(n) a value instead of O(n²) fits a Jacobian row). Same sketch in
  the sketch crate (dev build, load 35–50): the Coincident 68 s → 0.3–0.7 s, the −10 dimension after it 0.2–0.4 s, both
  still "over defined" as before. T1 test sketch solve_time::sk211_constraints_on_a_94_point_fit_spline_solve_quickly
  skasm 10-27 15:05: also 0b0f1abd (regions with long splines 36× faster) and allocation-free spline evaluation: qa's
  bin/skfuzz.py 211 200 on the dev CLI: slowest step 0.74 s (was up to 290 s), mean 0.34 s a step at 300–440 entities
  (each step includes a full sketch_get); sk211time.py: Coincident 0.8 s, the dimension 0.5 s
- [x] verified 4e568f3f (qa 13:12: asm5 Assembly 1 › Replicate, Instances Part 2 <1> (Revolute 1 fills itself): "Match edges on face", tooltip "Edge match scope", last field "Faces to find match in") 12740dfc P2 REPLICATE-EDGE-DEFAULT Replicate on a mate whose connectors sit on circular edges: ours lists the right options
  (Match edges on plane / on face / individual edges) but starts on **Match edges on plane** and keeps the last field as
  "Parts to find match in"; live starts on **Match edges on face** (the select's tooltip "Edge match scope") and renames
  the last field **"Faces to find match in"** (behaviours "qa (2026-10-27, P2 batch 2)" (4)); repro asm5.noshape-like
  plate + pin, Cylindrical on edge centres, Replicate › Instances = the pin (qa 2026-10-27, green 16cdcf6d)
- [x] verified release v0.4.10 (qa 11:56, NOSHAPE_PERF=1, load 47: bigsketch.noshape, drag a corner vertex 20 steps: the sketch regen per step 7–16 ms, median 9, every step under a 60 Hz frame; 204 ms for the commit on release; tickets/sketch-drag-big-v0410-perf.txt) 3bcb5145 P1 SKETCH-DRAG-BIG dragging a corner in a 1000-entity sketch (10 × 10 rectangles + 100 circles, made with
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
- [x] verified CLI 09:50 (qa 11:58: center rectangle on Top: sketch_get lists 4 points + 4 lines, no hidden point; its Midpoint constraints name "origin") 5c94bb51 P2 P1-4 data (qa5, carried over) sketches keep a hidden construction "Origin stand-in" point at (0,0) carrying the
  Horizontal/Vertical constraints (sketch_get lists it); live constrains to the external Origin (qa5 2026-10-26)
  skasm 10-26: fixed on main since 5c94bb51 (in green 4d2f6b0c): sketch_get leaves reference-only geometry out and a
  constraint names its reference ("origin"); e2e `sketch_get_names_the_origin_not_a_hidden_point`. The stored point is
  an external, reference-only entity standing for the Origin (live: external id IB), never drawn, picked or listed.

## Drawings / API / data exchange (dx)
- [ ] P1 XT-ASM-SCALE a Parasolid assembly imports ~1000× too small (new path of 541f78a1): + › Import… of live's /root/onshape-research/live/samples/dx-nist-assembly.x_t (= tests/perf/nist-mtc-crada-assembly.x_t), Import to this document → the tabs come right (Assembly "nist_mtc_crada_assembly_rev-D" with Instances (36), Part Studio "dx-nist-assembly parts" with Parts (8), assembly first, after the active tab; ~22 s), but every part is tiny: nist_mtc_crada_box_rev-D-1 → Volume 0 mm³, Surface area 0.093 mm² (tickets/nist-xt-scale-acd6.png); live: 152.4 × 101.6 × 73 mm (behaviours G41). Parasolid stores metres; this path seems to take them as mm (our own single-part x_t round trip has the right size). Consequences seen: the Assembly and the Part Studio show nothing, also after f (tickets/nist-xt-asm-empty-acd6.png), and the app sat at 100–200 % CPU with a helper at 500 % for 15+ min (meshing at a tolerance meant for real sizes?). Live: the whole import is done and drawn in ~24.5 s. dx's timed_nist_import checks the structure (Assembly + 8 parts), not sizes: add the volumes / bounding boxes (qa 2026-10-27 21:52, green acd6acce)
- [ ] P2 ASM-PARASOLID-FLAT an assembly exported as Parasolid loses its assembly structure: asm5 › Assembly 1 tab › Export… › PARASOLID (default) → tickets/asm5-export-acd6.x_t holds three loose bodies (no assembly / instance nodes); + › Import… of that file → a Part Studio "asm5-export" with Part 1–3 (tickets/asm-parasolid-flat-acd6.png), no Assembly. Live's Parasolid export of an assembly carries the assembly (behaviours "(2) NIST assembly: export, then import": samples/dx-nist-assembly.x_t re-imports as an Assembly plus a "<file> parts" Part Studio, which ours now reads, 541f78a1). STEP keeps the structure in ours (asmrt.py); Parasolid should too, so a round trip through our own export matches live's (qa 2026-10-27 21:30, green acd6acce)
- [ ] P3 DRAWING-SHIFT-SLASH Shift+/ does nothing in a drawing tab: asm5 drawing, click the sheet, Esc, pointer on the sheet, Shift+/ (or ?) → no Keyboard shortcuts modal. The Part Studio opens the modal (tabs General … Drawing) and the Assembly its shortcut list. Live (behaviours ~1189): Shift+/ opens the "Keyboard shortcuts" modal everywhere, with a Drawing tab; in a drawing it would start on that tab (qa 2026-10-27 21:24, green acd6acce)
- [ ] P2 BALLOON-WRONG-ITEM (regression of D8 in the app) Callout balloons in an assembly drawing's isometric view show the wrong BOM item: asm5 Assembly 1 › Create Drawing (ANSI_A_MM, No views), iso view, BOM table (rows Part 1 = item 1 qty 1, Part 2 = item 2 qty 2); Callout on the pin's silhouette edge (highlighted) → balloon **"1"** (should be 2); Callout on the plate's front edge → **"----"** (should be 1) (tickets/balloon-wrong-item-daf1.png; saved drawing tickets/balloon-wrong-item-daf1.noshape: drawing_get lists Balloon 19 text "1", Balloon 23 "----", Bom row_names [Part 1, Part 2]). dx's 10-27 CLI check (d8.py) got "2" on the pin, so the app's pick → instance → BOM item path differs from the API's, or is off by one / takes the wrong body (qa 2026-10-27 20:58, green daf1f50c)
- [ ] P2 DATUM-SCALE-DETACHED datums don't follow their edge when the sheet scale changes: asm5 Assembly 1 › Create Drawing (ANSI_A_MM, No views), front / right / top / iso views at 1:1, Datum A on the right view's bottom edge and B on its top edge (tickets/datum-before-scale-daf1.png); right-click sheet › Sheet properties… › Scale 1:2 › ✓ → the views shrink and the "60" dimension follows its edge, but both datums stay where they were, their triangles now in empty space below / beside the smaller view (tickets/datum-scale-detached-daf1.png). A datum is attached to an edge ("Click the edge or point the datum is attached to"), so it should move with it like the dimension; live asked (qa 2026-10-27 20:55, green daf1f50c)
- [ ] P2 DRAWING-BOX-SELECT (parity navigation.md 6.8, was ✅) in a drawing a left-drag on empty sheet draws no box and selects nothing: pin.noshape › Create Drawing of Assembly 1 (suggested views), press-drag from empty sheet across the top view (or from outside the sheet border) → no rectangle during the drag, nothing selected after (tickets/drawing-box-select-fa61.png, during | after); single clicks select (a centre mark highlights). Live (navigation/44, behaviours 19187): a blue window box (fill #1651B0 at 20 %, border at 60 %) during the drag, then the enclosed items selected. (qa 2026-10-27 19:08, green fa61ba6f)
- [ ] P3 VERSION-FOOTER-STALE a non-release build's start window footer reads "(0.4.6.1c853f78…)" (green 1c853f78, fresh profile): the crate version is still 0.4.6 while releases are at 0.4.10, so a green / CI build looks three releases old in the footer, exports and bug reports. Releases are fine (v0.4.10: "0.4.10.e8e0f62df5d6"). Bump the workspace version with each release (or derive it from the last tag). (qa 2026-10-27 16:11, green 1c853f78; still on green daf1f50c 20:38)
- [ ] P1 DWG-REVOLVE-SEAM drawing views draw a revolved body's seam edge as a visible line: tickets/tubes6.noshape (tubes from a 360° Revolve of a rectangle on Front about a sketch line), Part 1 › Create Drawing of Part 1… › ANSI_A_MM, Suggested views: the Right view is a rectangle split by a vertical line at the axis, and the iso view has a vertical line on the tube's right flank (tickets/sil/dwg-tube-4e56.png, zoom dwg-iso-seam-4e56.png). That is the cylinder's seam (OCCT's periodic face edge, in the sketch plane); the Part Studio's 3D view doesn't draw it, and extruded cylinders (asm5 pins) have no such line in drawings. Live's bodies are Parasolid, whose periodic faces have no seam edge, so a full revolve shows only its outline and circles; seams must be left out of drawing views (HLR input / visible-edge filter), like the 3D view does (qa 2026-10-27 13:20, green 4e568f3f)
- [x] verified watcher CLI 12:08 (0d75aa8e; qa 12:10, bin/cfgasm2.py: `{"Size": "Large"}` is stored as `{"Configuration": "Large"}` and the instance is Large (top z 28); the BOM groups the two Large instances; drawing_create by name draws 60 × 28; `{"Size": "Huge"}` is refused "no configuration `Huge`; configurations: Default (Default), Large (Large)") P2 API-ASM-CONFIG-NAMES assembly_insert / assembly_set_configuration take a configuration by input **name**
  (their own examples: `{"configuration": {"Size": "Large"}}`) and store it, but the instance regenerates in the default
  configuration. Steps (bin/cfgasm2.py, CLI 09:50): Part Studio with a List input named "Size" (id "Configuration",
  options Default / Large: Extrude 1 depth 10 / 20) and a Checkbox "NoBoss" (id "Checkbox_2", suppresses Extrude 2);
  insert Part 1 four times: no configuration, `{"Size": "Large"}`, `{"Configuration": "Large"}`, `{"Checkbox_2": true}`.
  assembly_entities faces: 11 / top z 18; **11 / z 18** (should be z 28); 11 / z 28; 6 / z 10. assembly_list still
  reports `{"Size": "Large"}` for <2>, and the BOM puts <1> and <2> in one row (qty 2). Names work where name == id
  (config Variable "h") and in config_activate. Resolve names to ids (as config_activate does) or refuse an unknown
  input; don't store and report a configuration that isn't applied (qa 2026-10-27 10:45, CLI 09:50)
  The app's Change configuration… is right: on the same instance the dialog shows "Size Default" (the applied value);
  choosing Large + ✓ stores `Configuration: Large` (z 28), but the stale `Size: Large` key stays in the instance's
  configuration map. Side note: the op says a repeat insert "offsets the new instance along X so they don't overlap";
  it offsets by (7.5, 5, 2.25) mm per copy (diagonal; a 60 × 40 part still overlaps), and an insert of the same part
  in another configuration isn't offset. Fix the text or the offset.
  drawing_create has the same fault: `{"configuration": {"Size": "Large"}}` draws the default part (front view 60 × 18),
  `{"Configuration": "Large"}` the Large one (60 × 28). query_mass_properties / query_parts resolve names (48800 both
  ways). Ops taking `configuration`: assembly_insert, assembly_replace, assembly_set_configuration, drawing_create,
  part_configured_properties, query_parts, query_mass_properties, variable_insert_studio, config_set_value.
- [x] verified green 3197362b / watcher CLI 09:50 (qa: Revolute 1 → slider "Slider 1", → fastened "Fastened 1") (dx 10-27: a default name follows the new type keeping its number, a custom name stays (noshape_model::default_names::retyped, the dialog's rule); e2e assembly_with_mates retypes Revolute 1 → Slider 1) P2 API-MATE-RETYPE-NAME assembly_mate editing an existing mate's `type` keeps its old name: asm5.noshape's "Revolute 1"
  retyped to fastened / slider / cylindrical / planar / ball / pin slot stays "Revolute 1" (DOF right in each case: 0 1 1 2 3
  3 2). In the app, retyping in the mate dialog renames the mate (Cylindrical 1 → Revolute: title and row "Revolute 1",
  green 16cdcf6d), so the API gives a different result for the same edit (qa 2026-10-27, watcher CLI 08:30)
- [x] verified green 3197362b / watcher CLI 09:50 (qa: edit Boss 4, no-op unsuppress, undo → "Edit : Boss 4" and Boss 4 back to 5 mm) (dx 10-27: the store's UndoStack::commit overwrote the previous step's changed names with the empty diff of a no-op commit; it now leaves them, test a_noop_commit_keeps_the_previous_edit_undoable; the repro now undoes Boss 4 to 5 mm at the first undo. The app's unchanged ✓ recording an edit is psui's dialog, pending live's answer) P1 UNDO-NOOP-SHIFT a no-op operation pushes an undo step without a history entry, so undo/redo shift by one and
  say the wrong thing: copy of tests/perf/live-30-features.noshape, feature_edit Boss 4 depth 5 → 2 mm, then
  feature_suppress Boss 2 suppressed:false (already unsuppressed: no history entry); history_undo → "Edit : Boss 4" but
  Boss 4 stays 2 mm; history_undo again → "Insert feature : Chamfer 5" but it is the Boss 4 edit that comes back (5 mm);
  history_redo → "Insert feature : Chamfer 5". Found by bin/undochk.py (seed 5: undoing 11 entries ends at 60784.58,
  not the original 60779.48). Related, app (green 4854cad4): opening Extrude 1 and ✓ without a change records "Edit :
  Extrude 1", so the next Ctrl+Z seems to do nothing (live asked whether an unchanged ✓ makes a history entry, qa batch 2
  (7)) (qa 2026-10-27, watcher CLI 08:12)
  Live answered (7): an unchanged ✓ is recorded as an edit there too and costs one Ctrl+Z, so the app's part is as live;
  the bug is the API's: a no-op call that records no history entry must not push an undo step either.
- [x] ba3f2c5e (dx 10-27, checked in green 0d75aa8e on dwg82: d, left edge, right edge → the preview is the horizontal 82 between them, placed by the third click, as live) (dx 10-27: the shared tools' hover took the topmost target and dropped it when not an edge, so an edge under an annotation, a snap point or the pending dimension's preview was never picked; edge-only tools now hit-test edges first (BuiltSheet::hit_where). API: line_to_line on dwg82's two edges now gives 82, e2e drawing_line_to_line_on_two_edges. App half (the second click placing the first edge's length) to re-check on the next green, then close) P2 DWG-DIM-TWO-EDGES drawing Dimension (d): click a view's left vertical edge, then its right vertical edge: ours
  places the left edge's length (82) at the second click; live switches to the horizontal line-to-line distance between
  the two edges after the second click and places it on the third (behaviours "qa (2026-10-27, P2 batch 2)" (2)); repro
  /root/qa-scratch/dwg82.noshape Top view (qa 2026-10-27, green bcf401ca)
  The API does it right: drawing_dimension kind smart, picks on the Front view's two vertical edges ([71.03, 93.858],
  [112.03, 93.858]) → LinearDimension "82" (the distance). But kind line_to_line with the same picks is refused
  ("nothing the tool takes there (click on a view edge …)", picks[0]) (watcher CLI 08:30).
- [x] verified e598c3a5 (qa 12:54: 30-feature part › Export › OBJ › Download writes `Part Studio 1 - Part 1.zip` with the .obj (header, mtllib, g Part 1, 2495 v, 751 vn, 48 `o`, one usemtl) and the colour-named .mtl; see EXPORT-OBJ-ZIP-NAME for two leftovers) (dx 10-27: OBJ downloads as a zip of <name>.obj + <name>.mtl laid out as live's sample (header "# OBJ generated by noshape <version> Units = meters", mtllib, g per part, welded v, shared vn, usemtl, o meshN per face; mtl newmtl named by colour with Ka/Kd/Ks/Tf/d/Ns); 3MF now in metres with an m:colorgroup as live's) P3 EXPORT-OBJ-MTL OBJ export: live downloads a zip with `<name>.obj` + `<name>.mtl`, header
  `# OBJ generated by ONSHAPE BY PTC INC, 1.222 Units = meters`; ours writes one .obj with "# noshape-kernel OBJ export"
  and no material file (units metres = live) (behaviours "qa (2026-10-27, P2 batch 2)" (3)) (qa 2026-10-27)
- [x] verified fa61ba6f (qa 19:11: Part 1 › Export… › OBJ › Export → prompt `…/Part Studio 1 - Part 1.zip`; the zip's .obj / .mtl dated 2026-10-27 19:11; tickets/obj-zip-prompt-fa61.png) 2d68ecd1 (dx: the prompt and its filter say .zip / ZIP for OBJ, URDF and individual files; entries dated at export, UTC) P3 EXPORT-OBJ-ZIP-NAME OBJ export saves a zip under a name the prompt didn't show: Part › Export › OBJ › Export, the save prompt offers `…/Part Studio 1 - Part 1.obj` (tickets/obj-zip-prompt-e598.png; the native dialog's filter is OBJ / *.obj too), but the file written is `Part Studio 1 - Part 1.zip`; and the zip's entries carry no date (DOS date 0: `unzip -l` shows 1980-00-00, extracted files dated 1979-12-31). Live's download is the .zip itself, so the prompt (and filter) should say `.zip` (ZIP) for OBJ, and the entries should carry the export time (qa 2026-10-27 12:54, green e598c3a5)
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
- [x] 13224588 (dx 10-27: counterbore / countersink heads too: the flat shows the through hole, the head as a circle in Counterbore/Countersink up / down, down only with hidden lines; native route, test features_c::counterbore_head_shows_in_flat) (dx 10-27: forms done: the flat pattern knows a form's direction (FlatForm.up, native and std routes) and a drawing's flat view draws up forms in Form up's style (Center), down ones only with hidden lines in Form down's; left: the counterbore / countersink heads) P3 FLAT-FORM-STYLES (dx 10-27, live spec behaviours "dx (2026-10-27, P3) flat pattern line styles in a drawing"):
  a drawing's flat pattern view draws "up" features always and "down" ones only with Show hidden lines, each in its
  Drawing properties › Flat pattern views row: a counterbore / countersink from the top = the hole and its Ø head as
  two solid circles (Counterbore/Countersink up); from the bottom = the hole only, its head with hidden lines shown
  (… down); a form embossed up = its two outline circles in Center (Form up); embossed down = not drawn unless hidden
  lines (Form down). Ours: forms always as solid outlines, no hole heads. Needs from the flat pattern (core:
  sheetmetal/flat.rs FlatForm / FlatMark): each form's direction (up = toward the flat's reference side) and the hole
  heads (centre, Ø, side); then dx draws them with drawing.props.flat_lines.
