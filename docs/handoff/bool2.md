# bool2: handed over to bool (11-19 20:45 UTC)

bool2 is merged into bool (the user's decision, lead 11-19 20:40); this thread stops. bool owns everything below. Scope
was the tail of declined cases in noshape's own Boolean: crates/brep, crates/kernel/cpp/rust_boolean.cpp and the unify
in boolean.cpp. Rules: docs/DECISIONS.md, especially 9e5b481d4 (land on correctness), 0e603d4c9 (a case that differs
is worse than one declined) and "live over OCCT" (a gate exception needs a live capture). Older state:
docs/handoff/archive/bool2.md.

## Landed 11-19
- 41fcfd2c (15:54) bool2-cbs: closed B-spline faces' period, and a flat closed B-spline's seam as a slit of its plane.
  2284dd9c, 1a2a8447 and d0e4644e are ours and match OCCT. 2a5740b0 differs from OCCT as a gate exception: in live's
  capture live/bool2/q-fb-ell-asym-1119 the boss edge is one closed edge, as in ours. Knurls 0 diff, hats/dock SAME.
- 09ad4e81 (20:20) bool2-rim: a B-spline edge lying along another argument's face rim (between the same vertices)
  lays nothing inside the face. Shell's common f9b29e1d now matches OCCT (T1
  an_edge_along_another_arguments_face_rim_stays_whole). gate85: 0 new differing; knurls 0, hats/dock SAME. The first
  version (all curve kinds) broke shell::a_sunk_boss_keeps_its_wall_as_live and took shfall b5c954ea wrongly; the
  B-spline-only form passes both.

## Measurement left running (vibex8; it finishes on its own)
- Unit bool2-tdbg53b, script /root/bool2-scratch/tdbg53.sh, output /root/bool2-scratch/tdbg53b.out (this machine).
  It builds bool2-exact plus `mod xprobe;` (uncommitted in bool2-ab's crates/kernel/tests/all.rs) as
  vibex8:~/tmp/bool2/ab/kp-exact3, then runs xprobe::probe_case_volumes. Each case prints two lines, "VOL ours" and
  "VOL occt": faces, edges, vertices, valid, volume.
  - B997 (fz1144 b997c4c389ff7abe): in gate86 ours gave 4.0555e-6 against the recorded 4.05697e-6 (0.036 % off,
    T1 failed), yet the earlier replay (tdbg40) took it without flagging a difference. If "VOL occt" now gives ~4.0555e-6, the recording
    is stale (an older OCCT) and ours matches OCCT as it is today. If it gives 4.05697e-6, the exact plane lines alone
    leave ours 0.036 % off, and b997 needs the rest of the tail stack (split_params ends allowance + cycle-area chords,
    on bool2-taildbg).
  - CASE 6bb081c8e7192051 / 5101adb84ed0a575 (shfall, Fuse with 32 tools, recorded path direct; the same part twice).
    Ours gives 5.295354594164e-4 for both. The recordings say 5.2953546335e-4 (7.4e-9 relative) and 5.2953739068e-4
    (3.6e-6 relative), so OCCT's two runs already disagree with each other. "VOL occt" shows today's OCCT. Where it
    equals ours, only history is left: the replay diff (/root/bool2-scratch/tdbg53.out) lists the "rust only"
    records: arg0/arg1/arg10 Edge0 Generated Vertex at (57467, 39967, 18000) and (57467, 39967, 18967) mm, Modified
    Edges along x 57500, arg1 Face0 Modified Face. Main declines both as "an edge off a plane past its tolerance";
    bool2-exact's relaxed off-plane check takes them.

## bool2-exact: not landable (gate86 FAILED 19:58)
- Worktree /root/projects/noshape-wt/bool2-ab, branch bool2-exact, tip 2a8da63d4, base main e2abe938a. Commits:
  819a9c81d off-plane check by curve ends and vertex tolerance (bridge); 9b9c09518 a piece keeps its edge's own end within its
  merged vertex's tolerance (bridge); 957b0f4b9 new plane-plane straight edges on the planes' exact line (brep
  Output.edge_line, ffi edge_line, bridge Geom_Line); 50a079ef6 a new arc ending at a tolerant vertex ends where it
  crosses an input line ending there; 79eb8832f the tangent case of that; 2a8da63d4 T1 b997c4c3 changed from "declines" to
  recorded_as_occt (1e-9). That T1 was never run before the gate; it fails (see B997 above).
- gate86 (vibe2, /root/bool2-ab-scratch/gate-2a8da63d4.log): +2 taken, differing 6 -> 8 (the two shfall fuses above),
  T1 fails, and one document moved closer to live (c2-parting-line-draft Rib 1: edges 234 -> 225, live 214).
- Lead's plan 20:30: find why the plane lines take the shfall fuses and differ, fix that on bool2-exact, rerun the gate
  on vibex8 through the slot, and land the whole stack at 0 differing. If the two VOL lines show today's OCCT equal
  to ours, the differences are recording drift and history only. In that case re-record those cases (or fix the
  history) rather than declining (DECISIONS 0e603d4c9: history differences are fixed).
- The full tail stack: branch bool2-taildbg (same worktree) = WIP debug commits + real ones. b02a9be42 (split_params
  ends allowance; Line only), 62e8ccdad (arrange: cycle areas with chords between curves' ends), 43eebd26d,
  38bad0785, f1bf78ea5, 855863591, 7b7052072 (= bool2-exact's), 07df21fea (rim skip, landed in its B-spline form).
  With all of it, 11cc92b2/dbc28f46 and b997c4c3 match OCCT in volume and history, but 11cc92b2/dbc28f46 are "unified
  differently": ours 25 faces, OCCT 27. OCCT's own ShapeUpgrade_UnifySameDomain merges two 0.052 mm2 cylinder
  slivers on our shape but not on OCCT's (BOOL2_USD print: same cylinders bit for bit; most likely its pcurve
  u-origin relocation fails on OCCT's pcurves). Ours is probably live's, but this corpus case has no document, so no
  capture is possible. Landing the full stack needs those two kept declined, or a live document showing the same.

## Worktrees and branches (all mine; bool may delete them)
- /root/projects/noshape-wt/bool2-ab: branches bool2-exact (above), bool2-taildbg (debug + tail work, tip
  "WIP debug: SEAMARC"), bool2-rimdbg (debug). rb.sh target on vibex8 ~/agents/bool2-ab; probe binaries in
  vibex8:~/tmp/bool2/ab (kp-*), corpus subsets c42..c53 there. Uncommitted: `mod xprobe;` in crates/kernel/tests/all.rs.
  Untracked crates/kernel/tests/xprobe.rs holds the probes (probe_small_faces with PROBE_NB, probe_case_volumes,
  probe_case_faces2 with PROBE_UNIFY, probe_arg_faces).
- /root/projects/noshape-wt/bool2-xs (bool2-rim, landed), bool2-k25 (bool2-cbs, landed), bool2 (old; dirty with
  11-18 work, 1084 commits behind), bool2-ho (this handoff).
- Gate trees: vibe2 ~/agents/bool2-ab-gate (gate86), vibex8 ~/agents/bool2-ab-gate (gate84, stopped). Scratch dirs:
  /root/bool2-scratch (gate*.out, tdbg*.sh/out, kn*/udoc* scripts), /root/bool2-{ab,xs,k25}-scratch (gate logs).
- Env debug prints on bool2-taildbg: BOOL2_ARR / ARRAT (arrange's vertex fan), BOOL2_COINC (coincident pieces),
  BOOL2_TOUCH (section touching a boundary), BOOL2_SAME (section with both ends on one vertex), BOOL2_LONGELL (lets a
  plane nearly along a cylinder through, prints |n.a|), BOOL2_SEAMARC (arc across a seam), BOOL2_USD (unify attempts,
  boolean.cpp), BOOL2_SLIT / FLATSEAM / OFFPL / TWICE / CROSS / ELT (bridge), PN2, APEX, ROUNDS, SHORT, VOFF.

## Parked tail reasons (all on bool2-taildbg; none produce an OCCT difference)
1. "two edges leave a vertex in one direction":
   - 11cc92b2/dbc28f46: the unify slivers above.
   - gpf ee95f96d / b7a02f84: need more than 4 regroup rounds (BOOL2_ROUNDS=12); then vertices 4 and 5 at one place
     never merge (two identical lines).
   - 9f0b0c04 / 54630d70: see 2.
2. "coincident pieces differ" (gpf 9f0b0c04, 54630d70; corpus4 3cca316c), gear commons with a drill-tip cone.
   - Fix 1: piece_key keys a cone's Apex sedge by its vertex (one piece has a half turn there, the other the input's
     whole apex edge). All 3 then pass this check; corpus4 0 differ. This doesn't touch `rank` / `tree`; bool asked to
     be told before anyone changes those.
   - Then 3cca316c stops at "a non-manifold result"; 9f0b0c04 and 54630d70 stop at a 0.5 um sliver on the cone face
     (arrange seg 35: a circle cut at the face's 2D x = 0, with vertex 11 5e-7 from it; the boundary rulings at u
     -5.0e-7 / -4.25e-7 are skewed). Next: why th0 (gap() midpoint / round()) puts x = 0 that close to a ruling; snap
     the cut to the vertex.
   - Also on taildbg: section_from drops a curve with both ends on one vertex shorter than 2x that vertex's tolerance
     (was lines only). No regression, but it isn't this cause.
3. "a plane nearly along a cylinder" (gpf 1b0a04c4 fuse, OCCT 1.3 s; eee70623 common, 0.4 s): |n.a| 6.6e-4, r 25.9 mm,
   so OCCT's ellipse is 39 m long, tilting 61 um across the face. BOOL2_LONGELL lets it through with 0 differences.
   1b0a04c4 then stops at "two edges leave a vertex": a 3.8 um line to vertex 14 runs the same way as a 0.5 mm line
   that isn't split at 14. eee70623 stops at "no point inside a face piece".
4. "an arc across a cylinder's seam" (parity56 ce4e3c99 cut, 45 ms): a whole circle (r 14.35, z 114, vertex at angle
   2.25 pi) on a face whose th0 is exactly pi, so the circle is never cut at the seam. Next: why th0 isn't the face's
   seam line (round() / gap()).
5. "a section touching a face's boundary" (bool-corpus c38a6db8, 890acd4d): c38a6db8 also has a real slit (passes
   with BOOL2_SLIT), but the decline is a tangency: the tool ring's r 10 cylinder touches the box's y 0 / y 20 walls.
   Its own reason; not started.
6. Short sections (30, parity56, one sprocket-like family): parked by the lead 09:45. Cause and notes in the archive
   (17.7 um sections where tooth-gap tools' round tips bulge 25 um past the blank's cylinder).
7. OCCT's result invalid, so there is nothing to match: "a tiny arc" (63bd60d0, b70f339a, fda1114a, 91923f72), "an
   internal face" (b25113011, addb9a14), "no clean ray" 09489f90, "a closed edge" 0c6f868e, "a face's vertex off its
   plane" (f516559d, 1ba3f24a).
8. Others: "an inside-out tool" (869aabac, 5059c49e, 269c06e4: needs OCCT's complement semantics); "a shell whose
   orientation is unclear" (a3bfaa87, adeba320, 6cf25eee: near-zero shells); "cones meeting at an apex" (ba2e862f:
   invalid shell, InvalidMultiConnexity; 620253d9); 50a8ad1c (a 1 mm vertex group, declined by design); fz1144
   a46cf4d0 differs on main too (OCCT's result is one degenerate face).

## How to run
- Gate: `systemd-run --unit=<u> -E HOME=/root -E PATH=$PATH --working-directory=<worktree> bash -c "bash
  tools/bool/gate.sh <sha> --jobs 4 --host <h> > out 2>&1"`. One gate per host (a lock in /root/projects/noshape-wt/
  .bool-gate-<host>.lock). vibe2's release ops link was SIGTERMed twice under other builds; check `free -g` >= 15 GB.
  The gate skips the knurls and the private docs; run those as kn*.sh / udoc*.sh do (/root/bool2-scratch/kn85.sh,
  udoc85.sh). Private docs are read-only (mode 400): udoc-cmp.sh chmods its copies.
- Probe/replay runs: tdbg*.sh pattern (rb.sh build on vibex8 + scratch/buildcp.sh, then ssh with
  NOSHAPE_BOOL_CORPUS=<dir> NOSHAPE_BREP_VS_OCCT=1 NOSHAPE_BREP_WHY=" " ... brep_replay::brep_corpus_replay).
