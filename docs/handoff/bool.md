# bool: current state

Owner: bool (bool2 merged in 11-19). noshape's own Boolean (crates/brep, kernel cpp rust_boolean.cpp / boolean.cpp's
unify), the Shell builder (by_quadrics, shell.cpp), the speed of Booleans, Shell and Split. History:
docs/handoff/archive/bool.md (the 11-19 16:45 handoff is at its end) and archive/bool2.md. Rulings: docs/DECISIONS.md.

Updated 11-26 08:30 UTC. Hosts by load (DECISIONS 11-19): read
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

## NEXT (state 11-26 08:30 UTC)
- SCREWS coverage (696a6e89d, gate 65a212abf clean, knurls 5x 0): land 86f06be4 failed 08:16 on a rebase conflict;
  rebased clean, QUEUED 00e677ac 08:21 (worktree bool-slow, branch bool-cov). Lead told.
- WRAP-TEXT a7a17af18 (on bool-cov after screws): the letter tools' rims are 3D circles at an angle to their cylinder
  face's axis (on it only along their stretch); c2_of gave them a flat Seg at the circle's centre height (0.9 mm off
  the vertices) -> 'no point inside a face piece'. Now: not at the seam, start = the arc's a0 point, and the circle's
  heights at start / middle / end > 10 um off its centre's -> C2::Conic with Map::Cyl (as cones' ellipses). Both
  wrap-text cases (vibex8 ~/tmp/bool/wt1: 34dfd505 40 tools, 4ff76287 7 tools) taken, 68 / 88 ms vs OCCT one-shot
  298 / 135 (recorded tree 523 / 262). Their recorded path is occt_tree (fuse_tree, scaled): its history marks
  everything Modified, ours Kept + solids Deleted; with NOSHAPE_BOOL_NO_TREE=1 the replay is 0 differ (history =).
  bool-corpora/pcb 101003c53c73e996 (occt_tree) differs the same way already. T1
  bool_patterns::letters_wrapped_on_a_cylinder_with_rims_at_an_angle_fuse (data 4ff76287). GATE RUNNING: unit
  bool-gate-a7a17af18-x8 -> /root/bool-hat-scratch/chain-a7a17af18.out; then knurls (chaincovk.sh pattern), land.
- PARKED (tangent family, OCCT's per ruling 11-15): knifehandle 856a411e / 244416f8, impeller ed1c251b / 552bc1ba
  (details + probe patches: archive "Handoff NEXT as of 11-26 08:10"). Remaining declines by OCCT ms: impeller
  1599/758/418/198/188, loft-paths 628 (tangent)/270, partial-fillet 545/146, setback 146. Next: loft-paths 270,
  partial-fillet 545/146, setback 146 (rp200: ~/agents/bool-keep/rp200-docs.txt maps declines to docs).
- Idea: the replay could run OCCT with set_boolean_no_tree for occt_tree cases (pcb 101003c5 then comparable).
- Slow-feature list, stacks, SP3 notes, unify try: archive (same section).
- Cleanup: vibex8 ~/tmp/bool {kt-slow, imp1, wt1, pw*.out, models, prof, fl-work, p0, kt}; worktrees bool-flush,
  bool-corner, bool-p0 (old) with their targets.

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
