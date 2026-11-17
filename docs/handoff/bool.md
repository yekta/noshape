# bool: current state

Owner: bool. noshape's own Boolean (crates/brep, kernel cpp rust_boolean.cpp / boolean.cpp), the Shell builder
(by_quadrics, shell.cpp), the speed of Booleans, Shell and Split. History: docs/handoff/archive/bool.md. Standing
rulings: docs/DECISIONS.md (wins over anything here). bool2's notes: docs/handoff/archive/bool2.md.

Updated 11-17 22:00 UTC. Hosts (lead 11-17): builds, gates, A/Bs on vibex8 (NOSHAPE_RB_HOST=vibex8); Mac for crate
tests and `sample` profiles (NOSHAPE_RB_HOST=mac); land `NOSHAPE_LAND_QUEUE_HOST=vibe2 tools/tiers/land.sh` (lead
21:23: queue moved to vibe2); private docs on vibex7 ~/lead-private (hat8, hat15, hat15b, AppleWatchDock); bool2 gates
on vibex6. Never kill a process tree from a computed pid (lead 21:06): `systemctl stop <unit>`; kill only own pids > 1.

## In flight
- Gate unit bool-gate-x8b -> /root/bool-hat-scratch/chain-26ce2dffa.out: built 18:55 on vibex8, waiting for the
  host's slot (asm's parity_assemblies holds it since 19:27, 4 h limit). Then unit bool-udoc8c runs
  /root/bool-hat-scratch/udoc-ab8.sh 26ce2dffa (4 private docs on vibex7) -> udoc-26ce2dffa.out. Then knurls
  ours-only, land, tell dx (now 2108c2b7-2f48-43a7-87a0-3f652e3cda6d) and core.
  Branch bool-shared2 (bool-hat) on main f6274d94e: the gated seven, then two more that need the next gate:
  1. e61278b1d usd: op_unify keeps USD's merge whose volume change fits in the merged faces' tolerant edges (sum of
     tol^2 x length, capped at 1e-8 of the volume, only where every face USD made of two or more is on an elementary
     surface). infento 1dc32b Boolean 4 Part 4 232/632/400 -> 216/600/384 as live. Grooves cd1c58431ab4a62c 9 -> 8.
  2. 8a9351e4b shared: solids' Booleans no longer decline "a shared shape OCCT may keep or remake" (Modified where
     OCCT, by rounding, says Kept). Lead 11-17 conditions: (a) model suite identical end to end, (b) hats/dock SAME,
     (c) replay differs only in those labels, listed by case ("shared shapes relabelled"; "seam placement only"),
     (d) T1 split_names::a_shared_edge_of_split_pieces_keeps_its_name_through_a_boolean. Sheets keep the decline.
  3. a1d7ca0b4 + 65befe285: NURBS piece boxes (Ctx::nurbs_box) for vertex search / intervals_on / split test.
  4. 51ee7c1cb unify room summed most tolerant edge first. infento Boolean 4 with 1-4: 751 -> 189 ms (live 135).
  5. 0b17fee66 par_map_by: a worker stopped by its own stop state declines "cancelled" (dx's boolean.rs:5058 abort).
  6. 26ce2dffa fuse_glued: a mirrored plane's normal is against its axis.
  7. b11d72be6 gap(): arcs meeting end to start, rounded a little past (4e-15 rad), leave no gap (it read a 186 deg
     gap on c2-config-parts-pulleys' rim; its 23-groove Circular pattern 1 cut declined "an arc across a cylinder's
     seam" to OCCT, 254 ms). T1 tests::arcs_meeting_end_to_start_leave_no_gap_between_them; case bf1b3ffd3777f210 in
     tests/data/brep.
  8. 29bcd92a8 pattern copies check their source curves' clearance once per (face, canonical pair) (Ctx::from_clear);
     seam_cuts samples only spans whose hull may reach the seam (a 131-pole section: 4096 samples x 46). Pulleys' 23-tool
     cut on the Mac 44 -> 28 ms (OCCT 32, live 23 for the feature).
- DECISION ASKED (lead, 22:00): every Boolean's check as made_valid (made / changed faces + shells) instead of
  BRepCheck of the whole result: pulleys' 5-tool cut 13.1 -> 6.2 ms (OCCT 8.2), 4-tool 7.1 -> 3.7. Untouched input
  faces then aren't re-checked. Experiment: DBG_MADE switch at rust_boolean.cpp `const bool valid` (reverted).
- Validity in the gate: tools/bool/gate-host.py (landed 3b8bfba0) reads blend's NOSHAPE_PARITY_VALIDITY CHECK lines
  (cfd943d5) and fails on a part valid on main and invalid on the branch. My own invalid_solids was withdrawn (lead).
- asm pattern-std 36b90f Extrude 12 (17/43/28 vs live 14/36/24): not the Boolean (same with NOSHAPE_BOOL_RUST=0):
  the up-to-surface extrude's caps; told asm 11-17. Logs /root/bool-scratch/mps-51ee{,-off}.log.
- Pulleys recordings: vibex8 ~/tmp/bool/pul (6 cases), Mac ~/noshape-host/bool-keep/pul; Mac scripts in bool-hat
  .samp.sh / .t6.sh / .t7.sh (untracked).
- GearLab herringbone (dx, asm-isaac-ball 8f5622, 14.8 s vs live 1.15): fuses 726897a5 (teeth on blank, ours "two
  edges leave a vertex in one direction"; glue changes history: no) and 68f32ae1 (half + mirror, ours "a piece's point
  on another solid's face"; OCCT 949 ms, glue shift 294 identical history). fuse_glued now sees the mirror (6.) but
  the half's z=5 cap is 41 faces (core's 5e-9 rad tilted sweep caps leave teeth end faces unmerged): with core's fix
  it should go glued. ShiftChecked's volumes_add_up (1e-7) rejects B-spline solids (integration noise ~1e-5).
  Cases Mac ~/noshape-host/agents/bool-hat/gear, vibex6 ~/agents/dx-keep/gearlab/booleans.
- blend: c2-impeller-loft Fillet 3's OCCT cut 9602f671f8528c1a (~650-800 ms, PerformEF 481): vibex7
  ~/agents/blend-keep/rec-f3, Mac ~/noshape-host/agents/blend10/rec-f3.
- SCISSOR-LIFT-BOM-ROWS: culprit core's 41f62884a; core's revert ddfea0eb restores 59/87. Done on my side.
- impeller 02f2f58d (bool2's): Linux "unified differently" only; on the Mac history differs with or without my change
  (platform). 7cb79697: seam placement only.
- vibe3's ~/agents/bool-cases copy went to vibe5 (gone): lost.
- Not mine (11-17): impeller 38073a30 (bool2: live wins, OCCT's the outlier), impeller Extrude impellerraum (core),
  c2-fan-blade-sweep Boolean 2 (qa: core's Subtract with Offset range; main 1d77cd633 back to 56/139/84).
- Landed 11-17: bool-knife (ecf8c33a), OCCT 0033 (9ece8f66), bool-knife2 (cf5a9aff), bool-march (d3f24a83), nuts
  (43335ce3: thread turn helix gap; gate clean, hats/dock/hat15b SAME), MSVC fix (01c5c13e: fitted's p static
  constexpr; CI 44039545068 green on all three OSes).
- bool-march's 4 replay cases "equivalent" (74acee2ff54886eb, e001fa8eb5dbfe24): ours the exact one (cap area x height =
  our volume exactly; OCCT's caps disagree): accepted per DECISIONS 11-15; told the lead.
- Mac 11-17 (loaded, hints): every qa-list document of mine under live (louver 1195/4987, rings 1387/1557, snap-hook
  206/274, housing 46/105, lighten 61/114, bottle 29/40, shaped-flange 455/1026); over live only features: rings Extrude 5
  71/39 (boolean.rust.brep 33: mapped-curve chords), knife Fillet 2 74-81/9 (op_split 18 OCCT EF on the grind B-spline;
  sections ahead 16), michael Fillet 2, tpd Fillet 1 (blend's). Parked: /root/bool-scratch/parked-offedge-boxes.diff
  (off_shared_edges box pre-check, identical, no gain).
- GATE GAP: the knurl corpora need a 48 GB host (none); ours-only replay instead (lead 11-17 ok):
  NOSHAPE_BOOL_CORPUS=~/agents/bool-corpora/<k> NOSHAPE_BREP_OURS_ONLY=1 <kernel tests> brep_replay::brep_corpus_replay.
- Knife cases: vibe5 ~/tmp/bool/knrec, Mac ~/noshape-host/agents/bool-hat/knrec (17b8 c1d9 Fillet 2, a3f9 Chamfer 1, 682a
  Mirror 1); knife Fillet 2's split dump Mac ~/noshape-host/agents/bool-hat/kdump2/split-11.

## Parked (branches / diffs, not landing as they are)
- Plane x B-spline marcher: un-parked 11-17, now bool-march (In flight above); its knife regression found and fixed.
- Circle-square-circle loft sides (creased at the square's corners) stall that fit in the face's parameters: why 10.
- Torus x cylinder sections: /root/bool-scratch/torus-cylinder-wip.diff (per u the tube circle against the cylinder,
  math_TrigonometricFunctionRoots, branches by Newton). michael: 5 more pairs, but two all but flat curves end
  2.5 um from OCCT's ends and the cut turns non-manifold (lead: a non-manifold cut is a no). 8 such pairs finding
  nothing still cost OCCT 75 ms.
- Offset-surface approximation cache (/root/bool-scratch/offset-cache.diff): 1-2 % CPU on cf-snap-hook's cuts: dropped.
- blend's pole-hull disjointness test: withdrawn by blend (the knife's costly empty pairs touch; tangent contacts).
- blend's known sections entry point: parked (few pairs are tangent contacts).
- Rings-loft Shell 1: skipping OCCT's global foot search where Newton from a 25 x 25 grid's local minima finds no
  square foot (/root/bool-scratch/parked-own-foot.diff): identical, only ~3 %; evidence (160/160), not a proof.
- (Handed to bool2 11-17.) Parallel-axis cone / cylinder disjointness (/root/bool-scratch/parked-cones-apart.diff: circles at each height,
  angular ranges as cosine bounds, quadratics in z): cf-snap-hook fuse 96de626280206723's 30 cone x cone pairs
  (OCCT ~30 ms each, none found) share one height where the circles are internally tangent (4e-7): only 1 proven.
  Tangent contacts stay OCCT's (lead 11-15); likewise housing's 4 cylinder x revolution pairs (20-27 ms each, none).

## Next, in order
1. qa's list (docs/handoff/qa.md "Speed"), mine: knife Split 4 (no exact lever: OCCT's plane x B-spline sections and a
   1 mm line to BRepProj), bottle Shell (566b1015) / Split (no single lever), cf-lighten and c2-sm-louver at or under
   live, CP2 and housing Rotieren 1 (bool-cp2-ahead above). Housing's other cuts: 6e0cb4e02a2efe47 (~7 ms quiet)
   is half OCCT's cylinder x cylinder walk (IntCyCy + WLApprox + PSO tolerance). Own cyl x cyl sections parked:
   /root/bool-scratch/parked-cyl-cyl.diff (quadric_across over a Ruled view) matches OCCT's curves but not its
   splits: OCCT breaks its lines at 12 critical points (CriticalPointsComputing) and re-joins some (JoinWLines:
   IsSeamOrBound, CheckArgumentsToJoin); probe cases in notes-snap.txt. Matching both exactly is the work left.
   CP2 remainder: its 8 tool cuts run in parallel (rust_fuse_trimmed), then the fuse; nothing single.
2. c2-rings-loft Shell 1 (~510-590 ms, live 235): repair 251 (traces 95, split 78, crossings 26, joins 17),
   self-interference 67 (its EF time in ~23 pairs on free-form faces; quadric pairs are cheap), edges 44, check 35,
   thin 34. Points with no square foot: OCCT's global search found none wherever a Newton search from the own 25 x 25
   grid's local minima found none (160 of 160), so the global search may be skipped there (needs the gate).
   Rings-loft profiles 11-16 15:2x (vibe5 ~/agents/bool-keep/rlb, xprobe probe_case_sample): cut 275ec41827174a35
   (51 slot tools, 488 ms) 43 % IntTools_FaceFace ImpPrm walking (plane / cylinder x rational B-spline), the main thread
   waiting on it (wait_until_cold 34 %): the plane x B-spline marcher (item 3) is the lever. Fuse 4fa72ce21506c5f7
   (8 tools, 210 ms): BRepCheck 20 % (cuts' results 8.5, the fuse's 10; the fuse could trust the cut pieces' checked
   faces as made_valid does, but `a`'s are unchecked), ::at 19 % (chords of free curves 50 % of it, piece_box 20 %).
3. The off-edge pre-check (In flight), then the knife Fillet 2 op_split and min_distance.
4. learning-center-michael (363 vs 96): cylinder x torus (22 pairs ~220 ms CPU), cylinder x sphere 24 pairs ~60 ms,
   cylinder x cylinder 10 pairs.
5. skasm's cf-sheet-metal-shaped-flange: vibe6 ~/agents/skasm-keep/sf-bool-cases (40 cases): ~15 corner-relief cuts of a
   6-face bend solid, 7-24 ms each; the part's final fuses 27-83 ms.
6. c2-rings-loft Circular pattern 1: 51 prism_until splits 254 ms (features.cpp, OCCT's Splitter; history and naming
   there: decision needed with the lead before touching).
7. core's FYI (qa's speed list 11-15, vibe7 loaded): c2-housing-draft Rotieren 1 77 ms (live 13; boolean.rust.brep 70,
   sections_ahead 41); c2-rings-loft Extrude 5 295 ms (live 39; boolean.rust 128-216, ahead 25-60); Shell 1 dump
   vibe7 ~/tmp/core/rh/dd/shell-9 (crash_inputs::replay_one).

## Measured (ours vs live, ms; vibe5 under load, candidates per DECISIONS 11-15)
c2-rings-loft 1880 / 1557; c2-sm-louver 4008 / 4987; c2-housing-draft 126 / 105; cf-lighten 158 / 114;
c2-tpd-fillet-variable 149 / 44 (blend's stack on main: 47 / 44 with the parked marcher); learning-center-michael 363 / 96;
cf-snap-hook 866 / 274 (suite run 18:5x); c2-bottle-split Split 1 ~12 ms in release.

## Open questions / decisions
- None open.

## How to run
- Worktrees: bool-split (builds via rb.sh on vibe5, target ~/agents/bool-split; kernel test binary
  target/release/deps/all-71797ea96afb1f39, ops test binary all-a79b487bf0dd0b06), bool-tube (docs / handoff),
  bool-hat (Mac builds: NOSHAPE_RB_HOST=mac). Probes parked outside the tree while branches rebase: copy
  /root/bool-scratch/probes/{xprobe,sampler}.rs into crates/kernel/tests and `git apply
  /root/bool-scratch/probes/all-rs.patch`; undo before a commit. Mac binaries: ~/noshape-host/agents/bool-hat/oc33.
- Gate: `systemd-run --unit=bool-gate-<sha> --collect /root/bool-scratch/gate-chain.sh <sha>` (gate-chain-own.sh: the
  branch's own gate.sh / gate-host.py, for changes to them; the branch's gate only;
  NOSHAPE_GATE_HOST to change host, default vibex8). Output /root/bool-hat-scratch/chain-<sha>.out (VERDICT,
  "CHAIN DONE rc N") and gate-<sha>.log; read with grep. gate.sh runs its replays through the host's ~/bin/slot
  (bool2's ff43a8c95). A crates/brep or Boolean change needs a clean gate and identical results end to end.
- Land: `NOSHAPE_LAND_QUEUE_HOST=vibe2 tools/tiers/land.sh` after
  `git diff --stat $(git merge-base HEAD origin/main) HEAD`; `tools/tiers/land.sh --status`.
- g++-13: /root/bool-scratch/gcc13-split.sh / gcc13-hat.sh <file.cpp> (name only, e.g. rust_boolean.cpp).
- A/B binaries: /root/bool-scratch/kb5.sh <worktree> <name> (kernel release test binary to vibe5 ~/tmp/bool/<name>).
- Private documents (hats, dock): vibe5 ~/agents/bool-keep/private (mode 700); compare main vs branch with
  /root/bool-hat-scratch/udoc-ab.sh <commit> (builds in bool-split, returns to its branch; copies in ~/tmp/bool/udoc, deleted after).
- Probes (xprobe.rs): probe_case_timed (PROBE_CORPUS, PROBE_CASE, PROBE_RUNS; NOSHAPE_SAMPLE_OUT samples all runs),
  probe_case_sample (PROBE_ONLY rust/occt, PROBE_SAMPLE), probe_case_unify, probe_shell_dump (PROBE_DUMP=vibe7
  ~/agents/bool-keep/rl/shell-9; NOSHAPE_SHELL_DEBUG for stage times), probe_split_dump, probe_project_dump.
- Profiles: vibe5 ~/agents/bool-keep/pu.py <log> [stack filter] [n] (inclusive and self; SAMPLE_WALL=1 logs need a
  filter such as probe_case_timed), ~/agents/profile-funcs.py; vibe7 ~/tmp/bool/c2
  pu.py, callers.py (caller chains), lines.py / lines2.py (shell.cpp stage buckets).
- Recordings: NOSHAPE_RECORD_BOOLEANS=<dir> on a parity run (ops binary, NOSHAPE_PARITY_MODELS=vibe5
  ~/tmp/bool/ab/models, NOSHAPE_PARITY_ONLY=<doc>). Corpora: vibe5 ~/agents/bool-corpora, ~/agents/bool-keep,
  ~/agents/blend-keep/known-sections (tpd 6d3bca8654a6d02d, michael 1447eaf22e0cd418), vibe7 ~/tmp/bool/c2.
- Debug switches: NOSHAPE_FREE_SECTIONS=0 / NOSHAPE_FREE_DEBUG (marcher branch), NOSHAPE_OWN_SECTIONS=0,
  NOSHAPE_PROJECT_OCCT, NOSHAPE_SPLIT_DEBUG, NOSHAPE_SHELL_DEBUG; per-pair section timer for occt_ahead:
  /root/bool-scratch/torus-and-ahead-debug.diff (NOSHAPE_AHEAD_DEBUG).
- Notes for the snap-hook work: /root/bool-scratch/notes-snap.txt.
