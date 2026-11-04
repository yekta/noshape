# Parity score (qa, daily; docs/PARITY-SUITES.md)

One line per day (UTC date, the green it was measured on): models matched / total and features matched / total
(dx's T2 `parity_models`), scenarios matched / total (psui's runner against live's recordings), then the five biggest
causes of difference, each with its ticket.

| date | green | models | features | scenarios | biggest causes (tickets) |
|---|---|---|---|---|---|
| 2026-11-03 | dx 21:29 (run 45; 3 dead-studio documents unscored; prize-wheel-tim refetched, its Hole as live) | 26/55 | 303/480 (custom 7/109) | – |
| 2026-11-03 | dx 22:49 (run 47: valuesSortedById in feature order, cf-parameter-pattern 9/9; old studios' lowercase types, id-only faces by tessellated area: cf-o-ring-generator 10/11) | 27/55 | 308/480 (custom 12/109) | – |
| 2026-11-03 | dx 23:17 (run 48: pinned studio imports, opSplitFace edges / leaveOpen / makeSolid in the host: cf-setback-fillet runs; custom-failing documents 8 → 6) | 27/55 | 308/480 (custom 12/109) | – |
| 2026-11-04 | dx 05:56 (run 53: a pattern instance id past the pattern's last is its seed (stef-radijator 3 -> 5/6); speed now compares features' times) | 29/55 | 321/480 (custom 17/109) | – |
| 2026-11-04 | dx 08:12 (run 56: live 1903's sketch wire bodies and REST ids for the 29 cf-* documents; a sketch line by id is that line (exercise-chino 9/9); the host runs cf-fill-pattern's Fill patterns (custom-failing documents 10 -> 8)) | 29/56 | 331/482 (custom 18/111) | – |
| 2026-11-04 | dx 08:28 (run 57: on main with core's latest (knob-jason 11/11); qCreatedBy's face creators cached, attributes indexed) | 30/56 | 332/482 (custom 18/111) | – |

dx 11-03 18:15: three documents are left out of the model score (tests/parity-models/unscored.json, printed as "not
scored" by `parity_models`): cf-ansi-washers, cf-socket-head-cap-screw-creator and cf-thread-creator. Their Feature
Studios import studios of documents that are no longer public (live gets 403 fetching them, REQUESTS 1885), so their
custom features can't be built in noshape or anywhere else; they measure nothing about us.

qa 11-03 05:55: the scenario column counts the scenarios with a live recording at compare time (112 of 162 runs; live was
re-recording); the evening run replaces today's numbers. Scenario differences are psui's (its runner reports its own
score), first-difference keys today: dialog field kinds / labels (toggle vs boolean, "Distance 1" vs "Distance"), feature
row states, selection after a step.
qa 11-03 08:04: scenarios on green 7d35f2cb against live's 184 recordings: **101/184 scenarios, 720/945 states**
(qa-scratch/uip-score3.log). First-difference causes: the sketch plane pick as an undo step (33 scenarios, SKETCH-UNDO-REDO-STATE
reopened), dialog field kinds (enum vs boolean), feature row states, a new dialog's Undo (NEW-DIALOG-UNDO-STEP).
qa 11-03 14:30: on green f5707754 against live's 63 re-recorded scenarios: **61/63 scenarios, 463/465 states**; the two left
are SKETCH-TOOL-PICK-SELECTION (P3).
qa 11-03 19:22: on green c4d5d8af against live's 128 recordings: **116/128 scenarios, 892/932 states** (psui's 17:30
run 114/128). Left, all already owned: 7 fillet-* (live's runner leaves Entities to fillet empty: REQUESTS 1850 / 15:05),
sketch-circle-at-origin-coincident (now VERTICAL vs our COINCIDENT at step 5), sketch-construction-toggle, sketch-fully-defined,
sketch-extend-to-line (stale recording, re-record asked) — skasm.md; extrude-end-type-up-to-face (live's start) — psui.md.
qa 11-03 21:16: the 12 left re-run on green ece6c4c8 against live's re-recordings (tests/ui-parity/live): sketch-construction-toggle
matches now (→ 117/128); sketch-fully-defined now differs only at step 13 (focus: live none, ours other); the 7 fillet-* re-recorded at
20:25 still open with Entities to fillet empty in live (REQUESTS 1850 / 15:05 stays open).
qa 11-03 23:02: on green 9cb98b67 against live's 136 recordings in tests/ui-parity/live: **124/136 scenarios, 967/997 states**.
Left: 7 fillet-* and extrude-end-type-up-to-face (live's recordings, as before), sketch-extend-to-line (stale, re-record asked),
sketch-fillet-corner (fixed by skasm 2e0485c2, after this green), sketch-delete-line, sketch-equal (psui's notes to skasm).
qa 11-04 04:14: on green ad60edf9 against live's 286 recordings: **259/286 scenarios, 1734/1818 states** (undo-rename
counted after a re-run: the first run's app couldn't connect to the X display at start, a harness hiccup; it matches, 6/6). Left:
fillet / chamfer "error" titles (10, incl. new-fillet-then-dblclick-other, undo-new-fillet: live's preselected edge, REQUESTS
1850), hole-* (6: live's account remembers M6x1.00 ↧ 15 mm, behaviours 1849 item 1, a neutral start pending), measure-fillet-face
(POINT vs EDGE), sketch-delete-line / -vertical-constraint / -extend-to-line (selection), revolve-dialog-axis, face-rclick-menu,
mass-properties-part (Mass override enabled), section-view-front (focus), extrude-end-type-up-to-face (live's start).
