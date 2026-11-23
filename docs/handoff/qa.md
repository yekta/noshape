# QA handoff (agent qa; current state only, updated 2026-11-22 19:0x)

qa tests releases and greens against the last release and live, times speed, files tickets in docs/TICKETS.md, and never
edits code. Standing instructions: /root/lead-scratch/prompt-qa.md. Rulings: docs/DECISIONS.md (read first). History:
docs/handoff/archive/qa.md. Scripts: /root/qa-scratch/bin (local), results under /root/qa-scratch. Docs commits from
/root/projects/noshape-wt/qa (`git checkout -B agent/qa origin/main`, commit, `git push origin HEAD:main`).

## IN FLIGHT 00:4x (after the compaction, first)
- **DONE since 22:1x**: matrix 113/125 on green 5e634b35e (psui b6812aae: manipulator ring over a selected edge, 4 cells =; PS cells
  pixel-identical; psui told). Re-verify pushed: skasm wf-1122 + walk 4 on 172785c95 (8 verified, 5 partly, new P0 DIM-PICKED-SIDE-BLACK,
  SPLINE-PLACED-POINTS, CLICK-EMPTY-SELECTS-PLANE (live asked), P1 LINE-STORES-PERPENDICULAR, P2 MIRROR-SYMMETRIC-COUNT); MC + sketch
  gestures on 02cea338f (6 verified, 7 partly, asm 3 still open; new P0 SK-LONGPRESS-SELECTS, MC-ASM-CYL-SIDE-OUTLINE, MC-ASM-SHIFT-RELEASE,
  P1 MC-PS-ORIGIN-CORNER-MARKER; live asked for a MARKER-REACH fine sweep). SM-TABLE-PANE-SPLIT closed (live default 50 %), P2
  SM-TABLE-SPLIT-KEPT. PS-RAIL-STOPWATCH-WARNING closed (llvmpipe badge). skasm, asm, psui told. Briefs: qa-scratch/brief-ver.md, brief-t23.md.
- **Subagents running (results come as notifications; if lost after compaction, rerun from the brief)**: (a) Mac timing vs live
  timing-1123 (brief-t23.md, raw in qa-scratch/t23/, gate: load < 6 + mem >= 25 % before EVERY Mac call, GUI also idle >= 600 s; lead
  00:0x: discard timings under load). (b) psui manip/nav + sel-gestures rows (brief-ver.md set "psui"; shots feel1122/manipnav/r6 on
  7fe68797a, fieldsel/r5 on 5e634b35e) -> mark rows like the skasm ones, tell psui.
- **Units (logs qa-scratch/tmp/<name>.log)**: qa-ver2 (workflows-1123 replay on vibex8 -> hm2/out-wf23b, log ver2-wf.log; then score per
  step vs out-wf23 and TICKETS blocks 21:4x/22:0x: mark fixed rows, file new); qa-ver3 (wf-1122 40 for CTX-MENU-FLIP-UP -> out-wf-v, then
  psui f5ebb8b3 'subj:a picked edge solid and bright under the preview' -> matrix hmr3-b/hmr3-ips: AXIS-UNDER-DISC, FILLET-FACE-OVER-PICKED
  #ffaf25, PICKED-EDGE-DASHED-UNDER-PREVIEW, HOVER-DISC-AFTER-COMMIT, PARTS-ROW-OF-PICK-SHEET-METAL, Corner break vertex disc);
  qa-ver4 (can't-tell reruns: xv-search-unfold + wf12/27 with WF_KEEP_WORLD=1 -> out-wfv2 (EXTRA-UNFOLD-REFOLD, UIS-WORLD-TARGET), walk3
  s3-sk-fillet -> out-wkv2 (SKFILLET-FLOW)); qa-rr-c6ec (asm c6ec3d1c subj "asm: tickets: notes folded into qa's rewritten rows" ->
  mcinfer r7 asm cells + asmfeel out-af-c6ec dr-BaseR: MARKER-LOOK-ASM, EDGE-BEFORE-VERTEX-ASM, FIXED-GLYPH-AA); qa-ver5 (psui to-verify
  FEEL-QUERY-PART-PICK-FILL wf-1122 19 + FEEL-DIM-SELECTED-REF-LINE-YELLOW 32v -> out-wfv3; then after c6ec matrix '*-mc-*' -> hmr4-mc:
  navy rim (psui: llvmpipe 1x, c6ec draws 4x settled) + asm's white notch in the ring); qa-wait-imp (bool 448c038e impeller ->
  wait-imp.log IMP-DONE: expect no invalid part, close IMPELLER-SHELL-INVALID, tell bool); qa-wait-sp (bool e7c98cc5a surface pattern ->
  wait-sp.log: expect ~13.6 s + Part 1 129/385/258, close COMPAT-SURFACE-PATTERN-SLOWER, tell bool). Hold each in a background task.
- Matrix scoring kit: hm2/crop2.sh <run> <cmpdir>; diff vs hm2/score-5e63/cmp-{b,ips}; score-a087/bin/sbs.sh for live|old|new.
- Runners take QA_HOST (fs.sh, mi.sh, fm.sh, asmfeel.sh; default vibex8); wf.sh WF_HOST/WF_XD/WF_SET/WF_OUT/WF_KEEP_WORLD; wf23.sh WF_OUT.
- Next from owners: asm Shift-release / cyl-side / rehover sha; psui rotate axis edge + Extrude top cap. Lead decisions open: Line end dot, ASM-MENU-WRAP-REVERSED.

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
- 11-22 06:57 suites/1122-0657-17157c2f (green 17157c2f, all stages vibex8 via slot, unit qa-suites): ui 2099/2230 (LOADED, peak 27); models stage running 11:5x. Unit qa-uip-rerun (bin/uip-rerun.sh <suite> tmp/uip-rerun.txt) reruns the 50 non-FS new failures + fs-open on vibex8 :481 once <suite>/status exists -> <suite>/uip-rerun.log. THEN: re-score uip with `run.py --compare-only --out <suite>/uip` (FS version now normalized: 77 FS scenarios match), suites-diff vs last run, file the still-failing ones as P0s (candidates: psui plane/sketch-entity menus 0102516b3, routing Points 88f31a8d2; asm face menu; studios drawing tab menu row 7; skasm gusset/bend titles + 4 sketch; 9 not-run), SCORE 11-22 row, lead report. Skip live-re-recorded: asm-root-menu-unfix-all(-mated), drw-custom-table-*, drw-export-formats, drw-note-format-lists, drw-table-cell-menu.
- FS VERSION (lead 11-22 11:4x, pushed 11:5x): ui-parity run.py fs_version_line(): "FeatureScript NNNN;" + std imports' version compared as <v>, except fs-open, where live reads LIVE_FS_VERSION (3132) and ours must match (a difference = studios ticket); bump LIVE_FS_VERSION when live's std moves. README "Comparison".
- **qa-suites**: bin/qa-suites-loop.sh, every 4 h: interaction suite, model suite (speed, debug) and assembly / exchange
  suites of the newest green, each through `~/bin/slot`: ui + models on **vibex7**, assemblies / exchange (suites-rest) on
  **vibex6** beside them since 11-17 04:51 (vibex7's slot was held 4 h by dx). 1117-0227-ecf8c33a's stage re-run as unit
  Results suites/<mmdd-hhmm-sha>/; host overrides QA_UI_HOST / QA_SPEED_HOST / QA_SUITES_HOST.
- **qa-cut-0415 / qa-cut-1615** timers: bin/cut-verdict.sh (cut-check.sh: newest green's compat vs the baseline) -> one GO /
  NO-GO line in /tmp/lead/qa-alerts and compat/cut-<mmdd-hhmm>.txt. Add the verdict as a "Release blockers" section in TICKETS.md.
  are listed, not blocking, in the script, but read them: a ticket doc's wrong geometry is a HOLD (11-17 f197).
- **qa-index-daily** (lead 11-20): bin/index-daily.sh "<since>": every 30 min, the first full suite run of the day (suites/<run>/index.json +
  a models score) -> one "Parity index" row in docs/parity/SCORE.md (bin/index-row.py; header order = index.json's areas), pushed, and a
  qa-alerts line. Started 07:3x with since "2026-11-20 07:30" (today's row from suite 1120-0407-81d23c9b or later).
- **qa-land-footer**: bin/land-footer.sh (land-queue footer checks).
- vibex7 user units **qa-fzloop** + **qa-fz-xvfb** (`systemctl --user` there): the GUI fuzz, ~/qa/bin/fzloop.sh :61 on
  ~/qa/rel/noshape-green (green 35baa32f since 11-17 03:34; refresh by copying a newer app to that path), seeds from ~/qa/fzseed.next,
  results ~/qa/fz*-result.txt. fzrun.sh kills the app's own `--reopen` restarts after each run. Pause: touch
  ~/tmp/qa-fz-pause-61; it also idles 01:50-04:05 every night (the window runs alone). App refreshed to green f7c318a5 11-17 10:08.

## Release checks (per release; lead cuts 05:00 / 17:00)
0r. DONE 09:32: corpus c2-loft-paths-guides + knifehandle re-saved on main 4e852218d (old copies vibex7 ~/qa/corpus-old-1122/); core: knifehandle 12 s = the now-reachable chain (Fillet 1 fails slowly, its speed queue). tablet-stand Sketch 3 alert stays open until skasm's stable naming. Recheck psui 6d72a4e3 MEASURE-PANEL-POSITION (drag by title, kept across close/reopen/restart, default bottom-right) by hand when green has it.
0. v0.4.66 DONE 08:13: rel-0.4.66.txt all SAME vs 0.4.65 (hat15 vs 0.4.43 known); relfeel 93 cells, SHOTS 0 differ. Green 0a91d98d vs 0.4.66 alerts all explained: lofts/knifehandle now Ok, workshop-split 14536.5 -> 14590.4 mm³ = live 14590.06 (core q-split-thicken1-1119), model-plus -0.015 mm³ noise, tablet-stand open (skasm).
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
   Quiet holds end at 04:00 sharp (lead 11-22 04:4x: vibex8 hold blocked builds to 04:41): nightpm-remote.sh and w7-quiet.sh each start a 04:00 remover (only a "qa" hold).
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
00c. 11-21 09:xx (lead): (1) TICKETS 'fixed, to verify' walk DONE on 5b840ddf0 (vibe2 walk2, ~/tmp/qa/walk2 + nscli copied there): verified today bool f724d647 (SHELL3-PART3-CONE-FACE), studios VARIABLE-KEYWORD-NAMES, asm FACE-MOMENT-DIGITS + INSERT-OTHER-DOC-GRAPH-MARKS (new P3 FACE-MOMENT-LXY-ZERO, INSERT-GRAPH-RESIDUALS), psui FEEL-CHIP-TOOLTIP-PLACE + SETLIB-FRAME-PROFILES, studios' BRAND line; FEEL-ROW-DBLCLICK-DISTANCE verified 09:5x on 219944450 (press gaps by xev: xdotool adds ~100 ms; chained 'click sleep S click' with S = target - 0.10; ~/tmp/qa/xev on vibe2); manualblend Move boundary Ok (c62392284), MANUALBLEND-BOUNDARY1 open. VALID-HGR-FOOT-OFFSET verified 12:4x (valid/hgr1121: all parts valid 0-51); FACE-MOMENT-LXY-ZERO verified 10:5x on 4a22b2966 (bin/lxy-walk.sh). (2) ui-parity 1121-0923-07879475-uip DONE 12:43: 2204, 59 failing, 33 worse: 15 = sketch row Section view… (P1 UIP-SKETCH-ROW-SECTION-VIEW, psui told); 13 'no recording' (silent app exits, neighbours fine) + face-menu x2 (menu not up at step 1), delete-face-tool, fillet-add-edge-in-dialog, ver-compare-hover-part: recheck in the loop's run 1121-0935-92d02c14 (watcher), route if they repeat. Note ~/qa-speed/bin/noshape is shared with the loop (my run used its 09:36 copy). (3) v0.4.64 published check DONE 10:11: 3 SHA OK, user docs ALL-SAME (vs 0.4.63 and vs the candidate), quit rc0 5/5, ticket docs' 4 changes all known/accepted; compat baseline now 0.4.64. Tonight qa-nightpm-1122: speedref now 20 Part Studios (live 11-20 tables 1-20; speedref.py EDITS with param key), dry check bin/speedref-dry.sh; add studios' zoom first-draw (TICKETS row 'a zoom shows the last frame') to the feel re-times.
00d. 11-21 USER P0 (lead): hover × selection matrix, interaction-feel.md 'Hover × selection state' Matched 90/125 (20:23: PS 26/26; batch 2 50/51; batch 3 0/15 (psui on it); batch 4 assembly 6/20 vs live as2/; batch 5 8/13 vs b5/ (gen2/gen-b5.py, out-b5, cmp-b5)). Open P0s psui: FEEL-SKETCH-BAND-MIDDLE (inf-on-line), FEEL-FIELD-HOVER-DIM, -FIELD-VERTEX-EDGE, FEEL-ORIGIN-HOVER-EDGE, FEEL-MC-LOOK, FEEL-FILLET-MANIPULATOR, FEEL-SKETCH-REGION-FILL-AFTER-DRAG, FEEL-SELECT-OTHER-LOOK. Open P0s asm (told 20:2x): FEEL-ASM-PS-MC-MISSING, -MANIPULATOR-RING, -VERTEX-LOOK, -MATE-VERTEX, -DRAG-LIT. Kit qa-scratch/hm2/gen2 (gen*.py; run.sh <app> <out> [height] on vibe2 :479, GLOB=…, NOT via slot (light, slot queue 7+); crop2.sh <out> <cmp>; cr.sh crop helper); bin/hm-rescore.sh 'subj:<text>' <glob> <out>. vibe2 ~/tmp/qa/apps/noshape-green = green 2fb0ed1ac. User walk 1 (kit hm2/walk, :481): REQUESTS 'qa 19:54' open (region shading after drag, chain box, plane lit while dimensioning, Line snaps). User walk 2 (lead 20:10: assemblies, dialog fields, box select, Select other, heavy hover): kit hm2/walk2 (gen.py, run.sh :482), out-w2, crops w2c/; real-pointer probes qa-scratch/hl (open.sh <doc> :154 <png>, hl.py time-to-light; box/asm drags). Matched: box select both ways + mid-drag, Fillet/Extrude field pick/remove/chip hover, Fastened face/edge, MC tool face/edge, plain part hover. REQUESTS 'qa 20:18' open: heavy hover times (ours 30-feature 150-245 ms, 400-hole plate 353-487, 1000-instance asm 560-750; file a speed P0 only if ≥2x and ≥100 ms vs live) and the amber second-picked face in Fastened.
00f. 11-21 20:2x sketch-tool walk 3 (lead: every sketch tool, menus, shortcuts): kit hm2/walk3 (gen.py s3 cells run.sh :483; gen4.py s4 PS menus run4.sh :484; gen5.py s5 isolations, run4.sh; inch docs: blk-in plate is 40x30 IN, targets in mm x25.4), out-w3/w4/w5, sheets hm2/sheet.sh, crops w4c/. Filed: P0 skasm FEEL-SKFILLET-FLOW, FEEL-POLYGON-SPINNER-DRIFT (closed: not a bug), FEEL-EXTEND-NO-EXTEND (closed: two picks as live), FEEL-OFFSET-PREVIEW-LOOK, FEEL-CHAIN-ORIGIN-DROP, FEEL-TANGENT-ARC-AT-ORIGIN (+slot), FEEL-SPLINE-PREVIEW-VANISHES (from live walk3-1121 answers 1-7, all read); P0 psui FEEL-HOVER-GLYPHS-SECOND-ENTITY, FEEL-HOVER-GLYPHS-IN-TOOL; P1 psui S-TOOLBAR-BORDER, MENU-TOP-ROW-SECTION-GROUP. All PS/sketch menus + shortcut sheet + dimensions (place, value box, re-edit, hover) + construction + chamfer match. REQUESTS 'qa 20:44' (10 items: axis lines black, chain through origin drops a segment, arc H/V on axes, circle boxed Ø, spline after click, tangent arc/slot at origin, Shift+/ tab, Extend, badges with a tool active, rectangle region hover) all 10 answered and filed (live/qa/walk3-1121/); 19:54 and 20:18 (2) answered + filed (FEEL-PLANE-LIT-WHILE-DIMENSIONING, FEEL-LINE-SNAP-MODEL-LOOK); 20:18 (1) heavy-doc times running on live lane 4: file per answer. psui batch 3 (666d9c47) watcher = systemd unit qa-b3psui-watch (log hm2/b3psui-watch.log): hm-rescore b[35]-* -> out-b3psui, then re-score + verify 5 tickets. core chamfer re-land 45fb8d24: read the compat loop file of the first green carrying it.
00g. 11-21 21:3x walks 4+5 (lead repeat of the sketch walk): gen6.py s6 (20 cells: remaining tools, dimension types, keys, badge hover, views) -> out-w6, 1 P0 FEEL-TEXT-BOX-PREVIEW (skasm); gen7.py s7 (36 cells: live ctx-menus-1117's 17 PS + 8 sketch cases on start ctx-blk, 6 assembly rows, 6 shortcut tabs by px y=120) -> out-w7; compare hm2/walk5/cmp-menus.py <out> (state list.rows vs live item lists): 14/25 same, P0 psui MENU-PLANE-GRAPHICS, -FACE-AND-EDGE, -SKETCH-IN-PS, -SK-HIDE-SKETCHES, -SK-MODEL-ENTITY, P1 KEYS-SHEET-DETAILS; assembly menus + 7 tabs match. REQUESTS 'qa 21:50' (default plane size vs sketches) open. Re-run all after psui menu fixes: GLOB=s7-* run4.sh, then cmp-menus.py.
00h. 11-21 22:2x walk 6 (lead: feature dialogs, drawings, doc tabs): kit hm2/walk6 (gen8.py s8 cells on ui-parity starts block / block-drawing / doc-tabs, run4.sh :484 on vibe2, rev.py <out> [glob] = per-step state summary; drawing park = sheet_point [140,120]; window 1079 cuts the bottom ~90 px: tab bar off-shot) -> out-w8, re-run out-w8b (walk6/rr). Filed P0 psui FEEL-EXTRUDE-DRAG-PREVIEW, FEEL-MANIP-DEAD-ZONE, MENU-SK-SKETCH-PLANE (live 21:50 answered: planes ±76 mm, sketch plane pickable in edit); P0 studios DRW-BORDER-LIT (to confirm); MENU-FACE-AND-EDGE closed (edge picks now (10,0,10) in gen7). REQUESTS 'qa 22:20' (5 looks) open; psui 21:58 decides ps-edge / sk-model-edge. Verify on psui's walk-5 menu land: GLOB=s7-* run4.sh + cmp-menus.py; drag preview: s8-ext-arrow-drag-*. psui 6e5e17c8 QUEUED (22:2x) = batch 3 + walk 3-5 fixes: units chain qa-b3psui-watch (gen2 b[35]) -> qa-w5-rescore (bin/w5-rescore.sh: walk3 s[357]-* -> hm2/out-w9 + out-w9/cmp-menus.txt) -> qa-w1-rescore (walk w-[dl]* -> out-w1b); then re-score interaction-feel 90/125 and tick/verify psui's 10 TICKETS rows. 22:5x live answered walk 6 (all 5) and 20:18 (1): filed P0 studios DRW-BORDER-DRAG (replaces BORDER-LIT), P0 psui FEEL-PREVIEW-HOVER-LIT, FEEL-SHELL-RECLICK-REMOVED-FACE, SPEED-HEAVY-HOVER (ours plate top 330-373 vs live 42-53; 1000 parts face 646-871 vs 265-270; plane-border control ~330 ms = frame cost; probe qa-scratch/hl open.sh <copy> :154 + hl.py :154 x y label n; plate top (1340,605) side (1250,816) plane (739,700); parts-1000 face (1566,900) edge (1566,884)), P1 FEEL-FILLET-ARROW-CLICK. 23:00 psui and studios told. 23:3x batch 3 re-scored on green 9f33d43 (f411d924): 4/15, matrix 94/125; verified FIELD-HOVER-DIM, FIELD-VERTEX-EDGE, FILLET-MANIPULATOR; reopened ORIGIN-HOVER-EDGE, MC-LOOK partly; new P0 FEEL-FILLET-PICKED-STRIP, FEEL-FILLET-ARROW-DRAG (out-w6fa: gen8 s8-fil-arrow-* cells, FA=(1247,544), run via bin/w6fa.sh XD=:486); psui told. REQUESTS 'qa 23:02' (Fillet arrow drags + Shell manipulator + pick point vs midpoint) open with live. psui's walk 3-5 menu fixes are a separate commit ('psui: hover and menus as live (qa walks 3-5)', not landed 23:35); qa-w5-rescore then qa-w1-rescore wait for it.
00i. 11-22 walk 7 (navigation feel): kit qa-scratch/w7 (appdoc.sh = app without ui-script; w7.py ffmpeg framemd5 wallclock; kit/ rec.py+cell.py+run7.sh for hosts without ffmpeg). Light block (software both sides): ours first frame 150-300 ms, live 340-1800. Tickets: P0 FEEL-CUBE-CORNER-HOVER, P1 FEEL-NAMED-VIEW-AFTER-SAVE (psui); FEEL-NAV-INERTIA + FEEL-NAV-ANIM-LOW-FPS re-ranked P2 software-GL-only 01:1x (lead 11-22: speed on real GPUs; ours drops both only < 20 fps). Live (8) heavy: SwiftShader 1 frame / 3-10 s, plate never loads. qa-w7-vx8 (llvmpipe heavy) CANCELLED 01:05. GPU instead: unit qa-w7-mac (bin/w7-mac.sh): v0.4.65 macOS app in mac:~/noshape-host/qa/w7 (scissor, infento, plate copies; NOSHAPE_CONFIG_DIR=w7/cfg with last-viewed.json opening the Assembly 1 tab: scissor 5f4da3e78c789552, infento 6243c2e4dc39b669), NOSHAPE_PERF_ORBIT=300 bench (orbit x3 docs, zoom plate+scissor), gated: user idle >= 600 s, load < 6, no other noshape app, timeout 170 -> w7/mac/*.txt + logs. Then qa-w7-macnav (bin/w7-macnav.sh): ui-script keys Shift+5/7, ArrowRight, Shift+1/7, F on plate+scissor with NOSHAPE_FRAME_LOG (TMPDIR=w7/tmp) -> w7/macnav/nav-*.frames: frames per view change (animated = several over ~500 ms). Targets: PLAN 2.7 orbit 60 fps on big models; file dx P0 if a heavy orbit < 60 fps on the Mac GPU... compare with live per lead rules (>=2x, >=100 ms). Mac CGEvent from ssh is untrusted (no inertia test there). Inertia (lead 11-22: no ssh Accessibility; use in-app input): psui 38229ff2 adds ui-script drag "button":"right", "moves", "interval_ms" -> once in a release (05:00 train) add a right-drag flick to w7/macnav nav-*.json and re-run qa-w7-macnav (frame log: frames after release = inertia).
00j. 11-22 01:1x walk 8 (feature list, Measure + Mass, mate dialog): kit hm2/walk8 (gen9.py w8-a/b/c cells on ui-parity starts + 'three'; run.sh :487 now Xvfb 2240x1440 + windowmove 0 0, so shots are window px at 0,0 (earlier walks: window at +242+90 on 1920x1080, right/bottom ~240/90 px cut: right-anchored UI was off-shot); bin/w8.sh [glob] -> out-w8*; cmp.sh/tile.sh assume +242+90). Filed P1 psui FEEL-DIALOG-FINAL-BUTTON, FEEL-RENAME-INPUT-LOOK; ~50 cells match (TICKETS 01:4x). Live answered REQUESTS 01:12 (1)-(9). Armed: qa-rs38 (bin/rs38.sh: psui 38229ff2 in green -> gen2 b3-* to out-b3r38 (ORIGIN-HOVER-EDGE, MC-LOOK, FILLET-PICKED-STRIP) + walk 6 s8-[efs]* to out-w6v (7 walk-6 tickets incl. FILLET-ARROW-DRAG); qa-w6v stopped, replaced). After 38229ff2: re-check CUBE-CORNER-HOVER + NAMED-VIEW-AFTER-SAVE by hand (w7/appdoc.sh on :154). asm 017520625 + f843236ed (MANIPULATOR-RING dashes, instance silhouette 2 px #ffa033) land after 33829c47: re-score batch 4 b4-* when asm sends the sha. Still armed: qa-w5-rescore -> qa-w1-rescore, qa-w7-mac, qa-w7-macnav.
00k. 11-22 02:0x: Mac GPU results (TICKETS walk 7): orbit plate 109 / scissor 112.6 / infento 110.4 fps, zoom plate 116.9, none over 16.7 ms -> no speed ticket. Scissor + infento ignore SIGTERM after their run (P2 APP-SIGTERM-IGNORED-MAC, dx told) -> unit qa-w7-macguard (bin/w7-macguard.sh) SIGKILLs only own qa/w7 app pids older than 200 s while qa-w7-mac/-macnav run. Left: scissor zoom + macnav (gated on user idle). Walk 6 (5): FEEL-FILLET-ARROW-ANCHOR filed then closed no-gap (psui: both anchor at the edge midpoint; ours picks 5/25 pixel-identical, hm2/out-w6a). psui is adding ui-script drag "hold": true + "release" step -> when it lands, re-run walk 8 held drags (feature-list pill + drop line below Right over Default geometry, rollback bar mid-drag).
00l. 11-22 03:0x: walks 1-5 re-score on green 09de25e3b (bin/w51.sh; vibe2 ~/tmp/qa/apps had been pruned, w51 restages with a fresh mtime): 11 verified, FEEL-PLANE-LIT-WHILE-DIMENSIONING reopened (psui asked live for the rule). Mac: view changes animate on the GPU (34-35 frames / 500 ms), scissor zoom 105 fps. qa-w7-inertia (bin/w7-inertia.sh) waits for a release carrying psui's ui-script commit, then a gated right-drag flick on the plate (w7/macnav/nav-inertia.json, frames after release = inertia). psui queued 0fc65a29 (walk 6, batch 3/walk 7, walk 5 edge menus, walk 8 Final button + rename box, drag "hold": true + "release"): qa-rs38 triggers on it (b3 + walk 6), then qa-w8r re-runs walk 8 incl. new held cells w8-h-* (gen9.py) -> hm2/out-w8. WORKFLOWS (lead 02:4x, top priority when they land): live/workflows-1122/<nn>-<name>/steps.json (scenario + per-step at_px, settled shot, first_change_ms, settled_ms; top: start, viewport, start_camera); scenarios also in /root/live2-scratch/scen. Kit hm2/wf (starts.json = tests/ui-parity/starts.json, run.sh 2240x1440 + NOSHAPE_FRAME_LOG); bin/wf.sh '<glob>' -> hm2/out-wf/<name> (shots, state.json, frames.txt); wf/wftime.py <out-wf/name> = per step ms to first present / settled. 01 + 06 already replay (17 / 4 shots, green e97ac7f1c). File every look/timing difference by owner (timing only >=2x and >=100 ms; software-GL caveat).
00m. 11-22: WALK 9 done (TICKETS "qa 03:3x"): live's drwm/vsm/fsm (live2-scratch/scen) via kit hm2/w9 (bin/w9.sh, w9cmp.py, w9crop.sh), px stand-ins for {"empty"}/"ace_word" (psui P2 UIS-EMPTY-NON-PS). Unit qa-w9r (bin/w9r.sh) replays the ORIGINAL scenarios (kit hm2/w9o) -> hm2/out-w9o once green carries psui 768e0e85; compare vs out-w9m. Unit qa-strs (bin/strs.sh, log hm2/strs.log) reruns walk 9 + workflow 09 when green gains "studios:" commits -> hm2/out-strs-<sha9>. psui 30d77c0f/0fc65a29: qa-rs38 -> qa-w8r -> qa-w51b re-score PLANE-LIT, SKETCH-DIM-PLACE-PICKS-PLANE (wf 01/02), MENU-SK-MODEL-ENTITY, walk 8 Final/rename/held drags; KEYS-SHEET-DETAILS by hand. asm: MANIPULATOR-RING rest + #ffa033 band + FEEL-MATE-SECOND-PICK-HOVER queued behind 33829c47 (asm sends the sha).
00n. 11-22: WORKFLOWS (live/workflows-1122/<nn>, TICKETS "qa 03:4x"): unit qa-wfwatch2 (bin/wfwatch.sh, log hm2/wfwatch.log, ~10 h) replays each folder on green when its steps.json changes (bin/wf.sh -> hm2/out-wf/<nn>, cmp.txt + tiles) and re-tiles when live pngs change. wf.sh: "world" -> "vertex" (UIS-WORLD-TARGET), live_only steps -> 100 ms wait, window height = live's viewport_css_px (02/06/07 are 992). Review each by hand: wf/wfcmp.py <nn>, wf/wfcrop.py <nn> <WxH+X+Y> <out> [steps] (live left). Variants: a hand-edited copy into vibe2 tmp/qa/wf/ + GLOB=<name> run.sh -> out-wfv (02w-arrowpx: arrow drag 25.2 vs live 24.9). Done 01-20 (filed by owner, all told; TICKETS "qa 03:4x"); variants for runner gaps: 02w (arrow px), 14v (config_row px + full menu text), 18v (Replicate fields px). 21-30 (lead 04:4x): done 21 (core THREAD-ON-REVOLVED-CYLINDER; psui FEEL-HELIX-FACE-PICKED-LOOK, PARTS-ROW-OF-PICK), 22 (skasm SKETCH-TRIM-SHORT-END P0, OFFSET-DEFAULT-SIDE, MIRROR-LINE-LOOK), 24 (asm EXPLODE-DRAG-NO-STEP-DIALOG, FEEL-EXPLODE-PICKED-INSTANCE-LOOK; 24v-config-px: asm ASM-CONFIG-ROW-MENU), 26 (studios FS-* x5), 27 (skasm FORM-REMEMBERED-PART-STUDIO); 23 (dx DWG-SECTION-HATCH-PER-PART; studios FEEL-DRW-BALLOON-HOVER-LEADER P0), 25 (asm MATE-LIMITS-TAB-FOCUS, MATE-GEAR-DRAG-TURNS-NOTHING; variant 25v-nogear), 28 (psui FEEL-SECTION-PLANE-LABEL-LOOK P0, APPEARANCE-RGB-FIELD-SCROLL; variant 28v-face-off-line), 30 (= live, all 51 steps); owners told (05:3x). Next: 29 (skasm PASTE-SKETCH-CENTRED, psui FEEL-PLANE-LIT-AFTER-MENU; told) -- 21-30 all reviewed. 31-40 (live 06:0x+): 38 (psui SPLIT-KEEP-TOOLS-AUTO), 36 (psui MEASURE-READOUT-VARIABLE-ICON; 36v-measure-px), 39 (dx EXPORT-STORE-DETAILS; 39v-short-waits: 7 x 20 s waits pass the app's 180 s script budget, cut to 3 s), 40 (psui FEEL-SKETCH-RCLICK-PLANE-LIT P0, CTX-MENU-FLIP-UP; asm ASM-FACE-MENU-ROWS; studios DRW-TAB-MENU-CHANGE-TO-VERSION; 40v-plane-row); all told. 37 (psui EXPR-AUTOCOMPLETE-LOOK P2). 33 (psui FEAT-MISSING-REF-CHIP; 33v-hover-tip). Next: 31-35 when live lands them. wf.sh/w8.sh now scp to noshape-green.new + mv (a busy binary blocked the copy). Corpus re-save c2-loft-paths-guides (core 06:3x: stale copy, ALERT-984adcc3 explained) running on vibex7 -> ~/qa-parity/resave-loft/saved; copy it over vibex7 ~/qa/corpus/c2-loft-paths-guides.noshape when done. Variants 21v-nohelix, 22v-trim in out-wfv. IMPORT-TAB-TOAST waits on live REQUESTS qa 04:35. Unit qa-skre (bin/skre.sh, kit hm2/skre -> out-skre) re-runs skasm's walk 3-5 sketch P0 cells when green carries f78f42fa's three subjects. Timing: ours faster at every step so far.
   07:0x: hm2/wf/<name>.remembered (raw feature_settings.json) seeds live's account memory (run.sh -> $C/feature_settings.json + NOSHAPE_UI_REMEMBERED=1; 27, 12 seeded). 35 reviewed (ISOLATE-SHOW-INTERNAL-GEOMETRY asm; 35v-px runner variant only partly right: 2nd-direction px wrong). 27/12 seeded: CORNER-RELIEF-TOO-SMALL-WARN skasm, SURFACE-LOFT-GUIDE-FAILS core. 34 = live once seeded (34.remembered: fillet radius 2 mm, chamfer width 0.5 mm; ours faster: tab 0.6 vs 3.5 s, dialog 0.8 vs 3.4, ✓ 0.8 vs 2.0). 36: FEEL-PICK-SKETCH-LINE-THROUGH-FACE psui. 31: DRW-TITLEBLOCK-EDIT-BOX studios (step 16 Dimension tool 4.3 s vs 0.9 at load 13: recheck quiet). 32: runner gap (live swallows the press after a drag; variant tmp/32v.json clicks empty space) -> FEEL-DIM-SELECTED-REF-LINE-YELLOW psui (+Front plane edge lit 35-38: re-check after 454c71f2). 11:1x re-score DONE on green 659f12d65 (skasm 3 + PASTE-SKETCH-CENTRED verified, SM-FLAT-VIEW-FIT reopened, new P0 BEND-FACE-RECLICK-EMPTIES; psui 11 verified, FEEL-QUERY-PART-PICK-FILL reopened; variants tmp/11v.json, 02v.json). Unit qa-rs-ec2f (bin/rescore-subj.sh ec2f0d3ca: psui "qa workflow tickets as live", subject prefix, any requeue; 19 38 29 05 20: psui 6d72a4e3 requeued as 91ae59a8; finds the landed tip by subject) -> recheck FEEL-QUERY-PART-PICK-FILL, SPLIT-KEEP-TOOLS-AUTO, wf29 grey edges, SKETCH-ON-FACE-EXITED-LOOK, MEASURE-PANEL-POSITION/-VALUE-CLICK-PIN (wf20; panel drag by title, kept across reopen/restart by hand). 12:4x: psui's 13 'to verify' feel tickets re-run (bin/b3run.sh, bin/psui-cells.sh -> hm2/out-b3-1122, out-pv): 7 verified, 6 duplicate rows dropped.
00e. 11-21 17:4x compat: core's chamfer revert dc58aff96 verified on 2d708fa4 (cf-surface-pattern, cli-draft-hang SAME vs 0.4.64); cpcurves volume shift + manualblend Boundary surface 1 Error accepted by core (accepted.txt note). Core re-lands the chamfer edge sense next: then check cf-surface-pattern, cli-draft-hang, infento Linear pattern 1 (+y), add a TICKETS row. UIP-THREAD-MESSAGE-TWICE verified 18:17 (green 02ccb4ef7, 11/11).
00e. skasm stack on 3fd4db71: 5 verified; FEEL-DIM-TWO-POINTS-ZERO / CHAIN-LASTPOINT / PASTE-PLACE still open. ANALYSIS-READOUT-HIDDEN
   verified 05:4x (2b3887ebe, 7/7); FEEL-DIM-PICK-READOUT (cov2-dim-orient step 8), ROLLBACK-DROP-ROW, SECTION-ARROW-FOOT: psui's next batch, on its word.
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
- dock B Govde 85/206/126 -> 83/200/122 since 61295b96f (Rib Round 1's tori one face each; accepted by the lead 11-20 15:0x;
  cut-verdict.sh skips exactly these two signatures). hat15 vs 0.4.43 always DIFFs (Fan Riser +5.83 mm³, STEP reference 51188.66 -> .65): known.

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
