# Parity score (qa, daily; docs/PARITY-SUITES.md)

One line per day (UTC date, the green it was measured on): models matched / total and features matched / total
(dx's T2 `parity_models`), scenarios matched / total (psui's runner against live's recordings), then the five biggest
causes of difference, each with its ticket.

| date | green | models | features | scenarios | biggest causes (tickets) |
|---|---|---|---|---|---|
| 2026-11-03 | (dx 09:45) | 10/14 | 72/93 | – | 2× CUT-CLOSED-EDGES-SPLIT, what is left after core's e77387ec (bushing2-0-joshua Extrude 4 18/42/26 vs 18/40/24, pipeflange-steven Extrude 3 24/46/20 vs 24/44/16); 1× EXTRUDE-UP-TO-FACE-BEYOND (core: clamp-hms); 1× EXTRUDE-SLIT-REGION (core: learning-center-michael) — dx 09:45: live's new batch, documents live fetched whole only (10 more have 429 lines, scored after the refetch); bearing-block-takumi, learning-center-ivan, clamp-abhil match after translator fixes; knob-jason (unscored) has FILLET-31S-FAIL |

qa 11-03 05:55: the scenario column counts the scenarios with a live recording at compare time (112 of 162 runs; live was
re-recording); the evening run replaces today's numbers. Scenario differences are psui's (its runner reports its own
score), first-difference keys today: dialog field kinds / labels (toggle vs boolean, "Distance 1" vs "Distance"), feature
row states, selection after a step.
qa 11-03 08:04: scenarios on green 7d35f2cb against live's 184 recordings: **101/184 scenarios, 720/945 states**
(qa-scratch/uip-score3.log). First-difference causes: the sketch plane pick as an undo step (33 scenarios, SKETCH-UNDO-REDO-STATE
reopened), dialog field kinds (enum vs boolean), feature row states, a new dialog's Undo (NEW-DIALOG-UNDO-STEP).
