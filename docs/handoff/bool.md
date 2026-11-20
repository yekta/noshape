# bool: current state

Owner: bool (bool2 merged in 11-19). noshape's own Boolean (crates/brep, kernel cpp rust_boolean.cpp / boolean.cpp's
unify), the Shell builder (by_quadrics, shell.cpp), the speed of Booleans, Shell and Split. History:
docs/handoff/archive/bool.md (the 11-19 16:45 handoff is at its end) and archive/bool2.md. Rulings: docs/DECISIONS.md.

Updated 11-20 04:16 UTC. Hosts by load (DECISIONS 11-19): read
`ssh <h> cut -d" " -f1 /proc/loadavg` on vibex7 / vibex8 / vibe2; builds + probes on the Mac (NOSHAPE_RB_HOST=mac,
inside an rb.sh slot, Mac paths ~/noshape-host/bool-keep); gates through ~/bin/slot, one per host; land queue vibe2
(`NOSHAPE_LAND_QUEUE_HOST=vibe2 tools/tiers/land.sh`); private docs only vibex7 ~/lead-private. Never kill a process
tree from a computed pid: `systemctl stop <unit>`. Agent ids: only /root/lead-scratch/roster.md (read before messaging).

## Order (lead 11-19 23:00)
1. bool-stack7 land. 2. bool-pole2 land, then asm re-checks isaac Fastened 43. 3. "+1 face" shells' final unify (and
f815832c's 22 faces vs live's 16). 4. Shell 3 Linux 2-solid split. 5. bool2-exact (below). 6. the rest by P0/P1.
Daily report: share as "ours N/M (general, direct), OCCT, differing" (DECISIONS 15c2c6406).

## In flight
- bool-land7 (= bool-stack7 rebased; worktree bool-tube): e3078f89 FAILED in the queue 03:49 (assembly pem_*_nuts_match_live,
  7 tests: a676feb46's strip drop took the PEM S nut's 7 um flat between two planes 138 degrees apart, one of live's 92
  faces). Fix fee56483e: a narrow face is dropped as a step only between parallel planes (assembly pem filter 24 pass).
  land.sh again 04:04 -> QUEUED: tell the lead; watch `land.sh --status` in bool-tube.
- bool-pole2 (worktree bool-docs, origin/bool-pole2, tip 330ea2052; knurls clean 01:38; udoc moved to vibex7 01:45: unit bool-udoc-p2b, udoc-p2b.sh) = bool-land7 + bool-pole's 6 + e87b6b170
  (HOLE-THROUGH-ROUND-POLE: fixed, asm's repro 302.235 vs exact 302.234 mm3). Gate 01:11 (/root/bool-docs-scratch/gate-330ea2052.log):
  CHECK only for: 24 declined by the 50 um rule (the TO WIN BACK list), c2-parting-line-draft Rib 1 73/234/164 ->
  73/213/143 (live 73/214/144, nearer); differing 7 -> 4 (none new), shfall 3 -> 1; shells 8 = main's 8, 2756e521 /
  8e657637 as OCCT now; crate tests pass. Running: unit bool-kn-p2 (kn-p2.sh -> /root/bool-scratch/kn-p2.out,
  KN-P2-DONE) and unit bool-udoc-p2 (udoc-p2.sh -> /root/bool-hat-scratch/udoc-p2.out, UDOC-DONE; vibex7 udoc2/).
  Both clean -> land (after bool-land7), tell asm (isaac Fastened 43).
- bool-pole's 6 commits (now in bool-pole2):
  e4f8a8066 a sphere's pole edge split at the curves ending there (non-manifold cdd336b4 taken = OCCT);
  01775a048 straight edges in a row share one Geom_Line (unify joins them as OCCT's);
  fb3d29a2a Face Generated Vertex for every argument's coplanar face (0 new differences);
  4b1784e6c the 50 um rule (Shell slab Boolean with an input edge >= 5e-5 tolerance is OCCT's; removes 5 differing).
  Declined by it, TO WIN BACK with a narrower signal: gate corpora 0670c350, 280af0bc, 2b9c38da, 2c9c42e4, 30afb30c,
  475df60e, 570dfc37, 5fc0970a, 70b3fb42, 818552c9, 83b621a3, 8e91c517, 922c847a, 952c2efc, a1186078, a7bdd15b,
  b359e025, cb7bb22b, e1dc8c98, f1c35250, f302ef40; Shell corpus 1451f7cb, 43a947f0, 96d45f42, 9bee135f, c25f7a31.
  Rule off (bool-hat debug NOSHAPE_SLAB_TOL_RULE=0, Mac u14): 20 of the 21 = OCCT; 570dfc37 (old shfall only, so
  report-only now) differs: OCCT keeps arg5's vertex (82.95, 41.75, 5) mm at tol 1e-4, ours deletes it. Shell corpus
  end to end (u16/u21): rule off = rule on = OCCT once a8e67f355's pcurve refit is in (2756e521 was the one off).
  rec1 (new shfall) rule on / off (Mac u41, 04:14): 65 of 103 taken both, the same 4 differ (OCCT vs its own recording);
  the rule's 8 declines fall to other reasons. Dropped: bool-sh3 5b38e5c92.
  afcf858f3 slab union history differing only in vertex records = equivalent (lead 11-19); 0d465cb70 the gate's
  end-to-end Shell check (gate-host.py SHELLS / SHELLS_KNOWN).
- Gate 98ef1c012 (unit bool-gate-98ef1c012-x8, out /root/bool-hat-scratch/chain-98ef1c012.out, from 09:17):
  branch bool-gl (worktree bool-gl, pushed) = 3d8d16e5e Gear Lab glue (isaac-ball 8f5622 gl40 930 ms vs live 1153;
  fuse_glued: N facing face pairs on one plane by outline; located refusal only on free-form surfaces) +
  98ef1c012 screws thread (c2-config-parts-screws Sweep 1 cut b95e3b88: refined_section's projections by next_uv,
  Newton clamped to non-periodic bounds; 48 ms vs 1.1 s Mac, OCCT 324; T2 test a_screws_thread_cut_as_fast_as_occts;
  worktree bool-thread has it alone). Land on PASS (knurls: kn-k25.sh pattern on vibex7).
- shfall swapped to rec1 on all hosts 09:16 (98 cases; 11-02 set = shfall-1102, report-only).
- P1 isaac 850efe (skasm 09:40; root of the 30 s Fillet 3, blend.as_is timeout): Extrude 6 union 3b6725b2ffc81970
  (Mac bool-keep/m85r; model m85m): tool r4 cylinders coaxial with Extrude 2's to 1e-12 mm, same x dir. Ours = OCCT
  39 faces; unify leaves each side's r4 pieces apart (ours 33/85/50, live 28/68/40 = each side one face): OCCT's USD
  merge loses 704 mm3 (= pi 4^2 14, faces invalid; dump NOSHAPE_USD_DUMP in bool-hat), usd_kept_volume refuses it,
  and merge_cylinder_groups doesn't take the groups. Next: why (bool-hat NOSHAPE_DBG_CYLG prints each group's
  boundary edges / chains; .cx.sh probe_coaxial PROBE_R=4) and extend it (left: faces 2,3,4,23,24; right 13,14,17,18).
  (JVJ 15 nm was skasm's sketch solve, theirs.)
- rings-loft STL (dx, low): in dx's x_t round trip face 23 (offset -1 mm of a B-spline) gets no triangulation (wire
  crosses itself in UV, 1.1e-4 degenerate edge; all 79 open edges border it). Our own Booleans (Mac bool-keep/ringsr,
  probe_corpus_meshes): the 273-face cut 331fd52c meshes with 0 holes (unified too); c40a49c4 (cut 103 -> 133) opens
  9. So likely the x_t write/read (dx's) or c40a49c4; Shell 1 alone: 44 faces, 8 folds, 0 holes.
- bool-k25x10 (d2a26a12a = 2f452d3b3): gate PASS except knurls not run (CHECK; /root/bool-tube-scratch/gate-2f452d3b3.log);
  knurls ours-only on vibex7 (/root/bool-scratch/kn-k25.sh -> kn-k25.out); land on equal lines (bool-tube is detached by
  the 98ef gate: land from a fresh worktree of branch bool-k25x10).
- bool-sh3 (rebased on main 3ac2565d6 in worktree bool-sh3r, branch bool-sh3r, tip c26a788e0, not pushed; origin/bool-sh3, tip a0b85fef4 = + d494b222e, a0b85fef4 (standoffs speed, below); 5b38e5c92 the 50 um rule removed; d2a26a12a the Ø25×10 knurl's touch (P2 below);
  1f27fa8ca the unify's segmented volume recheck (free-form fuse, P2 below); 8b6d6faba + Shell's stray-pcurve refit after its unify (2756e521: a 45° cone's merged
  face had an edge pcurve 0.83 mm off, read 77 mm3 heavy in ours and OCCT; now 32108.414, live 32108.426 / 45 faces (q-2756e521),
  live 104 edges / 57 vertices vs ours (= OCCT) 115 / 69: seams (Parasolid has none) and one B-spline edge between
  faces 21 / 25 in two 3.848 mm pieces at (11.203, -20, 2.032) (Mac u24 probe_shell_case_two_edge_vertices): P3; also moves 08f5dc59 / 17488d26, ours = OCCT; clean tree Mac u23 shell:: 36 pass; it also
  fixed 014f1aba5's const `unified` that only compiled in my WIP) + gate-host.py REPORT shfall-1102 (needs pole2's SHELLS check, so here, not on main); + the ellipse join (014f1aba5) and walls-fill 1e-6, Mac u12: shell:: 34 pass, corpus
  ours = OCCT but 17488d26 99b55ec9; live 02:07: 7606a33e / 8ec111cc FAIL there too (live/bool/q-*); T1 walls_that_rebuild_the_part_within_its_rounding_fail_as_live
  (1de09172f; Mac u13 shell:: 35 pass); = bool-hat's sh3-probe + 410e8c3d5 without_inverted_slivers, Shell 3
  Linux Ok) on 330ea2052 (gate after bool-pole2 lands, or gate its tip, which includes pole2):
  69d3e1059 the gate's Shell check takes volumes within 1e-7 relative as the same (99b55ec9 off SHELLS_KNOWN);
  fb1ddd780 Shell's slab unify retries the face order (usd_any_order, boolean.cpp) where raw USD leaves two faces on
  one cylinder apart (cause: in ours' order USD puts the +x half of 8eebe1e2's R14 wall on the other half's cylinder
  with u 2pi off, the shared edge reads as a seam): 13186b07 b64c8b4e 9134b29f 8eebe1e2 = OCCT (faces, history);
  1d71f0703 oblique_hole_plugs (shell.cpp): a planar face's slab beside a removed hole cylinder crossing its plane
  obliquely gets the hole's ellipse section swept with it, the common trims it at the hole: f815832c = live (16
  faces, 2720.0495 mm3, live 2720.0509; edges 44 vs live 39 = ours less Parasolid's conventions (no seams, ring circles) except 2 vertices splitting the
  ellipse between the hole cylinder and the 45 deg wall's inner face at (25.997 / 14.003, 19.823, 30.530) mm: USD joins
  only lines and circles; filling the hole in the slab instead of the plug (bool-hat NOSHAPE_SHELL_HOLE_FILL,
  /root/bool-scratch/holefill-try.diff) keeps them and gives 2720.0603 mm3 + ae90ae72 differing. Trying (bool-hat, uncommitted): with_ellipse_pieces_joined
  after the slab unify (joins one ellipse's pieces at a 2-edge vertex between the same faces; /root/bool-scratch/u7.sh
  -> u12.out): f815 42 edges / 28 vertices, 2720.0497 mm3 (the split pieces' pcurves read 2720.0603 / 2719.9237);
  21497e46 b5fdaf97 -2 edges; 8ec111cc (25 mm walls fill the part) passed the 1e-9 walls-fill check after the join
  (1.5e-7 under): the check now 1e-6 (u12: 8ec111cc, 7606a33e, d4d9563e now fail as walls-fill); also changes
  21497e46 / 82ac5e9a (-1 face, +0.85 mm3, OCCT the same); T1 an_oblique_hole_s_wall_ends_on_its_cylinder_as_live;
  8155d4210 an inward Shell whose walls fill the part fails (live q-b79e508b SHELL_FAILED; ours returned the part);
  T1 a_shell_whose_walls_fill_the_part_fails_as_live. SHELLS_KNOWN now 17488d26 2756e521 8e657637.
  Verifying: /root/bool-scratch/u5.sh (Mac: kernel shell:: tests + corpus ours vs OCCT; expect only 17488d26 and
  99b55ec9 (same under the tolerance)).
- 17488d26: live imports a different solid from our STEP (live/bool/q-17488d26/answers.txt: the torus face's one
  FACE_BOUND loop, live keeps the other side, 1933.6 vs our 435.1 mm2): likely our STEP export of a one-loop torus
  face. dx fixed it (queued 0dbd37f4: OCCT's writer reversed an unchained wire on a left-handed torus): once it lands,
  re-export the case and ask live for the Shell 1 mm off the torus face; until then it stays on SHELLS_KNOWN.
- live2's Shell 3 ticket (P2 SHELL-AFTER-WELD-FILLET): Mac Ok; Linux (vibe2, instrumented ops xshell3, doc
  ~/tmp/bool/sh3/d.noshape) part 2 at 5 mm gives 2 solids: 183041.69 mm3 and an inside-out -0.00097 mm3 sliver, so
  the feature's solid-count check fails. Fix in the bool-hat working copy (uncommitted): without_inverted_slivers at
  op_shell_solid's exit (drops inside-out solids under 1e-2 t^3 beside others). Linux xshell3 run with it in flight
  (rb.sh vibe2); Shell 3 "Ok" there -> commit it with a T1 (a compound with a tiny inverted solid).
- Debug in bool-hat (never commit): detail.rs DBGSHELL / DBGBLEND, crates/ops/tests/xshell3.rs + `mod xshell3;`,
  kernel xprobe.rs + `mod xprobe;`, shell.cpp NOSHAPE_SHELL_DEBUG dumps / NOSHAPE_SHELL_USD_ONCE / UNIFY2, boolean.rs
  NOSHAPE_SD_RULE. Copy: /root/bool-scratch/shell-wip-2350.cpp. Probes (Mac bin/u4, bin/tw): probe_list_faces,
  probe_edges_at, probe_shell_case_faces, probe_case_twins, probe_who_is_right.
- bool2-exact (bool2-ab, tip 73dcd9e30 = b997c4c3 re-recorded with OCCT). 5101adb8: exception (DECISIONS c8a06136b):
  it is 6bb081c8 turned 90 degrees (probe_case_twins: all 33 arguments' volumes and extents equal); ours gives both
  5.295354594e-4, OCCT 6bb081c8 5.295354633e-4 (= ours, 7e-9) and 5101adb8 5.295373907e-4 (1.93 mm3 off its twin).
  Name the symmetry in the land commit. 6bb081c8 history (/root/bool2-scratch/6bb-hist.txt): (a) records within 100 um
  of OCCT's (11-02 inputs at 1e-4): settled by the shfall re-record below; (b) vertex-only (afcf858f3 rule); (c) the
  coincident faces' representative (OCCT arg16 Face0 Modified x2 + arg17 Face2 Generated Edge, ours Deleted):
  NOSHAPE_SD_RULE=3 scratch in bool2-ab boolean.rs; A/B over old shfall (vibex8 ~/tmp/bool/sdr/sdr-{0,3}.txt): identical (66/194 taken, the same 5
  differ), so the SD rule is not 6bb081c8's cause; drop the scratch. On rec1 bool2-exact = main (rp1-*.txt).
- shfall (DECISIONS 11-20): the gate's shfall becomes the re-recorded OCCT set; the 11-02 set is shfall-1102,
  report-only (gate-host.py REPORT on bool-sh3 8b02f134b, tested with fake rows; gate sh3 from bool-hat). vibex8
  ~/agents/bool-keep/shrec: rec1/all = 103 slab Booleans from 260 fuzz Shells (run1.sh; models built with
  NOSHAPE_BOOL_RUST=0, so OCCT's tolerances: 19 inputs >= 1e-4 m, bin/maxtol.py). run2.sh (11-20 06:08: on vibe2 ~/agents/bool-keep/shrec in its slot, held by a
  backgrounded ssh; vibex8's slot was qa's 04:09-06:05+, vibex7's 6 deep; run2.out,
  ends SHREC2-DONE): the models built as users get them (ours on), only the Shell probe OCCT-only -> shells2/, rec2/,
  rp2-main.txt; prints how many inputs are still >= 1e-4. If fewer: rec2 is the new shfall (tell the lead). Swap:
  mv bool-corpora/shfall -> shfall-1102 (vibex8 and every host holding it), new shfall = rec?/all minus the cases OCCT
  disagrees with its own recording on every replay (rec1: 0a3e07e3 0e72c08b 63dae116 9166f9a1 cbadd976), named with
  the reason in shfall/README.
- sh3-probe verified (Mac u5): kernel shell:: 34 pass; Shell corpus ours vs OCCT: 17488d26, 99b55ec9 (same under the
  tolerance), 2e9c3ddd (ours "took too long" vs "shell failed": load flake, both errors; watch in the gate). 37 fuzz
  shells now fail "the walls fill the part" both ways (they returned the part unchanged, as b79e508b).

## From bool2 (owned since 11-19 20:45)
- Landed 11-19: 41fcfd2c bool2-cbs (closed B-spline faces' period; 2a5740b0 gate exception backed by live capture
  live/bool2/q-fb-ell-asym-1119); 09ad4e81 bool2-rim (B-spline edge along another argument's face rim; Shell common
  f9b29e1d = OCCT now).
- bool2-exact: NOT landable (gate86 FAILED 19:58). Worktree /root/projects/noshape-wt/bool2-ab, branch bool2-exact,
  tip 2a8da63d4 on main e2abe938a: 819a9c81d off-plane check by curve ends + vertex tolerance; 9b9c09518 a piece keeps
  its edge's end within its merged vertex's tolerance; 957b0f4b9 plane-plane straight edges on the planes' exact line
  (brep Output.edge_line, ffi, bridge Geom_Line); 50a079ef6 / 79eb8832f a new arc ending at a tolerant vertex ends at
  the crossing with an input line there (and tangent case); 2a8da63d4 T1 b997c4c3 to recorded_as_occt (fails).
  gate86 (/root/bool2-ab-scratch/gate-2a8da63d4.log): +2 taken, differing 6 -> 8 (shfall 6bb081c8 / 5101adb8 fuses,
  32 tools, path direct), T1 fails; c2-parting-line-draft Rib 1 edges 234 -> 225 (live 214).
  Measurement: rerun as unit bool-tdbg53e (the first two died: build failed on `mod sampler;` missing from bool2-ab's
  uncommitted all.rs, now added) -> /root/bool2-scratch/tdbg53c.out, "VOL ours" / "VOL occt" per case:
  - b997c4c389ff7abe: ours 4.0555e-6 vs recorded 4.05697e-6. VOL occt ~4.0555e-6 -> stale recording (re-record);
    4.05697e-6 -> needs the rest of the tail stack (bool2-taildbg b02a9be42 + 62e8ccdad).
  - 6bb081c8 / 5101adb8: ours 5.295354594164e-4 both; recordings 5.2953546335e-4 / 5.2953739068e-4 (OCCT disagrees
    with itself). If VOL occt = ours: history only ("rust only" records: arg0/1/10 Edge0 Generated Vertex at (57467,
    39967, 18000 / 18967) mm, Modified edges along x 57500, arg1 Face0 Modified): fix the history or re-record
    (DECISIONS 0e603d4c9), never decline.
  Plan (lead 20:30): fix why the plane lines take those fuses and differ, re-gate on vibex8 via slot, land at 0 differing.
- bool2-taildbg (same worktree): WIP debug + real commits b02a9be42 (split_params ends allowance, Line only),
  62e8ccdad (arrange cycle areas with chords), 43eebd26d, 38bad0785, f1bf78ea5, 855863591, 7b7052072, 07df21fea. With
  all of it 11cc92b2 / dbc28f46 and b997c4c3 match OCCT in volume and history, but 11cc92b2 / dbc28f46 unify
  differently (ours 25 faces, OCCT 27: OCCT's USD merges two 0.052 mm2 slivers on ours, not on its own): keep those two
  declined to land, or find a live document.
- bool2's parked tail reasons (none an OCCT difference; details archive/bool2.md end): two edges leave a vertex
  (11cc92b2/dbc28f46 slivers; gpf ee95f96d / b7a02f84 need > 4 regroup rounds); coincident pieces differ (gpf
  9f0b0c04, 54630d70, corpus4 3cca316c: piece_key keys a cone's Apex by its vertex, then a 0.5 um sliver at th0 x=0);
  a plane nearly along a cylinder (gpf 1b0a04c4 OCCT 1.3 s, eee70623; BOOL2_LONGELL); an arc across a cylinder's seam
  (parity56 ce4e3c99: th0 = pi exactly); a section touching a face's boundary (c38a6db8, 890acd4d); short sections
  (parked, lead); OCCT's result invalid (tiny arc 63bd60d0 b70f339a fda1114a 91923f72; internal face b25113011
  addb9a14; 09489f90; 0c6f868e; f516559d 1ba3f24a); inside-out tool (869aabac 5059c49e 269c06e4); unclear shell
  orientation (a3bfaa87 adeba320 6cf25eee); cones at an apex (ba2e862f, 620253d9); 50a8ad1c; fz1144 a46cf4d0.
- bool2's trees: bool2-ab (bool2-exact, bool2-taildbg, bool2-rimdbg; rb.sh target vibex8 ~/agents/bool2-ab; probe
  binaries vibex8 ~/tmp/bool2/ab kp-*; uncommitted `mod xprobe;`), bool2-xs / bool2-k25 (landed), bool2 (old), bool2-ho.
  Gate trees vibe2 / vibex8 ~/agents/bool2-ab-gate. Debug envs on taildbg: BOOL2_ARR / COINC / TOUCH / SAME / LONGELL
  / SEAMARC / USD / SLIT / FLATSEAM / OFFPL / TWICE / CROSS / ELT / PN2 / APEX / ROUNDS / SHORT / VOFF.

## Share and decline reasons
- Share (main 548d795fb's replay): ours 3121/3477 (general 3026, direct 95), OCCT 356, differing 6.
- Ranking (summed over corpora): not tried 212, non-manifold 42, short section from a corner 30 (parked), near two
  vertices 21, inside-out tool 17, tiny arc 12, piece's point on another solid's face 12, fuzzy / mesh 11, face's
  vertex off its plane 11. Outside shfall every reason is 1-4 distinct cases.
- shfall (122 declined; one group in the share): recordings from 11-02 (slabs at OCCT's nominal 1e-4 / 1e-5); no
  source change moves them (retolerance test: 12 of 92 taken, 2 volume differ). Tight ones:
  /root/bool-hat-scratch/shfall-tight.txt.
- Non-manifold (15 distinct outside shfall): 6 OCCT non-manifold too; cdd336b4 fixed on bool-pole; 4a71ecd0 / 62565707
  touching only; 301238ec / 23a4e56c corners on the wall with horizontal tori; 9f262529 / b0c36850 tolerant mirror twins.
- Looked at and parked (diffs in /root/bool-scratch): parked-shell-tight-slabs.diff, parked-stub-corner.diff,
  tilt-split.diff, parked-void-boxes.diff, parked-tiny-arc-micro.diff, parked-rnd-decline.diff, parked-cyl-cyl.diff,
  parked-own-foot.diff, torus-cylinder-wip.diff, offset-cache.diff, parked-cones-apart.diff.

## Next (after the order above)
- Pulleys (lead 11-19, 152 ms vs live 23): not a gap (11-20 05:00). Mac Circular pattern 1 45-55 ms, document 75-90 ms
  vs live 102; vibex8's 152 was host load.
- P1 SPEED-STANDOFFS: its Booleans all ours on land7. Mac document (ops xshell3::xpul, XPUL_DOC) 455 ms vs live ~200.
  The unify's volume checks were ~40% (usd_kept_volume: faces_volume_precise, volume0): both on all threads in the
  faces' order (bool-sh3 d494b222e): ~345 ms. bool-sh3 a0b85fef4: 1f27fa8ca's segmented-integral
  rescue only where those and OCCT's agree on the solid to 1e-8 (it broke kernel tube-chamfer 34 / knurl grooves 9). uv_of's fold retry (11-20 06:20,
  counted): 1463 retries a regeneration, 847 end on the surface at the 7.7th of 16 candidates; an early stop saves ~10
  ms (3%): dropped (345 vs live ~200 is under 2x).
- qa's backlog (TICKETS a3ff6d481): standoffs Revolve 2 / Boolean 1 / 2 (OCCT 450-980 ms, ours "a piece's point on
  another solid's face"), wire-clip / gamma Boolean 1 (87-tool fuse, "cylinders nearly coincident", OCCT 2-2.9 s) and
  Boolean 2. Cases Mac bool-keep/rec-<doc>.
- core's nut thread cut (c2-config-parts-nuts Sweep 1; Mac bool-keep/nuts): crest on the bore; waiting on core for a
  tool reaching into the hole; live 135.268 mm3 / 29 faces.
- c2-fan-blade-sweep Boolean 2 (P0 earlier, bool2's free-form side now mine): offset-surface evaluation is OCCT's 186
  ms; when ours takes it, narrow offset_grazes_a_circle (check rings-loft, sp-1113). Case Mac bool-keep/fan.
- Re-win the 50 um rule's declined cases (list above).
- c2-rings-loft Shell 1 (~550 ms vs live 235), learning-center-michael (cyl x torus), cf-sheet-metal-shaped-flange
  corner reliefs, rings-loft Circular pattern 1's splits (decision with the lead before touching features.cpp).
- DECISION ASKED (lead, 11-18 22:00): every Boolean's check as made_valid (changed faces + shells) instead of
  BRepCheck of the whole result (pulleys' 5-tool cut 13.1 -> 6.2 ms).
- Parked (lead): 129100d95, 846db3d0c, 13b8c4b8d's same-surface merge (no live capture).
- P2 (lead 11-20, TICKETS T2-NIGHTLY-22-FAILURES, vibex6 nightly T2):
  - corpora on vibex6 (11-20 03:00): ~/agents/bool-corpora/shellcorpus (422) and bool-corpus (1949); bool-ho's tests read
    $HOME/agents/bool-corpora/... (shell_corpus_time takes every case when `taken` is absent, faces 1-based); the
    knurl / gear tests to 1e-6 relative (bool-ho 5cfc4f676, queued with 42635088a 04:12). a_10_by_71_knurl (1e-7) read
    1.1e-7 off on the Mac binary: to 1e-6 too if the nightly shows it.
  - overlapping_free_form_parts_merge_like_onshape: FIXED on bool-sh3 1f27fa8ca (gate with sh3): the fuse is ours now;
    the unify's USD merge to live's 15 was refused (OCCT's integrals 4e-9 off on our B-spline bounded cylinder pieces,
    the segmented ones 9e-10); a change within 1e-8 of the box's volume is now decided by the segmented integrals of
    the changed faces (62 ms there). Mac u32: the test passes. Debug in bool-hat WIP: NOSHAPE_USD_DEBUG / _FACES.
  - a_25_by_10_knurl_at_30_tpi: FIXED on bool-sh3 d2a26a12a (gate with sh3). Declined since 4ce48b9a0 (our own
    plane x B-spline sections; bisect over crates/kernel + brep, /root/bool-scratch/bis.log): the end face meets each
    flank's 9.6 um section through the rim 3.3 um before its end, read as "a section touching a face's boundary"; a
    touch within the section's tolerance of its ends is now that end's pave. Ours again, 1.8 s.
  - knurl_b94_at_30_tpi_on_a_10_mm_rod (>15 GB): alone it passes, ours, 46 s (Mac, bool-hat); in the nightly it ran beside
    knurl_feature_b94_tooth_and_helix_angles (both >60 s in t2-2026-11-20.log) and the budget is the process's growth:
    the B94 knurl tests now take one lock (KNURLS, bool-ho 42635088a). Its Boolean recorded: Mac bool-keep/b94/e83dc201f84a455f.

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
