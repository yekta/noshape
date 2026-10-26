# QA handoff (agent qa, from 2026-10-26 evening)

qa tests the newest green build (/root/green/noshape, commit in /root/green/SHA) against live Onshape and files tickets
in docs/TICKETS.md. It never builds. Prompt: /root/lead-scratch/prompt-qa.md.

## Daily release smoke
(none yet: the first daily release under the new plan is due 2026-10-27 ~05:00 UTC)

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
- Mind dialog layout shifts: once a selection chip is added, every row below moves down ~16 px; screenshot before
  clicking a field.

## Tested
### 2026-10-26, green 4d2f6b0c (13:26), box load 35–40
- 30-feature part: open (full regen 970 ms), edit Extrude 1 → commit 1007 ms (REGEN-SPEED), undo/redo 7/5 ms, mass
  properties 60779.484 mm³ = live.
- New studio: centre rectangle + Ø20 circle, dims, extrude 10, fillet top face R2, chamfer 2 bottom edges 1 mm, 2 sketch
  points + Hole, Shell top face → SHELL-WALLS (P0) and SHELL-SLOW (P0). Same plate without the hole shells fine.
- Checked and same as live: R = centre-point rectangle, Shift+C = Curve/surface analysis (no Chamfer shortcut), Enter in
  Depth keeps the dialog, Esc in an open end-type list closes only the list, Hole dialog defaults (Inch tab, 1/4, 12 mm,
  118 deg), "Features …" + red badge while a new feature is invalid.

## Open findings (mine) by area
- core: SHELL-WALLS (P0), SHELL-SLOW (P0), REGEN-SPEED (P0); carried over qa5's P1-16 / P1-6.
- psui: carried over qa5's P1-8 caveat, P1-10.
- skasm: carried over qa5's P1-4 data.

## To verify on the next green (fixed on main by qa5 after green 4d2f6b0c)
P1-1, P1-8, P1-9, P1-17, P1-18, P2-A2…A8, P2-D2, P2-D3 (docs/handoff/qa5-routing.md "qa5 open items at the 2026-10-26
stop"; P2-A3 / A7 / A8 have no unit test: Esc in explode editing, the pending Replicate row, no "instance(s) created"
bubble).

## Next
Revolve / patterns / mirror / boolean on the new plate; feature-list reorder/delete/rollback on the 30-feature part;
selection, measure, section view; then P2 assembly and drawing passes; verify the list above once green moves.
