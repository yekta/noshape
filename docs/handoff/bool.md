# bool: current state

Owner: bool. noshape's own Boolean (crates/brep, kernel cpp rust_boolean.cpp / boolean.cpp), the Shell builder
(by_quadrics, shell.cpp), the speed of Booleans, Shell and Split. History: docs/handoff/archive/bool.md. Standing
rulings: docs/DECISIONS.md (wins over anything here). bool2's notes: docs/handoff/archive/bool2.md.

Updated 11-17 04:37 UTC.

## In flight
- Landed 11-17: bool-knife (ecf8c33a: hulls, Shell check, uv_of k-d tree, patch cache, validity-once unify, face
  tolerance margin + DECISIONS note), OCCT 0033 (9ece8f66).
- GATE GAP: the knurl corpora (kall, k10-25, k10-25-12, k25-15, k25-25, k25-71-12) need a 48 GB host for OCCT's cut;
  none left (29-31 GB), gate-host.py skips them and says so. Instead (lead 11-17 ok): ours-only replay on main and
  branch: NOSHAPE_BOOL_CORPUS=~/agents/bool-corpora/<k> NOSHAPE_BREP_OURS_ONLY=1 <kernel tests>
  brep_replay::brep_corpus_replay (compares with the recorded OCCT result).
- Landed 11-17 04:25: bool-knife2 as cf5a9aff (grid cache + Shell wall checked once; gate clean, Shell A/B 211/211,
  hats/dock SAME).
- bool-march (bool-hat, bb7c5c4c9 = knife2 + the parked plane x B-spline marcher ca8c628f6 + fix): the knife Mirror 1
  regression was the marcher's 1e-7 fit: Fillet 2's by-tools path refaces a face (build.cpp reface_on, forced
  BRepLib::SameParameter) and re-fitted those edges' curves on the faces (13 poles vs the 3D curve's 14); the faces'
  integrals then changed under a mirror and unify's volume check (usd_volume_change) refused Mirror 1's merge (31
  faces). Fit at 1e-8: knife 46/52 as main, Mirror 1 22/60/39, Extrude 2 29 -> 3-4 ms, Revolve 1 59 -> 21-26, knife
  699-752 ms Mac (main ~1000, live 622). slots_across_a_rings_rounds_cut_as_occt widened 1e-8 -> 2e-8 (ours 474.543498
  converged at fits 1e-7..1e-10; OCCT's sections 474.5435035; lead ok if gate + hats/dock identical). Gate clean
  (/root/bool-hat-scratch/chain-bb7c5c4c9.out: all corpora + parity56 0 differences, only the sprockets / rocker
  flake). Hats/dock: unit bool-udoc-march -> /root/bool-hat-scratch/udoc-march.out (in the slot). When SAME: land
  (bool-split: checkout bool-march, rebase, diff --stat, land.sh). g++-13 clean.
- c2-config-parts-nuts Sweep 1: FIXED on bool-nuts b6466ab04 (bool-hat, on bool-march): one of core's thread turn
  faces' helices fall 2.6e-8 rad short of a turn; that rounding gap became its th0 (a gap > 1e-9), both rulings on one
  side (right empty), its outline swallowed the bore's strip between turns (bore pieces 24 / 14 OnSame) -> non-manifold.
  Now a helix face's gap within the tolerance goes round between its rulings (falls back to the gap if the rulings
  don't qualify). Ours valid, 42 faces, 135.267625 mm3 (OCCT invalid 156.81). Gate: unit bool-gate-nuts (waits for
  core's) -> /root/bool-hat-scratch/chain-b6466ab04.out; then hats/dock, land. Debug diffs:
  /root/bool-scratch/nuts-fix-with-debug.diff (ZZ_FACE_PIECES, ZZ_BOUND=<faces> print pieces' states / outlines).
  Core told (its exact-cylinder sweep patch can unpark; it compares the volume with live).
- core-sing 4254e715e gate for core (op_defeature per feature, 300K-step bound): unit bool-gate-coresing ->
  /root/bool-hat-scratch/chain-4254e715e.out (started 04:28). Core checks the WORK_TRACE "(spent)" lines in its p6.
- qa's knife Mirror 1 130 -> 223 ms (0.4.53 -> 0.4.55): the marcher's 1e-7 fit (Mirror 1 kept its halves, 31 faces),
  fixed in bool-march. The mirror bisect (same recorded fuse, cold): 145ad9e67 21.4 ms, 35baa32fa 21.4: no Boolean code
  change; the remaining +12 ms on green is its different input (below).
- qa's Mirror 1 +12 ms vs v0.4.53 (vibe7 03:50, 3 runs each 129-131 vs 139): real, not the Boolean's code: unify
  99.8 -> 82.3, the fuse 28.8 -> 54.8 because its input differs (v0.4.53's blade 9881.011 mm3, green's 9881.086, both
  in live's band; upstream Fillet 2 / Chamfer 1); the same input times the same on both (cold 21.4 ms). On one binary
  green's input fuses in 26-28 ms, v0.4.53's in 13-19. Told the lead.
- Fillet 2 (11-17, Mac 100 ms vs live 9): op_boolean 30 (cut + fuse), op_split 28 (dump split-11: the 5-face
  run-on tool split by 2 planes + the grind B-spline, OCCT's splitter 25 ms), min_distance x31 11, op_fillet 10,
  props_surface x11 6. A cross-Boolean exact-box cache can't hit: the cut trims the grind face (a new face, a
  new box). No single lever left in mine.
- Revolve 1 (54f29c1fecb98afe) after the grid cache: 60 % OCCT's sections ahead (ComputeTolReached3d PSO 24,
  CheckSingularPoints' Extrema grid on the whole blade 14, WLApprox 10, ImpPrm 8), free::apart 8.5. Extrude 8 =
  5d62643e95aabaa7, Extrude 9 = b58272a7dbba8cc5 (all cut the blade's one B-spline face, x 6-122 mm).
- learning-center-michael (11-17 Mac): 88 ms vs live 96, under live (the 363 was loaded vibe5). Only Fillet 2 over
  (82 vs 46): its by_tools cuts 1447eaf22e0cd418 (12 tools, 34 ms in place) + 694e9aab23f65697 (9.8), seams 12.6;
  the cut half OCCT's plane / cylinder x torus walking (ImpPrm). Cases: vibe5 ~/agents/bool-keep/lcmrec.
- cf-spur-gear Boolean 1 (11-17 Mac): ~48 ms vs live 34 (fuse f30adc1991797392, 186 + 56 faces: ours 20, build 9,
  check 8; unify 14, usd.build 9.5); diffuse, no single lever. Doc 467 ms vs live 2112. Case: vibe5
  ~/agents/bool-keep/sgrec.
- blend FYI 11-17: cf-spur-gear's Boolean 1 54 ms vs live 34 (others under live); raw /root/blend-scratch/sg.log.
- Knife, mine (Mac): Fillet 2 85 ms vs live 10 (fuse: four plane x grind B-spline pairs 15-18 ms each, 1 line each,
  case c1d9f750d291ba85 / 17b8bafce508d06d; run-on tool op_split 25), next. Then core's handover: Revolve 1 (op_boolean
  63 ms Mac, sections ahead 44 vibe5), Extrude 8/9 (55/51), Extrude 2 (sections ahead 34), Extrude 3. Recorded cases:
  vibe5 ~/tmp/bool/knrec (Mac ~/noshape-host/agents/bool-hat/knrec; feature order by mtime: 17b8 c1d9 (Fillet 2),
  a3f9 (Chamfer 1), 682a (Mirror 1), then the cuts). Per-pair OCCT timing: /root/bool-scratch/ahead-debug3.diff
  (NOSHAPE_AHEAD_DEBUG). Chamfer 1's hulls: ~15 ms CPU a run; kind pre-check / plane give-up gained <= 3 ms: dropped.
- Fan-blade Boolean 2 (core-offset): OCCT's (tangent by design: thread crest r 4.000 along the 4 mm chamfer circle);
  no safe early decline ("one-face section" decline lost hundreds of taken cases). Lead agreed 11-17.
- Snap-hook Shell 1: its checks can't be skipped (the SI check is what finds offsets crossing; the wall can be
  invalid, the repair path); the wall was checked twice: dc060ff1c. Rings-loft Shell 1 dump ~165-180 ms Mac vs live 235.
- core: offset steps 1+2 gate passed (told 11-16 23:4x); offset_solid takes removed faces since 0c328e3b7: try on the
  Shell corpora (k::last_offset_report()).
- Scratch worktree with switches (ZZ_*): /root/bool-scratch/wt/bool-hat (rb.sh dir ~/agents/bool-hat on vibe5 / Mac,
  shared with bool-hat: never build both at once), never landed. Probes in untracked crates/kernel/tests/xprobe.rs
  (probe_shell_dump PROBE_REPS, probe_shell_corpus_ab, probe_unify_dump, probe_edge_off_face); before a build
  `git apply /root/bool-scratch/probe-allrs.patch`, after it `git checkout crates/kernel/tests/all.rs`.
- Not mine, handed back 11-16: tablet-stand Extrude 1, parting-line Rib (core bb61a968).

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
- Parallel-axis cone / cylinder disjointness (/root/bool-scratch/parked-cones-apart.diff: circles at each height,
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
3. Land bool-march (gate running), then the nuts cut (In flight).
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
  bool-hat (Mac builds: NOSHAPE_RB_HOST=mac). Before a commit: `git checkout crates/kernel/tests/all.rs`; after:
  `git apply /root/bool-scratch/probe-allrs.patch` (probe module xprobe.rs and sampler.rs are untracked scratch).
- Gate: `systemd-run --unit=bool-gate-<sha> --collect /root/bool-scratch/gate-chain.sh <sha>` (the branch's gate only;
  NOSHAPE_GATE_HOST to change host, default vibe5). Output /root/bool-hat-scratch/chain-<sha>.out (VERDICT,
  "CHAIN DONE rc N") and gate-<sha>.log; read with grep. gate.sh runs its replays through the host's ~/bin/slot
  (bool2's ff43a8c95). A crates/brep or Boolean change needs a clean gate and identical results end to end.
- Land: `NOSHAPE_LAND_QUEUE_HOST=vibe5 NOSHAPE_LAND_HOSTS="vibe5 vibe3" tools/tiers/land.sh` after
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
