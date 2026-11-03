# Parity score (qa, daily; docs/PARITY-SUITES.md)

One line per day (UTC date, the green it was measured on): models matched / total and features matched / total
(dx's T2 `parity_models`), scenarios matched / total (psui's runner against live's recordings), then the five biggest
causes of difference, each with its ticket.

| date | green | models | features | scenarios | biggest causes (tickets) |
|---|---|---|---|---|---|
| 2026-11-03 | d3627daf (dx 14:30) | 17/51 | 217/393 (custom 3/50) | – | 10× live's UI fetch without face / edge geometry or with lagging lines (asked, live 1892 / 13:20; half refetched); 4× SKETCH-COLLINEAR-EDGES-MERGE (core: learning-center-frank, exercise-martinl, clamp-guillem, motor-mount-jack); 4× custom studios not compiling (FS-OLD-STD-COMPAT, unfetched imports); 4× fillets (FILLET-PROPAGATION-VOLUME, FILLET-OVERLAP-FAIL, FILLET-KNOB-JASON-VOLUME, FILLET-MEETING-FILLETS-ONE-FACE); 2× SKETCH-FACE-FRAME (cf-hose-routing, learning-center-devon); also CUT-CLOSED-EDGES-SPLIT, MIRROR-FEATURE-UP-TO, EXTRUDE-SURFACE-CURVES, FILLET-BLEND-SPLIT, FS-WAVE-SPRING. Assemblies 8/8, mates 11/11 (with live's asm-hinge-live) |

qa 11-03 05:55: the scenario column counts the scenarios with a live recording at compare time (112 of 162 runs; live was
re-recording); the evening run replaces today's numbers. Scenario differences are psui's (its runner reports its own
score), first-difference keys today: dialog field kinds / labels (toggle vs boolean, "Distance 1" vs "Distance"), feature
row states, selection after a step.
qa 11-03 08:04: scenarios on green 7d35f2cb against live's 184 recordings: **101/184 scenarios, 720/945 states**
(qa-scratch/uip-score3.log). First-difference causes: the sketch plane pick as an undo step (33 scenarios, SKETCH-UNDO-REDO-STATE
reopened), dialog field kinds (enum vs boolean), feature row states, a new dialog's Undo (NEW-DIALOG-UNDO-STEP).
qa 11-03 14:30: on green f5707754 against live's 63 re-recorded scenarios: **61/63 scenarios, 463/465 states**; the two left
are SKETCH-TOOL-PICK-SELECTION (P3).
