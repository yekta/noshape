# bool: current state

Owner: bool. noshape's own Boolean (crates/brep, kernel cpp rust_boolean.cpp / boolean.cpp), the Shell builder
(by_quadrics, shell.cpp), the speed of Booleans, Shell and Split. History: docs/handoff/archive/bool.md. Standing
rulings: docs/DECISIONS.md (wins over anything here). bool2's notes: docs/handoff/archive/bool2.md.

Updated 11-15 21:46 UTC.

## In flight
- Queued to land: deb41792 (snap hook stack: uv_of cache; torus_sections by v for drafted planes; revolved circle
  arcs taken as their tori, spindle too; interpolated() early exit). Gate 8d7322fd7 clean (CHECK knurls only; replay 0
  new, the housing case not flagged; suite 78 -> 78, 881/935); hats/dock 12/12 SAME.
- Gating next: branch bool-shell-si 578353275 (on deb41792): Shell's self-interference pair filter tests an edge's
  points against the face's own parameter patch of a cylinder / cone / sphere / ring torus (closed form: the nearest
  angle round the axis in the box, then the generator's segment or the tube's arc) before meshing the face.
  c2-bottle-split Shell 1 33 -> 18 ms; identical output on the bottle and rings-loft shell-9 dumps; kernel shell
  tests pass. Then hats/dock (udoc-ab.sh: set its final `git switch` to the branch).
- Gate for core: 3315dcd6d (core-shellcheck, worktree core-oc), chain-3315dcd6d.out; send core the verdict.

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
1. qa's slower-than-live list (lead 11-15). Profiled: c2-hunting-knife Split 4 (108-174 ms): op_split ~28 ms, the
   rest Split Face projecting sketch lines along -y onto 2 B-spline faces (project_line_along: OCCT's plane section
   10-15 ms each, its tolerance check (GeomLib_CheckCurveOnSurface PSO) most of it; a 1 mm line along the blade's edge
   declines to BRepProj, ~20 ms, nothing). Dumps vibe5 ~/tmp/bool/kd (NOSHAPE_DUMP_OPS; probe_project_dump,
   probe_split_dump). c2-bottle-split Split 1: our own Boolean ~7 ms warm, spread out; Revolve 2 (25 ms, live 5): its
   fuse 4 ms, the rest building the revolution (not ours). Dumps ~/tmp/bool/bd.
   cf-snap-hook leftovers: offset x plane pairs (2, ~15-20 ms); our torus x plane pairs ~1 ms each; intervals_on's
   chords ~40 % of the cut's CPU.
2. c2-rings-loft Shell 1 (~510-590 ms, live 235): repair 251 (traces 95, split 78, crossings 26, joins 17),
   self-interference 67 (its EF time in ~23 pairs on free-form faces; quadric pairs are cheap), edges 44, check 35,
   thin 34. Points with no square foot: OCCT's global search found none wherever a Newton search from the own 25 x 25
   grid's local minima found none (160 of 160), so the global search may be skipped there (needs the gate).
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
- g++-13: /root/bool-scratch/gcc13-split.sh <file.cpp...> (bool-split's tree on vibe5).
- Private documents (hats, dock): vibe5 ~/agents/bool-keep/private (mode 700); compare main vs branch with
  /root/bool-hat-scratch/udoc-ab.sh (copies in ~/tmp/bool/udoc, deleted after).
- Probes (xprobe.rs): probe_case_timed (PROBE_CORPUS, PROBE_CASE, PROBE_RUNS; NOSHAPE_SAMPLE_OUT samples all runs),
  probe_case_sample (PROBE_ONLY rust/occt, PROBE_SAMPLE), probe_case_unify, probe_shell_dump (PROBE_DUMP=vibe7
  ~/agents/bool-keep/rl/shell-9; NOSHAPE_SHELL_DEBUG for stage times), probe_split_dump, probe_project_dump.
- Profiles: vibe5 ~/agents/profile-funcs.py, ~/tmp/bool/pu.py (inclusive under a function); vibe7 ~/tmp/bool/c2
  pu.py, callers.py (caller chains), lines.py / lines2.py (shell.cpp stage buckets).
- Recordings: NOSHAPE_RECORD_BOOLEANS=<dir> on a parity run (ops binary, NOSHAPE_PARITY_MODELS=vibe5
  ~/tmp/bool/ab/models, NOSHAPE_PARITY_ONLY=<doc>). Corpora: vibe5 ~/agents/bool-corpora, ~/agents/bool-keep,
  ~/agents/blend-keep/known-sections (tpd 6d3bca8654a6d02d, michael 1447eaf22e0cd418), vibe7 ~/tmp/bool/c2.
- Debug switches: NOSHAPE_FREE_SECTIONS=0 / NOSHAPE_FREE_DEBUG (marcher branch), NOSHAPE_OWN_SECTIONS=0,
  NOSHAPE_PROJECT_OCCT, NOSHAPE_SPLIT_DEBUG, NOSHAPE_SHELL_DEBUG; per-pair section timer for occt_ahead:
  /root/bool-scratch/torus-and-ahead-debug.diff (NOSHAPE_AHEAD_DEBUG).
- Notes for the snap-hook work: /root/bool-scratch/notes-snap.txt.
