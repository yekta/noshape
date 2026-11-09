# Blends (agent "blend")

Area (lead 11-07): fillets, chamfers and face blends: noshape's own blends (crates/brep/src/blend*, rust_blend.cpp,
ticket BLEND-RUST), OCCT's blend route and its patches, and the blend features in crates/model-core
(regen/features/detail.rs `fillet` / `chamfer` / `variable_fillet`, blend_section.rs, faceBlend). core keeps the rest
and reviews blend changes on request. History before 11-07: docs/handoff/core.md ("noshape's own blends",
"Mixed-radius corners", the 11-07 states).
Worktree /root/projects/noshape-wt/blend, builds on vibe5 (`NOSHAPE_RB_HOST=vibe5 tools/tiers/rb.sh ...`), scratch
/root/blend-scratch. Any change in crates/brep passes bool's gate first (handoff/bool.md: every corpus and parity56 at
0 differences from OCCT).

Parity runs: `NOSHAPE_RB_HOST=vibe5 tools/tiers/rb.sh env NOSHAPE_PARITY_MODELS=/home/ngyekta/parity/models
NOSHAPE_PARITY_ONLY=<docs> cargo test --release -p noshape-ops --test all parity::parity_models -- --ignored --nocapture`.

## From dx (2026-11-09 11:09 UTC, lead: blend paused till Monday): c2-fan-blade-sweep's Chamfer 2, in progress
- Cause: the thread's band (core's keep mark on its edge) survived OCCT's chamfer (91 faces, the mark on) and then
  `merge_full_rounds` (finish_blend_faces) joined the band and the boss next to it: two coaxial cylinders of one
  radius, its rule for a full round's halves. It now leaves a kept edge alone. And the fillet / variable fillet /
  chamfer copies of the input (`BRepBuilderAPI_Copy`) lost every keep mark: carried now (`carry_kept_marks`), as the
  Booleans' copies do.
- Source studio fb207f79: 18 -> 22/22 (to the end). Derived 2 (other source 8b633129, the STEP import) built too
  (parity translator: qEverything(BODY)'s default bodies aren't parts) and the blade's last edge joined (dx's
  import code). c2-fan-blade-sweep 0 -> 23/53; next: feature 5 Extrude 1 ("klammer" 7/15/10, live 6/12/8).
- Suite checks running (state below when done).

## From core (2026-11-09 02:48 UTC): c2-hunting-knife now stops at Fillet 2 (feature 14)
- With core's guided loft fix (Loft 3 as live's face, the blade 4983.62 mm³ in live's band), the knife runs 11 -> 14/52.
  Next difference: feature 14 "Fillet 2": ours ERROR "Failed to fillet selections.", live OK; "4.5\" BLADE" 16/43/28
  faces/edges/vertices before it, live 17/46/30 after. Run it alone: NOSHAPE_PARITY_ONLY=c2-hunting-knife parity_models
  (38 features after it): yours.

## From core (2026-11-09 00:54 UTC, lead's call): c2-fan-blade-sweep's source studio now stops at Chamfer 2
- With core's keep mark (intentional splits survive later unifies), the source studio
  (NOSHAPE_PARITY_MODELS=$HOME/parity/models/c2-fan-blade-sweep/derived NOSHAPE_PARITY_ONLY=fb207f797bca6b31509dba54)
  matches through Extrude 7 and Extrude 8: 18/22. Next difference: Chamfer 2, 90 vs live's 91 faces on "Poti2":
  our chamfer drops the 0.15 mm band External thread 1's Split face made on the boss (edge circle at x -13.8, r 3.4;
  marked "keep" since this change: `kept_edge()` in common.cpp, carried through copies with `carry_kept_marks`). That
  one face then carries into Derived 1 (82 vs 83), so the document stays 0/53 on it: yours now.

## From core (2026-11-08 20:05 UTC, lead's suite ranking): c2-fan-blade-sweep's 53 lost features start at a chamfer
- c2-fan-blade-sweep (0/53) fails at Derived 1 because its source studio fb207f797bca6b31509dba54 fails at feature 8
  "Chamfer 1": ERROR "Failed to chamfer selections.", live OK (Fan-Connector 32/78/52 faces/edges/vertices, live
  48/110/68). Run it alone: NOSHAPE_PARITY_MODELS=$HOME/parity/models/c2-fan-blade-sweep/derived
  NOSHAPE_PARITY_ONLY=fb207f797bca6b31509dba54. The biggest cascade in the suite: yours.
- Also yours now (lead 20:07): c2-rings-loft (12/32, 20 lost) fails at Circular pattern 1 on area alone: "Extruded
  ring" 1192.40 mm², live 1193.07 (-0.056 %), with live's counts (273/762/508); the shortfall is the ring's blend
  faces (core 11-07 19:56, core.md "seam pieces shown as one edge").

## Feature audit (lead 11-08 ~11:00): Fillet / Chamfer / Face blend options, tickets in TICKETS.md "Blends (blend)"
- Landed 11-08: 823698ce (face blend empty options, partial plane end, drafted-rib full round, CHAMFER-METHOD + DA on
  a round face (`blend_section::chamfer_offsets`), FACEBLEND-HOLD-TANGENT (`Sides.hold`, `hold_ranges`), stopped
  chamfer setbacks, face blend Adjacent / Custom (`grow_side`), Detached, Long / No trim error); 660f7ddd (tapered-rib
  cone `tapered_round_tool`, chamfer angle >= 90°, FACEBLEND-SURFACES `sheet_blend` (live 2321's alignment rule),
  detached No trim `run_on_round`); 66bf6d57 (blend_direct skips sections of rounds tangent along a shared circle:
  cf-snap-hook Fillet 3 91 -> 54 ms). Queued 116f47c6: variable fillet warnings (live 2325 A) and constant case.
- Every change ran the before/after suite on vibe5 (audit2..10); flagged "slower" documents re-run alternately
  were noise each time.
- opChamfer: routing it through the Chamfer evaluator broke cf-snap-hook; "Opposite off = the edge's second face"
  (live 2325 D) broke cf-spur-gear: both reverted; asked live 16:34 for the rule (caps / sides / version).
- Parked: FACEBLEND-HOLD-RIM (c2-variable-fillet-faceblend's Face blend 1, the only blend first-difference in the
  suite), branch `hold-rim-wip` (patch /root/blend-scratch/patches/hold-rim-wip.patch, ZZ_HOLD_RIM / ZZ_ONE_BY_ONE
  debug env). Radii match live (13.858 at (50, 25): live's JPC down to z 1.14) and the tool removes 5897 mm³ (live
  5898.7). One closed loft: 5 solids. Pieces split where another hold line takes over (5 lofts) + sections reaching
  0.1 r past the hold line into the pocket (`rim_section_face`): valid, main solid 136951.04 (live 136943.82) but 32
  faces and two stray solids; cut one by one, piece 3 (round the y 25..75 end, r up to 13.9) breaks it: its loft's
  box is ±230 mm. Next: that piece's sections (twist / a jump in r at the chain's vertex at (50, 25)).
  Tried 15:10: sections all started on side 1 (the frame swapped where the edge lists the wall first): the lofts
  come out invalid (7 faces each), still 3 solids (patch hold-rim-wip2.patch). Stopped there (time box).
  17:10 (branch hold-rim-wip3, patch hold-rim-wip3.patch): piece 3's sections are smooth (r 14.8 -> 2.5 -> 14.8,
  frame consistent; the ±230 mm box was the loose B-spline box). New cause: the tool's sides lie on the walls, and a
  loft's side only approximates the curved, filleted end walls. Lifting both sides off the part (`lifted_section_face`,
  into air for a cut) gives one valid part with the closed loft (2233's T1 still passes), but 32 faces (live 20) and
  only 4,858 mm³ cut (live 5,899): the rim top (2,859 mm²) stays though it lies inside the tool. With one loft per hold
  line the pieces' end sections touch: 2 invalid solids. No live capture needed: our radii and the tool's volume match.
  18:40 direct-build route (branch held-direct-wip, patch hold-rim-direct.patch: held lofts handed to
  k::blend_direct before the cut; blend_direct keeps a face whole whose cuts all lie on its boundary): the plain loft's
  wall side, a B-spline only near the curved end wall (a cylinder), gives 18 spurious sections with it: "a part face
  that wouldn't split"; the lifted loft has no face on the part: "a tool with no blend face or none on the part".
  Next idea: a hold-line blend at the rim's 90° is a rolling ball whose radius is the hold line's distance, r(t):
  OCCT's variable fillet over the whole tangent loop (one law per contour: BRepFilletAPI_MakeFillet::SetRadius(law,
  contour)) builds it with its tangency natively; our kernel has single-edge laws only (fillet_variable_checked).
  19:30 tried it (branch hold-rim-varlaw, patch hold-rim-varlaw.patch: op_fillet_variable_points places (point,
  radius) samples on OCCT's contour by RelativeAbscissa): OCCT fails at the exact radii (and at 1 - 1e-5); at 0.999
  it builds one valid part, 23 faces, 140,239 mm³ (live 136,944): a ball rolling with a fast-changing radius touches
  elsewhere than r from the edge, so it takes ~2,600 mm³ where live's circular sections take 5,899 (our loft tool:
  5,897). So the section loft is right; what's missing is placing it.
  What would fit: in blend_direct, a tool face that only approximates its part face (a loft's side on a curved wall)
  treated as lying on it: the tool's touch edge (t1 / t2 curves) projected onto that part face as the cut there,
  instead of sectioning the two surfaces (which gives the 18 spurious curves). A day's work, not an hour's.
- Asked live 12:50: surface walls' side directions (2306 D's apart case contradicts the touching rule).

## Cascades (lead 11-08 ~20:30)
- c2-fan-blade-sweep: Derived 1's source (derived/fb207f797…) failed at Chamfer 1 (two distances 0.4 / 0.2 on four
  0.7 mm pins' tops): our Distance 1 face was the top on three pins (the feature-wide first side). Fixed (b3f3df84e,
  `face_order_in`: each profile loop's first side): the source 8 -> 14/22; Fan-Connector as live. The document still
  stops at Derived 1 on part "Poti2" (82 vs 83 faces), from the source's Extrude 7 (feature 15): core's.
  Run the source alone: models dir with a link to it (vibe5 /home/ngyekta/blend-scratch/models/fan-source).
- c2-rings-loft: the 0.06 % area at Circular pattern 1 starts at Chamfer 1: ours 705.288 mm³ / 914.52 mm² against live
  705.853 / 914.99 (inside its band; Shell / Extrude 3 / the pattern carry it, the pattern's narrower band fails).
  Faces: our chamfer faces 10.044 mm² (live 9.8228), y to 1.81 (live 1.89); the side strips 8.854 (live 9.17, ours
  cut to y ±2.49, live ±2.53): the run-on past the arc's ends goes further than live's. Asked live (REQUESTS 21:06)
  for the chamfer face's surface and edges.
  Live answered (2384: blend/q2384/chamfer-run-on.json): JRC is one ruled B-spline (degree 1 x 3, 9.8524 mm²) running
  smoothly past the arc's end, trimmed by the strip plane JRy along KRpB to the strip's ellipse edge at y ±2.5349 (ours
  ±2.49). So live's run-on is its chamfer surface continued, ours the rim's chamfer continued along the rim. Not built
  yet (quota): next, compare KRpB's 17 points with our run-on's edge, then build the run-on as the chamfer surface
  extended along its own ruling (blend_section ring_blend RimBlend::Chamfer).
- opChamfer distance-and-angle side: live 2336's rule (off = cap) breaks cf-spur-gear's gear 8; reverted, see
  TICKETS CHAMFER-FS-OPCHAMFER (patch opchamfer-cap-rule.patch).

## Speed survey (lead 11-08 19:40: audit11's blend features slower than live, by ms over live)
Ours over live, from each document's slowest features and live.jsonl's per_feature_ms (only documents matching up to
the feature count):
- learning-center-michael Fillet 2 640 / 46: fillet_by_tools' need_ball cut, OCCT 314 face-face pairs: bool's (bool.md
  "From blend 11-07 23:10"); blend_direct can't take its ball / concave-corner tools (their faces border other
  tools, not the part: "a tool with no blend face or none on the part").
- c2-rings-loft Fillet 1 392 / 23, Chamfer 1 345 / 30: the same kind (ring_blend's cut), bool's.
- cf-part-draft Part Draft 2 368 / 39: op_defeature (OCCT defeaturing) 254 ms inside the custom feature, not a blend.
- c2-hunting-knife Chamfer 1 182 / 19, Fillet 3: after its first difference (Loft 3): not comparable.
- c2-tpd-fillet-variable Fillet 1 83 / 29: the Rust Boolean's one-by-one 35 ms + sections_ahead 46 ms, bool's.
- cf-snap-hook Fillet 3 54 / 7 (66bf6d57: was 91), Fillet 6 121 / 8 (blend_direct sections, cone x torus pairs
  tangent at the chain's joints), Fillet 5 118 / 6 (Rust Boolean, bool's).
- motor-mount-jack Fillet 2 39 / 3: the full round's tool now one revolve (4d21d6419); the cut itself ~35-45 ms.

## From core (11-07 19:56 UTC): 2e49088a9 regresses learning-center-michael and cf-lighten
- Corpus on vibe5, my tree rebased onto main: learning-center-michael 7 -> 5 (Fillet 2 now fails, "Failed to fillet
  selections.", after 22-52 s; live OK in 46 ms) and cf-lighten 5 -> 4 (Lighten 1's area 48549.85 mm², live
  48550.21). cf-surface-pattern 34 -> 36 in the same run.
- With 2e49088a9 undone in my tree (and nothing else changed), learning-center-michael matches to the end again
  (Fillet 2 662 ms). Repro: `NOSHAPE_PARITY_ONLY=learning-center-michael,cf-lighten` parity_models.

## List (lead 11-07 18:00)
1. cf-surface-pattern's Fillet 1 (side rounds against the slanted top extended).
2. c2-tpd-fillet-variable's variable Fillet 1: exact from its radius law, and fast (1.7 s, live 44 ms).
3. RING-BLEND-SPEED: c2-rings-loft's Fillet 1 370-410 ms vs live 23 ms (OCCT's cut of the two rounds).
4. Face blend tangent hold lines, once live answers REQUESTS core 17:27.
5. Blend differences in the parity suites / SCORE.md, most features first (cf-snap-hook's Fillet 4 is skasm's).

## State

**11-07 18:50 UTC: cf-surface-pattern's Fillet 1 as live (34 -> 36 of 39).**
- Live 2227: the round is the ellipse's alone (JtS top x nose cylinder JnW); a picked face's edges aren't propagated
  (live 2202: "with a face picked, the result doesn't depend on propagation"). Past each end of the ellipse the round
  runs straight on between the side plane (the cylinder's tangent plane there) and the top's plane (cylinders R 2.54,
  JuW / Jua) until the sweep below trims it, 1.617 mm up the side edge.
- `blend_section::run_on_rim_blend`: for a selection that stops where its plane x cylinder arcs run on tangentially
  into edges left out: a wedge (the slab under the plane with the arc and its two end tangent planes), its three top
  edges rounded by OCCT, the wedge minus its round cut from the body. G7's rule ("Adjoining edge not filleted." for a
  curved edge left open) now only where the tangent neighbour shares one of the edge's faces (G7's arc and lines share
  the top; Part 5's ellipse and side edges share none).
- 10/22/14, live's. T1 model features_c::a_rounded_arc_runs_straight_on_past_its_ends_as_live (Part 5 as imported,
  data/surface-pattern-part5.brep: face picked with propagation, and the edge alone without).
- The document's time isn't the fillet's (44 ms, live 11): Surface Pattern 3 7.7 s (live 537 ms), the next difference
  (11 parts, live 6): custom feature, not a blend.

**11-07 20:20 UTC: two P0 regressions (lead), both fixed in one change.**
- cf-snap-hook's Fillet 5 (85 faces into Shell 1, live 30; Shell ~27 s): not core's run-on change. skasm's 5baedbe49
  made Fillet 4 live's (each boss rim a torus R5 from its cap to the floor); Fillet 5 (R1, 4 concave circles where
  those tori meet the floor) then went through OCCT's fillet, whose volume is live's (401925.854 vs 401925.861 mm³)
  but whose round is 16 approximate B-spline pieces an edge, and Shell 1 crawled over them. `coaxial_round`: a whole
  circle between faces of revolution about it (plane across the axis, coaxial cylinder / cone / torus / sphere, a
  torus or sphere among them) rounds as a torus: the 2D round of the two faces' sections revolved, the region added
  (concave) or cut (convex). Its fuse runs at the working scale with Glue Shift (new `BooleanOptions::scaled`):
  306 ms in metres, 38 ms so; Fillet 5 68 ms (live 6; frame / containment checks ~7 ms an edge left), Shell 1
  535 ms (it was 5.8 s before Fillet 4 changed). Snap-hook 9 -> 11 of 18; next: Plane 1 "Point-Plane requires a
  point and a plane." (not a blend). T1 model features_c::rounds_where_boss_tori_meet_the_floor_are_tori_as_live
  (data/snap-hook-case.brep: the case as Fillet 4 leaves it).
- learning-center-michael's Fillet 2 (failed after 22-52 s, 7 -> 5): my 2e49088a9's "a picked face's edges aren't
  propagated". The translator gives Fillet 2's eight edge queries as seven edges and the cap face (one query maps to
  a Face), so the face's edges stopped propagating, the selection "stopped" and went to the section route. Now only a
  selection of faces alone isn't propagated (live 2202's case); with edges picked too, all are, as before.

**11-07 21:40 UTC: c2-tpd-fillet-variable's Fillet 1 156 -> 79 ms (live 29), still to the end.**
- core's exact round (44368ecdd) ran after OCCT's variable fillet failed (~70 ms). `variable_round_overflows`: a
  round wider than a face beside it (the law's largest Bézier value x tan(half the normals' angle) past the face's
  far side) goes to `variable_round` first, OCCT's only if that fails.
- Left: the cut, ~70 ms in noshape's Boolean, half of it a one-at-a-time attempt that fails (the rounds meet at the
  corners): asked bool (bool.md "From blend 21:34").
- Suite before / after on vibe5: 694 = 694 features; the four "slower" documents have no variable fillet (Mirror /
  Extrude / pattern times: load).

**11-07 22:15 UTC: RING-BLEND-SPEED, what was tried (nothing landed).** c2-rings-loft's Fillet 1 ~340 ms (vibe5):
the tools (two wedges, OCCT's lone-rim fillets, wedge minus round) ~25 ms, the final cut ~307 ms in OCCT
(boolean.build; 212 ms of it "Edge-Face intersection", 152 calls: the rounds' B-spline contact edges lying on the
plane and the bore, and the rims on the tools' strips). Tried:
- The rims are mirror images about x = 4 (square to the bore's axis), their rounds meeting on that plane: each region
  trimmed to its side (`mirror_pair`) so no blend-blend meeting curve is left to find. Same topology and volume, but the
  cut stays ~309 ms (and the trims cost 60): the time isn't the meeting curve, it's the edges lying on the faces.
- The cut at the working scale (`BooleanOptions::scaled`): 307 -> 156 ms, but Shell 1 then fails after 25 s on its
  result (12 -> 9 features). Not usable as is; fuzzy 1e-7 x size changes nothing.
- Next levers, in order of size: (a) the tool built with exact contact curves (both are ellipses: the spine, an
  ellipse where the offset plane meets the offset cylinder, projected onto the plane and radially onto the cylinder),
  only the blend face approximated: OCCT's edge-face work on exact conics is cheap; (b) no Boolean: the rims' faces
  re-bounded by the contact curves and the blend faces inserted, the two rounds' meeting curve and run-outs computed
  from their geometry (the lead's "built directly", crates/brep, gated). (a) is a day, (b) several.

**11-07 22:30 UTC: RING-BLEND-SPEED parked (lead: poor value for one 340 ms fillet while quota-limited).** Numbers and
the two designs ((a) exact contact ellipses in the tool, ~1 day, Fillet 1 to ~130 ms; (b) no Boolean, the rounds,
their meeting curve and run-outs built in crates/brep, several days, live's speed) are in the 22:15 entry above. It
comes back if the fillet speed table shows OCCT's final cut on exact contact edges dominating many fillets.
cf-setback-fillet's SETBACK-FILL-PATCH is blend's from now (lead 11-07 22:30; skasm told in skasm.md).

**11-07 22:40 UTC: fillet speed table (lead's ask).** Every fillet / chamfer of the model suite with a live time, one
run on vibe5 (main 11-07 ~22:00; `NOSHAPE_PARITY_ONLY=<all>` prints per-feature times; script
/root/blend-scratch/bin/blend-table.py, full table /root/blend-scratch/blend-table.txt): 91 features, ours 4584 ms
against live's 2031 in all; 22 slower than live, 69 level or faster. The worst, with what their time is (kernel
profile):

| document › feature | ours | live | where the time goes |
|---|---|---|---|
| cf-snap-hook › Fillet 6 | 187 | 8 | OCCT's fillet: the as-is try fails (14 ms), the scaled retry builds (175) |
| c2-rings-loft › Fillet 1 | 340-488 | 23 | ring_blend's cut: OCCT's edge-face work on the rounds' contact edges (RING-BLEND-SPEED) |
| learning-center-michael › Fillet 2 | 629-694 | 46 | OCCT's fillet falls back to `by_tools` (685; 363 of it face-face intersections) |
| motor-mount-jack › Fillet 2 | 39 | 3 | an OCCT cut (17) and properties |
| c2-rings-loft › Chamfer 1 | 340 | 30 | ring_blend's cut, as Fillet 1 |
| cf-snap-hook › Fillet 5 | 64 | 6 | coaxial_round: its checks (~7 ms an edge) and the glued fuse (38) |
| c2-impeller-loft › Fillet 1 | 374 | 38 | not the fillet (87): resolving its live-id query by geometry before it runs (eval.rs, ~114 `k::distance` against the body, 234 ms): the translator's |
| c2-impeller-loft › Fillet 4 | 541 | 102 | OCCT's fillet falls back to `past_end_faces` (defeaturing, 500) |
| c2-impeller-loft › Fillet Ansaugflansch | 940 | 321 | (not profiled yet) |

So the final cut on exact contact edges (design (b)) is only rings-loft's two features (~700 ms). The bigger share is
OCCT's own fillet falling into its slow fallbacks (the scaled retry, by_tools, past_end_faces: ~1.4 s over three
features): those are next, the slowest first (michael's Fillet 2).

**11-07 23:10 UTC: RABC parked; the speed table made fair; the fallback speeds.**
- SETBACK-FILL-PATCH's RABC parked (lead): one patch, a numerical fit, no quick fix. Reproduced with skasm's
  /root/skasm-scratch/corner/proto.py (RABC 35.66 mm², live 34.464; JcC / JuC within 0.08). Live's RABC is G1 along all
  six edges (0.09° at most), as the other two, so it isn't a position-only edge. Untried idea: Parasolid fixing the
  cross-boundary tangents' magnitudes (from the supports), not only their directions, which a free least-bending fit
  can't show on the two that match.
- Only blends before each document's first difference compare with live (after it the input already differs): 67
  features, ours 2221 ms against live's 773. snap-hook's Fillet 6 (187 ms) fails in ours on a wrong input (Plane 1
  fails upstream, Sweep 1 builds nothing), and the three c2-impeller-loft fillets run after its Loft Gehäuse
  differs: neither is a fair speed case. The fair top: c2-rings-loft Fillet 1 + Chamfer 1 828 ms (live 53),
  learning-center-michael Fillet 2 629 ms (live 46): both a cut of the part by blend-region tools lying on its faces
  (66 % of the comparable blend time). michael's cut: `fillet_by_tools`, declined by noshape's Boolean ("a non-manifold
  result"), OCCT ~600 ms: what the Boolean would need is in bool.md ("From blend 23:10").
- Kernel timers added for the record (not landed): blend.checked.build / .valid / .shrunk, blend.scaled.finish.

## DOCK-FILLET (P1, lead 11-08): AppleWatchDock's "Fis Gecidi R" folds Govde
User data /root/lead-scratch/user-docs/AppleWatchDock.noshape (never in the repo; copy in /root/blend-scratch/watchdock).
Tab "A - Yumusak Kutu", feature index 69 "Fis Gecidi R" (fillet on the USB-C plug passage, after 68 "USB-C Fis Gecidi"
extrude). Probe (not to land): crates/ops/tests/zz_dock_probe.rs (`DOCK=<copy> FROM= TO= NEAR=1 DUMP=<dir>`), and
`NOSHAPE_SI_DUMP=1` in kernel common.cpp `self_intersects` prints the faulty pairs. 01:00: six faulty pairs, mirrored
at x = ±5.5, y ≈ 13.3, z ≈ 9.8 (one shape of each a vertex-sized 0.2 mm box).
- 01:35 UTC: cause and fix. The pairs are vertices with 0.53 / 0.52 / 0.1 mm tolerance overlapping: OCCT's as-is
  fillet puts the corners where the wall underside's round (r 13.95 arc at z 9.9) meets the 0.1 mm step edge's round
  0.5 mm off; tighten_new_tolerances can't tighten them (keeps OCCT's), BRepCheck passes, BOP's check fails. Each
  round alone is fine; together OCCT's corner is wrong. Fix (kernel features.cpp `scaled_blend`): when the as-is
  corners are far off (tighten returns inf) and a loose vertex's ball reaches another vertex
  (`loose_vertices_overlap`), the working-scale build instead (valid, no SI). Dock: Part 1 valid, not SI, volume
  25097.046 mm³ (broken one 25097.085); Fis Gecidi R 102 -> 182 ms (two builds). T1 model
  features_c::rounds_over_a_tenth_mm_step_leave_a_valid_body (stand-in built with the kernel, no user data).
  Live asked (REQUESTS blend 01:21, same stand-in). Left: the subset "outer arc + 0.1 step + outer side" (inner arc
  left out) still SI after the scaled build (a 0.1 mm vertex 0.1 from another); probe /root/blend-scratch/dock/.

## SNAP-FILLET6 (lead 11-08, before BLEND-DIRECT): cf-snap-hook's Fillet 6
R10 on the case's lip, a closed G1 loop of eight concave edges (arcs between coaxial drafted cones, one line between
planes). OCCT's round: a 0.1 mm sliver face where each two arcs meet (90/178/92, live 84/160/80). Fix: when OCCT's
fillet has a sliver (a new face narrower than r / 20 across its size), `fillet_by_tools` without a mitre's ball (a
tangent chain of lines between planes and arcs between coaxial planes / cylinders / cones, all convex or all concave):
exact rounds (tori, the line's cylinder), the tools fused (concave) or cut (convex) in one Glue Shift Boolean. The
document 14 -> 18/18 (regen 4770 -> 3922 ms: Snap Hooks on the right input); Fillet 6 97 -> ~300 ms (live 8: OCCT's
glued Boolean of the tori ~190 ms, the slivered OCCT attempt ~100 first). T1 features_c::a_tangent_loop_of_concave_
rounds_has_no_slivers_as_live (data/snap-hook-lip.brep, Fillet 6's input).
11-08 06:00: such a chain (tangent arcs changing support) goes to the exact rounds first, OCCT's slivered attempt
skipped: Fillet 6 300 -> ~200 ms (the rest is OCCT's glued Boolean of the tori; live 8).
11-08 08:30: the chain's rounds now go through the direct build (blend_direct.cpp, extended: cones / tori by sampled
coincidence, tools taken or added by a point beside their strips, a piece kept by a point just off it, strips inside
the material output, section lines along a face's edge skipped, sections only with the supports' neighbours), the glued
Boolean the fallback: Fillet 6 ~200 -> ~110 ms. rings-loft's ring_blend doesn't use it yet (Shell 1, see BLEND-DIRECT).
Landed 09:10 as 0cb8ba5d. rings-loft through it (branch ring-direct-wip): Fillet 1 ~110 ms, Chamfer 1 ~275 ms, but
Shell 1 fails on the in-memory result and works after a BRep write/read (bool.md "From blend 05:06", update 09:54).
DOCK-CORNER through it (branch blend-direct-wip): the corner tools (trimmed arc, common, vertical) overlap and the 2D
splits don't close yet; next step there: fuse each corner's three small tools into one before the direct build.
11-08 11:10, branch dock-corner2 (WIP, not landed): each corner's tools fused into one (OCCT, ~10 ms a corner), the
direct build then makes the stand-in exactly as live (23/61/40, 8480.50537 mm³; T1 asserts it on the branch). Fixes on
the way: part vertices seeded so cuts ending at a face's corner share its vertex; a section stopping a hair short of
either face's edge (IntSS near a tangency) run on to it; overlaps counted only with real length (two pieces of one
curve touch); a free end run on only for arcs and only where a probe past it is air. In the user's document Fis
Gecidi R now builds directly (Part 1 valid, not self-intersecting, 227 faces) and so do USB Pencere R and Kapak Alt R,
but each is slower than main's OCCT route: 275 / 225 / 517 ms against 60 / 24 / 328 (classifications against the
tools ~0.6 ms each, 50-140 a fillet; sections ~0.5 ms each, 35-100 a fillet). Not landable as a speed regression; the
next levers: classify by 2D position on the tools' strip faces instead of 3D solid classifiers, and skip sections
whose faces only touch along an edge (end caps against the supports' neighbours).
: blends built with no cut (lead 11-07 23:20: design (b))
Why: of the blend time the suite compares with live (67 features before each document's first difference: ours 2221
ms, live 773), two thirds is OCCT cutting the part by blend-region tools whose faces lie on its faces (rings-loft's
Fillet 1 + Chamfer 1 828 ms, live 53; michael's Fillet 2 629 ms, live 46). The rounds themselves are cheap (OCCT's
lone-rim fillet ~9 ms); the cut has to rediscover what we know: which face each touch curve lies on.

Design (stages, each through bool's gate and the before / after suite):
1. **Rims: a plane x a cylinder, round or chamfer** (ring_blend's cases). Per rim, exactly: the spine (where the
   offset plane meets the offset cylinder: an ellipse), the touch curves (the spine moved onto the plane and radially
   onto the cylinder: both ellipses, sharing the spine's angle as parameter), the blend (a chamfer: the ruled surface
   between the touch ellipses, exact as a rational NURBS in that shared parameter; a round: the ball's pipe round the
   spine, a NURBS fitted to it within 1e-7 x size). Trims, all between surfaces we know: a touch curve leaving its
   face (where it crosses that face's other edges: a curve on a plane or cylinder against edges, 2D), two rims' blends
   meeting (one curve; on the mirror plane when the rims mirror each other, else marched in the two rims' angles), a
   blend running out onto another face (marched on the blend against that plane / cylinder). Then the faces: the
   plane and cylinder re-bounded by the touch curves (2D splits of their parameter domains), the blends bounded by
   their curves, the run-out faces split by theirs; sewn, every new edge with exact pcurves. Built in crates/brep
   (geometry + topology in its arenas) and handed to OCCT shapes by the blend bridge (rust_blend.cpp), names and
   history as ring_blend's.
2. **michael's Fillet 2** (fillet_by_tools' cases: lines between planes, arcs between a plane and the cylinder they
   bound, balls at mitres): the same scheme with cylinders, tori and sphere octants as the blends (all exact), the
   mitres where two rounds' axes cross.
Each stage falls back to today's route where its checks fail (a budget only as a hang guard). T1s: rings-loft's ring
(data/rings-loft-ring.brep) against live's numbers, and michael's part.

Stage 1, how (11-08 01:50): the cost is OCCT rediscovering the touch curves on the plane and the bore (the tool's faces
lie on the part's): no Boolean finds them faster, so they're imprinted, not found. Per rim, OCCT's lone-rim blend on
its wedge as now (~9 ms) gives the blend face B with exact edges on the plane and the cylinder. Then, in the kernel
(C++, one call): (1) the intersections that are real: B against each other face of the part it crosses (the outside
the rounds run out onto) and B against the other rims' blends, each a two-face section with pcurves; (2) every
face that gets a curve split in 2D by it (the touch curves on the plane / bore, the run-out curves on the outside, the
section curves on B: BRepFeat_SplitShape, no surface work); (3) the pieces kept by one point each: a part face's piece
outside the removed region (wedge minus its blend), a B piece inside the part; (4) the kept faces sewn into the
solid, history Modified for split faces, Generated from the rim edge for B. Checks: one valid solid whose volume is the part's less
the tools'; else ring_blend's cut as today. Target rings-loft Fillet 1 + Chamfer 1 under ~60 ms.

Stage 1 progress (11-08 02:33, local WIP commit, not landed): kernel `op_blend_direct` (cpp/blend_direct.cpp, Rust
`k::blend_direct`, tried first in `ring_blend`, its cut the fallback). Rings-loft's Fillet 1 builds directly: 9 faces,
719.382 mm³ (live 719.453), T1 ring test passes; Fillet 1 345 -> 114 ms (direct 79: sections 38, tool classifications
16, splits 7, sewing 2). Lessons: BRepAlgoAPI_Section is as slow as the cut (44 ms a pair): GeomInt_IntSS on the faces'
UV boxes instead, each curve trimmed in both faces' 2D domains (a run-out touches the part face's boundary tangentially,
the blend's it crosses square); FClass2d wraps periodic points by default (Perform(uv, false)); a seam is split once and
used both ways; cut edges need SameParameter cleared before BRepLib::SameParameter; two rounds tangent to each other
where they meet the bore: their section stops ~7 µm short, so it's extended to the touch curves' crossing
(GeomLib::ExtendCurveToPoint). Next: Chamfer 1 (free edges after sewing, notch region), then speed (sections, the
tools' classifier), then the suite and bool's gate (no crates/brep change so far).

**11-08 04:06 UTC: BLEND-DIRECT parked (lead: cf-snap-hook Fillet 6 first).** Local WIP commit only (branch state in
/root/projects/noshape-wt/blend, "blend: WIP blends built directly"); stage-2 attempt in
/root/blend-scratch/stage2-by-tools.patch.
- Rings-loft (vibe5): Fillet 1 345 -> ~100 ms and Chamfer 1 276 -> ~195 ms, both built directly, T1 ring test passes
  with live's counts (Fillet 1 719.385 mm³, live 719.453; Chamfer 1 705.021, live 705.853).
- Blocker: the suite's rings-loft drops 12 -> 9: Shell 1 fails ("Could not shell part with selections.", 13-17 s) on
  the direct Fillet 1 result (the chamfer's direct result is fine: with NOSHAPE_BLEND_DIRECT=chamfer the document stays
  12/32). Same faces, edges, lengths and vertices (to 0.05 µm) as the cut's result, same names; bool's quadrics Shell
  repairs both but on ours joins only 2 of 4 split edges and ends "kind 5 status 32" (a wire badly oriented in the
  offset of the x < 4 round). Done so far: touch edges keep OCCT's blend tolerance (Shell reads it as tangency slack),
  EncodeRegularity, section edges as loose as the blend's, unify after: no change. Next idea: dump both results and
  shell them with Shell 1's own parameters in a T1 to diff the offset walls.
- Stage 2 (michael's Fillet 2): the ball tool (box less sphere) has no face on the part, so the direct build declines;
  it needs the sphere's touch circles on the rounds put on directly.
- Dock stand-in vs live 2274: ours 25/68/45, 8480.517 mm³; live 23/61/40, 8480.505 (2 tori + 3 cylinders a side, no
  vertex blends): one face a side more than live (follow-up).
- 05:06: swapping the two rounds' section order changes nothing; the two results' vertex degrees match too. Asked bool
  (bool.md "From blend 05:06") what by_quadrics needs. SNAP-FILLET6 landed (4b3e0611f).

**DOCK-CORNER (follow-up of DOCK-FILLET, 05:15 UTC):** the stand-in (features_c `dock_passage`) after R0.8: ours 25/68/45,
live 23/61/40 (live 2274). Per side ours matches live's 2 tori (14.75 / 15.15) and 3 r0.8 cylinders, including the
step's round running up to z 10.7 (0.9 line) and ending on the inner torus (1.171 curve); the extra face is OCCT's
0.68 mm² B-spline corner patch where the outer arc's torus (15.15) meets the outer mouth's vertical cylinder (axis at
(±6.3, 13.78), on the torus' centre circle): live's cylinder ends on the torus along their intersection curve, no patch.
Next: equal-radius rounds whose spines meet (cylinder axis through the torus' centre circle) mitred, not patched.
- 11-08 06:00, local branch dock-corner-wip: fillet_by_tools extended (lines along cylinders with exact spandrels
  against the circle, arcs at concave corners cut back at the other wall's plane, free ends run on past where only air
  is taken, convex and concave edges in one fillet, an OCCT USD pass after) builds the stand-in exactly as live:
  23/61/40, 8480.50539 mm³ (live 8480.50526). Not landed: in the user's document it also takes two earlier fillets
  and OCCT's Booleans on the 217-face part make them slow (USB Pencere R 55 -> 175 ms, Kapak Alt R 18 -> 254, Fis
  Gecidi R 282 -> 492). Glue Shift is wrong there (tools run on through faces), pre-fusing the tools slower. Needs a
  local Boolean (only the faces the tools touch) or the direct build (BLEND-DIRECT) to be fast enough.
