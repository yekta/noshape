# Handoff: skasm (sketch + assembly), from 2026-10-26

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
- Table letters (10-30): `Context::sm_births`, the std rebuild's first-sight order of definition edges (by end points)
  and rolled faces (by box): an edge a later feature changes is new, as Onshape's transient ids; Bend / Rip `birth`;
  the panel sorts joints by it (live request 1604; T2 joint_births_follow_live_letters).
- Open (10-30 morning): (1) (was: table letters, done above). (2) evaluateQuery order generally: live returns faces in
  creation order (rebuilt faces last; request 1608); the host lists a body only one Extrude made in live's order
  (first side, start cap, end cap, other sides: query.rs `creation_order`, which fixed the Convert's Butt Direction 1),
  other bodies in body order. (3) rect→triangle loft: 51 vs 49 faces (live's
  2 swept faces where three bends meet; their place unknown). (4) Flat view: Copy Flat pattern (live pastes a flat part
  instance into an assembly), the bend lines' own selection and menu, the sketch frame in the flat view. Closed corner: done (live's regen
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
- Speed (ops regen_profile "sheet metal (std)": 200 × 150 plate, 4 flanges, hem): 2.9 s → 0.66 s (dev build) by a
  box test before projecting part faces onto construction pieces (host part_sources: O(faces × pieces) face
  classifications were ~35 %). Profile now flat (std's one-time load, allocation, OCCT adaptors).

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
(none: the branch equals main. Land only with tools/tiers/land.sh, run detached: /root/skasm-scratch/run.sh landsh.log
"tools/tiers/land.sh".)

## Next, in order
Lead 10-27 order: P0/P1 tickets → PLAN 2.7 speed (met; numbers above, live's beside them) → assembly.md 🔧 rows
(standard content) → mates/assembly workflow gaps (all ✅, UNCONFIRMED all answered 10-27) → standard content.
1. 10-29 night: landed ASM-PATTERN-SUBASSEMBLY a277db14 (dependent patterns move with their seeds, live's confirm modal
   only when there are dependents), SKETCH-F-FIT b7e933c3 (per live request 1583: F in a sketch = F outside, whole canvas,
   no dialog inset; my sketch-only fit and insets were wrong and are gone), ASM-INTERFERENCE-NONE 24aaab66,
   ASM-OVERDEFINED-TEXT 8ed2eeb4, ASM-TRIAD-VALUE-STAYS 9d70ff72, ASM-SELECT-OTHER 681183a9, SKETCH-VDIM-SIDE-ARROWS
   (+ RECT-DIM-GLYPHS (a)), SKETCH-DIM-CURVE-PAIR's warning bubble. SKETCH-MODEL-EDGE-SELECT residual answered (display
   latency: :103 frames cost 0.6–1.1 s GPU under load). An expanded subassembly row now lists its instances, its patterns (copies
   under their caret) and its own Mate features folder (live request 1582). Gap left: after Move to new subassembly
   live's Undo is disabled (ours undoes it). :103 note: this session xdotool digits didn't reach the app
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

## Requests to the lead
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
