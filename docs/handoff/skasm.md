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
  00956cb2 DIN 6908 (the previous session's). 2d9a275c speed-target tests; fbfe810c ISO 10511 / 7040 = live;
  0537ac91 OTF-FIRST-KEY; c8d26735 UI keystroke tests on GPUI's test platform (noshape-ui-test-support,
  Gpu::headless, docs/TESTS.md "UI keystroke tests"; first tests: OTF-FIRST-KEY's keystrokes). ee23767c drags beside
  conflicting constraints (UI test a_point_drags_beside_conflicting_dimensions fails without it).
  189663bf DIM-AC-ENTER + EXPR-PARSE-JUNK (Enter commits the dimension box with the unit list open; UI test);
  05dab13c ASM-DIR-NO-HOVER (linear pattern Direction picks the edge, highlighted on hover); dad69aa3 UI test of
  the drag beside conflicting constraints.
  dbc15478 value boxes of center rectangle / tangent arc (live 1503); 4d29be6a Derived mirror in XY (live's exact
  transform); 03915b0f ring snap as live's (5° / 1° / 0.1° by distance). Open (P3): ANSI taper pin crowned ends (behaviours
  "… ANSI Commercial class taper pin B-rep (request 1504)": spheres R = d + 0.010 in through the d/2 circle; the
  cone's end offset needs live's /features, 429 until ~22:45 UTC: dom/skasm-sc-gaps/ANSI_taper_pin__features.json).
  0b0f1abd sketch regions with long splines 36× faster (qa's fuzz sketch 1.46 s → 40 ms; fixture
  tests/fixtures/solve/fz211-final.json). qa's skfuzz 211 on the dev CLI: slowest step 0.74 s (qa: 290 s).
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
  **median 3.52 ms** (solve 1.03 ms; 10-27 after CONFLICT-COLLAPSE-ON-DRAG: a drag solves only the dragged
  components), worst 14 ms. Target < 8 ms. Full solve with the DOF analysis: **0.96 ms**
  (target < 8 ms). `cargo test -p noshape-scene --test all sketch_perf::speed_target -- --ignored --nocapture`.
- Assembly drag, 200 instances / 300 mates: frame with the scene **median 5.40 ms** in plane, **6.18 ms** off plane
  (worst 23–26 ms); target 120 fps = 8.33 ms. Insert into the 300-mate assembly **11.9 ms** (target ≤ 20 ms). Adding
  a mate 40 ms. 1000 free pins: drag frame 1.49 ms, insert 3.9 ms. `cargo test -p noshape-ops --test all
  assembly_perf:: -- --ignored --nocapture --test-threads=1`.
- Re-run 14:40 (load ~30): assembly drag frame median 1.48–2.93 ms, worst ≤ 3.05 ms; insert 5.7–7.7 ms; 1000 pins
  drag 1.4–1.5 ms. Sketch step per-step times (3 runs): worst 5–9 ms, sporadic (no step pattern): the earlier
  18–26 ms worst cases were machine load, not a code path.
- Live (behaviours "speed: skasm sketch and assembly", request 1501; server-solved, one request in flight): sketch
  drag ≈ 400 ms a step at 188 entities (≈ 310 ms at 20) → ours ≈ 4–5 ms (dev build), ~80× faster; adding a
  dimension ≈ 2.2 s and a Coincident ≈ 1.7 s of server solve → ours ≈ 1 ms; assembly drag ≈ 1.2 s a step at 200
  instances → ours 1.5–6 ms; insert 1.3–2.2 s to the assembly data → ours 6–12 ms.
- Next for speed: the sketch step's regeneration (3.7 of 5.2 ms in the dev build; JSON serialization in the regen
  path is the largest single item: core's code).

## WIP on agent/skasm
(none: the branch equals main. Land only with tools/tiers/land.sh, run detached: /root/skasm-scratch/run.sh landsh.log
"tools/tiers/land.sh".)

## Next, in order
Lead 10-27 order: P0/P1 tickets → PLAN 2.7 speed (met; numbers above, live's beside them) → assembly.md 🔧 rows
(standard content) → mates/assembly workflow gaps (all ✅, UNCONFIRMED all answered 10-27) → standard content.
1. Open tickets: none in my section (10-28 04:30; also fixed 10-28: SKETCH-FILLET-STUCK-RADIUS twice more, ASM-TOOLSEARCH,
   ASM-CHANGE-CONFIG-ICON, ASM-FILTER-PATTERN-INSTANCES with the filter's help popover). 10-27 late: Fixed this round: dc9db0a4 CONFLICT-COLLAPSE-ON-DRAG (a drag
   solves only the dragged components; the others stay as they are, also faster), 1ca0cf14 ASM-RENAME-ESC +
   J-KEY-NO-EFFECT, 07ead631 MATE-ANGLE-360, 5bc2e08e ORIGIN-COINCIDENT-GLYPH (a point on the Origin is coincident
   with it, no longer merged into it: circles centred on the Origin now carry a Coincident too, as live),
   bf1edd44 SKETCH-PATTERN-GLYPH-HOVER. e52e8015 point menus as live; UI keystroke tests 9.
2. (Tried: writing back solved components between conflict-exclusion retries: no gain measured, dropped.)
3. UI keystroke tests (docs/TESTS.md, part-studio-ui sketch_ui_tests.rs, 13 tests with psui's). Added 10-28: Tab
   between rectangle boxes (it dropped the typed width; fixed), `=#len` in a Line box (sketch_in builds on a studio
   prepared first). The value box's keys are bound per test App (entry_key_bindings). The harness
   is part-studio-ui only; assembly-ui could opt in the same way (dev-dependency on noshape-ui-test-support + gpui).
4. Standard content (P3): ANSI taper pin crowned ends done 10-27 (live's rule from its B-rep alone: the seams l
   apart, the large one at d/2, crown spheres symmetric; = live's volume / areas / bbox); DIN 7050 / 7051 /
   967 / 923, ISO 7379, NASM 45938 + /4, NAS 1129 ×2, PEM ×52 need live's /features (~10-28 03:00); live's studio
   default geometry for each is in behaviours "skasm (2026-10-27) standard content gaps" (faces, volumes) to test
   against.
5. Broken-sketch solve time: a constraint on qa's 438-entity fuzz sketch ~85–125 ms in the sketch crate (9
   exclusion attempts of ~15 ms, the failing component's LM each time). Candidate: retry only the failing component.
6. Assembly fuzz (T2) `random_assembly_edits_never_panic`: `ASM_FUZZ_SEEDS=1..200 ASM_FUZZ_STEPS=120 cargo test -p
   noshape-assembly --lib assembly::tests::random_assembly_edits_never_panic -- --ignored --nocapture`.
7. End-of-day walk (~04:00 UTC) on the newest /root/green/noshape against live: line chain + dimension, value boxes
   (line, rectangle, circle, center rectangle, tangent arc), polygon, mate dialog, Revolute + drag, linear pattern
   direction pick.

Walks: 10-28 04:10 on green 1b81b7ca: line chain 40 / 25 typed, rectangle 30 Tab 20, asm5 Revolute pin dragged
(stays), j shows the hidden Revolute: no regressions. Seen: a line's dimension text ("40") and a rectangle's height
("20") sit under the constraint glyph rows (glyphs 20.5 px off the line, the dimension ~28 px); live's spacing there not
recorded. 10-27 02:40 on green 19555ca3: line chain + dimension, polygon, V on H toast, mate dialog (types, limits,
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
- disk-loop (/tmp/lead/disk-loop.sh, every 30 min): its prune steps (prune-deps / prune-exes / incremental) run on
  target dirs mid-build; three of my lands failed 10-28 00:40–01:50 on .rmeta / fingerprint files deleted under a
  running build ("extern location for noshape_units does not exist: …/libnoshape_units-d2f9….rmeta", "failed to write
  …/invoked.timestamp"). Could they skip a dir whose cargo is running, as `floor` does with `busy`? 02:34 again,
  right after the 02:32 run ("prune-exes: removed 42 old builds"): prune-exes.py deletes an old build's fingerprint
  dir while cargo is rebuilding that very hash (release-ui, part-studio-ui).
  Fuzz 10-28 02:00–02:40: assembly 120 seeds × 120 steps clean; sketch wide 149 seeds × 150 steps clean.
- "Show calculation variance" in Mass properties (live: behaviours "skasm (2026-10-27) calculation variance": every
  value gets " ± 4.000e-1 mm³"-style text, 3-decimal scientific, not persisted). The assembly panel
  (assembly-ui/assembly_mass.rs) is mine, the Part Studio one psui's (part-studio-ui menus.rs ~1329), and the numbers
  need an error bound from the kernel's mass-property integration (core: e.g. BRepGProp::VolumeProperties with Eps
  returns the achieved error). Who owns it? I can do the assembly panel's text once the kernel gives a bound.
