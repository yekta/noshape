# bool: current state

Owner: bool. noshape's own Boolean (crates/brep, kernel cpp rust_boolean.cpp / boolean.cpp), the Shell builder
(by_quadrics, shell.cpp), the speed of Booleans, Shell and Split. History: docs/handoff/archive/bool.md. Standing
rulings: docs/DECISIONS.md (wins over anything here). bool2's notes: docs/handoff/archive/bool2.md.

Updated 11-17 01:13 UTC.

## In flight
- Landed 11-16: 190a71558 (cp2), 7d1b8799 (kernel profile per feature), 765b2281 (screws P0: OCCT 0031 + 0032),
  8e3067d8, 9d68c081, a8b8f1851, a2952b329.
- bool-knife (bool-hat; = bool-stack + 2, on main 8782214de): gate unit bool-gate-knife -> chain-cd9ad917c.out (queued
  in vibe5's slot). Then hats/dock (/root/bool-hat-scratch/udoc-ab.sh cd9ad917c), land.sh, and a one-line note under
  DECISIONS' 11-15 tangent ruling (lead 11-17: the face / face section margin may be 2 x the two faces' own
  tolerances; edge / vertex checks keep theirs; 21.6 um apart isn't a tangent contact). Commits:
  - 7e282fb90 hulls_apart (crossing patches given up past 8 halvings; knurl k25-15 hull CPU 473 -> 59 ms).
  - ce56b4632 Shell check (planes' open pairs by exact distance, vertex walk on non-plane faces, regions' pocket cut
    skipped when a cone / sphere / torus face would bound a pocket): Shell corpus 211/211 same, bottle Shell 1 Mac
    12-13 -> 8-9 ms; snap-hook neutral.
  - 3c8caba4f uv_of seed by k-d tree (T1 nearest_samples_are_the_scans); 7bc18f34b patch-grid cache (bit-exact key,
    LRU 64 MB); 7da692d7d rust_boolean.cpp:2703 parentheses.
  - 898c72461 unify's cylinder-group merges check the whole part once at the end (merge by merge again if the end
    isn't valid): Mirror 1's unify 151-158 -> 104-107 ms vibe5; the knife's 28 unify dumps identical
    (vibe5 ~/tmp/bool/kdump, probe_unify_dump).
  - cd9ad917c hulls_apart's margin from the faces' own tolerances (T1 a_blade_and_its_mirror_image_fuse_as_occt,
    case 682aa19d75e30e09): Mirror 1's fuse 115 -> 43 ms vibe5, same result.
  - Mac A/B main vs bool-stack (same session): knife 1042/1427/1443 -> 934/1307/1318 ms; bottle 48-51 -> 40-43 (live 40).
- OCCT 0033: commit f4ceaa2ff on branch bool-oc33 (patch only touches the non-MSVC branch; CI key hashes
  patches/occt/**). Libs STAGED (told the lead 11-17 01:1x): vibe5 ~/agents/bool-keep/occt-0033, Mac
  ~/noshape-host/occt-0033 (no OCC_CONVERT_SIGNALS). All checks clean: crash_inputs:: 4/4 Linux + Mac; qa's 3 segv
  docs SAME (Mac); identity vibe5 (replay 3025/6 -> 3025/6, parity56 512/0 -> 512/0, suite 1194/1573 both, only the
  sprockets / rocker region-order flake); hats/dock 12/12 SAME. Knife Mac: Split 4 129-158 -> 60-80 ms. Land
  bool-oc33 once the lead has installed the libs on every host.
- bool-knife2 (bool-hat, on bool-knife): a5c287f9a free-form samples + k-d index kept across Booleans (GRID_CACHE,
  same LRU as the patches; knife Extrude 8 22-34 -> 12-17, Extrude 9 24-34 -> 14-20, Revolve 1 58-76 -> 49-59 Mac);
  dc060ff1c Shell's wall checked once (it was checked twice), parallel past 16 faces (snap-hook Shell 1 dump
  -15 %, same faces / edges). Needs the gate + Shell corpus A/B + hats/dock after bool-knife lands.
- Fillet 2 (11-17, Mac 100 ms vs live 9): op_boolean 30 (cut + fuse), op_split 28 (dump split-11: the 5-face
  run-on tool split by 2 planes + the grind B-spline, OCCT's splitter 25 ms), min_distance x31 11, op_fillet 10,
  props_surface x11 6. A cross-Boolean exact-box cache can't hit: the cut trims the grind face (a new face, a
  new box). No single lever left in mine.
- Revolve 1 (54f29c1fecb98afe) after the grid cache: 60 % OCCT's sections ahead (ComputeTolReached3d PSO 24,
  CheckSingularPoints' Extrema grid on the whole blade 14, WLApprox 10, ImpPrm 8), free::apart 8.5. Extrude 8 =
  5d62643e95aabaa7, Extrude 9 = b58272a7dbba8cc5 (all cut the blade's one B-spline face, x 6-122 mm).
- blend FYI 11-17: cf-spur-gear's Boolean 1 54 ms vs live 34 (others under live); raw /root/blend-scratch/sg.log.
- Knife, mine (Mac): Fillet 2 85 ms vs live 10 (fuse: four plane x grind B-spline pairs 15-18 ms each, 1 line each,
  case c1d9f750d291ba85 / 17b8bafce508d06d; run-on tool op_split 25), next. Then core's handover: Revolve 1 (op_boolean
  63 ms Mac, sections ahead 44 vibe5), Extrude 8/9 (55/51), Extrude 2 (sections ahead 34), Extrude 3. Recorded cases:
  vibe5 ~/tmp/bool/knrec (Mac ~/noshape-host/agents/bool-hat/knrec; feature order by mtime: 17b8 c1d9 (Fillet 2),
  a3f9 (Chamfer 1), 682a (Mirror 1), then the cuts). Per-pair OCCT timing: /root/bool-scratch/ahead-debug3.diff
  (NOSHAPE_AHEAD_DEBUG). Chamfer 1's hulls: ~15 ms CPU a run; kind pre-check / plane give-up gained <= 3 ms: dropped.
- Fan-blade Boolean 2 (core-offset): OCCT's (tangent by design: thread crest r 4.000 along the 4 mm chamfer circle);
  no safe early decline ("one-face section" decline lost hundreds of taken cases). Lead agreed 11-17.
- Snap-hook Shell 1 (after the knife): SI check 35 %, BRepCheck 23 %, add_pcurve projections 19 % (vibe7 sample);
  lead's question: can either check be skipped where our builder guarantees the property? Then rings-loft Shell 1.
- core: offset steps 1+2 gate passed (told 11-16 23:4x); offset_solid takes removed faces since 0c328e3b7: try on the
  Shell corpora (k::last_offset_report()).
- Scratch worktree with switches (ZZ_*): /root/bool-scratch/wt/bool-hat (rb.sh dir ~/agents/bool-hat on vibe5 / Mac,
  shared with bool-hat: never build both at once), never landed. Probes in untracked crates/kernel/tests/xprobe.rs
  (probe_shell_dump PROBE_REPS, probe_shell_corpus_ab, probe_unify_dump, probe_edge_off_face); before a build
  `git apply /root/bool-scratch/probe-allrs.patch`, after it `git checkout crates/kernel/tests/all.rs`.
- Not mine, handed back 11-16: tablet-stand Extrude 1, parting-line Rib (core bb61a968).

## Parked (branches / diffs, not landing as they are)
- Plane x B-spline marcher + our own least-squares fit (branch bool-free-sections-wip: 71efdd87e, f6c77c748, ...):
  fitted() degree 5 by chord length, one solve for the curve and both pcurves; tpd's fillet cut CPU 3.4 -> 2.2 s per
  9 runs. FAILED gate f6c77c748: c2-hunting-knife's Mirror 1 31/81/51 vs live 22/60/39 (46 -> 39/52). Every recorded
  knife case gives the same counts marcher on / off on the same inputs (probe_case_unify); the cut decfe3013034332d
  (vibe5 ~/tmp/bool/kn) keeps 16/44, volume 1e-8 off, and the mirror's fuse 69dc7f64260fff42 (~/tmp/bool/kn1,
  recorded marcher on) then gives 36 faces, 31 after unify, for ours and OCCT alike. The curves' tolerance is not it
  (a 1e-9 floor: still 31/81/51). Next there: compare decfe3's result edges / pcurves on the mirror plane with
  OCCT's. Its plane_tube_sections refinement (steps hiding a short run cut 64x; flat arcs' steps skipped; tpd: 13
  more pairs) moves rings-loft's slot cut 1.2e-8 off OCCT (slots_across_a_rings_rounds_cut_as_occt).
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
3. The marcher's knife regression (above), then gate it again.
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
