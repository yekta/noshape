# bool: current state

Owner: bool (bool2 merged in 11-19). noshape's own Boolean (crates/brep, kernel cpp rust_boolean.cpp / boolean.cpp's
unify), the Shell builder (by_quadrics, shell.cpp), the speed of Booleans, Shell and Split. History:
docs/handoff/archive/bool.md (the 11-19 16:45 handoff is at its end) and archive/bool2.md. Rulings: docs/DECISIONS.md.

Updated 11-28 00:40 UTC. Hosts by load (DECISIONS 11-19): read
`ssh <h> cut -d" " -f1 /proc/loadavg` on vibex7 / vibex8 / vibe2 / vibex6 (crate tests only there); the Mac is back
(lead 21:36: headless, ~/noshape-host only, 2 slots): builds + probes there (NOSHAPE_RB_HOST=mac from worktree
bool-hat, inside an rb.sh slot, Mac paths ~/noshape-host/bool-keep); gates through ~/bin/slot, one per host, as systemd-run units
(a background task dies at 2 h); land queue vibe2 (`NOSHAPE_LAND_QUEUE_HOST=vibe2 tools/tiers/land.sh`); private docs
only vibex7 ~/lead-private. Never kill a process tree from a computed pid: `systemctl stop <unit>`. Agent ids: only
/root/lead-scratch/roster.md (read before messaging).

## P0 SPEED vs 0.4.73: LANDED (ccf1a1ed4d marcher opt-in NOSHAPE_BOOL_MARCH=1, f06939ddf6, fd7f64569e; 11-27 21:16)
- GAMMA lens LANDED 36f67e584d (gamma / wire-clip Boolean 2 886 -> 884 faces as live).

## DECLINES (state 11-28 00:40 UTC; lead: fix the 4 still declining, re-run the 21 inconclusive on green)
- Recheck dir /root/bool-scratch/nf (own copies of qa's nfrecheck.py / clifuzz_nf.py; bin/nfrecheck2.py = OTHER-OP rows
  only (NFONLY=<earlier out>) + "seen n": the reason anywhere in the run). 0.4.73: 16 GONE, 21 OTHER-OP, 4 STILL.
- 1846 "cones meeting at an apex": FIXED 8eae4ef17c (brep cyl_cyl), LANDING 00:40 (gate clean: replay +0 -0, decline
  reasons 0, suite 138 = 138 (its 2 changed docs = the lens already on main), shells 6 = 6, crates pass; knurls 5/5 0).
- 1192 "a non-manifold result" (a Fillet's common: 265 deg revolve with a torus face x half-space through its axis 0.955
  deg off its end cap): OCCT's section pcurve on the torus a turn up in v (2pi + pi/2), judged outside the face -> cut
  face a slit. FIXED 96623e8260 (section pcurves on a periodic free-form face moved by whole turns into its uvbox; T1
  bool_patterns::a_half_space_through_a_revolves_axis_beside_its_torus_end, recorded 9b6188a28c438ccf, fails without;
  bool_patterns 179, brep 68). GATING on vibex8 (unit bool-gate-96623e8260 from 00:35, chain-96623e8260.out); then
  knurls (kn-after.sh is vibex7-pathed: adapt to vibex8's gate dirs), then land both (bool-fb) and report.
- 1247 "an internal face": left to OCCT (Draft's defeaturing leaves 2 INTERNAL faces; OCCT's common keeps them).
- 1256 "no clean ray": no longer reached (path changes at step 36).
- 1234 "no point inside a face piece": minimised 69 -> 4 args (dump1234 decl7-{1,28,29,49}); arg 1 carries 4.7 um
  tolerance; a hyperbola edge's curve starts 4.4 um off its vertex, a 133 um sliver loop's area sign flips, no ray finds
  its inside. Needs a tolerant arrangement: left to OCCT for now.
- 21 OTHER-OP rerun on green 291923892 (nfrecheck-green.out): all still OTHER-OP (path build-dependent). Seen elsewhere in
  the run: 1192 non-manifold x4 (fixed above), 1192 "an edge on another argument's free-form face" x2 (its dumped cut
  dump1192/decl0 is ours and valid with default options on bool-fb, OCCT's invalid: recheck with a CLI of 96623e8260),
  2282 "OCCT's section failed" x1 (OCCT's). The other 18 not reached by their seeds.
- Probe tools (never commit): probe diffs /root/bool-scratch/probe-nm-1192.diff, probe-nm2-1192.diff (brep),
  probe-test-1192.diff (bool_replay probe_decl_pairs); dump CLI /root/bool-scratch/nf/cli-nf (catch(...) decline dump,
  NOSHAPE_PROBE_DECLINE_DUMP=<dir>, stderr "PROBE decl<n> op <0 fuse|1 cut|2 common>").
- Cleanup: /root/bool-scratch/nf/cli-nf, cli-green; vibex8 ~/tmp/bool/{d1234,d1192,rec1192}, cmp/cli-nf; vibex7
  ~/tmp/bool/kn4 after the knurls.

## Order (lead 11-21)
P0 impeller speed (c2-impeller-loft, blend measures per feature); P1 asm's SPEED-ASM-OPEN (infento). Gates + knurls
for every brep / Boolean / Shell land; list the corpora each gate ran. End a turn after each land.
Knurls: /root/bool-scratch/kn-run.sh <host> <dir> (systemd unit) with k-main / k-br = the gate trees'
all-96c035059fc28abb copied to ~/tmp/bool/<dir>; OCCT side ~/agents/bool-keep/knocc; delete the copies after.
Daily report: share as "ours N/M (general, direct), OCCT, differing" (DECISIONS 15c2c6406).

## NEXT (state 11-27 11:09 UTC; lead 04:0x: impeller early decline, nuts gate, unify, gamma slit fill)
- LANDED: stage 1 c9a9d4ed0, stage 2 2a4ab01fd, nuts 5e42155d5, unify f95397a1f. SLIT FILL QUEUED 0d6fc6b9 (bool-uv,
  branch bool-slits; gate 63ecbfdb3: replay +0 -0, knurls 0, crates pass). Lead 11-27 (DECISIONS f50149029): per-face
  match to live beats document totals; keep it.
- OPEN ROWS (gamma / wire-clip Boolean 2, case 317bec683037fff1, live capture
  /root/onshape-research/live/bool/gamma-b2-1127/boolean2-faces.json; score /root/bool-scratch/gamma-score.py on a
  probe_case_topo face dump, probes /root/bool-scratch/probes-topo.diff, never commit): ours 886/2411/1348, live
  884/2405/1344. Faces with live's area + edges 681 (main 666); (a) 7 live faces unmatched: the hole's two
  half-cylinders overlapping 50 um (z -7.814 / -7.764: live gives the overlap to the upper body, ours the lower) + two
  0.01 mm2 slivers at x 114.181; (b) the 6 extra edges / 4 extra vertices vs live (main's 2403/1344 was cancellation).
- IMPELLER EARLY DECLINE = 7ae48ba9e (worktree bool-early, branch bool-early): edges_trial() runs every pair's
  edges_in_pair before src.ahead, then puts back pool (Clone), regroup len, side_cache, in_face_done, pending len,
  faces' pcurves, face_segs/seg_section/seg_edge_tol lens; keeps only a decline. Case a7c0bb71d1230b3b (Fillet 3's cut):
  0.16 s decline vs 9.3 s of sections; T1 impeller_cut_declines_before_the_sections (k::last_ahead() == 0); kernel
  617/617; 5 knurls 0 diff. (b1024cf4a, the mutating pre-pass, made k25-71-12 decline 'a non-manifold result': vertex
  order changes results.) Lead: design fine, land at 0 worse. GATE 7ae48ba9e CLEAN (11:08): replay +0 -0, suite
  133 -> 133, speed 1.00-1.01x, shells 6, crates pass, knurls 0. Rebased onto main as ab9ee9b3f (brep_bridge/lib.rs
  conflict with stage 2's MARCH resolved: AHEAD counted at the top of Occt::ahead, last_ahead next to last_marched).
  ab9ee9b3f retested 11:23: impeller test 0.18 s, kernel 621/621, brep 68/68; landing (land.sh vibe2).
- NEXT (lead 11-27): gamma's 7 unmatched faces + 6 extra edges / 4 vertices (OPEN ROWS above).
- Cleanup after: vibex8 ~/tmp/bool/imp/rec; bool-uv / bool-ms2 / bool-fb targets once landed.

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
