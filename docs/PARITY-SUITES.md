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
  A body's edge `length` there is its tessellation's (a chord polyline), not the curve's: c2-fan-blade-sweep's
  Helix 1 reads 179.687 mm where live's evLength is 179.740 (live 2440). Don't compare curves by it.
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

**Area of procedural blend faces (core, lead 11-07).** Live measures the area of its procedural (OTHER) blend faces
about 0.22 % low, so a part's area band is widened above by 0.22 % of the area of our faces on other surfaces
(`LIVE_FREEFORM_AREA_SHORT`, `PartState::freeform_area`). Evidence: c2-extension-spring's Curvature rounds
(magnitude 0.5) read 0.8113 mm² (R 0.5 × 1 mm) and 2.4341 mm² (R 1.5 × 1 mm) in live, ours 0.8131 / 2.4393. Live's own
exact section for a Curvature fillet of magnitude 0.5 (behaviours.md G11 (a): poles (1, 0) (0.85, 0) (0.7, 0)
(1/3, 0.1) (0.1, 1/3) (0, 0.7) (0, 0.85) (0, 1) in setbacks, degree 5, knots 0⁶ ½² 1⁶) has an arc length of 1.62622 per
unit setback: × R × depth gives 0.8131 and 2.4393 to four digits, and live's face boxes show the full setbacks. So
ours is the exact area; live's reading of it is 0.22 % short.

**c2-impeller-loft's Shell Gehäuse: live's volume reading (qa, lead 11-12).** After Shell Gehäuse (feature 11) live reads
Part 1's volume as 45,272.919 mm³, at evVolume's HIGH accuracy and in massproperties alike; ours is 45,277.36. Live's own
tessellation of the same part converges to about 45,277.97 mm³ and 32,144.50 mm² (REQUESTS 2484; ours 32,144.49 mm²), so
evVolume mis-integrates the 3 mm offset face JKG; its evArea (32,145.58 mm²) is off the same way, and the centroid moves with
them (5-7 µm). From that feature on (JKG stays in Part 1), Part 1 may lie up to 5.05 mm³ above live's volume, 1.2 mm² below its area and 10 µm from its
centroid (`LIVE_VOLUME_ALLOWANCES`, crates/ops/tests/parity.rs); its topology and everything else is compared as usual.

**c2-gamma-imported / c2-wire-clip-imported: live's STEP importer refits the PEM FH4-M3 studs (qa, lead 11-12, core's
reason in core.md 11-11).** Live scales each stud's groove torus by 0.995 (its face 4.277452 mm², the exact 4.158573) and reads
24 near-straight B-spline serration edges as circles; ours imports the file exactly: 131.1503 mm³ / 203.1205 mm² against live's
131.1277 / 203.2332 (the M3-8 studs: 60.4651 / 108.8728 against 60.4419 / 108.9854), the same counts. In those two documents a
part named "PEM FH4-M3-…" is held within 0.03 mm³ of live's volume, 0.15 mm² of its area and 3 µm of its centroid (the refit's own
differences, the same on both stud lengths), its edges' kinds not compared, in every feature (the studs stay as imported); its
counts and every other part are compared as usual (`LIVE_IMPORT_REFITS`, crates/ops/tests/parity.rs).

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

## 4. Assembly suite: Onshape assemblies rebuilt in noshape (asm, lead 11-06)

**Corpus (live).** 15–20 public Onshape assemblies with real mates (every mate type, limits and offsets,
subassemblies, patterns, groups, relations, configurations, standard content), copied into our account and fetched
into `/root/parity/models/asm-<name>/` (never in the repo): `assembly.json` (the REST definition with each
occurrence's solved transform, fixed / hidden / suppressed, patterns and subassemblies; `features` with each mate's
status; `subAssemblyFeatures`; the assembly's `configuration`), `studios/<element>.json` + `.live.jsonl` for each Part
Studio an instance uses (`<element>@<n>` per non-default configuration), `bom.json` (indented and flat).

**Runner and score (asm).** The T2 `parity_assemblies` (crates/ops/tests/parity.rs, `rebuild_assembly` in
crates/ops/src/parity.rs) rebuilds each through crates/ops: the Part Studios by dx's translator, the instances, mates
between the same connectors; each occurrence placed where live has it and held while the mates are added, then freed.
Matched: an occurrence ours places within 1e-6 m / 1e-6 of live's that our mates don't move; a mate whose status is
live's; a BOM row equal to live's. Score per day in docs/parity/SCORE.md ("Assemblies"); each difference that isn't
the translator's becomes a ticket.

## The parity index (lead 11-07)

One overall percentage, weighted over twelve areas, computed by qa from these suites: docs/parity/INDEX.md.

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
