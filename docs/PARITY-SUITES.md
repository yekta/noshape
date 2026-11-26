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
Score: documents that match to the end, and features that match, out of all. Speed: live's latest timing line in live.jsonl (`full_regen_ms`, `per_feature_ms`; the fresh copies keyed by
featureId); ours is our feature list's total, and where that is a gap (>= 100 ms and >= 2x live's) the median of it and
two more runs, each in a fresh process (`time_child`; a reopen in the same process reads caches) (live speed audit 11-26). Each difference that isn't the
translator's becomes a ticket for its area.

**Pattern tracing (core, lead 11-14).** `NOSHAPE_PATTERN_TRACE=1` on any regeneration (parity_models,
parity_assemblies, the app, the CLI) prints `PATTERN <feature>:` lines on stderr for every pattern and mirror: the
path it took (face copies stitched in, one face tool, copied bodies, features re-applied as New tools or one by one,
and why the geometric path fell back), each instance's tool (volume, faces, centroid) and the volume it alone adds to
or takes from its part, and each part's volume before and after. Under the two suite runners the same lines carry
live's change of each part from the line before (`PATTERN <feature>: live part …`). Example: asm-hand-drill-pinslot
studio e53bba's Circular pattern 1, five tools of 22.41 mm³, +112.071 against live's +112.053.

**Area of procedural blend faces (core, lead 11-07).****Area of procedural blend faces (core, lead 11-07).** Live measures the area of its procedural (OTHER) blend faces
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

**Live's own scatter (lead 11-15).** The suite is never tighter than live's measurement of itself: where two of live's
readings of one quantity disagree, the document's parts are compared within the larger relative disagreement, either
way, from the feature those readings concern on (`LIVE_SCATTER`, crates/ops/tests/parity.rs, computed there from the
readings). c2-impeller-loft from Shell Gehäuse: the cap JKK's evArea (1,356.9395 mm², REQUESTS 2484) against Part 2's
volume over the 4 mm it is extruded (5,427.7079 / 4 = 1,356.9270 mm²): 9.2e-6; the offset face JKG's evArea (13,509.2197
mm²) against live's own tessellation of it converging to 13,508.158 (2484): 7.86e-5. So 7.86e-5. Our inner edges lie
within 0.64 µm of live's (2482); topology is compared as usual.
c2-surf-workshop-knifehandle from Sweep 2 (core 11-22): the Blade's area (18,227.2567 mm²) against the sum of its six
faces' evArea (18,225.5247): 9.5e-5. The 4 mm top face is the path spline (238.10967 mm long) swept, exactly 952.43867 mm²:
ours 952.438671, live's evArea 952.4035; ours, 18,225.8113 mm², lies between live's two readings.

**c2-gamma-imported / c2-wire-clip-imported: live's STEP importer refits the PEM FH4-M3 studs (qa, lead 11-12, core's
reason in core.md 11-11).** Live scales each stud's groove torus by 0.995 (its face 4.277452 mm², the exact 4.158573) and reads
24 near-straight B-spline serration edges as circles; ours imports the file exactly: 131.1503 mm³ / 203.1205 mm² against live's
131.1277 / 203.2332 (the M3-8 studs: 60.4651 / 108.8728 against 60.4419 / 108.9854), the same counts. In those two documents a
part named "PEM FH4-M3-…" is held within 0.03 mm³ of live's volume, 0.15 mm² of its area and 3 µm of its centroid (the refit's own
differences, the same on both stud lengths), its edges' kinds not compared, in every feature (the studs stay as imported); its
counts and every other part are compared as usual (`LIVE_IMPORT_REFITS`, crates/ops/tests/parity.rs).

**Straight edges of offset faces (core, lead 11-15).** Live's fetch types an edge LINE only when its tessellation is two
points. Parasolid keeps a face moved by Move face › Offset as an offset surface, so that face's straight edges come back
as longer polylines and are typed OTHER: c2-impeller-loft's Flansch verbreitern gives Part 2 8 line / 10 other edges,
ours 12 / 6, the same volume. A user never sees the difference: live's evCurveDefinition and Measure still call such an
edge a Line (live core/q-moveface). So from a feature named in `LIVE_LINES_AS_OTHER` (crates/ops/tests/parity.rs) on, that
part's lines and others are compared as one count; its other edge kinds, its counts and everything else as usual.

**Edges inside a near-tangent sliver (bool, lead 11-15, DECISIONS).** Where two faces cross at a tiny angle, Parasolid
may bound the crossing by extra short edges through the sliver between them, within its tolerance. c2-impeller-loft's
Boolean Flansch+Gehaeuse: the housing's 3 mm offset wall crosses the flange's wall, the plane y = +-27 (within 0.1 um),
at ~0.01 rad along a 45 degree line that meets their shared top curve at x 63.86 mm; live ends that line at x 63.50 /
29.41 / 61.99 and goes on by 3 short OTHER edges (KO5B 0.346 mm, KOFC 0.688, KOJC 2.307) where the walls lie 0.04-1.7
um apart. Ours is the exact intersection; no later feature references those edges or their vertices (Fillet
Ansaugflansch zum Gehaeuse fillets the z = -4 junction edges next to them; live keeps all three to the end). So from a
feature named in `LIVE_SLIVER_EDGES` (crates/ops/tests/parity.rs) on, that part's live counts are taken less those
edges (as OTHER) and their vertices; everything else is compared as usual. The same table takes a vertex live keeps
splitting one edge into two pieces of the same curve, the faces alike, the volume within its band and nothing
referencing it (lead 11-15, DECISIONS bb69638d4): c2-impeller-loft's Extrude 4 splits Part 3's +y cap edge 0.047 mm
from its corner (0.0469 + 65.4207 mm = ours, one 65.4677 mm edge), kept to the end in Part 1 after Boolean 1.

**Curve pattern along a helix: live frame noise ≤ 4.5 µm, accepted (dx, lead 11-17).**
c2-loft-guide-curves-driven-pattern's Curve pattern 1 places 61 plates, one every 18° along a 3-turn helix. Live's
instances are exact screw copies of the seed at 0° and 180° only. Elsewhere they tilt up to 0.51° about the radius,
which is 4.5 µm across a 0.508 mm plate, and the pattern repeats every half turn. That noise comes from Parasolid's
sweep of curvePattern.fs's 0.1 mm line along the helix's 9° arcs. Ours are exact screw copies: each frame's bend
comes from the fitted helix's axis. The suite is not loosened for this, and live's tilt is not chased. Since 11-17
the document matches to the end (11/11).

**Live's failed imports (dx, lead 11-17: parity, not shortcomings).** asm-robot-relations studio 0c3466's Import 1
reads a SolidWorks STEP whose one solid is a single closed shell (174 faces, every edge on two of them). Live's import
fails to make a solid from it and keeps six sheet bodies named REV-21-1650 (147 faces, no volume). Ours makes a valid
solid (BRepCheck passes, no self-intersection), 171986.6 mm³. From Import 1 on, live's sheets and our solid of that
name are left out of the comparison (`LIVE_IMPORT_FAILURES`, crates/ops/tests/parity.rs). Thicken 1's part is compared
as usual; its face of live's sheet JFD maps to the same face of our solid by geometry (asm's translator).

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
Where live's own import of a file splits its model's edges or faces, a row (ours importing live's file, or live
importing ours) passes when its counts lie between the model's and live's own import's, part for part, its volume
and area within the model's (live importing ours) or live's own import's of that file (ours importing live's: the
file's own approximations; lead 11-17: live splitting more is live's shortcoming, not a target; c2-rings-loft's
live.igs: ours 275 faces, live's own import 279, the model 273).
Its last line is the score: `exchange score: ours imports live's files m/n; live imports ours m/n; ours reads ours
back m/n`.

Live's mesh imports that make sheets (lead 11-17, as robot-relations' import): where live's import of its own STL /
OBJ / 3MF of a good closed mesh gives sheet bodies and ours gives valid solids, that is live's failure, not a target.
Those rows hold ours to the model's solids, each volume within its area times live's STL chord (`LIVE_MESH_SHEETS`):
c2-bottle-split (two solid halves touching along edges; live 148 sheets, ours the model's 2) and c2-duct-multipart
(live 9 sheets; ours 2 solids for the model's 4 overlapping parts, a difference still counted). The option rows of
those documents' mesh exports are held the same way, the model sized by the case's import unit over its file's (lead
11-26: the duct's STL options, live 9 sheets; its fine mm STL imported in Centimeter or Foot). Formats that can't
hold a mesh part (IGES, ACIS: live drops it) are compared with live's own import of that file, held to the parts it
keeps.

Reading our own files back (lead 11-14): the parts read back are compared with the model's, counts exact. A volume
within the largest edge tolerance (of the model's parts or the file's) times the part's area, and an area within it
times the edges' total length (`query_parts`' `edge_tolerance`, `edge_length`), is no difference. Our models carry
intersection edges up to 3e-5 m off their faces (OCCT's approximations): a file writes them as tolerant edges that
the reader fits again, which moves the volume by 1-2e-6, invisible to a user. Making those edges exact is a modelling
ticket (TICKETS PS-INEXACT-INTERSECTION-EDGES), not the suite's. A mesh read back is held to its total volume within
1 % of live's import of the same file where live reads it as solids, else of the model's (lead 11-26: live reads our
bottle's and duct's STL and the rings' OBJ with sheets).

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
