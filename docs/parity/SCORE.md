# Parity score (qa, daily; docs/PARITY-SUITES.md)

One line per day (UTC date, the green it was measured on): models matched / total and features matched / total
(dx's T2 `parity_models`), scenarios matched / total (psui's runner against live's recordings), then the five biggest
causes of difference, each with its ticket.

| date | green | models | features | scenarios | biggest causes (tickets) |
|---|---|---|---|---|---|
| 2026-11-03 | dx translator 18:45 (run 40; 3 dead-studio documents unscored) | 25/54 | 296/421 (custom 6/55) | – | 6× custom studios not compiling (3 import studios live can't fetch: 403, REQUESTS 1885; FS-OLD-STD-COMPAT); 5× fillets (FILLET-PROPAGATION-VOLUME, FILLET-OVERLAP-FAIL, FILLET-KNOB-JASON-VOLUME, FILLET-MEETING-FILLETS-ONE-FACE, FILLET-BLEND-SPLIT); 3× custom features failing to run (core); 2× live's UI fetch without geometry / in-feature mate connector queries (asked, 1892 / 13:20); 1× EXTRUDE-SURFACE-CURVES, MIRROR-FEATURE-UP-TO. Speed: 16 of 44 slower than live (PARITY-SPEED: cf-hose-routing ×230, cf-planetary-gears ×30, knob-jason ×29; vibe5 loaded during the run). cf-parameter-pattern flips between 9/9 and 8/9 run to run (FS-FEATURE-LIST-NONDETERMINISTIC, core); clamp-guillem 6 → 23 of 25. |

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
