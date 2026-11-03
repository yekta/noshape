# Parity score (qa, daily; docs/PARITY-SUITES.md)

One line per day (UTC date, the green it was measured on): models matched / total and features matched / total
(dx's T2 `parity_models`), scenarios matched / total (psui's runner against live's recordings), then the five biggest
causes of difference, each with its ticket.

| date | green | models | features | scenarios | biggest causes (tickets) |
|---|---|---|---|---|---|
| 2026-11-03 | (dx 12:43) | 12/31 | 135/207 (custom 3/25) | – | 3× CUT-CLOSED-EDGES-SPLIT family (bool / core: bushing2, motor-mount-jack) + FILLET-PROPAGATION-VOLUME (pipeflange); 1× ADD-INSIDE-OUT (core / bool: cf-parameter-pattern); 1× MIRROR-FEATURE-UP-TO (clamp-hms); 1× FILLET-OVERLAP-FAIL (learning-center-michael); custom: FS-OLD-STD-COMPAT (checkInput at 392…), FS-ANYTHING-PARAM, FS-WAVE-SPRING (inside out, 157 s), studios' imports and ids-only bodies waiting on live (REQUESTS 11:05 / 11:15) — dx 12:43: live mid-refetch |

qa 11-03 05:55: the scenario column counts the scenarios with a live recording at compare time (112 of 162 runs; live was
re-recording); the evening run replaces today's numbers. Scenario differences are psui's (its runner reports its own
score), first-difference keys today: dialog field kinds / labels (toggle vs boolean, "Distance 1" vs "Distance"), feature
row states, selection after a step.
qa 11-03 08:04: scenarios on green 7d35f2cb against live's 184 recordings: **101/184 scenarios, 720/945 states**
(qa-scratch/uip-score3.log). First-difference causes: the sketch plane pick as an undo step (33 scenarios, SKETCH-UNDO-REDO-STATE
reopened), dialog field kinds (enum vs boolean), feature row states, a new dialog's Undo (NEW-DIALOG-UNDO-STEP).
qa 11-03 14:30: on green f5707754 against live's 63 re-recorded scenarios: **61/63 scenarios, 463/465 states**; the two left
are SKETCH-TOOL-PICK-SELECTION (P3).
