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
  qa5's P1-16 (fillet offsets collapsing in shell.cpp; WIP on agent/qa5). Not a regression: the v0.4.8 release gives
  the same 10887.660 / 10220.907 mm³. **Pattern** (v0.4.8 release CLI, 96 cases, /root/qa-scratch/tickets/shellmatrix.py
  + shellmatrix-v048.txt: plates 80×50×10/30, no hole / Ø20 centred / Ø10 off-centre, top-face fillet R 0/1/2/4, shell
  t 1/1.5/2.5/4): the walls vanish exactly when there is a hole **and t is just above R** (R1 t1.5, R2 t2.5, all 8 such
  cases); t ≤ R, t ≥ 2R and every plate without a hole are fine (qa 2026-10-26, build 4d2f6b0c)
- [ ] P0 SHELL-FILLETED-BOX a box whose 12 edges are filleted R, shelled with t > R, is wrong (status OK); live (behaviours
  "qa (2026-10-26, P0 shell check 2)"): the inner offsets of the fillets collapse and the cavity is a **sharp** box, (a)
  8948.0826 / 32 faces, (b) 7076.0826 / 31, (c) 15476.0826 / 32 = the hand math; ours keeps rounded inner edges. 40×30×20, R1: Hollow t2 → 9016.378 mm³ (outer 23924.083 −
  sharp 36×26×16 cavity = 8948.083); open top t2 → 6680.0 (expected 7076.083); Hollow t4 → 16022.136 (15476.083). t ≤ R
  and R = 0 match to 4 decimals (40 cases). Same in the green app (9016.378). Matrix: /root/qa-scratch/tickets/
  hollowmatrix.py + hollowmatrix-v048.txt (qa 2026-10-26, v0.4.8 release CLI + build 4d2f6b0c)
- [ ] P0 SHELL-SLOW Shell takes seconds, also in the v0.4.8 release: same matrix, plates with a hole + top fillet R ≥ 2 and
  t ≤ R: **8–17.6 s per model** (R2 t1.5 + Ø20 hole: 17.2 s; R4 t1.5/2.5: 8–12 s), R1 cases 2.5–5 s, no-hole cases 0.1–0.7 s
  (whole CLI run, load ~25). In the app (dev build): the same plate (+ chamfer, 2 holes), Shell 1 2.5 mm → Shell eval 3354 ms; edit to
  1.5 mm → 8759 ms; without the fillet 2502 ms (NOSHAPE_PERF log; box load ~35; the green is the watcher's dev build:
  our crates opt-level 1, deps 3). v0.4.8 release CLI on the same doc: query_mass_properties 0.70 s with Shell 1 vs
  0.09 s with it suppressed, i.e. the shell alone ≈ 0.6 s at load ~25; live
  shells such parts in well under a second; ours freezes the preview for seconds (qa 2026-10-26, build 4d2f6b0c)
- [ ] P0 REGEN-SPEED edit commit on the 30-feature part: tests/perf/live-30-features.noshape, edit Extrude 1 10 → 12, ✓ →
  28 features re-evaluated in 1007 ms on the first edit, then 362–584 ms (5 more commits, load ~30) (Linear pattern 1 125, Boss 6 97, Chamfer 5 81, Boss 5 75, Hole 1 67, Chamfer 4 66;
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
- [ ] P1 Extrude add 308 ms (std logic 55 ms, ~250 ms in ops' add path regens): target ≤ 150 ms (PLAN 2.6).

## Part Studio UI (psui)
- [ ] P1 DCLICK-HEAVY on a heavy part a double-click on a feature row doesn't open the feature: tests/perf/fins-400.noshape,
  double-click Extrude 1 (or Linear pattern 1) in the feature list, clicks 1 ms or 150 ms apart → only the row gets
  selected, no dialog (5 tries); right-click › Edit… opens it. The same double-click on a light part opens the dialog at
  once. On fins-400 the first click's selection highlight shows ~0.9 s later (software GL, load ~25), so the click count
  is probably measured when the event is handled instead of from the event's own timestamp; live opens the dialog on any
  double-click (qa 2026-10-26, build 4d2f6b0c)
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
- [ ] P2 PERF-SECTION-PANIC with `NOSHAPE_PERF=1` the viewport render thread panics as soon as a section view cuts a
  part (Shift+X, pick Front plane): "wgpu error: Validation Error … In a CommandEncoder, label = 'frame' / In a pass
  parameter / no begin or end indices were specified for pass timestamp writes" (thread 'noshape-render'), after which the
  viewport is frozen for the rest of the session (the UI keeps working). Without NOSHAPE_PERF the section renders fine, so
  users don't hit it, but every perf run / perf.yml benchmark with a section does. Log tickets/perf-section-panic.log,
  doc tickets/qa-1026-main.noshape (tab Part Studio 6) (qa 2026-10-26, build 4d2f6b0c)

## Sketch + assembly (skasm)
- [ ] P2 P1-4 data (qa5, carried over) sketches keep a hidden construction "Origin stand-in" point at (0,0) carrying the
  Horizontal/Vertical constraints (sketch_get lists it); live constrains to the external Origin (qa5 2026-10-26)

## Drawings / API / data exchange (dx)
- [ ] P1 API-HOLE-STYLE (agent usability; v0.4.8 CLI, re-check on main) feature_schema "hole" lists both `style` and
  `styleV2` (same enum, same description "Style"); feature_add with `style: C_BORE` (or C_SINK) is accepted, status Ok, but
  the hole stays Simple (removed volume = plain hole); only `styleV2` works. Either hide the legacy params the dialog doesn't
  show (style, isV2, holeVersion, …) or map them; an ignored parameter should at least warn. Hole geometry itself checked
  exact for Simple / Counterbore / Countersink × Through / Blind (/root/qa-scratch/bin/holematrix.py) (qa 2026-10-26)
- [ ] P2 API-REMOVE-SCOPE (v0.4.8 CLI; please re-check on main) feature_add extrude with operationType REMOVE and no
  booleanScope → "Error: No merge scope selected." (faulty booleanScope); the dialog auto-fills Merge scope = Part 1
  when Remove is picked (live behaviours "Remove … Merge scope = Part 1 (auto-filled)"). AGENT_API.md on main describes
  the auto choices for Add and the Remove direction but not the Remove scope (qa 2026-10-26, v0.4.8 release CLI)
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
