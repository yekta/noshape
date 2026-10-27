# Handoff: skasm (sketch + assembly), from 2026-10-26

Area (PLAN 2.3): crates/sketch, crates/scene/src/sketch_mode.rs, sketch behaviour in the app (part_studio/sketch.rs),
crates/app/src/assembly_*.rs + assembly_view.rs, crates/model/src/assembly, standard content. Older notes:
docs/handoff/sketch-behaviour.md, docs/handoff/assembly-behaviour.md (skb), checklists docs/parity/sketch.md,
docs/parity/assembly.md.

Setup: worktree /root/projects/noshape-wt/skasm (branch agent/skasm), CARGO_TARGET_DIR=/root/target-skasm, env in
/root/skasm-scratch/env.sh, detached jobs via /root/skasm-scratch/run.sh <log> "<cmd>". GUI: /root/skasm-scratch/ui/
(start.sh, sketch_top.sh, shot.sh, stop.sh; DISPLAY=:103). Live: my own Chrome tab via /root/skasm-scratch/live/r.mjs
(copy of live-pw/r.mjs; tab id in .skasm-tab; CDP connect takes ~35 s under load); scratch Part Studio "skasm poly"
(569dabca19a397e932f2db54) in noshape mm test.

## Landed on main
- 10-27 13:xx (new session, Yekta account): f34c32be SKETCH-SOLVE-SPLINE-SLOW (P0): fit spline equations
  (FitPoint, FitEndDir, FitFitDir, CurvatureNum) have their own gradients from the fit's factored system
  (sketch/src/equations.rs FitDiff; nurbs.rs FitSystem, the fit's banded system; linalg.rs BandLu) instead of
  re-fitting per input. qa's sk211 (94-point spline): Coincident 68 s → 0.3–0.7 s in the sketch crate, CLI 175 s →
  2–4 s (dev build, load 35–50); T1 test solve_time::sk211_… (fixture tests/fixtures/solve/sk211-step89.json).
  26c7a498: a new mate between free parts of one Part Studio moves the later part's instance (live: the plate, Part 2,
  onto the smaller block, Part 1); other studios keep the size rule. Test of live's mate edits (Flip primary axis
  brings the pin back upright, Offset Z 5 mm): mate_flip::editing_a_mate_between_free_parts_moves_the_pin_back_upright.
  00956cb2 DIN 6908 (the previous session's).
- 10-27 afternoon (after the app split): 3d99e40d sketch value boxes as live's rule capture (corner rectangle, 3 point
  circle, center point arc, 3 point arc); 12740dfc REPLICATE-EDGE-DEFAULT; 71bc6f56 ISO 10512 nut (= live's B-rep);
  DIN 6908 washer (= live).
- e5e45de1 + 3f354c8c center point circle's value box opposite the rim click (4 live samples in my tab); the
  committed dimension keeps its usual place (where live puts it after Enter is not recorded). 4b5ac4fe used circle /
  arc centre is construction (live).
- 44180351 sketch edit fuzz (T2) `fuzz::random_sketch_edits_never_panic` (`SKETCH_FUZZ_SEEDS=1..100 SKETCH_FUZZ_STEPS=150
  cargo test -p noshape-sketch --test all fuzz::random_sketch_edits -- --ignored`: clean, 744 s).
- be0dd33e new mate between two free parts: the part live keeps stays (`eval::new_mate_stays`: exact flip → sign rule,
  else the smaller part moves; ties → the negative-Z one), in the mate dialog and assembly_mate (was: the second
  connector's part always stayed). Live runs in my own tab, behaviours "skasm (2026-10-27) follow-up: which part
  moves" (14 + 13 runs). d249b08a the solver holds the unturned part in an exact flip.
- 25dc002f fixed main (RED from psui's 79460041 graphics-menu "Cancel"): coverage entry → history_undo (Ui entries
  are rejected by coverage::no_ui_only_entries). 103ee7cd MidpointOf round-trip test.
- 10-27 07:00: sketch trim pieces Coincident curve to curve (live); exact-flip moving part by the common Z's sign.
- 10-27 06:45: sketch tools as live's REST spec (behaviours "spec: skasm sketch tools"): center point rectangle =
  construction centre point, `ConstraintKind::MidpointOf` of both diagonals (no diagonal line); 3 point circle's 3
  construction points; ellipse Horizontal / Vertical from the major-axis click; corner rectangle Horizontal /
  Perpendicular by click order; midpoint line from the clicked end. Slot, Dimension-tool and BOM specs already matched.
- 10-27 06:xx: assembly sweep, configured-assembly, exploded / named positions and parity/assembly G5, LA43, C25, A6,
  LA14 all rechecked in the GUI; NP2 header 3 px; empty relation dialog's title error; Select default plane popup.
- 10-27 06:00: assembly Section view "Select default plane" popup (Top / Front / Right, Flip plane, ✓ ✕), as
  the Part Studio's; f08016a2 a pending pattern row's badge repeats the dialog title's error.
- cdde1c06 F19 Width mate in any pick order + DOF cache across evaluations, per-group DOF length scale.
- c76cdcf4 assembly least change measured at the bodies' mate connectors (oracle_asm R1 matches live; R1 out of KNOWN;
  R3/C1 now keep rz 0, still KNOWN for live's −157.5°).
- 2d4c73d2 first insert into an empty assembly fitted beside the 300 px Insert dialog (qa5 A1).
- 7dc7aee4 assembly Section view: a clicked face fills Section planes (an instance press had armed a part drag, so the
  click became a selection; `update_capture` now treats the open dialog as busy).
- eb076e34, d1cf1ff7: tests for the secondary-axis and Replicate-nothing-matches texts.
- 8016ded6 committed polygon's grey-boxed Ø readout (live colours) until the next polygon.
- e5567eca SKETCH-DRAG-BIG in the app: the DOF analysis once before a drag (Sketch::refresh_analysis): 42–125 → 13–18 ms
  per step on qa's 1000-entity sketch (dev build, load ~40).
- ac0645bc SKETCH-PASTE: the Transform triad on the copy.
- dcd3cd30 oracle_asm R3 / C1 as live: exactly opposed axial connectors turn over about world Z × w (live's constant);
  KNOWN now holds only M1 (live's own over-constrained case).
- c95ed870 closing Animate mate gives the keyboard back (Alt+C search got no typing).
- a239abb0, 782944e5 sketch polygon side count per 10 px, click commits without a next polygon, a tool session starts
  at 6 sides; circle / polygon Ø readout halfway or outside as live.
- b154b333, 507ce0e5, 57cab72a: test of the assembly error texts against live's (assembly sweep rows checked).
- 4ca39911 assembly fuzz test (T2).
- 3bcb5145 SKETCH-DRAG-BIG: sketch regions' hole search by bounding box (regions 38 → 3.5 ms, regen 47 → 12 ms per
  drag step on 1000 entities; perf test scene sketch_perf::big_shape_sketch_drag_step_parts).
- 6329b168 sketch: polygons as live (seed + circular pattern; Inscribed = edge middle, Circumscribed = vertex; no Ø
  dimension), Point tool on the Origin, paste at the pointer + Transform (SKETCH-PASTE), unsolvable constraint keeps
  the shape; API polygon descriptions.

## Speed targets (PLAN 2.7), current numbers
Measured 10-27 14:00 in the dev test profile (opt-level 1; no release build: disk under the 30 GB floor), load ~40;
release is faster. Each is a timed T2/perf test with the target as its gate.
- Sketch drag step, ~200 entities (219 entities, 178 constraints; pointer move + solve + the sketch's regeneration):
  **median 5.19 ms** (solve 1.48 ms), worst 17 ms. Target < 8 ms. Full solve with the DOF analysis: **1.15 ms**
  (target < 8 ms). `cargo test -p noshape-scene --test all sketch_perf::speed_target -- --ignored --nocapture`.
- Assembly drag, 200 instances / 300 mates: frame with the scene **median 5.40 ms** in plane, **6.18 ms** off plane
  (worst 23–26 ms); target 120 fps = 8.33 ms. Insert into the 300-mate assembly **11.9 ms** (target ≤ 20 ms). Adding
  a mate 40 ms. 1000 free pins: drag frame 1.49 ms, insert 3.9 ms. `cargo test -p noshape-ops --test all
  assembly_perf:: -- --ignored --nocapture --test-threads=1`.
- Re-run 14:40 (load ~30): assembly drag frame median 1.48–2.93 ms, worst ≤ 3.05 ms; insert 5.7–7.7 ms; 1000 pins
  drag 1.4–1.5 ms. Sketch step per-step times (3 runs): worst 5–9 ms, sporadic (no step pattern): the earlier
  18–26 ms worst cases were machine load, not a code path.
- Next for speed: the sketch step's regeneration (3.7 of 5.2 ms in the dev build).

## WIP on agent/skasm
(none: the branch equals main. Land only with tools/tiers/land.sh, run detached: /root/skasm-scratch/run.sh landsh.log
"tools/tiers/land.sh".)

## Next, in order
Lead 10-27 order: P1 tickets (none open) → PLAN 2.7 speed targets (met, above) → assembly.md's 7 🔧 rows (T11 is the
theme owner's; I15 / LA50–LA55 are standard content) → mates/assembly workflow gaps (all ✅; UNCONFIRMED details asked
live, REQUESTS "assembly sweep leftovers") → standard content. 10-27 15:00: ISO 10511 / 7040 on live's template and
tables (fbfe810c). The other missing components (DIN 7050 / 7051 / 967 / 923, ISO 7379, NASM 45938 + /4, NAS 1129 ×2,
PEM ×52) need live's /features (the recipe; one size's surfaces don't give the formulas), 429 until ~10-28 03:00.
Sketch P1: value boxes done for every tool live measured (center rectangle and tangent arc not measured: default
place). parity/sketch.md all ✅. Assembly P2:
1. docs/parity/workflow-assembly-sweep.md: all rows ✅ 10-27 (34 left UNCONFIRMED: the 2° snap window). Filed
   PS-SECTION-RED for psui (Part Studio caps red where parts overlap; live only in the assembly). GUI docs:
   /root/skasm-scratch/qa/asm1.noshape (plate + 2 pins), asm2 (a pin through the plate), ps2 (Part Studio overlap).
2. docs/parity/assembly.md G5 exploded views: workflow-exploded-named-positions rechecked 10-27 (EXV6 rollback pill,
   NP2 header measured against live 155); qa/asm3.noshape = asm1 + Revolute + a named position driving it.
3. Old passes rechecked 10-27: workflow-mates (8, 10, 12, 13 ✅), workflow-assembly (A1–A6, A9, A11, A12 ✅; A10 fixed
   earlier, rows 109 / 170), workflow-configured-assembly (1–5, 8
   ✅; CA3 closed by ui, CA4 and A15 rechecked on green).
4. Which part moves when a new mate joins two free parts: done (be0dd33e, 26c7a498: the later part of one studio
   moves). Mate edits match live's two UI runs. Open (asked live, REQUESTS "a NEW Revolute made in the UI mate
   dialog"): a dialog-made flipped mate turns the pin about its connector X with rz 0 in ours (new_mate_holds), live's
   REST-made one turned it about Z × w (rz −157.48°); which does live's dialog do?
5. Slot: rechecked 10-27 against live spec-slot-06 (Ø20 leader, glyph groups, colours as live); the once-seen
   highlighted centre line was not reproduced.
6. Assembly drag bench (psui's, viewport.rs) gives up on llvmpipe ("nothing under the viewport centre": the hover pick
   isn't back within 300 ticks); fine on CI runners presumably.
7. Standard content (tests say P3 long tail): PEM® data crawled by live (52 components: FSRules, configuration,
   bodydetails at each Part Studio's default; /features still 429 until ~10-28 03:00) in dom/skb-sc-fsrules/PEM_*.
   Of the 11 others: ISO 10512 and DIN 6908 done; left DIN 7050/7051/967 (tapping, cross recess), DIN 923 (needs a
   head-shoulder-neck form: live's configuration lists the inputs), ISO 7379 (shoulder screw with neck widths /
   fillets), NASM 45938 + /4, NAS 1129 ×2. Live's volumes for each studio default are in behaviours "skasm
   (2026-10-27) standard content gaps"; asked for ISO 10511 / 7040 B-reps (ours come from the published tables). 11 non-PEM
   components are missing too (/root/skasm-scratch/sc/compare.py lists them: DIN 7050/7051/967 tapping, DIN 923, 6908,
   ISO 7379, 10512, NASM 45938 + /4, NAS 1129 ×2); their tables exist for some (dom/skb-sc-fsrules, skb-sc-comp) but
   skb's generator (/root/skb-scratch/gen) maps other column names; asked live for each default size's volume / faces
   to check the geometry against.
8. Sketch solve time in broken sketches: the fuzz (random constraints, 200–600 entities) shows failing solves
   taking ~95 ms (conflicts) and 230–940 ms (machine load) when not converging (profiled: 9 attempts of 3–40 ms,
   the DOF analysis 1–4 ms): `solve` retries up to 8 times with conflicts
   excluded (sketch/src/solver.rs ~922), each a whole-sketch solve. Real edits revert a conflicting constraint, so
   only a sketch opened already broken pays it; candidate fix: retry only the failing component.
8b. Assembly fuzz (T2) `random_assembly_edits_never_panic`: `ASM_FUZZ_SEEDS=1..200 ASM_FUZZ_STEPS=120 cargo test -p
   noshape-assembly --lib assembly::tests::random_assembly_edits_never_panic -- --ignored --nocapture` (clean 10-27, also
   after the exact-flip rule: 200 seeds, 418 s).
9. End-of-day walk, daily (~04:00 UTC): see below.

Walks: 10-27 02:40 on green 19555ca3: line chain + dimension, polygon, V on H toast, mate dialog (types, limits,
errors), Revolute + drag (only the dragged instance moves): no regressions. 04:00 on green 8165fca6: paste (copy at the
pointer, selected, Transform triad on it). Sketch options (Show constraints / expressions / errors) persist in
~/.config/noshape/sketch-options.json, so "Show expressions" can start ticked on this box; not a bug.

Assembly perf (10-27, dev build, `cargo test -p noshape-ops --test all assembly_perf:: -- --ignored --nocapture`):
200 pins / 300 mates: evaluation again 6.5 ms, adding a mate 16.5 ms, inserting a pin 7.8 ms (was ~50), drag steps
1.9–2.8 ms (frame with the scene 2.2–3.1 ms); 1000 free pins: drag 0.55 ms, insert 3.8 ms. All inside the budgets.

## Build times (10-26, load 30–55, 4 agents building)
- First build of app + cli on this target dir: 60 min. First test build of ops tests: 20 min.
- After rebasing on c9540ae6 (workspace-hack, tests/all.rs): `cargo test -p noshape-ops --test all mate_flip::`
  recompiled everything: 955 s.
- 10-27, noshape-assembly crate: edit → `cargo test -p noshape-assembly --lib assembly::tests::<name>` 23–32 s.
- 10-27, layered crates, load ~30: first build after the rebase 254 s (model-core lib tests). Edit in
  model-core/assembly/solver.rs → `cargo test -p noshape-model-core --lib assembly::solver` 108–122 s; →
  `cargo test -p noshape-model --test all oracle_asm::` 113 s. Sketch-crate edit → `cargo test -p noshape-sketch`
  35 s; scene edit → `cargo test -p noshape-scene --lib` 92 s (first after a sketch change). Clippy of model-core +
  model after an edit 53 s. T0 after a sketch/scene edit 830 s total chain (incl. the tests above).

## Requests to the lead
(none)
