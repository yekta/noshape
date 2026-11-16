# QA handoff (agent qa; current state only, rewritten 2026-11-15 18:00)

qa tests releases and greens against the last release and live, times speed, files tickets in docs/TICKETS.md, and never
edits code. Standing instructions: /root/lead-scratch/prompt-qa.md. Rulings: docs/DECISIONS.md (read first). History:
docs/handoff/archive/qa.md. Scripts: /root/qa-scratch/bin (local), results under /root/qa-scratch. Docs commits from
/root/projects/noshape-wt/qa (`git checkout -B agent/qa origin/main`, commit, `git push origin HEAD:main`).

## Running units
Local units are transient (`systemctl list-units 'qa-*'`); restart one with the `systemd-run` line in its description if it's gone.
- **qa-compat-loop**: bin/compat-loop.sh <baseline>, now **0.4.52**. On every watcher green, bin/compat-check.sh runs userdocs.py
  on vibe5 (hats, dock, 47 ticket docs, 16 authored docs from compat-docs/v0443) with the baseline's CLI against the green.
  A feature that gets worse writes compat/ALERT-<sha> and one line per feature to /tmp/lead/qa-alerts. Move the baseline by
  restarting the unit with the new version (rel-published.sh does it).
- **qa-w7-nightly**: bin/w7-nightly.sh. Sleeps to 02:00 UTC, then on vibe7 with the newest release (downloaded to vibe7
  ~/qa-rel/<v>/) runs: (1) opentime.py (hat8, hat15, hat15b, dock) and modeltime.py (84 suite docs), 3 runs, the previous release's CLI
  alternating with the current one (DECISIONS db4db21a0; the row shows the same-session sums and docs > 1.25x slower, details
  nightly/<mmdd>.json vs_prev); nightly-sum.py
  -> a row in SCORE.md "Nightly, vibe7 02:00" (pushed from the qa worktree); (2) the latency set (bin/../lat/hat/lat-priv.sh:
  hat15 / dock UI-script rounds + cold / reopen + rotate / pan / zoom, block baseline) -> nightly/lat-<mmdd>.txt; (3) the
  QUIT-ABORT-DOCK guard (qa-asm-a x5) -> nightly/quit-<mmdd>.txt. Failures and a crash append to /tmp/lead/qa-alerts. Log:
  suites/window-0200-<mmdd>.log. Skip a night: touch nightly/skip-<mmdd>. Runs alone (no slot: the window must be quiet).
- **qa-suites**: bin/qa-suites-loop.sh, every 4 h: interaction suite, model suite (speed, debug) and assembly / exchange
  suites of the newest green, all on **vibe7** (lead 11-15: vibe5 is the land queue), each through `~/bin/slot` there.
  Results suites/<mmdd-hhmm-sha>/; host overrides QA_UI_HOST / QA_SPEED_HOST / QA_SUITES_HOST.
- **qa-land-footer**: bin/land-footer.sh (land-queue footer checks).
- vibe7 user units **qa-fzloop** + **qa-fz-xvfb** (`systemctl --user` there): the GUI fuzz, ~/qa/bin/fzloop.sh :61 on
  ~/qa/rel/noshape-green (0.4.52-era green; refresh by copying a newer app to that path), seeds from ~/qa/fzseed.next,
  results ~/qa/fz*-result.txt. fzrun.sh kills the app's own `--reopen` restarts after each run. Pause: touch
  ~/tmp/qa-fz-pause-61; it also idles 01:50-04:05 every night (the window runs alone).

## Release checks (per release; lead cuts 05:00 / 17:00)
1. Candidate: bin/rel-cand.sh <prev> <tag> (checks out nothing: check out the candidate in worktree qa-bisect first; builds
   its CLI on vibe5 through rb.sh, then userdocs vs <prev>: user docs + f197, the 4 Hole docs, tickets, authored docs) ->
   compat/cand-<tag>.txt. Reply go / no-go. A user-document regression is reverted on the branch, never shipped.
2. Published: bin/rel-published.sh <v> <candidate CLI on vibe5> -> compat/published-<v>.txt: SHA256SUMS of the Linux
   assets, CLI installed as vibe5 ~/qa-rel/<v>/noshape-cli, userdocs candidate vs released (must be ALL-SAME), qa-asm-a x5
   on the released app (vibe6, slot), then the compat baseline moved. Then bin/rel-compat.sh <prev> <v> (released CLI vs
   the previous release) -> compat/rel-<v>.txt; any user-document worse = P0 to the lead at once.
3. The app spot-check rows (archive "Release checks: the app spot-check list"): user cases + 8 interaction-feel rows in
   rotation (next: rows 9-16).
- **v0.4.53** (145ad9e67): candidate go 11-15 17:50 (72 docs all same vs 0.4.52; CLI vibe5
  ~/agents/qa-bisect/target/release/noshape-cli). Next: when published, `rel-published.sh 0.4.53
  /home/ngyekta/agents/qa-bisect/target/release/noshape-cli`, then `rel-compat.sh 0.4.52 0.4.53`.

## Open items
- **Speed (P0)**: owners got their slower-than-live documents 11-15 (from 11-14's vibe7 window, v0.4.48): blend (faceblend,
  tpd, michael, motor-mount, bracket03, pipeflange, radijator), core (impeller defeature, louver, housing revolve, lighten's
  opEnclose / opOffsetFace done 3bfc17b1), bool (knife Split 4, bottle Shell / Split, rings Shell + CP2, housing / louver /
  snap-hook Booleans, lighten opShell), bool2 (valve CP1, helical Sweep, housing / impeller unify), dx (std cold compile,
  evEdgeConvexity), skasm (corner-break: 1850 ms vs live 1972 on main, closes unless the window disagrees). After each
  night: send blend faceblend / tpd / michael and skasm corner-break from the new SCORE row; re-list what is still slower.
- **Interaction latency**: absolute numbers only from the nightly (DECISIONS 11-15). Candidates from vibe6 11-15 (load
  3-30): rotate / pan / zoom 2-5 fps on hat / dock vs 11-15 on the block (llvmpipe), hat hover first frame 0.9 s (live 600
  ms on a block), dock's first switch to B 14.7 s settle. psui owns the hat15 fillet edit (Tab not applying a variable-bound
  Radius; ✓ leaving the dialog open); my 7.9 / 4.4 s were the caret blink (lat-steps now 450 ms quiet). Send owners only
  what the nightly confirms slow, or > 3x a PLAN 2.7 target.
- Clean A/B v0.4.48 vs v0.4.53 (impeller, helical, knife incl. Fillet 5): bin/ab-clean.sh as unit qa-ab-clean, after asm's
  full7 on vibe7, `SLOT_QUIET_OK=1 ~/bin/slot`, load every 30 s, removes ~/.slot-quiet -> nightly/ab-clean-<mmdd>.*; valid if
  load < ~1.5. Owners so far: impeller Delete face 2 is upstream (core: Move face 1 fails where live builds; blend Fillet 28/29),
  impeller fillet failure cost (blend 29c1841df), helical Sweep 1 fixed on main (bool2 895c8e3b / e47268f2).
- v0.4.54: unit qa-pub-0454 builds the candidate CLI (release/v0.4.54 8a8537439, qa-bisect, vibe5), then rel-published.sh
  0.4.54 and rel-compat.sh 0.4.53 0.4.54.
- The suites runs of 11-16 00:32 (f14f85a8) and 04:43 (601df0ac) were stopped to keep vibe7 quiet; the loop reruns.
- QUIT-ABORT-DOCK: verified fixed (80e16e42d); guarded by the nightly and rel-published.
- f197 Fillet 1 vs 0.4.51 was core's 1ac08e247 (reverted on v0.4.52's branch); 0.4.52 baseline is clean.

## Expected userdocs differences (not regressions)
- dock "E2 - C Stand" E2 Govde 1345 / 3103 / 1759 (was 3104 / 1760, volume 62443.95 the same) from core's ee16972d8 (shown
  topology hides edges internal to a face; DECISIONS e677ec1c0). Expected in every compat / release check against a release
  without it; drop this line once the baseline release has it.

## Tools
- userdocs.py <cliA> <tagA> <cliB> <tagB> <doc>...: fresh 600 copies in ~/tmp/qa/ud-<pid>, per Part Studio feature status /
  message and parts (faces, edges, vertices, volume); prints SAME / DIFF. Uses store/sig.py + bin/nscli.py.
- Latency (/root/qa-scratch/lat/hat): mkscen.py <out> <doc> <face x,y,z> <feature> <field> <value> <rounds> <filter>;
  split.py <name> [tabs] (one round per file: the runner's 180 s cap); lat-priv.sh <doc name> <scenario prefix> <open rounds>
  (APP, XD, DOC env; NOSHAPE_FRAME_LOG + NOSHAPE_PERF=1); lat-open.py <dir>; ../lat-steps.py <scenario> <frames log>.
  quitv.sh <runs> <scenario> <app>... (XD, SD env) for crash loops; quit scenarios qa-asm-a / qa-tabs / qa-edit.
- Speed: mac/opentime.py, mac/modeltime.py (vibe7 copies in ~/agents/qa-keep), bin/nightly-sum.py, mac/live-regen.json.
- Private documents (hat8, hat15, hat15b, dock in vibe5 / vibe7 ~/lead-private) only as mode-700 copies in ~/tmp/qa on a
  host, deleted after; never in the repo or tickets.

## Hosts
vibe5: land queue, compat loop (one CLI at a time), release builds via rb.sh (worktree qa-bisect). vibe6: ad-hoc app runs
(:661), release quit check. vibe7: suites (slot), fuzz :61, nightly :671. vibe3: watcher only. The Mac: off limits except
its single slot. Heavy runs through `~/bin/slot` (DECISIONS 11-15).
