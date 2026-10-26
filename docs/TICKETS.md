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
- [ ] P0 SHELL-WALLS Shell silently drops a filleted plate's outer walls (status OK, wrong solid). Repro (live's exact
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
  qa5's P1-16 (fillet offsets collapsing in shell.cpp; WIP on agent/qa5) (qa 2026-10-26, build 4d2f6b0c)
- [ ] P0 SHELL-SLOW Shell takes seconds: the same plate (+ chamfer, 2 holes), Shell 1 2.5 mm → Shell eval 3354 ms; edit to
  1.5 mm → 8759 ms; without the fillet 2502 ms (NOSHAPE_PERF log; box load ~35, so maybe 2–3× slower than idle); live
  shells such parts in well under a second; ours freezes the preview for seconds (qa 2026-10-26, build 4d2f6b0c)
- [ ] P0 REGEN-SPEED edit commit on the 30-feature part: tests/perf/live-30-features.noshape, edit Extrude 1 10 → 12, ✓ →
  28 features re-evaluated in 1007 ms (Linear pattern 1 125, Boss 6 97, Chamfer 5 81, Boss 5 75, Hole 1 67, Chamfer 4 66;
  load ~35 on 16 cores); budget ≤ 200 ms, live's server 171 ms for the whole part; undo/redo of it 7 / 5 ms (cached, OK)
  (qa 2026-10-26, build 4d2f6b0c)
- [x] not a bug (live answered 10-26, behaviours "qa (2026-10-26, P0 check)"): PATTERN-OFFPART-PARTS — live also keeps
  each fully-off copy of a Reapply feature pattern as its own floating part (OK, no message), so ours matches (qa)
- [ ] P1 P1-16 / P1-6 (qa5, carried over) Shell 1 mm on live's p2 model fails "Could not shell part with selections."
  (live OK 3998.627 mm³ / 33 faces), 2.5 mm 8487.861 / 53 faces (live 8503.961 / 47); cause, WIP branch agent/qa5 and test
  in docs/handoff/qa5-routing.md "qa5 open items at the 2026-10-26 stop" (qa5 2026-10-26, green a08e3eda)
- [ ] P2 I1 (qa5, re-checked) the same model imported three ways gives three volumes: live/samples/g25-b.x_t 7583.001,
  .sat 7582.996, .step 7582.693 mm³; live (Parasolid) 7582.713024 for all three. On 10-23 STEP was exact (7582.713) and
  SAT 7585.979: SAT got much closer, STEP drifted by −0.02 (qa 2026-10-26, build 4d2f6b0c)
- [ ] P1 Extrude add 308 ms (std logic 55 ms, ~250 ms in ops' add path regens): target ≤ 150 ms (PLAN 2.6).

## Part Studio UI (psui)
- [ ] P1 TAB-NO-PREVIEW a number typed into a feature dialog field and committed with **Tab** never reaches the preview:
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
- [ ] P2 DELETE-TAB-DIALOG deleting a Part Studio used by an assembly: ours shows plain text "Deleting "Part Studio 1" may
  cause errors in the tab that use it: Assembly 1." (tickets/delete-tab-dialog-ours.png); live: a yellow warning box "⚠
  Deleting this tab will result in failed or missing features in 1 tab" and below it one row per affected tab (tab icon +
  name), [Delete] [Cancel] (live part-studio/1584-p9-tab-delete-confirm.png, behaviours "(7) Delete the "qa5 src" tab")
  (qa 2026-10-26, build 4d2f6b0c)
- [ ] P2 LOFT-PROFILE-LIST Loft dialog with two profiles: ours lists the chips without live's › expander and repeats the
  names in a second, grey, indented list under the field ("Face of Sketch 2 / Face of Sketch 1",
  tickets/loft-profiles-ours.png); live: one list, each entry "› Face of Sketch 1 ×" with its chevron, ↑↓ reorder button,
  nothing below the field (live ui/961-loft-two-profiles.png) (qa 2026-10-26, build 4d2f6b0c)

## Sketch + assembly (skasm)
- [ ] P2 P1-4 data (qa5, carried over) sketches keep a hidden construction "Origin stand-in" point at (0,0) carrying the
  Horizontal/Vertical constraints (sketch_get lists it); live constrains to the external Origin (qa5 2026-10-26)

## Drawings / API / data exchange (dx)
- [ ] P2 RHINO-GREEN the green build (/root/green/noshape, the lead's watcher build) has no openNURBS: Part › Export… ›
  RHINO → toast "Export failed: unsupported: Rhino export needs openNURBS: build with OPENNURBS_ROOT
  (scripts/build-opennurbs.sh)", while RHINO stays offered in the format list. Releases build it (release.yml); the
  watcher's build should match the release configuration so QA tests what ships (lead: watcher build env); and a build
  without openNURBS shouldn't list RHINO (qa 2026-10-26, build 4d2f6b0c)
- [ ] P3 docs/USER_GUIDE.md says "Undo/redo … work across sessions"; they don't (the undo history starts empty after a
  reopen, which is also live's behaviour): fix the sentence (qa 2026-10-26, build 4d2f6b0c)
- [ ] P2 VERSION-0.4.6 exported files and the app report version 0.4.6 (STEP header `originating_system 'NOSHAPE 0.4.6'`)
  while the releases are v0.4.7 / v0.4.8: the workspace `version` in Cargo.toml was never bumped; the daily release cut
  should bump it (lead's release script) (qa 2026-10-26, build 4d2f6b0c)
- [ ] P2 `kernel/tests/export::rhino_breps_are_valid` red on Windows 10-26 (sphere volume 4e-5 off).
