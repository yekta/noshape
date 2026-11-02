# bool: noshape's own Boolean (docs/BOOLEAN-SPEEDUP.md phase 3)

Owner: bool (from 10-28). Worktree /root/projects/noshape-wt/bool (branch agent/bool), target /root/target-bool,
scratch /root/bool-scratch.

## From core (11-01 18:00): Shell's slab fuses, why yours declines them, ranked by OCCT's time
30 CLI fuzz seeds (3000-3029 × 60 steps, vibe4, NOSHAPE_SHELL_DEBUG: quick_boolean now prints noshape's decline;
logs ~/agents/fuzz/shellseeds2/err*.txt there): 188 Shells went to slabs, 1181 s in all, 54 % of it the slab fuse
(174 fuses, 540 s, every one OCCT's). The reasons, by the OCCT fuse time each one cost:
| s | fuses | decline |
|---|---|---|
| 147.4 | 22 | an offset face (the slabs of B-spline / offset faces and of cylinders / cones the exact revolved slab doesn't take: MakeThickSolidBySimple's offset surfaces) |
| 115.5 | 26 | a tolerant input |
| 55.0 | 2 | an internal face |
| 52.8 | 2 | a conic edge |
| 42.6 | 6 | a curved edge |
| 31.8 | 6 | two edges leave a vertex in one direction |
| 21.6 | 8 | a face of revolution |
| 16.6 | 19 | one cylinder in faces of two inputs, their seams apart |
| 12.9 | 6 | a piece's point near a coincident face's boundary |
| 12.6 | 4 | a plane at an angle to a cone |
| 9.7 | 1 | a point near two vertices |
| 8.8 | 6 | a piece's point on another solid's face |
| ≤ 5.7 each | 14 | a non-manifold result, coincident pieces differ, edge without its curve on a free-form face, a void in a result of several solids, a plane nearly along a cylinder, arguments share vertices |
Fusing the slabs yours takes first and adding the offset ones after doesn't help (the rest then declines for the
next reasons: 3.9 → 5.9 s on a dump); one OCCT common of the solid with all slabs as one tool group changes results.
Repro: ~/agents/fuzz/dump3005/shell-10 / -11 (NOSHAPE_DUMP_OPS dumps; core_scratch SHELLDUMP=<path without .brep>).

## From core (11-01 17:00): fins-400's patterns are OCCT's fuse
Regeneration profile (release, vibe4, NOSHAPE_KERNEL_PROFILE): tests/perf/fins-400.noshape regenerates in 1.6 s,
of which Linear pattern 2 1026 ms (op_boolean 910: OCCT's General Fuse of the plate with 399 fins, "Building splits
of containers" 257, "Preparing history information" 240 over 36 k calls, "Performing intersection of shapes" 284)
and Linear pattern 1 482 ms (op_boolean 393). Nothing else in the perf documents spends more in Booleans, apart
from Shell's slab fuses (core's).

## From core (10-31 00:20): CutByFaces history, record order
- op_boolean's CutByFaces build (patch 0001: a cut by 8+ tools in 2+ groups, crossing ones) now records each target
  solid Modified to the result's solids holding its faces or their pieces, as OCCT's usual build and the Rust
  Boolean do (it said Deleted: the result's solids are none of the target's images). T1
  `boolean_groups::cut_with_many_tools_some_crossing` asserts it (fails without the fix). The naming never read
  solid records (name_result keeps faces, edges, vertices), so nothing relied on Deleted. Boolean corpus cases
  recorded through CutByFaces (knurl-like cuts) now replay with the solid Modified: rebase those summaries.
  (10-31 03:40, done for /root/core-scratch/bool-corpus: 50004ac95f2ebada, 6cf0bfdcdd6e8601, 8c3247abe6a294a0, Cut
  by 14–24 tools, history only. NOSHAPE_BOOL_REBASE=1 rewrites every case, not just the differing ones: the other
  971 got this run's volumes, each within the replay's 1e-9 of the old (their counts, validity and history were
  already equal), and the unstable 5c40b5731295e50a this run's history. Your other corpora are untouched.)
- Records are in a stable order: each input's Modified and Generated images by their index in the output
  (HistoryBuilder::add_history; unify's lists came in another order run to run, the same records). The naming
  sorts what it reads, so names don't change.

## What it is
- `crates/brep` (`noshape-brep`, pure Rust, below the kernel): `Body` (vertices, edges, faces, solids in index
  arenas; one forward direction per edge) and `boolean(op, args, tol)`: fuse / cut / common of solids bounded by
  planes, cylinders and cones (pairs that meet: cylinders and cones on parallel axes, cones coaxial, planes square
  or parallel to them: holes and drilled holes with their 118° tips, countersinks, bosses, slots, notches, voids;
  a cone's apex is a degenerate edge), and a line's common with a solid (boolean/line.rs); OCCT's result without
  unify (every face split by the other solids' faces and by their edges on it, pieces kept by where they lie,
  coincident faces kept once) and OCCT's history.
- Tolerances: two points within `tol` are one, a point within `tol` of a plane is on it; `tol` = 2 × the inputs'
  largest tolerance (at least 1e-7; inputs over 1e-5 decline). A distance in (tol, 10 tol] is ambiguous: decline.
- How: where two faces' surfaces meet (a line, a circle) is clipped to both faces in their parameter planes
  (`curve2.rs`: segments and arcs; a cylinder's plane is (r θ, h), a seam's vertices sit at both ends); each face is
  split into regions there (`arrange.rs`), each region is in / out / on the other solids (ray parity, coincident
  planes), the op keeps some. Seams stay edges: a piece crossing one is two faces, as in OCCT.
- Kernel: `op_boolean` tries it after fuse_slabs / on_face / the local Boolean / through_holes (their results are
  what the corpus records; a big target the local Boolean declined comes here too: 10-29), on by default
  (`NOSHAPE_BOOL_RUST=0` off); Shell's slab and draft Booleans and Thicken's / Shell's offset cuts through
  `quick_boolean`. `cpp/rust_boolean.cpp` converts the arguments (their OcShape maps' indices are the body's; the
  body cached on the OcShape), builds the result (unchanged inputs reused, pieces on the input's curve / surface,
  new lines for sections) and a `BRepTools_History`; `src/brep_bridge.rs` is the Rust side. Declines: free-form
  surfaces and curves, sheets, shared sub-shapes between arguments, tolerant inputs, fuzzy, mesh bodies, common
  with several tools, and whatever the algorithm finds ambiguous. `last_rust_decline()` says why.
- OCCT's history, as learnt from the corpus: an input vertex on another argument's edge or face (or another's
  vertex) is replaced by a new one, so its edges and faces are Modified; new vertices are Generated by the input
  edges and faces they lie inside; section edges are Generated by the two faces that cross there; a cut's object
  solid and a common's solids are Modified to their images, a fuse's solids are Deleted, a solid left alone is Kept.

## P0 10-28 20:45: a race, fixed (21:20)
`brep_results_are_the_same_on_any_thread` failed on vibe2's watcher (8 cores; lead moved it to T2). Real: the
result's build gave *input* edges curves on the new faces' (copied) surfaces (`UpdateEdge` / `Range` on kept
edges, and plane pcurves on every edge of a new face), changing shared input TShapes while other threads read them
(`BRep_Builder::Range, no pcurve`, ~1 in 30–60 runs here). The test's switches were already per thread. Now the
build never changes an input shape: new faces stay on their input face's surface (whose curves the kept edges
have; a kept edge without one there declines), only edges the Boolean made get curves. The test now runs every
small case at 8 and 16 threads on the same input shapes: 0 failures in 200 runs (100 pinned to 8 cores); back in
T1. Corpus unchanged: 378 of 973 taken, 0 differ.

## A fresh corpus (10-28 22:00)
/root/bool-scratch/corpus2: 692 Booleans recorded under OCCT from today's model and kernel tests (incl. Shell's).
It found curves added to one face crossing without a vertex (two slabs' edges on a third's face: "no point inside
a face piece"); now a vertex goes where two such curves cross, and a face is said to generate a vertex only where
one input edge pierces it (two crossing edges made it; faces alone meeting: no one, as OCCT). Where that crossing
meets a vertex of three arguments OCCT's choices differ: declined. Now: main corpus 377 of 972, fresh corpus 369
of 692, Shell's direct ones (/root/bool-scratch/direct-corpus) 28 of 85 (was 13): all 0 differ. 5c40b573 (a
centroid on a .5 µm rounding boundary) is in the main corpus's `unstable` list.

## Left-handed planes (10-28 22:40)
A plane whose frame is left-handed has normal X × Y = minus its axis; the conversion took the axis, so such faces
came out inside out (seen as "a non-manifold result": two faces running a shared edge the same way). Fixed:
main corpus **411 of 972** taken (was 377), fresh corpus 387 of 692, Shell's 28 of 85: all 0 differ; < 50 input
faces median 0.58 ms vs 7.9 ms.

## Tangent curves, one axis, what the model does next (10-29 00:00)
- A face's curves leaving a vertex in one direction a rounding apart (an arc tangent to a side: a cylinder
  touching a box's edge) were ordered by the rounding, not by their turn, so the face had no pieces ("a face
  lost"): directions within 1e-9 are one now.
- The check that every cylinder of every argument shares one axis (and every plane is square or parallel to it)
  is gone: only faces of different arguments that meet are intersected, and those pairs decline themselves.
- The replay now also compares what `unify_same_domain` (which the model and Shell run on a result) makes of
  ours and of OCCT's (faces, edges). It found two kinds the plain comparison passed:
  - Seams (tried 10-28 and reverted): a circle on a part of a cylinder mapped either side of its gap, a section
    along a seam taken as the seam. +23 cases, but OCCT gives such a cut line a seam's two curves and the
    model's merging went differently (oracle_p1 LP2 24 faces vs 21, CP2 12 vs 10; land 11 failed on it). They
    decline again ("an arc across a cylinder's seam", "a section along a cylinder's seam").
  - Shell's slab fuses with two faces of one cylinder from different inputs meeting at a circle: OCCT's merge
    into one, ours didn't (bd16a559, f82019a4, since the direct path). `quick_boolean` now declines those
    (`noshape_brep::merge_check`, and `set_rust_boolean_merged_after` for the replay of direct cases); op_boolean's
    13 such cases merge alike and stay.
Main corpus **419 of 972** (was 411), fresh 385 of 692, Shell's 26 of 85: all 0 differ, incl. the merge check.

Then (10-29 00:20): inputs with tolerances up to 1e-3 (back to 1e-5 at 02:20: points are one within the largest
tolerance of all the inputs, where OCCT goes by each point's own, so 1e-3 could merge points 0.2 mm apart that
OCCT keeps; it took only 2 cases), and a cut's tools whose faces meet facing each other (slots of side-by-side tools,
Shell's slabs): those pieces are inside the tools, kept by neither. Facing one way still declines (OCCT then
says the object's solid was deleted, not modified). Fresh corpus 393 of 692, Shell's 34 of 85, main 419: 0 differ.

Voids (10-29 00:30): a result shell inside out is a void of the one solid there is (several solids: declined),
and an input solid's voids are its faces like the rest (a ray crosses them alike). The flat body the kernel
passes (BrepBody) has shells now: `solid_shells`, `shell_faces`, `faces_of_shells`. Fresh corpus 400 of 692,
Shell's 38 of 85, main 419: 0 differ.

A line's common with a solid (10-29 01:00; boolean/line.rs): model-core's `ray_intervals` falls back to
Common(edge, solid) where its ray meets an edge or vertex (~20 ms each in OCCT). Cut where the line meets the
solid's vertices (they're modified), edges and faces (the new vertex generated by the line and by that edge or
face); the pieces inside are the result (edges alone: the C++ build puts them in the compound), the rest of
the solid deleted. Declined: a line along a face or an edge, ending on the boundary, touching, near anything
within the grey zone, missing the solid. Main corpus **432 of 972** (was 419), fresh 410 of 692: 0 differ.

Seams (10-29 01:35): the seam changes of 10-28 are back (a circle on a part of a cylinder in parts either side
of its gap, a curve starting on the start angle going either way, a section along a seam being the seam), with
what made OCCT's unify_same_domain merge our results differently: (1) a piece of an edge that has two curves on
a face's surface (between two faces on it, once a seam) keeps both, as OCCT's piece does (we gave it one, at a
place good for one of the faces); (2) a face with no input edge of its own to place its angles goes by its input
face's (they hold it), and Rust's gap angle is reduced to [0, 2π). Also: a solid with another argument's piece
in or on it is built anew even when all its faces stay (OCCT deletes it: a cylinder inside a box fused).
Main corpus **451 of 972** (was 432), fresh 426 of 692, Shell's 40 of 85, and a corpus recorded with this
Boolean on (/root/bool-scratch/corpus3, 698 from the model and kernel tests, NOSHAPE_BREP_VS_OCCT=1) 431: all
0 differ incl. the merge check; oracle_p1 (LP2, CP2) passes. Shell's bd16a559/f82019a4 still unify differently
(merge_check stays). /root/bool-scratch/tools/usd_dump (a .brep's faces and edges, unified with `u`; PC=1: each
edge's curve on each face) showed it. For bd16 it shows why: the short wall below the hole (a reversed face on a
right-handed cylinder, its seam used both ways) has the same oriented edges and curves as OCCT's, but its loop
lists them -seam, circle, +seam, circle where OCCT's goes +seam, circle, -seam, circle: ours isn't continuous
in the face's parameter plane, and unify_same_domain then doesn't merge it with the wall above. Fixed (10-29
01:55): the C++ build kept a reversed face's loops in reversed order as well as its edges reversed; OCCT keeps
the order. With that, merge_check (and `set_rust_boolean_merged_after`) is gone: main corpus 451, fresh **434 of
692**, Shell's **48 of 85**, corpus3 439 of 698 (vs OCCT): all 0 differ incl. unify.

Numbers at 10-29 02:35 (tolerant inputs back to 1e-5; a face piece's inside point tried elsewhere when a ray
passes exactly between two curves; the shared-vertex check one map): main corpus **453 of 972**, fresh 432 of
692, Shell's 44 of 85, corpus3 (recorded with it on, vs OCCT) 437 of 698: all 0 differ incl. unify. No case is
slower than OCCT (the least gain 3.8×). Declining costs most on the 999-instance pattern fuses: ~2.4 s of Rust
before OCCT's ~120 s.

Big targets (10-29 03:05): the Rust path runs after the local Boolean, so a target of 200+ faces that reaches it
is one the local Boolean declined: no longer declined here (db5f2f3d 1004 faces, 99 tools: 178 ms vs OCCT 865;
11b69a56 412 faces: 9 vs 88 ms; both as OCCT).

What the model's unify does with a result (10-29 05:46): qa's halfmir2 variant B (a blind half pocket on the
mirror plane, Part mirror, Add) came out wrong with the Rust Boolean from 2520afc4 (01:36; the seam changes) on:
7869.1 mm³ with a −78.54 mm² face, OCCT 7607.301. Every Boolean in it matched OCCT, even after unify; the half
pocket's cut (a piece of the pocket's cylinder across its surface's seam angle, u 4.71–7.85) differed only in
its loop's order, and after the mirror OCCT's unify turned the merged pocket wall over. Now declined: a part of a
cylinder or cone across its surface's seam angle, or whose curves span more than a turn; and two faces of
different inputs on one cylinder meeting at a circle with their seams apart (x directions over a quarter turn
apart: 38b2cd00 and fd7ef128, a hole through a boss, were invalid after unify, since before today). The replay's
unify check compares validity and volume now too. halfmir2 A/B/C, mirhole, halfmir, rust-mirror-min, the rod: all
right. Main corpus 610 of 972, fresh 482 of 692, corpus3 489 of 698, Shell's 52 of 85: 0 differ.

Checked with a CLI of c79eed1d, Rust on vs off (10-29 06:15): qa's bin/facesweep.sh (19 documents, every part's volume / area / faces) identical; holematrix, shellmatrix, xformmatrix, patmatrix, featmatrix, massmatrix, topomatrix identical but for timings; halfmir2 A/B/C, mirhole, halfmir, rust-mirror-min right. Tag v0.4.13 (5e56477b) has 2520afc4 without c79eed1d: halfmir2 B wrong there; flagged P0 at the top of TICKETS. A fresh corpus recorded with it on from today's model and kernel tests (/root/bool-scratch/corpus4, 749 cases): 525 taken, 0 differ from OCCT incl. unify's validity and volume; <50 faces median 0.57 ms vs OCCT 7.89 ms. What it leaves to OCCT is still mostly curved edges (Shell's slabs on filleted parts: 34 s of 46 s).

Direct OCCT call sites (10-29 07:34, lead's list): Draft's two Commons and the chamfer fallbacks' two cuts go through `quick_boolean` now; Shell's offset cut and its regions cavity cuts stay OCCT's (with them, every Boolean matched OCCT but cli-fillet-segv / cli-draft-segv differed afterwards: Shell's own unify and part order); core's `cut_slabs` stays (it only runs after the Rust path declined). A corpus recorded from the model and kernel tests (/root/bool-scratch/corpus5, 929 cases, 120 of them direct_rust) replays 600 taken, 0 differ; qa's facesweep (19 documents), shellmatrix, featmatrix, filletmatrix identical Rust on/off.

RUST-MIRROR-EXTRA-FACE (qa P1, 10-29 04:30): a part mirrored onto itself (Add) kept its hole wall in two after
the model's unify_same_domain (8 faces, OCCT and live 7). The wall's halves are a cylinder and its mirror image
(left-handed); OCCT's unify goes by the order of a seam's uses in the loop, and on a left-handed surface ours ran
the other way round from OCCT's. A face going round a left-handed cylinder or cone now lists its loops' edges in
the opposite order: mirhole 7 faces (one 628.319 mm² wall), rust-mirror-min 19 faces, as OCCT. Not for a part of
one (no seam): then halfmir's half walls merged into a reversed face (7738.201 mm³, as OCCT's wrong result,
HALF-MIRROR-HOLE-INVERTED); with ours it stays right (7214.602, 7 faces). The rod mirrored across its end
(CYL-UNIFY-OPPOSITE) stays one face (= live). The replay's unify check would flag a halfmir-like case (OCCT's
unify is wrong there); none in the corpora. T1 case adf98991 (mirhole's fuse). All corpora 0 differ.

A line along a drill's axis touches its tip's cone only at the apex, a vertex: that vertex is the cut there
(10-29 04:15; it was declined as touching). Main corpus **613 of 972**, fresh 490 of 692, corpus3 497 of 698: 0
differ.

Where OCCT's time still goes (corpus3, recorded with this Boolean on from the model and kernel tests, 10-29
04:10): of ~118 s still in OCCT, 90 s are inputs with curved edges (B-spline, ellipse): Shell's slab Booleans on
filleted parts 1.7–7.8 s each (24 cases, 48 s), op_boolean's 42 s (60, most with free-form faces too). In all the
slow Shell ones the curved edges are within the slabs' reach (/root/bool-scratch/tools/bspl_far): carrying them
untouched wouldn't help; they need intersections with curves this Boolean doesn't have (OCCT's, core's phase 1).

Sheets (10-29 03:45): a Common of faces alone (model-core asking whether faces overlap) keeps only where they
overlap; faces on planes that cross or lie apart (or boxes apart) have nothing in common: an empty result, all
deleted, as OCCT's. Coplanar ones that meet still decline (33 of the main corpus, 1–8 ms each, most with a
face in common). Main corpus **604 of 972** (was 543), fresh 480 of 692: 0 differ.

A line along a face or an edge, and solids with a few tolerant vertices (10-29 03:35): model-core's rays fall
back to the Boolean exactly where they run along a face (77 of the main corpus) or an edge; such a piece is on
the solid's boundary and in the common, as OCCT's (an edge it runs along is modified to it too; the face's edges
it crosses made its ends). A solid whose vertices alone are tolerant (1e-4, edges and faces 1e-7: 84 of them)
goes by its edges' and faces' tolerance when no tolerant vertex is within 10 × its tolerance of the line (C++
side). Main corpus **543 of 972** (was 463), fresh 453 of 692, corpus3 460 of 698: all 0 differ.

Crossings and the faces that made vertices (10-29 03:20): OCCT says a face made a new vertex inside it where an
input edge pierces it across (edges lying on it don't count; of faces of several arguments on one plane there,
the first argument's), not "where exactly one input edge pierces it"; with that, edges of two arguments
crossing on a third's face decline only with a vertex of three arguments on that face or of four anywhere (at
four, OCCT leaves some input vertices without an image: tried 10-29, no rule found). Fresh corpus **444 of 692**,
Shell's 52 of 85, corpus3 451 of 698, main 463: all 0 differ incl. unify.

The grey zone (distances in (tol, GREY × tol] decline as ambiguous) is 10 × now, was 50 × (10-29 02:45): main
corpus **462 of 972**, fresh 435 of 692, Shell's 44 of 85, corpus3 442 of 698: all 0 differ incl. unify.

The 30-feature part's edit commit (`regen_profile::edit_commit_profile`, dev build, load 15–19, 10-29 01:44,
with everything above): Rust Boolean on 102, 124 ms (features 81, 90); off 158, 166 ms (features 130, 134). Hole
1 15 → 8.5 ms, Linear pattern 1 12 → 4 ms, Boss 6 9 → under 3.5 ms; the chamfers and fillets are now most of it.

Looked at and left (10-29 00:45):
- The seam cases (19 "an arc across a cylinder's seam" + 5 "a section along ..." in the fresh corpus, 8 s) are
  the merge problem above: with the seam change, 02fb69425a58e283's result has the tool's cylinder face (z
  0.02–0.03) and the target's two partial faces on one cylinder meeting at a circle; OCCT's unify_same_domain
  merges all three on OCCT's result, two on ours. Same as Shell's bd16a559 (op_boolean's 13 such cases merge
  alike). Representation (curves on the faces at that circle), not geometry: the next thing to find. Found so
  far: our pieces' curves on a partial cylinder can sit a turn or two up (u 4π–6π: Rust's gap angle isn't
  reduced to [0, 2π) and nothing anchors a face with no input edge); reducing it got 02fb to OCCT's face count
  after unify (10) but not its edges (21 vs 23), and didn't change bd16. Taking the window from the input
  face's UV bounds instead made 02fb invalid (face 12, all its edges new). /root/bool-scratch/tools/brep_edges.py
  lists a circle's edges' curves in a .brep (rough: trimmed 2D curves span lines).
- Edges of two arguments crossing on a third's face (12, 3 s): only declining near a vertex of three arguments on
  that face takes 9 more but 2 Shell slab fuses differ (OCCT says a face generated vertices our crossing
  doesn't make and deletes an input vertex we modify, at x = 20 near a four-argument corner): the global rule
  stays.
- Pattern fuses whose instances overlap (2 cases, 90 s and 127 s OCCT): their reference is core's
  `fuse_trimmed` (5006 faces), not a plain fuse (6005): declined.
- Big targets (a2121c5b, 1612 faces): the Rust side ~21–30 ms (output maps dense now), result and build ~15 ms,
  total ~54 ms warm vs the local Boolean's ~43 ms: the local one stays first.

The "no solid" Commons of a solid with one edge (121 in the fresh corpus, ~20 ms each, 2.4 s) are model-core's
`ray_intervals` falling back from `line_crossings` (a ray touching an edge, or not a single solid): an idea for
core, not this Boolean. Arguments sharing faces (17 in the fresh corpus, ~90 ms each): passing the shared
entities through as one (tried 10-28) gets the shared ones OCCT keeps right, but OCCT also makes anew 3 of the 4
corners of a shared face between two stacked boxes (a zero-length section where their side faces meet, by
tolerance): no rule to match; they decline.

## T2 with it on vs off (10-28 20:20, kernel + model T2)
Three differed: core's `bool_replay::bool_corpus_replay` (7 cases whose 1 µm-rounded centroid / edge midpoint sat
on a .5 boundary: a piece now keeps its edge's own parameters at the edge's ends, which fixes 6; one face centroid
remains, 5c40b573), `pattern_second::deleting_the_running_second_pattern_aborts_it_and_reuses_the_first` (the
Rust path ignored the regeneration's cancel flag: now checked before and after, failing "cancelled" like OCCT's)
and `sheetmetal_frames::convert_rips_and_bends_like_onshape` (the input-changing bug of the P0: passes since).
Timed (perf.yml, not T2) `pattern_second::timed_second_pattern_costs_about_the_same_as_the_first` now trips its
ratio because the first pattern got faster (LP1 1345 → 221–327 ms; LP2, a 400-face target the local Boolean /
OCCT does, 2.1 → 1.5–1.8 s; whole 3.9 → 2.4 s): see Requests.

## Public types (psui's `brep::mesh` reads them)
`Body`, `Face { surface, reversed, loops }`, `Edge { v, curve }`, `Use`, `Solid`; `Surface::{Plane, Cylinder,
Cone}` (cone: radius at height h along the axis = `radius + h tan(angle)`), `Curve::{Line, Circle, Degenerate}`
(Degenerate: a cone's apex, start = end). Loops run counter-clockwise about the outward normal. Changes to these
will be listed here.
- 10-29: a result `Solid` may have more than one shell: the outer one first, then voids (inside out: their faces'
  normals point into the void). Types unchanged.
- 10-29: a `Body` may be edges alone (no faces or solids): a line's common with a solid, and its input.
- 10-29: `GREY` (pub const) is 10, was 50.

## Tests
- T1 kernel `brep_replay::brep_kept_bodies_give_the_same_result` (each small case's result cut and fused again, from
  its kept body and from a new wrapper: the same).
- T1 kernel `brep_replay::brep_small_cases_match_occt` (46 corpus cases in tests/data/brep: planes, cylinders,
  drilled holes, two patterns: every one taken, each as OCCT; NOSHAPE_BREP_FACES=1 lists both results' faces, NOSHAPE_BREP_DUMP=<dir> writes both), `brep_replay::brep_results_are_the_same_on_any_thread` (alone vs 16 threads at once).
- T1 brep unit tests (boxes, boss, pocket, through hole, step, side by side, two holes, near coincidence).
- T2 `NOSHAPE_BOOL_CORPUS=/root/core-scratch/bool-corpus cargo test -p noshape-kernel --test all
  brep_replay::brep_corpus_replay -- --ignored --nocapture` (NOSHAPE_BREP_DIFF=1: the differing history records;
  NOSHAPE_BREP_ONLY=<case>). Declined cases fail at once (`set_rust_boolean_only`), so it takes ~30 s.

## Numbers (10-28, dev build, loaded machine)
Planes only (landed fcf860a3): 147 of 973 taken, 0 differ; median 0.61 ms vs 8.7 ms.
With cylinders (landed e0197071): 250 of 973 taken, 0 differ. Then (16:30): seams anywhere, faces nothing reaches
not split, 2D crossings of collinear / concentric curves ignored (a BREP round trip changed results before), bodies
kept on the OcShape: **268 of 973 taken, 0 differ from OCCT**.

| bucket | cases taken | median rust | median OCCT | total rust / OCCT |
|---|---|---|---|---|
| < 50 input faces | 267 | 0.72 ms | 18.0 ms | 257 / 6036 ms (23.5×) |
| 50–1000 | 1 | 4.4 ms | 42.5 ms | 9.7× |

Where the time goes (NOSHAPE_KERNEL_PROFILE=cpu, a 92-face fuse, warm): the Rust Boolean 2.5 ms, building the
shapes 0.3, make_result 0.3, keeping the body 0.2, conversion 0.1 (0.6 cold). Switches: NOSHAPE_BREP_NOCACHE=1
converts every time; NOSHAPE_BREP_BIG=1 lets targets of 200+ faces through (for the locality work).

Declined: 261 no solid (faces / shells as arguments: face–face fuse and common, solid–face common), 98 a curved
edge (ellipses, B-splines), 76 a degenerate edge (cones, spheres), 60 fuzzy / mesh / pattern glue, 50 an arc across
a cylinder's seam, 40 taken by an earlier path, 37 a plane tangent to a cylinder, 23 cylinders at an angle, 15 other
surfaces, 13 shared sub-shapes, ~30 ambiguous geometry and rarer cases.

Gate for default on (10-28 17:00): kernel, model-core, model, custom, assembly, drawing and ops tests with it on: all
green but core's `the_unify_after_a_local_boolean_matches_the_whole_one` (the local Boolean declined a pocket in a
part made by the Rust Boolean: OCCT's face classifier answered differently for a point near an 800-edge face's
boundary because that face's wire started at another edge). Loops now start where OCCT's do; green.
Shell's own Booleans (recorded from the kernel shell tests, /root/bool-scratch/direct-corpus): 13 of 85 taken, 0
differ (most decline: offset surfaces, cones, tolerant inputs).

Cones and the rest (10-28 18:45): coaxial cones (apex edges), fuzzy ≤ 1e-6 (added to the tolerance), a 2D
point-in-region fault fixed (a full circle's start level with the point counted once: `sin(-2π) != 0`; now a ray
in a direction no input favours, another one when it passes a curve's end), big targets cheaper (loop starts,
face pairs). Corpus: **349 of 973 taken, 0 differ from OCCT**; < 50 input faces: 348 cases, median 0.67 ms vs
17.1 ms (OCCT, same build), total 298 vs 7046 ms (23.6×).

Tangent contacts (19:10): a plane or cylinder touching a cylinder along a line both faces reach gets a section
there (both faces split along it), as OCCT does: 364 of 973 taken, 0 differ; < 50 input faces median 0.61 ms vs
23.0 ms.

Patterns (19:40): a pattern's fuse with checked glue (many instances on one part) is taken where the instances
only touch (no piece of one inside another: OCCT keeps its glued result only then, else trims): 379 of 973 taken,
0 differ; 50–1000 input faces: 4 cases, median 104 ms vs 754 ms; the 99-instance pattern 126 ms vs 1.24 s.

The 30-feature part (tests/perf/live-30-features, `regen_profile`, dev build, machine load 30–50, two runs each):
| | Rust Boolean on | off |
|---|---|---|
| full regen (features) | 329, 370 ms | 362, 302 ms |
| edit commit, Extrude 1 depth 10 → 12 (features, median of 5) | 208, 255 ms | 381, 372 ms |
Full regen is within noise (fillets, chamfers and Hole are most of it); the edit commit's features are ~35–45 %
cheaper (the Booleans after the edit: Bosses 3–5 were ~28–37 ms each, now a few ms).

Locality (lead 15:05): the 1612-face fin fuse (a2121c5b) through the Rust path (NOSHAPE_BREP_BIG=1,
NOSHAPE_BOOL_LOCAL=0) is right (0 differ) and ~55–65 ms (Rust ~30, building the shapes and make_result ~25), about
the local Boolean's ~50 ms; plain OCCT 1.1 s. So the local Boolean stays first for targets of 200+ faces and the
Rust path declines them. Next for it: don't touch untouched faces at all (C++ build and make_result over 1600
faces are most of what's left).

Big targets in place (10-29 08:00): a Rust result now gets a UnifyScope as the local Boolean's does (its target
known to have nothing to merge: the faces it changed, every face at their vertices or across their edges, and the
vertices of the target's edges it lost), so op_unify after it looks only there; and the result's body is remapped
onto its maps only when a next Boolean asks (the model's unify usually makes a new shape first): 4.5 ms less on a
1612-face result. NOSHAPE_UNIFY_CHECK=1 (scoped vs whole) clean over the model, kernel and ops tests. Still, on
local_fuse_bench (a2121c5b, 1612 faces, fresh target each round) the Rust path is ~2× the local Boolean (op_boolean
86 vs 37 ms: converting the fresh target 17 ms, the Boolean 37 ms, its unify scope 12 ms; the unify after it ~77 vs
23 ms in merge_planar): the local Boolean stays first. Doing better needs the Boolean itself local (only the faces
near the tool converted and rebuilt).

## PVZ-RUST-BOOL (10-29 08:36, f4ad907d)
- core's T2 `export::pvz_structure` / `pvz_import` failed with the Rust Boolean on: the test part's through hole had
  its seam's curves at u = -3.5e-16 (the face's start angle from its seam's point, which OCCT's own input puts a
  rounding off the axis) and its circles' x axis tilted by as much; the PVZ writer's NURBS conversion then trimmed
  out of range. A start angle within 1e-12 of a whole turn is the turn now, and a new circle starting on its
  surface's x axis takes that axis: the seam at 0 / 2π and the circles on (1, 0, 0), as OCCT's. T1
  `export::pvz_of_a_hole_cut_by_the_rust_boolean` (failed before).
- The same T2 run's `bool_replay::bool_corpus_replay` failed on 2e30111c: "an edge outside its face's range" was
  an exception, not a decline (brep_replay showed it as a decline with no reason, so I missed it). It and "an
  edge against its curve" decline now; brep_replay counts a failure without a reason as a failure.
Replays: main 610 / 972, corpus2 482 / 692, corpus5 600 / 929 (vs OCCT run here), all 0 differ; T2
bool_corpus_replay and export:: (24) pass.

## Ellipses: planes at an angle to cylinders (10-29 09:40, d421f1e4)
- A plane at an angle to a cylinder's axis (not a cone's; |n · a| ≥ 0.25, steeper declines as "a plane nearly
  along a cylinder") is cut in an ellipse, OCCT's own (IntAna_QuadQuadGeo: centre on the axis, the plane surface's
  normal, major axis (n × a) × n, radii r / |n · a| and r). In the faces' parameter planes it is a trigonometric
  curve (curve2's `C2::Trig`: an ellipse on the plane, a sinusoid on the cylinder; crossings by sampling and
  Newton's method); lengths along it are its arc length (never parameter × a radius).
- Its edges are OCCT's to the last bit: rust_boolean.cpp runs OCCT's own `IntTools_FaceFace` on the section's two
  argument faces (once per pair) and takes the curve, the part of it the edge is in and its curves on both faces
  from there, moved by turns as `BOPTools_AlgoTools::MakePCurve` does. (OCCT's curve on the cylinder is a
  30-pole B-spline 1e-8–6e-8 off the exact sinusoid, made over the part, and the parts come from the faces' UV
  boxes, a plane's grown by 10% (CorrectPlaneBoundaries): computing either ourselves gave volumes 1e-6 off.)
  The edge's tolerance is the section's (the curve's and the faces': MakeBlocks adds the tangential one and
  CorrectToleranceOfSE takes it off again), its vertices' that + 1e-12 (OCCT's MakeEdge; its vertex reduction can
  leave some 1e-12 lower: not matched). No SameParameter: OCCT's returns at once (the flag is set).
- Not yet: arguments with ellipses (to_body: "a curved edge"; the next Boolean on a part with a tilted hole goes
  to OCCT), planes at an angle to cones, a piece of an argument's ellipse.
- Replays, all 0 differ with the unify check: main 614 / 972 (+4), corpus2 483 (+1), corpus3 490 (+1), corpus4
  526 (+1), corpus5 607 / 929 (+7 of its 20 angled cases), Shell's direct 52 / 85. The taken ones are 2–7× OCCT
  (its face/face and projections are most of their time). The angled cases left are near-tangencies (an ellipse's
  top 2.7 µm over a counterbore's rim: OCCT keeps a 2.7 µm section across the cap, ours drops it, "an edge ends
  inside a face"), cones, and crossings on a third face.
- Tests: brep `a_hole_through_a_tilted_plate`, curve2 `an_ellipse_and_a_sinusoid`; kernel T1
  `brep_replay::brep_tilted_holes_match_occt` (36 tilted holes against OCCT run there) and 5 corpus5 cases in
  tests/data/brep (multi-tool d466cefc among them; the thread test runs them too).
- psui: `noshape_brep::Curve` has a new variant `Ellipse` (only in a Boolean's result); `mesh::edge_nodes` returns
  None for it (its `_` arm), so such faces mesh as before.

## Exact slabs for Shell, planes through a cone's axis (10-29 11:10)
- Shell's slabs of cylinder and cone faces bounded by two parallels and two rulings (closed faces are split first;
  a cone's may end at its apex) are built exactly (shell.cpp `revolved_slab`: the section in the plane of the first
  ruling revolved about the axis; corners snapped to the face's own vertices, straight edges through them) instead
  of OCCT's simple offset, whose walls along a cone's parallels are B-spline surfaces and whose cylinder edges are
  B-spline curves (the slabs' fuse took 5–13 s on them; the Rust Boolean declined them). Anything else, or a slab
  that fails the checks (valid, the inner point inside), goes the old way; `NOSHAPE_SHELL_OCCT_SLABS=1`: always
  the old way. Kernel shell tests (T1): 43 s → 18 s wall; the slab fuses' recorded OCCT time 65.7 → 35.9 s, the
  Rust path takes 50 of their 88 direct Booleans (was 42).
- Rust Boolean: a plane through a cone's axis cuts it in its two rulings there (`generator` now does cones); a
  plane along the axis but off it (a hyperbola) still declines, unless it is farther from the axis than the face's
  largest radius.
- OCCT's history for an input vertex inside coincident edges of two other arguments (a common block): replaced
  for each edge (Modified) and, when the block's tolerance grows past the new vertex's, replaced again and said
  Deleted (non-destructive mode). Whether it grows is rounding (a fillet's vertex 4.4e-16 off x = 0 grows it, an
  exact box doesn't), so such vertices are taken only where nothing can be rounded (straight edges along an axis,
  exactly through the vertex), else declined. Crossing edges (a line and a circle): Modified, as before.

## Cylinders at an angle of one radius, axes meeting (10-29 12:10)
- OCCT cuts them in two ellipses (IntAna_QuadQuadGeo: centred where the axes meet, normals a1 + a2 and a1 - a2,
  major axes the other, radii r / cos(A/2), r / sin(A/2) and r, a1 the earlier argument's face's axis); the
  Rust Boolean now does too (`cyl_cyl_angled`), its edges OCCT's own as for planes' ellipses. IntPatch also
  starts each closed ellipse at its parameter 0 (a vertex there; `section_cut`). Other radii or axes that miss
  (a curve of the fourth order) still decline, unless the axes are farther apart than the radii's sum.
- Cross-drilled holes (kernel T1 `brep_replay::brep_cross_drilled_holes_match_occt`, brep
  `cross_drilled_holes_of_one_radius`): 6 of 6 fuses / cuts taken, as OCCT. The filleted-box Shells' sector slabs
  (corpus5 d12ad61b, 49de15f4, 616ac3a3, 3–4 s each) now get past this and decline on the vertex rule above.
- rust_boolean.cpp: NOSHAPE_BOOL_DEBUG prints OCCT's ellipses when none matches ours.
- Corpus6 (/root/bool-scratch/corpus6, 828 Booleans of today's kernel + model T1 tests, exact slabs): Rust takes
  564, 0 differ. What's left, by recorded OCCT time (105 s, loaded machine): curved edges 32.8 s (73; helical
  sweeps against rods 15 s in 3 cases, fillet slabs), a plane along a cone off its axis (hyperbolas, in slab fuses)
  14.4 s (13), an internal face 13.8 s (1 slab fuse of 72), the vertex rule 10.6 s (5), tolerant inputs 6.9 s,
  edges crossing on a third's face 5.2 s, an edge ending inside a face 5.1 s.

## Hyperbolas: planes along a cone's axis, off it (10-29 12:40)
- OCCT's section there is a hyperbola's branch (IntAna_QuadQuadGeo; IntTools_FaceFace keeps it a
  `Geom_Hyperbola`, in pieces clipped to the faces): centred where the plane is nearest the apex, its normal the
  plane surface's, its major axis along the cone's axis into the face's nappe, radii |dd| / |slope| and |dd|.
  The Rust Boolean now makes it (`plane_cyl`; an `Ell` with `hyp`: c + a cosh t x + b sinh t y, not closed, its
  range from the face's heights), its curves in the faces' parameter planes curve2's new `C2::Hyp`
  (c + u cosh t + v sinh t + w atan(k sinh t): the plane's hyperbola, the cone's θ = φ + atan(b sinh t / |dd|)),
  its edges OCCT's own (rust_boolean.cpp's ellipse path takes `Geom_Hyperbola` too, by centre and normal).
- psui: `noshape_brep::Curve` has a new variant `Hyperbola` (only in a Boolean's result, like `Ellipse`).
- Tests: kernel T1 `brep_replay::brep_hyperbolas_match_occt` (15 box / cone cases, 14 taken, as OCCT), brep
  `a_cone_halved_through_its_axis` (now with the off-axis cut).
- Replays all 0 differ: main 615, corpus2 485, corpus3 492, corpus4 528, corpus5 609, corpus6 566, Shell's 70.

## P0 RUST-MIRROR-NOTCH / RUST-HALF-POCKET-3 (qa 10-29 10:03, fixed 13:20)
- Not the Booleans (every one matched OCCT): the unify after them. ShapeUpgrade_UnifySameDomain's result, where
  faces on one cylinder (cone, ...) merge, depends on where those faces' wires start: the notched cylinder fused
  with its mirror gave the solid of 2/3 its volume (a merged face more than a turn round) for 3 of 8 starts of one
  seamed wire, OCCT's own start among the good ones only by luck; the half pocket lost 2/3 of the pocket for 2 of 4
  starts of a half cylinder's wire (OCCT's too). The Rust Boolean's wires start elsewhere than OCCT's, so it hit the
  bad ones. USD also changed its input's shared edges in place (the Boolean result's OcShape then read the wrong
  volume too).
- Fix (core's unify, boolean.cpp `usd_kept_volume`): where two adjacent faces on one kind of periodic surface could
  merge (`round_candidates`), USD runs on a copy and must keep the volume (1e-9); else again with those faces' wires
  started 1..7 edges further on (`rotate_round_wires`); none keeping it: not unified. Costs a copy and two volumes
  there. core: please review.
- Also: new section circles take the frame of the surface they lie on (their parameter its angle, every piece of a
  circle on one frame, as OCCT's; each piece's parameter started at 0 before).
- Checked: qa's cylnotch / cylnotch-union / halfpock3 / halfpockunion / pocksweep (32 / 32) / halfmir2 A-C /
  halfmir / mirhole right on the dev CLI; boolmir seeds 4 8 16 22 39 45 69 75 80 84 × 50 identical Rust on / off;
  kernel and model T1. Test: kernel T1 `brep_replay::a_notched_cylinder_fused_with_its_mirror_unifies_whole`
  (the app's notch cut, recorded: tests/data/brep/ad14557ce79005e1).

## Free-form faces (10-29 14:50): the Rust Boolean takes B-spline faces, with OCCT's own sections of them
- `Body` carries NURBS: `Surface::Nurbs { index }`, `Curve::Nurbs { index, t0, t1 }`, `Body::nurbs_surfaces /
  nurbs_curves`, and `Body::pcurves` (an edge's curve on a NURBS face, OCCT's, in its parameters). The kernel
  converts Geom_BSplineSurface faces, B-spline edges and their curves on such faces (rust_boolean.cpp `to_body`,
  flat in `BrepBody::nurbs / pcurves`; brep_bridge decodes); `brep::nurbs` evaluates them (stack-only basis
  functions; points, first and second derivatives, a point's parameters).
- The Boolean (`boolean_with(.., src)`, boolean/free.rs): a face pair with a NURBS face gets its sections from
  `SectionSource` (the kernel's `Occt`: rust_boolean.cpp `brep_occt_sections` runs OCCT's own IntTools_FaceFace,
  as the pave filler asks, and keeps the curves); each is a `Path::Nurbs` clipped in both faces' parameter planes
  (on a NURBS face OCCT's curve on it, its parameters scaled by the surface's mean speeds so 2D lengths are about
  3D ones; on a plane, cylinder or cone `C2::Mapped`: the 3D curve through the face's own map, cut where it crosses
  a cylinder's seam / gap), then arranged and classified as any face (rays across NURBS faces: per patch box,
  Newton on (u, v, t)). Result edges on those sections are OCCT's curves and curves on faces (the ellipse path:
  `EllEdge`), pieces of input B-spline edges keep their curves and their curves on faces; faces stay on their
  input surfaces. Declined: edges lying on another argument's NURBS face (the thread fused with a rod its helical
  edges lie on), sections OCCT makes as conics, results with free-form faces aren't kept as bodies.
- Replays, all 0 differ: main 634 / 972 (+19), corpus2 504 (+19), corpus3 511 (+19), corpus4 547 (+19), corpus5
  628 / 929 (+19), corpus6 585 / 828 (+19), Shell's 70. The thread ∩ cylinder (74acee2f): OCCT 1.13 s, ours 0.67
  s (5.7 s before evaluation went allocation-free and patch boxes cached; OCCT's face/face is 0.15 s of it).
- Tests: kernel T1 `brep_replay::brep_free_form_faces_arrive_as_occt_has_them` (the decoded degree-10 surfaces
  against OCCT's points to 1e-12), `brep_free_form_cases_match_occt` (tests/data/brep-freeform); brep `nurbs`.
- psui: `noshape_brep::Surface` has `Nurbs { index }`, `Curve` has `Nurbs { index, t0, t1 }`, `Body` three more
  fields (`..Default::default()` in literals).

## Free-form: where OCCT's time goes (lead's step 1, 10-29 15:30)
The 57 free-form cases corpus6 still sends to OCCT, each run alone, serial, non-destructive, with core's
instrumented TKBO (/root/occt-kern, NOSHAPE_BOP_STATS; tools/bopstats + tools/split.py in /root/bool-scratch):
intersection steps (VE, EE, VF, EF, forced, FF of the first pave filler) 40.8 s, everything else (bookkeeping,
MakeBlocks, building, classification) 4.5 s. Most of the intersection time is edge/face, not face/face:
- the thread fused with its rod (8dc3e8d4, 45b63a7e): 16.2 / 16.7 s, of which EF 14.9 s, VF 0.9 s, FF 0.5 s
  (B-spline helices against the rod's cylinder and the B-spline faces). The hybrid needs no OCCT EF (an edge's
  crossings come from clipping the face/face curves), so the gain there is taking the case (its helical edges lie
  on the rod: declined so far), not an intersector of our own.
- the rest 0.01–2.8 s each, mostly 50–95 % intersections too (fillet / shell slabs with tori, B-spline walls).
- recorded (parallel, loaded) times overstate some: addb9a14 13.8 s recorded, 2.8 s alone.
The case already taken (74acee2f, thread ∩ cylinder): OCCT 1.98 s alone, 1.17 s of it intersections (EF 0.38,
VF 0.07, FF 0.16); ours 0.67 s with OCCT's FF.

## Arguments with ellipse edges (10-29 16:10): parts with tilted holes
- The kernel passes an argument's ellipse edges (kind 3, OCCT's frame and major radius, `BrepBody::ellipse_minor`;
  closed ones too, as circles) and the Rust Boolean takes them as `Geo::Ell` (OCCT's parameter): clipped, split,
  in a plane's face when they lie on it (a boss over a tilted hole's rim). Their pieces keep the argument's curve
  and curves on its faces (rust_boolean.cpp `piece_pcurve`); a piece the other way round still declines.
- Also: a B-spline edge lying on another argument's plane / cylinder / cone goes into that face (in parts at the
  seams it crosses); OCCT's sections of free-form faces carry OCCT's own tolerance, used for "along a boundary";
  a NURBS curve's piece whose middle is in the grey band is judged where it is farthest from the boundary, when
  that is clearly off it. The thread fused with its rod still declines (its 5 µm tolerant inputs put a helix's
  seam crossing 0.1 mm from the rod's vertex, in the grey band; then a vertex nearly on an edge): parked.
- Decline reasons from the converter are specific now: an ellipse edge, a conic edge, a torus / sphere / offset /
  revolution / extruded face, a curve on a free-form face other than a line or B-spline.
- Corpus6 after free-form faces (585 / 828 taken): what's left by recorded time: an internal face 13.8 s (1),
  two vertices nearly coincide 12.1 s (7, the thread fuses among them), ellipse edges 11.9 s (15), the vertex
  rule 10.6 s (5), curves nearly along a boundary 10.2 s (9), tolerant inputs 6.9 s; tori 1.2 s, spheres 2.1 s,
  extrusions 1.3 s (so the lead's step 3 is small here).
- A section or piece of an ellipse ending within 1e-5 of an axis' end but not on it declines (an edge tangent to
  the ellipse there: our crossing 1.5e-6 rad short of OCCT's exact 2π, a 3e-9 gap, 1e-9 of a volume); results
  with ellipses or hyperbolas aren't kept as bodies (converted, they are OCCT's curves); the builder's curves of
  other kinds decline instead of being taken for lines (two crashes in corpus5 / corpus4 before the fix).
- Replays all 0 differ: main 634, corpus2 504, corpus3 511, corpus4 549 (+2), corpus5 632 (+4), corpus6 585,
  Shell's 70. (Corpus6's ellipse cases mostly decline for other reasons still.)
- Tests: kernel T1 `brep_replay::brep_parts_with_tilted_holes_match_occt` (24 Booleans on plates with a tilted
  hole, 19 taken, as OCCT).

## Narrower grey band (10-29 17:30)
- `GREY` 10 → 4 (distances in (tol, 4 tol] ambiguous). OCCT merges within the tolerances' sum, which `tol` already
  is; the band only guards our rounding. Replays, all 0 differ: main 638 (+4), corpus2 508 (+4), corpus3 513 (+2),
  corpus4 551 (+2), corpus5 642 (+10), corpus6 595 (+10), Shell's 70, direct 52.
- Also: degenerate edges of a free-form face (the surface closing at a point) bring their curves on it; a piece
  takes its curve on a free-form face from whichever of its origins is that face's own edge.
- The thread fused with its rod stays parked: past the grey band it stops at a sliver near the rod's end.

## Windows CI: ellipse sections to the model tolerance (lead 16:25, fixed 10-29 16:30 UTC)
- `brep_parts_with_tilted_holes_match_occt` took 16 (needs 19) on MSVC: the 45° hole's Booleans declined "an
  ellipse unlike OCCT's sections". `ellipse_edge` compared OCCT's section with ours to a few ulps: a closed ellipse
  starting where OCCT's does got its start a turn past OCCT's first when the rounding went the other way (a -3e-15
  shift of the parameter reproduces it on Linux: 13 taken, the same three declines). Now: centre within the model
  tolerance, axis within it over the conic's size, parameters within it along the conic (tol / minor radius), a start
  just short of a turn past OCCT's first is its first, ends just outside OCCT's range clamp to it; "an ellipse ending
  nearly at an axis" from 1e-9 rad off (was 1e-12). Checked with the parameters shifted ±3e-15 … ±1e-10: 19 taken.
  Windows: CI on branch bool-ellipse-tol (run 40912100596) green on all three, the test passing on MSVC; landed
  b24ba37b. Replays all 0 differ, a few more taken: main
  638, corpus2 508, corpus3 513, corpus4 552 (+1), corpus5 643 (+1), corpus6 599 (+4), Shell's 70, direct 52.

## Tori, spheres, sweeps (lead's step 3, 10-29 16:50 UTC)
- `Surface::{Torus, Sphere, Extrusion, Revolution}` in `Body` (OCCT's frames and parameters; the swept curve a
  B-spline in `Body::nurbs_curves`), `noshape_brep::freeform::FreeSurface` evaluates them (and NURBS) with first and
  second derivatives; the Boolean treats them as it does B-spline faces: OCCT's IntTools_FaceFace for their sections,
  the rest ours. A free-form face's parameter box is what its edges' curves span (sweeps and tori are unbounded or
  periodic), samples / patches / scale over that box.
- OCCT's section tolerance as the pave filler sets it (`section_tol`: at least the faces', at least 5e-6 unless both
  faces are planes / cylinders / cones / spheres / tori), on the edge and for the Boolean's "along a boundary" (an
  invalid result before: the edge 1e-7 against OCCT's 5e-6). A section piece ending at the section's own end takes
  its bound exactly (PutBoundPaveOnCurve; ours was 2.8e-8 short, 2e-9 of a volume).
- A ray crossing a free-form face is placed on it by the crossing's own parameters, not by inverting the point (on
  a thread the inversion from the nearest sample could end on the face's edge: "no clean ray").
- A thin piece whose first inside point is in the grey band of its face's boundary looks for a deeper one
  (`arrange::deeper`) before declining.
- Spheres / tori / revolutions mostly still decline: "two edges leave a vertex in one direction" (a pole or seam
  vertex is one point in the face's parameter plane for free-form faces; it needs one point per place, and a pole's
  degenerate edge split where curves reach it). Small in corpus6 (tori 1.2 s, spheres 2.1 s recorded): stopped here.
- Replays (on top of the ellipse fix), all 0 differ: main 659 (+21: 20 cuts by an extruded fillet slab, one cylinder
  slab), corpus5 646 (+3), corpus2 508, corpus3 513, corpus4 552, corpus6 599, Shell's 70, direct 52.
- Tests: two extruded-fillet cuts in tests/data/brep-freeform (the T1 `brep_free_form_cases_match_occt`).
- The thread fused with its rod (8dc3e8d4) gets further (past the grey band and the thin piece) and now stops at
  "an edge ends inside a face" on the rod's face.

## Seams of curved faces (10-29 18:30 UTC)
- A curve once round a cylinder from its seam to its seam (a thread's helix on its rod, in parts at the seam)
  mapped both ends to the seam's near side (the side was chosen by the curve's middle, which is half a turn from
  both): now each end is on the side the curve leaves to or arrives from (`Map::map_on`). T1
  `curve2::tests::a_curve_once_round_a_cylinder_spans_its_angles`.
- On a free-form face a vertex is a point per place it is at (both sides of a seam), as on cylinders.
- Replays unchanged, all 0 differ (main 659, corpus2 508, corpus3 513, corpus4 552, corpus5 646, corpus6 599,
  Shell's 70, direct 52). The thread fused with its rod now gets through the Rust Boolean and stops in the builder:
  its helices' pieces on the rod need curves on the rod's cylinder (OCCT splits each helix in 21 and projects them,
  BOPTools_AlgoTools2D::BuildPCurveForEdgeOnFace, tolerance ~1.02e-7): next.

## Free-form speed: helices (lead's step 4, 10-29 19:45 UTC)
- The thread fused with its rod (8dc3e8d4) got through end to end once a piece of another argument's edge lying
  on a cylinder got its curve there projected as the pave filler does (`piece_pcurve`:
  BOPTools_AlgoTools2D::BuildPCurveForEdgeOnFace on the input face). It matched OCCT but took 77 s (OCCT 3-4 s
  wall): curve/curve crossings on its 290-pole degree-10 helices dominated. Now:
  - a NURBS part samples 8 points per knot span it covers, not per span of the whole curve;
  - crossings of two curves look only at runs of 16 chords whose boxes meet, and not at all when the curves'
    control-polygon boxes are apart (`C2::quick_box`); the nearest-curve distance skips curves whose box is
    farther than the nearest found yet; a face's vertices are looked for only in a NURBS path's box;
  - NURBS curves evaluate by de Casteljau on their spans' Bézier pieces, made once per curve (A5.6;
    `NurbsCurve::spans`, a cache field, `Default` when making a curve: psui, a public type change), instead of the
    basis functions each time (the same values to ~1e-14; T1 `nurbs::tests::spans_evaluate_as_the_basis_functions`).
  77 s → 1.6 s CPU for the Rust Boolean (0.35 s of it OCCT's face/face sections), against OCCT's 3-4 s wall.
- A copy of a NURBS curve starts with no spans (its poles may be changed after: the free-form faces' scaled curves
  on them did, and evaluated the original's; T1 `nurbs::tests::a_copy_with_other_poles_evaluates_them`).
- Replays, all 0 differ, one more each: main 660, corpus2 509, corpus3 514, corpus4 553, corpus5 647, corpus6 600
  (the thread fuse), Shell's 70, direct 52. The thread fuse: ours 2.3 s against OCCT's 3.1 s wall (replay, loaded
  machine); thread ∩ cylinder (74acee2f) 0.54 s against 0.99 s; extruded-fillet cuts 21 ms against 45 ms.
- The other thread fuse (45b63a7e) declined "a non-manifold result": its faces are reversed, and a reversed
  free-form face's parameter plane wasn't mirrored (planes' and cylinders' are), so its pieces' loops ran the wrong
  way round. Now its v scale is negative. Taken, as OCCT: 2.1 s against OCCT's 3.8 s. Replays, all 0 differ, ~15
  more each (reversed free-form faces declined before, mostly "a non-manifold result"): main 675, corpus2 524,
  corpus3 529, corpus4 569, corpus5 661, corpus6 615, Shell's 70, direct 52.

## Tolerant inputs, where it doesn't matter (10-29 21:00 UTC)
- An argument's sub-shapes with a tolerance over 1e-5 no longer decline the Boolean when their boxes, grown by their
  tolerances, reach no other argument's box: nothing of theirs meets another argument, so the Boolean goes by the
  rest's tolerance (`rust_boolean`, before "a tolerant input"). The tolerant ones where the arguments meet still
  decline (OCCT merges by each point's own tolerance, ours by the largest: per-point tolerances would take them; the
  three biggest such cases in corpus6, 10.2 s recorded, are fuses of boxes with 1e-4 edges where they touch).
- Replays, all 0 differ: main 680 (+5), corpus2 528 (+4), corpus3 533 (+4), corpus4 573 (+4), corpus5 666 (+5),
  corpus6 620 (+5), Shell's 71 (+1), direct 52. T1: a case (5f078363) in tests/data/brep.

## Empty results, edges touching a face (10-29 21:30 UTC)
- A piece of a face that another argument's edge touches without cutting it (a box's edge on a tilted box's face)
  no longer declines unless that piece is kept or coincident; an empty result (nothing kept) is everything
  deleted, solids too, as OCCT's empty compound (was "an empty result"). Model-core's overlap probes (commons of a
  thin box and a big tilted one touching along an edge) were most of corpus6's "an edge ends inside a face".
- Replays, all 0 differ: corpus5 682 (+16), corpus6 636 (+16); main 680, corpus2 528, corpus3 533, corpus4 573,
  Shell's 71, direct 52 as before. T1: a case (14e82fef) in tests/data/brep.

## Planes at an angle to cones (10-29 22:00 UTC)
- A plane at an angle to a cone's axis that cuts every ruling of the face's nappe: an ellipse (`plane_cone`), its
  axes from the two rulings in the plane through the axis and the plane's normal, its centre off the axis; on the
  cone's face it is mapped point by point (`C2::Conic`, a 3D ellipse through the face's map), its parameter at the
  seam's angle solved exactly. OCCT's ellipse is matched within the model tolerance as before. T1
  `brep_replay::brep_cones_cut_at_an_angle_match_occt` (a countersink's frustum and a tilted box: 9 taken, as OCCT).
- Parabolas (10-29 22:40 UTC): a plane along one of a cone's rulings, OCCT's gp_Parab (`Curve::Parabola` in the
  result, kind 6 in the bridge: vertex, normal, the way it opens, focal length; `Ell::par`, `C2::Conic` with `par`).
  T1 `brep_replay::brep_cones_cut_along_a_ruling_match_occt` (a 45° cone and a 45° box: 6 taken, as OCCT). A plane
  through the apex along a ruling declines ("a plane through a cone's apex": corpus6's 45° cases are those).
- (Fixed 10-30, below.) On a cylinder or cone a vertex exactly half a turn from the seam was two points in the parameter plane (the
  side of the seam is judged for every vertex, and rounding puts its two ends on either side; it should be only for
  vertices on the seam). Fixing it lets the tilted-holes test's 45° slot common through, whose history then differs
  from OCCT's: the part's edge 20 (a generator of the 45° hole) is in neither result, ours says Deleted, OCCT's
  history has no record for it at all (an OCCT quirk to find and mimic before the fix can land; the fix alone failed
  the land, 10-29 23:00).
- Hyperbolas at an angle (not along the axis) still decline "a plane at an angle to a cone".
- A piece of any curve within the grey band of a face's boundary is judged where it is farthest from the boundary,
  if clearly off it (was B-splines only): those five cases got past it to the parabola.
- Replays unchanged, all 0 differ (main 680, corpus2 528, corpus3 533, corpus4 573, corpus5 682, corpus6 636, Shell's
  71, direct 52).

## Planes through a cone's apex (10-29 23:00 UTC)
- A plane through a cone's apex declined whenever the apex was in the face's box. Now only when the face reaches
  its apex (the apex's height within its vertices'): a frustum's plane through the axis gets its two rulings, one
  cutting every ruling has nothing in common with it, one along a ruling touches it there (`touch`, as a plane a
  cylinder: a section where both faces reach the line, else nothing; now for cones too).
- Replays with the parabolas, all 0 differ: main 680, corpus2 528, corpus3 533, corpus4 574 (+1), corpus5 685 (+3),
  corpus6 640 (+4), Shell's 75 (+4), direct 52. T1: the ruling test's third configuration (through the apex; its fuse declines).

## Apexes of cone faces that don't go round (10-30 00:30 UTC)
- A cone face that doesn't go round and reaches its apex (a countersink's or drill point's half): its apex's
  degenerate edge in the parameter plane runs at the apex's height from the angle of the ruling coming to the apex
  to that of the one leaving it (found by their vertices: the face's edges aren't listed in loop order), and the
  apex vertex is a point per place there, as a free-form face's vertices.
- Replays, all 0 differ: main 683 (+3), corpus2 531 (+3), corpus3 536 (+3), corpus4 577 (+3), corpus5 688 (+3),
  corpus6 644 (+4), Shell's 76 (+1), direct 52. T1: a case (017a8af6) in tests/data/brep.

## Tolerant inputs where the arguments touch exactly (10-30 01:30 UTC)
- A tolerant sub-shape (over 1e-5) that reaches another argument no longer declines when every sub-shape of the
  other arguments near it (boxes grown by the grey band of the two tolerances) is either on it exactly (within the
  rest's tolerance: OCCT and the Boolean put them together alike) or clearly apart (past the grey band of the sum of
  the two tolerances, OCCT's merging distance): then the Boolean goes by the rest's tolerance. Distances in between
  still decline (per-point tolerances would take them; corpus6's biggest, f516559d, has a vertex of 1.7e-4 at 1e-4
  from another argument's face). Over 64 tolerant sub-shapes: declined (the distances cost).
- Replays, all 0 differ: corpus2 533 (+2), corpus4 579 (+2), corpus5 692 (+4), corpus6 646 (+2), Shell's 78 (+2),
  direct 54 (+2); main 683, corpus3 536.
- The slab fuse with an internal face (addb9a14, corpus6's biggest decline, 13.8 s): OCCT's own result is invalid
  (recorded valid 0): left declined.

## Closed edges on free-form faces; the seam-side vertex; OCCT's history merge (10-30 02:40 UTC)
- A whole closed edge (a full circle) on a free-form face took its curve there from its vertex to itself, nothing:
  it takes the whole curve now (tori, revolutions: a fillet's quarter torus "lost" a face).
- The seam-side vertex is fixed (a vertex half a turn from a cylinder's seam is one point). The history question it
  raised: OCCT's history had no record for the tilted-holes part's hole generator, which ours called Deleted. OCCT's
  BOPAlgo_Builder::PrepareHistory does mark it removed, but BRepAlgoAPI then merges the builder's history into its
  own (BRepTools_History::Merge), which keeps a removed shape out of the removed when it has generations ("Propagate
  R23 to R12": `!myShapeToGenerated.IsBound`). So OCCT's final history never says Deleted for an input that generated
  something; ours now drops such Deleted records too. The tilted-holes test takes all 24 now (was 19).
- Spheres / tori: of the nine small ones collected, five taken now; three concentric-sphere cuts decline "a void in a
  result of several solids" (the shells' orientation goes by an edge-polyline volume, empty for a sphere whose only
  edges are its seam and poles: a surface-based one next), one sphere whose poles lie on a box's faces declines.
- A partial cylinder's angles (its seam in the gap) come from its ellipses' and B-splines' too, not only its arcs
  (corpus2's "a cylinder face without arcs": 3 taken, 9 stop at "an edge on another argument's free-form face").
- A result with any free-form face (tori, spheres, sweeps too, not only B-splines) isn't kept as a body for the next
  Boolean (the kept-body test caught a torus: kept and converted afresh disagreed).
- Replays, all 0 differ (against the tolerant-inputs land): main 694 (+11), corpus2 537 (+4), corpus3 540 (+4),
  corpus4 584 (+5), corpus5 701 (+9), corpus6 648 (+2), Shell's 78, direct 56 (+2). T1: a torus fillet case
  (39a1a52d) in tests/data/brep; tilted holes ≥ 24.
- Not taken: the main corpus's three pattern fuses with overlapping instances (217 s recorded) go noshape's
  "trimmed" way (the tools cut back by the argument, then glued), not the plain Boolean: ours would need that
  too.
- Still declined, now understood: vertices inside coincident edges of two other arguments (OCCT's vertex
  tolerance growth decided by rounding, its history Deleted or Modified accordingly: not the merge rule).

## Windows CI again: a shell's orientation (lead 01:55, fixed 10-30 02:00 UTC)
- `brep_cones_cut_along_a_ruling_match_occt` took 7 on MSVC (needs 8): "0 mm, 2 mm Common: a void in a result of
  several solids". The result is one shell (the cone's piece and two planes); its volume by its edges' chords is ±5e-23,
  next to nothing (the cone piece's boundary is the two planes' boundaries, so its chords span nothing), and the sign
  rounding's: MSVC's came out negative, a void. Reproduced on Linux by tilting the cut 1e-15 rad or moving it 1e-18 m.
  Now a result's single shell is a solid whatever the chords say, and with several shells one whose chord volume is
  under 1e-6 of its box declines ("a shell whose orientation is unclear"). All 13 perturbations taken; replays
  unchanged, 0 differ. Windows CI on branch bool-void-fix (run 40988955215) before landing.
- (The 5 mm fuse declines on Linux and macOS too, "an edge ends inside a face": the test's 8 counts it out.)
- Rule from here (lead): a new test that asserts how many cases the Rust Boolean takes gets a Windows CI run on its
  branch before it lands.

## Straight and round edges on another argument's free-form face (10-30 03:00 UTC)
- A line or arc of one argument lying on another's free-form face (a cylinder's rim on a B-spline wall) goes into
  that face: its curve there by projection (`free::projected`: its points' parameters on the surface, Newton from
  the one before, cubic Bézier pieces between them with the surface's own tangents, in the edge's parameter; a point
  off the surface or a jump across its seam declines), then clipped and arranged as the face's own edges; the
  result's curves on the face are OCCT's projections (`piece_pcurve`).
- Replays, all 0 differ: corpus2 539 (+2), corpus3 542 (+2), corpus4 586 (+2), corpus5 703 (+2), direct 58 (+2);
  main 694, corpus6 648, Shell's 78. T1: a cylinder face without arcs (b79a02be) in tests/data/brep.

## Sheets, first step (10-30 04:50 UTC)
- A sheet argument (faces, no solid) is a solid of its own with nothing inside (a ray never finds a point in it).
  Taken: a fuse of sheets alone, and a sheet cut by solids or in common with them; the result is its faces in the
  compound, as OCCT's for face arguments (no shells, no closed-shell checks). Coplanar sheets are taken where they
  only meet at each other's vertices (side by side, sharing corners: OCCT gives two faces sharing an edge, as ours);
  a vertex of one inside the other or on its edge between its vertices declines ("coplanar sheets": OCCT left two
  strips overlapping by 5 µm, one's corner on the other's edge, as they were, 0c538d2c), and overlapping pieces
  decline ("sheets coincident"; OCCT splits two plainly overlapping rectangles into three faces: next).
- Not yet: sheets of shells ("a sheet of shells": OCCT's result keeps them shells; the first try returned faces and
  failed three sheet-metal jog / bend model tests in CI, features_c), sheets with solids otherwise, the commons of
  two sheets that meet.
- A kept body (a result's, for the next Boolean) carries its B-spline edges' curves now (`kept_body` copied the
  curves' indices but not the NURBS data: a panic across the bridge in the kept-body test on b79a02be); results with
  B-spline edges are kept again.
- Replays, all 0 differ: main 747 (+53), corpus2 547 (+8), corpus3 550 (+8), corpus4 594 (+8), corpus5 711 (+8),
  corpus6 656 (+8), Shell's 78, direct 58. T1: a sheet fuse (1a39dedd) and a sheet cut (dbdc4a4f) in tests/data/brep.
- skasm owns sheet metal: the shells and coplanar-overlap cases are what their models will hit; progress here.
- Coplanar overlaps, tried (10-30 06:50 UTC, not landed): treating sheets on each other in a fuse as one piece (the
  first argument's, either facing) matches OCCT on two plainly overlapping rectangles (0, 5 µm, 0.1 mm, 1 mm:
  OCCT's three faces) and takes 71 of the 114 corpus sheet cases, but two differ: 0c538d2c (strips sharing side
  lines, overlapping 5 µm: OCCT keeps all eleven faces as they were, plain OCCT path, not understood) and 7895f375
  (a disc filling a plate's hole exactly, the two circles starting at different points: ours invalid). Both have a
  vertex of one sheet on the other's edge between its vertices, as do the plain overlaps, so the vertex rule can't
  tell them apart yet: the conservative rule stays (53 taken).

## Patterns whose instances reach into the target (10-30 07:30 UTC)
- A pattern's checked glue (fuse, glue shift_checked, many tools) whose instances overlap the target declined
  ("pattern instances overlapping") and went OCCT's way: GlueShift, its volumes not adding up, then fuse_trimmed
  (each instance cut by the target, then a glued fuse of what's left). The main corpus's three such fuses were its
  biggest time left (217 s recorded; two of 999 instances). Now the Rust path does what fuse_trimmed does, noshape's
  Boolean throughout (`rust_fuse_trimmed`, boolean.cpp): the instances' cuts (in parallel, as OCCT's), then the
  touching fuse, the history composed as fuse_trimmed's; any decline gives up (OCCT's way then). op_boolean tries the
  touching fuse first under `RustLenient` (a decline returns even in the replay's strict mode) and reads why it
  declined (`rust_last_decline`, a new bridge call).
- The three taken, as OCCT: 999 instances in ~16–19 s against OCCT's ~37 s in the same replay (loaded machine);
  the 9-instance one 15 ms against 177 ms. Of the 16 s: the first, touching attempt ~2.6 s (it declines at
  classification), the cuts ~6 s, the fuse the rest; telling the overlap earlier would save the first.
- Told earlier now (10-30 09:30 UTC): before any section, an instance's vertex inside another argument's solid (by
  the ray, and at least 100 tolerances from all its surfaces; free-form solids not this way) declines at once. The
  999-instance case 13.4 s (was ~16 s; OCCT's 26.5 s here). Without the margin a vertex on another's face counted
  (5 or 6 touching fuses per corpus lost).
- T1: the 9-instance case (58c61d5d) in tests/data/brep (taken in strict mode).

## Cylinders at an angle, any radii (10-30 10:30 UTC)
- Two cylinders at an angle, other than of one radius with axes meeting (a curve of the fourth order), declined
  "cylinders at an angle" (main corpus 26 cases, 5.2 s recorded). Now they go the free-form way: OCCT's section
  (IntTools_FaceFace, as the pave filler asks it), everything else ours.
- OCCT's curve can end on a face's boundary at its parameter bounds (a half cylinder's straight edge), where the
  crossing with the boundary at the very end isn't found: such an open end is a point there now (OCCT's curves only).
- Replays, all 0 differ: main 768 (+18), corpus2 552 (+5), corpus3 555 (+5), corpus4 599 (+5), corpus5 716 (+5),
  corpus6 661 (+5), Shell's 78, direct 58. Times: most 2–10× OCCT's (5–35 ms against 35–170 ms), a few even
  (OCCT's own section is most of those). Six stop at "a section without ends": OCCT returns half of a thin tool's
  loop on a big cylinder, its ends inside both faces (understood no further; declined).
- T1: aff9a32d (angled cylinders) and 90710765 (a section ending on a half cylinder's edge) in tests/data/brep.
- Cones the same way (at an angle to another cone or cylinder, or off its axis). One cone at an angle differed:
  OCCT's section touches the tool's rim circle at two points and OCCT puts vertices there (the edges split), ours
  didn't. Now an OCCT curve clear of a face's boundary in the middle of a stretch but coming within the grey band of
  it (its distance's local minima narrowed down) declines "a section touching a face's boundary"; it declines that
  one case only.
- An argument's ellipse lying on another's cylinder (fuses of tilted holes into tilted bosses) declined "an
  argument's ellipse on another's face"; its pieces in the face go in now as on a plane: main 772 (+3, 7–12× OCCT),
  the rest unchanged, 0 differ.

## The knurl, first attempt: not landed (10-30 18:30 UTC)
- The B94.6 diamond knurl's one cut (148 helical grooves, two TShapes at 148 locations) through the generic Rust
  Boolean, on branch bool-knurl (2 commits, 40f006ee + 40a5d849 in /root/bool-scratch/dev4): OCCT's sections made once
  per placement (a pair of faces keyed by TShapes and relative location), faces apart by their Bézier pieces skipped,
  coincident B-spline edges of two arguments, a pattern's end caps on the object's face, and classification,
  crossings, splits and sections' places on all threads. Replays all 0 differ with it; the recorded knurl
  (d14b48a8, /root/bool-scratch/knurl-corpus) is taken in the replay (same faces, every history record; volume
  3.3e-8 off from the sections' approximations, ~1.4e-15 m³ per groove crossing).
- Not landed: in core's model test (the live feature) it declines "a non-manifold result" after ~100–300 s, then
  OCCT runs: GitHub runners 170–200 s against today's 135 s (Linux) / 81 s (macOS). The result tips on rounding at
  one near-degenerate crossing family (grooves grazing at the bottom plane): with OCCT's section computed per pair
  (not moved from the first of each placement) the same happens in the replay. Generic pairwise sectioning is the
  wrong shape for this case (lead 17:45).
- Baselines, GitHub runners, main 99729fe4: diamond 135.1 s +4.1 GB (Linux), 81.1 s (macOS); straight 0.7 / 0.6 s;
  vibe3 idle (lead): 137.6 s, 4.3 GB; live 49 s.
- Next (lead 17:45 / 18:00, binding: geometry only, nothing keyed on a feature or test): (1) a stage profile of the
  137 s through perf.sh (OCCT's progress scopes timed: sections, splitting, classification, building, history);
  (2) a pattern test set (T1/T2, against OCCT): circular patterns of holes / slots on a cylinder and a plate, linear
  pockets, gear-like teeth, the 400-fin plate, a helical pattern, the knurl, timed with perf.sh before / after;
  (3) tools that are exact rigid copies of each other (rotation about an axis, translation, both; found by comparing
  geometry under the transform within tolerance) solved once per period and the result's faces / edges moved into
  the other periods and sewn at the period boundaries, history per copy; anything not proven (copies not lining up,
  tools across period boundaries the sewing can't prove) the normal path; (4) exact sections where they exist (a
  helix-swept line against a cylinder or another such face).
- (1) Profile (perf.sh, vibe3, d4ee3290; bool_patterns::the_b94_diamond_knurl, NOSHAPE_KERNEL_PROFILE, OCCT's named
  steps timed through the kernel's progress indicator): op_boolean 136 s = OCCT's build 115 s (edge/face
  intersections 20 s, intersection otherwise 22 s, classifying faces against solids 16 s, edge/edge 10 s, section
  edges 7 s, internal shapes 6 s, vertex/face 5 s, making faces 5 s, splits of compounds 5 s, history 4 s, face/face
  3 s, the rest < 3 s each) + unify 16 s + unscale 1 s. Pairwise work over the 148 tools, not any one step.
- (2) bool_patterns (crates/kernel/tests, T2): each case both ways (noshape's / OCCT's alone), faces and volume
  compared; here (loaded): a pocket grid 71 ms (rust) / 1260 ms, gear slots 54 / 528 ms, radial holes 495 / 533
  (OCCT's), helical grooves 820 / 782 (OCCT's), 16 crossing helical grooves each way 13.2 / 14.6 s (OCCT's), holes on
  a circle 3 / 2.5 ms (through-holes path); the knurl's cut 183 s each way (vibe3, both OCCT's).
- (3), first piece: `noshape_brep::pattern` finds tools that are rigid copies (`copy_of`: one motion takes every
  vertex, curve, surface onto the other's of the same index, curves on faces unchanged, within a tolerance;
  `families`; `Rigid::rotation` its axis and angle). T1 `bool_patterns::copies_among_patterned_tools`: a circle of
  holes is one family turning 15°, the knurl's grooves two families of 74 turning 360° / 74. Not used by the Boolean
  yet.
- Also from this work, for core: with `unify` on, a Boolean's history records come out in a different order run to
  run (same records, same shape), the unify history's composition iterating something keyed by addresses (seen on
  the knurl's 2-groove piece; OCCT's path goes through the same code). core's `boolean_budget` test builds a 16-groove
  knurl to exhaust OCCT's 64 MB budget; noshape's Boolean takes that one (branch bool-knurl sets it to OCCT's path).

## Patterned tools, step 2 (10-30 21:30 UTC)
- The generic part of the first knurl attempt, on main now: OCCT's sections of a pair of faces of two tools that are
  rigid copies of another pair (`pattern::families`, by geometry, any feature) made once and moved
  (`SectionSource::sections(.., moved)`, `MovedPair`; the C++ side caches by the canonical pair); faces apart by
  their Bézier pieces' control points skipped; an edge on another's surface given up at its first clearly-off
  point; coincident B-spline edges of two arguments (the curve on the face whose own edge it isn't by its ends;
  the C++ side reparametrizes that face's own coincident edge's curve); the grey band on free-form faces against
  OCCT's own curves; classification, crossings, edge and section splits and the faces' pairing pieces on all
  threads, applied in their order (results unchanged); regions' longest-curve order with lengths found once.
- Found on the way: the classification's ray against a Bézier piece (exact boxes) missed a crossing (Newton from
  the piece's middle only, its steps stalling at rounding): a piece 1 mm from every face of a groove came out
  inside it, and the knurl's "non-manifold result" was that. Now 9 starts per piece met and a stalled point taken
  where it is on the ray (1e-12 m).
- The knurl itself still goes OCCT's way, now declined before any section ("tools coincide", 0.1 s instead of
  ~50–300 s of work first): each groove and its mirror image share their end cap on the cylinder's end face.
  OCCT's history then says the cylinder's solid Deleted or Modified by whether its image shares a face with
  another solid (BOPAlgo_BOP::BuildSolid keeps a solid as is only then; otherwise BOPAlgo_BuilderSolid makes a new
  one): Deleted in the 10- and 26-groove pieces of it, Modified on the whole knurl; ours can't tell yet, so
  declined (an early check: two tools' coplanar planar faces facing one way, one's middle clearly inside the
  other).
- bool_patterns now (here, loaded; ours / OCCT's): pocket grid 60–80 ms / 915 ms, gear slots 33 ms / 423 ms,
  helical grooves 425–600 ms (rust; was OCCT's) / 670 ms, radial holes 690 ms (OCCT's: "a section without ends") /
  450 ms, holes on a circle 3 ms (through-holes path), crossing helical grooves 11.5 s (OCCT's: tools coincide).
  OCCT's result of the 16+16 crossing grooves is not valid (BRepCheck): 6.33e-5 m³, 101 faces, where the grooves
  remove ~8e-7 m³ of the 7.85e-5 cylinder (ours, when it took it: 7.78e-5, 421 faces, valid). The bench compares
  faces and volume only where OCCT's is valid, else needs ours valid.
- psui: `noshape_brep::SectionSource` is `: Sync` and its `sections` takes `moved: Option<MovedPair>` (new public
  `MovedPair`); `noshape_brep::pattern` is new. Nothing else public changed.
- Replays, all 0 differ, the counts unchanged (main 772, corpus2 553, corpus3 556, corpus4 600, corpus5 717,
  corpus6 662, Shell's 78, direct 58).
- OCCT's curves of one pair that meet end to end (a hole's loop on a cylinder in two parts) have a vertex there,
  as the pave filler's: radial holes into a cylinder taken now (bool_patterns: 472 ms / OCCT 623 ms here, was
  OCCT's after "a section without ends"); main corpus 776 (+4), the rest unchanged, all 0 differ.
- OCCT's sections of a copied tool's face with a face of another argument whose plane or cylinder the copy's
  motion keeps (holes round a shaft, a pattern on a plate): made once for one member and moved, its curves on that
  face moved in its parameters too (the C++ builder takes OCCT's own curve on an analytic face); only where every
  curve, moved and as made, stays clear of the face's boundary and seam (OCCT trims and splits there), the member
  made with chosen per face (the next one where the first's reach the boundary), else made for the pair. Found by
  bool_patterns: without the curves on the face moved, radial holes came out invalid. Radial holes 278 ms / OCCT's
  439 ms here (was 472); replays all 0 differ, the counts unchanged.
- The knurl's open question, OCCT's record of the cylinder's solid (Modified or Deleted), from pieces of it (OCCT's
  own results, all 1 solid): 5 grooves (none sharing a start) Modified; 10 grooves (none sharing a start either)
  Deleted; 26 grooves Deleted; all 148 Modified. Not the shared caps then; for a cut BOPAlgo_BOP takes the result
  from the split solids (BuildRC), so the split solid of the cylinder is or isn't the result's solid: next, why
  BOPAlgo_Builder's split solid differs (every groove's end caps lie on the cylinder's end faces). Not the unify:
  with unify off the 10-groove piece's raw cut says Deleted too (158 faces both ways, ours Modified). For a cut,
  BOPAlgo_BOP::BuildRC keeps the cylinder's split images that no tool's image contains; next, OCCT's images of the
  cylinder's solid on that piece (BRepAlgoAPI_Cut::Modified on the solid) to see what replaced it. Ruled out
  since: the kernel's grouping (one OCCT build on that piece, profile), the open-solid fallback of
  BOPAlgo_BOP::BuildShape (CheckArgsForOpenSolid: every argument here closed). Hypothesis to test next: the
  kernel runs OCCT non-destructive (SetNonDestructive), so where an input's edge or vertex must take a larger
  tolerance (a B-spline section, 5e-6 or more, ending on the cylinder's rim) OCCT works on a copy of that
  argument and the original solid keeps no record: Deleted then, by where sections end, not by caps. The whole
  knurl (one scaled OCCT cut in the model) says Modified, as ours; our early "tools coincide" decline stays until
  this is understood.
  Answer (23:45): neither. Plain OCCT (BRepAlgoAPI_Cut, serial or parallel, non-destructive or not) says Modified on
  the 10-groove piece too. The Deleted is the kernel's: op_boolean builds a cut by 8 or more tools that fall into 2+
  disjoint groups with core's CutByFaces (the cut from the faces' classification, no split solids), whose history
  has no record for the target's solid (Deleted); where that build fails it builds again the usual way (Modified).
  5 grooves: under 8 tools, Modified; 10 / 26 grooves and 6cf0bfdc (14 tools): CutByFaces, Deleted; the whole
  knurl: CutByFaces failed, Modified. So "tools coincide" (its comment: "OCCT may build the object's solid anew")
  was most likely this too. Request to core below; until then noshape's Boolean can't match today's record on such
  cuts without knowing whether CutByFaces succeeds.
- perf.sh, vibe3, main 1009c879 (bool_patterns, one run; ours / OCCT's alone on the same inputs, faces and volume
  the same): pocket grid 23 ms / 396 ms (17×), gear slots 10 / 189 ms (19×), helical grooves 243 / 455 ms (1.9×;
  before this work OCCT's after a decline), radial holes 140 / 247 ms (1.8×; likewise), holes on a circle 1.2 / 1.3
  ms (the through-holes path), crossing helical grooves 8.1 s both (OCCT's, "tools coincide"; OCCT's result
  invalid). The knurl's cut: OCCT's (183 s here at scale 1, 137 s at the model's mm scale per lead), declined in
  0.1 s.
- perf.sh: both bool_patterns runs (cc8837a7, 620d5eaf) ran no test ("0 passed, 175 filtered out": the binary of
  another commit; the ~/perf/wt race in Requests), so vibe3 numbers for the pattern set are still to come; the
  numbers above are this machine's (relative only).
- Next for the knurl (lead 17:45): its end caps (the "tools coincide" history) and the 26-groove piece's volume
  (1.4e-9 off: OCCT seeds each section with the pave filler's edge/face points); then the period solve.

## Patterned tools, step 3 (10-31 UTC)
- OCCT's sections of a copied pair are now made once on the Rust side (`canon_secs`) and moved there; the C++ side
  only records each moved section (the one moved from, the motion, which faces are shared) and builds its curves
  when the build takes an edge from it (`brep_occt_moved`): most moved sections end up in no edge. The pair moved
  from takes its own sections from the same cache (no second OCCT call).
- Sections on another argument's face moved: the check that they stay clear of the face's boundary only counts
  the boundary the motion doesn't keep (a whole circle about a rotation's axis, a shaft's rim, is the same there)
  and tests the curves against it exactly (crossings, ends, the seam) instead of 33 points each; curves moved onto
  a cylinder or cone go by whole turns into [0, 2π) as OCCT's, and the seam-angle check allows 1e-7 rad of rounding.
- The classification's ray against a Bézier piece: pieces the ray passes clear of (by the control points'
  extent across the ray) or that lie behind the point skipped; Newton stops once a crossing in the piece is found
  or the steps leave the piece by more than its size. Regions' points inside on all threads.
- psui: `SectionSource::sections` lost its `moved` argument; a new `SectionSource::moved(ids, a, b, &MovedPair)`
  gives the ids of sections moved from the given ones. Nothing else public changed.
- Replays, all 0 differ, the counts unchanged (main 776, corpus2 553, corpus3 556, corpus4 600, corpus5 717,
  corpus6 662, Shell's 78, direct 58).
- The knurl, with the "tools coincide" declines lifted (not landed: waits for core's CutByFaces record), on vibe3:
  op_boolean 49 → 38.6 s, the Rust Boolean 38 → 27.7 s (OCCT's own: 137 s); result valid, 10362 faces, volume
  9.427289e-5 as live. Left: pairs' sections 9.1 s, crossings 4.0 s, splits 1.8 s, regions 6.5 s, classification
  5.4 s, then the unify's USD 10.7 s (Requests). Next: one period's crossings / classification reused for its
  copies, exact sections of helix-swept faces.

## Patterned tools, step 4 (10-31 UTC): the Rust Boolean's own time on many free-form faces
- Profiled on the knurl (local, perf): after step 3 the time was ours, not OCCT's. Changes, each giving the same
  results (bounds only skip work whose outcome they decide; replays below):
  - OCCT's sections the pairs will ask for (a copied pair's: the pair it is moved from) made ahead on all threads
    (`SectionSource::ahead`, `brep_occt_ahead`); asked for in the same order as before, so ids and curves are the
    same. Knurl: 5.1 s serial → 0.6 s.
  - A 2D curve's nearest sample (`param_of`): samples on spans whose box is farther than the nearest yet skipped;
    whether two curves run together decided after 5 of 9 points (and a point clearly farther than the other's span
    boxes not measured); a ray against a curve: samples on spans clearly on one side of its line not evaluated, a
    curve wholly on one side not sampled.
  - A section clear of a face's boundary: the 50-step search near a sampled minimum skipped where the curve's part
    there (its box, de Casteljau) can't come within the band.
  - Section ends' lengths: a B-spline's chord longer than the tolerance decides it (no 400-point quadrature).
  - Faces' pieces on all threads (applied in face order); `free::apart` found once per pair; patches' boxes in runs
    of 16 for rays; NURBS evaluation on short arrays (no 26 × 26 table cleared per call), points without the
    derivatives.
- Knurl, local (loaded machine; Rust Boolean, the "tools coincide" declines lifted as in step 3): 48 → 18.7 s
  (pairs 16 → 6.5 s, crossings 5.5 → 1.6 s, regions 14.7 → 2.4 s, classification 6.9 → 3.3 s); the same result
  (9.427289e-5 m³, 10362 faces, valid). vibe3 numbers below once measured.
- psui: `SectionSource` has a new provided method `ahead(&self, pairs)` (default: nothing). Nothing else public.
- Replays, all 0 differ, the counts unchanged (main 776, corpus2 553, corpus3 556, corpus4 600, corpus5 717,
  corpus6 662, Shell's 78, direct 58).
- Exact sections of helix-swept faces: not worth it for time now (OCCT's sections are 0.6 s of the knurl on all
  threads). Next: one period's classification for the rest (the arrangement the same under the pattern's rotation),
  then the same for the sections' places and the faces' pieces.

## Patterned tools, step 5 (10-31 UTC): one period's work for the rest
- Where the whole arrangement is the same under a rotation (`Ctx::symmetry`: the largest family of copies'
  motions are rotations about one axis by whole steps of a turn; every member of every family taken onto another
  member by each power, checked with the copy tolerance; every other argument kept: its faces' planes or cylinders
  kept, its edges whole circles about the axis or seams), found from geometry alone:
  - Classification: only the pieces whose point lies in the first period (by angle about the axis) are cast rays;
    any other piece takes the states of the piece its point turns back into (on the face its face turns back to;
    the same loop lengths, its vertices' middle where this one's turns back to, within 10 tol), the solids mapped
    the same way. A piece's state against a solid is a property of a point and that solid, and every point inside
    a piece has the same states. Not found: rays as before.
  - Places of sections' parts and of edges on faces: a part of a section moved from the same OCCT section (the
    curve records the id it is moved from) as one in the first period, on the faces those turn into, with the same
    range, or an edge on a face that is the turned image of one in the first period with the same parts, takes
    that one's intervals: the parameters as they are, its points turned, its vertices the face's (or edge's) at the
    turned place. Anything not matched, or whose turned ends don't fall on vertices there, is found as before.
- Also: a 3D B-spline's nearest point skips samples on spans whose box is farther than the nearest yet (the same
  sample, the first of equals); an edge's length found only when a vertex near it asks; section ends: equal
  parameters and a polynomial curve's control polygon shorter than the tolerance decide "the same point" without
  the quadrature; `free::apart` for all pairs on all threads before the pairing; the arrangement's half-edge
  directions and cycle areas on all threads.
- Knurl, local (loaded; Rust Boolean with the "tools coincide" declines lifted): 18.7 → 10.9 s: places 5.3 → 0.6 s
  (63k of 70k parts turned), classification 3.3 → 0.4 s (57.9k of 59.1k pieces turned, 1.1k by rays again), the
  cylinder's side face's arrangement 3.4 → 0.9 s. Same result (9.427289e-5 m³, 10362 faces, valid).
- bool_patterns (each still compared with OCCT): the symmetry is found in helical grooves (12 steps), radial holes
  (24), gear slots (40), not in the pocket grid (a linear pattern); results as OCCT's.
- Replays, all 0 differ, the counts unchanged (main 776, corpus2 553, corpus3 556, corpus4 600, corpus5 717,
  corpus6 662, Shell's 78, direct 58).
- psui: nothing public changed.

## Patterned tools, step 6 (10-31 UTC): crossings, splits and faces' pieces by the period too
- vibe3 (perf.sh, e2e43802 = step 5 plus scratch timers; the knurl with the "tools coincide" declines lifted):
  op_boolean 18.3 s, of which the Rust Boolean 7.3 s (27.7 s at step 3, 38 s before step 3) and unify.usd 10.7 s
  (Requests). Phases: pairs 2.0 s, crossings 1.3, splits 1.1, faces' pieces 1.7, classification 0.3, the result's
  faces and build 0.8.
- With the rotation found (step 5):
  - Crossings of curves on a face: a copied tool's face whose curves are its family's first member's face's turned,
    one for one, crosses where that one does: its crossings mapped by curve (each recorded on the earlier of the
    two in this face's order, in the order its own pairs come); each curve's length found once when asked.
  - Faces' pieces: such a face whose split curves are the first member's face's turned has that face's pieces, the
    curves the ones here, their points turned, its vertices inside likewise, in the order its own arrangement gives
    (each loop from its least half-edge, holes and pieces in the order of theirs: the arrangement finds a cycle
    first at its least half-edge); faces with seams found themselves.
  - A section's curve is split once for its two faces' lists and the sections' list (it was split three times).
- Knurl, local (loaded): Rust Boolean 10.9 → 9.3 s (faces' pieces 2.3 → 1.05 s, splits 2.8 → 1.7 s, all 874
  copied faces' pieces turned); the same result (9.427289e-5 m³, 10362 faces, valid).
- bool_patterns all as OCCT's (local: pocket grid 86 ms / 779, helical grooves 190 / 765, radial holes 222 / 504,
  gear slots 48 / 450; holes on a circle 2.3 / 2.5 by the through-holes path; crossing grooves still OCCT's).
- Replays, all 0 differ, the counts unchanged (main 776, corpus2 553, corpus3 556, corpus4 600, corpus5 717,
  corpus6 662, Shell's 78, direct 58).
- psui: nothing public changed.

## Patterned tools, step 7 (10-31 UTC)
- The periods start at 0.382 of a step from the frame's x axis: on the knurl the axis frame lined up with the
  pattern's mirror planes and 4144 section parts had their middles exactly on a period's start (no period, found
  directly); now none: parts found directly 6868 → 2747 of 70k, pieces cast rays 1174 → 798 of 59k.
- Whether an edge lies on another argument's face, where that takes points along it (free-form faces, B-spline and
  elliptic edges), found for all candidate pairs on all threads before the pairing (`on_free_surface`); the
  pairing takes the answers in its own order. The pairing's serial loop: edge_on_surface 79 → 35 samples.
- New T1 test `bool_patterns::radial_holes_by_one_period` (12 radial holes; the rotation found; as OCCT's, 0.85 s).
  New T2 bench `four_hundred_fins_on_a_plate`: 400 box fins fused onto a plate: ours 0.54 s / OCCT 5.3 s (9.8×,
  local), 2006 faces, the same volume (no rotation there: the Rust Boolean's generic path).
- Replays, all 0 differ, the counts unchanged.

## Patterned tools, step 8 (10-31 UTC)
- vibe3 at step 6 (perf.sh, 9f3e0b21): the knurl's Rust Boolean 5.7 s (pairs 2.0, crossings 0.92, splits 0.91,
  pieces 0.7, classification 0.29, build 0.69); op_boolean 16.7 s, unify.usd 10.75 s of it.
- Split points by the period: an input edge, section or curve on a face in a later period whose ends and middle
  turn back onto one in the first takes that one's split points turned (the vertices there; its own ends checked);
  any other found. Knurl local: 4.6 → 1.7 CPU-s.
- Crossings by the period, curve by curve: a curve on a copied tool's face (onto its first member's face) or on a
  face the rotation keeps (onto the face's own, first period) takes its image's crossings, the curves it crosses
  turned onto this face (one split here by the cylinder's seam line: the curve at its turned ends the crossing lies
  on); the images are crossed with all their face's curves for that; each pair kept by the earlier of its two, in
  the order the direct loop gives. Knurl: 3854 of 4074 curves left after step 6 turned (the rest are the seam-split
  ones, found directly).
- The same result (9.427289e-5 m³, 10362 faces, valid); replays all 0 differ, the counts unchanged.

## Unions with coplanar neighbouring faces (skasm's rows 141, 142, 144; 10-31 UTC)
- RUST-UNION-MERGES-DISJOINT-FACES: on boolgrid 15 #42 the cylinder's bottom circle touches the block top's edges
  at (5, 0) and (15, 10); the line/arc discriminant came out a hair below zero, so no vertex there and the three
  bits of the top stayed one face (a loop touching itself, not valid): Rust 12 faces after the unify, OCCT 14.
  Now a straight and a round 2D curve within the tolerance of touching meet where they are nearest (two round
  ones likewise), a vertex as OCCT's. Two more found on the way, in the same recorded grid cases: a loop through one
  vertex twice (a hole touching the boundary) built on a reversed face in an order that chains (BRepCheck
  rejected it), and a new vertex on an edge lying on a face is one of that face's vertices "on" it, not one it
  generated (OCCT's history; a Cut's history differed too).
- qa's seeds recorded on the OCCT path (boolgrid 15 × 43, 38 × 13, 68 × 26: 54 Booleans): before 41 taken, 4
  differ; now 42 taken, 0 differ. The five cases in kernel tests/data/bool (T1 bool_corpus_small_cases).
- Replays, all 0 differ, one more taken in six corpora (main 777, corpus2 554, corpus3 557, corpus4 601, corpus5
  718, corpus6 663; Shell's 78, direct 58).
- boolrand 17 #0 (OCCT's "no geometry change"): rebuilt by hand from Rust's exact volume (r5 cylinders 1 mm apart,
  heights 20 / 10, bottoms on Top, and variants): both paths agree on main; not reproduced.
- vibe3 (perf.sh, steps 1–8): the knurl's Rust Boolean 4.8 s (38 s before step 3), op_boolean 15.75 s of which
  unify.usd 10.7 s (Requests).

## The knurl's cut through noshape's Boolean (10-31 11:20 UTC)
- The "tools coincide" declines on a patterned cut are lifted: the planar-cap check is gone, and in a cut a piece on
  the object and a tool, or an object's piece on two tools sharing a cap there, is taken as on one (the object's, or
  the one facing its way). core's CutByFaces Modified fix (2c6e89c2) made the history agree.
- Found on the way: the period work turned a curve's split points and a copied tool's face's inside vertices from
  the first period, so an input vertex the rotation doesn't keep (the cylinder's seam vertex at its bottom rim) was
  missed by curves and faces in a later period. Now such vertices (input vertices whose turned point is no vertex)
  make any curve through them and any copied tool's face whose box holds them be worked out directly.
- Against OCCT (T2 `bool_patterns::the_b94_knurl_by_noshapes_boolean`, both ways, minutes): the same solids,
  faces, edges, vertices, validity, every history record. Two kinds of difference, neither ours to fix:
  - OCCT through op_boolean (64+ tools at the working scale) runs on scaled inputs and scales back, so the vertices
    it keeps come back new and its history says Modified (the seam vertex; after the unify that also showed as 27
    records of the seam edge's vertices). With NOSHAPE_BOOL_NO_SCALE=1 OCCT says Kept, as ours: the test compares
    with that.
  - Section edges: OCCT computes each groove's sections apart, ours turns one period's. Every OCCT section edge
    lies on ours within 2e-8 m (sampled 3000), but their curves are parameterized otherwise, so the replay's
    parameter-midpoint signature differs by 3–5 µm on 8200 records. The test places an edge by its ends and length
    instead. 22 crossings of the two groove families on the mirror planes, where the curves meet nearly tangent,
    are 11 µm apart along them (ours on the plane, OCCT's 2e-9 m off it): edge ends within 20 µm there. The volume
    differs by 1.1e-8 relative (OCCT's own moved 3.6e-9 between the recording and now): to 1e-7. The pattern bench
    `the_b94_diamond_knurl` compares the knurl's volume to 1e-7 too.
- Vertex tolerances: ours 5e-6 on the free-form sections' vertices (OCCT's pave-filler floor for free-form
  sections, as our section_tol), OCCT's ~1e-7 there only because op_boolean ran it scaled to mm (5e-6 mm). So it
  differs only on 64+-tool cuts at the working scale, which used to go to OCCT's scaled path. Not changed.

## The whole knurl test on vibe3, where the time goes (10-31 12:45 UTC)
- perf.sh at 909faec5 (the lift, = main bbbf7e82): the test 42.96 s (hourly baseline ~162 s before); the diamond's
  regeneration **18.3 s** against live's 49 s, the process growing 0.61 GB (4.45 GB before); straight 0.6 s.
- Breakdown (local, loaded, NOSHAPE_KERNEL_PROFILE + NOSHAPE_FS_PROFILE=1; proportions only): the regeneration
  37.8 s = op_boolean 33.1 s (unify.usd 25.4 s, core's guard, being made cheaper by core; noshape's Boolean
  8.8 s, of it the brep part 7.4 s, the build 1.4 s) + op_unify 1.9 s + everything else < 1 s (FeatureScript, the
  148 groove sweeps, transforms, helix: ~0.3 s together). After the regeneration the test's own checks:
  `volume()` 54 s (the segmented Gauss integral over 10k B-spline faces, one thread), meshing 1.4 s. So on vibe3
  ~24 s of the 43 s is the test's volume check, not the feature.
- Then: volume_props' faces on OCCT's pool, summed in order (bit-identical on both knurl results; 56 → 8.3 s
  here).

## The knurl at main (vibe3, perf.sh at 88c82e6f, 10-31 13:59 UTC)
The whole test **21.40 s** (162 s this morning); the diamond's regeneration **14.3 s** (live 49 s), growing the
process 0.62 GB; straight 0.6 s. The knurl target is met: the regeneration and the whole test are both under live's
49 s.

## A cone's (or a free-form surface's) apex made anew (10-31 UTC)
- Where another argument passes through a degenerate edge's vertex (a cone's apex, a corner patch's pole), OCCT
  makes a new vertex and a new degenerate edge there, the input's Modified into them. Ours now does too (it
  declined "a cone's apex changed"): the C++ build makes the degenerate edge (no curve of its own), its curve on
  the face the input's, its range that curve's (piece_pcurve and the free-form faces' loop).
- The 7 distinct cases (20 recorded): all taken, 0 differ incl. the unify check. Replays all 0 differ: main 782,
  corpus2 555, corpus3 558, corpus4 606 (+3), corpus5 727 (+7), corpus6 672 (+7), Shell's 81 (+3), direct 58.
  T1: e45f5fbf794b1d88 in tests/data/brep.
- Then (10-31 15:27 UTC): planes through a cone's apex. A plane through the axis and the apex (its two rulings)
  and a plane touching the cone along a ruling through the apex no longer decline: an input apex edge is split
  where curves on its cone come to the apex (`apex_parts`: a whole cone's from its seam round, a part's between
  the face's own rulings into and out of the apex), each piece `Geo::Apex { a0, a1 }` (its angles; NaN for an
  input's whole edge) and its own degenerate edge in the result, the input's Modified into each, as OCCT's. The
  C++ build gives a new degenerate edge on a cone its curve once its wire is made (`apex_pcurves`: along the
  apex from where the edge into its vertex ends to where the one out starts, the way the wire runs it).
  Public type change: `noshape_brep::Output::edge_apex` (per edge, a piece's angles; the bridge passes them in
  the curve's slots); blend.rs's Output gets it empty. 19 of the 27 recorded cases taken, 0 differ; of the rest 6
  stop at "a vertex inside coincident edges of two other arguments", 2 at a plane across the axis through the
  apex. Replays all 0 differ: main 786 (+4), corpus2 559 (+4), corpus3 562 (+4), corpus4 610 (+4), corpus5 746
  (+19), corpus6 676 (+4), Shell's 81, direct 58. T1: 079531266b0370d5 (a drilled hole's tip cut by two planes
  through its axis), 2b1c8fd20fcce7d5 (a plane along a ruling) in tests/data/brep.

## The knurl at 8d4cf857 (vibe3, 10-31 16:17 UTC; with core's cheaper unify)
The hourly baseline: the whole test 15.96 s (162.88 s at 09714588 this morning). Profiled there (KNURL=diamond,
NOSHAPE_KERNEL_PROFILE, the machine held): the regeneration 8.8 s (live 49 s), the feature 7.9 s = op_boolean
7.24 s (noshape's Boolean 4.93 s: its brep part 4.35, the build 0.57, OCCT's sections ahead 0.57; unify.usd 2.0 s:
USD 1.32, its copy 0.35) + op_unify 0.1 s; the rest of the test (its volume, meshing, the straight knurl) ~6.5 s.

## qa's documents, Rust on against off (10-31 16:51 UTC)
A release CLI of main (ede3be15, /root/bool-scratch/noshape-cli-main), every part's volume and face count of qa's
sweep documents (/root/bool-scratch/sweep.sh: live-30, big-plate, fins-400, the qa-1026 and ticket documents,
mirhole, halfmir, rust-mirror-min, ...; 22 documents): identical with NOSHAPE_BOOL_RUST=1 and =0 but
cli-draft-segv's Part 1, 112330.9947 against 112330.9948 mm³ (9e-10 relative). mirhole 14429.2037 / 7 faces,
halfmir 7214.6018, rust-mirror-min 1618.9016 / 19: their right values. RUST-SHELL-C79E doesn't reproduce (TICKETS).

## Sections split within their own error (11-02 10:00 UTC)
- A section (and a section added to a face as its own curve) is split at a pool vertex within its OCCT error
  (`acc`) off it, the ends kept at the Boolean's tolerance (split_reach): two sections crossing in a face's plane are
  that far apart in space at the crossing. Knurl Ø63 × 17 CP12 45/45 taken (7992 faces).
- The 30 TPI corner-edge class (Ø25 × 10 CP30 45/30) narrowed further: the cylinder's section is split at X (the
  crossing found in flank 165:0's plane), but X in space is 3.9 um from the flank-flank section, its nearest point
  being that section's end P on the corner edge; so the section's curve on the face and in space disagree by up to
  its 5 um tolerance near P, and X is P within it. Snapping X onto P (find_tol with the section's tolerance) made
  it worse (Ø63 × 17 declined again; the cylinder's section then vanishes from the flank's piece). Not symmetry:
  the same with it off (bool-a NOSHAPE_X_NOSYM).
- Not changed (looked at): "an edge nearly on another argument's face" in Shell's fuses is edges starting on a face
  and leaving it at a tiny angle (0 to 35 um along them): OCCT's partial common block, a feature; "a point near two
  vertices": mostly parts with absurd tolerances (vertices of 1.1 m, 7 cm, 4 cm), rightly declined.

## Tolerant faces; Shell's tools at their real tolerances (11-02 09:00 UTC)
- rust_boolean.cpp's tolerant-input check: only tolerant faces are checked against the other arguments (nothing in
  a face's grey band, the other's own tolerance counted up to the ordinary); tolerant vertices and edges go by their
  own rule as before. Before, any tolerant face made every tolerant vertex and edge go through the check too, and
  past 64 of them it declined. T1 a_part_with_a_tolerant_face_and_many_tolerant_vertices.
- shell.cpp by_slabs: every tool not sharing a vertex with the part gets BRepLib::UpdateTolerances (OCCT's
  constructions leave faces at 1e-5 to 1e-4 whose real deviation is 1e-7; another tool's vertex 4 um from such a
  face is on it by OCCT's rule, so the face's nominal tolerance can't just be ignored). Shell's tests pass.
- Shell's 30 seeds again (vibe5 ~/bool-fuzz/sh2): "a tolerant input" 18 → 0, a plane nearly along a cylinder 11 → 6,
  but those fuses decline next for: a point near two vertices 7 → 19, an edge nearly on another argument's face
  8 → 15; declined fuses 141 → 145 of 175 Shells by slabs. Top now: seams apart 24, a point near two vertices 19,
  an edge nearly on another argument's face 15, two edges leave a vertex in one direction 12, arguments share
  vertices 10, a face of revolution 8, a non-manifold result 7.
- Seams apart, with the end check off (bool-a NOSHAPE_X_NOSEAMS, Shell's recorded fuses vibe5
  ~/agents/bool-corpora/seams2, 14): 5 equivalent, 9 differ (history records in 7, unify in 1, and bf976715's volume
  6.154e-5 against OCCT's 5.755e-5: a wrong result). So the check guards real faults: next is finding them.
- Corpora unchanged, 0 differ. Scratch: /root/bool-scratch/bin/syncdbg.sh resets bool-a to bool-b plus the debug.

## A plane nearly along a cylinder; Shell's slab fuses re-measured; seams apart looked at (11-02 07:30 UTC)
- plane_cyl declined planes with |n · a| < 0.25 (within 14.5° of the axis: drafted faces cutting holes); OCCT's
  ellipse is taken down to 1e-3 now. nfall's 9 such taken (OCCT's results), corpus6 +6, 0 differ. T1
  a_plane_a_few_degrees_off_a_cylinders_axis.
- Shell's slab fuses over core's 30 seeds (3000-3029 × 60) with this build (vibe5 ~/bool-fuzz/sh/err-*.txt):
  "an offset face" gone; quick_boolean's declines now: one cylinder in faces of two inputs, their seams apart 24,
  a tolerant input 18, two edges leave a vertex in one direction 11, a plane nearly along a cylinder 11 (fixed
  here), arguments share vertices 10, an edge nearly on another argument's face 8, a face of revolution 8, a point
  near two vertices 7, then ≤ 5 each. Those fuses recorded (only the ones OCCT built) into vibe5
  ~/agents/bool-corpora/shf/<seed> (shrec.sh) for replays.
- Seams apart (the end check before unify): tried instead unifying our result in rust_boolean.cpp (twice, as
  op_boolean and the model do) and declining only an invalid one. 38b2cd00 / fd7ef128 still decline (their unify
  is invalid; a BRepBuilderAPI_Copy of the result unifies validly, so it is something in our result's sharing),
  corpus6 and sh-new2 +1 each, but that case (f864957f) differs from OCCT in one history record (an input vertex
  at (50, 25, 1.5) mm: ours Modified into a new vertex of tolerance 1e-5, OCCT's keeps the input vertex, tol
  2.1e-7, and says Deleted for that argument), and corpus3 lost one. Reverted; next.

## Fuzz declines: sheets on one torus; a tolerant vertex at its reach (11-02 06:30 UTC)
- "sheets that meet": all 8 left were two sheets on one torus (fillet faces, mirror's overlap tests; one pair
  with the axis reversed): taken as coplanar ones (T1 a_common_of_two_sheets_on_one_torus).
- "coincident pieces differ" (2 of seed 1195's): a vertex of tolerance 1e-4 lies 1e-4 from the other sheet's edge
  (on it by the sum with the edge's tolerance) while the edges' crossing beside it, a new point counted with
  tolerance 0, rounded just past 1e-4 and stayed its own vertex. Pool::find now gives a new point half the
  Boolean's tolerance (the inputs' largest, its curves'), OCCT's new vertex's (T1
  a_tolerant_vertex_at_its_reach_from_an_edge). Two others of these now build but fail the validity check.
- nfall: 35 of 155 taken (OCCT's results). Corpora unchanged, 0 differ (both).
- "two edges leave a vertex in one direction" (knurl Ø10 × 10 45/45, vibe5 kg-10-10-CP12-45-45): on the cylinder,
  six rotated pairs of new vertices 0.64 um apart (1712-1717 vs 1727-1732, z 20 um) whose curves' ends in the
  cylinder's plane are 4e-14 apart: one of each pair's curves doesn't end at its own vertex in 2D. Likely the
  pieces turned from the first period (sym); not fixed. The fuzz ones are mostly seed 1208's 1.34 mm vertices.
- Shell: shell-11's slabs of 1.5e-3 come from faces whose vertices already have it; the part has 6.9e-4 on
  entry to op_shell (the dump file 1e-4), so not Shell's methods. Re-measuring the slab fuses over core's 30 seeds
  with this build (vibe5 ~/bool-fuzz/shrun.sh, logs ~/bool-fuzz/sh/err-<seed>.txt).

## The knurls' corner-edge non-manifold class, narrowed (11-02 05:30 UTC)
- Ø25 × 10 CP30 45/30 (vibe5 ~/agents/bool-corpora/kg-25-10-CP30-45-30): at 30 TPI a groove's side corner edges
  (flank 0 / 1, flank 2 / 3) run 3 um outside the cylinder (r 12.503). Where two grooves cross (their sections
  coincide at the crossing height z_c), their corner edges cross each other at P (r 12.503, z_c ± 0.28 mm), where
  the two flanks' section (e.g. 84:0 × 185:0, from the V-bottoms' crossing at z_c up to P) ends. The cylinder's
  section with the flank runs 3.2 um inside the corner edge (OCCT tol 2e-8, ours 5e-6 as the pave filler's) and
  crosses that flank-flank section at X, 2.4 um from P. On the flank the crossing is split into the cylinder's
  section at X, but the flank-flank section's crossing is merged into its end P (within its 5 um), so the two cross
  without a vertex: the cylinder's section is a slit in one piece (both ways in its loop), the strip between it
  and the corner edge (outside the cylinder) joins the kept piece, and the corner edge piece is used once.
- Tried, not landed: a section's crossing snapped to a vertex within the section's tolerance (X onto P): the
  cylinder's section then disappears from the flank's piece instead (likely merged with the corner edge piece
  between the same two vertices). Nor did merging sections into edges within the section's tolerance. Next: make
  the flank-flank section split at X too (don't merge a crossing into a curve's end when the other curve is split
  there), or treat P's neighbourhood as one vertex on all curves alike.

## qa's NOSHAPE_BOOL_FALLBACK=0 fuzz again; sheets on one cylinder (11-02 04:30 UTC)
- qa's 29 BOOL-DECLINE tickets (seeds 1191-1210 on 45424566: 755 declined calls). The same seeds with a CLI of this
  worktree (vibe4 ~/bool-fuzz/nfrun.sh: clifuzz_nf.py, NOSHAPE_RECORD_FAILED; cases in
  ~/agents/bool-corpora/nf/<seed>, all in nfall): 155 declined calls, by seeds hit: coincident pieces differ 5,
  two edges leave a vertex in one direction 3, sheets that meet 3, an edge on another argument's free-form face 3,
  then 2 each (an ellipse across a cylinder's seam, an edge off a plane past its tolerance, a plane nearly along a
  cylinder, a piece across its surface's seam angle, a non-manifold result, a face lost) and 1 each (no clean ray,
  an invalid result, an edge nearly on another argument's face, an arc across a cylinder's seam, a tolerant input,
  a tiny arc, a new edge on a free-form face, a face's vertex off its plane, a conic edge, a closed edge). By calls:
  coincident pieces differ 36, a closed edge 26 (one model: a 0.19 mm line whose two ends are one vertex of
  tolerance 1.9e-4, a bad input), an invalid result 13, ellipse / arc across a cylinder's seam 12 + 12.
- Fixed: "sheets that meet" for two sheets on one cylinder (a hole's wall in pieces, 12 calls of seed 1192):
  coaxial cylinders with one radius go as coplanar sheets do, apart past the grey band as crossing ones. T1
  a_common_of_two_sheets_on_one_cylinder (OCCT's result).
- Fixed: an arc tangent to a line at a vertex a tolerance off the tangent point (two coplanar sheets, seed 1195):
  the two leave the vertex 2e-5 apart in angle, past arrange's 1e-9 for one direction, so they were ordered the
  wrong way round it and the face's main piece was lost ("coincident pieces differ"). Directions within
  1e-9 + 1e-6 (|k1| + |k2|) are one now, ordered by turn. T1 an_arc_tangent_to_an_edge_at_a_vertex. Corpora: +16
  taken (bool-corpus 822, corpus5 803, ...), 0 differ. nfall: 25 of
  155 taken now (all OCCT's results exactly); left: coincident pieces differ 28, a closed edge 26, an invalid result
  13, two edges leave a vertex in one direction 10, a plane nearly along a cylinder 9, sheets that meet 8, a piece
  across its surface's seam angle 7, a conic edge 6, and 1-3 each. Many of the rest are one fuzz model (seed 1208's)
  whose vertices carry 1.34 mm tolerances.
- Next, "coincident pieces differ" (smallest: nfall/a0c5604f279369fa, a common of two coplanar sheets on y = 40: a
  face of two arcs, arc0 on the circle about (-34.897, 22.584) and arc1 about (-18.897, 22.584), r 14.069 both
  (two discs' union), against a whole circle about (-34.897, 23.584), r 14.069, its seam vertex (-34.897, 9.515)
  inside the face; x, z in mm). Found: the circle is split where it crosses arc1 at D (-26.176, 34.623) on the
  face's arrangement (D is on both circles) but at E (-26.408, 34.803) on the circle's own face (E is on the circle
  only, 0.25 mm from D), and arc1 isn't split at D at all; so neither face's loops close (the face's piece A-B-C-D
  has no D-A, the circle's piece is A-B-E) and only one piece is on the other. Cause: the face's vertex A
  (-26.897, 34.157), where arc0 and arc1 meet, has tolerance 1.34 mm (the fuzz part's); E is A projected onto the
  circle (A 0.81 mm off it: on it by OCCT's rule, so the circle is split there on its own face), while D, 0.86 mm
  from A, wasn't merged into A on the face's arrangement. With a vertex that tolerant every crossing within its
  tolerance has to become it (as OCCT's paves do), on both faces alike. NOSHAPE_X_ALLP in bool-a prints every piece.
- Knurl grid at the end of this round (vibe4 ~/agents/sweep/final.tsv, stopped at 188 of 356 (vibe4 is the land server:
  sweeps, corpora and fuzz now run on vibe5 at nice 19, 8 cores; corpora copied to vibe5 ~/agents/bool-corpora), all of today's fixes, 126 of 356 done): 107
  taken; declining: non-manifold at Ø10 (heights 50-100, CP12-30, 30° helix) and CP30 at Ø25 (heights 10-17), one
  each at Ø25 × 71 CP12 45/30 and Ø63 × 17 CP12 45/45, Ø25 × 10 CP20 80/30 (the seam crossing), "two edges leave a
  vertex in one direction" Ø10 × 10 45/45 (pool vertices 4e-14 apart), "an edge against its curve" Ø25 × 25 CP20
  45/45. The Ø10 / CP30 non-manifold edges are short pieces of a groove's corner edge (3 um outside the cylinder)
  where another groove crosses it (not the whole section along the edge: merging sections into edges within the
  section's own 5 um tolerance didn't change them).

## Shell's slab fuses: offset faces; the seeded sections' cost (11-02 03:00 UTC)
- Offset faces (rust_boolean.cpp to_body): a Geom_OffsetSurface is taken as the elementary surface OCCT keeps as its
  equivalent (a plane's, cylinder's, cone's offset), else as a B-spline within 1e-7 of it over the face's parameter
  range (GeomConvert_ApproxSurface, same parameters, so the edges' curves on it stay).
- shell.cpp: a simple-offset slab (MakeThickSolidBySimple) gets BRepLib::UpdateTolerances: OCCT gives its offset
  face its 1e-4 working tolerance (the face's real one 1e-7), which made the whole slab fuse "a tolerant input".
- shell.cpp: a planar slab (a prism) loses the INTERNAL faces its face's internal edges sweep ("an internal face"
  for the fuse; shell-11's part has two such edges); the caps keep the edges, which noshape's Boolean takes. (Copying
  the face without them gave an invalid face for one of shell-11's, UnorientableShape, whatever way it was copied.)
- rust_boolean.cpp: Bézier curves and surfaces taken as the B-splines they are (GeomConvert, same parameters; some
  slabs have Bézier edges: "a curved edge").
- Core's repro dumps now: shell-10's fuse (26 shapes) declines "an edge without its curve on a free-form face" (an
  edge of one slab on another's B-spline face without its curve there); shell-11's (93) "a tolerant input": three
  slabs (all planes, or planes and an extrusion) come out with faces, edges and vertices at 1.5e-3, half the 3 mm
  wall (UpdateTolerances takes the faces to 1e-7, not the edges): which slab construction sets that is next. No T1
  for these (their inputs are Shell's own slabs); Shell's tests pass. Repro: bool-a's scratch example
  (crates/kernel/examples/bool_shell.rs, not landed: SHELLDUMP=<dump without .brep>, NOSHAPE_SHELL_DEBUG=1); the
  slab fuses' inputs: vibe4 /tmp/rb10-26.brep, /tmp/rb11-93.brep (local /root/bool-scratch/kh/).
- The seeded sections' alternating projection built a fresh GeomAPI_ProjectPointOnSurf each step (a grid over the
  whole surface each time): most of a Ø10 knurl's Boolean. Now ShapeAnalysis_Surface: one search, then local steps.
  Ø10 × 50 CP12 45/45 35.5 → 20.8 s, Ø10 × 33 CP30 45/30 97.8 → 16.0 s (loaded vibe4), same results.
  (NOSHAPE_KERNEL_PROFILE's boolean.rust.sections, 18 of the 34 s, counts the seeded calls too; gdb samples on a
  local copy of the test binary found the projections.)

## Knurl sweep: tilted caps, slivers between caps (11-02 02:00 UTC)
- A tool's plane face 1e-9 to 1e-4 rad off an object's plane face, its edges within the tolerance of that plane, is
  put on it before the pairing (boolean.rs, before `symmetry`): the knurl tools' end caps are 2.3e-8 to 2.8e-5 rad
  off the cylinder's end planes (the sweep's end sections), past plane_plane's 1e-9 parallel test, so not coplanar,
  and the end faces' pieces over them were ray-classified on them ("a piece's point on another solid's face", all
  of Ø100 × 10). T1 caps_a_hair_off_the_end_planes (OCCT's result exactly). Corpora unchanged.
- A piece whose point is in the grey band of a coplanar face's boundary (a sliver a micrometre wide between two caps'
  edges for 330 um, Ø10 × 25 CP30): on that face or not by a curve of the piece that is a piece of that face's edge,
  the way each runs along it (and the two normals), as OCCT's same-domain faces go by shared edges. T1
  slivers_between_two_caps: OCCT's cut of the two keeps one groove (1962.198 mm³); ours 1960.905690, the grooves'
  own cuts and OCCT's overlap 1960.905682.
- Grid (size × TPI × angle, vibe4 ~/agents/sweep/grid3.tsv, 94 of 252 done before these two): 81 taken. Still
  declining, by cause: (a) sections of the cylinder within a few um of a tool's corner edge that runs 3 um outside
  the cylinder (CP30 and Ø10 at 30° helix, mid-height): merged into the edge by get_sedge (reach from OCCT's
  accuracy), the strip between lost, non-manifold — next: merge a section with an edge only where the edge lies on
  the section's other face; (b) two grooves crossing at the cylinder's seam just above the end face (Ø25 × 24 45/30,
  Ø25 × 10 CP20 80/30): a cylinder piece holds both walls of one groove but its point is outside it; (c) two pool
  vertices 4e-14 apart not merged, two 0.32 um curves from one point (Ø10 × 10 45/45: "two edges leave a vertex in
  one direction"). Slow: Ø10 knurls 50-110 s (3600-8400 faces) on the loaded machine, against 2.8 s for Ø25 × 25;
  to profile.

## Knurl sweep: three more causes fixed (11-02 01:00 UTC)
- The boundary crossings for the seeded sections are now the curves' own closest points (Gauss-Newton on the two
  edges from each chord pair within reach), merged within 10 tol: chords of a 48 mm helix edge reach ~12 um, and
  merging within ten reaches made two crossings 70 um apart one (Ø25 × 34 45/45: tools 30, 51 cross 37 um above the
  end face, mirror images there; their flanks' 70 um sections down to the end plane weren't asked for).
- A piece a few tolerances across (8 GREY tol, its loop's box) gets `deeper`'s point (Ø25 × 43 45/45: two tools' end
  caps' corners overlapping in a 3 um diamond, its first point 0.64 um from the other cap's edge: "a piece's point
  near a coincident face's boundary").
- Rays: `free::ray_hits` tries every start on a Bézier piece (it stopped at the first crossing; a twisted helical
  flank's piece is crossed twice: Ø10 × 25 80/30, a cylinder sliver by the end face taken as in a groove,
  non-manifold), and `ray_state` takes another direction when the nearest crossing's side disagrees with the count.
  The Ø50 diamond knurl: 12.5 s on loaded vibe4, live's faces and volume.
- Ø25 heights 10–60 at 45/45 and 45/30: all taken but 45/30 at 24 mm. Corpora: bool-corpus 820 (+1), the rest
  unchanged, 0 differ. T1 grooves_crossing_37_um_above_the_end (OCCT's cut of the two leaves 29 mm³; against the
  grooves' own cuts and OCCT's (object ∩ one) ∩ other 0.088501 mm³), a_piece_a_few_tolerances_across (OCCT's volume;
  OCCT splits the cylinder's face at its seam where ours keeps one face: 16 faces vs 13), a_ray_crossing_a_twisted_flank_twice.
- Still declining in the size × TPI × angle grid (vibe4 ~/agents/sweep/grid2.tsv, before these fixes: 90 taken, 29
  not), causes found so far: (1) tool caps tilted 2.3e-8 rad from the end plane (Ø100 × 10): past plane_plane's
  1e-9 parallel test, so not coplanar and the end face's pieces over a cap ray-classified on it ("a piece's point on
  another solid's face"); next: planes within the tolerance of each other over both faces taken as coplanar. (2) Two
  grooves crossing at the cylinder's seam just above the end face (Ø25 × 24 45/30, Ø25 × 10 CP20 80/30):
  non-manifold, not yet understood. Recorded: vibe4 ~/agents/bool-corpora/kg-<Ø>-<h>-<pitch>-<tooth>-<helix>.

## The Ø25 × 25 knurl taken: short sections walked from where the faces' edges cross (11-02 00:10 UTC, user's P0)
- The missing curve: tools 28 and 61's ridges (flank 0 / flank 1 edges) cross at z = 44 µm at the same radius; below
  that each ridge is inside the other tool. Flank 28:0 meets 61:0 (and 28:1 meets 61:1) in a 94 µm curve from the
  ridges' crossing down to where their bottom edges cross on z = 0. OCCT's IntTools_FaceFace finds none (its start
  points come from sampling); given a start point on the curve (IntTools_FaceFace::SetList) it walks it at once.
- Fix: `occt_sections` (boolean.rs): where OCCT gives a free-form pair no section, the faces' edges as 64-chord
  polylines (made once per edge) are checked against each other; where they cross at 2–4 places (within the chords'
  sag and the tolerance), `SectionSource::seeded` asks OCCT again with start points: each two crossings' middle
  brought onto both surfaces by alternating projection (brep_occt_sections' new `ends`). Cheap: a first try doing
  OCCT's edge/edge and edge/face on every empty pair cost minutes on the knurl. Moved sections (patterned tools) go
  through it too (the canonical pair's are made by it).
- Results (NOSHAPE_BOOL_FALLBACK=0): Ø25 × 25 45/45 2.8 s in the T2 sweep, 11598.041 mm³, 4479 faces; live 16.8 s,
  11597.649 mm³, 4479 faces (+3.4e-5: live's 45° sweep error, see core's 11-01 20:00 note). Heights 10–60 at 45/45
  and 45/30: all taken but 45/45 at 34, 43 and 45/30 at 24, 46, 57, 60 (was 10 heights; the user's 25 taken).
  Corpora: more taken in every set (bool-corpus 793 → 819, corpus2 587 → 597, corpus3 592 → 602, corpus4 638 → 649,
  corpus5 782 → 793, corpus6 712 → 726, sh-new2 104 → 105), 0 differ from OCCT.
- The two tools alone (T1 bool_patterns::grooves_crossing_just_above_the_end): OCCT's cut of the cylinder by them
  removes nothing (12271.846 mm³ standalone, 12174.95 at the working scale); each alone removes 10.837 mm³, ours
  12250.172 mm³ = the two grooves' own less a 5e-5 mm³ sliver where they cross.
- App-level timed check: tests/perf/knurl-d25x25-45-45.noshape (built with the CLI), ops
  `regen_profile::timed_knurl_d25`: opened and regenerated with its Feature Studio and display meshes, 3 runs, live's
  faces and volume (7e-5), median < 60 s: 5.9 s on loaded vibe4. In perf.yml's timed checks; lead: add it to vibe3's
  hourly baseline (Requests).
- Recorded for the next step (vibe4 ~/agents/bool-corpora/kh<h>-<tooth>-<helix>, NOSHAPE_RECORD_FAILED): kh34-45-45
  fac6ef04, kh43-45-45 8c60693b, kh24-45-30 3c8b7215, kh46-45-30 324c79d1.

## The knurl on a Ø25 cylinder; NOSHAPE_BOOL_FALLBACK=0 (11-01 23:30 UTC, user's P0)
- User (v0.4.19 app, macOS): the B94.6 Diamond knurl, 12 TPI, 100 %, on a Ø25 × 25 cylinder: 45/30 and 80/30 ~3 s, 45/45
  still running after 2 min. Reproduced on Linux (not a platform difference): at Ø25 × 25 noshape's Boolean declines
  45/45 ("a non-manifold result") and OCCT's path runs past 15 min; Ø50 × 50 (our tests, live's) is taken (CLI with the
  app's sketch + extrude cylinder: 45/30 15 s, 45/45 32 s, live's faces).
- Height sweep (Ø25, 10–60 mm in 1 mm steps, NOSHAPE_BOOL_FALLBACK=0, the T2 sweep with KNURL_SIZE): 45/45 declines at
  17, 25, 34 ("a non-manifold result"), 43 mm ("a piece's point near a coincident face's boundary"); 45/30 at 14, 24, 35,
  46, 57, 60 mm; every other height taken in 1–10 s. Ø50 × 50 taken at both.
- Cause (recorded cut 0488ccf3, 74 tools, h25): at those heights a crossing of two opposite-hand grooves falls within
  ~45 µm above the cylinder's bottom face, where every groove tool's end cap lies on the bottom plane. The two tools
  (28 and 61) only meet in that sliver: OCCT gives a 44 µm section of their faces 1, nothing between their side faces 0
  (moved from the first period or asked directly alike), so a side face's piece reaching into the other tool there
  isn't split and is kept as outside (non-manifold). Next: the sliver's dividing curve (the side faces' section near
  their common cap plane, or the tools' cap edges on each other's side faces), then the size × TPI sweep.
  Narrowed (bool-a, debug only): each tool is four swept flanks (faces 0–3) and two caps (4 top, 5 on z = 0). OCCT's
  classifier agrees with ours (cylinder pieces next to edge 14358 IN tool 61, tool 28's flank piece 11708 OUT), so
  28:0 must leave tool 61 between them, through 61's flanks 0 / 1; IntTools_FaceFace returns no curve for 28:0 × 61:0
  / 61:1 / 28:1 × 61:0 / 61:1, on the faces or on faces grown a tenth in u and v; 28:0 × 61:2 / 61:3 aren't near; the
  only sections are on the cap plane (dropped as along 28:0's bottom edge, which lies on 61's cap). So the dividing
  curve is tens of µm long at both faces' bottom edges and OCCT's walking doesn't find it. Fix to try: make it
  ourselves for two free-form faces whose bottom edges lie on one plane inside each other's cap: its ends where each
  tool's flank-to-flank edges pierce the other's flanks (edge / face crossings ours already finds), the curve between
  them marched on both surfaces (a chord sags ~L²/8R ≈ 2.5e-7 over 45 µm on a 1 mm radius: march, don't chord).
  Repro: vibe4 ~/agents/bool-corpora/k25/0488ccf3c42f9c44 (recorded with NOSHAPE_RECORD_FAILED), `why.sh k25
  0488ccf3c42f9c44 NOSHAPE_BREP_VS_OCCT=1`.
- NOSHAPE_BOOL_FALLBACK=0 (user's decision, default stays 1): a Boolean noshape's declines fails at once, OCCT's not run:
  op_boolean throws "noshape's Boolean declined: <reason>" (also on stderr), quick_boolean (Shell's) returns failure as
  OCCT's failing would; the reason shows in the feature's error and FeatureScript's notices (custom host passes it
  through as CUSTOM_ERROR, Extrude doesn't replace it). With it the Ø25 × 25 45/45 knurl fails in 3 s with the reason.
  NOSHAPE_RECORD_FAILED=1 (with NOSHAPE_RECORD_BOOLEANS) records a failed call's inputs too (summary: path failed, its
  error), so every decline becomes a replayable case (replay with NOSHAPE_BREP_VS_OCCT=1).
- The T2 knurl sweep takes KNURL_SIZE=<Ø>,<h> and KNURL_PITCH=CP16 etc. (live's values compared only at Ø50 × 50, 12 TPI).
- Live asked (REQUESTS, bool 11-01 22:00): Ø25 × 25 at 45/45, 45/30, 80/30 and Ø25 × 50 at 45/45.

## RUST-EXTRUDE-SCOPE-SLOW fixed (11-01 20:40 UTC)
- psui's repro (qa's slow1151rep.py on tickets/slow1151-pre.noshape): the extrude's collision check took 237–435 s
  and the extrude failed with noshape's Boolean on. Its two commons (the extrude's tool against a part) went to OCCT
  (ours declined "a point near two vertices") and took 154 s each: the part had two vertices of tolerance 1.34e154.
  Found with a temporary tolerance watch on every op's result: OCCT's own draft (BRepOffsetAPI_DraftAngle, a
  Draft's face on a part built through our Booleans, its input clean at 5.5e-7) returned that, and BRepCheck still
  called it valid. op_draft (features.cpp) now fails a result with a vertex or edge tolerance past a hundredth of
  the part's size, so draft_robust's fallback (the wedge draft) builds it.
- The repro on a CLI of this change: 1.1 s, "No merge scope selected." (as with NOSHAPE_BOOL_RUST=0, 3.2 s). T1
  crash_inputs::a_draft_leaving_a_huge_tolerance_fails_and_the_wedges_build_it (the dumped draft input). Why OCCT's
  draft does that on this input (ours is valid) is open.
- Next (lead 11-01): Shell's slab fuses at the top of the decline list ("an offset face", "a tolerant input"), then
  fins-400's plate fuse with 399 fins.

## Coplanar sheets in a common (11-01 19:40 UTC)
- A common of two sheets (faces, no solids) whose faces pairwise cross or lie on one plane within the tolerance (or
  apart past the grey band) goes through the Boolean instead of declining: each face split by the other's edges on
  their plane, the object's pieces on the tool's faces kept (either facing), the tool's there Modified to them, the
  rest deleted (`sheet_common`; the "coplanar sheets" vertex rule and "sheets with solids" skip it). Model-core's
  face-overlap tests.
- Replays why50: main 819 (+22), corpus2 597, corpus3 602, corpus4 649, corpus5 793, corpus6 726 (+8 each), direct 78,
  Shell's 105; 0 differ. Share of all sets 86.2 % of cases, 81.3 % of OCCT time. "Sheets that meet" 77 → 15.
- T1 bool_patterns::a_common_of_two_overlapping_coplanar_faces (0b6e553b).
- Next: the 15 left (faces not planar, or in the grey band), then fuses / cuts of coplanar sheets (the 10-30 notes:
  0c538d2c and 7895f375 to understand first).

## Where the declines stand, and the next step (11-01 19:10 UTC)
- By OCCT time the rest is mostly bad inputs (self-intersecting Shell tools: e40bb5a1, 419e648e's family; Shell slabs
  of 0.7 mm tolerance: b2511301 / addb9a14; radius-1e-10 arcs: "a tiny arc"; zero-width cylinder strips: "a cylinder
  face without curved edges"), OCCT's choices we can't read off yet (same-domain faces of shared or coincident
  arguments: b7eb72a8, "arguments share vertices"; the seam order unify wants: "seams apart"), and the apex chain
  (b0c36850 / 9f262529 / d5cd8b8d: same-domain faces sharing their splits).
- By count the biggest open group is sheets: "sheets that meet" (77 cases), "coplanar sheets" (38), "a sheet of shells"
  (19): mostly model-core's face-overlap tests, a common of two single planar faces on one plane (OCCT's result: the
  overlap as faces, or nothing; ~10 ms each). Next: coplanar planar sheets in a common: each face split by the other's
  edges in their plane (edge_in_face), a piece kept where it lies inside the other face (2D, as coincident pieces
  are), the kept one the object's as OCCT's same-domain choice, the history as for solids' coincident pieces; then
  the fuse / cut of such sheets, then shells of faces.

## "A non-manifold result": the 30 left, surveyed (11-01 18:50 UTC)
- Debug summary per case (which edges end up used once or twice one way): most leave one piece's whole boundary
  open, i.e. one piece classified or split differently from its neighbours, each for its own reason; the rule "a ray
  meeting two faces at one place is unclean" (/root/bool-scratch/ray-twice.patch) takes none of them alone.
- Looked into: 419e648e & co (a rod cut by a swept square tube: a missed crossing with the tube's B-spline sides or
  its caps crossing, above); e40bb5a1 (a Shell's cut of a box with two blind holes 8.245 mm apart by its cavity, whose
  1 mm-larger hole walls (r 4.175) overlap each other by 0.1 mm: the tool solid intersects itself. OCCT doesn't
  intersect an argument with itself and keeps both whole tubes (its result's two full cylinders); ours classifies
  the overlap by ray parity through the self-intersecting tool and leaves a non-manifold result. A bad input from
  Shell's cavity (core: shell.cpp could check its tool), declining is right.)
- 419e648e's family (five): with nine Newton starts per patch in free::ray_hits (not only Bézier pieces) and the
  unclean-ray rule, still none taken: the rays really cross both caps and no side, i.e. the swept tube's caps cross
  each other (a square swept along a tight path: a self-intersecting tool, as e40bb5a1's). Declining is right.

## Fuzz seed 1144 diagnosed (11-01 18:20 UTC)
- With Shell's direct Booleans kept on OCCT (a diagnostic switch, not landed) the run is clean; with b997c4c3 (the one
  of those still differing) declined it still dies. The runs part at step 2 already: the first Shell's face ids differ
  between the paths (the naming reads the history, and our history differs from OCCT's in the known rounding records:
  a vertex inside coincident edges of three or more arguments, Deleted where OCCT says Modified, fe9670f8), so the
  fuzz picks other faces and builds another model, valid, on which OCCT's own op_prism (a sheet metal extrude) smashes
  its stack at step 51. That is OCCT's fault on a legitimate input (core: Requests); the rounding records are a
  known equivalence (OCCT's choice there depends on rounding), not a wrong result.
- b997c4c3 (the edge through a tolerant vertex off a tilted plane) now declines instead of raising the edge's
  tolerance: OCCT's result there is the exact one (ours was valid but 0.036 % off in volume). T1
  an_edge_through_a_tolerant_vertex_off_a_plane_declines.
- Seeds re-run on a CLI with today's fixes: 979, 1036, 1112, 1092 (twice), 1102 all clean.

## A validity guard on noshape's results (11-01 17:40 UTC)
- rust_boolean.cpp checks every result up to 2000 faces with BRepCheck_Analyzer; an invalid one declines ("an invalid
  result") and OCCT builds the Boolean. Invalid results of ours had gone on into later OCCT operations and crashed
  them (fuzz seed 1144's mirror fuse and Shell common, both fixed at their cause too). Cost on the main corpus: the
  median small Boolean 2.46 → 2.93 ms, the total 4062 → 4230 ms (+4 %); bigger results (the knurl's 17762 faces,
  ~2 s to check) skip it. Replays (why48): counts unchanged, 0 differ, no recorded case declines by it.

## RUST-MIRROR-GPF-DELTA looked at (11-01 17:10 UTC)
- qa's tickets/cli-shell-gpf.noshape recorded with OCCT and replayed through ours (vibe4 bool-corpora gpf): the
  Part mirror's fuse (49eaef95) is where the volume parts: ours 3.252513298621e-5 m³, OCCT's 3.252511742304e-5
  (+0.0156 mm³, qa's delta), same 26 faces. The one face that differs is the overlap's face at y = -1 mm: OCCT's result
  isn't symmetric (that face 1.362608e-3 m² on one side, its mirror image 1.362611e-3 on the other: the curve on it of
  a tolerant arc edge (4.6e-5) ends 2 um off at one side only); ours is 1.362611e-3 on both sides. So the volume delta is
  OCCT's asymmetry; ours is the symmetric result. The face count qa saw downstream (24 against 26) comes from a Shell
  slab fuse (ba295710) that unify merges further on ours (26 / 65 against 29 / 69 before the mirror): no live value for
  this fuzz document to say which is right; left.
- Also taken earlier the same way: the RUST-SHELL-2AFC and fuzz-1144 cases below. For qa: re-run seeds 979, 1036,
  1112 (clean here, no panics, on a CLI of 52fc54ec + the shell fix), 1144 (still dies: op_prism, below) and the
  Shell 6 ticket on the next green.

## Fuzz seed 1144: two invalid results of ours (11-01 16:40 UTC)
- Seed 1144 (qa's CLI fuzz) dies at step 51 with noshape's Boolean on (stack smashing on vibe4, a segfault in OCCT's
  BOPAlgo_PaveFiller::UpdateExistingPaveBlocks inside a Shell's slab Boolean under gdb), clean with it off. Its
  Booleans recorded with ours on and replayed against OCCT on the same inputs (bool-corpora fz1144 on vibe4): two of
  ours were invalid where OCCT's were valid, fed on into later operations:
  - e02980a2 (a mirror fuse): a face with a hole touching its outer boundary at one vertex came out as one wire through
    that vertex twice (BRepCheck_BadOrientationOfSubshape). The build now splits such a loop there into a wire each
    side, as OCCT's builder makes them: taken, as OCCT's.
  - b997c4c3 (a Shell's common): a new edge on a slightly tilted plane ran through a tolerant input vertex (1e-4) 1e-5
    off the plane with a 1e-7 tolerance (BRepCheck_InvalidCurveOnSurface). A new edge on a made plane whose ends lie off
    it past its tolerance now gets the tolerance to cover it (its vertices too; an argument's own vertex isn't
    changed: declined). Valid now; still 0.036 % in volume off OCCT's (OCCT runs the section on the planes' exact
    intersection, ours through the vertex).
  The loop split skips closed edges (a sphere's pole edge starts and ends at one vertex: split off alone it left
  sphere corners NotClosed, corpus4 43442a2f and others; why47 back to why45's counts, 0 differ). T1
  a_hole_touching_the_boundary_is_its_own_wire, an_edge_through_a_tolerant_vertex_off_a_plane (valid, as OCCT's on
  the same inputs).
- After these every recorded Boolean of the run is valid, but the run still dies at step 51 (deterministic on vibe4,
  clean with NOSHAPE_BOOL_RUST=0): NOSHAPE_GUARD_TRACE=1 puts the stack smash in op_prism (features.cpp, a sheet
  metal extrude's prism, std_route::sm_extrude) while regenerating after a suppress, not in a Boolean; its profile
  likely comes from a result of ours (b997c4c3, 0.036 % off OCCT's, is the one left differing). Under gdb on this
  machine the run takes another path (budgets) and segfaults in OCCT's BOPAlgo_PaveFiller::UpdateExistingPaveBlocks
  inside a Shell's slab fuse after ours declined it, not reproducible from the dumped Shell alone (either Boolean):
  a race, as qa saw for CRASH-1092. Requests: core to look at op_prism with the trace (vibe4
  ~/bool-fuzz/cli/err-1144tr.log, binary ~/bool-fuzz/noshape-cli-fix3); bool next: OCCT's exact section line
  between planes through a tolerant vertex (b997c4c3's volume).
- Note: cargo doesn't relink noshape-cli when only the kernel's C++ changed (its build script output); touch a CLI
  source file before building a CLI to test a C++ change.

## RUST-SHELL-2AFC fixed (11-01 16:10 UTC)
- qa's tickets/cli-suppress-segv.noshape, Shell 6 (two faces, 0.5 mm) shells Part 12 and Part 11. Part 12 is so thin
  that its slabs cover it whole, so its shelled result is the part as it was; shell.cpp's slab path then required an
  inward result strictly smaller than the solid. OCCT's Boolean happened to round the result 1e-16 under the solid,
  noshape's 4e-21 m³ over (5.7255126800665496e-6 against 5.7255126800665454e-6), so Part 12's slabs "failed", the
  feature errored and Part 11 stayed unshelled. Now equal within 1e-9 relative counts as not more (both Booleans give
  OCCT's outcome). Recorded and replayed both ways first (bool-corpora shell6 / shell6on on vibe4): every Boolean of
  the document within the replay's rules, so the fault was the shell's rounding-level test, not a wrong result.
- Checked with a release CLI of this change on vibe4: Part 11 5553.078513 mm³ / 17 faces, as with NOSHAPE_BOOL_RUST=0.
  Shell tests (kernel `shell`) pass.
- Fuzz seeds against the clamp fix (same CLI): 979 (22 steps), 1036 (90), 1112 (30) run clean with no panic; 1144
  dies at step 51 (a suppress) with "stack smashing detected" with noshape's Boolean on, deterministic, clean with it
  off: being looked at under gdb (CRASH-1092's family).

## P0s from core (11-01 15:30 UTC): RUST-BOOL-CLAMP-ABORT, CRASH-1092
- RUST-BOOL-CLAMP-ABORT (qa's fuzz seeds 979, 1112, 1036: "min > max, min = 8.302416520637978, max =
  6.283185307179586" in a Shell; core now catches our panics and falls back). The cause: a free-form face's parameter
  box (free::uv_box) is its edges' curves' box cut to the surface's own range; OCCT's curves on a periodic B-spline
  surface can lie a period on (u from 8.3 on a 0..2π surface), so the box came out inside out (8.3..2π) and
  FreeSurface::param_of's Newton clamp panicked. Now such a face declines ("a face's curves outside its surface's
  range"), and the clamps in freeform.rs / nurbs.rs take their bounds either way round (a B-spline piece may run
  backwards). T1 brep boolean::free::tests::curves_past_the_surface_range_give_no_box. Not reproduced from the seeds
  themselves (qa's CLI fuzz); qa to re-run 979 / 1036 / 1112.
- CRASH-1092 (OCCT's ProjLib_ComputeApproxOnPolarSurface inside BOPAlgo_FillIn3DParts of a Shell's slab Boolean, on
  our result: an edge without its curve on a cone or sphere face, which OCCT then projects onto the pole). The build
  now checks every made face off a plane after all faces are built: an edge without its curve on it declines ("an
  edge without its curve on a curved face"), so OCCT never has to project one of ours. Which path left it out isn't
  known (qa's runs since 9903a7e0 are clean); the check shows it as a decline if it happens again (replays: why45).

## The knurl and straight B-splines on a cone (11-01 15:00 UTC)
- The 13:30 change (queued as 09f4554f / 495d8020) passed every replay set but broke T1
  the_b94_knurl_is_cut_by_noshapes_boolean ("two edges leave a vertex in one direction"): its rule mapping a straight
  B-spline on a cylinder or cone as a line (a ruling, vertical in the parameter plane) also took short pieces of the
  knurl's sections, straight within the tolerance but not rulings. Now only a ruling (its middle at the angle of its end
  off the axis) is mapped so. The knurl's recorded case isn't in the replay sets: run bool_patterns before landing.
  Landed again with the parallel-plane rule below and the notes, as one change.
- Also: a piece of OCCT's hyperbola or parabola section (the conic branch of the build) took both ends' parameters as
  the start's (its parameter lookup knew circles and ellipses only: "an edge against its curve"); now by the curve's
  own parameter, not wrapped by a turn. And two coaxial cones one within the tolerance, apart past it (OCCT's rule).

## Parallel planes by OCCT's rule (11-01 14:30 UTC)
- Two parallel planes are one within the tolerance and apart past it (OCCT's rule), no grey band ("faces nearly
  coplanar"): 91a99c7c and 717092f3 taken as equivalents (volume 1.7e-6 relative, within the replay's area x 0.1 um),
  main corpus 797 (+2), the other sets unchanged, 0 differ (why42).
- "A tiny arc" (8 cases): the inputs have circle edges of radius 1e-10 m (quarter arcs, a zero-size corner round, in
  03d0da47, 91923f72, b70f339a, 63bd60d0): degenerate input, left declined; which feature makes them is for core.
- "An edge on another argument's free-form face" (d0e4644e, 2284dd9c): a whole circle lying on a closed B-spline face
  across its seam; projecting it jumps at the seam. OCCT splits it there; ours splits curves at seams of cylinders and
  B-spline paths only, and keeps one curve on a face per edge, so it needs a split at a free-form seam first.

## Looked at, not landed (11-01 14:00 UTC)
- "One cylinder in faces of two inputs, their seams apart" (8 cases, 10 s; 38b2cd00, fd7ef128 and their copies: a
  boss through a plate, Fuse of 1): with the check off, ours unifies to the same counts as OCCT's but invalid (and its
  volume 4e-9 relative off), OCCT's valid. Before unify the faces and their curves match; the two walls' loops list
  the seam's uses in another order than OCCT's (OCCT: circle, -seam, circle, +seam; ours -seam, circle, +seam,
  circle), but reversing ours (as for a left-handed cylinder) changed nothing, so the order isn't it. Next: compare
  the curves of the edges each wall shares with the plate (the circle where the seams' frames differ by half a turn).
- Single-tool cuts declined "a non-manifold result" (419e648e, 4a71ecd0, 83147dc0, 29fa521a, 49de1c1d: a rounded rod
  1-2 m from the origin cut by a swept square tube with flat caps, ~1.5 s each in OCCT): the rod's pieces past the
  cut are classified by rays, and their middles lie on the tool's symmetry plane. (1) The first ray met both caps at
  one place (where their planes cross) and counted both: a ray crossing two faces at one t is now taken as unclean
  in /root/bool-scratch/ray-twice.patch (piece 1 then comes out In, right). (2) The next ray crosses both caps and
  none of the four B-spline sides for pieces 0 and 4: either the tool's caps cross each other or free::ray_hits
  misses a crossing on the swept sides (one Newton start per patch there). To look at with the shape.
- "Arguments share vertices" (59 cases, mostly fuses of one tool: a body and one made from its faces, sharing
  vertices, edges and up to 7 faces). Without the decline, 4 of 20 (main corpus) come out as OCCT's and 13 differ in
  history only. OCCT keeps some shared sub-shapes as they are (no record: the outer faces of a pocketed box fused with
  the same box ribbed) and rebuilds others (Modified: the pocket walls the ribbed one's faces lie along, their edges
  and vertices). Taking several inputs that are one shape as one (unchanged, the earliest argument's) matched 1 of
  20; taking only results that keep no shared vertex took none. OCCT's DS holds a shared sub-shape once; its rule for
  which it rebuilds is to be read from BOPAlgo_PaveFiller / Builder before trying again. (The plumbing for it,
  per-argument shared [argument, index] lists from rust_boolean.cpp to Body, was simple; not kept.)
- (Land queue: `land.sh --status` lists the worktree it runs in only; 09f4554f was queued from bool-d and was
  re-queued by mistake as 495d8020, the same change: if it fails as already applied, nothing to do.)
## Arc pieces from their start, closed edges on free-form faces, apex fixes (11-01 13:30 UTC)
- A piece of an argument's circle or ellipse starting at its edge's start: its angle a rounding below the start made
  t0 a turn on, the end measured from there, then t0 reset to the start: the edge ran a turn past ("an edge outside
  its face's range", ac2ca18a in five sets, 967be48f, 2e30111c). Now t0 is the edge's start first.
- A piece of a closed edge (a whole circle on a torus) ending where the edge closes: its end on the edge's curve on
  the free-form face at whichever end keeps the piece's middle between its ends (both halves were mapped onto one:
  "two edges leave a vertex in one direction", 9f434299).
- A plane square to a cone's axis exactly through its apex: no section (the apex a vertex the plane holds); in the
  grey band still declined (068c67bd, an equivalent: the known vertex-in-coincident-edges records).
- The general fixes from the apex chain (below, 12:20): rulings of any curve kind at a cone's apex, gap points on a
  part of a cone (halves not joined across the gap), the nearest of two vertices where clearly on one, stricter arc /
  ellipse / B-spline merges (two curves between the same ends one edge only on one course), OCCT's section along
  another curve within its own error, straight B-splines on cones and cylinders as lines.
- T1 bool_patterns::a_piece_of_an_arc_from_its_start, halves_of_a_closed_edge_on_a_torus (0.3 s together, dev).
- Replays why41: main 795, corpus2 589, corpus3 594, corpus4 641, corpus5 785, corpus6 718, direct 78, Shell's 105
  (+23 in all), 0 differ. Share of all sets: 84.9 % of cases, 80.7 % of OCCT time (why36: 84.1 % / 74.7 %).

## Looked at, not landed (11-01 12:20 UTC)
- "An apex on a part of a cone" (b0c36850, 9f262529, d5cd8b8d: fuses of 13 sector bodies around drilled cones,
  ~19 s recorded). Each fix moved them to the next fault; the general fixes found on the way are in
  /root/bool-scratch/apex-wip2.patch (worktree bool-c, branch bool-apex, commit 06797001 with debug prints under
  NOSHAPE_BREP_NM): rulings of any curve kind at the apex (apex-any-kind.patch); a piece of a curve inside a part of a
  cone from or to its gap angle gets a point there, and the two halves aren't joined across the gap (a face going round
  with two sides there); a point within reach of two vertices is the one it is on (within a tenth of its reach) where
  the others are past half theirs; two arcs or ellipses between the same ends one edge only with their middles within
  the grey band (was 100 tol = 2 mm: a hole's circle and another part's crossing it at both ends were merged); two
  B-splines one edge by each one's points on the other's piece (pieces of one curve by their ranges); OCCT's section
  along another curve within its own error; a straight B-spline on a cone or cylinder mapped as its line (its apex end
  at the ruling's angle, its side of a seam). Full replay (why39): every set exactly main's counts, 0 differ; landed
  13:30 with the fixes above. Where they stop now: b0c36850 / 9f262529 "two edges leave a vertex in one
  direction" (the apex's pieces), d5cd8b8d "coincident pieces differ": a section of face 10:0 with a face of argument
  9 (an argument isn't intersected with itself) splits 10:0 but not the coincident 9:0; OCCT splits same-domain faces
  by each other's edges. That (same-domain faces sharing their splits) is the next step for these.
- "A new edge on a free-form face" (763e60bc, d33e88ff, b7eb72a8, ~15 s): projecting the edge's curve onto the face
  (BuildPCurveForEdgeOnFace, as for input edges; /root/bool-scratch/proj-newedge.patch) takes 763e60bc and d33e88ff
  as equivalents (2 vertex records apart, volume 1e-13), but b7eb72a8 (a Shell slab fuse of 17) then differs: after
  unify ours has 14 faces, OCCT's 18. Before unify the faces are the same; the coincident cylinder pieces at the boss
  (arg 4's cylinder and arg 11's reversed half cylinder, r 10 mm) are kept on arg 4's surface by ours and on arg 11's
  by OCCT (opposite axes: OCCT's unify then doesn't merge them). OCCT's FillSameDomainFaces says the original face
  with the least DS index, else the group's first (the least index's piece), which would be arg 4's: so its choice
  comes from how its pairs group, to be found with an instrumented TKBO. Trying "original faces first" (OCCT's rule,
  original = not split, its edges and vertices unchanged) changed nothing here. Not landed.
- "An internal face" (b2511301, addb9a14): see below, not worth it.

## Crossing grooves with one floor; a vertex on a plane by OCCT's rule (11-01 11:30 UTC)
- A cut's tool piece lying on other tools was never kept (right where they face each other: inside the tools). Grooves
  crossing in pairs (a 24-tool knurl-like cut, 50004ac9 and 8c3247ab, in every set: ~25 s recorded) have their floors on
  one plane facing one way: both pieces of the overlap were dropped, a non-manifold result. Now the earliest tool's
  piece is kept there (inside the object, in no tool), as OCCT keeps one of same-domain faces; the others are Modified
  to it (`partner`). Taken, 0 differ (history and unify too).
- A vertex against a plane: OCCT's rule alone (on it within its own tolerance and the face's, apart past that), the
  grey band ("a vertex nearly on a plane") gone, as for two vertices: 8 more taken in corpus5 and corpus6, 0 differ.
- T1 bool_patterns::crossing_grooves_with_one_floor (tests/data/booleans/50004ac95f2ebada, 0.2 s release).
- Replays why37 (the groove rule) and why36 (the plane rule): 0 differ everywhere; main 793, corpus2 587, corpus3 592,
  corpus4 638, corpus5 774 + 8, corpus6 702 + 8, direct 78, Shell's 102.
- Looked at and left: "an internal face" (b2511301, addb9a14: Shell slab fuses of 72): with the internal face left
  out of its solid's boundary (/root/bool-scratch/internal-face.patch) they stop at "a tolerant input": faces of
  7e-4 tolerance 1e-5 from other arguments, OCCT's own result invalid. Not worth taking.

## OCCT's ellipse round the other way; a B-spline face rising off a plane not coincident with it (11-01 10:14 UTC)
- "An ellipse unlike OCCT's sections" (13 cases, 26 s): OCCT's ellipse of a plane and a cylinder had our centre
  and axis the other way round, and only the same axis matched. Now matched either way: a reversed one
  (`EllEdge::reversed`) gives the part from our end to our start. Its edge is made from our end vertex to our start
  on OCCT's curve and pcurves, and each of its uses in a face flips again (`flipped`).
- `same_surface` (coincident faces of different kinds) skipped samples outside the other face's box. A B-spline
  face rising off a plane it starts on had only its samples along the plane tested, and so came out coincident
  with the plane ("coincident pieces differ"). The box skip now applies against a free-form patch only. A plane's,
  cylinder's or cone's surface goes on past its face.
- T1 bool_patterns::an_ellipse_round_the_other_way (tests/data/booleans/1310d7e980382436, a fuse of 25). Replays why35:
  one more taken in every set but the main corpus, 0 differ. Left: the same cases then reach "a new edge on a free-form face". Projecting its
  curve, as OCCT's builder would, takes them (763e60bc, d33e88ff equivalent). But b7eb72a8 (a fuse of 17) then
  differs after the unify: ours 14 faces / 30 edges, OCCT's 18 / 39 (the same before it). Not landed; to look at.
- The apex WIP (rulings of any curve kind at a cone's apex: /root/bool-scratch/apex-any-kind.patch) moves those 3
  cases to "an edge's piece without ends". "Two edges leave a vertex in one direction" (6): seam cases, the same
  curve twice with its ends' places swapped, or two vertices at one place on a cylinder's parameter plane.

## Inputs with INTERNAL edges, two vertices of one argument (11-01 09:21 UTC)
- An input face's INTERNAL edges ("an internal edge", 39 cases) are no longer declined.
  - to_body passes them as (face, edge) pairs (`Body::face_inner_edges`). Each face keeps them (`GFace::inner`),
    out of its arrangement: as in OCCT's builder they split nothing.
  - Each piece of them goes to the face's piece it lies in, as that piece's inner edges, and comes out INTERNAL. A
    face unchanged keeps the input's own.
  - Coincident pieces: the kept one takes the others' inner edges.
  - Faces with them skip the turned (patterned) copy.
- An edge without its curve on a free-form face in the result's build (another argument's edge inside or along it):
  projected (BOPTools_AlgoTools2D::BuildPCurveForEdgeOnFace), as OCCT's builder makes one.
- Two input vertices of one argument within the tolerance: both kept (OCCT doesn't intersect an argument with
  itself), not declined.
- T1 bool_patterns::an_input_edge_inside_a_face_stays_in_it (tests/data/booleans/818552c996311f5e, a cut).
  Replays why32: 2–5 more taken per set (corpus2/3 +3, corpus4/5/6 +5, Shell's +2), 0 differ.
- Still declined among the internal-edge inputs: "a tiny arc" (4: degenerate arcs between those coincident
  vertices), "a face's vertex off its plane", "a vertex in a face".

## RUST-SM-CUT-VOLUME fixed (11-01 08:30 UTC; skasm via lead): a face on a trimmed left-handed cylinder the other way round
- qa4_s1_enclosure_as_live cut 7 mm³ more than live with noshape's Boolean on. The cause, found by recording the
  test's Booleans with OCCT (NOSHAPE_BOOL_RUST=0) and replaying them with ours: one of 17, the fuse of the
  flanges, bends and hem (02215ef9a303de5c). Ours was invalid with +7.004 mm³: a bend face on a
  Geom_RectangularTrimmedSurface over a left-handed cylinder came out reversed. to_body reads a trimmed surface's
  basis (and so the left-handed frame's inward normal); the result's build took the trimmed surface's own type, so
  not round, and so no flip. Its orientation now goes by the basis, as to_body's.
- T1: bool_patterns::a_sheet_metal_enclosure_fuse_as_occt (the recorded fuse: valid, OCCT's volume and counts,
  0.1 s); sheetmetal_std::qa4_s1_enclosure_as_live is T1 now (2.7 s in the dev profile). Replays why31: 0 differ.
- Note for the lead: ours returned an invalid solid without a check. A validity check on every result of noshape's
  Boolean would have caught this, falling back to OCCT. It costs about BRepCheck's time (the knurl's: about 2 s of
  26). Not added; say if wanted.

## Edges ending inside a face: INTERNAL edges in our result (11-01 07:07 UTC, lead)
- OCCT's builder leaves an edge that ends inside a face (a tool's edge touching another's face) in that face as an
  INTERNAL edge. Ours now does the same:
  - the arrangement's regions record such curves (`Region::inner`, those with the region on both sides);
  - a piece keeps them (`Piece::inner`); they come out of its loops into `Output::face_internal`;
  - the bridge passes them as each face's last loop, flagged (`BrepResult::face_inner`);
  - the C++ builder adds them to the face as INTERNAL edges, their curves on it made as for the face's other edges.
  - They don't count as boundary uses in the manifold checks.
  - Coincident pieces with them: the kept one's (both split alike).
- T1 bool_patterns::edges_ending_inside_a_face_stay_in_it (tests/data/booleans/5fc0970a688d0fb9, 12 slabs fused on
  a plate): taken, OCCT's faces and edges, an INTERNAL edge.
- Replays why30 (with the tooth-15° rule): 3 more taken in every set but the main corpus (30afb30c, 5fc0970a,
  b96876ba), all equivalent, 0 differ. Their face records are OCCT's. Open: 80–116 vertex records differ. OCCT says
  the slabs' corner vertices are Deleted, ours Modified (to a vertex at the same place); to look at.
- Still "an edge ends inside a face" before, now "a non-manifold result" (12 cases): near-tangent geometry where a
  tilted plane's ellipse meets a cylinder's end circle (a 2.7 µm arc used on one side only), and a 1.5 µm edge at a
  chamfer corner. Not the internal edges.

## The user's P0: the B94.6 Diamond knurl at 45° / 45° (11-01 02:25 UTC)
- Live (request 1687): OK, 95473.389 mm³, 17762 faces, 114 s. Ours failed "non-manifold": noshape's Boolean declined
  (a non-manifold result) and OCCT's path took 324–391 s and came out non-manifold on the user's machine.
- At 45° the teeth are sharp: the profile reaches 10 µm past the cylinder, leaving lands ~8 µm wide at the tooth tips,
  and OCCT's section curves crowd their parameters (and knots) there. Four faults, each found in that order:
  1. Split edges: two sections with the same ends were one edge if their parametric middles lay within 100 tol
     (2e-5 m), which merged a land's two sides. Now two different sections are one edge when each one's points lie
     on the other within the curves' own accuracy (OCCT's IntTools_Curve::Tolerance(), passed as the eighth number
     of each section, SectionCurve::accuracy).
  2. Area: C2::area_term / length integrated in 8 even parts of the parameter, which turned a triangle's area
     negative (dropping it from the arrangement); now also split at the B-spline's knots (exact per span).
  3. Middles by length, not parameter: the interval test in `intervals` (a section judged inside a face at its
     middle, which sat at a corner) and the arrangement's point inside a region (curve2 at_fraction).
  4. The direction a curve leaves a vertex: OCCT's 2D curve hooked back 0.1 µm at its start, so its derivative there
     pointed the other way and sorted it outside its face. C2::leaving: the way to its point 1 µm on where that
     differs from the tangent by more than ~6°.
- Now: valid, 1 part, 17762 faces as live, 19.5 s on a loaded vibe2 (vibe3 time to come). Volume 95469.936 mm³, 3.45 mm³
  (3.6e-5) under live's; OCCT's Boolean on the same tools gives the same as ours (2.3e-8 apart), so the gap is the
  groove tools' (opSweep of the diamond along the helix; live asked for one tool's volume, REQUESTS 11-01 01:04).
- T2 custom_features::knurl_feature_b94_tooth_and_helix_angles, 14 pairs (tooth 15–80°, helix 15–45°: the feature
  takes helix angles up to 45°), volume and faces against live's where given (45/30, 15/15, 80/15 within 9e-6; 45/45
  at 4e-5 for the tools' gap), and noshape's decline printed. On vibe2 all valid, one part. Ours (fast): 45/x, 60/x,
  80/15, 80/30, 30/45, 80/45. Still declined to OCCT (83–428 s): 15/15, 15/30, 15/45, 30/15, 30/30.
- 80/45: noshape's Boolean declined "a piece on two solids" (the wide grooves' end caps overlapping, three on one
  plane past the cylinder's end) and OCCT's path stopped at the 16 GB budget after 482 s ("needed more than 16043 MB").
  A tool's piece on two other tools outside every object is never kept (as any tool's piece on another tool's): taken.
  Now valid, 1 part, 93782.726 mm³, 17762 faces, 20.5 s on vibe2 (OCCT gives none to compare; live asked).
- Tooth 30° (11-01 05:13 UTC): 30/15 and 30/30 declined "a non-manifold result". The cylinder's section with each inner flank
  runs 3.5 µm inside the flank's side edge (that edge 3.4 µm outside the cylinder). With the section's tolerance
  (OCCT's, at least 5e-6) the interval test called the whole section "along the boundary", and the sliver between
  them was lost. OCCT keeps it: its paves of a section are only the vertices on both faces. Now, in `intervals_on`
  for OCCT's sections: along the boundary or not by the curve's own accuracy (IntTools_Curve::Tolerance); cuts at one
  place merged within the section's tolerance; a face vertex taken onto it (inside or at an end) within that
  tolerance only where it also lies on the other face. And an edge whose seven sample points are all past the
  tolerance is off the face (OCCT's rule), not "nearly on". Taken: 30/15 in 3.8 s (OCCT 42–81 s), 30/30 in 8.7 s (OCCT
  127–178 s), both equivalent to OCCT. Replays why28: the same cases taken in every set, 0 differ.
- Live's numbers for the rest (request 1696): every face count as ours (15/30 is 10436 there too). Volumes within 1e-5
  at helix 15° and 30°. At helix 45° ours is 2.2–5.9 mm³ low, more as the tooth angle rises (30/45 −2.19, 45/45 −3.45,
  60/45 −4.6, 80/45 −5.85); our Boolean equals OCCT's there, so the gap is the tools'. One live groove tool (cut1,
  HIGH): 44.73340 mm³ at 45/45, 77.02478 at 80/30; ours to compare (the knurl with its Boolean commented out),
  then a request to core if the sweep differs.
- Tooth 15° (11-01 06:43 UTC, lead: decide the slivers by the rule that matches live, take them). At a 15° tooth the
  side edge is under a micrometre outside the cylinder, and the kept slivers (live's face counts keep them) are that
  thin. OCCT's rule is now the only one for two vertices and a vertex on an edge: together within the sum of their
  tolerances, apart past it. The grey band past that declined "two vertices nearly coincide" and "a vertex nearly
  on an edge", and a piece's point had to be 4 tol from its face's boundary (now tol). A piece whose point lies on
  another solid's face takes its neighbour's state across a section with that face, in and out the other way. All
  three taken, equivalent to OCCT: 15/15 12.7 s (OCCT 79 s), 15/30 37.6 s (187 s), 15/45 28.3 s (355 s) on a loaded
  vibe2. Replays why29: 3 more taken in the main corpus, 1 more each in corpus2 and corpus3, 0 differ.
- The T2 angle sweep now checks every pair against live's volume and faces (all 15 from requests 1615/1687/1696):
  7e-5 at a 45° helix (the sweep's gap), 2e-5 else.
- Recording Booleans (NOSHAPE_RECORD_BOOLEANS) adds per-face props for the summary: 45/45 took 128 s recorded, 19.5 s not.

## The knurl back on OCCT's path, fixed (11-01 01:44 UTC; lead's P0)
- vibe3's hourly baseline: knurl_feature_b94_on_a_cylinder 16.07 s at d7df1ce4, 138.2 s at 6628462c. Reproduced on
  vibe2 at 6628462c: noshape's Boolean ran 8.1 s, then declined "a piece's point near a coincident face's boundary",
  and OCCT took 157 s. The cause was mine, 2afca924: faces on one free-form surface count as coincident there. The
  knurl's grooves have such faces, each groove its own part of the surface, and a piece's point past a neighbour's
  patch has its parameters clamped onto that patch's edge (distance 0 to its boundary). Fixed: a free-form face the
  point is off by more than the tolerance isn't one it lies on. The recorded case has the same hash as before, so
  skasm's FORWARD edges (4e9b054e), core's 66658482 / cfb21072 and psui's 66cf2194 changed nothing there.
- T1 bool_patterns::the_b94_knurl_is_cut_by_noshapes_boolean: the recorded knurl cut (tests/data/patterns
  d14b48a8ddf91647) by noshape's Boolean alone (set_rust_boolean_only: a decline fails), its face count as recorded,
  valid. 26 s in the dev profile on vibe2; fails without the fix.

## The goal from 10-31 19:00 (user): noshape's Boolean takes every case; OCCT only a temporary fallback
Work the declines by OCCT time, biggest first: implement OCCT's rule where it's definite (per-vertex tolerances,
the pave filler's same-domain handling); where OCCT's choice is arbitrary rounding, match within tolerance (same
topology, volume and area within the model tolerance, valid, the same faces in the history, the same after the
unify) and count it "equivalent" with its deltas; an internal face's reference (OCCT's result invalid) is a valid
result and live's numbers; then the long tail, sheets and every surface type, until no decline is left. Track the
share by count and by OCCT time on all replay sets after each step (/root/bool-scratch/share.py <why.tsv>, from a
NOSHAPE_BREP_WHY="" replay of every set: /root/bool-scratch/why2N.sh).

| step (UTC) | count | OCCT time | main corpus by time | the other sets by time |
|---|---|---|---|---|
| before (why20, 17:13) | 80.5 % | 53.8 % | 89.6 % | 15–47 % |
| three arguments' coincident edges (why21, 18:16) | 80.7 % | 54.0 % | 89.6 % | 16–48 % |
| coincident pieces, curves along boundaries, equivalents (why22, 20:36; tolerances not in it) | 82.5 % | 70.9 % | 89.8 % | 50–70 % |
| per-vertex and per-edge tolerances (why23, 21:17) | 82.8 % | 71.3 % | 89.8 % | 51–72 % |
| knurl paves and slivers (OCCT's vertex rule), INTERNAL edges (why30, 11-01 07:00) | 83.3 % | 73.6 % | 89.8 % | 51–73 % |

## Coincident pieces OCCT's way; tolerant vertices; curves along a boundary (10-31 20:59 UTC)
- A vertex inside coincident edges of two other arguments no longer declines where nothing is exact: Modified as
  before (OCCT's history there goes by rounding: those cases come out equivalent, the vertex records apart).
- What that uncovered, OCCT's rules: (1) in a fuse the coincident pieces' group keeps a piece of a face that was
  split before a whole face (then the first argument's), as OCCT's same-domain faces (a slab's corner cylinder
  piece on the taller cylinder of the next slab: OCCT's unify then merges them; with ours the whole one it
  didn't); (2) pieces of two arguments' edges of different curve kinds on one course (a line and OCCT's rational
  B-spline section along it) are one split edge, as a common block (each point of one within the tolerance of
  the other); (3) faces on one surface though of different kinds (a cone and a B-spline of it, two copies of a
  patch) are coincident like coplanar ones (`same_surface`: each face's vertices, edge middles and a grid over
  its parameters on the other's surface).
- A section within the grey band of a face's boundary all along is on the boundary (OCCT's within its tolerance
  of the edge: a common block), not a decline.
- Tolerant vertices: one on a plane within its own tolerance and half the Boolean's (OCCT's vertex/face rule), apart
  past that + 1e-5; merged inputs grow the merged vertex's tolerance to cover both (and their distance); the result's
  vertices carry their tolerance into the C++ build (`Body::vertex_tol` of the result, `MakeVertex`).
- brep_replay's equivalents also take: OCCT's result invalid now but valid when recorded, ours valid with the
  recorded counts and volume; OCCT's unified result invalid where ours is valid.
- Replays all 0 differ: main 788 (1 equivalent), corpus2 577 (+14; 9), corpus3 582 (+14; 9), corpus4 627 (+13; 7),
  corpus5 763 (+13; 7), corpus6 691 (+11; 5), Shell's 96 (+11; 5), direct 74 (+12; 7). The coincident-edge cases:
  17 of 17 taken (3 exact, 14 equivalent); along a boundary 5 of 5 (1 equivalent).

## Equivalent results; per-entity tolerances (10-31 19:39 UTC)
- brep_replay: a case whose result differs from OCCT's only by OCCT's arbitrary choices is "equivalent" (not a
  difference): the same solids, faces, edges, vertices, both valid, the volume within area × 1e-7 m and the area
  within edge length × 1e-7 m, every face record the same (2 µm), the same after the unify; its deltas reported.
  NOSHAPE_BREP_EXACT=1 counts them as differing.
- Per-entity tolerances (OCCT's rule): `Body::vertex_tol` / `edge_tol` (public type change, filled by the C++
  conversion), the pool merging a vertex with another within the sum of their own tolerances where that's more
  than the Boolean's (`find_tol`; tolerant ones checked against every vertex), a vertex on an edge within the sum
  of the two's (`split_tol`). Between two inputs' vertices nothing but rounding is grey; beside a tolerant one a
  new vertex (OCCT's own tolerance, at most ~1e-5) is apart past the sum + 1e-5. The C++ side passes inputs whose
  tolerant sub-shapes are vertices and edges only (faces over 1e-5 still declined), the Boolean going by the rest.
- Replays all 0 differ, 0 equivalent: main 786, corpus2 563 (+2), corpus3 568 (+4), corpus4 614 (+2), corpus5 750
  (+2), corpus6 680 (+2), Shell's 85 (+2), direct 62 (+2). The tolerant-input cases: 6 of 18 taken; the rest stop
  at other reasons now (vertices near planes, apexes on parts of cones, ...).

## Vertices inside coincident edges of three other arguments; the crossing guard gone (10-31 18:16 UTC)
- Shell's slab fuses: slabs meeting along a line put an argument's vertex inside the coincident edges of three
  others. OCCT's history then says that vertex was Deleted (not Modified) when it is that one argument's alone;
  one already shared with another argument's vertex stays Modified (checked on the 8 cases of each kind). Ours
  does the same (exact straight edges along an axis, as before for two).
- The "edges of two arguments crossing on a third's face" decline (any vertex of three arguments on the face, or of
  four anywhere) is gone: what differed under it was that history alone.
- Replays all 0 differ: main 786, corpus2 561 (+2), corpus3 564 (+2), corpus4 612 (+2), corpus5 748 (+2), corpus6
  678 (+2), Shell's 83 (+2), direct 60 (+2). T1: 0456f872a355cd83 in tests/data/brep.

## What still declines, by OCCT's recorded time (replays 10-31 17:13 UTC on main with the apex work, each case once)
A vertex inside coincident edges of two other arguments 32.3 s (17), an internal face 25.3 s (2: b2511301 a Shell
slab fuse of 72 tools whose OCCT result is invalid, addb9a14), a tolerant input 23.9 s (18), edges of two arguments
crossing on a third's face 22.2 s (14), two vertices nearly coincide 9.7 s (10), a curve nearly along a face's
boundary 5.5 s (5), an edge on another argument's free-form face 4.9 s (4), an internal edge 4.0 s (11), the rest
under 4 s each.
(/root/bool-scratch/why20.tsv: corpus, case, recorded ms, reason.)

## Times, the cases taken (replays 10-29 23:30 UTC, ours measured here against OCCT's recorded time)
| corpus | taken | median ours / OCCT | total ours / OCCT |
|---|---|---|---|
| main | 680 | 0.48 / 7.7 ms (<50 faces); 18 / 137 ms (50–1000) | 7.3 / 19.1 s (2.6×) |
| corpus2 | 528 | 0.54 / 9.2 ms; 9.3 / 106 ms | 6.3 / 17.2 s (2.7×) |
| corpus3 | 533 | 0.62 / 10.6 ms; 11.5 / 163 ms | 7.2 / 21.5 s (3.0×) |
| corpus4 | 574 | 0.53 / 8.3 ms; 7.2 / 91 ms | 6.8 / 19.0 s (2.8×) |
| corpus5 | 685 | 0.54 / 8.2 ms; 8.4 / 82 ms | 7.0 / 20.8 s (3.0×) |
| corpus6 | 640 | 0.60 / 10.0 ms; 8.4 / 111 ms | 8.2 / 28.0 s (3.4×) |
| Shell's | 75 | 0.39 / 6.0 ms; 4.9 / 52 ms | 0.6 / 4.9 s (8×) |
| direct | 52 | 0.41 / 8.9 ms; 9.4 / 151 ms | 0.2 / 3.5 s (16×) |
Medians are 10–20× faster; totals 2.6–3.4× in the corpora, held down by a few slow free-form cases (the thread fuses
~2 s each against OCCT's 3–4 s). (OCCT's are the recorded times, parallel, on a loaded machine; ours serial.)

## Next
- The knurl is done (lead 10-31): 8.8 s regeneration on vibe3 at 8d4cf857 against live's 49 s, the test 15.96 s.
  What's left in it: noshape's Boolean 4.9 s (its brep part 4.35), core's unify 2.0 s.
- Pattern fuses of 128+ touching instances (fuse_slabs): waiting on core / lead (Requests): ours first would take
  them faster with the same shapes, but the plain Boolean's history.
- The declines are flat now (above): the top ones are OCCT's tolerance and rounding choices (coincident edges of
  three arguments, tolerant inputs, points 1–3 tolerances apart: checked 10-31, OCCT goes by each new vertex's own
  tolerance there), not geometry we lack. Looked at and left: fuse_slabs' inner fuses through ours (2.5 s against
  ~2.1 s on 1000 fins: little), a parallel BRepCheck in is_valid (the model checks small shapes).
- skasm's three union rows: done 10-31 (above, "Unions with coplanar neighbouring faces").
Free-form, the lead's four steps (13:35) where they stand (10-29 20:45 UTC): (1) profiled (above); (2) the hybrid
(OCCT's IntTools_FaceFace curves, everything else ours) takes B-spline, torus, sphere, extruded and revolved faces;
(3) tori / spheres / sweeps through the same hybrid: returns flat (spheres and tori mostly decline at poles and
seams: a few cases, ~4 s recorded in corpus6); (4) the helix cases: the hybrid plus faster curve evaluation and
crossings take both thread fuses, 2.1–2.3 s against OCCT's 3–4 s (recorded 4.8 / 6.5 s). OCCT's face/face share
there is 0.35 s: no own intersector needed for them now.
Corpus6 still declined, by recorded OCCT time (10-29 23:55 UTC, 640 / 828 taken): an internal face 13.8 s (1:
addb9a14, a slab fuse with a B-spline face, the shelled part's own internal edge), tolerant inputs 13.1 s (10: 1e-4
edges where the arguments meet; per-point tolerances would take them), vertices inside coincident edges of two
other arguments 11.8 s (6: OCCT's PostTreatFF gives such a vertex a second same-domain hop and its history then
says Deleted; not predictable cheaply), an apex on a part of a cone 8.5 s (7: a cone face that doesn't go round, its
apex's degenerate edge in its parameter plane: 4 taken since, the rest stop at other reasons), edges of two arguments crossing on a third's face 5.2 s (4), tools
coincide 5.1 s (4), curves in the grey band of a face's boundary 3.9 s (3), internal edges 3.6 s (6), a plane through
a pointed cone's apex 2.9 s (5), a cone's / sphere's / B-spline's apex changed 2.6 s (7: another argument through a
pole; OCCT makes a new degenerate edge there), the rest under 1.3 s each.
- The unify guard (`usd_kept_volume`) costs a copy and two volumes where round faces merge; measure on the regen
  profiles and make it cheaper (only the merged faces' share of the volume) if it shows.

## From core (10-31 21:50 UTC): cancel checks in crates/brep (user P1: cancel within ~100 ms)
- lib.rs: a thread cancel flag (`set_thread_cancel(addr)`, `thread_cancel()`, `cancelled()`, crate-private
  `check_cancel()`): the kernel's `with_cancel` sets it next to the C++ one. `par_map` returns `R<Vec<T>>` now (its
  callers `?`): it declines "cancelled" before it starts, every 16 items, and after the workers join; the workers take
  their caller's flag. `boolean_with` checks it at its phases (pairs, sections' places, crossings, split edges, pieces,
  classification, the result) and per solid in the pairs loop. rust_boolean.cpp: a decline under a set flag stops
  ("cancelled") instead of handing the Boolean to OCCT. T1 brep `a_cancelled_boolean_declines`. If a loop of yours
  runs long without par_map, add `crate::check_cancel()?;` in it.
- (11-01 02:30) `set_stop_hook(fn() -> bool)`: `cancelled()` also asks it; the kernel installs its
  `noshape_should_break` (the thread's cancel flag or its budget's deadline), so a budgeted op's Boolean (Shell's
  slabs, the blend fallbacks) declines at the deadline instead of the budget's timer jumping out of Rust code.
- (11-01 08:10) rust_boolean.cpp `build()` (your result into OCCT topology) checks `fail_if_cancelled()` every 256
  edges / faces in its three loops: the 45 / 45 knurl's took 1.3-1.8 s after a cancel there. op_boolean's
  history is `cancellable_history`, its unify steps check too, and a cancelled run's big objects are freed on
  other threads (FreeIfCancelled, kernel Shape drops): nothing changes unless the thread is cancelled.

## Requests
- lead (11-02 00:10, perf baseline): add the user's Ø25 × 25 knurl to vibe3's ~/lead/perf-baseline.sh CASES:
  `"knurl_d25|3|-p noshape-ops --test all -- regen_profile::timed_knurl_d25 --ignored --nocapture --test-threads=1"`
  (opens tests/perf/knurl-d25x25-45-45.noshape and regenerates it with its display meshes; 5.9 s median on vibe4).
- qa (11-01 23:30 UTC, user's decision): run the CLI fuzz with NOSHAPE_BOOL_FALLBACK=0 (and NOSHAPE_RECORD_BOOLEANS=<dir>
  NOSHAPE_RECORD_FAILED=1 to keep each decline's inputs): every remaining decline then shows as a feature error
  "noshape's Boolean declined: <reason>" instead of minutes in OCCT; report the reasons and counts (and the recorded
  cases' directory) to bool.
- core (11-01 17:10 UTC): fuzz seed 1144 dies at step 51 (a suppress) with noshape's Boolean on, after every recorded
  Boolean of ours in the run is valid: "stack smashing detected" in op_prism (features.cpp, a sheet metal extrude's
  prism during the regeneration), trace on vibe4 ~/bool-fuzz/cli/err-1144tr.log (NOSHAPE_GUARD_TRACE=1), binary
  ~/bool-fuzz/noshape-cli-fix3 (symbols), runner ~/bool-fuzz/clifuzz_time.py (`python3 clifuzz_time.py 1144 62` with
  NSCLI set). Clean with NOSHAPE_BOOL_RUST=0 only because that run builds another model (face ids part at step 2,
  see "Fuzz seed 1144 diagnosed"); the stack smash itself is in OCCT's prism code on a valid model.
- core (10-31 16:40 UTC, P0 SHELL-SLOW): Shell's slab union is OCCT's for concave parts, 1.5-2.3 s of a 2.6-3.4 s
  Shell: a 60 x 40 x 20 block, a 40 x 20 x 10 pocket from the top with R2 on its 4 floor edges, shell with the
  bottom removed, 1 / 2 mm (18 slabs: planes' boxes, the fillets' cylinder shells, filler prisms at the concave
  edges). Your Boolean declines the fuse of all 18 ("edges of two arguments crossing on a third's face"); fused one
  at a time it declines a slab touching the union along an edge only ("a non-manifold result") and the fillets'
  slabs ("two edges leave a vertex in one direction"); the common of the union with the solid: "a non-manifold
  result". If the many-argument fuse (or the tangent case) is in reach, Shell's slabs path drops to ~0.2 s there.
  Repro: kernel test shell::scratch_pocket_shell in /root/core-scratch/pocket/scratch_test.rs (paste into
  crates/kernel/tests/shell.rs; NOSHAPE_SHELL_DEBUG=1 prints the steps).
  core 10-31 20:05, on main with 108cdf21: the fuse of all 18 now declines "a vertex inside coincident edges of two
  other arguments", the common still "a non-manifold result" (read with a print of rust_last_decline() after
  quick_boolean's rust_boolean, not landed); Shell 2.8 / 4.3 s here (fuse 2.2-2.9 s, common 0.8 s).
  core 11-01 03:20, on main with 2afca924 / ba2b4db3: the 18-slab fuse now declines "a non-manifold result" (t 1 mm)
  and "coincident pieces differ" (t 2 mm); the common still "a non-manifold result". 3.4-5.7 s here (loaded).
- Done (core 2c6e89c2; the knurl's cut taken 10-31): core (10-30 23:45 UTC): op_boolean's CutByFaces build (a cut by 8+ tools in 2+ disjoint groups) leaves the
  target's solid without a record (history: Deleted), where every other path (OCCT's usual build, its fallback when
  CutByFaces fails, noshape's Boolean) says Modified into the result's solid. Could CutByFaces record the target's
  solid as Modified into the result's? Then noshape's Boolean can take those cuts (the B94.6 knurl's is one: it now
  declines "tools coincide" on that account) with the same history as today. If anything (names, bodies) relies on
  the Deleted, tell me and I'll mirror it instead.
- core 10-31 18:05, answer: take it. The naming (model-data naming.rs name_faces) drops Deleted records and reads Kept
  and Modified alike (a face continues its parent's name; several images get ordinals by geometry), so Kept where
  fuse_slabs said Deleted + Modified to the same faces gives the same names.
- core / lead (10-31 14:10 UTC): pattern fuses of 128+ touching instances go through fuse_slabs (OCCT, slabs glued
  and unified). noshape's Boolean takes all nine recorded from the pattern tests (999-fin fuses 2.1–2.5 s against
  fuse_slabs' 3.1–4.9 s here, loaded), and with fuse_slabs' unify after it the same faces, edges and volume as
  fuse_slabs. But not the same history: fuse_slabs' composition through the slab cuts records the block's faces
  and edges both Deleted and Modified (an artefact), where ours, as a plain Boolean, says Kept. So I haven't
  switched them over (patch: /root/bool-scratch/rust-before-slabs.patch: ours first where fuse_slabs' gaps
  apply, unified as fuse_slabs, fuse_slabs and the trimmed path after). If the naming would take the plain
  Boolean's history for these (Kept), say so and I'll land it with the corpus rebased.
  core 10-31 20:05, answer: take it, as for CutByFaces: the naming drops Deleted records and reads Kept and Modified
  alike, so Kept gives the same names as fuse_slabs' Deleted + Modified to the same faces.
- Done (core a9b4bf6a, fff8a036, 30cc7935, d29f53ce: the knurl 8.6 s on vibe3): core (10-31 UTC): the unify after a patterned cut: on the knurl, unify.usd takes 10.7 s of op_boolean's 18.3 s
  (vibe3, step 5; the Rust Boolean 7.3 s, less since), and all it merges is 14 of 10376 faces (cylinder-side pieces split at the
  seam; OCCT's raw result has them too) plus those faces' edge halves. Most of it is usd_kept_volume: two
  BRepGProp::VolumeProperties of the whole 10k-face result (B-spline faces) and a copy, around one USD. Could the
  volume check compare only the merged faces' share (the volume is a sum over faces: the faces USD replaced
  before, their replacements after), or the unify run on just the faces that share a surface with a neighbour?
  Same result, most of the 10 s less there.
- Done (core 26074458: a 1e-9 s budget, stopped at the indicator's first check everywhere): core (10-30 06:15 UTC): `boolean_groups::a_section_under_a_call_budget_gives_up` (fd65effd, 04:39) fails on
  Windows and macOS in my branch CI (run 41008061871: "" after 0.48 s, the section ran to the end under a 1e-4 s
  budget); Linux passes. It hasn't been on a scheduled run yet (main's last, 03:27, predates it). The budget is CPU
  time while BRepAlgoAPI_Section runs parallel (bop_parallel): the calling thread's CPU may stay under it there. Not
  bool's code (an OCCT section of a plate with 400 holes, the holes made by the Rust Boolean as before).
- Done (core 10-31 18:00: LP2 < max(3 LP1 + 300, 2000) ms): core / lead: `pattern_second::timed_second_pattern_costs_about_the_same_as_the_first` (perf.yml) compares LP2 to
  LP1; the Rust Boolean made LP1 ~5× faster, so `LP2 < 3 LP1 + 300 ms` fails though nothing got slower. Compare
  LP2 with an absolute budget instead (it's ~1.5–1.8 s dev here, 2.1 s before).
- psui (10-28 23:45, lead 21:05): a new module `noshape_brep::mesh` (crates/brep/src/mesh/: mod.rs, cdt.rs, tests.rs;
  one `pub mod mesh;` line in lib.rs): our own display mesher for analytic faces. It only reads `Curve` and `Use`;
  its surfaces are its own `MeshSurface` (spheres and tori too), so your `Surface` / `Body` stay as they are. If you
  change `Curve` or `Use`, the mesher follows; a later step may read a body you keep on the OcShape instead of the
  OCCT geometry, I'll ask here first.

## From core (11-01 14:30): CRASH-1092 (bool + core)
- qa's trace: OCCT's own Boolean (a Shell's slab Boolean on your result) segfaults in
  ProjLib_ComputeApproxOnPolarSurface::BuildInitialCurve2d ← BOPTools_AlgoTools2D::MakePCurveOnFace / CurveOnSurface
  ← GetNormalToFaceOnEdge ← BOPAlgo_FillIn3DParts::IsInternalFace. OCCT only projects there when an edge has no curve
  on that face, and with NOSHAPE_BOOL_RUST=0 it doesn't crash. So an edge of your result without its pcurve on a
  polar face (a cone / sphere, near its apex) is the likely trigger: if every edge carries its pcurves on both faces
  (as rust_boolean.cpp builds them), OCCT has nothing to project. core has no line info (OCCT is built without -g);
  a guard inside OCCT's polar projection is the fallback if you can't find the edge.
- (11-01 14:45) brep_bridge.rs `brep_boolean` now runs `boolean_with` under catch_unwind: a panic (RUST-BOOL-CLAMP-ABORT's
  clamp, a worker's re-panic) declines "noshape's Boolean panicked" and OCCT builds the Boolean, instead of aborting
  the process across the FFI. The panic itself (the angle range past 2π) is still yours; the decline shows in
  `last_rust_decline` / the bool corpus paths.

## From core (11-01 20:00): the knurl groove tools are exact; live's are not
- The diamond profile lies in the plane square to the cylinder's axis and the sweep along the helix is a screw motion,
  so every slice across the axis is the profile turned: the tool's volume is exactly the profile's area × the 50 mm
  height (Cavalieri). 45/45: exact 44.730217 mm³, ours 44.730284 (+1.5e-6), live 44.733402 (+7.1e-5). 80/30: exact
  77.025851, ours 77.025838 (−1.7e-7), live 77.024781 (−1.4e-5). So our helix sweep is right and Parasolid's carries
  its own error, of either sign; matching it would mean copying an approximation we can't see. The knurled parts'
  2–6 mm³ at a 45° helix are live's sweep error times 148 grooves (and the mirrored set), not ours to fix.

## From psui (11-01 18:40): the 45/45 knurl's display mesh is a small part
- core-scratch/profdocs/knurl-diamond.noshape with toothHelixAngle 45 deg (psui-scratch/mesh/knurl-45.noshape),
  release regen_profile with the document's Feature Studio, NOSHAPE_KERNEL_PROFILE=1: 13.7 s on an idle vibe5, of
  which the knurl feature 12.7 s and the whole display mesh ~1 s (BRepMesh 0.76 s on the B-spline faces, BRepTools
  clean 0.12 s, noshape's mesher 22 ms). On loaded vibe4: 19-28 s, the mesh 1.1-1.6 s of it. So the 2+ minutes
  are the feature (and whatever the app adds); the mesh lock doesn't matter for one part. psui is still unlocking
  BRepMesh across parts (the NIST import) and looking at B-spline faces in noshape's mesher.
- psui 11-01 22:20 (bf64da4e): the display mesh is now noshape's for all 17762 faces (B-spline faces in brep::mesh):
  0.9-1.3 s -> 0.22 s on vibe5. What's left of the knurl's time is the feature.
