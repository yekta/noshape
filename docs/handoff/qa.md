# QA handoff (agent qa; current state only, updated 2026-11-17 14:10 on the Ubeyd account)

qa tests releases and greens against the last release and live, times speed, files tickets in docs/TICKETS.md, and never
edits code. Standing instructions: /root/lead-scratch/prompt-qa.md. Rulings: docs/DECISIONS.md (read first). History:
docs/handoff/archive/qa.md. Scripts: /root/qa-scratch/bin (local), results under /root/qa-scratch. Docs commits from
/root/projects/noshape-wt/qa (`git checkout -B agent/qa origin/main`, commit, `git push origin HEAD:main`).

## Running units
Local units are transient (`systemctl list-units 'qa-*'`); restart one with the `systemd-run` line in its description if it's gone.
- **qa-compat-loop**: bin/compat-loop.sh <baseline>, now **0.4.56** (moved by qa-pub-0456 11-17 06:43). On every watcher green, bin/compat-check.sh runs userdocs.py
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
  Restarted 11-17 07:31: the lead's $HOME fix of the CLI pair (11-17's row INVALID: `~` after a comma isn't expanded, 1 of
  84 timed) and a guard: < 90 % of the docs timed or < 6 opens -> alert, no SCORE row. Earlier, 03:33: the clean A/B step (nightly/ab-<mmdd>.args) and the latency step now
  `cd … || exit 1;` (the A/B's `cd … && ( load loop ) &` backgrounded the cd, ab-feat.py ran in ~ and the loop held the ssh).
- **qa-suites**: bin/qa-suites-loop.sh, every 4 h: interaction suite, model suite (speed, debug) and assembly / exchange
  suites of the newest green, each through `~/bin/slot`: ui + models on **vibe7**, assemblies / exchange (suites-rest) on
  **vibe6** beside them since 11-17 04:51 (vibe7's slot was held 4 h by dx). 1117-0227-ecf8c33a's stage re-run as unit
  qa-suites-redo-0227 (QA_REDO), done 07:07: models 85/126 1233/1585 (5 better, none worse), assemblies 5/24 1614/1709
  664/755 (scissor-lift BOM 59 -> 57: P0 to asm), exchange 58/74. The models report.json is written beside the corpus
  snapshot (~/tmp/qa/report.json): qa-suites.sh copies it from there since 07:15 (lost since 11-16's snapshot change).
  Results suites/<mmdd-hhmm-sha>/; host overrides QA_UI_HOST / QA_SPEED_HOST / QA_SUITES_HOST.
  1117-0627-827a23a6 compared 11:20 (report scp'd by hand: the run began before the 07:15 path fix; next runs fetch it).
- **qa-cut-0415 / qa-cut-1615** timers: bin/cut-verdict.sh (cut-check.sh: newest green's compat vs the baseline) -> one GO /
  NO-GO line in /tmp/lead/qa-alerts and compat/cut-<mmdd-hhmm>.txt. Add the verdict as a "Release blockers" section in TICKETS.md.
  Fixed 11-17 04:25: cut-check waits for the loop's run under vibe5.lock (two runs at once gave "Text file busy" and a void GO);
  cut-verdict says NO VERDICT unless the green's compat file has COMPAT-DONE, 4 USERDOCS lines and no Traceback. Ticket documents
  are listed, not blocking, in the script, but read them: a ticket doc's wrong geometry is a HOLD (11-17 f197).
- tab-B hover-card A/B DONE 10:29 (units qa-tabab-verify2, qa-tabab-rel finished): B's first open ~5.0 s green vs ~6.4 s 0.4.53
  (hint, load 2-14); the ~700 ms trailing frames after a switch are in 0.4.54-0.4.56 too -> P2 DOCK-TAB-SWITCH-TRAILING-FRAMES (psui).
- **qa-land-footer**: bin/land-footer.sh (land-queue footer checks).
- vibe7 user units **qa-fzloop** + **qa-fz-xvfb** (`systemctl --user` there): the GUI fuzz, ~/qa/bin/fzloop.sh :61 on
  ~/qa/rel/noshape-green (green 35baa32f since 11-17 03:34; refresh by copying a newer app to that path), seeds from ~/qa/fzseed.next,
  results ~/qa/fz*-result.txt. fzrun.sh kills the app's own `--reopen` restarts after each run. Pause: touch
  ~/tmp/qa-fz-pause-61; it also idles 01:50-04:05 every night (the window runs alone). App refreshed to green f7c318a5 11-17 10:08.
  w7-quiet.sh end (local bin/ copy, scp'd each night) now recreates the transient unit with systemd-run when `start` fails:
  the stopped unit was gone, so the fuzz was down 11-17 02:01-10:08.

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
- **v0.4.56** published check done 11-17 06:58: clean, baseline 0.4.56. Next: the 17:00 cut (qa-cut-1615).

## Next (in this order)
1. **17:00 cut**: TICKETS.md "Release blockers for the 2026-11-17 17:00 release" (12:35: GO on 7f287e52; c101c3c0 has core's
   four Split / Use / Rib lands; suites 827a23a6 clean; fan-blade-sweep Boolean 2 is now live's volume, 8x slower: core's Offset, sent to core). Still to come:
   blend's impeller Fillet 3 (when it lands: compat on its green + the impeller re-time below).
   Coming for 17:00 (lead 13:20): psui 452838df + skasm b87dd3dcf (user P0s: verified in the app on c330a760 14:05), blend
   9e40ad95a, bool2 555bcb4c (queue/bool2-555bcb4c): compat on each green as it lands. Watcher: bin/watch-lands.sh <state>
   <commit>... (matches by subject, exits when one is in a green with COMPAT-DONE; state compat/lands-1700.state).
   App user-flow checks: /root/qa-scratch/uh/uh2.sh <app> <out> (sketch hover, real xdotool moves after a UI-script setup)
   and uf.sh (seeded feature_settings.json; NOSHAPE_UI_REMEMBERED=1 or the runner ignores the memory), on vibe6 :663.
   The qa-cut-1615 timer runs cut-verdict.sh at 16:15 -> one line in /tmp/lead/qa-alerts + compat/cut-1117-1615.txt. Read
   the ticket section of that compat file too (a ticket doc's wrong geometry is a HOLD), then update the entry with the
   verdict and the green it names, commit docs-only. HOLD only for a real P0 (crash, wrong geometry, speed regression, a user
   document changed). After the cut is published: `systemd-run --unit qa-pub-<v> bash /root/qa-scratch/bin/rel-pub-all.sh
   0.4.56 <v> <candidate binary on vibe5>` (stage the candidate app at vibe5 ~/tmp/qa/cand/noshape, one fixed path;
   delete it after); it waits for the release, checks, moves the compat baseline, writes one line to qa-alerts.
2. **qa-imp-verify is DONE** (09:27): green fa9f84b8 (core c85e75070) gives every c2-impeller-loft feature v0.4.56's status
   (ab1117/imp-verify.txt); IMPELLER-MOVE-FACE-2-REF marked verified. Nothing left to run for it.
3. Suites: the qa-suites loop's next run picks the newest green (4-hourly). Compare each finished run with the previous one
   (suites/<run>/worse.txt from bin/suites-diff.py: models, scenarios, `failure`, `speed`, `newdiff`; assemblies.log
   `^asm-…: … BOM rows` and `^assemblies:`; exchange.log `exchange score` and times over live). Regressions -> owners as P0
   (paused agents: to the lead), tickets the run confirms fixed -> mark verified.
4. Open P0s to re-check on the green that claims each fix: SCISSOR-LIFT-BOM-ROWS (culprit core 41f62884a, bool corrected; core has it:
   when it lands, the next qa-suites assemblies run must show asm-scissor-lift 59/87 BOM rows).
   IMPELLER-DELETE-FACE-2-SLOW reclassified 10:55: slower than live (~19 s vs 2.8 s), not a regression (v0.4.48 built other
   parts); core told. The knife Chamfer 2 / Fillet 5 / Fillet 3 "regressions" (v0.4.48 -> v0.4.53) were the same: blend's Mac
   A/B 10:40, v0.4.48 matched 20/52 knife features. Closed; they stay on blend's slower-than-live list. blend's impeller Fillet 3
   is WIP in blend10 (not landed): when it lands, re-time c2-impeller-loft v0.4.56 vs green on vibe6
   (bin/abn-feat.py 3 <cli,cli> ~/agents/qa-keep/abc/m <work> c2-impeller-loft; full paths, `~` after a comma isn't expanded).
5. Nightly (02:00, qa-w7-nightly; restarted 11:56 with two additions: live's fresh-copy times and the dock tab A/B, 0.4.53 vs
   the release, 3 rounds, Xvfb :672 -> nightly/tabab-<mmdd>.txt). On 11-18 send psui the tabab numbers: both ~100-200 ms settle
   at low load closes DOCK-TAB-SWITCH-TRAILING-FRAMES (psui: likely late software frames on a loaded host; vibe7 at load 17,
   0.4.53 818 ms too). After it, check suites/window-0200-<mmdd>.log ends NIGHTLY-DONE and the SCORE row is valid
   (the guard alerts otherwise); send blend faceblend / tpd / michael and skasm corner-break from the row.

## Open items
- **Speed (P0)**: owners got their slower-than-live documents 11-15 (blend, core, bool, bool2, dx, skasm; archive has the
  list). Re-list what is still slower after each valid nightly row.
- **Interaction latency**: absolute numbers only from the nightly (DECISIONS 11-15). Send owners only what the nightly
  confirms slow, or > 3x a PLAN 2.7 target. psui owns the hat15 fillet edit (Tab not applying a variable-bound Radius).
- dx: c2-rings-loft x_t import works since 0.4.56 but 9-10 s vs live 8.6 (sent to dx, queue item).
- bool: knife Mirror 1 128 / 220 / 140 ms (0.4.53 / 0.4.55 / 35baa32f): bool's 846aace6a says fixed; check in the next valid nightly.
- Agent ids changed 11-17 09:25 / 10:35 (psui 4973e295-9e92-463f-a13f-2ec5258bc5f9, asm d8d597e0-675c-497b-a804-544ceab99bbd, skasm 18a8ce79-5f2e-4089-9ad8-cc9d1182f0e6, bool2
  e98465f5-5cc9-4d4f-b67e-aae1d5e6d2e0, studios 58e9bca8-346c-4847-8012-e15006ae61ef, live e8549719-ef9a-42de-960c-e24b29f142a6,
  live2 79ae5cdb-42a4-4025-b43a-da61a4f92ad7 (11-17 13:2x); bool 7fa8ceab-4549-499f-9b01-3a289c2c95b4, core 6692ae35-758a-4442-81c8-29f67be2e21a,
  blend 0e6e9942-0713-46c1-ae55-132fbbb4a14b, dx f93a1292-5bb7-406f-9a25-59767e01878e). No qa script holds agent ids.
- QUIT-ABORT-DOCK: verified fixed (80e16e42d); guarded by the nightly and rel-published.

## Expected userdocs differences (not regressions)
- none against 0.4.56. hat15 vs 0.4.43 always DIFFs (Fan Riser +5.83 mm³, STEP reference 51188.66 -> .65): known.

## Tools
- userdocs.py <cliA> <tagA> <cliB> <tagB> <doc>...: fresh 600 copies in ~/tmp/qa/ud-<pid>, per Part Studio feature status /
  message and parts (faces, edges, vertices, volume); prints SAME / DIFF. Uses store/sig.py + bin/nscli.py.
- Latency (/root/qa-scratch/lat/hat): mkscen.py <out> <doc> <face x,y,z> <feature> <field> <value> <rounds> <filter>;
  split.py <name> [tabs] (one round per file: the runner's 180 s cap); lat-priv.sh <doc name> <scenario prefix> <open rounds>
  (APP, XD, DOC env; NOSHAPE_FRAME_LOG + NOSHAPE_PERF=1); lat-open.py <dir>; ../lat-steps.py <scenario> <frames log>.
  quitv.sh <runs> <scenario> <app>... (XD, SD env) for crash loops; quit scenarios qa-asm-a / qa-tabs / qa-edit.
- One feature's own result: bin/feat-at.sh <doc> <feature> <cli>... (on a vibe host: later features suppressed on a copy, status, ms, parts).
- A/B: bin/ab-feat.py (2 CLIs), bin/abn-feat.py <runs> <cli,cli,..> (n CLIs; full paths, no ~), bin/ab-sum.py <jsonl> [live-regen.json].
  Since 11-17 10:50 both write "parts_same" (each CLI's final parts on a fresh copy, outside the timed runs; scp sig.py + nscli.py
  beside them, else None). parts_same False = the builds model different geometry: no regression claim from such an A/B.
- Speed: mac/opentime.py, mac/modeltime.py (vibe7 copies in ~/agents/qa-keep), bin/nightly-sum.py, mac/live-regen.json.
- Live's times (live 11-17): bin/livetime.py reads <models>/<doc>/live.jsonl: the last "fresh-copy" line (median of 3
  fresh copies) else "open-sum" (no timing_kind); per_feature_ms by name or featureId, mapped to names. The nightly rebuilds
  mac/live-regen.json from it (its 84-doc set kept; .kinds beside it) and the SCORE row says "live: N of 84 fresh-copy";
  speed-gaps.py uses it too. live is re-timing the ~30 slowest documents, then the asm-* ("assembly-open-solved" lines,
  not read yet), so live's totals move: the knife 408 -> 622 ms. qa-w7-nightly restarted 11:29 to take it.
- Private documents (hat8, hat15, hat15b, dock in vibe5 / vibe7 ~/lead-private) only as mode-700 copies in ~/tmp/qa on a
  host, deleted after; never in the repo or tickets.

## Hosts
vibe5: land queue, compat loop (one CLI at a time), release builds via rb.sh (worktree qa-bisect). vibe6: ad-hoc app runs
(:661), release quit check. vibe7: suites (slot), fuzz :61, nightly :671. vibe3: watcher only. The Mac: off limits except
its single slot. Heavy runs through `~/bin/slot` (DECISIONS 11-15).
