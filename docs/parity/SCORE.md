# Parity score (qa, daily; docs/PARITY-SUITES.md)

One line per day (UTC date, the green it was measured on): models matched / total and features matched / total
(dx's T2 `parity_models`), scenarios matched / total (psui's runner against live's recordings), then the five biggest
causes of difference, each with its ticket.

| date | green | models | features | scenarios | biggest causes (tickets) |
|---|---|---|---|---|---|
| 2026-11-03 | 834983a7 | 4/11 | 51/78 | – | 2× SKETCH-FACE-IMPRINT (clamp-hms, untitled-document-chris); 2× CUT-CLOSED-EDGES-SPLIT (bushing2-0-joshua, clamp-abhil; pipeflange-steven too); 1× EXTRUDE-OVERLAP-LINES-FACES (bracket120-datenkrake); 1× FILLET-SPLIT-EDGE-FACES (bracket03-mattie) — first run, by dx at 05:05 (7 of live's 18 documents not fetched yet: REST 429) |
