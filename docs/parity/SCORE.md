# Parity score (qa, daily; docs/PARITY-SUITES.md)

One line per day (UTC date, the green it was measured on): models matched / total and features matched / total
(dx's T2 `parity_models`), scenarios matched / total (psui's runner against live's recordings), then the five biggest
causes of difference, each with its ticket.

| date | green | models | features | scenarios | biggest causes (tickets) |
|---|---|---|---|---|---|
| 2026-11-03 | (dx 11:43) | 11/20 | 112/146 (custom 0/3) | – | 3× CUT-CLOSED-EDGES-SPLIT family (bool / core: bushing2 Extrude 4, motor-mount-jack Extrude 2, pipeflange Fillet 2); 1× MIRROR-FEATURE-UP-TO (core: clamp-hms); 1× FILLET-OVERLAP-FAIL (core: learning-center-michael); knob-kraut Chamfer 1, cf-circular-pattern's custom pattern and prize-wheel's Hole INFO wait on live (bodies with geometry, the INFO text: REQUESTS 11:12 / 11:15) — dx 11:43: live mid-refetch (documents without live.jsonl are not scored); core's fixes moved clamp-hms and learning-center-michael on; FeatureScript corpus studios 13/34 compile |

qa 11-03 05:55: the scenario column counts the scenarios with a live recording at compare time (112 of 162 runs; live was
re-recording); the evening run replaces today's numbers. Scenario differences are psui's (its runner reports its own
score), first-difference keys today: dialog field kinds / labels (toggle vs boolean, "Distance 1" vs "Distance"), feature
row states, selection after a step.
qa 11-03 08:04: scenarios on green 7d35f2cb against live's 184 recordings: **101/184 scenarios, 720/945 states**
(qa-scratch/uip-score3.log). First-difference causes: the sketch plane pick as an undo step (33 scenarios, SKETCH-UNDO-REDO-STATE
reopened), dialog field kinds (enum vs boolean), feature row states, a new dialog's Undo (NEW-DIALOG-UNDO-STEP).
