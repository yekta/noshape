# QA handoff (agent qa, from 2026-10-26 evening)

qa tests the newest green build (/root/green/noshape, commit in /root/green/SHA) against live Onshape and files tickets
in docs/TICKETS.md. It never builds. Prompt: /root/lead-scratch/prompt-qa.md.

## Release blockers seen by qa (updated 2026-10-27 04:40, green bcf401ca)
- Verified in the green app bcf401ca: the five crash documents open (DOC-POISON guard), TWO-PROCESS-LOST-EDIT (both edits
  kept). Still open P0: DRAFT-HANG (draft defeaturing spins; the app's regen never ends and blocks later ones), FILLET-HANG
  (budget 97081557 on main, checked on the watcher CLI: fillet / chamfer fail after ~30 s; not in green yet), SHELL-SLOW,
  REGEN-SPEED (commit 1.0–1.7 s at load ~35; undo 3 back 886 ms).

### Earlier (P0, green 4d2f6b0c)
- DOC-POISON + SHELL-SLAB-GPF + FILLET-SEGV: ordinary Shell / Fillet / Delete operations can segfault the green app and
  leave a document that crashes on every open; v0.4.8 opens the same documents. DRAFT-SEGV-DOC also hits v0.4.8.
- SHELL-WALLS: wrong solid with status OK (also in v0.4.8).

## Daily release smoke
### v0.4.9 recut (release/v0.4.9 7ee597e0 = main 502416cc + "Release v0.4.9"), 2026-10-27 05:00, smoke on green 502416cc
- **The earlier blocker is gone**: the recut contains bf8aaee2 / 87e8275f (crash guard, OCCT patch, DOC-POISON rollback),
  7a04ba4d (SHELL-WALLS) and 97081557 (blend budget). The five crash documents open in the green app (bcf401ca and later);
  cli-fillet-hang.noshape opens in 47 s (Fillet 2 fails after the 30 s budget) instead of hanging.
- OK: 30-feature part full regen 634 ms on open; edit commits 707 / 396 / 344 / 200 / 318 / 498 ms (load ~25, REGEN-SPEED
  still over 200 ms); undo 1 / 1 ms, then 618 / 390 ms past two states; Create Drawing (suggested views, 1:2) + PDF (1 page
  letter, 1.5); STEP and Parasolid export → import 60779.484 = live (48 faces); STL / 3MF / OBJ meshes closed, 3MF manifold;
  assembly opens, a revolute pin drags without moving off its hole; kill -9 right after a Suppress keeps it; geometry
  matrices on the watcher CLI 04:48: fillet 32/32, boolean 11/11, shell 96/96 (walls everywhere; hole cases 3.5–7.4 s),
  hollow 60/60, rim 36/36, transform 5/5, split / mid plane exact; UI Part mirror about Right = 121558.969 (2×).
  featmatrix's hole patterns fail only through API-REMOVE-DIR; patmatrix's ×50/×100 overlapping holes now fail "Could not
  create all instances as entered" on purpose (live's rule, pattern.rs copies_fail); holematrix (now styleV2) 12/12 exact.
- Also OK (green 525c619b, 05:20–05:50): drawing Update after a depth change (views follow, the dimension stays, the
  button goes dim), STL import (+ › Import…: "l30" Part Studio, mesh 60774.111 = the file), UI sketch on a face → circle
  → Ø dimension 10 → Remove extrude (34044.055 exact), dimension editor, undo empty after reopen (= live).
- **New P0 seen while smoking: BOOL-THREAD-CRASH** (OCCT boolean segfaults on pool threads, nondeterministic; fuzz seed
  72 twice on the watcher CLI 04:06); REGEN-NO-CANCEL (P0). **The release CI's Windows build failed** (TICKETS).
- Known open in the release: DRAFT-HANG (P0: a draft can spin forever; the app's regen then never ends), SHELL-SLOW (a
  failing shell 52 s), REGEN-SPEED, API-REMOVE-DIR (P1: scripted REMOVE extrudes miss; featmatrix's hole patterns fail).
- Pending: the CI artifact (run 40452492018) for the version string and the crash documents.

### v0.4.9 candidate (release/v0.4.9 2650df69 = main 34ab98f7 + CI fix; CI run 40434076347 Linux artifact), 2026-10-27 03:10
- **Blocker:** the build segfaults opening 4 of qa's 5 crash documents (SHELL-SLAB-GPF, FILLET-SEGV, FILLET-SEGV-BLEND,
  DRAFT-SEGV-DOC); it predates bf8aaee2 / 87e8275f / 7a04ba4d. Green 19555ca3 opens all five. See TICKETS "Release v0.4.9".
- OK: 30-feature part opens (full regen 618 ms at load ~40), 60779.484 mm³ = live; edit Extrude 1 → 12 commits (710 ms),
  undo cached (2 ms); new Part Studio: centre rectangle + dims, extrude, fillet, shell; Create Drawing (suggested views,
  1:2, 4 views) in ~10 s; drawing PDF export (1 page letter, PDF 1.5).
- Not run: assembly, import/export round trip, kill -9 durability (done on green the day before).

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
- **Crash markers** (`/tmp/noshape-regen/*.marker`, CRASH-MARKER-COPIES): a process killed mid-regen (kill -9, `timeout`)
  leaves one, and the next document opened with that feature id (any copy of the same fixture) opens rolled back. After
  a kill test, expect the next copy of that fixture to open rolled back, or remove your own dead pids' markers first.
- **Give the app window X focus first** (`xdotool windowfocus --sync <id>`, q.sh `focusw`): :110 has no window manager, so
  without it GPUI sends no focus/blur events (Tab / click-away commits never fire; TAB-NO-PREVIEW was this artifact).
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
- tools/lead/disk-loop.sh deletes files named noshape* > 100 MB older than 2 h under /root/*-scratch (maxdepth 4): keep
  binaries at depth ≥ 5: green copy /root/qa-scratch/rel/k/green/x/noshape-<sha8> (symlinked from /root/qa-scratch),
  v0.4.8 in /root/qa-scratch/rel/k/v048/…, the watcher's main CLI in /root/qa-scratch/rel/k/main/x/.
- The watcher's own CLI build (target-lead, ~20:18, main d62f25e2/c9540ae6, not green) is copied to
  /root/qa-scratch/rel/k/main/x/noshape-cli (`NSCLI=… python3 …` with nscli.Cli(os.environ['NSCLI'])).
- "View in high quality" on this llvmpipe box makes viewport frames take > 10 s ("GPU frame still running after 10s;
  waiting"); the UI then stops taking input for a minute and replays the queued clicks. Click Disable on the blue bar.
- nscli.py starts the CLI with PDEATHSIG, so a `timeout`-killed fuzz run no longer leaves an orphan CLI; an orphan
  still burning CPU is a hang (seed 33 → FILLET-HANG).
- CLI modelling fuzzer: /root/qa-scratch/bin/clifuzz_dbg.py <seed> <n> (NSCLI=<cli>; random fillet / chamfer / shell /
  extrude / draft / pattern / mirror / suppress / delete / undo; log cli/clifuzz-<seed>.log; NSCLI_ERR=<file> keeps
  stderr); clifuzz_bisect.py replays a seed with a cold open in a fresh process after every step. Seed 1, 5 → DRAFT-SEGV-DOC; seed 7 → SHELL-SLAB-GPF; seed 9 → FILLET-SEGV; seed 11 (no drafts) → SHELL-SLAB abort; seeds 2, 3, 4, 6, 8 clean. The v0.4.8 release CLI
  (seeds 21–24, FZTAG=r): only the Draft crash (seed 22), so the Shell / Fillet crashes are new since v0.4.8.
  clifuzz_nodraft.py skips drafts; main seeds 30–35: 32 → FILLET-SEGV-BLEND (also v0.4.8), 33 timed out, rest clean.
  Sketch fuzzer skfuzz.py (draw / constrain / dimension / trim / offset / delete / undo): seeds 1–3 × 200 ops on main, no
  crash. Crash rate of the modelling fuzz on main: ~1 per 100–150 random operations.
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
- core: **DOC-POISON, DRAFT-SEGV-DOC, SHELL-SLAB-GPF** (P0 crashes that leave unopenable documents; SHELL-SLAB-GPF is a
  regressions since v0.4.8, all three crash the green app on open), SHELL-WALLS, SHELL-FILLETED-BOX, SHELL-SLOW,
  REGEN-SPEED, TWO-PROCESS-LOST-EDIT (all P0); I1 (P2); carried over qa5's P1-16 / P1-6.
- psui: TAB-NO-PREVIEW, DCLICK-HEAVY (P1); DELETE-TAB-DIALOG, LOFT-PROFILE-LIST, PERF-SECTION-PANIC (P2); carried over
  qa5's P1-8 caveat, P1-10.
- dx: API-HOLE-STYLE (P1), API-REMOVE-SCOPE, VERSION-0.4.6, RHINO-GREEN (P2), USER_GUIDE undo sentence (P3).
- Live requests: none open (title block: live's ANSI templates have no MATERIAL/WEIGHT fields either = ours).
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

## Green 502416cc (2026-10-27 04:53–05:22)
- Verified: FILLET-HANG (the hang doc opens in 47 s; residual 30 s stall per failing blend), EXPORT-MESH-SHARED,
  DEPTH-RANGE (pink only after blur, live also while focused), CHIP-HOVER.
- New: REGEN-NO-CANCEL (P0 psui: a newer regen waits for the stale one), CRASH-MARKER-COPIES (P2 dx), API-REMOVE-DIR
  raised to P1 (breaks scripted cuts; featmatrix fallout), DRAFT-HANG second occurrence.
- Checked = live: Line-angle plane normals (30° / 120° exactly live's), transform ×5, split, mid plane, UI mirror,
  mass properties panel (no material: CoM blank + toast), delete a used sketch (dependent red, undo restores), inline
  rename, new document from the Documents window → sketch (R) → extrude → fillet (volume exact), UI fuzz 2×700 clean.

## Green bcf401ca (2026-10-27 04:10–04:40)
- Verified: OPEN-ERROR-SILENT (my 16cdcf6d reopen was wrong: that build lacked 3dfa5710), SKETCH-PASTE, PERF-SECTION-PANIC,
  MENU-SKETCH-TAG, crash documents in the app, TWO-PROCESS-LOST-EDIT, API-REMOVE-SCOPE, USER_GUIDE; P1-16 / P1-6 shell on the
  watcher CLI 04:06 (live's numbers).
- New: API-REMOVE-DIR (P2 dx), EXPORT-MESH-SHARED (P2 dx: 3MF/OBJ not welded); FILLET-HANG gained a chamfer repro
  (bin/chrepro.py + tickets/cli-chamfer-hang-*). STL export OK (watertight, −0.009 % volume at Medium).
- Watcher CLI copies: rel/k/main/y = 19555ca3, rel/k/main/z = 04:06 build (has the blend budget).

## Green 16cdcf6d (2026-10-27 03:25–03:55)
- Verified: LOFT-PROFILE-LIST (› entries, ↑↓, nothing under the field), DRAWING-SCALE (1:2, centre mark on the hole only).
- Reopened: OPEN-ERROR-SILENT (one-line message right, but the bubble goes away by itself ~6–9 s after launch).
- New: DRAFT-HANG (P0 core, fuzz seed 60 step 76; the app opens the doc rolled back above Draft 8, rolling forward hangs),
  LOFT-SKETCH-HIDE (P2 psui: only the first profile's sketch is hidden).
- REGEN-SPEED re-timed (above). Not yet in green: SKETCH-PASTE fix ac0645bc, P1-16 shell fix 239fa250.

## Verified on green dc3a03f6 (2026-10-27 01:55)
qa5's P1-17 (mass properties blank with nothing measured, no toast), P1-18 (⚠ badge on a missing-selection row), P2-D3
(drawing Note tool previews "NOTE" at the pointer), P1-8 (one "Vertex of Sketch N" chip per clicked hole point), P1-9
(feature-pattern chip "Ø 6.35 mm ↧ 12 mm"), P2-A5 (suppressed mate row italic + strike-through, green 19555ca3);
TAB-NO-PREVIEW (with X focus); API-DERIVED-BY-NAME; crash documents open with the watcher CLI 19555ca3 (app pending
the next green, bf8aaee2). EXT-DIRECTION still open with focus.

## qa5 items (fixed on main by qa5 after green 4d2f6b0c): all verified
P1-8, P1-9, P1-17, P1-18, P2-A5, P2-D3 on dc3a03f6 / 19555ca3 (above). On green 16cdcf6d (2026-10-27 04:05, asm5.noshape:
plate with 4 holes, pin, Cylindrical → Revolute, linear pattern): P1-1 ("Features (9)" with a new sketch on Top open),
P2-A2 (1st Enter in a mate limit applies "30 deg" and keeps the dialog, 2nd accepts), P2-A3 (Esc in explode editing = Done:
toolbar back, unexploded, plain row; double-click re-enters exploded), P2-A4 (pattern copy stays drawn, no z-fighting, while
the seed's mate is edited and retyped), P2-A6 ("Instances (3)" = Part 1, Part 2, Linear pattern 1), P2-A7 (pending
"Replicate 1" row with red dot at open, mate auto-filled), P2-A8 (no "instance(s) created" bubble), P2-D2 (assembly drawing
Update stays dim after a sketch in an unused Part Studio 2; lights up after the plate's depth changes).

## Next
Revolve / patterns / mirror / boolean on the new plate; feature-list reorder/delete/rollback on the 30-feature part;
selection, measure, section view; then P2 assembly and drawing passes; verify the list above once green moves.
