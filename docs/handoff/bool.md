# bool: current state

Owner: bool (bool2 merged in 11-19). noshape's own Boolean (crates/brep, kernel cpp rust_boolean.cpp / boolean.cpp's
unify), the Shell builder (by_quadrics, shell.cpp), the speed of Booleans, Shell and Split. History:
docs/handoff/archive/bool.md (the 11-19 16:45 handoff is at its end) and archive/bool2.md. Rulings: docs/DECISIONS.md.

Updated 11-19 21:20 UTC (fresh bool thread after the 17:30-21:10 usage stall). Hosts by load (DECISIONS 11-19): read
`ssh <h> cut -d" " -f1 /proc/loadavg` on vibex7 / vibex8 / vibe2; builds + probes on the Mac (NOSHAPE_RB_HOST=mac,
inside an rb.sh slot, Mac paths ~/noshape-host/bool-keep); gates through ~/bin/slot, one per host; land queue vibe2
(`NOSHAPE_LAND_QUEUE_HOST=vibe2 tools/tiers/land.sh`); private docs only vibex7 ~/lead-private. Never kill a process
tree from a computed pid: `systemctl stop <unit>`.

## Order (lead 11-19 20:41)
1. bool-stack7 land. 2. HOLE-THROUGH-ROUND-POLE (TICKETS). 3. bool2-exact (below). 4. live2's Shell 3 ticket
(P2 SHELL-AFTER-WELD-FILLET). 5. the rest by P0/P1: Shell end-to-end list (9 -> under 5 by 11-20 18:00), decline
reasons ranked, speed list. Daily report: share as "ours N/M (general, direct), OCCT, differing" (DECISIONS 15c2c6406).

## In flight
- bool-stack7 (34 commits; tip 50e41e425 on main 0bfc1ed03): gate 1c8b8f4cf clean (replay taken 3044 -> 3138 +94 -0,
  differing 6 -> 6; suite features 1332 -> 1336; crate tests pass; hats/dock SAME, udoc-1c8b8f4cf.out); knurls not run
  in the gate (31 GB host). Knurls ours-only main vs branch: unit bool-kn-s7 (/root/bool-scratch/kn-s7.sh, runs on
  vibex7 ~/tmp/bool/kn7 via slot; `journalctl -u bool-kn-s7`; "diff-lines 0" per knurl = clean).
  Rebased on main 8403d168f as branch bool-land7 (worktree bool-tube; pushed origin/bool-land7), tip 07c3dc068. Four
  conflicts: main's 9f550d1ad touch_only kept, with the stack's "only sheets decline a shared shape" on top; arrange's
  -π directions = main's a1509406f (same case 553beda0); the plane / curved sheets' empty common = the stack's (adds
  sphere, torus to main's e466cfa77); the cylinder face's angles = main's 1b382d666. Re-gate: unit
  bool-gate-07c3dc068-x8 -> /root/bool-hat-scratch/chain-07c3dc068.out. Clean (and knurls clean) -> land from
  bool-tube (`git switch bool-land7` first; the gate detaches it), then tell dx / core / blend / asm.
- HOLE-THROUGH-ROUND-POLE (asm, isaac Fastened 43): repro recorded on vibex8 ~/tmp/bool/hp (mini.sh, rec/; copies
  /root/bool-scratch/hp/rec, Mac bool-keep/hp-rec). The cut 0ea218c03ac1f745 is OCCT's (ours declines), valid, volume
  16987 mm3 (= 17298 - 311, right), but eval.rs non_manifold() finds an edge bounding > 2 faces, so the feature errors
  "would result in non-manifold body". Probe: Mac bin/hp xprobe::probe_case_nonmanifold (PROBE_CORPUS=hp-rec
  PROBE_CASE=0ea218c03ac1f745, PROBE_RUST=1 for ours) prints that edge.
- bool-pole (bool-hat, 6 commits on bool-stack7, tip 0d465cb70; rebase on bool-land7 after it lands; then its own
  gate with the Shell check + udoc):
  e4f8a8066 a sphere's pole edge split at the curves ending there (non-manifold cdd336b4 taken = OCCT);
  01775a048 straight edges in a row share one Geom_Line (unify joins them as OCCT's);
  fb3d29a2a Face Generated Vertex for every argument's coplanar face (0 new differences);
  4b1784e6c the 50 um rule (Shell slab Boolean with an input edge >= 5e-5 tolerance is OCCT's; removes 5 differing).
  Declined by it, TO WIN BACK with a narrower signal: gate corpora 0670c350, 280af0bc, 2b9c38da, 2c9c42e4, 30afb30c,
  475df60e, 570dfc37, 5fc0970a, 70b3fb42, 818552c9, 83b621a3, 8e91c517, 922c847a, 952c2efc, a1186078, a7bdd15b,
  b359e025, cb7bb22b, e1dc8c98, f1c35250, f302ef40; Shell corpus 1451f7cb, 43a947f0, 96d45f42, 9bee135f, c25f7a31.
  afcf858f3 slab union history differing only in vertex records = equivalent (lead 11-19); 0d465cb70 the gate's
  end-to-end Shell check (gate-host.py SHELLS / SHELLS_KNOWN).
- Shell end to end (lead: 9 -> under 5 by 11-20 18:00, else tell the lead). Ours vs OCCT-only over core's 211 fuzz
  shells (Mac bool-keep/shellcorpus): bool-pole differs in 9: 13186b07 b64c8b4e 9134b29f 8eebe1e2 (+1 face), f815832c
  (+1 face, vol 5e-4), 8e657637 (+26 edges), b79e508b (-1 edge), 17488d26 (vol 5e-5), 99b55ec9 (vol 2e-8). Per shell:
  - f815832c: Common 9e2bf7c0: ours 2.719411e-6 vs OCCT 2.720903e-6 m3, 59 vs 58 faces; probe_who_is_right: ours right
    at 7 of 8 grid points. Likely OCCT's error: decision needed (lead).
  - 17488d26 (and 7724 / 86b50d59): 36-slab union; coincident cylinder faces arg9 Face4 / arg24 Face2: OCCT Deleted,
    ours Modified to both halves. Next: OCCT's same-domain representative (BOPAlgo_Builder_2.cxx FillSameDomainFaces).
  - Unify family: b79e508b = 4dc6b1d4, da3ee477 / 71ca, f97ca580, 3b56cb83, 75780985 (tried and dropped: lines shared
    per section; first argument's piece always; OCCT's rule literally; declining coincident cylinders).
  - 99b55ec9: Common df5e46e1 equivalent (1.7e-8) but the Shell's volume string differs.
  Scripts: /root/bool-scratch/shc-e2e2.sh <bin> <runs>, mac-shell-env.sh <bin> <ENV..>, mac-ab-vs.sh. Probes: xprobe
  probe_who_is_right, probe_faces_on_plane, probe_edges_on_line, probe_unified_edges.
- live2's Shell 3 ticket (P2 SHELL-AFTER-WELD-FILLET): Part 3's Shell itself is right but slow (Linux 15-24 s, Mac 3.5
  s): ~4 s is ours computing sections of the fillet's B-spline faces in the 45-slab union then declining, OCCT redoes
  them; ours also declines the next Common after ~0.3 s. The feature failure is NOT the time budget (120 s changes
  nothing). In progress at the stall: an instrumented ops test (bool-hat uncommitted crates/ops/tests/all.rs `mod
  xshell3;`, detail.rs DBGSHELL / DBGBLEND eprintlns) printing per part the Shell's solids, shell_is_whole and the
  kernel error. Not to commit.

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
  Measurement (vibex8 unit bool2-tdbg53b, /root/bool2-scratch/tdbg53b.out, "VOL ours" / "VOL occt" per case):
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
- Speed (lead 11-19): pulleys Circular pattern 1 152 ms vs live 23 (doc 238 vs 102), time in the Boolean.
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
