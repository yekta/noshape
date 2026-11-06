# Parity suites (lead, 2026-11-02)

The checklists in docs/parity/ say 99.9 % of 3791 behaviours match live, yet the user finds differences in minutes
of real use. They hold what agents audited, mostly after fixing it. Two suites measure parity instead: real Onshape
models rebuilt from their parameters, and scripted interactions run in both apps. Each gives a daily score that qa
records in docs/parity/SCORE.md.

## 1. Model suite: Onshape documents rebuilt in noshape

**Corpus (live).** Public Onshape documents and Onshape's learning material, copied into our live account. Start with
20 Part Studios that use only P1 features (sketch, extrude, revolve, fillet, chamfer, hole, shell, pattern, mirror,
boolean, plane), then sheet metal, surfaces, configurations, variables, custom features, assemblies. Per document,
fetched over REST from live's page into `/root/parity/models/<name>/` (never in the repo: other people's work):
- `source.json`: document / workspace / element ids, URL, licence, what it uses;
- `features.json`: the feature list as the API returns it (every parameter and query), the sketches, variables,
  configuration;
- `live.jsonl`: one line per feature, the rollback bar after it: status and message, part count, and per part
  faces / edges / vertices, volume, surface area, centre of mass, bounding box; live's full regeneration time.
The repo keeps only `tests/parity-models/index.json` (name, ids, URL) so anyone can fetch them again.

**Translator (dx).** Onshape's feature JSON → a noshape document through crates/ops, the same path as the CLI:
features and their parameters by FeatureScript name, sketches with their constraints, variables, configurations,
and Onshape's queries mapped onto our references. A test tool, not a user feature. What it can't translate it
reports by feature and reason ("query kind X not mapped"), never silently drops.

**Runner and score (dx, qa).** A T2 test `parity_models` translates every document under /root/parity/models,
regenerates feature by feature and compares with live.jsonl within live's tolerance band (counts exact; volume and
area 1e-6 relative or live's band; centre and box 1e-6 m). Per document: the first feature that differs and how.
Score: documents that match to the end, and features that match, out of all. Each difference that isn't the
translator's becomes a ticket for its area.

## 2. Interaction suite: the same steps in live and in noshape

**Scenarios (qa, live).** `tests/ui-parity/<name>.json`: short scripts of what a user does: keys, typed text, clicks
on named controls (a toolbar tool, a dialog field, a feature list row, a list entry, a point on a face), with a
starting document. Example: open Extrude, click Depth, type `#wall`, press Enter.

**State, after every step.** The same fields in both apps: the focused control, the open dialog and each field's
text and state, any open list or menu with its rows and the highlighted one, the selection, the feature list
(names, rollback bar, errors), the message bar.

**Live side (live).** A Playwright script over CDP runs the scenario in live's Part Studio and reads that state from
the DOM; it writes `tests/ui-parity/live/<name>.json` (the observed states; values only, no assets).

**noshape side (psui).** A driver in the app: `NOSHAPE_UI_SCRIPT=<scenario>` runs the steps through the real input
path (keys and clicks dispatched to the window, controls found by name) and writes the same state after each step;
`NOSHAPE_UI_STATE_OUT=<file>`. Then a runner compares step by step and names the first step that differs. T1 once
it is fast and deterministic (docs/TESTS.md), T2 until then.

**Growth.** Every difference anyone notices (the user, qa, an agent) becomes a scenario first, then a fix, so it stays
fixed. qa writes scenarios from everyday sessions: sketch, dimension, extrude with variables, fillet, edit, rollback,
undo, measure, the variable autocomplete, the feature list's keys. Target: 100 scenarios by 11-05, 300 by 11-09.

## 3. Exchange suite: files between live and noshape (dx, lead 11-06)

Per document under `/root/parity/exchange/<doc>/` (a corpus document's name, never in the repo):
- live's exports of its Part Studio, all visible parts, mm, Onshape's export defaults otherwise: `live.step`,
  `live.x_t`, `live.igs`, `live.stl` (binary);
- `live-self.json`: live's own import of each of those files, `{"STEP": {"parts": [{"name", "bodyType", "faces",
  "edges", "vertices", "volume" mm³, "volume_band" [lo, hi] mm³, "area" mm², "area_band"}]}, "PARASOLID": …,
  "IGES": …, "STL": …}` (massproperties' bands when it gives them);
- `ours.step` / `ours.x_t` / `ours.igs` / `ours.stl`: ours, written by the run (the corpus document rebuilt from
  `/root/parity/models`); `live-ours.json`: live's import of each, in live-self.json's form.

`cargo test -p noshape-ops --test all parity::exchange_round_trip -- --ignored --nocapture` (NOSHAPE_EXCHANGE,
NOSHAPE_PARITY_MODELS, NOSHAPE_PARITY_ONLY as the model suite): ours imports each live file and its parts are compared
with live-self.json's (counts exact, volume and area within live's band or 1e-6; an STL's mesh by its parts, volume
and area only); then ours is exported, and live-ours.json, once given, is compared with live-self.json the same way.
Its last line is the score: `exchange score: ours imports live's files m/n; live imports ours m/n`.

## Score (qa, daily, docs/parity/SCORE.md)
One line per day: models matched / total, features matched / total, scenarios matched / total, and the five biggest
causes of difference with their tickets.

qa's daily routine for the scenarios column (lead 11-06): on the morning's green, from a checkout at that commit on an
idle 16-core host (vibe5), `tools/ui-parity/shards.sh /root/green/noshape /tmp/uip-$(date -u +%m%d)` (all scenarios
with a live recording, 6 shards, ~4 minutes; the binary copied over if it runs elsewhere). Its last lines are the
scenarios that differ, `score: m/n scenarios, s/t states` and the differences by key: write m/n (s/t states) in the
day's line, each differing scenario's cause with its owner's ticket in the causes column. A scenario that differs only
now and then is flaky: run it alone 4 times (`tools/ui-parity/run.py --app … --display … <name>`) and ticket it.

## First milestones (check 11-03 evening)
- live: 10 P1 documents fetched with live.jsonl; 20 scenarios recorded in live.
- dx: the translator takes the first documents end to end; `parity_models` runs and gives a first score.
- psui: the driver and the state dump landed; the 20 scenarios run in ours; a first score; `#wall` fixed.
- qa: SCORE.md with the first line; scenarios for its last week of findings.
