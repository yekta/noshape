# Tickets (lead, from 2026-10-26)

T2 failures and QA findings, by area and priority (docs/PRIORITIES.md). The area agent owns its tickets; when one is
fixed, mark it `[x]` with the commit, and move the test back to T1 if it is now deterministic and fast
(docs/TESTS.md).

## Core modelling (core)
- [ ] P0 `model/tests/oracle_asm::assemblies_match_live` CP1 regressed by 339de774 (cylinder frame); fs suspects the
  cylinder axis should point toward the sketch (also CP1, R3, C1). Red at the halt.
- [ ] P0 `model/tests/qa_regressions::shell_that_breaks_the_part_fails` red on macOS arm64 in every CI run of 10-26 (T2
  since then): the shell result differs on arm64.
- [ ] P1 `model/tests/std_oracle::hole_scope_follows_depth_as_live`: "Merge scope at depth 15 mm: []" (Linux, Windows).
- [ ] P1 `kernel/tests/draft_curve::level_parting_line_matches_occt` red on all platforms 10-26 (exact match vs OCCT).
- [ ] P0 SHELL-WALLS Shell silently drops a filleted plate's outer walls: Part Studio, Sketch 1 on Top = rectangle 80 × 50
  + Ø20 circle inside; Extrude 1 the outer region 10 mm; Fillet 1 = the top face (its 4 outer edges + the hole rim), R2;
  Shell 1 = top face removed, 2.5 mm (also 1.5 mm, i.e. below the fillet radius); live: an open box (floor, four walls to
  full height, a tube around the hole; live numbers asked in REQUESTS "qa (2026-10-26, P0 wrong geometry check)"); ours:
  Shell 1 **OK** but only the floor and the hole tube remain, no outer walls (with Chamfer 1 + 2 holes on top: 10887.66
  mm³; ≈15.7k expected by hand); without the Fillet, or without the Ø20 hole, the walls are there. Doc
  /root/qa-scratch/tickets/qa-1026-ps2-shell.noshape (tab Part Studio 2), shots tickets/shell-fillet-walls-missing-iso.png,
  -front.png, shell-fillet-suppressed-walls-ok.png. Likely the same root as qa5's P1-16 (fillet offset collapsing in
  shell.cpp, WIP on agent/qa5, docs/handoff/qa5-routing.md "qa5 open items at the 2026-10-26 stop") (qa 2026-10-26,
  build 4d2f6b0c)
- [ ] P0 SHELL-SLOW Shell takes seconds: the same plate (+ chamfer, 2 holes), Shell 1 2.5 mm → Shell eval 3354 ms; edit to
  1.5 mm → 8759 ms; without the fillet 2502 ms (NOSHAPE_PERF log; box load ~35, so maybe 2–3× slower than idle); live
  shells such parts in well under a second; ours freezes the preview for seconds (qa 2026-10-26, build 4d2f6b0c)
- [ ] P0 REGEN-SPEED edit commit on the 30-feature part: tests/perf/live-30-features.noshape, edit Extrude 1 10 → 12, ✓ →
  28 features re-evaluated in 1007 ms (Linear pattern 1 125, Boss 6 97, Chamfer 5 81, Boss 5 75, Hole 1 67, Chamfer 4 66;
  load ~35 on 16 cores); budget ≤ 200 ms, live's server 171 ms for the whole part; undo/redo of it 7 / 5 ms (cached, OK)
  (qa 2026-10-26, build 4d2f6b0c)
- [ ] P0 PATTERN-OFFPART-PARTS (live check asked) a Feature pattern with Reapply features whose copies land off the part
  makes each off-part copy a new floating part: tests/perf/live-30-features.noshape, edit Linear pattern 1 (Boss 6, Right
  plane, 20 mm, Reapply ☑) count 2 → 6, ✓ → status OK, Parts (5) (plate + 4 floating bosses); fs's a122ed68 (in this
  green) says live drops added copies off the part and keeps one part (oracle CV1) — that rule doesn't reach the Reapply
  path; live answer asked in REQUESTS "qa (2026-10-26, P0 check)"; shot tickets/pattern-offpart-new-parts.png (qa
  2026-10-26, build 4d2f6b0c)
- [ ] P1 P1-16 / P1-6 (qa5, carried over) Shell 1 mm on live's p2 model fails "Could not shell part with selections."
  (live OK 3998.627 mm³ / 33 faces), 2.5 mm 8487.861 / 53 faces (live 8503.961 / 47); cause, WIP branch agent/qa5 and test
  in docs/handoff/qa5-routing.md "qa5 open items at the 2026-10-26 stop" (qa5 2026-10-26, green a08e3eda)
- [ ] P1 Extrude add 308 ms (std logic 55 ms, ~250 ms in ops' add path regens): target ≤ 150 ms (PLAN 2.6).

## Part Studio UI (psui)
- [ ] P1 TAB-NO-PREVIEW a number typed into a feature dialog field and committed with **Tab** never reaches the preview:
  edit Extrude 1 of tests/perf/live-30-features.noshape, Depth 10 → 14, Tab → the field reads "14 mm" but no
  schedule_regen in the perf log and the preview stays at 10 mm (tickets/tab-no-preview-extrude.png); the same in Linear
  pattern 1 (Instance count, Distance + Tab: nothing until Enter or a checkbox change, which then shows all the pending
  values; tickets/tab-no-preview-pattern.png). Enter previews fine. live: Tab commits the field and the preview follows
  (behaviours "Input behaviour"; qa4's verified 66563c00 "Tab → 40 mm and the preview follows"); feature_dialog.rs
  InputEvent::Blur → commit_number + flush_typed looks right, so the Changed event seems lost on the way (qa 2026-10-26,
  build 4d2f6b0c)
- [ ] P2 P1-8 caveat (qa5, carried over) a reopened Hole whose points were clicked one by one shows one grouped chip
  "Vertices of Sketch N"; live keeps one "Vertex of Sketch N" chip per click (the grouping isn't stored; details in
  docs/handoff/qa5-routing.md "qa5 open items at the 2026-10-26 stop") (qa5 2026-10-26)
- [ ] P2 P1-10 (qa5, carried over) Extrude › Remove tool preview pointing away from the part is grey; live translucent dark
  red (live part-studio/234–235) (qa5 2026-10-26)

## Sketch + assembly (skasm)
- [ ] P2 P1-4 data (qa5, carried over) sketches keep a hidden construction "Origin stand-in" point at (0,0) carrying the
  Horizontal/Vertical constraints (sketch_get lists it); live constrains to the external Origin (qa5 2026-10-26)

## Drawings / API / data exchange (dx)
- [ ] P2 VERSION-0.4.6 exported files and the app report version 0.4.6 (STEP header `originating_system 'NOSHAPE 0.4.6'`)
  while the releases are v0.4.7 / v0.4.8: the workspace `version` in Cargo.toml was never bumped; the daily release cut
  should bump it (lead's release script) (qa 2026-10-26, build 4d2f6b0c)
- [ ] P2 `kernel/tests/export::rhino_breps_are_valid` red on Windows 10-26 (sphere volume 4e-5 off).
