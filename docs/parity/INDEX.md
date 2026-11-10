# The parity index: one number for "how much of Onshape do we match" (lead, 2026-11-07 19:26 UTC)

The user asked for a single tracked percentage. This is it. qa computes it after every 4-hourly suite run
(`tools/parity/index.py`, from the suites' own outputs) and writes one line a day to the table at the end of
docs/parity/SCORE.md and the latest to /tmp/lead/parity-index.

## How it is made

Twelve areas, weighted by how much of a user's time and trust they carry. Each area scores 0–1:

- **Result suites** (a fixed corpus of real Onshape documents): the share that matches live.
- **Interaction areas:** `match × coverage`.
  - match: the share of recorded states that match live;
  - coverage: the share of that area's tools, menu items and shortcuts that have a scenario at all. What we don't
    test doesn't count as matching.
- **An area with no suite yet scores 0.** The index is a lower bound: it rises when we measure more, not only when we
  fix more. Writing the missing suites is part of the work.

| Area | Weight | Score from | Owner |
|---|---|---|---|
| Part Studio geometry | 25 | model suite: features matching live (`parity_models`) | core, blend, bool, dx, skasm |
| Part Studio and sketch interaction | 15 | ui-parity `ps-*`/`sketch-*` states × coverage (tools, menus, shortcuts) | psui, skasm |
| Assembly results | 8 | assembly suite: occurrences placed as live (`parity_assemblies`) | asm |
| Assembly interaction | 7 | ui-parity `asm-*` states × coverage | asm |
| Drawing interaction | 5 | ui-parity `drw-*` states × coverage | dx |
| Drawing export | 5 | exports matching live's files (tools/drawing-export/compare.py) | dx |
| Data exchange | 5 | exchange suite: mean of "ours imports live's" and "live imports ours" | dx |
| Speed against Onshape | 10 | share of timed cases at or under live's time: corpus regenerations (the features' own regeneration time, as live's Regeneration times panel counts), document opens (open to the model shown), the interaction suite's dialog and click latencies (each case 1 if ours ≤ live, else live/ours) | everyone; bool and psui first |
| Feature Studio and FeatureScript editor | 5 | no suite yet: 0 | dx |
| Variable Studio | 3 | no suite yet: 0 | psui |
| Versions, branches, history, release | 5 | no suite yet: 0 | skasm |
| Documents, tabs, start page, preferences | 7 | no suite yet: 0 | psui |

**Index = Σ weight × score.** A suite for an area that has none starts as soon as its first scenarios are recorded by
live and run in qa's 4-hourly run; coverage then grows as the owner writes more.

## Rules

- The weights don't change without the user. New areas take weight from "Documents, tabs, start page, preferences" until
  that is re-split with the user.
- A live quirk excluded from a suite (live's plan limits, account memory, a recorder artefact, an under-measured area)
  is written next to the exclusion with its evidence, as PARITY-SUITES.md already requires.
- The index never goes up by changing the scoring alone: a scoring change is landed with the index recomputed for the
  day before it, both written down.

## Scoring changes

- 2026-11-08 22:40 UTC (lead, psui): **Speed's regeneration cases compare like with like.** Live's `full_regen_ms` is its
  Regeneration times panel (the features' regeneration alone); ours was the wall time of reopening the document,
  listing its features and querying its parts (`query_parts`' mass properties, 0.1-1.6 s on some documents, and our
  display build). Ours is now the feature list's own regeneration time (crates/ops/tests/parity.rs; the wall time stays
  in models-report.json as `regen_ms.wall`); the time to a shown model stays measured by the opens cases. The last
  complete run before it (qa 2026-11-08 14:11, 4b09e98c), recomputed from its models.log (each document slower than
  live is listed there with its feature-list time; the others are at or under live by it): **Speed 70.8 % → 81.7 %**
  (40 → 60 of 98 cases at or under live), **index 78.67 → 79.75**.
- 2026-11-10 22:15 UTC (lead, qa): **Speed is measured on vibe5's EPYC, not vibe3's Xeon** (vibe3 runs our documents 1.5–2.5×
  slower; live's times are absolute). vibe5 is shared, so each document's time carries the host's 1-minute load
  (models-timed.log); a clean measurement is taken in the lead's 02:00–04:00 UTC window. The 15:02 run (e3a19f63)
  recomputed with only its speed stage moved (same index.py, same other areas): **Speed 81.9 % → 90.2 %** (57 → 78 of 97 cases
  at or under live), **index 90.5 → 91.4**, under load 4.1–11.8.
