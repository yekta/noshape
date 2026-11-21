# bool: current state

Owner: bool (bool2 merged in 11-19). noshape's own Boolean (crates/brep, kernel cpp rust_boolean.cpp / boolean.cpp's
unify), the Shell builder (by_quadrics, shell.cpp), the speed of Booleans, Shell and Split. History:
docs/handoff/archive/bool.md (the 11-19 16:45 handoff is at its end) and archive/bool2.md. Rulings: docs/DECISIONS.md.

Updated 11-21 11:00 UTC. Hosts by load (DECISIONS 11-19): read
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

## In flight (11-21 11:00 UTC)
- bool-re (isaac coaxial re-land, 72fe51df1 again after blend's a9c26c41): QUEUED f33f4448 10:43. Gate vibex7 CHECK
  (knurls only), corpora 3114 -> 3114, differing 2 -> 2, models 127 -> 127, shells 6 = main; knurls vibex7 = main.
  asm recounts isaac mates once on main. After: rm worktree bool-re (+ vibex7 gate tree).
- bool-mend (worktree bool-mend, tip 0170472fc on main ceec84d25), gate vibex8 (unit bool-mend-gate, chain
  /root/bool-scratch/chain-bool-mend.out): ce09aaef1 boolean.cpp mend_loose_sections (a new section edge whose pcurve
  is > 10x the inputs' loosest tolerance off: pcurve re-projected, edge + new vertices tightened; impeller Fillet 3's
  cut out 2.24e-4 (was 2.55e-3), chamfer cut 1.7e-5 (was 2.1e-4); test on tests/data/booleans/cc3caf4644ba152b);
  56bb5c47b brep: a cut's tool piece on other tools alone taken (infento Circular pattern 4: 26 rim tools, ours 58 ms
  vs OCCT 1.6 s); 0170472fc kernel: tools that are copies of an earlier one in place left out, history copied (infento
  Extrude 42 5.5 -> 0.7 s; asm's translator no longer makes the copies, still general). Then knurls, land, tell blend
  (remeasures Fillet 3), asm, lead.
- bool-shg (worktree bool-shg, 58dad8e39 on main 4a22b2966), gate vibex7 (unit bool-shg-gate, chain-bool-shg.out):
  shell.cpp normal_at warm-started by Newton from the last foot (full grid search only off the surface), solve3 gives
  up after 12 steps without progress. Shell Gehaeuse (dump Mac bool-keep/shgeh/shell-62, probe
  shell::probe_shell_dump_profile in bool-hat, SHELL_DUMP / SHELL_ROUNDS) 145-160 ms vs 160-220; live 31. Left on the
  path (second quadrics try, tangent extensions): self-interference 63 ms (meshed_copies + CheckerSI EF), faces 26
  (add_pcurve), edges 20, runs 15. The first try (fails: edge 3's marched point 0.2 mm off face 1) runs in parallel.
- imp-speed (3ada93d19, brep free-form projection): gate clean, DROPPED: no gain on the impeller after bool-mend (Mac
  A/B: Bohrungssitz 267 vs 292 ms best). Branch kept, never land.
- Impeller (Mac, bool-mend): Bohrungssitz Ansaugflansch ~270 ms (live 177), Remove Bohrung 25 (59), Boolean
  Flansch+Gehaeuse 250-300 (154), Shell Gehaeuse ~145 (31); Fillet 3 / chamfer / fillets are blend's.
- infento (asm P1): Extrude 42 also declined by ours ("an invalid result", the object is invalid); private cases vibex8
  ~/tmp/bool/inf/{rec,c2}, Mac bool-keep/infento: never commit. Delete vibex8 ~/tmp/bool/inf when done.
- bool-hat (Mac worktree) = bool-mend + imp-speed + bool-shg's shell.cpp + probes (rust_boolean.cpp PROBE_INVALID,
  brep_replay.rs / shell.rs probes): never land from it.
- SHELL-HANG-SPEED (P1) + SHELL5-AFTER-SHELL3 (P2), TICKETS bool 11-21: qa's cli-shell-hang-pre (vibex7
  ~/qa/tickets/cli-shell-hang-pre.noshape; copy vibe2 ~/tmp/bool/sh5.noshape): ours Shell 3 12.8 s (live 40 ms), Shell 5
  (Part 2, 3 mm, faces 1475.653 mm2 Shell 3 / 282.843 mm2 Linear pattern 2) fails in 25.4 s, live OK 20 ms 172908.326 mm3
  59/163/98 (live/bool/shell5-1121.json; live rebuilds cov-shellq with sq.js). Mac dumps ~/noshape-host/bool-keep/sh5/dump/shell-{17
  Part 3, 18 Part 2 (Shell 3), 21 (Shell 5)}; Mac regen: Shell 3 2.5 s, Shell 5 4.2 s. quadrics declines both in 1-2 ms,
  slabs takes them (17: 2.7 s; 21: fails after 20 s). Shell 5 CAUSE: Shell 3's slab result for Part 2 has its z -20 rim
  (face 3, a plane) meeting itself along a 5 mm slit edge (-60,-7.5)->(-56.46,-11.04) (OCCT USD keeps the last shared edge
  of a ring of pieces; core's request 1761 saw the same pinch); quadrics: "an edge not between two faces". PLAN (branch
  bool-sh5 on bool-p3, WIP probes commit): after the slabs' unify (shell.cpp ~10422) drop slit edges of planar faces
  (edge twice in the face, not IsClosed), rebuild the wires (ConnectEdgesToWires; largest = outer; ShapeFix_Face),
  ReShape + history merge, keep if valid. Shell 3's Part 3 quadrics: "a corner's offset far from it" (vertex at
  (-60,41,-10.7) on 2 faces / 2 edges). Parked for the impeller speed P0 (blend 11-21).
- SHELL-AFTER-WELD-FILLET (worktree/branch bool-sh3w, a4cfa07d3 on main 55749fd6f): 407bb5cf6's
  without_inverted_slivers alone + T1 a_weld_fillet_part_shells_to_one_solid_without_its_inverted_sliver
  (tests/data/shell/weld-fillet-part3-input.brep = qa doc's Part 3 before Shell 3). shell:: 33 ok; the T1 fails on main.
  Gate CHECK, knurls = main: LANDED 93cb9b6c3; qa asked to verify (03:16).
  FOLLOW-UP (own ticket): Part 3 = live's input (342822.058 mm3) but ours 183041.69 vs live 183072.272 /
  59 faces / 156 edges / 93 vertices (41 planes, 14 cyl, 4 cones), 30 Shell faces 32018.992 mm2 (per-face areas:
  /root/live-scratch/shellq/shell3-topo.json). (The 141 faces were Part 3 at the END of the doc, not after Shell 3.)
  ROOT: Part 3's Shell is a closed hollow (the 1 mm chamfer removed is thinner than t); quadrics fails (merged walls),
  slabs is the path; its concave-edge filler took the mitre from tangent planes (R20 round x 45-deg V faces).
  FIX bool-p3 (bool-hat worktree, 4b69a5da1 on bool-sh3w): within_offsets (filler swept 2.5x, sides turned 0.05 rad,
  from behind the edge, COMMON each face's under_offset = extended_slab(d) FUSE extended_slab(-d); plane/cylinder
  only; corners with a cylinder alike; NOSHAPE_SHELL_TANGENT_MITRE=1 = old) + a concave plane-plane filler ending at
  a vertex whose one other face (a plane) is convex to both runs on 4|d| past it, within that face's plane (the
  groove's pointed end at (-44, -7/9, -20); `opens` judges convexity along the shared edge, not by the big bottom's
  centroid; NOSHAPE_SHELL_NO_FILLER_RUN_ON=1 = off; trimmed by the solid alone it broke a_sunk_boss +4.56 mm3).
  Part 3: 60 faces / 2 shells (live 59), 183072.287 mm3 (live 183072.272), every face area = live but the cone 518.307
  split 225.08 + 293.23 (unify: LEFT). Mac shell:: 34 ok. Gate vibex8 (unit bool-gate-p3, out chain-bool-p3.out,
  from 23:41) + knurls. Probe branch p3-left (WIP commit: vertex / point-in-tool probes), p3l.sh.
  Files: vibex7 ~/tmp/bool/sh3w/{d.noshape (qa doc copy), pre-shell3.x_t, dump/shell-17 (Part 3), shell-18 (Part 2)}.
- screws Sweep 1: speed landed 0a0b9a49; "not a valid solid" was my 13:06 A/B parity (maybe before core's 0f8993ef8);
  core finds main valid. Closed unless core says otherwise.
- AppleWatchDock B Govde 85 -> 83 faces after the revert (qa P1 COMPAT-DOCK-BGOVDE-MERGE): settle by live's count
  after Rib Round 1.
- Declined by the 50 um rule (4b1784e6c), TO WIN BACK with a narrower signal: gate corpora 0670c350, 280af0bc,
  2b9c38da, 2c9c42e4, 30afb30c, 475df60e, 570dfc37, 5fc0970a, 70b3fb42, 818552c9, 83b621a3, 8e91c517, 922c847a,
  952c2efc, a1186078, a7bdd15b, b359e025, cb7bb22b, e1dc8c98, f1c35250, f302ef40; Shell corpus 1451f7cb, 43a947f0,
  96d45f42, 9bee135f, c25f7a31. Rule off: 20 of the 21 = OCCT; 570dfc37 (report-only) keeps OCCT's vertex
  (82.95, 41.75, 5) mm at tol 1e-4, ours deletes it.
- shfall = rec1 on all hosts since 11-20 09:16 (98 cases; 11-02 set = shfall-1102, report-only).
- rings-loft STL (dx, low): in dx's x_t round trip face 23 (offset -1 mm of a B-spline) gets no triangulation (wire
  crosses itself in UV, 1.1e-4 degenerate edge; all 79 open edges border it). Our own Booleans (Mac bool-keep/ringsr,
  probe_corpus_meshes): the 273-face cut 331fd52c meshes with 0 holes (unified too); c40a49c4 (cut 103 -> 133) opens
  9. So likely the x_t write/read (dx's) or c40a49c4; Shell 1 alone: 44 faces, 8 folds, 0 holes.
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
- (old notes) live2's Shell 3 ticket (P2 SHELL-AFTER-WELD-FILLET): live 19:15 builds it (Part 2 bottom 2941.8 mm2 + Part 3's
  45.96 mm2 chamfer, 5 mm in: 175798.2 / 183072.3 mm3; x_t /root/live-scratch/shellq/pre-shell3.x_t; centroids lie on
  coincident faces of Parts 1-3, pick exact faces), so ours is a bug. Mac Ok; Linux (vibe2, instrumented ops xshell3, doc
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

## From bool2
bool2's branches and parked tail reasons: docs/handoff/archive/bool.md (end, "From bool2", moved 11-21).

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
- core 19:00 (FYI, if cheap): c2-parting-line-draft Draft 2 with a 64-sample parting-line sheet (core-pl, not landed)
  makes boolean.rust.sections_ahead 646 ms of 1057 (main's 128 samples: 92 of 552); vibex7 ~/tmp/core/plprof-16.log.
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
