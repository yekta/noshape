# qa5: workflow QA against live Onshape (no building)

Agent qa5. Role: drive the app like a CAD user, compare every step with live Onshape (/root/onshape-research/live/behaviours.md),
keep one checklist per workflow in docs/parity/workflow-<name>.md, and send the lead batched findings (worst first, one owner each).
No code edits, no builds. Writes only under docs/parity/, docs/handoff/qa5.md and /root/qa5-scratch (+ "qa5 …" lines in live/REQUESTS.md).

## Setup
- Binaries: copy /root/target-shared/debug/noshape → /tmp/qa5/noshape-<HHMM>, /root/target-engine/debug/noshape-cli → /tmp/qa5/noshape-cli-<HHMM>.
  Check `free -g` (≥ 8 GB available) first. /tmp is tmpfs (RAM): delete old copies.
  **No pkill (10-22 rule):** start.sh records the app PID in /tmp/qa5/app.pid and stop.sh kills only that PID.
- Display :120 (Xvfb 1600×1000). Scripts in /root/qa5-scratch: `start.sh BIN [doc]` (memory check, `timeout 1800`, HOME=/tmp/qa5/home),
  `s.sh NAME "xdotool steps; sleep N" [crop]` (screenshot → /tmp/qa5/shots/NAME.png), `stop.sh`. After start: `xdotool windowmove/windowsize` to 1600×1000.
- Perf logs: start by hand with `NOSHAPE_PERF=1` (and `NOSHAPE_PERF_ORBIT=<frames>` for the orbit bench); start.sh doesn't set them.
- CLI scripts (pass 3/4) in /root/qa5-scratch/pass3: mklive.py (live's "qa5 advanced" cases), mkbracket.py, mkbig*.py, sf2.py,
  impmatrix.py (+ .log/.json), qacube.fs (FeatureScript sample), tools.json (CLI tool list). CLI tab kinds are "partstudio"/"assembly"/…
- Messaging: message_agent to live fails ("No agent you started has the id"); file requests in REQUESTS.md and ask the lead to ping live.

## Binary used
- Pass 1: target-shared 11:50; pass 2: 13:03 (+ CLI 13:33, which panicked on every call: clap duplicate `part`).
- Pass 3/4 (14:35–16:20): /tmp/qa5/noshape-1448 (target-shared 14:48), /tmp/qa5/noshape-cli-1436 (target-engine 14:36).
- Pass 5 (17:05–17:20): integration **5d28447** via /tmp/qa5/noshape-1659 = copy of /root/target-lead/debug/noshape (16:59; target-shared was still 14:48).
- Pass 6 (17:22–17:33, predecessor, 17:11 target-shared = d5745c9; stopped mid-pass, not written to checklists): ADV8, C5, A12, D5 fixed; rest unchanged.
- Pass 7 (18:56–19:50, new qa5 thread): app /tmp/qa5/noshape-1851 (target-shared 18:51, tree d5745c9 + uncommitted), CLI /tmp/qa5/noshape-cli-1809 (target-engine 18:09).
  Every checklist has a "Re-run pass 7" section; raw notes in /root/qa5-scratch/pass7.md. CLI scripts: pass3/impmatrix7.py (→ impmatrix7.log), roundtrip.py, sf2.py;
  doc copies for CLI checks in /tmp/qa5/cli (adv, surf, sm). CLI rollback: `feature_rollback {"after": "<feature id>"}`.
- Pass 8 (23:13–00:10): main **5bd50b2** via /tmp/qa5/noshape-2256 = copy of /root/target-lead/debug/noshape (22:56; target-shared was 22:02).
  No CLI newer than 18:09: geometry rows read in the app (Mass properties, UI import). Notes /root/qa5-scratch/pass8.md; "Re-run pass 8" sections in the checklists.
- Pass 9 (10-21 23:45 – 10-22 00:20, new thread on another account): app /tmp/qa5/noshape-2330 (target-shared 23:30, main still 5bd50b2), CLI still 18:09.
  Workflow 23 (derived) + live "qa5 pass 8" fold-in. Notes /root/qa5-scratch/pass9.md, setup pass9/mkderive.py, docs /tmp/qa5/d23/{derive,cfg,vs}.noshape.
- Pass 10 (10-22 00:25–01:00): app /tmp/qa5/noshape-0004 (target-shared 00:04, main 5bd50b2), workflow 24 (mates). Setup pass10/mkmate.py, doc /tmp/qa5/d24/mate.noshape (mate-base.noshape = clean copy).
  Gotcha: the view cube ▼ arrow rotates OVER the top (looking down upside-down); ▲ ×14 from there reaches the Bottom view. Picks after crops: add the crop offset.
- Pass 11 (10-22, finished 00:30–00:40 by the new thread): app /tmp/qa5/noshape-0014 (target-shared 00:14, main 5bd50b2), workflow 25 booleans **done**.
  Docs: /tmp/qa5/d25/bool.noshape (Split/Transform/Delete state), /tmp/qa5/d25b/merge.noshape (pass11/mkbool2.py: + Sketch 4 rect (30,5)–(70,15) for merge scope). Volumes: pass11/q.sh DOC.
  Live "qa5 booleans" answered and folded (10-22 ~01:20, section "Update from live's answer"): ✗ BOOL1, BOOL2, BOOL3, BOOL4 (revised), INT1, NAME1, NAME2, SPLIT1, COPY1 (psb); DEL1, DEL2, MENU1 (ui).
- Pass 12 (10-22 ~01:35): CLI /tmp/qa5/noshape-cli-0107 (target-engine 01:07); main still 5bd50b2, app still target-shared 00:14 (no app re-run needed yet).
  impmatrix12 (116 samples, 10 new g48–g50): IM3 FIXED; IM1, IM2, I1 still; new **IM4** g50-g46-shell1.x_t corrupt (bbox 37 m). SF2, SF5, SF6, SM1, hem, EXP1 still.
  SF5/SF6 are the existing Move boundary 1–3 in /tmp/qa5/cli/surf.noshape (copy it and run query_parts / feature_list); SM rows: feature_rollback after Flange 1 / Hem 1 on a copy of /tmp/qa5/cli/sm.noshape.
- Re-run rule: when a new Integrate commit lands (`git -C /root/noshape log -1 origin/main`) or a newer target-shared binary appears,
  copy it and re-run every ✗ row; each checklist has dated "Re-run pass N" / "Update from live's answer" sections.

## Progress
| # | Workflow | Checklist | State |
|---|---|---|---|
| 1 | sheet metal | workflow-sheet-metal.md | pass 3 re-run |
| 2 | multi-part assembly | workflow-assembly.md | pass 3 re-run (A2, A6–A10 fixed) |
| 3 | full drawing | workflow-drawing.md | pass 3 re-run (D1–D4, D7, D9, D11 fixed; D5, D10, D13 not re-run) |
| 4 | import & edit | workflow-import-edit.md | pass 3 re-run (I2, I4, I6 fixed) |
| 5 | surfaces & multi-body | workflow-surfaces.md | pass 3 re-run |
| 6 | configurations & variables | workflow-configurations.md | pass 3 re-run (C2, C4, C8 fixed) |
| 7 | advanced Part Studio | workflow-advanced-part-studio.md | pass 3 re-run + live answers (ADV6 fixed, ADV5 withdrawn) |
| 8 | analysis | workflow-analysis.md | pass 3 re-run |
| 9 | document management | workflow-document.md | pass 3 re-run |
| 10 | keyboard & mouse | workflow-keyboard-mouse.md | pass 3 re-run |
| 11 | bracket from a drawing, timed; CLI vs UI | workflow-bracket.md | done; live answered (77 ms total, ADV9 confirmed); UI build steps 1–3 = CLI |
| 12 | large models | workflow-large-models.md | done; live "qa5 big plate" pending |
| 13 | undo/redo across studios | workflow-undo.md | done; live "qa5 undo across tabs" pending |
| 14 | multi-tab work | workflow-tabs.md | done; live "qa5 tabs" pending |
| 15 | FeatureScript custom feature | workflow-featurescript.md | done; live "qa5 featurescript" pending |
| 16 | import formats matrix | workflow-import-matrix.md | done (CLI, all samples); live "qa5 zip import" pending |
| 17 | sheet-metal drawing with flat view | workflow-drawing-sheet-metal.md | done |
| 18 | export → re-import round trips | workflow-export-roundtrip.md | done (EXP1 Rhino, EXP2 OBJ) |
| 19 | workspace units | workflow-units.md | done; live "qa5 units" pending |
| 20 | configured part in an assembly (Change configuration, configured insert, BOM, Variable Studio) | workflow-configured-assembly.md | done pass 7 (CA1–CA4, A18 new) |
| 21 | drawing of a configured assembly + BOM table after a configuration change | workflow-configured-assembly-drawing.md | done pass 8 (CA5, CA6 new; 19:24 build) |
| 22 | Variable Studios driving a Part Studio (order, clashes, own variables) | workflow-variable-studios.md | done pass 8; live "qa5 pass 8" folded 10-22 (VS1 withdrawn, VS2 confirmed ✗, VS4 new) |
| 25 | multi-part booleans (union/subtract/intersect, Keep tools, names, Split, Transform copy, Delete part, merge scope) | workflow-booleans.md | done pass 11 + live folded (12 ✗, psb has fixes) |
| 24 | mates and mate connectors in depth (flip rule, HUD, connector panel, Offset/Limits, mate values, Animate, Mate connector tool, Replace instance) | workflow-mates.md | done pass 10 (MT1–MT9) |
| 23 | derived parts + in-context references (Derived dialog/panel, source changes, configured derive, derive twice, Insert, Edit in context, Update context) | workflow-derived.md | done pass 9 (DER1–DER12; live "qa5 pass 9 derived" pending) |

## Open live requests (REQUESTS.md, "qa5 (10-21 pass 3/4 …)")
- Answered: qa5 bracket, qa5 dim conflict, Hole row name (folded in).
- Answered and folded: undo across tabs, tabs, big plate, featurescript, zip import, sketch refs, relation dialogs.
- Answered and folded: qa5 pass 8 (10-22).
- Pending: "qa5 (10-21 pass 9, workflow 23 "derived parts" …)" (13 questions; fold into workflow-derived.md "?" rows).

## Findings sent (latest first)
- Pass 11 (sent 10-22): BOOL3 severe (Extrude Add unions the tool into each scope part separately), BOOL4 (empty default merge scope), DEL1 (Delete part chip "Part"), COPY1 ?; BOOL1 severe (disjoint Union Part 1 + Part 3 deletes Part 2 and duplicates Part 3 as "Part 4"); BOOL2 ? (Intersect ignores Keep tools).
- Pass 10: MT1–MT9 (workflow 24, mates).
- Pass 9: DER1–DER12 (workflow 23), DOC3 fixed, TAB1 still (also derived studios), VS1 withdrawn, VS2 confirmed, VS4 new.
- Pass 4: REG1, PERF2, ADV9 confirmed, ADV12, SK1 (live answers); FS1–FS3, RD1 (stale volume readout), SK2, SK3, IM1, IM2, DSM1–DSM3, SM13 partly fixed.
- Pass 3: TAB1, ADV7–ADV11, PERF1, A12–A14, DOC5, DOC6, UNDO1, UNDO2, D14, D15, API1, API2 + re-run verdicts.
- Pass 1/2: SM1–SM17, A1–A11, D1–D13, I1–I6, SF1–SF8, C1–C8, ADV1–ADV6, AN1–AN4, DOC1–DOC4, KB1–KB4.

## Next steps
- Pass 9 queue left: fold live "qa5 pass 9 derived" answers; CLI rows (SF5/SF6, IM1–IM3, EXP1) when target-engine's CLI is newer than 18:09;
  re-run all ✗ rows when origin/main moves past 5bd50b2 (target-shared/target-lead were rebuilt 23:58/23:59 but main had not moved at 00:20).
- Pass 8 fixed: ADV1, ADV7, SK1, TABS5, A15, A16, A18, CA1, CA2, CA5, CA6, D16, SM12, SM13; partly ADV12, KB3, CA4, DSM2.
  Still: ADV2, ADV12 row, TAB1 (PS2), KB3 opacity, A4, CA4 Tab (CA3 = ? pending ui crop: live is .95 alpha too), D8, SM1, hem, SM3, SF2, I1 (+ CLI-only SF5/SF6/IM*/EXP1 untested since 18:09 CLI).
  New: A19 (empty Replicate error state), D17 (drawing self-regenerates), D18 (drawing BOM ≠ assembly BOM), DSM4 (picker 4 vs 7 filters), DSM5 (shaded flat view broken, flat view opened shaded).
  Live "qa5 pass 7" answered and folded. No live requests pending.
- Pass 7 fixed: RD1, ADV3, AN1–AN3, KB1, DOC6, TABS1–TABS4, C1, A3, A5, A11, A13, A14, A1 (fit), D14, D15, DSM1, EXP2; SK1/KB3/D8/DSM3 mostly.
  Still open: ADV1, ADV2, ADV7, ADV12, TAB1, TABS5, A4, SM1, hem, SM3, SM12, SM13, DSM2, SF2, SF5 (now moves all edges), SF6, I1, IM1–IM3, EXP1.
  New: A15 (selected-instance look), A18 (repeat insert position), CA1–CA4 (workflow 20), A16 ("solving…" label, live ?), D16 (Sheet1 chip over Insert view title). Live request "qa5 pass 7" filed (4 checks).
- Pass 5 results are in each checklist's "Re-run on integration 5d28447" section. Fixed in 5d28447: ADV4, SK2, DOC5 (new tabs), TAB1 partly, KB3 partly.
- Next: target-shared was rebuilt at 17:11 and origin/main is now d5745c9: copy it and re-run all ✗ rows; when target-engine is rebuilt from 5d28447 (or newer), re-run with them (D14 and the CLI rows still open).
- Fold live's pending answers into the "?" rows.
- Possible next workflows: export round trips (STEP/Parasolid/PDF) checked by re-import;
  Preferences (units, decimals) effect on every readout; derived parts / Part Studio references.

## Gotchas
- A chip added to a query field pushes the fields below it down: re-take positions after every pick.
- F during a view animation is swallowed; in a sketch F fits the plane outline only (SK2): zoom with the wheel.
- Never run the app binary with --version (it starts a second app). Window title = the document's internal name (`xdotool search --name noshape`).
- Copying a .noshape without its -wal/-shm loses recent changes. Dimensioning to model edges creates "use" curves (SK3).
- The bottom-right Volume readout can be stale (RD1): read volumes with the CLI `query_parts` instead.
