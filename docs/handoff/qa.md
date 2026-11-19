# QA handoff (agent qa; current state only, updated 2026-11-19 07:40)

qa tests releases and greens against the last release and live, times speed, files tickets in docs/TICKETS.md, and never
edits code. Standing instructions: /root/lead-scratch/prompt-qa.md. Rulings: docs/DECISIONS.md (read first). History:
docs/handoff/archive/qa.md. Scripts: /root/qa-scratch/bin (local), results under /root/qa-scratch. Docs commits from
/root/projects/noshape-wt/qa (`git checkout -B agent/qa origin/main`, commit, `git push origin HEAD:main`).

## Running units
**Start every unit with `systemd-run --setenv=HOME=/root …`** (no HOME = gh fails; 11-17 the published check polled 25 min
after v0.4.57 was out). Scripts that call gh also `export HOME=${HOME:-/root}` themselves since 20:45.
Local units are transient (`systemctl list-units 'qa-*'`); restart one with the `systemd-run` line in its description if it's gone.
- **qa-compat-loop**: bin/compat-loop.sh <baseline>, now **0.4.57** (moved by qa-pub-0457 11-17 21:44). On every watcher green, bin/compat-check.sh runs userdocs.py
  on vibex7 (hats, dock, 47 ticket docs, 16 authored docs from ~/agents/qa-keep/compat-v0443) with the baseline's CLI against the green.
  A feature that gets worse writes compat/ALERT-<sha> and one line per feature to /tmp/lead/qa-alerts. Move the baseline by
  restarting the unit with the new version (rel-published.sh does it).
- **qa-w7-nightly**: bin/w7-nightly.sh. Sleeps to 02:00 UTC, then on vibex7 with the newest release (downloaded to vibex7
  ~/qa-rel/<v>/) runs: (1) opentime.py (hat8, hat15, hat15b, dock) and modeltime.py (84 suite docs), 3 runs, the previous release's CLI
  alternating with the current one (DECISIONS db4db21a0; the row shows the same-session sums and docs > 1.25x slower, details
  nightly/<mmdd>.json vs_prev); nightly-sum.py
  -> a row in SCORE.md "Nightly, vibex7 02:00" (pushed from the qa worktree); (2) the latency set (bin/../lat/hat/lat-priv.sh:
  hat15 / dock UI-script rounds + cold / reopen + rotate / pan / zoom, block baseline) -> nightly/lat-<mmdd>.txt; (3) the
  QUIT-ABORT-DOCK guard (qa-asm-a x5) -> nightly/quit-<mmdd>.txt. Failures and a crash append to /tmp/lead/qa-alerts. Log:
  suites/window-0200-<mmdd>.log. Skip a night: touch nightly/skip-<mmdd>. Runs alone (no slot: the window must be quiet).
  Restarted 11-17 07:31: the lead's $HOME fix of the CLI pair (11-17's row INVALID: `~` after a comma isn't expanded, 1 of
  84 timed) and a guard: < 90 % of the docs timed or < 6 opens -> alert, no SCORE row. Earlier, 03:33: the clean A/B step (nightly/ab-<mmdd>.args) and the latency step now
  `cd … || exit 1;` (the A/B's `cd … && ( load loop ) &` backgrounded the cd, ab-feat.py ran in ~ and the loop held the ssh).
- **qa-suites**: bin/qa-suites-loop.sh, every 4 h: interaction suite, model suite (speed, debug) and assembly / exchange
  suites of the newest green, each through `~/bin/slot`: ui + models on **vibex7**, assemblies / exchange (suites-rest) on
  **vibex6** beside them since 11-17 04:51 (vibex7's slot was held 4 h by dx). 1117-0227-ecf8c33a's stage re-run as unit
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
- vibex7 user units **qa-fzloop** + **qa-fz-xvfb** (`systemctl --user` there): the GUI fuzz, ~/qa/bin/fzloop.sh :61 on
  ~/qa/rel/noshape-green (green 35baa32f since 11-17 03:34; refresh by copying a newer app to that path), seeds from ~/qa/fzseed.next,
  results ~/qa/fz*-result.txt. fzrun.sh kills the app's own `--reopen` restarts after each run. Pause: touch
  ~/tmp/qa-fz-pause-61; it also idles 01:50-04:05 every night (the window runs alone). App refreshed to green f7c318a5 11-17 10:08.
  w7-quiet.sh end (local bin/ copy, scp'd each night) now recreates the transient unit with systemd-run when `start` fails:
  the stopped unit was gone, so the fuzz was down 11-17 02:01-10:08.

## Release checks (per release; lead cuts 05:00 / 17:00)
1. Candidate: bin/rel-cand.sh <prev> <tag> (checks out nothing: check out the candidate in worktree qa-bisect first; builds
   its CLI on vibex7 through rb.sh, then userdocs vs <prev>: user docs + f197, the 4 Hole docs, tickets, authored docs) ->
   compat/cand-<tag>.txt. Reply go / no-go. A user-document regression is reverted on the branch, never shipped.
2. Published: bin/rel-published.sh <v> <candidate CLI on vibex7> -> compat/published-<v>.txt: SHA256SUMS of the Linux
   assets, CLI installed as vibex7 ~/qa-rel/<v>/noshape-cli, userdocs candidate vs released (must be ALL-SAME), qa-asm-a x5
   on the released app (vibex6, slot), then the compat baseline moved. Then bin/rel-compat.sh <prev> <v> (released CLI vs
   the previous release) -> compat/rel-<v>.txt; any user-document worse = P0 to the lead at once.
3. The app spot-check rows (archive "Release checks: the app spot-check list"): user cases + 8 interaction-feel rows in
   rotation (next: rows 17-24).
4. Hover flow (lead 11-17 22:20, every release): uh/uh2.sh <release app> <out> on vibex6 Xvfb, 3 runs, xsheet.py for the sheet;
   USER-HOVER-STAYS-LIT is listed as known in the cut's entry until psui's latest-wins picking fix passes 3 clean runs.
- **v0.4.57** published check done 11-17 21:44: clean, baseline 0.4.57. Next: the 05:00 cut (qa-cut-0415).

## Next (in this order)
00b. **Night order (lead 11-19 03:45)**: qa-night-seq-1120 (flow/night-seq.sh latest 1120): 02:00 big document -> session replay ->
   everyday flows, then nightly/flows-done-<mmdd>; qa-w7-nightly waits for that flag (03:20 at the latest) before hat15 / dock;
   user flows after NIGHTLY-DONE. Re-arm each night with the next date. Cut verdict: a must-carry still queued reads PENDING
   at :15 and HOLD only if missing at :50 (timers qa-cut-0450 / 1650). qa-big-after fetches tonight's big run and fixes its fetch line.
00. **11-19 07:40 state.** v0.4.60 cut GO (fafc4a25, 04:42); cherry-picks c6b003415 + 5e04baac1 OK'd. Spur gear verified 07:28 (vibe2,
   7 alternating: 1.03x v0.4.58; speed/sg-ab-vibe2.sh, ab-feat2.py from bin/). Pending: qa-dx-prewarm (heavy/dx-prewarm.sh): when dx's
   f1e0c74e lands (by subject), release apps of it + parent on vibex7, asm-xdev-subs first tab x3 alternating on local :154 -> P0 to psui +
   dx if >= 20 % and >= 100 ms slower. Session replays (vibe2, ~/tmp/qa/uip.lock serialises runners: the saved copy is the newest
   /tmp/noshape-uip-*): session-1118 = live 04:23; session-sm-1119 (smsess/: mksm.py, smsess.sh; CLI fills Relief / Configure features /
   cells) = live geometry, skasm's bends + flat fixed 07:35; session-drw-1119 (drwsess/: d2.py, drw.sh, start drw-1119 in its starts.json)
   = live, open ≠: DXF LINE/MTEXT counts (dx), new assembly drawing tab place (studios asked live). Kept: vibe2 ~/agents/qa-keep/session-*.
   Studios "to verify" feel tickets all [x] incl. DRW-TOOLBAR-TOOLTIP-DELAY (live2 retake). Owed on studios 166b521c's green: asm session
   BOM / callout, studio session Tab / Checkbox dialog / merge+restore with B1 open. Core told: c2-surf-workshop cpcurves Thicken 1 Ok ->
   Warning, Split 6 / mutualtrim Fillet 1 messages (corpus, compat ALERT 814b48d8 / daa1bebd). cut-check: green re-read under the lock,
   30 min cap then the newest finished check (named in the line).
000. **11-19 12:30 state.** Ids: studios 31c9b9ba-e7f8-40f7-85cb-242bab86dbe6, dx aad5e825-f86c-432d-b4ac-3aab402e7052 (Motile);
   live stopped till ~21:00. v0.4.60 post-release done 10:59: clean (SHA, user docs / authored = 0.4.59, session-1118 + sm-1119 on the
   released app = live; compat baseline 0.4.60); f197 Part 1 +1 edge +1 vertex same volume -> core (low). ps2 = live since core's
   7bf5efe3 (verified 12:05). dx prewarm A/B: no regression. DXF on a rerun drawing session = live. In flight:
   Done 12:40: cpcurves / mutualtrim re-saved (bin/resave-corpus.sh via rb.sh on vibe2, models in ~/tmp/qa/resave) into vibex7
   ~/qa/corpus (old: ~/agents/qa-keep/*.bak-1118); cpcurves Split 5 Info + parts 4 vs live 3 -> core; mutualtrim Fillet 1 -> blend.
   compat/accepted.txt: those two + cli-shell-hang-pre Shell 3/5 + cli-draft-hang Fillet 3 (blend 92c8595b0, lead 11-19).
   TICKETS P2 SHELL-AFTER-WELD-FILLET (bool; live check owed when live has room). f197 +1 edge closed by core (DECISIONS 11-16).
   Studios' merge fix 258b8d40e verified 12:05 (m1 retake); only ≠ the tab rows' order -> studios.
   Kept vibe2 ~/agents/qa-keep/{session-ps2-1119,studio-1119-recheck}.
0a. **Validity (lead 11-17 ruling)**: one source, blend's cfd943d5 (queued; bool's invalid_solids withdrawn, don't read it):
   with NOSHAPE_PARITY_VALIDITY=1 (bin/suites-models.sh exports it) models.log prints per feature "CHECK <doc> feature <k>:
   <part> valid <true|false> raw f/e/v; ..." (PartState.check, query parts' "check"; not in report.json). suites-diff.py
   (18:40) prints "invalid <doc> › feature <k>: <part> valid false" for a part false in the new run and not in the previous
   (only when both logs have CHECK lines); qa-suites.sh copies those to qa-alerts. bool's gate 2d5bf386 does the same.
   Compat: v0.4.56's CLI query parts has no "check"; once cfd943d5 is in a release CLI, add it to sig.py's part tuple.
   (suites-models.sh was moved to bin/retired by mistake at 16:35 and restored 18:28; the hosts kept their copy meanwhile.)
0n. **Speed report reference**: live's UI speed of everyday actions (behaviours "psui + qa (lead 11-18): UI speed of everyday
   actions", live/psui/ui-speed-1118/table.md): open document 12231 ms settled, tab switch 2291-2733, enter sketch 1291 (dialog 226),
   exit 889, undo 903-1011, open Extrude 1320 (dialog 221), first preview 2004, commit 1115, rollback drop 918-1094, context menu 117,
   Mass dialog 161. Use it with the nightly / release-build timings.
0t. **Compat now covers the corpus (06:40)**: compat-check.sh section "corpus models" = vibex7 ~/qa/corpus/*.noshape (127 saved by the
   models suite; refresh from vibex6 ~/qa-parity/saved after a models run). cut-verdict lists corpus changes with the tickets (read
   them: a status that gets worse is a HOLD unless accepted). cut-must.txt: fff088e5, 120ee19c, Ctrl+Enter fix when it lands.
   Live's selection latency with qa's method (behaviours "qa (lead 11-18): live's selection / hover latency"): face 964 / plane 955 /
   row 693 / box 15321 ms screen medians (software); ours 216 / 153 / 163 / 218: ours faster on every row.
   feat3 done 06:32: features > 5x live routed (blend 12, core 11, bool 5 rows; messages 06:40).
0u. **Feel vs live2 11-18 (06:55)**: TICKETS "Interaction feel vs live2's 11-18 06:0x numbers": P0 FEEL-FL-AUTOSCROLL-DROP, P1
   FEEL-SKETCH-TOOL-PICK, P1 FEEL-FAST-OFF-CLEAR (psui). Tools qa-scratch/hv2 (hovt/hovd/hoff/hsk.py on vibex8 ~/tmp/qa/fl via fls.sh;
   docs skr / fl45 in vibex8 ~/tmp/qa/fl/docs). cut-must.txt now also 7b8bb249 + 2549be00 (Ctrl+Enter).
0v. **Feel: assemblies / drawings / list (07:10)**: TICKETS "Interaction feel in assemblies, drawings and the feature list": asm P1
   SNAPMODE-REACH (70 vs 57 px), P1 SNAPMODE-RELEASE-DIALOG; psui P1 ROW-DBLCLICK-DISTANCE. Runner-built docs reopened without the
   runner (vibex8 ~/tmp/qa/fl/docs: asm-feel, hole-plate-drawing): stable views. Drivers qa-scratch/af/snapm2.py, hv2/rowk.py, hv2/drw.py.
   qa-wait-33c5 re-runs FEEL-FL-AUTOSCROLL-DROP on psui's 33c57541 green.
0z3. **live2 fills interaction-feel's "Ours ?" cells on P0 rows (lead 11-18 ~12:15)**: done (2e8057c34): hover-p0 sketch reach outside
   sketch mode, hover-pick-1120 edge vs face, sel-feel press + move over a default plane, measure-mass-1118 place, ps-feel2-1118
   Y / Shift+Y. Next (live2's): manip-feel-1118 triad, other ps-feel2 rows, chips-expr-1118, field-keys-1118, asm, studios. Don't redo
   them; ticket any "Ours != live" row live2 reports that has no ticket yet.
   Ticketed 12:2x (batch 1: 5 rows, 9434d439a) and 12:3x (batch 2: 7 rows, 2ead983df). psui 12:4x: FILLET-DIALOG-WIDTH and the
   MASS place came from /root's shared config (live2 retakes with a fresh NOSHAPE_CONFIG_DIR; close FILLET at 220); PLANE-FIRST-MOVE
   and HIDE-TIMING fixed in psui's land queue: re-run both on their green (or take live2's retake).
   13:2x-13:31: batches 7-8 ticketed (DRW-TOOLBAR-TOOLTIP-DELAY studios; ROLLBACK-DROP-INPLACE, MASS-VALUES-DELAY psui); rows 1088 / 1089
   filled by qa (c563b91c4: wf/mkspd.py builds live's speed document, wf/spd.sh + spdlat.py on :153; ours faster). live2: every
   UI-measurable "?" row done; the rest are client constants for owners' tests. live2 re-measures 1313 + hover-p0 on skasm's green.
   Batch 3 (manip-feel 981-983) ticketed; live2 next: view-tools 992/993/995, field-keys 1309-1313, pattern-hole 1268-1280 (skip them).
0z2. **P0 HEAVY-ASM-SOFTWARE-FRAME (lead 11:45; asm leads, psui later)**: xDev frames render in 46.7-49.4 s on llvmpipe (live 3.4 s
   per zoom on SwiftShader); 84% llvmpipe in the profile. Repro + data sent 12:00 (TICKETS; qa-scratch/heavy perf1, prof1). Re-run
   runperf.sh / probe.py (screen timings; the frame log's presents are UI frames only) on asm's fix. Driven-pattern rows wait for core's
   DRIVEN-PATTERN-SWEEP (live's 723k-triangle solid is missing in ours).
   asm 12:2x: cause = the angle-based display mesh (7.76 M triangles, 3 bodies blow up; a chord mesh is 1.05 M, render 15-18 s ->
   2.95 s headless), handed to psui. Re-run heavy/runperf.sh + probe.py on psui's mesh fix. Snap reach: asm's live2-retake rule (51 px
   box of the face under the pointer) queued; HOLD (lead 12:28: asm pulls / redoes e2c20cb2, which froze off-part): qa-wait-reach stopped;
   start af/wait-reach.sh again only when asm says (it re-runs up / 45 deg on the first green with an asm
   snap/reach commit after 12:20 (live 50-51 / 72 px).
0z. **Heavy-model feel (lead 09:10)**: live measures c2-loft-guide-curves-driven-pattern (Part Studio, live 125 bodies; ours 124, Sweep 1
   fails: P1 DRIVEN-PATTERN-SWEEP core) + asm-xdev-subs (ours only exists in the assembly suite's scratch: not prepared). Ours:
   qa-scratch/heavy (heavy.py: live's moves / offsets / 600x400 poll at 140 ms + NOSHAPE_FRAME_LOG; hvs.sh on :154 1920x1080 with
   vibe2's release build of 80a64725 at ~/tmp/qa/hv/noshape-rel; run.sh as unit qa-heavy). First run r1-0914 (host load 22, live's
   Chrome running): orbit first 123 ms, 8 fps; pan 7 fps; zoom / fit / Shift+5 / Shift+7 last frame 793 / 829 / 722 / 724 ms; list
   scroll 8 fps. The model is long and thin: at fit its surfaces are sub-pixel, so screen polls and the hover point find nothing
   (same for live, probably). Re-run right after live's numbers land, then compare row by row; != -> psui.
0y. **Dock tab return P0 (lead 09:00)**: fff088e5 didn't fix v0.4.54's regression (09:01 same-run: v0.4.53 settle A / B-again
   137 / 189 ms, release build of 80a64725 829 / 813; speed/tab-80a64725.txt, sent to psui). bin/tabrel.sh <sha> [nobuild]
   (release build on vibe2, tabab vs v0.4.53 on vibex7, lock speed/tabrel.lock); unit qa-tabwatch runs it on the green of every
   psui commit whose subject mentions a tab / hover card (seen list speed/tabwatch.seen). cut-must.txt has a "pending:DOCK-TAB-RETURN"
   line (always HOLD): replace it with subject:<text>|<label> once a candidate brings settle to v0.4.53's. qa-wait-67eb retired.
   rel-pub-all.sh's alert now names each compat section.
   12:10: psui's fix is 9595bbf05 (handoff 182c0efc is skipped now). psui: the frame pair ~600-800 ms after each click is the UI
   driver's state-read redraw (all builds), so "settle" flips 200 / 800 ms; tabrel now reports psui's "tab drawn (2nd frame)" and the
   last frame over 10 returns (5 rounds), and lat-steps.py ends settle at the first "driver" event (logged once psui's 3de2cd04 lands;
   v0.4.53 has none, so compare the 2nd frame across builds). psui's A/B: 2nd frame fix 224 / 0.4.53 218 / green 321 ms.
0x. **Speed sweep (lead 07:45, done 08:53)**: bin/speed-sweep.sh <sha> (SKIP_BUILD=1 reuses vibe2 ~/agents/qa-speed/target, 7.9 GB):
   release build on vibe2, corpus A/B there + user docs / dock tab on vibex7, ab-feat2.py two passes, bin/speed-sum.py <dir> pass|ud.
   80a64725 vs v0.4.57: no regression (TICKETS 364fac560). A/Bs ran outside the slots at nice 10 (both slots queued ~2 h).
   v0.4.58 published check 08:28-08:4x: SHA OK, user docs SAME, quit 5/5, baseline -> 0.4.58.
0w. **Workflow feel vs live (07:38)**: TICKETS "Workflow feel vs live's 11-18 07:09-07:22 timings": ours faster on every row, no
   tickets (qa-scratch/wf: wflat.py on this host's Xvfb :153, two-blocks reopened without the runner, PIL crops padded to live's
   139 ms, timestamps at capture). FEEL-FL-AUTOSCROLL-DROP verified 07:21 (b5c4ccc4), psui told. psui's next land has ROW-DBLCLICK.
   Compat upload fixed 07:45: compat-check.sh snapshots the green, scp to noshape.new + mv (an scp onto the busy binary failed
   and cfc031fa's run used cafe3c2a's app: its file is renamed cfc031fa.stale-binary.txt when done). qa-wait-67eb's vibex7 load
   check was never true (ssh quoting); fixed 07:30, still waits for load < 2.
0b. **LOADED runs (lead 21:50)**: speed / timing at load > 16 (2 x 8 cores) don't count. suites-diff.py prints "LOADED: …" first,
   skips speed lines and tags timing scenarios "(LOADED)"; qa-suites.sh touches <run>/LOADED and alerts. Absolute speed only from
   the 02:00-04:00 nightly. The 21:03 run (vibex7, load ~60) and 11:19 run (load 19) are LOADED.
1. **17:00 cut (11-18): GO 16:40 on green 515102325 -> v0.4.59** (TICKETS 1f4521120; lead: ship, fix FORM-PREVIEW-SLOWER forward, skasm's
   app A/B ~17:40). qa-wait-5151 (bin/wait-compat.sh) alerts when 51510232's ticket / authored / corpus sections finish (new DIFF vs the
   known five -> lead at once). Impeller: core's 27b7836e0 lost the main body at feature 47 (bin/walk-parts.py, p47.py, bis47.sh: release
   CLIs on vibe2 -> vibex7 ~/tmp/qa/apps/noshape-bis); reverted 125e4d410. After the publish: `systemd-run --unit qa-pub-0459 -E HOME=/root
   -E PATH=$PATH bash /root/qa-scratch/bin/rel-pub-all.sh 0.4.58 0.4.59 /home/ngyekta/qa-speed/bin/noshape` (the compat app = 51510232;
   check it still is), then the spot-check rows 17-24 and the hover flow. cut-must.txt: clear it for the 05:00 cut.
2. **User-flow timings (lead 18:00)**: qa-scratch/flow: mkflow.py (block: sketch on a face / Fillet / Extrude; two-blocks: tab +
   Fastened mate; block-drawing-empty: Insert view, base + projected placed; one file per round, the runner stops at 180 s), flow.sh
   (frame log + lat-steps.py, flowsum.py). Unit qa-flow-night runs it in the 02:00 window after NIGHTLY-DONE on v0.4.59 (vibex7 :673)
   -> qa-alerts. Live's Fillet / mate / drawing numbers asked of live 17:5x. DECISIONS 11-18 (user): a gap counts at >= 100 ms and
   >= 2x on a whole user action / rebuild; a regression >= 20 % and >= 100 ms, A/B-confirmed; report spread + load; feel rows vs live.
   Waiters: qa-wait-c031 (sketch reach), qa-heavy-wait (asm-xdev-subs v0.4.59 vs ce832447d's green), qa-pub-0459. Main RED 17:53 (psui).
3. Suites: the qa-suites loop's next run picks the newest green (4-hourly). Compare each finished run with the previous one
   (suites/<run>/worse.txt from bin/suites-diff.py: models, scenarios, `failure`, `speed`, `newdiff`; assemblies.log
   `^asm-…: … BOM rows` and `^assemblies:`; exchange.log `exchange score` and times over live). Regressions -> owners as P0
   (paused agents: to the lead), tickets the run confirms fixed -> mark verified.
4. Open P0s to re-check on the green that claims each fix: SCISSOR-LIFT-BOM-ROWS (41f62884a shipped in v0.4.56; 17:00 must carry
   core's fix: unit qa-sl-verify (sl/sl-verify.sh) waits for it on green, runs scissor-lift alone, -> qa-alerts + sl/run.txt;
   when it lands, the next qa-suites assemblies run must show asm-scissor-lift 59/87 BOM rows).
   IMPELLER-DELETE-FACE-2-SLOW reclassified 10:55: slower than live (~19 s vs 2.8 s), not a regression (v0.4.48 built other
   parts); core told. The knife Chamfer 2 / Fillet 5 / Fillet 3 "regressions" (v0.4.48 -> v0.4.53) were the same: blend's Mac
   A/B 10:40, v0.4.48 matched 20/52 knife features. Closed; they stay on blend's slower-than-live list. blend's impeller Fillet 3
   is WIP in blend10 (not landed): when it lands, re-time c2-impeller-loft v0.4.56 vs green on vibex6
   (bin/abn-feat.py 3 <cli,cli> ~/agents/qa-keep/abc/m <work> c2-impeller-loft; full paths, `~` after a comma isn't expanded).
5. Nightly (02:00, qa-w7-nightly; restarted 11:56 with two additions: live's fresh-copy times and the dock tab A/B, 0.4.53 vs
   the release, 3 rounds, Xvfb :672 -> nightly/tabab-<mmdd>.txt). On 11-18 send psui the tabab numbers: both ~100-200 ms settle
   at low load closes DOCK-TAB-SWITCH-TRAILING-FRAMES (psui: likely late software frames on a loaded host; vibex7 at load 17,
   0.4.53 818 ms too). After it, check suites/window-0200-<mmdd>.log ends NIGHTLY-DONE and the SCORE row is valid
   (the guard alerts otherwise); send blend faceblend / tpd / michael and skasm corner-break from the row.

## Open items
- **Speed (P0)**: owners got their slower-than-live documents 11-15 (blend, core, bool, bool2, dx, skasm; archive has the
  list). Re-list what is still slower after each valid nightly row.
- **Speed rule**: DECISIONS 9e5b481d4 (>= 100 ms and >= 2x on a user action; P0 = >= 20 % and >= 100 ms, A/B >= 5 runs on one host).
- Agent ids changed 11-17 09:25 / 10:35 (psui 4973e295-9e92-463f-a13f-2ec5258bc5f9, asm d8d597e0-675c-497b-a804-544ceab99bbd, skasm 18a8ce79-5f2e-4089-9ad8-cc9d1182f0e6, bool2
  e98465f5-5cc9-4d4f-b67e-aae1d5e6d2e0, studios 58e9bca8-346c-4847-8012-e15006ae61ef, live e8549719-ef9a-42de-960c-e24b29f142a6,
  live2 79ae5cdb-42a4-4025-b43a-da61a4f92ad7 (11-17 13:2x); bool 7fa8ceab-4549-499f-9b01-3a289c2c95b4, core d30002ca-d013-4435-b7d9-7da9cb564cec (11-18 20:0x),
  blend de66e481-56e2-4005-a5d3-9fa897ca5fde (11-18 20:0x), dx 2108c2b7-2f48-43a7-87a0-3f652e3cda6d (11-17 18:4x)). No qa script holds agent ids.

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
- Speed: mac/opentime.py, mac/modeltime.py (vibex7 copies in ~/agents/qa-keep), bin/nightly-sum.py, mac/live-regen.json.
- Live's times (live 11-17): bin/livetime.py reads <models>/<doc>/live.jsonl: the last "fresh-copy" line (median of 3
  fresh copies) else "open-sum" (no timing_kind); per_feature_ms by name or featureId, mapped to names. The nightly rebuilds
  mac/live-regen.json from it (its 84-doc set kept; .kinds beside it) and the SCORE row says "live: N of 84 fresh-copy";
  speed-gaps.py uses it too. live is re-timing the ~30 slowest documents, so live's totals move: the knife 408 -> 622 ms.
  livetime.asm_open(<doc dir>): live's "assembly-open-solved" (open_solved_ms, the assembly's tab clicked in the open
  document until the Instances (N) count is final; 5 of 24 at 14:20, e.g. corexy 6125 ms). Ours isn't timed yet: the
  assembly suite saves no documents (NOSHAPE_PARITY_SAVE is Part Studios only); decision asked of the lead 14:20. qa-w7-nightly restarted 11:29 to take it.
- Private documents (hat8, hat15, hat15b, dock in vibe5 / vibex7 ~/lead-private) only as mode-700 copies in ~/tmp/qa on a
  host, deleted after; never in the repo or tickets.

## Hosts
**11-18 13:15 (lead): suite stages take the slot per step (suites-rest.sh: the build, each assembly, the exchange suite; suites-models.sh:
the build and the run), never for a whole stage (it held vibex8's slot 10:26-13:10). vibe2 OOM'd at 12:35 (30 GB): one qa build there at
a time (tabrel.sh builds with 4 jobs). Run 1118-0505's assembly stage re-runs per assembly (unit qa-rest-redo2, then the loop restarts).**
**11-18 08:00 (lead): ui-parity suite stages never on vibex7 (core / blend's build host; my shards held its slot 05:06-08:00):
vibex8 (<= 3 apps, slot) or vibe2. qa-suites.sh UH defaults to vibex8 now. vibex7 keeps the 02:00-04:00 window, compat and
the private-document runs. Binary uploads go to <path>.new + mv (an scp onto a busy binary fails).**
**11-17 21:23: land queue on vibe2 (user's server; `NOSHAPE_LAND_QUEUE_HOST=vibe2 tools/tiers/land.sh`); vibex7 drains its
queued changes. land-footer lands through vibe2. No suites on vibe2. Private docs stay on vibex7 ~/lead-private.**
**11-17 17:45: hosts renamed (user): vibex6 (watcher), vibex7 (land queue, ~/lead-private: hat8, hat15, hat15b, the dock),
vibex8; vibeN names are the user's own servers. qa's scripts, units and this handoff use vibexN.**
**11-17 17:20: vibe5 gone for good (lead), vibe3 gone; no qa script names either (the lock is qa-scratch/qa-host.lock).
vibex7 ~/lead-private has all four user documents (hat8, hat15, hat15b, the dock). One-off vibe5 scripts in bin/retired; the
local lock is still named vibe5.lock. Older host notes: archive. Heavy runs through `~/bin/slot` (DECISIONS 11-15).
