# bool: current state

Owner: bool (bool2 merged in 11-19). noshape's own Boolean (crates/brep, kernel cpp rust_boolean.cpp / boolean.cpp's
unify), the Shell builder (by_quadrics, shell.cpp), the speed of Booleans, Shell and Split. History:
docs/handoff/archive/bool.md (the 11-19 16:45 handoff is at its end) and archive/bool2.md. Rulings: docs/DECISIONS.md.

Updated 11-26 09:40 UTC. Hosts by load (DECISIONS 11-19): read
`ssh <h> cut -d" " -f1 /proc/loadavg` on vibex7 / vibex8 / vibe2 / vibex6 (crate tests only there); the Mac is back
(lead 21:36: headless, ~/noshape-host only, 2 slots): builds + probes there (NOSHAPE_RB_HOST=mac from worktree
bool-hat, inside an rb.sh slot, Mac paths ~/noshape-host/bool-keep); gates through ~/bin/slot, one per host, as systemd-run units
(a background task dies at 2 h); land queue vibe2 (`NOSHAPE_LAND_QUEUE_HOST=vibe2 tools/tiers/land.sh`); private docs
only vibex7 ~/lead-private. Never kill a process tree from a computed pid: `systemctl stop <unit>`. Agent ids: only
/root/lead-scratch/roster.md (read before messaging).

## Order (lead 11-21)
P0 impeller speed (c2-impeller-loft, blend measures per feature); P1 asm's SPEED-ASM-OPEN (infento). Gates + knurls
for every brep / Boolean / Shell land; list the corpora each gate ran. End a turn after each land.
Knurls: /root/bool-scratch/kn-run.sh <host> <dir> (systemd unit) with k-main / k-br = the gate trees'
all-96c035059fc28abb copied to ~/tmp/bool/<dir>; OCCT side ~/agents/bool-keep/knocc; delete the copies after.
Daily report: share as "ours N/M (general, direct), OCCT, differing" (DECISIONS 15c2c6406).

## NEXT (state 11-27 01:36 UTC; lead 01:1x: stage 1 land, stage 2 gate, nuts land, unify under the full gate)
- STAGE 1 = 568ee31d2 LANDING (gate dc6352480: replay +0 -0, suite 131 -> 131, speed 1.00x, shells 6 -> 6, crates
  pass; knurls on vibex7 (/root/bool-scratch/kn-s1.sh): 0 diff lines in all 5).
- STAGE 2 = 0c3bcfec3 (bool-ms2, branch bool-ffm2 on stage 1): gate unit bool-gate-0c3bcfec3-x8 running since 01:17
  -> /root/bool-hat-scratch/chain-0c3bcfec3.out; then knurls (kn-s1.sh pattern: copy the gate trees' binaries first).
  Ahead() runs march::own for ext x ext, ext x nurbs, ext x revolution, rational nurbs x torus; NOSHAPE_BOOL_MARCH=0
  off. Report docs moved + per-family ms (5 vs 66, 41 vs 199, 8 vs 26, 11 vs 164).
- NUTS DIVIDER = 44b3ab42e (bool-fb, branch bool-nuts-div on 0c3bcfec3; skasm's exact-cylinder patch inside): cylinder
  faces going round > a turn divided by iso-u lines, rejoined after. Nuts removes 8.615 (live 8.6), c2-config-parts-nuts
  17/17 (OCCT only 14/17); kernel 617/617, clippy, GCC 13 ok. Gate after stage 2's, land, tell skasm. bool-fb's
  brep_replay.rs probes uncommitted (/root/bool-scratch/probes-fb.diff): never commit.
- UNIFY (lead GO: after the three above; gate all corpora + parity56 + knurls + full model suite, 0 new differences or
  each closer to live). bool-uv, branch bool-unify-verts on 44b3ab42e, UNCOMMITTED, probes removed: OcShape::
  myInputVertices (op_boolean, !opts.unify); unify_keeping_inputs at every Boolean unify (rust, local, trimmed, OCCT
  paths) and op_unify: input vertices the unify removed with no edge of theirs removed -> rerun with t_unify_keep
  (keep_marked_edges KeepShape, merge_planar mergeable, join_loop_pieces joints skip them); merge_planar collinear =
  shared vertex within 1e-7 m of the far ends' line, between them. Rib 1 now 73/214/144 = live. T1
  boolean_groups::a_fuses_unify_joins_edges_only_at_the_booleans_vertices (data/topology/parting-line-rib1.brep.zst).
  Kernel 617/618: bool_patterns::an_invalid_fuse_result_unifies_as_usd 1173 -> 1131 faces (louver's invalid fuse
  result, only the distance test applies): look why before the gate.

## NEXT (state 11-26 22:40 UTC)
- IMPELLER MOVE FACE 1 (core P0 via lead): reported 22:15, NOT ours: the regen records the fuse as path occt, empty; ours
  declines ("a point near two vertices"); OCCT is empty on core's files (vibex8 ~/tmp/bool/mf1-bool, mode 700) with
  rust 0/1, fuzzy, serial, swap, scaled. RUST=0 is Ok only because its body differs (upstream feature 15 / Fillet 3).
- NUTS Sweep 1 (skasm's patch, bool-fb's copy /root/bool-scratch/bool-fb-nuts-applied.diff): the cut (recorded vibex8
  ~/tmp/bool/nr/rec/8f220cf13f4e37aa) goes to OCCT (ours declines the B-spline flanks) and fails: tool crest = exact cylinder
  r 1.621 on the bore's own cylinder, ONE face over 7 turns (u 5.8..50.7); OCCT common 0, cut removes 26 mm3 (live 8.6),
  18 non-manifold edges. Point classification is fine; OCCT's same-domain cylinder handling of a >2pi face is not.
  ShapeDivideAngle doesn't split it. Fix idea (not started, tell lead): split >1-turn cylinder faces at u=2pi*k before
  the Boolean (pcurves projected are wrong period: split_on won't do), rejoin after; or skasm's sweep makes per-turn faces.
- RIB 1 (parting-line, recorded vibex8 ~/tmp/bool/nr/recpl/f5c8541ff0c9e0c3, fuse BOTTOM + 8 ribs): ours = OCCT exactly
  (149/354/208, unified 73/213/143, live 73/214/144). Our unify (boolean.cpp ~12440: keeps every vertex not freed) leaves
  e136 8.338 + e137 0.224 mm collinear lines between faces z 3 and x -21.5 (vertex shared by only those two). Vertex
  policy question, not the Boolean; parked.
- STAGE 2 IN PROGRESS: worktree bool-ms2, branch bool-ffm2 on bool-ffm (uncommitted): march::family() (kinds nurbs /
  rational nurbs / torus / sphere / extrusion / revolution) + per-family counts in free_sections_vs_occt. Run over all
  corpora (vibex8 ~/tmp/bool/ffms2.out, kt-ms2): 0-differ families with matches: extrusion x extrusion (4, ours 5 vs
  OCCT 66 ms), extrusion x nurbs (11, 41 vs 199), extrusion x revolution (23, 8 vs 26), rational nurbs x torus (2, 11
  vs 164). Differing: nurbs x nurbs (3: shfall 08547623), nurbs x sphere (2: sp-cut 990256f0) -> OCCT. Others 0 matched.
  DESIGN (to write): march.rs FAMILIES + set_on (kernel: NOSHAPE_BOOL_MARCH=0 off) + takes(); lib.rs Output.marched
  Vec<Marched{faces [(arg,face);2], on [NurbsCurve;2], tol}> (blend.rs/prism.rs ctors too); boolean.rs ahead block
  (~5800): pairs of those families, not canonical pairs of moves, marched by par_map, ids OWN=1<<31|k, removed from
  src.ahead; occt_sections: cx.marched first (reverse order: swap on); kernel fill_result: march_faces (4 u32) + march
  (on0/on1 offsets via push_curve, tol) new BrepResult fields; rust_boolean.cpp type-5 branch (~1911): id&OWN -> EllEdge
  from decoded NURBS (3D at cv[1], 2D pcurves) and input(srcs, arg, FACE, idx) faces. T1, then gate + parity56 + knurls,
  A/B per family (docs moved, time).
- Probe trees (never commit): bool-mf (old main 13:45; probes in brep_replay.rs + features.cpp NOSHAPE_PROBE_ANGLE),
  bool-fb (detached origin/main c49a13ee0 + probe_pair_files). vibex8 binaries kt-fb, kt-main, kt-ms2, recordings
  ~/tmp/bool/nr, mf1-bool; vibex7 ~/tmp/bool/nr (ops-nuts, rec*): delete when done.

## NEXT (state 11-26 21:40 UTC)
- LANDED c31e98add (asm-scissor-lift Hole 2 falls back). Cleanup due: vibex7 + vibex8 ~/tmp/bool/hole2,
  /root/bool-scratch/h2, vibex7 ~/tmp/bool/knfb, bool-fb's target.
- MARCHER (lead 11-26 decision, DECISIONS): own free x free sections, staged: (1) standalone intersector + replay-pair check
  vs OCCT, landed unused (time-box ~2 days from 11-26 13:45, then report whether to go on); (2) on per pair family only at 0
  differences in gate + parity56 + knurls; (3) numbers each stage: pairs matched, time vs OCCT, documents moved.
  Worktree bool-march, branch bool-ffm (WIP commits, squash before land). crates/brep/src/march.rs (seeds, walk, Hermite
  fit, Fail::{Tangent, Coincident, OnEdges, Crowded, Lost, Unfit}); harness brep_replay::free_sections_vs_occt.
  STATE 16:10 (committed in bool-march with this handoff): along_both = ALL 31 points along an edge of each face (was 2
  in a row); fit: pieces under SHORT 1e-5 of the box not split (degenerate spot: b's pcurve tangent unbounded), a branch
  not fitting in 12 rounds = Fail::Unfit (was silently dropped: the "ours 0 / OCCT 1" pairs 1963 a3 b2, 424e a1/a3 b2,
  ae73 a1 b6, e49bd a30 b2); TOUCH_SIN 1e-2: a section whose normals come within that sine -> Fail::Tangent (9040230e
  a10 b2 sine 1.8e-4, ae73 a1/a3 b1 4e-3, OCCT 0; costs 4 matched at 5.4e-4: not separable; 11-19 ruling: declining
  beats differing). Harness: points within 10 um of an edge of EACH face counted apart ("differ only along both faces'
  edges": OCCT keeps/drops those by its marching: same geometry 1963 a0 b1 OCCT 0 vs 424e a0 b1 OCCT 1), OnEdges
  declines counted there; worst pair named. Tried + dropped: OnEdges for sections along ONE face's edge (matched 6).
  Numbers rec-imp+rec-lp (vibex8 ~/tmp/bool/ff20.out): 197 pairs: none 121, edges-only 24, matched 25 (worst 9.9e-7 at
  1963 a3 b2, the degenerate one: inside 1 um, check whose), DIFFER 0, declined Coincident 11 / Tangent 14 / Lost 2;
  matched ours 87 ms vs OCCT 525 ms (6x), all compared 575 vs 1988 ms. Clippy brep+kernel clean (16:00).
  STAGE 1 REPORTED 21:30, lead approved: land unused through the gate (squashed: branch bool-ffm = one commit on
  main; the WIP history kept as branch bool-ffm-wip). All 23 corpora (fix rerun ff23 in flight, vibex8 unit bool-ff23 ->
  ~/tmp/bool/ff23-<corpus>.out): 884 pairs: none 406, edges-only 71, matched 324, declined 79 (Coincident 37, Tangent
  29, Lost 7, Unfit 4, Crowded 2), DIFFER 4; matched ours 1588 vs OCCT 2194 ms. The 4: sp-cut 990256f0 a0 b0 (ours adds
  a stretch up to 54 um off OCCT's curve, on both faces to 1e-11; a3 b0 there was a harness polyline artifact) and
  shfall 08547623 a13 x b129 (ours 1 / OCCT 0), b163 (0 / 2), b166 (0 / 1). Hang fixed: shfall 05997f93 / 08547623
  seed search split cells forever (two smallness tests disagreeing by ulps): one test, VISITS 2^20 cap, T1
  a_sphere_cut_by_a_plane_with_offset_parameters_is_one_circle (hangs on the old code).
  STAGE 2 (lead 21:35): switch on only the pair families with 0 differences over all corpora + parity56 + knurls; the 4
  differing pairs' families and every declined pair stay OCCT's until settled; report user documents moved and time vs
  OCCT per family. Order: land stage 1, then gamma/wire-clip, nuts Sweep 1, Rib 1 interleaved with stage 2;
  correctness first.
  Open: e74bf Tangent x5 / Lost, 79058 a3 b2 Lost, 730b Tangent (edge-on-edge).
- AFTER THE MARCHER REPORT (lead 11-26, from skasm):
  (a) c2-gamma-imported and c2-wire-clip-imported first differ at Boolean 2 (imported bodies) after live's data refill:
  ours or the import? (vibex8 ~/parity/models/<doc>; NOSHAPE_BOOL_RUST=1 vs 0.)
  (b) nuts Sweep 1: a thread strip comes out a B-spline where live has a cylinder; made exact (skasm's patch
  /root/skasm-scratch/nuts-helical-cyl.patch) the cut fails non-manifold: our cylinder merging.
  (c) parting-line Rib 1: edge merging after the union: we merge each side wall's edge into one where live splits it in
  two, and split off a 0.224 mm piece near the base at x -21.5 where live keeps one line.
- Coverage ranking DONE: every decline >= 100 ms parked: partial-fillet 7340044b (pf-secant-order-probes.patch; 57 um micro edge),
  tangent family beb5d239, setback df44fb85, loft-paths e74bf29d (tb-probe.patch), body-draft e13ba873 (tool B-spline index 2
  lies on target plane y 14.58 mm along a straight stretch of its profile, then leaves it C2 past a knot: the overlap's edge is a
  tangent contact -> OCCT's; OCCT's result there is invalid, the doc's first live difference is Fillet 1 just after, not shown
  related). replace-face 5925baca: target from our Replace face inconsistent (+19 mm3 vs live): asked lead whether core's.
- SPEED LIST re-measured 11-26 10:50 on main e6bae0fd7 (vibex8 ~/tmp/bool: cli-main, models = copies of ~/qa-parity/saved,
  fl-main2.sh -> fl-main.jsonl, sel-main.jsonl 69 rows >= 100 ms and >= 2x live, prof-main.jsonl; recfeat.py names each recorded
  case's feature: rec-imp, rec-kh, rec-lp).
  SEED 8d1875822 (worktree bool-seed, branch bool-seed, on main 076da3e5f): seeded_sections stops projecting once 5 steps in a row
  come no closer (impeller Fillet 4's cut 9040230e: one seed stuck 7.9 mm apart 400 steps on an offset surface, 1.5 s; ours 1662 ->
  189 ms, OCCT 359, 0 differ; rec-imp 2 differ as before: 549d67 unify 76 vs 77, 79058fda OCCT wrong). Unit bool-chain-seed
  (/root/bool-scratch/chain-8d1875822.sh -> kn-8d1875822.out) waits for bool-chain-b, gates, knurls, lands bool-seed. No
  handoff commits in bool-seed (it is on an older main: handoff conflict).
  Checked, no Boolean fix: impeller Fillet 3 ae73c9d8 / Extrude impellerraum 6fd99e06 'a point near two vertices' (one of our points
  reaches 2.6 mm, input vertices 0.22 mm from the fillet: nv-probe in mf-probes-1126b.patch); knifehandle Fillet 1 856a411e (OCCT
  2 s, invalid; tool edge 4 = the fillet's contact line on the handle face, 0.68 mm off our face: tangent family; doc matches live
  there); impeller Chamfer e0dcafbf (tool plane x 1.5 mm through a spot three target faces meet within 6 um, 2-edge sliver);
  loft-paths Loft 5 e49bd394 (ours 302 vs OCCT 408: one B-spline x B-spline pair's OCCT section 280-365 ms; own B-spline sections
  would be the lever); rings Shell 1 (by_quadrics, repair ~420 ms; A/B without the parallel second try: rings 500-650 vs 600-850
  noisy, impeller worse without it: kept). Not Boolean (core's?): impeller Delete face 2 1414/11, surf-intro Derived 1 x5 ~512/65,
  parting-line Split 3 631/31 and Drafts ~350/20, sm-rocker Mirror 3 225/8.
- Idea: replay could run OCCT with set_boolean_no_tree for occt_tree cases (pcb 101003c5 then comparable).
- Cleanup: vibex8 ~/tmp/bool {kt-slow, wt1, rf1, models, rec-imp, rp1, fl-work, p0}; bool-sf/bool-mf targets after the lands.

## How to run
- Worktrees: bool-hat (bool-pole; Mac builds, `bash .bin.sh <name>` inside rb.sh builds xprobe into
  ~/noshape-host/bool-keep/bin/<name>; uncommitted probe lines in crates/kernel/tests/all.rs, xprobe.rs, sampler.rs,
  the Shell 3 debug; never commit them), bool-tube (gate tree + bool-land7), bool-docs (handoff), bool-split (old).
- Gate: `systemd-run --unit=bool-gate-<sha>-x8 --collect -E HOME=/root -E PATH=$PATH -E NOSHAPE_GATE_HOST=<h>
  /root/bool-scratch/gate-chain-own.sh <sha>` (detaches bool-tube at the sha) -> /root/bool-hat-scratch/chain-<sha>.out
  ("VERDICT", "CHAIN DONE rc N"; rc 3 = CHECK only, e.g. knurls not run). Knurls: kn-s7.sh pattern. Private docs:
  /root/bool-hat-scratch/udoc-ab.sh <commit> pattern (bool2's udoc85.sh), copies mode 700, deleted after.
- Land: `git diff --stat $(git merge-base HEAD origin/main) HEAD` first; `NOSHAPE_LAND_QUEUE_HOST=vibe2
  tools/tiers/land.sh`; `tools/tiers/land.sh --status`.
- g++-13: /root/bool-scratch/gcc13-hat.sh <file.cpp>.
- Recordings: NOSHAPE_RECORD_BOOLEANS=<dir> on a CLI / parity run; replay `NOSHAPE_BOOL_CORPUS=<dir>
  NOSHAPE_BREP_VS_OCCT=1 NOSHAPE_BREP_WHY=" " <kernel test bin> brep_replay::brep_corpus_replay --exact --ignored
  --nocapture --test-threads=1`. Corpora: vibex8 / vibe2 ~/agents/bool-corpora, Mac ~/noshape-host/bool-keep.
- Probes (xprobe.rs): probe_case_timed, probe_case_sample, probe_case_nonmanifold, probe_case_unify, probe_shell_dump,
  probe_split_dump, probe_who_is_right. Debug switches: NOSHAPE_FREE_SECTIONS=0, NOSHAPE_OWN_SECTIONS=0,
  NOSHAPE_SPLIT_DEBUG, NOSHAPE_SHELL_DEBUG, NOSHAPE_BOOL_SLAB.
