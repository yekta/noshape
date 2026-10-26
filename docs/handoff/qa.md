# QA handoff (agent qa, from 2026-10-26 evening)

qa tests the newest green build (/root/green/noshape, commit in /root/green/SHA) against live Onshape and files tickets
in docs/TICKETS.md. It never builds. Prompt: /root/lead-scratch/prompt-qa.md.

## Daily release smoke
(none yet: the first daily release under the new plan is due 2026-10-27 ~05:00 UTC)

Checklist (30 min, on /root/green/noshape at the release commit; numbers to compare in brackets):
1. tests/perf/live-30-features.noshape: opens, mass properties 60779.484 mm³; edit Extrude 1 10 → 12, ✓, Ctrl+Z, Ctrl+Y
   (regen line in the NOSHAPE_PERF log; undo/redo cached).
2. New Part Studio: centre rectangle 80×50 + dims, extrude 10; fillet top face R2; chamfer 1; sketch points + Hole
   (counterbore); Shell top 2.5 on the plate **without** a hole (14467.382 mm³); linear pattern; mirror; boolean.
3. Sketch: fillet R5 → R20, trim, offset, dimension edit, Esc / ✓.
4. Assembly: insert 2 parts, revolute mate via tool search, drag (only the dragged part moves).
5. Drawing: Create Drawing (suggested views), Update after a model change, export PDF + DXF (R14, mm).
6. Part › Export STEP, + › Import it back (same volume); Documents window › Create › Document.
7. Durability: kill -9 right after an edit, reopen, the edit is there.
8. Re-run /root/qa-scratch/bin/{shellmatrix,hollowmatrix,holematrix,rimmatrix,boolmatrix}.py if a CLI is shipped.

## Setup that works
- App: `cp /root/green/noshape /root/qa-scratch/noshape-<sha8>`, run with `NOSHAPE_PERF=1 NOSHAPE_PERF_PERIOD=1
  NOSHAPE_NO_NATIVE_DIALOGS=1 DISPLAY=:110`, window resized to 1600×1000 at 0,0 (xdotool). Helpers:
  /root/qa-scratch/bin/q.sh (shot, crop, clk, dclk, rclk, key, typ, settle). Regen times per feature come from the
  `regen[...]` lines of the perf log.
- Documents are SQLite (`.noshape` + `-wal`); to hand one over while the app runs:
  `python3 -c "import sqlite3; s=sqlite3.connect('file:X?mode=ro',uri=True); d=sqlite3.connect('Y'); s.backup(d)"`.
- Live: my own Chrome tab via /root/live-pw/rqa.mjs (copy of r.mjs; tab id in /root/qa-scratch/.qa-tab; 180 s CDP
  connect timeout). Each run costs ~2 min just to attach on this loaded box, and page.goto needs `timeout: 0`; batch
  checks into one script, or ask the live agent in REQUESTS.md.
- CLI (v0.4.8 release, /root/qa-scratch/rel/…/noshape-cli) through a small MCP client: /root/qa-scratch/bin/nscli.py
  (`Cli()('feature_add', {...})`). Matrices with analytic volumes: shellmatrix.py, hollowmatrix.py, filletmatrix.py,
  featmatrix.py, holematrix.py (same dir). Hole style needs `styleV2`; a Remove extrude needs `booleanScope`.
- tools/lead/disk-loop.sh deletes files named noshape* > 100 MB older than 2 h under /root/*-scratch (maxdepth 4): touch
  the green copy or keep binaries deeper (the v0.4.8 release is in /root/qa-scratch/rel/k/v048/…, depth 5).
- The watcher's own CLI build (target-lead, ~20:18, main d62f25e2/c9540ae6, not green) is copied to
  /root/qa-scratch/rel/k/main/x/noshape-cli (`NSCLI=… python3 …` with nscli.Cli(os.environ['NSCLI'])).
- "View in high quality" on this llvmpipe box makes viewport frames take > 10 s ("GPU frame still running after 10s;
  waiting"); the UI then stops taking input for a minute and replays the queued clicks. Click Disable on the blue bar.
- UI fuzzer: /root/qa-scratch/bin/fuzz.py <seed> <n> (random keys / clicks / drags on :110 against the pid in
  /tmp/qa_fzpid; actions logged to /root/qa-scratch/fuzz-actions.log). Seeds 1–3 (800 actions): no crash.
- Don't run the app with NOSHAPE_PERF=1 when testing section views (PERF-SECTION-PANIC freezes the viewport).
- Mind dialog layout shifts: once a selection chip is added, every row below moves down ~16 px; screenshot before
  clicking a field.

## Tested
### 2026-10-26, green 4d2f6b0c (13:26), box load 35–40
- 30-feature part: open (full regen 970 ms), edit Extrude 1 → commit 1007 ms (REGEN-SPEED), undo/redo 7/5 ms, mass
  properties 60779.484 mm³ = live.
- New studio: centre rectangle + Ø20 circle, dims, extrude 10, fillet top face R2, chamfer 2 bottom edges 1 mm, 2 sketch
  points + Hole, Shell top face → SHELL-WALLS (P0) and SHELL-SLOW (P0). Same plate without the hole shells fine.
- 20:30–20:52: kill -9 right after a Suppress → reopen shows it (durable); STEP export → import round trip exact (14467.382
  mm³); Revolve about a sketch line, Boolean union of disjoint parts (live's info text), counterbore hole −634.522 mm³
  (= hand calc), part pattern ×100 in 73 ms, sketch fillet R5 default then sticky, new document from the Documents page
  (Part Studio 1 + Assembly 1), "Close document" checkpoints the WAL into the .noshape. New: PATTERN-OFFPART-PARTS (P0,
  live asked), TAB-NO-PREVIEW (P1), DELETE-TAB-DIALOG (P2), VERSION-0.4.6, a 2nd SHELL-WALLS repro. Big plate (400 holes):
  regen 3.0 s (Extrude 2 2458 ms) vs live 617 ms, on a box at load ~40.
- 21:00–21:40: CLI matrices on the v0.4.8 release: fillet / chamfer of all 12 box edges (32 cases), extrude draft (inward,
  4 + 2 cases), partial revolve (5), feature pattern of a hole ×2…200 (0.86 s for 200), hole Simple / Counterbore /
  Countersink × Through / Blind (12): all exact. Shell: 96-case matrix → SHELL-WALLS pattern (hole + R < t ≲ 1.5R) and
  SHELL-SLOW (8–17 s); hollow / open-top shells of a filleted box with t > R wrong (SHELL-FILLETED-BOX, live confirmed).
  Import g25-b x_t / sat / step: three volumes (I1). PDF export of a drawing OK (1 page letter, v1.5). Measure readouts
  (face + edge "Perp line and plane angle") = live. New tickets: DCLICK-HEAVY, PERF-SECTION-PANIC, LOFT-PROFILE-LIST,
  RHINO-GREEN, API-HOLE-STYLE, API-REMOVE-SCOPE; PATTERN-OFFPART-PARTS closed (live does the same).
- 22:00–22:40: two app processes on one file → TWO-PROCESS-LOST-EDIT (P0); bad files open silently (OPEN-ERROR-SILENT);
  UI fuzzer 1300 random actions over Part Studio / drawing / assembly tabs, no crash (PANEL-BLEED found); feature-row drag
  reorder, rollback-bar drag, Roll to end, Variable Studio (#var autocomplete, change propagates), material assign +
  mass (0.191 kg), Duplicate / Delete of a tab with an open dialog, chamfer types (equal / two / distance-angle),
  up-to-next / face / part ends, thin extrudes, draft (neutral plane), split by plane: all as expected. Slot tool = live.
- 22:40–23:00: Add-extrude merge scope with two touched parts (empty + "No merge scope selected.", switches back to New
  when a depth change reaches a second part) = live; configurations via CLI (list input, configured depth and
  suppression: 11882.891 / 30000.000); sketch offset / constraints / dimensions via CLI exact; sweep along line + tangent
  arc exact (self-intersecting → "Could not create valid swept body, check input."); circular pattern exact; sketch
  cancel → Restore reopens the sketch with the edits; view cube arrows 15° per click (= live). New: HOLE-MISS-MSG,
  PATTERN-REAPPLY-CUT-ONLY.
- Checked and same as live: R = centre-point rectangle, Shift+C = Curve/surface analysis (no Chamfer shortcut), Enter in
  Depth keeps the dialog, Esc in an open end-type list closes only the list, Hole dialog defaults (Inch tab, 1/4, 12 mm,
  118 deg), "Features …" + red badge while a new feature is invalid.

## Open findings (mine) by area
- core: SHELL-WALLS, SHELL-FILLETED-BOX, SHELL-SLOW, REGEN-SPEED (all P0); I1 (P2); carried over qa5's P1-16 / P1-6.
- psui: TAB-NO-PREVIEW, DCLICK-HEAVY (P1); DELETE-TAB-DIALOG, LOFT-PROFILE-LIST, PERF-SECTION-PANIC (P2); carried over
  qa5's P1-8 caveat, P1-10.
- dx: API-HOLE-STYLE (P1), API-REMOVE-SCOPE, VERSION-0.4.6, RHINO-GREEN (P2), USER_GUIDE undo sentence (P3).
- Live requests open (REQUESTS.md "qa (2026-10-26 …)"): drawing title block MATERIAL/WEIGHT.
- skasm: carried over qa5's P1-4 data.

## qa4 round-5 routed items re-checked on green 4d2f6b0c (2026-10-26 22:00)
Fixed ✅: DRAG-RUNAWAY (19-cube planar stack: only the dragged cube moves), SUPPRESSED-MENU, SKETCH-UNCONSUMED (docs made
by this build; docs made by older builds keep a sticky Hide), TOOLTIP-PARENT, ERR-MISSING-REGION ("Select face or sketch
region to extrude." + "1 missing selection"), CHAMFER-OVERFLOW (25 on a 20 mm edge OK; v0.4.8 still fails), SECTION-MANIP
(ring + arrow + rotate glyph), empty rename → default name, SELECT-OTHER, BOX-PREHIGHLIGHT, MIDPOINT-MARKER (dark ring at the hovered
edge's midpoint), READOUT-AXES, EXT-2ND-DEPTH, EXPORT-DXF-OPTIONS (DXF R14, mm, 75 LINE / 40 MTEXT / 4 CIRCLE / 4 ELLIPSE).
FOLDER-EXPAND-UNDO, FOLDER-MENU (Rename | Add selection to folder… | Suppress | Dynamic suppression ▸ | Expand ▸ | Collapse ▸ |
Delete | Unpack folder). Still open (tickets): EXT-DIRECTION, CHIP-HOVER, FOLDER-NAME-INLINE. Not re-checked yet: SEL-REJECT-TOAST (couldn't trigger: an edge click
in Extrude's regions field picks the face), ORD-BASE.

## To verify on the next green (fixed on main by qa5 after green 4d2f6b0c)
P1-1, P1-8, P1-9, P1-17, P1-18, P2-A2…A8, P2-D2, P2-D3 (docs/handoff/qa5-routing.md "qa5 open items at the 2026-10-26
stop"; P2-A3 / A7 / A8 have no unit test: Esc in explode editing, the pending Replicate row, no "instance(s) created"
bubble).

## Next
Revolve / patterns / mirror / boolean on the new plate; feature-list reorder/delete/rollback on the 30-feature part;
selection, measure, section view; then P2 assembly and drawing passes; verify the list above once green moves.
