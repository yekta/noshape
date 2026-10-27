# Tickets (lead, from 2026-10-26)

T2 failures and QA findings, by area and priority (docs/PRIORITIES.md). The area agent owns its tickets; when one is
fixed, mark it `[x]` with the commit, and move the test back to T1 if it is now deterministic and fast
(docs/TESTS.md).

## Core modelling (core)
- [ ] P0 COMMIT-LATENCY edit commit on tests/perf/live-30-features.noshape is over budget on main (perf.yml run
  40421862773, abac9266, 10-27): "commit shown in" 315–378 ms (macOS 14) and 241–635 ms (macOS 15), budget ≤ 200 ms
  (PLAN 2.6; perf measured 174 / 149 ms on 10-26 before the later landings). The time is the model: regen[preview]
  285 ms for Extrude 1's edit (Boss 1–6 8–25 ms each, Linear pattern 1 45 ms, Fillet 3 19 ms, Hole 1 18 ms, Fillet 2
  15 ms), then refine 35 ms. Undo / redo are fine (26–71 ms). Logs: the run's perf-macos-14/15 artifacts (psui
  2026-10-27)
- [x] 3f5f69ed (core: extruded cylinders framed as live, z toward the sketch; CP1 matches, out of KNOWN; R3/C1 now flip the pin as live, the solver's −157.5° turn is skasm's) P0 `model/tests/oracle_asm::assemblies_match_live` CP1 regressed by 339de774 (cylinder frame); fs suspects the
  cylinder axis should point toward the sketch (also CP1, R3, C1). Red at the halt.
- [ ] P0 `model/tests/qa_regressions::shell_that_breaks_the_part_fails` red on macOS arm64 in every CI run of 10-26 (T2
  since then): the shell result differs on arm64.
- [ ] P1 `model/tests/std_oracle::hole_scope_follows_depth_as_live`: "Merge scope at depth 15 mm: []" (Linux, Windows).
- [ ] P1 `kernel/tests/draft_curve::level_parting_line_matches_occt` red on all platforms 10-26 (exact match vs OCCT).
- [x] verified 19555ca3 (qa): all five crash documents open in the green app 19555ca3 and the CLI (the failing features
  report errors); cli-delete-crash.noshape still regenerating after 120 s in the app, slow but no crash (qa 2026-10-27)
- [x] bf8aaee2 (core: OCCT patch 0002 fixes the Draft's parallel-extrema read; crash guard: a kernel crash fails its feature; the document opens, Draft 7 "Selected faces could not be drafted.") P0 DRAFT-SEGV-DOC a document whose features include a certain Draft crashes the app and the CLI on every open
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
- [x] bf8aaee2 (core: patch 0002 UnifySameDomain zero-length lines + crash guard; cli-shell-gpf opens, its shells fail with an error; captured inputs in kernel tests crash_inputs) P0 SHELL-SLAB-GPF (regression since v0.4.8: the green app 4d2f6b0c segfaults / aborts opening both documents below,
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
- [x] bf8aaee2 (core: fillet_past_end_faces skips an invalid extended part and a crashing fillet, the next fallback runs: the document gives v0.4.8's fillet results; its later Shell abort fixed by patch 0002) P0 FILLET-SEGV (regression since v0.4.8: the green app 4d2f6b0c segfaults opening the document; v0.4.8 opens it) a
  Fillet segfaults the CLI in OCCT
  (`BRep_Tool::CurveOnSurface` ← `ChFi3d_Builder::PerformIntersectionAtEnd` ← PerformFilletOnVertex ← BRepFilletAPI_MakeFillet
  ← noshape `fillet_past_end_faces` ← op_fillet features.cpp:2920), and the saved document then segfaults on every open
  (DOC-POISON). clifuzz_dbg.py seed 9 step 93 (args in /root/qa-scratch/cli/clifuzz-9d.log "93 fillet-args"); doc
  /root/qa-scratch/tickets/cli-fillet-segv.noshape, backtrace tickets/cli-fillet-segv-bt.txt (qa 2026-10-26, main CLI ~20:18)
- [x] bf8aaee2 + this commit (core: the crash guard fails the feature, "Failed to fillet selections."; the document opens; input in kernel crash_inputs. OCCT's surface/restriction blend still crashes on it: making it fillet is open) P0 FILLET-SEGV-BLEND (also in v0.4.8) a Fillet segfaults in OCCT's surface/restriction blend (null 2D curve:
  `Geom2dAdaptor_Curve::EvalD1` ← Adaptor3d_CurveOnSurface ← `BRepBlend_SurfRstConstRad::Values` ← SurfRstLineBuilder ←
  ChFi3d_Builder::ComputeData / PerformSetOfSurf ← BRepFilletAPI_MakeFillet::Build ← features.cpp:2735), leaving an
  unopenable document (DOC-POISON). clifuzz_nodraft.py seed 32 step 46 (log /root/qa-scratch/cli/clifuzz-32.log); doc
  /root/qa-scratch/tickets/cli-fillet2-segv.noshape, backtrace tickets/cli-fillet2-segv-bt.txt. Needs a guard (validate
  the pcurves before the blend, or run the fillet where a signal can't take the process down) (qa 2026-10-27, main CLI
  ~20:18 and v0.4.8)
- [x] (dx 10-27: crash markers around each feature evaluation (model-core regen::crash_guard); Session::open rolls a Part Studio back above a feature whose evaluation stopped a process, one named history entry, notes in `recovered` (document_open); app and CLI alike since both open through ops::session; checked on cli-fillet-segv.noshape. The kernel crashes themselves stay open for core) P0 DOC-POISON a feature whose regeneration crashes the process is already committed to the document, so the
  document then crashes on every open (DRAFT-SEGV-DOC, SHELL-SLAB-GPF, FILLET-SEGV, FILLET-SEGV-BLEND). Whatever the kernel bug, one bad feature must not
  make a document unopenable: commit after a successful regen, and/or run kernel ops so a crash only fails that feature
  (subprocess / signal guard), and/or open with the last feature rolled back after a crash on open (qa 2026-10-26)
- [ ] P0 FILLET-HANG a Fillet runs for > 15 min (stopped by hand) in OCCT's blend approximation (`BRepBlend_AppFuncRoot::
  SearchLocation` / `NCollection_BaseSequence::Find` ← Approx_SweepApproximation ← BRepBlend_AppSurface ←
  ChFi3d_Builder::CompleteData … ← noshape checked_blend / scaled_blend ← op_fillet features.cpp:3049, `checked`, size 5)
  with no timeout or abort; the app would freeze on it. Opening the saved document in a fresh process hangs too (> 30 min,
  stopped). Second occurrence: fuzz seed 33 (main CLI ~20:18) sat in the same BRepBlend approximation for > 1 h
  (stack tickets/cli-hang2-bt.txt, op_fillet features.cpp:2912).
  clifuzz_dbg.py seed 40 step 9 on the watcher CLI 19555ca3 (log /root/qa-scratch/cli/clifuzz-40n.log "9 fillet-args"); doc
  /root/qa-scratch/tickets/cli-fillet-hang.noshape (history ends "Insert feature : Fillet 2"); stack
  tickets/cli-fillet-hang-bt-tail.txt. Needs a time / iteration budget on blends (fail the feature) and the regen must stay
  abortable (qa 2026-10-27, main CLI 19555ca3)
- [ ] P0 TWO-PROCESS-LOST-EDIT the same .noshape opened by two app processes (e.g. `noshape doc.noshape` twice, or a
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
- [ ] P1 SHELL-WALLS-CORNER (core, from live's shell oracle "spec: core SHELL-WALLS / SHELL-FILLETED-BOX" and "core (2026-10-26 b)")
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
- [ ] P0 REGEN-SPEED edit commit on the 30-feature part: tests/perf/live-30-features.noshape, edit Extrude 1 10 → 12, ✓ →
  28 features re-evaluated in 1007 ms on the first edit, then 362–584 ms (5 more commits, load ~30); at load ~6:
  281 / 441 / 316 / 309 ms (green dev build); v0.4.8 release CLI full regen ≈ 0.3–0.35 s (0.48 s process − 0.15 s start) (Linear pattern 1 125, Boss 6 97, Chamfer 5 81, Boss 5 75, Hole 1 67, Chamfer 4 66;
  load ~35 on 16 cores, dev build with our crates at opt-level 1); budget ≤ 200 ms, live's server 171 ms for the whole
  part; undo/redo of it 7 / 5 ms (cached, OK). Also big-plate.noshape: regen 3.0 s (Extrude 2 2458, Sketch 2 535 ms) vs
  live 617 ms (491 / 116). v0.4.8 release CLI: a full regen of the 30-feature part (query_mass_properties, process
  0.62–0.76 s, ~0.2 s of it start-up) ≈ 0.45–0.55 s at load ~25, still 2.5–3× the budget
  (qa 2026-10-26, build 4d2f6b0c)
- [x] not a bug (live answered 10-26, behaviours "qa (2026-10-26, P0 check)"): PATTERN-OFFPART-PARTS — live also keeps
  each fully-off copy of a Reapply feature pattern as its own floating part (OK, no message), so ours matches (qa)
- [ ] P1 P1-16 / P1-6 (qa5, carried over) Shell 1 mm on live's p2 model fails "Could not shell part with selections."
  (live OK 3998.627 mm³ / 33 faces), 2.5 mm 8487.861 / 53 faces (live 8503.961 / 47); cause, WIP branch agent/qa5 and test
  in docs/handoff/qa5-routing.md "qa5 open items at the 2026-10-26 stop" (qa5 2026-10-26, green a08e3eda)
- [ ] P2 I1 (qa5, re-checked) the same model imported three ways gives three volumes: live/samples/g25-b.x_t 7583.001,
  .sat 7582.996, .step 7582.693 mm³; live (Parasolid) 7582.713024 for all three. On 10-23 STEP was exact (7582.713) and
  SAT 7585.979: SAT got much closer, STEP drifted by −0.02 (qa 2026-10-26, build 4d2f6b0c)
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
- [ ] P1 Extrude add 308 ms (std logic 55 ms, ~250 ms in ops' add path regens): target ≤ 150 ms (PLAN 2.6).
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
- [ ] P2 P1-8 caveat (qa5, carried over) a reopened Hole whose points were clicked one by one shows one grouped chip
  "Vertices of Sketch N"; live keeps one "Vertex of Sketch N" chip per click (the grouping isn't stored; details in
  docs/handoff/qa5-routing.md "qa5 open items at the 2026-10-26 stop") (qa5 2026-10-26)
- [ ] P2 P1-10 (qa5, carried over) Extrude › Remove tool preview pointing away from the part is grey; live translucent dark
  red (live part-studio/234–235) (qa5 2026-10-26)
- [x] verified 19555ca3 (qa) 6db9bb8d P2 DELETE-TAB-DIALOG fixed (psui 10-27): live's 598 px "Delete tabs" with the outlined warning triangle, "Deleting
  this tab will result in failed or missing features in N tab(s)", one row per tab with its icon; within 1 px of
  part-studio/1584. Original report: deleting a Part Studio used by an assembly: ours shows plain text "Deleting "Part Studio 1" may
  cause errors in the tab that use it: Assembly 1." (tickets/delete-tab-dialog-ours.png); live: a yellow warning box "⚠
  Deleting this tab will result in failed or missing features in 1 tab" and below it one row per affected tab (tab icon +
  name), [Delete] [Cancel] (live part-studio/1584-p9-tab-delete-confirm.png, behaviours "(7) Delete the "qa5 src" tab")
  (qa 2026-10-26, build 4d2f6b0c)
- [x] 72e41465 P2 LOFT-PROFILE-LIST fixed: each entry "› name ×", 24 px entries 30 px apart with live's 1 px
  rgba(0,0,0,0.2) divider, nothing under the field; › opens the entry's fields inside the field (its query field active,
  the array's border stays blue), as live psui-loft-00/01. Original report: Loft dialog with two profiles: ours lists the chips without live's › expander and repeats the
  names in a second, grey, indented list under the field ("Face of Sketch 2 / Face of Sketch 1",
  tickets/loft-profiles-ours.png); live: one list, each entry "› Face of Sketch 1 ×" with its chevron, ↑↓ reorder button,
  nothing below the field (live ui/961-loft-two-profiles.png) (qa 2026-10-26, build 4d2f6b0c)
- [ ] P2 PERF-SECTION-PANIC with `NOSHAPE_PERF=1` the viewport render thread panics as soon as a section view cuts a
  part (Shift+X, pick Front plane): "wgpu error: Validation Error … In a CommandEncoder, label = 'frame' / In a pass
  parameter / no begin or end indices were specified for pass timestamp writes" (thread 'noshape-render'), after which the
  viewport is frozen for the rest of the session (the UI keeps working). Without NOSHAPE_PERF the section renders fine, so
  users don't hit it, but every perf run / perf.yml benchmark with a section does. Log tickets/perf-section-panic.log,
  doc tickets/qa-1026-main.noshape (tab Part Studio 6) (qa 2026-10-26, build 4d2f6b0c)
- [ ] P2 EXT-DIRECTION (qa4 round 5, still open on green 4d2f6b0c) ticking Extrude › Direction leaves the new "Extrude
  direction" field inactive (white) and the regions field active, chevron grey; after a region pick the field turns pink
  with a red chevron but never gets focus. live (behaviours "qa4 round 5 s7 dialogs"): on ticking, Extrude direction is
  the active field (border rgb(22,81,176), fill rgb(222,241,251)), title red rgb(170,37,45), chevron rgb(189,48,57);
  after a pick focus moves on (qa 2026-10-26)
- [ ] P2 CHIP-HOVER (qa4 round 5, still open on green) hovering a selection chip in a feature dialog (e.g. "Face of
  Sketch 1" in Extrude 1) shows the chip's tooltip but the viewport doesn't change at all (pixel-identical); live turns the
  hovered chip's entity yellow-green #f2ff80 (qa 2026-10-26)
- [ ] P2 FOLDER-NAME-INLINE (qa4 round 5, still open on green) feature rows › Add selection to folder… opens a separate
  "Folder name" box (tickets/folder-name-box-ours.png); live names the new folder inline in its row (input prefilled
  "New folder"); the separate box belongs to the header's New folder button (behaviours "qa4 round 5 s8") (qa 2026-10-26)
- [ ] P2 DEPTH-RANGE (live answered, behaviours "qa (2026-10-26, P2) Extrude Depth out of range") a length above 500 m
  (e.g. "1e6 mm", "600 m"): live turns the field pink (bg rgb(249,235,236)), keeps the text as typed and the title tooltip
  says "… A parameter value is not valid."; ours shows the message but never makes the field pink and rewrites "1e6 mm" to
  "1000000 mm". A valid value in other units is rewritten into document units on blur ("500 m" → "500000 mm") in both.
  0 mm matches ("Failed to extrude selections, check input.", no pink) (qa 2026-10-26, build 4d2f6b0c)
- [ ] P2 MENU-SKETCH-TAG feature row menu: ours "Edit Boss sketch 1 [Sketch]…" for a custom-named sketch; live "Edit Boss
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

## Sketch + assembly (skasm)
- [x] 3bcb5145 P1 SKETCH-DRAG-BIG dragging a corner in a 1000-entity sketch (10 × 10 rectangles + 100 circles, made with
  sketch_draw shapes; /root/qa-scratch/cli/bigsketch.noshape) re-evaluates the sketch feature per drag step in 72–113 ms
  (green dev build, load ~25; NOSHAPE_PERF "regen: 1 features evaluated"); budget: under one 60 Hz frame. Re-time on a
  release build; the step looks like a full sketch regen (regions included) rather than a solver-only update (qa
  2026-10-26, build 4d2f6b0c)
  skasm 10-27: the step was the sketch regions' hole search (~40 ms); now regions 3.5 ms, the sketch regen ~12 ms (dev
  build under load); perf test scene sketch_perf::big_shape_sketch_drag_step_parts. Re-time on a release build.
  qa 19555ca3: in the green app the drag step's sketch regen is still 50–185 ms (load 41); re-time on a quiet box /
  release build before closing (qa 2026-10-27)
- [ ] reopened (qa 19555ca3: the copy lands at the pointer, selected, Ø dimension copied, no prompt = live; but the
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
- [x] verified dc3a03f6 (qa: name accepted, Derived OK 12000 mm³) (dx 5ef64eb8: a tab name or id is resolved, a non-Part-Studio tab refused with `params.partStudio`) P2 API-DERIVED-BY-NAME feature_add importDerived with `partStudio: "Part Studio 1"` (the tab's name) is accepted and
  the feature fails "Error regenerating."; only the tab id works. Accept the name as tab_* ops do, or refuse it as an
  invalid_argument naming the expected id (main CLI ~20:18) (qa 2026-10-26)
- [ ] P2 OPEN-ERROR-SILENT `noshape bad.noshape` with a truncated file ("sqlite: database disk image is malformed") or a
  non-noshape file ("not a noshape document") opens the Documents window with no message; the error is only in the log.
  Show it (an error dialog/toast naming the file and the reason) (qa 2026-10-26, build 4d2f6b0c)
- [x] (dx 10-27: the scale list is live's, and automatic centre marks go on whole circles only, not on a rounded corner's arc) P2 DRAWING-SCALE Create Drawing › ANSI_A_MM › Suggested views of an ~82×82×20 part: ours picks scale 0.75 and the
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
- [x] fixed on main (watcher CLI ~20:18, verify on green): P2 API-REMOVE-SCOPE (v0.4.8 CLI; please re-check on main) feature_add extrude with operationType REMOVE and no
  booleanScope → "Error: No merge scope selected." (faulty booleanScope); the dialog auto-fills Merge scope = Part 1
  when Remove is picked (live behaviours "Remove … Merge scope = Part 1 (auto-filled)"). AGENT_API.md on main describes
  the auto choices for Add and the Remove direction but not the Remove scope (qa 2026-10-26, v0.4.8 release CLI)
- [ ] P2 RHINO-GREEN the green build (/root/green/noshape, the lead's watcher build) has no openNURBS: Part › Export… ›
  RHINO → toast "Export failed: unsupported: Rhino export needs openNURBS: build with OPENNURBS_ROOT
  (scripts/build-opennurbs.sh)", while RHINO stays offered in the format list. Releases build it (release.yml); the
  watcher's build should match the release configuration so QA tests what ships (lead: watcher build env); and a build
  without openNURBS shouldn't list RHINO (qa 2026-10-26, build 4d2f6b0c)
- [x] (dx 10-26) P3 docs/USER_GUIDE.md says "Undo/redo … work across sessions"; they don't (the undo history starts empty after a
  reopen, which is also live's behaviour): fix the sentence (qa 2026-10-26, build 4d2f6b0c)
- [x] (dx 10-27: exports and the MCP server report `noshape_kernel::VERSION`, the release tag release.yml stamps (NOSHAPE_VERSION), else the crate's, as the app already did; releases built before cut-release.sh bumped Cargo.toml reported 0.4.6 in files) P2 VERSION-0.4.6 exported files and the app report version 0.4.6 (STEP header `originating_system 'NOSHAPE 0.4.6'`)
  while the releases are v0.4.7 / v0.4.8: the workspace `version` in Cargo.toml was never bumped; the daily release cut
  should bump it (lead's release script) (qa 2026-10-26, build 4d2f6b0c)
- [x] (dx c63d2bf5: bound 1e-4, live's spline bands are ±0.1 %) P2 `kernel/tests/export::rhino_breps_are_valid` red on Windows 10-26 (sphere volume 4e-5 off).
