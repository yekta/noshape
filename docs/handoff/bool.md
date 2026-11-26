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

## NEXT (state 11-26 09:40 UTC)
- SCREWS coverage QUEUED 00e677ac 08:21 (gate + knurls clean; lead told).
- WRAP-TEXT a7a17af18 (bool-slow, branch bool-cov): tilted arcs on a cylinder -> C2::Conic with Map::Cyl (not at the seam, start =
  a0's point, circle heights > 10 um off its centre's). Both cases (vibex8 ~/tmp/bool/wt1) ours 68/88 ms vs OCCT one-shot 298/135;
  recorded path occt_tree marks all Modified (ours Kept + solids Deleted); NOSHAPE_BOOL_NO_TREE=1 replay 0 differ. T1
  letters_wrapped_on_a_cylinder_with_rims_at_an_angle_fuse. GATE unit bool-gate-a7a17af18-x8 -> /root/bool-hat-scratch/chain-a7a17af18.out
  (waited ~55 min for a slot, building 09:30); chain unit bool-kn-a7a (/root/bool-scratch/chaina7a.sh -> kna7a.out) runs knurls and
  LANDS bool-slow if gate (rc 3, crate tests pass, replay differing equal, suite not fewer) + knurls 5x 0. Check kna7a.out; tell lead.
- LOFT-PATHS 4772e9a75 (worktree bool-sf, branch bool-loft, on a7a17): lies_on also needs parallel normals at the samples (a plane's
  samples are only its edges' points, all on a lofted wall from its outline -> 'same surface' -> wall OnSame, non-manifold).
  b076c08c ours 33 ms vs OCCT 151, 0 differ; T1 a_boss_lofted_from_a_slots_top_fuses_to_it. Chain unit bool-chain-4772
  (/root/bool-scratch/chain4772.sh -> kn4772.out) waits for kna7a's LAND rc 0, gates, knurls, lands bool-sf. Keep bool-sf and
  bool-slow CLEAN until then (probe worktree: bool-mf, detached).
- Rest of rp200 declines >= 100 ms (all parked): partial-fillet 7340044b: arrange.rs tie-break by a secant for Nurbs/Mapped fixes a
  real misorder (OCCT pcurve's end curvature wrong sign) but then a 57 um micro edge where 3 equal rounds meet (near-tangent;
  /root/bool-scratch/pf-secant-order-probes.patch); beb5d239, setback df44fb85, loft-paths e74bf29d (section along a face's own
  edge within 0.2 um; tb-probe.patch): tangent family. body-draft-naming e13ba873 528 ms: tool face a B-spline flat over part of
  its span ON the target plane, rising past a knot (needs splitting partly-flat B-splines; pp-pair-probes.patch). replace-face-
  exercise 5925baca 115 ms: OCCT's fuse EMPTY; ours 'no clean ray': all rays from a piece point meet target faces 11/29/30/31 facing
  the point with odd parity -> the target (from our Replace face) inconsistent; our doc differs from live at Replace face 1 (+19
  mm3): upstream, tell lead/core (ray-probe.patch, case vibex8 ~/tmp/bool/rf1). Everything else < 60 ms: coverage ~done.
- Next after the lands: report to the lead (<= 5 lines) with the parked list; then partly-flat B-spline split or the speed list.
- Idea: replay could run OCCT with set_boolean_no_tree for occt_tree cases (pcb 101003c5 then comparable).
- Slow-feature list, stacks, SP3 notes, unify try: archive "Handoff NEXT as of 11-26 08:10".
- Cleanup: vibex8 ~/tmp/bool {kt-slow, wt1, rf1, models, prof, fl-work, p0, kt}; worktrees bool-p0 (old), bool-sf/bool-mf targets
  after the lands. bool-corner removed 09:20 (probe saved bool-corner-probe.patch).

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
