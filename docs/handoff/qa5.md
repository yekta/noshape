# qa5: workflow QA against live Onshape (no building)

Agent qa5. Role: drive the app like a CAD user, compare every step with live Onshape (/root/onshape-research/live/behaviours.md),
keep one checklist per workflow in docs/parity/workflow-<name>.md, and send the lead batched findings (worst first, one owner each).
No code edits, no builds. Writes only under docs/parity/, docs/handoff/qa5.md and /root/qa5-scratch (+ "qa5 …" lines in live/REQUESTS.md).

## Setup
- Binaries: copy /root/target-shared/debug/noshape → /tmp/qa5/noshape-<HHMM>, /root/target-engine/debug/noshape-cli → /tmp/qa5/noshape-cli-<HHMM>.
  Check `free -g` (≥ 8 GB available) first. /tmp is tmpfs (RAM): delete old copies.
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

## Open live requests (REQUESTS.md, "qa5 (10-21 pass 3/4 …)")
- Answered: qa5 bracket, qa5 dim conflict, Hole row name (folded in).
- Answered and folded: undo across tabs, tabs, big plate, featurescript, zip import, sketch refs, relation dialogs.
- Pending: qa5 units, qa5 pass 7 (solving label, BOM undo, empty SCALE, flat filter / shaded item, configured BOM merge + Reset to default, out-of-date hint).

## Findings sent (latest first)
- Pass 4: REG1, PERF2, ADV9 confirmed, ADV12, SK1 (live answers); FS1–FS3, RD1 (stale volume readout), SK2, SK3, IM1, IM2, DSM1–DSM3, SM13 partly fixed.
- Pass 3: TAB1, ADV7–ADV11, PERF1, A12–A14, DOC5, DOC6, UNDO1, UNDO2, D14, D15, API1, API2 + re-run verdicts.
- Pass 1/2: SM1–SM17, A1–A11, D1–D13, I1–I6, SF1–SF8, C1–C8, ADV1–ADV6, AN1–AN4, DOC1–DOC4, KB1–KB4.

## Next steps
- Pass 7 fixed: RD1, ADV3, AN1–AN3, KB1, DOC6, TABS1–TABS4, C1, A3, A5, A11, A13, A14, A1 (fit), D14, D15, DSM1, EXP2; SK1/KB3/D8/DSM3 mostly.
  Still open: ADV1, ADV2, ADV7, ADV12, TAB1, TABS5, A4, SM1, hem, SM3, SM12, SM13, DSM2, SF2, SF5 (now moves all edges), SF6, I1, IM1–IM3, EXP1.
  New: A15 (selected-instance look), A18 (repeat insert position), CA1–CA4 (workflow 20), A16 ("solving…" label, live ?), D16 (Sheet1 chip over Insert view title). Live request "qa5 pass 7" filed (4 checks).
- Pass 5 results are in each checklist's "Re-run on integration 5d28447" section. Fixed in 5d28447: ADV4, SK2, DOC5 (new tabs), TAB1 partly, KB3 partly.
- Next: target-shared was rebuilt at 17:11 and origin/main is now d5745c9: copy it and re-run all ✗ rows; when target-engine is rebuilt from 5d28447 (or newer), re-run with them (D14 and the CLI rows still open).
- Fold live's pending answers into the "?" rows.
- Possible next workflows: export round trips (STEP/Parasolid/PDF) checked by re-import;
  Variable Studio driven bracket; Preferences (units, decimals) effect on every readout.

## Gotchas
- A chip added to a query field pushes the fields below it down: re-take positions after every pick.
- F during a view animation is swallowed; in a sketch F fits the plane outline only (SK2): zoom with the wheel.
- Never run the app binary with --version (it starts a second app). Window title = the document's internal name (`xdotool search --name noshape`).
- Copying a .noshape without its -wal/-shm loses recent changes. Dimensioning to model edges creates "use" curves (SK3).
- The bottom-right Volume readout can be stale (RD1): read volumes with the CLI `query_parts` instead.
