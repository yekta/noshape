# Handoff: skasm (sketch + assembly), from 2026-10-26

## Resume here (2026-11-03 01:30 UTC)
Landed overnight (all on main by dabe8513): sketch drag far hold (g6) and levels + farthest point (g4); drag from the
drag's start (live drag probes 12: the result depends only on where the drag ends) in dragged components of <= 60
curves (`Sketch::begin_drag` / `restore_geometry`, editor `drag_origin`; bigger ones step from the last, for speed);
repeated pointer positions not re-solved (sketch + assembly); constructive: s11 (Equal partner left out of a group,
touching tie-break, end-carried resized line keeps its direction), transitive aliasing (s485 / s496), chamfer slide
(s465), per-component fallback (s464 / s475); T2 readers (asm spec probes, probes 11 robust); fixtures batch 8 +
probes 11. Queued: SKETCH-UNDO-REDO-STATE (P2; sketch toolbar Undo / Redo follow the sketch's history; ticket [x]).
vibe3 scene sketch_perf, main 689c6070 -> dabe8513: S300 mixed point drag 7.7 -> 6.0 ms, the rest equal (179-curve
corner 1.11, drag frame 2.8, drag step 2.04 ms). GUI (my build): bolt-circle drag keeps c3, c4 radial from its start
whatever the path; sketch Undo / Redo greyed on entering, Undo on after a drag.
Numbers: drag probes 7 8/8, 9 6/6; edit 6/6; probes 1-6 all; batch 8 40/40; batches 1-8 1194 mm (k-sum).
Open (P2, need live data, nothing to guess): s6, s102 (probes 10), s218 / s236 / s226 (probes 11: s226's spline
loop moves rigidly in live; s236's third quad down in live, up in ours; s218's chain turns only through k19), s209 / s11
(probes 13 sent), s230 (tried keeping the Length side's Equal partner: worse elsewhere), s210 (an Angle with an H line
keeping the H member made s210 worse). Asm: f50 f53 f54 f55 f64; asm probes 4 and slow d6 (asm-drag-3) with live.
Live's single-jump rule (one drag event after the threshold moves nothing) not done: the agent API's drag op would
need an extra step first (P3).
Lead's list (20:15), in order:
1. Done: dx's X1 (B), other documents' assemblies (27549ea9; note to dx in docs/handoff/dx.md "From skasm (11-02)").
   If dx asks for more (configurations of another document's assembly aren't offered), that comes first.
2. Open sketch-fuzz cases. P1 drag probes g4 / g6: done (above; g6 c4 waits on live's probes 12). P2 (random batches 1–3,
   sums in /root/skasm-scratch/fz/*.k; `fz/runall.sh <tag> [ENV=..]` runs all, `fz/cmp.sh <batch> <seed>` one case,
   `fz/tr.sh <seed|probe>` traces the pass): s6 170 mm, s218 95, s236 81, s102 79, s226 58, s209 43, s11 10.
   Asm known: f50 f53 f54 f55 f64 (f64: live turns C 93.9° where the smallest turn, ours, is 90°: no rule yet).
3. With live (REQUESTS.md, not yet answered at this time): sketch batch 8 (REQUESTS.md line 1815,
   fuzz/skasm-sketch-8.json → .live.json: copy to crates/sketch/tests/fixtures/live_fuzz/sketch-8.live.json and run
   live_fuzz::live_matches), sketch probes 11 (leave-one-out of s218 / s236 / s226 → sketch-probe-11.live.json,
   compared by live_fuzz::probe_matches; per-case errors via fz/p10.sh-style parsing), asm probes 4 (c1–c7, f54's
   redundant coaxial loop reads "overdefines" in live → asm-probe-4.live.jsonl; no T2 reader yet: add one to
   oracle_asm like asm_probes_match_live).
4. End of day (~04:00 UTC): GUI walk of sketch + assembly workflows in /root/green/noshape against live; regressions
   to docs/TICKETS.md.

Area (PLAN 2.3): crates/sketch, crates/scene/src/sketch_mode.rs, sketch behaviour in the app (part_studio/sketch.rs),
crates/app/src/assembly_*.rs + assembly_view.rs, crates/model/src/assembly, standard content, and since 10-29 19:40
(lead) **sheet metal end to end**: model-core's crates/model-core/src/sheetmetal + regen/features/sheetmetal*.rs, the
FS host's custom/host/sheetmetal.rs (moved from core), the dialogs and the table / flat view in the Part Studio UI
(psui's crate: name psui in such commits). Checklist docs/parity/workflow-sheet-metal.md. Older notes:
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
On vibe3 (tools/tiers/perf.sh: release, the machine to itself, 3 runs; 10-30 evening at 94056b26). Each is a timed
T2/perf test with the target as its gate. (The 10-27 dev-profile numbers on this machine are superseded.)
- Sketch drag step, ~200 entities (219 entities, 178 constraints; pointer move + solve + the sketch's regeneration):
  **median 1.37 ms** (solve 0.43 ms), worst 1.64 ms; target < 8 ms. Full solve with the DOF analysis **0.36 ms**
  (worst 0.38). `tools/tiers/perf.sh -p noshape-scene --test all sketch_perf::speed_target -- --ignored --nocapture
  --test-threads=1`.
- Assembly, 200 instances / 300 mates: drag frame with the scene **median 0.75 ms** in plane (worst 0.91), **1.03 ms**
  off plane (worst 1.19); target 8.33 ms (120 fps). First evaluation 10.7 ms, again 2.8 ms; adding a mate 4.3 ms;
  inserting a pin **2.8 ms** (target ≤ 20 ms). 1000 free pins: drag frame 0.38 ms, insert 1.3 ms.
  `tools/tiers/perf.sh -p noshape-ops --test all assembly_perf:: -- --ignored --nocapture --test-threads=1`.
- Live (behaviours "speed: skasm sketch and assembly", request 1501; server-solved, one request in flight): sketch
  drag ≈ 400 ms a step at 188 entities (≈ 310 ms at 20) → ours ≈ 4–5 ms (dev build), ~80× faster; adding a
  dimension ≈ 2.2 s and a Coincident ≈ 1.7 s of server solve → ours ≈ 1 ms; assembly drag ≈ 1.2 s a step at 200
  instances → ours 1.5–6 ms; insert 1.3–2.2 s to the assembly data → ours 6–12 ms.
- Next for speed: the sketch step's regeneration (3.7 of 5.2 ms in the dev build; JSON serialization in the regen
  path is the largest single item: core's code).

## Sheet metal (since 10-29 19:40)
- **Std route is the default** (10-29): the built-in sheet metal features run std's own FeatureScript through the host
  (`model-core/src/regen/std_route.rs`; `NOSHAPE_STD_SHEET_METAL=0` = native; without the FS host registered, a bare
  model-core regen, native too). Tests that use sheet metal call `noshape_model::custom::register()` in their helpers.
- Fixed to get there: std Jog's `ownExistingImprints` split (split.rs `NO_SPLIT`); FIX on an arc = centre + radius
  and DISTANCE `direction` HORIZONTAL / VERTICAL / MINIMUM in the host's skConstraint (std's tear drop hem); form
  up/down by the side the tools reach past the sheet; the std flat holds only while the part is as the sheet metal
  feature built it (`FlatPattern::built_for`, `body_key`), else the native flattener on std's data (a later Hole's
  hole and counterbore heads); per-vertex Bend relief settings (`Definition::reliefs`); pulled wall sides that come
  out reversed are dropped (Modify joint radius on a corner a later flange shortened); ops' table skips stale parts.
- Table letters (10-30): `Context::sm_births`, the std rebuild's first-sight registry of definition edges (end points
  plus the boxes of the faces beside them) and rolled faces (box). An edge is new when its ends move, a face beside it
  is cut down (the Hem's trim of the plate renews the plate's bends) or gains a wall (a Flange); a face that only
  grows (a Tab merged in) keeps it. New ones: renewed-in-place first, then moved / new, each in definition order.
  Bend / Rip `birth`; the panel sorts joints by it (live 1604, 1614; T2 joint_births_follow_live_letters,
  qa5_letters_follow_live).
- Slanted bend ends (10-30, loft rect → triangle, live 1612): build_part's `skew` ends: where both walls stop on free
  sides (or a rip) at different places, the bend runs on and is cut back by a lofted ruled cutter; the definition-side
  arc lies on the plane of the two sides through the vertex, eased linearly onto a rip's pulled-back side. Edge-joint
  rips pull each wall back clearance / (1 − cos) of the walls' in-plane directions (0.02 at 90°, 0.018071 at 96.1°).
- Cuts across bends (10-30, qa4 S1, live 1617): four FS host fixes made std's sheetMetalAwareBoolean work on a flanged
  part: opThicken of a face thickened its whole body too; INTERSECTION ignored keepTools (std trims its tool by each
  thickened definition face in turn); attributes follow every piece of a split edge (`relocated_pieces`) and are
  re-keyed when their entity is renamed under them (else `sm_attribute`'s equality lookup missed the x = 100 bend).
  Walls with curved free sides (the notch arc) are built from the definition face less a strip per pulled side
  (`pulled_face`, `run_inside` for the pulled sides' exact ends), unified; a free side in line with a rip is pulled
  with it (the wall beside a Hem's end).
- Flat unfold (10-30): `place_from` chose a bend's turn by `dot > 0`, which both turns pass under 45°: the loft's
  33.8° bend stood its walls up out of the flat (area 2.6 % high). Now the nearer turn (T2 a_shallow_flange_unfolds_flat).
- Form library (10-30, live 1616, dom/sm-forms-v4/): `sheetmetal::forms` (model-core): each form a built-in Part Studio
  `library:<id>` (inputs named as live's variables so std's Form fills `thickness`; one hidden `smFormTool` feature per
  tool, then std `tag` FORM; two tools from one feature lost the negative one's attribute in std's instantiation), the
  regeneration's derived provider wrapped (`WithLibrary`). UI: the Select Part Studio panel's Libraries tab
  (select_item.rs `library_sources`, `input_label`). Done: Circle emboss, Circle extrusion, Countersink,
  Slot extrusion, Louver, Bridge lance, Lance (exact at live's measured configurations: NB the crawl's part figures are at each studio's
  currentConfiguration, not its defaults), Slot emboss (no live figures). Circle knockout too: all 9. Pictures: each
  form's Add material. Lance Straight, Slot emboss and Lance's placement checked
  against live 1618. Left: the preview. Flat sketches (10-30 evening): the circle and slot forms' studios have a "Form profile"
  (an smFormTool part SKETCH: the Ø d circle, clearance hole or width slot) as the Tag's flatFormSketch; the host's
  qModifiableEntityFilter keeps sketch bodies in a studio loaded for a derive (`CtxState.loaded`; std derive dropped
  them; everywhere else std's sheet metal needs them not modifiable: custom_features sm3 got 3 extra "parts"), opMergeContexts / opPattern copy sketches
  under op-marker ids (qCreatedBy finds them), moved, with their attributes, hidden (`Context.hidden_sketches`);
  `flat_of` draws the footprint plus the sketch's closed wires (live's Circle emboss flat: r 9.1569 + r 2.5).
  All forms but Lance have their sketch (live 1623 DXFs): non-construction = tear, construction = outline; the flat
  shows only the sketch. Louver placed at defaults: 0.15 mm³ over live's /massproperties, its evVolume pending (1623 (c)). Mate connectors still don't come through the derive (std deletes them; merge has no connectors).
  Copy Flat pattern needs an assembly instance source for a flat body (assembly crate, not mine).
- Open (10-30 morning): (1) (was: table letters, done above). (2) evaluateQuery order generally: live returns faces in
  creation order (rebuilt faces last; request 1608); the host lists a body only one Extrude made in live's order
  (first side, start cap, end cap, other sides: query.rs `creation_order`, which fixed the Convert's Butt Direction 1),
  other bodies in body order. (3) rect→triangle loft: done (request 1612; build_part's
  `skew` ends and rip_offset's edge-joint rule). (4) Flat view: Copy Flat pattern (live pastes a flat part
  instance into an assembly) is left. A bend line's Curve/surface analysis… / Shift+C (the bend's straight 3D edge:
  live analyses the flat's own edge, chip "Edge") and Select other… (Edge of <model>, Face of <feature> × 2, live
  1622) are done (130d51d3); the Form picker is live's layout (22f7b473). Bend lines select by a 5 px screen hit test, draw orange, and have Zoom to fit / Zoom to selection / Align view
  with vertical axis; a right-click anywhere while one is selected shows that menu, as live 1605). An open sketch on a
  face of the flat's part shows that face's box (+6 %) and the sketch's name in the flat view (live 1605 g-4). Closed corner: done (live's regen
  volume 8535.1655548, request 1606; its 8535.0418 was /massproperties).
- Cuts on sheet metal (10-30): Extrude / Revolve / Sweep / Loft adding to or cutting an active sheet metal part run by
  std (`std_route::SHEET_METAL_AWARE`), so the cut goes into the definition and lasts (host @opCreateOutline, planar
  faces; the rebuild cuts each wall's inner loops). Simple Holes on active sheet metal cut through std's booleanBodies
  from the native Hole (hole.rs `cut_sheet_metal`); counterbores / countersinks through std's
  registerSheetMetalBooleanTools from an internal FS feature (custom/sm_cut.rs, host hook `sheet_metal_cut`): the
  rebuild cuts the walls' cuttingToolBodyIds and `flat_of` draws the heads.
- Done 10-30: partial Outer flange (opEdgeChange steps a moved piece of an edge off its collinear neighbours and keeps
  the edges' names; the relief reaches its depth past a moved-in bend edge; request 1599 (a): 6027.5617 / 6204.5617
  exact); SM-FACES passes on std (3211.3274 mm³, 14 faces).
- Speed: see "10-30 evening batch" above (sheet metal model ~300 ms, Hem ~181 ms on vibe3 release after the
  keep_names box test and the glued fuse; was 1321 / 1089 ms).

- Std rebuild naming (10-29): part faces lying on the replaced part's faces keep their names (`keep_names`, several
  pieces: splits), the part its name and colour. Before, every face was new: a Flange dialog couldn't pick a second
  edge once the first previewed (PREVIEW-SELF-PICK guard), and the part changed colour.
- Old documents (10-29, P1 found in the GUI pass): studios saved while sheet metal ran natively reference native face
  names, so by std their Flange / Hem lost their edges ("Select edges to flange.", qa's smf.noshape a bare plate).
  ScopeRule 5: on load an older studio with sheet metal gets `PartStudio::legacy_sheet_metal` and keeps the native
  evaluators (`std_route::LegacySheetMetal`, set per regeneration, nested Derived ones restore it). New studios run
  std. Documents saved by main builds 10-29 21:07–23:50 (std default, still ScopeRule 4) load as native now and lose
  their std-named references; only scratch documents were made in that window as far as I know.

## WIP on agent/skasm
(none after the 19:30 land. Land only with tools/tiers/land.sh, run detached: /root/skasm-scratch/run.sh landsh.log
"tools/tiers/land.sh".)

## 10-31 19:30 (resumed on a new account)
- 15109d5b (SM-CLOSED-BUTT flat) passed its land checks but the final rebase hit the uncommitted Group work; it lands
  together with: **Group holds patterns** (live 1681): a pick on a pattern's copy, in the graphics or the list, takes the
  pattern ("Linear pattern 1" chip, no message; the copies drawn selected); ✓ with only a pattern is valid. Stored as
  `Group::patterns` (model-data), members = `Assembly::group_members` (own instances + the patterns' copies) in the
  solve, connectivity and the same-group error; a group goes with its pattern when nothing else is left in it.
  assembly_group takes a copy the same way. LA55's Near connector confirmed by live 1639.
- The never-checked rows the lead listed at 18:50 were already settled by the 10-31 (4) pass (counts from before it).
- Landed df144507 (Group holds patterns, + 15109d5b) and 4e9b054e (forward edges). GUI :103 checked the Group dialog
  as live 1681 (copy click → "Linear pattern 1" chip, copy drawn selected, ✓ with only the pattern, Edit shows it).
  Then: the open Group dialog highlights its instances' and patterns' rows in the list (live 1681: the pattern row);
  the assembly fuzz makes and deletes groups (seeds 1..60 × 120 clean; 400..480 before it clean).
- 8e85b590 landed (group rows, fuzz). T2 `butt_joint_plate_point_follows_gap_radius_and_thickness`: live 1680's (b) gap
  0.1, (c) R 4, (d) T 1 — the flat's plate cut at P = R + (K T + gap)/√2 exactly, volumes within 0.2 of live's Mass
  properties figures (their HIGH evVolumes are still pending at live). Live request 22:20: which list rows a Group
  dialog highlights (ours: its instances and patterns).
- c7fd2571 landed (1680 test). AssemblyView UI tests on GPUI's test platform (assembly_ui_tests.rs: a document made
  through the agent API, the real view, inert `assembly_hooks::register_for_tests`), first test the Group dialog's
  pattern pick (0.5 s). Add assembly dialog flows there as they are touched.
- **11-01 night (lead: knurl, then the unchecked rows):** the knurl's 16 → 138 s was bool's 2afca924 (declined the
  grooves), fixed by bool e9e0f8bc; not the forward edges. Sheets: the rows with an empty Verified cell re-checked and
  stamped (CLI scripts in skasm-scratch/feat: l17.py, adv2.py, adv.py, c2.py, imp; T2 runs; GUI where noted).
  Found: Thicken 4 (L17) now = live; navigation 1.18's zoom-out stop needs live's plane-size rule (REQUESTS 03:00);
  SM-THICKEN-SKETCH-INFO and SM-MAKE-JOINT-ONE-EDGE (TICKETS, live G34 c2). assembly.md: ten rows had their evidence
  one column late (moved into Verified), LA12's `||` split its row.
- 11-01 03:00–04:00: landed b6ba4390 (SM-THICKEN-SKETCH-INFO), 42ce4965 (a dimension's menu starts Configure dimension,
  live's toast). GUI-stamped rows: assembly.md LA8/9/16/25/26/29/30/33/34/38/40/41/46/48/50, I13, K10, K28; advanced
  ADV3 / TAB1 rows; sketch 4.33 / 12.9. **Still unverified**: assembly.md K9 / K11 (snap-mode q / Ctrl: in snap mode
  the moving instance doesn't preview at the target here, so q's turn can't be seen; needs a careful run), I14 (snap
  insert with explicit connectors), workflow-assembly SV2 (the base-handle press missed and box-selected: retry),
  navigation 1.18 (live request 03:00, plane size). RUST-SM-CUT-VOLUME filed for bool (qa4_s1 T2 7 mm³ over live on
  the Rust Boolean; passes with NOSHAPE_BOOL_RUST=0).
- 11-01 ~05:00, live answers folded in: 1703 (a dimension's Configure dimension ▸ submenu with several inputs, "angle"
  in the toast, a configured dimension has only Unconfigure dimension with "… removed from <input>"), 1694 (groups have
  Hide / Show like mates, new ones hidden; shown = black row "Group 1"; the edited group's row selected, name bold;
  a new group's pending row bold grey), 1695 (the sheet metal table over both parts = the a5ab5f91 fix). Navigation
  1.18 answered (1701): the planes don't grow, the zoom bounds grow by Mate connector 1's constant-pixel glyph
  (disc 18 px, triad +33 px x / y, +23 px z) — next.
- 11-01 ~07:20: landed d4e4a52f (navigation 1.18: connector glyph extent in the zoom bounds), e85234b8 (snap
  insert click places with a Fastened mate, live 1704). **Builds now run on vibe4 via tools/tiers/rb.sh** (lead):
  `export CARGO_TARGET_DIR=/root/target-skasm` before `rb.sh --get noshape cargo build -p noshape` (else the binary
  goes to the worktree's target/), and the vibe4 binary reads icons from /home/ngyekta/agents/skasm/src/crates/app/assets
  (copied there). Open next: SNAP-INSERT-FACE leftovers (face-centroid target; Part Studio circle-edge connector frame
  vs live, request 07:15), assembly.md K9 / K11 (snap-mode q / Ctrl), SM-MAKE-JOINT-ONE-EDGE, RUST-SM-CUT-VOLUME (bool).
- 11-01 ~08:00: b88b7699 (a snap drag's Ctrl cycles only explicit connectors, live 570; K9 / K11 checked). **The nine
  sheets the lead listed have no unverified row left** (feature-sweep, navigation, sheet-metal, import-edit, assembly,
  sketch, workflow-assembly, advanced, sheet-metal-joints). Open: SNAP-INSERT-FACE leftovers (live 07:15),
  SM-MAKE-JOINT-ONE-EDGE (P3), RUST-SM-CUT-VOLUME (bool).
- 11-01 ~08:40: d318c8dc (Part Studio mate connector at a circular edge's CENTER: Z out through the bounded planar face,
  X the sketch x, live 1708). Still open: SNAP-INSERT-FACE's face-centroid snap target (live 1704: at A the triad sat on
  the face centroid; ours takes the nearest inference point); SM-MAKE-JOINT-ONE-EDGE (the two coincident edges have
  different ids, so the double mapping is inside std's getSMDefinitionEntities over our attributes: probe it with a
  Feature Studio feature printing evaluateQuery sizes); RUST-SM-CUT-VOLUME still fails on main (bool's).
- **End-of-day walk 11-01 03:40, green b6ba4390:** line chain 40 / 25 typed; rectangle boxes (Tab on a typed value
  stays, as live); Revolute via tool search, both picks → HUD ✓ commits and starts Revolute 2 (= live), the pattern
  copy follows its seed; dragging the mated pin keeps it seated. No regressions.
- **SM-TABLE-PART-ORDER (psui's report, 11-01):** e2e `sheet_metal_table_ops` ended in a different table in ~half its
  runs. Once Convert to rip leaves Bend B's wall a part of its own, the model has two parts each with its own
  SheetMetalData, and the ops table (and the app's panel, psui's crate) took the model's "first" part from
  `context.sheet_metal`, a BTreeMap keyed by BodyId, whose ids carry random feature ids: half the runs got Part 2's
  data (only the rip, lettered "Joint A", no Bend A). Now `sheetmetal::model_joints` (model-core) merges a model's parts
  in part-list order (walls / bends offset, a rip between two parts once; `bend_of` maps rows back to each part's flat)
  and `models_of` lists models in part order; ops and the panel use them. The test now checks the whole table after
  every step (old code: 6/10 runs fail; new: 20/20 pass). Live asked whether its table lists both parts' joints
  (REQUESTS 11-01 01:40); the letters already run over the whole model.
- **Forward-oriented edges landed with core's patch** (kernel common.cpp `handed_out`: every edge FORWARD). What sheet
  metal read from the stored orientation, now explicit: (1) `wall_loop` chained a wall's outer loop from its edges' start
  / end, so it ran the face's way only while edges came stored that way: now turned counter-clockwise about the face
  normal (the plate's flat face, so the counterbore head's sections, sat on the wrong side); (2) the host's
  evEdgeTangentLines with a `face`: parameter 0 is the end the edge starts from running round that face (Parasolid's
  coedge; std's getOrderedEdgeVertices pairs vertices with it), decided once at mid-edge; before, only the tangent
  flipped (Flange 2 on the adjacent edge collided: its wall ran past the first flange); (3) flat bend lines run as
  their edge runs round wall a (live's DXF), not the edge's stored way; (4) a form's up / down from its added tools'
  reach (the cut + added reaches tie on a symmetric form, so rounding decided). T1 features_c / e2e sheet metal and all
  36 T2 sheetmetal_std pass with the patch and without it.

## 10-31 evening (after the never-checked rows)
- **Landed:** e7562f23 SKETCH-DEGENERATE-SOLVE (a fillet R6 → R20 that shrinks the next line to a point solves as live:
  Parallel / Perpendicular / point-on-line normalise by max(length, 1 µm); dimension edits may collapse a line,
  `SolveOptions::allow_collapse`, constraint edits keep CON-HV-COLLAPSE's refusal). 69ee2fc6 live 1668: thin loft of
  closed planar profiles = offsets lofted (outer less inner), Delete face whose Heal leaves the faces fails, Offset
  surface past a cylinder's axis fails, crossing Boundary surface profiles fail. 80ed96d6 live 1667 / 1671 / 1672:
  one-sided Body draft at a mid plane (only the pull side, a ledge), Show mates mode as live (no banner / pin, Esc stays,
  click = selection), Insert footer hint, named position toasts close by themselves, a new position is current.
- **Fuzz:** sketch seeds 400..480 × 150 steps clean (673 s); assembly seeds 300..400 × 120 clean (424 s); sketch 1..40
  after the solver change clean.
- **Open:** SM-CLOSED-BUTT (P3): live's flat rule is known (one cubic Bézier per bend end, handles 0.5 / 0.1 × chord, see the ticket), the plate point P isn't (a 3D clearance rule gives ~2.63 vs live 2.650538); a WIP cutter is parked in skasm-scratch/sm-closed-butt-wip.patch; live asked for P at other clearances / radii / thicknesses (REQUESTS 17:00). Thin loft of open / closed planar profiles = live 1668 / 1677 (69ee2fc6, c30d87d8). THIN-LOFT-TRIM-ENDS (P2, core):
  tapered closed lofts 0.15–0.3 % over live (live offsets the sheet along its normal), Trim ends ☐ ends; open profiles
  still use k::thicken. MODAL-HELP-MARK (psui).

## Lead 10-31 (4): never-checked rows (resumed 10:50 on a new account)
- **Landed:** aaa85f6a, 2ce42c21, ba8472f0 (and this handoff).
  - Move face through the far side of a part now fails with live's texts (Offset left an empty 0 mm³ part, Translate an
    inverted 1333 mm³ one, both "Ok"). core's direct.rs, T features_a::move_face_through_the_part_fails.
  - Body draft parted at the top face with the pull up: live's info "The draft has no effect for the given angle.", part
    unchanged (live 1664 (c)); every other case of live's matrix = ours (CLI skasm-scratch/feat/bdraft.py).
  - Flat pattern Export as DXF/DWG: no notice after Export (live 1665 (b)); the checkboxes come back as last exported.
  - Assembly: Alt+C popover and menus over the right strip; the popover footer's "?".
  - **Mate dialog regression (from psui's auto-advance 9f288920 / 720ef464, 10-31 morning):** the full last field keeps
    the focus (live R5), so `awaiting_pick()` stays true; the HUD, the default flip on the 2nd pick, the mate indicator
    and the X / Y / Z letters had all stopped. Now keyed on `MateDialog::connectors_complete()`. Anything else that
    reads "no field takes picks" as "dialog complete" may have the same problem (Part Studio dialogs are psui's).
- **Sheets:** sheet-metal-joints row 5 (T2 butt_joint_after_a_closed_corner_matches_live; SM-CLOSED-BUTT P3, 1.92 mm³
  short: live re-closes both bends against the through wall; REQUESTS 11:40 asks for the end curves). feature-sweep
  failure rows run by CLI (errsweep4/5.py): Pattern Reapply and Enclose texts = live; Draft / Delete / Offset surface /
  Replace / thin Loft / Boundary surface edge cases asked of live (REQUESTS 11:10 (a)–(k)): ours returns Ok for an 80°
  inward draft (a 5-face roof), a cylinder offset inwards past its axis (r 1), a thin loft of 4 × 4 squares (226.67 mm³
  at 1 mm: suspicious), crossing / coincident boundary profiles. assembly.md LA10, LA22, LA23, LA28, LA31, LA35–37,
  LA45, LA47 checked in the GUI. p2-look AL7 fixed (above). navigation 3.7 was already verified.
- **Open live requests (mine):** 10:05 (show mates mode), 11:10 (direct edits / loft / boundary), 11:40 (SM-CLOSED-BUTT
  curves), 12:00 (named position toast), 12:20 (one-sided body draft at a mid plane).

## Lead 10-31 (3): the view cube "Bottom"; the unowned sheets (import-edit, bracket, sheet-metal-joints, undo, document, derived, units, account)
- **n twice / view cube:** the GUI already matched live: Bottom drawn mirrored, X to the left, labels mirrored (GUI :103, ui/nn12.png). Only the CLI's `standard_view` field was null. It now names the view cube face looked at square on (`bottom`), T e2e view_camera_menus.
- **workflow-import-edit:** I1 = live within its bounds: SAT 7582.989, X_T 7583.001, STEP 7582.978 vs 7582.912 ± 4.22 (main CLI). Same-name imports "g25-b (1)", "(2)" = live. I4 / I6 / TAB1 / API1 / API2 point at their fixes.
- **workflow-bracket:**
  - ADV9: covered by T2 bracket_perf (every feature Ok, 67191.3275 / 20 faces, 73262.555 after the edit).
  - REG1: live's dialog. RD1: no part volume in the readout, and the measure cache is cleared on every geometry change.
  - PERF2: faster than live on vibe3 (perf.sh, release): open 26.5 ms, depth edit 27.4 ms (Onshape 77 / 70).
- **workflow-sheet-metal-joints:** SML1, SM4, RIP1, BEND1 / 2, JOG1 and the dialogs are fixed by later passes. The tab volume is exact by construction.
- **workflow-undo:** UNDO1 is fixed. **UNDO3** (live: a drawing note is two undo steps, the first invisible) stays with dx (drawing editor code).
- **workflow-document:** DOC1 IGES and I5 Parasolid headers re-checked by CLI. The Parasolid header has live's schema and names noshape as the application. "B1" prefill = live.
- **workflow-derived:**
  - Rows 6 / 9 / 11 = live ("qa5 derived" (2), (3), (6)); the row menu's only difference is the cloud-only Add comment.
  - Plane readout = live (none). DER4 / 13 / 14, CRASH1 fixed.
  - Live request "skasm (2026-10-31 09:30)": (a) context icon on another part of the same studio, (b) Ctrl+C on a part.
- **workflow-units:** an edited value shows converted, and as typed while focused, = live. Live's Pound units come from its imperial account Preferences, the same mechanism as ours.
- **account.md:** every ❌ / ⏳ row is built now (preferences.md is the current sheet). Only Labs (cloud) is left.
- **Live's derived answer (request 1648):**
  - (a) Every other instance of the context's studio gets the dashed ⇦; ours now too (assembly_props.rs).
  - (b) A part's Ctrl+V does nothing in a Part Studio and inserts it in an Assembly. The DOC3 paste → Derived is removed, and the Copy toast too. psui told.
- **psui's SM4i / SM4k (lead 10-31), model side done; psui told (psui.md):**
  - SM4i: std (the default) Modify joint restyles a rip and turns it into a bend (T2 `modify_joint_restyles_a_rip_and_turns_it_into_a_bend`). The native "already a rip" path is legacy only and was left as is.
  - SM4k: the flat pattern's shape is split along both sides of every bend zone (`split_bend_zones`, flat.rs). The outline loops are untouched. GUI: the zone edges draw.

## Lead 10-31 (2): sheet metal / advanced / booleans / surfaces / p1-pass / analysis / tabs re-checked
- **workflow-sheet-metal:** every row re-checked on main.
  - T2 sheetmetal_std passes 34/34.
  - GUI: SM5 "Up", SM9 2nd Esc cancels, SM11 no tooltip over an open list.
  - Rows carry the status table's evidence.
- **workflow-advanced-part-studio:** every row ✅.
  - GUI: ADV1 the Plane 1 row, ADV3 sweep / helix tooltips with the prefix, ADV4 countersink rows, TAB1 tab switch.
  - CLI: ADV7 thread faces.
- **workflow-booleans:**
  - qa's boolmatrix matches live except a Remove that only touches a part. Fixed bf3312aa: live's "Selected tools and targets do not intersect." info, and the touched part keeps its edges (REMOVE-NO-HIT-INFO).
  - GUI: DEL1 / DEL2 / MENU1 / BOOL4.
  - Open with bool: RUST-UNION-MERGES-DISJOINT-FACES (boolgrid 15 #42 and 38 #12).
- **workflow-surfaces:** SF4 = live; row 12 = live. Boundary surface with one U and one V profile that don't meet now marks U profiles.
- **workflow-p1-pass:** all ✅ except "n twice", which is asked of live.
  - P1-1 / P1-2 header, P1-18 ⚠ row badge: GUI.
  - P2-D2 drawing not out of date after an unrelated import: CLI.
- **workflow-analysis:** AN1–AN4 and rows 5 / 11 / 13–17 ✅ (GUI / CLI).
  - Draft analysis now opens at the bottom right with Entities to check empty, as live.
  - Curve/surface analysis labels now read as live. Knot points / Details are asked of live.
- **workflow-tabs:** all ✅ except row 3 (rename to an existing name), which is asked of live.
- **Live answers folded in (all requests from this batch are answered):**
  - **Thin loft:** Trim ends fails as live with a two-sided wall or three profiles (solid_a.rs, T engine). Live's volumes are in TICKETS THIN-LOFT-TRIM-ENDS. Ours are 18–21 % short. That is core's thicken, now P2.
  - **Curve/surface analysis:** as live (fresh defaults, Knot points, violet / red U / blue V control grids, Details hover table). Analysis row 18 ✅.
  - **Tabs row 3:** duplicate names are allowed, = live.
  - **n twice:** toggles normal ↔ the plane's back with Y kept up, = live (CLI view_camera ×4). The view cube label on the back side reads no standard view where live says "Bottom": cosmetic, not filed.
- **Stale old pass rows in the seven sheets:** the ✗ / ? cells whose fix was verified in a later row or ticket now carry "→ now ✅" with the reference (SPLIT1, BOOL1b / 3 / 4 / 4b, FILL-MERGE-WITH-ALL, Keep tools auto-on, Flange / Hem labels (behaviours 5831 / 5836), P1-8 / P1-9 (columns were swapped), P2-D1, DOC6, TABS1).
- **Only open ✗ left in these sheets:** booleans 141 / 142 / 144, bool's Rust / OCCT union cases (OCCT-UNION-OFFSET-CYL, RUST-UNION-MERGES-DISJOINT-FACES).
## Own areas, lead 10-31: workflow-assembly / navigation / workflow-sketch-tools
- **workflow-assembly.md: every row ✅.**
  - A9b re-checked by CLI. Edge directions now follow live in the assembly and in the Part Studio Part pattern: (a) +X, (b) −X, (c) −Z.
  - SV3, the coplanar section cap, matches live in the GUI.
  - The rest point at their fixes.
- **navigation.md: all ✅ except 2.9** (macOS only; needs a Mac).
  - 3.3 / 3.9: new T `step_rotation_is_linear_and_view_animations_smootherstep`.
  - 7.11 is n/a (mobile apps).
- **workflow-sketch-tools.md: all ✅.** The sketch tool search matches live by CLI.
- **sketch.md: all ✅.** 14.41 SKETCH-VDIM-SIDE-ARROWS, corner Coincident glyphs and the line highlight colours checked in the GUI.
- **assembly.md: standard content settled against live's dumps.**
  - All 333 components and their size lists are there, including failing sizes. All 2919 default Length lists match (skasm-scratch/sc/cmp3.py; compare.py over-reports).
  - LA54: the #8 Type VI mouth fillet builds (T `flat_countersunk_type_vi_mouth_fillet`).
  - T11: the assembly toolbar order matches live.
  - LA55's NAS Near connector is asked of live: REQUESTS "skasm (2026-10-31 04:10)".
- **workflow-configured-assembly and workflow-undo rows settled.**
  - Tab ops aren't undoable, as live (CLI).
  - The app clears undo / redo on reopen; the CLI keeps them on purpose.
  - UNDO3 (a drawing note takes two undo steps in live) goes to dx.
- GUI tip: a Line's value box exists only after the segment's second click (click, click, then type 6 Enter), as qa's flow. Typing after the first click does nothing, and that isn't a display problem.

## Part Studio feature parity (lead 10-30 21:00), 10-31
- workflow-feature-sweep.md: 136 of 141 main rows ✅ (errors, dialogs, std-routed features; Fill / Decal now run their
  editing logic: FILL-MERGE-WITH-ALL, 333f6a69). Left:
  - Loft 8 ◐: a thin loft's Trim ends is ticketed to core as THIN-LOFT-TRIM-ENDS. Our thicken of a creased sheet already
    self-intersects. My extension attempt is parked in /root/skasm-scratch/thinloft-trim.patch.
  - Loft 8 ◐ also: Show isocurves, drawn by no feature yet. Waiting for the live picture (REQUESTS "skasm (2026-10-31 03:40)" (e)).
  - Draft 10 ✅: our sides come from the geometry, so std's hint faces aren't needed.
  - Body draft 7: no live case.
  - Thicken 4: L17 join, core's.
- workflow-advanced-part-studio.md: every main-table row ✅ except 1.7 (a click on the circle's centre: point or
  region?), live request "skasm (2026-10-31 03:10)".
  - ADV2 withdrawn: live's evVolume HIGH 5927.754323 = ours; 5927.728 was /massproperties.
  - HOLE-MISS-MSG withdrawn: live auto-flips a bottom-face hole too.
  - ADV12: live's row is the whole callout.
- Later 10-31, other sheets re-checked against live answers / main CLI and marked:
  - surfaces SF1 / SF2 / SF5 / SF6, Split naming;
  - booleans NAME2 / COPY1;
  - core-workflow 3.5 ✅ / 3.6 ◐ (G46 shell 1 mm +1.19 mm³ → core);
  - p1-pass P1-6 / P1-16;
  - bracket ADV9;
  - derived 7 / 10 / 15 / 18 / 19;
  - import-edit I3;
  - assembly K27 / D21.
  PATTERN-REAPPLY-CUT-ONLY closed (live = ours). Landed SKETCH-DIM-CURVE-PAIR-PREVIEW (640f0c7c).
- Live answered 10-31: advanced 1.7 (centre click = point, region off it) and the search ranking ("Fill" lists Fillet first, Enter opens Fillet): both = ours.
- Live requests open:
  - "03:40": thin loft + the Show isocurves picture. Show isocurves is done from the help's colour (#d403f9) and pictures; compare with live's (e) when it's answered.
  - "04:50": part colours after Split / Mirror / pattern, SF4.
- Sketch: SKETCH-DEGENERATE-SOLVE (P3). Live solves a fillet R6 → R20 on a 20 × 10 rectangle with the top line shrunk to a
  point; ours stalls at 4e-10. Needs degenerate-line equations or a revised `collapsed_lines` rule.

## 10-30 evening batch (lead 16:55: Copy Flat pattern, Form ▾ pictures, perf.sh figures, then the rows left)
- Landed: Copy Flat pattern (flat body ids `noshape_model::sheetmetal::flat_body_id`; the assembly evaluates such a
  Part instance to the flat view's solid, "Flat pattern of <part>"; T1 a_flat_pattern_instance_is_the_flat; GUI: pasted
  into Assembly 1 as live), OsSelect pictures (`set_pictures`; Form ▾ rows 70 × 40, psui.md line), the partial cylinder
  Radius readout (MEASURE-PARTIAL-CYL-RADIUS), a user form studio's flat sketch (T1), the rebuild speed-up (keep_names
  box test, glued wall/bend fuse: the Hem's host rebuild 2.4 s → 0.47 s dev build), no comb on a straight curve.
- Louver settled: live's evVolume HIGH 19611.187053 = ours (its /massproperties was 0.15 low); T2 within 1e-4 for all three.
- workflow-sheet-metal.md: every row ✅. workflow-drawing-sheet-metal.md re-checked (DSM2 / DSM3 ✅). The assembly /
  configured-assembly / sketch docs' open-looking rows are all fixed in later passes (A7, A10, A19, CA4–6, D17, D18,
  C17 / ASM-PATTERN-SUBASSEMBLY verified fd036a5c).
- Speed (vibe3, release, perf.sh at 7f7364d3, `regen_profile::sheet_metal_profile`): the sheet metal model (200 × 150
  plate, 4 flanges, hem) **~300 ms** a regeneration (was 1321), **Hem ~181 ms** (was 1089), **Flange ~105 ms** (was
  ~225); cold = warm (std's module load is cheap in release: my "first-run load" explanation was wrong; the startup
  prewarm of the sheet metal modules I added is at most a small help). The full `regen_profile` run that still showed
  the Hem at ~1.1 s "at a88e8c29" most likely built stale sources (see the perf.sh request below): locally the same
  dev build gives the Hem 465 ms alone and 472–528 ms after the big fixtures, so no fixture effect.

## 11-01 morning: SNAP-INSERT-FACE landed b311fa26; the whole-surface audit (user 11-01)
docs/parity/audit-sketch-assembly.md: every live surface of sketch / assembly / sheet metal against ours (14 surfaces,
9 gaps, 7 fixed in one commit: tooltips, the top row's fix-within texts). Tools: /root/skasm-scratch/audit/extract.py
<dump> lists a dump's labels missing from our sources. Waiting on live: REQUESTS "sketch tool options whole dump"
(then ask the exploded-views / named-positions menus per state for G6). Next: the sketch tool options once answered,
a UI test pinning the assembly menus to live's dumps, G7 (vendor input clear-button tooltip).
11-01 later: audit 18 surfaces, 16 gaps, 9 fixed. Landed 1183f939 (tooltips); next commit: the **Flow** sketch constraint
(G12, P1, live 1716: tangent + G2 + equal dκ/ds; sketch equations FlowSide / FlowNum, Nurbs::eval_ders3), the BOM
excluded-row tooltip, assembly-ui UI test the_instance_and_top_row_menus_are_lives. Open in order: G16 (Curvature/Flow
from a far start with an arc don't converge; live moves the arc), G11 Insert DXF dialog, G13 Insert image Libraries,
G14 Constraint manager types / In context, G6 (live request posted 10:00), G15, G7. Flow isn't in Preferences ›
Shortcut toolbars (SKETCH_PREFS_LIST, from an older live dump): check when live's list is re-dumped.

## 11-01 midday: audit 18 surfaces, 18 gaps, 17 fixed (docs/parity/audit-sketch-assembly.md)
Landed: 660bd350 Flow constraint, ec909181 G2/G3 residual scaling (G16), 67cb2d16 Constraint manager / Profile
inspector / no Text option box. This commit: G6 inactive exploded-view menu (Activate / Reset starting position /
Duplicate), G18 no fix icon on parts inside subassemblies, G11 Insert DXF or DWG and G13 Insert image as live's
select-item dialog (ReferenceKind::Drawing2d; SelectItemPanel gained `ok` (✓) and `title`). Open: G7 (vendor input's
clear-button tooltip), G13 Libraries tab (no image library), Flow glyph position, "In context" constraint mode.
Landing now queues (land.sh returns QUEUED; `land.sh --status`); while iterating run single tests only.
Later 11-01: queued 17a951a9 (Flow glyph two-thirds along), f9137d7b (G7 Clear search, vendor input), 3be177d6
(Constraint manager In context mode). Audit: 18 surfaces, 18 gaps, 18 fixed (G13 Libraries open). Waiting on live
request "remaining assembly right-click menus" (REQUESTS 12:30): connector / relation / pattern / mirror / replicate
rows, mate folders, graphics mate indicators, nested rows, empty list space. Then: whether live's Customize shortcuts
and Shortcut toolbars lists now include Flow.
11-01 afternoon: live 1723 (remaining assembly menus) done: G20-G25, G27 fixed and queued (f363df06, 27efdc47).
Audit now 19 surfaces, 28 gaps, 26 fixed. Open, in order: G26 nested instance rows' menu (needs occurrence-level
hide / isolate / fix in the model: Instance has only `nested` placements), G28 a menu for several selected mates
(needs multi-select of mates: `selected_mate` is one id), G13 Insert image's Libraries tab (no image library).
Later: G27 (dynamic suppression on patterns / relations / connectors) and G28 (several mates, Ctrl+click) fixed too;
audit 19 surfaces, 28 gaps, 27 fixed; G26 planned in the audit doc (paths for hide / isolate). SM-MAKE-JOINT-ONE-EDGE:
two FS probes, see the ticket (each definition entity gets two association attributes). Live request "Preferences
shortcut lists" posted 12:05 (does Flow appear there).

## 11-01 afternoon (user: G26, G13, SM-MAKE-JOINT, keep auditing)
Done and queued: G26 per-occurrence state (Assembly::occurrences; isolate / transparent as occurrence paths in
scene, picking and ops; nested row menu; agent API paths "Sub <1>/Part 1 <1>"), G29 Flow in the Preferences lists,
G13 Insert image Libraries (our generated environment maps), SM-MAKE-JOINT-ONE-EDGE (root cause: two models' parts
named their fresh faces alike; a part whose names are taken now gets its own). Audit: 25 surfaces, 30 gaps, 29 fixed.
G30 Display states fixed (25f9e103 queued). Audit 26 surfaces, 31 gaps, 30 fixed (G31 variance: request to the
lead). a409fa87 failed land (registry_matches_onshape_lists counted 54 sketch shortcuts; fixed in 1b7767e2, which
re-queues everything after it). Waiting: live request 13:45
(assembly Mass and section properties panel whole dump). Not done in G26: Move to new subassembly from a nested row.

## 11-01 late afternoon (user: G26 remainder, then the document-level audit)
- G26 remainder done: a nested row's Move to new subassembly (6a810b33). All of G26 is fixed.
- New audit area: the document-level surfaces, in docs/parity/audit-document.md. 19 surfaces, 8 gaps, 7 fixed. Queued as 3aa350f6, which carries the whole stack.
  - D3: the version window's ☰ menu (Copy version…, Version properties…).
  - D10: ☰ › Workspace properties… opens the metadata modal.
  - D7: the Documents search header and empty text.
  - D8 (P1): part / tab search results get a row each. Each workspace keeps an index of its parts, written with the thumbnail (`app-core/search_index.rs`).
  - D6: Documents › Versions and history (graph, Show changes…, tooltip).
  - D9 and D11: tooltips.
- Landed 64023b00 (16:43): the whole stack (G26, G13, G29, G30, SM-MAKE-JOINT, the document audit D2-D15).
- For dx: your queued drawing_behind (a drawing pinned to an older version of this document) and my D12 put the same out-of-date icon on tabs in workspace.rs's tab bar; keep one icon shown when either holds (`self.behind.contains(&id) || drawing_behind(...)`).
- D2 fixed later with ops' `drawing::old_state`, and so was D15 (the Workspace protection notice).
- Later: D12 tab out-of-date icon; D13 the search box takes the query as text (`type:part name:pvz`). Audit at 25 surfaces, 15 gaps, 15 fixed (live 1738 answered: D16 Tab › Properties… per kind, D17 Document details…, D18 the copy dialogs; landed 61ef991d 17:25). The three P3s left after it are done too (Part Studio Properties card, Create version's drawing links, the Documents panel's Search history ▾).

## Solver fuzz against live (user 11-02 09:40): counts
Cases, generators and live's answers are T2 tests: sketches `-p noshape-sketch --test all live_fuzz:: -- --ignored`
(fixtures/live_fuzz/sketch-N.json: ready-to-post BTMSketch JSON + ours; sketch-N.live.json: live's answers),
assemblies `-p noshape-model --test all oracle_asm::asm_fuzz -- --ignored` (fixtures/live_fuzz/asm-N.json in the
fs-oracle-asm vocabulary, mates added one at a time as live does; asm-N.live.jsonl: live's lines);
`oracle_asm::asm_fuzz_trace` (FUZZ_CASE=<seed>) prints one case mate by mate.
| batch | cases | sent to live | answered | match | differences fixed |
|---|---|---|---|---|---|
| sketch 1 (lines, arcs, circles; 11 drags) | 20 | REQUESTS 09:45 | 20 + drags + curve colours | solvability 19 → 20; positions 0–1 (least change) | 1: a redundant dimension |
| least-change probes 1 | 14 | REQUESTS 11:30 | 14 | 4 → 14 | the least-change rule |
| least-change probes 2 | 10 | REQUESTS 12:15 | 10 | 0 → 10 | the least-change rule |
| asm 1 (BLOCK / PLATE / PIN, 1–4 mates) | 20 | REQUESTS 10:05 | 20 | 15 → 20 | perpendicular start; Slider pivot; "not consistent" rule; exact flip turns the free part |
| sketch probes 3 (rectangles) | 10 | REQUESTS 15:10 | 10 | 10 | – |
| sketch probes 4 (rectangle + line) | 8 | REQUESTS 1780 | 8 | 5 → 8 | follow check, order |
| sketch 3 (40, splines / conics / angles) | 40 | REQUESTS 1777 | 40 | solvability 40/40; positions open | – |
| asm 2 (3–5 instances, 99 mates) | 30 | REQUESTS 15:40 | – | – | – |
| asm probes (1776: a1–a6, b1–b5) | 11 | REQUESTS 14:00 | 11 | 8/8 encoded | – |
| sketch 2 (+ fit splines, conics, angles) | 20 | REQUESTS 10:40 | 10 (10 spline cases refused: our spline JSON form, live reposts) | solvability 10/10 | – |
Fixed before live answered: an axial mate whose connectors start exactly perpendicular (a part inserted square to its
mate) could solve reversed: the residual is as flat both ways, the orientation pass only turned parts more than 90°
off. The old code accepted the reversed pose as OK; the 11-02 conflict rule then flagged it. Now the moving part is
turned the shortest way onto the alignment first (T1 a_perpendicular_start_ends_aligned); asm batch 1's mates in error
went 9 → 5 (the rest are deliberate conflicts).
Sketch batch 1 against live (11-02 11:20): solvability (solved vs "Sketch could not be solved.") 19/20 → 20/20 after
fixing s7: **a driving dimension the rest already fixes over-defines the sketch** (a radius dimensioned twice at the
same value), where redundant relations (a second Perpendicular, an Equal twice) stay fine; ours called it "redundant"
and solved (constraints_more updated). Live, like ours, solves the rest of an over-defined sketch (its "could not be
solved" sketches still moved, up to 22 mm; only 3 kept the posted positions); tried and reverted keeping the geometry.
Positions: 1/20 exact. Live keeps well-tied geometry exactly where it was and moves free ends (a dimensioned line's
free end, the lone line an Equal ties to a rectangle), where ours spreads every change by a weighted minimum norm
(sum of squared moves: live 3153, ours 1270 over 6 cases). Weighting by constraint counts got 3/11 at best, so not
that; 14 minimal probes sent (REQUESTS 11:30) to pin the rule before changing the solver. live_matches fails on
solvability only and prints position differences until then.
Least-change probes (sketch-probe-1, live 11-02 10:04; 4/14 match: the four concentric pairs): live behaves as if
a line were (start point, direction, length) and nothing is split between entities:
- a length change moves the END only (p1 lone line, p2 horizontal, p4 an L's second line); when the end is tied and
  the start free, the START moves (p10 line, tangent arc, line: the arc and the other line stay exactly);
- what hangs on a moved end is carried rigidly (p3: the L's second line translates whole; p9: a rectangle's right side
  moves, its left stays);
- Parallel turns the second line about its start, whatever the argument order (p11, p12); Equal makes the 20 mm line
  10 mm, whatever the order (p13, p14); concentric: the second circle's center moves onto the first's (p5–p8, = ours).
Ours (weighted minimum norm over point coordinates) splits each change (p1: both ends 2.5 mm). Minimum norm in
(start, angle, length) gets p1, p2, p4 but still splits p3 (37.5 < live's 50), so live isn't a minimum norm at all:
it reads as a constructive solve (keep what is tied, move what dangles, carry rigid pieces), like the assembly tree
placement. That's the next solver change (it also governs the app's dimension edits: dimension_edit_options only
special-cases an arc's radius, so editing a line's length moves both its ends today, live moves the end).
Probes 2 (live 10:10, 10 cases, 1 match: q9 concentric) separate the readings: a lone line keeps its endpoint with
the SMALLER x whatever way it was drawn (q1 (100,50)→(120,50) keeps 100; q2 (20,0)→(0,0) keeps the end at 0, its start
goes to 15; q3 (−20,0)→(0,0) keeps −20, the end at 0 moves); Equal shrinks the longer line to the shorter in either
creation order (q5, q6); Parallel turns the line at the larger angle to the smaller, about its lower start (q7; q8 5° /
10° → both 5°); concentric moves the circle at larger coordinates (q9). Structure first: a dimensioned line whose end is
tied to an arc moves its free start (p10), what hangs on a moved end is carried rigidly (p3, p9, q4, q10).
Design for the solver change (next): a constructive pass like the assembly tree placement: entities ordered by how
constrained they are (ties: the "smaller" first: lower point, shorter, smaller angle), each solved alone against the
placed ones in its own (lower endpoint, angle, length) terms with the least change there, then every constraint
checked; anything unheld falls back to today's weighted minimum norm. Probes 1+2 (24 cases) and batch 1 are its tests.
**Done (11-02 11:00, constructive.rs):** a regeneration's solve and a dimension edit (SolveOptions::constructive,
default off: the tools rely on the solve to pull new geometry onto its inferences, live places it there exactly) first
place each component whose constraints don't hold (Sketch::unheld_constraints: one evaluation, plus coincident points
/ concentric centers apart, which the solver aliases): the next entity is one with a constraint to placed geometry
that doesn't hold; one hanging on it alone follows rigidly (carried onto the shared point), tied neighbours are held;
its size (length / radius) is kept when it can be; a relation that doesn't hold keeps its smaller member as is; an
entity that can't close a loop alone goes with its neighbours (the change shared within that group); kept only when
the regular solve then finds everything held where it was put, else today's weighted minimum norm. Probes 4/24 →
24/24 (all of live's answers); batch 1: positions now match in s3 and s12, the solved s1, s5, s6, s8, s10, s11, s14,
s17 still differ (loops, tangent chains: next, case by case); onshape_ux's "length edit moves both ends by half"
(never checked against live) now moves the far end as live. S300 timings unchanged on vibe3.
Batch 1/2 case work (11-02 11:30): positions now match live in s1, s3, s10, s12 (batch 1) and s106, s116 (batch 2);
s8 is 0.024 mm off (a vertical line's free end). Rules added from them: only joining constraints (shared points,
tangency, point on curve, concentric) hold neighbours, relations to unplaced geometry (Equal, Parallel…) wait for it
(s10); a relation keeps its member with more tied points before the size / angle / position key (s1: a rectangle side
kept, the lone line with a free end lengthened); a broken relation's kept member is settled before single dimensions
(s8: the dimensioned circle of a concentric pair moves onto the other); conics / splines are placed too, at their
lowest point (s116). Left among the solved: s5, s6, s11, s14, s17, s102, s104, s115 (loops / tangent chains), the
curve colours (live shows some lines black that ours calls under defined: s10 l1, s116 l2), the drags.
11-02 12:30: s5 matches too (a line placed alone keeps its length and direction first: a side carried by a distance
translates). Live's curve colours "fully" on dimensioned lines in an under-defined sketch (s5, s10, s116) look like the
black dimension label at the sampled midpoint; asked live to re-sample. Open, with what they show:
- s104 (a rectangle's diagonal dimensioned): live = minimum norm with the lower-left corner pinned (width and height
  grow in proportion); ours keeps the whole "kept" side. Suggests the model is "pin the smaller anchor of each change,
  then least change in line terms (start, direction, length)" rather than unit-by-unit placement;
- s115 (a rectangle's bottom shrunk, right side grown): live kept the TOP-RIGHT corner; probes 3 (REQUESTS 15:10,
  r1–r10 rectangles) asked to separate the rule;
- s17 (slot, the arc's chord dimensioned): live shrinks that arc about its center and shortens both lines at the far
  end (the other arc 1.309 mm further in); not derived yet;
- drags (s1, s3, s12, batch 2's 7): live keeps dimensioned geometry and changes free sizes (s3: l3 with its length
  stays, the arc's radius and l1's length change); ours drags by weighted minimum norm. Needs the drag routed through
  the constructive pass with backtracking (an entity revisited with less of its shape held when a later one can't
  be placed), and a cheap path for the editor's drag steps.
UI drags (11 in batch 1) compare ours dragged to where live's pointer ended (live lands within 0.2 mm): s12's differs
the same way (what follows the dragged point).
Assembly batch 1 against live (11-02 12:00): 15/20 match (statuses, messages, transforms to 1e-3 mm / 1e-6;
live's tooltips carry "<mate> has error: " and both solvers stop near 1e-7, so the first comparison's 8/20 was
mostly that). Open, as KNOWN in asm_fuzz_live_matches: f1 / f7 a conflicting Fastened reads "Mate overdefines the
assembly." in live where ours (1764 (5)'s rule: a Fastened that failed before the solve) says "not consistent"; the
orientation-holds refinement didn't separate them (one live data point for "not consistent"): needs a probe. f8 / f12:
a Slider's free value after a large reorientation lands 44 / 47 / 33 mm further along its axis in live (ours keeps the
connector's start position along the axis): live measures that least change about another pivot. f16: live leaves a
Parallel of the conflicting loop OK.
Assembly probes (live 1776, 11-02 11:20) settled two rules, both now ours (T2 oracle_asm::asm_probes_match_live, 8
probes; asm batch 1 15 → 19/20, f16 left): (a) a Fastened that can't hold reads "Mate is not consistent." only when
the failing mates around it span three rigid pieces or more (bodies a failing Fastened joins count as one, ground as
one: a4 / 1764 (5), a chain of Planars back to ground); otherwise every mate in the conflict "overdefines" (a1 a second
mate on the pair, a6 through a fastened intermediate, fuzz f1 / f7). (b) A Slider started perpendicular turns the part
about its connector's foot on the other connector's axis line, then slides it onto the axis (b1–b5: the slide value is
the start position along the axis less the connector's drop onto it); the Slider joins the orientation pass for that
(its residual holds the whole turn, so it never needed it otherwise). A Cylindrical keeps turning about its connector
(f2 / f3 / f4 / f9 match that way; the foot there broke them).
f16 (11-02 11:50): from D's side, as the lead asked: after Parallel 1 ours already matched live (C flipped onto B); at
Parallel 2 (D onto C, an exact half-turn with neither fixed) the R3 rule turned C, breaking it off Parallel 1; live
turned D, the part with no other mates. Now in an exact flip the part held by other mates stays and the free one
turns; between two free (or two held) parts the R3 rule as before. asm batch 1 20/20, oracle unchanged.
Probes 3 (rectangles, live 11:25): 10/10 already: a lone rectangle keeps its lower-left corner whatever is
dimensioned (r9 / r10 = s115's rectangle alone). So s115's top-right corner comes from the Perpendicular tying its
left side to the dimensioned chain line l1: geometry related to changing geometry gives way. One case; left open.
Live's batch-2 colour correction (1778): every "fully" was a dimension arrow / label artifact; they are blue as ours.
Next batches: splines / conics / angles (once live confirms their REST form), assemblies with drags and patterns.

11-02 12:00–13:30 (constructive.rs, from batch 1/2 cases, probes 4 and batch 3; summed position error over the solved
non-drag cases printed per case by live_matches, `one_case` with FUZZ_SEED / FUZZ_PROBE prints one): batches 1+2
435 → 388 mm (s8 exact; s11's triangle, s14's slot and arc chain, s104's rectangle and arc exact), all three batches
1855 → 1829 mm, probes 1–4 42/42 (probes 4 were 5/8), batch 3 solvability 40/40. Rules added: an arc placed alone
keeps its end directions; a loop the change can't pass pins the change's lower point and least-changes the rest
(a rectangle's diagonal scales it about the lower-left, s104); a distance between one curve's own ends (through shared
points) is that curve's size (s17); a relation member whose own H / V / dimension fixes the related value stays (s8);
a relation's keep waits for a chain of its kind to a dimension (s11), keeps only the shortest / smallest angle in
[0°, 180°) of its class (s14) and only a line agreeing with its Parallel / Perpendicular class; a released neighbour
must be able to follow (stretch along itself, or be carried across by neighbours that stretch; probe t1, s5, s8);
units ordered by pending, then joining constraints, then the lower one (t4); a line's least change measured from
before it was carried; sizes / directions kept are as the pass started (corner rectangles share points: the land
of 963fae85 failed e2e::configurations on that, fixed with T1 onshape_ux::corner_rectangle_width_edit_keeps_its_height).
Tried and dropped: keeping the other member when the relation's choice can't stay (s226 −62 mm but broke s1, exact).
Open, biggest first: s215 (331 mm), s6 (170), s102 (162), s236 (114), s226 (120), s218 (76), s209 (71); s17 live
scaled the whole slot about a2's center (a4 moved 1.309 mm in; no rule yet, probe it); s104's triangle turned about
the intersection of l9 with l3 (the Perpendicular's lines; one case, probe it); s11's quad kept the LONGER of an
Equal pair (l5) where its ends are both tied. Probes 5 next: Perpendicular pivot, slot scaling, Equal between tied lines.
11-02 13:00–13:40: probes 5 answered (10/10 now; probes 1–5 52/52): a Perpendicular turn pivots about the two lines'
intersection; a line moved at its end keeps its start and turns (one free in direction can always follow; u8 / s8);
Equal keeps the member whose far end lies further left (`equal_key`: lone lines the longer shrinks, a quad the
shorter grows); an arc with a new radius keeps its end on a held neighbour with its own dimension, else the lower end
(`lower_shared`); an arc's chord dimension scales a free component about the arc's centre (`scale_for_chord`).
Batches 1–3 summed error 1829 → 1700 mm (s17, s214, s223, s234 exact). Live 1775: a dimension typed in the EDITOR
grows a lone line about its midpoint (dimension_edit_options no longer constructive; the regeneration with one new
value still is: model-core regen/features/sketch.rs). Asked live: the same edit on a corner rectangle.
Assembly batch 2 answered: statuses / texts 89/89, transforms 27 → 48/50 after f43 (a part > 90° off an axial
mate turns the shortest way, not over about its X). Known: f40 (a Slider 67° off: live 31.7 mm further along; the
foot pivot gives 43.9), f50 (a Revolute in conflict with a Cylindrical still slid the plate along the axis); asked
live for asm probes 2. Tried and dropped (net worse): y-first "lower" points (s215 −183 mm but s1 broke), keeping the
Distance member at the lower point, releasing every moved unit when a loop can't close (s6 170 → 403).
Open, biggest: s215 (331: live moved the whole triangle 30 mm, kept the slot's bottom), s6 (170: the constructive
pass fails at the last side, l9's Equal length set before its partner settled), s226 (120), s236 (116), s102 (107).
13:40: landed the axis key (lines related by Parallel / Perpendicular: the one nearer an axis stays, ties the smaller
angle; s102 −28 mm; batches 1–3 1666 mm) and drags holding dimensioned curves (s3's drag exact; T2 drags now step 25
times as live's UI). Sent to live: probes 6 (w1–w10: a distance between two separate loops, which one moves: s226 /
s236 / s215 disagree), drag probes 7 (d1–d8, UI: s1 shows live translating a dragged rectangle whole, the other loop
untouched; our point-space least norm deforms it: the drag wants the shape-keeping placement, pending d1–d8), sketch
batch 4 (s301–s340, everyday profiles regenerated with new values), asm probes 2 (f40 slider, f50 conflict), the
corner-rectangle editor edit. T2 tests: live_fuzz::drag_probe_matches, batch 4 in live_matches once answered.
13:40–14:00 (all landed): probes 6 (10/10): a distance between two separate loops keeps the loop with more vertices
(`loop_points`: curve ends, a circle one), between equal ones the one at the smaller coordinate along it; the other
moves whole (`shift_loops_for_distances`, at the start and again once the kept loop settled), or rigidly onto several
relations to the kept loop (`place_loop_rigidly`: spokes from two vertices; s215 241 → 40 mm). Batches 1–3 now
1266 mm (from 1829 this morning). Drag probes 7: 8/8 (live confirms ours: undimensioned sizes give way, dimensioned
pieces translate). Live 1785: a length / distance typed in the EDITOR scales the free geometry uniformly about its
centre or its Origin point (`Sketch::scale_for_edit`, called by sketch_mode before the edit's solve; not radii:
SK5). Assembly: Slider = one turn about a point of the slide axis (probes 1786 sa1–sa6, f40 matches); an axial mate
opposed to the part's other axial mates still moves the part where its flipped self would (cb1, f50's B; C's free
Ball 2e-6 off: f50 stays KNOWN). Asm batches 1+2: 49/50.
Open: s6 (170: the pass fails at the last side, see above), s236 (93), s226 (58), s102 (79), s218 (90: a chain with
two distances to a merged loop that live translates, ours turns slightly; the direction-hold tweak didn't reach it),
batch 4 pending.
14:00–14:15: batch 4 (everyday profiles, s301–s340) answered: positions 14/40 at first, now 25/40 (24 exact + s332
curve colour only; b4338a57's message says "14 -> 24": counted before the hub rule); 2522 → 1662 mm. Rules from it:
a relation's keep decided by that relation alone (`kept_first_in`: slot centre distances), a line with its own length
can't stretch to follow (L-brackets), of tied candidates one that can be carried whole first (`can_keep_shape`), of
two equal loops the one more tied to other loops stays (bolt circles). Batches 1–3: 1266 mm.
NEXT (biggest left, the filleted rectangles s319 / s321 / s324 / s325, ~260 mm, constructive pass fails): live
(s321) keeps the leftmost UNCHANGED side (the left vertical) and walks round the loop from it: the corner arc next to
it grows about its end on that side (taking its new radius through the Equal chain to the dimensioned arc), the
dimensioned bottom is then carried from that arc's end and takes its length keeping its start, the next arc grows from
the bottom's end, the right side translates, and so on round. Ours places the dimensioned bottom first (own unheld
Length) keeping its own lower end, and fails to close. Needs: an anchor unit kept first when a closed loop changes
only in its sizes (its leftmost unit whose constraints hold), candidates adjacent to placed geometry first, and an
Equal partner of a dimensioned curve taking that value in its own placement. Also open: s307 (which side a slot's
new radius keeps: live kept the free top line, the bottom had Horizontal; s201 / s223 disagree on "dimensioned
neighbour first"), s6 (170), s226 (58), s236 (83), s102 (79), s218 (90).
14:30: the filleted-rectangle plan landed as `loop_anchors` (a closed loop changing only within itself keeps its
leftmost unchanged H / V line; its curves next to placed geometry first; `chained_size`: an Equal partner of a
dimension takes that size and counts as changing): s308, s316, s318, s324, s325 exact; batch 4 now 31/40 on
positions (s326, s332 only curve colours), batches 1–4 1412 mm. S300 timings unchanged (vibe5). Sent batch 5
(s341–s380, same everyday generator, new seeds) to check the rules generalise: compare its first answer with batch
4's first 14/40. Left in batch 4: s321 (two filleted rectangles: the second shifts 2 mm, a keep step on its arcs),
s307, s319 (26), s303, s331, s334, s335, s340, s320.
15:05: batch 5 (s341–s380, sent before its rules were tuned): live counted 22/40 against ours as sent (batch 4's
first answer: 14/40), so the rules generalise. Since then (all landed or queued): tied candidates the placed geometry
pulls furthest first (`pulled_by`); an own H / V reading unheld only through a moved shared end isn't pending; a
joining constraint reading unheld only that way (`aliased`) doesn't trigger the keep; an arc pinned to placed
geometry with its size held can't follow a carried neighbour; loop anchors only Vertical lines (and only loops with
arcs); `lower_shared` prefers a neighbour without its own H / V (a slot keeps its free top over its Horizontal
bottom). Now batch 4: 39/40 on positions (s303 left; s326 / s332 / s340 curve colours only), batch 5: 39/40 (s354
left), batches 1–5 1315 mm (1829 this morning on 1–3 alone, which are now ~1100). Asm batch 3 (f51–f80) is with
live. Next: s303 / s354, then batches 1–3's s6 (170), s226 (94), s218 (90), s236 (81), s102 (79).
15:55: asm batch 3 (f51–f80) answered: 23/30 on the first try (batch 2: 27/30, batch 1: 15/20). Landed (queued
a4b7006a): an exact flip treats a part held only by a Ball, or by a Fastened to a part nothing else holds, as free to
turn (f53's false "overdefines" gone). Known, asked live (asm probes 3): which mates of a conflicting loop read red (f80
spares a Fastened, f79 flags its Fastened mates: a "Fastened that held before stays OK" rule broke f62 / f79, dropped),
f69 "not consistent" on a Parallel, f53's half-turn axis with both parts held, f52 / f55 / f64 free poses. Sent UI edit
probes 8 (editor edits on dimensioned profiles). Tried and dropped for s303 / s354 (slot with a new centre distance
AND radius: live keeps the left arc's centre x and the free top): forcing the kept-but-resized arc to be placed next
(with and without a hold-one-neighbour shape-3 step) made both worse.
16:05: editor edit probes 8 answered (live 1793): with other dimensions present an editor edit is NOT scaled nor
split: the far side (right / up) takes all of it, a slot's radius keeps its top line, a filleted rectangle's radius
keeps its left side: i.e. live's least-change placement. So `dimension_edit_options` is constructive again, after
`scale_for_edit` (free profiles only: 1775 / 1785). 6/6 with T1 width_edit_with_other_dimensions_moves_the_far_side.
That test found the pass skipped curves SHARING point entities (the tools' rectangles / polylines have no Coincident
constraints, so the pass saw four unrelated lines and fell back to "both by half"): `place_constructively` now runs on
a copy with the shared points split (`split_shared_points`). Probes now: 52 + drags 8 + edits 6, all match; batches
1–5 1282 mm.
16:13: asm probes 3 (live 1798): (a) a conflict loop through a Slider spares the Fastened links that held (the
Slider(s) and the closing mate red), loops of Fastened / Revolute links read red throughout; (b) a Parallel that held,
caught in a conflict over three pieces, reads "Mate is not consistent." (flip either way); (c) f53's half-turn point
depends on what the turned part carries (not done). In (queued 3e346cfe) with: the orientation pass turns a part with
the parts Fastened to it (`turn_carrying`: f80's block left behind made its Fastened read failing). Asm batches 1–3
74/80; known: f50 (C's Ball 2e-6), f52 / f55 / f64 (free poses through Ball / Cylindrical / Slider chains), f53 (half-turn
point), f54 (a loop of Cylindricals: live flags Cylindrical 1 and 3). Tried, not needed: flagging exactly the loop the
failing mate closes (cut edges between its parts): no change once the carry was in.
With live now (16:30): sketch batch 6 (s381–s420: the everyday kinds plus washers, a plate with a located hole, a D
shape; generator kinds 5–7 only from seed 381, so batches 4–5 stay as sent) and drag probes 9 (g1–g6, everyday
profiles; `drag_probe_matches` runs probes 7 and 9). Tried and dropped: Equal relations not freezing a unit in the keep
step (s201 −26 but s214 / s209 +27 each; s6 still fails at l11, see above).
16:45: batch 6 answered: 37/40 on its FIRST answer (batch 5: 22/40, batch 4: 14/40): the rules generalise to new
everyday profiles and the three new kinds. The three (washers with a changing diameter) fixed: a Concentric pair tied
to nothing else places its kept circle first, not holding the other (40/40). Now: everyday batches 4 / 5 / 6 at
39 / 39 / 40 of 40, probes 52 + drags 8 + edits 6 all match, batches 1–6 1281 mm; drag probes 9 with live.
16:46: drag probes 9 (live 1801, everyday profiles): 4/6 match (fully dimensioned L-bracket / slot translate whole,
undimensioned sizes next to the dragged point give way, the filleted rectangle translates in x and grows up). Open:
g4 (an undimensioned slot's top corner dragged: live keeps the bottom line still and the radius grows; ours, least
norm, lowers the bottom 0.68 mm), g6 (a fully dimensioned bolt circle, one hole dragged: live turns the pattern
rigidly about the undragged lower-left hole; ours moves the pattern centre). Both say a drag keeps far geometry
stiller than our point-space least norm: the next drag change (weights growing with distance from the dragged point,
or the constructive pass with the dragged point pinned), with drag_probe_matches (7 + 9) as its test.
17:20: tried for the drags, both dropped: distance-falloff weights (never exact, no probe changes), the constructive
pass with the dragged point pinned (probes 7 8 → 3/8). Asked live for assembly drag probes d1–d6 (Revolute pin,
Slider, Planar, a two-link arm, Cylindrical, Ball); T2 oracle_asm::asm_drag_probes_match_live is ready for its
answer (`asm-drag-1.live.jsonl`; cases built from live's own input by `spec_assembly`, 25-step view-plane drags,
positions within 0.3 mm for the pointer's landing). Tried for f53: half-turning about the Ball's point: no change
(live's half-turn AXIS differs: about world y there).
17:48: assembly drag probes (live 1802): a drag moves only along the dragged part's own freedom (Planar follows,
Revolute turns to the pointer's angle, a Slider / radial pull on a Cylindrical do ~nothing, a Ball pivots, a two-link
arm turns only the dragged link). In (queued a7d3bf69): `solve_dragged_only` keeps a dragged part that moved alone
(its group moves only when it can't, a linkage); the pointer is a ray (`DragTarget::along`, eval
`SolveOptions::drag_view`, set by assembly_view from the drag plane's normal = camera forward). The two solver tests
that assumed whole-chain following now assert d4's rule (the iteration budget measured on its own following copy).
T2 asm_drag_probes_match_live: 2/6 within 0.3 mm (d2, d3); d1 / d5 / d6 differ by live's pointer lag (1–2°), d4's
small turn size. Asked live for drag probes 2 (a tangential pull on the arm, a Fastened link, a four-bar) to confirm
the own-freedom rule beyond a radial pull.
18:10: a7d3bf69 landed; GUI (asmchk copy, Revolute pin): a drag across the pin leaves it in place (it only spins).
s303 / s354 fixed: an arc the keep step keeps but whose Equal chain resizes it keeps the end it shares with its
`lower_shared` neighbour instead (that neighbour stays, the arc is placed next from it): the slot's free top line
stays and grows from its left end, the arcs grow down (T1 onshape_ux::slot_with_new_radius_and_centre_distance_keeps_its_top_line).
Everyday batches 4 / 5 / 6 now 40 / 40 / 40 on positions; batches 1–6 1295 → 1262 mm; probes unchanged. Tried and
dropped: skipping such an arc in the keep step (s303 12 → 55 mm: the right arc then keeps its x). Sent live sketch
probes 10 (s6 and s102, batches 1–2's largest, each with one relation left out: which relation moves what; T2 in
`probe_matches`, fixture sketch-probe-10.json).
18:28: probes 10 answered (live 1804, s6 / s102 leave-one-out). In s6 the middle rectangle grows only through k29
(l11's length) carried by k26 (l9 = l7) and the rectangle; without k29 (r6-k29) or k26 live keeps the middle rectangle
and brings the right one onto it whole. Fixed (r6-k26, r6-k29 exact): in the group fallback a closed loop (curves
joined end to end, `ring_of`) tied to another loop's placed curve gives way whole first, unless the pass kept one of
its curves (T1 onshape_ux::a_tilted_rectangle_comes_onto_the_kept_one). Batches unchanged (1262 mm). Still open in
s6: with k29, live keeps l11's start y (the middle rectangle comes down 7.8 mm) and both rectangles end 0.323 mm left
of where the distance alone puts them; r6-k28 (no distance) has the same 0.323 at the right rectangle's right side.
That shift needs k19 / k26 / k27 / k28 / k29 together; no rule found for it yet (looks like a least-squares share).
s102's l4 halves only with all of k10 / k11 / k14 / k15 (live's numeric share across two Equals and two distances).
Assembly drag probes 2 (live 1803): d7, a tangential pull on the arm's outer pin, turns only the pin about its joint
(B stays); d8, the pin Fastened to B, turns B (with C) about A's hole toward the pointer's angle: both as ours (the
own-freedom rule, a7d3bf69). In T2 asm_drag_probes_match_live (now probes 1 + 2, prints each miss as degrees / mm):
d7 1.6°, d8 0.12°, d1 1.3°, d5 1.2°, d4 2.1° (live's pointer landing); d6 (Ball) 6.3°: ours tips the block 12° (the
point nearest the pointer's ray), live 6.9° (a target in the drag plane at the point's start depth gives 5.4°):
open. d9 (four-bar) didn't close in live with vertex connectors (not a probe).
18:37: the keep step no longer keeps a unit with an unheld relation to placed geometry (s218's l10, its end due on
the kept l2, was kept by its Angle and the pass couldn't close): batches 1–6 1262 → 1223 mm (s215 40 → 16, s219 35 →
13, s218 90 → 95: now placed, but live keeps neither l8 nor l9, see the trace). Probes / drags / edits unchanged.
18:42: sent live sketch batch 7 (s421–s460: everyday kinds plus a rectangle Symmetric about a Vertical construction
line, a triangle with an Angle, a plate with a hole at a construction diagonal's Midpoint; generator kinds 8–10 from
seed 421, `Case::construction / symmetric / midpoint`) and asm probes 4 (c1–c7: f54's consistent coaxial loop reads
Cylindrical 1 and 3 "overdefines" in live, all OK in ours, while 1764's consistent Planar / Parallel loops were OK).
Tried for g4 (drag): only the dragged point's curves light in an undimensioned sketch: no change (the bottom line and
the far arc weigh the same; live's answer needs the H bottom kept, as the constructive `lower_shared`).
19:06: P0 SKETCH-CONSTRUCTIVE-RECURSION (qa, v0.4.21; landed 7607128b, qa verified on green fff177bd): a closed spline
repeats its first point id, so `split_shared_points` saw a shared point and `place_constructively` recursed on the
copy forever (stack overflow opening qa's sk211-step89.noshape). Fixed: each curve counted once, the copy placed by
`place_split` directly (no recursion left in constructive.rs). Tests: ops sketch_recursion (the document, fixture
crates/ops/tests/fixtures), sketch solver_basic closed spline / two 4000-curve chains.
19:32: the drag rule (a Horizontal / Vertical line away from the dragged point keeps its level: `SolveOptions::still_across`;
g4's bottom exact, s12's drag exact) is parked on branch skasm-drag-across (f1a81069): it makes the held-first step run
for most drags, the 1549-curve bench drag 12.3 → 14.6 ms avg on vibe3 (+18 %: a speed regression). Skipping the
weighted solve after a held-first step that converged breaks drags (probes 7 8 → 4/8, 9 4 → 0/6: that solve brings the
point onto the pointer). To land it the hold must cost nothing extra (inside the one weighted solve, or the held-first
only on the dragged component).
20:49: lead 20:15 item 1 (dx's X1 (B)): another document's Assembly was already evaluable (External key on an Assembly
tab: `external_assembly`); added the API path (assembly_insert `document` + `version`; part / Part Studio / Assembly),
live's "Versioned assembly from other document" tip, e2e assembly_inserts_another_documents_assembly_at_a_version
(landed 27549ea9). Told dx in docs/handoff/dx.md "From skasm (11-02)". App check: a document made by the CLI
(assembly_insert of another document's Assembly 1 at V1) shows "Assembly 1 <1>" with its parts, an expand arrow and
the version icon, tip "Versioned assembly from other document". Next: lead items 2 (open fuzz cases, P1 first)
and 3 (batch 7, asm probes 4 when live answers).
20:51: sketch batch 7 answered (live 1806): 40/40 match on the first answer, the new kinds too (a Symmetric rectangle's
width: left side kept, right side moves, the free centre line by half, as ours). Live's REST: Symmetric is `MIRROR`,
Midpoint `localEntity1` / `localEntity2` (the generator writes those now; sketch-7.json rewritten, same cases). Open
P1: drag probes g4 (parked, speed) and g6 (live keeps the lower-left hole, moves the centre on its circle, projects the
free hole radially: a constructive drag, not least norm; the constructive-drag try broke probes 7 before).
21:26: asm f52 fixed (a pin C, Parallel to A and on a Revolute in D's hole, kept its 137.3° turn in live; ours turned it
23°): `least_change` took free directions at σ ≤ 1e-4 σmax (its eigenvalue cut) but its correction's SVD inverted down to
1e-10 σmax, so a 1e-8 residual read along the free turn undid it and the rounds diverged (the result was then thrown
away). The correction now uses the same cut. Asm batches 1–3 75/80 (KNOWN f50 f53 f54 f55 f64); probes and drags
unchanged. Sent live sketch batch 8 (hexagon, chamfered rectangle, parallelogram kinds, s461–s500) and probes 11
(leave-one-out of s218 / s236 / s226).
Drag rule, second try (not landed): no held-first for undimensioned drags, the levels weighed 1e5 in the one solve:
g4's bottom 8e-6 off, but probes 7 8 → 7/8 and 9 4 → 3/6, and the bench (dimensioned) stays slow: its extra cost is
the held-first step with the levels fixed too (it fails more often and the weighted solve runs after it). A free
version must not add coordinates to the held-first step of dimensioned drags.

## Speed vs Onshape (user 11-01 evening): big assemblies and sketches
Cases (T2, `tools/tiers/perf.sh -n 3 -p noshape-ops --test all big_assembly_perf::big_assembly_ -- --ignored --nocapture --test-threads=1`;
sketch: `-p noshape-scene --test all sketch_perf::s300 -- --ignored --nocapture`):
- **a200**: 195 instances (420 occurrences), 378 mates. 8 stacks of 15 pins (Planar on the one below, Parallel to the
  base and to the one below); 20 instances of a flexible subassembly "Sub" (10 pins, 9 Planar) each Fastened on its own
  fixed pin; 30 standard content bolts Fastened on stack pins; a linear pattern of Sub <1> × 6.
- **a1000**: 1009 instances (1630 occurrences), 2380 mates: 40 stacks of 20, 60 Subs, 80 bolts, pattern × 10.
- **S300**: 80 corner rectangles (4 × 20, 10 × 6 mm, 15 mm pitch), width and height dimensioned: 640 entities, 480
  constraints.
- Steps as the app runs them: open (load, first evaluation from the saved solved positions, scene), evaluate again,
  regenerate (the pin's Extrude depth edited, then the assembly), add a mate (Parallel between two stack tops), drag a
  stack's top pin (solve + scene retransform per pointer move), insert a pin; sketch: open for edit (solve with DOF,
  regions, regeneration), change a dimension, drag a corner, add a dimension.

Numbers on vibe3 (release, 3 runs, medians), before (2f715f2d) → after (7e269c42 solver, connector frames, drag step; a5cec47e regenerate):
| | a200 | a1000 |
|---|---|---|
| open: first evaluation | 105 → 78 ms | 327 → 237 ms |
| evaluate again | 32 → 6.1 ms | 106 → 24 ms |
| regenerate | **621 → 177 ms** | **3693 → 840 ms** |
| add a mate | 42 → 15 ms | 120 → 37 ms |
| drag frame | 2.1 → 1.4 ms (723 fps) | **20.7 → 8.2 ms (48 → 123 fps)** |
| insert a pin | 33 → 6.1 ms | 107 → 24 ms |
S300 against live (live 1741, behaviours "skasm (2026-11-01, P1 speed vs Onshape) part 1"; live's sketch is the same
80 rectangles with its own corner-rectangle constraints, ≈ 800 constraints; ours on vibe4, loaded):
| | Onshape | noshape |
|---|---|---|
| open the sketch for edit | 1.2–1.9 s | 4.4 ms |
| change a dimension | 1.1 s | 3.5 ms |
| drag a corner | 3.8 fps, 1.6 solves/s (its VM's software renderer caps it; 2.9 fps with one circle) | 3.2 ms a step (≈ 300 fps) |
| add a dimension | 1.7 s | 3.4 ms |
Each Onshape solve ships the whole sketch (≈ 190 KB + 127 KB) over its WebSocket. Assemblies A200 / A1000: live's REST
calls are rate-limited until ~04:05 UTC 11-02 (≈ 600 / 3 500 calls to build them).
Live: asked (REQUESTS 18:30, scripting allowed); the old figures (request 1501): an assembly drag step ≈ 1.2 s at 200
instances, insert 1.3–2.2 s, sketch drag ≈ 400 ms a step at 188 entities.

Sketches on vibe3 (perf.sh, 11-02 09:30), `-p noshape-scene --test all sketch_perf::s300 -- --ignored --nocapture`:
| | live (S300, lines) | S300 (640 entities, lines) | S300 mixed (840 entities, 330 curves: slots, circles, fit splines, conics) |
|---|---|---|---|
| open for edit | 1.2–1.9 s | 3.4 ms | 12 → 7.2 ms |
| change a dimension | 1.1 s | 2.9 ms | 3.1 ms |
| drag | 3.8 fps | 2.7 ms (370 fps) | 11.3 → 6.6 ms (88 → 151 fps): solve 0.6, regions 1.7, regeneration 4.9 |
| add a dimension | 1.7 s | 2.9 ms | 9 → 6.0 ms |
| Part Studio, all 151 regions extruded (119 parts), a dimension changed | (asked) | | 265 ms (the one Extrude rebuilds every part) |
Fix (c202cb3f): a NURBS curve's box (a tessellation) and each curve pair's crossings are kept by the curves' exact
values, so a step only recomputes what moved (results identical). Left: the drag's sketch regeneration (curve and
region tessellation, 4.9 ms), the extrude of unchanged regions (regeneration, not the sketch).

Fixes so far (5bde395b / 34aa527a):
1. Profile (perf on vibe4/vibe5, sudo): ~70 % of an evaluation was dense SVD / symmetric eigendecomposition in the
   solver's `least_change`, group after group on one thread.
2. Groups solved and DOF-analysed in parallel (rayon; groups share no free body, so bit for bit the same results).
3. Groups already solved (residual < 1e-10, where LM stops without a move) skipped before copying the poses.
4. `least_change` stops at the finite-difference noise floor (step < 1e-12 instead of 1e-15; the rounds after the
   first moved ~1e-13 back and forth). Live oracle (oracle_asm, T2) and the assembly unit tests unchanged.
5. A subassembly's preparation and solve shared by all its instances within one evaluation (same tab, configuration,
   lock and stored positions).
6. Mate connector frames kept by the evaluator across evaluations (by the part's regeneration `Arc`, held so its
   address stays its own; dropped when nothing else holds it, all of them when the units change): an unchanged a1000
   evaluation spent a quarter of its time integrating the same face centroids (OCCT BRepGProp).
7. `orient_axial_mates` (run before every solve) copied every body's pose for every mate: one list kept in step
   (a1000: 5 of 7 ms of a drag step, and part of every evaluation).
8. `least_change` also stops when the free-direction pull is under the null basis's tilt (1e-8 of the pull-back,
   mates held): after a regenerate the rounds after the first only chased that leak (×0.66 a round); 12 → ~3.5.
9. The bench folds nested positions as the app does (`ops::assembly::fold_solved`); without it every evaluation
   re-solved each flexible subassembly from its unsolved start.
Tried and dropped: one SVD instead of the eigendecomposition + SVD per `least_change` round (changes which directions
count as free near the threshold: the live oracle failed).

A200 against live (live 1741 part 2; live built it by REST with **a Planar alone on each stack pin**, 162 mates, a pattern
of 7, since its Parallels as requested disagreed with the Planars; our bench case a200-live is the same; live on its software-GL VM,
wall clock; ours vibe3, release, median of 3):
| | Onshape | noshape (a200-live) |
|---|---|---|
| open | instance list 12–15 s, graphics stable 22–25 s | 44 ms (load 2.6 + first evaluation 37 + scene 4) |
| regenerate (Pins depth 10 → 12 mm) | view changes at 12.9 s, stable at 15.6 s | 63 ms |
| add a mate | 1.9 s to the answer (it overdefined), view at 7.5 s | 14 ms |
| drag a top pin | one update every 2.3–2.5 s (≈ 0.4 a second) | 0.6 ms a frame (≈ 1700 fps) |
| insert a pin | 5.0 s | 4.4 ms |
A1000: live left it for the lead (≈ 2800 more REST calls).
(The "refuses consistent redundant mates" gap noted here was wrong: 1764 shows live accepts them; 1741's Parallels
disagreed with the Planars. See "Redundant mates = live 1764" below.)

Pattern copies (11-02, user: decide against live; live 1754): Onshape's copies of a patterned (flexible) subassembly
have no freedom of their own. Their children keep the seed's positions; a drag on any copy's child turns every copy's,
the seed's too; a mate on a copy's child is solved by moving the seed's child; with the seed free (Planar) a mate on a
copy slides the whole pattern; a copy mate that would pull a copy off the pattern overdefines (nothing moves; the new
mate, the seed's grounding mate and the pattern red: "Mate overdefines the assembly."); copies show no DOF of their own.
So "seed first, then copies" was wrong; the solver now eliminates copies instead (solver::Driven, e37a02d0): a copy and
a flexible copy's every child are placed from their seed (and pattern axis) wherever poses change, never unknowns;
their mates constrain the seeds (finite-difference Jacobian through the placement). a1000's patterned subassembly:
600 unknowns → the seed's 60. T1 a_patterned_flexible_subassembly_follows_its_seed (fails on the old code: only the
dragged copy turned). Open (P2): the overdefinition messages and the red pattern of live's (C).
Also landed: one residual pass for status and convergence (99588045), the drag step's per-mate memo (16c76701).

vibe3, before (16c76701, benches open with no DOF cached) → after (e37a02d0):
| | a200 | a1000 |
|---|---|---|
| open: first evaluation | 77 → 47 → 39 ms | 258 → 102 → 78 ms (a216a0b9: a Sub's DOF analysed once per evaluation) |
| regenerate | 179 → 70 ms | 852 → 403 ms |
| drag frame | 0.73 → 0.75 ms | 4.1 → 4.7 ms (211 fps; ~0.5 ms of per-step follower work left) |
| evaluate again / add a mate / insert | 6 / 15 / 6 ms | 24 / 38 / 24 ms |
Against the start (2f715f2d): a1000 regenerate 3693 → 403 ms, first evaluation 327 → 102 ms, drag 20.7 → 4.7 ms.

Drag step bookkeeping (11-02 07:00, profile of 6000 a1000 drag steps on vibe4: the solve's maths was half a step, the
rest per-step setup): a drag step's two passes share one Driven, Prepared::solve copies the mates once (borrowed when
all are enforced), sizes its output maps up front, keeps no Placement copies, drops a dead relation-values pass. vibe3
a1000 drag: solve 3.5 → 2.5 ms, frame 4.6 → 3.4–3.7 ms (217 → 268–292 fps); a200-live 0.6 ms.

a1000 regenerate (380 ms on vibe3): the bench's stored poses are depth 10's, so the 12 mm side re-solves all 40 stacks
(365 ms) and the 10 mm side finds them solved (26 ms). Of the 365, ~80 % is the stacks' dense eig(JᵀJ) + SVD in
least_change (120 unknowns a stack). The block fix (place a mate tree body by body from the ground) changes the answer
when a parent's free direction could share a child's correction (global least change splits it); asked live which
(REQUESTS 07:20, P1 "which instances absorb a regenerate's change along a mate chain").

Parity (11-02, user: correctness before more speed):
- **Pattern copy conflict = live 1754 (C)** (done): a solve group that can't hold its mates, one of the failing mates on a
  pattern copy, keeps its start poses (nothing moves); the failing mates (the copy's new mate and the seed's grounding
  mate, as live) get "Mate overdefines the assembly.", the copy's follower fails, so the pattern row gets the same message
  and its copies (and all under them) are red with just their name as tooltip; the root badge follows. T1: part (C) of
  a_patterned_flexible_subassembly_follows_its_seed.
- **Redundant mates = live 1764** (done): live ACCEPTS consistent redundant mates (Planar + agreeing Parallel, a chain
  closed by a Parallel, a duplicate Planar: all OK; 1741's "refused" Parallels disagreed with the Planars by their
  default alignment, so the earlier gap note here was wrong and ours already matched). A conflict moves nothing (poses
  as before the mate), puts "Mate overdefines the assembly." on the mates the group's compromise fails, "Mate is not
  consistent." on a Fastened that already failed before the solve (1764 (5)), and the group shows no DOF. Ours did a
  compromise solve, and a disagreeing axial mate wasn't seen at all (residuals ignore the axis sign; the orientation
  pass turned the part over for the later mate, silently breaking the earlier). Now: a group whose solve leaves a mate
  failing, or an axial mate's Z against its alignment, keeps its start poses (from before the orientation pass) and
  flags as above; the pattern-copy rule above is its special case. T1 redundant_mates_are_fine_until_they_disagree
  (cases 1, 3, 4, 4', 5, 6); oracle M1 now reads "overdefines" on both mates as live and keeps B upright (its position is
  live's after the Revolute was added alone first; known list updated).
  1764 (4') (a chain A–B–C closed by a disagreeing Parallel) matches too: Planar 2 and the Parallel red, Planar 1 OK
  (the orientation pass turns C for the Parallel, so only C's Planar disagrees).

- **Mate chain regenerate = live 1763** (done): live keeps every instance whose geometry didn't change where it is
  (C's part moved +10 mm in its own coordinates: only C's transform changes, −10 mm; P, free on its Planar, and D on C
  stay). Ours split it (P slid 3.3 mm). Now a group whose mates form a tree hanging from fixed parts (no relations,
  drags or pattern copies; several mates on one pair allowed) is placed body by body from the ground, each body alone
  against its parent's mates (LM, then least change measured at its connectors over the whole group); anything left
  unheld falls back to the whole-group solve. T1 a_regenerate_moves_only_the_changed_instance_along_a_chain (fails on
  the old code); oracle_asm and the assembly suites unchanged.
  vibe3 regenerate: a200 69 → 34 ms, a200-live 63 → 19 ms (live 13–16 s), a1000 380 → 280 ms (its stacks' redundant
  Parallels close loops, so they aren't trees). New case a1000-live (A1000 with the Planar alone, 900 mates):
  open 6 + 67 + 14 ms, evaluate again 15 ms, regenerate 65 ms, add a mate 28 ms, drag 2.2–2.4 ms a frame (≈ 430 fps),
  insert 15 ms. Live's A1000 is not being timed (lead 11-02: REST budget for parity questions).

- **Placement order generalised** (11-02 08:30): the next body placed is the one most held (residual rows) by mates to
  the fixed and already placed bodies, solved against all of them, so a stack closed by consistent redundant mates
  (a1000's Parallels to the base) goes pin by pin too; a loop one body can't close (a linkage) falls back to the group
  solve. Trees are placed as before. vibe3 a1000 regenerate 280 → 79 ms (3693 ms at the start of the speed work),
  a1000-live 66 ms.
- Tried and dropped (11-02 08:50): the DOF analysis by tree blocks (each joint's 6-column null space carried down its
  subtree; ~a third of a1000's open is the dense eig). A debug cross-check against the dense analysis failed on
  chain_drag_follows_in_few_iterations (19 Revolutes): the joints' finite-difference noise grows with the levers (an
  absolute rank cut over-counts, a relative one drops genuine near-collinear directions). Needs exact joint null spaces
  (central differences or per-mate analytic freedoms) before it can be identical.
- RUST-SM-CUT-VOLUME closed: sheetmetal_std::qa4_s1_enclosure_as_live passes on main (20805.4123 / 61).

Next: each Sub instance's own solve from its
stored positions; whether live keeps a changed PARENT's children in place too (P's sketch moved: does C follow P's
moved top, or does P's transform absorb it? ours: C follows) — ask live if it matters.

## Next, in order
Lead 10-27 order: P0/P1 tickets → PLAN 2.7 speed (met; numbers above, live's beside them) → assembly.md 🔧 rows
(standard content) → mates/assembly workflow gaps (all ✅, UNCONFIRMED all answered 10-27) → standard content.
1. 10-29 night: landed ASM-PATTERN-SUBASSEMBLY a277db14 (dependent patterns move with their seeds, live's confirm modal
   only when there are dependents), SKETCH-F-FIT b7e933c3 (per live request 1583: F in a sketch = F outside, whole canvas,
   no dialog inset; my sketch-only fit and insets were wrong and are gone), ASM-INTERFERENCE-NONE 24aaab66,
   ASM-OVERDEFINED-TEXT 8ed2eeb4, ASM-TRIAD-VALUE-STAYS 9d70ff72, ASM-SELECT-OTHER 681183a9, SKETCH-VDIM-SIDE-ARROWS
   (+ RECT-DIM-GLYPHS (a)), SKETCH-DIM-CURVE-PAIR's warning bubble. SKETCH-MODEL-EDGE-SELECT residual answered (display
   latency: :103 frames cost 0.6–1.1 s GPU under load). An expanded subassembly row now lists its instances, its patterns (copies
   under their caret) and its own Mate features folder (live request 1582). Move to new subassembly / Dissolve subassembly
   take no undo step and turn the assembly's Undo off, as live (store NOT_UNDOABLE_PURGING; live re-enables the older
   history after a dissolve, ours can't safely: whole-tab undo). :103 note: this session xdotool digits didn't reach the app
   (letters did); windowfocus --sync before keys helps with letters.
   Older: open tickets (10-28 18:30): SKETCH-RECT-DIM-GLYPHS (a) vertical edge glyph side, (b) corner Coincident glyphs,
   (c) the top-edge glyphs on the height dimension's arrow: see its note (needs our rectangle's edges to run as live's,
   or a direction rule, and per-corner coincident glyphs for shared points). Fixed 10-28 afternoon: MATE-LIMITS-OPPOSITE-
   SIGN c725f68f, ASM-K-IMPLICIT-MCS 81519593, ASM-XT-ROUNDTRIP-NAMES b09814de, ASM-MC-LIST-ORDER 97c7aae3, dimension
   labels vs glyphs f3c804b9 + 103cd81d, MATE-CONNECTOR-PANEL-HIGHLIGHT c9ed5cdb, SKETCH-START-INFERENCE b7cd7b8c,
   SKETCH-MODEL-EDGE-SELECT 52ff661c, INSTANCE-NAME-STALE ff379982, SKETCH-HINT-OVERLAP + SKETCH-RECT-DROPDOWN-NAME
   df97926c, ASM-TRIAD-MENU + GROUP-ROW-ERROR 19f2d38a, MATE-EXACT-FLIP-MOVER 556b8fcf (UI rule: the first pick moves);
   SKETCH-MENU-ESC not a bug (live behaves as ours, behaviours ~197). Fixed 10-28 evening: SKETCH-USE-PRESELECT 000dfaf4,
   SKETCH-F-FIT f2b44e7a (F in a sketch fits its own curves); SKETCH-MODEL-EDGE-SELECT residual answered: every click
   is right (logged), the :103 frame takes 0.6–1.1 s GPU + 0.3 s present under load, so screenshots lag (see the ticket).
   Next: ASM-PATTERN-SUBASSEMBLY (P2), then the P3s. Fixed 10-28 later: SKETCH-FILLET-EQUAL + the fillet tool reworked to
   live's request-1530 answer (4ca44aa8), RECT-TAB-LIVE (c1b58ff7), SKETCH-ENTER-NO-PLANE (b08165c0, qa verified),
   ASM-MENU-WRAP + ASM-MATE-MENU-SEPARATORS (6444d892), ASM-SELECTED-EDGES-DARK (0d05bd3b). Earlier 10-28:
   SKETCH-FILLET-STUCK-RADIUS twice more, ASM-TOOLSEARCH,
   ASM-CHANGE-CONFIG-ICON, ASM-FILTER-PATTERN-INSTANCES with the filter's help popover). 10-27 late: Fixed this round: dc9db0a4 CONFLICT-COLLAPSE-ON-DRAG (a drag
   solves only the dragged components; the others stay as they are, also faster), 1ca0cf14 ASM-RENAME-ESC +
   J-KEY-NO-EFFECT, 07ead631 MATE-ANGLE-360, 5bc2e08e ORIGIN-COINCIDENT-GLYPH (a point on the Origin is coincident
   with it, no longer merged into it: circles centred on the Origin now carry a Coincident too, as live),
   bf1edd44 SKETCH-PATTERN-GLYPH-HOVER. e52e8015 point menus as live; UI keystroke tests 9.
2. (Tried: writing back solved components between conflict-exclusion retries: no gain measured, dropped.)
3. UI keystroke tests (docs/TESTS.md, part-studio-ui sketch_ui_tests.rs, 13 tests with psui's). Added 10-28: Tab
   in a rectangle's boxes as live (behaviours "skasm (2026-10-28) sketch value boxes: Tab": on a typed value it applies
   nothing and stays, "30" → "30 mm" selected; with nothing typed the other hint is active), `=#len` in a Line box (sketch_in builds on a studio
   prepared first). The value box's keys are bound per test App (entry_key_bindings). The harness
   is part-studio-ui only; assembly-ui could opt in the same way (dev-dependency on noshape-ui-test-support + gpui).
4. Standard content (P3), 10-29: DIN 923 6e3b291b and ISO 7379 7cc04a30 as live's components (raw /features: revolved
   profile = live's face types and volume; shoulderScrew / socketShoulderScrew script features, shoulder end at z = 0 =
   live's "Near"); DIN 7050 / 7051 / 967 tapping screws 345bf2c5 with live's lists, names and table values on the
   generic screw (generic cross recess and cone tip: live's drafted Type H / Z recess sketches and C / F / R tips are in
   dom/skasm-sc-gaps/*_features.json to match next). A size label's "{l}" places the length ("… 6.5 x 10 x M5x0.8",
   "…ST2.2x4.5"; 7049 repeated it before). NAS 1129 blind / thru 5166e6dd (standoff feature; a size's lengthDims /
   lengthNames for the hole depth per length, nameWithoutLength), NASM 45938/4 f85f4917 (flushNut; the Sheet thickness
   select per size, a param's names at the label's "{Sheet thickness}"); all = live's face types and volumes. Already
   there: DIN 6908, ISO 10512. Still missing: NASM 45938 *with* self-locking threads (48 faces, 2 B-spline: our form 10
   body + Revolve 2's conical lip under EDia, two crimp cuts from Plane 1 (Sketch 3: EDia*0.955 flats, EDia/6), two
   fillets; dom/skasm-sc-gaps/NAS_*45938__features.json) and PEM ×52. DIN 967 tapping head as live's dome 895ac8b6
   (87.36 vs live 87.06 mm³); DIN 7051 4 % heavy, 7050 within 0.02 %. Caveat: these three studio defaults are not the
   dialog's (7050 HeadDiameter 5.5 vs the table's 6.3 and tip R; 7051 / 967 Type Z recess), so their recorded volumes
   only roughly compare; match them per feature (head sketch, recess H / Z, tips C / F / R) rather than by volume. PEM plan: /features for all 52 are in dom/skb-sc-fsrules/PEM_PEM_*__features.json (10-29).
   Geometry is NAS 1129's pattern (revolved profile + extruded hex corners; the standoff feature can grow a PEM
   form: serration grooves at CDia/25, CDia/30, 0.98 / 0.9333 CDia). The blocker is naming: pem.json is empty, and
   live's names are part numbers from material + finish. Done 10-29 (live request 1591 gave the dialog): a component's
   materialCodes (material -> type code, its one finish, finish code), label "PEM {type}-<PEM_SIZE>-{l}", Unified
   lengths via lengthNames (LENGTH_CODE); pemStandoff forms 0 SO unthreaded 3b350963, 1 SO threaded 94df73d6, 2 BSO
   blind, 3 TSO 0e1689b3 (STYLE A through / B blind per length, decoded from the body details' face boxes); pemPin
   forms 0 TPS/TP4 38180516 (the Type select names it: param names at "{Type}"), 1 TPXS bff815ca; all = live's face
   types. Generator: /root/skasm-scratch/pemgen.py (build(cat, class, name, form, row_of, dims_of, lenrow_of) +
   add([...]); examples pem_so_threaded.py, pem_bso.py, pem_tso.py, pem_tps.py); /root/skasm-scratch/bd.py prints a
   body-details file's faces (type, radius / angle, z range, area): the fastest way to read live's profile. FH pins
   9660fad9 (pemPin form 2: 12 serration slots + Sketch 3's disc added back; undimensioned radii come from MIDPOINT
   constraints; ~2 s a part, the size sweep is T2). Keep that construction: one combined boolean split the r1 face
   (63 cylinders instead of 27). FH studs b13a2459 (pemPin form 3; the thread-start split by opSplitFace, as the
   askew bolt; a size can offer fewer materials: Size.materials / materials_for, pemgen handles it). Done: SO
   unthreaded / threaded, BSO, TSO, TPS, TPXS, FH pins, FH studs, FH4 ba0d267a, FHL 10f9970c (pemPin form 4: the
   slotted head and the shank built apart and united, so the shank's cylinder runs up each slot as one face as
   live's), TFH 1c68ed37 (form 5, new pemPin param td = T_DIM; steel names "PEM TFH-M3-6 ZI" as live's instance
   names), HFLH 07144d0c (form 6, param st = SHEET_THICKNESS; one material without a table: pemgen build(..., fixed=(material,
   code, finish, finish code))) (Metric + Unified each; the slotted ones' size sweep is the T2
   pem_fh_pin_sizes_build). The sketches in live's /features are pre-solve: read positions from the body details
   (TFH's thread start is from the head's underside, not the sketch's -1.8). Heavy-duty HFH / HF109 / HFG8 (form 7)
   and HFE / THFE (form 8: slots hd / 8; the Type select's options per size via Size.values, THFE's own T_DIM etc.
   as "td@Type=THFE" dims). FS `?:` is left-associative as live's: parenthesise nested ternaries. Heavy-duty 6c106f19.
   FHX 06c8d526 (flushHeadPin form 9: FH studs' serrations and disc, X-Press neck and 25° taper). SGPC (form 10:
   the collar knurled by cutting outside a 28-point star whose tips sit on it; ~350 s for the T2 sweep now). All
   PEM studs done. SGPC 563fc67c. DSO (pemStandoff form 4: SO's profile, round head knurled by knurlCut, the
   helper SGPC uses too) fbd2536c. SOSG (form 5: one groove, hex flats ±X, 32 V notches over the bottom tt by
   knurlCut) 9786920b. All PEM studs and standoffs done. Nuts: shape PemNut (pemNut), the length select is
   "Sheet thickness" (lengthLabel; lengths = its options, lengthNames = shank codes, lengthDims al = A_LEN),
   Size.typeCodes for per-size codes (M5 SS / CLSS); /root/skasm-scratch/pemnut.py build_nut(). S / SS / CLS / CLSS /
   SP done (form 0). Live's nuts' SWEEP faces are the engraved "PEM" (Sketch 3 text, OpenSans-Regular.ttf, one face
   per quadratic segment of Open Sans 1.10): sketch text's Open Sans is now the 1.10 drawing files (was the
   variable font: P 11 + M 6 curved walls instead of live's 7 + 2) 765e618c. CLA (form 0, aluminum only) and SMPS
   (form 1: al / 4.5 shank, 10° cone, chamfered body edges, engraved tl / 100; no Sheet thickness select: a size
   without lengths hides the length row) 3022b70b. SH (form 2: chamfers de / 80, de / 60; "PEM" + "SH", fixed
   letter height mh, tops 0.635 mm inside the chamfer). The T2 sweep pem_fh_pin_sizes_build takes ~10 min now.
   Letter placement per form: read the text entities' centre angle / radius / ascent (see /root/skasm-scratch
   notes in pem_sh.py); widths are live's box width / ascent. SH 044f90a6. S-RT (form 3: PEM + RT, 0.5 mm gaps,
   roundings de / 80 and de / 40), SL (form 4: SH's body, PEM in the middle third, 3 triangular pockets dm / 20
   deep, outer corner on that third's inner circle, inner corners on the bore's), H (form 5: 24 round scallops in
   the ring, one size, no Sheet thickness) 13a6130e. HNL (form 6: H's scallops with the ring's foot at live's literal
   1.47214 mm, flats 0.94 de apart 0.15 tl high, tabs 1.6 al wide hanging 0.8 al, coned inside from 0.35 de to (de
   - al) / 2, "PEM" between) 78557b36. All of live's PEM® catalogue is in. Read-only rows: Size.holeSize (every PEM
   size, HOLE_IN_SHEET), MaterialCode.recommended (nuts and standoffs: RECOMMENDED_MATERIAL, else the crawl's
   default), plain text under Length in the dialog, also in the ops info (hole_size, recommended_sheet_material);
   /root/skasm-scratch/pem_rows.py fills them d6f85faf. NASM 45938 with self-locking threads (nut form
   NUT_CLINCH_LOCKING = 11: the "without" nut's body, only the top edge rounded, a coned skirt cut to two tabs by
   flats and a band, the tab ends rounded de / 60; matches live's B-rep faces and area at 1/4-20 and 3/8-16). Every
   component of live's standard content (ANSI, DIN, ISO, NAS, PEM, SAE crawls) is now in the catalogue 96d4e856.
   SKETCH-RECT-DIM-GLYPHS (b): shared-point Coincident glyphs (sketch_mode SHARED_GLYPH_BASE ids, like the Use
   glyphs; Delete splits the point via Sketch::split_point). Locally, part-studio-ui's
   polygon_sides_typed_before_the_rim_click and section_rotate_handles fail with the shared config dir and pass
   with NOSHAPE_CONFIG_DIR empty (environment, not code). Not shown yet: the read-only Hole size / Recommended sheet material rows.
   Live's studio defaults sometimes differ from the table (NAS 1129 thru minor 0.067 vs 0.0667, NASM 45938/4 ax 0.0225
   vs 0.028125): check the construction at the studio values, then test ours at the table's.
   Earlier: ANSI taper pin crowned ends 10-27.
5. Broken-sketch solve time: a constraint on qa's 438-entity fuzz sketch ~85–125 ms in the sketch crate (9
   exclusion attempts of ~15 ms, the failing component's LM each time). Measured 10-29 (fz211-final.json, a
   Horizontal added): System::build ~0.3 ms, solve 12–19 ms per attempt at 4–9 LM iterations (~2 ms each, one big
   failing component); the residual stays ~1.48e-3 attempt after attempt until the culprit is excluded, then drops
   to 1e-12 (the plain re-solve converges only at attempt 8), so stopping when an exclusion doesn't lower the
   residual breaks sk211 (it needs 3 such attempts). Left as is: what's left is the LM cost of the failing
   component (live takes seconds on these anyway).
6. Assembly fuzz (T2) `random_assembly_edits_never_panic`: `ASM_FUZZ_SEEDS=1..200 ASM_FUZZ_STEPS=120 cargo test -p
   noshape-assembly --lib assembly::tests::random_assembly_edits_never_panic -- --ignored --nocapture`.
7. End-of-day walk (~04:00 UTC) on the newest /root/green/noshape against live: line chain + dimension, value boxes
   (line, rectangle, circle, center rectangle, tangent arc), polygon, mate dialog, Revolute + drag, linear pattern
   direction pick.

Dimension labels vs glyph rows (live 10-28, behaviours "skasm (2026-10-28) dimension labels vs glyph rows" and
"… rectangle glyphs and vertical dimension text"): f3c804b9 a line's value box / dimension 26 px above (away from its
glyph), a corner rectangle's width 42 px below the bottom edge, its height on the other vertical edge 41 px inside;
next commit: those labels follow the edges as they are after the typed value (the height's sat off once the width
changed), dimension text always centred on its line (the "beside" rule is gone), Parallel / Perpendicular glyphs once,
on the first entity. Open: live puts a vertical edge's glyph RIGHT of the rectangle's left edge but left of a lone
vertical line (sketch/28): a rule by the line's direction (glyph right of start→end) fits both, but our rectangle's
edges run CCW (top right→left, left downwards) unlike live's (both horizontals left→right, verticals upwards), so the
rule would put our top glyph outside. (Later: (a) 7d5be491; corner Coincident glyphs 297ea153.)
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

## From core (10-31 18:10): forward-oriented edges and sheet metal (done 10-31 evening, above)
- core tried handing every edge out FORWARD (kernel common.cpp; /root/core-scratch/forward-edges-1031.patch: an
  edge's start / end then always match its parameters, tangents and curve, Parasolid's direction). Everything passed
  but three sheet metal tests, which read the stored edge orientation: model features_c::
  counterbore_head_shows_in_flat, form_adds_and_cuts_and_shows_in_flat ("form direction vs the flat's viewing side")
  and ops e2e::sheet_metal_table_ops (Flange 2 "Collision in sheet metal model."). Not landed. If you get to it,
  apply the patch, look at what those three take from EdgeInfo.start / end or `edges()`' orientation, and land both
  together (or tell core it should stay as it is).

## Requests to the lead
- (11-01 20:30–23:33) main was red on psui's `a_shelled_face_is_highlighted_as_before` (green again at 45424566); the speed stack waited for it.
- (11-01) Document audit D2: done without dx after all. ops' `drawing::old_state` (the drawing update icon's check) gives the out-of-date drawings; dx FYI: history-ui's Create version and the Workspace protection dialog now call it.
- (11-01) Audit G31: the assembly (and Part Studio) Mass and section properties "Show calculation variance" needs an
  integration error estimate per value (live shows "± 1.774e-9 lb", "± 2.100e+0 mm³"). The kernel's mass properties
  don't return one (OCCT BRepGProp gives an error with Eps): core's area; once exposed the panel can show it.
- perf.sh (10-30 ~21:00–22:10): two runs of `regen_profile::sheet_metal_profile` failed to *build* on vibe3 in
  noshape-drawing (first `BrokenOut` missing `offset`, then `TableKind::Bom` has no `default_template`) at commits
  that compiled in their lands (8fd55b58 landed just before). The shared ~/perf/wt looks like it builds stale or mixed
  sources (another agent's run between fetch and build?). Its regen_profile at a88e8c29 also showed the old Hem
  time (1.1 s) where the same commit's own sheet_metal_profile later gave 181 ms: the run may have built a stale tree
  without failing. A third run at 7f7364d3 built and gave the numbers above.
- shared ~/.config/noshape (10-29 04:00): every agent's app runs and the GPUI UI tests read and write the same config
  dir; left_panel_width.txt was 853 (some GUI session dragged the feature list wide), which shrinks the test
  viewports and changed a measured value enough to fail part-studio-ui dimension_box_escape_as_live locally (it passed
  on vibe3; I made the test independent of it). Could the UI test harness (ui-test-support) set NOSHAPE_CONFIG_DIR to
  a temp dir, and the GUI start scripts use per-agent dirs? I now start mine with NOSHAPE_CONFIG_DIR=skasm-scratch/cfg.
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

## From psui (11-01 14:13): Mass and section properties › Show calculation variance
- In your crates (user's request, wiring core's G31 bounds): `assembly::analysis::assembly_mass_variance(result,
  tops, density)` next to `assembly_mass_properties` (core's `regen::measure::mass_property_variance` over the
  parts placed where they sit: live's floors, one bound for every inertia entry), tested in
  `assembly_mass_properties_combine_parts_where_they_sit`; assembly-ui assembly_mass.rs: the checkbox toggles
  `AsmMassUi::variance`, the bounds are cached per selection and result, values read "41032.079 mm³ ± 2.100e+0 mm³"
  (live G31), the inertia cells without a unit, none on an overridden mass.

## From psui (11-01 17:00): sheet metal face order seen through std's manipulators
- std's `addManipulators` now reaches the dialog (audit-part-studio M15). Sheet metal model by Thicken: std's
  addFlipDirectionUpManipulator puts its flip arrow on `qNthElement(qOwnedByBody(sheetBodies, FACE), 0)`; on our
  60 × 40 plate that first face is the x = 0 side (base (0, 20, 1) mm, direction +X), so the arrow lies sideways
  at the edge. Live would have the sheet's own face first (arrow along the normal). If our definition sheet body
  lists its faces in another order than Parasolid's (or has side faces live's has not), that's yours; the T1
  `sheetmetal_std::sheet_metal_model_adds_std_s_flip_manipulator` notes it.
