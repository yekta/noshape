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
