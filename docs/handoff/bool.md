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

## NEXT (state 11-26 10:57 UTC)
- QUEUED ac1aac8d 10:52 (bool-slow, branch bool-cov): screws coverage (00e677ac FAILED 08:58 on a handoff rebase conflict, rides
  along), WRAP-TEXT a7a17af18 (tilted arcs on a cylinder -> C2::Conic with Map::Cyl; ours 68/88 ms vs OCCT 298/135; T1
  letters_wrapped_on_a_cylinder_with_rims_at_an_angle_fuse), 5e07cd824 gate-host.py ignores "a gap" timing hints (its gate's only
  CHECK was 21 docs whose gap hint changed). Gate + knurls 5x 0. Lead told.
- LOFT-PATHS 063eb6fea (worktree bool-sf, branch bool-loft, rebased on 5e07cd824): lies_on also needs parallel normals at the
  samples. b076c08c ours 33 ms vs OCCT 151; T1 a_boss_lofted_from_a_slots_top_fuses_to_it. Unit bool-chain-b
  (/root/bool-scratch/chainb.sh -> chain-063eb6fea.sh -> kn4772.out) gates, knurls, lands bool-sf (only once wrap-text is on
  main). Keep bool-sf CLEAN until then.
- Coverage ranking DONE: every decline >= 100 ms parked: partial-fillet 7340044b (pf-secant-order-probes.patch; 57 um micro edge),
  tangent family beb5d239, setback df44fb85, loft-paths e74bf29d (tb-probe.patch), body-draft e13ba873 (tool B-spline index 2
  lies on target plane y 14.58 mm along a straight stretch of its profile, then leaves it C2 past a knot: the overlap's edge is a
  tangent contact -> OCCT's; OCCT's result there is invalid, the doc's first live difference is Fillet 1 just after, not shown
  related). replace-face 5925baca: target from our Replace face inconsistent (+19 mm3 vs live): asked lead whether core's.
- SPEED LIST re-measured 11-26 10:50 on main e6bae0fd7 (vibex8 ~/tmp/bool: cli-main, models = copies of ~/qa-parity/saved,
  fl-main2.sh -> fl-main.jsonl, sel-main.jsonl 69 rows >= 100 ms and >= 2x live, prof-main.jsonl). By Boolean/Shell time:
  impeller Fillet 4 (2129/91 ms: our Boolean 1661, OCCT's per-pair sections 1536 in 20 serial calls; case rec-imp
  9040230e4b11f11b, cut 45 faces, ours 1662 vs OCCT 364-432 ms, 0 differ), knifehandle Fillet 1 (OCCT path 1634), impeller
  Fillet 3 (1579, 3 Booleans), impeller Boolean Flansch+Gehaeuse (591, sections_ahead 361), rings Shell 1 (555), loft-paths
  Mirror 1 (487) / Loft 5 (383, sections_ahead 366), impeller Chamfer Stevenrohr (418), Shell Gehaeuse (297). NOW: why
  Fillet 4's 20 pairs are not made ahead in parallel: probe sec-probe.patch (PROBE_SEC per call: pair, path seeded/ahead/own/ff,
  ms) in bool-mf (detached at main). recfeat.py names each recorded case's feature (rec-imp, recfeat-imp.out).
- Idea: replay could run OCCT with set_boolean_no_tree for occt_tree cases (pcb 101003c5 then comparable).
- Cleanup: vibex8 ~/tmp/bool {kt-slow, wt1, rf1, models, rec-imp, rp1, fl-work, p0}; bool-sf/bool-mf targets after the lands.

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
