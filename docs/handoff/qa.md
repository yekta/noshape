# QA handoff (agent qa; current state only, updated 2026-11-20 09:32)

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
- **qa-index-daily** (lead 11-20): bin/index-daily.sh "<since>": every 30 min, the first full suite run of the day (suites/<run>/index.json +
  a models score) -> one "Parity index" row in docs/parity/SCORE.md (bin/index-row.py; header order = index.json's areas), pushed, and a
  qa-alerts line. Started 07:3x with since "2026-11-20 07:30" (today's row from suite 1120-0407-81d23c9b or later).
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
   rotation. Round 2 (lead 11-20 01:00): rows 1-8 on green eef639d7:
   scenarios 7/8, 63/67 (suites/1120-0059-green-spot1r2): sketch-dimension-variable = P0 SKETCH-DIM-PLACE-CLICK-SELECTS (skasm,
   since 592b0fc9 11-17; hidden as a focus.kind diff till live's camera retake). By hand: 3D drag from 3 device px = live
   (feel/3d.json; the list's 3D "5 px" fixed to 3 px; feel row 80 (sketch drag 5 px) stands), line end on a lit model edge = Coincident (=), edge midpoint
   rest = P1 EDGE-MIDPOINT-REST-LOOK (psui). Rows 9-16: 8/8 119/119 (1120-0109-green-spot9r2) + select by hand =; rows
   17-22: 13/13 130/130 (1120-0110-green-spot17r2); 23 sticky snap = scene test; 24 hat15b at the release; sketch colours
   (line / dimensioned / held circle / spline) by hand on 5b62f6ca =. On 5b62f6ca (vibe2): rows 25-40 (spot2-0461 + spot33
   names) 15/15 260/260 (1120-0128-green-spot25r2); rows 41-56 drawings + documents 16/16 67/67 (1120-0116-green-spot41r2).
   Round 2 done 01:20; only failure the P0 sketch-dimension-variable (skasm). STALE-SKETCH-HOVER verified a933e1b7 01:28. Suite
   1119-1302 diffed (suites-diff vs 1119-0551; LOADED): only P2 SUITE-INTRO-MUTUALTRIM-DERIVED (core); exchange 117/130. skasm stack queued 01:5x (94f6df34): waiter qa-wait-skasm-dim
   (first skasm commit since 01:53; check the green has 94f6df34's subjects). Recheck: sketch-dimension-variable (uip-rerun +
   by hand), OFFSET-EDIT-SCALES-SKETCH (free circle, O, Enter, 3: no scaling), SMM-EXTRUDE-CLOSED-PROFILE, SKETCH-SLOT-ON-ARC,
   SKETCH-DXF-EXPORT-POLYLINES (R14, LINE/CIRCLE/ARC/SPLINE on MODELSKETCH_VISIBLE, no construction / points). Night ui suite started 02:00:01 on vibex8 (a06ec927);
   vibex8 ~/.slot-quiet until 04:00 (timer qa-vibex8-unquiet removes it); core's .dbg.sh + skasm's uip4 build queued there, told.
   Lead 02:0x: once 69bcf1791 is green (waiter qa-wait-studios-drwdlg) spot 5 drawing dialogs vs live/drawing 1047-lnk-front-props,
   1037-lnk-move-to-sheet-dialog, 710-hole-table-dialog, 508-datum-target-tab, 1358-dk-export-default (all 1919x1079); dx STEP
   torus export (qa-wait-dx-torus) and hex-bookcase DXF counts vs live (qa-wait-dx-hexdxf; dx ba3d6cc1 determinism land). Pre-release 05:00 (lead 02:0x): GO 02:25 on a06ec927, vibe2
   ~/tmp/qa/prerel (700; bin/prerel-edits.py, prerel-open.sh, prerel-appedit.sh; out qa-scratch/prerel): userdocs ALL-SAME,
   edits SAME (hats 2 vars, dock 4), app open + hat15 edit pixel-identical to 0.4.61. A newer cut green: qa-cut-0415. Lead 02:3x: verify TICKETS rows marked fixed not verified
   (P0/P1 + feel first), close / reopen each; after the night suite, each new difference to its owner. Done 02:3x: ASM-NEW-MATE-
   PREVIEW-GROUP, ASM-UI-SIGSEGV-AT-EXIT, SKETCH-SCREEN-SPACE-REACH verified, SHELL-VOIDS-WINDOWS closed, 2 dup asm rows dropped
   (6e2c672c0, 06913c748). ~73 old P0/P1 rows (10-27..11-08) being triaged (tests still present / suite results), then P2/P3
   recent rows. psui asked 02:35 to keep builds off vibex8 (its kernel build ran beside the night suite). Compat: P1 SINGULARITIES-SKETCH2-UNSOLVED -> core (20ae6a506; core 4a52a26bb).
4. Hover flow (lead 11-17 22:20, every release): uh/uh2.sh <release app> <out> on vibex6 Xvfb, 3 runs, xsheet.py for the sheet;
   USER-HOVER-STAYS-LIT is listed as known in the cut's entry until psui's latest-wins picking fix passes 3 clean runs.

## Next (in this order)
00c. asm 04:5x: re-walk FEEL-ASM-DRAG-DEAD-ZONE (real-pointer 100 px drag, read a part edge, not the hover outline) + MATE-PREVIEW-COLOUR once asm 5bfaa556 is green (qa-wait-asm-drag); DRAG-START stays 15 px, no 0.6 s (lead: server latency).
00b. **Live's 21 feel rows (11-20, lead 04:1x) DONE 04:3x** on green 9ae062180: 18 P0 feel tickets, all routed (rows FEEL-* in TICKETS
   "qa, 2026-11-20 (live's feel coverage rows …)": 15e785522 sketch skasm 3 / psui 1, e79906b34 studios 8, 51d079b3c asm 3 / skasm 1 / psui 2).
   Sketch rows ran as live's recordings (qa-bisect, uncommitted, suites/1120-feel-cov2; R4 inconclusive per live). Crops /root/qa-scratch/feel1120/.
   Quiet-window re-time (02:00-04:00 11-21, vibex8 was at load 21-36): D3 drawing wheel zoom (ours 208-392 ms, live 8-23), V1 cell edit
   (150-200 vs 46), D1 release (470 vs 10 / 767), A1 per-move draw, A2 preview draws, S1 per-move draw.
00e. skasm stack on 3fd4db71: 5 verified; FEEL-DIM-TWO-POINTS-ZERO / CHAIN-LASTPOINT / PASTE-PLACE still open. ANALYSIS-READOUT-HIDDEN
   verified 05:4x (2b3887ebe, 7/7); FEEL-DIM-PICK-READOUT (cov2-dim-orient step 8), ROLLBACK-DROP-ROW, SECTION-ARROW-FOOT: psui's next batch, on its word.
00d. 05:00 cut GO (2a43b78b) -> v0.4.62 (published, lead 08:1x). 09:2x: studios 9/10, asm DEAD-ZONE verified, MATE-PREVIEW-COLOUR
   reopened; SAVED-DOC-REFS-LOST (skasm revert 83423fa5) verified 5ca8588b0; dx 0036 snap-hook DXF verified b2b0cf9a9 (bin/snapdxf.py on
   vibex7: saved suite doc -> drawing placed as live -> DXF 2018 -> dxfdiff). Seat lip: Ok in every release (not a ticket). OPEN:
   - blend car Fillet 1: bin/car-one.sh queued on vibex7's slot (09:16 third) at green bad5d2ecc (has 6b6b3b4a + dx's translator 8ba897c06;
     the night suite 81d23c9b lacked 8ba897c06: Derived 1 220 faces vs live 7, inconclusive) -> feel1120/car-one.log.
   - blend CHAMFER-ASYM-NO-PROPAGATION (5a7f9ba8e): unit qa-wait-chamfer-asym; walk on live's G7 block (40x20x10, 4 vertical R3; 1 mm
     equal chamfer of the front straight edge 7904.485): distance 1 + 45 deg = 7904.485, front + back 7886.227, two distances stop at the edge.
   - asm DRAG-START: unit qa-wait-asm-drag2 (48a2b02a6: 2 px / 50 ms). Parity index row: unit qa-index-daily.
   - psui 37741d38 (unit qa-wait-psui-37741): DIM-PICK-READOUT (cov2-dim-orient step 8), ROLLBACK-DROP-ROW, SECTION-ARROW-FOOT,
     EDGE-MIDPOINT-REST-LOOK, HOVER-LEAVES-WINDOW, FAST-OFF-CLEAR, MEASURE-CIRCLE-DEFAULT-PLANE (rows say what to check).
   - Seat lip: a47534be5 (Ok in 0.4.43-0.4.61 + green; vibex7 ~/agents/qa-keep/seatlip-hat8-1120.txt).
00a. **05:00 cut HELD (lead 04:0x) until a green has core's 2f4f3bc5 (singularities Sketch 2; Mirror 2 is as live, accepted in
   compat/accepted.txt; 2f4f's subject added to cut-must.txt).** Unit qa-prerel-2f4f (bin/prerel-when.sh "<subject>" cut0500): when
   that commit is in the green, bin/prerel-run.sh uploads it to vibe2 and runs userdocs + prerel-edits + prerel-open (pixel diff at 90 s)
   -> qa-scratch/prerel/cut0500-<sha>.txt and a qa-alerts line. Then read that + the compat loop's compat/<sha>.txt and send the lead GO.
   TICKETS sweep done (f79c29aca and after): ~200 fixed rows verified / closed, 2 reopened (blend CHAMFER-ASYM, skasm SNAP-INSERT-FACE): no owner commit by 04:08, both owners told 04:09; walk on their word. Night ui suite done 04:06 (2145/2172); last "not run" (vs-ps-own-table-menu) matched alone: all routed.
   Night ui run (1120-night-ui-a06ec927, vibex8) finishing: sent P1 ANALYSIS-READOUT-HIDDEN (psui, fix in its next land; waiter
   qa-wait-psui-readout), P2 ASM-INSTANCE-PROPS-FIELD (asm), DOCS-CRUMB-TOOLTIP and VER-COMPARE-READOUT (studios); re-run-alone
   matches were artefacts. When score.log lands: diff vs 1119-1302 for the last ~40, route, then restart qa-suites (rm suites/stop).
   P2 T2-NIGHTLY-22-FAILURES filed for the lead. Speed left: FORM-PREVIEW gap vs live ~1.5 s on a quiet host.
00b. **Night order (lead 11-19 03:45)**: qa-night-seq-1120 (flow/night-seq.sh latest 1120): 02:00 big document -> session replay ->
   everyday flows, then nightly/flows-done-<mmdd>; qa-w7-nightly waits for that flag (03:20 at the latest) before hat15 / dock;
   user flows after NIGHTLY-DONE. Re-arm each night with the next date. Cut verdict: a must-carry still queued reads PENDING
   at :15 and HOLD only if missing at :50 (timers qa-cut-0450 / 1650). qa-big-after fetches tonight's big run and fixes its fetch line.
0000. **Standing (from the 11-19 21:38 item; its history moved to archive "## 11-20 moved (02:4x)")**: agent ids only from
   /root/lead-scratch/roster.md. Walk apps: `xdotool windowfocus` FIRST (else Tab never previews); drawings have no alt+c. tabrel.sh:
   vibe2 slot + 1 retry on SIGTERM. Night ui run: bin/ui-night.sh (timer qa-night-ui, worktree qa-night, vibex8 slot); after it
   restart the suites loop (rm suites/stop; systemd-run qa-suites). bin/uip-rerun.sh needs ABSOLUTE names paths (uip-rerun-0461.sh
   = on the 0.4.61 app); systemd-run units have no cwd.
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
0z. Heavy-model feel (lead 09:10; c2-loft-guide-curves-driven-pattern r1-0914): full notes in archive "11-20 moved"; re-run when live's numbers land.
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
0b. **LOADED runs (lead 21:50)**: speed / timing at load > 16 (2 x 8 cores) don't count. suites-diff.py prints "LOADED: …" first,
   skips speed lines and tags timing scenarios "(LOADED)"; qa-suites.sh touches <run>/LOADED and alerts. Absolute speed only from
   the 02:00-04:00 nightly. The 21:03 run (vibex7, load ~60) and 11:19 run (load 19) are LOADED.
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
- Agent ids: only /root/lead-scratch/roster.md (read before every message_agent). No qa script holds agent ids.

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
